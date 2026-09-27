//! Deliberate deletion of one registered, isolated instance. Shared cache,
//! runtime, account and launcher preference data are never deletion targets.
use super::{InstanceId, InstanceRegistry, InstanceState};
use crate::{config, instance_content, launch, paths::ManagedPaths};
use std::path::Path;

#[derive(Debug)]
pub struct DeleteError(pub &'static str, pub String);
impl std::fmt::Display for DeleteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}
impl std::error::Error for DeleteError {}
fn failed(error: impl std::fmt::Display) -> DeleteError {
    DeleteError(
        "instance_delete_failed",
        format!("Deletion did not complete; the registry entry is retained for retry: {error}"),
    )
}
fn safe_metadata(path: &Path) -> Result<std::fs::Metadata, DeleteError> {
    let meta = std::fs::symlink_metadata(path).map_err(failed)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(DeleteError(
                "instance_delete_unsafe",
                "Remove links or junctions from this instance before deleting it.".into(),
            ));
        }
    }
    if meta.file_type().is_symlink() {
        return Err(DeleteError(
            "instance_delete_unsafe",
            "Linked instance data cannot be deleted by the launcher.".into(),
        ));
    }
    Ok(meta)
}
fn inspect_tree(path: &Path) -> Result<(), DeleteError> {
    if safe_metadata(path)?.is_dir() {
        for child in std::fs::read_dir(path).map_err(failed)? {
            inspect_tree(&child.map_err(failed)?.path())?;
        }
    }
    Ok(())
}
pub fn delete(
    managed: &ManagedPaths,
    id: &InstanceId,
    confirmed_name: &str,
) -> Result<(), DeleteError> {
    instance_content::with_instance_lock(id, || {
        Ok((|| {
            let _registry = super::lifecycle::registry_lock();
            let mut registry =
                InstanceRegistry::load(&managed.instance_registry_file()).map_err(failed)?;
            let record = registry.find(id).ok_or_else(|| {
                DeleteError(
                    "instance_not_found",
                    "This instance no longer exists.".into(),
                )
            })?;
            if confirmed_name != record.display_name() {
                return Err(DeleteError(
                    "instance_delete_confirmation",
                    "Type the current instance name to confirm deletion.".into(),
                ));
            }
            if record.state() != InstanceState::Ready {
                return Err(DeleteError(
                    "instance_delete_busy",
                    "Finish instance installation before deleting it.".into(),
                ));
            }
            let _process = launch::process::lock_stopped(id.as_str()).map_err(|_| {
                DeleteError(
                    "instance_delete_busy",
                    "Quit Minecraft before deleting this instance.".into(),
                )
            })?;
            let root = managed.instance_paths(id).root().to_owned();
            safe_metadata(&managed.instances_dir())?;
            let data = std::fs::canonicalize(managed.data_root()).map_err(failed)?;
            let instances = std::fs::canonicalize(managed.instances_dir()).map_err(failed)?;
            if instances != data.join("instances") {
                return Err(DeleteError(
                    "instance_delete_unsafe",
                    "The managed instances directory is redirected.".into(),
                ));
            }
            let exists = match std::fs::symlink_metadata(&root) {
                Ok(_) => {
                    if !safe_metadata(&root)?.is_dir()
                        || std::fs::canonicalize(&root).map_err(failed)?
                            != instances.join(id.as_str())
                    {
                        return Err(DeleteError(
                            "instance_delete_unsafe",
                            "The instance root is redirected or is not a directory.".into(),
                        ));
                    }
                    inspect_tree(&root)?;
                    true
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => return Err(failed(e)),
            };
            // Clear selection first with an atomic write. Every observable state
            // has a valid reference; never remove a registry row before the files.
            let mut selection = config::load(&managed.config_file())
                .map_err(failed)?
                .unwrap_or_default();
            let old_selection = selection.clone();
            if selection.selected_instance_id() == Some(id) {
                selection.set_selected_instance_id(None);
                config::save(&managed.config_file(), &selection).map_err(failed)?;
            }
            if exists && let Err(error) = std::fs::remove_dir_all(&root) {
                let _ = config::save(&managed.config_file(), &old_selection);
                return Err(failed(error));
            }
            // A filesystem error leaves the row and remaining bytes available for
            // retry. A registry-write failure after removal likewise reports an
            // error and leaves the row; retry safely handles the absent exact root.
            registry.instances_mut().retain(|record| record.id() != id);
            registry
                .save(&managed.instance_registry_file())
                .map_err(failed)?;
            Ok(())
        })())
    })
    .map_err(|_| {
        DeleteError(
            "instance_delete_busy",
            "Another operation is changing this instance.".into(),
        )
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::{
        InstanceRecord,
        platform::{InstalledConfiguration, PlatformPin},
        settings::{InstanceConfiguration, LoaderConfiguration},
    };
    struct World {
        managed: ManagedPaths,
        id: InstanceId,
    }
    impl World {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("aurora-delete-test-{}", uuid::Uuid::new_v4()));
            let managed = ManagedPaths::from_app_local_data_dir(root).unwrap();
            let id = super::super::generate_instance_id();
            std::fs::create_dir_all(managed.instance_paths(&id).root().join("saves")).unwrap();
            std::fs::write(
                managed.instance_paths(&id).root().join("saves/world.txt"),
                b"world",
            )
            .unwrap();
            std::fs::create_dir_all(managed.cache_dir()).unwrap();
            std::fs::write(managed.cache_dir().join("shared"), b"shared").unwrap();
            let mut configuration = InstanceConfiguration::from_parts(
                "1.21.11",
                LoaderConfiguration::Vanilla {},
                2048,
                String::new(),
                None,
            );
            configuration.set_aurora_enabled(false);
            let record = InstanceRecord::from_installed(
                id.clone(),
                "Disposable",
                InstanceState::Ready,
                InstalledConfiguration {
                    minecraft_version: "1.21.11".into(),
                    platform: PlatformPin::Vanilla {},
                    aurora: None,
                },
                configuration,
            )
            .unwrap();
            let mut registry = InstanceRegistry::empty();
            registry.instances_mut().push(record);
            registry.save(&managed.instance_registry_file()).unwrap();
            let mut selected = config::LauncherConfig::default();
            selected.set_selected_instance_id(Some(id.clone()));
            config::save(&managed.config_file(), &selected).unwrap();
            Self { managed, id }
        }
    }
    impl Drop for World {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.managed.data_root());
        }
    }
    #[test]
    fn deletion_removes_only_registered_root_and_clears_selection() {
        let world = World::new();
        let other = world.managed.instances_dir().join("unrelated");
        std::fs::create_dir(&other).unwrap();
        std::fs::write(other.join("keep"), b"keep").unwrap();
        delete(&world.managed, &world.id, "Disposable").unwrap();
        assert!(!world.managed.instance_paths(&world.id).root().exists());
        assert!(
            InstanceRegistry::load(&world.managed.instance_registry_file())
                .unwrap()
                .find(&world.id)
                .is_none()
        );
        assert!(
            config::load(&world.managed.config_file())
                .unwrap()
                .unwrap()
                .selected_instance_id()
                .is_none()
        );
        assert_eq!(std::fs::read(other.join("keep")).unwrap(), b"keep");
        assert_eq!(
            std::fs::read(world.managed.cache_dir().join("shared")).unwrap(),
            b"shared"
        );
    }
    #[test]
    fn confirmation_busy_and_malformed_registry_leave_files_untouched() {
        let world = World::new();
        let file = world
            .managed
            .instance_paths(&world.id)
            .root()
            .join("saves/world.txt");
        assert_eq!(
            delete(&world.managed, &world.id, "wrong").unwrap_err().0,
            "instance_delete_confirmation"
        );
        instance_content::with_instance_lock(&world.id, || {
            assert_eq!(
                delete(&world.managed, &world.id, "Disposable")
                    .unwrap_err()
                    .0,
                "instance_delete_busy"
            );
            Ok(())
        })
        .unwrap();
        std::fs::write(world.managed.instance_registry_file(), b"malformed").unwrap();
        assert!(delete(&world.managed, &world.id, "Disposable").is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"world");
        assert_eq!(
            std::fs::read(world.managed.instance_registry_file()).unwrap(),
            b"malformed"
        );
    }
    #[test]
    fn starting_and_running_processes_refuse_deletion() {
        let world = World::new();
        for status in [
            launch::state::LaunchProcessStatus::Starting,
            launch::state::LaunchProcessStatus::Running,
        ] {
            launch::process::with_test_state(world.id.as_str(), status, || {
                assert_eq!(
                    delete(&world.managed, &world.id, "Disposable")
                        .unwrap_err()
                        .0,
                    "instance_delete_busy"
                );
                assert!(
                    world
                        .managed
                        .instance_paths(&world.id)
                        .root()
                        .join("saves/world.txt")
                        .is_file()
                );
            });
        }
    }
    #[test]
    fn a_non_directory_root_is_never_removed() {
        let world = World::new();
        let root = world.managed.instance_paths(&world.id).root().to_owned();
        // This exact disposable root was allocated by this test.
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::write(&root, b"unexpected user bytes").unwrap();
        assert_eq!(
            delete(&world.managed, &world.id, "Disposable")
                .unwrap_err()
                .0,
            "instance_delete_unsafe"
        );
        assert_eq!(std::fs::read(root).unwrap(), b"unexpected user bytes");
    }
    #[test]
    fn failed_registry_commit_retains_entry_and_retry_handles_absent_root() {
        let world = World::new();
        let temporary = world.managed.launcher_dir().join("instances.json.tmp");
        std::fs::create_dir(&temporary).unwrap();
        assert_eq!(
            delete(&world.managed, &world.id, "Disposable")
                .unwrap_err()
                .0,
            "instance_delete_failed"
        );
        assert!(
            InstanceRegistry::load(&world.managed.instance_registry_file())
                .unwrap()
                .find(&world.id)
                .is_some()
        );
        assert!(
            config::load(&world.managed.config_file())
                .unwrap()
                .unwrap()
                .selected_instance_id()
                .is_none()
        );
        std::fs::remove_dir(temporary).unwrap();
        delete(&world.managed, &world.id, "Disposable").unwrap();
    }
}
