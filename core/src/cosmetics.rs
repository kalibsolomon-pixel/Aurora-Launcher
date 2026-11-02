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
const MAX_PRESETS: usize = 256;
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
    #[serde(default)]
    pub favorite: bool,
}

/// Import result: an exact (bytes, model) duplicate reports the existing
/// library entry instead of creating another record or managed file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub preset: SkinPreset,
    pub duplicate: bool,
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
            schema_version: 2,
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
    pub current_skin: Option<crate::auth::avatar::HeadAvatar>,
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
    if document.schema_version != 2 {
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
            || preset.name.chars().any(char::is_control)
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
    let bytes = match read_managed(paths, &metadata_path(paths), 256 * 1024) {
        Ok(bytes) => bytes,
        Err(e) if e.code == "skin_file_missing" => return Ok(PresetDocument::default()),
        Err(_) => return Err(storage_error()),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| malformed())?;
    let version = value["schemaVersion"].as_u64().ok_or_else(malformed)?;
    if !matches!(version, 1 | 2) {
        return Err(error(
            "skin_presets_schema_unsupported",
            "Skin preset metadata uses a newer schema; it was left unchanged.",
        ));
    }
    // Only the exact old shape migrates; reads preserve the original bytes.
    let rows = value["presets"].as_array().ok_or_else(malformed)?;
    if rows.iter().any(|row| {
        if version == 1 {
            row.get("favorite").is_some()
        } else {
            !row["favorite"].is_boolean()
        }
    }) {
        return Err(malformed());
    }
    let mut document: PresetDocument = serde_json::from_value(value).map_err(|_| malformed())?;
    document.schema_version = 2;
    validate_document(&document)?;
    Ok(document)
}
/// The store's fixed path must never be redirected through links or junctions.
/// Checks are process-local; external concurrent filesystem writers are out of scope.
fn contained(paths: &ManagedPaths, path: &Path) -> Result<()> {
    if !path.starts_with(paths.data_root()) {
        return Err(storage_error());
    }
    let mut current = Some(path);
    while let Some(candidate) = current {
        if let Ok(meta) = std::fs::symlink_metadata(candidate) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 {
                    return Err(storage_error());
                }
            }
            if meta.file_type().is_symlink() {
                return Err(storage_error());
            }
        }
        if candidate == paths.data_root() {
            break;
        }
        current = candidate.parent();
    }
    Ok(())
}
fn read_managed(paths: &ManagedPaths, path: &Path, limit: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    contained(paths, path)?;
    let file = std::fs::File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            error("skin_file_missing", "The saved skin file is missing.")
        } else {
            storage_error()
        }
    })?;
    if !file.metadata().map_err(|_| storage_error())?.is_file() {
        return Err(storage_error());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| storage_error())?;
    if bytes.len() as u64 > limit {
        return Err(malformed());
    }
    Ok(bytes)
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
    contained(paths, &metadata_path(paths))?;
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
) -> Result<ImportOutcome> {
    validate_png(bytes)?;
    validate_model(bytes, model)?;
    let name = name.trim();
    if name.is_empty() || name.len() > 80 || name.chars().any(char::is_control) {
        return Err(error(
            "skin_preset_name_invalid",
            "Give the saved skin a short name.",
        ));
    }
    let _guard = lock().lock().map_err(|_| storage_error())?;
    let mut document = read_document(paths)?;
    let content_digest = digest(bytes);
    // Content-hash deduplication: identical bytes under the same model keep
    // the existing entry. The same bytes under the other model stay an
    // independent library entry with its own model metadata.
    if let Some(existing) = document
        .presets
        .iter()
        .find(|preset| preset.sha256 == content_digest && preset.model == model)
    {
        // Revalidate a duplicate too; an absent/damaged object is never a success.
        let bytes = read_managed(paths, &png_path(paths, &existing.id)?, MAX_PNG as u64)?;
        validate_png(&bytes)?;
        if digest(&bytes) != existing.sha256 {
            return Err(error(
                "skin_preset_damaged",
                "The saved skin image has changed; it was left untouched.",
            ));
        }
        return Ok(ImportOutcome {
            preset: existing.clone(),
            duplicate: true,
        });
    }
    if document.presets.len() >= MAX_PRESETS {
        return Err(error(
            "skin_preset_limit",
            "The 256 saved skin limit has been reached.",
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
        sha256: content_digest,
        favorite: false,
    };
    let path = png_path(paths, &preset.id)?;
    contained(paths, &path)?;
    if path.exists() {
        return Err(storage_error());
    }
    // Different model records can share immutable local bytes without another copy.
    // Each read still rehashes; a modified shared object cannot authorize upload.
    if let Some(other) = document.presets.iter().find(|p| p.sha256 == preset.sha256) {
        let source = png_path(paths, &other.id)?;
        let stored = read_managed(paths, &source, MAX_PNG as u64)?;
        if digest(&stored) != other.sha256 {
            return Err(error(
                "skin_preset_damaged",
                "The saved skin image has changed; it was left untouched.",
            ));
        }
        std::fs::hard_link(source, &path).map_err(|_| storage_error())?;
    } else {
        write_atomic(&path, bytes)?;
    }
    document.presets.push(preset.clone());
    if let Err(e) = save_document(paths, &document) {
        let _ = std::fs::remove_file(&path);
        return Err(e);
    }
    Ok(ImportOutcome {
        preset,
        duplicate: false,
    })
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
    let bytes = read_managed(paths, &path, MAX_PNG as u64)?;
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
    let bytes = read_managed(paths, &png_path(paths, id)?, MAX_PNG as u64).map_err(|_| {
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

/// Metadata-only update: rename and/or the Classic/Slim model. The managed
/// image bytes and content hash never change here.
pub fn update_preset(
    paths: &ManagedPaths,
    id: &str,
    name: Option<&str>,
    model: Option<SkinModel>,
) -> Result<Vec<SkinPreset>> {
    let _guard = lock().lock().map_err(|_| storage_error())?;
    let mut document = read_document(paths)?;
    let preset = document
        .presets
        .iter_mut()
        .find(|preset| preset.id == id)
        .ok_or(error(
            "skin_preset_missing",
            "That saved skin preset is unavailable.",
        ))?;
    if let Some(name) = name {
        let name = name.trim();
        if name.is_empty() || name.len() > 80 || name.chars().any(char::is_control) {
            return Err(error(
                "skin_preset_name_invalid",
                "Give the saved skin a short name.",
            ));
        }
        preset.name = name.to_owned();
    }
    if let Some(model) = model {
        let bytes = read_managed(paths, &png_path(paths, id)?, MAX_PNG as u64)?;
        validate_png(&bytes)?;
        if digest(&bytes) != preset.sha256 {
            return Err(error(
                "skin_preset_damaged",
                "The saved skin image has changed; it was left untouched.",
            ));
        }
        validate_model(&bytes, model)?;
        preset.model = model;
    }
    save_document(paths, &document)?;
    Ok(document.presets)
}

fn validate_model(bytes: &[u8], model: SkinModel) -> Result<()> {
    if model == SkinModel::Slim && bytes.get(20..24) == Some(32u32.to_be_bytes().as_slice()) {
        return Err(error(
            "skin_model_invalid",
            "Legacy 64×32 skins use the Classic model. Choose Classic to import this skin.",
        ));
    }
    Ok(())
}

/// The saved skin's 8×8 head with hat composited — a cheap, static thumbnail
/// derived from verified managed bytes. Read-only; no network, no animation.
pub fn preset_thumbnail(paths: &ManagedPaths, id: &str) -> Result<Option<[u8; 256]>> {
    let (_, bytes) = preset_for_upload(paths, id)?;
    Ok(decode_head_thumbnail(&bytes))
}

pub fn set_favorite(paths: &ManagedPaths, id: &str, favorite: bool) -> Result<Vec<SkinPreset>> {
    let _guard = lock().lock().map_err(|_| storage_error())?;
    let mut document = read_document(paths)?;
    let preset = document
        .presets
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or(error(
            "skin_preset_missing",
            "That saved skin preset is unavailable.",
        ))?;
    preset.favorite = favorite;
    save_document(paths, &document)?;
    Ok(document.presets)
}

/// Full pixels use the existing account decoder and legacy normalization.
pub fn preset_preview(paths: &ManagedPaths, id: &str) -> Result<crate::auth::avatar::HeadAvatar> {
    let (preset, bytes) = preset_for_upload(paths, id)?;
    crate::auth::avatar::decode(&bytes, preset.model.service()).ok_or(error(
        "skin_png_malformed",
        "The saved skin could not be decoded.",
    ))
}

fn decode_head_thumbnail(bytes: &[u8]) -> Option<[u8; 256]> {
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
    let buffer = &pixels[..info.buffer_size()];
    let alpha = |x: usize, y: usize| {
        if channels == 4 {
            buffer[(y * 64 + x) * channels + 3] as u32
        } else {
            255
        }
    };
    // Same legacy rule as account avatars: an opaque legacy hat background
    // means no hat at all.
    let clear_hat = info.height == 32 && (0..32).all(|y| (32..64).all(|x| alpha(x, y) == 255));
    Some(crate::auth::avatar::composite_head(
        buffer, channels, clear_hat,
    ))
}

/// Bounded download of the account's active skin PNG from the validated
/// official texture locator. Feeds the same import validation as a file.
pub async fn download_current_skin(url: &url::Url) -> Result<Vec<u8>> {
    let mut response = client().get(url.clone()).send().await.map_err(|_| {
        error(
            "cosmetics_service_unavailable",
            "Minecraft skin textures could not be reached.",
        )
    })?;
    if response.status() != reqwest::StatusCode::OK
        || response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or("").trim() != "image/png")
            .unwrap_or(true)
        || response
            .content_length()
            .is_some_and(|length| length > MAX_PNG as u64)
    {
        return Err(error(
            "cosmetics_response_invalid",
            "The Minecraft skin texture could not be read.",
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        error(
            "cosmetics_service_unavailable",
            "Minecraft skin textures could not be reached.",
        )
    })? {
        if body.len() + chunk.len() > MAX_PNG {
            return Err(error(
                "cosmetics_response_invalid",
                "The Minecraft skin texture is oversized.",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Deserialize)]
struct WireProfile {
    id: String,
    #[serde(default)]
    skins: Vec<WireSkin>,
    #[serde(default)]
    capes: Vec<WireCape>,
}
#[derive(Deserialize, Serialize)]
struct WireSkin {
    id: Option<String>,
    state: String,
    variant: String,
    url: Option<String>,
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
        current_skin: None,
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
    Ok(fetch_profile_from(url, account_id, token).await?.0)
}
fn active_skin_id(profile: &WireProfile) -> Option<String> {
    let id = profile
        .skins
        .iter()
        .find(|skin| skin.state == "ACTIVE")?
        .id
        .as_ref()?;
    (!id.is_empty() && id.len() <= 80 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .then(|| id.clone())
}
async fn fetch_profile_from(
    url: &str,
    account_id: &str,
    token: &SecretString,
) -> Result<(CosmeticsState, Option<String>)> {
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
    let active_id = active_skin_id(&profile);
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
    let current_texture = crate::auth::avatar::SkinTexture::from_profile_skins(
        &serde_json::to_value(&profile.skins).unwrap_or_default(),
    );
    let mut state = normalize(profile, account_id)?;
    let skin_future = async {
        if let Some(texture) = current_texture {
            tokio::time::timeout(
                Duration::from_secs(5),
                crate::auth::avatar::head(&texture, true),
            )
            .await
            .ok()
            .flatten()
        } else {
            None
        }
    };
    let previews_future = join_all(preview_sources.into_iter().map(|(id, url)| async move {
        (
            id,
            tokio::time::timeout(Duration::from_secs(5), fetch_cape_preview(url))
                .await
                .ok()
                .flatten(),
        )
    }));
    let (previews, skin) = tokio::join!(previews_future, skin_future);
    state.current_skin = skin;
    for (id, preview) in previews {
        if let Some(cape) = state.capes.iter_mut().find(|cape| cape.id == id) {
            cape.preview = preview;
        }
    }
    Ok((state, active_id))
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
    let response = request(
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
    // Match the identity returned by the upload with a separate authoritative
    // GET. Matching only the model could confirm an unchanged same-model skin.
    let uploaded: Option<WireProfile> = serde_json::from_slice(&response).ok();
    let expected_id = uploaded.as_ref().and_then(active_skin_id);
    let upload_matches = uploaded.is_some_and(|profile| {
        normalize(profile, account_id).is_ok_and(|state| state.current_skin_model == Some(model))
    });
    let (state, current_id) = fetch_profile_from(profile_url, account_id, token).await?;
    if !upload_matches
        || expected_id.is_none()
        || expected_id != current_id
        || state.current_skin_model != Some(model)
    {
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
    fn schema_one_migrates_without_read_time_writes_and_favorites_survive_reopen() {
        let paths = paths();
        let first = import_preset(&paths, "Legacy", SkinModel::Classic, &fixture(64, 64)).unwrap();
        let mut legacy = serde_json::to_value(read_document(&paths).unwrap()).unwrap();
        legacy["schemaVersion"] = 1.into();
        legacy["presets"][0]
            .as_object_mut()
            .unwrap()
            .remove("favorite");
        let bytes = serde_json::to_vec(&legacy).unwrap();
        std::fs::write(metadata_path(&paths), &bytes).unwrap();
        assert!(!list_presets(&paths).unwrap()[0].favorite);
        assert_eq!(std::fs::read(metadata_path(&paths)).unwrap(), bytes);
        set_favorite(&paths, &first.preset.id, true).unwrap();
        let reopened = ManagedPaths::from_app_local_data_dir(paths.data_root().to_owned()).unwrap();
        let saved = list_presets(&reopened).unwrap();
        assert!(saved[0].favorite);
        assert_eq!(saved[0].id, first.preset.id);
        assert_eq!(
            preset_for_upload(&reopened, &saved[0].id).unwrap().1,
            fixture(64, 64)
        );
        let mut mixed = legacy.clone();
        mixed["presets"][0]["favorite"] = true.into();
        let bad = serde_json::to_vec(&mixed).unwrap();
        std::fs::write(metadata_path(&paths), &bad).unwrap();
        assert!(set_favorite(&paths, &first.preset.id, false).is_err());
        assert_eq!(std::fs::read(metadata_path(&paths)).unwrap(), bad);
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }
    #[test]
    fn library_exceeds_old_capacity_and_duplicate_reuse_requires_valid_bytes() {
        let paths = paths();
        for i in 0..80 {
            let mut pixels = vec![128; 64 * 64 * 4];
            pixels[0] = i;
            let mut bytes = Vec::new();
            let mut encoder = png::Encoder::new(&mut bytes, 64, 64);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
            import_preset(&paths, &format!("Skin {i}"), SkinModel::Classic, &bytes).unwrap();
        }
        assert_eq!(list_presets(&paths).unwrap().len(), 80);
        let saved = import_preset(&paths, "Legacy", SkinModel::Classic, &fixture(64, 32)).unwrap();
        assert_eq!(
            preset_preview(&paths, &saved.preset.id)
                .unwrap()
                .skin_height,
            32
        );
        assert_eq!(
            import_preset(&paths, "Wrong model", SkinModel::Slim, &fixture(64, 32))
                .unwrap_err()
                .code,
            "skin_model_invalid"
        );
        std::fs::write(png_path(&paths, &saved.preset.id).unwrap(), b"damaged").unwrap();
        assert!(import_preset(&paths, "Duplicate", SkinModel::Classic, &fixture(64, 32)).is_err());
        assert!(remove_preset(&paths, &saved.preset.id).is_err());
        assert_eq!(list_presets(&paths).unwrap().len(), 81);
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }
    #[test]
    fn full_preview_preserves_alpha_and_refuses_missing_or_changed_assets() {
        let paths = paths();
        let skin = import_preset(&paths, "Slim", SkinModel::Slim, &fixture(64, 64)).unwrap();
        let preview = preset_preview(&paths, &skin.preset.id).unwrap();
        assert_eq!(preview.model, "slim");
        assert_eq!(preview.skin_rgba.len(), 64 * 64 * 4);
        assert_eq!(preview.skin_rgba[3], 128);
        std::fs::remove_file(png_path(&paths, &skin.preset.id).unwrap()).unwrap();
        assert!(preset_preview(&paths, &skin.preset.id).is_err());
        assert_eq!(list_presets(&paths).unwrap().len(), 1);
        std::fs::remove_dir_all(paths.data_root()).unwrap();
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
        assert_ne!(first.preset.id, second.preset.id);
        assert!(!first.duplicate && !second.duplicate);
        assert_eq!(list_presets(&paths).unwrap().len(), 2);
        assert_eq!(
            preset_for_upload(&paths, &first.preset.id).unwrap().1,
            bytes
        );
        assert_eq!(
            preset_for_upload(&paths, "../escape").unwrap_err().code,
            "skin_preset_missing"
        );
        remove_preset(&paths, &first.preset.id).unwrap();
        assert_eq!(list_presets(&paths).unwrap().len(), 1);
        assert_eq!(list_presets(&paths).unwrap()[0].id, second.preset.id);
        assert!(!png_path(&paths, &first.preset.id).unwrap().exists());
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[test]
    fn identical_bytes_and_model_are_deduplicated_but_models_stay_independent() {
        let paths = paths();
        let bytes = fixture(64, 64);
        let first = import_preset(&paths, "Original", SkinModel::Classic, &bytes).unwrap();
        let again = import_preset(&paths, "Different name", SkinModel::Classic, &bytes).unwrap();
        assert!(again.duplicate);
        assert_eq!(again.preset.id, first.preset.id);
        // The duplicate import keeps the existing entry exactly as it was.
        assert_eq!(again.preset.name, "Original");
        assert_eq!(list_presets(&paths).unwrap().len(), 1);
        // Same bytes under the other model remain a distinct library entry
        // with independent model metadata.
        let slim = import_preset(&paths, "Original", SkinModel::Slim, &bytes).unwrap();
        assert!(!slim.duplicate);
        assert_ne!(slim.preset.id, first.preset.id);
        assert_eq!(list_presets(&paths).unwrap().len(), 2);
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[test]
    fn rename_and_model_updates_are_metadata_only() {
        let paths = paths();
        let bytes = fixture(64, 64);
        let imported = import_preset(&paths, "Before", SkinModel::Classic, &bytes).unwrap();
        let id = imported.preset.id.clone();
        let updated = update_preset(&paths, &id, Some("After"), Some(SkinModel::Slim)).unwrap();
        assert_eq!(updated[0].name, "After");
        assert_eq!(updated[0].model, SkinModel::Slim);
        // Content identity and managed bytes are untouched by the rename.
        assert_eq!(updated[0].sha256, imported.preset.sha256);
        assert_eq!(preset_for_upload(&paths, &id).unwrap().1, bytes);
        assert_eq!(
            update_preset(&paths, &id, Some("   "), None)
                .unwrap_err()
                .code,
            "skin_preset_name_invalid"
        );
        assert_eq!(
            update_preset(&paths, &id, Some(&"x".repeat(81)), None)
                .unwrap_err()
                .code,
            "skin_preset_name_invalid"
        );
        assert_eq!(
            update_preset(&paths, "unknown-id", Some("Nope"), None)
                .unwrap_err()
                .code,
            "skin_preset_missing"
        );
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[test]
    fn thumbnails_are_bounded_head_crops_of_verified_bytes() {
        let paths = paths();
        let bytes = fixture(64, 64);
        let imported = import_preset(&paths, "Thumb", SkinModel::Classic, &bytes).unwrap();
        let thumbnail = preset_thumbnail(&paths, &imported.preset.id)
            .unwrap()
            .expect("a saved 64x64 skin yields a head thumbnail");
        assert_eq!(thumbnail.len(), 256);
        assert_eq!(
            preset_thumbnail(&paths, "missing-id").unwrap_err().code,
            "skin_preset_missing"
        );
        // The legacy 32-high form produces the same shape.
        let legacy = import_preset(&paths, "Legacy", SkinModel::Classic, &fixture(64, 32)).unwrap();
        assert!(
            preset_thumbnail(&paths, &legacy.preset.id)
                .unwrap()
                .is_some()
        );
        std::fs::remove_dir_all(paths.data_root()).unwrap();
    }

    #[tokio::test]
    async fn current_skin_download_is_bounded_and_png_typed() {
        let png = fixture(64, 64);
        let server = TestServer::spawn(Arc::new(move |request| match request.path.as_str() {
            "/skin.png" => TestResponse::ok(&png).with_header("Content-Type", "image/png"),
            "/html" => TestResponse::ok(b"<html>").with_header("Content-Type", "text/html"),
            "/oversized" => TestResponse::ok(&vec![0u8; MAX_PNG + 1])
                .with_header("Content-Type", "image/png")
                .with_close_framing(),
            _ => TestResponse::status(404),
        }));
        let url = |path| url::Url::parse(&format!("{}{path}", server.base_url())).unwrap();
        assert_eq!(
            download_current_skin(&url("/skin.png")).await.unwrap(),
            fixture(64, 64)
        );
        for path in ["/html", "/oversized", "/missing"] {
            assert!(download_current_skin(&url(path)).await.is_err(), "{path}");
        }
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
        let future = br#"{"schemaVersion":999,"presets":[]}"#;
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
        serde_json::json!({"id": ACCOUNT, "skins":[{"id":"fixture-skin","state":"ACTIVE","variant":model}], "capes":[{"id":"owned-cape","state":if active {"ACTIVE"} else {"INACTIVE"},"alias":"Founder"}]}).to_string().into_bytes()
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
                    TestResponse::ok(&profile(false, "SLIM"))
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
    async fn successful_upload_requires_matching_fresh_identity_and_profile() {
        let token = SecretString::new("FIXTURE-SECRET-TOKEN");
        for mode in ["stale", "unavailable", "expired", "empty-upload"] {
            let server =
                TestServer::spawn(Arc::new(move |request| match request.method.as_str() {
                    "POST" => {
                        if mode == "empty-upload" {
                            TestResponse::ok(&[])
                        } else {
                            TestResponse::ok(&profile(false, "CLASSIC"))
                        }
                    }
                    "GET" if mode == "unavailable" => TestResponse::status(503),
                    "GET" if mode == "expired" => TestResponse::status(401),
                    "GET" => {
                        let mut body: serde_json::Value =
                            serde_json::from_slice(&profile(false, "CLASSIC")).unwrap();
                        if mode == "stale" {
                            body["skins"][0]["id"] = "previous-same-model-skin".into();
                        }
                        TestResponse::ok(&serde_json::to_vec(&body).unwrap())
                    }
                    _ => TestResponse::status(404),
                }));
            let failure = apply_skin_to(
                &format!("{}/minecraft/profile", server.base_url()),
                &format!("{}/minecraft/profile/skins", server.base_url()),
                ACCOUNT,
                &token,
                &fixture(64, 64),
                SkinModel::Classic,
            )
            .await
            .unwrap_err();
            assert_eq!(
                failure.code,
                match mode {
                    "unavailable" => "cosmetics_service_unavailable",
                    "expired" => "cosmetics_authentication_required",
                    _ => "cosmetics_refresh_failed",
                }
            );
            assert_eq!(server.request_count(), 2);
            assert!(!failure.message.contains("FIXTURE-SECRET-TOKEN"));
        }
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
