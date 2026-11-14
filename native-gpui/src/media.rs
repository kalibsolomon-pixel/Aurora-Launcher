//! The GPUI Media widget's system integration: Windows Global System Media
//! Transport Controls (the sessions behind the taskbar media flyout).
//!
//! Behavior authority is the preserved Slint host's `native/src/media.rs`:
//! an event-driven worker subscribes to session, media-property and playback
//! changes and refreshes only when Windows reports one; sessions can be
//! cycled; playback capability flags, previous/next/play-pause, bounded
//! sanitized metadata and bounded thumbnail decoding all follow the same
//! rules (≤2 MiB thumbnail stream, ≤4096 source dimensions, ≤96 px decoded
//! RGBA, no volume, no browser injection).
//!
//! GPUI-specific lifetime rules:
//! - The entity starts inactive. `set_active(true)` starts exactly one
//!   worker; `set_active(false)` and entity drop revoke the generation, send
//!   `Stop`, and the worker unregisters every manager/session handler before
//!   exiting. Handler closures only ever hold OS-side channel senders, so no
//!   callback sender cycles back into the entity.
//! - `Host` is only a capability gate here. `None` (diagnostic window) is a
//!   strict no-start gate: no OS media manager, session or worker call is
//!   ever made, not even on `set_active(true)`. No launcher, auth or network
//!   API is called in any mode.
//! - Worker replies travel through a bounded, awaited GPUI channel task and
//!   carry their worker generation; replies from a disabled or replaced
//!   worker are rejected, so old data can never revive a stopped session.
//! - There is no polling, no timer, no animation and no work in `Render`;
//!   refresh bursts are coalesced and only user activation sends commands.
//!   The worker rechecks playback capability and exact session identity
//!   before executing any command.
//! - Terminal initialization/spawn/panic failures land in an honest `Retry`
//!   state instead of a stuck loading indicator.
//! - Artwork is converted with `images::rgba_image` exactly once per actual
//!   artwork change and the `RenderImage` is retained across renders.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use aurora_launcher_lib::host::Host;
use gpui::base::Button;
use gpui::{Context, FontWeight, Hsla, Render, RenderImage, Window, div, img, prelude::*, px};
use tokio::runtime::Handle;

use crate::controls::{AuroraButton, ButtonSize};
use crate::images;
use crate::theme::theme;

/// Replies the UI awaits per worker; surplus snapshots are coalesced away.
const REPLY_QUEUE: usize = 16;

// ---------------------------------------------------------------------------
// Worker protocol (OS-agnostic shapes; the Windows worker fills them)
// ---------------------------------------------------------------------------

/// A transport command. Only ever produced by user activation or an explicit
/// refresh, never by a timer.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MediaCommand {
    Refresh,
    Toggle,
    Next,
    Previous,
    Cycle,
    Stop,
    Targeted {
        action: TransportAction,
        session: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportAction {
    Toggle,
    Next,
    Previous,
    Cycle,
}

impl MediaCommand {
    fn targeted(self, session: String) -> Option<Self> {
        let action = match self {
            Self::Toggle => TransportAction::Toggle,
            Self::Next => TransportAction::Next,
            Self::Previous => TransportAction::Previous,
            Self::Cycle => TransportAction::Cycle,
            _ => return None,
        };
        (!session.is_empty()).then_some(Self::Targeted { action, session })
    }

    fn unpack(self) -> (Self, Option<String>) {
        match self {
            Self::Targeted { action, session } => (
                match action {
                    TransportAction::Toggle => Self::Toggle,
                    TransportAction::Next => Self::Next,
                    TransportAction::Previous => Self::Previous,
                    TransportAction::Cycle => Self::Cycle,
                },
                Some(session),
            ),
            other => (other, None),
        }
    }
}

/// Raw decoded artwork as read by the worker, before GPUI conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ArtRaw {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
}

/// What the worker last read from Windows.
#[derive(Debug, Default)]
struct Snapshot {
    session_id: String,
    active: bool,
    title: String,
    artist: String,
    source: String,
    playing: bool,
    can_toggle: bool,
    can_next: bool,
    can_previous: bool,
    sessions: usize,
    art: Option<ArtRaw>,
}

/// A worker completion. The generation identifies the worker that produced
/// it; a disabled or replaced worker's replies are dropped by the core.
enum WorkerReply {
    Snapshot(u64, Snapshot),
    Failed(u64, String),
}
impl WorkerReply {
    fn generation(&self) -> u64 {
        match self {
            WorkerReply::Snapshot(generation, _) | WorkerReply::Failed(generation, _) => {
                *generation
            }
        }
    }
}

/// "Spotify.exe" / "Chrome" / "308046B0AF4A39CB" → a short source label.
fn source_label(app_id: &str) -> String {
    let base = app_id.rsplit(['\\', '!']).next().unwrap_or(app_id);
    let base = base.strip_suffix(".exe").unwrap_or(base);
    let known = [
        ("spotify", "Spotify"),
        ("chrome", "Google Chrome"),
        ("msedge", "Microsoft Edge"),
        ("firefox", "Firefox"),
        ("opera", "Opera"),
        ("brave", "Brave"),
        ("vlc", "VLC"),
        ("zunemusic", "Media Player"),
    ];
    let lower = app_id.to_ascii_lowercase();
    for (needle, label) in known {
        if lower.contains(needle) {
            return label.into();
        }
    }
    if base.len() > 32 || base.chars().all(|c| c.is_ascii_hexdigit()) {
        "Media app".into()
    } else {
        base.chars().filter(|c| !c.is_control()).collect()
    }
}

/// Session metadata is sanitized: control characters are dropped and the
/// text is capped, exactly like the preserved host.
fn sanitized(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(160).collect()
}

/// Collapses a leading burst of `Refresh` notifications into whichever
/// command follows (or the last `Refresh`), ported from the preserved
/// host's worker drain loop. `first` is the already-received command;
/// `queued` holds the burst that arrived behind it.
fn coalesce_burst(first: MediaCommand, queued: &mut VecDeque<MediaCommand>) -> MediaCommand {
    let mut command = first;
    while command == MediaCommand::Refresh {
        match queued.pop_front() {
            Some(next) => command = next,
            None => break,
        }
    }
    command
}

// ---------------------------------------------------------------------------
// Pure view state (no GPUI, no OS; fixture-testable without sessions)
// ---------------------------------------------------------------------------

/// One retained artwork conversion: the raw identity of the last accepted
/// art plus the GPUI image that was built from it exactly once.
#[derive(Default)]
struct MediaView {
    session_id: String,
    active: bool,
    title: String,
    artist: String,
    source: String,
    playing: bool,
    can_toggle: bool,
    can_next: bool,
    can_previous: bool,
    sessions: usize,
    art: Option<Arc<RenderImage>>,
    art_raw: Option<ArtRaw>,
}
impl MediaView {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn apply(&mut self, snapshot: &Snapshot) {
        self.session_id = snapshot.session_id.clone();
        self.active = snapshot.active;
        self.title = snapshot.title.clone();
        self.artist = snapshot.artist.clone();
        self.source = snapshot.source.clone();
        self.playing = snapshot.playing;
        self.can_toggle = snapshot.can_toggle;
        self.can_next = snapshot.can_next;
        self.can_previous = snapshot.can_previous;
        self.sessions = snapshot.sessions;
        self.set_art(&snapshot.art);
    }

    /// Converts artwork with `images::rgba_image` only when it actually
    /// changed; unchanged art keeps the retained `RenderImage` and malformed
    /// art (bad dimensions or pixel counts) is dropped, never faked.
    fn set_art(&mut self, art: &Option<ArtRaw>) {
        match art {
            None => {
                self.art = None;
                self.art_raw = None;
            }
            Some(raw) => {
                if self.art_raw.as_ref().is_some_and(|last| last == raw) {
                    return;
                }
                self.art = images::rgba_image(raw.width, raw.height, raw.bytes.clone());
                self.art_raw = Some(raw.clone());
            }
        }
    }
}

/// The widget's lifecycle phase as shown to the user.
#[derive(Debug, Clone, PartialEq)]
enum Phase {
    /// Present but not active: the worker is stopped and the view is cleared.
    Inactive,
    /// No launcher host: strict diagnostic gate, nothing is ever started.
    Diagnostics,
    /// This platform exposes no OS media sessions; nothing is started.
    Unsupported,
    /// A worker was started and its first snapshot is still pending.
    Loading,
    /// The latest snapshot is applied.
    Live,
    /// The worker failed terminally; an explicit Retry restarts it.
    Failed(String),
}

/// What [Media]'s activation logic asks its owner to do with the OS worker.
enum CoreOp {
    /// Start exactly one worker for this generation.
    Start { generation: u64 },
    /// The current generation is revoked; stop the worker and make it
    /// unregister every manager/session handler.
    Stop,
}

/// What an explicit refresh asks for.
enum RefreshOp {
    /// Ask the live worker to re-read now.
    Send { command: MediaCommand },
    /// The previous worker failed terminally; start a fresh generation.
    Restart { generation: u64 },
}

/// Pure media lifecycle state machine: activation gating, generation
/// guards, reply acceptance and command guards, independent of GPUI and of
/// any real Windows session.
struct MediaCore {
    host: bool,
    supported: bool,
    active: bool,
    generation: u64,
    phase: Phase,
    view: MediaView,
}
impl MediaCore {
    fn new(host: bool, supported: bool) -> Self {
        Self {
            host,
            supported,
            active: false,
            generation: 0,
            phase: Phase::Inactive,
            view: MediaView::default(),
        }
    }

    /// Activates or deactivates the widget. Every transition revokes the
    /// current generation first, so in-flight replies and queued controls
    /// from the old worker can never apply afterwards.
    fn set_active(&mut self, active: bool) -> Option<CoreOp> {
        if self.active == active {
            return None;
        }
        self.active = active;
        self.generation = self.generation.wrapping_add(1);
        self.view.clear();
        if !active {
            self.phase = Phase::Inactive;
            return Some(CoreOp::Stop);
        }
        // Strict diagnostic gate: without a host nothing is ever started.
        if !self.host {
            self.phase = Phase::Diagnostics;
            return None;
        }
        if !self.supported {
            self.phase = Phase::Unsupported;
            return None;
        }
        self.phase = Phase::Loading;
        Some(CoreOp::Start {
            generation: self.generation,
        })
    }

    /// Applies a worker reply. Replies from a disabled or replaced worker
    /// (stale generation, or inactive widget) are rejected. Returns whether
    /// visible state changed.
    fn accept(&mut self, reply: WorkerReply) -> bool {
        if reply.generation() != self.generation || !self.active {
            return false;
        }
        match reply {
            WorkerReply::Snapshot(_, snapshot) => {
                self.phase = Phase::Live;
                self.view.apply(&snapshot);
            }
            WorkerReply::Failed(_, message) => {
                self.phase = Phase::Failed(message);
                self.view.clear();
            }
        }
        true
    }

    /// Command guard: transport commands are only meaningful while a live
    /// session is actually shown. The worker rechecks capability and exact
    /// session identity again before executing anything.
    fn command(&mut self, command: MediaCommand) -> Option<MediaCommand> {
        let permitted = match &command {
            MediaCommand::Toggle => self.view.can_toggle,
            MediaCommand::Next => self.view.can_next,
            MediaCommand::Previous => self.view.can_previous,
            MediaCommand::Cycle => self.view.sessions > 1,
            _ => false,
        };
        (self.active && matches!(self.phase, Phase::Live) && self.view.active && permitted)
            .then_some(command)
    }

    /// Explicit Retry/Refresh. A terminally failed worker restarts under a
    /// fresh generation; a live worker is asked to re-read; a still-loading
    /// worker already owes a snapshot, so nothing new is enqueued.
    fn refresh(&mut self) -> Option<RefreshOp> {
        if !self.active || !self.host || !self.supported {
            return None;
        }
        match self.phase {
            Phase::Failed(_) => {
                self.generation = self.generation.wrapping_add(1);
                self.phase = Phase::Loading;
                self.view.clear();
                Some(RefreshOp::Restart {
                    generation: self.generation,
                })
            }
            Phase::Live => Some(RefreshOp::Send {
                command: MediaCommand::Refresh,
            }),
            Phase::Loading | Phase::Inactive | Phase::Diagnostics | Phase::Unsupported => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// The retained handle to one OS worker generation.
struct WorkerHandle {
    generation: u64,
    commands: std::sync::mpsc::Sender<MediaCommand>,
    revoked: Arc<AtomicBool>,
}

/// Whether OS media sessions exist on this platform.
pub fn supported() -> bool {
    cfg!(windows)
}

/// The Media Home widget view. Rendered inside a parent Home card; this
/// module renders only the compact contents (art, metadata, transport),
/// never a card heading or panel chrome.
pub struct Media {
    /// Capability gate only: never used to call launcher, auth or network
    /// APIs from this module.
    host: Option<Host>,
    /// Kept for interface parity with the other Home entities; the media
    /// worker owns its own thread and never uses this runtime.
    runtime: Handle,
    core: MediaCore,
    worker: Option<WorkerHandle>,
    replies: async_channel::Sender<WorkerReply>,
}
impl Media {
    /// Builds the widget inactive. Nothing is started, read or rendered
    /// beyond the inactive state until `set_active(true)`.
    pub fn new(
        host: Option<Host>,
        runtime: Handle,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (replies, rx) = async_channel::bounded::<WorkerReply>(REPLY_QUEUE);
        // The single awaited delivery task for every worker generation. It
        // ends when the worker side closes or this entity is dropped; there
        // is no polling and no idle redraw.
        cx.spawn(async move |weak, cx| {
            while let Ok(reply) = rx.recv().await {
                if weak
                    .update(cx, |this: &mut Self, cx| {
                        if this.core.accept(reply) {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            core: MediaCore::new(host.is_some(), supported()),
            host,
            runtime,
            worker: None,
            replies,
        }
    }

    /// Matches the worker to whether the widget is shown. `true` starts one
    /// worker; `false` revokes the generation and stops the worker, which
    /// unregisters every manager/session handler before exiting.
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.core.active == active {
            return;
        }
        match self.core.set_active(active) {
            Some(CoreOp::Start { generation }) => {
                self.revoke_worker();
                self.spawn_worker(generation);
            }
            Some(CoreOp::Stop) => self.revoke_worker(),
            None => {}
        }
        cx.notify();
    }

    /// Explicit Refresh/Retry: re-reads through the live worker, or
    /// restarts a terminally failed one under a fresh generation.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        match self.core.refresh() {
            Some(RefreshOp::Send { command }) => {
                if let Some(worker) = &self.worker {
                    let _ = worker.commands.send(command);
                }
            }
            Some(RefreshOp::Restart { generation }) => {
                self.revoke_worker();
                self.spawn_worker(generation);
            }
            None => {}
        }
        cx.notify();
    }

    /// Sends one transport command. Called only from user activation; the
    /// guard here keeps commands from ever leaving a non-live widget, and
    /// the worker rechecks capability and identity before executing.
    fn send_command(&mut self, command: MediaCommand, cx: &mut Context<Self>) {
        if let Some(command) = self
            .core
            .command(command)
            .and_then(|command| command.targeted(self.core.view.session_id.clone()))
        {
            if let Some(worker) = &self.worker {
                let _ = worker.commands.send(command);
            }
        }
        cx.notify();
    }

    fn spawn_worker(&mut self, generation: u64) {
        let revoked = Arc::new(AtomicBool::new(false));
        match imp::start(generation, self.replies.clone(), revoked.clone()) {
            Some(commands) => {
                self.worker = Some(WorkerHandle {
                    generation,
                    commands,
                    revoked,
                })
            }
            // Thread-spawn failure is a terminal, retryable state — never a
            // stuck loading indicator.
            None => {
                self.core.accept(WorkerReply::Failed(
                    generation,
                    "The media session worker could not start. Retry to reconnect.".into(),
                ));
            }
        }
    }

    fn revoke_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.revoked.store(true, Ordering::Release);
            let _ = worker.commands.send(MediaCommand::Stop);
        }
    }
}
impl Drop for Media {
    fn drop(&mut self) {
        self.revoke_worker();
    }
}

impl Render for Media {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme(cx).clone();
        let core = &self.core;
        let entity = cx.entity();
        // A centred, quiet message with a leading glyph.
        let note = |text: String, color: Hsla| {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(crate::symbols::icon(
                    Some("music_note"),
                    px(22.),
                    palette.subtle_foreground,
                ))
                .child(
                    div()
                        .max_w(px(300.))
                        .text_size(px(12.))
                        .text_color(color)
                        .text_center()
                        .child(text),
                )
        };

        let mut root = div()
            .flex()
            .flex_col()
            .flex_1()
            .justify_center()
            .gap(px(12.))
            .min_w_0();
        match &core.phase {
            Phase::Diagnostics => {
                root = root.child(note(
                    "Media controls are unavailable in this diagnostic window: there is no launcher host, so nothing is read or changed."
                        .into(),
                    palette.muted_foreground,
                ));
            }
            Phase::Unsupported => {
                root = root.child(note(
                    "Media controls are available on Windows.".into(),
                    palette.muted_foreground,
                ));
            }
            Phase::Inactive => {
                root = root.child(note("Nothing is playing.".into(), palette.muted_foreground));
            }
            Phase::Loading => {
                root = root.child(note(
                    "Connecting to system media…".into(),
                    palette.muted_foreground,
                ));
            }
            Phase::Failed(message) => {
                root = root.child(note(message.clone(), palette.danger)).child(
                    div().flex().justify_center().child(
                        AuroraButton::new("media-retry", "Retry")
                            .size(ButtonSize::Small)
                            .icon("refresh")
                            .on_click({
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    entity.update(cx, |this, cx| this.refresh(cx));
                                }
                            }),
                    ),
                );
            }
            Phase::Live if !core.view.active => {
                root = root.child(note("Nothing is playing.".into(), palette.muted_foreground));
            }
            Phase::Live => {
                let view = &core.view;
                let art = view.art.clone();
                let command = |command: MediaCommand| {
                    let entity = entity.clone();
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                        entity.update(cx, |this, cx| this.send_command(command.clone(), cx));
                    }
                };
                root = root
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(14.))
                            .min_w_0()
                            .child(match art {
                                Some(art) => div()
                                    .flex_shrink_0()
                                    .size(px(56.))
                                    .rounded(px(10.))
                                    .overflow_hidden()
                                    .border_1()
                                    .border_color(palette.border_subtle)
                                    .child(img(art).size(px(54.))),
                                None => div()
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .justify_center()
                                    .size(px(56.))
                                    .rounded(px(10.))
                                    .bg(palette.control)
                                    .border_1()
                                    .border_color(palette.border_subtle)
                                    .child(crate::symbols::icon(
                                        Some("music_note"),
                                        px(26.),
                                        palette.muted_foreground,
                                    )),
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(palette.foreground)
                                            .child(view.title.clone()),
                                    )
                                    .when(!view.artist.is_empty(), |col| {
                                        col.child(
                                            div()
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .whitespace_nowrap()
                                                .text_size(px(12.))
                                                .text_color(palette.muted_foreground)
                                                .child(view.artist.clone()),
                                        )
                                    })
                                    .when(!view.source.is_empty(), |col| {
                                        col.child(
                                            div()
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .whitespace_nowrap()
                                                .text_size(px(11.))
                                                .text_color(palette.subtle_foreground)
                                                .child(view.source.clone()),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .child(
                                transport_button(
                                    &palette,
                                    "media-previous",
                                    "skip_previous",
                                    "Previous track",
                                    !view.can_previous,
                                    false,
                                )
                                .on_click(command(MediaCommand::Previous)),
                            )
                            .child(
                                transport_button(
                                    &palette,
                                    "media-toggle",
                                    if view.playing { "pause" } else { "play_arrow" },
                                    if view.playing { "Pause" } else { "Play" },
                                    !view.can_toggle,
                                    true,
                                )
                                .on_click(command(MediaCommand::Toggle)),
                            )
                            .child(
                                transport_button(
                                    &palette,
                                    "media-next",
                                    "skip_next",
                                    "Next track",
                                    !view.can_next,
                                    false,
                                )
                                .on_click(command(MediaCommand::Next)),
                            )
                            // Session count decides whether switching players
                            // is offered at all; the worker rechecks.
                            .when(view.sessions > 1, |row| {
                                row.child(
                                    AuroraButton::new("media-cycle", "Next player")
                                        .size(ButtonSize::Small)
                                        .icon("sync")
                                        .on_click(command(MediaCommand::Cycle)),
                                )
                            }),
                    );
            }
        }
        root
    }
}

/// A round transport control: icon only, with the action as its accessible
/// name. The main action (play / pause) is the accent-filled one.
fn transport_button(
    t: &crate::theme::AuroraTheme,
    id: &'static str,
    icon: &'static str,
    label: &'static str,
    disabled: bool,
    main: bool,
) -> Button {
    let (bg, hover, fg) = if main {
        (t.accent, t.accent_hover, t.accent_foreground)
    } else {
        (t.control, t.control_hover, t.foreground)
    };
    let size = if main { 40. } else { 34. };
    Button::new(id)
        .aria_label(label)
        .disabled(disabled)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
        .bg(bg)
        .hover(move |style| style.bg(hover))
        .styles(|styles| styles.disabled(|style| style.opacity(0.42)))
        .child(crate::symbols::icon(
            Some(icon),
            px(if main { 22. } else { 18. }),
            fg,
        ))
}

// ---------------------------------------------------------------------------
// Windows worker (event-driven; ported from native/src/media.rs)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use std::collections::VecDeque;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::mpsc::{self, Receiver, Sender};

    use windows::Foundation::TypedEventHandler;
    use windows::Graphics::Imaging::{
        BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat,
        BitmapTransform, ColorManagementMode, ExifOrientationMode,
    };
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };

    use super::{
        Arc, ArtRaw, AtomicBool, MediaCommand, Ordering, Snapshot, WorkerReply, coalesce_burst,
        sanitized, source_label,
    };

    const MAX_ART_BYTES: u64 = 2 * 1024 * 1024;
    const ART_SIZE: u32 = 96;
    /// How many queued notifications are drained per wake for coalescing;
    /// anything beyond stays in the OS channel and is coalesced next wake.
    const DRAIN_CAP: usize = 64;

    /// Starts the worker thread for one generation. Returns the command
    /// sender the entity retains; `None` when the thread could not spawn.
    pub(super) fn start(
        generation: u64,
        replies: async_channel::Sender<WorkerReply>,
        revoked: Arc<AtomicBool>,
    ) -> Option<Sender<MediaCommand>> {
        let (sender, receiver) = mpsc::channel::<MediaCommand>();
        let events = sender.clone();
        std::thread::Builder::new()
            .name("aurora-media-gpui".into())
            .spawn(move || run(events, receiver, generation, replies, revoked))
            .ok()?;
        let _ = sender.send(MediaCommand::Refresh);
        Some(sender)
    }

    struct Subscription {
        session: Session,
        properties: i64,
        playback: i64,
    }

    impl Drop for Subscription {
        fn drop(&mut self) {
            let _ = self.session.RemoveMediaPropertiesChanged(self.properties);
            let _ = self.session.RemovePlaybackInfoChanged(self.playback);
        }
    }

    struct ManagerSubscriptions {
        manager: Manager,
        sessions: Option<i64>,
        current: Option<i64>,
    }
    impl Drop for ManagerSubscriptions {
        fn drop(&mut self) {
            if let Some(token) = self.sessions {
                let _ = self.manager.RemoveSessionsChanged(token);
            }
            if let Some(token) = self.current {
                let _ = self.manager.RemoveCurrentSessionChanged(token);
            }
        }
    }

    /// A panic inside the worker is converted into a terminal failure reply
    /// (honest Retry state) instead of a silently dead thread.
    fn run(
        events: Sender<MediaCommand>,
        commands: Receiver<MediaCommand>,
        generation: u64,
        replies: async_channel::Sender<WorkerReply>,
        revoked: Arc<AtomicBool>,
    ) {
        if catch_unwind(AssertUnwindSafe(|| {
            worker(events, commands, &replies, generation, &revoked)
        }))
        .is_err()
        {
            let _ = replies.force_send(WorkerReply::Failed(
                generation,
                "The media session worker stopped unexpectedly. Retry to reconnect.".into(),
            ));
        }
    }

    /// Delivers one reply on the bounded UI channel. A full queue coalesces
    /// the oldest queued reply away so the latest state is retained; a closed queue means
    /// the view is gone and the worker must stop.
    fn send(replies: &async_channel::Sender<WorkerReply>, reply: WorkerReply) -> bool {
        replies.force_send(reply).is_ok()
    }

    fn fail(replies: &async_channel::Sender<WorkerReply>, generation: u64, message: &str) {
        let _ = replies.force_send(WorkerReply::Failed(generation, message.into()));
    }

    fn worker(
        events: Sender<MediaCommand>,
        commands: Receiver<MediaCommand>,
        replies: &async_channel::Sender<WorkerReply>,
        generation: u64,
        revoked: &AtomicBool,
    ) {
        use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
        if revoked.load(Ordering::Acquire) {
            return;
        }
        if unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_err() {
            fail(
                replies,
                generation,
                "System media services could not initialize on this system.",
            );
            return;
        }
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe {
                    RoUninitialize();
                }
            }
        }
        let _apartment = Apartment;
        let Ok(manager) = Manager::RequestAsync().and_then(|op| op.get()) else {
            fail(
                replies,
                generation,
                "Media sessions are unavailable on this system.",
            );
            return;
        };
        if revoked.load(Ordering::Acquire) {
            return;
        }
        let notify = events.clone();
        let sessions_token = manager
            .SessionsChanged(&TypedEventHandler::new(move |_, _| {
                let _ = notify.send(MediaCommand::Refresh);
                Ok(())
            }))
            .ok();
        let notify = events.clone();
        let current_token = manager
            .CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
                let _ = notify.send(MediaCommand::Refresh);
                Ok(())
            }))
            .ok();
        let _manager_subscriptions = ManagerSubscriptions {
            manager: manager.clone(),
            sessions: sessions_token,
            current: current_token,
        };
        let mut chosen: Option<String> = None;
        let mut displayed: Option<Session> = None;
        let mut session_serial = 0u64;
        let mut session_id = String::new();
        let mut subscription: Option<Subscription> = None;
        let mut queued: VecDeque<MediaCommand> = VecDeque::new();
        loop {
            if revoked.load(Ordering::Acquire) {
                break;
            }
            let first = match queued.pop_front() {
                Some(command) => command,
                None => match commands.recv() {
                    // The entity dropped its sender or sent Stop: shut down.
                    Ok(command) => command,
                    Err(_) => break,
                },
            };
            while queued.len() < DRAIN_CAP {
                match commands.try_recv() {
                    Ok(command) => queued.push_back(command),
                    Err(_) => break,
                }
            }
            // Coalesce bursts of change notifications into one read.
            let (mut command, expected) = coalesce_burst(first, &mut queued).unpack();
            if command == MediaCommand::Stop || revoked.load(Ordering::Acquire) {
                break;
            }
            let sessions = all_sessions(&manager);
            let current = chosen
                .as_ref()
                .and_then(|id| {
                    sessions
                        .iter()
                        .find(|session| app_id(session) == *id)
                        .cloned()
                })
                .or_else(|| manager.GetCurrentSession().ok())
                .or_else(|| sessions.first().cloned());
            if expected
                .as_ref()
                .is_some_and(|id| *id != session_id || current != displayed)
            {
                command = MediaCommand::Refresh;
            }
            if command == MediaCommand::Cycle && sessions.len() > 1 {
                let ids: Vec<String> = sessions.iter().map(app_id).collect();
                let current_id = chosen
                    .clone()
                    .or_else(|| manager.GetCurrentSession().ok().map(|s| app_id(&s)));
                let current = current_id
                    .as_ref()
                    .and_then(|c| ids.iter().position(|id| id == c))
                    .unwrap_or(0);
                chosen = Some(ids[(current + 1) % ids.len()].clone());
            }
            // Commands are re-resolved against the current session list, so
            // an identity that disappeared is never commanded.
            let session = chosen
                .as_ref()
                .and_then(|c| sessions.iter().find(|s| &app_id(s) == c).cloned())
                .or_else(|| manager.GetCurrentSession().ok())
                .or_else(|| sessions.first().cloned());
            if revoked.load(Ordering::Acquire) {
                break;
            }
            if let Some(session) = &session {
                match command {
                    MediaCommand::Toggle => {
                        // Capability is rechecked here, immediately before
                        // the command, exactly like the preserved host.
                        if let Ok(info) = session.GetPlaybackInfo() {
                            if let Ok(controls) = info.Controls() {
                                if controls.IsPlayPauseToggleEnabled().unwrap_or(false) {
                                    let _ =
                                        session.TryTogglePlayPauseAsync().and_then(|op| op.get());
                                } else if info.PlaybackStatus().is_ok_and(|s| s == Status::Playing)
                                    && controls.IsPauseEnabled().unwrap_or(false)
                                {
                                    let _ = session.TryPauseAsync().and_then(|op| op.get());
                                } else if controls.IsPlayEnabled().unwrap_or(false) {
                                    let _ = session.TryPlayAsync().and_then(|op| op.get());
                                }
                            }
                        }
                    }
                    MediaCommand::Next => {
                        if session
                            .GetPlaybackInfo()
                            .and_then(|info| info.Controls())
                            .and_then(|controls| controls.IsNextEnabled())
                            .unwrap_or(false)
                            && !revoked.load(Ordering::Acquire)
                        {
                            let _ = session.TrySkipNextAsync().and_then(|op| op.get());
                        }
                    }
                    MediaCommand::Previous => {
                        if session
                            .GetPlaybackInfo()
                            .and_then(|info| info.Controls())
                            .and_then(|controls| controls.IsPreviousEnabled())
                            .unwrap_or(false)
                            && !revoked.load(Ordering::Acquire)
                        {
                            let _ = session.TrySkipPreviousAsync().and_then(|op| op.get());
                        }
                    }
                    _ => {}
                }
            }
            // Follow change events for the session being shown only.
            let same = match (&subscription, &session) {
                (Some(current), Some(session)) => current.session == *session,
                (None, None) => true,
                _ => false,
            };
            if !same {
                unsubscribe(subscription.take());
                subscription = session.as_ref().and_then(|s| subscribe(s, &events));
            }
            if displayed != session {
                session_serial = session_serial.wrapping_add(1);
                session_id = format!("{generation}:{session_serial}");
                displayed = session.clone();
            }
            let mut snapshot = session
                .as_ref()
                .map(|s| read(s, sessions.len()))
                .unwrap_or_default();
            snapshot.session_id = if session.is_some() {
                session_id.clone()
            } else {
                String::new()
            };
            if !send(replies, WorkerReply::Snapshot(generation, snapshot)) {
                break;
            }
        }
        unsubscribe(subscription.take());
    }

    fn all_sessions(manager: &Manager) -> Vec<Session> {
        manager
            .GetSessions()
            .map(|list| list.into_iter().collect())
            .unwrap_or_default()
    }

    fn app_id(session: &Session) -> String {
        session
            .SourceAppUserModelId()
            .map(|id| id.to_string())
            .unwrap_or_default()
    }

    fn subscribe(session: &Session, events: &Sender<MediaCommand>) -> Option<Subscription> {
        let notify = events.clone();
        let properties = session
            .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                let _ = notify.send(MediaCommand::Refresh);
                Ok(())
            }))
            .ok()?;
        let notify = events.clone();
        let playback = match session.PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
            let _ = notify.send(MediaCommand::Refresh);
            Ok(())
        })) {
            Ok(token) => token,
            Err(_) => {
                let _ = session.RemoveMediaPropertiesChanged(properties);
                return None;
            }
        };
        Some(Subscription {
            session: session.clone(),
            properties,
            playback,
        })
    }

    fn unsubscribe(subscription: Option<Subscription>) {
        drop(subscription);
    }

    fn read(session: &Session, sessions: usize) -> Snapshot {
        let mut snapshot = Snapshot {
            active: true,
            sessions,
            source: source_label(&app_id(session)),
            ..Snapshot::default()
        };
        if let Ok(info) = session.GetPlaybackInfo() {
            snapshot.playing = info.PlaybackStatus().is_ok_and(|s| s == Status::Playing);
            if let Ok(controls) = info.Controls() {
                snapshot.can_toggle = controls.IsPlayPauseToggleEnabled().unwrap_or(false)
                    || if snapshot.playing {
                        controls.IsPauseEnabled().unwrap_or(false)
                    } else {
                        controls.IsPlayEnabled().unwrap_or(false)
                    };
                snapshot.can_next = controls.IsNextEnabled().unwrap_or(false);
                snapshot.can_previous = controls.IsPreviousEnabled().unwrap_or(false);
            }
        }
        if let Ok(properties) = session.TryGetMediaPropertiesAsync().and_then(|op| op.get()) {
            snapshot.title = properties
                .Title()
                .map(|t| sanitized(&t.to_string()))
                .unwrap_or_default();
            snapshot.artist = properties
                .Artist()
                .map(|a| sanitized(&a.to_string()))
                .unwrap_or_default();
            snapshot.art = properties.Thumbnail().ok().and_then(|reference| {
                let stream = reference.OpenReadAsync().and_then(|op| op.get()).ok()?;
                if stream.Size().ok()? > MAX_ART_BYTES {
                    return None;
                }
                decode(&stream).ok()
            });
        }
        if snapshot.title.is_empty() {
            snapshot.title = "Playing".into();
        }
        snapshot
    }

    fn decode(
        stream: &windows::Storage::Streams::IRandomAccessStreamWithContentType,
    ) -> windows::core::Result<ArtRaw> {
        let decoder = BitmapDecoder::CreateAsync(stream)?.get()?;
        let (width, height) = (decoder.PixelWidth()?, decoder.PixelHeight()?);
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(windows::core::Error::empty());
        }
        let scale = (ART_SIZE as f64 / width.max(height) as f64).min(1.0);
        let (out_w, out_h) = (
            ((width as f64 * scale).round() as u32).max(1),
            ((height as f64 * scale).round() as u32).max(1),
        );
        let transform = BitmapTransform::new()?;
        transform.SetScaledWidth(out_w)?;
        transform.SetScaledHeight(out_h)?;
        transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
        let pixels = decoder
            .GetPixelDataTransformedAsync(
                BitmapPixelFormat::Rgba8,
                BitmapAlphaMode::Straight,
                &transform,
                ExifOrientationMode::RespectExifOrientation,
                ColorManagementMode::DoNotColorManage,
            )?
            .get()?
            .DetachPixelData()?;
        if pixels.len() != (out_w * out_h * 4) as usize {
            return Err(windows::core::Error::empty());
        }
        Ok(ArtRaw {
            bytes: pixels.to_vec(),
            width: out_w,
            height: out_h,
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::mpsc::Sender;

    use super::MediaCommand;

    /// No OS media sessions on this platform; the core shows the honest
    /// unsupported state and nothing is ever started.
    pub(super) fn start(
        _generation: u64,
        _replies: async_channel::Sender<super::WorkerReply>,
        _revoked: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Option<Sender<MediaCommand>> {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests (deterministic, offline; pure fixtures, no real Windows sessions)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn live_snapshot(title: &str) -> Snapshot {
        Snapshot {
            session_id: "fixture:1".into(),
            active: true,
            title: title.into(),
            artist: "Artist".into(),
            source: "Spotify".into(),
            playing: true,
            can_toggle: true,
            can_next: true,
            can_previous: true,
            sessions: 1,
            art: None,
        }
    }

    fn queue(commands: &[MediaCommand]) -> VecDeque<MediaCommand> {
        commands.iter().cloned().collect()
    }

    #[test]
    fn source_labels_are_short_and_friendly() {
        assert_eq!(source_label("Spotify.exe"), "Spotify");
        assert_eq!(source_label("Chrome"), "Google Chrome");
        assert_eq!(source_label("MSEdge"), "Microsoft Edge");
        assert_eq!(source_label("308046B0AF4A39CB"), "Media app");
        assert_eq!(source_label("Foo.exe"), "Foo");
    }

    #[test]
    fn sanitized_metadata_drops_control_characters_and_caps_length() {
        let raw = format!("A\u{7}B\u{1f}\u{e}C{}", "x".repeat(200));
        let clean = sanitized(&raw);
        assert_eq!(clean.len(), 160);
        assert!(clean.chars().all(|c| !c.is_control()));
        assert!(clean.starts_with("ABCx"));
        assert_eq!(sanitized(""), "");
    }

    #[test]
    fn diagnostic_window_gate_starts_nothing() {
        let mut core = MediaCore::new(false, true);
        // Even an explicit activation never starts a worker.
        assert!(core.set_active(true).is_none());
        assert_eq!(core.phase, Phase::Diagnostics);
        assert!(core.refresh().is_none());
        assert!(core.command(MediaCommand::Toggle).is_none());
        assert_eq!(core.generation, 1);
        assert!(!core.view.active);
    }

    #[test]
    fn unsupported_platform_reports_unavailable_and_starts_nothing() {
        let mut core = MediaCore::new(true, false);
        assert!(core.set_active(true).is_none());
        assert_eq!(core.phase, Phase::Unsupported);
        assert!(core.refresh().is_none());
        assert!(core.command(MediaCommand::Cycle).is_none());
    }

    #[test]
    fn activation_starts_one_worker_and_deactivation_rejects_old_replies() {
        let mut core = MediaCore::new(true, true);
        let Some(CoreOp::Start { generation: 1 }) = core.set_active(true) else {
            panic!("activation must start exactly one worker");
        };
        assert_eq!(core.phase, Phase::Loading);
        // Re-activating never starts a second worker.
        assert!(core.set_active(true).is_none());
        // A reply from a stale generation is rejected while loading.
        assert!(!core.accept(WorkerReply::Snapshot(0, live_snapshot("old"))));
        assert_eq!(core.phase, Phase::Loading);
        // The live generation's snapshot applies.
        assert!(core.accept(WorkerReply::Snapshot(1, live_snapshot("Track"))));
        assert_eq!(core.phase, Phase::Live);
        assert_eq!(core.view.title, "Track");
        assert!(core.view.active);
        // Deactivation stops the worker and clears the view.
        let Some(CoreOp::Stop) = core.set_active(false) else {
            panic!("deactivation must stop the worker");
        };
        assert_eq!(core.phase, Phase::Inactive);
        assert!(core.view.title.is_empty());
        assert!(!core.view.active);
        // The disabled worker's late reply cannot revive the session.
        assert!(!core.accept(WorkerReply::Snapshot(1, live_snapshot("late"))));
        assert_eq!(core.phase, Phase::Inactive);
        // Re-activation runs under a fresh generation (deactivation already
        // revoked generation 2, so this is the third generation).
        let Some(CoreOp::Start { generation }) = core.set_active(true) else {
            panic!("re-activation must start a worker");
        };
        assert_eq!(generation, 3);
        assert_eq!(core.phase, Phase::Loading);
        assert!(!core.accept(WorkerReply::Snapshot(2, live_snapshot("stale"))));
        assert!(core.accept(WorkerReply::Snapshot(generation, live_snapshot("Next"))));
        assert_eq!(core.view.title, "Next");
    }

    #[test]
    fn worker_failure_is_terminal_and_reachable_retry() {
        let mut core = MediaCore::new(true, true);
        let Some(CoreOp::Start { generation: 1 }) = core.set_active(true) else {
            panic!("activation must start a worker");
        };
        assert!(core.accept(WorkerReply::Failed(
            1,
            "Media sessions are unavailable on this system.".into()
        )));
        assert_eq!(
            core.phase,
            Phase::Failed("Media sessions are unavailable on this system.".into())
        );
        // No commands leave a failed widget, and re-activation is a no-op:
        // recovery is only the explicit Retry/Refresh.
        assert!(core.command(MediaCommand::Toggle).is_none());
        assert!(core.set_active(true).is_none());
        let Some(RefreshOp::Restart { generation: 2 }) = core.refresh() else {
            panic!("refresh after failure must restart a fresh generation");
        };
        assert_eq!(core.phase, Phase::Loading);
        // The failed worker's late reply is rejected by generation.
        assert!(!core.accept(WorkerReply::Snapshot(1, live_snapshot("ghost"))));
        assert!(core.accept(WorkerReply::Snapshot(2, live_snapshot("Back"))));
        assert_eq!(core.phase, Phase::Live);
    }

    #[test]
    fn commands_only_leave_a_live_active_session() {
        let mut core = MediaCore::new(true, true);
        core.set_active(true);
        // Loading: a snapshot is still owed; no transport commands.
        assert!(core.command(MediaCommand::Next).is_none());
        // Live with no session shown: nothing to command.
        let mut idle = live_snapshot("Idle");
        idle.active = false;
        idle.sessions = 0;
        assert!(core.accept(WorkerReply::Snapshot(1, idle)));
        assert!(core.command(MediaCommand::Toggle).is_none());
        // Live with a session: commands pass; the worker rechecks anyway.
        assert!(core.accept(WorkerReply::Snapshot(1, live_snapshot("Track"))));
        assert_eq!(
            core.command(MediaCommand::Toggle),
            Some(MediaCommand::Toggle)
        );
        assert!(core.command(MediaCommand::Cycle).is_none());
    }

    #[test]
    fn refresh_reuses_the_live_worker_and_skips_pending_loads() {
        let mut core = MediaCore::new(true, true);
        core.set_active(true);
        // While loading, the initial read is still owed: nothing is enqueued.
        assert!(core.refresh().is_none());
        assert!(core.accept(WorkerReply::Snapshot(1, live_snapshot("Track"))));
        let Some(RefreshOp::Send { command }) = core.refresh() else {
            panic!("live refresh must ride the current worker");
        };
        assert_eq!(command, MediaCommand::Refresh);
        // Diagnostic and unsupported widgets never refresh.
        let mut diagnostic = MediaCore::new(false, true);
        assert!(diagnostic.refresh().is_none());
        let mut unsupported = MediaCore::new(true, false);
        assert!(unsupported.refresh().is_none());
    }

    #[test]
    fn coalesces_refresh_bursts_into_the_latest_command() {
        let mut queued = queue(&[
            MediaCommand::Refresh,
            MediaCommand::Refresh,
            MediaCommand::Refresh,
        ]);
        assert_eq!(
            coalesce_burst(MediaCommand::Refresh, &mut queued),
            MediaCommand::Refresh
        );
        assert!(queued.is_empty());

        let mut queued = queue(&[MediaCommand::Refresh, MediaCommand::Toggle]);
        assert_eq!(
            coalesce_burst(MediaCommand::Refresh, &mut queued),
            MediaCommand::Toggle
        );
        assert!(queued.is_empty());

        let mut queued = queue(&[MediaCommand::Refresh, MediaCommand::Next]);
        assert_eq!(
            coalesce_burst(MediaCommand::Refresh, &mut queued),
            MediaCommand::Next
        );

        // A non-refresh first command is processed as-is; its trailing
        // refresh stays queued for the next wake.
        let mut queued = queue(&[MediaCommand::Refresh]);
        assert_eq!(
            coalesce_burst(MediaCommand::Previous, &mut queued),
            MediaCommand::Previous
        );
        assert_eq!(queued.pop_front(), Some(MediaCommand::Refresh));

        let mut queued = queue(&[MediaCommand::Stop]);
        assert_eq!(
            coalesce_burst(MediaCommand::Refresh, &mut queued),
            MediaCommand::Stop
        );
    }

    #[test]
    fn artwork_converts_once_per_change_and_bad_art_is_dropped() {
        let mut view = MediaView::default();
        let good = ArtRaw {
            bytes: vec![1, 2, 3, 4],
            width: 1,
            height: 1,
        };
        view.set_art(&Some(good.clone()));
        let retained = view.art.clone().unwrap();
        // Unchanged artwork keeps the retained RenderImage (same image).
        view.set_art(&Some(good.clone()));
        assert!(Arc::ptr_eq(&retained, view.art.as_ref().unwrap()));
        // A real change converts exactly once more.
        let changed = ArtRaw {
            bytes: vec![5, 6, 7, 8],
            width: 1,
            height: 1,
        };
        view.set_art(&Some(changed));
        assert!(!Arc::ptr_eq(&retained, view.art.as_ref().unwrap()));
        assert!(view.art.is_some());
        // Malformed decoded art (pixel count mismatch) never becomes an image.
        let bad = ArtRaw {
            bytes: vec![9, 10, 11],
            width: 1,
            height: 1,
        };
        view.set_art(&Some(bad));
        assert!(view.art.is_none());
        // Losing the artwork clears both the image and its identity.
        view.set_art(&None);
        assert!(view.art.is_none() && view.art_raw.is_none());
    }

    #[test]
    fn snapshot_apply_maps_metadata_and_capabilities() {
        let mut view = MediaView::default();
        let mut snapshot = live_snapshot("Track");
        snapshot.artist = "Artist\u{7}".into();
        snapshot.can_next = false;
        snapshot.sessions = 3;
        view.apply(&snapshot);
        assert!(view.active && view.playing);
        assert!(view.can_toggle && view.can_previous && !view.can_next);
        assert_eq!(view.sessions, 3);
        assert_eq!(view.source, "Spotify");
        view.clear();
        assert!(!view.active && view.sessions == 0 && view.art.is_none());
    }
}
