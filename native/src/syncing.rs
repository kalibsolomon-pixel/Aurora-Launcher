//! Automatic syncing preferences; the Rust minute worker owns execution.
use crate::{AppWindow, InstanceSync, SyncChoice, api, ui};
use slint::ComponentHandle;
use slint::Model;
const CATEGORIES: [(&str, &str); 6] = [
    ("settings", "Game settings"),
    ("controls", "Controls"),
    ("resourcePacks", "Resource packs"),
    ("shaderPacks", "Shader packs"),
    ("screenshots", "Screenshots"),
    ("configuration", "Selected configuration"),
];

pub fn bind(window: &AppWindow) {
    let sync = window.global::<InstanceSync>();
    sync.on_enable_changed(|enabled| change(move |policy| policy.enabled = enabled));
    sync.on_participant_changed(|index, checked| {
        let id = ui::state(|state| state.sync_instance_ids.get(index as usize).cloned());
        if let Some(id) = id {
            change(move |policy| {
                policy.instances.retain(|current| current != &id);
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
fn render() {
    ui::with(|window, state| {
        let Some(policy) = &state.sync_policy else {
            return;
        };
        let sync = window.global::<InstanceSync>();
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
