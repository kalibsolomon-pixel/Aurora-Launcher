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
///
/// The figure is monochrome: one soft lavender hue taken from the theme accent,
/// with tone (not colour) separating head, torso, arms and legs. Every face of
/// every box gets its own lightness (top lit, sides and underside falling off)
/// and a gentle vertical falloff, so the 3D form reads without cartoon colours.
fn neutral_skin(accent: gpui::Hsla) -> Arc<api::HeadAvatar> {
    const SIZE: usize = 64;
    let mut px = vec![0u8; SIZE * SIZE * 4];
    let hue = accent.h;
    let tone = |lightness: f32, saturation: f32| gpui::Hsla {
        h: hue,
        s: saturation,
        l: lightness.clamp(0., 1.),
        a: 1.,
    };
    // Paints one texture rectangle with a vertical lightness falloff.
    let mut paint = |x0: usize, y0: usize, w: usize, h: usize, base: gpui::Hsla, falloff: f32| {
        for row in 0..h {
            let k = if h > 1 {
                row as f32 / (h - 1) as f32
            } else {
                0.
            };
            let c: gpui::Rgba = gpui::Hsla {
                l: (base.l - falloff * k).clamp(0., 1.),
                ..base
            }
            .into();
            let rgb = [
                (c.r * 255.).round() as u8,
                (c.g * 255.).round() as u8,
                (c.b * 255.).round() as u8,
            ];
            for x in x0..x0 + w {
                let i = ((y0 + row) * SIZE + x) * 4;
                px[i..i + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    };
    // One box (width, height, depth at texture origin u, v) in a base tone.
    let mut part = |u: usize, v: usize, w: usize, h: usize, d: usize, lightness: f32, sat: f32| {
        let face = |delta: f32| tone(lightness + delta, sat);
        paint(u + d, v, w, d, face(0.10), 0.0); // top
        paint(u + d + w, v, w, d, face(-0.18), 0.0); // bottom
        paint(u, v + d, d, h, face(-0.05), 0.07); // right
        paint(u + d, v + d, w, h, face(0.0), 0.07); // front
        paint(u + d + w, v + d, d, h, face(-0.09), 0.07); // left
        paint(u + d + w + d, v + d, w, h, face(-0.06), 0.07); // back
    };
    part(0, 0, 8, 8, 8, 0.76, 0.30); // head
    part(16, 16, 8, 12, 4, 0.64, 0.30); // torso
    part(40, 16, 4, 12, 4, 0.70, 0.28); // right arm
    part(32, 48, 4, 12, 4, 0.70, 0.28); // left arm
    part(0, 16, 4, 12, 4, 0.50, 0.28); // right leg
    part(16, 48, 4, 12, 4, 0.50, 0.28); // left leg
    Arc::new(api::HeadAvatar {
        rgba: Vec::new(),
        skin_rgba: px,
        skin_height: 64,
        model: "classic".into(),
    })
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
        if request.skin.is_none() && request.cape.is_none() {
            self.state.finish(&request, true);
            self.image = None;
            return;
        }
        let mut work = request.clone();
        if work.skin.is_none() {
            // Neutral model: a tonal mannequin texture so the form reads.
            work.skin = Some(neutral_skin(theme::theme(cx).accent));
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
const STAGE_HEIGHT: f32 = 300.;

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
        // Icon-only tool button (32px) with an accessible name.
        let tool = |id: &'static str,
                    label: &'static str,
                    icon: &'static str,
                    action: fn(&mut Self, &mut Context<Self>)| {
            let entity = entity.clone();
            let (hover, ring, color) = (t.control_hover, t.ring, t.muted_foreground);
            gpui::base::Button::new(id)
                .aria_label(label)
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .size(px(32.))
                .rounded(t.radius)
                .border_1()
                .border_color(gpui::transparent_black())
                .hover(move |style| style.bg(hover))
                .active(|style| style.opacity(0.88))
                .focus(move |style| style.border_color(ring))
                .child(crate::symbols::icon(Some(icon), px(18.), color))
                .on_click(move |_, _, cx| {
                    entity.update(cx, |s, cx| action(s, cx));
                })
        };
        let layers_entity = entity.clone();
        let rear = self.rear;
        let neutral = self.state.scene.skin.is_none() && self.state.scene.cape.is_none();

        // The renderer produces a 320x440 image; the stage shows it scaled down
        // so the whole card (stage + controls) fits a 760px window.
        let scale = 0.68;
        let stage =
            div()
                .id("player-preview-surface")
                .role(Role::Group)
                .aria_label("Player preview")
                .aria_description(if neutral {
                    "Import a PNG in the local library to preview a skin."
                } else {
                    "Drag or use arrow keys to rotate. Plus and minus zoom; Home resets."
                })
                .track_focus(&self.focus)
                .tab_stop(true)
                .relative()
                .w_full()
                .flex_1()
                .min_h(px(if neutral { 240. } else { STAGE_HEIGHT }))
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
                .when_some(self.image.clone().filter(|_| !neutral), |d, image| {
                    d.child(
                        div()
                            .relative()
                            .flex_shrink_0()
                            .w(px(318. * scale))
                            .h(px(438. * scale))
                            .child(img(image).w(px(318. * scale)).h(px(438. * scale))),
                    )
                })
                .when(neutral || self.image.is_none(), |d| {
                    d.child(ui::icon_tile(&t, Some("checkroom"), 48., false))
                        .child(
                            div()
                                .text_size(px(ui::TEXT_BODY))
                                .text_color(t.muted_foreground)
                                .child(if neutral {
                                    "Preview a skin"
                                } else {
                                    "Player preview unavailable"
                                }),
                        )
                        .when(neutral, |body| {
                            body.child(div().max_w(px(200.)).text_center().child(ui::caption(
                                &t,
                                "Choose Import PNG to preview a skin here.",
                            )))
                        })
                })
                .when(!neutral, |d| {
                    d.child(
                        // Floating view tools, top-right of the stage. Presses here must
                        // not start a rotate-drag on the stage underneath.
                        div()
                            .absolute()
                            .top(px(10.))
                            .right(px(10.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .p(px(3.))
                            .rounded(t.radius_lg)
                            .bg(t.raised)
                            .border_1()
                            .border_color(t.border_subtle)
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(tool("preview-zoom-in", "Zoom in", "zoom_in", |s, cx| {
                                s.zoom(0.08, cx)
                            }))
                            .child(tool("preview-zoom-out", "Zoom out", "zoom_out", |s, cx| {
                                s.zoom(-0.08, cx)
                            }))
                            .child(tool(
                                "preview-reset",
                                "Reset view",
                                "restart_alt",
                                |s, cx| s.reset(cx),
                            )),
                    )
                });

        ui::card(&t)
            .gap(px(14.))
            .p(px(16.))
            .w(px(if neutral { 280. } else { 320. }))
            .h(px(if neutral { 280. } else { 430. }))
            .flex_shrink_0()
            .text_color(t.foreground)
            .child(stage)
            // View segments left, the outer-layers toggle right.
            .when(!neutral, |card| {
                card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .w(px(132.))
                                .flex_shrink_0()
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
                                .flex_shrink_0()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_size(px(ui::TEXT_CAPTION))
                                        .text_color(t.muted_foreground)
                                        .child("Outer layers"),
                                )
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
                        ),
                )
            })
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
