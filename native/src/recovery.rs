//! Automatic failure inspection; explicit approval is required to reconcile
//! local pack choices. Never launches or repeatedly retries Minecraft.
use crate::{AppWindow, Nav, Page, Recovery, api, ui};
use slint::ComponentHandle;

pub fn bind(window: &AppWindow) {
    window.global::<Recovery>().on_inspect(|| {
        let id = ui::with(|window, state| {
            if window.global::<Nav>().get_page() == Page::Instances
                && window.global::<Nav>().get_workspace_open()
            {
                state.workspace.as_ref().map(|w| w.id.clone())
            } else {
                state.selected_instance().map(|i| i.id.clone())
            }
        })
        .flatten();
        if let Some(id) = id {
            inspect(id, true);
        }
    });
    window.global::<Recovery>().on_confirm(confirm);
}
fn description(report: &api::RecoveryPreview) -> String {
    let mut lines = vec![report.summary.clone()];
    if !report.kept_settings.is_empty() {
        lines.push(format!(
            "Preserve local settings:\n{}",
            report.kept_settings.join("\n")
        ));
    }
    if !report.removed_mods.is_empty() {
        lines.push(format!(
            "Removed mods (kept removed only if you confirm below):\n{}",
            report.removed_mods.join("\n")
        ));
    }
    if !report.blockers.is_empty() {
        lines.push(format!("Needs attention:\n{}", report.blockers.join("\n")));
    }
    lines.join("\n\n")
}
pub fn inspect(id: String, show: bool) {
    let Some(serial) = ui::with(|window, state| {
        let target_name = state
            .instance(&id)
            .map(|i| i.display_name.clone())
            .unwrap_or_else(|| "Instance".into());
        let recovery = window.global::<Recovery>();
        if !show && recovery.get_open() {
            return None;
        }
        if recovery.get_busy() {
            if state.recovery_target.as_deref() == Some(id.as_str()) {
                if show {
                    recovery.set_open(true);
                }
            } else {
                state.recovery_pending = Some((id.clone(), show));
                if show {
                    recovery.set_open(false);
                }
            }
            return None;
        }
        state.recovery_target = Some(id.clone());
        state.recovery_serial += 1;
        state.recovery_preview = None;
        recovery.set_busy(true);
        recovery.set_allowed(false);
        recovery.set_removed_count(0);
        recovery.set_keep_removed(false);
        recovery.set_title(format!("Recovery: {target_name}").into());
        recovery.set_text("Checking installation, pack edits and mod requirements…".into());
        if show {
            recovery.set_open(true);
        }
        Some(state.recovery_serial)
    })
    .flatten() else {
        return;
    };
    ui::spawn_api(
        api::inspect_recovery(ui::host(), id.clone()),
        move |result| {
            let pending = ui::with(|window, state| {
                if state.recovery_serial != serial {
                    return None;
                }
                let recovery = window.global::<Recovery>();
                recovery.set_busy(false);
                match result {
                    Ok(report) if report.instance_id == id => {
                        recovery.set_text(description(&report).into());
                        recovery.set_allowed(report.can_apply);
                        recovery.set_removed_count(report.removed_mods.len() as i32);
                        state.recovery_preview = Some(report);
                    }
                    Err(error) => {
                        recovery.set_text(error.message.into());
                        recovery.set_allowed(false);
                    }
                    _ => {}
                }
                state.recovery_pending.take()
            })
            .flatten();
            if let Some((id, show)) = pending {
                inspect(id, show);
            }
        },
    );
}
fn confirm() {
    let request = ui::with(|window, state| {
        let recovery = window.global::<Recovery>();
        if recovery.get_busy() || !recovery.get_open() || !recovery.get_allowed() {
            return None;
        }
        let report = state.recovery_preview.as_ref()?;
        if !report.removed_mods.is_empty() && !recovery.get_keep_removed() {
            return None;
        }
        recovery.set_busy(true);
        recovery.set_allowed(false);
        Some((
            report.instance_id.clone(),
            report.fingerprint.clone(),
            recovery.get_keep_removed(),
        ))
    })
    .flatten();
    let Some((id, fingerprint, keep)) = request else {
        return;
    };
    ui::spawn_api(
        api::apply_recovery(ui::host(), id.clone(), fingerprint, keep),
        move |result| {
            ui::with(|window, state| {
                let recovery = window.global::<Recovery>();
                recovery.set_busy(false);
                recovery.set_allowed(false);
                match result {
                    Ok(report) => {
                        recovery.set_text(report.summary.clone().into());
                        recovery.set_removed_count(0);
                        if state
                            .selected_instance()
                            .is_some_and(|i| i.id == report.instance_id)
                        {
                            state.play_error = None;
                        }
                        state.recovery_preview = Some(report);
                    }
                    Err(error) => {
                        recovery.set_text(
                            format!(
                                "{}\n\nClose this review and inspect again before retrying.",
                                error.message
                            )
                            .into(),
                        );
                    }
                }
            });
            crate::instances::reload(move || {
                // Refresh status without discarding an unrelated settings draft.
                crate::home::refresh_readiness();
                if ui::state(|s| s.workspace.as_ref().is_some_and(|w| w.id == id)) {
                    crate::instances::validate();
                }
            });
        },
    );
}
