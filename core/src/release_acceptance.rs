//! Disposable current-candidate data compatibility, without credentials or games.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            assert!(!entry.file_type().unwrap().is_symlink());
            if entry.file_type().unwrap().is_dir() {
                walk(root, &entry.path(), out);
            } else {
                out.insert(
                    entry.path().strip_prefix(root).unwrap().into(),
                    std::fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}

#[tokio::test]
async fn release_candidate_copy_backup_and_new_preferences_survive_restart() {
    use crate::instance_sync::{Category, InstanceOverride, Policy};
    use crate::instances::lifecycle::tests::SyntheticWorld;
    let world = SyntheticWorld::new("release-migration");
    let record = world.create("Disposable migration fixture").await.unwrap();
    let managed = world.managed();
    crate::instance_icons::set(managed, record.id(), Some("swords")).unwrap();
    for relative in [
        "saves/World/level.dat",
        "saves/World/user-sentinel",
        "mods/user.jar",
        "resourcepacks/user.zip",
        "shaderpacks/user.zip",
        "config/user.json",
        "servers.dat",
        "user-owned-sentinel",
        ".aurora/provider-sentinel.json",
    ] {
        let path = managed.instance_paths(record.id()).root().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("preserve exactly: {relative}")).unwrap();
    }
    let registry_path = managed.instance_registry_file();
    let mut registry = crate::instances::InstanceRegistry::load(&registry_path).unwrap();
    let mut configuration = registry.instances()[0].configuration().clone();
    configuration.set_java(Some(crate::instances::settings::JavaSelection::Custom {
        path: if cfg!(windows) {
            r"C:\DisposableJava\bin\javaw.exe"
        } else {
            "/disposable/bin/java"
        }
        .into(),
    }));
    registry.instances_mut()[0].set_configuration(configuration);
    registry.save(&registry_path).unwrap();
    let mut config = crate::config::load(&managed.config_file())
        .unwrap()
        .unwrap();
    let mut widgets = config.home_widgets().clone();
    widgets.widgets[0].grid = Some(crate::home_widgets::WidgetGrid {
        columns: 2,
        rows: 2,
        position: Some([0, 1]),
    });
    config.set_home_widgets(widgets);
    crate::config::save(&managed.config_file(), &config).unwrap();
    let mut policy = Policy {
        instances: vec![record.id().clone()],
        ..Default::default()
    };
    policy.overrides.insert(
        record.id().as_str().into(),
        InstanceOverride {
            disabled: true,
            categories: BTreeMap::from([(Category::Servers, false)]),
            exclusions: vec!["config/private.json".into()],
        },
    );
    crate::instance_sync::save_policy(managed, policy.clone()).unwrap();
    // Unknown future history is retained, never interpreted or downgraded on boot.
    let history = managed.launcher_dir().join("sync-files.json");
    std::fs::write(
        &history,
        br#"{"schemaVersion":99,"conflicts":["preserve future history"]}"#,
    )
    .unwrap();
    let before = snapshot(managed.data_root());
    let root = std::env::temp_dir().join(format!("aurora-release-copy-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    for name in ["backup", "candidate"] {
        for (relative, bytes) in &before {
            let path = root.join(name).join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }
    let candidate =
        crate::paths::ManagedPaths::from_app_local_data_dir(root.join("candidate")).unwrap();
    let state = super::load_launcher_state(&candidate).unwrap();
    assert_eq!(state.instances.len(), 1);
    assert_eq!(crate::instance_sync::policy(&candidate).unwrap(), policy);
    let config = crate::config::load(&candidate.config_file())
        .unwrap()
        .unwrap();
    crate::config::save(&candidate.config_file(), &config).unwrap();
    let registry =
        crate::instances::InstanceRegistry::load(&candidate.instance_registry_file()).unwrap();
    registry.save(&candidate.instance_registry_file()).unwrap();
    assert_eq!(
        snapshot(candidate.data_root()),
        before,
        "restart and unrelated writes preserve every existing byte"
    );
    assert_eq!(snapshot(&root.join("backup")), before, "backup integrity");
    assert_eq!(snapshot(managed.data_root()), before, "source identity");
    // Exact disposable root created above; no production path is visited.
    std::fs::remove_dir_all(root).unwrap();
}
