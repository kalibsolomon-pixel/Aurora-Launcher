//! One lazy, non-overlapping minute worker for the explicitly selected pool.
//! Observations contain value hashes only. No timestamp chooses a winner.
use super::*;
use crate::host::Host;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    pub running: bool,
    pub last_run_unix_seconds: Option<u64>,
    pub files_changed: usize,
    pub conflicts_kept: usize,
    pub skipped_instances: usize,
    pub message: String,
    pub files_scanned: u64,
    pub files_hashed: u64,
    pub bytes_copied: u64,
    pub cycle_micros: u64,
}
#[derive(Default)]
struct Manager {
    generation: u64,
    wake: Option<tokio::sync::watch::Sender<()>>,
    status: Status,
}
fn manager() -> &'static Mutex<Manager> {
    static STATE: OnceLock<Mutex<Manager>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}
pub fn status() -> Status {
    manager().lock().expect("sync manager").status.clone()
}
#[cfg(test)]
pub(super) fn worker_active() -> bool {
    manager().lock().expect("sync manager").wake.is_some()
}

pub fn reconcile(host: Host, managed: ManagedPaths) -> Result<Status, SyncError> {
    let policy = policy(&managed)?;
    let eligible = policy.enabled && policy.instances.len() >= 2 && policy.has_content();
    let mut state = manager().lock().expect("sync manager");
    state.status.enabled = policy.enabled;
    if let Some(wake) = &state.wake {
        let _ = wake.send(());
        return Ok(state.status.clone());
    }
    if !eligible {
        state.status.message = if policy.enabled {
            "Select at least two instances and content to sync."
        } else {
            "Automatic syncing is off."
        }
        .into();
        return Ok(state.status.clone());
    }
    state.generation += 1;
    let generation = state.generation;
    let (wake, receiver) = tokio::sync::watch::channel(());
    state.wake = Some(wake);
    state.status.message = "Selected instances will sync every minute while stopped.".into();
    let task_host = host.clone();
    host.spawn(async move {
        worker(task_host, managed, receiver, generation).await;
    });
    Ok(state.status.clone())
}

async fn worker(
    host: Host,
    managed: ManagedPaths,
    mut wake: tokio::sync::watch::Receiver<()>,
    generation: u64,
) {
    let mut engine = Engine::default();
    let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + INTERVAL, INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        match policy(&managed) {
            Ok(p) if p.enabled && p.instances.len() >= 2 && p.has_content() => {}
            _ => break,
        }
        tokio::select! {
            _ = interval.tick() => {},
            changed = wake.changed() => { if changed.is_err() { break; } interval.reset(); continue; },
        }
        {
            let mut state = manager().lock().expect("sync manager");
            if state.generation != generation {
                return;
            }
            state.status.running = true;
            let _ = host.emit("instance-sync-state", &state.status);
        }
        let paths = managed.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let result = engine.cycle(&paths);
            (engine, result)
        })
        .await;
        let next = match outcome {
            Ok((returned, result)) => {
                engine = returned;
                result.unwrap_or_else(|error| Status {
                    message: error.message.into(),
                    ..Status::default()
                })
            }
            Err(_) => {
                engine = Engine::default();
                Status { message:"Automatic sync could not complete this minute; no unsafe retry was attempted.".into(), ..Status::default() }
            }
        };
        let mut state = manager().lock().expect("sync manager");
        if state.generation != generation {
            return;
        }
        state.status = Status {
            enabled: policy(&managed).is_ok_and(|p| p.enabled),
            last_run_unix_seconds: Some(now()),
            ..next
        };
        let _ = host.emit("instance-sync-state", &state.status);
    }
    let mut state = manager().lock().expect("sync manager");
    let mut restart = false;
    if state.generation == generation {
        state.wake = None;
        state.status.enabled = policy(&managed).is_ok_and(|p| p.enabled);
        state.status.running = false;
        state.status.message = if state.status.enabled {
            "Select at least two instances and content to sync."
        } else {
            "Automatic syncing is off."
        }
        .into();
        let _ = host.emit("instance-sync-state", &state.status);
        restart =
            policy(&managed).is_ok_and(|p| p.enabled && p.instances.len() >= 2 && p.has_content());
    }
    drop(state);
    if restart {
        let _ = reconcile(host, managed);
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Observation {
    group: String,
    hashes: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct History {
    schema_version: u32,
    instances: BTreeMap<String, Observation>,
}
impl Default for History {
    fn default() -> Self {
        Self {
            schema_version: 1,
            instances: BTreeMap::new(),
        }
    }
}
struct Snapshot {
    group: String,
    file_hash: String,
    values: BTreeMap<String, String>,
}

#[derive(Default)]
pub struct Engine {
    policy_hash: String,
    pub(super) files: files::Cache,
}
impl Engine {
    pub fn cycle(&mut self, managed: &ManagedPaths) -> Result<Status, SyncError> {
        COUNTERS.with(|c| c.set(Counters::default()));
        let started = std::time::Instant::now();
        let mut result = self.cycle_inner(managed)?;
        COUNTERS.with(|c| {
            let c = c.get();
            result.files_scanned = c.scanned;
            result.files_hashed = c.hashed;
            result.bytes_copied = c.copied;
        });
        result.cycle_micros = started.elapsed().as_micros() as u64;
        Ok(result)
    }
    fn cycle_inner(&mut self, managed: &ManagedPaths) -> Result<Status, SyncError> {
        let selected = policy(managed)?;
        if !selected.enabled || selected.instances.len() < 2 {
            return Ok(Status::default());
        }
        let _cycle = TargetLock::acquire(
            managed.data_root(),
            &managed.launcher_dir(),
            ".automatic-sync.lock",
        )
        .map_err(storage)?;
        let current_policy = digest(&serde_json::to_vec(&selected).map_err(storage)?);
        if self.policy_hash != current_policy {
            self.files.clear();
            self.policy_hash = current_policy;
        }
        let mut result = Status {
            enabled: true,
            ..Status::default()
        };
        let registry =
            InstanceRegistry::load(&managed.instance_registry_file()).map_err(storage)?;
        let ids: Vec<_> = selected
            .instances
            .iter()
            .filter(|id| {
                registry
                    .find(id)
                    .is_some_and(|r| r.state() == InstanceState::Ready)
                    && selected.effective(id).enabled
                    && !crate::launch::process::snapshot(id.as_str())
                        .status
                        .blocks_launch()
            })
            .cloned()
            .collect();
        result.skipped_instances = selected.instances.len() - ids.len();
        if ids.len() < 2 {
            result.message =
                "Waiting for at least two selected instances to be stopped and available.".into();
            return Ok(result);
        }
        let history_path = managed.launcher_dir().join("sync-observed.json");
        let previous_history = read_small(managed, &history_path, 16 * 1024 * 1024)?;
        let mut history = match &previous_history {
            Some(bytes) => serde_json::from_slice::<History>(bytes).map_err(|_| {
                fail(
                    "sync_history_invalid",
                    "Automatic sync history needs attention; existing files were left unchanged.",
                )
            })?,
            None => History::default(),
        };
        if history.schema_version != 1
            || history.instances.len() > 64
            || history.instances.iter().any(|(id, observation)| {
                InstanceId::new(id).is_err()
                    || observation.group.len() != 64
                    || !observation.group.bytes().all(|b| b.is_ascii_hexdigit())
                    || observation.hashes.len() > 512
                    || observation.hashes.iter().any(|(key, hash)| {
                        key.len() > 256
                            || hash.len() != 64
                            || !hash.bytes().all(|b| b.is_ascii_hexdigit())
                    })
            })
        {
            return Err(fail(
                "sync_history_invalid",
                "Automatic sync history is unsupported or malformed and was left unchanged.",
            ));
        }
        let mut option_conflicts = Vec::new();
        if ids.iter().any(|id| {
            selected
                .effective(id)
                .categories
                .iter()
                .any(|c| c.options())
        }) {
            let mut snapshots = BTreeMap::new();
            for id in &ids {
                match snapshot(managed, id, &selected.effective(id)) {
                    Ok(Some(value)) => {
                        snapshots.insert(id.to_string(), value);
                    }
                    Ok(None) => {}
                    Err(_) => {
                        result.skipped_instances += 1;
                    }
                }
            }
            let mut batches: BTreeMap<(String, String, String), Vec<String>> = BTreeMap::new();
            let mut pending = Vec::new();
            let mut conflicting_keys = BTreeSet::new();
            for category in [Category::Settings, Category::Controls].iter() {
                let keys: BTreeSet<_> = snapshots
                    .values()
                    .flat_map(|s| s.values.keys())
                    .filter(|key| eligible(key, category))
                    .cloned()
                    .collect();
                for key in keys {
                    let groups: BTreeSet<_> = snapshots
                        .values()
                        .filter(|s| s.values.contains_key(&key))
                        .map(|s| s.group.clone())
                        .collect();
                    for group in groups {
                        let copies: Vec<_> = snapshots
                            .iter()
                            .filter(|(id, s)| {
                                s.group == group
                                    && s.values.contains_key(&key)
                                    && InstanceId::new(id.as_str()).is_ok_and(|id| {
                                        selected.effective(&id).categories.contains(category)
                                    })
                            })
                            .collect();
                        let values: BTreeSet<_> = copies
                            .iter()
                            .map(|(_, s)| s.values[&key].as_str())
                            .collect();
                        let changed: Vec<_> = copies
                            .iter()
                            .filter(|(id, s)| {
                                history.instances.get(*id).is_some_and(|old| {
                                    old.group == group
                                        && old.hashes.get(&key).is_some_and(|h| {
                                            h != &digest(s.values[&key].as_bytes())
                                        })
                                })
                            })
                            .collect();
                        let changed_values: BTreeSet<_> = changed
                            .iter()
                            .map(|(_, s)| s.values[&key].as_str())
                            .collect();
                        if changed_values.len() == 1 {
                            let (source_id, source) = changed[0];
                            pending.push((
                                (*source_id).clone(),
                                key.clone(),
                                group.clone(),
                                digest(source.values[&key].as_bytes()),
                            ));
                        }
                        if values.len() < 2 {
                            continue;
                        }
                        if changed_values.len() != 1 {
                            for (id, _) in &copies {
                                conflicting_keys.insert(((*id).clone(), key.clone()));
                            }
                            result.conflicts_kept += 1;
                            continue;
                        }
                        let (source_id, source) = changed[0];
                        for (target_id, target) in &copies {
                            if target.values[&key] == source.values[&key] {
                                continue;
                            }
                            let unchanged = history.instances.get(*target_id).is_some_and(|old| {
                                old.group == group
                                    && old.hashes.get(&key)
                                        == Some(&digest(target.values[&key].as_bytes()))
                            });
                            if unchanged {
                                batches
                                    .entry((
                                        (*source_id).clone(),
                                        (*target_id).clone(),
                                        format!("{category:?}"),
                                    ))
                                    .or_default()
                                    .push(key.clone());
                            } else {
                                result.conflicts_kept += 1;
                            }
                        }
                    }
                }
            }
            for ((source, target, kind), keys) in batches {
                if policy(managed)? != selected {
                    return Ok(result);
                }
                let category = if kind == "Controls" {
                    Category::Controls
                } else {
                    Category::Settings
                };
                let request = Request {
                    source_id: InstanceId::new(&source).map_err(storage)?,
                    destination_id: InstanceId::new(&target).map_err(storage)?,
                    category,
                    exclusions: pair(
                        &selected,
                        &InstanceId::new(&source).map_err(storage)?,
                        &InstanceId::new(&target).map_err(storage)?,
                        category,
                    )
                    .exclusions,
                    selected_paths: vec![],
                    key_filter: Some(keys),
                };
                let Some(source_snapshot) = snapshots.get(&source) else {
                    continue;
                };
                let Some(target_snapshot) = snapshots.get(&target) else {
                    continue;
                };
                let reviewed = locked(managed, &request, || {
                    let (review, files) = plan(managed, &request)?;
                    let file = files.first().ok_or_else(stale)?;
                    if file.source != source_snapshot.file_hash
                        || file.destination.as_deref() != Some(&target_snapshot.file_hash)
                    {
                        return Err(stale());
                    }
                    Ok(review)
                });
                match reviewed
                    .and_then(|p| apply_selected(managed, &request, &p.fingerprint, &selected))
                {
                    Ok(receipt) => {
                        result.files_changed += receipt.copied;
                        if let Ok(Some(next)) = snapshot(
                            managed,
                            &request.destination_id,
                            &selected.effective(&request.destination_id),
                        ) {
                            snapshots.insert(target, next);
                        }
                    }
                    Err(_) => result.conflicts_kept += 1,
                }
            }
            history.instances.retain(|id, _| {
                selected
                    .instances
                    .iter()
                    .any(|selected| selected.as_str() == id)
            });
            for (source_id, source) in &snapshots {
                for (target_id, target) in snapshots.iter().filter(|(id, _)| *id > source_id) {
                    if source.group != target.group {
                        continue;
                    }
                    for (key, value) in &source.values {
                        // One representative per compatible key avoids quadratic
                        // duplicate conflicts for the 64-instance pool.
                        if snapshots.iter().any(|(id, s)| {
                            id < source_id && s.group == source.group && s.values.contains_key(key)
                        }) {
                            continue;
                        }
                        if !target.values.get(key).is_some_and(|other| other != value) {
                            continue;
                        }
                        let category = if key.starts_with("key_") {
                            Category::Controls
                        } else {
                            Category::Settings
                        };
                        if option_conflicts.len() >= 1024 {
                            return Err(unsafe_path());
                        }
                        option_conflicts.push(Conflict {
                            id: digest(
                                &serde_json::to_vec(&(
                                    source_id,
                                    target_id,
                                    key,
                                    &source.file_hash,
                                    &target.file_hash,
                                ))
                                .map_err(storage)?,
                            ),
                            source_id: InstanceId::new(source_id).map_err(storage)?,
                            destination_id: InstanceId::new(target_id).map_err(storage)?,
                            category,
                            path: "options.txt".into(),
                            source_hash: source.file_hash.clone(),
                            destination_hash: target.file_hash.clone(),
                            key: Some(key.clone()),
                        });
                    }
                }
            }
            let mut deferred = conflicting_keys;
            for (source, key, group, winner) in pending {
                // A stopped peer may return after several minutes. Keep the
                // source's acknowledged baseline until every previously seen
                // compatible peer receives it; a busy/failed peer must not
                // silently lose an edit merely because the minute elapsed.
                let outstanding = history.instances.iter().any(|(id, old)| {
                    selected
                        .instances
                        .iter()
                        .any(|selected| selected.as_str() == id)
                        && old.group == group
                        && old.hashes.contains_key(&key)
                        && snapshots.get(id).is_none_or(|current| {
                            current.group == group
                                && current
                                    .values
                                    .get(&key)
                                    .is_none_or(|value| digest(value.as_bytes()) != winner)
                        })
                });
                if outstanding {
                    deferred.insert((source, key));
                }
            }
            for (id, snapshot) in snapshots {
                let mut hashes: BTreeMap<_, _> = snapshot
                    .values
                    .into_iter()
                    .map(|(key, value)| (key, digest(value.as_bytes())))
                    .collect();
                if let Some(old) = history
                    .instances
                    .get(&id)
                    .filter(|old| old.group == snapshot.group)
                {
                    for (key, hash) in &old.hashes {
                        if !hashes.contains_key(key)
                            || deferred.contains(&(id.clone(), key.clone()))
                        {
                            hashes.insert(key.clone(), hash.clone());
                        }
                    }
                }
                if hashes.len() > 512 {
                    return Err(unsafe_path());
                }
                history.instances.insert(
                    id,
                    Observation {
                        group: snapshot.group,
                        hashes,
                    },
                );
            }
            let next_history = serde_json::to_vec(&history).map_err(storage)?;
            if previous_history.as_deref() != Some(next_history.as_slice()) {
                crate::install::journal::atomic_write(&history_path, &next_history)
                    .map_err(storage)?;
            }
        }
        files::cycle(
            &mut self.files,
            managed,
            &selected,
            &ids,
            &mut result,
            option_conflicts,
        )?;
        if result.message.is_empty() {
            result.message = format!(
                "Synced {} file(s). Kept {} existing differences or conflicting edits. Busy instances are skipped.",
                result.files_changed, result.conflicts_kept
            );
        }
        Ok(result)
    }
}

pub(super) fn pair(
    policy: &Policy,
    source: &InstanceId,
    target: &InstanceId,
    category: Category,
) -> Request {
    Request {
        source_id: source.clone(),
        destination_id: target.clone(),
        category,
        exclusions: {
            let mut paths = policy.effective(source).exclusions;
            paths.extend(policy.effective(target).exclusions);
            paths.sort();
            paths.dedup();
            paths
        },
        selected_paths: if category == Category::Configuration {
            policy.selected_paths.clone()
        } else {
            vec![]
        },
        key_filter: None,
    }
}
pub(super) fn apply_selected(
    managed: &ManagedPaths,
    request: &Request,
    fingerprint: &str,
    selected: &Policy,
) -> Result<Receipt, SyncError> {
    apply_with_checkpoint(managed, request, fingerprint, |_| {
        if policy(managed)? == *selected {
            Ok(())
        } else {
            Err(stale())
        }
    })
}
fn eligible(key: &str, category: &Category) -> bool {
    if key.len() > 256 {
        return false;
    }
    if *category == Category::Controls {
        key.starts_with("key_")
    } else {
        matches!(
            key,
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
    }
}
fn snapshot(
    managed: &ManagedPaths,
    id: &InstanceId,
    selected: &Policy,
) -> Result<Option<Snapshot>, SyncError> {
    let _play = crate::launch::process::PreparationGuard::acquire(id.as_str()).map_err(storage)?;
    let _operation =
        crate::instances::lifecycle::lock_instance_operation(managed, id).map_err(storage)?;
    crate::instance_content::with_instance_lock(id, || {
        Ok((|| {
            if selected
                .exclusions
                .iter()
                .any(|path| path.eq_ignore_ascii_case("options.txt"))
            {
                return Ok(None);
            }
            let registry =
                InstanceRegistry::load(&managed.instance_registry_file()).map_err(storage)?;
            let record = registry.find(id).ok_or_else(stale)?;
            let path = managed.instance_paths(id).root().join("options.txt");
            let Some(bytes) = read_small(managed, &path, MAX_OPTIONS)? else {
                return Ok(None);
            };
            let (_, mut values) = options(&bytes)?;
            values.retain(|key, value| {
                selected
                    .categories
                    .iter()
                    .filter(|c| c.options())
                    .any(|c| eligible(key, c))
                    && value.len() <= 128
            });
            if values.len() > 512 {
                return Err(unsafe_path());
            }
            Ok(Some(Snapshot {
                group: digest(
                    &serde_json::to_vec(&(
                        &record.installed().minecraft_version,
                        &record.installed().platform,
                    ))
                    .map_err(storage)?,
                ),
                file_hash: file_digest(&bytes),
                values,
            }))
        })())
    })
    .map_err(storage)?
}
pub(super) fn folder_signature(
    managed: &ManagedPaths,
    request: &Request,
) -> Result<String, SyncError> {
    let root = managed
        .instance_paths(&request.source_id)
        .root()
        .to_path_buf();
    checked_directory(managed.data_root(), &root).map_err(storage)?;
    let mut signature = Sha256::new();
    for relative in collect(managed, &root, request)? {
        let meta = match fs::symlink_metadata(root.join(&relative)) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(storage(error)),
        };
        if redirected(&meta) || !meta.is_file() {
            return Err(unsafe_path());
        }
        signature.update(relative.as_bytes());
        signature.update(meta.len().to_le_bytes());
        signature.update(
            meta.modified()
                .map_err(storage)?
                .duration_since(UNIX_EPOCH)
                .map_err(storage)?
                .as_nanos()
                .to_le_bytes(),
        );
    }
    Ok(format!("{:x}", signature.finalize()))
}
