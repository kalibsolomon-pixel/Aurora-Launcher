//! Read-only compatibility plus rejected-write probe on a marked synthetic copy.
use aurora_launcher_lib::{
    config, instance_sync, instances::InstanceRegistry, paths::ManagedPaths,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            if kind.is_dir() {
                visit(root, &entry.path(), out);
            } else {
                out.insert(
                    entry.path().strip_prefix(root).unwrap().into(),
                    std::fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 2);
    let root = PathBuf::from(&args[0]);
    assert!(
        root.parent() == Some(std::env::temp_dir().as_path())
            && root
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("aurora-native9-compatibility-")
            && root.join("NATIVE-RELEASE-DISPOSABLE.txt").is_file(),
        "marked synthetic compatibility root required"
    );
    assert!(
        std::env::var_os("AURORA_DIAGNOSTIC_DATA_ROOT").is_none(),
        "remove inherited diagnostic overrides"
    );
    let managed = ManagedPaths::from_app_local_data_dir(root.join("data")).unwrap();
    let before = snapshot(managed.data_root());
    let registry = InstanceRegistry::load(&managed.instance_registry_file()).is_ok();
    let configuration = config::load(&managed.config_file()).is_ok();
    let syncing = instance_sync::policy(&managed).is_ok();
    let rejected_write = if args[1] == "old" {
        instance_sync::save_policy(&managed, instance_sync::Policy::default()).is_err()
    } else {
        false
    };
    let after = snapshot(managed.data_root());
    let unchanged = before
        .iter()
        .all(|(path, bytes)| after.get(path) == Some(bytes));
    assert!(
        after.iter().all(|(path, bytes)| before.contains_key(path)
            || (path == Path::new("launcher/.sync-settings.lock") && bytes.is_empty())),
        "only the known coordination lock may be added by a refused write"
    );
    println!(
        "{}",
        serde_json::json!({"registryReadable":registry,"configurationReadable":configuration,"syncPolicyReadable":syncing,"rejectedSyncWrite":rejected_write,"allBytesPreserved":unchanged})
    );
    assert!(registry && unchanged);
    if args[1] == "old" {
        assert!(!configuration && !syncing && rejected_write);
    } else {
        assert!(configuration && syncing);
    }
}
