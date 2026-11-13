//! Retained Downloads queue for the GPUI host.
//!
//! Behavior authority is the Slint queue `native/src/activity.rs` over the
//! shared typed boundary `native/src/api.rs`; the core remains the sole
//! installation authority. Every job is a typed prepared request (built only
//! from a completed Discover review, an explicit quick-install choice, or a
//! kept-instance resume); the core re-resolves each request and its
//! fingerprint at commit. Queue rules preserved from the native host:
//!
//! - Installs targeting the same instance run one at a time, in order;
//!   different instance keys and new-pack jobs run concurrently.
//! - Network concurrency is bounded process-wide by the core's transfer
//!   permits; this queue never opens connections itself.
//! - Cancellation is cooperative through the shared core content-job scope
//!   (`Control::run`): pending work cancels exactly, active writes settle and
//!   commit boundaries finish to their real result.
//! - Lock-contention refusals wait with a bounded, task-scoped one-shot
//!   backoff (five waits); cancelled waiting work never restarts.
//! - Every run carries an exact attempt id, so duplicate or late replies
//!   cannot clear a newer retry or overwrite another task.
//! - Finished history is bounded (30 entries) with an explicit Clear.
//!
//! Disclosed difference: the Slint host's 1 s timer-driven aggregate byte
//! counter is not portable under this host's no-polling rule. Rows show each
//! running job's own core-accounted bytes (`Control::streamed_bytes`) at
//! event-driven renders only; exact continuously-updating aggregate byte
//! totals remain pending. Nothing here polls, animates or fabricates numbers.

use std::time::Duration;

use aurora_launcher_lib::content_jobs::{Control, Phase as JobPhase};
use aurora_launcher_lib::host::Host;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled, Window, div, px,
};
use serde_json::Value;

use crate::backend::guarded;
use crate::controls::{AuroraButton, ButtonSize, ButtonVariant};
use crate::{api, theme, ui};

/// Another short operation (such as the post-install inventory refresh)
/// briefly holds the instance's content lock. Such a refusal changed nothing,
/// so the task waits and tries again a bounded number of times.
const LOCK_CONTENTION: &str = "content_operation_in_progress";
const MAX_LOCK_WAITS: u8 = 5;
/// Keep the panel bounded: at most this many finished entries are retained.
const MAX_FINISHED: usize = 30;

/// Emitted so Discover and the parent can react without polling. `Changed`
/// fires for any task-list mutation; `Settled` additionally fires when a task
/// reaches a terminal phase (completed, failed, or cancelled after its
/// cooperative cancellation settled).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadsEvent {
    Changed,
    Settled {
        ok: bool,
    },
    /// The empty state's "Browse Discover" action; the parent opens Discover.
    OpenDiscover,
}

/// One typed prepared install request. Built only from a completed review
/// (packs, reviewed content), the explicit quick-install path, or a
/// kept-instance resume; commit re-resolves everything in the core.
#[derive(Clone, Debug, PartialEq)]
pub enum JobRequest {
    Quick {
        target: String,
        kind: &'static str,
        project: String,
    },
    Content {
        target: String,
        kind: &'static str,
        project: String,
        version: String,
        fingerprint: String,
    },
    Pack {
        merge: bool,
        request: Value,
    },
    Resume {
        instance: String,
    },
}

impl JobRequest {
    /// The project id used for busy markers (a resume is marked by its
    /// instance id, mirroring the reviewed kind it came from).
    pub(crate) fn project(&self) -> &str {
        match self {
            JobRequest::Quick { project, .. } | JobRequest::Content { project, .. } => project,
            JobRequest::Pack { request, .. } => request["projectId"].as_str().unwrap_or(""),
            JobRequest::Resume { instance } => instance,
        }
    }

    /// The serialization key: the target instance, or `None` for a new
    /// modpack instance that does not exist yet.
    pub(crate) fn key(&self) -> Option<String> {
        match self {
            JobRequest::Quick { target, .. } | JobRequest::Content { target, .. } => {
                Some(target.clone())
            }
            JobRequest::Pack { merge, request } => {
                if *merge {
                    request["instanceId"].as_str().map(str::to_owned)
                } else {
                    None
                }
            }
            JobRequest::Resume { instance } => Some(instance.clone()),
        }
    }
}

/// A frozen, cloneable submission: display strings plus the typed request.
/// Retries re-run the same frozen request (or resume the kept instance the
/// core reported); a consumed review request is never reused or rebuilt.
#[derive(Clone, Debug)]
pub struct PreparedJob {
    pub title: String,
    pub detail: String,
    pub request: JobRequest,
}

impl PreparedJob {
    fn project(&self) -> String {
        self.request.project().to_owned()
    }

    fn key(&self) -> Option<String> {
        self.request.key()
    }
}

async fn execute(host: &Host, request: &JobRequest) -> api::ApiResult<Value> {
    match request {
        JobRequest::Quick {
            target,
            kind,
            project,
        } => api::quick_install(host.clone(), target.clone(), kind, project.clone()).await,
        JobRequest::Content {
            target,
            kind,
            project,
            version,
            fingerprint,
        } => {
            api::install_content(
                host.clone(),
                target.clone(),
                kind,
                project.clone(),
                version.clone(),
                fingerprint.clone(),
            )
            .await
        }
        JobRequest::Pack { merge, request } => {
            api::install_pack(host.clone(), *merge, request.clone()).await
        }
        JobRequest::Resume { instance } => api::resume_pack(host.clone(), instance.clone()).await,
    }
}

/// A run that ended after its cancellation was observed. A kept instance
/// stays named, with the core's own wording when it already explains it.
fn cancelled(error: api::ApiError) -> api::ApiError {
    let message = match &error.instance_id {
        Some(_) if error.code == "installation_cancelled" => error.message,
        Some(_) => {
            "Installation cancelled. The new instance was kept and can be resumed from Downloads or Instances."
                .into()
        }
        None => "Installation cancelled. No uncommitted content was activated.".into(),
    };
    api::ApiError {
        code: "installation_cancelled".into(),
        message,
        instance_id: error.instance_id,
    }
}

/// The truthful outcome of a completed task, from the core's own result: a
/// new (or resumed) pack instance, a merge into an existing instance, or
/// ordinary content.
fn completion_message(value: &Value) -> String {
    if let (Some(added), Some(kept)) = (value["added"].as_u64(), value["keptExisting"].as_array()) {
        let mut message = format!(
            "Added {added} files; {} already present.",
            value["alreadyPresent"].as_u64().unwrap_or(0)
        );
        if !kept.is_empty() {
            message.push_str(&format!(
                " Kept {} of your differing config files.",
                kept.len()
            ));
        }
        message.push_str(" The instance will not follow pack updates.");
        return message;
    }
    if let (Some(name), Some(_)) = (value["displayName"].as_str(), value["id"].as_str()) {
        return format!("Created and verified the instance '{name}'.");
    }
    "Installed and verified.".into()
}

/// Labels for the core's `modpack-progress` vocabulary (a short Downloads-row
/// stage and a sentence). `None` for a name the core does not emit.
pub(crate) fn modpack_phase(phase: &str) -> Option<(&'static str, &'static str)> {
    use aurora_launcher_lib::modpacks::progress_phase as p;
    Some(match phase {
        p::RESOLVING_PACK => ("Resolving", "Verifying the exact modpack version"),
        p::DOWNLOADING => ("Downloading", "Downloading and verifying pack files"),
        p::PREPARING_INSTANCE => ("Preparing", "Preparing the new instance"),
        p::INSTALLING_GAME => ("Installing game", "Installing Minecraft and the mod loader"),
        p::INSTALLING_OVERRIDES => ("Writing files", "Writing pack files and reviewed overrides"),
        p::INSTALLING_COMPONENTS => ("Installing mods", "Installing verified pack components"),
        p::VALIDATING => ("Validating", "Validating the installation"),
        p::COMPLETE => ("Finishing", "Finishing the installation"),
        _ => return None,
    })
}

/// The sidebar label (`app.slint` downloads-nav): the active count rides in
/// the accessible name, while sighted users get the count badge.
pub(crate) fn nav_label(active: usize) -> String {
    if active > 0 {
        format!("Downloads ({active} active)")
    } else {
        "Downloads".into()
    }
}

/// Counts-only summary line. The Slint host appends its timer-driven byte
/// total; see the module note on why this host never fabricates one.
fn queue_summary(running: usize, pending: usize) -> String {
    if running + pending == 0 {
        "Nothing downloading.".to_owned()
    } else {
        format!("{running} installing Â· {pending} queued")
    }
}

/// What the Slint host toasts when a task settles. The parent shows it in its
/// status line, so a finished install is visible away from Downloads too.
fn settle_notice(result: &api::ApiResult<Value>) -> String {
    match result {
        Ok(_) => "Installation completed and verified.".into(),
        Err(error) if error.code == "installation_cancelled" => "Installation cancelled.".into(),
        Err(error) => error.message.clone(),
    }
}

/// Shown instead of the rows while the queue is empty.
/// The empty-queue state: `ui::empty_state` with the body held to a 400px
/// measure so the sentence wraps into balanced lines.
fn empty_queue(t: &crate::theme::AuroraTheme, title: String, body: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(10.))
        .py(px(40.))
        .px(px(24.))
        .rounded(t.radius_lg)
        .bg(t.card)
        .border_1()
        .border_color(t.border_subtle)
        .child(ui::icon_tile(t, Some("download"), 48., true))
        .child(
            div()
                .pt(px(4.))
                .text_size(px(ui::TEXT_SECTION))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(t.foreground)
                .child(title),
        )
        .child(
            div()
                .max_w(px(400.))
                .text_size(px(ui::TEXT_BODY))
                .text_color(t.muted_foreground)
                .child(body),
        )
}

const EMPTY_TEXT: &str = "Installs you start from Discover appear here. Several can run at once; installs into the same instance take turns.";

fn phase_label(phase: Phase) -> &'static str {
    match phase {
        Phase::Pending => "Queued",
        Phase::Running => "Downloading",
        Phase::Completed => "Completed",
        Phase::Failed => "Failed",
        Phase::Cancelled => "Cancelled",
    }
}

/// A running task's row stage and message. A modpack job reports its own named
/// step (attributed through its content-job control, so concurrent packs never
/// share one); other jobs show the content-job phase.
fn running_status(phase: JobPhase, stage: Option<&str>, message: &str) -> (&'static str, String) {
    if phase == JobPhase::Cancelling {
        return ("Cancelling", message.to_owned());
    }
    if let Some((stage, label)) = stage.and_then(modpack_phase) {
        let stage = match (stage, phase) {
            // Downloads verify each file after transfer.
            ("Downloading", JobPhase::Verifying) => "Verifying",
            (stage, _) => stage,
        };
        return (stage, format!("{label}â€¦"));
    }
    let stage = match phase {
        JobPhase::Queued | JobPhase::Downloading => "Downloading",
        JobPhase::Verifying => "Verifying",
        JobPhase::Installing => "Installing",
        JobPhase::Cancelling => "Cancelling",
    };
    (stage, message.to_owned())
}

/// Whether a host event is a well-formed progress notification. The payload
/// carries no task or instance identity, so it is advisory only: per-task
/// stages come from each job's own control (`stage()`), read at render.
fn progress_event(name: &str, payload: &Value) -> bool {
    name == "modpack-progress" && payload["phase"].as_str().is_some()
}

// ---------------------------------------------------------------- pure state

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

struct Task {
    id: u64,
    /// The exact run attempt; completions for any other attempt are stale.
    attempt: u64,
    title: String,
    detail: String,
    /// Serialization key: the target instance, or None for a new instance.
    key: Option<String>,
    project: String,
    phase: Phase,
    message: String,
    request: JobRequest,
    /// Automatic waits after transient instance-lock contention.
    lock_waits: u8,
    control: Control,
    /// A kept instance a failed or cancelled new-pack install created; Retry
    /// resumes it instead of installing (and duplicating) again.
    resume: Option<String>,
    /// Cancel was requested for the current run.
    cancelling: bool,
}

impl Task {
    /// Remembers the kept instance a failed or cancelled run reports. From
    /// then on the task's key is that instance, so it serializes with other
    /// work on it, and Retry resumes it. A failure before any instance was
    /// created reports none and keeps the original job.
    fn remember_kept_instance(&mut self, error: &api::ApiError) {
        if let Some(instance) = &error.instance_id {
            self.key = Some(instance.clone());
            self.resume = Some(instance.clone());
        }
    }

    /// Retry of a kept instance resumes it, and says so.
    fn retry_label(&self) -> &'static str {
        if self.resume.is_some() {
            "Waiting to resume the kept instance."
        } else {
            "Waiting to retry."
        }
    }

    fn start_label(&self) -> &'static str {
        if self.resume.is_some() {
            "Resuming the kept instanceâ€¦"
        } else {
            "Downloading and verifyingâ€¦"
        }
    }

    /// The work the next run performs.
    fn next_request(&self) -> JobRequest {
        match &self.resume {
            Some(instance) => JobRequest::Resume {
                instance: instance.clone(),
            },
            None => self.request.clone(),
        }
    }
}

/// One run the pump asks the runtime to start.
struct Start {
    id: u64,
    attempt: u64,
    request: JobRequest,
    control: Control,
}

/// What `apply_finished` produced: `None` for a stale reply.
enum Settled {
    /// Transient lock contention: wait once, then pump again.
    Backoff(Duration),
    /// The task reached a terminal phase.
    Terminal { ok: bool },
}

impl std::fmt::Debug for Settled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Settled::Backoff(delay) => write!(f, "Backoff({delay:?})"),
            Settled::Terminal { ok } => write!(f, "Terminal {{ ok: {ok} }}"),
        }
    }
}

/// Everything the queue presents, with every transition executable without a
/// GPUI context so ordering, retry and cancellation are unit-testable.
#[derive(Default)]
struct DownloadsState {
    next_id: u64,
    tasks: Vec<Task>,
    /// The most recent terminal outcome, worded like the Slint toast.
    notice: Option<String>,
}

impl DownloadsState {
    fn is_busy(&self) -> bool {
        self.tasks
            .iter()
            .any(|t| matches!(t.phase, Phase::Pending | Phase::Running))
    }

    /// Queued plus running tasks (`Activity.active`).
    fn active_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| matches!(t.phase, Phase::Pending | Phase::Running))
            .count()
    }

    fn instance_busy(&self, id: &str) -> bool {
        self.tasks.iter().any(|t| {
            t.key.as_deref() == Some(id) && matches!(t.phase, Phase::Pending | Phase::Running)
        })
    }

    fn busy_projects(&self) -> Vec<String> {
        self.tasks
            .iter()
            .filter(|t| matches!(t.phase, Phase::Pending | Phase::Running))
            .map(|t| t.project.clone())
            .collect()
    }

    fn busy_instances(&self) -> Vec<String> {
        self.tasks
            .iter()
            .filter(|t| t.key.is_some() && matches!(t.phase, Phase::Pending | Phase::Running))
            .filter_map(|t| t.key.clone())
            .collect()
    }

    /// Queues one prepared job and keeps the panel bounded by dropping the
    /// oldest finished entries. Active work is never dropped.
    fn submit(&mut self, job: PreparedJob) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let key = job.key();
        let project = job.project();
        self.tasks.push(Task {
            id,
            attempt: 0,
            title: job.title,
            detail: job.detail,
            key,
            project,
            phase: Phase::Pending,
            message: "Queued.".into(),
            request: job.request,
            lock_waits: 0,
            control: Control::default(),
            resume: None,
            cancelling: false,
        });
        self.trim_history();
        id
    }

    fn trim_history(&mut self) {
        while self.tasks.len() > MAX_FINISHED {
            if let Some(at) = self
                .tasks
                .iter()
                .position(|t| !matches!(t.phase, Phase::Pending | Phase::Running))
            {
                self.tasks.remove(at);
            } else {
                break;
            }
        }
    }

    /// Starts every pending task whose instance has nothing running, in
    /// submission order. Each start bumps the task's attempt id so late or
    /// duplicate completions can never match a newer run. Like the native
    /// host, this pump may also restart a lock-contention waiter early; the
    /// task-scoped backoff is only the guaranteed wakeup, never a lock.
    fn starts(&mut self) -> Vec<Start> {
        let mut started = Vec::new();
        let mut running_keys: Vec<Option<String>> = self
            .tasks
            .iter()
            .filter(|t| t.phase == Phase::Running && t.key.is_some())
            .map(|t| t.key.clone())
            .collect();
        for task in self.tasks.iter_mut().filter(|t| t.phase == Phase::Pending) {
            if task.key.is_some() && running_keys.contains(&task.key) {
                task.message = "Waiting for this instance's other install.".into();
                continue;
            }
            task.phase = Phase::Running;
            task.cancelling = false;
            task.message = task.start_label().into();
            if let Some(key) = task.key.clone() {
                running_keys.push(Some(key));
            }
            task.attempt += 1;
            started.push(Start {
                id: task.id,
                attempt: task.attempt,
                request: task.next_request(),
                control: task.control.clone(),
            });
        }
        started
    }

    /// Exact pending cancellation; a running task only requests cooperation
    /// (active writes and any commit in progress must finish safely).
    fn cancel(&mut self, id: u64) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        if !matches!(task.phase, Phase::Pending | Phase::Running) {
            return false;
        }
        task.control.cancel();
        if task.phase == Phase::Pending {
            task.phase = Phase::Cancelled;
            task.message = "Cancelled before it started.".into();
        } else if !task.cancelling {
            task.cancelling = true;
            task.message =
                "Cancellingâ€¦ Active writes and any commit in progress must finish safely.".into();
        }
        self.trim_history();
        true
    }

    fn cancel_and_start(&mut self, id: u64) -> Option<Vec<Start>> {
        self.cancel(id).then(|| self.starts())
    }

    /// Retry of a failed or cancelled task: a fresh control, a fresh lock-wait
    /// budget, and the kept-instance resume when the core kept one.
    fn retry(&mut self, id: u64) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        if !matches!(task.phase, Phase::Failed | Phase::Cancelled) {
            return false;
        }
        task.control = Control::default();
        task.lock_waits = 0;
        task.cancelling = false;
        task.phase = Phase::Pending;
        task.message = task.retry_label().into();
        true
    }

    /// Explicit Clear: keeps queued and running work only.
    fn clear_finished(&mut self) -> bool {
        let before = self.tasks.len();
        self.tasks
            .retain(|t| matches!(t.phase, Phase::Pending | Phase::Running));
        before != self.tasks.len()
    }

    /// Whether a task is still waiting out the exact backoff that scheduled
    /// this wakeup. A cancelled, restarted or cleared wait is stale, so
    /// cancelled waiting work can never restart through its timer.
    fn waiting(&self, id: u64, attempt: u64) -> bool {
        self.tasks.iter().any(|t| {
            t.id == id
                && t.attempt == attempt
                && t.phase == Phase::Pending
                && t.lock_waits > 0
                && t.message == "Waiting for another change to this instance to finish."
        })
    }

    /// Applies one completion. Stale replies (wrong task, wrong attempt, or a
    /// task no longer running) change nothing.
    fn apply_finished(
        &mut self,
        id: u64,
        attempt: u64,
        result: api::ApiResult<Value>,
    ) -> Option<Settled> {
        let task = self.tasks.iter_mut().find(|t| t.id == id)?;
        if task.attempt != attempt || task.phase != Phase::Running {
            return None;
        }
        if let Err(error) = &result {
            task.remember_kept_instance(error);
        }
        let settled = match &result {
            Err(error) if error.code == "installation_cancelled" => {
                task.phase = Phase::Cancelled;
                task.message = error.message.clone();
                Some(Settled::Terminal { ok: false })
            }
            Err(error) if error.code == LOCK_CONTENTION && task.lock_waits < MAX_LOCK_WAITS => {
                task.lock_waits += 1;
                task.phase = Phase::Pending;
                task.cancelling = false;
                task.message = "Waiting for another change to this instance to finish.".into();
                Some(Settled::Backoff(Duration::from_millis(
                    800 * u64::from(task.lock_waits),
                )))
            }
            Ok(value) => {
                task.phase = Phase::Completed;
                task.message = completion_message(value);
                Some(Settled::Terminal { ok: true })
            }
            Err(error) => {
                task.phase = Phase::Failed;
                task.message = format!("{} ({})", error.message, error.code);
                Some(Settled::Terminal { ok: false })
            }
        };
        if matches!(settled, Some(Settled::Terminal { .. })) {
            self.notice = Some(settle_notice(&result));
            self.trim_history();
        }
        settled
    }
}

// ------------------------------------------------------------------- view

enum Message {
    /// A run finished for one exact task attempt.
    Finished {
        id: u64,
        attempt: u64,
        result: api::ApiResult<Value>,
    },
    /// A lock-contention backoff elapsed for one exact task attempt.
    Backoff { id: u64, attempt: u64 },
}

/// The retained GPUI Downloads view. Completions arrive from the shared Tokio
/// runtime through one awaited channel (no polling, no idle animation); the
/// view notifies only when a message changed state.
pub struct Downloads {
    state: DownloadsState,
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    tx: async_channel::Sender<Message>,
    blocked: bool,
}

impl EventEmitter<DownloadsEvent> for Downloads {}

impl Downloads {
    /// Builds the retained queue. `host: None` constructs the diagnostic view
    /// that queues nothing and makes zero API calls.
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded();
        // Event-driven pump: completions arrive as messages; nothing polls.
        cx.spawn(async move |weak, cx| {
            while let Ok(message) = rx.recv().await {
                if weak
                    .update(cx, |view, cx| view.receive(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Downloads {
            state: DownloadsState::default(),
            host,
            rt: runtime,
            tx,
            blocked: false,
        }
    }

    /// Queues one prepared job. Returns `false` (and queues nothing) in the
    /// diagnostic view.
    pub fn submit(&mut self, job: PreparedJob, cx: &mut Context<Self>) -> bool {
        if self.host.is_none() || self.blocked {
            return false;
        }
        self.state.submit(job);
        self.pump();
        cx.emit(DownloadsEvent::Changed);
        cx.notify();
        true
    }

    /// True while any task is queued or running.
    pub fn is_busy(&self) -> bool {
        self.state.is_busy()
    }

    /// Queued plus running tasks, for the sidebar indicator.
    pub fn active_count(&self) -> usize {
        self.state.active_count()
    }

    /// The latest settled outcome in the Slint toast's wording; read it when
    /// handling [`DownloadsEvent::Settled`].
    pub fn notice(&self) -> Option<&str> {
        self.state.notice.as_deref()
    }

    /// Gate new submissions and retries during root launch/preparation or a
    /// Client update. Cancellation and finished-history controls remain usable.
    pub fn set_blocked(&mut self, blocked: bool, cx: &mut Context<Self>) {
        if self.blocked != blocked {
            self.blocked = blocked;
            cx.notify();
        }
    }

    /// True while the instance has a queued or running task (its key).
    pub fn instance_busy(&self, id: &str) -> bool {
        self.state.instance_busy(id)
    }

    /// Projects with a queued or running task.
    pub fn busy_projects(&self) -> Vec<String> {
        self.state.busy_projects()
    }

    /// Instances with a queued or running task.
    pub fn busy_instances(&self) -> Vec<String> {
        self.state.busy_instances()
    }

    /// Routes one host event. `modpack-progress` carries no task or instance
    /// identity, so it is advisory only: each running row reads its own
    /// control's stage and bytes at render. The event just requests a
    /// re-render; unknown names and malformed payloads are ignored, never
    /// fatal.
    pub fn on_host_event(&mut self, name: &str, payload: &Value, cx: &mut Context<Self>) {
        if progress_event(name, payload) {
            cx.notify();
        }
    }

    /// Exact pending cancellation; a running task settles cooperatively.
    pub fn cancel(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(starts) = self.state.cancel_and_start(id) {
            // A cancelled backoff has an intentionally stale wakeup. Start
            // the next eligible task now so that it cannot lose its only wake.
            self.dispatch(starts);
            cx.emit(DownloadsEvent::Changed);
            cx.notify();
        }
    }

    /// Retries a failed or cancelled task (resuming a kept instance when the
    /// core kept one).
    pub fn retry(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.blocked {
            return;
        }
        if self.state.retry(id) {
            self.pump();
            cx.emit(DownloadsEvent::Changed);
            cx.notify();
        }
    }

    /// Removes finished entries; queued and running work is retained.
    pub fn clear_finished(&mut self, cx: &mut Context<Self>) {
        if self.state.clear_finished() {
            cx.emit(DownloadsEvent::Changed);
            cx.notify();
        }
    }

    fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Finished {
                id,
                attempt,
                result,
            } => {
                let terminal = match self.state.apply_finished(id, attempt, result) {
                    Some(Settled::Backoff(delay)) => {
                        // Task-scoped one-shot backoff on the runtime, like
                        // the native host's single shot: no pump here â€” the
                        // wakeup (or an unrelated completion's pump) restarts
                        // the task. The (id, attempt) guard makes a cancelled,
                        // restarted or cleared wait stale.
                        let tx = self.tx.clone();
                        self.rt.spawn(async move {
                            tokio::time::sleep(delay).await;
                            let _ = tx.send(Message::Backoff { id, attempt }).await;
                        });
                        false
                    }
                    Some(Settled::Terminal { ok }) => {
                        cx.emit(DownloadsEvent::Settled { ok });
                        true
                    }
                    None => return,
                };
                cx.emit(DownloadsEvent::Changed);
                if terminal {
                    self.pump();
                }
                cx.notify();
            }
            Message::Backoff { id, attempt } => {
                if self.state.waiting(id, attempt) {
                    self.pump();
                    cx.emit(DownloadsEvent::Changed);
                    cx.notify();
                }
            }
        }
    }

    /// Starts every pending task whose instance is free. Cancellation is
    /// cooperative through the core content-job scope; the pre-activation
    /// checkpoint turns an already-cancelled start into an exact
    /// cancellation, and commit boundaries settle to their real result.
    fn pump(&mut self) {
        let starts = self.state.starts();
        self.dispatch(starts);
    }

    fn dispatch(&self, starts: Vec<Start>) {
        for start in starts {
            let Some(host) = self.host.clone() else {
                break;
            };
            let tx = self.tx.clone();
            let run = start.control.clone();
            let check = start.control.clone();
            let request = start.request;
            self.rt.spawn(async move {
                let result = guarded(async move {
                    run.run(async move {
                        if aurora_launcher_lib::content_jobs::checkpoint().is_err() {
                            return Err(api::ApiError::new(
                                "installation_cancelled",
                                "Installation cancelled before activation.",
                            ));
                        }
                        let result = execute(&host, &request).await;
                        match result {
                            Err(error)
                                if check.cancellation_observed()
                                    && !error.code.contains("rollback") =>
                            {
                                Err(cancelled(error))
                            }
                            other => other,
                        }
                    })
                    .await
                })
                .await;
                let _ = tx
                    .send(Message::Finished {
                        id: start.id,
                        attempt: start.attempt,
                        result,
                    })
                    .await;
            });
        }
    }
}

/// How far through its pipeline a running job is, from its named stage only.
/// The core reports no total byte count, so this is a stage indicator, never a
/// transfer percentage.
fn stage_fraction(stage: &str) -> f32 {
    match stage {
        "Downloading" => 0.34,
        "Verifying" => 0.67,
        "Installing" => 0.9,
        _ => 0.12,
    }
}

/// Everything one row presents, so the row is a pure function of plain data.
struct RowData {
    title: String,
    detail: String,
    phase: Phase,
    stage: &'static str,
    message: String,
    streamed: Option<u64>,
}

fn phase_tone(phase: Phase) -> ui::Tone {
    match phase {
        Phase::Pending => ui::Tone::Neutral,
        Phase::Running => ui::Tone::Accent,
        Phase::Completed => ui::Tone::Success,
        Phase::Failed => ui::Tone::Danger,
        Phase::Cancelled => ui::Tone::Warning,
    }
}

fn phase_icon(phase: Phase) -> &'static str {
    match phase {
        Phase::Pending => "schedule",
        Phase::Running => "download",
        Phase::Completed => "check_circle",
        Phase::Failed => "error",
        Phase::Cancelled => "block",
    }
}

/// One download row: icon, title and progress, status badge, trailing action.
fn download_row(
    t: &theme::AuroraTheme,
    data: &RowData,
    action: Option<gpui_kit::AnyElement>,
    last: bool,
) -> gpui_kit::Div {
    let running = data.phase == Phase::Running;
    let note = match data.streamed {
        // This job's own core-accounted bytes, read at this event-driven
        // render only; not a live counter.
        Some(bytes) => format!(
            "{} Â· {:.1} MB received",
            data.message,
            bytes as f64 / (1024.0 * 1024.0)
        ),
        None => data.message.clone(),
    };
    let badge_label = if running || data.phase == Phase::Pending {
        data.stage
    } else {
        phase_label(data.phase)
    };
    let mut body =
        ui::title_block(t, data.title.clone(), Some(data.detail.clone().into())).flex_1();
    if running {
        body = body.child(
            div()
                .mt(px(6.))
                .h(px(6.))
                .w_full()
                .rounded_full()
                .bg(t.surface_hover)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(gpui_kit::relative(stage_fraction(data.stage)))
                        .rounded_full()
                        .bg(t.accent),
                ),
        );
    }
    if !note.is_empty() {
        body = body.child(
            div()
                .when(running, |d| d.pt(px(2.)))
                .text_size(px(ui::TEXT_CAPTION))
                .text_color(if data.phase == Phase::Failed {
                    t.danger
                } else {
                    t.subtle_foreground
                })
                .text_ellipsis()
                .child(note),
        );
    }
    ui::row(t, last)
        .gap(px(14.))
        .py(px(14.))
        .child(ui::icon_tile(t, Some(phase_icon(data.phase)), 38., running))
        .child(body)
        .child(ui::status_badge(t, badge_label, phase_tone(data.phase)))
        .when_some(action, |row, action| {
            row.child(div().flex().flex_shrink_0().child(action))
        })
}

impl Render for Downloads {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx).clone();
        let t = &palette;
        if self.host.is_none() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .gap(px(ui::PAGE_GAP))
                .child(ui::page_header(
                    t,
                    "Downloads",
                    Some("Installs you start appear here.".into()),
                ))
                .child(ui::empty_state(
                    t,
                    "download",
                    "Downloads are unavailable",
                    "Downloads is disabled in diagnostic mode. No installs are queued or run.",
                ))
                .into_any_element();
        }

        let s = &self.state;
        let running = s.tasks.iter().filter(|t| t.phase == Phase::Running).count();
        let pending = s.tasks.iter().filter(|t| t.phase == Phase::Pending).count();
        // No timer under this host's no-polling rule: the summary states
        // counts only, never a fabricated byte total.
        let summary = queue_summary(running, pending);
        let any_finished = s
            .tasks
            .iter()
            .any(|t| !matches!(t.phase, Phase::Pending | Phase::Running));

        let mut active_rows = Vec::new();
        let mut finished_rows = Vec::new();
        for task in s.tasks.iter().rev() {
            let (stage, message, streamed) = if task.phase == Phase::Running {
                let (stage, message) =
                    running_status(task.control.phase(), task.control.stage(), &task.message);
                let bytes = task.control.streamed_bytes();
                (stage, message, (bytes > 0).then_some(bytes))
            } else {
                (phase_label(task.phase), task.message.clone(), None)
            };
            let cancelling = task.cancelling || task.control.phase() == JobPhase::Cancelling;
            let can_cancel = matches!(task.phase, Phase::Pending | Phase::Running) && !cancelling;
            let can_retry = matches!(task.phase, Phase::Failed | Phase::Cancelled);
            let action = if can_cancel {
                let id = task.id;
                Some(
                    AuroraButton::new(("cancel-download", id as usize), "Cancel")
                        .size(ButtonSize::Small)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.cancel(id, cx);
                        }))
                        .into_any_element(),
                )
            } else if can_retry {
                let id = task.id;
                // Slint labels both cases "Retry"; the row message already
                // says when the retry resumes a kept instance.
                Some(
                    AuroraButton::new(("retry-download", id as usize), "Retry")
                        .size(ButtonSize::Small)
                        .icon("refresh")
                        .disabled(self.blocked)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.retry(id, cx);
                        }))
                        .into_any_element(),
                )
            } else {
                None
            };
            let data = RowData {
                title: task.title.clone(),
                detail: task.detail.clone(),
                phase: task.phase,
                stage,
                message,
                streamed,
            };
            if matches!(task.phase, Phase::Pending | Phase::Running) {
                active_rows.push((data, action));
            } else {
                finished_rows.push((data, action));
            }
        }
        let list = |rows: Vec<(RowData, Option<gpui_kit::AnyElement>)>| {
            let count = rows.len();
            let mut card = ui::list_card(t);
            for (index, (data, action)) in rows.into_iter().enumerate() {
                card = card.child(download_row(t, &data, action, index + 1 == count));
            }
            card
        };

        let mut page = div()
            .id("downloads-rows")
            .size_full()
            .flex()
            .flex_col()
            .gap(px(ui::PAGE_GAP))
            .child(ui::page_header(
                t,
                "Downloads",
                Some(if active_rows.is_empty() && finished_rows.is_empty() {
                    "Installs you start appear here.".into()
                } else {
                    summary.clone().into()
                }),
            ));
        if active_rows.is_empty() && finished_rows.is_empty() {
            page = page.child(
                empty_queue(t, summary.clone(), EMPTY_TEXT)
                    .h(px(340.))
                    .justify_center()
                    .child(
                        div().pt(px(8.)).child(
                            AuroraButton::new("downloads-browse-discover", "Browse Discover")
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Medium)
                                .icon("explore")
                                .on_click(cx.listener(|_, _, _, cx| {
                                    cx.emit(DownloadsEvent::OpenDiscover);
                                })),
                        ),
                    ),
            );
        }
        if !active_rows.is_empty() {
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(ui::section_header(
                        t,
                        "Active",
                        Some(summary.clone().into()),
                    ))
                    .child(list(active_rows)),
            );
        }
        if !finished_rows.is_empty() || any_finished {
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(ui::section_header(
                                t,
                                "Finished",
                                Some(
                                    format!(
                                        "Your last {MAX_FINISHED} finished installs are kept here."
                                    )
                                    .into(),
                                ),
                            ))
                            .when(any_finished, |header| {
                                header.child(
                                    AuroraButton::new("clear-finished", "Clear finished")
                                        .size(ButtonSize::Small)
                                        .variant(ButtonVariant::Ghost)
                                        .icon("delete")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.clear_finished(cx);
                                        })),
                                )
                            }),
                    )
                    .child(list(finished_rows)),
            );
        }
        page.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;

    fn job(key: Option<&str>, project: &str) -> PreparedJob {
        let request = match key {
            // A merge pack carries its instance in the frozen request.
            Some(instance) => JobRequest::Pack {
                merge: true,
                request: json!({"projectId": project, "instanceId": instance, "fingerprint": "fp"}),
            },
            None => JobRequest::Pack {
                merge: false,
                request: json!({"projectId": project, "fingerprint": "fp"}),
            },
        };
        PreparedJob {
            title: project.to_owned(),
            detail: "Detail".into(),
            request,
        }
    }

    fn error(code: &str, instance: Option<&str>) -> api::ApiError {
        let mut error = api::ApiError::new(code, "message");
        error.instance_id = instance.map(str::to_owned);
        error
    }

    fn finish(
        state: &mut DownloadsState,
        id: u64,
        result: api::ApiResult<Value>,
    ) -> Option<Settled> {
        let attempt = state
            .tasks
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.attempt)
            .unwrap_or(0);
        state.apply_finished(id, attempt, result)
    }

    #[test]
    fn jobs_on_one_instance_run_in_order_but_keys_run_concurrently() {
        let mut state = DownloadsState::default();
        let a = state.submit(job(Some("i1"), "A"));
        let b = state.submit(job(Some("i1"), "B"));
        let c = state.submit(job(Some("i2"), "C"));
        let pack = state.submit(job(None, "PACK"));
        let started = state.starts();
        assert_eq!(
            started.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![a, c, pack],
            "the second i1 job waits"
        );
        assert_eq!(
            state.tasks.iter().find(|t| t.id == b).unwrap().message,
            "Waiting for this instance's other install."
        );
        // First attempt ids are exact and unique per task.
        assert!(started.iter().all(|s| s.attempt == 1));

        finish(&mut state, a, Ok(json!({"installed":[]})));
        let started = state.starts();
        assert_eq!(started.iter().map(|s| s.id).collect::<Vec<_>>(), vec![b]);
        assert_eq!(started[0].attempt, 1);
        finish(&mut state, b, Ok(json!({"installed":[]})));
        finish(&mut state, c, Ok(json!({"installed":[]})));
        finish(&mut state, pack, Ok(json!({"installed":[]})));
        assert!(!state.is_busy());
        assert!(state.starts().is_empty());
    }

    #[test]
    fn a_retry_runs_a_new_attempt_and_late_replies_cannot_clear_it() {
        let mut state = DownloadsState::default();
        let id = state.submit(job(None, "P"));
        state.starts();
        let first = state.tasks.iter().find(|t| t.id == id).unwrap().attempt;
        // The failed run settles terminally.
        assert!(matches!(
            state.apply_finished(id, first, Err(error("pack_download_failed", None))),
            Some(Settled::Terminal { ok: false })
        ));
        // A duplicate terminal reply for the settled attempt changes nothing.
        assert!(
            state
                .apply_finished(id, first, Ok(json!({"installed":[]})))
                .is_none()
        );
        assert_eq!(
            state.tasks.iter().find(|t| t.id == id).unwrap().phase,
            Phase::Failed
        );

        assert!(state.retry(id));
        let started = state.starts();
        assert_eq!(started[0].attempt, first + 1, "retry bumps the attempt");
        // The old attempt's reply cannot clear the newer retry.
        assert!(
            state
                .apply_finished(id, first, Ok(json!({"installed":[]})))
                .is_none()
        );
        assert_eq!(
            state.tasks.iter().find(|t| t.id == id).unwrap().phase,
            Phase::Running
        );
        // The new attempt's reply settles it.
        assert!(matches!(
            state.apply_finished(id, first + 1, Ok(json!({"installed":[]}))),
            Some(Settled::Terminal { ok: true })
        ));
    }

    #[test]
    fn a_completion_for_another_task_never_overwrites_this_one() {
        let mut state = DownloadsState::default();
        let a = state.submit(job(None, "A"));
        let b = state.submit(job(None, "B"));
        state.starts();
        // Unknown task ids, and stale attempts, change nothing.
        assert!(state.apply_finished(99, 1, Ok(json!({}))).is_none());
        assert!(state.apply_finished(a, 7, Ok(json!({}))).is_none());
        assert!(matches!(
            finish(&mut state, b, Err(error("pack_download_failed", None))),
            Some(Settled::Terminal { ok: false })
        ));
        assert_eq!(
            state.tasks.iter().find(|t| t.id == a).unwrap().phase,
            Phase::Running
        );
    }

    #[test]
    fn pending_cancellation_is_exact_and_never_starts() {
        let mut state = DownloadsState::default();
        let other = state.submit(job(Some("i1"), "Other"));
        let queued = state.submit(job(Some("i1"), "Queued"));
        state.starts();
        assert!(state.cancel(queued));
        let task = state.tasks.iter().find(|t| t.id == queued).unwrap();
        assert_eq!(task.phase, Phase::Cancelled);
        assert_eq!(task.message, "Cancelled before it started.");
        assert!(
            state.starts().is_empty(),
            "cancelled waiting work never restarts"
        );
        // Cancelling something finished changes nothing.
        finish(&mut state, other, Ok(json!({"installed":[]})));
        assert!(!state.cancel(other));
        // Restarting is always an explicit user action: an explicit Retry
        // requeues the cancelled task, never an automatic pump.
        assert!(
            state.retry(queued),
            "an explicit Retry requeues a cancelled task"
        );
        assert_eq!(
            state.tasks.iter().find(|t| t.id == queued).unwrap().phase,
            Phase::Pending
        );
    }

    #[test]
    fn running_cancellation_is_cooperative_and_settles_cancelled() {
        let mut state = DownloadsState::default();
        let id = state.submit(job(None, "P"));
        state.starts();
        assert!(state.cancel(id));
        let task = state.tasks.iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.control.phase(), JobPhase::Cancelling);
        assert!(task.message.contains("Cancelling"));
        assert_eq!(task.phase, Phase::Running, "the run settles on its own");
        // A second request changes nothing.
        assert!(state.cancel(id));
        assert!(matches!(
            finish(&mut state, id, Err(error("installation_cancelled", None))),
            Some(Settled::Terminal { ok: false })
        ));
        let task = state.tasks.iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.phase, Phase::Cancelled);
        assert_eq!(task.message, "message");
    }

    #[test]
    fn lock_contention_backs_off_five_times_then_fails() {
        let mut state = DownloadsState::default();
        let id = state.submit(job(Some("i1"), "A"));
        state.starts();
        let mut attempt = state.tasks.iter().find(|t| t.id == id).unwrap().attempt;
        for wait in 1..=5 {
            match state.apply_finished(id, attempt, Err(error(LOCK_CONTENTION, None))) {
                Some(Settled::Backoff(delay)) => {
                    assert_eq!(delay, Duration::from_millis(800 * wait));
                    // The requeue itself never restarts the run; the wakeup
                    // (or an unrelated completion's pump) does.
                    assert_eq!(
                        state.tasks.iter().find(|t| t.id == id).unwrap().phase,
                        Phase::Pending
                    );
                    // The wakeup is valid only while exactly this wait stands,
                    // and pumping it restarts a fresh attempt.
                    assert!(state.waiting(id, attempt));
                    let started = state.starts();
                    assert_eq!(started.iter().map(|s| s.id).collect::<Vec<_>>(), vec![id]);
                    assert_eq!(started[0].attempt, attempt + 1);
                    attempt += 1;
                }
                other => panic!("wait {wait}: {other:?}"),
            }
        }
        // The sixth contention refusal is terminal: the budget is exhausted.
        assert!(matches!(
            state.apply_finished(id, attempt, Err(error(LOCK_CONTENTION, None))),
            Some(Settled::Terminal { ok: false })
        ));
        assert!(!state.waiting(id, attempt));
        assert_eq!(
            state.tasks.iter().find(|t| t.id == id).unwrap().phase,
            Phase::Failed
        );
        // An explicit retry resets the wait budget.
        assert!(state.retry(id));
        assert_eq!(
            state.tasks.iter().find(|t| t.id == id).unwrap().lock_waits,
            0
        );
    }

    #[test]
    fn an_unrelated_completion_may_restart_a_waiter_with_a_fresh_attempt() {
        // Native queue behavior: the backoff is the guaranteed wakeup, never
        // a lock; another task's pump may restart the waiter earlier.
        let mut state = DownloadsState::default();
        let waiter = state.submit(job(Some("i1"), "A"));
        let other = state.submit(job(None, "B"));
        state.starts();
        let wait_attempt = state.tasks.iter().find(|t| t.id == waiter).unwrap().attempt;
        assert!(matches!(
            state.apply_finished(waiter, wait_attempt, Err(error(LOCK_CONTENTION, None))),
            Some(Settled::Backoff(_))
        ));
        // The other task finishes; its pump restarts the waiter.
        assert!(matches!(
            finish(&mut state, other, Ok(json!({"installed":[]}))),
            Some(Settled::Terminal { ok: true })
        ));
        let started = state.starts();
        assert_eq!(
            started.iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![waiter]
        );
        assert_eq!(started[0].attempt, wait_attempt + 1);
        // The scheduled wakeup for the old attempt is stale now.
        assert!(!state.waiting(waiter, wait_attempt));
    }

    #[test]
    fn cancelled_waiting_work_never_restarts_through_its_backoff() {
        let mut state = DownloadsState::default();
        let id = state.submit(job(Some("i1"), "A"));
        state.starts();
        let attempt = state.tasks.iter().find(|t| t.id == id).unwrap().attempt;
        assert!(matches!(
            state.apply_finished(id, attempt, Err(error(LOCK_CONTENTION, None))),
            Some(Settled::Backoff(_))
        ));
        // Cancel while waiting; the scheduled wakeup becomes stale.
        assert!(state.cancel(id));
        assert!(!state.waiting(id, attempt));
        assert!(state.starts().is_empty());
        // A retry during the stale window starts cleanly, not via the old
        // wakeup: the retried task is not in the waiting state either.
        assert!(state.retry(id));
        let started = state.starts();
        assert_eq!(started[0].attempt, attempt + 1);
        assert!(!state.waiting(id, attempt));
    }

    #[test]
    fn a_kept_instance_becomes_the_key_and_retry_resumes_it() {
        let mut state = DownloadsState::default();
        let id = state.submit(job(None, "PACK"));
        state.starts();
        let attempt = state.tasks.iter().find(|t| t.id == id).unwrap().attempt;
        let task = state.tasks.iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.retry_label(), "Waiting to retry.");
        assert_eq!(task.start_label(), "Downloading and verifyingâ€¦");

        assert!(matches!(
            state.apply_finished(
                id,
                attempt,
                Err(error("pack_component_install_failed", Some("abc123")))
            ),
            Some(Settled::Terminal { ok: false })
        ));
        let task = state.tasks.iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.key.as_deref(), Some("abc123"));
        assert_eq!(task.resume.as_deref(), Some("abc123"));
        assert_eq!(task.retry_label(), "Waiting to resume the kept instance.");
        assert_eq!(task.start_label(), "Resuming the kept instanceâ€¦");

        assert!(state.retry(id));
        // Once requeued, the kept key serializes new work on that instance.
        assert!(state.instance_busy("abc123"));
        let started = state.starts();
        assert_eq!(
            started[0].request,
            JobRequest::Resume {
                instance: "abc123".into()
            }
        );
        // A later resume failure without an id keeps resuming the same
        // instance; it never falls back to a fresh install.
        assert!(matches!(
            state.apply_finished(
                id,
                started[0].attempt,
                Err(error("pack_download_failed", None))
            ),
            Some(Settled::Terminal { ok: false })
        ));
        let task = state.tasks.iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.resume.as_deref(), Some("abc123"));
        assert_eq!(
            task.next_request(),
            JobRequest::Resume {
                instance: "abc123".into()
            }
        );
    }

    #[test]
    fn a_failure_before_instance_creation_retries_the_original_install() {
        let mut state = DownloadsState::default();
        let submitted = job(None, "PACK");
        let request = submitted.request.clone();
        let id = state.submit(submitted);
        state.starts();
        let attempt = state.tasks.iter().find(|t| t.id == id).unwrap().attempt;
        assert!(matches!(
            state.apply_finished(id, attempt, Err(error("pack_download_failed", None))),
            Some(Settled::Terminal { ok: false })
        ));
        let task = state.tasks.iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.key, None);
        assert_eq!(task.resume, None);
        assert!(state.retry(id));
        assert_eq!(state.starts()[0].request, request);
    }

    #[test]
    fn cancelling_a_backoff_schedules_its_waiting_successor_without_a_timer() {
        let mut state = DownloadsState::default();
        let first = state.submit(job(Some("same"), "first"));
        let next = state.submit(job(Some("same"), "next"));
        let starts = state.starts();
        assert_eq!(starts.len(), 1);
        let attempt = starts[0].attempt;
        assert!(matches!(
            state.apply_finished(first, attempt, Err(error(LOCK_CONTENTION, None))),
            Some(Settled::Backoff(_))
        ));
        let starts = state.cancel_and_start(first).unwrap();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].id, next);
        assert!(
            !state.waiting(first, attempt),
            "the cancelled wakeup is stale"
        );
        assert!(state.starts().is_empty(), "the successor runs exactly once");
    }

    #[test]
    fn finishing_a_large_concurrent_batch_bounds_history_without_another_submit() {
        let mut state = DownloadsState::default();
        let ids: Vec<_> = (0..MAX_FINISHED + 12)
            .map(|index| state.submit(job(None, &format!("pack-{index}"))))
            .collect();
        let starts = state.starts();
        assert_eq!(starts.len(), ids.len(), "active work is never trimmed");
        for start in starts {
            assert!(matches!(
                state.apply_finished(start.id, start.attempt, Ok(json!({}))),
                Some(Settled::Terminal { ok: true })
            ));
            assert!(
                state
                    .tasks
                    .iter()
                    .filter(|task| matches!(
                        task.phase,
                        Phase::Completed | Phase::Failed | Phase::Cancelled
                    ))
                    .count()
                    <= MAX_FINISHED
            );
        }
        assert_eq!(state.tasks.len(), MAX_FINISHED);
        assert_eq!(state.tasks.last().unwrap().id, *ids.last().unwrap());
        assert!(!state.is_busy());
    }

    #[test]
    fn finished_history_is_bounded_and_active_work_is_kept() {
        let mut state = DownloadsState::default();
        for index in 0..MAX_FINISHED + 5 {
            let id = state.submit(job(None, &format!("P{index}")));
            state.starts();
            assert!(matches!(
                finish(&mut state, id, Ok(json!({"installed":[]}))),
                Some(Settled::Terminal { ok: true })
            ));
        }
        assert_eq!(
            state.tasks.len(),
            MAX_FINISHED,
            "oldest finished entries drop"
        );
        // A submit while work is active trims only finished entries.
        let active = state.submit(job(Some("i1"), "Active"));
        let queued = state.submit(job(Some("i1"), "Queued"));
        state.starts();
        let overflow = state.submit(job(None, "Overflow"));
        assert!(state.tasks.len() <= MAX_FINISHED + 1);
        assert!(state.tasks.iter().any(|t| t.id == active));
        assert!(state.tasks.iter().any(|t| t.id == queued));
        assert!(state.tasks.iter().any(|t| t.id == overflow));
        // Clear keeps only queued and running work.
        assert!(state.clear_finished());
        assert!(
            state
                .tasks
                .iter()
                .all(|t| matches!(t.phase, Phase::Pending | Phase::Running))
        );
        assert!(!state.clear_finished(), "nothing finished left to clear");
    }

    #[test]
    fn busy_markers_report_queued_and_running_work_only() {
        let mut state = DownloadsState::default();
        let a = state.submit(job(Some("i1"), "A"));
        let b = state.submit(job(None, "PACK"));
        state.starts();
        assert!(state.is_busy());
        assert!(state.instance_busy("i1"));
        assert!(!state.instance_busy("i2"));
        assert_eq!(
            state.busy_projects(),
            vec!["A".to_owned(), "PACK".to_owned()]
        );
        assert_eq!(state.busy_instances(), vec!["i1".to_owned()]);
        finish(&mut state, a, Ok(json!({"installed":[]})));
        finish(&mut state, b, Err(error("pack_download_failed", None)));
        assert!(!state.is_busy());
        assert!(state.busy_projects().is_empty());
        assert!(state.busy_instances().is_empty());
    }

    #[test]
    fn a_panic_shaped_failure_settles_honestly() {
        // `backend::guarded` maps a panicked call to this exact error shape.
        let mut state = DownloadsState::default();
        let id = state.submit(job(None, "P"));
        state.starts();
        let task = state.tasks.iter().find(|t| t.id == id).unwrap();
        let attempt = task.attempt;
        let panic = api::ApiError::new(
            "native_operation_failed",
            "The operation stopped unexpectedly. Refresh the launcher state before retrying.",
        );
        assert!(matches!(
            state.apply_finished(id, attempt, Err(panic)),
            Some(Settled::Terminal { ok: false })
        ));
        let task = state.tasks.iter().find(|t| t.id == id).unwrap();
        assert_eq!(task.phase, Phase::Failed);
        assert!(task.message.contains("native_operation_failed"));
        assert!(!state.is_busy(), "the terminal path cleared the busy state");
    }

    #[test]
    fn cancelled_wording_keeps_the_core_instance_facts() {
        let mut cancelled_core = error("installation_cancelled", Some("abc123"));
        cancelled_core.message = "Installation cancelled. The instance 'Pack' was kept.".into();
        let kept = cancelled(cancelled_core);
        assert_eq!(kept.instance_id.as_deref(), Some("abc123"));
        assert_eq!(
            kept.message,
            "Installation cancelled. The instance 'Pack' was kept."
        );
        let other = cancelled(error("pack_instance_failed", Some("abc123")));
        assert_eq!(other.code, "installation_cancelled");
        assert_eq!(other.instance_id.as_deref(), Some("abc123"));
        assert!(other.message.contains("kept"), "{}", other.message);
        assert_eq!(
            other.message,
            "Installation cancelled. The new instance was kept and can be resumed from Downloads or Instances."
        );
        let none = cancelled(error("pack_download_failed", None));
        assert_eq!(none.instance_id, None);
        assert_eq!(
            none.message,
            "Installation cancelled. No uncommitted content was activated."
        );
    }

    #[test]
    fn completion_messages_follow_the_core_result() {
        let merge = json!({"instanceId":"abc","added":3,"alreadyPresent":2,
            "overridesWritten":1,"keptExisting":["config/a.toml"]});
        assert_eq!(
            completion_message(&merge),
            "Added 3 files; 2 already present. Kept 1 of your differing config files. The instance will not follow pack updates."
        );
        let created = json!({"id":"abc","displayName":"Fixture Pack","state":"ready"});
        assert_eq!(
            completion_message(&created),
            "Created and verified the instance 'Fixture Pack'."
        );
        assert_eq!(
            completion_message(&json!({"installed":[]})),
            "Installed and verified."
        );
    }

    #[test]
    fn a_running_row_shows_the_core_phase_or_its_own_stage() {
        // A modpack job reports its own named step, attributed through its
        // content-job control.
        for phase in aurora_launcher_lib::modpacks::progress_phase::ALL {
            let (stage, message) = running_status(JobPhase::Downloading, Some(*phase), "Queued.");
            let (expected, label) = modpack_phase(phase).unwrap();
            assert_eq!(stage, expected, "{phase}");
            assert_eq!(message, format!("{label}â€¦"));
        }
        // Game setup downloads are not reported as pack downloads.
        assert_eq!(
            running_status(JobPhase::Downloading, Some("installingGame"), "").0,
            "Installing game"
        );
        // Downloads verify each file after transfer.
        assert_eq!(
            running_status(JobPhase::Verifying, Some("downloading"), "").0,
            "Verifying"
        );
        // Unknown stages and ordinary jobs show the content-job phase.
        assert_eq!(
            running_status(
                JobPhase::Installing,
                Some("notAPhase"),
                "Downloading and verifyingâ€¦"
            ),
            ("Installing", "Downloading and verifyingâ€¦".to_owned())
        );
        assert_eq!(
            running_status(JobPhase::Installing, None, "Downloading and verifyingâ€¦"),
            ("Installing", "Downloading and verifyingâ€¦".to_owned())
        );
        // Cancellation keeps its wording.
        assert_eq!(
            running_status(
                JobPhase::Cancelling,
                Some("installingGame"),
                "Cancellingâ€¦"
            ),
            ("Cancelling", "Cancellingâ€¦".to_owned())
        );
    }

    #[test]
    fn every_core_modpack_phase_has_its_own_label() {
        let all = aurora_launcher_lib::modpacks::progress_phase::ALL;
        let mut stages = HashSet::new();
        let mut sentences = HashSet::new();
        for phase in all {
            let (stage, sentence) =
                modpack_phase(phase).unwrap_or_else(|| panic!("{phase} has no label"));
            assert!(stages.insert(stage), "{phase}: duplicate stage {stage}");
            assert!(sentences.insert(sentence), "{phase}: duplicate label");
        }
        assert_eq!(
            modpack_phase("installingGame").unwrap().1,
            "Installing Minecraft and the mod loader"
        );
        for retired in ["acquiringComponents", "creatingInstance", "completing"] {
            assert!(modpack_phase(retired).is_none(), "{retired}");
        }
    }

    #[test]
    fn unattributed_progress_events_are_advisory_only() {
        assert!(progress_event(
            "modpack-progress",
            &json!({"phase": "downloading"})
        ));
        // Malformed payloads and unknown names never count as progress.
        assert!(!progress_event("modpack-progress", &json!({"phase": 7})));
        assert!(!progress_event("modpack-progress", &json!({})));
        assert!(!progress_event("modpack-progress", &Value::Null));
        assert!(!progress_event(
            "other-progress",
            &json!({"phase": "downloading"})
        ));
    }

    #[test]
    fn prepared_jobs_carry_their_serialization_key_and_project() {
        let merge = job(Some("i2"), "M");
        assert_eq!(merge.key().as_deref(), Some("i2"));
        assert_eq!(merge.project(), "M");
        let pack = job(None, "PACK");
        assert_eq!(pack.key(), None);
        assert_eq!(pack.project(), "PACK");
        let content = PreparedJob {
            title: "Mod".into(),
            detail: "Into World".into(),
            request: JobRequest::Content {
                target: "i1".into(),
                kind: "mod",
                project: "MOD0001".into(),
                version: "1".into(),
                fingerprint: "fp".into(),
            },
        };
        assert_eq!(content.key().as_deref(), Some("i1"));
        assert_eq!(content.project(), "MOD0001");
        let resume = PreparedJob {
            title: "Pack".into(),
            detail: String::new(),
            request: JobRequest::Resume {
                instance: "kept0001".into(),
            },
        };
        assert_eq!(resume.key().as_deref(), Some("kept0001"));
        assert_eq!(resume.project(), "kept0001");
    }

    #[test]
    fn the_activity_indicator_counts_queued_and_running_work_only() {
        let mut state = DownloadsState::default();
        assert_eq!(state.active_count(), 0);
        assert_eq!(nav_label(state.active_count()), "Downloads");
        assert_eq!(queue_summary(0, 0), "Nothing downloading.");
        let a = state.submit(job(Some("i1"), "A"));
        let b = state.submit(job(Some("i1"), "B"));
        let c = state.submit(job(None, "C"));
        state.starts();
        // Two running, one waiting its turn on the same instance.
        assert_eq!(state.active_count(), 3);
        assert_eq!(nav_label(3), "Downloads (3 active)");
        assert_eq!(queue_summary(2, 1), "2 installing Â· 1 queued");
        // Finished work leaves the count; cancelled work does too.
        finish(&mut state, c, Ok(json!({"installed":[]})));
        assert!(state.cancel(b));
        assert_eq!(state.active_count(), 1);
        finish(&mut state, a, Err(error("pack_download_failed", None)));
        assert_eq!(state.active_count(), 0);
        assert_eq!(nav_label(0), "Downloads");
        // Clearing keeps nothing active to lose, and retry makes it active again.
        assert!(state.retry(a));
        assert_eq!(state.active_count(), 1);
        assert!(state.clear_finished());
        assert_eq!(state.tasks.len(), 1, "queued work survives Clear");
    }

    #[test]
    fn a_settled_task_leaves_the_slint_toast_wording_for_the_parent() {
        let mut state = DownloadsState::default();
        let ok = state.submit(job(None, "OK"));
        let cancelled = state.submit(job(None, "CANCELLED"));
        let failed = state.submit(job(None, "FAILED"));
        state.starts();
        assert_eq!(state.notice, None);
        finish(&mut state, ok, Ok(json!({"installed":[]})));
        assert_eq!(
            state.notice.as_deref(),
            Some("Installation completed and verified.")
        );
        finish(
            &mut state,
            cancelled,
            Err(error("installation_cancelled", None)),
        );
        assert_eq!(state.notice.as_deref(), Some("Installation cancelled."));
        finish(&mut state, failed, Err(error("pack_download_failed", None)));
        assert_eq!(state.notice.as_deref(), Some("message"));
        // A stale reply and a lock-contention backoff settle nothing, so they
        // never replace the notice.
        assert!(state.apply_finished(failed, 99, Ok(json!({}))).is_none());
        assert_eq!(state.notice.as_deref(), Some("message"));
        let waiter = state.submit(job(Some("i1"), "W"));
        state.starts();
        finish(&mut state, waiter, Err(error(LOCK_CONTENTION, None)));
        assert_eq!(state.notice.as_deref(), Some("message"));
    }

    #[test]
    fn the_empty_queue_explains_where_installs_come_from() {
        assert_eq!(
            EMPTY_TEXT,
            "Installs you start from Discover appear here. Several can run at once; installs into the same instance take turns."
        );
    }

    #[test]
    fn the_stage_bar_only_reflects_the_named_stage() {
        assert!(stage_fraction("Downloading") < stage_fraction("Verifying"));
        assert!(stage_fraction("Verifying") < stage_fraction("Installing"));
        assert!(stage_fraction("Installing") < 1.0);
    }
}
