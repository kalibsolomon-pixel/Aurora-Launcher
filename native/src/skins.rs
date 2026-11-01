//! Skins: the selected account's current skin, the local skin library and
//! owned capes. Reads and changes go through the cosmetics service, which
//! talks to Minecraft services with the Rust-owned session; the UI receives
//! only decoded pixels, names and ids.

use slint::ComponentHandle;
use std::sync::Arc;

use crate::api::{self, Cape, Cosmetics, HeadAvatar, SkinPreset, Texture};
use crate::ui;
use crate::{AppWindow, CapeRow, SkinRow, Skins, Tone};

pub struct PreviewState {
    pub serial: u64,
    local_revision: u64,
    preview_id: String,
    account_epoch: u64,
    cached_account: Option<String>,
    library_loaded: bool,
    library_epoch: u64,
    skin: Option<Arc<HeadAvatar>>,
    current: Option<Arc<HeadAvatar>>,
    capes: Vec<Cape>,
    cape: Option<Arc<Texture>>,
    cape_id: String,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    render_gate: crate::preview::RenderGate,
    render_timer_pending: bool,
    frame_started: Option<std::time::Instant>,
    #[cfg(feature = "disposable-data-root")]
    legacy_pending: bool,
    pending_apply: Option<(String, String, String)>,
}
impl Default for PreviewState {
    fn default() -> Self {
        Self {
            serial: 0,
            local_revision: 0,
            preview_id: String::new(),
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
            render_gate: Default::default(),
            render_timer_pending: false,
            frame_started: None,
            #[cfg(feature = "disposable-data-root")]
            legacy_pending: false,
            pending_apply: None,
        }
    }
}

fn preview_skin(id: slint::SharedString) {
    let serial = ui::with(|_, s| {
        s.skin_preview.serial += 1;
        s.skin_preview.local_revision += 1;
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
    let preview_id = id.to_string();
    ui::spawn(
        api::skin_preview(ui::host(), id.to_string()),
        move |result| {
            ui::with(|w, s| {
                if s.skin_preview.serial != serial {
                    return;
                }
                match result {
                    Ok(skin) => {
                        s.skin_preview.skin = Some(Arc::new(skin));
                        s.skin_preview.preview_id = preview_id;
                        s.skin_preview.yaw = crate::player::DEFAULT_YAW;
                        w.global::<Skins>().set_rear_view(false);
                        w.global::<Skins>()
                            .set_preview_label(format!("Preview: {name}").into());
                    }
                    Err(e) => w.global::<Skins>().set_error(e.message.into()),
                }
            });
            render_presets();
            schedule_render();
        },
    );
}

fn scene(p: &PreviewState, layers: bool) -> crate::preview::Scene {
    crate::preview::Scene {
        skin: p.skin.clone(),
        cape: p.cape.clone(),
        yaw: p.yaw,
        pitch: p.pitch,
        zoom: p.zoom,
        layers,
    }
}

/// Input updates the latest pose immediately. CPU rasterization never blocks
/// event dispatch; only one immutable frame is handed to a blocking worker.
fn schedule_render() {
    #[cfg(feature = "disposable-data-root")]
    if crate::preview_probe::legacy() {
        schedule_legacy_probe();
        return;
    }
    ui::with(|_, s| s.skin_preview.render_gate.request());
    dispatch_render();
}

fn dispatch_render() {
    enum Next {
        Frame(crate::preview::Scene),
        Wait(std::time::Duration),
    }
    let Some(next) = ui::with(|w, s| {
        if w.global::<crate::Nav>().get_page() != crate::Page::Skins
            || s.skin_preview.render_timer_pending
            || !s.skin_preview.render_gate.ready()
        {
            return None;
        }
        // An immediate first frame, then at most 120 frames/s while input
        // changes. A single timer reads the newest pose; idle schedules none.
        let budget = std::time::Duration::from_nanos(8_333_333);
        if let Some(remaining) = s
            .skin_preview
            .frame_started
            .and_then(|start| budget.checked_sub(start.elapsed()))
        {
            s.skin_preview.render_timer_pending = true;
            return Some(Next::Wait(remaining));
        }
        s.skin_preview.render_gate.start();
        s.skin_preview.frame_started = Some(std::time::Instant::now());
        Some(Next::Frame(scene(
            &s.skin_preview,
            w.global::<Skins>().get_outer_layers(),
        )))
    })
    .flatten() else {
        return;
    };
    let request = match next {
        Next::Frame(frame) => frame,
        Next::Wait(delay) => {
            slint::Timer::single_shot(delay, || {
                ui::with(|_, s| s.skin_preview.render_timer_pending = false);
                dispatch_render();
            });
            return;
        }
    };
    let work = request.clone();
    #[cfg(feature = "disposable-data-root")]
    let input = crate::preview_probe::input_time();
    ui::spawn(
        async move {
            tokio::task::spawn_blocking(move || {
                let started = std::time::Instant::now();
                let pixels = work.render();
                (pixels, started.elapsed())
            })
            .await
        },
        move |result| {
            ui::with(|w, s| {
                s.skin_preview.render_gate.finish();
                let current = scene(&s.skin_preview, w.global::<Skins>().get_outer_layers());
                if w.global::<crate::Nav>().get_page() == crate::Page::Skins
                    && request.same_appearance(&current)
                {
                    if let Ok((pixels, _render_time)) = result {
                        #[cfg(feature = "disposable-data-root")]
                        let handoff = std::time::Instant::now();
                        w.global::<Skins>().set_player(
                            ui::rectangular_image(
                                &pixels,
                                crate::player::WIDTH,
                                crate::player::HEIGHT,
                            )
                            .unwrap_or_default(),
                        );
                        #[cfg(feature = "disposable-data-root")]
                        crate::preview_probe::frame(input, _render_time, handoff.elapsed());
                    }
                }
            });
            dispatch_render();
        },
    );
}

#[cfg(feature = "disposable-data-root")]
fn schedule_legacy_probe() {
    if !ui::with(|_, s| {
        if s.skin_preview.legacy_pending {
            return false;
        }
        s.skin_preview.legacy_pending = true;
        true
    })
    .unwrap_or(false)
    {
        return;
    }
    let input = crate::preview_probe::input_time();
    slint::Timer::single_shot(std::time::Duration::from_millis(16), move || {
        ui::with(|w, s| {
            s.skin_preview.legacy_pending = false;
            let start = std::time::Instant::now();
            let pixels = scene(&s.skin_preview, w.global::<Skins>().get_outer_layers()).render();
            let render = start.elapsed();
            let handoff = std::time::Instant::now();
            w.global::<Skins>().set_player(
                ui::rectangular_image(&pixels, crate::player::WIDTH, crate::player::HEIGHT)
                    .unwrap_or_default(),
            );
            crate::preview_probe::frame(input, render, handoff.elapsed());
        });
    });
}

#[cfg(feature = "disposable-data-root")]
pub fn probe_scene() {
    let mut rgba = vec![0; 64 * 64 * 4];
    for (x, y, width, height, color) in [
        (0, 0, 32, 16, [188, 144, 104, 255]),
        (0, 16, 56, 16, [70, 115, 164, 255]),
        (16, 48, 32, 16, [70, 115, 164, 255]),
    ] {
        for py in y..y + height {
            for px in x..x + width {
                let i = (py * 64 + px) * 4;
                rgba[i..i + 4].copy_from_slice(&color);
            }
        }
    }
    let skin = Arc::new(HeadAvatar {
        rgba: vec![],
        skin_rgba: rgba,
        skin_height: 64,
        model: "classic".into(),
    });
    ui::with(|w, s| {
        s.skin_preview.skin = Some(skin.clone());
        s.skin_preview.current = Some(skin);
        let mut cape = vec![180; 64 * 32 * 4];
        for pixel in cape.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        s.skin_preview.cape = Some(Arc::new(Texture {
            width: 64,
            height: 32,
            rgba: cape,
        }));
        w.global::<Skins>()
            .set_preview_label("Synthetic performance fixture".into());
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
    let pixels = crate::player::cape_artwork(t)?;
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
        let owner = crate::chrome::dialog_owner();
        ui::spawn(
            async move {
                match crate::skin_import::choose(owner).await? {
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
            (s.skin_preview.yaw, s.skin_preview.pitch) =
                crate::player::apply_drag(s.skin_preview.yaw, s.skin_preview.pitch, dx, dy);
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
        ui::with(|w, s| {
            s.skin_preview.yaw = crate::player::DEFAULT_YAW;
            s.skin_preview.pitch = 0.1;
            s.skin_preview.zoom = 1.;
            w.global::<Skins>().set_rear_view(false);
        });
        schedule_render();
    });
    skins.on_view_back(|rear| {
        ui::with(|w, s| {
            s.skin_preview.yaw =
                crate::player::DEFAULT_YAW + if rear { std::f32::consts::PI } else { 0. };
            w.global::<Skins>().set_rear_view(rear);
        });
        schedule_render();
    });
    skins.on_layers_changed(schedule_render);
    skins.on_preview_skin(preview_skin);
    skins.on_preview_cape(|id| {
        ui::with(|w, s| {
            s.skin_preview.local_revision += 1;
            s.skin_preview.cape = s
                .skin_preview
                .capes
                .iter()
                .find(|c| c.id == id.as_str())
                .and_then(|c| c.preview.clone())
                .map(Arc::new);
            // Inspect a cape from the outside immediately, without changing
            // anything on the account or requiring a manual half-turn.
            s.skin_preview.yaw = crate::player::DEFAULT_YAW + std::f32::consts::PI;
            w.global::<Skins>().set_rear_view(true);
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
            s.skin_preview.local_revision += 1;
            s.skin_preview.preview_id.clear();
            s.skin_preview.skin = s.skin_preview.current.clone();
            s.skin_preview.yaw = crate::player::DEFAULT_YAW;
            w.global::<Skins>().set_rear_view(false);
            w.global::<Skins>().set_preview_label(
                if s.skin_preview.skin.is_some() {
                    "Current appearance"
                } else {
                    "Neutral model · no skin loaded"
                }
                .into(),
            );
        });
        render_presets();
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
    let serial = ui::with(|w, s| {
        s.skin_preview.account_epoch += 1;
        s.skin_preview.serial += 1;
        s.skin_preview.local_revision += 1;
        s.skin_preview.preview_id.clear();
        s.skin_preview.cached_account = account.clone();
        s.skin_preview.current = None;
        s.skin_preview.skin = None;
        s.skin_preview.cape = None;
        s.skin_preview.cape_id.clear();
        s.skin_preview.capes.clear();
        w.global::<Skins>().set_capes(ui::model(Vec::new()));
        w.global::<Skins>()
            .set_preview_label("Neutral model · no skin loaded".into());
        s.skin_preview.account_epoch
    })
    .unwrap_or(0);
    schedule_render();
    let preview_revision = ui::state(|s| s.skin_preview.local_revision);
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
            Ok(cosmetics) => apply_cosmetics_snapshot(
                cosmetics,
                ui::state(|s| s.skin_preview.local_revision != preview_revision),
            ),
            Err(error) => {
                ui::with(|window, _| window.global::<Skins>().set_error(error.message.into()));
            }
        }
    });
}

fn apply_cosmetics(cosmetics: Cosmetics) {
    apply_cosmetics_snapshot(cosmetics, false);
}

fn apply_cosmetics_snapshot(cosmetics: Cosmetics, preserve_preview: bool) {
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
        state.skin_preview.current = cosmetics.current_skin.map(Arc::new);
        if !preserve_preview {
            state.skin_preview.preview_id.clear();
            state.skin_preview.skin = state.skin_preview.current.clone();
            state.skin_preview.cape = cosmetics
                .capes
                .iter()
                .find(|c| c.selected)
                .and_then(|c| c.preview.clone())
                .map(Arc::new);
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
        }
        state.skin_preview.capes = cosmetics.capes;
    });
    render_capes();
    render_presets();
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
                    previewed: state.skin_preview.preview_id == preset.id,
                    thumb: thumb.unwrap_or_default(),
                }
            })
            .collect();
        window.global::<Skins>().set_presets(ui::model(rows));
    });
}
