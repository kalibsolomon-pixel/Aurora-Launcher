//! Shared presentations for pooled analysis and a focused recording.
use super::presentation::ui;
use super::{
    Pulsar,
    charts::{Chart, Kind, Sample, Shape},
    tabs,
};
use crate::{
    controls::{AuroraButton, ButtonSize, ButtonVariant},
    theme::AuroraTheme,
};
use aurora_launcher_lib::pulsar::view::*;
use gpui::{Context, Div, div, prelude::*, px};
use std::collections::BTreeMap;

fn button(id: impl Into<gpui::ElementId>, label: impl Into<gpui::SharedString>) -> AuroraButton {
    AuroraButton::new(id, super::presentation::text(label.into().as_ref()))
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Small)
}

fn value(m: &BTreeMap<String, MetricValue>, key: &str) -> String {
    m.get(key)
        .map(tabs::metric_value)
        .unwrap_or_else(|| "Unavailable".into())
}

fn denominator(m: &BTreeMap<String, MetricValue>, n: &str, d: &str) -> String {
    format!("{} of {}", value(m, n), value(m, d))
}

pub(super) fn stack(
    t: &AuroraTheme,
    id: &str,
    title: &str,
    entries: Vec<(&str, Option<f64>)>,
    note: &str,
) -> Div {
    Chart {
        id: id.into(),
        title: title.into(),
        x_label: "observations".into(),
        y_label: "Count".into(),
        samples: entries
            .into_iter()
            .enumerate()
            .map(|(i, (label, y))| Sample {
                x: i as f64,
                y,
                label: label.into(),
                shape: Shape::Square,
            })
            .collect(),
        kind: Kind::Stacked,
        domain: (0., 1.),
        range: (0., 1.),
        band: None,
        target: None,
        playhead: None,
        bands: vec![],
        incomplete: vec![],
        note: note.into(),
    }
    .render(t)
}

impl Pulsar {
    pub(super) fn disclosure(
        &self,
        _t: &AuroraTheme,
        id: &str,
        label: &str,
        content: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        let open = self.disclosures.contains(id);
        let key = id.to_owned();
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .min_w(px(0.))
            .child(
                div().flex().child(
                    button(format!("disclosure-{id}"), label.to_owned())
                        .icon(if open { "expand_less" } else { "expand_more" })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.disclosures.remove(&key) {
                                this.disclosures.insert(key.clone());
                            }
                            cx.notify();
                        })),
                ),
            )
            .when(open, |d| d.child(content))
    }

    pub(super) fn overview_lead(
        &self,
        t: &AuroraTheme,
        tab: &TabView,
        cx: &mut Context<Self>,
    ) -> Div {
        let primary = tab
            .metrics
            .iter()
            .filter(|(k, _)| matches!(k.as_str(), "accuracy" | "averageReach" | "longestCombo"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(self.metrics_panel(t, "Recent performance", &primary, cx));
        let key = ["accuracy", "reach", "critical", "interval"][self.trend_metric];
        let mut selectors = div().flex().flex_wrap().gap(px(2.));
        for (i, label) in ["Accuracy", "Reach", "Critical hits", "Hit interval"]
            .into_iter()
            .enumerate()
        {
            selectors = selectors.child(
                button(("overview-trend", i), label)
                    .selected(self.trend_metric == i)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.trend_metric = i;
                        cx.notify();
                    })),
            );
        }

        if let Some(a) = &self.analysis {
            if let Some(previous) = &a.previous_period {
                let comparisons = ["accuracy", "averageReach", "longestCombo"]
                    .into_iter()
                    .filter_map(|k| {
                        let now = tab.metrics.get(k)?;
                        let before = previous.metrics.get(k)?;
                        Some(format!(
                            "{}: {}",
                            now.definition.label,
                            super::presentation::comparison_delta(before, now)
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join(" \u{b7} ");
                body = body.child(ui::caption(
                    t,
                    format!(
                        "Last {} vs previous {} recordings{} \u{b7} {comparisons}",
                        a.coverage.recordings,
                        a.previous_recordings,
                        if a.previous_recordings < 5 {
                            " (small baseline)"
                        } else {
                            ""
                        }
                    ),
                ));
                if let (Some(now), Some(before)) = (
                    tab.metrics.get("accuracy").and_then(|m| m.value),
                    previous.metrics.get("accuracy").and_then(|m| m.value),
                ) {
                    body = body.child(ui::muted(t, if now > before { "Your confirmed-hit accuracy improved over the preceding recordings." } else if now < before { "Your confirmed-hit accuracy fell; inspect the misses in your recent fights." } else { "Your confirmed-hit accuracy held steady over the preceding recordings." }));
                }
            } else {
                body = body.child(ui::caption(t, "No earlier period in this scope. Review individual points to find your next focus."));
            }
        }
        body = body.child(selectors);
        if let Some(c) = tab
            .charts
            .iter()
            .find(|c| c.id.contains("trend") && c.id.to_lowercase().contains(key))
        {
            let mut chart = tabs::chart(c);
            chart.x_label = "Recording date (UTC)".into();
            chart.note =
                "Hollow points show genuine 0% accuracy. Missing or ineligible measurements leave gaps."
                    .into();
            body = body.child(chart.render_options(
                t,
                None,
                super::charts::Options {
                    preferences: self.preferences,
                    ..Default::default()
                },
            ));
        }

        body
    }

    pub(super) fn metrics_panel(
        &self,
        t: &AuroraTheme,
        title: &str,
        m: &BTreeMap<String, MetricValue>,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut group = div().flex().flex_wrap().gap(px(16.));
        for metric in m.values() {
            let key = metric.definition.key.clone();
            let metric_tip = format!("{}: {}", metric.definition.label, metric.definition.help);
            let metric_theme = t.clone();
            group = group.child(
                div()
                    .id(format!("metric-readout-{key}"))
                    .tooltip(move |_, cx| {
                        cx.new(|_| super::charts::Readout {
                            text: metric_tip.clone(),
                            theme: metric_theme.clone(),
                        })
                        .into()
                    })
                    .w(px(if m.len() == 4 && self.width >= 950. {
                        170.
                    } else if self.width < 950. {
                        180.
                    } else {
                        210.
                    }))
                    .when(m.len() == 5, |d| d.w(gpui::relative(0.30)).min_w(px(130.)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .h(px(if m.len() == 5 { 32. } else { 20. }))
                            .child(
                                ui::caption(
                                    t,
                                    match key.as_str() {
                                        "incoming" => "Incoming hits".into(),
                                        "incomingCritical" | "criticalOnLocal"
                                        | "opponentCrits" => "Incoming crits".into(),
                                        "thirdPartyIncoming" => "Third-party hits".into(),
                                        "unattributedIncoming" => "Unattributed hits".into(),
                                        "unexplainedHealthDrops" => "Health-drop gaps".into(),
                                        "emptyMisses" => "Aimed swings without a hit".into(),
                                        "eligibleMisses" => "Canonical miss classifications".into(),
                                        _ => display_label(&metric.definition.label),
                                    },
                                )
                                .flex_1()
                                .min_w(px(0.)),
                            )
                            .child(
                                button(format!("metric-info-{key}"), "")
                                    .icon("info")
                                    .accessibility_label(format!(
                                        "About {}",
                                        metric.definition.label
                                    ))
                                    .selected(self.metric_info.as_ref() == Some(&key))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.metric_info =
                                            if this.metric_info.as_ref() == Some(&key) {
                                                None
                                            } else {
                                                Some(key.clone())
                                            };
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(ui::TEXT_SECTION))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(t.foreground)
                            .child(
                                if metric.definition.key == "unexplainedHealthDrops"
                                    && metric.value.is_none()
                                {
                                    "\u{2014}".into()
                                } else {
                                    tabs::metric_value(metric)
                                },
                            ),
                    ),
            );
        }

        if m.get("unexplainedHealthDrops")
            .is_some_and(|v| v.value.is_none())
        {
            group = group.child(
                ui::caption(
                    t,
                    "Health-drop attribution was not recorded; the dash means unavailable.",
                )
                .w_full(),
            );
        }
        let mut panel = ui::card(t)
            .p(px(16.))
            .gap(px(12.))
            .when(title == "Recent performance", |d| d.gap(px(8.)).p(px(16.)))
            .child(ui::section_header(t, title.to_owned(), None))
            .child(group);
        if title == "Performance" || title == "Pooled performance" {
            let definitions = [
                (
                    "accuracy",
                    "Accuracy: confirmed hits out of eligible attempts.",
                ),
                ("averageReach", "Reach: distance at confirmed contact."),
                (
                    "longestCombo",
                    "Combo: consecutive hits before retaliation or a break.",
                ),
            ]
            .into_iter()
            .filter(|(key, _)| m.contains_key(*key))
            .map(|(_, text)| text)
            .collect::<Vec<_>>();
            if !definitions.is_empty() {
                panel = panel.child(ui::caption(t, definitions.join(" ")));
            }
        }
        if let Some(metric) = self.metric_info.as_ref().and_then(|k| m.get(k)) {
            panel = panel.child(explanation(
                t,
                format!(
                    "{} · {} | {}",
                    metric.definition.label, metric.definition.unit, metric.definition.confidence
                ),
                Some(
                    format!(
                        "{} {}",
                        metric.definition.help,
                        metric.reason.as_deref().unwrap_or("")
                    )
                    .into(),
                ),
            ));
        }

        panel
    }

    pub(super) fn table_view(
        &self,
        t: &AuroraTheme,
        data: &TableView,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let expanded = self.table_expanded.contains(&data.id);
        let mut projection = data.clone();
        if !expanded {
            projection.rows.truncate(8);
        }

        let id = data.id.clone();
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .min_w(px(0.))
            .child(tabs::table(t, &projection, narrow));
        if data.rows.len() > 8 {
            body = body.child(
                div().flex().child(
                    button(
                        format!("table-toggle-{id}"),
                        if expanded {
                            "Collapse table".into()
                        } else {
                            format!("Show all {} rows", data.rows.len())
                        },
                    )
                    .icon(if expanded {
                        "expand_less"
                    } else {
                        "expand_more"
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.table_expanded.remove(&id) {
                            this.table_expanded.insert(id.clone());
                        }

                        cx.notify();
                    })),
                ),
            );
        }

        body
    }

    pub(super) fn overview_groups(
        &self,
        t: &AuroraTheme,
        m: &BTreeMap<String, MetricValue>,
        cx: &mut Context<Self>,
    ) -> Div {
        let groups = [
            (
                "Accuracy",
                value(m, "accuracy"),
                format!(
                    "{} hits from eligible attempts",
                    denominator(m, "hits", "accuracyDenominator")
                ),
            ),
            (
                "Confirmed hits",
                value(m, "hits"),
                format!("{} attempts | confirmed evidence", value(m, "attempts")),
            ),
            (
                "Average eligible reach",
                value(m, "averageReach"),
                format!("{} comparable contact hits", value(m, "reachEligible")),
            ),
            (
                "Within target window",
                value(m, "reachWithin"),
                format!(
                    "{} contacts within the target window",
                    denominator(m, "reachWithinCount", "reachEligible")
                ),
            ),
            (
                "Critical conditions",
                value(m, "criticalRate"),
                format!(
                    "{} confirmed hits with critical conditions",
                    denominator(m, "criticalConditions", "criticalDenominator")
                ),
            ),
            (
                "Combos",
                format!(
                    "{} average · {} longest",
                    value(m, "averageCombo"),
                    value(m, "longestCombo")
                ),
                format!("{} multi-hit streaks", value(m, "comboCount")),
            ),
            (
                "Dominance shares",
                format!(
                    "{} yours · {} opponent",
                    value(m, "dominanceShare"),
                    value(m, "opponentDominanceShare")
                ),
                format!(
                    "{} observed active time; inactive/incomplete excluded",
                    value(m, "dominanceActiveSeconds")
                ),
            ),
            (
                "Hit interval",
                format!(
                    "{} average · {} standard deviation",
                    value(m, "averageInterval"),
                    value(m, "intervalDeviation")
                ),
                format!("{} eligible intervals", value(m, "intervalCount")),
            ),
            (
                "Recording coverage",
                format!(
                    "{} in scope of {} total",
                    self.analysis
                        .as_ref()
                        .map(|a| a.coverage.recordings)
                        .unwrap_or(0),
                    self.library.total
                ),
                format!(
                    "{} timelines | {} with context",
                    self.analysis
                        .as_ref()
                        .map(|a| a.coverage.timeline_recordings)
                        .unwrap_or(0),
                    self.analysis
                        .as_ref()
                        .map(|a| a.coverage.context_recordings)
                        .unwrap_or(0)
                ),
            ),
        ];
        ui::card(t)
            .child(ui::section_header(t, "Pooled measurements", None))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(24.))
                    .children(
                        groups
                            .into_iter()
                            .enumerate()
                            .map(|(i, (label, v, caption))| {
                                let key = [
                                    "accuracy",
                                    "hits",
                                    "averageReach",
                                    "reachWithin",
                                    "criticalRate",
                                    "averageCombo",
                                    "dominanceShare",
                                    "averageInterval",
                                    "recordings",
                                ][i];
                                self.measurement_group(t, m, key, label, v, caption, cx)
                            }),
                    ),
            )
    }

    pub(super) fn latest_denominators(
        &self,
        t: &AuroraTheme,
        s: &FightSummary,
        cx: &mut Context<Self>,
    ) -> Div {
        let m = &s.metrics;
        div().flex().flex_wrap().gap(px(24.)).children(
            [
                (
                    "Recording",
                    format!(
                        "{:.1} s · {} attempts",
                        s.duration_nanos as f64 / 1e9,
                        value(m, "attempts")
                    ),
                ),
                (
                    "Accuracy",
                    format!(
                        "{} · {} hits",
                        value(m, "accuracy"),
                        denominator(m, "hits", "accuracyDenominator")
                    ),
                ),
                (
                    "Within reach window",
                    format!(
                        "{} · {} contacts",
                        value(m, "reachWithin"),
                        denominator(m, "reachWithinCount", "reachEligible")
                    ),
                ),
                (
                    "Longest combo and combo count",
                    format!(
                        "{} hits · {} combos",
                        value(m, "longestCombo"),
                        value(m, "comboCount")
                    ),
                ),
                (
                    "Local dominance",
                    format!(
                        "{} · {}",
                        value(m, "dominanceShare"),
                        m.get("dominanceShare")
                            .map(|m| if m.value.is_some() {
                                "measured".to_owned()
                            } else {
                                "unavailable".to_owned()
                            })
                            .unwrap_or_else(|| "Unavailable".into())
                    ),
                ),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (label, v))| {
                self.measurement_group(
                    t,
                    m,
                    [
                        "duration",
                        "accuracy",
                        "reachWithin",
                        "longestCombo",
                        "dominanceShare",
                    ][i],
                    label,
                    v,
                    String::new(),
                    cx,
                )
            }),
        )
    }

    fn measurement_group(
        &self,
        t: &AuroraTheme,
        m: &BTreeMap<String, MetricValue>,
        key: &str,
        label: &str,
        value: String,
        caption: String,
        cx: &mut Context<Self>,
    ) -> Div {
        let selected = self.metric_info.as_deref() == Some(key);
        let id = key.to_owned();
        let mut group = div()
            .min_w(px(210.))
            .flex_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(ui::caption(t, label.to_owned()).flex_1().min_w(px(0.)))
                    .child(
                        button(format!("group-info-{key}"), "")
                            .icon("info")
                            .accessibility_label(format!("About {label}"))
                            .selected(selected)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.metric_info = if this.metric_info.as_ref() == Some(&id) {
                                    None
                                } else {
                                    Some(id.clone())
                                };
                                cx.notify();
                            })),
                    ),
            )
            .child(ui::muted(t, value));
        if !caption.is_empty() {
            group = group.child(ui::caption(t, caption));
        }

        if selected {
            if let Some(metric) = m.get(key) {
                group = group.child(ui::caption(
                    t,
                    format!(
                        "{} · {} | {} {}",
                        metric.definition.unit,
                        metric.definition.confidence,
                        metric.definition.help,
                        metric.reason.as_deref().unwrap_or("")
                    ),
                ));
            }
        }

        group
    }

    pub(super) fn compositions(&self, t: &AuroraTheme, tab: &TabView, index: usize) -> Div {
        let mut body = div().flex().flex_col().gap(px(24.));
        let section = if index < 4 { self.sections[index] } else { 0 };
        let get = |key: &str| {
            tab.metrics
                .get(key)
                .or_else(|| {
                    self.active_detail()
                        .and_then(|d| d.offense.metrics.get(key))
                })
                .or_else(|| {
                    self.analysis
                        .as_ref()
                        .and_then(|a| a.offense.metrics.get(key))
                })
                .and_then(|m| m.value)
        };
        if index == 0 && self.detail_request.is_some() {
            if let Some(d) = self.active_detail() {
                if let Some(c) = d.offense.charts.iter().find(|c| c.id == "attack_outcomes") {
                    body = body.child(tabs::chart(c).render(t));
                }
            }
        }

        if index == 1 && matches!(section, 0 | 3) {
            let table = tab.tables.iter().find(|t| t.id == "aim_samples");
            let entries = ["IDEAL", "GOOD", "PERIPHERAL", "POOR"]
                .into_iter()
                .map(|k| {
                    let count = table.map(|t| {
                        t.rows
                            .iter()
                            .filter(|r| {
                                r.cells.get("hit").is_some_and(|s| s == "true")
                                    && r.cells.get("grade").is_some_and(|s| s == k)
                            })
                            .count() as f64
                    });
                    (k, count)
                })
                .collect();
            body=body.child(stack(t,"aim-grade-distribution","Aim grade distribution",entries,"Confirmed measured contacts | grades use the recording's frozen ideal region. Unavailable grades remain unavailable."));
        }

        if index == 2 && matches!(section, 0 | 1) {
            body=body.child(stack(t,"incoming-attribution","Incoming attribution",vec![("Tracked opponent",get("incoming")),("Unattributed",get("unattributedIncoming")),("Third party",get("thirdPartyIncoming"))],"Counts are observed incoming events; third-party and unknown damage do not extend opponent combos."));
        }

        if index == 2 && matches!(section, 0 | 2) {
            let block_only = get("shieldBlocks")
                .zip(get("shieldDisables"))
                .map(|(b, d)| (b - d).max(0.));
            body=body.child(stack(t,"shield-offense","Attack outcomes and shield interactions",vec![
                ("Confirmed hit",get("hits")),("Block only",block_only),("Shield disable",get("shieldDisables")),
                ("Estimated block",get("estimatedShieldBlocks")),("Unknown result",get("uncertain").zip(get("estimatedShieldBlocks")).map(|(u,e)|(u-e).max(0.))),
                ("Empty swing",get("emptyMisses"))],"Block only excludes disables, so a disable is counted once. Estimated blocks remain uncertain."));
            body = body.child(stack(
                t,
                "shield-block-disable",
                "Confirmed shield interactions",
                vec![
                    ("Block only", block_only),
                    ("Disable", get("shieldDisables")),
                ],
                "Block only = confirmed blocks minus disables; estimated blocks are excluded.",
            ));
            body = body.child(stack(
                t,
                "local-shield-outcomes",
                "Your shield",
                vec![
                    ("Confirmed blocks", get("localShieldBlocks")),
                    ("Disabled", get("localShieldDisabled")),
                ],
                "Subject is your shield. No prevented-damage estimate is inferred.",
            ));
        }

        body
    }

    pub(super) fn scope_request(&self) -> Option<DetailRequest> {
        if self.detail_request.is_some() {
            self.detail_request.clone()
        } else {
            self.focus_request.clone()
        }
    }

    pub(super) fn focus_scopes(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let Some(detail) = self.active_detail() else {
            return div();
        };
        if !detail.summary.manual && detail.exchanges.is_empty() {
            return div();
        }
        let request = self.scope_request();
        let mut list = ui::list_card(t).child(ui::row(t, false).py(px(6.)).flex_wrap()
            .child(ui::caption(t, "Results belong to individual exchanges; the full session has no single opponent.").flex_1())
            .child(button("scope-entire-session", "Entire session").selected(request.as_ref().is_some_and(|r| r.participant.is_none() && r.exchange_id.is_none())).on_click(cx.listener(|this, _, _, cx| {
                if let Some(mut r) = this.scope_request() { r.participant = None; r.exchange_id = None; this.select_scope(r, cx); }
            }))));
        for (i, participant) in detail.participants.iter().enumerate() {
            let id = participant.id.clone();
            list = list.child(
                ui::row(t, false)
                    .py(px(2.))
                    .child(ui::caption(t, "Participant").w(px(80.)))
                    .child(
                        button(
                            ("scope-participant", i),
                            format!("{} (first exchange)", participant.label),
                        )
                        .selected(request.as_ref().is_some_and(|r| {
                            r.participant.as_ref() == Some(&id) && r.exchange_id.is_none()
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(mut r) = this.scope_request() {
                                r.participant = Some(id.clone());
                                r.exchange_id = None;
                                this.select_scope(r, cx);
                            }
                        })),
                    ),
            );
        }
        for (i, exchange) in detail.exchanges.iter().enumerate() {
            let id = exchange.id.clone();
            let keyboard_id = id.clone();
            let (label, tone) = super::presentation::resolution(&exchange.resolution);
            list = list.child(
                ui::row(t, i + 1 == detail.exchanges.len())
                    .py(px(7.))
                    .id(("scope-exchange", i))
                    .role(gpui::Role::Button)
                    .tab_index(0)
                    .cursor_pointer()
                    .aria_label(format!(
                        "Select exchange {} with {}, {}",
                        i + 1,
                        exchange.opponent,
                        label
                    ))
                    .when(detail.selected_exchange.as_ref() == Some(&id), |d| {
                        d.bg(t.accent_soft)
                    })
                    .focus_visible(|d| d.border_1().border_color(t.ring))
                    .child(ui::caption(t, format!("Exchange {}", i + 1)).w(px(80.)))
                    .child(ui::muted(t, exchange.opponent.clone()).flex_1())
                    .child(ui::badge(t, label.to_owned(), tone))
                    .child(
                        ui::caption(t, super::page::duration_text(exchange.duration_nanos))
                            .w(px(70.)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(mut r) = this.scope_request() {
                            r.participant = None;
                            r.exchange_id = Some(id.clone());
                            this.select_scope(r, cx);
                        }
                    }))
                    .on_key_down(cx.listener(move |this, e: &gpui::KeyDownEvent, _, cx| {
                        if matches!(e.keystroke.key.as_str(), "enter" | "space" | " ") {
                            if let Some(mut r) = this.scope_request() {
                                r.participant = None;
                                r.exchange_id = Some(keyboard_id.clone());
                                this.select_scope(r, cx);
                            }
                            cx.stop_propagation();
                        }
                    })),
            );
        }
        div().child(list)
    }

    pub(super) fn shortcut_guide(&self, t: &AuroraTheme) -> Div {
        ui::card(t).child(ui::section_header(t,"Shortcut guide",None)).children([
   ("Open dashboard in game","Uses Aurora's feature key system, including Ctrl, Shift and Alt. Binding: Change in game."),
   ("1-8 / 1-6","Switch library / fight-detail tabs. Left and Right wrap the focused tab strip."),
   ("Tab / Shift+Tab","Move focus. Enter or Space activates a focused control or chart mark."),
   ("Arrow keys","Change history selection and adjust focused timeline controls."),
   ("Page Up / Page Down | Home / End","Scroll the page except while typing. Tab scroll positions are remembered."),
   ("Escape","Dismiss the current dialog, then return to the previous screen."),
   ("Space | comma / period","Play or pause; step to the previous / next recorded event. Reduced motion uses event stepping.")
  ].into_iter().map(|(label,help)|explanation(t,label,Some(help.into()))))
    }
}

pub(super) fn display_label(label: &str) -> String {
    if label.contains(' ') || label.contains('_') || label.chars().all(|c| !c.is_lowercase()) {
        return super::presentation::text(label);
    }
    let mut out = String::new();
    for (i, c) in label.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(if i == 0 { c.to_ascii_uppercase() } else { c });
    }
    out
}

pub(super) fn explanation(
    t: &AuroraTheme,
    title: impl Into<gpui::SharedString>,
    detail: Option<gpui::SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w(px(0.))
        .gap(px(4.))
        .child(ui::section_header(t, title, None))
        .when_some(detail, |d, text| d.child(ui::caption(t, text)))
}
