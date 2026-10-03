//! Optional Aurora changes are approved, fingerprinted content transactions.
//! Provider provenance is never manufactured or transferred. Former launcher
//! artifacts needed by other content retain their own non-required ownership.
use super::lifecycle::{InstanceEndpoints, InstanceStatus, validate_instance};
use super::platform::{AuroraPin, InstalledConfiguration};
use super::{InstanceId, InstanceRecord, InstanceRegistry, InstanceState};
use crate::cache::ArtifactCache;
use crate::downloads::ArtifactSource;
use crate::integrity::{ArtifactDigest, verify_file};
use crate::paths::ManagedPaths;
use crate::{aurora, instance_content, instance_mods};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const RETAINED_FILE: &str = "aurora-retained.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransitionFile {
    pub relative_path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetainedState {
    schema_version: u32,
    files: Vec<TransitionFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionPreview {
    pub instance_id: String,
    pub current: InstalledConfiguration,
    pub target: InstalledConfiguration,
    pub requirements_added: Vec<String>,
    pub requirements_removed: Vec<String>,
    pub install: Vec<TransitionFile>,
    pub remove: Vec<TransitionFile>,
    pub retain: Vec<TransitionFile>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
    pub fingerprint: String,
}

#[derive(Debug)]
pub struct TransitionError(pub &'static str, pub String);
impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.1)
    }
}
impl std::error::Error for TransitionError {}
fn invalid(error: impl std::fmt::Display) -> TransitionError {
    TransitionError("aurora_transition_invalid", error.to_string())
}
fn regular(path: &Path) -> Result<(), TransitionError> {
    let meta = std::fs::symlink_metadata(path).map_err(invalid)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(invalid("reparse points are not allowed"));
        }
    }
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("managed state or artifact is not a regular file"));
    }
    Ok(())
}
fn optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, TransitionError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => {
            regular(path)?;
            std::fs::read(path).map(Some).map_err(invalid)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(invalid(e)),
    }
}
fn file_path(
    managed: &ManagedPaths,
    id: &InstanceId,
    file: &TransitionFile,
) -> Result<PathBuf, TransitionError> {
    let name = file
        .relative_path
        .strip_prefix("mods/")
        .ok_or_else(|| invalid("retained path is outside mods"))?;
    instance_content::validate_file_name(name).map_err(invalid)?;
    ArtifactDigest::parse(&file.sha256).map_err(invalid)?;
    if file.size_bytes == 0 || !name.ends_with(".jar") {
        return Err(invalid("invalid retained artifact"));
    }
    let directory =
        instance_content::validate_directory(managed, id, instance_content::ContentType::Mod)
            .map_err(invalid)?;
    Ok(directory.join(name))
}
pub(crate) fn retained_files(
    managed: &ManagedPaths,
    id: &InstanceId,
) -> Result<Vec<TransitionFile>, TransitionError> {
    let Some(bytes) = optional_bytes(&managed.instance_paths(id).root().join(RETAINED_FILE))?
    else {
        return Ok(vec![]);
    };
    let state: RetainedState = serde_json::from_slice(&bytes).map_err(invalid)?;
    if state.schema_version != 1 {
        return Err(invalid("unsupported retained ownership schema"));
    }
    let mut names = std::collections::HashSet::new();
    for file in &state.files {
        file_path(managed, id, file)?;
        if !names.insert(file.relative_path.to_lowercase()) {
            return Err(invalid("duplicate retained ownership"));
        }
    }
    Ok(state.files)
}
fn verify(
    managed: &ManagedPaths,
    id: &InstanceId,
    file: &TransitionFile,
) -> Result<(), TransitionError> {
    let path = file_path(managed, id, file)?;
    regular(&path)?;
    verify_file(
        &path,
        &ArtifactDigest::parse(&file.sha256).map_err(invalid)?,
        Some(file.size_bytes),
    )
    .map_err(invalid)?;
    Ok(())
}
fn from_artifact(artifact: &aurora::AuroraInstalledArtifact, reason: &str) -> TransitionFile {
    TransitionFile {
        relative_path: artifact.relative_path().into(),
        sha256: artifact.sha256().into(),
        size_bytes: artifact.size_bytes(),
        reason: reason.into(),
    }
}

pub fn preview(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    id: &InstanceId,
    enabled: bool,
) -> Result<TransitionPreview, TransitionError> {
    let registry = InstanceRegistry::load(&managed.instance_registry_file()).map_err(invalid)?;
    let record = registry
        .find(id)
        .ok_or_else(|| invalid("instance not found"))?;
    if record.state() != InstanceState::Ready
        || !record.configuration().matches_installed(record.installed())
    {
        return Err(invalid(
            "finish installation and resolve stale configuration first",
        ));
    }
    let validation = validate_instance(managed, &registry, id).map_err(invalid)?;
    if validation.status != InstanceStatus::Ready {
        return Err(invalid(
            "the current working configuration must validate before a transition",
        ));
    }
    let mut retained = Vec::new();
    for file in retained_files(managed, id)? {
        let path = file_path(managed, id, &file)?;
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if std::fs::symlink_metadata(path.with_extension("jar.disabled")).is_ok() {
                    return Err(invalid(
                        "Re-enable disabled retained bootstrap content in Mods before changing the original Aurora configuration.",
                    ));
                }
            }
            Err(error) => return Err(invalid(error)),
            Ok(_) => {
                verify(managed, id, &file)?;
                retained.push(file);
            }
        }
    }
    let inventory = instance_mods::scan(managed, id).map_err(invalid)?;
    let provider = instance_content::ContentState::load(managed, id).map_err(invalid)?;
    for record in &provider.entries {
        instance_content::validate_provider_file(managed, id, record).map_err(invalid)?;
    }
    let mut result = TransitionPreview {
        instance_id: id.to_string(),
        current: record.installed().clone(),
        target: record.installed().clone(),
        requirements_added: vec![],
        requirements_removed: vec![],
        install: vec![],
        remove: vec![],
        retain: retained.clone(),
        warnings: vec![],
        blockers: vec![],
        fingerprint: String::new(),
    };
    let state_path = managed
        .instance_paths(id)
        .root()
        .join(aurora::AURORA_INSTALLED_FILE_NAME);
    let ownership_bytes = optional_bytes(&state_path)?;
    let ownership = aurora::load_installed_state(managed, id).map_err(invalid)?;
    let game_manifest =
        crate::install::state::load_installed_state(managed.instance_paths(id).game())
            .map_err(invalid)?;
    if record.installed().aurora.is_some() {
        instance_mods::verified_required_mods_with_manifest(
            managed,
            id,
            &aurora::merged_release_manifest(managed, id, endpoints.release_manifest())
                .map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(invalid)?;
    }
    if enabled == record.installed().aurora.is_some() {
        result
            .blockers
            .push("Aurora already has the requested state.".into());
    }
    let mut release_identity = None;
    if enabled {
        let loader = record
            .installed()
            .platform
            .require_fabric()
            .map_err(invalid)?;
        let release = endpoints
            .release_manifest()
            .releases()
            .iter()
            .find(|release| {
                release.minecraft_version() == record.installed().minecraft_version
                    && release.fabric_loader_version() == loader
            });
        if let Some(release) = release {
            release_identity = Some(serde_json::to_value(release).map_err(invalid)?);
            result.target.aurora = Some(AuroraPin {
                channel: release.channel(),
                version: release.aurora_version().into(),
            });
            result
                .requirements_added
                .push(format!("Aurora Client {}", release.aurora_version()));
            let mut required = vec![TransitionFile {
                relative_path: aurora::managed_artifact_relative_path(release).map_err(invalid)?,
                sha256: release.artifact().sha256().into(),
                size_bytes: release
                    .artifact()
                    .size_bytes()
                    .ok_or_else(|| invalid("Aurora release size is missing"))?,
                reason: "Required by the selected Aurora release".into(),
            }];
            if let Some(api) = release.fabric_api() {
                result
                    .requirements_added
                    .push(format!("Fabric API {}", api.version()));
                required.push(TransitionFile {
                    relative_path: format!("mods/fabric-api-{}.jar", api.version()),
                    sha256: api.artifact().sha256().into(),
                    size_bytes: api
                        .artifact()
                        .size_bytes()
                        .ok_or_else(|| invalid("Fabric API size is missing"))?,
                    reason: "Required by the selected Aurora release".into(),
                });
            }
            for file in required {
                let name = file
                    .relative_path
                    .strip_prefix("mods/")
                    .expect("derived mod path");
                let id_hint = if name.starts_with("aurora-") {
                    "aurora"
                } else {
                    "fabric-api"
                };
                let previous = retained.iter().find(|old| {
                    old.relative_path == file.relative_path
                        && old.sha256 == file.sha256
                        && old.size_bytes == file.size_bytes
                });
                let provider_claim = provider.entries.iter().any(|entry| {
                    entry.content_type == instance_content::ContentType::Mod
                        && (entry.file_name.eq_ignore_ascii_case(name)
                            || entry.sha256 == file.sha256)
                });
                let conflict = provider_claim
                    || inventory.entries.iter().any(|entry| {
                        let exact_retained = previous.is_some() && entry.file_name == name;
                        !exact_retained
                            && (entry.file_name.eq_ignore_ascii_case(name)
                                || entry.metadata.as_ref().is_some_and(|meta| {
                                    meta.id == id_hint
                                        || meta.nested_mod_ids.iter().any(|id| id == id_hint)
                                }))
                    });
                if conflict {
                    result.blockers.push(format!("Existing content conflicts with {name}; provider/local content cannot be adopted into launcher ownership."));
                }
                if previous.is_some() {
                    result
                        .retain
                        .retain(|old| old.relative_path != file.relative_path);
                    let mut reused = file.clone();
                    reused.reason =
                        "Reuse exact verified bootstrap artifact; it remains user controllable"
                            .into();
                    result.install.push(reused);
                } else {
                    result.install.push(file);
                }
            }
        } else {
            result.blockers.push("No authoritative Aurora release matches this exact Minecraft/platform combination.".into());
        }
    } else {
        result.target.aurora = None;
        if let Some(state) = ownership {
            let pin = record
                .installed()
                .aurora
                .as_ref()
                .ok_or_else(|| invalid("unexpected Aurora ownership"))?;
            let merged = aurora::merged_release_manifest(managed, id, endpoints.release_manifest())
                .map_err(|error| invalid(error.to_string()))?;
            let release = merged
                .resolve_exact(&pin.version, Some(pin.channel))
                .ok_or_else(|| invalid("release metadata unavailable"))?;
            super::platform::required_content(record.installed(), Some(release))
                .map_err(invalid)?;
            release_identity = Some(serde_json::to_value(release).map_err(invalid)?);
            if state.artifact().sha256() != release.artifact().sha256()
                || Some(state.artifact().size_bytes()) != release.artifact().size_bytes()
            {
                return Err(invalid("Aurora ownership disagrees with release"));
            }
            result
                .requirements_removed
                .push(format!("Aurora Client {}", state.aurora_version()));
            result.remove.push(from_artifact(
                state.artifact(),
                "Exact former launcher requirement; safely removable",
            ));
            let managed_name = state
                .artifact()
                .relative_path()
                .strip_prefix("mods/")
                .expect("managed mod path");
            if inventory.entries.iter().any(|entry| {
                entry.enabled
                    && entry.file_name != managed_name
                    && entry.metadata.as_ref().is_some_and(|meta| {
                        meta.id == "aurora" || meta.nested_mod_ids.iter().any(|id| id == "aurora")
                    })
            }) {
                result.blockers.push("Another active local/provider Aurora identity exists; disabling cannot remove or adopt it.".into());
            }
            if let Some(api) = state.fabric_api() {
                if release.fabric_api().is_none_or(|expected| {
                    expected.version() != api.version()
                        || expected.artifact().sha256() != api.artifact().sha256()
                        || expected.artifact().size_bytes() != Some(api.artifact().size_bytes())
                }) {
                    return Err(invalid("Fabric API ownership disagrees with release"));
                }
                result
                    .requirements_removed
                    .push(format!("Fabric API {}", api.version()));
                let ambiguous = inventory.entries.iter().any(|entry| {
                    entry.enabled
                        && entry.ownership != instance_mods::ModOwnership::LauncherManagedRequired
                        && entry.metadata.is_none()
                });
                let needed = ambiguous
                    || provider.entries.iter().any(|entry| {
                        entry.dependencies.iter().any(|dep| {
                            dep.kind == instance_content::DependencyKind::Required
                                && dep.provider == "modrinth"
                                && dep.project_id == "P7dR8mSH"
                        })
                    })
                    || inventory.entries.iter().any(|entry| {
                        entry.metadata.as_ref().is_some_and(|meta| {
                            meta.depends.iter().any(|dep| dep.mod_id == "fabric-api")
                        })
                    });
                let file = from_artifact(
                    api.artifact(),
                    if needed {
                        "Retained former launcher ownership: installed content requires Fabric API; no provider provenance is invented"
                    } else {
                        "No remaining declared requirement; exact launcher artifact safely removable"
                    },
                );
                if needed {
                    result.retain.push(file);
                } else {
                    result.remove.push(file);
                }
            }
            if inventory.entries.iter().any(|entry| {
                entry.metadata.as_ref().is_some_and(|meta| {
                    meta.id != "aurora" && meta.depends.iter().any(|dep| dep.mod_id == "aurora")
                })
            }) {
                result.blockers.push(
                    "Installed content requires Aurora; disabling it would break that requirement."
                        .into(),
                );
            }
        }
    }
    // An explicit configuration change reconciles missing bootstrap artifacts
    // without recreating them. Disabled bytes require a deliberate Mods action;
    // this transaction never creates a second active copy beside them.
    let mut present_removals = Vec::new();
    for file in result.remove.drain(..) {
        let path = file_path(managed, id, &file)?;
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if std::fs::symlink_metadata(path.with_extension("jar.disabled")).is_ok() {
                    return Err(invalid(
                        "Re-enable disabled bootstrap content in Mods before changing the original Aurora configuration.",
                    ));
                }
                result.warnings.push(format!(
                    "{} is already absent; it will not be recreated.",
                    file.relative_path
                ));
            }
            Err(error) => return Err(invalid(error)),
            Ok(_) => present_removals.push(file),
        }
    }
    result.remove = present_removals;
    let mut present_retained = Vec::new();
    for file in result.retain.drain(..) {
        let path = file_path(managed, id, &file)?;
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if std::fs::symlink_metadata(path.with_extension("jar.disabled")).is_ok() {
                    return Err(invalid(
                        "Re-enable disabled retained bootstrap content in Mods before changing the original Aurora configuration.",
                    ));
                }
            }
            Err(error) => return Err(invalid(error)),
            Ok(_) => present_retained.push(file),
        }
    }
    result.retain = present_retained;
    for file in &result.remove {
        verify(managed, id, file)?;
        let name = file
            .relative_path
            .strip_prefix("mods/")
            .expect("validated path");
        if provider
            .entries
            .iter()
            .any(|entry| entry.file_name.eq_ignore_ascii_case(name) || entry.sha256 == file.sha256)
        {
            result.blockers.push(format!(
                "Ambiguous provider claim for {name}; removal blocked."
            ));
        }
    }
    if crate::launch::process::snapshot(id.as_str())
        .status
        .blocks_launch()
    {
        result
            .blockers
            .push("Quit Minecraft before changing Aurora.".into());
    }
    if !result.retain.is_empty() {
        result.warnings.push("Retained artifacts are no longer launcher-required. Their former ownership is preserved separately; provider ownership is unchanged.".into());
    }
    let state = serde_json::json!({"record":record,"game":game_manifest,"ownership":ownership_bytes,"retained":retained,"provider":provider,"inventory":inventory,"release":release_identity,"plan":result});
    result.fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&state).map_err(invalid)?)
    );
    Ok(result)
}

/// Faults are consumed only by deterministic tests, never by commands.
#[derive(Default, Clone, Copy)]
pub struct TransitionFaults {
    pub fail_stage: bool,
    pub fail_activation: bool,
    pub fail_persistence: bool,
}

pub async fn apply(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    id: &InstanceId,
    enabled: bool,
    fingerprint: &str,
    faults: TransitionFaults,
) -> Result<InstanceRecord, TransitionError> {
    let approved = preview(managed, endpoints, id, enabled)?;
    if approved.fingerprint != fingerprint {
        return Err(TransitionError(
            "aurora_transition_stale",
            "The approved state changed. Request a new preview.".into(),
        ));
    }
    if !approved.blockers.is_empty() {
        return Err(TransitionError(
            "aurora_transition_blocked",
            approved.blockers.join(" "),
        ));
    }
    let cache = ArtifactCache::new(managed.clone());
    let mut acquired = Vec::new();
    if enabled {
        let pin = approved.target.aurora.as_ref().expect("enable target");
        let merged = aurora::merged_release_manifest(managed, id, endpoints.release_manifest())
            .map_err(|error| invalid(error.to_string()))?;
        let release = merged
            .resolve_exact(&pin.version, Some(pin.channel))
            .ok_or_else(|| invalid("release disappeared"))?;
        let game_plan = super::lifecycle::resolve_instance_game_plan(
            managed,
            &managed.instance_registry_file(),
            endpoints,
            id,
        )
        .await
        .map_err(invalid)?;
        if game_plan.java().major_version() != release.java().major_version() {
            return Err(invalid(
                "Aurora Java compatibility assertion disagrees with the resolved game requirement",
            ));
        }
        let retained = retained_files(managed, id)?;
        for file in &approved.install {
            if retained.iter().any(|old| {
                old.relative_path == file.relative_path
                    && old.sha256 == file.sha256
                    && old.size_bytes == file.size_bytes
            }) {
                verify(managed, id, file)?;
                continue;
            }
            let artifact = if file.relative_path.starts_with("mods/aurora-") {
                release.artifact()
            } else {
                release.fabric_api().expect("planned API").artifact()
            };
            let source = ArtifactSource::https_or_loopback(
                artifact.url(),
                artifact.sha256(),
                artifact.size_bytes(),
            )
            .map_err(invalid)?;
            let verified = cache
                .acquire_with(&source, endpoints.download_options())
                .await
                .map_err(invalid)?;
            acquired.push((file.clone(), verified.path));
        }
    }
    let mut outcome = None;
    instance_content::with_instance_lock(id, || {
        let _registry_guard = super::lifecycle::registry_lock();
        outcome = Some(commit(
            managed, endpoints, id, enabled, &approved, &acquired, faults,
        ));
        Ok(())
    })
    .map_err(invalid)?;
    outcome.expect("transaction closure executed")
}

fn commit(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    id: &InstanceId,
    enabled: bool,
    approved: &TransitionPreview,
    acquired: &[(TransitionFile, PathBuf)],
    faults: TransitionFaults,
) -> Result<InstanceRecord, TransitionError> {
    let current = preview(managed, endpoints, id, enabled)?;
    if current.fingerprint != approved.fingerprint {
        return Err(TransitionError(
            "aurora_transition_stale",
            "State changed during acquisition. Request a new preview.".into(),
        ));
    }
    if !current.blockers.is_empty() {
        return Err(invalid(current.blockers.join(" ")));
    }
    let _process_guard = crate::launch::process::lock_stopped(id.as_str()).map_err(invalid)?;
    let registry_path = managed.instance_registry_file();
    regular(&registry_path)?;
    let old_registry = std::fs::read(&registry_path).map_err(invalid)?;
    let mut registry = InstanceRegistry::load(&registry_path).map_err(invalid)?;
    let root = managed.instance_paths(id).root().to_owned();
    let state_path = root.join(aurora::AURORA_INSTALLED_FILE_NAME);
    let retained_path = root.join(RETAINED_FILE);
    let old_state = optional_bytes(&state_path)?;
    let old_retained = optional_bytes(&retained_path)?;
    let mut staged: Vec<(PathBuf, PathBuf)> = vec![];
    let mut backups: Vec<(PathBuf, PathBuf, PathBuf)> = vec![];
    let mut activated: Vec<PathBuf> = vec![];
    let mut state_touched = false;
    let mut registry_touched = false;
    let result = (|| {
        let retained = retained_files(managed, id)?;
        for (file, source) in acquired {
            let target = file_path(managed, id, file)?;
            if retained.iter().any(|old| {
                old.relative_path == file.relative_path
                    && old.sha256 == file.sha256
                    && old.size_bytes == file.size_bytes
            }) {
                verify(managed, id, file)?;
                continue;
            }
            if std::fs::symlink_metadata(&target).is_ok() {
                return Err(invalid("activation destination exists"));
            }
            regular(source)?;
            verify_file(
                source,
                &ArtifactDigest::parse(&file.sha256).map_err(invalid)?,
                Some(file.size_bytes),
            )
            .map_err(invalid)?;
            let stage = target
                .parent()
                .expect("mods child")
                .join(format!(".aurora-transition-stage-{}", uuid::Uuid::new_v4()));
            staged.push((target, stage.clone()));
            std::fs::copy(source, &stage).map_err(invalid)?;
            verify_file(
                &stage,
                &ArtifactDigest::parse(&file.sha256).map_err(invalid)?,
                Some(file.size_bytes),
            )
            .map_err(invalid)?;
        }
        if faults.fail_stage {
            return Err(invalid("injected staging failure"));
        }
        for file in &approved.remove {
            verify(managed, id, file)?;
            let target = file_path(managed, id, file)?;
            let backup = target.parent().expect("mods child").join(format!(
                ".aurora-transition-retired-{}",
                uuid::Uuid::new_v4()
            ));
            let rollback_copy = target.parent().expect("mods child").join(format!(
                ".aurora-transition-rollback-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::copy(&target, &rollback_copy).map_err(invalid)?;
            verify_file(
                &rollback_copy,
                &ArtifactDigest::parse(&file.sha256).map_err(invalid)?,
                Some(file.size_bytes),
            )
            .map_err(invalid)?;
            backups.push((target.clone(), backup.clone(), rollback_copy));
            std::fs::rename(&target, &backup).map_err(invalid)?;
        }
        for (target, stage) in &staged {
            // Refuse an external collision rather than overwriting it.
            std::fs::hard_link(stage, target).map_err(invalid)?;
            activated.push(target.clone());
        }
        if faults.fail_activation {
            return Err(invalid("injected activation failure"));
        }
        state_touched = true;
        if enabled {
            let pin = approved.target.aurora.as_ref().expect("enable target");
            let merged = aurora::merged_release_manifest(managed, id, endpoints.release_manifest())
                .map_err(|error| invalid(error.to_string()))?;
            let release = merged
                .resolve_exact(&pin.version, Some(pin.channel))
                .expect("revalidated release");
            for file in &approved.install {
                verify(managed, id, file)?;
            }
            let state = aurora::AuroraInstalledState::for_verified_release(
                release,
                approved.install[0].size_bytes,
                approved.install.get(1).map(|file| file.size_bytes),
            )
            .map_err(invalid)?;
            atomic_write(&state_path, Some(state.to_json().as_bytes()))?;
        } else {
            atomic_write(&state_path, None)?;
        }
        let retained = RetainedState {
            schema_version: 1,
            files: approved.retain.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&retained).map_err(invalid)?;
        atomic_write(
            &retained_path,
            (!retained.files.is_empty()).then_some(bytes.as_slice()),
        )?;
        let record = registry.find_mut(id).expect("revalidated record");
        let mut desired = record.configuration().clone();
        desired.set_aurora_enabled(enabled);
        record.set_configuration(desired);
        record.set_installed(approved.target.clone());
        let updated = record.clone();
        let validation = validate_instance(managed, &registry, id).map_err(invalid)?;
        if validation.status != InstanceStatus::Ready {
            return Err(invalid("target configuration failed final validation"));
        }
        if faults.fail_persistence {
            return Err(invalid("injected persistence failure"));
        }
        registry_touched = true;
        registry.save(&registry_path).map_err(invalid)?;
        instance_mods::verified_required_mods_with_manifest(
            managed,
            id,
            &aurora::merged_release_manifest(managed, id, endpoints.release_manifest())
                .map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(invalid)?;
        // Rollback copies remain until every retirement and staging cleanup succeeds.
        for (_, stage) in &staged {
            regular(stage)?;
            std::fs::remove_file(stage).map_err(invalid)?;
        }
        for (_, backup, _) in &backups {
            regular(backup)?;
            std::fs::remove_file(backup).map_err(invalid)?;
        }
        Ok(updated)
    })();
    if let Err(error) = result {
        let rollback = (|| {
            for target in activated.iter().rev() {
                std::fs::remove_file(target).map_err(invalid)?;
            }
            for (target, backup, rollback_copy) in backups.iter().rev() {
                if backup.exists() {
                    std::fs::rename(backup, target).map_err(invalid)?;
                } else if !target.exists() {
                    std::fs::copy(rollback_copy, target).map_err(invalid)?;
                }
            }
            if state_touched {
                atomic_write(&state_path, old_state.as_deref())?;
                atomic_write(&retained_path, old_retained.as_deref())?;
            }
            if registry_touched {
                atomic_write(&registry_path, Some(&old_registry))?;
            }
            Ok::<_, TransitionError>(())
        })();
        if let Err(rollback) = rollback {
            return Err(TransitionError(
                "aurora_transition_rollback_failed",
                format!(
                    "{error}; rollback failed: {rollback}. Exact recovery files are preserved."
                ),
            ));
        }
        for (_, stage) in &staged {
            if stage.exists() {
                regular(stage)?;
                std::fs::remove_file(stage).map_err(invalid)?;
            }
        }
        for (_, _, copy) in &backups {
            regular(copy)?;
            std::fs::remove_file(copy).map_err(invalid)?;
        }
        return Err(error);
    }
    // These recovery copies are no longer transaction inputs after commit.
    // A cleanup failure preserves recovery bytes and does not misreport a
    // committed, validated configuration as a failed transition.
    for (_, _, copy) in &backups {
        if regular(copy)
            .and_then(|_| std::fs::remove_file(copy).map_err(invalid))
            .is_err()
        {
            eprintln!(
                "[aurora-transition] committed; an exact recovery copy could not be cleaned up"
            );
        }
    }
    result
}

fn atomic_write(path: &Path, bytes: Option<&[u8]>) -> Result<(), TransitionError> {
    if let Some(bytes) = bytes {
        let temporary =
            path.with_file_name(format!(".aurora-transition-state-{}", uuid::Uuid::new_v4()));
        use std::io::Write;
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(invalid)?;
            file.write_all(bytes).map_err(invalid)?;
            file.sync_all().map_err(invalid)?;
            std::fs::rename(&temporary, path).map_err(invalid)
        })();
        if result.is_err() && temporary.exists() {
            let _ = std::fs::remove_file(temporary);
        }
        result
    } else {
        if optional_bytes(path)?.is_some() {
            std::fs::remove_file(path).map_err(invalid)?;
        }
        Ok(())
    }
}
