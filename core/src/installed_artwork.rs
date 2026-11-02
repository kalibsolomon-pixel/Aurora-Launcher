//! Cosmetic exact-file identity, independent of adoption and management rights.
use crate::{
    instance_content::{self, ContentType},
    instance_mods,
    instances::InstanceId,
    paths::ManagedPaths,
};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use std::{collections::HashMap, io::Read, path::PathBuf};

const MAX_FILE: u64 = 512 * 1024 * 1024;
fn failure_memo() -> &'static std::sync::Mutex<HashMap<String, std::time::Instant>> {
    static FAILURES: std::sync::OnceLock<std::sync::Mutex<HashMap<String, std::time::Instant>>> =
        std::sync::OnceLock::new();
    FAILURES.get_or_init(Default::default)
}
fn remember_failure(key: String) {
    let mut failures = failure_memo().lock().unwrap();
    failures.retain(|_, deadline| *deadline > std::time::Instant::now());
    if failures.len() >= 128 {
        failures.clear();
    }
    failures.insert(
        key,
        std::time::Instant::now() + std::time::Duration::from_secs(60),
    );
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Identities {
    pub projects: HashMap<String, String>,
    pub retry_after_ms: Option<u64>,
}
struct Candidate {
    entry: String,
    name: String,
    expected: Option<String>,
    provider: Option<String>,
    first_party: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Identity {
    schema_version: u32,
    sha512: String,
    content_type: ContentType,
    project: Option<String>,
    checked_at: u64,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn cache_path(paths: &ManagedPaths, kind: ContentType, hash: &str) -> PathBuf {
    paths
        .cache_dir()
        .join("artwork")
        .join("identities")
        .join(kind.directory_name())
        .join(format!("{hash}.json"))
}
fn read_identity(paths: &ManagedPaths, kind: ContentType, hash: &str) -> Option<Identity> {
    let path = cache_path(paths, kind, hash);
    if !std::fs::symlink_metadata(&path).ok()?.is_file()
        || !path
            .canonicalize()
            .ok()?
            .starts_with(paths.data_root().canonicalize().ok()?)
    {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(4097)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 4096 {
        return None;
    }
    let identity: Identity = serde_json::from_slice(&bytes).ok()?;
    if identity.schema_version != 1
        || identity.sha512 != hash
        || identity.content_type != kind
        || identity
            .project
            .as_ref()
            .is_some_and(|id| id.len() != 8 || !id.bytes().all(|b| b.is_ascii_alphanumeric()))
        || identity.checked_at > now()
        || (identity.project.is_none() && now().saturating_sub(identity.checked_at) >= 3600)
    {
        return None;
    }
    Some(identity)
}
fn persist_identity(paths: &ManagedPaths, identity: &Identity) {
    let root = match paths.data_root().canonicalize() {
        Ok(root) => root,
        Err(_) => return,
    };
    let path = cache_path(paths, identity.content_type, &identity.sha512);
    for dir in [
        paths.cache_dir(),
        paths.cache_dir().join("artwork"),
        paths.cache_dir().join("artwork/identities"),
        path.parent().unwrap().to_owned(),
    ] {
        if !dir.exists() && std::fs::create_dir(&dir).is_err() {
            return;
        }
        if !dir.canonicalize().is_ok_and(|dir| dir.starts_with(&root)) {
            return;
        }
    }
    if let Ok(bytes) = serde_json::to_vec(identity) {
        let _ = crate::artwork::write_atomic(&path, &bytes);
    }
}
fn candidates(
    paths: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
) -> Option<Vec<Candidate>> {
    if kind == ContentType::Mod {
        let state = crate::aurora::load_installed_state(paths, instance)
            .ok()
            .flatten()
            .filter(|state| {
                crate::distribution::production_manifest()
                    .ok()
                    .and_then(|embedded| {
                        crate::aurora::merged_release_manifest(paths, instance, &embedded).ok()
                    })
                    .and_then(|manifest| {
                        manifest
                            .resolve_exact(state.aurora_version(), Some(state.channel()))
                            .cloned()
                    })
                    .is_some_and(|release| {
                        release
                            .artifact()
                            .sha256()
                            .eq_ignore_ascii_case(state.artifact().sha256())
                            && release
                                .artifact()
                                .size_bytes()
                                .is_none_or(|size| size == state.artifact().size_bytes())
                    })
            });
        Some(
            instance_mods::scan(paths, instance)
                .ok()?
                .entries
                .into_iter()
                .filter(|entry| {
                    matches!(
                        entry.file_type,
                        instance_mods::ModFileType::EnabledJar
                            | instance_mods::ModFileType::DisabledJar
                    ) && entry.ownership != instance_mods::ModOwnership::Unknown
                        && entry.size_bytes.is_some_and(|size| size <= MAX_FILE)
                })
                .map(|entry| {
                    let first_party = matches!(
                        entry.ownership,
                        instance_mods::ModOwnership::LauncherBootstrap
                            | instance_mods::ModOwnership::LauncherManagedRequired
                            | instance_mods::ModOwnership::LauncherManagedRetained
                    ) && state.as_ref().is_some_and(|state| {
                        entry.sha256.as_deref() == Some(state.artifact().sha256())
                    });
                    Candidate {
                        entry: entry.entry_id,
                        name: entry.file_name,
                        expected: entry.sha256,
                        provider: entry
                            .provenance
                            .filter(|p| p.provider == "modrinth")
                            .map(|p| p.project_id),
                        first_party,
                    }
                })
                .collect(),
        )
    } else {
        Some(
            instance_content::scan(paths, instance, kind)
                .ok()?
                .entries
                .into_iter()
                .filter(|entry| {
                    entry.file_type == "zip"
                        && entry.ownership != instance_content::ContentOwnership::Unknown
                        && entry.size_bytes.is_some_and(|size| size <= MAX_FILE)
                })
                .map(|entry| Candidate {
                    entry: entry.entry_id,
                    name: entry.file_name,
                    expected: entry.sha256,
                    provider: entry
                        .provenance
                        .filter(|p| p.provider == "modrinth")
                        .map(|p| p.project_id),
                    first_party: false,
                })
                .collect(),
        )
    }
}
fn local(
    paths: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
) -> Option<(Identities, Vec<(String, String)>)> {
    let mut result = Identities {
        projects: HashMap::new(),
        retry_after_ms: None,
    };
    let candidates = candidates(paths, instance, kind)?;
    if candidates.is_empty() {
        return Some((result, Vec::new()));
    }
    let directory = instance_content::validate_directory(paths, instance, kind).ok()?;
    let canonical_directory = directory.canonicalize().ok()?;
    let mut pending = Vec::new();
    for candidate in candidates.into_iter().take(256) {
        if candidate.first_party {
            result.projects.insert(candidate.entry, "aurora".into());
            continue;
        }
        if let Some(project) = candidate.provider {
            result.projects.insert(candidate.entry, project);
            continue;
        }
        instance_content::validate_file_name(&candidate.name).ok()?;
        let path = directory.join(candidate.name);
        let meta = std::fs::symlink_metadata(&path).ok()?;
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.len() > MAX_FILE
            || !path.canonicalize().ok()?.starts_with(&canonical_directory)
        {
            continue;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                continue;
            }
        }
        let mut sha256 = sha2::Sha256::new();
        let mut sha512 = sha2::Sha512::new();
        let mut file = std::fs::File::open(path).ok()?.take(MAX_FILE + 1);
        let mut buffer = [0; 65536];
        let mut total = 0;
        loop {
            let n = file.read(&mut buffer).ok()?;
            if n == 0 {
                break;
            }
            total += n as u64;
            sha256.update(&buffer[..n]);
            sha512.update(&buffer[..n]);
        }
        if total > MAX_FILE
            || candidate
                .expected
                .is_some_and(|expected| expected != format!("{:x}", sha256.finalize()))
        {
            continue;
        }
        let hash = format!("{:x}", sha512.finalize());
        if let Some(identity) = read_identity(paths, kind, &hash) {
            if let Some(project) = identity.project {
                result.projects.insert(candidate.entry, project);
            }
        } else {
            pending.push((candidate.entry, hash));
        }
    }
    Some((result, pending))
}
pub async fn resolve(
    paths: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
    client: &crate::modrinth::Client,
) -> Identities {
    let lock = crate::artwork::project_lock(&format!(
        "identity:{}:{instance}:{kind:?}",
        paths.data_root().display()
    ));
    let _guard = lock.lock().await;
    let managed = paths.clone();
    let id = instance.clone();
    let Some((mut result, pending)) =
        tokio::task::spawn_blocking(move || local(&managed, &id, kind))
            .await
            .ok()
            .flatten()
    else {
        return Identities {
            projects: HashMap::new(),
            retry_after_ms: Some(60_000),
        };
    };
    if pending.is_empty() {
        return result;
    }
    let key = format!("{}:{instance}:{kind:?}", paths.data_root().display());
    let failures = failure_memo();
    if failures
        .lock()
        .unwrap()
        .get(&key)
        .is_some_and(|deadline| *deadline > std::time::Instant::now())
    {
        result.retry_after_ms = Some(60_000);
        return result;
    }
    let hashes: Vec<_> = pending.iter().map(|(_, hash)| hash.clone()).collect();
    let Ok(found) = client.lookup_files(&hashes).await else {
        remember_failure(key);
        result.retry_after_ms = Some(60_000);
        return result;
    };
    let mut types = HashMap::new();
    for file in &found {
        if !types.contains_key(&file.project_id) {
            match client.project_type_and_title(&file.project_id).await {
                Ok((kind, _)) => {
                    types.insert(file.project_id.clone(), kind);
                }
                Err(_) => {
                    remember_failure(key);
                    result.retry_after_ms = Some(60_000);
                    return result;
                }
            }
        }
    }
    let expected = match kind {
        ContentType::Mod => "mod",
        ContentType::ResourcePack => "resourcepack",
        ContentType::ShaderPack => "shader",
    };
    for (entry, hash) in pending {
        let project = found
            .iter()
            .find(|file| {
                file.queried_sha512 == hash
                    && types
                        .get(&file.project_id)
                        .is_some_and(|kind| kind == expected)
            })
            .map(|file| file.project_id.clone());
        persist_identity(
            paths,
            &Identity {
                schema_version: 1,
                sha512: hash,
                content_type: kind,
                project: project.clone(),
                checked_at: now(),
            },
        );
        if let Some(project) = project {
            result.projects.insert(entry, project);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        instance_content::{
            ContentCompatibility, ContentState, ProviderOrigin, ProviderRecord, UpdateChannel,
        },
        test_support::{TestResponse, TestServer},
    };
    use serde_json::json;
    use std::{
        io::Write,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    struct Lab {
        paths: ManagedPaths,
        instance: InstanceId,
    }
    impl Lab {
        fn new() -> Self {
            let paths = ManagedPaths::from_app_local_data_dir(
                std::env::temp_dir()
                    .join(format!("aurora-installed-artwork-{}", uuid::Uuid::new_v4())),
            )
            .unwrap();
            std::fs::create_dir_all(paths.data_root()).unwrap();
            std::fs::create_dir_all(
                paths
                    .instance_paths(&InstanceId::new("artwork-test").unwrap())
                    .root(),
            )
            .unwrap();
            Self {
                paths,
                instance: InstanceId::new("artwork-test").unwrap(),
            }
        }
        fn write(&self, kind: ContentType, name: &str, bytes: &[u8]) {
            let dir =
                instance_content::ensure_directory(&self.paths, &self.instance, kind).unwrap();
            std::fs::write(dir.join(name), bytes).unwrap();
        }
    }
    impl Drop for Lab {
        fn drop(&mut self) {
            std::fs::remove_dir_all(self.paths.data_root()).unwrap();
        }
    }
    fn archive(kind: ContentType, id: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let (name, bytes) = match kind {
            ContentType::Mod => (
                "fabric.mod.json",
                json!({"schemaVersion":1,"id":id,"version":"1.0.0"}).to_string(),
            ),
            ContentType::ResourcePack => (
                "pack.mcmeta",
                json!({"pack":{"pack_format":42,"description":id}}).to_string(),
            ),
            ContentType::ShaderPack => ("shaders/basic.fsh", id.into()),
        };
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes.as_bytes()).unwrap();
        writer.finish().unwrap().into_inner()
    }
    fn server(hash: String, kind: &str, fail: Arc<AtomicBool>) -> TestServer {
        let kind = kind.to_owned();
        TestServer::spawn(Arc::new(move |request| {
            if fail.load(Ordering::SeqCst) {
                return TestResponse::status(503);
            }
            let path = request.path.split('?').next().unwrap();
            if path == "/v2/project/AAAABBBB" {
                return TestResponse::ok(&serde_json::to_vec(&json!({"id":"AAAABBBB","project_type":kind,"title":"Exact project","description":"","license":{"id":"MIT"},"game_versions":["1.21.11"],"loaders":["fabric"],"environment":["client_and_server"]})).unwrap());
            }
            let version = json!({"id":"11112222","project_id":"AAAABBBB","name":"Exact release","version_number":"1.0.0","version_type":"release","date_published":"2026-06-01T00:00:00Z","game_versions":["1.21.11"],"loaders":["fabric"],"environment":"client_and_server","files":[{"hashes":{"sha512":hash},"url":"https://cdn.modrinth.com/data/AAAABBBB/1.jar","filename":"provider-original.jar","primary":true,"size":64,"file_type":null}],"dependencies":[]});
            if path == format!("/v2/version_file/{hash}") {
                return TestResponse::ok(&serde_json::to_vec(&version).unwrap());
            }
            if path == "/v2/version_files" {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                return TestResponse::ok(
                    &serde_json::to_vec(&if body["hashes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|h| h.as_str() == Some(&hash))
                    {
                        json!({hash.clone():version})
                    } else {
                        json!({})
                    })
                    .unwrap(),
                );
            }
            TestResponse::status(404)
        }))
    }
    #[tokio::test]
    async fn exact_local_identity_is_persisted_offline_without_adoption_for_all_types() {
        crate::downloads::ensure_rustls_crypto_provider();
        for (kind, provider_kind, name) in [
            (ContentType::Mod, "mod", "renamed.jar"),
            (ContentType::ResourcePack, "resourcepack", "renamed.zip"),
            (ContentType::ShaderPack, "shader", "renamed.zip"),
        ] {
            let lab = Lab::new();
            let bytes = archive(kind, "matched");
            lab.write(kind, name, &bytes);
            let unknown = archive(kind, "only_local");
            let local_name = if kind == ContentType::Mod {
                "provider-original.jar"
            } else {
                "provider-original.zip"
            };
            lab.write(kind, local_name, &unknown);
            let hash = format!("{:x}", sha2::Sha512::digest(&bytes));
            let fail = Arc::new(AtomicBool::new(false));
            let server = server(hash, provider_kind, fail.clone());
            let client =
                crate::modrinth::Client::for_testing(&format!("{}/v2/", server.base_url()));
            let identities = resolve(&lab.paths, &lab.instance, kind, &client).await;
            assert_eq!(identities.projects.len(), 1);
            assert_eq!(identities.projects.values().next().unwrap(), "AAAABBBB");
            let requests = server.request_count();
            fail.store(true, Ordering::SeqCst);
            let restarted =
                ManagedPaths::from_app_local_data_dir(lab.paths.data_root().to_owned()).unwrap();
            assert_eq!(
                resolve(&restarted, &lab.instance, kind, &client)
                    .await
                    .projects,
                identities.projects
            );
            assert_eq!(server.request_count(), requests);
            let dir =
                instance_content::validate_directory(&lab.paths, &lab.instance, kind).unwrap();
            assert_eq!(std::fs::read(dir.join(name)).unwrap(), bytes);
            assert_eq!(std::fs::read(dir.join(local_name)).unwrap(), unknown);
            assert!(
                ContentState::load(&lab.paths, &lab.instance)
                    .unwrap()
                    .entries
                    .is_empty()
            );
            lab.write(kind, name, &unknown);
            assert!(
                resolve(&lab.paths, &lab.instance, kind, &client)
                    .await
                    .projects
                    .is_empty()
            );
        }
    }
    #[tokio::test]
    async fn provider_record_wins_and_damaged_provider_files_never_receive_artwork() {
        let lab = Lab::new();
        let bytes = archive(ContentType::Mod, "managed");
        lab.write(ContentType::Mod, "managed.jar", &bytes);
        let mut state = ContentState::empty();
        state.entries.push(ProviderRecord {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: "CCCCDDDD".into(),
            version_id: "11112222".into(),
            file_id: "f".repeat(128),
            file_name: "managed.jar".into(),
            sha256: format!("{:x}", sha2::Sha256::digest(&bytes)),
            display_version: Some("1.0.0".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: Some("fabric".into()),
                environment: Some("client".into()),
            },
            dependencies: vec![],
            explicitly_retained: true,
            requires: vec![],
            origin: ProviderOrigin::Direct,
            installed_at_unix_seconds: None,
            pinned: false,
            update_channel: UpdateChannel::Stable,
        });
        state.save(&lab.paths, &lab.instance).unwrap();
        let server = server(
            format!("{:x}", sha2::Sha512::digest(&bytes)),
            "mod",
            Arc::new(AtomicBool::new(true)),
        );
        let client = crate::modrinth::Client::for_testing(&format!("{}/v2/", server.base_url()));
        assert_eq!(
            resolve(&lab.paths, &lab.instance, ContentType::Mod, &client)
                .await
                .projects
                .values()
                .next()
                .unwrap(),
            "CCCCDDDD"
        );
        assert_eq!(server.request_count(), 0);
        lab.write(
            ContentType::Mod,
            "managed.jar",
            &archive(ContentType::Mod, "tampered"),
        );
        assert!(
            resolve(&lab.paths, &lab.instance, ContentType::Mod, &client)
                .await
                .projects
                .is_empty()
        );
        assert_eq!(server.request_count(), 0);
    }
    #[tokio::test]
    async fn transient_failure_retries_and_corrupt_identity_is_reconstructed() {
        let lab = Lab::new();
        let bytes = archive(ContentType::Mod, "retry");
        lab.write(ContentType::Mod, "retry.jar", &bytes);
        let hash = format!("{:x}", sha2::Sha512::digest(&bytes));
        let fail = Arc::new(AtomicBool::new(true));
        let server = server(hash.clone(), "mod", fail.clone());
        let client = crate::modrinth::Client::for_testing(&format!("{}/v2/", server.base_url()));
        assert_eq!(
            resolve(&lab.paths, &lab.instance, ContentType::Mod, &client)
                .await
                .retry_after_ms,
            Some(60_000)
        );
        assert!(!cache_path(&lab.paths, ContentType::Mod, &hash).exists());
        let count = server.request_count();
        resolve(&lab.paths, &lab.instance, ContentType::Mod, &client).await;
        assert_eq!(server.request_count(), count);
        let key = format!("{}:{}:Mod", lab.paths.data_root().display(), lab.instance);
        failure_memo()
            .lock()
            .unwrap()
            .insert(key, std::time::Instant::now());
        fail.store(false, Ordering::SeqCst);
        assert_eq!(
            resolve(&lab.paths, &lab.instance, ContentType::Mod, &client)
                .await
                .projects
                .len(),
            1
        );
        let path = cache_path(&lab.paths, ContentType::Mod, &hash);
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(read_identity(&lab.paths, ContentType::Mod, &hash).is_none());
        assert_eq!(
            resolve(&lab.paths, &lab.instance, ContentType::Mod, &client)
                .await
                .projects
                .len(),
            1
        );
        let identity = Identity {
            schema_version: 1,
            sha512: hash.clone(),
            content_type: ContentType::Mod,
            project: None,
            checked_at: 0,
        };
        persist_identity(&lab.paths, &identity);
        assert!(read_identity(&lab.paths, ContentType::Mod, &hash).is_none());
    }
    #[tokio::test]
    async fn cross_type_matches_are_not_used_and_empty_directories_are_unavailable() {
        let lab = Lab::new();
        let bytes = archive(ContentType::Mod, "wrong_type");
        lab.write(ContentType::Mod, "local.jar", &bytes);
        let server = server(
            format!("{:x}", sha2::Sha512::digest(&bytes)),
            "resourcepack",
            Arc::new(AtomicBool::new(false)),
        );
        let client = crate::modrinth::Client::for_testing(&format!("{}/v2/", server.base_url()));
        assert!(
            resolve(&lab.paths, &lab.instance, ContentType::Mod, &client)
                .await
                .projects
                .is_empty()
        );
        let empty = resolve(&lab.paths, &lab.instance, ContentType::ShaderPack, &client).await;
        assert!(empty.projects.is_empty());
        assert!(empty.retry_after_ms.is_none());
    }
}
