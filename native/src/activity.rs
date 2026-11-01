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
    /// Serialization key: the target instance, or None for a new instance.
    key: Option<String>,
    project: String,
    phase: Phase,
    message: String,
    job: Job,
    /// Automatic waits after transient instance-lock contention.
    lock_waits: u8,
    control: Control,
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
                task.message = "Waiting to retry.".into();
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
            task.message = "Downloading and verifying…".into();
            if task.key.is_some() {
                running_keys.push(task.key.clone());
            }
            started.push((task.id, Rc::clone(&task.job), task.control.clone()));
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
                                Err(crate::api::ApiError::new(
                                    "installation_cancelled",
                                    "Installation cancelled. No uncommitted content was activated.",
                                ))
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
            Ok(_) => {
                task.phase = Phase::Completed;
                task.message = "Installed and verified.".into();
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

fn render() {
    let (rows, summary, active) = QUEUE.with(|q| {
        let q = q.borrow();
        let rows: Vec<ActivityRow> = q
            .tasks
            .iter()
            .rev()
            .map(|t| ActivityRow {
                id: t.id as i32,
                title: t.title.clone().into(),
                detail: t.detail.clone().into(),
                phase: (if t.phase == Phase::Running {
                    match t.control.phase() {
                        JobPhase::Queued | JobPhase::Downloading => "Downloading",
                        JobPhase::Verifying => "Verifying",
                        JobPhase::Installing => "Installing",
                        JobPhase::Cancelling => "Cancelling",
                    }
                } else { phase_label(t.phase) }).into(),
                message: t.message.clone().into(),
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
