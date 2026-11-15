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
        let Some(detail) = self.active_detail() else {
            return body;
        };
        let d = &detail.derived;
        let tab = if self.detail_request.is_some() {
            self.detail_tab
        } else {
            self.top_tab
        };
        let section = self.sections[tab];
        match tab {
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
                            if let Some(request) = this.scope_request() {
                                this.select_scope(request, cx);
                            }
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
                            if let Some(request) = this.scope_request() {
                                this.select_scope(request, cx);
                            }
                        })),
                    );
                }
                if matches!(section, 0 | 2 | 3 | 4) {
                    body = body.child(filters);
                }
                if section == 0 || section == 2 {
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
                    body = body.child(histogram.render_options(
                        t,
                        None,
                        super::charts::Options {
                            preferences: self.preferences,
                            ..Default::default()
                        },
                    ));
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
                    body=body.child(plot("detail-reach-strip","Individual contacts","Recording time (s)","Distance (m)",Kind::Scatter,points,(0.,self.analysis_duration().max(1.)),(0.,nice_peak(reaches.iter().copied().fold(4.,f64::max))),"■ Confirmed hit · ◇ outline Estimated no-hit contact. Geometry is observed, not a server verdict.").render_options(t,None,super::charts::Options{preferences:self.preferences,..Default::default()}));
                }
                if section == 0 || section == 3 {
                    body = body.child(self.aim_view(t, d, cx));
                }
                if section == 0 || section == 4 {
                    let samples: Vec<_> = rows(d, "timingSamples")
                        .iter()
                        .filter(|s| self.weapon.as_ref().is_none_or(|w| text(s, "weapon") == w))
                        .filter(|s| {
                            self.timing_result.as_deref().is_none_or(|r| {
                                if r == "HITS" {
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
                        ("Hits", Some("HITS")),
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
                                    if let Some(request) = this.scope_request() {
                                        this.select_scope(request, cx);
                                    }
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
                    timing.band = detail
                        .offense
                        .metrics
                        .get("timingTolerance")
                        .and_then(|m| m.value)
                        .map(|v| (-v, v));
                    body = body.child(grade_legend(
                        t,
                        &["PERFECT", "TOO_FAST", "TOO_SLOW", "OPENER / RESET"],
                    ));
                    body = body.child(timing.render_options(
                        t,
                        None,
                        super::charts::Options {
                            preferences: self.preferences,
                            ..Default::default()
                        },
                    ));
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
            }
            2 => {
                if section == 0 || section == 3 {
                    let total = self.analysis_duration().max(1.);
                    let span = (self.jump_span / 100. * total).clamp(1., total);
                    let start = self.jump_start / 100. * (total - span).max(0.);
                    body = body.child(self.jump_controls(t, total, cx));
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
                        (start, start + span),
                        (-10., 10.),
                        "◆ Same tick · ■ Other measured offset. Candidate timing does not establish knockback reduction. Display clamps to ±10 ticks; hover gives the exact offset.",
                    );
                    jump.samples.retain(|s| s.x >= start && s.x <= start + span);
                    jump.target = Some(0.);
                    jump.band = num(d, "jumpWindowTicks").map(|w| (-w, w));
                    body = body.child(jump.render_options(
                        t,
                        None,
                        super::charts::Options {
                            preferences: self.preferences,
                            ..Default::default()
                        },
                    ));
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
                    let mut timeline = plot(
                        "detail-jump-lanes",
                        "Incoming hits & actual jumps",
                        "Recording time (s)",
                        "Bottom: incoming · top: actual jump",
                        Kind::Lanes,
                        lanes,
                        (start, start + span),
                        (-0.5, 1.5),
                        "■ Incoming hit · ◇ outline Actual observed jump. Client tick timing is affected by network delay; this is not a world replay.",
                    );
                    timeline
                        .samples
                        .retain(|s| s.x >= start && s.x <= start + span);
                    body = body.child(timeline.render_options(
                        t,
                        None,
                        super::charts::Options {
                            preferences: self.preferences,
                            ..Default::default()
                        },
                    ));
                }
                if section == 0 || section == 1 || section == 4 {
                    let incoming = rows(d, "incomingDetail");
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
            }
            3 => {
                if section == 0 || section == 1 {
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
                        (0., self.analysis_duration().max(1.)),
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
                    let streak_count = combo.bands.len();
                    for b in rows(d, "boundaries") {
                        if let Some(time) = num(b, "t").or_else(|| num(b, "time")) {
                            combo.bands.push(super::charts::Band {
                                start: time,
                                end: time + 0.02,
                                lane: None,
                                label: format!("Boundary | {}", text(b, "label")),
                                selected: false,
                            });
                        }
                    }
                    let mark_seqs: Vec<_> = marks.iter().map(|m| m["seq"].clone()).collect();
                    let combo_seqs: Vec<Vec<Value>> =
                        streaks.iter().map(|s| rows(s, "seqs").to_vec()).collect();
                    let entity = cx.entity().downgrade();
                    let activate = std::rc::Rc::new(
                        move |target, _: &mut gpui::Window, cx: &mut gpui::App| {
                            let _ = entity.update(cx, |this, cx| {
                                match target {
                                    super::charts::Target::Band(i) if i < streak_count => {
                                        this.combo_selection = i
                                    }
                                    super::charts::Target::Sample(i) => {
                                        if let Some(seq) = mark_seqs.get(i) {
                                            if let Some(index) =
                                                combo_seqs.iter().position(|s| s.contains(seq))
                                            {
                                                this.combo_selection = index;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                                cx.notify();
                            });
                        },
                    );
                    body = body.child(combo.render_options(
                        t,
                        None,
                        super::charts::Options {
                            preferences: self.preferences,
                            activate: Some(activate),
                            ..Default::default()
                        },
                    ));
                    let mut selectors = div().flex().flex_wrap().gap(px(4.));
                    selectors = selectors
                        .child(
                            button("combo-previous", "Previous combo")
                                .disabled(streaks.is_empty())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(d) = this.active_detail() {
                                        let n = rows(&d.derived["combos"], "streaks").len();
                                        if n > 0 {
                                            this.combo_selection =
                                                (this.combo_selection + n - 1) % n;
                                        }
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            button("combo-next", "Next combo")
                                .disabled(streaks.is_empty())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(d) = this.active_detail() {
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
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.combo_selection = i;
                                    cx.notify();
                                },
                            )),
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
                            } else {
                                let incoming = rows(d, "incomingDetail")
                                    .iter()
                                    .find(|h| h["sequence"] == *seq || h["seq"] == *seq);
                                let event = detail
                                    .timeline
                                    .iter()
                                    .find(|e| Some(e.sequence) == seq.as_i64());
                                card=card.child(ui::caption(t,format!("Opponent hit {} | evidence {} | attribution {} | damage {} | knockback {} | critical {}",seq,
                                    incoming.map(|h|format(num(h,"time"),"s")).or_else(||event.map(|e|format!("{:.2} s",e.time))).unwrap_or_else(||"Unavailable".into()),
                                    incoming.map(|h|text(h,"attribution")).unwrap_or("Tracked opponent evidence"),
                                    format(incoming.and_then(|h|num(h,"damage")),"HP"),format(incoming.and_then(|h|num(h,"knockback")),"blocks/tick"),
                                    incoming.and_then(|h|h["crit"].as_bool()).map(|v|v.to_string()).unwrap_or_else(||"Unavailable".into()))));
                            }
                        }
                        body = body.child(card);
                    }
                }
                if section == 0 || section == 2 {
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
                        (0., self.analysis_duration().max(1.)),
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
                    let longest = &dom["longest"];
                    let swing = &dom["swing"];
                    body = body.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(24.))
                            .child(
                                ui::card(t)
                                    .flex_1()
                                    .min_w(px(210.))
                                    .child(ui::section_header(t, "Longest advantage", None))
                                    .child(ui::muted(
                                        t,
                                        format!(
                                            "{} | {}",
                                            text(longest, "side"),
                                            format(
                                                num(longest, "end")
                                                    .zip(num(longest, "start"))
                                                    .map(|(e, s)| e - s),
                                                "s"
                                            )
                                        ),
                                    ))
                                    .child(ui::caption(
                                        t,
                                        format!(
                                            "{} | {}",
                                            format(num(longest, "start"), "s"),
                                            format(num(longest, "end"), "s")
                                        ),
                                    )),
                            )
                            .child(
                                ui::card(t)
                                    .flex_1()
                                    .min_w(px(210.))
                                    .child(ui::section_header(t, "Largest <=3 s swing", None))
                                    .child(ui::muted(
                                        t,
                                        format!(
                                            "{} | {}",
                                            format(num(swing, "fromValue"), "balance"),
                                            format(num(swing, "toValue"), "balance")
                                        ),
                                    ))
                                    .child(ui::caption(
                                        t,
                                        format!(
                                            "{} | {}",
                                            format(num(swing, "from"), "s"),
                                            format(num(swing, "to"), "s")
                                        ),
                                    )),
                            ),
                    );
                    body = body.child(chart.render_options(
                        t,
                        None,
                        super::charts::Options {
                            preferences: self.preferences,
                            ..Default::default()
                        },
                    ));
                    body=body.child(ui::title_block(t,"How dominance is measured",Some("D = (P - Q) / (P + Q + 1). Confirmed hits weigh 1; subsequent streak hits add 0.25. Blocking an opponent attack adds 0.25; an opponent shield block subtracts 0.25. Shield disables weigh +/-0.5. Evidence decays exponentially with | = 3 s. Advantage starts at +/-0.2; mass below 0.35 is insufficient, unattributed mass at 0.5 is incomplete. Shares exclude inactive and incomplete time and pool by observed active duration. Current exchange boundaries reset the model. This heuristic measures observed evidence, not damage, skill or a server verdict; aim, reach and accuracy do not enter the model.".into())));
                }
            }
            _ => {}
        }
        let _ = (narrow, tabs::metric_value);
        body
    }
    fn aim_view(&self, t: &AuroraTheme, d: &Value, cx: &mut Context<Self>) -> Div {
        let all = rows(&d["aim"], "samples");
        let duration = self.analysis_duration();
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
            let mut contact = plot(
                "aim-face",
                "Measured face contacts",
                "Normalized horizontal face axis (0-1)",
                "Normalized vertical face axis (0-1)",
                Kind::Grid,
                points,
                (0., 1.),
                (0., 1.),
                "Square: confirmed hit | Circle: other contact. Select a mark or use Tab and Enter. Background uses the selected contact's frozen ideal point and physical radii; it does not reconstruct occlusion.",
            );
            let regions = selected
                .map(|pick| frozen_regions(d, pick, face))
                .unwrap_or_default();
            if regions.is_empty() {
                contact
                    .note
                    .push_str(" Frozen region unavailable for this sample.");
            }
            let seqs: Vec<_> = contacts.iter().filter_map(|s| s["seq"].as_i64()).collect();
            let entity = cx.entity().downgrade();
            let activate =
                std::rc::Rc::new(move |target, _: &mut gpui::Window, cx: &mut gpui::App| {
                    if let super::charts::Target::Sample(i) = target {
                        if let Some(seq) = seqs.get(i) {
                            let _ = entity.update(cx, |this, cx| {
                                this.aim_selection = Some(*seq);
                                cx.notify();
                            });
                        }
                    }
                });
            card = card.child(contact.render_options(
                t,
                None,
                super::charts::Options {
                    preferences: self.preferences,
                    regions,
                    activate: Some(activate),
                },
            ));
            card = card.child(grade_legend(t, &["IDEAL", "GOOD", "PERIPHERAL", "POOR"]));
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

fn grade_legend(t: &AuroraTheme, grades: &[&str]) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap(px(16.))
        .children(grades.iter().map(|g| {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().size(px(8.)).bg(super::charts::grade_color(t, g)))
                .child(ui::caption(t, (*g).to_owned()))
        }))
}
/// Plane/sphere intersection in the selected normalized face. Uses frozen physical geometry.
fn frozen_regions(d: &Value, pick: &Value, face: &str) -> Vec<(f64, f64, f64, f64, usize)> {
    let measurement = pick.get("measurement").or_else(|| {
        rows(d, "hits")
            .iter()
            .find(|h| h["seq"] == pick["seq"])
            .and_then(|h| h.get("measurement"))
    });
    let Some(m) = measurement else {
        return vec![];
    };
    let r = pick.get("idealRegion").unwrap_or(&m["idealRegion"]);
    if r["reachable"] == false {
        return vec![(0.5, 0.5, 2., 2., 3)];
    }
    let axes = match face {
        "TOP" | "BOTTOM" => (0, 2, 1),
        "EAST" | "WEST" => (2, 1, 0),
        _ => (0, 1, 2),
    };
    let coord = ["x", "y", "z"];
    let mins = ["minX", "minY", "minZ"];
    let maxs = ["maxX", "maxY", "maxZ"];
    let sizes: Option<Vec<_>> = (0..3)
        .map(|i| Some(num(&m["box"], maxs[i])? - num(&m["box"], mins[i])?))
        .collect();
    let Some(size) = sizes.filter(|s| s.iter().all(|v| *v > 0.)) else {
        return vec![];
    };
    let Some((cx, cy)) = num(&r["ideal"], coord[axes.0]).zip(num(&r["ideal"], coord[axes.1]))
    else {
        return vec![];
    };
    let Some(fixed) = num(pick, coord[axes.2])
        .zip(num(&r["ideal"], coord[axes.2]))
        .map(|(p, i)| (p - i) * size[axes.2])
    else {
        return vec![];
    };
    let mut out = vec![(0.5, 0.5, 2., 2., 3)];
    for (key, grade) in [
        ("peripheralRadius", 2),
        ("goodRadius", 1),
        ("idealRadius", 0),
    ] {
        if let Some(radius) = num(r, key).filter(|r| *r >= fixed.abs()) {
            let plane = (radius * radius - fixed * fixed).sqrt();
            out.push((cx, cy, plane / size[axes.0], plane / size[axes.1], grade));
        }
    }
    out
}
impl Pulsar {
    fn jump_controls(&self, t: &AuroraTheme, total: f64, _cx: &mut Context<Self>) -> Div {
        let span = (self.jump_span / 100. * total).clamp(1., total);
        let start = self.jump_start / 100. * (total - span).max(0.);
        ui::card(t).child(ui::section_header(t,"Jump timeline window",Some(format!("{start:.2}-{:.2} s | {span:.2} s span | sliders use percent of the recording",start+span).into())))
   .child(crate::controls::AuroraSlider::new(&self.jump_span_slider,"Span (%)"))
   .child(crate::controls::AuroraSlider::new(&self.jump_start_slider,"Start (%)"))
    }
}
