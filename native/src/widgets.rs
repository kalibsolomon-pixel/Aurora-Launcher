//! Home widgets: layout, data and customization.
//!
//! The layout persists through the core's validated Home layout (shared with
//! the WebView host; entries this host does not show are preserved). Cards
//! read existing records only: gameplay history for playtime, servers and
//! worlds, and the already-loaded instance state for details. Nothing polls;
//! data refreshes when Home's selection or an instance changes.

use std::cell::RefCell;

use slint::ComponentHandle;

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
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn bind(window: &crate::AppWindow) {
    let widgets = window.global::<Widgets>();
    widgets.on_add(|id| edit(|layout| set_enabled(layout, id.as_str(), true)));
    widgets.on_remove(|id| edit(|layout| set_enabled(layout, id.as_str(), false)));
    widgets.on_move(|id, delta| edit(|layout| move_widget(layout, id.as_str(), delta)));
    crate::media::bind(window);
}

/// The core's untouched default means "never customized": this host shows
/// its own default (Playtime and Servers) instead.
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

fn edit(change: impl FnOnce(&mut HomeLayout)) {
    let Some(mut layout) = STATE.with(|s| s.borrow().layout.clone()) else {
        return;
    };
    change(&mut layout);
    STATE.with(|s| s.borrow_mut().layout = Some(layout.clone()));
    render();
    refresh();
    ui::spawn(api::set_home_widgets(ui::host(), layout), |result| {
        if let Err(error) = result {
            ui::error_toast(&error);
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
    let serial = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.serial += 1;
        s.instance = instance.clone();
        s.loading = wants_history;
        s.serial
    });
    render();
    if !wants_history {
        return;
    }
    let host = ui::host();
    ui::spawn(
        async move {
            let playtime = api::playtime(host.clone(), instance.clone()).await;
            let servers = api::recent_servers(host.clone(), None, 5).await;
            let worlds = match &instance {
                Some(id) => api::recent_worlds(host, Some(id.clone()), 5).await,
                None => Ok(Vec::new()),
            };
            (playtime, servers, worlds)
        },
        move |(playtime, servers, worlds)| {
            let current = STATE.with(|s| s.borrow().serial == serial);
            if !current {
                return;
            }
            let names = ui::state(|s| {
                s.instances()
                    .iter()
                    .map(|i| (i.id.clone(), i.display_name.clone()))
                    .collect::<std::collections::HashMap<_, _>>()
            });
            let now = now_secs();
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.loading = false;
                s.data.insert(
                    "playtime",
                    match playtime {
                        Ok(p) if p.all_time_ms > 0 => vec![
                            line("Today", &duration(p.today_ms), ""),
                            line("Last 7 days", &duration(p.last_7_days_ms), ""),
                            line("Last 30 days", &duration(p.last_30_days_ms), ""),
                            line("All time", &duration(p.all_time_ms), ""),
                        ],
                        _ => Vec::new(),
                    },
                );
                s.data.insert(
                    "recent-servers",
                    servers
                        .unwrap_or_default()
                        .into_iter()
                        .map(|t| {
                            let instance = names.get(&t.instance_id).cloned().unwrap_or_default();
                            line(
                                &t.display_name,
                                &relative(t.last_played_at, now),
                                &if t.favorite {
                                    format!("Favorite · {instance}")
                                } else {
                                    instance
                                },
                            )
                        })
                        .collect(),
                );
                s.data.insert(
                    "recent-worlds",
                    worlds
                        .unwrap_or_default()
                        .into_iter()
                        .map(|t| {
                            line(
                                &t.display_name,
                                &relative(t.last_played_at, now),
                                &if t.available {
                                    format!("Played {}", duration(t.duration_ms))
                                } else {
                                    "No longer in this instance".into()
                                },
                            )
                        })
                        .collect(),
                );
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
    let (layout, data, loading) = STATE.with(|s| {
        let s = s.borrow();
        (s.layout.clone(), s.data.clone(), s.loading)
    });
    let Some(layout) = layout else { return };
    let has_instance = ui::state(|s| s.selected_instance().is_some());
    let mut items = Vec::new();
    for widget in layout.widgets.iter().filter(|w| w.enabled) {
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
                data.get(id).cloned().unwrap_or_default(),
                "No servers yet. Servers you join from an Aurora instance appear here.",
                loading,
            ),
            "recent-worlds" => (
                data.get(id).cloned().unwrap_or_default(),
                "No worlds yet. Singleplayer worlds you play in this instance appear here.",
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
        widgets.set_items(ui::model(items));
        widgets.set_available(ui::model(available));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(id: &str, enabled: bool) -> WidgetPlacement {
        WidgetPlacement {
            id: id.into(),
            enabled,
            size: "small".into(),
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
