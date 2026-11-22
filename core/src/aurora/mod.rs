//! Aurora client-artifact installation: acquisition, managed
//! materialization, and the Aurora installed-state record.
//!
//! Aurora's client mod and any release-pinned required Fabric API mod have
//! pre-known SHA-256 digests in release metadata. Both use the Phase 2
//! verified store, never the transport-observed path for digest-less Fabric
//! loader artifacts.
//!
//! ## Managed ownership
//!
//! Materialized artifacts live at deterministic launcher-managed paths
//! inside the instance's mods directory:
//!
//! ```text
//! instances/<id>/mods/aurora-<validated aurora version>.jar
//! instances/<id>/mods/fabric-api-<validated API version>.jar  # when required
//! ```
//!
//! The filename derives from validated release metadata (never a remote
//! filename), so later update/repair logic knows exactly which file the
//! launcher owns. Every other file under `mods/` — and everything under the
//! instance's user-data areas — is user-owned: installation never
//! enumerates, moves, or removes it, and validation never treats unrelated
//! user mods as damage.
//!
//! ## Installed state
//!
//! `aurora-installed.json` (schema 1) sits at the instance root beside
//! `game/`, is written atomically, and is committed only after the
//! materialized artifact re-verifies. It deliberately does not duplicate
//! the game installed-state manifest; it records Aurora's own facts and is
//! compared against the registry's release pin and the game manifest by
//! complete-instance validation.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cache::ArtifactCache;
use crate::distribution::{AuroraRelease, ReleaseChannel};
use crate::downloads::{ArtifactSource, DownloadOptions};
use crate::instances::InstanceId;
use crate::integrity::{ArtifactDigest, ArtifactTrust, verify_file};
use crate::paths::ManagedPaths;

/// The only Aurora installed-state schema version this launcher understands.
pub const AURORA_INSTALLED_SCHEMA_VERSION: u32 = 1;

/// The installed-state file name at the instance root.
pub const AURORA_INSTALLED_FILE_NAME: &str = "aurora-installed.json";

/// The launcher-managed prefix for Aurora artifacts under the mods
/// directory, distinguishing launcher-owned files from user mods.
const MANAGED_FILE_PREFIX: &str = "aurora-";

/// The Aurora installed-state record of one instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuroraInstalledState {
    schema_version: u32,
    aurora_version: String,
    channel: ReleaseChannel,
    minecraft_version: String,
    fabric_loader_version: String,
    artifact: AuroraInstalledArtifact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fabric_api: Option<InstalledFabricApi>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ephemera: Option<InstalledEphemera>,
    installation_id: String,
    installed_at_unix_seconds: u64,
}

/// A verified Fabric API mod installed because the Aurora release requires it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFabricApi {
    version: String,
    artifact: AuroraInstalledArtifact,
}

impl InstalledFabricApi {
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn artifact(&self) -> &AuroraInstalledArtifact {
        &self.artifact
    }
}

/// A verified Ephemera rendering-engine mod managed alongside Aurora Client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledEphemera {
    version: String,
    artifact: AuroraInstalledArtifact,
}

impl InstalledEphemera {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn artifact(&self) -> &AuroraInstalledArtifact {
        &self.artifact
    }
}

/// The materialized launcher-managed artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuroraInstalledArtifact {
    /// Path relative to the instance root, forward slashes, validated safe
    /// and required to live under `mods/` with the managed prefix.
    relative_path: String,
    size_bytes: u64,
    sha256: String,
}

impl AuroraInstalledState {
    /// Construct ownership only from authoritative release expectations and
    /// the byte counts of separately verified materialized artifacts.
    pub(crate) fn for_verified_release(
        release: &AuroraRelease,
        bytes: u64,
        api_bytes: Option<u64>,
    ) -> Result<Self, AuroraInstallError> {
        Self::for_verified_release_with_provider_api(release, bytes, api_bytes, false)
    }

    pub(crate) fn for_verified_release_with_provider_api(
        release: &AuroraRelease,
        bytes: u64,
        api_bytes: Option<u64>,
        provider_api: bool,
    ) -> Result<Self, AuroraInstallError> {
        let artifact = AuroraInstalledArtifact {
            relative_path: managed_artifact_relative_path(release)?,
            size_bytes: bytes,
            sha256: release.artifact().sha256().into(),
        };
        let fabric_api = match (release.fabric_api(), api_bytes) {
            (Some(api), Some(size_bytes)) => Some(InstalledFabricApi {
                version: api.version().into(),
                artifact: AuroraInstalledArtifact {
                    relative_path: format!("mods/fabric-api-{}.jar", api.version()),
                    size_bytes,
                    sha256: api.artifact().sha256().into(),
                },
            }),
            (None, None) => None,
            (Some(_), None) if provider_api => None,
            _ => {
                return Err(AuroraInstallError::ReleaseInvalid(
                    "verified requirements do not match release".into(),
                ));
            }
        };
        let state = Self {
            schema_version: AURORA_INSTALLED_SCHEMA_VERSION,
            aurora_version: release.aurora_version().into(),
            channel: release.channel(),
            minecraft_version: release.minecraft_version().into(),
            fabric_loader_version: release.fabric_loader_version().into(),
            artifact,
            fabric_api,
            ephemera: None,
            installation_id: uuid::Uuid::new_v4().to_string(),
            installed_at_unix_seconds: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        Self::from_json(&state.to_json()).map_err(AuroraInstallError::State)
    }
    pub fn aurora_version(&self) -> &str {
        &self.aurora_version
    }

    pub fn channel(&self) -> ReleaseChannel {
        self.channel
    }

    pub fn minecraft_version(&self) -> &str {
        &self.minecraft_version
    }

    pub fn fabric_loader_version(&self) -> &str {
        &self.fabric_loader_version
    }

    pub fn artifact(&self) -> &AuroraInstalledArtifact {
        &self.artifact
    }

    pub fn fabric_api(&self) -> Option<&InstalledFabricApi> {
        self.fabric_api.as_ref()
    }

    pub fn ephemera(&self) -> Option<&InstalledEphemera> {
        self.ephemera.as_ref()
    }

    pub fn installation_id(&self) -> &str {
        &self.installation_id
    }

    /// Replaces the recorded Fabric API entry with an explicitly
    /// provider-owned one carried across an update. Crate-visible for the
    /// update transaction only; ordinary installs derive it from the
    /// release.
    pub(crate) fn set_provider_fabric_api(&mut self, fabric_api: Option<InstalledFabricApi>) {
        self.fabric_api = fabric_api;
    }

    pub(crate) fn set_ephemera(&mut self, ephemera: Option<InstalledEphemera>) {
        self.ephemera = ephemera;
    }

    /// Parses and validates an installed-state document. Unknown schema
    /// versions and malformed content fail deliberately; the caller never
    /// repairs or overwrites a damaged record.
    pub fn from_json(json: &str) -> Result<Self, AuroraStateError> {
        let state: Self =
            serde_json::from_str(json).map_err(|error| AuroraStateError::Malformed {
                reason: error.to_string(),
            })?;

        if state.schema_version != AURORA_INSTALLED_SCHEMA_VERSION {
            return Err(AuroraStateError::UnsupportedSchema {
                found: state.schema_version,
                supported: AURORA_INSTALLED_SCHEMA_VERSION,
            });
        }

        for (field, value) in [
            ("Aurora", &state.aurora_version),
            ("Minecraft", &state.minecraft_version),
            ("Fabric Loader", &state.fabric_loader_version),
        ] {
            if value.trim().is_empty() {
                return Err(AuroraStateError::Malformed {
                    reason: format!("the installed state must record a {field} version"),
                });
            }
        }
        if state.installation_id.trim().is_empty() {
            return Err(AuroraStateError::Malformed {
                reason: "the installed state must record an installation id".to_owned(),
            });
        }
        validate_managed_relative_path(&state.artifact.relative_path)
            .map_err(|reason| AuroraStateError::Malformed { reason })?;
        ArtifactDigest::parse(&state.artifact.sha256).map_err(|error| {
            AuroraStateError::Malformed {
                reason: format!("the artifact SHA-256 is invalid: {error}"),
            }
        })?;
        if state.artifact.size_bytes == 0 {
            return Err(AuroraStateError::Malformed {
                reason: "the artifact size must be greater than zero".to_owned(),
            });
        }
        if let Some(fabric_api) = &state.fabric_api {
            let expected_path = format!("mods/fabric-api-{}.jar", fabric_api.version);
            if fabric_api.version.is_empty() || fabric_api.artifact.relative_path != expected_path {
                return Err(AuroraStateError::Malformed {
                    reason: "the Fabric API managed path or version is invalid".to_owned(),
                });
            }
            validate_fabric_api_relative_path(&fabric_api.artifact.relative_path)
                .map_err(|reason| AuroraStateError::Malformed { reason })?;
            ArtifactDigest::parse(&fabric_api.artifact.sha256).map_err(|error| {
                AuroraStateError::Malformed {
                    reason: format!("the Fabric API SHA-256 is invalid: {error}"),
                }
            })?;
            if fabric_api.artifact.size_bytes == 0 {
                return Err(AuroraStateError::Malformed {
                    reason: "the Fabric API size must be greater than zero".to_owned(),
                });
            }
        }
        if let Some(ephemera) = &state.ephemera {
            let expected_path = format!("mods/ephemera-{}.jar", ephemera.version);
            if ephemera.version.is_empty() || ephemera.artifact.relative_path != expected_path {
                return Err(AuroraStateError::Malformed {
                    reason: "the Ephemera managed path or version is invalid".to_owned(),
                });
            }
            validate_ephemera_relative_path(&ephemera.artifact.relative_path)
                .map_err(|reason| AuroraStateError::Malformed { reason })?;
            ArtifactDigest::parse(&ephemera.artifact.sha256).map_err(|error| {
                AuroraStateError::Malformed {
                    reason: format!("the Ephemera SHA-256 is invalid: {error}"),
                }
            })?;
            if ephemera.artifact.size_bytes == 0 {
                return Err(AuroraStateError::Malformed {
                    reason: "the Ephemera size must be greater than zero".to_owned(),
                });
            }
        }

        Ok(state)
    }

    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .expect("Aurora installed-state serialization cannot fail");
        json.push('\n');
        json
    }

    /// The honest trust record: Aurora artifacts are verified against a
    /// pre-known expected SHA-256 from release metadata.
    pub fn trust(&self) -> ArtifactTrust {
        ArtifactTrust::ExpectedDigestVerified {
            algorithm: crate::integrity::DigestAlgorithm::Sha256,
            digest: self.artifact.sha256.clone(),
        }
    }
}

impl AuroraInstalledArtifact {
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// The managed artifact path for one release:
/// `mods/aurora-<validated version>.jar`.
///
/// The Aurora version must be a safe single filename segment (it becomes
/// part of a launcher-managed path), and the result is checked again on
/// load — belt and braces on both sides of persistence.
pub fn managed_artifact_relative_path(
    release: &AuroraRelease,
) -> Result<String, AuroraInstallError> {
    let version = release.aurora_version();
    if version.is_empty()
        || !version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
        || version.starts_with('.')
    {
        return Err(AuroraInstallError::ReleaseInvalid(format!(
            "Aurora version '{version}' cannot form a safe managed file name"
        )));
    }
    let path = format!("mods/{MANAGED_FILE_PREFIX}{version}.jar");
    validate_managed_relative_path(&path)
        .map_err(|reason| AuroraInstallError::ReleaseInvalid(reason))?;
    Ok(path)
}

/// A managed path is relative, forward-slashed, under `mods/`, and carries
/// the launcher-owned `aurora-` prefix — exactly the shape installation
/// writes and the only shape validation accepts as launcher-owned.
fn validate_managed_relative_path(path: &str) -> Result<(), String> {
    validate_managed_mod_path(path, MANAGED_FILE_PREFIX)
}

fn validate_fabric_api_relative_path(path: &str) -> Result<(), String> {
    validate_managed_mod_path(path, "fabric-api-")
}

fn validate_ephemera_relative_path(path: &str) -> Result<(), String> {
    validate_managed_mod_path(path, "ephemera-")
}

fn validate_managed_mod_path(path: &str, prefix: &str) -> Result<(), String> {
    let rest = path
        .strip_prefix("mods/")
        .ok_or("the managed artifact path must live under mods/")?;
    let file = rest
        .strip_prefix(prefix)
        .ok_or("the managed artifact path must use its launcher-owned prefix")?;
    if !file.ends_with(".jar") || file.len() < 5 {
        return Err("the managed artifact path must be a .jar file".to_owned());
    }
    if !path.split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
    }) {
        return Err(
            "the managed artifact path must be traversal-free with safe segments".to_owned(),
        );
    }
    Ok(())
}

/// The outcome of one Aurora installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledAurora {
    pub state: AuroraInstalledState,
    pub artifact_path: PathBuf,
}

/// A persisted release entry installed through the published-manifest update
/// path (`aurora-release.json`, schema 1). Launcher-owned, derived state:
/// it records the exact intentionally published release metadata an update
/// verified against, so an instance pinned to a version newer than this
/// launcher build's embedded manifest still resolves, validates, and
/// launches. Inert whenever the embedded manifest already knows the pin.
pub const INSTANCE_RELEASE_FILE_NAME: &str = "aurora-release.json";
const INSTANCE_RELEASE_SCHEMA_VERSION: u32 = 1;

/// The persisted shape of [`INSTANCE_RELEASE_FILE_NAME`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InstanceReleaseDocument {
    schema_version: u32,
    /// Provenance of the entry; only the published-manifest update path
    /// writes this document today.
    source: String,
    release: crate::distribution::AuroraRelease,
}

/// Loads the persisted installed-release entry, if present. A malformed or
/// unsupported document is a deliberate error, never overwritten.
pub fn load_instance_release(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<Option<crate::distribution::AuroraRelease>, AuroraInstallError> {
    let path = managed
        .instance_paths(instance)
        .root()
        .join(INSTANCE_RELEASE_FILE_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AuroraInstallError::StateRead(error)),
    };
    let document: InstanceReleaseDocument = serde_json::from_str(&text).map_err(|error| {
        AuroraInstallError::State(AuroraStateError::Malformed {
            reason: format!("the persisted release entry is malformed: {error}"),
        })
    })?;
    if document.schema_version != INSTANCE_RELEASE_SCHEMA_VERSION {
        return Err(AuroraInstallError::State(AuroraStateError::Malformed {
            reason: format!(
                "the persisted release entry schema {} is unsupported",
                document.schema_version
            ),
        }));
    }
    // Re-validate the entry through the manifest boundary: one entry,
    // full manifest validation, no second parser.
    let manifest = crate::distribution::ReleaseManifest::from_json(
        &serde_json::to_string(&serde_json::json!({
            "schemaVersion": crate::distribution::ReleaseManifest::SCHEMA_VERSION,
            "releases": [document.release],
        }))
        .map_err(|error| {
            AuroraInstallError::State(AuroraStateError::Malformed {
                reason: error.to_string(),
            })
        })?,
    )
    .map_err(|error| {
        AuroraInstallError::State(AuroraStateError::Malformed {
            reason: error.to_string(),
        })
    })?;
    Ok(manifest.releases().first().cloned())
}

/// Persists the installed-release entry atomically. Called only by the
/// verified update transaction, beside the installed-state commit.
pub(crate) fn write_instance_release(
    managed: &ManagedPaths,
    instance: &InstanceId,
    release: &crate::distribution::AuroraRelease,
) -> Result<(), AuroraInstallError> {
    let document = InstanceReleaseDocument {
        schema_version: INSTANCE_RELEASE_SCHEMA_VERSION,
        source: "published-release-manifest".to_owned(),
        release: release.clone(),
    };
    let target = managed
        .instance_paths(instance)
        .root()
        .join(INSTANCE_RELEASE_FILE_NAME);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(AuroraInstallError::StateWrite)?;
    }
    let json = serde_json::to_string_pretty(&document).map_err(|error| {
        AuroraInstallError::State(AuroraStateError::Malformed {
            reason: error.to_string(),
        })
    })?;
    let mut temporary = target.clone().into_os_string();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, format!("{json}\n")).map_err(AuroraInstallError::StateWrite)?;
    std::fs::rename(&temporary, &target).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        AuroraInstallError::StateWrite(error)
    })
}

/// The embedded manifest extended with this instance's persisted release
/// entry, when the embedded manifest does not already know that version.
///
/// Resolution stays exact: the sidecar only adds one reviewable entry, and
/// embedded entries always win over a stale sidecar.
pub fn merged_release_manifest(
    managed: &ManagedPaths,
    instance: &InstanceId,
    embedded: &crate::distribution::ReleaseManifest,
) -> Result<crate::distribution::ReleaseManifest, AuroraInstallError> {
    let Some(persisted) = load_instance_release(managed, instance)? else {
        return Ok(embedded.clone());
    };
    if embedded
        .resolve_environment(
            persisted.aurora_version(),
            Some(persisted.channel()),
            persisted.minecraft_version(),
        )
        .is_some()
    {
        return Ok(embedded.clone());
    }
    embedded.with_release(persisted).map_err(|error| {
        AuroraInstallError::State(AuroraStateError::Malformed {
            reason: error.to_string(),
        })
    })
}

/// Acquires the release's artifact through the SHA-256 verified store and
/// materializes it into the instance's managed mods path.
///
/// Order of operations is deliberate: acquire and verify in the cache,
/// copy into the instance with an immediate size check, re-hash the
/// materialized copy against the expected digest, and only then commit the
/// installed-state record. A failure at any point leaves no record, so a
/// partially installed Aurora artifact is never mistaken for installed.
pub async fn install_aurora(
    managed: &ManagedPaths,
    instance: &InstanceId,
    release: &AuroraRelease,
    options: &DownloadOptions,
) -> Result<InstalledAurora, AuroraInstallError> {
    // Bootstrap state remains strict even when its artifacts are optional.
    // Explicit restoration never overwrites malformed ownership evidence.
    let previous_state = load_installed_state(managed, instance)?;
    preflight_bootstrap(managed, instance, release)?;
    let provider_api = compatible_provider_api(managed, instance, release)?;
    let expected = ArtifactDigest::parse(release.artifact().sha256())
        .map_err(|error| AuroraInstallError::ReleaseInvalid(error.to_string()))?;
    let source = ArtifactSource::https_or_loopback(
        release.artifact().url(),
        &release.artifact().sha256(),
        release.artifact().size_bytes(),
    )
    .map_err(|error| AuroraInstallError::ArtifactInvalid(error.to_string()))?;

    let cache = ArtifactCache::new(managed.clone());
    let verified = cache
        .acquire_with(&source, options)
        .await
        .map_err(AuroraInstallError::Acquisition)?;

    // Acquire every required mod through the verified store before touching
    // the instance. A failed dependency cannot leave a completed state.
    let fabric_api = if let Some(dependency) = release.fabric_api().filter(|_| !provider_api) {
        let digest = ArtifactDigest::parse(dependency.artifact().sha256())
            .map_err(|error| AuroraInstallError::ReleaseInvalid(error.to_string()))?;
        let source = ArtifactSource::https_or_loopback(
            dependency.artifact().url(),
            dependency.artifact().sha256(),
            dependency.artifact().size_bytes(),
        )
        .map_err(|error| AuroraInstallError::ArtifactInvalid(error.to_string()))?;
        let verified = cache
            .acquire_with(&source, options)
            .await
            .map_err(AuroraInstallError::Acquisition)?;
        Some((dependency, digest, verified))
    } else {
        None
    };

    let relative = managed_artifact_relative_path(release)?;
    let instance_paths = managed.instance_paths(instance);
    let artifact_path = instance_paths
        .root()
        .join(relative.split('/').collect::<PathBuf>());
    if !artifact_path.starts_with(instance_paths.mods()) {
        return Err(AuroraInstallError::Materialization {
            path: relative.clone(),
            reason: "the derived path escaped the managed mods directory".to_owned(),
        });
    }

    if let Some(parent) = artifact_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| AuroraInstallError::Materialization {
            path: relative.clone(),
            reason: error.to_string(),
        })?;
    }
    materialize_bootstrap(
        managed,
        instance,
        &verified.path,
        &artifact_path,
        &expected,
        verified.bytes,
    )?;

    // Re-verify the materialized copy — a successful copy call is not
    // itself trusted content.
    let bytes = verify_file(&artifact_path, &expected, Some(verified.bytes)).map_err(|error| {
        AuroraInstallError::Materialization {
            path: relative.clone(),
            reason: format!("the materialized artifact failed verification: {error}"),
        }
    })?;

    let installed_fabric_api = if let Some((dependency, digest, verified)) = fabric_api {
        let relative_path = format!("mods/fabric-api-{}.jar", dependency.version());
        validate_fabric_api_relative_path(&relative_path)
            .map_err(AuroraInstallError::ReleaseInvalid)?;
        let path = instance_paths
            .root()
            .join(relative_path.split('/').collect::<PathBuf>());
        if !path.starts_with(instance_paths.mods()) {
            return Err(AuroraInstallError::Materialization {
                path: relative_path,
                reason: "the derived path escaped the managed mods directory".to_owned(),
            });
        }
        materialize_bootstrap(
            managed,
            instance,
            &verified.path,
            &path,
            &digest,
            verified.bytes,
        )?;
        let size_bytes = verify_file(&path, &digest, Some(verified.bytes)).map_err(|error| {
            AuroraInstallError::Materialization {
                path: relative_path.clone(),
                reason: format!("the materialized Fabric API failed verification: {error}"),
            }
        })?;
        Some(InstalledFabricApi {
            version: dependency.version().to_owned(),
            artifact: AuroraInstalledArtifact {
                relative_path,
                size_bytes,
                sha256: digest.as_hex(),
            },
        })
    } else if provider_api {
        previous_state
            .as_ref()
            .and_then(|state| state.fabric_api().cloned())
    } else {
        None
    };

    let state = AuroraInstalledState {
        schema_version: AURORA_INSTALLED_SCHEMA_VERSION,
        aurora_version: release.aurora_version().to_owned(),
        channel: release.channel(),
        minecraft_version: release.minecraft_version().to_owned(),
        fabric_loader_version: release.fabric_loader_version().to_owned(),
        artifact: AuroraInstalledArtifact {
            relative_path: relative,
            size_bytes: bytes,
            sha256: expected.as_hex(),
        },
        fabric_api: installed_fabric_api,
        ephemera: previous_state
            .as_ref()
            .and_then(|state| state.ephemera().cloned()),
        installation_id: format!(
            "aurora-install-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_millis())
                .unwrap_or_default(),
            std::process::id()
        ),
        installed_at_unix_seconds: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default(),
    };
    write_installed_state(managed, instance, &state)?;

    Ok(InstalledAurora {
        state,
        artifact_path,
    })
}

/// Read-only collision check, also used before changing a ready registry record.
pub(crate) fn compatible_provider_api(
    managed: &ManagedPaths,
    instance: &InstanceId,
    release: &AuroraRelease,
) -> Result<bool, AuroraInstallError> {
    // Reuse only a verified active provider-owned API. Never adopt local files
    // or fabricate a launcher ownership receipt for the provider's artifact.
    if release.fabric_api().is_none() || !managed.instance_paths(instance).mods().is_dir() {
        return Ok(false);
    }
    let state = crate::instance_content::ContentState::load(managed, instance)
        .map_err(|error| AuroraInstallError::ReleaseInvalid(error.to_string()))?;
    for record in state.entries.iter().filter(|record| {
        record.content_type == crate::instance_content::ContentType::Mod
            && record.compatibility.loader.as_deref() == Some("fabric")
            && (record.origin == crate::instance_content::ProviderOrigin::Pack
                || record
                    .compatibility
                    .minecraft_versions
                    .iter()
                    .any(|version| version == release.minecraft_version()))
    }) {
        let path = crate::instance_content::validate_provider_file(managed, instance, record)
            .map_err(|error| AuroraInstallError::ReleaseInvalid(error.to_string()))?;
        if path
            .extension()
            .is_some_and(|extension| extension == "disabled")
        {
            continue;
        }
        let (metadata, warnings) = crate::instance_mods::inspect_fabric_metadata(
            &path,
            std::fs::metadata(&path)
                .map_err(AuroraInstallError::StateRead)?
                .len(),
        );
        if warnings.is_empty() && metadata.is_some_and(|metadata| metadata.id == "fabric-api") {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn preflight_bootstrap(
    managed: &ManagedPaths,
    instance: &InstanceId,
    release: &AuroraRelease,
) -> Result<(), AuroraInstallError> {
    load_installed_state(managed, instance)?;
    let paths = managed.instance_paths(instance);
    if !paths.mods().exists() {
        return Ok(());
    }
    let mods =
        crate::instance_mods::validate_mods_directory(managed, instance).map_err(|error| {
            AuroraInstallError::Materialization {
                path: "mods".into(),
                reason: error.to_string(),
            }
        })?;
    let mut files = vec![(
        managed_artifact_relative_path(release)?,
        release.artifact().sha256(),
        release.artifact().size_bytes(),
    )];
    let provider_api = compatible_provider_api(managed, instance, release)?;
    if let Some(api) = release.fabric_api().filter(|_| !provider_api) {
        files.push((
            format!("mods/fabric-api-{}.jar", api.version()),
            api.artifact().sha256(),
            api.artifact().size_bytes(),
        ));
    }
    for (relative, hash, size) in files {
        let name = relative
            .strip_prefix("mods/")
            .expect("validated bootstrap path");
        if std::fs::symlink_metadata(mods.join(format!("{name}.disabled"))).is_ok() {
            return Err(AuroraInstallError::ReleaseInvalid("Re-enable disabled bootstrap content in Mods before restoring it. No duplicate will be created.".into()));
        }
        if std::fs::symlink_metadata(mods.join(name)).is_ok() {
            let target = crate::instance_mods::validate_current_regular_file(&mods, name)
                .map_err(|error| AuroraInstallError::ReleaseInvalid(error.to_string()))?;
            let digest = ArtifactDigest::parse(hash)
                .map_err(|error| AuroraInstallError::ReleaseInvalid(error.to_string()))?;
            verify_file(&target, &digest, size).map_err(|_| AuroraInstallError::ReleaseInvalid("Existing bootstrap content differs. Inspect or remove it explicitly before restoration; it will not be overwritten.".into()))?;
        }
    }
    Ok(())
}

fn materialize_bootstrap(
    managed: &ManagedPaths,
    instance: &InstanceId,
    source: &std::path::Path,
    target: &std::path::Path,
    digest: &ArtifactDigest,
    bytes: u64,
) -> Result<(), AuroraInstallError> {
    let fail = |reason: String| AuroraInstallError::Materialization {
        path: "bootstrap mod".into(),
        reason,
    };
    let mods = crate::instance_mods::validate_mods_directory(managed, instance)
        .map_err(|e| fail(e.to_string()))?;
    let name = target
        .file_name()
        .ok_or_else(|| fail("Invalid bootstrap destination.".into()))?
        .to_string_lossy();
    if std::fs::symlink_metadata(mods.join(format!("{name}.disabled"))).is_ok() {
        return Err(fail("A disabled bootstrap file exists. Re-enable it in Mods before restoring; no duplicate will be created.".into()));
    }
    if std::fs::symlink_metadata(target).is_ok() {
        crate::instance_mods::validate_current_regular_file(&mods, &name)
            .map_err(|e| fail(e.to_string()))?;
        verify_file(target, digest, Some(bytes)).map_err(|_| fail("Existing mod bytes differ. Inspect the file before restoring; user content will not be overwritten.".into()))?;
        return Ok(());
    }
    let stage = mods.join(format!(".aurora-bootstrap-stage-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        std::fs::copy(source, &stage).map_err(|e| fail(e.to_string()))?;
        verify_file(&stage, digest, Some(bytes)).map_err(|e| fail(e.to_string()))?;
        std::fs::hard_link(&stage, target).map_err(|e| fail(e.to_string()))?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&stage);
    result
}

/// Writes the installed-state record atomically (sibling temporary file plus
/// rename) at the instance root.
pub fn write_installed_state(
    managed: &ManagedPaths,
    instance: &InstanceId,
    state: &AuroraInstalledState,
) -> Result<(), AuroraInstallError> {
    let target = managed
        .instance_paths(instance)
        .root()
        .join(AURORA_INSTALLED_FILE_NAME);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| AuroraInstallError::StateWrite(error))?;
    }

    let mut temporary = target.clone().into_os_string();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, state.to_json())
        .map_err(|error| AuroraInstallError::StateWrite(error))?;
    std::fs::rename(&temporary, &target).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        AuroraInstallError::StateWrite(error)
    })
}

/// Loads the Aurora installed-state record of one instance, if present.
///
/// A missing record is `Ok(None)`; a present-but-malformed record is a
/// deliberate error that is never overwritten by the caller.
pub fn load_installed_state(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<Option<AuroraInstalledState>, AuroraInstallError> {
    let path = managed
        .instance_paths(instance)
        .root()
        .join(AURORA_INSTALLED_FILE_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AuroraInstallError::StateRead(error)),
    };
    AuroraInstalledState::from_json(&text)
        .map(Some)
        .map_err(AuroraInstallError::State)
}

/// Re-validates the materialized Aurora artifact against its recorded
/// state: existence, size, and SHA-256. Read-only.
pub fn validate_artifact(
    managed: &ManagedPaths,
    instance: &InstanceId,
    state: &AuroraInstalledState,
) -> Result<(), String> {
    let artifact_path = managed.instance_paths(instance).root().join(
        state
            .artifact()
            .relative_path()
            .split('/')
            .collect::<PathBuf>(),
    );
    let expected =
        ArtifactDigest::parse(state.artifact().sha256()).map_err(|error| error.to_string())?;
    verify_file(
        &artifact_path,
        &expected,
        Some(state.artifact().size_bytes()),
    )
    .map_err(|error| error.to_string())?;
    if let Some(fabric_api) = state.fabric_api() {
        let path = managed.instance_paths(instance).root().join(
            fabric_api
                .artifact()
                .relative_path()
                .split('/')
                .collect::<PathBuf>(),
        );
        let digest = ArtifactDigest::parse(fabric_api.artifact().sha256())
            .map_err(|error| error.to_string())?;
        verify_file(&path, &digest, Some(fabric_api.artifact().size_bytes()))
            .map_err(|error| format!("Fabric API verification failed: {error}"))?;
    }
    if let Some(ephemera) = state.ephemera() {
        let path = managed.instance_paths(instance).root().join(
            ephemera
                .artifact()
                .relative_path()
                .split('/')
                .collect::<PathBuf>(),
        );
        let digest = ArtifactDigest::parse(ephemera.artifact().sha256())
            .map_err(|error| error.to_string())?;
        verify_file(&path, &digest, Some(ephemera.artifact().size_bytes()))
            .map_err(|error| format!("Ephemera verification failed: {error}"))?;
    }
    Ok(())
}

/// Mods that replace Minecraft's terrain renderer and therefore cannot run
/// alongside Ephemera.
const CONFLICTING_RENDERER_MOD_IDS: &[&str] = &["sodium", "iris", "vulkanmod", "nvidium"];

/// Returns the first enabled conflicting renderer mod ID present in the
/// instance's `mods/` directory, if any.
pub fn conflicting_renderer_mod(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<Option<String>, AuroraInstallError> {
    let paths = managed.instance_paths(instance);
    if !paths.mods().is_dir() {
        return Ok(None);
    }
    let inventory = crate::instance_mods::scan(managed, instance).map_err(|error| {
        AuroraInstallError::Materialization {
            path: "mods".into(),
            reason: error.to_string(),
        }
    })?;
    for entry in inventory.entries.iter().filter(|entry| entry.enabled) {
        if let Some(meta) = &entry.metadata {
            if let Some(conflict) = CONFLICTING_RENDERER_MOD_IDS
                .iter()
                .find(|&&id| meta.id == id || meta.nested_mod_ids.iter().any(|n| n == id))
            {
                return Ok(Some((*conflict).to_owned()));
            }
        }
    }
    Ok(None)
}

/// Read-only status of launcher-managed Ephemera on one instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EphemeraStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub file_name: Option<String>,
    /// One of `"active"`, `"disabled"`, `"modified"`, `"missing"`, `"notInstalled"`.
    pub status: String,
    pub conflicting_mod: Option<String>,
}

/// Inspects the launcher-managed Ephemera status for one instance without
/// modifying any files.
pub fn ephemera_status(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<EphemeraStatus, AuroraInstallError> {
    let conflicting_mod = conflicting_renderer_mod(managed, instance)?;
    let Some(state) = load_installed_state(managed, instance)? else {
        return Ok(EphemeraStatus {
            installed: false,
            version: None,
            file_name: None,
            status: "notInstalled".into(),
            conflicting_mod,
        });
    };
    let Some(ephemera) = state.ephemera() else {
        return Ok(EphemeraStatus {
            installed: false,
            version: None,
            file_name: None,
            status: "notInstalled".into(),
            conflicting_mod,
        });
    };
    let file_name = ephemera
        .artifact()
        .relative_path()
        .strip_prefix("mods/")
        .unwrap_or(ephemera.artifact().relative_path())
        .to_owned();
    let instance_paths = managed.instance_paths(instance);
    let mods = instance_paths.mods();
    let active = mods.join(&file_name);
    let disabled = mods.join(format!("{file_name}.disabled"));
    let has_active = std::fs::symlink_metadata(&active).is_ok();
    let has_disabled = std::fs::symlink_metadata(&disabled).is_ok();
    let status = if has_active && has_disabled {
        "modified"
    } else if !has_active && !has_disabled {
        "missing"
    } else {
        let path = if has_active { &active } else { &disabled };
        let valid = ArtifactDigest::parse(ephemera.artifact().sha256()).is_ok_and(|digest| {
            verify_file(path, &digest, Some(ephemera.artifact().size_bytes())).is_ok()
        });
        if !valid {
            "modified"
        } else if has_active {
            "active"
        } else {
            "disabled"
        }
    };
    Ok(EphemeraStatus {
        installed: has_active || has_disabled,
        version: Some(ephemera.version().to_owned()),
        file_name: Some(file_name),
        status: status.into(),
        conflicting_mod,
    })
}

/// Outcome of attempting to install or update launcher-managed Ephemera on an
/// Aurora instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EphemeraInstallOutcome {
    Installed(InstalledEphemera),
    SkippedConflict { conflicting_mod: String },
    SkippedExisting,
}

/// Acquires a verified Ephemera release through [`ArtifactCache`], materializes
/// it to `mods/ephemera-<version>.jar`, re-verifies its SHA-256, and records
/// ownership atomically in `aurora-installed.json`.
///
/// Automatically skips if a conflicting renderer mod (`sodium`, `iris`,
/// `vulkanmod`, `nvidium`) or an unmanaged Ephemera copy is already installed.
pub async fn install_managed_ephemera(
    managed: &ManagedPaths,
    instance: &InstanceId,
    release: &crate::distribution::ephemera::EphemeraRelease,
    options: &DownloadOptions,
) -> Result<EphemeraInstallOutcome, AuroraInstallError> {
    let mut state = load_installed_state(managed, instance)?.ok_or_else(|| {
        AuroraInstallError::ReleaseInvalid(
            "Aurora Client must be installed before bundling Ephemera.".into(),
        )
    })?;
    if let Some(conflicting_mod) = conflicting_renderer_mod(managed, instance)? {
        return Ok(EphemeraInstallOutcome::SkippedConflict { conflicting_mod });
    }

    let relative = crate::distribution::ephemera::managed_artifact_relative_path(release.version())
        .map_err(AuroraInstallError::ReleaseInvalid)?;
    validate_ephemera_relative_path(&relative).map_err(AuroraInstallError::ReleaseInvalid)?;

    // Never overwrite or duplicate an existing user-installed or provider-managed Ephemera jar.
    let instance_paths = managed.instance_paths(instance);
    if instance_paths.mods().is_dir() {
        let managed_file_name = state
            .ephemera()
            .and_then(|e| e.artifact().relative_path().strip_prefix("mods/"))
            .map(str::to_owned);
        let inventory = crate::instance_mods::scan(managed, instance).map_err(|error| {
            AuroraInstallError::Materialization {
                path: relative.clone(),
                reason: error.to_string(),
            }
        })?;
        let has_unmanaged_ephemera = inventory.entries.iter().any(|entry| {
            let is_current_managed = managed_file_name.as_deref().is_some_and(|name| {
                entry.file_name.eq_ignore_ascii_case(name)
                    || entry
                        .file_name
                        .eq_ignore_ascii_case(&format!("{name}.disabled"))
            });
            !is_current_managed
                && entry
                    .metadata
                    .as_ref()
                    .is_some_and(|meta| meta.id == "ephemera")
        });
        if has_unmanaged_ephemera {
            return Ok(EphemeraInstallOutcome::SkippedExisting);
        }
    }

    let expected = ArtifactDigest::parse(release.artifact().sha256())
        .map_err(|error| AuroraInstallError::ReleaseInvalid(error.to_string()))?;
    let source = ArtifactSource::https_or_loopback(
        release.artifact().url(),
        release.artifact().sha256(),
        release.artifact().size_bytes(),
    )
    .map_err(|error| AuroraInstallError::ArtifactInvalid(error.to_string()))?;

    let cache = ArtifactCache::new(managed.clone());
    let verified = cache
        .acquire_with(&source, options)
        .await
        .map_err(AuroraInstallError::Acquisition)?;

    let target = instance_paths
        .root()
        .join(relative.split('/').collect::<PathBuf>());
    if !target.starts_with(instance_paths.mods()) {
        return Err(AuroraInstallError::Materialization {
            path: relative.clone(),
            reason: "the derived Ephemera path escaped the managed mods directory".into(),
        });
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| AuroraInstallError::Materialization {
            path: relative.clone(),
            reason: error.to_string(),
        })?;
    }

    materialize_bootstrap(
        managed,
        instance,
        &verified.path,
        &target,
        &expected,
        verified.bytes,
    )?;
    let bytes = verify_file(&target, &expected, Some(verified.bytes)).map_err(|error| {
        AuroraInstallError::Materialization {
            path: relative.clone(),
            reason: format!("the materialized Ephemera artifact failed verification: {error}"),
        }
    })?;

    // Retire an older launcher-managed Ephemera JAR only after the new copy verified.
    if let Some(previous) = state.ephemera() {
        if previous.artifact().relative_path() != relative {
            let old_path = instance_paths.root().join(
                previous
                    .artifact()
                    .relative_path()
                    .split('/')
                    .collect::<PathBuf>(),
            );
            if let Ok(old_digest) = ArtifactDigest::parse(previous.artifact().sha256()) {
                let mut disabled_os = old_path.as_os_str().to_os_string();
                disabled_os.push(".disabled");
                for candidate in [old_path.clone(), PathBuf::from(disabled_os)] {
                    if verify_file(
                        &candidate,
                        &old_digest,
                        Some(previous.artifact().size_bytes()),
                    )
                    .is_ok()
                    {
                        let _ = std::fs::remove_file(&candidate);
                    }
                }
            }
        }
    }

    let installed = InstalledEphemera {
        version: release.version().to_owned(),
        artifact: AuroraInstalledArtifact {
            relative_path: relative,
            size_bytes: bytes,
            sha256: expected.as_hex(),
        },
    };
    state.set_ephemera(Some(installed.clone()));
    write_installed_state(managed, instance, &state)?;
    Ok(EphemeraInstallOutcome::Installed(installed))
}

/// Removes launcher-managed Ephemera from one instance and clears its ownership
/// record from `aurora-installed.json`. Refuses to delete user-modified bytes.
pub fn remove_managed_ephemera(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<bool, AuroraInstallError> {
    let Some(mut state) = load_installed_state(managed, instance)? else {
        return Ok(false);
    };
    let Some(ephemera) = state.ephemera().cloned() else {
        return Ok(false);
    };
    let file_name = ephemera
        .artifact()
        .relative_path()
        .strip_prefix("mods/")
        .ok_or_else(|| {
            AuroraInstallError::ReleaseInvalid("invalid Ephemera managed path".into())
        })?;
    let instance_paths = managed.instance_paths(instance);
    let mods = instance_paths.mods();
    let expected = ArtifactDigest::parse(ephemera.artifact().sha256())
        .map_err(|error| AuroraInstallError::ReleaseInvalid(error.to_string()))?;

    for candidate in [
        mods.join(file_name),
        mods.join(format!("{file_name}.disabled")),
    ] {
        match std::fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(AuroraInstallError::Materialization {
                    path: ephemera.artifact().relative_path().to_owned(),
                    reason: error.to_string(),
                });
            }
            Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => {
                return Err(AuroraInstallError::Materialization {
                    path: ephemera.artifact().relative_path().to_owned(),
                    reason: "Ephemera file is not a regular contained file; inspect it manually."
                        .into(),
                });
            }
            Ok(_) => {
                verify_file(
                    &candidate,
                    &expected,
                    Some(ephemera.artifact().size_bytes()),
                )
                .map_err(|_| AuroraInstallError::Materialization {
                    path: ephemera.artifact().relative_path().to_owned(),
                    reason: "Ephemera bytes were modified externally; remove it manually in Mods."
                        .into(),
                })?;
                std::fs::remove_file(&candidate).map_err(|error| {
                    AuroraInstallError::Materialization {
                        path: ephemera.artifact().relative_path().to_owned(),
                        reason: error.to_string(),
                    }
                })?;
            }
        }
    }

    state.set_ephemera(None);
    write_installed_state(managed, instance, &state)?;
    Ok(true)
}

/// A failed Aurora installation.
#[derive(Debug)]
pub enum AuroraInstallError {
    /// The release entry cannot describe a safe, verifiable artifact.
    ReleaseInvalid(String),
    /// The release artifact metadata could not become a valid source.
    ArtifactInvalid(String),
    /// Acquisition (transport, integrity, or cache) failed.
    Acquisition(crate::cache::AcquisitionError),
    /// Materializing or re-verifying the artifact in the instance failed.
    Materialization { path: String, reason: String },
    /// Reading the persisted installed state failed.
    StateRead(std::io::Error),
    /// Writing the persisted installed state failed.
    StateWrite(std::io::Error),
    /// The persisted installed state is malformed or unsupported.
    State(AuroraStateError),
}

impl fmt::Display for AuroraInstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReleaseInvalid(reason) => {
                write!(formatter, "the Aurora release is unusable: {reason}")
            }
            Self::ArtifactInvalid(reason) => write!(
                formatter,
                "the Aurora release artifact metadata is invalid: {reason}"
            ),
            Self::Acquisition(error) => write!(formatter, "{error}"),
            Self::Materialization { path, reason } => write!(
                formatter,
                "the Aurora artifact '{path}' could not be materialized into the instance: {reason}"
            ),
            Self::StateRead(error) => write!(
                formatter,
                "the Aurora installed state could not be read: {error}"
            ),
            Self::StateWrite(error) => write!(
                formatter,
                "the Aurora installed state could not be written: {error}"
            ),
            Self::State(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AuroraInstallError {}

/// A malformed or unsupported Aurora installed-state document.
#[derive(Debug)]
pub enum AuroraStateError {
    Malformed { reason: String },
    UnsupportedSchema { found: u32, supported: u32 },
}

impl fmt::Display for AuroraStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { reason } => {
                write!(
                    formatter,
                    "the Aurora installed state is malformed: {reason}"
                )
            }
            Self::UnsupportedSchema { found, supported } => write!(
                formatter,
                "the Aurora installed state uses schema version {found}; this launcher supports {supported}"
            ),
        }
    }
}

impl std::error::Error for AuroraStateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestResponse, TestServer};
    use std::sync::Arc;
    use std::time::Duration;

    fn sha256_of(bytes: &[u8]) -> String {
        use sha2::Digest as _;
        let digest: [u8; 32] = sha2::Sha256::digest(bytes).into();
        ArtifactDigest::from_sha256(digest).as_hex()
    }

    fn release(url: &str, sha256: &str, size: Option<u64>) -> AuroraRelease {
        serde_json::from_value(serde_json::json!({
            "auroraVersion": "0.3.0",
            "channel": "stable",
            "minecraftVersion": "26.2",
            "fabricLoaderVersion": "0.19.5",
            "java": { "majorVersion": 25 },
            "artifact": { "url": url, "sha256": sha256, "sizeBytes": size }
        }))
        .unwrap()
    }

    fn test_managed(name: &str) -> (PathBuf, ManagedPaths, InstanceId) {
        let root = std::env::temp_dir()
            .join("aurora-aurora-test")
            .join(std::process::id().to_string())
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
        let instance = InstanceId::new("inst-test").unwrap();
        (root, managed, instance)
    }

    fn quick_options() -> DownloadOptions {
        DownloadOptions {
            connect_timeout: Duration::from_secs(5),
            idle_read_timeout: Duration::from_secs(5),
            max_redirects: crate::downloads::MAX_REDIRECTS,
        }
    }

    #[tokio::test]
    async fn installs_the_verified_artifact_as_a_managed_mod_and_commits_state_last() {
        let body = b"aurora client artifact bytes".to_vec();
        let served = body.clone();
        let server = TestServer::spawn(Arc::new(move |_request| TestResponse::ok(&served)));
        let (root, managed, instance) = test_managed("happy");
        let rel = release(
            &format!("{}/aurora-0.3.0-dev.jar", server.base_url()),
            &sha256_of(&body),
            Some(body.len() as u64),
        );

        // A user mod and config exist before installation; they must survive.
        let instance_paths = managed.instance_paths(&instance);
        std::fs::create_dir_all(instance_paths.mods()).unwrap();
        std::fs::write(
            instance_paths.mods().join("user-mod.jar"),
            b"user's own mod",
        )
        .unwrap();
        std::fs::create_dir_all(instance_paths.config()).unwrap();
        std::fs::write(instance_paths.config().join("options.txt"), b"gamma:1.0").unwrap();

        let installed = install_aurora(&managed, &instance, &rel, &quick_options())
            .await
            .expect("installation must succeed");

        let managed_path = instance_paths.mods().join("aurora-0.3.0.jar");
        assert_eq!(installed.artifact_path, managed_path);
        assert_eq!(std::fs::read(&managed_path).unwrap(), body);
        assert_eq!(
            std::fs::read(instance_paths.mods().join("user-mod.jar")).unwrap(),
            b"user's own mod",
            "user mods are untouched"
        );
        assert_eq!(
            std::fs::read(instance_paths.config().join("options.txt")).unwrap(),
            b"gamma:1.0"
        );

        let state = load_installed_state(&managed, &instance)
            .unwrap()
            .expect("state must be committed");
        assert_eq!(state.aurora_version(), "0.3.0");
        assert_eq!(state.artifact().relative_path(), "mods/aurora-0.3.0.jar");
        assert_eq!(state.artifact().sha256(), sha256_of(&body));
        assert_eq!(state.trust().kind_name(), "expectedDigestVerified");
        assert!(validate_artifact(&managed, &instance, &state).is_ok());
        let _ = root;
    }

    #[tokio::test]
    async fn required_fabric_api_is_verified_protected_by_state_and_detects_damage() {
        let body = b"two verified managed mod artifacts".to_vec();
        let served = body.clone();
        let server = TestServer::spawn(Arc::new(move |_request| TestResponse::ok(&served)));
        let (root, managed, instance) = test_managed("fabric-api");
        let mut value = serde_json::to_value(release(
            &format!("{}/aurora.jar", server.base_url()),
            &sha256_of(&body),
            Some(body.len() as u64),
        ))
        .unwrap();
        value["fabricApi"] = serde_json::json!({
            "version": "0.141.6+1.21.11",
            "artifact": {
                "url": format!("{}/fabric-api.jar", server.base_url()),
                "sha256": sha256_of(&body),
                "sizeBytes": body.len()
            }
        });
        let release: AuroraRelease = serde_json::from_value(value).unwrap();
        install_aurora(&managed, &instance, &release, &quick_options())
            .await
            .unwrap();
        let state = load_installed_state(&managed, &instance).unwrap().unwrap();
        let dependency = state.fabric_api().unwrap();
        assert_eq!(dependency.version(), "0.141.6+1.21.11");
        assert_eq!(
            dependency.artifact().relative_path(),
            "mods/fabric-api-0.141.6+1.21.11.jar"
        );
        assert!(validate_artifact(&managed, &instance, &state).is_ok());
        let path = managed
            .instance_paths(&instance)
            .mods()
            .join("fabric-api-0.141.6+1.21.11.jar");
        std::fs::write(&path, b"damage").unwrap();
        assert!(validate_artifact(&managed, &instance, &state).is_err());
        let _ = root;
    }

    #[tokio::test]
    async fn wrong_fabric_api_hash_never_activates_either_managed_mod() {
        let body = b"untrusted dependency bytes".to_vec();
        let served = body.clone();
        let server = TestServer::spawn(Arc::new(move |_request| TestResponse::ok(&served)));
        let (root, managed, instance) = test_managed("fabric-api-wrong-hash");
        let mut value = serde_json::to_value(release(
            &format!("{}/aurora.jar", server.base_url()),
            &sha256_of(&body),
            Some(body.len() as u64),
        ))
        .unwrap();
        value["fabricApi"] = serde_json::json!({
            "version": "0.141.6+1.21.11",
            "artifact": {
                "url": format!("{}/fabric-api.jar", server.base_url()),
                "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                "sizeBytes": body.len()
            }
        });
        let release: AuroraRelease = serde_json::from_value(value).unwrap();
        assert!(
            install_aurora(&managed, &instance, &release, &quick_options())
                .await
                .is_err()
        );
        assert!(load_installed_state(&managed, &instance).unwrap().is_none());
        assert!(!managed.instance_paths(&instance).mods().exists());
        let _ = root;
    }

    #[tokio::test]
    async fn a_hash_mismatch_fails_without_committing_state() {
        let body = b"tampered artifact bytes".to_vec();
        let served = body.clone();
        let server = TestServer::spawn(Arc::new(move |_request| TestResponse::ok(&served)));
        let (root, managed, instance) = test_managed("mismatch");
        let rel = release(
            &format!("{}/aurora.jar", server.base_url()),
            &sha256_of(b"different expected bytes"),
            None,
        );

        let error = install_aurora(&managed, &instance, &rel, &quick_options())
            .await
            .expect_err("a mismatched artifact must fail");

        assert!(
            matches!(error, AuroraInstallError::Acquisition(_)),
            "{error}"
        );
        assert!(load_installed_state(&managed, &instance).unwrap().is_none());
        assert!(
            !managed
                .instance_paths(&instance)
                .root()
                .join(AURORA_INSTALLED_FILE_NAME)
                .exists()
        );
        let _ = root;
    }

    #[tokio::test]
    async fn a_materialization_failure_leaves_no_state() {
        let body = b"artifact behind a blocked mods path".to_vec();
        let served = body.clone();
        let server = TestServer::spawn(Arc::new(move |_request| TestResponse::ok(&served)));
        let (root, managed, instance) = test_managed("blocked-mods");
        let instance_paths = managed.instance_paths(&instance);
        // `mods` exists as a FILE: the managed copy cannot be placed.
        std::fs::create_dir_all(instance_paths.root()).unwrap();
        std::fs::write(instance_paths.mods(), b"not a directory").unwrap();

        let rel = release(
            &format!("{}/aurora.jar", server.base_url()),
            &sha256_of(&body),
            None,
        );
        let error = install_aurora(&managed, &instance, &rel, &quick_options())
            .await
            .expect_err("materialization must fail");

        assert!(
            matches!(error, AuroraInstallError::Materialization { .. }),
            "{error}"
        );
        assert!(load_installed_state(&managed, &instance).unwrap().is_none());
        let _ = root;
    }

    #[test]
    fn managed_paths_derive_from_release_metadata_and_reject_unsafe_versions() {
        let rel = release(
            "https://releases.example.invalid/a.jar",
            &"a".repeat(64),
            None,
        );
        assert_eq!(
            managed_artifact_relative_path(&rel).unwrap(),
            "mods/aurora-0.3.0.jar"
        );

        for broken_version in ["", "../escape", "sub/dir", "weird name", ".hidden"] {
            let mut value = serde_json::to_value(&rel).unwrap();
            value["auroraVersion"] = serde_json::Value::String(broken_version.to_owned());
            let broken: AuroraRelease = serde_json::from_value(value).unwrap();
            assert!(
                managed_artifact_relative_path(&broken).is_err(),
                "{broken_version:?} must not form a managed path"
            );
        }
    }

    #[test]
    fn installed_state_validation_is_strict() {
        let state_json = || {
            r#"{
  "schemaVersion": 1,
  "auroraVersion": "0.3.0",
  "channel": "stable",
  "minecraftVersion": "26.2",
  "fabricLoaderVersion": "0.19.5",
  "artifact": { "relativePath": "mods/aurora-0.3.0.jar", "sizeBytes": 57, "sha256": "41765f8d32372d242b51a5509f467480be661c19d127ea203e5bdf9d5da4f2c3" },
  "installationId": "aurora-install-1-1",
  "installedAtUnixSeconds": 1789455335
}"#
        };

        let state = AuroraInstalledState::from_json(state_json()).unwrap();
        assert_eq!(state.artifact().relative_path(), "mods/aurora-0.3.0.jar");
        assert!(AuroraInstalledState::from_json(&state.to_json()).is_ok());

        for broken in [
            state_json().replace("\"schemaVersion\": 1", "\"schemaVersion\": 2"),
            state_json().replace("mods/aurora-0.3.0.jar", "mods/aurora-0.3.0.json"),
            state_json().replace("mods/aurora-0.3.0.jar", "user-mod.jar"),
            state_json().replace("mods/aurora-0.3.0.jar", "../aurora-0.3.0.jar"),
            state_json().replace(
                "\"sha256\": \"41765f8d32372d242b51a5509f467480be661c19d127ea203e5bdf9d5da4f2c3\"",
                "\"sha256\": \"zz\"",
            ),
            state_json().replace("\"sizeBytes\": 57", "\"sizeBytes\": 0"),
            "not json".to_owned(),
        ] {
            assert!(
                AuroraInstalledState::from_json(&broken).is_err(),
                "must be rejected: {}",
                &broken[..broken.len().min(50)]
            );
        }
    }

    #[test]
    fn installed_state_accepts_optional_ephemera_and_keeps_legacy_schema1_compatible() {
        let legacy = r#"{
  "schemaVersion": 1,
  "auroraVersion": "3.7.0",
  "channel": "stable",
  "minecraftVersion": "26.2",
  "fabricLoaderVersion": "0.20.1",
  "artifact": { "relativePath": "mods/aurora-3.7.0.jar", "sizeBytes": 57, "sha256": "41765f8d32372d242b51a5509f467480be661c19d127ea203e5bdf9d5da4f2c3" },
  "installationId": "aurora-install-1-1",
  "installedAtUnixSeconds": 1789455335
}"#;
        let parsed = AuroraInstalledState::from_json(legacy).unwrap();
        assert!(parsed.ephemera().is_none());
        assert!(!parsed.to_json().contains("ephemera"));

        let with_ephemera = r#"{
  "schemaVersion": 1,
  "auroraVersion": "3.7.0",
  "channel": "stable",
  "minecraftVersion": "26.2",
  "fabricLoaderVersion": "0.20.1",
  "artifact": { "relativePath": "mods/aurora-3.7.0.jar", "sizeBytes": 57, "sha256": "41765f8d32372d242b51a5509f467480be661c19d127ea203e5bdf9d5da4f2c3" },
  "ephemera": {
    "version": "0.2.0",
    "artifact": { "relativePath": "mods/ephemera-0.2.0.jar", "sizeBytes": 128, "sha256": "41765f8d32372d242b51a5509f467480be661c19d127ea203e5bdf9d5da4f2c3" }
  },
  "installationId": "aurora-install-1-1",
  "installedAtUnixSeconds": 1789455335
}"#;
        let parsed = AuroraInstalledState::from_json(with_ephemera).unwrap();
        let eph = parsed.ephemera().expect("ephemera entry must parse");
        assert_eq!(eph.version(), "0.2.0");
        assert_eq!(eph.artifact().relative_path(), "mods/ephemera-0.2.0.jar");

        for broken in [
            with_ephemera.replace("mods/ephemera-0.2.0.jar", "mods/other-0.2.0.jar"),
            with_ephemera.replace("\"sizeBytes\": 128", "\"sizeBytes\": 0"),
            with_ephemera.replace("\"version\": \"0.2.0\"", "\"version\": \"\""),
        ] {
            assert!(AuroraInstalledState::from_json(&broken).is_err());
        }
    }

    #[tokio::test]
    async fn managed_ephemera_install_verify_damage_and_remove_lifecycle() {
        let aurora_body = b"aurora client bytes".to_vec();
        let ephemera_body = b"ephemera verified engine jar bytes".to_vec();
        let aurora_served = aurora_body.clone();
        let ephemera_served = ephemera_body.clone();
        let server = TestServer::spawn(Arc::new(move |request| {
            if request.path.contains("ephemera") {
                TestResponse::ok(&ephemera_served)
            } else {
                TestResponse::ok(&aurora_served)
            }
        }));
        let (root, managed, instance) = test_managed("ephemera-lifecycle");
        let rel = release(
            &format!("{}/aurora-0.3.0.jar", server.base_url()),
            &sha256_of(&aurora_body),
            Some(aurora_body.len() as u64),
        );
        install_aurora(&managed, &instance, &rel, &quick_options())
            .await
            .unwrap();

        let eph_artifact = crate::distribution::ReleaseArtifact::new(
            format!("{}/ephemera-0.2.0-mc26.2.jar", server.base_url()),
            sha256_of(&ephemera_body),
            Some(ephemera_body.len() as u64),
        )
        .unwrap();
        let eph_release = crate::distribution::ephemera::EphemeraRelease::new(
            "0.2.0",
            "26.2",
            25,
            eph_artifact,
            None,
        )
        .unwrap();

        let outcome = install_managed_ephemera(&managed, &instance, &eph_release, &quick_options())
            .await
            .unwrap();
        assert!(matches!(outcome, EphemeraInstallOutcome::Installed(_)));

        let status = ephemera_status(&managed, &instance).unwrap();
        assert!(status.installed);
        assert_eq!(status.version.as_deref(), Some("0.2.0"));
        assert_eq!(status.status, "active");
        assert!(status.conflicting_mod.is_none());

        let state = load_installed_state(&managed, &instance).unwrap().unwrap();
        assert!(validate_artifact(&managed, &instance, &state).is_ok());

        let eph_path = managed
            .instance_paths(&instance)
            .mods()
            .join("ephemera-0.2.0.jar");
        std::fs::write(&eph_path, b"corrupted").unwrap();
        assert!(validate_artifact(&managed, &instance, &state).is_err());
        assert_eq!(
            ephemera_status(&managed, &instance).unwrap().status,
            "modified"
        );
        // Refuses to delete externally modified bytes.
        assert!(remove_managed_ephemera(&managed, &instance).is_err());

        // Restore verified bytes -> clean removal succeeds.
        std::fs::write(&eph_path, &ephemera_body).unwrap();
        assert!(remove_managed_ephemera(&managed, &instance).unwrap());
        assert!(!eph_path.exists());
        assert!(
            load_installed_state(&managed, &instance)
                .unwrap()
                .unwrap()
                .ephemera()
                .is_none()
        );
        let _ = root;
    }

    #[tokio::test]
    async fn managed_ephemera_skips_when_sodium_or_iris_is_present() {
        use std::io::Write;
        let aurora_body = b"aurora client bytes".to_vec();
        let ephemera_body = b"ephemera bytes".to_vec();
        let aurora_served = aurora_body.clone();
        let server = TestServer::spawn(Arc::new(move |_| TestResponse::ok(&aurora_served)));
        let (root, managed, instance) = test_managed("ephemera-sodium-skip");
        let rel = release(
            &format!("{}/aurora-0.3.0.jar", server.base_url()),
            &sha256_of(&aurora_body),
            Some(aurora_body.len() as u64),
        );
        install_aurora(&managed, &instance, &rel, &quick_options())
            .await
            .unwrap();

        // Place a valid Fabric JAR declaring id "sodium" in mods/.
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(br#"{"schemaVersion":1,"id":"sodium","version":"0.9.2"}"#)
            .unwrap();
        let sodium_bytes = archive.finish().unwrap().into_inner();
        std::fs::write(
            managed
                .instance_paths(&instance)
                .mods()
                .join("sodium-0.9.2.jar"),
            &sodium_bytes,
        )
        .unwrap();

        let eph_artifact = crate::distribution::ReleaseArtifact::new(
            format!("{}/ephemera-0.2.0-mc26.2.jar", server.base_url()),
            sha256_of(&ephemera_body),
            Some(ephemera_body.len() as u64),
        )
        .unwrap();
        let eph_release = crate::distribution::ephemera::EphemeraRelease::new(
            "0.2.0",
            "26.2",
            25,
            eph_artifact,
            None,
        )
        .unwrap();

        let outcome = install_managed_ephemera(&managed, &instance, &eph_release, &quick_options())
            .await
            .unwrap();
        assert_eq!(
            outcome,
            EphemeraInstallOutcome::SkippedConflict {
                conflicting_mod: "sodium".into()
            }
        );
        let status = ephemera_status(&managed, &instance).unwrap();
        assert!(!status.installed);
        assert_eq!(status.conflicting_mod.as_deref(), Some("sodium"));
        let _ = root;
    }
}
