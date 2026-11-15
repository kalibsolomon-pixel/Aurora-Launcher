//! Player-facing language. Wire identifiers and numerical values stay unchanged.
use crate::ui::Tone;
use aurora_launcher_lib::pulsar::view::{FightSummary, MetricValue, ResolutionView};
/// Core copy that names capture mechanics, rewritten for players. Core strings
/// stay unchanged because interpretations are part of the analytics oracles.
const PLAIN: &[(&str, &str)] = &[
    // Metric definitions.
    (
        "Confirmed hits / (attempts minus confirmed shield blocks). Uncertain attempts stay in the denominator.",
        "Confirmed hits out of your attacks, leaving out attacks that a shield confirmed it blocked. Attacks without a confirmed result still count.",
    ),
    (
        "Server melee damage events from the local player, matched to an attack.",
        "Hits the server confirmed as damage from your attacks.",
    ),
    (
        "Unique eligible combat attempts. Noncombat inputs and duplicates are excluded; older recordings keep their original policy.",
        "Attacks aimed at an opponent. Clicks outside combat and repeated clicks are left out.",
    ),
    (
        "Aimed empty swings with opponent geometry and no server confirmation in the confirmation window. Inferred misses, not proof of server rejection. Historical empty-swing counts retain their older policy.",
        "Swings aimed at the opponent that the server never confirmed. Likely misses, not proof that the server rejected them.",
    ),
    (
        "Entity attacks with no matching server report. Not proven misses.",
        "Attacks on the opponent that the server neither confirmed nor rejected. Not proven misses.",
    ),
    (
        "Eye to attack-pick contact on confirmed survival hits. Not a server verdict.",
        "Distance from your eyes to where the hit landed, on confirmed survival hits. Measured by your game, not the server.",
    ),
    (
        "Largest eye to attack-pick contact on a confirmed survival hit.",
        "Longest distance from your eyes to where a confirmed survival hit landed.",
    ),
    (
        "Mean attack-time gap between consecutive confirmed hits.",
        "Average time between consecutive confirmed hits.",
    ),
    (
        "Population standard deviation of hit intervals; lower is more consistent.",
        "How much the time between hits varied; lower is more consistent.",
    ),
    (
        "Actual jumps within the signed client-tick window around incoming damage. Same tick is an ideal candidate; success is unavailable.",
        "Jumps close in time to a hit on you. Jumping on the same tick is ideal; whether it reduced knockback is not measured.",
    ),
    (
        "Most consecutive confirmed hits before an attributed return hit, a same-tick trade or a disengagement gap (combo timeout).",
        "Most hits in a row before the opponent hit back, you traded hits or the fight paused.",
    ),
    (
        "Mean length of combos: runs of two or more consecutive confirmed hits.",
        "Average length of your combos: two or more confirmed hits in a row.",
    ),
    (
        "Older recordings' per-hit value: +0.20 per hit, -0.25 per attributed incoming hit, 3 s decay. Superseded by encounter dominance.",
        "Momentum score from older recordings. Newer recordings use dominance instead.",
    ),
    (
        "Server damage events whose cause entity is the tracked opponent. Only these break your combos or move dominance.",
        "Hits on you that the server credited to your opponent. Only these end your combos or change dominance.",
    ),
    (
        "Damage whose cause the server did not report (fall, fire, environment). Third-party damage is shown separately. Neither counts as opponent retaliation.",
        "Damage with no known source, such as falls or fire. Damage from other attackers is counted separately; neither counts as your opponent hitting back.",
    ),
    (
        "Attacks made while the target's client-visible state showed an active shield.",
        "Attacks made while your opponent was holding up a shield.",
    ),
    (
        "Attack into a blocking target followed by the server's shield sound at the target and no damage event. Removed from the accuracy denominator.",
        "Your attack met a raised shield, the server played the block sound and no damage followed. These do not count against your accuracy.",
    ),
    (
        "Attack into a raised, facing shield with no damage event and no shield sound. Stays uncertain and in the accuracy denominator.",
        "Your attack met a raised shield but no block sound was recorded. Still uncertain, so it counts as an attempt.",
    ),
    (
        "Your attack followed by the shield-disable sound at the opponent.",
        "Your attack disabled the opponent's shield.",
    ),
    (
        "Shield-block sounds at your position. The blocked attacker is not reported.",
        "Blocks made with your shield. The game does not report who you blocked.",
    ),
    (
        "Your confirmed hits with the server's critical-hit animation on the opponent within one tick. Damage is not reported.",
        "Confirmed hits where the server showed the critical-hit animation. Extra damage is not recorded.",
    ),
    (
        "Same rule using attributed incoming hits. Estimated or unavailable when some incoming damage is unattributed.",
        "The opponent's longest run of hits on you. Unavailable when some damage came from an unknown source.",
    ),
    (
        "Most consecutive confirmed hits before an attributed return hit, ignoring disengagement gaps.",
        "Most hits in a row before the opponent hit back, even across pauses.",
    ),
    (
        "Decayed share of confirmed evidence: (P - N) / (P + N + 1), 3 s time constant. Neutral or not drawn when evidence is thin or incoming damage is unattributed.",
        "Who was landing more of the recent hits, from -1 (opponent) to +1 (you). Not drawn when there is too little evidence.",
    ),
    (
        "Share of observed active combat time with dominance above +0.2 (you) or below -0.2 (opponent). Inactive and incomplete periods are excluded.",
        "Share of active combat in which you clearly held the advantage. Pauses and unclear periods are left out.",
    ),
    (
        "Eligible confirmed hits (ray contact) within the configured target and tolerance.",
        "Confirmed hits landed inside your target reach window.",
    ),
    (
        "Population standard deviation of eligible confirmed-hit reach; lower is more consistent.",
        "How much your hit distance varied; lower is more consistent.",
    ),
    (
        "Mean absolute difference between eligible hit reach and the configured target.",
        "Average distance between your hits and your target reach.",
    ),
    (
        "Hits graded ideal or good by the client hitbox heuristic. Not a damage measure.",
        "Hits that landed on the ideal or good part of the opponent's hitbox. Not a damage measure.",
    ),
    (
        "Mean blocks between the recorded contact and the region's ideal point; SD is aim consistency.",
        "Average distance in blocks between where you hit and the ideal point.",
    ),
    (
        "Graded attack intervals within the tick tolerance of the weapon's cooldown. Mechanical readiness, not tactical quality.",
        "Attacks timed to the weapon's cooldown within a few ticks. This shows readiness, not tactical timing.",
    ),
    (
        "Sum of observed decreases of your health plus absorption during the recording.",
        "Total health, including absorption hearts, that you lost during the recording.",
    ),
    (
        "Incoming hits with a server velocity update for you within one tick.",
        "Hits on you where the server pushed you back straight away.",
    ),
    (
        "Fights and manual sessions in local history.",
        "Fights and manual sessions in your library.",
    ),
    (
        "Confirmed source / inferred match",
        "Confirmed by the server",
    ),
    ("Confirmed source", "Confirmed by the server"),
    ("Observed geometry", "Measured position"),
    // Projection captions, chart notes and notices.
    (
        "Recorded analytical value; availability and evidence follow the selected scope.",
        "Recorded measurement for the fights in view.",
    ),
    (
        "Uncertain attacks stay in the accuracy denominator. Critical conditions do not establish damage, and sprint state does not establish knockback.",
        "Attacks without a confirmed result still count as attempts. A critical or sprint hit does not prove extra damage or knockback.",
    ),
    (
        "Confirmed, comparable ray-contact hits only.",
        "Only confirmed hits with a measured contact point.",
    ),
    (
        "Target recorded with this scope, independent of current preferences.",
        "The target saved with these recordings, not your current settings.",
    ),
    (
        "Inclusive target window, with a 1e-9 geometric tolerance.",
        "Hits exactly on the edge of the window count as inside.",
    ),
    (
        "Population standard deviation of distance to the frozen ideal region.",
        "How much your contact point varied from the ideal point; lower is more consistent.",
    ),
    (
        "Mechanical cooldown interval tolerance; not tactical timing.",
        "How close to the weapon cooldown an attack must be. This is not tactical timing.",
    ),
    (
        "Frozen canonical input policy. Historical recordings retain their original classification.",
        "Counted with the rules saved in each recording.",
    ),
    ("Canonical miss classifications", "Counted misses"),
    (
        "Accuracy is confirmed hits out of eligible attempts. Confirmed shield blocks are excluded; uncertain results remain in the denominator.",
        "Accuracy is confirmed hits out of eligible attempts. Confirmed shield blocks are left out; attempts without a confirmed result still count.",
    ),
    (
        "Distances use frozen geometry. Creative reach is unavailable for survival comparison.",
        "Distances come from the recorded hit positions. Creative-mode reach is not compared with survival.",
    ),
    (
        "Population standard deviation of graded confirmed hit distance.",
        "How much the distance of graded hits varied; lower is more consistent.",
    ),
    (
        "Measured contacts only. Hollow contacts did not establish a hit; grades are geometric, not damage.",
        "Measured contacts only. Hollow marks did not land a confirmed hit; grades describe where you hit, not damage.",
    ),
    (
        "Hit square, shield diamond, no-hit hollow circle; openers and ungraded resets stay unavailable.",
        "Square: hit · Diamond: shield · Hollow circle: no hit. First swings and resets have no interval.",
    ),
    (
        "Frozen mechanical interval difference.",
        "Difference between your attack interval and the weapon cooldown.",
    ),
    (
        "Observed context only; missing evidence cannot establish prevention, dealt damage or reset success.",
        "Only what was observed. Missing evidence cannot show prevented damage, damage dealt or a successful jump reset.",
    ),
    (
        "Actual jumps, not jump input. Same-tick/nearby candidates do not establish success.",
        "Jumps that happened, not key presses. A jump near the hit does not prove it reduced knockback.",
    ),
    (
        "Signed client-tick jump offsets",
        "Jump timing around each hit",
    ),
    (
        "Candidate window; zero is same tick",
        "Jump window; zero is the same tick",
    ),
    (
        "Offsets clamp to -10/+10 for display only. Pairing uses nearest actual jump with earlier sequence on ties.",
        "Offsets beyond 10 ticks are drawn at the edge. Each hit is paired with the nearest jump.",
    ),
    (
        "Sound observations identify shield subject; local attacker identity is unavailable.",
        "Shield sounds show whose shield blocked; who attacked your shield is not recorded.",
    ),
    (
        "Shared client-tick lanes with signed pairing offsets. Actual jumps, not inputs; no world reconstruction.",
        "Hits on you and your jumps on one game-tick timeline.",
    ),
    (
        "Combos use evidence time and attributed incoming hits; same-tick trades extend neither side's streak.",
        "A combo ends when the opponent hits back. A trade on the same tick extends neither side's combo.",
    ),
    (
        "Dominance is a deterministic decayed evidence share, not win probability or physical momentum.",
        "Dominance shows who was landing more of the recent hits. It is not a win probability.",
    ),
    (
        "Two-sided evidence timeline with trade marks and combo bands. Boundaries split recorded streaks.",
        "Your hits on top, the opponent's below; shaded bands are combos. Round boundaries split combos.",
    ),
    ("Decayed confirmed evidence", "Recent confirmed hits"),
    ("Frozen combat-active intervals", "Active combat periods"),
    (
        "Active intervals include the recorded timeout tail; these differ from hit-streak exchanges.",
        "Each period runs until combat paused, so it can be longer than the exchange of hits.",
    ),
    ("Evidence-time hit marks", "Hit marks"),
    ("Historical hit-streak exchanges", "Exchanges of hits"),
    ("Combat-active exchanges", "Active combat periods"),
    (
        "Entire manual sessions have no tracked opponent. Select one participant exchange for opponent combos and dominance.",
        "A manual session has no single opponent. Select an exchange above for opponent combos and dominance.",
    ),
    (
        "Evidence-time local marks; trades and attributed return hits reset the run.",
        "Your hits; a trade or a return hit from the opponent ends the run.",
    ),
    (
        "Canonical attempts use attack time; streak marks use evidence time. No world movement or damage dealt is reconstructed.",
        "Attempts are placed when you swung and combo marks when the hit was confirmed. Movement and damage dealt are not shown.",
    ),
    (
        "Chronological recorded sufficient statistics. Missing denominators are gaps; never sort formatted strings.",
        "One point per recording, oldest first. Gaps mean the value could not be measured.",
    ),
    (
        "Confirmed local combos from loaded timelines in this scope; single hits are excluded.",
        "Your combos, pooled across the selected recordings. A combo is 2+ hits in a row; single hits aren't combos.",
    ),
    (
        "Distributions include loaded timelines only; counts and sufficient-stat rates include all selected recordings.",
        "Charts use recordings with a full timeline; totals include every selected recording.",
    ),
    (
        "Duration-weighted complete active samples; never an average of fight percentages.",
        "Weighted by how long each fight was active, not an average of fight percentages.",
    ),
    (
        "Duration-weighted complete active samples.",
        "Weighted by how long each fight was active.",
    ),
    (
        "Fight-level evidence availability in the loaded timeline subset.",
        "Which recordings have a full timeline.",
    ),
    (
        "Incoming-hit attribution was incomplete, so encounter dominance could not be established confidently.",
        "Some hits on you came from an unknown source, so dominance could not be measured reliably.",
    ),
    (
        "Confirmed blocks are excluded from the accuracy denominator.",
        "Confirmed blocks do not count against your accuracy.",
    ),
    ("configured target window", "target window"),
    (
        "Frozen sufficient statistic for this recording or selected exchange; unavailable samples remain distinct from zero.",
        "Recorded count for this fight or exchange. Unavailable is not the same as zero.",
    ),
    (
        "Loaded comparable ray-contact hits",
        "Hits with a measured contact point",
    ),
    (
        "Comparable ray-contact hits",
        "Hits with a measured contact point",
    ),
    (
        "Loaded hits inside their frozen reach window",
        "Hits inside their reach window",
    ),
    (
        "Hits inside the frozen reach window",
        "Hits inside the reach window",
    ),
    ("Local advantage time", "Your advantage time"),
    (
        "Some incoming evidence could not be attributed.",
        "Some hits on you came from an unknown source.",
    ),
    (
        "Insufficient tracked-opponent evidence.",
        "Too little evidence about the opponent.",
    ),
    (
        "Loaded structured-context timelines",
        "Recordings with full context",
    ),
    (
        "Pooled frozen counters cover all selected recordings; ray reach and dominance denominators cover loaded timelines only. Dominance shares are duration-weighted complete observed time.",
        "Totals cover every selected recording; reach and dominance use recordings with a full timeline. Dominance shares are weighted by active time.",
    ),
    (
        "Focused recording's decayed-evidence model. This is not a pooled maximum or a probability; the swing window is at most three seconds.",
        "From the focused fight only. A swing is the biggest change in dominance within three seconds; it is not a probability.",
    ),
    (
        "The instance registry could not be read; retained library snapshots are still available.",
        "Your instance list could not be read; saved recordings are still available.",
    ),
    (
        "Search, offset or page size exceeds the bounded history query.",
        "That search is too long. Shorten it and try again.",
    ),
    (
        "Frozen count; confirmed blocks are excluded from the accuracy denominator.",
        "Confirmed blocks do not count against your accuracy.",
    ),
    (
        "Frozen observed raised-shield count.",
        "Attacks made while the opponent held up a shield.",
    ),
    (
        "This timeline exceeds the bounded analysis budget; its stored summary and original recording remain available.",
        "This recording is too large to analyse in detail; its summary and original file are still available.",
    ),
    (
        "The exchange does not belong to this participant/session.",
        "That exchange is not part of this session.",
    ),
    (
        "Elapsed time from the frozen recording timeline origin.",
        "Time since the recording started.",
    ),
    (
        "Detailed distributions cover loaded timelines only; stored totals include every selected recording.",
        "Charts use recordings with a full timeline; totals include every selected recording.",
    ),
    (
        "Mixed frozen reach policies; pooled target-window share uses each recording's own policy.",
        "These recordings used different reach targets; each is measured against its own target.",
    ),
    (
        "Mixed frozen reach policies; filtered distribution uses the newest loaded target/tolerance.",
        "These recordings used different reach targets; the chart uses the newest one.",
    ),
    (
        "Best among readable timelines within the 64 MiB / 50,000 event budget; other recordings were skipped.",
        "Best among recordings small enough to analyse in detail.",
    ),
    (
        "Differences are right minus left. Frozen policies and evidence coverage can differ; descriptive metrics are not skill scores.",
        "Each fight is measured with the settings it was recorded with. These describe the fights; they are not skill scores.",
    ),
    ("Generic result text with endpoint", "Server result message"),
    ("Local death observed", "Your death was recorded"),
    ("Opponent left the client world", "Opponent left the game"),
    ("External finalization", "Recording ended outside the game"),
    ("Client stopped or reset", "Game closed or reset"),
    (
        "Recorded observation; see the selected recording's evidence and coverage.",
        "Recorded measurement; see the fight's Evidence tab.",
    ),
    (
        "Structured context requires schema 3 or newer with loaded evidence.",
        "Needs a recording from a newer Aurora Client.",
    ),
    (
        "Structured shield context requires schema 3 or newer.",
        "Needs a recording from a newer Aurora Client.",
    ),
    (
        "No tracked opponent or incomplete incoming attribution.",
        "No single opponent, or some hits on you came from an unknown source.",
    ),
    (
        "Some incoming damage could not be attributed.",
        "Some damage you took came from an unknown source.",
    ),
    (
        "Too few confirmed exchanges were observed to estimate encounter dominance.",
        "Too few confirmed exchanges were recorded to measure dominance.",
    ),
    ("Stored reason: ", "Reason: "),
    (
        "Damage dealt, prevented damage and successful jump resets are not measured. Third-party damage is not opponent retaliation.",
        "Damage dealt, damage prevented and whether jump resets worked are not measured. Damage from other attackers does not count as your opponent hitting back.",
    ),
    (
        "Some incoming damage could not be attributed to the opponent, so opponent combos are",
        "Some damage you took came from an unknown source, so opponent combos are",
    ),
    (
        " attributed incoming hits had an observed jump",
        " hits from your opponent had a jump",
    ),
    (
        "Manual sessions have no tracked opponent, so opponent combos are unavailable.",
        "A manual session has no single opponent, so opponent combos are unavailable.",
    ),
    (
        "0.05 m interior buckets, underflow below 2 m and overflow at 4 m; target shading classifies bucket midpoints.",
        "Bars are 0.05 m wide; the first and last bars collect everything below 2 m and from 4 m. The shaded band is your target reach.",
    ),
    (
        "Cooldown readiness is not a judgement of tactical timing.",
        "This shows readiness, not tactical timing.",
    ),
    ("Frozen reach tolerance", "Reach tolerance"),
    ("Frozen timing tolerance", "Timing tolerance"),
    ("Frozen target window", "Target window"),
    ("Frozen exchange timeout", "Exchange timeout"),
    (
        "Confirmed server-declared win",
        "Win declared by the server",
    ),
    (
        "Confirmed server-declared loss",
        "Loss declared by the server",
    ),
];

/// Recorded enum values a player sees in tables and readouts.
fn word(value: &str) -> Option<&'static str> {
    Some(match value {
        "LOCAL" => "You",
        "OPPONENT_DIRECT" | "TRACKED_OPPONENT" => "Opponent",
        "UNATTRIBUTED" => "Unknown source",
        "THIRD_PARTY" => "Other attacker",
        "IDEAL_CANDIDATE" => "Same tick",
        "ATTEMPT_CANDIDATE" => "Near the hit",
        "NO_CANDIDATE" => "No jump nearby",
        _ => return None,
    })
}

pub fn text(input: &str) -> String {
    let mut plain = input.to_owned();
    for (from, to) in PLAIN {
        if plain.contains(from) {
            plain = plain.replace(from, to);
        }
    }
    let clean = plain
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
            if let Some(plain) = self::word(word) {
                return format!("{plain}{suffix}");
            }
            // Source paths, player identifiers and units retain their spelling.
            if word.contains(['\\', '/', '\u{2013}']) {
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
/// Accepts the recorded source identifier or the label core derives from it.
pub fn result_source(source: &str) -> &str {
    match source {
        "local-melee-source" => "Confirmed by recorded melee contact",
        "SERVER_MATCH_RESULT" | "Server match result" => {
            "Reported by the server at the end of the match"
        }
        "SERVER_ROUND_RESULT" | "Server round result" => {
            "Reported by the server at the end of the round"
        }
        "SERVER_KILL_MESSAGE" | "Server kill message" => {
            "Supported by the server's elimination message"
        }
        "LOCAL_DEATH" | "Local death observed" => "Your death was recorded",
        "OPPONENT_DEATH" | "Opponent death observed" => "Your opponent's death was recorded",
        "MANUAL" | "Ended by you" => "Captured during a manual recording",
        "LEGACY" | "Older recording" => "Saved by an older Aurora Client",
        _ => "Supported by the recording's result evidence",
    }
}
/// Describe event evidence without exposing capture-engine identifiers.
/// Original details remain available in the Evidence diagnostics.
pub fn event_detail(detail: &str) -> String {
    match detail {
        "local-melee-source" => "Recorded melee contact".into(),
        "synthetic melee attempt" => "Recorded attack attempt".into(),
        "empty-swing" => "Swing without a target".into(),
        _ => text(detail),
    }
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
/// One direction convention for every change: arrow plus unsigned magnitude.
pub fn delta_display(delta: &str) -> String {
    if let Some(rest) = delta.strip_prefix('+') {
        format!("\u{2191} {rest}")
    } else if let Some(rest) = delta.strip_prefix('-') {
        format!("\u{2193} {rest}")
    } else if delta == "Unavailable" {
        "\u{2014}".into()
    } else {
        "No change".into()
    }
}
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// `Oct 26 15:33` from a recorded `2026-10-26 15:33 UTC` or `10-26 15:33` stamp.
pub fn short_stamp(stamp: &str) -> String {
    let mut parts = stamp.split_whitespace();
    let (Some(date), Some(clock)) = (parts.next(), parts.next()) else {
        return stamp.to_owned();
    };
    let fields: Vec<_> = date.split('-').collect();
    let (month, day) = match fields.as_slice() {
        [_, m, d] | [m, d] => (m.parse::<usize>().ok(), d.parse::<u32>().ok()),
        _ => (None, None),
    };
    match (month.and_then(|m| MONTHS.get(m.wrapping_sub(1))), day) {
        (Some(month), Some(day)) => format!("{month} {day} {clock}"),
        _ => stamp.to_owned(),
    }
}
/// Session statistics and opponent-specific streak analysis have different scopes.
pub fn summary_insight(
    manual_session: bool,
    metrics: &std::collections::BTreeMap<String, MetricValue>,
    derived: Option<&String>,
) -> Option<String> {
    if manual_session {
        return metrics.get("longestCombo").and_then(|m| m.value).map(|n| format!("Entire manual session \u{b7} Longest recorded combo: {n:.0} hits. Select an exchange above for opponent-specific analysis."));
    }
    derived.cloned()
}
/// Same launcher components, with one consistent presentation boundary.
pub mod ui {
    use crate::theme::AuroraTheme;
    pub use crate::ui::*;
    use gpui::{Div, SharedString, prelude::*};
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
    /// Right-aligned tabular figures so a column of values shares one edge.
    pub fn numeric(d: Div) -> Div {
        d.text_right()
            .whitespace_nowrap()
            .font_features(gpui::FontFeatures(std::sync::Arc::new(vec![(
                "tnum".into(),
                1,
            )])))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_language_keeps_units_and_distinguishes_counts() {
        assert_eq!(
            text("OPPONENT_DIRECT | SHIELD_BLOCK / minecraft:diamond_sword"),
            "Opponent · Shield block · Diamond sword"
        );
        assert_eq!(
            text("Observed health loss. Stored reason: LOCAL"),
            "Observed health loss. Reason: You"
        );
        for (from, to) in PLAIN {
            for banned in [
                "denominator",
                "frozen",
                "Frozen",
                "canonical",
                "Canonical",
                "nullable",
            ] {
                assert!(!to.contains(banned), "{from} -> {to}");
            }
        }
        assert_eq!(
            text("true false untrue falsehood UTC"),
            "Yes No untrue falsehood UTC"
        );
        assert_eq!(
            text("C:\\some_path\\report.json"),
            "C:\\some_path\\report.json"
        );
        assert_eq!(delta_display("-9.6 pp"), "\u{2193} 9.6 pp");
        assert_eq!(delta_display("+<0.1 m"), "\u{2191} <0.1 m");
        assert_eq!(delta_display("0 m"), "No change");
        assert_eq!(delta_display("Unavailable"), "\u{2014}");
        assert_eq!(short_stamp("2026-10-26 15:33 UTC"), "Oct 26 15:33");
        assert_eq!(short_stamp("10-26 04:33"), "Oct 26 04:33");
        assert_eq!(short_stamp("Unavailable"), "Unavailable");
        assert_eq!(count(1, "fight"), "1 fight");
        assert_eq!(count(2, "fight"), "2 fights");
    }
    #[test]
    fn event_source_identifiers_have_player_facing_descriptions() {
        assert_eq!(event_detail("local-melee-source"), "Recorded melee contact");
        assert_eq!(
            result_source("Server match result"),
            result_source("SERVER_MATCH_RESULT")
        );
        assert_eq!(
            event_detail("synthetic melee attempt"),
            "Recorded attack attempt"
        );
        assert_eq!(event_detail("shield sound"), "shield sound");
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
