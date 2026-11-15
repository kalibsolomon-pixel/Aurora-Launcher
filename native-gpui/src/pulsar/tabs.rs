use super::{
    Pulsar,
    charts::{Chart, Kind, Sample, Shape, nice_peak},
};
use crate::{
    controls::{AuroraButton, ButtonSize, ButtonVariant},
    theme::AuroraTheme,
    ui,
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
    match metric.definition.unit.as_str() {
        "%" => format!("{n:.1}%"),
        "m" => format!("{n:.2} m"),
        "s" => format!("{n:.2} s"),
        "HP" => format!("{n:.1} HP"),
        "ticks" => format!("{n:.2} ticks"),
        "count" => format!("{n:.0}"),
        "" | "dimensionless" if n.fract() != 0. => format!("{n:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned(),
        "" | "dimensionless" => format!("{n:.0}"),
        unit => format!("{n:.2} {unit}"),
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
    row.metrics
        .get(column)
        .map(metric_value)
        .or_else(|| {
            row.cells
                .get(column)
                .cloned()
                .filter(|v| v != "-" && v != "null")
        })
        .unwrap_or_else(|| "Unavailable".into())
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
            ui::caption(t, (*c).clone())
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
            "Normalized face height: 1 at top, 0 at bottom ? horizontal position: 0 left, 1 right",
        ))
        .child(grid)
        .child(ui::caption(
            t,
            format!(
                "6 columns ? 12 rows ? cell unit: confirmed hits ? {}",
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
            shape: if p.category.contains("SHIELD") || p.category == "trade" {
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
    let max_x = samples
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
        if kind == Kind::Lanes {
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
        let mut body = div().flex().flex_col().gap(px(ui::PAGE_GAP)).min_w(px(0.));
        let index = if self.detail_request.is_some() {
            self.detail_tab
        } else {
            self.top_tab
        };
        let sections = super::sections::for_tab(index);
        let filtered;
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
            body = body.child(strip);
            filtered = super::sections::filter(tab, sections, selected);
            &filtered
        };
        if index == 0 && self.detail_request.is_none() {
            body = body.child(self.overview_groups(t, &tab.metrics, cx));
        }
        if !tab.metrics.is_empty() && !(index == 0 && self.detail_request.is_none()) {
            body = body.child(self.metrics_panel(
                t,
                if self.detail_request.is_some() && index == 1 {
                    "Selected measurements"
                } else {
                    "Recorded measurements"
                },
                &tab.metrics,
                cx,
            ));
        }
        for notice in &tab.notices {
            body = body.child(ui::caption(t, notice.clone()));
        }
        for (index, line) in tab.interpretations.iter().enumerate() {
            if self.top_tab != 0 || index < 4 {
                body = body.child(ui::muted(t, line.clone()));
            }
        }
        if (self.detail_request.is_some() && (1..=3).contains(&index))
            || (self.detail_request.is_none()
                && (index == 3 || (index == 1 && self.sections[1] == 3)))
        {
            if self.detail_request.is_none() {
                if let Some(d) = self.focus_detail.as_ref() {
                    body = body.child(ui::section_header(
                        t,
                        format!("Focused recording | {}", d.summary.opponent),
                        Some(
                            format!(
                                "{} | {} | {:.1} s",
                                d.summary.provenance.instance_name,
                                d.summary.date_group,
                                d.summary.duration_nanos as f64 / 1e9
                            )
                            .into(),
                        ),
                    ));
                }
                body = body.child(self.focus_scopes(t, cx));
            }
            body = body.child(self.advanced_body(t, narrow, cx));
        }
        body = body.child(self.compositions(t, tab, index));
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
                        .selected(self.show_methods)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_methods = !this.show_methods;
                            cx.notify();
                        })),
                )
                .child(
                    AuroraButton::new("pulsar-chart-data", "Chart data")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .selected(self.show_data)
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
        assert_eq!(cell(&row, "mean"), "2.78 m");
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
