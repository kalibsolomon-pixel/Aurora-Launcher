//! Storage & Cache Manager view embedded inside Settings -> Storage.

use aurora_launcher_lib::host::Host;
use gpui::{Context, Render, Window, div, prelude::*, px};

use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, AuroraCheckbox, ButtonSize, ButtonVariant},
    theme, ui,
    worlds::format_bytes,
};

enum Reply {
    Overview(u64, api::ApiResult<api::StorageOverview>),
    Pruned(u64, api::ApiResult<api::PruneReport>),
}

pub struct StorageView {
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    tx: async_channel::Sender<Reply>,
    serial: u64,
    overview: Option<api::StorageOverview>,
    prune_cache: bool,
    prune_artwork: bool,
    prune_runtimes: bool,
    loading: bool,
    pruning: bool,
    message: String,
}

impl StorageView {
    pub fn new(host: Option<Host>, rt: tokio::runtime::Handle, cx: &mut Context<Self>) -> Self {
        let (tx, rx) = async_channel::unbounded();
        cx.spawn(async move |weak, cx| {
            while let Ok(reply) = rx.recv().await {
                if weak
                    .update(cx, |this: &mut Self, cx| {
                        this.receive(reply, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let mut view = Self {
            host,
            rt,
            tx,
            serial: 0,
            overview: None,
            prune_cache: true,
            prune_artwork: false,
            prune_runtimes: true,
            loading: false,
            pruning: false,
            message: String::new(),
        };
        view.refresh(cx);
        view
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        self.serial = self.serial.wrapping_add(1);
        let serial = self.serial;
        self.loading = true;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::inspect_storage(host)).await;
            let _ = tx.send(Reply::Overview(serial, res)).await;
        });
        cx.notify();
    }

    fn prune(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.pruning || (!self.prune_cache && !self.prune_artwork && !self.prune_runtimes) {
            return;
        }
        let options = api::PruneOptions {
            prune_unreferenced_cache: self.prune_cache,
            prune_artwork_cache: self.prune_artwork,
            prune_unused_runtimes: self.prune_runtimes,
        };
        self.pruning = true;
        self.message = "Pruning unreferenced storage…".into();
        let serial = self.serial;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::prune_storage(host, options)).await;
            let _ = tx.send(Reply::Pruned(serial, res)).await;
        });
        cx.notify();
    }

    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Overview(serial, res) if serial == self.serial => {
                self.loading = false;
                match res {
                    Ok(overview) => self.overview = Some(overview),
                    Err(e) => self.message = e.message,
                }
            }
            Reply::Pruned(serial, res) if serial == self.serial => {
                self.pruning = false;
                match res {
                    Ok(report) => {
                        self.message = format!(
                            "Reclaimed {} ({} cache objects, {} artwork files, {} runtimes).",
                            format_bytes(report.reclaimed_bytes),
                            report.removed_cache_objects,
                            report.removed_artwork_files,
                            report.removed_runtimes
                        );
                        self.refresh(cx);
                    }
                    Err(e) => self.message = e.message,
                }
            }
            _ => {}
        }
    }
}

impl Render for StorageView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx).clone();
        let mut root = div().flex().flex_col().gap(px(16.)).w_full();

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(ui::section_header(
                &t,
                "Storage & verified cache",
                Some("Inspect disk usage across instances, shared caches, and managed Java runtimes. User worlds, mods, and configs are never pruned.".into()),
            ))
            .child(
                AuroraButton::new("storage-refresh", if self.loading { "Scanning…" } else { "Refresh" })
                    .size(ButtonSize::Small)
                    .disabled(self.loading || self.pruning)
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            );
        root = root.child(header);

        if !self.message.is_empty() {
            root = root.child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(t.radius)
                    .bg(t.surface)
                    .border_1()
                    .border_color(t.border_subtle)
                    .text_size(px(ui::TEXT_CAPTION))
                    .text_color(t.foreground)
                    .child(self.message.clone()),
            );
        }

        let Some(overview) = self.overview.clone() else {
            return root.child(ui::muted(&t, "Calculating managed storage…"));
        };

        let summary_card = ui::card(&t)
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::section_header(
                &t,
                format!("Total managed data: {}", format_bytes(overview.total_bytes)),
                None,
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.))
                    .child(ui::caption(
                        &t,
                        format!("Instances: {}", format_bytes(overview.instances_bytes)),
                    ))
                    .child(ui::caption(
                        &t,
                        format!(
                            "Shared cache: {} ({} unreferenced)",
                            format_bytes(overview.shared_cache_bytes),
                            format_bytes(overview.unreferenced_cache_bytes)
                        ),
                    ))
                    .child(ui::caption(
                        &t,
                        format!(
                            "Artwork cache: {}",
                            format_bytes(overview.artwork_cache_bytes)
                        ),
                    ))
                    .child(ui::caption(
                        &t,
                        format!(
                            "Managed Java: {} ({} unused)",
                            format_bytes(overview.runtimes_bytes),
                            format_bytes(overview.unreferenced_runtimes_bytes)
                        ),
                    )),
            );
        root = root.child(summary_card);

        let prune_card = ui::card(&t)
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ui::section_header(
                &t,
                "Safe cleanup",
                Some("Only unreferenced shared cache files, cached artwork, and unused managed Java runtimes can be pruned.".into()),
            ))
            .child({
                let view = cx.entity();
                AuroraCheckbox::new(
                    "prune-cache",
                    format!(
                        "Prune unreferenced shared cache objects ({})",
                        format_bytes(overview.unreferenced_cache_bytes)
                    ),
                )
                .checked(self.prune_cache)
                .disabled(self.pruning)
                .on_change(move |checked, _, _, cx| {
                    view.update(cx, |this, cx| {
                        this.prune_cache = checked;
                        cx.notify();
                    });
                })
            })
            .child({
                let view = cx.entity();
                AuroraCheckbox::new(
                    "prune-runtimes",
                    format!(
                        "Prune unused managed Java runtimes ({})",
                        format_bytes(overview.unreferenced_runtimes_bytes)
                    ),
                )
                .checked(self.prune_runtimes)
                .disabled(self.pruning)
                .on_change(move |checked, _, _, cx| {
                    view.update(cx, |this, cx| {
                        this.prune_runtimes = checked;
                        cx.notify();
                    });
                })
            })
            .child({
                let view = cx.entity();
                AuroraCheckbox::new(
                    "prune-artwork",
                    format!(
                        "Clear cached project artwork ({})",
                        format_bytes(overview.artwork_cache_bytes)
                    ),
                )
                .checked(self.prune_artwork)
                .disabled(self.pruning)
                .on_change(move |checked, _, _, cx| {
                    view.update(cx, |this, cx| {
                        this.prune_artwork = checked;
                        cx.notify();
                    });
                })
            })
            .child(
                div().flex().justify_end().child(
                    AuroraButton::new(
                        "storage-prune-btn",
                        if self.pruning {
                            "Pruning…"
                        } else {
                            "Prune selected"
                        },
                    )
                    .variant(ButtonVariant::Primary)
                    .disabled(
                        self.pruning
                            || (!self.prune_cache && !self.prune_artwork && !self.prune_runtimes),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.prune(cx))),
                ),
            );
        root = root.child(prune_card);

        if !overview.instances.is_empty() {
            let mut inst_card = ui::card(&t)
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(ui::section_header(&t, "Instances on disk", None));
            for inst in &overview.instances {
                inst_card = inst_card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .py(px(6.))
                        .border_t_1()
                        .border_color(t.border_subtle)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(ui::TEXT_BODY))
                                        .text_color(t.foreground)
                                        .child(inst.display_name.clone()),
                                )
                                .child(ui::caption(
                                    &t,
                                    format!(
                                        "Game {} · Mods {} · Saves {} · Backups {}",
                                        format_bytes(inst.game_bytes),
                                        format_bytes(inst.mods_bytes),
                                        format_bytes(inst.saves_bytes),
                                        format_bytes(inst.backups_bytes),
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(ui::TEXT_BODY))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(t.foreground)
                                .child(format_bytes(inst.total_bytes)),
                        ),
                );
            }
            root = root.child(inst_card);
        }

        if !overview.runtimes.is_empty() {
            let mut rt_card = ui::card(&t)
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(ui::section_header(&t, "Managed Java runtimes", None));
            for rt in &overview.runtimes {
                rt_card = rt_card.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .py(px(6.))
                        .border_t_1()
                        .border_color(t.border_subtle)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(ui::TEXT_BODY))
                                        .text_color(t.foreground)
                                        .child(rt.component.clone()),
                                )
                                .child(ui::caption(
                                    &t,
                                    if rt.referenced_by_instances > 0 {
                                        format!("{} · In use", rt.slot)
                                    } else {
                                        format!("{} · Unreferenced", rt.slot)
                                    },
                                )),
                        )
                        .child(ui::caption(&t, format_bytes(rt.size_bytes))),
                );
            }
            root = root.child(rt_card);
        }

        root
    }
}
