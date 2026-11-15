//! Shared presentations for pooled analysis and a focused recording.
use super::{
    Pulsar,
    charts::{Chart, Kind, Sample, Shape},
    tabs,
};
use crate::{
    controls::{AuroraButton, ButtonSize, ButtonVariant},
    theme::AuroraTheme,
    ui,
};
use aurora_launcher_lib::pulsar::view::*;
use gpui::{Context, Div, div, prelude::*, px};
use std::collections::BTreeMap;
fn button(id: impl Into<gpui::ElementId>, label: impl Into<gpui::SharedString>) -> AuroraButton {
    AuroraButton::new(id, label)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Small)
}
fn value(m: &BTreeMap<String, MetricValue>, key: &str) -> String {
    m.get(key)
        .map(tabs::metric_value)
        .unwrap_or_else(|| "Unavailable".into())
}
fn denominator(m: &BTreeMap<String, MetricValue>, n: &str, d: &str) -> String {
    format!("{} / {}", value(m, n), value(m, d))
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
    pub(super) fn metrics_panel(
        &self,
        t: &AuroraTheme,
        title: &str,
        m: &BTreeMap<String, MetricValue>,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut group = div().flex().flex_wrap().gap(px(24.));
        for metric in m.values() {
            let key = metric.definition.key.clone();
            group = group.child(
                div()
                    .min_w(px(180.))
                    .flex_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(
                                ui::caption(t, display_label(&metric.definition.label))
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
                    .child(ui::muted(t, tabs::metric_value(metric))),
            );
        }
        let mut panel = ui::card(t)
            .child(ui::section_header(t, title.to_owned(), None))
            .child(group);
        if let Some(metric) = self.metric_info.as_ref().and_then(|k| m.get(k)) {
            panel = panel.child(ui::title_block(
                t,
                format!(
                    "{} | {} | {}",
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
                .selected(expanded)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.table_expanded.remove(&id) {
                        this.table_expanded.insert(id.clone());
                    }
                    cx.notify();
                })),
            ),
        );
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
                    "{} hits / eligible attempts",
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
                    "{} within / eligible",
                    denominator(m, "reachWithinCount", "reachEligible")
                ),
            ),
            (
                "Critical conditions",
                value(m, "criticalRate"),
                format!(
                    "{} critical / confirmed hits",
                    denominator(m, "criticalConditions", "criticalDenominator")
                ),
            ),
            (
                "Combos",
                format!(
                    "{} avg | {} longest",
                    value(m, "averageCombo"),
                    value(m, "longestCombo")
                ),
                format!("{} multi-hit streaks", value(m, "comboCount")),
            ),
            (
                "Dominance shares",
                format!(
                    "{} you | {} opponent",
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
                    "{} avg | {} SD",
                    value(m, "averageInterval"),
                    value(m, "intervalDeviation")
                ),
                format!("{} eligible intervals", value(m, "intervalCount")),
            ),
            (
                "Recording coverage",
                format!(
                    "{} in scope / {} total",
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
            .child(ui::section_header(
                t,
                "Recent recordings | pooled measurements",
                None,
            ))
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
                    "Duration & attempts",
                    format!(
                        "{:.1} s | {} attempts",
                        s.duration_nanos as f64 / 1e9,
                        value(m, "attempts")
                    ),
                ),
                (
                    "Accuracy / hits / eligible attempts",
                    denominator(m, "hits", "accuracyDenominator"),
                ),
                (
                    "Reach window / within / eligible",
                    denominator(m, "reachWithinCount", "reachEligible"),
                ),
                (
                    "Combos / longest / count",
                    denominator(m, "longestCombo", "comboCount"),
                ),
                (
                    "Local dominance",
                    format!(
                        "{} | {}",
                        value(m, "dominanceShare"),
                        m.get("dominanceShare")
                            .map(|m| format!("{:?}", m.availability))
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
                        "{} | {} | {} {}",
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
            body=body.child(stack(t,"shield-offense","Attack results against shields",vec![
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
        let mut body = div().flex().flex_col().gap(px(8.));
        let Some(detail) = self.active_detail() else {
            return body;
        };
        if !detail.summary.manual && detail.exchanges.is_empty() {
            return body;
        }
        let request = self.scope_request();
        body = body.child(ui::section_header(
            t,
            "Manual session scope",
            Some(
                "Select a participant or exchange. The entire session has no tracked opponent."
                    .into(),
            ),
        ));
        let mut participants = div().flex().flex_wrap().gap(px(4.)).child(
            button("focus-all", "All participants")
                .selected(request.as_ref().is_some_and(|r| r.participant.is_none()))
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(mut r) = this.scope_request() {
                        r.participant = None;
                        r.exchange_id = None;
                        this.select_scope(r, cx);
                    }
                })),
        );
        for (i, p) in detail.participants.iter().enumerate() {
            let id = p.id.clone();
            participants = participants.child(
                button(("focus-participant", i), p.label.clone())
                    .selected(request.as_ref().and_then(|r| r.participant.as_ref()) == Some(&id))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(mut r) = this.scope_request() {
                            r.participant = Some(id.clone());
                            r.exchange_id = None;
                            this.select_scope(r, cx);
                        }
                    })),
            );
        }
        body = body.child(participants);
        let mut exchanges = div().flex().flex_wrap().gap(px(4.)).child(
            button("focus-entire", "Entire manual session")
                .selected(detail.selected_exchange.is_none())
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(mut r) = this.scope_request() {
                        r.participant = None;
                        r.exchange_id = None;
                        this.select_scope(r, cx);
                    }
                })),
        );
        for (i, e) in detail.exchanges.iter().enumerate() {
            let id = e.id.clone();
            exchanges = exchanges.child(
                button(
                    ("focus-exchange", i),
                    format!("{} | {:.1} s", e.opponent, e.duration_nanos as f64 / 1e9),
                )
                .selected(detail.selected_exchange.as_ref() == Some(&id))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(mut r) = this.scope_request() {
                        r.exchange_id = Some(id.clone());
                        this.select_scope(r, cx);
                    }
                })),
            );
        }
        body.child(exchanges)
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
  ].into_iter().map(|(label,help)|ui::title_block(t,label,Some(help.into()))))
    }
}

fn display_label(label: &str) -> String {
    let mut out = String::new();
    for (i, c) in label.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(if i == 0 { c.to_ascii_uppercase() } else { c });
    }
    out
}
