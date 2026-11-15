//! Home's server-list editor. The frozen instance, revision and row index are
//! sent to the core; presentation never writes NBT or derives instance paths.
use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, AuroraInput, ButtonSize, ButtonVariant},
    theme, ui,
};
use aurora_launcher_lib::host::Host;
use gpui::{
    Context, Entity, EventEmitter, IntoElement, ParentElement, Render, Styled, Window, div,
    prelude::*, px,
};
use gpui_kit::base::{
    Dialog,
    input::{InputEvent, InputState},
};

pub enum ServerEditorEvent {
    Changed,
    BusyChanged(bool),
}
impl EventEmitter<ServerEditorEvent> for ServerEditor {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    Add,
    Edit(usize),
    Delete(usize),
}

#[derive(Default)]
struct State {
    instance: Option<String>,
    instance_name: String,
    list: Option<api::ServerList>,
    form: Option<Form>,
    packs: api::ResourcePacks,
    busy: bool,
    serial: u64,
    message: String,
}
impl State {
    fn open(&mut self, instance: String, name: String) -> bool {
        if self.busy {
            return false;
        }
        *self = Self {
            instance: Some(instance),
            instance_name: name,
            serial: self.serial + 1,
            ..Default::default()
        };
        true
    }
    fn close(&mut self) -> bool {
        if self.busy {
            return false;
        }
        self.instance = None;
        self.list = None;
        self.form = None;
        self.serial += 1;
        true
    }
    fn choose(&mut self, form: Form) -> bool {
        if self.busy || self.list.is_none() {
            return false;
        }
        if let Form::Edit(i) | Form::Delete(i) = form {
            if self.list.as_ref().is_none_or(|l| i >= l.entries.len()) {
                return false;
            }
        }
        self.form = Some(form);
        self.message.clear();
        true
    }
    fn duplicate(&self, input: &api::ServerInput) -> bool {
        self.list.as_ref().is_some_and(|list| {
            list.entries.iter().enumerate().any(|(i, s)| {
                self.form != Some(Form::Edit(i))
                    && s.name == input.name
                    && s.address == input.address
            })
        })
    }
    fn receive(&mut self, serial: u64, result: api::ApiResult<api::ServerList>) -> bool {
        if serial != self.serial || self.instance.is_none() {
            return false;
        }
        self.busy = false;
        match result {
            Ok(list) => {
                self.list = Some(list);
                self.form = None;
                self.message.clear();
            }
            Err(error) => self.message = error.message,
        }
        true
    }
}

pub struct ServerEditor {
    host: Option<Host>,
    runtime: tokio::runtime::Handle,
    state: State,
    name: Entity<InputState>,
    address: Entity<InputState>,
    scroll: gpui::ScrollHandle,
    list_focus: gpui::FocusHandle,
    _inputs: Vec<gpui::Subscription>,
}
impl ServerEditor {
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Server name"));
        let address =
            cx.new(|cx| InputState::new(window, cx).placeholder("play.example.com:25565"));
        let inputs = [&name, &address]
            .into_iter()
            .map(|field| {
                cx.subscribe(field, |_: &mut Self, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
            })
            .collect();
        Self {
            host,
            runtime,
            state: State::default(),
            name,
            address,
            scroll: gpui::ScrollHandle::new(),
            list_focus: cx.focus_handle(),
            _inputs: inputs,
        }
    }
    pub fn open(&mut self, instance: String, name: String, cx: &mut Context<Self>) {
        if self.host.is_some() && self.state.open(instance, name) {
            self.request(None, cx);
        }
    }
    fn input(&self, cx: &Context<Self>) -> api::ServerInput {
        api::ServerInput {
            name: self.name.read(cx).value().to_string(),
            address: self.address.read(cx).value().to_string(),
            resource_packs: self.state.packs,
        }
    }
    fn choose(&mut self, form: Form, window: &mut Window, cx: &mut Context<Self>) {
        if !self.state.choose(form) {
            return;
        }
        let server = match form {
            Form::Edit(i) => self.state.list.as_ref().unwrap().entries[i].clone(),
            _ => api::ServerInput {
                name: String::new(),
                address: String::new(),
                resource_packs: api::ResourcePacks::Prompt,
            },
        };
        self.state.packs = server.resource_packs;
        self.name
            .update(cx, |field, cx| field.set_value(server.name, window, cx));
        self.address
            .update(cx, |field, cx| field.set_value(server.address, window, cx));
        cx.notify();
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.state.busy {
            return;
        }
        let edit = match self.state.form {
            Some(Form::Delete(index)) => api::ServerEdit::Delete { index },
            Some(form) => {
                let server = self.input(cx);
                if let Err(error) = aurora_launcher_lib::saved_servers::editor::validate(&server) {
                    self.state.message = error.message().into();
                    cx.notify();
                    return;
                }
                match form {
                    Form::Add => api::ServerEdit::Add { server },
                    Form::Edit(index) => api::ServerEdit::Edit { index, server },
                    _ => unreachable!(),
                }
            }
            None => return,
        };
        self.request(Some(edit), cx);
    }
    fn request(&mut self, edit: Option<api::ServerEdit>, cx: &mut Context<Self>) {
        if self.state.busy {
            return;
        }
        let (Some(host), Some(instance)) = (self.host.clone(), self.state.instance.clone()) else {
            return;
        };
        let revision = self
            .state
            .list
            .as_ref()
            .map(|l| l.revision.clone())
            .unwrap_or_default();
        if edit.is_some() && self.state.list.is_none() {
            return;
        }
        self.state.busy = true;
        self.state.serial += 1;
        self.state.message.clear();
        let serial = self.state.serial;
        let writing = edit.is_some();
        cx.emit(ServerEditorEvent::BusyChanged(true));
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.runtime.spawn(async move {
            let result = guarded(async move {
                match edit {
                    Some(edit) => api::edit_server_list(host, instance, revision, edit).await,
                    None => api::server_list(host, instance).await,
                }
            })
            .await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            let result = rx.await.unwrap_or_else(|_| {
                Err(api::ApiError::new(
                    "server_list_unavailable",
                    "The operation stopped. Reload the server list and try again.",
                ))
            });
            let changed = writing && result.is_ok();
            let _ = weak.update(cx, |view, cx| {
                if view.state.receive(serial, result) {
                    cx.emit(ServerEditorEvent::BusyChanged(false));
                    if changed {
                        cx.emit(ServerEditorEvent::Changed);
                    }
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for ServerEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.state.instance.is_none() {
            return div().absolute().size_0();
        }
        let t = theme::theme(cx).clone();
        let busy = self.state.busy;
        for field in [&self.name, &self.address] {
            field.update(cx, |field, cx| field.set_disabled(busy, cx));
        }
        let this = cx.entity();
        let cancel = this.clone();
        let changed = this.clone();
        let veto = this.clone();
        let width = (window.viewport_size().width - px(48.))
            .min(px(680.))
            .max(px(280.));
        let mut panel = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(24.))
            .w(width)
            .max_h((window.viewport_size().height - px(64.)).max(px(240.)))
            .rounded(t.radius_lg)
            .bg(t.card)
            .text_color(t.foreground)
            .child(ui::section_header(
                &t,
                "Manage servers",
                Some(self.state.instance_name.clone().into()),
            ));
        if let Some(form) = self.state.form {
            let title = match form {
                Form::Add => "Add server",
                Form::Edit(_) => "Edit server",
                Form::Delete(_) => "Delete server?",
            };
            panel = panel.child(ui::section_header(&t, title, None));
            if let Form::Delete(i) = form {
                let server = &self.state.list.as_ref().unwrap().entries[i];
                panel = panel.child(ui::muted(
                    &t,
                    format!(
                        "Remove “{}” ({}) from this instance’s multiplayer list?",
                        server.name, server.address
                    ),
                ));
            } else {
                panel = panel
                    .child(
                        div()
                            .child(ui::caption(&t, "Name"))
                            .child(AuroraInput::new(&self.name).label("Server name")),
                    )
                    .child(
                        div()
                            .child(ui::caption(&t, "Address"))
                            .child(AuroraInput::new(&self.address).label("Server address")),
                    )
                    .child(ui::caption(&t, "Server resource packs"));
                let mut choices = div().flex().flex_wrap().gap(px(8.));
                for (value, label) in [
                    (api::ResourcePacks::Prompt, "Prompt"),
                    (api::ResourcePacks::Enabled, "Enabled"),
                    (api::ResourcePacks::Disabled, "Disabled"),
                ] {
                    choices = choices.child(
                        AuroraButton::new(format!("packs-{label}"), label)
                            .size(ButtonSize::Small)
                            .selected(self.state.packs == value)
                            .variant(if self.state.packs == value {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Default
                            })
                            .disabled(busy)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.state.packs = value;
                                cx.notify();
                            })),
                    );
                }
                panel = panel.child(choices);
                if self.state.duplicate(&self.input(cx)) {
                    panel = panel.child(ui::muted(&t, "A server with this exact name and address is already saved. Saving will keep both entries."));
                }
            }
            panel = panel.child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("server-form-cancel", "Cancel")
                            .disabled(busy)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.state.form = None;
                                view.state.message.clear();
                                cx.notify();
                            })),
                    )
                    .child(
                        AuroraButton::new(
                            "server-form-save",
                            if matches!(form, Form::Delete(_)) {
                                "Delete server"
                            } else {
                                "Save server"
                            },
                        )
                        .variant(if matches!(form, Form::Delete(_)) {
                            ButtonVariant::Danger
                        } else {
                            ButtonVariant::Primary
                        })
                        .disabled(busy)
                        .on_click(cx.listener(|view, _, _, cx| view.save(cx))),
                    ),
            );
        } else {
            let scroll = self.scroll.clone();
            let mut rows = div()
                .id("server-editor-rows")
                .role(gpui::Role::ScrollView)
                .aria_label("Saved servers in Minecraft order")
                .track_focus(&self.list_focus)
                .tab_stop(true)
                .border_1()
                .border_color(gpui::transparent_black())
                .focus(move |style| style.border_color(t.ring))
                .on_key_down(cx.listener(move |_, event: &gpui::KeyDownEvent, _, cx| {
                    let mut offset = scroll.offset();
                    let page = scroll.bounds().size.height.max(px(32.));
                    offset.y = match event.keystroke.key.as_str() {
                        "up" => offset.y + px(32.),
                        "down" => offset.y - px(32.),
                        "pageup" => offset.y + page,
                        "pagedown" => offset.y - page,
                        "home" => px(0.),
                        "end" => -scroll.max_offset().y,
                        _ => return,
                    };
                    offset.y = offset.y.max(-scroll.max_offset().y).min(px(0.));
                    scroll.set_offset(offset);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .flex()
                .flex_col()
                .min_h_0()
                .flex_1()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .gap(px(8.));
            if let Some(list) = &self.state.list {
                if list.entries.is_empty() {
                    rows = rows.child(ui::muted(&t, "No servers saved. Add one to get started."));
                }
                for (index, server) in list.entries.iter().enumerate() {
                    let len = list.entries.len();
                    rows = rows.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(8.))
                            .py(px(4.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(140.))
                                    .child(
                                        div()
                                            .text_size(px(ui::TEXT_BODY))
                                            .child(server.name.clone()),
                                    )
                                    .child(ui::caption(&t, server.address.clone())),
                            )
                            .child(
                                AuroraButton::new(format!("server-edit-{index}"), "Edit")
                                    .size(ButtonSize::Small)
                                    .disabled(busy)
                                    .accessibility_label(format!(
                                        "Edit server {}: {}",
                                        index + 1,
                                        server.name
                                    ))
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.choose(Form::Edit(index), window, cx)
                                    })),
                            )
                            .child(
                                AuroraButton::new(format!("server-up-{index}"), "Up")
                                    .size(ButtonSize::Small)
                                    .disabled(busy || index == 0)
                                    .accessibility_label(format!(
                                        "Move server {} up: {}",
                                        index + 1,
                                        server.name
                                    ))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.request(
                                            Some(api::ServerEdit::Move {
                                                index,
                                                destination: index.saturating_sub(1),
                                            }),
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                AuroraButton::new(format!("server-down-{index}"), "Down")
                                    .size(ButtonSize::Small)
                                    .disabled(busy || index + 1 == len)
                                    .accessibility_label(format!(
                                        "Move server {} down: {}",
                                        index + 1,
                                        server.name
                                    ))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.request(
                                            Some(api::ServerEdit::Move {
                                                index,
                                                destination: index + 1,
                                            }),
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                AuroraButton::new(format!("server-delete-{index}"), "Delete")
                                    .size(ButtonSize::Small)
                                    .disabled(busy)
                                    .accessibility_label(format!(
                                        "Delete server {}: {}",
                                        index + 1,
                                        server.name
                                    ))
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.choose(Form::Delete(index), window, cx)
                                    })),
                            ),
                    );
                }
            } else {
                rows = rows.child(ui::muted(
                    &t,
                    if busy {
                        "Loading server list…"
                    } else {
                        "Reload the server list to try again."
                    },
                ));
            }
            panel = panel.child(rows).child(
                div()
                    .flex()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("server-add", "Add server")
                            .icon("add")
                            .variant(ButtonVariant::Primary)
                            .disabled(busy || self.state.list.is_none())
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.choose(Form::Add, window, cx)
                            })),
                    )
                    .child(
                        AuroraButton::new("server-close", "Done")
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                cancel.update(cx, |view, cx| {
                                    view.state.close();
                                    cx.notify();
                                });
                            }),
                    ),
            );
        }
        if !self.state.message.is_empty() {
            panel = panel
                .child(
                    div()
                        .text_size(px(ui::TEXT_BODY))
                        .text_color(t.danger)
                        .child(self.state.message.clone()),
                )
                .child(
                    AuroraButton::new("server-reload", "Reload list")
                        .disabled(busy)
                        .on_click(cx.listener(|view, _, _, cx| view.request(None, cx))),
                );
        }
        if busy {
            panel = panel.child(ui::caption(&t, "Working…"));
        }
        div().absolute().size_0().child(
            Dialog::new(cx)
                .open(self.state.instance.is_some())
                .accessibility_label("Manage Minecraft servers")
                .close_on_backdrop_press(false)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !veto.read(cx).state.busy)
                .on_open_change(move |open, _, _, cx| {
                    if !open {
                        changed.update(cx, |view, cx| {
                            view.state.close();
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
    fn server() -> api::ServerInput {
        api::ServerInput {
            name: "Practice".into(),
            address: "example.invalid".into(),
            resource_packs: api::ResourcePacks::Prompt,
        }
    }
    fn loaded() -> State {
        let mut state = State::default();
        state.open("instance-a".into(), "A".into());
        state.receive(
            state.serial,
            Ok(api::ServerList {
                revision: "hash".into(),
                entries: vec![server(), server()],
            }),
        );
        state
    }
    #[test]
    fn dialogs_require_loaded_rows_and_explicit_delete_selection() {
        let mut state = State::default();
        assert!(!state.choose(Form::Add));
        state = loaded();
        assert!(!state.choose(Form::Delete(2)));
        assert!(state.choose(Form::Delete(0)));
        assert_eq!(state.list.as_ref().unwrap().entries.len(), 2);
        assert!(state.close());
        assert_eq!(state.form, None);
    }
    #[test]
    fn busy_dialog_cannot_close_switch_instance_or_change_form() {
        let mut state = loaded();
        state.busy = true;
        assert!(!state.close());
        assert!(!state.open("b".into(), "B".into()));
        assert!(!state.choose(Form::Add));
        assert_eq!(state.instance.as_deref(), Some("instance-a"));
    }
    #[test]
    fn stale_replies_cannot_replace_another_instances_list() {
        let mut state = loaded();
        let serial = state.serial;
        state.close();
        state.open("b".into(), "B".into());
        assert!(!state.receive(
            serial,
            Ok(api::ServerList {
                revision: "stale".into(),
                entries: vec![server()]
            })
        ));
        assert!(state.list.is_none());
    }
    #[test]
    fn failed_save_preserves_form_revision_and_reports_reload() {
        let mut state = loaded();
        state.choose(Form::Edit(0));
        state.busy = true;
        state.receive(
            state.serial,
            Err(api::ApiError::new("server_list_changed", "Reload list")),
        );
        assert!(!state.busy);
        assert_eq!(state.form, Some(Form::Edit(0)));
        assert_eq!(state.list.as_ref().unwrap().revision, "hash");
        assert_eq!(state.message, "Reload list");
    }
    #[test]
    fn exact_duplicate_warning_excludes_the_edited_row_but_allows_duplicates() {
        let mut state = loaded();
        state.choose(Form::Edit(0));
        assert!(state.duplicate(&server()));
        state.list.as_mut().unwrap().entries.pop();
        assert!(!state.duplicate(&server()));
        state.choose(Form::Add);
        assert!(state.duplicate(&server()));
    }
    #[test]
    fn acknowledged_save_installs_new_order_and_closes_form() {
        let mut state = loaded();
        state.choose(Form::Add);
        state.receive(
            state.serial,
            Ok(api::ServerList {
                revision: "new".into(),
                entries: vec![],
            }),
        );
        assert_eq!(state.form, None);
        assert_eq!(state.list.unwrap().revision, "new");
    }
}
