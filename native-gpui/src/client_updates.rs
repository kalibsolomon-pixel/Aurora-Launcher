//! Explicit, reviewed Aurora Client (the Minecraft mod) updates for GPUI.
//!
//! Behavior authority is the shared typed boundary `native/src/api.rs` and the
//! launcher core behind it: discovery of published stable Client releases,
//! official GitHub SHA-256 verification, exact installed
//! Loader/Minecraft/Java compatibility, fingerprint re-derivation and the
//! rollback-safe transaction all live in the core. This module is exactly two
//! existing commands — [`api::preview_client_update`] and
//! [`api::apply_client_update`] — presented for one parent-supplied instance:
//! it derives no URLs, digests or compatibility facts, and it mutates
//! nothing the core has not already fingerprinted.
//!
//! The view is embedded by the parent for one exact instance: the parent
//! supplies the instance summary and its state revision through
//! [`ClientUpdates::set_target`] and gates its own controls on
//! [`ClientUpdatesEvent::BusyChanged`]. There is no instance selector and no
//! page heading here. One explicit Check runs one preview; confirming the
//! reviewed offer runs one apply. Constructing, retargeting or refreshing the
//! view never contacts the provider, and `host: None` builds the diagnostic
//! view that makes no requests at all.
//!
//! The approval is frozen at review time (target id and revision, instance
//! name, installed versions, candidate and fingerprint); apply sends exactly
//! that frozen instance and fingerprint once, and the core re-derives and
//! re-checks everything before any file changes. Enter never confirms; a
//! running apply stays visible and cannot be cancelled or abandoned, because
//! this API has no cancellation handle. `client-update-progress` events are
//! advisory labels only (they carry no instance attribution); the terminal
//! reply is the sole authority.

use aurora_launcher_lib::host::Host;
use gpui::{
    Context, EventEmitter, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    Render, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_kit::base::Dialog;
use serde_json::Value;
use tokio::runtime::Handle;

use crate::api;
use crate::backend::guarded;
use crate::controls::{AuroraButton, ButtonSize, ButtonVariant};
use crate::theme;
use crate::ui;

/// Emitted exactly when the single-operation busy state transitions (so the
/// parent can gate its own controls), and after an apply settles either way
/// (so the parent reloads launcher state — a partial or recovered failure may
/// need it). Reads, checks and dismissals alone never emit `Changed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientUpdatesEvent {
    Changed,
    BusyChanged(bool),
}

/// The frozen target the parent owns: exact instance id plus the parent's
/// state revision. Replies apply only while both still match.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Target {
    instance: String,
    revision: u64,
}

/// A terminal result belongs to an instance, independently of the revision
/// advanced by the parent's required post-apply reload. It carries no approval.
struct CompletedUpdate {
    instance: String,
    name: String,
}

/// One running operation; exactly one at a time. The serial is retained until
/// terminal completion, so a stale duplicate reply can never clear newer busy
/// state. The instance name is frozen for honest labels if the parent
/// retargets while the operation runs.
#[derive(Clone)]
struct Op {
    serial: u64,
    epoch: u64,
    kind: OpKind,
    target: Target,
    instance_name: String,
    phase: String,
}

#[derive(Clone, Debug)]
enum OpKind {
    Preview { serial: u64 },
    Apply { serial: u64 },
}

/// The frozen reviewed preview. `Candidate` is the only writable shape, and
/// its fingerprint is the sole approval token — display fields never
/// re-derive it. `Outcome` is informational only.
#[derive(Clone)]
struct Review {
    target: Target,
    instance_name: String,
    installed_version: String,
    installed_minecraft: String,
    kind: ReviewKind,
}

#[derive(Clone)]
enum ReviewKind {
    Candidate {
        version: String,
        minecraft_version: String,
        notes: Option<String>,
        warnings: Vec<String>,
        blockers: Vec<String>,
        fingerprint: String,
    },
    Outcome {
        text: String,
    },
}

// ------------------------------------------------------------- frozen calls

struct PreviewCall {
    op: u64,
    target: Target,
}

struct ApplyCall {
    op: u64,
    instance: String,
    fingerprint: String,
}

/// One completion arriving from the Tokio runtime. The operation serial
/// carries the guard: a reply settles only the operation it was started for.
enum Reply {
    Preview {
        op: u64,
        result: api::ApiResult<api::ClientUpdatePreview>,
    },
    Apply {
        op: u64,
        result: api::ApiResult<api::InstanceSummary>,
    },
}

fn reply_op(reply: &Reply) -> u64 {
    match reply {
        Reply::Preview { op, .. } | Reply::Apply { op, .. } => *op,
    }
}

/// A reply settles only the same-shape operation it was started for; the
/// review epoch guard for previews lives in `receive`.
fn reply_matches_op(op: &Op, reply: &Reply) -> bool {
    matches!(
        (&op.kind, reply),
        (OpKind::Preview { .. }, Reply::Preview { .. })
            | (OpKind::Apply { .. }, Reply::Apply { .. })
    )
}

// --------------------------------------------------------------- pure state

/// Presentation state and every guard, with no GPUI dependency so the state
/// transitions are directly testable.
#[derive(Default)]
struct UpdatesState {
    target: Option<Target>,
    instance_name: String,
    minecraft_version: String,
    /// The parent's summary shows an installed Aurora Client on a ready
    /// instance; anything else renders honestly and never checks.
    applicable: bool,
    op_serial: u64,
    epoch: u64,
    review_serial: u64,
    in_flight: Option<Op>,
    review: Option<Review>,
    review_open: bool,
    summary: Option<String>,
    message: String,
    error: Option<String>,
    completed: Option<CompletedUpdate>,
}

impl UpdatesState {
    fn busy(&self) -> bool {
        self.in_flight.is_some()
    }

    fn progress_label(&self) -> Option<String> {
        self.in_flight.as_ref().map(op_label)
    }

    fn current(&self, target: &Target) -> bool {
        self.target.as_ref() == Some(target)
    }

    /// Installs one operation, guarding the one-at-a-time rule.
    fn begin_op(&mut self, kind: OpKind) -> Option<u64> {
        if self.busy() || self.target.is_none() {
            return None;
        }
        self.op_serial = self.op_serial.wrapping_add(1);
        let serial = self.op_serial;
        self.in_flight = Some(Op {
            serial,
            epoch: self.epoch,
            kind,
            target: self.target.clone()?,
            instance_name: self.instance_name.clone(),
            phase: String::new(),
        });
        Some(serial)
    }

    /// The one-shot explicit check: one read-only preview for the exact
    /// current target. Opens the dialog immediately so the live check stays
    /// visible; nothing is writable until the frozen review exists.
    fn begin_preview(&mut self) -> Option<PreviewCall> {
        if self.busy() || self.target.is_none() || !self.applicable {
            return None;
        }
        self.clear_review_display();
        self.completed = None;
        self.message.clear();
        self.error = None;
        let target = self.target.clone()?;
        self.review_serial = self.review_serial.wrapping_add(1);
        let serial = self.review_serial;
        let op = self.begin_op(OpKind::Preview { serial })?;
        self.review_open = true;
        Some(PreviewCall { op, target })
    }

    /// Consumes the completed review into the exact apply the core will
    /// re-derive and fingerprint-check. Enabled only for an
    /// `updateAvailable` candidate with a nonempty version, Minecraft
    /// version and fingerprint, no blockers, and the current exact
    /// target/revision. The approval is the frozen record, never the display.
    fn confirm_apply(&mut self) -> Option<ApplyCall> {
        if self.busy() {
            return None;
        }
        let review = self.review.clone()?;
        let target = self.target.clone()?;
        let ReviewKind::Candidate {
            version,
            minecraft_version,
            fingerprint,
            blockers,
            ..
        } = &review.kind
        else {
            return None;
        };
        if review.target != target
            || fingerprint.is_empty()
            || version.is_empty()
            || minecraft_version.is_empty()
            || !blockers.is_empty()
        {
            return None;
        }
        self.review = None;
        // The dialog stays open while applying; the terminal settle closes it.
        let instance = target.instance.clone();
        let fingerprint = fingerprint.clone();
        let op = self.begin_op(OpKind::Apply {
            serial: self.review_serial,
        })?;
        Some(ApplyCall {
            op,
            instance,
            fingerprint,
        })
    }

    /// Drops the open review dialog's content without any provider call.
    /// Dismissal invalidates the frozen fingerprint; it is not a mutation and
    /// emits nothing. A committed apply keeps its progress visible.
    fn clear_review_display(&mut self) {
        if self.review.is_some() || self.review_open {
            self.review_serial = self.review_serial.wrapping_add(1);
        }
        self.review = None;
        let applying = matches!(
            self.in_flight.as_ref().map(|op| &op.kind),
            Some(OpKind::Apply { .. })
        );
        if !applying {
            self.review_open = false;
        }
    }

    /// Clears reviewed state; returns whether anything was dropped. Never
    /// touches an active operation: a running apply cannot be abandoned, and
    /// its terminal settle still reports to the parent.
    fn invalidate(&mut self) -> bool {
        self.epoch = self.epoch.wrapping_add(1);
        let had = self.review.is_some() || self.summary.is_some() || self.review_open;
        self.summary = None;
        self.clear_review_display();
        had
    }

    /// Adopts the parent's exact target. A changed instance id or revision
    /// invalidates every read; an unchanged target keeps the current review
    /// so a parent re-render never discards a fresh result.
    fn retarget(
        &mut self,
        target: Option<Target>,
        instance_name: String,
        minecraft_version: String,
        applicable: bool,
    ) {
        let changed = self.target != target;
        let keep_feedback = self.completed.as_ref().is_some_and(|completed| {
            target
                .as_ref()
                .is_none_or(|target| target.instance == completed.instance)
        });
        let keep_error_dialog = keep_feedback && self.error.is_some() && self.review_open;
        self.instance_name = instance_name;
        self.minecraft_version = minecraft_version;
        self.applicable = applicable;
        self.target = target;
        if changed {
            self.invalidate();
            if keep_feedback {
                self.review_open = keep_error_dialog;
            } else {
                self.completed = None;
                self.message.clear();
                self.error = None;
            }
        }
    }

    /// A `client-update-progress` event, attributed only to the apply this
    /// view itself started. The event carries no instance attribution, so its
    /// label is advisory only and never gates or settles anything; the exact
    /// terminal reply stays authoritative. Unknown names, malformed payloads,
    /// unknown phases and non-apply operations are ignored, never fatal.
    fn on_host_event(&mut self, name: &str, payload: &Value) -> bool {
        if name != "client-update-progress" {
            return false;
        }
        let Some(phase) = payload["phase"].as_str().and_then(client_phase_label) else {
            return false;
        };
        let Some(op) = &mut self.in_flight else {
            return false;
        };
        if !matches!(op.kind, OpKind::Apply { .. }) {
            return false;
        }
        let next = format!("{phase}…");
        if op.phase == next {
            return false;
        }
        op.phase = next;
        true
    }

    /// Applies one async completion; stale or duplicate messages change
    /// nothing, including busy state owned by a newer operation. An apply
    /// always reaches its terminal `BusyChanged(false)` + `Changed` on both
    /// success and failure, but a superseded-target settle never writes the
    /// old instance's feedback under the current target.
    fn receive(&mut self, reply: Reply) -> Effects {
        let mut fx = Effects::default();
        // Guard 1: only the current operation's terminal completion settles.
        let Some(op) = self.in_flight.take() else {
            return fx;
        };
        if op.serial != reply_op(&reply) || !reply_matches_op(&op, &reply) {
            self.in_flight = Some(op);
            return fx;
        }
        fx.busy = Some(false);
        fx.notify = true;
        let still_current = self.current(&op.target) && op.epoch == self.epoch;
        match reply {
            Reply::Preview { result, .. } => {
                self.review_open = false;
                // Guard 2: the review epoch and target must still be current.
                let serial = match op.kind {
                    OpKind::Preview { serial } => serial,
                    OpKind::Apply { .. } => return fx,
                };
                if serial != self.review_serial || !still_current {
                    return fx;
                }
                match result {
                    Ok(preview) => self.adopt_preview(preview, &op),
                    Err(e) => {
                        self.summary = None;
                        self.message.clear();
                        self.error = Some(error_text(&e));
                    }
                }
            }
            Reply::Apply { result, .. } => {
                fx.changed = true;
                self.review_open = false;
                if !still_current {
                    return fx;
                }
                self.completed = Some(CompletedUpdate {
                    instance: op.target.instance.clone(),
                    name: op.instance_name.clone(),
                });
                match result {
                    Ok(_) => {
                        self.summary = None;
                        self.error = None;
                        self.message = "Aurora Client updated and verified.".into();
                    }
                    Err(e) => {
                        self.summary = None;
                        self.message.clear();
                        self.error = Some(error_text(&e));
                        // The failure stays acknowledged in the open dialog.
                        self.review_open = true;
                    }
                }
            }
        }
        fx
    }

    /// Installs one settled preview, presenting every core outcome honestly:
    /// `updateAvailable`, `upToDate` and `installedNewer` are distinct, and
    /// unknown or incomplete shapes are reported as such — never as a false
    /// "up to date", and never as a writable review.
    fn adopt_preview(&mut self, preview: api::ClientUpdatePreview, op: &Op) {
        let installed = preview.installed_version.trim().to_owned();
        if installed.is_empty() {
            self.summary = None;
            self.message.clear();
            self.error =
                Some("The update service did not report the installed Aurora version.".into());
            return;
        }
        match preview.outcome.as_str() {
            "updateAvailable" => {
                let Some(candidate) = preview.candidate else {
                    self.incomplete_offer();
                    return;
                };
                let version = candidate.version.trim().to_owned();
                let minecraft = candidate.minecraft_version.trim().to_owned();
                let fingerprint = preview.fingerprint.trim().to_owned();
                if version.is_empty() || minecraft.is_empty() || fingerprint.is_empty() {
                    self.incomplete_offer();
                    return;
                }
                self.error = None;
                self.message.clear();
                self.summary = Some(if preview.blockers.is_empty() {
                    format!("Aurora {installed} → {version} available.")
                } else {
                    format!(
                        "Aurora {installed} → {version} is blocked. Resolve the blockers, then check again."
                    )
                });
                self.review = Some(Review {
                    target: op.target.clone(),
                    instance_name: self.instance_name.clone(),
                    installed_version: installed,
                    installed_minecraft: self.minecraft_version.clone(),
                    kind: ReviewKind::Candidate {
                        version,
                        minecraft_version: minecraft,
                        notes: candidate.notes,
                        warnings: preview.warnings,
                        blockers: preview.blockers,
                        fingerprint,
                    },
                });
                self.review_open = true;
            }
            "upToDate" => {
                let mut text = format!("Aurora {installed} is up to date.");
                for warning in &preview.warnings {
                    text.push_str("\n");
                    text.push_str(warning);
                }
                self.error = None;
                self.message.clear();
                self.summary = Some(format!("Aurora {installed} is up to date."));
                self.review = Some(Review {
                    target: op.target.clone(),
                    instance_name: self.instance_name.clone(),
                    installed_version: installed,
                    installed_minecraft: self.minecraft_version.clone(),
                    kind: ReviewKind::Outcome { text },
                });
                self.review_open = true;
            }
            "installedNewer" => {
                let mut text = String::from(
                    "The installed Aurora Client is newer than every eligible published release; it will not be downgraded.",
                );
                for warning in &preview.warnings {
                    text.push_str("\n");
                    text.push_str(warning);
                }
                self.error = None;
                self.message.clear();
                self.summary = Some(format!(
                    "Aurora {installed} is newer than every eligible published release."
                ));
                self.review = Some(Review {
                    target: op.target.clone(),
                    instance_name: self.instance_name.clone(),
                    installed_version: installed,
                    installed_minecraft: self.minecraft_version.clone(),
                    kind: ReviewKind::Outcome { text },
                });
                self.review_open = true;
            }
            other => {
                self.summary = None;
                self.message.clear();
                self.error = Some(format!(
                    "The update service returned an unrecognized result ({other}). Nothing can be installed."
                ));
            }
        }
    }

    fn incomplete_offer(&mut self) {
        self.summary = None;
        self.message.clear();
        self.error =
            Some("The published update offer is incomplete. Nothing can be installed.".into());
    }
}

/// Follow-up work a state transition asks the view to perform.
#[derive(Default, Debug, PartialEq)]
struct Effects {
    notify: bool,
    changed: bool,
    busy: Option<bool>,
}

// --------------------------------------------------------------- pure helpers

fn error_text(e: &api::ApiError) -> String {
    format!("{} ({})", e.message, e.code)
}

/// Labels for the core's `ClientPhase` vocabulary (serde camelCase);
/// `None` for a name the core does not emit.
fn client_phase_label(phase: &str) -> Option<&'static str> {
    Some(match phase {
        "acquiring" => "Downloading and verifying the published release",
        "staging" => "Staging the verified update",
        "activating" => "Activating the verified update",
        "validating" => "Validating the updated instance",
        "committing" => "Committing the update",
        _ => return None,
    })
}

fn op_label(op: &Op) -> String {
    match &op.kind {
        OpKind::Preview { .. } => {
            format!("Checking the published release in {}…", op.instance_name)
        }
        OpKind::Apply { .. } => {
            if op.phase.is_empty() {
                format!("Updating the Aurora Client in {}…", op.instance_name)
            } else {
                format!("{} in {}…", op.phase, op.instance_name)
            }
        }
    }
}

// ------------------------------------------------------------------ the view

pub struct ClientUpdates {
    state: UpdatesState,
    host: Option<Host>,
    runtime: Handle,
    tx: async_channel::Sender<Reply>,
    blocked: bool,
}

impl EventEmitter<ClientUpdatesEvent> for ClientUpdates {}

impl ClientUpdates {
    /// Builds the view. `host: None` constructs the diagnostic view that
    /// makes no provider, credential or integration calls. Constructing the
    /// view never checks for updates; the explicit Check button does.
    pub fn new(
        host: Option<Host>,
        runtime: Handle,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Bounded, event-driven pump: at most the settle of the one running
        // operation plus one stale duplicate can ever be queued; nothing polls.
        let (tx, rx) = async_channel::bounded(2);
        cx.spawn(async move |weak, cx| {
            while let Ok(reply) = rx.recv().await {
                if weak
                    .update(cx, |this: &mut Self, cx| {
                        let fx = this.state.receive(reply);
                        this.apply(fx, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            state: UpdatesState::default(),
            host,
            runtime,
            tx,
            blocked: false,
        }
    }

    /// Adopts the parent's exact instance and state revision. This never
    /// initiates a check or mutation; a changed instance id or revision
    /// invalidates any reviewed offer and waits for explicit activation.
    pub fn set_target(
        &mut self,
        instance: Option<api::InstanceSummary>,
        revision: u64,
        cx: &mut Context<Self>,
    ) {
        let (target, name, minecraft, applicable) = match instance {
            Some(instance) => {
                let applicable = instance.aurora.is_some() && instance.state == "ready";
                (
                    Some(Target {
                        instance: instance.id,
                        revision,
                    }),
                    instance.display_name,
                    instance.minecraft_version,
                    applicable,
                )
            }
            None => (None, String::new(), String::new(), false),
        };
        self.state.retarget(target, name, minecraft, applicable);
        cx.notify();
    }

    /// Clears reviewed state without any provider call: the parent invokes
    /// this after the instance changed elsewhere so a stale offer never
    /// describes superseded files. An explicit Check re-reads the preview.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.state.invalidate() && self.state.completed.is_none() {
            self.state.message =
                "The instance changed since the last check. Check again for current updates."
                    .into();
        }
        cx.notify();
    }

    /// True while the single preview or apply is running. Exactly one
    /// operation runs at a time; there is no queue.
    pub fn is_busy(&self) -> bool {
        self.state.busy()
    }

    /// Blocks starting new work while the parent is changing the instance.
    /// Closing an idle review and dismissing remain available.
    pub fn set_blocked(&mut self, blocked: bool, cx: &mut Context<Self>) {
        if self.blocked != blocked {
            self.blocked = blocked;
            cx.notify();
        }
    }

    /// Routes one host event. Only `client-update-progress` concerns this
    /// view and only while its own apply runs; unknown names and malformed
    /// payloads are ignored, never fatal.
    pub fn on_host_event(&mut self, name: &str, payload: &Value, cx: &mut Context<Self>) {
        if self.state.on_host_event(name, payload) {
            cx.notify();
        }
    }

    fn apply(&mut self, fx: Effects, cx: &mut Context<Self>) {
        if let Some(busy) = fx.busy {
            cx.emit(ClientUpdatesEvent::BusyChanged(busy));
        }
        if fx.changed {
            cx.emit(ClientUpdatesEvent::Changed);
        }
        if fx.notify {
            cx.notify();
        }
    }

    // ---------------------------------------------------------------- actions

    fn start_check(&mut self, cx: &mut Context<Self>) {
        // Live blocked check at action time, not only in the disabled style.
        if self.blocked {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let was_busy = self.state.busy();
        let Some(call) = self.state.begin_preview() else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        let instance = call.target.instance.clone();
        self.runtime.spawn(async move {
            let result = guarded(api::preview_client_update(host, instance)).await;
            let _ = tx
                .send(Reply::Preview {
                    op: call.op,
                    result,
                })
                .await;
        });
        if !was_busy {
            cx.emit(ClientUpdatesEvent::BusyChanged(true));
        }
        cx.notify();
    }

    fn start_apply(&mut self, cx: &mut Context<Self>) {
        // Live blocked check at action time, not only in the disabled style.
        if self.blocked {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let was_busy = self.state.busy();
        let Some(call) = self.state.confirm_apply() else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        let instance = call.instance.clone();
        let fingerprint = call.fingerprint.clone();
        self.runtime.spawn(async move {
            let result = guarded(api::apply_client_update(
                host,
                instance.clone(),
                fingerprint,
            ))
            .await;
            let _ = tx
                .send(Reply::Apply {
                    op: call.op,
                    result,
                })
                .await;
        });
        if !was_busy {
            cx.emit(ClientUpdatesEvent::BusyChanged(true));
        }
        cx.notify();
    }

    fn dismiss_review(&mut self, cx: &mut Context<Self>) {
        self.state.clear_review_display();
        cx.notify();
    }
}

// -------------------------------------------------------------------- render

/// Multi-line text as one child per line (no rich formatting invented).
fn text_block(text: &str, color: Hsla, size: Pixels) -> impl IntoElement + use<> {
    let block = div().flex().flex_col().text_size(size).text_color(color);
    text.lines()
        .fold(block, |block, line| block.child(line.to_owned()))
}

/// The compact card the embedded view lives in: one row, tidy and quiet.
fn update_card(t: &theme::AuroraTheme) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .px(px(14.))
        .py(px(10.))
        .rounded(t.radius_lg)
        .bg(t.card)
        .border_1()
        .border_color(t.border_subtle)
}

/// The same card with no action, for diagnostic and no-target states.
fn quiet_card(t: &theme::AuroraTheme, title: &str, detail: &str) -> gpui::Div {
    update_card(t).child(
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(ui::icon_tile(t, Some("system_update"), 28., false))
            .child(ui::title_block(t, title.to_owned(), Some(detail.to_owned().into())).flex_1()),
    )
}

impl Render for ClientUpdates {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx).clone();
        let view = cx.entity();
        let busy = self.state.busy();

        // Embedded view: no selector and no page heading — the parent names
        // the page and the instance context.
        let mut root = div().flex().flex_col();

        if self.host.is_none() {
            return root.child(quiet_card(
                &palette,
                "Aurora Client updates",
                "Unavailable in diagnostic mode. No provider requests are made from this view.",
            ));
        }

        if self.state.target.is_none()
            && !busy
            && !self.state.review_open
            && self.state.completed.is_none()
        {
            return root.child(quiet_card(
                &palette,
                "Aurora Client updates",
                "Choose an instance to check for Aurora Client updates.",
            ));
        }

        let checking = matches!(
            self.state.in_flight.as_ref().map(|op| &op.kind),
            Some(OpKind::Preview { .. })
        );
        let v = view.clone();

        // One short status line: progress, then the applicability reason, then
        // the last check summary, then the idle hint.
        let status = if let Some(label) = self.state.progress_label() {
            label
        } else if !self.state.applicable {
            "Available once the Client is installed and the instance is ready.".to_owned()
        } else if let Some(summary) = self.state.summary.clone() {
            summary
        } else if self.state.error.is_none() {
            "Check whether a newer Aurora Client is published for this instance.".to_owned()
        } else {
            "The last check did not finish.".to_owned()
        };
        let mut card = update_card(&palette).child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(ui::icon_tile(&palette, Some("system_update"), 28., false))
                .child(
                    ui::title_block(&palette, "Aurora Client updates", Some(status.into()))
                        .flex_1(),
                )
                .child(if self.state.applicable || busy {
                    AuroraButton::new(
                        "client-updates-check",
                        if checking {
                            "Checking…"
                        } else {
                            "Check for updates"
                        },
                    )
                    .size(ButtonSize::Small)
                    .icon("refresh")
                    .disabled(busy || self.blocked || !self.state.applicable)
                    .on_click(move |_, _, cx| {
                        v.update(cx, |s, cx| s.start_check(cx));
                    })
                    .into_any_element()
                } else {
                    // Nothing to run: a status, not a dead button.
                    ui::badge(&palette, "Not installed", ui::Tone::Neutral).into_any_element()
                }),
        );
        if !self.state.message.is_empty() {
            let message = self.state.completed.as_ref().map_or_else(
                || self.state.message.clone(),
                |completed| format!("{}: {}", completed.name, self.state.message),
            );
            card = card.child(
                div()
                    .pl(px(40.))
                    .text_size(px(ui::TEXT_CAPTION))
                    .text_color(palette.foreground)
                    .child(message),
            );
        }
        if let Some(error) = &self.state.error {
            card = card.child(
                div()
                    .pl(px(40.))
                    .text_size(px(ui::TEXT_CAPTION))
                    .text_color(palette.danger)
                    .child(error.clone()),
            );
        }
        root = root.child(card);

        // --------------------------------------------------------- the dialog
        let panel = self.review_panel(cx, &palette, busy);
        let veto = view.clone();
        let changed = view.clone();
        root.child(
            Dialog::new(cx)
                .open(self.state.review_open)
                .accessibility_label("Review Aurora Client update")
                .close_on_backdrop_press(false)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !veto.read(cx).state.busy())
                .on_open_change(move |open, _, _, cx| {
                    if !open {
                        changed.update(cx, |s, cx| {
                            if !s.state.busy() {
                                s.dismiss_review(cx);
                            }
                        });
                    }
                })
                .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                .popup(panel),
        )
    }
}

impl ClientUpdates {
    /// The explicit review dialog. The base `Dialog` owns the focus trap,
    /// Escape/Enter bindings and focus restoration; Enter never confirms
    /// (buttons own their own keyboard activation) and Escape is vetoed
    /// while work is running, so a committed apply cannot be abandoned.
    /// The bounded scroll surface keeps long notes, warnings and blockers
    /// from hiding the decision controls.
    fn review_panel(
        &mut self,
        cx: &mut Context<Self>,
        palette: &theme::AuroraTheme,
        busy: bool,
    ) -> impl IntoElement + use<> {
        let view = cx.entity();
        let operation_busy = self.state.busy();
        let mut panel = div()
            .id("client-updates-dialog")
            .w(px(640.))
            .max_h(px(560.))
            .overflow_y_scroll()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .rounded(palette.radius_lg)
            .bg(palette.raised)
            .border_1()
            .border_color(palette.border_subtle)
            .text_color(palette.foreground);

        if let Some(op) = self.state.in_flight.clone() {
            match &op.kind {
                OpKind::Preview { .. } => {
                    panel = panel
                        .child(
                            div()
                                .text_size(px(ui::TEXT_SECTION + 1.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(format!("Update Aurora Client in {}", op.instance_name)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette.muted_foreground)
                                .child("Checking the published release…"),
                        );
                }
                OpKind::Apply { .. } => {
                    // This API has no cancellation handle: the committed
                    // transaction stays visible and runs to its terminal
                    // reply. No Cancel control exists, and no byte counts
                    // are invented.
                    panel = panel
                        .child(
                            div()
                                .text_size(px(ui::TEXT_SECTION + 1.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(format!("Updating Aurora Client in {}", op.instance_name)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette.muted_foreground)
                                .child(op_label(&op)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette.muted_foreground)
                                .child(
                                    "This update runs to completion and cannot be cancelled here. If it fails, the previous Aurora Client is restored.",
                                ),
                        );
                }
            }
        } else if let Some(review) = self.state.review.clone() {
            let title = format!("Update Aurora Client in {}", review.instance_name);
            panel = panel.child(
                div()
                    .text_size(px(ui::TEXT_SECTION + 1.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(title),
            );
            let close = view.clone();
            match &review.kind {
                ReviewKind::Candidate {
                    version,
                    minecraft_version,
                    notes,
                    warnings,
                    blockers,
                    ..
                } => {
                    let mut text = format!(
                        "Aurora {} → {} (Minecraft {} → {}).",
                        review.installed_version,
                        version,
                        review.installed_minecraft,
                        minecraft_version
                    );
                    text.push_str(
                        "\nThe download is verified against its published SHA-256 before anything is replaced; your mods, worlds and settings are kept.",
                    );
                    if let Some(notes) = notes
                        .as_deref()
                        .map(str::trim)
                        .filter(|notes| !notes.is_empty())
                    {
                        text.push_str("\n\nRelease notes\n");
                        text.push_str(notes);
                    }
                    if !warnings.is_empty() {
                        text.push_str("\n\nWarnings\n");
                        for warning in warnings {
                            text.push_str(warning);
                            text.push('\n');
                        }
                    }
                    if !blockers.is_empty() {
                        text.push_str("\n\nBlocked\n");
                        for blocker in blockers {
                            text.push_str(&format!("Blocked: {blocker}\n"));
                        }
                    }
                    let v = view.clone();
                    panel = panel
                        .child(text_block(&text, palette.foreground, px(13.)))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(8.))
                                .pt(px(6.))
                                .child(
                                    AuroraButton::new(
                                        "client-updates-apply",
                                        "Update Aurora Client",
                                    )
                                    .variant(ButtonVariant::Primary)
                                    .disabled(busy || self.blocked || !blockers.is_empty())
                                    .on_click(
                                        move |_, _, cx| {
                                            v.update(cx, |s, cx| s.start_apply(cx));
                                        },
                                    ),
                                )
                                .child(
                                    AuroraButton::new("client-updates-close", "Close")
                                        .disabled(operation_busy)
                                        .on_click(move |_, _, cx| {
                                            close.update(cx, |s, cx| s.dismiss_review(cx));
                                        }),
                                ),
                        );
                }
                ReviewKind::Outcome { text } => {
                    panel = panel
                        .child(text_block(text, palette.foreground, px(13.)))
                        .child(
                            div().flex().flex_wrap().gap(px(8.)).pt(px(6.)).child(
                                AuroraButton::new("client-updates-close", "Close")
                                    .disabled(operation_busy)
                                    .on_click(move |_, _, cx| {
                                        close.update(cx, |s, cx| s.dismiss_review(cx));
                                    }),
                            ),
                        );
                }
            }
        } else if let Some(error) = self.state.error.clone() {
            // A failed check or apply stays acknowledged until dismissed.
            let close = view.clone();
            panel = panel
                .child(
                    div()
                        .text_size(px(ui::TEXT_SECTION + 1.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(self.state.completed.as_ref().map_or_else(
                            || "Aurora Client update".to_owned(),
                            |completed| format!("Aurora Client update in {}", completed.name),
                        )),
                )
                .child(div().text_color(palette.danger).child(error))
                .child(
                    div().flex().flex_wrap().gap(px(8.)).pt(px(6.)).child(
                        AuroraButton::new("client-updates-close", "Close")
                            .disabled(operation_busy)
                            .on_click(move |_, _, cx| {
                                close.update(cx, |s, cx| s.dismiss_review(cx));
                            }),
                    ),
                );
        } else {
            panel = panel.child("Check for Aurora Client updates");
        }
        panel
    }
}

// --------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn target(revision: u64) -> Target {
        Target {
            instance: "i1".into(),
            revision,
        }
    }

    fn retargeted(target: Option<Target>, applicable: bool) -> UpdatesState {
        let mut state = UpdatesState::default();
        state.retarget(target, "Instance".into(), "1.21.11".into(), applicable);
        state
    }

    fn preview(
        outcome: &str,
        candidate: Option<api::ClientUpdateCandidate>,
        fingerprint: &str,
    ) -> api::ClientUpdatePreview {
        api::ClientUpdatePreview {
            installed_version: "3.2.0".into(),
            outcome: outcome.into(),
            candidate,
            blockers: vec![],
            warnings: vec![],
            fingerprint: fingerprint.into(),
        }
    }

    fn candidate() -> api::ClientUpdateCandidate {
        api::ClientUpdateCandidate {
            version: "3.2.2".into(),
            notes: Some("Fixes things.".into()),
            minecraft_version: "1.21.11".into(),
        }
    }

    fn settle_preview(
        state: &mut UpdatesState,
        op: u64,
        result: api::ApiResult<api::ClientUpdatePreview>,
    ) -> Effects {
        state.receive(Reply::Preview { op, result })
    }

    /// A state holding one frozen, installable review for target revision 1.
    fn reviewed_state() -> UpdatesState {
        let mut state = retargeted(Some(target(1)), true);
        let call = state.begin_preview().unwrap();
        settle_preview(
            &mut state,
            call.op,
            Ok(preview("updateAvailable", Some(candidate()), "fp1")),
        );
        assert!(state.review.is_some());
        state
    }

    fn instance_summary() -> api::InstanceSummary {
        serde_json::from_value(json!({
            "id": "i1", "displayName": "Instance", "state": "ready",
            "aurora": { "channel": "stable", "version": "3.2.0" },
            "platform": { "kind": "fabric", "version": "0.20.1" },
            "minecraftVersion": "1.21.11",
            "configuration": {
                "minecraftVersion": "1.21.11", "auroraEnabled": true,
                "loader": { "kind": "fabric", "policy": { "type": "automatic" } },
                "memoryMib": 4096, "additionalJvmArguments": "", "window": null
            }
        }))
        .unwrap()
    }

    // ------------------------------------------------ explicit activation

    #[test]
    fn a_fresh_view_checks_nothing_until_explicitly_asked() {
        let mut state = retargeted(Some(target(1)), true);
        assert!(!state.busy());
        assert!(state.review.is_none());
        assert!(state.confirm_apply().is_none());
        let call = state.begin_preview().unwrap();
        assert!(state.busy());
        assert!(state.review_open, "the live check stays visible");
        assert!(
            state.begin_preview().is_none(),
            "exactly one operation at a time"
        );
        let fx = settle_preview(&mut state, call.op, Ok(preview("upToDate", None, "fp")));
        assert_eq!(fx.busy, Some(false));
        assert!(!fx.changed, "a read never emits Changed");
    }

    #[test]
    fn inapplicable_targets_never_check() {
        let mut state = retargeted(Some(target(1)), false);
        assert!(state.begin_preview().is_none());
        assert!(retargeted(None, true).begin_preview().is_none());
        state.retarget(Some(target(2)), "Instance".into(), "1.21.11".into(), true);
        assert!(state.begin_preview().is_some());
    }

    // ---------------------------------------------------- target identity

    #[test]
    fn targets_reject_changed_instance_and_revision() {
        let t = target(1);
        assert!(retargeted(Some(t.clone()), true).current(&t));
        let mut drifted = t.clone();
        drifted.revision += 1;
        assert!(!retargeted(Some(t.clone()), true).current(&drifted));
        let mut drifted = t.clone();
        drifted.instance = "i2".into();
        assert!(!retargeted(Some(t), true).current(&drifted));
    }

    #[test]
    fn retargeting_and_refresh_clear_stale_reads_but_keep_active_operations() {
        let mut state = reviewed_state();
        let call = state.begin_preview().unwrap();
        state.retarget(Some(target(2)), "Other".into(), "1.21.11".into(), true);
        assert!(state.busy(), "a running future is never abandoned");
        let fx = settle_preview(
            &mut state,
            call.op,
            Ok(preview("updateAvailable", Some(candidate()), "fp1")),
        );
        assert_eq!(
            fx.busy,
            Some(false),
            "the terminal settle still clears busy"
        );
        assert!(state.review.is_none(), "the superseded review is discarded");
        assert!(state.summary.is_none());
        assert!(!state.review_open);

        let mut state = reviewed_state();
        assert!(state.invalidate(), "a review was dropped");
        assert!(state.review.is_none() && state.summary.is_none());
        assert!(!state.invalidate(), "a second clear drops nothing");
    }

    #[test]
    fn same_target_refresh_rejects_the_settling_read() {
        let mut state = retargeted(Some(target(1)), true);
        let call = state.begin_preview().unwrap();
        state.invalidate();
        let fx = settle_preview(
            &mut state,
            call.op,
            Ok(preview("updateAvailable", Some(candidate()), "fp1")),
        );
        assert_eq!(fx.busy, Some(false));
        assert!(state.review.is_none() && !state.review_open);
    }

    // ---------------------------------------------------------- stale guards

    #[test]
    fn duplicate_and_late_settles_cannot_clear_newer_busy() {
        let mut state = retargeted(Some(target(1)), true);
        let first = state.begin_preview().unwrap();
        settle_preview(&mut state, first.op, Ok(preview("upToDate", None, "fp")));
        let second = state.begin_preview().unwrap();
        assert!(state.busy());
        let fx = settle_preview(&mut state, first.op, Ok(preview("upToDate", None, "fp")));
        assert_eq!(fx, Effects::default(), "a duplicate settle changes nothing");
        assert!(state.busy(), "the newer operation keeps its busy state");
        let fx = settle_preview(&mut state, second.op, Ok(preview("upToDate", None, "fp")));
        assert_eq!(fx.busy, Some(false));
    }

    #[test]
    fn replies_for_unknown_or_wrong_shape_operations_are_ignored() {
        let mut state = retargeted(Some(target(1)), true);
        let fx = state.receive(Reply::Preview {
            op: 99,
            result: Ok(preview("upToDate", None, "fp")),
        });
        assert_eq!(fx, Effects::default());

        let mut state = retargeted(Some(target(1)), true);
        let call = state.begin_preview().unwrap();
        let fx = state.receive(Reply::Apply {
            op: call.op,
            result: Ok(instance_summary()),
        });
        assert_eq!(fx, Effects::default());
        assert!(state.busy(), "the preview keeps running");
    }

    #[test]
    fn dismissal_invalidates_the_reviewed_fingerprint() {
        let mut state = reviewed_state();
        state.clear_review_display();
        assert!(
            state.confirm_apply().is_none(),
            "no frozen fingerprint remains"
        );
        assert!(!state.review_open);

        // A late duplicate settle cannot restore the dismissed review.
        let mut state = reviewed_state();
        let call = state.begin_preview().unwrap();
        state.clear_review_display();
        settle_preview(
            &mut state,
            call.op,
            Ok(preview("updateAvailable", Some(candidate()), "fp1")),
        );
        assert!(state.review.is_none());
    }

    // ---------------------------------------------------- distinct outcomes

    #[test]
    fn up_to_date_and_installed_newer_are_distinct_and_informational() {
        let mut state = retargeted(Some(target(1)), true);
        let call = state.begin_preview().unwrap();
        settle_preview(&mut state, call.op, Ok(preview("upToDate", None, "fp")));
        let review = state.review.clone().unwrap();
        assert!(matches!(review.kind, ReviewKind::Outcome { .. }));
        assert!(state.confirm_apply().is_none());
        assert_eq!(
            state.summary.as_deref(),
            Some("Aurora 3.2.0 is up to date.")
        );
        let ReviewKind::Outcome { text } = review.kind else {
            unreachable!();
        };
        assert_eq!(text, "Aurora 3.2.0 is up to date.");

        let mut state = retargeted(Some(target(1)), true);
        let call = state.begin_preview().unwrap();
        settle_preview(
            &mut state,
            call.op,
            Ok(preview("installedNewer", None, "fp")),
        );
        let review = state.review.clone().unwrap();
        assert!(matches!(review.kind, ReviewKind::Outcome { .. }));
        assert!(state.confirm_apply().is_none());
        assert_eq!(
            state.summary.as_deref(),
            Some("Aurora 3.2.0 is newer than every eligible published release.")
        );
        let ReviewKind::Outcome { text } = review.kind else {
            unreachable!();
        };
        assert!(text.contains("will not be downgraded"));
        assert_ne!(
            state.summary.as_deref(),
            Some("Aurora 3.2.0 is up to date."),
            "installedNewer is never presented as up to date"
        );
    }

    #[test]
    fn warnings_reach_the_outcome_review_text() {
        let mut state = retargeted(Some(target(1)), true);
        let call = state.begin_preview().unwrap();
        let mut payload = preview("installedNewer", None, "fp");
        payload
            .warnings
            .push("Published metadata is partial.".into());
        settle_preview(&mut state, call.op, Ok(payload));
        let review = state.review.clone().unwrap();
        let ReviewKind::Outcome { text } = review.kind else {
            unreachable!();
        };
        assert!(text.contains("Published metadata is partial."));
    }

    #[test]
    fn unknown_and_incomplete_shapes_are_honest_and_never_writable() {
        let mut state = retargeted(Some(target(1)), true);
        let call = state.begin_preview().unwrap();
        settle_preview(
            &mut state,
            call.op,
            Ok(preview("futureOutcome", None, "fp")),
        );
        assert!(state.review.is_none(), "unknown outcomes never review");
        assert!(
            state
                .error
                .as_deref()
                .is_some_and(|e| e.contains("unrecognized"))
        );

        // Empty installed version, missing candidate, empty candidate fields
        // and an empty fingerprint are incomplete offers, never up to date.
        let cases: Vec<api::ClientUpdatePreview> = vec![
            {
                let mut p = preview("updateAvailable", Some(candidate()), "fp1");
                p.installed_version.clear();
                p
            },
            preview("updateAvailable", None, "fp1"),
            {
                let mut c = candidate();
                c.version = "  ".into();
                preview("updateAvailable", Some(c), "fp1")
            },
            {
                let mut c = candidate();
                c.minecraft_version.clear();
                preview("updateAvailable", Some(c), "fp1")
            },
            preview("updateAvailable", Some(candidate()), ""),
        ];
        for payload in cases {
            let mut state = retargeted(Some(target(1)), true);
            let call = state.begin_preview().unwrap();
            settle_preview(&mut state, call.op, Ok(payload));
            assert!(state.review.is_none(), "an incomplete offer never reviews");
            assert!(state.error.is_some(), "the incompleteness is reported");
            assert!(state.confirm_apply().is_none());
        }
    }

    #[test]
    fn failed_checks_are_honest_not_silent() {
        let mut state = retargeted(Some(target(1)), true);
        let call = state.begin_preview().unwrap();
        let fx = settle_preview(
            &mut state,
            call.op,
            Err(api::ApiError::new("update_check_unavailable", "offline")),
        );
        assert_eq!(fx.busy, Some(false));
        assert!(!fx.changed);
        assert_eq!(
            state.error.as_deref(),
            Some("offline (update_check_unavailable)")
        );
        assert!(state.summary.is_none());
    }

    // ------------------------------------------------------- apply gating

    #[test]
    fn the_frozen_request_gates_on_target_candidate_and_fingerprint() {
        let mut state = reviewed_state();
        let call = state.confirm_apply().unwrap();
        assert_eq!(call.instance, "i1");
        assert_eq!(call.fingerprint, "fp1");
        assert!(state.busy(), "the apply owns the single operation slot");
        assert!(state.review.is_none(), "the approval is consumed once");
        assert!(state.confirm_apply().is_none());
        assert!(state.review_open, "the committed apply stays visible");

        // Target drift freezes the approval out: a new revision or instance
        // never approves the old fingerprint.
        let mut state = reviewed_state();
        state.retarget(
            Some(Target {
                instance: "i1".into(),
                revision: 2,
            }),
            "Instance".into(),
            "1.21.11".into(),
            true,
        );
        assert!(state.confirm_apply().is_none());
        let mut state = reviewed_state();
        state.retarget(
            Some(Target {
                instance: "i2".into(),
                revision: 1,
            }),
            "Other".into(),
            "1.21.11".into(),
            true,
        );
        assert!(state.confirm_apply().is_none());
    }

    #[test]
    fn blockers_keep_the_review_visible_but_not_installable() {
        let mut state = retargeted(Some(target(1)), true);
        let call = state.begin_preview().unwrap();
        let mut payload = preview("updateAvailable", Some(candidate()), "fp1");
        payload
            .blockers
            .push("Quit Minecraft before updating the Aurora Client.".into());
        settle_preview(&mut state, call.op, Ok(payload));
        assert!(state.review.is_some(), "the blocker is shown for review");
        assert!(state.confirm_apply().is_none(), "blockers never install");
        let review = state.review.clone().unwrap();
        let ReviewKind::Candidate { blockers, .. } = review.kind else {
            unreachable!();
        };
        assert_eq!(blockers.len(), 1);
    }

    #[test]
    fn apply_feedback_survives_parent_reload_revisions_until_instance_changes() {
        for failed in [false, true] {
            let mut state = reviewed_state();
            let apply = state.confirm_apply().unwrap();
            let result = if failed {
                Err(api::ApiError::new("fixture_failed", "The update failed."))
            } else {
                Ok(instance_summary())
            };
            let effects = state.receive(Reply::Apply {
                op: apply.op,
                result,
            });
            assert!(effects.changed);
            let message = state.message.clone();
            let error = state.error.clone();
            // Changed makes root reload_guarded advance the serial and immediately
            // resupply the selected instance, then the winning load supplies it again.
            for revision in [2, 3] {
                state.retarget(
                    Some(target(revision)),
                    "Instance".into(),
                    "1.21.11".into(),
                    true,
                );
                assert_eq!(state.message, message);
                assert_eq!(state.error, error);
                assert_eq!(state.review_open, failed);
                assert!(state.review.is_none() && state.confirm_apply().is_none());
            }
            // A failed registry read cannot erase the acknowledged result either.
            state.retarget(None, String::new(), String::new(), false);
            assert_eq!(state.message, message);
            assert_eq!(state.error, error);
            assert_eq!(state.review_open, failed);
            state.retarget(Some(target(4)), "Instance".into(), "1.21.11".into(), true);
            state.clear_review_display(); // The real Close/Escape path.
            state.retarget(Some(target(5)), "Instance".into(), "1.21.11".into(), true);
            assert!(
                !state.review_open,
                "an acknowledged error must not reopen on reload"
            );
            state.retarget(
                Some(Target {
                    instance: "other".into(),
                    revision: 6,
                }),
                "Other".into(),
                "1.21.11".into(),
                true,
            );
            assert!(state.completed.is_none() && state.error.is_none() && state.message.is_empty());
        }
    }

    #[test]
    fn unavailable_target_keeps_frozen_operation_progress_until_terminal_reply() {
        for applying in [false, true] {
            let mut state = reviewed_state();
            let op = if applying {
                state.confirm_apply().unwrap().op
            } else {
                state.begin_preview().unwrap().op
            };
            let label = state.progress_label().unwrap();
            assert!(label.contains("Instance"));
            state.retarget(None, String::new(), String::new(), false);
            assert!(state.busy() && !state.applicable);
            assert_eq!(state.progress_label().as_deref(), Some(label.as_str()));
            let reply = if applying {
                Reply::Apply {
                    op,
                    result: Ok(instance_summary()),
                }
            } else {
                Reply::Preview {
                    op,
                    result: Ok(preview("upToDate", None, "")),
                }
            };
            let effects = state.receive(reply);
            assert_eq!(effects.busy, Some(false));
            assert_eq!(effects.changed, applying);
            assert!(!state.busy() && state.progress_label().is_none());
            assert!(state.review.is_none() && !state.review_open && state.completed.is_none());
        }
    }

    #[test]
    fn apply_success_settles_and_reports_to_the_parent() {
        let mut state = reviewed_state();
        let call = state.confirm_apply().unwrap();
        let fx = state.receive(Reply::Apply {
            op: call.op,
            result: Ok(instance_summary()),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(fx.changed);
        assert_eq!(state.message, "Aurora Client updated and verified.");
        assert!(state.error.is_none());
        assert!(state.summary.is_none());
        assert!(!state.review_open);
    }

    #[test]
    fn failed_applies_settle_terminal_and_duplicates_change_nothing() {
        let mut state = reviewed_state();
        let call = state.confirm_apply().unwrap();
        let fx = state.receive(Reply::Apply {
            op: call.op,
            result: Err(api::ApiError::new(
                "update_stale",
                "State changed during acquisition.",
            )),
        });
        assert_eq!(fx.busy, Some(false), "failure is still terminal for busy");
        assert!(fx.changed, "failure still tells the parent to reload");
        assert!(state.review_open, "the failure stays acknowledged");
        assert_eq!(
            state.error.as_deref(),
            Some("State changed during acquisition. (update_stale)")
        );

        // A late duplicate of the failed apply cannot disturb newer work.
        let next = state.begin_preview().unwrap();
        assert!(state.busy());
        let fx = state.receive(Reply::Apply {
            op: call.op,
            result: Ok(instance_summary()),
        });
        assert_eq!(fx, Effects::default());
        assert!(state.busy());
        let fx = settle_preview(&mut state, next.op, Ok(preview("upToDate", None, "fp")));
        assert_eq!(fx.busy, Some(false));
    }

    #[test]
    fn retarget_during_apply_keeps_terminal_events_without_current_feedback() {
        for result in [
            Ok(instance_summary()),
            Err(api::ApiError::new("update_transaction_failed", "broke")),
        ] {
            let mut state = reviewed_state();
            let call = state.confirm_apply().unwrap();
            state.retarget(
                Some(Target {
                    instance: "i2".into(),
                    revision: 1,
                }),
                "Other".into(),
                "1.21.11".into(),
                true,
            );
            assert!(state.busy(), "the committed apply is never abandoned");
            let fx = state.receive(Reply::Apply {
                op: call.op,
                result,
            });
            assert_eq!(fx.busy, Some(false));
            assert!(fx.changed, "the parent still learns the terminal state");
            assert!(
                state.message.is_empty() && state.error.is_none() && state.summary.is_none(),
                "old instance feedback never lands under the current target"
            );
            assert!(!state.review_open);
        }
    }

    // ------------------------------------------------------- progress events

    #[test]
    fn progress_labels_route_only_during_the_own_apply() {
        let mut state = reviewed_state();
        let call = state.confirm_apply().unwrap();
        assert!(state.on_host_event("client-update-progress", &json!({"phase": "acquiring"})));
        assert!(
            state
                .in_flight
                .as_ref()
                .is_some_and(|op| op.phase.contains("Downloading and verifying"))
        );
        // A repeated phase changes nothing.
        assert!(!state.on_host_event("client-update-progress", &json!({"phase": "acquiring"})));
        assert!(state.on_host_event("client-update-progress", &json!({"phase": "committing"})));

        // The advisory label never changes the outcome.
        let fx = state.receive(Reply::Apply {
            op: call.op,
            result: Ok(instance_summary()),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(fx.changed);
    }

    #[test]
    fn malformed_or_unrelated_events_are_ignored() {
        let mut state = reviewed_state();
        let _call = state.confirm_apply().unwrap();
        assert!(!state.on_host_event("launcher-update-progress", &json!({"phase": "acquiring"})));
        assert!(!state.on_host_event("client-update-progress", &json!({})));
        assert!(!state.on_host_event("client-update-progress", &json!({"phase": "downloading"})));
        assert!(!state.on_host_event("client-update-progress", &json!({"phase": 3})));
        assert!(!state.on_host_event("client-update-progress", &json!({"phase": "Acquiring"})));
        assert!(
            state
                .in_flight
                .as_ref()
                .is_some_and(|op| op.phase.is_empty())
        );

        // During a preview, progress events are not this view's.
        let mut state = retargeted(Some(target(1)), true);
        let _call = state.begin_preview().unwrap();
        assert!(!state.on_host_event("client-update-progress", &json!({"phase": "acquiring"})));
    }

    #[test]
    fn the_phase_vocabulary_is_the_cores() {
        assert_eq!(
            client_phase_label("acquiring"),
            Some("Downloading and verifying the published release")
        );
        assert_eq!(
            client_phase_label("staging"),
            Some("Staging the verified update")
        );
        assert_eq!(
            client_phase_label("activating"),
            Some("Activating the verified update")
        );
        assert_eq!(
            client_phase_label("validating"),
            Some("Validating the updated instance")
        );
        assert_eq!(
            client_phase_label("committing"),
            Some("Committing the update")
        );
        assert_eq!(client_phase_label("checking"), None);
        assert_eq!(client_phase_label(""), None);
    }
}
