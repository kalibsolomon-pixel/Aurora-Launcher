//! Minecraft 1.21.11 QuickPlayLog. The client writes a one-element JSON
//! array after handling the login packet, including in an integrated server.
use crate::gameplay_history::Recorder;
use crate::gameplay_history::{Mode, ServerTarget, WorldSaveId};
use crate::install::coordination::{checked_directory, redirected};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

const MAX_BYTES: u64 = 8192;

#[derive(Clone)]
pub(crate) struct SessionLog {
    root: PathBuf,
    directory: PathBuf,
    filename: String,
    id: uuid::Uuid,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct GameVisit {
    pub mode: Mode,
    pub world: Option<WorldSaveId>,
    pub server: Option<ServerTarget>,
    pub display: Option<String>,
    pub written_at: u64,
}

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "type")]
    kind: String,
    id: String,
    name: String,
    #[serde(rename = "lastPlayedTime")]
    last_played_time: String,
    gamemode: String,
}

impl SessionLog {
    pub fn new(root: &Path, directory: &Path) -> Option<Self> {
        Self::with_id(root, directory, uuid::Uuid::new_v4())
    }

    pub fn with_id(root: &Path, directory: &Path, id: uuid::Uuid) -> Option<Self> {
        checked_directory(root, directory.parent()?).ok()?;
        match fs::symlink_metadata(directory) {
            Ok(meta) if meta.is_dir() && !redirected(&meta) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            _ => return None,
        }
        Some(Self {
            root: root.to_owned(),
            directory: directory.to_owned(),
            filename: format!("aurora-activity-{id}.json"),
            id,
        })
    }

    pub fn id(&self) -> uuid::Uuid {
        self.id
    }

    pub fn argument(&self) -> String {
        format!("logs/{}", self.filename)
    }

    fn path(&self) -> Option<PathBuf> {
        let directory = checked_directory(&self.root, &self.directory).ok()?;
        Some(directory.join(&self.filename))
    }

    pub fn snapshot(&self) -> Option<GameVisit> {
        let path = self.path()?;
        let metadata = fs::symlink_metadata(&path).ok()?;
        let bytes = crate::nbt::read_file(&path, MAX_BYTES).ok()?;
        let stamp = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        parse(&bytes, stamp.as_secs())
    }

    pub fn remove(&self) {
        if let Some(path) = self.path() {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) struct Observation {
    state: Arc<Mutex<ObservationState>>,
    _watcher: Option<notify::RecommendedWatcher>,
}

struct ObservationState {
    log: SessionLog,
    recorder: Arc<Recorder>,
    last: Option<GameVisit>,
    closed: bool,
}

impl ObservationState {
    fn capture(&mut self) {
        if self.closed {
            return;
        }
        if let Some(visit) = self.log.snapshot()
            && self.last.as_ref() != Some(&visit)
            && self.recorder.record_game_visit(visit.clone()).is_ok()
        {
            self.last = Some(visit);
        }
    }
}

impl Observation {
    pub fn start(log: SessionLog, recorder: Arc<Recorder>) -> Self {
        use notify::{Event, EventKind, RecursiveMode, Watcher};
        let directory = log.path().and_then(|path| Some(path.parent()?.to_owned()));
        let filename = log.filename.clone();
        let state = Arc::new(Mutex::new(ObservationState {
            log,
            recorder,
            last: None,
            closed: false,
        }));
        let callback = state.clone();
        let watcher = directory.and_then(|directory| {
            let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
                let relevant = match event {
                    Ok(event) => {
                        event.need_rescan()
                            || (!matches!(event.kind, EventKind::Access(_))
                                && event.paths.iter().any(|path| {
                                    path.file_name()
                                        .is_some_and(|name| name == filename.as_str())
                                }))
                    }
                    Err(_) => true,
                };
                if relevant && let Ok(mut state) = callback.lock() {
                    state.capture();
                }
            })
            .ok()?;
            watcher
                .watch(&directory, RecursiveMode::NonRecursive)
                .ok()?;
            Some(watcher)
        });
        if let Ok(mut state) = state.lock() {
            state.capture();
        }
        Self {
            state,
            _watcher: watcher,
        }
    }

    pub fn finish(self) {
        if let Ok(mut state) = self.state.lock() {
            state.capture();
            state.closed = true;
            state.log.remove();
        }
    }
}

fn parse(bytes: &[u8], written_at: u64) -> Option<GameVisit> {
    if bytes.len() as u64 > MAX_BYTES {
        return None;
    }
    let [entry]: [Entry; 1] = serde_json::from_slice(bytes).ok()?;
    if entry.last_played_time.is_empty()
        || !matches!(
            entry.gamemode.as_str(),
            "survival" | "creative" | "adventure" | "spectator"
        )
    {
        return None;
    }
    let (mode, world, server) = match entry.kind.as_str() {
        "singleplayer" => (
            Mode::Singleplayer,
            Some(WorldSaveId::parse(entry.id).ok()?),
            None,
        ),
        "multiplayer" => (
            Mode::Multiplayer,
            None,
            Some(ServerTarget::parse(entry.id).ok()?),
        ),
        _ => return None,
    };
    let name = super::activity_bridge::sanitize(&entry.name);
    let display = (!name.is_empty() && name.len() <= 512).then_some(name);
    Some(GameVisit {
        mode,
        world,
        server,
        display,
        written_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_record(kind: &str, id: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!([{
            "type": kind, "id": id, "name": "Fixture",
            "lastPlayedTime": "2026-11-03T18:04:05Z", "gamemode": "survival"
        }]))
        .unwrap()
    }

    #[test]
    fn watcher_records_multiple_destinations_in_one_session() {
        use crate::gameplay_history::{Outcome, Store};
        let root = std::env::temp_dir().join(format!("aurora-watch-{}", uuid::Uuid::new_v4()));
        let logs = root.join("logs");
        fs::create_dir_all(&logs).unwrap();
        let history = root.join("history.json");
        let store = Store::open(history.clone()).unwrap();
        let instance =
            crate::instances::InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        let recorder = Arc::new(store.start(instance).unwrap());
        let log = SessionLog::new(&root, &logs).unwrap();
        let path = root.join(log.argument());
        let watcher = Observation::start(log, recorder.clone());
        assert!(watcher._watcher.is_some());

        fs::write(&path, b"[").unwrap();
        fs::write(&path, fixture_record("multiplayer", "first.invalid")).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while store
            .recent(Mode::Multiplayer, None, 10)
            .unwrap()
            .is_empty()
        {
            assert!(
                std::time::Instant::now() < deadline,
                "first visit was not observed before exit"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let replacement = logs.join("replacement");
        fs::write(
            &replacement,
            fixture_record("multiplayer", "second.invalid"),
        )
        .unwrap();
        fs::rename(replacement, &path).unwrap();
        watcher.finish();
        recorder.finish(Outcome::Abnormal).unwrap();
        let visits = store.recent(Mode::Multiplayer, None, 10).unwrap();
        assert_eq!(visits.len(), 2);
        assert!(visits.iter().all(|visit| visit.duration_ms == 0));
        assert!(!path.exists());
        fs::remove_file(history).unwrap();
        fs::remove_dir(logs).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn restart_reconciles_the_session_uuid_without_adding_duration() {
        use crate::gameplay_history::Store;
        let root = std::env::temp_dir().join(format!("aurora-reconcile-{}", uuid::Uuid::new_v4()));
        let paths = crate::paths::ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let instance =
            crate::instances::InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        let instance_paths = paths.instance_paths(&instance);
        fs::create_dir_all(instance_paths.logs()).unwrap();
        let store = Store::open(root.join("before.json")).unwrap();
        let log = SessionLog::new(&root, instance_paths.logs()).unwrap();
        let _recorder = store.start_with_id(instance.clone(), log.id()).unwrap();
        fs::write(
            instance_paths.root().join(log.argument()),
            fixture_record("singleplayer", "Local World"),
        )
        .unwrap();
        fs::create_dir_all(paths.launcher_dir()).unwrap();
        fs::copy(root.join("before.json"), paths.gameplay_history_file()).unwrap();
        let recovered = Store::open_managed(&paths).unwrap();
        let recent = recovered
            .recent(Mode::Singleplayer, Some(&instance), 10)
            .unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(
            recovered
                .playtime(Some(&instance), recent[0].last_played_at)
                .unwrap()
                .all_time_ms,
            0
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn supervised_artifact_persists_distinct_worlds_and_a_server() {
        use crate::gameplay_history::{Outcome, Store};
        use crate::instances::InstanceId;

        let root =
            std::env::temp_dir().join(format!("aurora-activity-test-{}", uuid::Uuid::new_v4()));
        let logs = root.join("logs");
        fs::create_dir_all(&logs).unwrap();
        let path = root.join("history.json");
        let store = Store::open(path.clone()).unwrap();
        let instance = InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();

        for (kind, id) in [
            ("singleplayer", "save-a"),
            ("singleplayer", "save-b"),
            ("multiplayer", "example.invalid"),
        ] {
            let recorder = store.start(instance.clone()).unwrap();
            let log = SessionLog::new(&root, &logs).unwrap();
            let file = root.join(log.argument());
            assert!(log.snapshot().is_none());
            fs::write(
                &file,
                serde_json::to_vec(&serde_json::json!([{
                    "type": kind, "id": id, "name": "Same title",
                    "lastPlayedTime": "2026-11-03T18:04:05Z", "gamemode": "survival"
                }]))
                .unwrap(),
            )
            .unwrap();
            recorder.record_game_visit(log.snapshot().unwrap()).unwrap();
            recorder.finish(Outcome::Abnormal).unwrap();
            log.remove();
            assert!(!file.exists());
        }
        let saved = fs::read(&path).unwrap();
        let restarted = root.join("reopened.json");
        fs::write(&restarted, saved).unwrap();
        let reopened = Store::open(restarted.clone()).unwrap();
        let worlds = reopened
            .recent(Mode::Singleplayer, Some(&instance), 10)
            .unwrap();
        assert_eq!(worlds.len(), 2);
        assert_ne!(worlds[0].id, worlds[1].id);
        assert_eq!(
            reopened
                .recent(Mode::Multiplayer, Some(&instance), 10)
                .unwrap()
                .len(),
            1
        );
        fs::remove_file(restarted).unwrap();
        fs::remove_file(path).unwrap();
        fs::remove_dir(logs).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn accepts_game_written_entries_and_rejects_requests_and_invalid_destinations() {
        let entry = |kind, id| {
            serde_json::to_vec(&serde_json::json!([{
                "type": kind, "id": id, "name": "Destination",
                "lastPlayedTime": "2026-11-03T18:04:05Z", "gamemode": "survival"
            }]))
            .unwrap()
        };
        let world = parse(&entry("singleplayer", "Save directory"), 42).unwrap();
        assert_eq!(world.world.unwrap().as_str(), "Save directory");
        assert_eq!(world.written_at, 42);
        let server = parse(&entry("multiplayer", "example.invalid:25565"), 42).unwrap();
        assert_eq!(server.server.unwrap().as_str(), "example.invalid:25565");
        assert!(parse(&entry("singleplayer", "../escape"), 42).is_none());
        assert!(parse(&entry("multiplayer", "https://example.invalid"), 42).is_none());
        assert!(parse(br#"{"requested":"example.invalid"}"#, 42).is_none());
        assert!(parse(&vec![b' '; MAX_BYTES as usize + 1], 42).is_none());
    }
}
