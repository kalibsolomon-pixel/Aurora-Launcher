//! The Aurora Client production update: discovery, planning, and the
//! verified transactional activation of one newer intentionally published
//! release on one existing instance.
//!
//! Authority is the manually published public release manifest — never a
//! commit, branch head, or CI build. The remote manifest is bootstrap
//! discovery metadata (HTTPS plus strict parsing, exactly like the official
//! Mojang version manifest); the artifact it describes is acquired through
//! the SHA-256 verified store and only then activated. Discovery and
//! preview never mutate anything; activation is a fingerprinted
//! staged/backed-up transaction whose registry pin moves only after the
//! whole instance re-validates, and the previous valid Client jar stays
//! recoverable until the new release has activated and validated.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::Digest as _;
use url::Url;

use crate::aurora::{self, AuroraInstalledState};
use crate::cache::ArtifactCache;
use crate::distribution::{AuroraRelease, ManifestError, ReleaseManifest};
use crate::downloads::{self, ArtifactSource, DownloadOptions};
use crate::instance_content::{ContentState, ContentType};
use crate::instance_mods;
use crate::instances::lifecycle;
use crate::instances::platform::InstalledConfiguration;
use crate::instances::transition;
use crate::instances::{InstanceId, InstanceRecord, InstanceRegistry, InstanceState};
use crate::integrity::{ArtifactDigest, verify_file};
use crate::paths::ManagedPaths;
use crate::updates::{UpdateAvailability, parse_release_version};

/// The location the owner's manual publication gate exposes the Aurora
/// Client release manifest at. It exists only when the owner has published
/// it; before that, checks fail non-fatally as unavailable.
///
/// Diagnostic builds may compile a different endpoint (loopback) through
/// `AURORA_CLIENT_MANIFEST_URL`; production builds never set it. The
/// endpoint never weakens artifact trust — expected digests still come from
/// the manifest and verify the acquired bytes.
pub const PRODUCTION_MANIFEST_URL: &str = match option_env!("AURORA_CLIENT_MANIFEST_URL") {
    Some(diagnostic) => diagnostic,
    None => {
        "https://github.com/kalibsolomon-pixel/Aurora-Client/releases/download/release-manifest/aurora-releases.json"
    }
};

/// Hard cap for the fetched manifest document. Manifests are small; a
/// hostile endpoint must not stream unbounded data into memory.
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

/// A failed Client update check or transaction.
#[derive(Debug)]
pub enum ClientUpdateError {
    /// The instance is not in a state where an Aurora Client update applies.
    Inapplicable(String),
    /// The published manifest could not be fetched (offline, endpoint).
    ManifestUnavailable(String),
    /// The published manifest is malformed or untrusted.
    ManifestInvalid(String),
    /// The candidate cannot be proven compatible with the instance.
    Incompatible(String),
    /// The transaction is blocked by an explicit, named condition.
    Blocked(String),
    /// The approved plan no longer matches the world.
    Stale(String),
    /// The instance's game process is running.
    InstanceRunning,
    /// Acquisition, staging, activation, validation, or commit failed.
    Transaction(&'static str, String),
}

impl ClientUpdateError {
    /// The stable command error code for this failure.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Inapplicable(_) => "update_inapplicable",
            Self::ManifestUnavailable(_) => "update_check_unavailable",
            Self::ManifestInvalid(_) => "update_metadata_invalid",
            Self::Incompatible(_) => "update_incompatible",
            Self::Blocked(_) => "update_blocked",
            Self::Stale(_) => "update_stale",
            Self::InstanceRunning => "update_instance_running",
            Self::Transaction(code, _) => code,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Inapplicable(reason)
            | Self::ManifestUnavailable(reason)
            | Self::ManifestInvalid(reason)
            | Self::Incompatible(reason)
            | Self::Blocked(reason)
            | Self::Stale(reason)
            | Self::Transaction(_, reason) => reason.clone(),
            Self::InstanceRunning => "Quit Minecraft before updating the Aurora Client.".to_owned(),
        }
    }
}

impl From<ManifestError> for ClientUpdateError {
    fn from(error: ManifestError) -> Self {
        Self::ManifestInvalid(error.to_string())
    }
}

impl std::fmt::Display for ClientUpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message())
    }
}

/// Fetches the published release manifest. Bootstrap discovery metadata:
/// bounded, HTTPS-or-loopback, strictly parsed, never a "verified artifact".
pub async fn fetch_published_manifest(
    url: &str,
    options: &DownloadOptions,
) -> Result<ReleaseManifest, ClientUpdateError> {
    let parsed = Url::parse(url).map_err(|_| {
        ClientUpdateError::ManifestUnavailable("the manifest URL is not valid".into())
    })?;
    if parsed.scheme() == "https" {
        // Production transport.
    } else if parsed.scheme() == "http" && downloads::is_loopback_host(&parsed) {
        // The documented diagnostic/test transport only.
    } else {
        return Err(ClientUpdateError::ManifestUnavailable(
            "the manifest endpoint must use HTTPS".into(),
        ));
    }

    let client = downloads::build_client(options);
    let response = client
        .get(parsed)
        .send()
        .await
        .map_err(|error| ClientUpdateError::ManifestUnavailable(error.to_string()))?;
    if !response.status().is_success() {
        return Err(ClientUpdateError::ManifestUnavailable(format!(
            "the manifest endpoint answered HTTP {}",
            response.status().as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|declared| declared as usize > MAX_MANIFEST_BYTES)
    {
        return Err(ClientUpdateError::ManifestInvalid(
            "the published manifest exceeds the size limit".into(),
        ));
    }

    let mut document = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| ClientUpdateError::ManifestUnavailable(error.to_string()))?
    {
        if document.len() + chunk.len() > MAX_MANIFEST_BYTES {
            return Err(ClientUpdateError::ManifestInvalid(
                "the published manifest exceeds the size limit".into(),
            ));
        }
        document.extend_from_slice(&chunk);
    }
    let text = String::from_utf8(document).map_err(|_| {
        ClientUpdateError::ManifestInvalid("the published manifest is not valid UTF-8".into())
    })?;
    ReleaseManifest::from_json(&text).map_err(ClientUpdateError::from)
}

/// The outcome of candidate selection for one instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateOutcome {
    /// An eligible, compatible, semver-newer release exists.
    UpdateAvailable(AuroraRelease),
    /// The installed release is the newest eligible compatible one.
    UpToDate,
    /// The installed release is newer than everything eligible; the truth is
    /// reported instead of silently downgrading to a manifest version.
    InstalledNewer,
}

/// Selects the update candidate for one installed Client against one
/// intentionally published manifest. Legacy channel metadata has no eligibility role.
///
/// Publication is authority; "newer" is semver precedence only;
/// compatibility is exact environment identity (Minecraft version and
/// Fabric Loader version). Malformed version strings never become
/// candidates, and an installed version that cannot be compared fails
/// closed rather than guessing.
pub fn select_candidate(
    remote: &ReleaseManifest,
    installed: &InstalledConfiguration,
    installed_state: &AuroraInstalledState,
    required_java_major: u32,
) -> Result<CandidateOutcome, ClientUpdateError> {
    let Some(pin) = &installed.aurora else {
        return Err(ClientUpdateError::Inapplicable(
            "the Aurora Client is not installed on this instance".into(),
        ));
    };
    let installed_version = parse_release_version(&pin.version).ok_or_else(|| {
        ClientUpdateError::Incompatible(format!(
            "the installed Aurora version '{}' cannot be compared; no update is offered",
            pin.version
        ))
    })?;

    let mut newest: Option<(semver::Version, AuroraRelease)> = None;
    for release in remote.releases() {
        // Exact environment identity: a release for another Minecraft or
        // Loader version is incompatible with this instance, whatever its
        // version number says.
        if release.minecraft_version() != installed_state.minecraft_version()
            || release.fabric_loader_version() != installed_state.fabric_loader_version()
            || release.java().major_version() != required_java_major
        {
            continue;
        }
        let Some(version) = parse_release_version(release.aurora_version()) else {
            // Malformed version strings never become candidates.
            continue;
        };
        if !version.cmp_precedence(&installed_version).is_gt() {
            continue;
        }
        if newest
            .as_ref()
            .is_none_or(|(known, _)| version.cmp_precedence(known).is_gt())
        {
            newest = Some((version, release.clone()));
        }
    }

    Ok(match newest {
        Some((_, release)) => CandidateOutcome::UpdateAvailable(release),
        None => {
            if manifest_holds_older_eligible_release(
                remote,
                installed_state,
                &installed_version,
                required_java_major,
            ) {
                CandidateOutcome::InstalledNewer
            } else {
                CandidateOutcome::UpToDate
            }
        }
    })
}

/// Whether the manifest knows an eligible release for the instance's exact
/// environment that is older than the installed one — the honest signal
/// that the installed Client is ahead of the manifest.
fn manifest_holds_older_eligible_release(
    remote: &ReleaseManifest,
    installed_state: &AuroraInstalledState,
    installed_version: &semver::Version,
    required_java_major: u32,
) -> bool {
    remote.releases().iter().any(|release| {
        release.minecraft_version() == installed_state.minecraft_version()
            && release.fabric_loader_version() == installed_state.fabric_loader_version()
            && release.java().major_version() == required_java_major
            && parse_release_version(release.aurora_version())
                .is_some_and(|version| version.cmp_precedence(installed_version).is_lt())
    })
}

/// The user-facing candidate summary carried by previews.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUpdateCandidate {
    pub version: String,
    pub notes: Option<String>,
    pub minecraft_version: String,
    pub fabric_loader_version: String,
    pub java_major_version: u32,
    pub size_bytes: Option<u64>,
    pub sha256: String,
    pub fabric_api_version: Option<String>,
}

impl ClientUpdateCandidate {
    fn from_release(release: &AuroraRelease) -> Self {
        Self {
            version: release.aurora_version().to_owned(),
            notes: release.notes().map(|notes| notes.to_owned()),
            minecraft_version: release.minecraft_version().to_owned(),
            fabric_loader_version: release.fabric_loader_version().to_owned(),
            java_major_version: release.java().major_version(),
            size_bytes: release.artifact().size_bytes(),
            sha256: release.artifact().sha256().to_owned(),
            fabric_api_version: release.fabric_api().map(|api| api.version().to_owned()),
        }
    }
}

/// A read-only, fingerprinted update plan for one instance.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUpdatePreview {
    pub instance_id: String,
    pub installed_version: String,
    /// `updateAvailable`, `upToDate`, or `installedNewer`.
    pub outcome: &'static str,
    pub candidate: Option<ClientUpdateCandidate>,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub fingerprint: String,
}

/// Computes the read-only update preview for one instance against one
/// already-fetched published manifest.
///
/// Nothing here mutates the instance; blockers name every condition that
/// must clear before [`apply_update`] may run.
pub fn preview_update(
    managed: &ManagedPaths,
    endpoints: &lifecycle::InstanceEndpoints,
    id: &InstanceId,
    remote: &ReleaseManifest,
) -> Result<ClientUpdatePreview, ClientUpdateError> {
    let invalid = |reason: String| ClientUpdateError::Inapplicable(reason);
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| invalid(error.to_string()))?;
    let record = registry
        .find(id)
        .ok_or_else(|| invalid("instance not found".into()))?
        .clone();
    if record.state() != InstanceState::Ready
        || !record.configuration().matches_installed(record.installed())
    {
        return Err(invalid(
            "finish installation and resolve stale configuration first".into(),
        ));
    }
    let Some(pin) = record.installed().aurora.clone() else {
        return Err(invalid(
            "the Aurora Client is not installed on this instance".into(),
        ));
    };
    let state = aurora::load_installed_state(managed, id)
        .map_err(|error| invalid(error.to_string()))?
        .ok_or_else(|| invalid("no Aurora installation is present".into()))?;
    if state.aurora_version() != pin.version
        || state.channel() != pin.channel
        || state.minecraft_version() != record.installed().minecraft_version
        || Some(state.fabric_loader_version()) != record.installed().platform.version()
    {
        return Err(invalid(
            "the Aurora installation disagrees with the instance pin".into(),
        ));
    }

    // The current release must stay resolvable (embedded manifest plus the
    // locally persisted release metadata of a prior remote update).
    let merged = aurora::merged_release_manifest(managed, id, endpoints.release_manifest())
        .map_err(|error| invalid(error.to_string()))?;
    let current_release = merged
        .resolve_exact(&pin.version, Some(pin.channel))
        .ok_or_else(|| {
            invalid("the installed release metadata is missing; no update can be planned".into())
        })?;

    let validation = lifecycle::validate_instance(managed, &registry, id)
        .map_err(|error| invalid(error.to_string()))?;
    if validation.status != lifecycle::InstanceStatus::Ready {
        return Err(invalid(format!(
            "the instance must validate before an update; {}",
            validation
                .problems
                .iter()
                .map(|problem| format!("{}: {}", problem.component, problem.reason))
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    let outcome = select_candidate(
        remote,
        record.installed(),
        &state,
        current_release.java().major_version(),
    )?;
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let mut candidate_release = None;
    let mut candidate_dto = None;
    let mut outcome_label = "upToDate";

    if let CandidateOutcome::UpdateAvailable(release) = &outcome {
        outcome_label = "updateAvailable";
        candidate_release = Some(release.clone());
        candidate_dto = Some(ClientUpdateCandidate::from_release(release));

        // A Running/Starting game process blocks mutation at the backend,
        // never merely in the UI.
        if crate::launch::process::snapshot(id.as_str())
            .status
            .blocks_launch()
        {
            blockers.push("Quit Minecraft before updating the Aurora Client.".into());
        }

        // Destination collisions the launcher refuses to guess at.
        let aurora_target = aurora::managed_artifact_relative_path(release)
            .map_err(|error| invalid(error.to_string()))?;
        let retained =
            transition::retained_files(managed, id).map_err(|error| invalid(error.to_string()))?;
        if retained
            .iter()
            .any(|file| file.relative_path.eq_ignore_ascii_case(&aurora_target))
        {
            blockers.push(format!(
                "Retained former Aurora content occupies {aurora_target}; resolve it in Mods first."
            ));
        }
        let provider =
            ContentState::load(managed, id).map_err(|error| invalid(error.to_string()))?;
        let inventory =
            instance_mods::scan(managed, id).map_err(|error| invalid(error.to_string()))?;
        let aurora_name = aurora_target.strip_prefix("mods/").expect("managed path");
        if provider.entries.iter().any(|entry| {
            entry.content_type == ContentType::Mod
                && entry.file_name.eq_ignore_ascii_case(aurora_name)
        }) {
            blockers.push(format!(
                "Provider-managed content already occupies {aurora_name}; it will not be overwritten."
            ));
        }
        // A foreign second Aurora identity (a user-installed copy) blocks;
        // the launcher-managed old artifact being updated does not.
        let managed_name = state
            .artifact()
            .relative_path()
            .strip_prefix("mods/")
            .expect("managed mod path");
        if inventory.entries.iter().any(|entry| {
            entry.enabled
                && !entry.file_name.eq_ignore_ascii_case(aurora_name)
                && !entry.file_name.eq_ignore_ascii_case(managed_name)
                && entry.metadata.as_ref().is_some_and(|metadata| {
                    metadata.id == "aurora"
                        || metadata
                            .nested_mod_ids
                            .iter()
                            .any(|nested| nested == "aurora")
                })
        }) {
            blockers.push(
                "Another active Aurora identity is installed; it must be removed first.".into(),
            );
        }

        // An active provider-owned Fabric API may be carried across only
        // when it is exactly the release's pinned version.
        if aurora::compatible_provider_api(managed, id, release)
            .map_err(|error| invalid(error.to_string()))?
        {
            let pinned = release
                .fabric_api()
                .expect("a provider-owned API implies the release pins one");
            let installed_api = state.fabric_api().map(|api| api.version());
            if installed_api != Some(pinned.version()) {
                blockers.push(format!(
                    "The installed provider-managed Fabric API ({}) differs from the release's pinned {}; update it in Mods first.",
                    installed_api.unwrap_or("unknown"),
                    pinned.version()
                ));
            }
        }

        // The release's own bootstrap preflight: disabled variants, foreign
        // bytes at a planned destination, provider-collision rules.
        if let Err(error) = aurora::preflight_bootstrap(managed, id, release) {
            blockers.push(error.to_string());
        }
    } else if matches!(outcome, CandidateOutcome::InstalledNewer) {
        outcome_label = "installedNewer";
        warnings.push(
            "The installed Aurora Client is newer than every eligible published release; it will not be downgraded."
                .into(),
        );
    }

    let fingerprint = fingerprint_update(&record, &state, managed, id, candidate_release.as_ref())
        .map_err(|error| invalid(error.to_string()))?;

    Ok(ClientUpdatePreview {
        instance_id: id.to_string(),
        installed_version: pin.version,
        outcome: outcome_label,
        candidate: candidate_dto,
        blockers,
        warnings,
        fingerprint,
    })
}

/// Hashes the exact world the approval covered: the registry record, the
/// installed state bytes, retained ownership, provider state, the mod
/// inventory, the installed game manifest, the full candidate release. Any drift refuses the stale plan.
fn fingerprint_update(
    record: &InstanceRecord,
    state: &AuroraInstalledState,
    managed: &ManagedPaths,
    id: &InstanceId,
    candidate: Option<&AuroraRelease>,
) -> Result<String, ClientUpdateError> {
    let instance_paths = managed.instance_paths(id);
    let root = instance_paths.root();
    let ownership_bytes = std::fs::read(root.join(aurora::AURORA_INSTALLED_FILE_NAME))
        .map_err(|error| ClientUpdateError::Inapplicable(error.to_string()))?;
    let retained = transition::retained_files(managed, id)
        .map_err(|error| ClientUpdateError::Inapplicable(error.to_string()))?;
    let provider = ContentState::load(managed, id)
        .map_err(|error| ClientUpdateError::Inapplicable(error.to_string()))?;
    let inventory = instance_mods::scan(managed, id)
        .map_err(|error| ClientUpdateError::Inapplicable(error.to_string()))?;
    let game_manifest =
        crate::install::state::load_installed_state(managed.instance_paths(id).game())
            .map_err(|error| ClientUpdateError::Inapplicable(error.to_string()))?;
    let snapshot = serde_json::json!({
        "record": record,
        "state": state,
        "ownershipBytes": ownership_bytes,
        "retained": retained,
        "provider": provider,
        "inventory": inventory,
        "game": game_manifest,
        "candidate": candidate,
    });
    let bytes = serde_json::to_vec(&snapshot)
        .map_err(|error| ClientUpdateError::Inapplicable(error.to_string()))?;
    Ok(format!("{:x}", sha2::Sha256::digest(bytes)))
}

/// Faults consumed only by deterministic tests, never by commands.
#[derive(Default, Clone, Copy)]
pub struct UpdateFaults {
    pub fail_stage: bool,
    pub fail_activation: bool,
    pub fail_validation: bool,
    pub fail_persistence: bool,
}

/// Executes one approved Aurora Client update as a verified, staged,
/// backed-up, revalidated transaction.
///
/// The old valid Client jar stays recoverable until the new release has
/// activated, the whole instance re-validated, and the registry pin moved;
/// any failure restores the exact previous bytes.
#[allow(clippy::too_many_arguments)]
pub async fn apply_update(
    managed: &ManagedPaths,
    endpoints: &lifecycle::InstanceEndpoints,
    id: &InstanceId,
    fingerprint: &str,
    remote: &ReleaseManifest,
    options: &DownloadOptions,
    progress: &mut (dyn FnMut(crate::updates::ClientPhase) + Send),
    faults: UpdateFaults,
) -> Result<InstanceRecord, ClientUpdateError> {
    let approved = preview_update(managed, endpoints, id, remote)?;
    if approved.fingerprint != fingerprint {
        return Err(ClientUpdateError::Stale(
            "The approved update changed. Request a new preview.".into(),
        ));
    }
    if !approved.blockers.is_empty() {
        return Err(ClientUpdateError::Blocked(approved.blockers.join(" ")));
    }
    let release = approved
        .candidate
        .as_ref()
        .and_then(|candidate| remote.resolve_exact(&candidate.version, None))
        .cloned()
        .ok_or_else(|| ClientUpdateError::Stale("The approved candidate vanished".into()))?;

    // The Java assertion uses the resolved game plan, the same authority as
    // transitions; a release whose Java requirement disagrees with the
    // instance's resolved game never installs.
    {
        let registry = InstanceRegistry::load(&managed.instance_registry_file())
            .map_err(|error| ClientUpdateError::Inapplicable(error.to_string()))?;
        let record = registry
            .find(id)
            .ok_or_else(|| ClientUpdateError::Inapplicable("instance not found".into()))?
            .clone();
        let current_release = lifecycle::resolve_optional_aurora(managed, endpoints, &record)
            .map_err(|error| ClientUpdateError::Inapplicable(error.to_string()))?;
        let plan = lifecycle::resolve_record_game_plan(
            managed,
            endpoints,
            &record,
            current_release.as_ref(),
        )
        .await
        .map_err(|error| ClientUpdateError::Incompatible(error.to_string()))?;
        if plan.java().major_version() != release.java().major_version() {
            return Err(ClientUpdateError::Incompatible(
                "Aurora Java compatibility assertion disagrees with the resolved game requirement"
                    .into(),
            ));
        }
    }

    progress(crate::updates::ClientPhase::Acquiring);
    let cache = ArtifactCache::new(managed.clone());
    let provider_api = aurora::compatible_provider_api(managed, id, &release).map_err(|error| {
        ClientUpdateError::Transaction("update_preflight_failed", error.to_string())
    })?;
    let mut acquired = Vec::new();
    let aurora_source = ArtifactSource::https_or_loopback(
        release.artifact().url(),
        release.artifact().sha256(),
        release.artifact().size_bytes(),
    )
    .map_err(|error| ClientUpdateError::ManifestInvalid(error.to_string()))?;
    let aurora_digest = ArtifactDigest::parse(release.artifact().sha256())
        .map_err(|error| ClientUpdateError::ManifestInvalid(error.to_string()))?;
    let aurora_verified = cache
        .acquire_with(&aurora_source, options)
        .await
        .map_err(|error| {
            ClientUpdateError::Transaction("update_download_failed", error.to_string())
        })?;
    acquired.push((
        aurora::managed_artifact_relative_path(&release)
            .map_err(|error| ClientUpdateError::ManifestInvalid(error.to_string()))?,
        aurora_digest,
        aurora_verified.bytes,
        aurora_verified.path,
    ));
    if let Some(api) = release.fabric_api().filter(|_| !provider_api) {
        let digest = ArtifactDigest::parse(api.artifact().sha256())
            .map_err(|error| ClientUpdateError::ManifestInvalid(error.to_string()))?;
        let source = ArtifactSource::https_or_loopback(
            api.artifact().url(),
            api.artifact().sha256(),
            api.artifact().size_bytes(),
        )
        .map_err(|error| ClientUpdateError::ManifestInvalid(error.to_string()))?;
        let verified = cache
            .acquire_with(&source, options)
            .await
            .map_err(|error| {
                ClientUpdateError::Transaction("update_download_failed", error.to_string())
            })?;
        acquired.push((
            format!("mods/fabric-api-{}.jar", api.version()),
            digest,
            verified.bytes,
            verified.path,
        ));
    }

    let mut outcome = None;
    let locked = crate::instance_content::with_instance_lock(id, || {
        let _registry_guard = lifecycle::registry_lock();
        outcome = Some(commit_update(
            managed,
            endpoints,
            id,
            remote,
            &release,
            &acquired,
            provider_api,
            fingerprint,
            progress,
            faults,
        ));
        Ok::<(), crate::instance_content::ContentError>(())
    });
    locked
        .map_err(|error| ClientUpdateError::Transaction("update_lock_failed", error.to_string()))?;
    outcome.expect("transaction closure executed")
}

#[allow(clippy::too_many_arguments)]
fn commit_update(
    managed: &ManagedPaths,
    endpoints: &lifecycle::InstanceEndpoints,
    id: &InstanceId,
    remote: &ReleaseManifest,
    release: &AuroraRelease,
    acquired: &[(String, ArtifactDigest, u64, PathBuf)],
    provider_api: bool,
    fingerprint: &str,
    progress: &mut (dyn FnMut(crate::updates::ClientPhase) + Send),
    faults: UpdateFaults,
) -> Result<InstanceRecord, ClientUpdateError> {
    let invalid =
        |reason: String| ClientUpdateError::Transaction("update_transaction_failed", reason);

    // The world must still match the approval exactly, re-derived under the
    // instance and registry locks.
    let current = preview_update(managed, endpoints, id, remote)?;
    if current.fingerprint != fingerprint {
        return Err(ClientUpdateError::Stale(
            "State changed during acquisition. Request a new preview.".into(),
        ));
    }
    if !current.blockers.is_empty() {
        return Err(ClientUpdateError::Blocked(current.blockers.join(" ")));
    }

    let registry_path = managed.instance_registry_file();
    regular(&registry_path).map_err(|e| invalid(e.to_string()))?;
    let old_registry = std::fs::read(&registry_path).map_err(|e| invalid(e.to_string()))?;
    let mut registry =
        InstanceRegistry::load(&registry_path).map_err(|error| invalid(error.to_string()))?;
    registry
        .find(id)
        .ok_or_else(|| invalid("instance not found".into()))?;
    let root = managed.instance_paths(id).root().to_owned();
    let state_path = root.join(aurora::AURORA_INSTALLED_FILE_NAME);
    let release_path = root.join(aurora::INSTANCE_RELEASE_FILE_NAME);
    let old_state = optional_bytes(&state_path).map_err(|e| invalid(e.to_string()))?;
    let old_release_doc = optional_bytes(&release_path).map_err(|e| invalid(e.to_string()))?;

    let _process_guard = crate::launch::process::lock_stopped(id.as_str())
        .map_err(|_| ClientUpdateError::InstanceRunning)?;

    let mut staged: Vec<(PathBuf, PathBuf, ArtifactDigest, u64)> = Vec::new();
    let mut retired: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut activated: Vec<PathBuf> = Vec::new();
    let mut state_touched = false;
    let mut registry_touched = false;

    let result = (|| {
        // Stage every new artifact beside its destination and verify the
        // staged bytes before anything installed changes.
        progress(crate::updates::ClientPhase::Staging);
        for (relative, digest, bytes, source) in acquired {
            let target = root.join(relative.split('/').collect::<PathBuf>());
            if !target.starts_with(managed.instance_paths(id).mods()) {
                return Err(invalid(
                    "the derived path escaped the managed mods directory".into(),
                ));
            }
            regular(source).map_err(|e| invalid(e.to_string()))?;
            verify_file(source, digest, Some(*bytes)).map_err(|e| invalid(e.to_string()))?;
            let stage = target
                .parent()
                .expect("mods child")
                .join(format!(".aurora-update-stage-{}", uuid::Uuid::new_v4()));
            staged.push((target.clone(), stage.clone(), *digest, *bytes));
            std::fs::copy(source, &stage).map_err(|e| invalid(e.to_string()))?;
            verify_file(&stage, digest, Some(*bytes)).map_err(|e| invalid(e.to_string()))?;
        }
        if faults.fail_stage {
            return Err(invalid("injected staging failure".into()));
        }

        // Retire the previous valid artifacts by rename: fully recoverable
        // until the new release commits.
        let previous = aurora::load_installed_state(managed, id)
            .map_err(|error| invalid(error.to_string()))?
            .ok_or_else(|| invalid("no Aurora installation is present".into()))?;
        for artifact in std::iter::once(previous.artifact()).chain(
            previous
                .fabric_api()
                .filter(|_| !provider_api)
                .map(|api| api.artifact()),
        ) {
            let target = root.join(artifact.relative_path().split('/').collect::<PathBuf>());
            if staged
                .iter()
                .any(|(destination, _, _, _)| destination == &target)
            {
                continue;
            }
            match std::fs::symlink_metadata(&target) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(invalid(error.to_string())),
                Ok(_) => {
                    let backup = target
                        .parent()
                        .expect("mods child")
                        .join(format!(".aurora-update-retired-{}", uuid::Uuid::new_v4()));
                    std::fs::rename(&target, &backup).map_err(|e| invalid(e.to_string()))?;
                    retired.push((target.clone(), backup));
                }
            }
        }

        progress(crate::updates::ClientPhase::Activating);
        for (target, stage, digest, bytes) in &staged {
            match std::fs::symlink_metadata(target) {
                // An already-present destination with byte-identical
                // verified content is reused: the same-pinned Fabric API
                // across releases, or a prior interrupted attempt of this
                // exact release. Foreign bytes were already refused by the
                // preflight.
                Ok(_) => {
                    verify_file(target, digest, Some(*bytes)).map_err(|e| {
                        invalid(format!(
                            "the destination {} already exists with different bytes: {e}",
                            target.display()
                        ))
                    })?;
                    let _ = std::fs::remove_file(stage);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    // Refuse an external collision rather than overwriting it.
                    std::fs::hard_link(stage, target).map_err(|e| invalid(e.to_string()))?;
                    activated.push(target.clone());
                }
                Err(error) => return Err(invalid(error.to_string())),
            }
        }
        if faults.fail_activation {
            return Err(invalid("injected activation failure".into()));
        }

        progress(crate::updates::ClientPhase::Validating);
        let aurora_size = acquired
            .iter()
            .find(|(relative, _, _, _)| relative.starts_with("mods/aurora-"))
            .expect("the Aurora artifact is always acquired")
            .2;
        let api_size = acquired
            .iter()
            .find(|(relative, _, _, _)| relative.starts_with("mods/fabric-api-"))
            .map(|(_, _, bytes, _)| *bytes);
        let previous = aurora::load_installed_state(managed, id)
            .map_err(|error| invalid(error.to_string()))?;
        let mut state =
            aurora::AuroraInstalledState::for_verified_release(release, aurora_size, api_size)
                .map_err(|error| invalid(error.to_string()))?;
        if provider_api {
            // Carry the provider-owned Fabric API exactly as installed.
            state.set_provider_fabric_api(
                previous.as_ref().and_then(|old| old.fabric_api().cloned()),
            );
        }
        aurora::write_installed_state(managed, id, &state)
            .map_err(|error| invalid(error.to_string()))?;
        aurora::write_instance_release(managed, id, release)
            .map_err(|error| invalid(error.to_string()))?;
        state_touched = true;

        // The registry pin moves only now, after activation and validation.
        let record_handle = registry
            .find_mut(id)
            .ok_or_else(|| invalid("instance not found".into()))?;
        let mut installed = record_handle.installed().clone();
        installed.aurora = Some(crate::instances::platform::AuroraPin {
            channel: release.channel(),
            version: release.aurora_version().to_owned(),
        });
        record_handle.set_installed(installed);
        let updated = record_handle.clone();

        let validation = lifecycle::validate_instance(managed, &registry, id)
            .map_err(|error| invalid(error.to_string()))?;
        if validation.status != lifecycle::InstanceStatus::Ready {
            return Err(invalid(format!(
                "the updated instance failed final validation: {}",
                validation
                    .problems
                    .iter()
                    .map(|problem| format!("{}: {}", problem.component, problem.reason))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        if faults.fail_validation {
            return Err(invalid("injected validation failure".into()));
        }

        progress(crate::updates::ClientPhase::Committing);
        if faults.fail_persistence {
            return Err(invalid("injected persistence failure".into()));
        }
        registry
            .save(&registry_path)
            .map_err(|error| invalid(error.to_string()))?;
        registry_touched = true;

        let merged = aurora::merged_release_manifest(managed, id, endpoints.release_manifest())
            .map_err(|error| invalid(error.to_string()))?;
        instance_mods::verified_required_mods_with_manifest(managed, id, &merged)
            .map_err(|error| invalid(error.to_string()))?;

        for (_, backup) in &retired {
            regular(backup).map_err(|e| invalid(e.to_string()))?;
            std::fs::remove_file(backup).map_err(|e| invalid(e.to_string()))?;
        }
        Ok(updated)
    })();

    if let Err(error) = result {
        let rollback = (|| {
            for target in activated.iter().rev() {
                std::fs::remove_file(target).map_err(|e| invalid(e.to_string()))?;
            }
            for (target, backup) in retired.iter().rev() {
                std::fs::rename(backup, target).map_err(|e| invalid(e.to_string()))?;
            }
            if state_touched {
                atomic_write(&state_path, old_state.as_deref())
                    .map_err(|e| invalid(e.to_string()))?;
                atomic_write(&release_path, old_release_doc.as_deref())
                    .map_err(|e| invalid(e.to_string()))?;
            }
            if registry_touched {
                atomic_write(&registry_path, Some(&old_registry))
                    .map_err(|e| invalid(e.to_string()))?;
            }
            Ok::<(), ClientUpdateError>(())
        })();
        if let Err(rollback_error) = rollback {
            return Err(ClientUpdateError::Transaction(
                "update_rollback_failed",
                format!(
                    "{error}; rollback failed: {rollback_error}. Exact recovery files are preserved."
                ),
            ));
        }
        for (_, stage, _, _) in &staged {
            if stage.exists() {
                let _ = std::fs::remove_file(stage);
            }
        }
        return Err(error);
    }

    // Committed. Stage leftovers, if any, are removed; a cleanup failure
    // never misreports a committed, validated update as failed.
    for (_, stage, _, _) in &staged {
        if stage.exists() {
            let _ = std::fs::remove_file(stage);
        }
    }
    result
}

fn regular(path: &Path) -> Result<std::fs::Metadata, std::io::Error> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other(format!(
            "{} is not a regular contained file",
            path.display()
        )));
    }
    Ok(metadata)
}

fn optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, std::io::Error> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn atomic_write(path: &Path, bytes: Option<&[u8]>) -> Result<(), std::io::Error> {
    if let Some(bytes) = bytes {
        let temporary =
            path.with_file_name(format!(".aurora-update-state-{}", uuid::Uuid::new_v4()));
        use std::io::Write;
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            std::fs::rename(&temporary, path)
        })();
        if result.is_err() && temporary.exists() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    } else {
        if optional_bytes(path)?.is_some() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

/// The one-shot read-only Client update check for one instance. Discovery
/// failures are honest unavailability, never instance invalidity.
pub async fn check_client_update(
    managed: &ManagedPaths,
    endpoints: &lifecycle::InstanceEndpoints,
    id: &InstanceId,
) -> UpdateAvailability {
    check_client_update_with_url(managed, endpoints, id, PRODUCTION_MANIFEST_URL).await
}

/// The same check against an explicit manifest URL; the production wrapper
/// passes the compiled published endpoint. Crate-visible so deterministic
/// tests exercise offline behavior against a dead loopback endpoint.
pub(crate) async fn check_client_update_with_url(
    managed: &ManagedPaths,
    endpoints: &lifecycle::InstanceEndpoints,
    id: &InstanceId,
    url: &str,
) -> UpdateAvailability {
    match fetch_published_manifest(url, endpoints.download_options()).await {
        Ok(remote) => match preview_update(managed, endpoints, id, &remote) {
            Ok(preview) => match preview.outcome {
                "updateAvailable" => {
                    let candidate = preview.candidate.expect("outcome carries a candidate");
                    UpdateAvailability::UpdateAvailable {
                        current: preview.installed_version,
                        candidate: candidate.version,
                        notes: candidate.notes,
                    }
                }
                _ => UpdateAvailability::UpToDate,
            },
            Err(ClientUpdateError::Inapplicable(reason)) => {
                UpdateAvailability::NotApplicable { reason }
            }
            Err(error) => UpdateAvailability::unavailable(error.message()),
        },
        Err(error) => UpdateAvailability::unavailable(format!(
            "Could not check for updates: {}",
            error.message()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::lifecycle::tests::SyntheticWorld;
    use crate::test_support::{TestResponse, TestServer};
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::sync::Mutex as StdMutex;

    /// One deterministic update laboratory: a fully created, ready Aurora
    /// instance from the synthetic world, plus a loopback server for the
    /// published update manifest and its artifacts.
    struct Lab {
        world: SyntheticWorld,
        bodies: Arc<StdMutex<BTreeMap<String, Vec<u8>>>>,
        broken: Arc<StdMutex<std::collections::HashSet<String>>>,
        server: TestServer,
        instance: InstanceId,
    }

    impl Lab {
        /// A ready instance with real inspectable mod jars (the api world),
        /// so committed updates satisfy the mod-ownership checks exactly as
        /// production instances do.
        async fn new(name: &str) -> Self {
            let world = SyntheticWorld::with_api(name, true);
            let record = world
                .create("Update Lab")
                .await
                .expect("the synthetic instance must create completely");
            let instance = record.id().clone();
            let bodies: Arc<StdMutex<BTreeMap<String, Vec<u8>>>> =
                Arc::new(StdMutex::new(BTreeMap::new()));
            let broken: Arc<StdMutex<std::collections::HashSet<String>>> =
                Arc::new(StdMutex::new(std::collections::HashSet::new()));
            let handler_bodies = Arc::clone(&bodies);
            let handler_broken = Arc::clone(&broken);
            let server = TestServer::spawn(Arc::new(move |request| {
                if handler_broken.lock().unwrap().contains(&request.path) {
                    return TestResponse::status(404);
                }
                handler_bodies
                    .lock()
                    .unwrap()
                    .get(&request.path)
                    .map(|body| TestResponse::ok(body))
                    .unwrap_or(TestResponse::status(404))
            }));
            Self {
                world,
                bodies,
                broken,
                server,
                instance,
            }
        }

        fn break_path(&self, path: &str) {
            self.broken.lock().unwrap().insert(path.to_owned());
        }

        fn unbreak_path(&self, path: &str) {
            self.broken.lock().unwrap().remove(path);
        }

        fn endpoints(&self) -> lifecycle::InstanceEndpoints {
            self.world.endpoints()
        }

        fn manifest_url(&self) -> String {
            format!("{}/aurora-releases.json", self.server.base_url())
        }

        fn options(&self) -> DownloadOptions {
            self.endpoints().download_options().clone()
        }

        async fn remote(&self) -> ReleaseManifest {
            fetch_published_manifest(&self.manifest_url(), &self.options())
                .await
                .expect("the published manifest must fetch")
        }

        fn publish(&self, releases: Vec<serde_json::Value>) {
            let document = serde_json::json!({ "schemaVersion": 1, "releases": releases });
            self.bodies.lock().unwrap().insert(
                "/aurora-releases.json".to_owned(),
                document.to_string().into_bytes(),
            );
        }

        fn artifact(&self, path: &str, bytes: &[u8]) -> (String, String, u64) {
            use sha2::Digest as _;
            self.bodies
                .lock()
                .unwrap()
                .insert(path.to_owned(), bytes.to_vec());
            let digest =
                crate::integrity::ArtifactDigest::from_sha256(sha2::Sha256::digest(bytes).into())
                    .as_hex();
            (
                format!("{}{}", self.server.base_url(), path),
                digest,
                bytes.len() as u64,
            )
        }

        /// A synthetic newer release entry for the created environment.
        fn release_entry(
            &self,
            version: &str,
            channel: &str,
            minecraft: &str,
            loader: &str,
            java: u32,
            path: &str,
            bytes: &[u8],
            notes: Option<&str>,
        ) -> serde_json::Value {
            let (url, sha256, size) = self.artifact(path, bytes);
            let mut entry = serde_json::json!({
                "auroraVersion": version,
                "channel": channel,
                "minecraftVersion": minecraft,
                "fabricLoaderVersion": loader,
                "java": { "majorVersion": java },
                "artifact": { "url": url, "sha256": sha256, "sizeBytes": size }
            });
            if let Some(notes) = notes {
                entry["notes"] = serde_json::json!(notes);
            }
            entry
        }

        /// The fabric-api pin of the installed release, carried across to a
        /// newer release exactly as production pins do.
        fn carried_fabric_api(&self) -> serde_json::Value {
            let state = aurora::load_installed_state(self.world.managed(), &self.instance)
                .unwrap()
                .expect("the api world installs Fabric API");
            let api = state.fabric_api().expect("fabric api state");
            serde_json::json!({
                "version": api.version(),
                "artifact": {
                    // The world server still serves the exact pinned bytes;
                    // acquisition hits the verified cache by digest.
                    "url": format!("{}/aurora/fabric-api.jar", self.world.server().base_url()),
                    "sha256": api.artifact().sha256(),
                    "sizeBytes": api.artifact().size_bytes()
                }
            })
        }

        fn newer_release(&self, bytes: &[u8]) -> serde_json::Value {
            self.release_entry(
                "0.4.0",
                "stable",
                "26.2",
                "0.19.5",
                25,
                "/aurora/aurora-0.4.0-dev.jar",
                bytes,
                Some("Diagnostic release notes.\nSecond line."),
            )
        }

        fn root(&self) -> PathBuf {
            self.world
                .managed()
                .instance_paths(&self.instance)
                .root()
                .to_path_buf()
        }

        fn mods(&self) -> PathBuf {
            self.world
                .managed()
                .instance_paths(&self.instance)
                .mods()
                .to_path_buf()
        }

        fn state_bytes(&self) -> Vec<u8> {
            std::fs::read(self.root().join(aurora::AURORA_INSTALLED_FILE_NAME)).unwrap()
        }

        fn registry_bytes(&self) -> Vec<u8> {
            std::fs::read(self.world.managed().instance_registry_file()).unwrap()
        }

        fn write_user_content(&self) -> Vec<(PathBuf, Vec<u8>)> {
            let files = [
                ("mods/user-mod.jar", b"user mod bytes".to_vec()),
                ("config/options.txt", b"version:1\n".to_vec()),
                ("saves/world/level.dat", b"level bytes".to_vec()),
                ("resourcepacks/rp.zip", b"resource pack bytes".to_vec()),
                ("shaderpacks/sp.zip", b"shader pack bytes".to_vec()),
            ];
            files
                .iter()
                .map(|(relative, bytes)| {
                    let path = self.root().join(relative);
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(&path, bytes).unwrap();
                    (path, bytes.clone())
                })
                .collect()
        }
    }

    /// A real inspectable mod jar for the newer release, so committed
    /// updates satisfy the same mod-ownership checks as production.
    fn mod_jar(id: &str, version: &str) -> Vec<u8> {
        use std::io::Write;
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(
                format!(r#"{{"schemaVersion":1,"id":"{id}","version":"{version}"}}"#).as_bytes(),
            )
            .unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn newer_artifact() -> Vec<u8> {
        mod_jar("aurora", "0.4.0")
    }

    #[tokio::test]
    async fn current_client_reports_no_update_and_discovery_is_read_only() {
        let lab = Lab::new("current").await;
        // The manifest republishes exactly the installed release.
        let installed = lab.state_bytes();
        let registry = lab.registry_bytes();
        let (url, sha256, size) = lab.artifact(
            "/aurora/aurora-0.3.0-dev.jar",
            b"aurora development artifact bytes",
        );
        let installed_state = aurora::load_installed_state(lab.world.managed(), &lab.instance)
            .unwrap()
            .unwrap();
        let mut installed_entry = serde_json::json!({
            "auroraVersion": "0.3.0",
            "channel": "stable",
            "minecraftVersion": "26.2",
            "fabricLoaderVersion": "0.19.5",
            "java": { "majorVersion": 25 },
            "artifact": { "url": url, "sha256": sha256, "sizeBytes": size }
        });
        let api = installed_state.fabric_api().unwrap();
        installed_entry["fabricApi"] = serde_json::json!({
            "version": api.version(),
            "artifact": {
                "url": format!("{}/aurora/fabric-api.jar", lab.world.server().base_url()),
                "sha256": api.artifact().sha256(),
                "sizeBytes": api.artifact().size_bytes()
            }
        });
        lab.publish(vec![installed_entry]);
        let remote = lab.remote().await;
        let preview = preview_update(
            lab.world.managed(),
            &lab.endpoints(),
            &lab.instance,
            &remote,
        )
        .expect("preview runs");
        assert_eq!(preview.outcome, "upToDate");
        assert!(preview.candidate.is_none());
        // Read-only: neither the installed state nor the registry moved.
        assert_eq!(lab.state_bytes(), installed);
        assert_eq!(lab.registry_bytes(), registry);
    }

    #[tokio::test]
    async fn older_stable_client_updates_through_the_full_transaction() {
        let lab = Lab::new("happy-update").await;
        let user_content = lab.write_user_content();
        lab.publish(vec![lab.newer_release(&newer_artifact())]);
        let remote = lab.remote().await;
        let endpoints = lab.endpoints();
        let preview = preview_update(lab.world.managed(), &endpoints, &lab.instance, &remote)
            .expect("preview runs");
        assert_eq!(preview.outcome, "updateAvailable");
        let candidate = preview.candidate.as_ref().unwrap();
        assert_eq!(candidate.version, "0.4.0");
        assert_eq!(
            candidate.notes.as_deref(),
            Some("Diagnostic release notes.\nSecond line.")
        );
        assert!(
            preview.blockers.is_empty(),
            "unexpected blockers: {:#?}",
            preview.blockers
        );

        let mut phases = Vec::new();
        let record = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &preview.fingerprint,
            &remote,
            &lab.options(),
            &mut |phase| phases.push(phase),
            UpdateFaults::default(),
        )
        .await
        .expect("the update transaction commits");

        // The pin moved to the new exact identity only after commit.
        assert_eq!(record.installed().aurora.as_ref().unwrap().version, "0.4.0");
        let new_jar = lab.mods().join("aurora-0.4.0.jar");
        assert!(new_jar.is_file(), "the new artifact is installed");
        assert!(!lab.mods().join("aurora-0.3.0.jar").exists());
        let state = aurora::load_installed_state(lab.world.managed(), &lab.instance)
            .unwrap()
            .unwrap();
        assert_eq!(state.aurora_version(), "0.4.0");
        // The persisted release metadata makes the remote-only version
        // resolvable for validation and launch.
        let persisted = aurora::load_instance_release(lab.world.managed(), &lab.instance)
            .unwrap()
            .expect("the release sidecar is persisted");
        assert_eq!(persisted.aurora_version(), "0.4.0");
        let registry =
            InstanceRegistry::load(&lab.world.managed().instance_registry_file()).unwrap();
        let validation =
            lifecycle::validate_instance(lab.world.managed(), &registry, &lab.instance).unwrap();
        assert_eq!(validation.status, lifecycle::InstanceStatus::Ready);
        let resolved =
            lifecycle::resolve_optional_aurora(lab.world.managed(), &endpoints, &record).unwrap();
        assert_eq!(resolved.as_ref().unwrap().aurora_version(), "0.4.0");
        assert!(
            instance_mods::verified_required_mods(lab.world.managed(), &lab.instance).is_ok(),
            "provider reconciliation resolves the remotely updated release"
        );
        // Real transaction phases were reported, in order.
        assert_eq!(
            phases,
            vec![
                crate::updates::ClientPhase::Acquiring,
                crate::updates::ClientPhase::Staging,
                crate::updates::ClientPhase::Activating,
                crate::updates::ClientPhase::Validating,
                crate::updates::ClientPhase::Committing,
            ]
        );
        // Unrelated user content is untouched, byte for byte.
        for (path, bytes) in user_content {
            assert_eq!(std::fs::read(&path).unwrap(), bytes, "{}", path.display());
        }
        // A second check now sees the new version as current.
        let remote_again = lab.remote().await;
        lab.publish(vec![lab.newer_release(&newer_artifact())]);
        let _ = remote_again;
        let preview_after = preview_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &lab.remote().await,
        )
        .expect("preview runs");
        assert_eq!(preview_after.outcome, "upToDate");
    }

    #[tokio::test]
    async fn published_history_is_ordered_without_channel_filtering() {
        let lab = Lab::new("single-stream").await;
        lab.publish(vec![
            lab.release_entry(
                "0.4.0",
                "stable",
                "26.2",
                "0.19.5",
                25,
                "/aurora/older.jar",
                b"older",
                None,
            ),
            lab.release_entry(
                "0.5.0-diag.1",
                "nightly",
                "26.2",
                "0.19.5",
                25,
                "/aurora/newer.jar",
                b"newer",
                None,
            ),
            lab.release_entry(
                "0.4.1",
                "beta",
                "26.2",
                "0.19.5",
                25,
                "/aurora/middle.jar",
                b"middle",
                None,
            ),
        ]);
        let preview = preview_update(
            lab.world.managed(),
            &lab.endpoints(),
            &lab.instance,
            &lab.remote().await,
        )
        .unwrap();
        assert_eq!(preview.candidate.unwrap().version, "0.5.0-diag.1");
    }

    #[tokio::test]
    async fn unpublished_development_entries_never_enter_discovery() {
        let lab = Lab::new("unpublished").await;
        lab.publish(vec![]);
        // The operational catalog includes development artifacts. None is
        // an update unless it appears in the pinned published document.
        assert!(
            !crate::distribution::development_manifest()
                .unwrap()
                .releases()
                .is_empty()
        );
        let preview = preview_update(
            lab.world.managed(),
            &lab.endpoints(),
            &lab.instance,
            &lab.remote().await,
        )
        .unwrap();
        assert_eq!(preview.outcome, "upToDate");
        assert!(preview.candidate.is_none());
    }

    #[tokio::test]
    async fn malformed_release_versions_never_become_candidates() {
        let lab = Lab::new("malformed").await;
        lab.publish(vec![
            lab.release_entry(
                "latest",
                "stable",
                "26.2",
                "0.19.5",
                25,
                "/aurora/latest.jar",
                b"latest bytes",
                None,
            ),
            lab.newer_release(&newer_artifact()),
        ]);
        let preview = preview_update(
            lab.world.managed(),
            &lab.endpoints(),
            &lab.instance,
            &lab.remote().await,
        )
        .expect("preview runs");
        assert_eq!(
            preview.candidate.as_ref().unwrap().version,
            "0.4.0",
            "the malformed version is skipped, not guessed"
        );
    }

    #[tokio::test]
    async fn build_metadata_alone_is_not_a_newer_release() {
        let lab = Lab::new("build-precedence").await;
        lab.publish(vec![lab.release_entry(
            "0.3.0+public-build",
            "nightly",
            "26.2",
            "0.19.5",
            25,
            "/aurora/build.jar",
            b"build bytes",
            None,
        )]);
        let preview = preview_update(
            lab.world.managed(),
            &lab.endpoints(),
            &lab.instance,
            &lab.remote().await,
        )
        .unwrap();
        assert_eq!(preview.outcome, "upToDate");
        assert!(preview.candidate.is_none());
    }

    #[tokio::test]
    async fn channel_free_published_entry_can_be_installed_and_validated() {
        let lab = Lab::new("channel-free").await;
        let mut entry = lab.newer_release(&newer_artifact());
        entry.as_object_mut().unwrap().remove("channel");
        lab.publish(vec![entry]);
        let endpoints = lab.endpoints();
        let remote = lab.remote().await;
        let preview =
            preview_update(lab.world.managed(), &endpoints, &lab.instance, &remote).unwrap();
        let record = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &preview.fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults::default(),
        )
        .await
        .unwrap();
        assert_eq!(record.installed().aurora.as_ref().unwrap().version, "0.4.0");
        let registry =
            InstanceRegistry::load(&lab.world.managed().instance_registry_file()).unwrap();
        assert_eq!(
            lifecycle::validate_instance(lab.world.managed(), &registry, &lab.instance)
                .unwrap()
                .status,
            lifecycle::InstanceStatus::Ready
        );
    }

    #[tokio::test]
    async fn installed_newer_than_the_manifest_is_reported_truthfully() {
        let lab = Lab::new("installed-newer").await;
        // The manifest holds only an older release for this environment.
        lab.publish(vec![lab.release_entry(
            "0.2.0",
            "stable",
            "26.2",
            "0.19.5",
            25,
            "/aurora/aurora-0.2.0-dev.jar",
            b"old bytes",
            None,
        )]);
        let preview = preview_update(
            lab.world.managed(),
            &lab.endpoints(),
            &lab.instance,
            &lab.remote().await,
        )
        .expect("preview runs");
        assert_eq!(preview.outcome, "installedNewer");
        assert!(preview.candidate.is_none());
        assert!(!preview.warnings.is_empty());
    }

    #[tokio::test]
    async fn incompatible_minecraft_and_loader_releases_are_not_candidates() {
        let lab = Lab::new("incompatible").await;
        lab.publish(vec![
            lab.release_entry(
                "0.4.0",
                "stable",
                "1.21.11",
                "0.19.5",
                25,
                "/aurora/wrong-mc.jar",
                b"bytes",
                None,
            ),
            lab.release_entry(
                "0.4.1",
                "stable",
                "26.2",
                "0.19.6",
                25,
                "/aurora/wrong-loader.jar",
                b"bytes",
                None,
            ),
        ]);
        let preview = preview_update(
            lab.world.managed(),
            &lab.endpoints(),
            &lab.instance,
            &lab.remote().await,
        )
        .expect("preview runs");
        assert_eq!(
            preview.outcome, "upToDate",
            "environment-incompatible releases are never candidates"
        );
    }

    #[tokio::test]
    async fn java_mismatch_is_not_offered_and_cannot_apply() {
        let lab = Lab::new("java-mismatch").await;
        lab.publish(vec![lab.release_entry(
            "0.4.0",
            "stable",
            "26.2",
            "0.19.5",
            21,
            "/aurora/aurora-0.4.0-dev.jar",
            &newer_artifact(),
            None,
        )]);
        let endpoints = lab.endpoints();
        let remote = lab.remote().await;
        let preview = preview_update(lab.world.managed(), &endpoints, &lab.instance, &remote)
            .expect("preview runs");
        assert_eq!(preview.outcome, "upToDate");
        let state_before = lab.state_bytes();
        let error = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &preview.fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults::default(),
        )
        .await
        .expect_err("the Java assertion must fail closed");
        assert!(matches!(error, ClientUpdateError::Stale(_)));
        assert_eq!(lab.state_bytes(), state_before, "nothing changed");
    }

    #[tokio::test]
    async fn incorrect_digest_fails_acquisition_and_preserves_the_old_client() {
        let lab = Lab::new("bad-digest").await;
        let (url, _sha, size) = lab.artifact("/aurora/aurora-0.4.0-dev.jar", &newer_artifact());
        lab.publish(vec![serde_json::json!({
            "auroraVersion": "0.4.0",
            "channel": "stable",
            "minecraftVersion": "26.2",
            "fabricLoaderVersion": "0.19.5",
            "java": { "majorVersion": 25 },
            // A digest that matches nothing: the declared bytes never verify.
            "artifact": {
                "url": url,
                "sha256": "f".repeat(64),
                "sizeBytes": size
            }
        })]);
        let endpoints = lab.endpoints();
        let remote = lab.remote().await;
        let preview = preview_update(lab.world.managed(), &endpoints, &lab.instance, &remote)
            .expect("preview runs");
        let state_before = lab.state_bytes();
        let registry_before = lab.registry_bytes();
        let error = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &preview.fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults::default(),
        )
        .await
        .expect_err("a wrong digest must never activate");
        assert!(matches!(
            error,
            ClientUpdateError::Transaction("update_download_failed", _)
        ));
        assert!(lab.mods().join("aurora-0.3.0.jar").exists());
        assert!(!lab.mods().join("aurora-0.4.0.jar").exists());
        assert_eq!(lab.state_bytes(), state_before);
        assert_eq!(lab.registry_bytes(), registry_before);
    }

    async fn approved_newer(lab: &Lab) -> (lifecycle::InstanceEndpoints, ReleaseManifest, String) {
        lab.publish(vec![lab.newer_release(&newer_artifact())]);
        let endpoints = lab.endpoints();
        let remote = lab.remote().await;
        let preview = preview_update(lab.world.managed(), &endpoints, &lab.instance, &remote)
            .expect("preview runs");
        (endpoints, remote, preview.fingerprint)
    }

    #[tokio::test]
    async fn staging_failure_rolls_back_before_any_mutation() {
        let lab = Lab::new("fault-stage").await;
        let (endpoints, remote, fingerprint) = approved_newer(&lab).await;
        let state_before = lab.state_bytes();
        let error = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults {
                fail_stage: true,
                ..Default::default()
            },
        )
        .await
        .expect_err("injected staging failure");
        assert!(matches!(
            error,
            ClientUpdateError::Transaction("update_transaction_failed", _)
        ));
        assert!(lab.mods().join("aurora-0.3.0.jar").exists());
        assert!(!lab.mods().join("aurora-0.4.0.jar").exists());
        assert_eq!(lab.state_bytes(), state_before);
    }

    #[tokio::test]
    async fn activation_failure_preserves_the_old_client() {
        let lab = Lab::new("fault-activation").await;
        let (endpoints, remote, fingerprint) = approved_newer(&lab).await;
        let state_before = lab.state_bytes();
        let registry_before = lab.registry_bytes();
        let error = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults {
                fail_activation: true,
                ..Default::default()
            },
        )
        .await
        .expect_err("injected activation failure");
        assert!(matches!(
            error,
            ClientUpdateError::Transaction("update_transaction_failed", _)
        ));
        assert!(lab.mods().join("aurora-0.3.0.jar").exists());
        assert!(!lab.mods().join("aurora-0.4.0.jar").exists());
        assert_eq!(lab.state_bytes(), state_before);
        assert_eq!(lab.registry_bytes(), registry_before);
    }

    #[tokio::test]
    async fn validation_failure_restores_the_old_client() {
        let lab = Lab::new("fault-validation").await;
        let (endpoints, remote, fingerprint) = approved_newer(&lab).await;
        let state_before = lab.state_bytes();
        let registry_before = lab.registry_bytes();
        let error = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults {
                fail_validation: true,
                ..Default::default()
            },
        )
        .await
        .expect_err("injected validation failure");
        assert!(matches!(
            error,
            ClientUpdateError::Transaction("update_transaction_failed", _)
        ));
        assert!(lab.mods().join("aurora-0.3.0.jar").exists());
        assert!(!lab.mods().join("aurora-0.4.0.jar").exists());
        assert_eq!(lab.state_bytes(), state_before);
        assert_eq!(lab.registry_bytes(), registry_before);
    }

    #[tokio::test]
    async fn persistence_failure_moves_no_pin() {
        let lab = Lab::new("fault-persistence").await;
        let (endpoints, remote, fingerprint) = approved_newer(&lab).await;
        let registry_before = lab.registry_bytes();
        let error = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults {
                fail_persistence: true,
                ..Default::default()
            },
        )
        .await
        .expect_err("injected persistence failure");
        assert!(matches!(
            error,
            ClientUpdateError::Transaction("update_transaction_failed", _)
        ));
        assert!(
            !lab.mods().join("aurora-0.4.0.jar").exists(),
            "the rolled-back activation leaves no new jar"
        );
        assert!(lab.mods().join("aurora-0.3.0.jar").exists());
        assert_eq!(lab.registry_bytes(), registry_before, "the pin never moved");
        let registry =
            InstanceRegistry::load(&lab.world.managed().instance_registry_file()).unwrap();
        assert_eq!(
            registry
                .find(&lab.instance)
                .unwrap()
                .installed()
                .aurora
                .as_ref()
                .unwrap()
                .version,
            "0.3.0"
        );
    }

    #[tokio::test]
    async fn stale_plans_are_refused_when_the_world_changed() {
        let lab = Lab::new("stale").await;
        let (endpoints, remote, fingerprint) = approved_newer(&lab).await;
        // The user adds a mod after approving: the fingerprint must refuse.
        std::fs::write(lab.mods().join("late-user-mod.jar"), b"late bytes").unwrap();
        let error = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults::default(),
        )
        .await
        .expect_err("a changed world must refuse the stale plan");
        assert!(matches!(error, ClientUpdateError::Stale(_)));
        assert!(lab.mods().join("aurora-0.3.0.jar").exists());
    }

    #[tokio::test]
    async fn running_instances_cannot_update() {
        let lab = Lab::new("running").await;
        let (endpoints, remote, _fingerprint) = approved_newer(&lab).await;
        let _guard = crate::launch::process::force_running(lab.instance.as_str());
        let preview = preview_update(lab.world.managed(), &endpoints, &lab.instance, &remote)
            .expect("preview runs read-only");
        assert!(
            preview
                .blockers
                .iter()
                .any(|blocker| blocker.contains("Quit Minecraft")),
            "the running game is a named blocker: {preview:?}"
        );
        let fingerprint = preview.fingerprint.clone();
        let error = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults::default(),
        )
        .await
        .expect_err("a running instance must never mutate");
        assert!(matches!(
            error,
            ClientUpdateError::Blocked(_) | ClientUpdateError::InstanceRunning
        ));
        assert!(lab.mods().join("aurora-0.3.0.jar").exists());
    }

    #[tokio::test]
    async fn retry_succeeds_after_a_recoverable_acquisition_failure() {
        let lab = Lab::new("retry").await;
        lab.publish(vec![lab.newer_release(&newer_artifact())]);
        let endpoints = lab.endpoints();
        let remote = lab.remote().await;
        let preview = preview_update(lab.world.managed(), &endpoints, &lab.instance, &remote)
            .expect("preview runs");
        // Break acquisition, fail once, then heal and retry.
        lab.break_path("/aurora/aurora-0.4.0-dev.jar");
        let first = apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &preview.fingerprint,
            &remote,
            &lab.options(),
            &mut |_| {},
            UpdateFaults::default(),
        )
        .await;
        assert!(first.is_err(), "the broken artifact server must fail");
        assert!(lab.mods().join("aurora-0.3.0.jar").exists());
        // A fresh preview after the failure still approves the same release.
        lab.unbreak_path("/aurora/aurora-0.4.0-dev.jar");
        let fresh = preview_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &lab.remote().await,
        )
        .expect("preview runs");
        apply_update(
            lab.world.managed(),
            &endpoints,
            &lab.instance,
            &fresh.fingerprint,
            &lab.remote().await,
            &lab.options(),
            &mut |_| {},
            UpdateFaults::default(),
        )
        .await
        .expect("the retry commits after the recoverable failure");
        assert!(lab.mods().join("aurora-0.4.0.jar").exists());
    }

    #[tokio::test]
    async fn malformed_manifests_and_untrusted_urls_are_rejected() {
        let lab = Lab::new("bad-manifest").await;
        let options = lab.options();
        // Malformed JSON.
        lab.bodies
            .lock()
            .unwrap()
            .insert("/aurora-releases.json".to_owned(), b"{ not json".to_vec());
        let error = fetch_published_manifest(&lab.manifest_url(), &options)
            .await
            .unwrap_err();
        assert!(matches!(error, ClientUpdateError::ManifestInvalid(_)));
        // Untrusted artifact URL (cleartext, non-loopback) inside an
        // otherwise well-formed manifest.
        lab.bodies.lock().unwrap().insert(
            "/aurora-releases.json".to_owned(),
            serde_json::json!({
                "schemaVersion": 1,
                "releases": [{
                    "auroraVersion": "0.4.0",
                    "channel": "stable",
                    "minecraftVersion": "26.2",
                    "fabricLoaderVersion": "0.19.5",
                    "java": { "majorVersion": 25 },
                    "artifact": {
                        "url": "http://192.168.0.2/aurora.jar",
                        "sha256": "f".repeat(64),
                        "sizeBytes": 12
                    }
                }]
            })
            .to_string()
            .into_bytes(),
        );
        let error = fetch_published_manifest(&lab.manifest_url(), &options)
            .await
            .unwrap_err();
        assert!(
            matches!(error, ClientUpdateError::ManifestInvalid(_)),
            "untrusted URLs are rejected at the manifest boundary"
        );
        // A missing/short digest is equally unusable.
        lab.bodies.lock().unwrap().insert(
            "/aurora-releases.json".to_owned(),
            serde_json::json!({
                "schemaVersion": 1,
                "releases": [{
                    "auroraVersion": "0.4.0",
                    "channel": "stable",
                    "minecraftVersion": "26.2",
                    "fabricLoaderVersion": "0.19.5",
                    "java": { "majorVersion": 25 },
                    "artifact": { "url": lab.server.base_url().to_owned() + "/a.jar", "sha256": "deadbeef" }
                }]
            })
            .to_string()
            .into_bytes(),
        );
        assert!(
            fetch_published_manifest(&lab.manifest_url(), &options)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn unreachable_update_services_are_nonfatal() {
        let lab = Lab::new("offline").await;
        // Bind and drop: the endpoint refuses connections afterwards.
        let dead_url = lab.manifest_url();
        let endpoints = lab.endpoints();
        let managed = lab.world.managed().clone();
        let instance = lab.instance.clone();
        drop(lab.server);
        drop(lab.bodies);
        let availability =
            check_client_update_with_url(&managed, &endpoints, &instance, &dead_url).await;
        assert!(
            matches!(availability, UpdateAvailability::Unavailable { .. }),
            "offline discovery is honest unavailability, not an error"
        );
        // The instance itself remains valid.
        let registry =
            InstanceRegistry::load(&lab.world.managed().instance_registry_file()).unwrap();
        let validation =
            lifecycle::validate_instance(lab.world.managed(), &registry, &lab.instance).unwrap();
        assert_eq!(validation.status, lifecycle::InstanceStatus::Ready);
    }
}
