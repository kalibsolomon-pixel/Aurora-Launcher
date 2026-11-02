//! Shared bounded supervision for installer-tool subprocesses.
//!
//! Installer processors (and the managed-Java diagnostic) are downloaded
//! executable code running with the user's own authority: Aurora does not
//! claim an operating-system sandbox. What this module *does* enforce is
//! that no tool subprocess can exhaust the launcher through its own output
//! or outlive its supervision:
//!
//! - stdout and stderr are drained concurrently while the process runs, so
//!   a full pipe can never deadlock the child, and only a bounded tail of
//!   each stream is ever retained in memory (the rest is counted and
//!   discarded, with the truncation recorded);
//! - a wall-clock timeout and an explicit cancellation signal terminate the
//!   whole process tree — a Windows Job Object with
//!   `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` on Windows, a dedicated process
//!   group with a group kill on Linux — followed by a bounded shutdown
//!   grace period in which readers are given a second chance to finish;
//! - the exact spawn status, exit status, timeout, cancellation, and output
//!   truncation facts stay distinguishable instead of collapsing into one
//!   generic error.
//!
//! Dropping the returned future is also safe: the child was spawned with
//! `kill_on_drop`, and on Windows the Job Object guard keeps tree-kill
//! semantics alive until it is dropped with the run.

use std::process::ExitStatus;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::AsyncReadExt;

/// The supervision policy for one subprocess run.
#[derive(Debug, Clone)]
pub struct SupervisorLimits {
    /// The wall-clock bound on the whole run (spawn through exit).
    pub timeout: Duration,
    /// How long readers may keep draining after the child exits or the
    /// tree is terminated before the run gives up on further capture.
    pub shutdown_grace: Duration,
    /// Per-stream cap on the output tail retained for diagnostics.
    pub retained_output_bytes: usize,
}

impl Default for SupervisorLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(600),
            shutdown_grace: Duration::from_secs(10),
            retained_output_bytes: 256 * 1024,
        }
    }
}

/// The cooperative cancellation signal shared with a supervised run.
///
/// Cloning the signal clones the receiver; the paired [`CancelHandle`]
/// fires every clone at once. The default signal (never paired with a
/// handle) always reads as not-cancelled, which is what ordinary
/// installations use.
#[derive(Debug, Clone)]
pub struct CancelSignal {
    receiver: tokio::sync::watch::Receiver<bool>,
}

impl CancelSignal {
    pub fn is_cancelled(&self) -> bool {
        *self.receiver.borrow()
    }

    /// Resolves once the signal fires. A signal whose handle was dropped
    /// without firing never resolves.
    pub async fn cancelled(&mut self) {
        loop {
            if *self.receiver.borrow_and_update() {
                return;
            }
            if self.receiver.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
    }
}

impl Default for CancelSignal {
    fn default() -> Self {
        Self {
            receiver: tokio::sync::watch::channel(false).1,
        }
    }
}

/// The sender side of a [`CancelSignal`].
#[derive(Debug, Clone)]
pub struct CancelHandle {
    sender: tokio::sync::watch::Sender<bool>,
}

impl CancelHandle {
    pub fn cancel(&self) {
        let _ = self.sender.send(true);
    }
}

/// Creates a paired cancellation signal and handle.
pub fn cancellation_channel() -> (CancelHandle, CancelSignal) {
    let (sender, receiver) = tokio::sync::watch::channel(false);
    (CancelHandle { sender }, CancelSignal { receiver })
}

/// What one bounded stream capture kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamCapture {
    /// Total bytes the stream produced (counted even when discarded).
    pub total_bytes: u64,
    /// The retained tail, at most `retained_output_bytes` long.
    pub retained_tail: Vec<u8>,
    /// Whether any bytes had to be discarded to stay within the cap.
    pub truncated: bool,
}

#[derive(Debug, Default)]
struct CaptureSlot {
    capture: StreamCapture,
    finished: bool,
}

impl CaptureSlot {
    fn push(&mut self, chunk: &[u8], cap: usize) {
        self.capture.total_bytes += chunk.len() as u64;
        if self.capture.retained_tail.len() + chunk.len() > cap {
            self.capture.truncated = true;
            let overflow = self.capture.retained_tail.len() + chunk.len() - cap;
            let drop_from_tail = overflow.min(self.capture.retained_tail.len());
            self.capture.retained_tail.drain(..drop_from_tail);
        }
        // A single pipe read can exceed the entire retention budget.
        self.capture
            .retained_tail
            .extend_from_slice(&chunk[chunk.len().saturating_sub(cap)..]);
    }
}

/// How one supervised run ended.
#[derive(Debug)]
pub enum SupervisedExit {
    /// The process was spawned and exited with this status.
    Exited(ExitStatus),
    /// The process was spawned but exceeded the wall-clock bound; the
    /// whole tree was terminated before this was returned.
    TimedOut,
    /// Cancellation fired; the whole tree was terminated before this was
    /// returned.
    Cancelled,
    /// The process could not be spawned (or became unreapable).
    SpawnFailed(std::io::Error),
}

impl SupervisedExit {
    pub fn success(&self) -> bool {
        matches!(self, Self::Exited(status) if status.success())
    }
}

/// The complete record of one supervised run: exit classification plus the
/// bounded captures of both streams (present even on failure, holding
/// whatever was drained before the run ended).
#[derive(Debug)]
pub struct SupervisedRun {
    pub exit: SupervisedExit,
    pub stdout: StreamCapture,
    pub stderr: StreamCapture,
}

/// Runs one command under bounded supervision.
pub async fn run_supervised(
    mut command: tokio::process::Command,
    limits: &SupervisorLimits,
    cancel: &CancelSignal,
) -> SupervisedRun {
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // A dedicated process group lets a timeout kill the tool's
        // descendants, not just the immediate Java child.
        command.as_std_mut().process_group(0);
    }

    let mut supervisor = match command.spawn() {
        Ok(child) => TreeSupervisor::new(child),
        Err(error) => {
            return SupervisedRun {
                exit: SupervisedExit::SpawnFailed(error),
                stdout: StreamCapture::default(),
                stderr: StreamCapture::default(),
            };
        }
    };

    let stdout_slot = Arc::new(Mutex::new(CaptureSlot::default()));
    let stderr_slot = Arc::new(Mutex::new(CaptureSlot::default()));
    let cap = limits.retained_output_bytes;
    spawn_drain(supervisor.child.stdout.take(), stdout_slot.clone(), cap);
    spawn_drain(supervisor.child.stderr.take(), stderr_slot.clone(), cap);

    let mut cancel_watcher = cancel.clone();
    enum RunEvent {
        Exited(std::io::Result<ExitStatus>),
        Cancelled,
        TimedOut,
    }
    let event = tokio::select! {
        status = supervisor.child.wait() => RunEvent::Exited(status),
        _ = cancel_watcher.cancelled() => RunEvent::Cancelled,
        _ = tokio::time::sleep(limits.timeout) => RunEvent::TimedOut,
    };

    let exit = match event {
        RunEvent::Exited(Ok(status)) => {
            // The child exited, but descendants may still hold the output
            // pipes open. Bound the wait; terminate the tree to force EOF
            // if the readers are still running, then wait once more.
            if !readers_settled(&stdout_slot, &stderr_slot, limits.shutdown_grace).await {
                supervisor.terminate();
            }
            let _ = readers_settled(&stdout_slot, &stderr_slot, limits.shutdown_grace).await;
            SupervisedExit::Exited(status)
        }
        RunEvent::Exited(Err(error)) => SupervisedExit::SpawnFailed(error),
        RunEvent::Cancelled => {
            supervisor.terminate();
            let _ = readers_settled(&stdout_slot, &stderr_slot, limits.shutdown_grace).await;
            SupervisedExit::Cancelled
        }
        RunEvent::TimedOut => {
            supervisor.terminate();
            let _ = readers_settled(&stdout_slot, &stderr_slot, limits.shutdown_grace).await;
            SupervisedExit::TimedOut
        }
    };

    SupervisedRun {
        exit,
        stdout: take_capture(&stdout_slot),
        stderr: take_capture(&stderr_slot),
    }
}

/// Owns the child plus the platform tree-termination primitive.
struct TreeSupervisor {
    child: tokio::process::Child,
    #[cfg(windows)]
    job: Option<JobGuard>,
}

impl TreeSupervisor {
    fn new(child: tokio::process::Child) -> Self {
        #[cfg(windows)]
        {
            let job = JobGuard::assign(child.raw_handle());
            Self { child, job }
        }
        #[cfg(not(windows))]
        Self { child }
    }

    /// Terminates the whole process tree: the Job Object on Windows, the
    /// process group on Linux, and the immediate child as the portable
    /// floor. Non-Linux Unix builds can only kill the immediate child
    /// today — a documented limitation, never a reason to skip the kill.
    fn terminate(&mut self) {
        #[cfg(windows)]
        if let Some(job) = self.job.as_ref() {
            job.terminate();
        }
        #[cfg(target_os = "linux")]
        {
            let pid = self.child.id() as i32;
            if pid > 0 {
                // A negative pid addresses the whole process group.
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
            }
        }
        let _ = self.child.start_kill();
    }
}

fn spawn_drain<R>(pipe: Option<R>, slot: Arc<Mutex<CaptureSlot>>, cap: usize)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let Some(mut pipe) = pipe else {
        return;
    };
    tokio::task::spawn(async move {
        let mut buffer = [0u8; 64 * 1024];
        loop {
            match pipe.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if let Ok(mut slot) = slot.lock() {
                        slot.push(&buffer[..read], cap);
                    }
                }
            }
        }
        if let Ok(mut slot) = slot.lock() {
            slot.finished = true;
        }
    });
}

/// Waits (bounded) until both readers have hit EOF, so their captures are
/// complete. Returns false if the grace period elapsed first.
async fn readers_settled(
    first: &Arc<Mutex<CaptureSlot>>,
    second: &Arc<Mutex<CaptureSlot>>,
    grace: Duration,
) -> bool {
    let settled = |first: &Arc<Mutex<CaptureSlot>>, second: &Arc<Mutex<CaptureSlot>>| {
        let Ok(first) = first.lock() else {
            return true;
        };
        let Ok(second) = second.lock() else {
            return true;
        };
        first.finished && second.finished
    };
    if settled(first, second) {
        return true;
    }
    tokio::time::timeout(grace, async {
        while !settled(first, second) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok()
}

fn take_capture(slot: &Arc<Mutex<CaptureSlot>>) -> StreamCapture {
    slot.lock()
        .map(|slot| slot.capture.clone())
        .unwrap_or_default()
}

#[cfg(windows)]
mod job {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    /// A Job Object whose closed handle terminates every process inside
    /// it. The guard must be dropped only when the launcher is prepared
    /// for the supervised tree to die.
    pub struct JobGuard {
        handle: *mut core::ffi::c_void,
    }

    // The handle is owned exclusively and only touched through Win32 calls.
    unsafe impl Send for JobGuard {}
    unsafe impl Sync for JobGuard {}

    impl JobGuard {
        /// Creates a kill-on-close job and assigns the child to it.
        /// `None` means the host refused the job; supervision then degrades
        /// to killing the immediate child only — never to running
        /// unsupervised.
        pub fn assign(child_handle: Option<*mut core::ffi::c_void>) -> Option<Self> {
            let child_handle = child_handle.unwrap_or(std::ptr::null_mut());
            if child_handle.is_null() {
                return None;
            }
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let configured = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of_val(&info) as u32,
                ) != 0
                    && AssignProcessToJobObject(job, child_handle) != 0;
                if !configured {
                    CloseHandle(job);
                    return None;
                }
                Some(Self { handle: job })
            }
        }

        pub fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.handle, 1);
            }
        }
    }

    impl Drop for JobGuard {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(windows)]
use job::JobGuard;

/// Renders one bounded capture into flattened text with an explicit
/// truncation note, character-safe for multibyte UTF-8.
pub fn capture_text(capture: &StreamCapture) -> String {
    let mut text = String::from_utf8_lossy(&capture.retained_tail).into_owned();
    if capture.truncated {
        let seen = capture.total_bytes;
        let kept = capture.retained_tail.len();
        text = format!("[output truncated: {seen} bytes produced, last {kept} bytes kept] {text}");
    }
    text.replace(['\r', '\n'], " ")
}

/// A character-boundary-safe trailing slice of `text` (never panics on
/// multibyte UTF-8, unlike raw byte slicing).
pub fn tail_chars(text: &str, max_chars: usize) -> &str {
    let length = text.chars().count();
    if length <= max_chars {
        return text;
    }
    let skip = length - max_chars;
    let byte_index = text
        .char_indices()
        .nth(skip)
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    &text[byte_index..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_retention_is_bounded_and_counts_everything() {
        let mut slot = CaptureSlot::default();
        let cap = 10;
        slot.push(b"0123456789", cap);
        assert_eq!(slot.capture.retained_tail, b"0123456789");
        assert!(!slot.capture.truncated);
        assert_eq!(slot.capture.total_bytes, 10);
        slot.push(b"abcdef", cap);
        assert_eq!(slot.capture.retained_tail, b"6789abcdef");
        assert!(slot.capture.truncated);
        assert_eq!(slot.capture.total_bytes, 16);
        assert!(slot.capture.retained_tail.len() <= cap);
        slot.push(b"0123456789abcdefghijklmnopqrstuvwxyz", cap);
        assert_eq!(slot.capture.retained_tail, b"qrstuvwxyz");
        slot.push(b"zero", 0);
        assert!(slot.capture.retained_tail.is_empty());
    }

    #[test]
    fn tail_slices_never_split_characters() {
        let text = "αααααααααα";
        let tail = tail_chars(text, 3);
        assert_eq!(tail.chars().count(), 3);
        assert!(text.ends_with(tail));
        assert_eq!(tail_chars(text, 10), text);
        assert_eq!(tail_chars("", 3), "");
    }

    #[test]
    fn default_cancel_signals_never_fire() {
        let signal = CancelSignal::default();
        assert!(!signal.is_cancelled());
        let mut signal_clone = signal.clone();
        // Resolves only through the overall test timeout, proving the
        // dropped-sender case stays pending instead of firing spuriously.
        let outcome = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(Duration::from_millis(50), signal_clone.cancelled()).await
            });
        assert!(outcome.is_err());
    }

    #[tokio::test]
    async fn cancelled_signals_resolve_immediately() {
        let (handle, mut signal) = cancellation_channel();
        handle.cancel();
        assert!(signal.is_cancelled());
        tokio::time::timeout(Duration::from_secs(1), signal.cancelled())
            .await
            .expect("an already-fired signal resolves without waiting");
    }

    #[tokio::test]
    async fn spawn_failures_are_classified_not_panics() {
        let (_handle, signal) = cancellation_channel();
        let limits = SupervisorLimits::default();
        let command = tokio::process::Command::new("definitely-not-a-real-executable-aurora");
        let run = run_supervised(command, &limits, &signal).await;
        assert!(matches!(run.exit, SupervisedExit::SpawnFailed(_)));
        assert_eq!(run.stdout.total_bytes, 0);
        assert_eq!(run.stderr.total_bytes, 0);
    }

    #[tokio::test]
    async fn successful_exits_are_reported_with_captures() {
        let (_handle, signal) = cancellation_channel();
        let limits = SupervisorLimits::default();
        let mut command = tokio::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        let script = if cfg!(windows) {
            "/C echo processor hello"
        } else {
            "-c echo processor hello"
        };
        command.arg(script);
        let run = run_supervised(command, &limits, &signal).await;
        assert!(run.exit.success(), "{:?}", run.exit);
        assert!(capture_text(&run.stdout).contains("processor hello"));
    }

    #[tokio::test]
    async fn wall_clock_timeouts_terminate_running_children() {
        let (_handle, signal) = cancellation_channel();
        let limits = SupervisorLimits {
            timeout: Duration::from_millis(500),
            shutdown_grace: Duration::from_secs(2),
            retained_output_bytes: 1024,
        };
        let mut command = tokio::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        let script = if cfg!(windows) {
            "/C ping -n 30 127.0.0.1 > nul"
        } else {
            "-c sleep 30"
        };
        command.arg(script);
        let started = std::time::Instant::now();
        let run = run_supervised(command, &limits, &signal).await;
        assert!(matches!(run.exit, SupervisedExit::TimedOut));
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "a timeout must not wait for the child's own exit"
        );
    }

    #[tokio::test]
    async fn cancellation_terminates_a_running_child() {
        let (handle, signal) = cancellation_channel();
        let limits = SupervisorLimits::default();
        let mut command = tokio::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
        let script = if cfg!(windows) {
            "/C ping -n 30 127.0.0.1 > nul"
        } else {
            "-c sleep 30"
        };
        command.arg(script);
        let run = tokio::spawn(async move {
            let started = std::time::Instant::now();
            let run = run_supervised(command, &limits, &signal).await;
            (run, started.elapsed())
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        handle.cancel();
        let (run, elapsed) = run.await.unwrap();
        assert!(matches!(run.exit, SupervisedExit::Cancelled));
        assert!(
            elapsed < Duration::from_secs(20),
            "cancellation must not wait for the child's own exit"
        );
    }

    #[tokio::test]
    async fn output_floods_are_bounded_on_both_streams() {
        let (_handle, signal) = cancellation_channel();
        let limits = SupervisorLimits {
            timeout: Duration::from_secs(120),
            shutdown_grace: Duration::from_secs(10),
            retained_output_bytes: 16 * 1024,
        };
        let minimum_flood = 128 * 1024u64;
        let mut command;
        if cfg!(windows) {
            // One PowerShell invocation writes both floods in-process; a
            // cmd `for /l` loop is far too slow and a single giant echo
            // exceeds cmd's command-line limit.
            command = tokio::process::Command::new("powershell");
            command.arg("-NoProfile").arg("-Command").arg(
                "$flood = 'A' * 262144; [Console]::Out.Write($flood); [Console]::Out.Flush(); $more = 'B' * 262144; [Console]::Error.Write($more); [Console]::Error.Flush()",
            );
        } else {
            command = tokio::process::Command::new("sh");
            command.arg("-c 'i=0; while [ $i -lt 64 ]; do head -c 32768 /dev/zero | tr \"\\0\" \"A\"; head -c 32768 /dev/zero | tr \"\\0\" \"B\" >&2; i=$((i+1)); done'");
        }
        let run = run_supervised(command, &limits, &signal).await;
        assert!(run.exit.success(), "{:?}", run.exit);
        assert!(
            run.stdout.total_bytes >= minimum_flood,
            "stdout flood was not produced: {} bytes",
            run.stdout.total_bytes
        );
        assert!(
            run.stderr.total_bytes >= minimum_flood,
            "stderr flood was not produced: {} bytes",
            run.stderr.total_bytes
        );
        assert!(run.stdout.retained_tail.len() <= limits.retained_output_bytes);
        assert!(run.stderr.retained_tail.len() <= limits.retained_output_bytes);
        assert!(run.stdout.truncated && run.stderr.truncated);
        assert!(capture_text(&run.stdout).contains("output truncated"));
        assert!(capture_text(&run.stderr).contains("output truncated"));
    }
}
