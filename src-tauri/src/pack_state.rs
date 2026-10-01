//! Exact installed Modrinth pack identity and ownership. Provider records keep
//! their historical origin; this document references them by stable identity
//! and exact installed bytes. Unknown schemas and damaged state fail closed.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::instance_content::{ContentState, ContentType, ProviderIdentity};
use crate::instances::InstanceId;
use crate::integrity::{ArtifactDigest, verify_file};
use crate::paths::ManagedPaths;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackIdentity {
    pub provider: String,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub pack_version: String,
    pub artifact_sha512: String,
    pub artifact_sha256: String,
    pub minecraft_version: String,
    pub fabric_loader_version: String,
    pub installed_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedComponent {
    pub path: String,
    pub sha256: String,
    pub sha512: String,
    /// None means this exact pack file was not recognized by Modrinth. It is
    /// still pack-owned, hash-verified, and explicitly not provider-managed.
    pub provider: Option<ProviderIdentity>,
    pub provider_version_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedOverride {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledPack {
    schema_version: u32,
    pub instance_id: InstanceId,
    pub identity: PackIdentity,
    pub components: Vec<OwnedComponent>,
    pub overrides: Vec<OwnedOverride>,
    pub excluded_paths: Vec<String>,
}

#[derive(Debug)]
pub enum PackStateError {
    Io(std::io::Error),
    Malformed,
    UnsupportedSchema(u64),
    UnsafePath,
    Integrity,
    ProviderMismatch,
    InstanceMismatch,
}

impl PackStateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "pack_state_io_error",
            Self::Malformed => "pack_state_malformed",
            Self::UnsupportedSchema(_) => "pack_state_unsupported_schema",
            Self::UnsafePath => "pack_state_unsafe_path",
            Self::Integrity => "pack_state_integrity_failure",
            Self::ProviderMismatch => "pack_state_provider_mismatch",
            Self::InstanceMismatch => "pack_state_instance_mismatch",
        }
    }
}

impl fmt::Display for PackStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for PackStateError {}

impl InstalledPack {
    pub fn new(
        instance_id: InstanceId,
        identity: PackIdentity,
        components: Vec<OwnedComponent>,
        overrides: Vec<OwnedOverride>,
        excluded_paths: Vec<String>,
    ) -> Result<Self, PackStateError> {
        let value = Self {
            schema_version: SCHEMA_VERSION,
            instance_id,
            identity,
            components,
            overrides,
            excluded_paths,
        };
        value.validate_document()?;
        Ok(value)
    }

    fn validate_document(&self) -> Result<(), PackStateError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(PackStateError::UnsupportedSchema(u64::from(
                self.schema_version,
            )));
        }
        let id = &self.identity;
        if id.provider != "modrinth"
            || id.project_id.is_empty()
            || id.version_id.is_empty()
            || id.name.is_empty()
            || id.pack_version.is_empty()
            || id.minecraft_version.is_empty()
            || id.fabric_loader_version.is_empty()
            || !hex(&id.artifact_sha512, 128)
            || !hex(&id.artifact_sha256, 64)
        {
            return Err(PackStateError::Malformed);
        }
        let mut seen = HashSet::new();
        for path in self
            .components
            .iter()
            .map(|v| v.path.as_str())
            .chain(self.overrides.iter().map(|v| v.path.as_str()))
            .chain(self.excluded_paths.iter().map(String::as_str))
        {
            crate::mrpack::destination_path(path).map_err(|_| PackStateError::UnsafePath)?;
            if !seen.insert(path.to_lowercase()) {
                return Err(PackStateError::Malformed);
            }
        }
        for item in &self.components {
            if !hex(&item.sha256, 64)
                || !hex(&item.sha512, 128)
                || item.provider.is_some() != item.provider_version_id.is_some()
            {
                return Err(PackStateError::Malformed);
            }
            if let Some(provider) = &item.provider {
                if provider.provider != "modrinth"
                    || provider.project_id.is_empty()
                    || !matches!(
                        provider.content_type,
                        ContentType::Mod | ContentType::ResourcePack | ContentType::ShaderPack
                    )
                {
                    return Err(PackStateError::Malformed);
                }
            }
        }
        if self.overrides.iter().any(|v| !hex(&v.sha256, 64)) {
            return Err(PackStateError::Malformed);
        }
        Ok(())
    }

    pub fn from_json(text: &str) -> Result<Self, PackStateError> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|_| PackStateError::Malformed)?;
        let version = value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            .ok_or(PackStateError::Malformed)?;
        if version != u64::from(SCHEMA_VERSION) {
            return Err(PackStateError::UnsupportedSchema(version));
        }
        let parsed: Self = serde_json::from_value(value).map_err(|_| PackStateError::Malformed)?;
        parsed.validate_document()?;
        Ok(parsed)
    }

    pub fn load(
        managed: &ManagedPaths,
        instance: &InstanceId,
    ) -> Result<Option<Self>, PackStateError> {
        let path = state_path(managed, instance)?;
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(PackStateError::Io(error)),
        };
        let state = Self::from_json(&text)?;
        if &state.instance_id != instance {
            return Err(PackStateError::InstanceMismatch);
        }
        Ok(Some(state))
    }

    pub fn save(&self, managed: &ManagedPaths) -> Result<(), PackStateError> {
        self.validate_document()?;
        let path = state_path(managed, &self.instance_id)?;
        if Self::load(managed, &self.instance_id)?.is_some() {
            return Err(PackStateError::Malformed);
        }
        let temp = path.with_extension(format!("json.{}.tmp", uuid::Uuid::new_v4()));
        let mut json = serde_json::to_string_pretty(self).map_err(|_| PackStateError::Malformed)?;
        json.push('\n');
        std::fs::write(&temp, json).map_err(PackStateError::Io)?;
        if let Err(error) = std::fs::rename(&temp, &path) {
            let _ = std::fs::remove_file(&temp);
            return Err(PackStateError::Io(error));
        }
        Ok(())
    }

    pub fn owns_provider(&self, identity: &ProviderIdentity) -> bool {
        self.components
            .iter()
            .any(|component| component.provider.as_ref() == Some(identity))
    }

    pub fn owns_path(&self, path: &str) -> bool {
        self.components
            .iter()
            .any(|component| component.path.eq_ignore_ascii_case(path))
            || self
                .overrides
                .iter()
                .any(|override_file| override_file.path.eq_ignore_ascii_case(path))
    }

    /// Read-only, network-free validation against the exact installed pack
    /// snapshot. A modified user config remains visible but is reported as
    /// divergence instead of being silently repaired or overwritten.
    pub fn validate_installed(&self, managed: &ManagedPaths) -> Result<(), PackStateError> {
        self.validate_document()?;
        let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
            .map_err(|_| PackStateError::Malformed)?;
        let record = registry
            .find(&self.instance_id)
            .ok_or(PackStateError::InstanceMismatch)?;
        let pack = record.pack().ok_or(PackStateError::InstanceMismatch)?;
        if pack.provider != self.identity.provider
            || pack.project_id != self.identity.project_id
            || pack.version_id != self.identity.version_id
            || pack.name != self.identity.name
            || pack.pack_version != self.identity.pack_version
        {
            return Err(PackStateError::InstanceMismatch);
        }
        if record.installed().minecraft_version != self.identity.minecraft_version
            || record.installed().platform.version() != Some(&self.identity.fabric_loader_version)
            || record.installed().aurora.is_some()
        {
            return Err(PackStateError::InstanceMismatch);
        }
        let root = crate::instance_content::validated_instance_root(managed, &self.instance_id)
            .map_err(|_| PackStateError::UnsafePath)?;
        let provider = ContentState::load(managed, &self.instance_id)
            .map_err(|_| PackStateError::ProviderMismatch)?;
        for component in &self.components {
            let path = safe_file(&root, &component.path)?;
            let digest =
                ArtifactDigest::parse(&component.sha256).map_err(|_| PackStateError::Malformed)?;
            verify_file(&path, &digest, None).map_err(|_| PackStateError::Integrity)?;
            if let Some(identity) = &component.provider {
                let record = provider
                    .find(identity)
                    .ok_or(PackStateError::ProviderMismatch)?;
                if record.version_id
                    != *component
                        .provider_version_id
                        .as_ref()
                        .ok_or(PackStateError::Malformed)?
                    || record.sha256 != component.sha256
                    || component.path
                        != format!(
                            "{}/{}",
                            record.content_type.directory_name(),
                            record.file_name
                        )
                {
                    return Err(PackStateError::ProviderMismatch);
                }
            }
        }
        for item in &self.overrides {
            let path = safe_file(&root, &item.path)?;
            let digest =
                ArtifactDigest::parse(&item.sha256).map_err(|_| PackStateError::Malformed)?;
            verify_file(&path, &digest, None).map_err(|_| PackStateError::Integrity)?;
        }
        Ok(())
    }
}

fn hex(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|v| v.is_ascii_hexdigit())
}

fn state_path(managed: &ManagedPaths, instance: &InstanceId) -> Result<PathBuf, PackStateError> {
    let root = crate::instance_content::validated_instance_root(managed, instance)
        .map_err(|_| PackStateError::UnsafePath)?;
    let path = root.join("pack-installed.json");
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() || reparse(&meta) => {
            Err(PackStateError::UnsafePath)
        }
        Ok(_) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
        Err(error) => Err(PackStateError::Io(error)),
    }
}

fn safe_file(root: &Path, relative: &str) -> Result<PathBuf, PackStateError> {
    crate::mrpack::destination_path(relative).map_err(|_| PackStateError::UnsafePath)?;
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
        let meta = std::fs::symlink_metadata(&path).map_err(PackStateError::Io)?;
        if meta.file_type().is_symlink() || reparse(&meta) {
            return Err(PackStateError::UnsafePath);
        }
    }
    if !std::fs::symlink_metadata(&path)
        .map_err(PackStateError::Io)?
        .is_file()
    {
        return Err(PackStateError::UnsafePath);
    }
    Ok(path)
}

#[cfg(windows)]
fn reparse(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse(_: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_rejects_unknown_schema_damage_and_conflicting_ownership() {
        let identity = PackIdentity {
            provider: "modrinth".into(),
            project_id: "PACK0001".into(),
            version_id: "VERS0001".into(),
            name: "Pack".into(),
            pack_version: "1".into(),
            artifact_sha512: "a".repeat(128),
            artifact_sha256: "b".repeat(64),
            minecraft_version: "1.21.1".into(),
            fabric_loader_version: "0.16.0".into(),
            installed_at_unix_seconds: 1,
        };
        let state = InstalledPack::new(
            InstanceId::new("1234567890abcdef1234567890abcdef").unwrap(),
            identity,
            vec![OwnedComponent {
                path: "mods/a.jar".into(),
                sha256: "c".repeat(64),
                sha512: "d".repeat(128),
                provider: None,
                provider_version_id: None,
            }],
            vec![],
            vec![],
        )
        .unwrap();
        assert!(state.owns_path("mods/A.jar"));
        let text = serde_json::to_string(&state).unwrap();
        assert_eq!(
            InstalledPack::from_json(&text).unwrap().identity.version_id,
            "VERS0001"
        );
        assert!(matches!(
            InstalledPack::from_json(&text.replace("\"schemaVersion\":1", "\"schemaVersion\":2")),
            Err(PackStateError::UnsupportedSchema(2))
        ));
        assert!(matches!(
            InstalledPack::from_json("{}"),
            Err(PackStateError::Malformed)
        ));
        let mut conflict = state.clone();
        conflict.overrides.push(OwnedOverride {
            path: "mods/a.jar".into(),
            sha256: "e".repeat(64),
        });
        assert!(matches!(
            conflict.validate_document(),
            Err(PackStateError::Malformed)
        ));
    }
}
