//! Main-instance sync group (owner decisions, 2026-11-15).
//!
//! An instance joining the group is seeded from the main instance. Afterwards a
//! settings change captured from any member becomes the group value and is
//! applied to every other member right before that member launches. Capture
//! happens after a member's game exits, plus lazily before any launch or Sync
//! now. The newest capture wins; the main breaks exact ties. Resource and shader
//! packs are pooled: a member receives packs it lacks, nothing is deleted.
//!
//! Synchronized keys come only from the generated catalog; everything else
//! (world data, counters, guards, devices, consent) stays per-instance. Files
//! are never written while that instance's game is starting or running, and
//! every changed file is backed up first so it can be restored.
use super::group_catalog::{AURORA_FIELDS, VANILLA_KEYS};
use super::*;
use serde_json::Value;

const MAX_JSON: u64 = 4 * 1024 * 1024;
const MAX_PROFILES: usize = 64;
const MAX_HISTORY: usize = 100;
const MAX_POOL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// How a synchronized Aurora field is projected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuroraKind {
    Plain,
    /// Release-specific key codes: only applied between documents whose
    /// `keybindVersion` marker matches.
    KeyCombo,
    ModifierMask,
    ActiveProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupConfig {
    pub schema_version: u32,
    pub enabled: bool,
    pub main_instance_id: Option<InstanceId>,
    pub members: Vec<InstanceId>,
    pub new_instances_join: bool,
    pub vanilla: bool,
    pub aurora: bool,
    pub packs: bool,
    /// Members that have not received their initial seed from the main.
    #[serde(default)]
    pub pending_seed: Vec<InstanceId>,
    /// Instances that existed when the group was last saved; anything newer
    /// joins on first launch when `new_instances_join` is set.
    #[serde(default)]
    pub known: Vec<InstanceId>,
}
impl Default for GroupConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            enabled: false,
            main_instance_id: None,
            members: vec![],
            new_instances_join: true,
            vanilla: true,
            aurora: true,
            packs: true,
            pending_seed: vec![],
            known: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    value: Value,
    at: u64,
    source: String,
    /// The source document's keybind codec marker, for key-code fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    marker: Option<Value>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct GroupState {
    #[serde(default)]
    main_captured: bool,
    #[serde(default)]
    values: BTreeMap<String, Entry>,
    /// Per member: key -> digest of the value last captured from or applied to it.
    #[serde(default)]
    members: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub instance_id: String,
    pub at: u64,
    pub kind: String,
    pub keys: Vec<String>,
    pub files: Vec<String>,
    pub packs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub applied: Vec<String>,
    pub deferred: Vec<String>,
    pub changed_settings: usize,
    pub copied_packs: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberView {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub member: bool,
    pub main: bool,
    pub pending_seed: bool,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub config: GroupConfig,
    pub instances: Vec<MemberView>,
    pub history: Vec<HistoryEntry>,
    pub suggested_main: Option<String>,
}

fn config_file(managed: &ManagedPaths) -> PathBuf {
    managed.launcher_dir().join("sync-group.json")
}
fn state_file(managed: &ManagedPaths) -> PathBuf {
    managed.launcher_dir().join("sync-group-state.json")
}
fn history_file(managed: &ManagedPaths) -> PathBuf {
    managed.launcher_dir().join("sync-group-history.json")
}
fn backup_root(managed: &ManagedPaths) -> PathBuf {
    managed.launcher_dir().join("sync-group-backups")
}
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}
fn read_json<T: serde::de::DeserializeOwned + Default>(
    managed: &ManagedPaths,
    path: &Path,
) -> Result<T, SyncError> {
    match read_small(managed, path, MAX_JSON)? {
        None => Ok(T::default()),
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
            fail(
                "sync_group_malformed",
                "Sync group data is malformed and was left unchanged.",
            )
        }),
    }
}
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), SyncError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(storage)?;
    }
    crate::install::journal::atomic_write(path, &serde_json::to_vec_pretty(value).map_err(storage)?)
        .map_err(storage)
}
fn lock(managed: &ManagedPaths) -> Result<TargetLock, SyncError> {
    fs::create_dir_all(managed.launcher_dir()).map_err(storage)?;
    TargetLock::acquire(
        managed.data_root(),
        &managed.launcher_dir(),
        ".sync-group.lock",
    )
    .map_err(|_| {
        fail(
            "sync_group_busy",
            "Another sync is in progress. Try again in a moment.",
        )
    })
}

pub fn config(managed: &ManagedPaths) -> Result<GroupConfig, SyncError> {
    let value: GroupConfig = read_json(managed, &config_file(managed))?;
    if value.schema_version != 1 {
        return Err(fail(
            "sync_schema_unsupported",
            "This sync group schema is unsupported and was left unchanged.",
        ));
    }
    Ok(value)
}

/// The group is in charge of an instance's settings, so the legacy pool must
/// not also write them.
pub fn active(managed: &ManagedPaths) -> bool {
    config(managed).is_ok_and(|c| c.enabled && c.main_instance_id.is_some())
}

/// Validates and persists a proposed group. Newly added members (and every
/// member when the group is first enabled or the main changes from nothing)
/// are queued for seeding from the main; a UI cannot mark anyone seeded.
pub fn save_config(
    managed: &ManagedPaths,
    mut proposed: GroupConfig,
) -> Result<GroupConfig, SyncError> {
    let _lock = lock(managed)?;
    let current = config(managed)?;
    let registry = InstanceRegistry::load(&managed.instance_registry_file()).map_err(storage)?;
    let unique: BTreeSet<_> = proposed.members.iter().map(InstanceId::as_str).collect();
    if proposed.schema_version != 1
        || proposed.members.len() > 64
        || unique.len() != proposed.members.len()
        || proposed
            .members
            .iter()
            .any(|id| registry.find(id).is_none())
    {
        return Err(unsafe_path());
    }
    if proposed.enabled {
        let Some(main) = &proposed.main_instance_id else {
            return Err(fail("sync_main_required", "Choose a main instance first."));
        };
        if !registry
            .find(main)
            .is_some_and(|r| r.state() == InstanceState::Ready)
        {
            return Err(fail(
                "sync_main_not_ready",
                "Choose an installed, ready main instance.",
            ));
        }
        if !proposed.members.contains(main) {
            proposed.members.insert(0, main.clone());
        }
    }
    let main = proposed.main_instance_id.clone();
    let starting = proposed.enabled && (!current.enabled || current.main_instance_id.is_none());
    proposed.pending_seed = proposed
        .members
        .iter()
        .filter(|id| Some(*id) != main.as_ref())
        .filter(|id| starting || !current.members.contains(id) || current.pending_seed.contains(id))
        .cloned()
        .collect();
    proposed.known = registry
        .instances()
        .iter()
        .map(|r| r.id().clone())
        .collect();
    if starting {
        // A fresh group starts from the main's current settings.
        write_json(&state_file(managed), &GroupState::default())?;
    } else {
        let mut state: GroupState = read_json(managed, &state_file(managed))?;
        state
            .members
            .retain(|id, _| proposed.members.iter().any(|m| m.as_str() == id));
        if current.main_instance_id != proposed.main_instance_id {
            state.main_captured = false;
        }
        write_json(&state_file(managed), &state)?;
    }
    write_json(&config_file(managed), &proposed)?;
    Ok(proposed)
}

// ------------------------------------------------------------------ running

/// True only when the instance is verifiably stopped. A running game (from
/// this or any other launcher) keeps `logs/latest.log` open; an exclusive open
/// fails with a sharing violation while it runs. Uncertainty counts as running.
pub fn stopped(managed: &ManagedPaths, id: &InstanceId, launching: bool) -> bool {
    if crate::launch::process::snapshot(id.as_str())
        .status
        .blocks_launch()
    {
        return false;
    }
    if !launching && crate::launch::process::is_preparing(id.as_str()) {
        return false;
    }
    log_unlocked(&managed.instance_paths(id).logs().join("latest.log"))
}

#[cfg(windows)]
fn log_unlocked(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    match OpenOptions::new().read(true).share_mode(0).open(path) {
        Ok(_) => true,
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}
#[cfg(not(windows))]
fn log_unlocked(_: &Path) -> bool {
    true
}

// ------------------------------------------------------------- documents

/// The synchronized view of one member, read without modification.
#[derive(Default)]
struct Snapshot {
    values: BTreeMap<String, Value>,
    markers: BTreeMap<String, Option<Value>>,
}

fn aurora_kind(field: &str) -> Option<(AuroraKind, bool)> {
    AURORA_FIELDS
        .iter()
        .find(|(name, _, _)| *name == field)
        .map(|(_, kind, profile)| (*kind, *profile))
}

fn read_object(
    managed: &ManagedPaths,
    path: &Path,
) -> Result<Option<serde_json::Map<String, Value>>, SyncError> {
    match read_small(managed, path, MAX_JSON)? {
        None => Ok(None),
        Some(bytes) => match serde_json::from_slice(&bytes) {
            Ok(Value::Object(map)) => Ok(Some(map)),
            _ => Err(fail(
                "sync_aurora_invalid",
                "An Aurora settings file is not valid JSON; it was left unchanged.",
            )),
        },
    }
}

/// Profile files keyed by file name, in a bounded, non-redirected directory.
fn profile_files(managed: &ManagedPaths, id: &InstanceId) -> Result<Vec<String>, SyncError> {
    let dir = managed.instance_paths(id).config().join("profiles");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(storage(error)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(storage)?;
        let meta = fs::symlink_metadata(entry.path()).map_err(storage)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if meta.is_file() && !redirected(&meta) && name.ends_with(".json") && !name.contains(':') {
            names.push(name);
        }
    }
    names.sort();
    names.truncate(MAX_PROFILES);
    Ok(names)
}

fn snapshot(
    managed: &ManagedPaths,
    group: &GroupConfig,
    id: &InstanceId,
) -> Result<Snapshot, SyncError> {
    let paths = managed.instance_paths(id);
    let mut snap = Snapshot::default();
    if group.vanilla || group.packs {
        if let Some(bytes) = read_small(managed, &paths.root().join("options.txt"), MAX_OPTIONS)? {
            let (_, values) = options(&bytes)?;
            for key in VANILLA_KEYS {
                let pack_key = key == "resourcePacks";
                if (pack_key && group.packs) || (!pack_key && group.vanilla) {
                    if let Some(value) = values.get(key) {
                        snap.values
                            .insert(format!("o:{key}"), Value::String(value.clone()));
                    }
                }
            }
        }
    }
    if group.aurora {
        if let Some(config) = read_object(managed, &paths.config().join("aurora.json"))? {
            let marker = config.get("keybindVersion").cloned();
            for (field, value) in &config {
                if aurora_kind(field).is_some() {
                    let key = format!("a:{field}");
                    snap.markers.insert(key.clone(), marker.clone());
                    snap.values.insert(key, value.clone());
                }
            }
        }
        for file in profile_files(managed, id)? {
            let Some(profile) = read_object(managed, &paths.config().join("profiles").join(&file))?
            else {
                continue;
            };
            let Some(name) = profile.get("name").and_then(Value::as_str) else {
                continue;
            };
            snap.values
                .insert(format!("p:{file}:@name"), Value::String(name.to_owned()));
            for member in ["icon", "createdAtMs"] {
                if let Some(value) = profile.get(member) {
                    snap.values
                        .insert(format!("p:{file}:@{member}"), value.clone());
                }
            }
            if let Some(Value::Object(fields)) = profile.get("fields") {
                let marker = fields.get("keybindVersion").cloned();
                for (field, value) in fields {
                    if aurora_kind(field).is_some_and(|(_, profile)| profile) {
                        let key = format!("p:{file}:{field}");
                        snap.markers.insert(key.clone(), marker.clone());
                        snap.values.insert(key, value.clone());
                    }
                }
            }
        }
    }
    Ok(snap)
}

fn value_digest(value: &Value) -> String {
    digest(serde_json::to_string(value).unwrap_or_default().as_bytes())
}

// ---------------------------------------------------------------- capture

/// Records a member's edits into the group. A value differing from what the
/// group last saw in that member is an edit; the newest edit wins and the main
/// breaks exact ties. Pending members contribute nothing until seeded.
fn capture(
    managed: &ManagedPaths,
    group: &GroupConfig,
    state: &mut GroupState,
    id: &InstanceId,
    at: u64,
) -> Result<(), SyncError> {
    if group.pending_seed.contains(id) {
        return Ok(());
    }
    let main = group.main_instance_id.as_ref() == Some(id);
    if !main && !state.main_captured {
        return Ok(());
    }
    let snap = snapshot(managed, group, id)?;
    let seen = state.members.entry(id.to_string()).or_default();
    for (key, value) in snap.values {
        let hash = value_digest(&value);
        let edited = match seen.get(&key) {
            Some(previous) => *previous != hash,
            // First observation: the main defines the starting group values;
            // other members only fill keys the group does not know yet.
            None => main || !state.values.contains_key(&key),
        };
        seen.insert(key.clone(), hash);
        if !edited {
            continue;
        }
        let keep = state.values.get(&key).is_some_and(|existing| {
            existing.at > at
                || (existing.at == at
                    && group.main_instance_id.as_ref().map(InstanceId::as_str)
                        == Some(existing.source.as_str()))
        });
        if !keep {
            let marker = snap.markers.get(&key).cloned().flatten();
            let first = !state.main_captured && main;
            state.values.insert(
                key,
                Entry {
                    value,
                    at: if first { 0 } else { at },
                    source: id.to_string(),
                    marker,
                },
            );
        }
    }
    if main {
        state.main_captured = true;
    }
    Ok(())
}

// ------------------------------------------------------------------ apply

struct Change {
    file: PathBuf,
    bytes: Vec<u8>,
}

fn keybind_marker(map: &serde_json::Map<String, Value>) -> Option<Value> {
    map.get("keybindVersion").cloned()
}

fn applicable(entry: &Entry, kind: AuroraKind, target_marker: &Option<Value>) -> bool {
    kind != AuroraKind::KeyCombo || entry.marker == *target_marker
}

/// Builds the file updates that make `id` match the group. Only keys the
/// target document already supports are written; profiles missing from the
/// target are created with only synchronized fields.
fn plan(
    managed: &ManagedPaths,
    group: &GroupConfig,
    state: &GroupState,
    id: &InstanceId,
) -> Result<(Vec<Change>, Vec<String>), SyncError> {
    let paths = managed.instance_paths(id);
    let mut changes = Vec::new();
    let mut keys = Vec::new();

    // options.txt: replace values in place, preserving every other line.
    let options_path = paths.root().join("options.txt");
    if let Some(bytes) = read_small(managed, &options_path, MAX_OPTIONS)? {
        let (_, current) = options(&bytes)?;
        let text = String::from_utf8(bytes).map_err(storage)?;
        let mut updates = BTreeMap::new();
        for (key, entry) in &state.values {
            let Some(name) = key.strip_prefix("o:") else {
                continue;
            };
            if !(if name == "resourcePacks" {
                group.packs
            } else {
                group.vanilla
            }) {
                continue;
            }
            let (Some(old), Some(new)) = (current.get(name), entry.value.as_str()) else {
                continue;
            };
            if old != new && !new.contains(['\n', '\r']) {
                updates.insert(name.to_owned(), new.to_owned());
                keys.push(key.clone());
            }
        }
        if !updates.is_empty() {
            let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
            let mut out = String::with_capacity(text.len());
            for line in text.lines() {
                match line.split_once(':') {
                    Some((name, _)) if updates.contains_key(name) => {
                        out.push_str(name);
                        out.push(':');
                        out.push_str(&updates[name]);
                    }
                    _ => out.push_str(line),
                }
                out.push_str(newline);
            }
            changes.push(Change {
                file: options_path,
                bytes: out.into_bytes(),
            });
        }
    }

    // config/aurora.json: only fields the installed release already writes.
    let config_path = paths.config().join("aurora.json");
    let profiles_dir = paths.config().join("profiles");
    let existing_profiles = profile_files(managed, id)?;
    let group_profiles: BTreeSet<String> = state
        .values
        .keys()
        .filter_map(|k| k.strip_prefix("p:"))
        .filter_map(|k| k.split_once(':').map(|(file, _)| file.to_owned()))
        .collect();
    let aurora_config = if group.aurora {
        read_object(managed, &config_path)?
    } else {
        None
    };
    if let Some(mut config) = aurora_config {
        let marker = keybind_marker(&config);
        let mut changed = false;
        for (key, entry) in &state.values {
            let Some(field) = key.strip_prefix("a:") else {
                continue;
            };
            let Some((kind, _)) = aurora_kind(field) else {
                continue;
            };
            if !config.contains_key(field) || !applicable(entry, kind, &marker) {
                continue;
            }
            if kind == AuroraKind::ActiveProfile {
                let wanted = entry.value.as_str().unwrap_or_default();
                let available = group_profiles
                    .iter()
                    .chain(existing_profiles.iter())
                    .any(|file| {
                        state
                            .values
                            .get(&format!("p:{file}:@name"))
                            .and_then(|e| e.value.as_str())
                            == Some(wanted)
                    });
                if !available {
                    continue;
                }
            }
            if config.get(field) != Some(&entry.value) {
                config.insert(field.to_owned(), entry.value.clone());
                keys.push(key.clone());
                changed = true;
            }
        }
        if changed {
            changes.push(Change {
                file: config_path,
                bytes: serde_json::to_vec_pretty(&config).map_err(storage)?,
            });
        }

        // config/profiles/<file>.json: update existing, create missing ones.
        for file in &group_profiles {
            let path = profiles_dir.join(file);
            let prefix = format!("p:{file}:");
            let mut profile = if existing_profiles.contains(file) {
                match read_object(managed, &path)? {
                    Some(profile) => profile,
                    None => continue,
                }
            } else {
                let Some(name) = state.values.get(&format!("{prefix}@name")) else {
                    continue;
                };
                let mut fresh = serde_json::Map::new();
                fresh.insert("name".into(), name.value.clone());
                fresh.insert(
                    "createdAtMs".into(),
                    state
                        .values
                        .get(&format!("{prefix}@createdAtMs"))
                        .map_or(Value::from(now_ms()), |e| e.value.clone()),
                );
                fresh.insert("fields".into(), Value::Object(serde_json::Map::new()));
                fresh
            };
            let mut changed = !existing_profiles.contains(file);
            if let Some(icon) = state.values.get(&format!("{prefix}@icon")) {
                if profile.get("icon") != Some(&icon.value) {
                    profile.insert("icon".into(), icon.value.clone());
                    changed = true;
                }
            }
            let fields = profile
                .entry("fields")
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            let Value::Object(fields) = fields else {
                continue;
            };
            let marker = keybind_marker(fields);
            for (key, entry) in state.values.range(prefix.clone()..) {
                let Some(field) = key.strip_prefix(&prefix) else {
                    break;
                };
                if field.starts_with('@') {
                    continue;
                }
                let Some((kind, _)) = aurora_kind(field) else {
                    continue;
                };
                // Only fields this release understands (present in its config).
                if !config.contains_key(field) || !applicable(entry, kind, &marker) {
                    continue;
                }
                if fields.get(field) != Some(&entry.value) {
                    fields.insert(field.to_owned(), entry.value.clone());
                    keys.push(key.clone());
                    changed = true;
                }
            }
            if changed {
                changes.push(Change {
                    file: path,
                    bytes: serde_json::to_vec_pretty(&profile).map_err(storage)?,
                });
            }
        }
    }
    keys.sort();
    keys.dedup();
    Ok((changes, keys))
}

/// Copies packs other members have into `id`. Never deletes or replaces.
fn pool_packs(
    managed: &ManagedPaths,
    group: &GroupConfig,
    id: &InstanceId,
) -> Result<Vec<String>, SyncError> {
    let mut copied = Vec::new();
    let mut budget = MAX_POOL_BYTES;
    for folder in ["resourcepacks", "shaderpacks"] {
        let target = managed.instance_paths(id).root().join(folder);
        for source in group.members.iter().filter(|m| *m != id) {
            let dir = managed.instance_paths(source).root().join(folder);
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let destination = target.join(&name);
                if fs::symlink_metadata(&destination).is_ok() {
                    continue;
                }
                let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                    continue;
                };
                if redirected(&meta) {
                    continue;
                }
                fs::create_dir_all(&target).map_err(storage)?;
                if copy_tree(&entry.path(), &destination, &mut budget, 0).is_ok() {
                    copied.push(format!("{folder}/{}", name.to_string_lossy()));
                } else {
                    // A partial copy is removed rather than left half-pooled.
                    let _ =
                        fs::remove_dir_all(&destination).or_else(|_| fs::remove_file(&destination));
                }
            }
        }
    }
    Ok(copied)
}

fn copy_tree(from: &Path, to: &Path, budget: &mut u64, depth: u32) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(from)?;
    if redirected(&meta) || depth > 16 {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    if meta.is_file() {
        if meta.len() > MAX_FILE || meta.len() > *budget {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        *budget -= meta.len();
        let staging = to.with_extension("aurora-sync-part");
        fs::copy(from, &staging)?;
        return fs::rename(&staging, to);
    }
    if !meta.is_dir() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        copy_tree(
            &entry.path(),
            &to.join(entry.file_name()),
            budget,
            depth + 1,
        )?;
    }
    Ok(())
}

/// Backs up every file about to change, writes the new bytes atomically and
/// records a history entry. Nothing is written when there are no changes.
fn commit(
    managed: &ManagedPaths,
    id: &InstanceId,
    kind: &str,
    changes: Vec<Change>,
    keys: Vec<String>,
    packs: Vec<String>,
) -> Result<Option<HistoryEntry>, SyncError> {
    if changes.is_empty() && packs.is_empty() {
        return Ok(None);
    }
    let root = managed.instance_paths(id).root().to_path_buf();
    let entry_id = uuid::Uuid::new_v4().simple().to_string();
    let backup = backup_root(managed).join(&entry_id);
    let mut files = Vec::new();
    for change in &changes {
        let relative = change.file.strip_prefix(&root).map_err(|_| unsafe_path())?;
        let saved = backup.join(relative);
        fs::create_dir_all(saved.parent().unwrap_or(&backup)).map_err(storage)?;
        match read_small(managed, &change.file, MAX_JSON)? {
            Some(bytes) => fs::write(&saved, bytes).map_err(storage)?,
            // A file the sync creates is recorded so restore can remove it.
            None => fs::write(saved.with_extension("aurora-sync-absent"), b"").map_err(storage)?,
        }
        files.push(relative.to_string_lossy().replace('\\', "/"));
    }
    for change in changes {
        if let Some(parent) = change.file.parent() {
            fs::create_dir_all(parent).map_err(storage)?;
        }
        crate::install::journal::atomic_write(&change.file, &change.bytes).map_err(storage)?;
    }
    let entry = HistoryEntry {
        id: entry_id,
        instance_id: id.to_string(),
        at: now_ms(),
        kind: kind.to_owned(),
        keys: keys.into_iter().take(500).collect(),
        files,
        packs,
    };
    let mut history: Vec<HistoryEntry> = read_json(managed, &history_file(managed))?;
    history.insert(0, entry.clone());
    for old in history.drain(MAX_HISTORY.min(history.len())..) {
        let _ = fs::remove_dir_all(backup_root(managed).join(old.id));
    }
    write_json(&history_file(managed), &history)?;
    Ok(Some(entry))
}

/// Seeds a pending member or applies group values to a member, then records
/// what it now holds so its own later edits are recognized.
fn apply_member(
    managed: &ManagedPaths,
    group: &mut GroupConfig,
    state: &mut GroupState,
    id: &InstanceId,
    report: &mut SyncReport,
) -> Result<(), SyncError> {
    let seeding = group.pending_seed.contains(id);
    if !state.main_captured {
        report.deferred.push(id.to_string());
        return Ok(());
    }
    let packs = if group.packs {
        pool_packs(managed, group, id)?
    } else {
        vec![]
    };
    let (changes, keys) = plan(managed, group, state, id)?;
    report.changed_settings += keys.len();
    report.copied_packs += packs.len();
    crate::instance_content::with_instance_lock(id, || {
        Ok(commit(
            managed,
            id,
            if seeding { "seed" } else { "apply" },
            changes,
            keys,
            packs,
        ))
    })
    .map_err(|_| {
        fail(
            "sync_group_busy",
            "This instance is busy; its settings were not changed.",
        )
    })??;
    group.pending_seed.retain(|m| m != id);
    // Whatever the member holds now is acknowledged as the group's value.
    let snap = snapshot(managed, group, id)?;
    let seen = state.members.entry(id.to_string()).or_default();
    seen.clear();
    for (key, value) in snap.values {
        seen.insert(key, value_digest(&value));
    }
    report.applied.push(id.to_string());
    Ok(())
}

fn capture_stopped(
    managed: &ManagedPaths,
    group: &GroupConfig,
    state: &mut GroupState,
    skip: Option<&InstanceId>,
) -> Result<(), SyncError> {
    let at = now_ms();
    let mut order: Vec<&InstanceId> = group.members.iter().collect();
    // The main first, so a fresh group starts from its values.
    order.sort_by_key(|id| Some(*id) != group.main_instance_id.as_ref());
    for id in order {
        if Some(id) != skip && stopped(managed, id, false) {
            capture(managed, group, state, id, at)?;
        }
    }
    Ok(())
}

/// Runs right before `id` launches, inside its Play reservation. Joins a new
/// instance when configured, captures stopped members, then makes `id` match
/// the group. Returns `None` when the group does not manage this instance.
pub fn before_launch(
    managed: &ManagedPaths,
    id: &InstanceId,
) -> Result<Option<SyncReport>, SyncError> {
    let mut group = config(managed)?;
    if !group.enabled || group.main_instance_id.is_none() {
        return Ok(None);
    }
    let _lock = lock(managed)?;
    if !group.members.contains(id) {
        if group.new_instances_join && !group.known.contains(id) && group.members.len() < 64 {
            group.members.push(id.clone());
            group.pending_seed.push(id.clone());
            group.known.push(id.clone());
            write_json(&config_file(managed), &group)?;
        } else {
            return Ok(None);
        }
    }
    let mut state: GroupState = read_json(managed, &state_file(managed))?;
    capture_stopped(managed, &group, &mut state, Some(id))?;
    if stopped(managed, id, true) {
        capture(managed, &group, &mut state, id, now_ms())?;
    }
    let mut report = SyncReport::default();
    apply_member(managed, &mut group, &mut state, id, &mut report)?;
    write_json(&state_file(managed), &state)?;
    write_json(&config_file(managed), &group)?;
    report.message = summary(&report);
    Ok(Some(report))
}

/// Runs after a member's game has fully exited: its settings edits enter the
/// group with the exit time as their clock.
pub fn after_exit(managed: &ManagedPaths, id: &InstanceId) -> Result<(), SyncError> {
    let group = config(managed)?;
    if !group.enabled || !group.members.contains(id) {
        return Ok(());
    }
    let _lock = lock(managed)?;
    if !stopped(managed, id, false) {
        return Ok(());
    }
    let mut state: GroupState = read_json(managed, &state_file(managed))?;
    capture(managed, &group, &mut state, id, now_ms())?;
    write_json(&state_file(managed), &state)
}

/// Captures every stopped member, then brings every stopped member up to date.
pub fn sync_now(managed: &ManagedPaths) -> Result<SyncReport, SyncError> {
    let mut group = config(managed)?;
    if !group.enabled || group.main_instance_id.is_none() {
        return Err(fail("sync_group_off", "Turn on the synced group first."));
    }
    let _lock = lock(managed)?;
    let mut state: GroupState = read_json(managed, &state_file(managed))?;
    capture_stopped(managed, &group, &mut state, None)?;
    let mut report = SyncReport::default();
    for id in group.members.clone() {
        if stopped(managed, &id, false) {
            apply_member(managed, &mut group, &mut state, &id, &mut report)?;
        } else {
            report.deferred.push(id.to_string());
        }
    }
    write_json(&state_file(managed), &state)?;
    write_json(&config_file(managed), &group)?;
    report.message = summary(&report);
    Ok(report)
}

fn summary(report: &SyncReport) -> String {
    let mut text = format!(
        "{} setting{} updated",
        report.changed_settings,
        if report.changed_settings == 1 {
            ""
        } else {
            "s"
        }
    );
    if report.copied_packs > 0 {
        text.push_str(&format!(
            ", {} pack{} added",
            report.copied_packs,
            if report.copied_packs == 1 { "" } else { "s" }
        ));
    }
    if !report.deferred.is_empty() {
        text.push_str(&format!(
            "; {} instance{} skipped while running or waiting for the main to close",
            report.deferred.len(),
            if report.deferred.len() == 1 { "" } else { "s" }
        ));
    }
    text.push('.');
    text
}

pub fn history(managed: &ManagedPaths) -> Result<Vec<HistoryEntry>, SyncError> {
    read_json(managed, &history_file(managed))
}

/// Puts an instance's files back as they were before a sync and makes it
/// independent, so the restored values are not reapplied or spread.
pub fn restore(managed: &ManagedPaths, entry_id: &str) -> Result<GroupConfig, SyncError> {
    let _lock = lock(managed)?;
    let history: Vec<HistoryEntry> = read_json(managed, &history_file(managed))?;
    let entry = history
        .iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| fail("sync_restore_missing", "That backup no longer exists."))?;
    let id = InstanceId::new(&entry.instance_id).map_err(|_| unsafe_path())?;
    if !stopped(managed, &id, false) {
        return Err(fail(
            "sync_restore_running",
            "Close the game for this instance before restoring its settings.",
        ));
    }
    let root = managed.instance_paths(&id).root().to_path_buf();
    let backup = backup_root(managed).join(&entry.id);
    for relative in &entry.files {
        validate_relative(relative)?;
        let saved = backup.join(relative);
        let target = root.join(relative);
        if saved.with_extension("aurora-sync-absent").exists() {
            let _ = fs::remove_file(&target);
        } else {
            let bytes = fs::read(&saved).map_err(storage)?;
            crate::install::journal::atomic_write(&target, &bytes).map_err(storage)?;
        }
    }
    let mut group = config(managed)?;
    group.members.retain(|m| *m != id);
    group.pending_seed.retain(|m| *m != id);
    write_json(&config_file(managed), &group)?;
    let mut state: GroupState = read_json(managed, &state_file(managed))?;
    state.members.remove(id.as_str());
    write_json(&state_file(managed), &state)?;
    Ok(group)
}

pub fn overview(managed: &ManagedPaths) -> Result<Overview, SyncError> {
    let group = config(managed)?;
    let registry = InstanceRegistry::load(&managed.instance_registry_file()).map_err(storage)?;
    let legacy = super::policy(managed).ok();
    let instances = registry
        .instances()
        .iter()
        .map(|record| {
            let id = record.id();
            MemberView {
                id: id.to_string(),
                name: record.display_name().to_owned(),
                minecraft_version: record.installed().minecraft_version.clone(),
                member: if group.main_instance_id.is_some() {
                    group.members.contains(id)
                } else {
                    legacy.as_ref().is_some_and(|p| p.instances.contains(id))
                },
                main: group.main_instance_id.as_ref() == Some(id),
                pending_seed: group.pending_seed.contains(id),
                running: !stopped(managed, id, false),
            }
        })
        .collect();
    let suggested_main = legacy.as_ref().and_then(|p| {
        let bytes =
            read_small(managed, &managed.gameplay_history_file(), 16 * 1024 * 1024).ok()??;
        crate::gameplay_history::most_recent_member(&bytes, &p.instances).map(|id| id.to_string())
    });
    Ok(Overview {
        config: group,
        instances,
        history: history(managed)?.into_iter().take(30).collect(),
        suggested_main,
    })
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
