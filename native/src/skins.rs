//! Skins: the selected account's current skin, the local skin library and
//! owned capes. Reads and changes go through the cosmetics service, which
//! talks to Minecraft services with the Rust-owned session; the UI receives
//! only decoded pixels, names and ids.

use slint::ComponentHandle;

use crate::api::{self, Cosmetics, SkinPreset};
use crate::ui;
use crate::{AppWindow, CapeRow, SkinRow, Skins, Tone};

pub fn bind(window: &AppWindow) {
    let skins = window.global::<Skins>();
    skins.on_refresh(load);
    skins.on_save_current(|| {
        let Some(account) = selected_account() else {
            return;
        };
        mutate(api::save_current_skin(ui::host(), account), |_| {
            ui::toast("Saved to your skin library.", Tone::Success);
            load_presets();
        });
    });
    skins.on_apply(|preset| {
        let Some(account) = selected_account() else {
            return;
        };
        let model = ui::state(|s| {
            s.skin_presets
                .iter()
                .find(|p| p.id == preset.as_str())
                .map(|p| p.model.clone())
        });
        let Some(model) = model else { return };
        mutate(
            api::apply_skin_preset(ui::host(), account, preset.to_string(), model),
            |cosmetics| {
                apply_cosmetics(cosmetics);
                ui::toast("Skin updated.", Tone::Success);
            },
        );
    });
    skins.on_remove(|preset| {
        mutate(
            api::remove_skin_preset(ui::host(), preset.to_string()),
            |_| {
                load_presets();
            },
        );
    });
    skins.on_select_cape(|cape| {
        let Some(account) = selected_account() else {
            return;
        };
        mutate(
            api::select_cape(ui::host(), account, cape.to_string()),
            apply_cosmetics,
        );
    });
    skins.on_disable_cape(|| {
        let Some(account) = selected_account() else {
            return;
        };
        mutate(api::disable_cape(ui::host(), account), apply_cosmetics);
    });
}

fn selected_account() -> Option<String> {
    ui::state(|s| {
        let id = s.accounts.selected_account_id.clone()?;
        s.accounts
            .accounts
            .iter()
            .any(|a| a.account_id == id && a.status == "signedIn")
            .then_some(id)
    })
}

/// Runs one cosmetics change with the page marked busy; failures are shown
/// and the authoritative state is re-read.
fn mutate<T: Send + 'static>(
    work: impl std::future::Future<Output = api::ApiResult<T>> + Send + 'static,
    done: impl FnOnce(T) + Send + 'static,
) {
    ui::with(|window, _| {
        let skins = window.global::<Skins>();
        skins.set_busy(true);
        skins.set_error("".into());
    });
    ui::spawn(work, move |result| {
        ui::with(|window, _| window.global::<Skins>().set_busy(false));
        match result {
            Ok(value) => done(value),
            Err(error) => {
                ui::with(|window, _| window.global::<Skins>().set_error(error.message.into()));
                load();
            }
        }
    });
}

/// Called on navigation and account changes.
pub fn load() {
    let account = selected_account();
    let name = ui::state(|s| {
        s.accounts
            .accounts
            .iter()
            .find(|a| Some(&a.account_id) == account.as_ref())
            .map(|a| a.minecraft_name.clone())
            .unwrap_or_default()
    });
    ui::with(|window, _| {
        let skins = window.global::<Skins>();
        skins.set_has_account(account.is_some());
        skins.set_account_name(name.into());
        skins.set_error("".into());
    });
    let Some(account) = account else { return };
    ui::with(|window, _| window.global::<Skins>().set_loading(true));
    ui::spawn(api::cosmetics(ui::host(), account), |result| {
        ui::with(|window, _| window.global::<Skins>().set_loading(false));
        match result {
            Ok(cosmetics) => apply_cosmetics(cosmetics),
            Err(error) => {
                ui::with(|window, _| window.global::<Skins>().set_error(error.message.into()));
            }
        }
    });
    load_presets();
}

fn apply_cosmetics(cosmetics: Cosmetics) {
    ui::with(|window, _| {
        let skins = window.global::<Skins>();
        let image = cosmetics
            .current_skin
            .as_ref()
            .and_then(|head| ui::rgba_image(&head.rgba));
        skins.set_has_current(cosmetics.has_current_skin && image.is_some());
        skins.set_current(image.unwrap_or_default());
        skins.set_current_model(
            model_label(cosmetics.current_skin_model.as_deref().unwrap_or("classic")).into(),
        );
        let capes: Vec<CapeRow> = cosmetics
            .capes
            .iter()
            .map(|cape| CapeRow {
                id: cape.id.clone().into(),
                name: cape.name.clone().into(),
                selected: cape.selected,
            })
            .collect();
        skins.set_capes(ui::model(capes));
    });
}

fn model_label(model: &str) -> &'static str {
    if model == "slim" { "Slim" } else { "Classic" }
}

fn load_presets() {
    ui::spawn(api::skin_presets(ui::host()), |result| match result {
        Ok(presets) => {
            ui::with(|_, state| state.skin_presets = presets.clone());
            render_presets();
            for preset in presets {
                let id = preset.id.clone();
                ui::spawn(api::skin_thumbnail(ui::host(), id.clone()), move |result| {
                    if let Ok(Some(thumbnail)) = result {
                        if let Some(image) = ui::rgba_image(&thumbnail.rgba) {
                            ui::with(|_, state| state.skin_thumbnails.insert(id, image));
                            render_presets();
                        }
                    }
                });
            }
        }
        Err(error) => {
            ui::with(|window, _| window.global::<Skins>().set_error(error.message.into()));
        }
    });
}

fn render_presets() {
    ui::with(|window, state| {
        let rows: Vec<SkinRow> = state
            .skin_presets
            .iter()
            .map(|preset: &SkinPreset| {
                let thumb = state.skin_thumbnails.get(&preset.id).cloned();
                SkinRow {
                    id: preset.id.clone().into(),
                    name: if preset.favorite {
                        format!("★ {}", preset.name)
                    } else {
                        preset.name.clone()
                    }
                    .into(),
                    model: model_label(&preset.model).into(),
                    has_thumb: thumb.is_some(),
                    thumb: thumb.unwrap_or_default(),
                }
            })
            .collect();
        window.global::<Skins>().set_presets(ui::model(rows));
    });
}
