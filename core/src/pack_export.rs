//! Reviewed Modrinth `.mrpack` export for a managed instance.
//!
//! References verified Modrinth-managed files by their official SHA-1/SHA-512
//! hashes and download URLs, packages user-selected local overrides (`mods/`,
//! `config/`, `resourcepacks/`, `shaderpacks/`, `options.txt`), and explicitly
//! excludes sensitive or noisy instance data (`logs/`, `crash-reports/`,
//! `saves/`, `screenshots/`, `servers.dat`, `game/`, credentials, and
//! `.install-*` internal files) by default.

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha1::{Digest as _, Sha1};
use sha2::Sha512;

use crate::instance_content::{self, ContentType};
use crate::instances::{InstanceId, InstanceRegistry, platform::PlatformPin};
use crate::paths::ManagedPaths;

const MAX_EXPORT_OVERRIDES_FILES: usize = 10_000;
const MAX_EXPORT_OVERRIDES_BYTES: u64 = 512 * 1024 * 1024; // 512 MiB

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportOptions {
    pub pack_name: String,
    pub pack_version: String,
    pub summary: String,
    pub include_untracked_mods: bool,
    pub include_config: bool,
    pub include_resource_packs: bool,
    pub include_shader_packs: bool,
    pub include_options_txt: bool,
    pub include_servers_dat: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            pack_name: String::new(),
            pack_version: "1.0.0".into(),
            summary: String::new(),
            include_untracked_mods: true,
            include_config: true,
            include_resource_packs: true,
            include_shader_packs: true,
            include_options_txt: false,
            include_servers_dat: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreview {
    pub instance_name: String,
    pub minecraft_version: String,
    pub loader_kind: String,
    pub loader_version: Option<String>,
    pub referenced_modrinth_files: usize,
    pub override_files_count: usize,
    pub override_bytes: u64,
    pub excluded_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReceipt {
    pub destination: String,
    pub size_bytes: u64,
    pub referenced_files: usize,
    pub override_files: usize,
}

#[derive(Debug)]
pub struct ExportError(pub &'static str, pub String);

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

impl std::error::Error for ExportError {}

fn failed(err: impl std::fmt::Display) -> ExportError {
    ExportError(
        "pack_export_failed",
        format!("Could not export modpack: {err}"),
    )
}

fn is_sensitive_or_excluded_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".log")
        || lower.ends_with(".log.gz")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.starts_with("aurora-") && lower.ends_with(".jar")
}

fn loader_dependency(pin: &PlatformPin) -> Option<(&'static str, String)> {
    match pin {
        PlatformPin::Vanilla {} => None,
        PlatformPin::Fabric { version } => Some(("fabric-loader", version.clone())),
        PlatformPin::Forge { version } => Some(("forge", version.clone())),
        PlatformPin::NeoForge { version } => Some(("neoforge", version.clone())),
        PlatformPin::Quilt { version } => Some(("quilt-loader", version.clone())),
    }
}

struct CollectedOverride {
    archive_path: String,
    source_path: std::path::PathBuf,
    size: u64,
}

fn collect_dir_overrides(
    base: &Path,
    prefix: &str,
    skip_files: &HashSet<String>,
    out: &mut Vec<CollectedOverride>,
    total_bytes: &mut u64,
) -> Result<(), ExportError> {
    if !base.exists() {
        return Ok(());
    }
    fn walk(
        root: &Path,
        current: &Path,
        prefix: &str,
        skip_files: &HashSet<String>,
        out: &mut Vec<CollectedOverride>,
        total_bytes: &mut u64,
    ) -> Result<(), ExportError> {
        let meta = fs::symlink_metadata(current).map_err(failed)?;
        if meta.file_type().is_symlink() {
            return Err(ExportError(
                "pack_export_unsafe",
                "Symlinked files inside the instance cannot be exported.".into(),
            ));
        }
        for entry in fs::read_dir(current).map_err(failed)? {
            let entry = entry.map_err(failed)?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(failed)?;
            if meta.file_type().is_symlink() {
                return Err(ExportError(
                    "pack_export_unsafe",
                    "Symlinked files inside the instance cannot be exported.".into(),
                ));
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if is_sensitive_or_excluded_name(&file_name) {
                continue;
            }
            if meta.is_dir() {
                walk(root, &path, prefix, skip_files, out, total_bytes)?;
            } else if meta.is_file() {
                let rel = path
                    .strip_prefix(root)
                    .map_err(failed)?
                    .to_string_lossy()
                    .replace('\\', "/");
                let full_rel = format!("{prefix}/{rel}");
                if skip_files.contains(&full_rel) {
                    continue;
                }
                if out.len() >= MAX_EXPORT_OVERRIDES_FILES
                    || total_bytes.saturating_add(meta.len()) > MAX_EXPORT_OVERRIDES_BYTES
                {
                    return Err(ExportError(
                        "pack_export_limit",
                        "Selected overrides exceed the 10,000-file / 512 MiB export limit.".into(),
                    ));
                }
                *total_bytes = total_bytes.saturating_add(meta.len());
                out.push(CollectedOverride {
                    archive_path: format!("overrides/{full_rel}"),
                    source_path: path,
                    size: meta.len(),
                });
            }
        }
        Ok(())
    }
    walk(base, base, prefix, skip_files, out, total_bytes)
}

#[derive(Serialize)]
struct MrpackFile {
    path: String,
    hashes: BTreeMap<String, String>,
    env: BTreeMap<String, String>,
    downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    file_size: u64,
}

fn gather_export_plan(
    managed: &ManagedPaths,
    id: &InstanceId,
    options: &ExportOptions,
) -> Result<
    (
        ExportPreview,
        Vec<MrpackFile>,
        Vec<CollectedOverride>,
        BTreeMap<String, String>,
    ),
    ExportError,
> {
    let registry = InstanceRegistry::load(&managed.instance_registry_file()).map_err(failed)?;
    let record = registry.find(id).ok_or_else(|| {
        ExportError(
            "instance_not_found",
            "The instance no longer exists.".into(),
        )
    })?;
    let paths = managed.instance_paths(id);

    let mut dependencies = BTreeMap::new();
    dependencies.insert(
        "minecraft".to_owned(),
        record.installed().minecraft_version.clone(),
    );
    let (loader_kind, loader_version) = match loader_dependency(&record.installed().platform) {
        Some((k, v)) => {
            dependencies.insert(k.to_owned(), v.clone());
            (k.to_owned(), Some(v))
        }
        None => ("vanilla".to_owned(), None),
    };

    // Collect tracked Modrinth files whose local file is present & enabled.
    let mut tracked_paths = HashSet::new();
    let mut referenced = Vec::new();
    let state_path = paths.root().join("provider-content.json");
    if let Ok(text) = fs::read_to_string(&state_path)
        && let Ok(state) = instance_content::ContentState::from_json(&text)
    {
        for rec in &state.entries {
            if rec.provider != "modrinth" {
                continue;
            }
            let folder = match rec.content_type {
                ContentType::Mod => "mods",
                ContentType::ResourcePack => "resourcepacks",
                ContentType::ShaderPack => "shaderpacks",
            };
            let local_path = paths.root().join(folder).join(&rec.file_name);
            if !local_path.is_file() {
                continue;
            }
            let Ok(bytes) = fs::read(&local_path) else {
                continue;
            };
            let sha1 = format!("{:x}", Sha1::digest(&bytes));
            let sha512 = format!("{:x}", Sha512::digest(&bytes));
            let rel_path = format!("{folder}/{}", rec.file_name);
            let encoded_file = url::form_urlencoded::byte_serialize(rec.file_name.as_bytes())
                .collect::<String>()
                .replace('+', "%20");
            let cdn_url = format!(
                "https://cdn.modrinth.com/data/{}/versions/{}/{}",
                rec.project_id, rec.version_id, encoded_file
            );
            tracked_paths.insert(rel_path.clone());
            let mut hashes = BTreeMap::new();
            hashes.insert("sha1".into(), sha1);
            hashes.insert("sha512".into(), sha512);
            let mut env = BTreeMap::new();
            env.insert("client".into(), "required".into());
            env.insert("server".into(), "unsupported".into());
            referenced.push(MrpackFile {
                path: rel_path,
                hashes,
                env,
                downloads: vec![cdn_url],
                file_size: bytes.len() as u64,
            });
        }
    }

    let mut overrides = Vec::new();
    let mut override_bytes = 0u64;
    if options.include_untracked_mods {
        collect_dir_overrides(
            paths.mods(),
            "mods",
            &tracked_paths,
            &mut overrides,
            &mut override_bytes,
        )?;
    }
    if options.include_config {
        collect_dir_overrides(
            paths.config(),
            "config",
            &tracked_paths,
            &mut overrides,
            &mut override_bytes,
        )?;
    }
    if options.include_resource_packs {
        collect_dir_overrides(
            paths.resourcepacks(),
            "resourcepacks",
            &tracked_paths,
            &mut overrides,
            &mut override_bytes,
        )?;
    }
    if options.include_shader_packs {
        collect_dir_overrides(
            paths.shaderpacks(),
            "shaderpacks",
            &tracked_paths,
            &mut overrides,
            &mut override_bytes,
        )?;
    }
    if options.include_options_txt {
        let p = paths.root().join("options.txt");
        if p.is_file() {
            let len = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            override_bytes = override_bytes.saturating_add(len);
            overrides.push(CollectedOverride {
                archive_path: "overrides/options.txt".into(),
                source_path: p,
                size: len,
            });
        }
    }
    if options.include_servers_dat {
        let p = paths.root().join("servers.dat");
        if p.is_file() {
            let len = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            override_bytes = override_bytes.saturating_add(len);
            overrides.push(CollectedOverride {
                archive_path: "overrides/servers.dat".into(),
                source_path: p,
                size: len,
            });
        }
    }

    let excluded_notes = vec![
        "Singleplayer saves, logs, crash reports, screenshots, and credentials are always excluded.".into(),
        "Aurora Client bootstrap JARs are excluded (recipients install compatible Aurora Client from official releases).".into(),
    ];

    let preview = ExportPreview {
        instance_name: record.display_name().to_owned(),
        minecraft_version: record.installed().minecraft_version.clone(),
        loader_kind,
        loader_version,
        referenced_modrinth_files: referenced.len(),
        override_files_count: overrides.len(),
        override_bytes,
        excluded_notes,
    };

    Ok((preview, referenced, overrides, dependencies))
}

pub fn preview_export(
    managed: &ManagedPaths,
    id: &InstanceId,
    options: &ExportOptions,
) -> Result<ExportPreview, ExportError> {
    let (preview, _, _, _) = gather_export_plan(managed, id, options)?;
    Ok(preview)
}

pub fn export_mrpack(
    managed: &ManagedPaths,
    id: &InstanceId,
    options: &ExportOptions,
    destination: &Path,
) -> Result<ExportReceipt, ExportError> {
    if !destination.is_absolute() {
        return Err(ExportError(
            "pack_export_invalid_destination",
            "Choose an absolute destination file path ending in .mrpack.".into(),
        ));
    }
    let (preview, referenced, overrides, dependencies) = gather_export_plan(managed, id, options)?;
    let pack_name = if options.pack_name.trim().is_empty() {
        preview.instance_name.clone()
    } else {
        options.pack_name.trim().to_owned()
    };
    let pack_version = if options.pack_version.trim().is_empty() {
        "1.0.0".to_owned()
    } else {
        options.pack_version.trim().to_owned()
    };

    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": pack_version,
        "name": pack_name,
        "summary": options.summary.trim(),
        "files": referenced,
        "dependencies": dependencies,
    });

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(failed)?;
    }
    let tmp = destination.with_extension(format!("mrpack.tmp-{}", uuid::Uuid::new_v4().simple()));
    let write_res = (|| -> Result<(), ExportError> {
        let file = File::create(&tmp).map_err(failed)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("modrinth.index.json", opts)
            .map_err(failed)?;
        let bytes = serde_json::to_vec_pretty(&index).map_err(failed)?;
        zip.write_all(&bytes).map_err(failed)?;
        for item in &overrides {
            zip.start_file(&item.archive_path, opts).map_err(failed)?;
            let mut src = File::open(&item.source_path).map_err(failed)?;
            io::copy(&mut src, &mut zip).map_err(failed)?;
        }
        zip.finish().map_err(failed)?;
        Ok(())
    })();
    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    fs::rename(&tmp, destination).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        failed(e)
    })?;
    let size_bytes = fs::metadata(destination).map(|m| m.len()).unwrap_or(0);
    Ok(ExportReceipt {
        destination: destination.display().to_string(),
        size_bytes,
        referenced_files: preview.referenced_modrinth_files,
        override_files: overrides.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::{
        InstanceRecord, InstanceState, generate_instance_id,
        platform::InstalledConfiguration,
        settings::{InstanceConfiguration, LoaderConfiguration},
    };

    #[test]
    fn export_creates_valid_mrpack_and_excludes_sensitive_files() {
        let root =
            std::env::temp_dir().join(format!("aurora-export-test-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let id = generate_instance_id();
        let p = managed.instance_paths(&id);
        fs::create_dir_all(p.mods()).unwrap();
        fs::create_dir_all(p.config()).unwrap();
        fs::write(p.mods().join("custom.jar"), b"jar").unwrap();
        fs::write(p.mods().join("aurora-3.7.0.jar"), b"aurora-excluded").unwrap();
        fs::write(p.config().join("sodium.json"), b"{}").unwrap();
        fs::write(p.config().join("secret-token.txt"), b"nope").unwrap();

        let mut cfg = InstanceConfiguration::from_parts(
            "1.21.11",
            LoaderConfiguration::Fabric {
                policy: crate::instances::settings::LoaderPolicy::Pinned {
                    version: "0.16.0".into(),
                },
            },
            2048,
            String::new(),
            None,
        );
        cfg.set_aurora_enabled(false);
        let record = InstanceRecord::from_installed(
            id.clone(),
            "My Pack",
            InstanceState::Ready,
            InstalledConfiguration {
                minecraft_version: "1.21.11".into(),
                platform: PlatformPin::Fabric {
                    version: "0.16.0".into(),
                },
                aurora: None,
            },
            cfg,
        )
        .unwrap();
        let mut reg = InstanceRegistry::empty();
        reg.instances_mut().push(record);
        reg.save(&managed.instance_registry_file()).unwrap();

        let out_file = root.join("exports/my-pack.mrpack");
        let opts = ExportOptions::default();
        let receipt = export_mrpack(&managed, &id, &opts, &out_file).unwrap();
        assert_eq!(receipt.override_files, 2); // custom.jar + sodium.json
        assert!(out_file.is_file());

        // Verify the exported file parses with Aurora's own strict mrpack parser!
        let parsed = crate::mrpack::parse_verified_file(&out_file).unwrap();
        assert_eq!(parsed.minecraft_version, "1.21.11");
        assert_eq!(parsed.name, "My Pack");

        let _ = fs::remove_dir_all(&root);
    }
}
