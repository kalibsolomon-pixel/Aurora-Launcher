//! Persisted provider project artwork for installed content surfaces.
//! Installed rows render Aurora-cached bytes, never live provider URLs: the
//! cache is keyed by provider identity, bounded, decoded and normalized through
//! the shared static-PNG policy. Only freshly validated acquisition replaces it.
//! Artwork is cosmetic;
//! every failure degrades to the caller's generic fallback.

use std::{
    collections::HashSet,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use uuid::Uuid;

use crate::cosmetic_image::{self, ARTWORK, ValidatedPng};
use crate::paths::ManagedPaths;

const MAX_ARTWORK: usize = ARTWORK.max_encoded_bytes;
const OFFICIAL_CDN_HOST: &str = "cdn.modrinth.com";

fn in_flight() -> &'static Mutex<HashSet<String>> {
    static IN_FLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

struct Acquisition(String);
impl Acquisition {
    fn start(project_id: &str) -> Option<Self> {
        in_flight()
            .lock()
            .ok()?
            .insert(project_id.to_owned())
            .then(|| Self(project_id.to_owned()))
    }
}
impl Drop for Acquisition {
    fn drop(&mut self) {
        if let Ok(mut set) = in_flight().lock() {
            set.remove(&self.0);
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
    cosmetic_image::validate(bytes, ARTWORK).map(|image| image.data_url())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
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
    data_url(&bytes)
}

/// Bounded acquisition of one provider-published icon. The locator comes from
/// the provider's own project document (never the frontend), and redirects
/// must stay on the official CDN host.
async fn download(http: &reqwest::Client, url: &str, allowed_host: &str) -> Option<ValidatedPng> {
    let mut response = http.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let final_host = response.url().host_str()?.to_ascii_lowercase();
    if final_host != allowed_host.to_ascii_lowercase() {
        return None;
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ARTWORK as u64)
    {
        return None;
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if body.len() + chunk.len() > MAX_ARTWORK {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    cosmetic_image::validate(&body, ARTWORK)
}

/// Cache-first artwork for installed content rows. A miss resolves the icon
/// through the provider's project authority, downloads bounded bytes once and
/// persists them; concurrent requests for the same project fall back this pass
/// instead of duplicating network work.
pub async fn cached_artwork(paths: &ManagedPaths, project_id: &str) -> Option<String> {
    if !valid_id(project_id) {
        return None;
    }
    if let Some(hit) = read_cached(paths, project_id) {
        return Some(hit);
    }
    let _acquisition = Acquisition::start(project_id)?;
    let url = crate::modrinth::Client::official()
        .artwork(project_id)
        .await
        .ok()??;
    // Cosmetic CDN locators do not require redirects. Refusing all redirects
    // prevents even an intermediate request to a provider-controlled other host.
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("aurora-launcher/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;
    let image = download(&http, &url, OFFICIAL_CDN_HOST).await?;
    // A stale or damaged cosmetic object is replaced only by validated pixels.
    let _ = persist(paths, project_id, &image);
    Some(image.data_url())
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
        assert!(download(&http, &icon, &host).await.is_some());
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
            assert!(download(&http, &url, &host).await.is_none(), "{path}");
        }
        let wrong_host = "cdn.modrinth.com".to_owned();
        assert!(download(&http, &icon, &wrong_host).await.is_none());
    }

    #[test]
    fn duplicate_acquisitions_are_not_duplicated_in_flight() {
        let guard = Acquisition::start("DUPLICAT").unwrap();
        assert!(Acquisition::start("DUPLICAT").is_none());
        drop(guard);
        assert!(Acquisition::start("DUPLICAT").is_some());
    }
}
