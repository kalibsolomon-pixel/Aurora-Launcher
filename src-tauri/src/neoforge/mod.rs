//! NeoForge metadata resolution and install-plan composition.
//!
//! The pipeline owned by this module:
//!
//! ```text
//! exact Minecraft version + exact NeoForge version
//!         ↓ release listing (HTTPS discovery, no prior digest)
//! exact version lookup + Minecraft mapping cross-check
//!         ↓ installer jar (SHA-1-verified acquisition)
//! install_profile.json + version.json (bounded extraction, parse, validate)
//!         ↓ normalization
//! NeoForgePlan (Aurora-owned domain types)
//!         ↓ composition with MinecraftInstallPlan
//! GameInstallPlan
//! ```
//!
//! Everything here is planning except the one acquisition the plan itself
//! depends on: the installer jar is both the metadata document carrier and
//! a real artifact, so it is acquired through the verified SHA-1 store
//! before its embedded documents are read. Nothing is installed, extracted
//! into an instance, or launched by this module.

pub mod metadata;
pub mod mods;
pub mod plan;
pub mod processors;
pub mod versions;

use std::fmt;

use crate::cache::ArtifactCache;
use crate::downloads::DownloadOptions;
use crate::fabric::plan::GameInstallPlan;
use crate::minecraft::MinecraftResolutionError;
use crate::minecraft::metadata::{MetadataEndpoints, MinecraftVersionId};
use crate::minecraft::rules::PlatformProfile;
use crate::neoforge::metadata::{
    NeoForgeMavenEndpoints, NeoForgeMetadataError, NeoForgeVersionId, fetch_neoforge_versions,
    fetch_sidecar_sha1, read_installer_documents,
};
use crate::neoforge::plan::{NeoForgePlan, NeoForgePlanError, compose_neoforge_game_plan};
use crate::paths::ManagedPaths;

/// Resolves one exact Minecraft + NeoForge combination into a normalized
/// NeoForge plan.
///
/// Resolution is exact, mirroring the Fabric boundary: the requested
/// version must exist in the official release listing, its embedded
/// Minecraft mapping must equal the requested Minecraft version (an
/// unattributable or mismatched combination is reported, never
/// substituted), and the installer documents must agree with the requested
/// identity. The installer and universal artifacts are pinned by their
/// official SHA-1 sidecars before the plan is derived.
pub async fn resolve_neoforge_plan(
    managed: &ManagedPaths,
    endpoints: &NeoForgeMavenEndpoints,
    game: &MinecraftVersionId,
    loader: &NeoForgeVersionId,
    options: &DownloadOptions,
) -> Result<NeoForgePlan, NeoForgeResolutionError> {
    let listing = fetch_neoforge_versions(endpoints, options).await?;
    let Some(entry) = listing.find(loader) else {
        return Err(NeoForgeResolutionError::VersionNotFound {
            requested: loader.to_string(),
        });
    };
    if entry.minecraft_version() != game.as_str() {
        return Err(NeoForgeResolutionError::CombinationUnsupported {
            game: game.as_str().to_owned(),
            loader: loader.to_string(),
            addressed: entry.minecraft_version().to_owned(),
        });
    }

    let installer_url = endpoints.installer_url(loader);
    let installer_sha1 = fetch_sidecar_sha1(&installer_url, options).await?;
    let universal_sha1 = fetch_sidecar_sha1(&endpoints.universal_url(loader), options).await?;

    let cache = ArtifactCache::new(managed.clone());
    let installer_source = crate::downloads::Sha1ArtifactSource::https_or_loopback(
        installer_url.as_str(),
        &installer_sha1.as_hex(),
        None,
    )
    .map_err(|error| {
        NeoForgeResolutionError::Metadata(NeoForgeMetadataError::Malformed {
            reason: format!("the installer artifact source is invalid: {error}"),
        })
    })?;
    let installer = cache
        .acquire_sha1(&installer_source, options)
        .await
        .map_err(NeoForgeResolutionError::Acquisition)?;
    let (profile, version) = read_installer_documents(&installer.path)?;

    Ok(NeoForgePlan::from_documents(
        &profile,
        &version,
        game,
        loader,
        endpoints,
        installer_sha1,
        universal_sha1,
    )?)
}

/// Resolves and composes the complete Minecraft + NeoForge game plan.
pub async fn resolve_neoforge_game_plan(
    managed: &ManagedPaths,
    minecraft_endpoints: &MetadataEndpoints,
    neoforge_endpoints: &NeoForgeMavenEndpoints,
    game: &MinecraftVersionId,
    loader: &NeoForgeVersionId,
    platform: PlatformProfile,
    options: &DownloadOptions,
) -> Result<GameInstallPlan, NeoForgeGameResolutionError> {
    let minecraft =
        crate::minecraft::resolve_install_plan(minecraft_endpoints, game, platform, options)
            .await?;
    let neoforge =
        resolve_neoforge_plan(managed, neoforge_endpoints, game, loader, options).await?;
    let plan = compose_neoforge_game_plan(minecraft, neoforge)?;

    if std::env::var_os("AURORA_INSTALL_DIAGNOSTICS").is_some() {
        let loader = plan.loader().expect("NeoForge resolution");
        eprintln!(
            "[aurora-launcher] planned Minecraft {} + NeoForge {}: {} vanilla + {} NeoForge = {} libraries, Java {}, main class {}",
            plan.minecraft().minecraft_version(),
            loader.loader_version(),
            plan.vanilla_library_count(),
            plan.neoforge_library_count(),
            plan.libraries().len(),
            plan.java().major_version(),
            plan.main_class(),
        );
    }

    Ok(plan)
}

/// A failure anywhere in the combination → NeoForge plan pipeline.
#[derive(Debug)]
pub enum NeoForgeResolutionError {
    Metadata(NeoForgeMetadataError),
    /// The requested NeoForge version does not exist in official metadata.
    VersionNotFound {
        requested: String,
    },
    /// The version exists but addresses a different Minecraft version.
    CombinationUnsupported {
        game: String,
        loader: String,
        addressed: String,
    },
    /// The verified acquisition of the installer artifact failed.
    Acquisition(crate::cache::AcquisitionError),
    Planning(NeoForgePlanError),
}

impl fmt::Display for NeoForgeResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metadata(error) => write!(formatter, "{error}"),
            Self::VersionNotFound { requested } => write!(
                formatter,
                "NeoForge version '{requested}' does not exist in official NeoForge metadata"
            ),
            Self::CombinationUnsupported {
                game,
                loader,
                addressed,
            } => write!(
                formatter,
                "official NeoForge metadata addresses Minecraft '{addressed}' for NeoForge '{loader}', not '{game}'"
            ),
            Self::Acquisition(error) => write!(
                formatter,
                "the verified acquisition of the NeoForge installer failed: {error}"
            ),
            Self::Planning(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for NeoForgeResolutionError {}

impl From<NeoForgeMetadataError> for NeoForgeResolutionError {
    fn from(error: NeoForgeMetadataError) -> Self {
        Self::Metadata(error)
    }
}

impl From<NeoForgePlanError> for NeoForgeResolutionError {
    fn from(error: NeoForgePlanError) -> Self {
        Self::Planning(error)
    }
}

/// A failure while resolving the composed Minecraft + NeoForge plan.
#[derive(Debug)]
pub enum NeoForgeGameResolutionError {
    Minecraft(MinecraftResolutionError),
    NeoForge(NeoForgeResolutionError),
    Composition(crate::fabric::plan::CompositionError),
}

impl fmt::Display for NeoForgeGameResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Minecraft(error) => write!(formatter, "{error}"),
            Self::NeoForge(error) => write!(formatter, "{error}"),
            Self::Composition(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for NeoForgeGameResolutionError {}

impl From<MinecraftResolutionError> for NeoForgeGameResolutionError {
    fn from(error: MinecraftResolutionError) -> Self {
        Self::Minecraft(error)
    }
}

impl From<NeoForgeResolutionError> for NeoForgeGameResolutionError {
    fn from(error: NeoForgeResolutionError) -> Self {
        Self::NeoForge(error)
    }
}

impl From<crate::fabric::plan::CompositionError> for NeoForgeGameResolutionError {
    fn from(error: crate::fabric::plan::CompositionError) -> Self {
        Self::Composition(error)
    }
}
