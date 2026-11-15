//! Player-facing language. Wire identifiers and numerical values stay unchanged.
use crate::ui::Tone;
use aurora_launcher_lib::pulsar::view::{FightSummary, MetricValue, ResolutionView};
pub fn text(input: &str) -> String {
    let clean = input
        .replace(" | ", " · ")
        .replace(" / ", " · ")
        .replace("play/pause", "play or pause")
        .replace("Opener/reset/unavailable", "Opener, reset or unavailable")
        .replace("ideal/candidate", "ideal and candidate")
        .replace("Same-tick/nearby", "Same-tick and nearby")
        .replace("participant/session", "participant or session")
        .replace("target/tolerance", "target and tolerance")
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
                    && !matches!(word, "HP" | "UI" | "SD" | "UTC" | "MB" | "HUD" | "HTML"))
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
pub fn resolution(r: &ResolutionView) -> (&str, Tone) {
    match r.classification.as_str() {
        "CONFIRMED_WIN" if r.confidence.eq_ignore_ascii_case("confirmed") => {
            ("Win (confirmed)", Tone::Success)
        }
        "CONFIRMED_LOSS" if r.confidence.eq_ignore_ascii_case("confirmed") => {
            ("Loss (confirmed)", Tone::Danger)
        }
        _ => (r.label.as_str(), Tone::Neutral),
    }
}
pub fn count(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}
/// Compare the same rounded operands the player sees; never print a signed zero.
pub fn comparison_delta(left: &MetricValue, right: &MetricValue) -> String {
    let (Some(a), Some(b)) = (
        left.value.filter(|v| v.is_finite()),
        right.value.filter(|v| v.is_finite()),
    ) else {
        return "Unavailable".into();
    };
    let unit = left.definition.unit.as_str();
    let precision = if left.definition.key == "averageCombo" {
        1
    } else if unit == "count"
        || (matches!(unit, "" | "dimensionless") && a.fract() == 0. && b.fract() == 0.)
    {
        0
    } else if matches!(unit, "" | "dimensionless") {
        3
    } else {
        1
    };
    let round = |v: f64| format!("{v:.precision$}").parse::<f64>().unwrap_or(v);
    let n = round(b) - round(a);
    let suffix = if unit == "%" {
        "pp"
    } else if matches!(unit, "count" | "dimensionless") {
        ""
    } else {
        unit
    };
    if n.abs() < 10_f64.powi(-(precision as i32)) / 2. {
        if a != b {
            return format!(
                "{}<{} {}",
                if b > a { "+" } else { "-" },
                10_f64.powi(-(precision as i32)),
                suffix
            )
            .trim()
            .into();
        }
        return format!("0 {suffix}").trim().into();
    }
    format!("{n:+.precision$} {suffix}").trim().into()
}
/// Session statistics and opponent-specific streak analysis have different scopes.
pub fn summary_insight(
    manual_session: bool,
    metrics: &std::collections::BTreeMap<String, MetricValue>,
    derived: Option<&String>,
) -> Option<String> {
    if manual_session {
        return metrics.get("longestCombo").and_then(|m| m.value).map(|n| format!("Entire manual session \u{b7} Longest recorded combo: {n:.0} hits. Select a participant or exchange for opponent-specific analysis."));
    }
    derived.cloned()
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
    fn comparisons_match_visible_operands_without_signed_zero() {
        let mut a = MetricValue::default();
        a.definition.unit = "m".into();
        a.value = Some(0.24);
        let mut b = a.clone();
        b.value = Some(0.26);
        assert_eq!(comparison_delta(&a, &b), "+0.1 m");
        b.value = Some(0.245);
        assert_eq!(comparison_delta(&a, &b), "+<0.1 m");
        assert_eq!(comparison_delta(&b, &a), "-<0.1 m");
        assert_eq!(comparison_delta(&a, &a), "0 m");
        a.definition.unit.clear();
        a.value = Some(14.);
        b.value = Some(12.);
        assert_eq!(comparison_delta(&a, &b), "-2");
        b.value = None;
        assert_eq!(comparison_delta(&a, &b), "Unavailable");
    }
    #[test]
    fn manual_session_insight_uses_recorded_scope() {
        let mut m = MetricValue::default();
        m.value = Some(5.);
        let metrics = [("longestCombo".into(), m)].into_iter().collect();
        let derived = "No two of your confirmed hits landed consecutively.".to_owned();
        assert!(
            summary_insight(true, &metrics, Some(&derived))
                .unwrap()
                .contains("5 hits")
        );
        assert_eq!(
            summary_insight(false, &metrics, Some(&derived)),
            Some(derived)
        );
        assert_eq!(summary_insight(true, &Default::default(), None), None);
    }
}
