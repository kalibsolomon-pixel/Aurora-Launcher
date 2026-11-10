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
            key_filter: None,
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
fn pack_owned_destination_content_is_never_a_sync_target() {
    let (managed, mut request) = fixture();
    request.category = Category::ResourcePacks;
    let pack = crate::pack_state::InstalledPack::new(
        request.destination_id.clone(),
        crate::pack_state::PackIdentity {
            provider: "modrinth".into(),
            project_id: "PACKSYNC".into(),
            version_id: "VERSYNC".into(),
            name: "Sync pack".into(),
            pack_version: "1".into(),
            artifact_sha512: "a".repeat(128),
            artifact_sha256: "b".repeat(64),
            minecraft_version: "1.21.11".into(),
            loader: crate::instances::platform::PlatformPin::Fabric {
                version: "0.19.5".into(),
            },
            installed_at_unix_seconds: 1,
        },
        vec![crate::pack_state::OwnedComponent {
            path: "resourcepacks/pack-owned.zip".into(),
            sha256: "c".repeat(64),
            sha512: "d".repeat(128),
            provider: None,
            provider_version_id: None,
        }],
        vec![],
        vec![],
    )
    .unwrap();
    pack.save(&managed).unwrap();
    // Deterministic pack content differs between the instances: it must not
    // become a copy, replace, or conflict candidate. Unowned files in the
    // same category still sync normally.
    put(
        &managed,
        &request.source_id,
        "resourcepacks/pack-owned.zip",
        b"other instance copy",
    );
    put(
        &managed,
        &request.destination_id,
        "resourcepacks/pack-owned.zip",
        b"pack bytes",
    );
    put(
        &managed,
        &request.source_id,
        "resourcepacks/free.zip",
        b"free",
    );
    let p = preview(&managed, &request).unwrap();
    assert!(
        !p.entries
            .iter()
            .any(|entry| entry.path == "resourcepacks/pack-owned.zip"),
        "{:?}",
        p.entries
    );
    let receipt = apply(&managed, &request, &p.fingerprint).unwrap();
    assert_eq!(receipt.copied, 1);
    assert_eq!(receipt.conflicts_kept, 0);
    assert_eq!(
        get(
            &managed,
            &request.destination_id,
            "resourcepacks/pack-owned.zip"
        ),
        b"pack bytes"
    );
    assert_eq!(
        get(&managed, &request.destination_id, "resourcepacks/free.zip"),
        b"free"
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
    assert_eq!(
        serde_json::from_str::<Category>("\"servers\"").unwrap(),
        Category::Servers
    );
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

fn automatic_policy(
    managed: &ManagedPaths,
    request: &Request,
    categories: Vec<Category>,
) -> Policy {
    save_policy(
        managed,
        Policy {
            enabled: true,
            instances: vec![request.source_id.clone(), request.destination_id.clone()],
            categories,
            ..Policy::default()
        },
    )
    .unwrap()
}

#[test]
fn automatic_settings_follow_unique_edits_and_keep_simultaneous_conflicts() {
    let (managed, request) = fixture();
    automatic_policy(
        &managed,
        &request,
        vec![Category::Settings, Category::Controls],
    );
    let original =
        b"mouseSensitivity:0.25\nkey_jump:key.keyboard.space\nresourcePacks:[\"local\"]\n";
    for id in [&request.source_id, &request.destination_id] {
        put(&managed, id, "options.txt", original);
    }
    let mut engine = automatic::Engine::default();
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 0);
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:0.75\nkey_jump:key.keyboard.j\nresourcePacks:[\"source\"]\n",
    );
    assert!(engine.cycle(&managed).unwrap().files_changed > 0);
    assert_eq!(
        get(&managed, &request.destination_id, "options.txt"),
        b"mouseSensitivity:0.75\nkey_jump:key.keyboard.j\nresourcePacks:[\"local\"]\n"
    );
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 0);
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:0.6\nkey_jump:key.keyboard.j\n",
    );
    put(
        &managed,
        &request.destination_id,
        "options.txt",
        b"mouseSensitivity:0.4\nkey_jump:key.keyboard.j\nresourcePacks:[\"local\"]\n",
    );
    let conflict = engine.cycle(&managed).unwrap();
    assert_eq!(conflict.files_changed, 0);
    assert!(conflict.conflicts_kept > 0);
    assert!(
        String::from_utf8(get(&managed, &request.destination_id, "options.txt"))
            .unwrap()
            .contains("mouseSensitivity:0.4")
    );
    let history = fs::read_to_string(managed.launcher_dir().join("sync-observed.json")).unwrap();
    assert!(!history.contains("0.75"));
    assert!(!history.contains("key.keyboard.j"));
}

#[test]
fn automatic_initial_differences_are_preserved_and_later_single_edits_propagate() {
    let (managed, request) = fixture();
    automatic_policy(&managed, &request, vec![Category::Settings]);
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:0.7\n",
    );
    put(
        &managed,
        &request.destination_id,
        "options.txt",
        b"mouseSensitivity:0.2\n",
    );
    let mut engine = automatic::Engine::default();
    assert!(engine.cycle(&managed).unwrap().conflicts_kept > 0);
    assert_eq!(
        get(&managed, &request.destination_id, "options.txt"),
        b"mouseSensitivity:0.2\n"
    );
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:0.8\n",
    );
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 1);
    assert_eq!(
        get(&managed, &request.destination_id, "options.txt"),
        b"mouseSensitivity:0.8\n"
    );
}

#[test]
fn automatic_three_instance_pool_unions_missing_files_without_touching_unselected() {
    let (managed, request) = fixture();
    let mut registry = InstanceRegistry::load(&managed.instance_registry_file()).unwrap();
    let third = crate::instances::generate_instance_id();
    let extra = crate::instances::generate_instance_id();
    for id in [&third, &extra] {
        let mut configuration = InstanceConfiguration::for_minecraft_version("1.21.11");
        configuration.set_aurora_enabled(false);
        registry.instances_mut().push(
            InstanceRecord::from_installed(
                id.clone(),
                "Pool fixture",
                InstanceState::Ready,
                InstalledConfiguration {
                    minecraft_version: "1.21.11".into(),
                    platform: PlatformPin::Vanilla {},
                    aurora: None,
                },
                configuration,
            )
            .unwrap(),
        );
        fs::create_dir_all(managed.instance_paths(id).root()).unwrap();
    }
    registry.save(&managed.instance_registry_file()).unwrap();
    let mut selected = automatic_policy(&managed, &request, vec![Category::Screenshots]);
    selected.instances.push(third.clone());
    save_policy(&managed, selected).unwrap();
    for (id, name) in [
        (&request.source_id, "a"),
        (&request.destination_id, "b"),
        (&third, "c"),
    ] {
        put(
            &managed,
            id,
            &format!("screenshots/{name}.png"),
            name.as_bytes(),
        );
    }
    let mut engine = automatic::Engine::default();
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 6);
    for id in [&request.source_id, &request.destination_id, &third] {
        for name in ["a", "b", "c"] {
            assert_eq!(
                get(&managed, id, &format!("screenshots/{name}.png")),
                name.as_bytes()
            );
        }
    }
    assert!(
        !managed
            .instance_paths(&extra)
            .root()
            .join("screenshots")
            .exists()
    );
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 0);
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 0);
    put(&managed, &third, "screenshots/d.png", b"new");
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 2);
    let mut selected = policy(&managed).unwrap();
    selected.categories = vec![Category::Settings];
    save_policy(&managed, selected).unwrap();
    for id in [&request.source_id, &request.destination_id, &third] {
        put(&managed, id, "options.txt", b"mouseSensitivity:0.5\n");
    }
    engine.cycle(&managed).unwrap();
    let running = crate::launch::process::force_running(third.as_str());
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:0.8\n",
    );
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 1);
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 0);
    drop(running);
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 1);
    assert_eq!(
        get(&managed, &third, "options.txt"),
        b"mouseSensitivity:0.8\n"
    );
}

#[test]
fn automatic_skips_running_instances_and_rejects_stale_participation_and_history() {
    let (managed, request) = fixture();
    let selected = automatic_policy(&managed, &request, vec![Category::Screenshots]);
    put(
        &managed,
        &request.source_id,
        "screenshots/new.png",
        b"source",
    );
    let mut engine = automatic::Engine::default();
    let running = crate::launch::process::force_running(request.destination_id.as_str());
    assert!(engine.cycle(&managed).unwrap().skipped_instances > 0);
    assert!(
        !managed
            .instance_paths(&request.destination_id)
            .root()
            .join("screenshots/new.png")
            .exists()
    );
    drop(running);
    let mut copy = request.clone();
    copy.category = Category::Screenshots;
    let review = preview(&managed, &copy).unwrap();
    let mut changed = selected.clone();
    changed.instances.pop();
    save_policy(&managed, changed).unwrap();
    assert!(automatic::apply_selected(&managed, &copy, &review.fingerprint, &selected).is_err());
    save_policy(&managed, selected).unwrap();
    let path = managed.launcher_dir().join("sync-observed.json");
    fs::write(&path, b"{\"schemaVersion\":99,\"instances\":{}}").unwrap();
    let before = fs::read(&path).unwrap();
    assert!(engine.cycle(&managed).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(
        !managed
            .instance_paths(&request.destination_id)
            .root()
            .join("screenshots/new.png")
            .exists()
    );
}

#[test]
fn known_manual_preferences_never_silently_enable_automatic_participation() {
    let (managed, _) = fixture();
    fs::write(
        policy_file(&managed),
        br#"{"schemaVersion":1,"enabled":true}"#,
    )
    .unwrap();
    let loaded = policy(&managed).unwrap();
    assert!(!loaded.enabled);
    assert!(loaded.instances.is_empty());
    assert_eq!(
        fs::read(policy_file(&managed)).unwrap(),
        br#"{"schemaVersion":1,"enabled":true}"#
    );
}

#[tokio::test(start_paused = true)]
async fn automatic_worker_waits_one_minute_and_releases_its_timer_when_disabled() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (managed, request) = fixture();
    let selected = automatic_policy(&managed, &request, vec![Category::Screenshots]);
    put(
        &managed,
        &request.source_id,
        "screenshots/automatic.png",
        b"minute copy",
    );
    let cycles = Arc::new(AtomicUsize::new(0));
    let observer = Arc::clone(&cycles);
    let host = crate::host::Host::native(
        Ok(managed.data_root().to_path_buf()),
        "test",
        tokio::runtime::Handle::current(),
        move |_, payload| {
            if payload["lastRunUnixSeconds"].is_u64() && payload["running"] == false {
                observer.fetch_add(1, Ordering::SeqCst);
            }
        },
    );
    automatic::reconcile(host.clone(), managed.clone()).unwrap();
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(59)).await;
    tokio::task::yield_now().await;
    assert_eq!(cycles.load(Ordering::SeqCst), 0);
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    let deadline = std::time::Instant::now();
    while cycles.load(Ordering::SeqCst) == 0 {
        assert!(deadline.elapsed() < std::time::Duration::from_secs(3));
        tokio::task::yield_now().await;
    }
    assert_eq!(
        get(
            &managed,
            &request.destination_id,
            "screenshots/automatic.png"
        ),
        b"minute copy"
    );
    tokio::time::advance(std::time::Duration::from_secs(59)).await;
    tokio::task::yield_now().await;
    assert_eq!(cycles.load(Ordering::SeqCst), 1);
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    while cycles.load(Ordering::SeqCst) < 2 {
        assert!(deadline.elapsed() < std::time::Duration::from_secs(3));
        tokio::task::yield_now().await;
    }
    put(
        &managed,
        &request.source_id,
        "screenshots/disabled.png",
        b"must stay local",
    );
    let mut disabled = selected;
    disabled.enabled = false;
    save_policy(&managed, disabled).unwrap();
    automatic::reconcile(host, managed.clone()).unwrap();
    while automatic::worker_active() {
        assert!(deadline.elapsed() < std::time::Duration::from_secs(3));
        tokio::task::yield_now().await;
    }
    assert!(!automatic::status().enabled);
    tokio::time::advance(std::time::Duration::from_secs(120)).await;
    tokio::task::yield_now().await;
    assert!(
        !managed
            .instance_paths(&request.destination_id)
            .root()
            .join("screenshots/disabled.png")
            .exists()
    );
}

#[test]
fn automatic_combines_independent_setting_edits_from_both_instances() {
    let (managed, request) = fixture();
    automatic_policy(
        &managed,
        &request,
        vec![Category::Settings, Category::Controls],
    );
    let initial = b"mouseSensitivity:0.5\nkey_jump:key.keyboard.space\n";
    put(&managed, &request.source_id, "options.txt", initial);
    put(&managed, &request.destination_id, "options.txt", initial);
    let mut engine = automatic::Engine::default();
    engine.cycle(&managed).unwrap();
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:0.8\nkey_jump:key.keyboard.space\n",
    );
    put(
        &managed,
        &request.destination_id,
        "options.txt",
        b"mouseSensitivity:0.5\nkey_jump:key.keyboard.j\n",
    );
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 2);
    for id in [&request.source_id, &request.destination_id] {
        assert_eq!(
            get(&managed, id, "options.txt"),
            b"mouseSensitivity:0.8\nkey_jump:key.keyboard.j\n"
        );
    }
}

#[test]
fn automatic_preserves_pending_edits_until_a_running_peer_stops() {
    let (managed, request) = fixture();
    automatic_policy(&managed, &request, vec![Category::Settings]);
    for id in [&request.source_id, &request.destination_id] {
        put(&managed, id, "options.txt", b"mouseSensitivity:0.5\n");
    }
    let mut engine = automatic::Engine::default();
    engine.cycle(&managed).unwrap();
    let running = crate::launch::process::force_running(request.destination_id.as_str());
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:0.8\n",
    );
    engine.cycle(&managed).unwrap();
    engine.cycle(&managed).unwrap();
    drop(running);
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 1);
    assert_eq!(
        get(&managed, &request.destination_id, "options.txt"),
        b"mouseSensitivity:0.8\n"
    );
}

#[test]
fn file_edits_propagate_backup_and_concurrent_conflicts_persist_until_explicit_choice() {
    let (managed, request) = fixture();
    automatic_policy(&managed, &request, vec![Category::Screenshots]);
    for id in [&request.source_id, &request.destination_id] {
        put(&managed, id, "screenshots/a.png", b"base");
    }
    let mut engine = automatic::Engine::default();
    engine.cycle(&managed).unwrap();
    put(&managed, &request.source_id, "screenshots/a.png", b"edited");
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 1);
    assert_eq!(
        get(&managed, &request.destination_id, "screenshots/a.png"),
        b"edited"
    );
    put(&managed, &request.source_id, "screenshots/a.png", b"left");
    put(
        &managed,
        &request.destination_id,
        "screenshots/a.png",
        b"right",
    );
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 0);
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 0);
    let c = conflicts(&managed).unwrap().pop().unwrap();
    let receipt = resolve_conflict(&managed, &c.id, true).unwrap();
    assert_eq!(
        get(&managed, &c.destination_id, "screenshots/a.png"),
        get(&managed, &c.source_id, "screenshots/a.png")
    );
    let saved = managed
        .launcher_dir()
        .join("sync-backups")
        .join(receipt.backup_id.unwrap())
        .join("screenshots/a.png");
    assert!(
        fs::read(saved).unwrap() == b"left"
            || get(&managed, &c.source_id, "screenshots/a.png") == b"left"
    );
    assert!(conflicts(&managed).unwrap().is_empty());
    assert_eq!(engine.cycle(&managed).unwrap().files_changed, 0);
}
#[test]
fn file_edit_waits_across_restart_and_unavailable_third_peer_without_deleting() {
    let (managed, request) = fixture();
    let mut p = automatic_policy(&managed, &request, vec![Category::Screenshots]);
    let mut registry = InstanceRegistry::load(&managed.instance_registry_file()).unwrap();
    let third = crate::instances::generate_instance_id();
    let r = registry.instances()[0].clone();
    registry.instances_mut().push(
        InstanceRecord::from_installed(
            third.clone(),
            "third",
            InstanceState::Ready,
            r.installed().clone(),
            r.configuration().clone(),
        )
        .unwrap(),
    );
    registry.save(&managed.instance_registry_file()).unwrap();
    p.instances.push(third.clone());
    save_policy(&managed, p).unwrap();
    for id in [&request.source_id, &request.destination_id, &third] {
        put(&managed, id, "screenshots/a.png", b"base");
    }
    let mut engine = automatic::Engine::default();
    engine.cycle(&managed).unwrap();
    let running = crate::launch::process::force_running(third.as_str());
    put(&managed, &request.source_id, "screenshots/a.png", b"edit");
    engine.cycle(&managed).unwrap();
    let mut engine = automatic::Engine::default();
    engine.cycle(&managed).unwrap();
    drop(running);
    engine.cycle(&managed).unwrap();
    assert_eq!(get(&managed, &third, "screenshots/a.png"), b"edit");
    fs::remove_file(
        managed
            .instance_paths(&request.destination_id)
            .root()
            .join("screenshots/a.png"),
    )
    .unwrap();
    engine.cycle(&managed).unwrap();
    assert_eq!(
        get(&managed, &request.destination_id, "screenshots/a.png"),
        b"edit"
    );
}
#[test]
fn stale_conflict_choice_and_disabled_policy_never_replace() {
    let (managed, request) = fixture();
    automatic_policy(&managed, &request, vec![Category::Screenshots]);
    put(&managed, &request.source_id, "screenshots/a.png", b"a");
    put(&managed, &request.destination_id, "screenshots/a.png", b"b");
    automatic::Engine::default().cycle(&managed).unwrap();
    let c = conflicts(&managed).unwrap().pop().unwrap();
    put(&managed, &c.source_id, "screenshots/a.png", b"later");
    assert!(resolve_conflict(&managed, &c.id, true).is_err());
    assert_eq!(
        get(&managed, &c.destination_id, "screenshots/a.png"),
        if c.destination_id == request.source_id {
            b"a"
        } else {
            b"b"
        }
    );
    set_enabled(&managed, false).unwrap();
    assert!(resolve_conflict(&managed, &c.id, false).is_err());
}
#[test]
fn interrupted_replacement_and_stale_staging_preserve_original_and_recover() {
    let (managed, mut request) = fixture();
    request.category = Category::Screenshots;
    let selected = automatic_policy(&managed, &request, vec![Category::Screenshots]);
    put(&managed, &request.source_id, "screenshots/a.png", b"new");
    put(
        &managed,
        &request.destination_id,
        "screenshots/a.png",
        b"old",
    );
    let selection = BTreeMap::from([(
        "screenshots/a.png".into(),
        (digest(b"new"), Some(digest(b"old"))),
    )]);
    let p = locked(&managed, &request, || {
        plan_files(&managed, &request, Some(&selection)).map(|p| p.0)
    })
    .unwrap();
    assert!(
        apply_files(
            &managed,
            &request,
            &p.fingerprint,
            Some(&selection),
            &mut |at| if at == "copyStaged" {
                Err(storage(()))
            } else {
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(
        get(&managed, &request.destination_id, "screenshots/a.png"),
        b"old"
    );
    assert!(
        apply_files(
            &managed,
            &request,
            &p.fingerprint,
            Some(&selection),
            &mut |at| {
                if at == "copyStaged" {
                    put(
                        &managed,
                        &request.destination_id,
                        "screenshots/a.png",
                        b"racing",
                    );
                }
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(
        get(&managed, &request.destination_id, "screenshots/a.png"),
        b"racing"
    );
    put(
        &managed,
        &request.destination_id,
        "screenshots/a.png",
        b"old",
    );
    apply_files(
        &managed,
        &request,
        &p.fingerprint,
        Some(&selection),
        &mut |_| {
            if policy(&managed)? == selected {
                Ok(())
            } else {
                Err(stale())
            }
        },
    )
    .unwrap();
    assert_eq!(
        get(&managed, &request.destination_id, "screenshots/a.png"),
        b"new"
    );
}
#[test]
fn overrides_inherit_persist_invalidate_and_pause_without_losing_history() {
    let (managed, request) = fixture();
    let mut p = automatic_policy(&managed, &request, vec![Category::Screenshots]);
    assert_eq!(p.effective(&request.source_id).categories, p.categories);
    p.overrides.insert(
        request.destination_id.to_string(),
        InstanceOverride {
            disabled: false,
            categories: BTreeMap::from([(Category::Screenshots, false), (Category::Servers, true)]),
            exclusions: vec!["resourcepacks/private".into()],
        },
    );
    save_policy(&managed, p.clone()).unwrap();
    assert_eq!(policy(&managed).unwrap(), p);
    assert!(
        !p.effective(&request.destination_id)
            .categories
            .contains(&Category::Screenshots)
    );
    assert!(
        p.effective(&request.destination_id)
            .categories
            .contains(&Category::Servers)
    );
    put(&managed, &request.source_id, "screenshots/a.png", b"base");
    let mut e = automatic::Engine::default();
    assert_eq!(e.cycle(&managed).unwrap().files_changed, 0);
    p.overrides.clear();
    save_policy(&managed, p.clone()).unwrap();
    assert_eq!(e.cycle(&managed).unwrap().files_changed, 1);
    p.overrides.insert(
        request.destination_id.to_string(),
        InstanceOverride {
            disabled: true,
            ..Default::default()
        },
    );
    save_policy(&managed, p.clone()).unwrap();
    put(
        &managed,
        &request.source_id,
        "screenshots/a.png",
        b"changed",
    );
    assert_eq!(e.cycle(&managed).unwrap().files_changed, 0);
    p.overrides.clear();
    save_policy(&managed, p.clone()).unwrap();
    e.cycle(&managed).unwrap();
    assert_eq!(
        get(&managed, &request.destination_id, "screenshots/a.png"),
        b"changed"
    );
    p.categories.clear();
    save_policy(&managed, p.clone()).unwrap();
    put(&managed, &request.source_id, "screenshots/a.png", b"other");
    assert_eq!(e.cycle(&managed).unwrap().files_changed, 0);
    p.instances.pop();
    p.overrides.insert(
        request.destination_id.to_string(),
        InstanceOverride::default(),
    );
    assert!(save_policy(&managed, p).is_err());
}
#[test]
fn server_lists_add_both_directions_defer_edits_and_refuse_truncation() {
    let (managed, mut request) = fixture();
    request.category = Category::Servers;
    automatic_policy(&managed, &request, vec![Category::Servers]);
    let a = servers::tests::fixture(&[("A", "a", 1)]);
    let b = servers::tests::fixture(&[("B", "b", 2)]);
    put(&managed, &request.source_id, "servers.dat", &a);
    put(&managed, &request.destination_id, "servers.dat", &b);
    let mut e = automatic::Engine::default();
    assert!(e.cycle(&managed).unwrap().files_changed >= 2);
    assert_eq!(
        servers::merge(
            &get(&managed, &request.source_id, "servers.dat"),
            Some(&get(&managed, &request.destination_id, "servers.dat"))
        )
        .unwrap()
        .conflicts,
        0
    );
    let edited = servers::tests::fixture(&[("A", "changed", 1)]);
    put(&managed, &request.source_id, "servers.dat", &edited);
    e.cycle(&managed).unwrap();
    let c = conflicts(&managed)
        .unwrap()
        .into_iter()
        .find(|c| c.category == Category::Servers)
        .unwrap();
    assert!(resolve_conflict(&managed, &c.id, true).is_err());
    assert_eq!(get(&managed, &request.source_id, "servers.dat"), edited);
    put(
        &managed,
        &request.source_id,
        "servers.dat",
        &a[..a.len() - 1],
    );
    assert!(preview(&managed, &request).is_err());
}

#[test]
fn option_conflict_choice_only_merges_selected_key_and_preserves_pack_order() {
    let (managed, request) = fixture();
    automatic_policy(&managed, &request, vec![Category::Settings]);
    for id in [&request.source_id, &request.destination_id] {
        put(
            &managed,
            id,
            "options.txt",
            b"mouseSensitivity:1\nresourcePacks:[local]\n",
        );
    }
    let mut e = automatic::Engine::default();
    e.cycle(&managed).unwrap();
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:2\nresourcePacks:[first]\n",
    );
    put(
        &managed,
        &request.destination_id,
        "options.txt",
        b"mouseSensitivity:3\nresourcePacks:[second]\n",
    );
    e.cycle(&managed).unwrap();
    e.cycle(&managed).unwrap();
    let c = conflicts(&managed).unwrap().pop().unwrap();
    assert_eq!(c.key.as_deref(), Some("mouseSensitivity"));
    let before = get(&managed, &c.destination_id, "options.txt");
    resolve_conflict(&managed, &c.id, true).unwrap();
    let (_, before) = options(&before).unwrap();
    let (_, after) = options(&get(&managed, &c.destination_id, "options.txt")).unwrap();
    assert_eq!(before["resourcePacks"], after["resourcePacks"]);
    e.cycle(&managed).unwrap();
    assert!(conflicts(&managed).unwrap().is_empty());
}
#[test]
fn server_sync_excludes_running_games_and_interrupted_merge_keeps_backup() {
    let (managed, mut request) = fixture();
    request.category = Category::Servers;
    automatic_policy(&managed, &request, vec![Category::Servers]);
    let a = servers::tests::fixture(&[("A", "a", 1)]);
    let b = servers::tests::fixture(&[]);
    put(&managed, &request.source_id, "servers.dat", &a);
    put(&managed, &request.destination_id, "servers.dat", &b);
    let running = crate::launch::process::force_running(request.destination_id.as_str());
    assert!(preview(&managed, &request).is_err());
    drop(running);
    let p = preview(&managed, &request).unwrap();
    assert!(
        apply_with_checkpoint(&managed, &request, &p.fingerprint, |at| {
            if at == "backupSaved" {
                Err(storage(()))
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    assert_eq!(get(&managed, &request.destination_id, "servers.dat"), b);
    apply(&managed, &request, &p.fingerprint).unwrap();
    assert_eq!(get(&managed, &request.destination_id, "servers.dat"), a);
}

#[test]
fn server_list_missing_destination_uses_no_clobber_commit() {
    let (managed, mut request) = fixture();
    request.category = Category::Servers;
    automatic_policy(&managed, &request, vec![Category::Servers]);
    let a = servers::tests::fixture(&[("A", "a", 1)]);
    put(&managed, &request.source_id, "servers.dat", &a);
    let p = preview(&managed, &request).unwrap();
    assert!(
        apply_with_checkpoint(&managed, &request, &p.fingerprint, |at| {
            if at == "copyStaged" {
                put(
                    &managed,
                    &request.destination_id,
                    "servers.dat",
                    &servers::tests::fixture(&[("Local", "local", 2)]),
                );
            }
            Ok(())
        })
        .is_err()
    );
    assert_ne!(get(&managed, &request.destination_id, "servers.dat"), a);
    fs::remove_file(
        managed
            .instance_paths(&request.destination_id)
            .root()
            .join("servers.dat"),
    )
    .unwrap();
    let mut e = automatic::Engine::default();
    assert_eq!(e.cycle(&managed).unwrap().files_changed, 1);
    assert_eq!(get(&managed, &request.destination_id, "servers.dat"), a);
}

#[test]
fn per_instance_paths_global_changes_removal_and_damaged_history_are_safe() {
    let (managed, request) = fixture();
    let mut p = automatic_policy(&managed, &request, vec![Category::ResourcePacks]);
    p.overrides.insert(
        request.destination_id.to_string(),
        InstanceOverride {
            exclusions: vec!["resourcepacks/private".into()],
            ..Default::default()
        },
    );
    save_policy(&managed, p.clone()).unwrap();
    put(
        &managed,
        &request.source_id,
        "resourcepacks/private/a.zip",
        b"private",
    );
    put(
        &managed,
        &request.source_id,
        "resourcepacks/public.zip",
        b"public",
    );
    let mut e = automatic::Engine::default();
    assert_eq!(e.cycle(&managed).unwrap().files_changed, 1);
    assert!(
        !managed
            .instance_paths(&request.destination_id)
            .root()
            .join("resourcepacks/private/a.zip")
            .exists()
    );
    p.exclusions.push("resourcepacks/public.zip".into());
    save_policy(&managed, p.clone()).unwrap();
    put(
        &managed,
        &request.source_id,
        "resourcepacks/public.zip",
        b"new public",
    );
    assert_eq!(e.cycle(&managed).unwrap().files_changed, 0);
    let history = managed.launcher_dir().join("sync-files.json");
    let bad = b"{\"schemaVersion\":99,\"tracks\":{},\"conflicts\":[]}";
    fs::write(&history, bad).unwrap();
    assert!(e.cycle(&managed).is_err());
    assert_eq!(fs::read(&history).unwrap(), bad);
    fs::write(
        &history,
        b"{\"schemaVersion\":1,\"tracks\":{},\"conflicts\":[]}",
    )
    .unwrap();
    let mut registry = InstanceRegistry::load(&managed.instance_registry_file()).unwrap();
    registry
        .instances_mut()
        .retain(|r| r.id() != &request.destination_id);
    registry.save(&managed.instance_registry_file()).unwrap();
    assert!(e.cycle(&managed).unwrap().skipped_instances > 0);
    assert_eq!(
        get(
            &managed,
            &request.destination_id,
            "resourcepacks/public.zip"
        ),
        b"public"
    );
}

#[test]
fn temporarily_disabled_settings_category_retains_pending_baseline() {
    let (managed, request) = fixture();
    let mut p = automatic_policy(
        &managed,
        &request,
        vec![Category::Settings, Category::Controls],
    );
    for id in [&request.source_id, &request.destination_id] {
        put(
            &managed,
            id,
            "options.txt",
            b"mouseSensitivity:1\nkey_jump:key.keyboard.space\n",
        );
    }
    let mut e = automatic::Engine::default();
    e.cycle(&managed).unwrap();
    p.overrides.insert(
        request.destination_id.to_string(),
        InstanceOverride {
            categories: BTreeMap::from([(Category::Settings, false)]),
            ..Default::default()
        },
    );
    save_policy(&managed, p.clone()).unwrap();
    put(
        &managed,
        &request.source_id,
        "options.txt",
        b"mouseSensitivity:2\nkey_jump:key.keyboard.space\n",
    );
    e.cycle(&managed).unwrap();
    e.cycle(&managed).unwrap();
    assert!(
        String::from_utf8(get(&managed, &request.destination_id, "options.txt"))
            .unwrap()
            .contains("mouseSensitivity:1")
    );
    p.overrides.clear();
    save_policy(&managed, p).unwrap();
    e.cycle(&managed).unwrap();
    assert!(
        String::from_utf8(get(&managed, &request.destination_id, "options.txt"))
            .unwrap()
            .contains("mouseSensitivity:2")
    );
}

#[test]
fn retained_option_baselines_cannot_overflow_the_persisted_history_bound() {
    let (managed, request) = fixture();
    automatic_policy(&managed, &request, vec![Category::Controls]);
    let old = (0..512)
        .map(|i| format!("key_old_{i}:key.keyboard.a\n"))
        .collect::<String>();
    for id in [&request.source_id, &request.destination_id] {
        put(&managed, id, "options.txt", old.as_bytes());
    }
    let mut e = automatic::Engine::default();
    e.cycle(&managed).unwrap();
    let path = managed.launcher_dir().join("sync-observed.json");
    let before = fs::read(&path).unwrap();
    let new = (0..512)
        .map(|i| format!("key_new_{i}:key.keyboard.b\n"))
        .collect::<String>();
    for id in [&request.source_id, &request.destination_id] {
        put(&managed, id, "options.txt", new.as_bytes());
    }
    assert!(e.cycle(&managed).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    for id in [&request.source_id, &request.destination_id] {
        assert_eq!(get(&managed, id, "options.txt"), new.as_bytes());
    }
}
