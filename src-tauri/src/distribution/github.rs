//! Published Client discovery, independent of the Launcher release cycle.
//! GitHub provides the expected SHA-256; only verified store bytes supply mod
//! compatibility. Existing environment policy and exact installed pins remain.

use serde::Deserialize;

use super::ReleaseManifest;
use crate::downloads::{ArtifactSource, DownloadOptions};
use crate::paths::ManagedPaths;
use crate::updates::client::ClientUpdateError;

pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/kalibsolomon-pixel/Aurora-Client/releases/latest";
const REPOSITORY: &str = "https://github.com/kalibsolomon-pixel/Aurora-Client";
const MAX_METADATA_BYTES: usize = 1024 * 1024;
const MAX_JAR_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    #[serde(default)]
    body: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
    state: String,
}

fn invalid(reason: impl Into<String>) -> ClientUpdateError {
    ClientUpdateError::ManifestInvalid(reason.into())
}

impl GithubRelease {
    fn artifact(&self) -> Result<(String, ArtifactSource), ClientUpdateError> {
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
        let name = format!("aurora-{version}.jar");
        let mut matches = self.assets.iter().filter(|asset| asset.name == name);
        let asset = matches
            .next()
            .ok_or_else(|| invalid("The published Client JAR is missing."))?;
        if matches.next().is_some()
            || asset.state != "uploaded"
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
        let source = ArtifactSource::https(&asset.browser_download_url, digest, Some(asset.size))
            .map_err(|error| invalid(error.to_string()))?;
        Ok((version.to_owned(), source))
    }
}

/// Fetches one bounded official release, verifies its JAR through the trusted
/// cache and derives a normalized release from its actual Fabric dependencies.
/// Only cache resources change; no instance, user content or pin is modified.
pub async fn discover(
    managed: &ManagedPaths,
    options: &DownloadOptions,
) -> Result<ReleaseManifest, ClientUpdateError> {
    discover_at(managed, options, LATEST_RELEASE_URL).await
}

async fn discover_at(
    managed: &ManagedPaths,
    options: &DownloadOptions,
    endpoint: &str,
) -> Result<ReleaseManifest, ClientUpdateError> {
    let url =
        url::Url::parse(endpoint).map_err(|_| invalid("Invalid Client discovery endpoint."))?;
    if endpoint != LATEST_RELEASE_URL
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
    let release: GithubRelease = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("GitHub Client release metadata is malformed."))?;
    let (version, source) = release.artifact()?;
    let verified = crate::cache::ArtifactCache::new(managed.clone())
        .acquire_with(&source, options)
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
        || metadata.version.as_deref() != Some(&version)
    {
        return Err(invalid(
            "The verified JAR does not match the published Aurora Client identity.",
        ));
    }
    normalize(&release, &version, &metadata)
}

fn normalize(
    release: &GithubRelease,
    version: &str,
    metadata: &crate::instance_mods::ModMetadata,
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
        if !compatible {
            continue;
        }
        let (_, artifact) = release.artifact()?;
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
        let manifest = normalize(&release, "3.1.0", &metadata).unwrap();
        let normalized = &manifest.releases()[0];
        assert_eq!(normalized.aurora_version(), "3.1.0");
        assert_eq!(normalized.fabric_loader_version(), "0.19.5");
        assert!(normalized.artifact().url().contains("/v3.1.0/"));
        let mut incompatible = metadata;
        incompatible.depends[0].requirement = "~1.22.0".into();
        assert!(normalize(&release, "3.1.0", &incompatible).is_err());
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
            let body = document.to_string().into_bytes();
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
}
