//! One event-driven, virtualized game-output window per instance.
use crate::{
    api,
    backend::guarded,
    chrome,
    controls::{AuroraButton, AuroraInput, ButtonVariant},
    theme, ui,
};
use aurora_launcher_lib::host::Host;
use gpui::{
    Context, Entity, Render, Subscription, UniformListScrollHandle, Window, div, prelude::*, px,
    uniform_list,
};
use gpui_kit::base::{
    Dialog,
    input::{InputEvent, InputState},
};

pub struct GameLog {
    instance: String,
    name: String,
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    log: api::LogSnapshot,
    process: Option<api::LaunchProcess>,
    search: Entity<InputState>,
    filter: u8,
    visible: Vec<usize>,
    scroll: UniformListScrollHandle,
    follow: bool,
    upload_confirm: bool,
    busy: bool,
    upload: Option<api::UploadResult>,
    diagnosis: Option<aurora_launcher_lib::launch::diagnostics::LaunchFailureDiagnostic>,
    message: String,
    folders: (bool, bool),
    _search: Subscription,
    // Closing the window cancels its observer; the game and ring survive.
    observer: tokio::task::JoinHandle<()>,
}

impl Drop for GameLog {
    fn drop(&mut self) {
        self.observer.abort();
    }
}

pub fn matches(text: &str, level: &str, query: &str, filter: u8) -> bool {
    (filter == 0
        || (filter == 1 && matches!(level, "warn" | "error" | "fatal" | "trace"))
        || (filter == 2 && matches!(level, "error" | "fatal" | "trace")))
        && text.to_lowercase().contains(&query.to_lowercase())
}

fn category_label(category: &str) -> &'static str {
    match category {
        "modConfigurationConflict" => "Mod configuration conflict",
        "javaCompatibility" => "Java compatibility",
        "fabricDependencyResolution" => "Fabric dependencies",
        "forgeDependencyResolution" => "Forge / NeoForge mod loading",
        "corruptConfig" => "Corrupt configuration file",
        "mixinInitialization" => "Mixin initialization",
        "outOfMemory" => "Out of memory",
        "nativeCrash" => "Native library or driver crash",
        "graphicsDriver" => "Graphics driver",
        "diskFull" => "Disk space",
        "noKnownProblem" => "No known problem detected",
        _ => "Game output analysis",
    }
}

impl GameLog {
    pub fn new(
        instance: String,
        name: String,
        host: Option<Host>,
        rt: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_window_title(&format!("{} - Game log", name));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search game output"));
        let search_sub = cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.refilter(cx);
                cx.notify();
            }
        });
        let (tx, rx) = async_channel::bounded(1);
        let id = instance.clone();
        let read_host = host.clone();
        let observer = rt.spawn(async move {
            let Ok(mut changes) = api::subscribe_game_log(id.clone()) else {
                return;
            };
            loop {
                changes.borrow_and_update();
                let log = guarded(api::game_log(id.clone())).await;
                let process = guarded(api::launch_state(id.clone())).await.ok();
                let folders = if let Some(host) = read_host.clone() {
                    guarded(api::game_log_folders(host, id.clone()))
                        .await
                        .unwrap_or_default()
                } else {
                    (false, false)
                };
                if tx.send((log, process, folders)).await.is_err() {
                    break;
                }
                if changes.changed().await.is_err() {
                    break;
                }
                // Coalesce bursts only after output arrives; idle has no timer.
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        });
        cx.spawn(async move |weak, cx| {
            while let Ok((log, process, folders)) = rx.recv().await {
                if weak
                    .update(cx, |this: &mut Self, cx| {
                        // Observe the actual viewport before replacing the rows.
                        if this.follow && this.scroll.is_scrolled_to_end() == Some(false) {
                            this.follow = false;
                        }
                        if let Ok(log) = log {
                            this.log = log;
                        }
                        this.process = if crate::showcase()
                            && std::env::var("AURORA_SHOWCASE_GAME_STATE")
                                .is_ok_and(|s| s.starts_with("logs-"))
                        {
                            Some(showcase_process(&this.instance, "running", false))
                        } else {
                            process
                        };
                        this.folders = folders;
                        this.refilter(cx);
                        if this.follow {
                            this.scroll.scroll_to_bottom();
                        }
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
            instance,
            name,
            host,
            rt,
            log: Default::default(),
            process: None,
            search,
            filter: 0,
            visible: vec![],
            scroll: UniformListScrollHandle::new(),
            follow: true,
            upload_confirm: false,
            busy: false,
            upload: None,
            diagnosis: None,
            message: String::new(),
            folders: (false, false),
            _search: search_sub,
            observer,
        };
        if crate::showcase() {
            match std::env::var("AURORA_SHOWCASE_GAME_STATE")
                .unwrap_or_default()
                .as_str()
            {
                "logs-filtered" => view.filter = 1,
                "logs-upload" => view.upload_confirm = true,
                "logs-diagnose" => {
                    view.diagnosis = Some(aurora_launcher_lib::launch::diagnostics::diagnose(
                        "java.lang.OutOfMemoryError: Java heap space",
                        "",
                        None,
                    ))
                }
                _ => {}
            }
        }
        view
    }
    fn refilter(&mut self, cx: &gpui::App) {
        let query = self.search.read(cx).value().to_string();
        self.visible = self
            .log
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| matches(&l.text, &l.level, &query, self.filter))
            .map(|(i, _)| i)
            .collect();
    }
    fn request<T: Send + 'static>(
        &self,
        work: impl std::future::Future<Output = api::ApiResult<T>> + Send + 'static,
        cx: &mut Context<Self>,
        deliver: impl FnOnce(&mut Self, api::ApiResult<T>, &mut Context<Self>) + 'static,
    ) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let _ = tx.send(guarded(work).await);
        });
        cx.spawn(async move |weak, cx| {
            if let Ok(result) = rx.await {
                let _ = weak.update(cx, |this, cx| {
                    deliver(this, result, cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }
    fn upload(&mut self, cx: &mut Context<Self>) {
        self.upload_confirm = false;
        self.busy = true;
        self.message.clear();
        self.request(
            api::upload_game_log(self.instance.clone(), true),
            cx,
            |this, result, _| {
                this.busy = false;
                match result {
                    Ok(upload) => {
                        this.message = if upload.truncated {
                            "Uploaded. Older output was truncated.".into()
                        } else {
                            "Uploaded redacted log.".into()
                        };
                        this.upload = Some(upload);
                    }
                    Err(e) => this.message = e.message,
                }
            },
        );
    }
    fn diagnose(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        self.busy = true;
        self.request(
            api::diagnose_game_log(host, self.instance.clone()),
            cx,
            |this, result, _| {
                this.busy = false;
                match result {
                    Ok(d) => this.diagnosis = Some(d),
                    Err(e) => this.message = e.message,
                }
            },
        );
    }
    fn folder(&self, crash: bool, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        self.request(
            api::open_game_log_folder(host, self.instance.clone(), crash),
            cx,
            |this, r, _| {
                if let Err(e) = r {
                    this.message = e.message;
                }
            },
        );
    }
}

impl Render for GameLog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::theme(cx).clone();
        let status = self
            .process
            .as_ref()
            .map(|p| match p.status.as_str() {
                "running" => "Running".into(),
                "starting" => "Starting".into(),
                "failed" => "Crashed".into(),
                "exited" => format!(
                    "Exited (code {})",
                    p.exit_code.map_or("unavailable".into(), |c| c.to_string())
                ),
                "stopped" if p.message.is_some() => "Stopped by you".into(),
                _ => "No active session".into(),
            })
            .unwrap_or_else(|| "Waiting for game output".into());
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.background)
            .text_color(t.foreground)
            .text_size(px(ui::TEXT_BODY));
        if chrome::custom() {
            root = root.child(chrome::title_bar(window, cx, &t));
        }
        let actions = div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .child(
                AuroraButton::new("log-copy", "Copy all")
                    .disabled(self.log.lines.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            this.log
                                .lines
                                .iter()
                                .map(|l| l.text.as_str())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        ));
                        this.message = "Copied all retained output.".into();
                        cx.notify();
                    })),
            )
            .child(
                AuroraButton::new(
                    "log-upload",
                    if self.busy {
                        "Working…"
                    } else {
                        "Upload to mclo.gs"
                    },
                )
                .disabled(self.busy || self.log.lines.is_empty())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.upload_confirm = true;
                    cx.notify();
                })),
            )
            .child(
                AuroraButton::new("log-diagnose", "Diagnose")
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, _, cx| this.diagnose(cx))),
            )
            .child(
                AuroraButton::new("log-folder", "Open logs folder")
                    .disabled(!self.folders.0)
                    .on_click(cx.listener(|this, _, _, cx| this.folder(false, cx))),
            )
            .child(
                AuroraButton::new("crash-folder", "Open crash reports")
                    .disabled(!self.folders.1)
                    .on_click(cx.listener(|this, _, _, cx| this.folder(true, cx))),
            );
        let mut filters = div().flex().items_center().gap(px(8.)).child(
            div()
                .flex_1()
                .child(AuroraInput::new(&self.search).label("Search game output")),
        );
        for (index, label) in ["All", "Warnings", "Errors"].into_iter().enumerate() {
            filters = filters.child(
                AuroraButton::new(("log-filter", index), label)
                    .variant(ButtonVariant::Ghost)
                    .selected(self.filter == index as u8)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.filter = index as u8;
                        this.refilter(cx);
                        if this.follow {
                            this.scroll.scroll_to_bottom();
                        }
                        cx.notify();
                    })),
            );
        }
        let mut body = div()
            .flex_1()
            .min_h_0()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(ui::page_header(
                &t,
                format!("{} · Game log", self.name),
                Some(status.into()),
            ))
            .child(actions)
            .child(filters);
        if let Some(d) = self.diagnosis.clone() {
            let mut diag_card = ui::card(&t)
                .child(ui::section_header(
                    &t,
                    "Local diagnosis",
                    Some(category_label(&d.category).into()),
                ))
                .child(ui::muted(&t, d.message.clone()))
                .when(!d.implicated_mod_ids.is_empty(), |card| {
                    card.child(ui::caption(
                        &t,
                        format!("Implicated mods: {}", d.implicated_mod_ids.join(", ")),
                    ))
                });
            let mut fix_row = div().flex().flex_wrap().gap(px(8.)).pt(px(4.));
            let mut has_fix = false;
            if d.suggested_action.as_deref() == Some("increaseMemory") {
                has_fix = true;
                fix_row = fix_row.child(
                    AuroraButton::new("diag-fix-ram", "Increase RAM by +1 GiB")
                        .variant(ButtonVariant::Primary)
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let Some(host) = this.host.clone() else {
                                return;
                            };
                            this.busy = true;
                            this.request(
                                api::increase_instance_memory_quick(
                                    host,
                                    this.instance.clone(),
                                    1024,
                                ),
                                cx,
                                |this, res, _| {
                                    this.busy = false;
                                    match res {
                                        Ok(inst) => {
                                            this.message = format!(
                                                "Updated instance memory to {} MiB.",
                                                inst.configuration.memory_mib
                                            );
                                        }
                                        Err(e) => this.message = e.message,
                                    }
                                },
                            );
                        })),
                );
            }
            if d.suggested_action.as_deref() == Some("openConfigFolder") {
                has_fix = true;
                fix_row = fix_row.child(
                    AuroraButton::new("diag-fix-cfg", "Open config folder")
                        .variant(ButtonVariant::Primary)
                        .disabled(self.busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let Some(host) = this.host.clone() else {
                                return;
                            };
                            this.request(
                                api::open_instance_config_folder(host, this.instance.clone()),
                                cx,
                                |this, res, _| {
                                    if let Err(e) = res {
                                        this.message = e.message;
                                    }
                                },
                            );
                        })),
                );
            }
            if d.suggested_action.as_deref() == Some("disableMod")
                && let Some(first_mod) = d.implicated_mod_ids.first().cloned()
            {
                has_fix = true;
                let label = format!("Disable {first_mod}");
                fix_row = fix_row.child(
                    AuroraButton::new("diag-fix-mod", label)
                        .variant(ButtonVariant::Primary)
                        .disabled(self.busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let Some(host) = this.host.clone() else {
                                return;
                            };
                            let target = first_mod.clone();
                            let inst_id = this.instance.clone();
                            this.busy = true;
                            this.request(
                                async move {
                                    let inv = api::instance_mods(host.clone(), inst_id.clone()).await?;
                                    if let Some(entry) = inv.entries.iter().find(|e| {
                                        e.can_toggle
                                            && (e.file_name.to_lowercase().contains(&target.to_lowercase())
                                                || e.display_name.to_lowercase().contains(&target.to_lowercase()))
                                    }) {
                                        api::set_mod_enabled(host, inst_id, entry.entry_id.clone(), false).await?;
                                        Ok(format!("Disabled {}.", entry.display_name))
                                    } else {
                                        Err(api::ApiError::new(
                                            "mod_not_found",
                                            format!("Could not match '{target}' to a toggleable mod file."),
                                        ))
                                    }
                                },
                                cx,
                                |this, res, _| {
                                    this.busy = false;
                                    match res {
                                        Ok(msg) => this.message = msg,
                                        Err(e) => this.message = e.message,
                                    }
                                },
                            );
                        })),
                );
            }
            if has_fix {
                diag_card = diag_card.child(fix_row);
            }
            body = body.child(diag_card);
        }
        if let Some(upload) = self.upload.clone() {
            let copy = upload.url.clone();
            let open = upload.url.clone();
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::muted(&t, upload.url))
                    .child(AuroraButton::new("log-copy-link", "Copy link").on_click(
                        move |_, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone()))
                        },
                    ))
                    .child(
                        AuroraButton::new("log-open-link", "Open").on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.request(
                                    api::open_game_log_link(open.clone()),
                                    cx,
                                    |this, r, _| {
                                        if let Err(e) = r {
                                            this.message = e.message;
                                        }
                                    },
                                )
                            },
                        )),
                    ),
            );
        }
        let rows = uniform_list(
            "game-log-lines",
            self.visible.len(),
            cx.processor(|this: &mut Self, range: std::ops::Range<usize>, _, cx| {
                let t = theme::theme(cx).clone();
                range
                    .map(|index| {
                        let line = &this.log.lines[this.visible[index]];
                        let color = match line.level.as_str() {
                            "error" | "fatal" => t.danger,
                            "warn" => t.warning,
                            "trace" => t.muted_foreground,
                            _ => t.foreground,
                        };
                        // Full output remains available through Copy all.
                        div()
                            .h(px(22.))
                            .px(px(8.))
                            .text_size(px(ui::TEXT_CAPTION))
                            .font_family("Consolas")
                            .text_color(color)
                            .whitespace_nowrap()
                            .child(line.text.clone())
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(&self.scroll)
        .size_full();
        body = body.child(
            div()
                .relative()
                .flex_1()
                .min_h(px(100.))
                .overflow_hidden()
                .bg(t.sunken)
                .rounded(t.radius)
                .on_scroll(cx.listener(|this, _, _, cx| {
                    this.follow = false;
                    cx.notify();
                }))
                .child(rows)
                .child(ui::scrollbar(
                    &t,
                    "log-scrollbar",
                    &self.scroll.0.borrow().base_handle,
                )),
        );
        body = body.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(ui::caption(
                    &t,
                    format!(
                        "{} of {} lines · {} older lines omitted",
                        self.visible.len(),
                        self.log.lines.len(),
                        self.log.discarded_lines
                    ),
                ))
                .child(
                    AuroraButton::new(
                        "log-latest",
                        if self.follow {
                            "Following live output"
                        } else {
                            "Jump to latest"
                        },
                    )
                    .disabled(self.follow)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.follow = true;
                        this.scroll.scroll_to_bottom();
                        cx.notify();
                    })),
                ),
        );
        if !self.message.is_empty() {
            body = body.child(ui::caption(&t, self.message.clone()));
        }
        let view = cx.entity();
        let cancel = view.clone();
        let confirm = view.clone();
        let panel = ui::card(&t).w(px(460.)).bg(t.raised).flex().flex_col().gap(px(16.))
            .child(ui::section_header(&t, "Upload a public log?", None))
            .child(ui::muted(&t, "Your redacted game output will be sent to mclo.gs. Anyone with the public link can read it. Access tokens, session IDs, authentication UUIDs and user paths are removed. Other text written by the game or mods may still identify you. Older lines may be truncated."))
            .child(div().flex().justify_end().gap(px(8.))
                .child(AuroraButton::new("upload-cancel", "Cancel").on_click(move |_, _, cx| { cancel.update(cx, |s, cx| { s.upload_confirm = false; cx.notify(); }); }))
                .child(AuroraButton::new("upload-confirm", "Upload public log").variant(ButtonVariant::Primary).on_click(move |_, _, cx| { confirm.update(cx, |s, cx| s.upload(cx)); })));
        root.child(body).child(
            Dialog::new(cx)
                .open(self.upload_confirm)
                .accessibility_label("Upload a public log?")
                .close_on_backdrop_press(false)
                .on_ok(|_, _, _| false)
                .on_open_change(move |open, _, _, cx| {
                    if !open {
                        view.update(cx, |s, cx| {
                            s.upload_confirm = false;
                            cx.notify();
                        });
                    }
                })
                .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                .popup(panel),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_search_and_keep_stack_traces() {
        assert!(matches("WARN resource", "warn", "RESOURCE", 1));
        assert!(!matches("INFO ready", "info", "", 1));
        assert!(!matches("WARN resource", "warn", "", 2));
        assert!(matches("at example.Foo", "trace", "foo", 2));
    }
}

impl crate::Launcher {
    pub(super) fn can_kill(&self, id: &str) -> bool {
        if let Some(p) = self.processes.get(id).filter(|p| p.status == "running") {
            return p.can_stop;
        }
        self.launching.as_deref() == Some(id)
            || self
                .processes
                .get(id)
                .is_some_and(|p| p.can_stop && p.status == "starting")
    }
    pub(super) fn maybe_open_log(&self, id: String) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let enabled = guarded(api::game_log_preference(host, None))
                .await
                .unwrap_or(false);
            let _ = tx.send(crate::Message::MaybeOpenLog(id, enabled)).await;
        });
    }
    pub(super) fn open_game_log(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(handle) = self.log_windows.get(&id) {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
        }
        let name = self
            .launcher
            .as_ref()
            .and_then(|s| s.instances.iter().find(|i| i.id == id))
            .map(|i| i.display_name.clone())
            .unwrap_or_else(|| "Instance".into());
        let bounds = gpui::Bounds::centered(None, gpui::size(px(900.), px(760.)), cx);
        let host = self.host.clone();
        let rt = self.rt.clone();
        let instance = id.clone();
        match cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                titlebar: Some(chrome::titlebar()),
                window_min_size: Some(gpui::size(px(720.), px(520.))),
                ..Default::default()
            },
            move |window, cx| cx.new(|cx| GameLog::new(instance, name, host, rt, window, cx)),
        ) {
            Ok(handle) => {
                self.log_windows.insert(id, handle);
            }
            Err(_) => self.status = "The game log window could not be opened.".into(),
        }
        cx.notify();
    }
    pub(super) fn send_stop(&self, id: String, pid: Option<u32>) {
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let _ = tx
                .send(crate::Message::StopResult(
                    guarded(api::stop_game(id, pid)).await,
                ))
                .await;
        });
    }
    pub(super) fn request_kill(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(p) = self.processes.get(&id).filter(|p| p.status == "running") {
            if p.can_stop {
                if let Some(pid) = p.process_id {
                    self.kill_confirmation = Some((id, pid));
                }
            }
        } else if self.can_kill(&id) {
            // If the worker has not reserved preparation yet, latch the intent
            // until its Starting event. Otherwise dispatch cancellation now.
            if self
                .processes
                .get(&id)
                .is_some_and(|p| p.status == "starting" && p.can_stop)
            {
                self.send_stop(id, None);
            } else {
                self.pending_stop = Some(id);
            }
        }
        cx.notify();
    }
    pub(super) fn kill_dialog(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = theme::theme(cx).clone();
        let view = cx.entity();
        let confirm = view.clone();
        let cancel = view.clone();
        let panel = ui::card(&t)
            .w(px(440.))
            .bg(t.raised)
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(ui::section_header(&t, "Stop the game?", None))
            .child(ui::muted(
                &t,
                "Unsaved world progress may be lost. This closes the game immediately.",
            ))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(AuroraButton::new("kill-cancel", "Keep playing").on_click(
                        move |_, _, cx| {
                            cancel.update(cx, |s, cx| {
                                s.kill_confirmation = None;
                                cx.notify();
                            });
                        },
                    ))
                    .child(
                        AuroraButton::new("kill-confirm", "Kill game")
                            .variant(ButtonVariant::Danger)
                            .on_click(move |_, _, cx| {
                                confirm.update(cx, |s, cx| {
                                    if let Some((id, pid)) = s.kill_confirmation.take() {
                                        s.send_stop(id, Some(pid));
                                    }
                                    cx.notify();
                                });
                            }),
                    ),
            );
        Dialog::new(cx)
            .open(self.kill_confirmation.is_some())
            .accessibility_label("Stop the game?")
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_open_change(move |open, _, _, cx| {
                if !open {
                    view.update(cx, |s, cx| {
                        s.kill_confirmation = None;
                        cx.notify();
                    });
                }
            })
            .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
            .popup(panel)
            .into_any_element()
    }
}

fn showcase_process(id: &str, status: &str, can_stop: bool) -> api::LaunchProcess {
    serde_json::from_value(serde_json::json!({ "instanceId": id, "status": status, "canStop": can_stop, "processId": 424242, "exitCode": null, "message": null })).expect("showcase fixture")
}
impl crate::Launcher {
    pub(super) fn apply_game_showcase(&mut self, cx: &mut Context<Self>) {
        if !crate::showcase() || self.game_showcase_applied || !self.loaded {
            return;
        }
        let Ok(state) = std::env::var("AURORA_SHOWCASE_GAME_STATE") else {
            return;
        };
        let Some(id) = self.selected_id().map(str::to_owned) else {
            return;
        };
        self.game_showcase_applied = true;
        self.load_in_flight = false;
        if state == "preparing" {
            self.launching = Some(id.clone());
            self.processes
                .insert(id.clone(), showcase_process(&id, "starting", true));
        } else if state == "running" || state == "kill-confirmation" {
            self.processes
                .insert(id.clone(), showcase_process(&id, "running", true));
            if state == "kill-confirmation" {
                self.kill_confirmation = Some((id.clone(), 424242));
            }
        } else if state.starts_with("logs-") {
            let session = aurora_launcher_lib::launch::logs::session(&id);
            session.clear();
            for n in 0..36 {
                session.push(
                    &format!(
                        "[12:42:{:02}] [Render thread/INFO]: Preparing world resources ({}/36)",
                        n % 60,
                        n + 1
                    ),
                    "stdout",
                    &[],
                );
            }
            for line in [
                "[12:43:01] [Render thread/INFO]: Loading world: Aurora Valley",
                "[12:43:02] [Render thread/INFO]: Game session started",
                "[12:43:03] [Render thread/WARN]: Resource pack missing optional texture",
                "[12:43:04] [Render thread/ERROR]: java.lang.OutOfMemoryError: Java heap space",
                "    at net.minecraft.client.world.ClientWorld.tick(ClientWorld.java:214)",
                "    at net.minecraft.client.MinecraftClient.run(MinecraftClient.java:1034)",
                "[12:43:05] [Render thread/INFO]: Access token: [redacted]",
            ] {
                session.push(line, "stdout", &[]);
            }
            self.open_game_log(id, cx);
        }
        cx.notify();
    }
}
