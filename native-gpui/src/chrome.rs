//! Client-drawn window chrome, ported from the supported host
//! (`native/src/chrome.rs`, `native/ui/chrome.slint`).
//!
//! On Windows the native caption is hidden (`appears_transparent`), which
//! keeps the OS resize borders, drop shadow, Windows 11 rounded corners and
//! snap layouts. The platform layer already answers `WM_NCHITTEST` from the
//! window-control areas declared here: the drag strip is `HTCAPTION` (OS
//! window move, Aero Snap and double-click maximize/restore come from
//! `DefWindowProc`), and the caption buttons are `HTMINBUTTON`/`HTMAXBUTTON`/
//! `HTCLOSE` (so the Windows 11 snap flyout appears on the maximize button and
//! the click itself is performed by the platform). The buttons therefore carry
//! no click handler; assistive technology activates them through the
//! accessibility action instead. Each button is also a keyboard tab stop (in
//! tree order, after the sidebar and before the page content, like Slint's
//! `TitleBar`) with a focus ring and Enter/Space activation. Other platforms
//! keep their native frame (`custom()` is false), like the Slint host.
//!
//! The strip, glyphs, sizes and colours follow `chrome.slint`: a 44 px strip
//! above the page content holding only the three 52 px caption buttons, with
//! glyphs snapped to whole physical pixels so their one-pixel lines stay crisp
//! at every display scale. The sidebar header is a second drag region (the
//! Slint logo area).

use crate::theme::AuroraTheme;
use gpui::{
    AccessibleAction, App, FocusHandle, Hsla, InteractiveElement, IntoElement, KeyDownEvent,
    Modifiers, ParentElement, PathBuilder, Pixels, Role, SharedString, Size,
    StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowControlArea, canvas, div,
    point, px, rgba, size,
};

/// Height of the caption strip (`Chrome.title-height`).
pub const TITLE_HEIGHT: f32 = 44.0;
/// Width of one caption button.
const BUTTON_WIDTH: f32 = 52.0;
/// The hover/pressed highlight sits inside the button (x 4, y 6).
const HIGHLIGHT_INSET_X: f32 = 4.0;
const HIGHLIGHT_INSET_Y: f32 = 6.0;
/// Edge of the square the glyphs are drawn in.
const GLYPH_BOX: f32 = 10.0;
/// Overlap of the two squares of the restore glyph.
const GLYPH_INSET: f32 = 2.0;

/// The window's default and minimum size (`preferred-*` / `min-*` in
/// `native/ui/app.slint`).
const DEFAULT_SIZE: (f32, f32) = (1120.0, 760.0);
const MIN_SIZE: (f32, f32) = (720.0, 520.0);

pub fn default_size() -> Size<Pixels> {
    size(px(DEFAULT_SIZE.0), px(DEFAULT_SIZE.1))
}

pub fn min_size() -> Size<Pixels> {
    size(px(MIN_SIZE.0), px(MIN_SIZE.1))
}

/// Whether the launcher draws its own caption (Windows only, as in Slint).
pub const fn custom() -> bool {
    cfg!(windows)
}

/// The window title still names the window for the taskbar and assistive
/// technology; the caption is hidden only where the chrome is custom.
pub fn titlebar() -> TitlebarOptions {
    TitlebarOptions {
        title: Some("Aurora Launcher".into()),
        appears_transparent: custom(),
        ..Default::default()
    }
}

/// Makes `element` move the window when dragged (and maximize/restore it on a
/// double-click) wherever the chrome is custom. It must not be an ancestor of
/// the caption buttons: the platform takes the first control area under the
/// pointer in paint order, so the parent would win.
pub fn drag_region<E: InteractiveElement>(element: E) -> E {
    if custom() {
        element.window_control_area(WindowControlArea::Drag)
    } else {
        element
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Caption {
    Minimize,
    Maximize,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Glyph {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Caption {
    const ALL: [Caption; 3] = [Caption::Minimize, Caption::Maximize, Caption::Close];

    fn id(self) -> &'static str {
        match self {
            Self::Minimize => "caption-minimize",
            Self::Maximize => "caption-maximize",
            Self::Close => "caption-close",
        }
    }

    /// Accessible name; the maximize button becomes "Restore" while maximized.
    fn label(self, maximized: bool) -> &'static str {
        match self {
            Self::Minimize => "Minimize",
            Self::Maximize if maximized => "Restore",
            Self::Maximize => "Maximize",
            Self::Close => "Close",
        }
    }

    fn glyph(self, maximized: bool) -> Glyph {
        match self {
            Self::Minimize => Glyph::Minimize,
            Self::Maximize if maximized => Glyph::Restore,
            Self::Maximize => Glyph::Maximize,
            Self::Close => Glyph::Close,
        }
    }

    fn area(self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Maximize => WindowControlArea::Max,
            Self::Close => WindowControlArea::Close,
        }
    }

    fn command(self) -> Command {
        match self {
            Self::Minimize => Command::Minimize,
            Self::Maximize => Command::ToggleMaximize,
            Self::Close => Command::Close,
        }
    }

    /// Keyboard and assistive-technology activation. Pointer clicks never
    /// reach here: the platform performs those from the hit-test result.
    fn activate(self, window: &mut Window) {
        match self.command() {
            Command::Minimize => window.minimize_window(),
            Command::ToggleMaximize => window.zoom_window(),
            // The launcher registers no should-close hook, so removing the
            // window is the same teardown as the system close button (the
            // platform's WM_CLOSE default also just destroys the window), and
            // the app quits when its last window is gone.
            Command::Close => window.remove_window(),
        }
    }
}

/// What a caption button asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Minimize,
    ToggleMaximize,
    Close,
}

/// `chrome.slint` activates a focused caption button on Space or Return. Held
/// key repeats are ignored so a held Enter cannot flip maximize back and
/// forth; shortcut chords (Ctrl/Alt/Win) belong to someone else.
fn key_activates(key: &str, modifiers: &Modifiers, held: bool) -> bool {
    matches!(key, "enter" | "space")
        && !held
        && !(modifiers.control || modifiers.alt || modifiers.platform || modifiers.function)
}

/// Glyph geometry in logical pixels, every value a whole number of physical
/// pixels at the window's scale factor (Slint's `round(x / 1phx) * 1phx`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Metrics {
    line: f32,
    box_size: f32,
    inset: f32,
    left: f32,
    top: f32,
    mid: f32,
}

impl Metrics {
    fn new(scale: f32) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let snap = |logical: f32| (logical * scale).round() / scale;
        let box_size = snap(GLYPH_BOX);
        Self {
            line: scale.round().max(1.0) / scale,
            box_size,
            inset: snap(GLYPH_INSET),
            left: snap((BUTTON_WIDTH - box_size) / 2.0),
            top: snap((TITLE_HEIGHT - box_size) / 2.0),
            mid: snap(box_size / 2.0),
        }
    }
}

/// The caption strip above the page content: a drag region that fills the
/// space and the three caption buttons at the right edge. Rendered only where
/// `custom()`.
pub fn title_bar(window: &mut Window, cx: &mut App, palette: &AuroraTheme) -> impl IntoElement {
    let maximized = window.is_maximized();
    let metrics = Metrics::new(window.scale_factor());
    // One persistent tab stop per button, kept across frames.
    let buttons = Caption::ALL.map(|caption| {
        let focus = window
            .use_keyed_state(
                SharedString::from(format!("{}-focus", caption.id())),
                cx,
                |_, cx| cx.focus_handle().tab_stop(true),
            )
            .read(cx)
            .clone();
        caption_button(caption, maximized, metrics, palette, &focus)
    });
    div()
        .flex()
        .flex_shrink_0()
        .w_full()
        .h(px(TITLE_HEIGHT))
        .child(drag_region(div().flex_1().h_full()))
        .children(buttons)
}

/// Ink on a coloured fill that changes with the group's hover.
fn fill(part: gpui::Div, group: &SharedString, ink: Hsla, hover_ink: Hsla) -> gpui::Div {
    part.absolute()
        .bg(ink)
        .group_hover(group.clone(), move |style| style.bg(hover_ink))
}

/// Ink on a one-device-pixel outline.
fn outline(
    part: gpui::Div,
    group: &SharedString,
    line: f32,
    ink: Hsla,
    hover_ink: Hsla,
) -> gpui::Div {
    part.absolute()
        .border(px(line))
        .border_color(ink)
        .group_hover(group.clone(), move |style| style.border_color(hover_ink))
}

fn close_glyph(metrics: Metrics, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let (x, y, edge) = (bounds.origin.x, bounds.origin.y, bounds.size.width);
            let mut builder = PathBuilder::stroke(px(metrics.line));
            builder.move_to(point(x, y));
            builder.line_to(point(x + edge, y + edge));
            builder.move_to(point(x + edge, y));
            builder.line_to(point(x, y + edge));
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        },
    )
    .size_full()
}

fn caption_button(
    caption: Caption,
    maximized: bool,
    metrics: Metrics,
    palette: &AuroraTheme,
    focus: &FocusHandle,
) -> impl IntoElement + use<> {
    let danger = caption == Caption::Close;
    let group = SharedString::from(caption.id());
    // Slint: `text-secondary` ink; accent (error for Close) on hover; a soft
    // fill on hover and a softer-tinted one while pressed.
    let ink = palette.muted_foreground;
    let ring = palette.ring;
    let hover_ink = if danger { palette.danger } else { ring };
    let hover_fill = if danger {
        Hsla::from(rgba(0xc9585824))
    } else {
        palette.surface_hover
    };
    let pressed_fill = if danger {
        Hsla::from(rgba(0xc9585824))
    } else {
        Hsla {
            a: 0.14,
            ..palette.ring
        }
    };
    let Metrics {
        line,
        box_size,
        inset,
        left,
        top,
        mid,
    } = metrics;

    let mut button = div()
        .id(caption.id())
        .group(group.clone())
        .relative()
        .flex_shrink_0()
        .w(px(BUTTON_WIDTH))
        .h_full()
        .window_control_area(caption.area())
        .child(
            div()
                // Only a stateful element takes the group's pressed style.
                .id(SharedString::from(format!("{}-highlight", caption.id())))
                // The highlight rectangle is also the focus target and the
                // accessible button, so the ring hugs the same inset box as
                // Slint's `FocusRing`. The parent keeps only the platform
                // control area: the platform performs pointer clicks.
                .track_focus(focus)
                .role(Role::Button)
                .aria_label(caption.label(maximized))
                .on_a11y_action(AccessibleAction::Click, move |_, window, _| {
                    caption.activate(window);
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if key_activates(
                        event.keystroke.key.as_str(),
                        &event.keystroke.modifiers,
                        event.is_held,
                    ) {
                        cx.stop_propagation();
                        caption.activate(window);
                    }
                })
                .absolute()
                .left(px(HIGHLIGHT_INSET_X))
                .top(px(HIGHLIGHT_INSET_Y))
                .w(px(BUTTON_WIDTH - 2.0 * HIGHLIGHT_INSET_X))
                .h(px(TITLE_HEIGHT - 2.0 * HIGHLIGHT_INSET_Y))
                .rounded(palette.radius)
                .group_hover(group.clone(), move |style| style.bg(hover_fill))
                .group_active(group.clone(), move |style| style.bg(pressed_fill))
                .focus_visible(move |style| style.border_2().border_color(ring)),
        );
    button = match caption.glyph(maximized) {
        Glyph::Minimize => button.child(
            fill(div(), &group, ink, hover_ink)
                .left(px(left))
                .top(px(top + mid))
                .w(px(box_size))
                .h(px(line)),
        ),
        Glyph::Maximize => button.child(
            outline(div(), &group, line, ink, hover_ink)
                .left(px(left))
                .top(px(top))
                .w(px(box_size))
                .h(px(box_size)),
        ),
        Glyph::Restore => button
            .child(
                outline(div(), &group, line, ink, hover_ink)
                    .left(px(left))
                    .top(px(top + inset))
                    .w(px(box_size - inset))
                    .h(px(box_size - inset)),
            )
            .child(
                fill(div(), &group, ink, hover_ink)
                    .left(px(left + inset))
                    .top(px(top))
                    .w(px(box_size - inset))
                    .h(px(line)),
            )
            .child(
                fill(div(), &group, ink, hover_ink)
                    .left(px(left + box_size - line))
                    .top(px(top))
                    .w(px(line))
                    .h(px(box_size - inset)),
            ),
        // A canvas picks its colour at paint time, so hover swaps between two
        // stacked strokes instead.
        Glyph::Close => button
            .child(
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(box_size))
                    .h(px(box_size))
                    .group_hover(group.clone(), |style| style.invisible())
                    .child(close_glyph(metrics, ink)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(box_size))
                    .h(px(box_size))
                    .invisible()
                    .group_hover(group.clone(), |style| style.visible())
                    .child(close_glyph(metrics, hover_ink)),
            ),
    };
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximize_button_names_and_draws_the_action_it_performs() {
        assert_eq!(Caption::Maximize.label(false), "Maximize");
        assert_eq!(Caption::Maximize.glyph(false), Glyph::Maximize);
        assert_eq!(Caption::Maximize.label(true), "Restore");
        assert_eq!(Caption::Maximize.glyph(true), Glyph::Restore);
        for maximized in [false, true] {
            assert_eq!(Caption::Minimize.label(maximized), "Minimize");
            assert_eq!(Caption::Minimize.glyph(maximized), Glyph::Minimize);
            assert_eq!(Caption::Close.label(maximized), "Close");
            assert_eq!(Caption::Close.glyph(maximized), Glyph::Close);
        }
    }

    #[test]
    fn caption_keys_invoke_minimize_zoom_and_close() {
        let none = Modifiers::default();
        let shift = Modifiers {
            shift: true,
            ..Default::default()
        };
        for key in ["enter", "space"] {
            assert!(key_activates(key, &none, false), "{key}");
            assert!(key_activates(key, &shift, false), "{key} with shift");
        }
        // Held repeats, other keys and shortcut chords never activate.
        assert!(!key_activates("enter", &none, true));
        for key in ["tab", "escape", "a", "left"] {
            assert!(!key_activates(key, &none, false), "{key}");
        }
        for chord in [
            Modifiers::control(),
            Modifiers::alt(),
            Modifiers::command(),
            Modifiers::function(),
        ] {
            assert!(!key_activates("space", &chord, false), "{chord:?}");
        }
        assert_eq!(Caption::Minimize.command(), Command::Minimize);
        assert_eq!(Caption::Maximize.command(), Command::ToggleMaximize);
        assert_eq!(Caption::Close.command(), Command::Close);
    }

    #[test]
    fn caption_buttons_map_to_the_platform_control_areas() {
        assert_eq!(Caption::Minimize.area(), WindowControlArea::Min);
        assert_eq!(Caption::Maximize.area(), WindowControlArea::Max);
        assert_eq!(Caption::Close.area(), WindowControlArea::Close);
        let ids: Vec<_> = Caption::ALL.iter().map(|caption| caption.id()).collect();
        assert_eq!(
            ids,
            ["caption-minimize", "caption-maximize", "caption-close"]
        );
    }

    #[test]
    fn glyph_geometry_lands_on_whole_physical_pixels_at_every_scale() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0] {
            let m = Metrics::new(scale);
            for (name, value) in [
                ("line", m.line),
                ("box", m.box_size),
                ("inset", m.inset),
                ("left", m.left),
                ("top", m.top),
                ("mid", m.mid),
            ] {
                let device = value * scale;
                assert!(
                    (device - device.round()).abs() < 1e-3,
                    "{name} = {value} is {device} device px at {scale}x"
                );
            }
            assert!(m.line * scale >= 1.0, "a line is at least one device pixel");
            assert!(m.left >= 0.0 && m.left + m.box_size <= BUTTON_WIDTH);
            assert!(m.top >= 0.0 && m.top + m.box_size <= TITLE_HEIGHT);
        }
    }

    #[test]
    fn an_unusable_scale_factor_falls_back_to_one() {
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(Metrics::new(scale), Metrics::new(1.0));
        }
        let m = Metrics::new(1.0);
        assert_eq!((m.line, m.box_size, m.inset), (1.0, 10.0, 2.0));
        assert_eq!((m.left, m.top, m.mid), (21.0, 17.0, 5.0));
    }

    #[test]
    fn window_sizes_match_the_slint_host() {
        assert_eq!(DEFAULT_SIZE, (1120.0, 760.0));
        assert_eq!(MIN_SIZE, (720.0, 520.0));
        assert_eq!(min_size(), size(px(720.), px(520.)));
        assert_eq!(default_size(), size(px(1120.), px(760.)));
    }

    #[test]
    fn caption_is_hidden_exactly_where_the_chrome_is_custom() {
        let options = titlebar();
        assert_eq!(options.appears_transparent, custom());
        assert_eq!(options.title.as_deref(), Some("Aurora Launcher"));
        assert_eq!(custom(), cfg!(windows));
    }
}
