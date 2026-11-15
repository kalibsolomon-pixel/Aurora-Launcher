//! Supplemental presentation data over frozen statistics and canonical analytics.
//! Kept separate from the exact format-7 oracle payload.
use super::{
    analytics::{self, Analytics, metrics},
    projection,
    store::Entry,
    view::*,
    wire::FightRecord,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
type Metrics = BTreeMap<String, MetricValue>;

fn insert(out: &mut Metrics, key: &str, label: &str, unit: &str, value: Option<f64>, help: &str) {
    out.insert(
        key.into(),
        if metrics::definitions().iter().any(|d| d.key == key) {
            metrics::metric(key, value)
        } else {
            metrics::extra(key, label, unit, help, value)
        },
    );
}
pub(crate) fn summary(f: &FightRecord, a: Option<&Analytics>) -> Metrics {
    let mut out = Metrics::new();
    let help = "Frozen sufficient statistic for this recording or selected exchange; unavailable samples remain distinct from zero.";
    for (key, label, unit, value) in [
        (
            "attempts",
            "Attempts",
            "",
            Some(f.statistics.attempts as f64),
        ),
        (
            "accuracyDenominator",
            "Eligible accuracy attempts",
            "",
            Some((f.statistics.attempts - f.statistics.shield_blocks).max(0) as f64),
        ),
        (
            "comboCount",
            "Local combos",
            "",
            Some(f.statistics.combo_count as f64),
        ),
        ("duration", "Recorded duration", "s", Some(f.duration())),
        (
            "criticalConditions",
            "Hits with critical conditions",
            "",
            Some(f.statistics.critical_conditions as f64),
        ),
        (
            "criticalDenominator",
            "Confirmed hits for critical rate",
            "",
            Some(f.statistics.hits as f64),
        ),
        (
            "intervalCount",
            "Measured hit intervals",
            "",
            Some(f.statistics.interval_count as f64),
        ),
        (
            "reachEligible",
            "Comparable ray-contact hits",
            "",
            a.and_then(|a| a.derived["reach"]["eligible"].as_f64()),
        ),
        (
            "reachWithinCount",
            "Hits inside the frozen reach window",
            "",
            a.and_then(|a| a.derived["reach"]["within"].as_f64()),
        ),
        (
            "reachWithin",
            "Reach in target window",
            "%",
            a.and_then(|a| a.derived["reach"]["withinPercent"].as_f64()),
        ),
        (
            "dominanceActiveSeconds",
            "Complete observed dominance time",
            "s",
            a.map(|a| a.dominance.active),
        ),
        (
            "dominanceLocalSeconds",
            "Local advantage time",
            "s",
            a.map(|a| a.dominance.local),
        ),
        (
            "dominanceOpponentSeconds",
            "Opponent advantage time",
            "s",
            a.map(|a| a.dominance.opponent),
        ),
        (
            "dominanceNeutralSeconds",
            "Neutral observed time",
            "s",
            a.map(|a| a.dominance.neutral),
        ),
    ] {
        insert(&mut out, key, label, unit, value, help);
    }
    let mut share = metrics::metric(
        "dominanceShare",
        a.and_then(|a| a.derived["dominance"]["localShare"].as_f64()),
    );
    if let Some(a) = a {
        match a.dominance.availability {
            "INSUFFICIENT" => {
                share.availability = Availability::Unavailable;
                share.reason = Some("Insufficient tracked-opponent evidence.".into());
            }
            _ => {}
        }
    }
    out.insert("dominanceShare".into(), share);
    out
}

pub(crate) fn pooled(
    entries: &[&Entry],
    timelines: &[(FightKey, FightRecord, Analytics)],
) -> Metrics {
    let mut out = Metrics::new();
    let sum = |get: fn(&super::wire::Statistics) -> f64| {
        entries
            .iter()
            .map(|e| get(&e.record.statistics))
            .sum::<f64>()
    };
    let eligible: f64 = timelines
        .iter()
        .filter_map(|(_, _, a)| a.derived["reach"]["eligible"].as_f64())
        .sum();
    let within: f64 = timelines
        .iter()
        .filter_map(|(_, _, a)| a.derived["reach"]["within"].as_f64())
        .sum();
    let local: f64 = timelines.iter().map(|(_, _, a)| a.dominance.local).sum();
    let opponent: f64 = timelines.iter().map(|(_, _, a)| a.dominance.opponent).sum();
    let neutral: f64 = timelines.iter().map(|(_, _, a)| a.dominance.neutral).sum();
    let active = local + opponent + neutral;
    for (key, label, unit, value) in [
        ("attempts", "Attempts", "", Some(sum(|s| s.attempts as f64))),
        (
            "accuracyDenominator",
            "Eligible accuracy attempts",
            "",
            Some(sum(|s| (s.attempts - s.shield_blocks).max(0) as f64)),
        ),
        (
            "reachEligible",
            "Loaded comparable ray-contact hits",
            "",
            (!timelines.is_empty()).then_some(eligible),
        ),
        (
            "reachWithinCount",
            "Loaded hits inside their frozen reach window",
            "",
            (!timelines.is_empty()).then_some(within),
        ),
        (
            "criticalConditions",
            "Hits with critical conditions",
            "",
            Some(sum(|s| s.critical_conditions as f64)),
        ),
        (
            "criticalDenominator",
            "Confirmed hits for critical rate",
            "",
            Some(sum(|s| s.hits as f64)),
        ),
        (
            "comboCount",
            "Local combos",
            "",
            Some(sum(|s| s.combo_count as f64)),
        ),
        (
            "intervalCount",
            "Measured hit intervals",
            "",
            Some(sum(|s| s.interval_count as f64)),
        ),
        (
            "dominanceActiveSeconds",
            "Complete observed dominance time",
            "s",
            Some(active),
        ),
        (
            "dominanceLocalSeconds",
            "Local advantage time",
            "s",
            Some(local),
        ),
        (
            "dominanceOpponentSeconds",
            "Opponent advantage time",
            "s",
            Some(opponent),
        ),
        (
            "dominanceNeutralSeconds",
            "Neutral observed time",
            "s",
            Some(neutral),
        ),
        (
            "opponentDominanceShare",
            "Pooled opponent advantage share",
            "%",
            analytics::ratio(100.0 * opponent, active),
        ),
        (
            "neutralDominanceShare",
            "Pooled neutral share",
            "%",
            analytics::ratio(100.0 * neutral, active),
        ),
        (
            "recordings",
            "Selected recordings",
            "",
            Some(entries.len() as f64),
        ),
        (
            "timelineRecordings",
            "Loaded timelines",
            "",
            Some(timelines.len() as f64),
        ),
        (
            "contextRecordings",
            "Loaded structured-context timelines",
            "",
            Some(
                timelines
                    .iter()
                    .filter(|(_, f, _)| f.schema_version >= 3)
                    .count() as f64,
            ),
        ),
    ] {
        insert(
            &mut out,
            key,
            label,
            unit,
            value,
            "Pooled frozen counters cover all selected recordings; ray reach and dominance denominators cover loaded timelines only. Dominance shares are duration-weighted complete observed time.",
        );
    }
    out
}

pub(crate) fn aim_regions(a: &Analytics) -> BTreeMap<i64, Value> {
    a.events.iter().filter_map(|e| {
        let attack=e.attack.as_ref().filter(|a|a.ray_contact)?;
        let m=attack.measurement.as_ref()?;
        Some((e.sequence,json!({"idealRegion":m.ideal_region.as_ref()?,"box":m.r#box,"face":analytics::face(attack),"confidence":e.confidence})))
    }).collect()
}

pub(crate) fn detail_tables(f: &FightRecord) -> Vec<TableView> {
    let mut tables = Vec::new();
    let rounds:Vec<_>=f.evidence().rounds.iter().map(|r|json!({"round":r.number,"start":f.seconds(r.start_nanos),"end":r.end_nanos.map(|n|f.seconds(n)),"outcome":r.outcome,"evidence":r.rule,"confidence":r.confidence})).collect();
    tables.push(projection::table(
        "round_history",
        "Round history",
        &json!(rounds),
        None,
    ));
    if let Some(session) = &f.session {
        let rows:Vec<_>=session.announcements.iter().map(|e|json!({"time":f.seconds(e.nano_time),"kind":e.kind,"rule":e.rule,"evidence":e.text,"accepted":e.accepted,"origin":e.origin})).collect();
        tables.push(projection::table(
            "manual_announcements",
            "Manual session announcements",
            &json!(rows),
            None,
        ));
    }
    tables
}

pub(crate) fn focused_flow(tab: &mut TabView, d: &Value) {
    let longest = &d["dominance"]["longest"];
    let swing = &d["dominance"]["swing"];
    for (key, label, unit, value) in [
        (
            "longestAdvantageStart",
            "Longest advantage start",
            "s",
            longest["start"].as_f64(),
        ),
        (
            "longestAdvantageEnd",
            "Longest advantage end",
            "s",
            longest["end"].as_f64(),
        ),
        (
            "longestAdvantageDuration",
            "Longest advantage duration",
            "s",
            longest["start"]
                .as_f64()
                .zip(longest["end"].as_f64())
                .map(|(s, e)| e - s),
        ),
        (
            "largestSwingFrom",
            "Largest swing start",
            "s",
            swing["from"].as_f64(),
        ),
        (
            "largestSwingTo",
            "Largest swing end",
            "s",
            swing["to"].as_f64(),
        ),
        (
            "largestSwingFromValue",
            "Largest swing initial dominance",
            "",
            swing["fromValue"].as_f64(),
        ),
        (
            "largestSwingToValue",
            "Largest swing final dominance",
            "",
            swing["toValue"].as_f64(),
        ),
        (
            "largestSwingDifference",
            "Largest swing absolute difference",
            "",
            swing["fromValue"]
                .as_f64()
                .zip(swing["toValue"].as_f64())
                .map(|(s, e)| (e - s).abs()),
        ),
    ] {
        let mut m = metrics::extra(
            key,
            label,
            unit,
            "Focused recording's decayed-evidence model. This is not a pooled maximum or a probability; the swing window is at most three seconds.",
            value,
        );
        m.definition.confidence = metrics::definition("dominance").confidence;
        tab.metrics.insert(key.into(), m);
    }
    tab.tables.push(projection::table("dominance_longest","Focused longest advantage",&json!([{"side":longest["side"],"start":longest["start"],"end":longest["end"],"duration":longest["start"].as_f64().zip(longest["end"].as_f64()).map(|(s,e)|e-s)}]),None));
    tab.tables.push(projection::table("dominance_swing","Focused largest swing",&json!([{"from":swing["from"],"to":swing["to"],"fromValue":swing["fromValue"],"toValue":swing["toValue"],"absoluteDifference":swing["fromValue"].as_f64().zip(swing["toValue"].as_f64()).map(|(s,e)|(e-s).abs())}]),None));
}
