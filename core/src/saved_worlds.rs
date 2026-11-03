//! Read-only discovery of saved singleplayer worlds (`saves/<world>`)
//! inside managed instance game directories.
//!
//! `level.dat` is Minecraft's own record: `LastPlayed` and `LevelName` are
//! Minecraft-recorded metadata, never Aurora-observed visits. Directory
//! identity is the validated save-folder name. Nothing here writes into a
//! save, opens region files, or treats existence as gameplay evidence.

use crate::gameplay_history::WorldSaveId;
use crate::instances::InstanceId;
use crate::paths::ManagedPaths;
use crate::{cosmetic_image, nbt};
use std::io::Read;
use std::path::Path;

/// `level.dat` is normally a few hundred kilobytes; this bounds corrupt or
/// hostile documents before decompression.
const MAX_COMPRESSED: u64 = 2 * 1024 * 1024;
/// Decompressed documents are worldgen-heavy; discovery walks only named
/// levels, and anything beyond this bound is rejected outright.
const MAX_DECOMPRESSED: usize = 16 * 1024 * 1024;
/// Presentation bound for one instance's save list.
const MAX_WORLDS: usize = 200;
/// World icons are 64×64 PNGs in practice; the favicon policy bounds them.
const MAX_ICON: u64 = 128 * 1024;

#[derive(Debug)]
pub enum ReadError {
    Malformed,
    Redirected,
}

pub struct SavedWorld {
    /// Stable filesystem identity of the save directory.
    pub directory: WorldSaveId,
    /// Minecraft-recorded `Data.LevelName`.
    pub display_name: Option<String>,
    /// Minecraft-recorded `Data.LastPlayed` (epoch milliseconds).
    pub last_played_ms: u64,
    /// Minecraft-recorded `Data.GameType`, when present and recognized.
    pub game_mode: Option<&'static str>,
    /// Minecraft-recorded `Data.Version.Name` (for example "1.21.11").
    pub version_name: Option<String>,
    /// Minecraft `Data.DataVersion`.
    pub data_version: Option<i32>,
}

/// Lists one managed instance's saves. Directories are never followed through
/// reparse points. Reads are bounded snapshots and incomplete level files
/// are skipped, including during a concurrent Minecraft save.
pub fn read(paths: &ManagedPaths, instance: &InstanceId) -> Result<Vec<SavedWorld>, ReadError> {
    use crate::install::coordination::checked_directory;
    let saves = paths.instance_paths(instance).root().join("saves");
    match checked_directory(paths.data_root(), &saves) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(ReadError::Redirected),
    }
    let entries = std::fs::read_dir(&saves).map_err(|_| ReadError::Malformed)?;
    let mut worlds = Vec::new();
    for entry in entries.take(1024).flatten() {
        if worlds.len() == MAX_WORLDS {
            break;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Ok(directory) = WorldSaveId::parse(name.to_owned()) else {
            continue;
        };
        let Ok(world_path) = checked_directory(paths.data_root(), &saves.join(name)) else {
            continue;
        };
        if let Some(world) = read_level(&world_path, &directory) {
            worlds.push(world);
        }
    }
    Ok(worlds)
}

/// Reads and validates one world's `icon.png`, returning canonical PNG
/// bytes when it decodes within the favicon policy.
pub fn read_icon(
    paths: &ManagedPaths,
    instance: &InstanceId,
    directory: &WorldSaveId,
) -> Option<Vec<u8>> {
    let saves = paths.instance_paths(instance).root().join("saves");
    let world = crate::install::coordination::checked_directory(
        paths.data_root(),
        &saves.join(directory.as_str()),
    )
    .ok()?;
    let bytes = nbt::read_file(&world.join("icon.png"), MAX_ICON).ok()?;
    cosmetic_image::validate(&bytes, cosmetic_image::FAVICON).map(|png| png.bytes().to_vec())
}

fn read_level(world: &Path, directory: &WorldSaveId) -> Option<SavedWorld> {
    let compressed = nbt::read_file(&world.join("level.dat"), MAX_COMPRESSED).ok()?;
    let decompressed = gunzip_bounded(&compressed)?;
    let root = nbt::root(&decompressed).ok()?;
    let data = nbt::find(&root, "Data")?;
    if data.tag != 10 {
        return None;
    }
    let mut reader = nbt::reader_at(data.payload, 0);
    let fields = reader.compound(1).ok()?;
    let display_name = nbt::find(&fields, "LevelName")
        .and_then(|field| nbt::as_string(field).ok())
        .and_then(|bytes| simd_cesu8::mutf8::decode_strict(bytes).ok())
        .and_then(|text| {
            let clean = crate::launch::activity_bridge::sanitize(&text);
            (!clean.is_empty() && clean.len() <= 512).then_some(clean)
        });
    let last_played_ms = nbt::find(&fields, "LastPlayed")
        .and_then(|field| nbt::as_i64(field).ok())
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(0);
    let game_mode = nbt::find(&fields, "GameType")
        .and_then(|field| nbt::as_i32(field).ok())
        .and_then(|value| match value {
            0 => Some("Survival"),
            1 => Some("Creative"),
            2 => Some("Adventure"),
            3 => Some("Spectator"),
            _ => None,
        });
    let data_version = nbt::find(&fields, "DataVersion").and_then(|field| nbt::as_i32(field).ok());
    let version_name = nbt::find(&fields, "Version")
        .filter(|field| field.tag == 10)
        .and_then(|field| {
            let mut reader = nbt::reader_at(field.payload, 0);
            let fields = reader.compound(2).ok()?;
            nbt::find(&fields, "Name")
                .and_then(|field| nbt::as_string(field).ok())
                .and_then(|bytes| simd_cesu8::mutf8::decode_strict(bytes).ok())
                .and_then(|text| {
                    let clean = crate::launch::activity_bridge::sanitize(&text);
                    (!clean.is_empty() && clean.len() <= 64).then_some(clean)
                })
        });
    Some(SavedWorld {
        directory: directory.clone(),
        display_name,
        last_played_ms,
        game_mode,
        version_name,
        data_version,
    })
}

fn gunzip_bounded(compressed: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = flate2::bufread::GzDecoder::new(compressed);
    let mut out = Vec::new();
    // `take` bounds the decompressed size, so a hostile archive cannot
    // expand past the discovery budget; truncation then fails parsing.
    (&mut decoder)
        .take(MAX_DECOMPRESSED as u64 + 1)
        .read_to_end(&mut out)
        .ok()?;
    if out.len() > MAX_DECOMPRESSED || !decoder.get_ref().is_empty() {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use std::io::Write;

    fn string(value: &str) -> Vec<u8> {
        let bytes = value.as_bytes();
        let mut out = (bytes.len() as u16).to_be_bytes().to_vec();
        out.extend_from_slice(bytes);
        out
    }

    fn compound(fields: &[(&str, u8, Vec<u8>)], end: bool) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, tag, payload) in fields {
            let name = name.as_bytes();
            out.push(*tag);
            out.extend_from_slice(&(name.len() as u16).to_be_bytes());
            out.extend_from_slice(name);
            out.extend_from_slice(payload);
        }
        if end {
            out.push(0);
        }
        out
    }

    fn level_dat(name: &str, last_played: i64, game_type: i32) -> Vec<u8> {
        let version = compound(&[("Name", 8u8, string("1.21.11"))], true);
        let data = compound(
            &[
                ("LevelName", 8u8, string(name)),
                ("LastPlayed", 4u8, last_played.to_be_bytes().to_vec()),
                ("GameType", 3u8, game_type.to_be_bytes().to_vec()),
                ("DataVersion", 3u8, 4189_i32.to_be_bytes().to_vec()),
                ("Version", 10u8, version),
            ],
            true,
        );
        let mut document = vec![10u8];
        document.extend_from_slice(&string(""));
        document.extend_from_slice(&compound(&[("Data", 10u8, data)], true));
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&document).unwrap();
        encoder.finish().unwrap()
    }

    fn fixture(name: &str) -> (std::path::PathBuf, ManagedPaths) {
        let root =
            std::env::temp_dir().join(format!("aurora-saved-worlds-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let instance = InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        let saves = managed
            .instance_paths(&instance)
            .root()
            .join("saves")
            .join(name);
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(
            saves.join("level.dat"),
            level_dat(name, 1_730_000_000_000, 1),
        )
        .unwrap();
        (root, managed)
    }

    fn instance_of(managed: &ManagedPaths) -> InstanceId {
        let entries: Vec<_> = std::fs::read_dir(managed.instances_dir())
            .unwrap()
            .flatten()
            .collect();
        let name = entries[0].file_name().into_string().unwrap();
        InstanceId::new(name).unwrap()
    }

    #[test]
    fn lists_worlds_with_minecraft_recorded_metadata() {
        let (root, managed) = fixture("Home World");
        let instance = instance_of(&managed);
        let worlds = read(&managed, &instance).unwrap();
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].directory.as_str(), "Home World");
        assert_eq!(worlds[0].display_name.as_deref(), Some("Home World"));
        assert_eq!(worlds[0].last_played_ms, 1_730_000_000_000);
        assert_eq!(worlds[0].game_mode, Some("Creative"));
        assert_eq!(worlds[0].version_name.as_deref(), Some("1.21.11"));

        // Truncated and non-gzip documents are skipped, not fatal.
        let saves = managed.instance_paths(&instance).root().join("saves");
        std::fs::create_dir_all(saves.join("Broken")).unwrap();
        std::fs::write(saves.join("Broken").join("level.dat"), b"junk").unwrap();
        std::fs::create_dir_all(saves.join("Empty")).unwrap();
        let worlds = read(&managed, &instance).unwrap();
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].directory.as_str(), "Home World");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_oversized_and_trailing_gzip() {
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&vec![0; MAX_DECOMPRESSED + 1]).unwrap();
        assert!(gunzip_bounded(&encoder.finish().unwrap()).is_none());
        let mut valid = level_dat("Home", 0, 0);
        valid.push(1);
        assert!(gunzip_bounded(&valid).is_none());
    }

    #[test]
    fn preserves_directory_identity_for_duplicate_world_names() {
        let (root, managed) = fixture("First");
        let instance = instance_of(&managed);
        let saves = managed.instance_paths(&instance).root().join("saves");
        let second = saves.join("Second");
        std::fs::create_dir(&second).unwrap();
        std::fs::write(second.join("level.dat"), level_dat("First", 1, 0)).unwrap();
        let worlds = read(&managed, &instance).unwrap();
        assert_eq!(worlds.len(), 2);
        assert_ne!(worlds[0].directory, worlds[1].directory);
        assert_eq!(worlds[0].display_name, worlds[1].display_name);
        assert!(WorldSaveId::parse("../Second".into()).is_err());
        std::fs::remove_file(second.join("level.dat")).unwrap();
        std::fs::remove_dir(second).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn icon_policy_rejects_non_png_and_bounds_size() {
        let (root, managed) = fixture("Iconed");
        let instance = instance_of(&managed);
        let saves = managed.instance_paths(&instance).root().join("saves");
        assert!(
            read_icon(
                &managed,
                &instance,
                &WorldSaveId::parse("Iconed".into()).unwrap()
            )
            .is_none()
        );

        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 64, 64);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&vec![0u8; 64 * 64 * 4]).unwrap();
            writer.finish().unwrap();
        }
        std::fs::write(saves.join("Iconed").join("icon.png"), &png).unwrap();
        let icon = read_icon(
            &managed,
            &instance,
            &WorldSaveId::parse("Iconed".into()).unwrap(),
        );
        assert!(icon.is_some());

        std::fs::write(saves.join("Iconed").join("icon.png"), b"not a png").unwrap();
        assert!(
            read_icon(
                &managed,
                &instance,
                &WorldSaveId::parse("Iconed".into()).unwrap()
            )
            .is_none()
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
