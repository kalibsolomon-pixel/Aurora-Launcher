//! Conversion of a Minecraft status `description` component into a safe,
//! launcher-owned presentation representation.
//!
//! Server MOTDs are hostile input: a remote server fully controls the JSON.
//! This module never passes server JSON through; it extracts only bounded
//! text with strictly validated styling into plain Rust types. Control
//! characters, bidi/format characters, overlong content, deep component
//! trees and unknown colors are dropped or truncated — never forwarded.

/// Presentation text bounds: restrained on purpose. Vanilla MOTDs are one or
/// two lines; anything longer is clamped, not rendered in full.
pub const MAX_LINES: usize = 3;
pub const MAX_LINE_CHARS: usize = 128;
const MAX_COMPONENT_DEPTH: usize = 8;
const MAX_EXTRAS_PER_NODE: usize = 32;
const MAX_TOTAL_SEGMENTS: usize = 64;

/// One styled run of MOTD text. Colors are validated CSS hex strings (or
/// `None` to inherit); obfuscated and strikethrough are deliberately not
/// representable.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotdSegment {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
}

/// The named colors of Minecraft's formatting vocabulary, as CSS hex.
fn named_color(name: &str) -> Option<&'static str> {
    Some(match name {
        "black" => "#000000",
        "dark_blue" => "#0000AA",
        "dark_green" => "#00AA00",
        "dark_aqua" => "#00AAAA",
        "dark_red" => "#AA0000",
        "dark_purple" => "#AA00AA",
        "gold" => "#FFAA00",
        "gray" => "#555555",
        "dark_gray" => "#AAAAAA",
        "blue" => "#5555FF",
        "green" => "#55FF55",
        "aqua" => "#55FFFF",
        "red" => "#FF5555",
        "light_purple" => "#FF55FF",
        "yellow" => "#FFFF55",
        "white" => "#FFFFFF",
        _ => return None,
    })
}

fn valid_color(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Some(format!("#{}", hex.to_ascii_lowercase()));
        }
        return None;
    }
    named_color(value).map(str::to_owned)
}

/// Accumulator style while flattening a component tree.
#[derive(Clone, Default)]
struct Style {
    color: Option<String>,
    bold: bool,
    italic: bool,
    underline: bool,
}

struct Builder {
    lines: Vec<Vec<MotdSegment>>,
    segments: usize,
}

impl Builder {
    fn new() -> Self {
        Self {
            lines: vec![Vec::new()],
            segments: 0,
        }
    }
    /// Appends presentable text under one style, splitting on newlines and
    /// clamping each line to the presentation width.
    fn push(&mut self, text: &str, style: &Style) {
        for (index, part) in text.split('\n').enumerate() {
            if index > 0 {
                if self.lines.len() >= MAX_LINES {
                    return;
                }
                self.lines.push(Vec::new());
            }
            let used: usize = self
                .lines
                .last()
                .map(|line| {
                    line.iter()
                        .map(|segment| segment.text.chars().count())
                        .sum()
                })
                .unwrap_or(0);
            let remaining = MAX_LINE_CHARS.saturating_sub(used);
            if remaining == 0 {
                continue;
            }
            let filtered: String = part
                .chars()
                .filter(|c| crate::launch::activity_bridge::is_presentable_char(*c))
                .take(remaining)
                .collect();
            if filtered.is_empty() {
                continue;
            }
            if self.segments >= MAX_TOTAL_SEGMENTS {
                return;
            }
            if let Some(line) = self.lines.last_mut() {
                line.push(MotdSegment {
                    text: filtered,
                    color: style.color.clone(),
                    bold: style.bold,
                    italic: style.italic,
                    underline: style.underline,
                });
                self.segments += 1;
            }
        }
    }
}

/// Applies legacy `§x` formatting codes inside raw text, producing one segment
/// per style transition. Unknown codes are dropped with the marker itself.
fn push_legacy(builder: &mut Builder, text: &str, inherited: &Style) {
    let mut style = inherited.clone();
    let mut run = String::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '§'
            && let Some(&code) = characters.peek()
        {
            characters.next();
            if !run.is_empty() {
                builder.push(&run, &style);
                run.clear();
            }
            match code.to_ascii_lowercase() {
                '0'..='9' | 'a'..='f' => {
                    style.color = legacy_color(code.to_ascii_lowercase());
                }
                'l' => style.bold = true,
                'o' => style.italic = true,
                'n' => style.underline = true,
                'r' => style = Style::default(),
                _ => {}
            }
            continue;
        }
        run.push(character);
    }
    if !run.is_empty() {
        builder.push(&run, &style);
    }
}

fn legacy_color(code: char) -> Option<String> {
    let index = code.to_digit(16).unwrap_or_default() as usize;
    named_color(
        [
            "black",
            "dark_blue",
            "dark_green",
            "dark_aqua",
            "dark_red",
            "dark_purple",
            "gold",
            "gray",
            "dark_gray",
            "blue",
            "green",
            "aqua",
            "red",
            "light_purple",
            "yellow",
            "white",
        ][index],
    )
    .map(str::to_owned)
}

/// Flattens one chat-component JSON value (string, object or array — the
/// shapes real MOTDs use) into bounded presentation lines.
pub fn convert(description: &serde_json::Value) -> Vec<Vec<MotdSegment>> {
    let mut builder = Builder::new();
    flatten(description, &Style::default(), &mut builder, 0);
    builder
        .lines
        .into_iter()
        .filter(|line| !line.is_empty())
        .take(MAX_LINES)
        .collect()
}

fn flatten(value: &serde_json::Value, style: &Style, builder: &mut Builder, depth: usize) {
    if depth > MAX_COMPONENT_DEPTH || builder.segments >= MAX_TOTAL_SEGMENTS {
        return;
    }
    match value {
        serde_json::Value::String(text) => push_legacy(builder, text, style),
        serde_json::Value::Array(items) => {
            for item in items.iter().take(MAX_EXTRAS_PER_NODE) {
                flatten(item, style, builder, depth + 1);
            }
        }
        serde_json::Value::Object(fields) => {
            let mut child = style.clone();
            if let Some(color) = fields.get("color").and_then(|value| value.as_str()) {
                // An unrecognizable color is dropped rather than forwarded;
                // an absent color inherits the parent style.
                child.color = valid_color(color).or_else(|| style.color.clone());
            }
            for (key, flag) in [("bold", &mut child.bold), ("italic", &mut child.italic)] {
                if let Some(value) = fields.get(key).and_then(|value| value.as_bool()) {
                    *flag = value;
                }
            }
            if let Some(value) = fields.get("underlined").and_then(|value| value.as_bool()) {
                child.underline = value;
            }
            // `text` carries literal content; `translate`/`keybind` are server
            // intents the launcher cannot honor and contribute no text.
            if let Some(text) = fields.get("text").and_then(|value| value.as_str()) {
                push_legacy(builder, text, &child);
            }
            if let Some(extra) = fields.get("extra").and_then(|value| value.as_array()) {
                for item in extra.iter().take(MAX_EXTRAS_PER_NODE) {
                    flatten(item, &child, builder, depth + 1);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lines(value: serde_json::Value) -> Vec<Vec<MotdSegment>> {
        convert(&value)
    }

    fn plain(value: serde_json::Value) -> Vec<String> {
        convert(&value)
            .into_iter()
            .map(|line| {
                line.iter()
                    .map(|segment| segment.text.as_str())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn plain_string_and_component_tree_convert() {
        assert_eq!(
            plain(json!("A vanilla MOTD")),
            vec!["A vanilla MOTD".to_owned()]
        );
        let value = json!({
            "text": "",
            "extra": [
                { "text": "Welcome", "color": "gold", "bold": true },
                { "text": " to ", "color": "#33AAFF" },
                { "text": "the server", "italic": true, "underlined": true }
            ]
        });
        let converted = lines(value.clone());
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].len(), 3);
        assert_eq!(converted[0][0].color.as_deref(), Some("#FFAA00"));
        assert!(converted[0][0].bold);
        assert_eq!(converted[0][1].color.as_deref(), Some("#33aaff"));
        assert!(converted[0][2].italic && converted[0][2].underline);
        // Intra-segment spacing survives (the sanitizer filters characters,
        // it does not trim runs).
        assert_eq!(plain(value), vec!["Welcome to the server".to_owned()]);
    }

    #[test]
    fn array_root_and_legacy_section_codes_convert() {
        let converted = lines(json!(["§aGreen", " plain", "§lbold"]));
        assert_eq!(converted[0][0].color.as_deref(), Some("#55FF55"));
        assert_eq!(converted[0][1].color, None);
        assert!(converted[0][2].bold);
        assert_eq!(plain(json!("§cRed§r reset")), vec!["Red reset".to_owned()]);
    }

    #[test]
    fn newline_splitting_is_bounded_to_three_lines() {
        assert_eq!(
            plain(json!("one\ntwo\nthree")),
            vec!["one".to_owned(), "two".to_owned(), "three".to_owned()]
        );
        assert_eq!(
            plain(json!("a\nb\nc\nd\ne")),
            vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
        );
    }

    #[test]
    fn hostile_content_is_sanitized_and_bounded() {
        // Control characters and bidi/format overrides are stripped by the
        // same character vocabulary the activity bridge applies.
        let converted = lines(json!("ok\u{202e}evil\u{7}control"));
        let text: String = converted[0].iter().map(|s| s.text.clone()).collect();
        assert_eq!(text, "okevilcontrol");
        assert!(!text.contains('\u{7}'));
        assert!(!text.contains('\u{202e}'));

        // Line length is clamped.
        let long = "x".repeat(500);
        let converted = lines(json!(long));
        assert_eq!(converted[0][0].text.chars().count(), MAX_LINE_CHARS);

        // Depth, extras and total segments are bounded; nothing panics.
        let mut deep = json!({ "text": "root" });
        for _ in 0..64 {
            deep = json!({ "text": "", "extra": [deep] });
        }
        let _ = lines(deep);
        let mut wide = Vec::new();
        for index in 0..512 {
            wide.push(json!({ "text": format!("s{index}") }));
        }
        let converted = lines(json!(wide));
        let total: usize = converted.iter().map(|line| line.len()).sum();
        assert!(total <= MAX_TOTAL_SEGMENTS);

        // Unknown colors and unsupported styles contribute nothing unsafe.
        let converted = lines(json!({ "text": "hi", "color": "javascript", "obfuscated": true }));
        assert_eq!(converted[0][0].color, None);
        assert!(!converted[0][0].bold);
        // Invalid hex is dropped entirely, never forwarded.
        let converted = lines(json!({ "text": "hi", "color": "#ZZZZZZ" }));
        assert_eq!(converted[0][0].color, None);
    }

    #[test]
    fn translate_and_keybind_intents_produce_no_text() {
        assert!(
            lines(json!({ "translate": "multiplayer.status.hiding_ip", "with": [] })).is_empty()
        );
        assert_eq!(
            plain(json!("before \u{a7}kobfuscated\u{a7}r after")),
            vec!["before obfuscated after".to_owned()]
        );
    }
}
