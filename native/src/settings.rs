//! Launcher Settings: appearance (theme, accent, motion), desktop
//! integration, update status and About.

use slint::ComponentHandle;

use crate::api::{self, Appearance, DesktopIntegration, UpdateOverview};
use crate::ui;
use crate::{AppWindow, Settings, Swatch, Theme, Tone};

pub fn bind(window: &AppWindow) {
    let settings = window.global::<Settings>();
    settings.on_theme_changed(|index| {
        let theme = ui::state(|s| {
            s.appearance
                .as_ref()
                .and_then(|a| a.themes.get(index as usize).map(|t| t.id.clone()))
        });
        if let Some(theme) = theme {
            save_appearance(Some(theme), None);
        }
    });
    settings.on_accent_changed(|id| {
        save_appearance(
            None,
            Some(serde_json::json!({ "type": "preset", "id": id.as_str() })),
        );
    });
    settings.on_custom_accent_apply(|hex| {
        save_appearance(
            None,
            Some(serde_json::json!({ "type": "custom", "hex": hex.trim() })),
        );
    });
    settings.on_reduce_motion_changed(|value| {
        // Reduced motion follows the existing motion preference: 0 means none.
        ui::with(|window, _| window.global::<Theme>().set_reduce_motion(value));
        save_motion(if value { 0 } else { 50 });
    });
    settings.on_toggle_desktop_shortcut(|| {
        let present = ui::with(|window, _| window.global::<Settings>().get_desktop_present())
            .unwrap_or(false);
        shortcut_operation(if present {
            api::remove_desktop_shortcut().boxed()
        } else {
            api::create_desktop_shortcut().boxed()
        });
    });
    settings.on_create_start_menu_shortcut(|| {
        shortcut_operation(api::create_start_menu_shortcut().boxed())
    });
    settings.on_open_launcher_folder(|| {
        ui::spawn(api::open_launcher_folder(ui::host()), |result| {
            if let Err(error) = result {
                ui::error_toast(&error);
            }
        });
    });
    settings.on_review_client_update(review_client_update);
    settings.on_client_update_apply(apply_client_update);
    settings.on_check_updates(|| {
        ui::with(|window, _| window.global::<Settings>().set_update_busy(true));
        ui::spawn(api::check_for_updates(ui::host()), |result| {
            ui::with(|window, _| window.global::<Settings>().set_update_busy(false));
            match result {
                Ok(overview) => apply_updates(&overview),
                Err(error) => ui::error_toast(&error),
            }
        });
    });
}

trait Boxed: std::future::Future + Send + Sized + 'static {
    fn boxed(self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>> {
        Box::pin(self)
    }
}
impl<F: std::future::Future + Send + 'static> Boxed for F {}

fn shortcut_operation(
    work: std::pin::Pin<
        Box<dyn std::future::Future<Output = api::ApiResult<DesktopIntegration>> + Send>,
    >,
) {
    ui::with(|window, _| window.global::<Settings>().set_shortcuts_busy(true));
    ui::spawn(work, |result| {
        ui::with(|window, _| window.global::<Settings>().set_shortcuts_busy(false));
        match result {
            Ok(integration) => apply_desktop(&integration),
            Err(error) => ui::error_toast(&error),
        }
    });
}

pub fn load() {
    ui::spawn(api::desktop_integration(), |result| {
        if let Ok(integration) = result {
            apply_desktop(&integration);
        }
    });
}

fn shortcut_text(state: &str, managed_by: &str) -> String {
    match (state, managed_by) {
        ("present", "installer") => "Created by the Aurora installer.".into(),
        ("present", _) => "Present.".into(),
        ("absent", _) => "Not created.".into(),
        ("conflict", _) => "Another shortcut with this name exists; Aurora leaves it alone.".into(),
        _ => "Unavailable right now.".into(),
    }
}

fn apply_desktop(integration: &DesktopIntegration) {
    ui::with(|window, _| {
        let settings = window.global::<Settings>();
        settings.set_shortcuts_supported(integration.supported && integration.manageable);
        settings.set_desktop_present(integration.desktop_shortcut.state == "present");
        settings.set_start_menu_present(integration.start_menu_shortcut.state == "present");
        settings.set_desktop_state(
            shortcut_text(
                &integration.desktop_shortcut.state,
                &integration.desktop_shortcut.managed_by,
            )
            .into(),
        );
        settings.set_start_menu_state(
            shortcut_text(
                &integration.start_menu_shortcut.state,
                &integration.start_menu_shortcut.managed_by,
            )
            .into(),
        );
    });
}

pub fn apply_appearance(appearance: Appearance) {
    ui::with(|window, state| {
        crate::theme::apply(window, &appearance);
        let settings = window.global::<Settings>();
        settings.set_theme_labels(ui::model(
            appearance
                .themes
                .iter()
                .map(|t| t.label.clone().into())
                .collect(),
        ));
        settings.set_theme_descriptions(ui::model(
            appearance
                .themes
                .iter()
                .map(|t| t.description.clone().into())
                .collect(),
        ));
        settings.set_theme_index(
            appearance
                .themes
                .iter()
                .position(|t| t.id == appearance.theme)
                .unwrap_or(0) as i32,
        );
        let swatches: Vec<Swatch> = appearance
            .accents
            .iter()
            .map(|accent| Swatch {
                id: accent.id.clone().into(),
                label: accent.label.clone().into(),
                color: crate::theme::parse_color(&accent.hex).unwrap_or_default(),
            })
            .collect();
        settings.set_accents(ui::model(swatches));
        let accent_id = appearance
            .accent
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or("custom")
            .to_owned();
        if let Some(hex) = appearance.accent.get("hex").and_then(|hex| hex.as_str()) {
            settings.set_custom_accent(hex.into());
        }
        settings.set_accent_id(accent_id.into());
        let reduce = appearance.aurora_motion_speed == 0;
        settings.set_reduce_motion(reduce);
        window.global::<Theme>().set_reduce_motion(reduce);
        state.appearance = Some(appearance);
    });
}

fn save_appearance(theme: Option<String>, accent: Option<serde_json::Value>) {
    let Some((theme, accent, background, motion)) = ui::state(|s| {
        let current = s.appearance.as_ref()?;
        Some((
            theme.unwrap_or_else(|| current.theme.clone()),
            accent.unwrap_or_else(|| current.accent.clone()),
            current.background.clone(),
            current.aurora_motion_speed,
        ))
    }) else {
        return;
    };
    submit(theme, accent, background, motion);
}

fn save_motion(motion: u8) {
    let Some((theme, accent, background)) = ui::state(|s| {
        s.appearance
            .as_ref()
            .map(|a| (a.theme.clone(), a.accent.clone(), a.background.clone()))
    }) else {
        return;
    };
    submit(theme, accent, background, motion);
}

fn submit(theme: String, accent: serde_json::Value, background: String, motion: u8) {
    ui::with(|window, _| {
        let settings = window.global::<Settings>();
        settings.set_appearance_busy(true);
        settings.set_appearance_error("".into());
    });
    ui::spawn(
        api::set_appearance(ui::host(), theme, accent, background, motion),
        |result| {
            ui::with(|window, _| window.global::<Settings>().set_appearance_busy(false));
            match result {
                Ok(appearance) => apply_appearance(appearance),
                Err(error) => {
                    ui::with(|window, _| {
                        window
                            .global::<Settings>()
                            .set_appearance_error(error.message.clone().into())
                    });
                    ui::toast(error.message, Tone::Error);
                }
            }
        },
    );
}

fn availability_text(value: &serde_json::Value) -> String {
    match value.get("kind").and_then(|s| s.as_str()) {
        Some("upToDate") => "Up to date.".into(),
        Some("updateAvailable") => format!(
            "Version {} is available.",
            value
                .get("candidate")
                .and_then(|c| c.as_str())
                .unwrap_or("?")
        ),
        Some("unavailable") => value
            .get("reason")
            .and_then(|r| r.as_str())
            .unwrap_or("Update information is unavailable.")
            .to_owned(),
        Some("notApplicable") => value
            .get("reason")
            .and_then(|r| r.as_str())
            .unwrap_or("Not applicable.")
            .to_owned(),
        Some("notChecked") | None => "Not checked yet.".into(),
        Some(other) => other.to_owned(),
    }
}

pub fn apply_updates(overview: &UpdateOverview) {
    ui::with(|window, state| {
        let settings = window.global::<Settings>();
        let available =
            overview.client.get("kind").and_then(|k| k.as_str()) == Some("updateAvailable");
        state.client_update = match (&overview.client_instance_id, available) {
            (Some(id), true) => Some((
                id.clone(),
                overview.client_instance_name.clone().unwrap_or_default(),
            )),
            _ => None,
        };
        settings.set_client_update_available(state.client_update.is_some());
        settings.set_launcher_update(
            format!(
                "Version {}. {}",
                overview.launcher.installed_version,
                availability_text(&overview.launcher.availability)
            )
            .into(),
        );
        let client = match (
            &overview.client_instance_name,
            &overview.client_installed_version,
        ) {
            (Some(name), Some(version)) => format!(
                "{name}: Aurora {version}. {}",
                availability_text(&overview.client)
            ),
            _ => availability_text(&overview.client),
        };
        settings.set_client_update(client.into());
    });
}

thread_local! {
    static CLIENT_FINGERPRINT: std::cell::RefCell<Option<(String, String)>> = const { std::cell::RefCell::new(None) };
}

/// Fetches the fingerprinted, read-only Aurora Client update preview and
/// shows it for confirmation. Nothing changes until the user confirms.
fn review_client_update() {
    let Some((instance, name)) = ui::state(|s| s.client_update.clone()) else {
        return;
    };
    ui::with(|window, _| {
        let settings = window.global::<Settings>();
        settings.set_client_dialog_title(format!("Update Aurora Client in {name}").into());
        settings.set_client_dialog_detail("Checking the published release…".into());
        settings.set_client_dialog_can_apply(false);
        settings.set_client_dialog_busy(false);
        settings.set_client_dialog_open(true);
    });
    let id = instance.clone();
    ui::spawn(
        api::preview_client_update(ui::host(), instance),
        move |result| {
            ui::with(|window, _| {
                let settings = window.global::<Settings>();
                match result {
                    Ok(preview) => {
                        let mut lines = vec![];
                        match (&preview.candidate, preview.outcome.as_str()) {
                        (Some(candidate), "updateAvailable") => lines.push(format!(
                            "Aurora {} → {} (Minecraft {}). The download is verified against its published SHA-256 before anything is replaced; your mods, worlds and settings are kept.",
                            preview.installed_version, candidate.version, candidate.minecraft_version
                        )),
                        _ => lines.push(format!("Aurora {} is up to date.", preview.installed_version)),
                    }
                        if let Some(notes) =
                            preview.candidate.as_ref().and_then(|c| c.notes.clone())
                        {
                            lines.push(notes);
                        }
                        lines.extend(preview.warnings.iter().cloned());
                        lines.extend(preview.blockers.iter().map(|b| format!("Blocked: {b}")));
                        let can_apply =
                            preview.outcome == "updateAvailable" && preview.blockers.is_empty();
                        CLIENT_FINGERPRINT.with(|slot| {
                            *slot.borrow_mut() =
                                can_apply.then(|| (id.clone(), preview.fingerprint.clone()))
                        });
                        settings.set_client_dialog_detail(lines.join("\n\n").into());
                        settings.set_client_dialog_can_apply(can_apply);
                    }
                    Err(error) => settings.set_client_dialog_detail(error.message.into()),
                }
            });
        },
    );
}

fn apply_client_update() {
    let Some((instance, fingerprint)) = CLIENT_FINGERPRINT.with(|slot| slot.borrow_mut().take())
    else {
        return;
    };
    ui::with(|window, _| window.global::<Settings>().set_client_dialog_busy(true));
    ui::spawn(
        api::apply_client_update(ui::host(), instance, fingerprint),
        |result| {
            ui::with(|window, _| {
                let settings = window.global::<Settings>();
                settings.set_client_dialog_busy(false);
                settings.set_client_dialog_can_apply(false);
                match &result {
                    Ok(_) => settings.set_client_dialog_open(false),
                    Err(error) => settings.set_client_dialog_detail(error.message.clone().into()),
                }
            });
            if let Ok(instance) = result {
                ui::toast(
                    format!("Aurora Client updated in {}.", instance.display_name),
                    Tone::Success,
                );
                crate::instances::reload(crate::home::refresh_readiness);
            }
        },
    );
}
