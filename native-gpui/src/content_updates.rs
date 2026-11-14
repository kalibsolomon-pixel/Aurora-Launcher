//! Managed content update and managed-modpack update controls for GPUI.
//!
//! Behavior authority is the Slint controller `native/src/content_updates.rs`
//! over the shared typed boundary `native/src/api.rs`. The core owns update
//! discovery, dependency resolution, pin-policy persistence, modpack
//! three-way reconciliation and every fingerprint re-check; this module only
//! presents those results and freezes the exact reviewed roots, fingerprints
//! and conflict paths into typed requests before work leaves the UI thread.
//! Every asynchronous call goes through [`crate::api`] wrapped in
//! [`crate::backend::guarded`]; no downloads, filesystem mutation or
//! credential work happens here.
//!
//! The view is embedded in the Content page: the parent supplies the exact
//! instance, content kind and revision through
//! [`ContentUpdates::set_target`] and gates its own content controls on
//! [`ContentUpdatesEvent::BusyChanged`]. This module adds no instance or kind
//! selectors and reads no registry itself. Checks, previews and applies each
//! run one at a time and only on explicit activation; constructing the view,
//! retargeting or refreshing never contacts the provider.
//!
//! Differences from the Slint host are deliberate and disclosed: every
//! reviewed update (even a plain replacement) is confirmed in an explicit
//! dialog before apply, and there is no Downloads queue module in the GPUI
//! host yet, so a single active transaction is disclosed instead of a queue.
//! A pinned entry can also be unpinned without a current update offer, so
//! users can restore its eligibility for future updates at any time.

use aurora_launcher_lib::content_jobs::{Control, Phase as JobPhase};
use aurora_launcher_lib::host::Host;
use gpui::{
    Context, EventEmitter, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use gpui_kit::base::Dialog;
use gpui_kit::prelude::FluentBuilder as _;
use serde_json::Value;
use tokio::runtime::Handle;

use crate::api;
use crate::backend::guarded;
use crate::controls::{AuroraButton, AuroraToggle, ButtonSize, ButtonVariant};
use crate::theme;
use crate::ui;

/// Emitted after any mutation settles (including failures, which may retain
/// instance state) so the parent reloads content, and exactly when the
/// single-operation busy state transitions, so the parent can gate its own
/// content controls. Reads, cancels and dismissals alone never emit `Changed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentUpdatesEvent {
    Changed,
    BusyChanged(bool),
}

/// The frozen target the parent owns: exact instance, content kind and the
/// parent's content revision. Replies apply only while all three still match.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Target {
    instance: String,
    kind: &'static str,
    revision: u64,
}

/// Which availability check a mutation settle asks the view to repeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Recheck {
    Content,
    Pack,
}

/// Follow-up work a state transition asks the view to perform.
#[derive(Default, Debug, PartialEq)]
struct Effects {
    notify: bool,
    changed: bool,
    busy: Option<bool>,
    recheck: Option<Recheck>,
}

/// One running operation; exactly one at a time. The serial is retained until
/// terminal completion, so a stale duplicate reply can never clear newer busy
/// state. Previews cancel their staging transfers cooperatively; applies
/// settle their active writes into the verified cache.
struct Op {
    serial: u64,
    epoch: u64,
    kind: OpKind,
    target: Target,
    control: Option<Control>,
    cancelling: bool,
    phase: String,
}

#[derive(Clone, Debug)]
enum OpKind {
    Check,
    Pin { project: String, pinned: bool },
    Preview { serial: u64 },
    Apply { serial: u64 },
    PackCheck,
    PackPreview { serial: u64 },
    PackApply { serial: u64 },
}

/// The frozen reviewed content-update snapshot. Display is never authority:
/// apply re-resolves these exact roots and fingerprint in the core.
#[derive(Clone)]
struct UpdateReview {
    target: Target,
    roots: Vec<String>,
    fingerprint: String,
    text: String,
}

/// The frozen reviewed modpack plan snapshot.
#[derive(Clone)]
struct PackReview {
    target: Target,
    fingerprint: String,
    conflicts: Vec<String>,
    text: String,
}

// ------------------------------------------------------------- frozen calls

struct CheckCall {
    op: u64,
    target: Target,
}

struct PinCall {
    op: u64,
    target: Target,
    project: String,
}

struct PreviewCall {
    op: u64,
    target: Target,
    roots: Vec<String>,
    serial: u64,
    control: Control,
}

struct ApplyCall {
    op: u64,
    target: Target,
    roots: Vec<String>,
    fingerprint: String,
    serial: u64,
    control: Control,
}

struct PackCheckCall {
    op: u64,
    target: Target,
}

struct PackPreviewCall {
    op: u64,
    target: Target,
    serial: u64,
    control: Control,
}

struct PackApplyCall {
    op: u64,
    target: Target,
    fingerprint: String,
    resolutions: Vec<(String, String)>,
    serial: u64,
    control: Control,
}

/// One completion arriving from the Tokio runtime. The operation serial and
/// review serial carry the guards: a reply is applied only while it still
/// describes current state.
enum Reply {
    Check {
        op: u64,
        result: api::ApiResult<api::UpdatesReport>,
    },
    Pin {
        op: u64,
        result: api::ApiResult<Value>,
    },
    Preview {
        op: u64,
        serial: u64,
        roots: Vec<String>,
        observed_cancellation: bool,
        result: api::ApiResult<Value>,
    },
    Apply {
        op: u64,
        serial: u64,
        observed_cancellation: bool,
        result: api::ApiResult<Value>,
    },
    PackCheck {
        op: u64,
        result: api::ApiResult<Value>,
    },
    PackPreview {
        op: u64,
        serial: u64,
        observed_cancellation: bool,
        result: api::ApiResult<Value>,
    },
    PackApply {
        op: u64,
        serial: u64,
        observed_cancellation: bool,
        result: api::ApiResult<Value>,
    },
}

fn reply_op(reply: &Reply) -> u64 {
    match reply {
        Reply::Check { op, .. }
        | Reply::Pin { op, .. }
        | Reply::Preview { op, .. }
        | Reply::Apply { op, .. }
        | Reply::PackCheck { op, .. }
        | Reply::PackPreview { op, .. }
        | Reply::PackApply { op, .. } => *op,
    }
}

/// A reply settles only the same-shape operation it was started for, with a
/// matching review epoch where one was frozen.
fn reply_matches_op(op: &Op, reply: &Reply) -> bool {
    match (&op.kind, reply) {
        (OpKind::Check, Reply::Check { .. })
        | (OpKind::Pin { .. }, Reply::Pin { .. })
        | (OpKind::PackCheck, Reply::PackCheck { .. }) => true,
        (OpKind::Preview { serial }, Reply::Preview { serial: reply, .. })
        | (OpKind::Apply { serial }, Reply::Apply { serial: reply, .. })
        | (OpKind::PackPreview { serial }, Reply::PackPreview { serial: reply, .. })
        | (OpKind::PackApply { serial }, Reply::PackApply { serial: reply, .. }) => {
            *serial == *reply
        }
        _ => false,
    }
}

// --------------------------------------------------------------- pure state

/// Presentation state and every guard, with no GPUI dependency so the state
/// transitions are directly testable.
#[derive(Default)]
struct UpdatesState {
    target: Option<Target>,
    instance_name: String,
    /// `Some` iff the parent's instance was created from a managed modpack.
    pack_name: Option<String>,
    op_serial: u64,
    epoch: u64,
    review_serial: u64,
    in_flight: Option<Op>,
    message: String,
    error: Option<String>,
    // managed content availability
    entries: Vec<api::UpdateEntry>,
    dependency_managed: usize,
    checked: bool,
    review: Option<UpdateReview>,
    review_open: bool,
    // managed modpack
    pack_status: Option<(String, bool)>,
    pack_review: Option<PackReview>,
    pack_keep_local: bool,
}

impl UpdatesState {
    fn busy(&self) -> bool {
        self.in_flight.is_some()
    }

    fn is_pack_instance(&self) -> bool {
        self.pack_name.is_some()
    }

    fn current(&self, target: &Target) -> bool {
        self.target.as_ref() == Some(target)
    }

    fn eligible_projects(&self) -> Vec<String> {
        let mut projects: Vec<String> = self
            .entries
            .iter()
            .filter(|e| eligible(e))
            .map(|e| e.project_id.clone())
            .collect();
        projects.sort();
        projects
    }

    /// Installs one operation, guarding the one-at-a-time rule.
    fn begin_op(&mut self, kind: OpKind, control: Option<Control>, phase: &str) -> Option<u64> {
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
            control,
            cancelling: false,
            phase: phase.to_owned(),
        });
        Some(serial)
    }

    /// Explicit availability check for the target's content kind.
    fn begin_check(&mut self) -> Option<CheckCall> {
        if self.busy() || self.target.is_none() {
            return None;
        }
        self.clear_review_display();
        let target = self.target.clone()?;
        let op = self.begin_op(OpKind::Check, None, "")?;
        Some(CheckCall { op, target })
    }

    /// Persists one managed record's pin policy. Pin holds back an offered
    /// update; unpin re-enables updating. Files never change here.
    fn begin_pin(&mut self, project: &str, pinned: bool) -> Option<PinCall> {
        if self.busy() || !self.checked {
            return None;
        }
        let entry = self.entries.iter().find(|e| e.project_id == project)?;
        let allowed = if pinned {
            !entry.pinned && eligible(entry)
        } else {
            entry.pinned
        };
        if !allowed {
            return None;
        }
        let target = self.target.clone()?;
        let op = self.begin_op(
            OpKind::Pin {
                project: project.to_owned(),
                pinned,
            },
            None,
            "",
        )?;
        Some(PinCall {
            op,
            target,
            project: project.to_owned(),
        })
    }

    /// Freezes one coherent preview for the exact requested roots (`None`
    /// means every eligible project), sorted for determinism.
    fn begin_review_updates(&mut self, projects: Option<Vec<String>>) -> Option<PreviewCall> {
        if self.busy() || !self.checked {
            return None;
        }
        let roots = match projects {
            Some(projects) => {
                let mut roots: Vec<String> = projects
                    .into_iter()
                    .filter(|p| {
                        self.entries
                            .iter()
                            .any(|e| e.project_id == *p && eligible(e))
                    })
                    .collect();
                roots.sort();
                roots
            }
            None => self.eligible_projects(),
        };
        if roots.is_empty() {
            return None;
        }
        self.clear_review_display();
        let target = self.target.clone()?;
        self.review_serial = self.review_serial.wrapping_add(1);
        let serial = self.review_serial;
        // Read-only discovery: cancellation also stops its staging downloads.
        let control = Control::interrupting_transfers();
        let op = self.begin_op(OpKind::Preview { serial }, Some(control.clone()), "")?;
        Some(PreviewCall {
            op,
            target,
            roots,
            serial,
            control,
        })
    }

    /// Consumes the completed review into the exact apply the core will
    /// re-resolve and fingerprint-check.
    fn confirm_apply(&mut self) -> Option<ApplyCall> {
        if self.busy() {
            return None;
        }
        let review = self.review.clone()?;
        let target = self.target.clone()?;
        if review.target != target || review.fingerprint.is_empty() || review.roots.is_empty() {
            return None;
        }
        self.review = None;
        // The dialog stays open while applying; settles close it.
        // Mutations settle their active transfers into the verified cache.
        let control = Control::default();
        let op = self.begin_op(
            OpKind::Apply {
                serial: self.review_serial,
            },
            Some(control.clone()),
            "",
        )?;
        Some(ApplyCall {
            op,
            target,
            roots: review.roots,
            fingerprint: review.fingerprint,
            serial: self.review_serial,
            control,
        })
    }

    /// Pack-level availability check; only a managed modpack instance has one.
    fn begin_pack_check(&mut self) -> Option<PackCheckCall> {
        if self.busy() || !self.is_pack_instance() || self.target.is_none() {
            return None;
        }
        self.clear_review_display();
        let target = self.target.clone()?;
        let op = self.begin_op(OpKind::PackCheck, None, "")?;
        Some(PackCheckCall { op, target })
    }

    fn begin_pack_review(&mut self) -> Option<PackPreviewCall> {
        if self.busy()
            || !self.is_pack_instance()
            || !self
                .pack_status
                .as_ref()
                .is_some_and(|(_, available)| *available)
        {
            return None;
        }
        self.clear_review_display();
        let target = self.target.clone()?;
        self.review_serial = self.review_serial.wrapping_add(1);
        let serial = self.review_serial;
        let control = Control::interrupting_transfers();
        let op = self.begin_op(OpKind::PackPreview { serial }, Some(control.clone()), "")?;
        Some(PackPreviewCall {
            op,
            target,
            serial,
            control,
        })
    }

    /// Keeping local copies is the only conflict resolution offered; it is
    /// valid for every conflict kind and never overwrites user bytes.
    fn confirm_pack_apply(&mut self) -> Option<PackApplyCall> {
        if self.busy() {
            return None;
        }
        let review = self.pack_review.clone()?;
        let target = self.target.clone()?;
        if review.target != target || review.fingerprint.is_empty() {
            return None;
        }
        if !review.conflicts.is_empty() && !self.pack_keep_local {
            return None;
        }
        self.pack_review = None;
        let resolutions: Vec<(String, String)> = review
            .conflicts
            .iter()
            .map(|path| (path.clone(), "keepLocal".to_owned()))
            .collect();
        let control = Control::default();
        let op = self.begin_op(
            OpKind::PackApply {
                serial: self.review_serial,
            },
            Some(control.clone()),
            "Applying the modpack update…",
        )?;
        Some(PackApplyCall {
            op,
            target,
            fingerprint: review.fingerprint,
            resolutions,
            serial: self.review_serial,
            control,
        })
    }

    /// Cooperative cancel of the one running operation, where the core
    /// supports it. The operation id is retained until terminal completion; a
    /// bump of the review serial discards a cancelled preview's late result.
    /// Checks and pin writes have no cancellation path and simply settle.
    fn cancel_in_flight(&mut self) -> bool {
        let Some(op) = &mut self.in_flight else {
            return false;
        };
        if op.cancelling {
            return false;
        }
        let Some(control) = op.control.clone() else {
            return false;
        };
        if control.phase() == JobPhase::Cancelling {
            return false;
        }
        control.cancel();
        op.cancelling = true;
        op.phase.clear();
        if matches!(op.kind, OpKind::Preview { .. } | OpKind::PackPreview { .. }) {
            self.review_serial = self.review_serial.wrapping_add(1);
        }
        true
    }

    /// Drops the open review dialog's content without touching any check
    /// data. Dismissal is not a mutation and emits nothing.
    fn clear_review_display(&mut self) {
        if self.review.is_some() || self.pack_review.is_some() || self.review_open {
            self.review_serial = self.review_serial.wrapping_add(1);
        }
        self.review = None;
        self.pack_review = None;
        self.pack_keep_local = false;
        self.review_open = false;
    }

    /// Clears check and review state; returns whether anything was dropped.
    /// Never touches an active operation: a running mutation future cannot be
    /// abandoned, and its terminal settle still reports to the parent.
    fn invalidate(&mut self) -> bool {
        self.epoch = self.epoch.wrapping_add(1);
        let had = self.checked
            || self.review.is_some()
            || self.pack_status.is_some()
            || self.pack_review.is_some();
        self.entries.clear();
        self.dependency_managed = 0;
        self.checked = false;
        self.clear_review_display();
        self.pack_status = None;
        had
    }

    /// Adopts the parent's exact target. A changed target, kind, revision or
    /// pack identity invalidates every read; an unchanged target keeps the
    /// current check so a parent re-render never discards fresh results.
    fn retarget(
        &mut self,
        target: Option<Target>,
        instance_name: String,
        pack_name: Option<String>,
    ) {
        let changed = self.target != target || self.pack_name != pack_name;
        self.instance_name = instance_name;
        self.pack_name = pack_name;
        self.target = target;
        if changed {
            self.invalidate();
            self.message.clear();
            self.error = None;
        }
    }

    /// A `modpack-update-progress` event, attributed to the modpack apply
    /// this view runs; ignored once cancelling and by every other operation.
    fn apply_pack_phase(&mut self, phase: &str) -> bool {
        let Some(op) = &mut self.in_flight else {
            return false;
        };
        if !matches!(op.kind, OpKind::PackApply { .. }) || op.cancelling {
            return false;
        }
        let label = pack_phase_label(phase).unwrap_or("Applying the modpack update");
        let next = format!("{label}…");
        if op.phase == next {
            return false;
        }
        op.phase = next;
        true
    }

    fn on_host_event(&mut self, name: &str, payload: &Value) -> bool {
        name == "modpack-update-progress"
            && payload["phase"]
                .as_str()
                .is_some_and(|phase| self.apply_pack_phase(phase))
    }

    /// Applies one async completion; stale or duplicate messages change
    /// nothing, including busy state owned by a newer operation.
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
        if !still_current {
            fx.changed = matches!(
                op.kind,
                OpKind::Pin { .. } | OpKind::Apply { .. } | OpKind::PackApply { .. }
            );
            return fx;
        }
        const CANCELLED: &str =
            "Cancelled. Active writes finished safely; nothing uncommitted was activated.";
        match reply {
            Reply::Check { result, .. } => {
                if !still_current {
                    return fx;
                }
                match result {
                    Ok(report) => {
                        self.entries = report.entries;
                        self.dependency_managed = report.dependency_managed;
                        self.checked = true;
                        self.error = None;
                        self.message.clear();
                    }
                    Err(e) => {
                        self.entries.clear();
                        self.dependency_managed = 0;
                        self.checked = false;
                        self.message.clear();
                        self.error = Some(format!("Update check failed: {}", error_text(&e)));
                    }
                }
            }
            Reply::Pin { result, .. } => {
                let (pinned, project) = match &op.kind {
                    OpKind::Pin { pinned, project } => (*pinned, project.clone()),
                    _ => (false, String::new()),
                };
                fx.changed = true;
                match result {
                    Ok(_) => {
                        self.error = None;
                        self.message = if pinned {
                            format!(
                                "Pinned {project}. It stays on its current version until unpinned."
                            )
                        } else {
                            format!("Unpinned {project}. It can now be updated.")
                        };
                        if still_current {
                            self.checked = false;
                            self.entries.clear();
                            fx.recheck = Some(Recheck::Content);
                        }
                    }
                    Err(e) => {
                        // The policy write failed; the checked data still
                        // describes the instance.
                        self.message.clear();
                        self.error = Some(error_text(&e));
                    }
                }
            }
            Reply::Preview {
                serial,
                roots,
                observed_cancellation,
                result,
                ..
            } => {
                self.review_open = false;
                if observed_cancellation {
                    if still_current {
                        self.message = "Review cancelled. Nothing was updated.".into();
                    }
                    return fx;
                }
                // Guard 2: the review epoch and target must still be current.
                if serial != self.review_serial || !still_current {
                    return fx;
                }
                match result {
                    Ok(value) => match describe_update_preview(&value) {
                        Some((fingerprint, text)) => {
                            self.error = None;
                            self.message.clear();
                            self.review = Some(UpdateReview {
                                target: op.target,
                                roots,
                                fingerprint,
                                text,
                            });
                            self.review_open = true;
                        }
                        None => {
                            self.message.clear();
                            self.error =
                                Some("The update service did not provide a valid review.".into());
                        }
                    },
                    Err(e) => {
                        self.message.clear();
                        self.error = Some(error_text(&e));
                    }
                }
            }
            Reply::Apply {
                observed_cancellation,
                result,
                ..
            } => {
                fx.changed = true;
                self.review_open = false;
                match result {
                    Ok(_) => {
                        self.error = None;
                        self.message = "Updates applied and verified.".into();
                    }
                    Err(e) => {
                        self.message.clear();
                        self.error = Some(if observed_cancellation {
                            CANCELLED.to_owned()
                        } else {
                            error_text(&e)
                        });
                    }
                }
                if still_current {
                    self.checked = false;
                    self.entries.clear();
                    fx.recheck = Some(Recheck::Content);
                }
            }
            Reply::PackCheck { result, .. } => {
                if !still_current {
                    return fx;
                }
                match result {
                    Ok(value) => {
                        self.error = None;
                        self.message.clear();
                        self.pack_status = Some(pack_check_summary(&value));
                    }
                    Err(e) => {
                        self.pack_status = None;
                        self.message.clear();
                        self.error = Some(error_text(&e));
                    }
                }
            }
            Reply::PackPreview {
                serial,
                observed_cancellation,
                result,
                ..
            } => {
                self.review_open = false;
                if observed_cancellation {
                    if still_current {
                        self.message = "Review cancelled. Nothing was updated.".into();
                    }
                    return fx;
                }
                if serial != self.review_serial || !still_current {
                    return fx;
                }
                match result {
                    Ok(value) => match pack_plan_review(&value) {
                        Some((text, conflicts)) => {
                            let fingerprint =
                                value["fingerprint"].as_str().unwrap_or("").to_owned();
                            self.error = None;
                            self.message.clear();
                            self.pack_keep_local = false;
                            self.pack_review = Some(PackReview {
                                target: op.target,
                                fingerprint,
                                conflicts,
                                text,
                            });
                            self.review_open = true;
                        }
                        None => {
                            self.message.clear();
                            self.error = Some(
                                "The modpack update plan is not valid. Review it again.".into(),
                            );
                        }
                    },
                    Err(e) => {
                        self.message.clear();
                        self.error = Some(error_text(&e));
                    }
                }
            }
            Reply::PackApply {
                observed_cancellation,
                result,
                ..
            } => {
                fx.changed = true;
                self.review_open = false;
                match result {
                    Ok(_) => {
                        self.error = None;
                        self.message = "Modpack update applied and verified.".into();
                    }
                    Err(e) => {
                        self.message.clear();
                        self.error = Some(if observed_cancellation {
                            CANCELLED.to_owned()
                        } else {
                            error_text(&e)
                        });
                    }
                }
                if still_current {
                    self.pack_status = None;
                    fx.recheck = Some(Recheck::Pack);
                }
            }
        }
        fx
    }
}

// --------------------------------------------------------------- pure helpers

/// Legal managed content kinds for this view.
fn legal_kind(kind: &str) -> bool {
    matches!(kind, "mod" | "resourcePack" | "shaderPack")
}

fn kind_noun(kind: &str) -> &'static str {
    match kind {
        "mod" => "mods",
        "resourcePack" => "resource packs",
        "shaderPack" => "shader packs",
        _ => "content",
    }
}

/// Updatable now: a compatible candidate under policy, not pinned. Blocked,
/// disabled, locally modified, missing, conflicting and provider-owned states
/// never qualify; the core's status is the only authority.
fn eligible(entry: &api::UpdateEntry) -> bool {
    entry.status == "updateAvailable" && !entry.pinned && entry.candidate.is_some()
}

fn block_label(block: Option<&str>) -> &'static str {
    match block {
        Some("packOwned") => "Managed by the modpack",
        Some("disabled") => "Disabled — enable to update",
        Some("locallyModified") => "Changed locally — not updated",
        Some("missing") => "File missing",
        Some("conflict") => "File conflict",
        _ => "Update blocked",
    }
}

fn status_label(entry: &api::UpdateEntry) -> String {
    let candidate = entry
        .candidate
        .as_ref()
        .map(|c| c.version_number.clone())
        .unwrap_or_default();
    let base = match entry.status.as_str() {
        "updateAvailable" if entry.pinned => format!("Pinned · {candidate} available"),
        "updateAvailable" => format!("Update: {candidate}"),
        "pinnedUpdateAvailable" => format!("Pinned · {candidate} available"),
        "upToDate" => "Up to date".to_owned(),
        "noNewerUnderPolicy" => "No update in its release channel".to_owned(),
        "currentVersionUnknown" => "Version unknown to Modrinth".to_owned(),
        "providerUnavailable" => "Check failed".to_owned(),
        "blocked" => block_label(entry.block.as_deref()).to_owned(),
        _ => "Update status unknown".to_owned(),
    };
    match entry.detail.as_deref().filter(|d| !d.is_empty()) {
        Some(detail) => format!("{base}. {detail}"),
        None => base,
    }
}

/// "3 updates available · 1 pinned · 8 up to date".
fn summarize(entries: &[api::UpdateEntry], dependencies: usize) -> String {
    let count = |status: &str| entries.iter().filter(|e| e.status == status).count();
    let available = entries.iter().filter(|e| eligible(e)).count();
    let pinned = entries
        .iter()
        .filter(|e| e.pinned && e.candidate.is_some())
        .count();
    let current = count("upToDate") + count("noNewerUnderPolicy");
    let blocked = count("blocked") + count("currentVersionUnknown");
    let failed = count("providerUnavailable");
    if entries.is_empty() && dependencies == 0 {
        return "Nothing here is managed from Modrinth, so there is nothing to update.".into();
    }
    let mut parts = Vec::new();
    parts.push(match available {
        0 => "No updates available".to_owned(),
        1 => "1 update available".to_owned(),
        n => format!("{n} updates available"),
    });
    if pinned > 0 {
        parts.push(format!("{pinned} pinned"));
    }
    if current > 0 {
        parts.push(format!("{current} up to date"));
    }
    if blocked > 0 {
        parts.push(format!("{blocked} need attention"));
    }
    if failed > 0 {
        parts.push(format!("{failed} could not be checked"));
    }
    if dependencies > 0 {
        parts.push(format!("{dependencies} dependencies follow their parents"));
    }
    parts.join(" · ")
}

fn error_text(e: &api::ApiError) -> String {
    format!("{} ({})", e.message, e.code)
}

fn item_name(item: &Value) -> String {
    let file = item["fileName"].as_str().unwrap_or("Content");
    match item["displayVersion"].as_str() {
        Some(version) => format!("{file} ({version})"),
        None => file.to_owned(),
    }
}

/// The core-issued bulk preview, presented as review text. `None` means the
/// shape is malformed (no fingerprint, or targets is not an array); a
/// malformed preview can never enable Apply.
fn describe_update_preview(value: &Value) -> Option<(String, String)> {
    let fingerprint = value["previewFingerprint"].as_str().unwrap_or("");
    let targets = value["targets"].as_array()?;
    if fingerprint.is_empty() {
        return None;
    }
    let list = |key: &str| value["delta"][key].as_array().cloned().unwrap_or_default();
    let mut text = String::from("Updates\n");
    for target in targets {
        text.push_str(&format!(
            "{} → {}\n",
            item_name(&target["current"]),
            target["candidate"]["versionNumber"]
                .as_str()
                .unwrap_or("new version"),
        ));
    }
    let added = list("newRequirements");
    let removed = list("removedRequirements");
    let warnings = value["warnings"].as_array().cloned().unwrap_or_default();
    if !added.is_empty() {
        text.push_str("\nNew dependencies installed\n");
        for item in &added {
            text.push_str(&format!("{}\n", item_name(item)));
        }
    }
    if !removed.is_empty() {
        text.push_str("\nDependencies no longer required (removed)\n");
        for item in &removed {
            text.push_str(&format!("{}\n", item_name(item)));
        }
    }
    if !warnings.is_empty() {
        text.push_str("\nWarnings\n");
        for warning in &warnings {
            text.push_str(&format!(
                "{}\n",
                warning.as_str().unwrap_or("Review warning")
            ));
        }
    }
    Some((fingerprint.to_owned(), text))
}

/// Pack-level check summary, from the core's `PackUpdateCheck` shape.
fn pack_check_summary(value: &Value) -> (String, bool) {
    let current = value["currentPackVersion"].as_str().unwrap_or("");
    let modified = match value["divergences"].as_u64().unwrap_or(0) {
        0 => String::new(),
        n => format!(" ({n} local change(s) recorded)"),
    };
    match value["status"].as_str().unwrap_or("") {
        "updateAvailable" => {
            let candidate = value["candidate"]["versionNumber"]
                .as_str()
                .unwrap_or("A new version");
            (
                format!("{candidate} available (installed {current}){modified}"),
                true,
            )
        }
        "upToDate" => (format!("{current} is the newest version{modified}"), false),
        "blocked" => (
            value["blockedReason"]
                .as_str()
                .unwrap_or("This modpack cannot be updated right now.")
                .to_owned(),
            false,
        ),
        "currentVersionUnknown" => (
            "Modrinth no longer recognizes the installed pack version.".into(),
            false,
        ),
        _ => (
            "The modpack update check could not reach Modrinth.".into(),
            false,
        ),
    }
}

/// The three-way modpack plan as review text plus its conflict paths.
/// `None` means the plan is malformed; a malformed plan can never enable
/// Apply.
fn pack_plan_review(value: &Value) -> Option<(String, Vec<String>)> {
    let s = |key: &str| value[key].as_str().unwrap_or("");
    let n = |key: &str| value["counts"][key].as_u64().unwrap_or(0);
    if s("fingerprint").is_empty() || s("name").is_empty() {
        return None;
    }
    let (current, candidate) = (s("currentPackVersion"), s("candidatePackVersion"));
    if current.is_empty() || candidate.is_empty() {
        return None;
    }
    let rows = value["rows"].as_array()?;
    let mut text = format!("{} {current} → {candidate}\n", s("name"));
    if value["gameTransition"] == Value::Bool(true) {
        text.push_str(&format!(
            "Minecraft {} → {} · loader {} → {}. The game is reinstalled for the new version.\n",
            s("minecraftCurrent"),
            s("minecraftCandidate"),
            s("loaderCurrent"),
            s("loaderCandidate")
        ));
    } else {
        text.push_str(&format!(
            "Minecraft {} · loader {} (unchanged)\n",
            s("minecraftCurrent"),
            s("loaderCurrent")
        ));
    }
    text.push_str(&format!(
        "\n{} added · {} updated · {} removed · {} kept as is\n",
        n("added"),
        n("updated"),
        n("removed"),
        n("preserved")
    ));
    let conflicts: Vec<String> = rows
        .iter()
        .filter(|row| row["action"] == "conflict")
        .filter_map(|row| row["path"].as_str().map(str::to_owned))
        .collect();
    if !conflicts.is_empty() {
        text.push_str(
            "\nYou changed these files and the new version changes them too. Your copies are kept; the pack's new copy is not written over them.\n",
        );
        for path in &conflicts {
            text.push_str(&format!("{path}\n"));
        }
    }
    let unselected = value["newOptionalUnselected"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !unselected.is_empty() {
        text.push_str("\nNew optional files (not installed)\n");
        for path in unselected {
            text.push_str(&format!("{}\n", path.as_str().unwrap_or("")));
        }
    }
    if let Some(changelog) = value["changelog"].as_str().filter(|c| !c.trim().is_empty()) {
        let short: String = changelog.chars().take(1200).collect();
        text.push_str(&format!("\nChangelog\n{short}\n"));
    }
    Some((text, conflicts))
}

/// Labels for the core's modpack progress vocabulary; `None` for a name the
/// core does not emit.
fn pack_phase_label(phase: &str) -> Option<&'static str> {
    use aurora_launcher_lib::modpacks::progress_phase as p;
    Some(match phase {
        p::RESOLVING_PACK => "Verifying the exact modpack version",
        p::DOWNLOADING => "Downloading and verifying pack files",
        p::PREPARING_INSTANCE => "Preparing the new instance",
        p::INSTALLING_GAME => "Installing Minecraft and the mod loader",
        p::INSTALLING_OVERRIDES => "Writing pack files",
        p::INSTALLING_COMPONENTS => "Installing verified pack components",
        p::VALIDATING => "Validating the installation",
        p::COMPLETE => "Finishing the installation",
        _ => return None,
    })
}

fn op_label(op: &Op) -> String {
    if op.cancelling {
        return "Cancelling… Active writes and any commit in progress must finish safely.".into();
    }
    match &op.kind {
        OpKind::Check => "Checking for updates…".into(),
        OpKind::Pin { .. } => "Saving the update policy…".into(),
        OpKind::Preview { .. } => "Preparing the update review…".into(),
        OpKind::Apply { .. } => "Applying the reviewed updates…".into(),
        OpKind::PackCheck => "Checking for a modpack update…".into(),
        OpKind::PackPreview { .. } => "Preparing the modpack update review…".into(),
        OpKind::PackApply { .. } => {
            if op.phase.is_empty() {
                "Applying the modpack update…".into()
            } else {
                op.phase.clone()
            }
        }
    }
}

// ------------------------------------------------------------------ the view

pub struct ContentUpdates {
    state: UpdatesState,
    host: Option<Host>,
    runtime: Handle,
    tx: async_channel::Sender<Reply>,
    blocked: bool,
}

impl EventEmitter<ContentUpdatesEvent> for ContentUpdates {}

impl ContentUpdates {
    /// Builds the view. `host: None` constructs the diagnostic view that
    /// makes no provider, credential or integration calls. Constructing the
    /// view never checks for updates; the explicit Check button does.
    pub fn new(
        host: Option<Host>,
        runtime: Handle,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded();
        // Event-driven pump: completions arrive as messages; nothing polls.
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

    /// Adopts the parent's exact instance, content kind and content revision.
    /// This never initiates a check, download or mutation; a changed target
    /// clears stale check and review state and waits for explicit activation.
    /// A kind outside mod/resourcePack/shaderPack renders as unsupported.
    pub fn set_target(
        &mut self,
        instance: Option<api::InstanceSummary>,
        kind: &'static str,
        revision: u64,
        cx: &mut Context<Self>,
    ) {
        let (target, name, pack) = match instance {
            Some(instance) => {
                let target = legal_kind(kind).then(|| Target {
                    instance: instance.id,
                    kind,
                    revision,
                });
                (target, instance.display_name, instance.pack.map(|p| p.name))
            }
            None => (None, String::new(), None),
        };
        self.state.retarget(target, name, pack);
        cx.notify();
    }

    /// Clears check and review state without any provider call: the parent
    /// invokes this after content changed elsewhere so stale badges never
    /// describe superseded files. An explicit Check re-reads availability.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.state.invalidate() {
            self.state.message =
                "Content changed since the last check. Check again for current updates.".into();
        }
        cx.notify();
    }

    /// Routes one host event. Only `modpack-update-progress` concerns this view and
    /// only while its own modpack apply runs; unknown names and malformed
    /// payloads are ignored, never fatal.
    pub fn on_host_event(&mut self, name: &str, payload: &Value, cx: &mut Context<Self>) {
        if self.state.on_host_event(name, payload) {
            cx.notify();
        }
    }

    /// True while a check, preview, pin write or apply is running. Exactly
    /// one operation runs at a time; there is no queue.
    pub fn is_busy(&self) -> bool {
        self.state.busy()
    }

    /// Blocks starting work while the parent is changing content or preparing
    /// a local import. Cancellation and closing a review remain available.
    pub fn set_blocked(&mut self, blocked: bool, cx: &mut Context<Self>) {
        if self.blocked != blocked {
            self.blocked = blocked;
            cx.notify();
        }
    }

    fn apply(&mut self, fx: Effects, cx: &mut Context<Self>) {
        if let Some(busy) = fx.busy {
            cx.emit(ContentUpdatesEvent::BusyChanged(busy));
        }
        if fx.changed {
            cx.emit(ContentUpdatesEvent::Changed);
        }
        match fx.recheck {
            Some(Recheck::Content) => self.start_check(cx),
            Some(Recheck::Pack) => self.start_pack_check(cx),
            None => {}
        }
        if fx.notify {
            cx.notify();
        }
    }

    // ---------------------------------------------------------------- actions

    fn start_check(&mut self, cx: &mut Context<Self>) {
        if self.blocked {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let was_busy = self.state.busy();
        let Some(call) = self.state.begin_check() else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        let kind = call.target.kind;
        let instance = call.target.instance.clone();
        self.runtime.spawn(async move {
            let result = guarded(api::check_updates(host, instance, kind)).await;
            let _ = tx
                .send(Reply::Check {
                    op: call.op,
                    result,
                })
                .await;
        });
        if !was_busy {
            cx.emit(ContentUpdatesEvent::BusyChanged(true));
        }
        cx.notify();
    }

    fn start_pin(&mut self, project: String, pinned: bool, cx: &mut Context<Self>) {
        if self.blocked {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let was_busy = self.state.busy();
        let Some(call) = self.state.begin_pin(&project, pinned) else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        let kind = call.target.kind;
        let instance = call.target.instance.clone();
        let project = call.project.clone();
        self.runtime.spawn(async move {
            let result = guarded(api::set_update_pinned(
                host, instance, kind, project, pinned,
            ))
            .await;
            let _ = tx
                .send(Reply::Pin {
                    op: call.op,
                    result,
                })
                .await;
        });
        if !was_busy {
            cx.emit(ContentUpdatesEvent::BusyChanged(true));
        }
        cx.notify();
    }

    fn start_review_updates(&mut self, projects: Option<Vec<String>>, cx: &mut Context<Self>) {
        if self.blocked {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let was_busy = self.state.busy();
        let Some(call) = self.state.begin_review_updates(projects) else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        let kind = call.target.kind;
        let instance = call.target.instance.clone();
        let roots = call.roots.clone();
        let control = call.control.clone();
        self.runtime.spawn(async move {
            let run = control.clone();
            let result = guarded(async move {
                run.run(async move { api::preview_updates(host, instance, kind, roots).await })
                    .await
            })
            .await;
            let observed_cancellation = control.cancellation_observed();
            let _ = tx
                .send(Reply::Preview {
                    op: call.op,
                    serial: call.serial,
                    roots: call.roots,
                    observed_cancellation,
                    result,
                })
                .await;
        });
        if !was_busy {
            cx.emit(ContentUpdatesEvent::BusyChanged(true));
        }
        cx.notify();
    }

    fn start_apply(&mut self, cx: &mut Context<Self>) {
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
        let kind = call.target.kind;
        let instance = call.target.instance.clone();
        let roots = call.roots.clone();
        let fingerprint = call.fingerprint.clone();
        let control = call.control.clone();
        self.runtime.spawn(async move {
            let run = control.clone();
            let check = control.clone();
            let result = guarded(async move {
                run.run(async move {
                    if aurora_launcher_lib::content_jobs::checkpoint().is_err() {
                        return Err(api::ApiError::new(
                            "update_cancelled",
                            "Update cancelled before it started.",
                        ));
                    }
                    let result = api::apply_updates(host, instance, kind, roots, fingerprint).await;
                    match result {
                        Err(_) if check.cancellation_observed() => {
                            Err(api::ApiError::new("update_cancelled", "Update cancelled."))
                        }
                        other => other,
                    }
                })
                .await
            })
            .await;
            let observed_cancellation = control.cancellation_observed();
            let _ = tx
                .send(Reply::Apply {
                    op: call.op,
                    serial: call.serial,
                    observed_cancellation,
                    result,
                })
                .await;
        });
        if !was_busy {
            cx.emit(ContentUpdatesEvent::BusyChanged(true));
        }
        cx.notify();
    }

    fn start_pack_check(&mut self, cx: &mut Context<Self>) {
        if self.blocked {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let was_busy = self.state.busy();
        let Some(call) = self.state.begin_pack_check() else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        let instance = call.target.instance.clone();
        self.runtime.spawn(async move {
            let result = guarded(api::check_modpack_update(host, instance)).await;
            let _ = tx
                .send(Reply::PackCheck {
                    op: call.op,
                    result,
                })
                .await;
        });
        if !was_busy {
            cx.emit(ContentUpdatesEvent::BusyChanged(true));
        }
        cx.notify();
    }

    fn start_pack_review(&mut self, cx: &mut Context<Self>) {
        if self.blocked {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let was_busy = self.state.busy();
        let Some(call) = self.state.begin_pack_review() else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        let instance = call.target.instance.clone();
        let control = call.control.clone();
        self.runtime.spawn(async move {
            let run = control.clone();
            let result = guarded(async move {
                run.run(async move { api::preview_modpack_update(host, instance).await })
                    .await
            })
            .await;
            let observed_cancellation = control.cancellation_observed();
            let _ = tx
                .send(Reply::PackPreview {
                    op: call.op,
                    serial: call.serial,
                    observed_cancellation,
                    result,
                })
                .await;
        });
        if !was_busy {
            cx.emit(ContentUpdatesEvent::BusyChanged(true));
        }
        cx.notify();
    }

    fn start_pack_apply(&mut self, cx: &mut Context<Self>) {
        if self.blocked {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let was_busy = self.state.busy();
        let Some(call) = self.state.confirm_pack_apply() else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        let instance = call.target.instance.clone();
        let fingerprint = call.fingerprint.clone();
        let resolutions = call.resolutions.clone();
        let control = call.control.clone();
        self.runtime.spawn(async move {
            let run = control.clone();
            let check = control.clone();
            let result = guarded(async move {
                run.run(async move {
                    if aurora_launcher_lib::content_jobs::checkpoint().is_err() {
                        return Err(api::ApiError::new(
                            "update_cancelled",
                            "Update cancelled before it started.",
                        ));
                    }
                    let result =
                        api::apply_modpack_update(host, instance, fingerprint, resolutions).await;
                    match result {
                        Err(_) if check.cancellation_observed() => {
                            Err(api::ApiError::new("update_cancelled", "Update cancelled."))
                        }
                        other => other,
                    }
                })
                .await
            })
            .await;
            let observed_cancellation = control.cancellation_observed();
            let _ = tx
                .send(Reply::PackApply {
                    op: call.op,
                    serial: call.serial,
                    observed_cancellation,
                    result,
                })
                .await;
        });
        if !was_busy {
            cx.emit(ContentUpdatesEvent::BusyChanged(true));
        }
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.state.cancel_in_flight() {
            cx.notify();
        }
    }

    fn dismiss_review(&mut self, cx: &mut Context<Self>) {
        self.state.clear_review_display();
        cx.notify();
    }

    fn toggle_keep_local(&mut self, keep: bool, cx: &mut Context<Self>) {
        if self.state.pack_keep_local != keep {
            self.state.pack_keep_local = keep;
            cx.notify();
        }
    }
}

// -------------------------------------------------------------------- render

/// Multi-line text as one child per line (no rich formatting invented).
fn text_block(text: &str, color: Hsla, size: Pixels) -> impl IntoElement + use<> {
    let block = div().flex().flex_col().text_size(size).text_color(color);
    text.lines()
        .fold(block, |block, line| block.child(line.to_owned()))
}

/// Badge tone for one update entry's core status.
fn status_tone(entry: &api::UpdateEntry) -> ui::Tone {
    match entry.status.as_str() {
        "updateAvailable" if !entry.pinned => ui::Tone::Accent,
        "updateAvailable" | "pinnedUpdateAvailable" | "blocked" => ui::Tone::Warning,
        "upToDate" => ui::Tone::Success,
        "providerUnavailable" => ui::Tone::Danger,
        _ => ui::Tone::Neutral,
    }
}

/// A one-line notice inside the Updates card.
fn notice_line(icon: &'static str, color: Hsla, text: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .text_size(px(ui::TEXT_BODY))
        .text_color(color)
        .child(crate::symbols::icon(Some(icon), px(15.), color))
        .child(div().flex_1().min_w(px(0.)).child(text.into()))
}

impl Render for ContentUpdates {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx).clone();
        let t = &palette;
        let view = cx.entity();
        let busy = self.state.busy() || self.blocked;
        if self.host.is_none() {
            return ui::card(t)
                .gap(px(8.))
                .child(ui::section_header(t, "Updates", None))
                .child(ui::muted(
                    t,
                    "Update controls are unavailable in diagnostic mode. No provider requests are made from this view.",
                ));
        }

        let Some(target) = self.state.target.clone() else {
            let note = if self.state.instance_name.is_empty() {
                "Choose an instance to manage its updates."
            } else {
                "Updates are not supported for this content kind."
            };
            return ui::card(t)
                .gap(px(8.))
                .child(ui::section_header(t, "Updates", None))
                .child(ui::muted(t, note));
        };

        // ------------------------------------------------------- managed content
        let checking = matches!(
            self.state.in_flight.as_ref().map(|op| &op.kind),
            Some(OpKind::Check)
        );
        let noun = kind_noun(target.kind);
        let eligible_count = self.state.eligible_projects().len();
        let summary = if self.state.checked {
            summarize(&self.state.entries, self.state.dependency_managed)
        } else if checking {
            "Checking for updates…".to_owned()
        } else {
            format!("Check to see if any of this instance's {noun} have updates.")
        };
        let check = view.clone();
        let mut root = ui::card(t).gap(px(16.)).child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    ui::section_header(t, "Updates", Some(summary.into()))
                        .flex_1()
                        .min_w(px(240.)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            AuroraButton::new(
                                "cu-check",
                                if checking {
                                    "Checking…"
                                } else {
                                    "Check for updates"
                                },
                            )
                            .size(ButtonSize::Small)
                            .icon("update")
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                check.update(cx, |s, cx| s.start_check(cx));
                            }),
                        )
                        .when(eligible_count > 0, |row| {
                            let v = view.clone();
                            row.child(
                                AuroraButton::new(
                                    "cu-review-all",
                                    format!("Review all ({eligible_count})"),
                                )
                                .size(ButtonSize::Small)
                                .variant(ButtonVariant::Primary)
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    v.update(cx, |s, cx| s.start_review_updates(None, cx));
                                }),
                            )
                        }),
                ),
        );

        if self.state.checked && !self.state.entries.is_empty() {
            let mut list = ui::list_card(t).bg(palette.sunken);
            let last_index = self.state.entries.len() - 1;
            for (index, entry) in self.state.entries.iter().enumerate() {
                let project = entry.project_id.clone();
                let label = status_label(entry);
                let (head, tail) = match label.split_once(". ") {
                    Some((head, tail)) => (head.to_owned(), Some(tail.to_owned())),
                    None => (label.clone(), None),
                };
                let mut actions = div().flex().flex_shrink_0().items_center().gap(px(6.));
                if eligible(entry) {
                    let v = view.clone();
                    let target_project = project.clone();
                    actions = actions.child(
                        AuroraButton::new(
                            SharedString::from(format!("cu-review-{project}")),
                            "Review update",
                        )
                        .size(ButtonSize::Small)
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            let project = target_project.clone();
                            v.update(cx, |s, cx| {
                                s.start_review_updates(Some(vec![project.clone()]), cx)
                            });
                        }),
                    );
                }
                let pin_target = if entry.pinned {
                    Some(false)
                } else if eligible(entry) {
                    Some(true)
                } else {
                    None
                };
                if let Some(pinned) = pin_target {
                    let v = view.clone();
                    let label: &str = if pinned { "Pin" } else { "Unpin" };
                    actions = actions.child(
                        AuroraButton::new(SharedString::from(format!("cu-pin-{project}")), label)
                            .size(ButtonSize::Small)
                            .variant(ButtonVariant::Ghost)
                            .icon("push_pin")
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                let project = project.clone();
                                v.update(cx, |s, cx| s.start_pin(project.clone(), pinned, cx));
                            }),
                    );
                }
                list = list.child(
                    ui::row(t, index == last_index)
                        .child(
                            ui::title_block(
                                t,
                                entry.project_id.clone(),
                                Some(entry.current_version.clone().into()),
                            )
                            .flex_1(),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_end()
                                .gap(px(2.))
                                .child(ui::badge(t, head, status_tone(entry)))
                                .when_some(tail, |d, tail| {
                                    d.child(ui::caption(t, tail).max_w(px(260.)).text_ellipsis())
                                }),
                        )
                        .child(actions),
                );
            }
            root = root.child(list);
        }

        // ------------------------------------------------------- managed modpack
        if let Some(pack_name) = self.state.pack_name.clone() {
            let (status, available) = match self.state.pack_status.clone() {
                Some((status, available)) => (status, available),
                None => (
                    "Check to see if a new modpack version is available.".to_owned(),
                    false,
                ),
            };
            let has_status = self.state.pack_status.is_some();
            let check = view.clone();
            let mut actions = div().flex().flex_shrink_0().items_center().gap(px(6.));
            if has_status && available {
                let v = view.clone();
                actions = actions.child(
                    AuroraButton::new("cu-pack-review", "Review modpack update")
                        .size(ButtonSize::Small)
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            v.update(cx, |s, cx| s.start_pack_review(cx));
                        }),
                );
            }
            actions = actions.child(
                AuroraButton::new("cu-pack-check", "Check modpack")
                    .size(ButtonSize::Small)
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        check.update(cx, |s, cx| s.start_pack_check(cx));
                    }),
            );
            root = root.child(
                ui::list_card(t).bg(palette.sunken).child(
                    ui::row(t, true)
                        .child(ui::icon_tile(t, Some("inventory_2"), 36., false))
                        .child(
                            ui::title_block(
                                t,
                                format!("Modpack · {pack_name}"),
                                Some(status.into()),
                            )
                            .flex_1(),
                        )
                        .child(actions),
                ),
            );
        }

        // -------------------------------------------------------------- progress
        if let Some(op) = &self.state.in_flight {
            let cancellable = op.control.is_some() && !op.cancelling;
            let v = view.clone();
            root = root.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(10.))
                    .rounded(palette.radius)
                    .bg(palette.control)
                    .border_1()
                    .border_color(palette.border_subtle)
                    .child(crate::symbols::icon(Some("sync"), px(15.), palette.ring))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(ui::TEXT_BODY))
                                    .text_color(palette.foreground)
                                    .child(op_label(op)),
                            )
                            .child(ui::caption(t, "One update at a time.")),
                    )
                    .when(cancellable, |row| {
                        row.child(
                            AuroraButton::new("cu-cancel", "Cancel")
                                .size(ButtonSize::Small)
                                .variant(ButtonVariant::Danger)
                                .on_click(move |_, _, cx| {
                                    v.update(cx, |s, cx| s.cancel(cx));
                                }),
                        )
                    }),
            );
        }
        if !self.state.message.is_empty() {
            root = root.child(notice_line(
                "info",
                palette.muted_foreground,
                self.state.message.clone(),
            ));
        }
        if let Some(error) = &self.state.error {
            root = root.child(notice_line("error", palette.danger, error.clone()));
        }

        // --------------------------------------------------------- review dialog
        let panel = self.review_panel(cx, &palette, busy);
        let veto = view.clone();
        let changed = view.clone();
        // The dialog sits beside the card, not inside it, so it adds no card gap.
        div().flex().flex_col().child(root).child(
            Dialog::new(cx)
                .open(self.state.review_open)
                .accessibility_label("Review updates")
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

impl ContentUpdates {
    /// The explicit review dialog. The base `Dialog` owns the focus trap and
    /// Escape/Enter bindings; Enter never confirms (buttons own their own
    /// keyboard activation) and Escape is vetoed while work is running. The
    /// bounded scroll surface keeps large deltas and conflict lists from
    /// hiding the confirmation controls.
    fn review_panel(
        &mut self,
        cx: &mut Context<Self>,
        palette: &theme::AuroraTheme,
        busy: bool,
    ) -> impl IntoElement + use<> {
        let t = palette;
        let view = cx.entity();
        let operation_busy = self.state.busy();
        let heading = |text: String| {
            div()
                .text_size(px(ui::TEXT_SECTION))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(palette.foreground)
                .child(text)
        };
        let well = |text: &str| {
            div()
                .p(px(12.))
                .rounded(palette.radius)
                .bg(palette.sunken)
                .border_1()
                .border_color(palette.border_subtle)
                .child(text_block(text, palette.foreground, px(12.)))
        };
        let mut panel = div()
            .id("cu-review")
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
        let actions = || {
            div()
                .flex()
                .flex_wrap()
                .justify_end()
                .gap(px(8.))
                .pt(px(6.))
        };

        if let Some(review) = self.state.review.clone() {
            let noun = kind_noun(review.target.kind);
            let count = review.roots.len();
            let v = view.clone();
            let close = view.clone();
            panel = panel
                .child(heading(format!("Review updates ({count} {noun})")))
                .child(well(&review.text))
                .child(ui::caption(
                    t,
                    "Applying re-checks this exact review in the launcher core before any file changes.",
                ))
                .child(
                    actions()
                        .child(
                            AuroraButton::new("cu-dismiss", "Close review")
                                .disabled(operation_busy)
                                .on_click(move |_, _, cx| {
                                    close.update(cx, |s, cx| s.dismiss_review(cx));
                                }),
                        )
                        .child(
                            AuroraButton::new("cu-apply", format!("Update {count} {noun}"))
                                .variant(ButtonVariant::Primary)
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    v.update(cx, |s, cx| s.start_apply(cx));
                                }),
                        ),
                );
        } else if let Some(review) = self.state.pack_review.clone() {
            let conflicts = review.conflicts.len();
            let keep = self.state.pack_keep_local;
            let v = view.clone();
            let close = view.clone();
            panel = panel
                .child(heading("Review modpack update".to_owned()))
                .child(well(&review.text));
            if conflicts > 0 {
                let toggle = view.clone();
                panel = panel.child(
                    ui::setting_row(
                        t,
                        "Keep my changed files",
                        Some(
                            "Required: your copies are kept and the pack's new copies are not written over them."
                                .into(),
                        ),
                        true,
                    )
                    .child(
                        AuroraToggle::new("cu-keep-local")
                            .label("Keep my changed files")
                            .checked(keep)
                            .disabled(busy)
                            .on_change(move |next, _, _, cx| {
                                toggle.update(cx, |s, cx| s.toggle_keep_local(next, cx));
                            }),
                    ),
                );
            }
            panel = panel.child(
                actions()
                    .child(
                        AuroraButton::new("cu-pack-dismiss", "Close review")
                            .disabled(operation_busy)
                            .on_click(move |_, _, cx| {
                                close.update(cx, |s, cx| s.dismiss_review(cx));
                            }),
                    )
                    .child(
                        AuroraButton::new("cu-pack-apply", "Update modpack")
                            .variant(ButtonVariant::Primary)
                            .disabled(busy || (conflicts > 0 && !keep))
                            .on_click(move |_, _, cx| {
                                v.update(cx, |s, cx| s.start_pack_apply(cx));
                            }),
                    ),
            );
        } else if let Some(op) = self.state.in_flight.as_ref() {
            panel = panel
                .child(heading("Updating…".to_owned()))
                .child(ui::muted(t, op_label(op)));
            if op.control.is_some() && !op.cancelling {
                let v = view.clone();
                panel = panel.child(
                    actions().child(
                        AuroraButton::new("cu-cancel-apply", "Cancel update")
                            .variant(ButtonVariant::Danger)
                            .on_click(move |_, _, cx| {
                                v.update(cx, |s, cx| s.cancel(cx));
                            }),
                    ),
                );
            }
        } else {
            panel = panel.child(heading("Review updates".to_owned()));
        }
        panel
    }
}

// --------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(
        project: &str,
        status: &str,
        pinned: bool,
        candidate: Option<&str>,
    ) -> api::UpdateEntry {
        api::UpdateEntry {
            content_type: "mod".into(),
            project_id: project.into(),
            current_version: "v1".into(),
            status: status.into(),
            block: None,
            candidate: candidate.map(|version| api::UpdateCandidate {
                id: format!("{project}-{version}"),
                version_number: version.into(),
                version_type: "release".into(),
            }),
            pinned,
            detail: None,
        }
    }

    fn blocked_entry(project: &str, block: &str) -> api::UpdateEntry {
        api::UpdateEntry {
            block: Some(block.into()),
            ..entry(project, "blocked", false, Some("2.0"))
        }
    }

    fn target(revision: u64) -> Target {
        Target {
            instance: "i1".into(),
            kind: "mod",
            revision,
        }
    }

    fn pack_target(revision: u64) -> Target {
        Target {
            instance: "pack1".into(),
            kind: "mod",
            revision,
        }
    }

    fn report(entries: Vec<api::UpdateEntry>, dependency_managed: usize) -> api::UpdatesReport {
        api::UpdatesReport {
            entries,
            dependency_managed,
        }
    }

    fn retargeted(target: Option<Target>, pack: Option<&str>) -> UpdatesState {
        let mut state = UpdatesState::default();
        state.retarget(target, "Instance".into(), pack.map(str::to_owned));
        state
    }

    fn checked_state() -> UpdatesState {
        let mut state = retargeted(Some(target(1)), None);
        state.entries = vec![
            entry("a", "updateAvailable", false, Some("2.0")),
            entry("b", "pinnedUpdateAvailable", true, Some("3.0")),
            entry("c", "upToDate", false, None),
        ];
        state.dependency_managed = 1;
        state.checked = true;
        state
    }

    fn bulk_preview() -> Value {
        json!({
            "targets": [{"current": {"fileName": "a.jar", "displayVersion": "1.0"},
                         "candidate": {"versionNumber": "1.1"}}],
            "delta": {"newRequirements": [], "removedRequirements": []},
            "warnings": [], "previewFingerprint": "fp1"
        })
    }

    fn pack_plan() -> Value {
        json!({
            "name": "Pack", "currentPackVersion": "1", "candidatePackVersion": "2", "gameTransition": true,
            "minecraftCurrent": "1.21.1", "minecraftCandidate": "1.21.4", "loaderCurrent": "0.16.0", "loaderCandidate": "0.16.9",
            "counts": {"added": 1, "updated": 2, "removed": 0, "preserved": 5, "conflicts": 1},
            "rows": [{"path": "config/a.json", "action": "conflict"}, {"path": "mods/b.jar", "action": "replace"}],
            "newOptionalUnselected": ["mods/opt.jar"], "fingerprint": "pack-fp", "changelog": "Fixes things."
        })
    }

    // ---------------------------------------------------------- eligibility

    #[test]
    fn only_unpinned_compatible_candidates_are_updatable() {
        assert!(eligible(&entry("a", "updateAvailable", false, Some("2.0"))));
        assert!(!eligible(&entry("a", "updateAvailable", true, Some("2.0"))));
        assert!(!eligible(&entry(
            "a",
            "pinnedUpdateAvailable",
            true,
            Some("2.0")
        )));
        assert!(!eligible(&entry("a", "upToDate", false, None)));
        assert!(!eligible(&entry("a", "blocked", false, Some("2.0"))));
        assert!(!eligible(&entry("a", "noNewerUnderPolicy", false, None)));
        assert!(!eligible(&entry("a", "providerUnavailable", false, None)));
        assert!(!eligible(&entry("a", "updateAvailable", false, None)));
        for block in [
            "packOwned",
            "disabled",
            "locallyModified",
            "missing",
            "conflict",
        ] {
            assert!(!eligible(&blocked_entry("a", block)), "{block}");
        }
    }

    #[test]
    fn status_labels_and_block_reasons_stay_honest() {
        assert_eq!(
            status_label(&entry("a", "updateAvailable", false, Some("2.0"))),
            "Update: 2.0"
        );
        assert_eq!(
            status_label(&entry("a", "pinnedUpdateAvailable", true, Some("3.0"))),
            "Pinned · 3.0 available"
        );
        assert_eq!(
            status_label(&blocked_entry("a", "packOwned")),
            "Managed by the modpack"
        );
        assert_eq!(
            status_label(&blocked_entry("a", "locallyModified")),
            "Changed locally — not updated"
        );
        assert_eq!(
            status_label(&entry("a", "weirdState", false, None)),
            "Update status unknown"
        );
        let mut detailed = entry("a", "providerUnavailable", false, None);
        detailed.detail = Some("The provider is rate limiting requests.".into());
        assert_eq!(
            status_label(&detailed),
            "Check failed. The provider is rate limiting requests."
        );
    }

    #[test]
    fn summary_counts_each_state_honestly() {
        let entries = vec![
            entry("a", "updateAvailable", false, Some("2")),
            entry("b", "updateAvailable", false, Some("3")),
            entry("c", "pinnedUpdateAvailable", true, Some("4")),
            entry("d", "upToDate", false, None),
            entry("e", "blocked", false, None),
            entry("f", "providerUnavailable", false, None),
        ];
        assert_eq!(
            summarize(&entries, 2),
            "2 updates available · 1 pinned · 1 up to date · 1 need attention · 1 could not be checked · 2 dependencies follow their parents"
        );
        assert_eq!(
            summarize(&[], 0),
            "Nothing here is managed from Modrinth, so there is nothing to update."
        );
    }

    // ---------------------------------------------------- target identity

    #[test]
    fn targets_reject_changed_instance_kind_and_revision() {
        let t = target(1);
        assert!(retargeted(Some(t.clone()), None).current(&t));
        let mut n = t.clone();
        n.revision += 1;
        assert!(!retargeted(Some(t.clone()), None).current(&n));
        let mut n = t.clone();
        n.instance = "i2".into();
        assert!(!retargeted(Some(t.clone()), None).current(&n));
        let mut n = t.clone();
        n.kind = "shaderPack";
        assert!(!retargeted(Some(t), None).current(&n));
    }

    #[test]
    fn kind_vocabulary_is_closed() {
        for legal in ["mod", "resourcePack", "shaderPack"] {
            assert!(legal_kind(legal));
        }
        for illegal in ["modpack", "", "Mod", "resourcepack"] {
            assert!(!legal_kind(illegal));
        }
        assert_eq!(kind_noun("mod"), "mods");
        assert_eq!(kind_noun("resourcePack"), "resource packs");
        assert_eq!(kind_noun("shaderPack"), "shader packs");
    }

    #[test]
    fn retargeting_and_refresh_clear_stale_reads_but_keep_active_operations() {
        let mut state = checked_state();
        let call = state.begin_review_updates(None).unwrap();
        assert!(state.busy());
        state.retarget(
            Some(Target {
                revision: 2,
                ..target(1)
            }),
            "Instance".into(),
            None,
        );
        assert!(state.busy(), "a running future is never abandoned");
        assert!(!state.checked);
        assert!(state.entries.is_empty());
        let fx = state.receive(Reply::Preview {
            op: call.op,
            serial: call.serial,
            roots: call.roots,
            observed_cancellation: false,
            result: Ok(bulk_preview()),
        });
        assert_eq!(
            fx.busy,
            Some(false),
            "the terminal settle still clears busy"
        );
        assert!(state.review.is_none(), "the stale review is discarded");

        let mut state = checked_state();
        assert!(state.invalidate());
        assert!(!state.checked && state.entries.is_empty());
        assert!(!state.invalidate(), "a second clear drops nothing");
    }

    #[test]
    fn same_target_refresh_rejects_in_flight_checks_and_reviews() {
        let mut state = retargeted(Some(target(1)), None);
        let check = state.begin_check().unwrap();
        state.invalidate();
        let fx = state.receive(Reply::Check {
            op: check.op,
            result: Ok(report(
                vec![entry("a", "updateAvailable", false, Some("v2"))],
                0,
            )),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(!state.checked && state.entries.is_empty());

        let mut state = checked_state();
        let review = state.begin_review_updates(None).unwrap();
        state.invalidate();
        let fx = state.receive(Reply::Preview {
            op: review.op,
            serial: review.serial,
            roots: review.roots,
            observed_cancellation: false,
            result: Ok(bulk_preview()),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(state.review.is_none() && !state.review_open);
    }

    // ---------------------------------------------------------- check flow

    #[test]
    fn a_fresh_view_checks_nothing_until_explicitly_asked() {
        let mut state = retargeted(Some(target(1)), None);
        assert!(!state.checked);
        assert!(!state.busy());
        assert!(state.begin_review_updates(None).is_none());
        assert!(state.begin_pin("a", true).is_none());
        let call = state.begin_check().unwrap();
        assert!(state.busy());
        let fx = state.receive(Reply::Check {
            op: call.op,
            result: Ok(report(vec![entry("a", "upToDate", false, None)], 0)),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(!fx.changed, "a read never emits Changed");
        assert!(state.checked);
        assert_eq!(state.entries.len(), 1);
    }

    #[test]
    fn check_results_apply_only_to_the_current_target() {
        let mut state = retargeted(Some(target(1)), None);
        let call = state.begin_check().unwrap();
        state.retarget(Some(target(2)), "Instance".into(), None);
        let fx = state.receive(Reply::Check {
            op: call.op,
            result: Ok(report(vec![entry("a", "upToDate", false, None)], 0)),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(!state.checked, "a stale read applies nothing");
        assert!(state.entries.is_empty());
    }

    #[test]
    fn check_failures_are_honest_not_silent() {
        let mut state = retargeted(Some(target(1)), None);
        let call = state.begin_check().unwrap();
        state.receive(Reply::Check {
            op: call.op,
            result: Err(api::ApiError::new("provider_unavailable", "offline")),
        });
        assert!(!state.checked);
        assert_eq!(
            state.error.as_deref(),
            Some("Update check failed: offline (provider_unavailable)")
        );
    }

    // --------------------------------------------------------- stale guards

    #[test]
    fn duplicate_and_late_settles_cannot_clear_newer_busy() {
        let mut state = retargeted(Some(target(1)), None);
        let first = state.begin_check().unwrap();
        state.receive(Reply::Check {
            op: first.op,
            result: Ok(report(vec![], 0)),
        });
        let second = state.begin_check().unwrap();
        assert!(state.busy());
        let fx = state.receive(Reply::Check {
            op: first.op,
            result: Ok(report(vec![entry("stale", "upToDate", false, None)], 0)),
        });
        assert_eq!(fx, Effects::default(), "a duplicate settle changes nothing");
        assert!(state.busy(), "the newer operation keeps its busy state");
        assert!(state.entries.is_empty());
        let fx = state.receive(Reply::Check {
            op: second.op,
            result: Ok(report(vec![], 0)),
        });
        assert_eq!(fx.busy, Some(false));
    }

    #[test]
    fn a_reply_for_an_unknown_operation_is_ignored() {
        let mut state = retargeted(Some(target(1)), None);
        let fx = state.receive(Reply::Check {
            op: 99,
            result: Ok(report(vec![], 0)),
        });
        assert_eq!(fx, Effects::default());
    }

    #[test]
    fn a_reply_of_the_wrong_shape_never_settles_its_operation() {
        let mut state = checked_state();
        let call = state.begin_review_updates(Some(vec!["a".into()])).unwrap();
        let fx = state.receive(Reply::Check {
            op: call.op,
            result: Ok(report(vec![], 0)),
        });
        assert_eq!(fx, Effects::default());
        assert!(state.busy(), "the preview keeps running");
    }

    // ------------------------------------------------------- review gating

    #[test]
    fn previews_use_exact_eligible_project_ids_only() {
        let mut state = checked_state();
        assert!(
            state.begin_review_updates(Some(vec!["c".into()])).is_none(),
            "up to date"
        );
        assert!(
            state.begin_review_updates(Some(vec!["b".into()])).is_none(),
            "pinned"
        );
        assert!(
            state
                .begin_review_updates(Some(vec!["missing".into()]))
                .is_none()
        );
        let call = state.begin_review_updates(Some(vec!["a".into()])).unwrap();
        assert_eq!(call.roots, vec!["a".to_owned()]);
        state.receive(Reply::Preview {
            op: call.op,
            serial: call.serial,
            roots: call.roots.clone(),
            observed_cancellation: false,
            result: Ok(bulk_preview()),
        });
        assert!(state.review.is_some());
        state.clear_review_display();
        let call = state.begin_review_updates(None).unwrap();
        assert_eq!(call.roots, vec!["a".to_owned()], "only the eligible root");
    }

    #[test]
    fn malformed_previews_never_become_reviews() {
        let mut state = checked_state();
        for malformed in [
            json!({"targets": [], "previewFingerprint": ""}),
            json!({"targets": [], "previewFingerprint": Value::Null}),
            json!({"previewFingerprint": "fp"}),
            json!({"targets": {}, "previewFingerprint": "fp"}),
        ] {
            let call = state.begin_review_updates(Some(vec!["a".into()])).unwrap();
            let fx = state.receive(Reply::Preview {
                op: call.op,
                serial: call.serial,
                roots: vec!["a".into()],
                observed_cancellation: false,
                result: Ok(malformed.clone()),
            });
            assert_eq!(fx.busy, Some(false));
            assert!(state.review.is_none(), "{malformed} must not enable apply");
            assert!(state.error.is_some());
        }
        assert!(
            describe_update_preview(&json!({"targets": [], "previewFingerprint": "f"})).is_some()
        );
    }

    #[test]
    fn preview_text_lists_updates_dependencies_and_warnings() {
        let (_, text) = describe_update_preview(&bulk_preview()).unwrap();
        assert!(text.contains("a.jar (1.0) → 1.1"));
        let mut added = bulk_preview();
        added["delta"]["newRequirements"] = json!([{"fileName": "lib.jar"}]);
        let (_, text) = describe_update_preview(&added).unwrap();
        assert!(text.contains("New dependencies installed\nlib.jar"));
        let mut warned = bulk_preview();
        warned["warnings"] = json!(["Changes the loader requirement"]);
        warned["delta"]["removedRequirements"] = json!([{"fileName": "old.jar"}]);
        let (_, text) = describe_update_preview(&warned).unwrap();
        assert!(text.contains("Dependencies no longer required (removed)\nold.jar"));
        assert!(text.contains("Warnings\nChanges the loader requirement"));
    }

    #[test]
    fn observed_cancellation_never_becomes_a_review() {
        let mut state = checked_state();
        let call = state.begin_review_updates(Some(vec!["a".into()])).unwrap();
        let fx = state.receive(Reply::Preview {
            op: call.op,
            serial: call.serial,
            roots: call.roots,
            observed_cancellation: true,
            result: Ok(bulk_preview()),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(state.review.is_none());
        assert_eq!(state.message, "Review cancelled. Nothing was updated.");
    }

    #[test]
    fn cancelling_a_preview_discards_its_late_result() {
        let mut state = checked_state();
        let call = state.begin_review_updates(Some(vec!["a".into()])).unwrap();
        assert!(call.control.phase() != JobPhase::Cancelling);
        assert!(state.cancel_in_flight());
        assert_eq!(call.control.phase(), JobPhase::Cancelling);
        assert!(state.busy(), "the operation runs to its terminal settle");
        assert!(!state.cancel_in_flight(), "a second cancel changes nothing");
        let fx = state.receive(Reply::Preview {
            op: call.op,
            serial: call.serial, // stale: the cancel bumped the epoch
            roots: call.roots,
            observed_cancellation: false,
            result: Ok(bulk_preview()),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(state.review.is_none());
    }

    #[test]
    fn uncancellable_operations_ignore_cancel_requests() {
        let mut state = retargeted(Some(target(1)), None);
        let _call = state.begin_check().unwrap();
        assert!(!state.cancel_in_flight(), "a check has no cancel path");
        assert!(state.busy());
    }

    // --------------------------------------------------------- pin policy

    #[test]
    fn pin_policy_matches_the_presented_state() {
        // Each successful begin marks the single operation busy, so every
        // assertion starts from a fresh state.
        assert!(
            checked_state().begin_pin("b", false).is_some(),
            "unpin a pinned entry"
        );
        assert!(
            checked_state().begin_pin("a", true).is_some(),
            "pin an offered update"
        );
        let mut state = checked_state();
        assert!(state.begin_pin("a", false).is_none(), "already unpinned");
        assert!(
            state.begin_pin("c", true).is_none(),
            "nothing to pin without an offer"
        );
        assert!(state.begin_pin("missing", true).is_none());
        let mut unchecked = retargeted(Some(target(1)), None);
        assert!(
            unchecked.begin_pin("a", true).is_none(),
            "no check, no policy buttons"
        );
    }

    #[test]
    fn pin_settles_notify_the_parent_even_on_failure() {
        let mut state = checked_state();
        let call = state.begin_pin("a", true).unwrap();
        let fx = state.receive(Reply::Pin {
            op: call.op,
            result: Err(api::ApiError::new("provider_invalid_request", "no")),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(fx.changed, "a failed mutation may have retained state");
        assert_eq!(fx.recheck, None, "the failed write changed no policy");
        assert!(
            state.checked,
            "the previous check still describes the instance"
        );
        assert_eq!(
            state.error.as_deref(),
            Some("no (provider_invalid_request)")
        );

        let mut state = checked_state();
        let call = state.begin_pin("a", true).unwrap();
        let fx = state.receive(Reply::Pin {
            op: call.op,
            result: Ok(Value::Null),
        });
        assert!(fx.changed);
        assert_eq!(fx.recheck, Some(Recheck::Content));
        assert!(!state.checked, "badges never describe superseded policy");
        assert_eq!(
            state.message,
            "Pinned a. It stays on its current version until unpinned."
        );
    }

    #[test]
    fn pinned_entries_can_rejoin_future_updates_without_a_current_offer() {
        let mut state = checked_state();
        state.entries = vec![entry("held", "upToDate", true, None)];
        assert!(state.begin_pin("held", false).is_some());
    }

    #[test]
    fn superseded_mutations_settle_without_describing_the_new_target() {
        for kind in 0..3 {
            for fails in [false, true] {
                let mut state = match kind {
                    0 => checked_state(),
                    1 => reviewed_state(),
                    _ => pack_reviewed_state(),
                };
                let result = if fails {
                    Err(api::ApiError::new(
                        "old_target_failed",
                        "Old target failed.",
                    ))
                } else {
                    Ok(Value::Null)
                };
                let reply = match kind {
                    0 => Reply::Pin {
                        op: state.begin_pin("a", true).unwrap().op,
                        result,
                    },
                    1 => {
                        let call = state.confirm_apply().unwrap();
                        Reply::Apply {
                            op: call.op,
                            serial: call.serial,
                            observed_cancellation: false,
                            result,
                        }
                    }
                    _ => {
                        state.pack_keep_local = true;
                        let call = state.confirm_pack_apply().unwrap();
                        Reply::PackApply {
                            op: call.op,
                            serial: call.serial,
                            observed_cancellation: false,
                            result,
                        }
                    }
                };
                state.retarget(Some(target(99)), "New instance".into(), None);
                state.message = "New target message".into();
                state.error = Some("New target error".into());
                let fx = state.receive(reply);
                assert_eq!(fx.busy, Some(false));
                assert!(fx.changed);
                assert_eq!(fx.recheck, None);
                assert_eq!(state.message, "New target message");
                assert_eq!(state.error.as_deref(), Some("New target error"));
                assert!(!state.busy());
            }
        }
    }

    // -------------------------------------------------------- apply settles

    fn reviewed_state() -> UpdatesState {
        let mut state = checked_state();
        let call = state.begin_review_updates(Some(vec!["a".into()])).unwrap();
        state.receive(Reply::Preview {
            op: call.op,
            serial: call.serial,
            roots: call.roots,
            observed_cancellation: false,
            result: Ok(bulk_preview()),
        });
        state
    }

    #[test]
    fn confirm_consuming_the_review_freezes_exact_roots_and_fingerprint() {
        let mut state = reviewed_state();
        assert!(state.review_open);
        let call = state.confirm_apply().unwrap();
        assert_eq!(call.roots, vec!["a".to_owned()]);
        assert_eq!(call.fingerprint, "fp1");
        assert!(state.review.is_none(), "one review applies once");
        assert!(state.review_open, "the dialog stays open while applying");
        assert!(state.confirm_apply().is_none());
    }

    #[test]
    fn apply_settles_recheck_and_invalidate_even_after_errors() {
        let mut state = reviewed_state();
        let call = state.confirm_apply().unwrap();
        let fx = state.receive(Reply::Apply {
            op: call.op,
            serial: call.serial,
            observed_cancellation: false,
            result: Err(api::ApiError::new("provider_preview_changed", "changed")),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(fx.changed);
        assert_eq!(fx.recheck, Some(Recheck::Content));
        assert!(!state.checked);
        assert!(!state.review_open);
        assert_eq!(
            state.error.as_deref(),
            Some("changed (provider_preview_changed)")
        );
    }

    #[test]
    fn a_target_change_during_an_apply_still_reports_its_terminal_settle() {
        let mut state = reviewed_state();
        let call = state.confirm_apply().unwrap();
        state.retarget(Some(target(9)), "Instance".into(), None);
        assert!(state.busy(), "the mutation future is never abandoned");
        let fx = state.receive(Reply::Apply {
            op: call.op,
            serial: call.serial,
            observed_cancellation: false,
            result: Ok(Value::Null),
        });
        assert_eq!(fx.busy, Some(false), "actual busy clears");
        assert!(fx.changed, "the parent still learns the mutation settled");
        assert_eq!(fx.recheck, None, "no recheck for a superseded target");
    }

    #[test]
    fn cancelled_applies_report_cancellation_wording() {
        let mut state = reviewed_state();
        let call = state.confirm_apply().unwrap();
        state.cancel_in_flight();
        let fx = state.receive(Reply::Apply {
            op: call.op,
            serial: call.serial,
            observed_cancellation: true,
            result: Err(api::ApiError::new("update_cancelled", "Update cancelled.")),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(fx.changed);
        assert_eq!(
            state.error.as_deref(),
            Some("Cancelled. Active writes finished safely; nothing uncommitted was activated.")
        );
    }

    #[test]
    fn a_stale_fingerprint_never_reaches_apply() {
        let mut state = checked_state();
        state.review = Some(UpdateReview {
            target: target(1),
            roots: vec!["a".into()],
            fingerprint: String::new(),
            text: String::new(),
        });
        assert!(state.confirm_apply().is_none());
        state.review = None;
        state.review = Some(UpdateReview {
            target: Target {
                revision: 7,
                ..target(1)
            },
            roots: vec!["a".into()],
            fingerprint: "fp".into(),
            text: String::new(),
        });
        assert!(
            state.confirm_apply().is_none(),
            "a review of a superseded target never applies"
        );
    }

    // --------------------------------------------------------- modpack flow

    #[test]
    fn plain_instances_have_no_modpack_controls() {
        let mut state = checked_state();
        assert!(state.begin_pack_check().is_none());
        assert!(state.begin_pack_review().is_none());
    }

    #[test]
    fn pack_check_states_are_distinct_and_honest() {
        let available = json!({"status": "updateAvailable", "currentPackVersion": "1",
            "candidate": {"versionNumber": "2"}, "divergences": 2});
        assert_eq!(
            pack_check_summary(&available),
            (
                "2 available (installed 1) (2 local change(s) recorded)".to_owned(),
                true
            )
        );
        let up_to_date = json!({"status": "upToDate", "currentPackVersion": "1", "divergences": 0});
        assert_eq!(
            pack_check_summary(&up_to_date),
            ("1 is the newest version".to_owned(), false)
        );
        assert_eq!(
            pack_check_summary(&json!({"status": "blocked", "blockedReason": "Not ready."})),
            ("Not ready.".to_owned(), false)
        );
        assert_eq!(
            pack_check_summary(&json!({"status": "currentVersionUnknown"})).0,
            "Modrinth no longer recognizes the installed pack version."
        );
        assert_eq!(
            pack_check_summary(&json!({"status": "providerUnavailable"})).0,
            "The modpack update check could not reach Modrinth."
        );
    }

    #[test]
    fn a_pack_check_must_precede_a_pack_review() {
        let mut state = retargeted(Some(pack_target(1)), Some("Pack"));
        assert!(state.begin_pack_review().is_none());
        let call = state.begin_pack_check().unwrap();
        state.receive(Reply::PackCheck {
            op: call.op,
            result: Ok(json!({"status": "upToDate", "currentPackVersion": "1"})),
        });
        assert!(
            state.begin_pack_review().is_none(),
            "no candidate, no review"
        );
        let call = state.begin_pack_check().unwrap();
        state.receive(Reply::PackCheck {
            op: call.op,
            result: Ok(
                json!({"status": "updateAvailable", "currentPackVersion": "1",
                "candidate": {"versionNumber": "2"}}),
            ),
        });
        assert!(state.begin_pack_review().is_some());
    }

    fn pack_reviewed_state() -> UpdatesState {
        let mut state = retargeted(Some(pack_target(1)), Some("Pack"));
        state.pack_status = Some(("2 available (installed 1)".into(), true));
        let call = state.begin_pack_review().unwrap();
        state.receive(Reply::PackPreview {
            op: call.op,
            serial: call.serial,
            observed_cancellation: false,
            result: Ok(pack_plan()),
        });
        state
    }

    #[test]
    fn pack_plans_list_the_three_way_plan_and_all_conflict_paths() {
        let state = pack_reviewed_state();
        let review = state.pack_review.clone().unwrap();
        assert_eq!(review.fingerprint, "pack-fp");
        assert_eq!(review.conflicts, vec!["config/a.json".to_owned()]);
        assert!(review.text.contains("Pack 1 → 2"));
        assert!(review.text.contains("Minecraft 1.21.1 → 1.21.4"));
        assert!(
            review
                .text
                .contains("1 added · 2 updated · 0 removed · 5 kept as is")
        );
        assert!(review.text.contains("config/a.json"));
        assert!(
            review
                .text
                .contains("New optional files (not installed)\nmods/opt.jar")
        );
        assert!(review.text.contains("Changelog\nFixes things."));
    }

    #[test]
    fn malformed_pack_plans_never_enable_apply() {
        for malformed in [
            json!({"name": "P", "currentPackVersion": "1", "candidatePackVersion": "2",
                   "rows": [], "fingerprint": ""}),
            json!({"name": "P", "currentPackVersion": "1", "candidatePackVersion": "2",
                   "rows": [], "fingerprint": Value::Null}),
            json!({"name": "", "currentPackVersion": "1", "candidatePackVersion": "2",
                   "rows": [], "fingerprint": "f"}),
            json!({"name": "P", "currentPackVersion": "", "candidatePackVersion": "2",
                   "rows": [], "fingerprint": "f"}),
            json!({"name": "P", "currentPackVersion": "1", "candidatePackVersion": "2",
                   "fingerprint": "f"}),
        ] {
            assert!(pack_plan_review(&malformed).is_none(), "{malformed}");
        }
        let mut state = retargeted(Some(pack_target(1)), Some("Pack"));
        state.pack_status = Some(("x".into(), true));
        let call = state.begin_pack_review().unwrap();
        state.receive(Reply::PackPreview {
            op: call.op,
            serial: call.serial,
            observed_cancellation: false,
            result: Ok(json!({"name": "P", "rows": [], "fingerprint": ""})),
        });
        assert!(state.pack_review.is_none());
        assert!(state.error.is_some());
    }

    #[test]
    fn keep_local_is_the_only_resolution_and_requires_acknowledgment() {
        let mut state = pack_reviewed_state();
        assert!(
            state.confirm_pack_apply().is_none(),
            "conflicts need acknowledgment"
        );
        state.pack_keep_local = true;
        let call = state.confirm_pack_apply().unwrap();
        assert_eq!(call.fingerprint, "pack-fp");
        assert_eq!(
            call.resolutions,
            vec![("config/a.json".to_owned(), "keepLocal".to_owned())]
        );
        assert!(state.pack_review.is_none(), "one plan applies once");

        // A conflict-free plan needs no acknowledgment.
        let mut state = retargeted(Some(pack_target(1)), Some("Pack"));
        state.pack_status = Some(("x".into(), true));
        let call = state.begin_pack_review().unwrap();
        let mut plan = pack_plan();
        plan["rows"] = json!([{"path": "mods/b.jar", "action": "replace"}]);
        state.receive(Reply::PackPreview {
            op: call.op,
            serial: call.serial,
            observed_cancellation: false,
            result: Ok(plan),
        });
        let call = state.confirm_pack_apply().unwrap();
        assert!(call.resolutions.is_empty());
    }

    #[test]
    fn pack_apply_settles_notify_and_recheck_the_pack() {
        let mut state = pack_reviewed_state();
        state.pack_keep_local = true;
        let call = state.confirm_pack_apply().unwrap();
        let fx = state.receive(Reply::PackApply {
            op: call.op,
            serial: call.serial,
            observed_cancellation: false,
            result: Ok(Value::Null),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(fx.changed);
        assert_eq!(fx.recheck, Some(Recheck::Pack));
        assert!(!state.review_open);
        assert!(
            state.pack_status.is_none(),
            "the installed version moved on"
        );

        // A superseded target still reports its settle without a recheck.
        let mut state = pack_reviewed_state();
        state.pack_keep_local = true;
        let call = state.confirm_pack_apply().unwrap();
        state.retarget(Some(pack_target(3)), "Instance".into(), Some("Pack".into()));
        let fx = state.receive(Reply::PackApply {
            op: call.op,
            serial: call.serial,
            observed_cancellation: false,
            result: Err(api::ApiError::new("pack_download_failed", "boom")),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(fx.changed);
        assert_eq!(fx.recheck, None);
    }

    #[test]
    fn stale_pack_previews_are_discarded() {
        let mut state = retargeted(Some(pack_target(1)), Some("Pack"));
        state.pack_status = Some(("x".into(), true));
        let call = state.begin_pack_review().unwrap();
        state.retarget(Some(pack_target(2)), "Instance".into(), Some("Pack".into()));
        let fx = state.receive(Reply::PackPreview {
            op: call.op,
            serial: call.serial,
            observed_cancellation: false,
            result: Ok(pack_plan()),
        });
        assert_eq!(fx.busy, Some(false));
        assert!(state.pack_review.is_none());
    }

    // -------------------------------------------------------------- progress

    #[test]
    fn pack_update_events_ignore_install_events_and_malformed_payloads() {
        let mut state = pack_reviewed_state();
        state.pack_keep_local = true;
        state.confirm_pack_apply().unwrap();
        let payload = json!({"phase": "downloading"});
        let initial = state.in_flight.as_ref().unwrap().phase.clone();
        assert!(!state.on_host_event("modpack-progress", &payload));
        assert!(!state.on_host_event("modpack-update-progress", &Value::Null));
        assert!(!state.on_host_event("modpack-update-progress", &json!({"phase": 7})));
        assert_eq!(state.in_flight.as_ref().unwrap().phase, initial);
        assert!(state.on_host_event("modpack-update-progress", &payload));
        assert_eq!(
            state.in_flight.as_ref().unwrap().phase,
            "Downloading and verifying pack files…"
        );
        assert!(!state.on_host_event("modpack-update-progress", &payload));
    }

    #[test]
    fn modpack_progress_labels_only_the_running_pack_apply() {
        use aurora_launcher_lib::modpacks::progress_phase as p;
        let mut state = pack_reviewed_state();
        state.pack_keep_local = true;
        assert!(!state.apply_pack_phase(p::DOWNLOADING), "nothing running");
        state.confirm_pack_apply().unwrap();
        assert!(state.apply_pack_phase(p::DOWNLOADING));
        assert!(state.apply_pack_phase(p::INSTALLING_GAME));
        assert!(state.apply_pack_phase("notAPhase"));
        assert_eq!(
            op_label(state.in_flight.as_ref().unwrap()),
            "Applying the modpack update…"
        );
        state.cancel_in_flight();
        assert!(
            !state.apply_pack_phase(p::VALIDATING),
            "cancelling freezes the label"
        );
        assert!(op_label(state.in_flight.as_ref().unwrap()).contains("Cancelling"));
    }

    #[test]
    fn every_core_modpack_phase_has_a_label() {
        for phase in aurora_launcher_lib::modpacks::progress_phase::ALL {
            assert!(pack_phase_label(phase).is_some(), "{phase}");
        }
        assert!(pack_phase_label("notAPhase").is_none());
    }

    // ------------------------------------------------------------ dismissal

    #[test]
    fn dismissing_a_review_emits_nothing_and_cannot_dismiss_work() {
        let mut state = reviewed_state();
        state.clear_review_display();
        assert!(state.review.is_none() && !state.review_open);
        let mut state = reviewed_state();
        let call = state.confirm_apply().unwrap();
        state.clear_review_display();
        assert!(state.busy(), "dismissal never abandons a running mutation");
        let fx = state.receive(Reply::Apply {
            op: call.op,
            serial: call.serial,
            observed_cancellation: false,
            result: Ok(Value::Null),
        });
        assert!(fx.changed);
    }
}
