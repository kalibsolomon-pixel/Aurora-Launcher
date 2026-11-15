use super::presentation::ui;
use super::{Pulsar, Reply, tabs};
use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, ButtonSize, ButtonVariant},
    theme::AuroraTheme,
};
use aurora_launcher_lib::pulsar::view::*;
use gpui::{Context, Div, div, prelude::*, px};

impl Pulsar {
    pub(super) fn open_compare(&mut self, cx: &mut Context<Self>) {
        if self.compare_keys.len() != 2 {
            return;
        }
        self.pause();
        self.compare_revision = self.compare_revision.wrapping_add(1);
        self.compare_open = true;
        self.comparison = None;
        self.error = None;
        let request = CompareRequest {
            left: DetailRequest {
                key: self.compare_keys[0].clone(),
                ..Default::default()
            },
            right: DetailRequest {
                key: self.compare_keys[1].clone(),
                ..Default::default()
            },
        };
        if let Some(host) = self.host.clone() {
            let tx = self.tx.clone();
            let rev = self.compare_revision;
            self.runtime.spawn(async move {
                let r = guarded(api::pulsar_compare(host, request)).await;
                let _ = tx.send(Reply::Compare(rev, r)).await;
            });
        }
        cx.notify();
    }
    pub(super) fn compare_body(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                div().flex().child(
                    AuroraButton::new("compare-back", "Back to history")
                        .icon("arrow_back")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.compare_open = false;
                            this.top_tab = 4;
                            cx.notify();
                        })),
                ),
            )
            .child(ui::page_header(
                t,
                "Compare fights",
                Some("Each fight is measured with the settings it was recorded with.".into()),
            ));
        if let Some(error) = &self.error {
            page = page.child(ui::muted(t, error.clone()));
        }
        let Some(data) = &self.comparison else {
            return page.child(ui::muted(
                t,
                if self.error.is_some() {
                    "Comparison unavailable. Return to history and try again."
                } else {
                    "Loading comparison..."
                },
            ));
        };
        let (left_name, right_name) = side_names(&data.left, &data.right);
        // Sides keep selection order; their labels follow the recorded times.
        let left_earlier = data.left.start_epoch_millis <= data.right.start_epoch_millis;
        page = page.child(
            ui::card(t).child(
                div().flex().flex_wrap().gap(px(20.)).children(
                    [&data.left, &data.right]
                        .into_iter()
                        .enumerate()
                        .map(|(i, f)| {
                            div()
                                .flex_1()
                                .min_w(px(180.))
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(ui::caption(
                                    t,
                                    format!(
                                        "{} fight \u{b7} {}",
                                        if (i == 0) == left_earlier {
                                            "Earlier"
                                        } else {
                                            "Later"
                                        },
                                        if i == 0 { &left_name } else { &right_name }
                                    ),
                                ))
                                .child(ui::section_header(t, f.opponent.clone(), None))
                                .child(ui::caption(t, super::page::utc_time(f.start_epoch_millis)))
                                .child(ui::caption(
                                    t,
                                    format!(
                                        "{:.1} s · {}",
                                        f.duration_nanos as f64 / 1e9,
                                        f.provenance.instance_name
                                    ),
                                ))
                                .child(div().flex().child(ui::badge(
                                    t,
                                    f.result_label.clone(),
                                    super::presentation::outcome(f),
                                )))
                        }),
                ),
            ),
        );
        // Both values sit beside every change; one convention: arrow plus magnitude.
        let change_row = |label: String, left: String, right: String, delta: &str, color| {
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(2.))
                .child(ui::muted(t, label).flex_1().min_w(px(0.)))
                .child(ui::numeric(ui::muted(t, left)).w(px(VALUE_W)))
                .child(ui::numeric(ui::muted(t, right)).w(px(VALUE_W)))
                .child(
                    ui::numeric(ui::muted(t, super::presentation::delta_display(delta)))
                        .text_color(color)
                        .w(px(CHANGE_W)),
                )
        };
        let mut rows = Vec::new();
        for key in ["accuracy", "hits", "averageReach", "longestCombo"] {
            if let Some(m) = data.metrics.iter().find(|m| m.key == key) {
                let delta = super::presentation::comparison_delta(&m.left, &m.right);
                let label = match key {
                    "accuracy" => "Confirmed-hit accuracy",
                    "hits" => "Confirmed hits",
                    "averageReach" => "Average contact reach",
                    _ => "Longest combo",
                };
                rows.push((
                    key,
                    label.to_owned(),
                    tabs::metric_value(&m.left),
                    tabs::metric_value(&m.right),
                    delta,
                ));
            }
        }
        let rate = |f: &FightSummary| {
            hit_rate(f)
                .map(|v| format!("{v:.2} hits/s"))
                .unwrap_or_else(|| "Unavailable".into())
        };
        let rate_delta = match (hit_rate(&data.left), hit_rate(&data.right)) {
            (Some(a), Some(b)) => {
                let n = (b * 100.).round() / 100. - (a * 100.).round() / 100.;
                if n.abs() < 0.005 {
                    "0 hits/s".to_owned()
                } else {
                    format!("{n:+.2} hits/s")
                }
            }
            _ => "Unavailable".into(),
        };
        rows.insert(
            2,
            (
                "rate",
                "Confirmed hits per second".into(),
                rate(&data.left),
                rate(&data.right),
                rate_delta,
            ),
        );
        // Lead with the change that matters most: accuracy, else the hit rate.
        let lead = rows
            .iter()
            .find(|r| r.0 == "accuracy" && change_direction(&r.4) != 0)
            .or_else(|| rows.iter().find(|r| r.0 == "rate"))
            .map(|r| {
                format!(
                    "Main change: {} {} ({} at {left_name}, {} at {right_name}).",
                    r.1.to_lowercase(),
                    super::presentation::delta_display(&r.4),
                    r.2,
                    r.3
                )
            });
        let mut summary = ui::card(t)
            .p(px(16.))
            .gap(px(8.))
            .child(ui::section_header(t, "What changed", None))
            .when_some(lead, |d, lead| {
                d.child(
                    ui::muted(t, lead)
                        .text_color(t.foreground)
                        .font_weight(gpui::FontWeight::MEDIUM),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(ui::caption(t, "Measurement").flex_1())
                    .child(ui::numeric(ui::caption(t, left_name.clone())).w(px(VALUE_W)))
                    .child(ui::numeric(ui::caption(t, right_name.clone())).w(px(VALUE_W)))
                    .child(ui::numeric(ui::caption(t, "Change")).w(px(CHANGE_W))),
            );
        for (key, label, left, right, delta) in &rows {
            let color = delta_color(t, key, change_direction(delta));
            summary = summary.child(change_row(
                label.clone(),
                left.clone(),
                right.clone(),
                delta,
                color,
            ));
        }
        summary = summary.child(ui::caption(
            t,
            format!(
                "Change is the {right_name} fight minus the {left_name} fight. Counts grow with fight length; hits per second allows for it."
            ),
        ));
        page = page.child(summary);
        for group in ["Offense", "Defense", "Flow"] {
            let metrics = data
                .metrics
                .iter()
                .filter(|m| metric_group(&m.key) == group)
                .collect::<Vec<_>>();
            if metrics.is_empty() {
                continue;
            }
            let mut rows = ui::list_card(t).min_w(px(700.)).w_full().child(
                ui::row(t, false)
                    .py(px(7.))
                    .child(ui::caption(t, "Measurement").flex_1().min_w(px(0.)))
                    .child(ui::numeric(ui::caption(t, left_name.clone())).w(px(VALUE_W)))
                    .child(ui::numeric(ui::caption(t, right_name.clone())).w(px(VALUE_W)))
                    .child(ui::numeric(ui::caption(t, "Change")).w(px(CHANGE_W))),
            );
            for (i, m) in metrics.iter().enumerate() {
                let delta = super::presentation::comparison_delta(&m.left, &m.right);
                let direction = change_direction(&delta);
                let tip = m.left.definition.help.clone();
                let theme = t.clone();
                rows = rows.child(
                    ui::row(t, i + 1 == metrics.len())
                        .py(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .id(format!("compare-definition-{}", m.key))
                                .tooltip(move |_, cx| {
                                    cx.new(|_| super::charts::Readout {
                                        text: tip.clone(),
                                        theme: theme.clone(),
                                    })
                                    .into()
                                })
                                .child(ui::muted(t, player_label(&m.key, &m.left.definition.label)))
                                .when(
                                    !matches!(
                                        m.key.as_str(),
                                        "accuracy" | "hits" | "attempts" | "longestCombo"
                                    ),
                                    |d| d.child(ui::caption(t, brief_definition(&m.key))),
                                ),
                        )
                        .child(
                            ui::numeric(ui::muted(t, tabs::metric_value(&m.left))).w(px(VALUE_W)),
                        )
                        .child(
                            ui::numeric(ui::muted(t, tabs::metric_value(&m.right))).w(px(VALUE_W)),
                        )
                        .child(
                            ui::numeric(ui::muted(t, super::presentation::delta_display(&delta)))
                                .text_color(delta_color(t, &m.key, direction))
                                .w(px(CHANGE_W)),
                        ),
                );
            }
            page = page
                .child(ui::section_header(t, group, None))
                .when(self.width < 1000., |d| {
                    d.child(ui::caption(
                        t,
                        "Scroll horizontally to compare every column.",
                    ))
                })
                .child(
                    div()
                        .id(format!("compare-table-{group}"))
                        .w_full()
                        .min_w(px(0.))
                        .overflow_x_scroll()
                        .child(rows),
                );
        }
        for n in &data.notices {
            page = page.child(ui::caption(t, n.clone()));
        }
        page
    }
}

fn hit_rate(f: &FightSummary) -> Option<f64> {
    let hits = f.metrics.get("hits")?.value.filter(|v| v.is_finite())?;
    (f.duration_nanos > 0).then(|| hits / (f.duration_nanos as f64 / 1e9))
}
fn change_direction(delta: &str) -> i8 {
    if delta.starts_with('+') {
        1
    } else if delta.starts_with('-') {
        -1
    } else {
        0
    }
}
const VALUE_W: f32 = 120.;
const CHANGE_W: f32 = 110.;

/// Short side labels: the time alone on the same day, otherwise date and time.
fn side_names(left: &FightSummary, right: &FightSummary) -> (String, String) {
    let (a, b) = (
        super::page::utc_time(left.start_epoch_millis),
        super::page::utc_time(right.start_epoch_millis),
    );
    if a.get(..10) == b.get(..10) && a != b {
        (
            super::page::clock_time(left.start_epoch_millis),
            super::page::clock_time(right.start_epoch_millis),
        )
    } else {
        (
            super::presentation::short_stamp(&a),
            super::presentation::short_stamp(&b),
        )
    }
}
fn delta_color(t: &AuroraTheme, key: &str, change: i8) -> gpui::Hsla {
    if key == "accuracy" {
        match change {
            1 => t.success,
            -1 => t.danger,
            _ => t.muted_foreground,
        }
    } else {
        t.muted_foreground
    }
}
fn metric_group(key: &str) -> &'static str {
    if matches!(
        key,
        "longestUnanswered" | "momentum" | "fights" | "duration"
    ) || key.to_ascii_lowercase().contains("combo")
        || key.contains("dominance")
        || key.contains("Active")
        || key.contains("Duration")
    {
        "Flow"
    } else if matches!(
        key,
        "damageTaken" | "opponentCrits" | "unexplainedHealthDrops"
    ) || key.contains("incoming")
        || key.contains("Incoming")
        || key.contains("localShield")
        || key.contains("health")
        || key.contains("knockback")
        || key.contains("jump")
    {
        "Defense"
    } else {
        "Offense"
    }
}
/// Player names for measurements whose recorded labels describe capture mechanics.
fn player_label(key: &str, recorded: &str) -> String {
    match key {
        "incoming" => "Hits from your opponent",
        "nonMeleeDamage" => "Other damage",
        "jumpCandidates" => "Well-timed jumps",
        "attacks" | "attempts" => "Attack attempts",
        _ => recorded,
    }
    .into()
}
fn brief_definition(key: &str) -> &'static str {
    match key {
        "averageReach" => "Distance at confirmed contact",
        "aimCentral" => "Contacts inside the ideal and good aim regions",
        "aimDistance" => "Distance from the ideal contact point",
        "accuracyDenominator" => "Attempts eligible for confirmed-hit accuracy",
        "dominanceShare" => "Share of measured active combat with an advantage",
        "averageCombo" => "Average hits per combo",
        "emptyMisses" => "Attacks that neither hit nor met a shield",
        "maxReach" => "Farthest confirmed contact",
        "criticalRate" => "Confirmed hits with critical conditions",
        "sprintRate" => "Confirmed hits while sprinting",
        "averageInterval" => "Time between eligible attacks",
        "intervalDeviation" => "Spread of attack intervals",
        "jumpCandidates" => "Jumps close in time to a hit on you",
        "incoming" => "Opponent direct melee only",
        "nonMeleeDamage" => "Damage other than opponent direct melee",
        "attacksIntoShield" => "Attempts against a raised shield",
        "shieldBlocks" => "Confirmed block evidence",
        "shieldDisables" => "Shields disabled by your attacks",
        "hitsAfterDisable" => "Confirmed hits after a shield disable",
        "localShieldBlocks" => "Confirmed blocks by your shield",
        "serverCrits" => "Server critical animation on the opponent",
        "comboCount" => "Combos of 2+ hits",
        "comboHits" => "Confirmed hits inside combos",
        "longestOpponentCombo" => "Opponent's longest run of hits on you",
        "longestUnanswered" => "Hits before confirmed retaliation",
        "reachWithin" => "Hits inside your reach target window",
        "reachDeviation" => "Spread of confirmed contact distances",
        "reachError" => "Average distance from your reach target",
        "timingPerfect" => "Attacks timed to the weapon cooldown",
        "damageTaken" => "Observed health lost",
        "knockback" => "Incoming hits with velocity evidence",
        "fights" => "Number of recorded fights",
        "momentum" | "dominance" => "Score from the fight timeline; may be unavailable",
        _ => "Recorded measurement",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hit_rates_account_for_duration_and_raw_counts_stay_neutral() {
        let mut fight = FightSummary {
            duration_nanos: 25_000_000_000,
            ..Default::default()
        };
        fight.metrics.insert(
            "hits".into(),
            MetricValue {
                value: Some(10.),
                ..Default::default()
            },
        );
        assert_eq!(hit_rate(&fight), Some(0.4));
        fight.duration_nanos = 0;
        assert_eq!(hit_rate(&fight), None);
    }
    #[test]
    fn displayed_delta_direction_and_groups_are_consistent() {
        assert_eq!(change_direction("+0.1 m"), 1);
        assert_eq!(change_direction("-2"), -1);
        assert_eq!(change_direction("0 pp"), 0);
        assert_eq!(change_direction("Unavailable"), 0);
        let mut a = FightSummary {
            start_epoch_millis: 1_792_917_180_000,
            ..Default::default()
        };
        let mut b = a.clone();
        b.start_epoch_millis += 7_200_000;
        let (l, r) = side_names(&a, &b);
        assert_ne!(l, r);
        assert!(!l.contains(' '), "{l}");
        a.start_epoch_millis -= 86_400_000;
        assert!(side_names(&a, &b).0.contains(' '));
        assert_eq!(metric_group("accuracy"), "Offense");
        assert_eq!(metric_group("incoming"), "Defense");
        assert_eq!(metric_group("longestCombo"), "Flow");
        assert_eq!(metric_group("damageTaken"), "Defense");
        assert_eq!(metric_group("longestUnanswered"), "Flow");
    }
}
