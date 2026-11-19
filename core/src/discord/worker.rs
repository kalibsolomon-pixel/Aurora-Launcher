use super::{
    DiscordPreferences, GameActivity, activity, application_id,
    transport::{Failure, Native, Transport},
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

const HEALTH_INTERVAL: Duration = Duration::from_secs(15);
fn retry_delay(failures: u32) -> Duration {
    Duration::from_secs((15u64 << failures.saturating_sub(1).min(3)).min(120))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Connection {
    ConfigurationMissing,
    Ready,
    Connected,
    NotDetected,
    Closed,
    Failed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordState {
    pub connection: Connection,
    pub configured: bool,
    pub preferences: DiscordPreferences,
    pub gameplay_capability: bool,
}
impl Default for DiscordState {
    fn default() -> Self {
        Self {
            connection: if application_id().is_some() {
                Connection::Ready
            } else {
                Connection::ConfigurationMissing
            },
            configured: application_id().is_some(),
            preferences: Default::default(),
            gameplay_capability: true,
        }
    }
}
fn status() -> &'static Mutex<DiscordState> {
    static STATUS: OnceLock<Mutex<DiscordState>> = OnceLock::new();
    STATUS.get_or_init(|| Mutex::new(DiscordState::default()))
}
pub fn state() -> DiscordState {
    status().lock().unwrap().clone()
}
enum Message {
    Connect(oneshot::Sender<()>),
    Preferences,
    Process(GameActivity),
    Shutdown(oneshot::Sender<()>),
}
static SENDER: OnceLock<mpsc::UnboundedSender<Message>> = OnceLock::new();
static GAMEPLAY_WAKE: OnceLock<mpsc::Sender<()>> = OnceLock::new();
/// Coalesced wakeup only; private snapshots stay in native handles.
pub fn gameplay_changed() {
    if let Some(sender) = GAMEPLAY_WAKE.get() {
        let _ = sender.try_send(());
    }
}
pub async fn connect() -> DiscordState {
    if let Some(sender) = SENDER.get() {
        let (tx, rx) = oneshot::channel();
        let _ = sender.send(Message::Connect(tx));
        let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;
    }
    state()
}
pub fn preferences_changed(preferences: DiscordPreferences) {
    status().lock().unwrap().preferences = preferences;
    if let Some(sender) = SENDER.get() {
        let _ = sender.send(Message::Preferences);
    }
}
pub fn process_changed(game: GameActivity) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.send(Message::Process(game));
    }
}
pub async fn shutdown() {
    if let Some(sender) = SENDER.get() {
        let (tx, rx) = oneshot::channel();
        let _ = sender.send(Message::Shutdown(tx));
        let _ = tokio::time::timeout(Duration::from_secs(3), rx).await;
    }
}

struct Worker<T> {
    transport: T,
    preferences: DiscordPreferences,
    games: BTreeMap<String, GameActivity>,
    connection: Connection,
    retry_at: Option<Instant>,
    failures: u32,
    health_at: Instant,
}
impl<T: Transport> Worker<T> {
    fn new(transport: T, preferences: DiscordPreferences) -> Self {
        Self {
            transport,
            retry_at: None,
            failures: 0,
            health_at: Instant::now(),
            preferences,
            games: BTreeMap::new(),
            connection: Connection::Ready,
        }
    }
    fn change_preferences(&mut self, preferences: DiscordPreferences) {
        if preferences.enabled != self.preferences.enabled {
            self.retry_at = None;
            self.failures = 0;
            self.health_at = Instant::now();
        }
        self.preferences = preferences;
    }
    fn next_wake(&self, id: Option<&str>) -> Option<Instant> {
        (self.preferences.enabled && id.is_some())
            .then_some(self.retry_at.unwrap_or(self.health_at))
    }
    async fn reconcile(&mut self, id: Option<&str>, reconnect: bool) {
        if !self.preferences.enabled {
            if self.connection == Connection::Connected {
                let _ = self.transport.publish(None).await;
            }
            self.transport.disconnect();
            self.connection = if id.is_some() {
                Connection::Ready
            } else {
                Connection::ConfigurationMissing
            };
            self.retry_at = None;
            self.failures = 0;
            return;
        }
        let Some(id) = id else {
            self.connection = Connection::ConfigurationMissing;
            return;
        };
        if reconnect {
            self.transport.disconnect();
            self.connection = Connection::Ready;
            self.retry_at = None;
            self.failures = 0;
        }
        if self
            .retry_at
            .is_some_and(|deadline| Instant::now() < deadline)
        {
            return;
        }
        if self.connection != Connection::Connected {
            match self.transport.connect(id).await {
                Ok(()) => self.connection = Connection::Connected,
                Err(error) => {
                    self.fail(error);
                    return;
                }
            }
        }
        // Deterministic: most recently started Running child wins, then Starting.
        let game = self.games.values().max_by_key(|g| {
            (
                g.status == crate::launch::state::LaunchProcessStatus::Running,
                g.started_at,
                &g.instance_id,
            )
        });
        let activity = activity(&self.preferences, game);
        if let Err(error) = self.transport.publish(activity.as_ref()).await {
            self.fail(error);
        } else {
            self.failures = 0;
            self.retry_at = None;
            self.health_at = Instant::now() + HEALTH_INTERVAL;
        }
    }
    fn fail(&mut self, error: Failure) {
        self.transport.disconnect();
        self.failures = self.failures.saturating_add(1);
        self.retry_at = Some(Instant::now() + retry_delay(self.failures));
        self.connection = match error {
            Failure::Unavailable => Connection::NotDetected,
            Failure::Closed => Connection::Closed,
            Failure::Invalid => Connection::Failed,
        };
    }
    async fn stop(&mut self) {
        if self.connection == Connection::Connected {
            let _ = self.transport.publish(None).await;
        }
        self.transport.disconnect();
    }

    async fn run(
        &mut self,
        id: Option<&str>,
        mut receiver: mpsc::UnboundedReceiver<Message>,
        mut gameplay_wake: mpsc::Receiver<()>,
        status: &Mutex<DiscordState>,
        notify: impl Fn(DiscordState),
    ) {
        loop {
            let mut connected_reply = None;
            let next_wake = self.next_wake(id);
            tokio::select! {
                message = receiver.recv() => match message {
                    Some(Message::Connect(reply)) => connected_reply = Some(reply),
                    Some(Message::Preferences) => {}
                    Some(Message::Process(game)) => {
                        if game.status.blocks_launch() {
                            self.games.insert(game.instance_id.clone(), game);
                        } else {
                            self.games.remove(&game.instance_id);
                        }
                    }
                    Some(Message::Shutdown(reply)) => {
                        self.stop().await;
                        let _ = reply.send(());
                        break;
                    }
                    None => {
                        self.stop().await;
                        break;
                    }
                },
                _ = wait_for_deadline(next_wake) => {},
                Some(()) = gameplay_wake.recv() => {},
            }
            // Apply the latest persisted preferences before any IPC, including
            // a reconnect or health check queued ahead of the preference wake.
            self.change_preferences(status.lock().unwrap().preferences.clone());
            self.reconcile(id, connected_reply.is_some()).await;
            let changed = {
                let mut current = status.lock().unwrap();
                // A save can finish while IPC is in flight. Leave its state
                // intact and reconcile it on the next wake.
                if current.preferences == self.preferences && current.connection != self.connection
                {
                    current.connection = self.connection;
                    Some(current.clone())
                } else {
                    None
                }
            };
            if let Some(new) = changed {
                notify(new);
            }
            if let Some(reply) = connected_reply {
                let _ = reply.send(());
            }
        }
    }
}

async fn wait_for_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

pub fn initialize(app: crate::host::Host, preferences: DiscordPreferences) {
    let (sender, receiver) = mpsc::unbounded_channel();
    if SENDER.set(sender).is_err() {
        return;
    }
    let (wake, gameplay_wake) = mpsc::channel(1);
    let _ = GAMEPLAY_WAKE.set(wake);
    status().lock().unwrap().preferences = preferences.clone();
    app.clone().spawn(async move {
        Worker::new(Native::default(), preferences)
            .run(application_id(), receiver, gameplay_wake, status(), |new| {
                let _ = app.emit("discord-state", new);
            })
            .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        failure: Option<Failure>,
        connected: bool,
        calls: Vec<Option<super::super::Activity>>,
        connections: usize,
        unavailable_attempts: usize,
    }
    impl Transport for Fake {
        async fn connect(&mut self, _: &str) -> Result<(), Failure> {
            self.connections += 1;
            if self.connections <= self.unavailable_attempts {
                return Err(Failure::Unavailable);
            }
            if let Some(error) = self.failure {
                return Err(error);
            }
            self.connected = true;
            Ok(())
        }
        async fn publish(
            &mut self,
            activity: Option<&super::super::Activity>,
        ) -> Result<(), Failure> {
            if let Some(error) = self.failure {
                return Err(error);
            }
            self.calls.push(activity.cloned());
            Ok(())
        }
        fn disconnect(&mut self) {
            self.connected = false;
        }
    }
    #[tokio::test]
    async fn missing_configuration_and_disabled_boot_do_not_contact_discord() {
        let mut worker = Worker::new(Fake::default(), Default::default());
        worker.reconcile(Some("test-only"), false).await;
        assert_eq!(worker.transport.connections, 0);
        worker.preferences.enabled = true;
        worker.reconcile(None, false).await;
        assert_eq!(worker.connection, Connection::ConfigurationMissing);
        assert_eq!(worker.transport.connections, 0);
    }
    #[tokio::test]
    async fn unavailable_failure_reconnect_and_closed_are_isolated() {
        for (error, status) in [
            (Failure::Unavailable, Connection::NotDetected),
            (Failure::Invalid, Connection::Failed),
            (Failure::Closed, Connection::Closed),
        ] {
            let mut worker = Worker::new(
                Fake {
                    failure: Some(error),
                    ..Default::default()
                },
                DiscordPreferences {
                    enabled: true,
                    ..Default::default()
                },
            );
            worker.reconcile(Some("test-only"), false).await;
            assert_eq!(worker.connection, status);
            worker.transport.failure = None;
            worker.reconcile(Some("test-only"), true).await;
            assert_eq!(worker.connection, Connection::Connected);
            assert_eq!(worker.transport.connections, 2);
        }
    }
    #[tokio::test]
    async fn off_and_shutdown_clear_activity() {
        let mut worker = Worker::new(
            Fake::default(),
            DiscordPreferences {
                enabled: true,
                ..Default::default()
            },
        );
        worker.reconcile(Some("test-only"), false).await;
        assert_eq!(
            worker
                .transport
                .calls
                .last()
                .unwrap()
                .as_ref()
                .unwrap()
                .details,
            "In Launcher"
        );
        worker.preferences.enabled = false;
        worker.reconcile(Some("test-only"), false).await;
        assert_eq!(worker.transport.calls.last(), Some(&None));
        worker.stop().await;
        assert_eq!(worker.transport.calls.last(), Some(&None));
        assert!(!worker.transport.connected);
    }
    #[test]
    fn world_and_server_capability_is_available_independently_of_discord_setup() {
        assert!(DiscordState::default().gameplay_capability);
    }

    #[tokio::test]
    async fn late_discord_start_honors_capped_backoff_and_recovers_automatically() {
        let mut worker = Worker::new(
            Fake {
                failure: Some(Failure::Unavailable),
                ..Default::default()
            },
            DiscordPreferences {
                enabled: true,
                ..Default::default()
            },
        );
        assert!(worker.next_wake(Some("test-only")).is_some());
        for expected in [15, 30, 60, 120, 120] {
            worker.retry_at = None; // advance the test clock to the scheduled retry
            worker.reconcile(Some("test-only"), false).await;
            assert_eq!(retry_delay(worker.failures).as_secs(), expected);
            let calls = worker.transport.connections;
            for _ in 0..10 {
                worker.reconcile(Some("test-only"), false).await;
            }
            assert_eq!(
                worker.transport.connections, calls,
                "state events must not bypass backoff"
            );
        }
        worker.transport.failure = None;
        worker.retry_at = Some(Instant::now());
        worker.reconcile(Some("test-only"), false).await;
        assert_eq!(worker.connection, Connection::Connected);
        assert_eq!(worker.failures, 0);
        assert!(worker.retry_at.is_none());
        assert!(worker.next_wake(Some("test-only")).unwrap() > Instant::now());
    }

    #[tokio::test]
    async fn disconnect_disable_and_reenable_preserve_activity_and_stop_disabled_ipc() {
        let mut worker = Worker::new(
            Fake::default(),
            DiscordPreferences {
                enabled: true,
                ..Default::default()
            },
        );
        worker.reconcile(Some("test-only"), false).await;
        worker.transport.failure = Some(Failure::Closed);
        worker.reconcile(Some("test-only"), false).await;
        assert_eq!(worker.connection, Connection::Closed);
        worker.transport.failure = None;
        worker.retry_at = Some(Instant::now());
        worker.reconcile(Some("test-only"), false).await;
        assert_eq!(worker.connection, Connection::Connected);
        worker.change_preferences(DiscordPreferences::default());
        worker.reconcile(Some("test-only"), false).await;
        assert_eq!(worker.transport.calls.last(), Some(&None));
        assert!(!worker.transport.connected);
        assert!(worker.next_wake(Some("test-only")).is_none());
        let connections = worker.transport.connections;
        let publishes = worker.transport.calls.len();
        for _ in 0..10 {
            worker.reconcile(Some("test-only"), true).await;
        }
        assert_eq!(worker.transport.connections, connections);
        assert_eq!(worker.transport.calls.len(), publishes);
        worker.change_preferences(DiscordPreferences {
            enabled: true,
            ..Default::default()
        });
        worker.reconcile(Some("test-only"), false).await;
        assert_eq!(worker.connection, Connection::Connected);
        assert_eq!(worker.transport.connections, connections + 1);
        assert_eq!(
            worker
                .transport
                .calls
                .last()
                .unwrap()
                .as_ref()
                .unwrap()
                .details,
            "In Launcher"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn actor_retries_at_startup_and_publishes_the_supervised_game_lifecycle() {
        use crate::launch::state::LaunchProcessStatus;

        let preferences = DiscordPreferences {
            enabled: true,
            instance_name: true,
            minecraft_version: true,
            elapsed_time: true,
            ..Default::default()
        };
        let mut worker = Worker::new(
            Fake {
                unavailable_attempts: 1,
                ..Default::default()
            },
            preferences.clone(),
        );
        let status = Mutex::new(DiscordState {
            connection: Connection::Ready,
            configured: true,
            preferences,
            gameplay_capability: true,
        });
        let (sender, receiver) = mpsc::unbounded_channel();
        let (_wake, gameplay_wake) = mpsc::channel(1);
        let (notify, mut notifications) = mpsc::unbounded_channel();
        let game = GameActivity {
            instance_id: "fixture".into(),
            instance_name: "Test instance".into(),
            minecraft_version: "1.21.11".into(),
            platform: "Fabric".into(),
            aurora_active: false,
            status: LaunchProcessStatus::Starting,
            started_at: Some(123),
            gameplay: None,
        };

        let boot = Instant::now();
        tokio::join!(
            worker.run(
                Some("test-only"),
                receiver,
                gameplay_wake,
                &status,
                |state| {
                    notify.send(state).unwrap();
                }
            ),
            async {
                assert_eq!(
                    notifications.recv().await.unwrap().connection,
                    Connection::NotDetected
                );
                assert_eq!((Instant::now() - boot).as_secs(), 0);
                let first_failure = Instant::now();
                assert_eq!(
                    notifications.recv().await.unwrap().connection,
                    Connection::Connected
                );
                assert_eq!(
                    (Instant::now() - first_failure).as_secs(),
                    retry_delay(1).as_secs()
                );

                sender.send(Message::Process(game.clone())).unwrap();
                sender
                    .send(Message::Process(GameActivity {
                        status: LaunchProcessStatus::Running,
                        ..game.clone()
                    }))
                    .unwrap();
                let (reply, done) = oneshot::channel();
                sender.send(Message::Connect(reply)).unwrap();
                done.await.unwrap();
                assert_eq!(status.lock().unwrap().connection, Connection::Connected);

                sender
                    .send(Message::Process(GameActivity {
                        status: LaunchProcessStatus::Exited,
                        ..game
                    }))
                    .unwrap();
                let (reply, done) = oneshot::channel();
                sender.send(Message::Shutdown(reply)).unwrap();
                done.await.unwrap();
            }
        );

        assert_eq!(worker.transport.connections, 3);
        let calls = &worker.transport.calls;
        assert_eq!(calls.len(), 6);
        assert_eq!(calls[0].as_ref().unwrap().details, "In Launcher");
        assert_eq!(calls[1].as_ref().unwrap().details, "Starting Minecraft");
        let playing = calls[2].as_ref().unwrap();
        assert_eq!(playing.details, "Playing Minecraft");
        assert_eq!(
            playing.state.as_deref(),
            Some("Minecraft 1.21.11 · Test instance")
        );
        assert_eq!(playing.timestamps.as_ref().unwrap().start, 123);
        assert_eq!(calls[3].as_ref(), Some(playing));
        assert_eq!(calls[4], calls[0]);
        assert_eq!(calls[5], None);
        assert!(!worker.transport.connected);
        assert!(worker.games.is_empty());
    }
}
