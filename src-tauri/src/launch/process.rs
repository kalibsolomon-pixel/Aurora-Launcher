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

fn preparations() -> &'static Mutex<HashSet<String>> {
    static PREPARATIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    PREPARATIONS.get_or_init(|| Mutex::new(HashSet::new()))
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
        preparing.insert(instance_id.into());
        Ok(Self(instance_id.into()))
    }
}

impl Drop for PreparationGuard {
    fn drop(&mut self) {
        preparations()
            .lock()
            .expect("preparations not poisoned")
            .remove(&self.0);
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
        .kill_on_drop(true)
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

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return fail_before_spawn(
                &instance_id,
                started_at,
                &listener,
                LaunchProcessError::Spawn(error.to_string()),
            );
        }
    };
    let process_id = child.id();
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
        let stdout_task = tokio::spawn(async move {
            match stdout {
                Some(stream) => read_bounded(stream).await,
                None => Vec::new(),
            }
        });
        let stderr_task = tokio::spawn(async move {
            match stderr {
                Some(stream) => read_bounded(stream).await,
                None => Vec::new(),
            }
        });
        let exit = child.wait().await;
        // Invalidate identity immediately, before waiting for output drains.
        drop(receiver);
        let stdout = stdout_task.await.unwrap_or_default();
        let stderr = stderr_task.await.unwrap_or_default();

        let (status, exit_code, mut message) = match exit {
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
            for secret in redactions.iter().filter(|s| !s.is_empty()) {
                output = output.replace(secret, "[redacted]");
            }
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

async fn read_bounded(mut reader: impl AsyncRead + Unpin) -> Vec<u8> {
    let mut captured = Vec::new();
    let mut buffer = [0u8; 8 * 1024];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                captured.extend_from_slice(&buffer[..count]);
                if captured.len() > MAX_CAPTURE_BYTES {
                    // Preserve initialization and the fatal tail, draining the
                    // middle while always consuming child output to EOF.
                    let overflow = captured.len() - MAX_CAPTURE_BYTES;
                    captured.drain(MAX_CAPTURE_BYTES / 2..MAX_CAPTURE_BYTES / 2 + overflow);
                }
            }
        }
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
    let redact = |bytes: &[u8]| {
        let mut text = String::from_utf8_lossy(bytes).into_owned();
        for secret in redactions.iter().filter(|secret| !secret.is_empty()) {
            text = text.replace(secret, "[redacted]");
        }
        text
    };
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
    use std::time::Duration;

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
        let captured = read_bounded(reader).await;
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
