//! Launcher-side per-instance organization metadata (pinned state and custom tags).
//!
//! Stored in a separate schema-1 document (`launcher/instance-organization.json`)
//! so `instances.json` schema-6 compatibility stays strictly untouched.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::instances::InstanceId;
use crate::paths::ManagedPaths;

pub const ORGANIZATION_SCHEMA_VERSION: u32 = 1;
const MAX_TAGS_PER_INSTANCE: usize = 12;
const MAX_TAG_LEN: usize = 32;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceMeta {
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceOrganization {
    pub schema_version: u32,
    #[serde(default)]
    pub instances: BTreeMap<String, InstanceMeta>,
}

impl Default for InstanceOrganization {
    fn default() -> Self {
        Self {
            schema_version: ORGANIZATION_SCHEMA_VERSION,
            instances: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub enum OrganizationError {
    UnsupportedSchema { found: u32, supported: u32 },
    Malformed(String),
    InvalidTag(String),
    Io(String),
}

impl std::fmt::Display for OrganizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema { found, supported } => write!(
                f,
                "unsupported instance organization schema {found} (supported: {supported})"
            ),
            Self::Malformed(reason) => write!(f, "malformed instance organization file: {reason}"),
            Self::InvalidTag(reason) => write!(f, "invalid instance tag: {reason}"),
            Self::Io(reason) => write!(f, "instance organization I/O failed: {reason}"),
        }
    }
}

impl std::error::Error for OrganizationError {}

pub fn normalize_tags(raw: &[String]) -> Result<Vec<String>, OrganizationError> {
    let mut out = Vec::new();
    for tag in raw {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_TAG_LEN {
            return Err(OrganizationError::InvalidTag(format!(
                "tags must be at most {MAX_TAG_LEN} characters"
            )));
        }
        if trimmed.chars().any(char::is_control) {
            return Err(OrganizationError::InvalidTag(
                "tags cannot contain control characters".into(),
            ));
        }
        if !out
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(trimmed))
        {
            out.push(trimmed.to_owned());
        }
        if out.len() > MAX_TAGS_PER_INSTANCE {
            return Err(OrganizationError::InvalidTag(format!(
                "an instance can have at most {MAX_TAGS_PER_INSTANCE} tags"
            )));
        }
    }
    Ok(out)
}

fn file_path(managed: &ManagedPaths) -> std::path::PathBuf {
    managed.launcher_dir().join("instance-organization.json")
}

pub fn load(managed: &ManagedPaths) -> Result<InstanceOrganization, OrganizationError> {
    load_path(&file_path(managed))
}

fn load_path(path: &Path) -> Result<InstanceOrganization, OrganizationError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(InstanceOrganization::default());
        }
        Err(e) => return Err(OrganizationError::Io(e.to_string())),
    };
    let raw: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| OrganizationError::Malformed(e.to_string()))?;
    let version = raw
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| OrganizationError::Malformed("missing schemaVersion".into()))?;
    if version != u64::from(ORGANIZATION_SCHEMA_VERSION) {
        return Err(OrganizationError::UnsupportedSchema {
            found: u32::try_from(version).unwrap_or(u32::MAX),
            supported: ORGANIZATION_SCHEMA_VERSION,
        });
    }
    serde_json::from_value(raw).map_err(|e| OrganizationError::Malformed(e.to_string()))
}

pub fn save(
    managed: &ManagedPaths,
    organization: &InstanceOrganization,
) -> Result<(), OrganizationError> {
    let path = file_path(managed);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| OrganizationError::Io(e.to_string()))?;
    }
    let tmp = path.with_extension(format!("json.tmp-{}", uuid::Uuid::new_v4().simple()));
    let json = serde_json::to_string_pretty(organization)
        .map_err(|e| OrganizationError::Io(e.to_string()))?;
    fs::write(&tmp, format!("{json}\n")).map_err(|e| OrganizationError::Io(e.to_string()))?;
    if let Err(err) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(OrganizationError::Io(err.to_string()));
    }
    Ok(())
}

pub fn set_meta(
    managed: &ManagedPaths,
    id: &InstanceId,
    pinned: bool,
    tags: &[String],
) -> Result<InstanceOrganization, OrganizationError> {
    let tags = normalize_tags(tags)?;
    let mut org = load(managed)?;
    if !pinned && tags.is_empty() {
        org.instances.remove(id.as_str());
    } else {
        org.instances
            .insert(id.as_str().to_owned(), InstanceMeta { pinned, tags });
    }
    save(managed, &org)?;
    Ok(org)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tag_validation() {
        let root = std::env::temp_dir().join(format!("aurora-org-test-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let id = crate::instances::generate_instance_id();

        let org = set_meta(
            &managed,
            &id,
            true,
            &["PvP".into(), "  pvp ".into(), "SMP".into()],
        )
        .unwrap();
        let meta = org.instances.get(id.as_str()).unwrap();
        assert!(meta.pinned);
        assert_eq!(meta.tags, vec!["PvP", "SMP"]);

        let reloaded = load(&managed).unwrap();
        assert_eq!(reloaded, org);

        assert!(set_meta(&managed, &id, false, &["bad\nline".into()]).is_err());

        let _ = fs::remove_dir_all(&root);
    }
}
