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
use tauri::Emitter;
use tokio::sync::{mpsc, oneshot};

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
            gameplay_capability: false,
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
    Preferences(DiscordPreferences),
    Process(GameActivity),
    Shutdown(oneshot::Sender<()>),
}
static SENDER: OnceLock<mpsc::UnboundedSender<Message>> = OnceLock::new();
pub async fn connect() -> DiscordState {
    if let Some(sender) = SENDER.get() {
        let (tx, rx) = oneshot::channel();
        let _ = sender.send(Message::Connect(tx));
        let _ = tokio::time::timeout(Duration::from_secs(5), rx).await;
    }
    state()
}
pub fn preferences_changed(preferences: DiscordPreferences) {
    status().lock().unwrap().preferences = preferences.clone();
    if let Some(sender) = SENDER.get() {
        let _ = sender.send(Message::Preferences(preferences));
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
    requested: bool,
}
impl<T: Transport> Worker<T> {
    fn new(transport: T, preferences: DiscordPreferences) -> Self {
        Self {
            transport,
            requested: preferences.enabled,
            preferences,
            games: BTreeMap::new(),
            connection: Connection::Ready,
        }
    }
    async fn reconcile(&mut self, id: Option<&str>, reconnect: bool) {
        let Some(id) = id else {
            self.connection = Connection::ConfigurationMissing;
            return;
        };
        if reconnect {
            self.transport.disconnect();
            self.connection = Connection::Ready;
        }
        if self.connection != Connection::Connected {
            if !self.requested {
                return;
            }
            match self.transport.connect(id).await {
                Ok(()) => self.connection = Connection::Connected,
                Err(error) => {
                    self.fail(error);
                    return;
                }
            }
        }
        // Deterministic: most recently started Running child wins, then Starting.
        let game = self
            .games
            .values()
            .filter(|g| g.status.blocks_launch())
            .max_by_key(|g| {
                (
                    g.status == crate::launch::state::LaunchProcessStatus::Running,
                    g.started_at,
                    &g.instance_id,
                )
            });
        let activity = activity(&self.preferences, game);
        if let Err(error) = self.transport.publish(activity.as_ref()).await {
            self.fail(error);
        }
    }
    fn fail(&mut self, error: Failure) {
        self.requested = self.preferences.enabled;
        self.transport.disconnect();
        self.connection = match error {
            Failure::Unavailable => Connection::NotDetected,
            Failure::Closed => Connection::Closed,
            Failure::Invalid => Connection::Failed,
        };
    }
    async fn stop(&mut self) {
        let _ = self.transport.publish(None).await;
        self.transport.disconnect();
    }
}
pub fn initialize(app: tauri::AppHandle, preferences: DiscordPreferences) {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    if SENDER.set(sender).is_err() {
        return;
    }
    status().lock().unwrap().preferences = preferences.clone();
    tauri::async_runtime::spawn(async move {
        let mut worker = Worker::new(Native::default(), preferences);
        let mut timer = tokio::time::interval(Duration::from_secs(15));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let mut reconnect = false;
            let mut connected_reply = None;
            tokio::select! {
                message=receiver.recv()=>match message {
                    Some(Message::Connect(reply))=> {worker.requested=true; reconnect=true;connected_reply=Some(reply);},
                    Some(Message::Preferences(preferences))=> {worker.requested=preferences.enabled || worker.connection==Connection::Connected;worker.preferences=preferences;},
                    Some(Message::Process(game))=> {worker.games.insert(game.instance_id.clone(),game);},
                    Some(Message::Shutdown(reply))=> {worker.stop().await;let _=reply.send(());break;},
                    None=>{worker.stop().await;break;},
                },
                _=timer.tick()=>{},
            }
            worker.reconcile(application_id(), reconnect).await;
            let new = DiscordState {
                connection: worker.connection,
                configured: application_id().is_some(),
                preferences: worker.preferences.clone(),
                gameplay_capability: false,
            };
            let changed = {
                let mut current = status().lock().unwrap();
                if *current != new {
                    *current = new.clone();
                    true
                } else {
                    false
                }
            };
            if changed {
                let _ = app.emit("discord-state", new);
            }
            if let Some(reply) = connected_reply {
                let _ = reply.send(());
            }
        }
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
    }
    impl Transport for Fake {
        async fn connect(&mut self, _: &str) -> Result<(), Failure> {
            self.connections += 1;
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
        worker.requested = true;
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
    fn world_and_server_capability_is_explicitly_unavailable() {
        assert!(!DiscordState::default().gameplay_capability);
    }
}
