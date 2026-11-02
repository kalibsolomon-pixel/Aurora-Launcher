//! Selected automatic synchronization over reviewed per-file commits. No watchers, shared mutable directories,
//! timestamp winners, deletions, provider adoption or network work. Local hashes
//! bind the review to observed bytes; they are not publisher verification.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::install::coordination::{TargetLock, checked_directory, ensure_directory, redirected};
use crate::instances::{InstanceId, InstanceRegistry, InstanceState};
use crate::paths::ManagedPaths;

pub mod automatic;
mod files;
pub use files::{Conflict, conflicts, resolve as resolve_conflict};
mod servers;

// Internal hash-bound selection, never a frontend replacement authority.
type FileSelection = BTreeMap<String, (String, Option<String>)>;

#[derive(Clone, Copy, Default)]
struct Counters {
    scanned: u64,
    hashed: u64,
    copied: u64,
}
thread_local! { static COUNTERS: std::cell::Cell<Counters> = const { std::cell::Cell::new(Counters{scanned:0,hashed:0,copied:0}) }; }
fn record_count(edit: impl FnOnce(&mut Counters)) {
    COUNTERS.with(|c| {
        let mut value = c.get();
        edit(&mut value);
        c.set(value);
    });
}
const MAX_FILES: usize = 4096;
const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_TOTAL: u64 = 1024 * 1024 * 1024;
const MAX_OPTIONS: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    pub schema_version: u32,
    pub enabled: bool,
    pub instances: Vec<InstanceId>,
    pub categories: Vec<Category>,
    pub exclusions: Vec<String>,
    pub selected_paths: Vec<String>,
    #[serde(default)]
    pub overrides: BTreeMap<String, InstanceOverride>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            schema_version: 2,
            enabled: false,
            instances: vec![],
            categories: vec![
                Category::Settings,
                Category::Controls,
                Category::ResourcePacks,
                Category::ShaderPacks,
                Category::Screenshots,
            ],
            exclusions: vec![],
            selected_paths: vec![],
            overrides: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceOverride {
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub categories: BTreeMap<Category, bool>,
    #[serde(default)]
    pub exclusions: Vec<String>,
}
impl Policy {
    fn effective(&self, id: &InstanceId) -> Self {
        let mut result = self.clone();
        if let Some(value) = self.overrides.get(id.as_str()) {
            result.enabled &= !value.disabled;
            for (category, enabled) in &value.categories {
                result.categories.retain(|c| c != category);
                if *enabled {
                    result.categories.push(*category);
                }
            }
            result.exclusions.extend(value.exclusions.clone());
        }
        result
    }
    fn has_content(&self) -> bool {
        !self.categories.is_empty()
            || self
                .overrides
                .values()
                .any(|v| v.categories.values().any(|v| *v))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Settings,
    Controls,
    ResourcePacks,
    ShaderPacks,
    Screenshots,
    Configuration,
    Servers,
}
impl Category {
    fn root(self) -> &'static str {
        match self {
            Self::Settings | Self::Controls => "options.txt",
            Self::ResourcePacks => "resourcepacks",
            Self::ShaderPacks => "shaderpacks",
            Self::Screenshots => "screenshots",
            Self::Configuration => "config",
            Self::Servers => "servers.dat",
        }
    }
    fn options(self) -> bool {
        matches!(self, Self::Settings | Self::Controls)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub source_id: InstanceId,
    pub destination_id: InstanceId,
    pub category: Category,
    /// Exact relative paths or directory prefixes; never glob expressions.
    pub exclusions: Vec<String>,
    /// Configuration sync never enumerates all config implicitly. The user
    /// must select exact config/... files or directory roots for this run.
    #[serde(default)]
    pub selected_paths: Vec<String>,
    #[serde(default)]
    pub key_filter: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: String,
    /// copyMissing, mergeSettings, identical, or keepConflict.
    pub action: &'static str,
    pub changed_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub fingerprint: String,
    pub entries: Vec<Entry>,
    pub changes: usize,
    pub conflicts: usize,
    pub bytes_to_copy: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub copied: usize,
    pub conflicts_kept: usize,
    pub backup_id: Option<String>,
}

#[derive(Debug)]
pub struct SyncError {
    pub code: &'static str,
    pub message: &'static str,
}
impl std::fmt::Display for SyncError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}
impl std::error::Error for SyncError {}
fn fail(code: &'static str, message: &'static str) -> SyncError {
    SyncError { code, message }
}
fn storage<T>(_: T) -> SyncError {
    fail(
        "sync_storage",
        "Sync could not safely read or write its selected files. Completed files and backups are retained; automatic syncing will retry on the next minute.",
    )
}
fn unsafe_path() -> SyncError {
    fail(
        "sync_unsafe_path",
        "Sync refuses redirected, special, unsafe or oversized paths.",
    )
}
fn stale() -> SyncError {
    fail(
        "sync_review_stale",
        "Files, instances or sync preferences changed. Automatic syncing will retry on the next minute.",
    )
}

fn policy_file(managed: &ManagedPaths) -> PathBuf {
    managed.launcher_dir().join("instance-sync.json")
}
pub fn policy(managed: &ManagedPaths) -> Result<Policy, SyncError> {
    if !managed.launcher_dir().exists() {
        return Ok(Policy::default());
    }
    checked_directory(managed.data_root(), &managed.launcher_dir()).map_err(storage)?;
    let Some(bytes) = read_small(managed, &policy_file(managed), 256 * 1024)? else {
        return Ok(Policy::default());
    };
    let raw: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        fail(
            "sync_policy_invalid",
            "Sync preferences are malformed and were left unchanged.",
        )
    })?;
    if raw.get("schemaVersion").and_then(serde_json::Value::as_u64) == Some(1) {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Legacy {
            schema_version: u32,
            enabled: bool,
        }
        let legacy: Legacy = serde_json::from_value(raw).map_err(|_| {
            fail(
                "sync_policy_invalid",
                "Sync preferences are malformed and were left unchanged.",
            )
        })?;
        let _ = (legacy.schema_version, legacy.enabled);
        // The known manual schema has no participant authority. The new
        // automatic mode starts off until the owner explicitly selects it.
        return Ok(Policy::default());
    }
    if raw.get("schemaVersion").and_then(serde_json::Value::as_u64) != Some(2) {
        return Err(fail(
            "sync_schema_unsupported",
            "This sync preference schema is unsupported and was left unchanged.",
        ));
    }
    let value: Policy = serde_json::from_value(raw).map_err(|_| {
        fail(
            "sync_policy_invalid",
            "Sync preferences are malformed and were left unchanged.",
        )
    })?;
    if value.schema_version != 2 {
        return Err(fail(
            "sync_schema_unsupported",
            "This sync preference schema is unsupported and was left unchanged.",
        ));
    }
    validate_policy(&value)?;
    Ok(value)
}
fn validate_policy(value: &Policy) -> Result<(), SyncError> {
    if value.schema_version != 2
        || value.instances.len() > 64
        || value.categories.len() > 7
        || value.overrides.len() > 64
        || value.exclusions.len() > 128
        || value.selected_paths.len() > 128
    {
        return Err(unsafe_path());
    }
    let unique: BTreeSet<_> = value.instances.iter().map(InstanceId::as_str).collect();
    if unique.len() != value.instances.len() {
        return Err(unsafe_path());
    }
    for (id, overrides) in &value.overrides {
        if InstanceId::new(id).is_err()
            || !value.instances.iter().any(|v| v.as_str() == id)
            || overrides.categories.len() > 7
            || overrides.exclusions.len() > 128
        {
            return Err(unsafe_path());
        }
        for path in &overrides.exclusions {
            validate_relative(path)?;
        }
    }
    let mut categories = BTreeSet::new();
    for category in &value.categories {
        if !categories.insert(format!("{category:?}")) {
            return Err(unsafe_path());
        }
    }
    for path in value.exclusions.iter().chain(&value.selected_paths) {
        validate_relative(path)?;
    }
    if value
        .selected_paths
        .iter()
        .any(|path| !path.starts_with("config/"))
    {
        return Err(unsafe_path());
    }
    Ok(())
}
pub fn save_policy(managed: &ManagedPaths, proposed: Policy) -> Result<Policy, SyncError> {
    validate_policy(&proposed)?;
    let _guard = TargetLock::acquire(
        managed.data_root(),
        &managed.launcher_dir(),
        ".sync-settings.lock",
    )
    .map_err(storage)?;
    let current = policy(managed)?; // Never overwrite damaged/unknown preferences.
    let registry = InstanceRegistry::load(&managed.instance_registry_file()).map_err(storage)?;
    if proposed
        .instances
        .iter()
        .any(|id| registry.find(id).is_none() && !current.instances.contains(id))
    {
        return Err(stale());
    }
    crate::install::journal::atomic_write(
        &policy_file(managed),
        &serde_json::to_vec(&proposed).map_err(storage)?,
    )
    .map_err(storage)?;
    Ok(proposed)
}
pub fn set_enabled(managed: &ManagedPaths, enabled: bool) -> Result<Policy, SyncError> {
    let _lock = TargetLock::acquire(
        managed.data_root(),
        &managed.launcher_dir(),
        ".sync-settings.lock",
    )
    .map_err(storage)?;
    let mut current = policy(managed)?;
    current.enabled = enabled;
    crate::install::journal::atomic_write(
        &policy_file(managed),
        &serde_json::to_vec(&current).map_err(storage)?,
    )
    .map_err(storage)?;
    Ok(current)
}

#[derive(Serialize)]
struct FilePlan {
    path: String,
    source: String,
    destination: Option<String>,
    size: u64,
    #[serde(skip)]
    merged: Option<Vec<u8>>,
    output_hash: Option<String>,
    action: &'static str,
    changed_keys: Vec<String>,
}

fn validate_relative(path: &str) -> Result<(), SyncError> {
    if path.len() > 1024
        || path.is_empty()
        || path.split('/').any(|part| {
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with(['.', ' '])
                || part
                    .chars()
                    .any(|c| c.is_control() || "\\:*?\"<>|".contains(c))
                || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || (stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.as_bytes()[3].is_ascii_digit())
        })
    {
        return Err(unsafe_path());
    }
    Ok(())
}

fn excluded(request: &Request, path: &str) -> bool {
    request.exclusions.iter().any(|prefix| {
        path.eq_ignore_ascii_case(prefix)
            || path
                .to_ascii_lowercase()
                .starts_with(&(prefix.to_ascii_lowercase() + "/"))
    })
}

fn regular_file(managed: &ManagedPaths, path: &Path) -> Result<Option<File>, SyncError> {
    // Inspect every ancestor, including missing descendants, before opening.
    let relative = path.strip_prefix(managed.data_root()).map_err(storage)?;
    let mut current = managed.data_root().to_path_buf();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(unsafe_path());
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) if redirected(&meta) => return Err(unsafe_path()),
            Ok(meta) if current != path && !meta.is_dir() => return Err(unsafe_path()),
            Ok(meta) if current == path && (!meta.is_file() || meta.len() > MAX_FILE) => {
                return Err(unsafe_path());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(storage(error)),
        }
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1).custom_flags(0x0020_0000);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path).map_err(storage)?;
    let meta = file.metadata().map_err(storage)?;
    if !meta.is_file() || redirected(&meta) || meta.len() > MAX_FILE {
        return Err(unsafe_path());
    }
    Ok(Some(file))
}

fn read_small(
    managed: &ManagedPaths,
    path: &Path,
    limit: u64,
) -> Result<Option<Vec<u8>>, SyncError> {
    let Some(file) = regular_file(managed, path)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(storage)?;
    if bytes.len() as u64 > limit {
        return Err(unsafe_path());
    }
    Ok(Some(bytes))
}
fn file_digest(bytes: &[u8]) -> String {
    record_count(|c| c.hashed += 1);
    digest(bytes)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hash_file(managed: &ManagedPaths, path: &Path) -> Result<Option<(String, u64)>, SyncError> {
    let Some(mut file) = regular_file(managed, path)? else {
        return Ok(None);
    };
    record_count(|c| c.hashed += 1);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0;
    loop {
        let read = file.read(&mut buffer).map_err(storage)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_FILE {
            return Err(unsafe_path());
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Some((format!("{:x}", hasher.finalize()), total)))
}

fn options(bytes: &[u8]) -> Result<(Vec<String>, BTreeMap<String, String>), SyncError> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        fail(
            "sync_options_invalid",
            "Options must be valid UTF-8 with one unambiguous value per key.",
        )
    })?;
    let mut order = Vec::new();
    let mut values = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(fail(
                "sync_options_invalid",
                "Options contain an unsupported line; no settings were changed.",
            ));
        };
        if key.is_empty()
            || line.chars().any(|c| c.is_control())
            || values.insert(key.to_owned(), value.to_owned()).is_some()
        {
            return Err(fail(
                "sync_options_invalid",
                "Options contain duplicate or invalid keys; no settings were changed.",
            ));
        }
        order.push(key.to_owned());
    }
    Ok((order, values))
}

fn merge_options(
    source: &[u8],
    destination: &[u8],
    category: Category,
) -> Result<(Vec<u8>, Vec<String>), SyncError> {
    let (_, source) = options(source)?;
    let (order, mut target) = options(destination)?;
    let mut keys = Vec::new();
    for (key, value) in source {
        // Only common keys are eligible. Pack activation/order, data version,
        // paths, unknown mod settings and version-specific keys stay local.
        let allowed = if category == Category::Controls {
            key.starts_with("key_")
        } else {
            matches!(
                key.as_str(),
                "mouseSensitivity"
                    | "invertYMouse"
                    | "autoJump"
                    | "toggleCrouch"
                    | "toggleSprint"
                    | "subtitles"
                    | "showSubtitles"
                    | "chatOpacity"
                    | "chatScale"
                    | "chatWidth"
                    | "chatHeightFocused"
                    | "chatHeightUnfocused"
            ) || key.starts_with("soundCategory_")
        };
        if allowed && target.get(&key).is_some_and(|current| current != &value) {
            target.insert(key.clone(), value);
            keys.push(key);
        }
    }
    if keys.is_empty() {
        return Ok((destination.to_vec(), keys));
    }
    let mut result = String::new();
    for key in order {
        result.push_str(&key);
        result.push(':');
        result.push_str(&target[&key]);
        result.push('\n');
    }
    if result.len() as u64 > MAX_OPTIONS {
        return Err(unsafe_path());
    }
    Ok((result.into_bytes(), keys))
}

fn collect(
    managed: &ManagedPaths,
    root: &Path,
    request: &Request,
) -> Result<Vec<String>, SyncError> {
    if request.category.options() || request.category == Category::Servers {
        return Ok(if excluded(request, request.category.root()) {
            vec![]
        } else {
            vec![request.category.root().into()]
        });
    }
    let start = request.category.root().to_owned();
    let mut pending = if request.category == Category::Configuration {
        request
            .selected_paths
            .iter()
            .cloned()
            .map(|path| (path, 0))
            .collect()
    } else {
        vec![(start, 0)]
    };
    let mut result = Vec::new();
    let mut count = 0;
    while let Some((relative, depth)) = pending.pop() {
        validate_relative(&relative)?;
        if excluded(request, &relative) {
            continue;
        }
        count += 1;
        if count > MAX_FILES || depth > 16 {
            return Err(unsafe_path());
        }
        let path = root.join(&relative);
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(storage(error)),
        };
        if redirected(&meta) {
            return Err(unsafe_path());
        }
        if meta.is_dir() {
            checked_directory(managed.data_root(), &path).map_err(storage)?;
            for entry in fs::read_dir(&path).map_err(storage)? {
                let name = entry
                    .map_err(storage)?
                    .file_name()
                    .into_string()
                    .map_err(|_| unsafe_path())?;
                // A crash may leave our exact private staging name. It is
                // never user content or a source for a later sync.
                if name
                    .strip_prefix(".aurora-sync-")
                    .is_some_and(|id| id.len() == 32 && id.bytes().all(|c| c.is_ascii_hexdigit()))
                {
                    count += 1;
                    if count >= MAX_FILES {
                        return Err(unsafe_path());
                    }
                    continue;
                }
                if pending.len() + count >= MAX_FILES {
                    return Err(unsafe_path());
                }
                pending.push((format!("{relative}/{name}"), depth + 1));
            }
        } else if meta.is_file() {
            record_count(|c| c.scanned += 1);
            result.push(relative);
        } else {
            return Err(unsafe_path());
        }
    }
    result.sort();
    let mut names = BTreeSet::new();
    if result.iter().any(|p| !names.insert(p.to_ascii_lowercase())) {
        return Err(unsafe_path());
    }
    Ok(result)
}

fn plan(managed: &ManagedPaths, request: &Request) -> Result<(Preview, Vec<FilePlan>), SyncError> {
    plan_files(managed, request, None)
}
fn plan_files(
    managed: &ManagedPaths,
    request: &Request,
    selection: Option<&FileSelection>,
) -> Result<(Preview, Vec<FilePlan>), SyncError> {
    if request.key_filter.as_ref().is_some_and(|keys| {
        !request.category.options() || keys.len() > 1024 || keys.iter().any(|key| key.len() > 256)
    }) {
        return Err(unsafe_path());
    }
    if !policy(managed)?.enabled {
        return Err(fail(
            "sync_disabled",
            "Enable instance syncing before reviewing or applying it.",
        ));
    }
    if request.source_id == request.destination_id
        || request.exclusions.len() > 384
        || request.selected_paths.len() > 128
    {
        return Err(unsafe_path());
    }
    if request.category == Category::Configuration {
        if request.selected_paths.is_empty() {
            return Err(fail(
                "sync_config_selection_required",
                "Choose specific config/... files or directories before reviewing configuration sync.",
            ));
        }
        for path in &request.selected_paths {
            validate_relative(path)?;
            if !path.starts_with("config/") {
                return Err(unsafe_path());
            }
        }
    } else if !request.selected_paths.is_empty() {
        return Err(unsafe_path());
    }
    for path in &request.exclusions {
        validate_relative(path)?;
    }
    let registry = InstanceRegistry::load(&managed.instance_registry_file()).map_err(storage)?;
    let source = registry.find(&request.source_id).ok_or_else(stale)?;
    let destination = registry.find(&request.destination_id).ok_or_else(stale)?;
    if [source, destination]
        .iter()
        .any(|r| r.state() != InstanceState::Ready)
    {
        return Err(fail(
            "sync_instance_not_ready",
            "Both instances must have completed installation before syncing.",
        ));
    }
    if (request.category.options() || request.category == Category::Configuration)
        && (source.installed().minecraft_version != destination.installed().minecraft_version
            || source.installed().platform != destination.installed().platform)
    {
        return Err(fail(
            "sync_settings_incompatible",
            "Settings, controls and configuration sync requires matching installed Minecraft and loader versions.",
        ));
    }
    let source_root = managed
        .instance_paths(&request.source_id)
        .root()
        .to_path_buf();
    let destination_root = managed
        .instance_paths(&request.destination_id)
        .root()
        .to_path_buf();
    checked_directory(managed.data_root(), &source_root).map_err(storage)?;
    checked_directory(managed.data_root(), &destination_root).map_err(storage)?;
    let mut files = Vec::new();
    let mut total = 0;
    let paths = match selection {
        Some(files) => files.keys().cloned().collect(),
        None => collect(managed, &source_root, request)?,
    };
    for path in paths {
        validate_relative(&path)?;
        if path != request.category.root()
            && !path.starts_with(&format!("{}/", request.category.root()))
        {
            return Err(unsafe_path());
        }
        if request.category == Category::Configuration
            && !request
                .selected_paths
                .iter()
                .any(|p| path == *p || path.starts_with(&format!("{p}/")))
        {
            return Err(unsafe_path());
        }
        if excluded(request, &path) {
            return Err(stale());
        }
        let Some((hash, size)) = hash_file(managed, &source_root.join(&path))? else {
            continue;
        };
        total += size;
        if total > MAX_TOTAL {
            return Err(unsafe_path());
        }
        let old = hash_file(managed, &destination_root.join(&path))?.map(|(h, _)| h);
        if let Some(selection) = selection {
            if selection.get(&path) != Some(&(hash.clone(), old.clone())) {
                return Err(stale());
            }
        }
        let (merged, changed_keys, action) = if request.category == Category::Servers {
            let source =
                read_small(managed, &source_root.join(&path), MAX_OPTIONS)?.ok_or_else(stale)?;
            let target = read_small(managed, &destination_root.join(&path), MAX_OPTIONS)?;
            if file_digest(&source) != hash || target.as_ref().map(|b| file_digest(b)) != old {
                return Err(stale());
            }
            let merged = servers::merge(&source, target.as_deref())?;
            let action = if merged.conflicts > 0 {
                "keepConflict"
            } else if target.as_deref() == Some(merged.bytes.as_slice()) {
                "identical"
            } else {
                "mergeServers"
            };
            (Some(merged.bytes), vec![], action)
        } else if request.category.options() {
            let source =
                read_small(managed, &source_root.join(&path), MAX_OPTIONS)?.ok_or_else(stale)?;
            let target = read_small(managed, &destination_root.join(&path), MAX_OPTIONS)?
                .ok_or_else(|| fail("sync_options_missing", "Open the destination game once to create its version-specific options before syncing settings."))?;
            if file_digest(&source) != hash || Some(file_digest(&target)) != old {
                return Err(stale());
            }
            let source = if let Some(filter) = &request.key_filter {
                if filter.len() > 1024 {
                    return Err(unsafe_path());
                }
                let (_, values) = options(&source)?;
                values
                    .into_iter()
                    .filter(|(key, _)| filter.contains(key))
                    .map(|(key, value)| format!("{key}:{value}\n"))
                    .collect::<String>()
                    .into_bytes()
            } else {
                source
            };
            let (merged, keys) = merge_options(&source, &target, request.category)?;
            let action = if keys.is_empty() {
                "identical"
            } else {
                "mergeSettings"
            };
            (Some(merged), keys, action)
        } else {
            (
                None,
                vec![],
                match old.as_ref() {
                    None => "copyMissing",
                    Some(h) if h == &hash => "identical",
                    Some(_) if selection.is_some() => "replaceFile",
                    Some(_) => "keepConflict",
                },
            )
        };
        files.push(FilePlan {
            path,
            source: hash,
            destination: old,
            size,
            output_hash: merged.as_ref().map(|b| file_digest(b)),
            merged,
            action,
            changed_keys,
        });
    }
    let fingerprint = digest(
        &serde_json::to_vec(&(request, policy(managed)?, source, destination, &files))
            .map_err(storage)?,
    );
    let preview = Preview {
        fingerprint,
        entries: files
            .iter()
            .map(|f| Entry {
                path: f.path.clone(),
                action: f.action,
                changed_keys: f.changed_keys.clone(),
            })
            .collect(),
        changes: files
            .iter()
            .filter(|f| {
                matches!(
                    f.action,
                    "copyMissing" | "mergeSettings" | "replaceFile" | "mergeServers"
                )
            })
            .count(),
        conflicts: files.iter().filter(|f| f.action == "keepConflict").count(),
        bytes_to_copy: files
            .iter()
            .filter(|f| {
                matches!(
                    f.action,
                    "copyMissing" | "mergeSettings" | "replaceFile" | "mergeServers"
                )
            })
            .map(|f| f.merged.as_ref().map_or(f.size, |b| b.len() as u64))
            .sum(),
    };
    Ok((preview, files))
}

fn locked<T>(
    managed: &ManagedPaths,
    request: &Request,
    work: impl FnOnce() -> Result<T, SyncError>,
) -> Result<T, SyncError> {
    if request.source_id == request.destination_id {
        return Err(unsafe_path());
    }
    // Logical Play reservations exclude both in-flight preparations. Kernel
    // operation locks exclude other Aurora installation/sync writers.
    let registry = InstanceRegistry::load(&managed.instance_registry_file()).map_err(storage)?;
    if registry.find(&request.source_id).is_none()
        || registry.find(&request.destination_id).is_none()
    {
        return Err(stale());
    }
    let _source_play = crate::launch::process::PreparationGuard::acquire(
        request.source_id.as_str(),
    )
    .map_err(|_| {
        fail(
            "sync_instance_busy",
            "Stop both selected games and wait for active operations before syncing.",
        )
    })?;
    let _destination_play =
        crate::launch::process::PreparationGuard::acquire(request.destination_id.as_str())
            .map_err(|_| {
                fail(
                    "sync_instance_busy",
                    "Stop both selected games and wait for active operations before syncing.",
                )
            })?;
    let mut ids = [&request.source_id, &request.destination_id];
    ids.sort_by_key(|id| id.as_str());
    let _a =
        crate::instances::lifecycle::lock_instance_operation(managed, ids[0]).map_err(storage)?;
    let _b =
        crate::instances::lifecycle::lock_instance_operation(managed, ids[1]).map_err(storage)?;
    let _policy = TargetLock::acquire(
        managed.data_root(),
        &managed.launcher_dir(),
        ".instance-sync.lock",
    )
    .map_err(storage)?;
    // Content activation (including resource-pack options) has its own
    // process-local writer boundary. Share it as well as lifecycle locks.
    crate::instance_content::with_instance_lock(ids[0], || {
        Ok(crate::instance_content::with_instance_lock(ids[1], || {
            Ok(work())
        }))
    })
    .map_err(|_| {
        fail(
            "sync_instance_busy",
            "Selected instance content is being changed. Try again after it finishes.",
        )
    })?
    .map_err(|_| {
        fail(
            "sync_instance_busy",
            "Selected instance content is being changed. Try again after it finishes.",
        )
    })?
}
pub fn preview(managed: &ManagedPaths, request: &Request) -> Result<Preview, SyncError> {
    locked(managed, request, || {
        plan(managed, request).map(|(preview, _)| preview)
    })
}

pub fn apply(
    managed: &ManagedPaths,
    request: &Request,
    fingerprint: &str,
) -> Result<Receipt, SyncError> {
    apply_with_checkpoint(managed, request, fingerprint, |_| Ok(()))
}

fn apply_with_checkpoint(
    managed: &ManagedPaths,
    request: &Request,
    fingerprint: &str,
    mut checkpoint: impl FnMut(&'static str) -> Result<(), SyncError>,
) -> Result<Receipt, SyncError> {
    apply_files(managed, request, fingerprint, None, &mut checkpoint)
}
fn apply_files(
    managed: &ManagedPaths,
    request: &Request,
    fingerprint: &str,
    selection: Option<&FileSelection>,
    checkpoint: &mut impl FnMut(&'static str) -> Result<(), SyncError>,
) -> Result<Receipt, SyncError> {
    locked(managed, request, || {
        let active_policy = policy(managed)?;
        let (preview, files) = plan_files(managed, request, selection)?;
        if preview.fingerprint != fingerprint {
            return Err(stale());
        }
        let source_root = managed
            .instance_paths(&request.source_id)
            .root()
            .to_path_buf();
        let target_root = managed
            .instance_paths(&request.destination_id)
            .root()
            .to_path_buf();
        let mut receipt = Receipt {
            copied: 0,
            conflicts_kept: preview.conflicts,
            backup_id: None,
        };
        if preview.changes == 0 {
            return Ok(receipt);
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        let backup = managed.launcher_dir().join("sync-backups").join(&id);
        ensure_directory(managed.data_root(), &backup).map_err(storage)?;
        // Receipt exists before any destination mutation. A crash leaves the
        // reviewed inventory plus individual complete files/backups, never a
        // partial replacement. There is no automatic rollback of user content.
        crate::install::journal::atomic_write(
            &backup.join("review.json"),
            &serde_json::to_vec(&(request, &preview)).map_err(storage)?,
        )
        .map_err(storage)?;
        receipt.backup_id = Some(id);
        checkpoint("reviewSaved")?;
        for file in files.iter().filter(|f| {
            matches!(
                f.action,
                "copyMissing" | "mergeSettings" | "replaceFile" | "mergeServers"
            )
        }) {
            if policy(managed)? != active_policy {
                return Err(stale());
            }
            if hash_file(managed, &source_root.join(&file.path))?
                .map(|v| v.0)
                .as_deref()
                != Some(&file.source)
                || hash_file(managed, &target_root.join(&file.path))?.map(|v| v.0)
                    != file.destination
            {
                return Err(stale());
            }
            let target = target_root.join(&file.path);
            ensure_directory(
                managed.data_root(),
                target.parent().ok_or_else(unsafe_path)?,
            )
            .map_err(storage)?;
            if file.destination.is_some() {
                let saved = backup.join(&file.path);
                ensure_directory(managed.data_root(), saved.parent().ok_or_else(unsafe_path)?)
                    .map_err(storage)?;
                let mut handle = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&saved)
                    .map_err(storage)?;
                let input = regular_file(managed, &target)?.ok_or_else(stale)?;
                std::io::copy(&mut input.take(MAX_FILE + 1), &mut handle).map_err(storage)?;
                handle.sync_all().map_err(storage)?;
                drop(handle);
                if hash_file(managed, &saved)?.map(|v| v.0) != file.destination {
                    return Err(stale());
                }
                checkpoint("backupSaved")?;
            }
            // Every output is staged privately and rehashed before commit.
            // An absent target uses no-clobber creation, including NBT merges.
            let temp =
                target.with_file_name(format!(".aurora-sync-{}", uuid::Uuid::new_v4().simple()));
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(storage)?;
            let committed = (|| {
                let expected = if let Some(merged) = &file.merged {
                    output.write_all(merged).map_err(storage)?;
                    (
                        file.output_hash.as_deref().ok_or_else(stale)?,
                        merged.len() as u64,
                    )
                } else {
                    let input =
                        regular_file(managed, &source_root.join(&file.path))?.ok_or_else(stale)?;
                    let copied = std::io::copy(&mut input.take(MAX_FILE + 1), &mut output)
                        .map_err(storage)?;
                    if copied != file.size {
                        return Err(stale());
                    }
                    (&*file.source, file.size)
                };
                output.sync_all().map_err(storage)?;
                drop(output);
                if hash_file(managed, &temp)?
                    .as_ref()
                    .map(|(hash, size)| (hash.as_str(), *size))
                    != Some(expected)
                {
                    return Err(stale());
                }
                checkpoint("copyStaged")?;
                if policy(managed)? != active_policy
                    || hash_file(managed, &target)?.map(|v| v.0) != file.destination
                    || hash_file(managed, &source_root.join(&file.path))?
                        .map(|v| v.0)
                        .as_deref()
                        != Some(&file.source)
                {
                    return Err(stale());
                }
                checked_directory(
                    managed.data_root(),
                    target.parent().ok_or_else(unsafe_path)?,
                )
                .map_err(storage)?;
                if file.destination.is_some() {
                    fs::rename(&temp, &target).map_err(storage)
                } else {
                    fs::hard_link(&temp, &target).map_err(storage)
                }
            })();
            // This exact unique staging file is the only cleanup target.
            let _ = fs::remove_file(&temp);
            committed?;
            receipt.copied += 1;
            record_count(|c| {
                c.copied += file.merged.as_ref().map_or(file.size, |b| b.len() as u64)
            });
            checkpoint("fileCommitted")?;
        }
        crate::install::journal::atomic_write(
            &backup.join("completed.json"),
            &serde_json::to_vec(&receipt).map_err(storage)?,
        )
        .map_err(storage)?;
        Ok(receipt)
    })
}

#[cfg(test)]
#[path = "instance_sync_tests.rs"]
mod tests;
