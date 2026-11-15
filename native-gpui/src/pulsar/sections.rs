//! Contextual sections inside Offense, Defense and Flow (P1). Core names every
//! metric, chart and table semantically; a section is the set of id fragments
//! that belong to it, so filtering never invents or drops a measurement.
use aurora_launcher_lib::pulsar::view::TabView;
pub(super) type Section = (&'static str, &'static [&'static str]);
const OFFENSE: &[Section] = &[
    (
        "Accuracy",
        &[
            "accuracy",
            "accuracyDenominator",
            "hits",
            "attempts",
            "emptyMisses",
            "uncertain",
            "attack_outcomes",
            "totalInputs",
            "uniqueAttacks",
            "excludedSwings",
            "deduplicatedAttacks",
            "eligibleMisses",
            "unresolved",
        ],
    ),
    ("Reach & spacing", &["reach", "Reach"]),
    ("Aim", &["aim"]),
    ("Weapon timing", &["timing"]),
    ("Critical hits", &["critical", "Crit", "crit"]),
    ("Sprint hits", &["sprint"]),
];
const DEFENSE: &[Section] = &[
    (
        "Incoming hits",
        &["incoming", "Incoming", "opponentCrits", "unexplained"],
    ),
    ("Shields", &["shield", "Shield", "hitsAfterDisable"]),
    ("Jump resets", &["jump"]),
    ("Damage & knockback", &["damage", "knockback", "Knockback"]),
];
const FLOW: &[Section] = &[
    (
        "Combos",
        &[
            "combo",
            "Combo",
            "longestCombo",
            "longestOpponentCombo",
            "longestUnanswered",
            "hit_marks",
            "Unanswered",
        ],
    ),
    (
        "Dominance",
        &[
            "dominance",
            "Dominance",
            "turning",
            "longestAdvantage",
            "largestSwing",
        ],
    ),
    ("Exchanges", &["exchange", "Exchange", "Seconds"]),
];

/// Sections for a tab index shared by the pooled and fight-detail strips:
/// 1 Offense, 2 Defense, 3 Flow.
pub(super) fn for_tab(tab: usize) -> &'static [Section] {
    match tab {
        1 => OFFENSE,
        2 => DEFENSE,
        3 => FLOW,
        _ => &[],
    }
}

pub(super) fn matches(id: &str, fragments: &[&str]) -> bool {
    fragments.iter().any(|f| id.contains(f))
}

/// The selected section's slice of a tab. Index 0 means every section.
/// Interpretations and notices are tab-wide qualifications and always stay.
pub(super) fn filter(tab: &TabView, sections: &[Section], selected: usize) -> TabView {
    let Some((_, fragments)) = selected.checked_sub(1).and_then(|i| sections.get(i)) else {
        return tab.clone();
    };
    TabView {
        metrics: tab
            .metrics
            .iter()
            .filter(|(k, _)| matches(k, fragments))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        charts: tab
            .charts
            .iter()
            .filter(|c| matches(&c.id, fragments))
            .cloned()
            .collect(),
        tables: tab
            .tables
            .iter()
            .filter(|t| matches(&t.id, fragments))
            .cloned()
            .collect(),
        interpretations: tab.interpretations.clone(),
        notices: tab.notices.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_launcher_lib::pulsar::view::{ChartView, MetricValue, TableView};

    fn tab(metrics: &[&str], charts: &[&str], tables: &[&str]) -> TabView {
        TabView {
            metrics: metrics
                .iter()
                .map(|k| (k.to_string(), MetricValue::default()))
                .collect(),
            charts: charts
                .iter()
                .map(|id| ChartView {
                    id: id.to_string(),
                    ..Default::default()
                })
                .collect(),
            tables: tables
                .iter()
                .map(|id| TableView {
                    id: id.to_string(),
                    ..Default::default()
                })
                .collect(),
            interpretations: vec!["kept".into()],
            notices: vec!["kept".into()],
        }
    }

    #[test]
    fn offense_sections_partition_core_ids_and_keep_qualifications() {
        let t = tab(
            &[
                "accuracy",
                "averageReach",
                "aimCentral",
                "timingPerfect",
                "criticalRate",
                "sprintRate",
            ],
            &[
                "attack_outcomes",
                "reach_histogram",
                "aim_contacts",
                "timing_ordinal",
            ],
            &["reach_by_weapon", "aim_samples", "timing_latest"],
        );
        let reach = filter(&t, for_tab(1), 2);
        assert_eq!(reach.metrics.keys().collect::<Vec<_>>(), ["averageReach"]);
        assert_eq!(reach.charts[0].id, "reach_histogram");
        assert_eq!(reach.tables[0].id, "reach_by_weapon");
        assert_eq!(reach.notices, ["kept"]);
        let timing = filter(&t, for_tab(1), 4);
        assert_eq!(timing.charts.len(), 1);
        assert_eq!(timing.tables[0].id, "timing_latest");
        // Index 0 is the whole tab; an out-of-range index never hides data.
        assert_eq!(filter(&t, for_tab(1), 0).charts.len(), 4);
        assert_eq!(filter(&t, for_tab(1), 99).metrics.len(), 6);
    }

    #[test]
    fn every_tab_has_the_reference_sections() {
        let names = |i| for_tab(i).iter().map(|s| s.0).collect::<Vec<_>>();
        assert_eq!(
            names(1),
            [
                "Accuracy",
                "Reach & spacing",
                "Aim",
                "Weapon timing",
                "Critical hits",
                "Sprint hits"
            ]
        );
        assert_eq!(
            names(2),
            [
                "Incoming hits",
                "Shields",
                "Jump resets",
                "Damage & knockback"
            ]
        );
        assert_eq!(names(3), ["Combos", "Dominance", "Exchanges"]);
        assert!(for_tab(0).is_empty());
    }
}
