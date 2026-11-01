//! Local `.mrpack` import over the unchanged hosted pack engine.
//!
//! A local archive is accepted only when Modrinth recognizes its exact
//! SHA-512 as the primary archive of a published modpack version. The
//! verified local bytes are then registered in the SHA-512 store, so the
//! existing preview/install/merge engine resolves that exact provider identity
//! and reuses the local bytes instead of downloading them (it still re-verifies
//! them against the published SHA-512). No provider identity is ever invented
//! for an unrecognized archive: it is refused with an explanation.

use std::io::Read as _;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest as _, Sha256, Sha512};

use crate::cache::ArtifactCache;
use crate::integrity::{ArtifactDigest, Sha512Digest};
use crate::modpacks::PackInstallError;
use crate::paths::ManagedPaths;

fn fail(code: &'static str, message: impl Into<String>) -> PackInstallError {
    PackInstallError {
        code,
        message: message.into(),
    }
}

/// The published identity a local archive matched, plus its own index facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPackMatch {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub name: String,
    pub pack_version: String,
    pub minecraft_version: String,
    pub fabric_loader_version: String,
    pub sha512: String,
}

/// Streams the file once, computing both digests and its size.
fn digests(path: &Path) -> Result<(Sha512Digest, ArtifactDigest, u64), PackInstallError> {
    let unreadable = || {
        fail(
            "local_pack_unreadable",
            "The selected file could not be read.",
        )
    };
    let mut file = std::fs::File::open(path).map_err(|_| unreadable())?;
    let mut sha512 = Sha512::new();
    let mut sha256 = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|_| unreadable())?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > crate::mrpack::MAX_PACK_BYTES {
            return Err(fail(
                "pack_archive_limit",
                "The selected pack exceeds Aurora's 512 MiB archive limit.",
            ));
        }
        sha512.update(&buffer[..count]);
        sha256.update(&buffer[..count]);
    }
    let sha512 =
        Sha512Digest::parse(&format!("{:x}", sha512.finalize())).map_err(|_| unreadable())?;
    let sha256 =
        ArtifactDigest::parse(&format!("{:x}", sha256.finalize())).map_err(|_| unreadable())?;
    Ok((sha512, sha256, size))
}

/// Validate, hash, store and identify one explicitly selected local archive.
/// The archive is parsed from its verified store copy (never the live file),
/// so malformed, unsafe or unsupported packs fail before any network lookup.
pub async fn recognize(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    source: &Path,
) -> Result<LocalPackMatch, PackInstallError> {
    let is_mrpack = source
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mrpack"));
    if !is_mrpack {
        return Err(fail(
            "local_pack_invalid",
            "Choose a Modrinth modpack archive (.mrpack).",
        ));
    }
    crate::local_content::safe_source(source)
        .map_err(|error| fail(error.code(), error.to_string()))?;
    let file_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("modpack.mrpack")
        .to_owned();
    let owned = source.to_path_buf();
    let (sha512, sha256, size) = tokio::task::spawn_blocking(move || digests(&owned))
        .await
        .map_err(|_| {
            fail(
                "local_pack_unreadable",
                "The selected file could not be read.",
            )
        })??;
    let cache = ArtifactCache::new(managed.clone());
    let stored = cache
        .import_local(source, &sha256, size)
        .await
        .map_err(|_| {
            fail(
                "local_pack_unreadable",
                "The selected file changed while it was being read. Choose it again.",
            )
        })?;
    let stored_path = stored.path.clone();
    let pack =
        tokio::task::spawn_blocking(move || crate::mrpack::parse_verified_file(&stored_path))
            .await
            .map_err(|_| {
                fail(
                    "local_pack_unreadable",
                    "The archive could not be inspected.",
                )
            })?
            .map_err(|error| fail(error.code(), error.to_string()))?;
    let unrecognized = || {
        fail(
            "local_pack_unrecognized",
            "Modrinth does not publish this exact file as a modpack version. Aurora installs local packs only when Modrinth recognizes the exact archive, so its contents, updates and provenance stay verifiable. Download the pack from Modrinth or install it from Discover.",
        )
    };
    let matches: Vec<_> = provider
        .lookup_files(&[sha512.as_hex()])
        .await
        .map_err(|error| fail(error.code(), error.to_string()))?
        .into_iter()
        .filter(|item| item.queried_sha512 == sha512.as_hex())
        .collect();
    let [found] = matches.as_slice() else {
        return Err(unrecognized());
    };
    let artifact = provider
        .pack_artifact(&found.project_id, &found.version_id)
        .await
        .map_err(|error| match error {
            crate::modrinth::Error::InvalidRequest => unrecognized(),
            other => fail(other.code(), other.to_string()),
        })?;
    if artifact.sha512 != sha512.as_hex()
        || artifact
            .source
            .size_bytes()
            .is_some_and(|published| published != size)
    {
        // The hash matched a secondary file of that version, not its pack archive.
        return Err(unrecognized());
    }
    cache
        .record_sha512_alias(&sha512, &sha256, size)
        .map_err(|error| fail("local_pack_unreadable", error.to_string()))?;
    Ok(LocalPackMatch {
        project_id: artifact.project_id,
        version_id: artifact.version_id,
        title: artifact.title,
        version_number: artifact.version_number,
        file_name,
        size_bytes: size,
        name: pack.name,
        pack_version: pack.pack_version,
        minecraft_version: pack.minecraft_version,
        fabric_loader_version: pack.fabric_loader_version,
        sha512: sha512.as_hex(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestRequest, TestResponse, TestServer};
    use std::io::{Cursor, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn archive(index: &serde_json::Value, extra: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("modrinth.index.json", options).unwrap();
            zip.write_all(index.to_string().as_bytes()).unwrap();
            for (name, bytes) in extra {
                zip.start_file(*name, options).unwrap();
                zip.write_all(bytes).unwrap();
            }
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    fn index(minecraft: &str) -> serde_json::Value {
        serde_json::json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "local-1", "name": "Local Fixture",
            "files": [], "dependencies": {"minecraft": minecraft, "fabric-loader": "0.16.0"}
        })
    }

    struct Fixture {
        root: std::path::PathBuf,
        managed: ManagedPaths,
        server: TestServer,
        requests: Arc<AtomicUsize>,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl Fixture {
        fn client(&self) -> crate::modrinth::Client {
            crate::modrinth::Client::for_testing(&format!("{}/v2/", self.server.base_url()))
        }
        fn write(&self, name: &str, bytes: &[u8]) -> std::path::PathBuf {
            let path = self.root.join("selected").join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bytes).unwrap();
            path
        }
    }

    /// `published` is the archive Modrinth publishes as VERS0001's primary file.
    fn fixture(published: Vec<u8>, recognized: Option<Vec<u8>>) -> Fixture {
        let root = std::env::temp_dir().join(format!("aurora-local-pack-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&requests);
        let recognized_hash = recognized.map(|bytes| format!("{:x}", Sha512::digest(bytes)));
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            counter.fetch_add(1, Ordering::SeqCst);
            let path = request
                .path
                .split('?')
                .next()
                .unwrap_or_default()
                .to_owned();
            let published_hash = format!("{:x}", Sha512::digest(&published));
            // A recognized hash that is not the primary archive is published
            // as the version's secondary file.
            let secondary_hash = recognized_hash
                .clone()
                .filter(|hash| *hash != published_hash)
                .unwrap_or_else(|| format!("{:x}", Sha512::digest(b"secondary")));
            let version = serde_json::json!({
                "id": "VERS0001", "project_id": "PACK0001", "name": "One", "version_number": "1.0.0",
                "version_type": "release", "date_published": "2026-01-01T00:00:00Z", "game_versions": ["1.21.1"],
                "loaders": ["fabric"], "environment": "client_only", "dependencies": [],
                "files": [
                    {"hashes": {"sha512": published_hash, "sha1": "0".repeat(40)}, "url": format!("{}/pack.mrpack", request.base_url),
                     "filename": "pack.mrpack", "primary": true, "size": published.len()},
                    {"hashes": {"sha512": secondary_hash, "sha1": "1".repeat(40)}, "url": format!("{}/extra.zip", request.base_url),
                     "filename": "extra.zip", "primary": false, "size": 9}
                ]
            });
            if let Some(hash) = path.strip_prefix("/v2/version_file/") {
                return if recognized_hash.as_deref() == Some(hash) {
                    TestResponse::ok(version.to_string().as_bytes())
                } else {
                    TestResponse::status(404)
                };
            }
            match path.as_str() {
                "/v2/project/PACK0001" => TestResponse::ok(serde_json::json!({
                    "id": "PACK0001", "project_type": "modpack", "title": "Fixture Pack", "description": "Fixture",
                    "license": {"id":"mit"}, "game_versions": ["1.21.1"], "loaders": ["fabric"], "environment": ["client_only"]
                }).to_string().as_bytes()),
                "/v2/version/VERS0001" => TestResponse::ok(version.to_string().as_bytes()),
                _ => TestResponse::status(404),
            }
        }));
        Fixture {
            root,
            managed,
            server,
            requests,
        }
    }

    #[tokio::test]
    async fn a_published_local_archive_seeds_a_verified_cache_hit_for_the_engine() {
        let bytes = archive(&index("1.21.1"), &[]);
        let fixture = fixture(bytes.clone(), Some(bytes.clone()));
        let path = fixture.write("Local Fixture.MRPACK", &bytes);
        let found = recognize(&fixture.managed, &fixture.client(), &path)
            .await
            .unwrap();
        assert_eq!(
            (found.project_id.as_str(), found.version_id.as_str()),
            ("PACK0001", "VERS0001")
        );
        assert_eq!(found.minecraft_version, "1.21.1");
        assert_eq!(found.name, "Local Fixture");
        // The engine's own acquisition is now a verified hit: no archive download.
        let client = fixture.client();
        let artifact = client.pack_artifact("PACK0001", "VERS0001").await.unwrap();
        let before = fixture.requests.load(Ordering::SeqCst);
        let verified = ArtifactCache::new(fixture.managed.clone())
            .acquire_sha512(&artifact.source)
            .await
            .unwrap();
        assert_eq!(verified.origin, crate::cache::ArtifactOrigin::CacheHit);
        assert_eq!(fixture.requests.load(Ordering::SeqCst), before);
        assert_eq!(std::fs::read(&verified.path).unwrap(), bytes);
        // The selected file itself is never modified.
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[tokio::test]
    async fn an_unpublished_archive_is_refused_without_an_invented_identity() {
        let published = archive(&index("1.21.1"), &[]);
        let local = archive(
            &index("1.21.1"),
            &[("overrides/config/custom.txt", b"private change")],
        );
        let fixture = fixture(published, None);
        let path = fixture.write("private.mrpack", &local);
        let error = recognize(&fixture.managed, &fixture.client(), &path)
            .await
            .unwrap_err();
        assert_eq!(error.code, "local_pack_unrecognized");
        let map = fixture
            .managed
            .cache_dir()
            .join("artifacts")
            .join("sha512-map");
        assert!(!map.exists() || std::fs::read_dir(map).unwrap().next().is_none());
    }

    #[tokio::test]
    async fn a_hash_matching_only_a_secondary_file_is_not_the_pack() {
        // Modrinth recognizes the local bytes, but as some other file of the
        // version: the primary pack archive differs, so nothing is accepted.
        let published = archive(&index("1.21.1"), &[]);
        let local = archive(&index("1.21.1"), &[("overrides/readme.txt", b"different")]);
        let fixture = fixture(published, Some(local.clone()));
        let path = fixture.write("other.mrpack", &local);
        let error = recognize(&fixture.managed, &fixture.client(), &path)
            .await
            .unwrap_err();
        assert_eq!(error.code, "local_pack_unrecognized");
    }

    #[tokio::test]
    async fn malformed_unsafe_and_mislabelled_archives_fail_before_any_lookup() {
        let fixture = fixture(Vec::new(), None);
        let client = fixture.client();
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("not-a-zip.mrpack", b"plain text".to_vec()),
            ("no-index.mrpack", {
                let mut buffer = Cursor::new(Vec::new());
                let mut zip = zip::ZipWriter::new(&mut buffer);
                zip.start_file("readme.txt", zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(b"x").unwrap();
                zip.finish().unwrap();
                buffer.into_inner()
            }),
            ("bad-manifest.mrpack", {
                let mut buffer = Cursor::new(Vec::new());
                let mut zip = zip::ZipWriter::new(&mut buffer);
                zip.start_file(
                    "modrinth.index.json",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
                zip.write_all(b"{not json").unwrap();
                zip.finish().unwrap();
                buffer.into_inner()
            }),
            (
                "traversal.mrpack",
                archive(
                    &serde_json::json!({
                        "formatVersion": 1, "game": "minecraft", "versionId": "x", "name": "Bad",
                        "files": [{"path": "../escape.jar", "hashes": {"sha1": "0".repeat(40), "sha512": "0".repeat(128)},
                            "downloads": ["https://cdn.modrinth.com/data/x/escape.jar"], "fileSize": 1}],
                        "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.0"}
                    }),
                    &[],
                ),
            ),
            (
                "unsupported-loader.mrpack",
                archive(
                    &serde_json::json!({
                        "formatVersion": 1, "game": "minecraft", "versionId": "x", "name": "Forge pack",
                        "files": [], "dependencies": {"minecraft": "1.20.1", "forge": "47.2.0"}
                    }),
                    &[],
                ),
            ),
        ];
        for (name, bytes) in cases {
            let path = fixture.write(name, &bytes);
            let error = recognize(&fixture.managed, &client, &path)
                .await
                .unwrap_err();
            assert_ne!(
                error.code, "local_pack_unrecognized",
                "{name} reached the provider lookup"
            );
        }
        let jar = fixture.write("pack.zip", &archive(&index("1.21.1"), &[]));
        assert_eq!(
            recognize(&fixture.managed, &client, &jar)
                .await
                .unwrap_err()
                .code,
            "local_pack_invalid"
        );
        let relative = std::path::PathBuf::from("relative.mrpack");
        assert!(
            recognize(&fixture.managed, &client, &relative)
                .await
                .is_err()
        );
        assert_eq!(fixture.requests.load(Ordering::SeqCst), 0);
    }
}
