//! Instance management presentation. All persistence and validation remain in the API.
use crate::{
    api,
    backend::guarded,
    controls::{AuroraButton, AuroraInput, AuroraSelect, AuroraToggle, ButtonSize, ButtonVariant},
    symbols,
    theme::AuroraTheme,
    ui,
};
use aurora_launcher_lib::host::Host;
use gpui::{
    Bounds, Context, Div, Entity, EventEmitter, FocusHandle, Hsla, KeyDownEvent, Pixels, Role,
    ScrollHandle, SharedString, Subscription, Window, div, prelude::*, px, uniform_list,
};
use gpui_kit::base::{
    Button, Dialog,
    input::{InputEvent, InputState},
};
use std::{cell::Cell, collections::BTreeMap, ops::Range, rc::Rc};

pub enum InstanceEditorEvent {
    Changed,
}
impl EventEmitter<InstanceEditorEvent> for InstanceEditor {}

struct SelectState {
    trigger: FocusHandle,
    content: FocusHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    open: bool,
    highlight: usize,
}
impl SelectState {
    fn new(cx: &mut Context<InstanceEditor>) -> Self {
        Self {
            trigger: cx.focus_handle(),
            content: cx.focus_handle(),
            bounds: Rc::new(Cell::new(None)),
            open: false,
            highlight: 0,
        }
    }
}

struct ReviewViewport {
    focus: FocusHandle,
    scroll: ScrollHandle,
}

fn review_body(
    id: &'static str,
    text: String,
    height: Pixels,
    window: &mut Window,
    cx: &mut Context<InstanceEditor>,
) -> impl IntoElement {
    let viewport = window.use_keyed_state(id, cx, |_, cx| ReviewViewport {
        focus: cx.focus_handle(),
        scroll: ScrollHandle::new(),
    });
    let focus = viewport.read(cx).focus.clone();
    let scroll = viewport.read(cx).scroll.clone();
    let ring = crate::theme::theme(cx).ring;
    div()
        .id(id)
        .role(Role::ScrollView)
        .aria_label("Review details")
        .aria_description("Use arrow keys, Page Up, Page Down, Home or End to scroll.")
        .track_focus(&focus)
        .tab_stop(true)
        .min_h_0()
        .max_h(height)
        .overflow_y_scroll()
        .track_scroll(&scroll)
        .border_1()
        .border_color(gpui::transparent_black())
        .focus(move |style| style.border_color(ring))
        .on_click(move |_, window, cx| focus.focus(window, cx))
        .on_key_down(cx.listener(move |_, event: &KeyDownEvent, _, cx| {
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
        .child(text)
}

/// The game identity as edited; `pin: None` is the automatic policy.
#[derive(Clone, Debug, PartialEq, Eq)]
struct GameChoice {
    minecraft: String,
    kind: String,
    pin: Option<String>,
}

fn loader_kind(loader: &serde_json::Value) -> String {
    loader["kind"].as_str().unwrap_or("vanilla").to_owned()
}
fn loader_pin(loader: &serde_json::Value) -> Option<String> {
    if loader["policy"]["type"] == "pinned" {
        loader["policy"]["version"].as_str().map(str::to_owned)
    } else {
        None
    }
}
fn loader_json(kind: &str, pin: Option<&str>) -> serde_json::Value {
    match (kind, pin.filter(|version| !version.is_empty())) {
        ("vanilla", _) => serde_json::json!({"kind":"vanilla"}),
        (kind, Some(version)) => {
            serde_json::json!({"kind":kind,"policy":{"type":"pinned","version":version}})
        }
        (kind, None) => serde_json::json!({"kind":kind,"policy":{"type":"automatic"}}),
    }
}
fn game_choice(configuration: &api::InstanceConfiguration) -> GameChoice {
    GameChoice {
        minecraft: configuration.minecraft_version.clone(),
        kind: loader_kind(&configuration.loader),
        pin: loader_pin(&configuration.loader),
    }
}
/// The loader as edited: the exact persisted JSON unless the user changed it.
fn draft_loader(base: &api::InstanceConfiguration, choice: &GameChoice) -> serde_json::Value {
    let unchanged = choice.minecraft == base.minecraft_version
        && choice.kind == loader_kind(&base.loader)
        && choice.pin == loader_pin(&base.loader);
    if unchanged {
        base.loader.clone()
    } else {
        loader_json(&choice.kind, choice.pin.as_deref())
    }
}

/// The saved configuration differs from the installed game (instanceStatus.ts).
fn needs_install(instance: &api::InstanceSummary) -> bool {
    let configuration = &instance.configuration;
    instance.state == "ready"
        && (configuration.minecraft_version != instance.minecraft_version
            || loader_kind(&configuration.loader) != instance.platform.kind
            || configuration.aurora_enabled != instance.aurora.is_some()
            || loader_pin(&configuration.loader)
                .is_some_and(|pin| Some(&pin) != instance.platform.version.as_ref()))
}
fn platform_label(kind: &str) -> &'static str {
    match kind {
        "vanilla" => "Vanilla",
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoForge" => "NeoForge",
        "quilt" => "Quilt",
        _ => "Other",
    }
}
fn recovery_label(status: &str) -> &'static str {
    match status {
        "checking" => "Checking recovery…",
        "busy" => "Installation busy.",
        "interrupted" => "Installation interrupted.",
        "previousRestored" => "Previous installation restored.",
        "manualRepair" => "Recovery required.",
        _ => "Recovery completed.",
    }
}
fn instance_note(instance: &api::InstanceSummary) -> String {
    if let Some(recovery) = &instance.recovery {
        if recovery.status != "completed" || instance.state != "ready" {
            return format!("{} {}", recovery_label(&recovery.status), recovery.message)
                .trim()
                .to_owned();
        }
    }
    match instance.state.as_str() {
        "ready" if needs_install(instance) => {
            "Saved settings differ from the installed game. Install them to apply the new version or loader."
                .into()
        }
        "ready" => "Saved settings match the installed game.".into(),
        "installing" => "Installation incomplete; install or retry to finish it.".into(),
        other => format!("Instance status: {other}."),
    }
}

fn versions_reply_current(
    serial: u64,
    active: u64,
    snapshots: bool,
    active_snapshots: bool,
) -> bool {
    serial == active && snapshots == active_snapshots
}
fn loaders_reply_current(
    serial: u64,
    active: u64,
    minecraft: &str,
    kind: &str,
    draft: Option<&GameChoice>,
) -> bool {
    serial == active
        && draft.is_some_and(|choice| choice.minecraft == minecraft && choice.kind == kind)
}
fn aurora_reply_current(
    serial: u64,
    active: u64,
    instance: &str,
    installed: bool,
    generation: u64,
    identity: Option<(&str, bool, u64)>,
) -> bool {
    serial == active
        && identity.is_some_and(|(current, installed_now, generation_now)| {
            current == instance && installed_now == installed && generation_now == generation
        })
}
fn recovery_reply_current(
    serial: u64,
    active: u64,
    instance: &str,
    generation: u64,
    identity: Option<(&str, u64)>,
) -> bool {
    serial == active
        && identity.is_some_and(|(current, generation_now)| {
            current == instance && generation_now == generation
        })
}

#[derive(Clone)]
struct AuroraReview {
    instance: String,
    enabled: bool,
    installed: bool,
    generation: u64,
    preview: api::AuroraTransition,
}
fn aurora_apply_allowed(
    review: Option<&AuroraReview>,
    identity: Option<(&str, bool, u64)>,
    busy: bool,
) -> bool {
    let Some(review) = review else {
        return false;
    };
    let Some((instance, installed, generation)) = identity else {
        return false;
    };
    !busy
        && review.instance == instance
        && review.installed == installed
        && review.generation == generation
        && !review.preview.fingerprint.is_empty()
        && review.preview.blockers.is_empty()
}
fn aurora_review_text(review: &AuroraReview) -> String {
    let mut lines = vec![match &review.preview.target.aurora {
        Some(pin) => format!("Target: Aurora Client {} ({}).", pin.version, pin.channel),
        None => "Target: Aurora Client removed from this instance.".to_owned(),
    }];
    for requirement in &review.preview.requirements_added {
        lines.push(format!("Add: {requirement}"));
    }
    for requirement in &review.preview.requirements_removed {
        lines.push(format!("Remove: {requirement}"));
    }
    for warning in &review.preview.warnings {
        lines.push(format!("Warning: {warning}"));
    }
    for blocker in &review.preview.blockers {
        lines.push(format!("Blocked: {blocker}"));
    }
    lines.join("\n")
}

#[derive(Clone)]
struct RecoveryReview {
    instance: String,
    generation: u64,
    preview: api::RecoveryPreview,
}
fn recovery_apply_allowed(
    review: Option<&RecoveryReview>,
    instance: Option<&str>,
    generation: u64,
    keep_removed: bool,
    busy: bool,
) -> bool {
    let Some(review) = review else {
        return false;
    };
    let Some(current) = instance else {
        return false;
    };
    !busy
        && review.instance == current
        && review.generation == generation
        && review.preview.instance_id == current
        && review.preview.can_apply
        && !review.preview.fingerprint.is_empty()
        && (review.preview.removed_mods.is_empty() || keep_removed)
}
fn recovery_text(preview: &api::RecoveryPreview) -> String {
    let mut lines = vec![preview.summary.clone()];
    if !preview.kept_settings.is_empty() {
        lines.push(format!(
            "Preserve local settings:\n{}",
            preview.kept_settings.join("\n")
        ));
    }
    if !preview.removed_mods.is_empty() {
        lines.push(format!(
            "Removed mods (kept removed only if you confirm below):\n{}",
            preview.removed_mods.join("\n")
        ));
    }
    if !preview.blockers.is_empty() {
        lines.push(format!("Needs attention:\n{}", preview.blockers.join("\n")));
    }
    lines.join("\n\n")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mutation {
    Select,
    Rename,
    Save,
    Delete,
    Install,
    Retry,
}

/// Only an explicit configuration save adopts a new expected configuration.
/// Rename and installation acknowledgments preserve the user's complete draft.
fn accept_mutation_configuration(
    kind: Mutation,
    configuration: &api::InstanceConfiguration,
    baseline: &mut Option<api::InstanceConfiguration>,
    game: &mut Option<GameChoice>,
) -> Option<api::InstanceConfiguration> {
    if kind != Mutation::Save {
        return None;
    }
    *baseline = Some(configuration.clone());
    *game = Some(game_choice(configuration));
    Some(configuration.clone())
}

struct LoaderCatalog {
    minecraft: String,
    kind: String,
    versions: Vec<api::LoaderVersion>,
}

impl LoaderCatalog {
    fn for_game(&self, game: Option<&GameChoice>) -> &[api::LoaderVersion] {
        if game.is_some_and(|game| game.minecraft == self.minecraft && game.kind == self.kind) {
            &self.versions
        } else {
            &[]
        }
    }
}

/// Read-only per-instance replies (memory advice, managed Java status) belong
/// to the request that is still the latest for the instance still open.
fn read_reply_current(serial: u64, active: u64, instance: &str, selected: Option<&str>) -> bool {
    serial == active && selected == Some(instance)
}

/// The memory control's hint, worded as the supported host words it.
fn memory_hint_text(advice: &api::MemoryAdvice) -> String {
    match advice.recommended_mib {
        Some(mib) => format!(
            "Recommended: {:.1} GB. {}",
            mib as f32 / 1024.0,
            advice.explanation
        ),
        None => advice.explanation.clone(),
    }
}

/// The managed Java runtime row, worded as the supported host words it.
fn runtime_status_text(result: &api::ApiResult<api::RuntimeStatus>) -> String {
    match result {
        Ok(status) => format!(
            "{} (Java {}) · {}",
            status.component,
            status.required_major_version,
            match status.status.as_str() {
                "ready" if status.component == "Custom Java" => "checked and compatible".to_owned(),
                "ready" => "installed and verified".to_owned(),
                "missing" => "not installed yet; installed automatically when needed".to_owned(),
                other => other.to_owned(),
            }
        ),
        Err(error) => error.message.clone(),
    }
}

/// The stored icon name for an instance (`None`: the default icon).
fn icon_for<'a>(icons: &'a BTreeMap<String, String>, instance: &str) -> Option<&'a str> {
    icons.get(instance).map(String::as_str)
}

/// What saving the picker's choice means right now.
#[derive(Debug, PartialEq, Eq)]
enum IconSave {
    /// Another operation is running or nothing is open: do nothing.
    Blocked,
    /// The choice is what is stored already: only close the picker.
    Unchanged,
    Persist {
        instance: String,
        icon: Option<String>,
    },
}

fn icon_save(
    busy: bool,
    selected: Option<&str>,
    stored: Option<&str>,
    choice: Option<&str>,
) -> IconSave {
    match selected {
        Some(instance) if !busy => {
            if stored == choice {
                IconSave::Unchanged
            } else {
                IconSave::Persist {
                    instance: instance.to_owned(),
                    icon: choice.map(str::to_owned),
                }
            }
        }
        _ => IconSave::Blocked,
    }
}

/// A search result applies only to the latest request while the picker is open.
fn icons_reply_current(serial: u64, active: u64, open: bool) -> bool {
    open && serial == active
}

enum Reply {
    State(u64, api::ApiResult<api::LauncherState>),
    Mutation(Mutation, api::ApiResult<Option<api::InstanceSummary>>),
    Folder(api::ApiResult<()>),
    Validation(api::ApiResult<api::InstanceValidation>),
    Advice(u64, String, api::ApiResult<api::MemoryAdvice>),
    Runtime(u64, String, api::ApiResult<api::RuntimeStatus>),
    Icons(u64, String, Vec<symbols::Symbol>),
    IconSaved(api::ApiResult<BTreeMap<String, String>>),
    Java(String, api::ApiResult<api::JavaReport>),
    Versions(u64, bool, api::ApiResult<Vec<api::MinecraftVersion>>),
    Loaders(u64, String, String, api::ApiResult<Vec<api::LoaderVersion>>),
    AuroraPreview(
        u64,
        String,
        bool,
        bool,
        u64,
        api::ApiResult<api::AuroraTransition>,
    ),
    AuroraApplied(api::ApiResult<api::InstanceSummary>),
    RecoveryPreview(u64, String, u64, api::ApiResult<api::RecoveryPreview>),
    RecoveryApplied(api::ApiResult<api::RecoveryPreview>),
}

/// The last registry view of the selected instance; a change bumps the
/// generation that freezes reviews to their target.
#[derive(Clone, PartialEq)]
struct Observed {
    configuration: api::InstanceConfiguration,
    aurora: Option<api::AuroraPin>,
    state: String,
}
impl Observed {
    fn of(instance: &api::InstanceSummary) -> Self {
        Self {
            configuration: instance.configuration.clone(),
            aurora: instance.aurora.clone(),
            state: instance.state.clone(),
        }
    }
}

pub struct InstanceEditor {
    section: usize,
    memory_details: bool,
    host: Option<Host>,
    runtime: tokio::runtime::Handle,
    tx: async_channel::Sender<Reply>,
    serial: u64,
    generation: u64,
    busy: bool,
    state: Option<api::LauncherState>,
    selected: Option<String>,
    baseline: Option<api::InstanceConfiguration>,
    observed: Option<Observed>,
    name: Entity<InputState>,
    memory: Entity<InputState>,
    arguments: Entity<InputState>,
    width: Entity<InputState>,
    height: Entity<InputState>,
    java: Entity<InputState>,
    confirm: Entity<InputState>,
    deleting: bool,
    message: String,
    acknowledged: Option<(Option<api::InstanceConfiguration>, Option<String>)>,
    snapshots: bool,
    versions: Vec<api::MinecraftVersion>,
    catalog_serial: u64,
    catalog_loading: bool,
    kinds: Vec<String>,
    game: Option<GameChoice>,
    loaders: Option<LoaderCatalog>,
    loader_serial: u64,
    loader_loading: bool,
    aurora_serial: u64,
    aurora_review: Option<AuroraReview>,
    aurora_open: bool,
    recovery_serial: u64,
    recovery_review: Option<RecoveryReview>,
    recovery_open: bool,
    recovery_keep: bool,
    /// Memory advice and managed Java status of the open instance; each read
    /// carries a serial so only the latest request for it can be shown.
    advice_serial: u64,
    memory_hint: Option<String>,
    runtime_serial: u64,
    runtime_text: Option<String>,
    /// The icon picker: bounded, off-thread search over the bundled symbols.
    icon_query: Entity<InputState>,
    icon_open: bool,
    icon_serial: u64,
    icon_category: usize,
    icon_choice: Option<String>,
    icon_results: Vec<symbols::Symbol>,
    icon_summary: String,
    _icon_events: Subscription,
    selects: [SelectState; 4],
    /// Design screenshots only: the sample instance was opened once.
    showcase_seeded: bool,
}
impl InstanceEditor {
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded();
        let mut input = |placeholder: &'static str| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let name = input("Instance name");
        let memory = input("e.g. 4096");
        let arguments = input("-Xmx flags, etc.");
        let width = input("Width");
        let height = input("Height");
        let java = input("Custom Java executable path");
        let confirm = input("Type the instance name");
        let icon_query = input("Search icons: sword, server, tools…");
        let icon_events =
            cx.subscribe(&icon_query, |this: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) && this.icon_open {
                    this.request_icons(cx);
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
        let mut this = Self {
            section: 0,
            memory_details: false,
            host,
            runtime,
            tx,
            serial: 0,
            generation: 0,
            busy: false,
            state: None,
            selected: None,
            baseline: None,
            observed: None,
            name,
            memory,
            arguments,
            width,
            height,
            java,
            confirm,
            deleting: false,
            message: String::new(),
            acknowledged: None,
            snapshots: false,
            versions: Vec::new(),
            catalog_serial: 0,
            catalog_loading: false,
            kinds: Vec::new(),
            game: None,
            loaders: None,
            loader_serial: 0,
            loader_loading: false,
            aurora_serial: 0,
            aurora_review: None,
            aurora_open: false,
            recovery_serial: 0,
            recovery_review: None,
            recovery_open: false,
            recovery_keep: false,
            advice_serial: 0,
            memory_hint: None,
            runtime_serial: 0,
            runtime_text: None,
            icon_query,
            icon_open: false,
            icon_serial: 0,
            icon_category: 0,
            icon_choice: None,
            icon_results: Vec::new(),
            icon_summary: String::new(),
            _icon_events: icon_events,
            selects: [
                SelectState::new(cx),
                SelectState::new(cx),
                SelectState::new(cx),
                SelectState::new(cx),
            ],
            showcase_seeded: false,
        };
        this.refresh(cx);
        this
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            self.message = "Instance management is unavailable in diagnostic mode.".into();
            cx.notify();
            return;
        };
        self.serial += 1;
        let serial = self.serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::launcher_state(host)).await;
            let _ = tx.send(Reply::State(serial, r)).await;
        });
    }
    pub fn on_host_event(
        &mut self,
        name: &str,
        payload: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if name == "launch-state" && matches!(payload["status"].as_str(), Some("exited" | "failed"))
        {
            self.refresh(cx);
        }
    }
    fn selected_instance(&self) -> Option<&api::InstanceSummary> {
        self.state
            .as_ref()?
            .instances
            .iter()
            .find(|instance| Some(&instance.id) == self.selected.as_ref())
    }
    fn aurora_identity(&self) -> Option<(String, bool, u64)> {
        let instance = self.selected_instance()?;
        Some((
            self.selected.clone()?,
            instance.aurora.is_some(),
            self.generation,
        ))
    }
    fn recovery_identity(&self) -> Option<(String, u64)> {
        Some((self.selected.clone()?, self.generation))
    }
    fn invalidate_reviews(&mut self) {
        self.aurora_review = None;
        self.recovery_review = None;
        self.recovery_keep = false;
    }
    fn close_reviews(&mut self) {
        self.aurora_open = false;
        self.recovery_open = false;
        self.invalidate_reviews();
    }
    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::State(serial, result) if serial == self.serial => self.apply_state(result, cx),
            Reply::State(..) => {}
            Reply::Versions(serial, snapshots, result) => {
                if versions_reply_current(serial, self.catalog_serial, snapshots, self.snapshots) {
                    self.catalog_loading = false;
                    match result {
                        Ok(versions) => self.versions = versions,
                        Err(e) => self.message = error(e),
                    }
                }
            }
            Reply::Loaders(serial, minecraft, kind, result) => {
                if loaders_reply_current(
                    serial,
                    self.loader_serial,
                    &minecraft,
                    &kind,
                    self.game.as_ref(),
                ) {
                    self.loader_loading = false;
                    match result {
                        Ok(versions) => {
                            self.loaders = Some(LoaderCatalog {
                                minecraft,
                                kind,
                                versions,
                            })
                        }
                        Err(e) => self.message = error(e),
                    }
                }
            }
            Reply::Mutation(kind, result) => self.finish_mutation(kind, result, cx),
            Reply::Folder(result) => {
                self.busy = false;
                self.message = match result {
                    Ok(()) => "Instance folder opened.".into(),
                    Err(e) => error(e),
                };
            }
            Reply::Validation(result) => {
                self.busy = false;
                self.message = match result {
                    Ok(v) => format!(
                        "{}: {}",
                        v.status,
                        v.problems
                            .iter()
                            .map(|p| format!("{}: {}", p.component, p.reason))
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                    Err(e) => error(e),
                };
            }
            Reply::Advice(serial, instance, result) => {
                if read_reply_current(
                    serial,
                    self.advice_serial,
                    &instance,
                    self.selected.as_deref(),
                ) {
                    // A failed read shows no hint, as in the supported host.
                    self.memory_hint = result.ok().map(|advice| memory_hint_text(&advice));
                }
            }
            Reply::Runtime(serial, instance, result) => {
                if read_reply_current(
                    serial,
                    self.runtime_serial,
                    &instance,
                    self.selected.as_deref(),
                ) {
                    self.runtime_text = Some(runtime_status_text(&result));
                }
            }
            Reply::Icons(serial, query, results) => {
                if icons_reply_current(serial, self.icon_serial, self.icon_open) {
                    self.icon_summary = symbols::summary(results.len(), &query);
                    self.icon_results = results;
                }
            }
            Reply::IconSaved(result) => {
                self.busy = false;
                match result {
                    Ok(icons) => {
                        if let Some(state) = self.state.as_mut() {
                            state.icons = icons;
                        }
                        self.icon_open = false;
                        self.message.clear();
                    }
                    // Keep the picker open so the choice can be retried.
                    Err(e) => self.message = error(e),
                }
                cx.emit(InstanceEditorEvent::Changed);
                self.refresh(cx);
            }
            Reply::Java(path, result) => {
                self.busy = false;
                if self.java.read(cx).value().as_str() != path {
                    self.message = "The Java path changed. Validate the current path again.".into();
                    return;
                }
                self.message = match result {
                    Ok(v) => {
                        if v.valid {
                            format!(
                                "Java validated for major version {}.",
                                v.required_major_version
                            )
                        } else {
                            v.problem
                                .unwrap_or_else(|| "Java validation failed.".into())
                        }
                    }
                    Err(e) => error(e),
                };
            }
            Reply::AuroraPreview(serial, instance, enabled, installed, generation, result) => {
                self.busy = false;
                let identity = self.aurora_identity();
                if aurora_reply_current(
                    serial,
                    self.aurora_serial,
                    &instance,
                    installed,
                    generation,
                    identity.as_ref().map(|(i, s, g)| (i.as_str(), *s, *g)),
                ) {
                    match result {
                        Ok(preview) if preview.instance_id == instance => {
                            self.aurora_review = Some(AuroraReview {
                                instance,
                                enabled,
                                installed,
                                generation,
                                preview,
                            });
                            self.message.clear();
                        }
                        Ok(_) => {
                            self.message = "The Aurora review returned another instance.".into();
                            self.aurora_open = false;
                        }
                        Err(e) => self.message = error(e),
                    }
                } else {
                    self.aurora_open = false;
                }
            }
            Reply::AuroraApplied(result) => {
                self.busy = false;
                self.aurora_review = None;
                match result {
                    Ok(summary) => {
                        self.aurora_open = false;
                        self.message = "Aurora change applied.".into();
                        self.acknowledge(summary, None);
                    }
                    Err(e) => {
                        self.message =
                            format!("{} Review the change again before applying.", error(e));
                    }
                }
                // Failed activation/rollback can retain changed state too.
                self.generation += 1;
                self.invalidate_reviews();
                cx.emit(InstanceEditorEvent::Changed);
                self.refresh(cx);
            }
            Reply::RecoveryPreview(serial, instance, generation, result) => {
                self.busy = false;
                let identity = self.recovery_identity();
                if recovery_reply_current(
                    serial,
                    self.recovery_serial,
                    &instance,
                    generation,
                    identity.as_ref().map(|(i, g)| (i.as_str(), *g)),
                ) {
                    match result {
                        Ok(preview) if preview.instance_id == instance => {
                            self.recovery_review = Some(RecoveryReview {
                                instance,
                                generation,
                                preview,
                            });
                            self.message.clear();
                        }
                        Ok(_) => {
                            self.message = "The recovery review returned another instance.".into();
                        }
                        Err(e) => self.message = error(e),
                    }
                } else {
                    self.recovery_open = false;
                }
            }
            Reply::RecoveryApplied(result) => {
                self.busy = false;
                self.recovery_review = None;
                match result {
                    Ok(preview) => {
                        self.message = format!("Recovery applied. {}", preview.summary);
                    }
                    Err(e) => {
                        self.message = format!("{} Inspect again before retrying.", error(e));
                    }
                }
                self.generation += 1;
                self.invalidate_reviews();
                cx.emit(InstanceEditorEvent::Changed);
                self.refresh(cx);
            }
        }
    }
    /// Applies a state read without ever replacing the user's draft; only the
    /// frozen review generation reacts to acknowledged external changes.
    fn apply_state(&mut self, result: api::ApiResult<api::LauncherState>, _cx: &mut Context<Self>) {
        match result {
            Ok(state) => {
                if let Some(id) = self.selected.clone() {
                    match state.instances.iter().find(|instance| instance.id == id) {
                        None => {
                            // The instance disappeared from the registry: close honestly.
                            self.selected = None;
                            self.baseline = None;
                            self.game = None;
                            self.observed = None;
                            self.kinds.clear();
                            self.loaders = None;
                            self.deleting = false;
                            self.acknowledged = None;
                            self.generation += 1;
                            self.close_reviews();
                            self.forget_reads();
                        }
                        Some(instance) => {
                            let observed = Observed::of(instance);
                            if self.observed.as_ref() != Some(&observed) {
                                self.observed = Some(observed);
                                self.generation += 1;
                                self.close_reviews();
                            }
                        }
                    }
                }
                self.state = Some(state);
            }
            Err(e) => self.message = error(e),
        }
    }
    /// Adopts an acknowledged instance record: new expected baseline, seeded
    /// game choice and (via the next render) refreshed config fields.
    fn acknowledge(&mut self, summary: api::InstanceSummary, name: Option<String>) {
        if Some(&summary.id) != self.selected.as_ref() {
            return;
        }
        let previous = self.game.clone();
        self.baseline = Some(summary.configuration.clone());
        self.game = Some(game_choice(&summary.configuration));
        self.observed = Some(Observed::of(&summary));
        self.acknowledged = Some((Some(summary.configuration), name));
        if previous.as_ref() != self.game.as_ref() {
            self.load_loaders();
        }
    }
    fn finish_mutation(
        &mut self,
        kind: Mutation,
        result: api::ApiResult<Option<api::InstanceSummary>>,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        match result {
            Ok(summary) => {
                let mut message = match kind {
                    Mutation::Select => "Instance selected.".to_owned(),
                    Mutation::Rename => "Instance renamed.".to_owned(),
                    Mutation::Save => "Configuration saved.".to_owned(),
                    Mutation::Delete => "Instance deleted.".to_owned(),
                    Mutation::Install => "Saved configuration installed.".to_owned(),
                    Mutation::Retry => "Installation retried.".to_owned(),
                };
                if kind == Mutation::Delete {
                    self.selected = None;
                    self.baseline = None;
                    self.game = None;
                    self.observed = None;
                    self.kinds.clear();
                    self.loaders = None;
                    self.deleting = false;
                    self.acknowledged = None;
                    self.forget_reads();
                } else if let Some(summary) = summary {
                    if kind == Mutation::Save && needs_install(&summary) {
                        message =
                            "Settings saved. Install them to apply the new game version or loader."
                                .into();
                    }
                    if Some(&summary.id) == self.selected.as_ref() {
                        let previous_game = self.game.clone();
                        let config = accept_mutation_configuration(
                            kind,
                            &summary.configuration,
                            &mut self.baseline,
                            &mut self.game,
                        );
                        let name =
                            (kind == Mutation::Rename).then_some(summary.display_name.clone());
                        if config.is_some() || name.is_some() {
                            self.acknowledged = Some((config, name));
                        }
                        self.observed = Some(Observed::of(&summary));
                        if previous_game != self.game {
                            self.load_loaders();
                        }
                        // An install or repair is what changes the managed
                        // Java runtime; the supported host re-reads it then.
                        if matches!(kind, Mutation::Install | Mutation::Retry) {
                            self.load_runtime();
                        }
                    }
                }
                self.generation += 1;
                self.close_reviews();
                self.message = message;
                cx.emit(InstanceEditorEvent::Changed);
                self.refresh(cx);
            }
            Err(e) => {
                // Core may have retained state; refresh honestly without claiming success.
                self.message = error(e);
                cx.emit(InstanceEditorEvent::Changed);
                self.refresh(cx);
            }
        }
    }
    fn choose(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(instance) = self
            .state
            .as_ref()
            .and_then(|s| s.instances.iter().find(|i| i.id == id))
            .cloned()
        else {
            return;
        };
        self.selected = Some(id);
        self.baseline = Some(instance.configuration.clone());
        self.observed = Some(Observed::of(&instance));
        self.generation += 1;
        self.deleting = false;
        self.close_reviews();
        self.acknowledged = None;
        let kind = loader_kind(&instance.configuration.loader);
        self.kinds = self
            .state
            .as_ref()
            .map(|s| {
                s.platform_capabilities
                    .iter()
                    .filter(|c| c.can_install || c.can_create)
                    .map(|c| c.kind.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !self.kinds.contains(&kind) {
            self.kinds.push(kind.clone());
        }
        self.game = Some(game_choice(&instance.configuration));
        // A different (or reopened) instance: its advice is read once now, and
        // nothing read for the previous one may show.
        self.forget_reads();
        self.icon_open = false;
        self.load_advice();
        let c = instance.configuration;
        for (field, value) in [
            (&self.name, instance.display_name),
            (&self.memory, c.memory_mib.to_string()),
            (&self.arguments, c.additional_jvm_arguments.clone()),
            (
                &self.width,
                c.window.map(|w| w.width.to_string()).unwrap_or_default(),
            ),
            (
                &self.height,
                c.window.map(|w| w.height.to_string()).unwrap_or_default(),
            ),
            (&self.java, c.custom_java().unwrap_or("").to_string()),
            (&self.confirm, String::new()),
        ] {
            field.update(cx, |field, cx| field.set_value(value, window, cx));
        }
        if self.versions.is_empty() {
            self.load_versions();
        }
        self.load_loaders();
        cx.notify();
    }
    /// Drops what was read for the previous target so a late reply, or the old
    /// text, can never describe another instance.
    fn forget_reads(&mut self) {
        self.advice_serial += 1;
        self.runtime_serial += 1;
        self.memory_hint = None;
        self.runtime_text = None;
    }
    /// Memory advice is read when an instance is opened, never per keystroke.
    fn load_advice(&mut self) {
        let (Some(host), Some(id)) = (self.host.clone(), self.selected.clone()) else {
            return;
        };
        self.advice_serial += 1;
        let serial = self.advice_serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::performance_advice(host, id.clone())).await;
            let _ = tx.send(Reply::Advice(serial, id, r)).await;
        });
    }
    /// The managed Java runtime is read when validating and after an install
    /// or repair, the same triggers as the supported host.
    fn load_runtime(&mut self) {
        let (Some(host), Some(id)) = (self.host.clone(), self.selected.clone()) else {
            return;
        };
        self.runtime_serial += 1;
        let serial = self.runtime_serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::runtime_status(host, id.clone())).await;
            let _ = tx.send(Reply::Runtime(serial, id, r)).await;
        });
    }
    fn stored_icon(&self) -> Option<&str> {
        icon_for(&self.state.as_ref()?.icons, self.selected.as_deref()?)
    }
    fn open_icon_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.selected.is_none() {
            return;
        }
        self.icon_choice = self.stored_icon().map(str::to_owned);
        self.icon_category = 0;
        self.icon_open = true;
        self.icon_query
            .update(cx, |field, cx| field.set_value("", window, cx));
        self.request_icons(cx);
        cx.notify();
    }
    /// Searches the bundled symbols off the UI thread; only the latest request
    /// may fill the grid, and the grid never holds more than the bounded set.
    fn request_icons(&mut self, cx: &mut Context<Self>) {
        self.icon_serial += 1;
        let serial = self.icon_serial;
        let query: String = self.icon_query.read(cx).value().chars().take(64).collect();
        let category = self.icon_category;
        let tx = self.tx.clone();
        self.runtime.spawn_blocking(move || {
            let results = symbols::browse(&query, category);
            let _ = tx.send_blocking(Reply::Icons(serial, query, results));
        });
    }
    fn save_icon(&mut self, cx: &mut Context<Self>) {
        let request = icon_save(
            self.busy,
            self.selected.as_deref(),
            self.stored_icon(),
            self.icon_choice.as_deref(),
        );
        match request {
            IconSave::Blocked => {}
            IconSave::Unchanged => {
                self.icon_open = false;
                cx.notify();
            }
            IconSave::Persist { instance, icon } => {
                let Some(host) = self.host.clone() else {
                    return;
                };
                self.run(async move {
                    Reply::IconSaved(guarded(api::set_instance_icon(host, instance, icon)).await)
                });
                cx.notify();
            }
        }
    }
    /// The icon picker: preview, search, category and a virtualized grid over
    /// the bounded result set (only the visible rows are built each frame).
    fn icon_panel(&mut self, width: Pixels, cx: &mut Context<Self>) -> gpui::AnyElement {
        if !self.icon_open {
            return div().into_any_element();
        }
        let this = cx.entity();
        let palette = crate::theme::theme(cx).clone();
        let busy = self.busy;
        let choice = self.icon_choice.clone();
        // Cell (48px) plus gap (4px) across the panel's inner width.
        let columns = ((f32::from(width - px(48.)) / 52.0).floor() as usize).max(1);
        let rows = self.icon_results.len().div_ceil(columns);
        let grid = uniform_list(
            "icon-grid",
            rows,
            cx.processor(move |this: &mut Self, range: Range<usize>, _, cx| {
                let palette = crate::theme::theme(cx).clone();
                let view = cx.entity();
                range
                    .map(|row| {
                        div().flex().gap(px(4.)).h(px(52.)).children(
                            this.icon_results
                                .iter()
                                .skip(row * columns)
                                .take(columns)
                                .map(|symbol| {
                                    let name = symbol.name;
                                    let chosen = this.icon_choice.as_deref() == Some(name);
                                    let view = view.clone();
                                    Button::new(SharedString::from(format!("icon-{name}")))
                                        .accessibility_label(symbols::label(name))
                                        .disabled(busy)
                                        .selected(chosen)
                                        .w(px(48.))
                                        .h(px(48.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(palette.radius)
                                        .border_1()
                                        .border_color(if chosen {
                                            palette.ring
                                        } else {
                                            gpui::transparent_black()
                                        })
                                        .bg(if chosen {
                                            palette.accent_soft
                                        } else {
                                            gpui::transparent_black()
                                        })
                                        .hover(|style| style.bg(palette.control_hover))
                                        .on_click(move |_, _, cx| {
                                            view.update(cx, |v, cx| {
                                                v.icon_choice = Some(name.to_owned());
                                                cx.notify();
                                            });
                                        })
                                        .child(symbols::icon(
                                            Some(name),
                                            px(26.),
                                            if chosen {
                                                palette.ring
                                            } else {
                                                palette.muted_foreground
                                            },
                                        ))
                                }),
                        )
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .h(px(248.))
        .w_full();
        let s = &self.selects[3];
        let open_view = this.clone();
        let highlight_view = this.clone();
        let confirm_view = this.clone();
        let default_view = this.clone();
        let cancel_view = this.clone();
        let save_view = this.clone();
        let mut panel = dialog_surface(&palette, width)
            .child(ui::section_header(
                &palette,
                "Instance icon",
                Some(
                    "Changes only how the instance looks in Aurora. Folders and data stay where they are."
                        .into(),
                ),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(ui::icon_tile(&palette, choice.as_deref(), 44., true))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .flex_1()
                            .child(
                                div()
                                    .text_size(px(ui::TEXT_BODY + 1.))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(symbols::caption(choice.as_deref())),
                            )
                            .child(ui::caption(
                                &palette,
                                if choice.is_none() {
                                    "Default icon"
                                } else {
                                    "Material Symbols"
                                },
                            )),
                    )
                    .child(
                        AuroraButton::new("icon-default", "Use default")
                            .variant(ButtonVariant::Ghost)
                            .disabled(busy || choice.is_none())
                            .on_click(move |_, _, cx| {
                                default_view.update(cx, |v, cx| {
                                    v.icon_choice = None;
                                    cx.notify();
                                });
                            }),
                    ),
            )
            .child(AuroraInput::new(&self.icon_query).label("Search icons"))
            .child(
                AuroraSelect::new(
                    "icon-category",
                    "Icon category",
                    symbols::category_labels(),
                    &s.trigger,
                    &s.content,
                    s.bounds.clone(),
                )
                .selected(Some(self.icon_category))
                .open(s.open)
                .disabled(busy)
                .highlight(s.highlight)
                .on_open_change(move |open, _, cx| {
                    open_view.update(cx, |v, cx| {
                        v.selects[3].open = open;
                        cx.notify();
                    });
                })
                .on_highlight(move |highlight, _, cx| {
                    highlight_view.update(cx, |v, cx| {
                        v.selects[3].highlight = highlight;
                        cx.notify();
                    });
                })
                .on_confirm(move |chosen, _, cx| {
                    confirm_view.update(cx, |v, cx| {
                        if v.icon_category != chosen {
                            v.icon_category = chosen;
                            v.request_icons(cx);
                            cx.notify();
                        }
                    });
                }),
            )
            .child(ui::caption(&palette, self.icon_summary.clone()))
            .child(grid);
        if !self.message.is_empty() {
            panel = panel.child(
                div()
                    .text_size(px(ui::TEXT_BODY))
                    .text_color(palette.danger)
                    .child(self.message.clone()),
            );
        }
        panel
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("icon-cancel", "Cancel")
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                cancel_view.update(cx, |v, cx| {
                                    v.icon_open = false;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        AuroraButton::new("icon-save", "Save")
                            .variant(ButtonVariant::Primary)
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                save_view.update(cx, |v, cx| v.save_icon(cx));
                            }),
                    ),
            )
            .into_any_element()
    }
    /// Official catalogs load only after the user selects an instance; the
    /// editor never requests them at startup.
    fn load_versions(&mut self) {
        if self.host.is_none() {
            return;
        }
        self.catalog_serial += 1;
        self.catalog_loading = true;
        let serial = self.catalog_serial;
        let snapshots = self.snapshots;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::minecraft_versions(snapshots)).await;
            let _ = tx.send(Reply::Versions(serial, snapshots, r)).await;
        });
    }
    fn load_loaders(&mut self) {
        self.loader_serial += 1;
        self.loaders = None;
        let Some(game) = self.game.clone() else {
            self.loaders = None;
            self.loader_loading = false;
            return;
        };
        if game.kind == "vanilla" || self.host.is_none() {
            self.loaders = None;
            self.loader_loading = false;
            return;
        }
        self.loader_loading = true;
        let serial = self.loader_serial;
        let minecraft = game.minecraft.clone();
        let kind = game.kind.clone();
        let request_minecraft = minecraft.clone();
        let request_kind = kind.clone();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::loader_versions(request_kind, request_minecraft)).await;
            let _ = tx.send(Reply::Loaders(serial, minecraft, kind, r)).await;
        });
    }
    fn version_values(&self) -> Vec<String> {
        let mut values: Vec<String> = self.versions.iter().map(|v| v.id.clone()).collect();
        for keep in [
            self.game.as_ref().map(|g| g.minecraft.clone()),
            self.baseline.as_ref().map(|b| b.minecraft_version.clone()),
        ]
        .into_iter()
        .flatten()
        {
            if !values.contains(&keep) {
                values.insert(0, keep);
            }
        }
        values
    }
    fn loader_values(&self) -> Vec<String> {
        let mut values = vec![String::new()];
        if let Some(pin) = self
            .game
            .as_ref()
            .and_then(|g| g.pin.clone())
            .filter(|pin| !pin.is_empty())
        {
            if !self
                .current_loaders()
                .iter()
                .any(|version| version.version == pin)
            {
                values.push(pin);
            }
        }
        for version in self.current_loaders() {
            values.push(version.version.clone());
        }
        values
    }
    fn current_loaders(&self) -> &[api::LoaderVersion] {
        self.loaders
            .as_ref()
            .map(|catalog| catalog.for_game(self.game.as_ref()))
            .unwrap_or_default()
    }
    fn select_game(&mut self, which: usize, ix: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(base) = self.baseline.clone() else {
            return;
        };
        let mut game = self.game.clone().unwrap_or_else(|| game_choice(&base));
        match which {
            0 => {
                let Some(version) = self.version_values().get(ix).cloned() else {
                    return;
                };
                game.minecraft = version;
            }
            1 => {
                let Some(kind) = self.kinds.get(ix).cloned() else {
                    return;
                };
                game.kind = kind;
            }
            2 => {
                let value = self.loader_values().get(ix).cloned().unwrap_or_default();
                game.pin = (!value.is_empty()).then_some(value);
                self.game = Some(game);
                self.message.clear();
                cx.notify();
                return;
            }
            _ => {}
        }
        // A saved pin applies only to the saved version and loader.
        let saved = game_choice(&base);
        game.pin = if game.minecraft == saved.minecraft && game.kind == saved.kind {
            saved.pin
        } else {
            None
        };
        self.game = Some(game);
        self.message.clear();
        self.load_loaders();
        cx.notify();
    }
    fn draft(&self, cx: &gpui::App) -> Option<api::InstanceConfiguration> {
        let base = self.baseline.as_ref()?.clone();
        let game = self.game.clone().unwrap_or_else(|| game_choice(&base));
        edited_config(
            base,
            &game,
            &self.memory.read(cx).value(),
            &self.arguments.read(cx).value(),
            &self.width.read(cx).value(),
            &self.height.read(cx).value(),
            &self.java.read(cx).value(),
        )
        .ok()
    }
    fn draft_matches_base(&self, cx: &gpui::App) -> bool {
        self.draft(cx).as_ref() == self.baseline.as_ref()
    }
    fn run<F>(&mut self, work: F)
    where
        F: std::future::Future<Output = Reply> + Send + 'static,
    {
        if self.busy {
            return;
        }
        self.busy = true;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let reply = work.await;
            let _ = tx.send(reply).await;
        });
    }
    fn java_check_ready(&mut self, cx: &mut Context<Self>) -> bool {
        let path = self.java.read(cx).value().to_string();
        if path.trim().is_empty() {
            self.message = "Enter a custom Java executable path first.".into();
            return false;
        }
        let Some(base) = self.baseline.clone() else {
            return false;
        };
        let game = self.game.clone().unwrap_or_else(|| game_choice(&base));
        if game.minecraft != base.minecraft_version || draft_loader(&base, &game) != base.loader {
            self.message = "Save and install game changes before checking Java.".into();
            return false;
        }
        true
    }
    fn action(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let (Some(host), Some(id)) = (self.host.clone(), self.selected.clone()) else {
            return;
        };
        let Some(instance) = self.selected_instance().cloned() else {
            return;
        };
        let name = self.name.read(cx).value().to_string();
        let confirmation = self.confirm.read(cx).value().to_string();
        let path = self.java.read(cx).value().to_string();
        let config = if action == "save" {
            let Some(base) = self.baseline.clone() else {
                return;
            };
            let game = self.game.clone().unwrap_or_else(|| game_choice(&base));
            match edited_config(
                base.clone(),
                &game,
                &self.memory.read(cx).value(),
                &self.arguments.read(cx).value(),
                &self.width.read(cx).value(),
                &self.height.read(cx).value(),
                &path,
            ) {
                Ok(c) => Some((base, c)),
                Err(e) => {
                    self.message = e;
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        if action == "delete" && (!self.deleting || confirmation != instance.display_name) {
            self.message = "Type the exact instance name to confirm deletion.".into();
            cx.notify();
            return;
        }
        match action {
            "validate" => {
                self.load_runtime();
                self.run(async move {
                    Reply::Validation(guarded(api::validate_instance(host, id)).await)
                });
            }
            "java" => {
                if !self.java_check_ready(cx) {
                    cx.notify();
                    return;
                }
                let checked = path.clone();
                self.run(async move {
                    Reply::Java(
                        checked.clone(),
                        guarded(api::check_custom_java(host, id, checked)).await,
                    )
                });
            }
            "folder" => self.run(async move {
                Reply::Folder(guarded(api::open_instance_folder(host, id)).await)
            }),
            _ => {
                let kind = match action {
                    "select" => Mutation::Select,
                    "rename" => Mutation::Rename,
                    "save" => Mutation::Save,
                    "delete" => Mutation::Delete,
                    "install" => Mutation::Install,
                    "repair" => Mutation::Retry,
                    _ => return,
                };
                self.run(async move {
                    let result = match kind {
                        Mutation::Select => {
                            guarded(api::select_instance(host, id)).await.map(|_| None)
                        }
                        Mutation::Rename => {
                            guarded(api::rename_instance(host, id, instance.display_name, name))
                                .await
                                .map(Some)
                        }
                        Mutation::Save => {
                            let (expected, configuration) = config.unwrap();
                            guarded(api::update_configuration(host, id, expected, configuration))
                                .await
                                .map(Some)
                        }
                        Mutation::Delete => guarded(api::delete_instance(host, id, confirmation))
                            .await
                            .map(|_| None),
                        Mutation::Install => guarded(api::install_configuration(host, id))
                            .await
                            .map(Some),
                        Mutation::Retry => guarded(api::retry_install(host, id)).await.map(Some),
                    };
                    Reply::Mutation(kind, result)
                });
            }
        }
        cx.notify();
    }
    /// Aurora changes are reviewed transactions; this never touches
    /// `configuration.aurora_enabled` directly.
    fn review_aurora(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(instance) = self.selected_instance().cloned() else {
            return;
        };
        if needs_install(&instance) {
            self.message = "Install the saved configuration before changing Aurora.".into();
            cx.notify();
            return;
        }
        if !self.draft_matches_base(cx) {
            self.message = "Save or revert configuration changes before changing Aurora.".into();
            cx.notify();
            return;
        }
        let installed = instance.aurora.is_some();
        let id = instance.id.clone();
        let generation = self.generation;
        self.aurora_serial += 1;
        self.aurora_review = None;
        self.aurora_open = true;
        self.busy = true;
        self.message.clear();
        let serial = self.aurora_serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::preview_aurora(host, id.clone(), enabled)).await;
            let _ = tx
                .send(Reply::AuroraPreview(
                    serial, id, enabled, installed, generation, r,
                ))
                .await;
        });
        cx.notify();
    }
    fn apply_aurora_transition(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let identity = self.aurora_identity();
        if !aurora_apply_allowed(
            self.aurora_review.as_ref(),
            identity.as_ref().map(|(i, s, g)| (i.as_str(), *s, *g)),
            self.busy,
        ) {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(review) = self.aurora_review.clone() else {
            return;
        };
        self.busy = true;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::apply_aurora(
                host,
                review.instance.clone(),
                review.enabled,
                review.preview.fingerprint.clone(),
            ))
            .await;
            let _ = tx.send(Reply::AuroraApplied(r)).await;
        });
        cx.notify();
    }
    /// Home's "Review recovery" hand-off: opens the editor's own recovery
    /// review for `instance_id` when that instance is the one shown. Returns
    /// whether the editor took the request; the same draft and busy guards as
    /// the editor's Inspect recovery button apply.
    pub fn review_recovery(&mut self, instance_id: &str, cx: &mut Context<Self>) -> bool {
        if self.selected.as_deref() != Some(instance_id) || self.host.is_none() {
            return false;
        }
        self.inspect_recovery(cx);
        true
    }

    fn inspect_recovery(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(id) = self.selected.clone() else {
            return;
        };
        if !self.draft_matches_base(cx) {
            self.message = "Save or revert configuration changes before recovery.".into();
            cx.notify();
            return;
        }
        let generation = self.generation;
        self.recovery_serial += 1;
        self.recovery_review = None;
        self.recovery_keep = false;
        self.recovery_open = true;
        self.busy = true;
        self.message.clear();
        let serial = self.recovery_serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::inspect_recovery(host, id.clone())).await;
            let _ = tx
                .send(Reply::RecoveryPreview(serial, id, generation, r))
                .await;
        });
        cx.notify();
    }
    fn apply_recovery_now(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if !recovery_apply_allowed(
            self.recovery_review.as_ref(),
            self.selected.as_deref(),
            self.generation,
            self.recovery_keep,
            self.busy,
        ) {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(review) = self.recovery_review.clone() else {
            return;
        };
        let keep = self.recovery_keep;
        self.busy = true;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::apply_recovery(
                host,
                review.instance.clone(),
                review.preview.fingerprint.clone(),
                keep,
            ))
            .await;
            let _ = tx.send(Reply::RecoveryApplied(r)).await;
        });
        cx.notify();
    }
}
fn error(e: api::ApiError) -> String {
    format!("{}: {}", e.code, e.message)
}
fn edited_config(
    base: api::InstanceConfiguration,
    game: &GameChoice,
    memory: &str,
    args: &str,
    width: &str,
    height: &str,
    java: &str,
) -> Result<api::InstanceConfiguration, String> {
    let mut c = base.clone();
    c.minecraft_version = game.minecraft.clone();
    c.loader = draft_loader(&base, game);
    c.memory_mib = memory
        .trim()
        .parse()
        .map_err(|_| "Enter memory as a whole number in MiB.".to_string())?;
    c.additional_jvm_arguments = args.to_string();
    c.window = if width.trim().is_empty() && height.trim().is_empty() {
        None
    } else {
        Some(api::WindowSize {
            width: width
                .trim()
                .parse()
                .map_err(|_| "Enter a valid window width.".to_string())?,
            height: height
                .trim()
                .parse()
                .map_err(|_| "Enter a valid window height.".to_string())?,
        })
    };
    // Preserve the exact persisted Java JSON unless the user explicitly
    // replaces the path or clears the custom selection.
    if !java.trim().is_empty() {
        if base.custom_java() == Some(java.trim()) {
            c.java = base.java.clone();
        } else {
            c.java = Some(serde_json::json!({"kind":"custom", "path":java.trim()}));
        }
    } else if base.custom_java().is_some() {
        c.java = None;
    }
    Ok(c)
}
/// Control widths in settings rows. `AuroraSelect` is a fixed 220px wide, so
/// short fields match it and long text fields get a wider column; every
/// control is right-aligned to the same edge.
const SHORT_W: f32 = 240.;
const WIDE_W: f32 = 240.;
/// Widest a stacked (full-width) text field grows.
const STACK_W: f32 = 420.;
/// The instance list column of the master-detail layout.
const LIST_W: f32 = 272.;
/// Sidebar plus the page gutters, subtracted from the window width.
const PAGE_CHROME: f32 = 276.;
/// Distance from the window top to the master-detail row (page header, import
/// hint and gaps) plus the page's bottom padding. The row takes the rest of the
/// viewport so each column scrolls on its own and no void opens under the list.
const ROW_TOP_CHROME: f32 = 164. + 40.;
/// The master-detail row never gets shorter than this.
const ROW_MIN_H: f32 = 420.;
/// Unwrapped text reports its full line as its minimum width, so free text is
/// capped: it wraps here instead of widening the whole page.
const TEXT_MAX: f32 = 420.;

/// A card header: section title (and caption) left, an optional action right.
fn card_head(
    t: &AuroraTheme,
    title: &'static str,
    caption: Option<&'static str>,
    action: Option<gpui::AnyElement>,
) -> Div {
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap(px(16.))
        .child(ui::section_header(
            t,
            title,
            caption.map(SharedString::from),
        ))
        .when_some(action, |this, action| this.child(action))
}

/// A setting's title and description. Text wraps instead of widening the page
/// (the shared `ui::setting_row` keeps long descriptions on one line, so a long
/// hint would push the whole page past the window).
fn label_block(
    t: &AuroraTheme,
    title: impl Into<SharedString>,
    description: Option<SharedString>,
) -> Div {
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
                    .max_w(px(TEXT_MAX))
                    .text_size(px(ui::TEXT_CAPTION))
                    .text_color(t.muted_foreground)
                    .child(text),
            )
        })
}

/// A settings row: label left, control right.
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
        .gap(px(20.))
        .py(px(10.))
        .when(!last, |this| {
            this.border_b_1().border_color(t.border_subtle)
        })
        .child(label_block(t, title, description))
}

/// A name with its detail line, wrapping rather than widening the page.
fn name_block(t: &AuroraTheme, title: String, detail: String) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .min_w(px(0.))
        .max_w(px(TEXT_MAX))
        .child(
            div()
                .text_size(px(ui::TEXT_BODY + 1.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(t.foreground)
                .child(title),
        )
        .child(
            div()
                .text_size(px(ui::TEXT_CAPTION))
                .text_color(t.muted_foreground)
                .child(detail),
        )
}

/// Settings rows with a fixed-width control column and correct hairlines.
fn rows(t: &AuroraTheme, items: Vec<(&'static str, Option<String>, gpui::AnyElement)>) -> Div {
    rows_with(
        t,
        items
            .into_iter()
            .map(|(title, description, control)| (title, description, control, false))
            .collect(),
    )
}

/// Like [`rows`]; a `true` flag stacks that row's control full-width beneath
/// its label (long text fields in a narrow column).
fn rows_with(
    t: &AuroraTheme,
    items: Vec<(&'static str, Option<String>, gpui::AnyElement, bool)>,
) -> Div {
    let count = items.len();
    div()
        .flex()
        .flex_col()
        .children(items.into_iter().enumerate().map(
            |(ix, (title, description, control, stacked))| {
                let last = ix + 1 == count;
                let description = description.map(SharedString::from);
                if stacked {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .py(px(10.))
                        .when(!last, |this| {
                            this.border_b_1().border_color(t.border_subtle)
                        })
                        .child(label_block(t, title, description))
                        .child(div().w_full().child(control))
                } else {
                    setting(t, title, description, last).child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_end()
                            .gap(px(8.))
                            .child(control),
                    )
                }
            },
        ))
}

/// A form card: header on top, rows beneath.
fn form_card(t: &AuroraTheme, head: Div, body: Div) -> Div {
    ui::card(t).p(px(16.)).gap(px(4.)).child(head).child(body)
}

/// A control-shaped, non-interactive box (loading and locked values).
fn value_box(t: &AuroraTheme, text: impl Into<SharedString>, icon: Option<&'static str>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .w(px(SHORT_W))
        .h(px(34.))
        .px(px(12.))
        .rounded(t.radius)
        .bg(t.sunken)
        .border_1()
        .border_color(t.border_subtle)
        .text_size(px(ui::TEXT_BODY))
        .text_color(t.muted_foreground)
        .when_some(icon, |this, icon| {
            this.child(symbols::icon(Some(icon), px(14.), t.subtle_foreground))
        })
        .child(text.into())
}

/// A tinted notice with a leading symbol.
fn callout(t: &AuroraTheme, icon: &'static str, tone: Hsla, wash: bool, text: String) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(10.))
        .px(px(12.))
        .py(px(10.))
        .rounded(t.radius)
        .bg(if wash {
            Hsla { a: 0.10, ..tone }
        } else {
            t.control
        })
        .border_1()
        .border_color(if wash {
            Hsla { a: 0.26, ..tone }
        } else {
            t.border_subtle
        })
        .child(symbols::icon(Some(icon), px(16.), tone))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .max_w(px(720.))
                .text_size(px(ui::TEXT_BODY))
                .text_color(t.foreground)
                .child(text),
        )
}

/// A quiet explanatory note under a group of rows.
fn hint_note(t: &AuroraTheme, text: String) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(10.))
        .px(px(12.))
        .py(px(10.))
        .rounded(t.radius)
        .bg(t.control)
        .border_1()
        .border_color(t.border_subtle)
        .child(symbols::icon(Some("info"), px(15.), t.subtle_foreground))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .max_w(px(720.))
                .text_size(px(ui::TEXT_CAPTION))
                .text_color(t.muted_foreground)
                .child(text),
        )
}

/// Whether an instance needs the user's attention (install or recovery).
fn needs_attention(instance: &api::InstanceSummary) -> bool {
    needs_install(instance)
        || instance.state != "ready"
        || instance
            .recovery
            .as_ref()
            .is_some_and(|recovery| recovery.status != "completed")
}

/// Status pills for an instance: active, loader extras and install state.
fn instance_badges(
    t: &AuroraTheme,
    instance: &api::InstanceSummary,
    active: bool,
    compact: bool,
) -> Option<Div> {
    let mut badges = div().flex().flex_shrink_0().items_center().gap(px(6.));
    let mut any = false;
    if active {
        any = true;
        badges = badges.child(ui::status_badge(t, "Active", ui::Tone::Accent));
    }
    if !compact {
        if let Some(aurora) = &instance.aurora {
            any = true;
            badges = badges.child(ui::badge(
                t,
                format!("Aurora {}", aurora.version),
                ui::Tone::Accent,
            ));
        }
        if let Some(pack) = &instance.pack {
            any = true;
            badges = badges.child(ui::badge(
                t,
                format!("{} {}", pack.name, pack.pack_version),
                ui::Tone::Neutral,
            ));
        }
    }
    if instance
        .recovery
        .as_ref()
        .is_some_and(|recovery| recovery.status != "completed")
    {
        any = true;
        badges = badges.child(ui::badge(t, "Recovery needed", ui::Tone::Warning));
    } else if instance.state == "installing" {
        any = true;
        badges = badges.child(ui::badge(t, "Install incomplete", ui::Tone::Warning));
    } else if needs_install(instance) {
        any = true;
        badges = badges.child(ui::badge(t, "Update pending", ui::Tone::Warning));
    } else if instance.state != "ready" {
        any = true;
        badges = badges.child(ui::badge(t, instance.state.clone(), ui::Tone::Warning));
    }
    any.then_some(badges)
}

/// The surface of a dialog popup.
fn dialog_surface(t: &AuroraTheme, width: Pixels) -> Div {
    div()
        .w(width)
        .p(px(24.))
        .rounded(t.radius_lg)
        .bg(t.raised)
        .border_1()
        .border_color(t.border_subtle)
        .text_color(t.foreground)
        .flex()
        .flex_col()
        .gap(px(16.))
}

impl Render for InstanceEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let busy = self.busy;
        if let Some((config, name)) = self.acknowledged.take() {
            if let Some(config) = config {
                for (field, value) in [
                    (&self.memory, config.memory_mib.to_string()),
                    (
                        &self.width,
                        config
                            .window
                            .map(|w| w.width.to_string())
                            .unwrap_or_default(),
                    ),
                    (
                        &self.height,
                        config
                            .window
                            .map(|w| w.height.to_string())
                            .unwrap_or_default(),
                    ),
                    (&self.java, config.custom_java().unwrap_or("").to_string()),
                    (&self.arguments, config.additional_jvm_arguments.clone()),
                ] {
                    field.update(cx, |field, cx| field.set_value(value, window, cx));
                }
            }
            if let Some(name) = name {
                self.name
                    .update(cx, |field, cx| field.set_value(name, window, cx));
            }
        }
        // Design screenshots run against disposable data: open one instance so
        // the page shows its editor. `AURORA_SHOWCASE_INSTANCE=<index>|none`.
        if crate::showcase() && !self.showcase_seeded {
            if let Some(state) = &self.state {
                self.showcase_seeded = true;
                let wanted = std::env::var("AURORA_SHOWCASE_INSTANCE").unwrap_or_default();
                if wanted != "none" {
                    let index = wanted.parse::<usize>().unwrap_or(0);
                    if let Some(id) = state.instances.get(index).map(|i| i.id.clone()) {
                        self.choose(id, window, cx);
                    }
                }
            }
        }
        for field in [
            &self.name,
            &self.memory,
            &self.arguments,
            &self.width,
            &self.height,
            &self.java,
            &self.confirm,
        ] {
            field.update(cx, |field, cx| field.set_disabled(busy, cx));
        }
        let draft_clean = self.draft_matches_base(cx);
        let t = crate::theme::theme(cx).clone();
        // The page column sizes itself to its widest content, so give the
        // master-detail a definite width: the window minus the sidebar and the
        // page gutters. Everything inside then wraps instead of widening it.
        let page_width = (window.viewport_size().width - px(PAGE_CHROME)).max(px(560.));
        let row_height = (window.viewport_size().height - px(ROW_TOP_CHROME)).max(px(ROW_MIN_H));
        let mut root = div()
            .flex()
            .flex_col()
            .w(page_width)
            .min_w(px(0.))
            .gap(px(ui::PAGE_GAP));

        // ---- The instance list (master column) -----------------------------
        let mut list_col: Option<Div> = None;
        if let Some(state) = &self.state {
            if state.instances.is_empty() {
                root = root.child(ui::empty_state(
                    &t,
                    "grid_view",
                    "No instances yet",
                    "Create an instance or import a modpack to get started.",
                ));
            } else {
                let count = state.instances.len();
                let list = ui::card(&t).p(px(8.)).gap(px(2.)).child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .px(px(8.))
                        .pt(px(6.))
                        .pb(px(8.))
                        .child(
                            div()
                                .text_size(px(ui::TEXT_BODY))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(t.foreground)
                                .child("Your instances"),
                        )
                        .child(ui::caption(
                            &t,
                            if count == 1 {
                                "1 instance".to_owned()
                            } else {
                                format!("{count} instances")
                            },
                        )),
                );
                let mut rows = div()
                    .id("instance-list-rows")
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .max_h(row_height - px(110.))
                    .min_h(px(0.))
                    .overflow_y_scroll();
                for instance in state.instances.iter() {
                    let id = instance.id.clone();
                    let view = this.clone();
                    let selected = self.selected.as_deref() == Some(&id);
                    let active = state.config.selected_instance_id.as_deref() == Some(&id);
                    let label = format!("{} — {}", instance.display_name, instance.subtitle());
                    let detail = format!(
                        "{} · {}",
                        instance.minecraft_version,
                        instance.platform_label()
                    );
                    rows = rows.child(
                        ui::list_button(
                            &t,
                            SharedString::from(format!("instance-{}", id)),
                            selected,
                        )
                        .accessibility_label(label)
                        .disabled(busy)
                        .on_click(move |_, window, cx| {
                            view.update(cx, |v, cx| v.choose(id.clone(), window, cx));
                        })
                        .child(ui::icon_tile(
                            &t,
                            icon_for(&state.icons, &instance.id),
                            32.,
                            selected,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(1.))
                                .flex_1()
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(0.))
                                                .truncate()
                                                .text_size(px(ui::TEXT_BODY))
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(t.foreground)
                                                .child(instance.display_name.clone()),
                                        )
                                        .when(needs_attention(instance), |row| {
                                            row.child(symbols::icon(
                                                Some("warning"),
                                                px(13.),
                                                t.warning,
                                            ))
                                        })
                                        .when(active, |row| {
                                            row.child(
                                                div()
                                                    .flex()
                                                    .flex_shrink_0()
                                                    .items_center()
                                                    .gap(px(4.))
                                                    .text_size(px(ui::TEXT_MICRO))
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .text_color(t.ring)
                                                    .child(
                                                        div()
                                                            .size(px(6.))
                                                            .rounded_full()
                                                            .bg(t.ring),
                                                    )
                                                    .child("Active"),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(ui::TEXT_CAPTION))
                                        .text_color(t.muted_foreground)
                                        .child(detail),
                                ),
                        ),
                    );
                }
                // One master card: the scrolling list, then (below) a pinned
                // summary footer.
                list_col = Some(list.child(rows).flex_shrink_0().w(px(LIST_W)));
            }
        }
        let mut detail = div()
            .id("instance-detail")
            .flex()
            .flex_col()
            .gap(px(16.))
            .flex_1()
            .min_w(px(0.));

        // ---- The selected instance ---------------------------------------
        if let Some(id) = self.selected.clone() {
            let instance = self.selected_instance().cloned();
            let active = self
                .state
                .as_ref()
                .is_some_and(|s| s.config.selected_instance_id.as_deref() == Some(&id));
            let stored_icon = self.stored_icon().map(str::to_owned);
            let action_button = |id: &'static str,
                                 label: &'static str,
                                 icon: Option<&'static str>,
                                 variant: ButtonVariant| {
                let view = this.clone();
                let mut button = AuroraButton::new(id, label)
                    .variant(variant)
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        view.update(cx, |v, cx| v.action(id, cx));
                    });
                if let Some(icon) = icon {
                    button = button.icon(icon);
                }
                button
            };

            // Header: who this is and whether it is in a good state. The
            // version and loader live in the list, so they are not repeated.
            if let Some(instance) = &instance {
                let attention = needs_attention(instance);
                let (icon, tone) = if attention {
                    ("warning", t.warning)
                } else {
                    ("check_circle", t.success)
                };
                let note = instance_note(instance);
                let mut head = ui::card(&t).gap(px(14.)).child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .child(ui::icon_tile(&t, stored_icon.as_deref(), 44., true))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(5.))
                                .flex_1()
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .min_w(px(0.))
                                        .max_w(px(TEXT_MAX))
                                        .text_size(px(18.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(t.foreground)
                                        .child(instance.display_name.clone()),
                                )
                                // The list already marks the active instance.
                                .children(instance_badges(&t, instance, false, false))
                                .when(!attention, |col| {
                                    col.child(
                                        div()
                                            .flex()
                                            .items_start()
                                            .gap(px(6.))
                                            .child(div().pt(px(1.)).child(symbols::icon(
                                                Some(icon),
                                                px(14.),
                                                tone,
                                            )))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w(px(0.))
                                                    .text_size(px(ui::TEXT_CAPTION))
                                                    .text_color(t.muted_foreground)
                                                    .child(note.clone()),
                                            ),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .gap(px(8.))
                                .child(
                                    action_button(
                                        "folder",
                                        "Open folder",
                                        Some("folder_open"),
                                        ButtonVariant::Default,
                                    )
                                    .size(ButtonSize::Small),
                                )
                                .when(!active, |buttons| {
                                    buttons.child(
                                        action_button(
                                            "select",
                                            "Make active",
                                            None,
                                            ButtonVariant::Soft,
                                        )
                                        .size(ButtonSize::Small),
                                    )
                                }),
                        ),
                );
                if attention {
                    head = head.child(callout(&t, icon, tone, true, note));
                }
                detail = detail.child(head);
            }

            // Compact summary of the selected instance under the list.
            if let (Some(_), Some(col)) = (&instance, list_col.take()) {
                let memory = self
                    .baseline
                    .as_ref()
                    .map(|config| format!("{} MiB", config.memory_mib))
                    .unwrap_or_else(|| "—".to_owned());
                let java = self
                    .baseline
                    .as_ref()
                    .and_then(|config| config.custom_java().map(str::to_owned))
                    .map(|_| "Custom path".to_owned())
                    .unwrap_or_else(|| "Managed runtime".to_owned());
                let lines = [("Memory", memory), ("Java", java)];
                // Footer of the master card: version and loader are already in
                // the rows above, so only Memory and Java are summarised.
                let cell = |label: &'static str, value: String| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .flex_1()
                        .min_w(px(0.))
                        .child(ui::caption(&t, label))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(ui::TEXT_BODY))
                                .text_color(t.foreground)
                                .child(value),
                        )
                };
                let mut lines = lines.into_iter();
                let mut grid = div().flex().flex_col().gap(px(10.)).px(px(8.)).pb(px(2.));
                while let (Some(left), right) = (lines.next(), lines.next()) {
                    grid = grid.child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(cell(left.0, left.1))
                            .children(right.map(|(label, value)| cell(label, value))),
                    );
                }
                list_col = Some(
                    col.child(div().mx(px(8.)).my(px(6.)).h(px(1.)).bg(t.border_subtle))
                        .child(grid),
                );
            }

            let mut tabs = div()
                .flex()
                .flex_wrap()
                .gap(px(4.))
                .pb(px(8.))
                .border_b_1()
                .border_color(t.border_subtle);
            for (index, label) in ["General", "Game version", "Runtime", "Java", "Maintenance"]
                .into_iter()
                .enumerate()
            {
                tabs = tabs.child(
                    AuroraButton::new(("instance-section", index), label)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .selected(self.section == index)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.section = index;
                            cx.notify();
                        })),
                );
            }
            detail = detail.child(tabs);
            if self.section == 0 {
                // General settings and the reviewed Client transition.
                // Aurora is a reviewed transition, never a direct configuration toggle.
                let installed = self
                    .selected_instance()
                    .is_some_and(|instance| instance.aurora.is_some());
                let aurora_note = match self.selected_instance() {
                    Some(instance) => match (&instance.aurora, instance.platform.kind.as_str()) {
                        (Some(pin), _) => format!("Aurora Client {} installed.", pin.version),
                        (None, "fabric") => "Aurora Client is not installed.".to_owned(),
                        (None, _) => "Aurora Client requires Fabric.".to_owned(),
                    },
                    None => String::new(),
                };
                let aurora_view = this.clone();
                let icon_view = this.clone();
                detail = detail.child(ui::card(&t).p(px(16.)).gap(px(0.)).child(rows(
                    &t,
                    vec![
                            (
                                "Instance name",
                                Some("Renaming applies immediately.".into()),
                                div()
                                    .flex()
                                    .gap(px(8.))
                                    .w(px(WIDE_W))
                                    .child(
                                        div().flex_1().min_w(px(0.)).child(
                                            AuroraInput::new(&self.name).label("Instance name"),
                                        ),
                                    )
                                    .child(action_button(
                                        "rename",
                                        "Rename",
                                        None,
                                        ButtonVariant::Default,
                                    ))
                                    .into_any_element(),
                            ),
                            (
                                "Icon",
                                Some(format!(
                                    "Instance icon: {}",
                                    symbols::caption(stored_icon.as_deref())
                                )),
                                AuroraButton::new("icon-change", "Change icon…")
                                    .icon("image")
                                    .disabled(busy)
                                    .on_click(move |_, window, cx| {
                                        icon_view
                                            .update(cx, |v, cx| v.open_icon_picker(window, cx));
                                    })
                                    .into_any_element(),
                            ),
                            (
                                "Aurora Client",
                                Some(aurora_note),
                                AuroraButton::new(
                                    "editor-aurora",
                                    if installed {
                                        "Remove Aurora Client…"
                                    } else {
                                        "Install Aurora Client…"
                                    },
                                )
                                .icon(if installed { "remove" } else { "download" })
                                .disabled(
                                    busy || !draft_clean
                                        || self.selected_instance().is_some_and(needs_install)
                                        || (!installed
                                            && self.selected_instance().is_some_and(|instance| {
                                                instance.platform.kind != "fabric"
                                            })),
                                )
                                .on_click(move |_, _, cx| {
                                    aurora_view.update(cx, |v, cx| v.review_aurora(!installed, cx));
                                })
                                .into_any_element(),
                            ),
                        ],
                )));
            }
            if self.section == 1 {
                // Game version and loader; official catalogs load lazily.
                let snapshots_view = this.clone();
                let game = self
                    .game
                    .clone()
                    .or_else(|| self.baseline.as_ref().map(game_choice));
                let version_values = self.version_values();
                let version_options: Vec<SharedString> = version_values
                    .iter()
                    .map(|value| {
                        if let Some(found) = self.versions.iter().find(|v| &v.id == value) {
                            if found.version_type == "snapshot" {
                                format!("{} (snapshot)", found.id).into()
                            } else {
                                found.id.clone().into()
                            }
                        } else {
                            format!("{value} (saved)").into()
                        }
                    })
                    .collect();
                let kind_options: Vec<SharedString> = self
                    .kinds
                    .iter()
                    .map(|kind| platform_label(kind).into())
                    .collect();
                let loader_values = self.loader_values();
                let loader_options: Vec<SharedString> = loader_values
                    .iter()
                    .map(|value| {
                        if value.is_empty() {
                            return SharedString::from("Automatic compatible version");
                        }
                        match self.current_loaders().iter().find(|v| &v.version == value) {
                            Some(found) if !found.stable => {
                                format!("{} (unstable)", found.version).into()
                            }
                            Some(found) => found.version.clone().into(),
                            None => format!("{value} (saved)").into(),
                        }
                    })
                    .collect();
                let selected = [
                    game.as_ref().and_then(|g| {
                        version_values
                            .iter()
                            .position(|value| value == &g.minecraft)
                    }),
                    game.as_ref()
                        .and_then(|g| self.kinds.iter().position(|kind| kind == &g.kind)),
                    game.as_ref().map(|g| {
                        g.pin
                            .as_ref()
                            .and_then(|pin| loader_values.iter().position(|value| value == pin))
                            .unwrap_or(0)
                    }),
                ];
                let loader_locked = self
                    .baseline
                    .as_ref()
                    .is_some_and(|base| base.aurora_enabled);
                let mut game_items: Vec<(&'static str, Option<String>, gpui::AnyElement)> = vec![(
                    "Include snapshots",
                    Some("Also list snapshot builds of Minecraft.".into()),
                    AuroraToggle::new("editor-snapshots")
                        .label("Include snapshots")
                        .checked(self.snapshots)
                        .disabled(busy)
                        .on_change(move |checked, _, _, cx| {
                            snapshots_view.update(cx, |v, cx| {
                                if v.snapshots != checked {
                                    v.snapshots = checked;
                                    v.load_versions();
                                    cx.notify();
                                }
                            });
                        })
                        .into_any_element(),
                )];
                for (ix, label, id) in [
                    (0usize, "Minecraft version", "editor-minecraft"),
                    (1, "Platform", "editor-platform"),
                    (2, "Loader policy", "editor-loader"),
                ] {
                    if ix == 2 && game.as_ref().is_some_and(|g| g.kind == "vanilla") {
                        continue;
                    }
                    let description = match ix {
                        1 if loader_locked => Some(
                            "Aurora Client requires Fabric; disable Aurora before changing it."
                                .into(),
                        ),
                        2 => Some("Automatic follows the newest compatible loader.".into()),
                        _ => None,
                    };
                    if ix == 0 && self.catalog_loading {
                        game_items.push((
                            label,
                            description,
                            value_box(&t, "Loading versions…", None).into_any_element(),
                        ));
                        continue;
                    }
                    if ix == 1 && loader_locked {
                        game_items.push((
                            label,
                            description,
                            value_box(
                                &t,
                                game.as_ref()
                                    .map(|g| platform_label(&g.kind))
                                    .unwrap_or("—"),
                                Some("lock"),
                            )
                            .into_any_element(),
                        ));
                        continue;
                    }
                    if ix == 2 && self.loader_loading {
                        game_items.push((
                            label,
                            description,
                            value_box(&t, "Loading loader versions…", None).into_any_element(),
                        ));
                        continue;
                    }
                    let s = &self.selects[ix];
                    let open_view = this.clone();
                    let highlight_view = this.clone();
                    let confirm_view = this.clone();
                    let options = match ix {
                        0 => version_options.clone(),
                        1 => kind_options.clone(),
                        _ => loader_options.clone(),
                    };
                    game_items.push((
                        label,
                        description,
                        AuroraSelect::new(
                            id,
                            label,
                            options,
                            &s.trigger,
                            &s.content,
                            s.bounds.clone(),
                        )
                        .width(px(SHORT_W))
                        .selected(selected[ix])
                        .open(s.open)
                        .disabled(busy)
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
                        .on_confirm(move |chosen, _, cx| {
                            confirm_view.update(cx, |v, cx| v.select_game(ix, chosen, cx));
                        })
                        .into_any_element(),
                    ));
                }
                let catalog_view = this.clone();
                let mut game_card = form_card(
                    &t,
                    card_head(
                        &t,
                        "Game version and loader",
                        Some("Version and loader changes apply after you save and install."),
                        Some(
                            AuroraButton::new("editor-catalogs", "Refresh catalogs")
                                .size(ButtonSize::Small)
                                .icon("refresh")
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    catalog_view.update(cx, |v, cx| {
                                        v.load_versions();
                                        v.load_loaders();
                                        cx.notify();
                                    });
                                })
                                .into_any_element(),
                        ),
                    ),
                    rows(&t, game_items),
                );
                if game.as_ref().is_some_and(|g| g.kind != "vanilla")
                    && self
                        .loaders
                        .as_ref()
                        .is_some_and(|catalog| catalog.for_game(self.game.as_ref()).is_empty())
                {
                    game_card = game_card.child(
                        div()
                            .pt(px(8.))
                            .text_size(px(ui::TEXT_CAPTION))
                            .text_color(t.warning)
                            .child("No supported loader versions for this Minecraft version."),
                    );
                }
                detail = detail.child(game_card);
            }
            if self.section == 2 {
                // Memory and window.
                detail = detail.child(
                    ui::card(&t)
                        .gap(px(4.))
                        .child(card_head(&t, "Memory and window", None, None))
                        .child(rows_with(
                            &t,
                            vec![
                                (
                                    "Memory (MiB)",
                                    Some("How much memory the game may use.".into()),
                                    div()
                                        .w(px(SHORT_W))
                                        .child(AuroraInput::new(&self.memory).label("Memory (MiB)"))
                                        .into_any_element(),
                                    false,
                                ),
                                (
                                    "Additional JVM arguments",
                                    Some("Extra flags passed to Java when the game starts.".into()),
                                    div()
                                        .w(px(SHORT_W))
                                        .child(
                                            AuroraInput::new(&self.arguments)
                                                .label("Additional JVM arguments"),
                                        )
                                        .into_any_element(),
                                    false,
                                ),
                                (
                                    "Window size",
                                    Some(
                                        "Width × height in pixels. Leave blank for the default."
                                            .into(),
                                    ),
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .w(px(SHORT_W))
                                        .child(
                                            div().flex_1().min_w(px(0.)).child(
                                                AuroraInput::new(&self.width)
                                                    .label("Window width (blank for default)"),
                                            ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(ui::TEXT_BODY))
                                                .text_color(t.subtle_foreground)
                                                .child("×"),
                                        )
                                        .child(
                                            div().flex_1().min_w(px(0.)).child(
                                                AuroraInput::new(&self.height)
                                                    .label("Window height (blank for default)"),
                                            ),
                                        )
                                        .into_any_element(),
                                    false,
                                ),
                            ],
                        ))
                        // The advice for this instance, read once when it was opened.
                        .when_some(self.memory_hint.clone(), |card, hint| {
                            card.child(
                                div()
                                    .pt(px(4.))
                                    .child(
                                        AuroraButton::new(
                                            "memory-advice",
                                            if self.memory_details {
                                                "Hide memory recommendation"
                                            } else {
                                                "Memory recommendation"
                                            },
                                        )
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Small)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.memory_details = !this.memory_details;
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .when(self.memory_details, |body| {
                                        body.child(hint_note(&t, hint))
                                    }),
                            )
                        }),
                );
            }
            if self.section == 3 {
                // Java. The managed runtime is read when validating or after an install.
                detail = detail.child(form_card(
                    &t,
                    card_head(&t, "Java", None, None),
                    rows_with(
                        &t,
                        vec![
                            (
                                "Custom Java path",
                                Some("Leave blank to use the managed runtime.".into()),
                                div()
                                    .w(px(SHORT_W))
                                    .child(
                                        AuroraInput::new(&self.java)
                                            .label("Custom Java path (blank for managed)"),
                                    )
                                    .into_any_element(),
                                false,
                            ),
                            (
                                "Java runtime",
                                Some(
                                    self.runtime_text
                                        .clone()
                                        .filter(|text| !text.is_empty())
                                        .unwrap_or_else(|| {
                                            "Check the path above before launching.".into()
                                        }),
                                ),
                                action_button(
                                    "java",
                                    "Validate custom Java",
                                    None,
                                    ButtonVariant::Default,
                                )
                                .into_any_element(),
                                false,
                            ),
                        ],
                    ),
                ));
            }
            if self.section == 4 {
                // Installation and recovery.
                let recovery_view = this.clone();
                detail = detail.child(form_card(
                    &t,
                    card_head(&t, "Installation and recovery", None, None),
                    div().grid().grid_cols(2).gap(px(16.)).children(
                        [
                            (
                                "Check installed files and the game runtime.",
                                action_button(
                                    "validate",
                                    "Validate instance",
                                    None,
                                    ButtonVariant::Default,
                                )
                                .into_any_element(),
                            ),
                            (
                                "Install the saved version, loader and Aurora choice.",
                                action_button(
                                    "install",
                                    "Install saved configuration",
                                    None,
                                    ButtonVariant::Default,
                                )
                                .into_any_element(),
                            ),
                            (
                                "Finish an interrupted or failed installation.",
                                action_button(
                                    "repair",
                                    "Retry installation",
                                    None,
                                    ButtonVariant::Default,
                                )
                                .into_any_element(),
                            ),
                            (
                                "Review what can be restored before applying changes.",
                                AuroraButton::new("recovery", "Inspect recovery")
                                    .disabled(busy)
                                    .on_click(move |_, _, cx| {
                                        recovery_view.update(cx, |v, cx| v.inspect_recovery(cx));
                                    })
                                    .into_any_element(),
                            ),
                        ]
                        .into_iter()
                        .map(|(description, action)| {
                            div()
                                .flex()
                                .flex_col()
                                .items_start()
                                .min_w(px(0.))
                                .gap(px(8.))
                                .child(action)
                                .child(ui::caption(&t, description))
                        }),
                    ),
                ));
            }
            if self.section == 4 {
                // Danger zone, kept apart from everything else.
                let delete_view = this.clone();
                detail = detail.child(
                    ui::card(&t)
                        .gap(px(4.))
                        .border_color(Hsla {
                            a: 0.30,
                            ..t.danger
                        })
                        .child(card_head(&t, "Danger zone", None, None))
                        .child(rows(
                            &t,
                            vec![(
                                "Delete this instance",
                                Some(
                                    "Permanently removes the instance and all of its data.".into(),
                                ),
                                AuroraButton::new("delete", "Delete…")
                                    .variant(ButtonVariant::Danger)
                                    .icon("delete")
                                    .disabled(busy)
                                    .on_click(move |_, window, cx| {
                                        delete_view.update(cx, |v, cx| {
                                            v.confirm.update(cx, |field, cx| {
                                                field.set_value("", window, cx)
                                            });
                                            v.deleting = true;
                                            cx.notify();
                                        });
                                    })
                                    .into_any_element(),
                            )],
                        )),
                );
            }
            // The one primary action of the configuration form.
            let revert_view = this.clone();
            let revert_id = id.clone();
            if (1..=3).contains(&self.section) || !draft_clean || !self.message.is_empty() {
                detail = detail.child(
                    ui::card(&t)
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap(px(16.))
                        .py(px(10.))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_start()
                                .gap(px(6.))
                                .flex_1()
                                .min_w(px(0.))
                                .child(if draft_clean {
                                    ui::caption(&t, "No unsaved changes")
                                } else {
                                    ui::badge(&t, "Unsaved changes", ui::Tone::Warning)
                                })
                                .when(!self.message.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .max_w(px(560.))
                                            .text_size(px(ui::TEXT_BODY))
                                            .text_color(t.foreground)
                                            .child(self.message.clone()),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .gap(px(8.))
                                .child(
                                    AuroraButton::new("revert", "Revert draft")
                                        .icon("history")
                                        .disabled(busy || draft_clean)
                                        .on_click(move |_, window, cx| {
                                            revert_view.update(cx, |v, cx| {
                                                v.choose(revert_id.clone(), window, cx)
                                            });
                                        }),
                                )
                                .child(
                                    action_button(
                                        "save",
                                        "Save configuration",
                                        None,
                                        ButtonVariant::Primary,
                                    )
                                    .disabled(busy || draft_clean),
                                ),
                        ),
                );
            }
        } else if !self.message.is_empty() {
            root = root.child(callout(&t, "info", t.ring, false, self.message.clone()));
        }

        if let Some(list) = list_col {
            root = root.child(
                div()
                    .flex()
                    .items_start()
                    .w_full()
                    .min_w(px(0.))
                    .gap(px(ui::PAGE_GAP))
                    .child(list)
                    .child(detail),
            );
        }

        // ---- Dialogs ------------------------------------------------------
        let palette = t.clone();
        let cancel_view = this.clone();
        let delete_view = this.clone();
        let changed_view = this.clone();
        let veto_view = this.clone();
        let panel = dialog_surface(&palette, px(440.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .size(px(40.))
                            .rounded(px(10.))
                            .bg(Hsla {
                                a: 0.12,
                                ..palette.danger
                            })
                            .child(symbols::icon(Some("delete"), px(22.), palette.danger)),
                    )
                    .child(ui::section_header(&palette, "Delete instance", None)),
            )
            .child(ui::muted(
                &palette,
                "This removes the instance and its data. Type its exact current name to confirm.",
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(ui::TEXT_CAPTION))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(palette.muted_foreground)
                            .child("Instance name"),
                    )
                    .child(AuroraInput::new(&self.confirm).label("Confirm instance name")),
            )
            .when(!self.message.is_empty(), |this| {
                this.child(
                    div()
                        .text_size(px(ui::TEXT_BODY))
                        .text_color(palette.danger)
                        .child(self.message.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("delete-cancel", "Cancel")
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                cancel_view.update(cx, |v, cx| {
                                    v.deleting = false;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        AuroraButton::new("delete-confirm", "Delete instance")
                            .variant(ButtonVariant::Danger)
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                delete_view.update(cx, |v, cx| v.action("delete", cx));
                            }),
                    ),
            );
        let aurora_body = match (&self.aurora_review, self.busy) {
            (Some(review), _) => aurora_review_text(review),
            (None, true) => "Reviewing the Aurora change…".to_owned(),
            (None, false) => {
                if self.message.is_empty() {
                    "Close this review and request it again.".to_owned()
                } else {
                    self.message.clone()
                }
            }
        };
        let aurora_identity = self.aurora_identity();
        let aurora_allowed = aurora_apply_allowed(
            self.aurora_review.as_ref(),
            aurora_identity
                .as_ref()
                .map(|(i, s, g)| (i.as_str(), *s, *g)),
            busy,
        );
        let aurora_cancel_view = this.clone();
        let aurora_apply_view = this.clone();
        let aurora_changed_view = this.clone();
        let aurora_veto_view = this.clone();
        let review_width = px(560.).min(window.viewport_size().width - px(48.));
        let review_height = window.viewport_size().height - px(48.);
        let body_height = (review_height - px(260.)).max(px(48.));
        let aurora_panel = dialog_surface(&palette, review_width)
            .max_h(review_height)
            .child(div().flex_shrink_0().child(ui::section_header(
                &palette,
                "Review Aurora change",
                Some("Check what will change, then apply.".into()),
            )))
            .child(
                div()
                    .p(px(12.))
                    .rounded(palette.radius)
                    .bg(palette.sunken)
                    .border_1()
                    .border_color(palette.border_subtle)
                    .text_size(px(ui::TEXT_BODY))
                    .text_color(palette.muted_foreground)
                    .child(review_body(
                        "aurora-review-body",
                        aurora_body,
                        body_height,
                        window,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .flex_shrink_0()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("aurora-cancel", "Cancel")
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                aurora_cancel_view.update(cx, |v, cx| {
                                    v.aurora_open = false;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        AuroraButton::new("aurora-apply", "Apply Aurora change")
                            .variant(ButtonVariant::Primary)
                            .disabled(!aurora_allowed)
                            .on_click(move |_, _, cx| {
                                aurora_apply_view.update(cx, |v, cx| v.apply_aurora_transition(cx));
                            }),
                    ),
            );
        let recovery_body = match (&self.recovery_review, self.busy) {
            (Some(review), _) => recovery_text(&review.preview),
            (None, true) => "Checking installation, pack edits and mod requirements…".to_owned(),
            (None, false) => {
                if self.message.is_empty() {
                    "Close this review and inspect again.".to_owned()
                } else {
                    self.message.clone()
                }
            }
        };
        let removed_count = self
            .recovery_review
            .as_ref()
            .map(|review| review.preview.removed_mods.len())
            .unwrap_or(0);
        let recovery_allowed = recovery_apply_allowed(
            self.recovery_review.as_ref(),
            self.selected.as_deref(),
            self.generation,
            self.recovery_keep,
            busy,
        );
        let recovery_close_view = this.clone();
        let recovery_inspect_view = this.clone();
        let recovery_apply_view = this.clone();
        let recovery_keep_view = this.clone();
        let recovery_changed_view = this.clone();
        let recovery_veto_view = this.clone();
        let mut recovery_panel = dialog_surface(&palette, review_width)
            .max_h(review_height)
            .child(div().flex_shrink_0().child(ui::section_header(
                &palette,
                "Recovery review",
                Some("See what a safe recovery would keep and remove.".into()),
            )))
            .child(
                div()
                    .p(px(12.))
                    .rounded(palette.radius)
                    .bg(palette.sunken)
                    .border_1()
                    .border_color(palette.border_subtle)
                    .text_size(px(ui::TEXT_BODY))
                    .text_color(palette.muted_foreground)
                    .child(review_body(
                        "recovery-review-body",
                        recovery_body,
                        body_height,
                        window,
                        cx,
                    )),
            );
        if removed_count > 0 {
            recovery_panel = recovery_panel.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(ui::muted(
                        &palette,
                        format!("Keep the {removed_count} listed mod(s) removed"),
                    ))
                    .child(
                        AuroraToggle::new("recovery-keep")
                            .label(format!("Keep the {removed_count} listed mod(s) removed"))
                            .checked(self.recovery_keep)
                            .disabled(busy)
                            .on_change(move |checked, _, _, cx| {
                                recovery_keep_view.update(cx, |v, cx| {
                                    v.recovery_keep = checked;
                                    cx.notify();
                                });
                            }),
                    ),
            );
        }
        let recovery_panel = recovery_panel.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .flex_shrink_0()
                .gap(px(8.))
                .child(
                    AuroraButton::new("recovery-inspect", "Inspect again")
                        .variant(ButtonVariant::Ghost)
                        .icon("refresh")
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            recovery_inspect_view.update(cx, |v, cx| v.inspect_recovery(cx));
                        }),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            AuroraButton::new("recovery-close", "Close")
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    recovery_close_view.update(cx, |v, cx| {
                                        v.recovery_open = false;
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            AuroraButton::new("recovery-apply", "Apply safe recovery")
                                .variant(ButtonVariant::Primary)
                                .disabled(!recovery_allowed)
                                .on_click(move |_, _, cx| {
                                    recovery_apply_view
                                        .update(cx, |v, cx| v.apply_recovery_now(cx));
                                }),
                        ),
                ),
        );
        let icon_panel = self.icon_panel(review_width, cx);
        let icon_changed_view = this.clone();
        let icon_veto_view = this.clone();
        // Dialogs render out of flow; hold them in a zero-size box so the
        // closed ones add no gaps to the page.
        root.child(
            div()
                .absolute()
                .size_0()
                .child(
                    Dialog::new(cx)
                        .open(self.icon_open)
                        .accessibility_label("Instance icon")
                        .close_on_backdrop_press(false)
                        .on_ok(|_, _, _| false)
                        .on_cancel(move |_, _, cx| !icon_veto_view.read(cx).busy)
                        .on_open_change(move |open, _, _, cx| {
                            icon_changed_view.update(cx, |v, cx| {
                                v.icon_open = open;
                                cx.notify();
                            });
                        })
                        .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                        .popup(icon_panel),
                )
                .child(
                    Dialog::new(cx)
                        .open(self.deleting)
                        .accessibility_label("Delete instance")
                        .close_on_backdrop_press(false)
                        // Destructive submission is explicit; Enter in the text field must not bypass review.
                        .on_ok(|_, _, _| false)
                        .on_cancel(move |_, _, cx| !veto_view.read(cx).busy)
                        .on_open_change(move |open, _, _, cx| {
                            changed_view.update(cx, |v, cx| {
                                v.deleting = open;
                                cx.notify();
                            });
                        })
                        .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                        .popup(panel),
                )
                .child(
                    Dialog::new(cx)
                        .open(self.aurora_open)
                        .accessibility_label("Review Aurora change")
                        .close_on_backdrop_press(false)
                        .on_ok(|_, _, _| false)
                        .on_cancel(move |_, _, cx| !aurora_veto_view.read(cx).busy)
                        .on_open_change(move |open, _, _, cx| {
                            aurora_changed_view.update(cx, |v, cx| {
                                v.aurora_open = open;
                                cx.notify();
                            });
                        })
                        .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                        .popup(aurora_panel),
                )
                .child(
                    Dialog::new(cx)
                        .open(self.recovery_open)
                        .accessibility_label("Recovery review")
                        .close_on_backdrop_press(false)
                        .on_ok(|_, _, _| false)
                        .on_cancel(move |_, _, cx| !recovery_veto_view.read(cx).busy)
                        .on_open_change(move |open, _, _, cx| {
                            recovery_changed_view.update(cx, |v, cx| {
                                v.recovery_open = open;
                                cx.notify();
                            });
                        })
                        .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                        .popup(recovery_panel),
                ),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn configuration(loader: serde_json::Value) -> api::InstanceConfiguration {
        serde_json::from_value(serde_json::json!({
            "minecraftVersion":"1.21","auroraEnabled":false,"loader":loader,
            "memoryMib":2048,"additionalJvmArguments":"","window":null,"future":{"x":1}
        }))
        .unwrap()
    }
    fn instance(configuration: api::InstanceConfiguration) -> api::InstanceSummary {
        serde_json::from_value(serde_json::json!({
            "id":"a","displayName":"Aurora","state":"ready","aurora":null,
            "platform":{"kind":"vanilla"},"minecraftVersion":"1.21",
            "configuration": serde_json::to_value(configuration).unwrap()
        }))
        .unwrap()
    }

    #[test]
    fn configuration_preserves_unknown_fields_and_loader() {
        let c = configuration(
            serde_json::json!({"kind":"fabric","policy":{"type":"pinned","version":"x"}}),
        );
        let game = game_choice(&c);
        let edited = edited_config(c.clone(), &game, "4096", "-Xfoo", "", "", "").unwrap();
        assert_eq!(edited.other, c.other);
        assert_eq!(edited.loader, c.loader);
        assert_eq!(edited.memory_mib, 4096);
    }
    #[test]
    fn incomplete_window_is_rejected() {
        let c = configuration(serde_json::json!({"kind":"vanilla"}));
        let game = game_choice(&c);
        assert!(edited_config(c, &game, "2048", "", "800", "", "").is_err());
    }
    #[test]
    fn game_edits_change_only_explicit_fields() {
        let base = configuration(
            serde_json::json!({"kind":"fabric","policy":{"type":"pinned","version":"x"},"futurePolicy":{"keep":true}}),
        );
        // An unchanged choice keeps the exact persisted loader JSON, unknown fields included.
        assert_eq!(draft_loader(&base, &game_choice(&base)), base.loader);
        // A new version drops the pin to automatic but keeps unknown configuration fields.
        let moved = GameChoice {
            minecraft: "1.21.4".into(),
            kind: "fabric".into(),
            pin: None,
        };
        assert_eq!(
            draft_loader(&base, &moved),
            serde_json::json!({"kind":"fabric","policy":{"type":"automatic"}})
        );
        let edited = edited_config(base.clone(), &moved, "2048", "", "", "", "").unwrap();
        assert_eq!(edited.other, base.other);
        assert_eq!(edited.minecraft_version, "1.21.4");
        // Vanilla has no policy, and memory-only edits keep the exact pin.
        let vanilla = GameChoice {
            minecraft: "1.21.4".into(),
            kind: "vanilla".into(),
            pin: Some("ignored".into()),
        };
        assert_eq!(
            draft_loader(&base, &vanilla),
            serde_json::json!({"kind":"vanilla"})
        );
        let memory_only =
            edited_config(base.clone(), &game_choice(&base), "8192", "", "", "", "").unwrap();
        assert_eq!(memory_only.loader, base.loader);
        assert_eq!(memory_only.minecraft_version, base.minecraft_version);
        // A custom Java shape is preserved unless the user explicitly replaces it.
        let mut custom = base.clone();
        custom.java =
            Some(serde_json::json!({"kind":"custom","path":"C:\\jdk\\bin\\java.exe","extra":true}));
        let kept = edited_config(
            custom.clone(),
            &game_choice(&custom),
            "2048",
            "",
            "",
            "",
            "C:\\jdk\\bin\\java.exe",
        )
        .unwrap();
        assert_eq!(kept.java, custom.java);
        let replaced =
            edited_config(custom, &game_choice(&base), "2048", "", "", "", "D:\\jdk").unwrap();
        assert_eq!(
            replaced.java,
            Some(serde_json::json!({"kind":"custom","path":"D:\\jdk"}))
        );
    }
    #[test]
    fn stale_catalog_and_review_identity_are_rejected() {
        let draft = GameChoice {
            minecraft: "a".into(),
            kind: "fabric".into(),
            pin: None,
        };
        assert!(versions_reply_current(2, 2, true, true));
        assert!(!versions_reply_current(1, 2, true, true));
        assert!(!versions_reply_current(2, 2, false, true));
        assert!(loaders_reply_current(3, 3, "a", "fabric", Some(&draft)));
        assert!(!loaders_reply_current(2, 3, "a", "fabric", Some(&draft)));
        let other_version = GameChoice {
            minecraft: "b".into(),
            kind: "fabric".into(),
            pin: None,
        };
        assert!(!loaders_reply_current(
            3,
            3,
            "a",
            "fabric",
            Some(&other_version)
        ));
        assert!(!loaders_reply_current(3, 3, "a", "fabric", None));
        assert!(aurora_reply_current(
            4,
            4,
            "i1",
            false,
            7,
            Some(("i1", false, 7))
        ));
        assert!(!aurora_reply_current(
            3,
            4,
            "i1",
            false,
            7,
            Some(("i1", false, 7))
        ));
        assert!(!aurora_reply_current(
            4,
            4,
            "i2",
            false,
            7,
            Some(("i1", false, 7))
        ));
        assert!(!aurora_reply_current(
            4,
            4,
            "i1",
            true,
            7,
            Some(("i1", false, 7))
        ));
        assert!(!aurora_reply_current(
            4,
            4,
            "i1",
            false,
            6,
            Some(("i1", false, 7))
        ));
        assert!(!aurora_reply_current(4, 4, "i1", false, 7, None));
        assert!(recovery_reply_current(5, 5, "i1", 7, Some(("i1", 7))));
        assert!(!recovery_reply_current(4, 5, "i1", 7, Some(("i1", 7))));
        assert!(!recovery_reply_current(5, 5, "i1", 7, Some(("i1", 8))));
        assert!(!recovery_reply_current(5, 5, "i2", 7, Some(("i1", 7))));
        assert!(!recovery_reply_current(5, 5, "i1", 7, None));
    }
    fn transition(fingerprint: &str, blockers: &[&str]) -> api::AuroraTransition {
        api::AuroraTransition {
            instance_id: "i1".into(),
            target: api::AuroraTarget {
                aurora: Some(api::AuroraPin {
                    channel: "stable".into(),
                    version: "3.2.0".into(),
                }),
            },
            requirements_added: vec!["Fabric Loader 0.17.3".into()],
            requirements_removed: vec![],
            warnings: vec![],
            blockers: blockers.iter().map(|s| (*s).to_owned()).collect(),
            fingerprint: fingerprint.to_owned(),
        }
    }
    #[test]
    fn aurora_apply_requires_fingerprint_without_blockers_for_the_same_target() {
        let review = |fingerprint: &str, blockers: &[&str]| AuroraReview {
            instance: "i1".into(),
            enabled: true,
            installed: false,
            generation: 7,
            preview: transition(fingerprint, blockers),
        };
        let identity = Some(("i1", false, 7u64));
        assert!(aurora_apply_allowed(
            Some(&review("fp", &[])),
            identity,
            false
        ));
        assert!(!aurora_apply_allowed(
            Some(&review("", &[])),
            identity,
            false
        ));
        assert!(!aurora_apply_allowed(
            Some(&review("fp", &["Java 8 required"])),
            identity,
            false
        ));
        assert!(!aurora_apply_allowed(
            Some(&review("fp", &[])),
            identity,
            true
        ));
        assert!(!aurora_apply_allowed(Some(&review("fp", &[])), None, false));
        assert!(!aurora_apply_allowed(
            Some(&review("fp", &[])),
            Some(("i2", false, 7)),
            false
        ));
        assert!(!aurora_apply_allowed(
            Some(&review("fp", &[])),
            Some(("i1", true, 7)),
            false
        ));
        assert!(!aurora_apply_allowed(
            Some(&review("fp", &[])),
            Some(("i1", false, 8)),
            false
        ));
        assert!(!aurora_apply_allowed(None, identity, false));
    }
    fn recovery_preview(
        can_apply: bool,
        removed: &[&str],
        fingerprint: &str,
    ) -> api::RecoveryPreview {
        api::RecoveryPreview {
            instance_id: "i1".into(),
            summary: "Recovery can reconcile this instance.".into(),
            kept_settings: vec!["options.txt".into()],
            removed_mods: removed.iter().map(|s| (*s).to_owned()).collect(),
            blockers: vec![],
            can_apply,
            fingerprint: fingerprint.to_owned(),
        }
    }
    #[test]
    fn recovery_apply_requires_confirmed_inspection_and_choice() {
        let review = |can: bool, removed: &[&str], fingerprint: &str| RecoveryReview {
            instance: "i1".into(),
            generation: 3,
            preview: recovery_preview(can, removed, fingerprint),
        };
        assert!(recovery_apply_allowed(
            Some(&review(true, &[], "fp")),
            Some("i1"),
            3,
            false,
            false
        ));
        assert!(!recovery_apply_allowed(
            Some(&review(false, &[], "fp")),
            Some("i1"),
            3,
            false,
            false
        ));
        assert!(!recovery_apply_allowed(
            Some(&review(true, &["mod.jar"], "fp")),
            Some("i1"),
            3,
            false,
            false
        ));
        assert!(recovery_apply_allowed(
            Some(&review(true, &["mod.jar"], "fp")),
            Some("i1"),
            3,
            true,
            false
        ));
        assert!(!recovery_apply_allowed(
            Some(&review(true, &[], "")),
            Some("i1"),
            3,
            false,
            false
        ));
        assert!(!recovery_apply_allowed(
            Some(&review(true, &[], "fp")),
            Some("i1"),
            3,
            false,
            true
        ));
        assert!(!recovery_apply_allowed(
            Some(&review(true, &[], "fp")),
            Some("i2"),
            3,
            false,
            false
        ));
        assert!(!recovery_apply_allowed(
            Some(&review(true, &[], "fp")),
            Some("i1"),
            4,
            false,
            false
        ));
        assert!(!recovery_apply_allowed(None, Some("i1"), 3, false, false));
    }
    #[test]
    fn save_gating_and_saved_versus_installed_state() {
        let c = configuration(serde_json::json!({"kind":"vanilla"}));
        let game = game_choice(&c);
        assert!(
            edited_config(c.clone(), &game, "not-a-number", "", "", "", "").is_err(),
            "an invalid draft must block the save mutation"
        );
        assert!(!needs_install(&instance(c.clone())));
        let mut moved = c.clone();
        moved.minecraft_version = "1.21.4".into();
        assert!(needs_install(&instance(moved)));
        let mut aurora_changed = c.clone();
        aurora_changed.aurora_enabled = true;
        assert!(needs_install(&instance(aurora_changed)));
        let pinned = serde_json::from_value(serde_json::json!({
            "minecraftVersion":"1.21","auroraEnabled":false,
            "loader":{"kind":"fabric","policy":{"type":"pinned","version":"0.16.0"}},
            "memoryMib":2048,"additionalJvmArguments":"","window":null
        }))
        .unwrap();
        let mut summary = instance(pinned);
        summary.platform = api::Platform {
            kind: "fabric".into(),
            version: Some("0.16.0".into()),
        };
        assert!(!needs_install(&summary));
        summary.platform.version = Some("0.15.0".into());
        assert!(needs_install(&summary));
        summary.state = "installing".into();
        assert!(!needs_install(&summary));
    }

    #[test]
    fn unrelated_mutation_preserves_dirty_draft_and_original_expected_configuration() {
        let original = configuration(
            serde_json::json!({"kind":"fabric","policy":{"type":"pinned","version":"0.16.0","future":true}}),
        );
        let mut baseline = Some(original.clone());
        let mut game = Some(GameChoice {
            minecraft: "1.21.4".into(),
            kind: "fabric".into(),
            pin: Some("0.17.0".into()),
        });
        let before = game.clone();
        let dirty = |baseline: &api::InstanceConfiguration, game: &GameChoice| {
            edited_config(
                baseline.clone(),
                game,
                "6144",
                "-Dkeep=draft",
                "1200",
                "800",
                "C:\\custom\\java.exe",
            )
            .unwrap()
        };
        let draft_before = dirty(baseline.as_ref().unwrap(), game.as_ref().unwrap());
        let mut concurrent = original.clone();
        concurrent.memory_mib = 4096;
        for kind in [
            Mutation::Rename,
            Mutation::Install,
            Mutation::Retry,
            Mutation::Select,
        ] {
            let reload = accept_mutation_configuration(kind, &concurrent, &mut baseline, &mut game);
            assert!(
                reload.is_none(),
                "unrelated acknowledgments must not reset text fields"
            );
            assert_eq!(baseline.as_ref(), Some(&original));
            assert_eq!(game, before);
            assert_eq!(
                dirty(baseline.as_ref().unwrap(), game.as_ref().unwrap()),
                draft_before
            );
        }
        // A subsequent save still sends the original expected snapshot. Core
        // can reject concurrent configuration changes instead of rebasing them.
        assert_ne!(baseline.as_ref(), Some(&concurrent));
        let reload =
            accept_mutation_configuration(Mutation::Save, &draft_before, &mut baseline, &mut game);
        assert_eq!(reload.as_ref(), Some(&draft_before));
        assert_eq!(baseline.as_ref(), Some(&draft_before));
        assert_eq!(game, Some(game_choice(&draft_before)));
    }

    #[test]
    fn loader_catalog_cannot_leak_between_game_identities_after_failure() {
        let catalog = LoaderCatalog {
            minecraft: "a".into(),
            kind: "fabric".into(),
            versions: vec![api::LoaderVersion {
                version: "a-only".into(),
                stable: true,
            }],
        };
        let a = GameChoice {
            minecraft: "a".into(),
            kind: "fabric".into(),
            pin: None,
        };
        let b = GameChoice {
            minecraft: "b".into(),
            kind: "fabric".into(),
            pin: Some("b-saved".into()),
        };
        assert_eq!(catalog.for_game(Some(&a))[0].version, "a-only");
        // Even if B's request fails, the previously accepted A catalog cannot
        // supply choices for B. Only B's explicit pin and automatic remain.
        assert!(catalog.for_game(Some(&b)).is_empty());
        assert!(!loaders_reply_current(1, 2, "a", "fabric", Some(&b)));
        assert!(catalog.for_game(None).is_empty());
    }

    fn advice(recommended: Option<u32>, explanation: &str) -> api::MemoryAdvice {
        serde_json::from_value(serde_json::json!({
            "recommendedMib": recommended,
            "explanation": explanation,
            "facts": {"totalMib": 16384},
        }))
        .unwrap()
    }
    fn runtime(status: &str, component: &str, major: u32) -> api::RuntimeStatus {
        serde_json::from_value(serde_json::json!({
            "status": status,
            "component": component,
            "requiredMajorVersion": major,
        }))
        .unwrap()
    }

    #[test]
    fn stale_advice_and_runtime_replies_are_dropped() {
        // Same instance, latest request.
        assert!(read_reply_current(4, 4, "i1", Some("i1")));
        // A newer request (e.g. validate pressed again) supersedes it.
        assert!(!read_reply_current(3, 4, "i1", Some("i1")));
        // The user opened another instance, or none, meanwhile.
        assert!(!read_reply_current(4, 4, "i1", Some("i2")));
        assert!(!read_reply_current(4, 4, "i1", None));
    }

    #[test]
    fn memory_hint_and_runtime_text_match_the_supported_host() {
        assert_eq!(
            memory_hint_text(&advice(Some(6144), "Based on 16 GB of installed memory.")),
            "Recommended: 6.0 GB. Based on 16 GB of installed memory."
        );
        assert_eq!(
            memory_hint_text(&advice(Some(2560), "Light pack.")),
            "Recommended: 2.5 GB. Light pack."
        );
        assert_eq!(
            memory_hint_text(&advice(None, "Memory could not be measured.")),
            "Memory could not be measured."
        );
        let text = |status: &str, component: &str| {
            runtime_status_text(&Ok(runtime(status, component, 21)))
        };
        assert_eq!(
            text("ready", "Managed Java"),
            "Managed Java (Java 21) · installed and verified"
        );
        assert_eq!(
            text("ready", "Custom Java"),
            "Custom Java (Java 21) · checked and compatible"
        );
        assert_eq!(
            text("missing", "Managed Java"),
            "Managed Java (Java 21) · not installed yet; installed automatically when needed"
        );
        assert_eq!(
            text("broken", "Managed Java"),
            "Managed Java (Java 21) · broken"
        );
        assert_eq!(
            runtime_status_text(&Err(api::ApiError::new("runtime", "Runtime unavailable."))),
            "Runtime unavailable."
        );
    }

    #[test]
    fn stored_icons_resolve_per_instance_with_fallback_to_default() {
        let icons = BTreeMap::from([("i1".to_owned(), "swords".to_owned())]);
        assert_eq!(icon_for(&icons, "i1"), Some("swords"));
        assert_eq!(icon_for(&icons, "i2"), None);
        // A stored name this build no longer knows draws the fallback glyph.
        let stale = BTreeMap::from([("i1".to_owned(), "renamed_glyph".to_owned())]);
        assert_eq!(
            symbols::path_or_fallback(icon_for(&stale, "i1")),
            symbols::path_or_fallback(None)
        );
        assert_eq!(symbols::caption(icon_for(&stale, "i2")), "Deployed code");
    }

    #[test]
    fn set_icon_is_guarded_while_busy_and_skips_no_ops() {
        let persist = |icon: &str| IconSave::Persist {
            instance: "i1".into(),
            icon: Some(icon.into()),
        };
        assert_eq!(
            icon_save(false, Some("i1"), None, Some("swords")),
            persist("swords")
        );
        assert_eq!(
            icon_save(false, Some("i1"), Some("shield"), Some("swords")),
            persist("swords")
        );
        // Choosing the default clears the stored icon.
        assert_eq!(
            icon_save(false, Some("i1"), Some("shield"), None),
            IconSave::Persist {
                instance: "i1".into(),
                icon: None
            }
        );
        // Unchanged choices persist nothing.
        assert_eq!(
            icon_save(false, Some("i1"), Some("swords"), Some("swords")),
            IconSave::Unchanged
        );
        assert_eq!(
            icon_save(false, Some("i1"), None, None),
            IconSave::Unchanged
        );
        // Another operation is running, or no instance is open.
        assert_eq!(
            icon_save(true, Some("i1"), None, Some("swords")),
            IconSave::Blocked
        );
        assert_eq!(
            icon_save(false, None, None, Some("swords")),
            IconSave::Blocked
        );
    }

    #[test]
    fn stale_icon_searches_are_dropped() {
        assert!(icons_reply_current(5, 5, true));
        assert!(!icons_reply_current(4, 5, true));
        // A search finishing after the picker was closed is ignored.
        assert!(!icons_reply_current(5, 5, false));
    }
}
