//! Cosmetic official profile textures. No credential, filesystem or launch authority.
//! Only validated native profile URLs reach transport; the UI receives bounded decoded pixels.
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use url::Url;

const MAX_BYTES: usize = 128 * 1024;
const CACHE_LIMIT: usize = 16;
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinTexture {
    url: Url,
    pub model: String,
}

impl SkinTexture {
    pub fn from_profile_skins(skins: &serde_json::Value) -> Option<Self> {
        let active = skins
            .as_array()?
            .iter()
            .find(|skin| skin["state"] == "ACTIVE")?;
        let url = validated_url(active["url"].as_str()?)?;
        let model = match active["variant"].as_str()? {
            "CLASSIC" => "classic",
            "SLIM" => "slim",
            _ => return None,
        };
        Some(Self {
            url,
            model: model.into(),
        })
    }
}

pub(crate) fn validated_url(value: &str) -> Option<Url> {
    let mut url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str() != Some("textures.minecraft.net")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let digest = url.path().strip_prefix("/texture/")?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    // Official profiles historically publish HTTP texture locators. Upgrade only
    // this exact validated host/path to HTTPS; cleartext is never requested.
    url.set_scheme("https").ok()?;
    Some(url)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadAvatar {
    pub rgba: Vec<u8>,
    pub model: String,
    pub skin_rgba: Vec<u8>,
    pub skin_height: u32,
}

impl HeadAvatar {
    fn valid(&self) -> bool {
        self.rgba.len() == 256
            && matches!(self.skin_height, 32 | 64)
            && self.skin_rgba.len() == 64 * self.skin_height as usize * 4
            && matches!(self.model.as_str(), "classic" | "slim")
    }
}

struct CacheEntry {
    url: Url,
    avatar: HeadAvatar,
    checked_at: Instant,
}
#[derive(Default)]
struct AvatarCache {
    entries: VecDeque<CacheEntry>,
}
impl AvatarCache {
    fn get(&mut self, skin: &SkinTexture) -> Option<HeadAvatar> {
        self.entries
            .retain(|entry| entry.checked_at.elapsed() < CACHE_TTL && entry.avatar.valid());
        self.entries
            .iter()
            .find(|entry| {
                entry.url == skin.url
                    && (entry.avatar.model == skin.model || entry.avatar.skin_height == 32)
            })
            .map(|entry| entry.avatar.clone())
    }
    fn put(&mut self, skin: &SkinTexture, avatar: HeadAvatar) {
        if !avatar.valid() {
            return;
        }
        self.entries.retain(|entry| entry.url != skin.url);
        while self.entries.len() >= CACHE_LIMIT {
            self.entries.pop_front();
        }
        self.entries.push_back(CacheEntry {
            url: skin.url.clone(),
            avatar,
            checked_at: Instant::now(),
        });
    }
}
fn cache() -> &'static Mutex<AvatarCache> {
    static CACHE: OnceLock<Mutex<AvatarCache>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

pub async fn head(skin: &SkinTexture, refresh: bool) -> Option<HeadAvatar> {
    if !refresh {
        if let Some(avatar) = cache().lock().ok()?.get(skin) {
            return Some(avatar);
        }
    }
    crate::downloads::ensure_rustls_crypto_provider();
    let client = reqwest::Client::builder()
        .user_agent(crate::downloads::user_agent())
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .build()
        .ok()?;
    let avatar = fetch(&client, &skin.url, &skin.model).await?;
    cache().lock().ok()?.put(skin, avatar.clone());
    Some(avatar)
}

// Private transport helper permits deterministic loopback tests, never frontend URLs.
async fn fetch(client: &reqwest::Client, url: &Url, model: &str) -> Option<HeadAvatar> {
    let mut response = client.get(url.clone()).send().await.ok()?;
    if response.status() != reqwest::StatusCode::OK
        || response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)?
            .to_str()
            .ok()?
            .split(';')
            .next()?
            .trim()
            != "image/png"
        || response
            .content_length()
            .is_some_and(|length| length > MAX_BYTES as u64)
    {
        return None;
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if body.len().checked_add(chunk.len())? > MAX_BYTES {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    decode(&body, model)
}

fn decode(bytes: &[u8], model: &str) -> Option<HeadAvatar> {
    if !matches!(model, "classic" | "slim")
        || bytes.len() > MAX_BYTES
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
    {
        return None;
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: 1024 * 1024 });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    if reader.info().width != 64
        || !matches!(reader.info().height, 32 | 64)
        || reader.info().animation_control.is_some()
    {
        return None;
    }
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).ok()?;
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return None,
    };
    let mut skin_rgba = Vec::with_capacity(64 * info.height as usize * 4);
    for pixel in pixels[..info.buffer_size()].chunks_exact(channels) {
        skin_rgba.extend_from_slice(&pixel[..3]);
        skin_rgba.push(if channels == 4 { pixel[3] } else { 255 });
    }
    // Legacy opaque hat backgrounds mean no hat, as in the game renderer.
    if info.height == 32
        && (0..32).all(|y| (32..64).all(|x| skin_rgba[(y * 64 + x) * 4 + 3] == 255))
    {
        // Only hat UVs are cleared; the lower half contains the base right arm.
        for y in 0..16 {
            for x in 32..64 {
                skin_rgba[(y * 64 + x) * 4 + 3] = 0;
            }
        }
    }
    let mut rgba = Vec::with_capacity(256);
    for y in 8..16 {
        for x in 8..16 {
            let base = (y * 64 + x) * channels;
            let hat = (y * 64 + x + 32) * channels;
            let base_alpha = if channels == 4 {
                pixels[base + 3] as u32
            } else {
                255
            };
            let hat_alpha = skin_rgba[(y * 64 + x + 32) * 4 + 3] as u32;
            let alpha = hat_alpha * 255 + base_alpha * (255 - hat_alpha);
            for channel in 0..3 {
                let value = if alpha == 0 {
                    0
                } else {
                    (pixels[hat + channel] as u32 * hat_alpha * 255
                        + pixels[base + channel] as u32 * base_alpha * (255 - hat_alpha)
                        + alpha / 2)
                        / alpha
                };
                rgba.push(value as u8);
            }
            rgba.push(((alpha + 127) / 255) as u8);
        }
    }
    Some(HeadAvatar {
        rgba,
        model: if info.height == 32 {
            "classic".into()
        } else {
            model.into()
        },
        skin_rgba,
        skin_height: info.height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestResponse, TestServer};
    use std::sync::Arc;

    fn skin() -> SkinTexture {
        SkinTexture::from_profile_skins(&serde_json::json!([{"state":"ACTIVE","variant":"SLIM","url":format!("https://textures.minecraft.net/texture/{}", "a".repeat(64))}])).unwrap()
    }
    fn fixture(height: u32) -> Vec<u8> {
        let mut body = Vec::new();
        let mut pixels = vec![0u8; 64 * height as usize * 4];
        for y in 8..16 {
            for x in 8..16 {
                pixels[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4].copy_from_slice(&[100, 0, 0, 255]);
                pixels[(y * 64 + x + 32) * 4..(y * 64 + x + 32) * 4 + 4]
                    .copy_from_slice(&[0, 100, 0, 128]);
            }
        }
        {
            let mut encoder = png::Encoder::new(&mut body, 64, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
        body
    }
    #[test]
    fn official_active_texture_and_authoritative_model() {
        assert_eq!(skin().model, "slim");
        let http = format!("http://textures.minecraft.net/texture/{}", "a".repeat(64));
        assert_eq!(validated_url(&http).unwrap().scheme(), "https");
        assert!(SkinTexture::from_profile_skins(&serde_json::json!([])).is_none());
        assert!(SkinTexture::from_profile_skins(&serde_json::json!({"url":"bad"})).is_none());
    }
    #[test]
    fn texture_locator_rejects_untrusted_destinations() {
        for url in [
            "file:///texture/a",
            "ftp://textures.minecraft.net/texture/a",
            "https://evil.example/texture/a",
            "https://textures.minecraft.net.evil.example/texture/a",
            "https://textures.minecraft.net:8443/texture/a",
            "https://user@textures.minecraft.net/texture/a",
            "https://textures.minecraft.net/texture/../a",
        ] {
            assert!(validated_url(url).is_none(), "{url}");
        }
        let valid = skin().url;
        assert!(validated_url(&format!("{valid}?query=x")).is_none());
        assert!(validated_url(&format!("{valid}#fragment")).is_none());
    }
    #[test]
    fn modern_and_legacy_head_composite_overlay_alpha() {
        for height in [32, 64] {
            let head = decode(&fixture(height), "classic").unwrap();
            assert_eq!(head.rgba.len(), 256);
            assert_eq!(head.skin_rgba.len(), 64 * height as usize * 4);
            assert_eq!(head.skin_height, height);
            assert_eq!(
                &head.skin_rgba[(8 * 64 + 8) * 4..(8 * 64 + 8) * 4 + 4],
                &[100, 0, 0, 255]
            );
            assert_eq!(&head.rgba[..4], &[50, 50, 0, 255]);
        }
    }
    #[test]
    fn legacy_is_classic_and_corrupt_body_is_not_cached() {
        assert_eq!(decode(&fixture(32), "slim").unwrap().model, "classic");
        let mut avatar = decode(&fixture(64), "slim").unwrap();
        avatar.skin_rgba.pop();
        let mut cache = AvatarCache::default();
        cache.put(&skin(), avatar);
        assert!(cache.get(&skin()).is_none());
    }
    #[test]
    fn opaque_legacy_hat_background_does_not_erase_the_base_arm() {
        let mut bytes = Vec::new();
        let pixels = vec![100u8; 64 * 32 * 3];
        {
            let mut encoder = png::Encoder::new(&mut bytes, 64, 32);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
        let avatar = decode(&bytes, "classic").unwrap();
        assert_eq!(avatar.skin_rgba[(8 * 64 + 40) * 4 + 3], 0);
        assert_eq!(avatar.skin_rgba[(20 * 64 + 44) * 4 + 3], 255);
        assert_eq!(&avatar.rgba[..4], &[100, 100, 100, 255]);
    }
    #[test]
    fn malformed_oversized_and_wrong_dimensions_are_cosmetic_failures() {
        assert!(decode(b"not png", "classic").is_none());
        assert!(decode(&vec![0; MAX_BYTES + 1], "classic").is_none());
        assert!(decode(&fixture(16), "classic").is_none());
        let mut corrupt = fixture(64);
        corrupt.truncate(40);
        assert!(decode(&corrupt, "classic").is_none());
    }
    #[test]
    fn memory_cache_is_bounded_validated_and_refreshable() {
        let mut cache = AvatarCache::default();
        let mut texture = skin();
        let avatar = decode(&fixture(64), "slim").unwrap();
        cache.put(&texture, avatar.clone());
        assert_eq!(cache.get(&texture), Some(avatar.clone()));
        cache.entries[0].avatar.rgba.clear();
        assert!(cache.get(&texture).is_none());
        cache.put(&texture, avatar.clone());
        cache.entries[0].checked_at = Instant::now() - CACHE_TTL;
        assert!(cache.get(&texture).is_none());
        for value in 0..20 {
            texture.url = validated_url(&format!(
                "https://textures.minecraft.net/texture/{value:064x}"
            ))
            .unwrap();
            cache.put(&texture, avatar.clone());
        }
        assert_eq!(cache.entries.len(), CACHE_LIMIT);
        texture.model = "classic".into();
        assert!(cache.get(&texture).is_none());
    }
    #[tokio::test]
    async fn bounded_transport_failures_and_success_use_only_loopback() {
        crate::downloads::ensure_rustls_crypto_provider();
        let png = fixture(64);
        let server = TestServer::spawn(Arc::new(move |request| match request.path.as_str() {
            "/ok" => TestResponse::ok(&png).with_header("Content-Type", "image/png"),
            "/oversized" => TestResponse::ok(&vec![0; MAX_BYTES + 1])
                .with_header("Content-Type", "image/png")
                .with_close_framing(),
            "/invalid" => TestResponse::ok(b"malformed").with_header("Content-Type", "image/png"),
            "/wrong-type" => TestResponse::ok(&png).with_header("Content-Type", "text/html"),
            "/redirect" => TestResponse::redirect_to(format!("{}/ok", request.base_url)),
            _ => TestResponse::status(503),
        }));
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let url = |path| Url::parse(&format!("{}{path}", server.base_url())).unwrap();
        assert!(fetch(&client, &url("/ok"), "slim").await.is_some());
        for path in [
            "/oversized",
            "/invalid",
            "/wrong-type",
            "/redirect",
            "/failure",
        ] {
            assert!(fetch(&client, &url(path), "slim").await.is_none(), "{path}");
        }
    }
}
