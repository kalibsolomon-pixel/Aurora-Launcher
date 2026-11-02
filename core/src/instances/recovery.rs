//! Read-only failure diagnosis and explicit, metadata-only reconciliation of
//! local modpack edits. No content download, deletion, execution or relaunch.
use super::{InstanceId, InstanceRegistry, InstanceState, lifecycle};
use crate::{
    instance_content::{ContentState, ContentType},
    pack_state::{InstalledPack, PackDivergence},
    paths::ManagedPaths,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, io::Read, path::Path};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPreview {
    pub instance_id: String,
    pub summary: String,
    pub kept_settings: Vec<String>,
    pub removed_mods: Vec<String>,
    pub blockers: Vec<String>,
    pub can_apply: bool,
    pub fingerprint: String,
}
#[derive(Debug)]
pub struct RecoveryError(pub &'static str, pub String);
impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}
impl std::error::Error for RecoveryError {}
fn fail(e: impl std::fmt::Display) -> RecoveryError {
    RecoveryError("instance_recovery_unavailable", e.to_string())
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn file_hash(path: &Path) -> Result<String, RecoveryError> {
    let mut input = std::fs::File::open(path).map_err(fail)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).map_err(fail)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn settings_file(relative: &str, path: &Path, observed: &str) -> bool {
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    if relative != "options.txt"
        && (!relative.starts_with("config/")
            || !matches!(
                extension,
                "json" | "toml" | "properties" | "txt" | "cfg" | "ini" | "yaml" | "yml"
            ))
    {
        return false;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut bytes = Vec::new();
    if file
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > 4 * 1024 * 1024
        || hash(&bytes) != observed
        || std::str::from_utf8(&bytes).is_err()
    {
        return false;
    }
    extension != "json" || serde_json::from_slice::<serde_json::Value>(&bytes).is_ok()
}

struct Plan {
    preview: RecoveryPreview,
    old_pack: Option<InstalledPack>,
    next_pack: Option<InstalledPack>,
    old_content: ContentState,
    next_content: ContentState,
}

pub fn inspect(paths: &ManagedPaths, id: &InstanceId) -> Result<RecoveryPreview, RecoveryError> {
    crate::instance_content::with_instance_lock(id, || Ok(plan(paths, id).map(|p| p.preview)))
        .map_err(fail)?
}
fn plan(paths: &ManagedPaths, id: &InstanceId) -> Result<Plan, RecoveryError> {
    let root = crate::instance_content::validated_instance_root(paths, id).map_err(fail)?;
    let registry = InstanceRegistry::load(&paths.instance_registry_file()).map_err(fail)?;
    let record = registry
        .find(id)
        .ok_or_else(|| fail("Instance no longer exists."))?;
    let validation = lifecycle::validate_instance(paths, &registry, id).map_err(fail)?;
    let old_pack = InstalledPack::load(paths, id).map_err(fail)?;
    let old_content = ContentState::load(paths, id).map_err(fail)?;
    let mut next_content = old_content.clone();
    let mut next_pack = old_pack.clone();
    let mut report = RecoveryPreview {
        instance_id: id.to_string(),
        summary: String::new(),
        kept_settings: vec![],
        removed_mods: vec![],
        blockers: vec![],
        can_apply: false,
        fingerprint: String::new(),
    };
    for problem in validation.problems {
        if problem.component != "modpack" {
            report
                .blockers
                .push(format!("{}: {}", problem.component, problem.reason));
        }
    }
    if crate::launch::process::snapshot(id.as_str())
        .status
        .blocks_launch()
        || crate::launch::process::is_preparing(id.as_str())
    {
        report
            .blockers
            .push("Quit Minecraft and wait for launch preparation to finish.".into());
    }
    if let (Some(pack), Some(expected)) = (&mut next_pack, record.pack()) {
        if pack.identity.provider != expected.provider
            || pack.identity.project_id != expected.project_id
            || pack.identity.version_id != expected.version_id
            || pack.identity.name != expected.name
            || pack.identity.pack_version != expected.pack_version
            || pack.identity.minecraft_version != record.installed().minecraft_version
            || Some(pack.identity.fabric_loader_version.as_str())
                != record.installed().platform.version()
        {
            return Err(fail(
                "The pack and instance identities disagree; preserve their metadata for inspection.",
            ));
        }
        if pack.components.len() + pack.overrides.len() + pack.divergences.len() > 8192 {
            return Err(fail("Pack exceeds the recovery inspection limit."));
        }
        let mut keep = vec![];
        let mut removed = vec![];
        for (relative, expected, component) in pack
            .components
            .iter()
            .map(|c| (c.path.as_str(), c.sha256.as_str(), true))
            .chain(
                pack.overrides
                    .iter()
                    .map(|c| (c.path.as_str(), c.sha256.as_str(), false)),
            )
            .chain(
                pack.divergences
                    .iter()
                    .map(|c| (c.path.as_str(), c.local_sha256.as_str(), c.component)),
            )
        {
            let path = match crate::pack_state::safe_file(&root, relative) {
                Ok(path) => Some(path),
                Err(crate::pack_state::PackStateError::Io(e))
                    if e.kind() == std::io::ErrorKind::NotFound
                        && relative.starts_with("mods/")
                        && relative.ends_with(".jar") =>
                {
                    match crate::pack_state::safe_file(&root, &format!("{relative}.disabled")) {
                        Ok(path) => Some(path),
                        Err(crate::pack_state::PackStateError::Io(e))
                            if e.kind() == std::io::ErrorKind::NotFound =>
                        {
                            None
                        }
                        Err(e) => return Err(fail(e)),
                    }
                }
                Err(e) => {
                    report.blockers.push(format!("{relative}: {e}"));
                    continue;
                }
            };
            match path {
                None => {
                    removed.push(relative.to_owned());
                    report.removed_mods.push(relative.to_owned());
                }
                Some(path) => {
                    let observed = file_hash(&path)?;
                    if observed.eq_ignore_ascii_case(expected) {
                        continue;
                    }
                    if !component && settings_file(relative, &path, &observed) {
                        keep.push((relative.to_owned(), observed));
                        report.kept_settings.push(relative.to_owned());
                    } else {
                        report.blockers.push(format!("{relative}: installed bytes changed. Restore or review this file explicitly; recovery will not trust changed mod or resource artifacts."));
                    }
                }
            }
        }
        for relative in &removed {
            pack.components.retain(|c| c.path != *relative);
            pack.overrides.retain(|c| c.path != *relative);
            pack.divergences.retain(|c| c.path != *relative);
            pack.excluded_paths.push(relative.clone());
            let name = relative.strip_prefix("mods/").expect("inspected mod");
            next_content.entries.retain(|entry| {
                !(entry.content_type == ContentType::Mod
                    && entry.file_name.eq_ignore_ascii_case(name))
            });
        }
        for (relative, observed) in keep {
            if let Some(divergence) = pack.divergences.iter_mut().find(|d| d.path == relative) {
                divergence.local_sha256 = observed;
            } else {
                let authored = pack
                    .overrides
                    .iter()
                    .find(|o| o.path == relative)
                    .expect("inspected override");
                pack.divergences.push(PackDivergence {
                    path: relative.clone(),
                    component: false,
                    expected_sha256: Some(authored.sha256.clone()),
                    expected_sha512: None,
                    local_sha256: observed,
                    resolution: "keepLocal".into(),
                    recorded_at_unix_seconds: 0,
                });
                pack.overrides.retain(|o| o.path != relative);
            }
        }
        let identities: HashSet<_> = next_content.entries.iter().map(|r| r.identity()).collect();
        for entry in &mut next_content.entries {
            entry.requires.retain(|id| identities.contains(id));
        }
        if report.blockers.is_empty() {
            let mut inventory = crate::instance_mods::scan(paths, id).map_err(fail)?;
            crate::instance_mods::add_loader_builtins(paths, id, &mut inventory);
            report.blockers.extend(
                crate::mod_compatibility::validate(
                    &inventory,
                    &record.installed().minecraft_version,
                    record.installed().platform.version().unwrap_or(""),
                    None,
                    record.installed().platform.kind(),
                )
                .into_iter()
                .map(|p| p.message),
            );
        }
        if report.kept_settings.len() > 128 || report.removed_mods.len() > 128 {
            report.blockers.push(
                "Too many local changes for one recovery review. Inspect this pack manually."
                    .into(),
            );
        }
        report.can_apply = report.blockers.is_empty()
            && (record.state() == InstanceState::Installing
                || !report.kept_settings.is_empty()
                || !report.removed_mods.is_empty());
    } else {
        report.blockers.extend(if record.pack().is_some() { vec!["The installed pack marker is missing. Retry installation from the exact pack version.".into()] } else {vec!["Use the instance's installation or Java controls for this failure. No local pack metadata can be reconciled.".into()]});
    }
    report.kept_settings.sort();
    report.removed_mods.sort();
    report.blockers.sort();
    report.blockers.dedup();
    report.summary = if report.can_apply {
        format!(
            "Keep {} changed settings file(s), review {} removed mod(s), then verify the installation. No game files will be overwritten.",
            report.kept_settings.len(),
            report.removed_mods.len()
        )
    } else if report.blockers.is_empty() {
        "No recoverable pack inconsistency was found. The instance content is intact; review the launch diagnostic or Java settings.".into()
    } else {
        "Recovery found issues requiring attention. Your files remain unchanged.".into()
    };
    let manifest = crate::install::state::load_installed_state(paths.instance_paths(id).game())
        .map_err(fail)?;
    report.fingerprint=hash(&serde_json::to_vec(&serde_json::json!({"record":record,"pack":old_pack,"content":old_content,"report":report,"next":next_pack,"game":manifest})).map_err(fail)?);
    Ok(Plan {
        preview: report,
        old_pack,
        next_pack,
        old_content,
        next_content,
    })
}

/// Explicit approval changes receipts only, then promotes Ready after complete
/// verification. A stopped game and lifecycle/content locks exclude writers.
pub fn apply(
    paths: &ManagedPaths,
    id: &InstanceId,
    fingerprint: &str,
    keep_removed_mods: bool,
) -> Result<RecoveryPreview, RecoveryError> {
    apply_with_fault(paths, id, fingerprint, keep_removed_mods, false)
}
fn apply_with_fault(
    paths: &ManagedPaths,
    id: &InstanceId,
    fingerprint: &str,
    keep_removed_mods: bool,
    fail_commit: bool,
) -> Result<RecoveryPreview, RecoveryError> {
    crate::instance_content::with_instance_lock(id, || {
        Ok(apply_locked(
            paths,
            id,
            fingerprint,
            keep_removed_mods,
            fail_commit,
        ))
    })
    .map_err(fail)?
}
fn apply_locked(
    paths: &ManagedPaths,
    id: &InstanceId,
    fingerprint: &str,
    keep_removed_mods: bool,
    fail_commit: bool,
) -> Result<RecoveryPreview, RecoveryError> {
    let _operation = lifecycle::lock_instance_operation(paths, id).map_err(fail)?;
    let _registry = lifecycle::registry_lock();
    let plan = plan(paths, id)?;
    let _process = crate::launch::process::lock_stopped(id.as_str()).map_err(fail)?;
    if plan.preview.fingerprint != fingerprint {
        return Err(RecoveryError(
            "instance_recovery_stale",
            "Files or settings changed. Inspect recovery again before applying it.".into(),
        ));
    }
    if !plan.preview.can_apply {
        return Err(fail(plan.preview.blockers.join("\n")));
    }
    if !keep_removed_mods && !plan.preview.removed_mods.is_empty() {
        return Err(fail(
            "Review and confirm keeping the listed removed mods removed first.",
        ));
    }
    crate::instance_content::validated_instance_root(paths, id).map_err(fail)?;
    // Retain the derived platform path for APIs that compare against the managed
    // root; canonical Windows paths carry a different extended-path prefix.
    let root = paths.instance_paths(id).root().to_owned();
    let old_pack = plan.old_pack.as_ref().expect("reviewed pack");
    let mut next = plan.next_pack.clone().expect("reviewed pack");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    for divergence in &mut next.divergences {
        if plan.preview.kept_settings.contains(&divergence.path) {
            divergence.recorded_at_unix_seconds = now;
        }
    }
    let mut registry = InstanceRegistry::load(&paths.instance_registry_file()).map_err(fail)?;
    let previous_registry = registry.clone();
    let base = root.join(".smart-recovery");
    crate::install::coordination::ensure_directory(paths.data_root(), &base).map_err(fail)?;
    let backup = crate::install::coordination::ensure_directory(
        paths.data_root(),
        &base.join(uuid::Uuid::new_v4().simple().to_string()),
    )
    .map_err(fail)?;
    // Exact original metadata survives an interruption. No user/game bytes are
    // copied, overwritten, deleted, or used as executable recovery authority.
    for (name, path) in [
        ("pack-installed.json", root.join("pack-installed.json")),
        ("content-managed.json", root.join("content-managed.json")),
        ("instances.json", paths.instance_registry_file()),
    ] {
        use std::io::Write;
        crate::install::coordination::require_regular_or_missing(&path).map_err(fail)?;
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(fail(e)),
        };
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(backup.join(name))
            .map_err(fail)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(fail)?;
    }
    let mut content_changed = false;
    let mut pack_changed = false;
    let mut registry_changed = false;
    let result = (|| {
        registry
            .find_mut(id)
            .expect("registered")
            .set_state(InstanceState::Installing);
        registry
            .save(&paths.instance_registry_file())
            .map_err(fail)?;
        registry_changed = true;
        plan.next_content.save(paths, id).map_err(fail)?;
        content_changed = true;
        next.replace(paths, old_pack).map_err(fail)?;
        pack_changed = true;
        registry
            .find_mut(id)
            .expect("registered")
            .set_state(InstanceState::Ready);
        let validation = lifecycle::validate_instance(paths, &registry, id).map_err(fail)?;
        if validation.status != lifecycle::InstanceStatus::Ready {
            return Err(fail(format!(
                "Recovery did not pass verification: {}",
                validation
                    .problems
                    .iter()
                    .map(|p| format!("{}: {}", p.component, p.reason))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        for item in &next.divergences {
            let file = crate::pack_state::safe_file(&root, &item.path).map_err(fail)?;
            if file_hash(&file)? != item.local_sha256 {
                return Err(fail("Settings changed during recovery; inspect again."));
            }
        }
        if fail_commit {
            return Err(fail("injected registry commit failure"));
        }
        registry
            .save(&paths.instance_registry_file())
            .map_err(fail)?;
        let mut report = plan.preview.clone();
        report.can_apply = false;
        report.summary="Recovery verified the installation. Your settings and removed-mod choices were preserved. Play will check Java and account readiness again.".into();
        Ok(report)
    })();
    if result.is_err() {
        if pack_changed {
            old_pack.replace(paths, &next).map_err(fail)?;
        }
        if content_changed {
            plan.old_content.save(paths, id).map_err(fail)?;
        }
        if registry_changed {
            previous_registry
                .save(&paths.instance_registry_file())
                .map_err(fail)?;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance_content::{
        ContentCompatibility, ProviderOrigin, ProviderRecord, UpdateChannel,
    };
    use crate::instances::{PackRegistryIdentity, lifecycle::tests::SyntheticWorld};
    use crate::pack_state::{OwnedComponent, OwnedOverride, PackIdentity};
    use std::io::Write;
    async fn fixture(name: &str) -> (SyntheticWorld, InstanceId) {
        let world = SyntheticWorld::with_api(name, true);
        let record = world.create("Local pack edits").await.unwrap();
        let id = record.id().clone();
        let root = world.managed().instance_paths(&id).root().to_owned();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("config/settings.properties"), b"owner settings").unwrap();
        std::fs::write(root.join("user-sentinel"), b"owner data").unwrap();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"schemaVersion":1,"id":"clientsort","version":"1"}"#)
            .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let sha = hash(&bytes);
        let sha512 = format!("{:x}", sha2::Sha512::digest(&bytes));
        let mut content = ContentState::empty();
        content.entries.push(ProviderRecord {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: "SORTMOD1".into(),
            version_id: "SORTVER1".into(),
            file_id: sha512.clone(),
            file_name: "clientsort.jar".into(),
            sha256: sha.clone(),
            display_version: Some("1".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["26.2".into()],
                loader: Some("fabric".into()),
                environment: Some("client".into()),
            },
            dependencies: vec![],
            explicitly_retained: false,
            requires: vec![],
            origin: ProviderOrigin::Pack,
            installed_at_unix_seconds: None,
            pinned: false,
            update_channel: UpdateChannel::Stable,
        });
        content.save(&world.managed(), &id).unwrap();
        let pack = InstalledPack::new(
            id.clone(),
            PackIdentity {
                provider: "modrinth".into(),
                project_id: "PACK0001".into(),
                version_id: "PACKVER1".into(),
                name: "Pack".into(),
                pack_version: "1".into(),
                artifact_sha512: "a".repeat(128),
                artifact_sha256: "b".repeat(64),
                minecraft_version: "26.2".into(),
                fabric_loader_version: "0.19.5".into(),
                installed_at_unix_seconds: 1,
            },
            vec![OwnedComponent {
                path: "mods/clientsort.jar".into(),
                sha256: sha,
                sha512,
                provider: Some(content.entries[0].identity()),
                provider_version_id: Some("SORTVER1".into()),
            }],
            vec![OwnedOverride {
                path: "config/settings.properties".into(),
                sha256: hash(b"pack defaults"),
            }],
            vec![],
        )
        .unwrap();
        pack.save(&world.managed()).unwrap();
        let mut registry =
            InstanceRegistry::load(&world.managed().instance_registry_file()).unwrap();
        let record = registry.find_mut(&id).unwrap();
        record
            .set_pack(PackRegistryIdentity {
                provider: "modrinth".into(),
                project_id: "PACK0001".into(),
                version_id: "PACKVER1".into(),
                name: "Pack".into(),
                pack_version: "1".into(),
            })
            .unwrap();
        record.set_state(InstanceState::Installing);
        registry
            .save(&world.managed().instance_registry_file())
            .unwrap();
        (world, id)
    }
    #[tokio::test]
    async fn recovery_keeps_settings_and_removed_mod_choice_and_validates_before_ready() {
        let (world, id) = fixture("smart-recovery-good").await;
        let root = world.managed().instance_paths(&id).root().to_owned();
        let registry_before = std::fs::read(world.managed().instance_registry_file()).unwrap();
        let pack_before = std::fs::read(root.join("pack-installed.json")).unwrap();
        let preview = inspect(&world.managed(), &id).unwrap();
        assert!(preview.can_apply, "{:?}", preview.blockers);
        assert_eq!(preview.kept_settings, ["config/settings.properties"]);
        assert_eq!(preview.removed_mods, ["mods/clientsort.jar"]);
        assert!(apply(&world.managed(), &id, &preview.fingerprint, false).is_err());
        assert_eq!(
            std::fs::read(root.join("pack-installed.json")).unwrap(),
            pack_before
        );
        apply(&world.managed(), &id, &preview.fingerprint, true).unwrap();
        assert_eq!(
            std::fs::read(root.join("config/settings.properties")).unwrap(),
            b"owner settings"
        );
        assert_eq!(
            std::fs::read(root.join("user-sentinel")).unwrap(),
            b"owner data"
        );
        assert!(!root.join("mods/clientsort.jar").exists());
        let pack = InstalledPack::load(&world.managed(), &id).unwrap().unwrap();
        assert_eq!(pack.excluded_paths, ["mods/clientsort.jar"]);
        assert_eq!(
            pack.divergences[0].expected_sha256,
            Some(hash(b"pack defaults"))
        );
        assert_eq!(pack.divergences[0].local_sha256, hash(b"owner settings"));
        assert!(pack.divergences[0].recorded_at_unix_seconds > 0);
        assert!(
            ContentState::load(&world.managed(), &id)
                .unwrap()
                .entries
                .is_empty()
        );
        let registry = InstanceRegistry::load(&world.managed().instance_registry_file()).unwrap();
        assert_eq!(
            lifecycle::validate_instance(&world.managed(), &registry, &id)
                .unwrap()
                .status,
            lifecycle::InstanceStatus::Ready
        );
        let backup = std::fs::read_dir(root.join(".smart-recovery"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            std::fs::read(backup.join("instances.json")).unwrap(),
            registry_before
        );
        assert_eq!(
            std::fs::read(backup.join("pack-installed.json")).unwrap(),
            pack_before
        );
        assert!(!inspect(&world.managed(), &id).unwrap().can_apply);
    }
    #[tokio::test]
    async fn stale_settings_and_running_or_preparing_game_cannot_apply() {
        let (world, id) = fixture("smart-recovery-stale").await;
        let preview = inspect(&world.managed(), &id).unwrap();
        let root = world.managed().instance_paths(&id).root().to_owned();
        std::fs::write(root.join("config/settings.properties"), b"new edit").unwrap();
        assert_eq!(
            apply(&world.managed(), &id, &preview.fingerprint, true)
                .unwrap_err()
                .0,
            "instance_recovery_stale"
        );
        assert!(!root.join(".smart-recovery").exists());
        let running = crate::launch::process::force_running(id.as_str());
        assert!(!inspect(&world.managed(), &id).unwrap().can_apply);
        drop(running);
        let preparing = crate::launch::process::PreparationGuard::acquire(id.as_str()).unwrap();
        assert!(!inspect(&world.managed(), &id).unwrap().can_apply);
        drop(preparing);
    }
    #[tokio::test]
    async fn failed_commit_restores_metadata_and_keeps_backups_and_user_bytes() {
        let (world, id) = fixture("smart-recovery-rollback").await;
        let root = world.managed().instance_paths(&id).root().to_owned();
        let pack = std::fs::read(root.join("pack-installed.json")).unwrap();
        let content = std::fs::read(root.join("content-managed.json")).unwrap();
        let registry = std::fs::read(world.managed().instance_registry_file()).unwrap();
        let preview = inspect(&world.managed(), &id).unwrap();
        assert!(apply_with_fault(&world.managed(), &id, &preview.fingerprint, true, true).is_err());
        assert_eq!(
            std::fs::read(root.join("pack-installed.json")).unwrap(),
            pack
        );
        assert_eq!(
            std::fs::read(root.join("content-managed.json")).unwrap(),
            content
        );
        assert_eq!(
            std::fs::read(world.managed().instance_registry_file()).unwrap(),
            registry
        );
        assert_eq!(
            std::fs::read(root.join("config/settings.properties")).unwrap(),
            b"owner settings"
        );
        assert_eq!(
            std::fs::read_dir(root.join(".smart-recovery"))
                .unwrap()
                .count(),
            1
        );
    }
    #[tokio::test]
    async fn missing_required_dependency_changed_mod_and_unknown_schema_stay_blocked() {
        let (world, id) = fixture("smart-recovery-blocked").await;
        let root = world.managed().instance_paths(&id).root().to_owned();
        let parent = root.join("mods/parent.jar");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&parent).unwrap());
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(
            br#"{"schemaVersion":1,"id":"parent","version":"1","depends":{"clientsort":"*"}}"#,
        )
        .unwrap();
        zip.finish().unwrap();
        assert!(!inspect(&world.managed(), &id).unwrap().can_apply);
        std::fs::remove_file(parent).unwrap();
        std::fs::write(
            root.join("mods/clientsort.jar"),
            b"changed unverified bytes",
        )
        .unwrap();
        assert!(!inspect(&world.managed(), &id).unwrap().can_apply);
        let future = b"{\"schemaVersion\":999}";
        std::fs::write(root.join("pack-installed.json"), future).unwrap();
        assert!(inspect(&world.managed(), &id).is_err());
        assert_eq!(
            std::fs::read(root.join("pack-installed.json")).unwrap(),
            future
        );
    }
    #[tokio::test]
    async fn interrupted_receipt_reconciliation_can_finish_without_touching_user_files() {
        let (world, id) = fixture("smart-recovery-resume").await;
        let first = inspect(world.managed(), &id).unwrap();
        apply(world.managed(), &id, &first.fingerprint, true).unwrap();
        let mut registry =
            InstanceRegistry::load(&world.managed().instance_registry_file()).unwrap();
        registry
            .find_mut(&id)
            .unwrap()
            .set_state(InstanceState::Installing);
        registry
            .save(&world.managed().instance_registry_file())
            .unwrap();
        let resume = inspect(world.managed(), &id).unwrap();
        assert!(resume.can_apply);
        assert!(resume.kept_settings.is_empty() && resume.removed_mods.is_empty());
        apply(world.managed(), &id, &resume.fingerprint, false).unwrap();
        let registry = InstanceRegistry::load(&world.managed().instance_registry_file()).unwrap();
        assert_eq!(
            lifecycle::validate_instance(world.managed(), &registry, &id)
                .unwrap()
                .status,
            lifecycle::InstanceStatus::Ready
        );
    }
    #[tokio::test]
    async fn invalid_json_settings_are_not_automatically_accepted() {
        let (world, id) = fixture("smart-recovery-json").await;
        let root = world.managed().instance_paths(&id).root().to_owned();
        let old = InstalledPack::load(world.managed(), &id).unwrap().unwrap();
        let mut changed = old.clone();
        changed.overrides.push(OwnedOverride {
            path: "config/bad.json".into(),
            sha256: hash(b"{}"),
        });
        changed.replace(world.managed(), &old).unwrap();
        std::fs::write(root.join("config/bad.json"), b"{bad JSON").unwrap();
        let preview = inspect(world.managed(), &id).unwrap();
        assert!(!preview.can_apply);
        assert!(
            preview
                .blockers
                .iter()
                .any(|b| b.contains("config/bad.json"))
        );
        assert!(!root.join(".smart-recovery").exists());
    }
}
