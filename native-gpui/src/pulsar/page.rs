use super::presentation::ui;
use super::{Pulsar, TOP_TABS, tabs};
use crate::{
    controls::{
        AuroraButton, AuroraCheckbox, AuroraInput, AuroraSelect, ButtonSize, ButtonVariant,
    },
    theme::AuroraTheme,
};
use aurora_launcher_lib::pulsar::view::*;
use gpui::{Context, Div, KeyDownEvent, Window, div, prelude::*, px};

fn small(id: impl Into<gpui::ElementId>, label: impl Into<gpui::SharedString>) -> AuroraButton {
    AuroraButton::new(id, super::presentation::text(label.into().as_ref()))
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Small)
}

impl Pulsar {
    fn scope_bar(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let ids: Vec<_> = std::iter::once(None)
            .chain(
                self.library
                    .instances
                    .iter()
                    .map(|i| Some(i.instance_id.clone())),
            )
            .collect();
        let options = std::iter::once(if self.top_tab == 7 {
            "Choose an instance".into()
        } else {
            "All instances".into()
        })
        .chain(
            self.library
                .instances
                .iter()
                .map(|i| i.instance_name.clone().into()),
        )
        .collect();
        let selected = ids.iter().position(|id| id == &self.query.instance_id);
        let view = cx.entity();
        let highlight = view.clone();
        let confirm = view.clone();
        let select = AuroraSelect::new(
            "pulsar-instance",
            "Instance",
            options,
            &self.scope_trigger,
            &self.scope_content,
            self.scope_bounds.clone(),
        )
        .selected(selected)
        .open(self.scope_open)
        .highlight(self.scope_highlight)
        .width(px(if self.width < 900. { 160. } else { 210. }))
        .on_open_change(move |open, _, cx| {
            view.update(cx, |this, cx| {
                this.scope_open = open;
                cx.notify();
            })
        })
        .on_highlight(move |index, _, cx| {
            highlight.update(cx, |this, cx| {
                this.scope_highlight = index;
                cx.notify();
            })
        })
        .on_confirm(move |index, _, cx| {
            confirm.update(cx, |this, cx| {
                this.scope_open = false;
                this.query.instance_id = ids.get(index).cloned().flatten();
                this.query.offset = 0;
                this.close_detail();
                this.load(false, cx);
            })
        });
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(select)
            .child(div().flex_1().min_w(px(100.)).max_w(px(380.)).child(
                AuroraInput::new(&self.search).label(if self.width < 900. {
                    "Search"
                } else {
                    "Search opponents or servers"
                }),
            ))
            .when(self.width >= 1100., |d| {
                d.child(ui::caption(
                    t,
                    format!(
                        "{} in library",
                        super::presentation::count(self.library.total, "recording")
                    ),
                ))
            })
    }

    pub(super) fn fight_row(
        &self,
        t: &AuroraTheme,
        row: &FightSummary,
        index: usize,
        last: bool,
        narrow: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let columns = history_columns(self.width);
        let key = row.key.clone();
        let compare_key = key.clone();
        let open_key = key.clone();
        let chosen = self.selected.as_ref() == Some(&key);
        let checked = self.compare_keys.contains(&key);
        let accuracy = row
            .metrics
            .get("accuracy")
            .map(tabs::metric_value)
            .unwrap_or_else(|| "Unavailable".into());
        let hits = row
            .metrics
            .get("hits")
            .map(tabs::metric_value)
            .unwrap_or_else(|| "Unavailable".into());
        let mut row_readout = format!(
            "{} \u{b7} {} \u{b7} {} \u{b7} {} \u{b7} {}",
            row.opponent,
            utc_time(row.start_epoch_millis),
            row.provenance.instance_name,
            duration_text(row.duration_nanos),
            row.result_label
        );
        if let Some(server) = &row.server {
            row_readout.push_str(&format!(" \u{b7} Server: {server}"));
        }
        if row.stale {
            row_readout.push_str(" \u{b7} Source changed since import");
        }
        if !row.provenance.conflicts.is_empty() {
            row_readout.push_str(" \u{b7} Conflicting snapshot");
        }
        let row_theme = t.clone();
        let confirmed = row.resolution.confidence.eq_ignore_ascii_case("confirmed");
        let tone = if confirmed {
            match row.outcome.as_str() {
                "WIN" => ui::Tone::Success,
                "LOSS" => ui::Tone::Danger,
                _ => ui::Tone::Neutral,
            }
        } else {
            ui::Tone::Neutral
        };
        div().child(
            ui::row(t, last)
                .py(px(2.))
                .w_full()
                .overflow_hidden()
                .min_w(px(0.))
                .when(chosen, |d| d.bg(t.accent_soft))
                .id(format!("history-row-readout-{index}"))
                .tooltip(move |_, cx| {
                    cx.new(|_| super::charts::Readout {
                        text: row_readout.clone(),
                        theme: row_theme.clone(),
                    })
                    .into()
                })
                .child(
                    AuroraCheckbox::new(
                        format!("pulsar-compare-{index}"),
                        format!(
                            "Compare {} from {}",
                            row.opponent, row.provenance.instance_name
                        ),
                    )
                    .checked(checked)
                    .disabled(!checked && self.compare_keys.len() >= 2)
                    .on_change({
                        let view = cx.entity();
                        move |checked, _, _, cx| {
                            view.update(cx, |this, cx| {
                                if checked
                                    && !this.compare_keys.contains(&compare_key)
                                    && this.compare_keys.len() < 2
                                {
                                    this.compare_keys.push(compare_key.clone());
                                } else if !checked {
                                    this.compare_keys.retain(|k| k != &compare_key);
                                }

                                cx.notify();
                            });
                        }
                    }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(0.))
                        .flex_1()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .child(
                            div().flex().child(
                                small(
                                    format!("pulsar-select-{index}"),
                                    if row.manual {
                                        "Manual session".to_owned()
                                    } else {
                                        short_label(&row.opponent, if narrow { 18 } else { 30 })
                                    },
                                )
                                .selected(chosen)
                                .accessibility_label(format!(
                                    "Select {}. {}, {} hits, accuracy {}",
                                    row.opponent, row.result_label, hits, accuracy
                                ))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if this.selected.as_ref() == Some(&key) {
                                            this.open_detail(key.clone(), cx);
                                        } else {
                                            this.selected = Some(key.clone());
                                            cx.notify();
                                        }
                                    },
                                )),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    ui::caption(t, clock_time(row.start_epoch_millis))
                                        .line_height(px(16.))
                                        .flex_shrink_0(),
                                )
                                .child(
                                    ui::caption(t, row.provenance.instance_name.clone())
                                        .line_height(px(16.))
                                        .flex_1()
                                        .min_w(px(0.))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap(),
                                )
                                .child(
                                    ui::caption(t, duration_text(row.duration_nanos))
                                        .line_height(px(16.))
                                        .flex_shrink_0(),
                                ),
                        )
                        .when(narrow, |d| {
                            d.child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(ui::badge(t, row.result_label.clone(), tone))
                                    .child(ui::caption(
                                        t,
                                        format!("{hits} hits \u{b7} {accuracy} accuracy"),
                                    )),
                            )
                        }),
                )
                .when(!narrow, |d| {
                    d.child(
                        ui::muted(t, hits)
                            .w(px(if narrow { 40. } else { 55. }))
                            .flex_shrink_0(),
                    )
                })
                .when(!narrow, |d| {
                    d.child(ui::muted(t, accuracy).w(px(85.)).flex_shrink_0())
                })
                .when(columns.reach && !narrow, |d| {
                    d.child(
                        ui::muted(
                            t,
                            row.metrics
                                .get("averageReach")
                                .map(tabs::metric_value)
                                .unwrap_or_else(|| "Unavailable".into()),
                        )
                        .w(px(92.))
                        .flex_shrink_0(),
                    )
                })
                .when(!narrow, |d| {
                    d.child(div().w(px(130.)).flex_shrink_0().flex().child(ui::badge(
                        t,
                        row.result_label.clone(),
                        tone,
                    )))
                })
                .child(
                    div()
                        .w(px(if narrow { 75. } else { 100. }))
                        .flex_shrink_0()
                        .flex()
                        .child(
                            small(
                                format!("pulsar-open-{index}"),
                                if narrow { "Open" } else { "Open fight" },
                            )
                            .icon("arrow_forward")
                            .accessibility_label(format!("Open fight against {}", row.opponent))
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.open_detail(open_key.clone(), cx),
                            )),
                        ),
                ),
        )
    }

    fn history(&self, t: &AuroraTheme, narrow: bool, cx: &mut Context<Self>) -> Div {
        let sorts = ["Newest", "Oldest", "Hits", "Accuracy", "Reach", "Duration"];
        let sort_index = match self.query.sort {
            HistorySort::Newest => 0,
            HistorySort::Oldest => 1,
            HistorySort::Hits => 2,
            HistorySort::Accuracy => 3,
            HistorySort::Reach => 4,
            HistorySort::Duration => 5,
        };
        let tools = div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .items_center()
            .child(
                small("history-sort", format!("Sort: {}", sorts[sort_index])).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.query.sort = match (sort_index + 1) % 6 {
                            0 => HistorySort::Newest,
                            1 => HistorySort::Oldest,
                            2 => HistorySort::Hits,
                            3 => HistorySort::Accuracy,
                            4 => HistorySort::Reach,
                            _ => HistorySort::Duration,
                        };
                        this.load(false, cx);
                    }),
                ),
            )
            .child(
                small("history-hits", "Hits only")
                    .selected(self.query.hits_only)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.query.hits_only = !this.query.hits_only;
                        this.query.offset = 0;
                        this.load(false, cx);
                    })),
            );
        let mut actions = div().flex().flex_wrap().gap(px(4.)).items_center();
        actions = actions.child(
            small(
                "history-compare",
                if self.compare_keys.len() == 2 {
                    "Compare fights"
                } else {
                    "Compare (select 2)"
                },
            )
            .disabled(self.compare_keys.len() != 2)
            .on_click(cx.listener(|this, _, _, cx| this.open_compare(cx))),
        );

        // Contextual actions for the selected fight (P42); disabled without a selection.
        let chosen = self
            .selected
            .as_ref()
            .and_then(|k| self.library.rows.iter().find(|r| &r.key == k))
            .map(|r| (r.key.clone(), r.report_available));
        let none = chosen.is_none();
        let report = chosen.as_ref().is_some_and(|c| c.1);
        for (id, label, icon, action) in [
            ("history-details", "Details", "info", 0),
            ("history-report", "View report", "open_in_new", 1),
            ("history-export", "Export report", "download", 2),
            ("history-delete", "Delete fight", "delete", 3),
        ] {
            let key = chosen.as_ref().map(|c| c.0.clone());
            if chosen.is_some() {
                actions = actions.child(
                    small(id, label)
                        .icon(icon)
                        .disabled(none || self.busy || (matches!(action, 1 | 2) && !report))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let Some(key) = key.clone() else { return };
                            match action {
                                1 => this.view_report(key),
                                _ => {
                                    this.open_detail(key, cx);
                                    this.export_open = action == 2;
                                    this.delete_open = action == 3;
                                    this.delete_source = false;
                                }
                            }

                            cx.notify();
                        })),
                );
            }
        }

        let mut outcomes = div().flex().flex_wrap().gap(px(2.));
        for (i, (label, outcome)) in [
            ("All", OutcomeFilter::All),
            ("Win", OutcomeFilter::Win),
            ("Loss", OutcomeFilter::Loss),
            ("Draw", OutcomeFilter::Draw),
            ("Unknown", OutcomeFilter::Unknown),
            ("Interrupted", OutcomeFilter::Interrupted),
        ]
        .into_iter()
        .enumerate()
        {
            let selected =
                std::mem::discriminant(&self.query.outcome) == std::mem::discriminant(&outcome);
            outcomes = outcomes.child(
                small(("pulsar-outcome", i), label)
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.query.outcome = outcome.clone();
                        this.query.offset = 0;
                        this.load(false, cx);
                    })),
            )
        }

        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(tools.child(if narrow {
                let values = [
                    OutcomeFilter::All,
                    OutcomeFilter::Win,
                    OutcomeFilter::Loss,
                    OutcomeFilter::Draw,
                    OutcomeFilter::Unknown,
                    OutcomeFilter::Interrupted,
                ];
                let index = values
                    .iter()
                    .position(|v| {
                        std::mem::discriminant(v) == std::mem::discriminant(&self.query.outcome)
                    })
                    .unwrap_or(0);
                div().child(
                    small(
                        "compact-outcome-filter",
                        format!(
                            "Result: {}",
                            ["All", "Win", "Loss", "Draw", "Unknown", "Interrupted"][index]
                        ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.query.outcome = values[(index + 1) % 6].clone();
                        this.query.offset = 0;
                        this.load(false, cx);
                    })),
                )
            } else {
                outcomes
            }))
            .child(actions)
            .child(ui::caption(
                t,
                format!(
                    "{} matching recordings · select two to compare",
                    self.library.filtered_total
                ),
            ));
        if self.library.rows.is_empty() {
            return body.child(ui::empty_state(
                t,
                "search_off",
                "No matching fights",
                "Try a different opponent, server, result, or instance.",
            ));
        }

        if !narrow {
            body = body.child(history_header(t));
        }
        let entries: Vec<_> = history_entries(&self.library.rows)
            .into_iter()
            .filter(|e| matches!(e, HistoryEntry::Fight(_)))
            .collect();
        let callback_entries = entries.clone();
        let list = gpui::uniform_list(
            "pulsar-history-rows",
            entries.len(),
            cx.processor(
                move |this: &mut Self, range: std::ops::Range<usize>, _, cx| {
                    let t = crate::theme::theme(cx).clone();
                    range
                        .map(|i| {
                            let row = match &callback_entries[i] {
                                HistoryEntry::Day(day) => {
                                    ui::row(&t, false).child(ui::section_header(
                                        &t,
                                        day.clone(),
                                        Some("UTC recording date".into()),
                                    ))
                                }

                                HistoryEntry::Fight(index) => this.fight_row(
                                    &t,
                                    &this.library.rows[*index],
                                    *index,
                                    true,
                                    narrow,
                                    cx,
                                ),
                            };
                            let grouped = match &callback_entries[i] {
                                HistoryEntry::Fight(index)
                                    if *index == 0
                                        || this.library.rows[*index - 1].date_group
                                            != this.library.rows[*index].date_group =>
                                {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            ui::caption(
                                                &t,
                                                this.library.rows[*index].date_group.clone(),
                                            )
                                            .px(px(16.)),
                                        )
                                        .child(row)
                                }
                                _ => row,
                            };
                            grouped
                                .w_full()
                                .min_w(px(0.))
                                .h(px(if narrow { 84. } else { 64. }))
                                .into_any_element()
                        })
                        .collect::<Vec<_>>()
                },
            ),
        )
        .track_scroll(&self.history_scroll)
        .h(px(480.))
        .w_full();
        body=body.child(ui::list_card(t).child(div().id("pulsar-history-keyboard").track_focus(&self.history_focus).tab_index(0)
            .aria_label("Fight history. Up and Down select, Enter opens. Home and End jump to the first or last recording.")
            .on_key_down(cx.listener(|this,e:&KeyDownEvent,_,cx|{
                if this.library.rows.is_empty(){return;}
                let current=this.selected.as_ref().and_then(|key|this.library.rows.iter().position(|r|&r.key==key)).unwrap_or(0);
                let index=match e.keystroke.key.as_str(){"up"=>current.saturating_sub(1),"down"=>(current+1).min(this.library.rows.len()-1),"home"=>0,"end"=>this.library.rows.len()-1,"enter"=>{this.open_detail(this.library.rows[current].key.clone(),cx);cx.stop_propagation();return;},_=>return};
                this.selected=Some(this.library.rows[index].key.clone());
                this.history_scroll.scroll_to_item(index,gpui::ScrollStrategy::Center);
                cx.stop_propagation();cx.notify();
            })).child(list)));
        if self.library.has_more {
            body = body.child(
                small("pulsar-more", "Next 100 recordings").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.query.offset += 100;
                        this.load(false, cx);
                    },
                )),
            );
        }

        if self.query.offset > 0 {
            body = body.child(
                small("pulsar-first", "Back to newest page").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.query.offset = 0;
                        this.load(false, cx);
                    },
                )),
            );
        }

        body
    }

    pub(super) fn focus_navigation(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let focus = self.analysis.as_ref().and_then(|a| a.focus.as_ref());
        let index = focus
            .and_then(|k| self.library.rows.iter().position(|r| &r.key == k))
            .unwrap_or(0);
        let mut controls = div().flex().flex_wrap().gap(px(8.));
        for newer in [true, false] {
            let target = if newer {
                index.checked_sub(1)
            } else {
                Some(index + 1)
            }
            .and_then(|i| self.library.rows.get(i));
            let label = target
                .map(|f| {
                    format!(
                        "{} fight: {}, {}",
                        if newer { "Next" } else { "Previous" },
                        f.opponent,
                        utc_time(f.start_epoch_millis).trim_end_matches(" UTC")
                    )
                })
                .unwrap_or_else(|| format!("No {} fight", if newer { "newer" } else { "earlier" }));
            controls = controls.child(
                small(if newer { "flow-newer" } else { "flow-older" }, label)
                    .variant(if target.is_none() {
                        ButtonVariant::Subtle
                    } else {
                        ButtonVariant::Ghost
                    })
                    .disabled(target.is_none())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let i = if newer {
                            index.saturating_sub(1)
                        } else {
                            (index + 1).min(this.library.rows.len().saturating_sub(1))
                        };
                        if let Some(row) = this.library.rows.get(i) {
                            this.selected = Some(row.key.clone());
                            this.combo_selection = 0;
                            this.load_sections();
                            cx.notify();
                        }
                    })),
            );
        }

        if let Some(key) = focus.cloned() {
            controls = controls
                .child(small("flow-open-focused", "Open focused fight").on_click(
                    cx.listener(move |this, _, _, cx| this.open_detail(key.clone(), cx)),
                ));
        }

        let identity = focus.and_then(|k| self.library.rows.iter().find(|r| &r.key == k));
        let mut focus_bar = ui::card(t);
        if let Some(f) = identity {
            focus_bar = focus_bar.child(ui::section_header(
                t,
                format!("Focused fight · {}", f.opponent),
                Some(
                    format!(
                        "{} · {} · {:.1} s",
                        f.provenance.instance_name,
                        utc_time(f.start_epoch_millis),
                        f.duration_nanos as f64 / 1e9
                    )
                    .into(),
                ),
            ));
        }

        focus_bar.child(controls)
    }

    fn opponents_body(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let mut list = ui::list_card(t).min_w(px(844.)).w_full().child(
            ui::row(t, false)
                .py(px(6.))
                .gap(px(8.))
                .child(ui::caption(t, "Opponent").flex_1().min_w(px(98.)))
                .children(
                    [
                        ("Wins", 34.),
                        ("Losses", 40.),
                        ("Draws", 38.),
                        ("Unknown", 54.),
                    ]
                    .into_iter()
                    .map(|(v, width)| ui::caption(t, v).w(px(width)).flex_shrink_0()),
                )
                .child(ui::caption(t, "Accuracy").w(px(68.)))
                .child(ui::caption(t, "Reach").w(px(56.)))
                .child(ui::caption(t, "Latest fight").w(px(132.)))
                .child(div().w(px(132.))),
        );
        for (i, o) in self.opponents.iter().enumerate() {
            let latest = o.latest.as_ref();
            let key = latest
                .map(|f| f.key.clone())
                .or_else(|| o.fights.first().cloned());
            let opponent = o.label.clone();
            list = list.child(
                ui::row(t, i + 1 == self.opponents.len())
                    .py(px(8.))
                    .gap(px(8.))
                    .child(
                        ui::title_block(
                            t,
                            o.label.clone(),
                            Some(super::presentation::count(o.fights.len(), "fight").into()),
                        )
                        .flex_1()
                        .min_w(px(98.)),
                    )
                    .children(
                        [
                            (o.wins, t.success, 34.),
                            (o.losses, t.danger, 40.),
                            (o.draws, t.foreground, 38.),
                            (o.unknown, t.foreground, 54.),
                        ]
                        .into_iter()
                        .map(|(n, color, width)| {
                            ui::muted(t, n.to_string())
                                .w(px(width))
                                .flex_shrink_0()
                                .text_color(if n == 0 {
                                    t.muted_foreground.opacity(0.5)
                                } else {
                                    color
                                })
                        }),
                    )
                    .children([("accuracy", 68.), ("averageReach", 56.)].into_iter().map(
                        |(key, width)| {
                            ui::muted(
                                t,
                                o.metrics
                                    .get(key)
                                    .filter(|m| m.value.is_some())
                                    .map(tabs::metric_value)
                                    .unwrap_or_else(|| "\u{2014}".into()),
                            )
                            .w(px(width))
                            .id(format!("opponent-metric-{i}-{key}"))
                            .aria_label(
                                o.metrics
                                    .get(key)
                                    .map(tabs::metric_value)
                                    .unwrap_or_else(|| "Unavailable".into()),
                            )
                        },
                    ))
                    .child(div().w(px(132.)).flex().flex_col().gap(px(2.)).when_some(
                        latest,
                        |d, f| {
                            d.child(
                                ui::caption(t, utc_time(f.start_epoch_millis)).whitespace_nowrap(),
                            )
                            .child(div().flex().child(ui::badge(
                                t,
                                f.result_label.clone(),
                                super::presentation::outcome(f),
                            )))
                        },
                    ))
                    .child(
                        div()
                            .w(px(132.))
                            .flex()
                            .flex_wrap()
                            .gap(px(2.))
                            .child(small(("opponent-history", i), "View fights").on_click(
                                cx.listener(move |this, _, window, cx| {
                                    this.query.search = opponent.clone();
                                    this.query.offset = 0;
                                    this.top_tab = 4;
                                    this.search.update(cx, |input, cx| {
                                        input.set_value(opponent.clone(), window, cx)
                                    });
                                    this.load(false, cx);
                                    cx.notify();
                                }),
                            ))
                            .child(
                                small(("opponent-open", i), "Open latest")
                                    .icon("arrow_forward")
                                    .disabled(key.is_none())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(k) = &key {
                                            this.open_detail(k.clone(), cx);
                                        }
                                    })),
                            ),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(ui::section_header(
                t,
                "Opponents",
                Some("Results from your recorded fights".into()),
            ))
            .when(self.width < 1100., |d| {
                d.child(ui::caption(
                    t,
                    "Scroll horizontally for all measurements and actions.",
                ))
            })
            .child(
                div()
                    .id("opponent-table")
                    .min_w(px(0.))
                    .w_full()
                    .overflow_x_scroll()
                    .child(list),
            )
    }

    fn records_body(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let mut rows = ui::list_card(t);
        for (i, r) in self.records.iter().enumerate() {
            let key = r.key.clone();
            let source = r.source.as_ref();
            let record_id = format!("record-definition-{i}");
            let open = self.disclosures.contains(&record_id);
            let value = if r.metric.definition.unit == "s" {
                r.metric
                    .value
                    .map(|v| duration_text((v * 1e9) as i64))
                    .unwrap_or_else(|| "Unavailable".into())
            } else {
                tabs::metric_value(&r.metric)
            };
            rows = rows.child(
                ui::row(t, i + 1 == self.records.len())
                    .py(px(8.))
                    .flex_wrap()
                    .child(
                        div()
                            .w(px(170.))
                            .child(ui::muted(t, r.label.clone()))
                            .when_some(r.minimum_attempts, |d, n| {
                                d.child(ui::caption(t, format!("At least {n} eligible attempts")))
                            }),
                    )
                    .child(
                        div()
                            .w(px(120.))
                            .text_size(px(ui::TEXT_SECTION))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(value),
                    )
                    .child(div().flex_1().min_w(px(160.)).when_some(source, |d, f| {
                        d.child(ui::caption(
                            t,
                            format!("{} \u{b7} {}", f.opponent, utc_time(f.start_epoch_millis)),
                        ))
                    }))
                    .child(div().w(px(125.)).flex().when_some(source, |d, f| {
                        d.child(ui::badge(
                            t,
                            f.result_label.clone(),
                            super::presentation::outcome(f),
                        ))
                    }))
                    .child(
                        small(("record-info", i), "")
                            .icon("info")
                            .accessibility_label(format!("About {}", r.label))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.disclosures.remove(&record_id) {
                                    this.disclosures.insert(record_id.clone());
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        small(("record-open", i), "Open fight")
                            .icon("arrow_forward")
                            .disabled(key.is_none())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(k) = &key {
                                    this.open_detail(k.clone(), cx);
                                }
                            })),
                    )
                    .when(open, |d| {
                        d.child(ui::caption(t, r.metric.definition.help.clone()).w_full())
                    }),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(ui::section_header(
                t,
                "Personal records",
                Some(
                    "Records measure performance; a fight can qualify without a recorded result."
                        .into(),
                ),
            ))
            .child(rows)
    }

    fn settings_body(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(ui::section_header(
            t,
            "Settings inspector",
            Some(
                "Choose an instance above to inspect its saved preferences. Live controls are available in game."
                    .into(),
            ),
        ));
        let Some(s) = &self.settings else {
            return body.child(ui::muted(
                t,
                "Select an instance to inspect its saved Pulsar settings.",
            ));
        };
        body = body.child(ui::section_header(
            t,
            self.library
                .instances
                .iter()
                .find(|i| i.instance_id == s.instance_id)
                .map(|i| format!("Inspecting: {}", i.instance_name))
                .unwrap_or_else(|| "Inspecting selected instance".into()),
            None,
        ));
        let mut groups = std::collections::BTreeMap::<&str, Vec<&SettingView>>::new();
        for row in &s.settings {
            groups
                .entry(
                    if row.key == "schemaVersion" || row.key.starts_with("standalone:") {
                        "Diagnostics"
                    } else {
                        &row.group
                    },
                )
                .or_default()
                .push(row);
        }

        for group in [
            "Recording",
            "Server capture",
            "Combat exchanges",
            "Attack classification",
            "Controls",
            "Audio",
            "Per-style audio",
            "Appearance",
            "Data & exports",
            "Diagnostics",
        ] {
            let Some(rows) = groups.remove(group) else {
                continue;
            };
            let mut list = ui::list_card(t).child(
                ui::row(t, false)
                    .child(ui::section_header(t, group.to_owned(), None).flex_1())
                    .child(ui::badge(t, "Change in game", ui::Tone::Neutral)),
            );
            if group == "Diagnostics" {
                list = list.child(
                    ui::row(t, false).child(ui::caption(
                        t,
                        s.source_path
                            .clone()
                            .unwrap_or_else(|| "No saved configuration; defaults shown".into()),
                    )),
                );
            }
            let count = rows.len();
            for (i, row) in rows.into_iter().enumerate() {
                let full_value = setting_readout(row);
                let value = if row.key == "feedback" {
                    row.value
                        .as_array()
                        .map(|v| super::presentation::count(v.len(), "feedback style"))
                        .unwrap_or_else(|| "Unavailable".into())
                } else {
                    full_value.clone()
                };
                let value_theme = t.clone();
                let help_id = format!("setting-help-{group}-{i}");
                let help_open = self.disclosures.contains(&help_id);
                list = list.child(
                    ui::row(t, i + 1 == count)
                        .py(px(3.))
                        .flex_wrap()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .flex_1()
                                .min_w(px(200.))
                                .child(ui::muted(t, setting_label(&row.key, &row.label)))
                                .child(
                                    small(format!("setting-info-{group}-{i}"), "")
                                        .icon("info")
                                        .accessibility_label(format!("About {}", row.label))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if !this.disclosures.remove(&help_id) {
                                                this.disclosures.insert(help_id.clone());
                                            }
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            ui::muted(t, value)
                                .id(format!("setting-value-{group}-{i}"))
                                .w(px(180.))
                                .flex_shrink_0()
                                .text_right()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .aria_label(full_value.clone())
                                .tooltip(move |_, cx| {
                                    cx.new(|_| super::charts::Readout {
                                        text: full_value.clone(),
                                        theme: value_theme.clone(),
                                    })
                                    .into()
                                }),
                        )
                        .when(help_open, |d| {
                            d.child(ui::caption(t, row.help.clone()).w_full())
                        }),
                );
            }

            body = if group == "Diagnostics" {
                body.child(self.disclosure(t, "settings-diagnostics", "Diagnostics", list, cx))
            } else {
                body.child(list)
            };
        }

        body = body.child(self.disclosure(
            t,
            "shortcuts",
            "Keyboard shortcuts",
            self.shortcut_guide(t),
            cx,
        ));

        for warning in &s.warnings {
            body = body.child(ui::caption(t, warning.clone()));
        }

        body
    }
}

fn history_header(t: &AuroraTheme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(16.))
        .child(ui::caption(t, "Recording").flex_1())
        .child(ui::caption(t, "Hits").w(px(55.)))
        .child(ui::caption(t, "Accuracy").w(px(85.)))
        .child(ui::caption(t, "Reach").w(px(92.)))
        .child(ui::caption(t, "Result").w(px(130.)))
        .child(div().w(px(100.)))
}

/// Human-readable saved values. Raw standalone documents live only in Diagnostics.
fn setting_readout(row: &SettingView) -> String {
    let v = &row.value;
    if v.is_null() {
        return "In-game control".into();
    }
    if matches!(
        row.key.as_str(),
        "accent" | "forwardColor" | "backwardColor" | "neutralColor"
    ) {
        if let Some(n) = v.as_i64() {
            let argb = n as i32 as u32;
            let color = format!("#{:06X}", argb & 0xffffff);
            return if argb >> 24 == 255 {
                color
            } else {
                format!(
                    "{color} ({:.0}% opacity)",
                    (argb >> 24) as f64 * 100. / 255.
                )
            };
        }
    }
    if matches!(row.key.as_str(), "analytics" | "direction" | "alert") && v.is_object() {
        let number = |key: &str, factor: f64, digits: usize| {
            v.get(key)
                .and_then(|v| v.as_f64())
                .map(|n| format!("{:.*}", digits, n * factor))
                .unwrap_or_else(|| "Unavailable".into())
        };
        let flag = |key: &str, yes: &'static str, no: &'static str| {
            v.get(key)
                .and_then(|v| v.as_bool())
                .map(|b| if b { yes } else { no })
                .unwrap_or("Unavailable")
        };
        return format!(
            "Position {}%, {}% \u{b7} Scale {}\u{d7} \u{b7} {}% opacity \u{b7} {} \u{b7} {} \u{b7} {}",
            number("x", 100., 0),
            number("y", 100., 0),
            number("scale", 1., 1),
            number("opacity", 100., 0),
            flag("visible", "Visible", "Hidden"),
            flag("background", "Background on", "Background off"),
            flag("rightAligned", "Right aligned", "Left aligned")
        );
    }
    if row.key == "feedback" {
        if let Some(styles) = v.as_array() {
            return styles.iter().enumerate().map(|(i, style)| format!("Style {}: {} \u{b7} Color {} \u{b7} Volume {} \u{b7} Pitch {} \u{b7} Visual {} \u{b7} Audio {}", i + 1, style["sound"].as_str().unwrap_or("Unavailable"), style.get("color").and_then(|v| v.as_i64()).map(|n| format!("#{:06X}", n as i32 as u32 & 0xffffff)).unwrap_or_else(|| "Unavailable".into()), style.get("volume").map(ToString::to_string).unwrap_or_else(|| "Unavailable".into()), style.get("pitch").map(ToString::to_string).unwrap_or_else(|| "Unavailable".into()), style.get("visual").map(ToString::to_string).unwrap_or_else(|| "Unavailable".into()), style.get("audio").map(ToString::to_string).unwrap_or_else(|| "Unavailable".into()))).collect::<Vec<_>>().join("; ");
        }
    }
    v.as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| v.to_string())
}

fn setting_label(key: &str, fallback: &str) -> String {
    match key {
        "schemaVersion" => "Saved format version",
        "alertSeconds" => "Alert duration (seconds)",
        "retention" => "Recordings to keep",
        "maxStorageMb" => "Storage limit (MB)",
        "serverInactivitySeconds" => "Server timeout (seconds)",
        "resultGraceSeconds" => "Result wait (seconds)",
        "inactivitySeconds" => "Fight timeout (seconds)",
        "exchangeInactivitySeconds" => "Exchange timeout (seconds)",
        "comboSeconds" => "Combo timeout (seconds)",
        "correlationMillis" => "Hit evidence window (milliseconds)",
        "jumpWindowMillis" => "Jump window (milliseconds)",
        "jumpWindowTicks" => "Jump window (ticks)",
        "perfectReachTarget" => "Target reach (metres)",
        "perfectReachTolerance" => "Reach tolerance (metres)",
        "timingToleranceTicks" => "Timing tolerance (ticks)",
        "analytics" => "Analytics HUD layout",
        "direction" => "Direction HUD layout",
        "alert" => "Alert HUD layout",
        "hudOverlay" => "HUD overlay",
        "feedback" => "Hit feedback styles",
        "chartGrid" => "Chart gridlines",
        "chartPoints" => "Chart point markers",
        "trendWindow" => "Recordings in trends",
        "animate" => "Interface animations",
        "summary" => "Post-fight summary",
        "tracking" => "Fight tracking",
        "recording" => "Save recordings",
        _ => {
            let mut label = super::analysis_ui::display_label(fallback).to_lowercase();
            if let Some(first) = label.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            return label;
        }
    }
    .into()
}

/// PageUp/PageDown/Home/End over the launcher's page scroll. Offsets are

/// negative downward, as GPUI stores them.
pub(super) fn scroll_target(key: &str, offset: f32, max: f32, page: f32) -> Option<f32> {
    let step = (page * 0.9).max(40.);
    let y = match key {
        "pageup" => offset + step,
        "pagedown" => offset - step,
        "home" => 0.,
        "end" => -max,
        _ => return None,
    };
    Some(y.clamp(-max.max(0.), 0.))
}

impl gpui::Render for Pulsar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.remember_scroll(window, cx);
        let body = self.page_body(window, cx);
        div()
            .id("pulsar-page")
            .track_focus(&self.root_focus)
            .flex()
            .flex_col()
            .min_w(px(0.))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                let key = e.keystroke.key.as_str();
                if e.keystroke.modifiers.control || e.keystroke.modifiers.alt {
                    return;
                }

                if this.on_key(key, window, cx) {
                    cx.stop_propagation();
                    return;
                }

                if this.typing(window, cx) {
                    return;
                }

                if let Some(scroll) = &this.page_scroll {
                    let max = scroll.max_offset().y.as_f32();
                    let page = scroll.bounds().size.height.as_f32();
                    if let Some(y) = scroll_target(key, scroll.offset().y.as_f32(), max, page) {
                        scroll.set_offset(gpui::point(px(0.), px(y)));
                        cx.stop_propagation();
                        cx.notify();
                    }
                }
            }))
            .child(body)
    }
}

impl Pulsar {
    fn page_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let t = crate::theme::theme(cx).clone();
        self.width = window.viewport_size().width.as_f32();
        let narrow = self.width < 1100.;
        if self.compare_open {
            return self.compare_body(&t, cx);
        }

        if self.detail_request.is_some() {
            return self.detail_body(&t, narrow, cx);
        }

        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(if self.width < 950. { 8. } else { 16. }))
            .min_w(px(0.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .justify_between()
                    .gap(px(16.))
                    .child(
                        ui::page_header(
                            &t,
                            "Pulsar",
                            Some("Understand your fights and choose what to improve next.".into()),
                        )
                        .flex_1()
                        .min_w(px(100.)),
                    )
                    .child(
                        AuroraButton::new(
                            "pulsar-refresh",
                            if self.loading {
                                "Refreshing…"
                            } else {
                                "Refresh"
                            },
                        )
                        .icon("refresh")
                        .disabled(self.loading || self.busy)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .child(self.scope_bar(&t, cx));
        let mut strip = div()
            .flex()
            .flex_wrap()
            .gap(px(2.))
            .p(px(2.))
            .rounded(t.radius)
            .bg(t.sunken)
            .border_1()
            .border_color(t.border_subtle)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                let delta = match e.keystroke.key.as_str() {
                    "left" => 7,
                    "right" => 1,
                    _ => return,
                };
                cx.stop_propagation();
                this.top_tab = (this.top_tab + delta) % 8;
                this.load_sections();
                cx.notify();
            }));
        for (index, label) in TOP_TABS.iter().enumerate().take(if self.width < 900. {
            0
        } else if narrow {
            4
        } else {
            8
        }) {
            strip = strip.child(
                small(("pulsar-tab", index), *label)
                    .selected(self.top_tab == index)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.top_tab = index;
                        this.load_sections();
                        cx.notify();
                    })),
            );
        }

        if self.width < 900. {
            strip = strip.bg(gpui::transparent_black()).border_0().p(px(0.));
        }
        if narrow {
            let view = cx.entity();
            let hi = view.clone();
            let confirm = view.clone();
            let first = if self.width < 900. { 0 } else { 4 };
            strip = strip.child(
                AuroraSelect::new(
                    "pulsar-more-tabs",
                    if self.width < 900. {
                        "View"
                    } else {
                        "More views"
                    },
                    TOP_TABS[if self.width < 900. { 0 } else { 4 }..]
                        .iter()
                        .map(|v| (*v).into())
                        .collect(),
                    &self.tab_trigger,
                    &self.tab_content,
                    self.tab_bounds.clone(),
                )
                .selected(self.top_tab.checked_sub(first))
                .open(self.tab_open)
                .highlight(self.tab_highlight)
                .width(px(if self.width < 900. {
                    (self.width - 100.).max(200.)
                } else {
                    132.
                }))
                .on_open_change(move |v, _, cx| {
                    view.update(cx, |this, cx| {
                        this.tab_open = v;
                        cx.notify();
                    })
                })
                .on_highlight(move |v, _, cx| {
                    hi.update(cx, |this, cx| {
                        this.tab_highlight = v;
                        cx.notify();
                    })
                })
                .on_confirm(move |v, _, cx| {
                    confirm.update(cx, |this, cx| {
                        this.tab_open = false;
                        this.top_tab = v + first;
                        this.load_sections();
                        cx.notify();
                    })
                }),
            );
        }
        page = page.child(strip);
        if let Some(error) = &self.error {
            page = page.child(
                ui::card(&t)
                    .child(ui::title_block(
                        &t,
                        "Pulsar could not load this view",
                        Some(error.clone().into()),
                    ))
                    .child(
                        div().flex().child(
                            small("pulsar-retry", "Try again")
                                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                        ),
                    ),
            );
        }

        if !self.message.is_empty() {
            page = page.child(ui::caption(&t, self.message.clone()));
        }

        if self.loading && self.library.rows.is_empty() {
            return page.child(ui::muted(&t, "Reading your launcher library…"));
        }

        if self.library.total == 0 && self.error.is_none() {
            let setup = ui::empty_state(&t, "swords", "Your first fight starts in game", "Enable Pulsar in Aurora Client, then play a fight or start a manual recording.")
                .child(div().flex().flex_wrap().justify_center().gap(px(8.))
                    .child(AuroraButton::new("pulsar-setup", "Open Instances").variant(ButtonVariant::Primary).on_click(cx.listener(|_, _, _, cx| cx.emit(super::PulsarEvent::OpenInstances))))
                    .child(small("pulsar-enable-help", "How to enable Pulsar").on_click(cx.listener(|this, _, _, cx| { if !this.disclosures.remove("enable-help") { this.disclosures.insert("enable-help".into()); } cx.notify(); }))))
                .when(self.disclosures.contains("enable-help"), |d| d.child(ui::muted(&t, "Launch an instance with Aurora Client installed. Open Aurora settings in game, find Pulsar and enable fight tracking and recording. Play a fight, then return here and select Refresh.")));
            return page.child(setup);
        }

        if self.top_tab < 4 {
            let mut window_bar = div().flex().flex_wrap().gap(px(4.)).items_center();
            for (i, (label, window)) in [
                ("Last 10", AnalysisWindow::Last10),
                ("Last 25", AnalysisWindow::Last25),
                ("All", AnalysisWindow::All),
            ]
            .into_iter()
            .enumerate()
            {
                let selected =
                    std::mem::discriminant(&self.window) == std::mem::discriminant(&window);
                window_bar = window_bar.child(
                    small(("pulsar-window", i), label)
                        .selected(selected)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.window = window.clone();
                            this.load_sections();
                            cx.notify();
                        })),
                );
            }

            if let Some(a) = &self.analysis {
                window_bar = window_bar.child(ui::caption(
                    &t,
                    format!(
                        "Showing {} of {} recordings",
                        a.coverage.recordings, self.library.total
                    ),
                ));
            }

            page = page.child(window_bar);
        }

        if self.top_tab == 1 && matches!(self.sections[1], 0 | 2 | 3 | 4) {
            let mut scope_filters = div().flex().flex_wrap().gap(px(8.));
            let mut weapons = std::collections::BTreeSet::new();
            if let Some(a) = &self.analysis {
                for table in &a.offense.tables {
                    for row in &table.rows {
                        for value in row.cells.values() {
                            if value.starts_with("minecraft:") {
                                weapons.insert(value.clone());
                            }
                        }
                    }
                }
            }

            let mut filters = div().flex().flex_wrap().gap(px(4.)).child(
                small("pooled-weapon-all", "All weapons")
                    .selected(self.weapon.is_none())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.weapon = None;
                        this.load_sections();
                        cx.notify();
                    })),
            );
            for (i, w) in weapons.into_iter().enumerate() {
                let id = w.clone();
                filters = filters.child(
                    small(
                        ("pooled-weapon", i),
                        w.strip_prefix("minecraft:").unwrap_or(&w).replace('_', " "),
                    )
                    .selected(self.weapon.as_ref() == Some(&w))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.weapon = Some(id.clone());
                        this.load_sections();
                        cx.notify();
                    })),
                );
            }

            scope_filters = scope_filters.child(filters);
            if matches!(self.sections[1], 0 | 4) {
                let mut results = div().flex().flex_wrap().gap(px(4.));
                for (i, (label, result)) in [
                    ("All attempts", None),
                    ("Hits", Some("HITS")),
                    ("No hit", Some("NO_HIT")),
                ]
                .into_iter()
                .enumerate()
                {
                    results = results.child(
                        small(("pooled-timing-result", i), label)
                            .selected(self.timing_result.as_deref() == result)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.timing_result = result.map(str::to_owned);
                                this.load_sections();
                                cx.notify();
                            })),
                    );
                }

                scope_filters = scope_filters.child(results);
            }
            page = page.child(self.disclosure(
                &t,
                "offense-filters",
                "Weapon and outcome filters",
                scope_filters,
                cx,
            ));
        }

        match self.top_tab {
            4 => page = page.child(self.history(&t, narrow, cx)),
            5 => page = page.child(self.opponents_body(&t, cx)),
            6 => page = page.child(self.records_body(&t, cx)),
            7 => page = page.child(self.settings_body(&t, cx)),
            index => {
                if let Some(a) = &self.analysis {
                    if index == 0 {
                        page = page.child(self.overview_lead(&t, &a.overview, cx));
                        if let Some(latest) = &a.latest {
                            let key = latest.key.clone();
                            page = page.child(
                                ui::card(&t)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .items_center()
                                            .justify_between()
                                            .gap(px(12.))
                                            .child(ui::title_block(
                                                &t,
                                                if latest.manual {
                                                    "Latest manual session".to_owned()
                                                } else {
                                                    format!("Latest fight · {}", latest.opponent)
                                                },
                                                Some(
                                                    format!(
                                                        "{} · {} · {}",
                                                        latest.provenance.instance_name,
                                                        latest.date_group,
                                                        latest.result_label
                                                    )
                                                    .into(),
                                                ),
                                            ))
                                            .child(
                                                AuroraButton::new("pulsar-latest", "Open fight")
                                                    .variant(ButtonVariant::Primary)
                                                    .icon("arrow_forward")
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.open_detail(key.clone(), cx)
                                                        },
                                                    )),
                                            ),
                                    )
                                    .when_some(a.overview.interpretations.first(), |d, insight| {
                                        d.child(ui::muted(&t, insight.clone()))
                                    })
                                    .child(self.latest_denominators(&t, latest, cx)),
                            );
                        }
                    }

                    if index != 0 {
                        page = page.child(self.tab_body(
                            &t,
                            match index {
                                1 => &a.offense,
                                2 => &a.defense,
                                3 => &a.flow,
                                _ => &a.overview,
                            },
                            narrow,
                            cx,
                        ));
                    } else {
                        page = page.child(self.disclosure(
                            &t,
                            "overview-details",
                            "All nine measurement groups",
                            self.overview_groups(&t, &a.overview.metrics, cx),
                            cx,
                        ));
                        let context = div()
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .children(
                                a.overview
                                    .notices
                                    .iter()
                                    .map(|s| ui::caption(&t, s.clone())),
                            )
                            .children(
                                a.overview
                                    .interpretations
                                    .iter()
                                    .skip(1)
                                    .map(|s| ui::muted(&t, s.clone())),
                            );
                        if !a.overview.notices.is_empty() || a.overview.interpretations.len() > 1 {
                            page = page.child(self.disclosure(
                                &t,
                                "interpretation",
                                "Interpretation and limitations",
                                context,
                                cx,
                            ));
                        }
                        page = page.child(ui::caption(
                            &t,
                            "Live capture and recording controls: Change in game.",
                        ));
                    }

                    if index == 0 {
                        let mut recent =
                            ui::list_card(&t).child(ui::row(&t, false).child(ui::section_header(
                                &t,
                                "Recent recordings",
                                None,
                            )));
                        if !narrow {
                            recent = recent.child(history_header(&t));
                        }
                        for (i, row) in self.library.rows.iter().take(5).enumerate() {
                            recent = recent.child(self.fight_row(
                                &t,
                                row,
                                i,
                                i + 1 == self.library.rows.len().min(5),
                                narrow,
                                cx,
                            ));
                        }

                        page = page.child(recent);
                    }
                } else if self.error.is_none() {
                    page = page.child(ui::muted(&t, "Loading analysis…"));
                }
            }
        }

        page = page.child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.))
                .child(
                    small("pulsar-recordings-folder", "Recordings folder")
                        .icon("folder_open")
                        .on_click(
                            cx.listener(|this, _, _, cx| this.folder(FolderKind::Recordings, cx)),
                        ),
                )
                .child(
                    small("pulsar-exports-folder", "Exports folder")
                        .icon("folder_open")
                        .on_click(
                            cx.listener(|this, _, _, cx| this.folder(FolderKind::Exports, cx)),
                        ),
                ),
        );
        for issue in &self.library.errors {
            page = page.child(ui::caption(
                &t,
                format!("{} · {}", issue.code, issue.message),
            ));
        }

        page
    }
}

/// FightList columns that fit a viewport width. Duration, Reach, Date and

/// Result drop in that order (P40); opponent, hits and accuracy always stay.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Columns {
    pub duration: bool,

    pub reach: bool,

    pub date: bool,

    pub result: bool,
}

pub(super) fn history_columns(width: f32) -> Columns {
    Columns {
        duration: width >= 1100.,
        reach: width >= 980.,
        date: width >= 860.,
        result: width >= 760.,
    }
}

#[derive(Clone, Debug, PartialEq)]
enum HistoryEntry {
    Day(String),
    Fight(usize),
}

fn history_entries(rows: &[FightSummary]) -> Vec<HistoryEntry> {
    let mut day = "";
    let mut entries = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if row.date_group != day {
            day = &row.date_group;
            entries.push(HistoryEntry::Day(day.into()));
        }

        entries.push(HistoryEntry::Fight(i));
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_fields_keep_time_and_duration_independent_of_instance_length() {
        assert_eq!(clock_time(0), "00:00");
        assert_eq!(duration_text(303_400_000_000), "5 min 3 s");
        assert_eq!(duration_text(18_000_000_000), "18 s");
        assert_eq!(duration_text(-1), "0 s");
    }

    #[test]
    fn saved_setting_values_use_player_units_and_preserve_layout_flags() {
        let mut row = SettingView {
            key: "accent".into(),
            value: serde_json::json!(-7812950),
            ..Default::default()
        };
        assert!(setting_readout(&row).starts_with('#'));
        assert!(!setting_readout(&row).contains("-7812950"));
        row.key = "analytics".into();
        row.value = serde_json::json!({"x":0.02,"y":0.2,"scale":1.0,"opacity":0.8,"visible":true,"background":false,"rightAligned":true});
        let value = setting_readout(&row);
        for part in [
            "2%",
            "20%",
            "80% opacity",
            "Visible",
            "Background off",
            "Right aligned",
        ] {
            assert!(value.contains(part), "{value}");
        }
        row.value = serde_json::Value::Null;
        assert_eq!(setting_readout(&row), "In-game control");
    }
    #[test]

    fn page_keys_scroll_within_bounds() {
        assert_eq!(scroll_target("pagedown", 0., 1000., 400.), Some(-360.));
        assert_eq!(scroll_target("pageup", -100., 1000., 400.), Some(0.));
        assert_eq!(scroll_target("end", 0., 1000., 400.), Some(-1000.));
        assert_eq!(scroll_target("home", -700., 1000., 400.), Some(0.));
        assert_eq!(scroll_target("pagedown", -900., 1000., 400.), Some(-1000.));
        assert_eq!(scroll_target("pagedown", 0., 0., 400.), Some(0.));
        assert_eq!(scroll_target("a", 0., 1000., 400.), None);
    }

    #[test]

    fn history_columns_drop_in_reference_order() {
        let wide = history_columns(1400.);
        assert!(wide.duration && wide.reach && wide.date && wide.result);
        let order = [1050., 950., 800., 700.].map(history_columns);
        assert!(!order[0].duration && order[0].reach);
        assert!(!order[1].reach && order[1].date);
        assert!(!order[2].date && order[2].result);
        assert!(!order[3].result);
    }

    #[test]

    fn history_keeps_date_groups_and_stable_fight_indices() {
        let rows = vec![
            FightSummary {
                date_group: "2026-11-14".into(),
                ..Default::default()
            },
            FightSummary {
                date_group: "2026-11-14".into(),
                ..Default::default()
            },
            FightSummary {
                date_group: "2026-11-13".into(),
                ..Default::default()
            },
        ];
        assert_eq!(
            history_entries(&rows),
            vec![
                HistoryEntry::Day("2026-11-14".into()),
                HistoryEntry::Fight(0),
                HistoryEntry::Fight(1),
                HistoryEntry::Day("2026-11-13".into()),
                HistoryEntry::Fight(2)
            ]
        );
    }
}

fn short_label(value: &str, max: usize) -> String {
    if value.chars().count() > max {
        format!(
            "{}...",
            value
                .chars()
                .take(max.saturating_sub(3))
                .collect::<String>()
        )
    } else {
        value.into()
    }
}

pub(super) fn utc_time(epoch_millis: i64) -> String {
    let seconds = epoch_millis.div_euclid(1000);
    let day = seconds.div_euclid(86400);
    let clock = seconds.rem_euclid(86400);
    let z = day + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let date = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{date:02} {:02}:{:02} UTC",
        clock / 3600,
        clock / 60 % 60
    )
}

pub(super) fn clock_time(epoch_millis: i64) -> String {
    utc_time(epoch_millis)
        .split_whitespace()
        .nth(1)
        .unwrap_or("Time unavailable")
        .to_owned()
}
pub(super) fn duration_text(nanos: i64) -> String {
    let seconds = (nanos.max(0) as f64 / 1e9).round() as u64;
    if seconds >= 60 {
        format!("{} min {} s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds} s")
    }
}
