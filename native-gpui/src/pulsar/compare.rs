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
                Some("Shared measurements, with each recording’s own policy and coverage.".into()),
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
                                    if i == 0 {
                                        "First fight"
                                    } else {
                                        "Second fight"
                                    },
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
        let mut summary =
            ui::card(t)
                .p(px(16.))
                .gap(px(8.))
                .child(ui::section_header(t, "What changed", None));
        for key in ["accuracy", "hits", "averageReach", "longestCombo"] {
            if let Some(m) = data.metrics.iter().find(|m| m.key == key) {
                let delta = super::presentation::comparison_delta(&m.left, &m.right);
                let change = change_direction(&delta);
                let wording = match key {
                    "accuracy" => format!("Confirmed-hit accuracy {}", change_word(change)),
                    "hits" => format!("Confirmed hits {}", change_word(change)),
                    "averageReach" => format!("Average contact reach {}", change_word(change)),
                    _ => format!("Longest combo {}", change_word(change)),
                };
                summary = summary.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(ui::muted(t, wording).flex_1())
                        .child(
                            ui::muted(t, format!("{} {delta}", direction_arrow(change)))
                                .text_color(delta_color(t, key, change)),
                        ),
                );
            }
        }
        page = page.child(summary).child(ui::caption(t, "Changes compare the second fight with the first. Green and red mark accuracy or confirmed-hit changes only; all other changes are neutral."));
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
                    .child(ui::caption(t, "First fight").w(px(120.)))
                    .child(ui::caption(t, "Second fight").w(px(120.)))
                    .child(ui::caption(t, "Change").w(px(130.))),
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
                                .child(ui::muted(t, m.left.definition.label.clone()))
                                .when(
                                    !matches!(
                                        m.key.as_str(),
                                        "accuracy" | "hits" | "attempts" | "longestCombo"
                                    ),
                                    |d| d.child(ui::caption(t, brief_definition(&m.key))),
                                ),
                        )
                        .child(ui::muted(t, tabs::metric_value(&m.left)).w(px(120.)))
                        .child(ui::muted(t, tabs::metric_value(&m.right)).w(px(120.)))
                        .child(
                            ui::caption(t, format!("{} {delta}", direction_arrow(direction)))
                                .text_color(delta_color(t, &m.key, direction))
                                .w(px(130.)),
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

fn change_direction(delta: &str) -> i8 {
    if delta.starts_with('+') {
        1
    } else if delta.starts_with('-') {
        -1
    } else {
        0
    }
}
fn direction_arrow(change: i8) -> &'static str {
    match change {
        1 => "\u{2191}",
        -1 => "\u{2193}",
        _ => "\u{2194}",
    }
}
fn change_word(change: i8) -> &'static str {
    match change {
        1 => "increased",
        -1 => "decreased",
        _ => "held steady or is unavailable",
    }
}
fn delta_color(t: &AuroraTheme, key: &str, change: i8) -> gpui::Hsla {
    if matches!(key, "accuracy" | "hits") {
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
fn brief_definition(key: &str) -> &'static str {
    match key {
        "averageReach" => "Distance at confirmed contact",
        "aimCentral" => "Contacts inside the frozen target region",
        "aimDistance" => "Distance from the ideal contact point",
        "accuracyDenominator" => "Attempts eligible for confirmed-hit accuracy",
        "dominanceShare" => "Share of measured active combat with an advantage",
        "averageCombo" => "Confirmed hits per multi-hit streak",
        _ => "Recorded measurement; hover for its definition",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn displayed_delta_direction_and_groups_are_consistent() {
        assert_eq!(change_direction("+0.1 m"), 1);
        assert_eq!(change_direction("-2"), -1);
        assert_eq!(change_direction("0 pp"), 0);
        assert_eq!(change_direction("Unavailable"), 0);
        assert_eq!(metric_group("accuracy"), "Offense");
        assert_eq!(metric_group("incoming"), "Defense");
        assert_eq!(metric_group("longestCombo"), "Flow");
        assert_eq!(metric_group("damageTaken"), "Defense");
        assert_eq!(metric_group("longestUnanswered"), "Flow");
    }
}
