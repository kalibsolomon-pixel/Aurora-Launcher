//! Local, per-instance screenshots. No watcher, network access or arbitrary-path API.
use crate::install::coordination::{checked_directory, redirected};
use crate::{instances::InstanceId, paths::ManagedPaths};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::UNIX_EPOCH,
};

#[cfg(windows)]
mod windows;

const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_PIXELS: u64 = 32 * 1024 * 1024;
pub const THUMBNAIL_SIDE: u32 = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Screenshot {
    pub name: String,
    pub bytes: u64,
    /// Nanoseconds make cache invalidation independent of display-date rounding.
    pub modified_ns: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}

/// One Windows-compatible leaf, including its PNG/JPEG extension. No ADS,
/// reserved devices, trailing dots/spaces, separators or control characters.
pub fn validate_name(name: &str) -> io::Result<()> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end()
        .to_ascii_uppercase();
    if name.is_empty()
        || name.encode_utf16().count() > 240
        || name.starts_with('.')
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        )
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|s| {
                matches!(
                    s,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        })
        || !matches!(
            Path::new(name)
                .extension()
                .and_then(|s| s.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("png" | "jpg" | "jpeg")
        )
    {
        return Err(invalid(
            "Use a PNG or JPG filename without path separators or reserved characters.",
        ));
    }
    Ok(())
}

/// Pin every managed ancestor against replacement while a shell/file operation
/// uses the checked pathname. The platform-granted data root may itself alias.
struct Directory {
    path: PathBuf,
    #[cfg(windows)]
    _pins: Vec<fs::File>,
}
fn directory(paths: &ManagedPaths, instance: &InstanceId) -> io::Result<Directory> {
    let target = paths.instance_paths(instance).root().join("screenshots");
    let checked = checked_directory(paths.data_root(), &target)?;
    #[cfg(windows)]
    let pins = {
        use std::os::windows::fs::OpenOptionsExt;
        let root = fs::canonicalize(paths.data_root())?;
        let mut current = root.clone();
        let mut pins = Vec::new();
        for part in std::iter::once(None).chain(
            checked
                .strip_prefix(&root)
                .map_err(|_| invalid("Unsafe screenshots directory."))?
                .components()
                .map(Some),
        ) {
            if let Some(part) = part {
                current.push(part);
            }
            let file = fs::OpenOptions::new()
                .read(true)
                .share_mode(3)
                .custom_flags(0x0220_0000)
                .open(&current)?; // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
            let meta = file.metadata()?;
            if !meta.is_dir() || redirected(&meta) {
                return Err(invalid("Redirected screenshots directory."));
            }
            pins.push(file);
        }
        pins
    };
    Ok(Directory {
        path: checked,
        #[cfg(windows)]
        _pins: pins,
    })
}

fn describe(path: &Path, name: String) -> io::Result<Screenshot> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || redirected(&meta) {
        return Err(invalid("Screenshots must be regular files, without links."));
    }
    Ok(Screenshot {
        name,
        bytes: meta.len(),
        modified_ns: meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    })
}
fn checked_file(dir: &Directory, shot: &Screenshot) -> io::Result<PathBuf> {
    validate_name(&shot.name)?;
    let path = dir.path.join(&shot.name);
    if describe(&path, shot.name.clone())? != *shot {
        return Err(invalid(
            "The screenshot changed. Refresh the gallery and try again.",
        ));
    }
    Ok(path)
}

pub fn list(paths: &ManagedPaths, instance: &InstanceId) -> io::Result<Vec<Screenshot>> {
    let dir = match directory(paths, instance) {
        Ok(dir) => dir,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut shots = Vec::new();
    for entry in fs::read_dir(&dir.path)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if validate_name(&name).is_err() {
            continue;
        }
        match describe(&entry.path(), name) {
            Ok(shot) => shots.push(shot),
            // Vanishing files and links are omitted; never followed.
            Err(_) => continue,
        }
    }
    shots.sort_by(|a, b| {
        b.modified_ns
            .cmp(&a.modified_ns)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(shots)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CacheKey {
    path: PathBuf,
    bytes: u64,
    modified_ns: u128,
    side: u32,
}
impl CacheKey {
    fn new(path: PathBuf, shot: &Screenshot, side: u32) -> Self {
        Self {
            path,
            bytes: shot.bytes,
            modified_ns: shot.modified_ns,
            side,
        }
    }
}
#[derive(Default)]
struct ThumbnailCache(VecDeque<(CacheKey, Arc<Bitmap>)>);
impl ThumbnailCache {
    fn get(&self, key: &CacheKey) -> Option<Arc<Bitmap>> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }
    fn insert(&mut self, key: CacheKey, bitmap: Arc<Bitmap>) {
        self.0.retain(|(old, _)| old.path != key.path);
        self.0.push_back((key, bitmap));
        while self.0.len() > 96 {
            self.0.pop_front();
        } // at most 24 MiB of thumbnail pixels
    }
}
static CACHE: OnceLock<Mutex<ThumbnailCache>> = OnceLock::new();
static DECODE: Mutex<()> = Mutex::new(()); // one full-size decode at a time, across all views

fn decode(path: &Path) -> io::Result<image::DynamicImage> {
    let bytes = crate::nbt::read_file(path, MAX_FILE)?;
    decode_bytes(&bytes)
}

fn decode_bytes(bytes: &[u8]) -> io::Result<image::DynamicImage> {
    let reader = image::ImageReader::new(io::Cursor::new(bytes)).with_guessed_format()?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err(invalid("This file is not a PNG or JPEG image."));
    }
    let (width, height) = image::ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()?
        .into_dimensions()
        .map_err(io::Error::other)?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(invalid("Screenshot exceeds the 32 megapixel limit."));
    }
    let mut reader = reader;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|_| {
        invalid(
            "The screenshot could not be decoded. It may still be saving; refresh and try again.",
        )
    })
}

/// Called only on a blocking worker by the typed API. Preview is bounded too;
/// clipboard decoding retains the original dimensions within the same limits.
pub fn bitmap(
    paths: &ManagedPaths,
    instance: &InstanceId,
    shot: &Screenshot,
    preview: bool,
) -> io::Result<Bitmap> {
    let dir = directory(paths, instance)?;
    let path = checked_file(&dir, shot)?;
    let side = if preview { 1920 } else { THUMBNAIL_SIDE };
    let key = CacheKey::new(path.clone(), shot, side);
    let cache = CACHE.get_or_init(Default::default);
    let _decode = DECODE
        .lock()
        .map_err(|_| invalid("Image worker unavailable."))?;
    if !preview {
        if let Some(bitmap) = cache
            .lock()
            .map_err(|_| invalid("Thumbnail cache unavailable."))?
            .get(&key)
        {
            return Ok((*bitmap).clone());
        }
    }
    let bytes = crate::nbt::read_file(&path, MAX_FILE)?;
    let bitmap = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        let (width,height,rgba) = crate::cosmetic_image::screenshot_thumbnail_rgba(&bytes,side)
            .ok_or_else(|| invalid("The screenshot is incomplete, invalid or exceeds image limits. Refresh after it finishes saving."))?;
        Bitmap {
            width,
            height,
            rgba,
        }
    } else {
        let image = decode_bytes(&bytes)?.thumbnail(side, side).into_rgba8();
        Bitmap {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        }
    };
    checked_file(&dir, shot)?; // reject partial writes / replacement during decode
    if !preview {
        cache
            .lock()
            .map_err(|_| invalid("Thumbnail cache unavailable."))?
            .insert(key, Arc::new(bitmap.clone()));
    }
    Ok(bitmap)
}

pub fn rename(
    paths: &ManagedPaths,
    instance: &InstanceId,
    shot: &Screenshot,
    name: &str,
) -> io::Result<()> {
    validate_name(name)?;
    if !Path::new(name).extension().unwrap().eq_ignore_ascii_case(
        Path::new(&shot.name)
            .extension()
            .ok_or_else(|| invalid("Invalid screenshot."))?,
    ) {
        return Err(invalid("Keep the original image extension when renaming."));
    }
    let dir = directory(paths, instance)?;
    let source = checked_file(&dir, shot)?;
    if name == shot.name {
        return Ok(());
    }
    let destination = dir.path.join(name);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(invalid("A file already has that name."));
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let from: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe { windows::move_file(&from, &to) }
    }
    #[cfg(not(windows))]
    {
        // Atomic no-clobber destination, never std::fs::rename's overwrite.
        fs::hard_link(&source, &destination)?;
        fs::remove_file(source)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    CopyImage,
    CopyFile,
    Reveal,
    Delete,
}

pub fn action(
    paths: &ManagedPaths,
    instance: &InstanceId,
    shot: &Screenshot,
    action: Action,
    confirmed: bool,
) -> io::Result<()> {
    if matches!(action, Action::Delete) && !confirmed {
        return Err(invalid(
            "Confirm moving this screenshot to the Recycle Bin first.",
        ));
    }
    let dir = directory(paths, instance)?;
    let path = checked_file(&dir, shot)?;
    #[cfg(windows)]
    match action {
        Action::CopyImage => {
            let _decode = DECODE
                .lock()
                .map_err(|_| invalid("Image worker unavailable."))?;
            let rgba = decode(&path)?.into_rgba8();
            checked_file(&dir, shot)?;
            windows::clipboard(
                17,
                &dib_payload(rgba.width(), rgba.height(), rgba.as_raw())?,
            )
        }
        Action::CopyFile => windows::clipboard(15, &file_drop_payload(&path)?),
        Action::Reveal => windows::reveal(&path),
        Action::Delete => windows::recycle(&path),
    }
    #[cfg(not(windows))]
    {
        let _ = (path, action);
        Err(invalid(
            "Screenshot clipboard and shell actions currently require Windows.",
        ))
    }
}

/// CF_HDROP: DROPFILES followed by a double-NUL-terminated UTF-16 file list.
pub fn file_drop_payload(path: &Path) -> io::Result<Vec<u8>> {
    if !path.is_absolute() {
        return Err(invalid("Clipboard file must be absolute."));
    }
    #[cfg(windows)]
    let wide: Vec<u16> = {
        use std::os::windows::ffi::OsStrExt;
        windows::shell_path(path)
            .as_os_str()
            .encode_wide()
            .collect()
    };
    #[cfg(not(windows))]
    let wide: Vec<u16> = path
        .to_str()
        .ok_or_else(|| invalid("Invalid file path."))?
        .encode_utf16()
        .collect();
    if wide.contains(&0) {
        return Err(invalid("Invalid file path."));
    }
    let mut bytes = vec![0; 20];
    bytes[..4].copy_from_slice(&20u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
    for unit in wide.into_iter().chain([0, 0]) {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    Ok(bytes)
}

/// CF_DIBV5: top-down, straight-alpha BGRA bitmap, no BITMAPFILEHEADER.
pub fn dib_payload(width: u32, height: u32, rgba: &[u8]) -> io::Result<Vec<u8>> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels == 0
        || pixels > MAX_PIXELS
        || width > i32::MAX as u32
        || height > i32::MAX as u32
        || rgba.len() as u64 != pixels * 4
    {
        return Err(invalid("Invalid clipboard bitmap."));
    }
    let mut bytes = vec![0; 124];
    for (offset, value) in [
        (0, 124),
        (4, width),
        (8, (-(height as i32)) as u32),
        (16, 3),
        (20, rgba.len() as u32),
        (40, 0x00ff0000),
        (44, 0x0000ff00),
        (48, 0x000000ff),
        (52, 0xff000000),
        (56, 0x73524742),
        (108, 4),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
    bytes[14..16].copy_from_slice(&32u16.to_le_bytes());
    bytes.reserve(rgba.len());
    for p in rgba.chunks_exact(4) {
        bytes.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
    }
    Ok(bytes)
}

/// Stable UTC display date without locale or a new time dependency.
pub fn date(modified_ns: u128) -> String {
    let seconds = (modified_ns / 1_000_000_000).min(i64::MAX as u128) as i64;
    let days = seconds / 86400 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} UTC",
        year + i64::from(month <= 2),
        month,
        day,
        seconds / 3600 % 24,
        seconds / 60 % 60
    )
}

#[cfg(test)]
mod tests;
