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
    match metric.definition.unit.as_str() {
        "%" => format!("{n:.1}%"),
        "m" => format!("{n:.2} m"),
        "s" => format!("{n:.2} s"),
        "HP" => format!("{n:.1} HP"),
        "ticks" => format!("{n:.2} ticks"),
        "count" | "" | "dimensionless" => format!("{n:.0}"),
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
pub(super) fn table(t: &AuroraTheme, data: &TableView, narrow: bool) -> Div {
    let count = if narrow { 3 } else { data.columns.len().min(7) };
    let columns: Vec<_> = data.columns.iter().take(count).collect();
    let mut list = ui::list_card(t)
        .min_w(px(0.))
        .child(ui::row(t, false).child(ui::section_header(t, data.title.clone(), None)));
    list = list.child(
        ui::row(t, false).children(
            columns
                .iter()
                .map(|c| ui::caption(t, (*c).clone()).flex_1().min_w(px(0.))),
        ),
    );
    for (i, row) in data.rows.iter().take(50).enumerate() {
        let full = data
            .columns
            .iter()
            .map(|c| {
                format!(
                    "{c}: {}",
                    row.cells
                        .get(c)
                        .cloned()
                        .or_else(|| row.metrics.get(c).map(metric_value))
                        .unwrap_or_else(|| "Unavailable".into())
                )
            })
            .collect::<Vec<_>>()
            .join(". ");
        list = list.child(
            ui::row(t, i + 1 == data.rows.len().min(50))
                .id(format!("{}-row-{i}", data.id))
                .role(Role::Group)
                .aria_label(full)
                .children(columns.iter().map(|c| {
                    ui::caption(
                        t,
                        row.cells
                            .get(*c)
                            .cloned()
                            .or_else(|| row.metrics.get(*c).map(metric_value))
                            .unwrap_or_else(|| "Unavailable".into()),
                    )
                    .flex_1()
                    .min_w(px(0.))
                })),
        );
    }
    if data.rows.is_empty() {
        list =
            list.child(ui::row(t, true).child(ui::muted(t, "No measured samples in this scope.")));
    }
    list
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
            y: p.y,
            label: format!(
                    "{}: {} - {} {}",
                    s.label,
                    p.label,
                    p.y.map(|y| format!("{y:.2}"))
                        .unwrap_or_else(|| "Unavailable".into()),
                    view.y_definition.unit
                ),
            shape: if p.category.contains("SHIELD") {
                Shape::Diamond
            } else if p.category.contains("MISS") || p.category.contains("UNRESOLVED") {
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
        .filter(|x| x.is_finite())
        .fold(1_f64, f64::max);
    let min_x = samples
        .iter()
        .map(|s| s.x)
        .filter(|x| x.is_finite())
        .fold(0_f64, f64::min);
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
    let range = if kind == Kind::Area {
        (-1., 1.)
    } else if view.y_definition.unit == "%" {
        (0., 100.)
    } else if kind == Kind::Scatter && view.y_definition.unit == "ticks" {
        let n = (max_y.abs().max(min_y.abs()).max(4.) + 1.).ceil().min(40.);
        (-n, n)
    } else {
        (min_y, nice_peak(max_y))
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
                "LOCAL" | "local" => Some(1.),
                "OPPONENT" | "opponent" => Some(0.),
                _ => None,
            },
            label: b.label.clone(),
            selected: false,
        })
        .collect();
    Chart {
        id: view.id.clone(),
        title: view.title.clone(),
        x_label: view.x_unit.clone(),
        y_label: format!("{} ({})", view.y_definition.label, view.y_definition.unit),
        samples,
        kind,
        domain: (min_x, max_x),
        range,
        band: y_band.or(reach_band),
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
        incomplete: vec![],
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
        if !tab.metrics.is_empty() {
            body = body.child(metric_card(t, "Recorded measurements", &tab.metrics, &[]));
        }
        for notice in &tab.notices {
            body = body.child(ui::caption(t, notice.clone()));
        }
        for (index, line) in tab.interpretations.iter().enumerate() {
            if self.top_tab != 0 || index < 4 {
                body = body.child(ui::muted(t, line.clone()));
            }
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
            let id = view.id.to_lowercase();
            let trend = id.contains("trend");
            let key = ["accuracy", "reach", "critical", "interval"][self.trend_metric];
            if self.detail_request.is_none() && self.top_tab == 0 && trend && !id.contains(key) {
                continue;
            }
            body = body.child(chart(view).render(t));
        }
        for data in &tab.tables {
            body = body.child(table(t, data, narrow));
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
                definitions = definitions.child(ui::title_block(
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
