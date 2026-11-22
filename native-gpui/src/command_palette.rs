//! Global Ctrl+K Command Palette for instant keyboard navigation and actions.

use gpui::{
    Context, Entity, EventEmitter, KeyDownEvent, Render, Subscription, Window, div, prelude::*, px,
};
use gpui_kit::base::{
    Dialog,
    input::{InputEvent, InputState},
};

use crate::{controls::AuroraInput, theme, ui};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    NavigatePage(u8),
    OpenInstance(String),
    PlayInstance(String),
    OpenSettingsSection(usize),
    OpenGameLog,
    CreateInstance,
}

#[derive(Clone, Debug)]
pub struct PaletteItem {
    pub title: String,
    pub subtitle: String,
    pub group: &'static str,
    pub action: PaletteAction,
}

pub enum CommandPaletteEvent {
    Execute(PaletteAction),
    Closed,
}

impl EventEmitter<CommandPaletteEvent> for CommandPalette {}

pub struct CommandPalette {
    open: bool,
    query: Entity<InputState>,
    items: Vec<PaletteItem>,
    filtered: Vec<usize>,
    highlight: usize,
    _sub: Subscription,
}

impl CommandPalette {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Jump to instance, page, or action…")
        });
        let sub = cx.subscribe(&query, |this, _, event: &InputEvent, cx| match event {
            InputEvent::Change => {
                this.refilter(cx);
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                this.confirm(cx);
            }
            _ => {}
        });
        Self {
            open: false,
            query,
            items: Vec::new(),
            filtered: Vec::new(),
            highlight: 0,
            _sub: sub,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open_with(
        &mut self,
        items: Vec<PaletteItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.items = items;
        self.open = true;
        self.highlight = 0;
        self.query.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.refilter(cx);
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.emit(CommandPaletteEvent::Closed);
            cx.notify();
        }
    }

    fn refilter(&mut self, cx: &gpui::App) {
        let q = self.query.read(cx).value().trim().to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                q.is_empty()
                    || item.title.to_lowercase().contains(&q)
                    || item.subtitle.to_lowercase().contains(&q)
                    || item.group.to_lowercase().contains(&q)
            })
            .map(|(idx, _)| idx)
            .take(12)
            .collect();
        if self.highlight >= self.filtered.len() {
            self.highlight = 0;
        }
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        let Some(&idx) = self.filtered.get(self.highlight) else {
            return;
        };
        let Some(item) = self.items.get(idx).cloned() else {
            return;
        };
        self.open = false;
        cx.emit(CommandPaletteEvent::Execute(item.action));
        cx.notify();
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let t = theme::theme(cx).clone();
        let mut list = div()
            .id("palette-list")
            .flex()
            .flex_col()
            .gap(px(4.))
            .max_h(px(360.))
            .overflow_y_scroll();
        if self.filtered.is_empty() {
            list = list.child(
                div()
                    .p(px(16.))
                    .text_size(px(ui::TEXT_BODY))
                    .text_color(t.muted_foreground)
                    .child("No matching instances or commands."),
            );
        } else {
            for (pos, &idx) in self.filtered.iter().enumerate() {
                let item = &self.items[idx];
                let active = pos == self.highlight;
                let action = item.action.clone();
                list = list.child(
                    div()
                        .id(("palette-row", pos))
                        .px(px(12.))
                        .py(px(8.))
                        .rounded(t.radius)
                        .bg(if active {
                            t.accent_soft
                        } else {
                            gpui::transparent_black()
                        })
                        .border_1()
                        .border_color(if active {
                            t.accent
                        } else {
                            gpui::transparent_black()
                        })
                        .cursor_pointer()
                        .hover(|s| s.bg(t.surface_hover))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open = false;
                            cx.emit(CommandPaletteEvent::Execute(action.clone()));
                            cx.notify();
                        }))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(ui::TEXT_BODY))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(t.foreground)
                                        .text_ellipsis()
                                        .child(item.title.clone()),
                                )
                                .child(ui::caption(&t, item.subtitle.clone()).text_ellipsis()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .px(px(8.))
                                .py(px(2.))
                                .rounded_full()
                                .bg(t.surface)
                                .text_size(px(ui::TEXT_CAPTION))
                                .text_color(t.muted_foreground)
                                .child(item.group),
                        ),
                );
            }
        }

        let panel = ui::card(&t)
            .w(px(540.))
            .bg(t.raised)
            .flex()
            .flex_col()
            .gap(px(10.))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        this.close(cx);
                        cx.stop_propagation();
                    }
                    "down" => {
                        if !this.filtered.is_empty() {
                            this.highlight = (this.highlight + 1) % this.filtered.len();
                            cx.notify();
                        }
                        cx.stop_propagation();
                    }
                    "up" => {
                        if !this.filtered.is_empty() {
                            this.highlight = if this.highlight == 0 {
                                this.filtered.len() - 1
                            } else {
                                this.highlight - 1
                            };
                            cx.notify();
                        }
                        cx.stop_propagation();
                    }
                    "enter" => {
                        this.confirm(cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(AuroraInput::new(&self.query).label("Command palette"))
            .child(list)
            .child(
                div()
                    .pt(px(6.))
                    .border_t_1()
                    .border_color(t.border_subtle)
                    .flex()
                    .justify_between()
                    .child(ui::caption(&t, "↑↓ Navigate · Enter Run · Esc Close"))
                    .child(ui::caption(&t, "Ctrl+K")),
            );

        let view = cx.entity();
        Dialog::new(cx)
            .open(true)
            .accessibility_label("Command palette")
            .on_open_change(move |open, _, _, cx| {
                if !open {
                    view.update(cx, |s, cx| s.close(cx));
                }
            })
            .backdrop(div().absolute().size_full().bg(gpui::rgba(0x00000088)))
            .popup(panel)
            .into_any_element()
    }
}
