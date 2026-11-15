use super::presentation::ui;
use super::{DETAIL_TABS, Pulsar, Reply, tabs};
use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, AuroraCheckbox, AuroraInput, ButtonSize, ButtonVariant},
    theme::AuroraTheme,
};
use aurora_launcher_lib::pulsar::view::*;
use gpui::{Context, Div, KeyDownEvent, div, prelude::*, px};
use gpui_kit::base::Dialog;

fn button(id: impl Into<gpui::ElementId>, label: impl Into<gpui::SharedString>) -> AuroraButton {
    AuroraButton::new(id, super::presentation::text(label.into().as_ref()))
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Small)
}

impl Pulsar {
    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let (Some(host), Some(request)) = (self.host.clone(), self.detail_request.clone()) else {
            return;
        };
        self.busy = true;
        let tx = self.tx.clone();
        let also_delete_from_instance = self.delete_source;
        self.runtime.spawn(async move {
            let r = guarded(api::pulsar_delete(
                host,
                DeleteRequest {
                    key: request.key,
                    confirmed: true,
                    also_delete_from_instance,
                },
            ))
            .await;
            let _ = tx.send(Reply::Action(r, true)).await;
        });
        cx.notify();
    }
    fn confirm_export(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let (Some(host), Some(request)) = (self.host.clone(), self.detail_request.clone()) else {
            return;
        };
        let destination = self.export_path.read(cx).value().to_string();
        if destination.trim().is_empty() {
            self.message = "Choose a destination folder for the report.".into();
            return;
        }
        self.busy = true;
        self.export_open = false;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::pulsar_export(
                host,
                ExportRequest {
                    key: request.key,
                    destination,
                    confirmed: true,
                },
            ))
            .await;
            let _ = tx.send(Reply::Action(r, false)).await;
        });
        cx.notify();
    }
    pub(super) fn detail_body(&self, t: &AuroraTheme, narrow: bool, cx: &mut Context<Self>) -> Div {
        let summary = self.detail.as_ref().map(|d| &d.summary).or_else(|| {
            self.selected
                .as_ref()
                .and_then(|k| self.library.rows.iter().find(|r| &r.key == k))
        });
        let mut header = div().flex().flex_wrap().items_center().gap(px(8.));
        if let Some(s) = summary {
            header = header
                .child(
                    ui::page_header(
                        t,
                        if s.manual
                            && self
                                .detail_request
                                .as_ref()
                                .is_some_and(|r| r.exchange_id.is_some() || r.participant.is_some())
                        {
                            format!("Exchange with {}", s.opponent)
                        } else if s.manual {
                            "Manual session".to_owned()
                        } else {
                            format!("Fight against {}", s.opponent)
                        },
                        Some(
                            format!(
                                "{} · {} · {:.1} s",
                                s.provenance.instance_name,
                                super::page::utc_time(s.start_epoch_millis),
                                s.duration_nanos as f64 / 1e9
                            )
                            .into(),
                        ),
                    )
                    .flex_1(),
                )
                .child(ui::badge(
                    t,
                    s.result_label.clone(),
                    super::presentation::outcome(s),
                ));
        }
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .min_w(px(0.))
            .child(
                div().flex().child(
                    button("fight-back", "Back to library")
                        .icon("arrow_back")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_detail();
                            cx.notify();
                        })),
                ),
            )
            .child(header);
        if let Some(s) = summary {
            page = page.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .child(
                        button("fight-report", "View report")
                            .icon("open_in_new")
                            .disabled(!s.report_available)
                            .on_click({
                                let key = s.key.clone();
                                cx.listener(move |this, _, _, _| this.view_report(key.clone()))
                            }),
                    )
                    .child(
                        button("fight-export", "Export report")
                            .icon("download")
                            .disabled(!s.report_available || self.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.export_open = true;
                                cx.notify();
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        button("fight-delete", "Delete fight")
                            .variant(ButtonVariant::DangerSubtle)
                            .icon("delete")
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.delete_open = true;
                                this.delete_source = false;
                                this.pause();
                                cx.notify();
                            })),
                    ),
            );
        }
        if let Some(error) = &self.error {
            page = page.child(ui::card(t).child(ui::title_block(
                t,
                "Detail unavailable",
                Some(error.clone().into()),
            )));
        }
        if !self.message.is_empty() {
            page = page.child(ui::caption(t, self.message.clone()));
        }
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
                    "left" => 5,
                    "right" => 1,
                    _ => return,
                };
                cx.stop_propagation();
                this.pause();
                this.detail_tab = (this.detail_tab + delta) % 6;
                cx.notify();
            }));
        for (index, label) in DETAIL_TABS.iter().enumerate() {
            strip = strip.child(
                button(("fight-tab", index), *label)
                    .selected(self.detail_tab == index)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.pause();
                        this.detail_tab = index;
                        if index == 4 {
                            this.playback_focus.focus(window, cx);
                        }
                        cx.notify();
                    })),
            );
        }
        page = page.child(div().flex().child(strip));
        if self.detail_loading {
            if let Some(s) = summary {
                page = page.child(tabs::metric_card(
                    t,
                    "Recorded summary",
                    &s.metrics,
                    &["accuracy", "hits", "averageReach", "longestCombo"],
                ));
            }
            return page.child(ui::muted(t, "Loading the recorded timeline…"));
        }
        let Some(detail) = &self.detail else {
            return page;
        };
        if detail.summary.manual || !detail.exchanges.is_empty() {
            page = page.child(self.focus_scopes(t, cx));
            if detail.exchanges.is_empty() {
                page = page.child(ui::empty_state(
                    t,
                    "swords",
                    "No combat in this manual session",
                    "The session was recorded, but no exchange with another player or mob was observed.",
                ));
            } else if self.detail_tab == 0 {
                // P63: the exchange inventory, independent of the parent session result.
                let mut list = ui::list_card(t).child(ui::row(t, false).child(
                    ui::section_header(
                        t,
                        "Exchanges",
                        Some("Each exchange keeps its own result; the session has none of its own opponent.".into()),
                    ),
                ));
                for (i, e) in detail.exchanges.iter().enumerate() {
                    let (label, tone) = super::presentation::resolution(&e.resolution);
                    list = list.child(
                        ui::row(t, i + 1 == detail.exchanges.len())
                            .id(("manual-exchange-row", i))
                            .role(gpui::Role::Group)
                            .aria_label(format!(
                                "Exchange {}: {}, {}, {:.1} seconds",
                                i + 1,
                                e.opponent,
                                e.resolution.label,
                                e.duration_nanos as f64 / 1e9
                            ))
                            .child(ui::caption(t, format!("#{}", i + 1)).w(px(40.)))
                            .child(ui::muted(t, e.opponent.clone()).flex_1().min_w(px(0.)))
                            .child(div().w(px(150.)).flex().child(ui::badge(
                                t,
                                label.to_owned(),
                                tone,
                            )))
                            .child(
                                ui::caption(t, format!("{:.1} s", e.duration_nanos as f64 / 1e9))
                                    .w(px(64.)),
                            ),
                    );
                }
                page = page.child(list);
            }
        }
        match self.detail_tab {
            4 => page = page.child(self.playback_body(t, cx)),
            5 => {
                let r = &detail.summary.resolution;
                page=page.child(ui::card(t).child(ui::section_header(t,"Why this result was recorded",None))
                    .child(ui::muted(t,format!("{} · {}",detail.summary.result_label,r.source)))
                    .children(r.notes.iter().map(|n|ui::muted(t,n.clone())))
                    .child(ui::caption(t,format!("Recorded in {} on {}. Your launcher retains this snapshot independently of the instance.",detail.summary.provenance.instance_name,super::page::utc_time(detail.summary.start_epoch_millis)))));
                let full_path = detail.summary.provenance.source_path.clone();
                let copy_path = full_path.clone();
                let diagnostics=ui::card(t).child(ui::section_header(t,"Recording diagnostics",None))
                    .child(ui::caption(t,format!("Recording identifier: {}",detail.summary.key.fight_id)))
                    .child(ui::caption(t,format!("Classification: {}. Confidence: {}. Scope: {}.",r.classification,r.confidence,r.scope)))
                    .child(ui::caption(t,format!("Source: …/{}",full_path.rsplit(['/', '\\']).next().unwrap_or("recording.json"))))
                    .child(div().flex().flex_wrap().gap(px(8.)).child(button("copy-recording-path","Copy full path").on_click(move|_,_,cx|cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_path.clone()))))
                        .child(button("open-recording-source","Open recordings folder").on_click(cx.listener(|this,_,_,cx|this.folder(FolderKind::Recordings,cx)))))
                    .children(detail.summary.provenance.conflicts.iter().map(|c|ui::caption(t,format!("Conflicting snapshot: {c}"))))
                    .child(ui::caption(t,if detail.summary.stale{"The source changed after import. This view shows the retained snapshot."}else{"The retained snapshot matches the imported source."}));
                let diagnostics =
                    diagnostics.child(self.table_view(t, &detail.evidence, narrow, cx));
                page =
                    page.child(self.disclosure(t, "diagnostics", "Diagnostics", diagnostics, cx));
                let mut evidence = detail.evidence.clone();
                evidence.columns.retain(|c| c != "nanoTime");
                page = page.child(self.disclosure(
                    t,
                    "result-evidence",
                    "Result evidence",
                    self.table_view(t, &evidence, narrow, cx),
                    cx,
                ));
                let events = detail
                    .timeline
                    .iter()
                    .filter(|e| {
                        self.event_filter
                            .as_deref()
                            .map(|kind| e.kind == kind)
                            .unwrap_or(e.kind != "MOTION")
                    })
                    .collect::<Vec<_>>();
                page = page
                    .child(self.event_filters(cx))
                    .child(self.event_table(t, &events));
            }
            i => {
                let tab = match i {
                    1 => &detail.offense,
                    2 => &detail.defense,
                    3 => &detail.flow,
                    _ => &detail.summary_tab,
                };
                page = page.child(self.tab_body(t, tab, narrow, cx));
            }
        }
        if let Some(termination) = &detail.termination {
            page = page.child(ui::caption(t, format!("Finalization: {termination}")));
        }
        let view = cx.entity();
        let changed = view.clone();
        let cancel = view.clone();
        let mut panel=ui::card(t).w(px(if narrow{400.}else{480.})).max_w(gpui::relative(0.94)).bg(t.raised).child(ui::section_header(t,"Delete this fight?",None))
            .child(ui::muted(t,format!("{} · {} · {} hits. Delete the launcher copy and prevent re-import. Existing exports are retained.",detail.summary.opponent,detail.summary.date_group,detail.summary.metrics.get("hits").map(tabs::metric_value).unwrap_or_else(||"Unavailable".into()))))
            .child(div().flex().items_center().gap(px(8.)).child(AuroraCheckbox::new("delete-source","Also delete the recording from the instance").checked(self.delete_source).disabled(self.busy).on_change(move|checked,_,_,cx|{view.update(cx,|this,cx|{this.delete_source=checked;cx.notify();});})).child(ui::muted(t,"Also delete the recording from the instance")))
            .child(ui::caption(t,"Deleting the instance recording is refused while that instance is running."));
        panel = panel.child(
            div()
                .flex()
                .justify_end()
                .gap(px(8.))
                .child(
                    button("delete-cancel", "Cancel")
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.delete_open = false;
                            cx.notify();
                        })),
                )
                .child(
                    AuroraButton::new(
                        "delete-confirm",
                        if self.busy {
                            "Deleting…"
                        } else {
                            "Delete fight"
                        },
                    )
                    .variant(ButtonVariant::Danger)
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, _, cx| this.confirm_delete(cx))),
                ),
        );
        page = page.child(
            Dialog::new(cx)
                .open(self.delete_open)
                .accessibility_label("Delete fight confirmation")
                .close_on_backdrop_press(false)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !cancel.read(cx).busy)
                .on_open_change(move |open, _, _, cx| {
                    if !open {
                        changed.update(cx, |this, cx| {
                            this.delete_open = false;
                            cx.notify();
                        });
                    }
                })
                .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                .popup(panel),
        );
        let changed = cx.entity();
        let panel=ui::card(t).w(px(if narrow{400.}else{480.})).max_w(gpui::relative(0.94)).bg(t.raised).child(ui::section_header(t,"Export existing report",None))
            .child(ui::muted(t,"Copy this fight’s existing report folder unchanged. Its saved privacy and motion settings are preserved."))
            .child(AuroraInput::new(&self.export_path).label("Export destination folder"))
            .child(div().flex().justify_end().gap(px(8.)).child(button("export-cancel","Cancel").on_click(cx.listener(|this,_,_,cx|{this.export_open=false;cx.notify();})))
                .child(AuroraButton::new("export-confirm","Confirm export").variant(ButtonVariant::Primary).disabled(self.busy).on_click(cx.listener(|this,_,_,cx|this.confirm_export(cx)))));
        page.child(
            Dialog::new(cx)
                .open(self.export_open)
                .accessibility_label("Export report confirmation")
                .on_ok(|_, _, _| false)
                .on_open_change(move |open, _, _, cx| {
                    if !open {
                        changed.update(cx, |this, cx| {
                            this.export_open = false;
                            cx.notify();
                        });
                    }
                })
                .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                .popup(panel),
        )
    }
}
