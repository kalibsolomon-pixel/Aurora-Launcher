use super::presentation::ui;
use super::{
    Pulsar,
    charts::{Chart, Kind, Sample, Shape, nice_peak},
};
use crate::{
    controls::{AuroraButton, ButtonSize, ButtonVariant},
    theme::AuroraTheme,
};
use aurora_launcher_lib::pulsar::view::*;
use gpui::{Context, Div, Role, div, prelude::*, px};
use std::collections::BTreeMap;
pub(super) fn metric_value(metric: &MetricValue) -> String {
    if metric.availability == Availability::RuntimeOnly {
        return "Change in game".into();
    }
    let Some(n) = metric.value.filter(|v| v.is_finite()) else {
        return "Unavailable".into();
    };
    // These are continuous model balances, although the contract carries no
    // physical unit. Preserve their precision instead of treating them as counts.
    if matches!(
        metric.definition.key.as_str(),
        "largestSwingFromValue" | "largestSwingToValue" | "largestSwingDifference"
    ) {
        return format!("{n:.2}");
    }
    if metric.definition.key == "averageCombo" {
        return format!("{n:.1}");
    }
    match metric.definition.unit.as_str() {
        "%" => format!("{n:.1}%"),
        "m" => format!("{n:.1} m"),
        "s" => format!("{n:.1} s"),
        "HP" => format!("{n:.1} HP"),
        "ticks" => format!("{n:.1} ticks"),
        "count" => format!("{n:.0}"),
        "" | "dimensionless" if n.fract() != 0. => format!("{n:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned(),
        "" | "dimensionless" => format!("{n:.0}"),
        unit => format!("{n:.1} {unit}"),
    }
}
pub(super) fn stats(
    t: &AuroraTheme,
    metrics: &BTreeMap<String, MetricValue>,
    keys: &[&str],
) -> Div {
    let entries: Vec<_> = if keys.is_empty() {
        metrics.values().collect()
    } else {
        keys.iter().filter_map(|k| metrics.get(*k)).collect()
    };
    div()
        .flex()
        .flex_wrap()
        .gap(px(24.))
        .children(entries.into_iter().map(|m| {
            let description = format!(
                "{}: {}. {}. {}. {}",
                m.definition.label,
                metric_value(m),
                m.definition.confidence,
                m.definition.help,
                m.reason.as_deref().unwrap_or("")
            );
            ui::stat(t, m.definition.label.clone(), metric_value(m))
                .id(format!("metric-{}", m.definition.key))
                .min_w(px(132.))
                .flex_1()
                .role(Role::Group)
                .aria_label(description)
        }))
}
pub(super) fn metric_card(
    t: &AuroraTheme,
    title: &str,
    metrics: &BTreeMap<String, MetricValue>,
    keys: &[&str],
) -> Div {
    ui::card(t)
        .child(ui::section_header(t, title.to_owned(), None))
        .child(stats(t, metrics, keys))
}
/// Identity and evidence stay visible when a narrow table drops secondary fields.
fn table_columns(columns: &[String]) -> Vec<&String> {
    let priority = [
        "name",
        "weapon",
        "time",
        "t",
        "start",
        "round",
        "opponent",
        "kind",
        "type",
        "attribution",
        "grade",
        "outcome",
        "hits",
        "within",
        "mean",
        "deviation",
        "max",
        "damage",
        "knockback",
        "crit",
        "jumpOffsetTicks",
        "jumpClass",
        "accepted",
        "origin",
        "rule",
        "text",
        "duration",
        "end",
    ];
    let mut ordered = Vec::new();
    for name in priority {
        if let Some(column) = columns.iter().find(|c| c.as_str() == name) {
            ordered.push(column);
        }
    }
    for column in columns {
        if !ordered.contains(&column) {
            ordered.push(column);
        }
    }
    ordered
}
fn cell(row: &TableRow, column: &str) -> String {
    let value = row
        .metrics
        .get(column)
        .map(metric_value)
        .or_else(|| {
            row.cells
                .get(column)
                .cloned()
                .filter(|v| v != "-" && v != "null")
        })
        .unwrap_or_else(|| "Unavailable".into());
    super::presentation::text(&value)
}
pub(super) fn table(t: &AuroraTheme, data: &TableView, narrow: bool) -> Div {
    let _ = narrow;
    let count = data.columns.len();
    let order = table_columns(&data.columns);
    let columns: Vec<_> = order.into_iter().take(count).collect();
    let width = (count.max(1) as f32 * 144.).max(420.);
    let mut list = ui::list_card(t)
        .w(px(width))
        .child(ui::row(t, false).child(ui::section_header(t, data.title.clone(), None)))
        .child(ui::row(t, false).children(columns.iter().map(|c| {
            ui::caption(t, super::analysis_ui::display_label(c))
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
        })));
    for (i, row) in data.rows.iter().enumerate() {
        let full = data
            .columns
            .iter()
            .map(|c| format!("{c}: {}", cell(row, c)))
            .collect::<Vec<_>>()
            .join(". ");
        let theme = t.clone();
        list = list.child(
            ui::row(t, i + 1 == data.rows.len())
                .id(format!("{}-row-{i}", data.id))
                .role(Role::Group)
                .when(
                    row.cells.get("Establishing").is_some_and(|v| v == "true"),
                    |d| d.bg(t.accent_soft).border_l_2().border_color(t.ring),
                )
                .aria_label(full.clone())
                .tooltip(move |_, cx| {
                    cx.new(|_| super::charts::Readout {
                        text: full.clone(),
                        theme: theme.clone(),
                    })
                    .into()
                })
                .children(columns.iter().map(|c| {
                    ui::caption(t, cell(row, c))
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                })),
        );
    }
    if data.rows.is_empty() {
        list =
            list.child(ui::row(t, true).child(ui::muted(t, "No measured samples in this scope.")));
    }
    div().min_w(px(0.)).w_full().child(
        div()
            .id(format!("{}-horizontal", data.id))
            .min_w(px(0.))
            .w_full()
            .overflow_x_scroll()
            .child(list),
    )
}

fn heatmap(t: &AuroraTheme, view: &ChartView) -> Div {
    let peak = view
        .series
        .iter()
        .flat_map(|s| &s.points)
        .filter_map(|p| p.y)
        .fold(1.0_f64, f64::max);
    let mut grid = div().flex().flex_col().gap(px(2.)).max_w(px(300.));
    for (r, series) in view.series.iter().enumerate() {
        grid = grid.child(
            div()
                .flex()
                .gap(px(2.))
                .children(series.points.iter().enumerate().map(|(c, p)| {
                    let label = p.label.clone();
                    let tip = label.clone();
                    let theme = t.clone();
                    div()
                        .id(format!("{}-{r}-{c}", view.id))
                        .flex_1()
                        .h(px(18.))
                        .bg(p
                            .y
                            .filter(|n| *n > 0.)
                            .map(|n| t.ring.opacity(0.2 + 0.8 * (n / peak) as f32))
                            .unwrap_or(t.sunken))
                        .role(Role::Image)
                        .aria_label(label)
                        .tooltip(move |_, cx| {
                            cx.new(|_| super::charts::Readout {
                                text: tip.clone(),
                                theme: theme.clone(),
                            })
                            .into()
                        })
                })),
        );
    }
    ui::card(t)
        .child(ui::section_header(t, view.title.clone(), None))
        .child(ui::caption(
            t,
            "Normalized face height: 1 at top, 0 at bottom · horizontal position: 0 left, 1 right",
        ))
        .child(grid)
        .child(ui::caption(
            t,
            format!(
                "6 columns · 12 rows · cell unit: confirmed hits · {}",
                view.description
            ),
        ))
}
pub(super) fn chart(view: &ChartView) -> Chart {
    let kind = match view.kind.as_str() {
        "histogram" | "bar" => Kind::Histogram,
        "stackedBar" => Kind::Stacked,
        "scatter" | "timing" | "reachScatter" => Kind::Scatter,
        "area" | "dominance" => Kind::Area,
        "timeline" | "combo" => Kind::Timeline,
        "lanes" => Kind::Lanes,
        "grid" | "heatmap" => Kind::Grid,
        _ => Kind::Line,
    };
    let mut samples = Vec::new();
    for (series, s) in view.series.iter().enumerate() {
        if !samples.is_empty() && matches!(kind, Kind::Line | Kind::Area) {
            samples.push(Sample {
                x: 0.,
                y: None,
                label: "Series boundary".into(),
                shape: Shape::Reset,
            });
        }
        samples.extend(s.points.iter().map(|p| Sample {
            x: p.x,
            y: if kind == Kind::Lanes {
                p.y.map(|_| series as f64)
            } else if view.y_definition.unit == "%" {
                p.y.filter(|v| v.is_finite()).map(|v| v.clamp(0., 100.))
            } else {
                p.y
            },
            label: if kind == Kind::Stacked {
                p.label.clone()
            } else {
                format!(
                    "{}: {} - {} {} {}",
                    s.label,
                    p.label,
                    p.y.map(|y| format!("{y:.2}"))
                        .unwrap_or_else(|| "Unavailable".into()),
                    view.y_definition.unit,
                    p.category
                )
            },
            shape: if view.id.contains("trend") && p.y == Some(0.) {
                Shape::Hollow
            } else if p.category.contains("SHIELD") || p.category == "trade" {
                Shape::Diamond
            } else if p.category.contains("MISS")
                || p.category.contains("UNRESOLVED")
                || p.category == "hollow"
            {
                Shape::Hollow
            } else if p.y.is_none() || p.category.contains("OPENER") {
                Shape::Reset
            } else if series > 0 {
                Shape::Diamond
            } else {
                Shape::Square
            },
        }));
    }
    let mut max_x = samples
        .iter()
        .map(|s| s.x)
        .chain(
            view.bands
                .iter()
                .filter(|b| b.lane != "yTolerance")
                .map(|b| b.start.max(b.end)),
        )
        .filter(|x| x.is_finite())
        .fold(1_f64, f64::max);
    let mut min_x = samples
        .iter()
        .map(|s| s.x)
        .filter(|x| x.is_finite())
        .fold(0_f64, f64::min);
    if kind == Kind::Histogram && view.x_unit == "m" {
        min_x = samples
            .iter()
            .map(|s| s.x)
            .filter(|x| x.is_finite())
            .min_by(f64::total_cmp)
            .unwrap_or(0.)
            - 0.025;
    }
    let combo_distribution = view.id == "pooled_combo_distribution";
    if combo_distribution {
        min_x = samples
            .iter()
            .map(|s| s.x)
            .min_by(f64::total_cmp)
            .unwrap_or(2.)
            - 0.5;
        max_x += 0.5;
    }
    let max_y = samples
        .iter()
        .filter_map(|s| s.y)
        .filter(|y| y.is_finite())
        .fold(0_f64, f64::max);
    let min_y = samples
        .iter()
        .filter_map(|s| s.y)
        .filter(|y| y.is_finite())
        .fold(0_f64, f64::min);
    let range = if view.id == "active_exchanges" {
        (-0.5, 0.5)
    } else if kind == Kind::Area {
        (-1., 1.)
    } else if view.y_definition.unit == "%" {
        (0., 100.)
    } else if kind == Kind::Scatter && view.y_definition.unit == "ticks" {
        let n = (max_y.abs().max(min_y.abs()).max(4.) + 1.).ceil().min(40.);
        (-n, n)
    } else {
        if combo_distribution {
            // Four equal integer intervals keep both count axes truthful.
            (0., (max_y / 4.).ceil().max(1.) * 4.)
        } else if kind == Kind::Lanes {
            (-0.5, view.series.len().max(1) as f64 - 0.5)
        } else {
            (min_y, nice_peak(max_y))
        }
    };
    let y_band = view
        .bands
        .iter()
        .find(|b| b.lane == "yTolerance")
        .map(|b| (b.start, b.end));
    let reach_band = if kind == Kind::Histogram {
        view.bands.first().map(|b| (b.start, b.end))
    } else {
        None
    };
    let bands = view
        .bands
        .iter()
        .filter(|b| b.lane != "yTolerance" && kind != Kind::Histogram)
        .map(|b| super::charts::Band {
            start: b.start,
            end: b.end,
            lane: match b.lane.as_str() {
                "activeExchange" => Some(0.),
                "LOCAL" | "local" => Some(1.),
                "OPPONENT" => Some(-1.),
                "opponent" => Some(0.),
                _ => None,
            },
            label: b.label.clone(),
            selected: false,
        })
        .collect();
    let mut incomplete = Vec::new();
    for series in &view.series {
        for pair in series.points.windows(2) {
            if pair[0].availability == Availability::Incomplete || pair[0].category == "C" {
                incomplete.push((pair[0].x, pair[1].x));
            }
        }
    }
    Chart {
        id: view.id.clone(),
        title: view.title.clone(),
        x_label: if kind == Kind::Stacked && view.x_unit.is_empty() {
            "observations".into()
        } else {
            view.x_unit.clone()
        },
        y_label: if view.y_definition.unit.is_empty() {
            view.y_definition.label.clone()
        } else {
            format!("{} ({})", view.y_definition.label, view.y_definition.unit)
        },
        samples,
        kind,
        domain: (min_x, max_x),
        range,
        band: y_band
            .or(reach_band)
            .or_else(|| (kind == Kind::Area).then_some((-0.2, 0.2))),
        target: if kind == Kind::Area {
            Some(0.)
        } else if let Some((a, b)) = reach_band {
            Some((a + b) / 2.)
        } else if y_band.is_some() {
            Some(0.)
        } else {
            None
        },
        playhead: None,
        bands,
        incomplete,
        note: view.description.clone(),
    }
}

impl Pulsar {
    pub(super) fn tab_body(
        &self,
        t: &AuroraTheme,
        tab: &TabView,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut body = div().flex().flex_col().gap(px(12.)).min_w(px(0.));
        let index = if self.detail_request.is_some() {
            self.detail_tab
        } else {
            self.top_tab
        };
        let sections = super::sections::for_tab(index);
        let filtered;
        let mut flow_selector = None;
        let tab = if sections.is_empty() {
            tab
        } else {
            let selected = self.sections[index];
            let mut strip = div().flex().flex_wrap().gap(px(4.)).child(
                AuroraButton::new(("pulsar-section", 0usize), "All sections")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Small)
                    .selected(selected == 0)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sections[index] = 0;
                        cx.notify();
                    })),
            );
            for (i, (label, _)) in sections.iter().enumerate() {
                strip = strip.child(
                    AuroraButton::new(("pulsar-section", i + 1), *label)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .selected(selected == i + 1)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.sections[index] = i + 1;
                            cx.notify();
                        })),
                );
            }
            if index == 3 {
                flow_selector = Some(strip);
            } else {
                body = body.child(strip);
            }
            filtered = super::sections::filter(tab, sections, selected);
            &filtered
        };
        if index == 0 && self.detail_request.is_none() {
            body = body.child(self.overview_groups(t, &tab.metrics, cx));
        }
        let diagnostic = |k: &str| {
            matches!(
                k,
                "totalInputs"
                    | "uniqueAttacks"
                    | "deduplicatedAttacks"
                    | "excludedSwings"
                    | "eligibleMisses"
                    | "unresolved"
                    | "uncertainShield"
            )
        };
        let summary = index == 0 && self.detail_request.is_some();
        let mut summary_disclosures = div().flex().flex_wrap().gap(px(4.));
        let accuracy = index == 1 && self.sections[1] == 1;
        let dominance = index == 3 && self.sections[3] == 2;
        let primary_dominance = |k: &str| {
            matches!(
                k,
                "dominanceShare" | "opponentDominanceShare" | "dominanceActiveSeconds"
            )
        };
        let compact_flow =
            index == 3 && self.detail_request.is_none() && matches!(self.sections[3], 0 | 1);
        let primary_accuracy = |k: &str| matches!(k, "hits" | "accuracy" | "accuracyDenominator");
        let main: BTreeMap<_, _> = tab
            .metrics
            .iter()
            .filter(|(k, _)| {
                !diagnostic(k)
                    && (!compact_flow
                        || matches!(
                            k.as_str(),
                            "averageCombo" | "longestCombo" | "longestOpponentCombo" | "comboCount"
                        ))
                    && (!accuracy || primary_accuracy(k))
                    && (!dominance || primary_dominance(k))
                    && (!summary
                        || matches!(
                            k.as_str(),
                            "accuracy" | "hits" | "averageReach" | "longestCombo"
                        ))
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if index == 3 && self.detail_request.is_none() && matches!(self.sections[3], 2 | 3) {
            let metric = |key: &str| {
                tab.metrics
                    .get(key)
                    .map(metric_value)
                    .unwrap_or_else(|| "Unavailable".into())
            };
            let line = if self.sections[3] == 2 {
                format!(
                    "You held the advantage for {} of measured active combat; the opponent held {}.",
                    metric("dominanceShare"),
                    metric("opponentDominanceShare")
                )
            } else if self.sections[3] == 3 {
                format!(
                    "{} of active combat was recorded; {} lacks enough evidence for the dominance model.",
                    metric("combatActiveSeconds"),
                    metric("dominanceInsufficientSeconds")
                )
            } else {
                format!(
                    "Your combos averaged {} hits, with a longest run of {}. Opponent combos peaked at {}.",
                    metric("averageCombo"),
                    metric("longestCombo"),
                    metric("longestOpponentCombo")
                )
            };
            body = body.child(ui::muted(t, line));
        }
        if !main.is_empty() && !(index == 0 && self.detail_request.is_none()) {
            if summary {
                if let Some(insight) = super::presentation::summary_insight(
                    self.detail.as_ref().is_some_and(|d| d.summary.manual)
                        && self
                            .detail_request
                            .as_ref()
                            .is_some_and(|r| r.participant.is_none() && r.exchange_id.is_none()),
                    &tab.metrics,
                    tab.interpretations.first(),
                ) {
                    body = body.child(ui::muted(t, insight.clone()));
                }
            }
            body = body.child(self.metrics_panel(
                t,
                if summary {
                    "At a glance"
                } else if self.detail_request.is_none() {
                    "Pooled performance"
                } else {
                    "Performance"
                },
                &main,
                cx,
            ));
        }
        if index == 1 && matches!(self.sections[1], 0 | 1) {
            let eligible = tab.metrics.get("accuracyDenominator").and_then(|v| v.value);
            let observed = tab
                .charts
                .iter()
                .find(|c| c.id == "attack_outcomes")
                .map(|c| {
                    c.series
                        .iter()
                        .flat_map(|s| &s.points)
                        .filter_map(|p| p.y)
                        .sum::<f64>()
                });
            if let (Some(eligible), Some(observed)) = (eligible, observed) {
                body = body.child(ui::caption(t, format!("{eligible:.0} eligible = {observed:.0} observed - {:.0} confirmed shield blocks. Accuracy excludes confirmed blocks.", (observed - eligible).max(0.))));
            }
        }
        if index == 3 && self.detail_request.is_none() {
            body = body.child(self.focus_navigation(t, cx));
            if let Some(c) = tab
                .charts
                .iter()
                .find(|c| c.id == "pooled_combo_distribution")
            {
                body = body.child(chart(c).render_options(
                    t,
                    None,
                    super::charts::Options {
                        preferences: self.preferences,
                        ..Default::default()
                    },
                ));
            }
        }
        if let Some(strip) = flow_selector {
            body = body.child(strip);
        }
        // Accuracy starts with the outcome breakdown; counters remain inspectable.
        if index == 1 && matches!(self.sections[1], 0 | 1) {
            for c in tab.charts.iter().filter(|c| c.id == "attack_outcomes") {
                body = body.child(chart(c).render(t));
            }
        }
        if index == 2 {
            body = body.child(self.compositions(t, tab, index));
        }
        let diagnostics: BTreeMap<_, _> = tab
            .metrics
            .iter()
            .filter(|(k, _)| {
                diagnostic(k)
                    || ((summary || accuracy || dominance || compact_flow)
                        && !main.contains_key(*k))
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if !diagnostics.is_empty() {
            let control = self.disclosure(
                t,
                if summary {
                    "supporting-metrics"
                } else if dominance {
                    "flow-metrics"
                } else {
                    "counting"
                },
                if summary {
                    "Supporting measurements"
                } else if dominance {
                    "Model measurements"
                } else {
                    "How this is counted"
                },
                div().flex().flex_col().gap(px(8.)).when(index == 1 && matches!(self.sections[1], 0 | 1), |d| d.child(ui::caption(t, "Accuracy counts confirmed hits out of eligible attempts. Confirmed shield blocks are excluded; uncertain attempts remain in the denominator. Zero-count outcomes stay in the legend and have no bar width."))).child(self.metrics_panel(t, "Counting details", &diagnostics, cx)),
                cx,
            );
            if summary {
                summary_disclosures = summary_disclosures.child(control);
            } else {
                body = body.child(control);
            }
        }
        let mut context = div().flex().flex_col().gap(px(12.));
        for notice in &tab.notices {
            context = context.child(ui::caption(t, notice.clone()));
        }
        for line in tab.interpretations.iter().skip(if summary { 1 } else { 0 }) {
            context = context.child(ui::muted(t, line.clone()));
        }
        if !tab.notices.is_empty() || !tab.interpretations.is_empty() {
            let control = self.disclosure(
                t,
                "interpretation",
                "Interpretation and limitations",
                context,
                cx,
            );
            if summary {
                summary_disclosures = summary_disclosures.child(control);
            } else {
                body = body.child(control);
            }
        }
        if summary {
            body = body.child(summary_disclosures);
        }
        if (self.detail_request.is_some() && (1..=3).contains(&index))
            || (self.detail_request.is_none()
                && (index == 3 || (index == 1 && self.sections[1] == 3)))
        {
            if self.detail_request.is_none() {
                if index != 3 {
                    body = body.child(self.focus_navigation(t, cx));
                }
                body = body.child(self.focus_scopes(t, cx));
            }
            body = body.child(self.advanced_body(t, narrow, cx));
        }
        if index != 2 {
            body = body.child(self.compositions(t, tab, index));
        }
        if self.detail_request.is_none() && self.top_tab == 0 {
            let mut selectors = div().flex().flex_wrap().gap(px(4.));
            for (i, label) in [
                "Accuracy",
                "Average reach",
                "Critical-hit rate",
                "Hit interval",
            ]
            .into_iter()
            .enumerate()
            {
                selectors = selectors.child(
                    AuroraButton::new(("trend-metric", i), label)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .selected(self.trend_metric == i)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.trend_metric = i;
                            cx.notify();
                        })),
                );
            }
            body = body.child(selectors);
        }
        for view in &tab.charts {
            if index == 3 && self.detail_request.is_none() && view.id == "pooled_combo_distribution"
            {
                continue;
            }
            if index == 1 && view.id == "attack_outcomes" && matches!(self.sections[1], 0 | 1) {
                continue;
            }
            // The detailed controls below replace these static projections.
            if (self.detail_request.is_some()
                || (index == 3 && self.focus_detail.is_some())
                || (index == 1 && self.sections[1] == 3 && self.focus_detail.is_some()))
                && matches!(
                    view.id.as_str(),
                    "reach_histogram"
                        | "reach_scatter"
                        | "aim_contacts"
                        | "aim_density"
                        | "timing_ordinal"
                        | "combos"
                        | "dominance"
                )
            {
                continue;
            }
            if view.kind == "heatmap" {
                body = body.child(heatmap(t, view));
                continue;
            }
            let id = view.id.to_lowercase();
            let trend = id.contains("trend");
            let key = ["accuracy", "reach", "critical", "interval"][self.trend_metric];
            if self.detail_request.is_none() && self.top_tab == 0 && trend && !id.contains(key) {
                continue;
            }
            body = body.child(chart(view).render_options(
                t,
                None,
                super::charts::Options {
                    preferences: self.preferences,
                    ..Default::default()
                },
            ));
        }
        for data in &tab.tables {
            if self.detail_request.is_some()
                && matches!(data.id.as_str(), "incoming_latest" | "timing_latest")
            {
                continue;
            }
            body = body.child(self.table_view(t, data, narrow, cx));
        }
        body = body.child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.))
                .child(
                    AuroraButton::new("pulsar-methods", "Definitions & evidence")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .icon(if self.show_methods {
                            "expand_less"
                        } else {
                            "expand_more"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_methods = !this.show_methods;
                            cx.notify();
                        })),
                )
                .child(
                    AuroraButton::new("pulsar-chart-data", "Chart data")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .icon(if self.show_data {
                            "expand_less"
                        } else {
                            "expand_more"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_data = !this.show_data;
                            cx.notify();
                        })),
                ),
        );
        if self.show_methods {
            let mut definitions =
                ui::card(t).child(ui::section_header(t, "Definitions & evidence", None));
            for m in tab.metrics.values() {
                definitions = definitions.child(super::analysis_ui::explanation(
                    t,
                    format!(
                        "{} · {} · {}",
                        m.definition.label, m.definition.unit, m.definition.confidence
                    ),
                    Some(
                        format!(
                            "{} {}",
                            m.definition.help,
                            m.reason.as_deref().unwrap_or("")
                        )
                        .into(),
                    ),
                ));
            }
            body = body.child(definitions);
        }
        if self.show_data {
            for c in &tab.charts {
                let mut data =
                    ui::card(t).child(ui::section_header(t, format!("{} · data", c.title), None));
                for s in &c.series {
                    for p in s.points.iter().take(120) {
                        data = data.child(ui::caption(
                            t,
                            format!(
                                "{} · x {:.2} {} · {} {} · {}",
                                s.label,
                                p.x,
                                c.x_unit,
                                p.y.map(|v| format!("{v:.3}"))
                                    .unwrap_or_else(|| "Unavailable".into()),
                                c.y_definition.unit,
                                p.label
                            ),
                        ));
                    }
                }
                body = body.child(data);
            }
        }
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dominance_balances_keep_precision_without_turning_missing_into_zero() {
        let mut metric = MetricValue {
            value: Some(-0.04),
            availability: Availability::Available,
            definition: MetricDef {
                key: "largestSwingToValue".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(metric_value(&metric), "-0.04");
        metric.definition.key = "largestSwingDifference".into();
        metric.value = Some(0.94);
        assert_eq!(metric_value(&metric), "0.94");
        metric.value = None;
        assert_eq!(metric_value(&metric), "Unavailable");
        metric.definition.key = "hits".into();
        metric.value = Some(13.);
        assert_eq!(metric_value(&metric), "13");
        metric.definition.key = "x".into();
        metric.value = Some(0.34);
        assert_eq!(metric_value(&metric), "0.34");
        metric.definition.unit = "dimensionless".into();
        metric.value = Some(0.075);
        assert_eq!(metric_value(&metric), "0.075");
        metric.definition.key = "meanHorizontalKnockback".into();
        metric.definition.unit = "blocks/tick".into();
        metric.value = Some(0.37);
        assert_eq!(metric_value(&metric), "0.4 blocks/tick");
    }
    #[test]
    fn narrow_tables_keep_identity_and_measurement_units() {
        let columns = ["deviation", "max", "mean", "name", "eligible"].map(String::from);
        let ordered = table_columns(&columns);
        assert_eq!(ordered[0], "name");
        assert_eq!(ordered[1], "mean");
        let row = TableRow {
            cells: [
                ("mean".into(), "2.777777".into()),
                ("missing".into(), "-".into()),
            ]
            .into(),
            metrics: [(
                "mean".into(),
                MetricValue {
                    value: Some(2.777777),
                    availability: Availability::Available,
                    definition: MetricDef {
                        unit: "m".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )]
            .into(),
            ..Default::default()
        };
        assert_eq!(cell(&row, "mean"), "2.8 m");
        assert_eq!(cell(&row, "missing"), "Unavailable");
    }
    #[test]
    fn reach_domain_retains_fine_bins_and_lanes_separate_subjects() {
        let view = ChartView {
            kind: "histogram".into(),
            x_unit: "m".into(),
            series: vec![ChartSeries {
                points: vec![
                    ChartPoint {
                        x: 1.95,
                        y: Some(0.),
                        ..Default::default()
                    },
                    ChartPoint {
                        x: 4.05,
                        y: Some(1.),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let projected = chart(&view);
        assert!((projected.domain.0 - 1.925).abs() < 1e-9);
        let lane = ChartView {
            kind: "lanes".into(),
            series: vec![
                ChartSeries {
                    label: "local".into(),
                    points: vec![ChartPoint {
                        y: Some(1.),
                        ..Default::default()
                    }],
                },
                ChartSeries {
                    label: "opponent".into(),
                    points: vec![ChartPoint {
                        y: Some(1.),
                        ..Default::default()
                    }],
                },
            ],
            ..Default::default()
        };
        let projected = chart(&lane);
        assert_eq!(projected.samples[0].y, Some(0.));
        assert_eq!(projected.samples[1].y, Some(1.));
    }

    #[test]
    fn combo_distribution_centers_integer_lengths_inside_plot() {
        let view = ChartView {
            id: "pooled_combo_distribution".into(),
            kind: "histogram".into(),
            series: vec![ChartSeries {
                points: [(2., 33.), (6., 27.)]
                    .into_iter()
                    .map(|(x, y)| ChartPoint {
                        x,
                        y: Some(y),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let p = chart(&view);
        assert_eq!(p.domain, (1.5, 6.5));
        assert_eq!(p.range, (0., 36.));
        assert_eq!((p.range.1 / 4.).fract(), 0.);
        for s in &p.samples {
            assert!((0.09..0.91).contains(&super::super::charts::fraction(s.x, p.domain)));
        }
    }

    #[test]
    fn percentage_charts_clamp_values_and_preserve_missing_samples() {
        let view = ChartView {
            id: "accuracy_trend".into(),
            y_definition: MetricDef {
                unit: "%".into(),
                ..Default::default()
            },
            series: vec![ChartSeries {
                points: [Some(-2.), Some(102.), None, Some(0.)]
                    .into_iter()
                    .enumerate()
                    .map(|(i, y)| ChartPoint {
                        x: i as f64,
                        y,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let p = chart(&view);
        assert_eq!(p.range, (0., 100.));
        assert_eq!(
            p.samples.iter().map(|s| s.y).collect::<Vec<_>>(),
            [Some(0.), Some(100.), None, Some(0.)]
        );
    }

    #[test]
    fn interval_only_charts_and_incomplete_dominance_keep_their_extents() {
        let interval = ChartView {
            kind: "timeline".into(),
            bands: vec![ChartBand {
                start: 4.,
                end: 12.,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(chart(&interval).domain, (0., 12.));
        let area = ChartView {
            kind: "dominance".into(),
            series: vec![ChartSeries {
                points: vec![
                    ChartPoint {
                        x: 2.,
                        y: None,
                        category: "C".into(),
                        ..Default::default()
                    },
                    ChartPoint {
                        x: 3.,
                        y: Some(0.5),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let projected = chart(&area);
        assert_eq!(projected.incomplete, [(2., 3.)]);
        assert_eq!(projected.band, Some((-0.2, 0.2)));
        assert_eq!(projected.samples[0].y, None);
    }
}
