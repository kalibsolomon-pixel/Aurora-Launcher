//! Exact-child process spawning and process-local supervision.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncRead, AsyncReadExt};
use zeroize::Zeroize;
use zeroize::Zeroizing;

use super::resolve::LaunchSpec;
use super::state::LaunchProcessStatus;

const MAX_CAPTURE_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSnapshot {
    pub instance_id: String,
    pub status: LaunchProcessStatus,
    pub process_id: Option<u32>,
    pub started_at_unix_seconds: Option<u64>,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
    pub diagnostic: Option<super::diagnostics::LaunchFailureDiagnostic>,
}

impl ProcessSnapshot {
    pub fn stopped(instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            status: LaunchProcessStatus::Stopped,
            process_id: None,
            started_at_unix_seconds: None,
            exit_code: None,
            message: None,
            diagnostic: None,
        }
    }
}

fn process_states() -> &'static Mutex<HashMap<String, ProcessSnapshot>> {
    static STATES: OnceLock<Mutex<HashMap<String, ProcessSnapshot>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn snapshot(instance_id: &str) -> ProcessSnapshot {
    process_states()
        .lock()
        .expect("launch process state is not poisoned")
        .get(instance_id)
        .cloned()
        .unwrap_or_else(|| ProcessSnapshot::stopped(instance_id))
}

/// Returns every instance currently in `Starting` or `Running` state.
pub fn active_snapshots() -> Vec<ProcessSnapshot> {
    process_states()
        .lock()
        .expect("launch process state is not poisoned")
        .values()
        .filter(|s| s.status.blocks_launch())
        .cloned()
        .collect()
}

/// Prevent spawn/starting transitions while a synchronous content transaction
/// commits. Only process-local exclusion is claimed.
pub(crate) fn lock_stopped(
    instance_id: &str,
) -> Result<std::sync::MutexGuard<'static, HashMap<String, ProcessSnapshot>>, LaunchProcessError> {
    let states = process_states()
        .lock()
        .expect("launch process state is not poisoned");
    if states
        .get(instance_id)
        .is_some_and(|state| state.status.blocks_launch())
    {
        return Err(LaunchProcessError::AlreadyRunning {
            instance_id: instance_id.into(),
        });
    }
    Ok(states)
}

pub type StateListener = Arc<dyn Fn(ProcessSnapshot) + Send + Sync + 'static>;

pub(crate) fn complete_preparation(snapshot: ProcessSnapshot) {
    let mut states = process_states().lock().unwrap();
    if !states
        .get(&snapshot.instance_id)
        .is_some_and(|s| s.status == LaunchProcessStatus::Running)
    {
        states.insert(snapshot.instance_id.clone(), snapshot);
    }
}

#[cfg(test)]
pub(crate) fn with_test_state<T>(
    instance_id: &str,
    status: LaunchProcessStatus,
    operation: impl FnOnce() -> T,
) -> T {
    let mut state = ProcessSnapshot::stopped(instance_id);
    state.status = status;
    process_states()
        .lock()
        .unwrap()
        .insert(instance_id.into(), state);
    let result = operation();
    process_states().lock().unwrap().remove(instance_id);
    result
}

/// Test-only guard that marks one instance as Running for the lifetime of
/// the guard, so async transactions can be exercised against a running
/// process without blocking inside a sync closure.
#[cfg(test)]
pub(crate) struct TestRunningGuard(String);

#[cfg(test)]
impl Drop for TestRunningGuard {
    fn drop(&mut self) {
        process_states().lock().unwrap().remove(&self.0);
    }
}

#[cfg(test)]
pub(crate) fn force_running(instance_id: &str) -> TestRunningGuard {
    let mut state = ProcessSnapshot::stopped(instance_id);
    state.status = LaunchProcessStatus::Running;
    process_states()
        .lock()
        .unwrap()
        .insert(instance_id.into(), state);
    TestRunningGuard(instance_id.to_owned())
}

fn preparations() -> &'static Mutex<HashSet<String>> {
    static PREPARATIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    PREPARATIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn is_preparing(instance_id: &str) -> bool {
    preparations()
        .lock()
        .expect("preparations not poisoned")
        .contains(instance_id)
}

/// A logical per-instance Play reservation, not a filesystem lock. It may
/// survive awaits; dropping on error or cancellation permits another attempt.
pub(crate) struct PreparationGuard(String);

impl PreparationGuard {
    pub(crate) fn acquire(instance_id: &str) -> Result<Self, LaunchProcessError> {
        let mut preparing = preparations().lock().expect("preparations not poisoned");
        if preparing.contains(instance_id) || snapshot(instance_id).status.blocks_launch() {
            return Err(LaunchProcessError::AlreadyRunning {
                instance_id: instance_id.into(),
            });
        }
        let (signal, _) = tokio::sync::watch::channel(false);
        controls().lock().unwrap().insert(
            instance_id.into(),
            StopControl {
                signal,
                pid: None,
                preparation: crate::content_jobs::Control::interrupting_transfers(),
            },
        );
        preparing.insert(instance_id.into());
        Ok(Self(instance_id.into()))
    }
}

impl Drop for PreparationGuard {
    fn drop(&mut self) {
        let mut controls = controls().lock().unwrap();
        if controls.get(&self.0).is_some_and(|c| c.pid.is_none()) {
            controls.remove(&self.0);
        }
        drop(controls);
        preparations()
            .lock()
            .expect("preparations not poisoned")
            .remove(&self.0);
    }
}

/// A stop capability is owned by this supervisor, never recovered from a PID.
#[derive(Clone)]
struct StopControl {
    signal: tokio::sync::watch::Sender<bool>,
    pid: Option<u32>,
    preparation: crate::content_jobs::Control,
}
fn controls() -> &'static Mutex<HashMap<String, StopControl>> {
    static CONTROLS: OnceLock<Mutex<HashMap<String, StopControl>>> = OnceLock::new();
    CONTROLS.get_or_init(Default::default)
}
pub fn can_stop(instance: &str) -> bool {
    controls().lock().unwrap().contains_key(instance)
}
pub(crate) fn preparation_was_cancelled(instance: &str) -> bool {
    controls()
        .lock()
        .unwrap()
        .get(instance)
        .is_some_and(|c| c.pid.is_none() && *c.signal.borrow())
}
/// `None` only cancels preparation. A running stop must name the displayed PID.
pub fn stop(instance: &str, expected_pid: Option<u32>) -> Result<(), &'static str> {
    let controls = controls().lock().unwrap();
    let control = controls
        .get(instance)
        .ok_or("This launcher no longer supervises this instance.")?;
    if control.pid != expected_pid {
        return Err("The game changed. Review its current state before stopping it.");
    }
    control.signal.send_replace(true);
    control.preparation.cancel();
    Ok(())
}
pub(crate) fn preparation_job(instance: &str) -> crate::content_jobs::Control {
    controls()
        .lock()
        .unwrap()
        .get(instance)
        .expect("reserved preparation")
        .preparation
        .clone()
}
pub(crate) fn preparation_signal(instance: &str) -> tokio::sync::watch::Receiver<bool> {
    controls()
        .lock()
        .unwrap()
        .get(instance)
        .expect("reserved preparation")
        .signal
        .subscribe()
}

/// Keep cooperative cleanup alive, and let cancellation win even when a
/// checkpoint returns its error in the same poll as the stop request.
pub(crate) async fn await_preparation<T>(
    work: impl std::future::Future<Output = T>,
    cancelled: &mut tokio::sync::watch::Receiver<bool>,
    on_cancel: impl FnOnce(),
) -> Option<T> {
    tokio::pin!(work);
    tokio::select! {
        biased;
        _ = async {
            while !*cancelled.borrow_and_update() {
                if cancelled.changed().await.is_err() {
                    // Spawn replaces the preparation sender with the running
                    // capability. Closing that channel is not a stop request.
                    std::future::pending::<()>().await;
                }
            }
        } => {
            on_cancel();
            let _ = work.await;
            None
        }
        result = &mut work => {
            if *cancelled.borrow() {
                on_cancel();
                None
            } else {
                Some(result)
            }
        }
    }
}

// A non-kill-on-close job: launcher shutdown releases supervision without killing
// the game. Termination is explicit and only targets the child assigned at spawn.
#[cfg(windows)]
struct GameJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for GameJob {}
#[cfg(windows)]
impl GameJob {
    fn assign(child: &tokio::process::Child) -> Option<Self> {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW},
        };
        unsafe {
            let handle = child.raw_handle()?;
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            if AssignProcessToJobObject(job, handle) == 0 {
                CloseHandle(job);
                return None;
            }
            Some(Self(job))
        }
    }
    fn terminate(&self) -> bool {
        unsafe { windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1) != 0 }
    }
}
#[cfg(windows)]
impl Drop for GameJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn resume_spawned_child(child: &tokio::process::Child) -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                Thread32Next,
            },
            Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };
    let Some(pid) = child.id() else {
        return false;
    };
    // CREATE_SUSPENDED prevents descendants from escaping before job assignment.
    // The retained child handle prevents PID reuse while locating its one thread.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: THREADENTRY32 = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut found = Thread32First(snapshot, &mut entry) != 0;
        let mut resumed = false;
        while found {
            if entry.th32OwnerProcessID == pid {
                let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                if !thread.is_null() {
                    resumed |= ResumeThread(thread) != u32::MAX;
                    CloseHandle(thread);
                }
            }
            found = Thread32Next(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        resumed
    }
}

/// Starts the exact process described by `LaunchSpec`, then supervises the
/// exact returned child handle in the background. No process-name lookup,
/// shell, or reconstructed command line exists anywhere in this boundary.
pub fn spawn_supervised(
    spec: LaunchSpec,
    logs_directory: &Path,
    listener: StateListener,
) -> Result<ProcessSnapshot, LaunchProcessError> {
    spawn_supervised_with_runtime(spec, logs_directory, listener, None)
}

pub fn spawn_supervised_with_runtime(
    spec: LaunchSpec,
    logs_directory: &Path,
    listener: StateListener,
    java_major: Option<u32>,
) -> Result<ProcessSnapshot, LaunchProcessError> {
    spawn_supervised_with_bridge(spec, logs_directory, listener, java_major, None)
}

pub(crate) fn spawn_supervised_with_bridge(
    spec: LaunchSpec,
    logs_directory: &Path,
    listener: StateListener,
    java_major: Option<u32>,
    bridge: Option<super::activity_bridge::Session>,
) -> Result<ProcessSnapshot, LaunchProcessError> {
    let instance_id = spec.instance_id().to_owned();
    let started_at = unix_seconds();
    let starting = ProcessSnapshot {
        instance_id: instance_id.clone(),
        status: LaunchProcessStatus::Starting,
        process_id: None,
        started_at_unix_seconds: Some(started_at),
        exit_code: None,
        message: None,
        diagnostic: None,
    };
    {
        let mut states = process_states()
            .lock()
            .expect("launch process state is not poisoned");
        if states
            .get(&instance_id)
            .is_some_and(|state| state.status.blocks_launch())
        {
            return Err(LaunchProcessError::AlreadyRunning { instance_id });
        }
        states.insert(instance_id.clone(), starting.clone());
    }
    listener(starting);

    let expected_logs = spec.working_directory().join("logs");
    if logs_directory != expected_logs || !logs_directory.starts_with(spec.working_directory()) {
        return fail_before_spawn(
            &instance_id,
            started_at,
            &listener,
            LaunchProcessError::Log(
                "the launch log path is not the exact derived instance logs directory".to_owned(),
            ),
        );
    }
    std::fs::create_dir_all(logs_directory).map_err(|error| {
        let failure = LaunchProcessError::Log(error.to_string());
        record_failure(
            &instance_id,
            started_at,
            None,
            failure.to_string(),
            &listener,
        );
        failure
    })?;
    let log_path = reserve_log_path(logs_directory).map_err(|error| {
        let failure = LaunchProcessError::Log(error.to_string());
        record_failure(
            &instance_id,
            started_at,
            None,
            failure.to_string(),
            &listener,
        );
        failure
    })?;

    let arguments = spec.command_arguments();
    let mut redactions = Zeroizing::new(spec.sensitive_values());
    let mut command = tokio::process::Command::new(spec.java_executable());
    command
        .args(arguments.iter().map(|argument| argument.expose()))
        .current_dir(spec.working_directory())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // A game outlives the launcher window/runtime. Dropping supervision
        // on shutdown must release the handle without terminating that game.
        .kill_on_drop(false)
        // Prevent ambient Java injection while retaining the ordinary host
        // environment graphics/audio/native libraries rely on.
        .env_remove("CLASSPATH")
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS");
    for name in super::activity_bridge::ENVIRONMENT {
        command.env_remove(name);
    }
    if let Some(bridge) = &bridge {
        bridge.inject(&mut command);
        redactions.push(bridge.redaction());
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    // The lock closes the cancellation/spawn race: a cancelled preparation
    // cannot become a running process after Kill returned.
    let mut stop_controls = controls().lock().unwrap();
    if stop_controls
        .get(&instance_id)
        .is_some_and(|c| *c.signal.borrow())
    {
        let mut stopped = ProcessSnapshot::stopped(&instance_id);
        stopped.message = Some("Stopped by you".into());
        process_states()
            .lock()
            .unwrap()
            .insert(instance_id.clone(), stopped.clone());
        drop(stop_controls);
        listener(stopped.clone());
        return Ok(stopped);
    }
    #[cfg(windows)]
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_SUSPENDED);
    let spawned = {
        let _performance = crate::performance::scope(crate::performance::Event::JavaSpawn);
        command.spawn()
    };
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            drop(stop_controls);
            return fail_before_spawn(
                &instance_id,
                started_at,
                &listener,
                LaunchProcessError::Spawn(error.to_string()),
            );
        }
    };
    let process_id = child.id();
    #[cfg(windows)]
    let job = GameJob::assign(&child);
    #[cfg(windows)]
    if !resume_spawned_child(&child) {
        let _ = child.start_kill();
        drop(stop_controls);
        return fail_before_spawn(
            &instance_id,
            started_at,
            &listener,
            LaunchProcessError::Spawn("could not resume the exact spawned process".into()),
        );
    }
    #[cfg(windows)]
    let tree_supervised = job.is_some();
    #[cfg(not(windows))]
    let tree_supervised = cfg!(target_os = "linux");
    let (signal, mut stop_rx) = tokio::sync::watch::channel(false);
    if tree_supervised {
        stop_controls.insert(
            instance_id.clone(),
            StopControl {
                signal,
                pid: process_id,
                preparation: crate::content_jobs::Control::default(),
            },
        );
    } else {
        stop_controls.remove(&instance_id);
    }
    drop(stop_controls);
    let live_log = super::logs::session(&instance_id);
    live_log.clear();
    let running = ProcessSnapshot {
        instance_id: instance_id.clone(),
        status: LaunchProcessStatus::Running,
        process_id,
        started_at_unix_seconds: Some(started_at),
        exit_code: None,
        message: None,
        diagnostic: None,
    };
    process_states()
        .lock()
        .expect("launch process state is not poisoned")
        .insert(instance_id.clone(), running.clone());
    listener(running.clone());
    let receiver = bridge.map(super::activity_bridge::Session::start);
    // Command retains the environment; release it immediately after spawn.
    drop(command);

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    tokio::spawn(async move {
        let secrets = Arc::new(Zeroizing::new(redactions.to_vec()));
        let output_log = live_log.clone();
        let output_secrets = secrets.clone();
        let stdout_task = tokio::spawn(async move {
            match stdout {
                Some(stream) => read_stream(stream, output_log, "stdout", output_secrets).await,
                None => Vec::new(),
            }
        });
        let stderr_task = tokio::spawn(async move {
            match stderr {
                Some(stream) => read_stream(stream, live_log, "stderr", secrets).await,
                None => Vec::new(),
            }
        });
        let mut killed = false;
        let exit = loop {
            tokio::select! {
                biased;
                exit = child.wait() => break exit,
                changed = stop_rx.changed() => {
                    if changed.is_err() { std::future::pending::<()>().await; }
                    if *stop_rx.borrow() {
                        #[cfg(windows)]
                        let terminated = job.as_ref().is_some_and(GameJob::terminate);
                        #[cfg(target_os = "linux")]
                        let terminated = process_id.is_some_and(|pid| unsafe { libc::kill(-(pid as i32), libc::SIGKILL) == 0 });
                        #[cfg(not(any(windows, target_os = "linux")))]
                        let terminated = false;
                        if terminated { killed = true; let _ = child.start_kill(); break child.wait().await; }
                    }
                }
            }
        };
        controls().lock().unwrap().remove(&instance_id);
        // Invalidate identity immediately, before waiting for output drains.
        drop(receiver);
        // A descendant can inherit the pipe. Never keep the session Running
        // forever waiting for EOF after the supervised child has exited.
        let mut stdout_task = stdout_task;
        let mut stderr_task = stderr_task;
        let stdout =
            match tokio::time::timeout(std::time::Duration::from_secs(2), &mut stdout_task).await {
                Ok(r) => r.unwrap_or_default(),
                Err(_) => {
                    stdout_task.abort();
                    Vec::new()
                }
            };
        let stderr =
            match tokio::time::timeout(std::time::Duration::from_secs(2), &mut stderr_task).await {
                Ok(r) => r.unwrap_or_default(),
                Err(_) => {
                    stderr_task.abort();
                    Vec::new()
                }
            };

        let (status, exit_code, mut message) = match exit {
            _ if killed => (
                LaunchProcessStatus::Stopped,
                exit.as_ref().ok().and_then(|s| s.code()),
                Some("Stopped by you".into()),
            ),
            Ok(status) if status.success() => (LaunchProcessStatus::Exited, status.code(), None),
            Ok(status) => (
                LaunchProcessStatus::Failed,
                status.code(),
                Some("Minecraft exited unsuccessfully; review the instance launch log.".to_owned()),
            ),
            Err(_) => (
                LaunchProcessStatus::Failed,
                None,
                Some("Minecraft process supervision failed after start.".to_owned()),
            ),
        };
        let diagnostic = if status == LaunchProcessStatus::Failed {
            let mut output = format!(
                "{}\n{}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
            output = super::logs::redact(&output, &redactions);
            let result = super::diagnostics::classify(&output, java_major);
            message = Some(result.message.clone());
            Some(result)
        } else {
            None
        };
        if let Err(error) = write_redacted_log(&log_path, exit_code, &stdout, &stderr, &redactions)
        {
            eprintln!("[aurora-launcher] could not write the supervised launch log: {error}");
        }
        redactions.zeroize();
        let snapshot = ProcessSnapshot {
            instance_id: instance_id.clone(),
            status,
            process_id: None,
            started_at_unix_seconds: Some(started_at),
            exit_code,
            message,
            diagnostic,
        };
        process_states()
            .lock()
            .expect("launch process state is not poisoned")
            .insert(instance_id, snapshot.clone());
        super::logs::session(&snapshot.instance_id).changed();
        listener(snapshot);
    });

    Ok(running)
}

fn fail_before_spawn<T>(
    instance_id: &str,
    started_at: u64,
    listener: &StateListener,
    error: LaunchProcessError,
) -> Result<T, LaunchProcessError> {
    record_failure(instance_id, started_at, None, error.to_string(), listener);
    Err(error)
}

fn record_failure(
    instance_id: &str,
    started_at: u64,
    exit_code: Option<i32>,
    message: String,
    listener: &StateListener,
) {
    let snapshot = ProcessSnapshot {
        instance_id: instance_id.to_owned(),
        status: LaunchProcessStatus::Failed,
        process_id: None,
        started_at_unix_seconds: Some(started_at),
        exit_code,
        message: Some(message),
        diagnostic: None,
    };
    process_states()
        .lock()
        .expect("launch process state is not poisoned")
        .insert(instance_id.to_owned(), snapshot.clone());
    listener(snapshot);
}

/// Drain continuously, frame complete lines across arbitrary pipe chunks, and
/// discard overlong records whole. Neither idle reads nor notifications poll.
async fn read_stream(
    mut reader: impl AsyncRead + Unpin,
    log: Arc<super::logs::Session>,
    stream: &str,
    secrets: Arc<Zeroizing<Vec<String>>>,
) -> Vec<u8> {
    let mut captured = Vec::new();
    let mut pending = Vec::new();
    let mut overlong = false;
    let mut chunk = [0u8; 8192];
    loop {
        let count = match reader.read(&mut chunk).await {
            Ok(n) => n,
            Err(_) => 0,
        };
        if count == 0 {
            break;
        }
        captured.extend_from_slice(&chunk[..count]);
        if captured.len() > MAX_CAPTURE_BYTES {
            let overflow = captured.len() - MAX_CAPTURE_BYTES;
            captured.drain(MAX_CAPTURE_BYTES / 2..MAX_CAPTURE_BYTES / 2 + overflow);
        }
        for byte in &chunk[..count] {
            if *byte == b'\n' {
                if overlong {
                    log.push("[Overlong log line omitted]", stream, &[]);
                } else {
                    log.push(
                        String::from_utf8_lossy(&pending).trim_end_matches('\r'),
                        stream,
                        &secrets,
                    );
                }
                pending.clear();
                overlong = false;
            } else if !overlong {
                if pending.len() == 64 * 1024 {
                    pending.clear();
                    overlong = true;
                } else {
                    pending.push(*byte);
                }
            }
        }
    }
    if overlong {
        log.push("[Overlong log line omitted]", stream, &[]);
    } else if !pending.is_empty() {
        log.push(&String::from_utf8_lossy(&pending), stream, &secrets);
    }
    captured
}

fn write_redacted_log(
    path: &Path,
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
    redactions: &[String],
) -> std::io::Result<()> {
    let redact = |bytes: &[u8]| super::logs::redact(&String::from_utf8_lossy(bytes), redactions);
    let content = format!(
        "Aurora supervised Minecraft process\nExit code: {}\n\n[stdout]\n{}\n[stderr]\n{}",
        exit_code.map_or_else(|| "unavailable".to_owned(), |code| code.to_string()),
        redact(stdout),
        redact(stderr),
    );
    std::fs::write(path, content)
}

fn reserve_log_path(logs: &Path) -> std::io::Result<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    for _ in 0..32 {
        let path = logs.join(format!(
            "aurora-launch-{}-{}.log",
            unix_seconds(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique launch log",
    ))
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchProcessError {
    AlreadyRunning { instance_id: String },
    Spawn(String),
    Log(String),
}

impl LaunchProcessError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AlreadyRunning { .. } => "launch_already_running",
            Self::Spawn(_) => "launch_spawn_failure",
            Self::Log(_) => "launch_process_failure",
        }
    }
}

impl fmt::Display for LaunchProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning { instance_id } => write!(
                formatter,
                "instance '{instance_id}' is already starting or running"
            ),
            Self::Spawn(_) => write!(formatter, "the managed Java process could not be started"),
            Self::Log(_) => write!(formatter, "the supervised launch log could not be prepared"),
        }
    }
}

impl std::error::Error for LaunchProcessError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn preparation_cancellation_wins_over_a_simultaneous_checkpoint_error() {
        let (signal, mut cancelled) = tokio::sync::watch::channel(false);
        let mut reported_stopped = false;
        let result = await_preparation(
            async {
                signal.send_replace(true);
                Err::<(), _>("download cancelled")
            },
            &mut cancelled,
            || reported_stopped = true,
        )
        .await;
        assert_eq!(result, None);
        assert!(reported_stopped);

        let (signal, mut cancelled) = tokio::sync::watch::channel(true);
        let mut cleanup_finished = false;
        let result =
            await_preparation(async { cleanup_finished = true }, &mut cancelled, || {}).await;
        assert_eq!(result, None);
        assert!(cleanup_finished);
        drop(signal);
    }

    #[tokio::test]
    async fn replacing_the_preparation_channel_does_not_report_a_stop() {
        let (signal, mut cancelled) = tokio::sync::watch::channel(false);
        drop(signal);
        let result = await_preparation(
            async {
                tokio::task::yield_now().await;
                42
            },
            &mut cancelled,
            || panic!("closing the preparation capability is not cancellation"),
        )
        .await;
        assert_eq!(result, Some(42));
    }
    use std::time::Duration;

    #[test]
    #[ignore = "fake long-running game fixture; never Java"]
    fn fake_child_live_tree() {
        use std::io::Write;
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "launch::process::tests::fake_child_descendant",
                "--ignored",
                "--nocapture",
            ])
            .spawn()
            .unwrap();
        std::fs::write("descendant.pid", child.id().to_string()).unwrap();
        println!("INFO live FIXTURE-PROCESS-TOKEN");
        std::io::stdout().flush().unwrap();
        eprintln!("WARN stderr is live");
        std::thread::sleep(Duration::from_secs(30));
    }

    #[test]
    #[ignore = "fake supervised descendant"]
    fn fake_child_descendant() {
        std::thread::sleep(Duration::from_secs(30));
    }

    #[tokio::test]
    async fn live_stream_arrives_before_exit_and_kill_stops_only_its_tree() {
        let root = test_root("live-kill-tree");
        let other_root = test_root("live-other-tree");
        let id = "live-kill-tree";
        let log = super::super::logs::session(id);
        let mut changes = log.subscribe();
        let spec = |id: &str, root: PathBuf| {
            LaunchSpec::fake_process(
                id,
                std::env::current_exe().unwrap(),
                "launch::process::tests::fake_child_live_tree",
                root,
            )
        };
        let running =
            spawn_supervised(spec(id, root.clone()), &root.join("logs"), Arc::new(|_| {})).unwrap();
        let other = spawn_supervised(
            spec("live-other-tree", other_root.clone()),
            &other_root.join("logs"),
            Arc::new(|_| {}),
        )
        .unwrap();
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                changes.borrow_and_update();
                let text = log
                    .snapshot()
                    .lines
                    .iter()
                    .map(|l| l.text.clone())
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.contains("INFO live [redacted]") && text.contains("WARN stderr is live") {
                    break;
                }
                changes.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(snapshot(id).status, LaunchProcessStatus::Running);
        assert!(
            !log.snapshot()
                .lines
                .iter()
                .any(|l| l.text.contains("FIXTURE-PROCESS-TOKEN"))
        );
        assert!(
            stop(id, None).is_err(),
            "running requires the confirmed PID"
        );
        assert!(stop(id, running.process_id.map(|p| p + 1)).is_err());
        #[cfg(windows)]
        let descendant = {
            use std::os::windows::io::{FromRawHandle, OwnedHandle};
            use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};
            let pid = std::fs::read_to_string(root.join("descendant.pid"))
                .unwrap()
                .parse::<u32>()
                .unwrap();
            let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
            assert!(!handle.is_null());
            unsafe { OwnedHandle::from_raw_handle(handle) }
        };
        stop(id, running.process_id).unwrap();
        let stopped = wait_terminal(id).await;
        assert_eq!(stopped.status, LaunchProcessStatus::Stopped);
        assert_eq!(stopped.message.as_deref(), Some("Stopped by you"));
        assert!(stopped.diagnostic.is_none());
        assert!(!can_stop(id));
        assert!(stop(id, running.process_id).is_err());
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::{
                Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject,
            };
            assert_eq!(
                unsafe { WaitForSingleObject(descendant.as_raw_handle(), 5_000) },
                WAIT_OBJECT_0
            );
        }
        assert_eq!(
            snapshot("live-other-tree").status,
            LaunchProcessStatus::Running
        );
        stop("live-other-tree", other.process_id).unwrap();
        assert_eq!(
            wait_terminal("live-other-tree").await.status,
            LaunchProcessStatus::Stopped
        );
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(other_root).unwrap();
    }

    #[tokio::test]
    async fn streaming_frames_split_secrets_and_bounds_unterminated_lines() {
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(64);
        let log = super::super::logs::session("split-secret-stream");
        log.clear();
        let read = tokio::spawn(read_stream(
            reader,
            log.clone(),
            "stdout",
            Arc::new(Zeroizing::new(vec!["secret-token".into()])),
        ));
        writer.write_all(b"INFO secret-").await.unwrap();
        writer.write_all(b"token\n").await.unwrap();
        writer.write_all(&vec![b'x'; 128 * 1024]).await.unwrap();
        writer
            .write_all(b"secret-token\npartial secret-token")
            .await
            .unwrap();
        drop(writer);
        read.await.unwrap();
        let lines = log.snapshot().lines;
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "INFO [redacted]");
        assert_eq!(lines[1].text, "[Overlong log line omitted]");
        assert_eq!(lines[2].text, "partial [redacted]");
    }

    #[tokio::test]
    async fn cancelled_preparation_cannot_spawn_and_unsupervised_state_cannot_kill() {
        let root = test_root("cancel-before-spawn");
        let id = "cancel-before-spawn";
        let guard = PreparationGuard::acquire(id).unwrap();
        let mut signal = preparation_signal(id);
        assert!(can_stop(id));
        assert!(stop(id, Some(42)).is_err());
        stop(id, None).unwrap();
        signal.changed().await.unwrap();
        assert!(*signal.borrow());
        let spec = LaunchSpec::fake_process(
            id,
            std::env::current_exe().unwrap(),
            "launch::process::tests::fake_child_live_tree",
            root.clone(),
        );
        let stopped = spawn_supervised(spec, &root.join("logs"), Arc::new(|_| {})).unwrap();
        assert_eq!(stopped.status, LaunchProcessStatus::Stopped);
        assert!(stopped.process_id.is_none());
        assert!(!root.join("descendant.pid").exists());
        drop(guard);
        assert!(!can_stop(id));
        with_test_state("unsupervised-game", LaunchProcessStatus::Running, || {
            assert!(!can_stop("unsupervised-game"));
            assert!(stop("unsupervised-game", Some(42)).is_err());
        });
        std::fs::remove_dir_all(root).unwrap();
    }

    fn test_root(name: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "aurora-launch-process-{}-{name}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    async fn wait_terminal(instance_id: &str) -> ProcessSnapshot {
        for _ in 0..100 {
            let state = snapshot(instance_id);
            if !state.status.blocks_launch() {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("fake child did not reach a terminal state");
    }

    #[test]
    #[ignore]
    fn fake_child_success() {
        println!("stdout says FIXTURE-PROCESS-TOKEN");
        eprintln!("stderr says FIXTURE-PROCESS-TOKEN");
    }

    #[test]
    #[ignore]
    fn fake_child_failure() {
        eprintln!("expected fake failure");
        std::process::exit(17);
    }

    #[test]
    #[ignore]
    fn fake_child_slow() {
        std::thread::sleep(Duration::from_millis(500));
    }

    #[test]
    #[ignore = "supervised shutdown fixture subprocess"]
    fn fake_child_survives_launcher_shutdown() {
        std::fs::write("child-started", b"started").unwrap();
        std::thread::sleep(Duration::from_millis(600));
        std::fs::write("child-survived", b"finished").unwrap();
    }

    #[test]
    fn launcher_runtime_shutdown_does_not_terminate_the_game() {
        let root = test_root("shutdown-survival");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let spec = LaunchSpec::fake_process(
            "process-shutdown-survival",
            std::env::current_exe().unwrap(),
            "launch::process::tests::fake_child_survives_launcher_shutdown",
            root.clone(),
        );
        let running = runtime.block_on(async {
            spawn_supervised(spec, &root.join("logs"), Arc::new(|_| {})).unwrap()
        });
        // Retain the exact child handle before shutting down supervision.
        // Its success marker can appear before Windows releases its working
        // directory, so observing that file alone is insufficient for cleanup.
        #[cfg(windows)]
        let child_handle = {
            use std::os::windows::io::{FromRawHandle, OwnedHandle};
            use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};
            let handle =
                unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, running.process_id.unwrap()) };
            assert!(
                !handle.is_null(),
                "could not retain the fixture child handle"
            );
            unsafe { OwnedHandle::from_raw_handle(handle) }
        };
        #[cfg(not(windows))]
        let _ = running;
        for _ in 0..100 {
            if root.join("child-started").is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            root.join("child-started").is_file(),
            "fixture did not start"
        );
        runtime.shutdown_timeout(Duration::from_millis(100));
        for _ in 0..200 {
            if root.join("child-survived").is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let survived = root.join("child-survived").is_file();
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::{
                Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject,
            };
            assert_eq!(
                unsafe { WaitForSingleObject(child_handle.as_raw_handle(), 5_000) },
                WAIT_OBJECT_0,
                "fixture child did not exit after launcher shutdown"
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            survived,
            "closing the launcher's runtime terminated its supervised game"
        );
    }

    #[tokio::test]
    async fn long_child_output_keeps_initialization_and_fatal_tail() {
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(8192);
        let task = tokio::spawn(async move {
            writer.write_all(b"initialization\n").await.unwrap();
            writer
                .write_all(&vec![b'x'; MAX_CAPTURE_BYTES * 2])
                .await
                .unwrap();
            writer
                .write_all(b"\nException: late fatal evidence")
                .await
                .unwrap();
        });
        let captured = read_stream(
            reader,
            super::super::logs::session("bounded-initialization-tail"),
            "stdout",
            Arc::new(Zeroizing::new(vec![])),
        )
        .await;
        task.await.unwrap();
        assert_eq!(captured.len(), MAX_CAPTURE_BYTES);
        assert!(captured.starts_with(b"initialization\n"));
        assert!(captured.ends_with(b"Exception: late fatal evidence"));
    }
    #[tokio::test]
    async fn preparing_play_is_reserved_once_and_released_on_cancellation() {
        let id = "preparing-duplicate";
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (_resume_tx, resume_rx) = tokio::sync::oneshot::channel::<()>();
        let first = tokio::spawn(async move {
            let _guard = PreparationGuard::acquire(id).unwrap();
            entered_tx.send(()).unwrap();
            let _ = resume_rx.await;
        });
        entered_rx.await.unwrap();
        assert!(matches!(
            PreparationGuard::acquire(id),
            Err(LaunchProcessError::AlreadyRunning { .. })
        ));
        // A preparation reservation has no fake process state.
        assert_eq!(snapshot(id).status, LaunchProcessStatus::Stopped);
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        assert!(PreparationGuard::acquire(id).is_ok());
    }

    #[tokio::test]
    async fn concurrent_preparations_allow_only_one_supervised_spawn() {
        let root = test_root("preparation-spawn");
        let id = "preparation-single-spawn";
        let (paused_tx, paused_rx) = tokio::sync::oneshot::channel();
        let (resume_tx, resume_rx) = tokio::sync::oneshot::channel();
        let child_root = root.clone();
        let first = tokio::spawn(async move {
            let _guard = PreparationGuard::acquire(id).unwrap();
            paused_tx.send(()).unwrap();
            resume_rx.await.unwrap();
            let spec = LaunchSpec::fake_process(
                id,
                std::env::current_exe().unwrap(),
                "launch::process::tests::fake_child_success",
                child_root.clone(),
            );
            spawn_supervised(spec, &child_root.join("logs"), Arc::new(|_| {})).unwrap()
        });
        paused_rx.await.unwrap();
        assert!(matches!(
            PreparationGuard::acquire(id),
            Err(LaunchProcessError::AlreadyRunning { .. })
        ));
        resume_tx.send(()).unwrap();
        assert_eq!(first.await.unwrap().status, LaunchProcessStatus::Running);
        assert_eq!(wait_terminal(id).await.exit_code, Some(0));
        assert_eq!(std::fs::read_dir(root.join("logs")).unwrap().count(), 1);
        assert!(PreparationGuard::acquire(id).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn supervised_process_transitions_logs_and_redacts() {
        let root = test_root("success");
        let spec = LaunchSpec::fake_process(
            "process-success",
            std::env::current_exe().unwrap(),
            "launch::process::tests::fake_child_success",
            root.clone(),
        );
        let running = spawn_supervised(spec, &root.join("logs"), Arc::new(|_| {})).unwrap();
        assert_eq!(running.status, LaunchProcessStatus::Running);
        assert!(running.process_id.is_some());
        let exited = wait_terminal("process-success").await;
        assert_eq!(exited.status, LaunchProcessStatus::Exited);
        assert_eq!(exited.exit_code, Some(0));
        let log = std::fs::read_to_string(
            std::fs::read_dir(root.join("logs"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap();
        assert!(log.contains("stdout says [redacted]"));
        assert!(log.contains("stderr says [redacted]"));
        assert!(!log.contains("FIXTURE-PROCESS-TOKEN"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn nonzero_spawn_failure_and_duplicate_instance_are_structured() {
        let root = test_root("states");
        let failing = LaunchSpec::fake_process(
            "process-failure",
            std::env::current_exe().unwrap(),
            "launch::process::tests::fake_child_failure",
            root.clone(),
        );
        spawn_supervised(failing, &root.join("logs"), Arc::new(|_| {})).unwrap();
        let failed = wait_terminal("process-failure").await;
        assert_eq!(failed.status, LaunchProcessStatus::Failed);
        assert_eq!(failed.exit_code, Some(17));
        assert_eq!(failed.diagnostic.as_ref().unwrap().category, "unknownCrash");

        let missing = LaunchSpec::fake_process(
            "process-missing",
            root.join("does-not-exist"),
            "unused",
            root.clone(),
        );
        assert!(matches!(
            spawn_supervised(missing, &root.join("logs"), Arc::new(|_| {})),
            Err(LaunchProcessError::Spawn(_))
        ));

        let slow = LaunchSpec::fake_process(
            "process-duplicate",
            std::env::current_exe().unwrap(),
            "launch::process::tests::fake_child_slow",
            root.clone(),
        );
        spawn_supervised(slow.clone(), &root.join("logs"), Arc::new(|_| {})).unwrap();
        assert!(matches!(
            spawn_supervised(slow, &root.join("logs"), Arc::new(|_| {})),
            Err(LaunchProcessError::AlreadyRunning { .. })
        ));

        let other = LaunchSpec::fake_process(
            "process-other",
            std::env::current_exe().unwrap(),
            "launch::process::tests::fake_child_slow",
            root.clone(),
        );
        spawn_supervised(other, &root.join("logs"), Arc::new(|_| {})).unwrap();
        wait_terminal("process-duplicate").await;
        wait_terminal("process-other").await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "supervised fixture subprocess"]
    fn fake_activity_child() {
        use std::io::{Read, Write};
        let env = |name| std::env::var(name).expect("fixture bootstrap");
        let capability = env(super::super::activity_bridge::ENVIRONMENT[2]);
        println!("{capability}");
        eprintln!("{capability}");
        let mut stream =
            std::net::TcpStream::connect(env(super::super::activity_bridge::ENVIRONMENT[0]))
                .unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let id = env(super::super::activity_bridge::ENVIRONMENT[1]);
        let hello = serde_json::json!({"type":"hello","schemaVersion":1,"sessionId":id,"capability":capability});
        writeln!(stream, "{hello}").unwrap();
        let expected = b"{\"type\":\"accepted\",\"schemaVersion\":1}\n";
        let mut ack = vec![0; expected.len()];
        stream.read_exact(&mut ack).unwrap();
        assert_eq!(ack, expected);
        writeln!(stream,"{}",serde_json::json!({"type":"activity","schemaVersion":1,"sessionId":id,"sequence":1,"state":"MAIN_MENU"})).unwrap();
        writeln!(stream,"{}",serde_json::json!({"type":"activity","schemaVersion":1,"sessionId":id,"sequence":2,"state":"SINGLEPLAYER","worldDisplayName":"Fixture World"})).unwrap();
        std::thread::sleep(Duration::from_millis(350));
    }
    #[tokio::test]
    async fn supervised_bridge_exit_and_spawn_failure_release_identity_listener_and_secret() {
        use super::super::activity_bridge::Session;
        let root = test_root("bridge");
        let bridge = Session::prepare().unwrap();
        let handle = bridge.handle();
        let secret = bridge.redaction();
        let spec = LaunchSpec::fake_process(
            "bridge-child",
            std::env::current_exe().unwrap(),
            "launch::process::tests::fake_activity_child",
            root.clone(),
        );
        assert!(!format!("{spec:?}").contains(&secret));
        spawn_supervised_with_bridge(
            spec,
            &root.join("logs"),
            Arc::new(|_| {}),
            Some(21),
            Some(bridge),
        )
        .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while handle.snapshot().is_none() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(wait_terminal("bridge-child").await.exit_code, Some(0));
        assert!(handle.snapshot().is_none());
        let log = std::fs::read_to_string(
            std::fs::read_dir(root.join("logs"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
        )
        .unwrap();
        assert!(!log.contains(&secret));
        assert!(log.contains("[redacted]"));
        let bridge = Session::prepare().unwrap();
        let handle = bridge.handle();
        let mut command = tokio::process::Command::new("unused");
        bridge.inject(&mut command);
        let address = command
            .as_std()
            .get_envs()
            .find(|(key, _)| *key == super::super::activity_bridge::ENVIRONMENT[0])
            .unwrap()
            .1
            .unwrap()
            .to_string_lossy()
            .to_string();
        let spec = LaunchSpec::fake_process(
            "bridge-missing",
            root.join("missing"),
            "unused",
            root.clone(),
        );
        assert!(
            spawn_supervised_with_bridge(
                spec,
                &root.join("logs"),
                Arc::new(|_| {}),
                Some(21),
                Some(bridge)
            )
            .is_err()
        );
        assert!(tokio::net::TcpStream::connect(address).await.is_err());
        assert!(handle.snapshot().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
