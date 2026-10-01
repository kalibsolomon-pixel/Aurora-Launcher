//! Bounded, fail-closed Modrinth pack format 1 parser. The archive is already
//! acquired through the verified artifact cache before this boundary runs.
//! No archive entry is extracted here; the output is an immutable install plan.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

pub const MAX_PACK_BYTES: u64 = 512 * 1024 * 1024;
const MAX_INDEX_BYTES: u64 = 1024 * 1024;
const MAX_ENTRIES: usize = 8192;
const MAX_OVERRIDE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Index {
    format_version: u32,
    game: String,
    version_id: String,
    name: String,
    #[serde(default)]
    summary: Option<String>,
    files: Vec<IndexFile>,
    dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IndexFile {
    path: String,
    hashes: FileHashes,
    downloads: Vec<String>,
    file_size: u64,
    #[serde(default)]
    env: Option<FileEnvironment>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileHashes {
    sha1: String,
    sha512: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileEnvironment {
    client: EnvironmentSide,
    #[serde(rename = "server")]
    _server: EnvironmentSide,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EnvironmentSide {
    Required,
    Optional,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub path: String,
    pub sha1: String,
    pub sha512: String,
    pub downloads: Vec<String>,
    pub file_size: u64,
    pub client: EnvironmentSide,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideFile {
    pub path: String,
    pub archive_index: usize,
    pub sha256: String,
    pub size: u64,
    pub layer: OverrideLayer,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OverrideLayer {
    Common,
    Client,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackPlan {
    pub name: String,
    pub pack_version: String,
    pub minecraft_version: String,
    pub fabric_loader_version: String,
    pub required_files: Vec<PackFile>,
    pub optional_files: Vec<PackFile>,
    pub excluded_paths: Vec<String>,
    pub overrides: Vec<OverrideFile>,
}

#[derive(Debug)]
pub enum PackError {
    Io,
    ArchiveLimit,
    InvalidArchive,
    InvalidIndex,
    UnsupportedFormat,
    UnsupportedLoader,
    InvalidPath,
    PathCollision,
    InvalidHash,
    InvalidDownload,
    UnsafeOverride,
}

impl PackError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io => "pack_io_error",
            Self::ArchiveLimit => "pack_archive_limit",
            Self::InvalidArchive => "pack_invalid_archive",
            Self::InvalidIndex => "pack_invalid_index",
            Self::UnsupportedFormat => "pack_unsupported_format",
            Self::UnsupportedLoader => "pack_unsupported_loader",
            Self::InvalidPath => "pack_invalid_path",
            Self::PathCollision => "pack_path_collision",
            Self::InvalidHash => "pack_invalid_hash",
            Self::InvalidDownload => "pack_invalid_download",
            Self::UnsafeOverride => "pack_unsafe_override",
        }
    }
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Io => "The verified pack archive could not be read.",
            Self::ArchiveLimit => "The pack exceeds Aurora's archive, entry, or expansion limit.",
            Self::InvalidArchive => {
                "The pack ZIP structure is invalid or contains unsupported entries."
            }
            Self::InvalidIndex => "The pack index is missing, malformed, or too large.",
            Self::UnsupportedFormat => "Aurora supports Minecraft .mrpack format version 1 only.",
            Self::UnsupportedLoader => {
                "This pack requires a loader that Aurora cannot install as a Fabric pack."
            }
            Self::InvalidPath => {
                "A pack file names a path outside Aurora's supported game-content roots."
            }
            Self::PathCollision => "Pack files or overrides claim the same destination path.",
            Self::InvalidHash => "A pack file lacks a valid SHA-1 or SHA-512 digest.",
            Self::InvalidDownload => "A pack file has no approved HTTPS download source.",
            Self::UnsafeOverride => {
                "An override has an unsafe path, file type, or changed content."
            }
        })
    }
}

impl std::error::Error for PackError {}

fn digest_shape(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Paths are checked for both Windows and Unix, regardless of host. A strict
/// game-root allowlist stops pack data from naming launcher state files.
pub fn destination_path(path: &str) -> Result<String, PackError> {
    if path.is_empty() || path.len() > 512 || path.starts_with('/') || path.contains('\\') {
        return Err(PackError::InvalidPath);
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() > 32
        || parts.iter().any(|part| {
            part.is_empty()
                || *part == "."
                || *part == ".."
                || part.ends_with(['.', ' '])
                || part
                    .chars()
                    .any(|ch| ch.is_control() || "<>:\"|?*".contains(ch))
                || matches!(
                    part.to_ascii_uppercase().split('.').next().unwrap_or(""),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
    {
        return Err(PackError::InvalidPath);
    }
    let root = parts[0].to_ascii_lowercase();
    let allowed = if parts.len() == 1 {
        matches!(
            root.as_str(),
            "options.txt" | "servers.dat" | "optionsshaders.txt"
        )
    } else {
        matches!(
            root.as_str(),
            "mods"
                | "config"
                | "defaultconfigs"
                | "resourcepacks"
                | "shaderpacks"
                | "kubejs"
                | "scripts"
                | "patchouli_books"
                | "journeymap"
                | "fancymenu"
                | "datapacks"
        )
    };
    if !allowed {
        return Err(PackError::InvalidPath);
    }
    Ok(path.to_owned())
}

fn archive_name(path: &str) -> Result<(), PackError> {
    // Directory entries have one trailing slash; all components still use
    // the same platform-independent segment rules.
    let core = path.strip_suffix('/').unwrap_or(path);
    if core.is_empty() || core.len() > 600 || core.starts_with('/') || core.contains('\\') {
        return Err(PackError::InvalidPath);
    }
    if core
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == ".." || part.contains(':'))
    {
        return Err(PackError::InvalidPath);
    }
    Ok(())
}

fn read_bounded<R: Read>(reader: &mut R, limit: u64) -> Result<Vec<u8>, PackError> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PackError::InvalidArchive)?;
    if bytes.len() as u64 > limit {
        return Err(PackError::ArchiveLimit);
    }
    Ok(bytes)
}

fn safe_download(raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    crate::downloads::mrpack_allowed_url(&url)
}

pub fn parse_verified_file(path: &Path) -> Result<PackPlan, PackError> {
    let file = File::open(path).map_err(|_| PackError::Io)?;
    if file.metadata().map_err(|_| PackError::Io)?.len() > MAX_PACK_BYTES {
        return Err(PackError::ArchiveLimit);
    }
    parse_reader(file)
}

/// Reopen only the planned entry of a verified archive. The second read is
/// bounded and checked against the digest fixed during planning, so changed
/// bytes or archive indices cannot turn a preview into different overrides.
pub fn read_override(path: &Path, planned: &OverrideFile) -> Result<Vec<u8>, PackError> {
    let file = File::open(path).map_err(|_| PackError::Io)?;
    if file.metadata().map_err(|_| PackError::Io)?.len() > MAX_PACK_BYTES {
        return Err(PackError::ArchiveLimit);
    }
    let mut zip = ZipArchive::new(file).map_err(|_| PackError::InvalidArchive)?;
    let mut entry = zip
        .by_index(planned.archive_index)
        .map_err(|_| PackError::InvalidArchive)?;
    let expected = match planned.layer {
        OverrideLayer::Common => format!("overrides/{}", planned.path),
        OverrideLayer::Client => format!("client-overrides/{}", planned.path),
    };
    if entry.name() != expected || entry.size() != planned.size {
        return Err(PackError::UnsafeOverride);
    }
    let bytes = read_bounded(&mut entry, MAX_OVERRIDE_BYTES)?;
    if bytes.len() as u64 != planned.size
        || format!("{:x}", Sha256::digest(&bytes)) != planned.sha256
    {
        return Err(PackError::UnsafeOverride);
    }
    Ok(bytes)
}

fn parse_reader<R: Read + Seek>(reader: R) -> Result<PackPlan, PackError> {
    let mut zip = ZipArchive::new(reader).map_err(|_| PackError::InvalidArchive)?;
    if zip.len() > MAX_ENTRIES {
        return Err(PackError::ArchiveLimit);
    }
    let mut names = HashSet::new();
    let mut index_position = None;
    let mut expanded = 0u64;
    let mut archive_overrides = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|_| PackError::InvalidArchive)?;
        let name = entry.name().to_owned();
        archive_name(&name)?;
        let collision_key = name.trim_end_matches('/').to_lowercase();
        if !names.insert(collision_key) {
            return Err(PackError::PathCollision);
        }
        if entry.unix_mode().is_some_and(|mode| {
            let file_type = mode & 0o170000;
            file_type != 0 && file_type != if entry.is_dir() { 0o040000 } else { 0o100000 }
        }) {
            return Err(PackError::UnsafeOverride);
        }
        expanded = expanded
            .checked_add(entry.size())
            .ok_or(PackError::ArchiveLimit)?;
        if expanded > MAX_EXPANDED_BYTES {
            return Err(PackError::ArchiveLimit);
        }
        if name == "modrinth.index.json" {
            if entry.is_dir() || entry.size() > MAX_INDEX_BYTES {
                return Err(PackError::InvalidIndex);
            }
            index_position = Some(i);
        } else if name.starts_with("overrides/") || name.starts_with("client-overrides/") {
            if !entry.is_dir() {
                if entry.size() > MAX_OVERRIDE_BYTES {
                    return Err(PackError::ArchiveLimit);
                }
                archive_overrides.push((i, name, entry.size()));
            }
        } else if name.starts_with("server-overrides/") {
            // Client import ignores server-only override files but validates
            // their archive names and bounds above.
        } else if !matches!(
            name.as_str(),
            "overrides/" | "client-overrides/" | "server-overrides/"
        ) {
            return Err(PackError::InvalidArchive);
        }
    }
    let index_position = index_position.ok_or(PackError::InvalidIndex)?;
    let mut index_entry = zip
        .by_index(index_position)
        .map_err(|_| PackError::InvalidArchive)?;
    let bytes = read_bounded(&mut index_entry, MAX_INDEX_BYTES)?;
    drop(index_entry);
    let index: Index = serde_json::from_slice(&bytes).map_err(|_| PackError::InvalidIndex)?;
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(PackError::UnsupportedFormat);
    }
    if index.version_id.trim().is_empty()
        || index.version_id.len() > 256
        || index.name.trim().is_empty()
        || index.name.len() > 256
        || index.summary.as_ref().is_some_and(|v| v.len() > 4096)
    {
        return Err(PackError::InvalidIndex);
    }
    let minecraft_version = index
        .dependencies
        .get("minecraft")
        .filter(|s| !s.is_empty())
        .ok_or(PackError::InvalidIndex)?
        .clone();
    let fabric_loader_version = index
        .dependencies
        .get("fabric-loader")
        .filter(|s| !s.is_empty())
        .ok_or(PackError::UnsupportedLoader)?
        .clone();
    if index.dependencies.len() != 2
        || minecraft_version.len() > 128
        || fabric_loader_version.len() > 128
    {
        return Err(PackError::UnsupportedLoader);
    }
    let mut destinations = HashSet::new();
    let mut required_files = Vec::new();
    let mut optional_files = Vec::new();
    let mut excluded_paths = Vec::new();
    for file in index.files {
        let path = destination_path(&file.path)?;
        if !destinations.insert(path.to_lowercase()) {
            return Err(PackError::PathCollision);
        }
        if !digest_shape(&file.hashes.sha1, 40) || !digest_shape(&file.hashes.sha512, 128) {
            return Err(PackError::InvalidHash);
        }
        if file.downloads.is_empty()
            || file.downloads.len() > 8
            || !file.downloads.iter().all(|url| safe_download(url))
        {
            return Err(PackError::InvalidDownload);
        }
        if file.file_size > MAX_PACK_BYTES {
            return Err(PackError::ArchiveLimit);
        }
        let client = file.env.map_or(EnvironmentSide::Required, |env| env.client);
        let planned = PackFile {
            path,
            sha1: file.hashes.sha1.to_lowercase(),
            sha512: file.hashes.sha512.to_lowercase(),
            downloads: file.downloads,
            file_size: file.file_size,
            client,
        };
        match client {
            EnvironmentSide::Required => required_files.push(planned),
            EnvironmentSide::Optional => optional_files.push(planned),
            EnvironmentSide::Unsupported => excluded_paths.push(planned.path),
        }
    }
    let mut overrides = BTreeMap::new();
    for (i, name, size) in archive_overrides {
        let (layer, relative) = if let Some(rest) = name.strip_prefix("client-overrides/") {
            (OverrideLayer::Client, rest)
        } else {
            (
                OverrideLayer::Common,
                name.strip_prefix("overrides/")
                    .ok_or(PackError::UnsafeOverride)?,
            )
        };
        let path = destination_path(relative).map_err(|_| PackError::UnsafeOverride)?;
        if destinations.contains(&path.to_lowercase()) {
            return Err(PackError::PathCollision);
        }
        let mut entry = zip.by_index(i).map_err(|_| PackError::InvalidArchive)?;
        let bytes = read_bounded(&mut entry, MAX_OVERRIDE_BYTES)?;
        if bytes.len() as u64 != size {
            return Err(PackError::InvalidArchive);
        }
        let override_file = OverrideFile {
            path: path.clone(),
            archive_index: i,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            size,
            layer,
        };
        let key = path.to_lowercase();
        if let Some(previous) = overrides.get(&key) {
            let previous: &OverrideFile = previous;
            if previous.layer == layer {
                return Err(PackError::PathCollision);
            }
            if layer == OverrideLayer::Common {
                return Err(PackError::PathCollision);
            }
        }
        overrides.insert(key, override_file);
    }
    Ok(PackPlan {
        name: index.name,
        pack_version: index.version_id,
        minecraft_version,
        fabric_loader_version,
        required_files,
        optional_files,
        excluded_paths,
        overrides: overrides.into_values().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn pack(index: &str, extras: &[(&str, &[u8])]) -> Result<PackPlan, PackError> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            zip.start_file(
                "modrinth.index.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(index.as_bytes()).unwrap();
            for (name, bytes) in extras {
                zip.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(bytes).unwrap();
            }
            zip.finish().unwrap();
        }
        parse_reader(Cursor::new(buffer.into_inner()))
    }

    fn index(files: &str) -> String {
        format!(
            r#"{{"formatVersion":1,"game":"minecraft","versionId":"v1","name":"Pack","files":[{files}],"dependencies":{{"minecraft":"1.21.1","fabric-loader":"0.16.0"}}}}"#
        )
    }

    fn file(path: &str, client: &str) -> String {
        format!(
            r#"{{"path":"{path}","hashes":{{"sha1":"{}","sha512":"{}"}},"downloads":["https://cdn.modrinth.com/file.jar"],"fileSize":4,"env":{{"client":"{client}","server":"required"}}}}"#,
            "a".repeat(40),
            "b".repeat(128)
        )
    }

    #[test]
    fn parses_format_one_and_client_semantics() {
        let files = [
            file("mods/a.jar", "required"),
            file("mods/b.jar", "optional"),
            file("mods/c.jar", "unsupported"),
        ]
        .join(",");
        let plan = pack(
            &index(&files),
            &[
                ("overrides/config/a.toml", b"first"),
                ("client-overrides/config/a.toml", b"client"),
            ],
        )
        .unwrap();
        assert_eq!(plan.minecraft_version, "1.21.1");
        assert_eq!(plan.required_files.len(), 1);
        assert_eq!(plan.optional_files.len(), 1);
        assert_eq!(plan.excluded_paths, ["mods/c.jar"]);
        assert_eq!(plan.overrides.len(), 1);
        assert_eq!(plan.overrides[0].layer, OverrideLayer::Client);
    }

    #[test]
    fn rejects_paths_collisions_and_unsupported_loader() {
        for path in [
            "../mods/a.jar",
            "/mods/a.jar",
            "C:/mods/a.jar",
            "mods\\..\\a.jar",
            "content-managed.json",
            "mods/CON.jar",
        ] {
            assert!(
                pack(&index(&file(path, "required")), &[]).is_err(),
                "{path}"
            );
        }
        let files = [
            file("mods/A.jar", "required"),
            file("mods/a.jar", "required"),
        ]
        .join(",");
        assert!(matches!(
            pack(&index(&files), &[]),
            Err(PackError::PathCollision)
        ));
        let forge = index("").replace("fabric-loader", "forge");
        assert!(matches!(
            pack(&forge, &[]),
            Err(PackError::UnsupportedLoader)
        ));
    }

    #[test]
    fn rejects_archive_traversal_and_component_override_collision() {
        assert!(matches!(
            pack(&index(""), &[("overrides/../config/a", b"x")]),
            Err(PackError::InvalidPath)
        ));
        assert!(matches!(
            pack(
                &index(&file("mods/a.jar", "required")),
                &[("overrides/mods/a.jar", b"x")]
            ),
            Err(PackError::PathCollision)
        ));
    }

    #[test]
    fn rejects_unsupported_and_malformed_indexes() {
        let valid = index("");
        assert!(matches!(
            pack(
                &valid.replace("\"formatVersion\":1", "\"formatVersion\":2"),
                &[]
            ),
            Err(PackError::UnsupportedFormat)
        ));
        assert!(matches!(
            pack("{not json", &[]),
            Err(PackError::InvalidIndex)
        ));
        assert!(matches!(
            pack(&valid.replace("\"name\":\"Pack\",", ""), &[]),
            Err(PackError::InvalidIndex)
        ));
        assert!(matches!(
            pack(&"x".repeat((MAX_INDEX_BYTES + 1) as usize), &[]),
            Err(PackError::InvalidIndex)
        ));
    }

    #[test]
    fn rejects_duplicate_archive_names_and_unsafe_urls() {
        assert!(matches!(
            pack(
                &index(""),
                &[
                    ("overrides/config/a.toml", b"a"),
                    ("overrides/config/A.toml", b"b")
                ]
            ),
            Err(PackError::PathCollision)
        ));
        let unsafe_url = index(&file("mods/a.jar", "required")).replace(
            "https://cdn.modrinth.com/file.jar",
            "https://cdn.modrinth.com.evil.invalid/file.jar",
        );
        assert!(matches!(
            pack(&unsafe_url, &[]),
            Err(PackError::InvalidDownload)
        ));
        let missing_hash = index(&file("mods/a.jar", "required")).replace(&"b".repeat(128), "bad");
        assert!(matches!(
            pack(&missing_hash, &[]),
            Err(PackError::InvalidHash)
        ));
    }

    #[test]
    fn rejects_override_escape_and_oversized_entries() {
        for path in [
            "overrides/C:/config/a",
            "client-overrides/config\\..\\a",
            "overrides/pack-installed.json",
        ] {
            assert!(pack(&index(""), &[(path, b"x")]).is_err(), "{path}");
        }
        let too_large = vec![0u8; (MAX_OVERRIDE_BYTES + 1) as usize];
        assert!(matches!(
            pack(&index(""), &[("overrides/config/large.bin", &too_large)]),
            Err(PackError::ArchiveLimit)
        ));
    }
}
