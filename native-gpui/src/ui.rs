//! Aurora layout primitives shared by every page: page and section headers,
//! cards, list rows, badges, tiles and empty states.
//!
//! Presentation only. Every helper returns a plain styled element, so callers
//! keep their own ids, handlers and semantics. Spacing follows a 4px grid
//! (4, 8, 12, 16, 20, 24, 32); type follows the scale below.
#![allow(dead_code)]

use gpui_kit::base::Button;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Div, ElementId, FontWeight, Hsla, InteractiveElement, ParentElement, SharedString, Styled, div,
    px,
};

use crate::theme::AuroraTheme;

/// Type scale (logical px).
pub const TEXT_DISPLAY: f32 = 28.;
pub const TEXT_TITLE: f32 = 24.;
pub const TEXT_SECTION: f32 = 16.;
pub const TEXT_BODY: f32 = 14.;
pub const TEXT_CAPTION: f32 = 13.;
pub const TEXT_MICRO: f32 = 12.;

/// Vertical rhythm between page blocks and inside cards.
pub const PAGE_GAP: f32 = 20.;
pub const CARD_PADDING: f32 = 20.;
/// Comfortable maximum reading width for main page content on wide displays.
pub const MAX_CONTENT_WIDTH: f32 = 1220.;

/// A persistent native scroll affordance, painted only when content overflows.
pub fn scrollbar(
    t: &AuroraTheme,
    id: impl Into<ElementId>,
    handle: &gpui_kit::ScrollHandle,
) -> gpui_kit::base::Scrollbar {
    let thumb = t.subtle_foreground.opacity(0.38);
    let active = t.muted_foreground.opacity(0.80);
    gpui_kit::base::Scrollbar::vertical(handle)
        .id(id)
        .mode(gpui_kit::base::ScrollbarMode::Hover)
        .styles(move |styles| {
            styles
                .thumb(|style| style.bg(thumb).width(px(4.)).radius(px(4.)))
                .thumb_hover(|style| style.bg(active))
                .thumb_active(|style| style.bg(active))
        })
}

/// A page title with an optional one-line description.
pub fn page_header(
    t: &AuroraTheme,
    title: impl Into<SharedString>,
    subtitle: Option<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(TEXT_TITLE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.foreground)
                .child(title.into()),
        )
        .when_some(subtitle, |this, subtitle| {
            this.child(
                div()
                    .text_size(px(TEXT_BODY))
                    .text_color(t.muted_foreground)
                    .child(subtitle),
            )
        })
}

/// A section title inside a page or card, with an optional caption beneath.
pub fn section_header(
    t: &AuroraTheme,
    title: impl Into<SharedString>,
    caption: Option<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(px(TEXT_SECTION))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.foreground)
                .child(title.into()),
        )
        .when_some(caption, |this, caption| {
            this.child(
                div()
                    .text_size(px(TEXT_CAPTION))
                    .text_color(t.muted_foreground)
                    .child(caption),
            )
        })
}

/// Accent mark beside a control whose accessible description carries the status.
pub fn update_dot(t: &AuroraTheme) -> Div {
    div().size(px(8.)).flex_none().rounded_full().bg(t.accent)
}

/// Small uppercase group label (sidebar groups, dense forms).
pub fn eyebrow(t: &AuroraTheme, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(TEXT_MICRO))
        .font_weight(FontWeight::BOLD)
        .text_color(t.muted_foreground)
        .child(text.into())
}

/// The standard content card.
pub fn card(t: &AuroraTheme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .p(px(CARD_PADDING))
        .rounded(t.radius_lg)
        .bg(t.card)
        .border_1()
        .border_color(t.border_subtle)
}

/// An Aurora-accented hero card anchored to the single active theme accent
/// with a subtle analogous glow and uniform accent border.
pub fn aurora_card(t: &AuroraTheme) -> Div {
    let h1 = t.ring.h;
    let primary = Hsla {
        h: h1,
        s: t.ring.s.clamp(0.45, 0.92),
        l: 0.58,
        a: 0.28,
    };
    let secondary = Hsla {
        h: h1,
        s: t.ring.s.clamp(0.38, 0.88),
        l: 0.46,
        a: 0.12,
    };
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .p(px(CARD_PADDING))
        .rounded(t.radius_lg)
        .bg(t.card)
        .border_1()
        .border_color(Hsla { a: 0.48, ..t.ring })
        .shadow_md()
        .relative()
        .overflow_hidden()
        .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
            122.,
            gpui_kit::linear_color_stop(primary, 0.),
            gpui_kit::linear_color_stop(secondary, 0.52),
        )))
        .child(div().absolute().inset_0().bg(gpui_kit::linear_gradient(
            302.,
            gpui_kit::linear_color_stop(Hsla { a: 0.12, ..primary }, 0.),
            gpui_kit::linear_color_stop(gpui_kit::transparent_black(), 0.48),
        )))
}

/// Unified segmented tab container used across Instance detail, Settings, Discover, and Pulsar.
pub fn segmented_tabs(t: &AuroraTheme) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(3.))
        .p(px(3.))
        .rounded(t.radius)
        .bg(t.sunken)
        .border_1()
        .border_color(t.border_subtle)
}

/// Signature Aurora Client badge anchored to the single active theme accent.
pub fn aurora_badge(t: &AuroraTheme, label: impl Into<SharedString>) -> Div {
    let h1 = t.ring.h;
    let start = Hsla {
        h: h1,
        s: t.ring.s.clamp(0.40, 0.90),
        l: 0.60,
        a: 0.16,
    };
    let end = Hsla {
        h: h1,
        s: t.ring.s.clamp(0.35, 0.86),
        l: 0.52,
        a: 0.10,
    };
    let ink = Hsla {
        h: h1,
        s: t.ring.s.clamp(0.30, 0.90),
        l: 0.86,
        a: 1.0,
    };
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(5.))
        .h(px(22.))
        .px(px(9.))
        .rounded_full()
        .bg(gpui_kit::linear_gradient(
            120.,
            gpui_kit::linear_color_stop(start, 0.),
            gpui_kit::linear_color_stop(end, 1.),
        ))
        .border_1()
        .border_color(Hsla { a: 0.44, ..t.ring })
        .text_size(px(TEXT_MICRO))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(crate::symbols::icon(Some("auto_awesome"), px(12.), ink))
        .child(label.into())
}

/// Compact Aurora version pill without a second sparkle glyph (for cards whose
/// large icon tile already carries the glowing Aurora sparkle corner emblem).
pub fn aurora_version_pill(t: &AuroraTheme, label: impl Into<SharedString>) -> Div {
    let h1 = t.ring.h;
    let start = Hsla {
        h: h1,
        s: t.ring.s.clamp(0.40, 0.90),
        l: 0.60,
        a: 0.22,
    };
    let end = Hsla {
        h: h1,
        s: t.ring.s.clamp(0.35, 0.86),
        l: 0.52,
        a: 0.12,
    };
    let ink = Hsla {
        h: h1,
        s: t.ring.s.clamp(0.30, 0.90),
        l: 0.84,
        a: 1.0,
    };
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .h(px(22.))
        .px(px(9.))
        .rounded_full()
        .bg(gpui_kit::linear_gradient(
            120.,
            gpui_kit::linear_color_stop(start, 0.),
            gpui_kit::linear_color_stop(end, 1.),
        ))
        .border_1()
        .border_color(Hsla { a: 0.38, ..t.ring })
        .text_size(px(TEXT_MICRO))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(label.into())
}

/// A settings row with a compact leading icon tile for visual rhythm.
pub fn setting_row_icon(
    t: &AuroraTheme,
    icon: &'static str,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    last: bool,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(20.))
        .py(px(14.))
        .when(!last, |this| {
            this.border_b_1().border_color(t.border_subtle)
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .flex_1()
                .min_w(px(0.))
                .child(icon_tile(t, Some(icon), 32., false))
                .child(title_block(t, title, description).flex_1().min_w(px(0.))),
        )
}

/// A card whose rows run edge to edge (lists). Put [`row`]s inside.
pub fn list_card(t: &AuroraTheme) -> Div {
    div()
        .flex()
        .flex_col()
        .rounded(t.radius_lg)
        .bg(t.card)
        .border_1()
        .border_color(t.border_subtle)
        .overflow_hidden()
}

/// One list row: leading content, flexible middle, trailing actions.
/// `last` drops the hairline under the final row.
pub fn row(t: &AuroraTheme, last: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(16.))
        .py(px(12.))
        .when(!last, |this| {
            this.border_b_1().border_color(t.border_subtle)
        })
}

/// Primary + secondary text stacked, for list rows and tiles.
pub fn title_block(
    t: &AuroraTheme,
    title: impl Into<SharedString>,
    detail: Option<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .min_w(px(0.))
        .child(
            div()
                .text_size(px(TEXT_BODY + 1.))
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.foreground)
                .text_ellipsis()
                .child(title.into()),
        )
        .when_some(detail, |this, detail| {
            this.child(
                div()
                    .text_size(px(TEXT_CAPTION))
                    .line_height(px(17.))
                    .text_color(t.muted_foreground)
                    .text_ellipsis()
                    .child(detail),
            )
        })
}

/// A hairline separator.
pub fn divider(t: &AuroraTheme) -> Div {
    div().h(px(1.)).w_full().bg(t.border_subtle)
}

pub fn muted(t: &AuroraTheme, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(TEXT_BODY))
        .text_color(t.muted_foreground)
        .child(text.into())
}

pub fn caption(t: &AuroraTheme, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(TEXT_CAPTION))
        .text_color(t.subtle_foreground)
        .child(text.into())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
}

fn tone_color(t: &AuroraTheme, tone: Tone) -> Hsla {
    match tone {
        Tone::Neutral => t.muted_foreground,
        Tone::Accent => t.ring,
        Tone::Success => t.success,
        Tone::Warning => t.warning,
        Tone::Danger => t.danger,
    }
}

/// A compact status pill.
pub fn badge(t: &AuroraTheme, label: impl Into<SharedString>, tone: Tone) -> Div {
    pill(t, label.into(), tone, false)
}

/// A badge led by a small status dot.
pub fn status_badge(t: &AuroraTheme, label: impl Into<SharedString>, tone: Tone) -> Div {
    pill(t, label.into(), tone, true)
}

fn pill(t: &AuroraTheme, label: SharedString, tone: Tone, dot: bool) -> Div {
    let color = tone_color(t, tone);
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(5.))
        .h(px(22.))
        .px(px(9.))
        .rounded_full()
        .bg(Hsla { a: 0.18, ..color })
        .text_size(px(TEXT_CAPTION))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if tone == Tone::Neutral {
            t.muted_foreground
        } else {
            color
        })
        .when(dot, |this| {
            this.child(div().size(px(6.)).rounded_full().bg(color))
        })
        .child(label)
}

/// A rounded square holding a symbol (instances, empty states, settings rows).
pub fn icon_tile(t: &AuroraTheme, icon: Option<&str>, size: f32, accent: bool) -> Div {
    let glyph = if accent { t.ring } else { t.muted_foreground };
    let h1 = t.ring.h;
    let h2 = (h1 - 0.05).rem_euclid(1.0);
    let start = Hsla {
        h: h1,
        s: t.ring.s.clamp(0.40, 0.90),
        l: 0.58,
        a: 0.24,
    };
    let end = Hsla {
        h: h2,
        s: t.ring.s.clamp(0.35, 0.86),
        l: 0.54,
        a: 0.14,
    };
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px((size * 0.26).clamp(8., 22.)))
        .when(accent, |d| {
            d.bg(gpui_kit::linear_gradient(
                135.,
                gpui_kit::linear_color_stop(start, 0.),
                gpui_kit::linear_color_stop(end, 1.),
            ))
        })
        .when(!accent, |d| d.bg(t.control))
        .border_1()
        .border_color(if accent {
            Hsla { a: 0.36, ..t.ring }
        } else {
            t.border_subtle
        })
        .child(crate::symbols::icon(icon, px(size * 0.54), glyph))
}

/// A centered placeholder for empty lists and pages.
pub fn empty_state(
    t: &AuroraTheme,
    icon: &str,
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(10.))
        .py(px(40.))
        .px(px(24.))
        .rounded(t.radius_lg)
        .bg(t.card)
        .border_1()
        .border_color(t.border_subtle)
        .child(icon_tile(t, Some(icon), 60., true))
        .child(
            div()
                .pt(px(4.))
                .text_size(px(TEXT_SECTION))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.foreground)
                .child(title.into()),
        )
        .child(
            div()
                .max_w(px(440.))
                .text_size(px(TEXT_BODY))
                .text_color(t.muted_foreground)
                .text_center()
                .child(body.into()),
        )
}

/// A labelled value (stats, metadata grids).
pub fn stat(
    t: &AuroraTheme,
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(px(TEXT_BODY))
                .text_color(t.muted_foreground)
                .child(label.into()),
        )
        .child(
            div()
                .text_size(px(TEXT_BODY + 1.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.foreground)
                .child(value.into()),
        )
}

/// A settings-style row: title and description on the left, a control on the
/// right. Add the control with `.child(...)`.
pub fn setting_row(
    t: &AuroraTheme,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    last: bool,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(24.))
        .min_h(px(62.))
        .py(px(12.))
        .when(!last, |this| {
            this.border_b_1().border_color(t.border_subtle)
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(px(TEXT_BODY + 1.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.foreground)
                        .child(title.into()),
                )
                // Descriptions wrap: settings copy must never be truncated.
                .when_some(description, |this, description| {
                    this.child(
                        div()
                            .text_size(px(TEXT_CAPTION))
                            .text_color(t.muted_foreground)
                            .child(description),
                    )
                }),
        )
}

/// A selectable full-width list row (pickers inside a card). Selection is the
/// app-wide accent wash; no border, so lists stay quiet.
pub fn list_button(t: &AuroraTheme, id: impl Into<ElementId>, selected: bool) -> Button {
    let hover = t.control;
    let ring = t.ring;
    Button::new(id)
        .selected(selected)
        .flex()
        .items_center()
        .gap(px(10.))
        .w_full()
        .px(px(8.))
        .py(px(5.))
        .rounded(t.radius)
        .when(selected, |b| b.bg(t.accent_soft))
        .hover(move |style| style.bg(hover))
        .border_1()
        .border_color(gpui_kit::transparent_black())
        .focus(move |style| style.border_color(ring))
}

/// A selectable card-like button (instance pickers, choice grids). The base
/// [`Button`] keeps role, focus and keyboard activation; add children freely.
pub fn tile_button(t: &AuroraTheme, id: impl Into<ElementId>, selected: bool) -> Button {
    let ring = t.ring;
    let hover = t.control_hover;
    Button::new(id)
        .selected(selected)
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .rounded(t.radius_lg)
        .bg(if selected { t.accent_soft } else { t.card })
        .border_1()
        .border_color(if selected {
            Hsla { a: 0.55, ..ring }
        } else {
            t.border_subtle
        })
        .hover(move |style| style.bg(hover))
        .focus(move |style| style.border_color(ring))
}
