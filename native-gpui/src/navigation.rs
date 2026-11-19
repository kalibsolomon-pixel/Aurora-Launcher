//! Launcher rail, instance grid and detail routing, and the Home switcher.
use std::{cell::Cell, rc::Rc};

use gpui::{
    AnyElement, Bounds, Context, FocusHandle, IntoElement, Pixels, SharedString, Window, deferred,
    div, prelude::*, px,
};
use gpui_kit::base::{Align, Button, ElementExt, Placement, Positioner};

use crate::{
    InstanceTab, Launcher, api, chrome,
    controls::{AuroraButton, AuroraSelect, ButtonSize, ButtonVariant},
    downloads, images, instance_icon, instance_metadata, stored_icon, symbols,
    theme::AuroraTheme,
    ui,
};

const RAIL_WIDTH: f32 = 80.;
pub(crate) const PAGE_GUTTER: f32 = 32.;
pub(crate) const PAGE_CHROME: f32 = RAIL_WIDTH + 2. * PAGE_GUTTER;

struct Switcher {
    trigger: FocusHandle,
    menu: FocusHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    open: bool,
    highlight: usize,
}

struct RailFocus {
    handle: FocusHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

const NAV_GROUPS: &[&[(&str, u8, &str)]] = &[
    &[("Home", 0, "home"), ("Instances", 3, "grid_view")],
    &[
        ("Discover", 5, "explore"),
        ("Downloads", 8, "download"),
        ("Pulsar", 9, "swords"),
    ],
    &[("Skins", 7, "checkroom"), ("Accounts", 1, "account_circle")],
    &[
        ("Sync", 6, "sync"),
        ("Settings", 2, "settings"),
        ("Refresh launcher", u8::MAX, "refresh"),
    ],
];

impl Launcher {
    pub(crate) fn navigation_rail(
        &self,
        t: &AuroraTheme,
        downloads_active: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let compact = window.viewport_size().height < px(640.);
        let hit_height = px(if compact { 42. } else { 52. });
        let has_updates = self.updates_pending();
        let mut rail = div()
            .id("launcher-rail")
            .w(px(RAIL_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .overflow_y_scroll()
            .bg(t.sidebar)
            .border_r_1()
            .border_color(t.border_subtle)
            .py(px(if compact { 6. } else { 10. }))
            .child(chrome::drag_region(
                div()
                    .id("brand")
                    .flex()
                    .flex_none()
                    .w_full()
                    .h(hit_height)
                    .items_center()
                    .justify_center()
                    .child(gpui::img(images::aurora_logo()).w(px(34.)).h(px(27.2))),
            ));
        let running = self.client_update_running(cx);
        for (group, items) in NAV_GROUPS.iter().enumerate() {
            if group == NAV_GROUPS.len() - 1 {
                rail = rail.child(div().flex_1());
            }
            if group != 0 {
                rail = rail.child(
                    div()
                        .flex_none()
                        .w(px(32.))
                        .h(px(1.))
                        .my(px(if compact { 4. } else { 8. }))
                        .bg(t.border_subtle),
                );
            }
            for &(label, page, icon) in *items {
                let refresh = page == u8::MAX;
                let selected = self.page == page;
                let disabled = if refresh {
                    self.busy
                        || self.account_busy
                        || self.skin_busy
                        || running
                        || self.load_in_flight
                        || self.host.is_none()
                } else {
                    running && !selected
                };
                let count = if page == 8 { downloads_active } else { 0 };
                let dot = page == 2 && has_updates;
                let state = window
                    .use_keyed_state(("rail-focus", page as usize), cx, |_, cx| RailFocus {
                        handle: cx.focus_handle(),
                        bounds: Rc::new(Cell::new(None)),
                    })
                    .read(cx);
                let focus = state.handle.clone();
                let bounds = state.bounds.clone();
                let is_focused = focus.is_focused(window) && window.last_input_was_keyboard();
                let ring = t.ring;
                let hover = t.control_hover;
                let accessible_label = if page == 8 {
                    downloads::nav_label(count)
                } else {
                    label.to_owned()
                };
                let button = Button::new(("rail-action", page as usize))
                    .accessibility_label(accessible_label)
                    .when(dot, |button| button.aria_description("Updates available"))
                    .track_focus(&focus)
                    .selected(selected)
                    .disabled(disabled)
                    .relative()
                    .w(px(52.))
                    .h(hit_height)
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(t.radius_lg)
                    .border_1()
                    .border_color(gpui::transparent_black())
                    .hover(move |style| style.bg(hover))
                    .focus_visible(move |style| style.border_color(ring))
                    .styles(|styles| styles.selected(|style| style.bg(t.accent_soft)))
                    .child(symbols::icon(
                        Some(icon),
                        px(26.),
                        if disabled {
                            t.subtle_foreground
                        } else if selected {
                            t.ring
                        } else {
                            t.muted_foreground
                        },
                    ))
                    .when(dot, |button| {
                        button.child(
                            div()
                                .absolute()
                                .top(px(7.))
                                .right(px(7.))
                                .child(ui::update_dot(t)),
                        )
                    })
                    .when(count > 0, |button| {
                        button.child(
                            div()
                                .absolute()
                                .top(px(3.))
                                .right(px(2.))
                                .min_w(px(17.))
                                .h(px(17.))
                                .px(px(3.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(t.accent)
                                .text_color(t.accent_foreground)
                                .text_size(px(10.))
                                .child(count.to_string()),
                        )
                    })
                    .when(!is_focused, |button| {
                        button.tooltip(move |_, cx| cx.new(|_| NavHint(label)).into())
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if refresh {
                            this.refresh_page(cx);
                        } else {
                            this.open_page(page, window, cx);
                            cx.notify();
                        }
                    }));
                rail = rail.child(
                    div()
                        .id(("rail-hint", page as usize))
                        .flex_none()
                        .on_prepaint({
                            let bounds = bounds.clone();
                            move |rect, _, _| bounds.set(Some(rect))
                        })
                        .child(button)
                        .when_some(
                            bounds.get().filter(|_| is_focused && !disabled),
                            |item, bounds| {
                                item.child(
                                    deferred(
                                        Positioner::side(bounds)
                                            .placement(Placement::Right)
                                            .align(Align::Center)
                                            .offset(px(16.))
                                            .child(nav_hint(t, label)),
                                    )
                                    .with_priority(2),
                                )
                            },
                        ),
                );
            }
        }
        rail
    }

    pub(crate) fn instances_children(
        &self,
        t: &AuroraTheme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if let Some(((id, tab), state, instance)) =
            self.instance_detail.as_ref().and_then(|detail| {
                let state = self.launcher.as_ref()?;
                let instance = state.instances.iter().find(|i| i.id == detail.0)?;
                Some((detail, state, instance))
            })
        {
            let editor_busy = self
                .instance_editor
                .as_ref()
                .is_some_and(|editor| editor.read(cx).is_busy());
            let back = AuroraButton::new("instances-back", "Back to instances")
                .size(ButtonSize::Small)
                .variant(ButtonVariant::Ghost)
                .icon("arrow_back")
                .disabled(editor_busy)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_page(3, window, cx);
                    cx.notify();
                }));
            let header = div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(instance_icon(
                    stored_icon(&state.icons, id),
                    48.,
                    self.selected_id() == Some(id),
                    t,
                ))
                .child(ui::page_header(
                    t,
                    instance.display_name.clone(),
                    Some(instance_metadata(instance).into()),
                ));
            let tabs = div().flex().flex_wrap().gap(px(4.)).children(
                [
                    (InstanceTab::Overview, "Overview"),
                    (InstanceTab::Content, "Content"),
                    (InstanceTab::Settings, "Settings"),
                ]
                .into_iter()
                .map(|(next, label)| {
                    let id = id.clone();
                    AuroraButton::new(("instance-detail-tab", next as usize), label)
                        .variant(ButtonVariant::Ghost)
                        .selected(*tab == next)
                        .disabled(editor_busy)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this
                                .instance_detail
                                .as_ref()
                                .is_some_and(|(_, current)| *current != next)
                            {
                                this.open_instance(id.clone(), next, 0, window, cx);
                                cx.notify();
                            }
                        }))
                }),
            );
            let mut out = vec![
                div().flex().child(back).into_any_element(),
                header.into_any_element(),
                tabs.into_any_element(),
            ];
            match tab {
                InstanceTab::Overview => {
                    let (status, tone) = self.instance_status(instance);
                    let mut card = ui::card(t)
                        .child(ui::section_header(t, "Installation", None))
                        .child(div().flex().child(ui::status_badge(t, status, tone)))
                        .child(ui::muted(
                            t,
                            crate::instance_editor::instance_note(instance),
                        ));
                    if let Some(pack) = &instance.pack {
                        card = card.child(ui::caption(
                            t,
                            format!("{} {}", pack.name, pack.pack_version),
                        ));
                    }
                    if self.selected_id() == Some(id) {
                        card = card.child(ui::caption(t, "Active instance"));
                    } else {
                        let id = id.clone();
                        card = card.child(
                            div().flex().child(
                                AuroraButton::new("overview-select-instance", "Make active")
                                    .disabled(!self.can_select(&id))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.action("select", id.clone(), cx);
                                        cx.notify();
                                    })),
                            ),
                        );
                    }
                    out.push(card.into_any_element());
                }
                InstanceTab::Content => {
                    if let Some(content) = &self.content {
                        out.push(content.clone().into_any_element());
                    }
                }
                InstanceTab::Settings => {
                    if let Some(editor) = &self.instance_editor {
                        out.push(editor.clone().into_any_element());
                    }
                }
            }
            return out;
        }
        let mut out = Vec::new();
        if let Some(import) = &self.pack_import {
            out.push(import.clone().into_any_element());
        }
        if let Some(creator) = &self.instance_creator {
            out.push(creator.clone().into_any_element());
        }
        let Some(state) = &self.launcher else {
            return out;
        };
        if state.instances.is_empty() {
            out.push(
                ui::empty_state(
                    t,
                    "grid_view",
                    "No instances yet",
                    "Create an instance or import a modpack to get started.",
                )
                .into_any_element(),
            );
            return out;
        }
        let available = f32::from(window.viewport_size().width) - PAGE_CHROME;
        let columns = ((available + 12.) / 252.).floor().max(1.) as usize;
        let width = (available - 12. * (columns - 1) as f32) / columns as f32;
        let editor_busy = self
            .instance_editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).is_busy());
        out.push(
            div()
                .flex()
                .flex_wrap()
                .gap(px(12.))
                .children(state.instances.iter().map(|instance| {
                    let id = instance.id.clone();
                    let active = self.selected_id() == Some(id.as_str());
                    let (status, tone) = self.instance_status(instance);
                    ui::tile_button(t, SharedString::from(format!("instance-grid-{id}")), active)
                        .w(px(width.floor()))
                        .min_h(px(96.))
                        .overflow_hidden()
                        .disabled(editor_busy)
                        .accessibility_label(format!(
                            "{} · {} · {}",
                            instance.display_name,
                            instance_metadata(instance),
                            if active { "Active" } else { status }
                        ))
                        .child(instance_icon(
                            stored_icon(&state.icons, &id),
                            44.,
                            active,
                            t,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .min_w(px(0.))
                                .flex_1()
                                .child(ui::title_block(
                                    t,
                                    instance.display_name.clone(),
                                    Some(
                                        format!(
                                            "{} · {}",
                                            instance.minecraft_version,
                                            instance.platform_label()
                                        )
                                        .into(),
                                    ),
                                ))
                                .child(div().flex().child(ui::status_badge(
                                    t,
                                    if active { "Active" } else { status },
                                    tone,
                                ))),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_instance(id.clone(), InstanceTab::Overview, 0, window, cx);
                            cx.notify();
                        }))
                }))
                .into_any_element(),
        );
        out
    }

    fn instance_status(&self, instance: &api::InstanceSummary) -> (&'static str, ui::Tone) {
        match instance.state.as_str() {
            "ready" if crate::instance_editor::needs_attention(instance) => {
                ("Action needed", ui::Tone::Warning)
            }
            "ready" => self
                .readiness_badge(&instance.id)
                .filter(|_| self.selected_id() == Some(instance.id.as_str()))
                .unwrap_or(("Installed", ui::Tone::Success)),
            "installing" => ("Installing", ui::Tone::Accent),
            "failed" => ("Action needed", ui::Tone::Warning),
            _ => ("Pending", ui::Tone::Neutral),
        }
    }

    pub(crate) fn instance_switcher(
        &self,
        state: &api::LauncherState,
        current: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AuroraSelect {
        let entity = window.use_keyed_state("home-instance-switcher", cx, |_, cx| Switcher {
            trigger: cx.focus_handle(),
            menu: cx.focus_handle(),
            bounds: Rc::new(Cell::new(None)),
            open: false,
            highlight: 0,
        });
        let switcher = entity.read(cx);
        let (trigger, menu, bounds, open, highlight) = (
            switcher.trigger.clone(),
            switcher.menu.clone(),
            switcher.bounds.clone(),
            switcher.open,
            switcher.highlight,
        );
        let palette = crate::theme::theme(cx).clone();
        let t = &palette;
        let mut ids = Vec::new();
        let mut labels = Vec::new();
        let mut rows = Vec::new();
        for instance in &state.instances {
            let active = current == Some(instance.id.as_str());
            let (status, tone) = self.instance_status(instance);
            let version = format!(
                "{} · {}",
                instance.minecraft_version,
                instance.platform_label()
            );
            ids.push(instance.id.clone());
            labels.push(format!("{} · {version} · {status}", instance.display_name).into());
            rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .min_w(px(0.))
                    .child(instance_icon(
                        stored_icon(&state.icons, &instance.id),
                        36.,
                        active,
                        t,
                    ))
                    .child(
                        ui::title_block(t, instance.display_name.clone(), Some(version.into()))
                            .flex_1()
                            .min_w(px(0.)),
                    )
                    .child(
                        div()
                            .size(px(8.))
                            .flex_none()
                            .rounded_full()
                            .bg(match tone {
                                ui::Tone::Success => t.success,
                                ui::Tone::Warning => t.warning,
                                ui::Tone::Accent => t.accent,
                                _ => t.muted_foreground,
                            }),
                    )
                    .when(active, |row| {
                        row.child(symbols::icon(Some("check"), px(18.), t.ring))
                    })
                    .into_any_element(),
            );
        }
        labels.push("Manage instances".into());
        rows.push(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .pt(px(8.))
                .border_t_1()
                .border_color(t.border_subtle)
                .child(symbols::icon(
                    Some("grid_view"),
                    px(18.),
                    t.muted_foreground,
                ))
                .child("Manage instances")
                .into_any_element(),
        );
        let selected = ids.iter().position(|id| current == Some(id.as_str()));
        let current_name = selected.map(|i| state.instances[i].display_name.clone());
        AuroraSelect::new(
            "switch-instance",
            "Switch instance",
            labels,
            &trigger,
            &menu,
            bounds,
        )
        .width(px(330.))
        .instance_switcher(rows, px(348.))
        .trigger_value(current_name.map(SharedString::from))
        .selected(selected)
        .highlight(highlight.min(ids.len()))
        .open(open)
        .disabled(self.controls_blocked())
        .on_open_change({
            let entity = entity.clone();
            move |open, _, cx| {
                entity.update(cx, |state, cx| {
                    state.open = open;
                    if open {
                        state.highlight = selected.unwrap_or(0);
                    }
                    cx.notify();
                })
            }
        })
        .on_highlight(move |highlight, _, cx| {
            entity.update(cx, |state, cx| {
                if state.highlight != highlight {
                    state.highlight = highlight;
                    cx.notify();
                }
            })
        })
        .on_confirm(cx.processor(move |this, index: usize, window, cx| {
            if let Some(id) = ids.get(index) {
                this.action("select", id.clone(), cx);
            } else {
                this.open_page(3, window, cx);
            }
            cx.notify();
        }))
    }
}

struct NavHint(&'static str);
impl gpui::Render for NavHint {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        nav_hint(crate::theme::theme(cx), self.0)
    }
}

fn nav_hint(t: &AuroraTheme, label: &'static str) -> gpui::Div {
    div()
        .px(px(10.))
        .py(px(6.))
        .rounded(t.radius)
        .border_1()
        .border_color(t.border_subtle)
        .bg(t.raised)
        .text_color(t.foreground)
        .text_size(px(ui::TEXT_BODY))
        .child(label)
}
