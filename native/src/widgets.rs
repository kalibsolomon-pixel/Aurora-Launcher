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
use crate::{WidgetChoice, WidgetLine, WidgetView, Widgets, symbols, ui};

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
    daily: Vec<api::DailyPlaytime>,
    historical: Vec<api::PlaytimeInterval>,
    server_favorites: bool,
    saved_servers: bool,
    saved_worlds: bool,
    saved_errors: std::collections::HashMap<&'static str, String>,
    refreshing_servers: bool,
    server_status: std::collections::HashMap<String, (String, slint::Image)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn bind(window: &crate::AppWindow) {
    let widgets = window.global::<Widgets>();
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

/// The core's untouched default means "never customized": this host shows
/// its own default (Playtime and Servers) instead.
fn effective(saved: HomeLayout) -> HomeLayout {
    let core_default = default_layout();
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
    let (serial, saved_servers, saved_worlds) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.serial += 1;
        s.instance = instance.clone();
        s.data.remove("saved-worlds");
        s.data.remove("saved-servers");
        s.saved_errors.clear();
        s.loading = wants_history;
        (s.serial, s.saved_servers, s.saved_worlds)
    });
    render();
    if !wants_history {
        return;
    }
    let host = ui::host();
    ui::spawn(
        async move {
            let (playtime, daily, historical) = if shown.contains(&"playtime") {
                (
                    api::playtime(host.clone(), instance.clone()).await.ok(),
                    api::daily_playtime(host.clone(), instance.clone())
                        .await
                        .unwrap_or_default(),
                    api::historical_playtime(host.clone(), instance.clone())
                        .await
                        .unwrap_or_default(),
                )
            } else {
                (None, Vec::new(), Vec::new())
            };
            let servers = if shown.contains(&"recent-servers") {
                api::recent_servers(host.clone(), None, 200)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let worlds = if shown.contains(&"recent-worlds") {
                api::recent_worlds(host.clone(), instance.clone(), 32)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
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
            (playtime, daily, historical, servers, worlds, saved)
        },
        move |(playtime, daily, historical, servers, worlds, saved)| {
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
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.loading = false;
                s.daily = daily;
                s.historical = historical;
                s.data.insert(
                    "playtime",
                    match playtime {
                        Some(p) if p.all_time_ms > 0 => vec![
                            line("Today", &duration(p.today_ms), ""),
                            line("Last 7 days", &duration(p.last_7_days_ms), ""),
                            line("Last 30 days", &duration(p.last_30_days_ms), ""),
                            line("All time", &duration(p.all_time_ms), ""),
                        ],
                        _ => Vec::new(),
                    },
                );
                for (id, targets) in [("recent-servers", servers), ("recent-worlds", worlds)] {
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
        },
    );
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
    let days = (now / 86_400).saturating_sub(at / 86_400);
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
        daily,
        historical,
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
            s.daily.clone(),
            s.historical.clone(),
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
    ui::with(|window, _| {
        let widgets = window.global::<Widgets>();
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
        widgets.set_refreshing_servers(refreshing);
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
        widgets.set_week(ui::model(chart_points(&daily, 7)));
        widgets.set_month(ui::model(chart_points(&daily, 30)));
        let month_compact: Vec<_> = daily
            .chunks(5)
            .filter_map(|chunk| {
                Some(api::PlaytimeInterval {
                    start_day: chunk.first()?.day,
                    end_day: chunk.last()?.day,
                    duration_ms: chunk.iter().map(|day| day.duration_ms).sum(),
                })
            })
            .collect();
        let (points, peak) = interval_points(&month_compact);
        widgets.set_month_compact(ui::model(points));
        widgets.set_month_compact_peak(peak.into());
        let (points, peak) = interval_points(&historical);
        widgets.set_historical(ui::model(points));
        widgets.set_historical_peak(peak.into());
        widgets.set_week_peak(
            duration(
                daily
                    .iter()
                    .rev()
                    .take(7)
                    .map(|d| d.duration_ms)
                    .max()
                    .unwrap_or(0)
                    .max(60_000),
            )
            .into(),
        );
        widgets.set_month_peak(
            duration(
                daily
                    .iter()
                    .map(|d| d.duration_ms)
                    .max()
                    .unwrap_or(0)
                    .max(60_000),
            )
            .into(),
        );
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
    if !layout
        .widgets
        .iter()
        .any(|w| w.id == id && w.enabled && is_native(id))
    {
        return;
    }
    let width = width.clamp(1, 6);
    let height = height.clamp(1, 3);
    let shown_width = width.min(columns);
    let column = column.clamp(0, (columns - shown_width).min(6 - width));
    let row = row.clamp(0, 63);
    let visible = layout
        .widgets
        .iter()
        .filter(|w| w.enabled && is_native(&w.id));
    let positions = grid_positions(layout, columns);
    if visible.zip(&positions).any(|(widget, &(c, r, w, h))| {
        widget.id != id
            && column < c + w
            && c < column + shown_width
            && row < r + h
            && r < row + height
    }) {
        return;
    }
    let mut positions = positions.into_iter();
    for widget in layout
        .widgets
        .iter_mut()
        .filter(|w| w.enabled && is_native(&w.id))
    {
        let (c, r, _, h) = positions.next().expect("one position per visible widget");
        if widget.id == id {
            widget.grid = Some(api::WidgetGrid {
                columns: width as u8,
                rows: height as u8,
                position: Some([column as u8, row as u8]),
            });
        } else if widget
            .grid
            .as_ref()
            .and_then(|grid| grid.position)
            .is_none()
        {
            // Freeze legacy auto-placed neighbors when the layout first gains an anchor.
            let grid = widget.grid.get_or_insert_with(|| api::WidgetGrid {
                columns: if widget.size == "small" { 2 } else { 4 },
                rows: h as u8,
                position: None,
            });
            grid.position = Some([c.min(6 - i32::from(grid.columns)) as u8, r as u8]);
        }
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

fn interval_points(intervals: &[api::PlaytimeInterval]) -> (Vec<crate::PlaytimePoint>, String) {
    let peak = intervals
        .iter()
        .map(|interval| interval.duration_ms)
        .max()
        .unwrap_or(0)
        .max(60_000);
    let today = intervals.last().map_or(0, |interval| interval.end_day);
    let points = intervals
        .iter()
        .map(|interval| {
            let from = today - interval.start_day;
            let until = today - interval.end_day;
            let label = match (from, until) {
                (0, 0) => "Today".to_owned(),
                (from, 0) => format!("Last {} days", from + 1),
                (from, until) if from == until => format!("{from} days ago"),
                (from, until) => format!("{from}–{until} days ago"),
            };
            let value = duration(interval.duration_ms);
            crate::PlaytimePoint {
                detail: format!("{label} (UTC), {value}").into(),
                label: label.into(),
                value: value.into(),
                fraction: interval.duration_ms as f32 / peak as f32,
            }
        })
        .collect();
    (points, duration(peak))
}

pub(crate) fn chart_points(
    daily: &[api::DailyPlaytime],
    count: usize,
) -> Vec<crate::PlaytimePoint> {
    let values = &daily[daily.len().saturating_sub(count)..];
    let peak = values
        .iter()
        .map(|d| d.duration_ms)
        .max()
        .unwrap_or(0)
        .max(60_000);
    values
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let back = values.len() - 1 - i;
            let label = if back == 0 {
                "Today".into()
            } else {
                ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][((d.day + 4) % 7) as usize]
                    .to_string()
            };
            crate::PlaytimePoint {
                label: label.clone().into(),
                detail: format!("{label}, {back} days ago: {}", duration(d.duration_ms)).into(),
                fraction: d.duration_ms as f32 / peak as f32,
                value: duration(d.duration_ms).into(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn reset_restores_native_defaults_and_keeps_unknown_widget_settings() {
        let unknown = placement("future-widget", true);
        let mut layout = HomeLayout {
            widgets: vec![placement("media", true), unknown.clone()],
        };
        reset_layout(&mut layout);
        assert_eq!(layout.widgets.last(), Some(&unknown));
        let active: Vec<_> = layout
            .widgets
            .iter()
            .filter(|widget| widget.enabled && is_native(&widget.id))
            .map(|widget| widget.id.as_str())
            .collect();
        assert_eq!(active, ["playtime", "recent-servers"]);
    }

    #[test]
    fn dragging_refuses_collisions_and_preserves_unrecognized_widgets() {
        let mut layout = HomeLayout {
            widgets: vec![
                placement("playtime", true),
                placement("media", true),
                placement("future-widget", true),
            ],
        };
        let unknown = layout.widgets[2].clone();
        let original = layout.clone();
        arrange(&mut layout, "media", 0, 0, 3, 2, 6);
        assert_eq!(layout, original);
        arrange(&mut layout, "media", 2, 0, 3, 2, 6);
        let positions = grid_positions(&layout, 6);
        assert_eq!(positions[0], (0, 0, 2, 1));
        assert_eq!(positions[1], (2, 0, 3, 2));
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

    #[test]
    fn playtime_graph_uses_daily_history_and_shared_scale_without_inventing_activity() {
        let days: Vec<_> = (0..30)
            .map(|day| api::DailyPlaytime {
                day,
                duration_ms: if day == 29 { 300_000 } else { 0 },
            })
            .collect();
        let week = chart_points(&days, 7);
        assert_eq!(week.len(), 7);
        assert_eq!(week[6].value.as_str(), "5 min");
        assert_eq!(week[6].fraction, 1.0);
        assert!(week[..6].iter().all(|p| p.fraction == 0.0));
        assert_eq!(chart_points(&days, 30).len(), 30);
        assert!(chart_points(&[], 7).is_empty());
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
