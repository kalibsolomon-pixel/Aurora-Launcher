//! Cosmetic installed-mod icons: local JAR metadata only, never network or extraction.
//! Static high-DPI pixels are memoized by verified JAR digest, including absent icons.
use crate::{
    instance_mods,
    instances::InstanceId,
    integrity::{ArtifactDigest, verify_file},
    paths::ManagedPaths,
};
use base64ct::{Base64, Encoding};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    path::Path,
    sync::{Mutex, OnceLock},
};
const CACHE_LIMIT: usize = 128;
const ICON_SIDE: u32 = 96;
const MAX_JAR: u64 = 512 * 1024 * 1024;
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IconRequest {
    pub file_name: String,
    pub sha256: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub rgba_base64: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconResult {
    pub sha256: String,
    pub bitmap: Option<Bitmap>,
}
#[derive(Default)]
struct Memo {
    values: HashMap<String, Option<Bitmap>>,
    order: VecDeque<String>,
}
fn memo() -> &'static Mutex<Memo> {
    static MEMO: OnceLock<Mutex<Memo>> = OnceLock::new();
    MEMO.get_or_init(Default::default)
}
fn member(bytes: &[u8], name: &str) -> Option<String> {
    if name == "fabric.mod.json" {
        let icon = instance_mods::fabric_metadata_json(bytes).ok()?.icon?;
        if let Some(path) = icon.as_str() {
            return Some(path.into());
        }
        let object = icon.as_object()?;
        let mut sizes: Vec<_> = object
            .iter()
            .filter_map(|(n, p)| Some((n.parse::<u32>().ok()?, p.as_str()?)))
            .collect();
        sizes.sort_by_key(|(size, _)| *size);
        sizes
            .iter()
            .find(|(size, _)| *size >= ICON_SIDE)
            .or_else(|| sizes.last())
            .map(|(_, p)| (*p).into())
    } else {
        crate::neoforge::mods::NeoForgeModsDocument::parse(std::str::from_utf8(bytes).ok()?)
            .ok()?
            .mods
            .first()?
            .icon_file
            .clone()
    }
}
fn safe_member(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && !path.contains(['\\', ':', '\0'])
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn read_member<R: Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    bound: u64,
) -> Option<Vec<u8>> {
    if !safe_member(name) || zip.file_names().filter(|n| *n == name).count() != 1 {
        return None;
    }
    let file = zip.by_name(name).ok()?;
    if file.size() > bound || file.is_dir() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(bound + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= bound).then_some(bytes)
}
const MAX_ICON_BYTES: u64 = 512 * 1024;

pub(crate) fn pack_directory_icon(path: &Path) -> Option<Bitmap> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_ICON_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_ICON_BYTES + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= MAX_ICON_BYTES)
        .then(|| bitmap(&bytes))
        .flatten()
}

pub(crate) fn pack_archive_icon(path: &Path) -> Option<Bitmap> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_JAR {
        return None;
    }
    let mut zip = zip::ZipArchive::new(file).ok()?;
    if zip.len() > 16_384 {
        return None;
    }
    bitmap(&read_member(&mut zip, "pack.png", MAX_ICON_BYTES)?)
}

pub(crate) fn bitmap(bytes: &[u8]) -> Option<Bitmap> {
    let (width, height, rgba) = crate::cosmetic_image::artwork_thumbnail_rgba(bytes, ICON_SIDE)?;
    Some(Bitmap {
        width,
        height,
        rgba_base64: Base64::encode_string(&rgba),
    })
}

fn extract(path: &Path) -> Option<Bitmap> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    if zip.len() > 4096 {
        return None;
    }
    let mut icon = None;
    for name in [
        "fabric.mod.json",
        "META-INF/neoforge.mods.toml",
        "META-INF/mods.toml",
    ] {
        if let Some(bytes) = read_member(&mut zip, name, 256 * 1024) {
            icon = member(&bytes, name);
            if icon.is_some() {
                break;
            }
        }
    }
    let bytes = read_member(&mut zip, &icon?, 512 * 1024)?;
    let (width, height, rgba) = crate::cosmetic_image::artwork_thumbnail_rgba(&bytes, ICON_SIDE)?;
    Some(Bitmap {
        width,
        height,
        rgba_base64: Base64::encode_string(&rgba),
    })
}
pub fn load(
    paths: &ManagedPaths,
    instance: &InstanceId,
    requests: &[IconRequest],
) -> Vec<IconResult> {
    if requests.len() > 128 {
        return Vec::new();
    }
    let Ok(directory) = instance_mods::validate_mods_directory(paths, instance) else {
        return Vec::new();
    };
    requests
        .iter()
        .map(|request| {
            let bitmap = (|| {
                let digest = ArtifactDigest::parse(&request.sha256).ok()?;
                let key = digest.as_hex();
                let name = request.file_name.to_ascii_lowercase();
                if !(name.ends_with(".jar") || name.ends_with(".jar.disabled")) {
                    return None;
                }
                let path =
                    instance_mods::validate_current_regular_file(&directory, &request.file_name)
                        .ok()?;
                if let Some(value) = memo().lock().unwrap().values.get(&key).cloned() {
                    return value;
                }
                let size = std::fs::metadata(&path).ok()?.len();
                if size > MAX_JAR {
                    return None;
                }
                verify_file(&path, &digest, Some(size)).ok()?;
                let value = extract(&path);
                let mut cache = memo().lock().unwrap();
                if !cache.values.contains_key(&key) {
                    while cache.values.len() >= CACHE_LIMIT {
                        if let Some(old) = cache.order.pop_front() {
                            cache.values.remove(&old);
                        }
                    }
                    cache.order.push_back(key.clone());
                    cache.values.insert(key, value.clone());
                }
                value
            })();
            IconResult {
                sha256: request.sha256.clone(),
                bitmap,
            }
        })
        .collect()
}

/// A closed content kind and a direct-child name select local saved artwork.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackIconRequest {
    pub file_name: String,
    pub sha256: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIconResult {
    pub file_name: String,
    pub bitmap: Option<Bitmap>,
}

pub fn load_packs(
    paths: &ManagedPaths,
    instance: &InstanceId,
    kind: crate::instance_content::ContentType,
    requests: &[PackIconRequest],
) -> Vec<PackIconResult> {
    use crate::instance_content::{ContentType, validate_directory, validate_file_name};
    if requests.len() > 128 || kind == ContentType::Mod {
        return Vec::new();
    }
    let Ok(directory) = validate_directory(paths, instance, kind) else {
        return Vec::new();
    };
    requests
        .iter()
        .map(|request| {
            let bitmap = (|| {
                validate_file_name(&request.file_name).ok()?;
                let path = directory.join(&request.file_name);
                let metadata = std::fs::symlink_metadata(&path).ok()?;
                if metadata.file_type().is_symlink() || reparse_point(&metadata) {
                    return None;
                }
                let bytes = if metadata.is_dir() {
                    // Admit the pack directory itself, then the one regular root PNG.
                    let canonical = std::fs::canonicalize(&path).ok()?;
                    if canonical.parent() != Some(std::fs::canonicalize(&directory).ok()?.as_path())
                    {
                        return None;
                    }
                    let png =
                        instance_mods::validate_current_regular_file(&path, "pack.png").ok()?;
                    let file = std::fs::File::open(png).ok()?;
                    if file.metadata().ok()?.len() > 512 * 1024 {
                        return None;
                    }
                    let mut bytes = Vec::new();
                    file.take(512 * 1024 + 1).read_to_end(&mut bytes).ok()?;
                    if bytes.len() > 512 * 1024 {
                        return None;
                    }
                    bytes
                } else {
                    if !metadata.is_file()
                        || metadata.len() > MAX_JAR
                        || !request.file_name.to_ascii_lowercase().ends_with(".zip")
                    {
                        return None;
                    }
                    let path = instance_mods::validate_current_regular_file(
                        &directory,
                        &request.file_name,
                    )
                    .ok()?;
                    if let Some(hash) = &request.sha256 {
                        let digest = ArtifactDigest::parse(hash).ok()?;
                        verify_file(&path, &digest, Some(metadata.len())).ok()?;
                    }
                    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
                    if zip.len() > 4096 {
                        return None;
                    }
                    read_member(&mut zip, "pack.png", 512 * 1024)?
                };
                let (width, height, rgba) =
                    crate::cosmetic_image::artwork_thumbnail_rgba(&bytes, ICON_SIDE)?;
                Some(Bitmap {
                    width,
                    height,
                    rgba_base64: Base64::encode_string(&rgba),
                })
            })();
            PackIconResult {
                file_name: request.file_name.clone(),
                bitmap,
            }
        })
        .collect()
}

#[cfg(windows)]
fn reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse_point(_: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn jar(path: &Path, icon: &str, bytes: &[u8]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(
            serde_json::to_string(&serde_json::json!({"id":"test", "version":"1", "icon":icon}))
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        zip.start_file(icon, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
        zip.finish().unwrap();
    }
    fn png_rgba(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(pixels).unwrap();
            writer.finish().unwrap();
        }
        bytes
    }
    #[test]
    fn quality_artwork_preserves_aspect_alpha_and_averages_detail_without_upscaling() {
        let png = png_rgba(2, 1, &[255, 0, 0, 255, 0, 0, 255, 0]);
        let (w, h, pixels) = crate::cosmetic_image::artwork_thumbnail_rgba(&png, 1).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(pixels, [255, 0, 0, 128]);
        let (_, _, pixels) = crate::cosmetic_image::artwork_thumbnail_rgba(&png, 96).unwrap();
        assert_eq!(pixels.len(), 2 * 4);
        let checker: Vec<u8> = (0..8)
            .flat_map(|i| {
                if i % 2 == 0 {
                    [0, 0, 0, 255]
                } else {
                    [255, 255, 255, 255]
                }
            })
            .collect();
        let png = png_rgba(4, 2, &checker);
        let (w, h, pixels) = crate::cosmetic_image::artwork_thumbnail_rgba(&png, 2).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(pixels, [128, 128, 128, 255, 128, 128, 128, 255]);
        assert!(crate::cosmetic_image::artwork_thumbnail_rgba(&png, 97).is_none());
        assert!(
            crate::cosmetic_image::artwork_thumbnail_rgba(
                &crate::cosmetic_image::fixtures::animated_png(),
                96
            )
            .is_none()
        );
    }
    #[test]
    fn saved_zip_and_directory_artwork_is_local_bounded_and_path_checked() {
        let root = std::env::temp_dir().join(format!("aurora-pack-icons-{}", uuid::Uuid::new_v4()));
        let paths = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let id = InstanceId::new("pack-icon-fixture").unwrap();
        let directory = paths.instance_paths(&id).root().join("resourcepacks");
        std::fs::create_dir_all(directory.join("folder")).unwrap();
        let png = crate::cosmetic_image::fixtures::png(128, 64);
        std::fs::write(directory.join("folder/pack.png"), &png).unwrap();
        let archive = directory.join("local.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        zip.start_file("pack.png", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&png).unwrap();
        zip.finish().unwrap();
        let request = |name: &str| PackIconRequest {
            file_name: name.into(),
            sha256: None,
        };
        let kind = crate::instance_content::ContentType::ResourcePack;
        for name in ["folder", "local.zip"] {
            let result = load_packs(&paths, &id, kind, &[request(name)]);
            let bitmap = result[0].bitmap.as_ref().unwrap();
            assert_eq!((bitmap.width, bitmap.height), (96, 48));
        }
        for name in [
            "../local.zip",
            "/local.zip",
            "C:/local.zip",
            "folder/pack.png",
            "absent.zip",
        ] {
            assert!(
                load_packs(&paths, &id, kind, &[request(name)])[0]
                    .bitmap
                    .is_none()
            );
        }
        assert!(
            load_packs(
                &paths,
                &id,
                kind,
                &[PackIconRequest {
                    file_name: "local.zip".into(),
                    sha256: Some("a".repeat(64))
                }]
            )[0]
            .bitmap
            .is_none()
        );
        std::fs::write(directory.join("folder/pack.png"), vec![0; 512 * 1024 + 1]).unwrap();
        assert!(
            load_packs(&paths, &id, kind, &[request("folder")])[0]
                .bitmap
                .is_none()
        );
        assert!(
            load_packs(
                &paths,
                &id,
                crate::instance_content::ContentType::Mod,
                &[request("local.zip")]
            )
            .is_empty()
        );
        assert!(load_packs(&paths, &id, kind, &vec![request("local.zip"); 129]).is_empty());
        // Only this exact unique disposable root was created by this test.
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pack_artwork_uses_only_a_bounded_root_png() {
        let fixture =
            std::env::temp_dir().join(format!("aurora-pack-icons-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&fixture).unwrap();
        let path = fixture.join("pack.zip");
        let png = crate::cosmetic_image::fixtures::png(128, 64);
        for (name, bytes, expected) in [
            ("pack.png", png.as_slice(), true),
            ("nested/pack.png", png.as_slice(), false),
            ("pack.png", b"corrupt".as_slice(), false),
            (
                "pack.png",
                vec![0; MAX_ICON_BYTES as usize + 1].as_slice(),
                false,
            ),
        ] {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
            zip.finish().unwrap();
            assert_eq!(pack_archive_icon(&path).is_some(), expected);
        }
        std::fs::remove_file(path).unwrap();
        let path = fixture.join("pack.png");
        std::fs::write(&path, png).unwrap();
        let image = pack_directory_icon(&path).unwrap();
        assert_eq!((image.width, image.height), (96, 48));
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(fixture).unwrap();
    }

    #[test]
    fn static_thumbnail_is_bounded_and_rejects_animation_truncation_and_bad_crc() {
        let png = crate::cosmetic_image::fixtures::png(128, 64);
        let (w, h, pixels) = crate::cosmetic_image::thumbnail_rgba(&png, 48).unwrap();
        assert_eq!((w, h, pixels.len()), (48, 24, 48 * 24 * 4));
        assert_eq!(&pixels[..4], &[127, 127, 127, 255]);
        assert!(
            crate::cosmetic_image::thumbnail_rgba(
                &crate::cosmetic_image::fixtures::animated_png(),
                48
            )
            .is_none()
        );
        assert!(crate::cosmetic_image::thumbnail_rgba(&png[..png.len() - 5], 48).is_none());
        let mut bad = png.clone();
        bad[29] ^= 1;
        assert!(crate::cosmetic_image::thumbnail_rgba(&bad, 48).is_none());
    }
    #[test]
    fn archive_paths_and_size_maps_are_strict_and_local() {
        for path in [
            "../x.png", "/x.png", "C:/x.png", "a\\b", "a//b", "a/./b", "a/../b",
        ] {
            assert!(!safe_member(path));
        }
        assert!(safe_member("assets/test/icon.png"));
        assert_eq!(
            member(
                br#"{"id":"test","icon":{"16":"small.png","64":"best.png","128":"large.png"}}"#,
                "fabric.mod.json"
            )
            .as_deref(),
            Some("large.png")
        );
        assert!(member(br#"{"icon": 42}"#, "fabric.mod.json").is_none());
    }
    #[test]
    fn icon_hash_cache_survives_toggle_and_missing_icons_are_negative_cached() {
        use sha2::Digest;
        let root = std::env::temp_dir().join(format!("aurora-mod-icons-{}", uuid::Uuid::new_v4()));
        let paths = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let id = InstanceId::new("icon-fixture").unwrap();
        let mods = paths.instance_paths(&id).mods().to_owned();
        std::fs::create_dir_all(&mods).unwrap();
        let path = mods.join("test.jar");
        jar(
            &path,
            "assets/test/icon.png",
            &crate::cosmetic_image::fixtures::png(128, 64),
        );
        let hash = format!("{:x}", sha2::Sha256::digest(std::fs::read(&path).unwrap()));
        let start = std::time::Instant::now();
        let request = IconRequest {
            file_name: "test.jar".into(),
            sha256: hash.clone(),
        };
        let cold = load(&paths, &id, &[request.clone()]);
        let cold_time = start.elapsed();
        assert_eq!(cold[0].bitmap.as_ref().unwrap().width, 96);
        std::fs::rename(&path, mods.join("test.jar.disabled")).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            assert!(
                load(
                    &paths,
                    &id,
                    &[IconRequest {
                        file_name: "test.jar.disabled".into(),
                        sha256: hash.clone()
                    }]
                )[0]
                .bitmap
                .is_some()
            );
        }
        eprintln!(
            "mod icons cold={:?} warm100={:?} thumbnailBytes={}",
            cold_time,
            start.elapsed(),
            96 * 48 * 4
        );
        assert!(
            load(
                &paths,
                &id,
                &[IconRequest {
                    file_name: "../outside.jar".into(),
                    sha256: hash
                }]
            )[0]
            .bitmap
            .is_none()
        );
        let other = mods.join("missing.jar");
        jar(&other, "missing.png", b"bad PNG");
        let hash = format!("{:x}", sha2::Sha256::digest(std::fs::read(&other).unwrap()));
        assert!(
            load(
                &paths,
                &id,
                &[IconRequest {
                    file_name: "missing.jar".into(),
                    sha256: hash.clone()
                }]
            )[0]
            .bitmap
            .is_none()
        );
        assert!(memo().lock().unwrap().values.get(&hash).unwrap().is_none());
        assert!(
            load(
                &paths,
                &id,
                &[IconRequest {
                    file_name: "missing.jar".into(),
                    sha256: "a".repeat(64)
                }]
            )[0]
            .bitmap
            .is_none()
        );
        // Only the exact disposable root created by this test.
        std::fs::remove_dir_all(root).unwrap();
    }
}
