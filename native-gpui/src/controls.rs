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
    Align, Button, ElementExt, InputBase, POPUP_PRIORITY, Placement, Positioner, RoleOverride,
    Select, Slider, SliderIndicator, SliderThumb, SliderTrack, Switch, SwitchThumb, SwitchTrack,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Axis, Bounds, ClickEvent, Edges, ElementId, Entity, FocusHandle, Hsla, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Pixels, RenderOnce, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Window, deferred, div, px, relative,
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
    Nav,
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
        let nav = self.variant == ButtonVariant::Nav;
        let primary = self.variant == ButtonVariant::Primary;
        let (bg, fg, hover_bg, border) = match self.variant {
            ButtonVariant::Default => (t.control, t.foreground, t.control_hover, t.border_subtle),
            ButtonVariant::Primary => (t.accent, t.accent_foreground, t.accent_hover, t.accent),
            ButtonVariant::Danger => (
                Hsla { a: 0.10, ..t.danger },
                t.danger,
                Hsla { a: 0.18, ..t.danger },
                Hsla { a: 0.28, ..t.danger },
            ),
            // Transparent at rest so translucent Borealis surfaces show through.
            ButtonVariant::Ghost => (clear, t.foreground, t.control, clear),
            ButtonVariant::Nav => (clear, t.muted_foreground, t.control, clear),
        };
        let (height, pad, text, icon_size) = match self.size {
            ButtonSize::Small => (28., 10., 12., 14.),
            ButtonSize::Medium => (34., 14., 13., 16.),
            ButtonSize::Large => (44., 22., 15., 20.),
        };
        let (soft, ring, selected_fg) = (t.accent_soft, t.ring, t.foreground);
        let icon_color = if nav && self.selected {
            t.ring
        } else if primary || nav {
            fg
        } else {
            Hsla { a: 0.85, ..fg }
        };
        let weight = if primary || (nav && self.selected) {
            gpui_kit::FontWeight::SEMIBOLD
        } else {
            gpui_kit::FontWeight::MEDIUM
        };

        let mut button = Button::new(self.id)
            .disabled(self.disabled)
            .selected(self.selected)
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(if nav { 10. } else { 7. }))
            .when(nav || self.full_width, |b| b.w_full())
            .when(nav, |b| b.justify_start())
            .when(!nav, |b| b.justify_center())
            .h(px(height))
            .px(px(if nav { 10. } else { pad }))
            .rounded(if self.size == ButtonSize::Large {
                px(10.)
            } else {
                t.radius
            })
            .text_size(px(text))
            .font_weight(weight)
            .bg(bg)
            .text_color(fg)
            .border_1()
            .border_color(border)
            .hover(|style| style.bg(hover_bg))
            .active(|style| style.opacity(0.88))
            .focus(|style| style.border_color(ring))
            .styles(|styles| {
                styles
                    .selected(|style| {
                        style
                            .bg(soft)
                            .border_color(if nav { clear } else { Hsla { a: 0.55, ..ring } })
                            .text_color(selected_fg)
                    })
                    .disabled(|style| style.opacity(0.42))
            })
            .when_some(self.icon, |b, icon| {
                b.child(crate::symbols::icon(Some(icon), px(icon_size), icon_color))
            })
            .child(self.label);

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
            .text_size(px(13.))
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
        let track_on: Hsla = t.accent;
        let track_off: Hsla = t.surface_hover;
        let thumb_on: Hsla = t.accent_foreground;
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
                    .w(px(36.))
                    .h(px(20.))
                    .rounded_full()
                    .bg(track_off)
                    .styles(|styles| {
                        styles
                            .checked(|style| style.bg(track_on))
                            .disabled(|style| style.opacity(0.45))
                    })
                    .child(
                        SwitchThumb::new(self.checked)
                            .disabled(self.disabled)
                            .absolute()
                            .top(px(2.))
                            .left(px(2.))
                            .when(self.checked, |this| this.left(px(18.)))
                            .size(px(16.))
                            .rounded_full()
                            .bg(thumb_off)
                            .styles(|styles| {
                                styles
                                    .checked(|style| style.bg(thumb_on))
                                    .disabled(|style| style.opacity(0.6))
                            }),
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
    disabled: bool,
}

impl AuroraSlider {
    pub fn new(state: &Entity<SliderState>, label: impl Into<SharedString>) -> Self {
        Self {
            state: state.clone(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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
                            .child(format!("{}", value.end() as i64)),
                    ),
            )
            .child(
                div()
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
                                _ => {
                                    state.step_thumb(false, state.accessibility_step(), window, cx)
                                }
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
                                                    .child(
                                                        div()
                                                            .size(px(10.))
                                                            .rounded_full()
                                                            .bg(t.accent),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            )
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
    highlight: usize,
    open: bool,
    disabled: bool,
    trigger_focus: FocusHandle,
    content_focus: FocusHandle,
    bounds: BoundsCell,
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
            highlight: 0,
            open: false,
            disabled: false,
            trigger_focus: trigger_focus.clone(),
            content_focus: content_focus.clone(),
            bounds,
            on_open_change: None,
            on_highlight: None,
            on_confirm: None,
        }
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
                state.scroll.scroll_to_item(self.highlight);
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
        let current = self
            .selected
            .and_then(|ix| self.options.get(ix))
            .cloned()
            .unwrap_or_else(|| "—".into());
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

        let trigger = div()
            .id((self.id.clone(), "trigger"))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(6.))
            .w_full()
            .h(px(34.))
            .px(px(12.))
            .rounded(t.radius)
            .bg(t.control)
            .border_1()
            .border_color(if self.open || focused {
                t.ring
            } else {
                t.border_subtle
            })
            .text_size(px(13.))
            .when(self.disabled, |style| style.opacity(0.45))
            .hover(|style| style.bg(t.surface_hover))
            .child(div().child(current.clone()))
            .child(div().text_color(t.muted_foreground).child("▾"))
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

        let list = div()
            .id((self.id.clone(), "menu"))
            .occlude()
            .track_focus(&self.content_focus)
            .flex()
            .flex_col()
            .max_h(available_height.min(px(288.)))
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .p(px(4.))
            .rounded(t.radius)
            .border_1()
            .border_color(t.border)
            .bg(t.raised)
            .shadow_lg()
            .on_mouse_down_out({
                let close = close.clone();
                move |_, window, cx| close(window, cx)
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
            .children(self.options.iter().enumerate().map(|(ix, option)| {
                let commit = commit.clone();
                let on_highlight = self.on_highlight.clone();
                let selected = self.selected == Some(ix);
                let highlighted = self.highlight == ix;
                div()
                    .id((self.id.clone(), format!("{ix}")))
                    .role(Role::ListBoxOption)
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
                    .child(option.clone())
            }));

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
            .w(px(220.))
            .child(
                // Capture the trigger bounds every prepaint so the popup can be
                // positioned under it (the pattern gpui-component's Select uses).
                div()
                    .relative()
                    .w_full()
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
                            .child(div().w(bounds.size.width).child(list)),
                    )
                    .with_priority(POPUP_PRIORITY),
                )
            })
    }
}
