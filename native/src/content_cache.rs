//! Bounded, UI-thread snapshots of installed content. Snapshots are display
//! hints only; every entry still refreshes through the authoritative scanner.
use std::collections::{HashMap, VecDeque};

use crate::instances::ContentItem;

const MAX_ENTRIES: usize = 12;
pub const MAX_ROWS: usize = 4096;
const MAX_ICONS: usize = 64;

#[derive(Clone, Default)]
pub struct Snapshot {
    pub items: Vec<ContentItem>,
    pub icons: HashMap<String, Option<slint::Image>>,
    pub revision: Option<u64>,
}

#[derive(Default)]
pub struct ContentCache {
    entries: VecDeque<(String, i32, Snapshot)>,
}

impl ContentCache {
    pub fn get(&mut self, id: &str, tab: i32) -> Option<Snapshot> {
        let index = self
            .entries
            .iter()
            .position(|(key, kind, _)| key == id && *kind == tab)?;
        let entry = self.entries.remove(index)?;
        let snapshot = entry.2.clone();
        self.entries.push_back(entry);
        Some(snapshot)
    }

    pub fn put(&mut self, id: &str, tab: i32, mut snapshot: Snapshot) {
        self.entries
            .retain(|(key, kind, _)| key != id || *kind != tab);
        // An unusually large inventory remains usable in the active workspace,
        // but must not evict the memory bound of the navigation cache.
        if snapshot.items.len() > MAX_ROWS {
            return;
        }
        if snapshot.icons.len() > MAX_ICONS {
            snapshot.icons = snapshot.icons.into_iter().take(MAX_ICONS).collect();
        }
        self.entries.push_back((id.to_owned(), tab, snapshot));
        while self.entries.len() > MAX_ENTRIES
            || self
                .entries
                .iter()
                .map(|(_, _, value)| value.items.len())
                .sum::<usize>()
                > MAX_ROWS
        {
            self.entries.pop_front();
        }
    }

    pub fn invalidate(&mut self, id: &str) {
        self.entries.retain(|(key, _, _)| key != id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// A cheap artwork freshness hint, evaluated on a blocking worker only when a
/// content tab is requested. No watcher, idle timer, file hashing, or network.
/// Always scan the inventory too: mtimes are not filesystem authority.
pub async fn folder_revision(
    host: aurora_launcher_lib::host::Host,
    id: String,
    tab: i32,
) -> Option<u64> {
    tokio::task::spawn_blocking(move || {
        use aurora_launcher_lib::{instances::InstanceId, paths::ManagedPaths};
        let id = InstanceId::new(id).ok()?;
        let managed = ManagedPaths::from_app_local_data_dir(host.local_data_dir().ok()?).ok()?;
        let paths = managed.instance_paths(&id);
        let directory = match tab {
            0 => paths.mods(),
            1 => paths.resourcepacks(),
            2 => paths.shaderpacks(),
            _ => return None,
        };
        // Match the managed layout before reading metadata; a redirected
        // instance or content directory must be left to the core scanner.
        let root = managed.data_root().canonicalize().ok()?;
        let expected = root.join("instances").join(id.as_str());
        if paths.root().canonicalize().ok()? != expected
            || directory.canonicalize().ok()? != expected.join(directory.file_name()?)
        {
            return None;
        }
        revision(directory)
    })
    .await
    .ok()
    .flatten()
}

fn revision(directory: &std::path::Path) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let metadata = std::fs::symlink_metadata(directory).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return None;
    }
    metadata.modified().ok()?.hash(&mut hash);
    let mut children = std::fs::read_dir(directory)
        .ok()?
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    children.sort_by_key(|entry| entry.file_name());
    for entry in children {
        entry.file_name().hash(&mut hash);
        let metadata = std::fs::symlink_metadata(entry.path()).ok()?;
        metadata.modified().ok()?.hash(&mut hash);
        metadata.len().hash(&mut hash);
        // Directory packs can change artwork without changing their parent.
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            for name in ["pack.png", "pack.mcmeta"] {
                if let Ok(art) = std::fs::symlink_metadata(entry.path().join(name)) {
                    art.modified().ok()?.hash(&mut hash);
                    art.len().hash(&mut hash);
                }
            }
        }
    }
    Some(hash.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(count: usize) -> Snapshot {
        Snapshot {
            items: (0..count)
                .map(|i| ContentItem {
                    row: crate::ContentRow {
                        id: i.to_string().into(),
                        ..Default::default()
                    },
                    icon_source: None,
                    project: None,
                })
                .collect(),
            icons: HashMap::new(),
            revision: None,
        }
    }

    #[test]
    fn snapshots_are_scoped_invalidated_and_bounded_by_rows_and_lru() {
        let mut cache = ContentCache::default();
        cache.put("a", 0, snapshot(1));
        cache.put("a", 1, snapshot(2));
        cache.put("b", 0, snapshot(3));
        assert_eq!(cache.get("a", 0).unwrap().items.len(), 1);
        assert_eq!(cache.get("a", 1).unwrap().items.len(), 2);
        cache.invalidate("a");
        assert!(cache.get("a", 0).is_none());
        assert!(cache.get("a", 1).is_none());
        assert_eq!(cache.get("b", 0).unwrap().items.len(), 3);
        for i in 0..MAX_ENTRIES + 2 {
            cache.put(&i.to_string(), 0, snapshot(1));
        }
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        assert!(cache.get("0", 0).is_none());
        assert!(cache.get("2", 0).is_some());
        cache.put("last", 0, snapshot(1));
        assert!(cache.get("3", 0).is_none());
        cache.put("large", 0, snapshot(MAX_ROWS));
        assert_eq!(cache.entries.len(), 1);
        cache.put("too-large", 0, snapshot(MAX_ROWS + 1));
        assert!(cache.get("too-large", 0).is_none());
        cache.clear();
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn folder_freshness_changes_after_external_file_edits() {
        let root = std::env::temp_dir().join(format!(
            "aurora-content-revision-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let first = revision(&root).unwrap();
        std::fs::write(root.join("pack.zip"), b"first").unwrap();
        let added = revision(&root).unwrap();
        assert_ne!(first, added);
        std::fs::write(root.join("pack.zip"), b"longer replacement").unwrap();
        assert_ne!(added, revision(&root).unwrap());
        // The disposable fixture is retained for evidence, never owner content.
    }
}
