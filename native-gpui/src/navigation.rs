//! Launcher sidebar, instance grid and detail routing, and the Home switcher.
use std::{cell::Cell, rc::Rc};

use gpui::{
    AnyElement, Bounds, Context, FocusHandle, FontWeight, Hsla, IntoElement, Pixels, SharedString,
    Window, deferred, div, prelude::*, px,
};
use gpui_kit::base::{Align, Button, ElementExt, Placement, Positioner};

use crate::{
    InstanceTab, Launcher, api, aurora_instance_icon, chrome,
    controls::{AuroraButton, AuroraInput, AuroraSelect, ButtonSize, ButtonVariant},
    downloads, images, instance_icon, instance_metadata, stored_icon, symbols,
    theme::AuroraTheme,
    ui,
};

const SIDEBAR_EXPANDED: f32 = 204.;
const SIDEBAR_COLLAPSED: f32 = 90.;
const SIDEBAR_AUTOCOLLAPSE_WIDTH: f32 = 980.;
pub(crate) const PAGE_GUTTER: f32 = 32.;
pub(crate) const PAGE_CHROME: f32 = SIDEBAR_COLLAPSED + 2. * PAGE_GUTTER;

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

/// Primary navigation destinations: crisp labeled compact rail with Pulsar's own clear home.
const PRIMARY_NAV: &[(&str, u8, &str)] = &[
    ("Home", 0, "home"),
    ("Instances", 3, "grid_view"),
    ("Discover", 5, "explore"),
    ("Pulsar", 9, "timeline"),
    ("Skins", 7, "checkroom"),
];

const UTILITY_NAV: &[(&str, u8, &str)] = &[("Settings", 2, "settings")];

pub(crate) fn sidebar_expanded(viewport_width: Pixels, collapsed: bool) -> bool {
    !collapsed && f32::from(viewport_width) >= SIDEBAR_AUTOCOLLAPSE_WIDTH
}

pub(crate) fn sidebar_width(viewport_width: Pixels, collapsed: bool) -> f32 {
    if sidebar_expanded(viewport_width, collapsed) {
        SIDEBAR_EXPANDED
    } else {
        SIDEBAR_COLLAPSED
    }
}

/// Concise grid loader label (e.g. `Fabric`, `Forge`, `Vanilla`) without noisy patch digits.
pub(crate) fn short_platform_label(instance: &api::InstanceSummary) -> &'static str {
    match instance.platform.kind.as_str() {
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "vanilla" => "Vanilla",
        _ => "Modded",
    }
}

/// Presentation-clean loader label: avoids repeating the Minecraft version in Forge builds
/// (e.g. `1.20.1 · Forge 47.3.0` instead of `1.20.1 · Forge 1.20.1-47.3.0`).
pub(crate) fn clean_platform_label(instance: &api::InstanceSummary) -> String {
    let raw = instance.platform_label();
    let prefix = format!("Forge {}-", instance.minecraft_version);
    if let Some(rest) = raw.strip_prefix(&prefix) {
        format!("Forge {rest}")
    } else {
        raw
    }
}

impl Launcher {
    pub(crate) fn navigation_rail(
        &self,
        t: &AuroraTheme,
        downloads_active: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let viewport = window.viewport_size();
        let expanded = sidebar_expanded(viewport.width, self.sidebar_collapsed);
        let compact = viewport.height < px(660.);
        let width = if expanded {
            SIDEBAR_EXPANDED
        } else {
            SIDEBAR_COLLAPSED
        };
        let has_updates = self.updates_pending();
        let running = self.client_update_running(cx);

        // Top brand lockup + collapse toggle (drag region stays separate from the toggle button).
        let brand_row = if expanded {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .w_full()
                .h(px(52.))
                .px(px(14.))
                .child(chrome::drag_region(
                    div()
                        .id("brand")
                        .flex()
                        .flex_1()
                        .min_w(px(0.))
                        .h_full()
                        .items_center()
                        .gap(px(10.))
                        .child(gpui::img(images::aurora_logo()).w(px(30.)).h(px(24.)))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .line_height(px(17.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(t.foreground)
                                        .child("Aurora Launcher"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .line_height(px(14.))
                                        .text_color(t.subtle_foreground)
                                        .child("for Aurora Client"),
                                ),
                        ),
                ))
                .child(
                    Button::new("sidebar-collapse-toggle")
                        .accessibility_label("Collapse sidebar")
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(28.))
                        .rounded(t.radius)
                        .hover(|s| s.bg(t.control_hover))
                        .child(symbols::icon(
                            Some("chevron_left"),
                            px(18.),
                            t.muted_foreground,
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sidebar_collapsed = true;
                            cx.notify();
                        })),
                )
        } else {
            div()
                .flex()
                .flex_none()
                .flex_col()
                .items_center()
                .w_full()
                .pt(px(12.))
                .pb(px(10.))
                .child(chrome::drag_region(
                    Button::new("sidebar-expand-toggle")
                        .accessibility_label("Expand sidebar")
                        .flex()
                        .w_full()
                        .h(px(42.))
                        .items_center()
                        .justify_center()
                        .rounded(t.radius)
                        .hover(|s| s.bg(t.control_hover))
                        .child(gpui::img(images::aurora_logo()).w(px(36.)).h(px(28.)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.sidebar_collapsed {
                                this.sidebar_collapsed = false;
                                cx.notify();
                            }
                        })),
                ))
        };

        let render_nav_item = |label: &'static str,
                               page: u8,
                               icon: &'static str,
                               window: &mut Window,
                               cx: &mut Context<Self>| {
            let selected = self.page == page || (page == 3 && self.page == 6);
            let disabled = running && !selected;
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
            let item_h = px(if compact {
                46.
            } else if expanded {
                42.
            } else {
                58.
            });
            let icon_color = if disabled {
                t.subtle_foreground
            } else if selected {
                t.ring
            } else {
                Hsla {
                    a: 0.82,
                    ..t.foreground
                }
            };
            let label_color = if disabled {
                t.subtle_foreground
            } else if selected {
                t.foreground
            } else {
                Hsla {
                    a: 0.76,
                    ..t.foreground
                }
            };
            let button = Button::new(("rail-action", page as usize))
                .accessibility_label(accessible_label)
                .when(dot, |b| b.aria_description("Updates available"))
                .track_focus(&focus)
                .selected(selected)
                .disabled(disabled)
                .relative()
                .flex_none()
                .flex()
                .items_center()
                .when(expanded, |b| b.w_full().px(px(10.)).gap(px(10.)))
                .when(!expanded, |b| {
                    b.flex_col().w(px(72.)).justify_center().gap(px(4.))
                })
                .h(item_h)
                .rounded(px(12.))
                .border_1()
                .border_color(if selected {
                    Hsla { a: 0.52, ..t.ring }
                } else {
                    gpui::transparent_black()
                })
                .hover(move |style| style.bg(hover))
                .focus_visible(move |style| style.border_color(ring))
                .when(selected, |b| {
                    let bright = Hsla {
                        l: (t.ring.l + 0.06).min(0.78),
                        a: 0.28,
                        ..t.ring
                    };
                    let deep = Hsla { a: 0.16, ..t.ring };
                    b.bg(gpui::linear_gradient(
                        135.,
                        gpui::linear_color_stop(bright, 0.),
                        gpui::linear_color_stop(deep, 1.),
                    ))
                })
                .child(symbols::icon(
                    Some(icon),
                    px(if expanded { 22. } else { 26. }),
                    icon_color,
                ))
                .when(!expanded, |b| {
                    b.child(
                        div()
                            .text_size(px(11.))
                            .line_height(px(13.))
                            .font_weight(if selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(label_color)
                            .child(label),
                    )
                })
                .when(expanded, |b| {
                    b.child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_ellipsis()
                            .text_size(px(ui::TEXT_BODY))
                            .font_weight(if selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(label_color)
                            .child(label),
                    )
                })
                .when(dot, |b| {
                    if expanded {
                        b.child(ui::update_dot(t))
                    } else {
                        b.child(
                            div()
                                .absolute()
                                .top(px(7.))
                                .right(px(7.))
                                .child(ui::update_dot(t)),
                        )
                    }
                })
                .when(count > 0, |b| {
                    let pill = div()
                        .min_w(px(18.))
                        .h(px(18.))
                        .px(px(5.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(t.accent)
                        .text_color(t.accent_foreground)
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(count.to_string());
                    if expanded {
                        b.child(pill)
                    } else {
                        b.child(div().absolute().top(px(4.)).right(px(4.)).child(pill))
                    }
                })
                .when(!expanded && !is_focused, |b| {
                    b.tooltip(move |_, cx| cx.new(|_| NavHint(label)).into())
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_page(page, window, cx);
                    cx.notify();
                }));

            div()
                .id(("rail-hint", page as usize))
                .flex_none()
                .when(expanded, |d| d.w_full())
                .on_prepaint({
                    let bounds = bounds.clone();
                    move |rect, _, _| bounds.set(Some(rect))
                })
                .child(button)
                .when_some(
                    bounds
                        .get()
                        .filter(|_| !expanded && is_focused && !disabled),
                    |item, bounds| {
                        item.child(
                            deferred(
                                Positioner::side(bounds)
                                    .placement(Placement::Right)
                                    .align(Align::Center)
                                    .offset(px(14.))
                                    .child(nav_hint(t, label)),
                            )
                            .with_priority(2),
                        )
                    },
                )
        };

        let mut nav_body = div()
            .id("launcher-rail-nav")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .overflow_y_scroll()
            .pt(px(8.))
            .when(expanded, |d| d.px(px(10.)).gap(px(6.)))
            .when(!expanded, |d| d.items_center().gap(px(10.)));

        for &(label, page, icon) in PRIMARY_NAV {
            nav_body = nav_body.child(render_nav_item(label, page, icon, window, cx));
        }

        // Bottom utility group + Account footer chip.
        let mut footer = div()
            .flex()
            .flex_none()
            .flex_col()
            .w_full()
            .gap(px(4.))
            .pt(px(8.))
            .pb(px(10.))
            .border_t_1()
            .border_color(t.border_subtle)
            .when(expanded, |d| d.px(px(10.)))
            .when(!expanded, |d| d.items_center());

        for &(label, page, icon) in UTILITY_NAV {
            footer = footer.child(render_nav_item(label, page, icon, window, cx));
        }

        // Persistent Account chip at bottom of sidebar (Modrinth / Pandora pattern).
        let active_account = self
            .accounts
            .selected_account_id
            .as_deref()
            .and_then(|id| self.accounts.accounts.iter().find(|a| a.account_id == id));
        let account_selected = self.page == 1;
        let account_signed_in = active_account.is_some_and(|a| a.status == "signedIn");
        let account_name = active_account
            .map(|a| a.minecraft_name.clone())
            .unwrap_or_else(|| "Sign in".into());
        let account_sub = match active_account {
            Some(a) if a.status == "signedIn" => "Minecraft account",
            Some(_) => "Sign in again",
            None => "Microsoft account",
        };
        let dot_color = if account_signed_in {
            t.success
        } else {
            t.warning
        };
        let ring = t.ring;
        let hover_bg = t.control_hover;
        let account_chip = Button::new("rail-account-chip")
            .accessibility_label(format!("Accounts: {account_name}"))
            .selected(account_selected)
            .disabled(running && !account_selected)
            .flex()
            .flex_none()
            .items_center()
            .when(expanded, |b| {
                b.w_full().mt(px(4.)).px(px(8.)).py(px(6.)).gap(px(10.))
            })
            .when(!expanded, |b| {
                b.mt(px(4.))
                    .w(px(62.))
                    .py(px(6.))
                    .flex_col()
                    .justify_center()
                    .gap(px(4.))
            })
            .rounded(t.radius)
            .bg(if account_selected {
                t.accent_soft
            } else {
                gpui::transparent_black()
            })
            .border_1()
            .border_color(if account_selected {
                Hsla { a: 0.45, ..t.ring }
            } else {
                gpui::transparent_black()
            })
            .hover(move |s| s.bg(hover_bg))
            .focus_visible(move |s| s.border_color(ring))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(34.))
                    .rounded(px(10.))
                    .overflow_hidden()
                    .bg(if account_signed_in {
                        t.accent_soft
                    } else {
                        t.sunken
                    })
                    .border_1()
                    .border_color(if account_signed_in {
                        Hsla { a: 0.48, ..t.ring }
                    } else {
                        t.border_subtle
                    })
                    .child(if let Some(avatar) = self.rail_avatar.clone() {
                        gpui::img(avatar).size(px(34.)).into_any_element()
                    } else {
                        symbols::icon(
                            Some("person"),
                            px(20.),
                            if account_signed_in {
                                t.foreground
                            } else {
                                t.muted_foreground
                            },
                        )
                        .into_any_element()
                    })
                    .when(!account_signed_in, |tile| {
                        tile.child(
                            div()
                                .absolute()
                                .bottom(px(-2.))
                                .right(px(-2.))
                                .size(px(9.))
                                .rounded_full()
                                .bg(dot_color)
                                .border_1()
                                .border_color(t.sidebar),
                        )
                    }),
            )
            .when(!expanded, |b| {
                let short_label = if account_signed_in {
                    account_name.clone()
                } else {
                    "Account".to_owned()
                };
                b.child(
                    div()
                        .max_w(px(62.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(px(11.))
                        .font_weight(if account_selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .text_color(if account_selected {
                            t.foreground
                        } else {
                            t.muted_foreground
                        })
                        .child(short_label),
                )
            })
            .when(expanded, |b| {
                b.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .child(
                            div()
                                .text_ellipsis()
                                .text_size(px(ui::TEXT_CAPTION))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.foreground)
                                .child(account_name.clone()),
                        )
                        .child(
                            div()
                                .text_ellipsis()
                                .text_size(px(11.))
                                .text_color(t.subtle_foreground)
                                .child(account_sub),
                        ),
                )
            })
            .when(!expanded, |b| {
                b.tooltip(move |_, cx| cx.new(|_| NavHint("Accounts")).into())
            })
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_page(1, window, cx);
                cx.notify();
            }));
        footer = footer.child(account_chip);

        div()
            .id("launcher-rail")
            .w(px(width))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(t.sidebar)
            .border_r_1()
            .border_color(t.border_subtle)
            .child(brand_row)
            .child(nav_body)
            .child(footer)
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
            return self.instance_detail_children(t, id, *tab, state, instance, window, cx);
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
                    "Create an instance or import a .mrpack modpack to start playing with Aurora Client.",
                )
                .into_any_element(),
            );
            return out;
        }

        let total_count = state.instances.len();
        let aurora_count = state
            .instances
            .iter()
            .filter(|i| i.aurora.is_some())
            .count();
        let other_count = total_count.saturating_sub(aurora_count);

        // Search, filter, sort, and tag toolbar.
        let mut toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(10.))
            .w_full();
        let mut left_controls = div().flex().flex_wrap().items_center().gap(px(8.));
        if let Some(search) = &self.instances_search {
            left_controls = left_controls.child(
                div()
                    .w(px(220.))
                    .child(AuroraInput::new(search).label("Search instances")),
            );
        }
        if aurora_count > 0 && other_count > 0 {
            let mut filter_tabs = ui::segmented_tabs(t);
            for (idx, label) in [(0u8, "All"), (1u8, "Aurora Client"), (2u8, "Other")] {
                filter_tabs = filter_tabs.child(
                    AuroraButton::new(("instances-filter", idx as usize), label)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .selected(self.instances_filter == idx)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.instances_filter = idx;
                            cx.notify();
                        })),
                );
            }
            left_controls = left_controls.child(filter_tabs);
        }
        let mut sort_tabs = ui::segmented_tabs(t);
        for (idx, label) in [(0u8, "Pinned"), (1u8, "Name"), (2u8, "Version")] {
            sort_tabs = sort_tabs.child(
                AuroraButton::new(("instances-sort", idx as usize), label)
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Small)
                    .selected(self.instances_sort == idx)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.instances_sort = idx;
                        cx.notify();
                    })),
            );
        }
        toolbar = toolbar.child(left_controls).child(sort_tabs);
        out.push(toolbar.into_any_element());

        let mut all_tags: Vec<String> = self
            .organization
            .instances
            .values()
            .flat_map(|e| e.tags.iter().cloned())
            .collect();
        all_tags.sort();
        all_tags.dedup();
        if !all_tags.is_empty() {
            let mut tag_row = div().flex().flex_wrap().items_center().gap(px(6.));
            tag_row = tag_row.child(
                AuroraButton::new("inst-tag-all", "All tags")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Small)
                    .selected(self.instances_tag_filter.is_none())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.instances_tag_filter = None;
                        cx.notify();
                    })),
            );
            for (ti, tag) in all_tags.iter().enumerate() {
                let tval = tag.clone();
                let active_tag = self.instances_tag_filter.as_deref() == Some(tag.as_str());
                tag_row = tag_row.child(
                    AuroraButton::new(("inst-tag-chip", ti), format!("#{tag}"))
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .selected(active_tag)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.instances_tag_filter.as_deref() == Some(tval.as_str()) {
                                this.instances_tag_filter = None;
                            } else {
                                this.instances_tag_filter = Some(tval.clone());
                            }
                            cx.notify();
                        })),
                );
            }
            out.push(tag_row.into_any_element());
        }

        let search_q = self
            .instances_search
            .as_ref()
            .map(|s| s.read(cx).value().trim().to_ascii_lowercase())
            .unwrap_or_default();

        let mut visible: Vec<&api::InstanceSummary> = state
            .instances
            .iter()
            .filter(|i| match self.instances_filter {
                1 => i.aurora.is_some(),
                2 => i.aurora.is_none(),
                _ => true,
            })
            .filter(|i| {
                if let Some(tag) = &self.instances_tag_filter {
                    let has = self
                        .organization
                        .instances
                        .get(&i.id)
                        .is_some_and(|e| e.tags.iter().any(|t| t == tag));
                    if !has {
                        return false;
                    }
                }
                if search_q.is_empty() {
                    return true;
                }
                i.display_name.to_ascii_lowercase().contains(&search_q)
                    || i.minecraft_version.to_ascii_lowercase().contains(&search_q)
                    || short_platform_label(i)
                        .to_ascii_lowercase()
                        .contains(&search_q)
                    || self.organization.instances.get(&i.id).is_some_and(|e| {
                        e.tags
                            .iter()
                            .any(|t| t.to_ascii_lowercase().contains(&search_q))
                    })
            })
            .collect();

        visible.sort_by(|a, b| {
            let a_pin = self
                .organization
                .instances
                .get(&a.id)
                .is_some_and(|e| e.pinned);
            let b_pin = self
                .organization
                .instances
                .get(&b.id)
                .is_some_and(|e| e.pinned);
            match self.instances_sort {
                1 => a
                    .display_name
                    .to_ascii_lowercase()
                    .cmp(&b.display_name.to_ascii_lowercase()),
                2 => b.minecraft_version.cmp(&a.minecraft_version).then_with(|| {
                    a.display_name
                        .to_ascii_lowercase()
                        .cmp(&b.display_name.to_ascii_lowercase())
                }),
                _ => b_pin.cmp(&a_pin).then_with(|| {
                    a.display_name
                        .to_ascii_lowercase()
                        .cmp(&b.display_name.to_ascii_lowercase())
                }),
            }
        });

        let sb_width = sidebar_width(window.viewport_size().width, self.sidebar_collapsed);
        let available = (f32::from(window.viewport_size().width) - sb_width - 2. * PAGE_GUTTER)
            .min(ui::MAX_CONTENT_WIDTH);
        let columns = ((available + 16.) / 460.).floor().clamp(1., 3.) as u16;
        let editor_busy = self
            .instance_editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).is_busy());

        let mut grid = div().grid().grid_cols(columns).gap(px(16.)).w_full();

        for &instance in &visible {
            let id = instance.id.clone();
            let active = self.selected_id() == Some(id.as_str());
            let (status, _tone) = self.instance_status(instance);
            let has_aurora = instance.aurora.is_some();
            let can_kill = self.can_kill(&id);
            let needs_sign_in = active
                && !can_kill
                && self
                    .visible_blockers(&id)
                    .iter()
                    .any(|b| b.code == "launch_authentication_required");

            let card_border = if active {
                Hsla { a: 0.85, ..t.ring }
            } else if has_aurora {
                Hsla { a: 0.26, ..t.ring }
            } else {
                t.border_subtle
            };

            let open_id = id.clone();
            let play_or_select_id = id.clone();
            let hover_bg = t.control_hover;

            // Uniform circular card-level Play control (accent fill on hover only so
            // active selection is signaled cleanly by the card outline alone).
            let play_hover = if can_kill { t.danger_hover } else { t.accent };
            let play_ring = t.ring;
            let primary_action = if active {
                let icon = if can_kill {
                    "stop"
                } else if needs_sign_in {
                    "login"
                } else {
                    "play_arrow"
                };
                let label = if needs_sign_in {
                    "Sign in to play".to_owned()
                } else {
                    format!("{} {}", self.play_label(&id), instance.display_name)
                };
                Button::new(SharedString::from(format!("inst-card-play-{id}")))
                    .accessibility_label(label)
                    .disabled(if needs_sign_in {
                        self.controls_blocked()
                    } else {
                        !can_kill && !self.can_play(&id)
                    })
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .rounded_full()
                    .bg(if can_kill { t.danger } else { t.accent_soft })
                    .border_1()
                    .border_color(if can_kill {
                        t.danger
                    } else {
                        Hsla { a: 0.45, ..t.ring }
                    })
                    .hover(move |s| s.bg(play_hover))
                    .focus_visible(move |s| s.border_1().border_color(play_ring))
                    .child(symbols::icon(
                        Some(icon),
                        px(18.),
                        if can_kill {
                            t.accent_foreground
                        } else {
                            t.foreground
                        },
                    ))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if can_kill {
                            this.request_kill(play_or_select_id.clone(), cx);
                        } else if needs_sign_in {
                            this.open_page(1, window, cx);
                        } else {
                            this.action("play", play_or_select_id.clone(), cx);
                        }
                        cx.notify();
                    }))
            } else {
                Button::new(SharedString::from(format!("inst-card-select-{id}")))
                    .accessibility_label(format!("Play {}", instance.display_name))
                    .disabled(!self.can_select(&id) || editor_busy)
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .rounded_full()
                    .bg(t.control)
                    .border_1()
                    .border_color(t.border)
                    .hover(move |s| s.bg(play_hover))
                    .focus_visible(move |s| s.border_color(play_ring))
                    .child(symbols::icon(Some("play_arrow"), px(18.), t.foreground))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.action("select", play_or_select_id.clone(), cx);
                        cx.notify();
                    }))
            };

            let is_pinned = self
                .organization
                .instances
                .get(&id)
                .is_some_and(|e| e.pinned);
            let card_tags: Vec<String> = self
                .organization
                .instances
                .get(&id)
                .map(|e| e.tags.clone())
                .unwrap_or_default();
            let pin_id = id.clone();
            let pin_tags = card_tags.clone();
            let pin_button = Button::new(SharedString::from(format!("inst-card-pin-{id}")))
                .accessibility_label(if is_pinned {
                    format!("Unpin {}", instance.display_name)
                } else {
                    format!("Pin {}", instance.display_name)
                })
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(28.))
                .rounded_full()
                .hover(move |s| s.bg(hover_bg))
                .child(
                    div()
                        .text_size(px(14.))
                        .text_color(if is_pinned {
                            t.ring
                        } else {
                            t.subtle_foreground
                        })
                        .child(if is_pinned { "★" } else { "☆" }),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_instance_pin(pin_id.clone(), !is_pinned, pin_tags.clone(), cx);
                }));

            let card = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .p(px(14.))
                .rounded(t.radius_lg)
                .bg(if active {
                    Hsla { a: 0.14, ..t.ring }.blend(t.card)
                } else {
                    t.card
                })
                .border_1()
                .border_color(card_border)
                .min_w(px(0.))
                .child(
                    Button::new(SharedString::from(format!("instance-grid-{id}")))
                        .disabled(editor_busy)
                        .accessibility_label(format!(
                            "Open {} · {} · {}",
                            instance.display_name,
                            instance_metadata(instance),
                            status
                        ))
                        .flex()
                        .flex_1()
                        .items_center()
                        .gap(px(12.))
                        .min_w(px(0.))
                        .p(px(4.))
                        .m(px(-4.))
                        .rounded(t.radius)
                        .hover(move |s| s.bg(hover_bg))
                        .child(aurora_instance_icon(
                            stored_icon(&state.icons, &id),
                            56.,
                            active || has_aurora,
                            has_aurora,
                            t,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .flex_1()
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .min_w(px(0.))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_size(px(ui::TEXT_SECTION + 1.))
                                        .line_height(px(22.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(t.foreground)
                                        .child(instance.display_name.clone()),
                                )
                                .child(
                                    div().flex().items_center().gap(px(5.)).min_w(px(0.)).child(
                                        div()
                                            .min_w(px(0.))
                                            .overflow_hidden()
                                            .text_size(px(ui::TEXT_CAPTION))
                                            .line_height(px(18.))
                                            .text_color(t.muted_foreground)
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .child(match instance.aurora.as_ref() {
                                                Some(pin) => format!(
                                                    "{} · {} · Aurora Client {}",
                                                    instance.minecraft_version,
                                                    short_platform_label(instance),
                                                    pin.version
                                                ),
                                                None => format!(
                                                    "{} · {}",
                                                    instance.minecraft_version,
                                                    short_platform_label(instance)
                                                ),
                                            }),
                                    ),
                                )
                                .when(!card_tags.is_empty(), |col| {
                                    col.child(div().flex().flex_wrap().gap(px(4.)).children(
                                        card_tags.iter().take(4).map(|tag| {
                                            ui::badge(t, format!("#{tag}"), ui::Tone::Neutral)
                                        }),
                                    ))
                                }),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_instance(
                                open_id.clone(),
                                InstanceTab::Overview,
                                0,
                                window,
                                cx,
                            );
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(pin_button)
                        .child(primary_action),
                );
            grid = grid.child(card);
        }

        if visible.is_empty() {
            out.push(
                ui::empty_state(
                    t,
                    "search",
                    "No matching instances",
                    "Try a different search term or clear your active filter.",
                )
                .child(
                    AuroraButton::new("instances-clear-filters", "Clear filters")
                        .size(ButtonSize::Small)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.instances_filter = 0;
                            this.instances_tag_filter = None;
                            if let Some(input) = &this.instances_search {
                                input.update(cx, |state, cx| {
                                    state.set_value("", window, cx);
                                });
                            }
                            cx.notify();
                        })),
                )
                .into_any_element(),
            );
            return out;
        }

        out.push(grid.into_any_element());
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn instance_detail_children(
        &self,
        t: &AuroraTheme,
        id: &str,
        tab: InstanceTab,
        state: &api::LauncherState,
        instance: &api::InstanceSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let editor_busy = self
            .instance_editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).is_busy());
        let active = self.selected_id() == Some(id);
        let (status, tone) = self.instance_status(instance);
        let can_kill = self.can_kill(id);
        let needs_sign_in = active
            && !can_kill
            && self
                .visible_blockers(id)
                .iter()
                .any(|b| b.code == "launch_authentication_required");

        let back = AuroraButton::new("instances-back", "Instances")
            .size(ButtonSize::Small)
            .variant(ButtonVariant::Ghost)
            .icon("arrow_back")
            .disabled(editor_busy)
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_page(3, window, cx);
                cx.notify();
            }));

        let _ = back;
        let mut header_actions = div().flex().flex_wrap().items_center().gap(px(8.));
        if active {
            let play_id = id.to_owned();
            header_actions = header_actions.child(
                AuroraButton::new(
                    "detail-play",
                    if needs_sign_in {
                        "Sign in"
                    } else {
                        self.play_label(id)
                    },
                )
                .variant(if can_kill {
                    ButtonVariant::Danger
                } else {
                    ButtonVariant::Primary
                })
                .icon(if can_kill {
                    "stop"
                } else if needs_sign_in {
                    "login"
                } else {
                    "play_arrow"
                })
                .disabled(if needs_sign_in {
                    self.controls_blocked()
                } else {
                    !can_kill && !self.can_play(id)
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    if can_kill {
                        this.request_kill(play_id.clone(), cx);
                    } else if needs_sign_in {
                        this.open_page(1, window, cx);
                    } else {
                        this.action("play", play_id.clone(), cx);
                    }
                    cx.notify();
                })),
            );
        } else {
            let select_id = id.to_owned();
            header_actions = header_actions.child(
                AuroraButton::new("detail-make-active", "Select")
                    .variant(ButtonVariant::Primary)
                    .icon("check_circle")
                    .disabled(!self.can_select(id) || editor_busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.action("select", select_id.clone(), cx);
                        cx.notify();
                    })),
            );
        }
        let folder_id = id.to_owned();
        let log_id = id.to_owned();
        header_actions = header_actions
            .child(
                AuroraButton::new("detail-open-folder", "Folder")
                    .variant(ButtonVariant::Default)
                    .disabled(self.host.is_none())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(editor) = &this.instance_editor {
                            editor.update(cx, |ed, cx| ed.open_folder_for(folder_id.clone(), cx));
                        }
                    })),
            )
            .child(
                AuroraButton::new("detail-game-log", "View log")
                    .variant(ButtonVariant::Default)
                    .disabled(self.host.is_none())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_game_log(log_id.clone(), cx);
                    })),
            );

        let mut meta_badges =
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(10.))
                .child(ui::muted(
                    t,
                    format!(
                        "{} · {}",
                        instance.minecraft_version,
                        short_platform_label(instance)
                    ),
                ));
        if let Some(pin) = &instance.aurora {
            meta_badges = meta_badges.child(ui::aurora_badge(
                t,
                format!("Aurora Client {}", pin.version),
            ));
        }
        meta_badges = meta_badges.child(ui::status_badge(t, status, tone));

        let header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .min_w(px(0.))
                    .child(aurora_instance_icon(
                        stored_icon(&state.icons, id),
                        68.,
                        active || instance.aurora.is_some(),
                        instance.aurora.is_some(),
                        t,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .min_w(px(0.))
                            .child(
                                div()
                                    .text_size(px(ui::TEXT_TITLE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.foreground)
                                    .child(instance.display_name.clone()),
                            )
                            .child(meta_badges),
                    ),
            )
            .child(header_actions);

        // ONE unified text-only top-level tab strip (Overview, Content, Worlds, Screenshots, Options).
        let tabs = div().flex().child(
            ui::segmented_tabs(t).children(
                [
                    (InstanceTab::Overview, "Overview"),
                    (InstanceTab::Content, "Content"),
                    (InstanceTab::Worlds, "Worlds"),
                    (InstanceTab::Screenshots, "Screenshots"),
                    (InstanceTab::Settings, "Options"),
                ]
                .into_iter()
                .map(|(next, label)| {
                    let target_id = id.to_owned();
                    AuroraButton::new(("instance-detail-tab", next as usize), label)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .selected(tab == next)
                        .disabled(editor_busy)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this
                                .instance_detail
                                .as_ref()
                                .is_some_and(|(_, current)| *current != next)
                            {
                                this.open_instance(target_id.clone(), next, 0, window, cx);
                                cx.notify();
                            }
                        }))
                }),
            ),
        );

        let mut out = vec![header.into_any_element(), tabs.into_any_element()];

        match tab {
            InstanceTab::Overview => {
                let stacked = window.viewport_size().width < px(860.);
                let mem = if instance.configuration.memory_mib % 1024 == 0 {
                    format!("{} GB", instance.configuration.memory_mib / 1024)
                } else {
                    format!(
                        "{:.1} GB",
                        instance.configuration.memory_mib as f32 / 1024.0
                    )
                };
                let java = if instance.configuration.custom_java().is_some() {
                    "Custom Java"
                } else {
                    "Managed Java"
                };
                let client_offer = self.client_offer(instance).map(str::to_owned);

                // Row 1: Clean Aurora Client status card (Pulsar & Updates actions unified; zero decorative icon duplication).
                let aurora_hero = match (&instance.aurora, instance.platform.kind.as_str()) {
                    (Some(pin), _) => {
                        let channel_label = if pin.channel.eq_ignore_ascii_case("stable") {
                            "Stable".to_owned()
                        } else {
                            pin.channel.clone()
                        };
                        ui::aurora_card(t).p(px(20.)).child(
                            div()
                                .relative()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap(px(16.))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .text_size(px(ui::TEXT_SECTION + 2.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(t.foreground)
                                                .child("Aurora Client"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_wrap()
                                                .items_center()
                                                .gap(px(10.))
                                                .child(ui::status_badge(
                                                    t,
                                                    if let Some(ver) = &client_offer {
                                                        format!("v{ver} available")
                                                    } else {
                                                        "Verified".to_owned()
                                                    },
                                                    if client_offer.is_some() {
                                                        ui::Tone::Accent
                                                    } else {
                                                        ui::Tone::Success
                                                    },
                                                ))
                                                .child(ui::caption(
                                                    t,
                                                    format!("{channel_label} channel"),
                                                )),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(
                                            AuroraButton::new("overview-open-pulsar", "Pulsar")
                                                .variant(ButtonVariant::Default)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.open_page(9, window, cx);
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            AuroraButton::new(
                                                "overview-client-updates",
                                                if client_offer.is_some() {
                                                    "Update"
                                                } else {
                                                    "Check updates"
                                                },
                                            )
                                            .variant(if client_offer.is_some() {
                                                ButtonVariant::Primary
                                            } else {
                                                ButtonVariant::Default
                                            })
                                            .on_click(
                                                {
                                                    let target_id = id.to_owned();
                                                    cx.listener(move |this, _, window, cx| {
                                                        this.client_update_instance =
                                                            Some(target_id.clone());
                                                        this.open_page(2, window, cx);
                                                        if let Some(settings) = &this.settings {
                                                            settings.update(cx, |s, cx| {
                                                                s.select_section(3, cx)
                                                            });
                                                        }
                                                        cx.notify();
                                                    })
                                                },
                                            ),
                                        ),
                                ),
                        )
                    }
                    (None, "fabric") => {
                        let settings_id = id.to_owned();
                        ui::card(t).child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .justify_between()
                                .gap(px(16.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(14.))
                                        .child(instance_icon(Some("auto_awesome"), 54., false, t))
                                        .child(ui::title_block(
                                            t,
                                            "Aurora Client",
                                            Some("Not installed".into()),
                                        )),
                                )
                                .child(
                                    AuroraButton::new(
                                        "overview-install-aurora",
                                        "Install Aurora Client",
                                    )
                                    .variant(ButtonVariant::Soft)
                                    .icon("auto_awesome")
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.open_instance(
                                                settings_id.clone(),
                                                InstanceTab::Settings,
                                                0,
                                                window,
                                                cx,
                                            );
                                            cx.notify();
                                        },
                                    )),
                                ),
                        )
                    }
                    (None, _) => ui::card(t).child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(14.))
                            .child(instance_icon(Some("auto_awesome"), 52., false, t))
                            .child(ui::title_block(
                                t,
                                "Aurora Client",
                                Some("Requires Fabric".into()),
                            )),
                    ),
                };
                out.push(aurora_hero.into_any_element());

                // Row 2: Equal-weight stat tiles for runtime and recent activity.
                let runtime_line = format!("{mem} · {java}");
                let recent_server = self.hero_activity.get(id).and_then(|rows| {
                    rows.iter()
                        .find(|(k, _)| k.as_ref() == "Server")
                        .map(|(_, v)| v.clone())
                });
                let mut tiles: Vec<(SharedString, SharedString)> =
                    vec![("Runtime".into(), runtime_line.into())];
                if let Some(server) = recent_server {
                    tiles.push(("Last server".into(), server));
                }
                if let Some(rows) = self.hero_activity.get(id) {
                    for (k, v) in rows.iter().filter(|(k, _)| k.as_ref() != "Server").take(2) {
                        tiles.push((k.clone(), v.clone()));
                    }
                }
                let tile_cols = if stacked {
                    1
                } else {
                    tiles.len().min(2) as u16
                };
                let stat_grid = div()
                    .grid()
                    .grid_cols(tile_cols)
                    .gap(px(14.))
                    .w_full()
                    .children(tiles.into_iter().map(|(label, value)| {
                        ui::card(t)
                            .px(px(20.))
                            .py(px(18.))
                            .gap(px(0.))
                            .justify_center()
                            .child(ui::stat(t, label, value))
                    }));
                out.push(stat_grid.into_any_element());
            }
            InstanceTab::Content => {
                if let Some(content) = &self.content {
                    out.push(content.clone().into_any_element());
                }
            }
            InstanceTab::Worlds => {
                if let Some(worlds) = &self.worlds {
                    out.push(worlds.clone().into_any_element());
                }
            }
            InstanceTab::Screenshots => {
                if let Some(editor) = &self.instance_editor {
                    out.push(editor.read(cx).screenshots_view().into_any_element());
                }
            }
            InstanceTab::Settings => {
                if let Some(editor) = &self.instance_editor {
                    out.push(editor.clone().into_any_element());
                }
            }
        }
        out
    }

    pub(crate) fn instance_status(
        &self,
        instance: &api::InstanceSummary,
    ) -> (&'static str, ui::Tone) {
        match instance.state.as_str() {
            "ready" if crate::instance_editor::needs_attention(instance) => {
                ("Needs attention", ui::Tone::Warning)
            }
            "ready" => self
                .readiness_badge(&instance.id)
                .filter(|_| self.selected_id() == Some(instance.id.as_str()))
                .unwrap_or(("Installed", ui::Tone::Success)),
            "installing" => ("Installing", ui::Tone::Accent),
            "failed" => ("Install failed", ui::Tone::Danger),
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
        let mut ids = Vec::with_capacity(state.instances.len());
        let mut labels = Vec::with_capacity(state.instances.len() + 1);
        let mut rows = Vec::with_capacity(if open { state.instances.len() + 1 } else { 0 });
        for instance in &state.instances {
            ids.push(instance.id.clone());
            if open {
                let active = current == Some(instance.id.as_str());
                let (status, tone) = self.instance_status(instance);
                let mut version = format!(
                    "{} · {}",
                    instance.minecraft_version,
                    clean_platform_label(instance)
                );
                if let Some(pin) = &instance.aurora {
                    version.push_str(&format!(" · Aurora {}", pin.version));
                }
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
                                    ui::Tone::Danger => t.danger,
                                    ui::Tone::Accent => t.accent,
                                    _ => t.muted_foreground,
                                }),
                        )
                        .when(active, |row| {
                            row.child(symbols::icon(Some("check"), px(18.), t.ring))
                        })
                        .into_any_element(),
                );
            } else {
                labels.push(instance.display_name.clone().into());
            }
        }
        labels.push("Manage instances".into());
        if open {
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
        }
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
        .width(px(216.))
        .instance_switcher(rows, px(340.))
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

struct OwnedNavHint(String);
impl gpui::Render for OwnedNavHint {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        nav_hint(crate::theme::theme(cx), self.0.clone())
    }
}

fn nav_hint(t: &AuroraTheme, label: impl Into<SharedString>) -> gpui::Div {
    div()
        .px(px(10.))
        .py(px(6.))
        .rounded(t.radius)
        .border_1()
        .border_color(t.border_subtle)
        .bg(t.raised)
        .text_color(t.foreground)
        .text_size(px(ui::TEXT_BODY))
        .child(label.into())
}
