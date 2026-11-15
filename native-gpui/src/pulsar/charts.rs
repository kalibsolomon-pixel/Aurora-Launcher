//! Native, event-driven charts. A missing sample always breaks a path.
use super::presentation::ui;
use crate::theme::AuroraTheme;
use gpui::{
    Context, Div, Hsla, PathBuilder, Role, Window, canvas, div, point, prelude::*, px, relative,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Shape {
    #[default]
    Square,
    Diamond,
    Hollow,
    Reset,
    Triangle,
    Cross,
    Plus,
    Tick,
}

/// Saved report preferences also govern native charts. No animation is scheduled here.
#[derive(Clone, Copy, Debug)]
pub(super) struct Preferences {
    pub grid: bool,
    pub points: bool,
    pub reduced_motion: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            grid: true,
            points: true,
            reduced_motion: false,
        }
    }
}
impl Preferences {
    pub fn from_settings(v: &aurora_launcher_lib::pulsar::view::SettingsView) -> Self {
        let flag = |key: &str, default| {
            v.settings
                .iter()
                .find(|s| s.key == key)
                .and_then(|s| s.value.as_bool())
                .unwrap_or(default)
        };
        Self {
            grid: flag("chartGrid", true),
            points: flag("chartPoints", true),
            reduced_motion: flag("reducedMotion", false),
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Target {
    Sample(usize),
    Band(usize),
}
type Activate = std::rc::Rc<dyn Fn(Target, &mut Window, &mut gpui::App)>;
#[derive(Default)]
pub(super) struct Options {
    pub preferences: Preferences,
    pub activate: Option<Activate>,
    /// Frozen region ellipses: center x/y, radii x/y, grade index; largest first.
    pub regions: Vec<(f64, f64, f64, f64, usize)>,
    pub connectors: Vec<(f64, f64, f64, f64)>,
    pub selected_sample: Option<usize>,
}
pub(super) fn grade_color(t: &AuroraTheme, label: &str) -> Hsla {
    if label.contains("PERIPHERAL") || label.contains("TOO_FAST") {
        t.warning
    } else if label.contains("POOR") || label.contains("TOO_SLOW") {
        t.danger
    } else if label.contains("IDEAL") || label.contains("PERFECT") {
        t.success
    } else if label.contains("GOOD") {
        t.ring
    } else {
        t.muted_foreground
    }
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub x: f64,
    pub y: Option<f64>,
    pub label: String,
    pub shape: Shape,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Line,
    Histogram,
    Stacked,
    Scatter,
    Timeline,
    Area,
    Lanes,
    Grid,
}

#[derive(Clone, Debug)]
pub struct Band {
    pub start: f64,
    pub end: f64,
    pub lane: Option<f64>,
    pub label: String,
    pub selected: bool,
}

#[derive(Clone)]
pub struct Chart {
    pub id: String,
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub samples: Vec<Sample>,
    pub kind: Kind,
    pub domain: (f64, f64),
    pub range: (f64, f64),
    pub band: Option<(f64, f64)>,
    pub target: Option<f64>,
    pub playhead: Option<f64>,
    pub bands: Vec<Band>,
    pub incomplete: Vec<(f64, f64)>,
    pub note: String,
}

pub fn fraction(value: f64, domain: (f64, f64)) -> f32 {
    if !value.is_finite() || !domain.0.is_finite() || !domain.1.is_finite() || domain.1 <= domain.0
    {
        return 0.;
    }
    ((value - domain.0) / (domain.1 - domain.0)).clamp(0., 1.) as f32
}

pub fn nice_peak(value: f64) -> f64 {
    if !value.is_finite() || value <= 0. {
        return 1.;
    }
    let magnitude = 10_f64.powf(value.log10().floor());
    [1., 2., 5., 10.]
        .into_iter()
        .map(|n| n * magnitude)
        .find(|n| *n >= value)
        .unwrap_or(value)
}

pub fn segments(samples: &[Sample]) -> Vec<Vec<(f64, f64)>> {
    let mut out = Vec::new();
    let mut run = Vec::new();
    for s in samples {
        if let Some(y) = s.y.filter(|y| y.is_finite() && s.x.is_finite()) {
            run.push((s.x, y));
        } else if !run.is_empty() {
            out.push(std::mem::take(&mut run));
        }
    }
    if !run.is_empty() {
        out.push(run);
    }
    out
}

/// Frozen fine-reach display bins: underflow, forty 0.05 m bins, overflow.
pub fn reach_bins(values: impl IntoIterator<Item = f64>) -> [u64; 42] {
    let mut bins = [0; 42];
    for value in values.into_iter().filter(|v| v.is_finite() && *v >= 0.) {
        let index = if value < 2. {
            0
        } else if value >= 4. {
            41
        } else {
            1 + (((value - 2.) / 0.05 + 1e-9).floor() as usize).min(39)
        };
        bins[index] += 1;
    }
    bins
}

#[derive(Clone)]
pub(super) struct Readout {
    pub text: String,
    pub theme: AuroraTheme,
}
impl gpui::Render for Readout {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(320.))
            .p(px(12.))
            .rounded(self.theme.radius)
            .bg(self.theme.raised)
            .border_1()
            .border_color(self.theme.border)
            .text_size(px(ui::TEXT_CAPTION))
            .text_color(self.theme.foreground)
            .child(super::presentation::text(&self.text))
    }
}

fn stroke(window: &mut Window, coords: &[(gpui::Pixels, gpui::Pixels)], color: Hsla, width: f32) {
    let mut p = PathBuilder::stroke(px(width));
    for (i, &(x, y)) in coords.iter().enumerate() {
        if i == 0 {
            p.move_to(point(x, y));
        } else {
            p.line_to(point(x, y));
        }
    }
    if let Ok(path) = p.build() {
        window.paint_path(path, color);
    }
}
fn polygon(window: &mut Window, coords: &[(gpui::Pixels, gpui::Pixels)], color: Hsla) {
    let mut p = PathBuilder::fill();
    for (i, &(x, y)) in coords.iter().enumerate() {
        if i == 0 {
            p.move_to(point(x, y));
        } else {
            p.line_to(point(x, y));
        }
    }
    p.close();
    if let Ok(path) = p.build() {
        window.paint_path(path, color);
    }
}

fn stack_color(t: &AuroraTheme, label: &str, index: usize) -> Hsla {
    let label = label.to_ascii_uppercase();
    if label.contains("DISABLE") {
        return t.danger.opacity(0.75);
    }
    if label.contains("ESTIMATED") && label.contains("BLOCK") {
        return t.success.opacity(0.4);
    }
    if label.contains("UNKNOWN") || label.contains("UNATTRIBUTED") {
        return t.muted_foreground.opacity(0.55);
    }
    if label.contains("THIRD PARTY") {
        return t.warning.opacity(0.85);
    }
    if label.contains("HIT") && !label.contains("NO_HIT") {
        return t.ring;
    }
    if label.contains("BLOCK") {
        return t.success.opacity(0.8);
    }
    if label.contains("MISS") || label.contains("EMPTY") {
        return t.warning.opacity(0.85);
    }
    if label.contains("UNRESOLVED") || label.contains("UNCERTAIN") {
        return t.muted_foreground.opacity(0.55);
    }
    [
        t.ring,
        t.warning.opacity(0.85),
        t.success.opacity(0.8),
        t.danger.opacity(0.75),
        t.muted_foreground.opacity(0.55),
    ][index % 5]
}

impl Chart {
    pub fn render(&self, t: &AuroraTheme) -> Div {
        self.render_with_bounds(t, None)
    }
    pub fn render_with_bounds(
        &self,
        t: &AuroraTheme,
        plot_bounds: Option<std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>>,
    ) -> Div {
        self.render_options(t, plot_bounds, Options::default())
    }
    pub(super) fn render_options(
        &self,
        t: &AuroraTheme,
        plot_bounds: Option<std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>>,
        options: Options,
    ) -> Div {
        if self.kind == Kind::Stacked {
            let total = self
                .samples
                .iter()
                .filter_map(|s| s.y)
                .filter(|n| n.is_finite() && *n >= 0.)
                .sum::<f64>();
            let mut bar = div()
                .flex()
                .w_full()
                .min_w(px(0.))
                .h(px(20.))
                .overflow_hidden();
            let mut labels = div().flex().flex_wrap().gap(px(16.));
            for (i, s) in self.samples.iter().enumerate() {
                let theme = t.clone();
                let label = s.label.clone();
                let n = s.y.filter(|n| n.is_finite() && *n >= 0.);
                let color = if self.id.contains("aim-grade") {
                    grade_color(t, &s.label)
                } else {
                    stack_color(t, &s.label, i)
                };
                labels = labels.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(div().w(px(10.)).h(px(10.)).rounded(px(2.)).bg(color))
                        .child(ui::caption(
                            t,
                            format!(
                                "{}: {}",
                                s.label,
                                n.map(|n| format!("{n:.0}"))
                                    .unwrap_or_else(|| "Unavailable".into())
                            ),
                        )),
                );
                if let Some(n) = n.filter(|n| *n > 0.) {
                    bar = bar.child(
                        div()
                            .id(format!("{}-stack-{i}", self.id))
                            .h_full()
                            .w(px(0.))
                            .flex_grow((n / total.max(1.)) as f32)
                            .min_w(px(0.))
                            .bg(color)
                            .role(Role::Image)
                            .aria_label(format!("{}: {n:.0}", s.label))
                            .tooltip(move |_, cx| {
                                cx.new(|_| Readout {
                                    text: super::presentation::text(&format!("{label}: {n:.0}")),
                                    theme: theme.clone(),
                                })
                                .into()
                            }),
                    );
                }
            }
            return ui::card(t)
                .p(px(16.))
                .gap(px(10.))
                .w_full()
                .min_w(px(0.))
                .child(ui::section_header(t, self.title.clone(), None))
                .when(self.id == "incoming-attribution", |d| {
                    d.child(ui::caption(
                        t,
                        "Only attributed opponent hits extend combos. Zero-count outcomes stay in the key.",
                    ))
                })
                .child(bar)
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(ui::caption(t, "0"))
                        .child(ui::caption(
                            t,
                            if self.samples.iter().any(|s| s.y.is_some()) {
                                format!("{total:.0} {}", self.x_label)
                            } else {
                                "Unavailable".into()
                            },
                        )),
                )
                .child(labels)
                .when(self.id != "incoming-attribution", |d| {
                    d.child(ui::caption(
                        t,
                        if self.id == "attack_outcomes" {
                            "Confirmed hits and observed attack outcomes.".into()
                        } else {
                            format!("{} Zero-count categories have no bar width.", self.note)
                        },
                    ))
                });
        }
        let chart = self.clone();
        let preferences = options.preferences;
        let regions = options.regions;
        let connectors = options.connectors;
        let selected_sample = options.selected_sample;
        let palette = t.clone();
        let available =
            !self.bands.is_empty() || self.samples.iter().any(|p| p.y.is_some_and(f64::is_finite));
        let description = format!(
            "{}. Horizontal axis: {} ({:.2} to {:.2}). Vertical axis: {} ({:.2} to {:.2}). {}. {}",
            self.title,
            self.x_label,
            self.domain.0,
            self.domain.1,
            self.y_label,
            self.range.0,
            self.range.1,
            if available {
                "Missing samples are gaps"
            } else {
                "Unavailable"
            },
            self.note
        );
        let plot_height = if self.kind == Kind::Lanes {
            (self.y_label.lines().count() as f32 * 24.).max(164.)
        } else if self.kind == Kind::Grid {
            240.
        } else if self.id.contains("trend") {
            96.
        } else {
            164.
        };
        let mut plot = div()
            .id(format!("{}-plot", self.id))
            .role(Role::Image)
            .aria_label(description.clone())
            .relative()
            .overflow_hidden()
            .w_full()
            .h(px(plot_height))
            .child(
                canvas(
                    move |bounds, _, _| {
                        if let Some(recorded) = &plot_bounds {
                            recorded.set(Some(bounds));
                        }
                    },
                    move |bounds, _, window, _| {
                        let left = bounds.origin.x;
                        let top = bounds.origin.y;
                        let width = bounds.size.width;
                        let height = bounds.size.height;
                        let x = |v| left + width * fraction(v, chart.domain);
                        let y = |v| top + height * (1. - fraction(v, chart.range));
                        for n in 0..=if preferences.grid { 4 } else { 0 } {
                            let yy = top + height * (n as f32 / 4.);
                            stroke(
                                window,
                                &[(left, yy), (left + width, yy)],
                                palette.border_subtle,
                                1.,
                            );
                        }
                        for &(cx, cy, rx, ry, grade) in &regions {
                            let coords = (0..=64)
                                .map(|i| {
                                    let a = i as f64 * std::f64::consts::TAU / 64.;
                                    (x(cx + rx * a.cos()), y(cy + ry * a.sin()))
                                })
                                .collect::<Vec<_>>();
                            let color = grade_color(
                                &palette,
                                ["IDEAL", "GOOD", "PERIPHERAL", "POOR"][grade.min(3)],
                            );
                            polygon(window, &coords, color.opacity(0.12));
                            stroke(window, &coords, color.opacity(0.55), 1.);
                        }
                        for band in chart
                            .bands
                            .iter()
                            .filter(|b| !b.selected)
                            .chain(chart.bands.iter().filter(|b| b.selected))
                        {
                            let (a, b) = if let Some(lane) = band.lane {
                                (y(lane) - px(13.), y(lane) + px(13.))
                            } else {
                                (top, top + height)
                            };
                            polygon(
                                window,
                                &[
                                    (x(band.start), a),
                                    (x(band.end), a),
                                    (x(band.end), b),
                                    (x(band.start), b),
                                ],
                                if band.selected {
                                    palette.ring.opacity(0.25)
                                } else {
                                    if band.lane.is_some() {
                                        palette.ring.opacity(0.28)
                                    } else {
                                        palette.control.opacity(0.2)
                                    }
                                },
                            );
                            stroke(
                                window,
                                &[(x(band.start), a), (x(band.start), b)],
                                palette.ring.opacity(0.65),
                                1.,
                            );
                            stroke(
                                window,
                                &[(x(band.end), a), (x(band.end), b)],
                                palette.ring.opacity(0.65),
                                1.,
                            );
                        }
                        for &(a, b, c, d) in &connectors {
                            stroke(
                                window,
                                &[(x(a), y(b)), (x(c), y(d))],
                                palette.ring.opacity(0.65),
                                1.5,
                            );
                        }
                        if let Some((a, b)) = chart.band {
                            let pts = if chart.kind == Kind::Histogram {
                                vec![
                                    (x(a), top),
                                    (x(b), top),
                                    (x(b), top + height),
                                    (x(a), top + height),
                                ]
                            } else {
                                vec![
                                    (left, y(a)),
                                    (left + width, y(a)),
                                    (left + width, y(b)),
                                    (left, y(b)),
                                ]
                            };
                            polygon(window, &pts, palette.accent_soft);
                        }
                        if let Some(target) = chart.target {
                            let pts = if chart.kind == Kind::Histogram {
                                vec![(x(target), top), (x(target), top + height)]
                            } else {
                                vec![(left, y(target)), (left + width, y(target))]
                            };
                            stroke(window, &pts, palette.muted_foreground, 1.);
                        }
                        if matches!(chart.kind, Kind::Line | Kind::Area) {
                            for run in segments(&chart.samples) {
                                let pts: Vec<_> = run.iter().map(|&(a, b)| (x(a), y(b))).collect();
                                if chart.kind == Kind::Area {
                                    // Fill each segment separately: a sign change never colors the opposite side.
                                    for pair in run.windows(2) {
                                        let (a, b) = pair[0];
                                        let (c, d) = pair[1];
                                        let color = if b >= 0. {
                                            palette.ring
                                        } else {
                                            palette.muted_foreground
                                        };
                                        if b * d < 0. {
                                            let cross = a + (c - a) * b / (b - d);
                                            polygon(
                                                window,
                                                &[(x(a), y(0.)), (x(a), y(b)), (x(cross), y(0.))],
                                                color.opacity(0.18),
                                            );
                                            polygon(
                                                window,
                                                &[(x(cross), y(0.)), (x(c), y(d)), (x(c), y(0.))],
                                                if d >= 0. {
                                                    palette.ring.opacity(0.18)
                                                } else {
                                                    palette.muted_foreground.opacity(0.18)
                                                },
                                            );
                                        } else {
                                            polygon(
                                                window,
                                                &[
                                                    (x(a), y(0.)),
                                                    (x(a), y(b)),
                                                    (x(c), y(d)),
                                                    (x(c), y(0.)),
                                                ],
                                                color.opacity(0.18),
                                            );
                                        }
                                    }
                                }
                                stroke(window, &pts, palette.ring, 2.);
                            }
                        }
                        let bar = width / (chart.samples.len().max(1) as f32);
                        for (sample_index, s) in chart.samples.iter().enumerate() {
                            let Some(v) = s.y.filter(|v| v.is_finite()) else {
                                if s.shape == Shape::Reset && s.x.is_finite() {
                                    let xx = x(s.x);
                                    stroke(
                                        window,
                                        &[
                                            (xx - px(3.5), top + height - px(4.)),
                                            (xx + px(3.5), top + height - px(4.)),
                                        ],
                                        palette.subtle_foreground,
                                        2.,
                                    );
                                }
                                continue;
                            };
                            let xx = x(s.x);
                            let yy = y(v);
                            if selected_sample == Some(sample_index) {
                                let r = px(7.);
                                let outline = (0..=24)
                                    .map(|i| {
                                        let a = i as f32 * std::f32::consts::TAU / 24.;
                                        (xx + r * a.cos(), yy + r * a.sin())
                                    })
                                    .collect::<Vec<_>>();
                                stroke(window, &outline, palette.foreground, 2.);
                            }
                            let color = if chart.kind == Kind::Histogram
                                && chart.band.is_some_and(|(a, b)| s.x >= a && s.x <= b)
                            {
                                palette.ring
                            } else {
                                palette.muted_foreground
                            };
                            match chart.kind {
                                Kind::Histogram => {
                                    let half = (bar * 0.4).min(px(14.));
                                    polygon(
                                        window,
                                        &[
                                            (xx - half, y(0.)),
                                            (xx - half, yy),
                                            (xx + half, yy),
                                            (xx + half, y(0.)),
                                        ],
                                        color.opacity(0.8),
                                    );
                                }
                                Kind::Scatter
                                | Kind::Timeline
                                | Kind::Lanes
                                | Kind::Grid
                                | Kind::Line => {
                                    if chart.kind == Kind::Line
                                        && (!preferences.points || chart.samples.len() > 120)
                                    {
                                        continue;
                                    }
                                    let xx = xx.clamp(left + px(4.), left + width - px(4.));
                                    let yy = yy.clamp(top + px(4.), top + height - px(4.));
                                    let r = px(3.5);
                                    let ink = if chart.kind == Kind::Lanes {
                                        match s.shape {
                                            Shape::Square => palette.ring,
                                            Shape::Diamond => palette.warning,
                                            Shape::Hollow | Shape::Plus => palette.success,
                                            Shape::Triangle | Shape::Cross => palette.danger,
                                            _ => palette.muted_foreground,
                                        }
                                    } else {
                                        grade_color(&palette, &s.label)
                                    };
                                    match s.shape {
                                        Shape::Square => polygon(
                                            window,
                                            &[
                                                (xx - r, yy - r),
                                                (xx + r, yy - r),
                                                (xx + r, yy + r),
                                                (xx - r, yy + r),
                                            ],
                                            ink,
                                        ),
                                        Shape::Diamond => polygon(
                                            window,
                                            &[
                                                (xx, yy - r - px(1.)),
                                                (xx + r + px(1.), yy),
                                                (xx, yy + r + px(1.)),
                                                (xx - r - px(1.), yy),
                                            ],
                                            ink,
                                        ),
                                        Shape::Hollow => {
                                            let circle = (0..=16)
                                                .map(|i| {
                                                    let angle =
                                                        i as f32 * std::f32::consts::TAU / 16.;
                                                    (xx + r * angle.cos(), yy + r * angle.sin())
                                                })
                                                .collect::<Vec<_>>();
                                            stroke(window, &circle, ink, 1.5);
                                        }
                                        Shape::Triangle => polygon(
                                            window,
                                            &[
                                                (xx, yy - r - px(1.)),
                                                (xx + r + px(1.), yy + r),
                                                (xx - r - px(1.), yy + r),
                                            ],
                                            ink,
                                        ),
                                        Shape::Cross => {
                                            stroke(
                                                window,
                                                &[(xx - r, yy - r), (xx + r, yy + r)],
                                                ink,
                                                1.5,
                                            );
                                            stroke(
                                                window,
                                                &[(xx - r, yy + r), (xx + r, yy - r)],
                                                ink,
                                                1.5,
                                            );
                                        }
                                        Shape::Plus => {
                                            stroke(window, &[(xx - r, yy), (xx + r, yy)], ink, 1.5);
                                            stroke(window, &[(xx, yy - r), (xx, yy + r)], ink, 1.5);
                                        }
                                        Shape::Tick => stroke(
                                            window,
                                            &[(xx, yy - r - px(2.)), (xx, yy + r + px(2.))],
                                            ink,
                                            2.,
                                        ),
                                        Shape::Reset => stroke(
                                            window,
                                            &[
                                                (xx - r, top + height - px(4.)),
                                                (xx + r, top + height - px(4.)),
                                            ],
                                            palette.subtle_foreground,
                                            2.,
                                        ),
                                    }
                                }
                                Kind::Area | Kind::Stacked => {}
                            }
                        }
                        for &(from, to) in &chart.incomplete {
                            let mut xx = x(from);
                            let end = x(to);
                            while xx < end {
                                stroke(
                                    window,
                                    &[(xx, top), (xx, top + height)],
                                    palette.muted_foreground.opacity(0.35),
                                    1.,
                                );
                                xx += px(6.);
                            }
                        }
                        if let Some(head) = chart.playhead {
                            stroke(
                                window,
                                &[(x(head), top), (x(head), top + height)],
                                palette.foreground,
                                3.,
                            );
                        }
                    },
                )
                .size_full(),
            );
        if let Some(head) = self.playhead.filter(|_| self.kind != Kind::Lanes) {
            plot = plot.child(
                div()
                    .absolute()
                    .left(relative(fraction(head, self.domain).min(0.85)))
                    .top_0()
                    .px(px(6.))
                    .py(px(2.))
                    .rounded(t.radius)
                    .bg(t.foreground)
                    .text_color(t.background)
                    .text_size(px(ui::TEXT_CAPTION))
                    .child(format!("{head:.1} s")),
            );
        }
        // Each measured point has a native hover readout and exposed text. Paths do no I/O.
        for (i, s) in self
            .samples
            .iter()
            .enumerate()
            .take(2048)
            .filter(|(_, s)| s.y.is_some())
        {
            let theme = t.clone();
            let label = s.label.clone();
            let tip = label.clone();
            plot = plot.child(
                div()
                    .id(format!("{}-sample-{i}", self.id))
                    .absolute()
                    .left(relative(fraction(s.x, self.domain)))
                    .top(relative(
                        1. - fraction(s.y.unwrap_or(self.range.0), self.range),
                    ))
                    .w(px(12.))
                    .h(px(12.))
                    .mt(px(-6.))
                    .ml(px(-6.))
                    .role(if options.activate.is_some() {
                        Role::Button
                    } else {
                        Role::Image
                    })
                    .aria_label(label)
                    .tab_index(0)
                    .on_key_down(|e, w, cx| {
                        match e.keystroke.key.as_str() {
                            "left" | "up" => w.focus_prev(cx),
                            "right" | "down" => w.focus_next(cx),
                            _ => return,
                        }
                        cx.stop_propagation();
                    })
                    .focus_visible(|s| s.border_1().border_color(t.ring))
                    .when_some(options.activate.clone(), |d, a| {
                        let k = a.clone();
                        d.cursor_pointer()
                            .on_click(move |_, w, cx| a(Target::Sample(i), w, cx))
                            .on_key_down(move |e, w, cx| {
                                if matches!(e.keystroke.key.as_str(), "enter" | "space" | " ") {
                                    k(Target::Sample(i), w, cx);
                                    cx.stop_propagation();
                                }
                            })
                    })
                    .tooltip(move |_, cx| {
                        cx.new(|_| Readout {
                            text: tip.clone(),
                            theme: theme.clone(),
                        })
                        .into()
                    }),
            );
        }
        for (i, b) in self.bands.iter().enumerate() {
            let theme = t.clone();
            let label = b.label.clone();
            plot = plot.child(
                div()
                    .id(format!("{}-band-{i}", self.id))
                    .absolute()
                    .left(relative(fraction(b.start, self.domain)))
                    .top(relative(
                        b.lane.map(|v| 1. - fraction(v, self.range)).unwrap_or(0.),
                    ))
                    .w(relative(
                        (fraction(b.end, self.domain) - fraction(b.start, self.domain)).max(0.01),
                    ))
                    .h(px(20.))
                    .mt(px(-10.))
                    .role(Role::Image)
                    .aria_label(label.clone())
                    .tab_index(0)
                    .on_key_down(|e, w, cx| {
                        match e.keystroke.key.as_str() {
                            "left" | "up" => w.focus_prev(cx),
                            "right" | "down" => w.focus_next(cx),
                            _ => return,
                        }
                        cx.stop_propagation();
                    })
                    .focus_visible(|s| s.border_1().border_color(t.ring))
                    .when_some(options.activate.clone(), |d, a| {
                        let k = a.clone();
                        d.cursor_pointer()
                            .on_click(move |_, w, cx| a(Target::Band(i), w, cx))
                            .on_key_down(move |e, w, cx| {
                                if matches!(e.keystroke.key.as_str(), "enter" | "space" | " ") {
                                    k(Target::Band(i), w, cx);
                                    cx.stop_propagation();
                                }
                            })
                    })
                    .tooltip(move |_, cx| {
                        cx.new(|_| Readout {
                            text: super::presentation::text(&label),
                            theme: theme.clone(),
                        })
                        .into()
                    }),
            );
        }
        ui::card(t)
            .when(self.id.contains("trend"), |d| d.gap(px(8.)))
            .min_w(px(0.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(ui::section_header(t, self.title.clone(), None))
                    .when(self.kind == Kind::Lanes, |d| {
                        d.child(ui::caption(
                            t,
                            format!("Playhead {:.1} s", self.playhead.unwrap_or(0.)),
                        ))
                    })
                    .when(self.id.contains("trend"), |d| {
                        d.child(ui::caption(t, self.x_label.clone()))
                    }),
            )
            .when(self.id.contains("trend"), |d| {
                d.child(ui::caption(t, self.note.clone()))
            })
            .when(
                self.kind != Kind::Lanes && !self.id.contains("trend"),
                |d| d.child(ui::caption(t, self.y_label.clone())),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(if self.kind == Kind::Lanes {
                        div()
                            .w(px(92.))
                            .flex_shrink_0()
                            .h(px(plot_height))
                            .flex()
                            .flex_col()
                            .children(self.y_label.lines().rev().map(|label| {
                                div()
                                    .h(px(24.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .child(ui::caption(t, label.to_owned()))
                            }))
                    } else {
                        let percent = self.y_label.contains('%');
                        let combo = self.kind == Kind::Timeline && self.range == (-0.5, 1.5);
                        let labels = if combo {
                            vec!["You".into(), "Opponent".into()]
                        } else {
                            (0..=4)
                                .map(|i| {
                                    let v = self.range.1
                                        - (self.range.1 - self.range.0) * i as f64 / 4.;
                                    if percent {
                                        format!("{v:.0}%")
                                    } else {
                                        format!("{v:.1}")
                                    }
                                })
                                .collect()
                        };
                        div()
                            .w(px(if combo { 64. } else { 44. }))
                            .flex_shrink_0()
                            .h(px(plot_height))
                            .relative()
                            .children(labels.into_iter().enumerate().map(|(i, v)| {
                                let position = if combo {
                                    0.25 + i as f32 * 0.5
                                } else {
                                    i as f32 / 4.
                                };
                                ui::caption(t, v)
                                    .absolute()
                                    .top(px(plot_height * position - 8.))
                                    .h(px(16.))
                                    .line_height(px(16.))
                            }))
                    })
                    .child(plot.flex_1().when(!available, |d| {
                        d.child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child("Unavailable"),
                        )
                    })),
            )
            .child(
                div()
                    .pl(px(if self.kind == Kind::Lanes { 104. } else { 56. }))
                    .flex()
                    .justify_between()
                    .children((0..=4).map(|i| {
                        let label = if self.id.contains("trend") {
                            self.samples
                                .get((self.samples.len().saturating_sub(1) * i) / 4)
                                .map(|s| axis_date(&s.label))
                                .unwrap_or("Unavailable")
                                .to_owned()
                        } else {
                            format!(
                                "{:.1}",
                                self.domain.0 + (self.domain.1 - self.domain.0) * i as f64 / 4.
                            )
                        };
                        ui::caption(t, label)
                    })),
            )
            .when(!self.id.contains("trend"), |d| {
                d.child(ui::caption(t, self.x_label.clone()).text_center())
            })
            .when(!self.id.contains("trend"), |d| {
                d.child(ui::caption(t, self.note.clone()))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bins_preserve_boundaries_and_missing() {
        let b = reach_bins([1.9, 2., 2.05, 3.999, 4., f64::NAN, -1.]);
        assert_eq!(b.iter().sum::<u64>(), 5);
        assert_eq!((b[0], b[1], b[2], b[40], b[41]), (1, 1, 1, 1, 1));
    }
    #[test]
    fn gaps_split_lines_instead_of_zero_filling() {
        let s = |x, y| Sample {
            x,
            y,
            label: String::new(),
            shape: Shape::Square,
        };
        assert_eq!(
            segments(&[s(0., Some(2.)), s(1., None), s(2., Some(0.))]),
            vec![vec![(0., 2.)], vec![(2., 0.)]]
        );
    }
    #[test]
    fn scales_are_finite_and_bounded() {
        assert_eq!(fraction(1., (1., 1.)), 0.);
        assert_eq!(fraction(-2., (0., 10.)), 0.);
        assert_eq!(fraction(20., (0., 10.)), 1.);
        assert_eq!(nice_peak(23.), 50.);
        assert_eq!(nice_peak(f64::NAN), 1.);
    }
}

fn axis_date(label: &str) -> &str {
    let part = label.split_once(": ").map(|(_, s)| s).unwrap_or(label);
    let date = part.split(';').next().unwrap_or(part).trim();
    if date.chars().filter(|c| c.is_ascii_digit()).count() >= 4 && date.contains('-') {
        date
    } else {
        "Date unavailable"
    }
}

#[cfg(test)]
mod preference_tests {
    use super::*;
    use aurora_launcher_lib::pulsar::view::{SettingView, SettingsView};
    #[test]
    fn native_chart_options_follow_saved_flags_and_defaults() {
        let settings = SettingsView {
            settings: [
                ("chartGrid", false),
                ("chartPoints", false),
                ("reducedMotion", true),
            ]
            .into_iter()
            .map(|(k, v)| SettingView {
                key: k.into(),
                value: serde_json::json!(v),
                ..Default::default()
            })
            .collect(),
            ..Default::default()
        };
        let p = Preferences::from_settings(&settings);
        assert!(!p.grid && !p.points && p.reduced_motion);
        let p = Preferences::from_settings(&SettingsView::default());
        assert!(p.grid && p.points && !p.reduced_motion);
    }
    #[test]
    fn trend_axis_uses_the_recorded_date_label() {
        assert_eq!(
            axis_date("Accuracy: 10-26 12:00; 90.0% - 90 %"),
            "10-26 12:00"
        );
        assert_eq!(axis_date("Accuracy: 2026-10-26; unavailable"), "2026-10-26");
        assert_eq!(axis_date("Unavailable"), "Date unavailable");
    }
}
