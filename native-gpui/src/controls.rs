//! Aurora-owned controls built on `gpui-base` behavior primitives.
//!
//! Everything here is presentation + wiring around the unstyled behavior
//! elements that `gpui_kit::base` exports (`Button`, `Switch`, `Slider*`,
//! `Select`, `InputBase`/`Input`). No `gpui-component` dependency: the launcher
//! owns its design system, and the palette comes from [`crate::theme`].
//!
//! Builder methods the probe binary does not exercise yet stay: this file is
//! the reusable control library for the launcher.
#![allow(dead_code)]

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::base::actions::{Confirm, SelectDown, SelectUp};
use gpui_kit::base::input::{Input, InputAccessibility, InputEditorStyle, InputState};
use gpui_kit::base::slider::SliderState;
use gpui_kit::base::{
    Align, Button, Checkbox, CheckboxState, ElementExt, InputBase, POPUP_PRIORITY, Placement,
    Positioner, RoleOverride, Select, Slider, SliderIndicator, SliderThumb, SliderTrack, Switch,
    SwitchThumb, SwitchTrack,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, Axis, Bounds, ClickEvent, Edges, ElementId, Entity, FocusHandle, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, RenderOnce, Role,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, deferred, div, px,
    relative,
};

use crate::theme::theme;

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ChangeHandler = Rc<dyn Fn(bool, &ClickEvent, &mut Window, &mut App)>;
type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type OpenChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type BoundsCell = Rc<Cell<Option<Bounds<Pixels>>>>;

// ---------------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Default,
    Primary,
    Danger,
    Ghost,
    /// Sidebar navigation: full width, left aligned, accent wash when selected.
    /// Accent-tinted secondary action for repeated per-row actions (Install,
    /// Play) where a solid primary per row would be too loud.
    Soft,
    /// Quiet tertiary text action (Details, Cancel in dense rows).
    Subtle,
    /// Quiet destructive text action in dense rows.
    DangerSubtle,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// A themed button on the unstyled [`Button`] behavior root.
///
/// The base root owns the semantics: `Role::Button`, pointer + Enter/Space
/// activation, tab-stop participation, `aria-disabled`, and the semantic
/// `selected`/`disabled` style hooks. This wrapper owns only palette and
/// geometry.
#[derive(IntoElement)]
pub struct AuroraButton {
    id: ElementId,
    label: SharedString,
    accessibility_label: Option<SharedString>,
    notification: bool,
    variant: ButtonVariant,
    size: ButtonSize,
    icon: Option<&'static str>,
    full_width: bool,
    disabled: bool,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl AuroraButton {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            accessibility_label: None,
            notification: false,
            variant: ButtonVariant::Default,
            size: ButtonSize::Medium,
            icon: None,
            full_width: false,
            disabled: false,
            selected: false,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Context for repeated row actions without repeating it in the visible label.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    pub fn notification(mut self, pending: bool) -> Self {
        self.notification = pending;
        self
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// A leading Material Symbols glyph (a `crate::symbols` name).
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AuroraButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let clear = gpui_kit::transparent_black();
        let primary = self.variant == ButtonVariant::Primary;
        let (bg, fg, hover_bg, border) = match self.variant {
            // Subtle elevated secondary control with hairline edge for clear visual hierarchy.
            ButtonVariant::Default => (t.control, t.foreground, t.control_hover, t.border_subtle),
            ButtonVariant::Primary => (t.accent, t.accent_foreground, t.accent_hover, t.accent),
            ButtonVariant::Danger => (
                Hsla {
                    a: 0.12,
                    ..t.danger
                },
                t.danger,
                Hsla {
                    a: 0.20,
                    ..t.danger
                },
                Hsla {
                    a: 0.32,
                    ..t.danger
                },
            ),
            // Transparent at rest so translucent Borealis surfaces show through.
            ButtonVariant::Ghost => (clear, t.foreground, t.control, clear),
            ButtonVariant::Soft => (
                Hsla { a: 0.22, ..t.ring },
                t.foreground,
                Hsla { a: 0.34, ..t.ring },
                Hsla { a: 0.38, ..t.ring },
            ),
            ButtonVariant::Subtle => (clear, t.muted_foreground, t.control, clear),
            ButtonVariant::DangerSubtle => (
                Hsla {
                    a: 0.07,
                    ..t.danger
                },
                t.danger,
                Hsla {
                    a: 0.16,
                    ..t.danger
                },
                Hsla {
                    a: 0.30,
                    ..t.danger
                },
            ),
        };
        let (height, pad, text, icon_size) = match self.size {
            ButtonSize::Small => (30., 10., 13., 15.),
            ButtonSize::Medium => (34., 14., 14., 16.),
            ButtonSize::Large => (44., 22., 15., 20.),
        };
        let (ring, selected_fg) = (t.ring, t.foreground);
        let (disabled_bg, disabled_fg, disabled_border) =
            (t.control, t.muted_foreground, t.border_subtle);
        let icon_color = if self.disabled {
            disabled_fg
        } else if primary {
            fg
        } else {
            Hsla { a: 0.85, ..fg }
        };
        let weight = if primary || self.selected {
            gpui_kit::FontWeight::SEMIBOLD
        } else {
            gpui_kit::FontWeight::MEDIUM
        };

        let mut button = Button::new(self.id)
            .aria_label(
                self.accessibility_label
                    .unwrap_or_else(|| self.label.clone()),
            )
            .disabled(self.disabled)
            .selected(self.selected)
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(7.))
            .when(self.full_width, |b| b.w_full())
            .justify_center()
            .h(px(height))
            .px(px(pad))
            .rounded(t.radius)
            .text_size(px(text))
            .font_weight(weight)
            .when(primary && !self.disabled, |b| {
                b.bg(gpui_kit::linear_gradient(
                    125.,
                    gpui_kit::linear_color_stop(t.accent, 0.),
                    gpui_kit::linear_color_stop(
                        Hsla {
                            h: t.accent.h,
                            s: t.accent.s,
                            l: (t.accent.l + 0.05).min(0.72),
                            a: 1.0,
                        },
                        1.,
                    ),
                ))
            })
            .when(!primary || self.disabled, |b| b.bg(bg))
            .text_color(fg)
            .border_1()
            .border_color(border)
            .hover(|style| style.bg(hover_bg))
            .active(|style| style.opacity(0.88))
            .focus_visible(|style| style.border_color(ring))
            .styles(|styles| {
                styles
                    .selected(|style| {
                        style
                            .bg(Hsla { a: 0.24, ..ring })
                            .border_color(Hsla { a: 0.58, ..ring })
                            .text_color(selected_fg)
                    })
                    .disabled(|style| {
                        style
                            .bg(disabled_bg)
                            .text_color(disabled_fg)
                            .border_color(disabled_border)
                            .opacity(0.62)
                    })
            })
            .when_some(self.icon, |b, icon| {
                b.child(crate::symbols::icon(Some(icon), px(icon_size), icon_color))
            })
            .child(self.label)
            .when(self.notification, |button| {
                button
                    .aria_description("Updates available")
                    .child(crate::ui::update_dot(t))
            });

        if let Some(on_click) = self.on_click {
            button = button.on_click(move |event, window, cx| on_click(event, window, cx));
        }
        button
    }
}

// ---------------------------------------------------------------------------
// Text input
// ---------------------------------------------------------------------------

/// A themed single-line text input around [`InputState`].
///
/// The state entity is the editing engine: text, caret, selection, clipboard,
/// undo, IME. This wrapper renders it inside a styled [`InputBase`] frame and
/// projects the Aurora palette onto the engine's painting colors each render
/// (`InputEditorStyle`), exactly like `gpui-component::Input` does, minus the
/// component library.
#[derive(IntoElement)]
pub struct AuroraInput {
    state: Entity<InputState>,
    label: Option<SharedString>,
}

impl AuroraInput {
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            label: None,
        }
    }

    /// Accessible name; falls back to the state's placeholder when unset.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl RenderOnce for AuroraInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx).clone();
        let state = self.state.clone();
        let presentation = state.read(cx).presentation();

        let label = self
            .label
            .clone()
            .or_else(|| Some(presentation.placeholder().clone()).filter(|p| !p.is_empty()));

        let label_clone = label.clone();
        let masked = presentation.is_masked();
        state.update(cx, |state, _| {
            // Engine painting colors. Applied every render so a runtime accent
            // color change is picked up without rebuilding state.
            state.set_editor_style(InputEditorStyle {
                foreground: t.foreground,
                muted_foreground: t.muted_foreground,
                background: t.surface,
                border: t.border,
                selection: t.selection,
                caret: t.caret,
                ..InputEditorStyle::default()
            });
            state.set_accessibility(
                InputAccessibility::new(None)
                    .with_label(label_clone)
                    .with_value_exposed(!masked),
            );
            // Single-line inputs take zero engine padding; the frame provides it.
            state.set_editor_paddings(Edges::default());
        });

        // A frame-scoped focus handle, so focus styling can observe "the editor
        // or anything inside this frame is focused" without registering the
        // editor's own focus handle a second time.
        let frame_focus = window
            .use_keyed_state(("aurora-input-frame", state.entity_id()), cx, |_, cx| {
                cx.focus_handle()
            })
            .read(cx)
            .clone();
        let focused = !presentation.is_disabled()
            && (presentation.focus_handle().is_focused(window)
                || frame_focus.contains_focused(window, cx));

        InputBase::new(("aurora-input", state.entity_id()))
            .focused(focused)
            .disabled(presentation.is_disabled())
            .track_focus(&frame_focus)
            // The editor child owns text semantics; the frame stays presentational.
            .role(RoleOverride::Presentational)
            .flex()
            .items_center()
            .w_full()
            .h(px(34.))
            .px(px(12.))
            .rounded(t.radius)
            .bg(t.sunken)
            .border_1()
            .border_color(t.border_subtle)
            .text_size(px(crate::ui::TEXT_BODY))
            .styles(|styles| {
                styles
                    .focused(|style| style.border_color(t.ring))
                    .disabled(|style| style.opacity(0.45))
            })
            .child(Input::new(&state))
    }
}

// ---------------------------------------------------------------------------
// Toggle
// ---------------------------------------------------------------------------

/// A themed binary toggle around the unstyled [`Switch`].
///
/// Controlled: `checked` is rendered back from application state and
/// `on_change` reports the *next* value. Keyboard and accessibility activation
/// (Space/Enter, `Role::Switch`, `aria-checked`) live in the base root.
#[derive(IntoElement)]
pub struct AuroraToggle {
    id: ElementId,
    label: SharedString,
    checked: bool,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl AuroraToggle {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: "Toggle".into(),
            checked: false,
            disabled: false,
            on_change: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AuroraToggle {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let track_on: Hsla = if self.disabled {
            t.accent.opacity(0.45)
        } else {
            t.accent
        };
        let track_off: Hsla = if self.disabled {
            t.sunken
        } else {
            t.surface_hover
        };
        let thumb_on: Hsla = if self.disabled {
            t.accent_foreground.opacity(0.75)
        } else {
            t.accent_foreground
        };
        let thumb_off: Hsla = t.muted_foreground;

        let mut switch = Switch::new(self.id.clone())
            .checked(self.checked)
            .disabled(self.disabled)
            .accessibility_label(self.label)
            .relative()
            .w(px(44.))
            .h(px(36.))
            .flex()
            .items_center()
            .justify_center()
            .child(
                SwitchTrack::new((self.id, "track"))
                    .checked(self.checked)
                    .disabled(self.disabled)
                    .relative()
                    .w(px(40.))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .px(px(2.))
                    .when(self.checked, |this| this.justify_end())
                    .rounded_full()
                    .bg(track_off)
                    .border_1()
                    .border_color(t.subtle_foreground.opacity(0.7))
                    .styles(|styles| {
                        styles.checked(|style| style.bg(track_on).border_color(track_on))
                    })
                    .child(
                        SwitchThumb::new(self.checked)
                            .disabled(self.disabled)
                            .flex_shrink_0()
                            .size(px(18.))
                            .rounded_full()
                            .bg(thumb_off)
                            .styles(|styles| styles.checked(|style| style.bg(thumb_on))),
                    ),
            );

        if let Some(on_change) = self.on_change {
            // The base root computes and reports the next value itself.
            switch =
                switch.on_change(move |next, event, window, cx| on_change(next, event, window, cx));
        }
        switch
    }
}

/// File selections are checkboxes; switches remain reserved for on/off operations.
#[derive(IntoElement)]
pub struct AuroraCheckbox {
    id: ElementId,
    label: SharedString,
    visible_label: bool,
    checked: bool,
    disabled: bool,
    dormant: bool,
    on_change: Option<ChangeHandler>,
}

impl AuroraCheckbox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            visible_label: false,
            checked: false,
            disabled: false,
            dormant: false,
            on_change: None,
        }
    }
    /// Render the label visibly beside the checkbox box so clicking either toggles it.
    pub fn with_visible_label(mut self) -> Self {
        self.visible_label = true;
        self
    }
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    /// A saved choice whose feature is currently off: still editable, but a
    /// checked box renders neutral instead of in the accent.
    pub fn dormant(mut self, dormant: bool) -> Self {
        self.dormant = dormant;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn on_change(
        mut self,
        handler: impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AuroraCheckbox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let ring = t.ring;
        let mark = match (self.checked, self.dormant) {
            (true, false) => t.accent_foreground,
            (true, true) => t.foreground,
            _ => gpui_kit::transparent_black(),
        };
        let label_text = self.label.clone();
        let mut checkbox = Checkbox::new(self.id)
            .accessibility_label(self.label)
            .checked(self.checked)
            .disabled(self.disabled)
            .flex()
            .items_center()
            .when(self.visible_label, |this| {
                this.gap(px(10.)).h(px(34.)).px(px(8.))
            })
            .when(!self.visible_label, |this| {
                this.justify_center().size(px(36.))
            })
            .rounded(t.radius)
            .border_1()
            .border_color(gpui_kit::transparent_black())
            .focus_visible(move |style| style.border_color(ring))
            .hover(|style| style.bg(t.control_hover))
            .styles(|styles| styles.disabled(|style| style.opacity(0.55)))
            .child(
                div()
                    .size(px(20.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .border_1()
                    .border_color(match (self.checked, self.dormant) {
                        (true, false) => t.accent,
                        (true, true) => t.muted_foreground,
                        _ => t.subtle_foreground,
                    })
                    .bg(match (self.checked, self.dormant) {
                        (true, false) => t.accent,
                        (true, true) => t.control_hover,
                        _ => t.sunken,
                    })
                    .child(crate::symbols::icon(Some("check"), px(15.), mark)),
            )
            .when(self.visible_label, |this| {
                this.child(
                    div()
                        .text_size(px(crate::ui::TEXT_BODY))
                        .text_color(t.foreground)
                        .child(label_text),
                )
            });
        if let Some(on_change) = self.on_change {
            checkbox = checkbox.on_change(move |state, event, window, cx| {
                on_change(state == CheckboxState::Checked, event, window, cx)
            });
        }
        checkbox
    }
}

// ---------------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------------

/// A themed single-thumb horizontal slider around [`SliderState`].
///
/// Drag/click value mapping, step snapping, range clamping, accessibility
/// values and AT Increment/Decrement live in the base elements. Arrow-key
/// stepping is *not* provided by gpui-base (only the a11y actions are), so this
/// wrapper binds Left/Right/Up/Down/Home/End on an ancestor of the focused
/// slider node and drives the same `step_thumb`/`set_value` entry points.
#[derive(IntoElement)]
pub struct AuroraSlider {
    state: Entity<SliderState>,
    label: SharedString,
    value_label: Option<SharedString>,
    disabled: bool,
    show_label: bool,
}

impl AuroraSlider {
    pub fn new(state: &Entity<SliderState>, label: impl Into<SharedString>) -> Self {
        Self {
            state: state.clone(),
            label: label.into(),
            value_label: None,
            disabled: false,
            show_label: true,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    /// A unit-aware readout supplied by the domain using the slider.
    pub fn value_label(mut self, label: impl Into<SharedString>) -> Self {
        self.value_label = Some(label.into());
        self
    }
}

impl RenderOnce for AuroraSlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx).clone();
        let state = self.state.clone();
        let entity_id = state.entity_id();
        let percentage = state.read(cx).percentage();
        let value = state.read(cx).value();
        let disabled = self.disabled;

        let focus = window
            .use_keyed_state(("aurora-slider", entity_id), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();

        let value_text = self
            .value_label
            .unwrap_or_else(|| format!("{} / 100", value.end() as i64).into());
        let track = div()
            .flex_1()
            .min_w(px(0.))
            .on_key_down({
                let state = state.clone();
                move |event: &KeyDownEvent, window, cx| {
                    if disabled {
                        return;
                    }
                    let key = event.keystroke.key.as_str();
                    if !matches!(key, "left" | "right" | "up" | "down" | "home" | "end") {
                        return;
                    }
                    cx.stop_propagation();
                    state.update(cx, |state, cx| match key {
                        "home" => state.set_value(state.min_value(), window, cx),
                        "end" => state.set_value(state.max_value(), window, cx),
                        "left" | "down" => {
                            state.step_thumb(false, -state.accessibility_step(), window, cx)
                        }
                        _ => state.step_thumb(false, state.accessibility_step(), window, cx),
                    });
                }
            })
            .child(
                Slider::new(&state)
                    .horizontal()
                    .disabled(disabled)
                    .accessibility_label(self.label.clone())
                    .track_focus(&focus)
                    .flex()
                    .flex_1()
                    .items_center()
                    .w_full()
                    .child(
                        SliderTrack::new(&state)
                            .axis(Axis::Horizontal)
                            .disabled(disabled)
                            .flex()
                            .items_center()
                            .w_full()
                            .h(px(18.))
                            .child(
                                SliderIndicator::new(&state)
                                    .relative()
                                    .w_full()
                                    .h(px(6.))
                                    .rounded_full()
                                    .bg(t.surface_hover)
                                    .child(
                                        div()
                                            .absolute()
                                            .h_full()
                                            .left(relative(percentage.start))
                                            .right(relative(1. - percentage.end))
                                            .rounded_full()
                                            .bg(t.accent),
                                    )
                                    .child(
                                        SliderThumb::new(&state)
                                            .axis(Axis::Horizontal)
                                            .disabled(disabled)
                                            .absolute()
                                            .top(px(-5.))
                                            .left(relative(percentage.end))
                                            .ml(-px(8.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(px(16.))
                                            .rounded_full()
                                            .bg(t.foreground)
                                            .child(div().size(px(10.)).rounded_full().bg(t.accent)),
                                    ),
                            ),
                    ),
            );

        if !self.show_label {
            return div()
                .id(("aurora-slider", entity_id))
                .flex()
                .items_center()
                .gap(px(12.))
                .w_full()
                .when(disabled, |this| this.opacity(0.45))
                .child(track)
                .child(
                    div()
                        .flex_shrink_0()
                        .w(px(54.))
                        .text_right()
                        .text_size(px(13.))
                        .text_color(t.muted_foreground)
                        .child(value_text),
                );
        }

        div()
            .id(("aurora-slider", entity_id))
            .flex()
            .flex_col()
            .gap(px(6.))
            .when(disabled, |this| this.opacity(0.45))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(t.muted_foreground)
                            .child(self.label.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(t.foreground)
                            .child(value_text),
                    ),
            )
            .child(track)
    }
}

// ---------------------------------------------------------------------------
// Select / dropdown
// ---------------------------------------------------------------------------

/// A themed controlled dropdown around the unstyled [`Select`] root.
///
/// The base root owns combobox semantics (`Role::ComboBox`, `aria-expanded`),
/// keyboard opening (Enter/Space/Up/Down on the trigger), dismissal (Escape,
/// accessible deactivation), and focus transfer between trigger and content.
/// The application owns the open state, the highlighted index, the selected
/// value, the two focus handles (trigger + content), and the trigger-bounds
/// cell that [`ElementExt::on_prepaint`] keeps current each frame.
#[derive(IntoElement)]
pub struct AuroraSelect {
    id: ElementId,
    label: SharedString,
    options: Vec<SharedString>,
    selected: Option<usize>,
    rows: Option<Vec<AnyElement>>,
    placeholder: SharedString,
    title_trigger: bool,
    trigger_value: Option<SharedString>,
    menu_width: Option<Pixels>,
    highlight: usize,
    open: bool,
    disabled: bool,
    trigger_focus: FocusHandle,
    content_focus: FocusHandle,
    bounds: BoundsCell,
    width: Pixels,
    full_width: bool,
    on_open_change: Option<OpenChangeHandler>,
    on_highlight: Option<IndexHandler>,
    on_confirm: Option<IndexHandler>,
}

impl AuroraSelect {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        options: Vec<SharedString>,
        trigger_focus: &FocusHandle,
        content_focus: &FocusHandle,
        bounds: BoundsCell,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options,
            selected: None,
            rows: None,
            placeholder: "—".into(),
            title_trigger: false,
            trigger_value: None,
            menu_width: None,
            highlight: 0,
            open: false,
            disabled: false,
            trigger_focus: trigger_focus.clone(),
            content_focus: content_focus.clone(),
            bounds,
            width: px(220.),
            full_width: false,
            on_open_change: None,
            on_highlight: None,
            on_confirm: None,
        }
    }

    /// Rich rows for the instance switcher. Options still supply the plain
    /// labels and the selected value exposed to assistive technology.
    pub fn instance_switcher(mut self, rows: Vec<AnyElement>, menu_width: Pixels) -> Self {
        self.rows = Some(rows);
        self.placeholder = "Choose an instance".into();
        self.title_trigger = true;
        self.menu_width = Some(menu_width);
        self
    }

    pub fn trigger_value(mut self, value: Option<SharedString>) -> Self {
        self.trigger_value = value;
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    pub fn highlight(mut self, highlight: usize) -> Self {
        self.highlight = highlight;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Trigger width (default 220px). Long values are truncated with an ellipsis.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    pub fn on_highlight(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_highlight = Some(Rc::new(handler));
        self
    }

    pub fn on_confirm(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Rc::new(handler));
        self
    }
}

#[derive(Default)]
struct SelectViewport {
    scroll: ScrollHandle,
    open: bool,
    highlight: usize,
}

impl RenderOnce for AuroraSelect {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let open = self.open && !self.disabled;
        let viewport = window.use_keyed_state((self.id.clone(), "viewport"), cx, |_, _| {
            SelectViewport::default()
        });
        let scroll = viewport.update(cx, |state, _| {
            if open && (!state.open || state.highlight != self.highlight) {
                let last_row =
                    self.options
                        .len()
                        .saturating_sub(if self.title_trigger { 2 } else { 1 });
                state.scroll.scroll_to_item(self.highlight.min(last_row));
            }
            state.open = open;
            state.highlight = self.highlight;
            state.scroll.clone()
        });
        let bounds = self.bounds.get().unwrap_or_default();
        let available_height = (bounds.top() - px(12.))
            .max(window.viewport_size().height - bounds.bottom() - px(12.))
            .max(px(30.));
        let t = theme(cx);
        let current = self.selected.and_then(|ix| self.options.get(ix)).cloned();
        let current = self.trigger_value.or(current).unwrap_or(self.placeholder);
        let focused = self.trigger_focus.is_focused(window)
            || self.content_focus.contains_focused(window, cx);

        // One shared close path: notify, then restore focus to the trigger.
        let close: CloseHandler = {
            let on_open_change = self.on_open_change.clone();
            let trigger = self.trigger_focus.clone();
            Rc::new(move |window, cx| {
                if let Some(handler) = on_open_change.as_ref() {
                    handler(false, window, cx);
                }
                trigger.focus(window, cx);
            })
        };

        // One shared commit path: confirm, then close.
        let commit: IndexHandler = {
            let on_confirm = self.on_confirm.clone();
            let close = close.clone();
            Rc::new(move |ix, window, cx| {
                if let Some(handler) = on_confirm.as_ref() {
                    handler(ix, window, cx);
                }
                close(window, cx);
            })
        };

        let clear = gpui_kit::transparent_black();
        let trigger = div()
            .id((self.id.clone(), "trigger"))
            .flex()
            .items_center()
            .when(!self.title_trigger, |d| d.justify_between().w_full())
            .when(self.title_trigger, |d| {
                d.justify_start().ml(px(-8.)).max_w_full()
            })
            .gap(px(6.))
            .h(px(if self.title_trigger { 36. } else { 34. }))
            .px(px(if self.title_trigger { 8. } else { 12. }))
            .rounded(t.radius)
            .bg(if self.title_trigger && !self.open && !focused {
                clear
            } else {
                t.control
            })
            .border_1()
            .border_color(if self.open || focused {
                t.ring
            } else if self.title_trigger {
                clear
            } else {
                t.border_subtle
            })
            .text_size(px(if self.title_trigger { 22. } else { 13. }))
            .when(self.title_trigger, |trigger| {
                trigger.font_weight(gpui_kit::FontWeight::BOLD)
            })
            .when(self.disabled, |style| style.opacity(0.45))
            .hover(|style| {
                style.bg(if self.title_trigger {
                    t.control_hover
                } else {
                    t.surface_hover
                })
            })
            .child(
                div()
                    .when(!self.title_trigger, |d| {
                        d.flex_1().min_w(px(0.)).overflow_hidden().text_ellipsis()
                    })
                    .when(self.title_trigger, |d| d.flex_shrink_0())
                    .whitespace_nowrap()
                    .child(current.clone()),
            )
            .child(crate::symbols::icon(
                Some("expand_more"),
                px(if self.title_trigger { 20. } else { 18. }),
                t.muted_foreground,
            ))
            .on_click({
                let on_open_change = self.on_open_change.clone();
                let content = self.content_focus.clone();
                let close = close.clone();
                let open = self.open;
                let disabled = self.disabled;
                move |_, window, cx| {
                    if disabled {
                        return;
                    }
                    if open {
                        close(window, cx);
                    } else {
                        if let Some(handler) = on_open_change.as_ref() {
                            handler(true, window, cx);
                        }
                        content.focus(window, cx);
                    }
                }
            });

        let mut rows = self.rows.into_iter().flatten();
        let mut items: Vec<_> = self
            .options
            .iter()
            .enumerate()
            .map(|(ix, option)| {
                let commit = commit.clone();
                let on_highlight = self.on_highlight.clone();
                let selected = self.selected == Some(ix);
                let highlighted = self.highlight == ix;
                div()
                    .id((self.id.clone(), format!("{ix}")))
                    .role(Role::ListBoxOption)
                    .aria_label(option.clone())
                    .aria_selected(selected)
                    .when(highlighted, |this| this.aria_active_descendant())
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .flex_shrink_0()
                    .text_ellipsis()
                    .text_size(px(13.))
                    .when(highlighted, |this| this.bg(t.surface_hover))
                    .when(selected, |this| this.text_color(t.accent))
                    .hover(|style| style.bg(t.surface_hover))
                    .on_hover(move |hovered, window, cx| {
                        if *hovered {
                            if let Some(handler) = on_highlight.as_ref() {
                                handler(ix, window, cx);
                            }
                        }
                    })
                    .on_click(move |_, window, cx| commit(ix, window, cx))
                    .child(
                        rows.next()
                            .unwrap_or_else(|| option.clone().into_any_element()),
                    )
            })
            .collect();

        let footer = if self.title_trigger {
            items.pop()
        } else {
            None
        };
        let list = div()
            .id((self.id.clone(), "menu"))
            .occlude()
            .track_focus(&self.content_focus)
            .flex()
            .flex_col()
            .max_h(available_height.min(px(288.)))
            .p(px(4.))
            .rounded(t.radius)
            .border_1()
            .border_color(t.border)
            .bg(t.raised)
            .shadow_lg()
            .on_mouse_down_out({
                let close = close.clone();
                let trigger_bounds = self.bounds.clone();
                move |event, window, cx| {
                    if !trigger_bounds
                        .get()
                        .is_some_and(|bounds| bounds.contains(&event.position))
                    {
                        close(window, cx);
                    }
                }
            })
            .on_action({
                let on_highlight = self.on_highlight.clone();
                let last = self.options.len().saturating_sub(1);
                let highlight = self.highlight;
                move |_: &SelectUp, window, cx| {
                    if let Some(handler) = on_highlight.as_ref() {
                        handler(highlight.wrapping_sub(1).min(last), window, cx);
                    }
                }
            })
            .on_action({
                let on_highlight = self.on_highlight.clone();
                let last = self.options.len().saturating_sub(1);
                let highlight = self.highlight;
                move |_: &SelectDown, window, cx| {
                    if let Some(handler) = on_highlight.as_ref() {
                        handler((highlight + 1).min(last), window, cx);
                    }
                }
            })
            .on_action({
                let commit = commit.clone();
                let highlight = self.highlight;
                move |_: &Confirm, window, cx| {
                    cx.stop_propagation();
                    commit(highlight, window, cx);
                }
            })
            .child(
                div()
                    .id((self.id.clone(), "menu-rows"))
                    .flex()
                    .flex_col()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .children(items),
            )
            .children(footer);

        Select::new(self.id.clone())
            .open(open)
            .disabled(self.disabled)
            .focus_handle(&self.trigger_focus)
            .content_focus_handle(&self.content_focus)
            .accessibility_label(self.label.clone())
            .accessibility_value(current)
            .on_open_change({
                let on_open_change = self.on_open_change.clone();
                move |open, window, cx| {
                    if let Some(handler) = on_open_change.as_ref() {
                        handler(open, window, cx);
                    }
                }
            })
            .when(!self.title_trigger, |select| select.w(self.width))
            .when(self.title_trigger, |select| select.max_w(px(360.)))
            .when(self.full_width, |select| select.w_full())
            .child(
                // Capture the trigger bounds every prepaint so the popup can be
                // positioned under it (the pattern gpui-component's Select uses).
                div()
                    .relative()
                    .when(!self.title_trigger, |d| d.w_full())
                    .on_prepaint({
                        let cell = self.bounds.clone();
                        move |bounds, _, _| cell.set(Some(bounds))
                    })
                    .child(trigger),
            )
            .when(open, |this| {
                let bounds = self.bounds.get().unwrap_or_default();
                this.child(
                    deferred(
                        Positioner::side(bounds)
                            .placement(Placement::Bottom)
                            .align(Align::Start)
                            .offset(px(4.))
                            .margin(px(8.))
                            .child(
                                div()
                                    .w(self.menu_width.unwrap_or(bounds.size.width))
                                    .child(list),
                            ),
                    )
                    .with_priority(POPUP_PRIORITY),
                )
            })
    }
}
