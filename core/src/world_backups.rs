//! Explicit, lock-safe local ZIP backups and atomic restores for singleplayer worlds.
//!
//! Backups live under `instances/<id>/.world-backups/<world_dir>/<backup_id>.zip`.
//! Worlds are never mutated while Minecraft is running; restores stage extraction
//! in a private sibling directory, move the existing world aside as a safety
//! copy, promote the restored tree, and only remove the temporary directory
//! after promotion succeeds.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::gameplay_history::WorldSaveId;
use crate::install::coordination::checked_directory;
use crate::instances::InstanceId;
use crate::launch;
use crate::paths::ManagedPaths;

const MAX_WORLD_FILES: usize = 50_000;
const MAX_WORLD_BYTES: u64 = 4 * 1024 * 1024 * 1024; // 4 GiB per world backup
const MAX_BACKUPS_LISTED: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldBackupSummary {
    pub id: String,
    pub world_directory: String,
    pub created_at_ms: u64,
    pub size_bytes: u64,
    pub label: Option<String>,
}

#[derive(Debug)]
pub struct WorldBackupError(pub &'static str, pub String);

impl std::fmt::Display for WorldBackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

impl std::error::Error for WorldBackupError {}

fn failed(error: impl std::fmt::Display) -> WorldBackupError {
    WorldBackupError(
        "world_backup_failed",
        format!("World backup failed: {error}"),
    )
}

fn check_no_reparse(path: &Path) -> Result<fs::Metadata, WorldBackupError> {
    let meta = fs::symlink_metadata(path).map_err(failed)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(WorldBackupError(
                "world_backup_unsafe",
                "Remove links or junctions inside the world folder before backing up or restoring."
                    .into(),
            ));
        }
    }
    if meta.file_type().is_symlink() {
        return Err(WorldBackupError(
            "world_backup_unsafe",
            "Symlinked world files cannot be backed up or restored.".into(),
        ));
    }
    Ok(meta)
}

fn backups_root(managed: &ManagedPaths, id: &InstanceId, world: &WorldSaveId) -> PathBuf {
    managed
        .instance_paths(id)
        .root()
        .join(".world-backups")
        .join(world.as_str())
}

fn valid_backup_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn sanitize_label(label: Option<&str>) -> Option<String> {
    let raw = label?.trim();
    if raw.is_empty() {
        return None;
    }
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' '))
        .take(32)
        .collect::<String>()
        .trim()
        .replace(' ', "_");
    (!cleaned.is_empty()).then_some(cleaned)
}

pub fn list_backups(
    managed: &ManagedPaths,
    id: &InstanceId,
    world: &WorldSaveId,
) -> Result<Vec<WorldBackupSummary>, WorldBackupError> {
    let dir = backups_root(managed, id, world);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    check_no_reparse(&dir)?;
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(failed)?.flatten() {
        if out.len() >= MAX_BACKUPS_LISTED {
            break;
        }
        let path = entry.path();
        let Ok(meta) = check_no_reparse(&path) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".zip") else {
            continue;
        };
        if !valid_backup_id(stem) {
            continue;
        }
        let (ts, label) = match stem.split_once('_') {
            Some((ts_str, lbl)) => (
                ts_str.parse::<u64>().unwrap_or(0),
                Some(lbl.replace('_', " ")),
            ),
            None => (stem.parse::<u64>().unwrap_or(0), None),
        };
        let created_at_ms = if ts > 0 {
            ts
        } else {
            meta.modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        };
        out.push(WorldBackupSummary {
            id: stem.to_owned(),
            world_directory: world.as_str().to_owned(),
            created_at_ms,
            size_bytes: meta.len(),
            label,
        });
    }
    out.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));
    Ok(out)
}

fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<File>,
    root: &Path,
    current: &Path,
    files: &mut usize,
    bytes: &mut u64,
) -> Result<(), WorldBackupError> {
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for entry in fs::read_dir(current).map_err(failed)? {
        let entry = entry.map_err(failed)?;
        let path = entry.path();
        let meta = check_no_reparse(&path)?;
        let rel = path
            .strip_prefix(root)
            .map_err(failed)?
            .to_string_lossy()
            .replace('\\', "/");
        if meta.is_dir() {
            add_dir_to_zip(zip, root, &path, files, bytes)?;
        } else if meta.is_file() {
            if rel.ends_with("session.lock") {
                continue;
            }
            *files += 1;
            *bytes = bytes.saturating_add(meta.len());
            if *files > MAX_WORLD_FILES || *bytes > MAX_WORLD_BYTES {
                return Err(WorldBackupError(
                    "world_backup_limit",
                    "This world exceeds the 50,000-file / 4 GiB backup limit.".into(),
                ));
            }
            zip.start_file(rel, opts).map_err(failed)?;
            let mut src = File::open(&path).map_err(failed)?;
            io::copy(&mut src, zip).map_err(failed)?;
        }
    }
    Ok(())
}

pub fn create_backup(
    managed: &ManagedPaths,
    id: &InstanceId,
    world: &WorldSaveId,
    label: Option<&str>,
) -> Result<WorldBackupSummary, WorldBackupError> {
    let _process = launch::process::lock_stopped(id.as_str()).map_err(|_| {
        WorldBackupError(
            "world_backup_busy",
            "Quit Minecraft before creating a world backup.".into(),
        )
    })?;
    create_backup_holding_lock(managed, id, world, label)
}

fn create_backup_holding_lock(
    managed: &ManagedPaths,
    id: &InstanceId,
    world: &WorldSaveId,
    label: Option<&str>,
) -> Result<WorldBackupSummary, WorldBackupError> {
    let saves = managed.instance_paths(id).root().join("saves");
    let world_dir =
        checked_directory(managed.data_root(), &saves.join(world.as_str())).map_err(|_| {
            WorldBackupError(
                "world_not_found",
                "That singleplayer world no longer exists or is redirected.".into(),
            )
        })?;

    let out_dir = backups_root(managed, id, world);
    fs::create_dir_all(&out_dir).map_err(failed)?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let clean_label = sanitize_label(label);
    let backup_id = match &clean_label {
        Some(lbl) => format!("{now_ms}_{lbl}"),
        None => format!("{now_ms}"),
    };
    let final_path = out_dir.join(format!("{backup_id}.zip"));
    let tmp_path = out_dir.join(format!(".tmp-{}.zip", uuid::Uuid::new_v4().simple()));

    let write_res = (|| -> Result<(), WorldBackupError> {
        let file = File::create(&tmp_path).map_err(failed)?;
        let mut zip = zip::ZipWriter::new(file);
        let mut files = 0usize;
        let mut bytes = 0u64;
        add_dir_to_zip(&mut zip, &world_dir, &world_dir, &mut files, &mut bytes)?;
        zip.finish().map_err(failed)?;
        Ok(())
    })();
    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
    fs::rename(&tmp_path, &final_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        failed(e)
    })?;
    let size_bytes = fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
    Ok(WorldBackupSummary {
        id: backup_id,
        world_directory: world.as_str().to_owned(),
        created_at_ms: now_ms,
        size_bytes,
        label: clean_label.map(|s| s.replace('_', " ")),
    })
}

pub fn delete_backup(
    managed: &ManagedPaths,
    id: &InstanceId,
    world: &WorldSaveId,
    backup_id: &str,
) -> Result<(), WorldBackupError> {
    if !valid_backup_id(backup_id) {
        return Err(WorldBackupError(
            "world_backup_invalid",
            "Invalid backup ID.".into(),
        ));
    }
    let path = backups_root(managed, id, world).join(format!("{backup_id}.zip"));
    check_no_reparse(&path)?;
    fs::remove_file(&path).map_err(failed)?;
    Ok(())
}

fn validate_archive_entry_path(name: &str) -> Result<PathBuf, WorldBackupError> {
    if name.is_empty() || name.starts_with('/') || name.contains('\\') || name.contains(':') {
        return Err(WorldBackupError(
            "world_backup_unsafe",
            "Backup archive contains an unsafe path.".into(),
        ));
    }
    let mut rel = PathBuf::new();
    for segment in name.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(WorldBackupError(
                "world_backup_unsafe",
                "Backup archive contains a path traversal segment.".into(),
            ));
        }
        rel.push(segment);
    }
    Ok(rel)
}

/// Restores a world backup atomically via private staging, retaining a
/// pre-restore safety backup of the current world first.
pub fn restore_backup(
    managed: &ManagedPaths,
    id: &InstanceId,
    world: &WorldSaveId,
    backup_id: &str,
) -> Result<(), WorldBackupError> {
    if !valid_backup_id(backup_id) {
        return Err(WorldBackupError(
            "world_backup_invalid",
            "Invalid backup ID.".into(),
        ));
    }
    let _process = launch::process::lock_stopped(id.as_str()).map_err(|_| {
        WorldBackupError(
            "world_backup_busy",
            "Quit Minecraft before restoring a world backup.".into(),
        )
    })?;
    let archive_path = backups_root(managed, id, world).join(format!("{backup_id}.zip"));
    check_no_reparse(&archive_path)?;

    let saves = managed.instance_paths(id).root().join("saves");
    fs::create_dir_all(&saves).map_err(failed)?;
    checked_directory(managed.data_root(), &saves).map_err(|_| {
        WorldBackupError(
            "world_backup_unsafe",
            "The saves directory is redirected.".into(),
        )
    })?;
    let target_world = saves.join(world.as_str());
    if target_world.exists() {
        check_no_reparse(&target_world)?;
        // Automatically snapshot current world before overwriting (already holding process lock).
        let _ = create_backup_holding_lock(managed, id, world, Some("pre_restore"));
    }

    let staging = managed
        .instance_paths(id)
        .root()
        .join(format!(".world-restore-{}", uuid::Uuid::new_v4().simple()));
    fs::create_dir_all(&staging).map_err(failed)?;

    let extract_res = (|| -> Result<(), WorldBackupError> {
        let file = File::open(&archive_path).map_err(failed)?;
        let mut archive = zip::ZipArchive::new(file).map_err(failed)?;
        if archive.len() > MAX_WORLD_FILES {
            return Err(WorldBackupError(
                "world_backup_limit",
                "Backup archive contains too many entries.".into(),
            ));
        }
        let mut total_bytes = 0u64;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(failed)?;
            let name = entry.name().to_owned();
            if entry.is_dir() {
                continue;
            }
            let rel = validate_archive_entry_path(&name)?;
            let dest = staging.join(rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(failed)?;
            }
            let mut out = File::create(&dest).map_err(failed)?;
            let mut limited = (&mut entry).take(MAX_WORLD_BYTES.saturating_sub(total_bytes) + 1);
            let copied = io::copy(&mut limited, &mut out).map_err(failed)?;
            total_bytes = total_bytes.saturating_add(copied);
            if total_bytes > MAX_WORLD_BYTES {
                return Err(WorldBackupError(
                    "world_backup_limit",
                    "Backup archive exceeds the safe restore size limit.".into(),
                ));
            }
        }
        Ok(())
    })();

    if let Err(err) = extract_res {
        let _ = fs::remove_dir_all(&staging);
        return Err(err);
    }

    let old_aside = managed
        .instance_paths(id)
        .root()
        .join(format!(".world-old-{}", uuid::Uuid::new_v4().simple()));
    let moved_old = if target_world.exists() {
        fs::rename(&target_world, &old_aside).map_err(|e| {
            let _ = fs::remove_dir_all(&staging);
            failed(e)
        })?;
        true
    } else {
        false
    };

    if let Err(err) = fs::rename(&staging, &target_world) {
        if moved_old {
            let _ = fs::rename(&old_aside, &target_world);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(failed(err));
    }
    if moved_old {
        let _ = fs::remove_dir_all(&old_aside);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_list_restore_and_delete_world_backup() {
        let root = std::env::temp_dir().join(format!("aurora-world-bk-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let id = crate::instances::generate_instance_id();
        let world_id = WorldSaveId::parse("MyWorld".into()).unwrap();
        let world_dir = managed.instance_paths(&id).root().join("saves/MyWorld");
        fs::create_dir_all(&world_dir).unwrap();
        fs::write(world_dir.join("level.dat"), b"original-level").unwrap();

        let created = create_backup(&managed, &id, &world_id, Some("Before Dragon")).unwrap();
        assert_eq!(created.label.as_deref(), Some("Before Dragon"));
        assert!(created.size_bytes > 0);

        let listed = list_backups(&managed, &id, &world_id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        // Mutate world and restore.
        fs::write(world_dir.join("level.dat"), b"damaged-level").unwrap();
        restore_backup(&managed, &id, &world_id, &created.id).unwrap();
        assert_eq!(
            fs::read(world_dir.join("level.dat")).unwrap(),
            b"original-level"
        );

        // Pre-restore safety backup was also created.
        assert!(list_backups(&managed, &id, &world_id).unwrap().len() >= 2);

        delete_backup(&managed, &id, &world_id, &created.id).unwrap();
        let _ = fs::remove_dir_all(&root);
    }
}
