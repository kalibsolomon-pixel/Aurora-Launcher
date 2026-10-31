//! Skins: the selected account's current skin, the local skin library and
//! owned capes. Reads and changes go through the cosmetics service, which
//! talks to Minecraft services with the Rust-owned session; the UI receives
//! only decoded pixels, names and ids.

use slint::ComponentHandle;

use crate::api::{self, Cape, Cosmetics, HeadAvatar, SkinPreset, Texture};
use crate::ui;
use crate::{AppWindow, CapeRow, SkinRow, Skins, Tone};

pub struct PreviewState {
    pub serial: u64,
    account_epoch: u64,
    cached_account: Option<String>,
    library_loaded: bool,
    library_epoch: u64,
    skin: Option<HeadAvatar>,
    current: Option<HeadAvatar>,
    capes: Vec<Cape>,
    cape: Option<Texture>,
    cape_id: String,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    render_pending: bool,
    pending_apply: Option<(String, String, String)>,
}
impl Default for PreviewState {
    fn default() -> Self {
        Self {
            serial: 0,
            account_epoch: 0,
            cached_account: None,
            library_loaded: false,
            library_epoch: 0,
            skin: None,
            current: None,
            capes: Vec::new(),
            cape: None,
            cape_id: String::new(),
            yaw: crate::player::DEFAULT_YAW,
            pitch: 0.1,
            zoom: 1.,
            render_pending: false,
            pending_apply: None,
        }
    }
}

fn preview_skin(id: slint::SharedString) {
    let serial = ui::with(|_, s| {
        s.skin_preview.serial += 1;
        s.skin_preview.serial
    })
    .unwrap_or(0);
    let name = ui::state(|s| {
        s.skin_presets
            .iter()
            .find(|p| p.id == id.as_str())
            .map(|p| p.name.clone())
    })
    .unwrap_or_default();
    ui::spawn(
        api::skin_preview(ui::host(), id.to_string()),
        move |result| {
            ui::with(|w, s| {
                if s.skin_preview.serial != serial {
                    return;
                }
                match result {
                    Ok(skin) => {
                        s.skin_preview.skin = Some(skin);
                        w.global::<Skins>()
                            .set_preview_label(format!("Preview: {name}").into());
                    }
                    Err(e) => w.global::<Skins>().set_error(e.message.into()),
                }
            });
            schedule_render();
        },
    );
}

/// A one-shot render coalesces pointer motion; no work is scheduled at idle.
fn schedule_render() {
    let pending = ui::with(|_, s| {
        if s.skin_preview.render_pending {
            return false;
        }
        s.skin_preview.render_pending = true;
        true
    })
    .unwrap_or(false);
    if !pending {
        return;
    }
    slint::Timer::single_shot(std::time::Duration::from_millis(16), || {
        ui::with(|w, s| {
            let p = &mut s.skin_preview;
            p.render_pending = false;
            if w.global::<crate::Nav>().get_page() != crate::Page::Skins {
                return;
            }
            let pixels = crate::player::render(
                p.skin.as_ref(),
                p.cape.as_ref(),
                p.yaw,
                p.pitch,
                p.zoom,
                w.global::<Skins>().get_outer_layers(),
            );
            w.global::<Skins>().set_player(
                ui::rectangular_image(&pixels, crate::player::WIDTH, crate::player::HEIGHT)
                    .unwrap_or_default(),
            );
        });
    });
}

fn render_capes() {
    ui::with(|w, s| {
        let rows = s
            .skin_preview
            .capes
            .iter()
            .map(|cape| {
                let picture = cape.preview.as_ref().and_then(cape_thumbnail);
                CapeRow {
                    id: cape.id.clone().into(),
                    name: cape.name.clone().into(),
                    selected: cape.selected,
                    previewed: cape.id == s.skin_preview.cape_id,
                    picture: picture.clone().unwrap_or_default(),
                    has_picture: picture.is_some(),
                }
            })
            .collect();
        w.global::<Skins>().set_capes(ui::model(rows));
    });
}

fn cape_thumbnail(t: &Texture) -> Option<slint::Image> {
    if !matches!((t.width, t.height), (64, 32) | (64, 64) | (128, 64))
        || t.rgba.len() != (t.width * t.height * 4) as usize
    {
        return None;
    }
    let scale = t.width / 64;
    let mut pixels = Vec::with_capacity(10 * 16 * 4);
    for y in 1..17 {
        for x in 12..22 {
            let i = ((y * scale * t.width + x * scale) * 4) as usize;
            pixels.extend_from_slice(&t.rgba[i..i + 4]);
        }
    }
    ui::rectangular_image(&pixels, 10, 16)
}

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
    skins.on_import_file(|| {
        let Some(model) = ui::with(|w, _| {
            let skins = w.global::<Skins>();
            if skins.get_busy() {
                return None;
            }
            skins.set_busy(true);
            skins.set_error("".into());
            Some(
                if skins.get_import_model() == 1 {
                    "slim"
                } else {
                    "classic"
                }
                .to_owned(),
            )
        })
        .flatten() else {
            return;
        };
        let host = ui::host();
        ui::spawn(
            async move {
                match crate::skin_import::choose().await? {
                    Some(bytes) => api::import_skin(host, "Imported skin".into(), model, bytes)
                        .await
                        .map(Some),
                    None => Ok(None),
                }
            },
            |result| {
                ui::with(|w, _| w.global::<Skins>().set_busy(false));
                match result {
                    Ok(Some(_)) => {
                        load_presets();
                        ui::toast("Skin imported to your local library.", Tone::Success);
                    }
                    Ok(None) => {}
                    Err(e) => {
                        ui::with(|w, _| w.global::<Skins>().set_error(e.message.into()));
                    }
                }
            },
        );
    });
    skins.on_apply(|preset| {
        let Some(account) = selected_account() else {
            return;
        };
        ui::with(|w, s| {
            if w.global::<Skins>().get_busy() {
                return;
            }
            if let Some(p) = s.skin_presets.iter().find(|p| p.id == preset.as_str()) {
                s.skin_preview.pending_apply = Some((account, p.id.clone(), p.model.clone()));
                w.global::<Skins>()
                    .set_apply_label(format!("Apply {} to your Minecraft account?", p.name).into());
                w.global::<Skins>().set_apply_open(true);
            }
        });
    });
    skins.on_confirm_apply(|| {
        let Some(account) = selected_account() else {
            return;
        };
        let pending = ui::with(|w, s| {
            w.global::<Skins>().set_apply_open(false);
            if w.global::<Skins>().get_busy() {
                return None;
            }
            s.skin_preview.pending_apply.take()
        })
        .flatten();
        if let Some((target, preset, model)) = pending {
            if target != account {
                ui::toast(
                    "The selected account changed. Review the skin again.",
                    Tone::Warning,
                );
                return;
            }
            mutate(
                api::apply_skin_preset(ui::host(), account, preset, model),
                |cosmetics| {
                    apply_cosmetics(cosmetics);
                    ui::toast("Skin updated.", Tone::Success);
                },
            );
        }
    });
    skins.on_rotate(|dx, dy| {
        ui::with(|_, s| {
            s.skin_preview.yaw = (s.skin_preview.yaw + dx * 0.012) % std::f32::consts::TAU;
            s.skin_preview.pitch = (s.skin_preview.pitch + dy * 0.006).clamp(-0.6, 0.6);
        });
        schedule_render();
    });
    skins.on_zoom(|delta| {
        ui::with(|_, s| {
            s.skin_preview.zoom = (s.skin_preview.zoom + delta * 0.08).clamp(0.65, 1.2)
        });
        schedule_render();
    });
    skins.on_reset_view(|| {
        ui::with(|_, s| {
            s.skin_preview.yaw = crate::player::DEFAULT_YAW;
            s.skin_preview.pitch = 0.1;
            s.skin_preview.zoom = 1.;
        });
        schedule_render();
    });
    skins.on_layers_changed(schedule_render);
    skins.on_preview_skin(preview_skin);
    skins.on_preview_cape(|id| {
        ui::with(|w, s| {
            s.skin_preview.cape = s
                .skin_preview
                .capes
                .iter()
                .find(|c| c.id == id.as_str())
                .and_then(|c| c.preview.clone());
            s.skin_preview.cape_id = id.to_string();
            w.global::<Skins>().set_cape_label(
                s.skin_preview
                    .capes
                    .iter()
                    .find(|c| c.id == id.as_str())
                    .map(|c| format!("Preview: {}", c.name))
                    .unwrap_or_else(|| "No cape preview".into())
                    .into(),
            );
        });
        render_capes();
        schedule_render();
    });
    skins.on_preview_current(|| {
        ui::with(|w, s| {
            s.skin_preview.serial += 1;
            s.skin_preview.skin = s.skin_preview.current.clone();
            w.global::<Skins>().set_preview_label(
                if s.skin_preview.skin.is_some() {
                    "Current appearance"
                } else {
                    "Neutral model · no skin loaded"
                }
                .into(),
            );
        });
        schedule_render();
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
    let Some(epoch) = ui::with(|window, s| {
        let skins = window.global::<Skins>();
        if skins.get_busy() {
            return None;
        }
        skins.set_busy(true);
        skins.set_error("".into());
        Some(s.skin_preview.account_epoch)
    })
    .flatten() else {
        return;
    };
    ui::spawn(work, move |result| {
        let current = ui::with(|window, s| {
            window.global::<Skins>().set_busy(false);
            s.skin_preview.account_epoch == epoch
        })
        .unwrap_or(false);
        if !current {
            load_presets();
            return;
        }
        match result {
            Ok(value) => done(value),
            Err(error) => {
                ui::with(|window, _| window.global::<Skins>().set_error(error.message.into()));
            }
        }
    });
}

/// Called on navigation and account changes.
pub fn on_shown() {
    let account = selected_account();
    if ui::state(|s| !s.skin_preview.library_loaded || s.skin_preview.cached_account != account) {
        load();
    } else {
        schedule_render();
    }
}

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
        skins.set_loading(false);
        skins.set_has_current(false);
        skins.set_cape_label("No cape".into());
    });
    load_presets();
    schedule_render();
    let serial = ui::with(|w, s| {
        s.skin_preview.account_epoch += 1;
        s.skin_preview.cached_account = account.clone();
        s.skin_preview.current = None;
        s.skin_preview.skin = None;
        s.skin_preview.cape = None;
        s.skin_preview.capes.clear();
        w.global::<Skins>().set_capes(ui::model(Vec::new()));
        w.global::<Skins>()
            .set_preview_label("Neutral model · no skin loaded".into());
        s.skin_preview.account_epoch
    })
    .unwrap_or(0);
    let Some(account) = account else { return };
    ui::with(|window, _| window.global::<Skins>().set_loading(true));
    let expected_account = account.clone();
    ui::spawn(api::cosmetics(ui::host(), account), move |result| {
        if !ui::state(|s| {
            s.skin_preview.account_epoch == serial
                && s.accounts.selected_account_id.as_ref() == Some(&expected_account)
        }) {
            return;
        }
        ui::with(|window, _| window.global::<Skins>().set_loading(false));
        match result {
            Ok(cosmetics) => apply_cosmetics(cosmetics),
            Err(error) => {
                ui::with(|window, _| window.global::<Skins>().set_error(error.message.into()));
            }
        }
    });
}

fn apply_cosmetics(cosmetics: Cosmetics) {
    ui::with(|window, state| {
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
        state.skin_preview.current = cosmetics.current_skin.clone();
        state.skin_preview.skin = cosmetics.current_skin;
        state.skin_preview.cape = cosmetics
            .capes
            .iter()
            .find(|c| c.selected)
            .and_then(|c| c.preview.clone());
        state.skin_preview.cape_id = cosmetics
            .capes
            .iter()
            .find(|c| c.selected)
            .map(|c| c.id.clone())
            .unwrap_or_default();
        skins.set_cape_label(
            cosmetics
                .capes
                .iter()
                .find(|c| c.selected)
                .map(|c| format!("Wearing {}", c.name))
                .unwrap_or_else(|| "No cape".into())
                .into(),
        );
        skins.set_preview_label(
            if state.skin_preview.skin.is_some() {
                "Current appearance"
            } else {
                "Neutral model · no skin loaded"
            }
            .into(),
        );
        state.skin_preview.capes = cosmetics.capes;
    });
    render_capes();
    schedule_render();
}

fn model_label(model: &str) -> &'static str {
    if model == "slim" { "Slim" } else { "Classic" }
}

fn load_presets() {
    let epoch = ui::with(|_, s| {
        s.skin_preview.library_epoch += 1;
        s.skin_preview.library_epoch
    })
    .unwrap_or(0);
    ui::spawn(api::skin_presets(ui::host()), move |result| {
        if !ui::state(|s| s.skin_preview.library_epoch == epoch) {
            return;
        }
        match result {
            Ok(presets) => {
                ui::with(|_, state| {
                    let ids: std::collections::HashSet<_> =
                        presets.iter().map(|p| p.id.clone()).collect();
                    state.skin_thumbnails.retain(|id, _| ids.contains(id));
                    state.skin_preview.library_loaded = true;
                    state.skin_presets = presets.clone();
                });
                render_presets();
                for preset in presets {
                    let id = preset.id.clone();
                    ui::spawn(api::skin_thumbnail(ui::host(), id.clone()), move |result| {
                        if let Ok(Some(thumbnail)) = result {
                            if let Some(image) = ui::rgba_image(&thumbnail.rgba) {
                                ui::with(|_, state| {
                                    if state.skin_presets.iter().any(|p| p.id == id) {
                                        state.skin_thumbnails.insert(id, image);
                                    }
                                });
                                render_presets();
                            }
                        }
                    });
                }
            }
            Err(error) => {
                ui::with(|window, _| window.global::<Skins>().set_error(error.message.into()));
            }
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
