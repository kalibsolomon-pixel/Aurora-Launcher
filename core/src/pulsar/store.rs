//! Atomic, launcher-owned import index. Source recordings are always read-only here.
use super::{Error, parse, view::*, wire::FightRecord};
use crate::{instances::InstanceId, paths::ManagedPaths};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const INDEX_LIMIT: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;
const MAX_ISSUES: usize = 128;

#[derive(Debug, Clone)]
pub(crate) struct Source {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Entry {
    pub key: FightKey,
    pub provenance: Provenance,
    pub record: FightRecord,
    pub bytes: u64,
    pub event_count: usize,
    pub legacy: bool,
    pub stale: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Index {
    pub schema_version: u32,
    pub revision: u64,
    pub entries: Vec<Entry>,
    pub tombstones: Vec<FightKey>,
    pub issues: BTreeMap<String, Vec<FileIssue>>,
}
impl Default for Index {
    fn default() -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            entries: Vec::new(),
            tombstones: Vec::new(),
            issues: BTreeMap::new(),
        }
    }
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn library_root(paths: &ManagedPaths) -> PathBuf {
    paths.launcher_dir().join("pulsar")
}
pub(crate) fn instance_root(paths: &ManagedPaths, id: &str) -> Result<PathBuf, Error> {
    let id = InstanceId::new(id)
        .map_err(|_| Error::new("pulsar_invalid_instance", "Invalid instance identity."))?;
    Ok(paths.instance_paths(&id).root().to_path_buf())
}
pub(crate) fn library_file(paths: &ManagedPaths, key: &FightKey) -> Result<PathBuf, Error> {
    InstanceId::new(&key.instance_id)
        .map_err(|_| Error::new("pulsar_invalid_instance", "Invalid instance identity."))?;
    parse::canonical_id(&key.fight_id)?;
    Ok(library_root(paths)
        .join("library")
        .join(&key.instance_id)
        .join(format!("{}.json", key.fight_id)))
}
fn version_file(paths: &ManagedPaths, key: &FightKey, hash: &str) -> Result<PathBuf, Error> {
    library_file(paths, key)?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(Error::new(
            "pulsar_index_invalid",
            "Invalid library content digest.",
        ));
    }
    Ok(library_root(paths)
        .join("versions")
        .join(&key.instance_id)
        .join(&key.fight_id)
        .join(format!("{hash}.json")))
}

pub(crate) fn read_index(paths: &ManagedPaths) -> Result<Index, Error> {
    let path = library_root(paths).join("index.json");
    if !path.try_exists()? {
        return Ok(Index::default());
    }
    let bytes = parse::read_bytes(paths.data_root(), &path, INDEX_LIMIT)?;
    let index: Index = serde_json::from_slice(&bytes).map_err(|_| {
        Error::new(
            "pulsar_index_invalid",
            "The library index is damaged; it was preserved.",
        )
    })?;
    if index.schema_version != 1 {
        return Err(Error::new(
            "pulsar_index_schema",
            "Unsupported library index version; it was preserved.",
        ));
    }
    if index.entries.len() > MAX_ENTRIES || index.tombstones.len() > MAX_ENTRIES {
        return Err(Error::new(
            "pulsar_index_limit",
            "The library index exceeds its bounded capacity.",
        ));
    }
    let mut keys = BTreeSet::new();
    for e in &index.entries {
        version_file(paths, &e.key, &e.provenance.digest)?;
        if e.key.fight_id != e.record.id
            || !keys.insert((e.key.instance_id.clone(), e.key.fight_id.clone()))
        {
            return Err(Error::new(
                "pulsar_index_invalid",
                "Duplicate or inconsistent library identity.",
            ));
        }
        if e.record.policy.is_none() || e.record.match_evidence.is_none() {
            return Err(Error::new(
                "pulsar_index_invalid",
                "Incomplete normalized library summary.",
            ));
        }
    }
    for key in &index.tombstones {
        library_file(paths, key)?;
    }
    Ok(index)
}

pub(crate) fn atomic_write(paths: &ManagedPaths, path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("pulsar_unsafe_path", "No destination directory."))?;
    crate::install::coordination::ensure_directory(paths.data_root(), parent)?;
    crate::install::coordination::require_regular_or_missing(path)?;
    let temp = parent.join(format!(".pulsar-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        parse::safe_path(paths.data_root(), parent)?;
        crate::install::coordination::require_regular_or_missing(path)?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
fn write_index(paths: &ManagedPaths, index: &Index) -> Result<(), Error> {
    let bytes = serde_json::to_vec(index).map_err(|_| {
        Error::new(
            "pulsar_index_invalid",
            "The library index could not be encoded.",
        )
    })?;
    if bytes.len() as u64 > INDEX_LIMIT {
        return Err(Error::new(
            "pulsar_index_limit",
            "The library index exceeds its byte budget.",
        ));
    }
    atomic_write(paths, &library_root(paths).join("index.json"), &bytes)
}
fn issue(issues: &mut Vec<FileIssue>, path: &Path, e: Error) {
    if issues.len() < MAX_ISSUES {
        issues.push(FileIssue {
            path: path.to_string_lossy().into_owned(),
            code: e.code.into(),
            message: e.message,
        });
    }
}

/// Walk only files directly in fights/ or one named child directory. Never follow links.
fn candidates(source: &Source, issues: &mut Vec<FileIssue>) -> Vec<(PathBuf, bool, u64)> {
    let mut files = Vec::new();
    let mut examined = 0;
    for (root_name, legacy) in [("phenomena", false), ("kwamster", true)] {
        let root = source.root.join(root_name).join("fights");
        if !root.exists() {
            continue;
        }
        let mut pending = vec![(root, 0)];
        while let Some((folder, depth)) = pending.pop() {
            if let Err(e) = parse::safe_path(&source.root, &folder) {
                issue(issues, &folder, e);
                continue;
            }
            let entries = match fs::read_dir(&folder) {
                Ok(e) => e,
                Err(e) => {
                    issue(issues, &folder, e.into());
                    continue;
                }
            };
            for entry in entries {
                examined += 1;
                // Bound non-json paths as well as candidates so junk cannot exhaust a scan.
                if examined > 10_000 || files.len() >= parse::MAX_CANDIDATES {
                    issue(
                        issues,
                        &folder,
                        Error::new(
                            "pulsar_scan_limit",
                            "The discovery budget was reached; retained library history remains available.",
                        ),
                    );
                    return files;
                }
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        issue(issues, &folder, e.into());
                        continue;
                    }
                };
                let path = entry.path();
                let meta = match fs::symlink_metadata(&path) {
                    Ok(m) => m,
                    Err(e) => {
                        issue(issues, &path, e.into());
                        continue;
                    }
                };
                if crate::install::coordination::redirected(&meta) {
                    issue(
                        issues,
                        &path,
                        Error::new("pulsar_unsafe_path", "Link/reparse entry ignored."),
                    );
                    continue;
                }
                if meta.is_dir() {
                    if depth == 0 {
                        pending.push((path, 1));
                    }
                    continue;
                }
                if !meta.is_file() || path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
                    .unwrap_or(0);
                files.push((path, legacy, modified));
            }
        }
    }
    files.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    files
}

fn preferred(new: &Entry, old: &Entry) -> bool {
    (
        !new.legacy,
        new.event_count,
        new.record.schema_version,
        new.provenance.modified_epoch_millis,
    ) > (
        !old.legacy,
        old.event_count,
        old.record.schema_version,
        old.provenance.modified_epoch_millis,
    ) || ((
        !new.legacy,
        new.event_count,
        new.record.schema_version,
        new.provenance.modified_epoch_millis,
    ) == (
        !old.legacy,
        old.event_count,
        old.record.schema_version,
        old.provenance.modified_epoch_millis,
    ) && new.provenance.source_path < old.provenance.source_path)
}

pub(crate) fn refresh(paths: &ManagedPaths, sources: &[Source]) -> Result<Index, Error> {
    let root = library_root(paths);
    let _guard = crate::install::coordination::TargetLock::acquire(
        paths.data_root(),
        &root,
        ".library.lock",
    )
    .map_err(|_| Error::new("pulsar_busy", "Another library transaction is in progress."))?;
    let mut index = read_index(paths)?;
    for source in sources {
        InstanceId::new(&source.id)
            .map_err(|_| Error::new("pulsar_invalid_instance", "Invalid source instance."))?;
        let mut issues = Vec::new();
        let files = candidates(source, &mut issues);
        for e in index
            .entries
            .iter_mut()
            .filter(|e| e.key.instance_id == source.id)
        {
            e.stale = false;
        }
        for (path, legacy, modified) in files {
            let (mut record, bytes) = match parse::read_fight(&source.root, &path) {
                Ok(f) => f,
                Err(e) => {
                    for entry in &mut index.entries {
                        if entry.provenance.source_path == path.to_string_lossy() {
                            entry.stale = true;
                        }
                    }
                    issue(&mut issues, &path, e);
                    continue;
                }
            };
            let key = FightKey {
                instance_id: source.id.clone(),
                fight_id: record.id.clone(),
            };
            if index.tombstones.contains(&key) {
                continue;
            }
            let hash = digest(&bytes);
            let event_count = record.events.len();
            record.summary_only();
            let mut entry = Entry {
                key: key.clone(),
                provenance: Provenance {
                    instance_name: source.name.clone(),
                    source_path: path.to_string_lossy().into_owned(),
                    source_root: source
                        .root
                        .join(if legacy { "kwamster" } else { "phenomena" })
                        .to_string_lossy()
                        .into_owned(),
                    modified_epoch_millis: modified,
                    digest: hash.clone(),
                    imported_epoch_millis: now(),
                    conflicts: Vec::new(),
                },
                record,
                bytes: bytes.len() as u64,
                event_count,
                legacy,
                stale: false,
            };
            let old = index.entries.iter().position(|e| e.key == key);
            if let Some(i) = old {
                let previous = &mut index.entries[i];
                if previous.provenance.digest == hash {
                    if preferred(&entry, previous)
                        || previous.provenance.source_path == entry.provenance.source_path
                    {
                        entry.provenance.imported_epoch_millis =
                            previous.provenance.imported_epoch_millis;
                        entry.provenance.conflicts = previous.provenance.conflicts.clone();
                        *previous = entry;
                    }
                    continue;
                }
                let conflict = format!("{} [{}]", entry.provenance.source_path, hash);
                issue(
                    &mut issues,
                    &path,
                    Error::new(
                        "pulsar_conflict",
                        "Distinct valid snapshots share this fight UUID; one deterministic snapshot is counted.",
                    ),
                );
                // Preserve every distinct byte snapshot, including losing conflicts.
                atomic_write(paths, &version_file(paths, &key, &hash)?, &bytes)?;
                if !preferred(&entry, previous) {
                    if previous.provenance.conflicts.len() < 64
                        && !previous.provenance.conflicts.contains(&conflict)
                    {
                        previous.provenance.conflicts.push(conflict);
                    }
                    continue;
                }
                entry.provenance.conflicts = previous.provenance.conflicts.clone();
                let old_conflict = format!(
                    "{} [{}]",
                    previous.provenance.source_path, previous.provenance.digest
                );
                if entry.provenance.conflicts.len() < 64
                    && !entry.provenance.conflicts.contains(&old_conflict)
                {
                    entry.provenance.conflicts.push(old_conflict);
                }
            } else if index.entries.len() >= MAX_ENTRIES {
                issue(
                    &mut issues,
                    &path,
                    Error::new("pulsar_index_limit", "Library capacity reached."),
                );
                continue;
            }
            atomic_write(paths, &version_file(paths, &key, &hash)?, &bytes)?;
            atomic_write(paths, &library_file(paths, &key)?, &bytes)?;
            if let Some(i) = old {
                index.entries[i] = entry;
            } else {
                index.entries.push(entry);
            }
        }
        index.issues.insert(source.id.clone(), issues);
    }
    index.revision = index.revision.saturating_add(1);
    write_index(paths, &index)?;
    Ok(index)
}

pub(crate) fn entry<'a>(index: &'a Index, key: &FightKey) -> Result<&'a Entry, Error> {
    index.entries.iter().find(|e| e.key == *key).ok_or_else(|| {
        Error::new(
            "pulsar_not_found",
            "The fight is not in the launcher library.",
        )
    })
}
pub(crate) fn load_record(paths: &ManagedPaths, entry: &Entry) -> Result<FightRecord, Error> {
    // Immutable digest snapshots make a crash between the per-file atomic writes recoverable.
    let path = version_file(paths, &entry.key, &entry.provenance.digest)?;
    let bytes = parse::read_bytes(paths.data_root(), &path, parse::MAX_FILE_BYTES)?;
    if digest(&bytes) != entry.provenance.digest {
        return Err(Error::new(
            "pulsar_digest_mismatch",
            "The imported recording changed; retained summary remains available.",
        ));
    }
    parse::decode(&bytes)
}

pub(crate) fn delete(
    paths: &ManagedPaths,
    request: &DeleteRequest,
) -> Result<ActionReceipt, Error> {
    if !request.confirmed {
        return Err(Error::new(
            "pulsar_confirmation_required",
            "Confirm deletion of the selected fight.",
        ));
    }
    let root = library_root(paths);
    let _guard = crate::install::coordination::TargetLock::acquire(
        paths.data_root(),
        &root,
        ".library.lock",
    )
    .map_err(|_| Error::new("pulsar_busy", "Another library transaction is in progress."))?;
    let mut index = read_index(paths)?;
    let selected = entry(&index, &request.key)?.clone();
    let instance = instance_root(paths, &request.key.instance_id)?;
    let _stopped = if request.also_delete_from_instance {
        Some(
            crate::launch::process::lock_stopped(&request.key.instance_id).map_err(|_| {
                Error::new(
                    "pulsar_instance_running",
                    "Stop this instance before deleting its recording.",
                )
            })?,
        )
    } else {
        None
    };
    let mut source_paths = Vec::new();
    if request.also_delete_from_instance {
        let source = Source {
            id: request.key.instance_id.clone(),
            name: selected.provenance.instance_name.clone(),
            root: instance.clone(),
        };
        let mut issues = Vec::new();
        for (path, _, _) in candidates(&source, &mut issues) {
            if path.file_stem().and_then(|s| s.to_str()) == Some(&request.key.fight_id) {
                let (record, _) = parse::read_fight(&instance, &path)?;
                if record.id == request.key.fight_id {
                    source_paths.push(path);
                }
            }
        }
        if !issues.is_empty() {
            return Err(Error::new(
                "pulsar_source_unavailable",
                "Source deletion could not safely inspect every recording; nothing was deleted.",
            ));
        }
    }
    if index.tombstones.len() >= MAX_ENTRIES {
        return Err(Error::new(
            "pulsar_index_limit",
            "Tombstone capacity reached.",
        ));
    }
    index.entries.retain(|e| e.key != request.key);
    if !index.tombstones.contains(&request.key) {
        index.tombstones.push(request.key.clone());
    }
    index.revision = index.revision.saturating_add(1);
    // Publish the tombstone first; a crash must never re-import a confirmed deletion.
    write_index(paths, &index)?;
    let main = library_file(paths, &request.key)?;
    if main.exists() {
        parse::safe_path(paths.data_root(), &main)?;
        fs::remove_file(main)?;
    }
    let versions = version_file(paths, &request.key, &selected.provenance.digest)?
        .parent()
        .unwrap()
        .to_path_buf();
    if versions.exists() {
        parse::safe_path(paths.data_root(), &versions)?;
        let mut count = 0;
        for f in fs::read_dir(&versions)? {
            count += 1;
            if count > 2000 {
                return Err(Error::new(
                    "pulsar_scan_limit",
                    "Deletion reached its version-file budget.",
                ));
            }
            let path = f?.path();
            parse::safe_path(paths.data_root(), &path)?;
            if fs::symlink_metadata(&path)?.is_file() {
                fs::remove_file(path)?;
            }
        }
        fs::remove_dir(versions)?;
    }
    for path in source_paths {
        parse::safe_path(&instance, &path)?;
        fs::remove_file(path)?;
    }
    Ok(ActionReceipt {
        revision: index.revision,
        message: "Fight deleted; reports and recovery checkpoints were retained.".into(),
        path: None,
    })
}
