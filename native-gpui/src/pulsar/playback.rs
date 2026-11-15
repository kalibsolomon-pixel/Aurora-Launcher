//! Playback operates on one immutable event snapshot; ticks never call the API.
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
use aurora_launcher_lib::pulsar::view::{EventView, PlaybackCheckpoint};
use gpui::{Context, Div, KeyDownEvent, MouseButton, Role, div, prelude::*, px};
pub(super) fn advance(head: f64, elapsed: f64, speed: f64, duration: f64) -> f64 {
    if !elapsed.is_finite() || !speed.is_finite() || !duration.is_finite() {
        return head;
    }

    (head + elapsed.max(0.) * speed.clamp(0.25, 4.)).clamp(0., duration.max(0.))
}

pub(super) fn step(events: &[EventView], head: f64, forward: bool, duration: f64) -> f64 {
    if forward {
        events
            .iter()
            .filter(|e| e.kind != "MOTION" && e.time > head + 1e-9)
            .map(|e| e.time)
            .min_by(f64::total_cmp)
            .unwrap_or(duration)
    } else {
        events
            .iter()
            .filter(|e| e.kind != "MOTION" && e.time < head - 1e-9)
            .map(|e| e.time)
            .max_by(f64::total_cmp)
            .unwrap_or(0.)
    }
}

pub(super) fn visible<'a>(
    events: &'a [EventView],
    head: f64,
    filter: Option<&str>,
) -> Vec<&'a EventView> {
    let mut rows: Vec<_> = events
        .iter()
        .filter(|e| e.time <= head + 1e-9 && filter.map_or(e.kind != "MOTION", |f| e.kind == f))
        .collect();
    if rows.len() > 200 {
        rows.drain(..rows.len() - 200);
    }

    rows
}

pub(super) fn checkpoint_at(
    points: &[PlaybackCheckpoint],
    head: f64,
) -> Option<&PlaybackCheckpoint> {
    let i = points.partition_point(|p| p.time <= head + 1e-9);
    i.checked_sub(1).and_then(|i| points.get(i))
}

fn button(id: impl Into<gpui::ElementId>, label: impl Into<gpui::SharedString>) -> AuroraButton {
    AuroraButton::new(id, super::presentation::text(label.into().as_ref()))
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Small)
}

impl Pulsar {
    pub(super) fn event_table(&self, t: &AuroraTheme, events: &[&EventView]) -> Div {
        let total = self
            .active_detail()
            .map(|d| {
                d.timeline
                    .iter()
                    .filter(|e| {
                        self.event_filter
                            .as_deref()
                            .map(|kind| e.kind == kind)
                            .unwrap_or(e.kind != "MOTION")
                    })
                    .count()
            })
            .unwrap_or(events.len());
        let mut list = ui::list_card(t)
            .child(ui::row(t, false).child(ui::section_header(
                t,
                format!("Showing {} of {total} events", events.len()),
                Some("Time in seconds; recorded observations in chronological order".into()),
            )))
            .child(
                ui::row(t, false)
                    .py(px(5.))
                    .child(ui::caption(t, "Time").w(px(64.)).flex_shrink_0())
                    .child(ui::caption(t, "Event and supporting evidence").flex_1()),
            );
        for (i, e) in events.iter().enumerate() {
            let line = format!(
                "{:.2} s · {} · {} · {}{}",
                e.time,
                e.kind,
                e.confidence,
                super::presentation::event_detail(&e.detail),
                e.outcome
                    .as_ref()
                    .map(|s| format!(" · {s}"))
                    .unwrap_or_default()
            );
            list = list.child(
                ui::row(t, i + 1 == events.len())
                    .py(px(5.))
                    .id(format!("event-{}", e.sequence))
                    .child(
                        ui::caption(t, format!("{:.2} s", e.time))
                            .w(px(64.))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .child(ui::muted(
                                t,
                                format!(
                                    "{}{}",
                                    e.kind,
                                    e.outcome
                                        .as_ref()
                                        .map(|s| format!(" \u{b7} {s}"))
                                        .unwrap_or_default()
                                ),
                            ))
                            .child(ui::caption(
                                t,
                                format!(
                                    "{} \u{b7} {}",
                                    e.confidence,
                                    super::presentation::event_detail(&e.detail)
                                ),
                            )),
                    )
                    .role(Role::Group)
                    .aria_label(line),
            );
        }

        if events.is_empty() {
            list = list.child(
                ui::row(t, true).child(ui::muted(t, "No matching observations at this time.")),
            );
        }

        list
    }

    pub(super) fn event_filters(&self, cx: &mut Context<Self>) -> Div {
        let mut filters = div().flex().flex_wrap().gap(px(4.));
        for (i, kind) in [
            "All",
            "ATTACK",
            "HURT",
            "DAMAGE_IN",
            "SHIELD",
            "CRIT",
            "JUMP",
            "SPRINT",
            "HEALTH",
            "VELOCITY",
            "DEATH",
            "MOTION",
        ]
        .into_iter()
        .enumerate()
        {
            let selected =
                self.event_filter.as_deref() == if kind == "All" { None } else { Some(kind) };
            filters = filters.child(
                button(("event-filter", i), super::presentation::text(kind))
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.event_filter = if kind == "All" {
                            None
                        } else {
                            Some(kind.into())
                        };
                        cx.notify();
                    })),
            );
        }

        filters
    }
    pub(super) fn playback_body(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let mut page = div()
            .id("pulsar-playback")
            .flex()
            .flex_col()
            .gap(px(12.))
            .track_focus(&self.playback_focus)
            .tab_index(0)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                match e.keystroke.key.as_str() {
                    "space" | " " => this.toggle_play(cx),
                    "," | "comma" => this.step(false, cx),
                    "." | "period" => this.step(true, cx),
                    _ => return,
                }

                cx.stop_propagation();
                cx.notify();
            }));
        let Some(p) = &self.playback else {
            return div().child(page.child(ui::muted(
                t,
                "Playback unavailable: this recording has no event timeline.",
            )));
        };
        let duration = self.duration();
        let mut controls = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(
                AuroraButton::new(
                    "playback-play",
                    if self.preferences.reduced_motion {
                        "Step event"
                    } else if self.playing {
                        "Pause"
                    } else {
                        "Play"
                    },
                )
                .variant(ButtonVariant::Primary)
                .icon(if self.playing { "pause" } else { "play_arrow" })
                .disabled(duration <= 0.)
                .on_click(cx.listener(|this, _, _, cx| this.toggle_play(cx))),
            )
            .child(
                button("playback-previous", "Previous event")
                    .on_click(cx.listener(|this, _, _, cx| this.step(false, cx))),
            )
            .child(
                button("playback-next", "Next event")
                    .on_click(cx.listener(|this, _, _, cx| this.step(true, cx))),
            )
            .child(ui::muted(
                t,
                format!("{:.1} of {:.1} s", self.playhead, duration),
            ));
        let mut speeds = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(t.radius)
            .bg(t.sunken)
            .child(ui::caption(t, "Speed").px(px(6.)));
        for (i, speed) in [0.25, 0.5, 1., 2., 4.].into_iter().enumerate() {
            speeds = speeds.child(
                button(("playback-speed", i), format!("{speed}×"))
                    .selected(self.speed == speed)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.speed = speed;
                        cx.notify();
                    })),
            );
        }
        controls = controls.child(speeds);

        let selected = p
            .events
            .iter()
            .rev()
            .find(|e| e.kind != "MOTION" && e.time <= self.playhead + 1e-9);
        controls = controls.child(
            div().flex_1().min_w(px(180.)).child(ui::caption(
                t,
                selected
                    .map(|e| {
                        format!(
                            "{:.2} s \u{b7} {} \u{b7} {}",
                            e.time,
                            e.kind,
                            super::presentation::event_detail(&e.detail)
                        )
                    })
                    .unwrap_or_else(|| "Start of recording; no event selected yet.".into()),
            )),
        );
        page = page.child(ui::card(t).p(px(12.)).gap(px(4.)).child(controls));
        // The seek rail is keyboard accessible and pointer seeking uses its actual layout bounds.
        let bounds = self.seek_bounds.clone();
        let recorded = bounds.clone();
        let rail=div().id("playback-seek").relative().w_full().h(px(30.)).rounded(t.radius).bg(t.sunken).role(Role::Slider)
            .aria_label(format!("Playback position {:.2} seconds of {:.2}. Left and right seek one second; Home and End jump.",self.playhead,duration))
            .track_focus(&self.seek_focus).tab_index(0)
            .child(gpui::canvas(move|b,_,_|{recorded.set(Some(b));},|_,_,_,_|{}).absolute().size_full())
            .focus_visible(|s| s.border_1().border_color(t.ring))
            .child(div().absolute().left_0().right_0().top(px(12.)).h(px(6.)).rounded_full().bg(t.muted_foreground.opacity(0.45)))
            .child(div().absolute().left_0().top(px(12.)).h(px(6.)).w(gpui::relative((self.playhead/duration.max(0.001))as f32)).rounded_full().bg(t.ring))
            .child(div().absolute().left(gpui::relative((self.playhead/duration.max(0.001))as f32)).ml(px(-7.)).top(px(8.)).w(px(14.)).h(px(14.)).rounded_full().bg(t.foreground))
            .on_mouse_down(MouseButton::Left,cx.listener(move|this,e:&gpui::MouseDownEvent,_,cx|{if let Some(b)=bounds.get(){let f=((e.position.x-b.origin.x)/b.size.width).clamp(0.,1.);this.playhead=f as f64*this.duration();this.pause();cx.notify();}}))
            .on_key_down(cx.listener(|this,e:&KeyDownEvent,_,cx|{let head=match e.keystroke.key.as_str(){"left"=>this.playhead-1.,"right"=>this.playhead+1.,"home"=>0.,"end"=>this.duration(),_=>return};this.playhead=head.clamp(0.,this.duration());this.pause();cx.stop_propagation();cx.notify();}));
        page = page.child(rail);
        let lanes = if p.lanes.is_empty() {
            vec![
                "rounds".into(),
                "exchanges".into(),
                "attacks".into(),
                "incoming".into(),
                "shields".into(),
                "jumps".into(),
                "sprint".into(),
            ]
        } else {
            p.lanes.clone()
        };
        let samples = p
            .events
            .iter()
            .filter(|e| e.kind != "MOTION")
            .filter(|e| !e.detail.starts_with("Not counted:") && e.detail != "Stun follow-up")
            .map(|e| Sample {
                x: e.time,
                y: Some(lanes.iter().position(|l| l == &e.lane).unwrap_or(0) as f64),
                label: format!(
                    "{:.2} s · {} · {} · {}",
                    e.time,
                    e.lane,
                    e.kind,
                    super::presentation::event_detail(&e.detail)
                ),
                shape: event_shape(e),
            })
            .collect();
        let event_times = p
            .events
            .iter()
            .filter(|e| e.kind != "MOTION")
            .map(|e| e.time)
            .collect::<Vec<_>>();
        let view = cx.entity();
        let lane_bounds = std::rc::Rc::new(std::cell::Cell::new(None));
        let measured = lane_bounds.clone();
        let lane_chart = Chart {
            id: "playback-lanes".into(),
            title: "Event lanes".into(),
            x_label: "Recording time (s)".into(),
            y_label: lanes
                .iter()
                .map(|s| {
                    if s == "activeExchanges" {
                        "Combat".into()
                    } else if s == "events" {
                        "Other events".into()
                    } else if s == "attacks" {
                        "Attacks".into()
                    } else {
                        super::analysis_ui::display_label(s)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            samples,
            kind: Kind::Lanes,
            domain: (0., duration.max(1.)),
            range: (-0.5, lanes.len() as f64 - 0.5),
            band: None,
            target: None,
            playhead: Some(self.playhead),
            bands: p
                .boundaries
                .iter()
                .map(|b| super::charts::Band {
                    start: b.start,
                    end: b.end,
                    lane: lanes.iter().position(|l| l == &b.lane).map(|i| i as f64),
                    label: b.label.clone(),
                    selected: false,
                })
                .collect(),
            incomplete: vec![],
            note: "Shaded bands are rounds and combat; the white line is the playhead. Select a mark to jump to it."
                .into(),
        };
        // Two compact legend rows; each key is drawn with the plot's own mark.
        let legend_row = |title: &str, keys: Vec<(Shape, &str, gpui::Hsla)>| {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(12.))
                .child(
                    ui::caption(t, title.to_owned())
                        .w(px(104.))
                        .flex_shrink_0()
                        .font_weight(gpui::FontWeight::MEDIUM),
                )
                .children(keys.into_iter().map(|(shape, label, color)| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(super::charts::glyph(shape, color).w(px(12.)).h(px(12.)))
                        .child(ui::caption(t, label))
                }))
        };
        page = page.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(legend_row(
                    "Your actions",
                    vec![
                        (Shape::Square, "Attack attempt", t.ring),
                        (Shape::Diamond, "Shield or trade", t.warning),
                        (Shape::Hollow, "Jump", t.success),
                        (Shape::Tick, "Sprint change", t.foreground),
                    ],
                ))
                .child(legend_row(
                    "Hits and results",
                    vec![
                        (Shape::Plus, "Hit evidence", t.success),
                        (Shape::Star, "Critical animation", t.success),
                        (Shape::Triangle, "Incoming hit", t.danger),
                        (Shape::Cross, "Death", t.danger),
                    ],
                )),
        );
        let shortcuts = ui::caption(
            t,
            "Space: play or pause \u{b7} Comma: previous event \u{b7} Period: next event \u{b7} Arrow keys: seek one second",
        );
        page = page.child(
            lane_chart
                .render_options(
                    t,
                    Some(measured),
                    super::charts::Options {
                        preferences: self.preferences,
                        activate: Some(std::rc::Rc::new(move |target, _, cx| {
                            if let super::charts::Target::Sample(i) = target {
                                if let Some(time) = event_times.get(i) {
                                    view.update(cx, |this, cx| {
                                        this.playhead = *time;
                                        this.pause();
                                        cx.notify();
                                    });
                                }
                            }
                        })),
                        ..Default::default()
                    },
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &gpui::MouseDownEvent, _, cx| {
                        if let Some(bounds) = lane_bounds.get() {
                            let fraction = ((e.position.x - bounds.origin.x) / bounds.size.width)
                                .clamp(0., 1.);
                            this.playhead = fraction as f64 * this.duration();
                            this.pause();
                            cx.notify();
                        }
                    }),
                ),
        );
        page = page.child(shortcuts);
        let mut jumps =
            div()
                .flex()
                .flex_wrap()
                .gap(px(4.))
                .child(button("playback-start", "Start").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.playhead = 0.;
                        this.pause();
                        cx.notify();
                    },
                )));
        for (i, band) in p.boundaries.iter().enumerate().take(40) {
            let time = band.start;
            let label = if band.lane == "activeExchanges" {
                "Combat".to_owned()
            } else {
                super::presentation::text(&band.label)
            };
            jumps = jumps.child(
                button(
                    ("playback-boundary", i),
                    format!("{label} · {time:.1}–{:.1} s", band.end),
                )
                .accessibility_label(format!("Seek to {label} at {time:.1} seconds"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.playhead = time.clamp(0., this.duration());
                    this.pause();
                    cx.notify();
                })),
            );
        }

        page = page.child(jumps);
        let checkpoint = checkpoint_at(&p.checkpoints, self.playhead);
        let count = |get: fn(&aurora_launcher_lib::pulsar::view::PlaybackCheckpoint) -> usize| {
            checkpoint
                .map(|c| get(c).to_string())
                .unwrap_or_else(|| "Unavailable".into())
        };
        let dominance = checkpoint.and_then(|c| c.dominance);
        page = page.child(
            ui::card(t).child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(24.))
                    .child(ui::stat(t, "Confirmed hits", count(|c| c.hits)))
                    .child(ui::stat(t, "Eligible attempts", count(|c| c.attempts)))
                    .child(ui::stat(
                        t,
                        "Hits from your opponent",
                        count(|c| c.incoming),
                    ))
                    .child(ui::stat(t, "Current streak", count(|c| c.local_streak)))
                    .child(ui::stat(
                        t,
                        "Dominance",
                        dominance
                            .map(|d| format!("{d:+.3}"))
                            .unwrap_or_else(|| "Unavailable".into()),
                    )),
            ),
        );
        let filters = self.event_filters(cx);
        page = page.child(filters).child(self.event_table(
            t,
            &visible(&p.events, self.playhead, self.event_filter.as_deref()),
        ));
        for n in &p.notices {
            page = page.child(ui::caption(t, n.clone()));
        }

        let _ = tabs::metric_value;
        div().child(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(t: f64, k: &str) -> EventView {
        EventView {
            time: t,
            kind: k.into(),
            ..Default::default()
        }
    }

    #[test]

    fn cached_checkpoint_keeps_attribution_and_canonical_counts() {
        let points = vec![
            PlaybackCheckpoint {
                time: 0.,
                ..Default::default()
            },
            PlaybackCheckpoint {
                time: 1.,
                hits: 2,
                attempts: 3,
                incoming: 1,
                local_streak: 2,
                ..Default::default()
            },
        ];
        assert_eq!(checkpoint_at(&points, 0.5).unwrap().hits, 0);
        assert_eq!(checkpoint_at(&points, 1.).unwrap().attempts, 3);
        assert_eq!(checkpoint_at(&points, 3.).unwrap().incoming, 1);
        assert!(checkpoint_at(&points, -1.).is_none());
    }

    #[test]

    fn playhead_stepping_skips_motion_and_clamps() {
        let e = vec![
            event(0.1, "ATTACK"),
            event(0.2, "MOTION"),
            event(0.3, "JUMP"),
        ];
        assert_eq!(step(&e, 0.1, true, 1.), 0.3);
        assert_eq!(step(&e, 0.3, false, 1.), 0.1);
        assert_eq!(step(&e, 0., false, 1.), 0.);
        assert_eq!(advance(0.9, 1., 4., 1.), 1.);
        assert_eq!(advance(0.5, f64::NAN, 1., 1.), 0.5);
    }

    #[test]

    fn playback_filter_excludes_future_and_motion_by_default() {
        let e = vec![
            event(0.1, "ATTACK"),
            event(0.2, "MOTION"),
            event(0.3, "JUMP"),
        ];
        assert_eq!(visible(&e, 0.2, None).len(), 1);
        assert_eq!(visible(&e, 0.2, Some("MOTION"))[0].kind, "MOTION");
    }
}

fn event_shape(e: &EventView) -> Shape {
    if e.detail.to_lowercase().contains("trade") {
        return Shape::Diamond;
    }

    match e.kind.as_str() {
        "SHIELD" => Shape::Diamond,
        "JUMP" => Shape::Hollow,
        "DAMAGE_IN" => Shape::Triangle,
        "HURT" => Shape::Plus,
        "CRIT" => Shape::Star,
        "DEATH" => Shape::Cross,
        "SPRINT" | "ROUND" | "COMBAT_EXCHANGE" => Shape::Tick,
        _ => Shape::Square,
    }
}
