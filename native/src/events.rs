//! Routes host events (the same names and payloads the WebView receives) to
//! UI controllers. Payloads are parsed into mirror types; unknown events and
//! unparseable payloads are ignored, never fatal.

use serde::Deserialize;
use serde_json::Value;

use crate::api::{LaunchProcess, UpdateOverview};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemProgress {
    phase: String,
    completed_items: u64,
    total_items: u64,
}

#[derive(Deserialize)]
struct Phase {
    phase: String,
}

#[derive(Deserialize)]
struct InstanceProgress {
    phase: String,
    game: Option<ItemProgress>,
}

fn fraction(progress: &ItemProgress) -> f32 {
    if progress.total_items == 0 {
        -1.0
    } else {
        progress.completed_items as f32 / progress.total_items as f32
    }
}

fn install_phase(phase: &str) -> &'static str {
    match phase {
        "resolving" | "resolvingGame" => "Resolving the game",
        "acquiring" | "downloading" => "Downloading and verifying files",
        "materializing" => "Copying verified files",
        "extracting" => "Extracting natives",
        "validating" => "Validating the installation",
        "committing" | "completing" => "Finishing",
        _ => "Installing",
    }
}

fn instance_phase(phase: &str) -> &'static str {
    match phase {
        "resolvingRelease" => "Resolving the Aurora release",
        "resolvingGame" => "Resolving Minecraft and the loader",
        "installingGame" => "Installing Minecraft",
        "installingAurora" => "Installing Aurora Client",
        "validating" => "Validating the instance",
        "completing" => "Finishing",
        _ => "Installing",
    }
}

fn runtime_phase(phase: &str) -> &'static str {
    match phase {
        "acquiring" => "Downloading and verifying Java",
        "materializing" => "Copying verified Java files",
        "validating" => "Validating Java",
        "executing" => "Checking the Java executable",
        "committing" => "Finishing the Java installation",
        _ => "Installing Java",
    }
}

fn counted(step: &str, progress: &ItemProgress) -> String {
    if progress.total_items > 0 {
        format!(
            "{step} · {} of {}",
            progress.completed_items, progress.total_items
        )
    } else {
        format!("{step}…")
    }
}

/// Handles one event on the UI thread.
pub fn dispatch(name: &str, payload: Value) {
    match name {
        "instance-sync-state" => {
            if let Ok(status) = serde_json::from_value::<crate::api::SyncStatus>(payload) {
                crate::syncing::on_status(status);
            }
        }
        "launcher-update-progress" => {
            if let Some(done) = payload["downloadedBytes"].as_u64() {
                crate::settings::launcher_download_progress(done, payload["totalBytes"].as_u64());
            }
        }
        "modpack-progress" => {
            if let Ok(event) = serde_json::from_value::<Phase>(payload) {
                crate::discover::progress(&event.phase);
            }
        }
        "launch-progress" => {
            if let Ok(event) = serde_json::from_value::<Phase>(payload) {
                crate::home::on_launch_progress(&event.phase);
            }
        }
        "launch-state" => {
            if let Ok(process) = serde_json::from_value::<LaunchProcess>(payload) {
                crate::home::on_launch_state(process);
            }
        }
        "install-progress" => {
            if let Ok(event) = serde_json::from_value::<ItemProgress>(payload) {
                let text = counted(install_phase(&event.phase), &event);
                crate::instances::on_create_progress(&text, fraction(&event));
                crate::home::on_progress(text, fraction(&event));
            }
        }
        "instance-progress" => {
            if let Ok(event) = serde_json::from_value::<InstanceProgress>(payload) {
                let step = instance_phase(&event.phase);
                let (text, value) = match &event.game {
                    Some(game) => (
                        format!("{step} · {}", counted(install_phase(&game.phase), game)),
                        fraction(game),
                    ),
                    None => (format!("{step}…"), -1.0),
                };
                crate::instances::on_create_progress(&text, value);
                crate::home::on_progress(text, value);
            }
        }
        "runtime-progress" => {
            if let Ok(event) = serde_json::from_value::<ItemProgress>(payload) {
                crate::home::on_progress(
                    counted(runtime_phase(&event.phase), &event),
                    fraction(&event),
                );
            }
        }
        "auth-progress" => {
            if let Ok(event) = serde_json::from_value::<Phase>(payload) {
                crate::accounts::on_auth_progress(&event.phase);
            }
        }
        "update-status" => {
            if let Ok(overview) = serde_json::from_value::<UpdateOverview>(payload) {
                crate::settings::apply_updates(&overview);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_wording_counts_native_items_only() {
        let event: ItemProgress =
            serde_json::from_value(serde_json::json!({ "phase": "acquiring", "completedItems": 3, "totalItems": 10, "currentItem": null })).unwrap();
        assert_eq!(
            counted(runtime_phase(&event.phase), &event),
            "Downloading and verifying Java · 3 of 10"
        );
        assert!((fraction(&event) - 0.3).abs() < 1e-6);
        let unknown: ItemProgress = serde_json::from_value(
            serde_json::json!({ "phase": "x", "completedItems": 0, "totalItems": 0 }),
        )
        .unwrap();
        assert_eq!(fraction(&unknown), -1.0);
    }
}
