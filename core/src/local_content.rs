//! Reviewed local JAR/resource-pack/shader import. No extraction, network,
//! implicit overwrite, provider adoption or arbitrary destination paths.
use crate::instance_content::{self as content, ContentError, ContentType};
use crate::instances::{InstanceId, InstanceRegistry, InstanceState};
use crate::integrity::{ArtifactDigest, verify_file};
use crate::paths::ManagedPaths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_BYTES: u64 = 512 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub ticket: String,
    pub fingerprint: String,
    pub instance_name: String,
    pub file_name: String,
    pub content_type: ContentType,
    pub size_bytes: u64,
    pub sha256: String,
    pub blockers: Vec<String>,
    pub note: String,
}
#[derive(Clone)]
struct Ticket {
    root: PathBuf,
    source: PathBuf,
    instance: InstanceId,
    preview: Preview,
    expires: std::time::Instant,
}
fn tickets() -> &'static Mutex<HashMap<String, Ticket>> {
    static TICKETS: OnceLock<Mutex<HashMap<String, Ticket>>> = OnceLock::new();
    TICKETS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn invalid(message: &str) -> ContentError {
    ContentError::UnsupportedActionWith(message.into())
}
/// Absolute regular file, no symlink/reparse ancestor, 1 byte – 512 MiB.
/// Shared with the local `.mrpack` recognizer.
pub(crate) fn safe_source(path: &Path) -> Result<u64, ContentError> {
    if !path.is_absolute() {
        return Err(ContentError::UnsafePath);
    }
    for ancestor in path.ancestors() {
        let meta = std::fs::symlink_metadata(ancestor).map_err(ContentError::Io)?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err(ContentError::UnsafePath);
            }
        }
        if meta.file_type().is_symlink() {
            return Err(ContentError::UnsafePath);
        }
    }
    let meta = std::fs::metadata(path).map_err(ContentError::Io)?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_BYTES {
        return Err(invalid("Choose a regular supported archive up to 512 MiB."));
    }
    Ok(meta.len())
}

fn inspect(path: &Path) -> Result<ContentType, ContentError> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).map_err(ContentError::Io)?)
        .map_err(|_| invalid("The selected file is not a valid ZIP/JAR archive."))?;
    if archive.len() > 4096 {
        return Err(invalid("The archive has too many entries."));
    }
    let mut names = HashSet::new();
    let mut mod_metadata = false;
    let mut resource = false;
    let mut shader = false;
    let mut total = 0u64;
    for index in 0..archive.len() {
        crate::content_jobs::checkpoint().map_err(|_| ContentError::Cancelled)?;
        let mut entry = archive
            .by_index(index)
            .map_err(|_| ContentError::InvalidProviderArtifact)?;
        let declared_size = entry.size();
        let mut consumed = 0;
        let name = entry.name().to_owned();
        let relative = name.strip_suffix('/').unwrap_or(&name);
        if relative.is_empty()
            || relative
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || relative
                .chars()
                .any(|c| c == '\\' || c == ':' || c.is_control())
            || relative.starts_with('/')
            || !names.insert(name.to_lowercase())
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(ContentError::UnsafePath);
        }
        if name == "modrinth.index.json" {
            return Err(invalid(
                "Local .mrpack import is not supported by this build. Use the exact hosted Modrinth pack review.",
            ));
        }
        total = total
            .checked_add(entry.size())
            .ok_or(ContentError::InvalidProviderArtifact)?;
        if entry.size() > 128 * 1024 * 1024 || total > 2 * 1024 * 1024 * 1024 {
            return Err(invalid("The archive exceeds safe unpacked-size limits."));
        }
        mod_metadata |= matches!(
            name.as_str(),
            "fabric.mod.json" | "META-INF/mods.toml" | "META-INF/neoforge.mods.toml"
        );
        shader |= name.starts_with("shaders/") && !entry.is_dir();
        if name == "pack.mcmeta" {
            if entry.size() > 256 * 1024 {
                return Err(ContentError::InvalidProviderArtifact);
            }
            let mut bytes = Vec::new();
            entry
                .by_ref()
                .take(256 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(ContentError::Io)?;
            consumed = bytes.len() as u64;
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|_| ContentError::InvalidProviderArtifact)?;
            resource = value["pack"].is_object();
        }
        // Fully read bounded entries so corrupt compressed data/CRC is rejected.
        let copied = std::io::copy(&mut entry.take(declared_size + 1), &mut std::io::sink())
            .map_err(ContentError::Io)?;
        if consumed + copied != declared_size {
            return Err(ContentError::InvalidProviderArtifact);
        }
    }
    match (mod_metadata, resource, shader) {
        (true, false, false) => Ok(ContentType::Mod),
        (false, true, false) => Ok(ContentType::ResourcePack),
        (false, _, true) => Ok(ContentType::ShaderPack),
        _ => Err(invalid(
            "The archive has no unambiguous supported mod, resource-pack or shader content.",
        )),
    }
}

fn review(
    managed: &ManagedPaths,
    instance: &InstanceId,
    source: &Path,
) -> Result<Preview, ContentError> {
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|e| ContentError::StateMalformed(e.to_string()))?;
    let record = registry.find(instance).ok_or(ContentError::UnsafePath)?;
    if record.state() != InstanceState::Ready {
        return Err(invalid("Finish installing the target instance first."));
    }
    let size = safe_source(source)?;
    let kind = inspect(source)?;
    let name = source
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(ContentError::UnsafePath)?;
    content::validate_file_name(name)?;
    let extension = if kind == ContentType::Mod {
        ".jar"
    } else {
        ".zip"
    };
    if !name.to_ascii_lowercase().ends_with(extension) {
        return Err(invalid(
            "The validated content must also use the appropriate .jar or .zip filename.",
        ));
    }
    let mut hash = Sha256::new();
    let mut file = std::fs::File::open(source).map_err(ContentError::Io)?;
    let mut buffer = [0u8; 65536];
    let mut read = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(ContentError::Io)?;
        if n == 0 {
            break;
        }
        read += n as u64;
        if read > MAX_BYTES {
            return Err(ContentError::InvalidProviderArtifact);
        }
        hash.update(&buffer[..n]);
    }
    if read != size {
        return Err(ContentError::ChangedSinceScan);
    }
    let digest = format!("{:x}", hash.finalize());
    let directory = content::validate_directory(managed, instance, kind)?;
    let mut blockers = Vec::new();
    if directory.exists()
        && std::fs::read_dir(&directory)
            .map_err(ContentError::Io)?
            .any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)))
    {
        blockers.push("A file with that name already exists. Nothing will be overwritten.".into());
    }
    if let Err(error) =
        content::validate_local_import(managed, instance, kind, name, &digest, source, size)
    {
        blockers.push(error.to_string());
    }
    let revision = content::local_inventory_revision(managed, instance)?;
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(record, kind, name, size, &digest, revision, &blockers))
                .map_err(|_| ContentError::InvalidProviderArtifact)?
        )
    );
    Ok(Preview { ticket: String::new(), fingerprint, instance_name: record.display_name().into(), file_name:name.into(), content_type:kind, size_bytes:size, sha256:digest, blockers,
        note:"Installs the reviewed local bytes as user-managed content. Publisher authenticity is not established. Mods must satisfy the current loader/dependency checks; resource-pack format and shader activation are selected in Minecraft.".into() })
}

pub fn preview(
    managed: &ManagedPaths,
    instance: InstanceId,
    source: PathBuf,
) -> Result<Preview, ContentError> {
    let mut preview =
        content::with_instance_lock(&instance, || review(managed, &instance, &source))?;
    preview.ticket = uuid::Uuid::new_v4().to_string();
    let mut tickets = tickets().lock().expect("local import tickets");
    tickets.retain(|_, t| t.expires > std::time::Instant::now());
    if tickets.len() >= 16 {
        return Err(invalid(
            "Close or finish the existing local import reviews first.",
        ));
    }
    tickets.insert(
        preview.ticket.clone(),
        Ticket {
            root: managed.data_root().to_path_buf(),
            source,
            instance,
            preview: preview.clone(),
            expires: std::time::Instant::now() + std::time::Duration::from_secs(1800),
        },
    );
    Ok(preview)
}

pub async fn install(
    managed: &ManagedPaths,
    instance: &InstanceId,
    ticket_id: &str,
    fingerprint: &str,
) -> Result<(), ContentError> {
    let ticket = tickets()
        .lock()
        .expect("local import tickets")
        .get(ticket_id)
        .cloned()
        .ok_or(ContentError::ChangedSinceScan)?;
    if ticket.root != managed.data_root()
        || &ticket.instance != instance
        || ticket.preview.fingerprint != fingerprint
        || ticket.expires < std::time::Instant::now()
    {
        return Err(ContentError::ChangedSinceScan);
    }
    let fresh =
        content::with_instance_lock(instance, || review(managed, instance, &ticket.source))?;
    if fresh.fingerprint != fingerprint || !fresh.blockers.is_empty() {
        return Err(ContentError::ChangedSinceScan);
    }
    let digest = ArtifactDigest::parse(&fresh.sha256).map_err(|_| ContentError::HashMismatch)?;
    let artifact = crate::cache::ArtifactCache::new(managed.clone())
        .import_local(&ticket.source, &digest, fresh.size_bytes)
        .await
        .map_err(|e| ContentError::Acquisition(e.to_string()))?;
    if inspect(&artifact.path)? != fresh.content_type {
        return Err(ContentError::InvalidProviderArtifact);
    }
    content::with_instance_lock(instance, || {
        let fresh = review(managed, instance, &ticket.source)?;
        if fresh.fingerprint != fingerprint || !fresh.blockers.is_empty() {
            return Err(ContentError::ChangedSinceScan);
        }
        let _commit = crate::content_jobs::commit().map_err(|_| ContentError::Cancelled)?;
        let directory = content::ensure_directory(managed, instance, fresh.content_type)?;
        let target = directory.join(&fresh.file_name);
        let temporary = directory.join(format!(".local-import-{}", uuid::Uuid::new_v4()));
        let result = (|| {
            std::fs::copy(&artifact.path, &temporary).map_err(ContentError::Io)?;
            verify_file(&temporary, &digest, Some(fresh.size_bytes))
                .map_err(|_| ContentError::HashMismatch)?;
            content::validate_local_import(
                managed,
                instance,
                fresh.content_type,
                &fresh.file_name,
                &fresh.sha256,
                &temporary,
                fresh.size_bytes,
            )?;
            std::fs::hard_link(&temporary, &target).map_err(|e| {
                if target.exists() {
                    ContentError::Collision
                } else {
                    ContentError::Io(e)
                }
            })?;
            Ok(())
        })();
        let _ = std::fs::remove_file(&temporary);
        result
    })?;
    tickets()
        .lock()
        .expect("local import tickets")
        .remove(ticket_id);
    Ok(())
}

pub fn discard(
    managed: &ManagedPaths,
    instance: &InstanceId,
    ticket_id: &str,
) -> Result<(), ContentError> {
    let mut tickets = tickets().lock().expect("local import tickets");
    if let Some(ticket) = tickets.get(ticket_id) {
        if ticket.root != managed.data_root() || &ticket.instance != instance {
            return Err(ContentError::ChangedSinceScan);
        }
        tickets.remove(ticket_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn archive(path: &Path, name: &str, bytes: &[u8]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
        zip.finish().unwrap();
    }
    fn fixture() -> (ManagedPaths, InstanceId) {
        use crate::instances::{
            InstanceRecord,
            platform::{InstalledConfiguration, PlatformPin},
            settings::InstanceConfiguration,
        };
        let root =
            std::env::temp_dir().join(format!("aurora-native9-import-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root).unwrap();
        let instance = crate::instances::generate_instance_id();
        std::fs::create_dir_all(managed.instance_paths(&instance).root()).unwrap();
        std::fs::create_dir(managed.instance_paths(&instance).mods()).unwrap();
        let mut configuration = InstanceConfiguration::for_minecraft_version("1.21.11");
        configuration.set_aurora_enabled(false);
        let record = InstanceRecord::from_installed(
            instance.clone(),
            "Synthetic import",
            InstanceState::Ready,
            InstalledConfiguration {
                minecraft_version: "1.21.11".into(),
                platform: PlatformPin::Fabric {
                    version: "0.19.5".into(),
                },
                aurora: None,
            },
            configuration,
        )
        .unwrap();
        let mut registry = InstanceRegistry::empty();
        registry.instances_mut().push(record);
        registry.save(&managed.instance_registry_file()).unwrap();
        (managed, instance)
    }
    #[tokio::test]
    async fn native9_local_mod_resource_and_shader_round_trip_without_provider_adoption() {
        for (name, entry, bytes, kind) in [
            (
                "fixture.jar",
                "fabric.mod.json",
                br#"{"schemaVersion":1,"id":"native9fixture","version":"1"}"#.as_slice(),
                ContentType::Mod,
            ),
            (
                "resource.zip",
                "pack.mcmeta",
                br#"{"pack":{"pack_format":75,"description":"Synthetic"}}"#.as_slice(),
                ContentType::ResourcePack,
            ),
            (
                "shader.zip",
                "shaders/basic.fsh",
                b"void main(){}".as_slice(),
                ContentType::ShaderPack,
            ),
        ] {
            let (managed, instance) = fixture();
            let source = managed.data_root().join(name);
            archive(&source, entry, bytes);
            let plan = preview(&managed, instance.clone(), source.clone()).unwrap();
            assert_eq!(plan.content_type, kind);
            assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
            install(&managed, &instance, &plan.ticket, &plan.fingerprint)
                .await
                .unwrap();
            let target = managed
                .instance_paths(&instance)
                .root()
                .join(kind.directory_name())
                .join(name);
            assert_eq!(
                std::fs::read(&source).unwrap(),
                std::fs::read(&target).unwrap()
            );
            assert!(
                content::ContentState::load(&managed, &instance)
                    .unwrap()
                    .entries
                    .is_empty()
            );
            let conflict = preview(&managed, instance.clone(), source).unwrap();
            assert!(!conflict.blockers.is_empty());
            assert!(
                install(&managed, &instance, &conflict.ticket, &conflict.fingerprint)
                    .await
                    .is_err()
            );
        }
    }
    #[tokio::test]
    async fn native9_local_import_refuses_stale_bytes_and_cancelled_activation() {
        let (managed, instance) = fixture();
        let source = managed.data_root().join("resource.zip");
        archive(&source, "pack.mcmeta", br#"{"pack":{"pack_format":75}}"#);
        let plan = preview(&managed, instance.clone(), source.clone()).unwrap();
        archive(&source, "pack.mcmeta", br#"{"pack":{"pack_format":76}}"#);
        assert!(
            install(&managed, &instance, &plan.ticket, &plan.fingerprint)
                .await
                .is_err()
        );
        let plan = preview(&managed, instance.clone(), source.clone()).unwrap();
        let control = crate::content_jobs::Control::default();
        control.cancel();
        assert!(
            control
                .run(install(
                    &managed,
                    &instance,
                    &plan.ticket,
                    &plan.fingerprint
                ))
                .await
                .is_err()
        );
        assert!(
            !managed
                .instance_paths(&instance)
                .root()
                .join("resourcepacks/resource.zip")
                .exists()
        );
        install(&managed, &instance, &plan.ticket, &plan.fingerprint)
            .await
            .unwrap();
    }
    #[test]
    fn native9_review_discard_is_scoped_to_data_root_and_instance() {
        let (managed, instance) = fixture();
        let source = managed.data_root().join("resource.zip");
        archive(&source, "pack.mcmeta", br#"{"pack":{"pack_format":75}}"#);
        let plan = preview(&managed, instance.clone(), source).unwrap();
        let (other, _) = fixture();
        assert!(discard(&other, &instance, &plan.ticket).is_err());
        assert!(
            discard(
                &managed,
                &crate::instances::generate_instance_id(),
                &plan.ticket
            )
            .is_err()
        );
        assert!(tickets().lock().unwrap().contains_key(&plan.ticket));
        discard(&managed, &instance, &plan.ticket).unwrap();
        assert!(!tickets().lock().unwrap().contains_key(&plan.ticket));
        discard(&managed, &instance, &plan.ticket).unwrap();
    }

    #[test]
    fn native9_local_archive_safety_and_format_are_checked_beyond_the_extension() {
        let (managed, _) = fixture();
        for entry in [
            "../escape",
            "/absolute",
            "C:/drive",
            "nested\\escape",
            "a/./b",
            "modrinth.index.json",
        ] {
            let source = managed.data_root().join("unsafe.zip");
            archive(&source, entry, b"{}");
            assert!(inspect(&source).is_err(), "{entry}");
        }
        let source = managed.data_root().join("fake.jar");
        std::fs::write(&source, b"not a jar").unwrap();
        assert!(inspect(&source).is_err());
    }
}
