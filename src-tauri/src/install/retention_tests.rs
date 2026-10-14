use super::*;
use crate::install::state::{InstalledFile, InstalledFileRole, NativesRecord};
use crate::instances::platform::PlatformPin;
use crate::integrity::{ArtifactTrust, DigestAlgorithm};

struct World {
    root: PathBuf,
    id: InstanceId,
    cleanup: bool,
}
impl World {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("aurora-f3rb-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("owner-sentinel"), b"owner").unwrap();
        Self {
            root,
            id: InstanceId::new("retention-test").unwrap(),
            cleanup: true,
        }
    }
    fn game(&self, path: &Path, id: &str) {
        self.game_payload(path, id, b"client");
    }
    fn game_payload(&self, path: &Path, id: &str, payload: &[u8]) {
        fs::create_dir_all(path.join("natives")).unwrap();
        fs::write(path.join("client.jar"), payload).unwrap();
        fs::write(path.join("natives/native.dll"), b"native").unwrap();
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
                    digest: hash(payload),
                },
                payload.len() as u64,
            )],
            NativesRecord::new("natives".into()),
        );
        fs::write(path.join(INSTALLED_GAME_FILE_NAME), manifest.to_json()).unwrap();
        seal_staging(
            path,
            &manifest,
            vec![OwnedFile::observed("natives/native.dll".into(), b"native")],
        )
        .unwrap();
    }
    fn replace(&self) -> String {
        if !self.root.join("game").exists() {
            let mut fresh = Journal::begin(&self.root, &self.id).unwrap();
            self.game(
                &self.root.join(".install-staging/game"),
                fresh.transaction_id(),
            );
            fresh.promote(&self.root, Default::default()).unwrap();
        }
        let mut journal = Journal::begin(&self.root, &self.id).unwrap();
        self.game(
            &self.root.join(".install-staging/game"),
            journal.transaction_id(),
        );
        journal.promote(&self.root, Default::default()).unwrap();
        journal.transaction_id().to_owned()
    }
    fn slot(&self, id: &str) -> PathBuf {
        self.root.join(journal::HISTORY_DIR).join(id)
    }
    fn dated(&self, id: &str, time: u64) {
        let path = self.slot(id).join(RECEIPT);
        let mut receipt: Receipt = serde_json::from_slice(&read_bounded(&path).unwrap()).unwrap();
        receipt.completed_at_unix_seconds = time;
        write_json(&path, &receipt).unwrap();
    }
    fn run(&self, time: u64) -> RetentionReport {
        maintain_locked(&self.root, &self.id, time, |_| Ok(()))
    }
    fn three(&self) -> (String, String, String) {
        let old = self.replace();
        let recent = self.replace();
        let active = self.replace();
        self.dated(&old, 1);
        self.dated(&recent, 2);
        self.dated(&active, 3);
        (old, recent, active)
    }
}

#[test]
fn representative_storage_measurement() {
    let world = World::new();
    let payload = vec![42u8; 16 * 1024 * 1024];
    let mut fresh = Journal::begin(&world.root, &world.id).unwrap();
    world.game_payload(
        &world.root.join(".install-staging/game"),
        fresh.transaction_id(),
        &payload,
    );
    fresh.promote(&world.root, Default::default()).unwrap();
    for time in 1..=3 {
        let mut journal = Journal::begin(&world.root, &world.id).unwrap();
        world.game_payload(
            &world.root.join(".install-staging/game"),
            journal.transaction_id(),
            &payload,
        );
        journal.promote(&world.root, Default::default()).unwrap();
        world.dated(journal.transaction_id(), time);
    }
    let result = world.run(GRACE_SECONDS + 10);
    assert_eq!(result.retired_backups, 1);
    assert!(result.reclaimed_bytes >= payload.len() as u64);
    eprintln!(
        "[f3rb-representative-measurement] {}",
        serde_json::to_string(&result).unwrap()
    );
}

// Test-only fixture boundary; direct invocation receives the same validation
// and token checks as the PowerShell driver.
#[cfg(windows)]
#[path = "retention_symlink_gate.rs"]
mod symlink_gate;

#[cfg(windows)]
#[test]
#[ignore = "opt-in real Windows symlink acceptance; use run-symlink-gate.ps1"]
fn windows_file_symlink_refusal_helper() {
    symlink_gate::run_from_environment().unwrap();
}

/// Opt-in small storage fixture for the REAL desktop, not a gameplay fixture.
/// It uses production journal promotion; no owner root or account is accepted.
#[test]
fn desktop_storage_fixture_helper() {
    let Some(root) = std::env::var_os("AURORA_F3RB_DESKTOP_FIXTURE") else {
        return;
    };
    let root = PathBuf::from(root);
    assert_eq!(
        root.canonicalize().unwrap().parent(),
        Some(std::env::temp_dir().canonicalize().unwrap().as_path())
    );
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("aurora-f3ra-F3RB-")
    );
    assert_eq!(
        fs::read(root.join("F3RA-ISOLATED.txt")).unwrap(),
        b"F3RA isolated desktop acceptance\n"
    );
    let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
    let requested_id =
        std::env::var("AURORA_F3RB_STORAGE_ID").unwrap_or_else(|_| "f3rb-storage-fixture".into());
    assert!(requested_id.starts_with("f3rb-storage-"));
    let id = InstanceId::new(requested_id).unwrap();
    let fixture_root = managed.instance_paths(&id).root().to_path_buf();
    assert!(!fixture_root.exists());
    fs::create_dir(&fixture_root).unwrap();
    fs::write(fixture_root.join("owner-sentinel"), b"owner").unwrap();
    let world = World {
        root: fixture_root,
        id: id.clone(),
        cleanup: false,
    };
    let _lifecycle = crate::instances::lifecycle::lock_instance_operation(&managed, &id).unwrap();
    let _game = super::super::lock_game_target(&managed, &id).unwrap();
    let payload = vec![42u8; 16 * 1024 * 1024];
    let mut fresh = Journal::begin(&world.root, &id).unwrap();
    world.game_payload(
        &world.root.join(".install-staging/game"),
        fresh.transaction_id(),
        &payload,
    );
    fresh.promote(&world.root, Default::default()).unwrap();
    let mut transactions = Vec::new();
    for time in 1..=5 {
        let mut journal = Journal::begin(&world.root, &id).unwrap();
        world.game_payload(
            &world.root.join(".install-staging/game"),
            journal.transaction_id(),
            &payload,
        );
        journal.promote(&world.root, Default::default()).unwrap();
        world.dated(journal.transaction_id(), time);
        transactions.push(journal.transaction_id().to_owned());
    }
    let installed = crate::instances::platform::InstalledConfiguration {
        minecraft_version: "1.20.1".into(),
        platform: PlatformPin::Vanilla {},
        aurora: None,
    };
    let configuration = serde_json::from_value(serde_json::json!({"minecraftVersion":"1.20.1","auroraEnabled":false,"loader":{"kind":"vanilla"},"memoryMib":2048,"additionalJvmArguments":"","window":null})).unwrap();
    let record = crate::instances::InstanceRecord::from_installed(
        id.clone(),
        "F3RB storage fixture (synthetic)",
        crate::instances::InstanceState::Ready,
        installed,
        configuration,
    )
    .unwrap();
    let mut registry =
        crate::instances::InstanceRegistry::load(&managed.instance_registry_file()).unwrap();
    registry.instances_mut().push(record);
    registry.save(&managed.instance_registry_file()).unwrap();
    let evidence_name = if std::env::var_os("AURORA_F3RB_STORAGE_ID").is_some() {
        format!("storage-fixture-{id}.json")
    } else {
        "storage-fixture.json".into()
    };
    fs::write(root.join("evidence").join(evidence_name), serde_json::to_vec(&serde_json::json!({"transactions":transactions,"bytesPerClient":payload.len(),"synthetic":true,"timestampsBackdatedForAcceptance":true})).unwrap()).unwrap();
    // Acceptance evidence is deliberately retained; ordinary tests use Drop.
    std::mem::forget(world);
}

#[test]
fn retention_process_helper() {
    let Some(root) = std::env::var_os("AURORA_F3RB_CHILD_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(
        root.parent() == Some(std::env::temp_dir().as_path())
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-f3rb-")
    );
    let _lifecycle =
        super::super::coordination::TargetLock::acquire(&root, &root, ".instance-operation.lock")
            .unwrap();
    let _game = super::super::coordination::TargetLock::acquire(&root, &root, ".game-install.lock")
        .unwrap();
    let requested = std::env::var("AURORA_F3RB_CHILD_POINT").unwrap();
    maintain_locked(
        &root,
        &InstanceId::new("retention-test").unwrap(),
        u64::MAX,
        |point| {
            if format!("{point:?}") == requested {
                fs::write(root.join("child-parked"), b"parked")?;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
            Ok(())
        },
    );
}

#[test]
fn real_process_termination_during_retirement_releases_locks_and_restarts() {
    for point in ["Intent", "Renamed", "FileRemoved", "Empty"] {
        let world = World::new();
        let (old, _, _) = world.three();
        let authority = fs::read(world.root.join(journal::JOURNAL_FILE)).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "install::retention::tests::retention_process_helper",
                "--nocapture",
            ])
            .env("AURORA_F3RB_CHILD_ROOT", &world.root)
            .env("AURORA_F3RB_CHILD_POINT", point)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !world.root.join("child-parked").exists() && start.elapsed().as_secs() < 15 {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let parked = world.root.join("child-parked").exists();
        let busy = matches!(
            super::super::coordination::TargetLock::acquire(
                &world.root,
                &world.root,
                ".game-install.lock"
            ),
            Err(super::super::coordination::LockError::Busy)
        );
        let pid = child.id();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(parked && busy);
        let _lifecycle = super::super::coordination::TargetLock::acquire(
            &world.root,
            &world.root,
            ".instance-operation.lock",
        )
        .unwrap();
        let _game = super::super::coordination::TargetLock::acquire(
            &world.root,
            &world.root,
            ".game-install.lock",
        )
        .unwrap();
        world.run(u64::MAX);
        world.run(u64::MAX);
        assert!(
            !world.slot(&old).join("previous-game").exists()
                && !world.slot(&old).join("retiring-game").exists()
        );
        assert_eq!(
            fs::read(world.root.join(journal::JOURNAL_FILE)).unwrap(),
            authority
        );
        eprintln!(
            "[f3rb-process-death] pid={pid} checkpoint={point} restarted=true authorityUnchanged=true"
        );
        drop(_game);
        drop(_lifecycle);
    }
}
impl Drop for World {
    fn drop(&mut self) {
        // Only World::new registers a positively created disposable root.
        // Caller-supplied gate/desktop worlds never acquire cleanup authority.
        // Preserve fixtures on assertion failure, including during unwinding.
        if !self.cleanup || std::thread::panicking() {
            return;
        }
        assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            self.root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-f3rb-")
        );
        assert!(!redirected(&fs::symlink_metadata(&self.root).unwrap()));
        let sentinel = self.root.join("owner-sentinel");
        assert!(!redirected(&fs::symlink_metadata(&sentinel).unwrap()));
        assert_eq!(fs::read(sentinel).unwrap(), b"owner");
        fs::remove_dir_all(&self.root).unwrap(); // World::new's exact disposable root
    }
}

#[test]
fn policy_grace_latest_active_repetition_and_storage_accounting() {
    let world = World::new();
    let (old, recent, active) = world.three();
    assert_eq!(world.run(GRACE_SECONDS).retired_backups, 0);
    let start = Instant::now();
    let result = world.run(GRACE_SECONDS + 1);
    assert_eq!(result.retired_backups, 1);
    assert!(result.reclaimed_bytes > 12);
    assert_eq!(
        result.verified_backup_bytes_before - result.reclaimed_bytes,
        result.verified_backup_bytes_after
    );
    assert!(!world.slot(&old).join("previous-game").exists());
    for id in [&recent, &active] {
        assert!(world.slot(id).join("previous-game").exists());
    }
    assert!(world.slot(&old).join("journal.json").exists());
    let repeat = world.run(GRACE_SECONDS + 2);
    assert_eq!(repeat.reclaimed_bytes, 0);
    assert_eq!(repeat.retired_backups, 0);
    assert!(
        matches!(super::super::validate_game_directory(&world.root.join("game")), Ok(ValidationOutcome::Installed(result)) if result.status == ValidationStatus::Valid)
    );
    eprintln!(
        "[f3rb-measurement] before={} after={} reclaimed={} eligible={} protected={} retired={} validation_ms={} maintenance_ms={} wall_us={}",
        result.verified_backup_bytes_before,
        result.verified_backup_bytes_after,
        result.reclaimed_bytes,
        result.eligible_backups,
        result.protected_backups,
        result.retired_backups,
        result.validation_ms,
        result.elapsed_ms,
        start.elapsed().as_micros()
    );
}

#[test]
fn future_clock_and_equal_time_ties_preserve_all() {
    let world = World::new();
    let (old, recent, _) = world.three();
    world.dated(&old, 2);
    assert_eq!(world.run(u64::MAX).retired_backups, 0);
    world.dated(&old, 100);
    world.dated(&recent, 200);
    assert_eq!(world.run(50).retired_backups, 0);
}

#[test]
fn all_unresolved_states_manual_repair_and_invalid_current_preserve_backups() {
    for kind in [
        "Prepared",
        "Replacing",
        "Promoting",
        "Promoted",
        "Restoring",
        "Restored",
        "Abandoned",
        "corrupt",
        "missing",
        "damage",
    ] {
        let world = World::new();
        let (old, _, _) = world.three();
        let path = world.root.join(journal::JOURNAL_FILE);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match kind {
            "corrupt" => fs::write(&path, b"{").unwrap(),
            "missing" => fs::remove_file(&path).unwrap(),
            "damage" => fs::write(world.root.join("game/client.jar"), b"damage").unwrap(),
            _ => {
                value["stage"] =
                    serde_json::json!(format!("{}{}", kind[..1].to_lowercase(), &kind[1..]));
                fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
        }
        let result = world.run(u64::MAX);
        assert!(result.skipped.is_some(), "{kind}");
        assert_eq!(result.reclaimed_bytes, 0);
        assert!(world.slot(&old).join("previous-game/client.jar").exists());
    }
}

#[test]
fn legacy_missing_corrupt_future_or_wrong_identity_metadata_never_enrolls() {
    for kind in [
        "missing",
        "corrupt",
        "future",
        "instance",
        "seal",
        "journal",
        "unknownHistory",
        "pendingHistory",
        "missingTree",
    ] {
        let world = World::new();
        let (old, _, _) = world.three();
        let slot = world.slot(&old);
        let path = slot.join(RECEIPT);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match kind {
            "missing" => fs::remove_file(&path).unwrap(),
            "corrupt" => fs::write(&path, b"{").unwrap(),
            "future" => {
                value["schemaVersion"] = 99.into();
                fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            "instance" => {
                value["journal"]["instanceId"] = "wrong".into();
                fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            "seal" => fs::write(slot.join("previous-game").join(SEAL), b"{}").unwrap(),
            "journal" => fs::write(slot.join("journal.json"), b"{").unwrap(),
            "unknownHistory" => {
                fs::create_dir(world.root.join(journal::HISTORY_DIR).join("unknown")).unwrap();
            }
            "pendingHistory" => {
                let p = slot.join("journal.json");
                let mut v: serde_json::Value =
                    serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
                v["stage"] = "promoted".into();
                fs::write(p, serde_json::to_vec(&v).unwrap()).unwrap();
            }
            "missingTree" => {
                fs::rename(slot.join("previous-game"), world.root.join("displaced")).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(world.run(u64::MAX).reclaimed_bytes, 0, "{kind}");
    }
}

#[test]
fn owner_content_modified_files_and_empty_directories_are_preserved() {
    for kind in [
        "extraFile",
        "extraDirectory",
        "modifiedNative",
        "modifiedClient",
        "extraSlot",
    ] {
        let world = World::new();
        let (old, _, _) = world.three();
        let backup = world.slot(&old).join("previous-game");
        let sentinel = match kind {
            "extraFile" => backup.join("owner-world.dat"),
            "extraDirectory" => {
                fs::create_dir(backup.join("owner-empty")).unwrap();
                backup.join("client.jar")
            }
            "modifiedNative" => backup.join("natives/native.dll"),
            "modifiedClient" => backup.join("client.jar"),
            "extraSlot" => world.slot(&old).join("owner-note.txt"),
            _ => unreachable!(),
        };
        if kind != "extraDirectory" {
            fs::write(&sentinel, b"owner").unwrap();
        }
        let before = fs::read(&sentinel).unwrap();
        assert_eq!(world.run(u64::MAX).reclaimed_bytes, 0, "{kind}");
        assert_eq!(fs::read(&sentinel).unwrap(), before);
    }
}

#[test]
fn damaged_newer_backup_cannot_evict_last_good_backup() {
    let world = World::new();
    let (old, recent, _) = world.three();
    fs::write(
        world.slot(&recent).join("previous-game/client.jar"),
        b"owner",
    )
    .unwrap();
    assert_eq!(world.run(u64::MAX).retired_backups, 0);
    assert!(world.slot(&old).join("previous-game/client.jar").exists());
}

#[test]
fn modified_seal_before_replacement_cannot_adopt_owner_files() {
    let world = World::new();
    world.replace();
    let game = world.root.join("game");
    fs::write(game.join("owner.dat"), b"owner").unwrap();
    let mut seal: Seal = serde_json::from_slice(&fs::read(game.join(SEAL)).unwrap()).unwrap();
    seal.files
        .push(OwnedFile::observed("owner.dat".into(), b"owner"));
    write_json(&game.join(SEAL), &seal).unwrap();
    let replacement = world.replace();
    assert!(!world.slot(&replacement).join(RECEIPT).exists());
    world.replace();
    world.replace();
    let result = world.run(u64::MAX);
    assert_eq!(
        fs::read(world.slot(&replacement).join("previous-game/owner.dat")).unwrap(),
        b"owner"
    );
    assert!(
        result
            .entries
            .iter()
            .any(|entry| entry.transaction_id == replacement
                && entry.outcome == Outcome::RetainedUnknownOwnership)
    );
}

#[test]
fn ownership_seal_precedes_and_does_not_replace_the_completion_marker() {
    let world = World::new();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let game = world.root.join(".install-staging/game");
    world.game(&game, &id);
    let manifest = super::super::state::load_installed_state(&game)
        .unwrap()
        .unwrap();
    fs::remove_file(game.join(SEAL)).unwrap();
    fs::remove_file(game.join(INSTALLED_GAME_FILE_NAME)).unwrap();
    seal_staging(
        &game,
        &manifest,
        vec![OwnedFile::observed("natives/native.dll".into(), b"native")],
    )
    .unwrap();
    assert!(game.join(SEAL).exists());
    assert!(matches!(
        super::super::validate_game_directory(&game).unwrap(),
        ValidationOutcome::NotInstalled
    ));
    fs::write(game.join(INSTALLED_GAME_FILE_NAME), manifest.to_json()).unwrap();
    assert!(
        matches!(super::super::validate_game_directory(&game).unwrap(), ValidationOutcome::Installed(result) if result.status == ValidationStatus::Valid)
    );
}

#[test]
fn interrupted_retirement_replays_every_boundary_without_changing_authority() {
    for point in [
        Point::Intent,
        Point::Renamed,
        Point::FileRemoved,
        Point::Empty,
        Point::Finished,
    ] {
        let world = World::new();
        let (old, _, _) = world.three();
        let authority = fs::read(world.root.join(journal::JOURNAL_FILE)).unwrap();
        let report = maintain_locked(&world.root, &world.id, u64::MAX, |at| {
            if at == point {
                Err(io::Error::other("interrupted"))
            } else {
                Ok(())
            }
        });
        assert!(report.entries.iter().any(|entry| matches!(
            entry.outcome,
            Outcome::CleanupInterrupted | Outcome::CleanupFailedSafely
        )));
        world.run(u64::MAX);
        world.run(u64::MAX);
        assert!(!world.slot(&old).join("previous-game").exists());
        assert!(!world.slot(&old).join("retiring-game").exists());
        assert_eq!(
            fs::read(world.root.join(journal::JOURNAL_FILE)).unwrap(),
            authority
        );
    }
}

#[test]
fn partial_cleanup_with_new_owner_content_refuses_further_deletion() {
    let world = World::new();
    let (old, _, _) = world.three();
    maintain_locked(&world.root, &world.id, u64::MAX, |point| {
        if point == Point::FileRemoved {
            Err(unsafe_path())
        } else {
            Ok(())
        }
    });
    let tree = world.slot(&old).join("retiring-game");
    fs::write(tree.join("owner.txt"), b"owner").unwrap();
    assert_eq!(world.run(u64::MAX).reclaimed_bytes, 0);
    assert_eq!(fs::read(tree.join("owner.txt")).unwrap(), b"owner");
}

#[test]
fn path_traversal_aliases_and_corrupted_seals_are_rejected() {
    for path in [
        "../owner",
        "/owner",
        "C:/owner",
        "a\\b",
        "a/./b",
        "a//b",
        "a/nul.dat",
        "a/COM1",
        "a/end.",
        "a:b",
    ] {
        assert!(safe_relative(path).is_err(), "{path}");
    }
    let mut seal = Seal {
        schema_version: 1,
        installation_id: uuid::Uuid::new_v4().simple().to_string(),
        files: vec![OwnedFile::observed(
            INSTALLED_GAME_FILE_NAME.into(),
            b"manifest",
        )],
    };
    seal.files
        .push(OwnedFile::observed("Installed-game.json".into(), b"owner"));
    assert!(seal.validate().is_err());
    seal.files.pop();
    seal.schema_version = 99;
    assert!(seal.validate().is_err());
}

/// The seal must bind exactly the independently-constructed observations:
/// one per manifest file, the extraction observations for natives, and the
/// in-memory completion-manifest bytes — byte-for-byte, in sorted path order.
#[test]
fn sealing_matches_independent_reference_observation() {
    let world = World::new();
    let id = uuid::Uuid::new_v4().simple().to_string();
    let game = world.root.join(".install-staging/game");
    fs::create_dir_all(game.join("natives")).unwrap();
    fs::write(game.join("client.jar"), b"client").unwrap();
    fs::write(game.join("natives/native.dll"), b"native").unwrap();
    let manifest = InstalledGameManifest::for_platform(
        "1.20.1",
        PlatformPin::Vanilla {},
        id.clone(),
        1,
        vec![InstalledFile::new(
            InstalledFileRole::Client,
            "client.jar".into(),
            ArtifactTrust::ExpectedDigestVerified {
                algorithm: DigestAlgorithm::Sha256,
                digest: hash(b"client"),
            },
            6,
        )],
        NativesRecord::new("natives".into()),
    );
    fs::write(game.join(INSTALLED_GAME_FILE_NAME), manifest.to_json()).unwrap();
    seal_staging(
        &game,
        &manifest,
        vec![OwnedFile::observed("natives/native.dll".into(), b"native")],
    )
    .unwrap();
    let mut reference = BTreeMap::new();
    reference.insert(
        "client.jar".to_owned(),
        OwnedFile::observed("client.jar".into(), b"client"),
    );
    reference.insert(
        "natives/native.dll".to_owned(),
        OwnedFile::observed("natives/native.dll".into(), b"native"),
    );
    reference.insert(
        INSTALLED_GAME_FILE_NAME.to_owned(),
        OwnedFile::observed(
            INSTALLED_GAME_FILE_NAME.into(),
            manifest.to_json().as_bytes(),
        ),
    );
    let expected = serde_json::to_vec(&Seal {
        schema_version: 1,
        installation_id: id,
        files: reference.into_values().collect(),
    })
    .unwrap();
    assert_eq!(fs::read(game.join(SEAL)).unwrap(), expected);
}

/// Sealing refuses everything the installer did not account for, and never
/// overwrites a stale seal from an interrupted attempt.
#[test]
fn sealing_refuses_unaccounted_staging_content() {
    for kind in [
        "extraFile",
        "missingFile",
        "staleSeal",
        "nativeMismatch",
        "completionMismatch",
        "unknownDir",
    ] {
        let world = World::new();
        let id = uuid::Uuid::new_v4().simple().to_string();
        let game = world.root.join(".install-staging/game");
        fs::create_dir_all(game.join("natives")).unwrap();
        fs::write(game.join("client.jar"), b"client").unwrap();
        fs::write(game.join("natives/native.dll"), b"native").unwrap();
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
                    digest: hash(b"client"),
                },
                6,
            )],
            NativesRecord::new("natives".into()),
        );
        match kind {
            "extraFile" => fs::write(game.join("extra.txt"), b"x").unwrap(),
            "missingFile" => fs::remove_file(game.join("client.jar")).unwrap(),
            "staleSeal" => fs::write(game.join(SEAL), b"{}").unwrap(),
            "nativeMismatch" => fs::write(game.join("natives/native.dll"), b"tampered").unwrap(),
            "completionMismatch" => {
                fs::write(game.join(INSTALLED_GAME_FILE_NAME), b"different").unwrap()
            }
            "unknownDir" => fs::create_dir(game.join("owner-empty")).unwrap(),
            _ => unreachable!(),
        }
        let result = seal_staging(
            &game,
            &manifest,
            vec![OwnedFile::observed("natives/native.dll".into(), b"native")],
        );
        assert!(result.is_err(), "{kind}");
        if kind == "staleSeal" {
            assert_eq!(fs::read(game.join(SEAL)).unwrap(), b"{}");
        }
    }
}

#[test]
fn locks_cover_same_instance_and_allow_different_instances() {
    let root = std::env::temp_dir().join(format!("aurora-f3rb-locks-{}", uuid::Uuid::new_v4()));
    let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
    let a = InstanceId::new("target-a").unwrap();
    let b = InstanceId::new("target-b").unwrap();
    let lifecycle = crate::instances::lifecycle::lock_instance_operation(&managed, &a).unwrap();
    assert!(matches!(
        maintain(&managed, &a),
        Err(InstallError::AlreadyInProgress { .. })
    ));
    let other = crate::instances::lifecycle::lock_instance_operation(&managed, &b).unwrap();
    drop(other);
    drop(lifecycle);
    let game = super::super::lock_game_target(&managed, &a).unwrap();
    assert!(matches!(
        maintain(&managed, &a),
        Err(InstallError::AlreadyInProgress { .. })
    ));
    drop(game);
    fs::remove_dir_all(root).unwrap();
}

/// Opt-in F3R-B1 ownership-sealing benchmark on a deterministic disposable
/// fixture sized near the measured warm Vanilla profile (4,669 files /
/// ~570.9 MB). Exercises the real `seal_staging` code path plus decomposition
/// replicas (enumeration, repeated raw hash passes, per-file containment).
/// Accepts only a fresh direct temp child with the aurora-f3rb1- prefix.
#[test]
fn ownership_sealing_benchmark_helper() {
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
    }
    fn write_seeded(path: &Path, size: usize, rng: &mut Rng) -> String {
        use std::io::Write;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = io::BufWriter::with_capacity(4 * 1024 * 1024, File::create(path).unwrap());
        let mut sha = Sha256::new();
        let mut remaining = size;
        while remaining > 0 {
            let take = remaining.min(4 * 1024 * 1024);
            let mut block = Vec::with_capacity(take);
            while block.len() < take {
                let word = rng.next().to_le_bytes();
                let copy = (take - block.len()).min(word.len());
                block.extend_from_slice(&word[..copy]);
            }
            file.write_all(&block).unwrap();
            sha.update(&block);
            remaining -= take;
        }
        file.flush().unwrap();
        drop(file);
        crate::integrity::ArtifactDigest::from_sha256(sha.finalize().into()).as_hex()
    }
    let Some(root) = std::env::var_os("AURORA_F3RB1_SEAL_BENCH") else {
        return;
    };
    let root = PathBuf::from(root);
    assert!(
        root.is_absolute()
            && root.parent() == Some(std::env::temp_dir().as_path())
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-f3rb1-")
            && !root.exists()
    );
    fs::create_dir(&root).unwrap();
    let started = Instant::now();
    let mut rng = Rng(0xF3B1_5EED_0000_0001);
    let mut entries: Vec<(String, usize, InstalledFileRole)> = Vec::new();
    let mut digests: BTreeMap<String, String> = BTreeMap::new();
    let stage = |path: String,
                 size: usize,
                 role: InstalledFileRole,
                 rng: &mut Rng,
                 entries: &mut Vec<(String, usize, InstalledFileRole)>,
                 digests: &mut BTreeMap<String, String>| {
        let digest = write_seeded(&root.join(&path), size, rng);
        entries.push((path.clone(), size, role));
        digests.insert(path, digest);
    };
    stage(
        "client.jar".into(),
        26_214_400,
        InstalledFileRole::Client,
        &mut rng,
        &mut entries,
        &mut digests,
    );
    for index in 0..100 {
        stage(
            format!("libraries/net/minecraft/mirror-{index}/mirror-{index}.jar"),
            2_097_152,
            InstalledFileRole::Library,
            &mut rng,
            &mut entries,
            &mut digests,
        );
    }
    for index in 0..4_500 {
        stage(
            format!("libraries/com/aurora/bench/lib-{index:04}/lib-{index:04}.jar"),
            65_536,
            InstalledFileRole::Library,
            &mut rng,
            &mut entries,
            &mut digests,
        );
    }
    for name in [
        "config/log4j2.xml",
        "asset-index/11.json",
        "version-range.json",
    ] {
        stage(
            name.into(),
            2_048,
            InstalledFileRole::AssetIndex,
            &mut rng,
            &mut entries,
            &mut digests,
        );
    }
    let mut native_ownership = Vec::new();
    for index in 0..12 {
        let path = format!("natives/win/x64/native-{index:02}.dll");
        let digest = write_seeded(&root.join(&path), 131_072, &mut rng);
        let bytes = fs::read(root.join(&path)).unwrap();
        assert_eq!(hash(&bytes), digest);
        native_ownership.push(OwnedFile::observed(path, &bytes));
    }
    let total_bytes: u64 =
        entries.iter().map(|(_, size, _)| *size as u64).sum::<u64>() + 12 * 131_072;
    let manifest = InstalledGameManifest::for_platform(
        "1.21.11",
        PlatformPin::Vanilla {},
        // Fixed identity so the seal digest is comparable across A/B builds.
        "fa3b10000000000000000000000000b1".to_owned(),
        1,
        entries
            .into_iter()
            .map(|(path, size, role)| {
                InstalledFile::new(
                    role,
                    path.clone(),
                    ArtifactTrust::ExpectedDigestVerified {
                        algorithm: DigestAlgorithm::Sha256,
                        digest: digests[&path].clone(),
                    },
                    size as u64,
                )
            })
            .collect(),
        NativesRecord::new("natives".into()),
    );
    let generated_ms = started.elapsed().as_millis();
    let file_count = manifest.files().len() + native_ownership.len();

    // Deterministic file list shared by all decomposition replicas.
    let mut files: Vec<PathBuf> = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    let enumerate = |label: &str| {
        let started = Instant::now();
        let mut seen = 0usize;
        let mut directories = 0usize;
        let mut pending = vec![root.clone()];
        while let Some(directory) = pending.pop() {
            directories += 1;
            for entry in fs::read_dir(&directory).unwrap() {
                let path = entry.unwrap().path();
                let meta = fs::symlink_metadata(&path).unwrap();
                if redirected(&meta) || (!meta.is_dir() && !meta.is_file()) {
                    panic!("unexpected fixture entry");
                }
                if meta.is_dir() {
                    pending.push(path);
                } else {
                    seen += 1;
                }
            }
        }
        eprintln!(
            "[f3rb1-bench] phase={label} elapsed_us={} files={seen} dirs={directories}",
            started.elapsed().as_micros()
        );
        seen
    };
    let hash_pass = |label: &str| {
        let started = Instant::now();
        let mut bytes = 0u64;
        for path in &files {
            let mut file = File::open(path).unwrap();
            let mut sha = Sha256::new();
            bytes += io::copy(&mut file, &mut sha).unwrap();
        }
        eprintln!(
            "[f3rb1-bench] phase={label} elapsed_us={} bytes={bytes}",
            started.elapsed().as_micros()
        );
    };
    let containment_pass = |label: &str| {
        let started = Instant::now();
        for path in &files {
            let parent = path.parent().unwrap();
            checked_directory(&root, parent).unwrap();
            let meta = fs::symlink_metadata(path).unwrap();
            assert!(meta.is_file());
        }
        eprintln!(
            "[f3rb1-bench] phase={label} elapsed_us={} files={}",
            started.elapsed().as_micros(),
            files.len()
        );
    };
    // Warm caches once with a full read before any timed measurement.
    hash_pass("hash_warmup_discarded");
    enumerate("enumerate_1");
    enumerate("enumerate_2");
    hash_pass("hash_pass_1");
    hash_pass("hash_pass_2");
    hash_pass("hash_pass_3");
    containment_pass("containment_pass_1");
    containment_pass("containment_pass_2");

    let mut seal_samples = Vec::new();
    let mut seal_digests = BTreeSet::new();
    for _ in 0..5 {
        let _ = fs::remove_file(root.join(SEAL));
        let started = Instant::now();
        seal_staging(&root, &manifest, native_ownership.clone()).unwrap();
        seal_samples.push(started.elapsed().as_micros());
        seal_digests.insert(hash(&fs::read(root.join(SEAL)).unwrap()));
    }
    assert_eq!(seal_digests.len(), 1, "sealing must be deterministic");
    let mut sorted = seal_samples.clone();
    sorted.sort_unstable();
    let median = sorted[sorted.len() / 2];
    eprintln!("[f3rb1-bench] phase=seal_samples elapsed_us={seal_samples:?} median_us={median}");
    eprintln!(
        "[f3rb1-bench] phase=summary file_count={file_count} total_bytes={total_bytes} generated_ms={generated_ms} seal_sha256={}",
        seal_digests.iter().next().unwrap()
    );
    // The fixture stays on disk for inspection; the driver owns this root.
}

#[cfg(windows)]
#[test]
fn windows_junction_and_locked_leaf_preserve_owner_data_and_resume() {
    use std::os::windows::{fs::OpenOptionsExt, process::CommandExt};
    let world = World::new();
    let (old, _, _) = world.three();
    let outside = world.root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("owner"), b"owner").unwrap();
    let link = world.slot(&old).join("previous-game/redirected");
    let status = std::process::Command::new("powershell.exe")
        .creation_flags(0x08000000)
        .env("F3RB_LINK", &link)
        .env("F3RB_OUTSIDE", &outside)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "New-Item -ItemType Junction -Path $env:F3RB_LINK -Target $env:F3RB_OUTSIDE | Out-Null",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(world.run(u64::MAX).reclaimed_bytes, 0);
    assert_eq!(fs::read(outside.join("owner")).unwrap(), b"owner");
    fs::remove_dir(&link).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(world.slot(&old).join("previous-game/client.jar"))
        .unwrap();
    let failed = world.run(u64::MAX);
    assert_eq!(failed.retired_backups, 0);
    drop(held);
    assert_eq!(world.run(u64::MAX).retired_backups, 1);
}

/// A reparse point substituted AFTER eligibility was assessed and durable
/// Deleting intent exists must still refuse every remaining deletion; the
/// retirement only completes once the tree is exactly the sealed content.
#[cfg(windows)]
#[test]
fn junction_substituted_after_eligibility_refuses_retirement() {
    use std::os::windows::process::CommandExt;
    let world = World::new();
    let (old, _, _) = world.three();
    let outside = world.root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("owner"), b"owner").unwrap();
    let slot = world.slot(&old);
    // Stop after durable Deleting intent but before any mutation of the tree.
    maintain_locked(&world.root, &world.id, u64::MAX, |point| {
        if point == Point::Intent {
            Err(io::Error::other("interrupted after intent"))
        } else {
            Ok(())
        }
    });
    let link = slot.join("previous-game/redirected");
    let status = std::process::Command::new("powershell.exe")
        .creation_flags(0x0800_0000)
        .env("F3RB1_SUBSTITUTION_LINK", &link)
        .env("F3RB1_SUBSTITUTION_OUTSIDE", &outside)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "New-Item -ItemType Junction -Path $env:F3RB1_SUBSTITUTION_LINK -Target $env:F3RB1_SUBSTITUTION_OUTSIDE | Out-Null",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let refused = world.run(u64::MAX);
    assert_eq!(refused.reclaimed_bytes, 0);
    assert_eq!(refused.retired_backups, 0);
    assert!(
        refused
            .entries
            .iter()
            .any(|entry| entry.transaction_id == old
                && matches!(
                    entry.outcome,
                    Outcome::CleanupInterrupted | Outcome::CleanupFailedSafely
                ))
    );
    assert_eq!(fs::read(outside.join("owner")).unwrap(), b"owner");
    assert!(slot.join("previous-game/client.jar").exists());
    fs::remove_dir(&link).unwrap();
    assert_eq!(world.run(u64::MAX).retired_backups, 1);
    assert_eq!(world.run(u64::MAX).retired_backups, 0);
}
