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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestResponse, TestServer};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn sha1_hex(bytes: &[u8]) -> String {
        crate::integrity::Sha1Digest::compute(bytes).as_hex()
    }

    /// Builds one synthetic installer jar: the two embedded documents plus
    /// the binpatch bundle the processor references.
    fn installer_jar(profile_json: &str, version_json: &str) -> Vec<u8> {
        use std::io::Write as _;
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            for (name, bytes) in [
                ("install_profile.json", profile_json.as_bytes().to_vec()),
                ("version.json", version_json.as_bytes().to_vec()),
                ("data/client.lzma", b"synthetic binpatch bundle".to_vec()),
            ] {
                writer
                    .start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(&bytes).unwrap();
            }
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    fn representative_documents(base: &str) -> (String, String) {
        let profile = format!(
            r#"{{
              "spec": 1,
              "profile": "NeoForge",
              "version": "neoforge-26.2.0.88",
              "minecraft": "26.2",
              "json": "/version.json",
              "data": {{
                "BINPATCH": {{"client": "/data/client.lzma", "server": "/data/client.lzma"}},
                "PATCHED": {{"client": "[net.neoforged:minecraft-client-patched:26.2.0.88]", "server": "[net.neoforged:minecraft-server-patched:26.2.0.88]"}}
              }},
              "processors": [
                {{"sides": ["server"], "jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": ["net.neoforged.installertools:installertools:4.0.17:fatjar"], "args": ["--task", "EXTRACT_FILES"]}},
                {{"jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": ["net.neoforged.installertools:installertools:4.0.17:fatjar"], "args": ["--task", "PROCESS_MINECRAFT_JAR", "--input", "{{MINECRAFT_JAR}}", "--output", "{{PATCHED}}", "--apply-patches", "{{BINPATCH}}"]}}
              ],
              "libraries": [
                {{"name": "net.neoforged.installertools:installertools:4.0.17:fatjar", "downloads": {{"artifact": {{"path": "net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar", "url": "{base}/neoforge-maven/net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar", "sha1": "{}", "size": 100000}}}}}}
              ]
            }}"#,
            sha1_hex(b"synthetic installertools fatjar"),
        );
        let version = format!(
            r#"{{
              "id": "neoforge-26.2.0.88",
              "inheritsFrom": "26.2",
              "mainClass": "net.neoforged.fml.startup.Client",
              "arguments": {{
                "jvm": ["-DlibraryDirectory=${{library_directory}}"],
                "game": ["--fml.neoForgeVersion", "26.2.0.88", "--fml.mcVersion", "26.2"]
              }},
              "libraries": [
                {{"name": "net.neoforged.fancymodloader:loader:11.0.16", "downloads": {{"artifact": {{"path": "net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "url": "{base}/neoforge-maven/net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "sha1": "{}", "size": 669160}}}}}}
              ]
            }}"#,
            sha1_hex(b"synthetic fml loader jar"),
        );
        (profile, version)
    }

    fn listing_body() -> String {
        r#"{"isSnapshot":false,"versions":["21.1.252","26.2.0.88","26.2.0.80-beta","26.3.0.40-beta","0.25w14craftmine.3-beta"]}"#.to_owned()
    }

    fn spawn_neoforge_server(listing_status: u16) -> (TestServer, BTreeMap<String, Vec<u8>>) {
        // Phase one: a mutable body map so the installer jar can embed the
        // live loopback base once the server exists.
        let bodies = Arc::new(std::sync::Mutex::new(BTreeMap::<String, Vec<u8>>::new()));
        bodies.lock().unwrap().insert(
            "/api/maven/versions/releases/net/neoforged/neoforge".to_owned(),
            listing_body().into_bytes(),
        );
        bodies.lock().unwrap().insert(
            "/neoforge-maven/net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar".to_owned(),
            b"synthetic installertools fatjar".to_vec(),
        );
        bodies.lock().unwrap().insert(
            "/neoforge-maven/net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar"
                .to_owned(),
            b"synthetic fml loader jar".to_vec(),
        );

        let bodies_for_handler = Arc::clone(&bodies);
        let server = TestServer::spawn(Arc::new(move |request| {
            if request.path == "/api/maven/versions/releases/net/neoforged/neoforge"
                && listing_status != 200
            {
                return TestResponse::status(listing_status);
            }
            let bodies = bodies_for_handler.lock().unwrap();
            match bodies.get(&request.path) {
                Some(body) => TestResponse::ok(body),
                None => TestResponse::status(404),
            }
        }));

        // Phase two: rebuild the installer with the live base URL, embed it
        // in the map, and publish its sidecar digests.
        let base = server.base_url().to_owned();
        let (profile, version) = representative_documents(&base);
        let installer = installer_jar(&profile, &version);
        let universal = b"synthetic universal jar".to_vec();
        let installer_path =
            "/releases/net/neoforged/neoforge/26.2.0.88/neoforge-26.2.0.88-installer.jar";
        let universal_path =
            "/releases/net/neoforged/neoforge/26.2.0.88/neoforge-26.2.0.88-universal.jar";
        let mut map = bodies.lock().unwrap();
        map.insert(installer_path.to_owned(), installer.clone());
        map.insert(
            format!("{installer_path}.sha1"),
            sha1_hex(&installer).into_bytes(),
        );
        map.insert(universal_path.to_owned(), universal.clone());
        map.insert(
            format!("{universal_path}.sha1"),
            sha1_hex(&universal).into_bytes(),
        );
        drop(map);
        let snapshot = bodies.lock().unwrap().clone();
        (server, snapshot)
    }

    fn managed_root(name: &str) -> (crate::paths::ManagedPaths, std::path::PathBuf) {
        let root = std::env::temp_dir()
            .join("aurora-neoforge-resolution-tests")
            .join(std::process::id().to_string())
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let managed = crate::paths::ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        (managed, root)
    }

    #[tokio::test]
    async fn an_exact_combination_resolves_into_a_deterministic_plan() {
        let (server, _) = spawn_neoforge_server(200);
        let (managed, root) = managed_root("exact");
        let endpoints = metadata::NeoForgeMavenEndpoints::loopback_for_testing(server.base_url());
        let game = crate::minecraft::metadata::MinecraftVersionId::new("26.2").unwrap();
        let loader = metadata::NeoForgeVersionId::new("26.2.0.88").unwrap();
        let options = test_options();

        let first = resolve_neoforge_plan(&managed, &endpoints, &game, &loader, &options)
            .await
            .unwrap();
        let second = resolve_neoforge_plan(&managed, &endpoints, &game, &loader, &options)
            .await
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.neoforge_version(), "26.2.0.88");
        assert_eq!(first.version_id(), "neoforge-26.2.0.88");
        assert_eq!(first.main_class(), "net.neoforged.fml.startup.Client");
        assert_eq!(first.libraries().len(), 1);
        assert_eq!(first.installer_libraries().len(), 1);
        assert_eq!(first.processors().len(), 1);
        // The plan's URLs are pinned to the resolved endpoint base; the
        // documents the frontend never supplies cannot forge them.
        assert!(
            first
                .installer()
                .url()
                .as_str()
                .starts_with(server.base_url())
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn an_unknown_version_is_reported_not_substituted() {
        let (server, _) = spawn_neoforge_server(200);
        let (managed, root) = managed_root("unknown");
        let endpoints = metadata::NeoForgeMavenEndpoints::loopback_for_testing(server.base_url());
        let game = crate::minecraft::metadata::MinecraftVersionId::new("26.2").unwrap();
        let loader = metadata::NeoForgeVersionId::new("26.9.9.9").unwrap();
        let error = resolve_neoforge_plan(&managed, &endpoints, &game, &loader, &test_options())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            NeoForgeResolutionError::VersionNotFound { .. }
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_minecraft_mapping_mismatch_is_rejected() {
        let (server, _) = spawn_neoforge_server(200);
        let (managed, root) = managed_root("mismatch");
        let endpoints = metadata::NeoForgeMavenEndpoints::loopback_for_testing(server.base_url());
        // 21.1.252 exists in the listing but addresses Minecraft 1.21.1.
        let game = crate::minecraft::metadata::MinecraftVersionId::new("26.2").unwrap();
        let loader = metadata::NeoForgeVersionId::new("21.1.252").unwrap();
        let error = resolve_neoforge_plan(&managed, &endpoints, &game, &loader, &test_options())
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                NeoForgeResolutionError::CombinationUnsupported { .. }
            ),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn metadata_unavailability_fails_safely() {
        let (server, _) = spawn_neoforge_server(503);
        let (managed, root) = managed_root("unavailable");
        let endpoints = metadata::NeoForgeMavenEndpoints::loopback_for_testing(server.base_url());
        let game = crate::minecraft::metadata::MinecraftVersionId::new("26.2").unwrap();
        let loader = metadata::NeoForgeVersionId::new("26.2.0.88").unwrap();
        let error = resolve_neoforge_plan(&managed, &endpoints, &game, &loader, &test_options())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            NeoForgeResolutionError::Metadata(metadata::NeoForgeMetadataError::HttpStatus {
                status: 503
            })
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    fn test_options() -> crate::downloads::DownloadOptions {
        crate::downloads::DownloadOptions {
            connect_timeout: std::time::Duration::from_secs(5),
            idle_read_timeout: std::time::Duration::from_secs(5),
            max_redirects: crate::downloads::MAX_REDIRECTS,
        }
    }
}
