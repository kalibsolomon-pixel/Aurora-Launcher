//! Launcher-side Pulsar fight annotations (bookmarks, tags, and notes).
//!
//! Stored in `launcher/pulsar-annotations.json` (schema 1) so immutable
//! Aurora Client recording files are never modified.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::Error;
use super::view::FightKey;
use crate::paths::ManagedPaths;

pub const ANNOTATIONS_SCHEMA_VERSION: u32 = 1;
const MAX_TAGS: usize = 12;
const MAX_TAG_LEN: usize = 32;
const MAX_NOTE_LEN: usize = 1000;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FightAnnotation {
    #[serde(default)]
    pub bookmarked: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnnotationsStore {
    pub schema_version: u32,
    #[serde(default)]
    pub fights: BTreeMap<String, FightAnnotation>,
}

impl Default for AnnotationsStore {
    fn default() -> Self {
        Self {
            schema_version: ANNOTATIONS_SCHEMA_VERSION,
            fights: BTreeMap::new(),
        }
    }
}

pub fn composite_key(key: &FightKey) -> String {
    format!("{}:{}", key.instance_id, key.fight_id)
}

fn path(managed: &ManagedPaths) -> PathBuf {
    managed.launcher_dir().join("pulsar-annotations.json")
}

pub fn load(managed: &ManagedPaths) -> Result<AnnotationsStore, Error> {
    let text = match fs::read_to_string(path(managed)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AnnotationsStore::default());
        }
        Err(e) => {
            return Err(Error::new(
                "pulsar_annotations_io",
                format!("Could not read Pulsar annotations: {e}"),
            ));
        }
    };
    let store: AnnotationsStore = serde_json::from_str(&text).map_err(|e| {
        Error::new(
            "pulsar_annotations_malformed",
            format!("Pulsar annotations file is malformed: {e}"),
        )
    })?;
    if store.schema_version != ANNOTATIONS_SCHEMA_VERSION {
        return Err(Error::new(
            "pulsar_annotations_unsupported_schema",
            format!(
                "Unsupported Pulsar annotations schema {}.",
                store.schema_version
            ),
        ));
    }
    Ok(store)
}

pub fn save(managed: &ManagedPaths, store: &AnnotationsStore) -> Result<(), Error> {
    let file_path = path(managed);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| Error::new("pulsar_annotations_io", e.to_string()))?;
    }
    let tmp = file_path.with_extension(format!("json.tmp-{}", uuid::Uuid::new_v4().simple()));
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| Error::new("pulsar_annotations_io", e.to_string()))?;
    fs::write(&tmp, format!("{json}\n"))
        .map_err(|e| Error::new("pulsar_annotations_io", e.to_string()))?;
    fs::rename(&tmp, &file_path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::new("pulsar_annotations_io", e.to_string())
    })
}

pub fn set_annotation(
    managed: &ManagedPaths,
    key: &FightKey,
    bookmarked: bool,
    tags: &[String],
    note: &str,
) -> Result<FightAnnotation, Error> {
    let mut clean_tags = Vec::new();
    for t in tags {
        let trimmed = t.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_TAG_LEN || trimmed.chars().any(char::is_control) {
            return Err(Error::new(
                "pulsar_annotation_invalid",
                "Tags must be at most 32 visible characters.",
            ));
        }
        if !clean_tags
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(trimmed))
        {
            clean_tags.push(trimmed.to_owned());
        }
        if clean_tags.len() > MAX_TAGS {
            return Err(Error::new(
                "pulsar_annotation_invalid",
                "A fight can have at most 12 tags.",
            ));
        }
    }
    let clean_note: String = note
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(MAX_NOTE_LEN)
        .collect::<String>()
        .trim()
        .to_owned();

    let mut store = load(managed)?;
    let entry = FightAnnotation {
        bookmarked,
        tags: clean_tags,
        note: clean_note,
    };
    let k = composite_key(key);
    if !entry.bookmarked && entry.tags.is_empty() && entry.note.is_empty() {
        store.fights.remove(&k);
    } else {
        store.fights.insert(k, entry.clone());
    }
    save(managed, &store)?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_fight_annotations() {
        let root = std::env::temp_dir().join(format!("aurora-pulsar-ann-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let key = FightKey {
            instance_id: "inst-1".into(),
            fight_id: "fight-42".into(),
        };
        let saved = set_annotation(
            &managed,
            &key,
            true,
            &["Ranked".into(), " ranked ".into(), "Axe".into()],
            "Good shield disable punish",
        )
        .unwrap();
        assert!(saved.bookmarked);
        assert_eq!(saved.tags, vec!["Ranked", "Axe"]);
        assert_eq!(saved.note, "Good shield disable punish");

        let reloaded = load(&managed).unwrap();
        assert_eq!(reloaded.fights.get(&composite_key(&key)), Some(&saved));
        let _ = fs::remove_dir_all(&root);
    }
}
