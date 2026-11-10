//! Home widgets: layout, data and customization.
//!
//! The layout persists through the core's validated Home layout (shared with
//! the WebView host; entries this host does not show are preserved). Cards
//! read existing records only: gameplay history for playtime, servers and
//! worlds, and the already-loaded instance state for details. Nothing polls;
//! data refreshes when Home's selection or an instance changes.

use std::cell::RefCell;

use slint::{ComponentHandle, Model};

use crate::api::{self, HomeLayout, WidgetPlacement};
use crate::{ServerRow, WidgetChoice, WidgetLine, WidgetView, Widgets, symbols, ui};

// Each tab has ten entries; their union fits the existing 20-target enrichment API.
const SERVER_ROWS: usize = 10;

/// The widgets this host renders, in their default order.
const NATIVE: [(&str, &str, &str); 5] = [
    ("playtime", "Playtime", "schedule"),
    ("recent-servers", "Servers", "dns"),
    ("recent-worlds", "Singleplayer worlds", "landscape"),
    ("instance-details", "Instance details", "info"),
    ("media", "Media", "music_note"),
];
/// Shown on a Home that was never customized.
const DEFAULT_ON: [&str; 2] = ["playtime", "recent-servers"];

#[derive(Default)]
struct State {
    layout: Option<HomeLayout>,
    data: std::collections::HashMap<&'static str, Vec<WidgetLine>>,
    loading: bool,
    serial: u64,
    instance: Option<String>,
    columns: i32,
    saving: bool,
    servers: Vec<api::RecentTarget>,
    presentations: Vec<api::ServerPresentation>,
    server_icons: std::collections::HashMap<String, slint::Image>,
    server_error: bool,
    server_enriching: bool,
    favorite_pending: std::collections::HashSet<String>,
    favorite_revision: u64,
    server_launch: Option<String>,
    server_favorites: bool,
    saved_servers: bool,
    saved_worlds: bool,
    saved_errors: std::collections::HashMap<&'static str, String>,
    refreshing_servers: bool,
    server_status: std::collections::HashMap<String, (String, slint::Image)>,
}

impl State {
    fn begin_favorite(&mut self, id: &str) -> Option<bool> {
        let favorite = !self.servers.iter().find(|server| server.id == id)?.favorite;
        self.favorite_pending
            .insert(id.to_owned())
            .then_some(favorite)
    }

    fn finish_favorite(&mut self, id: &str, favorite: bool, succeeded: bool) {
        if !self.favorite_pending.remove(id) || !succeeded {
            return;
        }
        self.favorite_revision += 1;
        if let Some(server) = self.servers.iter_mut().find(|server| server.id == id) {
            server.favorite = favorite;
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn bind(window: &crate::AppWindow) {
    let widgets = window.global::<Widgets>();
    widgets.on_server_favorite(|id| favorite_server(id.as_str()));
    widgets.on_server_play(|id| play_server(id.as_str()));
    widgets.on_add(|id| edit(|layout| set_enabled(layout, id.as_str(), true)));
    widgets.on_remove(|id| edit(|layout| set_enabled(layout, id.as_str(), false)));
    widgets.on_reset_layout(|| edit(reset_layout));
    widgets.on_move(|id, delta| edit(|layout| move_widget(layout, id.as_str(), delta)));
    widgets.on_place(|id, index| edit(|layout| place_widget(layout, id.as_str(), index)));
    widgets.on_resize(|id, columns, rows| {
        edit(|layout| {
            if let Some(w) = layout.widgets.iter_mut().find(|w| w.id == id.as_str()) {
                w.grid = Some(api::WidgetGrid {
                    columns: columns.clamp(1, 6) as u8,
                    rows: rows.clamp(1, 3) as u8,
                    position: None,
                });
            }
        })
    });
    widgets.on_columns_changed(|columns| {
        // Slint layout may dispatch while another render owns UI state.
        // Record synchronously: an older queued render must not restore a
        // transient narrow width after a newer layout has already arrived.
        STATE.with(|s| s.borrow_mut().columns = columns.clamp(1, 6));
        let _ = slint::invoke_from_event_loop(render);
    });
    widgets.on_arrange(|id, column, row, width, height, columns| {
        let columns = columns.clamp(1, 6);
        STATE.with(|s| s.borrow_mut().columns = columns);
        edit(|layout| arrange(layout, id.as_str(), column, row, width, height, columns));
    });
    widgets.on_quick_play(|mode, id| crate::home::quick_play(mode.into(), id.into()));
    widgets.on_server_view(|favorites| {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.server_favorites = favorites;
            s.saved_servers = false;
            s.saved_errors.remove("saved-servers");
        });
        crate::ui::with(|window, _| {
            window
                .global::<Widgets>()
                .set_server_tab(if favorites { 1 } else { 0 })
        });
        render();
    });
    widgets.on_saved_view(|worlds, saved| {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if worlds {
                s.saved_worlds = saved;
            } else {
                s.saved_servers = saved;
                s.server_favorites = false;
            }
        });
        refresh();
    });
    widgets.on_refresh_servers(refresh_servers);
    widgets.on_favorite_server(|id, favorite| {
        ui::spawn_api(
            api::favorite_server(ui::host(), id.into(), favorite),
            |result| match result {
                Ok(()) => refresh(),
                Err(error) => ui::error_toast(&error),
            },
        );
    });
    crate::media::bind(window);
}

fn favorite_server(id: &str) {
    let Some(favorite) = STATE.with(|state| state.borrow_mut().begin_favorite(id)) else {
        return;
    };
    let id = id.to_owned();
    render();
    ui::spawn_api(
        api::server_favorite(ui::host(), id.clone(), favorite),
        move |result| {
            STATE.with(|state| {
                state
                    .borrow_mut()
                    .finish_favorite(&id, favorite, result.is_ok())
            });
            if let Err(error) = result {
                ui::error_toast(&error);
            }
            render();
        },
    );
}

fn server_target<'a>(
    entries: &'a [api::RecentTarget],
    id: &str,
) -> api::ApiResult<&'a api::RecentTarget> {
    entries
        .iter()
        .find(|target| target.id == id && target.available)
        .ok_or_else(|| {
            api::ApiError::new(
                "quick_target_stale",
                "This server is no longer available in local history.",
            )
        })
}

fn begin_server_launch(
    state: &mut ui::AppState,
    target: &api::RecentTarget,
) -> api::ApiResult<Option<String>> {
    // Share Home's in-flight launch guard; the core additionally rejects
    // a starting/running child for the stored target's own instance.
    if state.play_busy || state.pending_play.is_some() {
        return Ok(None);
    }
    if state.instance(&target.instance_id).is_none() {
        return Err(api::ApiError::new(
            "quick_instance_missing",
            "The instance for this server no longer exists.",
        ));
    }
    let account = state.accounts.selected_account_id.clone().ok_or_else(|| {
        api::ApiError::new(
            "launch_authentication_required",
            "Sign in to quick play this server.",
        )
    })?;
    state.play_busy = true;
    state.play_progress = Some(format!("Preparing server: {}", target.display_name));
    Ok(Some(account))
}

fn play_server(id: &str) {
    let target = STATE.with(|state| server_target(&state.borrow().servers, id).cloned());
    let target = match target {
        Ok(target) => target,
        Err(error) => {
            ui::error_toast(&error);
            return;
        }
    };
    let request = ui::with(|_, state| begin_server_launch(state, &target));
    let account = match request {
        Some(Ok(Some(account))) => account,
        Some(Err(error)) => {
            ui::error_toast(&error);
            if error.code == "launch_authentication_required" {
                crate::accounts::open_dialog();
            }
            return;
        }
        _ => return,
    };
    STATE.with(|state| state.borrow_mut().server_launch = Some(target.id.clone()));
    crate::home::render();
    render();
    ui::spawn_api(
        api::quick_play_server(ui::host(), target.id, account),
        |result| {
            STATE.with(|state| state.borrow_mut().server_launch = None);
            ui::with(|_, state| {
                state.play_busy = false;
                state.play_progress = None;
            });
            match result {
                Ok(process) => crate::home::on_launch_state(process),
                Err(error) => ui::error_toast(&error),
            }
            crate::home::refresh_readiness();
            render();
        },
    );
}

/// The core's untouched default means "never customized": this host shows
/// its own default (Playtime and Servers) instead.
fn refresh_servers() {
    use base64::Engine;
    let (ids, saved_instance) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.refreshing_servers {
            return (Vec::new(), None);
        }
        let ids: Vec<_> = state
            .data
            .get(if state.saved_servers {
                "saved-servers"
            } else {
                "recent-servers"
            })
            .into_iter()
            .flatten()
            .filter(|row| {
                state.saved_servers
                    || (if state.server_favorites {
                        row.favorite
                    } else {
                        !row.unvisited_favorite
                    })
            })
            .take(20)
            .map(|row| row.target_id.to_string())
            .collect();
        state.refreshing_servers = !ids.is_empty();
        (
            ids,
            if state.saved_servers {
                state.instance.clone()
            } else {
                None
            },
        )
    });
    if ids.is_empty() {
        return;
    }
    render();
    let host = ui::host();
    ui::spawn_api(
        async move {
            match saved_instance {
                Some(instance) => api::refresh_saved_server_status(host, instance, ids).await,
                None => api::refresh_server_status(host, ids).await,
            }
        },
        |result| {
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                state.refreshing_servers = false;
                if let Ok(statuses) = &result {
                    state.server_status = statuses
                        .iter()
                        .map(|row| {
                            let image = row
                                .favicon
                                .as_deref()
                                .and_then(|s| s.strip_prefix("data:image/png;base64,"))
                                .filter(|s| s.len() <= 128 * 1024)
                                .and_then(|s| {
                                    base64::engine::general_purpose::STANDARD.decode(s).ok()
                                })
                                .and_then(|bytes| {
                                    slint::Image::load_from_data(&bytes, Some("png")).ok()
                                })
                                .unwrap_or_default();
                            let label = if row.status == "online" {
                                "Online"
                            } else {
                                "No response"
                            };
                            (row.target_id.clone(), (label.to_owned(), image))
                        })
                        .collect();
                }
            });
            if let Err(error) = result {
                ui::error_toast(&error);
            }
            render();
        },
    );
}

fn default_layout() -> HomeLayout {
    serde_json::to_value(aurora_launcher_lib::home_widgets::HomeLayout::default())
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or(HomeLayout {
            widgets: Vec::new(),
        })
}

fn reset_layout(layout: &mut HomeLayout) {
    let mut reset = effective(default_layout());
    reset.widgets.retain(|widget| is_native(&widget.id));
    reset.widgets.extend(
        layout
            .widgets
            .iter()
            .filter(|widget| !is_native(&widget.id))
            .cloned(),
    );
    *layout = reset;
}

fn effective(saved: HomeLayout) -> HomeLayout {
    let core_default: HomeLayout =
        serde_json::to_value(aurora_launcher_lib::home_widgets::HomeLayout::default())
            .ok()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or(HomeLayout {
                widgets: Vec::new(),
            });
    let mut layout = saved.clone();
    if saved == core_default {
        for widget in &mut layout.widgets {
            if NATIVE.iter().any(|(id, ..)| *id == widget.id) {
                widget.enabled = DEFAULT_ON.contains(&widget.id.as_str());
            }
        }
        // Servers directly after Playtime on a fresh Home.
        let order = |id: &str| NATIVE.iter().position(|(n, ..)| *n == id);
        let mut native: Vec<WidgetPlacement> = layout
            .widgets
            .iter()
            .filter(|w| order(&w.id).is_some())
            .cloned()
            .collect();
        native.sort_by_key(|w| order(&w.id));
        let others: Vec<WidgetPlacement> = layout
            .widgets
            .iter()
            .filter(|w| order(&w.id).is_none())
            .cloned()
            .collect();
        layout.widgets = native.into_iter().chain(others).collect();
    }
    for (id, ..) in NATIVE {
        if !layout.widgets.iter().any(|w| w.id == id) {
            layout.widgets.push(WidgetPlacement {
                id: id.into(),
                enabled: false,
                size: "small".into(),
                grid: None,
            });
        }
    }
    layout
}

fn set_enabled(layout: &mut HomeLayout, id: &str, enabled: bool) {
    if let Some(widget) = layout.widgets.iter_mut().find(|w| w.id == id) {
        widget.enabled = enabled;
    }
    // A newly added widget goes after the visible ones.
    if enabled {
        if let Some(from) = layout.widgets.iter().position(|w| w.id == id) {
            let widget = layout.widgets.remove(from);
            let to = layout
                .widgets
                .iter()
                .rposition(|w| w.enabled && is_native(&w.id))
                .map_or(0, |p| p + 1);
            layout.widgets.insert(to, widget);
        }
    }
}

fn is_native(id: &str) -> bool {
    NATIVE.iter().any(|(n, ..)| *n == id)
}

/// Moves `id` past its visible neighbour (`delta` is -1 or +1).
fn move_widget(layout: &mut HomeLayout, id: &str, delta: i32) {
    let visible: Vec<usize> = layout
        .widgets
        .iter()
        .enumerate()
        .filter(|(_, w)| w.enabled && is_native(&w.id))
        .map(|(i, _)| i)
        .collect();
    let Some(at) = visible.iter().position(|&i| layout.widgets[i].id == id) else {
        return;
    };
    let target = at as i32 + delta.signum();
    if target < 0 || target as usize >= visible.len() {
        return;
    }
    layout.widgets.swap(visible[at], visible[target as usize]);
}

fn place_widget(layout: &mut HomeLayout, id: &str, position: i32) {
    let visible: Vec<_> = layout
        .widgets
        .iter()
        .enumerate()
        .filter(|(_, w)| w.enabled && is_native(&w.id))
        .map(|(i, _)| i)
        .collect();
    let Some(from) = visible.iter().position(|&i| layout.widgets[i].id == id) else {
        return;
    };
    let to = position.clamp(0, visible.len().saturating_sub(1) as i32) as usize;
    if from < to {
        for i in from..to {
            layout.widgets.swap(visible[i], visible[i + 1]);
        }
    } else {
        for i in (to..from).rev() {
            layout.widgets.swap(visible[i], visible[i + 1]);
        }
    }
}

fn edit(change: impl FnOnce(&mut HomeLayout)) {
    let Some(mut layout) = STATE.with(|s| s.borrow().layout.clone()) else {
        return;
    };
    let before = layout.clone();
    change(&mut layout);
    if before == layout {
        return;
    }
    let visibility_changed = before
        .widgets
        .iter()
        .filter(|w| w.enabled)
        .map(|w| &w.id)
        .collect::<Vec<_>>()
        != layout
            .widgets
            .iter()
            .filter(|w| w.enabled)
            .map(|w| &w.id)
            .collect::<Vec<_>>();
    STATE.with(|s| s.borrow_mut().layout = Some(layout.clone()));
    render();
    if visibility_changed {
        refresh();
    }
    save_layout();
}

fn save_layout() {
    let Some(layout) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.saving {
            return None;
        }
        let layout = s.layout.clone()?;
        s.saving = true;
        Some(layout)
    }) else {
        return;
    };
    let submitted = layout.clone();
    ui::spawn(api::set_home_widgets(ui::host(), layout), move |result| {
        let newer = STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.saving = false;
            s.layout.as_ref() != Some(&submitted)
        });
        if let Err(error) = result {
            ui::error_toast(&error);
        }
        if newer {
            save_layout();
        }
    });
}

/// Loads the persisted layout, then the data for the visible widgets.
pub fn load() {
    ui::spawn(api::home_widgets(ui::host()), |result| {
        let layout = effective(result.unwrap_or(HomeLayout {
            widgets: Vec::new(),
        }));
        STATE.with(|s| s.borrow_mut().layout = Some(layout));
        render();
        refresh();
    });
}

fn visible() -> Vec<&'static str> {
    STATE.with(|s| {
        s.borrow()
            .layout
            .as_ref()
            .map(|layout| {
                layout
                    .widgets
                    .iter()
                    .filter(|w| w.enabled)
                    .filter_map(|w| NATIVE.iter().find(|(id, ..)| *id == w.id).map(|n| n.0))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Called when Home's selection or instance state changes. Reads history
/// once per change; Instance details come from state already in memory.
pub fn refresh() {
    let shown = visible();
    crate::media::set_enabled(shown.contains(&"media"));
    let instance = ui::state(|s| s.selected_instance().map(|i| i.id.clone()));
    let wants_history = shown
        .iter()
        .any(|id| matches!(*id, "playtime" | "recent-servers" | "recent-worlds"));
    let (serial, favorite_revision, saved_servers, saved_worlds) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.serial += 1;
        s.instance = instance.clone();
        s.data.remove("saved-worlds");
        s.data.remove("saved-servers");
        s.saved_errors.clear();
        s.loading = wants_history;
        (
            s.serial,
            s.favorite_revision,
            s.saved_servers,
            s.saved_worlds,
        )
    });
    render();
    if !wants_history {
        return;
    }
    let host = ui::host();
    ui::spawn(
        async move {
            let playtime = api::playtime(host.clone(), instance.clone()).await;
            let daily = api::daily_playtime(host.clone(), instance.clone()).await;
            // The local store bounds history to 100 recents plus 100 favorites.
            // Read both before filtering so older favorites cannot be hidden by recents.
            let servers = if shown.contains(&"recent-servers") {
                api::recent_servers(host.clone(), None, 200).await
            } else {
                Ok(Vec::new())
            };
            let worlds = match &instance {
                Some(id) => api::recent_worlds(host.clone(), Some(id.clone()), 32).await,
                None => Ok(Vec::new()),
            };
            let mut saved = Vec::new();
            if let Some(instance) = instance {
                for (recent, key, enabled, world) in [
                    ("recent-servers", "saved-servers", saved_servers, false),
                    ("recent-worlds", "saved-worlds", saved_worlds, true),
                ] {
                    if enabled && shown.contains(&recent) {
                        let result =
                            api::saved_destinations(host.clone(), instance.clone(), world).await;
                        saved.push((key, instance.clone(), world, result));
                    }
                }
            }
            (playtime, daily, servers, worlds, saved)
        },
        move |(playtime, daily, servers, worlds, saved)| {
            let current = STATE.with(|s| s.borrow().serial == serial);
            if !current {
                return;
            }
            let names = ui::state(|s| {
                s.instances()
                    .iter()
                    .map(|i| {
                        (
                            i.id.clone(),
                            (i.display_name.clone(), i.minecraft_version == "1.21.11"),
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>()
            });
            let now = now_secs();
            present_playtime(playtime.ok(), daily.as_deref().ok());
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.loading = false;
                // A history read begun before a favorite save must not restore
                // its stale star state after the successful write.
                if s.favorite_revision == favorite_revision {
                    s.server_error = servers.is_err();
                    s.server_enriching = false;
                    s.servers = server_entries(&servers.unwrap_or_default());
                    s.presentations.clear();
                    s.server_icons.clear();
                }
                for (id, targets) in [("recent-worlds", worlds.unwrap_or_default())] {
                    s.data.insert(
                        id,
                        targets
                            .into_iter()
                            .map(|target| {
                                let (instance, compatible) =
                                    names.get(&target.instance_id).cloned().unwrap_or_default();
                                WidgetLine {
                                    label: target.display_name.into(),
                                    value: if target.last_played_at == 0 {
                                        "Favorite".into()
                                    } else {
                                        relative(target.last_played_at, now).into()
                                    },
                                    detail: if !target.available {
                                        "Destination unavailable".into()
                                    } else if !compatible {
                                        format!("{instance} · Quick Play requires 1.21.11").into()
                                    } else {
                                        instance.into()
                                    },
                                    target_id: target.id.into(),
                                    playable: target.available && compatible,
                                    favorite: target.favorite,
                                    unvisited_favorite: target.last_played_at == 0,
                                    ..Default::default()
                                }
                            })
                            .collect(),
                    );
                }
                for (key, id, worlds, result) in saved {
                    let (instance, compatible) = names.get(&id).cloned().unwrap_or_default();
                    match result {
                        Ok(entries) => {
                            s.data.insert(
                                key,
                                entries
                                    .into_iter()
                                    .map(|entry| {
                                        let value = if worlds && entry.last_played_ms != 0 {
                                            format!(
                                                "{} · {}",
                                                entry.identity,
                                                relative(entry.last_played_ms / 1000, now)
                                            )
                                        } else {
                                            entry.identity
                                        };
                                        let detail = if entry.detail.is_empty() {
                                            instance.clone()
                                        } else {
                                            format!("{instance} · {}", entry.detail)
                                        };
                                        WidgetLine {
                                            label: entry.display_name.into(),
                                            value: value.into(),
                                            detail: detail.into(),
                                            target_id: entry.id.into(),
                                            playable: compatible,
                                            favorite: entry.favorite,
                                            unvisited_favorite: false,
                                            artwork: entry
                                                .icon
                                                .and_then(|bytes| {
                                                    slint::Image::load_from_data(
                                                        &bytes,
                                                        Some("png"),
                                                    )
                                                    .ok()
                                                })
                                                .unwrap_or_default(),
                                        }
                                    })
                                    .collect(),
                            );
                        }
                        Err(error) => {
                            s.saved_errors.insert(key, error.message);
                        }
                    }
                }
            });
            render();
            enrich_servers(serial);
        },
    );
}

/// Keep the newest played entries and the newest favorites independently.
/// Never-played favorites belong only to Favorites. Ties keep core ordering.
fn server_tabs(entries: &[api::RecentTarget]) -> (Vec<api::RecentTarget>, Vec<api::RecentTarget>) {
    let mut ordered = entries.to_vec();
    ordered.sort_by_key(|t| std::cmp::Reverse(server_timestamp(t.last_played_at)));
    let recents = ordered
        .iter()
        .filter(|t| t.last_played_at > 0)
        .take(SERVER_ROWS)
        .cloned()
        .collect();
    let favorites = ordered
        .into_iter()
        .filter(|t| t.favorite)
        .take(SERVER_ROWS)
        .collect();
    (recents, favorites)
}

fn server_entries(entries: &[api::RecentTarget]) -> Vec<api::RecentTarget> {
    let (mut recents, favorites) = server_tabs(entries);
    for favorite in favorites {
        if !recents.iter().any(|t| t.id == favorite.id) {
            recents.push(favorite);
        }
    }
    recents
}

fn server_motd(presentation: &api::ServerPresentation) -> String {
    // Display-only flattening: servers centre MOTD lines with leading/trailing
    // spaces, which would otherwise break the row's shared left edge. Trim each
    // line and drop ones that become empty; stored data stays untouched.
    presentation
        .motd
        .iter()
        .map(|line| {
            line.iter()
                .map(|segment| segment.text.as_str())
                .collect::<String>()
        })
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join("\n")
}

fn server_timestamp(at: u64) -> u64 {
    if at > 100_000_000_000 { at / 1000 } else { at }
}

fn server_played(at: u64, now: u64) -> String {
    if at == 0 {
        "Not played yet".into()
    } else {
        relative(at, now)
    }
}

fn enrich_servers(serial: u64) {
    let ids = STATE.with(|state| {
        let mut state = state.borrow_mut();
        let ids: Vec<_> = state.servers.iter().map(|t| t.id.clone()).collect();
        state.server_enriching = !ids.is_empty();
        ids
    });
    if ids.is_empty() {
        return;
    }
    render();
    ui::spawn(api::server_presentations(ui::host(), ids), move |result| {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.serial != serial {
                return;
            }
            state.server_enriching = false;
            state.presentations = result.unwrap_or_default();
            // Only core-validated, bounded static PNGs reach Slint, once per refresh.
            use base64::Engine as _;
            state.server_icons = state
                .presentations
                .iter()
                .filter_map(|p| {
                    let encoded = p
                        .favicon
                        .as_deref()?
                        .strip_prefix("data:image/png;base64,")?;
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .ok()?;
                    let icon = slint::Image::load_from_data(&bytes, Some("png")).ok()?;
                    Some((p.target_id.clone(), icon))
                })
                .collect();
        });
        render();
    });
}

fn render_servers(widgets: &Widgets, names: &std::collections::HashMap<String, String>) {
    STATE.with(|state| {
        let state = state.borrow();
        let (recents, favorites) = server_tabs(&state.servers);
        let rows = |entries: Vec<api::RecentTarget>| {
            entries
                .into_iter()
                .map(|target| {
                    let presentation = state
                        .presentations
                        .iter()
                        .find(|p| p.target_id == target.id);
                    let icon = state.server_icons.get(&target.id);
                    let name = presentation
                        .and_then(|p| p.name.as_deref())
                        .filter(|n| !n.is_empty())
                        .unwrap_or(&target.display_name);
                    let played = if state.server_launch.as_deref() == Some(target.id.as_str()) {
                        "Starting…".into()
                    } else {
                        server_played(target.last_played_at, now_secs())
                    };
                    let instance = names
                        .get(&target.instance_id)
                        .map(String::as_str)
                        .unwrap_or("Instance unavailable");
                    ServerRow {
                        id: target.id.clone().into(),
                        favorite: target.favorite,
                        favorite_busy: state.favorite_pending.contains(&target.id),
                        available: target.available && names.contains_key(&target.instance_id),
                        name: name.into(),
                        detail: format!(
                            "{played} · {instance}{}",
                            if target.available {
                                ""
                            } else {
                                " · Unavailable"
                            }
                        )
                        .into(),
                        motd: presentation.map(server_motd).unwrap_or_default().into(),
                        picture: icon.cloned().unwrap_or_default(),
                        has_picture: icon.is_some(),
                        letter: ui::letter(name).into(),
                    }
                })
                .collect::<Vec<_>>()
        };
        widgets.set_server_recents(server_model(widgets.get_server_recents(), rows(recents)));
        widgets.set_server_favorites(server_model(
            widgets.get_server_favorites(),
            rows(favorites),
        ));
        widgets.set_server_error(state.server_error);
        widgets.set_server_enriching(state.server_enriching);
        widgets.set_server_launch_busy(state.server_launch.is_some());
    });
}

fn server_model(
    current: slint::ModelRc<ServerRow>,
    rows: Vec<ServerRow>,
) -> slint::ModelRc<ServerRow> {
    if current.row_count() == rows.len()
        && rows
            .iter()
            .enumerate()
            .all(|(index, row)| current.row_data(index).is_some_and(|old| old.id == row.id))
        && let Some(model) = current
            .as_any()
            .downcast_ref::<slint::VecModel<ServerRow>>()
    {
        // Keep the focused star/Play control alive during confirmed data updates.
        for (index, row) in rows.into_iter().enumerate() {
            model.set_row_data(index, row);
        }
        return current;
    }
    ui::model(rows)
}

fn line(label: &str, value: &str, detail: &str) -> WidgetLine {
    WidgetLine {
        label: label.into(),
        value: value.into(),
        detail: detail.into(),
        ..Default::default()
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

pub fn duration(ms: u64) -> String {
    let minutes = ms / 60_000;
    match minutes {
        0 if ms > 0 => "<1 min".into(),
        0 => "0 min".into(),
        1..=59 => format!("{minutes} min"),
        _ if minutes % 60 == 0 => format!("{} h", minutes / 60),
        _ => format!("{} h {} min", minutes / 60, minutes % 60),
    }
}

/// Seconds-since-epoch → "Today", "Yesterday", "3 days ago", "5 weeks ago".
pub fn relative(at: u64, now: u64) -> String {
    let at = if at > 100_000_000_000 { at / 1000 } else { at };
    let days = now.saturating_sub(at) / 86_400;
    match days {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        2..=13 => format!("{days} days ago"),
        14..=59 => format!("{} weeks ago", days / 7),
        _ => format!("{} months ago", days / 30),
    }
}

fn details() -> Vec<WidgetLine> {
    ui::state(|s| {
        let Some(instance) = s.selected_instance() else {
            return Vec::new();
        };
        let (status, _) = crate::instances::content_status(instance);
        let c = &instance.configuration;
        let mut lines = vec![
            line("Minecraft", &instance.minecraft_version, ""),
            line("Loader", &instance.platform_label(), ""),
            line(
                "Memory",
                &format!("{:.1} GB", c.memory_mib as f32 / 1024.0),
                "",
            ),
            line("State", &status, ""),
        ];
        if let Some(pack) = &instance.pack {
            lines.insert(2, line("Modpack", &pack.pack_version, ""));
        }
        lines.truncate(5);
        lines
    })
}

pub fn render() {
    let (
        layout,
        data,
        loading,
        columns,
        favorites,
        status,
        refreshing,
        saved_servers,
        saved_worlds,
        saved_errors,
    ) = STATE.with(|s| {
        let s = s.borrow();
        (
            s.layout.clone(),
            s.data.clone(),
            s.loading,
            if s.columns == 0 { 6 } else { s.columns },
            s.server_favorites,
            s.server_status.clone(),
            s.refreshing_servers,
            s.saved_servers,
            s.saved_worlds,
            s.saved_errors.clone(),
        )
    });
    let Some(layout) = layout else { return };
    let has_instance = ui::state(|s| s.selected_instance().is_some());
    let mut items = Vec::new();
    let placements = grid_positions(&layout, columns);
    for (widget, (column, row, width, height)) in layout
        .widgets
        .iter()
        .filter(|w| w.enabled && is_native(&w.id))
        .zip(placements.iter().copied())
    {
        let Some((id, title, icon)) = NATIVE.iter().find(|(id, ..)| *id == widget.id) else {
            continue;
        };
        let (lines, empty, busy) = match *id {
            "playtime" => (
                data.get(id).cloned().unwrap_or_default(),
                if has_instance {
                    "No playtime yet for this instance. Aurora records time only while Minecraft runs."
                } else {
                    "Choose an instance to see its playtime."
                },
                loading,
            ),
            "recent-servers" => (
                data.get(if saved_servers { "saved-servers" } else { id })
                    .into_iter()
                    .flatten()
                    .filter(|row| {
                        saved_servers
                            || (if favorites {
                                row.favorite
                            } else {
                                !row.unvisited_favorite
                            })
                    })
                    .cloned()
                    .map(|mut row| {
                        if let Some((label, icon)) = status.get(row.target_id.as_str()) {
                            row.value = format!("{} · {label}", row.value).into();
                            if icon.size().width > 0 {
                                row.artwork = icon.clone();
                            }
                        }
                        row
                    })
                    .collect(),
                if saved_servers {
                    saved_errors
                        .get("saved-servers")
                        .map(String::as_str)
                        .unwrap_or(if has_instance {
                            "No saved servers in this instance."
                        } else {
                            "Choose an instance to browse its saved servers."
                        })
                } else if favorites {
                    "No favorite servers yet."
                } else {
                    "Observed server visits appear here as Aurora records them."
                },
                loading,
            ),
            "recent-worlds" => (
                data.get(if saved_worlds { "saved-worlds" } else { id })
                    .cloned()
                    .unwrap_or_default(),
                if saved_worlds {
                    saved_errors
                        .get("saved-worlds")
                        .map(String::as_str)
                        .unwrap_or(if has_instance {
                            "No saved worlds in this instance."
                        } else {
                            "Choose an instance to browse its saved worlds."
                        })
                } else {
                    "No worlds yet. Singleplayer worlds you play in this instance appear here."
                },
                loading,
            ),
            "instance-details" => (details(), "Choose an instance to see its details.", false),
            _ => (Vec::new(), "", false),
        };
        items.push(WidgetView {
            id: (*id).into(),
            title: (*title).into(),
            icon: symbols::path_or_fallback(Some(icon)).into(),
            lines: ui::model(lines),
            empty: empty.into(),
            loading: busy,
            column,
            row,
            width,
            height,
            requested_width: widget
                .grid
                .as_ref()
                .map_or(if widget.size == "small" { 2 } else { 4 }, |g| {
                    g.columns as i32
                }),
            requested_height: widget
                .grid
                .as_ref()
                .map_or(if widget.size == "large" { 2 } else { 1 }, |g| {
                    g.rows as i32
                }),
        });
    }
    let available: Vec<WidgetChoice> = NATIVE
        .iter()
        .filter(|(id, ..)| !layout.widgets.iter().any(|w| w.enabled && w.id == *id))
        .map(|(id, title, icon)| WidgetChoice {
            id: (*id).into(),
            title: (*title).into(),
            icon: symbols::path_or_fallback(Some(icon)).into(),
        })
        .collect();
    let server_names = ui::state(|state| {
        state
            .instances()
            .iter()
            .map(|i| (i.id.clone(), i.display_name.clone()))
            .collect::<std::collections::HashMap<_, _>>()
    });
    ui::with(|window, _| {
        let widgets = window.global::<Widgets>();
        widgets.set_saved_servers(saved_servers);
        widgets.set_saved_worlds(saved_worlds);
        widgets.set_refreshing_servers(refreshing);
        render_servers(&widgets, &server_names);
        let existing = widgets.get_items();
        if existing.row_count() == items.len()
            && items
                .iter()
                .enumerate()
                .all(|(i, item)| existing.row_data(i).is_some_and(|old| old.id == item.id))
            && let Some(model) = existing
                .as_any()
                .downcast_ref::<slint::VecModel<WidgetView>>()
        {
            // Preserve pointer capture and keyboard focus when data/layout refreshes.
            for (i, item) in items.into_iter().enumerate() {
                model.set_row_data(i, item);
            }
        } else {
            widgets.set_items(ui::model(items));
        }
        widgets.set_available_names(ui::model(
            available
                .iter()
                .map(|w| w.title.clone())
                .collect::<Vec<_>>(),
        ));
        widgets.set_available(ui::model(available));
        widgets.set_grid_rows(
            placements
                .iter()
                .map(|(_, r, _, h)| r + h)
                .max()
                .unwrap_or(0),
        );
        widgets.set_names(ui::model(
            layout
                .widgets
                .iter()
                .filter(|w| w.enabled)
                .filter_map(|w| {
                    NATIVE
                        .iter()
                        .find(|(id, _, _)| *id == w.id)
                        .map(|(_, title, _)| (*title).into())
                })
                .collect::<Vec<_>>(),
        ));
        widgets.set_positions(ui::model(
            (1..=placements.len())
                .map(|i| format!("Position {i}").into())
                .collect::<Vec<_>>(),
        ));
    });
}

/// Apply one completed gesture. Moving into another card displaces it without overlap.
fn arrange(
    layout: &mut HomeLayout,
    id: &str,
    column: i32,
    row: i32,
    width: i32,
    height: i32,
    columns: i32,
) {
    let Some(index) = layout
        .widgets
        .iter()
        .position(|w| w.id == id && w.enabled && is_native(id))
    else {
        return;
    };
    let width = width.clamp(1, 6);
    let max_column = (columns.clamp(1, 6) - width.min(columns.clamp(1, 6))).min(6 - width);
    layout.widgets[index].grid = Some(api::WidgetGrid {
        columns: width as u8,
        rows: height.clamp(1, 3) as u8,
        position: Some([column.clamp(0, max_column) as u8, row.clamp(0, 63) as u8]),
    });
    // Give the manipulated card first claim, while preserving the saved entry order.
    let mut prioritized = layout.clone();
    let moved = prioritized.widgets.remove(index);
    prioritized.widgets.insert(0, moved);
    let positions = grid_positions(&prioritized, columns);
    for (widget, (c, r, w, h)) in prioritized
        .widgets
        .iter()
        .filter(|w| w.enabled && is_native(&w.id))
        .zip(positions)
    {
        let target = layout
            .widgets
            .iter_mut()
            .find(|w| w.id == widget.id)
            .unwrap();
        let mut grid = target.grid.clone().unwrap_or(api::WidgetGrid {
            columns: w as u8,
            rows: h as u8,
            position: None,
        });
        grid.position = Some([c as u8, r as u8]);
        // A move on a narrow viewport must not erase the saved wider size.
        if target.id == id {
            grid.columns = width as u8;
        }
        target.grid = Some(grid);
    }
}

/// First-fit placement keeps custom dimensions while preventing overlaps at narrow widths.
pub(crate) fn grid_positions(layout: &HomeLayout, columns: i32) -> Vec<(i32, i32, i32, i32)> {
    let columns = columns.clamp(1, 6) as usize;
    let mut occupied = vec![[false; 6]; 64 * 3];
    let widgets: Vec<_> = layout
        .widgets
        .iter()
        .filter(|w| w.enabled && is_native(&w.id))
        .collect();
    let mut result = vec![(0, 0, 1, 1); widgets.len()];
    let mut order: Vec<_> = (0..widgets.len()).collect();
    order.sort_by_key(|&i| widgets[i].grid.as_ref().and_then(|g| g.position).is_none());
    for index in order {
        let w = widgets[index];
        let width = w
            .grid
            .as_ref()
            .map_or(if w.size == "small" { 2 } else { 4 }, |g| {
                g.columns as usize
            })
            .clamp(1, columns);
        let height = w
            .grid
            .as_ref()
            .map_or(if w.size == "large" { 2 } else { 1 }, |g| g.rows as usize)
            .clamp(1, 3);
        let anchor = w
            .grid
            .as_ref()
            .and_then(|g| g.position)
            .map(|[c, r]| ((c as usize).min(columns - width), r as usize));
        let candidates = anchor.into_iter().chain(
            (0..occupied.len() - height + 1)
                .flat_map(|r| (0..=columns - width).map(move |c| (c, r))),
        );
        for (col, row) in candidates {
            if (row..row + height).all(|r| (col..col + width).all(|c| !occupied[r][c])) {
                for cells in &mut occupied[row..row + height] {
                    cells[col..col + width].fill(true);
                }
                result[index] = (col as i32, row as i32, width as i32, height as i32);
                break;
            }
        }
    }
    result
}

/// Bounded UTC-day projection. Missing days are zero, never fabricated activity.
/// The endpoint returns at most 30 days including its query's UTC today.
#[derive(Debug, PartialEq, Eq)]
struct PlaytimeBin {
    first: u64,
    last: u64,
    duration_ms: u64,
}

fn playtime_bins(
    daily: &[api::DailyPlaytime],
    today: u64,
    days: usize,
    count: usize,
) -> Vec<PlaytimeBin> {
    if daily.is_empty() {
        return Vec::new();
    }
    let days = days.clamp(1, 30).min(today.saturating_add(1) as usize);
    let count = count.clamp(1, days);
    let first = today - (days - 1) as u64;
    (0..count)
        .map(|i| {
            let start = first + (i * days / count) as u64;
            let end = first + ((i + 1) * days / count - 1) as u64;
            PlaytimeBin {
                first: start,
                last: end,
                duration_ms: daily
                    .iter()
                    .filter(|d| (start..=end).contains(&d.day))
                    .fold(0u64, |sum, d| sum.saturating_add(d.duration_ms)),
            }
        })
        .collect()
}

fn bin_label(bin: &PlaytimeBin, today: u64) -> String {
    let relative = |day| match today - day {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        n => format!("{n} days ago"),
    };
    if bin.first == bin.last {
        relative(bin.first)
    } else {
        format!("{} – {}", relative(bin.first), relative(bin.last))
    }
}

fn points_from_bins(bins: &[PlaytimeBin], today: u64) -> Vec<crate::PlaytimePoint> {
    let peak = chart_peak(bins);
    bins.iter()
        .map(|bin| {
            let label = bin_label(bin, today);
            crate::PlaytimePoint {
                label: label.clone().into(),
                detail: format!(
                    "{label} · UTC: {} ({} ms)",
                    duration(bin.duration_ms),
                    bin.duration_ms
                )
                .into(),
                fraction: bin.duration_ms as f32 / peak as f32,
                value: duration(bin.duration_ms).into(),
            }
        })
        .collect()
}

// Existing synthetic layout acceptance uses this projection helper only.
#[cfg(test)]
pub(crate) fn chart_points(
    daily: &[api::DailyPlaytime],
    count: usize,
) -> Vec<crate::PlaytimePoint> {
    let today = daily.iter().map(|d| d.day).max().unwrap_or(0);
    points_from_bins(&playtime_bins(daily, today, count, count), today)
}

fn chart_peak(bins: &[PlaytimeBin]) -> u64 {
    bins.iter()
        .map(|b| b.duration_ms)
        .max()
        .unwrap_or(0)
        .max(60_000)
}

fn axis_duration(ms: u64) -> String {
    // Keep extreme totals bounded; exact milliseconds remain on accessible bars.
    let hours = ms / 3_600_000;
    if hours >= 1000 {
        format!("{:.1e} h", hours as f64)
    } else {
        duration(ms)
    }
}

fn playtime_total(bins: &[PlaytimeBin]) -> u64 {
    bins.iter()
        .fold(0u64, |sum, b| sum.saturating_add(b.duration_ms))
}

fn lifetime_text(summary: Option<api::Playtime>) -> (String, String) {
    summary.map_or_else(
        || {
            (
                "Total unavailable".into(),
                "Lifetime playtime is unavailable.".into(),
            )
        },
        |p| {
            (
                duration(p.all_time_ms),
                format!(
                    "All time: {} ({} ms). Chart shows only the last 30 UTC days.",
                    duration(p.all_time_ms),
                    p.all_time_ms
                ),
            )
        },
    )
}

/// Called only when the existing asynchronous history refresh completes.
/// Selection and resizing switch cached models without I/O or aggregation.
fn present_playtime(summary: Option<api::Playtime>, daily: Option<&[api::DailyPlaytime]>) {
    let history = daily.unwrap_or_default();
    // Anchor labels to the API snapshot, including a refresh that crosses UTC midnight.
    let today = history
        .iter()
        .map(|d| d.day)
        .max()
        .unwrap_or(now_secs() / 86_400);
    let week = playtime_bins(history, today, 7, 7);
    let month = playtime_bins(history, today, 30, 30);
    let binned = playtime_bins(history, today, 30, 7);
    let (lifetime, detail) = lifetime_text(summary);
    ui::with(|window, _| {
        let widgets = window.global::<Widgets>();
        widgets.set_week_total(
            if daily.is_some_and(|days| !days.is_empty()) {
                duration(playtime_total(&week))
            } else {
                "Total unavailable".into()
            }
            .into(),
        );
        widgets.set_month_total(
            if daily.is_some_and(|days| !days.is_empty()) {
                duration(playtime_total(&month))
            } else {
                "Total unavailable".into()
            }
            .into(),
        );
        widgets.set_lifetime_total(lifetime.into());
        widgets.set_week_empty(playtime_total(&week) == 0);
        widgets.set_month_empty(playtime_total(&month) == 0);
        widgets.set_lifetime_detail(detail.into());
        widgets.set_week_peak(axis_duration(chart_peak(&week)).into());
        widgets.set_month_peak(axis_duration(chart_peak(&month)).into());
        widgets.set_month_binned_peak(axis_duration(chart_peak(&binned)).into());
        widgets.set_week(ui::model(points_from_bins(&week, today)));
        widgets.set_month(ui::model(points_from_bins(&month, today)));
        widgets.set_month_binned(ui::model(points_from_bins(&binned, today)));
        widgets.set_playtime_status(
            match daily {
                None => "Daily playtime is unavailable.",
                Some([]) => "No daily history available.",
                Some(_) => "",
            }
            .into(),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(id: usize, at: u64, favorite: bool) -> api::RecentTarget {
        api::RecentTarget {
            id: id.to_string(),
            instance_id: "instance".into(),
            display_name: "Server".into(),
            last_played_at: at,
            duration_ms: 0,
            available: true,
            favorite,
        }
    }

    #[test]
    fn favorite_actions_wait_for_confirmation_reject_duplicates_and_recover_after_failure() {
        let mut state = State::default();
        state.servers = vec![server(1, 1, false), server(2, 1, true)];
        assert_eq!(state.begin_favorite("missing"), None);
        assert_eq!(state.begin_favorite("1"), Some(true));
        assert!(!state.servers[0].favorite);
        assert_eq!(state.begin_favorite("1"), None);
        assert_eq!(state.begin_favorite("2"), Some(false));
        state.finish_favorite("1", true, false);
        assert!(!state.servers[0].favorite);
        assert_eq!(state.favorite_revision, 0);
        assert_eq!(state.begin_favorite("1"), Some(true));
        state.finish_favorite("1", true, true);
        state.finish_favorite("2", false, true);
        assert!(state.servers[0].favorite);
        assert!(!state.servers[1].favorite);
        assert_eq!(state.favorite_revision, 2);
        // A duplicate completion cannot undo the confirmed state.
        state.finish_favorite("1", false, true);
        assert!(state.servers[0].favorite);
        assert_eq!(state.favorite_revision, 2);
    }

    #[test]
    fn server_launch_uses_the_exact_history_target_and_its_original_instance() {
        let mut entries = vec![server(1, 1, false), server(2, 1, false)];
        entries[0].instance_id = "original-instance".into();
        entries[1].available = false;
        let target = server_target(&entries, "1").unwrap();
        assert_eq!(target.id, "1");
        assert_eq!(target.instance_id, "original-instance");
        assert_eq!(
            server_target(&entries, "2").unwrap_err().code,
            "quick_target_stale"
        );
        assert_eq!(
            server_target(&entries, "missing").unwrap_err().code,
            "quick_target_stale"
        );
        let mut state = ui::AppState::default();
        assert_eq!(
            begin_server_launch(&mut state, target).unwrap_err().code,
            "quick_instance_missing"
        );
        assert!(!state.play_busy);
        state.play_busy = true;
        assert_eq!(begin_server_launch(&mut state, target).unwrap(), None);
        state.play_busy = false;
        state.pending_play = Some("other-instance".into());
        assert_eq!(begin_server_launch(&mut state, target).unwrap(), None);
        state.pending_play = None;
        state.launcher = Some(
            serde_json::from_value(serde_json::json!({
                "config": {"selectedInstanceId": "different-selected-instance"},
                "instances": [{
                    "id": "original-instance", "displayName": "Stored target instance",
                    "state": "ready", "aurora": null, "platform": {"kind": "vanilla"},
                    "minecraftVersion": "1.21.1",
                    "configuration": {"minecraftVersion":"1.21.1", "auroraEnabled":false,
                        "loader":null, "memoryMib":4096, "additionalJvmArguments":"", "window":null}
                }]
            }))
            .unwrap(),
        );
        assert_eq!(
            begin_server_launch(&mut state, target).unwrap_err().code,
            "launch_authentication_required"
        );
        assert!(!state.play_busy);
        state.accounts.selected_account_id = Some("selected-account".into());
        assert_eq!(
            begin_server_launch(&mut state, target).unwrap(),
            Some("selected-account".into())
        );
        assert!(state.play_busy);
        assert_eq!(begin_server_launch(&mut state, target).unwrap(), None);
    }

    #[test]
    fn server_tabs_keep_old_favorites_and_bound_the_enrichment_union() {
        let now = 1_800_000_000;
        let mut entries: Vec<_> = (0..100).map(|i| server(i, now - i as u64, false)).collect();
        entries.extend((100..200).map(|i| server(i, 0, true)));
        let bounded = server_entries(&entries);
        let (recents, favorites) = server_tabs(&bounded);
        assert_eq!(recents.len(), SERVER_ROWS);
        assert_eq!(favorites.len(), SERVER_ROWS);
        assert_eq!(bounded.len(), 2 * SERVER_ROWS);
        assert_eq!(recents[0].id, "0");
        assert_eq!(favorites[0].id, "100");
        assert!(favorites.iter().all(|t| t.last_played_at == 0));
        assert_eq!(server_played(0, now), "Not played yet");
        assert!(server_entries(&[]).is_empty());
        let (recents, favorites) = server_tabs(&[]);
        assert!(recents.is_empty() && favorites.is_empty());
    }

    #[test]
    fn server_tabs_deduplicate_shared_targets_and_order_seconds_and_milliseconds() {
        let now = 1_800_000_000;
        let entries = [
            server(1, (now - 10) * 1000, true),
            server(2, now, true),
            server(3, 0, true),
        ];
        let bounded = server_entries(&entries);
        let (recents, favorites) = server_tabs(&bounded);
        assert_eq!(bounded.len(), 3);
        assert_eq!(
            recents.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["2", "1"]
        );
        assert_eq!(
            favorites.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            ["2", "1", "3"]
        );
        assert_eq!(server_played((now - 86400) * 1000, now), "Yesterday");
    }

    #[test]
    fn server_presentation_mirror_retains_real_identity_and_two_motd_lines() {
        let presentation: api::ServerPresentation = serde_json::from_value(serde_json::json!({
            "targetId": "opaque", "status": "offline", "name": "Saved server",
            "favicon": null,
            "motd": [[{"text":"Welcome ", "bold":true}, {"text":"back", "color":"#ffffff"}],
                     [{"text":"Second line"}], [{"text":"Third line"}]]
        }))
        .unwrap();
        assert_eq!(presentation.target_id, "opaque");
        assert_eq!(presentation.name.as_deref(), Some("Saved server"));
        assert_eq!(server_motd(&presentation), "Welcome back\nSecond line");
        assert!(presentation.favicon.is_none());
        // Server-centred MOTDs pad lines with spaces; display trims them and
        // drops whitespace-only lines so both lines share the name's left edge.
        let centred: api::ServerPresentation = serde_json::from_value(serde_json::json!({
            "targetId": "centred", "status": "offline", "name": "Centred server",
            "favicon": null,
            "motd": [[{"text":"      PVPHQ "}, {"text":"| discord.gg/pvphq    "}],
                     [{"text":"          "}],
                     [{"text":"   Actual second line   "}]]
        }))
        .unwrap();
        assert_eq!(
            server_motd(&centred),
            "PVPHQ | discord.gg/pvphq\nActual second line"
        );
        let blank: api::ServerPresentation = serde_json::from_value(serde_json::json!({
            "targetId": "blank", "status": "offline", "name": "Blank server",
            "favicon": null,
            "motd": [[{"text":"   "}], [{"text":"\t"}]]
        }))
        .unwrap();
        assert_eq!(server_motd(&blank), "");
    }

    #[test]
    fn custom_widget_geometry_survives_roundtrip_and_never_overlaps() {
        let mut layout = effective(HomeLayout {
            widgets: vec![
                placement("playtime", true),
                placement("media", true),
                placement("recent-servers", true),
            ],
        });
        layout.widgets[0].grid = Some(api::WidgetGrid {
            columns: 5,
            rows: 3,
            position: None,
        });
        let saved: HomeLayout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        assert_eq!(saved, layout);
        for columns in 1..=6 {
            let positions = grid_positions(&saved, columns);
            let mut cells = std::collections::HashSet::new();
            for (col, row, width, height) in positions {
                assert!(col + width <= columns);
                for y in row..row + height {
                    for x in col..col + width {
                        assert!(cells.insert((x, y)), "overlapping widgets");
                    }
                }
            }
        }
        assert_eq!(saved.widgets[0].grid.as_ref().unwrap().columns, 5);
        place_widget(&mut layout, "playtime", 2);
        assert_eq!(
            layout
                .widgets
                .iter()
                .filter(|w| w.enabled && is_native(&w.id))
                .last()
                .unwrap()
                .id,
            "playtime"
        );
    }

    #[test]
    fn dragging_displaces_collisions_and_preserves_unrecognized_widgets() {
        let mut layout = HomeLayout {
            widgets: vec![
                placement("playtime", true),
                placement("media", true),
                placement("future-widget", true),
            ],
        };
        let unknown = layout.widgets[2].clone();
        arrange(&mut layout, "media", 0, 0, 3, 2, 6);
        let positions = grid_positions(&layout, 6);
        assert_eq!(positions[1], (0, 0, 3, 2));
        assert_eq!(layout.widgets[2], unknown);
        let mut cells = std::collections::HashSet::new();
        for (c, r, w, h) in positions {
            for y in r..r + h {
                for x in c..c + w {
                    assert!(cells.insert((x, y)));
                }
            }
        }
        let saved = serde_json::to_string(&layout).unwrap();
        let restored: HomeLayout = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored, layout);
        let core: aurora_launcher_lib::home_widgets::HomeLayout =
            serde_json::from_str(&saved).unwrap();
        core.validate().unwrap();
        arrange(&mut layout, "media", 99, 99, 99, 99, 6);
        assert_eq!(grid_positions(&layout, 6)[1], (0, 63, 6, 3));
    }

    fn daily(day: u64, duration_ms: u64) -> api::DailyPlaytime {
        api::DailyPlaytime { day, duration_ms }
    }

    #[test]
    fn playtime_ranges_include_utc_today_and_exclude_outside_days() {
        let at = 20_000 * 86_400 + 86_399;
        let today = at / 86_400;
        let history = vec![
            daily(today - 30, 1),
            daily(today - 29, 2),
            daily(today - 7, 4),
            daily(today - 6, 8),
            daily(today, 16),
            daily(today + 1, 32),
        ];
        let week = playtime_bins(&history, today, 7, 7);
        let month = playtime_bins(&history, today, 30, 30);
        assert_eq!(week.first().unwrap().first, today - 6);
        assert_eq!(week.last().unwrap().last, today);
        assert_eq!(playtime_total(&week), 24);
        assert_eq!(playtime_total(&month), 30);
        assert_eq!(bin_label(week.last().unwrap(), today), "Today");
        assert_eq!(
            playtime_total(&playtime_bins(&history, (at + 1) / 86_400, 7, 7)),
            48
        );
    }

    #[test]
    fn playtime_missing_zero_and_empty_days_stay_honest() {
        let bins = playtime_bins(&[daily(99, 0), daily(100, 300_000)], 100, 7, 7);
        let points = points_from_bins(&bins, 100);
        assert_eq!(bins.len(), 7);
        assert!(bins[..6].iter().all(|b| b.duration_ms == 0));
        assert!(points[..6].iter().all(|p| p.fraction == 0.0));
        assert_eq!(points[6].fraction, 1.0);
        assert!(points[6].detail.contains("300000 ms"));
        assert!(playtime_bins(&[], 100, 30, 7).is_empty());
        assert_eq!(
            chart_peak(&playtime_bins(&[daily(100, 0)], 100, 7, 7)),
            60_000
        );
    }

    #[test]
    fn playtime_narrow_bins_cover_every_selected_utc_day_once_and_preserve_sums() {
        let history: Vec<_> = (70..=100)
            .map(|day| daily(day, (day - 70) * 60_000))
            .collect();
        let bins = playtime_bins(&history, 100, 30, 7);
        assert_eq!(bins.len(), 7);
        assert_eq!(bins.first().unwrap().first, 71);
        assert_eq!(bins.last().unwrap().last, 100);
        assert!(
            bins.windows(2)
                .all(|pair| pair[0].last + 1 == pair[1].first)
        );
        assert_eq!(
            playtime_total(&bins),
            playtime_total(&playtime_bins(&history, 100, 30, 30))
        );
        assert_eq!(bin_label(&bins[0], 100), "29 days ago – 26 days ago");
        assert_eq!(bin_label(&bins[6], 100), "4 days ago – Today");
        assert_eq!(points_from_bins(&bins, 100).len(), 7);
    }

    #[test]
    fn playtime_lifetime_uses_real_summary_without_extending_daily_history() {
        let summary = api::Playtime {
            all_time_ms: 360_000_000,
            ..Default::default()
        };
        let bins = playtime_bins(&[daily(100, 60_000)], 100, 30, 7);
        let (total, detail) = lifetime_text(Some(summary));
        assert_eq!(total, "100 h");
        assert!(detail.contains("last 30 UTC days"));
        assert_eq!(playtime_total(&bins), 60_000);
        assert_eq!(bins.first().unwrap().first, 71);
        assert_eq!(lifetime_text(None).0, "Total unavailable");
    }

    #[test]
    fn playtime_peak_labels_keep_minutes_and_bound_large_durations() {
        assert_eq!(axis_duration((4 * 60 + 52) * 60_000), "4 h 52 min");
        assert_eq!(axis_duration(0), "0 min");
        assert!(axis_duration(u64::MAX).len() <= 10);
        let bins = playtime_bins(&[daily(100, u64::MAX)], 100, 30, 7);
        assert_eq!(playtime_total(&bins), u64::MAX);
        assert_eq!(points_from_bins(&bins, 100).last().unwrap().fraction, 1.0);
        assert!(
            points_from_bins(&bins, 100)
                .last()
                .unwrap()
                .detail
                .contains(&u64::MAX.to_string())
        );
    }

    fn placement(id: &str, enabled: bool) -> WidgetPlacement {
        WidgetPlacement {
            id: id.into(),
            enabled,
            size: "small".into(),
            grid: None,
        }
    }

    #[test]
    fn an_untouched_layout_shows_only_playtime_and_servers() {
        let core = aurora_launcher_lib::home_widgets::HomeLayout::default();
        let saved: HomeLayout =
            serde_json::from_value(serde_json::to_value(core).unwrap()).unwrap();
        let layout = effective(saved);
        let shown: Vec<_> = layout
            .widgets
            .iter()
            .filter(|w| w.enabled && is_native(&w.id))
            .map(|w| w.id.as_str())
            .collect();
        assert_eq!(shown, ["playtime", "recent-servers"]);
        // Other hosts' entries survive.
        assert!(layout.widgets.iter().any(|w| w.id == "skin-manager"));
        assert!(layout.widgets.iter().any(|w| w.id == "media" && !w.enabled));
    }

    #[test]
    fn customized_layouts_are_respected_and_reorderable() {
        let mut layout = effective(HomeLayout {
            widgets: vec![
                placement("recent-servers", true),
                placement("media", true),
                placement("custom-from-elsewhere", true),
            ],
        });
        move_widget(&mut layout, "media", -1);
        let order: Vec<_> = layout
            .widgets
            .iter()
            .filter(|w| w.enabled && is_native(&w.id))
            .map(|w| w.id.as_str())
            .collect();
        assert_eq!(order, ["media", "recent-servers"]);
        set_enabled(&mut layout, "playtime", true);
        set_enabled(&mut layout, "media", false);
        let order: Vec<_> = layout
            .widgets
            .iter()
            .filter(|w| w.enabled && is_native(&w.id))
            .map(|w| w.id.as_str())
            .collect();
        assert_eq!(order, ["recent-servers", "playtime"]);
        assert!(
            layout
                .widgets
                .iter()
                .any(|w| w.id == "custom-from-elsewhere")
        );
        // The persisted shape stays valid for the core validator.
        let core: aurora_launcher_lib::home_widgets::HomeLayout =
            serde_json::from_value(serde_json::to_value(&layout).unwrap()).unwrap();
        core.validate().unwrap();
    }

    #[test]
    fn durations_and_relative_days_read_naturally() {
        assert_eq!(duration(0), "0 min");
        assert_eq!(duration(30_000), "<1 min");
        assert_eq!(duration(45 * 60_000), "45 min");
        assert_eq!(duration(120 * 60_000), "2 h");
        assert_eq!(duration(135 * 60_000), "2 h 15 min");
        let now = 1_800_000_000;
        assert_eq!(relative(now - 10, now), "Today");
        assert_eq!(relative(now - 86_400 - 5, now), "Yesterday");
        assert_eq!(relative(now - 3 * 86_400, now), "3 days ago");
        assert_eq!(relative((now - 3 * 86_400) * 1000, now), "3 days ago");
    }
}
