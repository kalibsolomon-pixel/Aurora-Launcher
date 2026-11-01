use super::*;
use crate::instances::InstanceRecord;
use crate::instances::platform::{InstalledConfiguration, PlatformPin};
use crate::instances::settings::InstanceConfiguration;

fn fixture() -> (ManagedPaths, Request) {
    let root = std::env::temp_dir().join(format!(
        "aurora-pandora-sync-{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir(&root).unwrap();
    let managed = ManagedPaths::from_app_local_data_dir(root).unwrap();
    let source_id = crate::instances::generate_instance_id();
    let destination_id = crate::instances::generate_instance_id();
    let mut registry = InstanceRegistry::empty();
    for id in [&source_id, &destination_id] {
        let mut configuration = InstanceConfiguration::for_minecraft_version("1.21.11");
        configuration.set_aurora_enabled(false);
        let record = InstanceRecord::from_installed(
            id.clone(),
            "Sync fixture",
            InstanceState::Ready,
            InstalledConfiguration {
                minecraft_version: "1.21.11".into(),
                platform: PlatformPin::Vanilla {},
                aurora: None,
            },
            configuration,
        )
        .unwrap();
        registry.instances_mut().push(record);
        fs::create_dir_all(managed.instance_paths(id).root()).unwrap();
    }
    registry.save(&managed.instance_registry_file()).unwrap();
    set_enabled(&managed, true).unwrap();
    (
        managed,
        Request {
            source_id,
            destination_id,
            category: Category::Settings,
            exclusions: vec![],
            selected_paths: vec![],
        },
    )
}
fn put(managed: &ManagedPaths, id: &InstanceId, relative: &str, bytes: &[u8]) {
    let path = managed.instance_paths(id).root().join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
fn get(managed: &ManagedPaths, id: &InstanceId, relative: &str) -> Vec<u8> {
    fs::read(managed.instance_paths(id).root().join(relative)).unwrap()
}

#[test]
fn options_merge_preserves_local_pack_order_unknown_keys_and_keeps_backup() {
    let (managed, request) = fixture();
    let before = b"version:1\nmouseSensitivity:0.25\nresourcePacks:[\"target\"]\nmodOnly:keep\nkey_jump:key.keyboard.space\n";
    put(&managed, &request.source_id, "options.txt", b"version:2\nmouseSensitivity:0.75\nresourcePacks:[\"source\"]\nmodOnly:replace\nsourceOnly:ignored\nkey_jump:key.keyboard.j\n");
    put(&managed, &request.destination_id, "options.txt", before);
    let preview = preview(&managed, &request).unwrap();
    assert_eq!(preview.entries[0].changed_keys, ["mouseSensitivity"]);
    let receipt = apply(&managed, &request, &preview.fingerprint).unwrap();
    let result = String::from_utf8(get(&managed, &request.destination_id, "options.txt")).unwrap();
    assert!(result.contains("mouseSensitivity:0.75\n"));
    for line in [
        "version:1\n",
        "resourcePacks:[\"target\"]\n",
        "modOnly:keep\n",
        "key_jump:key.keyboard.space\n",
    ] {
        assert!(result.contains(line));
    }
    assert!(!result.contains("sourceOnly"));
    assert_eq!(
        fs::read(
            managed
                .launcher_dir()
                .join("sync-backups")
                .join(receipt.backup_id.unwrap())
                .join("options.txt")
        )
        .unwrap(),
        before
    );
    assert_eq!(super::preview(&managed, &request).unwrap().changes, 0);
}

#[test]
fn controls_are_separate_and_duplicate_options_fail_closed() {
    let (managed, mut request) = fixture();
    request.category = Category::Controls;
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:1\nkey_jump:key.keyboard.j\nkey_mod:key.keyboard.m\n",
    );
    put(
        &managed,
        &request.destination_id,
        "options.txt",
        b"mouseSensitivity:0\nkey_jump:key.keyboard.space\n",
    );
    let p = preview(&managed, &request).unwrap();
    assert_eq!(p.entries[0].changed_keys, ["key_jump"]);
    apply(&managed, &request, &p.fingerprint).unwrap();
    assert_eq!(
        get(&managed, &request.destination_id, "options.txt"),
        b"mouseSensitivity:0\nkey_jump:key.keyboard.j\n"
    );
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"key_jump:a\nkey_jump:b\n",
    );
    assert_eq!(
        preview(&managed, &request).unwrap_err().code,
        "sync_options_invalid"
    );
}

#[test]
fn stale_source_destination_exclusions_or_disable_prevent_mutation() {
    let (managed, mut request) = fixture();
    request.category = Category::Screenshots;
    put(&managed, &request.source_id, "screenshots/a.png", b"first");
    let p = preview(&managed, &request).unwrap();
    put(&managed, &request.source_id, "screenshots/a.png", b"other");
    assert_eq!(
        apply(&managed, &request, &p.fingerprint).unwrap_err().code,
        "sync_review_stale"
    );
    let p = preview(&managed, &request).unwrap();
    put(
        &managed,
        &request.destination_id,
        "screenshots/a.png",
        b"local",
    );
    assert_eq!(
        apply(&managed, &request, &p.fingerprint).unwrap_err().code,
        "sync_review_stale"
    );
    let p = preview(&managed, &request).unwrap();
    request.exclusions.push("screenshots/a.png".into());
    assert_eq!(
        apply(&managed, &request, &p.fingerprint).unwrap_err().code,
        "sync_review_stale"
    );
    set_enabled(&managed, false).unwrap();
    assert_eq!(
        apply(&managed, &request, &p.fingerprint).unwrap_err().code,
        "sync_disabled"
    );
    assert_eq!(
        get(&managed, &request.destination_id, "screenshots/a.png"),
        b"local"
    );
}

#[test]
fn additive_sync_keeps_conflicts_excludes_paths_and_never_links_instances() {
    let (managed, mut request) = fixture();
    request.category = Category::ResourcePacks;
    request.exclusions.push("resourcepacks/private".into());
    put(
        &managed,
        &request.source_id,
        "resourcepacks/.aurora-sync-0123456789abcdef0123456789abcdef",
        b"interrupted uncommitted bytes",
    );
    for (path, bytes) in [
        ("resourcepacks/add.zip", b"new".as_slice()),
        ("resourcepacks/conflict.zip", b"source"),
        ("resourcepacks/private/secret", b"excluded"),
    ] {
        put(&managed, &request.source_id, path, bytes);
    }
    put(
        &managed,
        &request.destination_id,
        "resourcepacks/conflict.zip",
        b"local",
    );
    let p = preview(&managed, &request).unwrap();
    assert_eq!((p.changes, p.conflicts, p.entries.len()), (1, 1, 2));
    let receipt = apply(&managed, &request, &p.fingerprint).unwrap();
    assert_eq!((receipt.copied, receipt.conflicts_kept), (1, 1));
    put(
        &managed,
        &request.destination_id,
        "resourcepacks/add.zip",
        b"destination edited",
    );
    assert_eq!(
        get(&managed, &request.source_id, "resourcepacks/add.zip"),
        b"new"
    );
    assert_eq!(
        get(
            &managed,
            &request.destination_id,
            "resourcepacks/conflict.zip"
        ),
        b"local"
    );
    assert!(
        !managed
            .instance_paths(&request.destination_id)
            .root()
            .join("resourcepacks/private")
            .exists()
    );
}

#[test]
fn preferences_default_off_and_malformed_unknown_schema_are_never_overwritten() {
    let (managed, _) = fixture();
    assert!(!Policy::default().enabled);
    for bytes in [
        b"broken".as_slice(),
        br#"{"schemaVersion":99,"enabled":true}"#,
        br#"{"schemaVersion":1,"enabled":true,"extra":1}"#,
    ] {
        fs::write(policy_file(&managed), bytes).unwrap();
        assert!(set_enabled(&managed, false).is_err());
        assert_eq!(fs::read(policy_file(&managed)).unwrap(), bytes);
    }
}

#[test]
fn active_games_preparations_and_other_writers_are_excluded() {
    let (managed, mut request) = fixture();
    request.category = Category::Screenshots;
    let running = crate::launch::process::force_running(request.source_id.as_str());
    assert!(preview(&managed, &request).is_err());
    drop(running);
    let preparing =
        crate::launch::process::PreparationGuard::acquire(request.destination_id.as_str()).unwrap();
    assert!(preview(&managed, &request).is_err());
    drop(preparing);
    let other_writer =
        crate::instances::lifecycle::lock_instance_operation(&managed, &request.source_id).unwrap();
    assert!(preview(&managed, &request).is_err());
    drop(other_writer);
    assert!(preview(&managed, &request).is_ok());
    crate::instance_content::with_instance_lock(&request.source_id, || {
        assert_eq!(
            preview(&managed, &request).unwrap_err().code,
            "sync_instance_busy"
        );
        Ok(())
    })
    .unwrap();
    // An unrelated running instance does not prevent this sync.
    let id = crate::instances::generate_instance_id();
    let _unrelated = crate::launch::process::force_running(id.as_str());
    assert!(preview(&managed, &request).is_ok());
}

#[test]
fn unsafe_names_and_unsupported_categories_are_rejected() {
    for path in [
        "../saves",
        "C:/outside",
        "resourcepacks\\outside",
        "x//y",
        "x/NUL.zip",
        "x/trailing.",
        "x/stream:ads",
    ] {
        assert!(validate_relative(path).is_err(), "{path}");
    }
    assert!(serde_json::from_str::<Category>("\"worlds\"").is_err());
    assert!(serde_json::from_str::<Category>("\"servers\"").is_err());
}

#[cfg(windows)]
#[test]
fn junctions_at_source_are_refused() {
    use std::process::Command;
    let (managed, mut request) = fixture();
    request.category = Category::ShaderPacks;
    let outside = managed.data_root().join("outside-sentinel");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep.zip"), b"sentinel").unwrap();
    let source = managed
        .instance_paths(&request.source_id)
        .shaderpacks()
        .to_path_buf();
    // Structured cmd arguments solely to create this exact test junction;
    // no process search, deletion or cleanup shell is used.
    assert!(
        Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&source)
            .arg(&outside)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(preview(&managed, &request).is_err());
    assert_eq!(fs::read(outside.join("keep.zip")).unwrap(), b"sentinel");
}

#[cfg(windows)]
#[test]
fn destination_and_backup_junctions_cannot_redirect_writes() {
    use std::process::Command;
    for backup in [false, true] {
        let (managed, mut request) = fixture();
        request.category = Category::Screenshots;
        put(
            &managed,
            &request.source_id,
            "screenshots/new.png",
            b"source",
        );
        let outside = managed.data_root().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"keep").unwrap();
        let link = if backup {
            managed.launcher_dir().join("sync-backups")
        } else {
            managed
                .instance_paths(&request.destination_id)
                .root()
                .join("screenshots")
        };
        let fingerprint = if backup {
            Some(preview(&managed, &request).unwrap().fingerprint)
        } else {
            None
        };
        assert!(
            Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(&link)
                .arg(&outside)
                .output()
                .unwrap()
                .status
                .success()
        );
        if let Some(fingerprint) = fingerprint {
            assert!(apply(&managed, &request, &fingerprint).is_err());
        } else {
            assert!(preview(&managed, &request).is_err());
        }
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"keep");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    }
}

#[test]
fn interrupted_settings_sync_retains_original_or_complete_replacement_and_backup() {
    for stage in ["reviewSaved", "backupSaved", "fileCommitted"] {
        let (managed, request) = fixture();
        let original = b"mouseSensitivity:0.25\nresourcePacks:[\"local\"]\n";
        let expected = b"mouseSensitivity:0.75\nresourcePacks:[\"local\"]\n";
        put(
            &managed,
            &request.source_id,
            "options.txt",
            b"mouseSensitivity:0.75\n",
        );
        put(&managed, &request.destination_id, "options.txt", original);
        let review = preview(&managed, &request).unwrap();
        let result = apply_with_checkpoint(&managed, &request, &review.fingerprint, |point| {
            if point == stage {
                Err(fail("sync_test_interrupted", "test interruption"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.unwrap_err().code, "sync_test_interrupted");
        let bytes = get(&managed, &request.destination_id, "options.txt");
        assert_eq!(
            bytes,
            if stage == "fileCommitted" {
                expected.as_slice()
            } else {
                original.as_slice()
            }
        );
        if stage != "reviewSaved" {
            let saved = fs::read_dir(managed.launcher_dir().join("sync-backups"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            assert_eq!(fs::read(saved.join("options.txt")).unwrap(), original);
        }
        let next = preview(&managed, &request).unwrap();
        assert_eq!(next.changes, if stage == "fileCommitted" { 0 } else { 1 });
        apply(&managed, &request, &next.fingerprint).unwrap();
    }
}

#[test]
fn unknown_instance_does_not_allocate_a_directory() {
    let (managed, mut request) = fixture();
    request.destination_id = crate::instances::generate_instance_id();
    assert!(preview(&managed, &request).is_err());
    assert!(
        !managed
            .instance_paths(&request.destination_id)
            .root()
            .exists()
    );
}

#[test]
fn configuration_requires_explicit_paths_and_keeps_existing_files() {
    let (managed, mut request) = fixture();
    request.category = Category::Configuration;
    assert_eq!(
        preview(&managed, &request).unwrap_err().code,
        "sync_config_selection_required"
    );
    request.selected_paths = vec!["config/chosen".into()];
    put(
        &managed,
        &request.source_id,
        "config/chosen/new.toml",
        b"new",
    );
    put(
        &managed,
        &request.source_id,
        "config/chosen/local.toml",
        b"source",
    );
    put(
        &managed,
        &request.source_id,
        "config/unselected.toml",
        b"private",
    );
    put(
        &managed,
        &request.destination_id,
        "config/chosen/local.toml",
        b"local",
    );
    let review = preview(&managed, &request).unwrap();
    assert_eq!((review.changes, review.conflicts), (1, 1));
    apply(&managed, &request, &review.fingerprint).unwrap();
    assert_eq!(
        get(&managed, &request.destination_id, "config/chosen/new.toml"),
        b"new"
    );
    assert_eq!(
        get(
            &managed,
            &request.destination_id,
            "config/chosen/local.toml"
        ),
        b"local"
    );
    assert!(
        !managed
            .instance_paths(&request.destination_id)
            .config()
            .join("unselected.toml")
            .exists()
    );
    request.selected_paths = vec!["saves/private".into()];
    assert!(preview(&managed, &request).is_err());
}
