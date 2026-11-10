//! Downloads and installs started from Discover.
//!
//! - Installs targeting the same instance run one at a time, in order (the
//!   core allows one content mutation per instance). Installs for different
//!   instances, and new modpack instances, run concurrently.
//! - Network concurrency is bounded once, process-wide, by the core's
//!   transfer permits; this queue never opens connections itself.
//! - Cancellation is cooperative through the shared core content-job scope.
//!   Active cache writes settle; commit boundaries finish to their real result.
//! - While anything runs, a 1 s timer shows aggregate transfer counters;
//!   it stops when the queue is idle.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use slint::ComponentHandle;

use crate::api::ApiResult;
use crate::{Activity, ActivityRow, ui};
use aurora_launcher_lib::content_jobs::{Control, Phase as JobPhase};

type Job = Rc<dyn Fn() -> Pin<Box<dyn Future<Output = ApiResult<serde_json::Value>> + Send>>>;

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
    title: String,
    detail: String,
    /// Serialization key: the target instance, or None for a new instance
    /// that does not exist yet.
    key: Option<String>,
    project: String,
    phase: Phase,
    message: String,
    job: Job,
    /// Automatic waits after transient instance-lock contention.
    lock_waits: u8,
    control: Control,
    /// The instance a failed or cancelled new-pack install created and kept.
    /// Retry resumes it instead of installing (and duplicating) again.
    resume: Option<String>,
}

impl Task {
    /// Remembers the kept instance a failed or cancelled run reports. From
    /// then on the task's key is that instance, so it serializes with other
    /// work on it, and Retry resumes it. A failure before any instance was
    /// created reports none and keeps the original job.
    fn remember_kept_instance(&mut self, error: &crate::api::ApiError) {
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
            "Resuming the kept instance…"
        } else {
            "Downloading and verifying…"
        }
    }

    /// The work the next run performs.
    fn next_job(&self) -> Job {
        match &self.resume {
            Some(instance) => resume_job(instance.clone()),
            None => Rc::clone(&self.job),
        }
    }
}

fn resume_job(instance: String) -> Job {
    Rc::new(move || {
        let (host, instance) = (ui::host(), instance.clone());
        Box::pin(async move { crate::api::resume_pack(host, instance).await })
    })
}

/// A run that ended after its cancellation was observed. A kept instance
/// stays named, with the core's own wording when it already explains it.
fn cancelled(error: crate::api::ApiError) -> crate::api::ApiError {
    let message = match &error.instance_id {
        Some(_) if error.code == "installation_cancelled" => error.message,
        Some(_) => "Installation cancelled. The new instance was kept and can be resumed from Downloads or Instances.".into(),
        None => "Installation cancelled. No uncommitted content was activated.".into(),
    };
    crate::api::ApiError {
        code: "installation_cancelled".into(),
        message,
        instance_id: error.instance_id,
    }
}

#[derive(Default)]
struct Queue {
    next: u64,
    tasks: Vec<Task>,
    timer: Option<slint::Timer>,
    baseline: Option<aurora_launcher_lib::transfer::TransferSnapshot>,
}

thread_local! {
    static QUEUE: RefCell<Queue> = RefCell::new(Queue::default());
}

pub fn bind(window: &crate::AppWindow) {
    let activity = window.global::<Activity>();
    activity.on_cancel(|id| {
        QUEUE.with(|q| {
            if let Some(task) =
                q.borrow_mut().tasks.iter_mut().find(|t| {
                    t.id == id as u64 && matches!(t.phase, Phase::Pending | Phase::Running)
                })
            {
                task.control.cancel();
                if task.phase == Phase::Pending {
                    task.phase = Phase::Cancelled;
                    task.message = "Cancelled before it started.".into();
                } else {
                    task.message =
                        "Cancelling… Active writes and any commit in progress must finish safely."
                            .into();
                }
            }
        });
        render();
    });
    activity.on_retry(|id| {
        QUEUE.with(|q| {
            if let Some(task) =
                q.borrow_mut().tasks.iter_mut().find(|t| {
                    t.id == id as u64 && matches!(t.phase, Phase::Failed | Phase::Cancelled)
                })
            {
                task.control = Control::default();
                task.lock_waits = 0;
                task.phase = Phase::Pending;
                task.message = task.retry_label().into();
            }
        });
        pump();
    });
    activity.on_clear(|| {
        QUEUE.with(|q| {
            q.borrow_mut()
                .tasks
                .retain(|t| matches!(t.phase, Phase::Pending | Phase::Running))
        });
        render();
    });
}

/// Queues one install. `job` is re-invoked on retry, so it must rebuild its
/// request from captured values (it never reuses a consumed future).
pub fn submit(
    title: String,
    detail: String,
    key: Option<String>,
    project: String,
    job: impl Fn() -> Pin<Box<dyn Future<Output = ApiResult<serde_json::Value>> + Send>> + 'static,
) {
    QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        q.next += 1;
        let id = q.next;
        q.tasks.push(Task {
            id,
            title,
            detail,
            key,
            project,
            phase: Phase::Pending,
            message: "Queued.".into(),
            job: Rc::new(job),
            lock_waits: 0,
            control: Control::default(),
            resume: None,
        });
        // Keep the panel bounded: drop the oldest finished entries.
        while q.tasks.len() > 30 {
            if let Some(at) = q
                .tasks
                .iter()
                .position(|t| !matches!(t.phase, Phase::Pending | Phase::Running))
            {
                q.tasks.remove(at);
            } else {
                break;
            }
        }
    });
    pump();
}

/// Projects with a queued or running install (shown as busy in results).
pub fn instance_busy(id: &str) -> bool {
    QUEUE.with(|q| {
        q.borrow().tasks.iter().any(|t| {
            t.key.as_deref() == Some(id) && matches!(t.phase, Phase::Pending | Phase::Running)
        })
    })
}

pub fn busy_projects() -> Vec<String> {
    QUEUE.with(|q| {
        q.borrow()
            .tasks
            .iter()
            .filter(|t| matches!(t.phase, Phase::Pending | Phase::Running))
            .map(|t| t.project.clone())
            .collect()
    })
}

/// Starts every pending task whose instance has nothing running.
fn pump() {
    let start: Vec<(u64, Job, Control)> = QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        let mut started = Vec::new();
        let mut running_keys: Vec<Option<String>> = q
            .tasks
            .iter()
            .filter(|t| t.phase == Phase::Running && t.key.is_some())
            .map(|t| t.key.clone())
            .collect();
        for task in q.tasks.iter_mut().filter(|t| t.phase == Phase::Pending) {
            if task.key.is_some() && running_keys.contains(&task.key) {
                task.message = "Waiting for this instance's other install.".into();
                continue;
            }
            task.phase = Phase::Running;
            task.message = task.start_label().into();
            if task.key.is_some() {
                running_keys.push(task.key.clone());
            }
            started.push((task.id, task.next_job(), task.control.clone()));
        }
        started
    });
    for (id, job, control) in start {
        let work = job();
        ui::spawn(
            async move {
                control
                    .run(async {
                        aurora_launcher_lib::content_jobs::checkpoint().map_err(|_| {
                            crate::api::ApiError::new(
                                "installation_cancelled",
                                "Installation cancelled before activation.",
                            )
                        })?;
                        let result = work.await;
                        match result {
                            Err(error)
                                if control.cancellation_observed()
                                    && !error.code.contains("rollback") =>
                            {
                                Err(cancelled(error))
                            }
                            result => result,
                        }
                    })
                    .await
            },
            move |result| finish(id, result),
        );
    }
    sync_timer();
    render();
}

/// Another short operation (such as the post-install inventory refresh)
/// briefly holds the instance's content lock. Such a refusal changed
/// nothing, so the task waits and tries again a bounded number of times.
const LOCK_CONTENTION: &str = "content_operation_in_progress";
const MAX_LOCK_WAITS: u8 = 5;

fn finish(id: u64, result: ApiResult<serde_json::Value>) {
    let waiting = QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        let task = q.tasks.iter_mut().find(|t| t.id == id)?;
        if let Err(error) = &result {
            task.remember_kept_instance(error);
        }
        match &result {
            Err(error) if error.code == "installation_cancelled" => {
                task.phase = Phase::Cancelled;
                task.message = error.message.clone();
                None
            }
            Err(error) if error.code == LOCK_CONTENTION && task.lock_waits < MAX_LOCK_WAITS => {
                task.lock_waits += 1;
                task.phase = Phase::Pending;
                task.message = "Waiting for another change to this instance to finish.".into();
                Some(task.lock_waits)
            }
            Ok(value) => {
                task.phase = Phase::Completed;
                task.message = completion_message(value);
                None
            }
            Err(error) => {
                task.phase = Phase::Failed;
                task.message = format!("{} ({})", error.message, error.code);
                None
            }
        }
    });
    if let Some(attempt) = waiting {
        render();
        slint::Timer::single_shot(
            std::time::Duration::from_millis(800 * u64::from(attempt)),
            pump,
        );
        return;
    }
    match &result {
        Ok(_) => ui::toast("Installation completed and verified.", crate::Tone::Success),
        Err(error) if error.code == "installation_cancelled" => {
            ui::toast("Installation cancelled.", crate::Tone::Neutral)
        }
        Err(error) => ui::error_toast(error),
    }
    crate::discover::after_install();
    pump();
}

/// The truthful outcome of a completed task, from the core's own result:
/// a new (or resumed) pack instance, a merge into an existing instance, or
/// ordinary content.
fn completion_message(value: &serde_json::Value) -> String {
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

fn sync_timer() {
    let running = QUEUE.with(|q| q.borrow().tasks.iter().any(|t| t.phase == Phase::Running));
    QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        match (running, q.timer.is_some()) {
            (true, false) => {
                q.baseline = Some(aurora_launcher_lib::transfer::snapshot());
                let timer = slint::Timer::default();
                timer.start(
                    slint::TimerMode::Repeated,
                    std::time::Duration::from_secs(1),
                    render,
                );
                q.timer = Some(timer);
            }
            (false, true) => {
                q.timer = None;
            }
            _ => {}
        }
    });
}

fn phase_label(phase: Phase) -> &'static str {
    match phase {
        Phase::Pending => "Queued",
        Phase::Running => "Downloading",
        Phase::Completed => "Completed",
        Phase::Failed => "Failed",
        Phase::Cancelled => "Cancelled",
    }
}

/// A running task's row stage and message. A modpack job reports its own
/// named step (attributed through its content-job control, so concurrent
/// packs never share one); other jobs show the content-job phase.
fn running_status(phase: JobPhase, stage: Option<&str>, message: &str) -> (&'static str, String) {
    if phase == JobPhase::Cancelling {
        return ("Cancelling", message.to_owned());
    }
    if let Some((stage, label)) = stage.and_then(crate::discover::modpack_phase) {
        let stage = match (stage, phase) {
            // Downloads verify each file after transfer.
            ("Downloading", JobPhase::Verifying) => "Verifying",
            (stage, _) => stage,
        };
        return (stage, format!("{label}…"));
    }
    let stage = match phase {
        JobPhase::Queued | JobPhase::Downloading => "Downloading",
        JobPhase::Verifying => "Verifying",
        JobPhase::Installing => "Installing",
        JobPhase::Cancelling => "Cancelling",
    };
    (stage, message.to_owned())
}

/// Re-renders the Downloads rows now (for example on a modpack step).
pub fn refresh() {
    render();
}

fn render() {
    let (rows, summary, active) = QUEUE.with(|q| {
        let q = q.borrow();
        let rows: Vec<ActivityRow> = q
            .tasks
            .iter()
            .rev()
            .map(|t| {
                let (phase, message) = if t.phase == Phase::Running {
                    running_status(t.control.phase(), t.control.stage(), &t.message)
                } else {
                    (phase_label(t.phase), t.message.clone())
                };
                (t, phase, message)
            })
            .map(|(t, phase, message)| ActivityRow {
                id: t.id as i32,
                title: t.title.clone().into(),
                detail: t.detail.clone().into(),
                phase: phase.into(),
                message: message.into(),
                tone: match t.phase {
                    Phase::Completed => crate::Tone::Success,
                    Phase::Failed => crate::Tone::Error,
                    Phase::Running => crate::Tone::Working,
                    Phase::Pending => crate::Tone::Neutral,
                    Phase::Cancelled => crate::Tone::Neutral,
                },
                can_cancel: matches!(t.phase, Phase::Pending | Phase::Running) && t.control.phase() != JobPhase::Cancelling,
                can_retry: matches!(t.phase, Phase::Failed | Phase::Cancelled),
                running: t.phase == Phase::Running,
            })
            .collect();
        let running = q.tasks.iter().filter(|t| t.phase == Phase::Running).count();
        let pending = q.tasks.iter().filter(|t| t.phase == Phase::Pending).count();
        let summary = if running + pending == 0 {
            "Nothing downloading.".to_owned()
        } else {
            let now = aurora_launcher_lib::transfer::snapshot();
            let base = q.baseline.unwrap_or_default();
            let mib = now.bytes_downloaded.saturating_sub(base.bytes_downloaded) as f64
                / (1024.0 * 1024.0);
            format!(
                "{running} installing · {pending} queued · {} active transfers · {mib:.1} MB downloaded",
                now.active_transfers
            )
        };
        (rows, summary, running + pending)
    });
    ui::with(|window, _| {
        let activity = window.global::<Activity>();
        activity.set_rows(ui::model(rows));
        activity.set_summary(summary.into());
        activity.set_active(active as i32);
    });
    crate::discover::render_busy();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_pack_task() -> Task {
        Task {
            id: 1,
            title: "Pack".into(),
            detail: "New modpack instance".into(),
            key: None,
            project: "PACK0001".into(),
            phase: Phase::Failed,
            message: String::new(),
            job: Rc::new(|| Box::pin(async { Ok(serde_json::Value::Null) })),
            lock_waits: 0,
            control: Control::default(),
            resume: None,
        }
    }

    fn error(code: &str, instance: Option<&str>) -> crate::api::ApiError {
        let mut error = crate::api::ApiError::new(code, "message");
        error.instance_id = instance.map(str::to_owned);
        error
    }

    #[test]
    fn a_failure_before_instance_creation_retries_the_original_install() {
        let mut task = new_pack_task();
        task.remember_kept_instance(&error("pack_download_failed", None));
        assert_eq!(task.key, None);
        assert_eq!(task.resume, None);
        assert!(Rc::ptr_eq(&task.next_job(), &task.job));
    }

    #[test]
    fn a_kept_instance_becomes_the_key_and_retry_resumes_it() {
        for code in ["pack_component_install_failed", "installation_cancelled"] {
            let mut task = new_pack_task();
            task.remember_kept_instance(&error(code, Some("abc123")));
            assert_eq!(task.key.as_deref(), Some("abc123"), "{code}");
            assert_eq!(task.resume.as_deref(), Some("abc123"), "{code}");
            assert!(!Rc::ptr_eq(&task.next_job(), &task.job), "{code}");
            // A later resume failure without an id keeps resuming the same
            // instance; it never falls back to a fresh install.
            task.remember_kept_instance(&error("pack_download_failed", None));
            assert_eq!(task.resume.as_deref(), Some("abc123"));
            assert_eq!(task.key.as_deref(), Some("abc123"));
        }
    }

    #[test]
    fn observed_cancellation_keeps_the_kept_instance_and_its_wording() {
        let mut core = error("installation_cancelled", Some("abc123"));
        core.message = "Installation cancelled. The instance 'Pack' was kept.".into();
        let kept = cancelled(core);
        assert_eq!(kept.instance_id.as_deref(), Some("abc123"));
        assert_eq!(
            kept.message,
            "Installation cancelled. The instance 'Pack' was kept."
        );
        let other = cancelled(error("pack_instance_failed", Some("abc123")));
        assert_eq!(other.code, "installation_cancelled");
        assert_eq!(other.instance_id.as_deref(), Some("abc123"));
        assert!(other.message.contains("kept"), "{}", other.message);
        let none = cancelled(error("pack_download_failed", None));
        assert_eq!(none.instance_id, None);
        assert_eq!(
            none.message,
            "Installation cancelled. No uncommitted content was activated."
        );
    }

    #[test]
    fn the_core_error_shape_carries_the_kept_instance_id() {
        let parsed: crate::api::ApiError = serde_json::from_value(serde_json::json!({
            "code": "pack_component_install_failed", "message": "m", "instanceId": "abc123"
        }))
        .unwrap();
        assert_eq!(parsed.instance_id.as_deref(), Some("abc123"));
        let plain: crate::api::ApiError =
            serde_json::from_value(serde_json::json!({"code": "c", "message": "m"})).unwrap();
        assert_eq!(plain.instance_id, None);
    }

    #[test]
    fn completion_messages_follow_the_core_result() {
        let merge = serde_json::json!({"instanceId":"abc","added":3,"alreadyPresent":2,
            "overridesWritten":1,"keptExisting":["config/a.toml"]});
        assert_eq!(
            completion_message(&merge),
            "Added 3 files; 2 already present. Kept 1 of your differing config files. The instance will not follow pack updates."
        );
        let created = serde_json::json!({"id":"abc","displayName":"Fixture Pack","state":"ready"});
        assert_eq!(
            completion_message(&created),
            "Created and verified the instance 'Fixture Pack'."
        );
        assert_eq!(
            completion_message(&serde_json::json!({"installed":[]})),
            "Installed and verified."
        );
    }

    #[test]
    fn a_retried_kept_instance_is_labelled_as_resuming() {
        let mut task = new_pack_task();
        assert_eq!(task.retry_label(), "Waiting to retry.");
        assert_eq!(task.start_label(), "Downloading and verifying…");
        task.remember_kept_instance(&error("pack_component_install_failed", Some("abc123")));
        assert_eq!(task.retry_label(), "Waiting to resume the kept instance.");
        assert_eq!(task.start_label(), "Resuming the kept instance…");
    }

    #[test]
    fn a_running_modpack_row_shows_its_own_stage() {
        for phase in aurora_launcher_lib::modpacks::progress_phase::ALL {
            let (stage, message) = running_status(JobPhase::Downloading, Some(*phase), "Queued.");
            let (expected, label) = crate::discover::modpack_phase(phase).unwrap();
            if *phase != "downloading" {
                assert_eq!(stage, expected, "{phase}");
            }
            assert_eq!(message, format!("{label}…"));
        }
        // Game setup downloads are not reported as pack downloads.
        assert_eq!(
            running_status(JobPhase::Downloading, Some("installingGame"), "").0,
            "Installing game"
        );
        assert_eq!(
            running_status(JobPhase::Verifying, Some("downloading"), "").0,
            "Verifying"
        );
        // Cancellation and ordinary content jobs keep their wording.
        assert_eq!(
            running_status(JobPhase::Cancelling, Some("installingGame"), "Cancelling…"),
            ("Cancelling", "Cancelling…".to_owned())
        );
        assert_eq!(
            running_status(JobPhase::Installing, None, "Downloading and verifying…"),
            ("Installing", "Downloading and verifying…".to_owned())
        );
    }
}
