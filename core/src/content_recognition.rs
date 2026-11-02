//! Explicit recognition of existing local instance content against Modrinth.
//! A scan is read-only and user-triggered; adoption registers provider
//! metadata only. The local file's bytes, name, location, and enabled state
//! are never touched by either operation.

use std::collections::HashSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use crate::instance_content::{
    self, ContentCompatibility, ContentError, ContentState, ContentType, ProviderIdentity,
    ProviderOrigin, ProviderRecord,
};
use crate::instances::InstanceId;
use crate::modrinth::{Client, RecognizedFile};
use crate::paths::ManagedPaths;

/// Hashing bound, aligned with the existing archive-inspection safety bound.
const MAX_RECOGNITION_BYTES: u64 = 512 * 1024 * 1024;
/// Bounded candidate set per scan; excess entries are reported as skipped.
const MAX_SCAN_CANDIDATES: usize = 256;
const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RecognitionStatus {
    Recognized,
    Unrecognized,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCandidate {
    pub status: RecognitionStatus,
    /// Local file name exactly as it appears in the content directory.
    pub file_name: String,
    /// The name a provider record would carry (mods: without `.disabled`).
    pub canonical_file_name: String,
    pub size_bytes: Option<u64>,
    pub disabled: bool,
    /// Verified provider identity for recognized candidates.
    pub recognition: Option<RecognizedFile>,
    /// Whether the canonical local name matches the published filename.
    pub filename_matches: Option<bool>,
    /// Digests are diagnostics; they never authorize anything by themselves.
    pub sha256: Option<String>,
    pub sha512: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionScan {
    pub instance_id: String,
    pub content_type: ContentType,
    /// Binds the preview to the exact inventory state that was inspected.
    pub inventory_revision: String,
    pub scan_fingerprint: String,
    pub candidates: Vec<ScanCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredFileApproval {
    pub file_name: String,
    pub sha512: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredContentApproval {
    pub instance_id: String,
    pub content_type: ContentType,
    pub inventory_revision: String,
    pub files: Vec<RecoveredFileApproval>,
}

/// The blocking local half of a scan: classification, bounded hashing, and
/// the inventory revision. Runs on a worker; performs no network I/O.
pub(crate) struct LocalScan {
    candidates: Vec<ScanCandidate>,
    hashed: Vec<HashedCandidate>,
    inventory_revision: String,
}

struct HashedCandidate {
    file_name: String,
    canonical_file_name: String,
    size_bytes: u64,
    disabled: bool,
    sha256: String,
    sha512: String,
}

/// Compute both digests in one read pass over a bounded regular file.
fn hash_file(path: &Path) -> Result<(String, String), ContentError> {
    let mut file = std::fs::File::open(path).map_err(ContentError::Io)?;
    let mut sha256 = sha2::Sha256::new();
    let mut sha512 = sha2::Sha512::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(ContentError::Io)?;
        if read == 0 {
            break;
        }
        sha256.update(&buffer[..read]);
        sha512.update(&buffer[..read]);
    }
    Ok((
        format!("{:x}", sha256.finalize()),
        format!("{:x}", sha512.finalize()),
    ))
}

/// The record name for a local file. Mods keep the canonical `.jar` name of
/// their disabled counterpart; packs use the local name as-is.
fn canonical_name(local: &str) -> String {
    if local.len() > DISABLED_SUFFIX.len()
        && local[..local.len() - DISABLED_SUFFIX.len()]
            .to_ascii_lowercase()
            .ends_with(".jar")
        && local[local.len() - DISABLED_SUFFIX.len()..].eq_ignore_ascii_case(DISABLED_SUFFIX)
    {
        local[..local.len() - DISABLED_SUFFIX.len()].to_owned()
    } else {
        local.to_owned()
    }
}

fn skipped(
    file_name: &str,
    size_bytes: Option<u64>,
    disabled: bool,
    reason: &str,
) -> ScanCandidate {
    ScanCandidate {
        status: RecognitionStatus::Skipped,
        file_name: file_name.to_owned(),
        canonical_file_name: canonical_name(file_name),
        size_bytes,
        disabled,
        recognition: None,
        filename_matches: None,
        sha256: None,
        sha512: None,
        reason: Some(reason.into()),
    }
}

pub(crate) fn scan_local(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
) -> Result<LocalScan, ContentError> {
    let mut candidates = Vec::new();
    let mut eligible: Vec<(String, PathBuf, u64, bool)> = Vec::new();
    match kind {
        ContentType::Mod => {
            let inventory = crate::instance_mods::scan(managed, instance)
                .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
            for entry in inventory.entries {
                let disabled = entry.file_type == crate::instance_mods::ModFileType::DisabledJar;
                let jar = matches!(
                    entry.file_type,
                    crate::instance_mods::ModFileType::EnabledJar
                        | crate::instance_mods::ModFileType::DisabledJar
                );
                if !jar {
                    candidates.push(skipped(
                        &entry.file_name,
                        entry.size_bytes,
                        false,
                        "Only enabled or disabled JAR files can be recognized.",
                    ));
                    continue;
                }
                match entry.ownership {
                    crate::instance_mods::ModOwnership::ProviderManaged => {
                        candidates.push(skipped(
                            &entry.file_name,
                            entry.size_bytes,
                            disabled,
                            "This file is already managed through its provider record.",
                        ));
                    }
                    crate::instance_mods::ModOwnership::LauncherManagedRequired => {
                        candidates.push(skipped(
                            &entry.file_name,
                            entry.size_bytes,
                            disabled,
                            "Required Aurora content is protected and never adopted.",
                        ))
                    }
                    crate::instance_mods::ModOwnership::LauncherBootstrap => {
                        candidates.push(skipped(
                            &entry.file_name,
                            entry.size_bytes,
                            disabled,
                            "Launcher bootstrap content is protected and never adopted.",
                        ))
                    }
                    crate::instance_mods::ModOwnership::Unknown => candidates.push(skipped(
                        &entry.file_name,
                        entry.size_bytes,
                        disabled,
                        "Aurora cannot prove this entry is a plain user-managed mod file.",
                    )),
                    crate::instance_mods::ModOwnership::UserManaged
                    | crate::instance_mods::ModOwnership::LauncherManagedRetained => {
                        eligible.push((
                            entry.file_name.clone(),
                            managed
                                .instance_paths(instance)
                                .mods()
                                .join(&entry.file_name),
                            entry.size_bytes.unwrap_or_default(),
                            disabled,
                        ));
                    }
                }
            }
        }
        kind => {
            let inventory = instance_content::scan(managed, instance, kind)?;
            let directory = instance_content::validate_directory(managed, instance, kind)?;
            for entry in inventory.entries {
                if entry.file_type != "zip" {
                    candidates.push(skipped(
                        &entry.file_name,
                        entry.size_bytes,
                        false,
                        "Only ZIP pack files can be recognized; folders and other entries are left untouched.",
                    ));
                    continue;
                }
                match entry.ownership {
                    instance_content::ContentOwnership::ProviderManaged => {
                        candidates.push(skipped(
                            &entry.file_name,
                            entry.size_bytes,
                            false,
                            "This file is already managed through its provider record.",
                        ));
                    }
                    instance_content::ContentOwnership::Unknown => candidates.push(skipped(
                        &entry.file_name,
                        entry.size_bytes,
                        false,
                        "This file no longer matches its provider-managed record; inspect it first.",
                    )),
                    instance_content::ContentOwnership::UserManaged => eligible.push((
                        entry.file_name.clone(),
                        directory.join(&entry.file_name),
                        entry.size_bytes.unwrap_or_default(),
                        false,
                    )),
                    instance_content::ContentOwnership::LauncherManagedRequired => {
                        candidates.push(skipped(
                            &entry.file_name,
                            entry.size_bytes,
                            false,
                            "Launcher-managed content is protected and never adopted.",
                        ));
                    }
                }
            }
        }
    }
    eligible.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hashed = Vec::new();
    for (index, (file_name, path, size_bytes, disabled)) in eligible.into_iter().enumerate() {
        if index >= MAX_SCAN_CANDIDATES {
            candidates.push(skipped(
                &file_name,
                Some(size_bytes),
                disabled,
                "The scan candidate bound was reached; adopt the current results and scan again.",
            ));
            continue;
        }
        if size_bytes > MAX_RECOGNITION_BYTES {
            candidates.push(skipped(
                &file_name,
                Some(size_bytes),
                disabled,
                "This file exceeds the 512 MiB recognition bound.",
            ));
            continue;
        }
        let (sha256, sha512) = match hash_file(&path) {
            Ok(hashes) => hashes,
            Err(_) => {
                candidates.push(skipped(
                    &file_name,
                    Some(size_bytes),
                    disabled,
                    "This file could not be read for hashing.",
                ));
                continue;
            }
        };
        hashed.push(HashedCandidate {
            canonical_file_name: canonical_name(&file_name),
            file_name,
            size_bytes,
            disabled,
            sha256,
            sha512,
        });
    }
    Ok(LocalScan {
        candidates,
        hashed,
        inventory_revision: inventory_revision(managed, instance, kind)?,
    })
}

/// The inventory revision a preview is bound to. Mods reuse the existing
/// mod-inventory revision; pack directories hash their sorted entry facts.
pub(crate) fn inventory_revision(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
) -> Result<String, ContentError> {
    match kind {
        ContentType::Mod => instance_content::local_inventory_revision(managed, instance),
        kind => {
            let inventory = instance_content::scan(managed, instance, kind)?;
            let material: Vec<_> = inventory
                .entries
                .iter()
                .map(|entry| {
                    format!(
                        "{}|{}|{:?}",
                        entry.file_name, entry.file_type, entry.size_bytes
                    )
                })
                .collect();
            Ok(format!("{:x}", sha2::Sha256::digest(material.join("\n"))))
        }
    }
}

/// The async provider half of a scan: batched SHA-512 lookup with response
/// verification, one project request per matched project for the type gate
/// and title, and final classification.
pub(crate) async fn recognize(
    instance: &InstanceId,
    kind: ContentType,
    local: LocalScan,
    client: &Client,
) -> Result<RecognitionScan, ContentError> {
    let LocalScan {
        mut candidates,
        hashed,
        inventory_revision,
    } = local;
    let lookup = client
        .lookup_files(
            &hashed
                .iter()
                .map(|candidate| candidate.sha512.clone())
                .collect::<Vec<_>>(),
        )
        .await
        .map_err(|error| ContentError::Acquisition(error.to_string()))?;
    let mut project_types = std::collections::HashMap::new();
    let mut titles = std::collections::HashMap::new();
    let mut project_ids: Vec<String> = lookup.iter().map(|file| file.project_id.clone()).collect();
    project_ids.sort();
    project_ids.dedup();
    for project_id in project_ids {
        let (project_type, title) = client
            .project_type_and_title(&project_id)
            .await
            .map_err(|error| ContentError::Acquisition(error.to_string()))?;
        project_types.insert(project_id.clone(), project_type);
        titles.insert(project_id, title);
    }
    let expected_type = match kind {
        ContentType::Mod => "mod",
        ContentType::ResourcePack => "resourcepack",
        ContentType::ShaderPack => "shader",
    };
    for candidate in hashed {
        let Some(recognized) = lookup
            .iter()
            .find(|file| file.queried_sha512 == candidate.sha512)
            .cloned()
        else {
            candidates.push(ScanCandidate {
                status: RecognitionStatus::Unrecognized,
                file_name: candidate.file_name,
                canonical_file_name: candidate.canonical_file_name,
                size_bytes: Some(candidate.size_bytes),
                disabled: candidate.disabled,
                recognition: None,
                filename_matches: None,
                sha256: Some(candidate.sha256),
                sha512: Some(candidate.sha512),
                reason: Some("No Modrinth version publishes these exact bytes.".into()),
            });
            continue;
        };
        if project_types
            .get(&recognized.project_id)
            .is_none_or(|kind| kind != expected_type)
        {
            candidates.push(ScanCandidate {
                status: RecognitionStatus::Unrecognized,
                file_name: candidate.file_name,
                canonical_file_name: candidate.canonical_file_name,
                size_bytes: Some(candidate.size_bytes),
                disabled: candidate.disabled,
                recognition: Some(recognized),
                filename_matches: None,
                sha256: Some(candidate.sha256),
                sha512: Some(candidate.sha512),
                reason: Some("Modrinth publishes these bytes as a different content type.".into()),
            });
            continue;
        }
        let filename_matches = candidate
            .canonical_file_name
            .eq_ignore_ascii_case(&recognized.file_name);
        candidates.push(ScanCandidate {
            status: RecognitionStatus::Recognized,
            file_name: candidate.file_name,
            canonical_file_name: candidate.canonical_file_name,
            size_bytes: Some(candidate.size_bytes),
            disabled: candidate.disabled,
            recognition: Some(recognized),
            filename_matches: Some(filename_matches),
            sha256: Some(candidate.sha256),
            sha512: Some(candidate.sha512),
            reason: None,
        });
    }
    candidates.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    let scan = RecognitionScan {
        instance_id: instance.to_string(),
        content_type: kind,
        inventory_revision,
        scan_fingerprint: String::new(),
        candidates,
    };
    let scan_fingerprint = fingerprint(&scan);
    Ok(RecognitionScan {
        scan_fingerprint,
        ..scan
    })
}

fn fingerprint(scan: &RecognitionScan) -> String {
    let material = serde_json::to_vec(&(
        &scan.instance_id,
        &scan.content_type,
        &scan.inventory_revision,
        &scan.candidates,
    ))
    .unwrap_or_default();
    format!("{:x}", sha2::Sha256::digest(material))
}

/// Resolve the local file an approved candidate names. Mods may live at
/// their disabled counterpart; both forms present at once is a collision.
fn resolve_local_file(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
    canonical_file_name: &str,
) -> Result<Option<PathBuf>, ContentError> {
    let directory = instance_content::validate_directory(managed, instance, kind)?;
    let active = directory.join(canonical_file_name);
    let disabled = directory.join(format!("{canonical_file_name}{DISABLED_SUFFIX}"));
    let has_active = std::fs::symlink_metadata(&active).is_ok();
    let has_disabled = std::fs::symlink_metadata(&disabled).is_ok();
    if kind == ContentType::Mod && has_active && has_disabled {
        return Err(ContentError::Collision);
    }
    let path = if kind == ContentType::Mod && has_disabled {
        disabled
    } else if has_active {
        active
    } else {
        return Ok(None);
    };
    let metadata = std::fs::symlink_metadata(&path).map_err(ContentError::Io)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ContentError::UnsafePath);
    }
    Ok(Some(path))
}

/// Metadata-only adoption. Every approved file is re-hashed and its bytes
/// re-verified against the previewed digest, provider identity is
/// re-established from a fresh verified lookup, and ownership conflicts are
/// checked under the instance content lock. No file is renamed, moved,
/// replaced, redownloaded, deleted, rewritten, or toggled; adoption changes
/// management metadata only.
pub(crate) async fn register_recovered_content(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
    approval: &RecoveredContentApproval,
    client: &Client,
) -> Result<Vec<ProviderRecord>, ContentError> {
    if approval.files.is_empty() || approval.files.len() > MAX_SCAN_CANDIDATES {
        return Err(ContentError::InvalidApproval);
    }
    let mut seen = HashSet::new();
    for file in &approval.files {
        instance_content::validate_file_name(&canonical_name(&file.file_name))?;
        if file.file_name.len() > 512
            || file.sha512.len() != 128
            || !file.sha512.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !seen.insert(file.file_name.to_ascii_lowercase())
        {
            return Err(ContentError::InvalidApproval);
        }
    }
    // Fresh provider verification outside the instance lock. The published
    // SHA-512 for each approved digest must still establish identity.
    let lookup = client
        .lookup_files(
            &approval
                .files
                .iter()
                .map(|file| file.sha512.to_ascii_lowercase())
                .collect::<Vec<_>>(),
        )
        .await
        .map_err(|error| ContentError::Acquisition(error.to_string()))?;
    let mut projects = std::collections::HashMap::new();
    let mut project_ids: Vec<String> = lookup.iter().map(|file| file.project_id.clone()).collect();
    project_ids.sort();
    project_ids.dedup();
    for project_id in project_ids {
        let (project_type, _) = client
            .project_type_and_title(&project_id)
            .await
            .map_err(|error| ContentError::Acquisition(error.to_string()))?;
        projects.insert(project_id, project_type);
    }
    let expected_type = match kind {
        ContentType::Mod => "mod",
        ContentType::ResourcePack => "resourcepack",
        ContentType::ShaderPack => "shader",
    };

    instance_content::with_instance_lock(instance, || {
        let current_revision = inventory_revision(managed, instance, kind)?;
        if current_revision != approval.inventory_revision {
            return Err(ContentError::ChangedSinceScan);
        }
        let mut state = ContentState::load(managed, instance)?;
        let existing_identities: Vec<ProviderIdentity> =
            state.entries.iter().map(ProviderRecord::identity).collect();
        let now = instance_content::now_unix_seconds();
        let mut records = Vec::new();
        let mut approved_names = HashSet::new();
        let mut approved_hashes = HashSet::new();
        let mut approved_projects = HashSet::new();
        for file in &approval.files {
            let canonical = canonical_name(&file.file_name);
            let Some(path) = resolve_local_file(managed, instance, kind, &canonical)? else {
                return Err(ContentError::ChangedSinceScan);
            };
            let size = std::fs::symlink_metadata(&path)
                .map_err(ContentError::Io)?
                .len();
            if size > MAX_RECOGNITION_BYTES {
                return Err(ContentError::UnsupportedAction);
            }
            let (sha256, sha512) = hash_file(&path)?;
            if sha512 != file.sha512.to_ascii_lowercase() {
                return Err(ContentError::HashMismatch);
            }
            let Some(recognized) = lookup
                .iter()
                .find(|file| file.queried_sha512 == sha512)
                .cloned()
            else {
                // The provider no longer publishes the previewed identity.
                return Err(ContentError::ChangedSinceScan);
            };
            if projects
                .get(&recognized.project_id)
                .is_none_or(|kind| kind != expected_type)
            {
                return Err(ContentError::UnsupportedAction);
            }
            check_ownership_conflicts(
                managed,
                instance,
                kind,
                &state,
                &canonical,
                &sha256,
                &recognized.project_id,
            )?;
            if !approved_names.insert(canonical.to_ascii_lowercase())
                || !approved_hashes.insert(sha256.clone())
                || !approved_projects.insert(recognized.project_id.clone())
            {
                return Err(ContentError::Collision);
            }
            if kind == ContentType::Mod {
                validate_recovered_mod(managed, instance, &path, size, &canonical)?;
            }
            // Requires edges point only at provider identities already
            // installed when recovery was approved; recovery never installs,
            // removes, or updates any dependency file.
            let requires = instance_content::required_edges_for(
                &recognized.dependencies,
                &existing_identities,
            );
            let record = ProviderRecord {
                content_type: kind,
                provider: "modrinth".into(),
                project_id: recognized.project_id.clone(),
                version_id: recognized.version_id.clone(),
                file_id: recognized.queried_sha512.clone(),
                file_name: canonical,
                sha256,
                display_version: Some(recognized.version_number.clone()),
                compatibility: ContentCompatibility {
                    minecraft_versions: recognized.game_versions.clone(),
                    loader: match kind {
                        ContentType::Mod => {
                            let platform_kind =
                                crate::instance_mods::platform_kind_of(managed, instance);
                            recognized
                                .loaders
                                .iter()
                                .find(|loader| loader.as_str() == platform_kind)
                                .or_else(|| recognized.loaders.first())
                                .cloned()
                        }
                        _ => None,
                    },
                    environment: Some(recognized.environment.clone()),
                },
                dependencies: recognized.dependencies.clone(),
                explicitly_retained: true,
                requires,
                origin: ProviderOrigin::Recovered,
                installed_at_unix_seconds: Some(now),
                pinned: false,
                update_channel: instance_content::UpdateChannel::Stable,
            };
            record.validate()?;
            records.push(record);
        }
        state.entries.extend(records.iter().cloned());
        state.save(managed, instance)?;
        Ok(records)
    })
}

/// A recovered file is adoptable only while no stronger evidence class claims
/// it: an existing provider record by name, bytes, or project identity; the
/// verified Aurora/Fabric API requirements; or retained bootstrap files.
fn check_ownership_conflicts(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
    state: &ContentState,
    canonical: &str,
    sha256: &str,
    project_id: &str,
) -> Result<(), ContentError> {
    for record in &state.entries {
        if record.content_type == kind && record.file_name.eq_ignore_ascii_case(canonical) {
            return Err(ContentError::Collision);
        }
        if record.sha256 == sha256 {
            return Err(ContentError::Collision);
        }
        if record.provider == "modrinth"
            && record.content_type == kind
            && record.project_id == project_id
        {
            return Err(ContentError::Collision);
        }
    }
    if kind == ContentType::Mod {
        let required = crate::instance_mods::managed_artifact_file_name(managed, instance)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
        if required.is_some_and(|names| {
            names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(canonical))
        }) {
            return Err(ContentError::Collision);
        }
        if let Some(state) = crate::aurora::load_installed_state(managed, instance)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?
        {
            let artifacts = std::iter::once(state.artifact())
                .chain(state.fabric_api().map(|api| api.artifact()));
            for artifact in artifacts {
                if artifact.sha256() == sha256 {
                    return Err(ContentError::Collision);
                }
            }
        }
        for file in crate::instances::transition::retained_files(managed, instance)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?
        {
            if let Some(name) = file.relative_path.strip_prefix("mods/") {
                if name.eq_ignore_ascii_case(canonical) || file.sha256 == sha256 {
                    return Err(ContentError::Collision);
                }
            }
        }
    }
    Ok(())
}

/// Fabric metadata must be inspectable, and the recovered mod may not
/// duplicate another installed top-level mod identity. This mirrors the
/// provider-install policy without gating on instance compatibility: the
/// bytes already live in this instance, and adoption records identity only.
fn validate_recovered_mod(
    managed: &ManagedPaths,
    instance: &InstanceId,
    path: &Path,
    bytes: u64,
    canonical: &str,
) -> Result<(), ContentError> {
    let (metadata, _) = crate::instance_mods::inspect_mod_metadata(
        path,
        bytes,
        &crate::instance_mods::platform_kind_of(managed, instance),
    );
    let Some(metadata) = metadata else {
        return Err(ContentError::InvalidProviderArtifact);
    };
    let inventory = crate::instance_mods::scan(managed, instance)
        .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
    for entry in &inventory.entries {
        if entry.file_name.eq_ignore_ascii_case(canonical)
            || entry
                .file_name
                .eq_ignore_ascii_case(&format!("{canonical}{DISABLED_SUFFIX}"))
        {
            continue;
        }
        if let Some(existing) = &entry.metadata {
            if let Some(id) = instance_content::conflicting_mod_identity(&metadata, existing) {
                return Err(ContentError::ModCollision(
                    instance_content::ProviderConflict {
                        mod_id: Some(id.to_owned()),
                        file_name: entry.file_name.clone(),
                        ownership: entry.ownership,
                        reason: "A top-level mod identity would duplicate an installed mod or bundled module. Neither file will be replaced or adopted.".into(),
                    },
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestRequest, TestResponse, TestServer};
    use serde_json::{Value, json};
    use std::io::Write as _;
    use std::sync::Arc;

    struct Fixture {
        root: PathBuf,
        managed: ManagedPaths,
        instance: InstanceId,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join("aurora-recognition-tests")
                .join(uuid::Uuid::new_v4().to_string());
            std::fs::create_dir_all(root.join("instances")).unwrap();
            let instance =
                InstanceId::new(format!("safe-{}", uuid::Uuid::new_v4().simple())).unwrap();
            std::fs::create_dir(root.join("instances").join(instance.as_str())).unwrap();
            let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
            register_fabric(&managed, &instance);
            Self {
                root,
                managed,
                instance,
            }
        }
        fn mods_dir(&self) -> PathBuf {
            let mods = self
                .managed
                .instance_paths(&self.instance)
                .mods()
                .to_path_buf();
            std::fs::create_dir_all(&mods).unwrap();
            mods
        }
        fn pack_dir(&self, kind: ContentType) -> PathBuf {
            instance_content::ensure_directory(&self.managed, &self.instance, kind).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn register_fabric(managed: &ManagedPaths, instance: &InstanceId) {
        use crate::instances::{
            InstanceRecord, InstanceRegistry, InstanceState, platform::InstalledConfiguration,
            settings::InstanceConfiguration,
        };
        let mut config = InstanceConfiguration::for_minecraft_version("1.21.11");
        config.set_aurora_enabled(false);
        let mut registry = InstanceRegistry::empty();
        registry.instances_mut().push(
            InstanceRecord::from_installed(
                instance.clone(),
                "Recognition fixture",
                InstanceState::Ready,
                InstalledConfiguration {
                    minecraft_version: "1.21.11".into(),
                    platform: crate::instances::platform::PlatformPin::Fabric {
                        version: "0.19.5".into(),
                    },
                    aurora: None,
                },
                config,
            )
            .unwrap(),
        );
        std::fs::create_dir_all(managed.launcher_dir()).unwrap();
        registry.save(&managed.instance_registry_file()).unwrap();
    }

    fn fabric_jar(id: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(
                json!({"schemaVersion":1,"id":id,"version":"1.0.0"})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn zip_pack(kind: ContentType) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let entry = if kind == ContentType::ShaderPack {
            "shaders/basic.fsh"
        } else {
            "pack.mcmeta"
        };
        writer
            .start_file(entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(b"{\"pack\":{\"pack_format\":42}}")
            .unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn sha256_of(bytes: &[u8]) -> String {
        use sha2::Digest as _;
        format!("{:x}", sha2::Sha256::digest(bytes))
    }

    fn sha512_of(bytes: &[u8]) -> String {
        use sha2::Digest as _;
        format!("{:x}", sha2::Sha512::digest(bytes))
    }

    fn project_json(id: &str, project_type: &str, title: &str) -> Value {
        json!({
            "id": id, "project_type": project_type, "title": title,
            "description": "d", "license": {"id": "MIT"},
            "game_versions": ["1.21.11"], "loaders": ["fabric"],
            "environment": ["client_and_server"]
        })
    }

    fn version_json(id: &str, project_id: &str, sha512: &str, filename: &str) -> Value {
        json!({
            "id": id, "project_id": project_id, "name": "Recognized release",
            "version_number": "1.4.2", "version_type": "release",
            "date_published": "2026-06-01T00:00:00Z",
            "game_versions": ["1.21.11"], "loaders": ["fabric"],
            "environment": "client_and_server",
            "files": [{
                "hashes": {"sha512": sha512},
                "url": "https://cdn.modrinth.com/data/t/f.jar",
                "filename": filename, "primary": true,
                "size": 64, "file_type": null
            }],
            "dependencies": []
        })
    }

    /// Serves version-file lookups for the given (sha512, project, filename)
    /// triples and one project document per matched project id.
    fn recognition_server(
        versions: Vec<(String, String, String)>,
        projects: Vec<(String, String, String)>,
    ) -> TestServer {
        TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let url = url::Url::parse(&format!("http://localhost{}", request.path)).unwrap();
            let path = url.path().to_owned();
            if let Some(id) = path.strip_prefix("/v2/project/") {
                return match projects.iter().find(|(project, _, _)| project == id) {
                    Some((project, project_type, title)) => TestResponse::ok(
                        &serde_json::to_vec(&project_json(project, project_type, title)).unwrap(),
                    ),
                    None => TestResponse::status(404),
                };
            }
            if path == "/v2/version_files" {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                let hashes: Vec<String> = body["hashes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap().to_owned())
                    .collect();
                let found: serde_json::Map<String, Value> = versions
                    .iter()
                    .filter(|(hash, _, _)| hashes.contains(hash))
                    .map(|(hash, project, filename)| {
                        (
                            hash.clone(),
                            version_json("11112222", project, hash, filename),
                        )
                    })
                    .collect();
                return TestResponse::ok(&serde_json::to_vec(&found).unwrap());
            }
            if let Some(hash) = path.strip_prefix("/v2/version_file/") {
                return match versions.iter().find(|(known, _, _)| known == hash) {
                    Some((hash, project, filename)) => TestResponse::ok(
                        &serde_json::to_vec(&version_json("11112222", project, hash, filename))
                            .unwrap(),
                    ),
                    None => TestResponse::status(404),
                };
            }
            TestResponse::status(404)
        }))
    }

    fn client(server: &TestServer) -> Client {
        Client::for_testing(&format!("{}/v2/", server.base_url()))
    }

    fn write_mod(fixture: &Fixture, name: &str, bytes: &[u8]) {
        std::fs::write(fixture.mods_dir().join(name), bytes).unwrap();
    }

    async fn run_scan(
        fixture: &Fixture,
        kind: ContentType,
        server: &TestServer,
    ) -> RecognitionScan {
        let local = scan_local(&fixture.managed, &fixture.instance, kind).unwrap();
        recognize(&fixture.instance, kind, local, &client(server))
            .await
            .unwrap()
    }

    fn approval(scan: &RecognitionScan, names: &[&str]) -> RecoveredContentApproval {
        RecoveredContentApproval {
            instance_id: scan.instance_id.clone(),
            content_type: scan.content_type,
            inventory_revision: scan.inventory_revision.clone(),
            files: scan
                .candidates
                .iter()
                .filter(|candidate| names.contains(&candidate.file_name.as_str()))
                .map(|candidate| RecoveredFileApproval {
                    file_name: candidate.file_name.clone(),
                    sha512: candidate.sha512.clone().expect("hashed candidate"),
                })
                .collect(),
        }
    }

    fn synthetic_record(file_name: &str, sha256: String) -> ProviderRecord {
        ProviderRecord {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: "AAAABBBB".into(),
            version_id: "11112222".into(),
            file_id: "f".repeat(128),
            file_name: file_name.into(),
            sha256,
            display_version: Some("1.0.0".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: Some("fabric".into()),
                environment: Some("client".into()),
            },
            dependencies: vec![],
            explicitly_retained: true,
            requires: vec![],
            origin: ProviderOrigin::Direct,
            installed_at_unix_seconds: Some(1),
            pinned: false,
            update_channel: instance_content::UpdateChannel::Stable,
        }
    }

    #[tokio::test]
    async fn scan_recognizes_unmanaged_mods_and_skips_protected_classes() {
        let fixture = Fixture::new();
        let recognized = fabric_jar("sodium");
        let unrecognized = fabric_jar("local-only");
        write_mod(&fixture, "sodium-renamed.jar", &recognized);
        write_mod(&fixture, "mystery.jar", &unrecognized);

        // An existing provider record claims this file by name.
        let managed_jar = fabric_jar("already-managed");
        write_mod(&fixture, "already-managed.jar", &managed_jar);
        let mut state = ContentState::empty();
        state.entries.push(synthetic_record(
            "already-managed.jar",
            sha256_of(&managed_jar),
        ));
        // A second record whose bytes drifted: ownership becomes Unknown.
        let drifted_jar = fabric_jar("drifted");
        write_mod(&fixture, "drifted.jar", &drifted_jar);
        let mut drifted = synthetic_record("drifted.jar", sha256_of(b"other bytes"));
        drifted.project_id = "BBBBCCCC".into();
        state.entries.push(drifted);
        state.save(&fixture.managed, &fixture.instance).unwrap();

        // A non-JAR entry.
        std::fs::write(fixture.mods_dir().join("readme.txt"), b"notes").unwrap();

        let hash = sha512_of(&recognized);
        let server = recognition_server(
            vec![(hash.clone(), "AAAABBBB".into(), "sodium-1.4.2.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Sodium".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        let by_name = |name: &str| {
            scan.candidates
                .iter()
                .find(|candidate| candidate.file_name == name)
                .unwrap_or_else(|| panic!("candidate {name} missing"))
        };
        let found = by_name("sodium-renamed.jar");
        assert_eq!(found.status, RecognitionStatus::Recognized);
        let identity = found.recognition.as_ref().unwrap();
        assert_eq!(identity.project_id, "AAAABBBB");
        assert_eq!(identity.version_number, "1.4.2");
        assert_eq!(found.filename_matches, Some(false));
        assert_eq!(found.sha512.as_deref(), Some(hash.as_str()));

        let missing = by_name("mystery.jar");
        assert_eq!(missing.status, RecognitionStatus::Unrecognized);
        assert!(
            missing
                .reason
                .as_deref()
                .is_some_and(|reason| !reason.is_empty())
        );

        let provider_owned = by_name("already-managed.jar");
        assert_eq!(provider_owned.status, RecognitionStatus::Skipped);

        let drifted = by_name("drifted.jar");
        assert_eq!(drifted.status, RecognitionStatus::Skipped);

        let readme = by_name("readme.txt");
        assert_eq!(readme.status, RecognitionStatus::Skipped);
        assert_eq!(scan.instance_id, fixture.instance.to_string());
        assert!(!scan.inventory_revision.is_empty());
        assert_eq!(scan.scan_fingerprint.len(), 64);
    }

    #[tokio::test]
    async fn scan_recognizes_resource_packs_and_rejects_cross_type_matches() {
        let fixture = Fixture::new();
        let pack = zip_pack(ContentType::ResourcePack);
        std::fs::write(
            fixture
                .pack_dir(ContentType::ResourcePack)
                .join("fancy.zip"),
            &pack,
        )
        .unwrap();
        // A directory-form pack is visible but not recognizable.
        std::fs::create_dir(
            fixture
                .pack_dir(ContentType::ResourcePack)
                .join("folder-pack"),
        )
        .unwrap();

        let hash = sha512_of(&pack);
        let server = recognition_server(
            vec![(hash.clone(), "EEEEFFFF".into(), "fancy-pack.zip".into())],
            // The bytes exist on Modrinth, but as a mod project.
            vec![("EEEEFFFF".into(), "mod".into(), "Actually a mod".into())],
        );
        let scan = run_scan(&fixture, ContentType::ResourcePack, &server).await;
        let fancy = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == "fancy.zip")
            .unwrap();
        assert_eq!(
            fancy.status,
            RecognitionStatus::Unrecognized,
            "a different declared project type must not be adopted as a pack"
        );
        let folder = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == "folder-pack")
            .unwrap();
        assert_eq!(folder.status, RecognitionStatus::Skipped);

        // With an honest resourcepack project the same bytes are recognized.
        let server = recognition_server(
            vec![(hash, "EEEEFFFF".into(), "fancy.zip".into())],
            vec![(
                "EEEEFFFF".into(),
                "resourcepack".into(),
                "Fancy Pack".into(),
            )],
        );
        let scan = run_scan(&fixture, ContentType::ResourcePack, &server).await;
        let fancy = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == "fancy.zip")
            .unwrap();
        assert_eq!(fancy.status, RecognitionStatus::Recognized);
        assert_eq!(fancy.filename_matches, Some(true));
    }

    #[tokio::test]
    async fn scan_binds_the_preview_to_the_inspected_inventory() {
        let fixture = Fixture::new();
        let pack = zip_pack(ContentType::ShaderPack);
        std::fs::write(
            fixture.pack_dir(ContentType::ShaderPack).join("shader.zip"),
            &pack,
        )
        .unwrap();
        let hash = sha512_of(&pack);
        let server = recognition_server(
            vec![(hash, "GGGGHHHH".into(), "shader.zip".into())],
            vec![("GGGGHHHH".into(), "shader".into(), "Shader".into())],
        );
        let first = run_scan(&fixture, ContentType::ShaderPack, &server).await;
        assert_eq!(first.candidates.len(), 1);
        // The revision is stable while the directory is untouched.
        let second = run_scan(&fixture, ContentType::ShaderPack, &server).await;
        assert_eq!(first.inventory_revision, second.inventory_revision);
        // Adding any entry changes the revision.
        std::fs::write(
            fixture.pack_dir(ContentType::ShaderPack).join("other.zip"),
            b"other",
        )
        .unwrap();
        let third = run_scan(&fixture, ContentType::ShaderPack, &server).await;
        assert_ne!(first.inventory_revision, third.inventory_revision);
    }

    #[tokio::test]
    async fn scan_skips_oversized_candidates() {
        let fixture = Fixture::new();
        let oversized = fixture.mods_dir().join("huge.jar");
        let file = std::fs::File::create(&oversized).unwrap();
        // A sparse file beyond the bound; hashing is never attempted.
        file.set_len(MAX_RECOGNITION_BYTES + 1).unwrap();
        drop(file);
        let small = fabric_jar("small");
        write_mod(&fixture, "small.jar", &small);
        let server = recognition_server(
            vec![(sha512_of(&small), "AAAABBBB".into(), "small.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Small".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        let huge = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == "huge.jar")
            .unwrap();
        assert_eq!(huge.status, RecognitionStatus::Skipped);
        assert!(huge.sha512.is_none());
    }

    #[tokio::test]
    async fn scan_reports_disabled_mods_with_canonical_names() {
        let fixture = Fixture::new();
        let jar = fabric_jar("paused");
        write_mod(&fixture, "paused.jar.disabled", &jar);
        let server = recognition_server(
            vec![(sha512_of(&jar), "AAAABBBB".into(), "paused-1.0.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Paused".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        let candidate = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == "paused.jar.disabled")
            .unwrap();
        assert_eq!(candidate.status, RecognitionStatus::Recognized);
        assert!(candidate.disabled);
        assert_eq!(candidate.canonical_file_name, "paused.jar");
    }

    #[tokio::test]
    async fn adoption_preserves_file_bytes_path_and_enabled_state() {
        let fixture = Fixture::new();
        let jar = fabric_jar("lithium");
        write_mod(&fixture, "lithium-kept-name.jar", &jar);
        let hash = sha512_of(&jar);
        let server = recognition_server(
            vec![(hash.clone(), "AAAABBBB".into(), "lithium-1.4.2.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Lithium".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        let path = fixture.mods_dir().join("lithium-kept-name.jar");
        let before = std::fs::read(&path).unwrap();
        let before_meta = std::fs::metadata(&path).unwrap();

        let approval = approval(&scan, &["lithium-kept-name.jar"]);
        let records = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &approval,
            &client(&server),
        )
        .await
        .unwrap();
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.origin, ProviderOrigin::Recovered);
        assert_eq!(record.provider, "modrinth");
        assert_eq!(record.project_id, "AAAABBBB");
        assert_eq!(record.version_id, "11112222");
        assert_eq!(record.file_id, hash);
        assert_eq!(record.file_name, "lithium-kept-name.jar");
        assert_eq!(record.sha256, sha256_of(&jar));
        assert!(record.installed_at_unix_seconds.is_some());
        assert!(record.explicitly_retained);

        // Absolute file preservation: same path, same bytes, same size.
        let after = std::fs::read(&path).unwrap();
        let after_meta = std::fs::metadata(&path).unwrap();
        assert_eq!(before, after);
        assert_eq!(before_meta.len(), after_meta.len());
        assert!(before_meta.is_file() && after_meta.is_file());

        // The persisted state carries the recovered record.
        let state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].origin, ProviderOrigin::Recovered);

        // The inventory now classifies the same untouched file as
        // provider-managed through the ordinary evidence path.
        let inventory = crate::instance_mods::scan(&fixture.managed, &fixture.instance).unwrap();
        let entry = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "lithium-kept-name.jar")
            .unwrap();
        assert_eq!(
            entry.ownership,
            crate::instance_mods::ModOwnership::ProviderManaged
        );
        assert!(entry.enabled);
    }

    #[tokio::test]
    async fn adoption_keeps_disabled_mods_disabled() {
        let fixture = Fixture::new();
        let jar = fabric_jar("ferrite");
        write_mod(&fixture, "ferrite.jar.disabled", &jar);
        let server = recognition_server(
            vec![(sha512_of(&jar), "AAAABBBB".into(), "ferrite.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Ferrite".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        let approval = approval(&scan, &["ferrite.jar.disabled"]);
        let records = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &approval,
            &client(&server),
        )
        .await
        .unwrap();
        assert_eq!(records[0].file_name, "ferrite.jar");
        // The disabled file still exists; nothing toggled enabled state.
        assert!(fixture.mods_dir().join("ferrite.jar.disabled").is_file());
        assert!(!fixture.mods_dir().join("ferrite.jar").exists());
    }

    #[tokio::test]
    async fn adoption_refuses_a_stale_preview_revision() {
        let fixture = Fixture::new();
        let jar = fabric_jar("hydrogen");
        write_mod(&fixture, "hydrogen.jar", &jar);
        let server = recognition_server(
            vec![(sha512_of(&jar), "AAAABBBB".into(), "hydrogen.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Hydrogen".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        // The directory changed after the scan: a new file appeared.
        write_mod(&fixture, "late.jar", &fabric_jar("late"));
        let approval = approval(&scan, &["hydrogen.jar"]);
        let result = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &approval,
            &client(&server),
        )
        .await;
        assert!(matches!(result, Err(ContentError::ChangedSinceScan)));
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );
    }

    #[tokio::test]
    async fn adoption_refuses_changed_bytes() {
        // A same-length pack replacement keeps the pack revision stable, so
        // the refusal comes from the digest re-verification itself.
        let fixture = Fixture::new();
        let pack = zip_pack(ContentType::ResourcePack);
        std::fs::write(
            fixture
                .pack_dir(ContentType::ResourcePack)
                .join("changed.zip"),
            &pack,
        )
        .unwrap();
        let server = recognition_server(
            vec![(sha512_of(&pack), "EEEEFFFF".into(), "changed.zip".into())],
            vec![("EEEEFFFF".into(), "resourcepack".into(), "Changed".into())],
        );
        let scan = run_scan(&fixture, ContentType::ResourcePack, &server).await;
        // Replace the file with different bytes of the same length.
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(b"{\"pack\":{\"pack_format\":43}}")
            .unwrap();
        let replacement = writer.finish().unwrap().into_inner();
        assert_eq!(replacement.len(), pack.len());
        std::fs::write(
            fixture
                .pack_dir(ContentType::ResourcePack)
                .join("changed.zip"),
            &replacement,
        )
        .unwrap();
        let pack_approval = approval(&scan, &["changed.zip"]);
        let result = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &pack_approval,
            &client(&server),
        )
        .await;
        assert!(matches!(result, Err(ContentError::HashMismatch)));
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );

        // For mods, the inventory revision itself covers per-file digests, so
        // replaced bytes are refused as a stale preview before re-hashing.
        let fixture = Fixture::new();
        let jar = fabric_jar("changed");
        write_mod(&fixture, "changed.jar", &jar);
        let server = recognition_server(
            vec![(sha512_of(&jar), "AAAABBBB".into(), "changed.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Changed".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        write_mod(&fixture, "changed.jar", &fabric_jar("different"));
        let mod_approval = approval(&scan, &["changed.jar"]);
        let result = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &mod_approval,
            &client(&server),
        )
        .await;
        assert!(matches!(result, Err(ContentError::ChangedSinceScan)));
    }

    #[tokio::test]
    async fn adoption_refuses_existing_provider_and_retained_ownership() {
        // Provider ownership by project identity.
        let fixture = Fixture::new();
        let jar = fabric_jar("claimed");
        write_mod(&fixture, "claimed.jar", &jar);
        let mut state = ContentState::empty();
        let mut existing =
            synthetic_record("claimed-old-name.jar", sha256_of(&fabric_jar("other")));
        existing.project_id = "AAAABBBB".into();
        state.entries.push(existing.clone());
        state.save(&fixture.managed, &fixture.instance).unwrap();
        let server = recognition_server(
            vec![(sha512_of(&jar), "AAAABBBB".into(), "claimed.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Claimed".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        let approval = approval(&scan, &["claimed.jar"]);
        let result = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &approval,
            &client(&server),
        )
        .await;
        assert!(matches!(result, Err(ContentError::Collision)));
        // The existing record is untouched.
        let after = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(after.entries.len(), 1);
        assert_eq!(after.entries[0], existing);

        // Retained bootstrap ownership refuses adoption by name and bytes.
        let fixture = Fixture::new();
        let jar = fabric_jar("retained");
        write_mod(&fixture, "retained.jar", &jar);
        std::fs::write(
            fixture
                .managed
                .instance_paths(&fixture.instance)
                .root()
                .join("aurora-retained.json"),
            serde_json::to_vec(&json!({
                "schemaVersion": 1,
                "files": [{
                    "relativePath": "mods/retained.jar",
                    "sha256": sha256_of(&jar),
                    "sizeBytes": jar.len() as u64,
                    "reason": "retained by transition"
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let server = recognition_server(
            vec![(sha512_of(&jar), "AAAABBBB".into(), "retained.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Retained".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        // Retained evidence classifies the file as protected at scan time.
        let candidate = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == "retained.jar")
            .unwrap();
        assert_eq!(candidate.status, RecognitionStatus::Skipped);
        // Even a directly forged approval cannot adopt it.
        let approval = RecoveredContentApproval {
            instance_id: fixture.instance.to_string(),
            content_type: ContentType::Mod,
            inventory_revision: scan.inventory_revision.clone(),
            files: vec![RecoveredFileApproval {
                file_name: "retained.jar".into(),
                sha512: sha512_of(&jar),
            }],
        };
        let result = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &approval,
            &client(&server),
        )
        .await;
        assert!(matches!(result, Err(ContentError::Collision)));
    }

    #[tokio::test]
    async fn adoption_refuses_duplicate_mod_identities() {
        let fixture = Fixture::new();
        let jar = fabric_jar("indium");
        write_mod(&fixture, "indium.jar", &jar);
        write_mod(&fixture, "indium-copy.jar", &jar);
        let server = recognition_server(
            vec![(sha512_of(&jar), "AAAABBBB".into(), "indium.jar".into())],
            vec![("AAAABBBB".into(), "mod".into(), "Indium".into())],
        );
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        let approval = approval(&scan, &["indium.jar"]);
        let result = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &approval,
            &client(&server),
        )
        .await;
        // The duplicate identity is a conflict; neither file is touched.
        assert!(matches!(result, Err(ContentError::ModCollision(_))));
        assert!(fixture.mods_dir().join("indium.jar").is_file());
        assert!(fixture.mods_dir().join("indium-copy.jar").is_file());
    }

    #[tokio::test]
    async fn unrecognized_files_are_never_touched_by_scan_or_adoption() {
        let fixture = Fixture::new();
        let unknown = fabric_jar("personal");
        write_mod(&fixture, "personal.jar", &unknown);
        // The provider knows nothing: batch lookups answer an empty list and
        // the single-hash endpoint answers a real 404.
        let server = TestServer::spawn(Arc::new(|request: &TestRequest| {
            if request.method == "POST" {
                TestResponse::ok(b"[]")
            } else {
                TestResponse::status(404)
            }
        }));
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        let candidate = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == "personal.jar")
            .unwrap();
        assert_eq!(candidate.status, RecognitionStatus::Unrecognized);
        // Adoption of a file the provider does not know is a typed refusal.
        let approval = RecoveredContentApproval {
            instance_id: fixture.instance.to_string(),
            content_type: ContentType::Mod,
            inventory_revision: scan.inventory_revision.clone(),
            files: vec![RecoveredFileApproval {
                file_name: "personal.jar".into(),
                sha512: sha512_of(&unknown),
            }],
        };
        let result = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &approval,
            &client(&server),
        )
        .await;
        assert!(matches!(result, Err(ContentError::ChangedSinceScan)));
        assert_eq!(
            std::fs::read(fixture.mods_dir().join("personal.jar")).unwrap(),
            unknown
        );
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );
    }

    #[tokio::test]
    async fn adoption_links_only_preexisting_provider_requirements() {
        let fixture = Fixture::new();
        let jar = fabric_jar("requiring");
        write_mod(&fixture, "requiring.jar", &jar);
        let dependency = fabric_jar("dependency");
        write_mod(&fixture, "dependency.jar", &dependency);

        // Install the dependency as an ordinary provider record first.
        let dep_server = recognition_server(
            vec![(
                sha512_of(&dependency),
                "DEP00001".into(),
                "dependency.jar".into(),
            )],
            vec![("DEP00001".into(), "mod".into(), "Dependency".into())],
        );
        let dep_scan = run_scan(&fixture, ContentType::Mod, &dep_server).await;
        let dep_approval = approval(&dep_scan, &["dependency.jar"]);
        register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &dep_approval,
            &client(&dep_server),
        )
        .await
        .unwrap();

        // The root declares a required dependency on DEP00001.
        let root_hash = sha512_of(&jar);
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let url = url::Url::parse(&format!("http://localhost{}", request.path)).unwrap();
            let path = url.path().to_owned();
            if path == "/v2/project/ROOT0001" {
                return TestResponse::ok(
                    &serde_json::to_vec(&project_json("ROOT0001", "mod", "Root")).unwrap(),
                );
            }
            if path == "/v2/version_files" || path.starts_with("/v2/version_file/") {
                let version = json!({
                    "id": "22223333", "project_id": "ROOT0001", "name": "Root",
                    "version_number": "2.0.0", "version_type": "release",
                    "date_published": "2026-06-01T00:00:00Z",
                    "game_versions": ["1.21.11"], "loaders": ["fabric"],
                    "environment": "client_and_server",
                    "files": [{
                        "hashes": {"sha512": root_hash},
                        "url": "https://cdn.modrinth.com/data/t/r.jar",
                        "filename": "requiring.jar", "primary": true,
                        "size": 64, "file_type": null
                    }],
                    "dependencies": [
                        {"project_id": "DEP00001", "version_id": null, "dependency_type": "required"}
                    ]
                });
                if path.starts_with("/v2/version_file/") {
                    return TestResponse::ok(&serde_json::to_vec(&version).unwrap());
                }
                return TestResponse::ok(&serde_json::to_vec(&[version]).unwrap());
            }
            TestResponse::status(404)
        }));
        let scan = run_scan(&fixture, ContentType::Mod, &server).await;
        let approval = approval(&scan, &["requiring.jar"]);
        let records = register_recovered_content(
            &fixture.managed,
            &fixture.instance,
            ContentType::Mod,
            &approval,
            &client(&server),
        )
        .await
        .unwrap();
        assert_eq!(records.len(), 1);
        // The edge points at the already-installed provider identity.
        assert_eq!(records[0].requires.len(), 1);
        assert_eq!(records[0].requires[0].project_id, "DEP00001");
        // Both files still exist, untouched.
        assert!(fixture.mods_dir().join("requiring.jar").is_file());
        assert!(fixture.mods_dir().join("dependency.jar").is_file());
    }

    #[test]
    fn canonical_name_strips_only_a_jar_disabled_suffix() {
        assert_eq!(canonical_name("mod.jar.disabled"), "mod.jar");
        assert_eq!(canonical_name("mod.JAR.DISABLED"), "mod.JAR");
        assert_eq!(canonical_name("mod.jar"), "mod.jar");
        assert_eq!(canonical_name("pack.zip"), "pack.zip");
        assert_eq!(canonical_name("notes.disabled"), "notes.disabled");
        assert_eq!(canonical_name(".disabled"), ".disabled");
    }
}

#[cfg(test)]
mod performance {
    use super::*;
    use crate::test_support::{TestRequest, TestResponse, TestServer};
    use std::io::Write as _;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    /// A representative explicit scan: hashing runs on bounded local files,
    /// provider lookups batch at the audit bound, and no idle work exists
    /// before or after the scan.
    #[tokio::test]
    async fn scan_measures_hashing_and_batched_requests() {
        let root = std::env::temp_dir()
            .join("aurora-recognition-perf")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(root.join("instances")).unwrap();
        let instance = InstanceId::new(format!("safe-{}", uuid::Uuid::new_v4().simple())).unwrap();
        std::fs::create_dir_all(root.join("instances").join(instance.as_str())).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        instance_content::ensure_directory(&managed, &instance, ContentType::Mod).unwrap();
        {
            use crate::instances::{
                InstanceRecord, InstanceRegistry, InstanceState, platform::InstalledConfiguration,
                settings::InstanceConfiguration,
            };
            let mut config = InstanceConfiguration::for_minecraft_version("1.21.11");
            config.set_aurora_enabled(false);
            let mut registry = InstanceRegistry::empty();
            registry.instances_mut().push(
                InstanceRecord::from_installed(
                    instance.clone(),
                    "Recognition perf",
                    InstanceState::Ready,
                    InstalledConfiguration {
                        minecraft_version: "1.21.11".into(),
                        platform: crate::instances::platform::PlatformPin::Fabric {
                            version: "0.19.5".into(),
                        },
                        aurora: None,
                    },
                    config,
                )
                .unwrap(),
            );
            std::fs::create_dir_all(managed.launcher_dir()).unwrap();
            registry.save(&managed.instance_registry_file()).unwrap();
        }
        let mods = managed.instance_paths(&instance).mods().to_path_buf();

        let files = 130usize;
        let payload: Vec<u8> = (0..64 * 1024usize).map(|byte| (byte % 251) as u8).collect();
        let mut jars = Vec::new();
        for index in 0..files {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(
                    format!("{{\"schemaVersion\":1,\"id\":\"perf{index}\",\"version\":\"1.0.0\"}}")
                        .as_bytes(),
                )
                .unwrap();
            writer
                .start_file("payload.bin", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&payload).unwrap();
            jars.push(writer.finish().unwrap().into_inner());
        }
        for (index, jar) in jars.iter().enumerate() {
            std::fs::write(mods.join(format!("perf-{index:03}.jar")), jar).unwrap();
        }
        let bytes_total: u64 = jars.iter().map(|jar| jar.len() as u64).sum();

        let requests = Arc::new(Mutex::new(0usize));
        let seen = requests.clone();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            *seen.lock().unwrap() += 1;
            if request.method == "POST" {
                TestResponse::ok(b"{}")
            } else {
                TestResponse::status(404)
            }
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));

        let total_start = Instant::now();
        let hash_start = Instant::now();
        let local = scan_local(&managed, &instance, ContentType::Mod).unwrap();
        let hash_duration = hash_start.elapsed();
        let scan = recognize(&instance, ContentType::Mod, local, &client)
            .await
            .unwrap();
        let total_duration = total_start.elapsed();

        println!(
            "files scanned: {files}, bytes hashed: {bytes_total}, hash duration: {:?}, provider requests: {}, total duration: {:?}",
            hash_duration,
            *requests.lock().unwrap(),
            total_duration
        );
        assert_eq!(scan.candidates.len(), files);
        assert!(
            scan.candidates
                .iter()
                .all(|candidate| candidate.status == RecognitionStatus::Unrecognized)
        );
        // Batch expectation: ceil(130 / 64) = 3, with no per-file requests.
        assert_eq!(*requests.lock().unwrap(), 3);
        let _ = std::fs::remove_dir_all(&root);
    }
}
#[cfg(test)]
mod live_acceptance {
    use super::*;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};

    /// Explicit live acceptance against real Modrinth with a disposable
    /// managed root. Never points at production application data. Enabled
    /// only through AURORA_RECOGNITION_ROOT pointing at a disposable
    /// temporary directory.
    #[tokio::test]
    #[ignore = "requires live Modrinth and an explicit disposable root"]
    async fn live_modrinth_recognition_adopts_without_touching_bytes() {
        let root =
            PathBuf::from(std::env::var_os("AURORA_RECOGNITION_ROOT").expect("disposable root"));
        assert!(root.starts_with(std::env::temp_dir()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-recognition-acceptance-")
        );
        std::fs::create_dir_all(root.join("instances")).unwrap();
        let instance = InstanceId::new(format!("safe-{}", uuid::Uuid::new_v4().simple())).unwrap();
        std::fs::create_dir_all(root.join("instances").join(instance.as_str())).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        crate::instance_content::ensure_directory(&managed, &instance, ContentType::Mod).unwrap();

        // Acquire a real, officially published Modrinth artifact through the
        // verified SHA-512 store, then place an unmanaged local copy under a
        // different filename: the exact "existing local content" scenario.
        let cache = crate::cache::ArtifactCache::new(managed.clone());
        let source = crate::downloads::Sha512ArtifactSource::https(
            "https://cdn.modrinth.com/data/eXts2L7r/versions/qxjzQ9xY/placeholder-api-2.8.2%2B1.21.10.jar",
            "507ab10b7938dcd14d33121b8462649bdbe575cef248e917dfdf7566078ab5d0195ca1add95eae4863de3f652eb56db0a8669a67d5b5344e094d086f9dab5a08",
            Some(268662),
        )
        .unwrap();
        let artifact = cache.acquire_sha512(&source).await.unwrap();
        let mods = managed.instance_paths(&instance).mods().to_path_buf();
        let recognized_path = mods.join("placeholder-api-local-copy.jar");
        std::fs::copy(&artifact.path, &recognized_path).unwrap();

        // An unknown local mod: real Fabric metadata, bytes Modrinth never
        // published.
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(br#"{"schemaVersion":1,"id":"aurora-private-acceptance","version":"1.0.0"}"#)
            .unwrap();
        let private: Vec<u8> = writer.finish().unwrap().into_inner();
        let unknown_path = mods.join("aurora-private-acceptance.jar");
        std::fs::write(&unknown_path, &private).unwrap();

        fn facts(path: &Path) -> (u64, String, String, Vec<u8>) {
            let bytes = std::fs::read(path).unwrap();
            let size = std::fs::metadata(path).unwrap().len();
            (
                size,
                format!("{:x}", sha2::Sha256::digest(&bytes)),
                format!("{:x}", sha2::Sha512::digest(&bytes)),
                bytes,
            )
        }
        let before_recognized = facts(&recognized_path);
        let before_unknown = facts(&unknown_path);

        // Register the instance so ownership evidence resolves normally.
        {
            use crate::instances::{
                InstanceRecord, InstanceRegistry, InstanceState, platform::InstalledConfiguration,
                settings::InstanceConfiguration,
            };
            let mut config = InstanceConfiguration::for_minecraft_version("1.21.11");
            config.set_aurora_enabled(false);
            let mut registry = InstanceRegistry::empty();
            registry.instances_mut().push(
                InstanceRecord::from_installed(
                    instance.clone(),
                    "Recognition acceptance",
                    InstanceState::Ready,
                    InstalledConfiguration {
                        minecraft_version: "1.21.11".into(),
                        platform: crate::instances::platform::PlatformPin::Fabric {
                            version: "0.19.5".into(),
                        },
                        aurora: None,
                    },
                    config,
                )
                .unwrap(),
            );
            std::fs::create_dir_all(managed.launcher_dir()).unwrap();
            registry.save(&managed.instance_registry_file()).unwrap();
        }

        let local = scan_local(&managed, &instance, ContentType::Mod).unwrap();
        let scan = recognize(
            &instance,
            ContentType::Mod,
            local,
            &crate::modrinth::Client::official(),
        )
        .await
        .unwrap();

        let recognized = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == "placeholder-api-local-copy.jar")
            .expect("recognized candidate")
            .clone();
        assert_eq!(recognized.status, RecognitionStatus::Recognized);
        assert_eq!(
            recognized.recognition.as_ref().unwrap().project_id,
            "eXts2L7r"
        );
        assert_eq!(recognized.filename_matches, Some(false));
        let unknown = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == "aurora-private-acceptance.jar")
            .expect("unknown candidate")
            .clone();
        assert_eq!(unknown.status, RecognitionStatus::Unrecognized);

        // Explicit approval adopts only the recognized file.
        let records = register_recovered_content(
            &managed,
            &instance,
            ContentType::Mod,
            &RecoveredContentApproval {
                instance_id: instance.to_string(),
                content_type: ContentType::Mod,
                inventory_revision: scan.inventory_revision.clone(),
                files: vec![RecoveredFileApproval {
                    file_name: recognized.file_name.clone(),
                    sha512: recognized.sha512.clone().unwrap(),
                }],
            },
            &crate::modrinth::Client::official(),
        )
        .await
        .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provider, "modrinth");
        assert_eq!(records[0].project_id, "eXts2L7r");
        assert_eq!(records[0].origin, ProviderOrigin::Recovered);

        // PATH IDENTICAL, SIZE IDENTICAL, SHA-256 IDENTICAL, SHA-512
        // IDENTICAL, FILE BYTES IDENTICAL.
        let after_recognized = facts(&recognized_path);
        assert_eq!(before_recognized.0, after_recognized.0);
        assert_eq!(before_recognized.1, after_recognized.1);
        assert_eq!(before_recognized.2, after_recognized.2);
        assert_eq!(before_recognized.3, after_recognized.3);
        assert_eq!(records[0].sha256, before_recognized.1);

        // The unknown file remains exactly as it was.
        let after_unknown = facts(&unknown_path);
        assert_eq!(before_unknown.3, after_unknown.3);

        // The ordinary inventory now proves provider ownership of the exact
        // untouched file, and the unknown file stays user-managed.
        let inventory = crate::instance_mods::scan(&managed, &instance).unwrap();
        let entry = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "placeholder-api-local-copy.jar")
            .unwrap();
        assert_eq!(
            entry.ownership,
            crate::instance_mods::ModOwnership::ProviderManaged
        );
        assert!(inventory.entries.iter().any(|entry| {
            entry.file_name == "aurora-private-acceptance.jar"
                && entry.ownership == crate::instance_mods::ModOwnership::UserManaged
        }));
    }
}
