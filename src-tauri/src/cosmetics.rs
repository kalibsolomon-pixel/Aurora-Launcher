//! Launcher-local skin presets and narrowly scoped Minecraft Services cosmetics.
//! Tokens, remote URLs and filesystem paths never cross the command boundary.
use std::{
    collections::HashSet,
    io::Cursor,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    auth::{accounts::AccountId, credentials::SecretString},
    paths::ManagedPaths,
};

const MAX_PNG: usize = 128 * 1024;
const MAX_PRESETS: usize = 64;
const MAX_RESPONSE: usize = 256 * 1024;
const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SKIN_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins";
const CAPE_URL: &str = "https://api.minecraftservices.com/minecraft/profile/capes/active";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkinModel {
    Classic,
    Slim,
}
impl SkinModel {
    fn service(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Slim => "slim",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkinPreset {
    pub id: String,
    pub name: String,
    pub model: SkinModel,
    pub imported_at: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PresetDocument {
    schema_version: u32,
    presets: Vec<SkinPreset>,
}
impl Default for PresetDocument {
    fn default() -> Self {
        Self {
            schema_version: 1,
            presets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cape {
    pub id: String,
    pub name: String,
    pub selected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<CapePreview>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapePreview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CosmeticsState {
    pub account_id: String,
    pub current_skin_model: Option<SkinModel>,
    pub has_current_skin: bool,
    pub capes: Vec<Cape>,
}

#[derive(Debug, Clone, Copy)]
pub struct CosmeticsError {
    pub code: &'static str,
    pub message: &'static str,
}
type Result<T> = std::result::Result<T, CosmeticsError>;
fn error(code: &'static str, message: &'static str) -> CosmeticsError {
    CosmeticsError { code, message }
}
fn storage_error() -> CosmeticsError {
    error(
        "skin_storage_unavailable",
        "The managed skin preset storage is unavailable.",
    )
}
fn malformed() -> CosmeticsError {
    error(
        "skin_presets_malformed",
        "Skin preset metadata is damaged; it was left unchanged.",
    )
}
fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
fn root(paths: &ManagedPaths) -> std::path::PathBuf {
    paths.launcher_dir().join("skin-presets")
}
fn metadata_path(paths: &ManagedPaths) -> std::path::PathBuf {
    root(paths).join("presets.json")
}
fn png_path(paths: &ManagedPaths, id: &str) -> Result<std::path::PathBuf> {
    if Uuid::parse_str(id).is_err() || id.len() != 36 || id != id.to_ascii_lowercase() {
        return Err(error(
            "skin_preset_missing",
            "That saved skin preset is unavailable.",
        ));
    }
    Ok(root(paths).join(format!("{id}.png")))
}
fn validate_document(document: &PresetDocument) -> Result<()> {
    if document.schema_version != 1 {
        return Err(error(
            "skin_presets_schema_unsupported",
            "Skin preset metadata uses a newer schema; it was left unchanged.",
        ));
    }
    if document.presets.len() > MAX_PRESETS {
        return Err(malformed());
    }
    let mut ids = HashSet::new();
    for preset in &document.presets {
        if png_path_dummy(&preset.id).is_err()
            || !ids.insert(&preset.id)
            || preset.name.trim().is_empty()
            || preset.name.len() > 80
            || preset.sha256.len() != 64
            || !preset.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(malformed());
        }
    }
    Ok(())
}
fn png_path_dummy(id: &str) -> Result<()> {
    if Uuid::parse_str(id).is_err() || id.len() != 36 || id != id.to_ascii_lowercase() {
        Err(malformed())
    } else {
        Ok(())
    }
}
fn read_document(paths: &ManagedPaths) -> Result<PresetDocument> {
    let bytes = match std::fs::read(metadata_path(paths)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(PresetDocument::default()),
        Err(_) => return Err(storage_error()),
    };
    if bytes.len() > 32 * 1024 {
        return Err(malformed());
    }
    let document: PresetDocument = serde_json::from_slice(&bytes).map_err(|_| malformed())?;
    validate_document(&document)?;
    Ok(document)
}
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(storage_error)?;
    std::fs::create_dir_all(parent).map_err(|_| storage_error())?;
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let result = (|| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| storage_error())?;
        file.write_all(bytes).map_err(|_| storage_error())?;
        file.sync_all().map_err(|_| storage_error())?;
        std::fs::rename(&temporary, path).map_err(|_| storage_error())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
fn save_document(paths: &ManagedPaths, document: &PresetDocument) -> Result<()> {
    validate_document(document)?;
    let bytes = serde_json::to_vec_pretty(document).map_err(|_| storage_error())?;
    write_atomic(&metadata_path(paths), &bytes)
}

/// Decode the entire PNG under a strict size/dimension bound before storing or uploading it.
fn validate_png(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_PNG {
        return Err(error(
            "skin_file_too_large",
            "The PNG exceeds the 128 KiB skin limit.",
        ));
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(error("unsupported_skin_image", "Choose a PNG skin image."));
    }
    validate_png_chunks(bytes)?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: 1024 * 1024 });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|_| error("skin_png_malformed", "The PNG could not be decoded."))?;
    if reader.info().width != 64
        || !matches!(reader.info().height, 32 | 64)
        || reader.info().animation_control.is_some()
    {
        return Err(error(
            "unsupported_skin_image",
            "Java skins must be 64×64 or 64×32 PNG images.",
        ));
    }
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut pixels)
        .map_err(|_| error("skin_png_malformed", "The PNG image data is malformed."))?;
    if !matches!(frame.color_type, png::ColorType::Rgb | png::ColorType::Rgba) {
        return Err(error(
            "unsupported_skin_image",
            "The PNG must use RGB or RGBA colors.",
        ));
    }
    Ok(())
}
fn validate_png_chunks(bytes: &[u8]) -> Result<()> {
    let bad = || {
        error(
            "skin_png_malformed",
            "The PNG image structure is malformed.",
        )
    };
    let mut offset = 8usize;
    let mut saw_image = false;
    let mut first = true;
    loop {
        if offset.checked_add(12).is_none_or(|end| end > bytes.len()) {
            return Err(bad());
        }
        let length =
            u32::from_be_bytes(bytes[offset..offset + 4].try_into().map_err(|_| bad())?) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(bad)?;
        if end > bytes.len() {
            return Err(bad());
        }
        let kind = &bytes[offset + 4..offset + 8];
        if first && (kind != b"IHDR" || length != 13) {
            return Err(bad());
        }
        first = false;
        let payload_end = offset + 8 + length;
        let mut crc = !0u32;
        for byte in &bytes[offset + 4..payload_end] {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        let recorded = u32::from_be_bytes(bytes[payload_end..end].try_into().map_err(|_| bad())?);
        if !crc != recorded {
            return Err(bad());
        }
        if kind == b"IDAT" {
            saw_image = true;
        }
        if kind == b"IEND" {
            return if length == 0 && saw_image && end == bytes.len() {
                Ok(())
            } else {
                Err(bad())
            };
        }
        offset = end;
    }
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn list_presets(paths: &ManagedPaths) -> Result<Vec<SkinPreset>> {
    let _guard = lock().lock().map_err(|_| storage_error())?;
    Ok(read_document(paths)?.presets)
}
pub fn import_preset(
    paths: &ManagedPaths,
    name: &str,
    model: SkinModel,
    bytes: &[u8],
) -> Result<SkinPreset> {
    validate_png(bytes)?;
    let name = name.trim();
    if name.is_empty() || name.len() > 80 || name.chars().any(char::is_control) {
        return Err(error(
            "skin_preset_name_invalid",
            "Give the saved skin a short name.",
        ));
    }
    let _guard = lock().lock().map_err(|_| storage_error())?;
    let mut document = read_document(paths)?;
    if document.presets.len() >= MAX_PRESETS {
        return Err(error(
            "skin_preset_limit",
            "The 64 saved skin limit has been reached.",
        ));
    }
    let preset = SkinPreset {
        id: Uuid::new_v4().to_string(),
        name: name.to_owned(),
        model,
        imported_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        sha256: digest(bytes),
    };
    let path = png_path(paths, &preset.id)?;
    if path.exists() {
        return Err(storage_error());
    }
    write_atomic(&path, bytes)?;
    document.presets.push(preset.clone());
    if let Err(e) = save_document(paths, &document) {
        let _ = std::fs::remove_file(&path);
        return Err(e);
    }
    Ok(preset)
}
pub fn remove_preset(paths: &ManagedPaths, id: &str) -> Result<()> {
    let _guard = lock().lock().map_err(|_| storage_error())?;
    let mut document = read_document(paths)?;
    let index = document
        .presets
        .iter()
        .position(|p| p.id == id)
        .ok_or(error(
            "skin_preset_missing",
            "That saved skin preset is unavailable.",
        ))?;
    let path = png_path(paths, id)?;
    let bytes = std::fs::read(&path).map_err(|_| storage_error())?;
    if digest(&bytes) != document.presets[index].sha256 {
        return Err(error(
            "skin_preset_damaged",
            "The saved skin image has changed; it was left untouched.",
        ));
    }
    document.presets.remove(index);
    save_document(paths, &document)?;
    // Only the exact generated managed file is removed. A failed removal leaves an orphan, not a remote mutation.
    std::fs::remove_file(path).map_err(|_| storage_error())
}
pub fn preset_for_upload(paths: &ManagedPaths, id: &str) -> Result<(SkinPreset, Vec<u8>)> {
    let _guard = lock().lock().map_err(|_| storage_error())?;
    let preset = read_document(paths)?
        .presets
        .into_iter()
        .find(|p| p.id == id)
        .ok_or(error(
            "skin_preset_missing",
            "That saved skin preset is unavailable.",
        ))?;
    let bytes = std::fs::read(png_path(paths, id)?).map_err(|_| {
        error(
            "skin_preset_missing",
            "That saved skin image is unavailable.",
        )
    })?;
    validate_png(&bytes)?;
    if digest(&bytes) != preset.sha256 {
        return Err(error(
            "skin_preset_damaged",
            "The saved skin image has changed; it cannot be applied.",
        ));
    }
    Ok((preset, bytes))
}

#[derive(Deserialize)]
struct WireProfile {
    id: String,
    #[serde(default)]
    skins: Vec<WireSkin>,
    #[serde(default)]
    capes: Vec<WireCape>,
}
#[derive(Deserialize)]
struct WireSkin {
    state: String,
    variant: String,
}
#[derive(Deserialize)]
struct WireCape {
    id: String,
    state: String,
    alias: Option<String>,
    url: Option<String>,
}
fn normalize(profile: WireProfile, account_id: &str) -> Result<CosmeticsState> {
    if profile.id.to_ascii_lowercase() != account_id {
        return Err(error(
            "cosmetics_wrong_account",
            "Minecraft returned a different account profile.",
        ));
    }
    let current = profile.skins.iter().find(|skin| skin.state == "ACTIVE");
    let current_skin_model = match current.map(|skin| skin.variant.as_str()) {
        Some("CLASSIC") => Some(SkinModel::Classic),
        Some("SLIM") => Some(SkinModel::Slim),
        _ => None,
    };
    let mut ids = HashSet::new();
    let capes = profile
        .capes
        .into_iter()
        .filter_map(|cape| {
            if cape.id.is_empty()
                || cape.id.len() > 80
                || !cape
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || !ids.insert(cape.id.clone())
            {
                return None;
            }
            let name = cape
                .alias
                .filter(|s| {
                    !s.trim().is_empty() && s.len() <= 80 && !s.chars().any(char::is_control)
                })
                .unwrap_or_else(|| "Minecraft cape".into());
            Some(Cape {
                id: cape.id,
                name,
                selected: cape.state == "ACTIVE",
                preview: None,
            })
        })
        .collect();
    Ok(CosmeticsState {
        account_id: account_id.into(),
        current_skin_model,
        has_current_skin: current.is_some(),
        capes,
    })
}
fn client() -> reqwest::Client {
    crate::downloads::ensure_rustls_crypto_provider();
    reqwest::Client::builder()
        .user_agent(crate::downloads::user_agent())
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("cosmetic client")
}
async fn request(request: reqwest::RequestBuilder) -> Result<Vec<u8>> {
    let mut response = request.send().await.map_err(|_| {
        error(
            "cosmetics_service_unavailable",
            "Minecraft Services could not be reached.",
        )
    })?;
    let status = response.status().as_u16();
    if status == 429 {
        return Err(error(
            "cosmetics_rate_limited",
            "Minecraft Services is rate limiting cosmetic changes. Try again later.",
        ));
    }
    if status == 401 {
        return Err(error(
            "cosmetics_authentication_required",
            "The Minecraft session expired. Reauthenticate this account.",
        ));
    }
    if status == 403 {
        return Err(error(
            "cosmetics_service_rejected",
            "Minecraft Services rejected this cosmetic change.",
        ));
    }
    if status == 400 {
        return Err(error(
            "cosmetics_bad_request",
            "Minecraft Services rejected the cosmetic request.",
        ));
    }
    if status == 404 {
        return Err(error(
            "cosmetics_profile_unavailable",
            "The Minecraft profile is unavailable.",
        ));
    }
    if status >= 500 {
        return Err(error(
            "cosmetics_service_unavailable",
            "Minecraft Services is temporarily unavailable.",
        ));
    }
    if !(200..300).contains(&status) {
        return Err(error(
            "cosmetics_service_rejected",
            "Minecraft Services rejected this cosmetic request.",
        ));
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE as u64)
    {
        return Err(error(
            "cosmetics_response_invalid",
            "Minecraft Services returned an oversized response.",
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        error(
            "cosmetics_service_unavailable",
            "Minecraft Services response was interrupted.",
        )
    })? {
        if body.len() + chunk.len() > MAX_RESPONSE {
            return Err(error(
                "cosmetics_response_invalid",
                "Minecraft Services returned an oversized response.",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
pub async fn fetch(account_id: &str, token: &SecretString) -> Result<CosmeticsState> {
    fetch_from(PROFILE_URL, account_id, token).await
}
async fn fetch_from(url: &str, account_id: &str, token: &SecretString) -> Result<CosmeticsState> {
    AccountId::validate(account_id).map_err(|_| {
        error(
            "cosmetics_account_invalid",
            "Choose a valid Minecraft account.",
        )
    })?;
    let body = request(client().get(url).bearer_auth(token.expose())).await?;
    let profile: WireProfile = serde_json::from_slice(&body).map_err(|_| {
        error(
            "cosmetics_response_invalid",
            "Minecraft Services returned an invalid profile.",
        )
    })?;
    let preview_sources: Vec<_> = profile
        .capes
        .iter()
        .take(16)
        .filter_map(|cape| {
            Some((
                cape.id.clone(),
                crate::auth::avatar::validated_url(cape.url.as_deref()?)?,
            ))
        })
        .collect();
    let mut state = normalize(profile, account_id)?;
    let previews = join_all(preview_sources.into_iter().map(|(id, url)| async move {
        (
            id,
            tokio::time::timeout(Duration::from_secs(5), fetch_cape_preview(url))
                .await
                .ok()
                .flatten(),
        )
    }))
    .await;
    for (id, preview) in previews {
        if let Some(cape) = state.capes.iter_mut().find(|cape| cape.id == id) {
            cape.preview = preview;
        }
    }
    Ok(state)
}
async fn fetch_cape_preview(url: url::Url) -> Option<CapePreview> {
    let mut response = client().get(url).send().await.ok()?;
    if response.status() != reqwest::StatusCode::OK
        || response
            .content_length()
            .is_some_and(|n| n > MAX_PNG as u64)
    {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() + chunk.len() > MAX_PNG {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    decode_cape_preview(&bytes)
}
fn decode_cape_preview(bytes: &[u8]) -> Option<CapePreview> {
    if bytes.len() > MAX_PNG
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || validate_png_chunks(bytes).is_err()
    {
        return None;
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: 1024 * 1024 });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = (reader.info().width, reader.info().height);
    if !matches!((width, height), (64, 32) | (64, 64) | (128, 64))
        || reader.info().animation_control.is_some()
    {
        return None;
    }
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut pixels).ok()?;
    let channels = match frame.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return None,
    };
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for pixel in pixels[..frame.buffer_size()].chunks_exact(channels) {
        rgba.extend_from_slice(&pixel[..3]);
        rgba.push(if channels == 4 { pixel[3] } else { 255 });
    }
    if rgba.len() != (width * height * 4) as usize {
        return None;
    }
    Some(CapePreview {
        width,
        height,
        rgba,
    })
}
pub async fn apply_skin(
    account_id: &str,
    token: &SecretString,
    bytes: &[u8],
    model: SkinModel,
) -> Result<CosmeticsState> {
    apply_skin_to(PROFILE_URL, SKIN_URL, account_id, token, bytes, model).await
}
async fn apply_skin_to(
    profile_url: &str,
    skin_url: &str,
    account_id: &str,
    token: &SecretString,
    bytes: &[u8],
    model: SkinModel,
) -> Result<CosmeticsState> {
    validate_png(bytes)?;
    let boundary = format!("aurora-{}", Uuid::new_v4());
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n", model.service()).into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    request(
        client()
            .post(skin_url)
            .bearer_auth(token.expose())
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body),
    )
    .await
    .map_err(|failure| {
        if failure.code == "cosmetics_bad_request" {
            error(
                "skin_service_rejected",
                "Minecraft Services rejected this skin image or model.",
            )
        } else {
            failure
        }
    })?;
    let state = fetch_from(profile_url, account_id, token).await?;
    if state.current_skin_model != Some(model) {
        return Err(error(
            "cosmetics_refresh_failed",
            "The skin request succeeded, but the updated profile could not be confirmed yet.",
        ));
    }
    Ok(state)
}
pub async fn select_cape(
    account_id: &str,
    token: &SecretString,
    cape_id: &str,
) -> Result<CosmeticsState> {
    select_cape_at(PROFILE_URL, CAPE_URL, account_id, token, cape_id).await
}
async fn select_cape_at(
    profile_url: &str,
    cape_url: &str,
    account_id: &str,
    token: &SecretString,
    cape_id: &str,
) -> Result<CosmeticsState> {
    let before = fetch_from(profile_url, account_id, token).await?;
    if !before.capes.iter().any(|cape| cape.id == cape_id) {
        return Err(error(
            "cape_not_owned",
            "This cape is no longer available on the selected account.",
        ));
    }
    if before
        .capes
        .iter()
        .any(|cape| cape.id == cape_id && cape.selected)
    {
        return Ok(before);
    }
    let body = serde_json::json!({"capeId": cape_id});
    request(
        client()
            .put(cape_url)
            .bearer_auth(token.expose())
            .header("Content-Type", "application/json")
            .body(body.to_string()),
    )
    .await
    .map_err(|failure| {
        if failure.code == "cosmetics_bad_request" {
            error(
                "cape_unavailable",
                "The cape is no longer available to this account.",
            )
        } else {
            failure
        }
    })?;
    let after = fetch_from(profile_url, account_id, token).await?;
    if !after
        .capes
        .iter()
        .any(|cape| cape.id == cape_id && cape.selected)
    {
        return Err(error(
            "cosmetics_refresh_failed",
            "The cape selection could not be confirmed yet.",
        ));
    }
    Ok(after)
}
pub async fn disable_cape(account_id: &str, token: &SecretString) -> Result<CosmeticsState> {
    disable_cape_at(PROFILE_URL, CAPE_URL, account_id, token).await
}
async fn disable_cape_at(
    profile_url: &str,
    cape_url: &str,
    account_id: &str,
    token: &SecretString,
) -> Result<CosmeticsState> {
    request(client().delete(cape_url).bearer_auth(token.expose())).await?;
    let state = fetch_from(profile_url, account_id, token).await?;
    if state.capes.iter().any(|cape| cape.selected) {
        return Err(error(
            "cosmetics_refresh_failed",
            "The cape change could not be confirmed yet.",
        ));
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestResponse, TestServer};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    fn fixture(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let pixels = vec![128u8; (width * height * 4) as usize];
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        bytes
    }
    fn paths() -> ManagedPaths {
        ManagedPaths::from_app_local_data_dir(
            std::env::temp_dir().join(format!("aurora-cosmetics-test-{}", Uuid::new_v4())),
        )
        .unwrap()
    }
    #[test]
    fn png_validation_is_bounded_and_rejects_malformed_images() {
        assert!(validate_png(&fixture(64, 64)).is_ok());
        assert!(validate_png(&fixture(64, 32)).is_ok());
        assert_eq!(
            validate_png(&fixture(128, 128)).unwrap_err().code,
            "unsupported_skin_image"
        );
        assert_eq!(
            validate_png(b"not a PNG").unwrap_err().code,
            "unsupported_skin_image"
        );
        assert_eq!(
            validate_png(&vec![0; MAX_PNG + 1]).unwrap_err().code,
            "skin_file_too_large"
        );
        let mut broken = fixture(64, 64);
        broken.truncate(50);
        assert_eq!(
            validate_png(&broken).unwrap_err().code,
            "skin_png_malformed"
        );
        let mut missing_end = fixture(64, 64);
        missing_end.truncate(missing_end.len() - 12);
        assert_eq!(
            validate_png(&missing_end).unwrap_err().code,
            "skin_png_malformed"
        );
    }
    #[test]
    fn presets_use_generated_paths_and_preserve_duplicate_display_names() {
        let paths = paths();
        let bytes = fixture(64, 64);
        let first = import_preset(&paths, "Same name", SkinModel::Classic, &bytes).unwrap();
        let second = import_preset(&paths, "Same name", SkinModel::Slim, &bytes).unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(list_presets(&paths).unwrap().len(), 2);
        assert_eq!(preset_for_upload(&paths, &first.id).unwrap().1, bytes);
        assert_eq!(
            preset_for_upload(&paths, "../escape").unwrap_err().code,
            "skin_preset_missing"
        );
        remove_preset(&paths, &first.id).unwrap();
        assert_eq!(list_presets(&paths).unwrap().len(), 1);
        assert_eq!(list_presets(&paths).unwrap()[0].id, second.id);
        assert!(!png_path(&paths, &first.id).unwrap().exists());
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }
    #[test]
    fn damaged_or_future_metadata_is_never_overwritten() {
        let paths = paths();
        std::fs::create_dir_all(root(&paths)).unwrap();
        let bad = b"{broken";
        std::fs::write(metadata_path(&paths), bad).unwrap();
        assert_eq!(
            import_preset(&paths, "test", SkinModel::Classic, &fixture(64, 64))
                .unwrap_err()
                .code,
            "skin_presets_malformed"
        );
        assert_eq!(std::fs::read(metadata_path(&paths)).unwrap(), bad);
        let future = br#"{"schemaVersion":2,"presets":[]}"#;
        std::fs::write(metadata_path(&paths), future).unwrap();
        assert_eq!(
            list_presets(&paths).unwrap_err().code,
            "skin_presets_schema_unsupported"
        );
        assert_eq!(std::fs::read(metadata_path(&paths)).unwrap(), future);
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }
    #[test]
    fn profile_mapping_is_account_bound_and_ownership_scoped() {
        let id = "986dec87b7ec47ff89ff033fdb95c4b5";
        let profile: WireProfile = serde_json::from_value(serde_json::json!({
            "id": id, "skins": [{"state":"ACTIVE","variant":"SLIM"}],
            "capes": [{"id":"owned-1","state":"ACTIVE","alias":"Founder"}, {"id":"owned-2","state":"INACTIVE","alias":"Other"}]
        })).unwrap();
        let state = normalize(profile, id).unwrap();
        assert_eq!(state.current_skin_model, Some(SkinModel::Slim));
        assert_eq!(state.capes.len(), 2);
        assert!(state.capes[0].selected);
        let other = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let profile: WireProfile = serde_json::from_value(serde_json::json!({"id":id})).unwrap();
        assert_eq!(
            normalize(profile, other).unwrap_err().code,
            "cosmetics_wrong_account"
        );
    }
    #[test]
    fn cape_preview_is_native_decoded_and_bounded() {
        let preview = decode_cape_preview(&fixture(64, 32)).unwrap();
        assert_eq!(preview.rgba.len(), 64 * 32 * 4);
        assert!(decode_cape_preview(&fixture(128, 128)).is_none());
        assert!(decode_cape_preview(b"not png").is_none());
    }
    const ACCOUNT: &str = "986dec87b7ec47ff89ff033fdb95c4b5";
    fn profile(active: bool, model: &str) -> Vec<u8> {
        serde_json::json!({"id": ACCOUNT, "skins":[{"state":"ACTIVE","variant":model}], "capes":[{"id":"owned-cape","state":if active {"ACTIVE"} else {"INACTIVE"},"alias":"Founder"}]}).to_string().into_bytes()
    }
    #[tokio::test]
    async fn skin_upload_is_multipart_and_refreshes_profile() {
        let uploaded = Arc::new(AtomicBool::new(false));
        let sent = Arc::clone(&uploaded);
        let server = TestServer::spawn(Arc::new(move |request| {
            assert!(request.has_bearer_authorization);
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/minecraft/profile/skins") => {
                    assert!(
                        request
                            .body
                            .windows(b"name=\"variant\"\r\n\r\nslim".len())
                            .any(|w| w == b"name=\"variant\"\r\n\r\nslim")
                    );
                    assert!(request.body.windows(8).any(|w| w == b"\x89PNG\r\n\x1a\n"));
                    sent.store(true, Ordering::SeqCst);
                    TestResponse::ok(&[])
                }
                ("GET", "/minecraft/profile") => TestResponse::ok(&profile(
                    false,
                    if sent.load(Ordering::SeqCst) {
                        "SLIM"
                    } else {
                        "CLASSIC"
                    },
                )),
                _ => TestResponse::status(404),
            }
        }));
        let token = SecretString::new("FIXTURE-SECRET-TOKEN");
        let state = apply_skin_to(
            &format!("{}/minecraft/profile", server.base_url()),
            &format!("{}/minecraft/profile/skins", server.base_url()),
            ACCOUNT,
            &token,
            &fixture(64, 64),
            SkinModel::Slim,
        )
        .await
        .unwrap();
        assert!(uploaded.load(Ordering::SeqCst));
        assert_eq!(state.current_skin_model, Some(SkinModel::Slim));
    }
    #[tokio::test]
    async fn cape_selection_checks_owned_set_and_disable_refreshes() {
        let active = Arc::new(AtomicBool::new(false));
        let changed = Arc::clone(&active);
        let server = TestServer::spawn(Arc::new(move |request| {
            assert!(request.has_bearer_authorization);
            match (request.method.as_str(), request.path.as_str()) {
                ("GET", "/minecraft/profile") => {
                    TestResponse::ok(&profile(changed.load(Ordering::SeqCst), "CLASSIC"))
                }
                ("PUT", "/minecraft/profile/capes/active") => {
                    assert_eq!(
                        serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()["capeId"],
                        "owned-cape"
                    );
                    changed.store(true, Ordering::SeqCst);
                    TestResponse::ok(&[])
                }
                ("DELETE", "/minecraft/profile/capes/active") => {
                    changed.store(false, Ordering::SeqCst);
                    TestResponse::ok(&[])
                }
                _ => TestResponse::status(404),
            }
        }));
        let profile_url = format!("{}/minecraft/profile", server.base_url());
        let cape_url = format!("{}/minecraft/profile/capes/active", server.base_url());
        let token = SecretString::new("FIXTURE-SECRET-TOKEN");
        assert_eq!(
            select_cape_at(&profile_url, &cape_url, ACCOUNT, &token, "unowned")
                .await
                .unwrap_err()
                .code,
            "cape_not_owned"
        );
        assert_eq!(server.request_count(), 1);
        assert!(
            select_cape_at(&profile_url, &cape_url, ACCOUNT, &token, "owned-cape")
                .await
                .unwrap()
                .capes[0]
                .selected
        );
        assert!(
            !disable_cape_at(&profile_url, &cape_url, ACCOUNT, &token)
                .await
                .unwrap()
                .capes[0]
                .selected
        );
    }
    #[tokio::test]
    async fn remote_failures_are_structured_and_redacted() {
        let server = TestServer::spawn(Arc::new(|request| {
            assert!(request.has_bearer_authorization);
            TestResponse::ok(b"FIXTURE-SECRET-TOKEN").with_status(429)
        }));
        let token = SecretString::new("FIXTURE-SECRET-TOKEN");
        let failure = fetch_from(
            &format!("{}/minecraft/profile", server.base_url()),
            ACCOUNT,
            &token,
        )
        .await
        .unwrap_err();
        assert_eq!(failure.code, "cosmetics_rate_limited");
        assert!(!failure.message.contains("FIXTURE-SECRET-TOKEN"));
    }
    #[tokio::test]
    async fn rejected_skin_and_unavailable_service_have_specific_errors() {
        let rejected = TestServer::spawn(Arc::new(|request| {
            assert_eq!(request.method, "POST");
            TestResponse::status(400)
        }));
        let token = SecretString::new("FIXTURE-SECRET-TOKEN");
        let failure = apply_skin_to(
            &format!("{}/minecraft/profile", rejected.base_url()),
            &format!("{}/minecraft/profile/skins", rejected.base_url()),
            ACCOUNT,
            &token,
            &fixture(64, 64),
            SkinModel::Classic,
        )
        .await
        .unwrap_err();
        assert_eq!(failure.code, "skin_service_rejected");
        assert_eq!(rejected.request_count(), 1);
        let unavailable = TestServer::spawn(Arc::new(|_| TestResponse::status(503)));
        let failure = fetch_from(
            &format!("{}/minecraft/profile", unavailable.base_url()),
            ACCOUNT,
            &token,
        )
        .await
        .unwrap_err();
        assert_eq!(failure.code, "cosmetics_service_unavailable");
    }
    #[tokio::test]
    async fn cape_already_selected_skips_mutation_and_stale_ownership_is_rejected() {
        let server = TestServer::spawn(Arc::new(|request| match request.method.as_str() {
            "GET" => TestResponse::ok(&profile(true, "CLASSIC")),
            _ => panic!("already selected cape must not issue a mutation"),
        }));
        let token = SecretString::new("FIXTURE-SECRET-TOKEN");
        let profile_url = format!("{}/minecraft/profile", server.base_url());
        let cape_url = format!("{}/minecraft/profile/capes/active", server.base_url());
        assert!(
            select_cape_at(&profile_url, &cape_url, ACCOUNT, &token, "owned-cape")
                .await
                .unwrap()
                .capes[0]
                .selected
        );
        assert_eq!(server.request_count(), 1);
        let stale = TestServer::spawn(Arc::new(|request| match request.method.as_str() {
            "GET" => TestResponse::ok(&profile(false, "CLASSIC")),
            "PUT" => TestResponse::status(400),
            _ => TestResponse::status(404),
        }));
        let failure = select_cape_at(
            &format!("{}/minecraft/profile", stale.base_url()),
            &format!("{}/minecraft/profile/capes/active", stale.base_url()),
            ACCOUNT,
            &token,
            "owned-cape",
        )
        .await
        .unwrap_err();
        assert_eq!(failure.code, "cape_unavailable");
    }
}
