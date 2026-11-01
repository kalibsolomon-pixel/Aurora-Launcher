//! Home: the selected instance, Rust's Play decision, Play and live process
//! state. This module presents `get_play_readiness`; it never computes a
//! second readiness of its own (same rule as `src/lib/launcher/home.ts`).

use slint::ComponentHandle;

use crate::api::{self, ApiError, LaunchProcess, PlayReadiness};
use crate::ui::{self, AppState};
use crate::{AppWindow, BlockerRow, Home, InstanceRow, Tone};

pub fn bind(window: &AppWindow) {
    let home = window.global::<Home>();
    home.on_play(play);
    home.on_select_index(|index| {
        if let Some(id) = ui::state(|s| s.instances().get(index as usize).map(|i| i.id.clone())) {
            crate::instances::select(id);
        }
    });
    home.on_select(|id| crate::instances::select(id.to_string()));
    home.on_refresh(|| {
        refresh_readiness();
    });
    home.on_blocker_action(|code| blocker_action(code.as_str()));
}

/// Instance rows shared by Home and the instance list.
pub fn instance_rows(state: &AppState) -> Vec<InstanceRow> {
    let selected = state
        .launcher
        .as_ref()
        .and_then(|s| s.config.selected_instance_id.clone());
    state
        .instances()
        .iter()
        .map(|instance| {
            let (status, tone) = crate::instances::content_status(instance);
            InstanceRow {
                id: instance.id.clone().into(),
                name: instance.display_name.clone().into(),
                subtitle: instance.subtitle().into(),
                letter: ui::letter(&instance.display_name).into(),
                status: status.into(),
                tone,
                selected: selected.as_deref() == Some(instance.id.as_str()),
                icon: state.icon_path(&instance.id).into(),
            }
        })
        .collect()
}

fn blocker_label(code: &str) -> &'static str {
    match code {
        "launch_authentication_required" => "Sign in",
        "launch_runtime_not_ready" => "Install Java",
        "launch_custom_java_invalid" => "Java settings",
        "launch_instance_not_ready" | "launch_instance_damaged" => "Review recovery",
        "launch_configuration_changed" => "Open instance",
        "launch_mod_incompatible" => "Review mods",
        _ => "",
    }
}

fn blocker_action(code: &str) {
    match code {
        "launch_authentication_required" => crate::accounts::open_dialog(),
        "launch_runtime_not_ready" => {
            let Some(id) = ui::state(|s| s.selected_instance().map(|i| i.id.clone())) else {
                return;
            };
            ui::with(|_, state| state.play_progress = Some("Installing Java…".into()));
            render();
            ui::spawn(api::ensure_runtime(ui::host(), id), |result| {
                ui::with(|_, state| {
                    state.play_progress = None;
                    state.install_progress = None;
                });
                if let Err(error) = result {
                    ui::error_toast(&error);
                }
                refresh_readiness();
            });
        }
        "launch_instance_not_ready" | "launch_instance_damaged" => {
            if let Some(id) = ui::state(|s| s.selected_instance().map(|i| i.id.clone())) {
                crate::recovery::inspect(id, true);
            }
        }
        "launch_mod_incompatible" => {
            if let Some(id) = ui::state(|s| s.selected_instance().map(|i| i.id.clone())) {
                crate::instances::open_workspace(id, crate::instances::TAB_MODS);
            }
        }
        _ => {
            if let Some(id) = ui::state(|s| s.selected_instance().map(|i| i.id.clone())) {
                crate::instances::open_workspace(id, crate::instances::TAB_SETTINGS);
            }
        }
    }
}

/// Re-reads Rust's Play decision for the selected instance and account.
/// Responses for superseded inputs are discarded, and at most one check runs
/// at a time (each deep-verifies installed files); requests made meanwhile
/// collapse into one follow-up check.
pub fn refresh_readiness() {
    let Some((serial, instance, account)) = ui::with(|_, state| {
        state.readiness_serial += 1;
        let instance = state.selected_instance().map(|i| i.id.clone());
        if instance.is_none() {
            state.readiness = None;
        }
        if instance.is_some() {
            if state.readiness_in_flight {
                state.readiness_pending = true;
                return (state.readiness_serial, None, None);
            }
            state.readiness_in_flight = true;
        }
        (
            state.readiness_serial,
            instance,
            state.accounts.selected_account_id.clone(),
        )
    }) else {
        return;
    };
    render();
    let Some(instance) = instance else { return };
    let host = ui::host();
    let launch_instance = instance.clone();
    ui::spawn(
        async move {
            let readiness = api::play_readiness(host, instance, account).await;
            let process = api::launch_state(launch_instance).await.ok();
            (readiness, process)
        },
        move |(readiness, process): (Result<PlayReadiness, ApiError>, Option<LaunchProcess>)| {
            let follow_up = ui::with(|_, state| {
                state.readiness_in_flight = false;
                std::mem::take(&mut state.readiness_pending)
            })
            .unwrap_or(false);
            if follow_up {
                refresh_readiness();
                return;
            }
            let recheck = ui::with(|_, state| {
                if state.readiness_serial != serial {
                    return false;
                }
                let stale_running = readiness
                    .as_ref()
                    .is_ok_and(|d| matches!(d.process_status.as_str(), "running" | "starting"))
                    && process
                        .as_ref()
                        .or(state.process.as_ref())
                        .is_some_and(|p| matches!(p.status.as_str(), "exited" | "failed"));
                match readiness {
                    Ok(decision) => {
                        state.readiness = Some(decision);
                    }
                    Err(error) => {
                        state.readiness = None;
                        state.play_error = Some(error.message);
                    }
                }
                if let Some(process) = process {
                    state.process = Some(process);
                }
                if stale_running && state.readiness_rechecks < 5 {
                    state.readiness_rechecks += 1;
                    true
                } else {
                    if !stale_running {
                        state.readiness_rechecks = 0;
                    }
                    false
                }
            })
            .unwrap_or(false);
            render();
            continue_requested_play();
            if let Some(id) = ui::state(|s| {
                s.readiness
                    .as_ref()
                    .filter(|d| matches!(d.instance_status.as_str(), "installing" | "damaged"))
                    .map(|d| d.instance_id.clone())
            }) {
                if !ui::state(|s| {
                    s.recovery_preview
                        .as_ref()
                        .is_some_and(|p| p.instance_id == id)
                }) {
                    crate::recovery::inspect(id, false);
                }
            }
            if recheck {
                // The supervisor reported exit before its readiness state settled.
                slint::Timer::single_shot(std::time::Duration::from_millis(750), refresh_readiness);
            }
        },
    );
}

/// The workspace Play button selects its exact target, then waits for the
/// winning readiness check before launching or presenting recovery.
pub fn request_play(id: String) {
    ui::navigate(crate::Page::Home);
    ui::with(|_, state| {
        state.pending_play = Some(id.clone());
        state.readiness = None;
    });
    ui::spawn_api(
        api::select_instance(ui::host(), id.clone()),
        move |result| match result {
            Ok(()) => crate::instances::reload(refresh_readiness),
            Err(error) => {
                ui::with(|_, state| {
                    state.pending_play = None;
                    state.play_error = Some(error.message);
                });
                render();
            }
        },
    );
}
fn continue_requested_play() {
    let action = ui::with(|window, state| {
        let id = state.pending_play.as_ref()?;
        if window.global::<crate::Nav>().get_page() != crate::Page::Home {
            state.pending_play = None;
            return None;
        }
        let decision = state.readiness.as_ref()?;
        if decision.instance_id != *id {
            return None;
        }
        let ready = decision.ready || decision.can_prepare;
        let blocker = decision.blockers.first().map(|b| b.code.clone());
        Some((state.pending_play.take()?, ready, blocker))
    })
    .flatten();
    match action {
        Some((_, true, _)) => play(),
        Some((id, false, Some(code)))
            if matches!(
                code.as_str(),
                "launch_instance_not_ready" | "launch_instance_damaged"
            ) =>
        {
            crate::recovery::inspect(id, true)
        }
        Some((_, false, Some(code))) => blocker_action(&code),
        _ => {}
    }
}

fn play() {
    let request = ui::with(|_, state| {
        let instance = state.selected_instance()?.id.clone();
        let account = state.accounts.selected_account_id.clone()?;
        let decision = state.readiness.as_ref()?;
        if state.play_busy
            || !(decision.ready || decision.can_prepare)
            || decision.instance_id != instance
            || decision.account_id.as_deref() != Some(account.as_str())
        {
            return None;
        }
        state.play_busy = true;
        state.play_error = None;
        state.install_progress = None;
        state.play_progress = Some("Checking readiness…".into());
        Some((instance, account))
    })
    .flatten();
    let Some((instance, account)) = request else {
        return;
    };
    render();
    let failed_id = instance.clone();
    ui::spawn_api(api::play(ui::host(), instance, account), move |result| {
        let failed = result.is_err();
        ui::with(|_, state| {
            state.play_busy = false;
            state.play_progress = None;
            match result {
                Ok(process) => state.process = Some(process),
                Err(error) => state.play_error = Some(error.message),
            }
        });
        render();
        if failed {
            crate::recovery::inspect(failed_id, false);
        }
        refresh_readiness();
    });
}

/// `launch-progress` event.
pub fn on_launch_progress(phase: &str) {
    let label = match phase {
        "checkingPreconditions" => "Checking readiness…",
        "preparingJava" => "Preparing managed Java…",
        "resolvingLaunch" => "Preparing the game…",
        "restoringSession" => "Restoring the Minecraft session…",
        "assemblingArguments" => "Assembling the launch…",
        "startingProcess" => "Starting Minecraft…",
        _ => "Preparing…",
    };
    ui::with(|_, state| {
        if state.play_busy {
            state.play_progress = Some(label.into());
        }
    });
    render();
}

/// `launch-state` event: the supervised child changed state.
pub fn on_launch_state(process: LaunchProcess) {
    let finished = matches!(process.status.as_str(), "exited" | "failed");
    let failed_id = (process.status == "failed").then(|| process.instance_id.clone());
    ui::with(|_, state| state.process = Some(process));
    render();
    if let Some(id) = failed_id {
        crate::recovery::inspect(id, false);
    }
    if finished {
        refresh_readiness();
    }
}

/// `install-progress` / `instance-progress` / `runtime-progress` events.
pub fn on_progress(text: String, fraction: f32) {
    ui::with(|_, state| state.install_progress = Some((text, fraction)));
    render();
}

pub fn clear_progress() {
    ui::with(|_, state| state.install_progress = None);
    render();
}

pub fn render() {
    ui::with(|window, state| {
        let home = window.global::<Home>();
        home.set_instances(ui::model(instance_rows(state)));
        home.set_instance_names(ui::model(
            state
                .instances()
                .iter()
                .map(|i| i.display_name.clone().into())
                .collect(),
        ));
        home.set_instance_index(
            state
                .instances()
                .iter()
                .position(|i| {
                    state
                        .selected_instance()
                        .is_some_and(|selected| selected.id == i.id)
                })
                .map(|i| i as i32)
                .unwrap_or(-1),
        );
        let Some(instance) = state.selected_instance().cloned() else {
            home.set_has_instance(false);
            home.set_can_play(false);
            home.set_status("Choose an instance".into());
            home.set_tone(Tone::Neutral);
            home.set_subtitle("".into());
            home.set_blockers(ui::model(Vec::new()));
            home.set_busy(false);
            home.set_checking(false);
            home.set_running(false);
            home.set_play_label("Play".into());
            home.set_progress_text("".into());
            home.set_message("".into());
            return;
        };
        home.set_has_instance(true);
        home.set_name(instance.display_name.clone().into());
        home.set_icon(state.icon_path(&instance.id).into());
        home.set_subtitle(instance.subtitle().into());
        home.set_letter(ui::letter(&instance.display_name).into());

        let decision = state
            .readiness
            .as_ref()
            .filter(|d| d.instance_id == instance.id);
        let child = state
            .process
            .as_ref()
            .filter(|p| p.instance_id == instance.id);
        // The supervised child's own snapshot is newer than a readiness
        // decision taken while it ran, so it decides process state when present.
        let child_finished =
            child.is_some_and(|c| matches!(c.status.as_str(), "exited" | "failed"));
        let decision_active =
            |status: &str| !child_finished && decision.is_some_and(|d| d.process_status == status);
        let running = child.is_some_and(|c| c.status == "running") || decision_active("running");
        let starting = !running
            && (child.is_some_and(|c| c.status == "starting")
                || decision_active("starting")
                || state.play_busy);
        let checking = decision.is_none() && state.play_error.is_none();
        let ready = decision.is_some_and(|d| d.ready);
        let preparable = decision.is_some_and(|d| d.can_prepare);

        let (status, tone) = if running {
            ("Running", Tone::Success)
        } else if starting {
            ("Starting…", Tone::Working)
        } else if checking {
            ("Checking…", Tone::Working)
        } else if ready {
            ("Ready", Tone::Success)
        } else if preparable {
            ("Play will prepare Java", Tone::Neutral)
        } else {
            ("Needs attention", Tone::Warning)
        };
        home.set_status(status.into());
        home.set_tone(tone);
        home.set_checking(checking);
        home.set_running(running);
        home.set_busy(starting);
        home.set_play_label(
            if running {
                "Running"
            } else if starting {
                "Starting…"
            } else {
                if preparable { "Prepare & play" } else { "Play" }
            }
            .into(),
        );
        home.set_can_play(
            (ready || preparable)
                && !running
                && !starting
                && !checking
                && state.accounts.selected_account_id.is_some(),
        );

        let blockers: Vec<BlockerRow> = decision
            .filter(|_| !running && !starting)
            .map(|d| {
                d.blockers
                    .iter()
                    .map(|b| BlockerRow {
                        code: b.code.clone().into(),
                        message: b.message.clone().into(),
                        action: blocker_label(&b.code).into(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        home.set_blockers(ui::model(blockers));

        let (progress_text, progress) = match (&state.play_progress, &state.install_progress) {
            (Some(text), _) => (text.clone(), -1.0),
            (None, Some((text, fraction))) => (text.clone(), *fraction),
            _ => (String::new(), -1.0),
        };
        home.set_progress_text(progress_text.into());
        home.set_progress(progress);

        let (message, tone) = if let Some(error) = &state.play_error {
            (error.clone(), Tone::Error)
        } else if let Some(child) = child {
            match child.status.as_str() {
                "failed" => (
                    child
                        .diagnostic
                        .as_ref()
                        .map(|d| d.message.clone())
                        .or_else(|| child.message.clone())
                        .unwrap_or_else(|| "Minecraft exited unsuccessfully.".into()),
                    Tone::Error,
                ),
                "exited" => (
                    child
                        .exit_code
                        .map(|code| format!("Last game exited with code {code}."))
                        .unwrap_or_default(),
                    Tone::Neutral,
                ),
                _ => (String::new(), Tone::Neutral),
            }
        } else {
            (String::new(), Tone::Neutral)
        };
        home.set_message(message.into());
        home.set_message_tone(tone);
    });
}
