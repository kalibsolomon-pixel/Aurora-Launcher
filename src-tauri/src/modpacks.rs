//! Exact Modrinth pack planning and installation. Provider identity is
//! recovered before installation; unresolved files stay explicitly external.
//! A fresh instance remains Installing until all owned bytes and their state
//! validate, so an interrupted operation never becomes launchable.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::cache::{ArtifactCache, VerifiedArtifact};
use crate::downloads::Sha512ArtifactSource;
use crate::instance_content::{ProviderArtifactSource, ProviderInstallPlan};
use crate::instances::InstanceRecord;
use crate::instances::lifecycle::{CreateInstanceRequest, InstanceEndpoints, InstanceProgress};
use crate::instances::settings::{
    DEFAULT_MEMORY_MIB, InstanceConfiguration, LoaderConfiguration, LoaderPolicy,
};
use crate::integrity::{ArtifactDigest, Sha1Digest, verify_file, verify_file_sha1};
use crate::mrpack::{PackFile, PackPlan};
use crate::pack_state::{InstalledPack, OwnedComponent, OwnedOverride, PackIdentity};
use crate::paths::ManagedPaths;

#[derive(Debug)]
pub struct PackInstallError {
    pub code: &'static str,
    pub message: String,
}
impl fmt::Display for PackInstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for PackInstallError {}
fn fail(code: &'static str, message: impl Into<String>) -> PackInstallError {
    PackInstallError {
        code,
        message: message.into(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackPreview {
    pub provider: &'static str,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub pack_version: String,
    pub minecraft_version: String,
    pub fabric_loader_version: String,
    pub archive_sha512: String,
    pub recognized: Vec<RecognizedPreview>,
    pub unresolved: Vec<String>,
    pub optional: Vec<String>,
    pub excluded: Vec<String>,
    pub overrides: Vec<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognizedPreview {
    pub path: String,
    pub project_id: String,
    pub version_id: String,
    pub title: String,
}

pub(crate) struct Resolved {
    pub(crate) archive: VerifiedArtifact,
    pub(crate) archive_sha512: String,
    pub(crate) provider_project_id: String,
    pub(crate) provider_version_id: String,
    pub(crate) pack: PackPlan,
    pub(crate) provider_plans: Vec<ProviderInstallPlan>,
    pub(crate) unresolved: Vec<PackFile>,
    pub(crate) preview: PackPreview,
}

pub async fn preview(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
) -> Result<PackPreview, PackInstallError> {
    Ok(
        resolve(managed, provider, project_id, version_id, selected_optional)
            .await?
            .preview,
    )
}

/// Resolve one exact pack version into its verified archive, parsed plan,
/// provider component plans, and unresolved external files. Shared by the
/// Phase I install path and the Phase J update planner; re-resolving is the
/// staleness check, so callers never trust a previously rendered snapshot.
pub(crate) async fn resolve(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
) -> Result<Resolved, PackInstallError> {
    let artifact = provider
        .pack_artifact(project_id, version_id)
        .await
        .map_err(|e| fail(e.code(), e.to_string()))?;
    if artifact
        .source
        .size_bytes()
        .is_some_and(|size| size > crate::mrpack::MAX_PACK_BYTES)
    {
        return Err(fail(
            "pack_archive_limit",
            "The selected pack exceeds Aurora's 512 MiB archive limit.",
        ));
    }
    let archive = ArtifactCache::new(managed.clone())
        .acquire_sha512(&artifact.source)
        .await
        .map_err(|e| fail("pack_download_failed", e.to_string()))?;
    let pack = crate::mrpack::parse_verified_file(&archive.path)
        .map_err(|e| fail(e.code(), e.to_string()))?;
    if !artifact
        .game_versions
        .iter()
        .any(|v| v == &pack.minecraft_version)
        || !artifact.loaders.iter().any(|v| v == "fabric")
    {
        return Err(fail(
            "pack_identity_conflict",
            "The pack index does not agree with this Modrinth version's Minecraft and Fabric metadata.",
        ));
    }
    let optional_set: HashSet<_> = selected_optional.iter().cloned().collect();
    if optional_set.len() != selected_optional.len()
        || !optional_set
            .iter()
            .all(|path| pack.optional_files.iter().any(|f| &f.path == path))
    {
        return Err(fail(
            "pack_optional_invalid",
            "The optional file selection does not match the selected pack version.",
        ));
    }
    let files: Vec<&PackFile> = pack
        .required_files
        .iter()
        .chain(
            pack.optional_files
                .iter()
                .filter(|f| optional_set.contains(&f.path)),
        )
        .collect();
    let hashes: Vec<_> = files.iter().map(|file| file.sha512.clone()).collect();
    let recognized = provider
        .lookup_files(&hashes)
        .await
        .map_err(|e| fail(e.code(), e.to_string()))?;
    let mut identities = BTreeMap::new();
    for item in recognized {
        if identities
            .insert(item.queried_sha512.clone(), item)
            .is_some()
        {
            return Err(fail(
                "pack_identity_conflict",
                "Modrinth returned more than one identity for a pack file hash.",
            ));
        }
    }
    let mut provider_plans = Vec::new();
    let mut provider_preview = Vec::new();
    let mut unresolved = Vec::new();
    let mut project_set = HashSet::new();
    for file in files {
        if let Some(item) = identities.get(&file.sha512) {
            let planned = provider
                .pack_component_plan(item, &file.path, file.file_size, &pack.minecraft_version)
                .await
                .map_err(|e| {
                    if matches!(e, crate::modrinth::Error::DependencyConflict) {
                        fail(
                            "pack_component_incompatible",
                            format!(
                                "{}: Modrinth marks this file incompatible with a client Fabric installation.",
                                file.path
                            ),
                        )
                    } else {
                        fail("pack_identity_conflict", format!("{}: {e}", file.path))
                    }
                })?;
            project_set.insert(planned.project_id.clone());
            provider_preview.push(RecognizedPreview {
                path: file.path.clone(),
                project_id: planned.project_id.clone(),
                version_id: planned.version_id.clone(),
                title: item.version_name.clone(),
            });
            provider_plans.push(planned);
        } else {
            unresolved.push(file.clone());
        }
    }
    for planned in &provider_plans {
        for dependency in &planned.dependencies {
            match dependency.kind {
                crate::instance_content::DependencyKind::Required => {
                    if !provider_plans.iter().any(|candidate| {
                        candidate.project_id == dependency.project_id
                            && dependency
                                .version_id
                                .as_ref()
                                .is_none_or(|exact| exact == &candidate.version_id)
                    }) {
                        return Err(fail(
                            "pack_dependency_conflict",
                            format!(
                                "{} requires a Modrinth dependency absent from this exact pack snapshot.",
                                planned.file_name
                            ),
                        ));
                    }
                }
                crate::instance_content::DependencyKind::Incompatible => {
                    if project_set.contains(&dependency.project_id) {
                        return Err(fail(
                            "pack_dependency_conflict",
                            format!(
                                "{} conflicts with another pack component.",
                                planned.file_name
                            ),
                        ));
                    }
                }
                crate::instance_content::DependencyKind::Optional => {}
            }
        }
    }
    let mut excluded = pack.excluded_paths.clone();
    excluded.extend(
        pack.optional_files
            .iter()
            .filter(|f| !optional_set.contains(&f.path))
            .map(|f| f.path.clone()),
    );
    excluded.sort();
    let mut preview = PackPreview {
        provider: "modrinth",
        project_id: artifact.project_id.clone(),
        version_id: artifact.version_id.clone(),
        name: pack.name.clone(),
        pack_version: pack.pack_version.clone(),
        minecraft_version: pack.minecraft_version.clone(),
        fabric_loader_version: pack.fabric_loader_version.clone(),
        archive_sha512: artifact.sha512.clone(),
        recognized: provider_preview,
        unresolved: unresolved.iter().map(|f| f.path.clone()).collect(),
        optional: pack.optional_files.iter().map(|f| f.path.clone()).collect(),
        excluded,
        overrides: pack.overrides.iter().map(|f| f.path.clone()).collect(),
        fingerprint: String::new(),
    };
    let bytes = serde_json::to_vec(&preview).map_err(|_| {
        fail(
            "pack_plan_invalid",
            "The pack preview could not be encoded.",
        )
    })?;
    preview.fingerprint = format!("{:x}", Sha256::digest(bytes));
    Ok(Resolved {
        archive,
        archive_sha512: artifact.sha512,
        provider_project_id: artifact.project_id,
        provider_version_id: artifact.version_id,
        pack,
        provider_plans,
        unresolved,
        preview,
    })
}

struct AcquiredExternal {
    file: PackFile,
    artifact: VerifiedArtifact,
}

/// Re-resolves provider authority and rejects stale previews before creating
/// any instance. Downloads complete in the shared verified cache first.
pub async fn install(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    provider: &crate::modrinth::Client,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
    fingerprint: &str,
    progress: &mut (dyn FnMut(&'static str) + Send),
) -> Result<InstanceRecord, PackInstallError> {
    progress("resolvingPack");
    let resolved = resolve(managed, provider, project_id, version_id, selected_optional).await?;
    if fingerprint != resolved.preview.fingerprint {
        return Err(fail(
            "pack_preview_stale",
            "The pack preview changed. Review this exact version again before installing.",
        ));
    }
    let cache = ArtifactCache::new(managed.clone());
    progress("downloading");
    let mut external = Vec::new();
    for file in &resolved.unresolved {
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
        verify_external(file, &artifact)?;
        external.push(AcquiredExternal {
            file: file.clone(),
            artifact,
        });
    }
    for planned in &resolved.provider_plans {
        let artifact = match &planned.source {
            ProviderArtifactSource::Sha512(source) => cache.acquire_sha512(source).await,
            ProviderArtifactSource::Sha256(source) => cache.acquire(source).await,
        }
        .map_err(|e| fail("pack_download_failed", e.to_string()))?;
        let file = resolved
            .pack
            .required_files
            .iter()
            .chain(resolved.pack.optional_files.iter())
            .find(|file| {
                file.path
                    == format!(
                        "{}/{}",
                        planned.content_type.directory_name(),
                        planned.file_name
                    )
            })
            .ok_or_else(|| fail("pack_plan_invalid", "A recognized file left the pack plan."))?;
        verify_external(file, &artifact)?;
    }
    progress("preparingInstance");
    let mut configuration = InstanceConfiguration::from_parts(
        &resolved.pack.minecraft_version,
        LoaderConfiguration::Fabric {
            policy: LoaderPolicy::Pinned {
                version: resolved.pack.fabric_loader_version.clone(),
            },
        },
        DEFAULT_MEMORY_MIB,
        String::new(),
        None,
    );
    configuration.set_aurora_enabled(false);
    let mut display_name = resolved.pack.name.clone();
    while display_name.len() > 80 {
        display_name.pop();
    }
    let mut game_progress = |_event: InstanceProgress| {
        progress("installingGame");
    };
    let record = crate::instances::lifecycle::begin_pack_instance(
        managed,
        &managed.instance_registry_file(),
        endpoints,
        CreateInstanceRequest::new(&display_name, configuration),
        crate::instances::PackRegistryIdentity {
            provider: "modrinth".into(),
            project_id: resolved.provider_project_id.clone(),
            version_id: resolved.provider_version_id.clone(),
            name: resolved.pack.name.clone(),
            pack_version: resolved.pack.pack_version.clone(),
        },
        &mut game_progress,
    )
    .await
    .map_err(|e| fail("pack_instance_failed", e.to_string()))?;
    let root = managed.instance_paths(record.id()).root().to_path_buf();
    let mut created: Vec<(PathBuf, String)> = Vec::new();
    let result = (|| -> Result<InstalledPack, PackInstallError> {
        progress("installingOverrides");
        let mut components = Vec::new();
        for item in &external {
            let target = materialize(
                &root,
                &item.file.path,
                &item.artifact.path,
                &item.artifact.sha256.as_hex(),
            )?;
            created.push((target, item.artifact.sha256.as_hex()));
            components.push(OwnedComponent {
                path: item.file.path.clone(),
                sha256: item.artifact.sha256.as_hex(),
                sha512: item.file.sha512.clone(),
                provider: None,
                provider_version_id: None,
            });
        }
        let mut overrides = Vec::new();
        for item in &resolved.pack.overrides {
            let bytes = crate::mrpack::read_override(&resolved.archive.path, item)
                .map_err(|e| fail(e.code(), e.to_string()))?;
            let target = materialize_bytes(&root, &item.path, &bytes, &item.sha256)?;
            created.push((target, item.sha256.clone()));
            overrides.push(OwnedOverride {
                path: item.path.clone(),
                sha256: item.sha256.clone(),
            });
        }
        Ok(InstalledPack::new(
            record.id().clone(),
            PackIdentity {
                provider: "modrinth".into(),
                project_id: resolved.provider_project_id.clone(),
                version_id: resolved.provider_version_id.clone(),
                name: resolved.pack.name.clone(),
                pack_version: resolved.pack.pack_version.clone(),
                artifact_sha512: resolved.archive_sha512.clone(),
                artifact_sha256: resolved.archive.sha256.as_hex(),
                minecraft_version: resolved.pack.minecraft_version.clone(),
                fabric_loader_version: resolved.pack.fabric_loader_version.clone(),
                installed_at_unix_seconds: crate::instance_content::now_unix_seconds(),
            },
            components,
            overrides,
            resolved.preview.excluded.clone(),
        )
        .map_err(|e| fail(e.code(), e.to_string()))?)
    })();
    let mut state = match result {
        Ok(value) => value,
        Err(error) => {
            return Err(rollback_pack_install(
                managed,
                &record,
                &created,
                &[],
                error,
            ));
        }
    };
    progress("installingComponents");
    let provider_records = match crate::instance_content::install_pack_provider_plans(
        managed,
        record.id(),
        resolved.provider_plans,
    )
    .await
    {
        Ok(records) => records,
        Err(error) => {
            return Err(rollback_pack_install(
                managed,
                &record,
                &created,
                &[],
                fail("pack_component_install_failed", error.to_string()),
            ));
        }
    };
    for record in &provider_records {
        let path = format!(
            "{}/{}",
            record.content_type.directory_name(),
            record.file_name
        );
        let file = resolved
            .pack
            .required_files
            .iter()
            .chain(resolved.pack.optional_files.iter())
            .find(|file| file.path == path)
            .ok_or_else(|| fail("pack_plan_invalid", "A provider record left the pack plan."))?;
        state.components.push(OwnedComponent {
            path,
            sha256: record.sha256.clone(),
            sha512: file.sha512.clone(),
            provider: Some(record.identity()),
            provider_version_id: Some(record.version_id.clone()),
        });
    }
    progress("validating");
    if let Err(error) = state.save(managed) {
        return Err(rollback_pack_install(
            managed,
            &record,
            &created,
            &provider_records,
            fail(error.code(), error.to_string()),
        ));
    }
    if let Err(error) = state.validate_installed(managed) {
        return Err(rollback_pack_install(
            managed,
            &record,
            &created,
            &provider_records,
            fail(error.code(), error.to_string()),
        ));
    }
    let record = match crate::instances::lifecycle::complete_pack_instance(
        managed,
        &managed.instance_registry_file(),
        &managed.config_file(),
        record.id(),
    ) {
        Ok(record) => record,
        Err(error) => {
            return Err(rollback_pack_install(
                managed,
                &record,
                &created,
                &provider_records,
                fail("pack_validation_failed", error.to_string()),
            ));
        }
    };
    progress("complete");
    Ok(record)
}

pub(crate) fn verify_external(
    file: &PackFile,
    artifact: &VerifiedArtifact,
) -> Result<(), PackInstallError> {
    if artifact.bytes != file.file_size {
        return Err(fail(
            "pack_digest_mismatch",
            "A pack file has a different size than the pack declared.",
        ));
    }
    let sha1 = Sha1Digest::parse(&file.sha1)
        .map_err(|_| fail("pack_invalid_hash", "A pack file has an invalid SHA-1."))?;
    verify_file_sha1(&artifact.path, &sha1, Some(file.file_size)).map_err(|_| {
        fail(
            "pack_digest_mismatch",
            "A pack file failed the pack's SHA-1 verification.",
        )
    })?;
    Ok(())
}

pub(crate) fn materialize(
    root: &Path,
    relative: &str,
    source: &Path,
    digest: &str,
) -> Result<PathBuf, PackInstallError> {
    let target = target_for(root, relative)?;
    let temporary = target.with_extension(format!("pack-staged-{}", uuid::Uuid::new_v4()));
    if std::fs::copy(source, &temporary).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(fail(
            "pack_io_error",
            "A verified pack artifact could not be staged.",
        ));
    }
    activate_temporary(temporary, target, digest)
}

pub(crate) fn materialize_bytes(
    root: &Path,
    relative: &str,
    bytes: &[u8],
    digest: &str,
) -> Result<PathBuf, PackInstallError> {
    let target = target_for(root, relative)?;
    let temporary = target.with_extension(format!("pack-staged-{}", uuid::Uuid::new_v4()));
    if std::fs::write(&temporary, bytes).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(fail(
            "pack_io_error",
            "A pack override could not be staged.",
        ));
    }
    activate_temporary(temporary, target, digest)
}

pub(crate) fn target_for(root: &Path, relative: &str) -> Result<PathBuf, PackInstallError> {
    crate::mrpack::destination_path(relative)
        .map_err(|_| fail("pack_invalid_path", "A pack file destination is unsafe."))?;
    let mut target = root.to_path_buf();
    let mut segments = relative.split('/').peekable();
    while let Some(part) = segments.next() {
        target.push(part);
        if segments.peek().is_some() {
            match std::fs::symlink_metadata(&target) {
                Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() && !reparse(&meta) => {}
                Ok(_) => {
                    return Err(fail(
                        "pack_invalid_path",
                        "A pack directory is redirected or occupied.",
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::create_dir(&target).map_err(|_| {
                        fail("pack_io_error", "A pack directory could not be created.")
                    })?
                }
                Err(_) => {
                    return Err(fail(
                        "pack_io_error",
                        "A pack directory could not be inspected.",
                    ));
                }
            }
        }
    }
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(fail(
            "pack_path_collision",
            "A pack file collides with existing instance content.",
        ));
    }
    Ok(target)
}

pub(crate) fn activate_temporary(
    temporary: PathBuf,
    target: PathBuf,
    digest: &str,
) -> Result<PathBuf, PackInstallError> {
    let expected = ArtifactDigest::parse(digest)
        .map_err(|_| fail("pack_invalid_hash", "The pack file digest is invalid."))?;
    if verify_file(&temporary, &expected, None).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(fail(
            "pack_digest_mismatch",
            "A staged pack file failed verification.",
        ));
    }
    if let Err(error) = std::fs::hard_link(&temporary, &target) {
        let _ = std::fs::remove_file(&temporary);
        return Err(fail(
            if target.exists() {
                "pack_path_collision"
            } else {
                "pack_io_error"
            },
            format!("A pack file could not be activated: {}", error.kind()),
        ));
    }
    let _ = std::fs::remove_file(&temporary);
    Ok(target)
}

fn rollback_pack_install(
    managed: &ManagedPaths,
    record: &InstanceRecord,
    created: &[(PathBuf, String)],
    provider_records: &[crate::instance_content::ProviderRecord],
    original: PackInstallError,
) -> PackInstallError {
    let paths = managed.instance_paths(record.id());
    let root = paths.root();
    let mut owned = created.to_vec();
    owned.extend(provider_records.iter().map(|provider| {
        (
            root.join(provider.content_type.directory_name())
                .join(&provider.file_name),
            provider.sha256.clone(),
        )
    }));
    for (path, digest) in owned.iter().rev() {
        if !path.starts_with(root) || path == root {
            return rollback_failure();
        }
        let Ok(expected) = ArtifactDigest::parse(digest) else {
            return rollback_failure();
        };
        if verify_file(path, &expected, None).is_err() || std::fs::remove_file(path).is_err() {
            return rollback_failure();
        }
    }
    let pack_path = root.join("pack-installed.json");
    if pack_path.exists() {
        if InstalledPack::load(managed, record.id())
            .ok()
            .flatten()
            .is_none()
            || std::fs::remove_file(pack_path).is_err()
        {
            return rollback_failure();
        }
    }
    let content_path = root.join("content-managed.json");
    if content_path.exists() {
        let Ok(state) = crate::instance_content::ContentState::load(managed, record.id()) else {
            return rollback_failure();
        };
        if state.entries.len() != provider_records.len()
            || std::fs::remove_file(content_path).is_err()
        {
            return rollback_failure();
        }
    }
    for (path, _) in owned {
        let mut parent = path.parent();
        while let Some(dir) = parent.filter(|dir| *dir != root && dir.starts_with(root)) {
            if std::fs::remove_dir(dir).is_err() {
                break;
            }
            parent = dir.parent();
        }
    }
    // The game tree is the installer's manifest-proven managed directory.
    // User content outside it is never recursively removed.
    match crate::install::validate_installed_game(managed, record.id()) {
        Ok(crate::install::ValidationOutcome::Installed(validation))
            if validation.status == crate::install::ValidationStatus::Valid
                && validation.minecraft_version == record.configuration().minecraft_version() => {}
        _ => return rollback_failure(),
    }
    if std::fs::remove_dir_all(paths.game()).is_err() {
        return rollback_failure();
    }
    for directory in [
        paths.mods(),
        paths.resourcepacks(),
        paths.shaderpacks(),
        paths.config(),
    ] {
        let _ = std::fs::remove_dir(directory);
    }
    if std::fs::remove_dir(root).is_err() {
        return rollback_failure();
    }
    if crate::instances::lifecycle::abandon_pack_instance(
        &managed.instance_registry_file(),
        record.id(),
    )
    .is_err()
    {
        return rollback_failure();
    }
    original
}

fn rollback_failure() -> PackInstallError {
    fail(
        "pack_rollback_failed",
        "Pack installation failed and its exact new instance could not be cleaned up safely. The instance remains unavailable for launch and needs inspection.",
    )
}

#[cfg(windows)]
fn reparse(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse(_: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestRequest, TestResponse, TestServer};
    use sha1::Sha1;
    use sha2::Sha512;
    use std::io::{Cursor, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn archive(index: &str) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            zip.start_file(
                "modrinth.index.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(index.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[tokio::test]
    async fn preview_recovers_no_fake_provider_identity_for_an_unresolved_file() {
        let payload = b"unresolved pack component";
        let sha1 = format!("{:x}", Sha1::digest(payload));
        let sha512 = format!("{:x}", Sha512::digest(payload));
        let index = serde_json::json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "pack-authored-1", "name": "Fixture Pack",
            "files": [{ "path": "mods/external.jar", "hashes": {"sha1": sha1, "sha512": sha512},
                "downloads": ["https://github.com/example/pack/releases/download/1/external.jar"], "fileSize": payload.len(),
                "env": {"client": "required", "server": "unsupported"} }],
            "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.0"}
        });
        let bytes = archive(&index.to_string());
        let mut changed_index = index;
        changed_index["name"] = serde_json::json!("Revised Fixture Pack");
        let changed_bytes = archive(&changed_index.to_string());
        let changed = Arc::new(AtomicBool::new(false));
        let serving_changed = Arc::clone(&changed);
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let path = request.path.split('?').next().unwrap_or_default();
            let archive_bytes = if serving_changed.load(Ordering::SeqCst) {
                &changed_bytes
            } else {
                &bytes
            };
            match path {
                "/pack.mrpack" => TestResponse::ok(archive_bytes),
                "/v2/project/PACK0001" => TestResponse::ok(serde_json::json!({
                    "id": "PACK0001", "project_type": "modpack", "title": "Fixture Pack", "description": "Fixture",
                    "license": {"id":"mit"}, "game_versions": ["1.21.1"], "loaders": ["fabric"], "environment": ["client_only"]
                }).to_string().as_bytes()),
                "/v2/version/VERS0001" => TestResponse::ok(serde_json::json!({
                    "id": "VERS0001", "project_id": "PACK0001", "name": "One", "version_number": "1",
                    "version_type": "release", "date_published": "2026-01-01T00:00:00Z", "game_versions": ["1.21.1"],
                    "loaders": ["fabric"], "environment": "client_only", "dependencies": [],
                    "files": [{"hashes": {"sha512": format!("{:x}", Sha512::digest(archive_bytes))}, "url": format!("{}/pack.mrpack", request.base_url),
                        "filename": "pack.mrpack", "primary": true, "size": archive_bytes.len()}]
                }).to_string().as_bytes()),
                _ => TestResponse::status(404),
            }
        }));
        let root =
            std::env::temp_dir().join(format!("aurora-pack-preview-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let client = crate::modrinth::Client::for_testing(&format!("{}/v2/", server.base_url()));
        let result = preview(&managed, &client, "PACK0001", "VERS0001", &[])
            .await
            .unwrap();
        assert_eq!(result.version_id, "VERS0001");
        assert_eq!(result.minecraft_version, "1.21.1");
        assert_eq!(result.unresolved, ["mods/external.jar"]);
        assert!(result.recognized.is_empty());
        assert!(!result.fingerprint.is_empty());
        changed.store(true, Ordering::SeqCst);
        let endpoints = InstanceEndpoints::development().unwrap();
        let stale = install(
            &managed,
            &endpoints,
            &client,
            "PACK0001",
            "VERS0001",
            &[],
            &result.fingerprint,
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(stale.code, "pack_preview_stale");
        assert!(!managed.instance_registry_file().exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn unresolved_fallback_keeps_external_identity_and_rejects_bad_bytes() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(br#"{"schemaVersion":1,"id":"external_fixture","version":"1.0"}"#)
            .unwrap();
        let payload = writer.finish().unwrap().into_inner();
        let sha1 = format!("{:x}", Sha1::digest(&payload));
        let sha512 = format!("{:x}", Sha512::digest(&payload));
        let served = payload.clone();
        let server = TestServer::spawn(Arc::new(move |_: &TestRequest| TestResponse::ok(&served)));
        let root =
            std::env::temp_dir().join(format!("aurora-pack-fallback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let instance =
            crate::instances::InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        let instance_root = managed.instance_paths(&instance).root().to_path_buf();
        std::fs::create_dir_all(&instance_root).unwrap();
        let unrelated = root.join("unrelated.txt");
        std::fs::write(&unrelated, b"owner bytes").unwrap();
        let file = PackFile {
            path: "mods/external.jar".into(),
            sha1,
            sha512: sha512.clone(),
            downloads: vec![format!("{}/external.jar", server.base_url())],
            file_size: payload.len() as u64,
            client: crate::mrpack::EnvironmentSide::Required,
        };
        let source = Sha512ArtifactSource::mrpack_fallback(
            &file.downloads[0],
            &file.sha512,
            Some(file.file_size),
        )
        .unwrap();
        let cache = ArtifactCache::new(managed.clone());
        let artifact = cache.acquire_sha512(&source).await.unwrap();
        verify_external(&file, &artifact).unwrap();
        let target = materialize(
            &instance_root,
            &file.path,
            &artifact.path,
            &artifact.sha256.as_hex(),
        )
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), payload);
        let state = InstalledPack::new(
            instance.clone(),
            PackIdentity {
                provider: "modrinth".into(),
                project_id: "PACK0001".into(),
                version_id: "VERS0001".into(),
                name: "External fixture".into(),
                pack_version: "1".into(),
                artifact_sha512: "a".repeat(128),
                artifact_sha256: "b".repeat(64),
                minecraft_version: "1.21.11".into(),
                fabric_loader_version: "0.19.3".into(),
                installed_at_unix_seconds: 1,
            },
            vec![OwnedComponent {
                path: file.path.clone(),
                sha256: artifact.sha256.as_hex(),
                sha512: file.sha512.clone(),
                provider: None,
                provider_version_id: None,
            }],
            vec![],
            vec![],
        )
        .unwrap();
        state.save(&managed).unwrap();
        let restored = InstalledPack::load(&managed, &instance).unwrap().unwrap();
        assert_eq!(restored.components, state.components);
        assert!(restored.components[0].provider.is_none());

        let bad_sha1 = PackFile {
            sha1: "0".repeat(40),
            path: "mods/bad-sha1.jar".into(),
            ..file.clone()
        };
        assert_eq!(
            verify_external(&bad_sha1, &artifact).unwrap_err().code,
            "pack_digest_mismatch"
        );
        assert!(!instance_root.join("mods/bad-sha1.jar").exists());
        let wrong_sha512 = format!("{:x}", Sha512::digest(b"different bytes"));
        let bad_source = Sha512ArtifactSource::mrpack_fallback(
            &file.downloads[0],
            &wrong_sha512,
            Some(file.file_size),
        )
        .unwrap();
        assert!(cache.acquire_sha512(&bad_source).await.is_err());
        assert!(artifact.path.is_file());
        assert_eq!(std::fs::read(&target).unwrap(), payload);
        assert_eq!(std::fs::read(&unrelated).unwrap(), b"owner bytes");
        assert!(
            Sha512ArtifactSource::mrpack_fallback(
                "https://unapproved.example/external.jar",
                &sha512,
                Some(file.file_size),
            )
            .is_err()
        );
        assert_eq!(
            materialize_bytes(&instance_root, "../outside.txt", b"escape", &"0".repeat(64))
                .unwrap_err()
                .code,
            "pack_invalid_path"
        );
        assert!(!root.join("outside.txt").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
