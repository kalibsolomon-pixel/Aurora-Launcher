//! Per-instance display icons: a Material Symbols glyph name per instance.
//!
//! Icons live in their own small versioned document instead of the instance
//! registry, so the registry schema (and older launchers reading it) is
//! untouched. Names are presentation-only identifiers; they never reach a
//! path, URL or command. Unknown schemas and malformed files fail deliberately
//! and are never overwritten.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::instances::{InstanceId, InstanceRegistry};
use crate::paths::ManagedPaths;

pub const INSTANCE_ICONS_SCHEMA_VERSION: u32 = 1;
const MAX_ICON_NAME_LENGTH: usize = 64;
const MAX_ENTRIES: usize = 4096;

#[derive(Debug)]
pub enum IconError {
    InvalidName,
    InstanceNotFound,
    UnsupportedSchema(u32),
    Malformed(String),
    Registry(String),
    Io(std::io::Error),
}

impl std::fmt::Display for IconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName => {
                f.write_str("Icon names are 1–64 lowercase letters, digits or underscores.")
            }
            Self::InstanceNotFound => f.write_str("The instance no longer exists."),
            Self::UnsupportedSchema(version) => write!(
                f,
                "Instance icons use unsupported schema {version}; the file was left unchanged."
            ),
            Self::Malformed(reason) => write!(
                f,
                "Instance icons could not be read ({reason}); the file was left unchanged."
            ),
            Self::Registry(reason) => f.write_str(reason),
            Self::Io(error) => write!(f, "Instance icons could not be saved: {}", error.kind()),
        }
    }
}

impl IconError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidName => "instance_icon_invalid",
            Self::InstanceNotFound => "instance_not_found",
            Self::UnsupportedSchema(_) => "instance_icons_unsupported_schema",
            Self::Malformed(_) => "instance_icons_malformed",
            Self::Registry(_) => "instance_registry_unavailable",
            Self::Io(_) => "instance_icons_io",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IconDocument {
    schema_version: u32,
    icons: BTreeMap<String, String>,
}

pub fn validate_icon_name(name: &str) -> Result<(), IconError> {
    if name.is_empty()
        || name.len() > MAX_ICON_NAME_LENGTH
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(IconError::InvalidName);
    }
    Ok(())
}

fn document_path(managed: &ManagedPaths) -> PathBuf {
    managed.launcher_dir().join("instance-icons.json")
}

fn parse(text: &str) -> Result<BTreeMap<String, String>, IconError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| IconError::Malformed(e.to_string()))?;
    let version = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| IconError::Malformed("missing schemaVersion".into()))?;
    if version != u64::from(INSTANCE_ICONS_SCHEMA_VERSION) {
        return Err(IconError::UnsupportedSchema(version as u32));
    }
    let document: IconDocument =
        serde_json::from_value(value).map_err(|e| IconError::Malformed(e.to_string()))?;
    if document.icons.len() > MAX_ENTRIES {
        return Err(IconError::Malformed("too many entries".into()));
    }
    for (id, name) in &document.icons {
        InstanceId::new(id.clone())
            .map_err(|_| IconError::Malformed("invalid instance id".into()))?;
        validate_icon_name(name).map_err(|_| IconError::Malformed("invalid icon name".into()))?;
    }
    Ok(document.icons)
}

fn load_from(path: &Path) -> Result<BTreeMap<String, String>, IconError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(IconError::Io(error)),
    }
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Icons for registered instances only; entries for removed instances are
/// ignored (and dropped on the next write).
pub fn load(managed: &ManagedPaths) -> Result<BTreeMap<String, String>, IconError> {
    let _guard = lock();
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|e| IconError::Registry(e.to_string()))?;
    let mut icons = load_from(&document_path(managed))?;
    icons.retain(|id, _| InstanceId::new(id.clone()).is_ok_and(|id| registry.find(&id).is_some()));
    Ok(icons)
}

/// Set (`Some`) or clear (`None`) one registered instance's icon.
pub fn set(
    managed: &ManagedPaths,
    instance: &InstanceId,
    icon: Option<&str>,
) -> Result<BTreeMap<String, String>, IconError> {
    if let Some(name) = icon {
        validate_icon_name(name)?;
    }
    let _guard = lock();
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|e| IconError::Registry(e.to_string()))?;
    if registry.find(instance).is_none() {
        return Err(IconError::InstanceNotFound);
    }
    let path = document_path(managed);
    let mut icons = load_from(&path)?;
    icons.retain(|id, _| InstanceId::new(id.clone()).is_ok_and(|id| registry.find(&id).is_some()));
    match icon {
        Some(name) => {
            icons.insert(instance.to_string(), name.to_owned());
        }
        None => {
            icons.remove(instance.as_str());
        }
    }
    let document = IconDocument {
        schema_version: INSTANCE_ICONS_SCHEMA_VERSION,
        icons: icons.clone(),
    };
    let bytes =
        serde_json::to_vec_pretty(&document).map_err(|e| IconError::Malformed(e.to_string()))?;
    std::fs::create_dir_all(managed.launcher_dir()).map_err(IconError::Io)?;
    crate::install::journal::atomic_write(&path, &bytes).map_err(IconError::Io)?;
    Ok(icons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_names_are_bounded_symbol_identifiers() {
        for good in ["rocket_launch", "home", "looks_3", "a"] {
            validate_icon_name(good).unwrap();
        }
        for bad in [
            "",
            "Rocket",
            "rocket-launch",
            "../x",
            "a b",
            &"a".repeat(65),
        ] {
            assert!(validate_icon_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn documents_fail_deliberately_on_unknown_schema_and_malformed_data() {
        assert!(matches!(
            parse(r#"{"schemaVersion":2,"icons":{}}"#),
            Err(IconError::UnsupportedSchema(2))
        ));
        assert!(matches!(parse("{"), Err(IconError::Malformed(_))));
        assert!(matches!(
            parse(r#"{"schemaVersion":1,"icons":{},"extra":true}"#),
            Err(IconError::Malformed(_))
        ));
        assert!(matches!(
            parse(r#"{"schemaVersion":1,"icons":{"../x":"home"}}"#),
            Err(IconError::Malformed(_))
        ));
        assert!(matches!(
            parse(r#"{"schemaVersion":1,"icons":{"abc":"Not Valid"}}"#),
            Err(IconError::Malformed(_))
        ));
        let icons = parse(r#"{"schemaVersion":1,"icons":{"abc":"rocket_launch"}}"#).unwrap();
        assert_eq!(icons.get("abc").map(String::as_str), Some("rocket_launch"));
    }

    #[test]
    fn missing_document_is_empty_and_a_malformed_one_is_left_unchanged() {
        let root = std::env::temp_dir().join(format!("aurora-icons-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("instance-icons.json");
        assert!(load_from(&path).unwrap().is_empty());
        std::fs::write(&path, b"{not json").unwrap();
        assert!(load_from(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{not json");
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }
}
