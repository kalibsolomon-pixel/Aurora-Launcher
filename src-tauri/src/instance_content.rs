//! Provider-independent instance content. Filesystem authority stays here: a
//! validated instance id and a closed content type select a direct child of
//! the isolated game directory. Provider records are evidence, never guesses.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use futures_util::{StreamExt as _, stream};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use crate::cache::ArtifactCache;
use crate::downloads::{ArtifactSource, Sha512ArtifactSource};
use crate::instances::InstanceId;
use crate::integrity::{ArtifactDigest, verify_file};
use crate::paths::ManagedPaths;

const SCHEMA_VERSION: u32 = 4;
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4_096;
const MAX_METADATA_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentType {
    Mod,
    ResourcePack,
    ShaderPack,
}

impl ContentType {
    pub fn directory_name(self) -> &'static str {
        match self {
            Self::Mod => "mods",
            Self::ResourcePack => "resourcepacks",
            Self::ShaderPack => "shaderpacks",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentOwnership {
    LauncherManagedRequired,
    ProviderManaged,
    UserManaged,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DependencyKind {
    Required,
    Optional,
    Incompatible,
}

/// How a provider record entered management. Historical schema-1/2 records
/// migrate to the strongest recorded evidence only; provenance is never
/// invented for content Aurora did not observe being installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderOrigin {
    Direct,
    Dependency,
    Recovered,
    Pack,
}

/// Per-record release-channel policy for update candidate selection. This is
/// a policy about which provider version types may be selected, never a pin
/// of one exact version. Schema-3 and older records migrate to Stable: the
/// previous updater derived its channel rule from a live provider lookup of
/// the installed version, and a file migration must not perform network I/O
/// or fabricate the historical policy it would have produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
    Alpha,
}

impl UpdateChannel {
    /// Whether a Modrinth `version_type` may be selected under this policy.
    /// Unknown provider version types are never selectable.
    pub fn allows(self, version_type: &str) -> bool {
        match self {
            Self::Stable => version_type == "release",
            Self::Beta => matches!(version_type, "release" | "beta"),
            Self::Alpha => matches!(version_type, "release" | "beta" | "alpha"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderDependency {
    pub kind: DependencyKind,
    pub provider: String,
    pub project_id: String,
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentCompatibility {
    pub minecraft_versions: Vec<String>,
    pub loader: Option<String>,
    pub environment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderRecord {
    pub content_type: ContentType,
    pub provider: String,
    pub project_id: String,
    pub version_id: String,
    pub file_id: String,
    pub file_name: String,
    pub sha256: String,
    pub display_version: Option<String>,
    pub compatibility: ContentCompatibility,
    pub dependencies: Vec<ProviderDependency>,
    /// A direct user choice survives even while another installed item needs
    /// this artifact. Legacy records are migrated conservatively to true.
    pub explicitly_retained: bool,
    /// Required edges that were actually satisfied at installation time.
    /// Remote dependency metadata above is descriptive, not ownership.
    pub requires: Vec<ProviderIdentity>,
    /// How this record entered provider management. Schema-2 migration maps
    /// `explicitlyRetained` to direct/dependency; schema-1 records are direct.
    pub origin: ProviderOrigin,
    /// Receipt timestamp recorded when Aurora registered the record. Absent
    /// for migrated historical records; never fabricated.
    pub installed_at_unix_seconds: Option<u64>,
    /// A user pin against provider-version advancement. Pinned roots may
    /// still report an available candidate, but normal update actions
    /// (single, selected, all) never advance them. Not a version pin: the
    /// record keeps identifying the installed version.
    pub pinned: bool,
    /// Release-channel policy for candidate selection. Independent of
    /// pinning; migrating records default to Stable.
    pub update_channel: UpdateChannel,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderIdentity {
    pub content_type: ContentType,
    pub provider: String,
    pub project_id: String,
}

impl ProviderRecord {
    pub fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            content_type: self.content_type,
            provider: self.provider.clone(),
            project_id: self.project_id.clone(),
        }
    }
}

impl ProviderRecord {
    pub(crate) fn validate(&self) -> Result<(), ContentError> {
        validate_file_name(&self.file_name)?;
        if !self
            .file_name
            .to_ascii_lowercase()
            .ends_with(match self.content_type {
                ContentType::Mod => ".jar",
                _ => ".zip",
            })
        {
            return Err(ContentError::StateMalformed(
                "provider filename has the wrong extension".into(),
            ));
        }
        for value in [
            &self.provider,
            &self.project_id,
            &self.version_id,
            &self.file_id,
        ] {
            if value.trim().is_empty() || value.len() > 256 {
                return Err(ContentError::StateMalformed(
                    "provider identity is empty or oversized".into(),
                ));
            }
        }
        ArtifactDigest::parse(&self.sha256)
            .map_err(|_| ContentError::StateMalformed("provider digest is invalid".into()))?;
        if self.origin == ProviderOrigin::Pack
            && (self.file_id.len() != 128
                || !self.file_id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(ContentError::StateMalformed(
                "pack provider file identity is not SHA-512".into(),
            ));
        }
        if self
            .compatibility
            .minecraft_versions
            .iter()
            .any(|value| value.trim().is_empty())
            || self.dependencies.iter().any(|dependency| {
                dependency.provider.trim().is_empty() || dependency.project_id.trim().is_empty()
            })
        {
            return Err(ContentError::StateMalformed(
                "provider compatibility or dependency is invalid".into(),
            ));
        }
        if self.requires.iter().any(|edge| {
            edge.provider.trim().is_empty()
                || edge.provider.len() > 256
                || edge.project_id.trim().is_empty()
                || edge.project_id.len() > 256
        }) {
            return Err(ContentError::StateMalformed(
                "provider edge is invalid".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentState {
    schema_version: u32,
    pub entries: Vec<ProviderRecord>,
}

impl ContentState {
    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }

    pub fn from_json(text: &str) -> Result<Self, ContentError> {
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
        let version = value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ContentError::StateMalformed("schemaVersion is required".into()))?;
        let mut value = value;
        if version == 1 || version == 2 || version == 3 {
            let entries = value
                .get_mut("entries")
                .and_then(serde_json::Value::as_array_mut)
                .ok_or_else(|| ContentError::StateMalformed("entries are required".into()))?;
            for entry in entries {
                let fields = entry.as_object_mut().ok_or_else(|| {
                    ContentError::StateMalformed("provider record is invalid".into())
                })?;
                if version == 1
                    && (fields.contains_key("explicitlyRetained")
                        || fields.contains_key("requires"))
                {
                    return Err(ContentError::StateMalformed(
                        "v1 record contains v2 fields".into(),
                    ));
                }
                if version <= 2
                    && (fields.contains_key("origin")
                        || fields.contains_key("installedAtUnixSeconds"))
                {
                    return Err(ContentError::StateMalformed(
                        "legacy record contains v3 fields".into(),
                    ));
                }
                if version <= 3
                    && (fields.contains_key("pinned") || fields.contains_key("updateChannel"))
                {
                    return Err(ContentError::StateMalformed(
                        "legacy record contains v4 fields".into(),
                    ));
                }
                if version == 1 {
                    fields.insert("explicitlyRetained".into(), serde_json::Value::Bool(true));
                    fields.insert("requires".into(), serde_json::json!([]));
                }
                if version <= 2 {
                    // Schema 2 recorded only retention, so migration derives
                    // origin from the strongest evidence Aurora actually
                    // observed.
                    let retained = fields
                        .get("explicitlyRetained")
                        .and_then(serde_json::Value::as_bool)
                        .ok_or_else(|| {
                            ContentError::StateMalformed("retention is required".into())
                        })?;
                    fields.insert(
                        "origin".into(),
                        serde_json::json!(match retained {
                            true => ProviderOrigin::Direct,
                            false => ProviderOrigin::Dependency,
                        }),
                    );
                    // Historical installation times were never recorded; the
                    // timestamp is a receipt, not inferred history.
                    fields.insert("installedAtUnixSeconds".into(), serde_json::Value::Null);
                }
                // Schema 3 predates update policy: unpinned, stable. The old
                // updater's channel rule came from a live provider lookup, so
                // no historical policy is fabricated here.
                fields.insert("pinned".into(), serde_json::Value::Bool(false));
                fields.insert(
                    "updateChannel".into(),
                    serde_json::json!(UpdateChannel::Stable),
                );
            }
            value["schemaVersion"] = serde_json::json!(SCHEMA_VERSION);
        } else if version != u64::from(SCHEMA_VERSION) {
            return Err(ContentError::StateVersion(version));
        }
        let state: Self = serde_json::from_value(value)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
        state.validate()?;
        Ok(state)
    }

    pub(crate) fn validate(&self) -> Result<(), ContentError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ContentError::StateVersion(u64::from(self.schema_version)));
        }
        let mut names = HashSet::new();
        let mut identities = HashSet::new();
        let mut project_records: HashMap<ProviderIdentity, &ProviderRecord> = HashMap::new();
        let mut pack_files = HashMap::new();
        for entry in &self.entries {
            entry.validate()?;
            if !names.insert((entry.content_type, entry.file_name.to_lowercase())) {
                return Err(ContentError::StateMalformed(
                    "duplicate provider filename".into(),
                ));
            }
            let identity = entry.identity();
            if let Some(previous) = project_records.insert(identity.clone(), entry) {
                if previous.origin != ProviderOrigin::Pack || entry.origin != ProviderOrigin::Pack {
                    return Err(ContentError::StateMalformed(
                        "duplicate provider project outside a pack".into(),
                    ));
                }
            }
            if entry.origin == ProviderOrigin::Pack {
                let key = (
                    identity.clone(),
                    entry.version_id.clone(),
                    entry.file_id.to_lowercase(),
                );
                if pack_files
                    .insert(key, &entry.sha256)
                    .is_some_and(|digest| digest != &entry.sha256)
                {
                    return Err(ContentError::StateMalformed(
                        "one provider file identity has conflicting bytes".into(),
                    ));
                }
            }
            identities.insert(identity);
        }
        for entry in &self.entries {
            let mut edges = HashSet::new();
            for edge in &entry.requires {
                if !edges.insert(edge) || edge == &entry.identity() || !identities.contains(edge) {
                    return Err(ContentError::StateMalformed(
                        "invalid installed dependency edge".into(),
                    ));
                }
            }
        }
        let mut graph: HashMap<ProviderIdentity, Vec<ProviderIdentity>> = HashMap::new();
        for entry in &self.entries {
            let edges = graph.entry(entry.identity()).or_default();
            for edge in &entry.requires {
                if !edges.contains(edge) {
                    edges.push(edge.clone());
                }
            }
        }
        fn cycle(
            node: &ProviderIdentity,
            graph: &HashMap<ProviderIdentity, Vec<ProviderIdentity>>,
            active: &mut HashSet<ProviderIdentity>,
            done: &mut HashSet<ProviderIdentity>,
        ) -> bool {
            if done.contains(node) {
                return false;
            }
            if !active.insert(node.clone()) {
                return true;
            }
            if graph
                .get(node)
                .is_some_and(|edges| edges.iter().any(|edge| cycle(edge, graph, active, done)))
            {
                return true;
            }
            active.remove(node);
            done.insert(node.clone());
            false
        }
        let mut done = HashSet::new();
        for identity in &identities {
            if cycle(identity, &graph, &mut HashSet::new(), &mut done) {
                return Err(ContentError::StateMalformed(
                    "installed dependency graph contains a cycle".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn load(managed: &ManagedPaths, instance: &InstanceId) -> Result<Self, ContentError> {
        let path = state_path(managed, instance)?;
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_json(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::empty()),
            Err(error) => Err(ContentError::Io(error)),
        }
    }

    pub fn load_and_migrate(
        managed: &ManagedPaths,
        instance: &InstanceId,
    ) -> Result<Self, ContentError> {
        with_instance_lock(instance, || {
            Self::load_and_migrate_locked(managed, instance)
        })
    }

    fn load_and_migrate_locked(
        managed: &ManagedPaths,
        instance: &InstanceId,
    ) -> Result<Self, ContentError> {
        let path = state_path(managed, instance)?;
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::empty()),
            Err(error) => return Err(ContentError::Io(error)),
        };
        let legacy = serde_json::from_str::<serde_json::Value>(&text)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|version| version == 1 || version == 2 || version == 3);
        let state = Self::from_json(&text)?;
        if legacy {
            state.save(managed, instance)?;
        }
        Ok(state)
    }

    pub fn required_by(&self, identity: &ProviderIdentity) -> Vec<ProviderIdentity> {
        self.entries
            .iter()
            .filter(|record| record.requires.contains(identity))
            .map(ProviderRecord::identity)
            .collect()
    }

    pub fn find(&self, identity: &ProviderIdentity) -> Option<&ProviderRecord> {
        self.entries
            .iter()
            .find(|record| record.identity() == *identity)
    }

    pub fn save(&self, managed: &ManagedPaths, instance: &InstanceId) -> Result<(), ContentError> {
        self.validate()?;
        // Refuse to repair or overwrite a damaged or future-version document.
        let _ = Self::load(managed, instance)?;
        let path = state_path(managed, instance)?;
        let temporary = path.with_extension(format!("json.{}.tmp", uuid::Uuid::new_v4()));
        let mut sorted = self.clone();
        sorted.entries.sort_by(|a, b| {
            a.content_type
                .directory_name()
                .cmp(b.content_type.directory_name())
                .then_with(|| a.file_name.cmp(&b.file_name))
        });
        let mut json = serde_json::to_string_pretty(&sorted)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
        json.push('\n');
        std::fs::write(&temporary, json).map_err(ContentError::Io)?;
        if let Err(error) = std::fs::rename(&temporary, &path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(ContentError::Io(error));
        }
        Ok(())
    }
}

pub fn validate_provider_file(
    managed: &ManagedPaths,
    instance: &InstanceId,
    record: &ProviderRecord,
) -> Result<PathBuf, ContentError> {
    let directory = validate_directory(managed, instance, record.content_type)?;
    let active = directory.join(&record.file_name);
    let disabled = directory.join(format!("{}.disabled", record.file_name));
    if record.content_type == ContentType::Mod
        && std::fs::symlink_metadata(&active).is_ok()
        && std::fs::symlink_metadata(&disabled).is_ok()
    {
        return Err(ContentError::Collision);
    }
    let path = if record.content_type == ContentType::Mod
        && std::fs::symlink_metadata(&active)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        disabled
    } else {
        active
    };
    let metadata = std::fs::symlink_metadata(&path).map_err(ContentError::Io)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(ContentError::UnsafePath);
    }
    let digest = ArtifactDigest::parse(&record.sha256).map_err(|_| ContentError::HashMismatch)?;
    verify_file(&path, &digest, None).map_err(|_| ContentError::HashMismatch)?;
    Ok(path)
}

/// Promote an already installed dependency when the user chooses it directly.
/// The exact bytes are revalidated and no network acquisition occurs.
pub fn retain_provider(
    managed: &ManagedPaths,
    instance: &InstanceId,
    identity: &ProviderIdentity,
) -> Result<ProviderRecord, ContentError> {
    with_instance_lock(instance, || {
        let mut state = ContentState::load(managed, instance)?;
        let record = state
            .entries
            .iter_mut()
            .find(|record| record.identity() == *identity)
            .ok_or(ContentError::ChangedSinceScan)?;
        validate_provider_file(managed, instance, record)?;
        if !record.explicitly_retained {
            record.explicitly_retained = true;
            // The user now chooses this artifact directly; the original
            // registration receipt stays untouched.
            record.origin = ProviderOrigin::Direct;
            let updated = record.clone();
            state.save(managed, instance)?;
            Ok(updated)
        } else {
            Ok(record.clone())
        }
    })
}

fn required_edges(
    record: &ProviderRecord,
    identities: &[ProviderIdentity],
) -> Vec<ProviderIdentity> {
    required_edges_for(&record.dependencies, identities)
}

/// Required dependency declarations that are actually satisfied by installed
/// provider identities. Descriptive only; never an ownership statement.
pub(crate) fn required_edges_for(
    dependencies: &[ProviderDependency],
    identities: &[ProviderIdentity],
) -> Vec<ProviderIdentity> {
    dependencies
        .iter()
        .filter(|dependency| dependency.kind == DependencyKind::Required)
        .filter_map(|dependency| {
            identities
                .iter()
                .find(|identity| {
                    identity.provider == dependency.provider
                        && identity.project_id == dependency.project_id
                })
                .cloned()
        })
        .collect()
}

fn prune_orphans(state: &mut ContentState) {
    loop {
        let removable: HashSet<_> = state
            .entries
            .iter()
            .filter(|record| {
                !record.explicitly_retained && state.required_by(&record.identity()).is_empty()
            })
            .map(ProviderRecord::identity)
            .collect();
        if removable.is_empty() {
            break;
        }
        state
            .entries
            .retain(|record| !removable.contains(&record.identity()));
    }
}

pub fn removal_state(
    current: &ContentState,
    identity: &ProviderIdentity,
) -> Result<ContentState, ContentError> {
    let mut next = current.clone();
    let record = next
        .entries
        .iter_mut()
        .find(|record| record.identity() == *identity)
        .ok_or(ContentError::ChangedSinceScan)?;
    if !record.explicitly_retained && !current.required_by(identity).is_empty() {
        return Err(ContentError::RequiredByInstalledContent);
    }
    record.explicitly_retained = false;
    prune_orphans(&mut next);
    next.validate()?;
    Ok(next)
}

/// Plan one coherent multi-root update transaction. Roots must be explicitly
/// retained Modrinth-style records; each root's replacement keeps its
/// installation origin (updating recovered content does not rewrite how
/// management began). A replacement may also supersede an installed
/// dependency of the updating graph, but only when that dependency is not
/// itself explicitly retained and every installed record requiring it is
/// inside the same transaction — an outside dependent never has its
/// dependency swapped underneath a plan it did not join.
pub fn updated_state_multi(
    current: &ContentState,
    roots: &[ProviderIdentity],
    mut replacements: Vec<ProviderRecord>,
) -> Result<ContentState, ContentError> {
    if roots.is_empty() || roots.len() > 64 {
        return Err(ContentError::StateMalformed(
            "update graph has no roots".into(),
        ));
    }
    let mut root_set = HashSet::new();
    for root in roots {
        if !root_set.insert(root.clone()) {
            return Err(ContentError::StateMalformed(
                "update graph repeats a root".into(),
            ));
        }
        let old = current.find(root).ok_or(ContentError::ChangedSinceScan)?;
        if !old.explicitly_retained {
            return Err(ContentError::RequiredByInstalledContent);
        }
    }
    for root in &root_set {
        // Existing lifecycle rule: an installed dependent outside this
        // transaction blocks advancing its requirement. Dependents updating
        // in the same graph do not.
        if current
            .required_by(root)
            .iter()
            .any(|dependent| !root_set.contains(dependent))
        {
            return Err(ContentError::RequiredByInstalledContent);
        }
    }
    let replacement_identities: HashSet<_> =
        replacements.iter().map(ProviderRecord::identity).collect();
    if !roots
        .iter()
        .all(|root| replacement_identities.contains(root))
    {
        return Err(ContentError::StateMalformed(
            "update graph is missing a root replacement".into(),
        ));
    }
    // A replaced dependency keeps the same identity as the record it
    // supersedes; the root identities themselves are removed as roots.
    let superseded: HashSet<_> = replacement_identities
        .iter()
        .filter(|identity| current.find(identity).is_some())
        .cloned()
        .collect();
    for identity in &superseded {
        if root_set.contains(identity) {
            continue;
        }
        let old = current
            .find(identity)
            .expect("superseded identities exist in current");
        // A pin follows the record even if a later removal leaves it as a
        // non-retained dependency. Advancing a parent must not supersede that
        // exact pinned dependency as an incidental graph replacement.
        if old.explicitly_retained || old.pinned {
            return Err(ContentError::RequiredByInstalledContent);
        }
        let outsiders: Vec<_> = current
            .required_by(identity)
            .into_iter()
            .filter(|dependent| !superseded.contains(dependent))
            .collect();
        if !outsiders.is_empty() {
            return Err(ContentError::DependencyBlocked(format!(
                "Updating would replace a dependency still required outside this update: {}. Update its parent in the same operation or review it separately.",
                outsiders
                    .iter()
                    .filter_map(|identity| current
                        .find(identity)
                        .map(|record| record.file_name.clone()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    let mut next = current.clone();
    next.entries
        .retain(|record| !superseded.contains(&record.identity()));
    let mut identities: Vec<_> = next.entries.iter().map(ProviderRecord::identity).collect();
    for replacement in &replacements {
        let identity = replacement.identity();
        if identities.contains(&identity) {
            return Err(ContentError::Collision);
        }
        identities.push(identity);
    }
    for replacement in &mut replacements {
        let identity = replacement.identity();
        let is_root = root_set.contains(&identity);
        replacement.explicitly_retained = is_root;
        replacement.requires = required_edges(replacement, &identities);
        if is_root {
            // Origin records how management began; an update does not change
            // that story, so the root keeps the origin of the record it
            // replaces. The receipt is refreshed: it documents when these
            // exact bytes were registered.
            let old = current.find(&identity).expect("root exists");
            replacement.origin = old.origin;
            replacement.pinned = old.pinned;
            replacement.update_channel = old.update_channel;
        } else {
            replacement.origin = ProviderOrigin::Dependency;
        }
        replacement.installed_at_unix_seconds = Some(now_unix_seconds());
    }
    let new_identities: HashSet<_> = replacements.iter().map(ProviderRecord::identity).collect();
    next.entries.extend(replacements);
    prune_orphans(&mut next);
    if new_identities
        .iter()
        .any(|identity| next.find(identity).is_none())
    {
        return Err(ContentError::StateMalformed(
            "update graph contains an unowned artifact".into(),
        ));
    }
    next.validate()?;
    Ok(next)
}

pub fn updated_state(
    current: &ContentState,
    root: &ProviderIdentity,
    replacements: Vec<ProviderRecord>,
) -> Result<ContentState, ContentError> {
    updated_state_multi(current, std::slice::from_ref(root), replacements)
}

pub fn update_preview_state(
    current: &ContentState,
    root: &ProviderIdentity,
    plans: &[ProviderInstallPlan],
) -> Result<ContentState, ContentError> {
    updated_state(
        current,
        root,
        plans
            .iter()
            .map(|plan| plan.record("0".repeat(64), ProviderOrigin::Direct))
            .collect(),
    )
}

/// Preview-time state planning for a multi-root update. Placeholder digests
/// stand in for artifacts that are only acquired at execution time.
pub fn update_preview_state_multi(
    current: &ContentState,
    roots: &[ProviderIdentity],
    plans: &[ProviderInstallPlan],
) -> Result<ContentState, ContentError> {
    updated_state_multi(
        current,
        roots,
        plans
            .iter()
            .map(|plan| plan.record("0".repeat(64), ProviderOrigin::Direct))
            .collect(),
    )
}

fn state_path(managed: &ManagedPaths, instance: &InstanceId) -> Result<PathBuf, ContentError> {
    let root = validated_instance_root(managed, instance)?;
    let path = root.join("content-managed.json");
    if let Ok(meta) = std::fs::symlink_metadata(&path) {
        if meta.file_type().is_symlink() || is_reparse_point(&meta) || !meta.is_file() {
            return Err(ContentError::UnsafePath);
        }
    }
    Ok(path)
}

pub(crate) fn validated_instance_root(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<PathBuf, ContentError> {
    let root = std::fs::canonicalize(managed.data_root()).map_err(ContentError::Io)?;
    let instances = std::fs::canonicalize(managed.instances_dir()).map_err(ContentError::Io)?;
    let target = managed.instance_paths(instance).root().to_path_buf();
    let canonical = std::fs::canonicalize(&target).map_err(ContentError::Io)?;
    if !instances.starts_with(&root) || canonical != instances.join(instance.as_str()) {
        return Err(ContentError::UnsafePath);
    }
    Ok(target)
}

pub fn validate_directory(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
) -> Result<PathBuf, ContentError> {
    let root = validated_instance_root(managed, instance)?;
    let directory = root.join(kind.directory_name());
    match std::fs::symlink_metadata(&directory) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() && !is_reparse_point(&meta) => {
            let canonical_root = std::fs::canonicalize(&root).map_err(ContentError::Io)?;
            let canonical_directory =
                std::fs::canonicalize(&directory).map_err(ContentError::Io)?;
            if canonical_directory != canonical_root.join(kind.directory_name()) {
                return Err(ContentError::UnsafePath);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Ok(_) => return Err(ContentError::UnsafePath),
        Err(error) => return Err(ContentError::Io(error)),
    }
    Ok(directory)
}

pub fn ensure_directory(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
) -> Result<PathBuf, ContentError> {
    let path = validate_directory(managed, instance, kind)?;
    if !path.exists() {
        std::fs::create_dir(&path).map_err(ContentError::Io)?;
    }
    validate_directory(managed, instance, kind)
}

pub fn validate_file_name(name: &str) -> Result<(), ContentError> {
    let path = Path::new(name);
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let reserved = ["con", "prn", "aux", "nul"].contains(&stem.as_str())
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with([' ', '.'])
        || name.contains(['/', '\\', ':', '<', '>', '"', '|', '?', '*'])
        || name.chars().any(char::is_control)
        || reserved
        || path.is_absolute()
        || path.components().count() != 1
    {
        return Err(ContentError::UnsafePath);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentWarning {
    pub code: String,
    pub message: String,
}

fn warning(code: &str, message: impl Into<String>) -> ContentWarning {
    ContentWarning {
        code: code.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemovalPath {
    /// The entry-level file removal owns this artifact (unmanaged local ZIP).
    LocalFile,
    /// The provider graph lifecycle owns this artifact (verified managed ZIP);
    /// dependency enforcement happens in its removal preview/transaction.
    ProviderGraph,
    /// No removal path exists for this entry; the reason is carried alongside.
    Blocked,
}

/// The backend-owned statement of which management operations Aurora safely
/// supports for one installed pack entry. Svelte renders this object; it never
/// infers legality from content types, ownership strings or provider names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentManagement {
    pub can_remove: bool,
    pub removal_path: RemovalPath,
    pub removal_blocked_reason: Option<String>,
    pub can_toggle: bool,
    pub active: Option<bool>,
    /// Truthful representation for content whose activation Aurora does not
    /// own (shader packs: loader-specific, in-game configuration).
    pub activation_managed_in_game: bool,
    pub toggle_blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentEntry {
    pub entry_id: String,
    pub content_type: ContentType,
    pub file_name: String,
    pub display_name: String,
    pub file_type: String,
    pub size_bytes: Option<u64>,
    pub modified_unix_millis: Option<u64>,
    pub ownership: ContentOwnership,
    pub sha256: Option<String>,
    pub provenance: Option<ProviderRecord>,
    pub description: Option<String>,
    pub pack_format: Option<u64>,
    pub warnings: Vec<ContentWarning>,
    pub management: ContentManagement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentInventory {
    pub instance_id: String,
    pub content_type: ContentType,
    pub entries: Vec<ContentEntry>,
    pub missing_managed: Vec<ProviderRecord>,
}

/// One on-demand read of the instance's activation knowledge for a scan.
/// Nothing watches the underlying file; every scan re-reads it.
struct ActivationKnowledge {
    /// Enabled references from options.txt. `None` when the document could
    /// not be read safely (malformed); the read error explains why.
    enabled: Option<Vec<String>>,
    read_error: Option<&'static str>,
}

fn activation_knowledge(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
) -> ActivationKnowledge {
    if kind != ContentType::ResourcePack {
        return ActivationKnowledge {
            enabled: None,
            read_error: None,
        };
    }
    let paths = managed.instance_paths(instance);
    let root = paths.root().to_path_buf();
    match crate::pack_activation::enabled_references(&root) {
        Ok(enabled) => ActivationKnowledge {
            enabled,
            read_error: None,
        },
        Err(_) => ActivationKnowledge {
            enabled: None,
            read_error: Some(
                "Minecraft's options.txt could not be read safely; pack activation is unavailable.",
            ),
        },
    }
}

/// The backend-owned capability statement for one inspected entry.
fn management_capabilities(
    kind: ContentType,
    file_type: &str,
    ownership: ContentOwnership,
    file_name: &str,
    activation: &ActivationKnowledge,
) -> ContentManagement {
    let (can_remove, removal_path, removal_blocked_reason) = match (file_type, ownership) {
        ("zip", ContentOwnership::UserManaged) => (true, RemovalPath::LocalFile, None),
        ("zip", ContentOwnership::ProviderManaged) => (true, RemovalPath::ProviderGraph, None),
        ("zip", ContentOwnership::Unknown) => (
            false,
            RemovalPath::Blocked,
            Some(
                "This file no longer matches its provider record; removal stays blocked until it is inspected."
                    .into(),
            ),
        ),
        ("directory", _) => (
            false,
            RemovalPath::Blocked,
            Some("Folder packs are left untouched; remove them from the content folder directly.".into()),
        ),
        ("link", _) => (
            false,
            RemovalPath::Blocked,
            Some("Links are shown but never followed or removed.".into()),
        ),
        ("unreadable", _) => (
            false,
            RemovalPath::Blocked,
            Some("This entry could not be inspected.".into()),
        ),
        _ => (
            false,
            RemovalPath::Blocked,
            Some("This entry is not a removable pack file.".into()),
        ),
    };
    let (can_toggle, active, activation_managed_in_game, toggle_blocked_reason) =
        if kind == ContentType::ShaderPack {
            // Shader loaders (Iris, OptiFine-like) each own activation state in
            // loader-specific configuration Aurora does not own. Installation is
            // reported truthfully; activation is never guessed or faked.
            (false, None, true, None)
        } else if let Some(reason) = activation.read_error {
            (false, None, false, Some(reason.to_owned()))
        } else if !crate::pack_activation::options_representable(file_name) {
            (
            false,
            None,
            false,
            Some(
                "This pack's file name cannot be represented safely in Minecraft's options.txt."
                    .into(),
            ),
        )
        } else {
            let reference = format!("file/{file_name}");
            let on = activation
                .enabled
                .as_ref()
                .is_some_and(|names| names.iter().any(|entry| entry == &reference));
            (true, Some(on), false, None)
        };
    ContentManagement {
        can_remove,
        removal_path,
        removal_blocked_reason,
        can_toggle,
        active,
        activation_managed_in_game,
        toggle_blocked_reason,
    }
}

pub fn scan(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
) -> Result<ContentInventory, ContentError> {
    let directory = validate_directory(managed, instance, kind)?;
    let state = ContentState::load(managed, instance)?;
    let pack = crate::pack_state::InstalledPack::load(managed, instance)
        .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
    let activation = activation_knowledge(managed, instance, kind);
    let mut entries = Vec::new();
    if directory.exists() {
        for item in std::fs::read_dir(directory).map_err(ContentError::Io)? {
            let item = item.map_err(ContentError::Io)?;
            let name = item.file_name().to_string_lossy().into_owned();
            let record = state.entries.iter().find(|record| {
                record.content_type == kind && record.file_name.eq_ignore_ascii_case(&name)
            });
            let mut entry = inspect(&item.path(), kind, record, &activation);
            if let Some(pack) = pack
                .as_ref()
                .filter(|pack| pack.owns_path(&format!("{}/{}", kind.directory_name(), name)))
            {
                entry.management.can_remove = false;
                entry.management.removal_path = RemovalPath::Blocked;
                entry.management.removal_blocked_reason = Some(format!(
                    "Required by {} {}. Pack component removal is deferred until pack reconciliation is available.",
                    pack.identity.name, pack.identity.pack_version
                ));
            }
            entries.push(entry);
        }
    }
    let present: HashSet<_> = entries
        .iter()
        .map(|entry| entry.file_name.to_lowercase())
        .collect();
    let missing_managed = state
        .entries
        .iter()
        .filter(|record| {
            record.content_type == kind && !present.contains(&record.file_name.to_lowercase())
        })
        .cloned()
        .collect();
    entries.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    Ok(ContentInventory {
        instance_id: instance.to_string(),
        content_type: kind,
        entries,
        missing_managed,
    })
}

fn inspect(
    path: &Path,
    kind: ContentType,
    record: Option<&ProviderRecord>,
    activation: &ActivationKnowledge,
) -> ContentEntry {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let meta = std::fs::symlink_metadata(path);
    let (file_type, size, modified) = match &meta {
        Ok(meta) => {
            let file_type = if meta.file_type().is_symlink() || is_reparse_point(meta) {
                "link"
            } else if meta.is_dir() {
                "directory"
            } else if meta.is_file() && name.to_ascii_lowercase().ends_with(".zip") {
                "zip"
            } else {
                "unexpectedFile"
            };
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| u64::try_from(duration.as_millis()).ok());
            (file_type, meta.is_file().then_some(meta.len()), modified)
        }
        Err(_) => ("unreadable", None, None),
    };
    let mut warnings = Vec::new();
    if file_type == "link" {
        warnings.push(warning(
            "link_not_managed",
            "Links and reparse points are shown but never followed.",
        ));
    }
    if file_type == "unexpectedFile" {
        warnings.push(warning(
            "unexpected_file",
            "This is not a ZIP pack and is left untouched.",
        ));
    }
    if file_type == "unreadable" {
        warnings.push(warning(
            "entry_unreadable",
            "This entry could not be inspected.",
        ));
    }
    let mut description = None;
    let mut pack_format = None;
    if file_type == "zip" {
        let (found_description, found_format, issue) =
            inspect_archive(path, kind, size.unwrap_or_default());
        description = found_description;
        pack_format = found_format;
        if let Some(issue) = issue {
            warnings.push(issue);
        }
    } else if file_type == "directory" && kind == ContentType::ResourcePack {
        let metadata = path.join("pack.mcmeta");
        if let Ok(file_meta) = std::fs::symlink_metadata(&metadata) {
            if file_meta.is_file()
                && !file_meta.file_type().is_symlink()
                && !is_reparse_point(&file_meta)
                && file_meta.len() <= MAX_METADATA_BYTES
            {
                if let Ok(file) = std::fs::File::open(metadata) {
                    let mut bytes = Vec::new();
                    if file
                        .take(MAX_METADATA_BYTES + 1)
                        .read_to_end(&mut bytes)
                        .is_ok()
                        && bytes.len() as u64 <= MAX_METADATA_BYTES
                    {
                        (description, pack_format) = parse_pack_metadata(&bytes);
                    } else {
                        warnings.push(warning(
                            "pack_metadata_unreadable",
                            "Pack metadata exceeded the safe read limit.",
                        ));
                    }
                }
            } else {
                warnings.push(warning(
                    "pack_metadata_unsafe",
                    "Pack metadata could not be read safely.",
                ));
            }
        } else {
            warnings.push(warning(
                "pack_metadata_missing",
                "No root-level pack.mcmeta was found.",
            ));
        }
    } else if file_type == "directory" && kind == ContentType::ShaderPack {
        let shaders = path.join("shaders");
        if !std::fs::symlink_metadata(shaders).is_ok_and(|meta| {
            meta.is_dir() && !meta.file_type().is_symlink() && !is_reparse_point(&meta)
        }) {
            warnings.push(warning(
                "shader_structure_unknown",
                "No safe root shaders/ directory was found; compatibility is unknown.",
            ));
        }
    }
    let mut ownership = if file_type == "zip" || file_type == "directory" {
        ContentOwnership::UserManaged
    } else {
        ContentOwnership::Unknown
    };
    let mut sha256 = None;
    let mut provenance = None;
    if let Some(record) = record {
        if file_type == "zip" {
            match ArtifactDigest::parse(&record.sha256)
                .ok()
                .and_then(|digest| verify_file(path, &digest, None).ok())
            {
                Some(_) => {
                    ownership = ContentOwnership::ProviderManaged;
                    sha256 = Some(record.sha256.clone());
                    provenance = Some(record.clone());
                }
                None => {
                    ownership = ContentOwnership::Unknown;
                    warnings.push(warning("content_hash_mismatch", "This file no longer matches its provider-managed record. Actions are blocked."));
                }
            }
        } else {
            ownership = ContentOwnership::Unknown;
            warnings.push(warning(
                "content_record_mismatch",
                "The provider record no longer describes a regular ZIP file.",
            ));
        }
    }
    let material = format!("{kind:?}|{name}|{file_type}|{size:?}|{modified:?}");
    let entry_id: String = sha2::Sha256::digest(material.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let display_name = name.strip_suffix(".zip").unwrap_or(&name).to_owned();
    let management = management_capabilities(kind, file_type, ownership, &name, activation);
    ContentEntry {
        entry_id,
        content_type: kind,
        file_name: name,
        display_name,
        file_type: file_type.into(),
        size_bytes: size,
        modified_unix_millis: modified,
        ownership,
        sha256,
        provenance,
        description,
        pack_format,
        warnings,
        management,
    }
}

fn inspect_archive(
    path: &Path,
    kind: ContentType,
    size: u64,
) -> (Option<String>, Option<u64>, Option<ContentWarning>) {
    if size > MAX_ARCHIVE_BYTES {
        return (
            None,
            None,
            Some(warning(
                "archive_too_large",
                "Archive inspection exceeds the 512 MiB safety bound.",
            )),
        );
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => {
            return (
                None,
                None,
                Some(warning(
                    "archive_unreadable",
                    "Archive could not be opened.",
                )),
            );
        }
    };
    let mut zip = match zip::ZipArchive::new(file) {
        Ok(zip) => zip,
        Err(_) => {
            return (
                None,
                None,
                Some(warning(
                    "archive_malformed",
                    "This is not a readable ZIP archive.",
                )),
            );
        }
    };
    if zip.len() > MAX_ARCHIVE_ENTRIES {
        return (
            None,
            None,
            Some(warning(
                "archive_entry_limit",
                "Archive has too many entries for safe inspection.",
            )),
        );
    }
    if kind == ContentType::ShaderPack {
        let has_shaders = (0..zip.len()).any(|i| {
            zip.by_index(i)
                .is_ok_and(|entry| entry.name().starts_with("shaders/"))
        });
        return (
            None,
            None,
            (!has_shaders).then(|| {
                warning(
                    "shader_structure_unknown",
                    "No root shaders/ directory was found; compatibility is unknown.",
                )
            }),
        );
    }
    let matches: Vec<_> = (0..zip.len())
        .filter(|index| {
            zip.by_index(*index)
                .is_ok_and(|entry| entry.name() == "pack.mcmeta")
        })
        .collect();
    if matches.len() != 1 {
        return (
            None,
            None,
            Some(warning(
                "pack_metadata_missing",
                "Expected one root-level pack.mcmeta.",
            )),
        );
    }
    let mut entry = match zip.by_index(matches[0]) {
        Ok(entry) => entry,
        Err(_) => {
            return (
                None,
                None,
                Some(warning(
                    "pack_metadata_unreadable",
                    "Pack metadata could not be opened.",
                )),
            );
        }
    };
    if entry.size() > MAX_METADATA_BYTES {
        return (
            None,
            None,
            Some(warning(
                "pack_metadata_too_large",
                "Pack metadata exceeds the 256 KiB safety bound.",
            )),
        );
    }
    let mut bytes = Vec::new();
    if entry
        .by_ref()
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_METADATA_BYTES
    {
        return (
            None,
            None,
            Some(warning(
                "pack_metadata_unreadable",
                "Pack metadata could not be read within the safety bound.",
            )),
        );
    }
    let (description, pack_format) = parse_pack_metadata(&bytes);
    let issue = (description.is_none() && pack_format.is_none()).then(|| {
        warning(
            "pack_metadata_malformed",
            "Pack metadata is malformed or has no usable pack fields.",
        )
    });
    (description, pack_format, issue)
}

fn parse_pack_metadata(bytes: &[u8]) -> (Option<String>, Option<u64>) {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return (None, None);
    };
    let Some(pack) = value.get("pack") else {
        return (None, None);
    };
    let description = pack
        .get("description")
        .and_then(serde_json::Value::as_str)
        .map(|text| text.chars().take(500).collect());
    let format = pack.get("pack_format").and_then(serde_json::Value::as_u64);
    (description, format)
}

fn locks() -> &'static Mutex<std::collections::HashMap<String, Arc<Mutex<()>>>> {
    static LOCKS: OnceLock<Mutex<std::collections::HashMap<String, Arc<Mutex<()>>>>> =
        OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

pub fn with_instance_lock<T>(
    instance: &InstanceId,
    operation: impl FnOnce() -> Result<T, ContentError>,
) -> Result<T, ContentError> {
    let lock = locks()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(instance.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();
    let _guard = lock
        .try_lock()
        .map_err(|_| ContentError::OperationInProgress)?;
    operation()
}

pub fn remove(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
    entry_id: &str,
) -> Result<ContentInventory, ContentError> {
    with_instance_lock(instance, || {
        let inventory = scan(managed, instance, kind)?;
        let entry = inventory
            .entries
            .iter()
            .find(|entry| entry.entry_id == entry_id)
            .ok_or(ContentError::ChangedSinceScan)?;
        if !entry.management.can_remove || entry.management.removal_path != RemovalPath::LocalFile {
            return Err(
                match (
                    entry.management.removal_blocked_reason.clone(),
                    entry.management.removal_path,
                ) {
                    (Some(reason), _) => ContentError::UnsupportedActionWith(reason),
                    (None, RemovalPath::ProviderGraph) => ContentError::UnsupportedActionWith(
                        "Provider-managed content is removed through its provider lifecycle."
                            .into(),
                    ),
                    _ => ContentError::UnsupportedAction,
                },
            );
        }
        let directory = validate_directory(managed, instance, kind)?;
        validate_file_name(&entry.file_name)?;
        let target = directory.join(&entry.file_name);
        let meta = std::fs::symlink_metadata(&target).map_err(ContentError::Io)?;
        if !meta.is_file() || meta.file_type().is_symlink() || is_reparse_point(&meta) {
            return Err(ContentError::UnsafePath);
        }
        if std::fs::canonicalize(&target)
            .map_err(ContentError::Io)?
            .parent()
            != Some(
                std::fs::canonicalize(&directory)
                    .map_err(ContentError::Io)?
                    .as_path(),
            )
        {
            return Err(ContentError::UnsafePath);
        }
        let temporary = directory.join(format!(".content-removing-{}", uuid::Uuid::new_v4()));
        std::fs::rename(&target, &temporary).map_err(ContentError::Io)?;
        // Removing an enabled resource pack must also retire its enabled
        // reference in one coherent operation: if the options.txt update
        // fails, the file returns to its original name untouched.
        let activation_change =
            kind == ContentType::ResourcePack && entry.management.active == Some(true);
        if activation_change {
            let root = managed.instance_paths(instance).root().to_path_buf();
            if let Err(error) = crate::pack_activation::apply(
                &root,
                &[crate::pack_activation::ReferenceChange::Disable {
                    name: entry.file_name.clone(),
                }],
            ) {
                std::fs::rename(&temporary, &target).map_err(ContentError::Io)?;
                return Err(error);
            }
        }
        if let Err(error) = std::fs::remove_file(&temporary) {
            let mut restored = std::fs::rename(&temporary, &target).is_ok();
            if activation_change {
                let root = managed.instance_paths(instance).root().to_path_buf();
                restored &= crate::pack_activation::apply(
                    &root,
                    &[crate::pack_activation::ReferenceChange::Enable {
                        name: entry.file_name.clone(),
                    }],
                )
                .is_ok();
            }
            if !restored {
                return Err(ContentError::StateMalformed(
                    "removal failed and the original file could not be restored".into(),
                ));
            }
            return Err(ContentError::Io(error));
        }
        scan(managed, instance, kind)
    })
}

/// Toggles one resource pack's enabled reference in Minecraft's options.txt.
/// Shader packs and mods are refused: shader activation belongs to the
/// in-game shader loader, and mod enablement is the mods inventory's own
/// `.jar.disabled` lifecycle.
pub fn set_pack_enabled(
    managed: &ManagedPaths,
    instance: &InstanceId,
    kind: ContentType,
    entry_id: &str,
    enabled: bool,
) -> Result<ContentInventory, ContentError> {
    if kind != ContentType::ResourcePack {
        return Err(ContentError::UnsupportedActionWith(match kind {
            ContentType::ShaderPack => {
                "Shader packs are activated in-game with a compatible shader loader.".into()
            }
            _ => "Pack activation applies to resource packs only.".into(),
        }));
    }
    with_instance_lock(instance, || {
        let inventory = scan(managed, instance, kind)?;
        let entry = inventory
            .entries
            .iter()
            .find(|entry| entry.entry_id == entry_id)
            .ok_or(ContentError::ChangedSinceScan)?;
        if !entry.management.can_toggle {
            return Err(entry
                .management
                .toggle_blocked_reason
                .clone()
                .map(ContentError::UnsupportedActionWith)
                .unwrap_or(ContentError::UnsupportedAction));
        }
        let paths = managed.instance_paths(instance);
        let root = paths.root().to_path_buf();
        crate::pack_activation::apply(
            &root,
            &[if enabled {
                crate::pack_activation::ReferenceChange::Enable {
                    name: entry.file_name.clone(),
                }
            } else {
                crate::pack_activation::ReferenceChange::Disable {
                    name: entry.file_name.clone(),
                }
            }],
        )?;
        scan(managed, instance, kind)
    })
}

/// Backend-only normalized plan. Source URL authority belongs to its adapter.
/// The installed SHA-256 is filled only after expected-digest acquisition.
#[derive(Clone)]
pub struct ProviderInstallPlan {
    pub content_type: ContentType,
    pub provider: String,
    pub project_id: String,
    pub version_id: String,
    pub file_id: String,
    pub file_name: String,
    pub display_version: Option<String>,
    pub compatibility: ContentCompatibility,
    pub dependencies: Vec<ProviderDependency>,
    pub source: ProviderArtifactSource,
}

#[derive(Clone)]
pub enum ProviderArtifactSource {
    Sha256(ArtifactSource),
    Sha512(Sha512ArtifactSource),
}

impl ProviderInstallPlan {
    fn record(&self, sha256: String, origin: ProviderOrigin) -> ProviderRecord {
        ProviderRecord {
            content_type: self.content_type,
            provider: self.provider.clone(),
            project_id: self.project_id.clone(),
            version_id: self.version_id.clone(),
            file_id: self.file_id.clone(),
            file_name: self.file_name.clone(),
            sha256,
            display_version: self.display_version.clone(),
            compatibility: self.compatibility.clone(),
            dependencies: self.dependencies.clone(),
            explicitly_retained: true,
            requires: Vec::new(),
            origin,
            installed_at_unix_seconds: Some(now_unix_seconds()),
            pinned: false,
            update_channel: UpdateChannel::Stable,
        }
    }

    /// Build the lifecycle record for an acquired artifact. Pack-owned
    /// components register through the same shape as any provider content;
    /// the pack transaction finalizes `requires`, origin, and receipts.
    pub(crate) fn provider_record(&self, sha256: String, origin: ProviderOrigin) -> ProviderRecord {
        self.record(sha256, origin)
    }
}

pub(crate) fn now_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub async fn install_provider_artifact(
    managed: &ManagedPaths,
    instance: &InstanceId,
    plan: ProviderInstallPlan,
) -> Result<ContentInventory, ContentError> {
    let cache = ArtifactCache::new(managed.clone());
    let artifact = match &plan.source {
        ProviderArtifactSource::Sha256(source) => cache.acquire(source).await,
        ProviderArtifactSource::Sha512(source) => cache.acquire_sha512(source).await,
    }
    .map_err(|error| ContentError::Acquisition(error.to_string()))?;
    let record = plan.record(artifact.sha256.as_hex(), ProviderOrigin::Direct);
    record.validate()?;
    activate_verified(
        managed,
        instance,
        record,
        &artifact.path,
        Some(artifact.bytes),
    )
}

/// Acquire a dependency graph with bounded concurrency, then activate every
/// file under one instance lock. State is written only after all names exist;
/// any failure before that removes only names created by this transaction.
pub async fn install_provider_plans(
    managed: &ManagedPaths,
    instance: &InstanceId,
    plans: Vec<ProviderInstallPlan>,
) -> Result<Vec<ProviderRecord>, ContentError> {
    let revision = local_inventory_revision(managed, instance)?;
    install_provider_plans_reviewed(managed, instance, plans, Some(&revision)).await
}

/// Install every file authored by one exact pack snapshot. Pack provenance is
/// per file; project identity remains useful for dependency and ownership guards.
pub(crate) async fn install_pack_provider_plans(
    managed: &ManagedPaths,
    instance: &InstanceId,
    plans: Vec<ProviderInstallPlan>,
) -> Result<Vec<ProviderRecord>, ContentError> {
    let revision = local_inventory_revision(managed, instance)?;
    let acquired = acquire_provider_plans(managed, plans).await?;
    activate_provider_transaction_with_revision(
        managed,
        instance,
        acquired,
        Some(&revision),
        true,
        |state| state.save(managed, instance),
    )
}

pub(crate) async fn install_provider_plans_reviewed(
    managed: &ManagedPaths,
    instance: &InstanceId,
    plans: Vec<ProviderInstallPlan>,
    revision: Option<&str>,
) -> Result<Vec<ProviderRecord>, ContentError> {
    let acquired = acquire_provider_plans(managed, plans).await?;
    activate_provider_transaction_with_revision(
        managed,
        instance,
        acquired,
        revision,
        false,
        |state| state.save(managed, instance),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConflict {
    pub mod_id: Option<String>,
    pub file_name: String,
    pub ownership: crate::instance_mods::ModOwnership,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencySatisfaction {
    pub mod_id: String,
    pub version: String,
    pub requirement: String,
    pub file_name: String,
    pub ownership: crate::instance_mods::ModOwnership,
}

pub(crate) fn local_inventory_revision(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<String, ContentError> {
    if !managed.instance_paths(instance).mods().is_dir() {
        return Ok(String::new());
    }
    let inventory = crate::instance_mods::scan(managed, instance)
        .map_err(|e| ContentError::StateMalformed(e.to_string()))?;
    let bytes =
        serde_json::to_vec(&inventory).map_err(|_| ContentError::InvalidProviderArtifact)?;
    Ok(format!("{:x}", sha2::Sha256::digest(bytes)))
}

/// Reconcile a provider graph with locally hashed capabilities. A published
/// SHA-512 match establishes project identity for resolution only; it never
/// creates ownership. Otherwise verified candidate metadata establishes the
/// identity and the parent's actual Fabric predicate governs satisfaction.
pub async fn reconcile_provider_resolution(
    managed: &ManagedPaths,
    instance: &InstanceId,
    resolved: &mut crate::modrinth::Resolved,
) -> Result<(), ContentError> {
    if resolved.preview.content_type != ContentType::Mod || resolved.plans.is_empty() {
        return Ok(());
    }
    let inventory = crate::instance_mods::scan(managed, instance)
        .map_err(|e| ContentError::StateMalformed(e.to_string()))?;
    resolved.preview.inventory_revision = Some(local_inventory_revision(managed, instance)?);
    let cache = ArtifactCache::new(managed.clone());
    let root = resolved
        .plans
        .last()
        .ok_or(ContentError::InvalidProviderArtifact)?;
    let artifact = match &root.source {
        ProviderArtifactSource::Sha256(s) => cache.acquire(s).await,
        ProviderArtifactSource::Sha512(s) => cache.acquire_sha512(s).await,
    }
    .map_err(|e| ContentError::Acquisition(e.to_string()))?;
    let platform_kind = crate::instance_mods::platform_kind_of(managed, instance);
    let (root_metadata, warnings) =
        crate::instance_mods::inspect_mod_metadata(&artifact.path, artifact.bytes, &platform_kind);
    let root_metadata = root_metadata.ok_or(ContentError::InvalidProviderArtifact)?;
    let all_dependencies: Vec<_> = resolved
        .plans
        .iter()
        .flat_map(|p| p.dependencies.clone())
        .collect();
    let mut candidates = Vec::new();
    if warnings
        .iter()
        .any(|w| w.code.starts_with("nested_") || w.code == "mixin_metadata_invalid")
    {
        return Err(ContentError::InvalidProviderArtifact);
    }
    let mut retained = Vec::new();
    let count = resolved.plans.len();
    for (index, plan) in resolved.plans.iter().enumerate() {
        if index + 1 == count || plan.content_type != ContentType::Mod {
            continue;
        }
        let exact = inventory.entries.iter().find(|entry| {
            if !crate::mod_compatibility::usable(entry) {
                return false;
            }
            match &plan.source {
                ProviderArtifactSource::Sha256(s) => {
                    entry.sha256.as_deref() == Some(s.sha256().as_hex().as_str())
                }
                ProviderArtifactSource::Sha512(s) => {
                    let path = managed
                        .instance_paths(instance)
                        .mods()
                        .join(&entry.file_name);
                    std::fs::File::open(path).is_ok_and(|mut file| {
                        let mut hash = sha2::Sha512::new();
                        let mut buffer = [0u8; 65536];
                        loop {
                            match file.read(&mut buffer) {
                                Ok(0) => break,
                                Ok(n) => hash.update(&buffer[..n]),
                                Err(_) => return false,
                            }
                        }
                        format!("{:x}", hash.finalize()) == s.sha512().as_hex()
                    })
                }
            }
        });
        let candidate = if let Some(entry) = exact {
            entry.metadata.clone().unwrap()
        } else {
            let artifact = match &plan.source {
                ProviderArtifactSource::Sha256(s) => cache.acquire(s).await,
                ProviderArtifactSource::Sha512(s) => cache.acquire_sha512(s).await,
            }
            .map_err(|e| ContentError::Acquisition(e.to_string()))?;
            let (metadata, warnings) =
                crate::instance_mods::inspect_fabric_metadata(&artifact.path, artifact.bytes);
            if warnings
                .iter()
                .any(|w| w.code.starts_with("nested_") || w.code == "mixin_metadata_invalid")
            {
                return Err(ContentError::InvalidProviderArtifact);
            }
            metadata.ok_or(ContentError::InvalidProviderArtifact)?
        };
        candidates.push((index, candidate, exact.map(|e| e.entry_id.clone())));
    }
    // Evaluate all parents, including transitive parents, before dropping a
    // download. OR alternatives stay grouped in their normalized predicate.
    let parents: Vec<_> = std::iter::once(&root_metadata)
        .chain(candidates.iter().map(|(_, m, _)| m))
        .collect();
    let decisions: Vec<_> = candidates
        .iter()
        .map(|(index, candidate, exact)| {
            let constraints: Vec<_> = parents
                .iter()
                .flat_map(|m| m.depends.iter())
                .filter(|r| r.mod_id == candidate.id)
                .map(|r| r.requirement.clone())
                .collect();
            let constraints = if constraints.is_empty() {
                vec![format!(
                    "={}",
                    candidate.version.as_deref().unwrap_or("unknown")
                )]
            } else {
                constraints
            };
            (*index, candidate.clone(), exact.clone(), constraints)
        })
        .collect();
    for (index, plan) in resolved.plans.drain(..).enumerate() {
        let Some((_, candidate, exact, constraints)) =
            decisions.iter().find(|(i, _, _, _)| *i == index)
        else {
            retained.push(plan);
            continue;
        };
        let requirement = constraints
            .iter()
            .map(|p| format!("({p})"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let existing = inventory
            .entries
            .iter()
            .filter(|e| crate::mod_compatibility::usable(e))
            .find(|entry| {
                entry
                    .metadata
                    .as_ref()
                    .and_then(|m| crate::mod_compatibility::version_for(m, &candidate.id))
                    .is_some_and(|v| {
                        constraints
                            .iter()
                            .all(|p| crate::fabric::versions::satisfies(v, p) == Ok(true))
                    })
            });
        if let Some(entry) = existing {
            // Exact Modrinth pins still require the exact published artifact;
            // no Fabric predicate silently weakens a provider version pin.
            let pinned = all_dependencies
                .iter()
                .any(|d| d.project_id == plan.project_id && d.version_id.is_some());
            if pinned && exact.is_none() {
                return Err(ContentError::DependencyBlocked(format!(
                    "{} requires exact provider version {} of {}; installed {} is a different artifact.",
                    root_metadata.id, plan.version_id, candidate.id, entry.file_name
                )));
            }
            let version = entry
                .metadata
                .as_ref()
                .and_then(|m| crate::mod_compatibility::version_for(m, &candidate.id))
                .unwrap()
                .to_owned();
            if let Some(item) = resolved
                .preview
                .items
                .iter_mut()
                .find(|i| i.project_id == plan.project_id)
            {
                item.already_installed = true;
                item.satisfied_by = Some(DependencySatisfaction {
                    mod_id: candidate.id.clone(),
                    version,
                    requirement,
                    file_name: entry.file_name.clone(),
                    ownership: entry.ownership,
                });
            }
        } else {
            if let Some(entry) = inventory.entries.iter().find(|e| {
                e.metadata.as_ref().is_some_and(|m| {
                    m.id == candidate.id || m.nested_mod_ids.contains(&candidate.id)
                })
            }) {
                return Err(ContentError::ModCollision(ProviderConflict {
                    mod_id: Some(candidate.id.clone()),
                    file_name: entry.file_name.clone(),
                    ownership: entry.ownership,
                    reason: format!(
                        "{} requires {} {}. Installed: {} ({:?}, {}). {} No file will be replaced or adopted.",
                        root_metadata.id,
                        candidate.id,
                        requirement,
                        entry
                            .metadata
                            .as_ref()
                            .and_then(|m| crate::mod_compatibility::version_for(m, &candidate.id))
                            .unwrap_or("unreadable version"),
                        entry.ownership,
                        entry.file_name,
                        if !entry.enabled {
                            "The installed dependency is disabled; re-enable it explicitly."
                        } else {
                            "The active artifact does not satisfy the requirement or cannot be verified safely."
                        }
                    ),
                }));
            }
            retained.push(plan);
        }
    }
    resolved.plans = retained;
    Ok(())
}

/// Review may populate the verified cache, but never changes instance content or ownership.
pub async fn preview_provider_conflicts(
    managed: &ManagedPaths,
    instance: &InstanceId,
    plans: &[ProviderInstallPlan],
) -> Result<Vec<ProviderConflict>, ContentError> {
    let cache = ArtifactCache::new(managed.clone());
    let mut conflicts = Vec::new();
    for plan in plans {
        validate_file_name(&plan.file_name)?;
        let artifact = match &plan.source {
            ProviderArtifactSource::Sha256(source) => cache.acquire(source).await,
            ProviderArtifactSource::Sha512(source) => cache.acquire_sha512(source).await,
        }
        .map_err(|error| ContentError::Acquisition(error.to_string()))?;
        let record = plan.record(artifact.sha256.as_hex(), ProviderOrigin::Direct);
        record.validate()?;
        match validate_provider_mod_artifact(
            managed,
            instance,
            &record,
            &artifact.path,
            artifact.bytes,
        ) {
            Err(ContentError::ModCollision(conflict)) => {
                conflicts.push(conflict);
                continue;
            }
            other => other?,
        }
        let directory = validate_directory(managed, instance, record.content_type)?;
        // A content directory that does not exist yet simply has no files to
        // collide with; activation creates it. Reading it as an error broke
        // every first install into a fresh destination (shader packs).
        match std::fs::read_dir(&directory) {
            Ok(entries) => {
                if let Some(name) = entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .find(|name| name.eq_ignore_ascii_case(&record.file_name))
                {
                    conflicts.push(ProviderConflict {
                        mod_id: None,
                        file_name: name,
                        ownership: crate::instance_mods::ModOwnership::Unknown,
                        reason:
                            "A destination file already exists; it will not be overwritten or adopted."
                                .into(),
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ContentError::Io(error)),
        }
    }
    Ok(conflicts)
}

pub(crate) async fn acquire_provider_plans(
    managed: &ManagedPaths,
    plans: Vec<ProviderInstallPlan>,
) -> Result<Vec<(ProviderRecord, PathBuf, u64)>, ContentError> {
    if plans.is_empty() || plans.len() > 64 {
        return Err(ContentError::StateMalformed(
            "provider plan size is invalid".into(),
        ));
    }
    for plan in &plans {
        validate_file_name(&plan.file_name)?;
        let provisional = plan.record("0".repeat(64), ProviderOrigin::Direct);
        provisional.validate()?;
    }
    // Distinct destinations may name the same exact provider file. Acquire
    // each expected digest once even when concurrent cache misses occur.
    let mut unique = Vec::new();
    let mut indices = Vec::with_capacity(plans.len());
    let mut by_digest = HashMap::new();
    for (position, plan) in plans.iter().enumerate() {
        let key = match &plan.source {
            ProviderArtifactSource::Sha256(source) => {
                format!("sha256:{}", source.sha256().as_hex())
            }
            ProviderArtifactSource::Sha512(source) => {
                format!("sha512:{}", source.sha512().as_hex())
            }
        };
        let index = *by_digest.entry(key).or_insert_with(|| {
            let index = unique.len();
            unique.push(position);
            index
        });
        indices.push(index);
    }
    let cache = Arc::new(ArtifactCache::new(managed.clone()));
    let acquired = stream::iter(unique.into_iter().map(|position| {
        let cache = cache.clone();
        let source = &plans[position].source;
        async move {
            let artifact = match source {
                ProviderArtifactSource::Sha256(source) => cache.acquire(source).await,
                ProviderArtifactSource::Sha512(source) => cache.acquire_sha512(source).await,
            }
            .map_err(|error| ContentError::Acquisition(error.to_string()))?;
            Ok::<_, ContentError>((artifact.sha256.as_hex(), artifact.path, artifact.bytes))
        }
    }))
    .buffered(8)
    .collect::<Vec<_>>()
    .await;
    let acquired = acquired.into_iter().collect::<Result<Vec<_>, _>>()?;
    plans
        .into_iter()
        .zip(indices)
        .map(|(plan, index)| {
            let (sha256, path, bytes) = &acquired[index];
            let expected_size = match &plan.source {
                ProviderArtifactSource::Sha256(source) => source.size_bytes(),
                ProviderArtifactSource::Sha512(source) => source.size_bytes(),
            };
            if expected_size.is_some_and(|size| size != *bytes) {
                return Err(ContentError::Acquisition(
                    "provider artifact size conflicts with another reference to the same digest"
                        .into(),
                ));
            }
            let record = plan.record(sha256.clone(), ProviderOrigin::Direct);
            record.validate()?;
            Ok((record, path.clone(), *bytes))
        })
        .collect()
}

#[cfg(test)]
fn activate_provider_transaction(
    managed: &ManagedPaths,
    instance: &InstanceId,
    acquired: Vec<(ProviderRecord, PathBuf, u64)>,
    commit: impl FnOnce(&ContentState) -> Result<(), ContentError>,
) -> Result<Vec<ProviderRecord>, ContentError> {
    activate_provider_transaction_with_revision(managed, instance, acquired, None, false, commit)
}

fn activate_provider_transaction_with_revision(
    managed: &ManagedPaths,
    instance: &InstanceId,
    mut acquired: Vec<(ProviderRecord, PathBuf, u64)>,
    revision: Option<&str>,
    pack_install: bool,
    commit: impl FnOnce(&ContentState) -> Result<(), ContentError>,
) -> Result<Vec<ProviderRecord>, ContentError> {
    with_instance_lock(instance, || {
        if revision.is_some_and(|expected| {
            !local_inventory_revision(managed, instance).is_ok_and(|current| current == expected)
        }) {
            return Err(ContentError::ChangedSinceScan);
        }
        let mut state = ContentState::load(managed, instance)?;
        let direct = acquired
            .last()
            .map(|(record, _, _)| record.identity())
            .ok_or_else(|| ContentError::StateMalformed("empty provider graph".into()))?;
        let identities: Vec<_> = state
            .entries
            .iter()
            .map(ProviderRecord::identity)
            .chain(acquired.iter().map(|(record, _, _)| record.identity()))
            .collect();
        for (record, _, _) in &mut acquired {
            record.explicitly_retained = pack_install || record.identity() == direct;
            record.origin = if pack_install {
                ProviderOrigin::Pack
            } else if record.identity() == direct {
                ProviderOrigin::Direct
            } else {
                ProviderOrigin::Dependency
            };
            record.installed_at_unix_seconds = Some(now_unix_seconds());
            record.requires = record
                .dependencies
                .iter()
                .filter(|dependency| dependency.kind == DependencyKind::Required)
                .filter_map(|dependency| {
                    identities
                        .iter()
                        .find(|identity| {
                            identity.provider == dependency.provider
                                && identity.project_id == dependency.project_id
                        })
                        .cloned()
                })
                .collect();
        }
        let mut keys = HashSet::new();
        let mut targets = Vec::new();
        let required = crate::instance_mods::managed_artifact_file_name(managed, instance)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?
            .unwrap_or_default();
        for (record, _, _) in &acquired {
            let key = (record.content_type, record.file_name.to_lowercase());
            if !keys.insert(key.clone())
                || state.entries.iter().any(|entry| {
                    entry.content_type == record.content_type
                        && entry.file_name.eq_ignore_ascii_case(&record.file_name)
                })
                || (record.content_type == ContentType::Mod
                    && required
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(&record.file_name)))
            {
                return Err(ContentError::Collision);
            }
            let directory = ensure_directory(managed, instance, record.content_type)?;
            let target = directory.join(&record.file_name);
            if std::fs::symlink_metadata(&target).is_ok()
                || std::fs::read_dir(&directory)
                    .map_err(ContentError::Io)?
                    .any(|item| {
                        item.is_ok_and(|item| {
                            item.file_name()
                                .to_string_lossy()
                                .eq_ignore_ascii_case(&record.file_name)
                        })
                    })
            {
                return Err(ContentError::Collision);
            }
            targets.push(target);
        }
        validate_projected_artifacts(managed, instance, &acquired, None)?;
        let mut created = Vec::new();
        let result = (|| {
            for ((record, source, bytes), target) in acquired.iter().zip(targets.iter()) {
                let digest = ArtifactDigest::parse(&record.sha256)
                    .map_err(|_| ContentError::HashMismatch)?;
                verify_file(source, &digest, Some(*bytes))
                    .map_err(|_| ContentError::HashMismatch)?;
                validate_provider_mod_artifact(managed, instance, record, source, *bytes)?;
                let temporary =
                    target.with_file_name(format!(".content-installing-{}", uuid::Uuid::new_v4()));
                if let Err(error) = std::fs::copy(source, &temporary) {
                    let _ = std::fs::remove_file(&temporary);
                    return Err(ContentError::Io(error));
                }
                if verify_file(&temporary, &digest, Some(*bytes)).is_err() {
                    let _ = std::fs::remove_file(&temporary);
                    return Err(ContentError::HashMismatch);
                }
                let linked = std::fs::hard_link(&temporary, target);
                let _ = std::fs::remove_file(&temporary);
                if let Err(error) = linked {
                    return if target.exists() {
                        Err(ContentError::Collision)
                    } else {
                        Err(ContentError::Io(error))
                    };
                }
                created.push(target.clone());
            }
            state
                .entries
                .extend(acquired.iter().map(|(record, _, _)| record.clone()));
            commit(&state)?;
            Ok(())
        })();
        if let Err(error) = result {
            for path in created.into_iter().rev() {
                std::fs::remove_file(path).map_err(ContentError::Io)?;
            }
            return Err(error);
        }
        Ok(acquired.into_iter().map(|(record, _, _)| record).collect())
    })
}

fn same_file(left: &ProviderRecord, right: &ProviderRecord) -> bool {
    left.content_type == right.content_type
        && left.file_name == right.file_name
        && left.sha256 == right.sha256
}

fn validate_projected_artifacts(
    managed: &ManagedPaths,
    instance: &InstanceId,
    artifacts: &[(ProviderRecord, PathBuf, u64)],
    java_major: Option<u32>,
) -> Result<(), ContentError> {
    validate_projected_artifacts_scoped(managed, instance, &[], &[], artifacts, java_major, None)
}

/// Projected-inventory validation for a pack reconciliation: the artifacts a
/// pack update activates, the managed records it retires, the file names
/// whose current bytes are acknowledged divergences the update preserves
/// (kept-local user files leave the projected check to their own drift
/// detection), and the Minecraft / loader identity the instance will have
/// after the update (which may be a game transition the registry record
/// does not reflect yet).
pub(crate) fn validate_projected_pack_artifacts(
    managed: &ManagedPaths,
    instance: &InstanceId,
    removed: &[ProviderRecord],
    tolerated: &[String],
    artifacts: &[(ProviderRecord, PathBuf, u64)],
    target_minecraft: &str,
    target_loader_version: &str,
) -> Result<(), ContentError> {
    validate_projected_artifacts_scoped(
        managed,
        instance,
        removed,
        tolerated,
        artifacts,
        None,
        Some((target_minecraft, target_loader_version)),
    )
}

fn validate_projected_artifacts_scoped(
    managed: &ManagedPaths,
    instance: &InstanceId,
    removed: &[ProviderRecord],
    tolerated: &[String],
    artifacts: &[(ProviderRecord, PathBuf, u64)],
    java_major: Option<u32>,
    target: Option<(&str, &str)>,
) -> Result<(), ContentError> {
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|e| ContentError::StateMalformed(e.to_string()))?;
    let Some(record) = registry.find(instance) else {
        return Ok(());
    };
    let (minecraft_version, loader_version) = match target {
        Some((minecraft, loader)) => (minecraft.to_string(), loader.to_string()),
        None => (
            record.installed().minecraft_version.clone(),
            record
                .installed()
                .platform
                .version()
                .unwrap_or_default()
                .to_string(),
        ),
    };
    let loader_family = record.installed().platform.kind();
    if !matches!(loader_family, "fabric" | "neoForge") {
        return Ok(());
    }
    let mut inventory = match std::fs::symlink_metadata(managed.instance_paths(instance).mods()) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::instance_mods::ModInventory {
                instance_id: instance.to_string(),
                entries: Vec::new(),
                missing_managed: Vec::new(),
            }
        }
        _ => crate::instance_mods::scan(managed, instance)
            .map_err(|e| ContentError::StateMalformed(e.to_string()))?,
    };
    // Retired managed files leave the projected inventory exactly as their
    // on-disk files leave the instance; the transaction proves the actual
    // bytes at retirement, so the file name identifies the leaving entry.
    // Tolerated names are acknowledged divergences the update preserves as
    // user content — their drift is watched by the pack state, not by this
    // transaction projection.
    for retired in removed
        .iter()
        .filter(|record| record.content_type == ContentType::Mod)
    {
        inventory
            .entries
            .retain(|entry| !entry.file_name.eq_ignore_ascii_case(&retired.file_name));
    }
    for name in tolerated {
        inventory
            .entries
            .retain(|entry| !entry.file_name.eq_ignore_ascii_case(name));
    }
    for (incoming, path, bytes) in artifacts
        .iter()
        .filter(|(r, _, _)| r.content_type == ContentType::Mod)
    {
        inventory.entries.retain(|entry| {
            entry
                .provenance
                .as_ref()
                .is_none_or(|old| old.identity() != incoming.identity())
        });
        let (metadata, warnings) =
            crate::instance_mods::inspect_mod_metadata(path, *bytes, loader_family);
        crate::mod_compatibility::add_artifact(
            &mut inventory,
            incoming.file_name.clone(),
            metadata.ok_or(ContentError::InvalidProviderArtifact)?,
            incoming.sha256.clone(),
            warnings,
        );
    }
    if let Some(problem) = crate::mod_compatibility::validate(
        &inventory,
        &minecraft_version,
        &loader_version,
        java_major,
        loader_family,
    )
    .first()
    {
        return Err(ContentError::ModCollision(ProviderConflict {
            mod_id: Some(problem.mod_id.clone()),
            file_name: problem.file_name.clone(),
            ownership: problem.ownership,
            reason: problem.message.clone(),
        }));
    }
    Ok(())
}

pub async fn preview_provider_requirements(
    managed: &ManagedPaths,
    instance: &InstanceId,
    plans: &[ProviderInstallPlan],
    java_major: u32,
) -> Result<(), ContentError> {
    let cache = ArtifactCache::new(managed.clone());
    let mut artifacts = Vec::new();
    for plan in plans.iter().filter(|p| p.content_type == ContentType::Mod) {
        let artifact = match &plan.source {
            ProviderArtifactSource::Sha256(s) => cache.acquire(s).await,
            ProviderArtifactSource::Sha512(s) => cache.acquire_sha512(s).await,
        }
        .map_err(|e| ContentError::Acquisition(e.to_string()))?;
        artifacts.push((
            plan.record(artifact.sha256.as_hex(), ProviderOrigin::Direct),
            artifact.path,
            artifact.bytes,
        ));
    }
    validate_projected_artifacts(managed, instance, &artifacts, Some(java_major))
}

struct RetiredFile {
    target: PathBuf,
    backup: PathBuf,
    rollback_copy: PathBuf,
}

/// Commit one provider-independent lifecycle state transition. Every old file
/// has a verified rollback copy before any target is moved; every new file is
/// copied and verified from the content-addressed store before activation.
#[cfg(test)]
fn apply_lifecycle_state(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    next: &ContentState,
    acquired: &[(ProviderRecord, PathBuf, u64)],
) -> Result<(), ContentError> {
    apply_lifecycle_state_with_revision(managed, instance, expected, next, acquired, None)
}

fn apply_lifecycle_state_with_revision(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    next: &ContentState,
    acquired: &[(ProviderRecord, PathBuf, u64)],
    revision: Option<&str>,
) -> Result<(), ContentError> {
    apply_lifecycle_state_reviewed(
        managed,
        instance,
        expected,
        next,
        acquired,
        revision,
        |state| state.save(managed, instance),
        || Ok(()),
    )
}

#[cfg(test)]
fn apply_lifecycle_state_with_hooks(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    next: &ContentState,
    acquired: &[(ProviderRecord, PathBuf, u64)],
    commit: impl FnOnce(&ContentState) -> Result<(), ContentError>,
    before_cleanup: impl FnOnce() -> Result<(), ContentError>,
) -> Result<(), ContentError> {
    apply_lifecycle_state_reviewed(
        managed,
        instance,
        expected,
        next,
        acquired,
        None,
        commit,
        before_cleanup,
    )
}

fn apply_lifecycle_state_reviewed(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    next: &ContentState,
    acquired: &[(ProviderRecord, PathBuf, u64)],
    revision: Option<&str>,
    commit: impl FnOnce(&ContentState) -> Result<(), ContentError>,
    before_cleanup: impl FnOnce() -> Result<(), ContentError>,
) -> Result<(), ContentError> {
    with_instance_lock(instance, || {
        if revision.is_some_and(|expected| {
            !local_inventory_revision(managed, instance).is_ok_and(|current| current == expected)
        }) {
            return Err(ContentError::ChangedSinceScan);
        }
        let current = ContentState::load(managed, instance)?;
        if let Some(pack) = crate::pack_state::InstalledPack::load(managed, instance)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?
        {
            for old in &current.entries {
                if pack.owns_provider(&old.identity()) && next.find(&old.identity()) != Some(old) {
                    return Err(ContentError::UnsupportedActionWith(
                        "This component belongs to the installed modpack; updating or removing it is part of a modpack update on the instance's Overview page.".into(),
                    ));
                }
            }
        }
        if !acquired.is_empty() {
            validate_projected_artifacts(managed, instance, acquired, None)?;
        }
        if &current != expected {
            return Err(ContentError::ChangedSinceScan);
        }
        next.validate()?;
        let required = crate::instance_mods::managed_artifact_file_name(managed, instance)
            .map_err(|error| ContentError::StateMalformed(error.to_string()))?
            .unwrap_or_default();
        let protected = |record: &ProviderRecord| {
            record.content_type == ContentType::Mod
                && required
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&record.file_name))
        };
        for record in &current.entries {
            if protected(record) {
                return Err(ContentError::Collision);
            }
            validate_provider_file(managed, instance, record)?;
        }
        let retired: Vec<_> = current
            .entries
            .iter()
            .filter(|record| {
                next.find(&record.identity())
                    .is_none_or(|updated| !same_file(record, updated))
            })
            .collect();
        let incoming: Vec<_> = next
            .entries
            .iter()
            .filter(|record| {
                current
                    .find(&record.identity())
                    .is_none_or(|old| !same_file(old, record))
            })
            .collect();
        let removed_mods: Vec<_> = retired
            .iter()
            .filter(|record| next.find(&record.identity()).is_none())
            .map(|record| (*record).clone())
            .collect();
        crate::instance_mods::validate_provider_removals(managed, instance, &removed_mods)
            .map_err(|error| ContentError::DependencyBlocked(error.to_string()))?;
        // Retired resource packs must not leave enabled references behind,
        // and a managed update that renames the pack file migrates the
        // reference. The change is applied inside the transaction, after the
        // state commits and before cleanup, and rolls back with everything
        // else on failure.
        let reference_changes: Vec<crate::pack_activation::ReferenceChange> = retired
            .iter()
            .filter(|record| record.content_type == ContentType::ResourcePack)
            .filter_map(|record| match next.find(&record.identity()) {
                None => Some(crate::pack_activation::ReferenceChange::Disable {
                    name: record.file_name.clone(),
                }),
                Some(updated) if !same_file(record, updated) => {
                    Some(crate::pack_activation::ReferenceChange::Rename {
                        from: record.file_name.clone(),
                        to: updated.file_name.clone(),
                    })
                }
                _ => None,
            })
            .collect();
        let options_path = managed.instance_paths(instance).root().join("options.txt");
        if incoming.len() != acquired.len() {
            return Err(ContentError::StateMalformed(
                "acquired graph does not match lifecycle state".into(),
            ));
        }
        let retired_paths: HashSet<_> = retired
            .iter()
            .map(|record| validate_provider_file(managed, instance, record))
            .collect::<Result<_, _>>()?;
        let mut staged = Vec::<(PathBuf, PathBuf)>::new();
        let mut rollback = Vec::<RetiredFile>::new();
        let mut activated = Vec::<PathBuf>::new();
        let mut moved = 0usize;
        let mut state_committed = false;
        let mut options_original: Option<Vec<u8>> = None;
        let result = (|| {
            let mut targets = HashSet::new();
            for record in &incoming {
                if protected(record)
                    || !targets.insert((record.content_type, record.file_name.to_lowercase()))
                {
                    return Err(ContentError::Collision);
                }
                let (acquired_record, source, size) = acquired
                    .iter()
                    .find(|(item, _, _)| item.identity() == record.identity())
                    .ok_or_else(|| {
                        ContentError::StateMalformed("acquired artifact is missing".into())
                    })?;
                let mut normalized_acquired = acquired_record.clone();
                normalized_acquired.explicitly_retained = record.explicitly_retained;
                normalized_acquired.requires = record.requires.clone();
                // Origin, the receipt timestamp and the update policy are
                // lifecycle bookkeeping assigned by the state planner; the
                // acquired record carries placeholders until the transaction
                // finalizes them.
                normalized_acquired.origin = record.origin;
                normalized_acquired.installed_at_unix_seconds = record.installed_at_unix_seconds;
                normalized_acquired.pinned = record.pinned;
                normalized_acquired.update_channel = record.update_channel;
                if normalized_acquired != **record {
                    return Err(ContentError::StateMalformed(
                        "acquired artifact changed".into(),
                    ));
                }
                let directory = ensure_directory(managed, instance, record.content_type)?;
                if record.content_type == ContentType::Mod
                    && current.entries.iter().any(|old| {
                        old.identity() == record.identity()
                            && validate_provider_file(managed, instance, old).is_ok_and(|path| {
                                path.extension()
                                    .is_some_and(|extension| extension == "disabled")
                            })
                    })
                {
                    return Err(ContentError::DependencyBlocked("Re-enable this mod before updating it. Removing a disabled mod is supported.".into()));
                }
                let target = directory.join(&record.file_name);
                if !retired_paths.contains(&target)
                    && (std::fs::symlink_metadata(&target).is_ok()
                        || std::fs::read_dir(&directory)
                            .map_err(ContentError::Io)?
                            .any(|entry| {
                                entry.is_ok_and(|entry| {
                                    entry
                                        .file_name()
                                        .to_string_lossy()
                                        .eq_ignore_ascii_case(&record.file_name)
                                })
                            }))
                {
                    return Err(ContentError::Collision);
                }
                let digest = ArtifactDigest::parse(&record.sha256)
                    .map_err(|_| ContentError::HashMismatch)?;
                verify_file(source, &digest, Some(*size))
                    .map_err(|_| ContentError::HashMismatch)?;
                validate_provider_mod_artifact(managed, instance, record, source, *size)?;
                let stage = directory.join(format!(".content-staged-{}", uuid::Uuid::new_v4()));
                staged.push((target, stage.clone()));
                std::fs::copy(source, &stage).map_err(ContentError::Io)?;
                verify_file(&stage, &digest, Some(*size))
                    .map_err(|_| ContentError::HashMismatch)?;
            }
            for record in &retired {
                let target = validate_provider_file(managed, instance, record)?;
                let directory = target.parent().ok_or(ContentError::UnsafePath)?;
                let backup = directory.join(format!(".content-retired-{}", uuid::Uuid::new_v4()));
                let rollback_copy =
                    directory.join(format!(".content-rollback-{}", uuid::Uuid::new_v4()));
                std::fs::copy(&target, &rollback_copy).map_err(ContentError::Io)?;
                rollback.push(RetiredFile {
                    target,
                    backup,
                    rollback_copy,
                });
                let digest = ArtifactDigest::parse(&record.sha256)
                    .map_err(|_| ContentError::HashMismatch)?;
                verify_file(
                    &rollback.last().expect("just pushed").rollback_copy,
                    &digest,
                    None,
                )
                .map_err(|_| ContentError::HashMismatch)?;
            }
            for file in &rollback {
                std::fs::rename(&file.target, &file.backup).map_err(ContentError::Io)?;
                moved += 1;
            }
            for (target, stage) in &staged {
                std::fs::hard_link(stage, target).map_err(|error| {
                    if target.exists() {
                        ContentError::Collision
                    } else {
                        ContentError::Io(error)
                    }
                })?;
                activated.push(target.clone());
            }
            commit(next)?;
            state_committed = true;
            if !reference_changes.is_empty() {
                options_original = std::fs::read(&options_path).ok();
                if let Some(root) = options_path.parent() {
                    crate::pack_activation::apply(root, &reference_changes)?;
                }
            }
            before_cleanup()?;
            for file in &rollback {
                std::fs::remove_file(&file.backup).map_err(ContentError::Io)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let mut rollback_failed = false;
            for target in activated.iter().rev() {
                if std::fs::remove_file(target).is_err() {
                    rollback_failed = true;
                }
            }
            for file in rollback.iter().take(moved).rev() {
                let source = if file.backup.exists() {
                    &file.backup
                } else {
                    &file.rollback_copy
                };
                if std::fs::hard_link(source, &file.target).is_err() {
                    rollback_failed = true;
                }
            }
            if state_committed && current.save(managed, instance).is_err() {
                rollback_failed = true;
            }
            if let Some(original) = &options_original {
                if std::fs::write(&options_path, original).is_err() {
                    rollback_failed = true;
                }
            }
            for (_, stage) in &staged {
                let _ = std::fs::remove_file(stage);
            }
            if rollback_failed {
                return Err(ContentError::StateMalformed(
                    "lifecycle rollback could not restore every file".into(),
                ));
            }
            for file in &rollback {
                let _ = std::fs::remove_file(&file.backup);
                let _ = std::fs::remove_file(&file.rollback_copy);
            }
            return Err(error);
        }
        for (_, stage) in &staged {
            let _ = std::fs::remove_file(stage);
        }
        for file in &rollback {
            let _ = std::fs::remove_file(&file.rollback_copy);
        }
        Ok(())
    })
}

pub fn remove_provider_graph(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    identity: &ProviderIdentity,
) -> Result<ContentState, ContentError> {
    let next = removal_state(expected, identity)?;
    apply_lifecycle_state_with_revision(managed, instance, expected, &next, &[], None)?;
    Ok(next)
}

pub async fn update_provider_graph(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    root: &ProviderIdentity,
    plans: Vec<ProviderInstallPlan>,
) -> Result<ContentState, ContentError> {
    let revision = local_inventory_revision(managed, instance)?;
    update_provider_graph_reviewed(managed, instance, expected, root, plans, Some(&revision)).await
}

pub(crate) async fn update_provider_graph_reviewed(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    root: &ProviderIdentity,
    plans: Vec<ProviderInstallPlan>,
    revision: Option<&str>,
) -> Result<ContentState, ContentError> {
    update_provider_graph_multi_reviewed(
        managed,
        instance,
        expected,
        std::slice::from_ref(root),
        plans,
        revision,
    )
    .await
}

/// Execute one coherent multi-root update transaction: acquire every planned
/// artifact, plan the combined next state, and commit it through the same
/// staged/verified/rollback lifecycle as every provider transition.
pub async fn update_provider_graph_multi(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    roots: &[ProviderIdentity],
    plans: Vec<ProviderInstallPlan>,
) -> Result<ContentState, ContentError> {
    let revision = local_inventory_revision(managed, instance)?;
    update_provider_graph_multi_reviewed(managed, instance, expected, roots, plans, Some(&revision))
        .await
}

pub(crate) async fn update_provider_graph_multi_reviewed(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    roots: &[ProviderIdentity],
    plans: Vec<ProviderInstallPlan>,
    revision: Option<&str>,
) -> Result<ContentState, ContentError> {
    let acquired = acquire_provider_plans(managed, plans).await?;
    let next = updated_state_multi(
        expected,
        roots,
        acquired
            .iter()
            .map(|(record, _, _)| record.clone())
            .collect(),
    )?;
    apply_lifecycle_state_with_revision(managed, instance, expected, &next, &acquired, revision)?;
    Ok(next)
}

/// Test-only multi-root update with injectable commit/cleanup hooks so
/// deterministic failures can be simulated at every transaction stage.
#[cfg(test)]
async fn update_provider_graph_multi_with_hooks(
    managed: &ManagedPaths,
    instance: &InstanceId,
    expected: &ContentState,
    roots: &[ProviderIdentity],
    plans: Vec<ProviderInstallPlan>,
    commit: impl FnOnce(&ContentState) -> Result<(), ContentError>,
) -> Result<ContentState, ContentError> {
    let acquired = acquire_provider_plans(managed, plans).await?;
    let next = updated_state_multi(
        expected,
        roots,
        acquired
            .iter()
            .map(|(record, _, _)| record.clone())
            .collect(),
    )?;
    apply_lifecycle_state_reviewed(
        managed,
        instance,
        expected,
        &next,
        &acquired,
        None,
        commit,
        || Ok(()),
    )?;
    Ok(next)
}

/// Change only the persisted update policy of one managed record. Files,
/// bytes, versions and provenance are untouched; the next lifecycle
/// fingerprint changes because the managed state changed.
pub fn set_update_policy(
    managed: &ManagedPaths,
    instance: &InstanceId,
    identity: &ProviderIdentity,
    pinned: Option<bool>,
    channel: Option<UpdateChannel>,
) -> Result<ProviderRecord, ContentError> {
    with_instance_lock(instance, || {
        let mut state = ContentState::load(managed, instance)?;
        let record = state
            .entries
            .iter_mut()
            .find(|record| record.identity() == *identity)
            .ok_or(ContentError::ChangedSinceScan)?;
        if record.provider != "modrinth" {
            return Err(ContentError::UnsupportedAction);
        }
        if pinned.is_none() && channel.is_none() {
            return Err(ContentError::StateMalformed(
                "policy change is empty".into(),
            ));
        }
        if let Some(pinned) = pinned {
            record.pinned = pinned;
        }
        if let Some(channel) = channel {
            record.update_channel = channel;
        }
        let updated = record.clone();
        state.save(managed, instance)?;
        Ok(updated)
    })
}

fn activate_verified(
    managed: &ManagedPaths,
    instance: &InstanceId,
    record: ProviderRecord,
    verified_path: &Path,
    expected_size: Option<u64>,
) -> Result<ContentInventory, ContentError> {
    activate_verified_with_commit(
        managed,
        instance,
        record,
        verified_path,
        expected_size,
        |state| state.save(managed, instance),
    )
}

fn validate_provider_mod_artifact(
    managed: &ManagedPaths,
    instance: &InstanceId,
    record: &ProviderRecord,
    path: &Path,
    bytes: u64,
) -> Result<(), ContentError> {
    if record.content_type != ContentType::Mod {
        return Ok(());
    }
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
    let platform_kind = crate::instance_mods::platform_kind_of(managed, instance);
    if let Some(instance_record) = registry.find(instance) {
        let installed = instance_record.installed();
        let loader = installed
            .platform
            .provider_loader()
            .map_err(|_| ContentError::UnsupportedAction)?;
        if record.compatibility.loader.as_deref() != Some(loader)
            || !record
                .compatibility
                .minecraft_versions
                .contains(&installed.minecraft_version)
        {
            return Err(ContentError::UnsupportedAction);
        }
    }
    let (metadata, _) = crate::instance_mods::inspect_mod_metadata(path, bytes, &platform_kind);
    let Some(metadata) = metadata else {
        return Err(ContentError::InvalidProviderArtifact);
    };
    // Ownership is configuration-derived. A provider may supply any otherwise
    // compatible mod identity unless a verified active launcher requirement owns it.
    crate::instance_mods::verified_required_mods(managed, instance)
        .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
    let inventory = crate::instance_mods::scan(managed, instance)
        .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
    for entry in &inventory.entries {
        // Lifecycle updates may replace only their own verified provider record.
        if entry.ownership == crate::instance_mods::ModOwnership::ProviderManaged
            && entry
                .provenance
                .as_ref()
                .is_some_and(|old| old.identity() == record.identity())
        {
            continue;
        }
        if let Some(existing) = &entry.metadata {
            if let Some(id) = conflicting_mod_identity(&metadata, existing) {
                return Err(ContentError::ModCollision(ProviderConflict {
                    mod_id: Some(id.into()), file_name: entry.file_name.clone(), ownership: entry.ownership,
                    reason: "A top-level mod identity would duplicate an installed mod or bundled module. Neither file will be replaced or adopted.".into(),
                }));
            }
        }
    }
    // Required content was deeply verified above; shared nested modules are descriptive
    // declarations, not independently owned top-level artifacts. Fabric resolves them.
    Ok(())
}

pub(crate) fn conflicting_mod_identity<'a>(
    incoming: &'a crate::instance_mods::ModMetadata,
    existing: &crate::instance_mods::ModMetadata,
) -> Option<&'a str> {
    if incoming.id.eq_ignore_ascii_case(&existing.id) {
        return Some(&incoming.id);
    }
    if existing
        .nested_mod_ids
        .iter()
        .any(|id| id.eq_ignore_ascii_case(&incoming.id))
    {
        let same = crate::mod_compatibility::version_for(existing, &incoming.id)
            .zip(incoming.version.as_deref())
            .is_some_and(|(a, b)| {
                crate::fabric::versions::satisfies(a, &format!("={b}")) == Ok(true)
            });
        if !same {
            return Some(&incoming.id);
        }
    }
    incoming
        .nested_mod_ids
        .iter()
        .find(|id| {
            id.eq_ignore_ascii_case(&existing.id)
                && !crate::mod_compatibility::version_for(incoming, id)
                    .zip(existing.version.as_deref())
                    .is_some_and(|(a, b)| {
                        crate::fabric::versions::satisfies(a, &format!("={b}")) == Ok(true)
                    })
        })
        .map(String::as_str)
}

fn activate_verified_with_commit(
    managed: &ManagedPaths,
    instance: &InstanceId,
    record: ProviderRecord,
    verified_path: &Path,
    expected_size: Option<u64>,
    commit: impl FnOnce(&ContentState) -> Result<(), ContentError>,
) -> Result<ContentInventory, ContentError> {
    with_instance_lock(instance, || {
        record.validate()?;
        let digest =
            ArtifactDigest::parse(&record.sha256).map_err(|_| ContentError::HashMismatch)?;
        verify_file(verified_path, &digest, expected_size)
            .map_err(|_| ContentError::HashMismatch)?;
        let bytes = std::fs::metadata(verified_path)
            .map_err(ContentError::Io)?
            .len();
        validate_provider_mod_artifact(managed, instance, &record, verified_path, bytes)?;
        let kind = record.content_type;
        if kind == ContentType::Mod {
            let required = crate::instance_mods::managed_artifact_file_name(managed, instance)
                .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
            if required.is_some_and(|files| {
                files
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(&record.file_name))
            }) {
                return Err(ContentError::Collision);
            }
        }
        let directory = ensure_directory(managed, instance, kind)?;
        let mut state = ContentState::load(managed, instance)?;
        let target = directory.join(&record.file_name);
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(ContentError::Collision);
        }
        if state.entries.iter().any(|entry| {
            entry.content_type == kind && entry.file_name.eq_ignore_ascii_case(&record.file_name)
        }) {
            return Err(ContentError::Collision);
        }
        let temporary = directory.join(format!(".content-installing-{}", uuid::Uuid::new_v4()));
        if let Err(error) = std::fs::copy(verified_path, &temporary) {
            let _ = std::fs::remove_file(&temporary);
            return Err(ContentError::Io(error));
        }
        if verify_file(&temporary, &digest, expected_size).is_err() {
            let _ = std::fs::remove_file(&temporary);
            return Err(ContentError::HashMismatch);
        }
        // A hard link creates the exact final name without overwriting an
        // entry that raced into place. It links only this private copy, never
        // the shared cache object. The temporary name is then removed.
        if let Err(error) = std::fs::hard_link(&temporary, &target) {
            let _ = std::fs::remove_file(&temporary);
            return if target.exists() {
                Err(ContentError::Collision)
            } else {
                Err(ContentError::Io(error))
            };
        }
        let _ = std::fs::remove_file(&temporary);
        state.entries.push(record);
        if let Err(error) = commit(&state) {
            let _ = std::fs::remove_file(&target);
            return Err(error);
        }
        scan(managed, instance, kind)
    })
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse_point(_: &std::fs::Metadata) -> bool {
    false
}

#[derive(Debug)]
pub enum ContentError {
    DependencyBlocked(String),
    Io(std::io::Error),
    UnsafePath,
    StateMalformed(String),
    StateVersion(u64),
    ChangedSinceScan,
    UnsupportedAction,
    Collision,
    ModCollision(ProviderConflict),
    HashMismatch,
    OperationInProgress,
    Acquisition(String),
    InvalidProviderArtifact,
    RequiredByInstalledContent,
    InvalidApproval,
    OptionsMalformed(String),
    UnsupportedActionWith(String),
}
impl ContentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DependencyBlocked(_) => "mod_dependency_blocked",
            Self::Io(_) => "content_unavailable",
            Self::UnsafePath => "content_unsafe_path",
            Self::StateMalformed(_) | Self::StateVersion(_) => "content_state_malformed",
            Self::ChangedSinceScan => "content_changed_since_scan",
            Self::UnsupportedAction | Self::UnsupportedActionWith(_) => {
                "unsupported_content_action"
            }
            Self::OptionsMalformed(_) => "options_malformed",
            Self::Collision | Self::ModCollision(_) => "content_collision",
            Self::HashMismatch => "content_hash_mismatch",
            Self::OperationInProgress => "content_operation_in_progress",
            Self::Acquisition(_) => "content_acquisition_failed",
            Self::InvalidProviderArtifact => "content_invalid_artifact",
            Self::RequiredByInstalledContent => "content_required_by_installed",
            Self::InvalidApproval => "content_invalid_approval",
        }
    }
}
impl fmt::Display for ContentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DependencyBlocked(reason) => formatter.write_str(reason),
            Self::Io(error) => write!(formatter, "content I/O failed: {error}"),
            Self::UnsafePath => formatter.write_str("content path is unsafe"),
            Self::StateMalformed(reason) => {
                write!(formatter, "content state is malformed: {reason}")
            }
            Self::StateVersion(version) => {
                write!(formatter, "content state schema {version} is unsupported")
            }
            Self::ChangedSinceScan => formatter.write_str("content changed since the scan"),
            Self::UnsupportedAction => formatter.write_str("this content action is unsupported"),
            Self::Collision => formatter.write_str("content target already exists"),
            Self::ModCollision(conflict) => write!(
                formatter,
                "{} conflicts with {:?} file '{}': {}",
                conflict.mod_id.as_deref().unwrap_or("Content"),
                conflict.ownership,
                conflict.file_name,
                conflict.reason
            ),
            Self::HashMismatch => formatter.write_str("content hash does not match"),
            Self::OperationInProgress => {
                formatter.write_str("another content operation is in progress")
            }
            Self::Acquisition(reason) => write!(formatter, "content acquisition failed: {reason}"),
            Self::InvalidProviderArtifact => write!(
                formatter,
                "the provider artifact is not a valid Fabric mod JAR"
            ),
            Self::RequiredByInstalledContent => {
                formatter.write_str("this content is still required by another installed item")
            }
            Self::InvalidApproval => write!(
                formatter,
                "the recognition approval is invalid; scan again and re-approve"
            ),
            Self::OptionsMalformed(reason) => {
                write!(formatter, "Minecraft's options.txt is malformed: {reason}")
            }
            Self::UnsupportedActionWith(reason) => formatter.write_str(reason),
        }
    }
}
impl std::error::Error for ContentError {}

#[cfg(test)]
mod compatibility_acceptance {
    use super::*;

    /// Explicit live provider acceptance against a session-created disposable
    /// copy. Never points at the production application-data directory.
    #[tokio::test]
    #[ignore = "requires explicit disposable copy and live Modrinth"]
    async fn reproduce_provider_compatibility() {
        let root =
            PathBuf::from(std::env::var_os("AURORA_COMPATIBILITY_ROOT").expect("disposable root"));
        assert!(root.starts_with(std::env::temp_dir()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-compatibility-")
        );
        let managed = ManagedPaths::from_app_local_data_dir(root).unwrap();
        let instance = InstanceId::new("8a59a5fe0a354c3fab444b38fc33d73f").unwrap();
        let cache = ArtifactCache::new(managed.clone());
        let source = Sha512ArtifactSource::https(
            "https://cdn.modrinth.com/data/eXts2L7r/versions/qxjzQ9xY/placeholder-api-2.8.2%2B1.21.10.jar",
            "507ab10b7938dcd14d33121b8462649bdbe575cef248e917dfdf7566078ab5d0195ca1add95eae4863de3f652eb56db0a8669a67d5b5344e094d086f9dab5a08",
            Some(268662),
        ).unwrap();
        let artifact = cache.acquire_sha512(&source).await.unwrap();
        let placeholder = managed
            .instance_paths(&instance)
            .mods()
            .join("placeholder-api-2.8.2+1.21.10.jar");
        if !placeholder.exists() {
            std::fs::copy(&artifact.path, &placeholder).unwrap();
        }
        let (metadata, warnings) =
            crate::instance_mods::inspect_fabric_metadata(&placeholder, artifact.bytes);
        println!("Placeholder: {metadata:?}; warnings: {warnings:?}");
        let context = crate::modrinth::Context {
            minecraft_version: "1.21.11".into(),
            loader: "fabric".into(),
            fabric_api_protected: true,
        };
        let state = ContentState::load(&managed, &instance).unwrap();
        let inventory = crate::instance_mods::scan(&managed, &instance).unwrap();
        let issues =
            crate::mod_compatibility::validate(&inventory, "1.21.11", "0.19.5", Some(21), "fabric");
        println!("Existing environment issues: {issues:?}");
        assert!(
            issues.is_empty(),
            "known-good environment must remain usable"
        );

        // Read-only acquisition of exact published diagnostic fixtures. These
        // are never activated, and all supplied Modrinth bytes use SHA-512.
        let evidence = managed
            .launcher_dir()
            .parent()
            .unwrap()
            .parent()
            .expect("evidence directory")
            .to_path_buf();
        for name in ["iris", "chloride"] {
            let json: serde_json::Value = serde_json::from_slice(
                &std::fs::read(evidence.join(format!("{name}-metadata.json")))
                    .unwrap()
                    .into_iter()
                    .skip_while(|b| matches!(*b, 0xef | 0xbb | 0xbf))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            let file = &json["version"]["files"][0];
            let source = Sha512ArtifactSource::https(
                file["url"].as_str().unwrap(),
                file["hashes"]["sha512"].as_str().unwrap(),
                file["size"].as_u64(),
            )
            .unwrap();
            let artifact = cache.acquire_sha512(&source).await.unwrap();
            let (metadata, warnings) =
                crate::instance_mods::inspect_fabric_metadata(&artifact.path, artifact.bytes);
            println!(
                "Verified {name}: sha256={}, metadata={metadata:?}, warnings={warnings:?}",
                artifact.sha256.as_hex()
            );
        }
        let java25 = evidence.join("java25-inspection.jar");
        let bytes = std::fs::read(&java25).unwrap();
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(&bytes)),
            "db9446e3956ac7d7527e470bc8fae89fd48f36605ce792165401135465f18e7c"
        );
        let (metadata, warnings) =
            crate::instance_mods::inspect_fabric_metadata(&java25, bytes.len() as u64);
        assert!(warnings.is_empty());
        let metadata = metadata.unwrap();
        assert_eq!(metadata.mixin_java_requirements.len(), 2);
        let mut projected = inventory.clone();
        crate::mod_compatibility::add_artifact(
            &mut projected,
            "java25-fixture.jar".into(),
            metadata,
            "db9446e3956ac7d7527e470bc8fae89fd48f36605ce792165401135465f18e7c".into(),
            warnings,
        );
        let issues =
            crate::mod_compatibility::validate(&projected, "1.21.11", "0.19.5", Some(21), "fabric");
        assert!(issues.iter().any(|i| i.code == "mod_java_incompatible"));
        println!("Exact retained Java25 artifact pre-Play issues: {issues:?}");
        for project in ["mOgUt4GM", "YL57xq9U"] {
            match crate::modrinth::Client::official()
                .resolve_latest(&context, ContentType::Mod, project, &state)
                .await
            {
                Ok(mut resolved) => {
                    reconcile_provider_resolution(&managed, &instance, &mut resolved)
                        .await
                        .unwrap();
                    preview_provider_requirements(&managed, &instance, &resolved.plans, 21)
                        .await
                        .unwrap();
                    println!(
                        "Project {project}: {}",
                        serde_json::to_string(&resolved.preview).unwrap()
                    );
                    println!(
                        "Conflicts: {:?}",
                        preview_provider_conflicts(&managed, &instance, &resolved.plans)
                            .await
                            .unwrap()
                    );
                    assert_eq!(project, "mOgUt4GM");
                    let original = std::fs::read(&placeholder).unwrap();
                    let before = crate::instance_mods::scan(&managed, &instance)
                        .unwrap()
                        .entries
                        .into_iter()
                        .find(|e| e.file_name == "placeholder-api-2.8.2+1.21.10.jar")
                        .unwrap();
                    let records = install_provider_plans_reviewed(
                        &managed,
                        &instance,
                        resolved.plans,
                        resolved.preview.inventory_revision.as_deref(),
                    )
                    .await
                    .unwrap();
                    assert_eq!(records.len(), 1);
                    assert_eq!(std::fs::read(&placeholder).unwrap(), original);
                    let restarted = ContentState::load(&managed, &instance).unwrap();
                    let inventory = crate::instance_mods::scan(&managed, &instance).unwrap();
                    let local = inventory
                        .entries
                        .iter()
                        .find(|e| e.file_name == before.file_name)
                        .unwrap();
                    assert_eq!(local.ownership, before.ownership);
                    assert_eq!(local.provenance, before.provenance);
                    assert_eq!(
                        inventory
                            .entries
                            .iter()
                            .filter(|e| e
                                .metadata
                                .as_ref()
                                .is_some_and(|m| m.id == "placeholder-api"))
                            .count(),
                        1
                    );
                    let installed = inventory
                        .entries
                        .iter()
                        .find(|e| e.metadata.as_ref().is_some_and(|m| m.id == "modmenu"))
                        .unwrap();
                    assert_eq!(
                        installed.ownership,
                        crate::instance_mods::ModOwnership::ProviderManaged
                    );
                    remove_provider_graph(&managed, &instance, &restarted, &records[0].identity())
                        .unwrap();
                    assert_eq!(std::fs::read(&placeholder).unwrap(), original);
                    assert!(
                        crate::instance_mods::scan(&managed, &instance)
                            .unwrap()
                            .entries
                            .iter()
                            .all(|e| e.metadata.as_ref().is_none_or(|m| m.id != "modmenu"))
                    );
                    println!(
                        "Mod Menu normal verified install/reload/removal passed; local bytes, ownership, provenance unchanged; one top-level Placeholder API."
                    );
                }
                Err(error) => println!("Project {project}: {}: {error}", error.code()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestRequest, TestResponse, TestServer};
    use sha2::Sha512;
    use std::io::Write as _;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn pack_records_keep_distinct_files_from_one_project() {
        let fixture = Fixture::new();
        let first_path = fixture.dir(ContentType::Mod).join("first.jar");
        let second_path = fixture.dir(ContentType::Mod).join("second.jar");
        std::fs::write(&first_path, b"first file").unwrap();
        std::fs::write(&second_path, b"second file").unwrap();
        let mut first = fixture.record(ContentType::Mod, &first_path);
        first.origin = ProviderOrigin::Pack;
        first.file_id = format!("{:x}", Sha512::digest(b"first file"));
        let mut second = fixture.record(ContentType::Mod, &second_path);
        second.origin = ProviderOrigin::Pack;
        second.file_id = format!("{:x}", Sha512::digest(b"second file"));
        let state = ContentState {
            schema_version: SCHEMA_VERSION,
            entries: vec![first.clone(), second.clone()],
        };
        state.validate().unwrap();
        assert_eq!(state.entries.len(), 2);
        assert_eq!(state.entries[0].identity(), state.entries[1].identity());
        assert_ne!(state.entries[0].file_id, state.entries[1].file_id);
        let restored = ContentState::from_json(&serde_json::to_string(&state).unwrap()).unwrap();
        assert_eq!(restored.entries, state.entries);

        let mut ordinary = state.clone();
        ordinary.entries[1].origin = ProviderOrigin::Direct;
        assert!(ordinary.validate().is_err());
        let mut conflicting = state;
        conflicting.entries[1].file_id = first.file_id;
        assert!(conflicting.validate().is_err());
        std::fs::remove_dir_all(&fixture.root).unwrap();
    }

    #[tokio::test]
    async fn pack_installs_distinct_files_from_one_project() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        let records = install_pack_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![
                mod_plan(&server, "SameProj", "pack-first", false),
                mod_plan(&server, "SameProj", "pack-second", false),
            ],
        )
        .await
        .unwrap();
        assert_eq!(records.len(), 2);
        assert!(
            records
                .iter()
                .all(|record| record.origin == ProviderOrigin::Pack)
        );
        assert!(records.iter().all(|record| record.explicitly_retained));
        assert_ne!(records[0].file_id, records[1].file_id);
        let restored = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(restored.entries, records);
        for record in &records {
            assert!(
                fixture
                    .dir(ContentType::Mod)
                    .join(&record.file_name)
                    .is_file()
            );
        }
    }

    #[tokio::test]
    async fn redundant_exact_pack_file_acquisition_reuses_verified_object() {
        let fixture = Fixture::new();
        let hits = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&hits);
        let bytes = fabric_jar_with_id("same-file");
        let server = TestServer::spawn(Arc::new(move |_: &TestRequest| {
            observed.fetch_add(1, Ordering::SeqCst);
            TestResponse::ok(&bytes)
        }));
        let acquired = acquire_provider_plans(
            &fixture.managed,
            vec![
                mod_plan(&server, "SameProj", "same-file", false),
                mod_plan(&server, "SameProj", "same-file", false),
            ],
        )
        .await
        .unwrap();
        assert_eq!(acquired.len(), 2);
        assert_eq!(acquired[0].1, acquired[1].1);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    struct Fixture {
        root: PathBuf,
        managed: ManagedPaths,
        instance: InstanceId,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join("aurora-content-tests")
                .join(uuid::Uuid::new_v4().to_string());
            std::fs::create_dir_all(root.join("instances")).unwrap();
            let instance =
                InstanceId::new(format!("safe-{}", uuid::Uuid::new_v4().simple())).unwrap();
            std::fs::create_dir(root.join("instances").join(instance.as_str())).unwrap();
            let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
            Self {
                root,
                managed,
                instance,
            }
        }
        fn dir(&self, kind: ContentType) -> PathBuf {
            ensure_directory(&self.managed, &self.instance, kind).unwrap()
        }
        fn zip(&self, kind: ContentType, name: &str, metadata: Option<&str>) -> PathBuf {
            let path = self.dir(kind).join(name);
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            let entry = if kind == ContentType::ShaderPack {
                "shaders/basic.fsh"
            } else {
                "pack.mcmeta"
            };
            writer
                .start_file(entry, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(metadata.unwrap_or("void main(){}").as_bytes())
                .unwrap();
            writer.finish().unwrap();
            path
        }
        fn record(&self, kind: ContentType, path: &Path) -> ProviderRecord {
            let bytes = std::fs::read(path).unwrap();
            ProviderRecord {
                content_type: kind,
                provider: "synthetic".into(),
                project_id: "opaque-project".into(),
                version_id: "opaque-version".into(),
                file_id: "opaque-file".into(),
                file_name: path.file_name().unwrap().to_string_lossy().into_owned(),
                sha256: format!("{:x}", sha2::Sha256::digest(&bytes)),
                display_version: Some("1.2.3".into()),
                compatibility: ContentCompatibility {
                    minecraft_versions: vec!["1.21.11".into()],
                    loader: Some("fabric".into()),
                    environment: Some("client".into()),
                },
                dependencies: vec![ProviderDependency {
                    kind: DependencyKind::Required,
                    provider: "synthetic".into(),
                    project_id: "another-project".into(),
                    version_id: None,
                }],
                explicitly_retained: true,
                requires: vec![],
                origin: ProviderOrigin::Direct,
                installed_at_unix_seconds: None,
                pinned: false,
                update_channel: UpdateChannel::Stable,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn transaction_record(kind: ContentType, name: &str, bytes: &[u8]) -> ProviderRecord {
        ProviderRecord {
            content_type: kind,
            provider: "modrinth".into(),
            project_id: "AAAABBBB".into(),
            version_id: "11112222".into(),
            file_id: format!("{:x}", Sha512::digest(bytes)),
            file_name: name.into(),
            sha256: format!("{:x}", sha2::Sha256::digest(bytes)),
            display_version: Some("1.0.0".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: (kind == ContentType::Mod).then(|| "fabric".into()),
                environment: Some("client_and_server".into()),
            },
            dependencies: vec![],
            explicitly_retained: true,
            requires: vec![],
            origin: ProviderOrigin::Direct,
            installed_at_unix_seconds: None,
            pinned: false,
            update_channel: UpdateChannel::Stable,
        }
    }

    fn fabric_jar_with_id(id: &str) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(
                serde_json::json!({"schemaVersion":1,"id":id,"version":"1.0.0"})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn register_no_aurora(fixture: &Fixture, platform: crate::instances::platform::PlatformPin) {
        use crate::instances::{
            InstanceRecord, InstanceRegistry, InstanceState,
            platform::InstalledConfiguration,
            settings::{InstanceConfiguration, LoaderConfiguration},
        };
        let mut config = InstanceConfiguration::for_minecraft_version("1.21.11");
        config.set_aurora_enabled(false);
        if matches!(
            platform,
            crate::instances::platform::PlatformPin::Vanilla {}
        ) {
            config.set_loader(LoaderConfiguration::Vanilla {});
        }
        let mut registry = InstanceRegistry::empty();
        registry.instances_mut().push(
            InstanceRecord::from_installed(
                fixture.instance.clone(),
                "Provider regression",
                InstanceState::Ready,
                InstalledConfiguration {
                    minecraft_version: "1.21.11".into(),
                    platform,
                    aurora: None,
                },
                config,
            )
            .unwrap(),
        );
        std::fs::create_dir_all(fixture.managed.launcher_dir()).unwrap();
        registry
            .save(&fixture.managed.instance_registry_file())
            .unwrap();
    }

    fn no_aurora_fixture() -> Fixture {
        let fixture = Fixture::new();
        register_no_aurora(
            &fixture,
            crate::instances::platform::PlatformPin::Fabric {
                version: "0.19.5".into(),
            },
        );
        fixture
    }

    fn served_mods() -> TestServer {
        TestServer::spawn(Arc::new(|request: &TestRequest| {
            TestResponse::ok(&fabric_jar_with_id(request.path.trim_start_matches('/')))
        }))
    }

    fn mod_plan(
        server: &TestServer,
        project: &str,
        mod_id: &str,
        dependency: bool,
    ) -> ProviderInstallPlan {
        let bytes = fabric_jar_with_id(mod_id);
        let hash = format!("{:x}", Sha512::digest(&bytes));
        ProviderInstallPlan {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: project.into(),
            version_id: "11112222".into(),
            file_id: hash.clone(),
            file_name: format!("{mod_id}.jar"),
            display_version: Some("1.0.0".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: Some("fabric".into()),
                environment: Some("client_and_server".into()),
            },
            dependencies: if dependency {
                vec![ProviderDependency {
                    kind: DependencyKind::Required,
                    provider: "modrinth".into(),
                    project_id: "P7dR8mSH".into(),
                    version_id: None,
                }]
            } else {
                vec![]
            },
            source: ProviderArtifactSource::Sha512(
                Sha512ArtifactSource::loopback_http_for_testing(
                    &format!("{}/{mod_id}", server.base_url()),
                    &hash,
                    Some(bytes.len() as u64),
                )
                .unwrap(),
            ),
        }
    }

    async fn install_api_parent(fixture: &Fixture, server: &TestServer) -> ContentState {
        install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![
                mod_plan(server, "P7dR8mSH", "fabric-api", false),
                mod_plan(server, "AAAABBBB", "parent-a", true),
            ],
        )
        .await
        .unwrap();
        ContentState::load(&fixture.managed, &fixture.instance).unwrap()
    }

    fn identity(project: &str) -> ProviderIdentity {
        ProviderIdentity {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: project.into(),
        }
    }

    fn document_jar(document: serde_json::Value) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(document.to_string().as_bytes()).unwrap();
        zip.finish().unwrap().into_inner()
    }
    fn fixture_plan(
        server: &TestServer,
        path: &str,
        project: &str,
        bytes: &[u8],
        dependencies: Vec<ProviderDependency>,
    ) -> ProviderInstallPlan {
        let hash = format!("{:x}", Sha512::digest(bytes));
        ProviderInstallPlan {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: project.into(),
            version_id: "11112222".into(),
            file_id: hash.clone(),
            file_name: format!("{path}.jar"),
            display_version: Some("1".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: Some("fabric".into()),
                environment: Some("client_and_server".into()),
            },
            dependencies,
            source: ProviderArtifactSource::Sha512(
                Sha512ArtifactSource::loopback_http_for_testing(
                    &format!("{}/{path}", server.base_url()),
                    &hash,
                    Some(bytes.len() as u64),
                )
                .unwrap(),
            ),
        }
    }
    #[tokio::test]
    async fn local_dependency_satisfaction_install_reload_removal_and_changed_update() {
        let fixture = no_aurora_fixture();
        let dep = document_jar(serde_json::json!({"id":"dependency","version":"2.5"}));
        let parent = document_jar(
            serde_json::json!({"id":"parent","version":"1","depends":{"dependency":">=2 <3"}}),
        );
        let update = document_jar(
            serde_json::json!({"id":"parent","version":"2","depends":{"dependency":">=3"}}),
        );
        let dependency_requests = Arc::new(AtomicUsize::new(0));
        let requests = dependency_requests.clone();
        let served_dep = dep.clone();
        let served_parent = parent.clone();
        let served_update = update.clone();
        let server = TestServer::spawn(Arc::new(move |r: &TestRequest| {
            if r.path == "/dependency" {
                requests.fetch_add(1, Ordering::SeqCst);
                TestResponse::ok(&served_dep)
            } else if r.path == "/update" {
                TestResponse::ok(&served_update)
            } else {
                TestResponse::ok(&served_parent)
            }
        }));
        let local = fixture.dir(ContentType::Mod).join("local-custom-name.jar");
        std::fs::write(&local, &dep).unwrap();
        let mut resolved = crate::modrinth::Resolved {
            preview: crate::modrinth::InstallPreview {
                inventory_revision: None,
                project_id: "AAAABBBB".into(),
                version_id: "11112222".into(),
                content_type: ContentType::Mod,
                items: vec![crate::modrinth::PreviewItem {
                    project_id: "BBBBCCCC".into(),
                    title: "Dependency".into(),
                    version_id: "11112222".into(),
                    version_number: "2.5".into(),
                    file_name: "dependency.jar".into(),
                    already_installed: false,
                    satisfied_by: None,
                    changelog: None,
                }],
                warnings: vec![],
            },
            plans: vec![
                fixture_plan(&server, "dependency", "BBBBCCCC", &dep, vec![]),
                fixture_plan(
                    &server,
                    "parent",
                    "AAAABBBB",
                    &parent,
                    vec![ProviderDependency {
                        kind: DependencyKind::Required,
                        provider: "modrinth".into(),
                        project_id: "BBBBCCCC".into(),
                        version_id: None,
                    }],
                ),
            ],
        };
        reconcile_provider_resolution(&fixture.managed, &fixture.instance, &mut resolved)
            .await
            .unwrap();
        assert_eq!(resolved.plans.len(), 1);
        assert_eq!(dependency_requests.load(Ordering::SeqCst), 0);
        assert_eq!(
            resolved.preview.items[0]
                .satisfied_by
                .as_ref()
                .unwrap()
                .ownership,
            crate::instance_mods::ModOwnership::UserManaged
        );
        let records = install_provider_plans_reviewed(
            &fixture.managed,
            &fixture.instance,
            resolved.plans,
            resolved.preview.inventory_revision.as_deref(),
        )
        .await
        .unwrap();
        assert!(records[0].requires.is_empty());
        let restarted = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        let inventory = crate::instance_mods::scan(&fixture.managed, &fixture.instance).unwrap();
        let local_entry = inventory
            .entries
            .iter()
            .find(|e| e.file_name == "local-custom-name.jar")
            .unwrap();
        assert!(matches!(
            crate::instance_mods::remove(
                &fixture.managed,
                &fixture.instance,
                &local_entry.entry_id
            ),
            Err(crate::instance_mods::ModError::DependencyBlocked(_))
        ));
        assert!(matches!(
            update_provider_graph(
                &fixture.managed,
                &fixture.instance,
                &restarted,
                &records[0].identity(),
                vec![fixture_plan(&server, "update", "AAAABBBB", &update, vec![])]
            )
            .await,
            Err(ContentError::ModCollision(_))
        ));
        assert_eq!(std::fs::read(&local).unwrap(), dep);
        remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &restarted,
            &records[0].identity(),
        )
        .unwrap();
        let inventory = crate::instance_mods::scan(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(inventory.entries.len(), 1);
        assert_eq!(
            inventory.entries[0].ownership,
            crate::instance_mods::ModOwnership::UserManaged
        );
        assert!(inventory.entries[0].provenance.is_none());
        assert_eq!(std::fs::read(&local).unwrap(), dep);
    }
    #[tokio::test]
    async fn reviewed_inventory_revision_excludes_external_mod_changes() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        fixture.dir(ContentType::Mod);
        let revision = local_inventory_revision(&fixture.managed, &fixture.instance).unwrap();
        std::fs::write(
            fixture.dir(ContentType::Mod).join("external.jar"),
            fabric_jar_with_id("external"),
        )
        .unwrap();
        assert!(matches!(
            install_provider_plans_reviewed(
                &fixture.managed,
                &fixture.instance,
                vec![mod_plan(&server, "AAAABBBB", "ordinary", false)],
                Some(&revision)
            )
            .await,
            Err(ContentError::ChangedSinceScan)
        ));
        assert!(!fixture.dir(ContentType::Mod).join("ordinary.jar").exists());
    }

    #[tokio::test]
    async fn no_aurora_direct_api_and_ordinary_mod_activate_with_provider_ownership() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        for (project, id) in [("P7dR8mSH", "fabric-api"), ("AAAABBBB", "ordinary")] {
            let records = install_provider_plans(
                &fixture.managed,
                &fixture.instance,
                vec![mod_plan(&server, project, id, false)],
            )
            .await
            .unwrap();
            assert!(records[0].explicitly_retained);
            assert!(records[0].requires.is_empty());
        }
        let inventory = crate::instance_mods::scan(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(inventory.entries.len(), 2);
        assert!(inventory.entries.iter().all(|entry| entry.ownership == crate::instance_mods::ModOwnership::ProviderManaged));
        assert!(
            crate::instance_mods::verified_required_mods(&fixture.managed, &fixture.instance)
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn no_aurora_api_dependency_graph_survives_reload_and_shared_parent_removal() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        let first = install_api_parent(&fixture, &server).await;
        assert!(
            !first
                .find(&identity("P7dR8mSH"))
                .unwrap()
                .explicitly_retained
        );
        assert_eq!(
            first.find(&identity("AAAABBBB")).unwrap().requires,
            vec![identity("P7dR8mSH")]
        );
        install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![mod_plan(&server, "BBBBCCCC", "parent-b", true)],
        )
        .await
        .unwrap();
        let state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(state.required_by(&identity("P7dR8mSH")).len(), 2);
        assert!(matches!(
            removal_state(&state, &identity("P7dR8mSH")),
            Err(ContentError::RequiredByInstalledContent)
        ));
        let after_a = removal_state(&state, &identity("AAAABBBB")).unwrap();
        assert!(after_a.find(&identity("P7dR8mSH")).is_some());
        remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &state,
            &identity("AAAABBBB"),
        )
        .unwrap();
        assert!(
            fixture
                .dir(ContentType::Mod)
                .join("fabric-api.jar")
                .exists()
        );
        let after_b = removal_state(&after_a, &identity("BBBBCCCC")).unwrap();
        assert!(after_b.entries.is_empty());
        // Preview determines the orphan set; activation checks exact old bytes.
        remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &after_a,
            &identity("BBBBCCCC"),
        )
        .unwrap();
        assert!(
            !fixture
                .dir(ContentType::Mod)
                .join("fabric-api.jar")
                .exists()
        );
    }

    #[tokio::test]
    async fn no_aurora_explicit_api_promotion_reuses_bytes_and_retains_relationships() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        let before = install_api_parent(&fixture, &server).await;
        let path = fixture.dir(ContentType::Mod).join("fabric-api.jar");
        let bytes = std::fs::read(&path).unwrap();
        // Promotion is the existing direct-selection path for an already installed identity.
        drop(server);
        let promoted =
            retain_provider(&fixture.managed, &fixture.instance, &identity("P7dR8mSH")).unwrap();
        assert!(promoted.explicitly_retained);
        let state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(
            state.find(&identity("AAAABBBB")).unwrap().requires,
            before.find(&identity("AAAABBBB")).unwrap().requires
        );
        let next = remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &state,
            &identity("AAAABBBB"),
        )
        .unwrap();
        assert_eq!(next.entries, vec![promoted]);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[tokio::test]
    async fn disabled_provider_removal_uses_verified_disabled_bytes_and_preserves_graph_rules() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        let state = install_api_parent(&fixture, &server).await;
        let inventory = crate::instance_mods::scan(&fixture.managed, &fixture.instance).unwrap();
        let parent = inventory
            .entries
            .iter()
            .find(|entry| entry.file_name == "parent-a.jar")
            .unwrap();
        crate::instance_mods::set_enabled(
            &fixture.managed,
            &fixture.instance,
            &parent.entry_id,
            false,
        )
        .unwrap();
        let record = state.find(&identity("AAAABBBB")).unwrap();
        assert!(
            validate_provider_file(&fixture.managed, &fixture.instance, record)
                .unwrap()
                .ends_with("parent-a.jar.disabled")
        );
        let next = remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &state,
            &identity("AAAABBBB"),
        )
        .unwrap();
        assert!(next.entries.is_empty());
        assert!(
            !fixture
                .dir(ContentType::Mod)
                .join("parent-a.jar.disabled")
                .exists()
        );
    }

    #[tokio::test]
    async fn no_aurora_api_update_and_rollback_keep_verified_provider_state() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        let before = install_api_parent(&fixture, &server).await;
        let mut plan = mod_plan(&server, "AAAABBBB", "parent-a-v2", true);
        plan.version_id = "22223333".into();
        let preview = update_preview_state(&before, &identity("AAAABBBB"), &[plan]).unwrap();
        assert!(preview.find(&identity("P7dR8mSH")).is_some());
        let mut plan = mod_plan(&server, "AAAABBBB", "parent-a-v2", true);
        plan.version_id = "22223333".into();
        let acquired = acquire_provider_plans(&fixture.managed, vec![plan])
            .await
            .unwrap();
        let next = updated_state(
            &before,
            &identity("AAAABBBB"),
            acquired
                .iter()
                .map(|(record, _, _)| record.clone())
                .collect(),
        )
        .unwrap();
        assert!(
            apply_lifecycle_state_with_hooks(
                &fixture.managed,
                &fixture.instance,
                &before,
                &next,
                &acquired,
                |_| Err(ContentError::Io(std::io::Error::other(
                    "injected persistence failure"
                ))),
                || Ok(())
            )
            .is_err()
        );
        assert_eq!(
            ContentState::load(&fixture.managed, &fixture.instance).unwrap(),
            before
        );
        assert!(fixture.dir(ContentType::Mod).join("parent-a.jar").exists());
        assert!(
            !fixture
                .dir(ContentType::Mod)
                .join("parent-a-v2.jar")
                .exists()
        );
        apply_lifecycle_state(
            &fixture.managed,
            &fixture.instance,
            &before,
            &next,
            &acquired,
        )
        .unwrap();
        assert_eq!(
            ContentState::load(&fixture.managed, &fixture.instance).unwrap(),
            next
        );
        assert_eq!(
            next.find(&identity("AAAABBBB")).unwrap().requires,
            vec![identity("P7dR8mSH")]
        );
    }

    #[tokio::test]
    async fn no_aurora_api_collision_and_tampering_remain_fail_closed() {
        for directory_target in [false, true] {
            let fixture = no_aurora_fixture();
            let path = fixture.dir(ContentType::Mod).join("fabric-api.jar");
            if directory_target {
                std::fs::create_dir(&path).unwrap();
            } else {
                std::fs::write(&path, b"manual content").unwrap();
            }
            let server = served_mods();
            assert!(matches!(
                install_provider_plans(
                    &fixture.managed,
                    &fixture.instance,
                    vec![mod_plan(&server, "P7dR8mSH", "fabric-api", false)]
                )
                .await,
                Err(ContentError::Collision)
            ));
            assert!(
                ContentState::load(&fixture.managed, &fixture.instance)
                    .unwrap()
                    .entries
                    .is_empty()
            );
            if !directory_target {
                assert_eq!(std::fs::read(path).unwrap(), b"manual content");
            }
        }
        let fixture = no_aurora_fixture();
        let server = served_mods();
        let state = install_api_parent(&fixture, &server).await;
        let path = fixture.dir(ContentType::Mod).join("fabric-api.jar");
        std::fs::write(&path, b"tampered provider bytes").unwrap();
        assert!(
            remove_provider_graph(
                &fixture.managed,
                &fixture.instance,
                &state,
                &identity("AAAABBBB")
            )
            .is_err()
        );
        assert_eq!(
            ContentState::load(&fixture.managed, &fixture.instance).unwrap(),
            state
        );
        assert_eq!(std::fs::read(path).unwrap(), b"tampered provider bytes");
        assert!(fixture.dir(ContentType::Mod).join("parent-a.jar").exists());
    }

    fn install_protected_fixture(fixture: &Fixture) -> PathBuf {
        install_protected_api(fixture, &fabric_jar_with_id("fabric-api"))
    }

    fn bundled_jar(id: &str, module: &str) -> Vec<u8> {
        bundled_jar_version(id, module, "1.0.0")
    }
    fn fabric_jar_version(id: &str, version: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(
                serde_json::json!({"schemaVersion":1,"id":id,"version":version})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        writer.finish().unwrap().into_inner()
    }
    fn bundled_jar_version(id: &str, module: &str, version: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(serde_json::json!({"id":id,"version":"1.0","jars":[{"file":"META-INF/jars/module.jar"}]}).to_string().as_bytes()).unwrap();
        writer
            .start_file(
                "META-INF/jars/module.jar",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer
            .write_all(&fabric_jar_version(module, version))
            .unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn shared_nested_modules_do_not_fabricate_protected_top_level_ownership() {
        let fixture = Fixture::new();
        let required = bundled_jar("fabric-api", "shared-api-module");
        let protected_path = install_protected_api(&fixture, &required);
        let incoming = bundled_jar("ordinary-renderer", "shared-api-module");
        let source = fixture.root.join("verified-renderer.jar");
        std::fs::write(&source, &incoming).unwrap();
        let record = transaction_record(ContentType::Mod, "renderer.jar", &incoming);
        // The former rule rejected this exact nested/nested intersection.
        let (metadata, _) =
            crate::instance_mods::inspect_fabric_metadata(&source, incoming.len() as u64);
        assert_eq!(metadata.unwrap().nested_mod_ids, ["shared-api-module"]);
        activate_provider_transaction(
            &fixture.managed,
            &fixture.instance,
            vec![(record.clone(), source, incoming.len() as u64)],
            |state| state.save(&fixture.managed, &fixture.instance),
        )
        .unwrap();
        assert_eq!(std::fs::read(protected_path).unwrap(), required);
        let saved = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(saved.entries.len(), 1);
        assert_eq!(saved.entries[0].identity(), record.identity());
        assert_eq!(saved.entries[0].file_name, record.file_name);
        assert_eq!(saved.entries[0].sha256, record.sha256);
        assert_eq!(saved.entries[0].origin, ProviderOrigin::Direct);
        // Transactions stamp a receipt; the fixture record predates it.
        assert!(saved.entries[0].installed_at_unix_seconds.is_some());
        let inventory = crate::instance_mods::scan(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(
            inventory
                .entries
                .iter()
                .filter(|entry| entry.ownership
                    == crate::instance_mods::ModOwnership::LauncherManagedRequired)
                .count(),
            2
        );
        assert_eq!(
            inventory
                .entries
                .iter()
                .filter(
                    |entry| entry.ownership == crate::instance_mods::ModOwnership::ProviderManaged
                )
                .count(),
            1
        );
    }

    #[test]
    fn root_collisions_and_incompatible_bundled_versions_remain_blocked() {
        for (installed, incoming) in [
            (
                fabric_jar_with_id("ordinary"),
                fabric_jar_with_id("ordinary"),
            ),
            (
                bundled_jar_version("parent", "module", "2.0"),
                fabric_jar_with_id("module"),
            ),
            (
                fabric_jar_with_id("module"),
                bundled_jar_version("parent", "module", "2.0"),
            ),
        ] {
            let fixture = no_aurora_fixture();
            let local = fixture.dir(ContentType::Mod).join("local.jar");
            std::fs::write(&local, &installed).unwrap();
            let source = fixture.root.join("verified.jar");
            std::fs::write(&source, &incoming).unwrap();
            let record = transaction_record(ContentType::Mod, "different-name.jar", &incoming);
            let error = validate_provider_mod_artifact(
                &fixture.managed,
                &fixture.instance,
                &record,
                &source,
                incoming.len() as u64,
            )
            .unwrap_err();
            assert!(
                matches!(error,ContentError::ModCollision(ProviderConflict {file_name,ownership:crate::instance_mods::ModOwnership::UserManaged,..}) if file_name=="local.jar")
            );
            assert_eq!(std::fs::read(local).unwrap(), installed);
        }
    }

    #[test]
    fn incoming_nested_root_of_protected_artifact_is_still_blocked() {
        let fixture = Fixture::new();
        install_protected_fixture(&fixture);
        let bytes = bundled_jar_version("ordinary", "aurora", "2.0");
        let path = fixture.root.join("verified.jar");
        std::fs::write(&path, &bytes).unwrap();
        let record = transaction_record(ContentType::Mod, "ordinary.jar", &bytes);
        assert!(matches!(
            validate_provider_mod_artifact(
                &fixture.managed,
                &fixture.instance,
                &record,
                &path,
                bytes.len() as u64
            ),
            Err(ContentError::ModCollision(ProviderConflict {
                ownership: crate::instance_mods::ModOwnership::LauncherManagedRequired,
                ..
            }))
        ));
    }

    #[tokio::test]
    async fn review_reports_structured_local_collision_without_installing() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        let local = fixture.dir(ContentType::Mod).join("manual.jar");
        std::fs::write(&local, fabric_jar_with_id("ordinary")).unwrap();
        let conflicts = preview_provider_conflicts(
            &fixture.managed,
            &fixture.instance,
            &[mod_plan(&server, "AAAABBBB", "ordinary", false)],
        )
        .await
        .unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].file_name, "manual.jar");
        assert_eq!(conflicts[0].mod_id.as_deref(), Some("ordinary"));
        assert_eq!(
            conflicts[0].ownership,
            crate::instance_mods::ModOwnership::UserManaged
        );
        assert_eq!(
            std::fs::read_dir(fixture.dir(ContentType::Mod))
                .unwrap()
                .count(),
            1
        );
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );
    }

    fn install_protected_api(fixture: &Fixture, api: &[u8]) -> PathBuf {
        let directory = fixture.dir(ContentType::Mod);
        let aurora = fabric_jar_with_id("aurora");
        std::fs::write(directory.join("aurora-1.jar"), &aurora).unwrap();
        let api_path = directory.join("fabric-api-1.jar");
        std::fs::write(&api_path, &api).unwrap();
        let state = serde_json::json!({"schemaVersion":1,"auroraVersion":"1","channel":"stable","minecraftVersion":"1.21.11","fabricLoaderVersion":"0.19.5","installationId":"fixture","installedAtUnixSeconds":1,"artifact":{"relativePath":"mods/aurora-1.jar","sizeBytes":aurora.len(),"sha256":format!("{:x}", sha2::Sha256::digest(&aurora))},"fabricApi":{"version":"1","artifact":{"relativePath":"mods/fabric-api-1.jar","sizeBytes":api.len(),"sha256":format!("{:x}", sha2::Sha256::digest(&api))}}});
        std::fs::write(
            fixture
                .managed
                .instance_paths(&fixture.instance)
                .root()
                .join("aurora-installed.json"),
            state.to_string(),
        )
        .unwrap();
        api_path
    }

    #[tokio::test]
    async fn active_managed_mod_identity_is_protected_even_under_a_different_filename() {
        let fixture = Fixture::new();
        let path = install_protected_fixture(&fixture);
        let bytes = std::fs::read(&path).unwrap();
        let server = served_mods();
        for id in ["fabric-api", "aurora"] {
            let mut plan = mod_plan(&server, "P7dR8mSH", id, false);
            plan.file_name = format!("alternate-{id}.jar");
            assert!(matches!(
                install_provider_plans(&fixture.managed, &fixture.instance, vec![plan]).await,
                Err(ContentError::ModCollision(ProviderConflict {
                    ownership: crate::instance_mods::ModOwnership::LauncherManagedRequired,
                    ..
                }))
            ));
        }
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );
        let inventory = crate::instance_mods::scan(&fixture.managed, &fixture.instance).unwrap();
        assert!(inventory.entries.iter().all(|entry| entry.ownership
            == crate::instance_mods::ModOwnership::LauncherManagedRequired
            && !entry.can_remove));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let parent = install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![mod_plan(&server, "BBBBCCCC", "parent", true)],
        )
        .await
        .unwrap();
        // Descriptive dependency metadata survives, but an external launcher
        // requirement is never forged into a provider-owned graph node.
        assert_eq!(parent[0].dependencies[0].project_id, "P7dR8mSH");
        assert!(parent[0].requires.is_empty());
        let state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &state,
            &identity("BBBBCCCC"),
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        // Even a forged provider ownership claim cannot remove the protected destination.
        let mut state = ContentState::empty();
        state.entries.push(transaction_record(
            ContentType::Mod,
            "fabric-api-1.jar",
            &bytes,
        ));
        state.save(&fixture.managed, &fixture.instance).unwrap();
        assert!(
            crate::instance_mods::verified_required_mods(&fixture.managed, &fixture.instance)
                .is_err()
        );
        assert!(matches!(
            remove_provider_graph(
                &fixture.managed,
                &fixture.instance,
                &state,
                &identity("AAAABBBB")
            ),
            Err(ContentError::Collision)
        ));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn protected_requirement_satisfaction_rejects_tampering_missing_and_ambiguous_state() {
        for failure in ["tampered", "missing", "directory", "unconfigured"] {
            let fixture = Fixture::new();
            let path = install_protected_fixture(&fixture);
            assert_eq!(
                crate::instance_mods::verified_required_mods(&fixture.managed, &fixture.instance)
                    .unwrap()
                    .len(),
                2
            );
            match failure {
                "tampered" => std::fs::write(path, b"tampered").unwrap(),
                "missing" => std::fs::remove_file(path).unwrap(),
                "directory" => {
                    std::fs::remove_file(&path).unwrap();
                    std::fs::create_dir(path).unwrap();
                }
                _ => register_no_aurora(
                    &fixture,
                    crate::instances::platform::PlatformPin::Fabric {
                        version: "0.19.5".into(),
                    },
                ),
            }
            assert!(
                crate::instance_mods::verified_required_mods(&fixture.managed, &fixture.instance)
                    .is_err(),
                "{failure}"
            );
        }
    }

    #[tokio::test]
    async fn vanilla_cannot_activate_fabric_api_from_a_fabric_provider_plan() {
        let fixture = Fixture::new();
        register_no_aurora(
            &fixture,
            crate::instances::platform::PlatformPin::Vanilla {},
        );
        let server = served_mods();
        assert!(matches!(
            install_provider_plans(
                &fixture.managed,
                &fixture.instance,
                vec![mod_plan(&server, "P7dR8mSH", "fabric-api", false)]
            )
            .await,
            Err(ContentError::UnsupportedAction)
        ));
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );
    }

    #[test]
    fn transaction_rolls_back_after_second_file_and_state_failure() {
        let fixture = Fixture::new();
        let bytes = b"verified content";
        let source = fixture.root.join("verified-source");
        std::fs::write(&source, bytes).unwrap();
        let existing = fixture.dir(ContentType::ResourcePack).join("manual.zip");
        std::fs::write(&existing, b"user content").unwrap();
        let first = transaction_record(ContentType::ResourcePack, "first.zip", bytes);
        let second = transaction_record(ContentType::ShaderPack, "second.zip", bytes);
        let result = activate_provider_transaction(
            &fixture.managed,
            &fixture.instance,
            vec![
                (first.clone(), source.clone(), bytes.len() as u64),
                (
                    second.clone(),
                    fixture.root.join("missing"),
                    bytes.len() as u64,
                ),
            ],
            |state| state.save(&fixture.managed, &fixture.instance),
        );
        assert!(result.is_err());
        assert!(
            !fixture
                .dir(ContentType::ResourcePack)
                .join("first.zip")
                .exists()
        );
        assert!(
            !fixture
                .dir(ContentType::ShaderPack)
                .join("second.zip")
                .exists()
        );
        assert_eq!(std::fs::read(&existing).unwrap(), b"user content");
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );

        let result = activate_provider_transaction(
            &fixture.managed,
            &fixture.instance,
            vec![(first, source, bytes.len() as u64)],
            |_| {
                Err(ContentError::StateMalformed(
                    "injected state failure".into(),
                ))
            },
        );
        assert!(result.is_err());
        assert!(
            !fixture
                .dir(ContentType::ResourcePack)
                .join("first.zip")
                .exists()
        );
        assert_eq!(std::fs::read(existing).unwrap(), b"user content");
    }

    #[test]
    fn transaction_rejects_collision_before_activating_any_file() {
        let fixture = Fixture::new();
        let bytes = b"verified content";
        let source = fixture.root.join("verified-source");
        std::fs::write(&source, bytes).unwrap();
        let existing = fixture.dir(ContentType::ShaderPack).join("taken.zip");
        std::fs::write(&existing, b"user content").unwrap();
        let result = activate_provider_transaction(
            &fixture.managed,
            &fixture.instance,
            vec![
                (
                    transaction_record(ContentType::ResourcePack, "first.zip", bytes),
                    source.clone(),
                    bytes.len() as u64,
                ),
                (
                    transaction_record(ContentType::ShaderPack, "taken.zip", bytes),
                    source,
                    bytes.len() as u64,
                ),
            ],
            |state| state.save(&fixture.managed, &fixture.instance),
        );
        assert!(matches!(result, Err(ContentError::Collision)));
        assert!(
            !fixture
                .dir(ContentType::ResourcePack)
                .join("first.zip")
                .exists()
        );
        assert_eq!(std::fs::read(existing).unwrap(), b"user content");
    }

    #[tokio::test]
    async fn sha512_plans_acquire_concurrently_and_persist_provider_ownership() {
        let fixture = Fixture::new();
        let served_bytes = Arc::new([
            fabric_jar_with_id("fixture-mod"),
            fabric_jar_with_id("fixture-resource"),
            fabric_jar_with_id("fixture-shader"),
        ]);
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(AtomicUsize::new(0));
        let handler_active = active.clone();
        let handler_peak = peak.clone();
        let handler_requests = requests.clone();
        let handler_bytes = Arc::clone(&served_bytes);
        let server = TestServer::spawn(Arc::new(move |_request: &TestRequest| {
            handler_requests.fetch_add(1, Ordering::SeqCst);
            let now = handler_active.fetch_add(1, Ordering::SeqCst) + 1;
            handler_peak.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(35));
            handler_active.fetch_sub(1, Ordering::SeqCst);
            let index: usize = _request.path.trim_start_matches("/file/").parse().unwrap();
            TestResponse::ok(&handler_bytes[index])
        }));
        let cases = [
            (ContentType::Mod, "fixture.jar"),
            (ContentType::ResourcePack, "fixture.zip"),
            (ContentType::ShaderPack, "shader.zip"),
        ];
        let plans: Vec<_> = cases
            .iter()
            .enumerate()
            .map(|(index, (kind, name))| {
                let bytes = &served_bytes[index];
                let sha512 = format!("{:x}", Sha512::digest(bytes));
                let mut record = transaction_record(*kind, name, &bytes);
                record.project_id = format!("project-{index}");
                ProviderInstallPlan {
                    content_type: *kind,
                    provider: record.provider,
                    project_id: record.project_id,
                    version_id: record.version_id,
                    file_id: record.file_id,
                    file_name: record.file_name,
                    display_version: record.display_version,
                    compatibility: record.compatibility,
                    dependencies: vec![],
                    source: ProviderArtifactSource::Sha512(
                        Sha512ArtifactSource::loopback_http_for_testing(
                            &format!("{}/file/{index}", server.base_url()),
                            &sha512,
                            Some(bytes.len() as u64),
                        )
                        .unwrap(),
                    ),
                }
            })
            .collect();
        let records = install_provider_plans(&fixture.managed, &fixture.instance, plans)
            .await
            .unwrap();
        assert_eq!(records.len(), 3);
        assert!(peak.load(Ordering::SeqCst) > 1);
        assert_eq!(requests.load(Ordering::SeqCst), 3);
        assert_eq!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .len(),
            3
        );
        for (kind, name) in cases {
            if kind == ContentType::Mod {
                let inventory =
                    crate::instance_mods::scan(&fixture.managed, &fixture.instance).unwrap();
                assert_eq!(inventory.entries[0].file_name, name);
                assert_eq!(
                    inventory.entries[0].ownership,
                    crate::instance_mods::ModOwnership::ProviderManaged
                );
            } else {
                let inventory = scan(&fixture.managed, &fixture.instance, kind).unwrap();
                assert_eq!(inventory.entries[0].file_name, name);
                assert_eq!(
                    inventory.entries[0].ownership,
                    ContentOwnership::ProviderManaged
                );
            }
        }
        let count = requests.load(Ordering::SeqCst);
        let bytes = &served_bytes[0];
        let sha512 = format!("{:x}", Sha512::digest(bytes));
        let source = Sha512ArtifactSource::loopback_http_for_testing(
            &format!("{}/file/0", server.base_url()),
            &sha512,
            Some(bytes.len() as u64),
        )
        .unwrap();
        let cached = ArtifactCache::new(fixture.managed.clone())
            .acquire_sha512(&source)
            .await
            .unwrap();
        assert_eq!(cached.origin, crate::cache::ArtifactOrigin::CacheHit);
        assert_eq!(requests.load(Ordering::SeqCst), count);
        std::fs::write(
            fixture.dir(ContentType::ResourcePack).join("fixture.zip"),
            b"tampered",
        )
        .unwrap();
        assert_eq!(
            scan(
                &fixture.managed,
                &fixture.instance,
                ContentType::ResourcePack
            )
            .unwrap()
            .entries[0]
                .ownership,
            ContentOwnership::Unknown
        );
    }

    #[tokio::test]
    async fn wrong_published_sha512_never_activates_content() {
        let fixture = Fixture::new();
        let server = TestServer::spawn(Arc::new(|_| TestResponse::ok(b"actual bytes")));
        let record = transaction_record(ContentType::ResourcePack, "bad.zip", b"expected bytes");
        let plan = ProviderInstallPlan {
            content_type: record.content_type,
            provider: record.provider,
            project_id: record.project_id,
            version_id: record.version_id,
            file_id: record.file_id.clone(),
            file_name: record.file_name,
            display_version: record.display_version,
            compatibility: record.compatibility,
            dependencies: vec![],
            source: ProviderArtifactSource::Sha512(
                Sha512ArtifactSource::loopback_http_for_testing(
                    &format!("{}/bad", server.base_url()),
                    &record.file_id,
                    Some(12),
                )
                .unwrap(),
            ),
        };
        assert!(
            install_provider_plans(&fixture.managed, &fixture.instance, vec![plan])
                .await
                .is_err()
        );
        assert!(
            !fixture
                .dir(ContentType::ResourcePack)
                .join("bad.zip")
                .exists()
        );
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );
    }

    #[test]
    fn content_directories_are_derived_from_validated_ids_and_closed_types() {
        let fixture = Fixture::new();
        for (kind, name) in [
            (ContentType::Mod, "mods"),
            (ContentType::ResourcePack, "resourcepacks"),
            (ContentType::ShaderPack, "shaderpacks"),
        ] {
            let path = validate_directory(&fixture.managed, &fixture.instance, kind).unwrap();
            assert_eq!(
                path,
                fixture
                    .root
                    .join("instances")
                    .join(fixture.instance.as_str())
                    .join(name)
            );
        }
        assert!(InstanceId::new("../outside").is_err());
        assert!(validate_file_name("../bad.zip").is_err());
    }

    #[test]
    fn manual_packs_and_malformed_archives_are_visible_without_provider_ownership() {
        let fixture = Fixture::new();
        fixture.zip(
            ContentType::ResourcePack,
            "textures.zip",
            Some(r#"{"pack":{"pack_format":64,"description":"Textures"}}"#),
        );
        fixture.zip(ContentType::ShaderPack, "shaders.zip", None);
        std::fs::create_dir(fixture.dir(ContentType::ResourcePack).join("folder pack")).unwrap();
        std::fs::write(
            fixture.dir(ContentType::ShaderPack).join("bad.zip"),
            b"not a ZIP",
        )
        .unwrap();
        let resource = scan(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
        )
        .unwrap();
        assert_eq!(resource.entries.len(), 2);
        assert!(
            resource
                .entries
                .iter()
                .all(|entry| entry.ownership == ContentOwnership::UserManaged)
        );
        assert!(
            resource
                .entries
                .iter()
                .any(|entry| entry.description.as_deref() == Some("Textures"))
        );
        assert!(
            !resource
                .entries
                .iter()
                .find(|entry| entry.file_type == "directory")
                .unwrap()
                .management
                .can_remove
        );
        let shader = scan(&fixture.managed, &fixture.instance, ContentType::ShaderPack).unwrap();
        assert_eq!(shader.entries.len(), 2);
        assert!(shader.entries.iter().any(|entry| {
            entry
                .warnings
                .iter()
                .any(|warning| warning.code == "archive_malformed")
        }));
    }

    #[test]
    fn provider_evidence_is_versioned_and_tamper_or_missing_is_reported() {
        let fixture = Fixture::new();
        let path = fixture.zip(
            ContentType::ResourcePack,
            "managed.zip",
            Some(r#"{"pack":{"pack_format":64}}"#),
        );
        let record = fixture.record(ContentType::ResourcePack, &path);
        let mut state = ContentState::empty();
        state.entries.push(record.clone());
        state.save(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries,
            vec![record.clone()]
        );
        let before = scan(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
        )
        .unwrap();
        assert_eq!(
            before.entries[0].ownership,
            ContentOwnership::ProviderManaged
        );
        assert_eq!(
            before.entries[0].provenance.as_ref().unwrap().project_id,
            "opaque-project"
        );
        std::fs::write(&path, b"tampered").unwrap();
        let tampered = scan(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
        )
        .unwrap();
        assert_eq!(tampered.entries[0].ownership, ContentOwnership::Unknown);
        assert!(!tampered.entries[0].management.can_remove);
        assert!(
            tampered.entries[0]
                .warnings
                .iter()
                .any(|warning| warning.code == "content_hash_mismatch")
        );
        std::fs::remove_file(path).unwrap();
        let missing = scan(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
        )
        .unwrap();
        assert_eq!(missing.missing_managed, vec![record]);
    }

    #[test]
    fn malformed_future_and_duplicate_state_never_get_overwritten() {
        let fixture = Fixture::new();
        let state_file = fixture
            .root
            .join("instances")
            .join(fixture.instance.as_str())
            .join("content-managed.json");
        std::fs::write(&state_file, "broken").unwrap();
        assert!(matches!(
            ContentState::load(&fixture.managed, &fixture.instance),
            Err(ContentError::StateMalformed(_))
        ));
        assert!(
            ContentState::empty()
                .save(&fixture.managed, &fixture.instance)
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&state_file).unwrap(), "broken");
        std::fs::write(&state_file, r#"{"schemaVersion":99,"entries":[]}"#).unwrap();
        assert!(matches!(
            ContentState::load(&fixture.managed, &fixture.instance),
            Err(ContentError::StateVersion(99))
        ));
        std::fs::remove_file(state_file).unwrap();
        let path = fixture.zip(ContentType::ShaderPack, "duplicate.zip", None);
        let record = fixture.record(ContentType::ShaderPack, &path);
        let mut state = ContentState::empty();
        state.entries = vec![record.clone(), record];
        assert!(state.save(&fixture.managed, &fixture.instance).is_err());
    }

    #[test]
    fn activation_refuses_collisions_hash_mismatches_and_state_failures() {
        let fixture = Fixture::new();
        let source = fixture.root.join("verified.zip");
        std::fs::write(&source, b"verified bytes").unwrap();
        let mut record = fixture.record(ContentType::ResourcePack, &source);
        record.file_name = "collision.zip".into();
        let target = fixture
            .dir(ContentType::ResourcePack)
            .join(&record.file_name);
        std::fs::write(&target, b"user bytes").unwrap();
        assert!(matches!(
            activate_verified(
                &fixture.managed,
                &fixture.instance,
                record.clone(),
                &source,
                None
            ),
            Err(ContentError::Collision)
        ));
        assert_eq!(std::fs::read(&target).unwrap(), b"user bytes");
        std::fs::remove_file(&target).unwrap();
        let mut wrong = record.clone();
        wrong.sha256 = "0".repeat(64);
        assert!(matches!(
            activate_verified(&fixture.managed, &fixture.instance, wrong, &source, None),
            Err(ContentError::HashMismatch)
        ));
        assert!(!target.exists());
        let state_file = fixture
            .root
            .join("instances")
            .join(fixture.instance.as_str())
            .join("content-managed.json");
        std::fs::write(&state_file, "broken").unwrap();
        assert!(
            activate_verified(&fixture.managed, &fixture.instance, record, &source, None).is_err()
        );
        assert!(!target.exists());
    }

    #[test]
    fn activation_rolls_back_when_state_commit_fails() {
        let fixture = Fixture::new();
        let source = fixture.root.join("verified.zip");
        std::fs::write(&source, b"verified bytes").unwrap();
        let mut record = fixture.record(ContentType::ShaderPack, &source);
        record.file_name = "rollback.zip".into();
        let target = fixture.dir(ContentType::ShaderPack).join(&record.file_name);
        let result = activate_verified_with_commit(
            &fixture.managed,
            &fixture.instance,
            record,
            &source,
            None,
            |_| {
                Err(ContentError::StateMalformed(
                    "injected commit failure".into(),
                ))
            },
        );
        assert!(
            matches!(result, Err(ContentError::StateMalformed(_))),
            "{result:?}"
        );
        assert!(!target.exists());
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );
    }

    #[test]
    fn provider_remove_updates_state_and_stale_entry_is_refused() {
        let fixture = Fixture::new();
        let path = fixture.zip(
            ContentType::ResourcePack,
            "managed.zip",
            Some(r#"{"pack":{"pack_format":64}}"#),
        );
        let mut state = ContentState::empty();
        state
            .entries
            .push(fixture.record(ContentType::ResourcePack, &path));
        state.save(&fixture.managed, &fixture.instance).unwrap();
        let entry = scan(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
        )
        .unwrap()
        .entries
        .remove(0);
        assert!(matches!(
            remove(
                &fixture.managed,
                &fixture.instance,
                ContentType::ResourcePack,
                "arbitrary-path"
            ),
            Err(ContentError::ChangedSinceScan)
        ));
        assert!(matches!(
            remove(
                &fixture.managed,
                &fixture.instance,
                ContentType::ResourcePack,
                &entry.entry_id,
            ),
            Err(ContentError::UnsupportedAction) | Err(ContentError::UnsupportedActionWith(_))
        ));
        let current = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &current,
            &current.entries[0].identity(),
        )
        .unwrap();
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );
    }

    // ── Pack management capabilities, activation and coherent removal ──

    fn options_file(fixture: &Fixture) -> PathBuf {
        fixture
            .managed
            .instance_paths(&fixture.instance)
            .root()
            .join("options.txt")
    }
    fn write_options(fixture: &Fixture, text: &str) {
        std::fs::write(options_file(fixture), text).unwrap();
    }
    fn read_options(fixture: &Fixture) -> String {
        std::fs::read_to_string(options_file(fixture)).unwrap()
    }
    fn save_record(fixture: &Fixture, record: ProviderRecord) {
        let mut state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        state.entries.push(record);
        state.save(&fixture.managed, &fixture.instance).unwrap();
    }
    fn pack_entry(
        fixture: &Fixture,
        kind: ContentType,
        file_name: &str,
    ) -> crate::instance_content::ContentEntry {
        scan(&fixture.managed, &fixture.instance, kind)
            .unwrap()
            .entries
            .into_iter()
            .find(|entry| entry.file_name == file_name)
            .unwrap()
    }

    #[test]
    fn pack_capabilities_report_activation_and_removal_paths() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ResourcePack, "local.zip", None);
        let managed_path = fixture.zip(ContentType::ResourcePack, "managed.zip", None);
        let record = fixture.record(ContentType::ResourcePack, &managed_path);
        save_record(&fixture, record);
        std::fs::create_dir(fixture.dir(ContentType::ResourcePack).join("Folder Pack")).unwrap();
        write_options(
            &fixture,
            "resourcePacks:[\"file/managed.zip\",\"file/Folder Pack\",\"vanilla\"]\n",
        );
        let inventory = scan(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
        )
        .unwrap()
        .entries;
        let local = inventory
            .iter()
            .find(|entry| entry.file_name == "local.zip")
            .unwrap();
        assert!(local.management.can_remove);
        assert_eq!(local.management.removal_path, RemovalPath::LocalFile);
        assert!(local.management.can_toggle);
        assert_eq!(local.management.active, Some(false));
        assert!(!local.management.activation_managed_in_game);
        let managed = inventory
            .iter()
            .find(|entry| entry.file_name == "managed.zip")
            .unwrap();
        assert!(managed.management.can_remove);
        assert_eq!(managed.management.removal_path, RemovalPath::ProviderGraph);
        assert!(managed.management.can_toggle);
        assert_eq!(managed.management.active, Some(true));
        let folder = inventory
            .iter()
            .find(|entry| entry.file_name == "Folder Pack")
            .unwrap();
        assert!(!folder.management.can_remove);
        assert_eq!(folder.management.removal_path, RemovalPath::Blocked);
        assert!(folder.management.removal_blocked_reason.is_some());
        assert!(folder.management.can_toggle);
        assert_eq!(folder.management.active, Some(true));
    }

    #[test]
    fn shader_capabilities_stay_truthful_about_in_game_activation() {
        let fixture = Fixture::new();
        let managed_path = fixture.zip(ContentType::ShaderPack, "shader.zip", None);
        save_record(
            &fixture,
            fixture.record(ContentType::ShaderPack, &managed_path),
        );
        fixture.zip(ContentType::ShaderPack, "unmanaged.zip", None);
        let inventory = scan(&fixture.managed, &fixture.instance, ContentType::ShaderPack)
            .unwrap()
            .entries;
        for entry in &inventory {
            assert!(!entry.management.can_toggle);
            assert!(entry.management.activation_managed_in_game);
            assert_eq!(entry.management.active, None);
            assert!(entry.management.toggle_blocked_reason.is_none());
            assert!(entry.management.can_remove);
            assert_eq!(
                if entry.file_name == "shader.zip" {
                    RemovalPath::ProviderGraph
                } else {
                    RemovalPath::LocalFile
                },
                entry.management.removal_path
            );
        }
    }

    #[test]
    fn set_pack_enabled_round_trips_and_survives_reload() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ResourcePack, "faithful.zip", None);
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "faithful.zip");
        assert_eq!(entry.management.active, Some(false));
        let inventory = set_pack_enabled(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &entry.entry_id,
            true,
        )
        .unwrap();
        assert_eq!(inventory.entries[0].management.active, Some(true));
        assert_eq!(
            read_options(&fixture),
            "resourcePacks:[\"file/faithful.zip\",\"vanilla\"]\n"
        );
        // A fresh scan re-reads options.txt: disabled/enabled state persists
        // across inventory reloads (restarts) without launcher-side caching.
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "faithful.zip");
        assert_eq!(entry.management.active, Some(true));
        set_pack_enabled(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &entry.entry_id,
            false,
        )
        .unwrap();
        assert_eq!(read_options(&fixture), "resourcePacks:[\"vanilla\"]\n");
        assert_eq!(
            pack_entry(&fixture, ContentType::ResourcePack, "faithful.zip")
                .management
                .active,
            Some(false)
        );
        set_pack_enabled(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &entry.entry_id,
            true,
        )
        .unwrap();
        assert_eq!(
            pack_entry(&fixture, ContentType::ResourcePack, "faithful.zip")
                .management
                .active,
            Some(true)
        );
    }

    #[test]
    fn set_pack_enabled_preserves_order_and_unrelated_options() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ResourcePack, "ours.zip", None);
        write_options(
            &fixture,
            "volume:0.7\nresourcePacks:[\"file/other.zip\",\"vanilla\"]\nfov:70.0\n",
        );
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "ours.zip");
        set_pack_enabled(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &entry.entry_id,
            true,
        )
        .unwrap();
        assert_eq!(
            read_options(&fixture),
            "volume:0.7\nresourcePacks:[\"file/ours.zip\",\"file/other.zip\",\"vanilla\"]\nfov:70.0\n"
        );
        set_pack_enabled(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &entry.entry_id,
            false,
        )
        .unwrap();
        assert_eq!(
            read_options(&fixture),
            "volume:0.7\nresourcePacks:[\"file/other.zip\",\"vanilla\"]\nfov:70.0\n"
        );
    }

    #[test]
    fn malformed_options_disable_activation_safely() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ResourcePack, "pack.zip", None);
        write_options(&fixture, "resourcePacks:[broken\n");
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "pack.zip");
        assert!(!entry.management.can_toggle);
        assert_eq!(entry.management.active, None);
        assert!(entry.management.toggle_blocked_reason.is_some());
        assert!(matches!(
            set_pack_enabled(
                &fixture.managed,
                &fixture.instance,
                ContentType::ResourcePack,
                &entry.entry_id,
                true,
            ),
            Err(ContentError::UnsupportedActionWith(_))
        ));
        assert_eq!(read_options(&fixture), "resourcePacks:[broken\n");
    }

    #[test]
    fn removing_an_active_local_pack_updates_activation_coherently() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ResourcePack, "active.zip", None);
        fixture.zip(ContentType::ResourcePack, "kept.zip", None);
        write_options(
            &fixture,
            "volume:0.7\nresourcePacks:[\"file/kept.zip\",\"file/active.zip\",\"vanilla\"]\n",
        );
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "active.zip");
        assert_eq!(entry.management.active, Some(true));
        let inventory = remove(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &entry.entry_id,
        )
        .unwrap();
        assert!(
            inventory
                .entries
                .iter()
                .all(|entry| entry.file_name != "active.zip")
        );
        assert!(
            !fixture
                .dir(ContentType::ResourcePack)
                .join("active.zip")
                .exists()
        );
        assert_eq!(
            read_options(&fixture),
            "volume:0.7\nresourcePacks:[\"file/kept.zip\",\"vanilla\"]\n"
        );
    }

    #[test]
    fn removing_an_inactive_local_pack_leaves_options_untouched() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ResourcePack, "inactive.zip", None);
        write_options(&fixture, "resourcePacks:[\"file/other.zip\",\"vanilla\"]\n");
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "inactive.zip");
        remove(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &entry.entry_id,
        )
        .unwrap();
        assert_eq!(
            read_options(&fixture),
            "resourcePacks:[\"file/other.zip\",\"vanilla\"]\n"
        );
    }

    #[test]
    fn removal_never_touches_an_unreadable_options_document() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ResourcePack, "active.zip", None);
        write_options(
            &fixture,
            "resourcePacks:[\"file/active.zip\",\"vanilla\"]\n",
        );
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "active.zip");
        assert_eq!(entry.management.active, Some(true));
        // Corrupt the document after the scan: Aurora cannot prove the pack
        // is still referenced, so the file removal proceeds without
        // activation reconciliation and the malformed document is preserved
        // byte-for-byte (Minecraft drops references to missing files).
        write_options(&fixture, "resourcePacks:[broken\n");
        remove(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &entry.entry_id,
        )
        .unwrap();
        assert!(
            !fixture
                .dir(ContentType::ResourcePack)
                .join("active.zip")
                .exists()
        );
        assert_eq!(read_options(&fixture), "resourcePacks:[broken\n");
    }

    #[test]
    fn provider_removal_of_an_enabled_pack_clears_its_reference() {
        let fixture = Fixture::new();
        let path = fixture.zip(ContentType::ResourcePack, "managed.zip", None);
        let record = fixture.record(ContentType::ResourcePack, &path);
        save_record(&fixture, record);
        write_options(
            &fixture,
            "volume:0.7\nresourcePacks:[\"file/managed.zip\",\"file/user.zip\",\"vanilla\"]\n",
        );
        let current = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &current,
            &current.entries[0].identity(),
        )
        .unwrap();
        assert!(
            !fixture
                .dir(ContentType::ResourcePack)
                .join("managed.zip")
                .exists()
        );
        assert_eq!(
            read_options(&fixture),
            "volume:0.7\nresourcePacks:[\"file/user.zip\",\"vanilla\"]\n"
        );
    }

    #[test]
    fn shader_provider_removal_preserves_unrelated_files() {
        let fixture = Fixture::new();
        let path = fixture.zip(ContentType::ShaderPack, "managed.zip", None);
        save_record(&fixture, fixture.record(ContentType::ShaderPack, &path));
        fixture.zip(ContentType::ShaderPack, "user.zip", None);
        let current = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &current,
            &current.entries[0].identity(),
        )
        .unwrap();
        let directory = fixture.dir(ContentType::ShaderPack);
        assert!(!directory.join("managed.zip").exists());
        assert!(directory.join("user.zip").exists());
    }

    #[test]
    fn recovered_packs_receive_normal_capabilities_and_keep_provenance() {
        let fixture = Fixture::new();
        let path = fixture.zip(ContentType::ResourcePack, "recovered.zip", None);
        let mut record = fixture.record(ContentType::ResourcePack, &path);
        record.origin = ProviderOrigin::Recovered;
        save_record(&fixture, record);
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "recovered.zip");
        assert_eq!(entry.management.removal_path, RemovalPath::ProviderGraph);
        assert!(entry.management.can_toggle);
        assert_eq!(entry.management.active, Some(false));
        assert_eq!(
            entry.provenance.as_ref().unwrap().origin,
            ProviderOrigin::Recovered
        );
        set_pack_enabled(
            &fixture.managed,
            &fixture.instance,
            ContentType::ResourcePack,
            &entry.entry_id,
            true,
        )
        .unwrap();
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "recovered.zip");
        assert_eq!(entry.management.active, Some(true));
        assert_eq!(
            entry.provenance.as_ref().unwrap().origin,
            ProviderOrigin::Recovered
        );
        let shader_path = fixture.zip(ContentType::ShaderPack, "recovered-shader.zip", None);
        let mut shader = fixture.record(ContentType::ShaderPack, &shader_path);
        shader.origin = ProviderOrigin::Recovered;
        save_record(&fixture, shader);
        let shader_entry = pack_entry(&fixture, ContentType::ShaderPack, "recovered-shader.zip");
        assert_eq!(
            shader_entry.management.removal_path,
            RemovalPath::ProviderGraph
        );
        assert!(shader_entry.management.activation_managed_in_game);
        assert_eq!(
            shader_entry.provenance.as_ref().unwrap().origin,
            ProviderOrigin::Recovered
        );
    }

    #[test]
    fn unmanaged_local_packs_never_gain_provider_capabilities() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ResourcePack, "local.zip", None);
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "local.zip");
        assert_eq!(entry.management.removal_path, RemovalPath::LocalFile);
        assert!(entry.provenance.is_none());
        assert!(
            !fixture
                .managed
                .instance_paths(&fixture.instance)
                .root()
                .join("content-managed.json")
                .exists()
        );
        // Provider-only operations key on project identity, which a local
        // file does not have: there is no record to resolve.
        assert!(
            ContentState::load(&fixture.managed, &fixture.instance)
                .unwrap()
                .entries
                .is_empty()
        );
    }

    #[test]
    fn stale_and_wrong_identity_mutations_are_refused() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ResourcePack, "pack.zip", None);
        let other = Fixture::new();
        other.zip(ContentType::ResourcePack, "other.zip", None);
        let other_entry = pack_entry(&other, ContentType::ResourcePack, "other.zip");
        assert!(matches!(
            set_pack_enabled(
                &fixture.managed,
                &fixture.instance,
                ContentType::ResourcePack,
                &other_entry.entry_id,
                true,
            ),
            Err(ContentError::ChangedSinceScan)
        ));
        assert!(matches!(
            remove(
                &fixture.managed,
                &fixture.instance,
                ContentType::ResourcePack,
                &other_entry.entry_id,
            ),
            Err(ContentError::ChangedSinceScan)
        ));
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "pack.zip");
        std::fs::remove_file(fixture.dir(ContentType::ResourcePack).join("pack.zip")).unwrap();
        assert!(matches!(
            set_pack_enabled(
                &fixture.managed,
                &fixture.instance,
                ContentType::ResourcePack,
                &entry.entry_id,
                true,
            ),
            Err(ContentError::ChangedSinceScan)
        ));
    }

    #[test]
    fn shader_activation_is_refused_as_in_game_managed() {
        let fixture = Fixture::new();
        fixture.zip(ContentType::ShaderPack, "shader.zip", None);
        let entry = pack_entry(&fixture, ContentType::ShaderPack, "shader.zip");
        assert!(matches!(
            set_pack_enabled(
                &fixture.managed,
                &fixture.instance,
                ContentType::ShaderPack,
                &entry.entry_id,
                true,
            ),
            Err(ContentError::UnsupportedActionWith(_))
        ));
        assert!(matches!(
            set_pack_enabled(
                &fixture.managed,
                &fixture.instance,
                ContentType::Mod,
                &entry.entry_id,
                true,
            ),
            Err(ContentError::UnsupportedActionWith(_))
        ));
    }

    #[test]
    fn dependency_blockers_still_guard_managed_pack_removal() {
        let fixture = Fixture::new();
        let parent_path = fixture.zip(ContentType::ResourcePack, "parent.zip", None);
        let child_path = fixture.zip(ContentType::ResourcePack, "child.zip", None);
        let mut parent = fixture.record(ContentType::ResourcePack, &parent_path);
        let mut child = fixture.record(ContentType::ResourcePack, &child_path);
        child.explicitly_retained = false;
        child.project_id = "child-project".into();
        parent.requires = vec![child.identity()];
        let child_identity = child.identity();
        let mut state = ContentState::empty();
        state.entries.push(parent);
        state.entries.push(child);
        state.save(&fixture.managed, &fixture.instance).unwrap();
        // The saved document is the deterministic expected state (saving
        // orders entries); never a hand-arranged in-memory order.
        let state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        assert!(matches!(
            remove_provider_graph(&fixture.managed, &fixture.instance, &state, &child_identity,),
            Err(ContentError::RequiredByInstalledContent)
        ));
        assert!(
            fixture
                .dir(ContentType::ResourcePack)
                .join("child.zip")
                .exists()
        );
        // The explicitly retained parent still removes cleanly, taking its
        // dependency along through the ordinary graph rules.
        let parent_identity = state
            .entries
            .iter()
            .find(|record| record.file_name == "parent.zip")
            .unwrap()
            .identity();
        remove_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &state,
            &parent_identity,
        )
        .unwrap();
        assert!(
            !fixture
                .dir(ContentType::ResourcePack)
                .join("parent.zip")
                .exists()
        );
    }

    fn pack_plan(
        server: &TestServer,
        path: &str,
        project: &str,
        kind: ContentType,
        file_name: &str,
    ) -> ProviderInstallPlan {
        let entry = if kind == ContentType::ShaderPack {
            "shaders/basic.fsh"
        } else {
            "pack.mcmeta"
        };
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file(entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"void main(){}").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let hash = format!("{:x}", Sha512::digest(&bytes));
        ProviderInstallPlan {
            content_type: kind,
            provider: "modrinth".into(),
            project_id: project.into(),
            version_id: "11112222".into(),
            file_id: hash.clone(),
            file_name: file_name.into(),
            display_version: Some("1.0.0".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: None,
                environment: Some("client".into()),
            },
            dependencies: vec![],
            source: ProviderArtifactSource::Sha512(
                Sha512ArtifactSource::loopback_http_for_testing(
                    &format!("{}/{path}", server.base_url()),
                    &hash,
                    Some(bytes.len() as u64),
                )
                .unwrap(),
            ),
        }
    }

    #[tokio::test]
    async fn provider_update_renames_migrate_the_enabled_reference() {
        let fixture = Fixture::new();
        let server = TestServer::spawn(Arc::new(|_request: &TestRequest| {
            let cursor = std::io::Cursor::new(Vec::new());
            let mut writer = zip::ZipWriter::new(cursor);
            writer
                .start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"void main(){}").unwrap();
            TestResponse::ok(&writer.finish().unwrap().into_inner())
        }));
        install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![pack_plan(
                &server,
                "old",
                "AAAABBBB",
                ContentType::ResourcePack,
                "Pack 1.0.zip",
            )],
        )
        .await
        .unwrap();
        write_options(
            &fixture,
            "volume:0.7\nresourcePacks:[\"file/Pack 1.0.zip\",\"file/user.zip\",\"vanilla\"]\n",
        );
        let state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        update_provider_graph(
            &fixture.managed,
            &fixture.instance,
            &state,
            &state.entries[0].identity(),
            vec![pack_plan(
                &server,
                "new",
                "AAAABBBB",
                ContentType::ResourcePack,
                "Pack 2.0.zip",
            )],
        )
        .await
        .unwrap();
        let directory = fixture.dir(ContentType::ResourcePack);
        assert!(!directory.join("Pack 1.0.zip").exists());
        assert!(directory.join("Pack 2.0.zip").exists());
        assert_eq!(
            read_options(&fixture),
            "volume:0.7\nresourcePacks:[\"file/Pack 2.0.zip\",\"file/user.zip\",\"vanilla\"]\n"
        );
        let entry = pack_entry(&fixture, ContentType::ResourcePack, "Pack 2.0.zip");
        assert_eq!(entry.management.active, Some(true));
        assert!(entry.management.can_toggle);
    }

    #[test]
    fn unsafe_directories_are_rejected_for_every_domain() {
        let fixture = Fixture::new();
        for kind in [
            ContentType::Mod,
            ContentType::ResourcePack,
            ContentType::ShaderPack,
        ] {
            let path = fixture
                .root
                .join("instances")
                .join(fixture.instance.as_str())
                .join(kind.directory_name());
            std::fs::write(&path, b"not a directory").unwrap();
            assert!(matches!(
                validate_directory(&fixture.managed, &fixture.instance, kind),
                Err(ContentError::UnsafePath)
            ));
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn substantial_resource_and_shader_inventories_stay_shallow() {
        let fixture = Fixture::new();
        for kind in [ContentType::ResourcePack, ContentType::ShaderPack] {
            let dir = fixture.dir(kind);
            for index in 0..120 {
                std::fs::write(
                    dir.join(format!("pack-{index:03}.zip")),
                    b"malformed but visible",
                )
                .unwrap();
            }
            let entries = scan(&fixture.managed, &fixture.instance, kind)
                .unwrap()
                .entries;
            assert_eq!(entries.len(), 120);
            assert!(
                entries
                    .iter()
                    .all(|entry| entry.ownership == ContentOwnership::UserManaged)
            );
        }
    }

    #[test]
    fn links_are_visible_but_not_followed_or_removed_when_supported() {
        let fixture = Fixture::new();
        let outside = fixture.root.join("outside.zip");
        std::fs::write(&outside, b"external user bytes").unwrap();
        for kind in [
            ContentType::Mod,
            ContentType::ResourcePack,
            ContentType::ShaderPack,
        ] {
            let dir = fixture.dir(kind);
            let link = dir.join("link.zip");
            #[cfg(windows)]
            let result = std::os::windows::fs::symlink_file(&outside, &link);
            #[cfg(unix)]
            let result = std::os::unix::fs::symlink(&outside, &link);
            if result.is_err() {
                continue;
            } // Windows installations without symlink privilege.
            let inventory = scan(&fixture.managed, &fixture.instance, kind).unwrap();
            assert_eq!(inventory.entries[0].file_type, "link");
            assert!(!inventory.entries[0].management.can_remove);
            assert!(
                remove(
                    &fixture.managed,
                    &fixture.instance,
                    kind,
                    &inventory.entries[0].entry_id
                )
                .is_err()
            );
            assert_eq!(std::fs::read(&outside).unwrap(), b"external user bytes");
        }
    }

    #[test]
    fn v1_migration_retains_all_historical_content_and_rejects_bad_graphs() {
        let fixture = Fixture::new();
        let mut state = ContentState::empty();
        for (kind, project, name) in [
            (ContentType::Mod, "root", "root.jar"),
            (ContentType::Mod, "old-dependency", "dependency.jar"),
            (ContentType::ResourcePack, "pack", "pack.zip"),
            (ContentType::ShaderPack, "shader", "shader.zip"),
        ] {
            let mut record = transaction_record(kind, name, b"bytes");
            record.project_id = project.into();
            state.entries.push(record);
        }
        let mut legacy = serde_json::to_value(&state).unwrap();
        legacy["schemaVersion"] = serde_json::json!(1);
        for entry in legacy["entries"].as_array_mut().unwrap() {
            entry.as_object_mut().unwrap().remove("explicitlyRetained");
            entry.as_object_mut().unwrap().remove("requires");
            entry.as_object_mut().unwrap().remove("origin");
            entry
                .as_object_mut()
                .unwrap()
                .remove("installedAtUnixSeconds");
            entry.as_object_mut().unwrap().remove("pinned");
            entry.as_object_mut().unwrap().remove("updateChannel");
        }
        let path = state_path(&fixture.managed, &fixture.instance).unwrap();
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let migrated = ContentState::load_and_migrate(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(migrated.entries.len(), 4);
        assert!(
            migrated
                .entries
                .iter()
                .all(|record| record.explicitly_retained && record.requires.is_empty())
        );
        // Schema-1 records had no recorded provenance beyond "launcher
        // installed this"; migration claims direct origin only and never
        // fabricates an installation receipt.
        assert!(
            migrated
                .entries
                .iter()
                .all(|record| record.origin == ProviderOrigin::Direct
                    && record.installed_at_unix_seconds.is_none())
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap()["schemaVersion"],
            4
        );
        // Phase H policy migration: legacy records are unpinned and stable;
        // no historical channel preference is fabricated.
        assert!(
            migrated
                .entries
                .iter()
                .all(|record| !record.pinned && record.update_channel == UpdateChannel::Stable)
        );
        legacy["entries"][0]["provider"] = serde_json::json!(null);
        assert!(matches!(
            ContentState::from_json(&legacy.to_string()),
            Err(ContentError::StateMalformed(_))
        ));
        let mut broken = migrated.clone();
        broken.entries[0].requires.push(ProviderIdentity {
            content_type: ContentType::Mod,
            provider: "missing".into(),
            project_id: "missing".into(),
        });
        assert!(matches!(
            broken.validate(),
            Err(ContentError::StateMalformed(_))
        ));
        assert!(matches!(
            ContentState::from_json(r#"{"schemaVersion":99,"entries":[]}"#),
            Err(ContentError::StateVersion(99))
        ));
    }

    #[test]
    fn v2_migration_maps_retention_to_origin_without_fabricating_receipts() {
        let fixture = Fixture::new();
        let mut state = ContentState::empty();
        let mut direct = transaction_record(ContentType::Mod, "direct.jar", b"direct");
        direct.project_id = "direct".into();
        let mut dependency = transaction_record(ContentType::Mod, "dependency.jar", b"dep");
        dependency.project_id = "dependency".into();
        dependency.explicitly_retained = false;
        state.entries.push(direct);
        state.entries.push(dependency);
        let mut legacy = serde_json::to_value(&state).unwrap();
        legacy["schemaVersion"] = serde_json::json!(2);
        for entry in legacy["entries"].as_array_mut().unwrap() {
            entry.as_object_mut().unwrap().remove("origin");
            entry
                .as_object_mut()
                .unwrap()
                .remove("installedAtUnixSeconds");
            entry.as_object_mut().unwrap().remove("pinned");
            entry.as_object_mut().unwrap().remove("updateChannel");
        }
        let path = state_path(&fixture.managed, &fixture.instance).unwrap();
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let migrated = ContentState::load_and_migrate(&fixture.managed, &fixture.instance).unwrap();
        let direct = migrated
            .entries
            .iter()
            .find(|record| record.project_id == "direct")
            .unwrap();
        let dependency = migrated
            .entries
            .iter()
            .find(|record| record.project_id == "dependency")
            .unwrap();
        assert_eq!(direct.origin, ProviderOrigin::Direct);
        assert_eq!(dependency.origin, ProviderOrigin::Dependency);
        // Historical installation times were never recorded.
        assert!(
            migrated
                .entries
                .iter()
                .all(|record| record.installed_at_unix_seconds.is_none())
        );
        // Provider identities, hashes, and dependency relationships survive.
        assert_eq!(migrated.entries[0].sha256, state.entries[0].sha256);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap()["schemaVersion"],
            4
        );
        // A v2 document that already carries v3 fields is malformed, not a
        // migration candidate.
        let mut smuggled = legacy.clone();
        smuggled["entries"][0]["origin"] = serde_json::json!("recovered");
        assert!(matches!(
            ContentState::from_json(&smuggled.to_string()),
            Err(ContentError::StateMalformed(_))
        ));
    }

    #[test]
    fn v3_round_trip_and_strict_validation() {
        let mut state = ContentState::empty();
        let mut record = transaction_record(ContentType::Mod, "recovered.jar", b"recovered");
        record.origin = ProviderOrigin::Recovered;
        record.installed_at_unix_seconds = Some(1_700_000_000);
        state.entries.push(record);
        let text = serde_json::to_string(&state).unwrap();
        let parsed = ContentState::from_json(&text).unwrap();
        assert_eq!(parsed, state);
        assert_eq!(parsed.entries[0].origin, ProviderOrigin::Recovered);
        assert_eq!(
            parsed.entries[0].installed_at_unix_seconds,
            Some(1_700_000_000)
        );
        // deny_unknown_fields keeps unknown future fields from being silently
        // dropped on load.
        let mut smuggled = serde_json::to_value(&state).unwrap();
        smuggled["entries"][0]["pinState"] = serde_json::json!(true);
        assert!(matches!(
            ContentState::from_json(&smuggled.to_string()),
            Err(ContentError::StateMalformed(_))
        ));
    }

    #[tokio::test]
    async fn transactions_assign_origin_and_receipt_timestamps() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        let root_plan = mod_plan(&server, "AAAABBBB", "root", false);
        let dependency_plan = mod_plan(&server, "BBBBCCCC", "nested", true);
        let records = install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![dependency_plan, root_plan],
        )
        .await
        .unwrap();
        let root = records
            .iter()
            .find(|record| record.project_id == "AAAABBBB")
            .unwrap();
        let dependency = records
            .iter()
            .find(|record| record.project_id == "BBBBCCCC")
            .unwrap();
        assert_eq!(root.origin, ProviderOrigin::Direct);
        assert_eq!(dependency.origin, ProviderOrigin::Dependency);
        assert!(root.installed_at_unix_seconds.is_some());
        assert!(dependency.installed_at_unix_seconds.is_some());

        // Promoting an exact dependency to direct retention updates the
        // origin to the user's direct choice and keeps the first receipt.
        let promoted =
            retain_provider(&fixture.managed, &fixture.instance, &dependency.identity()).unwrap();
        assert_eq!(promoted.origin, ProviderOrigin::Direct);
        assert_eq!(
            promoted.installed_at_unix_seconds,
            dependency.installed_at_unix_seconds
        );
    }

    #[test]
    fn shared_dependencies_and_direct_promotion_control_orphan_cleanup() {
        let mut state = ContentState::empty();
        let mut a = transaction_record(ContentType::ResourcePack, "a.zip", b"a");
        a.project_id = "A".into();
        let mut b = transaction_record(ContentType::ResourcePack, "b.zip", b"b");
        b.project_id = "B".into();
        b.explicitly_retained = false;
        let mut c = transaction_record(ContentType::ResourcePack, "c.zip", b"c");
        c.project_id = "C".into();
        c.explicitly_retained = false;
        let mut d = transaction_record(ContentType::ResourcePack, "d.zip", b"d");
        d.project_id = "D".into();
        a.requires.push(b.identity());
        b.requires.push(c.identity());
        d.requires.push(c.identity());
        state.entries = vec![a.clone(), b.clone(), c.clone(), d.clone()];
        state.validate().unwrap();
        assert!(matches!(
            removal_state(&state, &b.identity()),
            Err(ContentError::RequiredByInstalledContent)
        ));
        let after_a = removal_state(&state, &a.identity()).unwrap();
        assert!(after_a.find(&a.identity()).is_none());
        assert!(after_a.find(&b.identity()).is_none());
        assert!(after_a.find(&c.identity()).is_some());
        let mut promoted = state.clone();
        promoted.find(&b.identity()).unwrap();
        promoted
            .entries
            .iter_mut()
            .find(|record| record.identity() == b.identity())
            .unwrap()
            .explicitly_retained = true;
        let after_promotion = removal_state(&promoted, &a.identity()).unwrap();
        assert!(after_promotion.find(&b.identity()).is_some());
        assert!(after_promotion.find(&c.identity()).is_some());
        let after_d = removal_state(&after_promotion, &d.identity()).unwrap();
        assert!(after_d.find(&c.identity()).is_some());
        let after_b = removal_state(&after_d, &b.identity()).unwrap();
        assert!(after_b.entries.is_empty());
    }

    #[test]
    fn lifecycle_replaces_same_and_changed_names_without_losing_old_bytes_on_failure() {
        let fixture = Fixture::new();
        let dir = fixture.dir(ContentType::ResourcePack);
        let mut old_bytes = b"old working pack".to_vec();
        let mut old = transaction_record(ContentType::ResourcePack, "pack.zip", &old_bytes);
        old.project_id = "PACK".into();
        std::fs::write(dir.join(&old.file_name), &old_bytes).unwrap();
        let mut current = ContentState::empty();
        current.entries.push(old.clone());
        current.save(&fixture.managed, &fixture.instance).unwrap();
        for name in ["pack.zip", "pack-v2.zip"] {
            let new_bytes = format!("new bytes for {name}").into_bytes();
            let source = fixture
                .root
                .join(format!("source-{}", uuid::Uuid::new_v4()));
            std::fs::write(&source, &new_bytes).unwrap();
            let mut new = transaction_record(ContentType::ResourcePack, name, &new_bytes);
            new.project_id = "PACK".into();
            new.version_id = format!("version-{name}");
            let next = updated_state(&current, &old.identity(), vec![new.clone()]).unwrap();
            let wrong_source = fixture.root.join("wrong-source");
            std::fs::write(&wrong_source, b"tampered").unwrap();
            assert!(matches!(
                apply_lifecycle_state(
                    &fixture.managed,
                    &fixture.instance,
                    &current,
                    &next,
                    &[(new.clone(), wrong_source, new_bytes.len() as u64)]
                ),
                Err(ContentError::HashMismatch)
            ));
            assert_eq!(std::fs::read(dir.join(&old.file_name)).unwrap(), old_bytes);
            assert_eq!(
                ContentState::load(&fixture.managed, &fixture.instance).unwrap(),
                current
            );
            apply_lifecycle_state(
                &fixture.managed,
                &fixture.instance,
                &current,
                &next,
                &[(new.clone(), source, new_bytes.len() as u64)],
            )
            .unwrap();
            assert_eq!(std::fs::read(dir.join(name)).unwrap(), new_bytes);
            if name != old.file_name {
                assert!(!dir.join(&old.file_name).exists());
            }
            current = next;
            old = new;
            old_bytes = new_bytes;
        }
    }

    #[test]
    fn lifecycle_restores_old_content_after_state_or_cleanup_failure() {
        let fixture = Fixture::new();
        let directory = fixture.dir(ContentType::ResourcePack);
        let old = transaction_record(ContentType::ResourcePack, "old.zip", b"old verified bytes");
        std::fs::write(directory.join("old.zip"), b"old verified bytes").unwrap();
        let mut current = ContentState::empty();
        current.entries.push(old.clone());
        current.save(&fixture.managed, &fixture.instance).unwrap();
        let mut new =
            transaction_record(ContentType::ResourcePack, "new.zip", b"new verified bytes");
        new.version_id = "22223333".into();
        let source = fixture.root.join("verified-new");
        std::fs::write(&source, b"new verified bytes").unwrap();
        let next = updated_state(&current, &old.identity(), vec![new.clone()]).unwrap();
        let artifact = [(new, source, b"new verified bytes".len() as u64)];
        for cleanup_failure in [false, true] {
            let result = apply_lifecycle_state_with_hooks(
                &fixture.managed,
                &fixture.instance,
                &current,
                &next,
                &artifact,
                |state| {
                    if cleanup_failure {
                        state.save(&fixture.managed, &fixture.instance)
                    } else {
                        Err(ContentError::StateMalformed(
                            "injected state failure".into(),
                        ))
                    }
                },
                || {
                    if cleanup_failure {
                        Err(ContentError::StateMalformed(
                            "injected cleanup failure".into(),
                        ))
                    } else {
                        Ok(())
                    }
                },
            );
            assert!(result.is_err());
            assert_eq!(
                std::fs::read(directory.join("old.zip")).unwrap(),
                b"old verified bytes"
            );
            assert!(!directory.join("new.zip").exists());
            assert_eq!(
                ContentState::load(&fixture.managed, &fixture.instance).unwrap(),
                current
            );
        }
    }

    // ---- Phase H: policy schema, multi-root planning, provenance, failure ----

    fn policy_record(
        project: &str,
        name: &str,
        bytes: &[u8],
        retained: bool,
        origin: ProviderOrigin,
    ) -> ProviderRecord {
        let mut record = transaction_record(ContentType::Mod, name, bytes);
        record.project_id = project.into();
        record.explicitly_retained = retained;
        record.origin = origin;
        record
    }

    #[test]
    fn v3_migration_moves_to_schema_four_with_conservative_policy() {
        let fixture = Fixture::new();
        let record = transaction_record(ContentType::Mod, "legacy.jar", b"legacy");
        let mut legacy = serde_json::to_value(ContentState {
            schema_version: 3,
            entries: vec![record.clone()],
        })
        .unwrap();
        legacy["schemaVersion"] = serde_json::json!(3);
        for entry in legacy["entries"].as_array_mut().unwrap() {
            entry.as_object_mut().unwrap().remove("pinned");
            entry.as_object_mut().unwrap().remove("updateChannel");
        }
        let path = state_path(&fixture.managed, &fixture.instance).unwrap();
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let migrated = ContentState::load_and_migrate(&fixture.managed, &fixture.instance).unwrap();
        assert!(!migrated.entries[0].pinned);
        assert_eq!(migrated.entries[0].update_channel, UpdateChannel::Stable);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap()["schemaVersion"],
            4
        );
        // A v3 document already carrying v4 fields is malformed, never a
        // migration candidate, and is never overwritten.
        legacy["entries"][0]["pinned"] = serde_json::json!(true);
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert!(matches!(
            ContentState::load(&fixture.managed, &fixture.instance),
            Err(ContentError::StateMalformed(_))
        ));
    }

    #[test]
    fn update_policy_persists_and_validates() {
        let fixture = no_aurora_fixture();
        let path = fixture.dir(ContentType::Mod).join("managed.jar");
        std::fs::write(&path, fabric_jar_with_id("managed")).unwrap();
        let record = {
            let mut record = fixture.record(ContentType::Mod, &path);
            record.provider = "modrinth".into();
            record.project_id = "AAAABBBB".into();
            record
        };
        let mut state = ContentState::empty();
        state.entries.push(record.clone());
        state.save(&fixture.managed, &fixture.instance).unwrap();
        let identity = record.identity();

        let pinned = set_update_policy(
            &fixture.managed,
            &fixture.instance,
            &identity,
            Some(true),
            None,
        )
        .unwrap();
        assert!(pinned.pinned);
        assert_eq!(pinned.update_channel, UpdateChannel::Stable);
        let widened = set_update_policy(
            &fixture.managed,
            &fixture.instance,
            &identity,
            None,
            Some(UpdateChannel::Beta),
        )
        .unwrap();
        assert!(widened.pinned);
        assert_eq!(widened.update_channel, UpdateChannel::Beta);
        let reloaded = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        assert!(reloaded.entries[0].pinned);
        assert_eq!(reloaded.entries[0].update_channel, UpdateChannel::Beta);
        // An empty policy change is refused; unknown records are stale.
        assert!(matches!(
            set_update_policy(&fixture.managed, &fixture.instance, &identity, None, None),
            Err(ContentError::StateMalformed(_))
        ));
        let unknown = ProviderIdentity {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: "CCCCDDDD".into(),
        };
        assert!(matches!(
            set_update_policy(
                &fixture.managed,
                &fixture.instance,
                &unknown,
                Some(true),
                None
            ),
            Err(ContentError::ChangedSinceScan)
        ));
        // An invalid channel value never parses.
        let mut smuggled = serde_json::to_value(&reloaded).unwrap();
        smuggled["entries"][0]["updateChannel"] = serde_json::json!("nightly");
        assert!(matches!(
            ContentState::from_json(&smuggled.to_string()),
            Err(ContentError::StateMalformed(_))
        ));
    }

    #[test]
    fn updated_state_multi_preserves_root_provenance_and_policy() {
        let root_old = policy_record(
            "AAAA0001",
            "root.jar",
            b"root-old",
            true,
            ProviderOrigin::Recovered,
        );
        let mut dep_old = policy_record(
            "BBBB0002",
            "dep.jar",
            b"dep-old",
            false,
            ProviderOrigin::Dependency,
        );
        dep_old.version_id = "dep-v1".into();
        let mut current = ContentState::empty();
        current.entries = vec![root_old.clone(), dep_old.clone()];
        current.entries[0].requires = vec![dep_old.identity()];
        current.entries[0].pinned = false;
        current.entries[0].update_channel = UpdateChannel::Beta;
        current.entries[0].installed_at_unix_seconds = None;
        current.validate().unwrap();

        let mut root_new = policy_record(
            "AAAA0001",
            "root-v2.jar",
            b"root-new",
            true,
            ProviderOrigin::Direct,
        );
        root_new.version_id = "root-v2".into();
        root_new.dependencies = vec![ProviderDependency {
            kind: DependencyKind::Required,
            provider: "modrinth".into(),
            project_id: "BBBB0002".into(),
            version_id: None,
        }];
        let mut dep_new = policy_record(
            "BBBB0002",
            "dep-v2.jar",
            b"dep-new",
            false,
            ProviderOrigin::Direct,
        );
        dep_new.version_id = "dep-v2".into();
        let next = updated_state_multi(
            &current,
            &[root_old.identity()],
            vec![dep_new.clone(), root_new.clone()],
        )
        .unwrap();
        let updated_root = next.find(&root_old.identity()).unwrap();
        // Origin records how management began; updating recovered content
        // keeps it recovered. The policy fields survive; the receipt is fresh.
        assert_eq!(updated_root.origin, ProviderOrigin::Recovered);
        assert!(!updated_root.pinned);
        assert_eq!(updated_root.update_channel, UpdateChannel::Beta);
        assert!(updated_root.installed_at_unix_seconds.is_some());
        assert_eq!(updated_root.file_name, "root-v2.jar");
        assert!(updated_root.requires.contains(&dep_old.identity()));
        let updated_dep = next.find(&dep_old.identity()).unwrap();
        assert!(!updated_dep.explicitly_retained);
        assert_eq!(updated_dep.origin, ProviderOrigin::Dependency);
        assert_eq!(updated_dep.version_id, "dep-v2");
        assert!(next.validate().is_ok());
    }

    #[test]
    fn updated_state_multi_replaces_only_owned_dependencies() {
        // A replaced dependency that an outside record still requires blocks
        // the whole transaction; the parent must join the same update.
        let root_a = policy_record("AAAA0001", "a.jar", b"a-old", true, ProviderOrigin::Direct);
        let shared = policy_record(
            "BBBB0002",
            "shared.jar",
            b"shared-v1",
            false,
            ProviderOrigin::Dependency,
        );
        let outsider = policy_record(
            "CCCC0003",
            "outsider.jar",
            b"outsider",
            true,
            ProviderOrigin::Direct,
        );
        let mut current = ContentState::empty();
        current.entries = vec![root_a.clone(), shared.clone(), outsider.clone()];
        current.entries[0].requires = vec![shared.identity()];
        current.entries[2].requires = vec![shared.identity()];
        current.validate().unwrap();

        let mut a_new = policy_record(
            "AAAA0001",
            "a-v2.jar",
            b"a-new",
            true,
            ProviderOrigin::Direct,
        );
        a_new.version_id = "a-v2".into();
        let mut shared_new = policy_record(
            "BBBB0002",
            "shared-v2.jar",
            b"shared-v2",
            false,
            ProviderOrigin::Dependency,
        );
        shared_new.version_id = "shared-v2".into();
        assert!(matches!(
            updated_state_multi(
                &current,
                &[root_a.identity()],
                vec![shared_new.clone(), a_new.clone()]
            ),
            Err(ContentError::DependencyBlocked(_))
        ));

        // An explicitly retained dependency is never silently replaced.
        let mut retained_dep = shared.clone();
        retained_dep.explicitly_retained = true;
        let mut retained_state = current.clone();
        retained_state.entries[1] = retained_dep.clone();
        retained_state.entries[2].requires = Vec::new();
        retained_state.validate().unwrap();
        assert!(matches!(
            updated_state_multi(
                &retained_state,
                &[root_a.identity()],
                vec![shared_new.clone(), a_new.clone()]
            ),
            Err(ContentError::RequiredByInstalledContent)
        ));

        // Dependency records are not roots: an unretained identity cannot be
        // updated on its own.
        assert!(matches!(
            updated_state_multi(&current, &[shared.identity()], vec![shared_new]),
            Err(ContentError::RequiredByInstalledContent)
        ));
    }

    #[test]
    fn pinned_non_retained_dependency_is_not_superseded_by_parent_update() {
        let root = policy_record(
            "AAAA0001",
            "root.jar",
            b"old-root",
            true,
            ProviderOrigin::Direct,
        );
        let mut dependency = policy_record(
            "BBBB0002",
            "dep.jar",
            b"old-dep",
            false,
            ProviderOrigin::Dependency,
        );
        dependency.pinned = true;
        let mut current = ContentState::empty();
        current.entries = vec![root.clone(), dependency.clone()];
        current.entries[0].requires = vec![dependency.identity()];
        current.validate().unwrap();

        let mut new_root = policy_record(
            "AAAA0001",
            "root-new.jar",
            b"new-root",
            true,
            ProviderOrigin::Direct,
        );
        new_root.version_id = "new-root".into();
        new_root.dependencies = vec![ProviderDependency {
            kind: DependencyKind::Required,
            provider: "modrinth".into(),
            project_id: dependency.project_id.clone(),
            version_id: None,
        }];
        let mut new_dependency = policy_record(
            "BBBB0002",
            "dep-new.jar",
            b"new-dep",
            false,
            ProviderOrigin::Dependency,
        );
        new_dependency.version_id = "new-dep".into();
        assert!(matches!(
            updated_state_multi(&current, &[root.identity()], vec![new_dependency, new_root]),
            Err(ContentError::RequiredByInstalledContent)
        ));
        assert_eq!(current.entries[1], dependency);
    }

    #[test]
    fn updated_state_multi_updates_two_roots_and_a_shared_dependency() {
        let root_a = policy_record("AAAA0001", "a.jar", b"a-old", true, ProviderOrigin::Direct);
        let root_b = policy_record("CCCC0003", "b.jar", b"b-old", true, ProviderOrigin::Direct);
        let shared = policy_record(
            "BBBB0002",
            "shared.jar",
            b"shared-v1",
            false,
            ProviderOrigin::Dependency,
        );
        let mut current = ContentState::empty();
        current.entries = vec![root_a.clone(), root_b.clone(), shared.clone()];
        current.entries[0].requires = vec![shared.identity()];
        current.entries[1].requires = vec![shared.identity()];
        current.validate().unwrap();

        let mut a_new = policy_record(
            "AAAA0001",
            "a-v2.jar",
            b"a-new",
            true,
            ProviderOrigin::Direct,
        );
        a_new.version_id = "a-v2".into();
        a_new.dependencies = vec![ProviderDependency {
            kind: DependencyKind::Required,
            provider: "modrinth".into(),
            project_id: "BBBB0002".into(),
            version_id: None,
        }];
        let mut b_new = policy_record(
            "CCCC0003",
            "b-v2.jar",
            b"b-new",
            true,
            ProviderOrigin::Direct,
        );
        b_new.version_id = "b-v2".into();
        b_new.dependencies = a_new.dependencies.clone();
        let mut shared_new = policy_record(
            "BBBB0002",
            "shared-v2.jar",
            b"shared-v2",
            false,
            ProviderOrigin::Dependency,
        );
        shared_new.version_id = "shared-v2".into();
        let next = updated_state_multi(
            &current,
            &[root_a.identity(), root_b.identity()],
            vec![shared_new.clone(), a_new.clone(), b_new.clone()],
        )
        .unwrap();
        assert_eq!(next.entries.len(), 3);
        let shared_updated = next.find(&shared.identity()).unwrap();
        assert_eq!(shared_updated.version_id, "shared-v2");
        assert!(!shared_updated.explicitly_retained);
        // Both updating roots keep their requirement edges on the identity.
        assert_eq!(
            next.find(&root_a.identity()).unwrap().requires,
            vec![shared.identity()]
        );
        assert_eq!(
            next.find(&root_b.identity()).unwrap().requires,
            vec![shared.identity()]
        );
        assert!(next.validate().is_ok());

        // Repeating a root in one transaction is refused.
        assert!(matches!(
            updated_state_multi(
                &current,
                &[root_a.identity(), root_a.identity()],
                vec![shared_new, a_new, b_new]
            ),
            Err(ContentError::StateMalformed(_))
        ));
    }

    fn versioned_plan(
        server: &TestServer,
        project: &str,
        mod_id: &str,
        version_id: &str,
        dependency: Option<&str>,
    ) -> ProviderInstallPlan {
        let bytes = fabric_jar_with_id(mod_id);
        let hash = format!("{:x}", Sha512::digest(&bytes));
        ProviderInstallPlan {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: project.into(),
            version_id: version_id.into(),
            file_id: hash.clone(),
            file_name: format!("{mod_id}.jar"),
            display_version: Some(mod_id.into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: Some("fabric".into()),
                environment: Some("client_and_server".into()),
            },
            dependencies: match dependency {
                Some(project) => vec![ProviderDependency {
                    kind: DependencyKind::Required,
                    provider: "modrinth".into(),
                    project_id: project.into(),
                    version_id: None,
                }],
                None => Vec::new(),
            },
            source: ProviderArtifactSource::Sha512(
                Sha512ArtifactSource::loopback_http_for_testing(
                    &format!("{}/{mod_id}", server.base_url()),
                    &hash,
                    Some(bytes.len() as u64),
                )
                .unwrap(),
            ),
        }
    }

    #[tokio::test]
    async fn multi_root_update_executes_atomically_and_preserves_provenance() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        // Each install transaction has exactly one root; two roots require
        // two explicit installs sharing one dependency graph.
        install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![
                versioned_plan(&server, "BBBB0002", "shared-v1", "sv1", None),
                versioned_plan(&server, "AAAA0001", "root-a-v1", "av1", Some("BBBB0002")),
            ],
        )
        .await
        .unwrap();
        install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![versioned_plan(
                &server,
                "CCCC0003",
                "root-b-v1",
                "bv1",
                Some("BBBB0002"),
            )],
        )
        .await
        .unwrap();
        // Install assigns dependency origin to the shared mod; give the roots
        // a recovered origin to prove updates never rewrite provenance.
        let mut state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        let roots: Vec<_> = state
            .entries
            .iter()
            .filter(|record| record.explicitly_retained)
            .map(|record| record.identity())
            .collect();
        assert_eq!(roots.len(), 2);
        for record in &mut state.entries {
            if record.explicitly_retained {
                record.origin = ProviderOrigin::Recovered;
            }
        }
        state.save(&fixture.managed, &fixture.instance).unwrap();

        let next = update_provider_graph_multi(
            &fixture.managed,
            &fixture.instance,
            &state,
            &roots,
            vec![
                versioned_plan(&server, "BBBB0002", "shared-v2", "sv2", None),
                versioned_plan(&server, "AAAA0001", "root-a-v2", "av2", Some("BBBB0002")),
                versioned_plan(&server, "CCCC0003", "root-b-v2", "bv2", Some("BBBB0002")),
            ],
        )
        .await
        .unwrap();
        let persisted = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        // save() sorts entries for the on-disk document; compare by identity.
        assert_eq!(persisted.entries.len(), next.entries.len());
        for record in &next.entries {
            assert_eq!(persisted.find(&record.identity()), Some(record));
        }
        let directory = fixture.dir(ContentType::Mod);
        for name in ["root-a-v2.jar", "root-b-v2.jar", "shared-v2.jar"] {
            assert!(directory.join(name).is_file(), "{name} missing");
        }
        for name in ["root-a-v1.jar", "root-b-v1.jar", "shared-v1.jar"] {
            assert!(!directory.join(name).exists(), "{name} still present");
        }
        for record in &persisted.entries {
            if record.explicitly_retained {
                assert_eq!(record.origin, ProviderOrigin::Recovered);
            } else {
                assert_eq!(record.origin, ProviderOrigin::Dependency);
                assert_eq!(record.version_id, "sv2");
            }
        }
        // The inventory stays valid after the swap.
        let inventory = crate::instance_mods::scan(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(inventory.entries.len(), 3);
    }

    #[tokio::test]
    async fn multi_root_failures_leave_both_old_versions_intact() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![versioned_plan(
                &server,
                "AAAA0001",
                "root-a-v1",
                "av1",
                None,
            )],
        )
        .await
        .unwrap();
        install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![versioned_plan(
                &server,
                "CCCC0003",
                "root-b-v1",
                "bv1",
                None,
            )],
        )
        .await
        .unwrap();
        let state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        let roots: Vec<_> = state
            .entries
            .iter()
            .filter(|record| record.explicitly_retained)
            .map(|record| record.identity())
            .collect();
        assert_eq!(roots.len(), 2);
        let directory = fixture.dir(ContentType::Mod);
        let originals: Vec<_> = roots
            .iter()
            .filter_map(|identity| state.find(identity))
            .map(|record| {
                (
                    record.file_name.clone(),
                    std::fs::read(directory.join(&record.file_name)).unwrap(),
                )
            })
            .collect();

        // Download failure for the second root aborts before any mutation.
        let failing = TestServer::spawn(Arc::new(|request: &TestRequest| {
            if request.path.starts_with("/root-a-v2") {
                TestResponse::ok(&fabric_jar_with_id("root-a-v2"))
            } else {
                TestResponse::status(500)
            }
        }));
        assert!(matches!(
            update_provider_graph_multi(
                &fixture.managed,
                &fixture.instance,
                &state,
                &roots,
                vec![
                    versioned_plan(&failing, "AAAA0001", "root-a-v2", "av2", None),
                    versioned_plan(&failing, "CCCC0003", "root-b-v2", "bv2", None),
                ],
            )
            .await,
            Err(ContentError::Acquisition(_))
        ));
        for (name, bytes) in &originals {
            assert_eq!(std::fs::read(directory.join(name)).unwrap(), *bytes);
        }
        assert_eq!(
            ContentState::load(&fixture.managed, &fixture.instance).unwrap(),
            state
        );

        // State-commit failure after activation rolls both roots back.
        let plans = vec![
            versioned_plan(&server, "AAAA0001", "root-a-v2", "av2", None),
            versioned_plan(&server, "CCCC0003", "root-b-v2", "bv2", None),
        ];
        assert!(matches!(
            update_provider_graph_multi_with_hooks(
                &fixture.managed,
                &fixture.instance,
                &state,
                &roots,
                plans,
                |_| Err(ContentError::StateMalformed(
                    "injected commit failure".into()
                )),
            )
            .await,
            Err(ContentError::StateMalformed(_))
        ));
        for (name, bytes) in &originals {
            assert!(directory.join(name).is_file(), "{name} not restored");
            assert_eq!(std::fs::read(directory.join(name)).unwrap(), *bytes);
        }
        assert!(!directory.join("root-a-v2.jar").exists());
        assert!(!directory.join("root-b-v2.jar").exists());
        assert_eq!(
            ContentState::load(&fixture.managed, &fixture.instance).unwrap(),
            state
        );

        // A stale inventory revision is refused before mutation.
        std::fs::write(
            directory.join("external.jar"),
            fabric_jar_with_id("external"),
        )
        .unwrap();
        let stale_revision = String::new();
        assert!(matches!(
            update_provider_graph_multi_reviewed(
                &fixture.managed,
                &fixture.instance,
                &state,
                &roots,
                vec![
                    versioned_plan(&server, "AAAA0001", "root-a-v2", "av2", None),
                    versioned_plan(&server, "CCCC0003", "root-b-v2", "bv2", None),
                ],
                Some(&stale_revision),
            )
            .await,
            Err(ContentError::ChangedSinceScan)
        ));
    }

    #[tokio::test]
    async fn updating_a_disabled_root_is_refused_with_state_untouched() {
        let fixture = no_aurora_fixture();
        let server = served_mods();
        let installed = install_provider_plans(
            &fixture.managed,
            &fixture.instance,
            vec![versioned_plan(
                &server,
                "AAAA0001",
                "root-a-v1",
                "av1",
                None,
            )],
        )
        .await
        .unwrap();
        let state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        let directory = fixture.dir(ContentType::Mod);
        let active = directory.join("root-a-v1.jar");
        let disabled = directory.join("root-a-v1.jar.disabled");
        std::fs::rename(&active, &disabled).unwrap();

        assert!(matches!(
            update_provider_graph(
                &fixture.managed,
                &fixture.instance,
                &state,
                &installed[0].identity(),
                vec![versioned_plan(
                    &server,
                    "AAAA0001",
                    "root-a-v2",
                    "av2",
                    None
                )],
            )
            .await,
            Err(ContentError::DependencyBlocked(_))
        ));
        assert!(disabled.is_file());
        assert!(!active.exists());
        assert!(!directory.join("root-a-v2.jar").exists());
        assert_eq!(
            ContentState::load(&fixture.managed, &fixture.instance).unwrap(),
            state
        );
    }

    #[tokio::test]
    async fn shader_preview_and_install_work_when_the_destination_does_not_exist_yet() {
        // Regression: a first shader install into an instance whose
        // shaderpacks directory has never been created must not fail the
        // preview with a raw I/O error (the original generic-toast defect).
        let fixture = no_aurora_fixture();
        let shader_bytes: Vec<u8> = {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer
                .start_file(
                    "shaders/basic.fsh",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(b"void main(){}").unwrap();
            writer.finish().unwrap().into_inner()
        };
        let hash = format!("{:x}", Sha512::digest(&shader_bytes));
        let served = shader_bytes.clone();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            if request.path == "/shader-pack" {
                TestResponse::ok(&served)
            } else {
                TestResponse::status(404)
            }
        }));
        let make_plan = || {
            let source = Sha512ArtifactSource::loopback_http_for_testing(
                &format!("{}/shader-pack", server.base_url()),
                &hash,
                Some(shader_bytes.len() as u64),
            )
            .unwrap();
            ProviderInstallPlan {
                content_type: ContentType::ShaderPack,
                provider: "modrinth".into(),
                project_id: "SHDR0001".into(),
                version_id: "shdr0002".into(),
                file_id: hash.clone(),
                file_name: "shader-pack.zip".into(),
                display_version: Some("1.0.0".into()),
                compatibility: ContentCompatibility {
                    minecraft_versions: vec!["1.21.11".into()],
                    loader: None,
                    environment: Some("client_and_server".into()),
                },
                dependencies: vec![],
                source: ProviderArtifactSource::Sha512(source),
            }
        };
        let shaderpacks = fixture
            .managed
            .instance_paths(&fixture.instance)
            .shaderpacks()
            .to_path_buf();
        assert!(
            !shaderpacks.exists(),
            "fixture must start without the directory"
        );

        let preview_plan = make_plan();
        let conflicts =
            preview_provider_conflicts(&fixture.managed, &fixture.instance, &[preview_plan])
                .await
                .unwrap();
        assert!(conflicts.is_empty());

        let records =
            install_provider_plans(&fixture.managed, &fixture.instance, vec![make_plan()])
                .await
                .unwrap();
        assert_eq!(records[0].content_type, ContentType::ShaderPack);
        assert!(shaderpacks.join("shader-pack.zip").is_file());
        let state = ContentState::load(&fixture.managed, &fixture.instance).unwrap();
        assert_eq!(state.entries.len(), 1);
    }
}
