//! Bounded, account-independent player preview.
use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, AuroraToggle},
    images, player,
    preview::{RenderGate, Scene},
    theme,
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
        let work = request.clone();
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
impl Render for SkinPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx).clone();
        let focus = self.focus.clone();
        let entity = cx.entity();
        let button = |id, label, action: fn(&mut Self, &mut Context<Self>)| {
            let entity = entity.clone();
            AuroraButton::new(id, label).on_click(move |_, _, cx| {
                entity.update(cx, |s, cx| action(s, cx));
            })
        };
        let layers_entity = entity.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .text_color(t.foreground)
            .child(
                div()
                    .id("player-preview-surface")
                    .role(Role::Group)
                    .aria_label("Player preview")
                    .aria_description(
                        "Drag or use arrow keys to rotate. Plus and minus zoom; Home resets.",
                    )
                    .track_focus(&self.focus)
                    .tab_stop(true)
                    .w(px(320.))
                    .h(px(440.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(gpui::transparent_black())
                    .focus(move |style| style.border_color(t.ring))
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
                    .when_some(self.image.clone(), |d, image| {
                        d.child(img(image).w(px(320.)).h(px(440.)))
                    })
                    .when(self.image.is_none(), |d| {
                        d.child(if self.state.scene.skin.is_none() {
                            "Neutral model"
                        } else {
                            "Player preview unavailable"
                        })
                    }),
            )
            .when(
                self.state.scene.skin.is_none() && self.image.is_some(),
                |d| d.child("Neutral model"),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(button("preview-zoom-out", "Zoom out", |s, cx| {
                        s.zoom(-0.08, cx)
                    }))
                    .child(button("preview-zoom-in", "Zoom in", |s, cx| {
                        s.zoom(0.08, cx)
                    }))
                    .child(button("preview-reset", "Reset", |s, cx| s.reset(cx))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(button("preview-front", "Front", |s, cx| {
                        s.view_back(false, cx)
                    }))
                    .child(button("preview-back", "Back", |s, cx| {
                        s.view_back(true, cx)
                    }))
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
                    )
                    .child("Outer layers"),
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
