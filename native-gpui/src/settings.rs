//! Launcher Settings for the GPUI host: appearance (theme, accent presets,
//! custom accent, background), Discord Rich Presence preferences, desktop
//! shortcuts and the launcher folder, and update status.
//!
//! The preserved Slint host (`native/src/settings.rs`) remains the reference
//! for behavior. This module keeps its contract: appearance writes always
//! carry the latest complete snapshot, one write is in flight at a time with
//! newer changes coalesced into a pending request, stale or duplicate
//! completions are ignored, a failed write never overwrites the acknowledged
//! state, and malformed persisted state is never overwritten.
//!
//! Appearance controls, ported from the Slint host:
//! - Background (Simple / Borealis) is a dropdown disabled while a write is
//!   in flight. The Borealis layer itself lives in [`borealis`]; the root
//!   owns that view and follows [`SettingsEvent`].
//! - Motion speed is a 0 to 100 slider. Dragging previews the speed live
//!   ([`SettingsEvent::Motion`]); releasing it (or any keyboard or assistive
//!   step) writes the complete snapshot; leaving the page discards an
//!   unreleased draft ([`Settings::cancel_previews`]), restoring the queued
//!   or acknowledged value. An acknowledged write never overwrites an
//!   unreleased draft. Reduce motion shares the same effective value and
//!   writes 0 or 50 exactly like the Slint host.
//! - The custom accent picker is the Slint HSV picker (saturation/value
//!   square, hue strip, hex field, preview swatch, Cancel/Apply). It
//!   previews the accent live and writes `#rrggbb` only through the core's
//!   validated custom-accent derivation.
//!
//! Honest parity limits, kept deliberate:
//! - Launcher updates use the shared signed download and installer handoff.
//!   No updater trust or installation policy lives in the renderer.
//! - The picker is an inline panel under the Accent row, not a window-level
//!   popover with outside-click dismissal; Escape and Cancel dismiss it.

#![allow(dead_code)]

use std::cell::Cell;
use std::rc::Rc;

use aurora_launcher_lib::host::Host;
use gpui_kit::base::ElementExt as _;
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::base::slider::{SliderEvent, SliderState};
use gpui_kit::{
    AppContext as _, Bounds, Context, Div, DragMoveEvent, Entity, EventEmitter, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    ParentElement as _, Pixels, Point, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled, Subscription, WeakEntity, Window, div, hsla, linear_color_stop, linear_gradient, px,
    relative, rgb,
};

use crate::api::{self, ApiResult};
use crate::backend::guarded;
use crate::borealis;
use crate::controls::{
    AuroraButton, AuroraInput, AuroraSelect, AuroraSlider, AuroraToggle, ButtonSize, ButtonVariant,
};
use crate::theme;
use crate::ui;

type BoundsCell = Rc<Cell<Option<gpui::Bounds<gpui::Pixels>>>>;

/// Emitted when the root must re-derive what it shows from Settings.
#[derive(Debug)]
pub enum SettingsEvent {
    /// An appearance read or save was acknowledged with no newer pending
    /// request (the saved background, theme and accent are authoritative).
    Changed,
    /// The accent preview of the custom picker changed or was discarded:
    /// re-read [`Settings::display_appearance`].
    Preview,
    /// The effective Motion speed changed: a live slider preview, a write
    /// being queued, a restored value, or an acknowledged one. Feed it to the
    /// Borealis view.
    Motion(u8),
    Installing(bool),
}

// ---------------------------------------------------------------------------
// Appearance snapshot queue (behavior ported from native/src/settings.rs)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct AppearanceRequest {
    pub revision: u64,
    pub theme: String,
    pub accent: serde_json::Value,
    pub background: String,
    pub motion: u8,
}

#[derive(Default, Debug)]
pub struct AppearanceQueue {
    revision: u64,
    in_flight: Option<AppearanceRequest>,
    pending: Option<AppearanceRequest>,
}

impl AppearanceQueue {
    /// Registers a request. The first one is returned for dispatch; later
    /// ones coalesce into a single pending snapshot.
    fn enqueue(&mut self, mut request: AppearanceRequest) -> Option<AppearanceRequest> {
        self.revision = self.revision.wrapping_add(1);
        request.revision = self.revision;
        if self.in_flight.is_some() {
            self.pending = Some(request);
            None
        } else {
            self.in_flight = Some(request.clone());
            Some(request)
        }
    }

    /// Completes the matching in-flight write. Stale or duplicate
    /// completions do nothing. A pending request takes over in flight
    /// immediately so later completions cannot observe a free queue.
    fn finish(&mut self, revision: u64) -> Option<(AppearanceRequest, Option<AppearanceRequest>)> {
        if self.in_flight.as_ref()?.revision != revision {
            return None;
        }
        let completed = self.in_flight.take()?;
        let pending = self.pending.take();
        self.in_flight = pending.clone();
        Some((completed, pending))
    }

    fn latest(&self) -> Option<AppearanceRequest> {
        self.pending.as_ref().or(self.in_flight.as_ref()).cloned()
    }
}

/// Outcome of an acknowledged appearance write for the presentation layer.
#[derive(Debug, PartialEq)]
pub enum Completion {
    /// A stale or duplicate completion was ignored.
    Ignored,
    /// No newer request existed: the response is the acknowledged UI state.
    Applied,
    /// A newer request was pending: the response is only the disk baseline.
    BaselineStored,
    /// The write failed. With no pending request the acknowledged state
    /// stands unchanged; a pending request still follows.
    Failed,
}

/// All appearance state as pure, testable logic. The presentation layer owns
/// the acknowledged snapshot and never rebuilds it from partial data.
#[derive(Default)]
pub struct AppearanceState {
    appearance: Option<api::Appearance>,
    error: Option<String>,
    busy: bool,
    queue: AppearanceQueue,
    /// An unreleased Motion slider draft. It wins over every other source of
    /// the effective speed, so an acknowledged response never overwrites it.
    motion_draft: Option<u8>,
}

/// Motion speeds are whole numbers from 0 to 100.
pub fn normalize_motion(value: f32) -> u8 {
    value.round().clamp(0.0, 100.0) as u8
}

/// Dropdown wording, in the order of [`borealis::BACKGROUNDS`].
pub const BACKGROUND_LABELS: [&str; 2] = ["Simple", "Borealis"];
pub const BACKGROUND_DESCRIPTIONS: [&str; 2] = [
    "Plain surfaces in the selected theme.",
    "Soft, stylized aurora bands behind translucent surfaces.",
];

/// The speed Reduce motion writes: on stops motion, off returns to the
/// default speed (the Slint host's `value ? 0 : 50`).
pub fn reduce_motion_speed(on: bool) -> u8 {
    if on { 0 } else { 50 }
}

/// The Motion row's description; a software renderer keeps Borealis static.
pub fn motion_detail(accelerated: bool) -> String {
    format!(
        "Borealis drift speed. Set to 0 for a still background.{}",
        if accelerated {
            ""
        } else {
            " Software rendering keeps Borealis static."
        }
    )
}

/// The edits behind every appearance write this page makes. Each starts from
/// the complete current snapshot, so the other fields are never rebuilt.
pub fn theme_edit(theme: String) -> impl FnOnce(&mut AppearanceRequest) {
    move |request| request.theme = theme
}

pub fn background_edit(background: &str) -> impl FnOnce(&mut AppearanceRequest) {
    let background = background.to_owned();
    move |request| request.background = background
}

pub fn motion_edit(motion: u8) -> impl FnOnce(&mut AppearanceRequest) {
    move |request| request.motion = motion
}

pub fn accent_edit(accent: serde_json::Value) -> impl FnOnce(&mut AppearanceRequest) {
    move |request| request.accent = accent
}

/// The validated custom accent selection the core accepts, lowercased.
pub fn custom_accent_value(hex: &str) -> serde_json::Value {
    serde_json::json!({ "type": "custom", "hex": hex.to_ascii_lowercase() })
}

impl AppearanceState {
    /// The request a write should carry: the coalesced queue when present,
    /// otherwise the latest acknowledged snapshot. Malformed state (no
    /// snapshot) yields no request, so a write can never overwrite it.
    pub fn request(&self) -> Option<AppearanceRequest> {
        self.queue.latest().or_else(|| {
            self.appearance
                .as_ref()
                .map(|appearance| AppearanceRequest {
                    revision: 0,
                    theme: appearance.theme.clone(),
                    accent: appearance.accent.clone(),
                    background: appearance.background.clone(),
                    motion: appearance.aurora_motion_speed,
                })
        })
    }

    /// The Motion speed the slider and Borealis should show: an unreleased
    /// draft, else the queued request, else the acknowledged snapshot. With
    /// no valid snapshot at all this is nothing, so the controls are hidden
    /// rather than fabricating a state from malformed or unavailable data.
    pub fn motion_speed(&self) -> Option<u8> {
        self.motion_draft
            .or_else(|| self.queue.latest().map(|request| request.motion))
            .or_else(|| {
                self.appearance
                    .as_ref()
                    .map(|appearance| appearance.aurora_motion_speed)
            })
    }

    /// Reduce motion follows the effective speed (Slint: `motion == 0`).
    pub fn reduce_motion(&self) -> Option<bool> {
        self.motion_speed().map(|speed| speed == 0)
    }

    /// A slider change before release: previews the speed without writing.
    pub fn preview_motion(&mut self, speed: u8) -> u8 {
        let speed = speed.min(100);
        self.motion_draft = Some(speed);
        speed
    }

    pub fn motion_preview_active(&self) -> bool {
        self.motion_draft.is_some()
    }

    /// The draft was released (or replaced by a toggle) and is now a write.
    pub fn end_motion_preview(&mut self) {
        self.motion_draft = None;
    }

    /// Navigation discards only an unreleased draft; a released request stays
    /// the latest value even while its serialized save is still running.
    /// Returns the restored effective speed when a draft was discarded.
    pub fn cancel_motion_preview(&mut self) -> Option<u8> {
        self.motion_draft.take()?;
        self.motion_speed()
    }

    /// Records a user change and returns the request to dispatch now, if any.
    pub fn submit(&mut self, mut request: AppearanceRequest) -> Option<AppearanceRequest> {
        request.revision = 0;
        let dispatch = self.queue.enqueue(request);
        self.busy = true;
        self.error = None;
        dispatch
    }

    /// Applies `edit` to the latest complete snapshot (queued, else
    /// acknowledged) and records it. `None` means there is no valid snapshot
    /// to write from; otherwise the inner value is the request to dispatch
    /// now, if the queue has no write in flight.
    pub fn stage(
        &mut self,
        edit: impl FnOnce(&mut AppearanceRequest),
    ) -> Option<Option<AppearanceRequest>> {
        let mut request = self.request()?;
        edit(&mut request);
        Some(self.submit(request))
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn appearance(&self) -> Option<&api::Appearance> {
        self.appearance.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Completes a write. A failed write without a pending request leaves the
    /// acknowledged snapshot untouched; the UI re-applies it instead of
    /// keeping an error-mixed draft.
    pub fn complete(
        &mut self,
        revision: u64,
        result: ApiResult<api::Appearance>,
    ) -> (Completion, Option<AppearanceRequest>) {
        let Some((_, pending)) = self.queue.finish(revision) else {
            return (Completion::Ignored, None);
        };
        match result {
            Ok(appearance) => {
                self.appearance = Some(appearance);
                if pending.is_some() {
                    (Completion::BaselineStored, pending)
                } else {
                    self.busy = false;
                    self.error = None;
                    (Completion::Applied, None)
                }
            }
            Err(error) => {
                self.error = Some(error.message);
                match pending {
                    Some(pending) => (Completion::Failed, Some(pending)),
                    None => {
                        self.busy = false;
                        (Completion::Failed, None)
                    }
                }
            }
        }
    }

    /// Records a read result (initial load or refresh). A malformed response
    /// never fabricates a snapshot: controls stay empty and disabled until a
    /// valid snapshot arrives.
    pub fn loaded(&mut self, result: ApiResult<api::Appearance>) {
        match result {
            Ok(appearance) => {
                self.appearance = Some(appearance);
                self.error = None;
            }
            Err(error) => {
                self.appearance = None;
                self.motion_draft = None;
                self.error = Some(error.message);
            }
        }
        if self.queue.in_flight.is_none() && self.queue.pending.is_none() {
            self.busy = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Pure helpers for host events and update text
// ---------------------------------------------------------------------------

/// Parses a host event into an update overview, or nothing when the payload
/// is malformed (unknown events are never fatal).
pub fn update_status_event(name: &str, payload: &serde_json::Value) -> Option<api::UpdateOverview> {
    if name != "update-status" {
        return None;
    }
    serde_json::from_value(payload.clone()).ok()
}

/// Download progress text for the signed Launcher installer.
pub fn launcher_progress_text(payload: &serde_json::Value) -> Option<String> {
    let done = payload.get("downloadedBytes")?.as_u64()?;
    Some(match payload.get("totalBytes").and_then(|t| t.as_u64()) {
        Some(total) => format!(
            "Downloading: {:.1} / {:.1} MiB",
            done as f64 / 1048576.0,
            total as f64 / 1048576.0
        ),
        None => format!("Downloading: {done} bytes"),
    })
}

fn launcher_offer(value: &serde_json::Value) -> Option<String> {
    (value.get("kind")?.as_str()? == "updateAvailable").then(|| {
        value
            .get("candidate")?
            .as_str()
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    })?
}

fn availability_text(value: &serde_json::Value) -> String {
    match value.get("kind").and_then(serde_json::Value::as_str) {
        Some("upToDate") => "Up to date.".into(),
        Some("updateAvailable") => format!(
            "Version {} is available.",
            value
                .get("candidate")
                .and_then(|c| c.as_str())
                .unwrap_or("?")
        ),
        Some("unavailable") => value
            .get("reason")
            .and_then(|r| r.as_str())
            .unwrap_or("Update information is unavailable.")
            .to_owned(),
        Some("notApplicable") => value
            .get("reason")
            .and_then(|r| r.as_str())
            .unwrap_or("Not applicable.")
            .to_owned(),
        Some("notChecked") | None => "Not checked yet.".into(),
        Some(other) => other.to_owned(),
    }
}

fn shortcut_text(state: &str, managed_by: &str) -> String {
    match (state, managed_by) {
        ("present", "installer") => "Created by the Aurora installer.".into(),
        ("present", _) => "Present.".into(),
        ("absent", _) => "Not created.".into(),
        ("conflict", _) => "Another shortcut with this name exists; Aurora leaves it alone.".into(),
        _ => "Unavailable right now.".into(),
    }
}

fn discord_status(state: &api::DiscordState) -> String {
    if !state.configured {
        return "This build has no Discord application ID, so presence cannot connect. Choices are saved; nothing is sent.".into();
    }
    match (state.preferences.enabled, state.connection.as_str()) {
        (false, _) => "Ready. Aurora's Discord application is configured. Turn on Rich Presence to share activity.".into(),
        (true, "connected") => "Connected to the Discord app on this computer.".into(),
        (true, "notDetected") => {
            "Discord is not running. Aurora checks again occasionally while this is on.".into()
        }
        (true, "failed") => "Discord could not be reached. Use Reconnect to try again.".into(),
        (true, "closed") => "Disconnected from Discord.".into(),
        (true, _) => "Waiting for the Discord app on this computer.".into(),
    }
}

/// `#rrggbb`, nothing else (surrounding whitespace is ignored).
pub(crate) fn parse_hex(text: &str) -> Option<(u8, u8, u8)> {
    let hex = text.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    Some(((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

fn parse_hex_color(text: &str) -> Option<Hsla> {
    parse_hex(text).map(|(r, g, b)| rgb_color(r, g, b))
}

fn rgb_color(r: u8, g: u8, b: u8) -> Hsla {
    rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)).into()
}

fn hex_text(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Hue (degrees), saturation and value (0 to 1) of an sRGB color.
pub(crate) fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    (hue, saturation, max)
}

pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let channel = |value: f32| ((value + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (channel(r), channel(g), channel(b))
}

/// The palette the core derives for a custom accent, or the reason it is not
/// usable (too dark to stay visible, malformed, ...). The same rules gate the
/// saved value, so a preview that passes here is accepted by the write.
pub(crate) fn custom_palette(hex: &str) -> Result<api::AccentPalette, String> {
    use aurora_launcher_lib::appearance::AccentSelection;
    AccentSelection::Custom {
        hex: hex.to_owned(),
    }
    .palette()
    .map(|palette| api::AccentPalette {
        accent: palette.base.as_hex(),
        accent_strong: palette.strong.as_hex(),
        accent_hover: palette.hover.as_hex(),
        accent_pressed: palette.pressed.as_hex(),
        accent_contrast: palette.on_accent.as_hex(),
        accent_soft: palette.soft_css(),
        accent_outline: palette.outline_css(),
    })
    .map_err(|error| error.reason)
}

/// Wording of the picker's status line.
pub const PICKER_PREVIEWING: &str = "Previewing. Aurora derives readable button colors from it.";
pub const PICKER_HEX_HINT: &str = "Enter a color as #rrggbb.";

/// The custom accent picker's value, ported from the Slint `AccentPicker`
/// global: hue (0 to 360), saturation and value (0 to 1), the hex field text,
/// and whether that color passed the core's validation.
#[derive(Clone, Debug, PartialEq)]
pub struct AccentPickerState {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    pub hex: String,
    pub valid: bool,
    pub message: String,
}

impl AccentPickerState {
    /// Opens on the saved accent (`start`), falling back to the default
    /// violet when it is not a usable `#rrggbb`.
    pub fn open(start: &str) -> Self {
        let (r, g, b) = parse_hex(start).unwrap_or((0x8b, 0x80, 0xff));
        let (hue, saturation, value) = rgb_to_hsv(r, g, b);
        let mut picker = Self {
            hue,
            saturation,
            value,
            hex: String::new(),
            valid: true,
            message: String::new(),
        };
        picker.sync_hex();
        picker
    }

    fn sync_hex(&mut self) {
        let (r, g, b) = hsv_to_rgb(self.hue, self.saturation, self.value);
        self.hex = hex_text(r, g, b);
    }

    pub fn set_hue(&mut self, hue: f32) {
        self.hue = hue.clamp(0.0, 360.0);
        self.sync_hex();
    }

    pub fn set_sv(&mut self, saturation: f32, value: f32) {
        self.saturation = saturation.clamp(0.0, 1.0);
        self.value = value.clamp(0.0, 1.0);
        self.sync_hex();
    }

    /// The hex field was edited. A valid `#rrggbb` moves the hue, saturation
    /// and value (the typed text stays as typed) and returns the normalized
    /// color to preview; anything else only flags the field.
    pub fn edit_hex(&mut self, text: &str) -> Option<String> {
        self.hex = text.to_owned();
        match parse_hex(text) {
            Some((r, g, b)) => {
                (self.hue, self.saturation, self.value) = rgb_to_hsv(r, g, b);
                Some(hex_text(r, g, b))
            }
            None => {
                self.valid = false;
                self.message = PICKER_HEX_HINT.into();
                None
            }
        }
    }

    /// The core's verdict on the previewed color.
    pub fn set_validity(&mut self, verdict: Result<(), &str>) {
        match verdict {
            Ok(()) => {
                self.valid = true;
                self.message = PICKER_PREVIEWING.into();
            }
            Err(reason) => {
                self.valid = false;
                self.message = format!("Not usable: {reason}.");
            }
        }
    }

    /// The color written by Apply: only a valid one, lowercased.
    pub fn apply(&self) -> Option<String> {
        self.valid.then(|| self.hex.trim().to_ascii_lowercase())
    }

    pub fn rgb(&self) -> (u8, u8, u8) {
        hsv_to_rgb(self.hue, self.saturation, self.value)
    }

    pub fn hue_rgb(&self) -> (u8, u8, u8) {
        hsv_to_rgb(self.hue, 1.0, 1.0)
    }
}

/// The fraction of `extent` that `offset` reaches, clamped to the track.
fn track_fraction(offset: f32, extent: f32) -> f32 {
    (offset / extent.max(1.0)).clamp(0.0, 1.0)
}

fn pointer_fractions(position: Point<Pixels>, bounds: Bounds<Pixels>) -> (f32, f32) {
    (
        track_fraction(
            f32::from(position.x - bounds.origin.x),
            f32::from(bounds.size.width),
        ),
        track_fraction(
            f32::from(position.y - bounds.origin.y),
            f32::from(bounds.size.height),
        ),
    )
}

/// Marker types for the picker's two drag gestures (distinct, because GPUI
/// routes drag moves by payload type).
struct SvDrag;
struct HueDrag;

/// The invisible drag preview GPUI requires for a drag payload.
struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

enum Message {
    AppearanceRead(ApiResult<api::Appearance>),
    AppearanceSaved(u64, ApiResult<api::Appearance>),
    DiscordRead(ApiResult<api::DiscordState>),
    DiscordSaved(u64, ApiResult<api::DiscordState>),
    DesktopRead(ApiResult<api::DesktopIntegration>),
    DesktopSaved(ApiResult<api::DesktopIntegration>),
    FolderOpened(ApiResult<()>),
    StatusRead(ApiResult<api::ApplicationStatus>),
    UpdatesChecked(ApiResult<api::UpdateOverview>),
    LauncherDownloaded(String, ApiResult<serde_json::Value>),
    LauncherInstalled(ApiResult<serde_json::Value>),
}
enum ShortcutOperation {
    CreateDesktop,
    RemoveDesktop,
    CreateStartMenu,
}

// ---------------------------------------------------------------------------
// Settings view
// ---------------------------------------------------------------------------

pub struct Settings {
    section: usize,
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    appearance: AppearanceState,
    appearance_read: u64,
    discord_read: u64,
    discord_event: u64,
    desktop_read: u64,
    discord: Option<api::DiscordState>,
    discord_busy: bool,
    discord_error: Option<String>,
    desktop: Option<api::DesktopIntegration>,
    shortcuts_busy: bool,
    desktop_error: Option<String>,
    data_root: Option<String>,
    updates: Option<api::UpdateOverview>,
    update_busy: bool,
    downloaded_version: Option<String>,
    install_blocked: bool,
    update_detail: Option<String>,
    updates_error: Option<String>,
    accent_hex: Entity<InputState>,
    /// The open custom accent picker, if any.
    picker: Option<AccentPickerState>,
    /// The last valid palette the picker previewed. It stays applied after
    /// Apply until the write is acknowledged (or fails and is restored), and
    /// is dropped by Cancel and by every acknowledged appearance.
    accent_preview: Option<api::AccentPalette>,
    sv_bounds: BoundsCell,
    hue_bounds: BoundsCell,
    sv_focus: FocusHandle,
    hue_focus: FocusHandle,
    motion_slider: Entity<SliderState>,
    /// A pointer is down on the slider: its changes are a draft until the
    /// release. Keyboard and assistive steps have no release, so they commit
    /// immediately, like the Slint slider.
    motion_pointer: Rc<Cell<bool>>,
    /// The last Motion speed told to the root through [`SettingsEvent::Motion`].
    published_motion: Option<u8>,
    /// Whether the host renders on a real GPU, asked of the window once.
    accelerated: Option<bool>,
    theme_open: bool,
    theme_highlight: usize,
    theme_trigger: FocusHandle,
    theme_content: FocusHandle,
    theme_bounds: BoundsCell,
    background_open: bool,
    background_highlight: usize,
    background_trigger: FocusHandle,
    background_content: FocusHandle,
    background_bounds: BoundsCell,
    _events: Vec<Subscription>,
}

/// Which dropdown a [`Settings::select`] call renders.
#[derive(Clone, Copy)]
enum Choice {
    Theme,
    Background,
}

impl Settings {
    /// Builds the page. With a host, read-only state is loaded (appearance,
    /// Discord preferences, desktop shortcuts, data root); nothing connects,
    /// no update check runs and nothing writes. Without a host the page
    /// stays in an empty diagnostic state and makes no calls at all.
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let accent_hex = cx.new(|inner| InputState::new(window, inner).placeholder("#rrggbb"));
        let motion_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(50.0)
        });
        let events = vec![
            cx.subscribe(&motion_slider, |this, _, event: &SliderEvent, cx| {
                this.on_motion_slider(event, cx);
            }),
            cx.subscribe(&accent_hex, |this, input, event: &InputEvent, cx| {
                let text = input.read(cx).value().to_string();
                match event {
                    InputEvent::Change => this.picker_hex_edited(&text, cx),
                    InputEvent::PressEnter { .. } => this.picker_apply(cx),
                    _ => {}
                }
            }),
        ];
        let mut view = Self {
            section: 0,
            host: host.clone(),
            rt: runtime,
            appearance: AppearanceState::default(),
            appearance_read: 0,
            discord_read: 0,
            discord_event: 0,
            desktop_read: 0,
            discord: None,
            discord_busy: false,
            discord_error: None,
            desktop: None,
            shortcuts_busy: false,
            desktop_error: None,
            data_root: None,
            updates: None,
            update_busy: false,
            downloaded_version: None,
            install_blocked: true,
            update_detail: None,
            updates_error: None,
            accent_hex,
            picker: None,
            accent_preview: None,
            sv_bounds: Rc::new(Cell::new(None)),
            hue_bounds: Rc::new(Cell::new(None)),
            sv_focus: cx.focus_handle(),
            hue_focus: cx.focus_handle(),
            motion_slider,
            motion_pointer: Rc::new(Cell::new(false)),
            published_motion: None,
            accelerated: None,
            theme_open: false,
            theme_highlight: 0,
            theme_trigger: cx.focus_handle(),
            theme_content: cx.focus_handle(),
            theme_bounds: Rc::new(Cell::new(None)),
            background_open: false,
            background_highlight: 0,
            background_trigger: cx.focus_handle(),
            background_content: cx.focus_handle(),
            background_bounds: Rc::new(Cell::new(None)),
            _events: events,
        };
        view.load_reads(cx);
        view
    }

    /// Re-reads the read-only settings state. Same contract as the initial
    /// load: reads only, no side effects.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load_reads(cx);
    }
    pub fn appearance(&self) -> Option<&api::Appearance> {
        self.appearance.appearance()
    }

    fn load_reads(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if !self.appearance.is_busy() {
            self.appearance_read += 1;
            let revision = self.appearance_read;
            self.request_read(
                cx,
                {
                    let host = host.clone();
                    async move { api::appearance(host).await }
                },
                move |view, result, cx| {
                    if view.appearance_read == revision {
                        view.receive(Message::AppearanceRead(result), cx);
                    }
                },
            );
        }
        if !self.discord_busy {
            self.discord_read += 1;
            let revision = self.discord_read;
            self.request_read(
                cx,
                async move { api::discord_state() },
                move |view, result, cx| {
                    if view.discord_read == revision {
                        view.receive(Message::DiscordRead(result), cx);
                    }
                },
            );
        }
        if !self.shortcuts_busy {
            self.desktop_read += 1;
            let revision = self.desktop_read;
            self.request_read(
                cx,
                async move { api::desktop_integration().await },
                move |view, result, cx| {
                    if view.desktop_read == revision {
                        view.receive(Message::DesktopRead(result), cx);
                    }
                },
            );
        }
        self.request_read(
            cx,
            {
                let host = host.clone();
                async move { api::application_status(&host) }
            },
            |view, result, cx| view.receive(Message::StatusRead(result), cx),
        );
    }

    /// Runs one read on the Tokio runtime and delivers its result through an
    /// awaited oneshot channel, never by polling.
    fn request_read<T, A>(
        &self,
        cx: &mut Context<Self>,
        work: A,
        deliver: impl Fn(&mut Self, ApiResult<T>, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
        A: std::future::Future<Output = ApiResult<T>> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let result = guarded(work).await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            if let Ok(result) = rx.await {
                let _ = weak.update(cx, |view, cx| deliver(view, result, cx));
            }
        })
        .detach();
    }

    // -- appearance ---------------------------------------------------------

    /// An acknowledged (or restored) appearance is authoritative again: any
    /// accent preview gives way to it, like the Slint host's `theme::apply`.
    fn apply_appearance(&mut self, cx: &mut Context<Self>) {
        self.accent_preview = None;
        theme::apply(self.appearance.appearance(), cx);
        cx.emit(SettingsEvent::Changed);
        self.publish_motion(cx);
    }

    /// The acknowledged appearance with the picker's live accent preview, for
    /// the root to theme itself with. Nothing without a valid snapshot.
    pub fn display_appearance(&self) -> Option<api::Appearance> {
        let mut appearance = self.appearance.appearance()?.clone();
        if let Some(palette) = &self.accent_preview {
            appearance.palette = palette.clone();
        }
        Some(appearance)
    }

    /// The Motion speed the Borealis view should run at (draft, queued or
    /// acknowledged), for a root that builds this page after startup.
    pub fn motion_speed(&self) -> Option<u8> {
        self.appearance.motion_speed()
    }

    /// Tells the root when the effective Motion speed changed.
    fn publish_motion(&mut self, cx: &mut Context<Self>) {
        let Some(speed) = self.appearance.motion_speed() else {
            return;
        };
        if self.published_motion != Some(speed) {
            self.published_motion = Some(speed);
            cx.emit(SettingsEvent::Motion(speed));
        }
    }

    fn save_appearance(
        &mut self,
        edit: impl FnOnce(&mut AppearanceRequest),
        cx: &mut Context<Self>,
    ) {
        let Some(host) = self.host.clone() else {
            return;
        };
        // A complete snapshot (acknowledged or queued) is required; malformed
        // state is never overwritten with a partial write.
        let Some(dispatch) = self.appearance.stage(edit) else {
            return;
        };
        self.appearance_read += 1;
        if let Some(request) = dispatch {
            self.dispatch_appearance(host, request, cx);
        }
        self.publish_motion(cx);
        cx.notify();
    }

    // -- motion -------------------------------------------------------------

    /// A slider change. Pointer drags are drafts that preview live; keyboard
    /// and assistive steps (no pointer down, no release) commit at once, like
    /// the Slint slider's `released` after every key.
    fn on_motion_slider(&mut self, event: &SliderEvent, cx: &mut Context<Self>) {
        match event {
            SliderEvent::Change(value) => {
                let speed = normalize_motion(value.end());
                self.appearance.preview_motion(speed);
                if self.motion_pointer.get() {
                    self.publish_motion(cx);
                    cx.notify();
                } else {
                    self.release_motion(speed, cx);
                }
            }
            SliderEvent::Release(value) => {
                self.motion_pointer.set(false);
                self.release_motion(normalize_motion(value.end()), cx);
            }
        }
    }

    /// Ends the draft and writes the speed through the shared queue.
    fn release_motion(&mut self, speed: u8, cx: &mut Context<Self>) {
        self.appearance.end_motion_preview();
        self.save_appearance(motion_edit(speed), cx);
        self.publish_motion(cx);
        cx.notify();
    }

    /// Navigation discards only an unreleased slider draft, restoring the
    /// queued or acknowledged speed; a released write is never undone.
    pub fn cancel_motion_preview(&mut self, cx: &mut Context<Self>) {
        self.motion_pointer.set(false);
        if self.appearance.cancel_motion_preview().is_some() {
            self.publish_motion(cx);
            cx.notify();
        }
    }

    /// Everything a page change must discard: an unreleased Motion draft and
    /// an open accent picker (restoring the saved accent). Call it whenever
    /// the root navigates, including away from Settings.
    pub fn cancel_previews(&mut self, cx: &mut Context<Self>) {
        self.cancel_motion_preview(cx);
        self.picker_cancel(cx);
    }

    // -- custom accent picker -----------------------------------------------

    /// Slint `show`: opens on the saved accent hex (else the palette accent).
    fn picker_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(appearance) = self.appearance.appearance() else {
            return;
        };
        let start = appearance
            .accent
            .get("hex")
            .and_then(|hex| hex.as_str())
            .unwrap_or(&appearance.palette.accent)
            .to_owned();
        let picker = AccentPickerState::open(&start);
        self.picker = Some(picker);
        self.picker_changed(window, cx);
    }

    /// Mirrors the picker's hex into the field and previews it.
    fn picker_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = &self.picker else {
            return;
        };
        let hex = picker.hex.clone();
        self.accent_hex
            .update(cx, |input, cx| input.set_value(hex.clone(), window, cx));
        self.picker_preview(&hex, cx);
    }

    /// Validates `hex` with the core rules and previews it live. A rejected
    /// color keeps the last valid preview applied, like the Slint host.
    fn picker_preview(&mut self, hex: &str, cx: &mut Context<Self>) {
        let verdict = custom_palette(hex);
        if let Some(picker) = &mut self.picker {
            picker.set_validity(verdict.as_ref().map(|_| ()).map_err(String::as_str));
        }
        if let Ok(palette) = verdict {
            self.accent_preview = Some(palette);
            theme::apply(self.display_appearance().as_ref(), cx);
            cx.emit(SettingsEvent::Preview);
        }
        cx.notify();
    }

    fn picker_set_sv(
        &mut self,
        saturation: f32,
        value: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(picker) = &mut self.picker {
            picker.set_sv(saturation, value);
            self.picker_changed(window, cx);
        }
    }

    fn picker_set_hue(&mut self, hue: f32, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(picker) = &mut self.picker {
            picker.set_hue(hue);
            self.picker_changed(window, cx);
        }
    }

    /// The hex field's text changed. Programmatic mirrors of the picker's own
    /// hex arrive here too and are recognized by being identical.
    fn picker_hex_edited(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        if picker.hex == text {
            return;
        }
        match picker.edit_hex(text) {
            Some(hex) => self.picker_preview(&hex, cx),
            None => cx.notify(),
        }
    }

    /// Apply writes the validated color and closes the picker; the live
    /// preview stays until the write is acknowledged or restored.
    fn picker_apply(&mut self, cx: &mut Context<Self>) {
        let Some(hex) = self.picker.as_ref().and_then(AccentPickerState::apply) else {
            return;
        };
        if self.host.is_none() || self.appearance.request().is_none() {
            self.picker_cancel(cx);
            return;
        }
        self.picker = None;
        self.save_appearance(accent_edit(custom_accent_value(&hex)), cx);
        cx.notify();
    }

    /// Cancel (and Escape) restore the saved accent after any live preview.
    fn picker_cancel(&mut self, cx: &mut Context<Self>) {
        // An applied accent awaiting its acknowledgement is not a preview to
        // discard; only an open picker is.
        if self.picker.take().is_none() {
            return;
        }
        self.accent_preview = None;
        theme::apply(self.appearance.appearance(), cx);
        cx.emit(SettingsEvent::Preview);
        cx.notify();
    }

    fn dispatch_appearance(
        &mut self,
        host: Host,
        request: AppearanceRequest,
        cx: &mut Context<Self>,
    ) {
        let revision = request.revision;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let result = guarded(api::set_appearance(
                host,
                request.theme,
                request.accent,
                request.background,
                request.motion,
            ))
            .await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            if let Ok(result) = rx.await {
                let _ = weak.update(cx, |view, cx| {
                    view.receive(Message::AppearanceSaved(revision, result), cx)
                });
            }
        })
        .detach();
    }

    // -- discord ------------------------------------------------------------

    fn save_discord(
        &mut self,
        edit: impl FnOnce(&mut api::DiscordPreferences),
        cx: &mut Context<Self>,
    ) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(state) = self.discord.as_ref() else {
            return;
        };
        if self.discord_busy {
            return;
        }
        let mut preferences = state.preferences.clone();
        edit(&mut preferences);
        // The address is a sub-choice of sharing the server at all.
        preferences.server_address = preferences.server && preferences.server_address;
        self.discord_busy = true;
        self.discord_read += 1;
        let event_revision = self.discord_event;
        self.discord_error = None;
        cx.notify();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let result = guarded(api::set_discord_preferences(host, preferences)).await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            if let Ok(result) = rx.await {
                let _ = weak.update(cx, |view, cx| {
                    view.receive(Message::DiscordSaved(event_revision, result), cx)
                });
            }
        })
        .detach();
    }

    fn connect_discord(&mut self, cx: &mut Context<Self>) {
        if self.host.is_none() || self.discord_busy {
            return;
        }
        self.discord_busy = true;
        self.discord_read += 1;
        let event_revision = self.discord_event;
        cx.notify();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let result = guarded(api::connect_discord()).await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            if let Ok(result) = rx.await {
                let _ = weak.update(cx, |view, cx| {
                    view.receive(Message::DiscordSaved(event_revision, result), cx)
                });
            }
        })
        .detach();
    }

    // -- desktop ------------------------------------------------------------

    fn shortcut_operation(&mut self, operation: ShortcutOperation, cx: &mut Context<Self>) {
        if self.host.is_none() || self.shortcuts_busy {
            return;
        }
        self.shortcuts_busy = true;
        self.desktop_read += 1;
        cx.notify();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let result = match operation {
                ShortcutOperation::CreateDesktop => guarded(api::create_desktop_shortcut()).await,
                ShortcutOperation::RemoveDesktop => guarded(api::remove_desktop_shortcut()).await,
                ShortcutOperation::CreateStartMenu => {
                    guarded(api::create_start_menu_shortcut()).await
                }
            };
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            if let Ok(result) = rx.await {
                let _ = weak.update(cx, |view, cx| {
                    view.receive(Message::DesktopSaved(result), cx)
                });
            }
        })
        .detach();
    }

    fn open_launcher_folder(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let result = guarded(api::open_launcher_folder(host)).await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            if let Ok(result) = rx.await {
                let _ = weak.update(cx, |view, cx| {
                    view.receive(Message::FolderOpened(result), cx)
                });
            }
        })
        .detach();
    }

    // -- updates ------------------------------------------------------------

    pub fn set_install_blocked(&mut self, blocked: bool, cx: &mut Context<Self>) {
        if self.install_blocked != blocked {
            self.install_blocked = blocked;
            cx.notify();
        }
    }

    fn launcher_operation(&mut self, install: bool, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(version) = self
            .updates
            .as_ref()
            .and_then(|o| launcher_offer(&o.launcher.availability))
        else {
            return;
        };
        if self.update_busy
            || (install
                && (self.install_blocked || self.downloaded_version.as_deref() != Some(&version)))
        {
            return;
        }
        self.update_busy = true;
        self.updates_error = None;
        self.update_detail = Some(
            if install {
                "Revalidating the signed installer before exiting\u{2026}"
            } else {
                "Downloading and verifying the signed installer\u{2026}"
            }
            .into(),
        );
        if install {
            cx.emit(SettingsEvent::Installing(true));
        }
        cx.notify();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let message = if install {
                Message::LauncherInstalled(
                    guarded(api::install_launcher_update(host, version)).await,
                )
            } else {
                let result = guarded(api::download_launcher_update(host, version.clone())).await;
                Message::LauncherDownloaded(version, result)
            };
            let _ = tx.send(message);
        });
        cx.spawn(async move |weak, cx| {
            if let Ok(message) = rx.await {
                let _ = weak.update(cx, |view, cx| view.receive(message, cx));
            }
        })
        .detach();
    }

    fn check_updates(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.update_busy {
            return;
        }
        self.update_busy = true;
        self.updates_error = None;
        cx.notify();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let result = guarded(api::check_for_updates(host)).await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            if let Ok(result) = rx.await {
                let _ = weak.update(cx, |view, cx| {
                    view.receive(Message::UpdatesChecked(result), cx)
                });
            }
        })
        .detach();
    }

    // -- message routing ----------------------------------------------------

    fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::AppearanceRead(result) => {
                self.appearance.loaded(result);
                self.apply_appearance(cx);
            }
            Message::AppearanceSaved(revision, result) => {
                let (completion, next) = self.appearance.complete(revision, result);
                if let Some(request) = next {
                    if let Some(host) = self.host.clone() {
                        self.dispatch_appearance(host, request, cx);
                    }
                }
                if completion == Completion::Applied
                    || (completion == Completion::Failed && !self.appearance.is_busy())
                {
                    self.apply_appearance(cx);
                }
            }
            Message::DiscordRead(result) => {
                self.discord_busy = false;
                match result {
                    Ok(state) => self.discord = Some(state),
                    Err(error) => self.discord_error = Some(error.message),
                }
            }
            Message::DiscordSaved(event_revision, result) => {
                self.discord_busy = false;
                match result {
                    Ok(mut state) => {
                        if event_revision != self.discord_event {
                            if let Some(current) = &self.discord {
                                state.connection = current.connection.clone();
                                state.configured = current.configured;
                            }
                        }
                        self.discord = Some(state);
                        self.discord_error = None;
                    }
                    Err(error) => self.discord_error = Some(error.message),
                }
            }
            Message::DesktopRead(result) => {
                self.shortcuts_busy = false;
                match result {
                    Ok(integration) => self.desktop = Some(integration),
                    Err(error) => self.desktop_error = Some(error.message),
                }
            }
            Message::DesktopSaved(result) => {
                self.shortcuts_busy = false;
                match result {
                    Ok(integration) => {
                        self.desktop = Some(integration);
                        self.desktop_error = None;
                    }
                    Err(error) => self.desktop_error = Some(error.message),
                }
            }
            Message::FolderOpened(Err(error)) => self.desktop_error = Some(error.message),
            Message::FolderOpened(Ok(())) => {}
            Message::StatusRead(result) => {
                if let Ok(status) = result {
                    self.data_root = Some(status.managed_data_root);
                }
            }
            Message::LauncherDownloaded(version, result) => {
                self.update_busy = false;
                match result {
                    Ok(_) => {
                        self.downloaded_version = Some(version);
                        self.update_detail = Some("Signature, hash and package identity verified. Exit and install closes Aurora and starts the installer.".into());
                    }
                    Err(error) => {
                        self.downloaded_version = None;
                        self.updates_error = Some(error.message);
                    }
                }
            }
            Message::LauncherInstalled(result) => {
                self.update_busy = false;
                match result {
                    Ok(_) => cx.quit(),
                    Err(error) => {
                        cx.emit(SettingsEvent::Installing(false));
                        self.updates_error = Some(error.message);
                    }
                }
            }
            Message::UpdatesChecked(result) => {
                self.update_busy = false;
                match result {
                    Ok(overview) => {
                        self.updates = Some(overview);
                        self.update_detail = None;
                        self.updates_error = None;
                    }
                    Err(error) => self.updates_error = Some(error.message),
                }
            }
        }
        cx.notify();
    }

    /// Routes one host event. Unknown names and unparseable payloads are
    /// ignored, never fatal.
    pub fn on_host_event(
        &mut self,
        name: &str,
        payload: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        match name {
            "discord-state" => {
                if let Ok(mut state) = serde_json::from_value::<api::DiscordState>(payload.clone())
                {
                    self.discord_event += 1;
                    self.discord_read += 1;
                    if self.discord_busy {
                        if let Some(current) = &self.discord {
                            state.preferences = current.preferences.clone();
                        }
                    }
                    self.discord = Some(state);
                    cx.notify();
                }
            }
            "update-status" => {
                if let Some(overview) = update_status_event(name, payload) {
                    self.updates = Some(overview);
                    cx.notify();
                }
            }
            "launcher-update-progress" => {
                if let Some(text) = launcher_progress_text(payload) {
                    self.update_detail = Some(text);
                    cx.notify();
                }
            }
            _ => {}
        }
    }

    // -- rendering ----------------------------------------------------------

    /// A themed select whose callbacks are bound to this view through the
    /// entity's weak handle (the base handlers report values, not events).
    fn select(
        &self,
        choice: Choice,
        label: &'static str,
        options: Vec<SharedString>,
        selected: Option<usize>,
        disabled: bool,
        weak: WeakEntity<Self>,
    ) -> AuroraSelect {
        let (open, highlight, trigger, content, bounds) = match choice {
            Choice::Theme => (
                self.theme_open,
                self.theme_highlight,
                &self.theme_trigger,
                &self.theme_content,
                &self.theme_bounds,
            ),
            Choice::Background => (
                self.background_open,
                self.background_highlight,
                &self.background_trigger,
                &self.background_content,
                &self.background_bounds,
            ),
        };
        AuroraSelect::new(label, label, options, trigger, content, bounds.clone())
            .selected(selected)
            .highlight(highlight)
            .open(open && !disabled)
            .on_open_change({
                let weak = weak.clone();
                move |open, _window, app| {
                    let _ = weak.update(app, |this, cx| {
                        if open && disabled {
                            return;
                        }
                        match choice {
                            Choice::Theme => this.theme_open = open,
                            Choice::Background => this.background_open = open,
                        }
                        cx.notify();
                    });
                }
            })
            .on_highlight({
                let weak = weak.clone();
                move |index, _window, app| {
                    let _ = weak.update(app, |this, cx| {
                        match choice {
                            Choice::Theme => this.theme_highlight = index,
                            Choice::Background => this.background_highlight = index,
                        }
                        cx.notify();
                    });
                }
            })
            .on_confirm(move |index, _window, app| {
                let _ = weak.update(app, |this, cx| {
                    match choice {
                        Choice::Theme => {
                            this.theme_open = false;
                            let option = this
                                .appearance
                                .appearance()
                                .and_then(|a| a.themes.get(index).map(|t| t.id.clone()));
                            if let Some(theme_id) = option {
                                this.save_appearance(theme_edit(theme_id), cx);
                            }
                        }
                        Choice::Background => {
                            this.background_open = false;
                            // An index outside the known backgrounds is ignored.
                            if let Some(background) = borealis::BACKGROUNDS.get(index) {
                                this.save_appearance(background_edit(background), cx);
                            }
                        }
                    }
                    cx.notify();
                });
            })
    }

    fn accent_selected(&self) -> Option<usize> {
        let appearance = self.appearance.appearance()?;
        let accent_id = appearance.accent.get("id").and_then(|id| id.as_str())?;
        appearance.accents.iter().position(|a| a.id == accent_id)
    }

    /// The saved custom accent and whether it is the one in use: Slint's
    /// `has-custom-accent` / `accent-id == "custom"`.
    fn custom_accent(&self) -> (Option<Hsla>, bool) {
        let Some(appearance) = self.appearance.appearance() else {
            return (None, false);
        };
        let color = appearance
            .accent
            .get("hex")
            .and_then(|hex| hex.as_str())
            .and_then(parse_hex_color);
        let chosen = appearance
            .accent
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or("custom")
            == "custom";
        (color, chosen)
    }

    /// The Slint picker's panel: saturation/value square, hue strip, preview
    /// swatch with the hex field, status line, and Cancel/Apply.
    fn accent_picker(
        &self,
        picker: &AccentPickerState,
        palette: &theme::AuroraTheme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let white: Hsla = rgb(0xffffff).into();
        let black: Hsla = rgb(0x000000).into();
        let hue_color = {
            let (r, g, b) = picker.hue_rgb();
            rgb_color(r, g, b)
        };
        let preview = {
            let (r, g, b) = picker.rgb();
            rgb_color(r, g, b)
        };

        let sv = div()
            .id("accent-sv")
            .relative()
            .w_full()
            .h(px(152.))
            .rounded(px(10.))
            .overflow_hidden()
            .bg(linear_gradient(
                90.,
                linear_color_stop(white, 0.),
                linear_color_stop(hue_color, 1.),
            ))
            .track_focus(&self.sv_focus)
            .focus(|style| style.border_1().border_color(palette.ring))
            .role(Role::Slider)
            .aria_label("Saturation and brightness")
            .aria_value(format!(
                "{}% saturation, {}% brightness",
                (picker.saturation * 100.0).round() as i32,
                (picker.value * 100.0).round() as i32
            ))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(hsla(0., 0., 0., 0.), 0.),
                        linear_color_stop(black, 1.),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(picker.saturation))
                    .top(relative(1.0 - picker.value))
                    .ml(px(-8.))
                    .mt(px(-8.))
                    .size(px(16.))
                    .rounded_full()
                    .border_2()
                    .border_color(white)
                    .bg(preview),
            )
            .on_prepaint({
                let cell = self.sv_bounds.clone();
                move |bounds, _, _| cell.set(Some(bounds))
            })
            .on_mouse_down(MouseButton::Left, {
                let weak = weak.clone();
                let cell = self.sv_bounds.clone();
                let focus = self.sv_focus.clone();
                move |event: &MouseDownEvent, window, cx| {
                    window.focus(&focus, cx);
                    let Some(bounds) = cell.get() else {
                        return;
                    };
                    let (x, y) = pointer_fractions(event.position, bounds);
                    let _ = weak.update(cx, |this, cx| {
                        this.picker_set_sv(x, 1.0 - y, window, cx);
                    });
                }
            })
            .on_drag(SvDrag, |_, _, _, cx| cx.new(|_| DragGhost))
            .on_drag_move::<SvDrag>({
                let weak = weak.clone();
                move |event: &DragMoveEvent<SvDrag>, window, cx| {
                    let (x, y) = pointer_fractions(event.event.position, event.bounds);
                    let _ = weak.update(cx, |this, cx| {
                        this.picker_set_sv(x, 1.0 - y, window, cx);
                    });
                }
            })
            .on_key_down({
                let weak = weak.clone();
                move |event: &KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.clone();
                    if !matches!(key.as_str(), "left" | "right" | "up" | "down") {
                        return;
                    }
                    cx.stop_propagation();
                    let _ = weak.update(cx, |this, cx| {
                        let Some((s, v)) = this.picker.as_ref().map(|p| (p.saturation, p.value))
                        else {
                            return;
                        };
                        // Slint steps two percent per arrow key.
                        let (s, v) = match key.as_str() {
                            "left" => (s - 0.02, v),
                            "right" => (s + 0.02, v),
                            "up" => (s, v + 0.02),
                            _ => (s, v - 0.02),
                        };
                        this.picker_set_sv(s, v, window, cx);
                    });
                }
            });

        let mut strip = div()
            .flex()
            .w_full()
            .h_full()
            .rounded_full()
            .overflow_hidden();
        for pair in HUE_STOPS.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            strip = strip.child(div().flex_1().h_full().bg(linear_gradient(
                90.,
                linear_color_stop(rgb_color(from.0, from.1, from.2), 0.),
                linear_color_stop(rgb_color(to.0, to.1, to.2), 1.),
            )));
        }
        let hue = div()
            .id("accent-hue")
            .relative()
            .w_full()
            .h(px(14.))
            .track_focus(&self.hue_focus)
            .focus(|style| style.border_1().border_color(palette.ring))
            .role(Role::Slider)
            .aria_label("Hue")
            .aria_value(format!("{} degrees", picker.hue.round() as i32))
            .child(strip)
            .child(
                div()
                    .absolute()
                    .left(relative(picker.hue / 360.0))
                    .ml(px(-9.))
                    .top(px(-2.))
                    .size(px(18.))
                    .rounded_full()
                    .border_2()
                    .border_color(white)
                    .bg(hue_color),
            )
            .on_prepaint({
                let cell = self.hue_bounds.clone();
                move |bounds, _, _| cell.set(Some(bounds))
            })
            .on_mouse_down(MouseButton::Left, {
                let weak = weak.clone();
                let cell = self.hue_bounds.clone();
                let focus = self.hue_focus.clone();
                move |event: &MouseDownEvent, window, cx| {
                    window.focus(&focus, cx);
                    let Some(bounds) = cell.get() else {
                        return;
                    };
                    let (x, _) = pointer_fractions(event.position, bounds);
                    let _ = weak.update(cx, |this, cx| {
                        this.picker_set_hue(x * 360.0, window, cx);
                    });
                }
            })
            .on_drag(HueDrag, |_, _, _, cx| cx.new(|_| DragGhost))
            .on_drag_move::<HueDrag>({
                let weak = weak.clone();
                move |event: &DragMoveEvent<HueDrag>, window, cx| {
                    let (x, _) = pointer_fractions(event.event.position, event.bounds);
                    let _ = weak.update(cx, |this, cx| {
                        this.picker_set_hue(x * 360.0, window, cx);
                    });
                }
            })
            .on_key_down({
                let weak = weak.clone();
                move |event: &KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.clone();
                    if !matches!(key.as_str(), "left" | "right" | "up" | "down") {
                        return;
                    }
                    cx.stop_propagation();
                    let _ = weak.update(cx, |this, cx| {
                        let Some(hue) = this.picker.as_ref().map(|p| p.hue) else {
                            return;
                        };
                        // Slint steps five degrees per arrow key.
                        let hue = match key.as_str() {
                            "left" | "down" => hue - 5.0,
                            _ => hue + 5.0,
                        };
                        this.picker_set_hue(hue, window, cx);
                    });
                }
            });

        let mut panel = div()
            .id("accent-picker")
            .flex()
            .flex_col()
            .gap(px(12.))
            .w(px(272.))
            .p(px(14.))
            .rounded(px(14.))
            .border_1()
            .border_color(palette.border)
            .bg(palette.surface)
            .role(Role::Group)
            .aria_label("Custom accent color")
            .on_key_down({
                let weak = weak.clone();
                move |event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        let _ = weak.update(cx, |this, cx| this.picker_cancel(cx));
                    }
                }
            })
            .child(sv)
            .child(hue)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex_shrink_0()
                            .size(px(36.))
                            .rounded_full()
                            .border_1()
                            .border_color(palette.border)
                            .bg(preview),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(AuroraInput::new(&self.accent_hex).label("Hex color")),
                    ),
            );
        if !picker.message.is_empty() {
            panel = panel.child(
                div()
                    .text_size(px(12.))
                    .text_color(if picker.valid {
                        palette.muted_foreground
                    } else {
                        palette.danger
                    })
                    .child(picker.message.clone()),
            );
        }
        panel.child(
            div()
                .flex()
                .justify_end()
                .gap(px(8.))
                .child(
                    AuroraButton::new("accent-cancel", "Cancel")
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| this.picker_cancel(cx))),
                )
                .child(
                    AuroraButton::new("accent-apply", "Apply")
                        .variant(ButtonVariant::Primary)
                        .disabled(!picker.valid)
                        .on_click(cx.listener(|this, _, _, cx| this.picker_apply(cx))),
                ),
        )
    }
}

/// The hue strip's colour stops: red through yellow, green, cyan, blue and
/// magenta back to red, in sixths.
const HUE_STOPS: [(u8, u8, u8); 7] = [
    (255, 0, 0),
    (255, 255, 0),
    (0, 255, 0),
    (0, 255, 255),
    (0, 0, 255),
    (255, 0, 255),
    (255, 0, 0),
];

// ---------------------------------------------------------------------------
// Presentation helpers
// ---------------------------------------------------------------------------

/// Width of the right-hand control column every settings row shares.
const CONTROL_WIDTH: f32 = 220.;

/// A right-aligned slot for one row's control, so selects, sliders, toggles
/// and buttons all hang off the same edge.
fn control_slot(child: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_end()
        .w(px(CONTROL_WIDTH))
        .child(child)
}

/// A settings card: rows run inside it separated by hairlines.
fn section_card(t: &theme::AuroraTheme) -> Div {
    // Rows carry their own vertical padding, so the card's bottom is trimmed.
    ui::card(t).gap(px(0.)).pb(px(6.))
}

/// The card's title (and caption) on the left, optional actions on the right.
fn card_header(
    t: &theme::AuroraTheme,
    title: &'static str,
    caption: Option<SharedString>,
    trailing: impl IntoElement,
) -> Div {
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap(px(16.))
        .pb(px(6.))
        .child(ui::section_header(t, title, caption).flex_1().min_w(px(0.)))
        .child(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(8.))
                .child(trailing),
        )
}

/// A caption-sized error under a card's rows.
fn card_error(t: &theme::AuroraTheme, text: impl Into<SharedString>) -> Div {
    div()
        .pt(px(12.))
        .text_size(px(ui::TEXT_CAPTION))
        .text_color(t.danger)
        .child(text.into())
}

/// The short status word and tone for an update availability value.
fn availability_badge(value: &serde_json::Value) -> (&'static str, ui::Tone) {
    match value.get("kind").and_then(serde_json::Value::as_str) {
        Some("upToDate") => ("Up to date", ui::Tone::Success),
        Some("updateAvailable") => ("Update available", ui::Tone::Accent),
        Some("unavailable") => ("Unavailable", ui::Tone::Warning),
        Some("notApplicable") => ("Not applicable", ui::Tone::Neutral),
        _ => ("Not checked", ui::Tone::Neutral),
    }
}

/// Text that says more than the status badge does (a version, a reason).
fn availability_detail(value: &serde_json::Value) -> Option<String> {
    match value.get("kind").and_then(serde_json::Value::as_str) {
        Some("upToDate" | "notChecked") | None => None,
        Some(_) => Some(availability_text(value)),
    }
}

/// The Discord card's status pill.
fn discord_badge(state: Option<&api::DiscordState>) -> Option<(&'static str, ui::Tone)> {
    let state = state?;
    Some(if !state.configured {
        ("Not configured", ui::Tone::Warning)
    } else if !state.preferences.enabled {
        // "Off" would only repeat the Rich Presence toggle below.
        return None;
    } else {
        match state.connection.as_str() {
            "connected" => ("Connected", ui::Tone::Success),
            "notDetected" => ("Discord not running", ui::Tone::Warning),
            "failed" => ("Connection failed", ui::Tone::Danger),
            "closed" => ("Disconnected", ui::Tone::Neutral),
            _ => ("Connecting", ui::Tone::Accent),
        }
    })
}

impl EventEmitter<SettingsEvent> for Settings {}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx).clone();
        let t = &palette;
        let weak = cx.entity().downgrade();
        let no_host = self.host.is_none();
        let busy = self.appearance.is_busy() || no_host;

        // The slider shows the effective speed (draft, queued or
        // acknowledged), so a restored or acknowledged value moves it too.
        // `set_value` emits no slider event, so this never feeds back.
        if let Some(speed) = self.appearance.motion_speed() {
            let shown = self.motion_slider.read(cx).value().end();
            if normalize_motion(shown) != speed {
                self.motion_slider.update(cx, |slider, cx| {
                    slider.set_value(f32::from(speed), window, cx)
                });
            }
        }

        let mut page = div()
            .id("settings-content")
            .flex()
            .flex_col()
            .gap(px(ui::PAGE_GAP))
            .w_full()
            .text_color(palette.foreground)
            .child(ui::page_header(
                t,
                "Settings",
                Some(
                    "Personalize Aurora, connect Discord and keep the launcher up to date.".into(),
                ),
            ));

        if no_host {
            return page.child(ui::empty_state(
                t,
                "settings",
                "Settings are unavailable",
                "Settings are unavailable: this diagnostic window has no launcher host, so no preferences, credentials or integrations are touched.",
            ));
        }

        let mut tabs = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(t.radius)
            .bg(t.sunken)
            .border_1()
            .border_color(t.border_subtle);
        for (index, label) in ["Appearance", "Discord", "Desktop", "Updates & about"]
            .into_iter()
            .enumerate()
        {
            tabs = tabs.child(
                AuroraButton::new(("settings-section", index), label)
                    .variant(ButtonVariant::Ghost)
                    .selected(self.section == index)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.cancel_previews(cx);
                        this.section = index;
                        cx.notify();
                    })),
            );
        }
        page = page.child(div().flex().child(tabs));
        if self.section == 0 {
            // --- Appearance ----------------------------------------------------
            {
                let appearance = self.appearance.appearance();
                let has_motion = self.appearance.motion_speed().is_some();
                let mut card = section_card(t).child(card_header(
                    t,
                    "Appearance",
                    Some("How the launcher looks and moves.".into()),
                    div(),
                ));

                let theme_options = appearance
                    .map(|a| {
                        a.themes
                            .iter()
                            .map(|t| SharedString::from(t.label.clone()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let theme_selected =
                    appearance.and_then(|a| a.themes.iter().position(|t| t.id == a.theme));
                card = card.child(
                    ui::setting_row(
                        t,
                        "Theme",
                        Some("The color scheme of every surface.".into()),
                        false,
                    )
                    .child(control_slot(self.select(
                        Choice::Theme,
                        "Theme",
                        theme_options,
                        theme_selected,
                        busy,
                        weak.clone(),
                    ))),
                );

                // Accent: the presets, then the custom swatch that opens the picker.
                let accents = appearance.map(|a| a.accents.clone()).unwrap_or_default();
                let selected = self.accent_selected();
                let (custom_color, custom_chosen) = self.custom_accent();
                let picker_open = self.picker.is_some();
                let ring = palette.foreground;
                let mut swatches = div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_start()
                    .gap(px(8.));
                for (ix, accent) in accents.iter().enumerate() {
                    let color = parse_hex_color(&accent.hex).unwrap_or(palette.accent);
                    let chosen = selected == Some(ix);
                    let accent_id = accent.id.clone();
                    swatches = swatches.child(
                        gpui_kit::base::Button::new(("accent", ix))
                            .aria_label(accent.label.clone())
                            .selected(chosen)
                            .disabled(busy)
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .h(px(32.))
                            .px(px(10.))
                            .rounded(palette.radius)
                            .bg(palette.control)
                            .border_1()
                            .border_color(if chosen { ring } else { palette.border_subtle })
                            .text_size(px(ui::TEXT_BODY))
                            .text_color(palette.foreground)
                            .focus(move |style| style.border_color(ring).border_2())
                            .hover(move |style| style.border_color(ring))
                            .styles(|styles| styles.disabled(|style| style.opacity(0.45)))
                            .child(div().size(px(14.)).rounded_full().bg(color))
                            .child(accent.label.clone())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.save_appearance(
                                    accent_edit(
                                        serde_json::json!({"type":"preset","id":accent_id}),
                                    ),
                                    cx,
                                );
                            })),
                    );
                }
                let custom_selected = custom_chosen || picker_open;
                swatches = swatches.child(
                    gpui_kit::base::Button::new("accent-custom")
                        .aria_label("Custom accent color")
                        .selected(custom_selected)
                        .disabled(appearance.is_none())
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(32.))
                        .px(px(10.))
                        .rounded(palette.radius)
                        .bg(palette.control)
                        .border_1()
                        .border_color(if custom_selected {
                            ring
                        } else {
                            palette.border_subtle
                        })
                        .text_size(px(ui::TEXT_BODY))
                        .text_color(palette.foreground)
                        .focus(move |style| style.border_color(ring).border_2())
                        .hover(move |style| style.border_color(ring))
                        .child(match custom_color {
                            Some(color) => div()
                                .size(px(14.))
                                .rounded_full()
                                .bg(color)
                                .into_any_element(),
                            // No custom color yet: an add affordance, not a gray swatch.
                            None => crate::symbols::icon(Some("add"), px(16.), palette.ring)
                                .into_any_element(),
                        })
                        .child("Custom…")
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.picker.is_some() {
                                this.picker_cancel(cx);
                            } else {
                                this.picker_open(window, cx);
                            }
                        })),
                );
                card = card.child(
                    ui::setting_row(
                        t,
                        "Accent",
                        Some("Used for selection, focus and primary actions.".into()),
                        false,
                    )
                    .flex_col()
                    .items_start()
                    .gap(px(12.))
                    .child(swatches),
                );
                if let Some(picker) = &self.picker {
                    card = card.child(
                        div()
                            .flex()
                            .justify_end()
                            .py(px(14.))
                            .border_b_1()
                            .border_color(palette.border_subtle)
                            .child(self.accent_picker(picker, &palette, cx)),
                    );
                }

                // Background: Simple or the Borealis layer.
                let background_selected = appearance.map(|a| {
                    borealis::BACKGROUNDS
                        .iter()
                        .position(|background| *background == a.background)
                        .unwrap_or(0)
                });
                card = card.child(
                    ui::setting_row(
                        t,
                        "Background",
                        background_selected.map(|index| BACKGROUND_DESCRIPTIONS[index].into()),
                        !has_motion,
                    )
                    .child(control_slot(self.select(
                        Choice::Background,
                        "Background",
                        BACKGROUND_LABELS.map(SharedString::from).to_vec(),
                        background_selected,
                        busy || appearance.is_none(),
                        weak.clone(),
                    ))),
                );

                // Motion speed: a live preview while dragging, one write on
                // release. Only offered with a valid snapshot, never fabricated
                // from malformed or unavailable state.
                if has_motion {
                    let accelerated = *self
                        .accelerated
                        .get_or_insert_with(|| borealis::host_accelerated(window));
                    card = card.child(
                        ui::setting_row(
                            t,
                            "Motion speed",
                            Some(motion_detail(accelerated).into()),
                            false,
                        )
                        .child(
                            control_slot(
                                div().w_full().child(
                                    AuroraSlider::new(&self.motion_slider, "Motion speed")
                                        .show_label(false),
                                ),
                            )
                            .w(px(CONTROL_WIDTH + 56.))
                            // The slider reports no pointer state, so the
                            // press and release around it are tracked here.
                            .capture_any_mouse_down({
                                let pointer = self.motion_pointer.clone();
                                move |event: &MouseDownEvent, _, _| {
                                    if event.button == MouseButton::Left {
                                        pointer.set(true);
                                    }
                                }
                            })
                            .capture_any_mouse_up({
                                let pointer = self.motion_pointer.clone();
                                move |_, window, cx| {
                                    // After the slider's own release handling.
                                    let pointer = pointer.clone();
                                    window.defer(cx, move |_, _| pointer.set(false));
                                }
                            }),
                        ),
                    );
                }

                // Reduce motion writes 0 (on) or 50 (off) with the Slint host's
                // parity and follows the same effective value as the slider.
                if let Some(reduced) = self.appearance.reduce_motion() {
                    card = card.child(
                        ui::setting_row(
                            t,
                            "Reduce motion",
                            Some("Stops transitions and Borealis motion.".into()),
                            true,
                        )
                        .child(control_slot(
                            AuroraToggle::new("reduce-motion")
                                .label("Reduce motion")
                                .checked(reduced)
                                .on_change({
                                    let weak = weak.clone();
                                    move |next: bool, _event, _window, app| {
                                        let _ = weak.update(app, |this, cx| {
                                            this.release_motion(reduce_motion_speed(next), cx);
                                        });
                                    }
                                }),
                        )),
                    );
                }
                if let Some(error) = self.appearance.error() {
                    card = card.child(card_error(t, error.to_string()));
                }
                page = page.child(card);
            }
        }
        if self.section == 1 {
            // --- Discord -------------------------------------------------------
            {
                let state = self.discord.clone();
                let configured = state.as_ref().map(|s| s.configured).unwrap_or(false);
                let preferences = state
                    .as_ref()
                    .map(|s| s.preferences.clone())
                    .unwrap_or_default();
                let discord_disabled = self.discord_busy || no_host;
                let header_actions = div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .children((preferences.enabled && configured).then(|| {
                        AuroraButton::new("discord-reconnect", "Reconnect")
                            .size(ButtonSize::Small)
                            .disabled(discord_disabled)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.connect_discord(cx);
                            }))
                    }))
                    .children(
                        discord_badge(state.as_ref())
                            .map(|(label, tone)| ui::status_badge(t, label, tone)),
                    );
                let mut card = section_card(t).child(card_header(
                    t,
                    "Discord",
                    state.as_ref().map(|s| discord_status(s).into()),
                    header_actions,
                ));
                let weak_toggle = weak.clone();
                card = card.child(
                ui::setting_row(
                    t,
                    "Rich Presence",
                    Some(
                        "Shows that you are playing through Aurora in your Discord status. Nothing is shared until this is on."
                            .into(),
                    ),
                    !preferences.enabled && self.discord_error.is_none(),
                )
                .child(control_slot(
                    AuroraToggle::new("discord-enabled")
                        .label("Discord Rich Presence")
                        .checked(preferences.enabled)
                        .disabled(discord_disabled)
                        .on_change({
                            let weak_toggle = weak_toggle.clone();
                            move |next: bool, _event, _window, app| {
                                let _ = weak_toggle.update(app, |this, cx| {
                                    this.save_discord(
                                        move |preferences| preferences.enabled = next,
                                        cx,
                                    );
                                });
                            }
                        }),
                )),
            );
                if preferences.enabled {
                    let rows = [
                        ("discord-instance-name", "Instance name", "instance_name"),
                        (
                            "discord-minecraft-version",
                            "Minecraft version",
                            "minecraft_version",
                        ),
                        ("discord-platform", "Mod loader", "platform"),
                        (
                            "discord-aurora-active",
                            "Aurora Client status",
                            "aurora_active",
                        ),
                        ("discord-elapsed-time", "Elapsed time", "elapsed_time"),
                        ("discord-world", "World name", "world"),
                        ("discord-server", "Server name", "server"),
                        ("discord-server-address", "Server address", "server_address"),
                    ];
                    let rows: Vec<_> = rows
                        .into_iter()
                        .filter(|(_, _, field)| *field != "server_address" || preferences.server)
                        .collect();
                    let split = rows.len().div_ceil(2);
                    let mut columns = [
                        div().flex().flex_col().flex_1().min_w(px(0.)),
                        div().flex().flex_col().flex_1().min_w(px(0.)),
                    ];
                    for (ix, (id, label, field)) in rows.into_iter().enumerate() {
                        let checked = match field {
                            "instance_name" => preferences.instance_name,
                            "minecraft_version" => preferences.minecraft_version,
                            "platform" => preferences.platform,
                            "aurora_active" => preferences.aurora_active,
                            "elapsed_time" => preferences.elapsed_time,
                            "world" => preferences.world,
                            "server" => preferences.server,
                            _ => preferences.server_address,
                        };
                        let cell = div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .h(px(40.))
                            .child(
                                div()
                                    .text_size(px(ui::TEXT_BODY))
                                    .text_color(palette.foreground)
                                    .child(label),
                            )
                            .child(
                                AuroraToggle::new(id)
                                    .label(format!("Share the {label}"))
                                    .checked(checked)
                                    .disabled(discord_disabled)
                                    .on_change({
                                        let weak = weak_toggle.clone();
                                        move |next: bool, _event, _window, app| {
                                            let _ = weak.update(app, |this, cx| {
                                                this.save_discord(
                                                    move |preferences| match field {
                                                        "instance_name" => {
                                                            preferences.instance_name = next
                                                        }
                                                        "minecraft_version" => {
                                                            preferences.minecraft_version = next
                                                        }
                                                        "platform" => preferences.platform = next,
                                                        "aurora_active" => {
                                                            preferences.aurora_active = next
                                                        }
                                                        "elapsed_time" => {
                                                            preferences.elapsed_time = next
                                                        }
                                                        "world" => preferences.world = next,
                                                        "server" => preferences.server = next,
                                                        _ => preferences.server_address = next,
                                                    },
                                                    cx,
                                                );
                                            });
                                        }
                                    }),
                            );
                        let [left, right] = &mut columns;
                        if ix < split {
                            *left = std::mem::replace(left, div()).child(cell);
                        } else {
                            *right = std::mem::replace(right, div()).child(cell);
                        }
                    }
                    let [left, right] = columns;
                    card = card.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .pt(px(14.))
                            .child(ui::eyebrow(t, "SHARED DETAILS"))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(32.))
                                    .px(px(16.))
                                    .py(px(6.))
                                    .rounded(palette.radius)
                                    .bg(palette.sunken)
                                    .border_1()
                                    .border_color(palette.border_subtle)
                                    .child(left)
                                    .child(right),
                            ),
                    );
                }
                if let Some(error) = &self.discord_error {
                    card = card.child(card_error(t, error.clone()));
                }
                page = page.child(card);
            }
        }
        if self.section == 2 {
            // --- Desktop -------------------------------------------------------
            {
                let integration = self.desktop.clone();
                let supported = integration
                    .as_ref()
                    .map(|i| i.supported && i.manageable)
                    .unwrap_or(false);
                let disabled = self.shortcuts_busy || no_host;
                let mut card = section_card(t).child(card_header(
                    t,
                    "Desktop",
                    Some("Shortcuts and where Aurora keeps its files.".into()),
                    div(),
                ));
                if let Some(integration) = &integration {
                    if supported {
                        let desktop_present = integration.desktop_shortcut.state == "present";
                        card = card.child(
                            ui::setting_row(
                                t,
                                "Desktop shortcut",
                                Some(
                                    shortcut_text(
                                        &integration.desktop_shortcut.state,
                                        &integration.desktop_shortcut.managed_by,
                                    )
                                    .into(),
                                ),
                                false,
                            )
                            .child(control_slot(
                                AuroraButton::new(
                                    "toggle-desktop-shortcut",
                                    if desktop_present { "Remove" } else { "Create" },
                                )
                                .size(ButtonSize::Small)
                                .disabled(disabled)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.shortcut_operation(
                                            if desktop_present {
                                                ShortcutOperation::RemoveDesktop
                                            } else {
                                                ShortcutOperation::CreateDesktop
                                            },
                                            cx,
                                        );
                                    },
                                )),
                            )),
                        );
                        if integration.start_menu_shortcut.state != "present" {
                            card = card.child(
                                ui::setting_row(
                                    t,
                                    "Start menu shortcut",
                                    Some(
                                        shortcut_text(
                                            &integration.start_menu_shortcut.state,
                                            &integration.start_menu_shortcut.managed_by,
                                        )
                                        .into(),
                                    ),
                                    false,
                                )
                                .child(control_slot(
                                    AuroraButton::new("create-start-menu-shortcut", "Create")
                                        .size(ButtonSize::Small)
                                        .disabled(disabled)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.shortcut_operation(
                                                ShortcutOperation::CreateStartMenu,
                                                cx,
                                            );
                                        })),
                                )),
                            );
                        }
                    }
                }
                card = card.child(
                    ui::setting_row(
                        t,
                        "Launcher folder",
                        self.data_root.clone().map(SharedString::from),
                        true,
                    )
                    .child(control_slot(
                        AuroraButton::new("open-launcher-folder", "Open")
                            .size(ButtonSize::Small)
                            .icon("folder_open")
                            .disabled(no_host)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_launcher_folder(cx);
                            })),
                    )),
                );
                if let Some(error) = &self.desktop_error {
                    card = card.child(card_error(t, error.clone()));
                }
                page = page.child(card);
            }
        }
        if self.section == 3 {
            // --- Updates -------------------------------------------------------
            {
                let mut card = section_card(t).child(card_header(
                    t,
                    "Updates",
                    Some("Whether the launcher and Aurora Client are current.".into()),
                    div(),
                ));
                if let Some(overview) = &self.updates {
                    let (launcher_label, launcher_tone) =
                        availability_badge(&overview.launcher.availability);
                    let launcher_detail = [
                        Some(format!(
                            "Installed version {}",
                            overview.launcher.installed_version
                        )),
                        availability_detail(&overview.launcher.availability),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                    card = card.child(
                        ui::setting_row(t, "Launcher", Some(launcher_detail.into()), false).child(
                            control_slot(ui::status_badge(t, launcher_label, launcher_tone)),
                        ),
                    );
                    let (client_label, client_tone) = availability_badge(&overview.client);
                    card = card.child(
                        ui::setting_row(
                            t,
                            "Aurora Client",
                            availability_detail(&overview.client).map(SharedString::from),
                            false,
                        )
                        .child(control_slot(ui::status_badge(
                            t,
                            client_label,
                            client_tone,
                        ))),
                    );
                } else {
                    card = card.child(
                        ui::setting_row(
                            t,
                            "Update status",
                            Some("Update status is not checked yet.".into()),
                            false,
                        )
                        .child(control_slot(ui::status_badge(
                            t,
                            "Not checked",
                            ui::Tone::Neutral,
                        ))),
                    );
                }
                card = card.child(
                ui::setting_row(
                    t,
                    "Check for updates",
                    Some(
                        "Updates are installed only after you download and verify the signed installer."
                            .into(),
                    ),
                    true,
                )
                .child(control_slot(
                    AuroraButton::new("check-updates", "Check now")
                        .size(ButtonSize::Small)
                        .icon("refresh")
                        .disabled(self.update_busy || no_host)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.check_updates(cx);
                        })),
                )),
            );
                if let Some(version) = self
                    .updates
                    .as_ref()
                    .and_then(|o| launcher_offer(&o.launcher.availability))
                {
                    let ready = self.downloaded_version.as_deref() == Some(&version);
                    let install = ready;
                    card = card.child(ui::setting_row(t, format!("Aurora Launcher {version}"), Some(if ready && self.install_blocked {
                        "Finish running games, sign-in and other operations before installing.".into()
                    } else { "Your instances, accounts and preferences stay in place.".into() }), true)
                        .child(control_slot(AuroraButton::new("launcher-update-action", if ready { "Exit and install" } else { "Download update" })
                            .variant(ButtonVariant::Primary)
                            .disabled(self.update_busy || no_host || (ready && self.install_blocked))
                            .on_click(cx.listener(move |this, _, _, cx| this.launcher_operation(install, cx))))));
                }
                if let Some(detail) = &self.update_detail {
                    card = card.child(div().pt(px(12.)).child(ui::caption(t, detail.clone())));
                }
                if let Some(error) = &self.updates_error {
                    card = card.child(card_error(t, error.clone()));
                }
                page = page.child(card);
            }
        }
        if self.section == 3 {
            // --- About ---------------------------------------------------------
            page = page.child(
                ui::card(t).child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .child(ui::icon_tile(t, Some("rocket_launch"), 40., true))
                        .child(ui::title_block(
                            t,
                            format!("Aurora Launcher {}", env!("CARGO_PKG_VERSION")),
                            Some(
                                format!(
                                    "Released under the {} license.",
                                    env!("CARGO_PKG_LICENSE")
                                )
                                .into(),
                            ),
                        )),
                ),
            );
        }

        page
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn appearance(motion: u8) -> api::Appearance {
        serde_json::from_value(serde_json::json!({
            "auroraMotionSpeed": motion,
            "background": "borealis",
            "theme": "oled",
            "accent": { "type": "custom", "hex": "#ffffff" },
            "palette": {
                "accent": "#8b80ff", "accentStrong": "#6f5df2", "accentHover": "#6f63e8",
                "accentPressed": "#5b4ad6", "accentContrast": "#ffffff",
                "accentSoft": "#8b80ff22", "accentOutline": "#8b80ff55"
            },
            "themes": [{ "id": "oled", "label": "OLED", "description": "Black surfaces." }],
            "accents": [{ "id": "blue", "label": "Blue", "hex": "#3b82f6" }]
        }))
        .unwrap()
    }

    fn request(motion: u8) -> AppearanceRequest {
        AppearanceRequest {
            revision: 0,
            theme: "oled".into(),
            accent: serde_json::json!({ "type": "custom", "hex": "#ffffff" }),
            background: "borealis".into(),
            motion,
        }
    }

    #[test]
    fn rapid_releases_keep_one_in_flight_and_only_the_latest_pending_snapshot() {
        let mut queue = AppearanceQueue::default();
        let first = queue
            .enqueue(request(50))
            .expect("first request dispatches");
        assert_eq!(first.motion, 50);

        let mut middle = request(37);
        middle.theme = "midnight".into();
        assert!(queue.enqueue(middle).is_none());
        let mut latest = request(90);
        latest.background = "simple".into();
        latest.accent = serde_json::json!({ "type": "preset", "id": "blue" });
        assert!(queue.enqueue(latest.clone()).is_none());
        latest.revision = 3;
        assert_eq!(queue.latest(), Some(latest.clone()));

        let (completed, pending) = queue.finish(first.revision).expect("ordered completion");
        assert_eq!(completed, first);
        assert_eq!(pending, Some(latest.clone()));
        assert_eq!(queue.in_flight, Some(latest.clone()));
        assert!(queue.pending.is_none());

        let mut during_follow_up = latest.clone();
        during_follow_up.motion = 100;
        assert!(queue.enqueue(during_follow_up).is_none());
        let (completed, follow_up) = queue.finish(latest.revision).unwrap();
        assert_eq!(completed, latest);
        let follow_up = follow_up.expect("latest release follows the active write");
        assert_eq!(follow_up.motion, 100);
        assert_eq!(follow_up.background, "simple");
        assert_eq!(follow_up.theme, "oled");
        assert!(queue.finish(follow_up.revision).is_some());
        assert!(queue.latest().is_none());
    }

    #[test]
    fn stale_or_duplicate_completion_cannot_finish_a_newer_request() {
        let mut queue = AppearanceQueue::default();
        let first = queue.enqueue(request(70)).unwrap();
        assert!(queue.enqueue(request(90)).is_none());
        assert!(queue.finish(first.revision + 1).is_none());
        assert_eq!(queue.latest().unwrap().motion, 90);
        assert!(queue.finish(first.revision).unwrap().1.unwrap().motion == 90);
    }

    #[test]
    fn a_write_never_builds_a_partial_request_from_malformed_state() {
        let mut state = AppearanceState::default();
        assert!(state.request().is_none());
        assert!(state.submit(request(50)).is_some());
        // Without an acknowledged snapshot the queued request is the only
        // truth; a second edit merges into it, it never invents fields.
        let pending = state.request().unwrap();
        assert_eq!(pending.motion, 50);
        assert_eq!(pending.background, "borealis");
    }

    #[test]
    fn acknowledged_response_with_pending_updates_only_the_baseline() {
        let mut state = AppearanceState::default();
        state.loaded(Ok(appearance(50)));
        let first = state.submit(request(70)).unwrap();
        let second = state.submit(request(90));
        assert!(second.is_none(), "edits during one save stay coalesced");

        let (completion, next) = state.complete(first.revision, Ok(appearance(70)));
        assert_eq!(completion, Completion::BaselineStored);
        assert!(state.is_busy(), "the pending write is still in flight");
        // The stale whole-appearance response is stored as the disk baseline,
        // and the already-coalesced pending request retains every newer field.
        assert_eq!(state.appearance().unwrap().aurora_motion_speed, 70);
        let next = next.unwrap();
        assert_eq!(next.motion, 90);

        let (completion, _) = state.complete(next.revision, Ok(appearance(90)));
        assert_eq!(completion, Completion::Applied);
        assert!(!state.is_busy());
        assert_eq!(state.appearance().unwrap().aurora_motion_speed, 90);
    }

    #[test]
    fn a_failed_write_restores_the_acknowledged_state_without_a_compensating_write() {
        let mut state = AppearanceState::default();
        state.loaded(Ok(appearance(50)));
        let first = state.submit(request(70)).unwrap();
        let failure = Err(api::ApiError::new("appearance_invalid", "Rejected."));
        let (completion, next) = state.complete(first.revision, failure);
        assert_eq!(completion, Completion::Failed);
        assert!(next.is_none());
        assert!(!state.is_busy());
        assert_eq!(state.appearance().unwrap().aurora_motion_speed, 50);
        assert_eq!(state.error(), Some("Rejected."));
        // The next write carries the latest complete acknowledged snapshot.
        assert_eq!(state.request().unwrap().motion, 50);
    }

    #[test]
    fn a_failed_write_with_pending_still_follows_through() {
        let mut state = AppearanceState::default();
        state.loaded(Ok(appearance(50)));
        let first = state.submit(request(70)).unwrap();
        state.submit(request(90));
        let (completion, next) = state.complete(
            first.revision,
            Err(api::ApiError::new("appearance_invalid", "Rejected.")),
        );
        assert_eq!(completion, Completion::Failed);
        assert!(state.is_busy());
        assert_eq!(next.unwrap().motion, 90);
    }

    #[test]
    fn a_malformed_read_leaves_no_snapshot_to_write() {
        let mut state = AppearanceState::default();
        state.loaded(Err(api::ApiError::new(
            "state_invalid",
            "Damaged configuration.",
        )));
        assert!(state.appearance().is_none());
        assert!(state.request().is_none());
        assert_eq!(state.error(), Some("Damaged configuration."));
    }

    #[test]
    fn reduce_motion_toggle_preserves_unrelated_fields_and_honors_malformed_state() {
        let mut state = AppearanceState::default();
        state.loaded(Ok(appearance(30)));
        assert_eq!(state.reduce_motion(), Some(false));

        // Enabling reduce motion changes only the motion field; theme,
        // accent and background come from the acknowledged snapshot.
        let mut request = state.request().expect("valid snapshot yields a request");
        request.motion = 0;
        let first = state.submit(request).unwrap();
        assert_eq!(first.motion, 0);
        assert_eq!(first.theme, "oled");
        assert_eq!(
            first.accent,
            serde_json::json!({ "type": "custom", "hex": "#ffffff" })
        );
        assert_eq!(first.background, "borealis");

        // A second toggle while the first write is in flight coalesces into
        // the pending snapshot, still carrying every unrelated field.
        let mut follow = state.request().unwrap();
        follow.motion = 50;
        assert!(state.submit(follow).is_none(), "writes stay coalesced");

        // A failed committed write never overwrites acknowledged state; the
        // queued toggle still follows through the existing queue handling.
        let failure = Err(api::ApiError::new("appearance_invalid", "Rejected."));
        let (completion, next) = state.complete(first.revision, failure);
        assert_eq!(completion, Completion::Failed);
        assert_eq!(state.appearance().unwrap().aurora_motion_speed, 30);
        let next = next.expect("the queued toggle follows a failed write");
        assert_eq!(next.motion, 50);
        assert_eq!(next.theme, "oled");
        assert_eq!(next.background, "borealis");

        // Malformed or unavailable state offers nothing to show or save.
        let mut broken = AppearanceState::default();
        broken.loaded(Err(api::ApiError::new("state_invalid", "Damaged.")));
        assert!(broken.reduce_motion().is_none());
        assert!(broken.request().is_none());
    }

    #[test]
    fn unknown_host_events_and_malformed_payloads_are_ignored() {
        let payload = serde_json::json!({
            "launcher": {"installedVersion": "1.5.1", "availability": {"kind": "updateAvailable", "candidate": "1.6.0"}},
            "client": {"kind": "upToDate"},
            "clientInstanceId": null,
            "clientInstanceName": null,
            "clientInstalledVersion": null
        });
        assert!(update_status_event("launch-state", &payload).is_none());
        assert!(update_status_event("update-status", &serde_json::Value::Null).is_none());
        assert!(update_status_event("update-status", &payload).is_some());
        assert!(launcher_progress_text(&serde_json::json!({})).is_none());
        assert_eq!(
            launcher_progress_text(
                &serde_json::json!({"downloadedBytes": 5242880, "totalBytes": 10485760})
            ),
            Some("Downloading: 5.0 / 10.0 MiB".into())
        );
    }

    #[test]
    fn hsv_round_trips_hex_colors() {
        for (r, g, b) in [
            (0x8b, 0x80, 0xff),
            (0, 0, 0),
            (255, 255, 255),
            (0x12, 0xa4, 0x55),
        ] {
            let (h, s, v) = rgb_to_hsv(r, g, b);
            assert_eq!(hsv_to_rgb(h, s, v), (r, g, b));
        }
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    let (h, s, v) = rgb_to_hsv(r, g, b);
                    assert!((0.0..360.0).contains(&h) && (0.0..=1.0).contains(&s));
                    assert!((0.0..=1.0).contains(&v));
                    assert_eq!(hsv_to_rgb(h, s, v), (r, g, b), "#{r:02x}{g:02x}{b:02x}");
                }
            }
        }
        // A hue of 360 wraps to red, like 0.
        assert_eq!(hsv_to_rgb(360.0, 1.0, 1.0), (255, 0, 0));
        assert_eq!(hex_text(0x8b, 0x80, 0xff), "#8b80ff");
    }

    #[test]
    fn hex_parsing_is_strict() {
        assert_eq!(parse_hex("#8B80ff"), Some((0x8b, 0x80, 0xff)));
        assert_eq!(parse_hex("  #8b80ff "), Some((0x8b, 0x80, 0xff)));
        for bad in [
            "8b80ff",
            "#8b80f",
            "#8b80fff",
            "#8b80fg",
            "",
            "#",
            "rgb(1,2,3)",
        ] {
            assert_eq!(parse_hex(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn picker_opens_on_the_saved_accent_and_tracks_hue_and_value() {
        let picker = AccentPickerState::open("#8b80ff");
        assert_eq!(picker.hex, "#8b80ff");
        assert_eq!(picker.rgb(), (0x8b, 0x80, 0xff));
        assert!(picker.valid);

        // A saved accent that is not #rrggbb falls back to the default violet.
        assert_eq!(AccentPickerState::open("violet").hex, "#8b80ff");

        let mut picker = picker;
        picker.set_hue(0.0);
        assert_eq!(
            picker.hex,
            hex_text(picker.rgb().0, picker.rgb().1, picker.rgb().2)
        );
        picker.set_hue(400.0);
        assert_eq!(picker.hue, 360.0, "hue clamps to the strip");
        picker.set_sv(2.0, -1.0);
        assert_eq!((picker.saturation, picker.value), (1.0, 0.0));
        assert_eq!(picker.hex, "#000000");
        picker.set_sv(1.0, 1.0);
        picker.set_hue(120.0);
        assert_eq!(picker.hex, "#00ff00");
        assert_eq!(picker.hue_rgb(), (0, 255, 0));
    }

    #[test]
    fn hex_edits_move_the_picker_and_invalid_text_is_flagged() {
        let mut picker = AccentPickerState::open("#8b80ff");
        // The typed text stays as typed; the color to preview is normalized.
        assert_eq!(picker.edit_hex("#12A455"), Some("#12a455".to_owned()));
        assert_eq!(picker.hex, "#12A455");
        let (h, s, v) = rgb_to_hsv(0x12, 0xa4, 0x55);
        assert_eq!((picker.hue, picker.saturation, picker.value), (h, s, v));

        assert_eq!(picker.edit_hex("#12"), None);
        assert!(!picker.valid);
        assert_eq!(picker.message, "Enter a color as #rrggbb.");
        assert_eq!(
            picker.apply(),
            None,
            "an unreadable field cannot be applied"
        );
    }

    #[test]
    fn apply_writes_only_a_validated_lowercase_color() {
        let mut picker = AccentPickerState::open("#8b80ff");
        picker.edit_hex("#12A455");
        picker.set_validity(Ok(()));
        assert!(picker.valid);
        assert_eq!(
            picker.message,
            "Previewing. Aurora derives readable button colors from it."
        );
        assert_eq!(picker.apply(), Some("#12a455".to_owned()));
        assert_eq!(
            custom_accent_value("#12A455"),
            serde_json::json!({ "type": "custom", "hex": "#12a455" })
        );

        picker.set_validity(Err("too dark"));
        assert!(!picker.valid);
        assert_eq!(picker.message, "Not usable: too dark.");
        assert_eq!(picker.apply(), None);
    }

    #[test]
    fn the_core_decides_which_custom_accents_are_usable() {
        let palette = custom_palette("#8b80ff").expect("the default violet is usable");
        assert_eq!(palette.accent, "#8b80ff");
        assert!(palette.accent_soft.starts_with("rgba("));
        let reason = custom_palette("#000000").expect_err("black is not visible");
        assert!(reason.contains("too dark"), "{reason}");
        assert!(custom_palette("nope").is_err());
    }

    #[test]
    fn motion_preview_is_a_draft_that_navigation_restores() {
        let mut state = AppearanceState::default();
        state.loaded(Ok(appearance(50)));
        assert_eq!(state.motion_speed(), Some(50));
        assert!(!state.motion_preview_active());
        assert_eq!(state.cancel_motion_preview(), None, "nothing to discard");

        assert_eq!(state.preview_motion(80), 80);
        assert_eq!(state.motion_speed(), Some(80));
        assert_eq!(state.reduce_motion(), Some(false));
        state.preview_motion(0);
        assert_eq!(
            state.reduce_motion(),
            Some(true),
            "preview 0 is reduced motion"
        );
        assert_eq!(state.preview_motion(250), 100, "out-of-range speeds clamp");

        // Cancelling restores the acknowledged value without any write.
        assert_eq!(state.cancel_motion_preview(), Some(50));
        assert_eq!(state.reduce_motion(), Some(false));
        assert!(!state.is_busy());
        assert_eq!(state.cancel_motion_preview(), None);

        // A released write that is still in flight is the value to restore.
        let first = state.stage(motion_edit(30)).unwrap().unwrap();
        state.preview_motion(90);
        assert_eq!(state.cancel_motion_preview(), Some(30));
        assert_eq!(state.motion_speed(), Some(30));
        let (_, next) = state.complete(first.revision, Ok(appearance(30)));
        assert!(next.is_none());
        assert_eq!(state.motion_speed(), Some(30));
    }

    #[test]
    fn an_acknowledged_write_never_overwrites_an_unreleased_draft() {
        let mut state = AppearanceState::default();
        state.loaded(Ok(appearance(50)));
        let first = state.stage(motion_edit(70)).unwrap().unwrap();
        state.preview_motion(10);
        let (completion, _) = state.complete(first.revision, Ok(appearance(70)));
        assert_eq!(completion, Completion::Applied);
        assert_eq!(state.appearance().unwrap().aurora_motion_speed, 70);
        assert_eq!(state.motion_speed(), Some(10), "the draft still wins");
        state.end_motion_preview();
        assert_eq!(state.motion_speed(), Some(70));

        // A failed read never leaves a draft behind with nothing to write to.
        state.preview_motion(5);
        state.loaded(Err(api::ApiError::new("state_invalid", "Damaged.")));
        assert_eq!(state.motion_speed(), None);
        assert!(!state.motion_preview_active());
    }

    #[test]
    fn every_new_write_carries_the_complete_snapshot() {
        let mut state = AppearanceState::default();
        state.loaded(Ok(appearance(30)));

        let first = state.stage(motion_edit(70)).unwrap().unwrap();
        assert_eq!(first.motion, 70);
        assert_eq!(first.theme, "oled");
        assert_eq!(first.background, "borealis");
        assert_eq!(
            first.accent,
            serde_json::json!({ "type": "custom", "hex": "#ffffff" })
        );

        // Later edits during the write coalesce and still carry every field.
        assert!(state.stage(background_edit("simple")).unwrap().is_none());
        let pending = state.request().unwrap();
        assert_eq!(pending.motion, 70, "the released speed survives");
        assert_eq!(pending.background, "simple");
        assert_eq!(pending.theme, "oled");

        assert!(
            state
                .stage(accent_edit(custom_accent_value("#12A455")))
                .unwrap()
                .is_none()
        );
        assert!(
            state
                .stage(theme_edit("midnight".into()))
                .unwrap()
                .is_none()
        );
        let latest = state.request().unwrap();
        assert_eq!(latest.motion, 70);
        assert_eq!(latest.background, "simple");
        assert_eq!(latest.theme, "midnight");
        assert_eq!(
            latest.accent,
            serde_json::json!({ "type": "custom", "hex": "#12a455" })
        );

        let (completion, next) = state.complete(first.revision, Ok(appearance(70)));
        assert_eq!(completion, Completion::BaselineStored);
        let next = next.expect("the coalesced snapshot follows");
        assert_eq!(
            (next.motion, next.background.as_str(), next.theme.as_str()),
            (70, "simple", "midnight")
        );
        assert_eq!(next.accent["hex"], "#12a455");

        // Without a valid snapshot nothing is staged, so nothing partial is
        // ever written over malformed state.
        assert!(AppearanceState::default().stage(motion_edit(1)).is_none());
        assert!(
            AppearanceState::default()
                .stage(background_edit("simple"))
                .is_none()
        );
    }

    #[test]
    fn reduce_motion_follows_the_slider_with_slint_parity() {
        assert_eq!(reduce_motion_speed(true), 0);
        assert_eq!(reduce_motion_speed(false), 50);
        let mut state = AppearanceState::default();
        state.loaded(Ok(appearance(0)));
        assert_eq!(state.reduce_motion(), Some(true));
        // Toggling off replaces any draft with a write of the default speed.
        state.preview_motion(40);
        state.end_motion_preview();
        let request = state
            .stage(motion_edit(reduce_motion_speed(false)))
            .unwrap()
            .unwrap();
        assert_eq!(request.motion, 50);
        assert_eq!(state.reduce_motion(), Some(false));
    }

    #[test]
    fn background_and_motion_wording_matches_the_slint_host() {
        assert_eq!(BACKGROUND_LABELS.len(), borealis::BACKGROUNDS.len());
        assert_eq!(BACKGROUND_LABELS, ["Simple", "Borealis"]);
        assert_eq!(
            BACKGROUND_DESCRIPTIONS,
            [
                "Plain surfaces in the selected theme.",
                "Soft, stylized aurora bands behind translucent surfaces."
            ]
        );
        assert_eq!(
            motion_detail(true),
            "Borealis drift speed. Set to 0 for a still background."
        );
        assert_eq!(
            motion_detail(false),
            "Borealis drift speed. Set to 0 for a still background. Software rendering keeps Borealis static."
        );
        assert_eq!(normalize_motion(-5.0), 0);
        assert_eq!(normalize_motion(37.5), 38);
        assert_eq!(normalize_motion(100.4), 100);
        assert_eq!(normalize_motion(250.0), 100);
    }

    #[test]
    fn pointer_positions_clamp_to_the_track() {
        assert_eq!(track_fraction(-5.0, 100.0), 0.0);
        assert_eq!(track_fraction(250.0, 100.0), 1.0);
        assert_eq!(track_fraction(25.0, 100.0), 0.25);
        assert_eq!(
            track_fraction(5.0, 0.0),
            1.0,
            "an empty track never divides by zero"
        );
    }
    #[test]
    fn launcher_actions_require_an_exact_available_offer() {
        assert_eq!(
            launcher_offer(&serde_json::json!({"kind":"updateAvailable","candidate":"2.3.0"}))
                .as_deref(),
            Some("2.3.0")
        );
        for state in [
            serde_json::json!({"kind":"upToDate","candidate":"2.3.0"}),
            serde_json::json!({"kind":"updateAvailable"}),
            serde_json::json!({"kind":"updateAvailable","candidate":""}),
        ] {
            assert!(launcher_offer(&state).is_none());
        }
    }
}
