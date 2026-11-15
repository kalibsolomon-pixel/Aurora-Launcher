//! Player-facing language. Wire identifiers and numerical values stay unchanged.
use crate::ui::Tone;
use aurora_launcher_lib::pulsar::view::{FightSummary, MetricValue};
pub fn text(input: &str) -> String {
    let clean = input
        .replace(" | ", " · ")
        .replace(" / ", " · ")
        .replace("play/pause", "play or pause")
        .replace("inactive/incomplete", "inactive and incomplete")
        .replace("Tab/Shift", "Tab or Shift");
    clean
        .split_inclusive(|c: char| {
            c.is_whitespace() || matches!(c, '·' | '(' | ')' | ',' | '|' | ';')
        })
        .map(|part| {
            let end = part.trim_end_matches(|c: char| {
                c.is_whitespace() || matches!(c, '·' | '(' | ')' | ',' | '|' | ';' | '.' | ':')
            });
            let suffix = &part[end.len()..];
            let word = end.strip_prefix("minecraft:").unwrap_or(end);
            if word == "true" || word == "false" {
                return format!("{}{suffix}", if word == "true" { "Yes" } else { "No" });
            }
            // Source paths, player identifiers and units retain their spelling.
            if word.contains(['\\', '/']) {
                return format!("{word}{suffix}");
            }
            if word.contains('_')
                || (word.len() > 1
                    && word.chars().any(|c| c.is_ascii_alphabetic())
                    && word.chars().all(|c| !c.is_ascii_lowercase())
                    && !matches!(word, "HP" | "UI" | "SD" | "UTC"))
            {
                let mut s = word.replace('_', " ").to_lowercase();
                if let Some(c) = s.get_mut(0..1) {
                    c.make_ascii_uppercase();
                }
                format!("{s}{suffix}")
            } else {
                format!("{word}{suffix}")
            }
        })
        .collect()
}
pub fn outcome(s: &FightSummary) -> Tone {
    if !s.resolution.confidence.eq_ignore_ascii_case("confirmed") {
        return Tone::Neutral;
    }
    match s.outcome.as_str() {
        "WIN" => Tone::Success,
        "LOSS" => Tone::Danger,
        _ => Tone::Neutral,
    }
}
pub fn count(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}
pub fn delta(m: &MetricValue, n: Option<f64>) -> String {
    let Some(n) = n.filter(|v| v.is_finite()) else {
        return "Unavailable".into();
    };
    match m.definition.unit.as_str() {
        "%" => format!("{n:+.1} pp"),
        "count" | "" if n.fract() == 0. => format!("{n:+.0}"),
        unit => format!("{n:+.1} {unit}"),
    }
}
/// Same launcher components, with one consistent presentation boundary.
pub mod ui {
    use crate::theme::AuroraTheme;
    pub use crate::ui::*;
    use gpui::{Div, SharedString};
    fn clean(v: impl Into<SharedString>) -> String {
        super::text(v.into().as_ref())
    }
    pub fn caption(t: &AuroraTheme, v: impl Into<SharedString>) -> Div {
        crate::ui::caption(t, clean(v))
    }
    pub fn muted(t: &AuroraTheme, v: impl Into<SharedString>) -> Div {
        crate::ui::muted(t, clean(v))
    }
    pub fn stat(
        t: &AuroraTheme,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Div {
        crate::ui::stat(t, clean(label), clean(value))
    }
    pub fn badge(t: &AuroraTheme, label: impl Into<SharedString>, tone: Tone) -> Div {
        crate::ui::badge(t, clean(label), tone)
    }
    pub fn section_header(
        t: &AuroraTheme,
        title: impl Into<SharedString>,
        caption: Option<SharedString>,
    ) -> Div {
        crate::ui::section_header(t, clean(title), caption.map(|c| clean(c).into()))
    }
    pub fn title_block(
        t: &AuroraTheme,
        title: impl Into<SharedString>,
        caption: Option<SharedString>,
    ) -> Div {
        crate::ui::title_block(t, clean(title), caption.map(|c| clean(c).into()))
    }
    pub fn page_header(
        t: &AuroraTheme,
        title: impl Into<SharedString>,
        caption: Option<SharedString>,
    ) -> Div {
        crate::ui::page_header(t, clean(title), caption.map(|c| clean(c).into()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_language_keeps_units_and_distinguishes_counts() {
        assert_eq!(
            text("OPPONENT_DIRECT | SHIELD_BLOCK / minecraft:diamond_sword"),
            "Opponent direct · Shield block · Diamond sword"
        );
        assert_eq!(
            text("true false untrue falsehood UTC"),
            "Yes No untrue falsehood UTC"
        );
        assert_eq!(
            text("C:\\some_path\\report.json"),
            "C:\\some_path\\report.json"
        );
        assert_eq!(count(1, "fight"), "1 fight");
        assert_eq!(count(2, "fight"), "2 fights");
    }
    #[test]
    fn comparison_preserves_units_direction_and_missing() {
        let mut m = MetricValue::default();
        m.definition.unit = "%".into();
        assert_eq!(delta(&m, Some(-2.5)), "-2.5 pp");
        m.definition.unit = "m".into();
        assert_eq!(delta(&m, Some(0.25)), "+0.2 m");
        m.definition.unit = "count".into();
        assert_eq!(delta(&m, Some(3.)), "+3");
        assert_eq!(delta(&m, None), "Unavailable");
    }
}
