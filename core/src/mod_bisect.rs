//! Built-in binary-search ("Mod Bisect") crash troubleshooter.
//!
//! Persists a schema-1 session file (`instances/<id>/.mod-bisect.json`) that
//! tracks the user's original enabled mod set and splits toggleable mods in
//! half across launch attempts. Launcher-protected mods (Aurora Client,
//! bootstrap Fabric API) are never disabled. Every mutation reuses
//! `instance_mods::set_mod_enabled` so file ownership and naming rules remain
//! authoritative.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::instance_mods;
use crate::instances::InstanceId;
use crate::paths::ManagedPaths;

pub const BISECT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BisectStatus {
    InProgress,
    FoundCulprit,
    NoCulpritFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BisectStepSnapshot {
    suspects: Vec<String>,
    known_good: Vec<String>,
    active_test_group: Vec<String>,
    step: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BisectSession {
    pub schema_version: u32,
    pub instance_id: String,
    pub status: BisectStatus,
    /// Original `enabled` state of every toggleable mod when bisect started.
    pub original_enabled: BTreeMap<String, bool>,
    /// Display names keyed by mod entry ID (`base_file_name`).
    pub display_names: BTreeMap<String, String>,
    /// Candidate mods that may contain the crashing mod.
    pub suspects: Vec<String>,
    /// Mods proven not to cause the crash on their own.
    pub known_good: Vec<String>,
    /// The half currently enabled for this test launch.
    pub active_test_group: Vec<String>,
    pub step: u32,
    pub estimated_total_steps: u32,
    #[serde(default)]
    pub culprit: Option<String>,
    #[serde(default)]
    history: Vec<BisectStepSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BisectView {
    pub status: BisectStatus,
    pub step: u32,
    pub estimated_total_steps: u32,
    pub remaining_suspects: usize,
    pub active_test_count: usize,
    pub can_undo: bool,
    pub culprit_id: Option<String>,
    pub culprit_name: Option<String>,
    pub active_test_names: Vec<String>,
}

impl BisectSession {
    pub fn to_view(&self) -> BisectView {
        let culprit_name = self.culprit.as_ref().map(|id| {
            self.display_names
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.clone())
        });
        let active_test_names = self
            .active_test_group
            .iter()
            .take(12)
            .map(|id| {
                self.display_names
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| id.clone())
            })
            .collect();
        BisectView {
            status: self.status,
            step: self.step,
            estimated_total_steps: self.estimated_total_steps,
            remaining_suspects: self.suspects.len(),
            active_test_count: self.active_test_group.len(),
            can_undo: !self.history.is_empty(),
            culprit_id: self.culprit.clone(),
            culprit_name,
            active_test_names,
        }
    }
}

#[derive(Debug)]
pub struct BisectError(pub &'static str, pub String);

impl std::fmt::Display for BisectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}

impl std::error::Error for BisectError {}

fn session_path(managed: &ManagedPaths, id: &InstanceId) -> PathBuf {
    managed.instance_paths(id).root().join(".mod-bisect.json")
}

pub fn load(managed: &ManagedPaths, id: &InstanceId) -> Result<Option<BisectSession>, BisectError> {
    let path = session_path(managed, id);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(BisectError(
                "bisect_io_error",
                format!("Could not read bisect session: {e}"),
            ));
        }
    };
    let session: BisectSession = serde_json::from_str(&text).map_err(|e| {
        BisectError(
            "bisect_malformed",
            format!("The bisect session file is malformed: {e}"),
        )
    })?;
    if session.schema_version != BISECT_SCHEMA_VERSION {
        return Err(BisectError(
            "bisect_unsupported_schema",
            format!(
                "Unsupported bisect session schema {}.",
                session.schema_version
            ),
        ));
    }
    Ok(Some(session))
}

fn save(
    managed: &ManagedPaths,
    id: &InstanceId,
    session: &BisectSession,
) -> Result<(), BisectError> {
    let path = session_path(managed, id);
    let tmp = path.with_extension(format!("json.tmp-{}", uuid::Uuid::new_v4().simple()));
    let json = serde_json::to_string_pretty(session)
        .map_err(|e| BisectError("bisect_io_error", e.to_string()))?;
    fs::write(&tmp, format!("{json}\n"))
        .map_err(|e| BisectError("bisect_io_error", e.to_string()))?;
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        BisectError("bisect_io_error", e.to_string())
    })
}

fn delete_session_file(managed: &ManagedPaths, id: &InstanceId) -> Result<(), BisectError> {
    let path = session_path(managed, id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(BisectError("bisect_io_error", e.to_string())),
    }
}

fn ceil_log2(n: usize) -> u32 {
    if n <= 1 {
        0
    } else {
        (usize::BITS - (n - 1).leading_zeros()) as u32
    }
}

fn base_file_name(file_name: &str) -> &str {
    file_name.strip_suffix(".disabled").unwrap_or(file_name)
}

/// Applies the desired enabled map (keyed by canonical base filename without `.disabled`)
/// to all toggleable mods in the instance.
fn apply_mod_states(
    managed: &ManagedPaths,
    id: &InstanceId,
    desired: &BTreeMap<String, bool>,
) -> Result<(), BisectError> {
    let inventory = instance_mods::scan(managed, id).map_err(|e| {
        BisectError(
            "bisect_mod_error",
            format!("Could not inspect instance mods: {e}"),
        )
    })?;
    for entry in &inventory.entries {
        if !entry.can_toggle {
            continue;
        }
        let key = base_file_name(&entry.file_name);
        let Some(&want) = desired.get(key) else {
            continue;
        };
        if entry.enabled != want {
            instance_mods::set_enabled(managed, id, &entry.entry_id, want).map_err(|e| {
                BisectError(
                    "bisect_mod_error",
                    format!("Could not toggle mod '{}': {e}", entry.file_name),
                )
            })?;
        }
    }
    Ok(())
}

fn apply_test_group(
    managed: &ManagedPaths,
    id: &InstanceId,
    session: &BisectSession,
) -> Result<(), BisectError> {
    let mut desired = BTreeMap::new();
    for mod_id in session.original_enabled.keys() {
        let enabled =
            session.active_test_group.contains(mod_id) || session.known_good.contains(mod_id);
        desired.insert(mod_id.clone(), enabled);
    }
    apply_mod_states(managed, id, &desired)
}

pub fn start(managed: &ManagedPaths, id: &InstanceId) -> Result<BisectSession, BisectError> {
    if load(managed, id)?.is_some() {
        return Err(BisectError(
            "bisect_already_active",
            "A mod bisect session is already active for this instance.".into(),
        ));
    }
    let inventory = instance_mods::scan(managed, id).map_err(|e| {
        BisectError(
            "bisect_mod_error",
            format!("Could not inspect instance mods: {e}"),
        )
    })?;
    let mut original_enabled = BTreeMap::new();
    let mut display_names = BTreeMap::new();
    let mut suspects = Vec::new();
    for entry in &inventory.entries {
        if !entry.can_toggle {
            continue;
        }
        let key = base_file_name(&entry.file_name).to_owned();
        original_enabled.insert(key.clone(), entry.enabled);
        display_names.insert(key.clone(), entry.display_name.clone());
        if entry.enabled {
            suspects.push(key);
        }
    }
    if suspects.len() < 2 {
        return Err(BisectError(
            "bisect_insufficient_mods",
            "Mod bisect requires at least 2 enabled user mods.".into(),
        ));
    }
    let mid = suspects.len() / 2;
    let active_test_group = suspects[..mid].to_vec();
    let estimated_total_steps = ceil_log2(suspects.len()).max(1);
    let session = BisectSession {
        schema_version: BISECT_SCHEMA_VERSION,
        instance_id: id.as_str().to_owned(),
        status: BisectStatus::InProgress,
        original_enabled,
        display_names,
        suspects,
        known_good: Vec::new(),
        active_test_group,
        step: 1,
        estimated_total_steps,
        culprit: None,
        history: Vec::new(),
    };
    apply_test_group(managed, id, &session)?;
    save(managed, id, &session)?;
    Ok(session)
}

/// Records the outcome of the current bisect launch (`crashed = true` means the
/// fault reproduced with `active_test_group` enabled; `crashed = false` means
/// the fault did NOT occur, so the culprit is in the disabled half).
pub fn record_outcome(
    managed: &ManagedPaths,
    id: &InstanceId,
    crashed: bool,
) -> Result<BisectSession, BisectError> {
    let mut session = load(managed, id)?.ok_or_else(|| {
        BisectError(
            "bisect_not_active",
            "No mod bisect session is active for this instance.".into(),
        )
    })?;
    if session.status != BisectStatus::InProgress {
        return Err(BisectError(
            "bisect_already_finished",
            "This bisect session has already finished.".into(),
        ));
    }
    session.history.push(BisectStepSnapshot {
        suspects: session.suspects.clone(),
        known_good: session.known_good.clone(),
        active_test_group: session.active_test_group.clone(),
        step: session.step,
    });

    let other_half: Vec<String> = session
        .suspects
        .iter()
        .filter(|m| !session.active_test_group.contains(m))
        .cloned()
        .collect();

    if crashed {
        // Culprit is inside active_test_group; other_half is cleared from suspects.
        session.suspects = session.active_test_group.clone();
    } else {
        // active_test_group did not crash, so mark them known_good and test other_half.
        session.known_good.extend(session.active_test_group.clone());
        session.suspects = other_half;
    }

    if session.suspects.len() == 1 {
        session.status = BisectStatus::FoundCulprit;
        session.culprit = Some(session.suspects[0].clone());
        session.active_test_group.clear();
        // Leave all original mods enabled EXCEPT the culprit so user can verify immediately.
        let mut desired = session.original_enabled.clone();
        if let Some(culprit) = &session.culprit {
            desired.insert(culprit.clone(), false);
        }
        apply_mod_states(managed, id, &desired)?;
    } else if session.suspects.is_empty() {
        session.status = BisectStatus::NoCulpritFound;
        session.active_test_group.clear();
        apply_mod_states(managed, id, &session.original_enabled)?;
    } else {
        session.step += 1;
        let mid = session.suspects.len() / 2;
        session.active_test_group = session.suspects[..mid].to_vec();
        apply_test_group(managed, id, &session)?;
    }

    save(managed, id, &session)?;
    Ok(session)
}

pub fn undo(managed: &ManagedPaths, id: &InstanceId) -> Result<BisectSession, BisectError> {
    let mut session = load(managed, id)?.ok_or_else(|| {
        BisectError(
            "bisect_not_active",
            "No mod bisect session is active for this instance.".into(),
        )
    })?;
    let prev = session.history.pop().ok_or_else(|| {
        BisectError(
            "bisect_cannot_undo",
            "There is no previous step to undo.".into(),
        )
    })?;
    session.status = BisectStatus::InProgress;
    session.culprit = None;
    session.suspects = prev.suspects;
    session.known_good = prev.known_good;
    session.active_test_group = prev.active_test_group;
    session.step = prev.step;
    apply_test_group(managed, id, &session)?;
    save(managed, id, &session)?;
    Ok(session)
}

/// Ends the bisect session. When `keep_culprit_disabled` is true and a culprit
/// was found, restores all other original mods while keeping the culprit
/// disabled; otherwise restores every mod to its exact original state.
pub fn finish(
    managed: &ManagedPaths,
    id: &InstanceId,
    keep_culprit_disabled: bool,
) -> Result<(), BisectError> {
    let Some(session) = load(managed, id)? else {
        return Ok(());
    };
    let mut desired = session.original_enabled.clone();
    if keep_culprit_disabled && let Some(culprit) = &session.culprit {
        desired.insert(culprit.clone(), false);
    }
    apply_mod_states(managed, id, &desired)?;
    delete_session_file(managed, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_fake_mod(dir: &std::path::Path, name: &str) {
        let path = dir.join(name);
        let file = fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("fabric.mod.json", opts).unwrap();
        let id = name.trim_end_matches(".jar");
        let meta = format!(r#"{{"schemaVersion":1,"id":"{id}","version":"1.0.0","name":"{id}"}}"#);
        zip.write_all(meta.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn binary_search_identifies_culprit_and_restores_cleanly() {
        let root =
            std::env::temp_dir().join(format!("aurora-bisect-test-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let id = crate::instances::generate_instance_id();
        let mods_dir = managed.instance_paths(&id).mods().to_path_buf();
        fs::create_dir_all(&mods_dir).unwrap();
        for name in ["alpha.jar", "beta.jar", "gamma.jar", "delta.jar"] {
            write_fake_mod(&mods_dir, name);
        }

        let mut session = start(&managed, &id).unwrap();
        assert_eq!(session.status, BisectStatus::InProgress);
        assert_eq!(session.suspects.len(), 4);
        assert_eq!(session.active_test_group.len(), 2);

        let gamma_id = session
            .display_names
            .iter()
            .find(|(_, name)| name.as_str() == "gamma")
            .map(|(k, _)| k.clone())
            .expect("gamma mod present");

        // Pretend "gamma" is the crashing mod.
        while session.status == BisectStatus::InProgress {
            let crashed = session.active_test_group.iter().any(|m| m == &gamma_id);
            session = record_outcome(&managed, &id, crashed).unwrap();
        }
        assert_eq!(session.status, BisectStatus::FoundCulprit);
        assert_eq!(session.culprit.as_deref(), Some(gamma_id.as_str()));

        // Undo brings us back to InProgress.
        let undone = undo(&managed, &id).unwrap();
        assert_eq!(undone.status, BisectStatus::InProgress);

        // Re-mark and finish keeping culprit disabled.
        let crashed = undone.active_test_group.iter().any(|m| m == &gamma_id);
        let found = record_outcome(&managed, &id, crashed).unwrap();
        assert_eq!(found.culprit.as_deref(), Some(gamma_id.as_str()));

        finish(&managed, &id, true).unwrap();
        assert!(load(&managed, &id).unwrap().is_none());
        let inv = instance_mods::scan(&managed, &id).unwrap();
        for entry in inv.entries {
            if base_file_name(&entry.file_name) == gamma_id {
                assert!(!entry.enabled);
            } else {
                assert!(entry.enabled);
            }
        }
        let _ = fs::remove_dir_all(&root);
    }
}
