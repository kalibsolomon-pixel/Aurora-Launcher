//! Durable game replacement authority, independent of disposable staging.
//! Paths are derived here; persisted bytes never supply filesystem paths.
use super::coordination::{checked_tree, ensure_directory, require_regular_or_missing};
use super::state::{INSTALLED_GAME_FILE_NAME, load_installed_state};
use super::{InstallError, ValidationOutcome, ValidationStatus};
use crate::instances::InstanceId;
use crate::integrity::ArtifactDigest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub(crate) const JOURNAL_FILE: &str = ".install-operation.json";
pub(crate) const HISTORY_DIR: &str = ".install-history";
const MAX_JOURNAL_BYTES: u64 = 16 * 1024;

/// Opt-in timings only; no change to verification, ordering, or authority.
fn profile(phase: &str, started: std::time::Instant, files: usize, bytes: u64) {
    if std::env::var_os("AURORA_INSTALL_DIAGNOSTICS").is_some() {
        eprintln!(
            "[aurora-durability] phase={phase} elapsed_us={} files={files} bytes={bytes}",
            started.elapsed().as_micros()
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Stage {
    Prepared,
    Replacing,
    Promoting,
    Promoted,
    Committed,
    Restoring,
    Restored,
    Abandoned,
}

impl Stage {
    pub(crate) fn terminal(self) -> bool {
        matches!(self, Self::Committed | Self::Restored | Self::Abandoned)
    }
}

/// Deterministic interruption boundaries; no frontend or environment control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionPoint {
    Prepared,
    BeforeBackup,
    AfterBackup,
    BeforePromotion,
    AfterPromotion,
    BeforeCommit,
    AfterCommit,
    BeforeCleanup,
    Restoring,
    AfterQuarantine,
    AfterRestore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Evidence {
    pub(crate) installation_id: String,
    pub(crate) manifest_sha256: String,
    pub(crate) tree_sha256: String,
    pub(crate) valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Journal {
    schema_version: u32,
    pub(crate) transaction_id: String,
    pub(crate) instance_id: InstanceId,
    pub(crate) stage: Stage,
    pub(crate) previous: Option<Evidence>,
    pub(crate) candidate: Option<Evidence>,
}

fn repair(reason: impl Into<String>) -> InstallError {
    InstallError::RecoveryRequired {
        reason: reason.into(),
    }
}

fn digest(bytes: &[u8]) -> String {
    ArtifactDigest::from_sha256(Sha256::digest(bytes).into()).as_hex()
}

pub(crate) fn safe_uuid(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn read_document(
    path: &Path,
    instance: &InstanceId,
) -> Result<Option<Journal>, InstallError> {
    require_regular_or_missing(path).map_err(InstallError::Storage)?;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(InstallError::Storage(error)),
    };
    let mut bytes = Vec::new();
    file.take(MAX_JOURNAL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(InstallError::Storage)?;
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(repair(
            "the installation journal exceeds its parsing bound; preserve it for manual repair",
        ));
    }
    let journal: Journal = serde_json::from_slice(&bytes).map_err(|_| {
        repair("the installation journal is malformed or unsupported; preserve all recovery trees")
    })?;
    journal.validate(instance)?;
    Ok(Some(journal))
}

impl Journal {
    pub(crate) fn load(root: &Path, instance: &InstanceId) -> Result<Option<Self>, InstallError> {
        read_document(&root.join(JOURNAL_FILE), instance)
    }

    fn validate(&self, instance: &InstanceId) -> Result<(), InstallError> {
        if self.schema_version != 1
            || &self.instance_id != instance
            || !safe_uuid(&self.transaction_id)
        {
            return Err(repair(
                "the installation journal has unsupported schema or conflicting target/transaction identity",
            ));
        }
        for evidence in [&self.previous, &self.candidate].into_iter().flatten() {
            if evidence.installation_id.is_empty()
                || evidence.installation_id.len() > 256
                || ArtifactDigest::parse(&evidence.manifest_sha256).is_err()
                || ArtifactDigest::parse(&evidence.tree_sha256).is_err()
            {
                return Err(repair(
                    "the installation journal has invalid installation evidence",
                ));
            }
        }
        if self.stage != Stage::Prepared
            && self.stage != Stage::Abandoned
            && self.candidate.is_none()
        {
            return Err(repair(
                "the installation journal lacks candidate evidence for its transition",
            ));
        }
        if self.candidate.as_ref().is_some_and(|evidence| {
            !evidence.valid || evidence.installation_id != self.transaction_id
        }) {
            return Err(repair(
                "the candidate is not a validated installation belonging to this transaction",
            ));
        }
        Ok(())
    }

    pub(crate) fn begin(root: &Path, instance: &InstanceId) -> Result<Self, InstallError> {
        check_derived_directories(root, None)?;
        let previous = capture(&root.join("game"))?;
        let journal = Self {
            schema_version: 1,
            transaction_id: uuid::Uuid::new_v4().simple().to_string(),
            instance_id: instance.clone(),
            stage: Stage::Prepared,
            previous,
            candidate: None,
        };
        // Journal exists before allocating a backup slot or resetting staging.
        journal.persist(root)?;
        ensure_directory(root, &journal.operation_dir(root)).map_err(InstallError::Storage)?;
        Ok(journal)
    }

    pub(crate) fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    fn operation_dir(&self, root: &Path) -> PathBuf {
        root.join(HISTORY_DIR).join(&self.transaction_id)
    }

    fn backup(&self, root: &Path) -> PathBuf {
        self.operation_dir(root).join("previous-game")
    }
    fn rejected(&self, root: &Path) -> PathBuf {
        self.operation_dir(root).join("rejected-game")
    }

    fn persist(&self, root: &Path) -> Result<(), InstallError> {
        let started = std::time::Instant::now();
        self.validate(&self.instance_id)?;
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|_| repair("journal serialization failed"))?;
        atomic_write(&root.join(JOURNAL_FILE), &bytes).map_err(InstallError::Storage)?;
        if std::env::var_os("AURORA_INSTALL_DIAGNOSTICS").is_some() {
            eprintln!(
                "[aurora-journal] transaction={} stage={:?} write_us={}",
                self.transaction_id,
                self.stage,
                started.elapsed().as_micros()
            );
        }
        Ok(())
    }

    fn transition(&mut self, root: &Path, stage: Stage) -> Result<(), InstallError> {
        let mut next = self.clone();
        next.stage = stage;
        next.persist(root)?;
        *self = next;
        Ok(())
    }

    fn archive(&self, root: &Path) -> Result<(), InstallError> {
        let started = std::time::Instant::now();
        ensure_directory(root, &self.operation_dir(root)).map_err(InstallError::Storage)?;
        atomic_write(
            &self.operation_dir(root).join("journal.json"),
            &serde_json::to_vec_pretty(self).map_err(|_| repair("journal serialization failed"))?,
        )
        .map_err(InstallError::Storage)?;
        profile("archive", started, 1, 0);
        Ok(())
    }

    pub(crate) fn promote(
        &mut self,
        root: &Path,
        faults: super::InstallFaults,
    ) -> Result<(), InstallError> {
        check_derived_directories(root, Some(self))?;
        let game = root.join("game");
        let staged = root.join(super::STAGING_DIR_NAME).join("game");
        if capture(&game)? != self.previous {
            return Err(repair(
                "the previous installation changed while the replacement was being prepared",
            ));
        }
        let candidate = capture(&staged)?
            .ok_or_else(|| repair("the staged candidate has no completion manifest"))?;
        if !candidate.valid || candidate.installation_id != self.transaction_id {
            return Err(repair(
                "the staged candidate failed deep validation or transaction identity",
            ));
        }
        sync_tree(&staged).map_err(InstallError::Storage)?;
        self.candidate = Some(candidate);
        self.transition(root, Stage::Replacing)?;
        checkpoint(root, faults, TransactionPoint::BeforeBackup)?;
        if self.previous.is_some() {
            rename_checked(root, &game, &self.backup(root))?;
        }
        checkpoint(root, faults, TransactionPoint::AfterBackup)?;
        if faults.fail_between_renames {
            return Err(repair(
                "deterministic interruption between replacement renames",
            ));
        }
        self.transition(root, Stage::Promoting)?;
        checkpoint(root, faults, TransactionPoint::BeforePromotion)?;
        rename_checked(root, &staged, &game)?;
        checkpoint(root, faults, TransactionPoint::AfterPromotion)?;
        self.transition(root, Stage::Promoted)?;
        let verification_started = std::time::Instant::now();
        if !matches_evidence(&game, self.candidate.as_ref(), true)? {
            return Err(repair(
                "promoted installation failed deep verification; previous tree remains recoverable",
            ));
        }
        profile("post_promotion_verification", verification_started, 0, 0);
        checkpoint(root, faults, TransactionPoint::BeforeCommit)?;
        self.transition(root, Stage::Committed)?;
        checkpoint(root, faults, TransactionPoint::AfterCommit)?;
        self.archive(root)?;
        if super::retention::record_completed(root, self).is_err() {
            eprintln!(
                "[aurora-retention] outcome=retainedUnknownOwnership reason=receiptUnavailable"
            );
        }
        checkpoint(root, faults, TransactionPoint::BeforeCleanup)?;
        // Deliberate retention: protected backups are never inside disposable
        // staging and are not garbage-collected by install or recovery.
        cleanup_staging(root)?;
        Ok(())
    }
}

pub(crate) fn checkpoint(
    _root: &Path,
    faults: super::InstallFaults,
    point: TransactionPoint,
) -> Result<(), InstallError> {
    #[cfg(all(debug_assertions, feature = "f3ra-desktop-acceptance"))]
    crate::desktop_acceptance::checkpoint(_root, point)?;
    if let Some(observer) = faults.transition_observer {
        observer(point);
    }
    if faults.stop_at == Some(point) {
        return Err(repair(format!("deterministic interruption at {point:?}")));
    }
    Ok(())
}

/// Public filesystem actions are performed only by callers holding the exact
/// target lock. The synchronous critical section has no cancellation await.
pub(crate) fn recover_locked(
    root: &Path,
    instance: &InstanceId,
    faults: super::InstallFaults,
) -> Result<RecoveryOutcome, InstallError> {
    check_derived_directories(root, None)?;
    let Some(mut journal) = Journal::load(root, instance)? else {
        return recover_legacy(root, instance);
    };
    check_derived_directories(root, Some(&journal))?;
    let game = root.join("game");
    if journal.stage == Stage::Committed {
        // A damaged committed tree remains repairable by explicit reinstall,
        // but a changed manifest is conflicting authority, not ordinary damage.
        if !same_manifest(&game, journal.candidate.as_ref())? {
            return Err(repair(
                "committed installation evidence conflicts with game state; preserve backup for manual repair",
            ));
        }
        journal.archive(root)?;
        cleanup_staging(root)?;
        return Ok(RecoveryOutcome::Completed);
    }
    if journal.stage.terminal() {
        if !matches_evidence(&game, journal.previous.as_ref(), false)? {
            return Err(repair(
                "restored installation evidence changed; manual repair required",
            ));
        }
        journal.archive(root)?;
        return Ok(if journal.stage == Stage::Restored {
            RecoveryOutcome::PreviousRestored
        } else {
            RecoveryOutcome::Interrupted
        });
    }
    if matches_evidence(&game, journal.previous.as_ref(), false)? {
        // This also handles a crash after restoring the old directory but
        // before writing Restored. No second move or deletion is needed.
        journal.transition(
            root,
            if journal.candidate.is_some() {
                Stage::Restored
            } else {
                Stage::Abandoned
            },
        )?;
        journal.archive(root)?;
        return Ok(RecoveryOutcome::Interrupted);
    }
    if journal.previous.as_ref().is_some_and(|old| old.valid)
        && matches_evidence(&journal.backup(root), journal.previous.as_ref(), true)?
    {
        journal.transition(root, Stage::Restoring)?;
        checkpoint(root, faults, TransactionPoint::Restoring)?;
        if path_exists(&game)? {
            if !same_manifest(&game, journal.candidate.as_ref())? {
                return Err(repair(
                    "promoted tree has conflicting identity; preserve both installations for manual repair",
                ));
            }
            rename_checked(root, &game, &journal.rejected(root))?;
        }
        checkpoint(root, faults, TransactionPoint::AfterQuarantine)?;
        rename_checked(root, &journal.backup(root), &game)?;
        checkpoint(root, faults, TransactionPoint::AfterRestore)?;
        if !matches_evidence(&game, journal.previous.as_ref(), true)? {
            return Err(repair(
                "the restored previous installation failed deep validation",
            ));
        }
        journal.transition(root, Stage::Restored)?;
        journal.archive(root)?;
        return Ok(RecoveryOutcome::PreviousRestored);
    }
    // Fresh installs (or replacements of previously damaged content) have no
    // valid old tree to prefer. Complete only the exact deeply valid candidate.
    if !journal.previous.as_ref().is_some_and(|old| old.valid)
        && matches_evidence(&game, journal.candidate.as_ref(), true)?
    {
        journal.transition(root, Stage::Committed)?;
        journal.archive(root)?;
        cleanup_staging(root)?;
        return Ok(RecoveryOutcome::Completed);
    }
    Err(repair(
        "interrupted installation has missing, damaged or conflicting recovery evidence; preserve all trees for manual repair",
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOutcome {
    None,
    Interrupted,
    PreviousRestored,
    Completed,
}

/// Startup avoids re-hashing stable installations. Any nonterminal authority
/// or leftover staging requires the same locked replay used by explicit retry.
pub(crate) fn needs_recovery(root: &Path, instance: &InstanceId) -> Result<bool, InstallError> {
    Ok(
        Journal::load(root, instance)?.is_some_and(|journal| !journal.stage.terminal())
            || path_exists(&root.join(super::STAGING_DIR_NAME))?,
    )
}

/// Read-only presentation of terminal authority after readiness gating.
pub(crate) fn recovery_status(
    root: &Path,
    instance: &InstanceId,
    instance_incomplete: bool,
) -> Result<Option<&'static str>, InstallError> {
    Ok(
        Journal::load(root, instance)?.and_then(|journal| match journal.stage {
            Stage::Restored => Some("previousRestored"),
            Stage::Abandoned => Some("interrupted"),
            // The game commit survives read-only reloads while the enclosing
            // instance still needs its explicit lifecycle retry. Ordinary
            // Ready instances should not be mislabeled as a recovery.
            Stage::Committed if instance_incomplete => Some("completed"),
            Stage::Committed => None,
            _ => None,
        }),
    )
}

/// Read-only readiness gate. A complete-looking manifest does not outrank an
/// unresolved replacement. Existing schema-1/2 installs without journals work.
pub(crate) fn readiness_gate(root: &Path, instance: &InstanceId) -> Result<(), InstallError> {
    if let Some(journal) = Journal::load(root, instance)? {
        if !journal.stage.terminal() {
            return Err(repair(
                "interrupted installation detected; restart or retry to recover before Play",
            ));
        }
        let evidence = if journal.stage == Stage::Committed {
            &journal.candidate
        } else {
            &journal.previous
        };
        if (evidence.is_some() && !same_manifest(&root.join("game"), evidence.as_ref())?)
            || (evidence.is_none() && path_exists(&root.join("game"))?)
        {
            return Err(repair(
                "installation commit evidence conflicts with the installed manifest",
            ));
        }
    } else {
        no_orphaned_history(root, instance)?;
        if path_exists(
            &root
                .join(super::STAGING_DIR_NAME)
                .join(super::REPLACED_GAME_DIR_NAME),
        )? {
            return Err(repair(
                "a legacy replacement backup awaits recovery; Ready is blocked",
            ));
        }
    }
    Ok(())
}

fn no_orphaned_history(root: &Path, instance: &InstanceId) -> Result<(), InstallError> {
    let history = root.join(HISTORY_DIR);
    if !path_exists(&history)? {
        return Ok(());
    }
    super::coordination::checked_directory(root, &history).map_err(InstallError::Storage)?;
    for (count, entry) in std::fs::read_dir(&history)
        .map_err(InstallError::Storage)?
        .enumerate()
    {
        if count >= 10_000 {
            return Err(repair(
                "too many retained operation records to safely discover recovery",
            ));
        }
        let path = entry.map_err(InstallError::Storage)?.path();
        if !path
            .file_name()
            .and_then(|part| part.to_str())
            .is_some_and(safe_uuid)
        {
            return Err(repair(
                "unknown recovery-history entry; preserve it for inspection",
            ));
        }
        super::coordination::checked_directory(root, &path).map_err(InstallError::Storage)?;
        let Some(journal) = read_document(&path.join("journal.json"), instance)? else {
            return Err(repair(
                "missing active journal with unarchived recovery trees; manual repair required",
            ));
        };
        if !journal.stage.terminal() || journal.operation_dir(root) != path {
            return Err(repair("recovery history contains conflicting authority"));
        }
    }
    Ok(())
}

fn recover_legacy(root: &Path, instance: &InstanceId) -> Result<RecoveryOutcome, InstallError> {
    no_orphaned_history(root, instance)?;
    let staging = root.join(super::STAGING_DIR_NAME);
    let backup = staging.join(super::REPLACED_GAME_DIR_NAME);
    if !path_exists(&backup)? {
        return Ok(RecoveryOutcome::None);
    }
    checked_tree(&staging).map_err(InstallError::Storage)?;
    // A legacy receipt does not contain new commit authority. Only a missing
    // game and a matching known replacement receipt allow old-tree restore.
    let matching = matches!(super::receipt::classify_existing(&staging),
        super::receipt::Disposition::Interrupted { instance_id, last_stage, .. }
        if instance_id == instance.as_str() && matches!(last_stage.as_str(), "replacing" | "committing"));
    if !matching
        || path_exists(&root.join("game"))?
        || !capture(&backup)?.is_some_and(|old| old.valid)
    {
        return Err(repair(
            "legacy schema-1 replacement evidence is ambiguous; preserve staging and game for manual repair",
        ));
    }
    rename_checked(root, &backup, &root.join("game"))?;
    Ok(RecoveryOutcome::PreviousRestored)
}

fn path_exists(path: &Path) -> Result<bool, InstallError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(InstallError::Storage(error)),
    }
}

fn check_derived_directories(root: &Path, journal: Option<&Journal>) -> Result<(), InstallError> {
    let mut directories = vec![
        root.join("game"),
        root.join(super::STAGING_DIR_NAME),
        root.join(HISTORY_DIR),
    ];
    if let Some(journal) = journal {
        directories.extend([
            journal.operation_dir(root),
            journal.backup(root),
            journal.rejected(root),
        ]);
    }
    for directory in directories {
        if path_exists(&directory)? {
            super::coordination::checked_directory(root, &directory)
                .map_err(InstallError::Storage)?;
        }
    }
    Ok(())
}

fn same_manifest(path: &Path, evidence: Option<&Evidence>) -> Result<bool, InstallError> {
    let Some(evidence) = evidence else {
        return Ok(false);
    };
    if !path_exists(path)? {
        return Ok(false);
    }
    checked_tree(path).map_err(InstallError::Storage)?;
    let Some(manifest) = load_installed_state(path).map_err(InstallError::State)? else {
        return Ok(false);
    };
    Ok(manifest.installation_id() == evidence.installation_id
        && digest(&bounded_manifest(path)?) == evidence.manifest_sha256)
}

fn bounded_manifest(path: &Path) -> Result<Vec<u8>, InstallError> {
    let mut bytes = Vec::new();
    File::open(path.join(INSTALLED_GAME_FILE_NAME))
        .map_err(InstallError::Storage)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(InstallError::Storage)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(repair("installed manifest exceeds recovery bounds"));
    }
    Ok(bytes)
}

pub(crate) fn matches_evidence(
    path: &Path,
    evidence: Option<&Evidence>,
    require_valid: bool,
) -> Result<bool, InstallError> {
    let Some(expected) = evidence else {
        return Ok(!path_exists(path)?);
    };
    let Some(actual) = capture(path)? else {
        return Ok(false);
    };
    Ok(&actual == expected && (!require_valid || actual.valid))
}

fn capture(path: &Path) -> Result<Option<Evidence>, InstallError> {
    if !path_exists(path)? {
        return Ok(None);
    }
    let started = std::time::Instant::now();
    let files = checked_tree(path).map_err(InstallError::Storage)?;
    profile("tree_enumeration", started, files.len(), 0);
    let started = std::time::Instant::now();
    let Some(manifest) = load_installed_state(path).map_err(InstallError::State)? else {
        return Err(repair(
            "a candidate tree exists without a valid installed-state manifest",
        ));
    };
    profile("manifest_parse", started, 1, 0);
    let started = std::time::Instant::now();
    let valid = matches!(super::validate_game_directory(path)?,
        ValidationOutcome::Installed(validation) if validation.status == ValidationStatus::Valid);
    profile("manifest_deep_validation", started, files.len(), 0);
    let started = std::time::Instant::now();
    let file_count = files.len();
    let mut total_bytes = 0;
    let mut tree = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    for file_path in files {
        let relative = file_path
            .strip_prefix(path)
            .map_err(|_| repair("tree containment failed"))?
            .to_str()
            .ok_or_else(|| repair("non-Unicode managed tree name"))?
            .replace('\\', "/");
        tree.update((relative.len() as u64).to_le_bytes());
        tree.update(relative.as_bytes());
        let mut file = File::open(&file_path).map_err(InstallError::Storage)?;
        let size = file.metadata().map_err(InstallError::Storage)?.len();
        total_bytes += size;
        tree.update(size.to_le_bytes());
        loop {
            let count = file.read(&mut buffer).map_err(InstallError::Storage)?;
            if count == 0 {
                break;
            }
            tree.update(&buffer[..count]);
        }
    }
    profile("tree_sha256", started, file_count, total_bytes);
    Ok(Some(Evidence {
        installation_id: manifest.installation_id().to_owned(),
        manifest_sha256: digest(&bounded_manifest(path)?),
        tree_sha256: ArtifactDigest::from_sha256(tree.finalize().into()).as_hex(),
        valid,
    }))
}

fn rename_checked(root: &Path, from: &Path, to: &Path) -> Result<(), InstallError> {
    let started = std::time::Instant::now();
    // No copy fallback: every transaction tree remains on the same managed
    // filesystem. Refuse existing destinations and redirected ancestors.
    for path in [from, to] {
        let parent = path
            .parent()
            .ok_or_else(|| repair("missing rename parent"))?;
        super::coordination::checked_directory(root, parent).map_err(InstallError::Storage)?;
    }
    checked_tree(from).map_err(InstallError::Storage)?;
    if path_exists(to)? {
        return Err(repair(
            "recovery destination is occupied; preserve both trees",
        ));
    }
    profile("rename_checks", started, 0, 0);
    let started = std::time::Instant::now();
    std::fs::rename(from, to).map_err(InstallError::Storage)?;
    sync_directory(from.parent().unwrap()).map_err(InstallError::Storage)?;
    sync_directory(to.parent().unwrap()).map_err(InstallError::Storage)?;
    profile(
        if to.file_name().is_some_and(|name| name == "previous-game") {
            "backup_rename"
        } else if to.file_name().is_some_and(|name| name == "rejected-game") {
            "quarantine_rename"
        } else {
            "promotion_or_restore_rename"
        },
        started,
        0,
        0,
    );
    Ok(())
}

pub(crate) fn cleanup_staging(root: &Path) -> Result<(), InstallError> {
    let started = std::time::Instant::now();
    let staging = root.join(super::STAGING_DIR_NAME);
    if path_exists(&staging)? {
        super::coordination::checked_directory(root, &staging).map_err(InstallError::Storage)?;
        checked_tree(&staging).map_err(InstallError::Storage)?;
        if path_exists(&staging.join(super::REPLACED_GAME_DIR_NAME))? {
            return Err(repair(
                "refusing staging disposal because it contains a legacy installation backup",
            ));
        }
        std::fs::remove_dir_all(&staging).map_err(InstallError::Storage)?;
    }
    profile("cleanup_bookkeeping", started, 0, 0);
    Ok(())
}

fn sync_tree(path: &Path) -> io::Result<()> {
    let started = std::time::Instant::now();
    for file in checked_tree(path)? {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(file)?
            .sync_all()?;
    }
    #[cfg(unix)]
    {
        let mut directories = vec![path.to_path_buf()];
        let mut index = 0;
        while index < directories.len() {
            for entry in std::fs::read_dir(&directories[index])? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    directories.push(entry.path());
                }
            }
            index += 1;
        }
        for directory in directories.into_iter().rev() {
            sync_directory(&directory)?;
        }
    }
    profile("staging_synchronization", started, 0, 0);
    Ok(())
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_with_checkpoints(path, bytes, |_, _| Ok(()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WritePoint {
    Created,
    Synced,
    Replaced,
}

fn atomic_write_with_checkpoints(
    path: &Path,
    bytes: &[u8],
    mut checkpoint: impl FnMut(WritePoint, &Path) -> io::Result<()>,
) -> io::Result<()> {
    require_regular_or_missing(path)?;
    let parent = path.parent().ok_or_else(super::coordination::unsafe_path)?;
    let temporary = parent.join(format!(".operation-{}.tmp", uuid::Uuid::new_v4().simple()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    checkpoint(WritePoint::Created, &temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    checkpoint(WritePoint::Synced, &temporary)?;
    drop(file);
    std::fs::rename(&temporary, path)?;
    checkpoint(WritePoint::Replaced, path)?;
    sync_directory(parent)
}

#[cfg(unix)]
pub(crate) fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}
#[cfg(not(unix))]
pub(crate) fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::state::{
        InstalledFile, InstalledFileRole, InstalledGameManifest, NativesRecord,
    };
    use crate::instances::platform::PlatformPin;
    use crate::integrity::{ArtifactTrust, DigestAlgorithm};

    struct World {
        root: PathBuf,
        id: InstanceId,
    }
    impl World {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "aurora-f3r-journal-{}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir(&root).unwrap();
            std::fs::write(root.join("user-sentinel"), b"owner bytes").unwrap();
            Self {
                root,
                id: InstanceId::new("journal-test").unwrap(),
            }
        }
        fn old(&self) -> Evidence {
            write_game(&self.root.join("game"), "previous", b"old");
            capture(&self.root.join("game")).unwrap().unwrap()
        }
        fn prepare(&self) -> Journal {
            let journal = Journal::begin(&self.root, &self.id).unwrap();
            write_game(
                &self.root.join(super::super::STAGING_DIR_NAME).join("game"),
                journal.transaction_id(),
                b"new",
            );
            journal
        }
        fn recover(&self) -> Result<RecoveryOutcome, InstallError> {
            recover_locked(&self.root, &self.id, Default::default())
        }
    }
    impl Drop for World {
        fn drop(&mut self) {
            assert_eq!(
                std::fs::read(self.root.join("user-sentinel")).unwrap(),
                b"owner bytes"
            );
            // The unique disposable root was positively created by this test.
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }
    fn write_game(path: &Path, id: &str, bytes: &[u8]) {
        std::fs::create_dir_all(path.join("natives")).unwrap();
        std::fs::write(path.join("natives/native.bin"), b"native").unwrap();
        std::fs::write(path.join("client.jar"), bytes).unwrap();
        let manifest = InstalledGameManifest::for_platform(
            "1.20.1",
            PlatformPin::Vanilla {},
            id,
            1,
            vec![InstalledFile::new(
                InstalledFileRole::Client,
                "client.jar".into(),
                ArtifactTrust::ExpectedDigestVerified {
                    algorithm: DigestAlgorithm::Sha256,
                    digest: digest(bytes),
                },
                bytes.len() as u64,
            )],
            NativesRecord::new("natives".into()),
        );
        std::fs::write(path.join(INSTALLED_GAME_FILE_NAME), manifest.to_json()).unwrap();
        assert!(capture(path).unwrap().unwrap().valid);
    }
    fn stop(point: TransactionPoint) -> super::super::InstallFaults {
        super::super::InstallFaults {
            stop_at: Some(point),
            ..Default::default()
        }
    }

    #[test]
    fn every_replacement_transition_preserves_authority_and_replays_idempotently() {
        for point in [
            TransactionPoint::BeforeBackup,
            TransactionPoint::AfterBackup,
            TransactionPoint::BeforePromotion,
            TransactionPoint::AfterPromotion,
            TransactionPoint::BeforeCommit,
            TransactionPoint::AfterCommit,
            TransactionPoint::BeforeCleanup,
        ] {
            let world = World::new();
            let previous = world.old();
            let mut journal = world.prepare();
            assert!(
                journal.promote(&world.root, stop(point)).is_err(),
                "{point:?}"
            );
            let committed = matches!(
                point,
                TransactionPoint::AfterCommit | TransactionPoint::BeforeCleanup
            );
            assert_eq!(
                readiness_gate(&world.root, &world.id).is_ok(),
                committed,
                "{point:?}"
            );
            world.recover().unwrap();
            assert_eq!(
                recovery_status(&world.root, &world.id, false).unwrap(),
                if committed {
                    None
                } else {
                    Some("previousRestored")
                }
            );
            assert_eq!(
                recovery_status(&world.root, &world.id, true).unwrap(),
                Some(if committed {
                    "completed"
                } else {
                    "previousRestored"
                })
            );
            let settled = std::fs::read(world.root.join(JOURNAL_FILE)).unwrap();
            world.recover().unwrap();
            assert_eq!(
                settled,
                std::fs::read(world.root.join(JOURNAL_FILE)).unwrap()
            );
            let game = capture(&world.root.join("game")).unwrap().unwrap();
            if committed {
                assert_eq!(game.installation_id, journal.transaction_id);
                assert_eq!(
                    capture(&journal.backup(&world.root)).unwrap(),
                    Some(previous)
                );
            } else {
                assert_eq!(game, previous);
            }
            readiness_gate(&world.root, &world.id).unwrap();
        }
    }

    #[test]
    fn recovery_can_be_interrupted_before_quarantine_after_quarantine_and_after_restore() {
        for point in [
            TransactionPoint::Restoring,
            TransactionPoint::AfterQuarantine,
            TransactionPoint::AfterRestore,
        ] {
            let world = World::new();
            let old = world.old();
            let mut journal = world.prepare();
            journal
                .promote(&world.root, stop(TransactionPoint::AfterPromotion))
                .unwrap_err();
            recover_locked(&world.root, &world.id, stop(point)).unwrap_err();
            world.recover().unwrap();
            world.recover().unwrap();
            assert_eq!(capture(&world.root.join("game")).unwrap(), Some(old));
            assert!(journal.rejected(&world.root).is_dir());
        }
    }

    #[test]
    fn damaged_candidate_restores_old_and_damaged_old_never_gets_selected() {
        let world = World::new();
        let old = world.old();
        let mut journal = world.prepare();
        journal
            .promote(&world.root, stop(TransactionPoint::AfterPromotion))
            .unwrap_err();
        std::fs::write(world.root.join("game/client.jar"), b"bad").unwrap();
        world.recover().unwrap();
        assert_eq!(capture(&world.root.join("game")).unwrap(), Some(old));
        assert_eq!(
            std::fs::read(journal.rejected(&world.root).join("client.jar")).unwrap(),
            b"bad"
        );

        let damaged = World::new();
        damaged.old();
        std::fs::write(damaged.root.join("game/client.jar"), b"bad").unwrap();
        let mut journal = damaged.prepare();
        journal
            .promote(&damaged.root, stop(TransactionPoint::AfterPromotion))
            .unwrap_err();
        assert_eq!(damaged.recover().unwrap(), RecoveryOutcome::Completed);
        assert_eq!(
            capture(&damaged.root.join("game"))
                .unwrap()
                .unwrap()
                .installation_id,
            journal.transaction_id
        );
        assert!(journal.backup(&damaged.root).exists());
    }

    #[test]
    fn missing_or_damaged_backup_never_silently_accepts_an_uncommitted_replacement() {
        for missing in [false, true] {
            let world = World::new();
            world.old();
            let mut journal = world.prepare();
            journal
                .promote(&world.root, stop(TransactionPoint::AfterPromotion))
                .unwrap_err();
            if missing {
                // Model disappearance without deleting the only previous tree.
                std::fs::rename(
                    journal.backup(&world.root),
                    world.root.join("displaced-previous"),
                )
                .unwrap();
            } else {
                std::fs::write(journal.backup(&world.root).join("client.jar"), b"bad").unwrap();
            }
            let before = capture(&world.root.join("game")).unwrap();
            world.recover().unwrap_err();
            world.recover().unwrap_err();
            assert_eq!(capture(&world.root.join("game")).unwrap(), before);
            readiness_gate(&world.root, &world.id).unwrap_err();
        }
    }

    #[test]
    fn malformed_future_path_injected_and_missing_journals_preserve_all_trees() {
        for kind in ["truncated", "future", "path", "identity", "missing"] {
            let world = World::new();
            world.old();
            let mut journal = world.prepare();
            journal
                .promote(&world.root, stop(TransactionPoint::AfterBackup))
                .unwrap_err();
            let mut value = serde_json::to_value(&journal).unwrap();
            match kind {
                "future" => value["schemaVersion"] = serde_json::json!(999),
                "path" => value["backupPath"] = serde_json::json!("../../owner"),
                "identity" => value["transactionId"] = serde_json::json!("../../owner"),
                _ => {}
            }
            let path = world.root.join(JOURNAL_FILE);
            if kind == "missing" {
                std::fs::remove_file(&path).unwrap();
            } else {
                std::fs::write(
                    &path,
                    if kind == "truncated" {
                        b"{".to_vec()
                    } else {
                        serde_json::to_vec(&value).unwrap()
                    },
                )
                .unwrap();
            }
            let before = capture(&journal.backup(&world.root)).unwrap();
            world.recover().unwrap_err();
            readiness_gate(&world.root, &world.id).unwrap_err();
            assert_eq!(capture(&journal.backup(&world.root)).unwrap(), before);
            assert!(
                world
                    .root
                    .join(super::super::STAGING_DIR_NAME)
                    .join("game")
                    .exists()
            );
        }
    }

    #[test]
    fn fresh_prepared_and_promoted_installs_have_deliberate_outcomes() {
        let world = World::new();
        world.prepare();
        world.recover().unwrap();
        world.recover().unwrap();
        assert!(!world.root.join("game").exists());
        let world = World::new();
        let mut journal = world.prepare();
        journal
            .promote(&world.root, stop(TransactionPoint::AfterPromotion))
            .unwrap_err();
        assert_eq!(world.recover().unwrap(), RecoveryOutcome::Completed);
        readiness_gate(&world.root, &world.id).unwrap();
    }

    #[test]
    fn schema_one_legacy_receipt_restores_only_unambiguous_deeply_valid_backup() {
        let world = World::new();
        let backup = world
            .root
            .join(super::super::STAGING_DIR_NAME)
            .join(super::super::REPLACED_GAME_DIR_NAME);
        write_game(&backup, "legacy", b"old");
        let manifest_path = backup.join(INSTALLED_GAME_FILE_NAME);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        value["schemaVersion"] = serde_json::json!(1);
        value.as_object_mut().unwrap().remove("platform");
        value["fabricLoaderVersion"] = serde_json::json!("0.16.0");
        std::fs::write(&manifest_path, serde_json::to_vec(&value).unwrap()).unwrap();
        let staging = backup.parent().unwrap();
        super::super::receipt::write_test_receipt(staging, &serde_json::json!({"schemaVersion":1,"transactionId":"legacy","instanceId":world.id,"stages":[{"stage":"replacing"}]}).to_string());
        write_game(&world.root.join("game"), "uncommitted", b"new");
        world.recover().unwrap_err();
        assert!(backup.exists());
        std::fs::rename(
            world.root.join("game"),
            world.root.join("uncommitted-retained"),
        )
        .unwrap();
        assert_eq!(world.recover().unwrap(), RecoveryOutcome::PreviousRestored);
        assert_eq!(
            load_installed_state(&world.root.join("game"))
                .unwrap()
                .unwrap()
                .schema_version(),
            1
        );
        world.recover().unwrap();
    }

    #[test]
    fn journal_write_and_rename_io_failures_keep_previous_tree() {
        let world = World::new();
        let old = world.old();
        let mut journal = world.prepare();
        std::fs::create_dir(journal.backup(&world.root)).unwrap();
        journal
            .promote(&world.root, Default::default())
            .unwrap_err();
        assert_eq!(capture(&world.root.join("game")).unwrap(), Some(old));
        assert!(
            world
                .root
                .join(super::super::STAGING_DIR_NAME)
                .join("game")
                .exists()
        );
        let world = World::new();
        let old = world.old();
        std::fs::create_dir(world.root.join(JOURNAL_FILE)).unwrap();
        Journal::begin(&world.root, &world.id).unwrap_err();
        assert_eq!(capture(&world.root.join("game")).unwrap(), Some(old));
    }

    #[test]
    fn partial_journal_write_sync_and_replace_errors_preserve_atomic_authority() {
        for kind in [
            io::ErrorKind::StorageFull,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::CrossesDevices,
        ] {
            for point in [
                WritePoint::Created,
                WritePoint::Synced,
                WritePoint::Replaced,
            ] {
                let world = World::new();
                let old = world.old();
                let mut journal = world.prepare();
                let path = world.root.join(JOURNAL_FILE);
                let before = std::fs::read(&path).unwrap();
                journal.stage = Stage::Abandoned;
                let after = serde_json::to_vec_pretty(&journal).unwrap();
                let error = atomic_write_with_checkpoints(&path, &after, |at, temporary| {
                    if at == point {
                        if at == WritePoint::Created {
                            std::fs::write(temporary, b"{")?;
                        }
                        return Err(io::Error::new(kind, "deterministic filesystem fault"));
                    }
                    Ok(())
                })
                .unwrap_err();
                assert_eq!(error.kind(), kind);
                assert_eq!(
                    std::fs::read(&path).unwrap(),
                    if point == WritePoint::Replaced {
                        after
                    } else {
                        before
                    }
                );
                world.recover().unwrap();
                world.recover().unwrap();
                assert_eq!(capture(&world.root.join("game")).unwrap(), Some(old));
            }
        }
    }

    #[test]
    fn lost_staging_can_restore_old_but_two_damaged_trees_require_manual_repair() {
        let world = World::new();
        let old = world.old();
        let mut journal = world.prepare();
        journal
            .promote(&world.root, stop(TransactionPoint::AfterBackup))
            .unwrap_err();
        std::fs::rename(
            world.root.join(super::super::STAGING_DIR_NAME),
            world.root.join("displaced-staging"),
        )
        .unwrap();
        world.recover().unwrap();
        assert_eq!(capture(&world.root.join("game")).unwrap(), Some(old));
        let world = World::new();
        world.old();
        let mut journal = world.prepare();
        journal
            .promote(&world.root, stop(TransactionPoint::AfterPromotion))
            .unwrap_err();
        for tree in [world.root.join("game"), journal.backup(&world.root)] {
            std::fs::write(tree.join("client.jar"), b"damaged").unwrap();
        }
        world.recover().unwrap_err();
        world.recover().unwrap_err();
        assert!(world.root.join("game").exists() && journal.backup(&world.root).exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_locked_backup_and_cleanup_failures_leave_recoverable_authority() {
        use std::os::windows::fs::OpenOptionsExt;
        let world = World::new();
        let old = world.old();
        let mut journal = world.prepare();
        journal
            .promote(&world.root, stop(TransactionPoint::AfterBackup))
            .unwrap_err();
        let locked_directory = OpenOptions::new()
            .read(true)
            .share_mode(1 | 2)
            .custom_flags(0x02000000)
            .open(journal.backup(&world.root))
            .unwrap();
        assert!(matches!(world.recover(), Err(InstallError::Storage(_))));
        readiness_gate(&world.root, &world.id).unwrap_err();
        assert_eq!(
            capture(&journal.backup(&world.root)).unwrap(),
            Some(old.clone())
        );
        drop(locked_directory);
        world.recover().unwrap();
        assert_eq!(capture(&world.root.join("game")).unwrap(), Some(old));

        let world = World::new();
        let old = world.old();
        let mut journal = world.prepare();
        journal
            .promote(&world.root, stop(TransactionPoint::AfterCommit))
            .unwrap_err();
        let staging = world.root.join(super::super::STAGING_DIR_NAME);
        std::fs::write(staging.join("locked-scratch"), b"scratch").unwrap();
        let held = OpenOptions::new()
            .read(true)
            .share_mode(1 | 2)
            .open(staging.join("locked-scratch"))
            .unwrap();
        assert!(matches!(world.recover(), Err(InstallError::Storage(_))));
        readiness_gate(&world.root, &world.id).unwrap();
        assert_eq!(capture(&journal.backup(&world.root)).unwrap(), Some(old));
        assert_eq!(
            capture(&world.root.join("game")).unwrap(),
            journal.candidate
        );
        drop(held);
        world.recover().unwrap();
        world.recover().unwrap();
        assert!(!staging.exists());
    }
}
