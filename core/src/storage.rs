//! Storage usage inspector and safe, lock-protected pruning for shared
//! verified caches, artwork cache, and unreferenced managed Java runtimes.
//!
//! User content (`saves/`, `mods/`, `config/`, `screenshots/`, `servers.dat`,
//! `options.txt`, accounts, and credentials) is NEVER pruned here.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::install::state::load_installed_state;
use crate::instances::{InstanceRegistry, InstanceState};
use crate::integrity::ArtifactTrust;
use crate::launch;
use crate::paths::ManagedPaths;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceStorageEntry {
    pub instance_id: String,
    pub display_name: String,
    pub total_bytes: u64,
    pub game_bytes: u64,
    pub mods_bytes: u64,
    pub saves_bytes: u64,
    pub backups_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStorageEntry {
    pub component: String,
    pub slot: String,
    pub size_bytes: u64,
    pub referenced_by_instances: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageOverview {
    pub total_bytes: u64,
    pub instances_bytes: u64,
    pub shared_cache_bytes: u64,
    pub unreferenced_cache_bytes: u64,
    pub artwork_cache_bytes: u64,
    pub runtimes_bytes: u64,
    pub unreferenced_runtimes_bytes: u64,
    pub instances: Vec<InstanceStorageEntry>,
    pub runtimes: Vec<RuntimeStorageEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PruneOptions {
    pub prune_unreferenced_cache: bool,
    pub prune_artwork_cache: bool,
    pub prune_unused_runtimes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PruneReport {
    pub reclaimed_bytes: u64,
    pub removed_cache_objects: usize,
    pub removed_artwork_files: usize,
    pub removed_runtimes: usize,
}

#[derive(Debug)]
pub struct StorageError(pub &'static str, pub String);

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

impl std::error::Error for StorageError {}

fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    let mut total = 0u64;
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        total = total.saturating_add(dir_size(&entry.path()));
    }
    total
}

/// Collects SHA-256 and SHA-1 digests referenced by all registered instances'
/// installed game manifests, plus required Java runtime components.
fn collect_references(
    managed: &ManagedPaths,
    registry: &InstanceRegistry,
) -> (HashSet<String>, HashSet<String>) {
    let mut referenced_digests = HashSet::new();
    let mut referenced_components = HashSet::new();

    for record in registry.instances() {
        let paths = managed.instance_paths(record.id());
        if let Ok(Some(manifest)) = load_installed_state(paths.game()) {
            for file in manifest.files() {
                match file.trust() {
                    ArtifactTrust::ExpectedDigestVerified { digest, .. } => {
                        referenced_digests.insert(digest.to_ascii_lowercase());
                    }
                    ArtifactTrust::SecureTransportObserved { observed_sha256 }
                    | ArtifactTrust::LocallyGenerated { observed_sha256 } => {
                        referenced_digests.insert(observed_sha256.to_ascii_lowercase());
                    }
                }
            }
        }
        if let Some(offline) = crate::launch::offline::load_verified_receipt(managed, record.id()) {
            referenced_components.insert(offline.runtime_component);
        }
        if let Ok(Some(aurora)) = crate::aurora::load_installed_state(managed, record.id()) {
            referenced_digests.insert(aurora.artifact().sha256().to_ascii_lowercase());
            if let Some(api) = aurora.fabric_api() {
                referenced_digests.insert(api.artifact().sha256().to_ascii_lowercase());
            }
        }
    }
    // If any ready instance exists without an offline receipt yet, conservatively
    // mark all existing runtime components as potentially referenced so we never
    // prune a runtime still needed by an instance that hasn't launched offline-capable yet.
    if registry.instances().iter().any(|r| {
        r.state() == InstanceState::Ready
            && !crate::launch::offline::has_valid_receipt(managed, r.id())
    }) && let Ok(entries) = fs::read_dir(managed.runtimes_dir())
    {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                referenced_components.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    (referenced_digests, referenced_components)
}

fn cache_artifact_dirs(managed: &ManagedPaths) -> [PathBuf; 3] {
    let base = managed.cache_dir().join("artifacts");
    [
        base.join("sha256"),
        base.join("sha1"),
        base.join("transport-observed"),
    ]
}

fn scan_unreferenced_cache(
    managed: &ManagedPaths,
    referenced_digests: &HashSet<String>,
) -> (u64, u64, Vec<PathBuf>) {
    let mut total = 0u64;
    let mut unreferenced = 0u64;
    let mut candidates = Vec::new();
    for dir in cache_artifact_dirs(managed) {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.is_file() || meta.file_type().is_symlink() {
                continue;
            }
            let len = meta.len();
            total = total.saturating_add(len);
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let digest = name.split('.').next().unwrap_or(&name);
            if !referenced_digests.contains(digest) {
                unreferenced = unreferenced.saturating_add(len);
                candidates.push(path);
            }
        }
    }
    (total, unreferenced, candidates)
}

fn scan_runtimes(
    managed: &ManagedPaths,
    referenced_components: &HashSet<String>,
) -> Vec<(RuntimeStorageEntry, PathBuf)> {
    let mut out = Vec::new();
    let Ok(components) = fs::read_dir(managed.runtimes_dir()) else {
        return out;
    };
    for comp_entry in components.flatten() {
        let comp_path = comp_entry.path();
        if !comp_path.is_dir() {
            continue;
        }
        let component = comp_entry.file_name().to_string_lossy().into_owned();
        let Ok(slots) = fs::read_dir(&comp_path) else {
            continue;
        };
        for slot_entry in slots.flatten() {
            let slot_path = slot_entry.path();
            if !slot_path.is_dir() {
                continue;
            }
            let slot = slot_entry.file_name().to_string_lossy().into_owned();
            // Only classify directories that carry Aurora's runtime-installed.json marker.
            if !slot_path.join("runtime-installed.json").is_file() {
                continue;
            }
            let size_bytes = dir_size(&slot_path);
            let refs = usize::from(referenced_components.contains(&component));
            out.push((
                RuntimeStorageEntry {
                    component: component.clone(),
                    slot,
                    size_bytes,
                    referenced_by_instances: refs,
                },
                slot_path,
            ));
        }
    }
    out
}

pub fn inspect_storage(managed: &ManagedPaths) -> Result<StorageOverview, StorageError> {
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .unwrap_or_else(|_| InstanceRegistry::empty());
    let (referenced_digests, referenced_components) = collect_references(managed, &registry);

    let mut instances = Vec::new();
    let mut instances_bytes = 0u64;
    for record in registry.instances() {
        let p = managed.instance_paths(record.id());
        let total_bytes = dir_size(p.root());
        let game_bytes = dir_size(p.game());
        let mods_bytes = dir_size(p.mods());
        let saves_bytes = dir_size(&p.root().join("saves"));
        let backups_bytes = dir_size(&p.root().join(".world-backups"))
            .saturating_add(dir_size(&p.root().join(".install-history")));
        instances_bytes = instances_bytes.saturating_add(total_bytes);
        instances.push(InstanceStorageEntry {
            instance_id: record.id().as_str().to_owned(),
            display_name: record.display_name().to_owned(),
            total_bytes,
            game_bytes,
            mods_bytes,
            saves_bytes,
            backups_bytes,
        });
    }
    instances.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));

    let (shared_cache_bytes, unreferenced_cache_bytes, _) =
        scan_unreferenced_cache(managed, &referenced_digests);
    let artwork_cache_bytes = dir_size(&managed.cache_dir().join("artwork"));

    let runtime_rows = scan_runtimes(managed, &referenced_components);
    let mut runtimes_bytes = 0u64;
    let mut unreferenced_runtimes_bytes = 0u64;
    let mut runtimes = Vec::new();
    for (entry, _) in runtime_rows {
        runtimes_bytes = runtimes_bytes.saturating_add(entry.size_bytes);
        if entry.referenced_by_instances == 0 {
            unreferenced_runtimes_bytes =
                unreferenced_runtimes_bytes.saturating_add(entry.size_bytes);
        }
        runtimes.push(entry);
    }

    let total_bytes = instances_bytes
        .saturating_add(shared_cache_bytes)
        .saturating_add(artwork_cache_bytes)
        .saturating_add(runtimes_bytes);

    Ok(StorageOverview {
        total_bytes,
        instances_bytes,
        shared_cache_bytes,
        unreferenced_cache_bytes,
        artwork_cache_bytes,
        runtimes_bytes,
        unreferenced_runtimes_bytes,
        instances,
        runtimes,
    })
}

pub fn prune_storage(
    managed: &ManagedPaths,
    options: &PruneOptions,
) -> Result<PruneReport, StorageError> {
    let _registry_guard = crate::instances::lifecycle::registry_lock();
    let registry = InstanceRegistry::load(&managed.instance_registry_file()).map_err(|e| {
        StorageError(
            "storage_registry_error",
            format!("Cannot prune storage while the instance registry is unreadable: {e}"),
        )
    })?;

    // Refuse pruning while any instance is installing or running.
    for record in registry.instances() {
        if record.state() == InstanceState::Installing {
            return Err(StorageError(
                "storage_prune_busy",
                "Wait for active instance installations to finish before pruning storage.".into(),
            ));
        }
        if launch::process::lock_stopped(record.id().as_str()).is_err() {
            return Err(StorageError(
                "storage_prune_busy",
                "Quit running Minecraft instances before pruning shared storage.".into(),
            ));
        }
    }

    let (referenced_digests, referenced_components) = collect_references(managed, &registry);
    let mut reclaimed_bytes = 0u64;
    let mut removed_cache_objects = 0usize;
    let mut removed_artwork_files = 0usize;
    let mut removed_runtimes = 0usize;

    if options.prune_unreferenced_cache {
        let (_, _, candidates) = scan_unreferenced_cache(managed, &referenced_digests);
        for path in candidates {
            if let Ok(meta) = fs::symlink_metadata(&path)
                && meta.is_file()
                && !meta.file_type().is_symlink()
                && fs::remove_file(&path).is_ok()
            {
                reclaimed_bytes = reclaimed_bytes.saturating_add(meta.len());
                removed_cache_objects += 1;
            }
        }
    }

    if options.prune_artwork_cache {
        let artwork_dir = managed.cache_dir().join("artwork");
        if artwork_dir.is_dir() {
            fn remove_files_recursively(dir: &Path, reclaimed: &mut u64, count: &mut usize) {
                let Ok(entries) = fs::read_dir(dir) else {
                    return;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Ok(meta) = fs::symlink_metadata(&path) else {
                        continue;
                    };
                    if meta.file_type().is_symlink() {
                        continue;
                    }
                    if meta.is_dir() {
                        remove_files_recursively(&path, reclaimed, count);
                    } else if meta.is_file() && fs::remove_file(&path).is_ok() {
                        *reclaimed = reclaimed.saturating_add(meta.len());
                        *count += 1;
                    }
                }
            }
            remove_files_recursively(
                &artwork_dir,
                &mut reclaimed_bytes,
                &mut removed_artwork_files,
            );
        }
    }

    if options.prune_unused_runtimes {
        for (entry, path) in scan_runtimes(managed, &referenced_components) {
            if entry.referenced_by_instances == 0
                && path.join("runtime-installed.json").is_file()
                && fs::remove_dir_all(&path).is_ok()
            {
                reclaimed_bytes = reclaimed_bytes.saturating_add(entry.size_bytes);
                removed_runtimes += 1;
            }
        }
    }

    Ok(PruneReport {
        reclaimed_bytes,
        removed_cache_objects,
        removed_artwork_files,
        removed_runtimes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_and_prune_removes_only_unreferenced_cache_and_runtimes() {
        let root =
            std::env::temp_dir().join(format!("aurora-storage-test-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        InstanceRegistry::empty()
            .save(&managed.instance_registry_file())
            .unwrap();

        let sha256_dir = managed.cache_dir().join("artifacts/sha256");
        fs::create_dir_all(&sha256_dir).unwrap();
        fs::write(sha256_dir.join("deadbeef"), vec![1u8; 4096]).unwrap();

        let rt_dir = managed
            .runtimes_dir()
            .join("java-runtime-gamma/windows-x64-abc");
        fs::create_dir_all(&rt_dir).unwrap();
        fs::write(rt_dir.join("runtime-installed.json"), b"{}").unwrap();
        fs::write(rt_dir.join("jvm.dll"), vec![2u8; 8192]).unwrap();

        let before = inspect_storage(&managed).unwrap();
        assert_eq!(before.unreferenced_cache_bytes, 4096);
        assert_eq!(before.runtimes.len(), 1);
        assert_eq!(before.runtimes[0].referenced_by_instances, 0);

        let report = prune_storage(
            &managed,
            &PruneOptions {
                prune_unreferenced_cache: true,
                prune_artwork_cache: false,
                prune_unused_runtimes: true,
            },
        )
        .unwrap();
        assert_eq!(report.removed_cache_objects, 1);
        assert_eq!(report.removed_runtimes, 1);
        assert!(report.reclaimed_bytes >= 4096 + 8192);

        let after = inspect_storage(&managed).unwrap();
        assert_eq!(after.unreferenced_cache_bytes, 0);
        assert!(after.runtimes.is_empty());

        let _ = fs::remove_dir_all(&root);
    }
}
