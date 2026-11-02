//! Explicit, downstream storage maintenance. Never changes recovery authority.
use super::coordination::{checked_directory, redirected, require_regular_or_missing, unsafe_path};
use super::journal::{self, Journal, Stage};
use super::state::{INSTALLED_GAME_FILE_NAME, InstalledGameManifest, validate_relative_game_path};
use super::{InstallError, ValidationOutcome, ValidationStatus};
use crate::instances::InstanceId;
use crate::paths::ManagedPaths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const SEAL: &str = ".installation-ownership.json";
const RECEIPT: &str = "retention.json";
const GRACE_SECONDS: u64 = 30 * 24 * 60 * 60;
const MAX_BYTES: u64 = 32 * 1024 * 1024;
const MAX_HISTORY: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OwnedFile {
    pub(crate) path: String,
    size: u64,
    sha256: String,
}

fn hash(bytes: &[u8]) -> String {
    crate::integrity::ArtifactDigest::from_sha256(Sha256::digest(bytes).into()).as_hex()
}

impl OwnedFile {
    pub(crate) fn observed(path: String, bytes: &[u8]) -> Self {
        Self {
            path,
            size: bytes.len() as u64,
            sha256: hash(bytes),
        }
    }
    fn read(root: &Path, path: String) -> io::Result<Self> {
        safe_relative(&path)?;
        checked_directory(root, root.join(&path).parent().ok_or_else(unsafe_path)?)?;
        let target = root.join(&path);
        let meta = fs::symlink_metadata(&target)?;
        if redirected(&meta) || !meta.is_file() {
            return Err(unsafe_path());
        }
        let mut file = File::open(target)?;
        let mut sha = Sha256::new();
        let size = io::copy(&mut file, &mut sha)?;
        Ok(Self {
            path,
            size,
            sha256: crate::integrity::ArtifactDigest::from_sha256(sha.finalize().into()).as_hex(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Seal {
    schema_version: u32,
    installation_id: String,
    files: Vec<OwnedFile>,
}

/// Windows path aliases (ADS, trailing dots, reserved devices, case collisions)
/// are rejected in addition to installed-state's portable relative vocabulary.
fn safe_relative(path: &str) -> io::Result<()> {
    validate_relative_game_path(path).map_err(|_| unsafe_path())?;
    if path.split('/').any(|part| {
        let base = part.split('.').next().unwrap().to_ascii_lowercase();
        part.ends_with('.')
            || matches!(base.as_str(), "con" | "prn" | "aux" | "nul")
            || (base.len() == 4
                && (base.starts_with("com") || base.starts_with("lpt"))
                && base.as_bytes()[3].is_ascii_digit())
    }) {
        return Err(unsafe_path());
    }
    Ok(())
}

impl Seal {
    fn validate(&self) -> io::Result<()> {
        if self.schema_version != 1
            || !journal::safe_uuid(&self.installation_id)
            || self.files.is_empty()
            || self.files.len() > 100_000
        {
            return Err(unsafe_path());
        }
        let mut names = BTreeSet::new();
        for item in &self.files {
            safe_relative(&item.path)?;
            if item.path == SEAL
                || !names.insert(item.path.to_ascii_lowercase())
                || crate::integrity::ArtifactDigest::parse(&item.sha256).is_err()
            {
                return Err(unsafe_path());
            }
        }
        if !names.contains(INSTALLED_GAME_FILE_NAME) {
            return Err(unsafe_path());
        }
        // A file cannot also be another recorded file's directory.
        for name in &names {
            for (offset, _) in name.match_indices('/') {
                if names.contains(&name[..offset]) {
                    return Err(unsafe_path());
                }
            }
        }
        Ok(())
    }

    fn directories(&self) -> BTreeSet<String> {
        let mut directories = BTreeSet::new();
        for file in &self.files {
            for (offset, _) in file.path.match_indices('/') {
                directories.insert(file.path[..offset].to_owned());
            }
        }
        directories
    }
}

fn read_bounded(path: &Path) -> io::Result<Vec<u8>> {
    require_regular_or_missing(path)?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(unsafe_path());
    }
    Ok(bytes)
}

fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| unsafe_path())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(unsafe_path());
    }
    journal::atomic_write(path, &bytes)
}

/// Only installer-supplied outputs grant ownership. Sealing is ONE immutable
/// observation of launcher-private staging: enumerate the staged tree once,
/// hash each expected file exactly once, and refuse everything unaccounted.
/// Re-hashing or per-file re-canonicalization inside this single synchronous
/// call would add no independent evidence — the tree is private staging under
/// the game lock — while every later consumer (staged validation, promotion
/// capture, retirement inspection) re-verifies bytes at its own boundary.
pub(crate) fn seal_staging(
    root: &Path,
    manifest: &InstalledGameManifest,
    natives: Vec<OwnedFile>,
) -> io::Result<()> {
    // Expected paths and the exact in-memory observations any on-disk copy
    // must equal: extraction observations for natives, and the completion
    // manifest bytes the executor writes last.
    let mut expected: BTreeMap<String, Option<OwnedFile>> = BTreeMap::new();
    for file in manifest.files() {
        safe_relative(file.path())?;
        if expected.insert(file.path().to_owned(), None).is_some() {
            return Err(unsafe_path());
        }
    }
    for item in natives {
        safe_relative(&item.path)?;
        let observation = Some(item);
        let path = observation.as_ref().unwrap().path.clone();
        let conflicting = match expected.get(&path) {
            Some(previous @ Some(_)) => *previous != observation,
            _ => false,
        };
        if conflicting {
            return Err(unsafe_path());
        }
        expected.insert(path, observation);
    }
    // Bind the exact bytes that the installer will write LAST. The ownership
    // seal is auxiliary staging evidence, never a completion marker itself.
    // As before, this deliberately supersedes any same-path expectation.
    let completion = OwnedFile::observed(
        INSTALLED_GAME_FILE_NAME.to_owned(),
        manifest.to_json().as_bytes(),
    );
    expected.insert(INSTALLED_GAME_FILE_NAME.to_owned(), Some(completion));

    let allowed_dirs: BTreeSet<String> = expected
        .keys()
        .flat_map(|path| {
            path.match_indices('/')
                .map(|(offset, _)| path[..offset].to_owned())
        })
        .collect();
    checked_directory(root, root)?;
    // Enumeration never adopts discovered files: every observed path must
    // already be installer-supplied, and directory metadata comes from the
    // enumeration itself so no descendant is followed through a redirect.
    let mut observed: BTreeMap<String, OwnedFile> = BTreeMap::new();
    let mut pending: Vec<(PathBuf, String, usize)> = vec![(root.to_path_buf(), String::new(), 0)];
    let mut count = 0usize;
    while let Some((directory, prefix, depth)) = pending.pop() {
        if depth > 64 {
            return Err(unsafe_path());
        }
        for entry in fs::read_dir(&directory)? {
            count += 1;
            if count > 200_000 {
                return Err(unsafe_path());
            }
            let entry = entry?;
            let name = entry.file_name().into_string().map_err(|_| unsafe_path())?;
            let relative = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            safe_relative(&relative)?;
            let meta = entry.metadata()?;
            if redirected(&meta) {
                return Err(unsafe_path());
            }
            if meta.is_dir() {
                if !allowed_dirs.contains(&relative) {
                    return Err(unsafe_path());
                }
                pending.push((entry.path(), relative, depth + 1));
            } else if meta.is_file() {
                // A stale seal from an interrupted attempt is never trusted.
                if relative == SEAL {
                    return Err(unsafe_path());
                }
                let (size, sha256) = observe_bytes(&entry.path())?;
                let item = OwnedFile {
                    path: relative.clone(),
                    size,
                    sha256,
                };
                match expected.get(&relative) {
                    // On-disk natives/completion bytes must equal the exact
                    // installer-supplied observation.
                    Some(Some(wanted)) if *wanted == item => {
                        observed.insert(relative.clone(), wanted.clone());
                    }
                    // Manifest-recorded files adopt this one fresh reading.
                    Some(None) => {
                        observed.insert(relative.clone(), item);
                    }
                    _ => return Err(unsafe_path()),
                }
            } else {
                return Err(unsafe_path());
            }
        }
    }
    // Everything the installer claims to have staged must have been present.
    // The completion manifest is the one entry bound ahead of its write.
    for path in expected.keys() {
        if path != INSTALLED_GAME_FILE_NAME && !observed.contains_key(path) {
            return Err(unsafe_path());
        }
    }
    let completion = expected
        .get(INSTALLED_GAME_FILE_NAME)
        .cloned()
        .flatten()
        .ok_or_else(unsafe_path)?;
    observed.insert(INSTALLED_GAME_FILE_NAME.to_owned(), completion);
    let seal = Seal {
        schema_version: 1,
        installation_id: manifest.installation_id().to_owned(),
        files: observed.into_values().collect(),
    };
    seal.validate()?;
    write_json(&root.join(SEAL), &seal)
}

/// Open and hash one already-validated regular file without re-walking its
/// ancestors; the caller's enumeration supplied the containment evidence.
fn observe_bytes(path: &Path) -> io::Result<(u64, String)> {
    let mut file = File::open(path)?;
    let mut sha = Sha256::new();
    let size = io::copy(&mut file, &mut sha)?;
    Ok((
        size,
        crate::integrity::ArtifactDigest::from_sha256(sha.finalize().into()).as_hex(),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Retirement {
    Retained,
    Deleting,
    Retired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    journal: Journal,
    completed_at_unix_seconds: u64,
    state: Retirement,
    seal: Seal,
    seal_sha256: String,
}

impl Receipt {
    fn validate(&self, journal: &Journal) -> io::Result<()> {
        self.seal.validate()?;
        let previous = journal.previous.as_ref().ok_or_else(unsafe_path)?;
        let manifest = self
            .seal
            .files
            .iter()
            .find(|item| item.path == INSTALLED_GAME_FILE_NAME)
            .ok_or_else(unsafe_path)?;
        if self.schema_version != 1
            || &self.journal != journal
            || journal.stage != Stage::Committed
            || !previous.valid
            || self.seal.installation_id != previous.installation_id
            || manifest.sha256 != previous.manifest_sha256
            || self.completed_at_unix_seconds == 0
            || self.seal_sha256 != hash(&serde_json::to_vec(&self.seal).map_err(|_| unsafe_path())?)
        {
            return Err(unsafe_path());
        }
        Ok(())
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

/// Best effort immediately AFTER durable commit/archive. Recovery does not
/// manufacture a missing completion timestamp or enroll a legacy backup.
pub(crate) fn record_completed(root: &Path, journal: &Journal) -> io::Result<()> {
    if journal.stage != Stage::Committed || !journal.previous.as_ref().is_some_and(|old| old.valid)
    {
        return Ok(());
    }
    let slot = root
        .join(journal::HISTORY_DIR)
        .join(&journal.transaction_id);
    checked_directory(root, &slot)?;
    let backup = slot.join("previous-game");
    checked_directory(root, &backup)?;
    let bytes = read_bounded(&backup.join(SEAL))?;
    let seal: Seal = serde_json::from_slice(&bytes).map_err(|_| unsafe_path())?;
    seal.validate()?;
    // The original install's committed candidate binds the staged seal and
    // complete tree. A later replacement must never bless a modified seal or
    // owner additions by merely taking a fresh recovery snapshot of them.
    prove_original(root, journal, &seal)?;
    let receipt = Receipt {
        schema_version: 1,
        journal: journal.clone(),
        completed_at_unix_seconds: now(),
        state: Retirement::Retained,
        seal,
        seal_sha256: hash(&bytes),
    };
    receipt.validate(journal)?;
    // Never overwrite uncertain pre-existing metadata.
    if fs::symlink_metadata(slot.join(RECEIPT)).is_ok() {
        return Err(unsafe_path());
    }
    write_json(&slot.join(RECEIPT), &receipt)
}

fn prove_original(root: &Path, replacement: &Journal, seal: &Seal) -> io::Result<()> {
    seal.validate()?;
    let origin_slot = root.join(journal::HISTORY_DIR).join(&seal.installation_id);
    checked_directory(root, &origin_slot)?;
    let origin =
        journal::read_document(&origin_slot.join("journal.json"), &replacement.instance_id)
            .map_err(|_| unsafe_path())?
            .ok_or_else(unsafe_path)?;
    if origin.stage != Stage::Committed
        || origin.transaction_id != seal.installation_id
        || origin.candidate != replacement.previous
    {
        return Err(unsafe_path());
    }
    Ok(())
}

/// All observed descendants must be recorded. Verify the WHOLE remaining tree
/// before deleting any leaf. Missing leaves are allowed only by durable intent.
/// Returns exact regular bytes and recorded directories currently present.
fn inspect(
    root: &Path,
    seal: &Seal,
    seal_hash: Option<&str>,
    partial: bool,
) -> io::Result<(u64, Vec<PathBuf>)> {
    checked_directory(root, root)?;
    seal.validate()?;
    let expected: BTreeMap<_, _> = seal
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let allowed_dirs = seal.directories();
    let mut pending = vec![(root.to_path_buf(), 0)];
    let mut directories = Vec::new();
    let mut found = BTreeSet::new();
    let mut bytes = 0u64;
    let mut count = 0;
    while let Some((directory, depth)) = pending.pop() {
        if depth > 64 {
            return Err(unsafe_path());
        }
        checked_directory(root, &directory)?;
        directories.push(directory.clone());
        for entry in fs::read_dir(directory)? {
            count += 1;
            if count > 200_000 {
                return Err(unsafe_path());
            }
            let entry = entry?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| unsafe_path())?
                .to_str()
                .ok_or_else(unsafe_path)?
                .replace('\\', "/");
            safe_relative(&relative)?;
            let meta = fs::symlink_metadata(&path)?;
            if redirected(&meta) {
                return Err(unsafe_path());
            }
            if meta.is_dir() {
                if !allowed_dirs.contains(&relative) {
                    return Err(unsafe_path());
                }
                pending.push((path, depth + 1));
            } else if meta.is_file() {
                if relative == SEAL {
                    if seal_hash != Some(hash(&read_bounded(&path)?).as_str()) {
                        return Err(unsafe_path());
                    }
                } else {
                    let item = expected.get(relative.as_str()).ok_or_else(unsafe_path)?;
                    if &OwnedFile::read(root, relative.clone())? != *item {
                        return Err(unsafe_path());
                    }
                }
                bytes = bytes.checked_add(meta.len()).ok_or_else(unsafe_path)?;
                found.insert(relative);
            } else {
                return Err(unsafe_path());
            }
        }
    }
    if !partial
        && (expected.keys().any(|name| !found.contains(*name))
            || (seal_hash.is_some() && !found.contains(SEAL)))
    {
        return Err(unsafe_path());
    }
    // Directories derive only from known files. Unknown EMPTY directories also
    // refuse retirement; missing expected directories in a partial pass are OK.
    Ok((bytes, directories))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    RetainedByPolicy,
    RetainedActiveTransaction,
    RetainedRecovery,
    RetainedManualRepair,
    RetainedUnknownOwnership,
    RetiredSuccessfully,
    CleanupInterrupted,
    CleanupFailedSafely,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryOutcome {
    pub transaction_id: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionReport {
    pub entries: Vec<EntryOutcome>,
    pub eligible_backups: usize,
    pub protected_backups: usize,
    pub retired_backups: usize,
    pub verified_backup_bytes_before: u64,
    pub reclaimed_bytes: u64,
    pub verified_backup_bytes_after: u64,
    pub validation_ms: u128,
    pub elapsed_ms: u128,
    pub skipped: Option<Outcome>,
}

/// Explicit API: never runs at startup. Both guards live through all decisions,
/// validation, rename, per-file verification/deletion and final accounting.
pub fn maintain(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<RetentionReport, InstallError> {
    let _lifecycle = crate::instances::lifecycle::lock_instance_operation(managed, instance)?;
    let guard = super::lock_game_target(managed, instance)?;
    guard
        .recheck(managed.data_root(), managed.instance_paths(instance).root())
        .map_err(InstallError::Storage)?;
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|_| InstallError::RecoveryRequired {
            reason: "backup maintenance requires a readable instance registry".into(),
        })?;
    if !registry
        .find(instance)
        .is_some_and(|record| record.state() == crate::instances::InstanceState::Ready)
    {
        return Ok(RetentionReport {
            skipped: Some(Outcome::RetainedRecovery),
            ..Default::default()
        });
    }
    if journal::needs_recovery(managed.instance_paths(instance).root(), instance).unwrap_or(true) {
        return Ok(RetentionReport {
            skipped: Some(Outcome::RetainedRecovery),
            ..Default::default()
        });
    }
    let validation_started = Instant::now();
    let validation = crate::instances::lifecycle::validate_instance_under_game_lock(
        managed, &registry, instance,
    );
    let validation_ms = validation_started.elapsed().as_millis();
    if !validation
        .is_ok_and(|result| result.status == crate::instances::lifecycle::InstanceStatus::Ready)
    {
        return Ok(RetentionReport {
            skipped: Some(Outcome::RetainedRecovery),
            validation_ms,
            ..Default::default()
        });
    }
    Ok(maintain_validated(
        managed.instance_paths(instance).root(),
        instance,
        now(),
        Some(validation_ms),
        |point| {
            #[cfg(all(debug_assertions, feature = "f3ra-desktop-acceptance"))]
            crate::desktop_acceptance::retention_checkpoint(
                managed.instance_paths(instance).root(),
                &format!("Retention{point:?}"),
            )
            .map_err(|_| io::Error::other("desktop retention checkpoint interrupted"))?;
            let _ = point;
            Ok(())
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Point {
    Intent,
    Renamed,
    FileRemoved,
    Empty,
    Finished,
}

#[cfg(test)]
fn maintain_locked(
    root: &Path,
    instance: &InstanceId,
    time: u64,
    checkpoint: impl FnMut(Point) -> io::Result<()>,
) -> RetentionReport {
    maintain_validated(root, instance, time, None, checkpoint)
}

fn maintain_validated(
    root: &Path,
    instance: &InstanceId,
    time: u64,
    validation_ms: Option<u128>,
    mut checkpoint: impl FnMut(Point) -> io::Result<()>,
) -> RetentionReport {
    let started = Instant::now();
    let mut report = RetentionReport::default();
    if let Err(reason) = assess_and_retire(
        root,
        instance,
        time,
        validation_ms,
        &mut checkpoint,
        &mut report,
    ) {
        report.skipped = Some(reason);
    }
    report.verified_backup_bytes_after = report
        .verified_backup_bytes_before
        .saturating_sub(report.reclaimed_bytes);
    report.elapsed_ms = started.elapsed().as_millis();
    eprintln!(
        "[aurora-retention] eligible={} protected={} retired={} reclaimed_bytes={} elapsed_ms={} skipped={:?}",
        report.eligible_backups,
        report.protected_backups,
        report.retired_backups,
        report.reclaimed_bytes,
        report.elapsed_ms,
        report.skipped
    );
    report
}

fn assess_and_retire(
    root: &Path,
    instance: &InstanceId,
    time: u64,
    validated_ms: Option<u128>,
    checkpoint: &mut impl FnMut(Point) -> io::Result<()>,
    report: &mut RetentionReport,
) -> Result<(), Outcome> {
    checked_directory(root, root).map_err(|_| Outcome::RetainedUnknownOwnership)?;
    // Classify only. Recovery must happen through its own existing workflow.
    if journal::needs_recovery(root, instance).map_err(|_| Outcome::RetainedManualRepair)? {
        return Err(Outcome::RetainedRecovery);
    }
    journal::readiness_gate(root, instance).map_err(|_| Outcome::RetainedManualRepair)?;
    let active = Journal::load(root, instance)
        .map_err(|_| Outcome::RetainedManualRepair)?
        .ok_or(Outcome::RetainedUnknownOwnership)?;
    if active.stage != Stage::Committed {
        return Err(Outcome::RetainedRecovery);
    }
    let validation = Instant::now();
    let valid = validated_ms.is_some()
        || matches!(super::validate_game_directory(&root.join("game")), Ok(ValidationOutcome::Installed(ref result)) if result.status == ValidationStatus::Valid);
    report.validation_ms = validated_ms.unwrap_or_else(|| validation.elapsed().as_millis());
    if !valid {
        return Err(Outcome::RetainedRecovery);
    }
    let history = root.join(journal::HISTORY_DIR);
    checked_directory(root, &history).map_err(|_| Outcome::RetainedUnknownOwnership)?;
    let mut candidates = Vec::new();
    // Finish ALL history classification before the first mutation. Unknown or
    // incomplete history can be unresolved recovery evidence: skip the instance.
    for (index, entry) in fs::read_dir(&history)
        .map_err(|_| Outcome::CleanupFailedSafely)?
        .enumerate()
    {
        if index >= MAX_HISTORY {
            return Err(Outcome::RetainedUnknownOwnership);
        }
        let slot = entry.map_err(|_| Outcome::CleanupFailedSafely)?.path();
        checked_directory(root, &slot).map_err(|_| Outcome::RetainedUnknownOwnership)?;
        let id = slot
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| journal::safe_uuid(name))
            .ok_or(Outcome::RetainedUnknownOwnership)?
            .to_owned();
        let history_journal = journal::read_document(&slot.join("journal.json"), instance)
            .map_err(|_| Outcome::RetainedManualRepair)?
            .ok_or(Outcome::RetainedManualRepair)?;
        if history_journal.transaction_id != id || !history_journal.stage.terminal() {
            return Err(Outcome::RetainedManualRepair);
        }
        let tree_count = ["previous-game", "rejected-game", "retiring-game"]
            .into_iter()
            .filter(|name| fs::symlink_metadata(slot.join(name)).is_ok())
            .count();
        if tree_count == 0
            && !exists(&slot.join(RECEIPT)).map_err(|_| Outcome::RetainedUnknownOwnership)?
        {
            continue; // Terminal evidence alone is not a retained backup.
        }
        if id == active.transaction_id {
            report.entries.push(EntryOutcome {
                transaction_id: id,
                outcome: Outcome::RetainedActiveTransaction,
            });
            report.protected_backups += tree_count;
            continue;
        }
        // Metadata and children must all be recognizable. Failed attempts and
        // rejected candidates deliberately never enter successful retention.
        let allowed = ["journal.json", RECEIPT, "previous-game", "retiring-game"];
        let clean = fs::read_dir(&slot)
            .map_err(|_| Outcome::CleanupFailedSafely)?
            .all(|entry| {
                entry.ok().is_some_and(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| allowed.contains(&name))
                })
            });
        let receipt = read_bounded(&slot.join(RECEIPT))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Receipt>(&bytes).ok())
            .filter(|receipt| {
                receipt.validate(&history_journal).is_ok()
                    && prove_original(root, &history_journal, &receipt.seal).is_ok()
            });
        if !clean || receipt.is_none() {
            report.protected_backups += tree_count;
            report.entries.push(EntryOutcome {
                transaction_id: id,
                outcome: if history_journal.stage != Stage::Committed {
                    Outcome::RetainedRecovery
                } else {
                    Outcome::RetainedUnknownOwnership
                },
            });
            continue;
        }
        candidates.push((slot, receipt.unwrap()));
    }
    // A newer damaged backup cannot evict the last valid historical backup.
    let mut validated = Vec::new();
    for (slot, receipt) in candidates {
        if receipt.state == Retirement::Retained {
            let backup = slot.join("previous-game");
            let valid = checked_directory(root, &backup).is_ok()
                && journal::matches_evidence(&backup, receipt.journal.previous.as_ref(), true)
                    .unwrap_or(false)
                && inspect(&backup, &receipt.seal, Some(&receipt.seal_sha256), false).is_ok();
            if !valid {
                report.protected_backups += 1;
                report.entries.push(EntryOutcome {
                    transaction_id: receipt.journal.transaction_id,
                    outcome: Outcome::RetainedUnknownOwnership,
                });
                continue;
            }
        }
        validated.push((slot, receipt));
    }
    // A partial retirement is no longer a rollback candidate. Equal timestamps
    // retain ALL ties: UUID ordering must never invent completion chronology.
    let newest = validated
        .iter()
        .filter(|(_, receipt)| receipt.state == Retirement::Retained)
        .map(|(_, receipt)| receipt.completed_at_unix_seconds)
        .max();
    for (slot, mut receipt) in validated {
        let protected = receipt.state == Retirement::Retained
            && (Some(receipt.completed_at_unix_seconds) == newest
                || time
                    .checked_sub(receipt.completed_at_unix_seconds)
                    .is_none_or(|age| age < GRACE_SECONDS));
        let result = (|| -> io::Result<Outcome> {
            let backup = slot.join("previous-game");
            let retiring = slot.join("retiring-game");
            let backup_exists = exists(&backup)?;
            let retiring_exists = exists(&retiring)?;
            if receipt.state == Retirement::Retired {
                return if backup_exists || retiring_exists {
                    Err(unsafe_path())
                } else {
                    Ok(Outcome::RetiredSuccessfully)
                };
            }
            if backup_exists && retiring_exists {
                return Err(unsafe_path());
            }
            if receipt.state == Retirement::Retained && !backup_exists {
                return Err(unsafe_path());
            }
            if backup_exists {
                checked_directory(root, &backup)?;
                if !journal::matches_evidence(&backup, receipt.journal.previous.as_ref(), true)
                    .map_err(|_| unsafe_path())?
                {
                    return Err(unsafe_path());
                }
            }
            let tree = if backup_exists { &backup } else { &retiring };
            let bytes = if exists(tree)? {
                checked_directory(root, tree)?;
                inspect(
                    tree,
                    &receipt.seal,
                    Some(&receipt.seal_sha256),
                    receipt.state == Retirement::Deleting,
                )?
                .0
            } else if receipt.state == Retirement::Deleting {
                0
            } else {
                return Err(unsafe_path());
            };
            report.verified_backup_bytes_before += bytes;
            if protected {
                return Ok(Outcome::RetainedByPolicy);
            }
            report.eligible_backups += 1;
            if receipt.state == Retirement::Retained {
                receipt.state = Retirement::Deleting;
                write_json(&slot.join(RECEIPT), &receipt)?;
                checkpoint(Point::Intent)?;
            }
            if backup_exists {
                // Exact same slot/volume; no rename-overwrite or copy fallback.
                checked_directory(root, &slot)?;
                if exists(&retiring)? {
                    return Err(unsafe_path());
                }
                fs::rename(&backup, &retiring)?;
                journal::sync_directory(&slot)?;
                checkpoint(Point::Renamed)?;
            }
            if exists(&retiring)? {
                let (_, mut directories) =
                    inspect(&retiring, &receipt.seal, Some(&receipt.seal_sha256), true)?;
                for item in &receipt.seal.files {
                    let path = retiring.join(&item.path);
                    if exists(&path)? {
                        checked_directory(root, &retiring)?;
                        if OwnedFile::read(&retiring, item.path.clone())? != *item {
                            return Err(unsafe_path());
                        }
                        fs::remove_file(&path)?;
                        report.reclaimed_bytes += item.size;
                        journal::sync_directory(path.parent().unwrap())?;
                        checkpoint(Point::FileRemoved)?;
                    }
                }
                let seal_path = retiring.join(SEAL);
                if exists(&seal_path)? {
                    checked_directory(root, &retiring)?;
                    let bytes = read_bounded(&seal_path)?;
                    if hash(&bytes) != receipt.seal_sha256 {
                        return Err(unsafe_path());
                    }
                    fs::remove_file(&seal_path)?;
                    report.reclaimed_bytes += bytes.len() as u64;
                    journal::sync_directory(&retiring)?;
                }
                directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
                for directory in directories {
                    checked_directory(root, &directory)?;
                    // remove_dir refuses nonempty; it never recursively deletes.
                    fs::remove_dir(&directory)?;
                    journal::sync_directory(directory.parent().unwrap())?;
                }
                checkpoint(Point::Empty)?;
            }
            receipt.state = Retirement::Retired;
            write_json(&slot.join(RECEIPT), &receipt)?;
            checkpoint(Point::Finished)?;
            if exists(&backup)? || exists(&retiring)? {
                return Err(unsafe_path());
            }
            report.retired_backups += 1;
            Ok(Outcome::RetiredSuccessfully)
        })();
        let outcome = result.unwrap_or(if receipt.state == Retirement::Deleting {
            Outcome::CleanupInterrupted
        } else {
            Outcome::CleanupFailedSafely
        });
        if outcome != Outcome::RetiredSuccessfully {
            report.protected_backups += 1;
        }
        report.entries.push(EntryOutcome {
            transaction_id: receipt.journal.transaction_id,
            outcome,
        });
    }
    Ok(())
}

fn exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
