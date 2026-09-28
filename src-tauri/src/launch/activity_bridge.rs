//! Client protocol v1. Private, ephemeral, exact-child activity; never game controls.
use serde::Deserialize;
use std::{
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
    time::{Instant, timeout, timeout_at},
};
use zeroize::{Zeroize, Zeroizing};

pub const ENVIRONMENT: [&str; 4] = [
    "AURORA_ACTIVITY_ENDPOINT",
    "AURORA_ACTIVITY_SESSION_ID",
    "AURORA_ACTIVITY_CAPABILITY",
    "AURORA_ACTIVITY_PROTOCOL",
];
const ACCEPTED: &[u8] = b"{\"type\":\"accepted\",\"schemaVersion\":1}\n";
const ACCEPTED_V2: &[u8] = b"{\"type\":\"accepted\",\"schemaVersion\":2}\n";
const FRAME_TIMEOUT: Duration = Duration::from_secs(2);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(120);
/// Independently verified immutable v2.1.3 artifact; bridge classes match reviewed a1f0ab6.
pub const BRIDGE_ARTIFACT_SHA256: &str =
    "4bf78dc1ef8f18e124377575c508ca357327be1c230b203e9f8181bdcb9ebc81";

pub fn supported(active: bool, minecraft: &str, digest: Option<&str>) -> bool {
    active && minecraft == "1.21.11" && digest == Some(BRIDGE_ARTIFACT_SHA256)
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum GameplayState {
    #[serde(rename = "MAIN_MENU")]
    MainMenu,
    #[serde(rename = "SINGLEPLAYER")]
    Singleplayer,
    #[serde(rename = "MULTIPLAYER")]
    Multiplayer,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub state: GameplayState,
    pub world: Option<String>,
    pub server_name: Option<String>,
    pub server_address: Option<String>,
    pub world_save_id: Option<crate::gameplay_history::WorldSaveId>,
    pub server_target: Option<crate::gameplay_history::ServerTarget>,
}
impl fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GameplaySnapshot[private]")
    }
}
struct ActivityState {
    valid: bool,
    snapshot: Option<Snapshot>,
    observer: Option<Arc<dyn Fn(Option<Snapshot>) + Send + Sync>>,
}
#[derive(Clone)]
pub struct ActivityHandle(Arc<Mutex<ActivityState>>);
impl Default for ActivityHandle {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(ActivityState {
            valid: true,
            snapshot: None,
            observer: None,
        })))
    }
}
impl fmt::Debug for ActivityHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ActivityHandle[private]")
    }
}
impl ActivityHandle {
    pub fn set_observer(&self, observer: Arc<dyn Fn(Option<Snapshot>) + Send + Sync>) {
        self.0.lock().unwrap().observer = Some(observer);
    }
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.0.lock().unwrap().snapshot.clone()
    }
    fn replace(&self, snapshot: Option<Snapshot>) {
        let mut current = self.0.lock().unwrap();
        if !current.valid {
            return;
        }
        current.snapshot = snapshot;
        let observer = current.observer.clone();
        let observed = current.snapshot.clone();
        drop(current);
        if let Some(observer) = observer {
            observer(observed);
        }
        crate::discord::gameplay_changed();
    }
    fn invalidate(&self) {
        let mut current = self.0.lock().unwrap();
        current.valid = false;
        current.snapshot = None;
        let observer = current.observer.clone();
        drop(current);
        if let Some(observer) = observer {
            observer(None);
        }
        crate::discord::gameplay_changed();
    }
    #[cfg(test)]
    pub(crate) fn fixture(snapshot: Snapshot) -> Self {
        Self(Arc::new(Mutex::new(ActivityState {
            valid: true,
            snapshot: Some(snapshot),
            observer: None,
        })))
    }
}

/// Not serializable; Debug never exposes bootstrap. Dropped on every pre-spawn error.
pub struct Session {
    listener: TcpListener,
    session: String,
    capability: Zeroizing<String>,
    activity: ActivityHandle,
    protocol: u32,
}
impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ActivitySession[redacted]")
    }
}
impl Session {
    pub fn prepare() -> Result<Self, ()> {
        Self::prepare_for_protocol(1)
    }
    pub fn prepare_for_protocol(protocol: u32) -> Result<Self, ()> {
        if protocol != 1 && protocol != 2 {
            return Err(());
        }
        let mut entropy = [0u8; 32];
        getrandom::fill(&mut entropy).map_err(|_| ())?;
        let capability = Zeroizing::new(entropy.iter().map(|byte| format!("{byte:02x}")).collect());
        entropy.zeroize();
        // No reuse-address/reuse-port; Windows requires explicit exclusivity.
        let socket = tokio::net::TcpSocket::new_v4().map_err(|_| ())?;
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawSocket;
            use windows_sys::Win32::Networking::WinSock::{
                SO_EXCLUSIVEADDRUSE, SOL_SOCKET, setsockopt,
            };
            let enabled = 1i32;
            // SAFETY: live owned socket, valid i32 option buffer for this call.
            if unsafe {
                setsockopt(
                    socket.as_raw_socket() as _,
                    SOL_SOCKET,
                    SO_EXCLUSIVEADDRUSE,
                    (&enabled as *const i32).cast(),
                    std::mem::size_of::<i32>() as i32,
                )
            } != 0
            {
                return Err(());
            }
        }
        socket
            .bind("127.0.0.1:0".parse().map_err(|_| ())?)
            .map_err(|_| ())?;
        let listener = socket.listen(8).map_err(|_| ())?;
        Ok(Self {
            listener,
            session: uuid::Uuid::new_v4().to_string(),
            capability,
            activity: ActivityHandle::default(),
            protocol,
        })
    }
    pub fn handle(&self) -> ActivityHandle {
        self.activity.clone()
    }
    pub fn inject(&self, command: &mut tokio::process::Command) {
        command
            .env(
                ENVIRONMENT[0],
                self.listener
                    .local_addr()
                    .expect("bound listener")
                    .to_string(),
            )
            .env(ENVIRONMENT[1], &self.session)
            .env(ENVIRONMENT[2], self.capability.as_str())
            .env(ENVIRONMENT[3], self.protocol.to_string());
    }
    pub fn redaction(&self) -> String {
        self.capability.to_string()
    }
    pub fn start(self) -> Receiver {
        let activity = self.handle();
        let task = tokio::spawn(async move {
            // This guard clears on EOF, errors, cancellation and task unwinding.
            let _clear = Clear(self.handle());
            self.receive().await;
        });
        Receiver { task, activity }
    }
    async fn receive(mut self) {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        for _ in 0..8 {
            let Ok(Ok((mut stream, peer))) = timeout_at(deadline, self.listener.accept()).await
            else {
                return;
            };
            if peer.ip() != std::net::Ipv4Addr::LOCALHOST {
                continue;
            }
            let authenticated = timeout_at(deadline.min(Instant::now() + FRAME_TIMEOUT), async {
                let bytes = frame(&mut stream, 1024, false).await?;
                let hello: Hello = serde_json::from_slice(&bytes).map_err(|_| ())?;
                if hello.kind != "hello"
                    || hello.schema_version != self.protocol
                    || hello.session_id != self.session
                    || hello.capability.len() != 64
                    || !bool::from(
                        hello
                            .capability
                            .as_bytes()
                            .ct_eq(self.capability.as_bytes()),
                    )
                {
                    return Err(());
                }
                stream
                    .write_all(if self.protocol == 2 {
                        ACCEPTED_V2
                    } else {
                        ACCEPTED
                    })
                    .await
                    .map_err(|_| ())
            })
            .await;
            if !matches!(authenticated, Ok(Ok(()))) {
                continue;
            }
            // Authentication is single-use: no more accepts, no receiver writes.
            drop(self.listener);
            self.capability.zeroize();
            let mut sequence = 0;
            loop {
                let Ok(bytes) = frame(&mut stream, 4096, true).await else {
                    return;
                };
                let Ok(snapshot) =
                    parse_activity_version(&bytes, &self.session, &mut sequence, self.protocol)
                else {
                    return;
                };
                self.activity.replace(Some(snapshot));
            }
        }
    }
}
struct Clear(ActivityHandle);
impl Drop for Clear {
    fn drop(&mut self) {
        self.0.invalidate();
    }
}
pub struct Receiver {
    task: JoinHandle<()>,
    activity: ActivityHandle,
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.activity.invalidate();
        self.task.abort();
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Hello {
    #[serde(rename = "type")]
    kind: String,
    schema_version: u32,
    session_id: String,
    capability: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireActivity {
    #[serde(rename = "type")]
    kind: String,
    schema_version: u32,
    session_id: String,
    sequence: i64,
    state: GameplayState,
    #[serde(default, deserialize_with = "present_string")]
    world_display_name: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    server_display_name: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    server_address: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    world_save_id: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    server_target: Option<String>,
}
fn present_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}
#[cfg(test)]
fn parse_activity(bytes: &[u8], session: &str, previous: &mut i64) -> Result<Snapshot, ()> {
    parse_activity_version(bytes, session, previous, 1)
}
fn parse_activity_version(
    bytes: &[u8],
    session: &str,
    previous: &mut i64,
    protocol: u32,
) -> Result<Snapshot, ()> {
    let wire: WireActivity = serde_json::from_slice(bytes).map_err(|_| ())?;
    if wire.kind != "activity"
        || wire.schema_version != protocol
        || wire.session_id != session
        || wire.sequence <= *previous
        || (*previous == 0 && (wire.sequence != 1 || wire.state != GameplayState::MainMenu))
    {
        return Err(());
    }
    if (wire.state != GameplayState::Singleplayer
        && (wire.world_display_name.is_some() || wire.world_save_id.is_some()))
        || (wire.state != GameplayState::Multiplayer
            && (wire.server_display_name.is_some()
                || wire.server_address.is_some()
                || wire.server_target.is_some()))
        || (protocol == 1 && (wire.world_save_id.is_some() || wire.server_target.is_some()))
    {
        return Err(());
    }
    let world = validate_text(wire.world_display_name, 128)?;
    let server_name = validate_text(wire.server_display_name, 128)?;
    let server_address = validate_text(wire.server_address, 255)?;
    let world_save_id = wire
        .world_save_id
        .map(crate::gameplay_history::WorldSaveId::parse)
        .transpose()
        .map_err(|_| ())?;
    let server_target = wire
        .server_target
        .map(crate::gameplay_history::ServerTarget::parse)
        .transpose()
        .map_err(|_| ())?;
    *previous = wire.sequence;
    Ok(Snapshot {
        state: wire.state,
        world,
        server_name,
        server_address,
        world_save_id,
        server_target,
    })
}
fn validate_text(text: Option<String>, limit: usize) -> Result<Option<String>, ()> {
    text.map(|text| {
        if text.is_empty() || text.chars().count() > limit || sanitize(&text) != text {
            Err(())
        } else {
            Ok(text)
        }
    })
    .transpose()
}
/// C0/C1, Unicode FORMAT and line separators, matching client sanitization.
pub fn sanitize(text: &str) -> String {
    text.chars().filter(|c| !c.is_control() && !matches!(*c as u32,
        0x00ad | 0x0600..=0x0605 | 0x061c | 0x06dd | 0x070f | 0x0890..=0x0891 | 0x08e2 | 0x180e |
        0x200b..=0x200f | 0x2028..=0x202e | 0x2060..=0x2064 | 0x2066..=0x206f | 0xfeff | 0xfff9..=0xfffb |
        0x110bd | 0x110cd | 0x13430..=0x1343f | 0x1bca0..=0x1bca3 | 0x1d173..=0x1d17a | 0xe0001 | 0xe0020..=0xe007f
    )).take(255).collect::<String>().trim().to_owned()
}
async fn frame(stream: &mut TcpStream, limit: usize, idle_allowed: bool) -> Result<Vec<u8>, ()> {
    let mut first = [0];
    if idle_allowed {
        stream.read_exact(&mut first).await.map_err(|_| ())?;
    }
    let read = async {
        let mut bytes = Vec::with_capacity(limit);
        if idle_allowed {
            bytes.push(first[0]);
        }
        loop {
            if bytes.last() == Some(&b'\n') {
                bytes.pop();
                if bytes.contains(&b'\r') || bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
                    return Err(());
                }
                std::str::from_utf8(&bytes).map_err(|_| ())?;
                return Ok(bytes);
            }
            if bytes.len() >= limit {
                return Err(());
            }
            let mut byte = [0];
            stream.read_exact(&mut byte).await.map_err(|_| ())?;
            bytes.push(byte[0]);
        }
    };
    timeout(FRAME_TIMEOUT, read).await.map_err(|_| ())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Run only with an explicit external Java 21 runtime, client test classpath,
    /// and disposable run root. The client source is not imported into this repo.
    #[tokio::test]
    #[ignore = "requires explicit external client test classpath and Java 21"]
    async fn compiled_java_v2_worker_interoperates_with_rust_receiver() {
        use crate::launch::{process, resolve::LaunchSpec};
        let java =
            std::path::PathBuf::from(std::env::var("AURORA_TEST_JAVA").expect("explicit Java"));
        let classpath =
            std::env::var("AURORA_TEST_CLIENT_CLASSPATH").expect("explicit client classpath");
        let root = std::path::PathBuf::from(
            std::env::var("AURORA_TEST_RUN_ROOT").expect("explicit disposable root"),
        );
        std::fs::create_dir_all(&root).unwrap();
        let spec = LaunchSpec::fixture_command(
            java,
            root.clone(),
            vec![
                "-cp".into(),
                classpath,
                "com.aurora.client.launcher.LauncherHistoryContractHarness".into(),
            ],
        );
        let session = Session::prepare_for_protocol(2).unwrap();
        let handle = session.handle();
        let running = process::spawn_supervised_with_bridge(
            spec,
            &root.join("logs"),
            Arc::new(|_| {}),
            Some(21),
            Some(session),
        )
        .unwrap();
        let mut saw_world = false;
        let mut saw_server = false;
        timeout(Duration::from_secs(15), async {
            loop {
                if let Some(snapshot) = handle.snapshot() {
                    if snapshot
                        .world_save_id
                        .as_ref()
                        .is_some_and(|id| id.as_str() == "stable-save")
                    {
                        saw_world = true;
                    }
                    if snapshot
                        .server_target
                        .as_ref()
                        .is_some_and(|target| target.as_str() == "example.invalid:25565")
                    {
                        saw_server = true;
                    }
                }
                if saw_world && saw_server {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("Java v2 transitions");
        assert!(running.status.blocks_launch());
        timeout(Duration::from_secs(15), async {
            while process::snapshot(&running.instance_id)
                .status
                .blocks_launch()
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("Java harness exit");
        assert_eq!(
            process::snapshot(&running.instance_id).status,
            crate::launch::state::LaunchProcessStatus::Exited
        );
    }

    #[tokio::test]
    async fn v2_authenticated_loopback_accepts_validated_identity() {
        let session = Session::prepare_for_protocol(2).unwrap();
        let addr = session.listener.local_addr().unwrap();
        let id = session.session.clone();
        let mut hello = serde_json::to_vec(&json!({"type":"hello","schemaVersion":2,"sessionId":id,"capability":session.capability.as_str()})).unwrap();
        hello.push(b'\n');
        let handle = session.handle();
        let receiver = session.start();
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(&hello).await.unwrap();
        let mut ack = vec![0; ACCEPTED_V2.len()];
        stream.read_exact(&mut ack).await.unwrap();
        assert_eq!(ack, ACCEPTED_V2);
        send(&mut stream, json!({"type":"activity","schemaVersion":2,"sessionId":id,"sequence":1,"state":"MAIN_MENU"})).await;
        send(&mut stream, json!({"type":"activity","schemaVersion":2,"sessionId":id,"sequence":2,"state":"SINGLEPLAYER","worldDisplayName":"Title","worldSaveId":"stable-save"})).await;
        wait_state(&handle, true).await;
        tokio::time::timeout(Duration::from_secs(2), async {
            while handle
                .snapshot()
                .and_then(|snapshot| snapshot.world_save_id)
                .is_none()
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            handle.snapshot().unwrap().world_save_id.unwrap().as_str(),
            "stable-save"
        );
        drop(receiver);
    }

    #[test]
    fn v2_identity_requires_negotiation_and_native_validation() {
        let id = "4b609826-a9a8-4fa6-a7d4-57e7f373900e";
        let mut sequence = 0;
        let menu = json!({"type":"activity","schemaVersion":2,"sessionId":id,"sequence":1,"state":"MAIN_MENU"});
        assert!(
            parse_activity_version(&serde_json::to_vec(&menu).unwrap(), id, &mut sequence, 2)
                .is_ok()
        );
        let world = json!({"type":"activity","schemaVersion":2,"sessionId":id,"sequence":2,"state":"SINGLEPLAYER","worldDisplayName":"Title","worldSaveId":"stable-save"});
        let parsed =
            parse_activity_version(&serde_json::to_vec(&world).unwrap(), id, &mut sequence, 2)
                .unwrap();
        assert_eq!(parsed.world_save_id.unwrap().as_str(), "stable-save");
        assert_eq!(parsed.world.as_deref(), Some("Title"));
        let mut old_sequence = 1;
        assert!(
            parse_activity_version(
                &serde_json::to_vec(&world).unwrap(),
                id,
                &mut old_sequence,
                1
            )
            .is_err()
        );
        let mut bad = world.clone();
        bad["sequence"] = json!(3);
        bad["worldSaveId"] = json!("../escape");
        assert!(
            parse_activity_version(&serde_json::to_vec(&bad).unwrap(), id, &mut sequence, 2)
                .is_err()
        );
        assert_eq!(sequence, 2);
    }

    fn hello(session: &Session) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&json!({"type":"hello","schemaVersion":1,"sessionId":session.session,"capability":session.capability.as_str()})).unwrap();
        bytes.push(b'\n');
        bytes
    }
    fn activity(session: &str, sequence: i64, state: &str) -> serde_json::Value {
        json!({"type":"activity","schemaVersion":1,"sessionId":session,"sequence":sequence,"state":state})
    }
    async fn wait_state(handle: &ActivityHandle, present: bool) {
        timeout(Duration::from_secs(3), async {
            while handle.snapshot().is_some() != present {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("receiver state deadline");
    }
    async fn connected() -> (TcpStream, Receiver, ActivityHandle, String) {
        let session = Session::prepare().unwrap();
        let addr = session.listener.local_addr().unwrap();
        let bytes = hello(&session);
        let id = session.session.clone();
        let handle = session.handle();
        let receiver = session.start();
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(&bytes).await.unwrap();
        let mut ack = vec![0; ACCEPTED.len()];
        stream.read_exact(&mut ack).await.unwrap();
        assert_eq!(ack, ACCEPTED);
        (stream, receiver, handle, id)
    }
    async fn send(stream: &mut TcpStream, value: serde_json::Value) {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        stream.write_all(&bytes).await.unwrap();
    }
    #[tokio::test]
    async fn bootstrap_is_fresh_exclusive_loopback_and_redacted() {
        let a = Session::prepare().unwrap();
        let b = Session::prepare().unwrap();
        let addr = a.listener.local_addr().unwrap();
        assert_eq!(addr.ip(), std::net::Ipv4Addr::LOCALHOST);
        assert_ne!(addr.port(), 0);
        assert!(std::net::TcpListener::bind(addr).is_err());
        assert!(a.capability != b.capability);
        assert_ne!(a.session, b.session);
        assert_eq!(a.capability.len(), 64);
        assert!(
            a.capability
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_eq!(
            uuid::Uuid::parse_str(&a.session).unwrap().to_string(),
            a.session
        );
        assert!(!format!("{a:?}").contains(a.capability.as_str()));
        let mut command = tokio::process::Command::new("unused");
        a.inject(&mut command);
        let env: std::collections::HashMap<_, _> = command
            .as_std()
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().to_string(),
                    v.unwrap().to_string_lossy().to_string(),
                )
            })
            .collect();
        assert_eq!(env.len(), 4);
        assert_eq!(env[ENVIRONMENT[0]], addr.to_string());
        assert_eq!(env[ENVIRONMENT[1]], a.session);
        assert!(env[ENVIRONMENT[2]] == *a.capability);
        assert_eq!(env[ENVIRONMENT[3]], "1");
    }
    #[tokio::test]
    async fn bad_hello_does_not_steal_valid_child_and_attempts_are_bounded() {
        let session = Session::prepare().unwrap();
        let addr = session.listener.local_addr().unwrap();
        let valid = hello(&session);
        let mut base: serde_json::Value = serde_json::from_slice(&valid).unwrap();
        let mut frames = vec![b"invalid\n".to_vec(), vec![0xff, b'\n'], vec![b'x'; 1025]];
        for (key, value) in [
            ("capability", json!("0".repeat(64))),
            ("sessionId", json!(uuid::Uuid::new_v4().to_string())),
            ("schemaVersion", json!(2)),
            ("type", json!("activity")),
        ] {
            let mut wrong = base.clone();
            wrong[key] = value;
            let mut bytes = serde_json::to_vec(&wrong).unwrap();
            bytes.push(b'\n');
            frames.push(bytes);
        }
        let receiver = session.start();
        for bytes in frames {
            let mut stream = TcpStream::connect(addr).await.unwrap();
            stream.write_all(&bytes).await.unwrap();
            let mut byte = [0];
            assert!(!matches!(
                timeout(Duration::from_secs(3), stream.read(&mut byte)).await,
                Ok(Ok(1))
            ));
        }
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(&valid).await.unwrap();
        let mut ack = vec![0; ACCEPTED.len()];
        stream.read_exact(&mut ack).await.unwrap();
        assert_eq!(ack, ACCEPTED);
        drop(receiver);
        // Eight rejections close the listener rather than an indefinite oracle.
        let session = Session::prepare().unwrap();
        let addr = session.listener.local_addr().unwrap();
        let receiver = session.start();
        base["schemaVersion"] = json!(99);
        let mut bytes = serde_json::to_vec(&base).unwrap();
        bytes.push(b'\n');
        for _ in 0..8 {
            let mut s = TcpStream::connect(addr).await.unwrap();
            s.write_all(&bytes).await.unwrap();
            let mut b = [0];
            let _ = s.read(&mut b).await;
        }
        assert!(TcpStream::connect(addr).await.is_err());
        drop(receiver);
    }
    #[test]
    fn strict_activity_contract_sequence_gaps_and_whole_snapshots() {
        let id = "4b609826-a9a8-4fa6-a7d4-57e7f373900e";
        let mut seq = 0;
        assert!(
            parse_activity(
                &serde_json::to_vec(&activity(id, 2, "MAIN_MENU")).unwrap(),
                id,
                &mut seq
            )
            .is_err()
        );
        assert!(
            parse_activity(
                &serde_json::to_vec(&activity(id, 1, "SINGLEPLAYER")).unwrap(),
                id,
                &mut seq
            )
            .is_err()
        );
        let menu = parse_activity(
            &serde_json::to_vec(&activity(id, 1, "MAIN_MENU")).unwrap(),
            id,
            &mut seq,
        )
        .unwrap();
        assert!(menu.world.is_none());
        // Exact non-sensitive example from the client protocol document.
        let server=br#"{"type":"activity","schemaVersion":1,"sessionId":"4b609826-a9a8-4fa6-a7d4-57e7f373900e","sequence":2,"state":"MULTIPLAYER","serverDisplayName":"Fixture SMP","serverAddress":"example.invalid"}"#;
        let snapshot = parse_activity(server, id, &mut seq).unwrap();
        assert_eq!(snapshot.server_name.as_deref(), Some("Fixture SMP"));
        for number in [1, 2] {
            assert!(
                parse_activity(
                    &serde_json::to_vec(&activity(id, number, "MAIN_MENU")).unwrap(),
                    id,
                    &mut seq
                )
                .is_err()
            );
        }
        let snapshot = parse_activity(
            &serde_json::to_vec(&activity(id, 4, "MAIN_MENU")).unwrap(),
            id,
            &mut seq,
        )
        .unwrap();
        assert!(
            snapshot.world.is_none()
                && snapshot.server_name.is_none()
                && snapshot.server_address.is_none()
        );
        let mut world = activity(id, 5, "SINGLEPLAYER");
        world["worldDisplayName"] = json!("Fixture World");
        let snapshot = parse_activity(&serde_json::to_vec(&world).unwrap(), id, &mut seq).unwrap();
        assert!(snapshot.server_name.is_none());
        assert!(parse_activity(br#"{"type":"activity","type":"activity"}"#, id, &mut seq).is_err());
    }
    #[test]
    fn rejects_unknown_null_inconsistent_unsafe_and_overlong_activity() {
        let id = "fixture";
        for (key, value) in [
            ("type", json!("hello")),
            ("schemaVersion", json!(2)),
            ("sessionId", json!("other")),
            ("sequence", json!(0)),
            ("sequence", json!(1.5)),
            ("sequence", json!(9223372036854775808u64)),
            ("state", json!("REALMS")),
            ("worldDisplayName", json!("world")),
            ("serverAddress", json!("example.invalid")),
            ("extra", json!(true)),
        ] {
            let mut wire = activity(id, 1, "MAIN_MENU");
            wire[key] = value;
            assert!(
                parse_activity(&serde_json::to_vec(&wire).unwrap(), id, &mut 0).is_err(),
                "invalid field {key}"
            );
        }
        for value in [
            json!(null),
            json!(""),
            json!("x".repeat(129)),
            json!(" unsafe "),
            json!("hidden\u{200b}name"),
            json!("bad\nname"),
        ] {
            let mut wire = activity(id, 2, "SINGLEPLAYER");
            wire["worldDisplayName"] = value;
            assert!(parse_activity(&serde_json::to_vec(&wire).unwrap(), id, &mut 1).is_err());
        }
        let mut wire = activity(id, 2, "MULTIPLAYER");
        wire["serverAddress"] = json!("x".repeat(256));
        assert!(parse_activity(&serde_json::to_vec(&wire).unwrap(), id, &mut 1).is_err());
    }
    #[tokio::test]
    async fn eof_protocol_failure_and_cancel_clear_without_resurrection() {
        for failure in 0..4 {
            let (mut stream, receiver, handle, id) = connected().await;
            send(&mut stream, activity(&id, 1, "MAIN_MENU")).await;
            wait_state(&handle, true).await;
            let mut world = activity(&id, 2, "SINGLEPLAYER");
            world["worldDisplayName"] = json!("Fixture World");
            send(&mut stream, world).await;
            match failure {
                0 => {
                    drop(stream);
                    wait_state(&handle, false).await;
                    drop(receiver);
                }
                1 => {
                    send(&mut stream, activity(&id, 1, "MAIN_MENU")).await;
                    wait_state(&handle, false).await;
                    drop(receiver);
                }
                2 => {
                    stream.write_all(&vec![b'x'; 4097]).await.unwrap();
                    wait_state(&handle, false).await;
                    drop(receiver);
                }
                _ => {
                    drop(receiver);
                    assert!(handle.snapshot().is_none());
                }
            }
            handle.replace(Some(Snapshot {
                state: GameplayState::Singleplayer,
                world: Some("late".into()),
                server_name: None,
                server_address: None,
                world_save_id: None,
                server_target: None,
            }));
            assert!(handle.snapshot().is_none());
            let replacement = ActivityHandle::default();
            assert!(replacement.snapshot().is_none());
        }
    }
    #[tokio::test]
    async fn total_frame_deadline_and_no_crlf_bom_or_oversize() {
        for bytes in [
            b"{}\r\n".to_vec(),
            vec![0xef, 0xbb, 0xbf, b'{', b'}', b'\n'],
            vec![0xff, b'\n'],
            vec![b'x'; 1025],
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let mut client = TcpStream::connect(listener.local_addr().unwrap())
                .await
                .unwrap();
            let (mut server, _) = listener.accept().await.unwrap();
            client.write_all(&bytes).await.unwrap();
            assert!(frame(&mut server, 1024, false).await.is_err());
        }
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (mut server, _) = listener.accept().await.unwrap();
        let started = Instant::now();
        let read = tokio::spawn(async move { frame(&mut server, 1024, false).await });
        for _ in 0..5 {
            let _ = client.write_all(b"x").await;
            tokio::time::sleep(Duration::from_millis(450)).await;
        }
        assert!(read.await.unwrap().is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn artifact_support_uses_verified_identity_not_version_or_filename() {
        assert!(supported(true, "1.21.11", Some(BRIDGE_ARTIFACT_SHA256)));
        assert!(!supported(false, "1.21.11", Some(BRIDGE_ARTIFACT_SHA256)));
        assert!(!supported(
            true,
            "1.21.11",
            Some("55ac97f7494daa3866bb3b4aa8d23e49b240fe5ced00fbf1742f7214fc77c52a")
        ));
        assert!(!supported(true, "other", Some(BRIDGE_ARTIFACT_SHA256)));
        assert!(!supported(true, "1.21.11", None));
    }
    /// Compile an external same-package harness against the inspected client JAR.
    /// No client source is copied into this repository and no client build is run.
    #[tokio::test]
    #[ignore = "requires explicit local client-JAR harness and Java paths"]
    async fn actual_shipped_java_worker_interoperates_with_supervisor() {
        use crate::launch::{process, resolve::LaunchSpec};
        let java =
            std::path::PathBuf::from(std::env::var("AURORA_TEST_JAVA").expect("explicit Java"));
        let classpath =
            std::env::var("AURORA_TEST_CLIENT_CLASSPATH").expect("explicit external classpath");
        let root = std::path::PathBuf::from(
            std::env::var("AURORA_TEST_RUN_ROOT").expect("explicit disposable run root"),
        );
        let spec = LaunchSpec::fixture_command(
            java,
            root.clone(),
            vec![
                "-cp".into(),
                classpath,
                "com.aurora.client.launcher.LauncherContractHarness".into(),
            ],
        );
        let session = Session::prepare().unwrap();
        let handle = session.handle();
        let running = process::spawn_supervised_with_bridge(
            spec,
            &root.join("logs"),
            Arc::new(|_| {}),
            Some(21),
            Some(session),
        )
        .unwrap();
        assert!(running.status.blocks_launch());
        let mut transitions = Vec::new();
        timeout(Duration::from_secs(15), async {
            loop {
                if let Some(snapshot) = handle.snapshot() {
                    if transitions.last() != Some(&snapshot.state) {
                        match snapshot.state {
                            GameplayState::MainMenu => {
                                assert!(
                                    snapshot.world.is_none()
                                        && snapshot.server_name.is_none()
                                        && snapshot.server_address.is_none()
                                );
                                println!("actual client: MAIN_MENU");
                            }
                            GameplayState::Singleplayer => {
                                assert_eq!(snapshot.world.as_deref(), Some("Fixture World"));
                                println!("actual client: SINGLEPLAYER identity verified");
                            }
                            GameplayState::Multiplayer => {
                                assert_eq!(snapshot.server_name.as_deref(), Some("Fixture SMP"));
                                assert_eq!(
                                    snapshot.server_address.as_deref(),
                                    Some("example.invalid")
                                );
                                println!("actual client: MULTIPLAYER distinct identities verified");
                            }
                        }
                        transitions.push(snapshot.state);
                    }
                }
                if !process::snapshot("actual-client-worker")
                    .status
                    .blocks_launch()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert!(
            transitions
                == vec![
                    GameplayState::MainMenu,
                    GameplayState::Singleplayer,
                    GameplayState::MainMenu,
                    GameplayState::Multiplayer,
                    GameplayState::MainMenu
                ]
        );
        assert_eq!(process::snapshot("actual-client-worker").exit_code, Some(0));
        assert!(handle.snapshot().is_none());
    }
}
