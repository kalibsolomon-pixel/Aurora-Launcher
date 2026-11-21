//! Lock-safe, path-contained duplication of an existing managed instance.
//!
//! Creates a fresh `InstanceId` with the source's desired configuration,
//! installed game pin/files (reusing local bytes without re-downloading), and
//! user-selected content folders. Reparse points and symlinks are refused so
//! duplication can never read or write outside managed instance roots.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::lifecycle::{lock_instance_parent, registry_lock};
use super::{
    InstanceId, InstanceRecord, InstanceRegistry, InstanceState, generate_instance_id,
    validate_display_name,
};
use crate::paths::ManagedPaths;
use crate::{instance_content, launch};

const MAX_COPY_FILES: usize = 50_000;
const MAX_COPY_BYTES: u64 = 16 * 1024 * 1024 * 1024; // 16 GiB safety cap

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DuplicateOptions {
    pub copy_mods: bool,
    pub copy_config: bool,
    pub copy_resource_packs: bool,
    pub copy_shader_packs: bool,
    pub copy_saves: bool,
    pub copy_servers: bool,
    pub copy_options: bool,
    pub copy_screenshots: bool,
}

impl Default for DuplicateOptions {
    fn default() -> Self {
        Self {
            copy_mods: true,
            copy_config: true,
            copy_resource_packs: true,
            copy_shader_packs: true,
            copy_saves: false,
            copy_servers: true,
            copy_options: true,
            copy_screenshots: false,
        }
    }
}

#[derive(Debug)]
pub struct DuplicateError(pub &'static str, pub String);

impl std::fmt::Display for DuplicateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

impl std::error::Error for DuplicateError {}

fn failed(error: impl std::fmt::Display) -> DuplicateError {
    DuplicateError(
        "instance_duplicate_failed",
        format!("Could not duplicate the instance: {error}"),
    )
}

fn check_no_reparse(path: &Path) -> Result<fs::Metadata, DuplicateError> {
    let meta = fs::symlink_metadata(path).map_err(failed)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(DuplicateError(
                "instance_duplicate_unsafe",
                "Remove links or junctions from this instance before duplicating it.".into(),
            ));
        }
    }
    if meta.file_type().is_symlink() {
        return Err(DuplicateError(
            "instance_duplicate_unsafe",
            "Symlinked instance files cannot be duplicated by the launcher.".into(),
        ));
    }
    Ok(meta)
}

struct CopyBudget {
    files: usize,
    bytes: u64,
}

impl CopyBudget {
    fn new() -> Self {
        Self { files: 0, bytes: 0 }
    }

    fn copy_file(&mut self, src: &Path, dst: &Path) -> Result<(), DuplicateError> {
        let meta = check_no_reparse(src)?;
        if !meta.is_file() {
            return Ok(());
        }
        self.files += 1;
        self.bytes = self.bytes.saturating_add(meta.len());
        if self.files > MAX_COPY_FILES || self.bytes > MAX_COPY_BYTES {
            return Err(DuplicateError(
                "instance_duplicate_limit",
                "This instance exceeds the safe duplication limit (50,000 files or 16 GiB). Exclude saves or screenshots and retry.".into(),
            ));
        }
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(failed)?;
        }
        fs::copy(src, dst).map_err(failed)?;
        Ok(())
    }

    fn copy_dir_recursive(&mut self, src: &Path, dst: &Path) -> Result<(), DuplicateError> {
        if !src.exists() {
            return Ok(());
        }
        let meta = check_no_reparse(src)?;
        if !meta.is_dir() {
            return Ok(());
        }
        fs::create_dir_all(dst).map_err(failed)?;
        for entry in fs::read_dir(src).map_err(failed)? {
            let entry = entry.map_err(failed)?;
            let name = entry.file_name();
            let child_src = entry.path();
            let child_dst = dst.join(&name);
            let child_meta = check_no_reparse(&child_src)?;
            if child_meta.is_dir() {
                self.copy_dir_recursive(&child_src, &child_dst)?;
            } else if child_meta.is_file() {
                self.copy_file(&child_src, &child_dst)?;
            }
        }
        Ok(())
    }
}

/// Duplicates one registered instance under a fresh opaque `InstanceId`.
///
/// The source instance must not be actively installing or running. If copying
/// fails partway through, the newly allocated destination directory is removed
/// and the registry remains unchanged.
pub fn duplicate(
    managed: &ManagedPaths,
    source_id: &InstanceId,
    new_display_name: &str,
    options: &DuplicateOptions,
) -> Result<InstanceRecord, DuplicateError> {
    let trimmed_name = new_display_name.trim();
    validate_display_name(trimmed_name)
        .map_err(|e| DuplicateError("instance_invalid_name", e.to_string()))?;

    instance_content::with_instance_lock(source_id, || {
        Ok((|| {
            let _operation = lock_instance_parent(managed, source_id).map_err(|_| {
                DuplicateError(
                    "instance_duplicate_busy",
                    "An installation is still using this instance.".into(),
                )
            })?;
            let _process = launch::process::lock_stopped(source_id.as_str()).map_err(|_| {
                DuplicateError(
                    "instance_duplicate_busy",
                    "Quit Minecraft before duplicating this instance.".into(),
                )
            })?;

            let _registry_guard = registry_lock();
            let mut registry =
                InstanceRegistry::load(&managed.instance_registry_file()).map_err(failed)?;
            let source = registry.find(source_id).cloned().ok_or_else(|| {
                DuplicateError(
                    "instance_not_found",
                    "The source instance no longer exists.".into(),
                )
            })?;
            if source.state() != InstanceState::Ready {
                return Err(DuplicateError(
                    "instance_duplicate_not_ready",
                    "Only a ready instance can be duplicated.".into(),
                ));
            }

            let src_paths = managed.instance_paths(source_id);
            check_no_reparse(&managed.instances_dir())?;
            let data = fs::canonicalize(managed.data_root()).map_err(failed)?;
            let instances = fs::canonicalize(managed.instances_dir()).map_err(failed)?;
            if instances != data.join("instances") {
                return Err(DuplicateError(
                    "instance_duplicate_unsafe",
                    "The managed instances directory is redirected.".into(),
                ));
            }
            check_no_reparse(src_paths.root())?;
            if fs::canonicalize(src_paths.root()).map_err(failed)?
                != instances.join(source_id.as_str())
            {
                return Err(DuplicateError(
                    "instance_duplicate_unsafe",
                    "The source instance root is redirected.".into(),
                ));
            }

            let new_id = generate_instance_id();
            let dst_paths = managed.instance_paths(&new_id);
            fs::create_dir_all(dst_paths.root()).map_err(failed)?;

            let copy_result = (|| -> Result<(), DuplicateError> {
                let mut budget = CopyBudget::new();
                // Always copy the verified managed game tree and Aurora state
                // (never copy transient staging, locks, or install history).
                budget.copy_dir_recursive(src_paths.game(), dst_paths.game())?;
                for meta_file in [
                    "aurora-installed.json",
                    "provider-content.json",
                    "pack-installed.json",
                    ".offline-launch-plan.json",
                ] {
                    let src_file = src_paths.root().join(meta_file);
                    if src_file.exists() {
                        budget.copy_file(&src_file, &dst_paths.root().join(meta_file))?;
                    }
                }
                // Copy instance icon if present.
                for icon_ext in ["png", "webp", "jpg"] {
                    let src_icon = src_paths.root().join(format!("icon.{icon_ext}"));
                    if src_icon.exists() {
                        budget.copy_file(
                            &src_icon,
                            &dst_paths.root().join(format!("icon.{icon_ext}")),
                        )?;
                    }
                }

                // Always copy Aurora-managed mod JAR if Aurora is installed, even when copy_mods is false.
                if options.copy_mods {
                    budget.copy_dir_recursive(src_paths.mods(), dst_paths.mods())?;
                } else if let Some(aurora_pin) = source.installed().aurora.as_ref() {
                    let aurora_jar = format!("aurora-{}.jar", aurora_pin.version);
                    let src_jar = src_paths.mods().join(&aurora_jar);
                    if src_jar.exists() {
                        budget.copy_file(&src_jar, &dst_paths.mods().join(&aurora_jar))?;
                    }
                }

                if options.copy_config {
                    budget.copy_dir_recursive(src_paths.config(), dst_paths.config())?;
                }
                if options.copy_resource_packs {
                    budget
                        .copy_dir_recursive(src_paths.resourcepacks(), dst_paths.resourcepacks())?;
                }
                if options.copy_shader_packs {
                    budget.copy_dir_recursive(src_paths.shaderpacks(), dst_paths.shaderpacks())?;
                }
                if options.copy_saves {
                    budget.copy_dir_recursive(
                        &src_paths.root().join("saves"),
                        &dst_paths.root().join("saves"),
                    )?;
                }
                if options.copy_screenshots {
                    budget.copy_dir_recursive(
                        &src_paths.root().join("screenshots"),
                        &dst_paths.root().join("screenshots"),
                    )?;
                }
                if options.copy_servers {
                    let src_servers = src_paths.root().join("servers.dat");
                    if src_servers.exists() {
                        budget.copy_file(&src_servers, &dst_paths.root().join("servers.dat"))?;
                    }
                }
                if options.copy_options {
                    let src_options = src_paths.root().join("options.txt");
                    if src_options.exists() {
                        budget.copy_file(&src_options, &dst_paths.root().join("options.txt"))?;
                    }
                }
                Ok(())
            })();

            if let Err(err) = copy_result {
                let _ = fs::remove_dir_all(dst_paths.root());
                return Err(err);
            }

            let mut cloned_record = InstanceRecord::from_installed(
                new_id.clone(),
                trimmed_name,
                InstanceState::Ready,
                source.installed().clone(),
                source.configuration().clone(),
            )
            .map_err(|e| {
                let _ = fs::remove_dir_all(dst_paths.root());
                failed(e)
            })?;
            if let Some(pack) = source.pack().cloned() {
                cloned_record.set_pack(pack).map_err(|e| {
                    let _ = fs::remove_dir_all(dst_paths.root());
                    failed(e)
                })?;
            }

            registry.instances_mut().push(cloned_record.clone());
            if let Err(err) = registry.save(&managed.instance_registry_file()) {
                let _ = fs::remove_dir_all(dst_paths.root());
                return Err(failed(err));
            }
            Ok(cloned_record)
        })())
    })
    .map_err(|_| {
        DuplicateError(
            "instance_duplicate_busy",
            "Another operation is changing this instance.".into(),
        )
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::platform::{InstalledConfiguration, PlatformPin};
    use crate::instances::settings::{InstanceConfiguration, LoaderConfiguration};

    #[test]
    fn duplicate_copies_selected_content_and_isolates_new_id() {
        let root =
            std::env::temp_dir().join(format!("aurora-duplicate-test-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let src_id = generate_instance_id();
        let src = managed.instance_paths(&src_id);
        fs::create_dir_all(src.game()).unwrap();
        fs::write(src.game().join("installed-game.json"), b"{}").unwrap();
        fs::create_dir_all(src.mods()).unwrap();
        fs::write(src.mods().join("sodium.jar"), b"mod-bytes").unwrap();
        fs::create_dir_all(src.root().join("saves/World1")).unwrap();
        fs::write(src.root().join("saves/World1/level.dat"), b"world").unwrap();
        fs::write(src.root().join("options.txt"), b"fov:0.5\n").unwrap();

        let mut cfg = InstanceConfiguration::from_parts(
            "1.21.11",
            LoaderConfiguration::Vanilla {},
            4096,
            String::new(),
            None,
        );
        cfg.set_aurora_enabled(false);
        let record = InstanceRecord::from_installed(
            src_id.clone(),
            "Main Instance",
            InstanceState::Ready,
            InstalledConfiguration {
                minecraft_version: "1.21.11".into(),
                platform: PlatformPin::Vanilla {},
                aurora: None,
            },
            cfg,
        )
        .unwrap();
        let mut reg = InstanceRegistry::empty();
        reg.instances_mut().push(record);
        reg.save(&managed.instance_registry_file()).unwrap();

        let cloned = duplicate(
            &managed,
            &src_id,
            "Main Instance Copy",
            &DuplicateOptions {
                copy_saves: false,
                ..DuplicateOptions::default()
            },
        )
        .unwrap();

        assert_ne!(cloned.id(), &src_id);
        assert_eq!(cloned.display_name(), "Main Instance Copy");
        assert_eq!(cloned.configuration().memory_mib(), 4096);
        let dst = managed.instance_paths(cloned.id());
        assert!(dst.game().join("installed-game.json").is_file());
        assert_eq!(
            fs::read(dst.mods().join("sodium.jar")).unwrap(),
            b"mod-bytes"
        );
        assert_eq!(
            fs::read(dst.root().join("options.txt")).unwrap(),
            b"fov:0.5\n"
        );
        assert!(!dst.root().join("saves").exists());

        let reloaded = InstanceRegistry::load(&managed.instance_registry_file()).unwrap();
        assert_eq!(reloaded.instances().len(), 2);

        let _ = fs::remove_dir_all(&root);
    }
}
