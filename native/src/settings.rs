//! Launcher Settings: appearance (theme, accent, motion), desktop
//! integration, update status and About.

use slint::ComponentHandle;

use crate::api::{self, Appearance, DesktopIntegration, UpdateOverview};
use crate::ui;
use crate::{AccentPicker, AppWindow, Settings, Swatch, Theme, Tone};

const BACKGROUNDS: [&str; 2] = ["simple", "borealis"];

pub fn bind(window: &AppWindow) {
    crate::syncing::bind(window);
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
    settings.on_background_changed(|index| {
        let Some(background) = BACKGROUNDS.get(index as usize) else {
            return;
        };
        let Some((theme, accent, motion)) = ui::state(|s| {
            s.appearance
                .as_ref()
                .map(|a| (a.theme.clone(), a.accent.clone(), a.aurora_motion_speed))
        }) else {
            return;
        };
        submit(theme, accent, (*background).to_owned(), motion);
    });
    bind_accent_picker(window);
    settings.on_discord_changed(save_discord);
    settings.on_discord_connect(|| {
        ui::with(|window, _| window.global::<Settings>().set_discord_busy(true));
        ui::spawn(api::connect_discord(), |result| {
            ui::with(|window, _| window.global::<Settings>().set_discord_busy(false));
            match result {
                Ok(state) => apply_discord(&state),
                Err(error) => ui::error_toast(&error),
            }
        });
    });
    settings.on_reduce_motion_changed(|value| {
        // Reduced motion follows the existing motion preference: 0 means none.
        ui::with(|window, _| window.global::<Theme>().set_reduce_motion(value));
        crate::borealis::sync();
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
    settings.on_review_launcher_update(|| {
        ui::with(|w, _| {
            let settings = w.global::<Settings>();
            settings.set_launcher_dialog_open(true);
            settings.set_launcher_update_detail("Download the signed native preview installer, then choose Exit and install. The installer preserves preview data and restarts the native application. WebView stable uses a separate installation and update authority.".into());
        });
    });
    settings.on_download_launcher_update(|| launcher_operation(false));
    settings.on_install_launcher_update(|| launcher_operation(true));
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
    crate::syncing::load();
    if let Ok(state) = api::discord_state() {
        apply_discord(&state);
    }
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
        let custom = appearance
            .accent
            .get("hex")
            .and_then(|hex| hex.as_str())
            .and_then(crate::theme::parse_color);
        settings.set_has_custom_accent(custom.is_some());
        if let Some(color) = custom {
            settings.set_custom_accent_color(color);
        }
        settings.set_accent_id(accent_id.into());
        settings.set_background_index(
            BACKGROUNDS
                .iter()
                .position(|b| *b == appearance.background)
                .unwrap_or(0) as i32,
        );
        let reduce = appearance.aurora_motion_speed == 0;
        settings.set_reduce_motion(reduce);
        window.global::<Theme>().set_reduce_motion(reduce);
        state.appearance = Some(appearance);
    });
    crate::borealis::sync();
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

// ---- Custom accent picker -------------------------------------------------

fn bind_accent_picker(window: &AppWindow) {
    let picker = window.global::<AccentPicker>();
    picker.on_show(|x, y| {
        let start = ui::state(|s| {
            s.appearance.as_ref().map(|a| {
                a.accent
                    .get("hex")
                    .and_then(|h| h.as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| a.palette.accent.clone())
            })
        })
        .unwrap_or_else(|| "#8b80ff".into());
        let (r, g, b) = parse_hex(&start).unwrap_or((0x8b, 0x80, 0xff));
        let (h, s, v) = rgb_to_hsv(r, g, b);
        ui::with(|window, _| {
            let picker = window.global::<AccentPicker>();
            picker.set_anchor_x(x);
            picker.set_anchor_y(y);
            picker.set_hue(h);
            picker.set_saturation(s);
            picker.set_value(v);
            picker.set_open(true);
        });
        update_from_hsv(false);
    });
    picker.on_set_hue(|hue| {
        ui::with(|window, _| {
            window
                .global::<AccentPicker>()
                .set_hue(hue.clamp(0.0, 360.0))
        });
        update_from_hsv(true);
    });
    picker.on_set_sv(|s, v| {
        ui::with(|window, _| {
            let picker = window.global::<AccentPicker>();
            picker.set_saturation(s.clamp(0.0, 1.0));
            picker.set_value(v.clamp(0.0, 1.0));
        });
        update_from_hsv(true);
    });
    picker.on_hex_edited(|text| {
        let text = text.trim().to_owned();
        match parse_hex(&text) {
            Some((r, g, b)) => {
                let (h, s, v) = rgb_to_hsv(r, g, b);
                ui::with(|window, _| {
                    let picker = window.global::<AccentPicker>();
                    picker.set_hue(h);
                    picker.set_saturation(s);
                    picker.set_value(v);
                });
                preview(&format!("#{r:02x}{g:02x}{b:02x}"), false);
            }
            None => ui::with(|window, _| {
                let picker = window.global::<AccentPicker>();
                picker.set_valid(false);
                picker.set_message("Enter a color as #rrggbb.".into());
            })
            .unwrap_or(()),
        }
    });
    picker.on_apply(|| {
        let Some(hex) = ui::with(|window, _| {
            let picker = window.global::<AccentPicker>();
            picker.get_valid().then(|| picker.get_hex().to_string())
        })
        .flatten() else {
            return;
        };
        ui::with(|window, _| window.global::<AccentPicker>().set_open(false));
        save_appearance(
            None,
            Some(serde_json::json!({ "type": "custom", "hex": hex.to_ascii_lowercase() })),
        );
    });
    picker.on_cancel(|| {
        // Restore the saved accent after any live preview.
        ui::with(|window, state| {
            window.global::<AccentPicker>().set_open(false);
            if let Some(appearance) = &state.appearance {
                crate::theme::apply_palette(window, &appearance.palette);
            }
        });
    });
}

fn update_from_hsv(edit_hex: bool) {
    let Some((h, s, v)) = ui::with(|window, _| {
        let picker = window.global::<AccentPicker>();
        (
            picker.get_hue(),
            picker.get_saturation(),
            picker.get_value(),
        )
    }) else {
        return;
    };
    let (r, g, b) = hsv_to_rgb(h, s, v);
    let hex = format!("#{r:02x}{g:02x}{b:02x}");
    let _ = edit_hex;
    ui::with(|window, _| {
        let picker = window.global::<AccentPicker>();
        let (hr, hg, hb) = hsv_to_rgb(h, 1.0, 1.0);
        picker.set_hue_color(slint::Color::from_rgb_u8(hr, hg, hb));
        picker.set_hex(hex.clone().into());
    });
    preview(&hex, true);
}

/// Validates `hex` with the core appearance rules and previews it live.
fn preview(hex: &str, _from_hsv: bool) {
    use aurora_launcher_lib::appearance::AccentSelection;
    let palette = AccentSelection::Custom {
        hex: hex.to_owned(),
    }
    .palette();
    ui::with(|window, _| {
        let picker = window.global::<AccentPicker>();
        if let Some(color) = crate::theme::parse_color(hex) {
            picker.set_preview(color);
        }
        match palette {
            Ok(palette) => {
                picker.set_valid(true);
                picker.set_message(
                    "Previewing. Aurora derives readable button colors from it.".into(),
                );
                crate::theme::apply_palette(
                    window,
                    &api::AccentPalette {
                        accent: palette.base.as_hex(),
                        accent_strong: palette.strong.as_hex(),
                        accent_hover: palette.hover.as_hex(),
                        accent_pressed: palette.pressed.as_hex(),
                        accent_contrast: palette.on_accent.as_hex(),
                        accent_soft: palette.soft_css(),
                        accent_outline: palette.outline_css(),
                    },
                );
            }
            Err(error) => {
                picker.set_valid(false);
                picker.set_message(format!("Not usable: {}.", error.reason).into());
            }
        }
    });
}

fn parse_hex(text: &str) -> Option<(u8, u8, u8)> {
    let hex = text.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    Some(((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

pub(crate) fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    (hue, saturation, max)
}

pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let channel = |value: f32| ((value + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (channel(r), channel(g), channel(b))
}

// ---- Discord Rich Presence -------------------------------------------------

fn discord_status(state: &api::DiscordState) -> String {
    if !state.configured {
        return "This build has no Discord application ID, so presence cannot connect. Choices are saved; nothing is sent.".into();
    }
    match (state.preferences.enabled, state.connection.as_str()) {
        (false, _) => "Ready. Aurora's Discord application is configured. Turn on Rich Presence to share activity.".into(),
        (true, "connected") => "Connected to the Discord app on this computer.".into(),
        (true, "notDetected") => {
            "Discord is not running. Aurora checks again occasionally while this is on.".into()
        }
        (true, "failed") => "Discord could not be reached. Use Reconnect to try again.".into(),
        (true, "closed") => "Disconnected from Discord.".into(),
        (true, _) => "Waiting for the Discord app on this computer.".into(),
    }
}

pub fn apply_discord(state: &api::DiscordState) {
    ui::with(|window, _| {
        let settings = window.global::<Settings>();
        let p = &state.preferences;
        settings.set_discord_configured(state.configured);
        settings.set_discord_status(discord_status(state).into());
        settings.set_discord_enabled(p.enabled);
        settings.set_discord_instance_name(p.instance_name);
        settings.set_discord_minecraft_version(p.minecraft_version);
        settings.set_discord_platform(p.platform);
        settings.set_discord_aurora_active(p.aurora_active);
        settings.set_discord_elapsed_time(p.elapsed_time);
        settings.set_discord_world(p.world);
        settings.set_discord_server(p.server);
        settings.set_discord_server_address(p.server && p.server_address);
    });
}

fn save_discord() {
    let Some(preferences) = ui::with(|window, _| {
        let s = window.global::<Settings>();
        s.set_discord_busy(true);
        api::DiscordPreferences {
            enabled: s.get_discord_enabled(),
            instance_name: s.get_discord_instance_name(),
            minecraft_version: s.get_discord_minecraft_version(),
            platform: s.get_discord_platform(),
            aurora_active: s.get_discord_aurora_active(),
            elapsed_time: s.get_discord_elapsed_time(),
            world: s.get_discord_world(),
            server: s.get_discord_server(),
            // The address is a sub-choice of sharing the server at all.
            server_address: s.get_discord_server() && s.get_discord_server_address(),
        }
    }) else {
        return;
    };
    ui::spawn(
        api::set_discord_preferences(ui::host(), preferences),
        |result| {
            ui::with(|window, _| window.global::<Settings>().set_discord_busy(false));
            match result {
                Ok(state) => apply_discord(&state),
                Err(error) => {
                    ui::error_toast(&error);
                    if let Ok(state) = api::discord_state() {
                        apply_discord(&state);
                    }
                }
            }
        },
    );
}

#[cfg(test)]
mod color_tests {
    use super::*;

    #[test]
    fn hsv_round_trips_hex_colors() {
        for (r, g, b) in [
            (0x8b, 0x80, 0xff),
            (0, 0, 0),
            (255, 255, 255),
            (0x12, 0xa4, 0x55),
        ] {
            let (h, s, v) = rgb_to_hsv(r, g, b);
            assert_eq!(hsv_to_rgb(h, s, v), (r, g, b));
        }
        assert_eq!(parse_hex("#8B80ff"), Some((0x8b, 0x80, 0xff)));
        assert_eq!(parse_hex("8b80ff"), None);
        assert_eq!(parse_hex("#8b80f"), None);
    }
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
        let launcher_offer = (overview.launcher.availability["kind"] == "updateAvailable")
            .then(|| {
                overview.launcher.availability["candidate"]
                    .as_str()
                    .map(str::to_owned)
            })
            .flatten();
        LAUNCHER_OFFER.with(|slot| {
            if *slot.borrow() != launcher_offer {
                settings.set_launcher_ready_to_install(false);
            }
            *slot.borrow_mut() = launcher_offer.clone();
        });
        settings.set_launcher_update_available(launcher_offer.is_some());
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
    static LAUNCHER_OFFER: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static CLIENT_FINGERPRINT: std::cell::RefCell<Option<(String, String)>> = const { std::cell::RefCell::new(None) };
}

fn launcher_operation(install: bool) {
    let Some(version) = LAUNCHER_OFFER.with(|slot| slot.borrow().clone()) else {
        return;
    };
    let allowed = ui::with(|w, s| {
        let settings = w.global::<Settings>();
        if settings.get_launcher_update_busy() { return false; }
        let running = s.play_busy || s.process.as_ref().is_some_and(|p| matches!(p.status.as_str(), "starting" | "running"));
        if install && (running || w.global::<crate::Activity>().get_active() > 0 || w.global::<crate::Instances>().get_create_busy() || w.global::<crate::Accounts>().get_login_busy()) {
            settings.set_launcher_update_detail("Finish running games, installations and sign-in before exiting to install an update.".into());
            return false;
        }
        settings.set_launcher_update_busy(true);
        settings.set_launcher_update_detail(if install { "Revalidating the signed offer and installer…" } else { "Downloading the native installer…" }.into());
        true
    }).unwrap_or(false);
    if !allowed {
        return;
    }
    let host = ui::host();
    ui::spawn(
        async move {
            if install {
                api::install_launcher_update(host, version).await
            } else {
                api::download_launcher_update(host, version).await
            }
        },
        move |result| {
            ui::with(|w, _| {
                let settings = w.global::<Settings>();
                settings.set_launcher_update_busy(false);
                match result {
                    Ok(_) if install => {
                        let _ = slint::quit_event_loop();
                    }
                    Ok(_) => {
                        settings.set_launcher_ready_to_install(true);
                        settings.set_launcher_update_detail("Signature, hash, package identity and compatibility verified. Exit and install will close Aurora and hand off to the native installer.".into());
                    }
                    Err(error) => {
                        settings.set_launcher_update_detail(error.message.into());
                    }
                }
            });
        },
    );
}

pub fn launcher_download_progress(done: u64, total: Option<u64>) {
    ui::with(|w, _| {
        let text = match total {
            Some(total) => format!(
                "Downloading: {:.1} / {:.1} MiB",
                done as f64 / 1048576.0,
                total as f64 / 1048576.0
            ),
            None => format!("Downloading: {done} bytes"),
        };
        w.global::<Settings>()
            .set_launcher_update_detail(text.into());
    });
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
