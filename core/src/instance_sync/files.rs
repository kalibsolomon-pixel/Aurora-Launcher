//! Ordinary-file baselines and pending conflicts over the existing transactions.
use super::*;

pub(super) type Cache = BTreeMap<String, (String, BTreeMap<String, (String, u64)>)>;
const MAX_OBSERVATIONS: usize = 32768;
const MAX_CONFLICTS: usize = 1024;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Track {
    category: Option<Category>,
    path: String,
    group: String,
    hashes: BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Conflict {
    pub id: String,
    pub source_id: InstanceId,
    pub destination_id: InstanceId,
    pub category: Category,
    pub path: String,
    pub source_hash: String,
    pub destination_hash: String,
    #[serde(default)]
    pub key: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    schema_version: u32,
    tracks: BTreeMap<String, Track>,
    conflicts: Vec<Conflict>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema_version: 1,
            tracks: BTreeMap::new(),
            conflicts: vec![],
        }
    }
}
fn path(managed: &ManagedPaths) -> PathBuf {
    managed.launcher_dir().join("sync-files.json")
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}
fn invalid() -> SyncError {
    fail(
        "sync_history_invalid",
        "File sync history is malformed, unsupported or full; it was left unchanged.",
    )
}
fn load(managed: &ManagedPaths) -> Result<State, SyncError> {
    let Some(bytes) = read_small(managed, &path(managed), 16 * 1024 * 1024)? else {
        return Ok(State::default());
    };
    let state: State = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if state.schema_version != 1
        || state.tracks.len() > MAX_OBSERVATIONS
        || state.conflicts.len() > MAX_CONFLICTS
        || state.tracks.values().map(|t| t.hashes.len()).sum::<usize>() > MAX_OBSERVATIONS
    {
        return Err(invalid());
    }
    for (key, t) in &state.tracks {
        if !valid_hash(key)
            || t.category.is_none()
            || validate_relative(&t.path).is_err()
            || !valid_hash(&t.group)
            || t.hashes.len() > 64
            || t.hashes
                .iter()
                .any(|(id, hash)| InstanceId::new(id).is_err() || !valid_hash(hash))
        {
            return Err(invalid());
        }
    }
    for c in &state.conflicts {
        if !valid_hash(&c.id)
            || !valid_hash(&c.source_hash)
            || !valid_hash(&c.destination_hash)
            || validate_relative(&c.path).is_err()
            || c.source_id == c.destination_id
            || c.category.options() != c.key.is_some()
            || c.key
                .as_ref()
                .is_some_and(|k| !c.category.options() || k.len() > 256)
        {
            return Err(invalid());
        }
    }
    Ok(state)
}
fn save(managed: &ManagedPaths, state: &State) -> Result<(), SyncError> {
    if state.tracks.values().map(|t| t.hashes.len()).sum::<usize>() > MAX_OBSERVATIONS
        || state.conflicts.len() > MAX_CONFLICTS
    {
        return Err(invalid());
    }
    let bytes = serde_json::to_vec(state).map_err(storage)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(invalid());
    }
    if read_small(managed, &path(managed), 16 * 1024 * 1024)?.as_deref() != Some(bytes.as_slice()) {
        checked_directory(managed.data_root(), &managed.launcher_dir()).map_err(storage)?;
        crate::install::journal::atomic_write(&path(managed), &bytes).map_err(storage)?;
    }
    Ok(())
}
pub fn conflicts(managed: &ManagedPaths) -> Result<Vec<Conflict>, SyncError> {
    Ok(load(managed)?.conflicts)
}

pub(super) fn cycle(
    cache: &mut Cache,
    managed: &ManagedPaths,
    selected: &Policy,
    ids: &[InstanceId],
    result: &mut automatic::Status,
    option_conflicts: Vec<Conflict>,
) -> Result<(), SyncError> {
    let mut state = load(managed)?;
    for track in state.tracks.values_mut() {
        track
            .hashes
            .retain(|id, _| selected.instances.iter().any(|i| i.as_str() == id));
    }
    state.tracks.retain(|_, t| !t.hashes.is_empty());
    state.conflicts.retain(|c| {
        selected.instances.contains(&c.source_id) && selected.instances.contains(&c.destination_id)
    });
    state.conflicts.retain(|c| {
        !c.category.options() || !ids.contains(&c.source_id) || !ids.contains(&c.destination_id)
    });
    state.conflicts.extend(option_conflicts);
    let mut remaining = MAX_TOTAL;
    for category in [
        Category::ResourcePacks,
        Category::ShaderPacks,
        Category::Screenshots,
        Category::Configuration,
        Category::Servers,
    ] {
        let participants: Vec<_> = ids
            .iter()
            .filter(|id| selected.effective(id).categories.contains(&category))
            .collect();
        if participants.len() < 2
            || (category == Category::Configuration && selected.selected_paths.is_empty())
        {
            continue;
        }
        let mut groups: BTreeMap<String, Vec<InstanceId>> = BTreeMap::new();
        let registry =
            InstanceRegistry::load(&managed.instance_registry_file()).map_err(storage)?;
        let mut current: BTreeMap<String, BTreeMap<String, (String, u64)>> = BTreeMap::new();
        for id in &participants {
            let other = participants.iter().find(|other| other != &id).unwrap();
            let mut request = automatic::pair(selected, id, other, category);
            request.exclusions = selected.effective(id).exclusions;
            let snapshot = (|| {
                let _play = crate::launch::process::PreparationGuard::acquire(id.as_str())
                    .map_err(storage)?;
                let _operation = crate::instances::lifecycle::lock_instance_operation(managed, id)
                    .map_err(storage)?;
                crate::instance_content::with_instance_lock(id, || {
                    Ok((|| {
                        let root = managed.instance_paths(id).root().to_path_buf();
                        let signature = automatic::folder_signature(managed, &request)?;
                        let key = format!("{category:?}:{}", id.as_str());
                        if let Some((old, files)) = cache.get(&key) {
                            if old == &signature {
                                return Ok(files.clone());
                            }
                        }
                        let mut files = BTreeMap::new();
                        let mut total = 0;
                        for relative in collect(managed, &root, &request)? {
                            if let Some((hash, size)) = hash_file(managed, &root.join(&relative))? {
                                total += size;
                                if total > MAX_TOTAL {
                                    return Err(unsafe_path());
                                }
                                files.insert(relative, (hash, size));
                            }
                        }
                        if cache.values().map(|(_, f)| f.len()).sum::<usize>() + files.len()
                            > MAX_OBSERVATIONS
                        {
                            cache.clear();
                        }
                        cache.insert(key, (signature, files.clone()));
                        Ok(files)
                    })())
                })
                .map_err(storage)?
            })();
            match snapshot {
                Ok(files) => {
                    if current.values().map(|files| files.len()).sum::<usize>() + files.len()
                        > MAX_OBSERVATIONS
                    {
                        return Err(unsafe_path());
                    }
                    let record = registry.find(id).ok_or_else(stale)?;
                    let group = if category == Category::Configuration {
                        digest(&serde_json::to_vec(&record.installed()).map_err(storage)?)
                    } else {
                        digest(b"shared-files")
                    };
                    groups.entry(group).or_default().push((*id).clone());
                    current.insert(id.to_string(), files);
                }
                Err(_) => result.skipped_instances += 1,
            }
        }
        for (group, peers) in groups {
            if peers.len() < 2 {
                continue;
            }
            let paths: BTreeSet<_> = peers
                .iter()
                .flat_map(|id| current[id.as_str()].keys().cloned())
                .collect();
            for relative in paths {
                let key =
                    digest(&serde_json::to_vec(&(category, &group, &relative)).map_err(storage)?);
                let track = state.tracks.entry(key).or_insert_with(|| Track {
                    category: Some(category),
                    path: relative.clone(),
                    group: group.clone(),
                    hashes: BTreeMap::new(),
                });
                // Replace records only for available peers for this path; unavailable conflicts remain pending.
                state.conflicts.retain(|c| {
                    c.category != category
                        || c.path != relative
                        || !peers.contains(&c.source_id)
                        || !peers.contains(&c.destination_id)
                });
                let copies: Vec<_> = peers
                    .iter()
                    .filter_map(|id| {
                        current[id.as_str()]
                            .get(&relative)
                            .map(|(hash, size)| (id.clone(), hash.clone(), *size))
                    })
                    .collect();
                let values: BTreeSet<_> = copies.iter().map(|(_, hash, _)| hash.clone()).collect();
                let changed: Vec<_> = copies
                    .iter()
                    .filter(|(id, hash, _)| {
                        track.hashes.get(id.as_str()).is_some_and(|old| old != hash)
                    })
                    .collect();
                let changed_values: BTreeSet<_> =
                    changed.iter().map(|(_, h, _)| h.clone()).collect();
                let winner = if values.len() == 1 {
                    copies.first()
                } else if changed_values.len() == 1 {
                    changed.first().copied()
                } else {
                    None
                };
                if let Some((source, hash, size)) = winner {
                    for target in peers.iter().filter(|id| *id != source) {
                        let destination = current[target.as_str()]
                            .get(&relative)
                            .map(|(h, _)| h.clone());
                        if destination.as_ref() == Some(hash) {
                            continue;
                        }
                        let effective = selected.effective(target);
                        if excluded(
                            &automatic::pair(&effective, source, target, category),
                            &relative,
                        ) {
                            continue;
                        }
                        let safe = destination.is_none()
                            || (destination.as_ref() == track.hashes.get(target.as_str())
                                && track.hashes.get(target.as_str())
                                    == track.hashes.get(source.as_str()));
                        if !safe && category != Category::Servers {
                            continue;
                        }
                        if *size > remaining {
                            result.message =
                                "Remaining copies will continue on the next minute.".into();
                            continue;
                        }
                        let request = automatic::pair(selected, source, target, category);
                        let selection =
                            BTreeMap::from([(relative.clone(), (hash.clone(), destination))]);
                        let attempt = locked(managed, &request, || {
                            plan_files(managed, &request, Some(&selection)).map(|p| p.0)
                        })
                        .and_then(|preview| {
                            apply_files(
                                managed,
                                &request,
                                &preview.fingerprint,
                                Some(&selection),
                                &mut |_| {
                                    if policy(managed)? == *selected {
                                        Ok(())
                                    } else {
                                        Err(stale())
                                    }
                                },
                            )
                        });
                        match attempt {
                            Ok(receipt) => {
                                result.files_changed += receipt.copied;
                                if category == Category::Servers {
                                    result.conflicts_kept += receipt.conflicts_kept;
                                }
                                remaining = remaining.saturating_sub(*size);
                                if receipt.copied > 0 {
                                    cache.retain(|key, _| !key.ends_with(target.as_str()));
                                    if let Some(next) = hash_file(
                                        managed,
                                        &managed.instance_paths(target).root().join(&relative),
                                    )? {
                                        current
                                            .get_mut(target.as_str())
                                            .unwrap()
                                            .insert(relative.clone(), next);
                                    }
                                }
                            }
                            Err(_) => result.skipped_instances += 1,
                        }
                    }
                } else if category == Category::Servers {
                    // Independent additions merge in both directions through the same NBT plan.
                    for source in &peers {
                        for target in peers.iter().filter(|t| *t != source) {
                            let request = automatic::pair(selected, source, target, category);
                            match preview(managed, &request).and_then(|p| {
                                automatic::apply_selected(
                                    managed,
                                    &request,
                                    &p.fingerprint,
                                    selected,
                                )
                            }) {
                                Ok(receipt) => {
                                    result.files_changed += receipt.copied;
                                    if category == Category::Servers {
                                        result.conflicts_kept += receipt.conflicts_kept;
                                    }
                                    if receipt.copied > 0 {
                                        cache.retain(|k, _| !k.ends_with(target.as_str()));
                                        if let Some(next) = hash_file(
                                            managed,
                                            &managed.instance_paths(target).root().join(&relative),
                                        )? {
                                            current
                                                .get_mut(target.as_str())
                                                .unwrap()
                                                .insert(relative.clone(), next);
                                        }
                                    }
                                }
                                Err(_) => result.skipped_instances += 1,
                            }
                        }
                    }
                }
                let final_copies: Vec<_> = peers
                    .iter()
                    .filter_map(|id| {
                        current[id.as_str()]
                            .get(&relative)
                            .map(|(hash, _)| (id.clone(), hash.clone()))
                    })
                    .collect();
                let final_values: BTreeSet<_> =
                    final_copies.iter().map(|(_, h)| h.clone()).collect();
                if final_values.len() > 1 {
                    let (source, source_hash) = &final_copies[0];
                    for (target, target_hash) in final_copies
                        .iter()
                        .skip(1)
                        .filter(|(_, h)| h != source_hash)
                    {
                        if category == Category::Servers {
                            let a = read_small(
                                managed,
                                &managed.instance_paths(source).root().join(&relative),
                                MAX_OPTIONS,
                            )?
                            .ok_or_else(stale)?;
                            let b = read_small(
                                managed,
                                &managed.instance_paths(target).root().join(&relative),
                                MAX_OPTIONS,
                            )?
                            .ok_or_else(stale)?;
                            if servers::merge(&a, Some(&b))?.conflicts == 0 {
                                continue;
                            }
                        }
                        let conflict = Conflict {
                            id: digest(
                                &serde_json::to_vec(&(
                                    source,
                                    target,
                                    category,
                                    &relative,
                                    source_hash,
                                    target_hash,
                                ))
                                .map_err(storage)?,
                            ),
                            source_id: source.clone(),
                            destination_id: target.clone(),
                            category,
                            path: relative.clone(),
                            source_hash: source_hash.clone(),
                            destination_hash: target_hash.clone(),
                            key: None,
                        };
                        if state.conflicts.len() >= MAX_CONFLICTS {
                            return Err(invalid());
                        }
                        state.conflicts.push(conflict);
                        result.conflicts_kept += 1;
                    }
                }
                let all_ack = final_values.len() == 1
                    && track.hashes.keys().all(|id| {
                        final_copies.iter().any(|(peer, hash)| {
                            peer.as_str() == id && Some(hash) == final_values.first()
                        })
                    });
                for (id, hash) in final_copies {
                    if all_ack || !track.hashes.contains_key(id.as_str()) {
                        track.hashes.insert(id.to_string(), hash);
                    }
                }
            }
        }
    }
    if policy(managed)? != *selected {
        return Err(stale());
    }
    save(managed, &state)
}

/// Explicit choice is bound to the pending hashes, policy and stopped-instance locks.
/// Keep/use are the same operation with opposite direction. Defer makes no writes.
pub fn resolve(managed: &ManagedPaths, id: &str, use_source: bool) -> Result<Receipt, SyncError> {
    let _cycle = TargetLock::acquire(
        managed.data_root(),
        &managed.launcher_dir(),
        ".automatic-sync.lock",
    )
    .map_err(storage)?;
    let selected = policy(managed)?;
    if !selected.enabled {
        return Err(stale());
    }
    let mut state = load(managed)?;
    let c = state
        .conflicts
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or_else(stale)?;
    for id in [&c.source_id, &c.destination_id] {
        let p = selected.effective(id);
        if !p.enabled || !selected.instances.contains(id) || !p.categories.contains(&c.category) {
            return Err(stale());
        }
    }
    let (source, target, hash, old) = if use_source {
        (
            &c.source_id,
            &c.destination_id,
            &c.source_hash,
            &c.destination_hash,
        )
    } else {
        (
            &c.destination_id,
            &c.source_id,
            &c.destination_hash,
            &c.source_hash,
        )
    };
    // Server edits remain unsupported; never replace entire lists on this action.
    if c.category == Category::Servers {
        return Err(fail(
            "sync_server_edit_unsupported",
            "Server-list edits or ambiguous entries must be resolved in Minecraft; both lists are preserved.",
        ));
    }
    let mut request = automatic::pair(&selected, source, target, c.category);
    request.key_filter = c.key.clone().map(|k| vec![k]);
    let selection = BTreeMap::from([(c.path.clone(), (hash.clone(), Some(old.clone())))]);
    let p = locked(managed, &request, || {
        plan_files(managed, &request, Some(&selection)).map(|p| p.0)
    })?;
    let receipt = apply_files(
        managed,
        &request,
        &p.fingerprint,
        Some(&selection),
        &mut |_| {
            if policy(managed)? == selected {
                Ok(())
            } else {
                Err(stale())
            }
        },
    )?;
    state.conflicts.retain(|v| v.id != c.id);
    // A deliberate resolution starts a common acknowledged baseline for these peers.
    for t in state.tracks.values_mut().filter(|t| {
        t.path == c.path
            && t.category == Some(c.category)
            && t.hashes.contains_key(source.as_str())
            && t.hashes.contains_key(target.as_str())
    }) {
        t.hashes.insert(source.to_string(), hash.clone());
        t.hashes.insert(target.to_string(), hash.clone());
    }
    save(managed, &state)?;
    Ok(receipt)
}
