//! Catalog-driven instance creation; installation remains in the core.
use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, AuroraInput, AuroraSelect, AuroraToggle, ButtonVariant},
    theme::AuroraTheme,
    ui,
};
use aurora_launcher_lib::host::Host;
use gpui::{
    Bounds, Context, Div, Entity, EventEmitter, FocusHandle, Global, Pixels, Render, SharedString,
    Subscription, WeakEntity, Window, div, prelude::*, px,
};
use gpui_kit::base::{
    Dialog,
    input::{InputEvent, InputState},
};
use std::{cell::Cell, rc::Rc};

pub enum InstanceCreatorEvent {
    Changed,
}
impl EventEmitter<InstanceCreatorEvent> for InstanceCreator {}

/// Lets the page header open the creator without owning its state.
struct CreatorHandle(WeakEntity<InstanceCreator>);
impl Global for CreatorHandle {}

struct SelectState {
    trigger: FocusHandle,
    content: FocusHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    open: bool,
    highlight: usize,
}
impl SelectState {
    fn new(cx: &mut Context<InstanceCreator>) -> Self {
        Self {
            trigger: cx.focus_handle(),
            content: cx.focus_handle(),
            bounds: Rc::new(Cell::new(None)),
            open: false,
            highlight: 0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Choice {
    minecraft: String,
    kind: String,
    pin: Option<String>,
}
fn loader(choice: &Choice) -> serde_json::Value {
    if choice.kind == "vanilla" {
        serde_json::json!({"kind":"vanilla"})
    } else if let Some(pin) = &choice.pin {
        serde_json::json!({"kind":choice.kind,"policy":{"type":"pinned","version":pin}})
    } else {
        serde_json::json!({"kind":choice.kind,"policy":{"type":"automatic"}})
    }
}
fn valid_choice(
    choice: &Choice,
    versions: &[api::MinecraftVersion],
    kinds: &[String],
    loaders: Option<&[api::LoaderVersion]>,
) -> bool {
    versions.iter().any(|v| v.id == choice.minecraft)
        && kinds.contains(&choice.kind)
        && (choice.kind == "vanilla"
            || loaders.is_some_and(|items| {
                !items.is_empty()
                    && choice
                        .pin
                        .as_ref()
                        .is_none_or(|pin| items.iter().any(|v| &v.version == pin))
            }))
}
fn current(serial: u64, active: u64) -> bool {
    serial == active
}
fn current_choice(serial: u64, active: u64, response: &Choice, selection: Option<&Choice>) -> bool {
    current(serial, active) && selection == Some(response)
}
fn aurora_ready(enabled: bool, compatibility: Option<&api::AuroraCompatibility>) -> bool {
    !enabled || compatibility.is_some_and(|c| c.available)
}
// Only an explicit Minecraft choice or starting creation freezes the default.
// Platform, loader policy and the Aurora toggle have independent intent.
fn late_minecraft_default(
    current: Option<&str>,
    versions: &[api::MinecraftVersion],
    published: Option<&str>,
    minecraft_selected: bool,
    creating: bool,
) -> Option<String> {
    if minecraft_selected || creating {
        return None;
    }
    let published = published?;
    versions
        .iter()
        .find(|version| version.id == published && current != Some(published))
        .map(|version| version.id.clone())
}

enum Reply {
    Catalog(
        u64,
        api::ApiResult<api::LauncherState>,
        api::ApiResult<Vec<api::MinecraftVersion>>,
    ),
    Releases(u64, api::ApiResult<Vec<api::AuroraRelease>>),
    Loaders(u64, String, String, api::ApiResult<Vec<api::LoaderVersion>>),
    Compatibility(u64, Choice, api::ApiResult<api::AuroraCompatibility>),
    Created(api::ApiResult<api::InstanceSummary>),
}

pub struct InstanceCreator {
    host: Option<Host>,
    runtime: tokio::runtime::Handle,
    tx: async_channel::Sender<Reply>,
    _subscriptions: Vec<Subscription>,
    name: Entity<InputState>,
    open: bool,
    busy: bool,
    snapshots: bool,
    aurora: bool,
    versions: Vec<api::MinecraftVersion>,
    releases: Vec<api::AuroraRelease>,
    minecraft_selected: bool,
    kinds: Vec<String>,
    minecraft: Option<String>,
    kind: Option<String>,
    pin: Option<String>,
    loaders: Option<Vec<api::LoaderVersion>>,
    compatibility: Option<api::AuroraCompatibility>,
    catalog_serial: u64,
    loader_serial: u64,
    compatibility_serial: u64,
    catalog_ready: bool,
    message: String,
    note: String,
    selects: [SelectState; 3],
    /// Design screenshots only: the dialog was opened once.
    showcase_seeded: bool,
}
impl InstanceCreator {
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded();
        cx.set_global(CreatorHandle(cx.weak_entity()));
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Instance name"));
        let subscription = cx.subscribe(&name, |this: &mut Self, _, _: &InputEvent, cx| {
            if !this.message.is_empty() {
                this.message.clear();
                cx.notify();
            }
        });
        cx.spawn(async move |weak, cx| {
            while let Ok(reply) = rx.recv().await {
                if weak
                    .update(cx, |this: &mut Self, cx| {
                        this.receive(reply, cx);
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
            runtime,
            tx,
            _subscriptions: vec![subscription],
            name,
            open: false,
            busy: false,
            snapshots: false,
            aurora: false,
            versions: vec![],
            releases: vec![],
            minecraft_selected: false,
            kinds: vec![],
            minecraft: None,
            kind: None,
            pin: None,
            loaders: None,
            compatibility: None,
            catalog_serial: 0,
            loader_serial: 0,
            compatibility_serial: 0,
            catalog_ready: false,
            message: String::new(),
            note: String::new(),
            selects: [
                SelectState::new(cx),
                SelectState::new(cx),
                SelectState::new(cx),
            ],
            showcase_seeded: false,
        }
    }
    /// Opens the creation dialog, loading the catalogs on first use.
    fn open_dialog(&mut self, cx: &mut Context<Self>) {
        self.open = true;
        if !self.catalog_ready {
            self.refresh(cx);
        }
        cx.notify();
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(host) = self.host.clone() else {
            self.message = "Instance creation is unavailable in diagnostic mode.".into();
            cx.notify();
            return;
        };
        self.catalog_serial += 1;
        self.loader_serial += 1;
        self.compatibility_serial += 1;
        self.catalog_ready = false;
        self.loaders = None;
        self.compatibility = None;
        self.note.clear();
        let serial = self.catalog_serial;
        let snapshots = self.snapshots;
        let tx = self.tx.clone();
        let releases_host = host.clone();
        let releases_tx = tx.clone();
        self.runtime.spawn(async move {
            let releases = guarded(api::aurora_releases(releases_host)).await;
            let _ = releases_tx.send(Reply::Releases(serial, releases)).await;
        });
        self.runtime.spawn(async move {
            let (state, versions) = tokio::join!(
                guarded(api::launcher_state(host)),
                guarded(api::minecraft_versions(snapshots))
            );
            let _ = tx.send(Reply::Catalog(serial, state, versions)).await;
        });
        cx.notify();
    }
    fn choice(&self) -> Option<Choice> {
        Some(Choice {
            minecraft: self.minecraft.clone()?,
            kind: self.kind.clone()?,
            pin: self.pin.clone(),
        })
    }
    fn reload_selection(&mut self) {
        self.loader_serial += 1;
        self.compatibility_serial += 1;
        self.loaders = None;
        self.compatibility = None;
        self.note.clear();
        let (Some(host), Some(choice)) = (self.host.clone(), self.choice()) else {
            return;
        };
        if choice.kind != "vanilla" {
            let serial = self.loader_serial;
            let mc = choice.minecraft.clone();
            let kind = choice.kind.clone();
            let tx = self.tx.clone();
            let mc2 = mc.clone();
            let kind2 = kind.clone();
            self.runtime.spawn(async move {
                let r = guarded(api::loader_versions(kind2, mc2)).await;
                let _ = tx.send(Reply::Loaders(serial, mc, kind, r)).await;
            });
        }
        self.request_compatibility(host, choice);
    }
    fn request_compatibility(&mut self, host: Host, choice: Choice) {
        self.compatibility_serial += 1;
        self.compatibility = None;
        let serial = self.compatibility_serial;
        let tx = self.tx.clone();
        let selected = choice.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::aurora_compatibility(
                host,
                selected.minecraft.clone(),
                loader(&selected),
            ))
            .await;
            let _ = tx.send(Reply::Compatibility(serial, choice, r)).await;
        });
    }
    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Catalog(serial, state, versions) if current(serial, self.catalog_serial) => {
                match (state, versions) {
                    (Ok(state), Ok(versions)) => {
                        self.kinds = state
                            .platform_capabilities
                            .into_iter()
                            .filter(|c| c.can_create)
                            .map(|c| c.kind)
                            .collect();
                        self.versions = versions;
                        if !self
                            .minecraft
                            .as_ref()
                            .is_some_and(|id| self.versions.iter().any(|v| &v.id == id))
                        {
                            self.minecraft = self
                                .releases
                                .first()
                                .and_then(|r| {
                                    self.versions.iter().find(|v| v.id == r.minecraft_version)
                                })
                                .or_else(|| self.versions.first())
                                .map(|v| v.id.clone());
                            self.pin = None;
                        }
                        if !self.kind.as_ref().is_some_and(|k| self.kinds.contains(k)) {
                            self.kind = self
                                .kinds
                                .iter()
                                .find(|k| *k == "fabric")
                                .or_else(|| self.kinds.first())
                                .cloned();
                            self.pin = None;
                        }
                        self.catalog_ready = true;
                        self.message.clear();
                        self.reload_selection();
                    }
                    (state, versions) => {
                        let e = state.err().or_else(|| versions.err()).unwrap();
                        self.message = format!("{}: {}", e.code, e.message);
                    }
                }
            }
            Reply::Releases(serial, result) if current(serial, self.catalog_serial) => {
                if let Ok(releases) = result {
                    self.releases = releases;
                    if self.catalog_ready {
                        if let Some(version) = late_minecraft_default(
                            self.minecraft.as_deref(),
                            &self.versions,
                            self.releases.first().map(|r| r.minecraft_version.as_str()),
                            self.minecraft_selected,
                            self.busy,
                        ) {
                            self.minecraft = Some(version);
                            self.pin = None;
                            self.reload_selection();
                        }
                    }
                }
            }
            Reply::Loaders(serial, mc, kind, result)
                if current(serial, self.loader_serial)
                    && self
                        .choice()
                        .is_some_and(|c| c.minecraft == mc && c.kind == kind) =>
            {
                match result {
                    Ok(v) => {
                        self.loaders = Some(v);
                    }
                    Err(e) => self.message = format!("{}: {}", e.code, e.message),
                }
            }
            Reply::Compatibility(serial, choice, result)
                if current_choice(
                    serial,
                    self.compatibility_serial,
                    &choice,
                    self.choice().as_ref(),
                ) =>
            {
                match result {
                    Ok(c) => {
                        self.note = if c.available {
                            format!(
                                "{}{}",
                                c.reason,
                                c.version
                                    .as_ref()
                                    .map(|v| format!(" Aurora {v}."))
                                    .unwrap_or_default()
                            )
                        } else {
                            c.reason.clone()
                        };
                        self.compatibility = Some(c);
                    }
                    Err(e) => {
                        self.note = format!("{}: {}", e.code, e.message);
                    }
                }
            }
            Reply::Created(result) => {
                self.busy = false;
                match result {
                    Ok(_) => {
                        self.open = false;
                        self.message = "Instance created.".into();
                        cx.emit(InstanceCreatorEvent::Changed);
                    }
                    Err(e) => {
                        if let Some(id) = &e.instance_id {
                            cx.emit(InstanceCreatorEvent::Changed);
                            self.message = format!(
                                "{}: {} The instance {id} was retained for retry.",
                                e.code, e.message
                            );
                        } else {
                            self.message = format!("{}: {}", e.code, e.message);
                        }
                    }
                }
            }
            _ => return,
        }
        cx.notify();
    }
    fn select(&mut self, which: usize, ix: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.message.clear();
        match which {
            0 => {
                if let Some(v) = self.versions.get(ix) {
                    self.minecraft_selected = true;
                    self.minecraft = Some(v.id.clone());
                    self.pin = None;
                    self.reload_selection();
                }
            }
            1 => {
                if let Some(k) = self.kinds.get(ix) {
                    self.kind = Some(k.clone());
                    self.pin = None;
                    self.reload_selection();
                }
            }
            2 => {
                let pin = if ix == 0 {
                    None
                } else {
                    self.loaders
                        .as_ref()
                        .and_then(|v| v.get(ix - 1))
                        .map(|v| v.version.clone())
                };
                if ix == 0 || pin.is_some() {
                    self.pin = pin;
                    self.compatibility_serial += 1;
                    self.compatibility = None;
                    if let (Some(host), Some(choice)) = (self.host.clone(), self.choice()) {
                        self.request_compatibility(host, choice);
                    }
                }
            }
            _ => {}
        }
        cx.notify();
    }
    fn create(&mut self, cx: &mut Context<Self>) {
        if self.busy || !self.catalog_ready {
            return;
        }
        let Some(choice) = self.choice() else {
            self.message = "Select a Minecraft version and platform.".into();
            cx.notify();
            return;
        };
        let name = self.name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.message = "Enter a name for the instance.".into();
        } else if !valid_choice(
            &choice,
            &self.versions,
            &self.kinds,
            self.loaders.as_deref(),
        ) {
            self.message = "Select a supported version and loader policy.".into();
        } else if !aurora_ready(self.aurora, self.compatibility.as_ref()) {
            self.message = "Aurora is unavailable for this selection.".into();
        } else if let Some(host) = self.host.clone() {
            self.busy = true;
            self.minecraft_selected = true;
            self.message = "Creating instance…".into();
            let aurora = self.aurora;
            let tx = self.tx.clone();
            self.runtime.spawn(async move {
                let value = loader(&choice);
                let r = guarded(api::create_instance(
                    host,
                    name,
                    choice.minecraft,
                    value,
                    aurora,
                ))
                .await;
                let _ = tx.send(Reply::Created(r)).await;
            });
        }
        cx.notify();
    }
}
/// The page header's "New instance" action. The creator owns the dialog and
/// its state; the Instances header (rendered by the import view) hosts the
/// button so the page has a single action row.
pub fn new_instance_button(cx: &gpui::App) -> Option<AuroraButton> {
    let creator = cx.try_global::<CreatorHandle>()?.0.upgrade()?;
    let (no_host, busy) = {
        let state = creator.read(cx);
        (state.host.is_none(), state.busy)
    };
    Some(
        AuroraButton::new("new-instance", "New instance")
            .variant(ButtonVariant::Primary)
            .icon("add")
            .disabled(no_host || busy)
            .on_click(move |_, _, cx| {
                creator.update(cx, |v, cx| v.open_dialog(cx));
            }),
    )
}

/// Reader-facing platform name for a creatable loader kind.
fn kind_label(kind: &str) -> String {
    match kind {
        "vanilla" => "Vanilla".into(),
        "fabric" => "Fabric".into(),
        "forge" => "Forge".into(),
        "neoForge" => "NeoForge".into(),
        "quilt" => "Quilt".into(),
        other => other.to_owned(),
    }
}

/// A settings row whose text wraps instead of widening the page (the shared
/// `ui::setting_row` keeps long descriptions on one line, so a long hint would
/// push the whole page past the window).
fn setting(
    t: &AuroraTheme,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    last: bool,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(24.))
        .py(px(14.))
        .when(!last, |this| {
            this.border_b_1().border_color(t.border_subtle)
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.))
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(px(ui::TEXT_BODY + 1.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(t.foreground)
                        .child(title.into()),
                )
                .when_some(description, |this, text| {
                    this.child(
                        div()
                            .text_size(px(ui::TEXT_CAPTION))
                            .text_color(t.muted_foreground)
                            .child(text),
                    )
                }),
        )
}

/// A form field: visible label above its control.
fn field(t: &AuroraTheme, label: &'static str, control: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .min_w(px(0.))
        .child(
            div()
                .text_size(px(ui::TEXT_CAPTION))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(t.muted_foreground)
                .child(label),
        )
        .child(control)
}

/// A control-shaped placeholder shown while a catalog loads.
fn placeholder(t: &AuroraTheme, text: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .w_full()
        .h(px(34.))
        .px(px(12.))
        .rounded(t.radius)
        .bg(t.sunken)
        .border_1()
        .border_color(t.border_subtle)
        .text_size(px(ui::TEXT_BODY))
        .text_color(t.subtle_foreground)
        .child(text)
}

impl Render for InstanceCreator {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        // Design screenshots: `AURORA_SHOWCASE_CREATOR=1` opens the dialog.
        if crate::showcase()
            && !self.showcase_seeded
            && std::env::var("AURORA_SHOWCASE_CREATOR").is_ok_and(|v| v == "1")
        {
            self.showcase_seeded = true;
            self.open_dialog(cx);
        }
        let busy = self.busy;
        self.name
            .update(cx, |state, cx| state.set_disabled(busy, cx));
        let t = crate::theme::theme(cx).clone();
        // Out of flow: the closed creator takes no space on the page. Only a
        // result message (created, failed) shows as a banner.
        let mut root = if self.open || self.message.is_empty() {
            div().absolute().size_0()
        } else {
            div()
                .flex()
                .items_start()
                .gap(px(8.))
                .px(px(12.))
                .py(px(10.))
                .rounded(t.radius)
                .bg(t.card)
                .border_1()
                .border_color(t.border_subtle)
                .child(crate::symbols::icon(Some("info"), px(16.), t.ring))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .max_w(px(720.))
                        .text_size(px(ui::TEXT_BODY))
                        .text_color(t.foreground)
                        .child(self.message.clone()),
                )
        };
        if !self.open {
            return root.into_any_element();
        }
        let snapshots_view = view.clone();
        let mut panel = div()
            .w(px(520.))
            .p(px(24.))
            .rounded(t.radius_lg)
            .bg(t.raised)
            .border_1()
            .border_color(t.border_subtle)
            .text_color(t.foreground)
            .flex()
            .flex_col()
            .gap(px(20.))
            .tab_group()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(ui::icon_tile(&t, Some("add"), 40., true))
                    .child(ui::section_header(
                        &t,
                        "New instance",
                        Some("Pick a name, a game version and a platform.".into()),
                    )),
            )
            .child(field(
                &t,
                "Instance name",
                AuroraInput::new(&self.name).label("Instance name"),
            ));
        let options: [Vec<SharedString>; 3] = [
            self.versions
                .iter()
                .map(|v| {
                    if v.version_type == "snapshot" {
                        format!("{} (snapshot)", v.id).into()
                    } else {
                        v.id.clone().into()
                    }
                })
                .collect(),
            self.kinds.iter().map(|k| kind_label(k).into()).collect(),
            std::iter::once(SharedString::from("Automatic compatible version"))
                .chain(self.loaders.iter().flatten().map(|v| {
                    if v.stable {
                        v.version.clone().into()
                    } else {
                        format!("{} (unstable)", v.version).into()
                    }
                }))
                .collect(),
        ];
        let selected = [
            self.minecraft
                .as_ref()
                .and_then(|m| self.versions.iter().position(|v| &v.id == m)),
            self.kind
                .as_ref()
                .and_then(|k| self.kinds.iter().position(|v| v == k)),
            if self.pin.is_none() {
                Some(0)
            } else {
                self.loaders
                    .as_ref()
                    .and_then(|v| v.iter().position(|v| Some(&v.version) == self.pin.as_ref()))
                    .map(|i| i + 1)
            },
        ];
        let mut controls: Vec<Option<gpui::AnyElement>> = vec![None, None, None];
        for (ix, label, id) in [
            (0, "Minecraft version", "creator-minecraft"),
            (1, "Platform", "creator-platform"),
            (2, "Loader policy", "creator-loader"),
        ] {
            if ix == 2 && self.kind.as_deref() == Some("vanilla") {
                continue;
            }
            if busy || !self.catalog_ready || (ix == 2 && self.loaders.is_none()) {
                controls[ix] = Some(
                    placeholder(
                        &t,
                        if busy {
                            "Unavailable while creating"
                        } else {
                            "Loading…"
                        },
                    )
                    .into_any_element(),
                );
                continue;
            }
            let s = &self.selects[ix];
            let open_view = view.clone();
            let highlight_view = view.clone();
            let confirm_view = view.clone();
            controls[ix] = Some(
                AuroraSelect::new(
                    id,
                    label,
                    options[ix].clone(),
                    &s.trigger,
                    &s.content,
                    s.bounds.clone(),
                )
                .selected(selected[ix])
                .open(s.open)
                .highlight(s.highlight)
                .on_open_change(move |open, _, cx| {
                    open_view.update(cx, |v, cx| {
                        v.selects[ix].open = open;
                        cx.notify();
                    });
                })
                .on_highlight(move |highlight, _, cx| {
                    highlight_view.update(cx, |v, cx| {
                        v.selects[ix].highlight = highlight;
                        cx.notify();
                    });
                })
                .on_confirm(move |selected, _, cx| {
                    confirm_view.update(cx, |v, cx| v.select(ix, selected, cx));
                })
                .into_any_element(),
            );
        }
        let loader_control = controls.pop().flatten();
        let platform_control = controls
            .pop()
            .flatten()
            .unwrap_or_else(|| div().into_any_element());
        let version_control = controls
            .pop()
            .flatten()
            .unwrap_or_else(|| div().into_any_element());
        panel = panel
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(field(&t, "Minecraft version", version_control).flex_1())
                    .child(field(&t, "Platform", platform_control).flex_1()),
            )
            .when_some(loader_control, |this, control| {
                this.child(field(&t, "Loader policy", control))
            });
        if self.kind.as_deref() != Some("vanilla")
            && self.loaders.as_ref().is_some_and(|v| v.is_empty())
        {
            panel = panel.child(
                div()
                    .text_size(px(ui::TEXT_CAPTION))
                    .text_color(t.warning)
                    .child("No supported loader versions for this Minecraft version."),
            );
        }
        let aurora_view = view.clone();
        let aurora_note = if self.compatibility.is_none() && self.note.is_empty() {
            "Checking Aurora compatibility…".to_string()
        } else {
            self.note.clone()
        };
        panel = panel.child(
            div()
                .flex()
                .flex_col()
                .px(px(16.))
                .rounded(t.radius)
                .bg(t.sunken)
                .border_1()
                .border_color(t.border_subtle)
                .child(
                    setting(
                        &t,
                        "Include snapshots",
                        Some("Also list snapshot builds of Minecraft.".into()),
                        false,
                    )
                    .child(
                        AuroraToggle::new("creator-snapshots")
                            .label("Include snapshots")
                            .checked(self.snapshots)
                            .disabled(busy)
                            .on_change(move |checked, _, _, cx| {
                                snapshots_view.update(cx, |v, cx| {
                                    if v.snapshots != checked {
                                        v.snapshots = checked;
                                        v.message.clear();
                                        v.refresh(cx);
                                    }
                                });
                            }),
                    ),
                )
                .child(
                    setting(&t, "Install Aurora", Some(aurora_note.into()), true).child(
                        AuroraToggle::new("creator-aurora")
                            .label("Install Aurora")
                            .checked(self.aurora)
                            .disabled(
                                busy || (!self.aurora
                                    && !self.compatibility.as_ref().is_some_and(|c| c.available)),
                            )
                            .on_change(move |checked, _, _, cx| {
                                aurora_view.update(cx, |v, cx| {
                                    v.aurora = checked;
                                    v.message.clear();
                                    cx.notify();
                                });
                            }),
                    ),
                ),
        );
        if !self.message.is_empty() {
            panel = panel.child(
                div()
                    .text_size(px(ui::TEXT_BODY))
                    .text_color(t.muted_foreground)
                    .child(self.message.clone()),
            );
        }
        let retry_view = view.clone();
        let cancel_view = view.clone();
        let create_view = view.clone();
        panel = panel.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    AuroraButton::new("creator-refresh", "Refresh catalogs")
                        .variant(ButtonVariant::Ghost)
                        .icon("refresh")
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            retry_view.update(cx, |v, cx| v.refresh(cx));
                        }),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            AuroraButton::new("creator-back", "Back")
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    cancel_view.update(cx, |v, cx| {
                                        v.open = false;
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            AuroraButton::new("creator-confirm", "Create instance")
                                .variant(ButtonVariant::Primary)
                                .disabled(
                                    busy || !self.catalog_ready
                                        || !aurora_ready(self.aurora, self.compatibility.as_ref()),
                                )
                                .on_click(move |_, _, cx| {
                                    create_view.update(cx, |v, cx| v.create(cx));
                                }),
                        ),
                ),
        );
        let changed = view.clone();
        let veto = view.clone();
        root = root.child(
            Dialog::new(cx)
                .open(self.open)
                .accessibility_label("New instance")
                .close_on_backdrop_press(false)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !veto.read(cx).busy)
                .on_open_change(move |open, _, _, cx| {
                    changed.update(cx, |v, cx| {
                        v.open = open;
                        cx.notify();
                    });
                })
                .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                .popup(panel),
        );
        root.into_any_element()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn version(id: &str) -> api::MinecraftVersion {
        api::MinecraftVersion {
            id: id.into(),
            version_type: "release".into(),
        }
    }
    #[test]
    fn exact_loader_policy_and_selection() {
        let mut c = Choice {
            minecraft: "a".into(),
            kind: "fabric".into(),
            pin: Some("x".into()),
        };
        let versions = [version("a")];
        let kinds = vec!["fabric".into()];
        let loaders = [api::LoaderVersion {
            version: "x".into(),
            stable: true,
        }];
        assert!(valid_choice(&c, &versions, &kinds, Some(&loaders)));
        assert_eq!(
            loader(&c),
            serde_json::json!({"kind":"fabric","policy":{"type":"pinned","version":"x"}})
        );
        c.pin = Some("old".into());
        assert!(!valid_choice(&c, &versions, &kinds, Some(&loaders)));
        c.pin = None;
        assert_eq!(loader(&c)["policy"]["type"], "automatic");
        assert!(!valid_choice(&c, &versions, &kinds, None));
        c.kind = "vanilla".into();
        assert_eq!(loader(&c), serde_json::json!({"kind":"vanilla"}));
    }
    #[test]
    fn stale_choice_is_distinct() {
        let a = Choice {
            minecraft: "a".into(),
            kind: "fabric".into(),
            pin: None,
        };
        let mut b = a.clone();
        b.pin = Some("x".into());
        assert_ne!(a, b);
        assert!(!current_choice(2, 2, &a, Some(&b)));
        assert!(!current_choice(1, 2, &a, Some(&a)));
        assert!(!current(1, 2));
        assert!(current_choice(2, 2, &a, Some(&a)));
        b = a.clone();
        b.minecraft = "b".into();
        assert_ne!(a, b);
        b = a.clone();
        b.kind = "forge".into();
        assert_ne!(a, b);
    }
    #[test]
    fn late_published_default_preserves_platform_but_respects_minecraft_intent() {
        let versions = [version("provisional"), version("published")];
        let mut choice = Choice {
            minecraft: "provisional".into(),
            kind: "forge".into(),
            pin: None,
        };
        choice.minecraft = late_minecraft_default(
            Some(&choice.minecraft),
            &versions,
            Some("published"),
            false,
            false,
        )
        .unwrap();
        assert_eq!(choice.minecraft, "published");
        assert_eq!(choice.kind, "forge");
        assert!(
            late_minecraft_default(
                Some("provisional"),
                &versions,
                Some("published"),
                true,
                false
            )
            .is_none()
        );
        assert!(
            late_minecraft_default(
                Some("provisional"),
                &versions,
                Some("published"),
                false,
                true
            )
            .is_none()
        );
        assert!(
            late_minecraft_default(Some("provisional"), &versions, Some("absent"), false, false)
                .is_none()
        );
    }

    #[test]
    fn creation_without_aurora_does_not_require_its_service() {
        assert!(aurora_ready(false, None));
        assert!(!aurora_ready(true, None));
        let mut compatibility = api::AuroraCompatibility {
            available: false,
            reason: "Unavailable".into(),
            version: None,
        };
        assert!(aurora_ready(false, Some(&compatibility)));
        assert!(!aurora_ready(true, Some(&compatibility)));
        compatibility.available = true;
        assert!(aurora_ready(true, Some(&compatibility)));
    }
}
