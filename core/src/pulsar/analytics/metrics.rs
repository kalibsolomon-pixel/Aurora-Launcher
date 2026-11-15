use super::super::view::*;
use super::{num, ratio};
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};
pub(crate) fn definitions() -> &'static [MetricDef] {
    static DEFS: OnceLock<Vec<MetricDef>> = OnceLock::new();
    DEFS.get_or_init(|| serde_json::from_str(include_str!("metric-defs.json")).unwrap())
}
pub(crate) fn definition(key: &str) -> MetricDef {
    definitions()
        .iter()
        .find(|d| d.key == key)
        .cloned()
        .unwrap_or_else(|| MetricDef {
            key: key.into(),
            label: key.into(),
            unit: String::new(),
            confidence: "Observed".into(),
            help: "Recorded observation; see the selected recording's evidence and coverage."
                .into(),
        })
}
pub(crate) fn metric(key: &str, value: Option<f64>) -> MetricValue {
    let value = value.filter(|v| v.is_finite());
    MetricValue {
        definition: definition(key),
        value,
        availability: if value.is_some() {
            Availability::Available
        } else {
            Availability::Unavailable
        },
        reason: value
            .is_none()
            .then(|| "No eligible recorded samples.".into()),
    }
}
pub(crate) fn values(derived: &Value) -> BTreeMap<String, MetricValue> {
    let s = &derived["summary"];
    let reach = &derived["reach"];
    let aim = &derived["aim"];
    let shields = &derived["shields"];
    let timing = derived["timingSamples"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let graded = timing
        .iter()
        .filter(|t| {
            matches!(
                t["grade"].as_str(),
                Some("PERFECT" | "TOO_FAST" | "TOO_SLOW")
            )
        })
        .count();
    let grades = aim["grades"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let sum: f64 = grades.iter().filter_map(Value::as_f64).sum();
    let central: f64 = grades.iter().take(2).filter_map(Value::as_f64).sum();
    definitions()
        .iter()
        .map(|d| {
            let v = match d.key.as_str() {
                "fights" => Some(1.0),
                "dominance" => None,
                "dominanceShare" => derived["dominance"]["localShare"].as_f64(),
                "reachWithin" => reach["withinPercent"].as_f64(),
                "reachDeviation" => reach["deviation"].as_f64(),
                "reachError" => reach["meanAbsoluteError"].as_f64(),
                "aimCentral" => ratio(100.0 * central, sum),
                "aimDistance" => aim["meanDistance"].as_f64(),
                "timingPerfect" => ratio(
                    100.0 * timing.iter().filter(|t| t["grade"] == "PERFECT").count() as f64,
                    graded as f64,
                ),
                "knockback" => derived["incomingDetail"].as_array().map(|rows| {
                    rows.iter()
                        .filter(|r| r["attributed"] == true && r["knockback"].as_f64().is_some())
                        .count() as f64
                }),
                "attacksIntoShield" => shields["attacksIntoShield"].as_f64(),
                "shieldBlocks" => shields["confirmedBlocks"].as_f64(),
                "estimatedShieldBlocks" => shields["estimatedBlocks"].as_f64(),
                "shieldDisables" => shields["disables"].as_f64(),
                "hitsAfterDisable" => shields["hitsAfterDisable"].as_f64(),
                "localShieldBlocks" => shields["localBlocks"].as_f64(),
                "momentum" => None,
                _ => s[&d.key].as_f64(),
            };
            let mut m = metric(&d.key, v);
            if d.key == "longestOpponentCombo" {
                match s["opponentComboAvailability"].as_str() {
                    Some("Unavailable") => {
                        m.value = None;
                        m.availability = Availability::Unavailable;
                        m.reason =
                            Some("No tracked opponent or incomplete incoming attribution.".into());
                    }
                    Some("Estimated") => {
                        m.availability = Availability::Estimated;
                        m.reason = Some("Some incoming damage could not be attributed.".into());
                    }
                    _ => {}
                }
            }
            if matches!(
                d.key.as_str(),
                "attacksIntoShield"
                    | "shieldBlocks"
                    | "estimatedShieldBlocks"
                    | "shieldDisables"
                    | "hitsAfterDisable"
                    | "localShieldBlocks"
            ) && shields["available"] != true
            {
                m.value = None;
                m.availability = Availability::Unavailable;
                m.reason = Some("Structured shield context requires schema 3 or newer.".into());
            }
            (d.key.clone(), m)
        })
        .collect()
}
pub(crate) fn summary_values(summary: &Value) -> BTreeMap<String, MetricValue> {
    definitions()
        .iter()
        .map(|d| (d.key.clone(), metric(&d.key, summary[&d.key].as_f64())))
        .collect()
}
pub(crate) fn extra(
    key: &str,
    label: &str,
    unit: &str,
    help: &str,
    value: Option<f64>,
) -> MetricValue {
    let mut m = metric(key, value);
    m.definition.label = label.into();
    m.definition.unit = unit.into();
    m.definition.help = help.into();
    m
}
#[allow(dead_code)]
fn _assert_num(v: &Value) -> f64 {
    num(v, "")
}
