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
        let options = std::iter::once("All instances".into())
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
                    super::presentation::count(self.library.total, "recording"),
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
        ui::row(t, last)
            .py(px(8.))
            .w_full()
            .overflow_hidden()
            .min_w(px(0.))
            .when(chosen, |d| d.bg(t.accent_soft))
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
                    .gap(px(4.))
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
                        ui::caption(t, {
                            let mut parts = vec![row.provenance.instance_name.clone()];
                            parts.push(utc_time(row.start_epoch_millis));
                            parts.push(format!("{:.1}\u{a0}s", row.duration_nanos as f64 / 1e9));

                            parts.extend(row.server.clone());
                            if row.stale {
                                parts.push("Source changed since import".into());
                            }

                            if !row.provenance.conflicts.is_empty() {
                                parts.push("Conflicting snapshot".into());
                            }

                            parts.join(" · ")
                        })
                        .w_full()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap(),
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
                        .on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.open_detail(open_key.clone(), cx)
                            }),
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
                                .h(px(if narrow { 140. } else { 100. }))
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
        for (newer, label) in [(true, "Newer recording"), (false, "Older recording")] {
            controls = controls.child(
                small(if newer { "flow-newer" } else { "flow-older" }, label)
                    .disabled(if newer {
                        index == 0
                    } else {
                        index + 1 >= self.library.rows.len()
                    })
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
        let mut list = ui::list_card(t).child(ui::row(t, false).child(ui::section_header(
            t,
            "Opponents",
            Some("Results from your recorded fights".into()),
        )));
        for (i, o) in self.opponents.iter().enumerate() {
            let key = o.fights.first().cloned();
            let latest = key
                .as_ref()
                .and_then(|key| self.library.rows.iter().find(|f| &f.key == key));
            let opponent = o.label.clone();
            let mut record = div().flex().gap(px(16.));
            for (label, n, tone) in [
                ("Wins", o.wins, ui::Tone::Success),
                ("Losses", o.losses, ui::Tone::Danger),
                ("Draws", o.draws, ui::Tone::Neutral),
                ("Unknown", o.unknown, ui::Tone::Neutral),
            ] {
                record = record.child(div().min_w(px(44.)).child(ui::caption(t, label)).child(
                    ui::badge(
                        t,
                        n.to_string(),
                        if n == 0 { ui::Tone::Neutral } else { tone },
                    ),
                ));
            }

            list = list.child(
                ui::row(t, i + 1 == self.opponents.len())
                    .flex_wrap()
                    .child(
                        ui::title_block(
                            t,
                            o.label.clone(),
                            Some(
                                format!(
                                    "{} · Last fought {}",
                                    super::presentation::count(o.fights.len(), "fight"),
                                    utc_time(o.last_fought_epoch_millis)
                                )
                                .into(),
                            ),
                        )
                        .flex_1()
                        .min_w(px(180.)),
                    )
                    .child(record)
                    .child(tabs::stats(t, &o.metrics, &["accuracy", "averageReach"]))
                    .when_some(latest, |d, f| {
                        d.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(ui::caption(t, utc_time(f.start_epoch_millis)))
                                .child(div().flex().child(ui::badge(
                                    t,
                                    f.result_label.clone(),
                                    super::presentation::outcome(f),
                                ))),
                        )
                    })
                    .child(
                        small(("opponent-history", i), "View fights").on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.query.search = opponent.clone();
                                this.query.offset = 0;
                                this.top_tab = 4;
                                this.search.update(cx, |input, cx| {
                                    input.set_value(opponent.clone(), window, cx)
                                });
                                this.load(false, cx);
                                cx.notify();
                            },
                        )),
                    )
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
            );
        }
        list
    }

    fn records_body(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(ui::section_header(
                t,
                "Personal records",
                Some("Your best recorded performances".into()),
            ));
        for (i, r) in self.records.iter().enumerate() {
            let key = r.key.clone();
            let source = key
                .as_ref()
                .and_then(|k| self.library.rows.iter().find(|f| &f.key == k));
            let record_id = format!("record-definition-{i}");
            let open = self.disclosures.contains(&record_id);
            list = list.child(
                ui::card(t)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .flex_wrap()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(160.))
                                    .child(ui::section_header(t, r.label.clone(), None))
                                    .when_some(source, |d, f| {
                                        d.child(ui::caption(
                                            t,
                                            format!(
                                                "{} \u{b7} {}",
                                                f.opponent,
                                                utc_time(f.start_epoch_millis)
                                            ),
                                        ))
                                    })
                                    .child(ui::caption(
                                        t,
                                        r.minimum_attempts
                                            .map(|n| format!("Requires {n} eligible attempts"))
                                            .unwrap_or_else(|| "Best measured performance".into()),
                                    )),
                            )
                            .child(
                                div()
                                    .w(px(130.))
                                    .flex_shrink_0()
                                    .text_right()
                                    .text_size(px(ui::TEXT_TITLE))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(t.foreground)
                                    .child(tabs::metric_value(&r.metric)),
                            )
                            .when_some(source, |d, f| {
                                d.child(div().w(px(150.)).flex().child(ui::badge(
                                    t,
                                    f.result_label.clone(),
                                    super::presentation::outcome(f),
                                )))
                            })
                            .child(
                                small(("record-info", i), "")
                                    .icon("info")
                                    .accessibility_label(format!("About {}", r.label))
                                    .selected(open)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.disclosures.remove(&record_id) {
                                            this.disclosures.insert(record_id.clone());
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                small(("record-open", i), "Open fight")
                                    .disabled(key.is_none())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(k) = &key {
                                            this.open_detail(k.clone(), cx);
                                        }
                                    })),
                            ),
                    )
                    .when(open, |d| {
                        d.child(ui::muted(t, r.metric.definition.help.clone()))
                    }),
            );
        }
        list
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
                "Saved preferences and recording policies. Live controls are available in game."
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
                .map(|i| i.instance_name.clone())
                .unwrap_or_else(|| "Selected instance preferences".into()),
            None,
        ));
        let mut groups = std::collections::BTreeMap::<&str, Vec<&SettingView>>::new();
        for row in &s.settings {
            groups
                .entry(if row.key == "schemaVersion" {
                    "Diagnostics"
                } else {
                    &row.group
                })
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
                    .child(ui::caption(t, "Change in game")),
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
                let value = if row.value.is_null() {
                    "In-game control".into()
                } else if let Some(s) = row.value.as_str() {
                    s.to_owned()
                } else {
                    row.value.to_string()
                };
                let value_theme = t.clone();
                let full_value = value.clone();
                let help_id = format!("setting-help-{group}-{i}");
                let help_open = self.disclosures.contains(&help_id);
                list = list.child(
                    ui::row(t, i + 1 == count)
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
        "chartGrid" => "Chart gridlines",
        "chartPoints" => "Chart point markers",
        "trendWindow" => "Recordings in trends",
        "animate" => "Interface animations",
        "summary" => "Post-fight summary",
        "tracking" => "Fight tracking",
        "recording" => "Save recordings",
        _ => return super::analysis_ui::display_label(fallback),
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
            .gap(px(16.))
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
                .width(px(132.))
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
            return page.child(ui::empty_state(&t,"swords","Your first fight starts in game","Enable Pulsar in Aurora Client settings, then play a fight or start a manual recording. Return here and refresh to keep it in your library."))
                .child(div().flex().child(AuroraButton::new("pulsar-setup","Open Instances").variant(ButtonVariant::Primary).on_click(cx.listener(|_,_,_,cx|cx.emit(super::PulsarEvent::OpenInstances)))));
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
                        "{} recordings · {} timelines · {} with context",
                        a.coverage.recordings,
                        a.coverage.timeline_recordings,
                        a.coverage.context_recordings
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
