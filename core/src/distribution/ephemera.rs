//! Published Ephemera rendering-engine discovery, independent of Launcher and
//! Aurora Client releases.
//!
//! GitHub release metadata supplies the expected SHA-256 digest; only bytes
//! verified through [`crate::cache::ArtifactCache`] supply Fabric compatibility
//! metadata. Missing releases for a Minecraft version return `Ok(None)` so
//! optional bundling degrades cleanly without blocking instance creation.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::ReleaseArtifact;
use crate::downloads::{ArtifactSource, DownloadOptions};
use crate::paths::ManagedPaths;

pub const RELEASES_URL: &str =
    "https://api.github.com/repos/kalibsolomon-pixel/Ephemera/releases?per_page=30";
const REPOSITORY: &str = "https://github.com/kalibsolomon-pixel/Ephemera";
const MAX_METADATA_BYTES: usize = 1024 * 1024;
const MAX_JAR_BYTES: u64 = 32 * 1024 * 1024;

/// A verified published Ephemera release compatible with one Minecraft environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EphemeraRelease {
    version: String,
    minecraft_version: String,
    java_major_version: u32,
    artifact: ReleaseArtifact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

impl EphemeraRelease {
    pub fn new(
        version: impl Into<String>,
        minecraft_version: impl Into<String>,
        java_major_version: u32,
        artifact: ReleaseArtifact,
        notes: Option<String>,
    ) -> Result<Self, String> {
        let version = version.into();
        let minecraft_version = minecraft_version.into();
        managed_artifact_relative_path(&version)?;
        if minecraft_version.trim().is_empty() || minecraft_version.trim() != minecraft_version {
            return Err("Ephemera release Minecraft version is invalid".into());
        }
        if java_major_version == 0 {
            return Err("Ephemera release Java requirement must be at least 1".into());
        }
        Ok(Self {
            version,
            minecraft_version,
            java_major_version,
            artifact,
            notes,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn minecraft_version(&self) -> &str {
        &self.minecraft_version
    }

    pub fn java_major_version(&self) -> u32 {
        self.java_major_version
    }

    pub fn artifact(&self) -> &ReleaseArtifact {
        &self.artifact
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }
}

/// A failed Ephemera release discovery check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EphemeraDiscoveryError {
    Unavailable(String),
    MetadataInvalid(String),
}

impl EphemeraDiscoveryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unavailable(_) => "ephemera_discovery_unavailable",
            Self::MetadataInvalid(_) => "ephemera_metadata_invalid",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Unavailable(reason) | Self::MetadataInvalid(reason) => reason,
        }
    }
}

impl fmt::Display for EphemeraDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for EphemeraDiscoveryError {}

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

fn invalid(reason: impl Into<String>) -> EphemeraDiscoveryError {
    EphemeraDiscoveryError::MetadataInvalid(reason.into())
}

/// Validates and constructs the deterministic launcher-managed path
/// `mods/ephemera-<version>.jar`.
pub fn managed_artifact_relative_path(version: &str) -> Result<String, String> {
    if version.is_empty()
        || !version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
        || version.starts_with('.')
        || version.contains("..")
    {
        return Err(format!(
            "Ephemera version '{version}' cannot form a safe managed file name"
        ));
    }
    Ok(format!("mods/ephemera-{version}.jar"))
}

/// Recognizes version-specific Ephemera runtime JAR names:
/// `ephemera-<version>-mc<minecraft>.jar`.
pub fn artifact_minecraft<'a>(name: &'a str, version: &str) -> Option<&'a str> {
    let suffix = name.strip_prefix(&format!("ephemera-{version}-mc"))?;
    let minecraft = suffix.strip_suffix(".jar")?;
    if minecraft.split('.').count() < 2
        || minecraft
            .split('.')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    Some(minecraft)
}

impl GithubRelease {
    fn artifacts(
        &self,
        allow_loopback_assets: bool,
    ) -> Result<(String, Vec<(&GithubAsset, &str, ArtifactSource)>), EphemeraDiscoveryError> {
        let version = self
            .tag_name
            .strip_prefix('v')
            .ok_or_else(|| invalid("The Ephemera release tag must name a version."))?;
        let parsed = crate::updates::parse_release_version(version)
            .filter(|v| v.pre.is_empty() && v.build.is_empty())
            .ok_or_else(|| invalid("The Ephemera release version is invalid."))?;
        if parsed.to_string() != version
            || self.draft
            || self.prerelease
            || self.published_at.as_deref().is_none_or(str::is_empty)
            || self.html_url != format!("{REPOSITORY}/releases/tag/{}", self.tag_name)
        {
            return Err(invalid(
                "Only an official published stable Ephemera release is eligible.",
            ));
        }

        let matches: Vec<_> = self
            .assets
            .iter()
            .filter_map(|asset| {
                artifact_minecraft(&asset.name, version).map(|minecraft| (asset, minecraft))
            })
            .collect();
        if matches.is_empty() {
            return Err(invalid("The published Ephemera JAR is missing."));
        }

        let mut environments = std::collections::HashSet::new();
        let mut artifacts = Vec::new();
        for (asset, minecraft) in matches {
            if !environments.insert(minecraft) {
                return Err(invalid(
                    "The published Ephemera JARs have ambiguous Minecraft identities.",
                ));
            }
            if asset.state != "uploaded" || asset.size == 0 || asset.size > MAX_JAR_BYTES {
                return Err(invalid(
                    "The published Ephemera artifact metadata is invalid.",
                ));
            }
            let expected_official_url = format!(
                "{REPOSITORY}/releases/download/{}/{}",
                self.tag_name, asset.name
            );
            let url_allowed = asset.browser_download_url == expected_official_url
                || (allow_loopback_assets
                    && url::Url::parse(&asset.browser_download_url).is_ok_and(|u| {
                        u.scheme() == "http" && crate::downloads::is_loopback_host(&u)
                    }));
            if !url_allowed {
                return Err(invalid(
                    "The published Ephemera artifact URL does not match the official repository.",
                ));
            }
            let digest = asset
                .digest
                .as_deref()
                .and_then(|d| d.strip_prefix("sha256:"))
                .ok_or_else(|| {
                    invalid("GitHub did not publish an expected SHA-256 for this Ephemera JAR.")
                })?;
            let source = if allow_loopback_assets {
                ArtifactSource::https_or_loopback(
                    &asset.browser_download_url,
                    digest,
                    Some(asset.size),
                )
            } else {
                ArtifactSource::https(&asset.browser_download_url, digest, Some(asset.size))
            }
            .map_err(|error| invalid(error.to_string()))?;
            artifacts.push((asset, minecraft, source));
        }
        Ok((version.to_owned(), artifacts))
    }
}

/// Discovers the newest published, SHA-256-verified Ephemera release compatible
/// with one Minecraft + Fabric Loader + Java environment.
///
/// Returns `Ok(None)` when no published Ephemera release supports `minecraft_version`.
pub async fn discover_for_minecraft(
    managed: &ManagedPaths,
    options: &DownloadOptions,
    minecraft_version: &str,
    fabric_loader_version: &str,
    java_major_version: u32,
) -> Result<Option<EphemeraRelease>, EphemeraDiscoveryError> {
    discover_for_minecraft_at(
        managed,
        options,
        RELEASES_URL,
        minecraft_version,
        fabric_loader_version,
        java_major_version,
    )
    .await
}

pub async fn discover_for_minecraft_at(
    managed: &ManagedPaths,
    options: &DownloadOptions,
    endpoint: &str,
    minecraft_version: &str,
    fabric_loader_version: &str,
    java_major_version: u32,
) -> Result<Option<EphemeraRelease>, EphemeraDiscoveryError> {
    let url =
        url::Url::parse(endpoint).map_err(|_| invalid("Invalid Ephemera discovery endpoint."))?;
    let is_loopback = url.scheme() == "http" && crate::downloads::is_loopback_host(&url);
    if endpoint != RELEASES_URL && !is_loopback {
        return Err(invalid(
            "Ephemera discovery must use the pinned official GitHub endpoint.",
        ));
    }

    let mut response = crate::downloads::build_metadata_client(options)
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| {
            EphemeraDiscoveryError::Unavailable("Could not reach GitHub Ephemera releases.".into())
        })?;
    if !response.status().is_success() {
        return Err(EphemeraDiscoveryError::Unavailable(format!(
            "GitHub Ephemera releases answered HTTP {}.",
            response.status().as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_METADATA_BYTES as u64)
    {
        return Err(invalid(
            "GitHub Ephemera release metadata exceeds the size limit.",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        EphemeraDiscoveryError::Unavailable("Could not read GitHub Ephemera releases.".into())
    })? {
        if bytes.len() + chunk.len() > MAX_METADATA_BYTES {
            return Err(invalid(
                "GitHub Ephemera release metadata exceeds the size limit.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }

    let mut releases: Vec<GithubRelease> = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("GitHub Ephemera release metadata is malformed."))?;
    if releases.len() > 30 {
        return Err(invalid("Too many Ephemera release records."));
    }
    releases.sort_by_key(|release| {
        std::cmp::Reverse(crate::updates::parse_release_version(
            release.tag_name.trim_start_matches('v'),
        ))
    });

    for release in releases {
        if release.draft || release.prerelease || release.published_at.is_none() {
            continue;
        }
        if release.assets.is_empty() {
            continue;
        }
        let (version, artifacts) = release.artifacts(is_loopback)?;
        for (_asset, asset_mc, source) in artifacts {
            if asset_mc != minecraft_version {
                continue;
            }
            if let Some(verified) = verify_ephemera_artifact(
                managed,
                options,
                &release,
                &version,
                &source,
                minecraft_version,
                fabric_loader_version,
                java_major_version,
            )
            .await?
            {
                return Ok(Some(verified));
            }
        }
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
async fn verify_ephemera_artifact(
    managed: &ManagedPaths,
    options: &DownloadOptions,
    release: &GithubRelease,
    version: &str,
    source: &ArtifactSource,
    minecraft_version: &str,
    fabric_loader_version: &str,
    java_major_version: u32,
) -> Result<Option<EphemeraRelease>, EphemeraDiscoveryError> {
    let verified = crate::cache::ArtifactCache::new(managed.clone())
        .acquire_with(source, options)
        .await
        .map_err(|error| {
            invalid(format!(
                "The published Ephemera JAR could not be verified: {error}"
            ))
        })?;
    let (metadata, warnings) =
        crate::instance_mods::inspect_fabric_metadata(&verified.path, verified.bytes);
    let metadata = metadata
        .ok_or_else(|| invalid("The verified Ephemera JAR has no valid Fabric metadata."))?;
    if !warnings.is_empty()
        || metadata.id != "ephemera"
        || metadata.version.as_deref() != Some(version)
    {
        return Err(invalid(
            "The verified JAR does not match the published Ephemera identity.",
        ));
    }
    if metadata.environment.as_deref() == Some("server") {
        return Ok(None);
    }
    if ["minecraft", "fabricloader", "java"]
        .iter()
        .any(|required| !metadata.depends.iter().any(|dep| dep.mod_id == *required))
    {
        return Err(invalid(
            "The verified Ephemera JAR must declare Minecraft, Fabric Loader and Java requirements.",
        ));
    }
    let compatible = metadata.depends.iter().all(|dependency| {
        let value = match dependency.mod_id.as_str() {
            "minecraft" => Some(minecraft_version.to_owned()),
            "fabricloader" => Some(fabric_loader_version.to_owned()),
            "java" => Some(java_major_version.to_string()),
            id => metadata.nested_mod_versions.get(id).cloned().flatten(),
        };
        value.is_some_and(|v| {
            crate::fabric::versions::satisfies(&v, &dependency.requirement).unwrap_or(false)
        })
    });
    if !compatible
        || metadata
            .mixin_java_requirements
            .iter()
            .any(|mixin| mixin.java_major > java_major_version)
    {
        return Ok(None);
    }
    let notes: String = release
        .body
        .as_deref()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t'))
        .take(2000)
        .collect();
    let artifact = ReleaseArtifact::new(
        source.url().as_str().to_owned(),
        source.sha256().as_hex(),
        source.size_bytes(),
    )
    .map_err(invalid)?;
    let entry = EphemeraRelease::new(
        version,
        minecraft_version,
        java_major_version,
        artifact,
        (!notes.is_empty()).then_some(notes),
    )
    .map_err(invalid)?;
    Ok(Some(entry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest as _;
    use std::io::Write;

    fn sample_release_json() -> serde_json::Value {
        serde_json::json!({
            "tag_name": "v0.2.0",
            "html_url": format!("{REPOSITORY}/releases/tag/v0.2.0"),
            "draft": false,
            "prerelease": false,
            "published_at": "2026-11-21T12:00:00Z",
            "body": "Ephemera 0.2.0 for Minecraft 26.2",
            "assets": [{
                "name": "ephemera-0.2.0-mc26.2.jar",
                "browser_download_url": format!("{REPOSITORY}/releases/download/v0.2.0/ephemera-0.2.0-mc26.2.jar"),
                "size": 1024,
                "digest": format!("sha256:{}", "b".repeat(64)),
                "state": "uploaded"
            }]
        })
    }

    #[test]
    fn version_specific_names_are_exact_and_reject_unsafe_names() {
        assert_eq!(
            artifact_minecraft("ephemera-0.2.0-mc26.2.jar", "0.2.0"),
            Some("26.2")
        );
        assert_eq!(
            artifact_minecraft("ephemera-1.0.0-mc26.3.jar", "1.0.0"),
            Some("26.3")
        );
        for bad in [
            "ephemera-0.2.0.jar",
            "ephemera-0.2.0-mc.jar",
            "ephemera-0.2.0-mc26..2.jar",
            "ephemera-0.2.0-mc26.2-sources.jar",
            "ephemera-0.2.0-mc../26.2.jar",
            "ephemera-0.2.0-mc26.2.jar.exe",
            "aurora-0.2.0-mc26.2.jar",
        ] {
            assert!(artifact_minecraft(bad, "0.2.0").is_none(), "{bad}");
        }
        assert_eq!(
            managed_artifact_relative_path("0.2.0").unwrap(),
            "mods/ephemera-0.2.0.jar"
        );
        for unsafe_ver in ["", "../escape", "sub/dir", ".hidden", "0.2..0"] {
            assert!(
                managed_artifact_relative_path(unsafe_ver).is_err(),
                "{unsafe_ver}"
            );
        }
    }

    #[test]
    fn unpublished_foreign_missing_digest_or_ambiguous_artifacts_fail() {
        for mutate in 0..9 {
            let mut value = sample_release_json();
            match mutate {
                0 => value["draft"] = true.into(),
                1 => value["prerelease"] = true.into(),
                2 => value["published_at"] = serde_json::Value::Null,
                3 => value["assets"][0]["digest"] = serde_json::Value::Null,
                4 => {
                    value["assets"][0]["browser_download_url"] =
                        "https://example.com/ephemera.jar".into()
                }
                5 => value["assets"][0]["size"] = 0.into(),
                6 => value["assets"][0]["digest"] = "sha256:not-hex".into(),
                7 => value["assets"]
                    .as_array_mut()
                    .unwrap()
                    .push(sample_release_json()["assets"][0].clone()),
                _ => value["tag_name"] = "v0.2.0-rc.1".into(),
            }
            assert!(
                serde_json::from_value::<GithubRelease>(value)
                    .unwrap()
                    .artifacts(false)
                    .is_err(),
                "case {mutate}"
            );
        }
    }

    #[tokio::test]
    async fn untrusted_endpoints_fail_closed_without_mutating_state() {
        let managed = ManagedPaths::from_app_local_data_dir(
            std::env::temp_dir().join(format!("ephemera-untrusted-{}", uuid::Uuid::new_v4())),
        )
        .unwrap();
        assert!(
            discover_for_minecraft_at(
                &managed,
                &DownloadOptions::default(),
                "https://example.com/releases",
                "26.2",
                "0.20.1",
                25
            )
            .await
            .is_err()
        );
        assert!(!managed.instance_registry_file().exists());
    }

    #[tokio::test]
    async fn loopback_discovery_verifies_jar_identity_and_filters_by_minecraft() {
        let managed = ManagedPaths::from_app_local_data_dir(
            std::env::temp_dir().join(format!("ephemera-loopback-{}", uuid::Uuid::new_v4())),
        )
        .unwrap();
        let make_jar = |id: &str, version: &str, mc: &str| {
            let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            archive
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            archive
                .write_all(
                    serde_json::json!({
                        "schemaVersion": 1,
                        "id": id,
                        "version": version,
                        "environment": "client",
                        "depends": {
                            "minecraft": format!("~{mc}"),
                            "fabricloader": ">=0.18.4",
                            "java": ">=25"
                        }
                    })
                    .to_string()
                    .as_bytes(),
                )
                .unwrap();
            let bytes = archive.finish().unwrap().into_inner();
            let digest = format!("{:x}", sha2::Sha256::digest(&bytes));
            let path = crate::cache::ArtifactCache::new(managed.clone())
                .verified_path(&crate::integrity::ArtifactDigest::parse(&digest).unwrap());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &bytes).unwrap();
            (bytes.len(), digest)
        };

        let (size_010, digest_010) = make_jar("ephemera", "0.1.0", "26.2");
        let (size_020, digest_020) = make_jar("ephemera", "0.2.0", "26.2");
        let docs = serde_json::json!([
            {
                "tag_name": "v0.1.0",
                "html_url": format!("{REPOSITORY}/releases/tag/v0.1.0"),
                "draft": false,
                "prerelease": false,
                "published_at": "2026-11-20T00:00:00Z",
                "assets": [{
                    "name": "ephemera-0.1.0-mc26.2.jar",
                    "browser_download_url": format!("{REPOSITORY}/releases/download/v0.1.0/ephemera-0.1.0-mc26.2.jar"),
                    "size": size_010,
                    "digest": format!("sha256:{digest_010}"),
                    "state": "uploaded"
                }]
            },
            {
                "tag_name": "v0.2.0",
                "html_url": format!("{REPOSITORY}/releases/tag/v0.2.0"),
                "draft": false,
                "prerelease": false,
                "published_at": "2026-11-21T00:00:00Z",
                "body": "Vulkan mesh shaders & Aurora ObjectShare bridge",
                "assets": [{
                    "name": "ephemera-0.2.0-mc26.2.jar",
                    "browser_download_url": format!("{REPOSITORY}/releases/download/v0.2.0/ephemera-0.2.0-mc26.2.jar"),
                    "size": size_020,
                    "digest": format!("sha256:{digest_020}"),
                    "state": "uploaded"
                }]
            }
        ]);
        let body = serde_json::to_vec(&docs).unwrap();
        let server = crate::test_support::TestServer::spawn(std::sync::Arc::new(move |_| {
            crate::test_support::TestResponse::ok(&body)
        }));

        let found = discover_for_minecraft_at(
            &managed,
            &DownloadOptions::default(),
            &format!("{}/releases", server.base_url()),
            "26.2",
            "0.20.1",
            25,
        )
        .await
        .unwrap()
        .expect("26.2 release must be found");
        assert_eq!(found.version(), "0.2.0");
        assert_eq!(found.artifact().sha256(), digest_020);
        assert_eq!(
            found.notes(),
            Some("Vulkan mesh shaders & Aurora ObjectShare bridge")
        );

        let none = discover_for_minecraft_at(
            &managed,
            &DownloadOptions::default(),
            &format!("{}/releases", server.base_url()),
            "1.21.11",
            "0.19.5",
            21,
        )
        .await
        .unwrap();
        assert!(none.is_none());

        // Foreign mod ID inside a signed asset must be rejected.
        let (bad_size, bad_digest) = make_jar("not-ephemera", "0.3.0", "26.2");
        let bad_body = serde_json::to_vec(&serde_json::json!([{
            "tag_name": "v0.3.0",
            "html_url": format!("{REPOSITORY}/releases/tag/v0.3.0"),
            "draft": false,
            "prerelease": false,
            "published_at": "2026-11-22T00:00:00Z",
            "assets": [{
                "name": "ephemera-0.3.0-mc26.2.jar",
                "browser_download_url": format!("{REPOSITORY}/releases/download/v0.3.0/ephemera-0.3.0-mc26.2.jar"),
                "size": bad_size,
                "digest": format!("sha256:{bad_digest}"),
                "state": "uploaded"
            }]
        }]))
        .unwrap();
        let bad_server = crate::test_support::TestServer::spawn(std::sync::Arc::new(move |_| {
            crate::test_support::TestResponse::ok(&bad_body)
        }));
        assert!(
            discover_for_minecraft_at(
                &managed,
                &DownloadOptions::default(),
                &format!("{}/releases", bad_server.base_url()),
                "26.2",
                "0.20.1",
                25,
            )
            .await
            .is_err()
        );
    }
}
