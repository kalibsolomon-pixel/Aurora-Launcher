//! Reviewed local JAR/resource-pack/shader import. No extraction, network,
//! implicit overwrite, provider adoption or arbitrary destination paths.
use crate::import_source::{self, Folder, MAX_BYTES, Scratch};
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub ticket: String,
    pub fingerprint: String,
    pub instance_name: String,
    pub file_name: String,
    pub file_names: Vec<String>,
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
const MAX_EXPANDED_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[cfg(test)]
fn inspect(path: &Path) -> Result<ContentType, ContentError> {
    let mut remaining = MAX_EXPANDED_BYTES;
    inspect_bounded(path, &mut remaining)
}

fn inspect_bounded(path: &Path, remaining: &mut u64) -> Result<ContentType, ContentError> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).map_err(ContentError::Io)?)
        .map_err(|_| invalid("The selected file is not a valid ZIP/JAR archive."))?;
    if archive.len() > 4096 {
        return Err(invalid("The archive has too many entries."));
    }
    let mut names = HashSet::new();
    let mut mod_metadata = false;
    let mut resource = false;
    let mut shader = false;
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
            return Err(invalid("Import modpacks from the Instances page."));
        }
        *remaining = remaining
            .checked_sub(entry.size())
            .ok_or_else(|| invalid("The selection exceeds safe unpacked-size limits."))?;
        if entry.size() > 128 * 1024 * 1024 {
            return Err(invalid(
                "An archive entry exceeds safe unpacked-size limits.",
            ));
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

pub(crate) struct ImportFile {
    pub(crate) kind: ContentType,
    pub(crate) name: String,
    pub(crate) digest: String,
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
}

fn inspect_snapshot(
    path: PathBuf,
    name: String,
    remaining: &mut u64,
) -> Result<ImportFile, ContentError> {
    let kind = inspect_bounded(&path, remaining)?;
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
    let mut reader = std::fs::File::open(&path)
        .map_err(ContentError::Io)?
        .take(MAX_BYTES + 1);
    let mut hash = Sha256::new();
    let size = std::io::copy(&mut reader, &mut hash).map_err(ContentError::Io)?;
    if size > MAX_BYTES {
        return Err(ContentError::InvalidProviderArtifact);
    }
    Ok(ImportFile {
        kind,
        name,
        digest: format!("{:x}", hash.finalize()),
        path,
        size,
    })
}

fn snapshots(source: &Path, scratch: &Scratch) -> Result<Vec<ImportFile>, ContentError> {
    let mut remaining = MAX_EXPANDED_BYTES;
    if !import_source::metadata(source)?.is_dir() {
        let (folder, entry) = Folder::selected_file(source)?;
        let (path, mut output) = scratch.file(&entry.name)?;
        folder.copy(&entry, &mut output)?;
        drop(output);
        return Ok(vec![inspect_snapshot(path, entry.name, &mut remaining)?]);
    }
    let folder = Folder::open(source)?;
    let top = folder.files(false)?;
    // Shader-only folders may have no files at the top level.
    let has_pack_metadata = top.iter().any(|entry| entry.name == "pack.mcmeta");
    let jars: Vec<_> = top
        .iter()
        .filter(|entry| entry.name.to_ascii_lowercase().ends_with(".jar"))
        .collect();
    if !has_pack_metadata && !jars.is_empty() {
        // The scan is nonrecursive for mods. Bound the whole batch as well as each JAR.
        if jars.len() > 128
            || jars
                .iter()
                .map(|entry| entry.size.min(MAX_BYTES + 1))
                .sum::<u64>()
                > MAX_BYTES
        {
            return Err(invalid(
                "Choose a folder with at most 128 top-level JARs totaling up to 512 MiB.",
            ));
        }
        let mut imports = Vec::new();
        let mut names = HashSet::new();
        for entry in jars {
            if !names.insert(entry.name.to_lowercase()) {
                return Err(ContentError::Collision);
            }
            let (path, mut output) = scratch.file(&entry.name)?;
            folder.copy(entry, &mut output)?;
            drop(output);
            let file = inspect_snapshot(path, entry.name.clone(), &mut remaining)?;
            if file.kind != ContentType::Mod {
                return Err(ContentError::InvalidProviderArtifact);
            }
            imports.push(file);
        }
        return Ok(imports);
    }
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ContentError::UnsafePath)?;
    let name = format!("{name}.zip");
    let (path, output) = scratch.file(&name)?;
    import_source::zip(&folder, &folder.files(true)?, None, output)?;
    let file = inspect_snapshot(path, name, &mut remaining)?;
    if file.kind == ContentType::Mod {
        return Err(invalid("Choose a folder of JARs to import mods."));
    }
    Ok(vec![file])
}

fn review(
    managed: &ManagedPaths,
    instance: &InstanceId,
    files: &[ImportFile],
) -> Result<Preview, ContentError> {
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|e| ContentError::StateMalformed(e.to_string()))?;
    let record = registry.find(instance).ok_or(ContentError::UnsafePath)?;
    if record.state() != InstanceState::Ready {
        return Err(invalid("Finish installing the target instance first."));
    }
    let first = files.first().ok_or(ContentError::InvalidProviderArtifact)?;
    let kind = first.kind;
    let directory = content::validate_directory(managed, instance, kind)?;
    let existing = match std::fs::read_dir(&directory) {
        Ok(entries) => entries
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().to_lowercase()))
            .collect::<Result<HashSet<_>, _>>()
            .map_err(ContentError::Io)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashSet::new(),
        Err(error) => return Err(ContentError::Io(error)),
    };
    let mut blockers = Vec::new();
    for file in files {
        if existing.contains(&file.name.to_lowercase()) {
            blockers.push(format!(
                "A file named '{}' already exists. Nothing will be overwritten.",
                file.name
            ));
        }
    }
    if let Err(error) = content::validate_local_import(managed, instance, files) {
        blockers.push(error.to_string());
    }
    let revision = content::local_inventory_revision(managed, instance)?;
    let evidence: Vec<_> = files
        .iter()
        .map(|file| (&file.name, file.size, &file.digest))
        .collect();
    let encoded =
        serde_json::to_vec(&evidence).map_err(|_| ContentError::InvalidProviderArtifact)?;
    let digest = if files.len() == 1 {
        first.digest.clone()
    } else {
        format!("{:x}", Sha256::digest(encoded))
    };
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(record, kind, evidence, revision, &blockers))
                .map_err(|_| ContentError::InvalidProviderArtifact)?
        )
    );
    Ok(Preview {
        ticket: String::new(), fingerprint, instance_name: record.display_name().into(),
        file_name: if files.len() == 1 { first.name.clone() } else { format!("{} mods", files.len()) },
        file_names: files.iter().map(|file| file.name.clone()).collect(),
        content_type: kind, size_bytes: files.iter().map(|file| file.size).sum(),
        sha256: digest, blockers,
        note: "Installs the reviewed local bytes as user-managed content. Publisher authenticity is not established. Mods must satisfy the current loader and dependency checks.".into(),
    })
}

pub fn preview(
    managed: &ManagedPaths,
    instance: InstanceId,
    source: PathBuf,
) -> Result<Preview, ContentError> {
    let scratch = Scratch::new()?;
    let files = snapshots(&source, &scratch)?;
    let mut preview =
        content::with_instance_lock(&instance, || review(managed, &instance, &files))?;
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
    let source = ticket.source;
    let (scratch, mut files) = tokio::task::spawn_blocking(move || {
        let scratch = Scratch::new()?;
        let files = snapshots(&source, &scratch)?;
        Ok::<_, ContentError>((scratch, files))
    })
    .await
    .map_err(|_| ContentError::ChangedSinceScan)??;
    let fresh = content::with_instance_lock(instance, || review(managed, instance, &files))?;
    if fresh.fingerprint != fingerprint || !fresh.blockers.is_empty() {
        return Err(ContentError::ChangedSinceScan);
    }
    let cache = crate::cache::ArtifactCache::new(managed.clone());
    for file in &mut files {
        let digest = ArtifactDigest::parse(&file.digest).map_err(|_| ContentError::HashMismatch)?;
        let artifact = cache
            .import_local(&file.path, &digest, file.size)
            .await
            .map_err(|e| ContentError::Acquisition(e.to_string()))?;
        file.path = artifact.path;
    }
    drop(scratch);
    content::with_instance_lock(instance, || {
        let fresh = review(managed, instance, &files)?;
        if fresh.fingerprint != fingerprint || !fresh.blockers.is_empty() {
            return Err(ContentError::ChangedSinceScan);
        }
        let _commit = crate::content_jobs::commit().map_err(|_| ContentError::Cancelled)?;
        let directory = content::ensure_directory(managed, instance, fresh.content_type)?;
        let mut staged = Vec::new();
        let result = (|| {
            for file in &files {
                let temporary = directory.join(format!(".local-import-{}", uuid::Uuid::new_v4()));
                let mut output = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temporary)
                    .map_err(ContentError::Io)?;
                staged.push((temporary, directory.join(&file.name), file, false));
                let mut input = std::fs::File::open(&file.path)
                    .map_err(ContentError::Io)?
                    .take(file.size + 1);
                std::io::copy(&mut input, &mut output).map_err(ContentError::Io)?;
                drop(output);
                let digest =
                    ArtifactDigest::parse(&file.digest).map_err(|_| ContentError::HashMismatch)?;
                verify_file(&staged.last().unwrap().0, &digest, Some(file.size))
                    .map_err(|_| ContentError::HashMismatch)?;
            }
            for (temporary, target, _, published) in &mut staged {
                std::fs::hard_link(temporary, target).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::AlreadyExists {
                        ContentError::Collision
                    } else {
                        ContentError::Io(e)
                    }
                })?;
                *published = true;
            }
            Ok(())
        })();
        let mut rollback_complete = true;
        for (temporary, target, file, published) in staged {
            if result.is_err() && published {
                // A game or file manager may have changed the new path. Only
                // roll back a regular file that still matches the staged bytes.
                let removed = (|| {
                    if !import_source::metadata(&target)?.is_file() {
                        return Err(ContentError::UnsafePath);
                    }
                    let digest = ArtifactDigest::parse(&file.digest)
                        .map_err(|_| ContentError::HashMismatch)?;
                    verify_file(&target, &digest, Some(file.size))
                        .map_err(|_| ContentError::HashMismatch)?;
                    std::fs::remove_file(target).map_err(ContentError::Io)
                })();
                rollback_complete &= removed.is_ok();
            }
            let _ = std::fs::remove_file(temporary);
        }
        if !rollback_complete {
            return Err(invalid(
                "The batch could not be completed and changed files were left in place.",
            ));
        }
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
    struct FixtureCleanup(PathBuf);
    impl Drop for FixtureCleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn extracted_resource_and_shader_packs_install_as_game_readable_zips() {
        let (managed, instance) = fixture();
        let _cleanup = FixtureCleanup(managed.data_root().to_owned());
        for (name, entry, bytes, kind) in [
            (
                "Resource Folder",
                "pack.mcmeta",
                br#"{"pack":{"pack_format":75}}"#.as_slice(),
                ContentType::ResourcePack,
            ),
            (
                "Shader Folder",
                "shaders/basic.fsh",
                b"void main(){}".as_slice(),
                ContentType::ShaderPack,
            ),
        ] {
            let source = managed.data_root().join(name);
            let file = source.join(entry);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, bytes).unwrap();
            let reviewed = preview(&managed, instance.clone(), source).unwrap();
            assert_eq!(reviewed.content_type, kind);
            assert!(reviewed.blockers.is_empty());
            install(&managed, &instance, &reviewed.ticket, &reviewed.fingerprint)
                .await
                .unwrap();
            let target = managed
                .instance_paths(&instance)
                .root()
                .join(kind.directory_name())
                .join(format!("{name}.zip"));
            assert_eq!(inspect(&target).unwrap(), kind);
            let mut zip = zip::ZipArchive::new(std::fs::File::open(target).unwrap()).unwrap();
            let mut extracted = Vec::new();
            zip.by_name(entry)
                .unwrap()
                .read_to_end(&mut extracted)
                .unwrap();
            assert_eq!(extracted, bytes);
            assert_eq!(std::fs::read(file).unwrap(), bytes);
        }
    }

    #[tokio::test]
    async fn mod_folder_reviews_top_level_jars_together_and_rejects_stale_batches() {
        let (managed, instance) = fixture();
        let _cleanup = FixtureCleanup(managed.data_root().to_owned());
        let source = managed.data_root().join("mods to import");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::write(source.join("nested/ignored.jar"), b"not a jar").unwrap();
        archive(
            &source.join("a.jar"),
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"example_a","version":"1","depends":{"example_b":"*"}} "#,
        );
        archive(
            &source.join("b.jar"),
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"example_b","version":"1"}"#,
        );
        let first = preview(&managed, instance.clone(), source.clone()).unwrap();
        assert_eq!(first.file_names, ["a.jar", "b.jar"]);
        assert!(first.blockers.is_empty(), "{:?}", first.blockers);
        archive(
            &source.join("b.jar"),
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"example_b","version":"2"}"#,
        );
        assert!(matches!(
            install(&managed, &instance, &first.ticket, &first.fingerprint).await,
            Err(ContentError::ChangedSinceScan)
        ));
        discard(&managed, &instance, &first.ticket).unwrap();
        let instance_paths = managed.instance_paths(&instance);
        let mods = instance_paths.mods();
        assert!(!mods.join("a.jar").exists());
        let second = preview(&managed, instance.clone(), source.clone()).unwrap();
        install(&managed, &instance, &second.ticket, &second.fingerprint)
            .await
            .unwrap();
        for name in ["a.jar", "b.jar"] {
            assert_eq!(
                std::fs::read(mods.join(name)).unwrap(),
                std::fs::read(source.join(name)).unwrap()
            );
        }
        assert!(!mods.join("ignored.jar").exists());
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
