//! Forge metadata resolution and install-plan composition.
//!
//! The pipeline owned by this module:
//!
//! ```text
//! exact Minecraft version + exact Forge version
//!         ↓ Maven metadata listing + promotions (HTTPS discovery, no prior digest)
//! exact version lookup + Minecraft mapping cross-check
//!         ↓ installer jar (SHA-1 sidecar-pinned verified acquisition)
//! install_profile.json + version.json (bounded extraction, parse, validate)
//!         ↓ normalization
//! ForgePlan (Aurora-owned domain types)
//!         ↓ composition with MinecraftInstallPlan through game::plan
//! GameInstallPlan
//! ```
//!
//! Everything here is planning except the one acquisition the plan itself
//! depends on: the installer jar is both the metadata document carrier and
//! a real artifact, so it is acquired through the verified SHA-1 store
//! before its embedded documents are read. Nothing is installed, executed
//! into an instance, or launched by this module.

pub mod metadata;
pub mod plan;
pub mod processors;

use std::fmt;

use crate::cache::ArtifactCache;
use crate::downloads::{DownloadOptions, Sha1ArtifactSource};
use crate::fabric::plan::GameInstallPlan;
use crate::forge::metadata::{
    ForgeMavenEndpoints, ForgeMetadataError, ForgeVersionId, fetch_forge_versions,
    fetch_sidecar_sha1, read_installer_documents,
};
use crate::forge::plan::{ForgePlan, ForgePlanError, compose_forge_game_plan};
use crate::minecraft::MinecraftResolutionError;
use crate::minecraft::metadata::{MetadataEndpoints, MinecraftVersionId};
use crate::minecraft::rules::PlatformProfile;
use crate::paths::ManagedPaths;

/// Resolves one exact Minecraft + Forge combination into a normalized
/// Forge plan.
///
/// Resolution is exact, mirroring the Fabric and NeoForge boundaries: the
/// requested version must exist in the official listing, its Minecraft
/// mapping must equal the requested game version (an unattributable or
/// mismatched combination is reported, never substituted), and the
/// installer documents must agree with the requested identity. The
/// installer artifact is pinned by its official `.sha1` sidecar before
/// the plan is derived.
pub async fn resolve_forge_plan(
    managed: &ManagedPaths,
    endpoints: &ForgeMavenEndpoints,
    game: &MinecraftVersionId,
    loader: &ForgeVersionId,
    options: &DownloadOptions,
) -> Result<ForgePlan, ForgeResolutionError> {
    loader.require_reviewed()?;
    if loader.minecraft_version() != game.as_str() {
        return Err(ForgeResolutionError::UnknownVersion {
            minecraft_version: game.as_str().to_owned(),
            forge_version: loader.to_string(),
        });
    }
    let listing = fetch_forge_versions(endpoints, options).await?;
    if listing.find(loader).is_none() {
        return Err(ForgeResolutionError::UnknownVersion {
            minecraft_version: game.as_str().to_owned(),
            forge_version: loader.to_string(),
        });
    }

    let installer_url = endpoints.installer_url(loader);
    let installer_sha1 = fetch_sidecar_sha1(&installer_url, options).await?;
    let cache = ArtifactCache::new(managed.clone());
    let installer_source = Sha1ArtifactSource::https_or_loopback(
        installer_url.as_str(),
        &installer_sha1.as_hex(),
        None,
    )
    .map_err(|error| {
        ForgeResolutionError::Metadata(ForgeMetadataError::Malformed {
            reason: error.to_string(),
        })
    })?;
    let installer = cache
        .acquire_sha1(&installer_source, options)
        .await
        .map_err(ForgeResolutionError::Acquisition)?;

    let (profile, version) = read_installer_documents(&installer.path)?;
    let plan =
        ForgePlan::from_documents(&profile, &version, game, loader, endpoints, installer_sha1)?;
    Ok(plan)
}

/// Resolves one exact Minecraft + Forge combination into the complete
/// composed game plan.
pub async fn resolve_forge_game_plan(
    managed: &ManagedPaths,
    minecraft: &MetadataEndpoints,
    forge: &ForgeMavenEndpoints,
    game: &MinecraftVersionId,
    loader: &ForgeVersionId,
    platform: PlatformProfile,
    options: &DownloadOptions,
) -> Result<GameInstallPlan, ForgeGameResolutionError> {
    let minecraft_plan = crate::minecraft::resolve_install_plan(minecraft, game, platform, options)
        .await
        .map_err(ForgeGameResolutionError::Minecraft)?;
    let forge_plan = resolve_forge_plan(managed, forge, game, loader, options).await?;
    compose_forge_game_plan(minecraft_plan, forge_plan)
        .map_err(ForgeGameResolutionError::Composition)
}

/// Why one exact Minecraft + Forge combination could not be resolved.
#[derive(Debug)]
pub enum ForgeResolutionError {
    /// The transport to an official Forge source failed.
    Metadata(ForgeMetadataError),
    /// The requested Forge version is not in the official listing for
    /// that Minecraft version.
    UnknownVersion {
        minecraft_version: String,
        forge_version: String,
    },
    /// The verified installer acquisition failed.
    Acquisition(crate::cache::AcquisitionError),
    /// The installer documents could not be normalized into a plan.
    Plan(ForgePlanError),
}

impl fmt::Display for ForgeResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metadata(error) => write!(formatter, "{error}"),
            Self::UnknownVersion {
                minecraft_version,
                forge_version,
            } => write!(
                formatter,
                "Forge {forge_version} is not published for Minecraft {minecraft_version}"
            ),
            Self::Acquisition(error) => {
                write!(
                    formatter,
                    "the Forge installer could not be acquired: {error}"
                )
            }
            Self::Plan(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ForgeResolutionError {}

impl From<ForgeMetadataError> for ForgeResolutionError {
    fn from(error: ForgeMetadataError) -> Self {
        Self::Metadata(error)
    }
}

impl From<ForgePlanError> for ForgeResolutionError {
    fn from(error: ForgePlanError) -> Self {
        Self::Plan(error)
    }
}

/// Why one exact Minecraft + Forge game plan could not be composed.
#[derive(Debug)]
pub enum ForgeGameResolutionError {
    /// The Mojang base-game half failed to resolve.
    Minecraft(MinecraftResolutionError),
    /// The Forge half failed to resolve.
    Forge(ForgeResolutionError),
    /// The two halves could not be composed.
    Composition(crate::game::plan::CompositionError),
}

impl fmt::Display for ForgeGameResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Minecraft(error) => write!(formatter, "{error}"),
            Self::Forge(error) => write!(formatter, "{error}"),
            Self::Composition(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ForgeGameResolutionError {}

impl From<ForgeResolutionError> for ForgeGameResolutionError {
    fn from(error: ForgeResolutionError) -> Self {
        Self::Forge(error)
    }
}

#[cfg(test)]
mod compatibility_tests;

#[cfg(test)]
mod processor_chain_tests;
