//! Persisted provider project artwork for installed content surfaces.
//! Installed rows render Aurora-cached bytes, never live provider URLs: the
//! cache is keyed by provider identity, bounded, decoded and normalized through
//! the shared image policy. Static WebP sources normalize to PNG; only validated
//! pixels replace stored objects.
//! Artwork is cosmetic;
//! every failure degrades to the caller's generic fallback.

use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

use uuid::Uuid;

use crate::cosmetic_image::{self, ARTWORK, ValidatedPng};
use crate::paths::ManagedPaths;

const MAX_ARTWORK: usize = ARTWORK.max_encoded_bytes;
const OFFICIAL_CDN_HOST: &str = "cdn.modrinth.com";

pub(crate) fn project_lock(key: &str) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, std::sync::Weak<tokio::sync::Mutex<()>>>>> =
        OnceLock::new();
    let mut locks = LOCKS.get_or_init(Default::default).lock().unwrap();
    locks.retain(|_, value| value.strong_count() > 0);
    if let Some(lock) = locks.get(key).and_then(std::sync::Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    locks.insert(key.into(), Arc::downgrade(&lock));
    lock
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkResult {
    pub source: Option<String>,
    pub status: &'static str,
    pub retry_after_ms: Option<u64>,
}
type ResultMemo = HashMap<String, (std::time::Instant, ArtworkResult)>;
fn result_memo() -> &'static Mutex<ResultMemo> {
    static RESULTS: OnceLock<Mutex<ResultMemo>> = OnceLock::new();
    RESULTS.get_or_init(Default::default)
}
impl ArtworkResult {
    fn available(source: String) -> Self {
        Self {
            source: Some(source),
            status: "available",
            retry_after_ms: None,
        }
    }
    fn unavailable() -> Self {
        Self {
            source: None,
            status: "unavailable",
            retry_after_ms: None,
        }
    }
    fn retry() -> Self {
        Self {
            source: None,
            status: "retryable",
            retry_after_ms: Some(60_000),
        }
    }
}

/// Provider project ids are exact 8-character base62 identifiers, matching the
/// upstream validation; nothing derived from user text reaches a path.
fn valid_id(project_id: &str) -> bool {
    project_id.len() == 8 && project_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn object_path(paths: &ManagedPaths, project_id: &str) -> PathBuf {
    paths
        .cache_dir()
        .join("artwork")
        .join("modrinth")
        .join(format!("{project_id}.img"))
}

fn data_url(bytes: &[u8]) -> Option<String> {
    cosmetic_image::validate_artwork(bytes).map(|image| image.data_url())
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("artwork cache path has no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn persist(paths: &ManagedPaths, project_id: &str, image: &ValidatedPng) -> Option<()> {
    let path = object_path(paths, project_id);
    let root = paths.data_root().canonicalize().ok()?;
    // Validate each fixed ancestor before creating the next directory.
    for directory in [
        paths.cache_dir(),
        paths.cache_dir().join("artwork"),
        path.parent()?.to_owned(),
    ] {
        if !directory.exists() {
            std::fs::create_dir(&directory).ok()?;
        }
        if !directory.canonicalize().ok()?.starts_with(&root) {
            return None;
        }
    }
    write_atomic(&path, image.bytes()).ok()
}

/// Cache-first read of one project's stored artwork.
pub fn read_cached(paths: &ManagedPaths, project_id: &str) -> Option<String> {
    if !valid_id(project_id) {
        return None;
    }
    let path = object_path(paths, project_id);
    if !std::fs::symlink_metadata(&path).ok()?.file_type().is_file()
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
        .take((MAX_ARTWORK + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    let image = cosmetic_image::validate_artwork(&bytes)?;
    // Validated legacy WebP objects become canonical PNG once, so ordinary
    // navigation and offline restarts need no decoder worker or CDN request.
    if bytes.starts_with(b"RIFF") {
        let _ = persist(paths, project_id, &image);
    }
    Some(image.data_url())
}

/// Bounded acquisition of one provider-published icon. The locator comes from
/// the provider's own project document (never the frontend), and redirects
/// must stay on the official CDN host.
enum DownloadFailure {
    Retryable,
    Unavailable,
}
async fn download(
    http: &reqwest::Client,
    url: &str,
    allowed_host: &str,
) -> Result<ValidatedPng, DownloadFailure> {
    let mut response = http
        .get(url)
        .send()
        .await
        .map_err(|_| DownloadFailure::Retryable)?;
    if !response.status().is_success() {
        return Err(if response.status() == reqwest::StatusCode::NOT_FOUND {
            DownloadFailure::Unavailable
        } else {
            DownloadFailure::Retryable
        });
    }
    let final_host = response
        .url()
        .host_str()
        .ok_or(DownloadFailure::Unavailable)?
        .to_ascii_lowercase();
    if final_host != allowed_host.to_ascii_lowercase() {
        return Err(DownloadFailure::Unavailable);
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ARTWORK as u64)
    {
        return Err(DownloadFailure::Unavailable);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| DownloadFailure::Retryable)?
    {
        if body.len() + chunk.len() > MAX_ARTWORK {
            return Err(DownloadFailure::Unavailable);
        }
        body.extend_from_slice(&chunk);
    }
    cosmetic_image::validate_artwork(&body).ok_or(DownloadFailure::Unavailable)
}

/// Cache-first artwork for installed content rows. A miss resolves the icon
/// through the provider's project authority, downloads bounded bytes once and
/// persists them; concurrent requests wait for the same completed acquisition.
pub async fn cached_artwork(paths: &ManagedPaths, project_id: &str) -> Option<String> {
    resolve_artwork(paths, project_id).await.source
}

pub async fn resolve_artwork(paths: &ManagedPaths, project_id: &str) -> ArtworkResult {
    if project_id == "aurora" {
        // Bundled first-party source, pinned independently of names/provider IDs.
        let bytes = include_bytes!("../icons/128x128.png");
        use sha2::Digest as _;
        if format!("{:x}", sha2::Sha256::digest(bytes))
            == "8e75257cc872c7df2078597f05a403b990f10b7b4b5f139015623fa74b0fee98"
        {
            if let Some(source) = data_url(bytes) {
                return ArtworkResult::available(source);
            }
        }
        return ArtworkResult::unavailable();
    }
    if !valid_id(project_id) {
        return ArtworkResult::unavailable();
    }
    resolve_with(paths, project_id, || acquire_artwork(paths, project_id)).await
}

async fn resolve_with<F, Fut>(paths: &ManagedPaths, project_id: &str, acquire: F) -> ArtworkResult
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ArtworkResult>,
{
    let key = format!("{}:{project_id}", paths.data_root().display());
    let lock = project_lock(&key);
    let _guard = lock.lock().await;
    if let Some(hit) = read_cached(paths, project_id) {
        return ArtworkResult::available(hit);
    }
    let results = result_memo();
    if let Some((expires, result)) = results.lock().unwrap().get(&key) {
        if *expires > std::time::Instant::now() {
            return result.clone();
        }
    }
    static SLOTS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    let _slot = SLOTS
        .get_or_init(|| tokio::sync::Semaphore::new(4))
        .acquire()
        .await
        .unwrap();
    let result = acquire().await;
    if result.source.is_none() {
        let mut entries = results.lock().unwrap();
        entries.retain(|_, (expires, _)| *expires > std::time::Instant::now());
        if entries.len() >= 128 {
            entries.clear();
        }
        entries.insert(
            key,
            (
                std::time::Instant::now()
                    + std::time::Duration::from_millis(result.retry_after_ms.unwrap_or(600_000)),
                result.clone(),
            ),
        );
    }
    result
}

async fn acquire_artwork(paths: &ManagedPaths, project_id: &str) -> ArtworkResult {
    let url = match crate::modrinth::Client::official()
        .artwork(project_id)
        .await
    {
        Ok(Some(url)) => url,
        Ok(None) | Err(crate::modrinth::Error::NotFound) => return ArtworkResult::unavailable(),
        Err(_) => return ArtworkResult::retry(),
    };
    // Cosmetic CDN locators do not require redirects. Refusing all redirects
    // prevents even an intermediate request to a provider-controlled other host.
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("aurora-launcher/{}", env!("CARGO_PKG_VERSION")))
        .build();
    let Ok(http) = http else {
        return ArtworkResult::retry();
    };
    let image = match download(&http, &url, OFFICIAL_CDN_HOST).await {
        Ok(image) => image,
        Err(DownloadFailure::Retryable) => return ArtworkResult::retry(),
        Err(DownloadFailure::Unavailable) => return ArtworkResult::unavailable(),
    };
    // A stale or damaged cosmetic object is replaced only by validated pixels.
    let _ = persist(paths, project_id, &image);
    ArtworkResult::available(image.data_url())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosmetic_image::fixtures;
    use crate::test_support::{TestResponse, TestServer};
    use std::sync::Arc;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest-of-image";

    fn small_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 32, 32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&vec![127; 32 * 32 * 4])
                .unwrap();
        }
        bytes
    }

    fn paths() -> ManagedPaths {
        ManagedPaths::from_app_local_data_dir(
            std::env::temp_dir().join(format!("aurora-artwork-test-{}", Uuid::new_v4())),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn coalesced_acquisition_restart_offline_and_corrupt_replacement() {
        let managed = paths();
        std::fs::create_dir_all(managed.data_root()).unwrap();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let acquire = || async {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tokio::task::yield_now().await;
            let image = cosmetic_image::validate_artwork(&small_png()).unwrap();
            persist(&managed, "COALESCE", &image).unwrap();
            ArtworkResult::available(image.data_url())
        };
        let (a, b) = tokio::join!(
            resolve_with(&managed, "COALESCE", acquire),
            resolve_with(&managed, "COALESCE", acquire)
        );
        assert_eq!(a.source, b.source);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        let restarted =
            ManagedPaths::from_app_local_data_dir(managed.data_root().to_owned()).unwrap();
        assert_eq!(
            resolve_with(&restarted, "COALESCE", || async {
                panic!("offline cache hit must never acquire")
            })
            .await
            .source,
            a.source
        );
        write_atomic(&object_path(&managed, "COALESCE"), b"damaged").unwrap();
        assert!(read_cached(&managed, "COALESCE").is_none());
        assert!(
            resolve_with(&managed, "COALESCE", acquire)
                .await
                .source
                .is_some()
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        std::fs::remove_dir_all(managed.data_root()).unwrap();
    }

    #[tokio::test]
    async fn failed_lookup_can_retry_but_absent_artwork_is_distinct() {
        let managed = paths();
        std::fs::create_dir_all(managed.data_root()).unwrap();
        let failed = resolve_with(&managed, "RETRYING", || async { ArtworkResult::retry() }).await;
        assert_eq!(failed.status, "retryable");
        assert_eq!(
            resolve_with(&managed, "RETRYING", || async { panic!("backoff") })
                .await
                .status,
            "retryable"
        );
        let key = format!("{}:RETRYING", managed.data_root().display());
        result_memo().lock().unwrap().get_mut(&key).unwrap().0 = std::time::Instant::now();
        let success = resolve_with(&managed, "RETRYING", || async {
            let image = cosmetic_image::validate_artwork(&small_png()).unwrap();
            persist(&managed, "RETRYING", &image).unwrap();
            ArtworkResult::available(image.data_url())
        })
        .await;
        assert_eq!(success.status, "available");
        assert_eq!(
            resolve_with(&managed, "NOARTNOW", || async {
                ArtworkResult::unavailable()
            })
            .await
            .status,
            "unavailable"
        );
        assert_eq!(
            resolve_with(&managed, "NOARTNOW", || async { panic!("negative reuse") })
                .await
                .status,
            "unavailable"
        );
        std::fs::remove_dir_all(managed.data_root()).unwrap();
    }

    #[test]
    fn cached_webp_is_bounded_revalidated_and_saved_as_canonical_png() {
        let managed = paths();
        std::fs::create_dir_all(managed.data_root()).unwrap();
        let mut webp = Vec::new();
        image_webp::WebPEncoder::new(&mut webp)
            .encode(&[1, 2, 3, 127], 1, 1, image_webp::ColorType::Rgba8)
            .unwrap();
        let image = cosmetic_image::validate_artwork(&webp).unwrap();
        persist(&managed, "WEBPICON", &image).unwrap();
        assert!(
            std::fs::read(object_path(&managed, "WEBPICON"))
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
        assert_eq!(read_cached(&managed, "WEBPICON"), Some(image.data_url()));
        write_atomic(&object_path(&managed, "OLDWEBPS"), &webp).unwrap();
        assert_eq!(read_cached(&managed, "OLDWEBPS"), Some(image.data_url()));
        std::fs::remove_dir_all(managed.data_root()).unwrap();
    }

    #[test]
    fn only_decoded_static_png_becomes_a_data_url() {
        assert!(data_url(&small_png()).is_some());
        for bytes in [
            PNG,
            b"\xFF\xD8\xFF",
            b"GIF89aanim",
            b"RIFF\0\0\0\0WEBPVP8 ",
            b"<html>",
            b"",
        ] {
            assert!(data_url(bytes).is_none());
        }
    }

    #[test]
    fn cache_roundtrip_is_bounded_and_path_safe() {
        let managed = paths();
        assert!(read_cached(&managed, "../escape").is_none());
        assert!(read_cached(&managed, "toolfriendly").is_none());
        write_atomic(&object_path(&managed, "AAAABBBB"), &small_png()).unwrap();
        let url = read_cached(&managed, "AAAABBBB").unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        // An undecodable stored object never becomes renderable data.
        write_atomic(&object_path(&managed, "BBBBCCCC"), b"<html>bytes").unwrap();
        assert!(read_cached(&managed, "BBBBCCCC").is_none());
        write_atomic(&object_path(&managed, "CCCCDDDD"), PNG).unwrap();
        assert!(read_cached(&managed, "CCCCDDDD").is_none());
        assert!(data_url(&vec![0; MAX_ARTWORK + 1]).is_none());
        std::fs::remove_dir_all(managed.data_root()).unwrap();
    }

    #[test]
    fn old_cache_images_are_revalidated_without_mutating_them() {
        let managed = paths();
        for bytes in [
            fixtures::JPEG_4096,
            fixtures::JPEG_8192,
            b"\xFF\xD8\xFF",
            &fixtures::png(1025, 1),
            &fixtures::animated_png(),
        ] {
            let path = object_path(&managed, "OLDCACHE");
            write_atomic(&path, bytes).unwrap();
            assert!(read_cached(&managed, "OLDCACHE").is_none());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        let validated = cosmetic_image::validate(&small_png(), ARTWORK).unwrap();
        persist(&managed, "NEWCACHE", &validated).unwrap();
        assert_eq!(
            read_cached(&managed, "NEWCACHE"),
            Some(validated.data_url())
        );
        std::fs::remove_dir_all(managed.data_root()).unwrap();
    }

    #[tokio::test]
    async fn acquisition_stays_on_the_official_cdn_and_is_bounded() {
        crate::downloads::ensure_rustls_crypto_provider();
        let server = TestServer::spawn(Arc::new(move |request| match request.path.as_str() {
            "/icon.png" => TestResponse::ok(&small_png()).with_header("Content-Type", "image/png"),
            "/truncated" => TestResponse::ok(PNG),
            "/jpeg" => TestResponse::ok(b"\xFF\xD8\xFF").with_header("Content-Type", "image/png"),
            "/jpeg4096" => TestResponse::ok(fixtures::JPEG_4096),
            "/jpeg8192" => TestResponse::ok(fixtures::JPEG_8192),
            "/animated" => TestResponse::ok(&fixtures::animated_png()),
            "/redirect-away" => TestResponse::redirect_to("https://example.com/icon.png".into()),
            "/oversized" => TestResponse::ok(&vec![0u8; MAX_ARTWORK + 1])
                .with_header("Content-Type", "image/png")
                .with_close_framing(),
            "/html" => TestResponse::ok(b"<html>").with_header("Content-Type", "image/png"),
            _ => TestResponse::status(404),
        }));
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let host = url::Url::parse(&server.base_url())
            .unwrap()
            .host_str()
            .unwrap()
            .to_owned();
        let icon = format!("{}/icon.png", server.base_url());
        assert!(download(&http, &icon, &host).await.is_ok());
        for path in [
            "/redirect-away",
            "/oversized",
            "/html",
            "/missing",
            "/truncated",
            "/jpeg",
            "/jpeg4096",
            "/jpeg8192",
            "/animated",
        ] {
            let url = format!("{}{path}", server.base_url());
            assert!(download(&http, &url, &host).await.is_err(), "{path}");
        }
        let wrong_host = "cdn.modrinth.com".to_owned();
        assert!(download(&http, &icon, &wrong_host).await.is_err());
    }

    #[tokio::test]
    async fn duplicate_acquisitions_wait_for_the_same_lock() {
        let first = project_lock("DUPLICAT");
        let second = project_lock("DUPLICAT");
        assert!(Arc::ptr_eq(&first, &second));
        let guard = first.lock().await;
        assert!(second.try_lock().is_err());
        drop(guard);
        assert!(second.try_lock().is_ok());
    }
}
