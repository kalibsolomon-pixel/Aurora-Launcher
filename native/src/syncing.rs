//! Automatic syncing preferences; the Rust minute worker owns execution.
use crate::{AppWindow, InstanceSync, SyncChoice, api, ui};
use slint::ComponentHandle;
use slint::Model;
const CATEGORIES: [(&str, &str); 7] = [
    ("settings", "Game settings"),
    ("controls", "Controls"),
    ("resourcePacks", "Resource packs"),
    ("shaderPacks", "Shader packs"),
    ("screenshots", "Screenshots"),
    ("configuration", "Selected configuration"),
    ("servers", "Server list"),
];

pub fn bind(window: &AppWindow) {
    let sync = window.global::<InstanceSync>();
    sync.on_enable_changed(|enabled| change(move |policy| policy.enabled = enabled));
    sync.on_participant_changed(|index, checked| {
        let id = ui::state(|state| state.sync_instance_ids.get(index as usize).cloned());
        if let Some(id) = id {
            change(move |policy| {
                policy.instances.retain(|current| current != &id);
                if !checked {
                    policy.overrides.remove(&id);
                }
                if checked {
                    policy.instances.push(id);
                }
            });
        }
    });
    sync.on_category_changed(|index, checked| {
        if let Some((id, _)) = CATEGORIES.get(index as usize) {
            let id = id.to_string();
            change(move |policy| {
                policy.categories.retain(|current| current != &id);
                if checked {
                    policy.categories.push(id);
                }
            });
        }
    });
    sync.on_paths_changed(|| {
        let paths = ui::with(|window, _| {
            let sync = window.global::<InstanceSync>();
            let split = |text: slint::SharedString| {
                text.split(';')
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            (
                split(sync.get_exclusions()),
                split(sync.get_selected_paths()),
            )
        });
        if let Some((exclusions, selected)) = paths {
            change(move |policy| {
                policy.exclusions = exclusions;
                policy.selected_paths = selected;
            });
        }
    });
    sync.on_instance_disabled_changed(|disabled| {
        edit_instance(move |value| value.disabled = disabled)
    });
    sync.on_instance_category_changed(|index, mode| {
        if let Some((category, _)) = CATEGORIES.get(index as usize) {
            let category = category.to_string();
            edit_instance(move |value| {
                value.categories.remove(&category);
                if mode == 1 || mode == 2 {
                    value.categories.insert(category, mode == 1);
                }
            });
        }
    });
    sync.on_instance_paths_changed(|| {
        if let Some(text) = ui::with(|w, _| {
            w.global::<InstanceSync>()
                .get_instance_exclusions()
                .to_string()
        }) {
            edit_instance(move |v| {
                v.exclusions = text
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            });
        }
    });
    sync.on_refresh_conflicts(refresh_conflicts);
    sync.on_defer_conflict(|| {
        ui::with(|w, _| {
            w.global::<InstanceSync>().set_message(
                "Deferred. Both versions remain unchanged; the conflict stays pending.".into(),
            )
        });
    });
    sync.on_resolve_conflict(|index, use_source| {
        let id = ui::with(|w, s| {
            if w.global::<InstanceSync>().get_busy() {
                return None;
            }
            let id = s.sync_conflicts.get(index as usize)?.id.clone();
            w.global::<InstanceSync>().set_busy(true);
            Some(id)
        })
        .flatten();
        if let Some(id) = id {
            ui::spawn(api::sync_resolve(ui::host(), id, use_source), |result| {
                ui::with(|w, _| {
                    let sync = w.global::<InstanceSync>();
                    sync.set_busy(false);
                    sync.set_message(match result {
                        Ok(_) => "Conflict resolved. The replaced version is backed up.".into(),
                        Err(e) => e.message.into(),
                    });
                });
                refresh_conflicts();
            });
        }
    });
}
pub fn load() {
    ui::with(|window, state| {
        state.sync_instance_ids = state
            .instances()
            .iter()
            .map(|instance| instance.id.clone())
            .collect();
        window.global::<InstanceSync>().set_loaded(false);
    });
    ui::spawn(api::sync_policy(ui::host()), |result| match result {
        Ok(policy) => {
            ui::with(|_, state| {
                for id in &policy.instances {
                    if !state.sync_instance_ids.contains(id) {
                        state.sync_instance_ids.push(id.clone());
                    }
                }
                state.sync_policy = Some(policy);
            });
            render();
        }
        Err(error) => {
            ui::with(|w, _| w.global::<InstanceSync>().set_message(error.message.into()));
        }
    });
    ui::spawn(api::sync_status(), |result| {
        if let Ok(status) = result {
            on_status(status);
        }
    });
}
fn change(edit: impl FnOnce(&mut api::SyncPolicy)) {
    let policy = ui::with(|window, state| {
        let sync = window.global::<InstanceSync>();
        if sync.get_busy() || !sync.get_loaded() {
            return None;
        }
        let mut policy = state.sync_policy.clone()?;
        edit(&mut policy);
        sync.set_busy(true);
        Some(policy)
    })
    .flatten();
    let Some(policy) = policy else {
        return;
    };
    ui::spawn(api::sync_save(ui::host(), policy), |result| {
        ui::with(|window, state| {
            window.global::<InstanceSync>().set_busy(false);
            match result {
                Ok(policy) => {
                    state.sync_policy = Some(policy);
                }
                Err(error) => window
                    .global::<InstanceSync>()
                    .set_message(error.message.into()),
            }
        });
        render();
        ui::spawn(api::sync_status(), |result| {
            if let Ok(status) = result {
                on_status(status);
            }
        });
    });
}
pub fn on_status(status: api::SyncStatus) {
    if !status.running {
        refresh_conflicts();
    }
    ui::with(|window, _| {
        window
            .global::<InstanceSync>()
            .set_message(if status.running {
                "Syncing selected instances…".into()
            } else {
                status.message.into()
            })
    });
}
pub fn render() {
    ui::with(|window, state| {
        let Some(policy) = &state.sync_policy else {
            return;
        };
        let sync = window.global::<InstanceSync>();
        let workspace = state.workspace.as_ref().map(|w| w.id.as_str());
        let participating = workspace.is_some_and(|id| policy.instances.iter().any(|v| v == id));
        let overrides = workspace.and_then(|id| policy.overrides.get(id));
        sync.set_instance_participating(participating);
        sync.set_instance_disabled(overrides.is_some_and(|v| v.disabled));
        sync.set_instance_exclusions(
            overrides
                .map(|v| v.exclusions.join("; "))
                .unwrap_or_default()
                .into(),
        );
        sync.set_instance_categories(override_choices(
            sync.get_instance_categories(),
            CATEGORIES
                .iter()
                .map(|(id, name)| crate::SyncOverrideChoice {
                    name: (*name).into(),
                    mode: match overrides.and_then(|v| v.categories.get(*id)) {
                        Some(true) => 1,
                        Some(false) => 2,
                        None => 0,
                    },
                })
                .collect(),
        ));
        sync.set_enabled(policy.enabled);
        sync.set_loaded(true);
        sync.set_exclusions(policy.exclusions.join("; ").into());
        sync.set_selected_paths(policy.selected_paths.join("; ").into());
        sync.set_configuration_selected(policy.categories.iter().any(|c| c == "configuration"));
        sync.set_instances(choices(
            sync.get_instances(),
            state
                .sync_instance_ids
                .iter()
                .map(|id| SyncChoice {
                    name: state
                        .instance(id)
                        .map(|instance| instance.display_name.clone())
                        .unwrap_or_else(|| "Unavailable instance".into())
                        .into(),
                    checked: policy.instances.contains(id),
                })
                .collect(),
        ));
        sync.set_categories(choices(
            sync.get_categories(),
            CATEGORIES
                .iter()
                .map(|(id, name)| SyncChoice {
                    name: (*name).into(),
                    checked: policy.categories.iter().any(|category| category == id),
                })
                .collect(),
        ));
    });
}

fn choices(
    current: slint::ModelRc<SyncChoice>,
    next: Vec<SyncChoice>,
) -> slint::ModelRc<SyncChoice> {
    if current.row_count() == next.len()
        && next.iter().enumerate().all(|(index, item)| {
            current
                .row_data(index)
                .is_some_and(|old| old.name == item.name)
        })
    {
        if let Some(model) = current
            .as_any()
            .downcast_ref::<slint::VecModel<SyncChoice>>()
        {
            for (index, item) in next.into_iter().enumerate() {
                if model
                    .row_data(index)
                    .is_none_or(|old| old.checked != item.checked)
                {
                    model.set_row_data(index, item);
                }
            }
            return current;
        }
    }
    ui::model(next)
}

fn edit_instance(edit: impl FnOnce(&mut api::SyncOverride)) {
    let id = ui::state(|s| s.workspace.as_ref().map(|w| w.id.clone()));
    if let Some(id) = id {
        change(move |p| {
            if p.instances.contains(&id) {
                let value = p.overrides.entry(id.clone()).or_default();
                edit(value);
                if !value.disabled && value.categories.is_empty() && value.exclusions.is_empty() {
                    p.overrides.remove(&id);
                }
            }
        });
    }
}
fn refresh_conflicts() {
    ui::spawn(api::sync_conflicts(ui::host()), |result| match result {
        Ok(conflicts) => {
            ui::with(|w, s| {
                w.global::<InstanceSync>().set_conflicts(ui::model(
                    conflicts
                        .iter()
                        .map(|c| crate::SyncConflictRow {
                            label: c
                                .key
                                .as_ref()
                                .map(|k| format!("{} · {k}", c.path))
                                .unwrap_or_else(|| c.path.clone())
                                .into(),
                            peers: format!(
                                "First: {} · Second: {}",
                                s.instance(&c.source_id)
                                    .map(|i| i.display_name.as_str())
                                    .unwrap_or("Unavailable"),
                                s.instance(&c.destination_id)
                                    .map(|i| i.display_name.as_str())
                                    .unwrap_or("Unavailable")
                            )
                            .into(),
                            resolvable: c.category != "servers",
                        })
                        .collect(),
                ));
                s.sync_conflicts = conflicts;
            });
        }
        Err(error) => {
            ui::with(|w, _| w.global::<InstanceSync>().set_message(error.message.into()));
        }
    });
}

fn override_choices(
    current: slint::ModelRc<crate::SyncOverrideChoice>,
    next: Vec<crate::SyncOverrideChoice>,
) -> slint::ModelRc<crate::SyncOverrideChoice> {
    if current.row_count() == next.len()
        && next
            .iter()
            .enumerate()
            .all(|(i, v)| current.row_data(i).is_some_and(|old| old.name == v.name))
    {
        if let Some(model) = current
            .as_any()
            .downcast_ref::<slint::VecModel<crate::SyncOverrideChoice>>()
        {
            for (i, v) in next.into_iter().enumerate() {
                if model.row_data(i).is_none_or(|old| old.mode != v.mode) {
                    model.set_row_data(i, v);
                }
            }
            return current;
        }
    }
    ui::model(next)
}
