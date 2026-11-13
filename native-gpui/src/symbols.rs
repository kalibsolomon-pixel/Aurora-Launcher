//! Instance icons: Google Material Symbols drawn straight from their 24x24 SVG
//! path data.
//!
//! The vocabulary, search ranking, labels and fallback are the supported
//! host's own (`native/src/symbols.rs` over `native/src/symbols_data.rs`), so
//! both hosts agree on what an icon name means. This module adds the GPUI
//! side: a painter that tints a symbol with a theme colour, and the picker
//! wording. No image asset is registered or rasterised ahead of time, and the
//! picker only ever receives the bounded search result, never the whole table.
#![allow(dead_code)]

use gpui::{
    FillOptions, Hsla, IntoElement, PathBuilder, PathStyle, Pixels, Point, SharedString, Styled,
    canvas, point, px,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

#[path = "../../native/src/symbols.rs"]
mod catalog;

#[allow(unused_imports)]
pub use catalog::{
    CATEGORIES, DEFAULT_ICONS, FALLBACK_ICON, RESULT_LIMIT, Symbol, label, path, path_or_fallback,
    search,
};

/// The symbols are authored on a 24x24 grid.
const VIEWBOX: f32 = 24.0;

/// Draws an instance's icon, or the fallback glyph when `name` is unset or no
/// longer a known symbol. `color` is the tint (the theme foreground or accent).
pub fn icon(name: Option<&str>, size: Pixels, color: Hsla) -> impl IntoElement {
    let data = path_or_fallback(name);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let Some(segments) = segments(data) else {
                return;
            };
            let scale = f32::from(bounds.size.width).min(f32::from(bounds.size.height)) / VIEWBOX;
            let origin = point(
                bounds.origin.x + px((f32::from(bounds.size.width) - VIEWBOX * scale) / 2.0),
                bounds.origin.y + px((f32::from(bounds.size.height) - VIEWBOX * scale) / 2.0),
            );
            if let Some(path) = outline(&segments, origin, scale) {
                window.paint_path(path, color);
            }
        },
    )
    .w(size)
    .h(size)
    .flex_shrink_0()
}

/// "Rocket launch" for the icon shown for `name` (the fallback when unset).
pub fn caption(name: Option<&str>) -> String {
    label(name.unwrap_or(FALLBACK_ICON))
}

/// Picker category choices: "All categories", then each category with its `&`
/// spaced out ("Audio & Video").
pub fn category_labels() -> Vec<SharedString> {
    let mut labels: Vec<SharedString> = vec!["All categories".into()];
    labels.extend(CATEGORIES.iter().map(|c| c.replace('&', " & ").into()));
    labels
}

/// The core category filter for a category choice (`0` is every category).
pub fn category_filter(choice: usize) -> Option<usize> {
    choice.checked_sub(1)
}

/// The bounded result set behind the picker grid.
pub fn browse(query: &str, category_choice: usize) -> Vec<Symbol> {
    search(query, category_filter(category_choice), RESULT_LIMIT)
}

/// The line under the search box.
pub fn summary(count: usize, query: &str) -> String {
    match (count, query.trim().is_empty()) {
        (0, _) => "No icons match. Try another word, such as sword, server or tools.".to_owned(),
        (n, true) if n >= RESULT_LIMIT => "Suggested icons. Search to find more.".to_owned(),
        (n, false) if n >= RESULT_LIMIT => {
            format!("First {n} matches. Add a word to narrow them.")
        }
        (1, _) => "1 icon".to_owned(),
        (n, _) => format!("{n} icons"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Segment {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

type Parsed = Option<Rc<[Segment]>>;

thread_local! {
    /// Parsed outlines by (address, length) of the static path string; at most
    /// one entry per bundled symbol, filled as icons are first drawn.
    static PARSED: RefCell<HashMap<(usize, usize), Parsed>> = RefCell::new(HashMap::new());
}

fn segments(data: &'static str) -> Parsed {
    let key = (data.as_ptr() as usize, data.len());
    PARSED.with(|cache| {
        cache
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| parse(data).map(Rc::from))
            .clone()
    })
}

/// Parses SVG path data: absolute and relative move, line, horizontal and
/// vertical line, quadratic and cubic curves, and close. The bundled symbols
/// use only absolute M, L, H, V, Q and Z; anything else is rejected rather
/// than half drawn.
fn parse(data: &str) -> Option<Vec<Segment>> {
    let bytes = data.as_bytes();
    let mut at = 0;
    let mut out = Vec::new();
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let (mut start_x, mut start_y) = (0.0f32, 0.0f32);
    let mut command: Option<u8> = None;
    loop {
        while at < bytes.len() && (bytes[at].is_ascii_whitespace() || bytes[at] == b',') {
            at += 1;
        }
        if at >= bytes.len() {
            return Some(out);
        }
        if bytes[at].is_ascii_alphabetic() {
            command = Some(bytes[at]);
            at += 1;
            if matches!(command, Some(b'Z' | b'z')) {
                out.push(Segment::Close);
                (x, y) = (start_x, start_y);
                command = None;
                continue;
            }
        }
        let current = command?;
        let relative = current.is_ascii_lowercase();
        let (base_x, base_y) = if relative { (x, y) } else { (0.0, 0.0) };
        let mut args = [0.0f32; 6];
        let count = match current.to_ascii_uppercase() {
            b'M' | b'L' => 2,
            b'H' | b'V' => 1,
            b'Q' => 4,
            b'C' => 6,
            _ => return None,
        };
        for slot in args.iter_mut().take(count) {
            *slot = number(bytes, &mut at)?;
        }
        match current.to_ascii_uppercase() {
            b'M' => {
                (x, y) = (base_x + args[0], base_y + args[1]);
                (start_x, start_y) = (x, y);
                out.push(Segment::Move(x, y));
                // Further coordinate pairs after a move are implicit lines.
                command = Some(if relative { b'l' } else { b'L' });
            }
            b'L' => {
                (x, y) = (base_x + args[0], base_y + args[1]);
                out.push(Segment::Line(x, y));
            }
            b'H' => {
                x = base_x + args[0];
                out.push(Segment::Line(x, y));
            }
            b'V' => {
                y = base_y + args[0];
                out.push(Segment::Line(x, y));
            }
            b'Q' => {
                out.push(Segment::Quad(
                    base_x + args[0],
                    base_y + args[1],
                    base_x + args[2],
                    base_y + args[3],
                ));
                (x, y) = (base_x + args[2], base_y + args[3]);
            }
            _ => {
                out.push(Segment::Cubic(
                    base_x + args[0],
                    base_y + args[1],
                    base_x + args[2],
                    base_y + args[3],
                    base_x + args[4],
                    base_y + args[5],
                ));
                (x, y) = (base_x + args[4], base_y + args[5]);
            }
        }
    }
}

/// Reads one number, separators first. A second `.` or a sign begins the next
/// number, so "1.5.3" and "4-2" are two numbers each.
fn number(bytes: &[u8], at: &mut usize) -> Option<f32> {
    while *at < bytes.len() && (bytes[*at].is_ascii_whitespace() || bytes[*at] == b',') {
        *at += 1;
    }
    let begin = *at;
    if matches!(bytes.get(*at), Some(b'+' | b'-')) {
        *at += 1;
    }
    let mut digits = false;
    while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
        *at += 1;
        digits = true;
    }
    if bytes.get(*at) == Some(&b'.') {
        *at += 1;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
            digits = true;
        }
    }
    if !digits {
        return None;
    }
    if matches!(bytes.get(*at), Some(b'e' | b'E')) {
        let mut probe = *at + 1;
        if matches!(bytes.get(probe), Some(b'+' | b'-')) {
            probe += 1;
        }
        if bytes.get(probe).is_some_and(u8::is_ascii_digit) {
            while bytes.get(probe).is_some_and(u8::is_ascii_digit) {
                probe += 1;
            }
            *at = probe;
        }
    }
    std::str::from_utf8(&bytes[begin..*at]).ok()?.parse().ok()
}

fn outline(segments: &[Segment], origin: Point<Pixels>, scale: f32) -> Option<gpui::Path<Pixels>> {
    let at = |x: f32, y: f32| point(origin.x + px(x * scale), origin.y + px(y * scale));
    // Non-zero winding, as every SVG renderer fills glyph outlines.
    let mut builder = PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::non_zero()));
    for segment in segments {
        match *segment {
            Segment::Move(x, y) => builder.move_to(at(x, y)),
            Segment::Line(x, y) => builder.line_to(at(x, y)),
            Segment::Quad(cx, cy, x, y) => builder.curve_to(at(x, y), at(cx, cy)),
            Segment::Cubic(ax, ay, bx, by, x, y) => {
                builder.cubic_bezier_to(at(x, y), at(ax, ay), at(bx, by))
            }
            Segment::Close => builder.close(),
        }
    }
    builder.build().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_and_unknown_icons_use_the_fallback_glyph_and_caption() {
        let fallback = path(FALLBACK_ICON).unwrap();
        assert_eq!(path_or_fallback(None), fallback);
        assert_eq!(path_or_fallback(Some("no_such_symbol")), fallback);
        assert_eq!(path_or_fallback(Some("swords")), path("swords").unwrap());
        assert_ne!(path("swords"), path(FALLBACK_ICON));
        assert_eq!(caption(None), "Deployed code");
        assert_eq!(caption(Some("rocket_launch")), "Rocket launch");
    }

    #[test]
    fn every_bundled_symbol_parses_inside_the_view_box() {
        let all = search("", None, 8000);
        assert!(all.len() > 3000, "{}", all.len());
        for symbol in all {
            let parsed = parse(symbol.path).unwrap_or_else(|| panic!("{}", symbol.name));
            assert!(
                matches!(parsed.first(), Some(Segment::Move(..)))
                    && matches!(parsed.last(), Some(Segment::Close)),
                "{}",
                symbol.name
            );
            for segment in parsed {
                let points: Vec<f32> = match segment {
                    Segment::Move(x, y) | Segment::Line(x, y) => vec![x, y],
                    Segment::Quad(a, b, c, d) => vec![a, b, c, d],
                    Segment::Cubic(a, b, c, d, e, f) => vec![a, b, c, d, e, f],
                    Segment::Close => vec![],
                };
                assert!(
                    points.iter().all(|v| (-1.0..=25.0).contains(v)),
                    "{}",
                    symbol.name
                );
            }
        }
    }

    #[test]
    fn path_data_forms_are_read_exactly_or_rejected() {
        assert_eq!(
            parse("M1 2L3 4H5V6Z").unwrap(),
            [
                Segment::Move(1.0, 2.0),
                Segment::Line(3.0, 4.0),
                Segment::Line(5.0, 4.0),
                Segment::Line(5.0, 6.0),
                Segment::Close,
            ]
        );
        // Relative commands, implicit repeats, packed numbers.
        assert_eq!(
            parse("m1.5.5 2 3-1 4z").unwrap(),
            [
                Segment::Move(1.5, 0.5),
                Segment::Line(3.5, 3.5),
                Segment::Line(2.5, 7.5),
                Segment::Close,
            ]
        );
        assert_eq!(
            parse("M0 0Q1 1 2 0c1 1 2 1 3 0").unwrap(),
            [
                Segment::Move(0.0, 0.0),
                Segment::Quad(1.0, 1.0, 2.0, 0.0),
                Segment::Cubic(3.0, 1.0, 4.0, 1.0, 5.0, 0.0),
            ]
        );
        // Arcs are not in the bundled vocabulary: never draw half an icon.
        assert!(parse("M0 0A1 1 0 0 1 2 2").is_none());
        assert!(parse("1 1").is_none());
        assert!(parse("M1").is_none());
    }

    #[test]
    fn picker_wording_and_categories() {
        assert_eq!(
            summary(0, "zzz"),
            "No icons match. Try another word, such as sword, server or tools."
        );
        assert_eq!(
            summary(RESULT_LIMIT, "  "),
            "Suggested icons. Search to find more."
        );
        assert_eq!(
            summary(RESULT_LIMIT, "sw"),
            format!("First {RESULT_LIMIT} matches. Add a word to narrow them.")
        );
        assert_eq!(summary(1, "sword"), "1 icon");
        assert_eq!(summary(7, ""), "7 icons");
        let labels = category_labels();
        assert_eq!(labels.len(), CATEGORIES.len() + 1);
        assert_eq!(labels[0].as_ref(), "All categories");
        assert!(labels.iter().any(|l| l.as_ref() == "Audio & Video"));
        assert_eq!(category_filter(0), None);
        assert_eq!(category_filter(3), Some(2));
    }

    #[test]
    fn picker_search_is_filtered_ranked_and_bounded() {
        let first = |query: &str, category: usize| -> Vec<&'static str> {
            browse(query, category)
                .into_iter()
                .map(|s| s.name)
                .collect()
        };
        assert_eq!(first("sword", 0)[0], "swords");
        assert!(first("server", 0).contains(&"dns"));
        assert!(first("zzzz-no-such-icon", 0).is_empty());
        assert_eq!(first("", 0).len(), RESULT_LIMIT);
        assert_eq!(first("", 0)[0], DEFAULT_ICONS[0]);
        assert!(first("a", 0).len() <= RESULT_LIMIT);
        // A category choice narrows to that category only.
        let maps = CATEGORIES.iter().position(|c| *c == "Maps").unwrap() + 1;
        let in_maps = first("", maps);
        assert!(!in_maps.is_empty() && in_maps.len() <= RESULT_LIMIT);
        assert_ne!(in_maps, first("", 0));
    }
}
