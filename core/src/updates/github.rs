//! Owner-authorized Aurora Client publication source: public, published GitHub
//! releases. Artifact identity comes from GitHub's SHA-256 release-asset digest;
//! compatibility comes only from the digest-verified JAR. No branch/CI artifacts.
use serde::Deserialize;

use super::{client::ClientUpdateError, parse_release_version};
use crate::{
    cache::ArtifactCache,
    distribution::{AuroraRelease, ReleaseManifest},
    downloads::{self, ArtifactSource, DownloadOptions},
    fabric::versions::satisfies,
    instance_mods,
    paths::ManagedPaths,
};

const RELEASES: &str =
    "https://api.github.com/repos/kalibsolomon-pixel/Aurora-Client/releases?per_page=30";
const DOWNLOAD_ROOT: &str =
    "https://github.com/kalibsolomon-pixel/Aurora-Client/releases/download/";

#[derive(Deserialize)]
struct Publication {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
}

fn artifact(publication: &Publication) -> Option<(&str, &Asset, &str)> {
    if publication.draft || publication.prerelease || publication.published_at.is_none() {
        return None;
    }
    let version = publication.tag_name.strip_prefix('v')?;
    let parsed = parse_release_version(version)?;
    if !parsed.pre.is_empty() || !version.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    let matching: Vec<_> = publication
        .assets
        .iter()
        .filter(|a| crate::distribution::github::artifact_minecraft(&a.name, version).is_some())
        .collect();
    if matching.len() != 1 {
        return None;
    }
    let asset = matching[0];
    let name = &asset.name;
    if asset.size == 0
        || asset.size > 32 * 1024 * 1024
        || asset.browser_download_url != format!("{DOWNLOAD_ROOT}{}/{name}", publication.tag_name)
    {
        return None;
    }
    let digest = asset.digest.as_deref()?.strip_prefix("sha256:")?;
    if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((version, asset, digest))
}

fn normalized_release(
    template: &AuroraRelease,
    version: &str,
    asset: &Asset,
    digest: &str,
    metadata: &instance_mods::ModMetadata,
) -> Result<Option<AuroraRelease>, ClientUpdateError> {
    if crate::distribution::github::artifact_minecraft(&asset.name, version)
        .flatten()
        .is_some_and(|minecraft| minecraft != template.minecraft_version())
    {
        return Ok(None);
    }
    if metadata.id != "aurora"
        || metadata.version.as_deref() != Some(version)
        || metadata.environment.as_deref() == Some("server")
    {
        return Err(ClientUpdateError::ManifestInvalid(
            "Published Aurora artifact identity does not match its release.".into(),
        ));
    }
    for (id, value) in [
        ("minecraft", template.minecraft_version().to_owned()),
        ("fabricloader", template.fabric_loader_version().to_owned()),
        ("java", template.java().major_version().to_string()),
    ] {
        let Some(relation) = metadata.depends.iter().find(|d| d.mod_id == id) else {
            return Err(ClientUpdateError::ManifestInvalid(format!(
                "Published Aurora artifact omits its {id} compatibility requirement."
            )));
        };
        if !satisfies(&value, &relation.requirement).map_err(|_| {
            ClientUpdateError::ManifestInvalid(format!(
                "Unsupported {id} requirement in published artifact."
            ))
        })? {
            return Ok(None);
        }
    }
    for dependency in &metadata.depends {
        if matches!(
            dependency.mod_id.as_str(),
            "minecraft" | "fabricloader" | "java"
        ) {
            continue;
        }
        if !matches!(dependency.mod_id.as_str(), "fabric-api" | "fabric") {
            return Err(ClientUpdateError::Incompatible(format!(
                "Published Aurora requires an unreviewed dependency: {}.",
                dependency.mod_id
            )));
        }
        let Some(api) = template.fabric_api() else {
            return Err(ClientUpdateError::Incompatible(
                "The instance has no reviewed Fabric API dependency metadata.".into(),
            ));
        };
        if !satisfies(api.version(), &dependency.requirement).map_err(|_| {
            ClientUpdateError::ManifestInvalid("Unsupported Fabric API requirement.".into())
        })? {
            return Ok(None);
        }
    }
    if metadata
        .mixin_java_requirements
        .iter()
        .any(|m| m.java_major > template.java().major_version())
    {
        return Ok(None);
    }
    let mut value = serde_json::to_value(template)
        .map_err(|e| ClientUpdateError::ManifestInvalid(e.to_string()))?;
    value["auroraVersion"] = version.into();
    value["channel"] = "stable".into();
    value["artifact"] = serde_json::json!({"url":asset.browser_download_url,"sha256":digest,"sizeBytes":asset.size});
    value["notes"] = format!("Published Aurora Client {version} on GitHub. Compatibility checked against its verified JAR.").into();
    let manifest = ReleaseManifest::from_json(
        &serde_json::json!({"schemaVersion":1,"releases":[value]}).to_string(),
    )?;
    Ok(manifest.releases().first().cloned())
}

pub(super) async fn release_manifest(
    managed: &ManagedPaths,
    options: &DownloadOptions,
    template: &AuroraRelease,
) -> Result<ReleaseManifest, ClientUpdateError> {
    let mut response = downloads::build_client(options)
        .get(RELEASES)
        .header("User-Agent", "Aurora-Launcher")
        .send()
        .await
        .map_err(|_| {
            ClientUpdateError::ManifestUnavailable(
                "The published Aurora releases could not be reached.".into(),
            )
        })?;
    if !response.status().is_success() {
        return Err(ClientUpdateError::ManifestUnavailable(format!(
            "GitHub release discovery returned HTTP {}.",
            response.status().as_u16()
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        ClientUpdateError::ManifestUnavailable("GitHub release metadata could not be read.".into())
    })? {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err(ClientUpdateError::ManifestInvalid(
                "GitHub release metadata exceeds its size limit.".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut publications: Vec<Publication> = serde_json::from_slice(&bytes).map_err(|_| {
        ClientUpdateError::ManifestInvalid("GitHub release metadata is malformed.".into())
    })?;
    if publications.len() > 30 {
        return Err(ClientUpdateError::ManifestInvalid(
            "Too many release records.".into(),
        ));
    }
    publications.sort_by(|a, b| {
        parse_release_version(b.tag_name.trim_start_matches('v'))
            .cmp(&parse_release_version(a.tag_name.trim_start_matches('v')))
    });
    let current = parse_release_version(template.aurora_version()).ok_or_else(|| {
        ClientUpdateError::Incompatible("Installed Aurora version cannot be compared.".into())
    })?;
    for publication in &publications {
        if publication.draft || publication.prerelease || publication.published_at.is_none() {
            continue;
        }
        let Some(version) = publication
            .tag_name
            .strip_prefix('v')
            .and_then(parse_release_version)
        else {
            continue;
        };
        if !version.pre.is_empty() || version <= current {
            continue;
        }
        let (version, asset, digest) = artifact(publication).ok_or_else(|| ClientUpdateError::ManifestInvalid("A newer published Aurora release has no uniquely identified JAR with a SHA-256 digest.".into()))?;
        let source = ArtifactSource::https(&asset.browser_download_url, digest, Some(asset.size))
            .map_err(|e| ClientUpdateError::ManifestInvalid(e.to_string()))?;
        let verified = ArtifactCache::new(managed.clone())
            .acquire_with(&source, options)
            .await
            .map_err(|e| ClientUpdateError::ManifestUnavailable(e.to_string()))?;
        let (metadata, warnings) =
            instance_mods::inspect_fabric_metadata(&verified.path, verified.bytes);
        if warnings.iter().any(|w| {
            w.code.starts_with("fabric_metadata_")
                || w.code.starts_with("nested_")
                || w.code == "mixin_metadata_invalid"
        }) {
            return Err(ClientUpdateError::ManifestInvalid(
                "Published Aurora JAR metadata could not be checked completely.".into(),
            ));
        }
        let metadata = metadata.ok_or_else(|| {
            ClientUpdateError::ManifestInvalid("Published artifact is not a Fabric mod JAR.".into())
        })?;
        if let Some(release) = normalized_release(template, version, asset, digest, &metadata)? {
            return ReleaseManifest::from_json(
                &serde_json::json!({"schemaVersion":1,"releases":[release]}).to_string(),
            )
            .map_err(ClientUpdateError::from);
        }
    }
    ReleaseManifest::from_json(r#"{"schemaVersion":1,"releases":[]}"#)
        .map_err(ClientUpdateError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata() -> instance_mods::ModMetadata {
        instance_mods::ModMetadata {
            id: "aurora".into(),
            name: None,
            version: Some("3.2.0".into()),
            description: None,
            authors: vec![],
            environment: Some("client".into()),
            depends: [
                ("minecraft", "1.21.11"),
                ("fabricloader", ">=0.16.0"),
                ("java", ">=21"),
                ("fabric-api", "*"),
            ]
            .into_iter()
            .map(|(id, requirement)| instance_mods::ModRelation {
                mod_id: id.into(),
                requirement: requirement.into(),
            })
            .collect(),
            recommends: vec![],
            suggests: vec![],
            conflicts: vec![],
            breaks: vec![],
            has_declared_icon: false,
            nested_mod_ids: vec![],
            nested_mod_versions: Default::default(),
            mixin_java_requirements: vec![],
        }
    }
    #[test]
    fn verified_metadata_keeps_exact_environment_and_rejects_incompatible_or_unknown_dependencies()
    {
        let embedded =
            ReleaseManifest::from_json(include_str!("../../production/aurora-releases.json"))
                .unwrap();
        let template = embedded.resolve_exact("3.0.0", None).unwrap();
        let asset = Asset {
            name: "aurora-3.2.0.jar".into(),
            browser_download_url: format!("{DOWNLOAD_ROOT}v3.2.0/aurora-3.2.0.jar"),
            digest: None,
            size: 3211226,
        };
        let digest = "fb7a7c0c10c383fd55da13627614099b35501d831b9372cac196bbb73ffaebb8";
        let mut data = metadata();
        let release = normalized_release(template, "3.2.0", &asset, digest, &data)
            .unwrap()
            .unwrap();
        assert_eq!(release.aurora_version(), "3.2.0");
        assert_eq!(
            release.fabric_loader_version(),
            template.fabric_loader_version()
        );
        assert_eq!(release.fabric_api(), template.fabric_api());
        assert_eq!(release.artifact().sha256(), digest);
        data.depends[0].requirement = "1.20.1".into();
        assert!(
            normalized_release(template, "3.2.0", &asset, digest, &data)
                .unwrap()
                .is_none()
        );
        data = metadata();
        data.depends[1].requirement = ">=0.99".into();
        assert!(
            normalized_release(template, "3.2.0", &asset, digest, &data)
                .unwrap()
                .is_none()
        );
        data = metadata();
        data.depends[2].requirement = ">=25".into();
        assert!(
            normalized_release(template, "3.2.0", &asset, digest, &data)
                .unwrap()
                .is_none()
        );
        data = metadata();
        data.id = "unrelated".into();
        assert!(normalized_release(template, "3.2.0", &asset, digest, &data).is_err());
        data = metadata();
        data.depends.push(instance_mods::ModRelation {
            mod_id: "unknown".into(),
            requirement: "*".into(),
        });
        assert!(normalized_release(template, "3.2.0", &asset, digest, &data).is_err());
    }
    #[test]
    fn only_published_stable_exact_assets_with_sha256_enter_discovery() {
        let mut release: Publication = serde_json::from_value(serde_json::json!({"tag_name":"v3.2.0","draft":false,"prerelease":false,"published_at":"2026-10-31T03:47:00Z","assets":[{"name":"aurora-3.2.0.jar","browser_download_url":format!("{DOWNLOAD_ROOT}v3.2.0/aurora-3.2.0.jar"),"digest":"sha256:fb7a7c0c10c383fd55da13627614099b35501d831b9372cac196bbb73ffaebb8","size":3211226}]})).unwrap();
        assert!(artifact(&release).is_some());
        release.draft = true;
        assert!(artifact(&release).is_none());
        release.draft = false;
        release.prerelease = true;
        assert!(artifact(&release).is_none());
        release.prerelease = false;
        release.assets[0].digest = None;
        assert!(artifact(&release).is_none());
        release.assets[0].digest = Some(format!("sha256:{}", "f".repeat(64)));
        release.assets[0].browser_download_url = "https://example.invalid/aurora-3.2.0.jar".into();
        assert!(artifact(&release).is_none());
    }

    #[test]
    fn version_specific_updates_keep_the_installed_environment() {
        let profiles = crate::distribution::production_manifest().unwrap();
        for (version, minecraft) in [("3.2.2", "26.1.2"), ("3.2.3", "26.2"), ("3.3.0", "26.3")] {
            let template = profiles.resolve_exact(version, None).unwrap();
            let mut publication: Publication = serde_json::from_value(serde_json::json!({"tag_name":format!("v{version}"),"draft":false,"prerelease":false,"published_at":"2026-11-11T00:00:00Z","assets":[{"name":format!("aurora-{version}-mc{minecraft}.jar"),"browser_download_url":template.artifact().url(),"digest":format!("sha256:{}",template.artifact().sha256()),"size":template.artifact().size_bytes()}]})).unwrap();
            let (parsed, asset, digest) = artifact(&publication).unwrap();
            let mut data = metadata();
            data.version = Some(version.into());
            data.depends[0].requirement = format!("~{minecraft}");
            data.depends[2].requirement = ">=25".into();
            let normalized = normalized_release(template, parsed, asset, digest, &data)
                .unwrap()
                .unwrap();
            assert_eq!(normalized.minecraft_version(), minecraft);
            assert_eq!(normalized.fabric_loader_version(), "0.20.1");
            assert_eq!(normalized.fabric_api(), template.fabric_api());
            let legacy = profiles.resolve_exact("3.0.0", None).unwrap();
            assert!(
                normalized_release(legacy, parsed, asset, digest, &data)
                    .unwrap()
                    .is_none()
            );
            data.depends[0].requirement = "*".into();
            let other = profiles
                .releases()
                .iter()
                .find(|profile| profile.minecraft_version() != minecraft)
                .unwrap();
            assert!(
                normalized_release(other, parsed, asset, digest, &data)
                    .unwrap()
                    .is_none()
            );
            publication.assets[0].browser_download_url = publication.assets[0]
                .browser_download_url
                .replace("-mc", "-wrong");
            assert!(artifact(&publication).is_none());
        }
    }
}
