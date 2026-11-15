//! Display projections from the core's normalized analytical payload.
//! This module never opens recordings or reports, and never reclassifies events.
use super::{
    Pulsar,
    charts::{Chart, Kind, Sample, Shape, nice_peak, reach_bins},
    tabs,
};
use crate::{
    controls::{AuroraButton, ButtonSize, ButtonVariant},
    theme::AuroraTheme,
    ui,
};
use gpui::{Context, Div, Role, div, prelude::*, px};
use serde_json::Value;

fn rows<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn num(v: &Value, key: &str) -> Option<f64> {
    v[key].as_f64().filter(|v| v.is_finite())
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("Unavailable")
}
fn format(v: Option<f64>, unit: &str) -> String {
    v.map(|v| format!("{v:.2} {unit}"))
        .unwrap_or_else(|| "Unavailable".into())
}
fn button(id: impl Into<gpui::ElementId>, label: impl Into<gpui::SharedString>) -> AuroraButton {
    AuroraButton::new(id, label)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Small)
}
fn plot(
    id: &str,
    title: &str,
    x: &str,
    y: &str,
    kind: Kind,
    samples: Vec<Sample>,
    domain: (f64, f64),
    range: (f64, f64),
    note: &str,
) -> Chart {
    Chart {
        id: id.into(),
        title: title.into(),
        x_label: x.into(),
        y_label: y.into(),
        samples,
        kind,
        domain,
        range,
        band: None,
        target: None,
        playhead: None,
        bands: vec![],
        incomplete: vec![],
        note: note.into(),
    }
}
fn sample(x: f64, y: Option<f64>, label: String, shape: Shape) -> Sample {
    Sample { x, y, label, shape }
}

pub(super) fn density(samples: &[(f64, f64)]) -> [[u64; 6]; 12] {
    let mut grid = [[0; 6]; 12];
    for &(x, y) in samples {
        if x.is_finite() && y.is_finite() && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) {
            grid[((1. - y) * 12.).floor().min(11.) as usize][(x * 6.).floor().min(5.) as usize] +=
                1;
        }
    }
    grid
}
pub(super) fn face_axes(face: &str, x: f64, y: f64, z: f64) -> (f64, f64) {
    match face {
        "TOP" | "BOTTOM" => (x, z),
        "EAST" | "WEST" => (z, y),
        _ => (x, y),
    }
}

impl Pulsar {
    pub(super) fn advanced_body(
        &self,
        t: &AuroraTheme,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut body = div().flex().flex_col().gap(px(24.)).min_w(px(0.));
        let Some(detail) = &self.detail else {
            return body;
        };
        let d = &detail.derived;
        match self.detail_tab {
            1 => {
                let weapons: Vec<_> = d["reach"]["byWeapon"]
                    .as_object()
                    .map(|m| m.keys().cloned().collect())
                    .unwrap_or_default();
                let mut filters = div().flex().flex_wrap().gap(px(4.)).child(
                    button("detail-weapon-all", "All weapons")
                        .selected(self.weapon.is_none())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.weapon = None;
                            this.aim_selection = None;
                            cx.notify();
                        })),
                );
                for (i, w) in weapons.iter().enumerate() {
                    let w = w.clone();
                    filters = filters.child(
                        button(
                            ("detail-weapon", i),
                            w.strip_prefix("minecraft:").unwrap_or(&w).replace('_', " "),
                        )
                        .selected(self.weapon.as_ref() == Some(&w))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.weapon = Some(w.clone());
                            this.aim_selection = None;
                            cx.notify();
                        })),
                    );
                }
                body = body.child(filters);
                body = body.child(self.outcomes(t, d));
                let target = num(&d["reach"], "target");
                let tolerance = num(&d["reach"], "tolerance");
                let hits: Vec<_> = rows(d, "hits")
                    .iter()
                    .filter(|h| self.weapon.as_ref().is_none_or(|w| text(h, "weapon") == w))
                    .collect();
                let reaches: Vec<_> = hits
                    .iter()
                    .filter(|h| h["contact"].as_bool() == Some(true))
                    .filter_map(|h| num(h, "reach"))
                    .collect();
                let bins = reach_bins(reaches.iter().copied());
                let peak = nice_peak(bins.iter().copied().max().unwrap_or(0) as f64);
                let samples = bins
                    .iter()
                    .enumerate()
                    .map(|(i, count)| {
                        let midpoint = if i == 0 {
                            1.95
                        } else if i == 41 {
                            4.05
                        } else {
                            2. + (i as f64 - 0.5) * 0.05
                        };
                        sample(
                            midpoint,
                            Some(*count as f64),
                            format!(
                                "{} · {count} eligible hits",
                                if i == 0 {
                                    "Below 2.00 m".into()
                                } else if i == 41 {
                                    "4.00 m and above".into()
                                } else {
                                    format!("{:.2}–{:.2} m", midpoint - 0.025, midpoint + 0.025)
                                }
                            ),
                            Shape::Square,
                        )
                    })
                    .collect();
                let mut histogram = plot(
                    "detail-reach-hist",
                    "Reach & spacing",
                    "Contact distance (m)",
                    "Confirmed hit count",
                    Kind::Histogram,
                    samples,
                    (1.925, 4.075),
                    (0., peak),
                    "0.05 m bins · target band and line use this recording’s frozen policy. Creative distances are unavailable for survival comparison.",
                );
                histogram.band = target.zip(tolerance).map(|(a, b)| (a - b, a + b));
                histogram.target = target;
                body = body.child(histogram.render(t));
                let within = target.zip(tolerance).map(|(a, b)| {
                    reaches
                        .iter()
                        .filter(|v| (**v - a).abs() <= b + 1e-9)
                        .count()
                });
                let mean = (!reaches.is_empty())
                    .then(|| reaches.iter().sum::<f64>() / reaches.len() as f64);
                body = body.child(
                    ui::card(t).child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(24.))
                            .child(ui::stat(
                                t,
                                "Eligible reach samples",
                                reaches.len().to_string(),
                            ))
                            .child(ui::stat(
                                t,
                                "In target window",
                                within
                                    .map(|n| format!("{n} / {}", reaches.len()))
                                    .unwrap_or_else(|| "Unavailable".into()),
                            ))
                            .child(ui::stat(t, "Mean contact distance", format(mean, "m")))
                            .child(ui::stat(t, "Frozen target", format(target, "m"))),
                    ),
                );
                let points = hits
                    .iter()
                    .filter_map(|h| {
                        Some(sample(
                            num(h, "t")?,
                            num(h, "reach"),
                            format!(
                                "{} s · {} · {}",
                                format(num(h, "t"), ""),
                                text(h, "weapon"),
                                format(num(h, "reach"), "m")
                            ),
                            Shape::Square,
                        ))
                    })
                    .chain(rows(d, "misses").iter().filter_map(|h| {
                        Some(sample(
                            num(h, "t")?,
                            num(h, "estimatedReach"),
                            format!(
                                "{:.2} s · estimated no-hit distance, not a confirmed hit",
                                num(h, "t")?
                            ),
                            Shape::Hollow,
                        ))
                    }))
                    .collect();
                body=body.child(plot("detail-reach-strip","Individual contacts","Recording time (s)","Distance (m)",Kind::Scatter,points,(0.,self.duration().max(1.)),(0.,nice_peak(reaches.iter().copied().fold(4.,f64::max))),"■ Confirmed hit · ◇ outline Estimated no-hit contact. Geometry is observed, not a server verdict.").render(t));
                body = body.child(self.aim_view(t, d, cx));
                let samples: Vec<_> = rows(d, "timingSamples")
                    .iter()
                    .filter(|s| self.weapon.as_ref().is_none_or(|w| text(s, "weapon") == w))
                    .filter(|s| {
                        self.timing_result.as_deref().is_none_or(|r| {
                            if r == "HIT" {
                                text(s, "outcome") == "HIT"
                            } else {
                                text(s, "outcome") != "HIT"
                            }
                        })
                    })
                    .collect();
                let mut timing_filters = div().flex().flex_wrap().gap(px(4.));
                for (i, (label, result)) in [
                    ("All attempts", None),
                    ("Hits", Some("HIT")),
                    ("No hit", Some("NO_HIT")),
                ]
                .into_iter()
                .enumerate()
                {
                    timing_filters = timing_filters.child(
                        button(("timing-result", i), label)
                            .selected(self.timing_result.as_deref() == result)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.timing_result = result.map(str::to_owned);
                                cx.notify();
                            })),
                    );
                }
                body = body.child(timing_filters);
                let range = (samples
                    .iter()
                    .filter_map(|s| num(s, "delta"))
                    .map(f64::abs)
                    .fold(4., f64::max)
                    + 1.)
                    .ceil()
                    .min(40.);
                let points = samples
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        sample(
                            i as f64 + 1.,
                            num(s, "delta"),
                            format!(
                                "Attempt {} · {} · {} · delta {} · expected {} · interval {} · {}",
                                i + 1,
                                text(s, "weapon"),
                                text(s, "grade"),
                                format(num(s, "delta"), "ticks"),
                                format(num(s, "expected"), "ticks"),
                                format(num(s, "interval"), "ticks"),
                                text(s, "outcome")
                            ),
                            if num(s, "delta").is_none() {
                                Shape::Reset
                            } else if text(s, "outcome").contains("SHIELD") {
                                Shape::Diamond
                            } else if text(s, "outcome") == "HIT" {
                                Shape::Square
                            } else {
                                Shape::Hollow
                            },
                        )
                    })
                    .collect();
                let mut timing = plot(
                    "detail-timing",
                    "Weapon timing",
                    "Attempt ordinal",
                    "Interval − expected cooldown (ticks)",
                    Kind::Scatter,
                    points,
                    (0., samples.len().max(1) as f64 + 1.),
                    (-range, range),
                    "■ Hit · ◆ Shield · ◇ outline No hit · — Opener/reset/unavailable. Mechanical cooldown matching is not tactical timing.",
                );
                timing.target = Some(0.);
                timing.band = num(&d["policy"], "timingToleranceTicks").map(|v| (-v, v));
                body = body.child(timing.render(t));
                let mut list=ui::list_card(t).child(ui::row(t,false).child(ui::section_header(t,"Latest timing samples",Some("Newest 50 · values are frozen per weapon; opener intervals remain unavailable.".into()))));
                for (i, s) in samples
                    .iter()
                    .skip(samples.len().saturating_sub(50))
                    .enumerate()
                {
                    list=list.child(ui::row(t,i+1==samples.len().min(50)).child(ui::title_block(t,format!("{} · {} · {}",text(s,"weapon"),text(s,"grade"),text(s,"outcome")),Some(format!("{:.2} s · speed {} · expected {} · interval {} · difference {}",num(s,"t").unwrap_or(0.),format(num(s,"attackSpeed"),"/s"),format(num(s,"expected"),"ticks"),format(num(s,"interval"),"ticks"),format(num(s,"delta"),"ticks")).into()))));
                }
                body = body.child(list);
            }
            2 => {
                let incoming = rows(d, "incomingDetail");
                let points = incoming
                    .iter()
                    .filter_map(|r| {
                        num(r, "jumpOffsetTicks").map(|v| {
                            sample(
                                num(r, "time").unwrap_or(0.),
                                Some(v.clamp(-10., 10.)),
                                format!(
                                    "{:.2} s · signed offset {v:+.0} ticks · {} · {}",
                                    num(r, "time").unwrap_or(0.),
                                    text(r, "jumpClass"),
                                    text(r, "attribution")
                                ),
                                if v == 0. {
                                    Shape::Diamond
                                } else {
                                    Shape::Square
                                },
                            )
                        })
                    })
                    .collect();
                let mut jump = plot(
                    "detail-jump-offset",
                    "Jump timing offsets",
                    "Incoming hit time (s)",
                    "Jump − incoming (client ticks)",
                    Kind::Scatter,
                    points,
                    (0., self.duration().max(1.)),
                    (-10., 10.),
                    "◆ Same tick · ■ Other measured offset. Candidate timing does not establish knockback reduction. Display clamps to ±10 ticks; hover gives the exact offset.",
                );
                jump.target = Some(0.);
                jump.band = num(d, "jumpWindowTicks").map(|w| (-w, w));
                body = body.child(jump.render(t));
                let lanes = rows(d, "incoming")
                    .iter()
                    .filter_map(|r| r.as_f64())
                    .map(|v| {
                        sample(
                            v,
                            Some(0.),
                            format!("{v:.2} s · attributed incoming"),
                            Shape::Square,
                        )
                    })
                    .chain(rows(d, "jumps").iter().filter_map(|j| {
                        num(j, "t").map(|time| {
                            sample(
                                time,
                                Some(1.),
                                format!(
                                    "{time:.2} s · actual jump · {} · {}",
                                    text(j, "classification"),
                                    format(num(j, "offsetTicks"), "ticks")
                                ),
                                Shape::Hollow,
                            )
                        })
                    }))
                    .collect();
                body=body.child(plot("detail-jump-lanes","Incoming hits & actual jumps","Recording time (s)","Bottom: incoming · top: actual jump",Kind::Lanes,lanes,(0.,self.duration().max(1.)),(-0.5,1.5),"■ Incoming hit · ◇ outline Actual observed jump. Client tick timing is affected by network delay; this is not a world replay.").render(t));
                let mut list=ui::list_card(t).child(ui::row(t,false).child(ui::section_header(t,"Incoming evidence",Some("Observed health loss and velocity are nullable. Other-entity damage is separate from opponent retaliation.".into()))));
                for (i, r) in incoming.iter().enumerate() {
                    list = list.child(
                        ui::row(t, i + 1 == incoming.len()).child(ui::title_block(
                            t,
                            format!(
                                "{:.2} s · {} · {}",
                                num(r, "time").unwrap_or(0.),
                                text(r, "attribution"),
                                text(r, "damageType")
                            ),
                            Some(
                                format!(
                                    "Health lost {} · knockback {} · jump {} ({}) · {}",
                                    format(num(r, "damage"), "HP"),
                                    format(num(r, "knockback"), "blocks/tick"),
                                    format(num(r, "jumpOffsetTicks"), "ticks"),
                                    text(r, "jumpClass"),
                                    if r["crit"].as_bool() == Some(true) {
                                        "server critical animation"
                                    } else {
                                        "no observed critical animation"
                                    }
                                )
                                .into(),
                            ),
                        )),
                    );
                }
                body=body.child(list).child(ui::caption(t,"Jump reset success: Not measured. Experimental speed comparison requires at least three qualified samples in each group and remains confounded by movement, armor and knockback context."));
            }
            3 => {
                let streaks = rows(&d["combos"], "streaks");
                let marks = rows(&d["combos"], "marks");
                let points = marks
                    .iter()
                    .filter_map(|m| {
                        num(m, "t").map(|time| {
                            sample(
                                time,
                                Some(if text(m, "side") == "LOCAL" { 1. } else { 0. }),
                                format!(
                                    "{time:.2} s · {}{}",
                                    text(m, "side"),
                                    if m["traded"].as_bool() == Some(true) {
                                        " · same-tick trade"
                                    } else {
                                        ""
                                    }
                                ),
                                if m["traded"].as_bool() == Some(true) {
                                    Shape::Diamond
                                } else {
                                    Shape::Square
                                },
                            )
                        })
                    })
                    .collect();
                let mut combo = plot(
                    "detail-combo-lanes",
                    "Combo timeline",
                    "Evidence time (s)",
                    "Bottom: opponent · top: you",
                    Kind::Timeline,
                    points,
                    (0., self.duration().max(1.)),
                    (-0.5, 1.5),
                    "■ Confirmed hit · ◆ Same-tick trade, extends neither streak. Combo bands use server-evidence time.",
                );
                combo.bands = streaks
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| {
                        Some(super::charts::Band {
                            start: num(s, "start")?,
                            end: num(s, "end")?.max(num(s, "start")? + 0.04),
                            lane: Some(if text(s, "side") == "LOCAL" { 1. } else { 0. }),
                            label: format!(
                                "{}: {} hits, {}",
                                text(s, "side"),
                                s["length"],
                                text(s, "endedBy")
                            ),
                            selected: i == self.combo_selection,
                        })
                    })
                    .collect();
                body = body.child(combo.render(t));
                let mut selectors = div().flex().flex_wrap().gap(px(4.));
                selectors = selectors
                    .child(
                        button("combo-previous", "Previous combo")
                            .disabled(streaks.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(d) = &this.detail {
                                    let n = rows(&d.derived["combos"], "streaks").len();
                                    if n > 0 {
                                        this.combo_selection = (this.combo_selection + n - 1) % n;
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        button("combo-next", "Next combo")
                            .disabled(streaks.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(d) = &this.detail {
                                    let n = rows(&d.derived["combos"], "streaks").len();
                                    if n > 0 {
                                        this.combo_selection = (this.combo_selection + 1) % n;
                                    }
                                }
                                cx.notify();
                            })),
                    );
                for (i, s) in streaks.iter().enumerate() {
                    selectors = selectors.child(
                        button(
                            ("combo-select", i),
                            format!("{} · {} hits", text(s, "side"), s["length"]),
                        )
                        .selected(self.combo_selection == i)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.combo_selection = i;
                            cx.notify();
                        })),
                    );
                }
                body = body.child(selectors);
                if let Some(s) = streaks.get(self.combo_selection) {
                    let mut card = ui::card(t).child(ui::section_header(
                        t,
                        format!("{} · {} consecutive hits", text(s, "side"), s["length"]),
                        Some(
                            format!(
                                "{} – {} · ended by {}",
                                format(num(s, "start"), "s"),
                                format(num(s, "end"), "s"),
                                text(s, "endedBy")
                            )
                            .into(),
                        ),
                    ));
                    for seq in rows(s, "seqs") {
                        if let Some(h) = rows(d, "hits").iter().find(|h| h["seq"] == *seq) {
                            card = card.child(ui::caption(
                                t,
                                format!(
                                    "Hit {} · evidence {} · {} · {} · critical conditions {}",
                                    seq,
                                    format(num(h, "evidenceT"), "s"),
                                    format(num(h, "reach"), "m"),
                                    text(h, "weapon"),
                                    h["critical"].as_bool().unwrap_or(false)
                                ),
                            ));
                        }
                    }
                    body = body.child(card);
                }
                let dom = &d["dominance"];
                let step = num(dom, "step").unwrap_or(0.);
                let points = rows(dom, "values")
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        sample(
                            i as f64 * step,
                            v.as_f64(),
                            format!(
                                "{:.2} s · {} · {}",
                                i as f64 * step,
                                v.as_f64()
                                    .map(|v| format!("{v:+.4}"))
                                    .unwrap_or_else(|| "Unavailable / inactive".into()),
                                dom["states"]
                                    .as_str()
                                    .and_then(|s| s.get(i..i + 1))
                                    .unwrap_or("")
                            ),
                            Shape::Square,
                        )
                    })
                    .collect();
                let mut chart = plot(
                    "detail-dominance",
                    "Encounter dominance",
                    "Recording time (s)",
                    "Signed evidence balance (−1 to +1)",
                    Kind::Area,
                    points,
                    (0., self.duration().max(1.)),
                    (-1., 1.),
                    "Above zero: you · below: opponent · ±0.2 neutral band · gaps: inactive / insufficient. Incomplete periods are excluded from shares. Decay τ = 3 s; no accuracy, aim or reach input.",
                );
                chart.band = Some((-0.2, 0.2));
                chart.target = Some(0.);
                if let Some(states) = dom["states"].as_str() {
                    let mut from = None;
                    for (i, state) in states.bytes().chain(std::iter::once(b'I')).enumerate() {
                        if state == b'C' {
                            if from.is_none() {
                                from = Some(i as f64 * step);
                            }
                        } else if let Some(start) = from.take() {
                            chart.incomplete.push((start, i as f64 * step));
                        }
                    }
                }
                body = body.child(chart.render(t));
            }
            _ => {}
        }
        let _ = (narrow, tabs::metric_value);
        body
    }
    fn outcomes(&self, t: &AuroraTheme, d: &Value) -> Div {
        let s = &d["summary"];
        let entries = [
            ("Confirmed hit", num(s, "hits")),
            ("Confirmed shield", num(s, "shieldBlocks")),
            ("Estimated shield", num(s, "estimatedShieldBlocks")),
            (
                "Other uncertain",
                num(s, "uncertain")
                    .zip(num(s, "estimatedShieldBlocks"))
                    .map(|(a, b)| (a - b).max(0.)),
            ),
            ("Eligible miss", num(s, "emptyMisses")),
        ];
        let total = entries.iter().filter_map(|(_, v)| *v).sum::<f64>();
        let mut bar = div().flex().h(px(16.)).w_full().gap(px(2.));
        for (i, (label, value)) in entries.iter().enumerate() {
            if let Some(v) = value.filter(|v| *v > 0.) {
                bar = bar.child(
                    div()
                        .id(("outcome-segment", i))
                        .role(Role::Image)
                        .aria_label(format!("{label}: {v:.0}"))
                        .w(gpui::relative((v / total.max(1.)) as f32))
                        .h_full()
                        .bg(if i == 0 {
                            t.ring
                        } else {
                            t.muted_foreground.opacity(0.9 - i as f32 * 0.12)
                        }),
                );
            }
        }
        ui::card(t).child(ui::section_header(t,"Attempt outcomes",None)).child(bar).child(div().flex().flex_wrap().gap(px(16.)).children(entries.into_iter().map(|(label,n)|ui::stat(t,label,n.map(|v|format!("{v:.0}")).unwrap_or_else(||"Unavailable".into())))))
            .child(ui::caption(t,"Combat accuracy = confirmed hits / (eligible attempts − confirmed shield blocks). Estimated blocks and unresolved attacks remain in the denominator."))
    }
    fn aim_view(&self, t: &AuroraTheme, d: &Value, cx: &mut Context<Self>) -> Div {
        let all = rows(&d["aim"], "samples");
        let duration = self.duration();
        let samples: Vec<_> = all
            .iter()
            .filter(|s| self.weapon.as_ref().is_none_or(|w| text(s, "weapon") == w))
            .filter(|s| {
                let time = num(s, "t").unwrap_or(0.);
                match self.aim_third {
                    1 => time < duration / 3.,
                    2 => time >= duration / 3. && time < duration * 2. / 3.,
                    3 => time >= duration * 2. / 3.,
                    _ => true,
                }
            })
            .collect();
        let selected = self
            .aim_selection
            .and_then(|seq| {
                samples
                    .iter()
                    .find(|s| s["seq"].as_i64() == Some(seq))
                    .copied()
            })
            .or_else(|| samples.last().copied());
        let face = selected.map(|s| text(s, "face")).unwrap_or("Unavailable");
        let contacts: Vec<_> = samples
            .iter()
            .copied()
            .filter(|s| text(s, "face") == face)
            .collect();
        let hit_contacts: Vec<_> = contacts
            .iter()
            .filter(|s| s["hit"].as_bool() == Some(true))
            .filter_map(|s| Some(face_axes(face, num(s, "x")?, num(s, "y")?, num(s, "z")?)))
            .collect();
        let grid = density(&hit_contacts);
        let peak = grid.iter().flatten().copied().max().unwrap_or(0);
        let mut card = ui::card(t).child(ui::section_header(
            t,
            "Contact & aim",
            Some(
                format!(
                    "Selected face: {face} · normalized face coordinates · geometry is not damage"
                )
                .into(),
            ),
        ));
        let mut filters = div().flex().flex_wrap().gap(px(4.)).child(
            button(
                "aim-density",
                if self.aim_density {
                    "Contact points"
                } else {
                    "Hit density"
                },
            )
            .selected(self.aim_density)
            .on_click(cx.listener(|this, _, _, cx| {
                this.aim_density = !this.aim_density;
                cx.notify();
            })),
        );
        for (i, label) in ["Whole fight", "First third", "Middle third", "Last third"]
            .into_iter()
            .enumerate()
        {
            filters = filters.child(
                button(("aim-third", i), label)
                    .selected(self.aim_third == i)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.aim_third = i;
                        this.aim_selection = None;
                        cx.notify();
                    })),
            );
        }
        card = card.child(filters);
        if self.aim_density {
            let mut cells = div().w(px(144.)).flex().flex_col().gap(px(2.));
            for (r, row) in grid.iter().enumerate() {
                cells = cells.child(
                    div()
                        .flex()
                        .gap(px(2.))
                        .children(row.iter().enumerate().map(|(c, n)| {
                            div()
                                .id(format!("aim-cell-{r}-{c}"))
                                .role(Role::Image)
                                .aria_label(format!(
                                    "Face {face}, row {}, column {}, {n} confirmed hits",
                                    r + 1,
                                    c + 1
                                ))
                                .w(px(22.))
                                .h(px(18.))
                                .bg(if *n == 0 {
                                    t.sunken
                                } else {
                                    t.ring.opacity(0.2 + 0.8 * (*n as f32 / peak.max(1) as f32))
                                })
                        })),
                );
            }
            card=card.child(cells).child(ui::caption(t,"6 columns × 12 rows · top = high contact · left to right = normalized horizontal face axis. Only measured confirmed contacts contribute."));
        } else {
            let points = contacts
                .iter()
                .filter_map(|s| {
                    let (x, y) = face_axes(face, num(s, "x")?, num(s, "y")?, num(s, "z")?);
                    Some(sample(
                        x,
                        Some(y),
                        format!(
                            "Hit {} · {} · distance {} · {}",
                            s["seq"],
                            text(s, "grade"),
                            format(num(s, "distance"), "m"),
                            if s["hit"].as_bool() == Some(true) {
                                "confirmed hit"
                            } else {
                                "other measured contact"
                            }
                        ),
                        if s["hit"].as_bool() == Some(true) {
                            Shape::Square
                        } else {
                            Shape::Hollow
                        },
                    ))
                })
                .collect();
            card=card.child(plot("aim-face","Measured face contacts","Normalized horizontal face axis (0–1)","Normalized vertical face axis (0–1)",Kind::Grid,points,(0.,1.),(0.,1.),"■ Confirmed hit · ◇ outline Other contact. The selected hit determines the face. No swing path or missed impact is reconstructed.").render(t));
        }
        let mut choose = div().flex().flex_wrap().gap(px(4.));
        for (i, s) in samples.iter().enumerate() {
            if let Some(seq) = s["seq"].as_i64() {
                choose = choose.child(
                    button(("aim-hit", i), format!("#{seq} · {}", text(s, "grade")))
                        .selected(selected.is_some_and(|s| s["seq"].as_i64() == Some(seq)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.aim_selection = Some(seq);
                            cx.notify();
                        })),
                );
            }
        }
        card = card.child(choose);
        if let Some(s) = selected {
            card = card.child(ui::caption(
                t,
                format!(
                    "Selected contact #{} · {:.2} s · {} · {} · distance from ideal {} · {}",
                    s["seq"],
                    num(s, "t").unwrap_or(0.),
                    text(s, "weapon"),
                    text(s, "grade"),
                    format(num(s, "distance"), "m"),
                    text(s, "band")
                ),
            ));
        }
        card
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contact_density_retains_edges_and_rejects_missing() {
        let g = density(&[(0., 1.), (1., 0.), (0.5, 0.5), (f64::NAN, 0.)]);
        assert_eq!(g[0][0], 1);
        assert_eq!(g[11][5], 1);
        assert_eq!(g.iter().flatten().sum::<u64>(), 3);
    }
    #[test]
    fn face_axes_follow_selected_hit_face() {
        assert_eq!(face_axes("TOP", 0.1, 0.2, 0.3), (0.1, 0.3));
        assert_eq!(face_axes("WEST", 0.1, 0.2, 0.3), (0.3, 0.2));
    }
}
