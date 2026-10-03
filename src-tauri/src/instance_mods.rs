//! Local, filesystem-authoritative inventory and safe mutation of one
//! registered instance's Fabric mods.
//!
//! The scanner is deliberately shallow: it reads only direct children of the
//! derived `instances/<id>/mods` directory. JARs are untrusted input. Aurora
//! never executes them, never extracts them, and reads only a root-level
//! `fabric.mod.json` under a 256 KiB uncompressed limit. Archives with more
//! than 4,096 entries or files larger than 512 MiB are reported without
//! metadata inspection. Icons are intentionally not extracted in this phase.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use crate::aurora;
use crate::instance_content::{ContentState, ContentType, ProviderRecord};
use crate::instances::InstanceId;
use crate::integrity::{ArtifactDigest, verify_file};
use crate::paths::ManagedPaths;

const MAX_METADATA_BYTES: u64 = 256 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4_096;
const MAX_INSPECTED_JAR_BYTES: u64 = 512 * 1024 * 1024;
const MAX_NESTED_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_NESTED_DEPTH: usize = 4;
const MAX_NESTED_COUNT: usize = 128;
const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInventory {
    pub instance_id: String,
    pub entries: Vec<ModEntry>,
    pub missing_managed: Vec<ProviderRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub entry_id: String,
    pub file_name: String,
    pub display_name: String,
    pub enabled: bool,
    pub file_type: ModFileType,
    pub size_bytes: Option<u64>,
    pub modified_unix_millis: Option<u64>,
    pub ownership: ModOwnership,
    pub sha256: Option<String>,
    pub provenance: Option<ProviderRecord>,
    pub metadata: Option<ModMetadata>,
    pub warnings: Vec<ModWarning>,
    pub can_toggle: bool,
    pub can_remove: bool,
    pub removal_blocked_reason: Option<String>,
    pub action_blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModFileType {
    EnabledJar,
    DisabledJar,
    UnexpectedFile,
    Directory,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModOwnership {
    LauncherBootstrap,
    LauncherManagedRequired,
    LauncherManagedRetained,
    ProviderManaged,
    UserManaged,
    Unknown,
}

pub(crate) fn bootstrap_status(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<Option<String>, ModError> {
    let Some(state) = aurora::load_installed_state(managed, instance)
        .map_err(|e| ModError::InstalledState(e.to_string()))?
    else {
        return Ok(None);
    };
    let mods = validate_mods_directory(managed, instance)?;
    let name = state
        .artifact()
        .relative_path()
        .strip_prefix("mods/")
        .expect("validated path");
    let active = mods.join(name);
    let disabled = mods.join(format!("{name}.disabled"));
    let has_active = std::fs::symlink_metadata(&active).is_ok();
    let has_disabled = std::fs::symlink_metadata(&disabled).is_ok();
    if has_active && has_disabled {
        return Ok(Some("modified".into()));
    }
    if !has_active && !has_disabled {
        return Ok(Some("missing".into()));
    }
    let path = if has_active { active } else { disabled };
    let name = path.file_name().expect("mod filename").to_string_lossy();
    let valid = validate_current_regular_file(&mods, &name).is_ok()
        && ArtifactDigest::parse(state.artifact().sha256()).is_ok_and(|digest| {
            verify_file(&path, &digest, Some(state.artifact().size_bytes())).is_ok()
        });
    Ok(Some(
        if !valid {
            "modified"
        } else if has_active {
            "active"
        } else {
            "disabled"
        }
        .into(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModMetadata {
    pub id: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub environment: Option<String>,
    pub depends: Vec<ModRelation>,
    pub recommends: Vec<ModRelation>,
    pub suggests: Vec<ModRelation>,
    pub conflicts: Vec<ModRelation>,
    pub breaks: Vec<ModRelation>,
    pub has_declared_icon: bool,
    pub nested_mod_ids: Vec<String>,
    #[serde(skip_serializing)]
    pub(crate) nested_mod_versions: HashMap<String, Option<String>>,
    pub mixin_java_requirements: Vec<MixinJavaRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixinJavaRequirement {
    pub mod_id: String,
    pub config: String,
    pub java_major: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRelation {
    pub mod_id: String,
    pub requirement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModWarning {
    pub code: String,
    pub message: String,
}

impl ModWarning {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FabricMetadataDocument {
    id: Option<String>,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    #[serde(default)]
    authors: Vec<serde_json::Value>,
    environment: Option<String>,
    #[serde(default)]
    depends: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    recommends: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    suggests: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    conflicts: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    breaks: serde_json::Map<String, serde_json::Value>,
    icon: Option<serde_json::Value>,
    #[serde(default)]
    jars: Vec<NestedJarDeclaration>,
    #[serde(default)]
    mixins: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct NestedJarDeclaration {
    file: String,
}

/// Reads the authoritative local inventory for one validated instance.
pub fn scan(managed: &ManagedPaths, instance: &InstanceId) -> Result<ModInventory, ModError> {
    let mods = validate_mods_directory(managed, instance)?;
    let managed_file = managed_artifact_file_name(managed, instance)?;
    // The loader family owns the metadata format read from mod jars.
    let platform_kind = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| ModError::InstalledState(error.to_string()))?
        .find(instance)
        .map(|record| record.installed().platform.kind().to_owned())
        .unwrap_or_else(|| "fabric".to_owned());
    let content_state = ContentState::load(managed, instance)
        .map_err(|error| ModError::ContentState(error.to_string()))?;
    let mut entries = Vec::new();
    let children = std::fs::read_dir(&mods).map_err(|source| ModError::DirectoryRead { source })?;

    for child in children {
        match child {
            Ok(child) => {
                let name = child.file_name().to_string_lossy().into_owned();
                let provider = content_state.entries.iter().find(|record| {
                    record.content_type == ContentType::Mod
                        && (record.file_name.eq_ignore_ascii_case(&name)
                            || format!("{}.disabled", record.file_name).eq_ignore_ascii_case(&name))
                });
                entries.push(inspect_entry(
                    &child.path(),
                    managed_file.as_deref(),
                    provider,
                    &platform_kind,
                ));
            }
            Err(source) => entries.push(ModEntry {
                entry_id: opaque_id(b"unreadable-directory-entry"),
                file_name: "Unreadable entry".to_owned(),
                display_name: "Unreadable entry".to_owned(),
                enabled: false,
                file_type: ModFileType::UnexpectedFile,
                size_bytes: None,
                modified_unix_millis: None,
                ownership: ModOwnership::Unknown,
                sha256: None,
                provenance: None,
                metadata: None,
                warnings: vec![ModWarning::new(
                    "entry_unreadable",
                    format!("A directory entry could not be inspected: {source}"),
                )],
                can_toggle: false,
                can_remove: false,
                removal_blocked_reason: None,
                action_blocked_reason: Some(
                    "Aurora cannot safely identify this directory entry.".to_owned(),
                ),
            }),
        }
    }

    // Legacy ownership documents now describe bootstrap provenance. This is
    // an idempotent interpretation migration: original records and bytes stay
    // recoverable, and no provider identities or files are manufactured.
    let bootstrap = aurora::load_installed_state(managed, instance)
        .map_err(|e| ModError::InstalledState(e.to_string()))?;
    if managed_file.is_none()
        && let Some(state) = &bootstrap
    {
        for artifact in
            std::iter::once(state.artifact()).chain(state.fabric_api().map(|api| api.artifact()))
        {
            let name = artifact
                .relative_path()
                .strip_prefix("mods/")
                .expect("validated mod path");
            for entry in entries.iter_mut().filter(|entry| {
                entry.file_name.eq_ignore_ascii_case(name)
                    || entry
                        .file_name
                        .eq_ignore_ascii_case(&format!("{name}.disabled"))
            }) {
                if entry.ownership == ModOwnership::ProviderManaged {
                    continue;
                }
                let matches = matches!(
                    entry.file_type,
                    ModFileType::EnabledJar | ModFileType::DisabledJar
                ) && !content_state.entries.iter().any(|record| {
                    record.content_type == ContentType::Mod
                        && (record.file_name.eq_ignore_ascii_case(name)
                            || record.sha256 == artifact.sha256())
                }) && ArtifactDigest::parse(artifact.sha256()).is_ok_and(|digest| {
                    verify_file(
                        &mods.join(&entry.file_name),
                        &digest,
                        Some(artifact.size_bytes()),
                    )
                    .is_ok()
                });
                entry.ownership = if matches {
                    ModOwnership::LauncherBootstrap
                } else {
                    ModOwnership::Unknown
                };
                entry.sha256 = matches.then(|| artifact.sha256().to_owned());
                entry.can_toggle = matches;
                entry.can_remove = matches;
                entry.action_blocked_reason = (!matches).then(|| "The bootstrap file was modified; inspect it in the instance folder before changing it.".into());
            }
        }
    }
    let retained = crate::instances::transition::retained_files(managed, instance)
        .map_err(|e| ModError::InstalledState(e.to_string()))?;
    for file in retained {
        let name = file
            .relative_path
            .strip_prefix("mods/")
            .expect("validated retained mod path");
        if let Some(entry) = entries.iter_mut().find(|entry| {
            entry.file_name.eq_ignore_ascii_case(name)
                || entry
                    .file_name
                    .eq_ignore_ascii_case(&format!("{name}.disabled"))
        }) {
            let matches = matches!(
                entry.file_type,
                ModFileType::EnabledJar | ModFileType::DisabledJar
            ) && entry.size_bytes == Some(file.size_bytes)
                && entry.provenance.is_none()
                && ArtifactDigest::parse(&file.sha256).is_ok_and(|digest| {
                    verify_file(&mods.join(&entry.file_name), &digest, Some(file.size_bytes))
                        .is_ok()
                });
            if matches {
                entry.sha256 = Some(file.sha256.clone());
            }
            entry.ownership = if matches {
                ModOwnership::LauncherBootstrap
            } else {
                ModOwnership::Unknown
            };
            entry.can_toggle = matches;
            entry.can_remove = matches;
            entry.action_blocked_reason = (!matches).then_some(file.reason);
        }
    }
    derive_local_warnings(&mut entries, &platform_kind);
    let active_providers: HashSet<_> = entries
        .iter()
        .filter(|entry| entry.enabled)
        .filter_map(|entry| entry.provenance.as_ref().map(ProviderRecord::identity))
        .collect();
    for entry in entries.iter_mut().filter(|entry| entry.enabled) {
        if entry.provenance.as_ref().is_some_and(|provider| {
            provider
                .requires
                .iter()
                .any(|required| !active_providers.contains(required))
        }) {
            entry.warnings.push(ModWarning::new(
                "required_dependency_missing",
                "A recorded required provider dependency is disabled or missing.",
            ));
        }
    }
    let disabled = DisabledState::load(managed, instance)?;
    for entry in &mut entries {
        if let Some(expected) = disabled.files.get(&entry.file_name) {
            if entry.file_type != ModFileType::DisabledJar
                || !ArtifactDigest::parse(expected).is_ok_and(|digest| {
                    verify_file(&mods.join(&entry.file_name), &digest, entry.size_bytes).is_ok()
                })
            {
                entry.ownership = ModOwnership::Unknown;
                entry.can_toggle = false;
                entry.can_remove = false;
                entry.action_blocked_reason = Some("Disabled bytes changed. Inspect the file in the instance folder; the launcher will not activate it.".into());
            } else {
                entry.sha256 = Some(expected.clone());
            }
        }
    }
    entries.sort_by(|left, right| {
        left.display_name
            .to_lowercase()
            .cmp(&right.display_name.to_lowercase())
            .then_with(|| {
                left.file_name
                    .to_lowercase()
                    .cmp(&right.file_name.to_lowercase())
            })
    });
    let mut inventory = ModInventory {
        instance_id: instance.to_string(),
        missing_managed: content_state
            .entries
            .iter()
            .filter(|record| {
                record.content_type == ContentType::Mod
                    && !entries.iter().any(|entry| {
                        entry.file_name.eq_ignore_ascii_case(&record.file_name)
                            || entry
                                .file_name
                                .eq_ignore_ascii_case(&format!("{}.disabled", record.file_name))
                    })
            })
            .cloned()
            .collect(),
        entries,
    };
    let blockers: Vec<_> = inventory
        .entries
        .iter()
        .map(|entry| {
            dependency_blockers(&inventory, entry, &platform_kind)
                .err()
                .map(|reason| reason.to_string())
        })
        .collect();
    for (entry, reason) in inventory.entries.iter_mut().zip(blockers) {
        entry.removal_blocked_reason = reason;
    }
    if let Some(pack) = crate::pack_state::InstalledPack::load(managed, instance)
        .map_err(|error| ModError::State(error.to_string()))?
    {
        for entry in &mut inventory.entries {
            let base = entry
                .file_name
                .strip_suffix(".disabled")
                .unwrap_or(&entry.file_name);
            if pack.owns_path(&format!("mods/{base}")) {
                let reason = format!(
                    "Required by {} {}. Modpack components are updated through Update Modpack on the instance's Overview page.",
                    pack.identity.name, pack.identity.pack_version
                );
                entry.can_remove = false;
                entry.can_toggle = false;
                entry.removal_blocked_reason = Some(reason.clone());
                entry.action_blocked_reason = Some(reason);
            }
        }
    }
    Ok(inventory)
}

/// Proves that the mods directory is the exact derived directory beneath the
/// canonical managed instance. The canonical path is validation evidence;
/// callers keep using the ordinary platform path for OS APIs.
pub fn validate_mods_directory(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<PathBuf, ModError> {
    let instance_paths = managed.instance_paths(instance);
    let mods = instance_paths.mods().to_path_buf();
    if !mods.is_dir() {
        return Err(ModError::DirectoryMissing);
    }
    let canonical_root = std::fs::canonicalize(managed.data_root())
        .map_err(|source| ModError::Boundary { source })?;
    let canonical_instances = std::fs::canonicalize(managed.instances_dir())
        .map_err(|source| ModError::Boundary { source })?;
    let canonical_instance = std::fs::canonicalize(instance_paths.root())
        .map_err(|source| ModError::Boundary { source })?;
    let canonical_mods =
        std::fs::canonicalize(&mods).map_err(|source| ModError::Boundary { source })?;

    if !canonical_instances.starts_with(&canonical_root)
        || canonical_instance != canonical_instances.join(instance.as_str())
        || canonical_mods != canonical_instance.join("mods")
    {
        return Err(ModError::BoundaryEscape);
    }
    Ok(mods)
}

pub(crate) fn managed_artifact_file_name(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<Option<Vec<String>>, ModError> {
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| ModError::InstalledState(error.to_string()))?;
    // Registered instances use user-controllable bootstrap provenance. Keeping
    // the legacy fallback for isolated component callers does not authorize
    // any application operation: commands require registry membership.
    if registry.find(instance).is_some() {
        aurora::load_installed_state(managed, instance)
            .map_err(|error| ModError::InstalledState(error.to_string()))?;
        return Ok(None);
    }
    if let Some(record) = registry.find(instance) {
        if record.installed().aurora.is_none() {
            return Ok(None);
        }
        record
            .installed()
            .platform
            .require_fabric()
            .map_err(ModError::InstalledState)?;
    }
    let mut configured_files = Vec::new();
    if let Some(record) = registry.find(instance) {
        let manifest = crate::distribution::operational_manifest()
            .map_err(|e| ModError::InstalledState(e.to_string()))?;
        let pin = record
            .installed()
            .aurora
            .as_ref()
            .expect("absence returned above");
        if let Some(release) = manifest.resolve_exact(&pin.version, Some(pin.channel)) {
            let requirements =
                crate::instances::platform::required_content(record.installed(), Some(release))
                    .map_err(ModError::InstalledState)?;
            if requirements.aurora {
                configured_files.push(
                    aurora::managed_artifact_relative_path(release)
                        .map_err(|e| ModError::InstalledState(e.to_string()))?
                        .strip_prefix("mods/")
                        .expect("managed mod path")
                        .to_owned(),
                );
            }
            if requirements.fabric_api {
                configured_files.push(format!(
                    "fabric-api-{}.jar",
                    release
                        .fabric_api()
                        .expect("requirement derived from release")
                        .version()
                ));
            }
        }
    }
    let state = aurora::load_installed_state(managed, instance)
        .map_err(|error| ModError::InstalledState(error.to_string()))?;
    if state.is_none() {
        return Ok((!configured_files.is_empty()).then_some(configured_files));
    }
    Ok(state.map(|state| {
        let mut files = vec![
            state
                .artifact()
                .relative_path()
                .strip_prefix("mods/")
                .expect("validated Aurora state always lives beneath mods")
                .to_owned(),
        ];
        if let Some(fabric_api) = state.fabric_api() {
            files.push(
                fabric_api
                    .artifact()
                    .relative_path()
                    .strip_prefix("mods/")
                    .expect("validated Fabric API state always lives beneath mods")
                    .to_owned(),
            );
        }
        for configured in configured_files {
            if !files.contains(&configured) {
                files.push(configured);
            }
        }
        files
    }))
}

/// Active launcher ownership, verified before provider reconciliation or mutation.
/// Read mod identities from the proven artifacts, never reserve an ID globally.
/// Missing, ambiguous, or damaged requirements cannot satisfy provider dependencies.
pub(crate) fn verified_required_mods(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<Vec<ModMetadata>, ModError> {
    let manifest = crate::distribution::operational_manifest()
        .map_err(|error| ModError::InstalledState(error.to_string()))
        .and_then(|embedded| {
            crate::aurora::merged_release_manifest(managed, instance, &embedded)
                .map_err(|error| ModError::InstalledState(error.to_string()))
        })?;
    verified_required_mods_with_manifest(managed, instance, &manifest)
}

pub(crate) fn verified_required_mods_with_manifest(
    managed: &ManagedPaths,
    instance: &InstanceId,
    manifest: &crate::distribution::ReleaseManifest,
) -> Result<Vec<ModMetadata>, ModError> {
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| ModError::InstalledState(error.to_string()))?;
    crate::instance_content::validate_directory(
        managed,
        instance,
        crate::instance_content::ContentType::Mod,
    )
    .map_err(|error| ModError::InstalledState(error.to_string()))?;
    let state_path = managed
        .instance_paths(instance)
        .root()
        .join(aurora::AURORA_INSTALLED_FILE_NAME);
    if let Ok(meta) = std::fs::symlink_metadata(&state_path) {
        if !meta.is_file() || meta.file_type().is_symlink() || is_reparse_point(&meta) {
            return Err(ModError::InstalledState(
                "managed ownership state is not a regular file".into(),
            ));
        }
    }
    let state = aurora::load_installed_state(managed, instance)
        .map_err(|error| ModError::InstalledState(error.to_string()))?;
    if let Some(record) = registry.find(instance) {
        if !record.configuration().matches_installed(record.installed()) {
            return Err(ModError::InstalledState(
                "instance configuration is stale".into(),
            ));
        }
        if record.installed().aurora.is_none() {
            if state.is_some() {
                return Err(ModError::InstalledState(
                    "managed Aurora state remains without an active requirement".into(),
                ));
            }
            return Ok(Vec::new());
        }
        let pin = record.installed().aurora.as_ref().expect("checked above");
        let release = manifest
            .resolve_exact(&pin.version, Some(pin.channel))
            .ok_or_else(|| ModError::InstalledState("active release metadata is missing".into()))?;
        crate::instances::platform::required_content(record.installed(), Some(release))
            .map_err(ModError::InstalledState)?;
        let Some(installed) = state.as_ref() else {
            return Ok(Vec::new());
        };
        if installed.aurora_version() != pin.version
            || installed.channel() != pin.channel
            || installed.minecraft_version() != record.installed().minecraft_version
            || Some(installed.fabric_loader_version()) != record.installed().platform.version()
            || installed.artifact().relative_path()
                != aurora::managed_artifact_relative_path(release)
                    .map_err(|error| ModError::InstalledState(error.to_string()))?
            || installed.artifact().sha256() != release.artifact().sha256()
            || Some(installed.artifact().size_bytes()) != release.artifact().size_bytes()
            || installed.fabric_api().map(|api| {
                (
                    api.version(),
                    api.artifact().sha256(),
                    Some(api.artifact().size_bytes()),
                )
            }) != release.fabric_api().map(|api| {
                (
                    api.version(),
                    api.artifact().sha256(),
                    api.artifact().size_bytes(),
                )
            })
        {
            return Err(ModError::InstalledState(
                "managed requirements do not match active release metadata".into(),
            ));
        }
    }
    let Some(state) = state else {
        return Ok(Vec::new());
    };
    let directory = validate_mods_directory(managed, instance)?;
    let provider_state = crate::instance_content::ContentState::load(managed, instance)
        .map_err(|error| ModError::ContentState(error.to_string()))?;
    let artifacts =
        std::iter::once(state.artifact()).chain(state.fabric_api().map(|api| api.artifact()));
    let mut identities = Vec::new();
    for artifact in artifacts {
        let name = artifact
            .relative_path()
            .strip_prefix("mods/")
            .expect("validated managed artifact path");
        if provider_state.entries.iter().any(|record| {
            record.content_type == crate::instance_content::ContentType::Mod
                && (record.file_name.eq_ignore_ascii_case(name)
                    || record.sha256 == artifact.sha256())
        }) {
            if registry.find(instance).is_some() {
                continue;
            }
            return Err(ModError::InstalledState(
                "provider state ambiguously claims a launcher requirement".into(),
            ));
        }
        let path = directory.join(name);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && registry.find(instance).is_some() =>
            {
                continue;
            }
            Err(error) => return Err(ModError::InstalledState(error.to_string())),
        };
        if !meta.is_file() || meta.file_type().is_symlink() || is_reparse_point(&meta) {
            return Err(ModError::InstalledState(
                "managed requirement is not a regular file".into(),
            ));
        }
        let digest = ArtifactDigest::parse(artifact.sha256())
            .map_err(|error| ModError::InstalledState(error.to_string()))?;
        verify_file(&path, &digest, Some(artifact.size_bytes()))
            .map_err(|error| ModError::InstalledState(error.to_string()))?;
        let (metadata, warnings) = inspect_fabric_metadata(&path, artifact.size_bytes());
        if !warnings.is_empty() {
            return Err(ModError::InstalledState(
                "managed requirement metadata is ambiguous".into(),
            ));
        }
        identities.push(metadata.ok_or_else(|| {
            ModError::InstalledState("managed requirement has no mod identity".into())
        })?);
    }
    Ok(identities)
}

fn inspect_entry(
    path: &Path,
    managed_files: Option<&[String]>,
    provider: Option<&ProviderRecord>,
    platform_kind: &str,
) -> ModEntry {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unreadable entry".to_owned());
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) => {
            return unavailable_entry(file_name, format!("The entry could not be read: {source}"));
        }
    };
    let is_link = metadata.file_type().is_symlink() || is_reparse_point(&metadata);
    let file_type = if is_link {
        ModFileType::Link
    } else if metadata.is_dir() {
        ModFileType::Directory
    } else if metadata.is_file() && is_enabled_jar(&file_name) {
        ModFileType::EnabledJar
    } else if metadata.is_file() && is_disabled_jar(&file_name) {
        ModFileType::DisabledJar
    } else {
        ModFileType::UnexpectedFile
    };
    let enabled = file_type == ModFileType::EnabledJar;
    let is_managed = managed_files.is_some_and(|files| {
        files.iter().any(|managed| {
            file_name.eq_ignore_ascii_case(managed)
                || file_name.eq_ignore_ascii_case(&format!("{managed}{DISABLED_SUFFIX}"))
        })
    });
    let mut ownership = if is_managed {
        ModOwnership::LauncherManagedRequired
    } else if matches!(
        file_type,
        ModFileType::EnabledJar | ModFileType::DisabledJar
    ) {
        ModOwnership::UserManaged
    } else {
        ModOwnership::Unknown
    };
    let mut sha256 = None;
    let mut provenance = None;
    let provider_mismatch = if !is_managed {
        if let Some(provider) = provider {
            if matches!(
                file_type,
                ModFileType::EnabledJar | ModFileType::DisabledJar
            ) {
                if let Ok(digest) = ArtifactDigest::parse(&provider.sha256) {
                    if verify_file(path, &digest, None).is_ok() {
                        ownership = ModOwnership::ProviderManaged;
                        sha256 = Some(provider.sha256.clone());
                        provenance = Some(provider.clone());
                        false
                    } else {
                        ownership = ModOwnership::Unknown;
                        true
                    }
                } else {
                    ownership = ModOwnership::Unknown;
                    true
                }
            } else {
                ownership = ModOwnership::Unknown;
                true
            }
        } else {
            false
        }
    } else {
        false
    };
    let size_bytes = metadata.is_file().then_some(metadata.len());
    let modified_unix_millis = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok());
    let mut id_material = Vec::new();
    id_material.extend_from_slice(file_name.as_bytes());
    id_material.extend_from_slice(
        format!("|{file_type:?}|{:?}|{modified_unix_millis:?}", size_bytes).as_bytes(),
    );
    if matches!(
        file_type,
        ModFileType::EnabledJar | ModFileType::DisabledJar
    ) {
        if let Ok(hash) = file_digest(path) {
            id_material.extend_from_slice(hash.as_bytes());
            if sha256.is_none() {
                sha256 = Some(hash);
            }
        } else {
            ownership = ModOwnership::Unknown;
        }
    }
    let entry_id = opaque_id(&id_material);

    let (fabric_metadata, mut warnings) = if matches!(
        file_type,
        ModFileType::EnabledJar | ModFileType::DisabledJar
    ) {
        inspect_mod_metadata(path, size_bytes.unwrap_or_default(), platform_kind)
    } else {
        (None, Vec::new())
    };
    if file_type == ModFileType::Link {
        warnings.push(ModWarning::new(
            "link_not_managed",
            "Links and Windows reparse points are shown but never followed or modified.",
        ));
    } else if file_type == ModFileType::Directory {
        warnings.push(ModWarning::new(
            "nested_directory_ignored",
            "Nested directories are not scanned or managed as mods.",
        ));
    } else if file_type == ModFileType::UnexpectedFile {
        warnings.push(ModWarning::new(
            "unexpected_file",
            "This file is not an enabled or disabled JAR and is left untouched.",
        ));
    }
    if is_managed && !enabled {
        warnings.push(ModWarning::new(
            "required_mod_disabled",
            "A required managed mod is disabled outside the launcher; instance readiness may be damaged.",
        ));
    }
    if provider_mismatch {
        warnings.push(ModWarning::new(
            "content_hash_mismatch",
            "This file no longer matches its provider-managed record. Actions are blocked.",
        ));
    }
    let blocked_reason = match ownership {
        ModOwnership::LauncherBootstrap => None,
        ModOwnership::LauncherManagedRetained => {
            Some("Retained former launcher artifact; no active launcher requirement.".into())
        }
        ModOwnership::LauncherManagedRequired => Some(
            "This mod is required and is maintained by the verified installation system."
                .to_owned(),
        ),
        ModOwnership::Unknown => {
            Some("Aurora cannot prove that this entry is a user-managed mod file.".to_owned())
        }
        ModOwnership::UserManaged => None,
        ModOwnership::ProviderManaged => {
            Some("Use the provider lifecycle preview to remove this item safely.".to_owned())
        }
    };
    let display_name = fabric_metadata
        .as_ref()
        .and_then(|metadata| metadata.name.as_deref())
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            fabric_metadata
                .as_ref()
                .map(|metadata| metadata.id.trim())
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| {
            if is_managed {
                "Aurora Client".to_owned()
            } else {
                file_name.clone()
            }
        });

    ModEntry {
        entry_id,
        file_name,
        display_name,
        enabled,
        file_type,
        size_bytes,
        modified_unix_millis,
        ownership,
        sha256,
        provenance,
        metadata: fabric_metadata,
        warnings,
        can_toggle: matches!(
            ownership,
            ModOwnership::UserManaged | ModOwnership::ProviderManaged
        ),
        can_remove: ownership == ModOwnership::UserManaged,
        removal_blocked_reason: None,
        action_blocked_reason: blocked_reason,
    }
}

fn unavailable_entry(file_name: String, reason: String) -> ModEntry {
    ModEntry {
        entry_id: opaque_id(file_name.as_bytes()),
        display_name: file_name.clone(),
        file_name,
        enabled: false,
        file_type: ModFileType::UnexpectedFile,
        size_bytes: None,
        modified_unix_millis: None,
        ownership: ModOwnership::Unknown,
        sha256: None,
        provenance: None,
        metadata: None,
        warnings: vec![ModWarning::new("entry_unreadable", reason)],
        can_toggle: false,
        can_remove: false,
        removal_blocked_reason: None,
        action_blocked_reason: Some("Aurora cannot safely inspect this entry.".to_owned()),
    }
}

/// The platform kind of one instance's installed pin, for loader-family
/// aware metadata inspection.
pub(crate) fn platform_kind_of(managed: &ManagedPaths, instance: &InstanceId) -> String {
    crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .ok()
        .and_then(|registry| {
            registry
                .find(instance)
                .map(|record| record.installed().platform.kind().to_owned())
        })
        .unwrap_or_else(|| "fabric".to_owned())
}

/// Reads one mod JAR's metadata in the format its instance's loader
/// family declares. The instance platform is the authority: NeoForge
/// instances read `META-INF/neoforge.mods.toml` (with the legacy
/// `META-INF/mods.toml` spelling accepted), everything else reads
/// `fabric.mod.json`.
pub(crate) fn inspect_mod_metadata(
    path: &Path,
    jar_size: u64,
    platform_kind: &str,
) -> (Option<ModMetadata>, Vec<ModWarning>) {
    if platform_kind == "neoForge" {
        inspect_neoforge_metadata(path, jar_size)
    } else {
        inspect_fabric_metadata(path, jar_size)
    }
}

/// Reads the NeoForge mod metadata of one JAR into the shared normalized
/// model: `depends` carries `required` dependencies, `recommends` carries
/// `optional`, and `conflicts` carries `incompatible` declarations.
pub(crate) fn inspect_neoforge_metadata(
    path: &Path,
    jar_size: u64,
) -> (Option<ModMetadata>, Vec<ModWarning>) {
    if jar_size > MAX_INSPECTED_JAR_BYTES {
        return (
            None,
            vec![ModWarning::new(
                "jar_too_large",
                "Metadata inspection was skipped because this JAR exceeds the 512 MiB safety bound.",
            )],
        );
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(source) => {
            return (
                None,
                vec![ModWarning::new(
                    "jar_unreadable",
                    format!("The JAR could not be opened: {source}"),
                )],
            );
        }
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(error) => {
            return (
                None,
                vec![ModWarning::new(
                    "jar_malformed",
                    format!("This file is not a readable ZIP/JAR archive: {error}"),
                )],
            );
        }
    };
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return (
            None,
            vec![ModWarning::new(
                "jar_entry_limit",
                "Metadata inspection was skipped because the archive contains more than 4,096 entries.",
            )],
        );
    }
    let mut primary = None;
    let mut legacy = None;
    for index in 0..archive.len() {
        match archive.by_index(index) {
            Ok(entry) => match entry.name() {
                "META-INF/neoforge.mods.toml" => primary = Some(index),
                "META-INF/mods.toml" => legacy = Some(index),
                _ => {}
            },
            Err(error) => {
                return (
                    None,
                    vec![ModWarning::new(
                        "jar_malformed",
                        format!("The JAR directory could not be inspected: {error}"),
                    )],
                );
            }
        }
    }
    let metadata_index = match (primary, legacy) {
        (Some(index), _) => index,
        (None, Some(index)) => index,
        (None, None) => {
            return (
                None,
                vec![ModWarning::new(
                    "neoforge_metadata_missing",
                    "No META-INF/neoforge.mods.toml metadata was found.",
                )],
            );
        }
    };
    let mut entry = match archive.by_index(metadata_index) {
        Ok(entry) => entry,
        Err(error) => {
            return (
                None,
                vec![ModWarning::new("jar_malformed", error.to_string())],
            );
        }
    };
    if entry.size() > MAX_METADATA_BYTES {
        return (
            None,
            vec![ModWarning::new(
                "neoforge_metadata_too_large",
                "neoforge.mods.toml exceeds the 256 KiB safety bound.",
            )],
        );
    }
    let mut bytes = Vec::with_capacity(entry.size().min(MAX_METADATA_BYTES as u64) as usize);
    if let Err(error) = entry
        .by_ref()
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
    {
        return (
            None,
            vec![ModWarning::new(
                "neoforge_metadata_unreadable",
                format!("neoforge.mods.toml could not be decompressed: {error}"),
            )],
        );
    }
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return (
            None,
            vec![ModWarning::new(
                "neoforge_metadata_too_large",
                "neoforge.mods.toml exceeded the 256 KiB read limit.",
            )],
        );
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return (
                None,
                vec![ModWarning::new(
                    "neoforge_metadata_malformed",
                    "neoforge.mods.toml is not valid UTF-8.",
                )],
            );
        }
    };
    let document = match crate::neoforge::mods::NeoForgeModsDocument::parse(&text) {
        Ok(document) => document,
        Err(reason) => {
            return (
                None,
                vec![ModWarning::new(
                    "neoforge_metadata_malformed",
                    format!("neoforge.mods.toml is malformed: {reason}"),
                )],
            );
        }
    };
    let Some(first) = document.mods.first() else {
        return (
            None,
            vec![ModWarning::new(
                "neoforge_metadata_malformed",
                "neoforge.mods.toml declares no [[mods]] entry.",
            )],
        );
    };
    if document.mods.len() > 1 {
        return (
            None,
            vec![ModWarning::new(
                "neoforge_metadata_malformed",
                "neoforge.mods.toml declares more than one [[mods]] entry.",
            )],
        );
    }
    let id = first.mod_id.clone().unwrap_or_default().trim().to_owned();
    if id.is_empty() {
        return (
            None,
            vec![ModWarning::new(
                "neoforge_metadata_malformed",
                "neoforge.mods.toml does not contain a usable mod id.",
            )],
        );
    }
    let mut depends = Vec::new();
    let mut recommends = Vec::new();
    let mut conflicts = Vec::new();
    for list in document.dependencies.values() {
        for dependency in list {
            if dependency.mod_id.trim().is_empty() {
                continue;
            }
            let relation = ModRelation {
                mod_id: dependency.mod_id.trim().to_owned(),
                requirement: if dependency.version_range.trim().is_empty() {
                    "*".to_owned()
                } else {
                    dependency.version_range.trim().to_owned()
                },
            };
            match dependency.kind.as_str() {
                "optional" => recommends.push(relation),
                "incompatible" => conflicts.push(relation),
                // `required` is the default spelling; discouraged future
                // types degrade conservatively to required.
                _ => depends.push(relation),
            }
        }
    }
    (
        Some(ModMetadata {
            id,
            name: first.display_name.clone().filter(|v| !v.trim().is_empty()),
            version: first.version.clone().filter(|v| !v.trim().is_empty()),
            description: first.description.clone().filter(|v| !v.trim().is_empty()),
            authors: first.authors.clone(),
            environment: None,
            depends,
            recommends,
            suggests: Vec::new(),
            conflicts,
            breaks: Vec::new(),
            has_declared_icon: first.icon_file.is_some(),
            nested_mod_ids: Vec::new(),
            nested_mod_versions: HashMap::new(),
            mixin_java_requirements: Vec::new(),
        }),
        Vec::new(),
    )
}

pub(crate) fn inspect_fabric_metadata(
    path: &Path,
    jar_size: u64,
) -> (Option<ModMetadata>, Vec<ModWarning>) {
    if jar_size > MAX_INSPECTED_JAR_BYTES {
        return (
            None,
            vec![ModWarning::new(
                "jar_too_large",
                "Metadata inspection was skipped because this JAR exceeds the 512 MiB safety bound.",
            )],
        );
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(source) => {
            return (
                None,
                vec![ModWarning::new(
                    "jar_unreadable",
                    format!("The JAR could not be opened: {source}"),
                )],
            );
        }
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(error) => {
            return (
                None,
                vec![ModWarning::new(
                    "jar_malformed",
                    format!("This file is not a readable ZIP/JAR archive: {error}"),
                )],
            );
        }
    };
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return (
            None,
            vec![ModWarning::new(
                "jar_entry_limit",
                "Metadata inspection was skipped because the archive contains more than 4,096 entries.",
            )],
        );
    }
    let mut metadata_indexes = Vec::new();
    for index in 0..archive.len() {
        match archive.by_index(index) {
            Ok(entry) if entry.name() == "fabric.mod.json" => metadata_indexes.push(index),
            Ok(_) => {}
            Err(error) => {
                return (
                    None,
                    vec![ModWarning::new(
                        "jar_malformed",
                        format!("The JAR directory could not be inspected: {error}"),
                    )],
                );
            }
        }
    }
    if metadata_indexes.is_empty() {
        return (
            None,
            vec![ModWarning::new(
                "fabric_metadata_missing",
                "No root-level fabric.mod.json metadata was found.",
            )],
        );
    }
    if metadata_indexes.len() != 1 {
        return (
            None,
            vec![ModWarning::new(
                "fabric_metadata_duplicate",
                "The JAR contains more than one fabric.mod.json entry.",
            )],
        );
    }
    let mut entry = match archive.by_index(metadata_indexes[0]) {
        Ok(entry) => entry,
        Err(error) => {
            return (
                None,
                vec![ModWarning::new("jar_malformed", error.to_string())],
            );
        }
    };
    if entry.size() > MAX_METADATA_BYTES {
        return (
            None,
            vec![ModWarning::new(
                "fabric_metadata_too_large",
                "fabric.mod.json exceeds the 256 KiB uncompressed safety bound.",
            )],
        );
    }
    let mut bytes = Vec::with_capacity(entry.size().min(MAX_METADATA_BYTES) as usize);
    if let Err(error) = entry
        .by_ref()
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
    {
        return (
            None,
            vec![ModWarning::new(
                "fabric_metadata_unreadable",
                format!("fabric.mod.json could not be decompressed: {error}"),
            )],
        );
    }
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return (
            None,
            vec![ModWarning::new(
                "fabric_metadata_too_large",
                "fabric.mod.json exceeded the 256 KiB read limit.",
            )],
        );
    }
    let document: FabricMetadataDocument = match serde_json::from_slice(&bytes) {
        Ok(document) => document,
        Err(error) => {
            return (
                None,
                vec![ModWarning::new(
                    "fabric_metadata_malformed",
                    format!("fabric.mod.json is malformed: {error}"),
                )],
            );
        }
    };
    let valid_relations = valid_relation_documents(&document);
    let Some(id) = document
        .id
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
    else {
        return (
            None,
            vec![ModWarning::new(
                "fabric_metadata_malformed",
                "fabric.mod.json does not contain a usable mod id.",
            )],
        );
    };
    drop(entry);
    let (mut mixin_java_requirements, mut mixin_warnings) =
        inspect_mixins(&mut archive, &id, &document.mixins);
    let mut nested_mod_ids = Vec::new();
    let mut nested_mod_versions = HashMap::new();
    let mut known_ids = HashSet::from([id.to_lowercase()]);
    let mut nested_budget = MAX_NESTED_TOTAL_BYTES;
    let mut nested_count = 0;
    let mut nested_warnings = if valid_relations {
        Vec::new()
    } else {
        vec![ModWarning::new(
            "fabric_metadata_malformed",
            "Invalid Fabric dependency relationship shape.",
        )]
    };
    inspect_declared_nested_jars(
        &mut archive,
        &document.jars,
        0,
        &mut nested_budget,
        &mut nested_count,
        &mut known_ids,
        &mut nested_mod_ids,
        &mut nested_mod_versions,
        &mut nested_warnings,
        &mut mixin_java_requirements,
    );
    nested_warnings.append(&mut mixin_warnings);
    let authors = document
        .authors
        .into_iter()
        .filter_map(|author| match author {
            serde_json::Value::String(name) => Some(name),
            serde_json::Value::Object(fields) => fields
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            _ => None,
        })
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .take(16)
        .collect();
    (
        Some(ModMetadata {
            id,
            name: clean_optional(document.name),
            version: clean_optional(document.version),
            description: clean_optional(document.description).map(|value| {
                if value.chars().count() > 2_000 {
                    value.chars().take(2_000).collect()
                } else {
                    value
                }
            }),
            authors,
            environment: clean_optional(document.environment),
            depends: relations(document.depends),
            recommends: relations(document.recommends),
            suggests: relations(document.suggests),
            conflicts: relations(document.conflicts),
            breaks: relations(document.breaks),
            has_declared_icon: document.icon.is_some(),
            nested_mod_ids,
            nested_mod_versions,
            mixin_java_requirements,
        }),
        nested_warnings,
    )
}

fn valid_nested_path(path: &str) -> bool {
    path.ends_with(".jar")
        && !path.starts_with('/')
        && !path.contains(['\\', ':'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

#[allow(clippy::too_many_arguments)]
fn inspect_declared_nested_jars<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    declarations: &[NestedJarDeclaration],
    depth: usize,
    budget: &mut u64,
    count: &mut usize,
    known_ids: &mut HashSet<String>,
    found_ids: &mut Vec<String>,
    found_versions: &mut HashMap<String, Option<String>>,
    warnings: &mut Vec<ModWarning>,
    mixin_requirements: &mut Vec<MixinJavaRequirement>,
) {
    let mut declared = HashSet::new();
    for declaration in declarations {
        let path = &declaration.file;
        if !valid_nested_path(path) || !declared.insert(path) {
            warnings.push(ModWarning::new(
                "nested_jar_invalid",
                "A declared nested JAR path is invalid or duplicated.",
            ));
            continue;
        }
        *count += 1;
        if depth >= MAX_NESTED_DEPTH || *count > MAX_NESTED_COUNT {
            warnings.push(ModWarning::new(
                "nested_jar_limit",
                "Nested JAR inspection exceeded its depth or count bound.",
            ));
            return;
        }
        let indexes: Vec<_> = (0..archive.len())
            .filter(|&index| {
                archive
                    .by_index(index)
                    .is_ok_and(|entry| entry.name() == path)
            })
            .collect();
        if indexes.len() != 1 {
            warnings.push(ModWarning::new(
                "nested_jar_missing",
                "A declared nested JAR is missing or duplicated.",
            ));
            continue;
        }
        let mut nested_file = match archive.by_index(indexes[0]) {
            Ok(file) => file,
            Err(_) => {
                warnings.push(ModWarning::new(
                    "nested_jar_malformed",
                    "A declared nested JAR cannot be read.",
                ));
                continue;
            }
        };
        if nested_file.size() > *budget {
            warnings.push(ModWarning::new(
                "nested_jar_limit",
                "Nested JAR bytes exceed the 64 MiB inspection bound.",
            ));
            continue;
        }
        let mut bytes = Vec::new();
        if nested_file
            .by_ref()
            .take(*budget + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() as u64 > *budget
        {
            warnings.push(ModWarning::new(
                "nested_jar_malformed",
                "A declared nested JAR cannot be decompressed within the inspection bound.",
            ));
            continue;
        }
        *budget -= bytes.len() as u64;
        drop(nested_file);
        let mut nested = match zip::ZipArchive::new(Cursor::new(bytes)) {
            Ok(nested) if nested.len() <= MAX_ARCHIVE_ENTRIES => nested,
            _ => {
                warnings.push(ModWarning::new(
                    "nested_jar_malformed",
                    "A declared nested JAR is malformed or exceeds the entry limit.",
                ));
                continue;
            }
        };
        let metadata_indexes: Vec<_> = (0..nested.len())
            .filter(|&index| {
                nested
                    .by_index(index)
                    .is_ok_and(|entry| entry.name() == "fabric.mod.json")
            })
            .collect();
        if metadata_indexes.len() != 1 {
            warnings.push(ModWarning::new(
                "nested_metadata_malformed",
                "A nested JAR lacks one valid root fabric.mod.json.",
            ));
            continue;
        }
        let mut metadata_file = match nested.by_index(metadata_indexes[0]) {
            Ok(file) if file.size() <= MAX_METADATA_BYTES => file,
            _ => {
                warnings.push(ModWarning::new(
                    "nested_metadata_malformed",
                    "Nested Fabric metadata exceeds its inspection bound.",
                ));
                continue;
            }
        };
        let mut metadata_bytes = Vec::new();
        if metadata_file
            .by_ref()
            .take(MAX_METADATA_BYTES + 1)
            .read_to_end(&mut metadata_bytes)
            .is_err()
            || metadata_bytes.len() as u64 > MAX_METADATA_BYTES
        {
            warnings.push(ModWarning::new(
                "nested_metadata_malformed",
                "Nested Fabric metadata is unreadable.",
            ));
            continue;
        }
        drop(metadata_file);
        let document: FabricMetadataDocument = match serde_json::from_slice(&metadata_bytes) {
            Ok(document) => document,
            Err(_) => {
                warnings.push(ModWarning::new(
                    "nested_metadata_malformed",
                    "Nested Fabric metadata is malformed.",
                ));
                continue;
            }
        };
        if !valid_relation_documents(&document) {
            warnings.push(ModWarning::new(
                "nested_metadata_malformed",
                "Invalid nested Fabric relationship shape.",
            ));
        }
        let Some(id) = document.id.filter(|id| !id.trim().is_empty()) else {
            warnings.push(ModWarning::new(
                "nested_metadata_malformed",
                "Nested Fabric metadata lacks a mod ID.",
            ));
            continue;
        };
        let normalized_id = id.to_lowercase();
        if document.environment.as_deref() != Some("server") {
            let (mut requirements, mut problems) =
                inspect_mixins(&mut nested, &id, &document.mixins);
            mixin_requirements.append(&mut requirements);
            warnings.append(&mut problems);
        }
        if !known_ids.insert(normalized_id.clone()) {
            warnings.push(ModWarning::new(
                "duplicate_mod_id",
                "Nested Fabric metadata declares a duplicate mod ID.",
            ));
        } else {
            found_ids.push(id);
            found_versions.insert(normalized_id, clean_optional(document.version));
        }
        inspect_declared_nested_jars(
            &mut nested,
            &document.jars,
            depth + 1,
            budget,
            count,
            known_ids,
            found_ids,
            found_versions,
            warnings,
            mixin_requirements,
        );
    }
}

/// Reads only declared client/common JSON resources. No extraction, plugin
/// loading, class loading, arbitrary JSON scan, or external path follows.
// Mixin uses Gson's comment-tolerant reader. Remove comments lexically,
// preserving quoted strings and line positions; never inspect or execute code.
fn mixin_json(bytes: &[u8]) -> Result<serde_json::Value, &'static str> {
    let mut normalized = bytes.to_vec();
    let mut i = 0;
    let mut string = false;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                string = false;
            }
            i += 1;
            continue;
        }
        if b == b'"' {
            string = true;
            i += 1;
            continue;
        }
        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                normalized[i] = b' ';
                i += 1;
            }
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            normalized[i] = b' ';
            normalized[i + 1] = b' ';
            i += 2;
            let mut closed = false;
            while i < bytes.len() {
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    normalized[i] = b' ';
                    normalized[i + 1] = b' ';
                    i += 2;
                    closed = true;
                    break;
                }
                if bytes[i] != b'\n' && bytes[i] != b'\r' {
                    normalized[i] = b' ';
                }
                i += 1;
            }
            if !closed {
                return Err("Declared Mixin comment is unterminated.");
            }
        } else {
            i += 1;
        }
    }
    serde_json::from_slice(&normalized).map_err(|_| "Declared Mixin JSON is malformed.")
}

fn inspect_mixins<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    mod_id: &str,
    declarations: &[serde_json::Value],
) -> (Vec<MixinJavaRequirement>, Vec<ModWarning>) {
    let mut requirements = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    if declarations.len() > 64 {
        return (
            requirements,
            vec![ModWarning::new(
                "mixin_metadata_invalid",
                "Too many declared Mixin configurations (limit 64).",
            )],
        );
    }
    for declaration in declarations {
        let config = match declaration {
            serde_json::Value::String(config) => Some(config.as_str()),
            serde_json::Value::Object(fields) => {
                if fields
                    .get("environment")
                    .and_then(serde_json::Value::as_str)
                    == Some("server")
                {
                    continue;
                }
                if fields
                    .get("environment")
                    .is_some_and(|v| !matches!(v.as_str(), Some("client" | "*")))
                {
                    warnings.push(ModWarning::new(
                        "mixin_metadata_invalid",
                        "Unknown declared Mixin environment.",
                    ));
                    continue;
                }
                fields.get("config").and_then(serde_json::Value::as_str)
            }
            _ => None,
        };
        let Some(config) = config.filter(|path| {
            path.len() <= 256
                && path.ends_with(".json")
                && !path.starts_with('/')
                && !path.contains(['\\', ':'])
                && path
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != "..")
        }) else {
            warnings.push(ModWarning::new(
                "mixin_metadata_invalid",
                "An unsafe or malformed declared Mixin resource was rejected.",
            ));
            continue;
        };
        if !seen.insert(config) {
            continue;
        }
        let result = (|| {
            let indexes: Vec<_> = (0..archive.len())
                .filter(|&i| archive.by_index(i).is_ok_and(|e| e.name() == config))
                .collect();
            if indexes.len() != 1 {
                return Err("Declared Mixin resource is missing or duplicated.");
            }
            let mut entry = archive
                .by_index(indexes[0])
                .map_err(|_| "Declared Mixin resource cannot be read.")?;
            if entry.size() > MAX_METADATA_BYTES {
                return Err("Declared Mixin resource exceeds the 256 KiB bound.");
            }
            let mut bytes = Vec::new();
            entry
                .by_ref()
                .take(MAX_METADATA_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Declared Mixin resource cannot be decompressed.")?;
            if bytes.len() as u64 > MAX_METADATA_BYTES {
                return Err("Declared Mixin resource exceeds its read bound.");
            }
            let value = mixin_json(&bytes)?;
            let fields = value
                .as_object()
                .ok_or("Declared Mixin JSON must be an object.")?;
            if let Some(level) = fields.get("compatibilityLevel") {
                let level = level
                    .as_str()
                    .ok_or("Mixin compatibilityLevel must be a string.")?;
                let major = level
                    .strip_prefix("JAVA_")
                    .and_then(|n| n.parse::<u32>().ok())
                    .filter(|n| (6..=99).contains(n))
                    .ok_or("Unknown Mixin compatibility level.")?;
                requirements.push(MixinJavaRequirement {
                    mod_id: mod_id.into(),
                    config: config.into(),
                    java_major: major,
                });
            }
            Ok(())
        })();
        if let Err(reason) = result {
            warnings.push(ModWarning::new(
                "mixin_metadata_invalid",
                format!("{mod_id}: {config}: {reason}"),
            ));
        }
    }
    (requirements, warnings)
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn valid_relation_documents(document: &FabricMetadataDocument) -> bool {
    [
        &document.depends,
        &document.recommends,
        &document.suggests,
        &document.conflicts,
        &document.breaks,
    ]
    .into_iter()
    .all(|fields| {
        fields.iter().all(|(id, value)| {
            !id.is_empty()
                && match value {
                    serde_json::Value::String(v) => !v.is_empty(),
                    serde_json::Value::Array(values) => {
                        !values.is_empty()
                            && values
                                .iter()
                                .all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                    }
                    _ => false,
                }
        })
    })
}

fn relations(fields: serde_json::Map<String, serde_json::Value>) -> Vec<ModRelation> {
    let mut relations: Vec<_> = fields
        .into_iter()
        .filter_map(|(mod_id, value)| {
            let mod_id = mod_id.trim().to_owned();
            if mod_id.is_empty() {
                return None;
            }
            let requirement = match value {
                serde_json::Value::String(value) => value,
                serde_json::Value::Array(values) => values
                    .into_iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
                    .join(" or "),
                value => value.to_string(),
            };
            Some(ModRelation {
                mod_id,
                requirement,
            })
        })
        .collect();
    relations.sort_by(|left, right| left.mod_id.cmp(&right.mod_id));
    relations
}

fn derive_local_warnings(entries: &mut [ModEntry], platform_kind: &str) {
    let module_versions: Vec<_> = entries
        .iter()
        .filter(|e| e.enabled)
        .filter_map(|e| e.metadata.clone())
        .collect();
    let enabled_ids: HashSet<String> = entries
        .iter()
        .filter(|entry| entry.enabled)
        .filter_map(|entry| entry.metadata.as_ref())
        .flat_map(|metadata| std::iter::once(&metadata.id).chain(metadata.nested_mod_ids.iter()))
        .map(|id| id.to_lowercase())
        .collect();
    let mut declarations = HashMap::<String, (usize, usize)>::new();
    for metadata in entries.iter().filter_map(|entry| entry.metadata.as_ref()) {
        let root = declarations.entry(metadata.id.to_lowercase()).or_default();
        root.0 += 1;
        for id in &metadata.nested_mod_ids {
            declarations.entry(id.to_lowercase()).or_default().1 += 1;
        }
    }
    // The builtin relation ids are loader-family dependent: NeoForge mods
    // declare `neoforge` (and historically `forge`), Fabric mods declare
    // `fabricloader`.
    let builtins: Vec<&str> = if platform_kind == "neoForge" {
        vec!["minecraft", "neoforge", "forge", "java"]
    } else {
        vec!["minecraft", "fabricloader", "java"]
    };
    for entry in entries.iter_mut() {
        let Some(metadata) = &entry.metadata else {
            continue;
        };
        if let Some(conflicting_id) = std::iter::once(&metadata.id)
            .chain(metadata.nested_mod_ids.iter())
            .find(|id| {
                declarations
                    .get(&id.to_lowercase())
                    .is_some_and(|(roots, nested)| {
                        *roots > 1
                            || (*roots > 0
                                && *nested > 0
                                && module_versions
                                    .iter()
                                    .filter_map(|m| crate::mod_compatibility::version_for(m, id))
                                    .collect::<HashSet<_>>()
                                    .len()
                                    != 1)
                    })
            })
        {
            entry.warnings.push(ModWarning::new(
                "duplicate_mod_id",
                format!(
                    "Local artifacts declare conflicting copies of the mod id '{}'.",
                    conflicting_id
                ),
            ));
        }
        if !entry.enabled {
            continue;
        }
        for dependency in &metadata.depends {
            let dependency_id = dependency.mod_id.to_lowercase();
            if builtins.contains(&dependency_id.as_str()) || enabled_ids.contains(&dependency_id) {
                continue;
            }
            entry.warnings.push(ModWarning::new(
                "required_dependency_missing",
                format!(
                    "Required dependency '{}' was not detected among enabled local mods.",
                    dependency.mod_id
                ),
            ));
        }
        for relation in metadata.conflicts.iter().chain(metadata.breaks.iter()) {
            if enabled_ids.contains(&relation.mod_id.to_lowercase()) {
                entry.warnings.push(ModWarning::new(
                    "declared_conflict_present",
                    format!(
                        "Metadata declares a conflict with '{}' matching '{}'. That mod is detected; Fabric evaluates the version constraint.",
                        relation.mod_id, relation.requirement
                    ),
                ));
            }
        }
        for relation in &metadata.recommends {
            if !enabled_ids.contains(&relation.mod_id) {
                entry.warnings.push(ModWarning::new(
                    "recommended_dependency_missing",
                    format!(
                        "{} {} is recommended, but optional.",
                        relation.mod_id, relation.requirement
                    ),
                ));
            }
        }
    }
}

/// Atomically renames one current user-owned JAR between `.jar` and
/// `.jar.disabled`, then returns a fresh authoritative inventory.
pub fn set_enabled(
    managed: &ManagedPaths,
    instance: &InstanceId,
    entry_id: &str,
    enabled: bool,
) -> Result<ModInventory, ModError> {
    set_enabled_with_commit(managed, instance, entry_id, enabled, |state| {
        state.save(managed, instance)
    })
}

fn set_enabled_with_commit(
    managed: &ManagedPaths,
    instance: &InstanceId,
    entry_id: &str,
    enabled: bool,
    commit: impl FnOnce(&DisabledState) -> Result<(), ModError>,
) -> Result<ModInventory, ModError> {
    with_mutation_lock(instance, || {
        let inventory = scan(managed, instance)?;
        let entry = resolve_mutable_entry(&inventory, entry_id, true)?;
        if entry.enabled == enabled {
            return Ok(inventory);
        }
        if !enabled {
            dependency_blockers(&inventory, entry, &platform_kind_of(managed, instance))?;
        } else {
            let mut proposed = inventory.entries.clone();
            proposed
                .iter_mut()
                .find(|candidate| candidate.entry_id == entry.entry_id)
                .expect("resolved entry")
                .enabled = true;
            derive_local_warnings(&mut proposed, &platform_kind_of(managed, instance));
            let candidate = proposed
                .iter()
                .find(|candidate| candidate.entry_id == entry.entry_id)
                .expect("resolved entry");
            if let Some(warning) = candidate
                .warnings
                .iter()
                .find(|warning| warning.code == "required_dependency_missing")
            {
                return Err(ModError::DependencyBlocked(warning.message.clone()));
            }
            if let Some(provider) = &candidate.provenance {
                for required in &provider.requires {
                    if !proposed.iter().any(|entry| {
                        entry.enabled
                            && entry
                                .provenance
                                .as_ref()
                                .is_some_and(|dependency| dependency.identity() == *required)
                    }) {
                        return Err(ModError::DependencyBlocked("A required provider dependency is disabled or missing. Re-enable or install it first.".into()));
                    }
                }
            }
        }
        let mods = validate_mods_directory(managed, instance)?;
        let source = validate_current_regular_file(&mods, &entry.file_name)?;
        let target_name = if enabled {
            entry
                .file_name
                .strip_suffix(DISABLED_SUFFIX)
                .ok_or_else(|| ModError::State("the disabled filename is malformed".to_owned()))?
                .to_owned()
        } else {
            format!("{}{DISABLED_SUFFIX}", entry.file_name)
        };
        validate_file_name(&target_name)?;
        let target = mods.join(&target_name);
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(ModError::TargetConflict(target_name));
        }
        if let Some(hash) = &entry.sha256 {
            verify_file(
                &source,
                &ArtifactDigest::parse(hash).map_err(|e| ModError::State(e.to_string()))?,
                entry.size_bytes,
            )
            .map_err(|_| ModError::StaleEntry)?;
        }
        let mut disabled = DisabledState::load(managed, instance)?;
        let previous_disabled = disabled.clone();
        if enabled {
            disabled.files.remove(&entry.file_name);
        } else {
            disabled.files.insert(
                target_name.clone(),
                file_digest(&source).map_err(|source| ModError::MutationIo { source })?,
            );
        }
        // No-clobber activation, including an external collision after the
        // preflight. The two names briefly refer to the same instance bytes.
        std::fs::hard_link(&source, &target).map_err(|source| ModError::MutationIo { source })?;
        if let Err(source_error) = std::fs::remove_file(&source) {
            let _ = std::fs::remove_file(&target);
            return Err(ModError::MutationIo {
                source: source_error,
            });
        }
        let result = commit(&disabled).and_then(|()| scan(managed, instance));
        match result {
            Ok(result) => Ok(result),
            Err(error) => {
                std::fs::hard_link(&target, &source)
                    .map_err(|source| ModError::MutationIo { source })?;
                std::fs::remove_file(&target).map_err(|source| ModError::MutationIo { source })?;
                previous_disabled.save(managed, instance)?;
                Err(error)
            }
        }
    })
}

/// Exact-byte receipts for launcher-disabled mods. Provider and bootstrap
/// records remain unchanged; these receipts make local activation fail closed
/// when disabled bytes have changed between launcher restarts.
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DisabledState {
    schema_version: u32,
    files: HashMap<String, String>,
}
impl DisabledState {
    fn load(managed: &ManagedPaths, instance: &InstanceId) -> Result<Self, ModError> {
        let path = managed
            .instance_paths(instance)
            .root()
            .join("mods-disabled.json");
        let meta = match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    schema_version: 1,
                    files: HashMap::new(),
                });
            }
            Err(source) => return Err(ModError::MutationIo { source }),
            Ok(meta) => meta,
        };
        if !meta.is_file() || meta.file_type().is_symlink() || is_reparse_point(&meta) {
            return Err(ModError::UnsafeEntry);
        }
        let bytes = std::fs::read(path).map_err(|source| ModError::MutationIo { source })?;
        let state: Self =
            serde_json::from_slice(&bytes).map_err(|e| ModError::State(e.to_string()))?;
        if state.schema_version != 1 {
            return Err(ModError::State(
                "Unsupported disabled-content schema.".into(),
            ));
        }
        for (name, hash) in &state.files {
            validate_file_name(name)?;
            if !is_disabled_jar(name) {
                return Err(ModError::State("Invalid disabled-content name.".into()));
            }
            ArtifactDigest::parse(hash).map_err(|e| ModError::State(e.to_string()))?;
        }
        Ok(state)
    }
    fn save(&self, managed: &ManagedPaths, instance: &InstanceId) -> Result<(), ModError> {
        Self::load(managed, instance)?;
        let path = managed
            .instance_paths(instance)
            .root()
            .join("mods-disabled.json");
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| ModError::State(e.to_string()))?;
        if let Err(source) =
            std::fs::write(&temporary, bytes).and_then(|()| std::fs::rename(&temporary, &path))
        {
            let _ = std::fs::remove_file(&temporary);
            return Err(ModError::MutationIo { source });
        }
        Ok(())
    }
}

/// Permanently removes one exact current user-owned local JAR, then returns
/// a fresh authoritative inventory. UI confirmation is required by product
/// policy; this backend still revalidates identity and ownership itself.
pub fn remove(
    managed: &ManagedPaths,
    instance: &InstanceId,
    entry_id: &str,
) -> Result<ModInventory, ModError> {
    with_mutation_lock(instance, || {
        let inventory = scan(managed, instance)?;
        let entry = resolve_mutable_entry(&inventory, entry_id, false)?;
        dependency_blockers(&inventory, entry, &platform_kind_of(managed, instance))?;
        let mods = validate_mods_directory(managed, instance)?;
        let target = validate_current_regular_file(&mods, &entry.file_name)?;
        if let Some(hash) = &entry.sha256 {
            verify_file(
                &target,
                &ArtifactDigest::parse(hash).map_err(|error| ModError::State(error.to_string()))?,
                entry.size_bytes,
            )
            .map_err(|_| ModError::StaleEntry)?;
        }
        std::fs::remove_file(target).map_err(|source| ModError::MutationIo { source })?;
        scan(managed, instance)
    })
}

fn resolve_mutable_entry<'a>(
    inventory: &'a ModInventory,
    entry_id: &str,
    toggle: bool,
) -> Result<&'a ModEntry, ModError> {
    let entry = inventory
        .entries
        .iter()
        .find(|entry| entry.entry_id == entry_id)
        .ok_or(ModError::StaleEntry)?;
    if entry.ownership == ModOwnership::LauncherManagedRequired {
        return Err(ModError::RequiredArtifact);
    }
    if (toggle && !entry.can_toggle) || (!toggle && !entry.can_remove) {
        return Err(ModError::UnsafeEntry);
    }
    Ok(entry)
}

fn dependency_blockers(
    inventory: &ModInventory,
    target: &ModEntry,
    loader_family: &str,
) -> Result<(), ModError> {
    if !target.enabled {
        return Ok(());
    }
    let ids: HashSet<&str> = target
        .metadata
        .iter()
        .flat_map(|metadata| {
            std::iter::once(metadata.id.as_str())
                .chain(metadata.nested_mod_ids.iter().map(String::as_str))
        })
        .collect();
    let mut blockers = Vec::new();
    for entry in inventory
        .entries
        .iter()
        .filter(|entry| entry.enabled && entry.entry_id != target.entry_id)
    {
        if let Some(meta) = &entry.metadata {
            for relation in &meta.depends {
                if ids.contains(relation.mod_id.as_str())
                    && !inventory
                        .entries
                        .iter()
                        .filter(|alternative| {
                            alternative.enabled && alternative.entry_id != target.entry_id
                        })
                        .filter_map(|alternative| alternative.metadata.as_ref())
                        .any(|alternative| {
                            crate::mod_compatibility::version_for(alternative, &relation.mod_id)
                                .is_some_and(|version| {
                                    crate::mod_compatibility::satisfies_for_family(
                                        version,
                                        &relation.requirement,
                                        loader_family,
                                    ) == Ok(true)
                                })
                        })
                {
                    blockers.push(format!(
                        "{} requires {}",
                        entry.display_name, relation.mod_id
                    ));
                }
            }
        }
        if let (Some(parent), Some(dependency)) = (&entry.provenance, &target.provenance)
            && parent.requires.contains(&dependency.identity())
        {
            blockers.push(format!(
                "{} requires this provider artifact",
                entry.display_name
            ));
        }
    }
    if blockers.is_empty() {
        Ok(())
    } else {
        Err(ModError::DependencyBlocked(blockers.join("; ")))
    }
}

pub(crate) fn validate_provider_removals(
    managed: &ManagedPaths,
    instance: &InstanceId,
    removed: &[ProviderRecord],
) -> Result<(), ModError> {
    if !removed
        .iter()
        .any(|record| record.content_type == ContentType::Mod)
    {
        return Ok(());
    }
    let mut inventory = scan(managed, instance)?;
    let targets: Vec<_> = inventory
        .entries
        .iter()
        .filter(|entry| {
            entry.provenance.as_ref().is_some_and(|record| {
                removed
                    .iter()
                    .any(|removed| removed.identity() == record.identity())
            })
        })
        .cloned()
        .collect();
    inventory.entries.retain(|entry| {
        !targets
            .iter()
            .any(|target| target.entry_id == entry.entry_id)
    });
    for target in targets {
        dependency_blockers(&inventory, &target, &platform_kind_of(managed, instance))?;
    }
    Ok(())
}

pub(crate) fn validate_current_regular_file(
    mods: &Path,
    file_name: &str,
) -> Result<PathBuf, ModError> {
    validate_file_name(file_name)?;
    let target = mods.join(file_name);
    let metadata = std::fs::symlink_metadata(&target).map_err(|_| ModError::StaleEntry)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(ModError::UnsafeEntry);
    }
    let canonical_target =
        std::fs::canonicalize(&target).map_err(|source| ModError::Boundary { source })?;
    let canonical_mods =
        std::fs::canonicalize(mods).map_err(|source| ModError::Boundary { source })?;
    if canonical_target.parent() != Some(canonical_mods.as_path()) {
        return Err(ModError::BoundaryEscape);
    }
    Ok(target)
}

fn validate_file_name(file_name: &str) -> Result<(), ModError> {
    let path = Path::new(file_name);
    if file_name.is_empty()
        || file_name == "."
        || file_name == ".."
        || path.is_absolute()
        || path.components().count() != 1
        || file_name.contains(['/', '\\', ':'])
    {
        return Err(ModError::BoundaryEscape);
    }
    Ok(())
}

fn with_mutation_lock<T>(
    instance: &InstanceId,
    operation: impl FnOnce() -> Result<T, ModError>,
) -> Result<T, ModError> {
    crate::instance_content::with_instance_lock(instance, || Ok(operation()))
        .map_err(|_| ModError::MutationInProgress)?
}

fn is_enabled_jar(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".jar")
}

fn is_disabled_jar(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".jar.disabled")
}

fn opaque_id(material: &[u8]) -> String {
    let digest: [u8; 32] = sha2::Sha256::digest(material).into();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn file_digest(path: &Path) -> Result<String, std::io::Error> {
    let mut input = std::fs::File::open(path)?;
    let mut hash = sha2::Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let bytes = input.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        hash.update(&buffer[..bytes]);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[derive(Debug)]
pub enum ModError {
    DependencyBlocked(String),
    DirectoryMissing,
    DirectoryRead { source: std::io::Error },
    Boundary { source: std::io::Error },
    BoundaryEscape,
    InstalledState(String),
    ContentState(String),
    StaleEntry,
    RequiredArtifact,
    UnsafeEntry,
    TargetConflict(String),
    MutationInProgress,
    MutationIo { source: std::io::Error },
    State(String),
}

impl ModError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DependencyBlocked(_) => "mod_dependency_blocked",
            Self::DirectoryMissing | Self::DirectoryRead { .. } | Self::Boundary { .. } => {
                "mod_inventory_unavailable"
            }
            Self::BoundaryEscape | Self::UnsafeEntry => "mod_entry_unsafe",
            Self::InstalledState(_) => "aurora_installation_invalid",
            Self::ContentState(_) => "content_state_malformed",
            Self::StaleEntry => "mod_entry_stale",
            Self::RequiredArtifact => "mod_required_artifact",
            Self::TargetConflict(_) => "mod_target_conflict",
            Self::MutationInProgress => "mod_mutation_in_progress",
            Self::MutationIo { .. } | Self::State(_) => "mod_mutation_failure",
        }
    }
}

impl fmt::Display for ModError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DependencyBlocked(reason) => {
                write!(formatter, "Cannot change this mod: {reason}.")
            }
            Self::DirectoryMissing => {
                write!(formatter, "the instance mods directory does not exist")
            }
            Self::DirectoryRead { source } => write!(
                formatter,
                "the instance mods directory could not be read: {source}"
            ),
            Self::Boundary { source } => write!(
                formatter,
                "the managed mods boundary could not be resolved: {source}"
            ),
            Self::BoundaryEscape => write!(
                formatter,
                "the resolved mod entry is outside the managed mods directory"
            ),
            Self::InstalledState(reason) => write!(
                formatter,
                "Aurora ownership could not be established from installed state: {reason}"
            ),
            Self::ContentState(reason) => {
                write!(formatter, "content state could not be used: {reason}")
            }
            Self::StaleEntry => write!(
                formatter,
                "this mod changed since the inventory was loaded; refresh and try again"
            ),
            Self::RequiredArtifact => write!(
                formatter,
                "a required launcher-managed mod cannot be disabled or removed from Mods"
            ),
            Self::UnsafeEntry => write!(
                formatter,
                "this entry is not a regular user-managed mod file"
            ),
            Self::TargetConflict(name) => write!(
                formatter,
                "the target filename '{name}' already exists; nothing was overwritten"
            ),
            Self::MutationInProgress => write!(
                formatter,
                "another mod change is already in progress for this instance"
            ),
            Self::MutationIo { source } => {
                write!(formatter, "the local mod change failed: {source}")
            }
            Self::State(reason) => write!(formatter, "the local mod state is invalid: {reason}"),
        }
    }
}

impl std::error::Error for ModError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neoforge_mod_metadata_normalizes_into_the_shared_model() {
        let directory =
            std::env::temp_dir().join(format!("aurora-neoforge-mods-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let jar = directory.join("neoforge-mod.jar");
        {
            use std::io::Write as _;
            let file = std::fs::File::create(&jar).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            writer
                .start_file(
                    "META-INF/neoforge.mods.toml",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer
                .write_all(
                    br#"modLoader="javafml"
loaderVersion="[3,]"

[[mods]]
modId="mymod"
version="1.2.3"
displayName="My Mod"

[[dependencies.mymod]]
modId="neoforge"
type="required"
versionRange="[26.2,)"

[[dependencies.mymod]]
modId="decorative"
type="optional"
versionRange="*"

[[dependencies.mymod]]
modId="rival"
type="incompatible"
versionRange="[1,2)"
"#,
                )
                .unwrap();
            writer.finish().unwrap();
        }
        let size = std::fs::metadata(&jar).unwrap().len();
        let (metadata, warnings) = inspect_neoforge_metadata(&jar, size);
        assert!(warnings.is_empty(), "{warnings:?}");
        let metadata = metadata.expect("metadata parses");
        assert_eq!(metadata.id, "mymod");
        assert_eq!(metadata.version.as_deref(), Some("1.2.3"));
        assert_eq!(metadata.name.as_deref(), Some("My Mod"));
        assert_eq!(metadata.depends.len(), 1);
        assert_eq!(metadata.depends[0].mod_id, "neoforge");
        assert_eq!(metadata.depends[0].requirement, "[26.2,)");
        assert_eq!(metadata.recommends.len(), 1);
        assert_eq!(metadata.recommends[0].mod_id, "decorative");
        assert_eq!(metadata.conflicts.len(), 1);
        assert_eq!(metadata.conflicts[0].requirement, "[1,2)");

        let (fabric_shaped, _) = inspect_mod_metadata(&jar, 0, "fabric");
        assert!(fabric_shaped.is_none());
        let (neoforge_shaped, _) = inspect_mod_metadata(&jar, size, "neoForge");
        assert!(neoforge_shaped.is_some());
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn toggle_receipt_failure_rolls_back_exact_bytes_and_filename() {
        let fixture = Fixture::new("toggle-rollback");
        jar(
            &fixture.mods().join("rollback.jar"),
            Some(&metadata("rollback", "Rollback", "{}")),
        );
        let before = std::fs::read(fixture.mods().join("rollback.jar")).unwrap();
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let entry = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "rollback.jar")
            .unwrap();
        let result = set_enabled_with_commit(
            &fixture.managed,
            &fixture.instance,
            &entry.entry_id,
            false,
            |_| Err(ModError::State("injected receipt write failure".into())),
        );
        assert!(result.is_err());
        assert_eq!(
            std::fs::read(fixture.mods().join("rollback.jar")).unwrap(),
            before
        );
        assert!(!fixture.mods().join("rollback.jar.disabled").exists());
        assert!(
            scan(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .iter()
                .find(|entry| entry.file_name == "rollback.jar")
                .unwrap()
                .enabled
        );
    }
    use std::io::Write as _;

    #[test]
    fn disabling_known_dependency_is_blocked_and_changed_disabled_bytes_never_activate() {
        let fixture = Fixture::new("dependency-toggle");
        jar(
            &fixture.mods().join("api.jar"),
            Some(br#"{"id":"fabric-api","version":"1"}"#),
        );
        jar(
            &fixture.mods().join("parent.jar"),
            Some(br#"{"id":"parent","depends":{"fabric-api":"*"}}"#),
        );
        let before = std::fs::read(fixture.mods().join("api.jar")).unwrap();
        let snapshot = scan(&fixture.managed, &fixture.instance).unwrap();
        let api = snapshot
            .entries
            .iter()
            .find(|entry| {
                entry
                    .metadata
                    .as_ref()
                    .is_some_and(|meta| meta.id == "fabric-api")
            })
            .unwrap();
        assert!(
            api.removal_blocked_reason
                .as_deref()
                .unwrap()
                .contains("parent requires fabric-api")
        );
        assert_eq!(
            set_enabled(&fixture.managed, &fixture.instance, &api.entry_id, false)
                .unwrap_err()
                .code(),
            "mod_dependency_blocked"
        );
        assert_eq!(
            std::fs::read(fixture.mods().join("api.jar")).unwrap(),
            before
        );
        let parent = snapshot
            .entries
            .iter()
            .find(|entry| {
                entry
                    .metadata
                    .as_ref()
                    .is_some_and(|meta| meta.id == "parent")
            })
            .unwrap();
        set_enabled(&fixture.managed, &fixture.instance, &parent.entry_id, false).unwrap();
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let api = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "api.jar")
            .unwrap();
        let disabled =
            set_enabled(&fixture.managed, &fixture.instance, &api.entry_id, false).unwrap();
        let entry = disabled
            .entries
            .iter()
            .find(|entry| entry.file_name == "api.jar.disabled")
            .unwrap();
        std::fs::write(fixture.mods().join("api.jar.disabled"), b"changed").unwrap();
        assert!(set_enabled(&fixture.managed, &fixture.instance, &entry.entry_id, true).is_err());
        assert!(!fixture.mods().join("api.jar").exists());
        std::fs::write(fixture.mods().join("api.jar.disabled"), &before).unwrap();
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let entry = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "api.jar.disabled")
            .unwrap();
        set_enabled(&fixture.managed, &fixture.instance, &entry.entry_id, true).unwrap();
        assert_eq!(
            std::fs::read(fixture.mods().join("api.jar")).unwrap(),
            before
        );
    }

    struct Fixture {
        root: PathBuf,
        managed: ManagedPaths,
        instance: InstanceId,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join("aurora-mods-test")
                .join(format!("{name}-{}", uuid::Uuid::new_v4()));
            let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
            let instance = InstanceId::new(format!("fixture-{name}")).unwrap();
            let paths = managed.instance_paths(&instance);
            std::fs::create_dir_all(paths.mods()).unwrap();
            std::fs::write(
                paths.root().join(aurora::AURORA_INSTALLED_FILE_NAME),
                r#"{
                  "schemaVersion": 1,
                  "auroraVersion": "0.3.0",
                  "channel": "stable",
                  "minecraftVersion": "26.2",
                  "fabricLoaderVersion": "0.19.5",
                  "artifact": {
                    "relativePath": "mods/aurora-0.3.0.jar",
                    "sizeBytes": 4,
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                  },
                  "installationId": "fixture",
                  "installedAtUnixSeconds": 1
                }"#,
            )
            .unwrap();
            Self {
                root,
                managed,
                instance,
            }
        }

        fn mods(&self) -> PathBuf {
            self.managed
                .instance_paths(&self.instance)
                .mods()
                .to_path_buf()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn registered_aurora_requirements_match_release_ownership_and_verified_metadata() {
        use crate::instances::{
            InstanceRecord, InstanceRegistry, InstanceState, PinnedRelease,
            settings::InstanceConfiguration,
        };
        let fixture = Fixture::new("active-requirements");
        let aurora_path = fixture.mods().join("aurora-2.1.5.jar");
        let api_path = fixture.mods().join("fabric-api-0.141.6+1.21.11.jar");
        jar(&aurora_path, Some(br#"{"id":"aurora","version":"2.1.5"}"#));
        jar(
            &api_path,
            Some(br#"{"id":"fabric-api","version":"0.141.6+1.21.11"}"#),
        );
        let mut release: serde_json::Value =
            serde_json::from_str(include_str!("../production/aurora-releases.json")).unwrap();
        for (field, path) in [("artifact", &aurora_path), ("fabricApi", &api_path)] {
            let bytes = std::fs::read(path).unwrap();
            let artifact = if field == "artifact" {
                &mut release["releases"][0][field]
            } else {
                &mut release["releases"][0][field]["artifact"]
            };
            artifact["sha256"] = serde_json::json!(format!("{:x}", sha2::Sha256::digest(&bytes)));
            artifact["sizeBytes"] = serde_json::json!(bytes.len());
        }
        let manifest =
            crate::distribution::ReleaseManifest::from_json(&release.to_string()).unwrap();
        let declaration = &release["releases"][0];
        let state = serde_json::json!({"schemaVersion":1,"auroraVersion":"2.1.5","channel":"stable","minecraftVersion":"1.21.11","fabricLoaderVersion":"0.19.5","installationId":"fixture","installedAtUnixSeconds":1,"artifact":{"relativePath":"mods/aurora-2.1.5.jar","sizeBytes":declaration["artifact"]["sizeBytes"],"sha256":declaration["artifact"]["sha256"]},"fabricApi":{"version":"0.141.6+1.21.11","artifact":{"relativePath":"mods/fabric-api-0.141.6+1.21.11.jar","sizeBytes":declaration["fabricApi"]["artifact"]["sizeBytes"],"sha256":declaration["fabricApi"]["artifact"]["sha256"]}}});
        let state_path = fixture
            .managed
            .instance_paths(&fixture.instance)
            .root()
            .join(aurora::AURORA_INSTALLED_FILE_NAME);
        std::fs::write(&state_path, state.to_string()).unwrap();
        let mut registry = InstanceRegistry::empty();
        registry.instances_mut().push(
            InstanceRecord::new(
                fixture.instance.clone(),
                "Active Aurora",
                InstanceState::Ready,
                PinnedRelease::new(
                    crate::distribution::ReleaseChannel::Stable,
                    "2.1.5",
                    "1.21.11",
                    "0.19.5",
                )
                .unwrap(),
                InstanceConfiguration::for_minecraft_version("1.21.11"),
            )
            .unwrap(),
        );
        std::fs::create_dir_all(fixture.managed.launcher_dir()).unwrap();
        registry
            .save(&fixture.managed.instance_registry_file())
            .unwrap();
        let verified =
            verified_required_mods_with_manifest(&fixture.managed, &fixture.instance, &manifest)
                .unwrap();
        assert_eq!(
            verified
                .iter()
                .map(|metadata| metadata.id.as_str())
                .collect::<Vec<_>>(),
            vec!["aurora", "fabric-api"]
        );
        for field in ["sha256", "sizeBytes"] {
            let mut changed = state.clone();
            changed["fabricApi"]["artifact"][field] = if field == "sha256" {
                serde_json::json!("f".repeat(64))
            } else {
                serde_json::json!(1)
            };
            std::fs::write(&state_path, changed.to_string()).unwrap();
            assert!(
                verified_required_mods_with_manifest(
                    &fixture.managed,
                    &fixture.instance,
                    &manifest
                )
                .is_err()
            );
        }
        std::fs::write(&state_path, state.to_string()).unwrap();
        // The production manifest cannot be replaced by self-consistent local hashes.
        assert!(verified_required_mods(&fixture.managed, &fixture.instance).is_err());
        std::fs::write(api_path, b"tampered").unwrap();
        assert!(
            verified_required_mods_with_manifest(&fixture.managed, &fixture.instance, &manifest)
                .is_err()
        );
    }

    fn jar(path: &Path, metadata: Option<&[u8]>) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(
                "META-INF/MANIFEST.MF",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"Manifest-Version: 1.0").unwrap();
        if let Some(metadata) = metadata {
            writer
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(metadata).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn mixin_comments_preserve_strings_and_do_not_mask_invalid_json() {
        assert_eq!(
            mixin_json(
                br#"{/* block */"compatibilityLevel":"JAVA_21",// comment
            "url":"https://example.test/a/*literal*/"}"#
            )
            .unwrap()["compatibilityLevel"],
            "JAVA_21"
        );
        assert!(mixin_json(b"{/*unterminated").is_err());
        assert!(mixin_json(b"{garbage//comment\n}").is_err());
    }
    #[test]
    fn declared_mixin_resources_are_bounded_and_only_declared_resources_are_read() {
        for (name, declarations, resources, expected, bad) in [
            (
                "java21",
                serde_json::json!(["config.json"]),
                vec![(
                    "config.json",
                    r#"{"compatibilityLevel":"JAVA_21"}"#.to_owned(),
                )],
                vec![21],
                false,
            ),
            (
                "java25",
                serde_json::json!(["config.json"]),
                vec![(
                    "config.json",
                    r#"{"compatibilityLevel":"JAVA_25"}"#.to_owned(),
                )],
                vec![25],
                false,
            ),
            (
                "missing",
                serde_json::json!(["absent.json"]),
                vec![],
                vec![],
                true,
            ),
            (
                "malformed",
                serde_json::json!(["config.json"]),
                vec![("config.json", "invalid".into())],
                vec![],
                true,
            ),
            (
                "oversized",
                serde_json::json!(["config.json"]),
                vec![("config.json", " ".repeat(256 * 1024 + 1))],
                vec![],
                true,
            ),
            (
                "traversal",
                serde_json::json!(["../config.json"]),
                vec![("../config.json", "{}".into())],
                vec![],
                true,
            ),
            (
                "undeclared",
                serde_json::json!([]),
                vec![("arbitrary.json", "invalid".into())],
                vec![],
                false,
            ),
            (
                "multiple",
                serde_json::json!(["first.json",{"config":"second.json","environment":"client"},{"config":"server.json","environment":"server"}]),
                vec![
                    ("first.json", r#"{"compatibilityLevel":"JAVA_21"}"#.into()),
                    ("second.json", r#"{"compatibilityLevel":"JAVA_25"}"#.into()),
                ],
                vec![21, 25],
                false,
            ),
        ] {
            let fixture = Fixture::new(name);
            let path = fixture.mods().join("mixins.jar");
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            writer
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(
                    serde_json::json!({"id":"fixture","version":"1","mixins":declarations})
                        .to_string()
                        .as_bytes(),
                )
                .unwrap();
            for (path, bytes) in resources {
                writer
                    .start_file(path, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(bytes.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
            let (metadata, warnings) =
                inspect_fabric_metadata(&path, std::fs::metadata(&path).unwrap().len());
            assert_eq!(
                warnings.iter().any(|w| w.code == "mixin_metadata_invalid"),
                bad,
                "{name}: {warnings:?}"
            );
            assert_eq!(
                metadata
                    .as_ref()
                    .unwrap()
                    .mixin_java_requirements
                    .iter()
                    .map(|r| r.java_major)
                    .collect::<Vec<_>>(),
                expected,
                "{name}"
            );
            let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
            if name == "java25" {
                assert!(
                    crate::mod_compatibility::validate(
                        &inventory,
                        "1.21.11",
                        "0.19.5",
                        Some(21),
                        "fabric"
                    )
                    .iter()
                    .any(|i| i.code == "mod_java_incompatible")
                );
                std::fs::rename(&path, fixture.mods().join("mixins.jar.disabled")).unwrap();
                assert!(
                    crate::mod_compatibility::validate(
                        &scan(&fixture.managed, &fixture.instance).unwrap(),
                        "1.21.11",
                        "0.19.5",
                        Some(21),
                        "fabric"
                    )
                    .is_empty()
                );
            }
            if name == "java21" {
                assert!(
                    crate::mod_compatibility::validate(
                        &inventory,
                        "1.21.11",
                        "0.19.5",
                        Some(21),
                        "fabric"
                    )
                    .is_empty()
                );
            }
        }
    }

    fn metadata(id: &str, name: &str, depends: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "id": id,
            "name": name,
            "version": "1.2.3",
            "description": "Fixture",
            "authors": ["Aurora Test", {"name": "Second"}],
            "environment": "client",
            "depends": serde_json::from_str::<serde_json::Value>(depends).unwrap(),
            "icon": "assets/fixture/icon.png"
        }))
        .unwrap()
    }

    #[test]
    fn inventory_discovers_enabled_disabled_and_resilient_failures() {
        let fixture = Fixture::new("inventory");
        jar(
            &fixture.mods().join("useful.jar"),
            Some(&metadata("useful", "Useful Mod", r#"{"minecraft": ">=1"}"#)),
        );
        jar(
            &fixture.mods().join("sleepy.jar.disabled"),
            Some(&metadata("sleepy", "Sleepy Mod", "{}")),
        );
        jar(&fixture.mods().join("library.jar"), None);
        std::fs::write(fixture.mods().join("broken.jar"), b"not a zip").unwrap();
        std::fs::write(fixture.mods().join("notes.txt"), b"keep me").unwrap();
        std::fs::create_dir(fixture.mods().join("nested")).unwrap();

        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(inventory.entries.len(), 6);
        let useful = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "useful.jar")
            .unwrap();
        assert_eq!(useful.display_name, "Useful Mod");
        assert!(useful.enabled && useful.can_toggle && useful.can_remove);
        assert_eq!(
            useful.metadata.as_ref().unwrap().authors,
            ["Aurora Test", "Second"]
        );
        assert!(useful.metadata.as_ref().unwrap().has_declared_icon);
        let sleepy = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name.ends_with(".disabled"))
            .unwrap();
        assert_eq!(sleepy.display_name, "Sleepy Mod");
        assert!(!sleepy.enabled);
        assert!(
            inventory
                .entries
                .iter()
                .find(|entry| entry.file_name == "broken.jar")
                .unwrap()
                .warnings
                .iter()
                .any(|warning| warning.code == "jar_malformed")
        );
        assert_eq!(
            inventory
                .entries
                .iter()
                .find(|entry| entry.file_name == "nested")
                .unwrap()
                .file_type,
            ModFileType::Directory
        );
    }

    #[test]
    fn installed_state_protects_the_required_artifact() {
        let fixture = Fixture::new("managed");
        std::fs::write(fixture.mods().join("aurora-0.3.0.jar"), b"tiny").unwrap();
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let required = &inventory.entries[0];
        assert_eq!(required.ownership, ModOwnership::LauncherManagedRequired);
        assert!(!required.can_toggle && !required.can_remove);
        assert_eq!(
            set_enabled(
                &fixture.managed,
                &fixture.instance,
                &required.entry_id,
                false
            )
            .unwrap_err()
            .code(),
            "mod_required_artifact"
        );
        assert_eq!(
            remove(&fixture.managed, &fixture.instance, &required.entry_id)
                .unwrap_err()
                .code(),
            "mod_required_artifact"
        );
        assert!(fixture.mods().join("aurora-0.3.0.jar").is_file());
    }

    #[test]
    fn installed_state_also_protects_required_fabric_api() {
        let fixture = Fixture::new("fabric-api-managed");
        let state_path = fixture
            .managed
            .instance_paths(&fixture.instance)
            .root()
            .join(aurora::AURORA_INSTALLED_FILE_NAME);
        let mut state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        state["fabricApi"] = serde_json::json!({
            "version": "0.141.6+1.21.11",
            "artifact": {
                "relativePath": "mods/fabric-api-0.141.6+1.21.11.jar",
                "sizeBytes": 4,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }
        });
        std::fs::write(&state_path, serde_json::to_string(&state).unwrap()).unwrap();
        let path = fixture.mods().join("fabric-api-0.141.6+1.21.11.jar");
        std::fs::write(&path, b"tiny").unwrap();
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let required = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "fabric-api-0.141.6+1.21.11.jar")
            .unwrap();
        assert_eq!(required.ownership, ModOwnership::LauncherManagedRequired);
        assert!(!required.can_toggle && !required.can_remove);
        assert_eq!(
            remove(&fixture.managed, &fixture.instance, &required.entry_id)
                .unwrap_err()
                .code(),
            "mod_required_artifact"
        );
        assert!(path.is_file());
    }

    #[test]
    fn provider_mod_evidence_is_distinct_from_fabric_metadata_and_detects_tamper() {
        let fixture = Fixture::new("provider-evidence");
        let path = fixture.mods().join("managed.jar");
        jar(&path, Some(&metadata("managed", "Managed Mod", "{}")));
        let digest = format!("{:x}", sha2::Sha256::digest(std::fs::read(&path).unwrap()));
        let record = ProviderRecord {
            content_type: ContentType::Mod,
            provider: "synthetic".into(),
            project_id: "project-opaque".into(),
            version_id: "version-opaque".into(),
            file_id: "file-opaque".into(),
            file_name: "managed.jar".into(),
            sha256: digest.clone(),
            display_version: Some("1.2.3".into()),
            compatibility: crate::instance_content::ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: Some("fabric".into()),
                environment: Some("client".into()),
            },
            dependencies: vec![],
            explicitly_retained: true,
            requires: vec![],
            origin: crate::instance_content::ProviderOrigin::Direct,
            installed_at_unix_seconds: None,
            pinned: false,
            update_channel: crate::instance_content::UpdateChannel::Stable,
        };
        let mut state = ContentState::empty();
        state.entries.push(record.clone());
        state.save(&fixture.managed, &fixture.instance).unwrap();
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let entry = &inventory.entries[0];
        assert_eq!(entry.ownership, ModOwnership::ProviderManaged);
        assert_eq!(entry.sha256.as_deref(), Some(digest.as_str()));
        assert_eq!(
            entry.provenance.as_ref().unwrap().project_id,
            "project-opaque"
        );
        assert!(entry.can_toggle && !entry.can_remove);
        let before = std::fs::read(
            fixture
                .managed
                .instance_paths(&fixture.instance)
                .root()
                .join("content-managed.json"),
        )
        .unwrap();
        let disabled =
            set_enabled(&fixture.managed, &fixture.instance, &entry.entry_id, false).unwrap();
        assert!(!disabled.entries[0].enabled);
        assert_eq!(disabled.entries[0].provenance.as_ref(), Some(&record));
        let enabled = set_enabled(
            &fixture.managed,
            &fixture.instance,
            &disabled.entries[0].entry_id,
            true,
        )
        .unwrap();
        assert!(enabled.entries[0].enabled);
        assert_eq!(
            std::fs::read(
                fixture
                    .managed
                    .instance_paths(&fixture.instance)
                    .root()
                    .join("content-managed.json")
            )
            .unwrap(),
            before
        );
        std::fs::write(&path, b"tampered").unwrap();
        let tampered = scan(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(tampered.entries[0].ownership, ModOwnership::Unknown);
        assert!(!tampered.entries[0].can_remove);
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            scan(&fixture.managed, &fixture.instance)
                .unwrap()
                .missing_managed,
            vec![record]
        );
    }

    #[test]
    fn enable_disable_is_atomic_reversible_and_detects_collisions() {
        let fixture = Fixture::new("toggle");
        jar(
            &fixture.mods().join("toggle.jar"),
            Some(&metadata("toggle", "Toggle", "{}")),
        );
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let id = inventory.entries[0].entry_id.clone();
        let disabled = set_enabled(&fixture.managed, &fixture.instance, &id, false).unwrap();
        assert!(fixture.mods().join("toggle.jar.disabled").is_file());
        let disabled_entry = disabled
            .entries
            .iter()
            .find(|entry| entry.file_name == "toggle.jar.disabled")
            .unwrap();
        assert_eq!(disabled_entry.display_name, "Toggle");
        let enabled = set_enabled(
            &fixture.managed,
            &fixture.instance,
            &disabled_entry.entry_id,
            true,
        )
        .unwrap();
        assert!(
            enabled
                .entries
                .iter()
                .any(|entry| entry.file_name == "toggle.jar" && entry.enabled)
        );

        std::fs::rename(
            fixture.mods().join("toggle.jar"),
            fixture.mods().join("toggle.jar.disabled"),
        )
        .unwrap();
        std::fs::write(fixture.mods().join("toggle.jar"), b"collision").unwrap();
        let current = scan(&fixture.managed, &fixture.instance).unwrap();
        let disabled_id = current
            .entries
            .iter()
            .find(|entry| entry.file_name.ends_with(".disabled"))
            .unwrap()
            .entry_id
            .clone();
        assert_eq!(
            set_enabled(&fixture.managed, &fixture.instance, &disabled_id, true)
                .unwrap_err()
                .code(),
            "mod_target_conflict"
        );
    }

    #[test]
    fn remove_revalidates_identity_and_never_accepts_a_path() {
        let fixture = Fixture::new("remove");
        jar(
            &fixture.mods().join("remove.jar"),
            Some(&metadata("remove", "Remove", "{}")),
        );
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let id = inventory.entries[0].entry_id.clone();
        std::fs::write(fixture.mods().join("remove.jar"), b"changed after scan").unwrap();
        assert_eq!(
            remove(&fixture.managed, &fixture.instance, &id)
                .unwrap_err()
                .code(),
            "mod_entry_stale"
        );
        let refreshed = scan(&fixture.managed, &fixture.instance).unwrap();
        let fresh_id = refreshed.entries[0].entry_id.clone();
        let empty = remove(&fixture.managed, &fixture.instance, &fresh_id).unwrap();
        assert!(empty.entries.is_empty());
        assert_eq!(
            remove(&fixture.managed, &fixture.instance, "../remove.jar")
                .unwrap_err()
                .code(),
            "mod_entry_stale"
        );
    }

    #[test]
    fn derives_conservative_local_dependency_and_duplicate_warnings() {
        let fixture = Fixture::new("warnings");
        jar(
            &fixture.mods().join("first.jar"),
            Some(&metadata("same", "First", r#"{"missing": "*"}"#)),
        );
        jar(
            &fixture.mods().join("second.jar"),
            Some(&metadata("same", "Second", "{}")),
        );
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        assert!(inventory.entries.iter().all(|entry| {
            entry
                .warnings
                .iter()
                .any(|warning| warning.code == "duplicate_mod_id")
        }));
        assert!(
            inventory
                .entries
                .iter()
                .find(|entry| entry.display_name == "First")
                .unwrap()
                .warnings
                .iter()
                .any(|warning| warning.code == "required_dependency_missing")
        );
    }

    #[test]
    fn bounds_metadata_reads_and_uses_filename_fallback() {
        let fixture = Fixture::new("bounds");
        let oversized = vec![b' '; MAX_METADATA_BYTES as usize + 1];
        jar(&fixture.mods().join("oversized.jar"), Some(&oversized));
        jar(
            &fixture.mods().join("malformed.jar"),
            Some(br#"{"name":"No id"}"#),
        );
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let oversized = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "oversized.jar")
            .unwrap();
        assert_eq!(oversized.display_name, "oversized.jar");
        assert!(
            oversized
                .warnings
                .iter()
                .any(|warning| warning.code == "fabric_metadata_too_large")
        );
        let malformed = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "malformed.jar")
            .unwrap();
        assert_eq!(malformed.display_name, "malformed.jar");
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn links_are_visible_but_never_followed_or_mutated() {
        let fixture = Fixture::new("links");
        let outside = fixture.root.join("outside.jar");
        std::fs::write(&outside, b"outside").unwrap();
        let link = fixture.mods().join("linked.jar");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(windows)]
        if std::os::windows::fs::symlink_file(&outside, &link).is_err() {
            return;
        }
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let entry = &inventory.entries[0];
        assert_eq!(entry.file_type, ModFileType::Link);
        assert!(!entry.can_remove && !entry.can_toggle);
        assert_eq!(
            remove(&fixture.managed, &fixture.instance, &entry.entry_id)
                .unwrap_err()
                .code(),
            "mod_entry_unsafe"
        );
        assert_eq!(std::fs::read(outside).unwrap(), b"outside");
    }

    #[test]
    fn shallow_scan_handles_three_hundred_entries() {
        let fixture = Fixture::new("large");
        for index in 0..300 {
            std::fs::write(
                fixture.mods().join(format!("entry-{index:03}.txt")),
                b"fixture",
            )
            .unwrap();
        }
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(inventory.entries.len(), 300);
        assert_eq!(
            inventory.entries.first().unwrap().file_name,
            "entry-000.txt"
        );
    }

    #[test]
    fn conflict_declarations_report_the_constraint_without_claiming_version_applicability() {
        let fixture = Fixture::new("conditional-conflict");
        jar(
            &fixture.mods().join("consumer.jar"),
            Some(br#"{"schemaVersion":1,"id":"consumer","version":"1.0","breaks":{"api":"<2.0"}}"#),
        );
        jar(
            &fixture.mods().join("api.jar"),
            Some(br#"{"schemaVersion":1,"id":"api","version":"2.0"}"#),
        );
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let warning = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "consumer.jar")
            .unwrap()
            .warnings
            .iter()
            .find(|warning| warning.code == "declared_conflict_present")
            .unwrap();
        assert!(warning.message.contains("'<2.0'"));
        assert!(
            warning
                .message
                .contains("Fabric evaluates the version constraint")
        );
        assert!(!warning.message.contains("is present and enabled locally"));
    }

    #[test]
    fn declared_nested_modules_satisfy_local_dependencies_without_hiding_bad_archives() {
        let fixture = Fixture::new("nested-modules");
        let nested_bytes = {
            let cursor = Cursor::new(Vec::new());
            let mut writer = zip::ZipWriter::new(cursor);
            writer
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(
                    br#"{"schemaVersion":1,"id":"fabric-resource-loader-v1","version":"1.0"}"#,
                )
                .unwrap();
            writer.finish().unwrap().into_inner()
        };
        let outer = fixture.mods().join("api-style.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&outer).unwrap());
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(br#"{"schemaVersion":1,"id":"api-style","version":"1.0","jars":[{"file":"META-INF/jars/module.jar"}]}"#).unwrap();
        writer
            .start_file(
                "META-INF/jars/module.jar",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(&nested_bytes).unwrap();
        writer.finish().unwrap();
        jar(
            &fixture.mods().join("consumer.jar"),
            Some(&metadata(
                "consumer",
                "Consumer",
                r#"{"fabric-resource-loader-v1":"*"}"#,
            )),
        );
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        let outer_entry = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "api-style.jar")
            .unwrap();
        assert_eq!(
            outer_entry.metadata.as_ref().unwrap().nested_mod_ids,
            ["fabric-resource-loader-v1"]
        );
        assert!(outer_entry.warnings.is_empty());
        let consumer = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "consumer.jar")
            .unwrap();
        assert!(
            !consumer
                .warnings
                .iter()
                .any(|warning| warning.code == "required_dependency_missing")
        );

        // Fabric resolves shared nested modules, including differing versions.
        // Descriptive nested identities do not become top-level ownership.
        let overlapping = fixture.mods().join("overlapping.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&overlapping).unwrap());
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(br#"{"schemaVersion":1,"id":"overlapping","version":"1.0","jars":[{"file":"module.jar"}]}"#).unwrap();
        writer
            .start_file("module.jar", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&nested_bytes).unwrap();
        writer.finish().unwrap();
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        assert!(inventory.entries.iter().all(|entry| {
            !entry
                .warnings
                .iter()
                .any(|warning| warning.code == "duplicate_mod_id")
        }));

        let divergent_nested = {
            let cursor = Cursor::new(Vec::new());
            let mut writer = zip::ZipWriter::new(cursor);
            writer
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(
                    br#"{"schemaVersion":1,"id":"fabric-resource-loader-v1","version":"2.0"}"#,
                )
                .unwrap();
            writer.finish().unwrap().into_inner()
        };
        let divergent = fixture.mods().join("divergent.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&divergent).unwrap());
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(br#"{"schemaVersion":1,"id":"divergent","version":"1.0","jars":[{"file":"module.jar"}]}"#).unwrap();
        writer
            .start_file("module.jar", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&divergent_nested).unwrap();
        writer.finish().unwrap();
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        assert!(
            !inventory
                .entries
                .iter()
                .find(|entry| entry.file_name == "divergent.jar")
                .unwrap()
                .warnings
                .iter()
                .any(|warning| warning.code == "duplicate_mod_id")
        );

        let root_copy = fixture.mods().join("root-copy.jar");
        jar(
            &root_copy,
            Some(br#"{"schemaVersion":1,"id":"fabric-resource-loader-v1","version":"2.0"}"#),
        );
        let inventory = scan(&fixture.managed, &fixture.instance).unwrap();
        for name in ["root-copy.jar", "divergent.jar"] {
            assert!(
                inventory
                    .entries
                    .iter()
                    .find(|entry| entry.file_name == name)
                    .unwrap()
                    .warnings
                    .iter()
                    .any(|warning| warning.code == "duplicate_mod_id")
            );
        }

        let missing = fixture.mods().join("missing-nested.jar");
        jar(&missing, Some(br#"{"schemaVersion":1,"id":"missing-nested","version":"1.0","jars":[{"file":"nested/absent.jar"}]}"#));
        let (_, warnings) =
            inspect_fabric_metadata(&missing, std::fs::metadata(&missing).unwrap().len());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == "nested_jar_missing")
        );

        let malformed = fixture.mods().join("bad-nested.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&malformed).unwrap());
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(br#"{"schemaVersion":1,"id":"bad-nested","version":"1.0","jars":[{"file":"nested/bad.jar"}]}"#).unwrap();
        writer
            .start_file("nested/bad.jar", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"not a jar").unwrap();
        writer.finish().unwrap();
        let (_, warnings) =
            inspect_fabric_metadata(&malformed, std::fs::metadata(&malformed).unwrap().len());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == "nested_jar_malformed")
        );

        let malformed_metadata = {
            let cursor = Cursor::new(Vec::new());
            let mut writer = zip::ZipWriter::new(cursor);
            writer
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"{").unwrap();
            writer.finish().unwrap().into_inner()
        };
        let bad_metadata = fixture.mods().join("bad-metadata.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&bad_metadata).unwrap());
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(br#"{"schemaVersion":1,"id":"bad-metadata","jars":[{"file":"module.jar"}]}"#)
            .unwrap();
        writer
            .start_file("module.jar", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&malformed_metadata).unwrap();
        writer.finish().unwrap();
        let (_, warnings) = inspect_fabric_metadata(
            &bad_metadata,
            std::fs::metadata(&bad_metadata).unwrap().len(),
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == "nested_metadata_malformed")
        );
        let too_many_entries = {
            let cursor = Cursor::new(Vec::new());
            let mut writer = zip::ZipWriter::new(cursor);
            for index in 0..=MAX_ARCHIVE_ENTRIES {
                writer
                    .start_file(
                        format!("entry-{index}.txt"),
                        zip::write::SimpleFileOptions::default(),
                    )
                    .unwrap();
            }
            writer.finish().unwrap().into_inner()
        };
        let bounded = fixture.mods().join("bounded.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&bounded).unwrap());
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(br#"{"schemaVersion":1,"id":"bounded","jars":[{"file":"module.jar"}]}"#)
            .unwrap();
        writer
            .start_file("module.jar", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&too_many_entries).unwrap();
        writer.finish().unwrap();
        let (_, warnings) =
            inspect_fabric_metadata(&bounded, std::fs::metadata(&bounded).unwrap().len());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == "nested_jar_malformed")
        );

        let oversized_metadata = {
            let cursor = Cursor::new(Vec::new());
            let mut writer = zip::ZipWriter::new(cursor);
            writer
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(&vec![b'x'; MAX_METADATA_BYTES as usize + 1])
                .unwrap();
            writer.finish().unwrap().into_inner()
        };
        let too_large = fixture.mods().join("oversized-metadata.jar");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&too_large).unwrap());
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(br#"{"schemaVersion":1,"id":"oversized","jars":[{"file":"module.jar"}]}"#)
            .unwrap();
        writer
            .start_file("module.jar", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&oversized_metadata).unwrap();
        writer.finish().unwrap();
        let (_, warnings) =
            inspect_fabric_metadata(&too_large, std::fs::metadata(&too_large).unwrap().len());
        assert!(
            warnings
                .iter()
                .any(|warning| warning.code == "nested_metadata_malformed")
        );
    }
}
