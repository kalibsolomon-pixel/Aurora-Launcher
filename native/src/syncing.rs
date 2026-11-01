//! Demand-driven sync controls. Review inputs are frozen; the backend repeats
//! byte validation before apply. No scan occurs merely by opening Settings.
use crate::{AppWindow, InstanceSync, api, ui};
use slint::ComponentHandle;

const CATEGORIES: [&str; 6] = [
    "settings",
    "controls",
    "resourcePacks",
    "shaderPacks",
    "screenshots",
    "configuration",
];

pub fn bind(window: &AppWindow) {
    let sync = window.global::<InstanceSync>();
    sync.on_changed(invalidate);
    sync.on_enable_changed(|enabled| {
        invalidate();
        ui::with(|w, _| w.global::<InstanceSync>().set_busy(true));
        ui::spawn(api::sync_enabled(ui::host(), enabled), |result| {
            ui::with(|w, _| {
                let sync = w.global::<InstanceSync>();
                sync.set_busy(false);
                match result {
                    Ok(policy) => {
                        sync.set_enabled(policy.enabled);
                        sync.set_message("".into());
                    }
                    Err(error) => sync.set_message(error.message.into()),
                }
            });
        });
    });
    sync.on_review(review);
    sync.on_apply(apply);
}

pub fn load() {
    invalidate();
    ui::with(|w, state| {
        let sync = w.global::<InstanceSync>();
        state.sync_instance_ids = state
            .instances()
            .iter()
            .map(|instance| instance.id.clone())
            .collect();
        sync.set_names(ui::model(
            state
                .instances()
                .iter()
                .map(|i| i.display_name.clone().into())
                .collect(),
        ));
        sync.set_loaded(false);
    });
    ui::spawn(api::sync_policy(ui::host()), |result| {
        ui::with(|w, _| {
            let sync = w.global::<InstanceSync>();
            match result {
                Ok(policy) => {
                    sync.set_enabled(policy.enabled);
                    sync.set_loaded(true);
                }
                Err(error) => sync.set_message(error.message.into()),
            }
        });
    });
}

fn invalidate() {
    ui::with(|w, state| {
        state.sync_review = None;
        w.global::<InstanceSync>().set_can_apply(false);
        w.global::<InstanceSync>().set_message("".into());
    });
}

fn choice() -> Option<serde_json::Value> {
    ui::with(|w, state| {
        let sync = w.global::<InstanceSync>();
        if sync.get_busy() || !sync.get_enabled() || !sync.get_loaded() { return None; }
        let source = state.instance(state.sync_instance_ids.get(usize::try_from(sync.get_source_index()).ok()?)?)?;
        let destination = state.instance(state.sync_instance_ids.get(usize::try_from(sync.get_destination_index()).ok()?)?)?;
        let category = CATEGORIES.get(usize::try_from(sync.get_category_index()).ok()?)?;
        Some(serde_json::json!({ "sourceId": source.id, "destinationId": destination.id, "category": category,
            "exclusions": sync.get_exclusions().split(';').map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>(),
            "selectedPaths": if *category == "configuration" { sync.get_selected_paths().split(';').map(str::trim).filter(|p| !p.is_empty()).map(str::to_owned).collect::<Vec<_>>() } else { vec![] } }))
    }).flatten()
}

pub fn summary(preview: &api::SyncPreview) -> String {
    let mut text = format!(
        "{} change(s) · {} conflicting file(s) kept · {:.1} MiB to copy",
        preview.changes,
        preview.conflicts,
        preview.bytes_to_copy as f64 / 1048576.0
    );
    for entry in preview
        .entries
        .iter()
        .filter(|e| e.action != "identical")
        .take(12)
    {
        let detail = if entry.action == "mergeSettings" {
            let keys = entry
                .changed_keys
                .iter()
                .take(8)
                .map(|key| key.chars().take(96).collect::<String>())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "set {}{}",
                keys,
                if entry.changed_keys.len() > 8 {
                    " (and additional common keys)"
                } else {
                    ""
                }
            )
        } else if entry.action == "copyMissing" {
            "add missing file".into()
        } else {
            "keep destination conflict".into()
        };
        text.push_str(&format!("\n{} — {}", entry.path, detail));
    }
    if preview.changes + preview.conflicts > 12 {
        text.push_str("\nAdditional files follow the same add-missing / keep-conflict rule.");
    }
    text
}

fn review() {
    let Some(request) = choice() else { return };
    invalidate();
    ui::with(|w, _| w.global::<InstanceSync>().set_busy(true));
    let frozen = request.clone();
    ui::spawn(api::sync_preview(ui::host(), request), move |result| {
        ui::with(|w, _| w.global::<InstanceSync>().set_busy(false));
        // Navigation or changed selection may invalidate a returning review.
        if choice().as_ref() != Some(&frozen) {
            return;
        }
        ui::with(|w, state| {
            let sync = w.global::<InstanceSync>();
            match result {
                Ok(preview) => {
                    sync.set_message(summary(&preview).into());
                    sync.set_can_apply(preview.changes > 0);
                    state.sync_review = Some((frozen, preview));
                }
                Err(error) => sync.set_message(error.message.into()),
            }
        });
    });
}

fn apply() {
    let Some(current) = choice() else { return };
    let Some((request, preview)) = ui::state(|s| s.sync_review.clone()) else {
        return;
    };
    if request != current {
        invalidate();
        return;
    }
    invalidate();
    ui::with(|w, _| w.global::<InstanceSync>().set_busy(true));
    ui::spawn(
        api::sync_apply(ui::host(), request, preview.fingerprint),
        |result| {
            ui::with(|w, _| {
                let sync = w.global::<InstanceSync>();
                sync.set_busy(false);
                sync.set_message(match result {
                Ok(receipt) => format!("Synced {} file(s); kept {} conflict(s). {}", receipt.copied, receipt.conflicts_kept,
                    if receipt.backup_id.is_some() { "Review and any settings backup are retained in the launcher folder." } else { "No files needed changing." }),
                Err(error) => error.message,
            }.into());
            });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_identifies_changes_without_exposing_setting_values() {
        let p = api::SyncPreview {
            fingerprint: "f".into(),
            entries: vec![api::SyncEntry {
                path: "options.txt".into(),
                action: "mergeSettings".into(),
                changed_keys: vec!["key_jump".into()],
            }],
            changes: 1,
            conflicts: 0,
            bytes_to_copy: 10,
        };
        let summary = summary(&p);
        assert!(summary.contains("set key_jump"));
        assert!(summary.contains("1 change(s)"));
    }
}
