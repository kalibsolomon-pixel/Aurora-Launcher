//! Exact Modrinth pack planning and installation. Provider identity is
//! recovered before installation; unresolved files stay explicitly external.
//! A fresh instance remains Installing until all owned bytes and their state
//! validate, so an interrupted operation never becomes launchable.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::cache::{ArtifactCache, VerifiedArtifact};
use crate::downloads::Sha512ArtifactSource;
use crate::instance_content::{ProviderArtifactSource, ProviderInstallPlan};
use crate::instances::lifecycle::{CreateInstanceRequest, InstanceEndpoints, InstanceProgress};
use crate::instances::platform::PlatformPin;
use crate::instances::settings::{
    DEFAULT_MEMORY_MIB, InstanceConfiguration, LoaderConfiguration, LoaderPolicy,
};
use crate::instances::{InstanceId, InstanceRecord};
use crate::integrity::{ArtifactDigest, Sha1Digest, verify_file, verify_file_sha1};
use crate::mrpack::{PackFile, PackLoaderKind, PackPlan};
use crate::pack_state::{InstalledPack, OwnedComponent, OwnedOverride, PackIdentity};
use crate::paths::ManagedPaths;

#[derive(Debug)]
pub struct PackInstallError {
    pub code: &'static str,
    pub message: String,
    /// The new instance this install created and kept (still Installing,
    /// resumable with [`resume`]). `None` when the failure happened before
    /// any instance existed, so nothing was created.
    pub instance_id: Option<InstanceId>,
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
        instance_id: None,
    }
}

impl PackInstallError {
    /// Attributes a failure after instance creation to the kept Installing
    /// instance and says so honestly. Codes stay unchanged. A rollback
    /// failure keeps its own "needs inspection" wording, and resume refusals
    /// already state that nothing was changed.
    fn kept(mut self, record: &InstanceRecord) -> Self {
        self.instance_id.get_or_insert_with(|| record.id().clone());
        let name = record.display_name();
        match self.code {
            "pack_rollback_failed" | "installation_already_in_progress" => {}
            code if code.starts_with("pack_resume_") => {}
            "installation_cancelled" => {
                self.message = format!(
                    "Installation cancelled. The instance '{name}' was kept and can be resumed from Downloads or Instances."
                );
            }
            _ => {
                self.message = format!(
                    "Installation failed: {}. The instance '{name}' was kept and can be retried from Downloads or Instances.",
                    self.message.trim_end_matches('.')
                );
            }
        }
        self
    }

    fn for_instance(mut self, instance: &InstanceId) -> Self {
        self.instance_id.get_or_insert_with(|| instance.clone());
        self
    }
}

/// The durable resume intent of a new pack instance, at the instance root.
/// Written (atomically, never over an existing file) before the Installing
/// record is persisted and before any instance-owned byte exists; removed
/// only after the instance is Ready. It pins exactly what the review
/// authorized: the provider version, its archive SHA-512, the optional
/// selection, and the reviewed preview fingerprint (which commits to the
/// archive digest, every recognized component identity, every unresolved
/// file, and every override path). Pack overrides can never name it
/// (`mrpack` launcher-owned root names).
pub(crate) const PENDING_FILE: &str = "pack-install-pending.json";
const PENDING_SCHEMA_VERSION: u32 = 1;
const MAX_PENDING_BYTES: u64 = 1024 * 1024;
const MAX_PENDING_OPTIONAL: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PendingInstall {
    schema_version: u32,
    instance_id: InstanceId,
    provider: String,
    project_id: String,
    version_id: String,
    archive_sha512: String,
    selected_optional: Vec<String>,
    fingerprint: String,
}

fn pending_damaged() -> PackInstallError {
    fail(
        "pack_resume_state_invalid",
        "This instance's modpack resume record is damaged, so Aurora won't resume the installation or overwrite the record.",
    )
}

impl PendingInstall {
    fn new(instance: &InstanceId, resolved: &Resolved, selected_optional: &[String]) -> Self {
        Self {
            schema_version: PENDING_SCHEMA_VERSION,
            instance_id: instance.clone(),
            provider: "modrinth".into(),
            project_id: resolved.provider_project_id.clone(),
            version_id: resolved.provider_version_id.clone(),
            archive_sha512: resolved.archive_sha512.clone(),
            selected_optional: selected_optional.to_vec(),
            fingerprint: resolved.preview.fingerprint.clone(),
        }
    }

    fn validate(&self) -> Result<(), PackInstallError> {
        let id = |value: &str| {
            !value.is_empty()
                && value.len() <= 64
                && value.bytes().all(|b| b.is_ascii_alphanumeric())
        };
        let hex = |value: &str, len: usize| {
            value.len() == len && value.bytes().all(|b| b.is_ascii_hexdigit())
        };
        let unique: HashSet<_> = self.selected_optional.iter().collect();
        if self.schema_version != PENDING_SCHEMA_VERSION
            || self.provider != "modrinth"
            || !id(&self.project_id)
            || !id(&self.version_id)
            || !hex(&self.archive_sha512, 128)
            || !hex(&self.fingerprint, 64)
            || self.selected_optional.len() > MAX_PENDING_OPTIONAL
            || unique.len() != self.selected_optional.len()
            || self
                .selected_optional
                .iter()
                .any(|path| crate::mrpack::destination_path(path).is_err())
        {
            return Err(pending_damaged());
        }
        Ok(())
    }

    /// The document path inside a validated instance root; a redirected or
    /// non-file entry is refused, never followed.
    fn path(managed: &ManagedPaths, instance: &InstanceId) -> Result<PathBuf, PackInstallError> {
        let root = crate::instance_content::validated_instance_root(managed, instance)
            .map_err(|_| fail("pack_invalid_path", "The instance folder is unsafe."))?;
        let path = root.join(PENDING_FILE);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() || reparse(&meta) => {
                Err(pending_damaged())
            }
            Ok(_) => Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
            Err(_) => Err(fail(
                "pack_io_error",
                "The modpack resume record could not be inspected.",
            )),
        }
    }

    /// Atomically writes a new document; any existing entry (valid,
    /// damaged or unknown) is never overwritten.
    fn save_new(&self, managed: &ManagedPaths) -> Result<(), PackInstallError> {
        self.validate()?;
        let path = Self::path(managed, &self.instance_id)?;
        if std::fs::symlink_metadata(&path).is_ok() {
            return Err(pending_damaged());
        }
        let mut json = serde_json::to_string_pretty(self).map_err(|_| pending_damaged())?;
        json.push('\n');
        let temporary = path.with_extension(format!("json.{}.tmp", uuid::Uuid::new_v4()));
        let written = std::fs::write(&temporary, json).and_then(|()| {
            if std::fs::symlink_metadata(&path).is_ok() {
                return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
            }
            std::fs::rename(&temporary, &path)
        });
        if written.is_err() {
            let _ = std::fs::remove_file(&temporary);
            return Err(fail(
                "pack_io_error",
                "The modpack resume record could not be saved, so no instance was created.",
            ));
        }
        Ok(())
    }

    /// Reads and validates the document. Missing is `None`; malformed,
    /// oversized, foreign, or unknown-schema documents are refused and left
    /// exactly as they are.
    pub(crate) fn load(
        managed: &ManagedPaths,
        instance: &InstanceId,
    ) -> Result<Option<Self>, PackInstallError> {
        use std::io::Read as _;
        let path = Self::path(managed, instance)?;
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => {
                return Err(fail(
                    "pack_io_error",
                    "The modpack resume record could not be read.",
                ));
            }
        };
        let mut bytes = Vec::new();
        file.take(MAX_PENDING_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                fail(
                    "pack_io_error",
                    "The modpack resume record could not be read.",
                )
            })?;
        if bytes.len() as u64 > MAX_PENDING_BYTES {
            return Err(pending_damaged());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| pending_damaged())?;
        match value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
        {
            Some(version) if version == u64::from(PENDING_SCHEMA_VERSION) => {}
            Some(_) => {
                return Err(fail(
                    "pack_resume_state_invalid",
                    "This instance's modpack resume record uses a format this version of Aurora doesn't know, so Aurora won't resume the installation or overwrite the record.",
                ));
            }
            None => return Err(pending_damaged()),
        }
        let document: Self = serde_json::from_value(value).map_err(|_| pending_damaged())?;
        document.validate()?;
        if &document.instance_id != instance {
            return Err(pending_damaged());
        }
        Ok(Some(document))
    }

    /// After the Ready commit: removes exactly this document when it is
    /// still the one written for this install. Failure leaves a harmless
    /// stale record (resume refuses any non-Installing instance) and never
    /// demotes the committed instance.
    fn remove_completed(&self, managed: &ManagedPaths) {
        if matches!(Self::load(managed, &self.instance_id), Ok(Some(stored)) if &stored == self)
            && let Ok(path) = Self::path(managed, &self.instance_id)
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The `modpack-progress` vocabulary. New-instance installs and merges into
/// an existing instance emit only these names; hosts label every one.
pub mod progress_phase {
    /// Re-resolving the exact version and its verified archive.
    pub const RESOLVING_PACK: &str = "resolvingPack";
    /// Acquiring pack files into the verified cache.
    pub const DOWNLOADING: &str = "downloading";
    /// Creating the new instance's configuration and record.
    pub const PREPARING_INSTANCE: &str = "preparingInstance";
    /// Minecraft and mod loader setup for a new instance (reported once).
    pub const INSTALLING_GAME: &str = "installingGame";
    /// Writing verified external files and reviewed overrides.
    pub const INSTALLING_OVERRIDES: &str = "installingOverrides";
    /// Activating Modrinth-recognized components.
    pub const INSTALLING_COMPONENTS: &str = "installingComponents";
    /// Saving and validating the installed pack state.
    pub const VALIDATING: &str = "validating";
    pub const COMPLETE: &str = "complete";
    pub const ALL: &[&str] = &[
        RESOLVING_PACK,
        DOWNLOADING,
        PREPARING_INSTANCE,
        INSTALLING_GAME,
        INSTALLING_OVERRIDES,
        INSTALLING_COMPONENTS,
        VALIDATING,
        COMPLETE,
    ];
}

/// One wording for the component bounds: a merge into an existing instance
/// keeps the ordinary per-operation bound; a new pack instance has its own.
pub(crate) fn component_limit_message(count: usize, merge: bool) -> String {
    if merge {
        let limit = crate::instance_content::MAX_PROVIDER_PLANS_PER_OPERATION;
        format!(
            "This pack would add {count} Modrinth-recognized components to this instance; Aurora currently installs at most {limit} components per operation."
        )
    } else {
        let limit = crate::instance_content::MAX_NEW_PACK_PROVIDER_PLANS;
        format!(
            "This pack has {count} Modrinth-recognized components; Aurora currently installs at most {limit} components in a new modpack instance."
        )
    }
}

/// Refuses, from Modrinth's version metadata alone, a pack Aurora cannot
/// install, so no archive is downloaded for it. After the verified download
/// the index is still cross-checked against this metadata, and an exact
/// Forge or NeoForge build is checked once the index names it.
pub(crate) fn version_metadata_refusal(
    artifact: &crate::modrinth::PackArtifact,
) -> Option<PackInstallError> {
    metadata_refusal(&artifact.loaders, &artifact.game_versions)
}

/// [`version_metadata_refusal`] over the two metadata lists themselves, so
/// version selection annotates every listed version with the same rule
/// before anything is chosen or downloaded.
pub(crate) fn metadata_refusal(
    loaders: &[String],
    game_versions: &[String],
) -> Option<PackInstallError> {
    let lists = |name: &str| loaders.iter().any(|value| value.eq_ignore_ascii_case(name));
    let (fabric, forge, neoforge) = (lists("fabric"), lists("forge"), lists("neoforge"));
    if !fabric && !forge && !neoforge {
        let message = if lists("quilt") {
            "This Modrinth version is a Quilt modpack. Aurora does not support Quilt."
        } else if loaders.is_empty() {
            "This Modrinth version names no mod loader, so Aurora can't install it as a modpack."
        } else {
            "This Modrinth version uses a mod loader Aurora doesn't install as a modpack. Aurora installs Fabric, Forge and NeoForge modpacks."
        };
        return Some(fail("pack_unsupported_loader", message));
    }
    if game_versions.is_empty() {
        return Some(fail(
            "pack_identity_conflict",
            "This Modrinth version names no Minecraft version, so Aurora can't confirm what it installs.",
        ));
    }
    // A version that also lists Fabric is decided by its index. Otherwise at
    // least one listed Forge-family loader must be installable for at least
    // one listed Minecraft version; the exact build is checked after parsing.
    let installs = |supported: fn(&str) -> bool| game_versions.iter().any(|game| supported(game));
    if fabric
        || (forge && installs(forge_installs_minecraft))
        || (neoforge && installs(neoforge_installs_minecraft))
    {
        return None;
    }
    let games = game_versions
        .iter()
        .take(8)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    let mut reasons = Vec::new();
    if forge {
        reasons.push(FORGE_MINECRAFT_RANGE);
    }
    if neoforge {
        reasons.push(NEOFORGE_MINECRAFT_RANGE);
    }
    let loader = match (forge, neoforge) {
        (true, true) => "Forge and NeoForge",
        (true, false) => "Forge",
        _ => "NeoForge",
    };
    Some(fail(
        "pack_unsupported_loader",
        format!(
            "This Modrinth version is a {loader} modpack for Minecraft {games}. {}",
            reasons.join(" ")
        ),
    ))
}

/// Refusal for a pack archive over `mrpack::MAX_PACK_BYTES`, shown both on
/// the version list (from Modrinth's published size) and by resolution.
pub(crate) const ARCHIVE_LIMIT_MESSAGE: &str =
    "The selected pack exceeds Aurora's 512 MiB archive limit.";

const FORGE_MINECRAFT_RANGE: &str =
    "Aurora installs Forge for Minecraft 1.15 and newer, and for 1.14.4 from Forge 28.1.16.";
const NEOFORGE_MINECRAFT_RANGE: &str = "Aurora installs NeoForge only for the year-numbered Minecraft releases (such as 26.2), not NeoForge for Minecraft 1.x.";

/// Whether Aurora's Forge support admits any build for this Minecraft
/// version. `ForgeVersionId::is_supported` stays the only rule: the build
/// half matters only within one Minecraft version (1.14.4), so the most
/// permissive build answers for the version as a whole.
fn forge_installs_minecraft(game: &str) -> bool {
    crate::forge::metadata::ForgeVersionId::new(&format!("{game}-{}", u64::MAX))
        .is_ok_and(|version| version.minecraft_version() == game && version.is_supported())
}

/// Whether an installable NeoForge generation addresses this Minecraft
/// version. NeoForge versions extend the version they address
/// (`<minecraft>.<build>`, with a `.0` patch for two-part versions), so the
/// zero build of that shape is judged by NeoForge's own attribution and
/// generation rule (`NeoForgeVersionEntry::parse`).
fn neoforge_installs_minecraft(game: &str) -> bool {
    let candidate = match game.split('.').count() {
        2 => format!("{game}.0.0"),
        3 => format!("{game}.0"),
        _ => return false,
    };
    crate::neoforge::metadata::NeoForgeVersionEntry::parse(&candidate)
        .is_ok_and(|entry| entry.installable_generation() && entry.minecraft_version() == game)
}

/// Refuses an exact Forge or NeoForge build the index names that Aurora's
/// installers would reject, before any instance exists. Fabric versions are
/// resolved exactly by the installer path, as before.
fn loader_build_refusal(pack: &PackPlan) -> Option<PackInstallError> {
    let minecraft = &pack.minecraft_version;
    match pack.loader.kind {
        PackLoaderKind::Fabric => None,
        PackLoaderKind::Forge => {
            let pin = format!("{minecraft}-{}", pack.loader.version);
            match crate::forge::metadata::ForgeVersionId::new(&pin) {
                // The combined id must be exactly this pack's Minecraft
                // version and build (no padding, no embedded second pin).
                Ok(version)
                    if version.as_str() == pin && version.minecraft_version() == minecraft =>
                {
                    version.require_supported().err().map(|error| {
                        fail(
                            "pack_unsupported_loader",
                            format!("This pack can't be installed: {error}."),
                        )
                    })
                }
                _ => Some(fail(
                    "pack_unsupported_loader",
                    "This pack names a Forge version Aurora can't read.",
                )),
            }
        }
        PackLoaderKind::NeoForge => {
            if crate::neoforge::metadata::NeoForgeVersionId::new(&pack.loader.version).is_err() {
                return Some(fail(
                    "pack_unsupported_loader",
                    "This pack names a NeoForge version Aurora can't read.",
                ));
            }
            let version = &pack.loader.version;
            match crate::neoforge::metadata::NeoForgeVersionEntry::parse(version) {
                Ok(entry) if entry.minecraft_version() != minecraft => Some(fail(
                    "pack_identity_conflict",
                    format!(
                        "This pack names NeoForge {version}, which is for Minecraft {}, not the pack's Minecraft {minecraft}.",
                        entry.minecraft_version()
                    ),
                )),
                Ok(entry) if entry.installable_generation() => None,
                _ => Some(fail(
                    "pack_unsupported_loader",
                    format!("This pack requires NeoForge {version}. {NEOFORGE_MINECRAFT_RANGE}"),
                )),
            }
        }
    }
}

/// The exact `LoaderPolicy::Pinned` configuration the pack's loader pin
/// requests through the ordinary validated installer path.
fn pinned_loader_configuration(pin: &PlatformPin) -> Result<LoaderConfiguration, PackInstallError> {
    let policy = |version: &str| LoaderPolicy::Pinned {
        version: version.to_owned(),
    };
    match pin {
        PlatformPin::Fabric { version } => Ok(LoaderConfiguration::Fabric {
            policy: policy(version),
        }),
        PlatformPin::Forge { version } => Ok(LoaderConfiguration::Forge {
            policy: policy(version),
        }),
        PlatformPin::NeoForge { version } => Ok(LoaderConfiguration::NeoForge {
            policy: policy(version),
        }),
        _ => Err(fail(
            "pack_unsupported_loader",
            "This pack's mod loader can't be installed as a modpack.",
        )),
    }
}

fn checkpoint() -> Result<(), PackInstallError> {
    crate::content_jobs::checkpoint().map_err(|_| {
        fail(
            "installation_cancelled",
            "Installation cancelled before activation.",
        )
    })
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
    /// The pack's one mod loader (`fabric`, `forge` or `neoForge`).
    pub loader: PackLoaderKind,
    /// The loader version exactly as the pack index declares it (for Forge,
    /// the bare build such as `47.4.10`).
    pub loader_version: String,
    pub archive_sha512: String,
    pub recognized: Vec<RecognizedPreview>,
    pub unresolved: Vec<String>,
    pub optional: Vec<String>,
    pub excluded: Vec<String>,
    pub overrides: Vec<String>,
    /// Pack overrides outside the game-content roots, never written.
    pub skipped_overrides: Vec<String>,
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

/// Pack files downloaded at once per pack; the process-wide transfer
/// permits still bound the total across concurrent installs.
pub(crate) const PACK_DOWNLOAD_CONCURRENCY: usize = 8;

pub(crate) struct Resolved {
    pub(crate) archive: VerifiedArtifact,
    pub(crate) archive_sha512: String,
    pub(crate) provider_project_id: String,
    pub(crate) provider_version_id: String,
    /// The Modrinth project title, used only as the new instance's display name.
    pub(crate) provider_title: String,
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
    Ok(resolve_bounded(
        managed,
        provider,
        project_id,
        version_id,
        selected_optional,
        Some(crate::instance_content::MAX_NEW_PACK_PROVIDER_PLANS),
        None,
    )
    .await?
    .preview)
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
    resolve_bounded(
        managed,
        provider,
        project_id,
        version_id,
        selected_optional,
        None,
        None,
    )
    .await
}

/// A destination's own admission rule, applied by [`resolve_gated`] where
/// the evidence first exists, so an incompatible destination costs neither
/// the archive download nor any component metadata or jar.
pub(crate) trait ResolveGate: Sync {
    /// From Modrinth's version metadata alone, before the archive download.
    fn before_download(
        &self,
        artifact: &crate::modrinth::PackArtifact,
    ) -> Result<(), PackInstallError>;
    /// From the verified, parsed index (exact Minecraft version and loader),
    /// before any component is looked up or downloaded.
    fn after_parse(&self, pack: &PackPlan) -> Result<(), PackInstallError>;
}

/// [`resolve`] for an existing destination (a merge target) whose
/// compatibility `gate` decides as early as its evidence allows.
pub(crate) async fn resolve_gated(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
    gate: &dyn ResolveGate,
) -> Result<Resolved, PackInstallError> {
    resolve_bounded(
        managed,
        provider,
        project_id,
        version_id,
        selected_optional,
        None,
        Some(gate),
    )
    .await
}

/// [`resolve`] that, for a new pack instance, refuses more recognized
/// components than a new pack instance installs, right after recognition and
/// before any component metadata or jar is fetched. Merges count only what
/// they would add, so they pass `None` and apply the bound in review.
async fn resolve_bounded(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
    component_limit: Option<usize>,
    gate: Option<&dyn ResolveGate>,
) -> Result<Resolved, PackInstallError> {
    let artifact = provider
        .pack_artifact(project_id, version_id)
        .await
        .map_err(|e| fail(e.code(), e.to_string()))?;
    if let Some(refusal) = version_metadata_refusal(&artifact) {
        return Err(refusal);
    }
    if let Some(gate) = gate {
        gate.before_download(&artifact)?;
    }
    if artifact
        .source
        .size_bytes()
        .is_some_and(|size| size > crate::mrpack::MAX_PACK_BYTES)
    {
        return Err(fail("pack_archive_limit", ARCHIVE_LIMIT_MESSAGE));
    }
    let archive = ArtifactCache::new(managed.clone())
        .acquire_sha512(&artifact.source)
        .await
        .map_err(|e| fail("pack_download_failed", e.to_string()))?;
    let pack = crate::mrpack::parse_verified_file(&archive.path)
        .map_err(|e| fail(e.code(), e.to_string()))?;
    // The instance platform this pack becomes, and the Modrinth loader tag of
    // its family: the same identity individual-mod installs check
    // (`PlatformPin::provider_loader`).
    let platform = pack.loader.platform_pin(&pack.minecraft_version);
    let loader_tag = platform
        .provider_loader()
        .map_err(|error| fail("pack_unsupported_loader", error))?;
    let loader_name = pack.loader.kind.display_name();
    // A version listing several loaders is decided by its index; the index's
    // loader must be one the version metadata lists.
    if !artifact
        .game_versions
        .iter()
        .any(|v| v == &pack.minecraft_version)
        || !artifact
            .loaders
            .iter()
            .any(|v| v.eq_ignore_ascii_case(loader_tag))
    {
        return Err(fail(
            "pack_identity_conflict",
            format!(
                "The pack index does not agree with this Modrinth version's Minecraft and {loader_name} metadata."
            ),
        ));
    }
    if let Some(refusal) = loader_build_refusal(&pack) {
        return Err(refusal);
    }
    if let Some(gate) = gate {
        gate.after_parse(&pack)?;
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
    let recognized_files: Vec<_> = files
        .iter()
        .filter_map(|file| identities.get(&file.sha512).map(|item| (*file, item)))
        .collect();
    if component_limit.is_some_and(|limit| recognized_files.len() > limit) {
        return Err(fail(
            "pack_component_limit",
            component_limit_message(recognized_files.len(), false),
        ));
    }
    let mut planned_files = Vec::new();
    crate::transfer::settled_window(&recognized_files, PACK_DOWNLOAD_CONCURRENCY,
        || crate::content_jobs::checkpoint().is_ok(),
        |(file, item)| async move {
            provider
                .pack_component_plan(item, &file.path, file.file_size, loader_tag)
                .await
                .map_err(|e| {
                    if matches!(e, crate::modrinth::Error::DependencyConflict) {
                        fail(
                            "pack_component_incompatible",
                            format!(
                                "{}: Modrinth marks this file incompatible with a client {loader_name} installation.",
                                file.path
                            ),
                        )
                    } else {
                        fail("pack_identity_conflict", format!("{}: {e}", file.path))
                    }
                })

        },
        |(file, item), planned| planned_files.push((*file, *item, planned)),
    ).await?;
    for (file, item, planned) in planned_files {
        project_set.insert(planned.project_id.clone());
        provider_preview.push(RecognizedPreview {
            path: file.path.clone(),
            project_id: planned.project_id.clone(),
            version_id: planned.version_id.clone(),
            title: item.version_name.clone(),
        });
        provider_plans.push(planned);
    }
    for file in files {
        if !identities.contains_key(&file.sha512) {
            unresolved.push(file.clone());
        }
    }
    // Modrinth-declared requirements absent from the pack are confirmed
    // against the components' own loader metadata below rather than failing
    // outright: mods commonly bundle a declared dependency as a jar-in-jar
    // (Mod Menu ships Text Placeholder API), and the mod loader, not Modrinth
    // metadata, decides what launches.
    let mut unconfirmed = Vec::new();
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
                        unconfirmed.push(planned.file_name.clone());
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
    if !unconfirmed.is_empty() {
        confirm_with_loader_metadata(
            managed,
            &pack,
            &platform,
            &provider_plans,
            &unresolved,
            &unconfirmed,
        )
        .await?;
        checkpoint()?;
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
        loader: pack.loader.kind,
        loader_version: pack.loader.version.clone(),
        archive_sha512: artifact.sha512.clone(),
        recognized: provider_preview,
        unresolved: unresolved.iter().map(|f| f.path.clone()).collect(),
        optional: pack.optional_files.iter().map(|f| f.path.clone()).collect(),
        excluded,
        overrides: pack.overrides.iter().map(|f| f.path.clone()).collect(),
        skipped_overrides: pack.skipped_overrides.clone(),
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
        provider_title: artifact.title,
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
/// any instance. Downloads complete in the shared verified cache first. The
/// resume intent is durable before the Installing record exists; from then
/// on every failure or cancellation keeps that instance, names it in the
/// error, and [`resume`] finishes the same instance instead of creating
/// another one.
#[allow(clippy::too_many_arguments)]
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
    checkpoint()?;
    progress(progress_phase::RESOLVING_PACK);
    let resolved = resolve_bounded(
        managed,
        provider,
        project_id,
        version_id,
        selected_optional,
        Some(crate::instance_content::MAX_NEW_PACK_PROVIDER_PLANS),
        None,
    )
    .await?;
    if fingerprint != resolved.preview.fingerprint {
        return Err(fail(
            "pack_preview_stale",
            "The pack preview changed. Review this exact version again before installing.",
        ));
    }
    progress(progress_phase::DOWNLOADING);
    let external = acquire_pack_files(managed, &resolved).await?;
    checkpoint()?;
    progress(progress_phase::PREPARING_INSTANCE);
    // The exact loader pin the instance records and pack state repeats; the
    // ordinary validated installer resolves and installs it.
    let platform = resolved
        .pack
        .loader
        .platform_pin(&resolved.pack.minecraft_version);
    let mut configuration = InstanceConfiguration::from_parts(
        &resolved.pack.minecraft_version,
        pinned_loader_configuration(&platform)?,
        DEFAULT_MEMORY_MIB,
        String::new(),
        None,
    );
    configuration.set_aurora_enabled(false);
    // The project title ("Vulkan Optimized") names the instance; a pack
    // index name is often just a version string. The pack identity
    // recorded below keeps the index name, which pack state verifies.
    let mut display_name = if resolved.provider_title.trim().is_empty() {
        resolved.pack.name.clone()
    } else {
        resolved.provider_title.trim().to_owned()
    };
    while display_name.len() > 80 {
        display_name.pop();
    }
    let identity = crate::instances::PackRegistryIdentity {
        provider: "modrinth".into(),
        project_id: resolved.provider_project_id.clone(),
        version_id: resolved.provider_version_id.clone(),
        name: resolved.pack.name.clone(),
        pack_version: resolved.pack.pack_version.clone(),
    };
    // The resume intent is written under the new instance's operation lock
    // before its Installing record is persisted; if it cannot be written,
    // no record exists and nothing needs resuming.
    let mut intent_error = None;
    let created = Box::pin(crate::instances::lifecycle::create_pack_record(
        managed,
        &managed.instance_registry_file(),
        endpoints,
        CreateInstanceRequest::new(&display_name, configuration),
        identity,
        &mut |record| {
            PendingInstall::new(record.id(), &resolved, selected_optional)
                .save_new(managed)
                .map_err(|error| {
                    let reason = error.message.clone();
                    intent_error = Some(error);
                    crate::instances::lifecycle::InstanceError::ReleaseInvalid(reason)
                })
        },
    ))
    .await;
    let (record, operation) = match created {
        Ok(created) => created,
        Err(error) => {
            return Err(intent_error
                .take()
                .unwrap_or_else(|| fail("pack_instance_failed", error.to_string())));
        }
    };
    let pending = PendingInstall::new(record.id(), &resolved, selected_optional);
    let result = async {
        install_game(managed, endpoints, &record, &operation, progress).await?;
        install_content(
            managed, &record, &operation, resolved, external, &pending, progress,
        )
        .await
    }
    .await;
    result.map_err(|error| error.kept(&record))
}

/// Resumes the interrupted new-pack installation of one Installing pack
/// instance, finishing that same instance. It never deletes the instance,
/// never adopts or overwrites bytes it did not just write, and never
/// substitutes content: the exact reviewed provider version is re-resolved
/// and must still match the stored archive SHA-512 and preview fingerprint.
/// A game left incomplete is installed through the ordinary validated
/// installer; a pack whose state was already saved is only completed.
pub async fn resume(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    provider: &crate::modrinth::Client,
    instance: &InstanceId,
    progress: &mut (dyn FnMut(&'static str) + Send),
) -> Result<InstanceRecord, PackInstallError> {
    checkpoint().map_err(|error| error.for_instance(instance))?;
    progress(progress_phase::RESOLVING_PACK);
    let operation = crate::instances::lifecycle::lock_instance_operation(managed, instance)
        .map_err(|error| {
            match error {
                crate::install::InstallError::AlreadyInProgress { .. } => fail(
                    "installation_already_in_progress",
                    "This instance is already being installed or changed. Retry when that finishes.",
                ),
                other => fail("pack_io_error", other.to_string()),
            }
            .for_instance(instance)
        })?;
    crate::install::recover_installation(managed, instance)
        .map_err(|error| fail("pack_instance_failed", error.to_string()).for_instance(instance))?;
    let record = {
        let _guard = crate::instances::lifecycle::registry_lock();
        crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
            .map_err(|error| fail("instance_registry_invalid", error.to_string()))
            .and_then(|registry| {
                registry.find(instance).cloned().ok_or_else(|| {
                    fail(
                        "instance_not_found",
                        format!("no instance '{instance}' exists in the launcher's registry"),
                    )
                })
            })
            .map_err(|error| error.for_instance(instance))?
    };
    if record.state() != crate::instances::InstanceState::Installing {
        return Err(fail(
            "pack_resume_not_installing",
            format!(
                "Only a modpack instance that is still installing can be resumed; this instance is {}.",
                record.state()
            ),
        )
        .for_instance(instance));
    }
    if record.pack().is_none() {
        return Err(fail(
            "pack_resume_unavailable",
            "This instance was not created from a modpack, so there is no modpack installation to resume.",
        )
        .for_instance(instance));
    }
    Box::pin(resume_record(
        managed, endpoints, provider, &record, &operation, progress,
    ))
    .await
    .map_err(|error| error.kept(&record))
}

async fn resume_record(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    provider: &crate::modrinth::Client,
    record: &InstanceRecord,
    operation: &crate::instances::lifecycle::InstanceOperationGuard,
    progress: &mut (dyn FnMut(&'static str) + Send),
) -> Result<InstanceRecord, PackInstallError> {
    let identity = record.pack().expect("checked by resume");
    // Interrupted after the pack state was saved: completion only, with no
    // network access and no writes beyond the Ready commit (the previous
    // completion-only retry behavior).
    match InstalledPack::load(managed, record.id()) {
        Ok(Some(_)) => {
            progress(progress_phase::VALIDATING);
            let completed = crate::instances::lifecycle::complete_pack_instance(
                managed,
                &managed.instance_registry_file(),
                &managed.config_file(),
                record.id(),
                operation,
            )
            .map_err(|error| fail("pack_validation_failed", error.to_string()))?;
            if let Ok(Some(pending)) = PendingInstall::load(managed, record.id()) {
                pending.remove_completed(managed);
            }
            progress(progress_phase::COMPLETE);
            return Ok(completed);
        }
        Ok(None) => {}
        Err(error) => {
            return Err(fail(
                "pack_resume_state_invalid",
                format!(
                    "An earlier attempt left modpack state Aurora can't use ({error}), so Aurora won't resume the installation or overwrite that state."
                ),
            ));
        }
    }
    let pending = PendingInstall::load(managed, record.id())?.ok_or_else(|| {
        fail(
            "pack_resume_unavailable",
            "This instance has no record of the reviewed modpack installation, so Aurora can't resume it safely.",
        )
    })?;
    if pending.provider != identity.provider
        || pending.project_id != identity.project_id
        || pending.version_id != identity.version_id
    {
        return Err(fail(
            "pack_resume_state_invalid",
            "This instance's modpack resume record names a different modpack than the instance, so Aurora won't resume it.",
        ));
    }
    let root = managed.instance_paths(record.id()).root().to_path_buf();
    // Provider records left by an interrupted activation are never wiped
    // or adopted.
    if std::fs::symlink_metadata(root.join("content-managed.json")).is_ok() {
        match crate::instance_content::ContentState::load(managed, record.id()) {
            Ok(state) if state.entries.is_empty() => {}
            Ok(_) => {
                return Err(fail(
                    "pack_resume_conflict",
                    "An earlier attempt left installed component records in this instance, so Aurora can't resume without overwriting them. Nothing was changed.",
                ));
            }
            Err(_) => {
                return Err(fail(
                    "pack_resume_state_invalid",
                    "This instance's component records are damaged, so Aurora won't resume the installation or overwrite them. Nothing was changed.",
                ));
            }
        }
    }
    checkpoint()?;
    let resolved = resolve_bounded(
        managed,
        provider,
        &pending.project_id,
        &pending.version_id,
        &pending.selected_optional,
        Some(crate::instance_content::MAX_NEW_PACK_PROVIDER_PLANS),
        None,
    )
    .await?;
    let platform = resolved
        .pack
        .loader
        .platform_pin(&resolved.pack.minecraft_version);
    if !resolved
        .archive_sha512
        .eq_ignore_ascii_case(&pending.archive_sha512)
        || resolved.preview.fingerprint != pending.fingerprint
        || resolved.provider_project_id != pending.project_id
        || resolved.provider_version_id != pending.version_id
        || resolved.pack.name != identity.name
        || resolved.pack.pack_version != identity.pack_version
        || resolved.pack.minecraft_version != record.installed().minecraft_version
        || platform != record.installed().platform
    {
        return Err(fail(
            "pack_resume_changed",
            "This modpack version changed on Modrinth since it was reviewed, so Aurora won't install different content into this instance. Nothing was changed.",
        ));
    }
    if let Some(path) = occupied_destination(&root, &resolved) {
        return Err(fail(
            "pack_resume_conflict",
            format!(
                "'{path}' already exists in this instance, and Aurora never overwrites or adopts files it didn't just write. Nothing was changed; move that file out of the instance folder, then retry."
            ),
        ));
    }
    progress(progress_phase::DOWNLOADING);
    let external = acquire_pack_files(managed, &resolved).await?;
    checkpoint()?;
    install_game(managed, endpoints, record, operation, progress).await?;
    install_content(
        managed, record, operation, resolved, external, &pending, progress,
    )
    .await
}

/// The first pack destination (external file, override, or recognized
/// component) that already has any filesystem entry in the instance.
fn occupied_destination(root: &Path, resolved: &Resolved) -> Option<String> {
    let components = resolved
        .provider_plans
        .iter()
        .map(|plan| format!("{}/{}", plan.content_type.directory_name(), plan.file_name));
    resolved
        .unresolved
        .iter()
        .map(|file| file.path.clone())
        .chain(resolved.pack.overrides.iter().map(|item| item.path.clone()))
        .chain(components)
        .find(|relative| {
            let mut path = root.to_path_buf();
            path.extend(relative.split('/'));
            std::fs::symlink_metadata(path).is_ok()
        })
}

/// Acquires every pack file into the verified cache and checks each against
/// the pack's own digests before anything is written to an instance.
async fn acquire_pack_files(
    managed: &ManagedPaths,
    resolved: &Resolved,
) -> Result<Vec<AcquiredExternal>, PackInstallError> {
    let cache = ArtifactCache::new(managed.clone());
    // Pack files download concurrently through the shared transfer permits;
    // each is verified against the pack's own digests before use.
    let mut external = Vec::new();
    crate::transfer::settled_window(
        &resolved.unresolved,
        PACK_DOWNLOAD_CONCURRENCY,
        || crate::content_jobs::checkpoint().is_ok(),
        |file| {
            let cache = &cache;
            async move {
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
                Ok(artifact)
            }
        },
        |file, artifact| {
            external.push(AcquiredExternal {
                file: file.clone(),
                artifact,
            })
        },
    )
    .await?;
    checkpoint()?;
    crate::transfer::settled_each(
        &resolved.provider_plans,
        PACK_DOWNLOAD_CONCURRENCY,
        || crate::content_jobs::checkpoint().is_ok(),
        |planned| {
            let cache = &cache;
            async move {
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
                    .ok_or_else(|| {
                        fail("pack_plan_invalid", "A recognized file left the pack plan.")
                    })?;
                verify_external(file, &artifact)
            }
        },
        |_, ()| {},
    )
    .await?;
    Ok(external)
}

/// The game foundation of the Installing pack instance, through the
/// ordinary validated installer (skipped when a matching valid game exists).
async fn install_game(
    managed: &ManagedPaths,
    endpoints: &InstanceEndpoints,
    record: &InstanceRecord,
    operation: &crate::instances::lifecycle::InstanceOperationGuard,
    progress: &mut (dyn FnMut(&'static str) + Send),
) -> Result<(), PackInstallError> {
    // Game installation reports many fine-grained events; the pack stage
    // changes once, so hosts are not re-rendered per event.
    let mut game_started = false;
    let mut game_progress = |_event: InstanceProgress| {
        if !std::mem::replace(&mut game_started, true) {
            progress(progress_phase::INSTALLING_GAME);
        }
    };
    Box::pin(crate::instances::lifecycle::install_pack_game(
        managed,
        &managed.instance_registry_file(),
        endpoints,
        record,
        operation,
        &mut game_progress,
    ))
    .await
    .map_err(|error| match error {
        // The game installer observed the job's cancellation itself and
        // preserved its previous state.
        crate::instances::lifecycle::InstanceError::GameInstall(
            crate::install::InstallError::Cancelled,
        ) => fail(
            "installation_cancelled",
            "Installation cancelled before activation.",
        ),
        other => fail("pack_instance_failed", other.to_string()),
    })?;
    Ok(())
}

/// Installs the resolved pack's content into its Installing instance and
/// promotes it to Ready: the one content path shared by [`install`] and
/// [`resume`]. Cancellation is honored only before the commit window opens.
/// A failure removes exactly the files this call created and keeps the
/// Installing record and its resume intent.
async fn install_content(
    managed: &ManagedPaths,
    record: &InstanceRecord,
    operation: &crate::instances::lifecycle::InstanceOperationGuard,
    resolved: Resolved,
    external: Vec<AcquiredExternal>,
    pending: &PendingInstall,
    progress: &mut (dyn FnMut(&'static str) + Send),
) -> Result<InstanceRecord, PackInstallError> {
    let _commit = crate::content_jobs::commit().map_err(|_| {
        fail(
            "installation_cancelled",
            "Installation cancelled before activation.",
        )
    })?;
    let platform = resolved
        .pack
        .loader
        .platform_pin(&resolved.pack.minecraft_version);
    let root = managed.instance_paths(record.id()).root().to_path_buf();
    let mut created: Vec<(PathBuf, String)> = Vec::new();
    let result = (|| -> Result<InstalledPack, PackInstallError> {
        progress(progress_phase::INSTALLING_OVERRIDES);
        let mut components = Vec::new();
        for item in &external {
            #[cfg(test)]
            materialize_fault::check(created.len())?;
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
            #[cfg(test)]
            materialize_fault::check(created.len())?;
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
                loader: platform.clone(),
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
            return Err(rollback_pack_install(managed, record, &created, &[], error));
        }
    };
    progress(progress_phase::INSTALLING_COMPONENTS);
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
                record,
                &created,
                &[],
                fail("pack_component_install_failed", error.to_string()),
            ));
        }
    };
    for provider_record in &provider_records {
        let path = format!(
            "{}/{}",
            provider_record.content_type.directory_name(),
            provider_record.file_name
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
            sha256: provider_record.sha256.clone(),
            sha512: file.sha512.clone(),
            provider: Some(provider_record.identity()),
            provider_version_id: Some(provider_record.version_id.clone()),
        });
    }
    progress(progress_phase::VALIDATING);
    if let Err(error) = state.save(managed) {
        return Err(rollback_pack_install(
            managed,
            record,
            &created,
            &provider_records,
            fail(error.code(), error.to_string()),
        ));
    }
    if let Err(error) = state.validate_installed(managed) {
        return Err(rollback_pack_install(
            managed,
            record,
            &created,
            &provider_records,
            fail(error.code(), error.to_string()),
        ));
    }
    let completed = match crate::instances::lifecycle::complete_pack_instance(
        managed,
        &managed.instance_registry_file(),
        &managed.config_file(),
        record.id(),
        operation,
    ) {
        Ok(completed) => completed,
        Err(error) => {
            return Err(rollback_pack_install(
                managed,
                record,
                &created,
                &provider_records,
                fail("pack_validation_failed", error.to_string()),
            ));
        }
    };
    // The Ready registry write is the commit; the intent is now obsolete.
    pending.remove_completed(managed);
    progress(progress_phase::COMPLETE);
    Ok(completed)
}

/// Test-only fault: fail pack file materialization once `count` files were
/// written by the current install.
#[cfg(test)]
pub(crate) mod materialize_fault {
    use std::cell::Cell;

    thread_local! {
        static FAIL_AT: Cell<Option<usize>> = const { Cell::new(None) };
    }

    pub(crate) fn fail_at(count: Option<usize>) {
        FAIL_AT.with(|cell| cell.set(count));
    }

    /// Fires once, then clears itself.
    pub(crate) fn check(created: usize) -> Result<(), super::PackInstallError> {
        FAIL_AT.with(|cell| {
            if cell.get() == Some(created) {
                cell.set(None);
                Err(super::fail(
                    "pack_io_error",
                    "deterministic materialization fault",
                ))
            } else {
                Ok(())
            }
        })
    }
}

/// Evaluates the pack's own mod jars the way the pack's loader would, with
/// the same per-loader metadata readers and compatibility rules Play
/// readiness uses (`instance_mods::inspect_mod_metadata`,
/// `mod_compatibility::validate`): every hard dependency must be satisfied
/// by a top-level pack mod, a bundled (jar-in-jar) copy, or that loader's
/// built-ins (`minecraft`, and `fabricloader`, `forge` or `neoforge`).
/// Forge and NeoForge jars are never read as Fabric metadata. Called only
/// when Modrinth metadata names a requirement the pack does not list, so
/// ordinary packs never download during review. The jars land in the
/// verified cache and are reused by installation.
async fn confirm_with_loader_metadata(
    managed: &ManagedPaths,
    pack: &PackPlan,
    platform: &PlatformPin,
    provider_plans: &[ProviderInstallPlan],
    unresolved: &[PackFile],
    unconfirmed: &[String],
) -> Result<(), PackInstallError> {
    use crate::instance_content::ContentType;
    let cache = ArtifactCache::new(managed.clone());
    let mod_plans: Vec<&ProviderInstallPlan> = provider_plans
        .iter()
        .filter(|plan| plan.content_type == ContentType::Mod)
        .collect();
    let external_mods: Vec<&PackFile> = unresolved
        .iter()
        .filter(|file| file.path.starts_with("mods/") && file.path.ends_with(".jar"))
        .collect();
    let provider_artifacts = acquire_provider_artifacts(&cache, &mod_plans).await?;
    let external_artifacts = acquire_externals(&cache, &external_mods).await?;
    let mut inventory = crate::instance_mods::ModInventory {
        instance_id: String::new(),
        entries: Vec::new(),
        missing_managed: Vec::new(),
    };
    let jars = mod_plans
        .iter()
        .map(|plan| plan.file_name.clone())
        .zip(&provider_artifacts)
        .chain(
            external_mods
                .iter()
                .map(|file| file.path.trim_start_matches("mods/").to_owned())
                .zip(&external_artifacts),
        );
    // The platform kind names both the metadata reader and the
    // compatibility family (`fabric`, `forge`, `neoForge`).
    let family = platform.kind();
    let loader_name = pack.loader.kind.display_name();
    for (file_name, artifact) in jars {
        let (metadata, warnings) =
            crate::instance_mods::inspect_mod_metadata(&artifact.path, artifact.bytes, family);
        let Some(metadata) = metadata else {
            if unconfirmed.contains(&file_name) {
                return Err(fail(
                    "pack_dependency_conflict",
                    format!(
                        "{file_name} requires a Modrinth dependency absent from this pack, and its {loader_name} metadata could not be read to confirm it is bundled."
                    ),
                ));
            }
            continue;
        };
        crate::mod_compatibility::add_artifact(
            &mut inventory,
            file_name,
            metadata,
            artifact.sha256.as_hex(),
            warnings,
        );
    }
    // The loader version in the instance pin format (Forge `<mc>-<build>`;
    // the Forge evaluator compares the build half, as at Play).
    let issues = crate::mod_compatibility::validate(
        &inventory,
        &pack.minecraft_version,
        platform.version().unwrap_or_default(),
        None,
        family,
    );
    // Only the jars Modrinth flagged are decided here. Other issues (for
    // example a requirement Fabric Loader itself supplies, such as
    // MixinExtras) stay with Play readiness, which evaluates the installed,
    // digest-recorded loader jar's modules; no loader jar exists yet here.
    if let Some(problem) = issues.iter().find(|issue| {
        unconfirmed.contains(&issue.file_name)
            && matches!(
                issue.code.as_str(),
                "mod_dependency_unsatisfied" | "mod_metadata_unusable"
            )
    }) {
        return Err(fail(
            "pack_dependency_conflict",
            format!("{}: {}", problem.file_name, problem.message),
        ));
    }
    Ok(())
}

/// Acquires and verifies external pack files concurrently, returning the
/// artifacts in input order.
pub(crate) async fn acquire_externals(
    cache: &ArtifactCache,
    files: &[&PackFile],
) -> Result<Vec<VerifiedArtifact>, PackInstallError> {
    let mut artifacts = Vec::with_capacity(files.len());
    crate::transfer::settled_window(
        files,
        PACK_DOWNLOAD_CONCURRENCY,
        || crate::content_jobs::checkpoint().is_ok(),
        |file| async move {
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
            Ok(artifact)
        },
        |_, artifact| artifacts.push(artifact),
    )
    .await?;
    checkpoint()?;
    Ok(artifacts)
}

/// Acquires provider-resolved component artifacts concurrently, in order.
pub(crate) async fn acquire_provider_artifacts(
    cache: &ArtifactCache,
    plans: &[&ProviderInstallPlan],
) -> Result<Vec<VerifiedArtifact>, PackInstallError> {
    let mut artifacts = Vec::with_capacity(plans.len());
    crate::transfer::settled_window(
        plans,
        PACK_DOWNLOAD_CONCURRENCY,
        || crate::content_jobs::checkpoint().is_ok(),
        |plan| async move {
            match &plan.source {
                ProviderArtifactSource::Sha512(source) => cache.acquire_sha512(source).await,
                ProviderArtifactSource::Sha256(source) => cache.acquire(source).await,
            }
            .map_err(|e| fail("pack_download_failed", e.to_string()))
        },
        |_, artifact| artifacts.push(artifact),
    )
    .await?;
    checkpoint()?;
    Ok(artifacts)
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
    // F3R retains the proven game, journal, lock rendezvous, and explicit
    // Installing record. Erasing the instance root would erase durable
    // recovery authority and could unlink a live rendezvous file.
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
pub(crate) mod tests {
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

    fn project_json() -> Vec<u8> {
        serde_json::json!({
            "id": "PACK0001", "project_type": "modpack", "title": "Fixture Pack", "description": "Fixture",
            "license": {"id":"mit"}, "game_versions": ["1.21.1"], "loaders": ["fabric", "forge", "neoforge", "quilt"],
            "environment": ["client_only"]
        })
        .to_string()
        .into_bytes()
    }

    fn version_json(
        id: &str,
        loaders: &[&str],
        game_versions: &[&str],
        archive: &[u8],
        base_url: &str,
    ) -> Vec<u8> {
        serde_json::json!({
            "id": id, "project_id": "PACK0001", "name": "One", "version_number": "1",
            "version_type": "release", "date_published": "2026-01-01T00:00:00Z", "game_versions": game_versions,
            "loaders": loaders, "environment": "client_only", "dependencies": [],
            "files": [{"hashes": {"sha512": format!("{:x}", Sha512::digest(archive))}, "url": format!("{base_url}/pack.mrpack"),
                "filename": "pack.mrpack", "primary": true, "size": archive.len()}]
        })
        .to_string()
        .into_bytes()
    }

    fn temporary_managed(label: &str) -> (PathBuf, ManagedPaths) {
        let root = std::env::temp_dir().join(format!("aurora-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        (root, managed)
    }

    #[tokio::test]
    async fn uninstallable_versions_are_refused_from_metadata_before_any_archive_request() {
        let bytes = archive(
            &serde_json::json!({
                "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Fixture Pack",
                "files": [], "dependencies": {"minecraft": "1.21.1", "neoforge": "21.1.1"}
            })
            .to_string(),
        );
        // (version id, loaders, game versions, error code, message parts)
        type Case<'a> = (
            &'a str,
            &'a [&'a str],
            &'a [&'a str],
            &'a str,
            &'a [&'a str],
        );
        let cases: &[Case] = &[
            (
                "NEOF0001",
                &["neoforge"],
                &["1.21.1"],
                "pack_unsupported_loader",
                &[
                    "NeoForge modpack for Minecraft 1.21.1",
                    NEOFORGE_MINECRAFT_RANGE,
                ],
            ),
            (
                "FORG0001",
                &["forge"],
                &["1.12.2"],
                "pack_unsupported_loader",
                &["Forge modpack for Minecraft 1.12.2", FORGE_MINECRAFT_RANGE],
            ),
            (
                // 1.13.2 through 1.14.3 have no installable Forge build at all.
                "FORG0002",
                &["forge"],
                &["1.13.2", "1.14.3"],
                "pack_unsupported_loader",
                &["Forge modpack for Minecraft 1.13.2, 1.14.3"],
            ),
            (
                "BOTH0001",
                &["forge", "neoforge"],
                &["1.12.2"],
                "pack_unsupported_loader",
                &[
                    "Forge and NeoForge modpack",
                    FORGE_MINECRAFT_RANGE,
                    NEOFORGE_MINECRAFT_RANGE,
                ],
            ),
            (
                "QUIL0001",
                &["quilt"],
                &["1.21.1"],
                "pack_unsupported_loader",
                &["Quilt modpack", "does not support Quilt"],
            ),
            (
                "NONE0001",
                &[],
                &["1.21.1"],
                "pack_unsupported_loader",
                &["names no mod loader"],
            ),
            (
                "ODDL0001",
                &["liteloader"],
                &["1.21.1"],
                "pack_unsupported_loader",
                &["Aurora installs Fabric, Forge and NeoForge modpacks"],
            ),
            (
                "NOMC0001",
                &["fabric"],
                &[],
                "pack_identity_conflict",
                &["names no Minecraft version"],
            ),
        ];
        let archive_requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = Arc::clone(&archive_requests);
        let served: Vec<(String, Vec<String>, Vec<String>)> = cases
            .iter()
            .map(|(id, loaders, games, _, _)| {
                (
                    (*id).to_owned(),
                    loaders.iter().map(|v| (*v).to_owned()).collect(),
                    games.iter().map(|v| (*v).to_owned()).collect(),
                )
            })
            .collect();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let path = request.path.split('?').next().unwrap_or_default();
            if path == "/pack.mrpack" {
                counted.fetch_add(1, Ordering::SeqCst);
                return TestResponse::ok(&bytes);
            }
            if path == "/v2/project/PACK0001" {
                return TestResponse::ok(&project_json());
            }
            for (id, loaders, games) in &served {
                if path == format!("/v2/version/{id}") {
                    let loaders: Vec<&str> = loaders.iter().map(String::as_str).collect();
                    let games: Vec<&str> = games.iter().map(String::as_str).collect();
                    return TestResponse::ok(&version_json(
                        id,
                        &loaders,
                        &games,
                        &bytes,
                        &request.base_url,
                    ));
                }
            }
            TestResponse::status(404)
        }));
        let (root, managed) = temporary_managed("pack-loader-refusal");
        let client = crate::modrinth::Client::for_testing(&format!("{}/v2/", server.base_url()));
        let endpoints = InstanceEndpoints::development().unwrap();
        for (version, _, _, code, wording) in cases {
            let error = preview(&managed, &client, "PACK0001", version, &[])
                .await
                .unwrap_err();
            assert_eq!(error.code, *code, "{version}: {error}");
            for expected in *wording {
                assert!(error.message.contains(expected), "{version}: {error}");
            }
            let error = install(
                &managed,
                &endpoints,
                &client,
                "PACK0001",
                version,
                &[],
                "fingerprint",
                &mut |_| {},
            )
            .await
            .unwrap_err();
            assert_eq!(error.code, *code, "{version}: {error}");
        }
        assert_eq!(archive_requests.load(Ordering::SeqCst), 0);
        assert!(!managed.instance_registry_file().exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn more_recognized_components_than_a_new_pack_instance_installs_are_refused_before_instance_creation()
     {
        let limit = crate::instance_content::MAX_NEW_PACK_PROVIDER_PLANS;
        let count = limit + 1;
        let hashes: Vec<String> = (0..count).map(|i| format!("{:0128x}", i + 1)).collect();
        let files: Vec<_> = hashes
            .iter()
            .enumerate()
            .map(|(i, sha512)| {
                serde_json::json!({
                    "path": format!("mods/component-{i}.jar"),
                    "hashes": {"sha1": "a".repeat(40), "sha512": sha512},
                    "downloads": [format!("https://cdn.modrinth.com/data/C{i:07}/component.jar")],
                    "fileSize": 4
                })
            })
            .collect();
        let bytes = archive(
            &serde_json::json!({
                "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Large Pack",
                "files": files, "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.0"}
            })
            .to_string(),
        );
        let component = |sha512: &str| {
            serde_json::json!({
                "id": format!("V{}", &sha512[120..]), "project_id": format!("P{}", &sha512[120..]),
                "name": "Component", "version_number": "1", "version_type": "release",
                "date_published": "2026-01-01T00:00:00Z", "game_versions": ["1.21.1"],
                "loaders": ["fabric"], "environment": "client_and_server", "dependencies": [],
                "files": [{"hashes": {"sha512": sha512, "sha1": "a".repeat(40)},
                    "url": "https://cdn.modrinth.com/data/test/component.jar",
                    "filename": "component.jar", "primary": true, "size": 4, "file_type": null}]
            })
        };
        let unexpected = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let recorded = Arc::clone(&unexpected);
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let path = request.path.split('?').next().unwrap_or_default();
            match path {
                "/pack.mrpack" => TestResponse::ok(&bytes),
                "/v2/project/PACK0001" => TestResponse::ok(&project_json()),
                "/v2/version/VERS0001" => TestResponse::ok(&version_json(
                    "VERS0001",
                    &["fabric"],
                    &["1.21.1"],
                    &bytes,
                    &request.base_url,
                )),
                "/v2/version_files" => {
                    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                    let mapped: serde_json::Map<String, serde_json::Value> = body["hashes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|hash| {
                            let hash = hash.as_str().unwrap();
                            (hash.to_owned(), component(hash))
                        })
                        .collect();
                    TestResponse::ok(serde_json::Value::Object(mapped).to_string().as_bytes())
                }
                other if other.starts_with("/v2/version_file/") => {
                    let hash = other.trim_start_matches("/v2/version_file/");
                    TestResponse::ok(component(hash).to_string().as_bytes())
                }
                other => {
                    recorded.lock().unwrap().push(other.to_owned());
                    TestResponse::status(404)
                }
            }
        }));
        let (root, managed) = temporary_managed("pack-component-limit");
        let client = crate::modrinth::Client::for_testing(&format!("{}/v2/", server.base_url()));
        let error = preview(&managed, &client, "PACK0001", "VERS0001", &[])
            .await
            .unwrap_err();
        assert_eq!(error.code, "pack_component_limit", "{error}");
        assert!(
            error.message.contains(&format!(
                "{count} Modrinth-recognized components; Aurora currently installs at most {limit}"
            )),
            "{error}"
        );
        let endpoints = InstanceEndpoints::development().unwrap();
        let error = install(
            &managed,
            &endpoints,
            &client,
            "PACK0001",
            "VERS0001",
            &[],
            "fingerprint",
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "pack_component_limit", "{error}");
        // No component metadata or jar was requested and no instance exists.
        assert!(unexpected.lock().unwrap().is_empty(), "{unexpected:?}");
        assert!(!managed.instance_registry_file().exists());
        assert!(!managed.instances_dir().exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn progress_vocabulary_is_unique_and_names_every_install_step() {
        let unique: HashSet<_> = progress_phase::ALL.iter().collect();
        assert_eq!(unique.len(), progress_phase::ALL.len());
        assert!(progress_phase::ALL.contains(&"installingGame"));
        assert_eq!(
            component_limit_message(70, true),
            "This pack would add 70 Modrinth-recognized components to this instance; Aurora currently installs at most 64 components per operation."
        );
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
                loader: crate::instances::platform::PlatformPin::Fabric {
                    version: "0.19.3".into(),
                },
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

    // ------------------------------------------------------------------
    // New-instance packs for Fabric, Forge and NeoForge (loopback only).
    // ------------------------------------------------------------------

    pub(crate) const LOADER_MC: &str = "26.2";
    pub(crate) const FABRIC_LOADER: &str = "0.19.5";
    /// The bare build a Forge index declares; the instance pins `26.2-63.0.1`.
    pub(crate) const FORGE_BUILD: &str = "63.0.1";
    pub(crate) const NEOFORGE_VERSION: &str = "26.2.0.88";

    fn sha1_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha1::digest(bytes))
    }

    fn sha512_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha512::digest(bytes))
    }

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    /// A minimal mod jar that the loader's own metadata reader accepts. With
    /// `requires`, the jar declares a hard dependency on that mod id in its
    /// loader's own dependency syntax, besides the loader built-ins.
    pub(crate) fn loader_mod_jar(
        kind: PackLoaderKind,
        id: &str,
        requires: Option<&str>,
    ) -> Vec<u8> {
        match kind {
            PackLoaderKind::Fabric => {
                let mut depends = serde_json::json!({"minecraft": "*", "fabricloader": ">=0.19"});
                if let Some(other) = requires {
                    depends[other] = serde_json::json!("*");
                }
                let document = serde_json::json!({
                    "schemaVersion": 1, "id": id, "version": "1.0.0", "depends": depends
                });
                zip_of(&[("fabric.mod.json", document.to_string().as_bytes())])
            }
            PackLoaderKind::Forge | PackLoaderKind::NeoForge => {
                let (entry, loader, range, required) = if kind == PackLoaderKind::Forge {
                    ("META-INF/mods.toml", "forge", "[63,)", "mandatory=true")
                } else {
                    (
                        "META-INF/neoforge.mods.toml",
                        "neoforge",
                        "[26.2.0.88,)",
                        "type=\"required\"",
                    )
                };
                let mut text = format!(
                    "modLoader=\"javafml\"\nloaderVersion=\"[1,)\"\nlicense=\"MIT\"\n[[mods]]\nmodId=\"{id}\"\nversion=\"1.0.0\"\n"
                );
                for (dependency, dependency_range) in std::iter::once((loader, range))
                    .chain(std::iter::once(("minecraft", "[26.2]")))
                    .chain(requires.map(|other| (other, "[1,)")))
                {
                    text.push_str(&format!(
                        "[[dependencies.{id}]]\nmodId=\"{dependency}\"\n{required}\nversionRange=\"{dependency_range}\"\nside=\"BOTH\"\n"
                    ));
                }
                zip_of(&[(entry, text.as_bytes())])
            }
        }
    }

    /// One Modrinth-recognized pack component.
    pub(crate) struct ComponentFixture {
        pub(crate) file_name: String,
        pub(crate) project: String,
        pub(crate) version: String,
        pub(crate) bytes: Vec<u8>,
        /// Modrinth loader tags on the component version.
        tags: Vec<&'static str>,
        /// Modrinth-declared required project dependencies.
        requires_projects: Vec<String>,
    }

    impl ComponentFixture {
        pub(crate) fn new(index: usize, kind: PackLoaderKind, tags: &[&'static str]) -> Self {
            Self {
                file_name: format!("component-{index}.jar"),
                project: format!("P{index:07}"),
                version: format!("V{index:07}"),
                bytes: loader_mod_jar(kind, &format!("component_{index}"), None),
                tags: tags.to_vec(),
                requires_projects: Vec::new(),
            }
        }
    }

    /// The complete offline world for one new pack instance: Mojang game
    /// artifacts for Minecraft 26.2, Fabric Meta, the Forge Maven listing,
    /// promotions and installer, the NeoForge listing, installer and
    /// universal jar (installers carry no processors, so no Java runs), and
    /// the Modrinth v2 API for the pack project and its components.
    pub(crate) struct LoaderWorld {
        _server: TestServer,
        base: String,
        bodies: Arc<std::sync::Mutex<BTreeMap<String, Vec<u8>>>>,
        hashes: Arc<std::sync::Mutex<BTreeMap<String, serde_json::Value>>>,
        requests: Arc<std::sync::Mutex<Vec<String>>>,
        root: PathBuf,
        pub(crate) managed: ManagedPaths,
        pub(crate) endpoints: InstanceEndpoints,
        pub(crate) client: crate::modrinth::Client,
    }

    impl Drop for LoaderWorld {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl LoaderWorld {
        pub(crate) fn new(label: &str) -> Self {
            let bodies = Arc::new(std::sync::Mutex::new(BTreeMap::<String, Vec<u8>>::new()));
            let hashes = Arc::new(std::sync::Mutex::new(
                BTreeMap::<String, serde_json::Value>::new(),
            ));
            let requests = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
            let (served, lookups, seen) = (
                Arc::clone(&bodies),
                Arc::clone(&hashes),
                Arc::clone(&requests),
            );
            let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
                let path = request
                    .path
                    .split('?')
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                seen.lock().unwrap().push(path.clone());
                if path == "/v2/version_files" {
                    let body: serde_json::Value =
                        serde_json::from_slice(&request.body).unwrap_or_default();
                    let map = lookups.lock().unwrap();
                    let answer: serde_json::Map<String, serde_json::Value> = body["hashes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|hash| hash.as_str())
                        .filter_map(|hash| map.get(hash).map(|v| (hash.to_owned(), v.clone())))
                        .collect();
                    return TestResponse::ok(
                        serde_json::Value::Object(answer).to_string().as_bytes(),
                    );
                }
                if let Some(hash) = path.strip_prefix("/v2/version_file/") {
                    return match lookups.lock().unwrap().get(hash) {
                        Some(version) => TestResponse::ok(version.to_string().as_bytes()),
                        None => TestResponse::status(404),
                    };
                }
                match served.lock().unwrap().get(&path) {
                    Some(body) => TestResponse::ok(body),
                    None => TestResponse::status(404),
                }
            }));
            let base = server.base_url().to_owned();
            let root = std::env::temp_dir().join(format!(
                "aurora-pack-loader-{label}-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
            let world = Self {
                _server: server,
                base: base.clone(),
                bodies,
                hashes,
                requests,
                root,
                managed,
                endpoints: InstanceEndpoints::for_testing(
                    crate::distribution::ReleaseManifest::from_json(
                        r#"{"schemaVersion": 1, "releases": []}"#,
                    )
                    .unwrap(),
                    crate::minecraft::metadata::MetadataEndpoints::loopback_for_testing(&base),
                    crate::fabric::metadata::FabricMetaEndpoints::loopback_for_testing(&format!(
                        "{base}/v2/"
                    )),
                    crate::neoforge::metadata::NeoForgeMavenEndpoints::loopback_for_testing(&base),
                    crate::forge::metadata::ForgeMavenEndpoints::loopback_for_testing(
                        &format!("{base}/forge-maven/"),
                        &format!("{base}/forge-promotions.json"),
                    )
                    .unwrap(),
                    crate::runtime::metadata::RuntimeMetadataEndpoints::loopback_for_testing(&base),
                    crate::install::InstallContext::loopback_for_testing(
                        crate::downloads::DownloadOptions {
                            connect_timeout: std::time::Duration::from_secs(5),
                            idle_read_timeout: std::time::Duration::from_secs(5),
                            max_redirects: crate::downloads::MAX_REDIRECTS,
                        },
                        crate::install::assets::AssetObjectEndpoints::loopback_for_testing(
                            &format!("{base}/assets/"),
                        ),
                    ),
                ),
                client: crate::modrinth::Client::for_testing(&format!("{base}/v2/")),
            };
            world.serve_game();
            world
        }

        pub(crate) fn insert(&self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
            self.bodies
                .lock()
                .unwrap()
                .insert(path.into(), bytes.into());
        }

        /// Stops serving `path`; returns its body so it can be restored.
        pub(crate) fn take(&self, path: &str) -> Vec<u8> {
            self.bodies.lock().unwrap().remove(path).unwrap()
        }

        pub(crate) fn total_requests(&self) -> usize {
            self.requests.lock().unwrap().len()
        }

        pub(crate) fn requested(&self, path: &str) -> usize {
            self.requests
                .lock()
                .unwrap()
                .iter()
                .filter(|seen| seen.as_str() == path)
                .count()
        }

        /// Mojang 26.2 plus every loader's official metadata and artifacts.
        fn serve_game(&self) {
            let base = &self.base;
            let sha1 = |bytes: &[u8]| crate::integrity::Sha1Digest::compute(bytes).as_hex();
            let client = b"synthetic client jar bytes".to_vec();
            let logging = b"<Configuration/>".to_vec();
            let library = b"synthetic mojang library jar".to_vec();
            let natives = zip_of(&[
                ("META-INF/MANIFEST.MF", b"synthetic native content"),
                ("lwjgl.dll", b"synthetic native content"),
            ]);
            let asset = b"tiny png bytes a".to_vec();
            let asset_hash = sha1(&asset);
            let asset_index = format!(
                r#"{{"objects": {{"icons/icon_16x16.png": {{"hash": "{asset_hash}", "size": {}}}}}}}"#,
                asset.len()
            );
            self.insert("/mojang/client.jar", client.clone());
            self.insert("/mojang/logging/client-1.21.2.xml", logging.clone());
            self.insert(
                "/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
                library.clone(),
            );
            self.insert(
                "/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar",
                natives.clone(),
            );
            self.insert("/mojang/asset-index/32.json", asset_index.clone());
            self.insert(format!("/assets/{}/{asset_hash}", &asset_hash[..2]), asset);
            let version_document = serde_json::json!({
                "id": LOADER_MC, "type": "release", "mainClass": "net.minecraft.client.main.Main",
                "javaVersion": {"component": "java-runtime-epsilon", "majorVersion": 25},
                "assetIndex": {"id": "32", "sha1": sha1(asset_index.as_bytes()), "size": asset_index.len(),
                    "totalSize": 999999, "url": format!("{base}/mojang/asset-index/32.json")},
                "downloads": {"client": {"sha1": sha1(&client), "size": client.len(), "url": format!("{base}/mojang/client.jar")}},
                "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}",
                    "file": {"id": "client-1.21.2.xml", "sha1": sha1(&logging), "size": logging.len(),
                        "url": format!("{base}/mojang/logging/client-1.21.2.xml")}, "type": "log4j2-xml"}},
                "libraries": [
                    {"downloads": {"artifact": {"path": "com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
                        "sha1": sha1(&library), "size": library.len(),
                        "url": format!("{base}/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar")}},
                        "name": "com.mojang:brigadier:1.0.18"},
                    {"downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar",
                        "sha1": sha1(&natives), "size": natives.len(),
                        "url": format!("{base}/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar")}},
                        "name": "org.lwjgl:lwjgl:3.4.1:natives-windows"}
                ],
                "arguments": {"game": ["--username", "${auth_player_name}"], "jvm": ["-Djava.library.path=${natives_directory}"]}
            })
            .to_string();
            self.insert(
                "/mc/game/version_manifest_v2.json",
                serde_json::json!({
                    "latest": {"release": LOADER_MC, "snapshot": LOADER_MC},
                    "versions": [{"id": LOADER_MC, "type": "release",
                        "url": format!("{base}/mojang/versions/{LOADER_MC}.json"),
                        "sha1": sha1(version_document.as_bytes())}]
                })
                .to_string(),
            );
            self.insert(
                format!("/mojang/versions/{LOADER_MC}.json"),
                version_document,
            );

            // Fabric Meta and Maven.
            let fabric_library = b"synthetic digested fabric library jar".to_vec();
            let loader_entry = serde_json::json!({"separator": ".", "build": 5,
                "maven": format!("net.fabricmc:fabric-loader:{FABRIC_LOADER}"), "version": FABRIC_LOADER, "stable": true});
            let launcher_meta = |libraries: serde_json::Value| {
                serde_json::json!({"version": 2, "min_java_version": 8,
                    "libraries": {"client": [], "common": libraries, "server": [], "development": []},
                    "mainClass": {"client": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                        "server": "net.fabricmc.loader.impl.launch.knot.KnotServer"}})
            };
            let intermediary = serde_json::json!({"maven": "net.fabricmc:intermediary:0.0.0", "version": "0.0.0", "stable": true});
            self.insert(
                "/v2/versions/loader",
                serde_json::json!([loader_entry]).to_string(),
            );
            self.insert(
                format!("/v2/versions/loader/{LOADER_MC}"),
                serde_json::json!([{"loader": loader_entry, "intermediary": intermediary,
                    "launcherMeta": launcher_meta(serde_json::json!([]))}])
                .to_string(),
            );
            self.insert(
                format!("/v2/versions/loader/{LOADER_MC}/{FABRIC_LOADER}"),
                serde_json::json!({"loader": loader_entry, "intermediary": intermediary,
                    "launcherMeta": launcher_meta(serde_json::json!([{
                        "name": "org.ow2.asm:asm:9.10.1", "url": format!("{base}/fabric-maven/"),
                        "sha256": format!("{:x}", Sha256::digest(&fabric_library)), "size": fabric_library.len()}]))})
                .to_string(),
            );
            self.insert(
                "/fabric-maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar",
                fabric_library,
            );
            self.insert(
                format!(
                    "/fabric-maven/net/fabricmc/fabric-loader/{FABRIC_LOADER}/fabric-loader-{FABRIC_LOADER}.jar"
                ),
                b"synthetic digest-less fabric loader jar".to_vec(),
            );
            self.insert(
                "/fabric-maven/net/fabricmc/intermediary/0.0.0/intermediary-0.0.0.jar",
                b"synthetic intermediary jar".to_vec(),
            );

            // Forge: listing, promotions, and a processor-free installer.
            let forge_pin = format!("{LOADER_MC}-{FORGE_BUILD}");
            let forge_id = format!("{LOADER_MC}-forge-{FORGE_BUILD}");
            self.insert(
                "/forge-maven/net/minecraftforge/forge/maven-metadata.xml",
                format!(
                    "<metadata><versioning><versions><version>{forge_pin}</version></versions></versioning></metadata>"
                ),
            );
            self.insert(
                "/forge-promotions.json",
                serde_json::json!({"promos": {format!("{LOADER_MC}-recommended"): FORGE_BUILD}})
                    .to_string(),
            );
            let forge_installer = zip_of(&[
                (
                    "install_profile.json",
                    serde_json::json!({"spec": 1, "profile": "forge", "version": forge_id,
                        "minecraft": LOADER_MC, "json": "/version.json", "data": {},
                        "processors": [], "libraries": []})
                    .to_string()
                    .as_bytes(),
                ),
                (
                    "version.json",
                    serde_json::json!({"id": forge_id, "inheritsFrom": LOADER_MC,
                        "mainClass": "net.minecraftforge.bootstrap.ForgeBootstrap", "logging": {},
                        "arguments": {"game": ["--launchTarget", "forge_client"], "jvm": []},
                        "libraries": []})
                    .to_string()
                    .as_bytes(),
                ),
            ]);
            let installer_path = format!(
                "/forge-maven/net/minecraftforge/forge/{forge_pin}/forge-{forge_pin}-installer.jar"
            );
            self.insert(format!("{installer_path}.sha1"), sha1(&forge_installer));
            self.insert(installer_path, forge_installer);

            // NeoForge: listing, processor-free installer, universal jar.
            self.insert(
                "/api/maven/versions/releases/net/neoforged/neoforge",
                serde_json::json!({"isSnapshot": false, "versions": ["21.1.77", NEOFORGE_VERSION]})
                    .to_string(),
            );
            let neo_id = format!("neoforge-{NEOFORGE_VERSION}");
            let neo_installer = zip_of(&[
                (
                    "install_profile.json",
                    serde_json::json!({"spec": 1, "profile": "NeoForge", "version": neo_id,
                        "minecraft": LOADER_MC, "json": "/version.json", "data": {},
                        "processors": [], "libraries": []})
                    .to_string()
                    .as_bytes(),
                ),
                (
                    "version.json",
                    serde_json::json!({"id": neo_id, "inheritsFrom": LOADER_MC,
                        "mainClass": "net.neoforged.fml.startup.Client",
                        "arguments": {"game": ["--fml.neoForgeVersion", NEOFORGE_VERSION,
                            "--fml.mcVersion", LOADER_MC], "jvm": []},
                        "libraries": []})
                    .to_string()
                    .as_bytes(),
                ),
            ]);
            let neo_root = format!("/releases/net/neoforged/neoforge/{NEOFORGE_VERSION}");
            let universal = b"synthetic neoforge universal jar".to_vec();
            self.insert(
                format!("{neo_root}/{neo_id}-installer.jar.sha1"),
                sha1(&neo_installer),
            );
            self.insert(format!("{neo_root}/{neo_id}-installer.jar"), neo_installer);
            self.insert(
                format!("{neo_root}/{neo_id}-universal.jar.sha1"),
                sha1(&universal),
            );
            self.insert(format!("{neo_root}/{neo_id}-universal.jar"), universal);
        }

        /// Publishes one pack version: its archive, its Modrinth version
        /// document, and every recognized component's identity and bytes.
        pub(crate) fn publish(
            &self,
            version_id: &str,
            metadata_loaders: &[&str],
            dependencies: serde_json::Value,
            components: &[ComponentFixture],
            externals: &[(&str, Vec<u8>)],
            overrides: &[(&str, &[u8])],
        ) {
            let base = &self.base;
            let mut files = Vec::new();
            for component in components {
                files.push(serde_json::json!({
                    "path": format!("mods/{}", component.file_name),
                    "hashes": {"sha1": sha1_hex(&component.bytes), "sha512": sha512_hex(&component.bytes)},
                    "downloads": [format!("{base}/files/{}/{}", component.project, component.file_name)],
                    "fileSize": component.bytes.len(),
                    "env": {"client": "required", "server": "required"},
                }));
            }
            for (path, bytes) in externals {
                files.push(serde_json::json!({
                    "path": path,
                    "hashes": {"sha1": sha1_hex(bytes), "sha512": sha512_hex(bytes)},
                    "downloads": [format!("{base}/external/{}", sha1_hex(bytes))],
                    "fileSize": bytes.len(),
                }));
                self.insert(format!("/external/{}", sha1_hex(bytes)), bytes.clone());
            }
            let index = serde_json::json!({
                "formatVersion": 1, "game": "minecraft", "versionId": format!("{version_id}-index"),
                "name": "Loader Pack", "files": files, "dependencies": dependencies,
            })
            .to_string();
            let mut entries: Vec<(String, Vec<u8>)> =
                vec![("modrinth.index.json".into(), index.into_bytes())];
            for (name, bytes) in overrides {
                entries.push((format!("overrides/{name}"), bytes.to_vec()));
            }
            let borrowed: Vec<(&str, &[u8])> = entries
                .iter()
                .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
                .collect();
            let archive = zip_of(&borrowed);
            self.insert(
                "/v2/project/PACK0001",
                serde_json::json!({
                    "id": "PACK0001", "project_type": "modpack", "title": "Loader Pack",
                    "description": "Fixture", "license": {"id": "mit"}, "game_versions": [LOADER_MC],
                    "loaders": ["fabric", "forge", "neoforge"], "environment": ["client_only"]
                })
                .to_string(),
            );
            self.insert(
                format!("/v2/version/{version_id}"),
                serde_json::json!({
                    "id": version_id, "project_id": "PACK0001", "name": version_id, "version_number": "1",
                    "version_type": "release", "date_published": "2026-01-01T00:00:00Z",
                    "game_versions": [LOADER_MC], "loaders": metadata_loaders,
                    "environment": "client_only", "dependencies": [],
                    "files": [{"hashes": {"sha512": sha512_hex(&archive)},
                        "url": format!("{base}/packs/{version_id}.mrpack"),
                        "filename": "pack.mrpack", "primary": true, "size": archive.len()}]
                })
                .to_string(),
            );
            self.insert(format!("/packs/{version_id}.mrpack"), archive);
            for component in components {
                self.insert(
                    format!("/v2/project/{}", component.project),
                    serde_json::json!({
                        "id": component.project, "project_type": "mod",
                        "title": format!("Component {}", component.project), "description": "d",
                        "license": {"id": "mit"}, "game_versions": [LOADER_MC],
                        "loaders": component.tags, "environment": ["client_only"]
                    })
                    .to_string(),
                );
                let dependencies: Vec<_> = component
                    .requires_projects
                    .iter()
                    .map(|project| {
                        serde_json::json!({"project_id": project, "version_id": null,
                            "dependency_type": "required"})
                    })
                    .collect();
                let url = format!("{base}/files/{}/{}", component.project, component.file_name);
                let version = serde_json::json!({
                    "id": component.version, "project_id": component.project, "name": "1.0.0",
                    "version_number": "1.0.0", "version_type": "release",
                    "date_published": "2026-01-01T00:00:00Z", "game_versions": [LOADER_MC],
                    "loaders": component.tags, "environment": "client_only",
                    "dependencies": dependencies,
                    "files": [{"hashes": {"sha512": sha512_hex(&component.bytes), "sha1": sha1_hex(&component.bytes)},
                        "url": url, "filename": component.file_name, "primary": true,
                        "size": component.bytes.len()}]
                });
                self.insert(
                    format!("/v2/version/{}", component.version),
                    version.to_string(),
                );
                self.hashes
                    .lock()
                    .unwrap()
                    .insert(sha512_hex(&component.bytes), version);
                self.insert(
                    format!("/files/{}/{}", component.project, component.file_name),
                    component.bytes.clone(),
                );
            }
        }

        async fn preview(&self, version_id: &str) -> Result<PackPreview, PackInstallError> {
            preview(&self.managed, &self.client, "PACK0001", version_id, &[]).await
        }

        async fn install(&self, version_id: &str) -> Result<InstanceRecord, PackInstallError> {
            let reviewed = self.preview(version_id).await?;
            install(
                &self.managed,
                &self.endpoints,
                &self.client,
                "PACK0001",
                version_id,
                &[],
                &reviewed.fingerprint,
                &mut |_| {},
            )
            .await
        }

        fn registry(&self) -> crate::instances::InstanceRegistry {
            crate::instances::InstanceRegistry::load(&self.managed.instance_registry_file())
                .unwrap()
        }
    }

    pub(crate) fn dependencies_for(kind: PackLoaderKind) -> serde_json::Value {
        match kind {
            PackLoaderKind::Fabric => {
                serde_json::json!({"minecraft": LOADER_MC, "fabric-loader": FABRIC_LOADER})
            }
            PackLoaderKind::Forge => {
                serde_json::json!({"minecraft": LOADER_MC, "forge": FORGE_BUILD})
            }
            PackLoaderKind::NeoForge => {
                serde_json::json!({"minecraft": LOADER_MC, "neoforge": NEOFORGE_VERSION})
            }
        }
    }

    pub(crate) fn tag_for(kind: PackLoaderKind) -> &'static str {
        match kind {
            PackLoaderKind::Fabric => "fabric",
            PackLoaderKind::Forge => "forge",
            PackLoaderKind::NeoForge => "neoforge",
        }
    }

    #[tokio::test]
    async fn fabric_forge_and_neoforge_packs_install_as_ready_instances_with_schema_three_state() {
        for (kind, pin) in [
            (
                PackLoaderKind::Fabric,
                PlatformPin::Fabric {
                    version: FABRIC_LOADER.into(),
                },
            ),
            (
                PackLoaderKind::Forge,
                PlatformPin::Forge {
                    version: format!("{LOADER_MC}-{FORGE_BUILD}"),
                },
            ),
            (
                PackLoaderKind::NeoForge,
                PlatformPin::NeoForge {
                    version: NEOFORGE_VERSION.into(),
                },
            ),
        ] {
            let world = LoaderWorld::new(tag_for(kind));
            let components = [
                ComponentFixture::new(1, kind, &[tag_for(kind)]),
                ComponentFixture::new(2, kind, &[tag_for(kind)]),
            ];
            let external = loader_mod_jar(kind, "external_mod", None);
            world.publish(
                "VERS0001",
                &[tag_for(kind)],
                dependencies_for(kind),
                &components,
                &[("mods/external.jar", external.clone())],
                &[("config/pack.toml", b"pack = true\n")],
            );
            let reviewed = world.preview("VERS0001").await.unwrap();
            assert_eq!(reviewed.loader, kind);
            assert_eq!(
                reviewed.loader_version,
                match kind {
                    PackLoaderKind::Fabric => FABRIC_LOADER,
                    PackLoaderKind::Forge => FORGE_BUILD,
                    PackLoaderKind::NeoForge => NEOFORGE_VERSION,
                }
            );
            assert_eq!(reviewed.recognized.len(), 2);
            let record = world
                .install("VERS0001")
                .await
                .unwrap_or_else(|error| panic!("{kind:?}: {} ({})", error.message, error.code));
            assert_eq!(
                record.state(),
                crate::instances::InstanceState::Ready,
                "{kind:?}"
            );
            let registry = world.registry();
            let stored = registry.find(record.id()).unwrap();
            assert_eq!(stored.state(), crate::instances::InstanceState::Ready);
            assert_eq!(stored.installed().platform, pin, "{kind:?}");
            assert!(stored.installed().aurora.is_none());
            assert_eq!(stored.pack().unwrap().project_id, "PACK0001");
            let root = world
                .managed
                .instance_paths(record.id())
                .root()
                .to_path_buf();
            let document: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(root.join("pack-installed.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(document["schemaVersion"], 3);
            assert_eq!(
                document["identity"]["loader"],
                serde_json::to_value(&pin).unwrap()
            );
            let state = InstalledPack::load(&world.managed, record.id())
                .unwrap()
                .unwrap();
            assert_eq!(state.identity.loader, pin);
            state.validate_installed(&world.managed).unwrap();
            assert_eq!(state.components.len(), 3);
            for component in &components {
                assert_eq!(
                    std::fs::read(root.join("mods").join(&component.file_name)).unwrap(),
                    component.bytes
                );
            }
            assert_eq!(
                std::fs::read(root.join("mods/external.jar")).unwrap(),
                external
            );
            let content =
                crate::instance_content::ContentState::load(&world.managed, record.id()).unwrap();
            assert!(
                content
                    .entries
                    .iter()
                    .all(|entry| entry.compatibility.loader.as_deref() == Some(tag_for(kind)))
            );
            assert_eq!(content.entries.len(), 2);
        }
    }

    #[tokio::test]
    async fn component_loader_tags_follow_the_individual_mod_rule_of_the_pack_family() {
        // (pack loader, component tags, accepted)
        let cases = [
            (PackLoaderKind::Forge, vec!["forge"], true),
            (PackLoaderKind::Forge, vec!["fabric"], false),
            (PackLoaderKind::NeoForge, vec!["neoforge"], true),
            // A NeoForge instance accepts only `neoforge`-tagged mods, never
            // `forge`-only ones (`compatible_browse` for BrowseKind::Mod).
            (PackLoaderKind::NeoForge, vec!["forge"], false),
            (PackLoaderKind::Fabric, vec!["forge", "neoforge"], false),
        ];
        for (index, (kind, tags, accepted)) in cases.into_iter().enumerate() {
            let world = LoaderWorld::new(&format!("tags-{index}"));
            world.publish(
                "VERS0001",
                &[tag_for(kind)],
                dependencies_for(kind),
                &[ComponentFixture::new(index, kind, &tags)],
                &[],
                &[],
            );
            let result = world.preview("VERS0001").await;
            if accepted {
                assert_eq!(result.unwrap().recognized.len(), 1, "{kind:?} {tags:?}");
            } else {
                let error = result.unwrap_err();
                assert_eq!(
                    error.code, "pack_component_incompatible",
                    "{kind:?} {tags:?}"
                );
                assert!(
                    error.message.contains(&format!(
                        "incompatible with a client {} installation",
                        kind.display_name()
                    )),
                    "{error}"
                );
            }
        }
    }

    #[tokio::test]
    async fn index_loader_must_match_metadata_and_exact_builds_are_checked_before_any_instance() {
        let world = LoaderWorld::new("mismatch");
        // (version, metadata loaders, index dependencies, code, message part)
        let cases = [
            (
                "MISM0001",
                vec!["forge"],
                dependencies_for(PackLoaderKind::Fabric),
                "pack_identity_conflict",
                "Fabric metadata",
            ),
            (
                // A version listing both is decided by its index: NeoForge is
                // not among them.
                "MISM0002",
                vec!["fabric", "forge"],
                dependencies_for(PackLoaderKind::NeoForge),
                "pack_identity_conflict",
                "NeoForge metadata",
            ),
            (
                "MISM0003",
                vec!["neoforge"],
                dependencies_for(PackLoaderKind::Forge),
                "pack_identity_conflict",
                "Forge metadata",
            ),
            (
                // The NeoForge build addresses another Minecraft version.
                "NEOV0001",
                vec!["neoforge"],
                serde_json::json!({"minecraft": LOADER_MC, "neoforge": "26.1.2.112"}),
                "pack_identity_conflict",
                "for Minecraft 26.1.2",
            ),
            (
                // A combined `<minecraft>-<build>` value is not the format's
                // Forge build and is refused, not reinterpreted.
                "FORV0001",
                vec!["forge"],
                serde_json::json!({"minecraft": LOADER_MC, "forge": "26.2-63.0.1"}),
                "pack_unsupported_loader",
                "Forge version Aurora can't read",
            ),
        ];
        for (version, loaders, dependencies, code, wording) in cases {
            world.publish(version, &loaders, dependencies, &[], &[], &[]);
            let error = world.install(version).await.unwrap_err();
            assert_eq!(error.code, code, "{version}: {error}");
            assert!(error.message.contains(wording), "{version}: {error}");
            assert_eq!(world.requested(&format!("/packs/{version}.mrpack")), 1);
        }
        // Both loaders listed and the index says Forge: accepted.
        world.publish(
            "BOTH0001",
            &["fabric", "forge"],
            dependencies_for(PackLoaderKind::Forge),
            &[],
            &[],
            &[],
        );
        assert_eq!(
            world.preview("BOTH0001").await.unwrap().loader,
            PackLoaderKind::Forge
        );
        assert!(!world.managed.instance_registry_file().exists());
    }

    #[test]
    fn forge_and_neoforge_support_reuses_each_loaders_own_rules() {
        for (game, forge) in [
            ("1.12.2", false),
            ("1.13.2", false),
            ("1.14.3", false),
            ("1.14.4", true),
            ("1.15", true),
            ("1.20.1", true),
            ("26.2", true),
        ] {
            assert_eq!(forge_installs_minecraft(game), forge, "{game}");
        }
        for (game, neoforge) in [
            ("1.20.4", false),
            ("1.21.1", false),
            ("26.1.2", true),
            ("26.2", true),
        ] {
            assert_eq!(neoforge_installs_minecraft(game), neoforge, "{game}");
        }
        let plan = |minecraft: &str, kind, version: &str| PackPlan {
            name: "Pack".into(),
            pack_version: "1".into(),
            minecraft_version: minecraft.into(),
            loader: crate::mrpack::PackLoader {
                kind,
                version: version.into(),
            },
            required_files: Vec::new(),
            optional_files: Vec::new(),
            excluded_paths: Vec::new(),
            overrides: Vec::new(),
            skipped_overrides: Vec::new(),
        };
        let forge = PackLoaderKind::Forge;
        let neoforge = PackLoaderKind::NeoForge;
        assert!(loader_build_refusal(&plan("1.14.4", forge, "28.1.16")).is_none());
        assert!(loader_build_refusal(&plan("1.20.1", forge, "47.4.10")).is_none());
        let old = loader_build_refusal(&plan("1.14.4", forge, "28.1.15")).unwrap();
        assert_eq!(old.code, "pack_unsupported_loader");
        assert!(old.message.contains("1.14.4-28.1.15"), "{old}");
        assert!(loader_build_refusal(&plan(LOADER_MC, neoforge, NEOFORGE_VERSION)).is_none());
        let generation = loader_build_refusal(&plan("1.21.1", neoforge, "21.1.77")).unwrap();
        assert_eq!(generation.code, "pack_unsupported_loader");
        assert!(generation.message.contains(NEOFORGE_MINECRAFT_RANGE));
        assert_eq!(
            loader_build_refusal(&plan(LOADER_MC, neoforge, "bad version"))
                .unwrap()
                .code,
            "pack_unsupported_loader"
        );
    }

    #[tokio::test]
    async fn dependency_confirmation_reads_forge_and_neoforge_jars_with_their_own_readers() {
        for kind in [PackLoaderKind::Forge, PackLoaderKind::NeoForge] {
            // Modrinth names a required project the pack lacks; the jar's own
            // metadata needs only its loader's built-ins, so it is confirmed.
            let world = LoaderWorld::new(&format!("confirm-{}", tag_for(kind)));
            let mut bundled = ComponentFixture::new(1, kind, &[tag_for(kind)]);
            bundled.requires_projects = vec!["ABSENT01".into()];
            world.publish(
                "VERS0001",
                &[tag_for(kind)],
                dependencies_for(kind),
                &[bundled],
                &[],
                &[],
            );
            world.preview("VERS0001").await.unwrap_or_else(|error| {
                panic!(
                    "{kind:?} confirmation failed: {} ({})",
                    error.message, error.code
                )
            });
            // When the jar's own metadata really needs the absent mod, the
            // pack is refused with that loader's dependency verdict.
            let mut missing = ComponentFixture::new(2, kind, &[tag_for(kind)]);
            missing.bytes = loader_mod_jar(kind, "component_2", Some("absent_mod"));
            missing.requires_projects = vec!["ABSENT01".into()];
            world.publish(
                "VERS0002",
                &[tag_for(kind)],
                dependencies_for(kind),
                &[missing],
                &[],
                &[],
            );
            let error = world.preview("VERS0002").await.unwrap_err();
            assert_eq!(error.code, "pack_dependency_conflict", "{kind:?}: {error}");
            assert!(error.message.contains("component-2.jar"), "{error}");
            // A jar without that loader's metadata is not read as Fabric.
            let mut unreadable = ComponentFixture::new(3, kind, &[tag_for(kind)]);
            unreadable.bytes = loader_mod_jar(PackLoaderKind::Fabric, "component_3", None);
            unreadable.requires_projects = vec!["ABSENT01".into()];
            world.publish(
                "VERS0003",
                &[tag_for(kind)],
                dependencies_for(kind),
                &[unreadable],
                &[],
                &[],
            );
            let error = world.preview("VERS0003").await.unwrap_err();
            assert_eq!(error.code, "pack_dependency_conflict");
            assert!(
                error.message.contains(&format!(
                    "its {} metadata could not be read",
                    kind.display_name()
                )),
                "{error}"
            );
        }
    }

    #[tokio::test]
    async fn forge_family_pack_updates_and_cross_loader_merges_are_refused_clearly() {
        let world = LoaderWorld::new("forge-scope");
        let kind = PackLoaderKind::Forge;
        world.publish(
            "VERS0001",
            &["forge"],
            dependencies_for(kind),
            &[ComponentFixture::new(1, kind, &["forge"])],
            &[],
            &[],
        );
        let record = world.install("VERS0001").await.unwrap();
        for result in [
            crate::pack_update::check_pack_update(
                &world.managed,
                &world.client,
                record.id().as_str(),
            )
            .await
            .map(|_| ()),
            crate::pack_update::build_plan(
                &world.managed,
                &world.endpoints,
                &world.client,
                record.id().as_str(),
                &[],
                false,
            )
            .await
            .map(|_| ()),
        ] {
            let error = result.unwrap_err();
            assert_eq!(error.code, "pack_update_unsupported_loader");
            assert_eq!(
                error.message,
                "Updating Forge/NeoForge modpacks isn't supported yet."
            );
        }
        // Merging a Forge pack into an ordinary Fabric instance is refused
        // from the version metadata, before the archive is downloaded.
        let mut configuration = InstanceConfiguration::from_parts(
            LOADER_MC,
            LoaderConfiguration::Fabric {
                policy: LoaderPolicy::Pinned {
                    version: FABRIC_LOADER.into(),
                },
            },
            DEFAULT_MEMORY_MIB,
            String::new(),
            None,
        );
        configuration.set_aurora_enabled(false);
        let fabric = crate::instances::lifecycle::create_instance(
            &world.managed,
            &world.managed.instance_registry_file(),
            &world.managed.config_file(),
            &world.endpoints,
            CreateInstanceRequest::new("Plain Fabric", configuration),
            &mut |_| {},
            crate::instances::lifecycle::InstanceFaults::default(),
        )
        .await
        .unwrap();
        let archives = world.requested("/packs/VERS0001.mrpack");
        let error = crate::pack_merge::preview(
            &world.managed,
            &world.client,
            fabric.id(),
            "PACK0001",
            "VERS0001",
            &[],
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "pack_merge_incompatible");
        assert_eq!(
            error.message,
            "This Modrinth version is a Forge modpack; this instance uses Fabric."
        );
        assert_eq!(world.requested("/packs/VERS0001.mrpack"), archives);
    }

    /// Evidence for `MAX_NEW_PACK_PROVIDER_PLANS`: exactly that many
    /// recognized components (far above the ordinary 64 bound) activate in
    /// one new pack instance, and a fault after half of them were activated
    /// rolls every created file back while the Installing record stays.
    #[tokio::test]
    async fn a_new_pack_at_the_component_bound_installs_and_a_mid_activation_fault_rolls_back() {
        const COUNT: usize = crate::instance_content::MAX_NEW_PACK_PROVIDER_PLANS;
        const { assert!(COUNT > crate::instance_content::MAX_PROVIDER_PLANS_PER_OPERATION) };
        let components: Vec<_> = (0..COUNT)
            .map(|index| ComponentFixture::new(index, PackLoaderKind::Fabric, &["fabric"]))
            .collect();
        let kind = PackLoaderKind::Fabric;

        // A fault after 150 activated files: everything created is removed.
        let failing = LoaderWorld::new("large-fault");
        failing.publish(
            "VERS0001",
            &["fabric"],
            dependencies_for(kind),
            &components,
            &[(
                "mods/external.jar",
                loader_mod_jar(kind, "external_mod", None),
            )],
            &[("config/pack.toml", b"pack = true\n")],
        );
        crate::instance_content::activation_fault::fail_after(Some(COUNT / 2));
        let started = std::time::Instant::now();
        let error = failing.install("VERS0001").await.unwrap_err();
        crate::instance_content::activation_fault::fail_after(None);
        eprintln!(
            "{COUNT}-component install with a mid-activation fault: {:?}",
            started.elapsed()
        );
        assert_eq!(error.code, "pack_component_install_failed", "{error}");
        // The fault fired inside the copy loop, after 150 files existed.
        assert!(
            error.message.contains("deterministic activation fault"),
            "{error}"
        );
        let registry = failing.registry();
        let [record] = registry.instances() else {
            panic!("exactly one instance record");
        };
        assert_eq!(record.state(), crate::instances::InstanceState::Installing);
        let root = failing
            .managed
            .instance_paths(record.id())
            .root()
            .to_path_buf();
        let leftovers: Vec<_> = std::fs::read_dir(root.join("mods"))
            .map(|entries| entries.flatten().map(|entry| entry.file_name()).collect())
            .unwrap_or_default();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        assert!(!root.join("content-managed.json").exists());
        assert!(!root.join("pack-installed.json").exists());
        assert!(!root.join("config/pack.toml").exists());

        // Without the fault the same pack becomes a Ready instance.
        let world = LoaderWorld::new("large");
        world.publish(
            "VERS0001",
            &["fabric"],
            dependencies_for(kind),
            &components,
            &[(
                "mods/external.jar",
                loader_mod_jar(kind, "external_mod", None),
            )],
            &[("config/pack.toml", b"pack = true\n")],
        );
        let started = std::time::Instant::now();
        let record = world
            .install("VERS0001")
            .await
            .unwrap_or_else(|error| panic!("{} ({})", error.message, error.code));
        eprintln!(
            "{COUNT}-component new pack instance: {:?}",
            started.elapsed()
        );
        assert_eq!(record.state(), crate::instances::InstanceState::Ready);
        let content =
            crate::instance_content::ContentState::load(&world.managed, record.id()).unwrap();
        assert_eq!(content.entries.len(), COUNT);
        let state = InstalledPack::load(&world.managed, record.id())
            .unwrap()
            .unwrap();
        assert_eq!(state.components.len(), COUNT + 1);
        state.validate_installed(&world.managed).unwrap();
    }

    #[tokio::test]
    async fn a_forge_pack_failing_after_instance_creation_keeps_its_installing_record() {
        let world = LoaderWorld::new("forge-failure");
        // Modrinth tags the component for Forge, but the jar carries no
        // Forge metadata, so activation refuses it after the game installs.
        let mut component = ComponentFixture::new(1, PackLoaderKind::Forge, &["forge"]);
        component.bytes = loader_mod_jar(PackLoaderKind::Fabric, "component_1", None);
        world.publish(
            "VERS0001",
            &["forge"],
            dependencies_for(PackLoaderKind::Forge),
            &[component],
            &[(
                "mods/external.jar",
                loader_mod_jar(PackLoaderKind::Forge, "external_mod", None),
            )],
            &[("config/pack.toml", b"pack = true\n")],
        );
        let error = world.install("VERS0001").await.unwrap_err();
        assert_eq!(error.code, "pack_component_install_failed", "{error}");
        let registry = world.registry();
        let [record] = registry.instances() else {
            panic!("exactly one instance record");
        };
        assert_eq!(record.state(), crate::instances::InstanceState::Installing);
        assert_eq!(
            record.installed().platform,
            PlatformPin::Forge {
                version: format!("{LOADER_MC}-{FORGE_BUILD}"),
            }
        );
        // The registry keeps the pack identity Phase 2b resumes from.
        assert_eq!(record.pack().unwrap().version_id, "VERS0001");
        let root = world
            .managed
            .instance_paths(record.id())
            .root()
            .to_path_buf();
        // Nothing claims a complete pack: no pack state, no provider state,
        // no pack-written files; the proven game stays for retry.
        assert!(!root.join("pack-installed.json").exists());
        assert!(!root.join("content-managed.json").exists());
        assert!(!root.join("mods/component-1.jar").exists());
        assert!(!root.join("mods/external.jar").exists());
        assert!(!root.join("config/pack.toml").exists());
        assert!(matches!(
            crate::install::validate_installed_game(&world.managed, record.id()),
            Ok(crate::install::ValidationOutcome::Installed(_))
        ));
    }

    // ------------------------------------------------ Phase 2b: resume

    fn pending_path(world: &LoaderWorld, id: &InstanceId) -> PathBuf {
        world.managed.instance_paths(id).root().join(PENDING_FILE)
    }

    /// The one registry record, which must still be Installing with its
    /// resume intent on disk and no pack state.
    fn kept_installing(world: &LoaderWorld, error: &PackInstallError) -> InstanceRecord {
        let registry = world.registry();
        let [record] = registry.instances() else {
            panic!("exactly one instance record: {error}");
        };
        assert_eq!(
            record.state(),
            crate::instances::InstanceState::Installing,
            "{error}"
        );
        assert_eq!(error.instance_id.as_ref(), Some(record.id()), "{error}");
        assert!(
            error.message.contains(&format!(
                "The instance '{}' was kept",
                record.display_name()
            )),
            "{error}"
        );
        assert!(pending_path(world, record.id()).is_file(), "{error}");
        assert!(
            !world
                .managed
                .instance_paths(record.id())
                .root()
                .join("pack-installed.json")
                .exists()
        );
        record.clone()
    }

    async fn resume_in(
        world: &LoaderWorld,
        id: &InstanceId,
    ) -> Result<InstanceRecord, PackInstallError> {
        resume(
            &world.managed,
            &world.endpoints,
            &world.client,
            id,
            &mut |_| {},
        )
        .await
    }

    /// The same instance id is Ready with valid schema-3 pack state, the
    /// registry holds exactly one instance, and the intent is gone.
    fn assert_resumed(world: &LoaderWorld, id: &InstanceId, record: &InstanceRecord) {
        assert_eq!(record.id(), id);
        assert_eq!(record.state(), crate::instances::InstanceState::Ready);
        let registry = world.registry();
        let [stored] = registry.instances() else {
            panic!("exactly one instance after resume");
        };
        assert_eq!(stored.id(), id);
        assert_eq!(stored.state(), crate::instances::InstanceState::Ready);
        let root = world.managed.instance_paths(id).root().to_path_buf();
        let document: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("pack-installed.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(document["schemaVersion"], 3);
        InstalledPack::load(&world.managed, id)
            .unwrap()
            .unwrap()
            .validate_installed(&world.managed)
            .unwrap();
        assert!(!pending_path(world, id).exists());
        assert_eq!(
            std::fs::read(root.join("config/pack.toml")).unwrap(),
            b"pack = true\n"
        );
    }

    fn publish_standard(world: &LoaderWorld, kind: PackLoaderKind) {
        world.publish(
            "VERS0001",
            &[tag_for(kind)],
            dependencies_for(kind),
            &[ComponentFixture::new(1, kind, &[tag_for(kind)])],
            &[(
                "mods/external.jar",
                loader_mod_jar(kind, "external_mod", None),
            )],
            &[("config/pack.toml", b"pack = true\n")],
        );
    }

    #[test]
    fn the_resume_intent_round_trips_and_damaged_or_unknown_documents_are_refused_and_kept() {
        let (root, managed) = temporary_managed("pending-document");
        let instance = InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        let instance_root = managed.instance_paths(&instance).root().to_path_buf();
        std::fs::create_dir_all(&instance_root).unwrap();
        let document = PendingInstall {
            schema_version: PENDING_SCHEMA_VERSION,
            instance_id: instance.clone(),
            provider: "modrinth".into(),
            project_id: "PACK0001".into(),
            version_id: "VERS0001".into(),
            archive_sha512: "a".repeat(128),
            selected_optional: vec!["mods/optional.jar".into()],
            fingerprint: "b".repeat(64),
        };
        assert!(PendingInstall::load(&managed, &instance).unwrap().is_none());
        document.save_new(&managed).unwrap();
        assert_eq!(
            PendingInstall::load(&managed, &instance).unwrap(),
            Some(document.clone())
        );
        let written: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(instance_root.join(PENDING_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(written["schemaVersion"], 1);
        assert_eq!(written["archiveSha512"], "a".repeat(128));
        assert_eq!(written["selectedOptional"][0], "mods/optional.jar");
        // A second write never replaces the existing document.
        assert_eq!(
            document.save_new(&managed).unwrap_err().code,
            "pack_resume_state_invalid"
        );
        // Invalid content is refused before anything is written.
        let other = InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        std::fs::create_dir_all(managed.instance_paths(&other).root()).unwrap();
        let mut invalid = document.clone();
        invalid.instance_id = other.clone();
        invalid.archive_sha512 = "short".into();
        assert!(invalid.save_new(&managed).is_err());
        assert!(
            !managed
                .instance_paths(&other)
                .root()
                .join(PENDING_FILE)
                .exists()
        );

        let mut future = written.clone();
        future["schemaVersion"] = 2.into();
        let mut extra = written.clone();
        extra["unknown"] = true.into();
        let mut foreign = written.clone();
        foreign["instanceId"] = other.as_str().into();
        let mut traversal = written.clone();
        traversal["selectedOptional"] = serde_json::json!(["../escape.jar"]);
        for (label, bytes) in [
            ("future schema", future.to_string().into_bytes()),
            ("unknown field", extra.to_string().into_bytes()),
            ("other instance", foreign.to_string().into_bytes()),
            ("unsafe optional path", traversal.to_string().into_bytes()),
            ("malformed", b"{ not json".to_vec()),
            ("no schema", b"{}".to_vec()),
        ] {
            std::fs::write(instance_root.join(PENDING_FILE), &bytes).unwrap();
            let error = PendingInstall::load(&managed, &instance).unwrap_err();
            assert_eq!(error.code, "pack_resume_state_invalid", "{label}");
            if label == "future schema" {
                assert!(error.message.contains("format"), "{error}");
            }
            assert!(document.save_new(&managed).is_err(), "{label}");
            // Removal after Ready never deletes a document it didn't write.
            document.remove_completed(&managed);
            assert_eq!(
                std::fs::read(instance_root.join(PENDING_FILE)).unwrap(),
                bytes,
                "{label}: never overwritten or removed"
            );
        }
        std::fs::write(
            instance_root.join(PENDING_FILE),
            serde_json::to_vec(&document).unwrap(),
        )
        .unwrap();
        document.remove_completed(&managed);
        assert!(!instance_root.join(PENDING_FILE).exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn a_pack_whose_game_install_failed_resumes_the_same_instance() {
        for kind in [PackLoaderKind::Fabric, PackLoaderKind::NeoForge] {
            let world = LoaderWorld::new(&format!("resume-game-{}", tag_for(kind)));
            publish_standard(&world, kind);
            let client = world.take("/mojang/client.jar");
            let error = world.install("VERS0001").await.unwrap_err();
            assert_eq!(error.code, "pack_instance_failed", "{kind:?}: {error}");
            let kept = kept_installing(&world, &error);
            let root = world.managed.instance_paths(kept.id()).root().to_path_buf();
            assert!(!root.join("mods/external.jar").exists());
            // Still failing: the instance stays exactly as it was.
            let again = resume_in(&world, kept.id()).await.unwrap_err();
            assert_eq!(again.instance_id.as_ref(), Some(kept.id()));
            kept_installing(&world, &again);
            world.insert("/mojang/client.jar", client);
            let record = resume_in(&world, kept.id())
                .await
                .unwrap_or_else(|error| panic!("{kind:?}: {} ({})", error.message, error.code));
            assert_resumed(&world, kept.id(), &record);
            assert_eq!(
                world.registry().instances()[0].installed().platform,
                kept.installed().platform
            );
        }
    }

    #[tokio::test]
    async fn a_pack_whose_file_materialization_failed_resumes_the_same_instance() {
        let world = LoaderWorld::new("resume-materialize");
        publish_standard(&world, PackLoaderKind::Fabric);
        // Fails after the external file was written, before the override.
        materialize_fault::fail_at(Some(1));
        let error = world.install("VERS0001").await.unwrap_err();
        materialize_fault::fail_at(None);
        assert_eq!(error.code, "pack_io_error", "{error}");
        let kept = kept_installing(&world, &error);
        let root = world.managed.instance_paths(kept.id()).root().to_path_buf();
        assert!(!root.join("mods/external.jar").exists(), "rolled back");
        let record = resume_in(&world, kept.id()).await.unwrap();
        assert_resumed(&world, kept.id(), &record);
    }

    #[tokio::test]
    async fn a_pack_whose_component_activation_failed_resumes_the_same_instance() {
        for kind in [PackLoaderKind::Fabric, PackLoaderKind::Forge] {
            let world = LoaderWorld::new(&format!("resume-activation-{}", tag_for(kind)));
            publish_standard(&world, kind);
            crate::instance_content::activation_fault::fail_after(Some(0));
            let error = world.install("VERS0001").await.unwrap_err();
            crate::instance_content::activation_fault::fail_after(None);
            assert_eq!(error.code, "pack_component_install_failed", "{error}");
            let kept = kept_installing(&world, &error);
            let record = resume_in(&world, kept.id())
                .await
                .unwrap_or_else(|error| panic!("{kind:?}: {} ({})", error.message, error.code));
            assert_resumed(&world, kept.id(), &record);
            let content =
                crate::instance_content::ContentState::load(&world.managed, kept.id()).unwrap();
            assert_eq!(content.entries.len(), 1);
        }
    }

    #[tokio::test]
    async fn cancellation_before_creation_leaves_nothing_and_after_creation_is_resumable() {
        let world = LoaderWorld::new("resume-cancel");
        publish_standard(&world, PackLoaderKind::Fabric);
        let reviewed = world.preview("VERS0001").await.unwrap();
        let attempt = |cancel_at: &'static str| {
            let control = crate::content_jobs::Control::default();
            let canceller = control.clone();
            let world = &world;
            let fingerprint = reviewed.fingerprint.clone();
            async move {
                control
                    .run(install(
                        &world.managed,
                        &world.endpoints,
                        &world.client,
                        "PACK0001",
                        "VERS0001",
                        &[],
                        &fingerprint,
                        &mut |phase| {
                            if phase == cancel_at {
                                canceller.cancel();
                            }
                        },
                    ))
                    .await
            }
        };
        let error = attempt(progress_phase::DOWNLOADING).await.unwrap_err();
        assert_eq!(error.code, "installation_cancelled");
        assert!(error.instance_id.is_none());
        assert!(!world.managed.instance_registry_file().exists());

        // Cancelled while the game installs: observed at the commit window,
        // before any pack file is written.
        let error = attempt(progress_phase::INSTALLING_GAME).await.unwrap_err();
        assert_eq!(error.code, "installation_cancelled", "{error}");
        assert!(error.message.contains("can be resumed"), "{error}");
        let kept = kept_installing(&world, &error);
        let root = world.managed.instance_paths(kept.id()).root().to_path_buf();
        assert!(!root.join("mods/external.jar").exists());
        assert!(!root.join("config/pack.toml").exists());
        let record = resume_in(&world, kept.id()).await.unwrap();
        assert_resumed(&world, kept.id(), &record);
    }

    #[tokio::test]
    async fn resume_refuses_changed_versions_missing_intent_foreign_files_and_ready_instances() {
        let world = LoaderWorld::new("resume-refusals");
        let kind = PackLoaderKind::Fabric;
        publish_standard(&world, kind);
        crate::instance_content::activation_fault::fail_after(Some(0));
        let error = world.install("VERS0001").await.unwrap_err();
        crate::instance_content::activation_fault::fail_after(None);
        let kept = kept_installing(&world, &error);
        let id = kept.id().clone();
        let root = world.managed.instance_paths(&id).root().to_path_buf();
        let intent = std::fs::read(pending_path(&world, &id)).unwrap();
        let unchanged = |world: &LoaderWorld, error: &PackInstallError, code: &str| {
            assert_eq!(error.code, code, "{error}");
            assert_eq!(error.instance_id.as_ref(), Some(&id));
            let registry = world.registry();
            assert_eq!(registry.instances().len(), 1);
            assert_eq!(
                registry.instances()[0].state(),
                crate::instances::InstanceState::Installing
            );
            assert!(!root.join("pack-installed.json").exists());
            assert!(!root.join("mods/external.jar").exists());
        };

        // A foreign file at a pack destination is never overwritten.
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/pack.toml"), b"owner edit\n").unwrap();
        let error = resume_in(&world, &id).await.unwrap_err();
        unchanged(&world, &error, "pack_resume_conflict");
        assert!(error.message.contains("config/pack.toml"), "{error}");
        assert_eq!(
            std::fs::read(root.join("config/pack.toml")).unwrap(),
            b"owner edit\n"
        );
        std::fs::remove_file(root.join("config/pack.toml")).unwrap();

        // Damaged component records are refused, not wiped.
        std::fs::write(root.join("content-managed.json"), b"{ damaged").unwrap();
        let error = resume_in(&world, &id).await.unwrap_err();
        unchanged(&world, &error, "pack_resume_state_invalid");
        assert_eq!(
            std::fs::read(root.join("content-managed.json")).unwrap(),
            b"{ damaged"
        );
        std::fs::remove_file(root.join("content-managed.json")).unwrap();

        // A malformed or missing intent never resumes.
        std::fs::write(pending_path(&world, &id), b"not json").unwrap();
        let error = resume_in(&world, &id).await.unwrap_err();
        unchanged(&world, &error, "pack_resume_state_invalid");
        assert_eq!(
            std::fs::read(pending_path(&world, &id)).unwrap(),
            b"not json"
        );
        std::fs::remove_file(pending_path(&world, &id)).unwrap();
        let error = resume_in(&world, &id).await.unwrap_err();
        unchanged(&world, &error, "pack_resume_unavailable");
        std::fs::write(pending_path(&world, &id), &intent).unwrap();

        // The provider now serves different archive bytes for the version.
        world.publish(
            "VERS0001",
            &[tag_for(kind)],
            dependencies_for(kind),
            &[ComponentFixture::new(1, kind, &[tag_for(kind)])],
            &[(
                "mods/external.jar",
                loader_mod_jar(kind, "external_mod", None),
            )],
            &[("config/pack.toml", b"pack = changed\n")],
        );
        let error = resume_in(&world, &id).await.unwrap_err();
        unchanged(&world, &error, "pack_resume_changed");
        assert_eq!(std::fs::read(pending_path(&world, &id)).unwrap(), intent);
        assert!(!root.join("config/pack.toml").exists());

        // The original version again: resume completes; afterwards a Ready
        // instance is refused.
        publish_standard(&world, kind);
        let record = resume_in(&world, &id).await.unwrap();
        assert_resumed(&world, &id, &record);
        let error = resume_in(&world, &id).await.unwrap_err();
        assert_eq!(error.code, "pack_resume_not_installing", "{error}");
        assert_eq!(error.instance_id.as_ref(), Some(&id));
    }

    #[tokio::test]
    async fn a_crash_after_saving_pack_state_resumes_without_downloading_and_application_retry_routes_to_resume()
     {
        let world = LoaderWorld::new("resume-crash-after-save");
        publish_standard(&world, PackLoaderKind::Fabric);
        let reviewed = world.preview("VERS0001").await.unwrap();
        let record = world.install("VERS0001").await.unwrap();
        let id = record.id().clone();
        // Interrupted between the pack-state save and the Ready commit: the
        // record is Installing and the intent is still on disk.
        let mut registry = world.registry();
        registry
            .find_mut(&id)
            .unwrap()
            .set_state(crate::instances::InstanceState::Installing);
        registry
            .save(&world.managed.instance_registry_file())
            .unwrap();
        PendingInstall {
            schema_version: PENDING_SCHEMA_VERSION,
            instance_id: id.clone(),
            provider: "modrinth".into(),
            project_id: "PACK0001".into(),
            version_id: "VERS0001".into(),
            archive_sha512: reviewed.archive_sha512.clone(),
            selected_optional: Vec::new(),
            fingerprint: reviewed.fingerprint.clone(),
        }
        .save_new(&world.managed)
        .unwrap();
        let pack_before = std::fs::read(
            world
                .managed
                .instance_paths(&id)
                .root()
                .join("pack-installed.json"),
        )
        .unwrap();
        let requests = world.total_requests();

        // The Instances-page retry: application routes a pack record to
        // resume (the provider-free lifecycle retry would also complete this
        // case, so the refusal below proves the route).
        let host = crate::host::Host::native(
            Ok(world.root.join("managed")),
            "test",
            tokio::runtime::Handle::current(),
            |_, _| {},
        );
        let retry = |id: &InstanceId| {
            crate::application::retry_instance_install(
                host.clone(),
                serde_json::from_value(serde_json::json!({ "instanceId": id.as_str() })).unwrap(),
            )
        };
        let summary = serde_json::to_value(retry(&id).await.unwrap()).unwrap();
        assert_eq!(summary["id"], id.as_str());
        assert_eq!(world.total_requests(), requests, "no network access");
        assert_eq!(
            std::fs::read(
                world
                    .managed
                    .instance_paths(&id)
                    .root()
                    .join("pack-installed.json")
            )
            .unwrap(),
            pack_before
        );
        assert_resumed(&world, &id, &world.registry().instances()[0].clone());

        // An Installing pack record without its intent: resume's own refusal,
        // carrying the instance id across the command boundary.
        let mut registry = world.registry();
        registry
            .find_mut(&id)
            .unwrap()
            .set_state(crate::instances::InstanceState::Installing);
        registry
            .save(&world.managed.instance_registry_file())
            .unwrap();
        std::fs::remove_file(
            world
                .managed
                .instance_paths(&id)
                .root()
                .join("pack-installed.json"),
        )
        .unwrap();
        let error = serde_json::to_value(retry(&id).await.unwrap_err()).unwrap();
        assert_eq!(error["code"], "pack_resume_unavailable", "{error}");
        assert_eq!(error["instanceId"], id.as_str());
        assert_eq!(world.total_requests(), requests, "no network access");
    }
}
