//! Minecraft 1.21.11 QuickPlayLog. The client writes a one-element JSON
//! array after handling the login packet, including in an integrated server.
use crate::gameplay_history::{Mode, ServerTarget, WorldSaveId};
use crate::install::coordination::{checked_directory, redirected};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_BYTES: u64 = 8192;

pub(crate) struct SessionLog {
    root: PathBuf,
    directory: PathBuf,
    filename: String,
}

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
        checked_directory(root, directory.parent()?).ok()?;
        match fs::symlink_metadata(directory) {
            Ok(meta) if meta.is_dir() && !redirected(&meta) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            _ => return None,
        }
        Some(Self {
            root: root.to_owned(),
            directory: directory.to_owned(),
            filename: format!("aurora-activity-{}.json", uuid::Uuid::new_v4()),
        })
    }

    pub fn argument(&self) -> String {
        format!("logs/{}", self.filename)
    }

    pub fn read(&self) -> Option<GameVisit> {
        let directory = checked_directory(&self.root, &self.directory).ok()?;
        let path = directory.join(&self.filename);
        let metadata = fs::symlink_metadata(&path).ok()?;
        if !metadata.is_file() || redirected(&metadata) {
            return None;
        }
        let mut bytes = Vec::new();
        let result = if metadata.len() <= MAX_BYTES {
            File::open(&path)
                .and_then(|file| file.take(MAX_BYTES + 1).read_to_end(&mut bytes))
                .ok()
                .and_then(|_| {
                    let stamp = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
                    parse(&bytes, stamp.as_secs())
                })
        } else {
            None
        };
        // The filename is generated for this exact child and never reused.
        let _ = fs::remove_file(path);
        result
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
            assert!(log.read().is_none());
            fs::write(
                &file,
                serde_json::to_vec(&serde_json::json!([{
                    "type": kind, "id": id, "name": "Same title",
                    "lastPlayedTime": "2026-11-03T18:04:05Z", "gamemode": "survival"
                }]))
                .unwrap(),
            )
            .unwrap();
            recorder.record_game_visit(log.read().unwrap()).unwrap();
            recorder.finish(Outcome::Abnormal).unwrap();
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
