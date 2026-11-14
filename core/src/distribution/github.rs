//! Published Client discovery, independent of the Launcher release cycle.
//! GitHub provides the expected SHA-256; only verified store bytes supply mod
//! compatibility. Existing environment policy and exact installed pins remain.

use serde::Deserialize;

use super::ReleaseManifest;
use crate::downloads::{ArtifactSource, DownloadOptions};
use crate::paths::ManagedPaths;
use crate::updates::client::ClientUpdateError;

pub const RELEASES_URL: &str =
    "https://api.github.com/repos/kalibsolomon-pixel/Aurora-Client/releases?per_page=30";
const REPOSITORY: &str = "https://github.com/kalibsolomon-pixel/Aurora-Client";
const MAX_METADATA_BYTES: usize = 1024 * 1024;
const MAX_JAR_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Deserialize)]
pub(crate) struct GithubRelease {
    pub(crate) tag_name: String,
    pub(crate) html_url: String,
    pub(crate) draft: bool,
    pub(crate) prerelease: bool,
    pub(crate) published_at: Option<String>,
    #[serde(default)]
    pub(crate) body: Option<String>,
    pub(crate) assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
pub(crate) struct GithubAsset {
    pub(crate) name: String,
    pub(crate) browser_download_url: String,
    pub(crate) size: u64,
    pub(crate) digest: Option<String>,
    pub(crate) state: String,
}

fn invalid(reason: impl Into<String>) -> ClientUpdateError {
    ClientUpdateError::ManifestInvalid(reason.into())
}

/// Recognizes historical JARs and version-specific runtime JARs. The suffix is
/// only an identity assertion; verified Fabric metadata decides compatibility.
pub(crate) fn artifact_minecraft<'a>(name: &'a str, version: &str) -> Option<Option<&'a str>> {
    let suffix = name.strip_prefix(&format!("aurora-{version}"))?;
    if suffix == ".jar" {
        return Some(None);
    }
    let minecraft = suffix.strip_prefix("-mc")?.strip_suffix(".jar")?;
    if minecraft.split('.').count() < 2
        || minecraft
            .split('.')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    Some(Some(minecraft))
}

impl GithubRelease {
    pub(crate) fn artifacts(
        &self,
    ) -> Result<(String, Vec<(&GithubAsset, ArtifactSource)>), ClientUpdateError> {
        let version = self
            .tag_name
            .strip_prefix('v')
            .ok_or_else(|| invalid("The Client release tag must name a version."))?;
        let parsed = crate::updates::parse_release_version(version)
            .filter(|version| version.pre.is_empty() && version.build.is_empty())
            .ok_or_else(|| invalid("The Client release version is invalid."))?;
        if parsed.to_string() != version
            || self.draft
            || self.prerelease
            || self.published_at.as_deref().is_none_or(str::is_empty)
            || self.html_url != format!("{REPOSITORY}/releases/tag/{}", self.tag_name)
        {
            return Err(invalid(
                "Only an official published stable Client release is eligible.",
            ));
        }
        let matches: Vec<_> = self
            .assets
            .iter()
            .filter(|asset| artifact_minecraft(&asset.name, version).is_some())
            .collect();
        if matches.is_empty() {
            return Err(invalid("The published Client JAR is missing."));
        }
        let mut environments = std::collections::HashSet::new();
        let mut artifacts = Vec::new();
        for asset in &matches {
            let environment = artifact_minecraft(&asset.name, version).unwrap();
            if !environments.insert(environment) || (matches.len() > 1 && environment.is_none()) {
                return Err(invalid(
                    "The published Client JARs have ambiguous Minecraft identities.",
                ));
            }
            let name = &asset.name;
            if asset.state != "uploaded"
                || asset.size == 0
                || asset.size > MAX_JAR_BYTES
                || asset.browser_download_url
                    != format!("{REPOSITORY}/releases/download/{}/{name}", self.tag_name)
            {
                return Err(invalid(
                    "The published Client artifact identity is invalid.",
                ));
            }
            let digest = asset
                .digest
                .as_deref()
                .and_then(|digest| digest.strip_prefix("sha256:"))
                .ok_or_else(|| {
                    invalid("GitHub did not publish an expected SHA-256 for this Client JAR.")
                })?;
            let source =
                ArtifactSource::https(&asset.browser_download_url, digest, Some(asset.size))
                    .map_err(|error| invalid(error.to_string()))?;
            artifacts.push((*asset, source));
        }
        Ok((version.to_owned(), artifacts))
    }
    #[cfg(test)]
    fn artifact(&self) -> Result<(String, ArtifactSource), ClientUpdateError> {
        let (version, mut assets) = self.artifacts()?;
        if assets.len() != 1 {
            return Err(invalid("Expected one test artifact."));
        }
        Ok((version, assets.remove(0).1))
    }
}

/// Fetches bounded official releases and selects the newest compatible Client
/// for each managed environment from digest-verified Fabric metadata.
/// Only cache resources change; no instance, user content or pin is modified.
pub async fn discover(
    managed: &ManagedPaths,
    options: &DownloadOptions,
) -> Result<ReleaseManifest, ClientUpdateError> {
    discover_at(managed, options, RELEASES_URL).await
}

async fn discover_at(
    managed: &ManagedPaths,
    options: &DownloadOptions,
    endpoint: &str,
) -> Result<ReleaseManifest, ClientUpdateError> {
    let url =
        url::Url::parse(endpoint).map_err(|_| invalid("Invalid Client discovery endpoint."))?;
    if endpoint != RELEASES_URL
        && !(url.scheme() == "http" && crate::downloads::is_loopback_host(&url))
    {
        return Err(invalid(
            "Client discovery must use the pinned official GitHub endpoint.",
        ));
    }
    let mut response = crate::downloads::build_client(options)
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| {
            ClientUpdateError::ManifestUnavailable("Could not reach GitHub Client releases.".into())
        })?;
    if !response.status().is_success() {
        return Err(ClientUpdateError::ManifestUnavailable(format!(
            "GitHub Client releases answered HTTP {}.",
            response.status().as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_METADATA_BYTES as u64)
    {
        return Err(invalid(
            "GitHub Client release metadata exceeds the size limit.",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        ClientUpdateError::ManifestUnavailable("Could not read GitHub Client releases.".into())
    })? {
        if bytes.len() + chunk.len() > MAX_METADATA_BYTES {
            return Err(invalid(
                "GitHub Client release metadata exceeds the size limit.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut releases: Vec<GithubRelease> = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("GitHub Client release metadata is malformed."))?;
    if releases.len() > 30 {
        return Err(invalid("Too many Client release records."));
    }
    releases.sort_by_key(|release| {
        std::cmp::Reverse(crate::updates::parse_release_version(
            release.tag_name.trim_start_matches('v'),
        ))
    });
    let profiles = super::production_manifest()?;
    let environments: std::collections::HashSet<_> = profiles
        .releases()
        .iter()
        .map(|profile| profile.minecraft_version())
        .collect();
    let mut selected = std::collections::HashSet::new();
    let mut manifest = ReleaseManifest::from_json(r#"{"schemaVersion":1,"releases":[]}"#)?;
    for release in releases {
        if release.draft || release.prerelease || release.published_at.is_none() {
            continue;
        }
        // Historical releases without a runtime JAR remain unavailable.
        if release.assets.is_empty() {
            continue;
        }
        let (version, artifacts) = release.artifacts()?;
        for (asset, source) in artifacts {
            if artifact_minecraft(&asset.name, &version)
                .flatten()
                .is_some_and(|minecraft| {
                    !environments.contains(minecraft) || selected.contains(minecraft)
                })
            {
                continue;
            }
            match discover_release(managed, options, &release, &version, asset, &source).await {
                Ok(found) => {
                    let candidate = &found.releases()[0];
                    if selected.insert(candidate.minecraft_version().to_owned()) {
                        manifest = manifest.with_release(candidate.clone())?;
                    }
                }
                Err(ClientUpdateError::Incompatible(_)) => continue,
                Err(error) => return Err(error),
            }
        }
        if selected.len() == environments.len() {
            break;
        }
    }
    if manifest.releases().is_empty() {
        return Err(ClientUpdateError::Incompatible(
            "No published Aurora Client supports the managed environments.".into(),
        ));
    }
    Ok(manifest)
}

async fn discover_release(
    managed: &ManagedPaths,
    options: &DownloadOptions,
    release: &GithubRelease,
    version: &str,
    asset: &GithubAsset,
    source: &ArtifactSource,
) -> Result<ReleaseManifest, ClientUpdateError> {
    let verified = crate::cache::ArtifactCache::new(managed.clone())
        .acquire_with(source, options)
        .await
        .map_err(|error| {
            invalid(format!(
                "The published Client JAR could not be verified: {error}"
            ))
        })?;
    let (metadata, warnings) =
        crate::instance_mods::inspect_fabric_metadata(&verified.path, verified.bytes);
    let metadata =
        metadata.ok_or_else(|| invalid("The verified Client JAR has no valid Fabric metadata."))?;
    if !warnings.is_empty()
        || metadata.id != "aurora"
        || metadata.version.as_deref() != Some(version)
    {
        return Err(invalid(
            "The verified JAR does not match the published Aurora Client identity.",
        ));
    }
    normalize(release, version, &metadata, asset, source)
}

fn normalize(
    release: &GithubRelease,
    version: &str,
    metadata: &crate::instance_mods::ModMetadata,
    asset: &GithubAsset,
    artifact: &ArtifactSource,
) -> Result<ReleaseManifest, ClientUpdateError> {
    if ["minecraft", "fabricloader", "java"]
        .iter()
        .any(|required| {
            !metadata
                .depends
                .iter()
                .any(|dependency| dependency.mod_id == *required)
        })
    {
        return Err(invalid(
            "The verified Client must declare Minecraft, Fabric Loader and Java requirements.",
        ));
    }
    let profiles = super::production_manifest()?;
    for profile in profiles.releases() {
        if artifact_minecraft(&asset.name, version)
            .flatten()
            .is_some_and(|minecraft| minecraft != profile.minecraft_version())
        {
            continue;
        }
        let compatible = metadata.depends.iter().all(|dependency| {
            let value = match dependency.mod_id.as_str() {
                "minecraft" => Some(profile.minecraft_version().to_owned()),
                "fabricloader" => Some(profile.fabric_loader_version().to_owned()),
                "java" => Some(profile.java().major_version().to_string()),
                "fabric-api" => profile.fabric_api().map(|api| api.version().to_owned()),
                id => metadata.nested_mod_versions.get(id).cloned().flatten(),
            };
            value.is_some_and(|value| {
                crate::fabric::versions::satisfies(&value, &dependency.requirement).unwrap_or(false)
            })
        });
        if !compatible
            || metadata.environment.as_deref() == Some("server")
            || metadata
                .mixin_java_requirements
                .iter()
                .any(|mixin| mixin.java_major > profile.java().major_version())
        {
            continue;
        }
        let mut value = serde_json::to_value(profile)
            .map_err(|_| invalid("Could not normalize Client compatibility."))?;
        value["auroraVersion"] = version.into();
        value["artifact"] = serde_json::json!({"url": artifact.url().as_str(), "sha256": artifact.sha256().as_hex(), "sizeBytes": artifact.size_bytes()});
        // Untrusted release copy is plain text, bounded independently of the API.
        let notes: String = release
            .body
            .as_deref()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
            .take(2000)
            .collect();
        value["notes"] = notes.into();
        return ReleaseManifest::from_json(
            &serde_json::json!({"schemaVersion":1,"releases":[value]}).to_string(),
        )
        .map_err(Into::into);
    }
    Err(ClientUpdateError::Incompatible("The latest Aurora Client release does not support this Launcher's managed Minecraft/Fabric/Java environment.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release() -> serde_json::Value {
        serde_json::json!({"tag_name":"v3.1.0","html_url":format!("{REPOSITORY}/releases/tag/v3.1.0"),"draft":false,"prerelease":false,"published_at":"2026-10-20T07:00:28Z","body":"Aurora Client 3.1.0","assets":[{"name":"aurora-3.1.0.jar","browser_download_url":format!("{REPOSITORY}/releases/download/v3.1.0/aurora-3.1.0.jar"),"size":3207919,"digest":format!("sha256:{}","a".repeat(64)),"state":"uploaded"}]})
    }
    #[test]
    fn published_github_version_is_not_the_bundled_client_version() {
        let value: GithubRelease = serde_json::from_value(release()).unwrap();
        let (version, artifact) = value.artifact().unwrap();
        assert_eq!(version, "3.1.0");
        assert_eq!(artifact.size_bytes(), Some(3207919));
        assert_ne!(
            version,
            super::super::production_manifest().unwrap().releases()[0].aurora_version()
        );
    }
    #[test]
    fn version_specific_names_are_exact_and_ambiguous_jars_fail() {
        for minecraft in ["26.1.2", "26.2", "26.3"] {
            let mut document = release();
            let name = format!("aurora-3.1.0-mc{minecraft}.jar");
            document["assets"][0]["name"] = name.clone().into();
            document["assets"][0]["browser_download_url"] =
                format!("{REPOSITORY}/releases/download/v3.1.0/{name}").into();
            let parsed: GithubRelease = serde_json::from_value(document.clone()).unwrap();
            assert!(parsed.artifact().is_ok());
            assert_eq!(artifact_minecraft(&name, "3.1.0"), Some(Some(minecraft)));
            document["assets"]
                .as_array_mut()
                .unwrap()
                .push(release()["assets"][0].clone());
            assert!(
                serde_json::from_value::<GithubRelease>(document)
                    .unwrap()
                    .artifact()
                    .is_err()
            );
        }
        for name in [
            "aurora-3.1.0-mc.jar",
            "aurora-3.1.0-mc26..3.jar",
            "aurora-3.1.0-mc26.3-sources.jar",
            "aurora-3.1.0-mc../26.3.jar",
            "aurora-3.1.00-mc26.3.jar",
            "aurora-3.1.0-mc26.3.jar.exe",
        ] {
            assert!(artifact_minecraft(name, "3.1.0").is_none(), "{name}");
        }
    }
    #[test]
    fn unpublished_foreign_missing_digest_or_ambiguous_artifacts_fail() {
        for mutate in 0..9 {
            let mut value = release();
            match mutate {
                0 => value["draft"] = true.into(),
                1 => value["prerelease"] = true.into(),
                2 => value["published_at"] = serde_json::Value::Null,
                3 => value["assets"][0]["digest"] = serde_json::Value::Null,
                4 => {
                    value["assets"][0]["browser_download_url"] =
                        "https://example.com/aurora.jar".into()
                }
                5 => value["assets"][0]["size"] = 0.into(),
                6 => value["assets"][0]["digest"] = "sha256:invalid".into(),
                7 => value["assets"]
                    .as_array_mut()
                    .unwrap()
                    .push(release()["assets"][0].clone()),
                _ => value["tag_name"] = "v3.1.0-rc.1".into(),
            }
            assert!(
                serde_json::from_value::<GithubRelease>(value)
                    .unwrap()
                    .artifact()
                    .is_err(),
                "case {mutate}"
            );
        }
    }
    #[test]
    fn normalized_release_uses_verified_dependencies_and_remote_artifact() {
        let release: GithubRelease = serde_json::from_value(release()).unwrap();
        let metadata = crate::instance_mods::ModMetadata {
            id: "aurora".into(),
            name: None,
            version: Some("3.1.0".into()),
            description: None,
            authors: vec![],
            environment: Some("client".into()),
            depends: vec![
                crate::instance_mods::ModRelation {
                    mod_id: "minecraft".into(),
                    requirement: "~1.21.11".into(),
                },
                crate::instance_mods::ModRelation {
                    mod_id: "fabricloader".into(),
                    requirement: ">=0.16.0".into(),
                },
                crate::instance_mods::ModRelation {
                    mod_id: "java".into(),
                    requirement: ">=21".into(),
                },
            ],
            recommends: vec![],
            suggests: vec![],
            conflicts: vec![],
            breaks: vec![],
            has_declared_icon: false,
            nested_mod_ids: vec![],
            nested_mod_versions: Default::default(),
            mixin_java_requirements: vec![],
        };
        let manifest = normalize(
            &release,
            "3.1.0",
            &metadata,
            &release.assets[0],
            &release.artifact().unwrap().1,
        )
        .unwrap();
        let normalized = &manifest.releases()[0];
        assert_eq!(normalized.aurora_version(), "3.1.0");
        assert_eq!(normalized.fabric_loader_version(), "0.19.5");
        assert!(normalized.artifact().url().contains("/v3.1.0/"));
        let mut incompatible = metadata;
        incompatible.depends[0].requirement = "~1.22.0".into();
        assert!(
            normalize(
                &release,
                "3.1.0",
                &incompatible,
                &release.assets[0],
                &release.artifact().unwrap().1
            )
            .is_err()
        );
    }
    #[tokio::test]
    async fn github_errors_and_untrusted_endpoints_fail_without_instance_mutation() {
        let managed = ManagedPaths::from_app_local_data_dir(
            std::env::temp_dir().join(format!("aurora-github-error-{}", uuid::Uuid::new_v4())),
        )
        .unwrap();
        assert!(
            discover_at(
                &managed,
                &DownloadOptions::default(),
                "https://example.com/latest"
            )
            .await
            .is_err()
        );
        let server = crate::test_support::TestServer::spawn(std::sync::Arc::new(|_| {
            crate::test_support::TestResponse::status(403)
        }));
        assert!(
            discover_at(
                &managed,
                &DownloadOptions::default(),
                &format!("{}/latest", server.base_url())
            )
            .await
            .is_err()
        );
        assert!(!managed.instance_registry_file().exists());
    }

    #[tokio::test]
    async fn loopback_discovery_verifies_cached_jar_identity_before_offering_remote_version() {
        use sha2::Digest as _;
        use std::io::Write;
        for id in ["aurora", "foreign"] {
            let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            archive
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(serde_json::json!({"schemaVersion":1,"id":id,"version":"3.1.0","environment":"client","depends":{"minecraft":"~1.21.11","fabricloader":">=0.16.0","java":">=21","fabric-api":"*"}}).to_string().as_bytes()).unwrap();
            let jar = archive.finish().unwrap().into_inner();
            let digest = format!("{:x}", sha2::Sha256::digest(&jar));
            let managed = ManagedPaths::from_app_local_data_dir(
                std::env::temp_dir()
                    .join(format!("aurora-github-loopback-{}", uuid::Uuid::new_v4())),
            )
            .unwrap();
            let path = crate::cache::ArtifactCache::new(managed.clone())
                .verified_path(&crate::integrity::ArtifactDigest::parse(&digest).unwrap());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &jar).unwrap();
            let mut document = release();
            document["assets"][0]["size"] = jar.len().into();
            document["assets"][0]["digest"] = format!("sha256:{digest}").into();
            let body = serde_json::json!([document]).to_string().into_bytes();
            let server = crate::test_support::TestServer::spawn(std::sync::Arc::new(move |_| {
                crate::test_support::TestResponse::ok(&body)
            }));
            let found = discover_at(
                &managed,
                &DownloadOptions::default(),
                &format!("{}/latest", server.base_url()),
            )
            .await;
            if id == "aurora" {
                let manifest = found.unwrap();
                assert_eq!(manifest.releases()[0].aurora_version(), "3.1.0");
                assert_eq!(manifest.releases()[0].artifact().sha256(), digest);
            } else {
                assert!(found.is_err());
            }
            assert_eq!(server.request_count(), 1);
            assert!(!managed.instance_registry_file().exists());
            assert_eq!(std::fs::read(&path).unwrap(), jar);
        }
    }

    #[tokio::test]
    async fn discovery_selects_newest_verified_jar_per_minecraft_and_preserves_legacy() {
        use sha2::Digest as _;
        use std::io::Write;
        let managed = ManagedPaths::from_app_local_data_dir(
            std::env::temp_dir().join(format!("aurora-github-matrix-{}", uuid::Uuid::new_v4())),
        )
        .unwrap();
        let mut documents = Vec::new();
        for (version, minecraft, java, named) in [
            ("3.2.2", "26.1.2", 25, true),
            ("3.2.1", "1.21.11", 21, false),
            ("3.3.0", "26.3", 25, true),
            ("3.2.3", "26.2", 25, true),
            ("3.3.1", "26.3", 25, true),
            ("3.4.0", "26.3", 25, true),
            ("3.4.0", "26.1.2", 25, true),
            ("3.4.0", "26.2", 25, true),
        ] {
            let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            archive
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(serde_json::json!({"schemaVersion":1,"id":"aurora","version":version,"environment":"client","depends":{"minecraft":format!("~{minecraft}"),"fabricloader":">=0.16.0","java":format!(">={java}"),"fabric-api":"*"}}).to_string().as_bytes()).unwrap();
            let jar = archive.finish().unwrap().into_inner();
            let digest = format!("{:x}", sha2::Sha256::digest(&jar));
            let path = crate::cache::ArtifactCache::new(managed.clone())
                .verified_path(&crate::integrity::ArtifactDigest::parse(&digest).unwrap());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, &jar).unwrap();
            let name = if named {
                format!("aurora-{version}-mc{minecraft}.jar")
            } else {
                format!("aurora-{version}.jar")
            };
            documents.push(serde_json::json!({"tag_name":format!("v{version}"),"html_url":format!("{REPOSITORY}/releases/tag/v{version}"),"draft":false,"prerelease":false,"published_at":"2026-11-11T00:00:00Z","assets":[{"name":name,"browser_download_url":format!("{REPOSITORY}/releases/download/v{version}/{name}"),"size":jar.len(),"digest":format!("sha256:{digest}"),"state":"uploaded"}]}));
        }
        // GitHub publishes all current environments together under one tag.
        let mut bundle = documents.pop().unwrap();
        for _ in 0..2 {
            let sibling = documents.pop().unwrap();
            bundle["assets"]
                .as_array_mut()
                .unwrap()
                .extend(sibling["assets"].as_array().unwrap().iter().cloned());
        }
        documents.push(bundle);
        let mut draft = documents[0].clone();
        draft["draft"] = true.into();
        draft["tag_name"] = "release-manifest".into();
        documents.push(draft);
        let body = serde_json::to_vec(&documents).unwrap();
        let server = crate::test_support::TestServer::spawn(std::sync::Arc::new(move |_| {
            crate::test_support::TestResponse::ok(&body)
        }));
        let found = discover_at(
            &managed,
            &DownloadOptions::default(),
            &format!("{}/releases", server.base_url()),
        )
        .await
        .unwrap();
        assert_eq!(found.releases().len(), 4);
        for (minecraft, version, java) in [
            ("26.1.2", "3.4.0", 25),
            ("26.2", "3.4.0", 25),
            ("26.3", "3.4.0", 25),
            ("1.21.11", "3.2.1", 21),
        ] {
            let candidate = found.latest_compatible(minecraft, None).unwrap();
            assert_eq!(candidate.aurora_version(), version);
            assert_eq!(candidate.java().major_version(), java);
        }
        assert!(found.latest_compatible("26.1", None).is_none());
        assert_eq!(server.request_count(), 1);
        assert!(!managed.instance_registry_file().exists());
    }
}
