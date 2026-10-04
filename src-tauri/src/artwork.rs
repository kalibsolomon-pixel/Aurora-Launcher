//! Persisted provider project artwork for installed content surfaces.
//! Installed rows render Aurora-cached bytes, never live provider URLs: the
//! cache is keyed by managed provider identity, bounded, magic-validated and
//! only replaced through a freshly verified acquisition. Artwork is cosmetic;
//! every failure degrades to the caller's generic fallback.

use std::{
    collections::HashSet,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use base64ct::{Base64, Encoding as _};
use uuid::Uuid;

use crate::paths::ManagedPaths;

const MAX_ARTWORK: usize = 512 * 1024;
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

/// Magic-byte image typing; unknown bytes never become renderable data.
fn sniff_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn data_url(bytes: &[u8]) -> Option<String> {
    if bytes.len() > MAX_ARTWORK {
        return None;
    }
    let media_type = sniff_media_type(bytes)?;
    if media_type == "image/png" {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_limits(png::Limits {
            bytes: 16 * 1024 * 1024,
        });
        let mut reader = decoder.read_info().ok()?;
        let info = reader.info();
        if info.width == 0 || info.height == 0 || info.width > 1024 || info.height > 1024 {
            return None;
        }
        let mut pixels = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut pixels).ok()?;
    }
    Some(format!(
        "data:{media_type};base64,{}",
        Base64::encode_string(bytes)
    ))
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

fn persist(paths: &ManagedPaths, project_id: &str, bytes: &[u8]) -> Option<()> {
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
    write_atomic(&path, bytes).ok()
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
async fn download(http: &reqwest::Client, url: &str, allowed_host: &str) -> Option<Vec<u8>> {
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
    sniff_media_type(&body)?;
    Some(body)
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
    let result = download(&http, &url, OFFICIAL_CDN_HOST).await;
    let image = result.as_deref().and_then(data_url)?;
    if let Some(bytes) = &result {
        // A stale or damaged cache object is replaced only by verified bytes.
        let _ = persist(paths, project_id, bytes);
    }
    Some(image)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn media_typing_accepts_known_formats_only() {
        assert_eq!(sniff_media_type(PNG), Some("image/png"));
        assert_eq!(sniff_media_type(b"\xFF\xD8\xFFjpeg"), Some("image/jpeg"));
        assert_eq!(sniff_media_type(b"GIF89aanim"), Some("image/gif"));
        assert_eq!(
            sniff_media_type(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
            Some("image/webp")
        );
        assert_eq!(sniff_media_type(b"<html>"), None);
        assert_eq!(sniff_media_type(b""), None);
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

    #[tokio::test]
    async fn acquisition_stays_on_the_official_cdn_and_is_bounded() {
        crate::downloads::ensure_rustls_crypto_provider();
        let server = TestServer::spawn(Arc::new(move |request| match request.path.as_str() {
            "/icon.png" => TestResponse::ok(PNG).with_header("Content-Type", "image/png"),
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
        for path in ["/redirect-away", "/oversized", "/html", "/missing"] {
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
