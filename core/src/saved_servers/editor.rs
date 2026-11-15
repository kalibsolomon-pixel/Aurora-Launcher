//! Revision-checked editing of vanilla's uncompressed named NBT list.
//! Unknown root/entry fields stay as raw spans, including icons and MUTF-8.
use super::MAX_DOCUMENT;
use crate::{gameplay_history::ServerTarget, instances::InstanceId, nbt, paths::ManagedPaths};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

const MAX_SERVERS: usize = 1024;
const SUPPRESSIONS: &str = ".aurora-server-edits.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ResourcePacks {
    #[default]
    Prompt,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerInput {
    pub name: String,
    pub address: String,
    pub resource_packs: ResourcePacks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerList {
    pub revision: String,
    pub entries: Vec<ServerInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Edit {
    Add { server: ServerInput },
    Edit { index: usize, server: ServerInput },
    Delete { index: usize },
    Move { index: usize, destination: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Malformed,
    Changed,
    Busy,
    Storage,
    Favorites,
}
impl Error {
    pub fn code(self) -> &'static str {
        match self {
            Self::Invalid => "server_input_invalid",
            Self::Malformed => "server_list_malformed",
            Self::Changed => "server_list_changed",
            Self::Busy => "server_instance_busy",
            Self::Storage => "server_list_unavailable",
            Self::Favorites => "server_favorite_unavailable",
        }
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::Invalid => {
                "Enter a name of 1–128 characters and a valid host[:port] address (IPv6 uses brackets). Control characters are not allowed."
            }
            Self::Malformed => {
                "The server list or its edit history is malformed or unsupported. It was left unchanged."
            }
            Self::Changed => {
                "The server list changed since it was loaded. Reload the list before trying again."
            }
            Self::Busy => {
                "Stop Minecraft and wait for instance operations to finish before changing servers. Another launcher may be using this instance."
            }
            Self::Storage => {
                "The server list could not be saved. Check access to the instance folder; the previous list is kept in servers.dat_old."
            }
            Self::Favorites => {
                "The server favorite could not be migrated. The previous server list was restored; check local gameplay history."
            }
        }
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn revision(bytes: Option<&[u8]>) -> String {
    bytes.map(hash).unwrap_or_else(|| "missing".into())
}

pub fn validate(server: &ServerInput) -> Result<(), Error> {
    if server.name.trim().is_empty()
        || server.name.chars().count() > 128
        || server.name.chars().any(char::is_control)
        || ServerTarget::parse(server.address.clone()).is_err()
        || (server.address.starts_with('[')
            && server
                .address
                .split_once(']')
                .is_none_or(|(_, rest)| !rest.is_empty() && !rest.starts_with(':')))
        || server.address.chars().any(char::is_control)
    {
        return Err(Error::Invalid);
    }
    Ok(())
}

struct Document<'a> {
    prefix: &'a [u8],
    fields: Vec<nbt::Field<'a>>,
    entries: Vec<&'a [u8]>,
    values: Vec<ServerInput>,
}
fn text(fields: &[nbt::Field<'_>], name: &str) -> Result<String, Error> {
    let bytes = fields
        .iter()
        .find(|f| f.name == name.as_bytes())
        .and_then(|f| nbt::as_string(f).ok())
        .ok_or(Error::Malformed)?;
    simd_cesu8::mutf8::decode_strict(bytes)
        .map(|s| s.into_owned())
        .map_err(|_| Error::Malformed)
}
fn parse(bytes: &[u8]) -> Result<Document<'_>, Error> {
    if bytes.len() as u64 > MAX_DOCUMENT {
        return Err(Error::Malformed);
    }
    let fields = nbt::root(bytes).map_err(|_| Error::Malformed)?;
    let mut reader = nbt::reader_at(bytes, 1);
    reader.string().map_err(|_| Error::Malformed)?;
    let prefix = &bytes[..reader.at];
    let list = fields
        .iter()
        .find(|f| f.name == b"servers" && f.tag == 9)
        .ok_or(Error::Malformed)?;
    let mut r = nbt::reader_at(list.payload, 0);
    let tag = r.byte().map_err(|_| Error::Malformed)?;
    let count = r.count().map_err(|_| Error::Malformed)?;
    if count > MAX_SERVERS || !(tag == 10 || (tag == 0 && count == 0)) {
        return Err(Error::Malformed);
    }
    let mut entries = Vec::new();
    let mut values = Vec::new();
    for _ in 0..count {
        let start = r.at;
        let entry = r.compound(0).map_err(|_| Error::Malformed)?;
        let resource_packs = match entry.iter().find(|f| f.name == b"acceptTextures") {
            None => ResourcePacks::Prompt,
            Some(f) if f.tag == 1 && f.payload == [0] => ResourcePacks::Disabled,
            Some(f) if f.tag == 1 && f.payload == [1] => ResourcePacks::Enabled,
            _ => return Err(Error::Malformed),
        };
        values.push(ServerInput {
            name: text(&entry, "name")?,
            address: text(&entry, "ip")?,
            resource_packs,
        });
        entries.push(&list.payload[start..r.at]);
    }
    Ok(Document {
        prefix,
        fields,
        entries,
        values,
    })
}
fn named(out: &mut Vec<u8>, tag: u8, name: &str, payload: &[u8]) {
    out.push(tag);
    out.extend_from_slice(&(name.len() as u16).to_be_bytes());
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(payload);
}
fn string(value: &str) -> Vec<u8> {
    let bytes = simd_cesu8::mutf8::encode(value);
    let mut out = (bytes.len() as u16).to_be_bytes().to_vec();
    out.extend_from_slice(&bytes);
    out
}
fn entry(server: &ServerInput, previous: Option<&[u8]>) -> Result<Vec<u8>, Error> {
    validate(server)?;
    let mut out = Vec::new();
    named(&mut out, 8, "name", &string(&server.name));
    named(&mut out, 8, "ip", &string(&server.address));
    match server.resource_packs {
        ResourcePacks::Prompt => {}
        ResourcePacks::Enabled => named(&mut out, 1, "acceptTextures", &[1]),
        ResourcePacks::Disabled => named(&mut out, 1, "acceptTextures", &[0]),
    }
    if let Some(bytes) = previous {
        for field in nbt::reader_at(bytes, 0)
            .compound(0)
            .map_err(|_| Error::Malformed)?
        {
            if !matches!(field.name, b"name" | b"ip" | b"acceptTextures") {
                out.extend_from_slice(field.raw);
            }
        }
    }
    out.push(0);
    Ok(out)
}
fn assemble(doc: &Document<'_>, entries: &[Vec<u8>]) -> Result<Vec<u8>, Error> {
    if entries.len() > MAX_SERVERS {
        return Err(Error::Invalid);
    }
    let mut out = doc.prefix.to_vec();
    for field in &doc.fields {
        if field.name == b"servers" {
            let mut payload = vec![10];
            payload.extend_from_slice(&(entries.len() as i32).to_be_bytes());
            for raw in entries {
                payload.extend_from_slice(raw);
            }
            named(&mut out, 9, "servers", &payload);
        } else {
            out.extend_from_slice(field.raw);
        }
    }
    out.push(0);
    if out.len() as u64 > MAX_DOCUMENT {
        return Err(Error::Invalid);
    }
    Ok(out)
}
const EMPTY: &[u8] = b"\x0a\0\0\x09\0\x07servers\x0a\0\0\0\0\0";

fn read_optional(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, Error> {
    match nbt::read_file(path, limit) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::Storage),
    }
}
fn directory(managed: &ManagedPaths, instance: &InstanceId) -> Result<std::path::PathBuf, Error> {
    crate::install::coordination::checked_directory(
        managed.data_root(),
        managed.instance_paths(instance).root(),
    )
    .map_err(|_| Error::Storage)
}
pub fn load(managed: &ManagedPaths, instance: &InstanceId) -> Result<ServerList, Error> {
    let root = directory(managed, instance)?;
    let bytes = read_optional(&root.join("servers.dat"), MAX_DOCUMENT)?;
    Ok(ServerList {
        revision: revision(bytes.as_deref()),
        entries: parse(bytes.as_deref().unwrap_or(EMPTY))?.values,
    })
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Suppressions {
    schema_version: u32,
    identities: BTreeSet<String>,
}
fn suppressions(bytes: Option<&[u8]>) -> Result<Suppressions, Error> {
    let value = match bytes {
        Some(bytes) => serde_json::from_slice(bytes).map_err(|_| Error::Malformed)?,
        None => Suppressions {
            schema_version: 1,
            ..Default::default()
        },
    };
    if value.schema_version != 1
        || value.identities.len() > 4096
        || value
            .identities
            .iter()
            .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(Error::Malformed);
    }
    Ok(value)
}
fn identity(server: &ServerInput) -> String {
    hash(&serde_json::to_vec(&(&server.name, &server.address)).expect("strings serialize"))
}
/// Shared by manual sync and the legacy minute worker. Explicit local edits
/// must not be resurrected by an additions-only merge from a stopped peer.
pub(crate) fn filter_sync_source(
    managed: &ManagedPaths,
    instance: &InstanceId,
    source: &[u8],
) -> Result<Vec<u8>, Error> {
    let root = directory(managed, instance)?;
    let bytes = read_optional(&root.join(SUPPRESSIONS), MAX_DOCUMENT)?;
    let suppressed = suppressions(bytes.as_deref())?;
    if suppressed.identities.is_empty() {
        return Ok(source.to_vec());
    }
    let doc = parse(source)?;
    let entries = doc
        .entries
        .iter()
        .zip(&doc.values)
        .filter(|(_, value)| !suppressed.identities.contains(&identity(value)))
        .map(|(raw, _)| raw.to_vec())
        .collect::<Vec<_>>();
    assemble(&doc, &entries)
}

/// Keep the exclusive Windows probe alive throughout the commit, preventing
/// another game from opening latest.log in the middle of the transaction.
fn stopped_log(root: &Path) -> Result<Option<fs::File>, Error> {
    let logs = root.join("logs");
    match fs::symlink_metadata(&logs) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Busy),
        Ok(_) => {
            crate::install::coordination::checked_directory(root, &logs)
                .map_err(|_| Error::Busy)?;
        }
    }
    let path = logs.join("latest.log");
    crate::install::coordination::require_regular_or_missing(&path).map_err(|_| Error::Busy)?;
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    match options.open(path) {
        Ok(file) => Ok(Some(file)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::Busy),
    }
}

pub fn apply(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &str,
    edit: &Edit,
) -> Result<ServerList, Error> {
    let _preparing = crate::launch::process::PreparationGuard::acquire(instance.as_str())
        .map_err(|_| Error::Busy)?;
    // Acquire the stable parent lock first: a concurrently deleted instance
    // must not be recreated by the legacy in-directory lock's create path.
    let _operation = crate::instances::lifecycle::lock_instance_parent(managed, instance)
        .map_err(|_| Error::Busy)?;
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|_| Error::Storage)?;
    let record = registry.find(instance).ok_or(Error::Changed)?;
    if record.state() != crate::instances::InstanceState::Ready {
        return Err(Error::Busy);
    }
    directory(managed, instance)?;
    let _legacy = crate::install::coordination::TargetLock::acquire(
        managed.data_root(),
        managed.instance_paths(instance).root(),
        ".instance-operation.lock",
    )
    .map_err(|_| Error::Busy)?;
    crate::instance_content::with_instance_lock(instance, || {
        Ok(apply_locked(managed, instance, expected, edit))
    })
    .map_err(|_| Error::Busy)?
}
fn apply_locked(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &str,
    edit: &Edit,
) -> Result<ServerList, Error> {
    let root = directory(managed, instance)?;
    let _log = stopped_log(&root)?;
    let _stopped =
        crate::launch::process::lock_stopped(instance.as_str()).map_err(|_| Error::Busy)?;
    let path = root.join("servers.dat");
    let before = read_optional(&path, MAX_DOCUMENT)?;
    if revision(before.as_deref()) != expected {
        return Err(Error::Changed);
    }
    let doc = parse(before.as_deref().unwrap_or(EMPTY))?;
    let mut entries = doc.entries.iter().map(|b| b.to_vec()).collect::<Vec<_>>();
    let mut removed = None;
    let mut migration = None;
    match edit {
        Edit::Add { server } => entries.push(entry(server, None)?),
        Edit::Edit { index, server } => {
            let old = doc.values.get(*index).ok_or(Error::Changed)?;
            entries[*index] = entry(server, Some(doc.entries[*index]))?;
            removed = Some(old);
            migration = Some((old, server));
        }
        Edit::Delete { index } => {
            removed = Some(doc.values.get(*index).ok_or(Error::Changed)?);
            entries.remove(*index);
        }
        Edit::Move { index, destination } => {
            if *index >= entries.len() || *destination >= entries.len() {
                return Err(Error::Changed);
            }
            let raw = entries.remove(*index);
            entries.insert(*destination, raw);
        }
    }
    let after = assemble(&doc, &entries)?;
    // Preflight history before changing any instance bytes. Visits keep their
    // original address; only a saved favorite follows the edited destination.
    let history = migration
        .map(|_| {
            crate::gameplay_history::Store::open_managed(managed).map_err(|_| Error::Favorites)
        })
        .transpose()?;
    let suppression_path = root.join(SUPPRESSIONS);
    let old_suppressions = read_optional(&suppression_path, MAX_DOCUMENT)?;
    let mut suppressed = suppressions(old_suppressions.as_deref())?;
    if let Some(old) = removed {
        suppressed.identities.insert(identity(old));
    }
    if suppressed.identities.len() > 4096 {
        return Err(Error::Malformed);
    }
    let next_suppressions = serde_json::to_vec(&suppressed).map_err(|_| Error::Storage)?;
    use crate::install::journal::atomic_write;
    if let Some(bytes) = &before {
        atomic_write(&root.join("servers.dat_old"), bytes).map_err(|_| Error::Storage)?;
    }
    // Recheck after preflight and backup; never overwrite an intervening edit.
    if revision(read_optional(&path, MAX_DOCUMENT)?.as_deref()) != expected {
        return Err(Error::Changed);
    }
    atomic_write(&suppression_path, &next_suppressions).map_err(|_| Error::Storage)?;
    if atomic_write(&path, &after).is_err() {
        restore(&suppression_path, old_suppressions.as_deref())?;
        return Err(Error::Storage);
    }
    if let (Some(history), Some((old, new))) = (history, migration) {
        if let (Ok(old_target), Ok(new_target)) = (
            ServerTarget::parse(old.address.clone()),
            ServerTarget::parse(new.address.clone()),
        ) {
            let old_still_saved = parse(&after)?
                .values
                .iter()
                .any(|s| ServerTarget::parse(s.address.clone()).ok().as_ref() == Some(&old_target));
            if history
                .migrate_saved_server_favorite(
                    instance,
                    &old_target,
                    &new_target,
                    &new.name,
                    old_still_saved,
                )
                .is_err()
            {
                restore(&path, before.as_deref())?;
                restore(&suppression_path, old_suppressions.as_deref())?;
                return Err(Error::Favorites);
            }
        }
    }
    Ok(ServerList {
        revision: revision(Some(&after)),
        entries: parse(&after)?.values,
    })
}
fn restore(path: &Path, bytes: Option<&[u8]>) -> Result<(), Error> {
    match bytes {
        Some(bytes) => crate::install::journal::atomic_write(path, bytes),
        None => fs::remove_file(path),
    }
    .map_err(|_| Error::Storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        managed: ManagedPaths,
        id: InstanceId,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("aurora-server-editor-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            let managed = ManagedPaths::from_app_local_data_dir(root).unwrap();
            let id = crate::instances::generate_instance_id();
            fs::create_dir_all(managed.instance_paths(&id).root()).unwrap();
            let mut configuration =
                crate::instances::settings::InstanceConfiguration::for_minecraft_version("1.21.11");
            configuration.set_aurora_enabled(false);
            let record = crate::instances::InstanceRecord::from_installed(
                id.clone(),
                "Fixture",
                crate::instances::InstanceState::Ready,
                crate::instances::platform::InstalledConfiguration {
                    minecraft_version: "1.21.11".into(),
                    platform: crate::instances::platform::PlatformPin::Vanilla {},
                    aurora: None,
                },
                configuration,
            )
            .unwrap();
            let mut registry = crate::instances::InstanceRegistry::empty();
            registry.instances_mut().push(record);
            registry.save(&managed.instance_registry_file()).unwrap();
            Self { managed, id }
        }
        fn root(&self) -> std::path::PathBuf {
            self.managed.instance_paths(&self.id).root().to_path_buf()
        }
        fn load(&self) -> ServerList {
            load(&self.managed, &self.id).unwrap()
        }
        fn apply(&self, edit: Edit) -> Result<ServerList, Error> {
            apply(&self.managed, &self.id, &self.load().revision, &edit)
        }
        fn bytes(&self) -> Vec<u8> {
            fs::read(self.root().join("servers.dat")).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(self.managed.data_root()).unwrap();
        }
    }
    fn server(name: &str, address: &str) -> ServerInput {
        ServerInput {
            name: name.into(),
            address: address.into(),
            resource_packs: ResourcePacks::Prompt,
        }
    }
    fn rich() -> Vec<u8> {
        let mut first = entry(&server("A 🐧", "a.invalid"), None).unwrap();
        first.pop();
        named(
            &mut first,
            8,
            "icon",
            &string("data:image/png;base64,untouched"),
        );
        named(&mut first, 10, "modData", &[1, 0, 1, b'x', 42, 0]);
        first.push(0);
        let doc = parse(EMPTY).unwrap();
        let mut bytes = assemble(
            &doc,
            &[first, entry(&server("B", "b.invalid"), None).unwrap()],
        )
        .unwrap();
        bytes.pop();
        named(
            &mut bytes,
            12,
            "unknownRoot",
            &[0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 42],
        );
        bytes.push(0);
        // A named root, not just the usual empty root name.
        bytes.splice(1..3, string("Multiplayer"));
        bytes
    }
    #[test]
    fn round_trip_preserves_named_root_unknown_tags_icons_and_order() {
        let bytes = rich();
        let doc = parse(&bytes).unwrap();
        assert_eq!(doc.values[0].name, "A 🐧");
        assert_eq!(
            assemble(
                &doc,
                &doc.entries.iter().map(|s| s.to_vec()).collect::<Vec<_>>()
            )
            .unwrap(),
            bytes
        );
        let edited = entry(&server("Edited 🐧", "new.invalid"), Some(doc.entries[0])).unwrap();
        let fields = nbt::reader_at(&edited, 0).compound(0).unwrap();
        for field in nbt::reader_at(doc.entries[0], 0).compound(0).unwrap() {
            if matches!(field.name, b"icon" | b"modData") {
                assert!(fields.iter().any(|f| f.raw == field.raw));
            }
        }
        let result = assemble(&doc, &[edited, doc.entries[1].to_vec()]).unwrap();
        let next = parse(&result).unwrap();
        assert_eq!(next.prefix, doc.prefix);
        assert!(
            next.fields
                .iter()
                .any(|f| f.raw == doc.fields.last().unwrap().raw)
        );
        assert_eq!(next.values[1], doc.values[1]);
    }
    #[test]
    fn add_edit_delete_reorder_and_previous_file_backup() {
        let f = Fixture::new();
        fs::write(f.root().join("servers.dat"), rich()).unwrap();
        let original = f.bytes();
        f.apply(Edit::Add {
            server: server("C", "c.invalid:123"),
        })
        .unwrap();
        assert_eq!(
            fs::read(f.root().join("servers.dat_old")).unwrap(),
            original
        );
        let mut edited = server("Renamed", "renamed.invalid");
        edited.resource_packs = ResourcePacks::Enabled;
        f.apply(Edit::Edit {
            index: 0,
            server: edited.clone(),
        })
        .unwrap();
        assert_eq!(f.load().entries[0], edited);
        f.apply(Edit::Move {
            index: 0,
            destination: 2,
        })
        .unwrap();
        assert_eq!(
            f.load()
                .entries
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["B", "C", "Renamed"]
        );
        f.apply(Edit::Delete { index: 1 }).unwrap();
        assert_eq!(f.load().entries.len(), 2);
        let bytes = f.bytes();
        let doc = parse(&bytes).unwrap();
        assert!(
            nbt::reader_at(doc.entries[1], 0)
                .compound(0)
                .unwrap()
                .iter()
                .any(|f| f.name == b"icon")
        );
    }
    #[test]
    fn missing_file_add_creates_valid_vanilla_document_and_duplicates_are_allowed() {
        let f = Fixture::new();
        assert_eq!(f.load().revision, "missing");
        f.apply(Edit::Add {
            server: server("Local", "localhost"),
        })
        .unwrap();
        assert_eq!(&f.bytes()[..3], &[10, 0, 0]);
        f.apply(Edit::Add {
            server: server("Local", "localhost"),
        })
        .unwrap();
        assert_eq!(f.load().entries.len(), 2);
        assert_eq!(super::super::parse(&f.bytes()).unwrap().len(), 2);
    }
    #[test]
    fn resource_pack_preference_matches_vanilla_byte_or_absence() {
        for (preference, payload) in [
            (ResourcePacks::Prompt, None),
            (ResourcePacks::Enabled, Some(vec![1])),
            (ResourcePacks::Disabled, Some(vec![0])),
        ] {
            let mut input = server("A", "localhost");
            input.resource_packs = preference;
            let raw = entry(&input, None).unwrap();
            let fields = nbt::reader_at(&raw, 0).compound(0).unwrap();
            assert_eq!(
                fields
                    .iter()
                    .find(|f| f.name == b"acceptTextures")
                    .map(|f| f.payload.to_vec()),
                payload
            );
        }
    }
    #[test]
    fn revision_mismatch_refuses_changes_including_missing_to_created() {
        let f = Fixture::new();
        let old = f.load();
        fs::write(f.root().join("servers.dat"), rich()).unwrap();
        assert_eq!(
            apply(&f.managed, &f.id, &old.revision, &Edit::Delete { index: 0 }).unwrap_err(),
            Error::Changed
        );
        assert_eq!(f.bytes(), rich());
        let old = f.load();
        fs::write(f.root().join("servers.dat"), EMPTY).unwrap();
        assert_eq!(
            apply(
                &f.managed,
                &f.id,
                &old.revision,
                &Edit::Add {
                    server: server("X", "localhost")
                }
            )
            .unwrap_err(),
            Error::Changed
        );
        assert!(!f.root().join("servers.dat_old").exists());
    }
    #[test]
    fn running_preparing_content_and_lifecycle_busy_instances_refuse_writes() {
        let f = Fixture::new();
        let edit = Edit::Add {
            server: server("X", "localhost"),
        };
        let running = crate::launch::process::force_running(f.id.as_str());
        assert_eq!(f.apply(edit.clone()).unwrap_err(), Error::Busy);
        drop(running);
        crate::launch::process::with_test_state(
            f.id.as_str(),
            crate::launch::state::LaunchProcessStatus::Starting,
            || {
                assert_eq!(f.apply(edit.clone()).unwrap_err(), Error::Busy);
            },
        );
        let preparing = crate::launch::process::PreparationGuard::acquire(f.id.as_str()).unwrap();
        assert_eq!(f.apply(edit.clone()).unwrap_err(), Error::Busy);
        drop(preparing);
        let operation =
            crate::instances::lifecycle::lock_instance_operation(&f.managed, &f.id).unwrap();
        assert_eq!(f.apply(edit.clone()).unwrap_err(), Error::Busy);
        drop(operation);
        crate::instance_content::with_instance_lock(&f.id, || {
            assert_eq!(f.apply(edit).unwrap_err(), Error::Busy);
            Ok(())
        })
        .unwrap();
        assert!(!f.root().join("servers.dat").exists());
    }
    #[cfg(windows)]
    #[test]
    fn external_game_log_lock_refuses_writes_without_starting_any_process() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        fs::create_dir(f.root().join("logs")).unwrap();
        let path = f.root().join("logs/latest.log");
        fs::write(&path, "fixture").unwrap();
        let owned = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        let edit = Edit::Add {
            server: server("X", "localhost"),
        };
        assert_eq!(f.apply(edit.clone()).unwrap_err(), Error::Busy);
        assert!(!f.root().join("servers.dat").exists());
        drop(owned);
        f.apply(edit).unwrap();
    }
    #[test]
    fn invalid_input_malformed_nbt_and_out_of_range_actions_leave_bytes_untouched() {
        let f = Fixture::new();
        fs::write(f.root().join("servers.dat"), rich()).unwrap();
        for input in [
            server("", "localhost"),
            server(&"x".repeat(129), "localhost"),
            server("A\nB", "localhost"),
            server("A", "host:0"),
            server("A", "https://host"),
            server("A", "host\n"),
            server("A", "[::1]garbage"),
        ] {
            assert_eq!(
                f.apply(Edit::Add { server: input }).unwrap_err(),
                Error::Invalid
            );
            assert_eq!(f.bytes(), rich());
        }
        assert_eq!(
            f.apply(Edit::Move {
                index: 0,
                destination: 2
            })
            .unwrap_err(),
            Error::Changed
        );
        assert_eq!(
            f.apply(Edit::Delete { index: 2 }).unwrap_err(),
            Error::Changed
        );
        assert!(validate(&server("IPv6", "[::1]:25565")).is_ok());
        let bytes = rich();
        for end in 0..bytes.len() {
            assert!(parse(&bytes[..end]).is_err());
        }
        fs::write(f.root().join("servers.dat"), b"bad").unwrap();
        assert_eq!(
            apply(&f.managed, &f.id, &hash(b"bad"), &Edit::Delete { index: 0 }).unwrap_err(),
            Error::Malformed
        );
        assert_eq!(f.bytes(), b"bad");
    }
    #[test]
    fn favorites_migrate_to_new_address_and_renames_update_the_label() {
        let f = Fixture::new();
        f.apply(Edit::Add {
            server: server("A", "old.invalid"),
        })
        .unwrap();
        let store = crate::gameplay_history::Store::open_managed(&f.managed).unwrap();
        let old = ServerTarget::parse("old.invalid".into()).unwrap();
        store
            .set_saved_server_favorite(f.id.clone(), old.clone(), "A".into(), true)
            .unwrap();
        f.apply(Edit::Edit {
            index: 0,
            server: server("New", "new.invalid"),
        })
        .unwrap();
        let rows = store.server_rows(Some(&f.id), 200, true).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].favorite);
        let new = ServerTarget::parse("new.invalid".into()).unwrap();
        assert_eq!(
            rows[0].id,
            crate::gameplay_history::target_id(
                crate::gameplay_history::Mode::Multiplayer,
                &f.id,
                new.as_str()
            )
        );
        assert_eq!(rows[0].display_name, "New");
        f.apply(Edit::Edit {
            index: 0,
            server: server("Renamed", "new.invalid"),
        })
        .unwrap();
        assert_eq!(
            store.server_rows(Some(&f.id), 200, true).unwrap()[0].display_name,
            "Renamed"
        );
        assert!(
            store
                .quick_target(crate::gameplay_history::Mode::Multiplayer, &rows[0].id)
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn editing_one_duplicate_keeps_both_address_favorites() {
        let f = Fixture::new();
        for _ in 0..2 {
            f.apply(Edit::Add {
                server: server("A", "old.invalid"),
            })
            .unwrap();
        }
        let store = crate::gameplay_history::Store::open_managed(&f.managed).unwrap();
        store
            .set_saved_server_favorite(
                f.id.clone(),
                ServerTarget::parse("old.invalid".into()).unwrap(),
                "A".into(),
                true,
            )
            .unwrap();
        f.apply(Edit::Edit {
            index: 0,
            server: server("New", "new.invalid"),
        })
        .unwrap();
        assert_eq!(
            store
                .server_rows(Some(&f.id), 200, true)
                .unwrap()
                .iter()
                .filter(|s| s.favorite)
                .count(),
            2
        );
    }
    #[test]
    fn deletions_and_edits_record_durable_sync_suppressions() {
        let f = Fixture::new();
        fs::write(f.root().join("servers.dat"), rich()).unwrap();
        f.apply(Edit::Delete { index: 0 }).unwrap();
        f.apply(Edit::Edit {
            index: 0,
            server: server("Replacement", "replacement.invalid"),
        })
        .unwrap();
        let filtered = filter_sync_source(&f.managed, &f.id, &rich()).unwrap();
        assert!(parse(&filtered).unwrap().values.is_empty());
        // A deliberate add can still recreate a suppressed identity.
        f.apply(Edit::Add {
            server: server("B", "b.invalid"),
        })
        .unwrap();
        assert_eq!(f.load().entries.len(), 2);
    }

    #[test]
    fn favorite_limit_failure_restores_list_and_sync_history() {
        let f = Fixture::new();
        for _ in 0..2 {
            f.apply(Edit::Add {
                server: server("A", "old.invalid"),
            })
            .unwrap();
        }
        let store = crate::gameplay_history::Store::open_managed(&f.managed).unwrap();
        store
            .set_saved_server_favorite(
                f.id.clone(),
                ServerTarget::parse("old.invalid".into()).unwrap(),
                "A".into(),
                true,
            )
            .unwrap();
        for index in 0..99 {
            store
                .set_saved_server_favorite(
                    f.id.clone(),
                    ServerTarget::parse(format!("host{index}.invalid")).unwrap(),
                    "Other".into(),
                    true,
                )
                .unwrap();
        }
        let before = f.bytes();
        let sync_before = fs::read(f.root().join(SUPPRESSIONS)).unwrap();
        assert_eq!(
            f.apply(Edit::Edit {
                index: 0,
                server: server("New", "new.invalid")
            })
            .unwrap_err(),
            Error::Favorites
        );
        assert_eq!(f.bytes(), before);
        assert_eq!(fs::read(f.root().join(SUPPRESSIONS)).unwrap(), sync_before);
        assert_eq!(
            store
                .server_rows(Some(&f.id), 200, true)
                .unwrap()
                .iter()
                .filter(|s| s.favorite)
                .count(),
            100
        );
    }

    #[test]
    fn document_bounds_and_damaged_edit_history_fail_closed() {
        assert!(parse(&vec![0; MAX_DOCUMENT as usize + 1]).is_err());
        let doc = parse(EMPTY).unwrap();
        let entries = vec![entry(&server("A", "localhost"), None).unwrap(); MAX_SERVERS + 1];
        assert_eq!(assemble(&doc, &entries).unwrap_err(), Error::Invalid);
        let f = Fixture::new();
        fs::write(f.root().join("servers.dat"), rich()).unwrap();
        fs::write(
            f.root().join(SUPPRESSIONS),
            br#"{"schemaVersion":2,"identities":[]}"#,
        )
        .unwrap();
        assert_eq!(
            f.apply(Edit::Delete { index: 0 }).unwrap_err(),
            Error::Malformed
        );
        assert_eq!(f.bytes(), rich());
        assert!(!f.root().join("servers.dat_old").exists());
    }

    #[test]
    fn a_deleted_instance_is_not_recreated_by_the_writer() {
        let f = Fixture::new();
        let loaded = f.load();
        let mut registry =
            crate::instances::InstanceRegistry::load(&f.managed.instance_registry_file()).unwrap();
        registry.instances_mut().clear();
        registry.save(&f.managed.instance_registry_file()).unwrap();
        fs::remove_dir(f.root()).unwrap();
        assert_eq!(
            apply(
                &f.managed,
                &f.id,
                &loaded.revision,
                &Edit::Add {
                    server: server("A", "localhost")
                }
            )
            .unwrap_err(),
            Error::Changed
        );
        assert!(!f.root().exists());
    }
}
