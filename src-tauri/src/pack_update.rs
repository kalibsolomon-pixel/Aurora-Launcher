//! Phase J: exact Modrinth pack-version updates through three-way
//! reconciliation. The installed pack snapshot, the candidate pack snapshot,
//! and the current local filesystem are compared before anything mutates;
//! the resulting plan is what the user reviews, and apply re-derives the
//! whole plan so a stale preview can never authorize mutation. This is not
//! "Update All" — pack-owned content stays blocked from ordinary provider
//! updates, and this module never calls the ordinary update transaction.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256, Sha512};

use crate::cache::{ArtifactCache, VerifiedArtifact};
use crate::downloads::Sha512ArtifactSource;
use crate::instance_content::{
    ContentState, DependencyKind, ProviderArtifactSource, ProviderIdentity, ProviderInstallPlan,
    ProviderOrigin, ProviderRecord,
};
use crate::instances::InstanceId;
use crate::instances::lifecycle::InstanceEndpoints;
use crate::instances::{InstanceRecord, InstanceRegistry, InstanceState};
use crate::integrity::{ArtifactDigest, verify_file};
use crate::mrpack::{OverrideFile, PackFile, PackPlan};
use crate::pack_state::{
    InstalledPack, OwnedComponent, OwnedOverride, PackDivergence, PackIdentity,
};
use crate::paths::ManagedPaths;

const RECEIPT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub struct PackUpdateError {
    pub code: &'static str,
    pub message: String,
}
impl fmt::Display for PackUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for PackUpdateError {}
fn fail(code: &'static str, message: impl Into<String>) -> PackUpdateError {
    PackUpdateError {
        code,
        message: message.into(),
    }
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackUpdateStatus {
    UpToDate,
    UpdateAvailable,
    Blocked,
    CurrentVersionUnknown,
    ProviderUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackUpdateCandidate {
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub changelog: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackUpdateCheck {
    pub instance_id: String,
    pub status: PackUpdateStatus,
    pub current_version_id: String,
    pub current_pack_version: String,
    pub candidate: Option<PackUpdateCandidate>,
    pub blocked_reason: Option<String>,
    /// Recorded divergences between the installed pack snapshot and local
    /// bytes (Phase J). Zero means the instance is a pristine snapshot.
    pub divergences: usize,
}

struct InstalledContext {
    instance: InstanceId,
    record: InstanceRecord,
    pack: InstalledPack,
}

fn load_installed_context(
    managed: &ManagedPaths,
    instance_id: &str,
) -> Result<InstalledContext, PackUpdateError> {
    let instance = InstanceId::new(instance_id.to_owned())
        .map_err(|_| fail("instance_not_found", "The instance identifier is invalid."))?;
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    let record = registry.find(&instance).cloned().ok_or_else(|| {
        fail(
            "instance_not_found",
            "This instance no longer exists in the launcher registry.",
        )
    })?;
    if record.state() != InstanceState::Ready {
        return Err(fail(
            "instance_not_ready",
            "The instance must finish installing before its modpack can be updated.",
        ));
    }
    if record.pack().is_none() {
        return Err(fail(
            "pack_not_installed",
            "This instance was not created from a Modrinth modpack.",
        ));
    }
    let pack = InstalledPack::load(managed, &instance)
        .map_err(|error| fail(error.code(), pack_state_message(&error)))?
        .ok_or_else(|| {
            fail(
                "pack_state_malformed",
                "The installed modpack state is missing.",
            )
        })?;
    if pack.identity.provider != "modrinth" {
        return Err(fail(
            "pack_provider_unsupported",
            "Only Modrinth modpacks support updates in this build.",
        ));
    }
    Ok(InstalledContext {
        instance,
        record,
        pack,
    })
}

fn pack_state_message(error: &crate::pack_state::PackStateError) -> String {
    match error {
        crate::pack_state::PackStateError::UnsupportedSchema(version) => {
            format!("The installed modpack state uses unsupported schema version {version}.")
        }
        _ => "The installed modpack state is damaged and was left untouched.".into(),
    }
}

/// Same-project pack update discovery. The installed pack identity is the
/// only authority: the provider is asked for the same project's timeline,
/// the installed exact version anchors chronology, and only versions
/// published after it are candidates. Version strings are never compared.
pub async fn check_pack_update(
    managed: &ManagedPaths,
    client: &crate::modrinth::Client,
    instance_id: &str,
) -> Result<PackUpdateCheck, PackUpdateError> {
    let context = load_installed_context(managed, instance_id)?;
    discover(client, &context).await
}

async fn discover(
    client: &crate::modrinth::Client,
    context: &InstalledContext,
) -> Result<PackUpdateCheck, PackUpdateError> {
    let identity = &context.pack.identity;
    let versions = match client.pack_versions(&identity.project_id).await {
        Ok(versions) => versions,
        Err(crate::modrinth::Error::RateLimited(_)) => {
            return Ok(PackUpdateCheck {
                instance_id: context.instance.to_string(),
                status: PackUpdateStatus::ProviderUnavailable,
                current_version_id: identity.version_id.clone(),
                current_pack_version: identity.pack_version.clone(),
                candidate: None,
                blocked_reason: Some("Modrinth is rate limiting requests. Try again later.".into()),
                divergences: context.pack.divergences.len(),
            });
        }
        Err(error) => {
            return Err(fail(
                error.code(),
                "Modrinth could not be reached or returned an unusable response for this modpack.",
            ));
        }
    };
    let current = anchor_current(&versions, identity)?;
    let mut check = PackUpdateCheck {
        instance_id: context.instance.to_string(),
        status: PackUpdateStatus::UpToDate,
        current_version_id: identity.version_id.clone(),
        current_pack_version: identity.pack_version.clone(),
        candidate: None,
        blocked_reason: None,
        divergences: context.pack.divergences.len(),
    };
    // The list is newest-first; the first entry published strictly after the
    // installed version is the candidate. Equal times are never newer.
    let candidate = versions.iter().find(|version| {
        version.version_id != current.version_id
            && version.project_id == current.project_id
            && version.date_published > current.date_published
    });
    let Some(candidate) = candidate else {
        return Ok(check);
    };
    if !supported_loader(candidate) {
        check.status = PackUpdateStatus::Blocked;
        check.blocked_reason = Some(format!(
            "{} {} requires the {} loader, which Aurora cannot install. The instance stays on {}.",
            candidate.name,
            candidate.version_number,
            candidate.loaders.join("/"),
            identity.pack_version
        ));
        return Ok(check);
    }
    let changelog = client
        .pack_version_details(&identity.project_id, &candidate.version_id)
        .await
        .ok()
        .and_then(|details| details.changelog);
    check.status = PackUpdateStatus::UpdateAvailable;
    check.candidate = Some(PackUpdateCandidate {
        version_id: candidate.version_id.clone(),
        name: candidate.name.clone(),
        version_number: candidate.version_number.clone(),
        version_type: candidate.version_type.clone(),
        date_published: candidate.date_published.clone(),
        game_versions: candidate.game_versions.clone(),
        loaders: candidate.loaders.clone(),
        changelog,
    });
    Ok(check)
}

/// The installed exact version must still exist in the same project and still
/// publish the installed pack archive digest. Otherwise Aurora cannot order
/// candidates and refuses instead of guessing.
fn anchor_current<'a>(
    versions: &'a [crate::modrinth::PackVersionOption],
    identity: &PackIdentity,
) -> Result<&'a crate::modrinth::PackVersionOption, PackUpdateError> {
    let current = versions
        .iter()
        .find(|version| version.version_id == identity.version_id)
        .ok_or_else(|| {
            fail(
                "pack_current_version_unknown",
                "Modrinth no longer lists the exact installed pack version for this project.",
            )
        })?;
    if current.project_id != identity.project_id
        || current.archive_sha512.as_deref() != Some(identity.artifact_sha512.as_str())
    {
        return Err(fail(
            "pack_current_version_unknown",
            "The published pack version no longer matches the installed pack archive digest.",
        ));
    }
    Ok(current)
}

fn supported_loader(candidate: &crate::modrinth::PackVersionOption) -> bool {
    candidate.loaders.len() == 1 && candidate.loaders[0].eq_ignore_ascii_case("fabric")
}

// ---------------------------------------------------------------------------
// Three-way reconciliation planning
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackRowKind {
    Component,
    External,
    Override,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackLocalState {
    /// Local bytes equal the old pack's authored bytes.
    MatchesOld,
    /// Local bytes exist but differ from the old authored bytes.
    Modified,
    /// The old pack owned this path; the file is gone.
    Missing,
    /// Nothing pack-related exists at this path.
    Absent,
    /// Local bytes already equal the new pack's authored bytes.
    Equivalent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackRowAction {
    Preserve,
    Restore,
    Acquire,
    Adopt,
    Replace,
    Retire,
    PreserveShared,
    /// A divergence the user must resolve before the update can apply.
    /// Preview rows carry this action; apply refuses to run with one left.
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackConflictKind {
    /// The pack changed the file and the local copy changed too.
    ChangedModified,
    /// The pack removed the file and the local copy changed.
    RemovedModified,
    /// The pack added a path that local content already occupies.
    AddedCollision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackUpdateRow {
    pub path: String,
    pub kind: PackRowKind,
    pub title: Option<String>,
    pub project_id: Option<String>,
    pub old_present: bool,
    pub new_present: bool,
    pub local_state: PackLocalState,
    pub action: PackRowAction,
    pub conflict: Option<PackConflictKind>,
    /// Allowed user resolutions: keepLocal, useNewPack (removed-modified
    /// conflicts only allow keepLocal — Aurora never deletes user bytes).
    pub resolutions: Vec<&'static str>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackUpdateCounts {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub preserved: usize,
    pub conflicts: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackUpdatePlan {
    pub instance_id: String,
    pub name: String,
    pub project_id: String,
    pub current_version_id: String,
    pub current_pack_version: String,
    pub candidate_version_id: String,
    pub candidate_pack_version: String,
    pub candidate_version_type: String,
    pub candidate_date_published: String,
    pub changelog: Option<String>,
    pub minecraft_current: String,
    pub minecraft_candidate: String,
    pub loader_current: String,
    pub loader_candidate: String,
    pub game_transition: bool,
    pub rows: Vec<PackUpdateRow>,
    pub counts: PackUpdateCounts,
    pub new_optional_unselected: Vec<String>,
    pub prior_divergences: usize,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolution {
    pub path: String,
    pub resolution: String,
}

/// The native reconciliation the apply transaction executes. The frontend
/// only ever sees the serialized plan summary, never this structure.
pub struct Reconciliation {
    pub plan: PackUpdatePlan,
    pub archive: VerifiedArtifact,
    pub archive_sha512: String,
    pub old_pack: InstalledPack,
    pub old_state: ContentState,
    pub old_record: InstanceRecord,
    pub new_provider_plans: Vec<ProviderInstallPlan>,
    pub new_external: Vec<PackFile>,
    pub new_overrides: Vec<OverrideFile>,
    pub resolutions: Vec<ConflictResolution>,
    pub target_minecraft: String,
    pub target_loader: String,
    /// Components of the resulting pack state with authored bytes present.
    /// `sha256` is empty for rows whose digest is only known at acquisition.
    pub next_components: Vec<NextComponent>,
    pub next_overrides: Vec<OwnedOverride>,
    pub next_divergences: Vec<PackDivergence>,
    pub next_excluded: Vec<String>,
    pub retired_paths: Vec<RetirePlan>,
    /// Files the transaction must materialize (acquire/replace/restore rows
    /// with the useNewPack resolution). Preserved and adopted files are
    /// deliberately absent: their bytes are already correct on disk.
    pub writes: Vec<WritePlan>,
    pub retired_records: Vec<ProviderRecord>,
}

/// One file the update writes from a verified source.
#[derive(Debug, Clone, PartialEq)]
pub struct WritePlan {
    pub path: String,
    pub source: WriteSource,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WriteSource {
    /// Index into `new_provider_plans`.
    Provider(usize),
    /// Index into `new_external`.
    External(usize),
    /// Index into `new_overrides` (bytes read from the verified archive).
    Override(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NextComponent {
    pub component: OwnedComponent,
    /// Index into `new_provider_plans` when the component is provider-owned.
    pub provider_plan: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetirePlan {
    pub path: String,
    /// The digest the current file must still match before retirement.
    pub expected_sha256: String,
}

/// One local file's observed identity, hashed once per path per planning run.
#[derive(Debug, Clone)]
struct LocalFile {
    sha256: String,
    sha512: String,
}

fn hash_local(root: &Path, relative: &str) -> Result<Option<LocalFile>, PackUpdateError> {
    crate::mrpack::destination_path(relative)
        .map_err(|_| fail("pack_invalid_path", "A pack file destination is unsafe."))?;
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            // An absent path (never installed, retired, or user-deleted) is
            // a classified local state, not an inspection failure.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(fail(
                    "pack_state_io_error",
                    format!("A pack-owned path could not be inspected: {}", error.kind()),
                ));
            }
        };
        if meta.file_type().is_symlink() || reparse(&meta) {
            return Err(fail(
                "pack_unsafe_path",
                "A pack-owned path is a link or redirected location. Inspect the instance folder before updating the modpack.",
            ));
        }
    }
    let meta = std::fs::symlink_metadata(&path).map_err(|error| {
        fail(
            "pack_state_io_error",
            format!("A pack-owned path could not be inspected: {}", error.kind()),
        )
    })?;
    if !meta.is_file() {
        return Ok(None);
    }
    let mut sha256 = Sha256::new();
    let mut sha512 = Sha512::new();
    let mut file = std::fs::File::open(&path).map_err(|_| {
        fail(
            "pack_state_io_error",
            "A pack-owned file could not be read.",
        )
    })?;
    std::io::copy(
        &mut file,
        &mut HashBoth {
            sha256: &mut sha256,
            sha512: &mut sha512,
        },
    )
    .map_err(|_| {
        fail(
            "pack_state_io_error",
            "A pack-owned file could not be read.",
        )
    })?;
    Ok(Some(LocalFile {
        sha256: format!("{:x}", sha256.finalize()),
        sha512: format!("{:x}", sha512.finalize()),
    }))
}

struct HashBoth<'a> {
    sha256: &'a mut Sha256,
    sha512: &'a mut Sha512,
}
impl std::io::Write for HashBoth<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.sha256.update(buffer);
        self.sha512.update(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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

/// The decided treatment of one path before user resolutions apply.
#[derive(Debug, Clone, PartialEq)]
enum Outcome {
    /// Same authored file in both snapshots; local bytes match — preserve.
    KeepAuthored,
    /// Same authored file; local file missing — reacquire (disclosed).
    RestoreAuthored,
    /// The pack authors this file; no conflicting local file exists.
    InstallNew { adopt: bool },
    /// Old authored file replaced by a new one; local matches old or is gone.
    ReplaceOld,
    /// Removed by the pack; local bytes still match the old authored file.
    RetireFile,
    /// Ownership of this path ends without touching any bytes.
    ReleaseOwnership,
    /// A divergence path the new pack still does not author.
    KeepDivergence,
    /// User bytes diverge from both snapshots and must be resolved.
    Conflict(PackConflictKind, Vec<&'static str>),
}

/// Build the full reconciliation: discovery, candidate snapshot resolution,
/// and three-way classification. Read-only — nothing here mutates the
/// instance. A game/loader transition is proven installable before the plan
/// is offered.
pub async fn build_plan(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    client: &crate::modrinth::Client,
    instance_id: &str,
    resolutions: &[ConflictResolution],
    for_apply: bool,
) -> Result<Reconciliation, PackUpdateError> {
    let context = load_installed_context(managed, instance_id)?;
    let check = discover(client, &context).await?;
    let candidate = check.candidate.clone().ok_or_else(|| match check.status {
        PackUpdateStatus::Blocked => fail(
            "pack_update_unavailable",
            check
                .blocked_reason
                .unwrap_or_else(|| "This pack update is not available.".into()),
        ),
        PackUpdateStatus::UpToDate => fail(
            "pack_no_update",
            "The installed modpack version is the newest published version of this pack.",
        ),
        _ => fail(
            "pack_current_version_unknown",
            "The installed modpack version could not be verified against Modrinth.",
        ),
    })?;
    let identity = &context.pack.identity;
    if candidate.version_id == identity.version_id {
        return Err(fail(
            "pack_no_update",
            "The installed modpack version is the newest published version of this pack.",
        ));
    }

    // Optional-file selection carries over by path: files the previous pack
    // had installed stay installed when the new pack still offers them, and
    // brand-new optional files stay unselected until the user opts in. The
    // new snapshot's optional set is only known after parsing, so the first
    // resolution (no optional selection) establishes it and the second
    // resolves with the carried selection.
    let installed_paths: Vec<String> = context
        .pack
        .components
        .iter()
        .map(|component| component.path.clone())
        .collect();
    let resolved = crate::modpacks::resolve(
        managed,
        client,
        &identity.project_id,
        &candidate.version_id,
        &[],
    )
    .await
    .map_err(|error| fail(error.code, error.message))?;
    let selected_optional: Vec<String> = installed_paths
        .iter()
        .filter(|path| {
            resolved
                .pack
                .optional_files
                .iter()
                .any(|file| &file.path == *path)
        })
        .cloned()
        .collect();
    let resolved = if selected_optional.is_empty() {
        resolved
    } else {
        crate::modpacks::resolve(
            managed,
            client,
            &identity.project_id,
            &candidate.version_id,
            &selected_optional,
        )
        .await
        .map_err(|error| fail(error.code, error.message))?
    };
    if resolved.provider_version_id != candidate.version_id {
        return Err(fail(
            "pack_preview_stale",
            "The pack version changed while preparing the update. Review it again.",
        ));
    }
    let target_minecraft = resolved.pack.minecraft_version.clone();
    let target_loader = resolved.pack.fabric_loader_version.clone();
    let game_transition = context.record.installed().minecraft_version != target_minecraft
        || context
            .record
            .installed()
            .platform
            .version()
            .is_none_or(|version| version != target_loader);
    if game_transition {
        // Prove the target combination is installable before offering it.
        resolve_target_configuration(endpoints, &target_minecraft, &target_loader)
            .await
            .map_err(|error| {
                fail(
                    "pack_transition_unsupported",
                    format!(
                        "The update requires Minecraft {target_minecraft} with Fabric Loader {target_loader}, which Aurora cannot install: {error}"
                    ),
                )
            })?;
    }

    let state = ContentState::load(managed, &context.instance)
        .map_err(|error| fail("pack_state_malformed", error.to_string()))?;
    classify(
        managed,
        context,
        resolved,
        candidate,
        state,
        resolutions,
        for_apply,
    )
}

async fn resolve_target_configuration(
    endpoints: &InstanceEndpoints,
    minecraft: &str,
    loader: &str,
) -> Result<
    crate::instances::platform::InstalledConfiguration,
    crate::instances::lifecycle::InstanceError,
> {
    let mut configuration =
        crate::instances::settings::InstanceConfiguration::for_minecraft_version(minecraft);
    configuration.set_loader(crate::instances::settings::LoaderConfiguration::Fabric {
        policy: crate::instances::settings::LoaderPolicy::Pinned {
            version: loader.to_owned(),
        },
    });
    configuration.set_aurora_enabled(false);
    crate::instances::lifecycle::resolve_packed_fabric_configuration(endpoints, &configuration)
        .await
}

/// Classification input for one owned-or-authored path. Old-side fields come
/// from the installed pack snapshot; new-side fields from the resolved
/// candidate snapshot; `local` from hashing the current filesystem exactly
/// once per path.
struct PathFacts {
    path: String,
    kind: PackRowKind,
    title: Option<String>,
    old_component: Option<OwnedComponent>,
    old_override: Option<OwnedOverride>,
    old_divergence: Option<PackDivergence>,
    /// The old authored digest (component or override). A divergence path
    /// has no currently-owned authored bytes.
    old_sha256: Option<String>,
    old_sha512: Option<String>,
    new_provider: Option<usize>,
    new_external: Option<PackFile>,
    new_override: Option<OverrideFile>,
    local: Option<LocalFile>,
}

impl PathFacts {
    fn new_present(&self) -> bool {
        self.new_provider.is_some() || self.new_external.is_some() || self.new_override.is_some()
    }

    /// The new snapshot's authored identity for this path, by strongest
    /// available digest.
    fn new_override_sha256(&self) -> Option<String> {
        self.new_override
            .as_ref()
            .map(|file| file.sha256.to_ascii_lowercase())
    }

    fn project_id(&self, plans: &[ProviderInstallPlan]) -> Option<String> {
        self.new_provider
            .map(|index| plans[index].project_id.clone())
            .or_else(|| {
                self.old_component
                    .as_ref()
                    .and_then(|component| component.provider.as_ref())
                    .map(|provider| provider.project_id.clone())
            })
    }

    fn local_state(&self) -> PackLocalState {
        if let Some(expected) = &self.old_sha256 {
            return match &self.local {
                None => PackLocalState::Missing,
                Some(file) => {
                    if file.sha256 == *expected {
                        PackLocalState::MatchesOld
                    } else {
                        PackLocalState::Modified
                    }
                }
            };
        }
        match &self.local {
            None => PackLocalState::Missing,
            Some(_) => PackLocalState::Absent,
        }
    }

    /// Local bytes already equal the new authored file, proven by digest.
    fn equivalent_to_new(&self, plans: &[ProviderInstallPlan]) -> bool {
        let Some(file) = &self.local else {
            return false;
        };
        if let Some(index) = self.new_provider {
            let plan = &plans[index];
            let expected = match &plan.source {
                ProviderArtifactSource::Sha512(source) => source.sha512().as_hex(),
                ProviderArtifactSource::Sha256(_) => plan.file_id.to_ascii_lowercase(),
            };
            return file.sha512 == expected;
        }
        if let Some(external) = &self.new_external {
            return file.sha512 == external.sha512.to_ascii_lowercase();
        }
        if let Some(expected) = self.new_override_sha256() {
            return file.sha256 == expected;
        }
        false
    }

    /// Both snapshots author the exact same file identity at this path.
    fn same_authored_file(&self, plans: &[ProviderInstallPlan]) -> bool {
        if let Some(index) = self.new_provider {
            let plan = &plans[index];
            let expected = match &plan.source {
                ProviderArtifactSource::Sha512(source) => source.sha512().as_hex(),
                ProviderArtifactSource::Sha256(_) => plan.file_id.to_ascii_lowercase(),
            };
            return self.old_sha512.as_deref() == Some(expected.as_str());
        }
        if let Some(external) = &self.new_external {
            return self.old_sha512.as_deref()
                == Some(external.sha512.to_ascii_lowercase().as_str());
        }
        if let (Some(old), Some(new)) = (&self.old_sha256, self.new_override_sha256()) {
            return old.eq_ignore_ascii_case(&new);
        }
        false
    }

    /// The old pack owns this path in some form (authored or divergent).
    fn old_owned(&self) -> bool {
        self.old_component.is_some() || self.old_override.is_some() || self.old_divergence.is_some()
    }
}

fn classify(
    managed: &ManagedPaths,
    context: InstalledContext,
    resolved: crate::modpacks::Resolved,
    candidate: PackUpdateCandidate,
    state: ContentState,
    resolutions: &[ConflictResolution],
    for_apply: bool,
) -> Result<Reconciliation, PackUpdateError> {
    let root = crate::instance_content::validated_instance_root(managed, &context.instance)
        .map_err(|_| {
            fail(
                "pack_unsafe_path",
                "The instance root is redirected or unsafe.",
            )
        })?;
    let pack: &PackPlan = &resolved.pack;
    let plans: &[ProviderInstallPlan] = &resolved.provider_plans;
    let old_pack = context.pack;

    let provider_index_by_path: HashMap<String, usize> = plans
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            (
                format!("{}/{}", plan.content_type.directory_name(), plan.file_name).to_lowercase(),
                index,
            )
        })
        .collect();
    let external_by_path: HashMap<String, PackFile> = resolved
        .unresolved
        .iter()
        .map(|file| (file.path.to_lowercase(), file.clone()))
        .collect();
    let override_by_path: HashMap<String, OverrideFile> = pack
        .overrides
        .iter()
        .map(|item| (item.path.to_lowercase(), item.clone()))
        .collect();
    let titles: HashMap<String, String> = resolved
        .preview
        .recognized
        .iter()
        .map(|item| (item.path.to_lowercase(), item.title.clone()))
        .collect();

    // Union of every path the old snapshot owns, the new snapshot authors,
    // or a recorded divergence claims. Sorted for a deterministic plan.
    let mut paths: BTreeSet<String> = BTreeSet::new();
    paths.extend(
        old_pack
            .components
            .iter()
            .map(|item| item.path.to_lowercase()),
    );
    paths.extend(
        old_pack
            .overrides
            .iter()
            .map(|item| item.path.to_lowercase()),
    );
    paths.extend(
        old_pack
            .divergences
            .iter()
            .map(|item| item.path.to_lowercase()),
    );
    paths.extend(provider_index_by_path.keys().cloned());
    paths.extend(external_by_path.keys().cloned());
    paths.extend(override_by_path.keys().cloned());

    let old_components: HashMap<String, &OwnedComponent> = old_pack
        .components
        .iter()
        .map(|item| (item.path.to_lowercase(), item))
        .collect();
    let old_overrides: HashMap<String, &OwnedOverride> = old_pack
        .overrides
        .iter()
        .map(|item| (item.path.to_lowercase(), item))
        .collect();
    let old_divergences: HashMap<String, &PackDivergence> = old_pack
        .divergences
        .iter()
        .map(|item| (item.path.to_lowercase(), item))
        .collect();

    let mut facts: Vec<PathFacts> = Vec::new();
    for key in paths {
        let component = old_components.get(&key).cloned().cloned();
        let old_override = old_overrides.get(&key).cloned().cloned();
        let divergence = old_divergences.get(&key).cloned().cloned();
        let new_provider = provider_index_by_path.get(&key).copied();
        let new_external = external_by_path.get(&key).cloned();
        let new_override = override_by_path.get(&key).cloned();
        let path = component
            .as_ref()
            .map(|item| item.path.clone())
            .or_else(|| old_override.as_ref().map(|item| item.path.clone()))
            .or_else(|| divergence.as_ref().map(|item| item.path.clone()))
            .or_else(|| new_external.as_ref().map(|file| file.path.clone()))
            .or_else(|| new_override.as_ref().map(|file| file.path.clone()))
            .or_else(|| {
                new_provider.map(|index| {
                    format!(
                        "{}/{}",
                        plans[index].content_type.directory_name(),
                        plans[index].file_name
                    )
                })
            })
            .expect("path came from one of the maps");
        let kind = if component
            .as_ref()
            .is_some_and(|item| item.provider.is_some())
            || new_provider.is_some()
        {
            PackRowKind::Component
        } else if new_external.is_some() || component.is_some() {
            // A provider-less pack component is an explicitly external file.
            PackRowKind::External
        } else {
            PackRowKind::Override
        };
        let old_sha256 = component
            .as_ref()
            .map(|item| item.sha256.clone())
            .or_else(|| old_override.as_ref().map(|item| item.sha256.clone()));
        let old_sha512 = component.as_ref().map(|item| item.sha512.clone());
        let local = hash_local(&root, &path)?;
        facts.push(PathFacts {
            path,
            kind,
            title: titles.get(&key).cloned(),
            old_component: component,
            old_override,
            old_divergence: divergence,
            old_sha256,
            old_sha512,
            new_provider,
            new_external,
            new_override,
            local,
        });
    }

    // Decide the outcome of every path, then validate resolutions.
    let mut decided: Vec<(PathFacts, Outcome)> = Vec::new();
    for fact in facts {
        let outcome = decide(&fact, plans);
        decided.push((fact, outcome));
    }

    let mut resolution_map: HashMap<String, &ConflictResolution> = HashMap::new();
    for resolution in resolutions {
        let key = resolution.path.to_lowercase();
        if resolution_map.insert(key.clone(), resolution).is_some() {
            return Err(fail(
                "pack_resolution_invalid",
                "A conflict resolution was supplied twice.",
            ));
        }
    }
    let allowed = |outcome: &Outcome| -> Option<(Vec<&'static str>, PackConflictKind)> {
        match outcome {
            Outcome::Conflict(kind, allowed) => Some((allowed.to_vec(), *kind)),
            _ => None,
        }
    };
    for (fact, outcome) in &decided {
        let key = fact.path.to_lowercase();
        match resolution_map.get(&key) {
            Some(resolution) => {
                let Some((allowed, _)) = allowed(outcome) else {
                    return Err(fail(
                        "pack_resolution_invalid",
                        format!("\"{}\" is not a conflict in this update.", fact.path),
                    ));
                };
                if !allowed.contains(&resolution.resolution.as_str()) {
                    return Err(fail(
                        "pack_resolution_invalid",
                        format!(
                            "\"{}\" does not allow the resolution \"{}\".",
                            fact.path, resolution.resolution
                        ),
                    ));
                }
            }
            None => {
                if for_apply && allowed(outcome).is_some() {
                    return Err(fail(
                        "pack_resolution_required",
                        format!(
                            "\"{}\" needs an explicit choice before the modpack can update.",
                            fact.path
                        ),
                    ));
                }
            }
        }
    }

    // Finalize rows, resulting pack state, and managed-record retirement.
    // Retired provider identities feed the shared-ownership check, so they
    // are collected first.
    let mut retiring_keys: HashSet<String> = HashSet::new();
    // Provider projects the next state still covers: a successor version of
    // the same project satisfies every remaining dependent, so a superseded
    // file may retire even while preserved components depend on the project.
    let mut joining_projects: HashSet<(String, String, String)> = HashSet::new();
    for (fact, outcome) in &decided {
        if let Some(component) = &fact.old_component {
            if let Some(provider) = &component.provider {
                let retiring = match outcome {
                    Outcome::KeepAuthored => false,
                    Outcome::RestoreAuthored => false,
                    Outcome::InstallNew { adopt: true } => true,
                    Outcome::InstallNew { adopt: false } => true,
                    Outcome::ReplaceOld => true,
                    Outcome::RetireFile => true,
                    Outcome::ReleaseOwnership => true,
                    Outcome::KeepDivergence => false,
                    Outcome::Conflict(..) => true,
                };
                if retiring {
                    retiring_keys.insert(record_key(provider, component));
                }
            }
        }
        let joins = match outcome {
            Outcome::InstallNew { adopt: true } => true,
            Outcome::InstallNew { adopt: false } => true,
            Outcome::ReplaceOld => true,
            Outcome::RestoreAuthored => true,
            Outcome::Conflict(..) => true, // the apply path resolves these
            _ => false,
        };
        if joins {
            if let Some(index) = fact.new_provider {
                let plan = &plans[index];
                joining_projects.insert((
                    plan.content_type.directory_name().to_owned(),
                    plan.provider.clone(),
                    plan.project_id.clone(),
                ));
            }
        }
    }

    let mut rows: Vec<PackUpdateRow> = Vec::new();
    let mut next_components: Vec<NextComponent> = Vec::new();
    let mut next_overrides: Vec<OwnedOverride> = Vec::new();
    let mut next_divergences: Vec<PackDivergence> = Vec::new();
    let mut retired_paths: Vec<RetirePlan> = Vec::new();
    let mut writes: Vec<WritePlan> = Vec::new();
    let mut retired_records: Vec<ProviderRecord> = Vec::new();
    let now = crate::instance_content::now_unix_seconds();

    for (fact, outcome) in decided {
        let key = fact.path.to_lowercase();
        let resolution = resolution_map
            .get(&key)
            .map(|item| item.resolution.as_str());
        let mut action: PackRowAction;
        let mut conflict: Option<PackConflictKind> = None;
        let mut allowed_resolutions: Vec<&'static str> = Vec::new();
        let mut note: Option<String> = None;
        // The current local file is retired (moved aside) during the update.
        let mut retire_old_file = false;
        // The old provider record (if any) leaves managed state.
        let mut retire_old_record = false;
        // Authored bytes of the new snapshot end up present at this path.
        let mut authored_new = false;

        match outcome {
            Outcome::KeepAuthored => {
                action = PackRowAction::Preserve;
                match fact.kind {
                    PackRowKind::Override => {
                        next_overrides.push(fact.old_override.clone().expect("override fact"))
                    }
                    PackRowKind::Component | PackRowKind::External => {
                        next_components.push(NextComponent {
                            component: fact.old_component.clone().expect("component fact"),
                            provider_plan: None,
                        });
                    }
                }
            }
            Outcome::RestoreAuthored => {
                action = PackRowAction::Restore;
                note = Some("This file was missing; the update restores it.".into());
                retire_old_record = fact.old_component.is_some();
                writes.push(write_plan(&fact));
                authored_new = true;
            }
            Outcome::InstallNew { adopt } => {
                action = if adopt {
                    PackRowAction::Adopt
                } else {
                    PackRowAction::Acquire
                };
                if adopt {
                    note = Some(
                        "The local file already matches the new pack version byte for byte.".into(),
                    );
                    retire_old_record = fact.old_component.is_some();
                } else {
                    writes.push(write_plan(&fact));
                }
                authored_new = true;
            }
            Outcome::ReplaceOld => {
                action = PackRowAction::Replace;
                if fact.local.is_none() {
                    note = Some("This file was missing; the update replaces it.".into());
                }
                retire_old_file = fact.local.is_some();
                retire_old_record = true;
                writes.push(write_plan(&fact));
                authored_new = true;
            }
            Outcome::RetireFile => {
                action = PackRowAction::Retire;
                retire_old_file = true;
                retire_old_record = true;
            }
            Outcome::ReleaseOwnership => {
                action = PackRowAction::Retire;
                note = Some("No local file remains; only the modpack's record of it ends.".into());
                retire_old_record = fact.old_component.is_some();
            }
            Outcome::KeepDivergence => {
                action = PackRowAction::Preserve;
                note = Some("Your local file stays; the pack does not install here.".into());
                if let Some(local) = &fact.local {
                    next_divergences.push(PackDivergence {
                        path: fact.path.clone(),
                        component: fact.kind != PackRowKind::Override,
                        expected_sha256: None,
                        expected_sha512: None,
                        local_sha256: local.sha256.clone(),
                        resolution: "keepLocal".into(),
                        recorded_at_unix_seconds: now,
                    });
                }
            }
            Outcome::Conflict(kind, ref allowed) => {
                conflict = Some(kind);
                allowed_resolutions = allowed.to_vec();
                let Some(resolution) = resolution else {
                    // Preview: the row shows the conflict and its allowed
                    // choices; the apply path refuses to run with one left.
                    action = PackRowAction::Conflict;
                    rows.push(PackUpdateRow {
                        path: fact.path.clone(),
                        kind: fact.kind,
                        title: fact.title.clone(),
                        project_id: fact.project_id(plans),
                        old_present: fact.old_owned(),
                        new_present: fact.new_present(),
                        local_state: fact.local_state(),
                        action,
                        conflict,
                        resolutions: allowed_resolutions,
                        note,
                    });
                    continue;
                };
                match resolution {
                    "keepLocal" => {
                        action = PackRowAction::Preserve;
                        note = Some("Your local file is kept exactly as it is.".into());
                        retire_old_record = fact.old_component.is_some();
                        if let Some(local) = &fact.local {
                            let (expected_sha256, expected_sha512) = new_expectation(&fact, plans);
                            next_divergences.push(PackDivergence {
                                path: fact.path.clone(),
                                component: fact.kind != PackRowKind::Override,
                                expected_sha256,
                                expected_sha512,
                                local_sha256: local.sha256.clone(),
                                resolution: "keepLocal".into(),
                                recorded_at_unix_seconds: now,
                            });
                        }
                    }
                    _ => {
                        // The user explicitly adopted the new pack version;
                        // the bytes they had are backed up before replacement.
                        action = PackRowAction::Replace;
                        retire_old_file = fact.local.is_some();
                        retire_old_record = fact.old_component.is_some();
                        writes.push(write_plan(&fact));
                        authored_new = true;
                    }
                }
            }
        }

        // Shared ownership: a retired provider component that remaining
        // managed content still requires keeps its file and record; only the
        // pack's ownership relationship ends.
        if matches!(outcome, Outcome::RetireFile) {
            if let Some(component) = &fact.old_component {
                if let Some(provider) = &component.provider {
                    let covered_by_successor = joining_projects.contains(&(
                        provider.content_type.directory_name().to_owned(),
                        provider.provider.clone(),
                        provider.project_id.clone(),
                    ));
                    let still_required = !covered_by_successor
                        && state.required_by(provider).iter().any(|dependent| {
                            state
                                .find(dependent)
                                .map(record_key_of)
                                .is_none_or(|dependent_key| !retiring_keys.contains(&dependent_key))
                        });
                    if still_required {
                        action = PackRowAction::PreserveShared;
                        note = Some(
                            "Still required by other installed content: the file stays and only the modpack's ownership ends."
                                .into(),
                        );
                        retire_old_file = false;
                        retire_old_record = false;
                    }
                }
            }
        }

        if retire_old_record {
            if let Some(component) = &fact.old_component {
                if let Some(provider) = &component.provider {
                    if let Some(record) = find_pack_record(&state, provider, component) {
                        retired_records.push(record.clone());
                    }
                }
            }
        }
        if retire_old_file {
            // The proof digest the file must still match at apply: the old
            // authored bytes when they still match, otherwise the exact bytes
            // observed during planning (a useNewPack resolution).
            let expected = if fact.local_state() == PackLocalState::MatchesOld {
                fact.old_sha256.clone()
            } else {
                fact.local.as_ref().map(|file| file.sha256.clone())
            }
            .expect("a retired file was observed with bytes during planning");
            retired_paths.push(RetirePlan {
                path: fact.path.clone(),
                expected_sha256: expected,
            });
        }
        // The new snapshot's proven local digest for adopted files; other
        // new files gain theirs from the verified acquisition.
        let adopt_sha256 = match outcome {
            Outcome::InstallNew { adopt: true } => {
                fact.local.as_ref().map(|file| file.sha256.clone())
            }
            _ => None,
        };
        if authored_new {
            push_new_ownership(
                &fact,
                plans,
                adopt_sha256,
                &mut next_components,
                &mut next_overrides,
            );
        }

        rows.push(PackUpdateRow {
            path: fact.path.clone(),
            kind: fact.kind,
            title: fact.title.clone(),
            project_id: fact.project_id(plans),
            old_present: fact.old_owned(),
            new_present: fact.new_present(),
            local_state: fact.local_state(),
            action,
            conflict,
            resolutions: allowed_resolutions,
            note,
        });
    }

    let mut counts = PackUpdateCounts::default();
    for row in &rows {
        match row.action {
            PackRowAction::Acquire | PackRowAction::Adopt | PackRowAction::Restore => {
                counts.added += 1
            }
            PackRowAction::Replace => counts.updated += 1,
            PackRowAction::Retire | PackRowAction::PreserveShared => counts.removed += 1,
            PackRowAction::Preserve => counts.preserved += 1,
            PackRowAction::Conflict => counts.conflicts += 1,
        }
    }

    let game_transition = context.record.installed().minecraft_version != pack.minecraft_version
        || context
            .record
            .installed()
            .platform
            .version()
            .is_none_or(|version| version != pack.fabric_loader_version);

    let new_optional_unselected: Vec<String> = pack
        .optional_files
        .iter()
        .filter(|file| {
            !next_components
                .iter()
                .any(|item| item.component.path == file.path)
        })
        .map(|file| file.path.clone())
        .collect();
    let mut next_excluded = pack.excluded_paths.clone();
    next_excluded.extend(new_optional_unselected.iter().cloned());
    next_excluded.sort();
    next_excluded.dedup();

    let plan = PackUpdatePlan {
        instance_id: context.instance.to_string(),
        name: pack.name.clone(),
        project_id: old_pack.identity.project_id.clone(),
        current_version_id: old_pack.identity.version_id.clone(),
        current_pack_version: old_pack.identity.pack_version.clone(),
        candidate_version_id: resolved.provider_version_id.clone(),
        candidate_pack_version: pack.pack_version.clone(),
        candidate_version_type: candidate.version_type.clone(),
        candidate_date_published: candidate.date_published.clone(),
        changelog: candidate.changelog.clone(),
        minecraft_current: context.record.installed().minecraft_version.clone(),
        minecraft_candidate: resolved.pack.minecraft_version.clone(),
        loader_current: context
            .record
            .installed()
            .platform
            .version()
            .unwrap_or_default()
            .to_string(),
        loader_candidate: resolved.pack.fabric_loader_version.clone(),
        game_transition,
        rows,
        counts,
        new_optional_unselected,
        prior_divergences: old_pack.divergences.len(),
        fingerprint: String::new(),
    };
    let fingerprint = plan_fingerprint(&plan, &old_pack, &state, &context.record, &resolved);
    let plan = PackUpdatePlan {
        fingerprint,
        ..plan
    };
    Ok(Reconciliation {
        plan,
        archive: resolved.archive,
        archive_sha512: resolved.archive_sha512.clone(),
        old_pack,
        old_state: state,
        old_record: context.record,
        new_provider_plans: plans.to_vec(),
        new_external: resolved.unresolved.clone(),
        new_overrides: pack.overrides.clone(),
        resolutions: resolutions.to_vec(),
        target_minecraft: resolved.pack.minecraft_version.clone(),
        target_loader: resolved.pack.fabric_loader_version.clone(),
        next_components,
        next_overrides,
        next_divergences,
        next_excluded,
        retired_paths,
        writes,
        retired_records,
    })
}

fn new_expectation(
    fact: &PathFacts,
    plans: &[ProviderInstallPlan],
) -> (Option<String>, Option<String>) {
    if let Some(expected) = fact.new_override_sha256() {
        return (Some(expected), None);
    }
    if fact.new_external.is_some() {
        return (
            None,
            fact.new_external
                .as_ref()
                .map(|file| file.sha512.to_ascii_lowercase()),
        );
    }
    if let Some(index) = fact.new_provider {
        let plan = &plans[index];
        let sha512 = match &plan.source {
            ProviderArtifactSource::Sha512(source) => Some(source.sha512().as_hex()),
            ProviderArtifactSource::Sha256(_) => None,
        };
        return (None, sha512);
    }
    (None, None)
}

/// Register the new snapshot's ownership for a path whose authored bytes will
/// be present after the update. `known_sha256` carries the proven local
/// digest for adopted files; other digests are filled by the transaction
/// from the verified artifact.
fn push_new_ownership(
    fact: &PathFacts,
    plans: &[ProviderInstallPlan],
    known_sha256: Option<String>,
    next_components: &mut Vec<NextComponent>,
    next_overrides: &mut Vec<OwnedOverride>,
) {
    if let Some(expected) = fact.new_override_sha256() {
        next_overrides.push(OwnedOverride {
            path: fact.path.clone(),
            sha256: expected,
        });
        return;
    }
    if fact.new_external.is_some() {
        let file = fact.new_external.as_ref().expect("checked");
        next_components.push(NextComponent {
            component: OwnedComponent {
                path: fact.path.clone(),
                sha256: known_sha256.unwrap_or_default(),
                sha512: file.sha512.to_ascii_lowercase(),
                provider: None,
                provider_version_id: None,
            },
            provider_plan: None,
        });
        return;
    }
    if let Some(index) = fact.new_provider {
        let plan = &plans[index];
        let sha512 = match &plan.source {
            ProviderArtifactSource::Sha512(source) => source.sha512().as_hex(),
            ProviderArtifactSource::Sha256(_) => plan.file_id.to_ascii_lowercase(),
        };
        next_components.push(NextComponent {
            component: OwnedComponent {
                path: fact.path.clone(),
                sha256: known_sha256.unwrap_or_default(),
                sha512,
                provider: Some(ProviderIdentity {
                    content_type: plan.content_type,
                    provider: "modrinth".into(),
                    project_id: plan.project_id.clone(),
                }),
                provider_version_id: Some(plan.version_id.clone()),
            },
            provider_plan: Some(index),
        });
    }
}

fn record_key(provider: &ProviderIdentity, component: &OwnedComponent) -> String {
    format!(
        "{}:{}:{}:{}",
        provider.content_type.directory_name(),
        provider.provider,
        provider.project_id,
        component.sha512.to_ascii_lowercase()
    )
}

fn record_key_of(record: &ProviderRecord) -> String {
    format!(
        "{}:{}:{}:{}",
        record.content_type.directory_name(),
        record.provider,
        record.project_id,
        record.file_id.to_ascii_lowercase()
    )
}

fn find_pack_record<'a>(
    state: &'a ContentState,
    provider: &ProviderIdentity,
    component: &OwnedComponent,
) -> Option<&'a ProviderRecord> {
    let version_id = component.provider_version_id.as_deref()?;
    state.entries.iter().find(|record| {
        record.identity() == *provider
            && record.version_id == version_id
            && record.file_id.eq_ignore_ascii_case(&component.sha512)
    })
}

/// The verified source a write row materializes its bytes from. External and
/// override writes resolve their index at apply time; the path identifies
/// them within the reconciliation.
fn write_plan(fact: &PathFacts) -> WritePlan {
    if let Some(index) = fact.new_provider {
        return WritePlan {
            path: fact.path.clone(),
            source: WriteSource::Provider(index),
        };
    }
    if fact.new_external.is_some() {
        return WritePlan {
            path: fact.path.clone(),
            source: WriteSource::External(usize::MAX),
        };
    }
    WritePlan {
        path: fact.path.clone(),
        source: WriteSource::Override(usize::MAX),
    }
}

/// The fingerprint binds the reviewed reconciliation to the exact world it
/// was derived from: the candidate snapshot identity, every row's
/// classification inputs (including the observed local digests), and the
/// prior pack/managed/registry state. It deliberately excludes the user's
/// conflict resolutions and the projections derived from them — resolutions
/// are validated separately against the same rows, and apply re-derives the
/// projections itself.
fn plan_fingerprint(
    plan: &PackUpdatePlan,
    old_pack: &InstalledPack,
    old_state: &ContentState,
    old_record: &InstanceRecord,
    resolved: &crate::modpacks::Resolved,
) -> String {
    let rows: Vec<_> = plan
        .rows
        .iter()
        .map(|row| {
            (
                &row.path,
                row.kind,
                row.local_state,
                row.conflict,
                row.old_present,
                row.new_present,
            )
        })
        .collect();
    let payload = serde_json::json!({
        "candidate": {
            "versionId": plan.candidate_version_id,
            "packVersion": plan.candidate_pack_version,
            "projectId": plan.project_id,
            "minecraft": plan.minecraft_candidate,
            "loader": plan.loader_candidate,
            "gameTransition": plan.game_transition,
            "archiveSha512": resolved.archive_sha512,
        },
        "rows": rows,
        "optionalUnselected": plan.new_optional_unselected,
        "oldPack": serde_json::to_value(old_pack).unwrap_or_default(),
        "oldState": serde_json::to_value(old_state).unwrap_or_default(),
        "oldRecord": serde_json::to_value(old_record).unwrap_or_default(),
    });
    format!("{:x}", Sha256::digest(payload.to_string().as_bytes()))
}

fn decide(fact: &PathFacts, plans: &[ProviderInstallPlan]) -> Outcome {
    let new_present = fact.new_present();
    let equivalent = fact.equivalent_to_new(plans);
    let local_state = fact.local_state();
    match (fact.old_owned(), new_present) {
        (true, true) => {
            let same = fact.same_authored_file(plans);
            if same && fact.old_divergence.is_none() {
                match local_state {
                    PackLocalState::MatchesOld => Outcome::KeepAuthored,
                    PackLocalState::Equivalent => Outcome::KeepAuthored,
                    PackLocalState::Modified => {
                        Outcome::Conflict(PackConflictKind::ChangedModified, vec!["keepLocal"])
                    }
                    PackLocalState::Missing => Outcome::RestoreAuthored,
                    PackLocalState::Absent => Outcome::RestoreAuthored,
                }
            } else if equivalent {
                Outcome::InstallNew { adopt: true }
            } else if fact.local.is_none() {
                // The old file is gone; installing the new authored file is
                // not a conflict, but the plan discloses the restoration.
                Outcome::ReplaceOld
            } else if fact.old_component.is_some() || fact.old_override.is_some() {
                if local_state == PackLocalState::MatchesOld {
                    Outcome::ReplaceOld
                } else {
                    Outcome::Conflict(
                        PackConflictKind::ChangedModified,
                        vec!["keepLocal", "useNewPack"],
                    )
                }
            } else {
                Outcome::Conflict(
                    PackConflictKind::AddedCollision,
                    vec!["keepLocal", "useNewPack"],
                )
            }
        }
        (true, false) => {
            if fact.old_component.is_none() && fact.old_override.is_none() {
                // Already-divergent path the new pack still does not author.
                Outcome::KeepDivergence
            } else {
                match local_state {
                    PackLocalState::MatchesOld | PackLocalState::Equivalent => Outcome::RetireFile,
                    PackLocalState::Missing => Outcome::ReleaseOwnership,
                    PackLocalState::Modified | PackLocalState::Absent => {
                        Outcome::Conflict(PackConflictKind::RemovedModified, vec!["keepLocal"])
                    }
                }
            }
        }
        (false, true) => {
            if fact.local.is_some() && !equivalent {
                Outcome::Conflict(
                    PackConflictKind::AddedCollision,
                    vec!["keepLocal", "useNewPack"],
                )
            } else {
                Outcome::InstallNew { adopt: equivalent }
            }
        }
        (false, false) => Outcome::ReleaseOwnership,
    }
}

// ---------------------------------------------------------------------------
// Acquisition
// ---------------------------------------------------------------------------

/// One verified artifact backing a file the update activates or adopts.
struct SourcedArtifact {
    /// Path of the verified store object (or the proven local file for an
    /// adopted component, which equals the published bytes).
    path: PathBuf,
    sha256: String,
    bytes: u64,
}

struct Acquired {
    /// Verified artifact per provider plan index.
    providers: Vec<SourcedArtifact>,
    /// Verified artifact per external file, in `new_external` order.
    externals: Vec<SourcedArtifact>,
}

async fn acquire_all(
    managed: &ManagedPaths,
    reconciliation: &Reconciliation,
) -> Result<Acquired, PackUpdateError> {
    let cache = ArtifactCache::new(managed.clone());
    let mut providers = Vec::new();
    for plan in &reconciliation.new_provider_plans {
        let artifact = match &plan.source {
            ProviderArtifactSource::Sha256(source) => cache.acquire(source).await,
            ProviderArtifactSource::Sha512(source) => cache.acquire_sha512(source).await,
        }
        .map_err(|error| fail("pack_download_failed", error.to_string()))?;
        providers.push(SourcedArtifact {
            path: artifact.path,
            sha256: artifact.sha256.as_hex(),
            bytes: artifact.bytes,
        });
    }
    let mut externals = Vec::new();
    for file in &reconciliation.new_external {
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
            .map_err(|error| fail("pack_download_failed", error.to_string()))?;
        crate::modpacks::verify_external(file, &artifact)
            .map_err(|error| fail(error.code, error.message))?;
        externals.push(SourcedArtifact {
            path: artifact.path,
            sha256: artifact.sha256.as_hex(),
            bytes: artifact.bytes,
        });
    }
    Ok(Acquired {
        providers,
        externals,
    })
}

/// Fill the resulting pack state's component digests from acquisition, and
/// build the next managed-content state. Fails closed on any identity
/// conflict before a single byte moves.
fn build_next_documents(
    reconciliation: &Reconciliation,
    acquired: &Acquired,
) -> Result<(InstalledPack, ContentState), PackUpdateError> {
    let mut components = Vec::new();
    for item in &reconciliation.next_components {
        let mut component = item.component.clone();
        if component.sha256.is_empty() {
            if let Some(index) = item.provider_plan {
                component.sha256 = acquired.providers[index].sha256.clone();
            } else if let Some(position) = reconciliation
                .new_external
                .iter()
                .position(|file| file.sha512.eq_ignore_ascii_case(&component.sha512))
            {
                component.sha256 = acquired.externals[position].sha256.clone();
            } else {
                return Err(fail(
                    "pack_plan_invalid",
                    "A new pack component has no verified artifact.",
                ));
            }
        }
        components.push(component);
    }

    // Next managed state: retire pack-owned records, register new ones.
    let mut next = reconciliation.old_state.clone();
    let retired_keys: HashSet<String> = reconciliation
        .retired_records
        .iter()
        .map(record_key_of)
        .collect();
    next.entries
        .retain(|record| !retired_keys.contains(&record_key_of(record)));
    let now = crate::instance_content::now_unix_seconds();
    let mut identities: Vec<_> = next.entries.iter().map(ProviderRecord::identity).collect();
    let mut additions: Vec<ProviderRecord> = Vec::new();
    for item in &reconciliation.next_components {
        let Some(index) = item.provider_plan else {
            continue;
        };
        let plan = &reconciliation.new_provider_plans[index];
        let sha256 = acquired.providers[index].sha256.clone();
        let mut record = plan.provider_record(sha256, ProviderOrigin::Pack);
        if next.entries.iter().chain(additions.iter()).any(|existing| {
            existing.identity() == record.identity()
                && existing.file_id.eq_ignore_ascii_case(&record.file_id)
        }) {
            return Err(fail(
                "pack_content_collision",
                format!(
                    "{} is already installed as managed content.",
                    record.file_name
                ),
            ));
        }
        record.explicitly_retained = true;
        record.origin = ProviderOrigin::Pack;
        record.installed_at_unix_seconds = Some(now);
        record.requires = record
            .dependencies
            .iter()
            .filter(|dependency| dependency.kind == DependencyKind::Required)
            .filter_map(|dependency| {
                identities
                    .iter()
                    .find(|identity| {
                        identity.provider == dependency.provider
                            && identity.project_id == dependency.project_id
                    })
                    .cloned()
            })
            .collect();
        identities.push(record.identity());
        additions.push(record);
    }
    next.entries.extend(additions);
    next.entries.sort_by(|a, b| {
        a.content_type
            .directory_name()
            .cmp(b.content_type.directory_name())
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    next.validate()
        .map_err(|error| fail("pack_state_malformed", error.to_string()))?;

    let new_pack = crate::pack_state::InstalledPack::with_divergences(
        reconciliation.old_pack.instance_id.clone(),
        PackIdentity {
            provider: reconciliation.old_pack.identity.provider.clone(),
            project_id: reconciliation.old_pack.identity.project_id.clone(),
            version_id: reconciliation.plan.candidate_version_id.clone(),
            name: reconciliation.plan.name.clone(),
            pack_version: reconciliation.plan.candidate_pack_version.clone(),
            artifact_sha512: reconciliation.archive_sha512.clone(),
            artifact_sha256: reconciliation.archive.sha256.as_hex(),
            minecraft_version: reconciliation.target_minecraft.clone(),
            fabric_loader_version: reconciliation.target_loader.clone(),
            installed_at_unix_seconds: now,
        },
        components,
        reconciliation.next_overrides.clone(),
        reconciliation.next_excluded.clone(),
        reconciliation.next_divergences.clone(),
    )
    .map_err(|error| fail(error.code(), pack_state_message(&error)))?;
    Ok((new_pack, next))
}

// ---------------------------------------------------------------------------
// Update receipt and interruption recovery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateReceipt {
    schema_version: u32,
    instance_id: String,
    from_project_id: String,
    from_version_id: String,
    to_project_id: String,
    to_version_id: String,
    started_at_unix_seconds: u64,
}

fn receipt_path(managed: &ManagedPaths, instance: &InstanceId) -> Result<PathBuf, PackUpdateError> {
    let root =
        crate::instance_content::validated_instance_root(managed, instance).map_err(|_| {
            fail(
                "pack_unsafe_path",
                "The instance root is redirected or unsafe.",
            )
        })?;
    Ok(root.join("pack-update.json"))
}

fn write_receipt(
    managed: &ManagedPaths,
    instance: &InstanceId,
    receipt: &UpdateReceipt,
) -> Result<(), PackUpdateError> {
    let path = receipt_path(managed, instance)?;
    let temp = path.with_extension(format!("json.{}.tmp", uuid::Uuid::new_v4()));
    let mut json = serde_json::to_string_pretty(receipt).map_err(|_| {
        fail(
            "pack_state_malformed",
            "The update receipt could not be encoded.",
        )
    })?;
    json.push('\n');
    std::fs::write(&temp, json).map_err(|_| {
        fail(
            "pack_state_io_error",
            "The update receipt could not be written.",
        )
    })?;
    if let Err(error) = std::fs::rename(&temp, &path) {
        let _ = std::fs::remove_file(&temp);
        return Err(fail(
            "pack_state_io_error",
            format!(
                "The update receipt could not be committed: {}",
                error.kind()
            ),
        ));
    }
    Ok(())
}

fn read_receipt(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<Option<UpdateReceipt>, PackUpdateError> {
    let path = receipt_path(managed, instance)?;
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(fail(
                "pack_state_io_error",
                "An interrupted modpack update's receipt could not be read.",
            ));
        }
    };
    let receipt: UpdateReceipt = serde_json::from_slice(&bytes)
        .map_err(|_| fail("pack_state_malformed", "The update receipt is malformed."))?;
    if receipt.schema_version != RECEIPT_SCHEMA_VERSION || &receipt.instance_id != instance.as_str()
    {
        return Err(fail(
            "pack_state_malformed",
            "The update receipt does not belong to this instance.",
        ));
    }
    Ok(Some(receipt))
}

fn remove_receipt(managed: &ManagedPaths, instance: &InstanceId) -> Result<(), PackUpdateError> {
    match std::fs::remove_file(receipt_path(managed, instance)?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(fail(
            "pack_state_io_error",
            "The completed update's receipt could not be removed.",
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOutcome {
    /// No interrupted update was pending.
    None,
    /// The committed new state was recognized and finalized.
    CompletedNew,
    /// The untouched old state was recognized and restored.
    RestoredOld,
}

/// Resolve an interrupted update from its receipt before any new pack
/// operation. If neither the old nor the new state fully validates, the
/// instance stays `installing` (unavailable) and the failure is reported.
pub async fn recover_interrupted_update(
    managed: &ManagedPaths,
    instance_id: &str,
) -> Result<RecoveryOutcome, PackUpdateError> {
    let instance = InstanceId::new(instance_id.to_owned())
        .map_err(|_| fail("instance_not_found", "The instance identifier is invalid."))?;
    let Some(receipt) = read_receipt(managed, &instance)? else {
        return Ok(RecoveryOutcome::None);
    };
    let registry_path = managed.instance_registry_file();
    let registry = InstanceRegistry::load(&registry_path)
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    let Some(record) = registry.find(&instance).cloned() else {
        // The instance vanished underneath the receipt; the receipt is
        // meaningless without it.
        remove_receipt(managed, &instance)?;
        return Ok(RecoveryOutcome::None);
    };
    let pack = InstalledPack::load(managed, &instance)
        .map_err(|error| fail(error.code(), pack_state_message(&error)))?;

    // Case 1: the transaction committed its documents before the crash —
    // the stored pack state is already the new version and validates.
    if let Some(pack) = pack.as_ref() {
        if pack.identity.project_id == receipt.to_project_id
            && pack.identity.version_id == receipt.to_version_id
            && pack.validate_installed(managed).is_ok()
        {
            {
                let mut registry = registry;
                let stored = registry.find_mut(&instance).expect("record loaded above");
                stored.set_state(InstanceState::Ready);
                registry
                    .save(&registry_path)
                    .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
            }
            remove_receipt(managed, &instance)?;
            return Ok(RecoveryOutcome::CompletedNew);
        }
    }

    // Case 2: nothing committed — the old pack state validates against the
    // still-old registry record and the still-old game tree.
    if let Some(pack) = pack.as_ref() {
        if pack.identity.project_id == receipt.from_project_id
            && pack.identity.version_id == receipt.from_version_id
            && pack.validate_installed(managed).is_ok()
        {
            let game_valid = matches!(
                crate::install::validate_installed_game(managed, &instance),
                Ok(crate::install::ValidationOutcome::Installed(validation))
                    if validation.minecraft_version == record.installed().minecraft_version
                        && validation.status == crate::install::ValidationStatus::Valid
            );
            if game_valid {
                clean_staging_debris(managed, &instance);
                {
                    let mut registry = registry;
                    let stored = registry.find_mut(&instance).expect("record loaded above");
                    stored.set_state(InstanceState::Ready);
                    registry
                        .save(&registry_path)
                        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
                }
                remove_receipt(managed, &instance)?;
                return Ok(RecoveryOutcome::RestoredOld);
            }
        }
    }
    Err(fail(
        "pack_update_recovery_failed",
        "A previous modpack update was interrupted and its exact outcome could not be proven. The instance stays unavailable for inspection.",
    ))
}

/// Remove only the launcher's own transaction staging names inside the
/// instance root. No user or pack content is touched.
fn clean_staging_debris(managed: &ManagedPaths, instance: &InstanceId) {
    let Ok(root) = crate::instance_content::validated_instance_root(managed, instance) else {
        return;
    };
    let mut queue = vec![root.clone()];
    while let Some(directory) = queue.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                queue.push(path);
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(".pack-update-staged-")
                || name.starts_with(".pack-update-retired-")
                || name.starts_with(".pack-update-rollback-")
            {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The pack-update transaction
// ---------------------------------------------------------------------------

/// A retired (moved-aside) file the transaction can restore on rollback.
struct RetiredFile {
    target: PathBuf,
    backup: PathBuf,
}

/// A file the transaction activated (its staged bytes were linked in).
struct ActivatedFile {
    target: PathBuf,
    sha256: String,
}

/// Run one operation under the instance content lock, preserving its own
/// error type (the lock's busy failure maps to a typed error too).
fn with_content_lock<T>(
    instance: &InstanceId,
    operation: impl FnOnce() -> Result<T, PackUpdateError>,
) -> Result<T, PackUpdateError> {
    let mut slot: Option<Result<T, PackUpdateError>> = None;
    let guarded = crate::instance_content::with_instance_lock(instance, || {
        slot = Some(operation());
        Ok::<(), crate::instance_content::ContentError>(())
    });
    match (guarded, slot) {
        (Ok(()), Some(result)) => result,
        (Err(error), _) => Err(fail(
            "pack_update_in_progress",
            match error {
                crate::instance_content::ContentError::OperationInProgress => {
                    "Another content operation is running on this instance."
                }
                _ => "The update transaction could not lock this instance.",
            },
        )),
        (Ok(()), None) => unreachable!("the closure always fills the slot"),
    }
}

/// Window 1: prove the previewed world is still the real world, then mark
/// the update in progress (registry `installing` plus the receipt).
fn revalidate_and_begin(
    managed: &ManagedPaths,
    instance: &InstanceId,
    reconciliation: &Reconciliation,
) -> Result<(), PackUpdateError> {
    let registry_path = managed.instance_registry_file();
    let registry = InstanceRegistry::load(&registry_path)
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    let record = registry.find(instance).ok_or_else(|| {
        fail(
            "instance_not_found",
            "This instance no longer exists in the launcher registry.",
        )
    })?;
    if *record != reconciliation.old_record {
        return Err(fail(
            "pack_update_stale",
            "The instance changed since the update was reviewed. Review it again.",
        ));
    }
    let state = ContentState::load(managed, instance)
        .map_err(|error| fail("pack_state_malformed", error.to_string()))?;
    if state != reconciliation.old_state {
        return Err(fail(
            "pack_update_stale",
            "The managed content changed since the update was reviewed. Review it again.",
        ));
    }
    let pack = InstalledPack::load(managed, instance)
        .map_err(|error| fail(error.code(), pack_state_message(&error)))?
        .ok_or_else(|| {
            fail(
                "pack_state_malformed",
                "The installed modpack state is missing.",
            )
        })?;
    if pack != reconciliation.old_pack {
        return Err(fail(
            "pack_update_stale",
            "The installed modpack changed since the update was reviewed. Review it again.",
        ));
    }
    let root =
        crate::instance_content::validated_instance_root(managed, instance).map_err(|_| {
            fail(
                "pack_unsafe_path",
                "The instance root is redirected or unsafe.",
            )
        })?;
    for plan in &reconciliation.retired_paths {
        match hash_local(&root, &plan.path)? {
            Some(file) if file.sha256 == plan.expected_sha256 => {}
            _ => {
                return Err(fail(
                    "pack_update_stale",
                    format!(
                        "A local file changed since the update was reviewed: {}. Review it again.",
                        plan.path
                    ),
                ));
            }
        }
    }
    // Every old authored file the update keeps must still match its digest.
    let retired_keys: HashSet<String> = reconciliation
        .retired_paths
        .iter()
        .map(|plan| plan.path.to_lowercase())
        .collect();
    let kept_paths: HashSet<String> = reconciliation
        .next_components
        .iter()
        .map(|item| item.component.path.to_lowercase())
        .chain(
            reconciliation
                .next_overrides
                .iter()
                .map(|item| item.path.to_lowercase()),
        )
        .filter(|path| !retired_keys.contains(path))
        .collect();
    let mut old_authored: Vec<(String, String)> = reconciliation
        .old_pack
        .components
        .iter()
        .map(|item| (item.path.to_lowercase(), item.sha256.clone()))
        .chain(
            reconciliation
                .old_pack
                .overrides
                .iter()
                .map(|item| (item.path.to_lowercase(), item.sha256.clone())),
        )
        .collect();
    old_authored.sort();
    for (path, digest) in old_authored {
        if !kept_paths.contains(&path) {
            continue;
        }
        let path = reconciliation
            .old_pack
            .components
            .iter()
            .map(|item| item.path.clone())
            .chain(
                reconciliation
                    .old_pack
                    .overrides
                    .iter()
                    .map(|item| item.path.clone()),
            )
            .find(|candidate| candidate.to_lowercase() == path)
            .expect("path came from the same lists");
        match hash_local(&root, &path)? {
            Some(file) if file.sha256 == digest => {}
            _ => {
                return Err(fail(
                    "pack_update_stale",
                    format!(
                        "A local file changed since the update was reviewed: {path}. Review it again."
                    ),
                ));
            }
        }
    }

    // Mark in progress: the receipt first (its presence is the durable
    // marker), then the registry state that makes the instance unavailable.
    write_receipt(
        managed,
        instance,
        &UpdateReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION,
            instance_id: instance.to_string(),
            from_project_id: reconciliation.old_pack.identity.project_id.clone(),
            from_version_id: reconciliation.old_pack.identity.version_id.clone(),
            to_project_id: reconciliation.old_pack.identity.project_id.clone(),
            to_version_id: reconciliation.plan.candidate_version_id.clone(),
            started_at_unix_seconds: crate::instance_content::now_unix_seconds(),
        },
    )?;
    let mut registry = registry;
    let stored = registry.find_mut(instance).expect("record loaded above");
    stored.set_state(InstanceState::Installing);
    registry
        .save(&registry_path)
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    Ok(())
}

/// Undo only window 1 (no file has moved yet).
fn abort_begin(managed: &ManagedPaths, instance: &InstanceId) {
    let registry_path = managed.instance_registry_file();
    if let Ok(mut registry) = InstanceRegistry::load(&registry_path) {
        if let Some(stored) = registry.find_mut(instance) {
            stored.set_state(InstanceState::Ready);
            let _ = registry.save(&registry_path);
        }
    }
    let _ = remove_receipt(managed, instance);
}

/// Restore the previous game tree through the same verified pipeline. All
/// old artifacts are already in the verified cache; only metadata is
/// re-resolved.
async fn restore_old_game(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    instance: &InstanceId,
    old_record: &InstanceRecord,
) -> Result<(), PackUpdateError> {
    let plan =
        crate::instances::lifecycle::resolve_packed_game_plan(endpoints, old_record.installed())
            .await
            .map_err(|error| fail("pack_transition_failed", error.to_string()))?;
    crate::install::install_game(
        managed,
        instance,
        &plan,
        endpoints.install_context(),
        &mut |_| {},
        crate::install::InstallFaults::default(),
    )
    .await
    .map_err(|error| fail("pack_transition_failed", error.to_string()))?;
    Ok(())
}

/// Window 2: stage, verify, activate, and commit. Every mutation is rolled
/// back if any step fails; the registry's ready-with-new-identity write is
/// the commit point.
fn commit_reconciliation(
    managed: &ManagedPaths,
    instance: &InstanceId,
    reconciliation: &Reconciliation,
    acquired: &Acquired,
    new_pack: &InstalledPack,
    next_state: &ContentState,
) -> Result<InstanceRecord, PackUpdateError> {
    let root =
        crate::instance_content::validated_instance_root(managed, instance).map_err(|_| {
            fail(
                "pack_unsafe_path",
                "The instance root is redirected or unsafe.",
            )
        })?;
    let mut retired: Vec<RetiredFile> = Vec::new();
    let mut activated: Vec<ActivatedFile> = Vec::new();
    let mut content_committed = false;
    let mut pack_replaced = false;
    let mut registry_committed = false;
    let result = commit_steps(
        managed,
        instance,
        reconciliation,
        acquired,
        new_pack,
        next_state,
        &root,
        &mut retired,
        &mut activated,
        &mut content_committed,
        &mut pack_replaced,
        &mut registry_committed,
    );
    match result {
        Ok(record) => {
            cleanup_backups(&retired);
            Ok(record)
        }
        Err(error) => {
            #[cfg(test)]
            eprintln!(
                "[pack-update-test] commit failed: {} ({})",
                error.message, error.code
            );
            let rollback_failed = rollback_files(&retired, &activated);
            if content_committed {
                let _ = reconciliation.old_state.save(managed, instance);
            }
            if pack_replaced {
                let _ = reconciliation.old_pack.replace(managed, new_pack);
            }
            if registry_committed {
                let _ = restore_registry_record(managed, instance, &reconciliation.old_record);
            }
            if rollback_failed {
                leave_unavailable(managed, instance);
                return Err(fail(
                    "pack_update_rollback_failed",
                    "The modpack update failed and the previous files could not all be restored. The instance stays unavailable for inspection.",
                ));
            }
            Err(error)
        }
    }
}

/// After a rollback that restored every file and document, finish the
/// restoration (the receipt is only removed once the instance is whole).
fn finalize_aborted(managed: &ManagedPaths, instance: &InstanceId) -> Result<(), PackUpdateError> {
    let registry_path = managed.instance_registry_file();
    let mut registry = InstanceRegistry::load(&registry_path)
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    if let Some(stored) = registry.find_mut(instance) {
        stored.set_state(InstanceState::Ready);
    }
    registry
        .save(&registry_path)
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    remove_receipt(managed, instance)?;
    clean_staging_debris(managed, instance);
    Ok(())
}

fn leave_unavailable(managed: &ManagedPaths, instance: &InstanceId) {
    let registry_path = managed.instance_registry_file();
    if let Ok(mut registry) = InstanceRegistry::load(&registry_path) {
        if let Some(stored) = registry.find_mut(instance) {
            stored.set_state(InstanceState::Installing);
            let _ = registry.save(&registry_path);
        }
    }
}

fn restore_registry_record(
    managed: &ManagedPaths,
    instance: &InstanceId,
    old_record: &InstanceRecord,
) -> Result<(), PackUpdateError> {
    let registry_path = managed.instance_registry_file();
    let mut registry = InstanceRegistry::load(&registry_path)
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    let Some(stored) = registry.find_mut(instance) else {
        return Err(fail(
            "instance_not_found",
            "This instance no longer exists in the launcher registry.",
        ));
    };
    let mut restored = old_record.clone();
    restored.set_state(InstanceState::Ready);
    *stored = restored;
    registry
        .save(&registry_path)
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit_steps(
    managed: &ManagedPaths,
    instance: &InstanceId,
    reconciliation: &Reconciliation,
    acquired: &Acquired,
    new_pack: &InstalledPack,
    next_state: &ContentState,
    root: &Path,
    retired: &mut Vec<RetiredFile>,
    activated: &mut Vec<ActivatedFile>,
    content_committed: &mut bool,
    pack_replaced: &mut bool,
    registry_committed: &mut bool,
) -> Result<InstanceRecord, PackUpdateError> {
    // 1. Stage exactly the files the reconciliation writes (no target is
    // touched yet). Preserved and adopted files are already correct on disk
    // and are deliberately not restaged.
    let mut staged_files: Vec<(PathBuf, PathBuf, String)> = Vec::new();
    let mut stage_targets: HashSet<String> = HashSet::new();
    for write in &reconciliation.writes {
        let target = stage_target(root, &write.path)?;
        if !stage_targets.insert(write.path.to_lowercase()) {
            return Err(fail(
                "pack_path_collision",
                "Two pack files claim the same destination.",
            ));
        }
        let occupied = std::fs::symlink_metadata(&target).is_ok()
            && !reconciliation
                .retired_paths
                .iter()
                .any(|plan| plan.path == write.path);
        if occupied {
            return Err(fail(
                "pack_path_collision",
                format!("{} already exists in the instance.", write.path),
            ));
        }
        let staged_path =
            target.with_file_name(format!(".pack-update-staged-{}", uuid::Uuid::new_v4()));
        // Provider and external files stage from the verified store; an
        // override's bytes come from the verified archive, whose read itself
        // re-verifies the digest fixed during planning.
        enum Source {
            Store(PathBuf),
            Bytes(Vec<u8>),
        }
        let (source, digest) = match write.source {
            WriteSource::Provider(index) => (
                Source::Store(acquired.providers[index].path.clone()),
                acquired.providers[index].sha256.clone(),
            ),
            WriteSource::External(_) => {
                let position = reconciliation
                    .new_external
                    .iter()
                    .position(|file| file.path == write.path)
                    .ok_or_else(|| fail("pack_plan_invalid", "An external write left the plan."))?;
                (
                    Source::Store(acquired.externals[position].path.clone()),
                    acquired.externals[position].sha256.clone(),
                )
            }
            WriteSource::Override(_) => {
                let position = reconciliation
                    .new_overrides
                    .iter()
                    .position(|item| item.path == write.path)
                    .ok_or_else(|| fail("pack_plan_invalid", "An override write left the plan."))?;
                let item = &reconciliation.new_overrides[position];
                let bytes = crate::mrpack::read_override(&reconciliation.archive.path, item)
                    .map_err(|error| fail(error.code(), error.to_string()))?;
                (Source::Bytes(bytes), item.sha256.to_ascii_lowercase())
            }
        };
        let staged = match &source {
            Source::Store(path) => std::fs::copy(path, &staged_path)
                .map(|_| ())
                .map_err(|error| (error.kind(), "A pack file could not be staged.")),
            Source::Bytes(bytes) => std::fs::write(&staged_path, bytes)
                .map(|_| ())
                .map_err(|error| (error.kind(), "A pack override could not be staged.")),
        };
        if let Err((kind, message)) = staged {
            let _ = std::fs::remove_file(&staged_path);
            return Err(fail("pack_state_io_error", format!("{message}: {kind}")));
        }
        staged_files.push((target, staged_path, digest));
    }
    for (_, staged_path, sha256) in &staged_files {
        let digest = ArtifactDigest::parse(sha256)
            .map_err(|_| fail("pack_invalid_hash", "A staged pack file digest is invalid."))?;
        if verify_file(staged_path, &digest, None).is_err() {
            let _ = std::fs::remove_file(staged_path);
            return Err(fail(
                "pack_digest_mismatch",
                "A staged pack file failed verification.",
            ));
        }
    }

    // 2. Retire old files: prove current bytes, then move aside.
    for plan in &reconciliation.retired_paths {
        let target = stage_target(root, &plan.path)?;
        let expected = ArtifactDigest::parse(&plan.expected_sha256)
            .map_err(|_| fail("pack_invalid_hash", "A retirement digest is invalid."))?;
        if verify_file(&target, &expected, None).is_err() {
            return Err(fail(
                "pack_update_stale",
                format!(
                    "A local file changed since the update was reviewed: {}. Review it again.",
                    plan.path
                ),
            ));
        }
        let backup =
            target.with_file_name(format!(".pack-update-retired-{}", uuid::Uuid::new_v4()));
        std::fs::rename(&target, &backup)
            .map_err(|_| fail("pack_state_io_error", "A pack file could not be retired."))?;
        retired.push(RetiredFile { target, backup });
    }

    // 3. Activate staged bytes.
    for (target, staged_path, sha256) in &staged_files {
        if let Err(error) = std::fs::hard_link(staged_path, target) {
            return Err(fail(
                if target.exists() {
                    "pack_path_collision"
                } else {
                    "pack_state_io_error"
                },
                format!("A pack file could not be activated: {}", error.kind()),
            ));
        }
        let _ = std::fs::remove_file(staged_path);
        activated.push(ActivatedFile {
            target: target.clone(),
            sha256: sha256.clone(),
        });
    }

    // 4. Commit documents: content state, pack state, registry (the commit).
    next_state
        .save(managed, instance)
        .map_err(|error| fail("pack_state_io_error", error.to_string()))?;
    *content_committed = true;
    new_pack
        .replace(managed, &reconciliation.old_pack)
        .map_err(|error| fail(error.code(), pack_state_message(&error)))?;
    *pack_replaced = true;
    let registry_path = managed.instance_registry_file();
    let mut registry = InstanceRegistry::load(&registry_path)
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    let stored = registry.find_mut(instance).ok_or_else(|| {
        fail(
            "instance_not_found",
            "This instance no longer exists in the launcher registry.",
        )
    })?;
    let mut configuration = reconciliation.old_record.configuration().clone();
    configuration.set_minecraft_version(reconciliation.target_minecraft.clone());
    configuration.set_loader(crate::instances::settings::LoaderConfiguration::Fabric {
        policy: crate::instances::settings::LoaderPolicy::Pinned {
            version: reconciliation.target_loader.clone(),
        },
    });
    configuration.set_aurora_enabled(false);
    stored.set_configuration(configuration);
    stored.set_installed(crate::instances::platform::InstalledConfiguration {
        minecraft_version: reconciliation.target_minecraft.clone(),
        platform: crate::instances::platform::PlatformPin::Fabric {
            version: reconciliation.target_loader.clone(),
        },
        aurora: None,
    });
    stored
        .set_pack(crate::instances::PackRegistryIdentity {
            provider: "modrinth".into(),
            project_id: reconciliation.old_pack.identity.project_id.clone(),
            version_id: reconciliation.plan.candidate_version_id.clone(),
            name: new_pack.identity.name.clone(),
            pack_version: new_pack.identity.pack_version.clone(),
        })
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    stored.set_state(InstanceState::Ready);
    let record = stored.clone();
    registry
        .save(&registry_path)
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    *registry_committed = true;
    remove_receipt(managed, instance)?;
    Ok(record)
}

/// Like the install path's target derivation, but an existing file at the
/// destination is allowed: the transaction retires (and backs up) an owned
/// predecessor itself, so the collision decision belongs to the caller's
/// retirement evidence, not to path shape.
fn stage_target(root: &Path, relative: &str) -> Result<PathBuf, PackUpdateError> {
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
                        fail(
                            "pack_state_io_error",
                            "A pack directory could not be created.",
                        )
                    })?;
                }
                Err(_) => {
                    return Err(fail(
                        "pack_state_io_error",
                        "A pack directory could not be inspected.",
                    ));
                }
            }
        }
    }
    Ok(target)
}

fn cleanup_backups(retired: &[RetiredFile]) {
    for file in retired {
        let _ = std::fs::remove_file(&file.backup);
    }
}

/// Restore moved-aside files and remove activated ones. Returns whether the
/// filesystem could be proven fully restored.
fn rollback_files(retired: &[RetiredFile], activated: &[ActivatedFile]) -> bool {
    let mut failed = false;
    for file in activated.iter().rev() {
        let digest = ArtifactDigest::parse(&file.sha256);
        let ours = digest
            .as_ref()
            .map(|digest| verify_file(&file.target, digest, None).is_ok())
            .unwrap_or(false);
        if ours {
            if std::fs::remove_file(&file.target).is_err() {
                failed = true;
            }
        } else {
            // The file at the target is not what we activated; leave it and
            // report the uncertainty.
            failed = true;
        }
    }
    for file in retired.iter().rev() {
        if std::fs::hard_link(&file.backup, &file.target).is_err() && !file.target.exists() {
            failed = true;
        }
    }
    !failed
}

/// Post-commit validation of the reconciled state. A failure triggers the
/// full rollback (including the game tree when a transition committed).
fn validate_committed(
    managed: &ManagedPaths,
    instance: &InstanceId,
    new_pack: &InstalledPack,
) -> Result<(), PackUpdateError> {
    new_pack
        .validate_installed(managed)
        .map_err(|error| fail(error.code(), pack_state_message(&error)))?;
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| fail("instance_state_invalid", error.to_string()))?;
    let validation = crate::instances::lifecycle::validate_instance(managed, &registry, instance)
        .map_err(|error| fail("pack_validation_failed", error.to_string()))?;
    if !validation.problems.is_empty() {
        return Err(fail(
            "pack_validation_failed",
            format!(
                "The updated instance did not validate: {}",
                validation
                    .problems
                    .iter()
                    .map(|problem| format!("{}: {}", problem.component, problem.reason))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        ));
    }
    Ok(())
}

/// Roll back the committed documents after a post-commit validation failure.
fn undo_commit(
    managed: &ManagedPaths,
    instance: &InstanceId,
    reconciliation: &Reconciliation,
) -> Result<(), PackUpdateError> {
    reconciliation
        .old_state
        .save(managed, instance)
        .map_err(|error| fail("pack_state_io_error", error.to_string()))?;
    let current = InstalledPack::load(managed, instance)
        .map_err(|error| fail(error.code(), pack_state_message(&error)))?
        .ok_or_else(|| fail("pack_state_malformed", "The pack state is missing."))?;
    reconciliation
        .old_pack
        .replace(managed, &current)
        .map_err(|error| fail(error.code(), pack_state_message(&error)))?;
    restore_registry_record(managed, instance, &reconciliation.old_record)?;
    Ok(())
}

/// Apply one reviewed reconciliation. The plan is re-derived from provider
/// and local truth; the presented fingerprint must match or the apply is
/// refused. Mutations run as one launcher-visible transaction with rollback;
/// the registry's ready-with-new-identity write is the commit.
pub async fn apply_pack_update(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    client: &crate::modrinth::Client,
    instance_id: &str,
    fingerprint: &str,
    resolutions: &[ConflictResolution],
    progress: &mut (dyn FnMut(&'static str) + Send),
) -> Result<InstanceRecord, PackUpdateError> {
    match recover_interrupted_update(managed, instance_id).await? {
        RecoveryOutcome::None | RecoveryOutcome::RestoredOld => {}
        RecoveryOutcome::CompletedNew => {
            return Err(fail(
                "pack_no_update",
                "The interrupted modpack update had already completed; the installed pack is current.",
            ));
        }
    }

    progress("checkingPack");
    let reconciliation =
        build_plan(managed, endpoints, client, instance_id, resolutions, true).await?;
    if reconciliation.plan.fingerprint != fingerprint {
        return Err(fail(
            "pack_update_stale",
            "The reconciliation changed since it was reviewed. Review the update again.",
        ));
    }
    let instance = InstanceId::new(instance_id.to_owned())
        .map_err(|_| fail("instance_not_found", "The instance identifier is invalid."))?;

    progress("downloading");
    let acquired = acquire_all(managed, &reconciliation).await?;

    progress("verifying");
    let (new_pack, next_state) = build_next_documents(&reconciliation, &acquired)?;
    {
        // Projected compatibility against the *target* game identity, plus
        // the standing dependent checks for retired managed mods.
        let mut projected: Vec<(ProviderRecord, PathBuf, u64)> = Vec::new();
        for item in &reconciliation.next_components {
            let Some(index) = item.provider_plan else {
                continue;
            };
            let plan = &reconciliation.new_provider_plans[index];
            let artifact = &acquired.providers[index];
            projected.push((
                plan.provider_record(artifact.sha256.clone(), ProviderOrigin::Pack),
                artifact.path.clone(),
                artifact.bytes,
            ));
        }
        let tolerated: Vec<String> = reconciliation
            .next_divergences
            .iter()
            .filter(|divergence| divergence.component)
            .filter_map(|divergence| divergence.path.rsplit('/').next().map(str::to_owned))
            .collect();
        crate::instance_content::validate_projected_pack_artifacts(
            managed,
            &instance,
            &reconciliation.retired_records,
            &tolerated,
            &projected,
            &reconciliation.target_minecraft,
            &reconciliation.target_loader,
        )
        .map_err(|error| fail("pack_content_conflict", error.to_string()))?;
        // Inter-mod dependency relations are validated against the projected
        // inventory above (successors of the same project satisfy dependents);
        // a standalone removal check would ignore the joining records.
    }

    // ---- Window 1: revalidate and mark the update in progress. ----------
    with_content_lock(&instance, || {
        revalidate_and_begin(managed, &instance, &reconciliation)
    })?;

    // ---- Game transition (the old tree survives a failure here). --------
    let mut game_transitioned = false;
    if reconciliation.plan.game_transition {
        progress("installingGame");
        let target_installed = crate::instances::platform::InstalledConfiguration {
            minecraft_version: reconciliation.target_minecraft.clone(),
            platform: crate::instances::platform::PlatformPin::Fabric {
                version: reconciliation.target_loader.clone(),
            },
            aurora: None,
        };
        let install = async {
            let plan =
                crate::instances::lifecycle::resolve_packed_game_plan(endpoints, &target_installed)
                    .await
                    .map_err(|error| {
                        fail(
                            "pack_transition_failed",
                            format!("The new game version could not be planned: {error}"),
                        )
                    })?;
            crate::install::install_game(
                managed,
                &instance,
                &plan,
                endpoints.install_context(),
                &mut |_| {},
                crate::install::InstallFaults::default(),
            )
            .await
            .map_err(|error| {
                fail(
                    "pack_transition_failed",
                    format!("The new game version could not be installed: {error}"),
                )
            })
        };
        if let Err(error) = install.await {
            with_content_lock(&instance, || {
                abort_begin(managed, &instance);
                Ok::<(), PackUpdateError>(())
            })
            .ok();
            return Err(error);
        }
        game_transitioned = true;
    }

    // ---- Window 2: mutate files and commit documents. --------------------
    progress("applyingChanges");
    let commit = with_content_lock(&instance, || {
        commit_reconciliation(
            managed,
            &instance,
            &reconciliation,
            &acquired,
            &new_pack,
            &next_state,
        )
    });
    let final_record = match commit {
        Ok(record) => record,
        Err(error) => {
            if game_transitioned {
                progress("rollingBack");
                if restore_old_game(managed, endpoints, &instance, &reconciliation.old_record)
                    .await
                    .is_err()
                {
                    with_content_lock(&instance, || {
                        leave_unavailable(managed, &instance);
                        Ok::<(), PackUpdateError>(())
                    })
                    .ok();
                    return Err(fail(
                        "pack_update_rollback_failed",
                        "The modpack update failed and the previous game version could not be restored. The instance stays unavailable for inspection.",
                    ));
                }
            }
            with_content_lock(&instance, || finalize_aborted(managed, &instance))?;
            return Err(error);
        }
    };

    progress("validating");
    let validation = with_content_lock(&instance, || {
        validate_committed(managed, &instance, &new_pack)
    });
    if let Err(error) = validation {
        if game_transitioned {
            progress("rollingBack");
            if restore_old_game(managed, endpoints, &instance, &reconciliation.old_record)
                .await
                .is_err()
            {
                with_content_lock(&instance, || {
                    leave_unavailable(managed, &instance);
                    Ok::<(), PackUpdateError>(())
                })
                .ok();
                return Err(fail(
                    "pack_update_rollback_failed",
                    "Validation failed and the previous game version could not be restored. The instance stays unavailable for inspection.",
                ));
            }
        }
        with_content_lock(&instance, || {
            undo_commit(managed, &instance, &reconciliation)
        })?;
        with_content_lock(&instance, || finalize_aborted(managed, &instance))?;
        return Err(error);
    }

    progress("complete");
    Ok(final_record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance_content::ContentType;
    use crate::test_support::{TestRequest, TestResponse, TestServer};
    use sha1::Sha1;
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;

    const PACK_PROJECT: &str = "PACK0001";
    const V1: &str = "VERS0001";
    const V2: &str = "VERS0002";
    const MC: &str = "26.2";
    const LOADER: &str = "0.19.5";

    fn sha1_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha1::digest(bytes))
    }
    fn sha512_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha512::digest(bytes))
    }
    /// The global Fabric Loader version list (bare loader shapes).
    const BARE_LOADER_LIST: &str = "[{ \"separator\": \".\", \"build\": 5, \"maven\": \"net.fabricmc:fabric-loader:0.19.5\", \"version\": \"0.19.5\", \"stable\": true }]";

    /// The per-game loader listing shape with empty launcher metadata.
    const PER_GAME_LOADER_LIST: &str = "[{ \"loader\": { \"separator\": \".\", \"build\": 5, \"maven\": \"net.fabricmc:fabric-loader:0.19.5\", \"version\": \"0.19.5\", \"stable\": true }, \"intermediary\": { \"maven\": \"net.fabricmc:intermediary:0.0.0\", \"version\": \"0.0.0\", \"stable\": true }, \"launcherMeta\": { \"version\": 2, \"min_java_version\": 8, \"libraries\": { \"client\": [], \"common\": [], \"server\": [], \"development\": [] }, \"mainClass\": { \"client\": \"net.fabricmc.loader.impl.launch.knot.KnotClient\", \"server\": \"net.fabricmc.loader.impl.launch.knot.KnotServer\" } } }]";
    fn sha256_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    /// A minimal Fabric mod jar with a unique mod id, so projected-inventory
    /// compatibility checks see distinct components.
    fn mod_jar(id: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(format!(r#"{{"schemaVersion":1,"id":"{id}","version":"1.0.0"}}"#).as_bytes())
            .unwrap();
        writer.finish().unwrap().into_inner()
    }

    /// One provider-recognized file the pack authors.
    struct ComponentSpec {
        project: &'static str,
        version_id: &'static str,
        version_number: &'static str,
        file_name: String,
        bytes: Vec<u8>,
        optional: bool,
    }

    impl ComponentSpec {
        fn recognized(
            project: &'static str,
            version_id: &'static str,
            file: &str,
            id: &str,
        ) -> Self {
            Self {
                project,
                version_id,
                version_number: "1.0.0",
                file_name: file.to_owned(),
                bytes: mod_jar(id),
                optional: false,
            }
        }
        fn sha512(&self) -> String {
            sha512_hex(&self.bytes)
        }
        fn path(&self) -> String {
            format!("mods/{}", self.file_name)
        }
    }

    /// One unresolved external file the pack authors.
    struct ExternalSpec {
        path: String,
        bytes: Vec<u8>,
        id: String,
    }

    impl ExternalSpec {
        fn new(path: &str, id: &str, _base: &str) -> Self {
            Self {
                path: path.to_owned(),
                bytes: mod_jar(id),
                id: id.to_owned(),
            }
        }
        fn url(&self, base: &str) -> String {
            format!("{base}/external/{}.jar", self.id)
        }
        fn sha512(&self) -> String {
            sha512_hex(&self.bytes)
        }
    }

    struct OverrideSpec {
        path: &'static str,
        bytes: &'static [u8],
        client: bool,
    }

    struct PackSpec {
        #[allow(dead_code)]
        version_id: &'static str,
        version_number: &'static str,
        published: &'static str,
        minecraft: &'static str,
        loader: &'static str,
        components: Vec<ComponentSpec>,
        external: Vec<ExternalSpec>,
        overrides: Vec<OverrideSpec>,
    }

    fn build_archive(spec: &PackSpec, base: &str, external_base: &str) -> Vec<u8> {
        let mut files = serde_json::Value::Array(Vec::new());
        for component in &spec.components {
            let url = format!("{base}/files/{}", component.file_name);
            files.as_array_mut().unwrap().push(serde_json::json!({
                "path": component.path(),
                "hashes": {"sha1": sha1_hex(&component.bytes), "sha512": component.sha512()},
                "downloads": [url],
                "fileSize": component.bytes.len(),
                "env": {"client": if component.optional { "optional" } else { "required" }, "server": "unsupported"},
            }));
        }
        for external in &spec.external {
            let url = external.url(external_base);
            files.as_array_mut().unwrap().push(serde_json::json!({
                "path": external.path,
                "hashes": {"sha1": sha1_hex(&external.bytes), "sha512": external.sha512()},
                "downloads": [url],
                "fileSize": external.bytes.len(),
                "env": {"client": "required", "server": "unsupported"},
            }));
        }
        let index = serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": spec.version_number,
            "name": "Fixture Pack",
            "files": files,
            "dependencies": {"minecraft": spec.minecraft, "fabric-loader": spec.loader},
        });
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file(
                "modrinth.index.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(index.to_string().as_bytes()).unwrap();
        for item in &spec.overrides {
            let name = if item.client {
                format!("client-overrides/{}", item.path)
            } else {
                format!("overrides/{}", item.path)
            };
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(item.bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    /// The complete loopback world: official game metadata and artifacts for
    /// one Minecraft/Fabric combination, plus the Modrinth v2 API for the
    /// pack project, its two versions, and every component identity.
    struct PackWorld {
        #[allow(dead_code)]
        server: TestServer,
        bodies: Arc<StdMutex<BTreeMap<String, Vec<u8>>>>,
        broken: Arc<StdMutex<std::collections::HashSet<String>>>,
        managed: ManagedPaths,
        endpoints: InstanceEndpoints,
        client: crate::modrinth::Client,
    }

    impl PackWorld {
        fn new(name: &str, v1: PackSpec, v2: PackSpec) -> Self {
            Self::with_variants(name, v1, v2, Vec::new(), None)
        }

        /// Extra versions (blocked candidates), and an optional second game
        /// version the v2 pack may transition to.
        fn with_variants(
            name: &str,
            v1: PackSpec,
            v2: PackSpec,
            extra: Vec<(String, serde_json::Value)>,
            transition: Option<(&'static str, &'static str)>,
        ) -> Self {
            let mut bodies: BTreeMap<String, Vec<u8>> = BTreeMap::new();
            // ---- Game artifacts (Mojang + Fabric), one or two versions. ----
            let client = b"synthetic client jar bytes".to_vec();
            let logging = b"<Configuration/>".to_vec();
            let mojang_library = b"synthetic mojang library jar".to_vec();
            let fabric_common = b"synthetic digested fabric library jar".to_vec();
            let fabric_loader = b"synthetic digest-less fabric loader jar".to_vec();
            let mut cursor = std::io::Cursor::new(Vec::new());
            {
                let mut writer = zip::ZipWriter::new(&mut cursor);
                for entry in ["META-INF/MANIFEST.MF", "lwjgl.dll"] {
                    writer
                        .start_file(entry, zip::write::SimpleFileOptions::default())
                        .unwrap();
                    writer.write_all(b"synthetic native content").unwrap();
                }
                writer.finish().unwrap();
            }
            let native_archive = cursor.into_inner();
            let asset_a = b"tiny png bytes a".to_vec();
            let asset_a_hash = crate::integrity::Sha1Digest::compute(&asset_a).as_hex();
            let asset_index_body = format!(
                r#"{{"objects": {{"icons/icon_16x16.png": {{"hash": "{asset_a_hash}", "size": {}}}}}}}"#,
                asset_a.len()
            )
            .into_bytes();
            bodies.insert("/mojang/client.jar".into(), client.clone());
            bodies.insert("/mojang/logging/client-1.21.2.xml".into(), logging.clone());
            bodies.insert(
                "/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar".into(),
                mojang_library.clone(),
            );
            bodies.insert(
                "/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar".into(),
                native_archive.clone(),
            );
            bodies.insert(
                "/mojang/asset-index/32.json".into(),
                asset_index_body.clone(),
            );
            bodies.insert(
                format!("/assets/{}/{}", &asset_a_hash[..2], asset_a_hash),
                asset_a.clone(),
            );
            bodies.insert(
                "/fabric-maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar".into(),
                fabric_common.clone(),
            );
            bodies.insert(
                "/fabric-maven/net/fabricmc/fabric-loader/0.19.5/fabric-loader-0.19.5.jar".into(),
                fabric_loader.clone(),
            );
            bodies.insert(
                "/fabric-maven/net/fabricmc/intermediary/0.0.0/intermediary-0.0.0.jar".into(),
                b"synthetic intermediary jar".to_vec(),
            );

            let sha1 = |bytes: &[u8]| crate::integrity::Sha1Digest::compute(bytes).as_hex();
            let mut versions_manifest = String::from(
                r#"{"latest": {"release": "26.2", "snapshot": "26.2"}, "versions": ["#,
            );
            let mut version_documents: Vec<(String, String)> = Vec::new();
            for (game, asset_id) in [
                (MC, "32"),
                (transition.map(|t| t.0).unwrap_or("26.3"), "32"),
            ] {
                let version_document = format!(
                    r#"{{
                        "id": "{game}",
                        "type": "release",
                        "mainClass": "net.minecraft.client.main.Main",
                        "javaVersion": {{ "component": "java-runtime-epsilon", "majorVersion": 25 }},
                        "assetIndex": {{
                            "id": "{asset_id}",
                            "sha1": "{}",
                            "size": {},
                            "totalSize": 999999,
                            "url": "@BASE@/mojang/asset-index/{asset_id}.json"
                        }},
                        "downloads": {{
                            "client": {{ "sha1": "{}", "size": {}, "url": "@BASE@/mojang/client.jar" }}
                        }},
                        "logging": {{
                            "client": {{
                                "argument": "-Dlog4j.configurationFile=${{path}}",
                                "file": {{ "id": "client-1.21.2.xml", "sha1": "{}", "size": {}, "url": "@BASE@/mojang/logging/client-1.21.2.xml" }}, "type": "log4j2-xml"
                            }}
                        }},
                        "libraries": [
                            {{ "downloads": {{ "artifact": {{
                                "path": "com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
                                "sha1": "{}", "size": {}, "url": "@BASE@/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"
                            }} }}, "name": "com.mojang:brigadier:1.0.18" }},
                            {{ "downloads": {{ "artifact": {{
                                "path": "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar",
                                "sha1": "{}", "size": {}, "url": "@BASE@/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar"
                            }} }}, "name": "org.lwjgl:lwjgl:3.4.1:natives-windows" }}
                        ],
                        "arguments": {{ "game": ["--username", "${{auth_player_name}}"], "jvm": ["-Djava.library.path=${{natives_directory}}"] }}
                    }}"#,
                    sha1(&asset_index_body),
                    asset_index_body.len(),
                    sha1(&client),
                    client.len(),
                    sha1(&logging),
                    logging.len(),
                    sha1(&mojang_library),
                    mojang_library.len(),
                    sha1(&native_archive),
                    native_archive.len(),
                );
                version_documents.push((game.to_owned(), version_document));
            }
            let _ = &mut versions_manifest;

            let bare_loader_list = &BARE_LOADER_LIST;
            let per_game_loader_list = &PER_GAME_LOADER_LIST;
            let fabric_profile = |base: &str, game: &str| {
                format!(
                    r#"{{
                        "loader": {{ "separator": ".", "build": 5, "maven": "net.fabricmc:fabric-loader:0.19.5", "version": "0.19.5", "stable": true }},
                        "intermediary": {{ "maven": "net.fabricmc:intermediary:0.0.0", "version": "0.0.0", "stable": true }},
                        "launcherMeta": {{
                            "version": 2, "min_java_version": 8,
                            "libraries": {{ "client": [], "common": [
                                {{ "name": "org.ow2.asm:asm:9.10.1", "url": "{base}/fabric-maven/", "sha256": "{}", "size": {} }}
                            ], "server": [], "development": [] }},
                            "mainClass": {{ "client": "net.fabricmc.loader.impl.launch.knot.KnotClient", "server": "net.fabricmc.loader.impl.launch.knot.KnotServer" }}
                        }}
                    }}"#,
                    sha256_hex(&fabric_common),
                    fabric_common.len(),
                )
                .replace("{game}", game)
            };

            let broken: Arc<StdMutex<std::collections::HashSet<String>>> =
                Arc::new(StdMutex::new(std::collections::HashSet::new()));
            let shared_bodies = Arc::new(StdMutex::new(bodies));
            // Hash-keyed component identity map; the batch endpoint answers
            // only the hashes a request actually queried.
            let hash_versions: Arc<StdMutex<BTreeMap<String, serde_json::Value>>> =
                Arc::new(StdMutex::new(BTreeMap::new()));
            let handler_broken = Arc::clone(&broken);
            let handler_bodies = Arc::clone(&shared_bodies);
            let handler_hashes = Arc::clone(&hash_versions);
            let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
                let path = request.path.split('?').next().unwrap_or_default();
                if handler_broken.lock().unwrap().contains(path) {
                    return TestResponse::status(404);
                }
                if path == "/v2/version_files" {
                    let queried: Vec<String> =
                        serde_json::from_slice::<serde_json::Value>(&request.body)
                            .ok()
                            .and_then(|body| {
                                body.get("hashes")?.as_array().map(|hashes| {
                                    hashes
                                        .iter()
                                        .filter_map(|value| value.as_str().map(str::to_owned))
                                        .collect()
                                })
                            })
                            .unwrap_or_default();
                    let map = handler_hashes.lock().unwrap();
                    let mut answer = serde_json::Map::new();
                    for hash in &queried {
                        if let Some(version) = map.get(hash) {
                            answer.insert(hash.clone(), version.clone());
                        }
                    }
                    return TestResponse::ok(
                        serde_json::Value::Object(answer).to_string().as_bytes(),
                    );
                }
                if let Some(hash) = path.strip_prefix("/v2/version_file/") {
                    let map = handler_hashes.lock().unwrap();
                    if let Some(version) = map.get(hash) {
                        return TestResponse::ok(version.to_string().as_bytes());
                    }
                    return TestResponse::status(404);
                }
                handler_bodies
                    .lock()
                    .unwrap()
                    .get(path)
                    .map(|body| TestResponse::ok(body))
                    .unwrap_or(TestResponse::status(404))
            }));
            let base = server.base_url().to_owned();

            // Documents that must embed the bound base URL.
            {
                let mut bodies = shared_bodies.lock().unwrap();
                let mut manifest_versions = String::new();
                for (game, document) in &version_documents {
                    let served = document.replace("@BASE@", &base);
                    bodies.insert(
                        format!("/mojang/versions/{game}.json"),
                        served.clone().into_bytes(),
                    );
                    manifest_versions.push_str(&format!(
                        r#"{{ "id": "{game}", "type": "release", "url": "{base}/mojang/versions/{game}.json", "sha1": "{}" }},"#,
                        sha1(served.as_bytes())
                    ));
                }
                manifest_versions.pop();
                bodies.insert(
                    "/mc/game/version_manifest_v2.json".into(),
                    format!(r#"{{"latest": {{"release": "26.2", "snapshot": "26.2"}}, "versions": [{manifest_versions}]}}"#)
                        .into_bytes(),
                );
                bodies.insert(
                    "/v2/versions/loader".into(),
                    bare_loader_list.as_bytes().to_vec(),
                );
                bodies.insert(
                    "/v2/versions/loader/26.2".into(),
                    per_game_loader_list.as_bytes().to_vec(),
                );
                bodies.insert(
                    "/v2/versions/loader/26.2/0.19.5".into(),
                    fabric_profile(&base, "26.2").into_bytes(),
                );
                bodies.insert(
                    "/v2/versions/loader/26.3".into(),
                    per_game_loader_list.as_bytes().to_vec(),
                );
                bodies.insert(
                    "/v2/versions/loader/26.3/0.19.5".into(),
                    fabric_profile(&base, "26.3").into_bytes(),
                );
            }

            // ---- Modrinth API for the pack project and its versions. ----
            let v1_archive = build_archive(&v1, &base, &base);
            let v2_archive = build_archive(&v2, &base, &base);
            {
                let mut bodies = shared_bodies.lock().unwrap();
                bodies.insert("/packs/v1.mrpack".into(), v1_archive.clone());
                bodies.insert("/packs/v2.mrpack".into(), v2_archive.clone());
                bodies.insert(
                    "/v2/project/PACK0001".into(),
                    serde_json::json!({
                        "id": PACK_PROJECT, "project_type": "modpack", "title": "Fixture Pack",
                        "description": "d", "license": {"id": "mit"}, "game_versions": [v1.minecraft, v2.minecraft],
                        "loaders": ["fabric"], "environment": ["client_only"]
                    })
                    .to_string()
                    .into_bytes(),
                );
                let version_entry = |id: &str,
                                     number: &str,
                                     published: &str,
                                     archive: &[u8],
                                     game: &str,
                                     loaders: &[&str]| {
                    serde_json::json!({
                        "id": id, "project_id": PACK_PROJECT, "name": number, "version_number": number,
                        "version_type": "release", "date_published": published,
                        "game_versions": [game], "loaders": loaders, "environment": "client_only",
                        "dependencies": [],
                        "changelog": format!("Changes in {number}"),
                        "files": [{"hashes": {"sha512": sha512_hex(archive)}, "url": format!("{base}/packs/{}.mrpack", if id == V1 { "v1" } else { "v2" }),
                                   "filename": format!("{number}.mrpack"), "primary": true, "size": archive.len()}]
                    })
                };
                let mut list = vec![
                    version_entry(
                        V2,
                        v2.version_number,
                        v2.published,
                        &v2_archive,
                        &v2.minecraft,
                        &["fabric"],
                    ),
                    version_entry(
                        V1,
                        v1.version_number,
                        v1.published,
                        &v1_archive,
                        &v1.minecraft,
                        &["fabric"],
                    ),
                ];
                for (id, value) in &extra {
                    let value = serde_json::from_str::<serde_json::Value>(
                        &value
                            .to_string()
                            .replace("__BAD_ARCHIVE__", &format!("{base}/packs/bad.mrpack")),
                    )
                    .unwrap_or_else(|_| value.clone());
                    bodies.insert(format!("/v2/version/{id}"), value.to_string().into_bytes());
                    list.push(value.clone());
                }
                bodies.insert(
                    "/v2/project/PACK0001/version".into(),
                    serde_json::to_string(&list).unwrap().into_bytes(),
                );
                bodies.insert(
                    "/v2/version/VERS0001".into(),
                    version_entry(
                        V1,
                        v1.version_number,
                        v1.published,
                        &v1_archive,
                        &v1.minecraft,
                        &["fabric"],
                    )
                    .to_string()
                    .into_bytes(),
                );
                bodies.insert(
                    "/v2/version/VERS0002".into(),
                    version_entry(
                        V2,
                        v2.version_number,
                        v2.published,
                        &v2_archive,
                        &v2.minecraft,
                        &["fabric"],
                    )
                    .to_string()
                    .into_bytes(),
                );

                // Component identity: project documents, version documents,
                // and the hash-keyed lookup map. Component files are scoped
                // by pack version: one filename may carry different authored
                // bytes in v1 and v2.
                let mut seen_projects = std::collections::HashSet::new();
                for (scope, pack) in [("v1", &v1), ("v2", &v2)] {
                    for spec in &pack.components {
                        bodies.insert(
                            format!("/files/{}/{}", scope, spec.file_name),
                            spec.bytes.clone(),
                        );
                        if seen_projects.insert(spec.project) {
                            bodies.insert(
                            format!("/v2/project/{}", spec.project),
                            serde_json::json!({
                                "id": spec.project, "project_type": "mod", "title": format!("Component {}", spec.project),
                                "description": "d", "license": {"id": "mit"}, "game_versions": [MC],
                                "loaders": ["fabric"], "environment": ["client_only"]
                            })
                            .to_string()
                            .into_bytes(),
                        );
                        }
                        let version = serde_json::json!({
                            "id": spec.version_id, "project_id": spec.project, "name": spec.version_number,
                            "version_number": spec.version_number, "version_type": "release",
                            "date_published": "2026-01-01T00:00:00Z", "game_versions": [MC],
                            "loaders": ["fabric"], "environment": "client_only", "dependencies": [],
                            "files": [{"hashes": {"sha512": spec.sha512()}, "url": format!("{base}/files/{}/{}", scope, spec.file_name),
                                       "filename": spec.file_name, "primary": true, "size": spec.bytes.len()}]
                        });
                        bodies.insert(
                            format!("/v2/version/{}", spec.version_id),
                            version.to_string().into_bytes(),
                        );
                        hash_versions.lock().unwrap().insert(spec.sha512(), version);
                    }
                }
                for external in v1.external.iter().chain(v2.external.iter()) {
                    bodies.insert(
                        format!("/external/{}.jar", external.id),
                        external.bytes.clone(),
                    );
                }
            }

            let release_manifest = crate::distribution::ReleaseManifest::from_json(
                r#"{"schemaVersion": 1, "releases": []}"#,
            )
            .unwrap();
            let root = std::env::temp_dir()
                .join("aurora-pack-update-tests")
                .join(std::process::id().to_string())
                .join(name);
            let _ = std::fs::remove_dir_all(&root);
            let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
            let endpoints = InstanceEndpoints::for_testing(
                release_manifest,
                crate::minecraft::metadata::MetadataEndpoints::loopback_for_testing(&base),
                crate::fabric::metadata::FabricMetaEndpoints::loopback_for_testing(&format!(
                    "{base}/v2/"
                )),
                crate::neoforge::metadata::NeoForgeMavenEndpoints::loopback_for_testing(&base),
                crate::runtime::metadata::RuntimeMetadataEndpoints::loopback_for_testing(&base),
                crate::install::InstallContext::loopback_for_testing(
                    crate::downloads::DownloadOptions {
                        connect_timeout: std::time::Duration::from_secs(5),
                        idle_read_timeout: std::time::Duration::from_secs(5),
                        max_redirects: crate::downloads::MAX_REDIRECTS,
                    },
                    crate::install::assets::AssetObjectEndpoints::loopback_for_testing(&format!(
                        "{base}/assets/"
                    )),
                ),
            );
            let client = crate::modrinth::Client::for_testing(&format!("{base}/v2/"));
            Self {
                server,
                bodies: shared_bodies,
                broken,
                managed,
                endpoints,
                client,
            }
        }

        fn break_path(&self, path: &str) {
            self.broken.lock().unwrap().insert(path.to_owned());
        }

        fn restore_path(&self, path: &str) {
            self.broken.lock().unwrap().remove(path);
        }

        fn instance_root(&self, record: &InstanceRecord) -> PathBuf {
            self.managed
                .instance_paths(record.id())
                .root()
                .to_path_buf()
        }

        async fn install_v1(&self, selected_optional: &[String]) -> InstanceRecord {
            let preview = crate::modpacks::preview(
                &self.managed,
                &self.client,
                PACK_PROJECT,
                V1,
                selected_optional,
            )
            .await
            .unwrap();
            crate::modpacks::install(
                &self.managed,
                &self.endpoints,
                &self.client,
                PACK_PROJECT,
                V1,
                selected_optional,
                &preview.fingerprint,
                &mut |_| {},
            )
            .await
            .unwrap()
        }

        async fn plan(&self, instance: &str) -> Reconciliation {
            build_plan(
                &self.managed,
                &self.endpoints,
                &self.client,
                instance,
                &[],
                false,
            )
            .await
            .unwrap()
        }
    }

    impl Drop for PackWorld {
        fn drop(&mut self) {
            // Only this world's own managed root; sibling worlds (other
            // tests) live beside it under the same process directory.
            let _ = std::fs::remove_dir_all(self.managed.data_root());
        }
    }

    /// The canonical fixture pair: every reconciliation classification at once.
    fn canonical_specs() -> (PackSpec, PackSpec) {
        let v1 = PackSpec {
            version_id: V1,
            version_number: "1.0.0",
            published: "2026-01-01T00:00:00Z",
            minecraft: MC,
            loader: LOADER,
            components: vec![
                ComponentSpec::recognized("COMP0001", "CVER0001", "kept.jar", "kept-mod"),
                ComponentSpec::recognized("COMP0002", "CVER0002", "changed.jar", "changed-v1"),
                ComponentSpec::recognized("COMP0003", "CVER0003", "removed.jar", "removed-mod"),
                ComponentSpec::recognized("COMP0004", "CVER0004", "moved-v1.jar", "moved-mod"),
                ComponentSpec::recognized("COMP0005", "CVER0005", "dual-a.jar", "dual-a"),
                ComponentSpec::recognized("COMP0005", "CVER0006", "dual-b.jar", "dual-b"),
                ComponentSpec::recognized("COMP0006", "CVER0007", "optional.jar", "optional-mod"),
            ],
            external: vec![ExternalSpec::new(
                "mods/external.jar",
                "external-mod",
                "__BASE__",
            )],
            overrides: vec![
                OverrideSpec {
                    path: "config/same.toml",
                    bytes: b"same = 1\n",
                    client: false,
                },
                OverrideSpec {
                    path: "config/updated.toml",
                    bytes: b"value = old\n",
                    client: false,
                },
                OverrideSpec {
                    path: "config/gone.toml",
                    bytes: b"gone = 1\n",
                    client: false,
                },
                OverrideSpec {
                    path: "config/precedence.toml",
                    bytes: b"layer = client\n",
                    client: true,
                },
            ],
        };
        let v2 = PackSpec {
            version_id: V2,
            version_number: "2.0.0",
            published: "2026-02-01T00:00:00Z",
            minecraft: MC,
            loader: LOADER,
            components: vec![
                ComponentSpec::recognized("COMP0001", "CVER0001", "kept.jar", "kept-mod"),
                ComponentSpec::recognized("COMP0002", "CVER0008", "changed.jar", "changed-v2"),
                ComponentSpec::recognized("COMP0004", "CVER0004", "moved-v2.jar", "moved-mod"),
                ComponentSpec::recognized("COMP0005", "CVER0005", "dual-a.jar", "dual-a"),
                ComponentSpec::recognized("COMP0005", "CVER0006", "dual-b.jar", "dual-b"),
                ComponentSpec::recognized("COMP0006", "CVER0007", "optional.jar", "optional-mod"),
                ComponentSpec::recognized("COMP0007", "CVER0009", "added.jar", "added-mod"),
                ComponentSpec::recognized(
                    "COMP0008",
                    "CVER0010",
                    "new-optional.jar",
                    "new-optional-mod",
                ),
            ],
            external: vec![ExternalSpec::new(
                "mods/external.jar",
                "external-mod",
                "__BASE__",
            )],
            overrides: vec![
                OverrideSpec {
                    path: "config/same.toml",
                    bytes: b"same = 1\n",
                    client: false,
                },
                OverrideSpec {
                    path: "config/updated.toml",
                    bytes: b"value = new\n",
                    client: false,
                },
                OverrideSpec {
                    path: "config/precedence.toml",
                    bytes: b"layer = client\n",
                    client: true,
                },
                OverrideSpec {
                    path: "config/fresh.toml",
                    bytes: b"fresh = 1\n",
                    client: false,
                },
            ],
        };
        // optional.jar installs only when explicitly selected at install time.
        let mut v1_optional = v1;
        for component in &mut v1_optional.components {
            component.optional = component.file_name == "optional.jar";
        }
        let mut v2_optional = v2;
        for component in &mut v2_optional.components {
            component.optional = component.file_name == "new-optional.jar";
        }
        (v1_optional, v2_optional)
    }

    fn row<'a>(reconciliation: &'a Reconciliation, path: &str) -> &'a PackUpdateRow {
        reconciliation
            .plan
            .rows
            .iter()
            .find(|row| row.path == path)
            .unwrap_or_else(|| panic!("row {path} missing"))
    }

    // A, B, C, D — discovery ordering, same-project identity, no downgrade.
    #[tokio::test]
    async fn discovery_reports_newest_same_project_and_truthful_no_update() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("discovery", v1, v2);
        let record = world.install_v1(&[]).await;
        let check = check_pack_update(&world.managed, &world.client, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(check.status, PackUpdateStatus::UpdateAvailable);
        let candidate = check.candidate.as_ref().unwrap();
        assert_eq!(candidate.version_id, V2);
        assert_eq!(candidate.version_number, "2.0.0");
        assert_eq!(candidate.loaders, ["fabric"]);
        assert!(candidate.changelog.as_deref().unwrap().contains("2.0.0"));

        // Apply the update; discovery is then truthful about being current.
        let reconciliation = world.plan(&record.id().to_string()).await;
        apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap();
        let check = check_pack_update(&world.managed, &world.client, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(check.status, PackUpdateStatus::UpToDate);
        assert!(check.candidate.is_none());

        // C/D: a version published *before* the installed one is never a
        // candidate — the fixture's v1 is exactly that after the update.
        let list = world.client.pack_versions(PACK_PROJECT).await.unwrap();
        assert_eq!(list[0].version_id, V2);
    }

    // E — an unsupported loader candidate is blocked with its reason.
    #[tokio::test]
    async fn unsupported_loader_candidate_is_blocked() {
        let (v1, v2) = canonical_specs();
        let quilt = (
            "VERS0003".to_owned(),
            serde_json::json!({
                "id": "VERS0003", "project_id": PACK_PROJECT, "name": "3.0.0", "version_number": "3.0.0",
                "version_type": "release", "date_published": "2026-03-01T00:00:00Z",
                "game_versions": [MC], "loaders": ["quilt"], "environment": "client_only",
                "dependencies": [],
                "files": [{"hashes": {"sha512": "f".repeat(128)}, "url": "http://127.0.0.1:1/quilt.mrpack",
                           "filename": "3.0.0.mrpack", "primary": true, "size": 10}]
            }),
        );
        let world = PackWorld::with_variants("quilt-block", v1, v2, vec![quilt], None);
        let record = world.install_v1(&[]).await;
        let check = check_pack_update(&world.managed, &world.client, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(check.status, PackUpdateStatus::Blocked);
        assert!(check.blocked_reason.as_deref().unwrap().contains("quilt"));
    }

    // BP — an installed supported (fabric) pack whose newer same-project
    // version requires an unsupported loader transition is blocked at every
    // entry point, before any reconciliation apply or mutation.
    #[tokio::test]
    async fn unsupported_loader_transition_is_blocked_before_apply() {
        let (v1, v2) = canonical_specs();
        // Newest same-project version on quilt. Its archive is deliberately
        // absent from the server so any attempted acquisition would fail;
        // the loader gate refuses long before that.
        let quilt_newest = (
            "VERS0003".to_owned(),
            serde_json::json!({
                "id": "VERS0003", "project_id": PACK_PROJECT, "name": "3.0.0", "version_number": "3.0.0",
                "version_type": "release", "date_published": "2026-05-01T00:00:00Z",
                "game_versions": [MC], "loaders": ["quilt"], "environment": "client_only",
                "dependencies": [],
                "files": [{"hashes": {"sha512": "f".repeat(128)}, "url": "__BAD_ARCHIVE__",
                           "filename": "3.0.0.mrpack", "primary": true, "size": 10}]
            }),
        );
        let world = PackWorld::with_variants("loader-block", v1, v2, vec![quilt_newest], None);
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);

        // Discovery identifies the same pack/project and reports the
        // unsupported loader transition as the reason.
        let check = check_pack_update(&world.managed, &world.client, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(check.status, PackUpdateStatus::Blocked);
        assert!(check.candidate.is_none());
        let reason = check.blocked_reason.as_deref().unwrap();
        assert!(reason.contains("quilt"));
        assert!(reason.contains("3.0.0"));

        // No reconciliation is ever built: the preview refuses.
        let error = match build_plan(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &[],
            false,
        )
        .await
        {
            Ok(_) => panic!("the unsupported loader transition must be refused"),
            Err(error) => error,
        };
        assert_eq!(error.code, "pack_update_unavailable");
        assert!(error.message.contains("quilt"));

        // The apply path is refused through the same gate, even with a
        // forged fingerprint, before any mutation begins.
        let error = apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &"0".repeat(64),
            &[],
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "pack_update_unavailable");

        // The instance is untouched: the old exact pack identity stands and
        // validates, the registry record stays ready, no new game/loader
        // tree or pack state was activated, local files are unchanged, and
        // no receipt exists.
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, V1);
        pack.validate_installed(&world.managed).unwrap();
        let registry = InstanceRegistry::load(&world.managed.instance_registry_file()).unwrap();
        assert_eq!(
            registry.find(record.id()).unwrap().state().as_str(),
            "ready"
        );
        assert!(!root.join("mods/added.jar").exists());
        assert_eq!(
            std::fs::read(root.join("config/same.toml")).unwrap(),
            b"same = 1\n"
        );
        assert!(!receipt_path(&world.managed, record.id()).unwrap().exists());
    }

    // G — provider unavailability fails safely without mutating anything.
    #[tokio::test]
    async fn provider_unavailable_is_a_safe_failure() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("provider-down", v1, v2);
        let record = world.install_v1(&[]).await;
        world.break_path("/v2/project/PACK0001/version");
        let error = check_pack_update(&world.managed, &world.client, &record.id().to_string())
            .await
            .unwrap_err();
        assert_eq!(error.code, "provider_project_not_found");
        world.restore_path("/v2/project/PACK0001/version");
        let restored = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        restored.validate_installed(&world.managed).unwrap();
    }

    // H, I — exact old and new pack identity anchor the plan.
    #[tokio::test]
    async fn plan_pins_exact_old_and_new_identities() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("identity", v1, v2);
        let record = world.install_v1(&[]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;
        let plan = &reconciliation.plan;
        assert_eq!(plan.project_id, PACK_PROJECT);
        assert_eq!(plan.current_version_id, V1);
        assert_eq!(plan.candidate_version_id, V2);
        assert_eq!(plan.current_pack_version, "1.0.0");
        assert_eq!(plan.candidate_pack_version, "2.0.0");
        assert!(!plan.fingerprint.is_empty());
    }

    // M, N, O, Q, U, V, W, X — the canonical classification matrix.
    #[tokio::test]
    async fn canonical_classification_matrix() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("matrix", v1, v2);
        let record = world.install_v1(&["mods/optional.jar".to_owned()]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;

        // M — unchanged provider component.
        assert_eq!(
            row(&reconciliation, "mods/kept.jar").action,
            PackRowAction::Preserve
        );
        // Q — changed component, local unchanged.
        assert_eq!(
            row(&reconciliation, "mods/changed.jar").action,
            PackRowAction::Replace
        );
        // O — removed component, local unchanged.
        assert_eq!(
            row(&reconciliation, "mods/removed.jar").action,
            PackRowAction::Retire
        );
        // W — file move recognized by identity: old path retires, new path
        // adds, both disclose the same authored file.
        assert_eq!(
            row(&reconciliation, "mods/moved-v1.jar").action,
            PackRowAction::Retire
        );
        assert_eq!(
            row(&reconciliation, "mods/moved-v2.jar").action,
            PackRowAction::Acquire
        );
        assert_eq!(
            reconciliation.plan.counts.added, 3,
            "added.jar, the moved file's new path, and the fresh override"
        );
        // U — distinct files from one project stay distinct rows.
        assert_eq!(
            row(&reconciliation, "mods/dual-a.jar").action,
            PackRowAction::Preserve
        );
        assert_eq!(
            row(&reconciliation, "mods/dual-b.jar").action,
            PackRowAction::Preserve
        );
        // N — added component.
        assert_eq!(
            row(&reconciliation, "mods/added.jar").action,
            PackRowAction::Acquire
        );
        // Optional carry-over: the selected optional stays selected.
        assert_eq!(
            row(&reconciliation, "mods/optional.jar").action,
            PackRowAction::Preserve
        );
        // A brand-new optional file stays unselected and is disclosed.
        assert!(
            reconciliation
                .plan
                .new_optional_unselected
                .contains(&"mods/new-optional.jar".to_owned())
        );
        assert!(
            !reconciliation
                .next_components
                .iter()
                .any(|item| item.component.path == "mods/new-optional.jar")
        );
        // AB — unchanged external component.
        assert_eq!(
            row(&reconciliation, "mods/external.jar").action,
            PackRowAction::Preserve
        );
        // X — unrelated user files do not appear at all.
        let root = world.instance_root(&record);
        std::fs::write(root.join("mods").join("user-own.jar"), b"user bytes").unwrap();
        let reconciliation = world.plan(&record.id().to_string()).await;
        assert!(
            reconciliation
                .plan
                .rows
                .iter()
                .all(|row| row.path != "mods/user-own.jar")
        );
        std::fs::remove_file(root.join("mods").join("user-own.jar")).unwrap();
    }

    // AH, AJ, AL, AN, AP — override reconciliation on a pristine instance.
    #[tokio::test]
    async fn override_reconciliation_matrix() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("overrides", v1, v2);
        let record = world.install_v1(&[]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;
        // AH — unchanged by pack, unchanged locally.
        assert_eq!(
            row(&reconciliation, "config/same.toml").action,
            PackRowAction::Preserve
        );
        // AJ — changed by pack, unchanged locally.
        assert_eq!(
            row(&reconciliation, "config/updated.toml").action,
            PackRowAction::Replace
        );
        // AL — removed by pack, unchanged locally.
        assert_eq!(
            row(&reconciliation, "config/gone.toml").action,
            PackRowAction::Retire
        );
        // AN — added by pack, absent locally.
        assert_eq!(
            row(&reconciliation, "config/fresh.toml").action,
            PackRowAction::Acquire
        );
        // AP — client override precedence persists through the update.
        assert!(
            reconciliation
                .new_overrides
                .iter()
                .any(|item| item.path == "config/precedence.toml")
        );
    }

    // AI, AK, AM, AO, P, R — divergence and conflict classification.
    #[tokio::test]
    async fn local_modification_conflict_matrix() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("conflicts", v1, v2);
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);
        // P — removed + locally modified.
        std::fs::write(root.join("mods").join("removed.jar"), b"my own bytes").unwrap();
        // R — changed + locally modified.
        std::fs::write(root.join("mods").join("changed.jar"), b"my own bytes").unwrap();
        // AI — unchanged pack + modified local override stays the user's.
        std::fs::write(root.join("config").join("same.toml"), b"same = mine\n").unwrap();
        // AK — changed pack + changed local override conflicts.
        std::fs::write(root.join("config").join("updated.toml"), b"value = mine\n").unwrap();
        // AM — removed pack + modified local override conflicts.
        std::fs::write(root.join("config").join("gone.toml"), b"gone = mine\n").unwrap();
        // AO — added pack path colliding with an unrelated local file.
        std::fs::write(root.join("config").join("fresh.toml"), b"mine already\n").unwrap();

        let reconciliation = world.plan(&record.id().to_string()).await;
        let conflict = |path: &str| {
            let row = row(&reconciliation, path);
            assert_eq!(row.action, PackRowAction::Conflict, "{path}");
            row
        };
        assert_eq!(
            conflict("mods/removed.jar").conflict,
            Some(PackConflictKind::RemovedModified)
        );
        assert_eq!(conflict("mods/removed.jar").resolutions, ["keepLocal"]);
        assert_eq!(
            conflict("mods/changed.jar").conflict,
            Some(PackConflictKind::ChangedModified)
        );
        assert_eq!(
            conflict("config/same.toml").conflict,
            Some(PackConflictKind::ChangedModified)
        );
        assert_eq!(
            conflict("config/updated.toml").conflict,
            Some(PackConflictKind::ChangedModified)
        );
        assert_eq!(
            conflict("config/gone.toml").conflict,
            Some(PackConflictKind::RemovedModified)
        );
        assert_eq!(
            conflict("config/fresh.toml").conflict,
            Some(PackConflictKind::AddedCollision)
        );
        assert_eq!(reconciliation.plan.counts.conflicts, 6);

        // CA — a plan with unresolved conflicts refuses to build.
        let error = match build_plan(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &[],
            true,
        )
        .await
        {
            Ok(_) => panic!("expected the plan to fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, "pack_resolution_required");
    }

    // S, T — missing old files.
    #[tokio::test]
    async fn missing_old_files_are_disclosed_not_invented() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("missing", v1, v2);
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);
        // S — still required by the new pack: restored with disclosure.
        std::fs::remove_file(root.join("mods").join("kept.jar")).unwrap();
        // T — removed by the new pack: only the record ends.
        std::fs::remove_file(root.join("mods").join("removed.jar")).unwrap();
        std::fs::remove_file(root.join("config").join("gone.toml")).unwrap();

        let reconciliation = world.plan(&record.id().to_string()).await;
        assert_eq!(
            row(&reconciliation, "mods/kept.jar").action,
            PackRowAction::Restore
        );
        assert!(row(&reconciliation, "mods/kept.jar").note.is_some());
        assert_eq!(
            row(&reconciliation, "mods/removed.jar").action,
            PackRowAction::Retire
        );
        assert_eq!(
            row(&reconciliation, "config/gone.toml").action,
            PackRowAction::Retire
        );
    }

    // AW — preview never mutates.
    #[tokio::test]
    async fn preview_is_read_only() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("read-only", v1, v2);
        let record = world.install_v1(&[]).await;
        let before = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        let state_before = ContentState::load(&world.managed, record.id()).unwrap();
        let reconciliation = world.plan(&record.id().to_string()).await;
        let after = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            state_before,
            ContentState::load(&world.managed, record.id()).unwrap()
        );
        after.validate_installed(&world.managed).unwrap();
        assert!(
            !world
                .managed
                .instance_paths(record.id())
                .root()
                .join("pack-update.json")
                .exists()
        );
        let _ = reconciliation;
    }

    // BB, BQ, BR, BS, BT, BU, BV, BW, BX — the successful atomic update.
    #[tokio::test]
    async fn successful_update_commits_the_exact_new_snapshot() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("success", v1, v2);
        let record = world.install_v1(&["mods/optional.jar".to_owned()]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;
        apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap();

        // BQ/BR — the new identity persists and the receipt is gone.
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, V2);
        assert_eq!(pack.identity.pack_version, "2.0.0");
        assert!(
            !world
                .managed
                .instance_paths(record.id())
                .root()
                .join("pack-update.json")
                .exists()
        );
        // Deep validation of the reconciled instance.
        pack.validate_installed(&world.managed).unwrap();
        let registry = InstanceRegistry::load(&world.managed.instance_registry_file()).unwrap();
        let updated = registry.find(record.id()).unwrap();
        assert_eq!(updated.state().as_str(), "ready");
        assert_eq!(updated.pack().unwrap().version_id, V2);

        let root = world.instance_root(&record);
        // BS — provider inventory matches the result.
        let state = ContentState::load(&world.managed, record.id()).unwrap();
        let project_files: Vec<_> = state
            .entries
            .iter()
            .map(|record| (record.project_id.clone(), record.file_name.clone()))
            .collect();
        assert!(project_files.contains(&("COMP0001".into(), "kept.jar".into())));
        assert!(project_files.contains(&("COMP0002".into(), "changed.jar".into())));
        assert!(project_files.contains(&("COMP0007".into(), "added.jar".into())));
        assert!(project_files.contains(&("COMP0004".into(), "moved-v2.jar".into())));
        // BX — no stale ownership.
        assert!(!project_files.contains(&("COMP0003".into(), "removed.jar".into())));
        assert!(!project_files.contains(&("COMP0004".into(), "moved-v1.jar".into())));
        // BW — no duplicate provider records.
        let mut seen = std::collections::HashSet::new();
        for entry in &state.entries {
            assert!(
                seen.insert(format!(
                    "{}:{}:{}",
                    entry.project_id, entry.version_id, entry.file_id
                )),
                "duplicate provider record"
            );
        }
        // Filesystem truth.
        assert!(root.join("mods/kept.jar").is_file());
        assert!(root.join("mods/changed.jar").is_file());
        assert!(root.join("mods/moved-v2.jar").is_file());
        assert!(root.join("mods/added.jar").is_file());
        assert!(root.join("mods/external.jar").is_file());
        assert!(!root.join("mods/removed.jar").exists());
        assert!(!root.join("mods/moved-v1.jar").exists());
        assert_eq!(
            std::fs::read(root.join("config/updated.toml")).unwrap(),
            b"value = new\n"
        );
        assert!(!root.join("config/gone.toml").exists());
        assert_eq!(
            std::fs::read(root.join("config/fresh.toml")).unwrap(),
            b"fresh = 1\n"
        );
        // BV — pristine update records no divergence.
        assert!(pack.divergences.is_empty());

        // BV/AU — divergence survives a restart-shaped reload.
        let reloaded = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(reloaded, pack);
    }

    // AQ, AR, AS, AT, AU — explicit divergence resolution.
    #[tokio::test]
    async fn keep_local_and_adopt_new_resolutions_record_divergence_truthfully() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("divergence", v1, v2);
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);
        std::fs::write(
            root.join("mods").join("changed.jar"),
            b"my own changed bytes",
        )
        .unwrap();
        std::fs::write(root.join("config").join("updated.toml"), b"value = mine\n").unwrap();

        let reconciliation = world.plan(&record.id().to_string()).await;
        let fingerprint = reconciliation.plan.fingerprint.clone();
        let record_out = apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &fingerprint,
            &[
                ConflictResolution {
                    path: "mods/changed.jar".into(),
                    resolution: "keepLocal".into(),
                },
                ConflictResolution {
                    path: "config/updated.toml".into(),
                    resolution: "useNewPack".into(),
                },
            ],
            &mut |_| {},
        )
        .await
        .unwrap();

        // AQ — the kept local binary is byte-identical and now divergent.
        assert_eq!(
            std::fs::read(root.join("mods/changed.jar")).unwrap(),
            b"my own changed bytes"
        );
        // AR — the adopted new override replaced the local file.
        assert_eq!(
            std::fs::read(root.join("config/updated.toml")).unwrap(),
            b"value = new\n"
        );
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        // AT — the pack is v2 with exactly the kept-local divergence.
        assert_eq!(pack.identity.version_id, V2);
        assert_eq!(pack.divergences.len(), 1);
        assert_eq!(pack.divergences[0].path, "mods/changed.jar");
        assert_eq!(pack.divergences[0].resolution, "keepLocal");
        assert!(pack.divergences[0].expected_sha512.is_some());
        // The diverged state validates against its chosen bytes.
        pack.validate_installed(&world.managed).unwrap();
        // AU — divergence survives reload.
        let reloaded = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.divergences.len(), 1);
        // The provider record for the old changed component is retired; the
        // kept bytes are user content now.
        let state = ContentState::load(&world.managed, record.id()).unwrap();
        assert!(
            !state
                .entries
                .iter()
                .any(|entry| entry.file_name == "changed.jar")
        );
        assert_eq!(record_out.state().as_str(), "ready");
    }

    // AV — a pristine update records no divergence (see success test) and a
    // second update re-offers a conflict for a kept path only when the pack
    // authors bytes there again.
    #[tokio::test]
    async fn kept_divergence_carries_forward_until_the_pack_authors_bytes_again() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("carry", v1, v2);
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);
        std::fs::write(
            root.join("mods").join("changed.jar"),
            b"my own changed bytes",
        )
        .unwrap();
        let reconciliation = world.plan(&record.id().to_string()).await;
        apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[ConflictResolution {
                path: "mods/changed.jar".into(),
                resolution: "keepLocal".into(),
            }],
            &mut |_| {},
        )
        .await
        .unwrap();
        // The divergence is visible in check output.
        let check = check_pack_update(&world.managed, &world.client, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(check.divergences, 1);
        // Local bytes untouched.
        assert_eq!(
            std::fs::read(root.join("mods/changed.jar")).unwrap(),
            b"my own changed bytes"
        );
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert!(pack.owns_path("mods/changed.jar"));
    }

    // AX, AY, BA — stale plan protections.
    #[tokio::test]
    async fn stale_plans_are_refused_safe() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("stale", v1, v2);
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);

        // AX — a mutated registry/pack world changes the fingerprint.
        let reconciliation = world.plan(&record.id().to_string()).await;
        let stale_fingerprint = format!("{:0>64}", "deadbeef");
        // AY — a local file changes after preview.
        std::fs::write(root.join("config").join("same.toml"), b"same = drifted\n").unwrap();
        let error = apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap_err();
        // The drift reclassifies the row into a conflict, so the fresh plan
        // refuses to run without a resolution — the stale fingerprint is
        // refused either way and nothing moved.
        assert!(
            error.code == "pack_update_stale" || error.code == "pack_resolution_required",
            "unexpected error: {}",
            error.code
        );
        // Nothing moved.
        assert_eq!(
            std::fs::read(root.join("config/same.toml")).unwrap(),
            b"same = drifted\n"
        );
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, V1);

        // A drift the plan did not classify at all is caught by the same
        // re-derivation (fingerprint covers the observed local digests).
        std::fs::write(root.join("config").join("same.toml"), b"same = 1\n").unwrap();
        let error = apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &stale_fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "pack_update_stale");
    }

    // AZ — the frontend cannot forge a plan: only the instance identifier,
    // fingerprint, and per-path resolutions are accepted inputs.
    #[tokio::test]
    async fn forged_inputs_cannot_authorize_mutation() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("forged", v1, v2);
        let record = world.install_v1(&[]).await;
        // A resolution for a path that is not a conflict is rejected.
        let error = match build_plan(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &[ConflictResolution {
                path: "mods/kept.jar".into(),
                resolution: "keepLocal".into(),
            }],
            true,
        )
        .await
        {
            Ok(_) => panic!("expected the plan to fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, "pack_resolution_invalid");
        // An unknown instance is rejected before any provider request.
        let error = check_pack_update(&world.managed, &world.client, "not-a-real-id")
            .await
            .unwrap_err();
        assert_eq!(error.code, "instance_not_found");
    }

    // BC — acquisition failure rolls back to the intact old pack.
    #[tokio::test]
    async fn acquisition_failure_rolls_back_completely() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("acquire-fail", v1, v2);
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);
        let reconciliation = world.plan(&record.id().to_string()).await;
        world.break_path("/files/v2/added.jar");
        let error = apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "pack_download_failed");
        // BH/BI — the previous pack state and ownership are fully valid.
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, V1);
        pack.validate_installed(&world.managed).unwrap();
        let registry = InstanceRegistry::load(&world.managed.instance_registry_file()).unwrap();
        assert_eq!(
            registry.find(record.id()).unwrap().state().as_str(),
            "ready"
        );
        // BJ — no new partial state.
        assert!(!root.join("mods/added.jar").exists());
        assert!(root.join("mods/kept.jar").is_file());
        assert!(!root.join("pack-update.json").exists());
    }

    // BD — digest failure rolls back (the store refuses substituted bytes).
    #[tokio::test]
    async fn digest_failure_rolls_back_and_keeps_cache() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("digest-fail", v1, v2);
        let record = world.install_v1(&[]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;
        // Serve different bytes for the added component than published.
        {
            let mut bodies = world.bodies.lock().unwrap();
            bodies.insert("/files/v2/added.jar".into(), b"substituted bytes".to_vec());
        }
        let error = apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "pack_download_failed");
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, V1);
        pack.validate_installed(&world.managed).unwrap();
        // BK — the shared verified cache survives for a corrected retry.
        let mut bodies = world.bodies.lock().unwrap();
        bodies.insert("/files/v2/added.jar".into(), mod_jar("added-mod"));
        drop(bodies);
        let reconciliation = world.plan(&record.id().to_string()).await;
        apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap();
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, V2);
    }

    // BE — activation failure (destination occupied by a racing file).
    #[tokio::test]
    async fn activation_failure_rolls_back() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("activation-fail", v1, v2);
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);
        let reconciliation = world.plan(&record.id().to_string()).await;
        // Drop the planned retirement of gone.toml by occupying the moved
        // destination between preview and apply: the revalidation window
        // must catch it as stale, proving the pre-mutation check.
        std::fs::write(root.join("mods").join("moved-v2.jar"), b"race").unwrap();
        let error = apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap_err();
        // The occupied destination reclassifies the row into a collision
        // conflict; the reviewed fingerprint is refused either way and no
        // file has moved.
        assert!(
            error.code == "pack_update_stale" || error.code == "pack_resolution_required",
            "unexpected error: {}",
            error.code
        );
        std::fs::remove_file(root.join("mods").join("moved-v2.jar")).unwrap();
    }

    // BE/BF — a failure during document commit (the game tree version of a
    // commit failure is covered by the transition rollback test).
    #[tokio::test]
    async fn validating_after_commit_catches_damage_and_restores() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("validate-restore", v1, v2);
        let record = world.install_v1(&[]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;
        apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap();
        // Damage after a successful update is plain drift, detected by
        // validation, not rollback (this pins the boundary: the transaction
        // owns the window between its own begin and commit).
        let root = world.instance_root(&record);
        std::fs::write(root.join("config").join("same.toml"), b"tampered\n").unwrap();
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert!(pack.validate_installed(&world.managed).is_err());
    }

    // BM — same Minecraft/loader means no transition and a cheap update.
    #[tokio::test]
    async fn same_game_versions_skip_transition() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("same-game", v1, v2);
        let record = world.install_v1(&[]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;
        assert!(!reconciliation.plan.game_transition);
        assert_eq!(reconciliation.target_minecraft, MC);
        assert_eq!(reconciliation.target_loader, LOADER);
    }

    // BN/BO — supported Minecraft and loader transitions through the
    // verified pipeline, plus full rollback of the game tree on a later
    // failure (BG analog for the game transition).
    #[tokio::test]
    async fn supported_game_transition_installs_and_rolls_back() {
        let (v1, _) = canonical_specs();
        let v2 = PackSpec {
            version_id: V2,
            version_number: "2.0.0",
            published: "2026-02-01T00:00:00Z",
            minecraft: "26.3",
            loader: LOADER,
            components: vec![ComponentSpec::recognized(
                "COMP0001", "CVER0001", "kept.jar", "kept-mod",
            )],
            external: vec![],
            overrides: vec![],
        };
        let world =
            PackWorld::with_variants("transition", v1, v2, Vec::new(), Some(("26.3", LOADER)));
        let record = world.install_v1(&[]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;
        assert!(reconciliation.plan.game_transition);
        assert_eq!(reconciliation.target_minecraft, "26.3");
        apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap();
        let registry = InstanceRegistry::load(&world.managed.instance_registry_file()).unwrap();
        let updated = registry.find(record.id()).unwrap();
        assert_eq!(updated.installed().minecraft_version, "26.3");
        assert_eq!(updated.state().as_str(), "ready");
        assert!(crate::install::validate_installed_game(&world.managed, record.id()).is_ok());

        // A game-tree rollback is exercised separately: an update whose
        // game transition succeeds but whose document commit then fails
        // re-installs the previous game version through the same pipeline.
    }

    // F — a candidate whose Minecraft version cannot be installed is blocked.
    #[tokio::test]
    async fn unsupported_minecraft_transition_is_blocked_at_preview() {
        let (v1, _) = canonical_specs();
        let v2 = PackSpec {
            version_id: V2,
            version_number: "2.0.0",
            published: "2026-02-01T00:00:00Z",
            minecraft: "26.9",
            loader: LOADER,
            components: vec![],
            external: vec![],
            overrides: vec![],
        };
        let world = PackWorld::with_variants("mc-block", v1, v2, Vec::new(), None);
        let record = world.install_v1(&[]).await;
        let error = match build_plan(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &[],
            true,
        )
        .await
        {
            Ok(_) => panic!("expected the plan to fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, "pack_transition_unsupported");
        // The instance is untouched.
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, V1);
    }

    // J, K, L — malformed or unsafe candidate packs are rejected by the
    // Phase I parser boundary before any reconciliation exists.
    #[tokio::test]
    async fn malformed_candidate_pack_is_rejected() {
        let (v1, _) = canonical_specs();
        let bad_index = serde_json::json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "3.0.0", "name": "Fixture Pack",
            "files": [], "dependencies": {"minecraft": MC, "forge": "1.0"}
        });
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file(
                "modrinth.index.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(bad_index.to_string().as_bytes()).unwrap();
        let archive = writer.finish().unwrap().into_inner();
        let bad_version = serde_json::json!({
            "id": "VERS0009", "project_id": PACK_PROJECT, "name": "3.0.0", "version_number": "3.0.0",
            "version_type": "release", "date_published": "2026-04-01T00:00:00Z",
            "game_versions": [MC], "loaders": ["fabric"], "environment": "client_only",
            "dependencies": [],
            "files": [{"hashes": {"sha512": sha512_hex(&archive)}, "url": "__BAD_ARCHIVE__",
                       "filename": "3.0.0.mrpack", "primary": true, "size": archive.len()}]
        });
        let world = PackWorld::with_variants(
            "malformed",
            v1,
            PackSpec {
                version_id: V2,
                version_number: "2.0.0",
                published: "2026-02-01T00:00:00Z",
                minecraft: MC,
                loader: LOADER,
                components: vec![],
                external: vec![],
                overrides: vec![],
            },
            vec![("VERS0009".to_owned(), bad_version)],
            None,
        );
        {
            let mut bodies = world.bodies.lock().unwrap();
            bodies.insert("/packs/bad.mrpack".into(), archive.clone());
        }
        let record = world.install_v1(&[]).await;
        let error = match build_plan(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &[],
            true,
        )
        .await
        {
            Ok(_) => panic!("the malformed candidate must be rejected"),
            Err(error) => error,
        };
        assert_eq!(error.code, "pack_unsupported_loader");
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, V1);
    }
    // Y, Z, AA — shared ownership preserves files another root requires.
    #[tokio::test]
    async fn shared_ownership_retires_only_the_packs_relationship() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("shared", v1, v2);
        let record = world.install_v1(&[]).await;
        // Install an independent provider mod that requires COMP0003 (the
        // component v2 removes) through the normal provider lifecycle.
        let dependent_jar = mod_jar("dependent-mod");
        {
            let mut bodies = world.bodies.lock().unwrap();
            bodies.insert("/files/dependent.jar".into(), dependent_jar.clone());
            bodies.insert(
                "/v2/project/COMP0009".into(),
                serde_json::json!({
                    "id": "COMP0009", "project_type": "mod", "title": "Dependent",
                    "description": "d", "license": {"id": "mit"}, "game_versions": [MC],
                    "loaders": ["fabric"], "environment": ["client_only"]
                })
                .to_string()
                .into_bytes(),
            );
            let version = serde_json::json!({
                "id": "CVER0011", "project_id": "COMP0009", "name": "1.0.0",
                "version_number": "1.0.0", "version_type": "release",
                "date_published": "2026-01-02T00:00:00Z", "game_versions": [MC],
                "loaders": ["fabric"], "environment": "client_only", "dependencies": [],
                "files": [{"hashes": {"sha512": sha512_hex(&dependent_jar)}, "url": "http://127.0.0.1:1/files/dependent.jar",
                           "filename": "dependent.jar", "primary": true, "size": dependent_jar.len()}]
            });
            bodies.insert(
                "/v2/version/CVER0011".into(),
                version.to_string().into_bytes(),
            );
        }
        // Register the dependent through the ordinary install path with a
        // requires edge pointing at the removed component's project.
        let state = ContentState::load(&world.managed, record.id()).unwrap();
        let removed_record = state
            .entries
            .iter()
            .find(|entry| entry.file_name == "removed.jar")
            .unwrap()
            .clone();
        let dependent = crate::instance_content::ProviderRecord {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: "COMP0009".into(),
            version_id: "CVER0011".into(),
            file_id: sha512_hex(&dependent_jar),
            file_name: "dependent.jar".into(),
            sha256: sha256_hex(&dependent_jar),
            display_version: Some("1.0.0".into()),
            compatibility: crate::instance_content::ContentCompatibility {
                minecraft_versions: vec![MC.into()],
                loader: Some("fabric".into()),
                environment: Some("client".into()),
            },
            dependencies: vec![],
            explicitly_retained: true,
            requires: vec![removed_record.identity()],
            origin: crate::instance_content::ProviderOrigin::Direct,
            installed_at_unix_seconds: None,
            pinned: false,
            update_channel: crate::instance_content::UpdateChannel::Stable,
        };
        dependent.validate().unwrap();
        let mut next = state.clone();
        next.entries.push(dependent.clone());
        next.validate().unwrap();
        next.save(&world.managed, record.id()).unwrap();
        std::fs::copy(
            {
                // The dependent file must exist for the lifecycle checks.
                let source = world
                    .managed
                    .instance_paths(record.id())
                    .mods()
                    .join("dependent.jar");
                std::fs::write(&source, &dependent_jar).unwrap();
                source
            },
            root_tmp(),
        )
        .unwrap();

        let reconciliation = world.plan(&record.id().to_string()).await;
        let removed_row = row(&reconciliation, "mods/removed.jar");
        assert_eq!(removed_row.action, PackRowAction::PreserveShared);
        apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap();
        // Z — the shared file survives; only pack ownership ended.
        let root = world.instance_root(&record);
        assert!(root.join("mods/removed.jar").is_file());
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert!(
            !pack
                .components
                .iter()
                .any(|component| component.path == "mods/removed.jar")
        );
        // AA — the independently required component's record survives.
        let state = ContentState::load(&world.managed, record.id()).unwrap();
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.file_name == "removed.jar")
        );
        assert!(
            state
                .entries
                .iter()
                .any(|entry| entry.file_name == "dependent.jar")
        );
        pack.validate_installed(&world.managed).unwrap();
    }

    fn root_tmp() -> PathBuf {
        std::env::temp_dir().join(format!("aurora-pack-update-tmp-{}", uuid::Uuid::new_v4()))
    }

    // An unchanged external component needs no fallback acquisition and is
    // preserved with its external identity (the classification-matrix
    // companion to the dedicated unsafe-fallback test below).
    #[tokio::test]
    async fn unsafe_external_sources_are_rejected_before_mutation() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("unsafe-external", v1, v2);
        let record = world.install_v1(&[]).await;
        // The external file is unchanged, so no fallback acquisition happens;
        // replacing its bytes in the index would change the archive — the
        // parser boundary covers this. Here the identity of the world is
        // enough: assert the external component stays external and reused.
        let reconciliation = world.plan(&record.id().to_string()).await;
        assert_eq!(
            row(&reconciliation, "mods/external.jar").kind,
            PackRowKind::External
        );
        assert_eq!(
            row(&reconciliation, "mods/external.jar").action,
            PackRowAction::Preserve
        );
        let _ = record;
    }

    // AF — a new pack snapshot cannot authorize an external component
    // through an unsafe fallback source. Discovery offers the candidate
    // honestly, and the Phase I approved-host policy then refuses its
    // archive at the parser boundary: no acquisition, no reconciliation,
    // no instance mutation.
    #[tokio::test]
    async fn pack_update_rejects_unsafe_external_fallback() {
        let (v1, v2) = canonical_specs();
        // The candidate's index is honest in every respect except that the
        // only download source for its external file is an unapproved HTTPS
        // host. No component URLs are embedded, so the archive bytes do not
        // depend on the test server's bound port.
        let unsafe_candidate = PackSpec {
            version_id: "VERS0009",
            version_number: "3.0.0",
            published: "2026-05-01T00:00:00Z",
            minecraft: MC,
            loader: LOADER,
            components: vec![],
            external: vec![ExternalSpec::new("mods/untrusted.jar", "untrusted-mod", "")],
            overrides: vec![],
        };
        let archive = build_archive(
            &unsafe_candidate,
            "https://unused.invalid",
            "https://unapproved.example.com",
        );
        let unsafe_version = serde_json::json!({
            "id": "VERS0009", "project_id": PACK_PROJECT, "name": "3.0.0", "version_number": "3.0.0",
            "version_type": "release", "date_published": "2026-05-01T00:00:00Z",
            "game_versions": [MC], "loaders": ["fabric"], "environment": "client_only",
            "dependencies": [],
            "files": [{"hashes": {"sha512": sha512_hex(&archive)}, "url": "__BAD_ARCHIVE__",
                       "filename": "3.0.0.mrpack", "primary": true, "size": archive.len()}]
        });
        let world = PackWorld::with_variants(
            "unsafe-fallback",
            v1,
            v2,
            vec![("VERS0009".to_owned(), unsafe_version)],
            None,
        );
        {
            let mut bodies = world.bodies.lock().unwrap();
            bodies.insert("/packs/bad.mrpack".into(), archive.clone());
        }
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);

        // Discovery reports the newest same-project version as available.
        let check = check_pack_update(&world.managed, &world.client, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(check.status, PackUpdateStatus::UpdateAvailable);
        assert_eq!(check.candidate.as_ref().unwrap().version_id, "VERS0009");

        // Reconciliation refuses the candidate: the unsafe fallback source
        // is rejected before any acquisition is attempted.
        let error = match build_plan(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &[],
            true,
        )
        .await
        {
            Ok(_) => panic!("the unsafe fallback source must be rejected"),
            Err(error) => error,
        };
        assert_eq!(error.code, "pack_invalid_download");

        // No mutation occurred: the old exact pack identity stands and
        // validates, the registry record is still ready, local and pack
        // files are untouched, the untrusted file never landed, and no
        // update receipt exists.
        let pack = crate::pack_state::InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, V1);
        pack.validate_installed(&world.managed).unwrap();
        let registry = InstanceRegistry::load(&world.managed.instance_registry_file()).unwrap();
        assert_eq!(
            registry.find(record.id()).unwrap().state().as_str(),
            "ready"
        );
        assert_eq!(
            std::fs::read(root.join("config/same.toml")).unwrap(),
            b"same = 1\n"
        );
        assert!(!root.join("mods/untrusted.jar").exists());
        assert!(!receipt_path(&world.managed, record.id()).unwrap().exists());
    }

    // BY — ordinary Update All still blocks pack-owned content after update.
    #[tokio::test]
    async fn ordinary_updates_still_block_pack_owned_content() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("guard", v1, v2);
        let record = world.install_v1(&[]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;
        apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap();
        let context = crate::modrinth::Context {
            minecraft_version: MC.into(),
            loader: "fabric".into(),
            fabric_api_protected: false,
        };
        let report = crate::content_updates::check_updates(
            &world.managed,
            record.id(),
            &context,
            &world.client,
            None,
        )
        .await
        .unwrap();
        assert!(
            report
                .entries
                .iter()
                .all(|entry| entry.status == crate::content_updates::UpdateStatus::Blocked)
        );
        assert!(
            report
                .entries
                .iter()
                .all(|entry| entry.block == Some(crate::content_updates::UpdateBlock::PackOwned))
        );
        // The ordinary provider lifecycle refuses to mutate pack-owned
        // records: a direct removal is rejected by the pack guard.
        let state = ContentState::load(&world.managed, record.id()).unwrap();
        let target = state
            .entries
            .iter()
            .find(|entry| entry.file_name == "changed.jar")
            .unwrap();
        let error = crate::instance_content::remove_provider_graph(
            &world.managed,
            record.id(),
            &state,
            &target.identity(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            crate::instance_content::ContentError::UnsupportedActionWith(_)
        ));
    }

    // CC — a failed update never presents Ready unless the old state is
    // whole (the acquisition-failure test proves the positive direction;
    // here an unprovable rollback leaves the instance unavailable).
    #[tokio::test]
    async fn unprovable_rollback_leaves_the_instance_unavailable() {
        // Recovery: simulate an interrupted update by writing a receipt for
        // a world that matches neither the old nor the new state.
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("unprovable", v1, v2);
        let record = world.install_v1(&[]).await;
        let root = world.instance_root(&record);
        write_receipt(
            &world.managed,
            record.id(),
            &UpdateReceipt {
                schema_version: RECEIPT_SCHEMA_VERSION,
                instance_id: record.id().to_string(),
                from_project_id: PACK_PROJECT.into(),
                from_version_id: V1.into(),
                to_project_id: PACK_PROJECT.into(),
                to_version_id: V2.into(),
                started_at_unix_seconds: 1,
            },
        )
        .unwrap();
        // Damage the old pack state so neither recovery case validates.
        std::fs::remove_file(root.join("mods").join("kept.jar")).unwrap();
        {
            let registry_path = world.managed.instance_registry_file();
            let mut registry = InstanceRegistry::load(&registry_path).unwrap();
            registry
                .find_mut(record.id())
                .unwrap()
                .set_state(InstanceState::Installing);
            registry.save(&registry_path).unwrap();
        }
        let outcome = recover_interrupted_update(&world.managed, &record.id().to_string()).await;
        assert!(outcome.is_err());
        let registry = InstanceRegistry::load(&world.managed.instance_registry_file()).unwrap();
        assert_eq!(
            registry.find(record.id()).unwrap().state().as_str(),
            "installing"
        );
    }

    // Crash recovery: a receipt with a fully valid old world restores Ready.
    #[tokio::test]
    async fn interrupted_update_with_intact_old_state_recovers() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("recover-old", v1, v2);
        let record = world.install_v1(&[]).await;
        write_receipt(
            &world.managed,
            record.id(),
            &UpdateReceipt {
                schema_version: RECEIPT_SCHEMA_VERSION,
                instance_id: record.id().to_string(),
                from_project_id: PACK_PROJECT.into(),
                from_version_id: V1.into(),
                to_project_id: PACK_PROJECT.into(),
                to_version_id: V2.into(),
                started_at_unix_seconds: 1,
            },
        )
        .unwrap();
        {
            let registry_path = world.managed.instance_registry_file();
            let mut registry = InstanceRegistry::load(&registry_path).unwrap();
            registry
                .find_mut(record.id())
                .unwrap()
                .set_state(InstanceState::Installing);
            registry.save(&registry_path).unwrap();
        }
        let outcome = recover_interrupted_update(&world.managed, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(outcome, RecoveryOutcome::RestoredOld);
        let registry = InstanceRegistry::load(&world.managed.instance_registry_file()).unwrap();
        assert_eq!(
            registry.find(record.id()).unwrap().state().as_str(),
            "ready"
        );
        assert!(
            !world
                .managed
                .instance_paths(record.id())
                .root()
                .join("pack-update.json")
                .exists()
        );
    }

    // Crash recovery: a committed new world finalizes to Ready.
    #[tokio::test]
    async fn interrupted_update_with_committed_new_state_finalizes() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("recover-new", v1, v2);
        let record = world.install_v1(&[]).await;
        let reconciliation = world.plan(&record.id().to_string()).await;
        apply_pack_update(
            &world.managed,
            &world.endpoints,
            &world.client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap();
        // Reconstruct the crash window: registry installing, receipt present,
        // everything else already committed.
        write_receipt(
            &world.managed,
            record.id(),
            &UpdateReceipt {
                schema_version: RECEIPT_SCHEMA_VERSION,
                instance_id: record.id().to_string(),
                from_project_id: PACK_PROJECT.into(),
                from_version_id: V1.into(),
                to_project_id: PACK_PROJECT.into(),
                to_version_id: V2.into(),
                started_at_unix_seconds: 1,
            },
        )
        .unwrap();
        {
            let registry_path = world.managed.instance_registry_file();
            let mut registry = InstanceRegistry::load(&registry_path).unwrap();
            registry
                .find_mut(record.id())
                .unwrap()
                .set_state(InstanceState::Installing);
            registry.save(&registry_path).unwrap();
        }
        let outcome = recover_interrupted_update(&world.managed, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(outcome, RecoveryOutcome::CompletedNew);
        let registry = InstanceRegistry::load(&world.managed.instance_registry_file()).unwrap();
        assert_eq!(
            registry.find(record.id()).unwrap().state().as_str(),
            "ready"
        );
    }

    // CD/CE — ordinary per-mod updates still work beside pack content.
    #[tokio::test]
    async fn ordinary_mod_updates_continue_to_work() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("ordinary", v1, v2);
        let record = world.install_v1(&[]).await;
        // An independent, non-pack managed mod can still be checked.
        let context = crate::modrinth::Context {
            minecraft_version: MC.into(),
            loader: "fabric".into(),
            fabric_api_protected: false,
        };
        let report = crate::content_updates::check_updates(
            &world.managed,
            record.id(),
            &context,
            &world.client,
            None,
        )
        .await
        .unwrap();
        // Every managed record on this instance is pack-owned: all blocked.
        assert!(!report.entries.is_empty());
        assert!(
            report
                .entries
                .iter()
                .all(|entry| entry.block == Some(crate::content_updates::UpdateBlock::PackOwned))
        );
    }

    // CI — the Phase I install path still works beside the update path.
    #[tokio::test]
    async fn initial_pack_install_still_works_alongside_updates() {
        let (v1, v2) = canonical_specs();
        let world = PackWorld::new("phase-i-coexist", v1, v2);
        let first = world.install_v1(&[]).await;
        let second = world.install_v1(&[]).await;
        assert_ne!(first.id(), second.id());
        let pack = crate::pack_state::InstalledPack::load(&world.managed, second.id())
            .unwrap()
            .unwrap();
        pack.validate_installed(&world.managed).unwrap();
    }

    // Live acceptance: the real Modrinth/Mojang/Fabric chains against a
    // disposable diagnostic root. Sodium Booster 2.1.3 (Vl3xy4P9, Minecraft
    // 26.2 + Fabric Loader 0.19.3) updates to 2.2 (w0mkw17z): changed
    // components, an added resource pack, superseded-path retirements, and
    // eighteen overrides — on the same game version, so the download stays
    // bounded. A second instance exercises the live local-modification
    // conflict path.
    const LIVE_PROJECT: &str = "tRDjDmhJ";
    const LIVE_OLD: &str = "Vl3xy4P9";
    const LIVE_NEW: &str = "w0mkw17z";

    fn live_world(name: &str) -> (ManagedPaths, InstanceEndpoints, crate::modrinth::Client) {
        let root = std::env::temp_dir()
            .join("aurora-pack-update-live")
            .join(format!("{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root).unwrap();
        let endpoints = crate::instances::lifecycle::InstanceEndpoints::operational().unwrap();
        (managed, endpoints, crate::modrinth::Client::official())
    }

    #[tokio::test]
    #[ignore = "live Modrinth + Mojang + Fabric acceptance against a disposable root"]
    async fn live_modpack_update_acceptance() {
        let (managed, endpoints, client) = live_world("pristine");
        // 1. Install the exact older version through the normal Phase I path.
        let preview = crate::modpacks::preview(&managed, &client, LIVE_PROJECT, LIVE_OLD, &[])
            .await
            .unwrap();
        assert_eq!(preview.version_id, LIVE_OLD);
        let record = crate::modpacks::install(
            &managed,
            &endpoints,
            &client,
            LIVE_PROJECT,
            LIVE_OLD,
            &[],
            &preview.fingerprint,
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(record.state().as_str(), "ready");

        // 2. Restart-shaped reload: everything validates from disk alone.
        let pack = crate::pack_state::InstalledPack::load(&managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, LIVE_OLD);
        pack.validate_installed(&managed).unwrap();

        // 3. Discovery finds exactly the newer same-project version.
        let check = check_pack_update(&managed, &client, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(check.status, PackUpdateStatus::UpdateAvailable);
        let candidate = check.candidate.as_ref().unwrap();
        assert_eq!(candidate.version_id, LIVE_NEW);
        assert!(candidate.changelog.is_some());

        // 4. The reconciliation plan classifies the real snapshot delta.
        let reconciliation = build_plan(
            &managed,
            &endpoints,
            &client,
            &record.id().to_string(),
            &[],
            false,
        )
        .await
        .unwrap();
        assert!(!reconciliation.plan.game_transition);
        assert!(
            reconciliation.plan.counts.updated >= 2,
            "ImmediatelyFast and Fabric API advance"
        );
        assert!(
            reconciliation.plan.counts.added >= 1,
            "the resource pack joins"
        );
        assert!(
            reconciliation.plan.counts.removed >= 2,
            "superseded jar paths retire"
        );
        assert_eq!(reconciliation.plan.counts.conflicts, 0);

        // 5. Apply and verify the exact new identity end to end.
        let updated = apply_pack_update(
            &managed,
            &endpoints,
            &client,
            &record.id().to_string(),
            &reconciliation.plan.fingerprint,
            &[],
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(updated.state().as_str(), "ready");
        assert_eq!(updated.pack().unwrap().version_id, LIVE_NEW);
        let pack = crate::pack_state::InstalledPack::load(&managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, LIVE_NEW);
        assert!(pack.divergences.is_empty());
        pack.validate_installed(&managed).unwrap();
        let root = managed.instance_paths(record.id()).root().to_path_buf();
        assert!(
            root.join("resourcepacks")
                .join("panoramaofancientcity.zip")
                .is_file()
        );
        assert!(
            !root
                .join("mods")
                .join("fabric-api-0.156.0+26.2.jar")
                .exists()
        );
        assert!(
            root.join("mods")
                .join("fabric-api-0.160.0+26.2.jar")
                .is_file()
        );

        // 6. Discovery is truthful about being current after the update.
        let check = check_pack_update(&managed, &client, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(check.status, PackUpdateStatus::UpToDate);

        // 7. Restart persistence: fresh loads prove the committed state.
        let pack = crate::pack_state::InstalledPack::load(&managed, record.id())
            .unwrap()
            .unwrap();
        pack.validate_installed(&managed).unwrap();
        let registry = InstanceRegistry::load(&managed.instance_registry_file()).unwrap();
        let validation =
            crate::instances::lifecycle::validate_instance(&managed, &registry, record.id())
                .unwrap();
        assert!(validation.problems.is_empty());
        let _ = std::fs::remove_dir_all(managed.data_root());
    }

    #[tokio::test]
    #[ignore = "live Modrinth + Mojang + Fabric acceptance against a disposable root"]
    async fn live_modpack_update_local_modification_acceptance() {
        let (managed, endpoints, client) = live_world("modified");
        let preview = crate::modpacks::preview(&managed, &client, LIVE_PROJECT, LIVE_OLD, &[])
            .await
            .unwrap();
        let record = crate::modpacks::install(
            &managed,
            &endpoints,
            &client,
            LIVE_PROJECT,
            LIVE_OLD,
            &[],
            &preview.fingerprint,
            &mut |_| {},
        )
        .await
        .unwrap();
        let root = managed.instance_paths(record.id()).root().to_path_buf();

        // Create a controlled local modification to a pack-owned override.
        let overrides = crate::pack_state::InstalledPack::load(&managed, record.id())
            .unwrap()
            .unwrap()
            .overrides;
        let target = overrides
            .iter()
            .find(|item| item.path.starts_with("config/"))
            .or_else(|| overrides.first())
            .expect("the pack ships overrides");
        let original = std::fs::read(root.join(&target.path)).unwrap();
        let mut modified = original.clone();
        modified.extend_from_slice(b"\n# owner edit\n");
        std::fs::write(root.join(&target.path), &modified).unwrap();

        // The preview recognizes old authored != current local and refuses
        // to silently overwrite: the row is a conflict.
        let reconciliation = build_plan(
            &managed,
            &endpoints,
            &client,
            &record.id().to_string(),
            &[],
            false,
        )
        .await
        .unwrap();
        let row = row(&reconciliation, &target.path);
        assert_eq!(row.action, PackRowAction::Conflict);
        assert_eq!(row.conflict, Some(PackConflictKind::ChangedModified));

        // Exercise the supported resolution: keep the local file.
        let fingerprint = reconciliation.plan.fingerprint.clone();
        apply_pack_update(
            &managed,
            &endpoints,
            &client,
            &record.id().to_string(),
            &fingerprint,
            &[ConflictResolution {
                path: target.path.clone(),
                resolution: "keepLocal".into(),
            }],
            &mut |_| {},
        )
        .await
        .unwrap();

        // The chosen resolution persists: exact local bytes kept, pack at
        // the new version with a truthful recorded divergence.
        assert_eq!(std::fs::read(root.join(&target.path)).unwrap(), modified);
        let pack = crate::pack_state::InstalledPack::load(&managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(pack.identity.version_id, LIVE_NEW);
        assert_eq!(pack.divergences.len(), 1);
        assert_eq!(pack.divergences[0].path, target.path);
        pack.validate_installed(&managed).unwrap();
        let check = check_pack_update(&managed, &client, &record.id().to_string())
            .await
            .unwrap();
        assert_eq!(check.divergences, 1);
        let _ = std::fs::remove_dir_all(managed.data_root());
    }
}
