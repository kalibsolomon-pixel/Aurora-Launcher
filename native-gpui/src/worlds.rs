//! Per-instance Worlds management tab: singleplayer world list, Quick Play,
//! explicit local ZIP backups/restore, and world-scoped Modrinth datapacks.

use aurora_launcher_lib::host::Host;
use gpui::{Context, Entity, EventEmitter, Render, Subscription, Window, div, prelude::*, px};
use gpui_kit::base::{
    Dialog,
    input::{InputEvent, InputState},
};

use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, AuroraInput, AuroraToggle, ButtonSize, ButtonVariant},
    theme, ui,
};

pub enum WorldsEvent {
    QuickPlayWorld {
        instance_id: String,
        world_dir: String,
    },
    Changed,
}

impl EventEmitter<WorldsEvent> for Worlds {}

enum Reply {
    Worlds(u64, api::ApiResult<Vec<api::InstanceWorld>>),
    Backups(u64, String, api::ApiResult<Vec<api::WorldBackupSummary>>),
    Datapacks(u64, String, api::ApiResult<Vec<api::DatapackEntry>>),
    BackupCreated(u64, String, api::ApiResult<api::WorldBackupSummary>),
    BackupAction(u64, String, &'static str, api::ApiResult<()>),
    DatapackAction(u64, String, &'static str, api::ApiResult<()>),
    DatapackSearch(u64, api::ApiResult<api::SearchPage>),
    DatapackInstalled(u64, String, api::ApiResult<api::DatapackEntry>),
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

pub struct Worlds {
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    tx: async_channel::Sender<Reply>,
    instance_id: Option<String>,
    quick_play_supported: bool,
    generation: u64,
    worlds: Vec<api::InstanceWorld>,
    selected_world: Option<String>,
    subtab: u8, // 0 = Backups, 1 = Datapacks
    backups: Vec<api::WorldBackupSummary>,
    datapacks: Vec<api::DatapackEntry>,
    backup_label: Entity<InputState>,
    dp_search: Entity<InputState>,
    dp_results: Vec<api::ProjectSummary>,
    dp_searching: bool,
    restore_confirm: Option<api::WorldBackupSummary>,
    loading: bool,
    busy: bool,
    status: String,
    _subs: Vec<Subscription>,
}

impl Worlds {
    pub fn new(
        host: Option<Host>,
        rt: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded();
        let backup_label = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Optional backup label (e.g. Before boss)")
        });
        let dp_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search Modrinth datapacks…"));
        let dp_sub = cx.subscribe(&dp_search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.search_datapacks(cx);
            }
        });

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

        Self {
            host,
            rt,
            tx,
            instance_id: None,
            quick_play_supported: false,
            generation: 0,
            worlds: Vec::new(),
            selected_world: None,
            subtab: 0,
            backups: Vec::new(),
            datapacks: Vec::new(),
            backup_label,
            dp_search,
            dp_results: Vec::new(),
            dp_searching: false,
            restore_confirm: None,
            loading: false,
            busy: false,
            status: String::new(),
            _subs: vec![dp_sub],
        }
    }

    pub fn set_instance(
        &mut self,
        instance_id: Option<String>,
        quick_play_supported: bool,
        cx: &mut Context<Self>,
    ) {
        if self.instance_id == instance_id && self.quick_play_supported == quick_play_supported {
            return;
        }
        self.instance_id = instance_id;
        self.quick_play_supported = quick_play_supported;
        self.selected_world = None;
        self.worlds.clear();
        self.backups.clear();
        self.datapacks.clear();
        self.dp_results.clear();
        self.restore_confirm = None;
        self.status.clear();
        self.refresh(cx);
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let (Some(host), Some(id)) = (self.host.clone(), self.instance_id.clone()) else {
            return;
        };
        self.generation = self.generation.wrapping_add(1);
        let rev = self.generation;
        self.loading = true;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::instance_worlds(host, id)).await;
            let _ = tx.send(Reply::Worlds(rev, res)).await;
        });
        cx.notify();
    }

    fn load_world_details(&mut self, world_dir: String, cx: &mut Context<Self>) {
        let (Some(host), Some(id)) = (self.host.clone(), self.instance_id.clone()) else {
            return;
        };
        let rev = self.generation;
        let tx = self.tx.clone();
        let w1 = world_dir.clone();
        let h1 = host.clone();
        let i1 = id.clone();
        self.rt.spawn(async move {
            let res = guarded(api::world_backups(h1, i1, w1.clone())).await;
            let _ = tx.send(Reply::Backups(rev, w1, res)).await;
        });
        let tx2 = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::world_datapacks(host, id, world_dir.clone())).await;
            let _ = tx2.send(Reply::Datapacks(rev, world_dir, res)).await;
        });
        cx.notify();
    }

    fn create_backup(&mut self, cx: &mut Context<Self>) {
        let (Some(host), Some(id), Some(world)) = (
            self.host.clone(),
            self.instance_id.clone(),
            self.selected_world.clone(),
        ) else {
            return;
        };
        if self.busy {
            return;
        }
        let label = self.backup_label.read(cx).value().trim().to_owned();
        let label = (!label.is_empty()).then_some(label);
        self.busy = true;
        self.status = "Creating world backup…".into();
        let rev = self.generation;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::create_world_backup(host, id, world.clone(), label)).await;
            let _ = tx.send(Reply::BackupCreated(rev, world, res)).await;
        });
        cx.notify();
    }

    fn restore_backup(&mut self, backup_id: String, cx: &mut Context<Self>) {
        let (Some(host), Some(id), Some(world)) = (
            self.host.clone(),
            self.instance_id.clone(),
            self.selected_world.clone(),
        ) else {
            return;
        };
        self.restore_confirm = None;
        self.busy = true;
        self.status = "Restoring world backup…".into();
        let rev = self.generation;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::restore_world_backup(
                host,
                id,
                world.clone(),
                backup_id,
            ))
            .await;
            let _ = tx
                .send(Reply::BackupAction(
                    rev,
                    world,
                    "World restored (a pre-restore safety backup was saved).",
                    res,
                ))
                .await;
        });
        cx.notify();
    }

    fn delete_backup(&mut self, backup_id: String, cx: &mut Context<Self>) {
        let (Some(host), Some(id), Some(world)) = (
            self.host.clone(),
            self.instance_id.clone(),
            self.selected_world.clone(),
        ) else {
            return;
        };
        self.busy = true;
        let rev = self.generation;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::delete_world_backup(host, id, world.clone(), backup_id)).await;
            let _ = tx
                .send(Reply::BackupAction(rev, world, "Backup deleted.", res))
                .await;
        });
        cx.notify();
    }

    fn toggle_datapack(&mut self, file_name: String, enabled: bool, cx: &mut Context<Self>) {
        let (Some(host), Some(id), Some(world)) = (
            self.host.clone(),
            self.instance_id.clone(),
            self.selected_world.clone(),
        ) else {
            return;
        };
        self.busy = true;
        let rev = self.generation;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::set_world_datapack_enabled(
                host,
                id,
                world.clone(),
                file_name,
                enabled,
            ))
            .await;
            let _ = tx
                .send(Reply::DatapackAction(rev, world, "Datapack updated.", res))
                .await;
        });
        cx.notify();
    }

    fn remove_datapack(&mut self, file_name: String, cx: &mut Context<Self>) {
        let (Some(host), Some(id), Some(world)) = (
            self.host.clone(),
            self.instance_id.clone(),
            self.selected_world.clone(),
        ) else {
            return;
        };
        self.busy = true;
        let rev = self.generation;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::remove_world_datapack(
                host,
                id,
                world.clone(),
                file_name,
            ))
            .await;
            let _ = tx
                .send(Reply::DatapackAction(rev, world, "Datapack removed.", res))
                .await;
        });
        cx.notify();
    }

    fn search_datapacks(&mut self, cx: &mut Context<Self>) {
        let (Some(host), Some(id)) = (self.host.clone(), self.instance_id.clone()) else {
            return;
        };
        let query = self.dp_search.read(cx).value().trim().to_owned();
        self.dp_searching = true;
        let rev = self.generation;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::browse_datapacks(host, id, query, 0)).await;
            let _ = tx.send(Reply::DatapackSearch(rev, res)).await;
        });
        cx.notify();
    }

    fn install_datapack(&mut self, project_id: String, cx: &mut Context<Self>) {
        let (Some(host), Some(id), Some(world)) = (
            self.host.clone(),
            self.instance_id.clone(),
            self.selected_world.clone(),
        ) else {
            return;
        };
        self.busy = true;
        self.status = "Downloading and verifying datapack…".into();
        let rev = self.generation;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let res = guarded(api::install_world_datapack(
                host,
                id,
                world.clone(),
                project_id,
            ))
            .await;
            let _ = tx.send(Reply::DatapackInstalled(rev, world, res)).await;
        });
        cx.notify();
    }

    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Worlds(rev, result) if rev == self.generation => {
                self.loading = false;
                match result {
                    Ok(worlds) => {
                        self.worlds = worlds;
                        let keep = self
                            .selected_world
                            .as_ref()
                            .filter(|w| self.worlds.iter().any(|item| &item.directory == *w))
                            .cloned();
                        let next =
                            keep.or_else(|| self.worlds.first().map(|w| w.directory.clone()));
                        self.selected_world = next.clone();
                        if let Some(dir) = next {
                            self.load_world_details(dir, cx);
                        }
                    }
                    Err(e) => self.status = e.message,
                }
            }
            Reply::Backups(rev, world, result)
                if rev == self.generation
                    && self.selected_world.as_deref() == Some(world.as_str()) =>
            {
                if let Ok(backups) = result {
                    self.backups = backups;
                }
            }
            Reply::Datapacks(rev, world, result)
                if rev == self.generation
                    && self.selected_world.as_deref() == Some(world.as_str()) =>
            {
                if let Ok(dps) = result {
                    self.datapacks = dps;
                }
            }
            Reply::BackupCreated(rev, _world, result) => {
                self.busy = false;
                if rev == self.generation {
                    match result {
                        Ok(created) => {
                            self.status =
                                format!("Created backup ({}).", format_bytes(created.size_bytes));
                            self.refresh(cx);
                            cx.emit(WorldsEvent::Changed);
                        }
                        Err(e) => self.status = e.message,
                    }
                }
            }
            Reply::BackupAction(rev, _world, ok_msg, result) => {
                self.busy = false;
                if rev == self.generation {
                    match result {
                        Ok(()) => {
                            self.status = ok_msg.into();
                            self.refresh(cx);
                            cx.emit(WorldsEvent::Changed);
                        }
                        Err(e) => self.status = e.message,
                    }
                }
            }
            Reply::DatapackAction(rev, _world, ok_msg, result) => {
                self.busy = false;
                if rev == self.generation {
                    match result {
                        Ok(()) => {
                            self.status = ok_msg.into();
                            self.refresh(cx);
                        }
                        Err(e) => self.status = e.message,
                    }
                }
            }
            Reply::DatapackSearch(rev, result) => {
                self.dp_searching = false;
                if rev == self.generation {
                    match result {
                        Ok(page) => self.dp_results = page.hits.into_iter().take(8).collect(),
                        Err(e) => self.status = e.message,
                    }
                }
            }
            Reply::DatapackInstalled(rev, _world, result) => {
                self.busy = false;
                if rev == self.generation {
                    match result {
                        Ok(entry) => {
                            self.status = format!("Installed datapack {}.", entry.display_name);
                            self.refresh(cx);
                        }
                        Err(e) => self.status = e.message,
                    }
                }
            }
            _ => {}
        }
    }
}

impl Render for Worlds {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx).clone();
        let mut root = div().flex().flex_col().gap(px(16.)).w_full();

        if !self.status.is_empty() {
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
                    .child(self.status.clone()),
            );
        }

        if self.loading && self.worlds.is_empty() {
            return root.child(ui::muted(&t, "Scanning singleplayer worlds…"));
        }

        if self.worlds.is_empty() {
            return root.child(
                ui::card(&t)
                    .child(ui::section_header(
                        &t,
                        "No singleplayer worlds yet",
                        Some("Create a world in Minecraft and it will appear here for Quick Play, backups, and datapacks.".into()),
                    )),
            );
        }

        let mut list = div().flex().flex_col().gap(px(8.)).w(px(300.)).flex_none();
        for (idx, world) in self.worlds.iter().enumerate() {
            let selected = self.selected_world.as_deref() == Some(world.directory.as_str());
            let dir_click = world.directory.clone();
            let dir_play = world.directory.clone();
            let instance_id = self.instance_id.clone().unwrap_or_default();
            let meta = format!(
                "{} · {} · {} backups · {} datapacks",
                world.game_mode.as_deref().unwrap_or("World"),
                format_bytes(world.size_bytes),
                world.backup_count,
                world.datapack_count
            );
            let mut row = div()
                .id(("world-row", idx))
                .p(px(12.))
                .rounded(t.radius)
                .border_1()
                .border_color(if selected { t.accent } else { t.border_subtle })
                .bg(if selected { t.accent_soft } else { t.surface })
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected_world = Some(dir_click.clone());
                    this.load_world_details(dir_click.clone(), cx);
                }))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(px(ui::TEXT_BODY))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(t.foreground)
                                .child(world.display_name.clone()),
                        )
                        .when(self.quick_play_supported, |d| {
                            d.child(
                                AuroraButton::new(("world-play", idx), "Play")
                                    .variant(ButtonVariant::Soft)
                                    .size(ButtonSize::Small)
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.emit(WorldsEvent::QuickPlayWorld {
                                            instance_id: instance_id.clone(),
                                            world_dir: dir_play.clone(),
                                        });
                                    })),
                            )
                        }),
                )
                .child(ui::caption(&t, meta));
            if let Some(ver) = &world.version_name {
                row = row.child(ui::caption(&t, format!("MC {ver}")));
            }
            list = list.child(row);
        }

        let mut detail = ui::card(&t)
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(12.));
        if let Some(world) = self
            .selected_world
            .as_ref()
            .and_then(|d| self.worlds.iter().find(|w| &w.directory == d))
        {
            let mut tabs = ui::segmented_tabs(&t);
            for (idx, label) in [(0u8, "Backups"), (1u8, "Datapacks")] {
                tabs = tabs.child(
                    AuroraButton::new(("world-subtab", idx as usize), label)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .selected(self.subtab == idx)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.subtab = idx;
                            cx.notify();
                        })),
                );
            }
            detail = detail.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(ui::section_header(
                        &t,
                        world.display_name.clone(),
                        Some(
                            format!(
                                "Folder: {} · {}",
                                world.directory,
                                format_bytes(world.size_bytes)
                            )
                            .into(),
                        ),
                    ))
                    .child(tabs),
            );

            if self.subtab == 0 {
                // Backups panel
                detail = detail.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .child(AuroraInput::new(&self.backup_label).label("Backup label")),
                        )
                        .child(
                            AuroraButton::new("world-create-backup", "Create backup")
                                .variant(ButtonVariant::Soft)
                                .disabled(self.busy)
                                .on_click(cx.listener(|this, _, _, cx| this.create_backup(cx))),
                        ),
                );
                if self.backups.is_empty() {
                    detail = detail.child(ui::muted(&t, "No backups yet."));
                } else {
                    for (idx, bk) in self.backups.iter().enumerate() {
                        let bk_restore = bk.clone();
                        let bk_del = bk.id.clone();
                        let title = bk
                            .label
                            .clone()
                            .unwrap_or_else(|| format!("Snapshot {}", bk.id));
                        detail = detail.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .py(px(8.))
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
                                                .child(title),
                                        )
                                        .child(ui::caption(&t, format_bytes(bk.size_bytes))),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(8.))
                                        .child(
                                            AuroraButton::new(("bk-restore", idx), "Restore")
                                                .size(ButtonSize::Small)
                                                .disabled(self.busy)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.restore_confirm = Some(bk_restore.clone());
                                                    cx.notify();
                                                })),
                                        )
                                        .child(
                                            AuroraButton::new(("bk-del", idx), "Delete")
                                                .variant(ButtonVariant::Ghost)
                                                .size(ButtonSize::Small)
                                                .disabled(self.busy)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.delete_backup(bk_del.clone(), cx);
                                                })),
                                        ),
                                ),
                        );
                    }
                }
            } else {
                // Datapacks panel
                if self.datapacks.is_empty() {
                    detail =
                        detail.child(ui::muted(&t, "No ZIP datapacks installed in this world."));
                } else {
                    for (idx, dp) in self.datapacks.iter().enumerate() {
                        let f_toggle = dp.file_name.clone();
                        let next_enabled = !dp.enabled;
                        let f_remove = dp.file_name.clone();
                        detail = detail.child(
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
                                                .child(dp.display_name.clone()),
                                        )
                                        .child(ui::caption(
                                            &t,
                                            format!(
                                                "{} · {}",
                                                dp.file_name,
                                                format_bytes(dp.size_bytes)
                                            ),
                                        )),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child({
                                            let view = cx.entity();
                                            AuroraToggle::new(("dp-toggle", idx))
                                                .checked(dp.enabled)
                                                .disabled(self.busy)
                                                .on_change(move |_, _, _, cx| {
                                                    let f = f_toggle.clone();
                                                    view.update(cx, |this, cx| {
                                                        this.toggle_datapack(f, next_enabled, cx);
                                                    });
                                                })
                                        })
                                        .child(
                                            AuroraButton::new(("dp-rm", idx), "Remove")
                                                .variant(ButtonVariant::Ghost)
                                                .size(ButtonSize::Small)
                                                .disabled(self.busy)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.remove_datapack(f_remove.clone(), cx);
                                                })),
                                        ),
                                ),
                        );
                    }
                }
                detail = detail.child(
                    div()
                        .pt(px(12.))
                        .border_t_1()
                        .border_color(t.border_subtle)
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(ui::section_header(&t, "Add datapack from Modrinth", None))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(div().flex_1().child(
                                    AuroraInput::new(&self.dp_search).label("Search datapacks"),
                                ))
                                .child(
                                    AuroraButton::new(
                                        "dp-search-go",
                                        if self.dp_searching {
                                            "Searching…"
                                        } else {
                                            "Search"
                                        },
                                    )
                                    .disabled(self.dp_searching)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.search_datapacks(cx)),
                                    ),
                                ),
                        ),
                );
                for (idx, hit) in self.dp_results.iter().enumerate() {
                    let pid = hit.project_id.clone();
                    detail = detail.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .py(px(6.))
                            .border_t_1()
                            .border_color(t.border_subtle)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(ui::TEXT_BODY))
                                            .text_color(t.foreground)
                                            .child(hit.title.clone()),
                                    )
                                    .child(ui::caption(&t, hit.summary.clone())),
                            )
                            .child(
                                AuroraButton::new(("dp-install", idx), "Install")
                                    .size(ButtonSize::Small)
                                    .disabled(self.busy)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.install_datapack(pid.clone(), cx);
                                    })),
                            ),
                    );
                }
            }
        }

        let split = div()
            .flex()
            .items_start()
            .gap(px(16.))
            .w_full()
            .child(list)
            .child(detail);

        let restore_open = self.restore_confirm.is_some();
        let restore_id = self
            .restore_confirm
            .as_ref()
            .map(|b| b.id.clone())
            .unwrap_or_default();
        let view = cx.entity();
        let cancel_view = view.clone();
        let ok_view = view.clone();
        let modal = ui::card(&t)
            .w(px(420.))
            .bg(t.raised)
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(ui::section_header(&t, "Restore world backup?", None))
            .child(ui::muted(
                &t,
                "Aurora will save a pre-restore safety backup of the current world first, then replace it atomically with the selected snapshot.",
            ))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(AuroraButton::new("restore-cancel", "Cancel").on_click(move |_, _, cx| {
                        cancel_view.update(cx, |s, cx| {
                            s.restore_confirm = None;
                            cx.notify();
                        });
                    }))
                    .child(
                        AuroraButton::new("restore-confirm", "Restore backup")
                            .variant(ButtonVariant::Primary)
                            .on_click(move |_, _, cx| {
                                ok_view.update(cx, |s, cx| s.restore_backup(restore_id.clone(), cx));
                            }),
                    ),
            );

        root.child(split).child(
            Dialog::new(cx)
                .open(restore_open)
                .accessibility_label("Restore world backup?")
                .on_open_change(move |open, _, _, cx| {
                    if !open {
                        view.update(cx, |s, cx| {
                            s.restore_confirm = None;
                            cx.notify();
                        });
                    }
                })
                .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                .popup(modal),
        )
    }
}
