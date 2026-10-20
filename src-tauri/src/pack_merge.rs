//! Merge one exact Modrinth pack version into an existing, ready Fabric
//! instance. The instance keeps its identity, configuration and Aurora
//! Client; the pack's files become ordinary managed content. This is
//! deliberately not pack ownership: pack-owned instances cannot contain
//! Aurora Client, and a merged instance does not follow pack updates.
//!
//! Safety rules: the pack's Minecraft version must equal the instance's and
//! the instance's Fabric Loader must be at least the pack's. Existing files are
//! never overwritten or adopted: byte-identical content is skipped, a differing
//! configuration override keeps the user's file, and a differing mod (or a
//! different version of an installed provider project) blocks the merge.
//! Planning is read-only; preview and install share it, and install re-runs
//! it and compares the fingerprint before any write.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::cache::{ArtifactCache, VerifiedArtifact};
use crate::downloads::Sha512ArtifactSource;
use crate::instance_content::{ContentState, ProviderInstallPlan, ProviderRecord};
use crate::instance_mods::ModOwnership;
use crate::instances::platform::PlatformPin;
use crate::instances::{InstanceId, InstanceRegistry, InstanceState};
use crate::integrity::{ArtifactDigest, Sha1Digest, verify_file, verify_file_sha1};
use crate::modpacks::{PackInstallError, PackPreview, Resolved};
use crate::mrpack::{OverrideFile, PackFile};
use crate::paths::ManagedPaths;

/// Modrinth project of Fabric API, which Aurora instances already provide as a
/// protected bootstrap mod. A pack's copy is skipped rather than duplicated.
const FABRIC_API_PROJECT: &str = "P7dR8mSH";

fn fail(code: &'static str, message: impl Into<String>) -> PackInstallError {
    PackInstallError {
        code,
        message: message.into(),
    }
}

/// What merging this exact pack version into the instance would do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackMergePreview {
    pub pack: PackPreview,
    pub instance_id: String,
    pub instance_minecraft_version: String,
    pub instance_fabric_loader_version: Option<String>,
    /// Files the merge adds (provider components and verified external files).
    pub added: Vec<String>,
    /// Pack files already present byte-for-byte (or provided by the instance).
    pub already_present: Vec<String>,
    /// Pack configuration overrides not applied because the user's file differs.
    pub kept_existing: Vec<String>,
    /// Configuration overrides the merge writes (none of these exist yet).
    pub overrides_written: Vec<String>,
    /// Reasons the merge cannot proceed. Install refuses while any remain.
    pub blockers: Vec<String>,
    pub fingerprint: String,
}

/// Outcome of a completed merge.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackMergeResult {
    pub instance_id: String,
    pub added: usize,
    pub already_present: usize,
    pub overrides_written: usize,
    pub kept_existing: Vec<String>,
}

struct MergePlan {
    preview: PackMergePreview,
    provider_plans: Vec<ProviderInstallPlan>,
    externals: Vec<PackFile>,
    overrides: Vec<OverrideFile>,
}

/// Compare dotted numeric versions (`0.19.5`); non-numeric parts compare as text.
fn version_at_least(installed: &str, required: &str) -> bool {
    let parts = |value: &str| -> Vec<String> {
        value
            .split(|c: char| c == '.' || c == '-' || c == '+')
            .map(str::to_owned)
            .collect()
    };
    let (a, b) = (parts(installed), parts(required));
    for index in 0..a.len().max(b.len()) {
        let left = a.get(index).map(String::as_str).unwrap_or("0");
        let right = b.get(index).map(String::as_str).unwrap_or("0");
        let ordering = match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(l), Ok(r)) => l.cmp(&r),
            _ => left.cmp(right),
        };
        match ordering {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    true
}

/// Read-only destination lookup. Never creates directories; a redirected or
/// file-occupied parent is reported as unsafe.
fn existing_destination(root: &Path, relative: &str) -> Result<Option<PathBuf>, PackInstallError> {
    crate::mrpack::destination_path(relative)
        .map_err(|_| fail("pack_invalid_path", "A pack file destination is unsafe."))?;
    let mut target = root.to_path_buf();
    let mut segments = relative.split('/').peekable();
    while let Some(part) = segments.next() {
        target.push(part);
        match std::fs::symlink_metadata(&target) {
            Ok(meta) if segments.peek().is_some() => {
                if !meta.is_dir() || meta.file_type().is_symlink() || reparse(&meta) {
                    return Err(fail(
                        "pack_invalid_path",
                        format!("{relative}: a parent directory is redirected or occupied."),
                    ));
                }
            }
            Ok(meta) => {
                if !meta.is_file() || meta.file_type().is_symlink() || reparse(&meta) {
                    return Err(fail(
                        "pack_invalid_path",
                        format!("{relative}: the destination is not a regular file."),
                    ));
                }
                return Ok(Some(target));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => {
                return Err(fail(
                    "pack_io_error",
                    format!("{relative}: the destination could not be inspected."),
                ));
            }
        }
    }
    Ok(None)
}

async fn plan(
    managed: &ManagedPaths,
    instance: &InstanceId,
    resolved: &Resolved,
) -> Result<MergePlan, PackInstallError> {
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?;
    let record = registry
        .find(instance)
        .ok_or_else(|| fail("instance_not_found", "The instance no longer exists."))?;
    let pack = &resolved.pack;
    let mut blockers = Vec::new();
    if record.state() != InstanceState::Ready {
        blockers.push("Finish installing this instance before adding a modpack.".to_owned());
    }
    if record.pack().is_some() {
        blockers.push(
            "This instance already follows a Modrinth modpack; use its pack update instead."
                .to_owned(),
        );
    }
    let installed = record.installed();
    let loader = match &installed.platform {
        PlatformPin::Fabric { version } => Some(version.clone()),
        _ => None,
    };
    if installed.minecraft_version != pack.minecraft_version {
        blockers.push(format!(
            "This pack is for Minecraft {}; this instance has Minecraft {}.",
            pack.minecraft_version, installed.minecraft_version
        ));
    }
    match &loader {
        None => blockers.push(
            "Modrinth modpacks need a Fabric instance; this instance uses another platform."
                .to_owned(),
        ),
        Some(version) if !version_at_least(version, &pack.fabric_loader_version) => {
            blockers.push(format!(
                "This pack needs Fabric Loader {} or newer; this instance has {}.",
                pack.fabric_loader_version, version
            ))
        }
        Some(_) => {}
    }
    if !blockers.is_empty() {
        // An incompatible instance is reported as such; its content is not
        // examined (a non-Fabric instance cannot answer mod-conflict queries).
        blockers.sort();
        let mut preview = PackMergePreview {
            pack: resolved.preview.clone(),
            instance_id: instance.to_string(),
            instance_minecraft_version: installed.minecraft_version.clone(),
            instance_fabric_loader_version: loader,
            added: Vec::new(),
            already_present: Vec::new(),
            kept_existing: Vec::new(),
            overrides_written: Vec::new(),
            blockers,
            fingerprint: String::new(),
        };
        preview.fingerprint = fingerprint_of(&preview)?;
        return Ok(MergePlan {
            preview,
            provider_plans: Vec::new(),
            externals: Vec::new(),
            overrides: Vec::new(),
        });
    }

    let root = managed.instance_paths(instance).root().to_path_buf();
    let state = ContentState::load(managed, instance)
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?;
    let mut added = Vec::new();
    let mut already_present = Vec::new();
    let mut kept_existing = Vec::new();
    let mut overrides_written = Vec::new();
    let mut provider_plans = Vec::new();
    let mut seen = HashSet::new();

    for candidate in &resolved.provider_plans {
        let path = format!(
            "{}/{}",
            candidate.content_type.directory_name(),
            candidate.file_name
        );
        if !seen.insert(path.to_lowercase()) {
            blockers.push(format!("{path}: the pack lists this destination twice."));
            continue;
        }
        if let Some(existing) = state.entries.iter().find(|entry| {
            entry.content_type == candidate.content_type
                && entry.provider == candidate.provider
                && entry.project_id == candidate.project_id
        }) {
            if existing.version_id == candidate.version_id {
                already_present.push(path);
            } else {
                blockers.push(format!(
                    "{}: a different version of this project is already installed ({}). Remove or update it first.",
                    candidate.file_name, existing.file_name
                ));
            }
            continue;
        }
        let conflicts = match crate::instance_content::preview_provider_conflicts(
            managed,
            instance,
            std::slice::from_ref(candidate),
        )
        .await
        {
            Ok(conflicts) => conflicts,
            // A pack may author a component Modrinth doesn't list for this exact
            // game/loader. A new pack instance trusts the pack's composition; an
            // existing instance keeps its own compatibility rule, so say why.
            Err(crate::instance_content::ContentError::UnsupportedAction) => {
                blockers.push(format!(
                    "{}: Modrinth doesn't list this file for Minecraft {} with Fabric, so it can't be added to an existing instance.",
                    candidate.file_name, installed.minecraft_version
                ));
                continue;
            }
            Err(error) => return Err(fail("pack_merge_unavailable", error.to_string())),
        };
        if conflicts.is_empty() {
            added.push(path);
            provider_plans.push(candidate.clone());
        } else if candidate.project_id == FABRIC_API_PROJECT
            && conflicts.iter().all(|conflict| {
                matches!(
                    conflict.ownership,
                    ModOwnership::LauncherBootstrap
                        | ModOwnership::LauncherManagedRequired
                        | ModOwnership::LauncherManagedRetained
                )
            })
        {
            // The instance's own protected Fabric API stays authoritative.
            already_present.push(path);
        } else {
            for conflict in conflicts {
                blockers.push(format!("{}: {}", candidate.file_name, conflict.reason));
            }
        }
    }

    let mut externals = Vec::new();
    for file in &resolved.unresolved {
        if !seen.insert(file.path.to_lowercase()) {
            blockers.push(format!(
                "{}: the pack lists this destination twice.",
                file.path
            ));
            continue;
        }
        match existing_destination(&root, &file.path) {
            Ok(None) => {
                added.push(file.path.clone());
                externals.push(file.clone());
            }
            Ok(Some(target)) => {
                let identical = Sha1Digest::parse(&file.sha1).is_ok_and(|sha1| {
                    verify_file_sha1(&target, &sha1, Some(file.file_size)).is_ok()
                });
                if identical {
                    already_present.push(file.path.clone());
                } else {
                    blockers.push(format!(
                        "{}: a different file already exists here and will not be overwritten.",
                        file.path
                    ));
                }
            }
            Err(error) => blockers.push(error.message),
        }
    }

    // The per-component check above compares each mod with what is installed.
    // The pack's own mods must also agree with each other (and external jars
    // with the instance), or install would fail after review said it is fine.
    if blockers.is_empty() {
        blockers
            .extend(batch_identity_blockers(managed, instance, &provider_plans, &externals).await?);
    }
    if provider_plans.len() > 64 {
        blockers.push(format!(
            "This pack adds {} Modrinth components; Aurora adds at most 64 to an existing instance at once.",
            provider_plans.len()
        ));
    }
    if blockers.is_empty() {
        // Exactly the dependency/compatibility validation install will run.
        if let Some(problem) = crate::instance_content::projected_merge_problem(
            managed,
            instance,
            provider_plans.clone(),
        )
        .await
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?
        {
            blockers.push(problem);
        }
    }

    let mut overrides = Vec::new();
    for item in &pack.overrides {
        if !seen.insert(item.path.to_lowercase()) {
            // A component already claims this destination; keep the component.
            continue;
        }
        match existing_destination(&root, &item.path) {
            Ok(None) => {
                overrides_written.push(item.path.clone());
                overrides.push(item.clone());
            }
            Ok(Some(target)) => {
                let identical = ArtifactDigest::parse(&item.sha256)
                    .is_ok_and(|digest| verify_file(&target, &digest, Some(item.size)).is_ok());
                if identical {
                    already_present.push(item.path.clone());
                } else if item.path.starts_with("mods/") {
                    blockers.push(format!(
                        "{}: a different mod file already exists here and will not be overwritten.",
                        item.path
                    ));
                } else {
                    kept_existing.push(item.path.clone());
                }
            }
            Err(error) => blockers.push(error.message),
        }
    }

    for list in [
        &mut added,
        &mut already_present,
        &mut kept_existing,
        &mut overrides_written,
        &mut blockers,
    ] {
        list.sort();
    }
    let mut preview = PackMergePreview {
        pack: resolved.preview.clone(),
        instance_id: instance.to_string(),
        instance_minecraft_version: installed.minecraft_version.clone(),
        instance_fabric_loader_version: loader,
        added,
        already_present,
        kept_existing,
        overrides_written,
        blockers,
        fingerprint: String::new(),
    };
    preview.fingerprint = fingerprint_of(&preview)?;
    Ok(MergePlan {
        preview,
        provider_plans,
        externals,
        overrides: overrides.into_iter().collect(),
    })
}

/// Mod-identity agreement across the batch, using the same rule activation
/// enforces (`conflicting_mod_identity`). Artifacts come from the verified
/// cache (already populated by the per-component checks); nothing is written.
async fn batch_identity_blockers(
    managed: &ManagedPaths,
    instance: &InstanceId,
    provider_plans: &[ProviderInstallPlan],
    externals: &[PackFile],
) -> Result<Vec<String>, PackInstallError> {
    use crate::instance_content::ProviderArtifactSource;
    let cache = ArtifactCache::new(managed.clone());
    let kind = crate::instance_mods::platform_kind_of(managed, instance);
    let mut batch: Vec<(String, crate::instance_mods::ModMetadata)> = Vec::new();
    for plan in provider_plans
        .iter()
        .filter(|plan| plan.content_type == crate::instance_content::ContentType::Mod)
    {
        let artifact = match &plan.source {
            ProviderArtifactSource::Sha256(source) => cache.acquire(source).await,
            ProviderArtifactSource::Sha512(source) => cache.acquire_sha512(source).await,
        }
        .map_err(|e| fail("pack_download_failed", e.to_string()))?;
        if let (Some(metadata), _) =
            crate::instance_mods::inspect_mod_metadata(&artifact.path, artifact.bytes, &kind)
        {
            batch.push((plan.file_name.clone(), metadata));
        }
    }
    let mut external_metadata = Vec::new();
    for file in externals
        .iter()
        .filter(|file| file.path.starts_with("mods/"))
    {
        let source = Sha512ArtifactSource::mrpack_fallback(
            &file.downloads[0],
            &file.sha512,
            (file.file_size > 0).then_some(file.file_size),
        )
        .map_err(|_| {
            fail(
                "pack_invalid_download",
                "An unresolved pack file has an invalid source.",
            )
        })?;
        let artifact = cache
            .acquire_sha512(&source)
            .await
            .map_err(|e| fail("pack_download_failed", e.to_string()))?;
        crate::modpacks::verify_external(file, &artifact)?;
        if let (Some(metadata), _) =
            crate::instance_mods::inspect_mod_metadata(&artifact.path, artifact.bytes, &kind)
        {
            external_metadata.push((file.path.clone(), metadata));
        }
    }
    let mut blockers = Vec::new();
    // External jars against what is installed now.
    if !external_metadata.is_empty() {
        let inventory = crate::instance_mods::scan(managed, instance)
            .map_err(|e| fail("pack_merge_unavailable", e.to_string()))?;
        for (path, incoming) in &external_metadata {
            for entry in &inventory.entries {
                if let Some(existing) = &entry.metadata {
                    if let Some(id) =
                        crate::instance_content::conflicting_mod_identity(incoming, existing)
                    {
                        blockers.push(format!(
                            "{path}: mod {id} would duplicate {} already in this instance.",
                            entry.file_name
                        ));
                    }
                }
            }
        }
    }
    batch.extend(external_metadata);
    for (index, (name, incoming)) in batch.iter().enumerate() {
        for (other_name, existing) in batch.iter().skip(index + 1) {
            if let Some(id) = crate::instance_content::conflicting_mod_identity(incoming, existing)
                .or_else(|| crate::instance_content::conflicting_mod_identity(existing, incoming))
            {
                blockers.push(format!(
                    "{name} and {other_name} both include mod {id} at different versions. Aurora won't install two copies of one mod."
                ));
            }
        }
    }
    Ok(blockers)
}

/// Fingerprint over the whole preview (computed with an empty fingerprint
/// field), so install can prove it is applying exactly what was reviewed.
fn fingerprint_of(preview: &PackMergePreview) -> Result<String, PackInstallError> {
    let bytes = serde_json::to_vec(preview).map_err(|_| {
        fail(
            "pack_plan_invalid",
            "The merge preview could not be encoded.",
        )
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub async fn preview(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    instance: &InstanceId,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
) -> Result<PackMergePreview, PackInstallError> {
    let resolved =
        crate::modpacks::resolve(managed, provider, project_id, version_id, selected_optional)
            .await?;
    Ok(plan(managed, instance, &resolved).await?.preview)
}

fn merges_in_progress() -> &'static Mutex<HashSet<String>> {
    static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashSet::new()))
}

struct MergeGuard(String);
impl Drop for MergeGuard {
    fn drop(&mut self) {
        merges_in_progress()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.0);
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn install(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    instance: &InstanceId,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
    fingerprint: &str,
    progress: &mut (dyn FnMut(&'static str) + Send),
) -> Result<PackMergeResult, PackInstallError> {
    if !merges_in_progress()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(instance.to_string())
    {
        return Err(fail(
            "pack_merge_in_progress",
            "A modpack is already being added to this instance.",
        ));
    }
    let _guard = MergeGuard(instance.to_string());

    progress("resolvingPack");
    let resolved =
        crate::modpacks::resolve(managed, provider, project_id, version_id, selected_optional)
            .await?;
    let revision = crate::instance_content::local_inventory_revision(managed, instance)
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?;
    let merge = plan(managed, instance, &resolved).await?;
    if merge.preview.fingerprint != fingerprint {
        return Err(fail(
            "pack_preview_stale",
            "This instance or the pack changed since review. Review it again before adding it.",
        ));
    }
    if let Some(blocker) = merge.preview.blockers.first() {
        return Err(fail("pack_merge_blocked", blocker.clone()));
    }

    progress("downloading");
    let cache = ArtifactCache::new(managed.clone());
    let mut external: Vec<(PackFile, VerifiedArtifact)> = Vec::new();
    for file in &merge.externals {
        let source = Sha512ArtifactSource::mrpack_fallback(
            &file.downloads[0],
            &file.sha512,
            (file.file_size > 0).then_some(file.file_size),
        )
        .map_err(|_| {
            fail(
                "pack_invalid_download",
                "An unresolved pack file has an invalid source.",
            )
        })?;
        let artifact = cache
            .acquire_sha512(&source)
            .await
            .map_err(|e| fail("pack_download_failed", e.to_string()))?;
        crate::modpacks::verify_external(file, &artifact)?;
        external.push((file.clone(), artifact));
    }

    progress("installingComponents");
    let records: Vec<ProviderRecord> = if merge.provider_plans.is_empty() {
        Vec::new()
    } else {
        crate::instance_content::install_merged_provider_plans(
            managed,
            instance,
            merge.provider_plans,
            &revision,
        )
        .await
        .map_err(|error| fail("pack_component_install_failed", error.to_string()))?
    };

    progress("installingOverrides");
    let root = managed.instance_paths(instance).root().to_path_buf();
    let mut created: Vec<(PathBuf, String)> = Vec::new();
    let written = (|| -> Result<(), PackInstallError> {
        for (file, artifact) in &external {
            let digest = artifact.sha256.as_hex();
            let target = crate::modpacks::materialize(&root, &file.path, &artifact.path, &digest)?;
            created.push((target, digest));
        }
        for item in &merge.overrides {
            let bytes = crate::mrpack::read_override(&resolved.archive.path, item)
                .map_err(|e| fail(e.code(), e.to_string()))?;
            let target =
                crate::modpacks::materialize_bytes(&root, &item.path, &bytes, &item.sha256)?;
            created.push((target, item.sha256.clone()));
        }
        Ok(())
    })();
    if let Err(error) = written {
        return Err(rollback(
            managed, instance, &root, &created, &records, error,
        ));
    }

    progress("complete");
    Ok(PackMergeResult {
        instance_id: instance.to_string(),
        added: merge.preview.added.len(),
        already_present: merge.preview.already_present.len(),
        overrides_written: merge.preview.overrides_written.len(),
        kept_existing: merge.preview.kept_existing,
    })
}

/// Remove only what this merge created, each proven by its digest. The
/// instance record, game tree and all pre-existing files are never touched.
fn rollback(
    managed: &ManagedPaths,
    instance: &InstanceId,
    root: &Path,
    created: &[(PathBuf, String)],
    records: &[ProviderRecord],
    original: PackInstallError,
) -> PackInstallError {
    let mut clean = true;
    for (path, digest) in created.iter().rev() {
        let proven = path.starts_with(root)
            && path != root
            && ArtifactDigest::parse(digest)
                .is_ok_and(|expected| verify_file(path, &expected, None).is_ok());
        if !proven || std::fs::remove_file(path).is_err() {
            clean = false;
        }
    }
    if !records.is_empty()
        && crate::instance_content::remove_merged_provider_records(managed, instance, records)
            .is_err()
    {
        clean = false;
    }
    if clean {
        original
    } else {
        fail(
            "pack_merge_rollback_incomplete",
            format!(
                "{} Some files this merge added could not be removed safely; review the instance's Mods tab.",
                original.message
            ),
        )
    }
}

#[cfg(windows)]
fn reparse(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse(_meta: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_comparison_accepts_equal_and_newer_only() {
        assert!(version_at_least("0.19.5", "0.19.5"));
        assert!(version_at_least("0.19.5", "0.16.14"));
        assert!(version_at_least("0.19.10", "0.19.9"));
        assert!(!version_at_least("0.16.14", "0.19.5"));
        assert!(!version_at_least("0.19", "0.19.1"));
        assert!(version_at_least("1.0", "0.99.99"));
    }

    #[test]
    fn destination_lookup_is_read_only_and_rejects_occupied_parents() {
        let root = std::env::temp_dir().join(format!("aurora-merge-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        // Missing parent: reported absent and never created.
        assert!(
            existing_destination(&root, "config/example.json")
                .unwrap()
                .is_none()
        );
        assert!(!root.join("config").exists());
        std::fs::write(root.join("options.txt"), b"user").unwrap();
        assert_eq!(
            existing_destination(&root, "options.txt").unwrap(),
            Some(root.join("options.txt"))
        );
        std::fs::write(root.join("config"), b"not a directory").unwrap();
        assert!(existing_destination(&root, "config/example.json").is_err());
        assert!(existing_destination(&root, "../escape.txt").is_err());
        std::fs::remove_file(root.join("options.txt")).unwrap();
        std::fs::remove_file(root.join("config")).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}
