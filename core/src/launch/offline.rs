//! Normalized, secret-free offline launch plan receipt.
//!
//! Saved alongside a validated ready instance (`instances/<id>/.offline-launch-plan.json`)
//! whenever an online launch resolves a `GameInstallPlan`. Contains ZERO tokens,
//! credentials, or session data — only the normalized `LaunchPlan` and Java
//! runtime component bound to the exact SHA-256 digest of `game/installed-game.json`.
//!
//! Offline Play re-verifies the installed game bytes, the manifest digest
//! binding, and the local Java executable before launching with an explicit
//! offline session for a previously entitled Microsoft account.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::game::plan::GameInstallPlan;
use crate::instances::InstanceId;
use crate::launch::resolve::LaunchPlan;
use crate::minecraft::rules::PlatformProfile;
use crate::paths::ManagedPaths;

pub const OFFLINE_PLAN_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfflinePlanReceipt {
    pub schema_version: u32,
    pub instance_id: String,
    /// Hex SHA-256 of `game/installed-game.json` at the time this plan was recorded.
    pub installed_manifest_sha256: String,
    /// Required Mojang Java runtime component (e.g., `java-runtime-delta`).
    pub runtime_component: String,
    pub launch_plan: LaunchPlan,
}

fn receipt_path(managed: &ManagedPaths, id: &InstanceId) -> PathBuf {
    managed
        .instance_paths(id)
        .root()
        .join(".offline-launch-plan.json")
}

fn manifest_digest(managed: &ManagedPaths, id: &InstanceId) -> Option<String> {
    let manifest_path = managed
        .instance_paths(id)
        .game()
        .join("installed-game.json");
    let bytes = fs::read(manifest_path).ok()?;
    Some(format!("{:x}", Sha256::digest(&bytes)))
}

/// Persists a verified, secret-free offline launch receipt bound to the
/// current `installed-game.json` bytes.
pub fn save_receipt(
    managed: &ManagedPaths,
    id: &InstanceId,
    plan: &GameInstallPlan,
    platform: PlatformProfile,
) -> std::io::Result<()> {
    let Some(digest) = manifest_digest(managed, id) else {
        return Ok(());
    };
    let receipt = OfflinePlanReceipt {
        schema_version: OFFLINE_PLAN_SCHEMA_VERSION,
        instance_id: id.as_str().to_owned(),
        installed_manifest_sha256: digest,
        runtime_component: plan.java().component().to_owned(),
        launch_plan: LaunchPlan::from_game_plan(plan, platform),
    };
    let path = receipt_path(managed, id);
    let tmp = path.with_extension(format!("json.tmp-{}", uuid::Uuid::new_v4().simple()));
    let json = serde_json::to_string_pretty(&receipt)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(&tmp, format!("{json}\n"))?;
    if let Err(err) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(())
}

/// Loads and verifies an offline launch receipt against the current instance
/// ID and `installed-game.json` SHA-256 digest. Returns `None` if absent,
/// mismatched, malformed, or unsupported.
pub fn load_verified_receipt(
    managed: &ManagedPaths,
    id: &InstanceId,
) -> Option<OfflinePlanReceipt> {
    let expected_digest = manifest_digest(managed, id)?;
    let text = fs::read_to_string(receipt_path(managed, id)).ok()?;
    let receipt: OfflinePlanReceipt = serde_json::from_str(&text).ok()?;
    if receipt.schema_version != OFFLINE_PLAN_SCHEMA_VERSION
        || receipt.instance_id != id.as_str()
        || receipt.installed_manifest_sha256 != expected_digest
    {
        return None;
    }
    Some(receipt)
}

pub fn has_valid_receipt(managed: &ManagedPaths, id: &InstanceId) -> bool {
    load_verified_receipt(managed, id).is_some()
}
