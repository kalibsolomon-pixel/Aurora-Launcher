use super::presentation::ui;
use super::{Pulsar, Reply, tabs};
use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, ButtonSize, ButtonVariant},
    theme::AuroraTheme,
};
use aurora_launcher_lib::pulsar::view::*;
use gpui::{Context, Div, div, prelude::*, px};

impl Pulsar {
    pub(super) fn open_compare(&mut self, cx: &mut Context<Self>) {
        if self.compare_keys.len() != 2 {
            return;
        }
        self.pause();
        self.compare_revision = self.compare_revision.wrapping_add(1);
        self.compare_open = true;
        self.comparison = None;
        self.error = None;
        let request = CompareRequest {
            left: DetailRequest {
                key: self.compare_keys[0].clone(),
                ..Default::default()
            },
            right: DetailRequest {
                key: self.compare_keys[1].clone(),
                ..Default::default()
            },
        };
        if let Some(host) = self.host.clone() {
            let tx = self.tx.clone();
            let rev = self.compare_revision;
            self.runtime.spawn(async move {
                let r = guarded(api::pulsar_compare(host, request)).await;
                let _ = tx.send(Reply::Compare(rev, r)).await;
            });
        }
        cx.notify();
    }
    pub(super) fn compare_body(&self, t: &AuroraTheme, cx: &mut Context<Self>) -> Div {
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(
                div().flex().child(
                    AuroraButton::new("compare-back", "Back to history")
                        .icon("arrow_back")
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.compare_open = false;
                            this.top_tab = 4;
                            cx.notify();
                        })),
                ),
            )
            .child(ui::page_header(
                t,
                "Compare fights",
                Some("Shared measurements, with each recording’s own policy and coverage.".into()),
            ));
        if let Some(error) = &self.error {
            page = page.child(ui::muted(t, error.clone()));
        }
        let Some(data) = &self.comparison else {
            return page.child(ui::muted(
                t,
                if self.error.is_some() {
                    "Comparison unavailable. Return to history and try again."
                } else {
                    "Loading comparison..."
                },
            ));
        };
        page = page.child(
            ui::card(t).child(
                div().flex().flex_wrap().gap(px(20.)).children(
                    [&data.left, &data.right]
                        .into_iter()
                        .enumerate()
                        .map(|(i, f)| {
                            div()
                                .flex_1()
                                .min_w(px(180.))
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(ui::caption(
                                    t,
                                    if i == 0 {
                                        "First fight"
                                    } else {
                                        "Second fight"
                                    },
                                ))
                                .child(ui::section_header(t, f.opponent.clone(), None))
                                .child(ui::caption(t, super::page::utc_time(f.start_epoch_millis)))
                                .child(ui::caption(
                                    t,
                                    format!(
                                        "{:.1} s · {}",
                                        f.duration_nanos as f64 / 1e9,
                                        f.provenance.instance_name
                                    ),
                                ))
                                .child(div().flex().child(ui::badge(
                                    t,
                                    f.result_label.clone(),
                                    super::presentation::outcome(f),
                                )))
                        }),
                ),
            ),
        );
        page=page.child(ui::caption(t,"Differences are second − first. Percentage point changes describe rates; a positive change is not always better."));
        let mut rows = ui::list_card(t).min_w(px(700.)).w_full().child(
            ui::row(t, false)
                .child(ui::caption(t, "Measurement").flex_1().min_w(px(0.)))
                .child(ui::caption(t, "First fight").flex_1().min_w(px(0.)))
                .child(ui::caption(t, "Second fight").flex_1().min_w(px(0.)))
                .child(ui::caption(t, "Second − first").w(px(130.))),
        );
        for (i, m) in data.metrics.iter().enumerate() {
            rows = rows.child(
                ui::row(t, i + 1 == data.metrics.len())
                    .child(
                        ui::muted(t, m.left.definition.label.clone())
                            .flex_1()
                            .min_w(px(0.)),
                    )
                    .child(
                        ui::muted(t, tabs::metric_value(&m.left))
                            .flex_1()
                            .min_w(px(0.)),
                    )
                    .child(
                        ui::muted(t, tabs::metric_value(&m.right))
                            .flex_1()
                            .min_w(px(0.)),
                    )
                    .child(
                        ui::caption(t, super::presentation::comparison_delta(&m.left, &m.right))
                            .w(px(130.)),
                    ),
            );
        }
        page = page.child(
            div()
                .id("pulsar-compare-table")
                .w_full()
                .min_w(px(0.))
                .overflow_x_scroll()
                .child(rows),
        );
        for n in &data.notices {
            page = page.child(ui::caption(t, n.clone()));
        }
        page
    }
}
