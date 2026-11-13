//! Home widgets: the Home page's customizable card grid.
//!
//! Behavior parity lives with the preserved Slint host's `native/src/widgets.rs`
//! and `native/ui/widgets.slint`: the layout persists through the core's
//! validated Home layout (unknown entries from other hosts survive), cards read
//! existing records only (gameplay history for playtime, servers and worlds,
//! loaded instance state for details), and nothing polls or animates. Data work
//! starts only while the page is active and unblocked; a hidden page defers to
//! the next actual activation or input change.
//!
//! Events for the integrating parent:
//! - [`HomeWidgetsEvent::Changed`] is emitted only for acknowledged layout or
//!   favorite persistence — never for reads, previews or cancelled gestures.
//! - [`HomeWidgetsEvent::BusyChanged`] reports each actual write transition.
//! - [`HomeWidgetsEvent::QuickPlay`] is an intent only: this module validates
//!   the exact current row identity and its instance, then hands the request to
//!   the parent, which owns the shared launch guard and API dispatch.
//!
//! Media card: the card body is the [`Media`] entity, created lazily the first
//! time the card is shown. Its single OS worker runs only while a launcher
//! host exists, Home is active and the Media widget is enabled in the committed
//! layout (see [`media_should_run`]); every other state — hidden page, widget
//! removed or reset away, failed layout read, dropped page — leaves it
//! stopped. Drag/resize previews are not part of the committed layout, so a
//! gesture never starts or stops it. The page-level `blocked` gate stops new
//! layout/data work only; it does not touch the media session. An explicit
//! [`HomeWidgets::refresh`] is forwarded to Media only while the card is shown.
//!
//! Honest parity limits, kept deliberate:
//! - Quick Play compatibility follows the preserved host's exact rule
//!   (Minecraft 1.21.11); no launch paths are ever derived here.

#![allow(dead_code)]

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use aurora_launcher_lib::host::Host;
use gpui_kit::base::{Button, ElementExt};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AnyWindowHandle, AppContext as _, Bounds, Context, Div, Entity, EventEmitter,
    FocusHandle, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement as _, Pixels, Render, RenderImage, Role,
    ScrollHandle, StatefulInteractiveElement as _, Styled, Window, div, img, px,
};
use serde_json::Value;
use tokio::runtime::Handle;

use crate::api::{self, ApiResult, HomeLayout, WidgetGrid, WidgetPlacement};
use crate::backend::guarded;
use crate::controls::{AuroraButton, ButtonSize, ButtonVariant};
use crate::images;
use crate::media::Media;
use crate::theme;
use crate::ui;

/// Events for the integrating parent view.
#[derive(Debug)]
pub enum HomeWidgetsEvent {
    /// A layout or favorite write was acknowledged by the core. Never a read.
    Changed,
    /// The module's write-busy state changed. Each transition is reported once.
    BusyChanged(bool),
    /// Quick Play intent. The parent owns the shared launch guard and dispatch.
    QuickPlay {
        mode: String,
        target_id: String,
        instance_id: String,
    },
}
impl EventEmitter<HomeWidgetsEvent> for HomeWidgets {}

// ---------------------------------------------------------------------------
// Preserved host constants
// ---------------------------------------------------------------------------

/// The widgets this host renders, in their default order (id, title).
const NATIVE: [(&str, &str); 5] = [
    ("playtime", "Playtime"),
    ("recent-servers", "Servers"),
    ("recent-worlds", "Singleplayer worlds"),
    ("instance-details", "Instance details"),
    ("media", "Media"),
];
/// Shown on a Home that was never customized.
const DEFAULT_ON: [&str; 2] = ["playtime", "recent-servers"];
/// Each rich server tab shows at most ten rows.
const SERVER_ROWS: usize = 10;
/// The enrichment union stays bounded by the existing 20-target API.
const ENRICH_LIMIT: usize = 20;
/// Read both recents and favorites before filtering (local store bound: 100+100).
const SERVER_READ: usize = 200;
const WORLD_READ: usize = 32;
/// The preserved host's Quick Play compatibility rule.
const QUICK_PLAY_MINECRAFT: &str = "1.21.11";
const MAX_GRID_COLUMNS: i32 = 6;
const MAX_GRID_ROWS: i32 = 3;
const MAX_ANCHOR_ROW: i32 = 63;

/// Responsive grid geometry (matches the Slint `WidgetGrid` constants).
const GRID_GAP: f32 = 14.0;
const MIN_CARD_WIDTH: f32 = 150.0;
const CARD_HEIGHT: f32 = 304.0;

/// Saved columns for a viewport width; clamping renders, it never writes.
pub(crate) fn responsive_columns(width_px: f32) -> i32 {
    let columns = ((width_px + GRID_GAP) / (MIN_CARD_WIDTH + GRID_GAP)).floor() as i32;
    columns.clamp(1, MAX_GRID_COLUMNS)
}

/// Whether the Media worker may run. It needs a launcher host (the diagnostic
/// window has none and never starts anything), an active Home page, and the
/// Media card shown by the *committed* layout.
pub(crate) fn media_should_run(has_host: bool, page_active: bool, card_shown: bool) -> bool {
    has_host && page_active && card_shown
}

// ---------------------------------------------------------------------------
// Pure formatting helpers (ported from the preserved host)
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Milliseconds → "45 min", "2 h", "2 h 15 min".
pub(crate) fn duration(ms: u64) -> String {
    let minutes = ms / 60_000;
    match minutes {
        0 if ms > 0 => "<1 min".into(),
        0 => "0 min".into(),
        1..=59 => format!("{minutes} min"),
        _ if minutes % 60 == 0 => format!("{} h", minutes / 60),
        _ => format!("{} h {} min", minutes / 60, minutes % 60),
    }
}

/// Seconds (or milliseconds) since epoch → "Today", "Yesterday", "3 days ago".
pub(crate) fn relative(at: u64, now: u64) -> String {
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

// ---------------------------------------------------------------------------
// Pure layout rules (ported from the preserved host)
// ---------------------------------------------------------------------------

fn is_native(id: &str) -> bool {
    NATIVE.iter().any(|(native, _)| *native == id)
}

fn default_layout() -> HomeLayout {
    serde_json::to_value(aurora_launcher_lib::home_widgets::HomeLayout::default())
        .ok()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or(HomeLayout {
            widgets: Vec::new(),
        })
}

/// The core's untouched default means "never customized": this host shows its
/// own default (Playtime and Servers) instead. Missing native entries are
/// appended disabled; foreign entries survive untouched.
pub(crate) fn effective(saved: HomeLayout) -> HomeLayout {
    let core_default = default_layout();
    let mut layout = saved.clone();
    if saved == core_default {
        for widget in &mut layout.widgets {
            if is_native(&widget.id) {
                widget.enabled = DEFAULT_ON.contains(&widget.id.as_str());
            }
        }
        let order = |id: &str| NATIVE.iter().position(|(native, _)| *native == id);
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
        // Fill the first row: Playtime beside a wider Servers list.
        for (id, columns) in [("playtime", 2), ("recent-servers", 3)] {
            if let Some(widget) = layout.widgets.iter_mut().find(|w| w.id == id)
                && widget.grid.is_none()
            {
                widget.grid = Some(WidgetGrid {
                    columns,
                    rows: 1,
                    position: None,
                });
            }
        }
    }
    for (id, _) in NATIVE {
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

pub(crate) fn reset_layout(layout: &mut HomeLayout) {
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

pub(crate) fn set_enabled(layout: &mut HomeLayout, id: &str, enabled: bool) {
    if let Some(widget) = layout.widgets.iter_mut().find(|w| w.id == id) {
        widget.enabled = enabled;
    }
    // A newly added widget goes after the visible ones.
    if enabled && let Some(from) = layout.widgets.iter().position(|w| w.id == id) {
        let widget = layout.widgets.remove(from);
        let to = layout
            .widgets
            .iter()
            .rposition(|w| w.enabled && is_native(&w.id))
            .map_or(0, |p| p + 1);
        layout.widgets.insert(to, widget);
    }
}

/// Moves `id` past its visible neighbour (`delta` is -1 or +1).
pub(crate) fn move_widget(layout: &mut HomeLayout, id: &str, delta: i32) {
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

pub(crate) fn place_widget(layout: &mut HomeLayout, id: &str, position: i32) {
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

/// The saved size request: grid dimensions when present, else the size preset.
pub(crate) fn requested_dims(widget: &WidgetPlacement) -> (i32, i32) {
    match &widget.grid {
        Some(grid) => (grid.columns as i32, grid.rows as i32),
        None => (
            if widget.size == "small" { 2 } else { 4 },
            if widget.size == "large" { 2 } else { 1 },
        ),
    }
}

/// Apply one completed gesture. Moving into another card displaces it without
/// overlap, and a move on a narrow viewport never erases the saved wider size.
pub(crate) fn arrange(
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
    let columns = columns.clamp(1, MAX_GRID_COLUMNS);
    let width = width.clamp(1, MAX_GRID_COLUMNS);
    let max_column = (columns - width.min(columns)).min(MAX_GRID_COLUMNS - width);
    layout.widgets[index].grid = Some(WidgetGrid {
        columns: width as u8,
        rows: height.clamp(1, MAX_GRID_ROWS) as u8,
        position: Some([
            column.clamp(0, max_column.max(0)) as u8,
            row.clamp(0, MAX_ANCHOR_ROW) as u8,
        ]),
    });
    // Give the manipulated card first claim, while preserving the saved order.
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
        let mut grid = target.grid.clone().unwrap_or(WidgetGrid {
            columns: w as u8,
            rows: h as u8,
            position: None,
        });
        grid.position = Some([c as u8, r as u8]);
        if target.id == id {
            grid.columns = width as u8;
        }
        target.grid = Some(grid);
    }
}

/// First-fit placement keeps custom dimensions while preventing overlaps at
/// narrow widths. Purely presentational: it never mutates the saved layout.
pub(crate) fn grid_positions(layout: &HomeLayout, columns: i32) -> Vec<(i32, i32, i32, i32)> {
    let columns = columns.clamp(1, MAX_GRID_COLUMNS) as usize;
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
        let widget = widgets[index];
        let width = (requested_dims(widget).0.max(1) as usize).clamp(1, columns);
        let height = (requested_dims(widget).1.max(1) as usize).clamp(1, 3);
        let anchor = widget
            .grid
            .as_ref()
            .and_then(|g| g.position)
            .map(|[c, r]| ((c as usize).min(columns.saturating_sub(width)), r as usize));
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

// ---------------------------------------------------------------------------
// Playtime projection (ported; missing days are zero, never fabricated)
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlaytimeBin {
    pub first: u64,
    pub last: u64,
    pub duration_ms: u64,
}

/// Bounded UTC-day projection over `days` days ending at `today`, split into
/// `count` bins. Days outside the window contribute nothing.
pub(crate) fn playtime_bins(
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

fn chart_peak(bins: &[PlaytimeBin]) -> u64 {
    bins.iter()
        .map(|b| b.duration_ms)
        .max()
        .unwrap_or(0)
        .max(60_000)
}

fn axis_duration(ms: u64) -> String {
    // Keep extreme totals bounded; exact milliseconds stay on accessible bars.
    let hours = ms / 3_600_000;
    if hours >= 1000 {
        format!("{:.1e} h", hours as f64)
    } else {
        duration(ms)
    }
}

/// Height of one bar in a plot `plot_height` tall: scaled into the plot minus
/// 4 px of headroom, and never under 2 px, so an empty day still shows its
/// stub (muted by the caller) exactly like the Slint chart.
pub(crate) fn bar_height(fraction: f32, plot_height: f32) -> f32 {
    (fraction * (plot_height - 4.)).max(2.)
}

/// The line under the chart. The peak lives in the plot's label gutter.
pub(crate) fn chart_note(period: Period, narrow: bool) -> String {
    format!(
        "Chart: last {} days · UTC{}",
        if period == Period::Week { 7 } else { 30 },
        if narrow && period != Period::Week {
            " · 7 summed bins"
        } else {
            ""
        }
    )
}

/// What assistive technology adds to [`chart_note`].
pub(crate) fn chart_note_description(period: Period, narrow: bool) -> String {
    format!(
        "{}{}{}",
        chart_note(period, narrow),
        if period == Period::AllTime {
            ". Total covers all time; earlier daily history is not shown."
        } else {
            ""
        },
        if narrow && period != Period::Week {
            ". Seven summed date bins."
        } else {
            ". Daily bars."
        }
    )
}

fn playtime_total(bins: &[PlaytimeBin]) -> u64 {
    bins.iter()
        .fold(0u64, |sum, b| sum.saturating_add(b.duration_ms))
}

fn lifetime_text(summary: Option<&api::Playtime>) -> (String, String) {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Period {
    Week,
    Month,
    AllTime,
}

impl Period {
    fn caption(self) -> &'static str {
        match self {
            Self::Week => "Last 7 days",
            Self::Month => "Last 30 days",
            Self::AllTime => "All time",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Week => "7 days",
            Self::Month => "30 days",
            Self::AllTime => "All time",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PointView {
    pub label: String,
    pub value: String,
    pub detail: String,
    pub fraction: f32,
}

fn points_from_bins(bins: &[PlaytimeBin], today: u64) -> Vec<PointView> {
    let peak = chart_peak(bins);
    bins.iter()
        .map(|bin| {
            let label = bin_label(bin, today);
            PointView {
                label: label.clone(),
                detail: format!(
                    "{label} · UTC: {} ({} ms)",
                    duration(bin.duration_ms),
                    bin.duration_ms
                ),
                fraction: bin.duration_ms as f32 / peak as f32,
                value: duration(bin.duration_ms),
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct PlaytimeData {
    pub summary: Option<api::Playtime>,
    /// `None`: the read failed. `Some([])`: no daily history exists.
    pub daily: Option<Vec<api::DailyPlaytime>>,
    /// Snapshot anchor (max day, else wall clock), stable across renders.
    pub today: u64,
}

impl PlaytimeData {
    fn from_results(
        summary: ApiResult<api::Playtime>,
        daily: ApiResult<Vec<api::DailyPlaytime>>,
    ) -> Self {
        let daily = daily.ok();
        let today = daily
            .as_ref()
            .and_then(|d| d.iter().map(|d| d.day).max())
            .unwrap_or_else(|| now_secs() / 86_400);
        Self {
            summary: summary.ok(),
            daily,
            today,
        }
    }

    fn history(&self) -> &[api::DailyPlaytime] {
        self.daily.as_deref().unwrap_or_default()
    }
}

pub(crate) struct PlaytimeView {
    pub points: Vec<PointView>,
    pub peak: String,
    pub total: String,
    pub total_detail: String,
    /// Rendered instead of the chart while data is loading or missing.
    pub status: Option<String>,
    pub empty_period: bool,
}

/// The chart projection for one period. A narrow card sums the 30-day range
/// into seven bins; card width changes presentation, never the selection.
pub(crate) fn playtime_view(
    data: Option<&PlaytimeData>,
    period: Period,
    narrow: bool,
    loading: bool,
) -> PlaytimeView {
    let unavailable = |status: &str| PlaytimeView {
        points: Vec::new(),
        peak: String::new(),
        total: String::new(),
        total_detail: String::new(),
        status: Some(status.into()),
        empty_period: false,
    };
    let Some(data) = data else {
        return unavailable("Daily playtime is unavailable.");
    };
    if loading {
        return unavailable("Loading playtime…");
    }
    let bins = match period {
        Period::Week => playtime_bins(data.history(), data.today, 7, 7),
        Period::Month | Period::AllTime if narrow => {
            playtime_bins(data.history(), data.today, 30, 7)
        }
        Period::Month | Period::AllTime => playtime_bins(data.history(), data.today, 30, 30),
    };
    let (total, total_detail) = match period {
        Period::AllTime => lifetime_text(data.summary.as_ref()),
        Period::Week | Period::Month => {
            let available = data.daily.as_ref().is_some_and(|d| !d.is_empty());
            (
                if available {
                    duration(playtime_total(&bins))
                } else {
                    "Total unavailable".into()
                },
                String::new(),
            )
        }
    };
    let status = match &data.daily {
        None => Some("Daily playtime is unavailable.".to_string()),
        Some(days) if days.is_empty() => Some("No daily history available.".to_string()),
        Some(_) => None,
    };
    PlaytimeView {
        points: points_from_bins(&bins, data.today),
        peak: axis_duration(chart_peak(&bins)),
        total,
        total_detail,
        status,
        empty_period: playtime_total(&bins) == 0,
    }
}

// ---------------------------------------------------------------------------
// Server tab projection (ported)
// ---------------------------------------------------------------------------

/// Keep the newest played entries and the newest favorites independently.
/// Never-played favorites belong only to Favorites. Ties keep core ordering.
pub(crate) fn server_tabs(
    entries: &[api::RecentTarget],
) -> (Vec<api::RecentTarget>, Vec<api::RecentTarget>) {
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

/// The bounded enrichment union: recents plus favorites not already present.
pub(crate) fn server_entries(entries: &[api::RecentTarget]) -> Vec<api::RecentTarget> {
    let (mut recents, favorites) = server_tabs(entries);
    for favorite in favorites {
        if !recents.iter().any(|t| t.id == favorite.id) {
            recents.push(favorite);
        }
    }
    recents
}

/// Display-only flattening: trim centred MOTD lines, drop empty ones, take two.
pub(crate) fn server_motd(presentation: &api::ServerPresentation) -> String {
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

// ---------------------------------------------------------------------------
// Instance details (ported wording)
// ---------------------------------------------------------------------------

/// The same content-status wording every surface shows. In-process install
/// activity belongs to the parent that started it; without it an installing
/// instance reports "Incomplete", exactly like the preserved host.
pub(crate) fn content_status(instance: &api::InstanceSummary, busy: bool) -> String {
    if let Some(recovery) = &instance.recovery
        && (recovery.status != "completed" || instance.state != "ready")
    {
        return match recovery.status.as_str() {
            "checking" => "Checking recovery…",
            "busy" => "Installation busy",
            "interrupted" => "Installation interrupted",
            "previousRestored" => "Previous installation restored",
            "manualRepair" => "Recovery required",
            _ => "Recovery completed",
        }
        .into();
    }
    if instance.state == "installing" {
        return if busy { "Installing" } else { "Incomplete" }.into();
    }
    "Installed".into()
}

pub(crate) fn detail_lines(instance: &api::InstanceSummary) -> Vec<(String, String)> {
    let status = content_status(instance, false);
    let configuration = &instance.configuration;
    let mut lines = vec![
        ("Minecraft".into(), instance.minecraft_version.clone()),
        ("Loader".into(), instance.platform_label()),
        (
            "Memory".into(),
            format!("{:.1} GB", configuration.memory_mib as f32 / 1024.0),
        ),
        ("State".into(), status),
    ];
    if let Some(pack) = &instance.pack {
        lines.insert(2, ("Modpack".into(), pack.pack_version.clone()));
    }
    lines.truncate(5);
    lines
}

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub id: String,
    pub instance: String,
    pub label: String,
    /// Saved-destination identity (address or world folder name).
    pub identity: String,
    pub detail: String,
    /// Normalized seconds; 0 means never played.
    pub last_played_at: u64,
    pub playable: bool,
    pub favorite: bool,
    pub unvisited_favorite: bool,
    pub motd: String,
    pub picture: Option<Arc<RenderImage>>,
}

fn world_value(at: u64, now: u64) -> String {
    if at == 0 {
        "Favorite".into()
    } else {
        relative(at, now)
    }
}

fn saved_value(identity: &str, at_ms: u64, worlds: bool, now: u64) -> String {
    if worlds && at_ms != 0 {
        format!("{} · {}", identity, relative(at_ms / 1000, now))
    } else {
        identity.to_string()
    }
}

// ---------------------------------------------------------------------------
// Snapped gesture machine (pure)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Gesture {
    pub id: String,
    pub resize: bool,
    pub origin: (f32, f32),
    /// (column, row, width, height) when the gesture started.
    pub base: (i32, i32, i32, i32),
    /// The saved size request, preserved by pure moves.
    pub requested: (i32, i32),
    pub preview: (i32, i32, i32, i32),
    /// Live viewport columns; width changes are recorded synchronously.
    pub columns: i32,
    pub canceled: bool,
}

impl Gesture {
    pub fn snap(&mut self, dx: f32, dy: f32, pitch_x: f32, pitch_y: f32) {
        let step_x = (dx / pitch_x.max(1.)).round() as i32;
        let step_y = (dy / pitch_y.max(1.)).round() as i32;
        let columns = self.columns.clamp(1, MAX_GRID_COLUMNS);
        if self.resize {
            let max_width = (columns - self.base.0).clamp(1, MAX_GRID_COLUMNS);
            self.preview.2 = (self.base.2 + step_x).clamp(1, max_width);
            self.preview.3 = (self.base.3 + step_y).clamp(1, MAX_GRID_ROWS);
        } else {
            self.preview.0 = (self.base.0 + step_x).clamp(0, (columns - self.base.2).max(0));
            self.preview.1 = (self.base.1 + step_y).clamp(0, MAX_ANCHOR_ROW);
        }
    }

    fn changed(&self) -> bool {
        self.preview != self.base
    }
}

// ---------------------------------------------------------------------------
// Pure page core (testable without a host or window)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServerView {
    Recents,
    Favorites,
    Saved,
}

#[derive(Clone, Debug, PartialEq)]
struct HistoryRead {
    serial: u64,
    favorite_revision: u64,
    instance: Option<String>,
    servers: bool,
    worlds: bool,
    saved_servers: bool,
    saved_worlds: bool,
}

struct HistoryResults {
    playtime: ApiResult<api::Playtime>,
    daily: ApiResult<Vec<api::DailyPlaytime>>,
    servers: ApiResult<Vec<api::RecentTarget>>,
    worlds: ApiResult<Vec<api::RecentTarget>>,
    saved_servers: Option<ApiResult<Vec<api::SavedDestination>>>,
    saved_worlds: Option<ApiResult<Vec<api::SavedDestination>>>,
}

#[derive(Clone, Debug, PartialEq)]
struct StatusRead {
    serial: u64,
    ids: Vec<String>,
    saved_instance: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SaveOutcome {
    /// The submitted snapshot is no longer in flight (a stale reply).
    Ignored,
    Acknowledged,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditOutcome {
    Ignored,
    Applied { reload: bool },
}

struct Core {
    // inputs and gating
    active: bool,
    blocked: bool,
    pending_refresh: bool,
    state: Option<api::LauncherState>,
    state_revision: u64,
    // persisted layout
    layout: Option<HomeLayout>,
    layout_error: Option<String>,
    layout_loading: bool,
    layout_serial: u64,
    save_in_flight: Option<HomeLayout>,
    /// The last known persisted snapshot: a successful read, or the last
    /// submitted write (acknowledged or not). Saves start only when the local
    /// layout diverges from it, so a failed read never fabricates a write.
    persisted: Option<HomeLayout>,
    save_error: Option<String>,
    // history data
    data_serial: u64,
    loading: bool,
    history_error: Option<String>,
    instance: Option<String>,
    playtime: Option<PlaytimeData>,
    servers: Vec<api::RecentTarget>,
    presentations: Vec<api::ServerPresentation>,
    server_pictures: HashMap<String, Arc<RenderImage>>,
    server_error: bool,
    server_enriching: bool,
    worlds: Vec<Row>,
    saved_servers_rows: Vec<Row>,
    saved_worlds_rows: Vec<Row>,
    saved_errors: HashMap<String, String>,
    // explicit server status refresh
    refreshing_servers: bool,
    status_serial: u64,
    server_status: HashMap<String, (String, Option<Arc<RenderImage>>)>,
    // favorites
    favorite_pending: HashSet<String>,
    favorite_revision: u64,
    saved_favorite_in_flight: Option<(String, bool)>,
    // customization
    editing: bool,
    columns: i32,
    gesture: Option<Gesture>,
    period: Period,
    server_view: ServerView,
    saved_worlds: bool,
}

impl Default for Core {
    fn default() -> Self {
        Self {
            active: true,
            blocked: false,
            pending_refresh: false,
            state: None,
            state_revision: 0,
            layout: None,
            layout_error: None,
            layout_loading: false,
            layout_serial: 0,
            save_in_flight: None,
            persisted: None,
            save_error: None,
            data_serial: 0,
            loading: false,
            history_error: None,
            instance: None,
            playtime: None,
            servers: Vec::new(),
            presentations: Vec::new(),
            server_pictures: HashMap::new(),
            server_error: false,
            server_enriching: false,
            worlds: Vec::new(),
            saved_servers_rows: Vec::new(),
            saved_worlds_rows: Vec::new(),
            saved_errors: HashMap::new(),
            refreshing_servers: false,
            status_serial: 0,
            server_status: HashMap::new(),
            favorite_pending: HashSet::new(),
            favorite_revision: 0,
            saved_favorite_in_flight: None,
            editing: false,
            columns: 0,
            gesture: None,
            period: Period::Week,
            server_view: ServerView::Recents,
            saved_worlds: false,
        }
    }
}

impl Core {
    fn is_busy(&self) -> bool {
        self.save_in_flight.is_some()
            || !self.favorite_pending.is_empty()
            || self.saved_favorite_in_flight.is_some()
    }

    fn selected_instance_id(&self) -> Option<&str> {
        self.state.as_ref()?.config.selected_instance_id.as_deref()
    }

    fn instance_exists(&self, id: &str) -> bool {
        self.state
            .as_ref()
            .is_some_and(|s| s.instances.iter().any(|i| i.id == id))
    }

    fn selected_instance(&self) -> Option<&api::InstanceSummary> {
        let id = self.selected_instance_id()?;
        self.state.as_ref()?.instances.iter().find(|i| i.id == id)
    }

    /// instance id → (display name, Quick Play compatible) at the state.
    fn instance_map(&self) -> HashMap<String, (String, bool)> {
        self.state
            .as_ref()
            .map(|s| {
                s.instances
                    .iter()
                    .map(|i| {
                        (
                            i.id.clone(),
                            (
                                i.display_name.clone(),
                                i.minecraft_version == QUICK_PLAY_MINECRAFT,
                            ),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Installs the parent's acknowledged launcher snapshot. Returns whether
    /// the selection-relevant inputs changed. Stale revisions are ignored.
    fn set_state(&mut self, state: Option<api::LauncherState>, revision: u64) -> bool {
        if self.state_revision != 0 && revision < self.state_revision {
            return false;
        }
        let selected = state
            .as_ref()
            .and_then(|s| s.config.selected_instance_id.clone());
        let old_instances = self.instance_map();
        let selection_changed = selected != self.instance;
        self.state = state;
        self.state_revision = revision;
        let changed = selection_changed || old_instances != self.instance_map();
        if changed {
            self.data_serial = self.data_serial.wrapping_add(1);
            self.status_serial = self.status_serial.wrapping_add(1);
            self.instance = selected;
            self.loading = false;
            self.playtime = None;
            self.worlds.clear();
            self.saved_servers_rows.clear();
            self.saved_worlds_rows.clear();
            self.server_status.clear();
            self.refreshing_servers = false;
        }
        changed
    }

    fn visible(&self) -> Vec<&'static str> {
        self.layout
            .as_ref()
            .map(|layout| {
                layout
                    .widgets
                    .iter()
                    .filter(|w| w.enabled)
                    .filter_map(|w| NATIVE.iter().find(|(id, _)| *id == w.id).map(|n| n.0))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The Media card is placed and rendered by the committed layout. Gesture
    /// previews live in `gesture`, never in `layout`, so they cannot change this.
    fn media_card_shown(&self) -> bool {
        self.visible().contains(&"media")
    }

    fn media_should_run(&self, has_host: bool) -> bool {
        media_should_run(has_host, self.active, self.media_card_shown())
    }

    fn render_columns(&self) -> i32 {
        if self.columns == 0 {
            MAX_GRID_COLUMNS
        } else {
            self.columns
        }
    }

    fn set_columns(&mut self, columns: i32) -> bool {
        let columns = columns.clamp(1, MAX_GRID_COLUMNS);
        if self.columns == columns {
            return false;
        }
        self.columns = columns;
        self.cancel_gesture();
        true
    }

    fn placements(&self) -> Vec<(String, (i32, i32, i32, i32))> {
        let Some(layout) = &self.layout else {
            return Vec::new();
        };
        layout
            .widgets
            .iter()
            .filter(|w| w.enabled && is_native(&w.id))
            .zip(grid_positions(layout, self.render_columns()))
            .map(|(w, p)| (w.id.clone(), p))
            .collect()
    }

    fn placement(&self, id: &str) -> Option<(i32, i32, i32, i32)> {
        self.placements()
            .into_iter()
            .find(|(widget, _)| widget == id)
            .map(|(_, p)| p)
    }

    fn widget(&self, id: &str) -> Option<&WidgetPlacement> {
        self.layout.as_ref()?.widgets.iter().find(|w| w.id == id)
    }

    // -- layout reads and writes -------------------------------------------

    fn receive_layout(&mut self, result: &ApiResult<HomeLayout>) {
        self.layout_loading = false;
        match result {
            Ok(saved) => {
                self.layout_error = None;
                self.layout = Some(effective(saved.clone()));
            }
            Err(error) => {
                // A failed or malformed read cannot fabricate a writable
                // default: fall back to the empty effective layout (every
                // native card disabled), surface the failure, and write
                // nothing until a successful explicit read restores authority.
                self.layout_error = Some(error.message.clone());
                self.layout = Some(effective(HomeLayout {
                    widgets: Vec::new(),
                }));
            }
        }
        // Nothing has been written for whatever is now shown.
        self.persisted = self.layout.clone();
    }

    /// Applies one layout change locally. Persistence is a separate serialized
    /// complete-snapshot write performed by the wrapper.
    fn edit(&mut self, change: impl FnOnce(&mut HomeLayout)) -> EditOutcome {
        if self.layout_error.is_some() {
            return EditOutcome::Ignored;
        }
        let Some(mut layout) = self.layout.clone() else {
            return EditOutcome::Ignored;
        };
        let before = layout.clone();
        change(&mut layout);
        if before == layout {
            return EditOutcome::Ignored;
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
        self.layout = Some(layout);
        if visibility_changed {
            self.pending_refresh = true;
        }
        EditOutcome::Applied {
            reload: visibility_changed,
        }
    }

    /// Serialized complete-snapshot writes: at most one in flight; a newer
    /// layout while saving triggers exactly one follow-up save. A layout equal
    /// to the last known persisted snapshot never writes.
    fn begin_save(&mut self) -> Option<HomeLayout> {
        if self.save_in_flight.is_some() || self.layout_error.is_some() {
            return None;
        }
        let layout = self.layout.clone()?;
        if self.persisted.as_ref() == Some(&layout) {
            return None;
        }
        self.save_in_flight = Some(layout.clone());
        Some(layout)
    }

    fn finish_save(
        &mut self,
        submitted: &HomeLayout,
        result: &ApiResult<HomeLayout>,
    ) -> SaveOutcome {
        if self.save_in_flight.as_ref() != Some(submitted) {
            return SaveOutcome::Ignored;
        }
        self.save_in_flight = None;
        self.persisted = Some(submitted.clone());
        match result {
            Ok(_) => {
                self.save_error = None;
                SaveOutcome::Acknowledged
            }
            Err(error) => {
                self.save_error = Some(error.message.clone());
                SaveOutcome::Failed
            }
        }
    }

    // -- history refresh ----------------------------------------------------

    fn begin_data_refresh(&mut self) -> Option<HistoryRead> {
        self.data_serial += 1;
        self.instance = self.selected_instance_id().map(str::to_owned);
        self.saved_errors.clear();
        self.worlds.clear();
        self.saved_servers_rows.clear();
        self.saved_worlds_rows.clear();
        let shown = self.visible();
        let wants_history = shown
            .iter()
            .any(|id| matches!(*id, "playtime" | "recent-servers" | "recent-worlds"));
        self.loading = wants_history;
        if !wants_history {
            return None;
        }
        Some(HistoryRead {
            serial: self.data_serial,
            favorite_revision: self.favorite_revision,
            instance: self.instance.clone(),
            servers: shown.contains(&"recent-servers"),
            worlds: shown.contains(&"recent-worlds"),
            saved_servers: shown.contains(&"recent-servers")
                && self.server_view == ServerView::Saved,
            saved_worlds: shown.contains(&"recent-worlds") && self.saved_worlds,
        })
    }

    /// Applies one history refresh. Stale serials and reads that started before
    /// an acknowledged favorite write are discarded. Returns the bounded
    /// enrichment id list when a cache-first enrichment may start.
    fn receive_history(
        &mut self,
        read: &HistoryRead,
        results: &HistoryResults,
    ) -> Option<Vec<String>> {
        if self.data_serial != read.serial {
            return None;
        }
        self.loading = false;
        self.history_error = None;
        self.playtime = Some(PlaytimeData::from_results(
            results.playtime.clone(),
            results.daily.clone(),
        ));
        let mut enrich = None;
        // A history read begun before a favorite save must not restore its
        // stale star state after the successful write.
        if self.favorite_revision == read.favorite_revision {
            self.server_error = results.servers.is_err();
            self.server_enriching = false;
            self.servers = server_entries(&results.servers.clone().unwrap_or_default());
            self.presentations.clear();
            self.server_pictures.clear();
            let ids: Vec<String> = self
                .servers
                .iter()
                .take(ENRICH_LIMIT)
                .map(|t| t.id.clone())
                .collect();
            if !ids.is_empty() {
                self.server_enriching = true;
                enrich = Some(ids);
            }
        }
        let names = self.instance_map();
        self.worlds = results
            .worlds
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|target| {
                let row = Row {
                    id: target.id,
                    instance: target.instance_id.clone(),
                    label: target.display_name,
                    identity: String::new(),
                    detail: String::new(),
                    last_played_at: target.last_played_at,
                    playable: false,
                    favorite: target.favorite,
                    unvisited_favorite: target.last_played_at == 0,
                    motd: String::new(),
                    picture: None,
                };
                match names.get(&target.instance_id) {
                    None => Row {
                        detail: if target.available {
                            "Instance unavailable".into()
                        } else {
                            "Destination unavailable".into()
                        },
                        ..row
                    },
                    Some((name, compatible)) => Row {
                        detail: if !target.available {
                            "Destination unavailable".into()
                        } else if !compatible {
                            format!("{name} · Quick Play requires {QUICK_PLAY_MINECRAFT}")
                        } else {
                            name.clone()
                        },
                        playable: target.available && *compatible,
                        ..row
                    },
                }
            })
            .collect();
        for (rows, result, key, worlds) in [
            (
                &mut self.saved_servers_rows,
                &results.saved_servers,
                "saved-servers",
                false,
            ),
            (
                &mut self.saved_worlds_rows,
                &results.saved_worlds,
                "saved-worlds",
                true,
            ),
        ] {
            if self.favorite_revision != read.favorite_revision {
                continue;
            }
            let Some(result) = result else {
                continue;
            };
            match result {
                Ok(entries) => {
                    let instance = read.instance.clone().unwrap_or_default();
                    let (name, compatible) = names
                        .get(&instance)
                        .cloned()
                        .unwrap_or(("Instance unavailable".into(), false));
                    *rows = entries
                        .iter()
                        .map(|entry| Row {
                            detail: if entry.detail.is_empty() {
                                name.clone()
                            } else {
                                format!("{name} · {}", entry.detail)
                            },
                            id: entry.id.clone(),
                            instance: instance.clone(),
                            label: entry.display_name.clone(),
                            identity: entry.identity.clone(),
                            last_played_at: if worlds && entry.last_played_ms != 0 {
                                entry.last_played_ms / 1000
                            } else {
                                0
                            },
                            playable: compatible,
                            favorite: entry.favorite,
                            unvisited_favorite: false,
                            motd: String::new(),
                            picture: entry.icon.as_deref().and_then(decode_png),
                        })
                        .collect();
                }
                Err(error) => {
                    self.saved_errors.insert(key.into(), error.message.clone());
                }
            }
        }
        enrich
    }

    fn settle_history(
        &mut self,
        read: &HistoryRead,
        result: &ApiResult<Box<HistoryResults>>,
    ) -> Option<Vec<String>> {
        match result {
            Ok(results) => self.receive_history(read, results),
            Err(error) if self.data_serial == read.serial => {
                self.loading = false;
                self.history_error = Some(error.message.clone());
                self.playtime = None;
                self.servers.clear();
                self.worlds.clear();
                self.saved_servers_rows.clear();
                self.saved_worlds_rows.clear();
                self.server_error = true;
                self.server_enriching = false;
                None
            }
            Err(_) => None,
        }
    }

    fn receive_enrich(&mut self, serial: u64, result: &ApiResult<Vec<api::ServerPresentation>>) {
        if self.data_serial != serial {
            return;
        }
        self.server_enriching = false;
        // A failed enrichment clears presentations honestly (no stale rows).
        let presentations = result
            .as_ref()
            .map(|presentations| presentations.clone())
            .unwrap_or_default();
        self.presentations = presentations;
        self.server_pictures = self
            .presentations
            .iter()
            .filter_map(|p| {
                let favicon = p.favicon.as_deref()?;
                Some((p.target_id.clone(), decode_favicon(favicon)?))
            })
            .collect();
    }

    // -- explicit server status refresh -------------------------------------

    /// Cache-first status refresh for the current tab's rows (bounded to 20).
    fn begin_status_refresh(&mut self) -> Option<StatusRead> {
        if self.refreshing_servers {
            return None;
        }
        let saved = self.server_view == ServerView::Saved;
        let ids: Vec<String> = if saved {
            self.saved_servers_rows
                .iter()
                .map(|row| row.id.clone())
                .take(ENRICH_LIMIT)
                .collect()
        } else {
            let (recents, favorites) = server_tabs(&self.servers);
            let entries = match self.server_view {
                ServerView::Favorites => favorites,
                _ => recents,
            };
            entries
                .iter()
                .map(|target| target.id.clone())
                .take(ENRICH_LIMIT)
                .collect()
        };
        if ids.is_empty() {
            return None;
        }
        self.refreshing_servers = true;
        self.status_serial += 1;
        Some(StatusRead {
            serial: self.status_serial,
            ids,
            saved_instance: if saved { self.instance.clone() } else { None },
        })
    }

    fn receive_status(&mut self, read: &StatusRead, result: &ApiResult<Vec<api::ServerStatus>>) {
        // A stale reply never wedges the refresh flag; statuses apply only to
        // the refresh that is still current.
        let current = self.status_serial == read.serial;
        if !current {
            return;
        }
        self.refreshing_servers = false;
        if current && let Ok(statuses) = result {
            self.server_status = statuses
                .iter()
                .map(|row| {
                    let label = if row.status == "online" {
                        "Online"
                    } else {
                        "No response"
                    };
                    (
                        row.target_id.clone(),
                        (
                            label.to_owned(),
                            row.favicon.as_deref().and_then(decode_favicon),
                        ),
                    )
                })
                .collect();
        }
    }

    // -- favorites ------------------------------------------------------------

    /// Optimistic favorite begin: returns the next value once per target.
    fn begin_favorite(&mut self, id: &str) -> Option<bool> {
        let favorite = !self.servers.iter().find(|t| t.id == id)?.favorite;
        self.favorite_pending
            .insert(id.to_owned())
            .then_some(favorite)
    }

    fn finish_favorite(&mut self, id: &str, favorite: bool, succeeded: bool) -> bool {
        if !self.favorite_pending.remove(id) || !succeeded {
            return false;
        }
        self.favorite_revision += 1;
        if let Some(server) = self.servers.iter_mut().find(|t| t.id == id) {
            server.favorite = favorite;
        }
        true
    }

    // -- gestures ---------------------------------------------------------------

    fn begin_gesture(&mut self, id: &str, resize: bool, origin: (f32, f32)) {
        let Some(base) = self.placement(id) else {
            return;
        };
        let requested = self.widget(id).map(requested_dims).unwrap_or((2, 1));
        self.gesture = Some(Gesture {
            id: id.to_owned(),
            resize,
            origin,
            base,
            requested,
            preview: base,
            columns: self.render_columns(),
            canceled: false,
        });
    }

    fn snap_gesture(&mut self, position: (f32, f32), pitch: (f32, f32)) {
        if let Some(gesture) = &mut self.gesture
            && !gesture.canceled
        {
            let dx = position.0 - gesture.origin.0;
            let dy = position.1 - gesture.origin.1;
            gesture.snap(dx, dy, pitch.0, pitch.1);
        }
    }

    /// A cancelled preview never writes.
    fn cancel_gesture(&mut self) {
        self.gesture = None;
    }

    /// Persists exactly one completed gesture.
    fn commit_gesture(&mut self) -> EditOutcome {
        let Some(gesture) = self.gesture.take() else {
            return EditOutcome::Ignored;
        };
        if gesture.canceled || gesture.columns != self.render_columns() || !gesture.changed() {
            return EditOutcome::Ignored;
        }
        let (width, height) = if gesture.resize {
            (gesture.preview.2, gesture.preview.3)
        } else {
            gesture.requested
        };
        self.edit(|layout| {
            arrange(
                layout,
                &gesture.id,
                gesture.preview.0,
                gesture.preview.1,
                width,
                height,
                gesture.columns,
            )
        })
    }

    /// Keyboard equivalents: each key press is one completed gesture. The
    /// resize handle inverts the Shift convention of the move handle.
    fn keyboard_arrange(&mut self, id: &str, key: &str, resize: bool) -> EditOutcome {
        let (dx, dy) = match key {
            "left" => (-1, 0),
            "right" => (1, 0),
            "up" => (0, -1),
            "down" => (0, 1),
            _ => return EditOutcome::Ignored,
        };
        let Some((column, row, width, height)) = self.placement(id) else {
            return EditOutcome::Ignored;
        };
        let requested = self
            .widget(id)
            .map(requested_dims)
            .unwrap_or((width, height));
        let columns = self.render_columns();
        let (next_column, next_row, next_width, next_height) = if resize {
            let max_width = (columns - column).clamp(1, MAX_GRID_COLUMNS);
            (
                column,
                row,
                (width + dx).clamp(1, max_width),
                (height + dy).clamp(1, MAX_GRID_ROWS),
            )
        } else {
            (
                (column + dx).clamp(0, (columns - width).max(0)),
                (row + dy).clamp(0, MAX_ANCHOR_ROW),
                requested.0,
                requested.1,
            )
        };
        self.edit(|layout| {
            arrange(
                layout,
                id,
                next_column,
                next_row,
                next_width,
                next_height,
                columns,
            )
        })
    }

    // -- Quick Play validation ---------------------------------------------------

    /// Resolves the exact current row identity and its original instance for
    /// one Quick Play intent. Existing modes only; raw paths are never derived.
    fn quick_play_target(&self, mode: &str, id: &str) -> Option<(String, String, String)> {
        let find_row = |rows: &Vec<Row>| {
            rows.iter()
                .find(|row| row.id == id && row.playable && self.instance_exists(&row.instance))
                .map(|row| (row.id.clone(), row.instance.clone()))
        };
        match mode {
            "server" => self
                .servers
                .iter()
                .find(|t| t.id == id && t.available && self.instance_exists(&t.instance_id))
                .map(|t| (mode.to_owned(), t.id.clone(), t.instance_id.clone())),
            "world" => find_row(&self.worlds).map(|(id, instance)| (mode.to_owned(), id, instance)),
            "saved-server" => find_row(&self.saved_servers_rows)
                .map(|(id, instance)| (mode.to_owned(), id, instance)),
            "saved-world" => find_row(&self.saved_worlds_rows)
                .map(|(id, instance)| (mode.to_owned(), id, instance)),
            _ => None,
        }
    }

    fn can_quick_play(&self, mode: &str, id: &str, instance_id: &str) -> bool {
        self.quick_play_target(mode, id)
            .is_some_and(|(_, _, row)| instance_id.is_empty() || instance_id == row)
    }
}

// ---------------------------------------------------------------------------
// Artwork decoding (core-validated inputs only)
// ---------------------------------------------------------------------------

fn decode_png(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let decoded = image::load_from_memory(bytes).ok()?;
    let rgba = decoded.into_rgba8();
    let (width, height) = rgba.dimensions();
    images::rgba_image(width, height, rgba.into_raw())
}

/// Decodes one core-sanitized, bounded static PNG favicon.
fn decode_favicon(data_uri: &str) -> Option<Arc<RenderImage>> {
    use base64::Engine as _;
    let encoded = data_uri.strip_prefix("data:image/png;base64,")?;
    if encoded.len() > 128 * 1024 {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    decode_png(&bytes)
}

// ---------------------------------------------------------------------------
// Async replies
// ---------------------------------------------------------------------------

enum Reply {
    Layout(u64, ApiResult<HomeLayout>),
    History(HistoryRead, ApiResult<Box<HistoryResults>>),
    Enrich(u64, ApiResult<Vec<api::ServerPresentation>>),
    Status(StatusRead, ApiResult<Vec<api::ServerStatus>>),
    Favorite(String, bool, ApiResult<()>),
    SavedFavorite(String, bool, ApiResult<()>),
    Save(HomeLayout, ApiResult<HomeLayout>),
}

// ---------------------------------------------------------------------------
// Home widgets page
// ---------------------------------------------------------------------------

pub struct HomeWidgets {
    host: Option<Host>,
    rt: Handle,
    core: Core,
    focus: FocusHandle,
    grid_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Stable per-widget focus and scroll handles, so keyboard focus and
    /// scroll positions survive data refreshes.
    card_focus: HashMap<String, FocusHandle>,
    card_scroll: HashMap<String, ScrollHandle>,
    busy_published: bool,
    /// Created the first time the Media card is shown; kept (inactive) after
    /// that. Dropping this page drops it, which revokes its worker.
    media: Option<Entity<Media>>,
    /// A first-time creation is awaiting the window; at most one at a time.
    media_creating: bool,
    /// Media's constructor takes a window, which entity updates do not carry.
    window: AnyWindowHandle,
}

impl HomeWidgets {
    /// Builds the inactive page. After activation with a host, the persisted
    /// layout loads first and the visible cards' data follows; with no host it stays in an
    /// unavailable diagnostic state and initiates zero API calls.
    pub fn new(
        host: Option<Host>,
        runtime: Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            host,
            rt: runtime,
            core: Core {
                active: false,
                ..Core::default()
            },
            focus: cx.focus_handle(),
            grid_bounds: Rc::new(Cell::new(None)),
            card_focus: HashMap::new(),
            card_scroll: HashMap::new(),
            busy_published: false,
            media: None,
            media_creating: false,
            window: window.window_handle(),
        };
        view.reload(cx);
        view
    }

    /// Installs the parent's acknowledged launcher snapshot. A changed
    /// selection invalidates Quick Play-relevant rows and schedules a refresh
    /// (deferred while hidden or blocked).
    pub fn set_state(
        &mut self,
        state: Option<api::LauncherState>,
        revision: u64,
        cx: &mut Context<Self>,
    ) {
        let changed = self.core.set_state(state, revision);
        if changed {
            self.core.pending_refresh = true;
        }
        self.dispatch_pending(cx);
        cx.notify();
    }

    /// Page visibility. A hidden page stops starting data work; the deferred
    /// refresh runs on the next actual activation or input change.
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.core.active != active {
            self.core.active = active;
            if !active {
                self.core.cancel_gesture();
            }
        }
        self.dispatch_pending(cx);
        self.sync_media(cx);
        cx.notify();
    }

    /// The parent's gate on new work; running writes still complete.
    pub fn set_blocked(&mut self, blocked: bool, cx: &mut Context<Self>) {
        if self.core.blocked == blocked {
            return;
        }
        self.core.blocked = blocked;
        if blocked {
            self.core.cancel_gesture();
        }
        if !blocked {
            self.dispatch_pending(cx);
        }
        cx.notify();
    }

    /// Explicit reload of the persisted layout and the visible cards' data.
    /// The Media card re-reads its session too, but only while it is shown.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.reload(cx);
        if let Some(media) = &self.media
            && self.core.media_should_run(self.host.is_some())
        {
            media.update(cx, |media, cx| media.refresh(cx));
        }
    }

    /// Layout and card-data loading; also the deferred, non-explicit path.
    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.host.is_none() || !self.core.active || self.core.blocked {
            if self.core.active && !self.core.blocked {
                // No host: the diagnostic state never starts work.
                cx.notify();
                return;
            }
            self.core.pending_refresh = true;
            cx.notify();
            return;
        }
        if self.core.layout.is_none() || self.core.layout_error.is_some() {
            if !self.core.layout_loading {
                self.core.layout_loading = true;
                self.core.layout_serial += 1;
                let serial = self.core.layout_serial;
                let host = self.host.clone().expect("host checked above");
                self.run(
                    cx,
                    async move { api::home_widgets(host).await },
                    move |view, result, cx| view.receive(Reply::Layout(serial, result), cx),
                );
            }
            // Data reads need the visible set; they follow the layout reply.
            self.core.pending_refresh = true;
            cx.notify();
            return;
        }
        self.core.pending_refresh = false;
        self.start_history(cx);
        cx.notify();
    }

    /// Routes one host event. A finished supervised launch invalidates the
    /// recorded history; every other event belongs to its owning module.
    pub fn on_host_event(&mut self, name: &str, payload: &Value, cx: &mut Context<Self>) {
        if name == "launch-state" && matches!(payload["status"].as_str(), Some("exited" | "failed"))
        {
            self.core.pending_refresh = true;
            self.dispatch_pending(cx);
        }
    }

    /// Whether a layout or favorite write is in flight.
    pub fn is_busy(&self) -> bool {
        self.core.is_busy()
    }

    /// Quick Play validation for the parent: the exact current row identity
    /// (and, for saved destinations, its frozen instance) must still exist.
    pub fn can_quick_play(&self, mode: &str, id: &str, instance_id: &str) -> bool {
        self.core.can_quick_play(mode, id, instance_id)
    }

    /// Display name of a recent server destination for the launch progress
    /// line; other modes have no name in the Slint wording.
    pub fn quick_play_server_name(&self, mode: &str, id: &str) -> Option<String> {
        if mode != "server" {
            return None;
        }
        self.core
            .servers
            .iter()
            .find(|target| target.id == id)
            .map(|target| target.display_name.clone())
    }

    fn dispatch_pending(&mut self, cx: &mut Context<Self>) {
        if self.core.active && !self.core.blocked && self.core.pending_refresh {
            self.reload(cx);
        }
    }

    /// Matches the Media worker to the committed layout and page activity.
    /// Idempotent, so repeated calls never restart anything; it is driven by
    /// state transitions, never by `render`, and gesture previews (which live
    /// outside the committed layout) cannot reach it.
    fn sync_media(&mut self, cx: &mut Context<Self>) {
        let run = self.core.media_should_run(self.host.is_some());
        match &self.media {
            Some(media) => media.update(cx, |media, cx| media.set_active(run, cx)),
            None if run => self.create_media(cx),
            None => {}
        }
    }

    /// First-time creation of the Media entity. It needs the window, which an
    /// entity update does not carry, so it completes on a foreground task that
    /// re-checks the committed state when it lands.
    fn create_media(&mut self, cx: &mut Context<Self>) {
        if self.media_creating {
            return;
        }
        self.media_creating = true;
        let (host, runtime, window) = (self.host.clone(), self.rt.clone(), self.window);
        cx.spawn(async move |weak, cx| {
            let created = window.update(cx, |_, window, app| {
                app.new(|cx| Media::new(host, runtime, window, cx))
            });
            let _ = weak.update(cx, |view, cx| view.install_media(created.ok(), cx));
        })
        .detach();
    }

    fn install_media(&mut self, media: Option<Entity<Media>>, cx: &mut Context<Self>) {
        self.media_creating = false;
        // A vanished window leaves nothing to show; do not retry in a loop.
        let Some(media) = media else {
            return;
        };
        self.media.get_or_insert(media);
        self.sync_media(cx);
        cx.notify();
    }

    fn publish_busy(&mut self, cx: &mut Context<Self>) {
        let busy = self.core.is_busy();
        if busy != self.busy_published {
            self.busy_published = busy;
            cx.emit(HomeWidgetsEvent::BusyChanged(busy));
        }
    }

    // -- dispatch -------------------------------------------------------------

    /// Runs one backend call through the panic-safe boundary and delivers the
    /// result through an awaited oneshot channel, never by polling. A dropped
    /// worker (terminal failure) becomes an explicit error reply.
    fn run<T, F>(
        &self,
        cx: &mut Context<Self>,
        work: F,
        deliver: impl Fn(&mut Self, ApiResult<T>, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
        F: std::future::Future<Output = ApiResult<T>> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let _ = tx.send(guarded(work).await);
        });
        cx.spawn(async move |weak, cx| {
            let result = rx.await.unwrap_or_else(|_| {
                Err(api::ApiError::new(
                    "native_operation_failed",
                    "The operation stopped unexpectedly. Refresh Home and try again.",
                ))
            });
            let _ = weak.update(cx, |view, cx| deliver(view, result, cx));
        })
        .detach();
    }

    fn start_history(&mut self, cx: &mut Context<Self>) {
        let Some(read) = self.core.begin_data_refresh() else {
            return;
        };
        let Some(host) = self.host.clone() else {
            return;
        };
        let request = read.clone();
        let reply_read = read;
        self.run(
            cx,
            async move {
                let instance = request.instance.clone();
                let playtime = api::playtime(host.clone(), instance.clone()).await;
                let daily = api::daily_playtime(host.clone(), instance.clone()).await;
                let servers = if request.servers {
                    api::recent_servers(host.clone(), None, SERVER_READ).await
                } else {
                    Ok(Vec::new())
                };
                let worlds = match &instance {
                    Some(id) => {
                        api::recent_worlds(host.clone(), Some(id.clone()), WORLD_READ).await
                    }
                    None => Ok(Vec::new()),
                };
                let saved = |worlds: bool| {
                    let host = host.clone();
                    let instance = instance.clone();
                    async move {
                        match instance {
                            Some(id) => Some(api::saved_destinations(host, id, worlds).await),
                            None => None,
                        }
                    }
                };
                let saved_servers = if request.saved_servers {
                    saved(false).await
                } else {
                    None
                };
                let saved_worlds = if request.saved_worlds {
                    saved(true).await
                } else {
                    None
                };
                Ok(Box::new(HistoryResults {
                    playtime,
                    daily,
                    servers,
                    worlds,
                    saved_servers,
                    saved_worlds,
                }))
            },
            move |view, result: ApiResult<Box<HistoryResults>>, cx| {
                view.receive(Reply::History(reply_read.clone(), result), cx);
            },
        );
    }

    fn start_enrich(&mut self, cx: &mut Context<Self>, ids: Vec<String>) {
        let Some(host) = self.host.clone().filter(|_| !crate::showcase()) else {
            return;
        };
        let serial = self.core.data_serial;
        self.run(
            cx,
            async move { api::server_presentations(host, ids).await },
            move |view, result, cx| view.receive(Reply::Enrich(serial, result), cx),
        );
    }

    fn start_status_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(read) = self.core.begin_status_refresh() else {
            cx.notify();
            return;
        };
        let Some(host) = self.host.clone() else {
            return;
        };
        let request = read.clone();
        let reply_read = read;
        self.run(
            cx,
            async move {
                match request.saved_instance.clone() {
                    Some(instance) => {
                        api::refresh_saved_server_status(host, instance, request.ids.clone()).await
                    }
                    None => api::refresh_server_status(host, request.ids.clone()).await,
                }
            },
            move |view, result, cx| view.receive(Reply::Status(reply_read.clone(), result), cx),
        );
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(submitted) = self.core.begin_save() else {
            return;
        };
        let Some(host) = self.host.clone() else {
            return;
        };
        self.publish_busy(cx);
        let payload = submitted.clone();
        self.run(
            cx,
            async move { api::set_home_widgets(host, payload).await },
            move |view, result, cx| {
                let submitted = submitted.clone();
                view.receive(Reply::Save(submitted, result), cx)
            },
        );
    }

    // -- customization actions --------------------------------------------------

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut HomeLayout)) {
        if self.host.is_none() || !self.core.active || self.core.blocked {
            return;
        }
        match self.core.edit(change) {
            EditOutcome::Applied { reload } => {
                self.save(cx);
                if reload {
                    self.dispatch_pending(cx);
                }
            }
            EditOutcome::Ignored => {}
        }
        self.sync_media(cx);
        cx.notify();
    }

    fn set_editing(&mut self, editing: bool, cx: &mut Context<Self>) {
        if editing
            && (self.host.is_none()
                || !self.core.active
                || self.core.blocked
                || self.core.layout_error.is_some())
        {
            return;
        }
        if self.core.editing == editing {
            return;
        }
        self.core.editing = editing;
        if !editing {
            self.core.cancel_gesture();
        }
        cx.notify();
    }

    fn set_server_view(&mut self, view: ServerView, cx: &mut Context<Self>) {
        let was_saved = self.core.server_view == ServerView::Saved;
        self.core.server_view = view;
        // Entering or leaving the Saved tab changes which lists are read.
        if view == ServerView::Saved || was_saved {
            self.core.pending_refresh = true;
            self.dispatch_pending(cx);
        }
        cx.notify();
    }

    fn set_saved_worlds(&mut self, saved: bool, cx: &mut Context<Self>) {
        if self.core.saved_worlds == saved {
            return;
        }
        self.core.saved_worlds = saved;
        self.core.pending_refresh = true;
        self.dispatch_pending(cx);
        cx.notify();
    }

    fn set_period(&mut self, period: Period, cx: &mut Context<Self>) {
        // Session-only choice; card width changes presentation, never this.
        if self.core.period != period {
            self.core.period = period;
            cx.notify();
        }
    }

    fn toggle_favorite(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.core.active || self.core.blocked || self.core.editing {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.core.server_view == ServerView::Saved {
            if self.core.saved_favorite_in_flight.is_some() {
                return;
            }
            // Saved rows favorite through the address-routed write; the
            // acknowledged list reloads afterwards.
            let Some(row) = self
                .core
                .saved_servers_rows
                .iter()
                .find(|row| row.id == id)
                .cloned()
            else {
                return;
            };
            let value = !row.favorite;
            self.core.saved_favorite_in_flight = Some((id.to_owned(), value));
            self.publish_busy(cx);
            let target = id.to_owned();
            let write_target = target.clone();
            self.run(
                cx,
                async move { api::favorite_server(host, write_target, value).await },
                move |view, result, cx| {
                    view.receive(Reply::SavedFavorite(target.clone(), value, result), cx)
                },
            );
            return;
        }
        let Some(value) = self.core.begin_favorite(id) else {
            return;
        };
        self.publish_busy(cx);
        cx.notify();
        let target = id.to_owned();
        let write_target = target.clone();
        self.run(
            cx,
            async move { api::server_favorite(host, write_target, value).await },
            move |view, result, cx| {
                view.receive(Reply::Favorite(target.clone(), value, result), cx)
            },
        );
    }

    fn quick_play(&mut self, mode: &str, id: &str, cx: &mut Context<Self>) {
        if self.core.blocked || self.core.editing || !self.core.active {
            return;
        }
        if let Some((mode, target_id, instance_id)) = self.core.quick_play_target(mode, id) {
            cx.emit(HomeWidgetsEvent::QuickPlay {
                mode,
                target_id,
                instance_id,
            });
        }
    }

    // -- replies -----------------------------------------------------------------

    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Layout(serial, result) => {
                if serial != self.core.layout_serial {
                    return;
                }
                self.core.receive_layout(&result);
                if result.is_err() {
                    self.core.pending_refresh = false;
                } else if self.core.data_serial == 0 || self.core.pending_refresh {
                    self.dispatch_pending(cx);
                }
            }
            Reply::History(read, result) => {
                if let Some(ids) = self.core.settle_history(&read, &result) {
                    self.start_enrich(cx, ids);
                }
            }
            Reply::Enrich(serial, result) => {
                self.core.receive_enrich(serial, &result);
            }
            Reply::Status(read, result) => {
                self.core.receive_status(&read, &result);
            }
            Reply::Favorite(id, value, result) => {
                if self.core.finish_favorite(&id, value, result.is_ok()) {
                    cx.emit(HomeWidgetsEvent::Changed);
                }
                if let Err(error) = result {
                    self.core.save_error = Some(error.message);
                }
                self.publish_busy(cx);
            }
            Reply::SavedFavorite(id, value, result) => {
                if self.core.saved_favorite_in_flight.as_ref() == Some(&(id.clone(), value)) {
                    self.core.saved_favorite_in_flight = None;
                    match result {
                        Ok(()) => {
                            self.core.favorite_revision =
                                self.core.favorite_revision.wrapping_add(1);
                            cx.emit(HomeWidgetsEvent::Changed);
                            self.core.pending_refresh = true;
                            self.dispatch_pending(cx);
                        }
                        Err(error) => self.core.save_error = Some(error.message),
                    }
                }
                self.publish_busy(cx);
            }
            Reply::Save(submitted, result) => {
                match self.core.finish_save(&submitted, &result) {
                    SaveOutcome::Acknowledged => cx.emit(HomeWidgetsEvent::Changed),
                    SaveOutcome::Failed | SaveOutcome::Ignored => {}
                }
                self.publish_busy(cx);
                self.save(cx);
            }
        }
        // A layout read replaces the committed layout; the others are no-ops.
        self.sync_media(cx);
        cx.notify();
    }

    // -- render helpers ------------------------------------------------------------

    fn card_focus(&mut self, id: &str, cx: &mut Context<Self>) -> FocusHandle {
        self.card_focus
            .entry(id.to_owned())
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    fn card_scroll(&mut self, id: &str) -> ScrollHandle {
        self.card_scroll
            .entry(id.to_owned())
            .or_insert_with(ScrollHandle::new)
            .clone()
    }

    fn pitch(&self) -> (f32, f32) {
        let width = self
            .grid_bounds
            .get()
            .map(|bounds| f32::from(bounds.size.width))
            .unwrap_or(MIN_CARD_WIDTH * MAX_GRID_COLUMNS as f32);
        let columns = responsive_columns(width).max(1);
        let card_width = (width - (columns as f32 - 1.) * GRID_GAP) / columns as f32;
        (card_width + GRID_GAP, CARD_HEIGHT + GRID_GAP)
    }

    /// Records the live viewport column count synchronously from the measured
    /// width, so queued gesture events cannot reinstate an obsolete layout.
    fn record_columns(&mut self, cx: &mut Context<Self>) -> i32 {
        if let Some(width) = self
            .grid_bounds
            .get()
            .map(|bounds| f32::from(bounds.size.width))
        {
            let columns = responsive_columns(width);
            if self.core.set_columns(columns) {
                cx.notify();
            }
        }
        self.core.render_columns()
    }

    /// The server rows for the current view: rich rows for Recents and
    /// Favorites (presentation-enriched), destination rows for Saved.
    fn server_rows(&self, now: u64) -> Vec<ServerRowView> {
        if self.core.server_view == ServerView::Saved {
            return self
                .core
                .saved_servers_rows
                .iter()
                .map(|row| ServerRowView {
                    id: row.id.clone(),
                    name: row.label.clone(),
                    detail: format!(
                        "{}{}",
                        saved_value(&row.identity, 0, false, now),
                        if row.detail.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", row.detail)
                        }
                    ),
                    motd: String::new(),
                    favorite: row.favorite,
                    favorite_busy: false,
                    playable: row.playable,
                    picture: row.picture.clone(),
                })
                .collect();
        }
        let (recents, favorites) = server_tabs(&self.core.servers);
        let entries = match self.core.server_view {
            ServerView::Favorites => favorites,
            _ => recents,
        };
        let instances = self.core.instance_map();
        entries
            .iter()
            .map(|target| {
                let presentation = self
                    .core
                    .presentations
                    .iter()
                    .find(|p| p.target_id == target.id);
                let name = presentation
                    .and_then(|p| p.name.as_deref())
                    .filter(|n| !n.is_empty())
                    .unwrap_or(&target.display_name)
                    .to_owned();
                let instance = instances
                    .get(&target.instance_id)
                    .map(|(name, _)| name.as_str())
                    .unwrap_or("Instance unavailable");
                let mut detail = format!(
                    "{} · {instance}",
                    server_played(server_timestamp(target.last_played_at), now)
                );
                if !target.available {
                    detail.push_str(" · Unavailable");
                }
                if let Some((label, _)) = self.core.server_status.get(&target.id) {
                    detail.push_str(&format!(" · {label}"));
                }
                ServerRowView {
                    id: target.id.clone(),
                    name,
                    detail,
                    motd: presentation.map(server_motd).unwrap_or_default(),
                    favorite: target.favorite,
                    favorite_busy: self.core.favorite_pending.contains(&target.id),
                    playable: target.available && self.core.instance_exists(&target.instance_id),
                    picture: self
                        .core
                        .server_pictures
                        .get(&target.id)
                        .cloned()
                        .or_else(|| {
                            self.core
                                .server_status
                                .get(&target.id)
                                .and_then(|(_, icon)| icon.clone())
                        }),
                }
            })
            .collect()
    }
}

struct ServerRowView {
    id: String,
    name: String,
    detail: String,
    motd: String,
    favorite: bool,
    favorite_busy: bool,
    playable: bool,
    picture: Option<Arc<RenderImage>>,
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn pointer_x(event: &MouseDownEvent) -> f32 {
    f32::from(event.position.x)
}

fn pointer_y(event: &MouseDownEvent) -> f32 {
    f32::from(event.position.y)
}

fn move_x(event: &MouseMoveEvent) -> f32 {
    f32::from(event.position.x)
}

fn move_y(event: &MouseMoveEvent) -> f32 {
    f32::from(event.position.y)
}

impl Render for HomeWidgets {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx).clone();
        let now = now_secs();
        let columns = self.record_columns(cx);
        let editing = self.core.editing;
        let no_host = self.host.is_none();

        let mut page = div()
            .id("home-widgets")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .gap(px(12.))
            .text_color(palette.foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(ui::section_header(
                        &palette,
                        "Widgets",
                        Some("Playtime, servers and more at a glance.".into()),
                    ))
                    .child(
                        AuroraButton::new(
                            "widgets-customize",
                            if editing { "Done" } else { "Customize" },
                        )
                        .variant(if editing {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Default
                        })
                        .size(ButtonSize::Small)
                        .icon(if editing { "check" } else { "tune" })
                        .disabled(
                            no_host
                                || self.core.is_busy()
                                || self.core.blocked
                                || self.core.layout_error.is_some(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_editing(!this.core.editing, cx);
                        })),
                    ),
            );

        if no_host {
            return div().w_full().child(
                page.child(
                    ui::card(&palette).child(ui::muted(
                        &palette,
                        "Home widgets are unavailable: this diagnostic window has no launcher host, so no layout or history is read.",
                    )),
                ),
            );
        }

        if editing {
            let layout = self.core.layout.clone().unwrap_or(HomeLayout {
                widgets: Vec::new(),
            });
            let mut adders = div().flex().flex_wrap().items_center().gap(px(8.));
            let mut any_adders = false;
            for (id, title) in NATIVE {
                if layout.widgets.iter().any(|w| w.id == id && w.enabled) {
                    continue;
                }
                any_adders = true;
                let target = id.to_owned();
                adders = adders.child(
                    AuroraButton::new(format!("widgets-add-{target}"), format!("Add {title}"))
                        .size(ButtonSize::Small)
                        .icon("add")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let target = target.clone();
                            this.edit(cx, |layout| set_enabled(layout, &target, true));
                        })),
                );
            }
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .px(px(16.))
                    .py(px(12.))
                    .rounded(palette.radius_lg)
                    .bg(palette.accent_soft)
                    .border_1()
                    .border_color(palette.border_subtle)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(240.))
                                    .text_size(px(ui::TEXT_CAPTION))
                                    .text_color(palette.muted_foreground)
                                    .child("Drag headers to move · Drag corners to resize · Arrows move, Shift and arrows resize · Escape cancels"),
                            )
                            .child(
                                AuroraButton::new("widgets-reset", "Reset layout")
                                    .size(ButtonSize::Small)
                                    .icon("restart_alt")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.edit(cx, reset_layout);
                                    })),
                            ),
                    )
                    .when(any_adders, |this| this.child(adders)),
            );
        }

        for error in [
            self.core.layout_error.clone(),
            self.core.save_error.clone(),
            self.core.history_error.clone(),
        ]
        .into_iter()
        .flatten()
        {
            page = page.child(
                div()
                    .text_size(px(ui::TEXT_CAPTION))
                    .text_color(palette.danger)
                    .child(error),
            );
        }
        if self.core.layout_error.is_some() || self.core.history_error.is_some() {
            page = page.child(
                div().flex().child(
                    AuroraButton::new("widgets-retry", "Retry Home")
                        .size(ButtonSize::Small)
                        .icon("refresh")
                        .disabled(self.core.blocked || self.core.layout_loading)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                ),
            );
        }

        let Some(layout) = self.core.layout.clone() else {
            return div()
                .w_full()
                .child(page.child(ui::muted(&palette, "Loading the saved widget layout…")));
        };

        let placements: Vec<(WidgetPlacement, (i32, i32, i32, i32))> = layout
            .widgets
            .iter()
            .filter(|w| w.enabled && is_native(&w.id))
            .cloned()
            .zip(grid_positions(&layout, columns))
            .collect();
        let grid_rows = placements
            .iter()
            .map(|(_, (_, row, _, height))| row + height)
            .max()
            .unwrap_or(0);
        let (pitch_x, pitch_y) = self.pitch();
        let card_width = pitch_x - GRID_GAP;
        let bounds_cell = self.grid_bounds.clone();
        let viewport_view = cx.entity().downgrade();
        let rows = if editing {
            (grid_rows + 2).max(3)
        } else {
            grid_rows
        };

        let mut grid = div()
            .id("widgets-grid")
            .relative()
            .w_full()
            .h(px(rows as f32 * pitch_y))
            .on_prepaint(move |bounds, _, app| {
                let previous = bounds_cell.replace(Some(bounds));
                let width_changed = previous.is_none_or(|old| old.size.width != bounds.size.width);
                if width_changed {
                    let _ = viewport_view.update(app, |view, cx| {
                        // Record geometry immediately; queued pointer callbacks
                        // must not commit coordinates from the previous width.
                        view.core.cancel_gesture();
                        view.core
                            .set_columns(responsive_columns(f32::from(bounds.size.width)));
                        let weak = cx.entity().downgrade();
                        cx.defer(move |cx| {
                            let _ = weak.update(cx, |_, cx| cx.notify());
                        });
                    });
                }
            })
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.core.gesture.is_some() && event.pressed_button == Some(MouseButton::Left) {
                    let pitch = this.pitch();
                    this.core
                        .snap_gesture((move_x(event), move_y(event)), pitch);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.core.gesture.is_some() {
                        this.finish_gesture(cx);
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.core.gesture.is_some() {
                        this.core.cancel_gesture();
                        cx.notify();
                    }
                }),
            );

        if editing {
            for row in 0..rows {
                for column in 0..columns {
                    grid = grid.child(
                        div()
                            .absolute()
                            .top(px(row as f32 * pitch_y))
                            .left(px(column as f32 * pitch_x))
                            .w(px(card_width))
                            .h(px(CARD_HEIGHT))
                            .rounded(palette.radius_lg)
                            .border_1()
                            .border_color(palette.border_subtle),
                    );
                }
            }
        }

        let busy = self.core.is_busy();
        let blocked = self.core.blocked;
        let loading = self.core.loading;
        let has_instance = self.core.selected_instance().is_some();
        let playtime_data = self.core.playtime.clone();
        let period = self.core.period;
        let active_gesture = self.core.gesture.clone();

        for (index, (widget, (column, row, width, height))) in placements
            .iter()
            .enumerate()
            .map(|(i, (w, p))| (i, (w.clone(), *p)))
        {
            let id = widget.id.clone();
            let title = NATIVE
                .iter()
                .find(|(native, _)| *native == id)
                .map(|(_, title)| *title)
                .unwrap_or("");
            let card = self.render_card(
                &widget,
                title,
                index,
                now,
                pitch_x,
                editing,
                blocked,
                busy,
                loading,
                has_instance,
                playtime_data.as_ref(),
                period,
                cx,
            );
            grid = grid.child(
                div()
                    .absolute()
                    .top(px(row as f32 * pitch_y))
                    .left(px(column as f32 * pitch_x))
                    .w(px(width as f32 * pitch_x - GRID_GAP))
                    .h(px(height as f32 * pitch_y - GRID_GAP))
                    .child(card),
            );

            if !editing {
                continue;
            }

            // Header drag handle with the keyboard move equivalent.
            let handle_focus = self.card_focus(&format!("{id}-move"), cx);
            let move_id = id.clone();
            let owns_gesture = active_gesture
                .as_ref()
                .is_some_and(|g| !g.resize && g.id == id);
            let key_id = move_id.clone();
            let handle = div()
                .id(format!("widget-move-{id}"))
                .role(Role::Button)
                .aria_label(format!("Move {title}"))
                .aria_description("Arrows move. Shift and arrows resize. Escape cancels.")
                .track_focus(&handle_focus)
                .tab_stop(true)
                .absolute()
                .top(px(8.))
                .left(px(8.))
                .w(px(((width as f32 * pitch_x - GRID_GAP) - 54.).max(40.)))
                .h(px(40.))
                .rounded(palette.radius)
                .when(owns_gesture, |this| {
                    this.border_1().border_color(palette.ring)
                })
                .focus(move |style| style.border_color(palette.ring))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        handle_focus.focus(window, cx);
                        let origin = (pointer_x(event), pointer_y(event));
                        match &mut this.core.gesture {
                            Some(gesture) if gesture.id == move_id => gesture.origin = origin,
                            Some(_) => {}
                            None => this.core.begin_gesture(&move_id, false, origin),
                        }
                        cx.notify();
                    }),
                )
                .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                    if event.pressed_button != Some(MouseButton::Left) {
                        return;
                    }
                    this.core
                        .snap_gesture((move_x(event), move_y(event)), (pitch_x, pitch_y));
                    cx.notify();
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.finish_gesture(cx);
                    }),
                )
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    let key = event.keystroke.key.as_str();
                    if key == "escape" {
                        this.core.cancel_gesture();
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    if matches!(key, "left" | "right" | "up" | "down") {
                        this.apply_keyboard_gesture(
                            &key_id,
                            key,
                            event.keystroke.modifiers.shift,
                            cx,
                        );
                    }
                }));
            grid = grid.child(
                div()
                    .absolute()
                    .top(px(row as f32 * pitch_y))
                    .left(px(column as f32 * pitch_x))
                    .w(px(width as f32 * pitch_x - GRID_GAP))
                    .h(px(height as f32 * pitch_y - GRID_GAP))
                    .child(handle),
            );

            // Corner resize handle with the keyboard resize equivalent.
            let resize_focus = self.card_focus(&format!("{id}-resize"), cx);
            let resize_id = id.clone();
            let resize_key_id = resize_id.clone();
            let owns_resize = active_gesture
                .as_ref()
                .is_some_and(|g| g.resize && g.id == id);
            let corner = div()
                .id(format!("widget-resize-{id}"))
                .role(Role::Button)
                .aria_label(format!("Resize {title}"))
                .aria_description("Arrows resize. Escape cancels.")
                .track_focus(&resize_focus)
                .tab_stop(true)
                .absolute()
                .top(px(height as f32 * pitch_y - GRID_GAP - 32.))
                .left(px(width as f32 * pitch_x - GRID_GAP - 32.))
                .w(px(32.))
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.))
                .text_color(palette.muted_foreground)
                .rounded(palette.radius)
                .when(owns_resize, |this| {
                    this.border_1().border_color(palette.ring)
                })
                .focus(move |style| style.border_color(palette.ring))
                .hover(move |style| style.border_color(palette.accent))
                .child("◢")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        resize_focus.focus(window, cx);
                        let origin = (pointer_x(event), pointer_y(event));
                        match &mut this.core.gesture {
                            Some(gesture) if gesture.id == resize_id => gesture.origin = origin,
                            Some(_) => {}
                            None => this.core.begin_gesture(&resize_id, true, origin),
                        }
                        cx.notify();
                    }),
                )
                .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                    if event.pressed_button != Some(MouseButton::Left) {
                        return;
                    }
                    this.core
                        .snap_gesture((move_x(event), move_y(event)), (pitch_x, pitch_y));
                    cx.notify();
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.finish_gesture(cx);
                    }),
                )
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    let key = event.keystroke.key.as_str();
                    if key == "escape" {
                        this.core.cancel_gesture();
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    if matches!(key, "left" | "right" | "up" | "down") {
                        // The resize handle resizes by default; Shift moves.
                        this.apply_keyboard_gesture(
                            &resize_key_id,
                            key,
                            !event.keystroke.modifiers.shift,
                            cx,
                        );
                    }
                }));
            grid = grid.child(
                div()
                    .absolute()
                    .top(px(row as f32 * pitch_y))
                    .left(px(column as f32 * pitch_x))
                    .w(px(width as f32 * pitch_x - GRID_GAP))
                    .h(px(height as f32 * pitch_y - GRID_GAP))
                    .child(corner),
            );
        }

        // Snapped transient outlines: accent preview plus danger collisions.
        // Cancelled previews and uncommitted gestures never write.
        if editing && let Some(gesture) = active_gesture.as_ref().filter(|g| !g.canceled) {
            let preview = gesture.preview;
            let base = gesture.base;
            grid = grid.child(
                div()
                    .absolute()
                    .top(px(preview.1 as f32 * pitch_y))
                    .left(px(preview.0 as f32 * pitch_x))
                    .w(px(preview.2 as f32 * pitch_x - GRID_GAP))
                    .h(px(preview.3 as f32 * pitch_y - GRID_GAP))
                    .rounded(palette.radius_lg)
                    .border_2()
                    .border_color(palette.accent),
            );
            if preview != base {
                for (_, geometry) in placements.iter() {
                    let (column, row, width, height) = *geometry;
                    let overlaps = preview.0 < column + width
                        && column < preview.0 + preview.2
                        && preview.1 < row + height
                        && row < preview.1 + preview.3;
                    if overlaps {
                        grid = grid.child(
                            div()
                                .absolute()
                                .top(px(row as f32 * pitch_y))
                                .left(px(column as f32 * pitch_x))
                                .w(px(width as f32 * pitch_x - GRID_GAP))
                                .h(px(height as f32 * pitch_y - GRID_GAP))
                                .rounded(palette.radius_lg)
                                .border_2()
                                .border_color(palette.danger),
                        );
                    }
                }
            }
        }

        div().w_full().child(page.child(div().w_full().child(grid)))
    }
}

impl HomeWidgets {
    fn finish_gesture(&mut self, cx: &mut Context<Self>) {
        if self.host.is_none() || !self.core.active || self.core.blocked {
            self.core.cancel_gesture();
            cx.notify();
            return;
        }
        match self.core.commit_gesture() {
            EditOutcome::Applied { reload } => {
                self.save(cx);
                if reload {
                    self.dispatch_pending(cx);
                }
            }
            EditOutcome::Ignored => {}
        }
        self.sync_media(cx);
        cx.notify();
    }

    fn apply_keyboard_gesture(
        &mut self,
        id: &str,
        key: &str,
        resize: bool,
        cx: &mut Context<Self>,
    ) {
        if self.host.is_none() || !self.core.active || self.core.blocked {
            return;
        }
        match self.core.keyboard_arrange(id, key, resize) {
            EditOutcome::Applied { reload } => {
                self.save(cx);
                if reload {
                    self.dispatch_pending(cx);
                }
            }
            EditOutcome::Ignored => {}
        }
        self.sync_media(cx);
        cx.stop_propagation();
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    fn render_card(
        &mut self,
        widget: &WidgetPlacement,
        title: &'static str,
        _index: usize,
        now: u64,
        pitch_x: f32,
        editing: bool,
        blocked: bool,
        busy: bool,
        loading: bool,
        has_instance: bool,
        playtime_data: Option<&PlaytimeData>,
        period: Period,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = theme::theme(cx).clone();
        let view = cx.entity();
        let id = widget.id.clone();
        // Rendered width of this card, used only to pick a header layout.
        let card_px = requested_dims(widget).0.max(1) as f32 * pitch_x - GRID_GAP;
        let wide = card_px >= 420.;

        // Transient work never reads as part of the title: it is a quiet
        // dot and caption beside the controls.
        let activity: Option<&'static str> = match id.as_str() {
            "recent-servers" if self.core.refreshing_servers => Some("Refreshing status…"),
            "recent-servers" if self.core.server_enriching && !loading => Some("Loading details…"),
            _ => None,
        };
        let caption: Option<String> = match id.as_str() {
            "instance-details" => self
                .core
                .selected_instance()
                .map(|i| i.display_name.clone()),
            _ => None,
        };
        let shows_loading =
            loading && matches!(id.as_str(), "playtime" | "recent-servers" | "recent-worlds");

        // The widget's own controls: segmented tabs (and quiet actions) go in
        // the header when the card is wide enough, on their own row otherwise.
        let mut tabs: Option<AnyElement> = None;
        let mut extra: Option<AnyElement> = None;
        match id.as_str() {
            "playtime" => {
                tabs = Some(
                    segmented(&palette)
                        .children(
                            [Period::Week, Period::Month, Period::AllTime].map(|choice| {
                                segment(
                                    format!("playtime-period-{}", choice.label()),
                                    choice.label(),
                                    period == choice,
                                )
                                .on_click({
                                    let view = view.clone();
                                    move |_, _, cx| {
                                        view.update(cx, |this, cx| this.set_period(choice, cx));
                                    }
                                })
                            }),
                        )
                        .into_any_element(),
                );
            }
            "recent-servers" => {
                let server_view = self.core.server_view;
                tabs = Some(
                    segmented(&palette)
                        .child(
                            segment(
                                "servers-recent",
                                "Recent",
                                server_view == ServerView::Recents,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_server_view(ServerView::Recents, cx);
                            })),
                        )
                        .child(
                            segment(
                                "servers-favorites",
                                "Favorites",
                                server_view == ServerView::Favorites,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_server_view(ServerView::Favorites, cx);
                            })),
                        )
                        .child(
                            segment("servers-saved", "Saved", server_view == ServerView::Saved)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_server_view(ServerView::Saved, cx);
                                })),
                        )
                        .into_any_element(),
                );
                extra = Some(
                    icon_button(
                        &palette,
                        "servers-refresh".to_owned(),
                        "refresh",
                        "Refresh status".to_owned(),
                        self.core.refreshing_servers || busy,
                        false,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.start_status_refresh(cx);
                    }))
                    .into_any_element(),
                );
            }
            "recent-worlds" => {
                let saved_worlds = self.core.saved_worlds;
                tabs = Some(
                    segmented(&palette)
                        .child(segment("worlds-recent", "Recent", !saved_worlds).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.set_saved_worlds(false, cx);
                            }),
                        ))
                        .child(segment("worlds-saved", "Saved", saved_worlds).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.set_saved_worlds(true, cx);
                            }),
                        ))
                        .into_any_element(),
                );
            }
            _ => {}
        }
        let inline_controls = wide && !editing;

        let mut header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_x(px(12.))
            .gap_y(px(8.))
            .min_h(px(28.))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.))
                    .min_w(px(0.))
                    .when(editing, |this| {
                        this.items_center().child(crate::symbols::icon(
                            Some("drag_indicator"),
                            px(16.),
                            palette.subtle_foreground,
                        ))
                    })
                    .child(
                        div()
                            .text_size(px(ui::TEXT_BODY + 1.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.foreground)
                            .whitespace_nowrap()
                            .child(title),
                    )
                    .when_some(caption, |this, caption| {
                        this.child(
                            div()
                                .min_w(px(0.))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .text_size(px(ui::TEXT_CAPTION))
                                .text_color(palette.subtle_foreground)
                                .child(caption),
                        )
                    }),
            );
        let mut right = div().flex().items_center().gap(px(8.));
        if let Some(activity) = activity.or(shows_loading.then_some("Loading…")) {
            right = right.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .size(px(6.))
                            .rounded_full()
                            .bg(palette.ring)
                            .opacity(0.8),
                    )
                    .child(
                        div()
                            .text_size(px(ui::TEXT_CAPTION))
                            .text_color(palette.subtle_foreground)
                            .whitespace_nowrap()
                            .child(activity),
                    ),
            );
        }
        if inline_controls {
            if let Some(tabs) = tabs.take() {
                right = right.child(tabs);
            }
            if let Some(extra) = extra.take() {
                right = right.child(extra);
            }
        }
        if editing {
            let target = id.clone();
            right = right.child(
                icon_button(
                    &palette,
                    format!("widget-remove-{id}"),
                    "close",
                    format!("Remove {title}"),
                    false,
                    false,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let target = target.clone();
                    this.edit(cx, |layout| set_enabled(layout, &target, false));
                })),
            );
        }
        header = header.child(right);

        let mut card = div()
            .id(format!("widget-card-{id}"))
            .role(Role::Group)
            .aria_label(title)
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .size_full()
            .rounded(palette.radius_lg)
            .bg(palette.card)
            .border_1()
            .border_color(if editing {
                palette.border
            } else {
                palette.border_subtle
            })
            .overflow_hidden()
            .child(header);
        if !inline_controls && (tabs.is_some() || extra.is_some()) {
            card = card.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .children(tabs.take())
                    .children(extra.take()),
            );
        }

        match id.as_str() {
            "playtime" => {
                let narrow = card_px < 440.;
                let projection = playtime_view(playtime_data, period, narrow, loading);
                let total = div()
                    .id("playtime-total")
                    .text_size(px(ui::TEXT_TITLE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.foreground)
                    .child(if loading {
                        "Loading…".to_string()
                    } else {
                        projection.total.clone()
                    });
                let total = if projection.total_detail.is_empty() {
                    total
                } else {
                    total.aria_description(projection.total_detail.clone())
                };
                card = card.child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(8.))
                        .child(total)
                        .child(
                            div()
                                .text_size(px(ui::TEXT_CAPTION))
                                .text_color(palette.subtle_foreground)
                                .child(period.caption()),
                        ),
                );
                match &projection.status {
                    Some(status) => {
                        card = card.child(empty_note(&palette, "bar_chart", status.clone()));
                    }
                    None => {
                        let label = |text: String| {
                            div()
                                .text_size(px(ui::TEXT_CAPTION))
                                .text_color(palette.muted_foreground)
                                .child(text)
                        };
                        let today = playtime_data.map_or(0, |data| data.today);
                        let count = projection.points.len();
                        // Weekday under each bar for the 7-day range; the two
                        // ends of the range otherwise.
                        let axis = if period == Period::Week && count == 7 {
                            div()
                                .flex()
                                .h(px(AXIS_HEIGHT))
                                .items_end()
                                .children((0..count).map(|index| {
                                    let day = today.saturating_sub((count - 1 - index) as u64);
                                    let last = index + 1 == count;
                                    div()
                                        .flex_1()
                                        .flex()
                                        .justify_center()
                                        .text_size(px(ui::TEXT_CAPTION))
                                        .when(last, |this| this.font_weight(FontWeight::SEMIBOLD))
                                        .text_color(if last {
                                            palette.foreground
                                        } else {
                                            palette.muted_foreground
                                        })
                                        .child(if last { "Today" } else { weekday_label(day) })
                                }))
                        } else {
                            div()
                                .flex()
                                .h(px(AXIS_HEIGHT))
                                .items_end()
                                .justify_between()
                                .child(label(format!(
                                    "{} days ago",
                                    if period == Period::Week { 7 } else { 30 }
                                )))
                                .child(
                                    label("Today".into())
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(palette.foreground),
                                )
                        };
                        card = card
                            .child(
                                div()
                                    .id("playtime-chart")
                                    .role(Role::Group)
                                    .aria_label("Playtime chart")
                                    .flex()
                                    .w_full()
                                    .flex_1()
                                    .min_h(px(40.))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .justify_between()
                                            .flex_shrink_0()
                                            .pb(px(AXIS_HEIGHT))
                                            .pr(px(8.))
                                            .child(label(projection.peak.clone()))
                                            .child(label("0 min".into())),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .child(
                                                div()
                                                    .relative()
                                                    .flex()
                                                    .items_end()
                                                    .flex_1()
                                                    .border_b_1()
                                                    .border_color(palette.border)
                                                    .children([0., 0.5].map(|at| {
                                                        div()
                                                            .absolute()
                                                            .top(gpui_kit::relative(at))
                                                            .left(px(0.))
                                                            .right(px(0.))
                                                            .h(px(1.))
                                                            .bg(palette.border_subtle)
                                                    }))
                                                    .when(projection.empty_period, |plot| {
                                                        plot.child(
                                                            div()
                                                                .absolute()
                                                                .size_full()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .text_size(px(ui::TEXT_CAPTION))
                                                                .text_color(
                                                                    palette.muted_foreground,
                                                                )
                                                                .child(
                                                                    "No playtime in this period.",
                                                                ),
                                                        )
                                                    })
                                                    .children(projection.points.iter().map(
                                                        |point| {
                                                            div()
                                                                .id(format!(
                                                                    "playtime-bar-{}",
                                                                    point.label
                                                                ))
                                                                .role(Role::Image)
                                                                .aria_label(point.detail.clone())
                                                                .flex_1()
                                                                .h_full()
                                                                .px(px(if count > 12 {
                                                                    2.
                                                                } else {
                                                                    5.
                                                                }))
                                                                .flex()
                                                                .items_end()
                                                                .justify_center()
                                                                .child(
                                                                    div()
                                                                        .w_full()
                                                                        .min_w(px(2.))
                                                                        .max_w(px(26.))
                                                                        .h(relative_bar(
                                                                            point.fraction,
                                                                        ))
                                                                        .rounded_t(px(4.))
                                                                        .rounded_b(px(1.))
                                                                        .bg(
                                                                            if point.fraction > 0. {
                                                                                palette.accent
                                                                            } else {
                                                                                palette
                                                                                    .control_hover
                                                                            },
                                                                        ),
                                                                )
                                                        },
                                                    )),
                                            )
                                            .child(axis.pt(px(4.))),
                                    ),
                            )
                            .child(
                                div()
                                    .id("playtime-chart-note")
                                    .aria_description(chart_note_description(period, narrow))
                                    .text_size(px(ui::TEXT_MICRO))
                                    .text_color(palette.subtle_foreground)
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(chart_note(period, narrow)),
                            );
                    }
                }
            }
            "recent-servers" => {
                let server_view = self.core.server_view;
                let rows = self.server_rows(now);
                if rows.is_empty() {
                    let empty = if loading {
                        "Loading servers…".to_string()
                    } else if let Some(error) = self.core.saved_errors.get("saved-servers") {
                        error.clone()
                    } else if self.core.server_error {
                        "Server history is unavailable.".to_string()
                    } else {
                        match server_view {
                            ServerView::Recents => {
                                "Servers you have played appear here.".to_string()
                            }
                            ServerView::Favorites => "No favorite servers yet.".to_string(),
                            ServerView::Saved => {
                                if has_instance {
                                    "No saved servers in this instance.".to_string()
                                } else {
                                    "Choose an instance to browse its saved servers.".to_string()
                                }
                            }
                        }
                    };
                    card = card.child(empty_note(&palette, "dns", empty));
                } else {
                    let scroll = self.card_scroll("recent-servers");
                    let mut list = div()
                        .id("servers-list")
                        .role(Role::List)
                        .aria_label("Server list")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.))
                        .overflow_y_scroll()
                        .track_scroll(&scroll);
                    let last_index = rows.len().saturating_sub(1);
                    for (index, row) in rows.into_iter().enumerate() {
                        let mode = if server_view == ServerView::Saved {
                            "saved-server"
                        } else {
                            "server"
                        };
                        let playable = row.playable && !busy && !blocked && !editing;
                        let name = row.name.clone();
                        let favorite_target = row.id.clone();
                        let play_target = row.id.clone();
                        let motd = row.motd.replace('\n', " · ");
                        list = list.child(
                            list_row(&palette, index == last_index)
                                .id(format!("server-row-{}", row.id))
                                .child(avatar(&palette, row.picture.clone(), "dns"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(1.))
                                        .flex_1()
                                        .min_w(px(0.))
                                        .child(row_title(&palette, row.name.clone()))
                                        .child(row_detail(&palette, row.detail.clone()))
                                        .when(!motd.is_empty(), |this| {
                                            this.child(
                                                row_detail(&palette, motd.clone())
                                                    .text_color(palette.subtle_foreground),
                                            )
                                        }),
                                )
                                .child(
                                    star_button(
                                        &palette,
                                        format!("server-favorite-{}", row.id),
                                        row.favorite,
                                        // Self-describing accessible name.
                                        if row.favorite {
                                            format!("Unfavorite {name}")
                                        } else {
                                            format!("Favorite {name}")
                                        },
                                        busy || editing || row.favorite_busy,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.toggle_favorite(&favorite_target, cx);
                                        },
                                    )),
                                )
                                .child(
                                    AuroraButton::new(format!("server-play-{}", row.id), "Play")
                                        .size(ButtonSize::Small)
                                        .icon("play_arrow")
                                        .disabled(!playable)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.quick_play(mode, &play_target, cx);
                                        })),
                                ),
                        );
                    }
                    card = card.child(list);
                }
            }
            "recent-worlds" => {
                let saved_worlds = self.core.saved_worlds;
                let rows = if saved_worlds {
                    self.core.saved_worlds_rows.clone()
                } else {
                    self.core.worlds.clone()
                };
                if rows.is_empty() {
                    let empty = if loading {
                        "Loading destinations…".to_string()
                    } else if let Some(error) = self.core.saved_errors.get("saved-worlds") {
                        error.clone()
                    } else if saved_worlds {
                        if has_instance {
                            "No saved worlds in this instance.".to_string()
                        } else {
                            "Choose an instance to browse its saved worlds.".to_string()
                        }
                    } else {
                        "No worlds yet. Singleplayer worlds you play in this instance appear here."
                            .to_string()
                    };
                    card = card.child(empty_note(&palette, "landscape", empty));
                } else {
                    let scroll = self.card_scroll("recent-worlds");
                    let mut list = div()
                        .id("worlds-list")
                        .role(Role::List)
                        .aria_label("World list")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(0.))
                        .overflow_y_scroll()
                        .track_scroll(&scroll);
                    let last_index = rows.len().saturating_sub(1);
                    for (index, row) in rows.into_iter().enumerate() {
                        let target = row.id.clone();
                        let mode = if saved_worlds { "saved-world" } else { "world" };
                        let playable = row.playable && !busy && !blocked && !editing;
                        let value = if row.identity.is_empty() {
                            world_value(row.last_played_at, now)
                        } else {
                            saved_value(
                                &row.identity,
                                row.last_played_at.saturating_mul(1000),
                                true,
                                now,
                            )
                        };
                        let detail = if row.detail.is_empty() {
                            value
                        } else {
                            format!("{value} · {}", row.detail)
                        };
                        list = list.child(
                            list_row(&palette, index == last_index)
                                .id(format!("world-row-{}", row.id))
                                .child(avatar(&palette, row.picture.clone(), "landscape"))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(1.))
                                        .flex_1()
                                        .min_w(px(0.))
                                        .child(row_title(&palette, row.label.clone()))
                                        .child(row_detail(&palette, detail)),
                                )
                                .child(
                                    AuroraButton::new(format!("world-play-{}", row.id), "Play")
                                        .size(ButtonSize::Small)
                                        .icon("play_arrow")
                                        .disabled(!playable)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let target = target.clone();
                                            this.quick_play(mode, &target, cx);
                                        })),
                                ),
                        );
                    }
                    card = card.child(list);
                }
            }
            "instance-details" => {
                let lines = self
                    .core
                    .selected_instance()
                    .map(detail_lines)
                    .unwrap_or_default();
                if lines.is_empty() {
                    card = card.child(empty_note(
                        &palette,
                        "info",
                        "Choose an instance to see its details.",
                    ));
                } else {
                    let last_index = lines.len() - 1;
                    let mut list = div().flex().flex_col();
                    for (index, (label, value)) in lines.into_iter().enumerate() {
                        list = list.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(12.))
                                .py(px(8.))
                                .when(index != last_index, |this| {
                                    this.border_b_1().border_color(palette.border_subtle)
                                })
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(px(ui::TEXT_BODY))
                                        .text_color(palette.muted_foreground)
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .min_w(px(0.))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_size(px(ui::TEXT_BODY))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(palette.foreground)
                                        .child(value),
                                ),
                        );
                    }
                    card = card.child(list);
                }
            }
            // The entity appears one task after the card is first shown.
            "media" => {
                if let Some(media) = &self.media {
                    card = card.child(media.clone());
                }
            }
            _ => {}
        }
        card.into_any_element()
    }
}

// ---------------------------------------------------------------------------
// Small presentation helpers (private to the widget cards)
// ---------------------------------------------------------------------------

/// Height of the weekday / range axis under the chart.
const AXIS_HEIGHT: f32 = 20.;

/// The plot's bar: a fraction of the plot, never under 2 px. The plot's own
/// height is flexible, so the bar is a percentage of it.
fn relative_bar(fraction: f32) -> gpui_kit::DefiniteLength {
    gpui_kit::relative((fraction.clamp(0., 1.) * 0.94).max(0.02))
}

fn weekday_label(day: u64) -> &'static str {
    // Day 0 (1970-01-01) was a Thursday.
    ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][((day + 4) % 7) as usize]
}

/// A pill holding segmented Small buttons (range and view tabs).
fn segmented(t: &theme::AuroraTheme) -> Div {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(2.))
        .p(px(2.))
        .rounded(t.radius)
        .bg(t.sunken)
        .border_1()
        .border_color(t.border_subtle)
}

fn segment(
    id: impl Into<gpui_kit::ElementId>,
    label: &'static str,
    selected: bool,
) -> AuroraButton {
    AuroraButton::new(id, label)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Small)
        .selected(selected)
}

/// A centred, quiet message that fills the card body.
fn empty_note(t: &theme::AuroraTheme, icon: &str, text: impl Into<gpui_kit::SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .child(crate::symbols::icon(
            Some(icon),
            px(22.),
            t.subtle_foreground,
        ))
        .child(
            div()
                .max_w(px(300.))
                .text_size(px(ui::TEXT_CAPTION))
                .text_color(t.muted_foreground)
                .text_center()
                .child(text.into()),
        )
}

/// One row of a card's list: hairline between rows, none after the last.
fn list_row(t: &theme::AuroraTheme, last: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .py(px(8.))
        .when(!last, |this| {
            this.border_b_1().border_color(t.border_subtle)
        })
}

fn row_title(t: &theme::AuroraTheme, text: String) -> Div {
    div()
        .min_w(px(0.))
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .text_size(px(ui::TEXT_BODY))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.foreground)
        .child(text)
}

fn row_detail(t: &theme::AuroraTheme, text: String) -> Div {
    div()
        .min_w(px(0.))
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .text_size(px(ui::TEXT_CAPTION))
        .text_color(t.muted_foreground)
        .child(text)
}

/// A server or world picture, or a quiet icon tile when it has none.
fn avatar(t: &theme::AuroraTheme, picture: Option<Arc<RenderImage>>, icon: &str) -> Div {
    match picture {
        Some(picture) => div()
            .flex_shrink_0()
            .size(px(32.))
            .rounded(px(8.))
            .overflow_hidden()
            .border_1()
            .border_color(t.border_subtle)
            .child(img(picture).size(px(30.))),
        None => ui::icon_tile(t, Some(icon), 32., false),
    }
}

/// An icon-only quiet action. `accent` tints the glyph (set favorites).
fn icon_button(
    t: &theme::AuroraTheme,
    id: String,
    icon: &'static str,
    label: String,
    disabled: bool,
    accent: bool,
) -> Button {
    let hover = t.control;
    Button::new(id)
        .aria_label(label)
        .disabled(disabled)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded(t.radius)
        .hover(move |style| style.bg(hover))
        .styles(|styles| styles.disabled(|style| style.opacity(0.42)))
        .child(crate::symbols::icon(
            Some(icon),
            px(18.),
            if accent {
                t.warning
            } else {
                t.muted_foreground
            },
        ))
}

/// The favorite toggle: an amber star when set, a quiet one when not.
fn star_button(
    t: &theme::AuroraTheme,
    id: String,
    favorite: bool,
    label: String,
    disabled: bool,
) -> Button {
    icon_button(t, id, "star", label, disabled, favorite)
}

// ---------------------------------------------------------------------------
// Tests (deterministic, offline; pure state-machine fixtures)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(id: &str, enabled: bool) -> WidgetPlacement {
        WidgetPlacement {
            id: id.into(),
            enabled,
            size: "small".into(),
            grid: None,
        }
    }

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

    fn daily(day: u64, duration_ms: u64) -> api::DailyPlaytime {
        api::DailyPlaytime { day, duration_ms }
    }

    /// (id, Minecraft version, selected?) instance summaries.
    fn launcher_state(selected: Option<&str>, instances: &[(&str, &str)]) -> api::LauncherState {
        serde_json::from_value(serde_json::json!({
            "config": {"selectedInstanceId": selected},
            "instances": instances.iter().map(|(id, version)| {
                serde_json::json!({
                    "id": id, "displayName": format!("{id} name"), "state": "ready",
                    "aurora": null, "platform": {"kind": "vanilla"},
                    "minecraftVersion": version,
                    "configuration": {
                        "minecraftVersion": version, "auroraEnabled": false, "loader": null,
                        "memoryMib": 4096, "additionalJvmArguments": "", "window": null
                    }
                })
            }).collect::<Vec<_>>()
        }))
        .unwrap()
    }

    fn empty_results() -> HistoryResults {
        HistoryResults {
            playtime: Ok(api::Playtime::default()),
            daily: Ok(Vec::new()),
            servers: Ok(Vec::new()),
            worlds: Ok(Vec::new()),
            saved_servers: None,
            saved_worlds: None,
        }
    }

    fn core_with_default_layout() -> Core {
        let mut core = Core::default();
        core.receive_layout(&Ok(default_layout()));
        core
    }

    fn validate_for_core(layout: &HomeLayout) {
        let saved: aurora_launcher_lib::home_widgets::HomeLayout =
            serde_json::from_value(serde_json::to_value(layout).unwrap()).unwrap();
        saved.validate().unwrap();
    }

    // -- ported projection tests ---------------------------------------------

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
        assert_eq!(server_played(0, now), "Not played yet");
    }

    #[test]
    fn an_untouched_layout_shows_only_playtime_and_servers() {
        let core = core_with_default_layout();
        let layout = core.layout.clone().unwrap();
        let shown: Vec<_> = layout
            .widgets
            .iter()
            .filter(|w| w.enabled && is_native(&w.id))
            .map(|w| w.id.as_str())
            .collect();
        assert_eq!(shown, ["playtime", "recent-servers"]);
        // Other hosts' entries survive untouched.
        assert!(layout.widgets.iter().any(|w| w.id == "skin-manager"));
        assert!(layout.widgets.iter().any(|w| w.id == "media" && !w.enabled));
    }

    #[test]
    fn media_runs_only_with_a_host_an_active_page_and_a_shown_card() {
        for has_host in [false, true] {
            for page_active in [false, true] {
                for card_shown in [false, true] {
                    assert_eq!(
                        media_should_run(has_host, page_active, card_shown),
                        has_host && page_active && card_shown,
                        "host={has_host} active={page_active} shown={card_shown}"
                    );
                }
            }
        }
        // The one running combination, and the diagnostic gate even when the
        // page is active and the card is enabled.
        assert!(media_should_run(true, true, true));
        assert!(!media_should_run(false, true, true));
    }

    #[test]
    fn media_follows_the_committed_layout_and_page_activity() {
        // No layout loaded yet: nothing is shown, so nothing runs.
        let mut core = Core::default();
        assert!(!core.media_card_shown());
        assert!(!core.media_should_run(true));

        // The untouched Home keeps Media off.
        core.receive_layout(&Ok(default_layout()));
        assert!(!core.media_should_run(true));

        // Enabling it starts; hiding the page or having no host stops.
        core.edit(|layout| set_enabled(layout, "media", true));
        assert!(core.media_should_run(true));
        assert!(!core.media_should_run(false));
        core.active = false;
        assert!(!core.media_should_run(true));
        core.active = true;
        assert!(core.media_should_run(true));

        // Removing the card stops it; adding it back resumes it.
        core.edit(|layout| set_enabled(layout, "media", false));
        assert!(!core.media_should_run(true));
        core.edit(|layout| set_enabled(layout, "media", true));
        assert!(core.media_should_run(true));

        // Reset restores the default Home, which has no Media card.
        core.edit(reset_layout);
        assert!(!core.media_card_shown());
        assert!(!core.media_should_run(true));

        // A failed layout read falls back to an empty layout: stopped, even
        // though the saved layout may still list Media.
        core.edit(|layout| set_enabled(layout, "media", true));
        assert!(core.media_should_run(true));
        core.receive_layout(&Err(api::ApiError::new("read_failed", "unreadable")));
        assert!(!core.media_should_run(true));
    }

    #[test]
    fn gesture_previews_never_change_the_media_decision() {
        let mut core = core_with_default_layout();
        core.edit(|layout| set_enabled(layout, "media", true));
        let before = core.media_should_run(true);
        assert!(before);

        // A moving preview, a resize preview and a cancel all leave the
        // committed layout (and therefore the decision) untouched.
        core.begin_gesture("media", false, (0., 0.));
        core.snap_gesture((328., 210.), (164., 210.));
        assert_eq!(core.media_should_run(true), before);
        core.cancel_gesture();
        assert_eq!(core.media_should_run(true), before);
        core.begin_gesture("media", true, (0., 0.));
        core.snap_gesture((0., 210.), (164., 210.));
        assert_eq!(core.media_should_run(true), before);

        // A committed gesture only rearranges the card; it stays shown.
        assert!(matches!(core.commit_gesture(), EditOutcome::Applied { .. }));
        assert!(core.media_card_shown());
        assert_eq!(core.media_should_run(true), before);

        // Hiding the page abandons a gesture and stops Media with it.
        core.begin_gesture("media", false, (0., 0.));
        core.active = false;
        core.cancel_gesture();
        assert!(!core.media_should_run(true));
    }

    #[test]
    fn customized_layouts_are_respected_and_reorderable() {
        let mut core = core_with_default_layout();
        core.edit(|layout| {
            set_enabled(layout, "media", true);
            set_enabled(layout, "playtime", false);
            set_enabled(layout, "recent-servers", false);
        });
        core.edit(|layout| move_widget(layout, "media", -1));
        let layout = core.layout.clone().unwrap();
        let order: Vec<_> = layout
            .widgets
            .iter()
            .filter(|w| w.enabled && is_native(&w.id))
            .map(|w| w.id.as_str())
            .collect();
        assert_eq!(order, ["media"]);
        core.edit(|layout| place_widget(layout, "playtime", 0));
        core.edit(|layout| set_enabled(layout, "playtime", true));
        let layout = core.layout.clone().unwrap();
        let order: Vec<_> = layout
            .widgets
            .iter()
            .filter(|w| w.enabled && is_native(&w.id))
            .map(|w| w.id.as_str())
            .collect();
        assert_eq!(order, ["media", "playtime"]);
        // Unknown entries survive every edit and the shape stays core-valid.
        assert!(layout.widgets.iter().any(|w| w.id == "skin-manager"));
        validate_for_core(&layout);
    }

    // -- responsive grid and gestures ------------------------------------------

    #[test]
    fn grid_positions_never_overlap_across_widths_and_never_write() {
        let mut core = core_with_default_layout();
        core.edit(|layout| set_enabled(layout, "media", true));
        core.edit(|layout| {
            layout.widgets[0].grid = Some(WidgetGrid {
                columns: 5,
                rows: 3,
                position: None,
            })
        });
        let saved = core.layout.clone().unwrap();
        for columns in 1..=6 {
            let positions = grid_positions(&saved, columns);
            let mut cells = HashSet::new();
            for (col, row, width, height) in positions {
                assert!(col + width <= columns);
                for y in row..row + height {
                    for x in col..col + width {
                        assert!(cells.insert((x, y)), "overlapping widgets");
                    }
                }
            }
        }
        // Viewport clamping is presentational only.
        assert_eq!(core.layout.clone().unwrap(), saved);
        assert_eq!(responsive_columns(0.), 1);
        assert_eq!(responsive_columns(163.), 1);
        assert_eq!(responsive_columns(164.), 1);
        assert_eq!(responsive_columns(805.), 4);
        assert_eq!(responsive_columns(806.), 5);
        assert_eq!(responsive_columns(969.), 5);
        assert_eq!(responsive_columns(970.), 6);
        assert_eq!(responsive_columns(10_000.), 6);
    }

    #[test]
    fn viewport_change_cancels_gesture_without_rewriting_saved_geometry() {
        let mut core = core_with_default_layout();
        core.set_columns(6);
        let saved = core.layout.clone();
        core.begin_gesture("playtime", true, (0., 0.));
        core.snap_gesture((164., 210.), (164., 210.));
        assert!(core.set_columns(2));
        assert_eq!(core.commit_gesture(), EditOutcome::Ignored);
        assert_eq!(core.layout, saved);
        assert!(core.begin_save().is_none());
    }

    #[test]
    fn dragging_displaces_collisions_and_preserves_unrecognized_widgets() {
        let mut layout = effective(HomeLayout {
            widgets: vec![
                placement("playtime", true),
                placement("media", true),
                placement("future-widget", true),
            ],
        });
        let unknown = layout.widgets[2].clone();
        arrange(&mut layout, "media", 0, 0, 3, 2, 6);
        let positions = grid_positions(&layout, 6);
        assert_eq!(positions[1], (0, 0, 3, 2));
        assert_eq!(layout.widgets[2], unknown);
        let mut cells = HashSet::new();
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
        validate_for_core(&layout);
        // One atomic commit: extreme input coordinates snap to the bounds.
        arrange(&mut layout, "media", 99, 99, 99, 99, 6);
        assert_eq!(grid_positions(&layout, 6)[1], (0, 63, 6, 3));
        validate_for_core(&layout);
    }

    #[test]
    fn gesture_snap_clamps_and_cancelled_previews_never_write() {
        let mut core = core_with_default_layout();
        core.columns = 6;
        core.begin_gesture("playtime", false, (0., 0.));
        core.snap_gesture((328., 0.), (164., 210.));
        let gesture = core.gesture.clone().unwrap();
        assert_eq!((gesture.preview.0, gesture.preview.1), (2, 0));
        // Clamped at the viewport edge.
        core.snap_gesture((10_000., 10_000.), (164., 210.));
        let gesture = core.gesture.clone().unwrap();
        assert_eq!(gesture.preview, (4, 48, 2, 1));
        // Escape cancels: the preview never writes.
        core.cancel_gesture();
        assert_eq!(core.commit_gesture(), EditOutcome::Ignored);
        assert!(core.begin_save().is_none());
        // A drag that returns to its origin commits nothing.
        core.begin_gesture("playtime", false, (0., 0.));
        core.snap_gesture((0., 0.), (164., 210.));
        assert_eq!(core.commit_gesture(), EditOutcome::Ignored);
        assert!(core.begin_save().is_none());
        // A real move commits exactly one gesture.
        core.begin_gesture("playtime", false, (0., 0.));
        core.snap_gesture((0., 210.), (164., 210.));
        assert_eq!(
            core.commit_gesture(),
            EditOutcome::Applied { reload: false }
        );
        assert_eq!(core.placement("playtime"), Some((0, 1, 2, 1)));
        let submitted = core.begin_save().unwrap();
        assert_eq!(
            core.finish_save(&submitted, &Ok(submitted.clone())),
            SaveOutcome::Acknowledged
        );
        assert!(core.begin_save().is_none());
    }

    #[test]
    fn resize_gestures_keep_requested_dimensions_for_moves() {
        let mut core = core_with_default_layout();
        core.columns = 6;
        core.begin_gesture("playtime", true, (0., 0.));
        core.snap_gesture((328., 210.), (164., 210.));
        assert_eq!(
            core.commit_gesture(),
            EditOutcome::Applied { reload: false }
        );
        assert_eq!(core.placement("playtime"), Some((0, 0, 4, 2)));
        let layout = core.layout.clone().unwrap();
        assert_eq!(
            layout.widgets[0].grid,
            Some(WidgetGrid {
                columns: 4,
                rows: 2,
                position: Some([0, 0])
            })
        );
        // A later pure move preserves the resized geometry.
        core.begin_gesture("playtime", false, (0., 0.));
        core.snap_gesture((164., 0.), (164., 210.));
        assert_eq!(
            core.commit_gesture(),
            EditOutcome::Applied { reload: false }
        );
        assert_eq!(core.placement("playtime"), Some((1, 0, 4, 2)));
    }

    #[test]
    fn keyboard_equivalents_apply_exactly_one_completed_gesture_each() {
        let mut core = core_with_default_layout();
        core.columns = 6;
        assert_eq!(
            core.keyboard_arrange("playtime", "down", false),
            EditOutcome::Applied { reload: false }
        );
        assert_eq!(core.placement("playtime"), Some((0, 1, 2, 1)));
        let first = core.begin_save().unwrap();
        assert_eq!(
            core.finish_save(&first, &Ok(first.clone())),
            SaveOutcome::Acknowledged
        );
        assert_eq!(
            core.keyboard_arrange("playtime", "right", false),
            EditOutcome::Applied { reload: false }
        );
        assert_eq!(core.placement("playtime"), Some((1, 1, 2, 1)));
        // Shift on the move handle resizes instead.
        assert_eq!(
            core.keyboard_arrange("playtime", "right", true),
            EditOutcome::Applied { reload: false }
        );
        assert_eq!(core.placement("playtime"), Some((1, 1, 3, 1)));
        // Bounded at the viewport edge and row bound.
        for _ in 0..80 {
            core.keyboard_arrange("playtime", "down", false);
        }
        let (_, row, _, _) = core.placement("playtime").unwrap();
        assert_eq!(row, 63);
        assert_eq!(
            core.keyboard_arrange("playtime", "home", false),
            EditOutcome::Ignored
        );
    }

    // -- saves -------------------------------------------------------------------

    #[test]
    fn layout_saves_are_serialized_coalesced_and_retried_only_on_change() {
        let mut core = core_with_default_layout();
        // An untouched loaded layout never writes.
        assert!(core.begin_save().is_none());
        core.edit(|layout| set_enabled(layout, "media", true));
        let first = core.begin_save().unwrap();
        // Serialized: nothing else starts while the first write is in flight.
        assert!(core.begin_save().is_none());
        // A newer edit while saving coalesces into one follow-up snapshot.
        core.edit(|layout| set_enabled(layout, "playtime", false));
        assert_eq!(
            core.finish_save(&first, &Ok(first.clone())),
            SaveOutcome::Acknowledged
        );
        let second = core.begin_save().unwrap();
        assert_ne!(second, first);
        // A duplicate completion cannot undo the acknowledged state.
        assert_eq!(
            core.finish_save(&first, &Ok(first.clone())),
            SaveOutcome::Ignored
        );
        assert_eq!(
            core.finish_save(
                &second,
                &Err(api::ApiError::new("backend_unavailable", "offline"))
            ),
            SaveOutcome::Failed
        );
        assert!(core.save_error.as_deref().is_some_and(|e| e == "offline"));
        // The failed snapshot is not retried until the layout changes again.
        assert!(core.begin_save().is_none());
        core.edit(|layout| set_enabled(layout, "media", false));
        assert!(core.begin_save().is_some());
    }

    #[test]
    fn failed_or_malformed_layout_reads_never_fabricate_a_writable_default() {
        let mut core = Core::default();
        core.receive_layout(&Err(api::ApiError::new(
            "native_contract_mismatch",
            "Unexpected launcher response",
        )));
        let layout = core.layout.clone().unwrap();
        assert_eq!(layout.widgets.len(), NATIVE.len());
        assert!(layout.widgets.iter().all(|w| !w.enabled));
        assert!(core.layout_error.is_some());
        assert!(core.begin_save().is_none());
        // Explicit edits must not overwrite malformed/unknown persisted state.
        assert_eq!(
            core.edit(|layout| set_enabled(layout, "playtime", true)),
            EditOutcome::Ignored
        );
        assert!(core.begin_save().is_none());
        core.receive_layout(&Ok(HomeLayout {
            widgets: Vec::new(),
        }));
        core.edit(|layout| set_enabled(layout, "playtime", true));
        validate_for_core(&core.begin_save().unwrap());
    }

    // -- history reads and guards --------------------------------------------------

    #[test]
    fn stale_history_reads_are_discarded_by_serial_and_favorite_revision() {
        let mut core = core_with_default_layout();
        core.set_state(
            Some(launcher_state(Some("instance"), &[("instance", "1.21.11")])),
            1,
        );
        let stale = core.begin_data_refresh().unwrap();
        let fresh = core.begin_data_refresh().unwrap();
        let mut results = empty_results();
        results.servers = Ok(vec![server(1, 1, false), server(2, 1, false)]);
        // A reply for a superseded refresh changes nothing.
        assert!(core.receive_history(&stale, &results).is_none());
        assert!(core.servers.is_empty());
        // A read begun before an acknowledged favorite write cannot restore
        // the stale star state.
        core.servers = vec![server(1, 1, true)];
        core.favorite_revision = 1;
        let pre_write = HistoryRead {
            favorite_revision: 0,
            ..fresh.clone()
        };
        assert!(core.receive_history(&pre_write, &results).is_none());
        assert!(core.servers.iter().all(|t| t.favorite));
        // The current revision applies and starts bounded enrichment.
        let current = HistoryRead {
            favorite_revision: 1,
            ..fresh
        };
        let enrich = core.receive_history(&current, &results).unwrap();
        assert_eq!(enrich.len(), 2);
        assert!(core.server_enriching);
    }

    #[test]
    fn a_stale_playtime_reply_never_replaces_the_current_chart_data() {
        let mut core = core_with_default_layout();
        core.set_state(
            Some(launcher_state(Some("instance"), &[("instance", "1.21.11")])),
            1,
        );
        let stale = core.begin_data_refresh().unwrap();
        let fresh = core.begin_data_refresh().unwrap();
        let mut old = empty_results();
        old.daily = Ok(vec![daily(90, 111)]);
        let mut current = empty_results();
        current.daily = Ok(vec![daily(100, 222)]);
        // The newer reply lands first; the superseded one arrives late.
        core.receive_history(&fresh, &current);
        assert!(core.receive_history(&stale, &old).is_none());
        let data = core.playtime.as_ref().expect("fresh playtime applied");
        assert_eq!(
            data.history().iter().map(|d| d.duration_ms).sum::<u64>(),
            222
        );
        assert_eq!(data.today, 100);
    }

    #[test]
    fn favorite_waits_for_confirmation_rejects_duplicates_and_recovers() {
        let mut core = core_with_default_layout();
        core.servers = vec![server(1, 1, false), server(2, 1, true)];
        assert_eq!(core.begin_favorite("missing"), None);
        assert_eq!(core.begin_favorite("1"), Some(true));
        assert!(!core.servers[0].favorite);
        assert_eq!(core.begin_favorite("1"), None);
        assert_eq!(core.begin_favorite("2"), Some(false));
        core.finish_favorite("1", true, false);
        assert!(!core.servers[0].favorite);
        assert_eq!(core.favorite_revision, 0);
        assert_eq!(core.begin_favorite("1"), Some(true));
        core.finish_favorite("1", true, true);
        core.finish_favorite("2", false, true);
        assert!(core.servers[0].favorite);
        assert!(!core.servers[1].favorite);
        assert_eq!(core.favorite_revision, 2);
        // A duplicate completion cannot undo the confirmed state.
        core.finish_favorite("1", false, true);
        assert!(core.servers[0].favorite);
        assert_eq!(core.favorite_revision, 2);
    }

    #[test]
    fn outer_history_worker_failure_clears_loading_and_rejects_stale_errors() {
        let mut core = core_with_default_layout();
        let old = core.begin_data_refresh().unwrap();
        let current = core.begin_data_refresh().unwrap();
        let failed = Err(api::ApiError::new(
            "native_operation_failed",
            "Worker stopped",
        ));
        assert!(core.settle_history(&old, &failed).is_none());
        assert!(core.loading);
        assert!(core.history_error.is_none());
        assert!(core.settle_history(&current, &failed).is_none());
        assert!(!core.loading);
        assert_eq!(core.history_error.as_deref(), Some("Worker stopped"));
        assert!(core.playtime.is_none());
        assert!(core.servers.is_empty());
        let retry = core.begin_data_refresh().unwrap();
        core.settle_history(&retry, &Ok(Box::new(empty_results())));
        assert!(core.history_error.is_none());
    }

    #[test]
    fn history_refresh_retains_committed_saved_favorite_until_terminal_reply() {
        let mut core = core_with_default_layout();
        core.saved_favorite_in_flight = Some(("instance:server".into(), true));
        core.begin_data_refresh();
        assert!(core.is_busy());
        assert_eq!(
            core.saved_favorite_in_flight,
            Some(("instance:server".into(), true))
        );
    }

    #[test]
    fn acknowledged_instance_version_change_invalidates_inflight_saved_rows() {
        let mut core = core_with_default_layout();
        core.set_state(
            Some(launcher_state(Some("instance"), &[("instance", "1.21.11")])),
            1,
        );
        let old = core.begin_data_refresh().unwrap();
        assert!(core.set_state(
            Some(launcher_state(Some("instance"), &[("instance", "1.20.1")])),
            2
        ));
        assert!(core.receive_history(&old, &empty_results()).is_none());
        assert!(!core.instance_map()["instance"].1);
        assert!(!core.set_state(
            Some(launcher_state(Some("instance"), &[("instance", "1.21.11")])),
            1
        ));
        assert!(!core.instance_map()["instance"].1);
    }

    #[test]
    fn terminal_worker_failure_is_reported_without_fabricating_data() {
        let mut core = core_with_default_layout();
        let read = core.begin_data_refresh().unwrap();
        let mut failed = empty_results();
        failed.playtime = Err(api::ApiError::new(
            "backend_unavailable",
            "The launcher worker did not complete.",
        ));
        failed.daily = Err(api::ApiError::new("backend_unavailable", "worker died"));
        failed.servers = Err(api::ApiError::new("backend_unavailable", "worker died"));
        failed.worlds = Err(api::ApiError::new("backend_unavailable", "worker died"));
        assert!(core.receive_history(&read, &failed).is_none());
        assert!(!core.loading);
        assert!(core.server_error);
        let view = playtime_view(core.playtime.as_ref(), Period::Week, false, false);
        assert_eq!(
            view.status.as_deref(),
            Some("Daily playtime is unavailable.")
        );
        assert_eq!(view.total, "Total unavailable");
        assert!(view.points.is_empty());
        assert!(core.worlds.is_empty());
        // Enrichment failure clears presentations honestly, without icons.
        core.receive_enrich(
            read.serial,
            &Err(api::ApiError::new("backend_unavailable", "enrichment died")),
        );
        assert!(!core.server_enriching);
        assert!(core.presentations.is_empty());
        assert!(core.server_pictures.is_empty());
        // A stale status reply must not settle a newer refresh.
        core.refreshing_servers = true;
        core.status_serial = 7;
        let stale = StatusRead {
            serial: 6,
            ids: vec!["1".into()],
            saved_instance: None,
        };
        core.receive_status(&stale, &Ok(Vec::new()));
        assert!(core.refreshing_servers);
        assert!(core.server_status.is_empty());
        core.receive_status(&StatusRead { serial: 7, ..stale }, &Ok(Vec::new()));
        assert!(!core.refreshing_servers);
    }

    // -- Quick Play identity ----------------------------------------------------

    #[test]
    fn quick_play_uses_exact_row_identity_frozen_instance_and_known_modes() {
        let mut core = core_with_default_layout();
        core.set_state(
            Some(launcher_state(
                Some("selected-instance"),
                &[
                    ("original-instance", "1.21.11"),
                    ("selected-instance", "1.21.11"),
                    ("old-game", "1.20.1"),
                ],
            )),
            1,
        );
        let mut target = server(1, 1, false);
        target.instance_id = "original-instance".into();
        let mut unavailable = server(2, 1, false);
        unavailable.available = false;
        let mut old = server(3, 1, false);
        old.instance_id = "missing-instance".into();
        let servers = vec![target, unavailable, old];
        core.servers = servers.clone();
        // The stored target's own instance is used, never the selected one.
        assert!(core.can_quick_play("server", "1", ""));
        assert!(core.can_quick_play("server", "1", "original-instance"));
        assert!(!core.can_quick_play("server", "1", "selected-instance"));
        assert!(!core.can_quick_play("server", "2", ""));
        assert!(!core.can_quick_play("server", "3", ""));
        assert!(!core.can_quick_play("server", "missing", ""));
        // Unknown modes are rejected outright.
        assert!(!core.can_quick_play("realms", "1", ""));
        // Worlds need the compatibility rule. (The history reply carries no
        // servers, so the exact server rows are restored afterwards.)
        let read = core.begin_data_refresh().unwrap();
        let mut results = empty_results();
        results.worlds = Ok(vec![
            api::RecentTarget {
                id: "w1".into(),
                instance_id: "selected-instance".into(),
                display_name: "New world".into(),
                last_played_at: 100,
                duration_ms: 0,
                available: true,
                favorite: false,
            },
            api::RecentTarget {
                id: "w2".into(),
                instance_id: "old-game".into(),
                display_name: "Old world".into(),
                last_played_at: 100,
                duration_ms: 0,
                available: true,
                favorite: false,
            },
        ]);
        core.receive_history(&read, &results);
        core.servers = servers;
        assert!(core.can_quick_play("world", "w1", "selected-instance"));
        assert!(!core.can_quick_play("world", "w2", "old-game"));
        // Saved destinations validate their frozen instance on activation.
        core.saved_worlds_rows = vec![Row {
            id: "sw1".into(),
            instance: "selected-instance".into(),
            label: "Saved world".into(),
            identity: "SavedWorld".into(),
            detail: String::new(),
            last_played_at: 0,
            playable: true,
            favorite: false,
            unvisited_favorite: false,
            motd: String::new(),
            picture: None,
        }];
        assert!(core.can_quick_play("saved-world", "sw1", "selected-instance"));
        // The frozen instance vanished: the row is not playable anymore.
        core.set_state(
            Some(launcher_state(
                Some("original-instance"),
                &[("original-instance", "1.21.11")],
            )),
            2,
        );
        assert!(!core.can_quick_play("saved-world", "sw1", "selected-instance"));
        assert!(core.can_quick_play("server", "1", "original-instance"));
    }

    // -- row mapping ---------------------------------------------------------------

    #[test]
    fn world_and_saved_rows_map_honestly() {
        let mut core = core_with_default_layout();
        core.set_state(Some(launcher_state(Some("i1"), &[("i1", "1.21.11")])), 1);
        let read = core.begin_data_refresh().unwrap();
        assert_eq!(read.instance.as_deref(), Some("i1"));
        let mut results = empty_results();
        results.worlds = Ok(vec![
            api::RecentTarget {
                id: "fresh".into(),
                instance_id: "i1".into(),
                display_name: "Fresh".into(),
                last_played_at: 86_400,
                duration_ms: 0,
                available: true,
                favorite: false,
            },
            api::RecentTarget {
                id: "pinned".into(),
                instance_id: "i1".into(),
                display_name: "Pinned".into(),
                last_played_at: 0,
                duration_ms: 0,
                available: true,
                favorite: true,
            },
        ]);
        results.saved_worlds = Some(Ok(vec![api::SavedDestination {
            id: "saved:world".into(),
            display_name: "Kept".into(),
            identity: "KeptWorld".into(),
            last_played_ms: 172_800_000,
            detail: String::new(),
            icon: None,
            favorite: false,
        }]));
        core.receive_history(&read, &results);
        let fresh = core.worlds.iter().find(|r| r.id == "fresh").unwrap();
        assert_eq!(fresh.detail, "i1 name");
        assert!(fresh.playable);
        assert_eq!(
            world_value(fresh.last_played_at, fresh.last_played_at),
            "Today"
        );
        let pinned = core.worlds.iter().find(|r| r.id == "pinned").unwrap();
        assert!(pinned.unvisited_favorite);
        assert_eq!(world_value(0, 1_800_000_000), "Favorite");
        let saved = core.saved_worlds_rows.first().unwrap();
        assert_eq!(saved.instance, "i1");
        assert_eq!(saved.detail, "i1 name");
        assert!(saved.playable);
        assert_eq!(
            saved_value(
                &saved.identity,
                172_800_000,
                true,
                172_800_000 / 1000 + 86_400
            ),
            "KeptWorld · Yesterday"
        );
        // Missing daily history stays unavailable, never zero activity.
        let view = playtime_view(core.playtime.as_ref(), Period::Month, false, false);
        assert_eq!(view.status.as_deref(), Some("No daily history available."));
        assert_eq!(view.total, "Total unavailable");
    }

    // -- ported playtime projection ----------------------------------------------

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
        let data = PlaytimeData {
            summary: None,
            daily: Some(history.clone()),
            today,
        };
        let view = playtime_view(Some(&data), Period::Week, false, false);
        assert_eq!(view.points.last().unwrap().label, "Today");
        // The fixture units are milliseconds: 24 ms renders as under a minute.
        assert_eq!(view.total, duration(playtime_total(&week)));
        assert_eq!(
            playtime_total(&playtime_bins(&history, (at + 1) / 86_400, 7, 7)),
            48
        );
    }

    #[test]
    fn playtime_missing_zero_and_empty_days_stay_honest() {
        let bins = playtime_bins(&[daily(99, 0), daily(100, 300_000)], 100, 7, 7);
        assert_eq!(bins.len(), 7);
        assert!(bins[..6].iter().all(|b| b.duration_ms == 0));
        assert!(playtime_bins(&[], 100, 30, 7).is_empty());
        let data = PlaytimeData {
            summary: Some(api::Playtime {
                all_time_ms: 60_000,
                ..Default::default()
            }),
            daily: Some(vec![daily(99, 0), daily(100, 300_000)]),
            today: 100,
        };
        let view = playtime_view(Some(&data), Period::Week, false, false);
        assert_eq!(view.points.len(), 7);
        assert!(view.points[..6].iter().all(|p| p.fraction == 0.0));
        assert_eq!(view.points[6].fraction, 1.0);
        assert!(view.points[6].detail.contains("300000 ms"));
        assert!(!view.empty_period);
        // A week with only zero days reports an empty period, never fabricated.
        let quiet = PlaytimeData {
            summary: Some(api::Playtime::default()),
            daily: Some((80..=90).map(|day| daily(day, 0)).collect()),
            today: 100,
        };
        let view = playtime_view(Some(&quiet), Period::Week, false, false);
        assert!(view.empty_period);
        assert!(view.points.iter().all(|p| p.fraction == 0.0));
        assert_eq!(axis_duration((4 * 60 + 52) * 60_000), "4 h 52 min");
        assert_eq!(axis_duration(0), "0 min");
        assert!(axis_duration(u64::MAX).len() <= 10);
    }

    #[test]
    fn playtime_narrow_cards_sum_the_month_into_seven_bins() {
        let history: Vec<_> = (70..=100)
            .map(|day| daily(day, (day - 70) * 60_000))
            .collect();
        let data = PlaytimeData {
            summary: None,
            daily: Some(history),
            today: 100,
        };
        let wide = playtime_view(Some(&data), Period::Month, false, false);
        let narrow = playtime_view(Some(&data), Period::Month, true, false);
        assert_eq!(wide.points.len(), 30);
        assert_eq!(narrow.points.len(), 7);
        let bins = playtime_bins(data.history(), 100, 30, 7);
        assert_eq!(bins.first().unwrap().first, 71);
        assert_eq!(bins.last().unwrap().last, 100);
        assert!(
            bins.windows(2)
                .all(|pair| pair[0].last + 1 == pair[1].first)
        );
        assert_eq!(
            playtime_total(&bins),
            playtime_total(&playtime_bins(data.history(), 100, 30, 30))
        );
        assert_eq!(
            narrow.points.first().unwrap().label,
            "29 days ago – 26 days ago"
        );
        assert_eq!(narrow.points.last().unwrap().label, "4 days ago – Today");
    }

    #[test]
    fn playtime_lifetime_uses_real_summary_without_extending_daily_history() {
        let summary = api::Playtime {
            all_time_ms: 360_000_000,
            ..Default::default()
        };
        let data = PlaytimeData {
            summary: Some(summary),
            daily: Some(vec![daily(100, 60_000)]),
            today: 100,
        };
        let view = playtime_view(Some(&data), Period::AllTime, false, false);
        assert_eq!(view.total, "100 h");
        assert!(view.total_detail.contains("last 30 UTC days"));
        let missing = PlaytimeData {
            summary: None,
            daily: None,
            today: 100,
        };
        let view = playtime_view(Some(&missing), Period::AllTime, false, false);
        assert_eq!(view.total, "Total unavailable");
        assert_eq!(
            view.status.as_deref(),
            Some("Daily playtime is unavailable.")
        );
        let none = playtime_view(None, Period::Week, false, false);
        assert_eq!(view.status, none.status);
    }

    #[test]
    fn playtime_chart_chrome_follows_the_slint_plot() {
        // An empty day keeps a 2 px stub; a full day leaves 4 px of headroom.
        assert_eq!(bar_height(0., 96.), 2.);
        assert_eq!(bar_height(1., 96.), 92.);
        assert_eq!(bar_height(0.5, 96.), 46.);
        // A sliver never vanishes below the stub.
        assert_eq!(bar_height(0.001, 96.), 2.);
        assert_eq!(chart_note(Period::Week, false), "Chart: last 7 days · UTC");
        // A narrow week is already seven daily bars; only longer ranges sum.
        assert_eq!(chart_note(Period::Week, true), "Chart: last 7 days · UTC");
        assert_eq!(
            chart_note(Period::Month, false),
            "Chart: last 30 days · UTC"
        );
        assert_eq!(
            chart_note(Period::Month, true),
            "Chart: last 30 days · UTC · 7 summed bins"
        );
        assert_eq!(
            chart_note_description(Period::AllTime, false),
            // Slint concatenates its sentences verbatim, doubled full stop included.
            "Chart: last 30 days · UTC. Total covers all time; earlier daily history is not shown.. Daily bars."
        );
        assert_eq!(
            chart_note_description(Period::Month, true),
            "Chart: last 30 days · UTC · 7 summed bins. Seven summed date bins."
        );
        // The peak stays in the gutter, never in the note.
        assert!(!chart_note(Period::Month, false).contains("Peak"));
    }

    // -- ported server projection -----------------------------------------------

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
        assert!(server_entries(&[]).is_empty());
        let ids: Vec<String> = bounded
            .iter()
            .take(ENRICH_LIMIT)
            .map(|t| t.id.clone())
            .collect();
        assert_eq!(ids.len(), ENRICH_LIMIT);
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
        assert_eq!(server_played((now - 86_400) * 1000, now), "Yesterday");
    }

    #[test]
    fn server_presentation_retains_two_trimmed_motd_lines() {
        let presentation: api::ServerPresentation = serde_json::from_value(serde_json::json!({
            "targetId": "opaque", "name": "Saved server", "favicon": null,
            "motd": [[{"text":"      PVPHQ "}, {"text":"| discord.gg/pvphq    "}],
                     [{"text":"          "}],
                     [{"text":"   Actual second line   "}],
                     [{"text":"Third line"}]]
        }))
        .unwrap();
        assert_eq!(
            server_motd(&presentation),
            "PVPHQ | discord.gg/pvphq\nActual second line"
        );
        assert!(presentation.favicon.is_none());
    }
}
