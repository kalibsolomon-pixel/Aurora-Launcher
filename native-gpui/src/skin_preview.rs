//! Bounded, account-independent player preview.
use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, AuroraToggle, ButtonSize, ButtonVariant},
    images, player,
    preview::{RenderGate, Scene},
    theme, ui,
};
use gpui::{
    Context, FocusHandle, KeyDownEvent, MouseButton, Pixels, Point, Render, RenderImage, Role,
    Window, div, img, prelude::*, px,
};
use std::sync::Arc;

struct PreviewState {
    scene: Scene,
    gate: RenderGate,
    active: bool,
}
impl PreviewState {
    fn new() -> Self {
        Self {
            scene: Scene {
                skin: None,
                cape: None,
                yaw: player::DEFAULT_YAW,
                pitch: 0.1,
                zoom: 1.,
                layers: true,
            },
            gate: RenderGate::default(),
            active: false,
        }
    }
    fn request(&mut self) {
        self.gate.request();
    }
    fn activate(&mut self, active: bool) {
        if self.active != active {
            self.active = active;
            if active {
                self.request();
            }
        }
    }
    fn start(&mut self) -> Option<Scene> {
        if self.active && self.gate.start() {
            Some(self.scene.clone())
        } else {
            None
        }
    }
    fn finish(&mut self, request: &Scene, succeeded: bool) -> bool {
        self.gate.finish();
        succeeded && self.active && request.same_appearance(&self.scene)
    }
}

/// A built-in mannequin skin (classic, 64x64) used only to render the neutral
/// model. Scene state keeps `skin: None`, so the UI still reports "neutral".
fn neutral_skin() -> Arc<api::HeadAvatar> {
    static SKIN: std::sync::OnceLock<Arc<api::HeadAvatar>> = std::sync::OnceLock::new();
    SKIN.get_or_init(|| {
        const SIZE: usize = 64;
        let mut px = vec![0u8; SIZE * SIZE * 4];
        let mut fill = |x0: usize, y0: usize, x1: usize, y1: usize, c: [u8; 3]| {
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * SIZE + x) * 4;
                    px[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                }
            }
        };
        let skin = [222, 190, 168];
        let hair = [88, 72, 70];
        let shirt = [118, 124, 206];
        let pants = [66, 74, 112];
        let shoe = [48, 52, 78];
        let eye = [58, 62, 96];
        // Head (8x8x8 at 0,0): skin, then hair on top, back and the upper rows.
        fill(0, 0, 32, 16, skin);
        fill(8, 0, 24, 8, hair);
        fill(24, 8, 32, 16, hair);
        fill(8, 8, 16, 10, hair);
        fill(0, 8, 8, 10, hair);
        fill(16, 8, 24, 10, hair);
        fill(10, 12, 11, 13, eye);
        fill(13, 12, 14, 13, eye);
        // Torso (8x12x4 at 16,16) and both arms: shirt with bare forearms.
        fill(16, 16, 40, 32, shirt);
        for (ax, ay) in [(40, 16), (32, 48)] {
            fill(ax, ay, ax + 16, ay + 16, shirt);
            fill(ax, ay + 10, ax + 16, ay + 16, skin);
        }
        // Legs (4x12x4 at 0,16 and 16,48): trousers with darker shoes.
        for (lx, ly) in [(0, 16), (16, 48)] {
            fill(lx, ly, lx + 16, ly + 16, pants);
            fill(lx, ly + 13, lx + 16, ly + 16, shoe);
        }
        Arc::new(api::HeadAvatar {
            rgba: Vec::new(),
            skin_rgba: px,
            skin_height: 64,
            model: "classic".into(),
        })
    })
    .clone()
}

pub struct SkinPreview {
    runtime: tokio::runtime::Handle,
    state: PreviewState,
    image: Option<Arc<RenderImage>>,
    focus: FocusHandle,
    drag: Option<Point<Pixels>>,
    rear: bool,
}
impl SkinPreview {
    pub fn new(
        runtime: tokio::runtime::Handle,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            runtime,
            state: PreviewState::new(),
            image: None,
            focus: cx.focus_handle(),
            drag: None,
            rear: false,
        }
    }
    pub fn set_appearance(
        &mut self,
        skin: Option<Arc<api::HeadAvatar>>,
        cape: Option<Arc<api::Texture>>,
        rear: bool,
        cx: &mut Context<Self>,
    ) {
        let old = self.state.scene.clone();
        self.state.scene.skin = skin;
        self.state.scene.cape = cape;
        let changed = !old.same_appearance(&self.state.scene);
        if changed {
            self.image = None;
        }
        if rear != self.rear {
            self.rear = rear;
            self.state.scene.yaw =
                player::DEFAULT_YAW + if rear { std::f32::consts::PI } else { 0. };
        }
        if changed || old.yaw != self.state.scene.yaw {
            self.changed(cx);
        }
    }
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.state.activate(active);
        if !active {
            self.drag = None;
        }
        self.dispatch(cx);
        cx.notify();
    }
    pub fn view_back(&mut self, rear: bool, cx: &mut Context<Self>) {
        self.rear = rear;
        self.state.scene.yaw = player::DEFAULT_YAW + if rear { std::f32::consts::PI } else { 0. };
        self.changed(cx);
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.state.request();
        self.dispatch(cx);
        cx.notify();
    }
    fn dispatch(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.state.start() else {
            return;
        };
        let mut work = request.clone();
        if work.skin.is_none() {
            // Neutral model: a tonal mannequin texture so the form reads.
            work.skin = Some(neutral_skin());
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.runtime.spawn(async move {
            let result = guarded(async move {
                tokio::task::spawn_blocking(move || work.render())
                    .await
                    .map_err(|_| {
                        api::ApiError::new(
                            "preview_render_failed",
                            "The player preview could not be rendered.",
                        )
                    })
            })
            .await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            let result = rx.await.ok().and_then(Result::ok);
            let _ = weak.update(cx, |this, cx| {
                if this.state.finish(&request, result.is_some()) {
                    this.image = result.and_then(|pixels| {
                        images::rgba_image(player::WIDTH, player::HEIGHT, pixels)
                    });
                }
                this.dispatch(cx);
                cx.notify();
            });
        })
        .detach();
    }
    fn rotate(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        (self.state.scene.yaw, self.state.scene.pitch) =
            player::apply_drag(self.state.scene.yaw, self.state.scene.pitch, dx, dy);
        self.changed(cx);
    }
    fn zoom(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.state.scene.zoom = (self.state.scene.zoom + delta).clamp(0.65, 1.2);
        self.changed(cx);
    }
    fn reset(&mut self, cx: &mut Context<Self>) {
        self.state.scene.yaw = player::DEFAULT_YAW;
        self.state.scene.pitch = 0.1;
        self.state.scene.zoom = 1.;
        self.rear = false;
        self.changed(cx);
    }
}
const STAGE_HEIGHT: f32 = 340.;

impl Render for SkinPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx).clone();
        let focus = self.focus.clone();
        let entity = cx.entity();
        let ring = t.ring;
        // One equal-width Small control; segments and the zoom row share it.
        let control = |id,
                       label,
                       icon: Option<&'static str>,
                       variant,
                       selected: bool,
                       action: fn(&mut Self, &mut Context<Self>)| {
            let entity = entity.clone();
            let mut button = AuroraButton::new(id, label)
                .variant(variant)
                .size(ButtonSize::Small)
                .selected(selected)
                .full_width()
                .on_click(move |_, _, cx| {
                    entity.update(cx, |s, cx| action(s, cx));
                });
            if let Some(icon) = icon {
                button = button.icon(icon);
            }
            div().flex_1().min_w(px(0.)).child(button)
        };
        let layers_entity = entity.clone();
        let rear = self.rear;
        let neutral = self.state.scene.skin.is_none();

        // The renderer produces a 320x440 image; the stage shows it scaled down
        // so the whole card (stage + controls) fits a 760px window.
        let scale = STAGE_HEIGHT / 440.;
        let stage = div()
            .id("player-preview-surface")
            .role(Role::Group)
            .aria_label("Player preview")
            .aria_description("Drag or use arrow keys to rotate. Plus and minus zoom; Home resets.")
            .track_focus(&self.focus)
            .tab_stop(true)
            .relative()
            .w(px(320.))
            .h(px(STAGE_HEIGHT))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .overflow_hidden()
            .rounded(t.radius)
            .bg(gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(
                    gpui::Hsla {
                        l: (t.sunken.l + 0.06).min(1.),
                        s: t.accent.s * 0.35,
                        a: 1.0,
                        ..t.accent
                    },
                    0.,
                ),
                gpui::linear_color_stop(
                    gpui::Hsla {
                        l: (t.sunken.l + 0.03).min(1.),
                        a: 1.0,
                        ..t.sunken
                    },
                    0.72,
                ),
            ))
            .border_1()
            .border_color(t.border_subtle)
            .focus(move |style| style.border_color(ring))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, e: &gpui::MouseDownEvent, window, cx| {
                    focus.focus(window, cx);
                    s.drag = Some(e.position);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|s, _, _, _| {
                    s.drag = None;
                }),
            )
            .on_mouse_move(cx.listener(|s, e: &gpui::MouseMoveEvent, _, cx| {
                if e.pressed_button != Some(MouseButton::Left) {
                    s.drag = None;
                    return;
                }
                if let Some(last) = s.drag {
                    s.drag = Some(e.position);
                    s.rotate(
                        f32::from(e.position.x - last.x),
                        f32::from(e.position.y - last.y),
                        cx,
                    );
                }
            }))
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, _, cx| {
                match e.keystroke.key.as_str() {
                    "left" => s.rotate(-12., 0., cx),
                    "right" => s.rotate(12., 0., cx),
                    "up" => s.rotate(0., -12., cx),
                    "down" => s.rotate(0., 12., cx),
                    "+" | "=" => s.zoom(0.08, cx),
                    "-" => s.zoom(-0.08, cx),
                    "home" => s.reset(cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            // Soft floor shadow under the model: stacked ellipses fade outward.
            .when(self.image.is_some(), |d| {
                let shadow = |w: f32, h: f32, a: f32| {
                    div()
                        .absolute()
                        .left(px((320. - w) / 2. - 1.))
                        .top(px(394. * scale - h / 2.))
                        .w(px(w))
                        .h(px(h))
                        .rounded_full()
                        .bg(gpui::Hsla {
                            h: 0.,
                            s: 0.,
                            l: 0.,
                            a,
                        })
                };
                d.child(shadow(200., 28., 0.16))
                    .child(shadow(150., 19., 0.22))
                    .child(shadow(100., 11., 0.34))
            })
            .when_some(self.image.clone(), |d, image| {
                d.child(img(image).w(px(318. * scale)).h(px(438. * scale)))
            })
            .when(self.image.is_none(), |d| {
                d.child(ui::icon_tile(&t, Some("person"), 44., false))
                    .child(
                        div()
                            .text_size(px(ui::TEXT_BODY))
                            .text_color(t.muted_foreground)
                            .child(if neutral {
                                "Neutral model"
                            } else {
                                "Player preview unavailable"
                            }),
                    )
            })
            .when(neutral && self.image.is_some(), |d| {
                d.child(div().absolute().top(px(10.)).left(px(10.)).child(ui::badge(
                    &t,
                    "Neutral model",
                    ui::Tone::Neutral,
                )))
            });

        ui::card(&t)
            .gap(px(14.))
            .p(px(16.))
            .w(px(354.))
            .flex_shrink_0()
            .text_color(t.foreground)
            .child(stage)
            // View: front / back as one segmented control.
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .p(px(3.))
                    .rounded(t.radius)
                    .bg(t.sunken)
                    .border_1()
                    .border_color(t.border_subtle)
                    .child(control(
                        "preview-front",
                        "Front",
                        None,
                        ButtonVariant::Ghost,
                        !rear,
                        |s, cx| s.view_back(false, cx),
                    ))
                    .child(control(
                        "preview-back",
                        "Back",
                        None,
                        ButtonVariant::Ghost,
                        rear,
                        |s, cx| s.view_back(true, cx),
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(control(
                        "preview-zoom-out",
                        "Zoom out",
                        Some("zoom_out"),
                        ButtonVariant::Default,
                        false,
                        |s, cx| s.zoom(-0.08, cx),
                    ))
                    .child(control(
                        "preview-zoom-in",
                        "Zoom in",
                        Some("zoom_in"),
                        ButtonVariant::Default,
                        false,
                        |s, cx| s.zoom(0.08, cx),
                    ))
                    .child(control(
                        "preview-reset",
                        "Reset",
                        Some("restart_alt"),
                        ButtonVariant::Default,
                        false,
                        |s, cx| s.reset(cx),
                    )),
            )
            .child(ui::divider(&t))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .child(ui::title_block(
                        &t,
                        "Outer layers",
                        Some("Hat, jacket and sleeve overlays".into()),
                    ))
                    .child(
                        AuroraToggle::new("preview-layers")
                            .label("Outer layers")
                            .checked(self.state.scene.layers)
                            .on_change(move |value, _, _, cx| {
                                layers_entity.update(cx, |s, cx| {
                                    s.state.scene.layers = value;
                                    s.image = None;
                                    s.changed(cx);
                                });
                            }),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_coalesces_dormant_and_rapid_pose_changes() {
        let mut s = PreviewState::new();
        s.request();
        s.scene.yaw = 1.;
        assert!(s.start().is_none());
        s.activate(true);
        let first = s.start().unwrap();
        for n in 0..100 {
            s.scene.yaw = n as f32;
            s.request();
            assert!(s.start().is_none());
        }
        assert!(s.finish(&first, true));
        let latest = s.start().unwrap();
        assert_eq!(latest.yaw, 99.);
        assert!(s.finish(&latest, true));
        assert!(s.start().is_none());
    }
    #[test]
    fn hidden_replaced_and_failed_results_are_terminal() {
        let mut s = PreviewState::new();
        s.activate(true);
        let old = s.start().unwrap();
        s.scene.cape = Some(Arc::new(api::Texture {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        }));
        s.request();
        assert!(!s.finish(&old, true));
        let current = s.start().unwrap();
        s.activate(false);
        assert!(!s.finish(&current, true));
        assert!(s.start().is_none());
        s.activate(true);
        let retry = s.start().unwrap();
        assert!(!s.finish(&retry, false));
        assert!(s.start().is_none());
    }
}
