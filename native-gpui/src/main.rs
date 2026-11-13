#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate gpui_kit as gpui;

mod accounts;
mod artwork;
mod backend;
mod borealis;
mod chrome;
mod client_updates;
mod content;
mod content_updates;
mod controls;
mod discover;
mod downloads;
mod file_picker;
mod home_widgets;
mod images;
mod instance_creator;
mod instance_editor;
mod media;
mod pack_import;
mod settings;
mod skin_preview;
mod skins;
mod symbols;
mod syncing;
mod theme;
mod ui;

#[allow(dead_code)]
#[path = "../../native/src/api.rs"]
mod api;

#[allow(dead_code)]
#[path = "../../native/src/player.rs"]
mod player;

#[allow(dead_code)]
#[path = "../../native/src/preview.rs"]
mod preview;

use accounts::{Accounts, AccountsEvent};
use aurora_launcher_lib::host::{APPLICATION_IDENTIFIER, Host, platform_local_data_dir};
use borealis::Borealis;
use client_updates::{ClientUpdates, ClientUpdatesEvent};
use content::{Content, ContentEvent};
use controls::{AuroraButton, ButtonSize, ButtonVariant};
use discover::{Discover, DiscoverEvent};
use downloads::{Downloads, DownloadsEvent};
use gpui::{
    App, Bounds, Context, ExternalPaths, Role, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, size,
};
use home_widgets::{HomeWidgets, HomeWidgetsEvent};
use instance_creator::{InstanceCreator, InstanceCreatorEvent};
use instance_editor::{InstanceEditor, InstanceEditorEvent};
use pack_import::{PackImport, PackImportEvent};
use settings::{Settings, SettingsEvent};
use skin_preview::SkinPreview;
use skins::{Skins, SkinsEvent};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use syncing::{Syncing, SyncingEvent};

use backend::guarded;

struct Snapshot {
    /// Present only for startup-sequence loads (first load and Retry): the
    /// application status read that must succeed before anything else loads.
    status: Option<api::ApiResult<()>>,
    launcher: api::ApiResult<api::LauncherState>,
    accounts: api::ApiResult<api::AccountsState>,
    appearance: api::ApiResult<api::Appearance>,
}
enum Message {
    Loaded(u64, Snapshot),
    Readiness(
        u64,
        u64,
        api::ApiResult<api::PlayReadiness>,
        Option<api::LaunchProcess>,
    ),
    /// One-shot follow-up for a readiness decision that still reports a process
    /// the supervisor has already finished; carries the serial it was armed for.
    RecheckReadiness(u64),
    StartupUpdates(api::ApiResult<api::UpdateOverview>),
    Launched(String, u64, api::ApiResult<api::LaunchProcess>),
    Recovery(u64, String, api::ApiResult<api::RecoveryPreview>),
    Done(api::ApiResult<()>),
    SyncInitialized(api::ApiResult<()>),
    Event(String, serde_json::Value),
}

/// The supervisor can report an exit slightly before readiness settles, so a
/// decision that still says running/starting is re-read a bounded number of
/// times (same limit and delay as the Slint host).
const READINESS_RECHECK_LIMIT: u8 = 5;

/// Design-review mode: a disposable seeded data root, no update check, sync,
/// server status or readiness network work. Set once before the window opens.
static SHOWCASE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn showcase() -> bool {
    SHOWCASE.load(std::sync::atomic::Ordering::Relaxed)
}

/// `--showcase <dir>`: the directory must carry the disposable marker so the
/// owner's managed data can never be opened this way.
fn showcase_root() -> Option<PathBuf> {
    let mut args = std::env::args().skip_while(|a| a != "--showcase").skip(1);
    let root = PathBuf::from(args.next()?);
    root.join("AURORA_DISPOSABLE_DIAGNOSTIC").is_file().then_some(root)
}
const READINESS_RECHECK_DELAY: std::time::Duration = std::time::Duration::from_millis(750);

/// A readiness decision that claims a live process while the supervised
/// child's own snapshot says it already finished.
fn stale_running(decision: &api::PlayReadiness, child: Option<&api::LaunchProcess>) -> bool {
    matches!(decision.process_status.as_str(), "running" | "starting")
        && child.is_some_and(|child| matches!(child.status.as_str(), "exited" | "failed"))
}

/// What a Play blocker's action button does. Codes without an action show only
/// their explanation, exactly like the Slint host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockerAction {
    SignIn,
    InstallJava,
    JavaSettings,
    ReviewRecovery,
    OpenInstance,
    ReviewMods,
}

impl BlockerAction {
    fn for_code(code: &str) -> Option<Self> {
        match code {
            "launch_authentication_required" => Some(Self::SignIn),
            "launch_runtime_not_ready" => Some(Self::InstallJava),
            "launch_custom_java_invalid" => Some(Self::JavaSettings),
            "launch_instance_not_ready" | "launch_instance_damaged" => Some(Self::ReviewRecovery),
            "launch_configuration_changed" => Some(Self::OpenInstance),
            "launch_mod_incompatible" => Some(Self::ReviewMods),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::SignIn => "Sign in",
            Self::InstallJava => "Install Java",
            Self::JavaSettings => "Java settings",
            Self::ReviewRecovery => "Review recovery",
            Self::OpenInstance => "Open instance",
            Self::ReviewMods => "Review mods",
        }
    }

    /// The page the action navigates to; Install Java runs a root action instead.
    fn page(self) -> Option<u8> {
        match self {
            Self::SignIn => Some(1),
            Self::JavaSettings | Self::ReviewRecovery | Self::OpenInstance => Some(3),
            Self::ReviewMods => Some(4),
            Self::InstallJava => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GamePhase {
    Idle,
    Starting,
    Running,
}

/// The shape the Settings page parses for an `update-status` host event, so a
/// startup result can reach a Settings page that did not exist yet.
fn update_overview_payload(overview: &api::UpdateOverview) -> serde_json::Value {
    serde_json::json!({
        "launcher": {
            "installedVersion": overview.launcher.installed_version,
            "availability": overview.launcher.availability,
        },
        "client": overview.client,
        "clientInstanceId": overview.client_instance_id,
        "clientInstanceName": overview.client_instance_name,
        "clientInstalledVersion": overview.client_installed_version,
    })
}

/// Whether the Borealis background is on and its Motion speed for the root's
/// appearance snapshot. Without a valid snapshot (a failed load, or diagnostic
/// mode, which has no Host to read one from) the background is plain and
/// static. `settings_speed` is the Settings page's effective speed (a live
/// slider draft or a queued write), which wins over the saved one; it is `None`
/// when that page was never built, so startup alone derives the saved speed.
fn borealis_inputs(appearance: Option<&api::Appearance>, settings_speed: Option<u8>) -> (bool, u8) {
    match appearance {
        Some(appearance) => (
            borealis::is_borealis(&appearance.background),
            settings_speed.unwrap_or(appearance.aurora_motion_speed),
        ),
        None => (false, 0),
    }
}

/// The message Slint shows when Quick Play cannot start for a reason the user
/// can act on (`widgets::begin_server_launch`, `home::quick_play`). Busy and
/// stale-row refusals stay silent there too.
fn quick_play_refusal(
    mode: &str,
    instance_exists: bool,
    account_selected: bool,
) -> Option<&'static str> {
    if mode == "server" {
        if !instance_exists {
            return Some("The instance for this server no longer exists.");
        }
        if !account_selected {
            return Some("Sign in to quick play this server.");
        }
    } else if !account_selected {
        return Some("Select a Minecraft account to use Quick Play.");
    }
    None
}

/// The progress line while Quick Play is starting: servers name their
/// destination, every other mode uses the generic readiness text.
fn quick_play_progress(mode: &str, server_name: Option<&str>) -> String {
    match (mode, server_name) {
        ("server", Some(name)) => format!("Preparing server: {name}"),
        _ => "Checking readiness…".into(),
    }
}

/// A page change is refused while a client update runs. Every accepted one,
/// including a click on the current page, discards the Settings previews like
/// Slint's `Nav.navigate` (`cancel_motion_preview`).
fn navigation_accepted(from: u8, to: u8, client_update_running: bool) -> bool {
    to == from || !client_update_running
}

/// The stored icon name of an instance; `None` (never set, or a damaged icon
/// store) draws the fallback glyph, like the Slint host.
fn stored_icon<'a>(icons: &'a BTreeMap<String, String>, instance: &str) -> Option<&'a str> {
    icons.get(instance).map(String::as_str)
}

/// The Slint `InstanceIcon`: the symbol on a rounded tile, in the accent for
/// the active instance and the secondary ink otherwise.
fn instance_icon(
    name: Option<&str>,
    size: f32,
    active: bool,
    palette: &theme::AuroraTheme,
) -> impl IntoElement {
    let ink = if active {
        palette.ring
    } else {
        palette.muted_foreground
    };
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(px(size))
        .h(px(size))
        .rounded(px((size * 0.24).min(28.)))
        .when(active, |tile| {
            tile.bg(gpui::linear_gradient(
                145.,
                gpui::linear_color_stop(gpui::Hsla { a: 0.30, ..palette.ring }, 0.),
                gpui::linear_color_stop(gpui::Hsla { a: 0.08, ..palette.ring }, 1.),
            ))
        })
        .when(!active, |tile| tile.bg(palette.control))
        .border_1()
        .border_color(if active {
            gpui::Hsla { a: 0.35, ..palette.ring }
        } else {
            palette.border_subtle
        })
        .child(symbols::icon(name, px(size * 0.54), ink))
}

/// How a load request was scheduled.
enum LoadPlan {
    /// Another load is in flight; one follow-up will run after it.
    Coalesced,
    Run {
        serial: u64,
        startup: bool,
    },
}

struct Launcher {
    /// The window background behind the chrome and pages; absent only in
    /// fixtures that have no window.
    borealis: Option<gpui::Entity<Borealis>>,
    home_widgets: Option<gpui::Entity<HomeWidgets>>,
    client_updates: Option<gpui::Entity<ClientUpdates>>,
    instance_editor: Option<gpui::Entity<InstanceEditor>>,
    instance_creator: Option<gpui::Entity<InstanceCreator>>,
    settings: Option<gpui::Entity<Settings>>,
    content: Option<gpui::Entity<Content>>,
    discover: Option<gpui::Entity<Discover>>,
    downloads: Option<gpui::Entity<Downloads>>,
    syncing: Option<gpui::Entity<Syncing>>,
    accounts_page: Option<gpui::Entity<Accounts>>,
    skins: Option<gpui::Entity<Skins>>,
    skin_preview: Option<gpui::Entity<SkinPreview>>,
    pack_import: Option<gpui::Entity<PackImport>>,
    _subscriptions: Vec<gpui::Subscription>,
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    tx: async_channel::Sender<Message>,
    page: u8,
    serial: u64,
    busy: bool,
    account_busy: bool,
    skin_busy: bool,
    client_update_busy: bool,
    downloads_busy: bool,
    home_busy: bool,
    pack_import_busy: bool,
    load_in_flight: bool,
    load_pending: bool,
    /// The next load issued must run the startup sequence (a superseded or
    /// stale startup attempt hands that duty to its successor).
    startup_pending: bool,
    /// The launcher state has been read at least once by a startup load.
    loaded: bool,
    /// The startup load failed; replaces the pages with an error and Retry.
    fatal_error: Option<String>,
    update_check_started: bool,
    /// Latest `update-status` payload, replayed into Settings when it is built.
    update_status: Option<serde_json::Value>,
    readiness_in_flight: bool,
    readiness_pending: bool,
    readiness_rechecks: u8,
    launcher: Option<api::LauncherState>,
    accounts: api::AccountsState,
    appearance: Option<api::Appearance>,
    readiness: Option<api::PlayReadiness>,
    processes: HashMap<String, api::LaunchProcess>,
    process_serial: u64,
    recovery: Option<api::RecoveryPreview>,
    load_error: Option<String>,
    sync_error: Option<String>,
    sync_running: bool,
    status: String,
}
impl Launcher {
    fn home_running(&self, cx: &gpui::App) -> bool {
        self.home_busy
            || self
                .home_widgets
                .as_ref()
                .is_some_and(|view| view.read(cx).is_busy())
    }

    fn downloads_running(&self, cx: &gpui::App) -> bool {
        self.downloads_busy
            || self
                .downloads
                .as_ref()
                .is_some_and(|view| view.read(cx).is_busy())
    }

    fn client_update_running(&self, cx: &gpui::App) -> bool {
        self.client_update_busy
            || self
                .client_updates
                .as_ref()
                .is_some_and(|view| view.read(cx).is_busy())
    }

    fn client_update_target(&self) -> Option<api::InstanceSummary> {
        let id = self.selected_id()?;
        self.launcher
            .as_ref()?
            .instances
            .iter()
            .find(|instance| instance.id == id)
            .cloned()
    }

    /// Feeds the Borealis layer from the current appearance snapshot, so the
    /// saved background and speed apply at startup (and after every reload)
    /// even if Settings is never opened. Setters that change nothing are free.
    fn sync_borealis(&self, cx: &mut Context<Self>) {
        let Some(layer) = &self.borealis else {
            return;
        };
        let settings_speed = self
            .settings
            .as_ref()
            .and_then(|settings| settings.read(cx).motion_speed());
        let (enabled, speed) = borealis_inputs(self.appearance.as_ref(), settings_speed);
        layer.update(cx, |layer, cx| {
            layer.set_background(enabled, cx);
            layer.set_motion(speed, cx);
        });
    }

    fn update_skin_preview(&self, cx: &mut Context<Self>) {
        if let (Some(skins), Some(preview)) = (&self.skins, &self.skin_preview) {
            let (skin, cape, rear) = skins.read(cx).preview_appearance();
            preview.update(cx, |preview, cx| {
                preview.set_appearance(skin, cape, rear, cx)
            });
        }
    }

    fn supply_skin_accounts(&self, cx: &mut Context<Self>) {
        if let Some(skins) = &self.skins {
            skins.update(cx, |skins, cx| skins.set_accounts(&self.accounts, cx));
            // Account invalidation can clear an inspected appearance synchronously.
            self.update_skin_preview(cx);
        }
    }

    fn sync_mutation_guards(&self, cx: &mut Context<Self>) {
        let client_update_busy = self.client_update_running(cx);
        let home_busy = self.home_running(cx);
        if let Some(downloads) = &self.downloads {
            downloads.update(cx, |downloads, cx| {
                downloads.set_blocked(self.busy || client_update_busy || home_busy, cx)
            });
        }
        let account_busy = self.account_busy
            || self
                .accounts_page
                .as_ref()
                .is_some_and(|view| view.read(cx).is_busy());
        let skin_busy = self.skin_busy
            || self
                .skins
                .as_ref()
                .is_some_and(|view| view.read(cx).is_busy());
        if let Some(accounts) = &self.accounts_page {
            accounts.update(cx, |accounts, cx| {
                accounts.set_blocked(
                    self.busy || skin_busy || client_update_busy || home_busy,
                    cx,
                )
            });
        }
        if let Some(skins) = &self.skins {
            skins.update(cx, |skins, cx| {
                skins.set_blocked(
                    self.busy
                        || account_busy
                        || self.load_in_flight
                        || client_update_busy
                        || home_busy,
                    cx,
                )
            });
        }
        if let Some(import) = &self.pack_import {
            let blocked = self.busy || client_update_busy || home_busy;
            import.update(cx, |import, cx| import.set_blocked(blocked, cx));
        }
        if let Some(updates) = &self.client_updates {
            let target = self.client_update_target();
            let game_running = target.as_ref().is_some_and(|target| {
                self.processes.get(&target.id).is_some_and(|process| {
                    matches!(process.status.as_str(), "starting" | "running")
                })
            });
            let blocked = self.busy
                || account_busy
                || skin_busy
                || home_busy
                || self.pack_import_busy
                || self.load_in_flight
                || self.readiness_in_flight
                || self.sync_running
                || game_running
                || self.downloads_running(cx)
                || self
                    .pack_import
                    .as_ref()
                    .is_some_and(|view| view.read(cx).is_busy())
                || self
                    .discover
                    .as_ref()
                    .is_some_and(|view| view.read(cx).is_busy());
            updates.update(cx, |updates, cx| {
                updates.set_target(target, self.serial, cx);
                updates.set_blocked(blocked, cx);
            });
        }
        if let Some(home) = &self.home_widgets {
            let blocked = self.busy
                || account_busy
                || skin_busy
                || client_update_busy
                || self.load_in_flight
                || self.sync_running
                || self.downloads_running(cx)
                || self.pack_import_busy
                || self
                    .pack_import
                    .as_ref()
                    .is_some_and(|view| view.read(cx).is_busy());
            home.update(cx, |home, cx| {
                if self.page != 0 {
                    home.set_active(false, cx);
                }
                home.set_blocked(blocked, cx);
                if self.page == 0 {
                    home.set_active(true, cx);
                }
            });
        }
    }

    fn initialize_sync(&self) {
        let Some(host) = self.host.clone().filter(|_| !showcase()) else {
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let result = guarded(api::initialize_sync(host)).await;
            let _ = tx.send(Message::SyncInitialized(result)).await;
        });
    }

    fn refresh_content_views(&self, cx: &mut Context<Self>) {
        if let Some(content) = &self.content {
            content.update(cx, |content, cx| content.refresh(cx));
        }
        if let Some(discover) = &self.discover {
            discover.update(cx, |discover, cx| discover.content_changed(cx));
        }
    }

    fn reload(&mut self) {
        self.begin_load(false);
    }

    /// The startup sequence: application status, then launcher state, accounts
    /// and appearance, then readiness and the one-time update check.
    fn start_load(&mut self) {
        self.begin_load(true);
    }

    /// Re-runs the whole startup sequence after a fatal load error. Replies of
    /// the failed attempt are dropped by the serial.
    fn retry_load(&mut self) {
        self.fatal_error = None;
        self.load_error = None;
        self.start_load();
    }

    /// Bumps the serial (superseding every earlier load) and decides whether a
    /// load starts now or coalesces into a single follow-up.
    fn plan_load(&mut self, startup: bool) -> LoadPlan {
        self.serial += 1;
        self.readiness = None;
        self.startup_pending |= startup;
        if self.load_in_flight {
            self.load_pending = true;
            return LoadPlan::Coalesced;
        }
        self.load_in_flight = true;
        self.load_pending = false;
        LoadPlan::Run {
            serial: self.serial,
            startup: std::mem::take(&mut self.startup_pending),
        }
    }

    fn begin_load(&mut self, startup: bool) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let LoadPlan::Run { serial, startup } = self.plan_load(startup) else {
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let status = if startup {
                let host = host.clone();
                Some(guarded(async move { api::application_status(&host).map(|_| ()) }).await)
            } else {
                None
            };
            // Without an application status nothing else is read; the error is final.
            let snapshot = if let Some(Err(error)) = &status {
                Snapshot {
                    launcher: Err(error.clone()),
                    accounts: Err(error.clone()),
                    appearance: Err(error.clone()),
                    status,
                }
            } else {
                let launcher = guarded(api::launcher_state(host.clone())).await;
                let accounts = guarded(api::accounts(host.clone())).await;
                let appearance = guarded(api::appearance(host)).await;
                Snapshot {
                    status,
                    launcher,
                    accounts,
                    appearance,
                }
            };
            let _ = tx.send(Message::Loaded(serial, snapshot)).await;
        });
    }

    /// Production only: with no Host (diagnostic mode) there is nothing to
    /// check and no message is ever sent. The core runs this check once per
    /// process, so it is requested once here as well.
    fn start_update_check(&mut self) {
        if self.update_check_started || showcase() {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        self.update_check_started = true;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let result = guarded(api::startup_update_check(host)).await;
            let _ = tx.send(Message::StartupUpdates(result)).await;
        });
    }

    fn reload_guarded(&mut self, cx: &mut Context<Self>) {
        self.reload();
        self.sync_mutation_guards(cx);
    }
    fn readiness(&mut self) {
        self.readiness = None;
        self.begin_readiness();
    }

    /// Counts one stale-running re-check; false once the limit is reached.
    fn take_recheck(&mut self) -> bool {
        if self.readiness_rechecks >= READINESS_RECHECK_LIMIT {
            return false;
        }
        self.readiness_rechecks += 1;
        true
    }

    /// A single timer, not a poll: it fires once and its reply is dropped if
    /// anything invalidated readiness in the meantime.
    fn schedule_readiness_recheck(&self) {
        let tx = self.tx.clone();
        let serial = self.serial;
        self.rt.spawn(async move {
            tokio::time::sleep(READINESS_RECHECK_DELAY).await;
            let _ = tx.send(Message::RecheckReadiness(serial)).await;
        });
    }

    /// Re-reads the decision without clearing the one on screen.
    fn begin_readiness(&mut self) {
        if self.readiness_in_flight {
            self.readiness_pending = true;
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(id) = self
            .launcher
            .as_ref()
            .and_then(|s| s.config.selected_instance_id.clone())
        else {
            return;
        };
        self.readiness_pending = false;
        self.readiness_in_flight = true;
        let account = self.accounts.selected_account_id.clone();
        let serial = self.serial;
        let process_serial = self.process_serial;
        let tx = self.tx.clone();
        if showcase() {
            let decision = api::PlayReadiness {
                ready: true,
                can_prepare: false,
                instance_id: id,
                account_name: self.accounts.selected_account_id.as_ref().and_then(|_| {
                    self.accounts.accounts.first().map(|a| a.minecraft_name.clone())
                }),
                account_id: account,
                instance_status: "ready".into(),
                runtime_status: "ready".into(),
                account_status: "ready".into(),
                process_status: "idle".into(),
                blockers: Vec::new(),
            };
            self.rt.spawn(async move {
                let _ = tx
                    .send(Message::Readiness(serial, process_serial, Ok(decision), None))
                    .await;
            });
            return;
        }
        self.rt.spawn(async move {
            let result = guarded(api::play_readiness(host, id.clone(), account)).await;
            let process = guarded(api::launch_state(id)).await.ok();
            let _ = tx
                .send(Message::Readiness(serial, process_serial, result, process))
                .await;
        });
    }

    fn selected_id(&self) -> Option<&str> {
        let launcher = self.launcher.as_ref()?;
        let id = launcher.config.selected_instance_id.as_deref()?;
        launcher
            .instances
            .iter()
            .any(|instance| instance.id == id)
            .then_some(id)
    }

    fn can_play(&self, id: &str) -> bool {
        let Some(account) = self.accounts.selected_account_id.as_deref() else {
            return false;
        };
        !self.busy
            && !self.account_busy
            && !self.skin_busy
            && !self.client_update_busy
            && !self.downloads_busy
            && !self.home_busy
            && !self.pack_import_busy
            && !self.load_in_flight
            && self.selected_id() == Some(id)
            && self.readiness.as_ref().is_some_and(|decision| {
                (decision.ready || decision.can_prepare)
                    && decision.instance_id == id
                    && decision.account_id.as_deref() == Some(account)
            })
            && !self
                .processes
                .get(id)
                .is_some_and(|process| matches!(process.status.as_str(), "starting" | "running"))
    }

    /// The supervised child's own snapshot is newer than a readiness decision
    /// taken while it ran, so it decides process state whenever it exists.
    fn game_phase(&self, id: &str) -> GamePhase {
        let child = self.processes.get(id).map(|child| child.status.as_str());
        let finished = matches!(child, Some("exited" | "failed"));
        let decision = self
            .readiness
            .as_ref()
            .filter(|decision| decision.instance_id == id && !finished)
            .map(|decision| decision.process_status.as_str());
        if child == Some("running") || decision == Some("running") {
            GamePhase::Running
        } else if child == Some("starting") || decision == Some("starting") {
            GamePhase::Starting
        } else {
            GamePhase::Idle
        }
    }

    fn play_label(&self, id: &str) -> &'static str {
        match self.game_phase(id) {
            GamePhase::Running => "Running",
            GamePhase::Starting => "Starting…",
            GamePhase::Idle => {
                let preparable = self
                    .readiness
                    .as_ref()
                    .is_some_and(|decision| decision.instance_id == id && decision.can_prepare);
                if preparable { "Prepare & play" } else { "Play" }
            }
        }
    }

    /// Blockers are explained only for the selected instance's own decision and
    /// never while its game is already starting or running.
    fn visible_blockers(&self, id: &str) -> &[api::PlayBlocker] {
        match &self.readiness {
            Some(decision)
                if decision.instance_id == id && self.game_phase(id) == GamePhase::Idle =>
            {
                &decision.blockers
            }
            _ => &[],
        }
    }

    /// Home selection: a registered instance other than the current one, and
    /// no operation that selection would race with.
    fn can_select(&self, id: &str) -> bool {
        !self.busy
            && !self.account_busy
            && !self.skin_busy
            && !self.client_update_busy
            && !self.downloads_busy
            && !self.home_busy
            && !self.pack_import_busy
            && self.selected_id() != Some(id)
            && self
                .launcher
                .as_ref()
                .is_some_and(|state| state.instances.iter().any(|i| i.id == id))
    }

    fn run_blocker_action(
        &mut self,
        action: BlockerAction,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            BlockerAction::InstallJava => self.action("runtime", id, cx),
            BlockerAction::ReviewRecovery => {
                self.open_page(3, window, cx);
                // The editor owns the actionable review; Home keeps its
                // read-only preview only when the editor cannot take it.
                let taken = self.instance_editor.as_ref().is_some_and(|editor| {
                    editor.update(cx, |editor, cx| editor.review_recovery(&id, cx))
                });
                if !taken {
                    self.inspect_recovery(id);
                }
            }
            other => {
                if let Some(page) = other.page() {
                    self.open_page(page, window, cx);
                }
            }
        }
    }

    fn inspect_recovery(&self, id: String) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.selected_id() != Some(id.as_str()) {
            return;
        }
        let serial = self.serial;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let result = guarded(api::inspect_recovery(host, id.clone())).await;
            let _ = tx.send(Message::Recovery(serial, id, result)).await;
        });
    }
    /// Quick Play may target a different registered instance. The core resolves
    /// the opaque destination and performs its own deep readiness verification.
    fn can_quick_launch(&self, id: &str) -> bool {
        self.page == 0
            && !id.is_empty()
            && !self.busy
            && !self.account_busy
            && !self.skin_busy
            && !self.client_update_busy
            && !self.downloads_busy
            && !self.home_busy
            && !self.pack_import_busy
            && !self.load_in_flight
            && !self.sync_running
            && self
                .launcher
                .as_ref()
                .is_some_and(|state| state.instances.iter().any(|i| i.id == id))
            && self
                .accounts
                .selected_account_id
                .as_deref()
                .is_some_and(|id| {
                    self.accounts
                        .accounts
                        .iter()
                        .any(|account| account.account_id == id && account.status == "signedIn")
                })
            && !self
                .processes
                .get(id)
                .is_some_and(|process| matches!(process.status.as_str(), "starting" | "running"))
    }

    fn quick_play(
        &mut self,
        mode: String,
        target_id: String,
        instance_id: String,
        cx: &mut Context<Self>,
    ) {
        if !self.busy {
            let instance_exists = self
                .launcher
                .as_ref()
                .is_some_and(|state| state.instances.iter().any(|i| i.id == instance_id));
            let account_selected = self.accounts.selected_account_id.is_some();
            if let Some(message) = quick_play_refusal(&mode, instance_exists, account_selected) {
                self.status = message.into();
                cx.notify();
                return;
            }
        }
        if !self.can_quick_launch(&instance_id)
            || self.home_running(cx)
            || self.client_update_running(cx)
            || self.downloads_running(cx)
            || self
                .pack_import
                .as_ref()
                .is_some_and(|view| view.read(cx).is_busy())
            || self
                .accounts_page
                .as_ref()
                .is_some_and(|view| view.read(cx).is_busy())
            || self
                .skins
                .as_ref()
                .is_some_and(|view| view.read(cx).is_busy())
            || !self.home_widgets.as_ref().is_some_and(|home| {
                home.read(cx)
                    .can_quick_play(&mode, &target_id, &instance_id)
            })
        {
            return;
        }
        let (Some(host), Some(account)) =
            (self.host.clone(), self.accounts.selected_account_id.clone())
        else {
            return;
        };
        self.busy = true;
        self.serial = self.serial.wrapping_add(1);
        self.readiness = None;
        let server_name = self
            .home_widgets
            .as_ref()
            .and_then(|home| home.read(cx).quick_play_server_name(&mode, &target_id));
        self.status = quick_play_progress(&mode, server_name.as_deref());
        self.sync_mutation_guards(cx);
        let tx = self.tx.clone();
        let process_serial = self.process_serial;
        self.rt.spawn(async move {
            let result = guarded(api::quick_play(host, mode, target_id, account)).await;
            let _ = tx
                .send(Message::Launched(instance_id, process_serial, result))
                .await;
        });
    }

    fn action(&mut self, op: &'static str, id: String, cx: &mut Context<Self>) {
        if self.busy
            || self.account_busy
            || self.skin_busy
            || self.client_update_running(cx)
            || self.downloads_running(cx)
            || self.home_running(cx)
            || self.pack_import_busy
            || self
                .pack_import
                .as_ref()
                .is_some_and(|page| page.read(cx).is_busy())
            || self
                .accounts_page
                .as_ref()
                .is_some_and(|page| page.read(cx).is_busy())
            || self
                .skins
                .as_ref()
                .is_some_and(|page| page.read(cx).is_busy())
        {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        if (op == "play" && !self.can_play(&id)) || (op == "select" && !self.can_select(&id)) {
            return;
        }
        self.busy = true;
        self.sync_mutation_guards(cx);
        self.serial += 1;
        self.readiness = None;
        self.status = if op == "runtime" {
            "Installing Java…"
        } else {
            "Working…"
        }
        .into();
        let tx = self.tx.clone();
        let account = self.accounts.selected_account_id.clone();
        let process_serial = self.process_serial;
        self.rt.spawn(async move {
            if op == "play" {
                let result = match account {
                    Some(account) => guarded(api::play(host, id.clone(), account)).await,
                    None => Err(api::ApiError::new(
                        "account_required",
                        "Select an account first.",
                    )),
                };
                let _ = tx.send(Message::Launched(id, process_serial, result)).await;
                return;
            }
            let result = guarded(async move {
                match op {
                    "select" => api::select_instance(host, id).await,
                    "runtime" => api::ensure_runtime(host, id).await.map(|_| ()),
                    "retry" => api::retry_install(host, id).await.map(|_| ()),
                    _ => unreachable!(),
                }
            })
            .await;
            let _ = tx.send(Message::Done(result)).await;
        });
    }
    /// Content's "Find more": Discover for one instance and content kind.
    /// Goes through `open_page`, so its navigation guard and Discover's lazy
    /// construction apply; a refused navigation opens nothing.
    fn open_discover_for(
        &mut self,
        instance: String,
        kind: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_page(5, window, cx);
        if self.page != 5 {
            return;
        }
        if let Some(discover) = &self.discover {
            discover.update(cx, |discover, cx| discover.open_for(instance, kind, cx));
        }
    }
    /// Discover's "Back to instance": Content with that instance and kind.
    fn return_to_content(
        &mut self,
        instance: String,
        kind: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_page(4, window, cx);
        if self.page != 4 {
            return;
        }
        if let Some(content) = &self.content {
            content.update(cx, |content, cx| content.show(instance, kind, cx));
        }
    }
    fn open_page(&mut self, page: u8, window: &mut Window, cx: &mut Context<Self>) {
        if !navigation_accepted(self.page, page, self.client_update_running(cx)) {
            return;
        }
        // An unreleased Motion draft and an open accent picker never outlive a
        // navigation, including the one that leaves Settings.
        if let Some(settings) = &self.settings {
            settings.update(cx, |settings, cx| settings.cancel_previews(cx));
        }
        if page == 7 && self.skins.is_none() {
            let preview = cx.new(|cx| SkinPreview::new(self.rt.clone(), window, cx));
            let skins = cx.new(|cx| Skins::new(self.host.clone(), self.rt.clone(), window, cx));
            self._subscriptions
                .push(cx.subscribe(&skins, |this, _, event: &SkinsEvent, cx| {
                    match event {
                        SkinsEvent::Changed => {
                            this.reload_guarded(cx);
                            if let Some(accounts) = &this.accounts_page {
                                accounts.update(cx, |accounts, cx| accounts.refresh(cx));
                            }
                        }
                        SkinsEvent::BusyChanged(busy) => this.skin_busy = *busy,
                        SkinsEvent::PreviewChanged => this.update_skin_preview(cx),
                    }
                    this.sync_mutation_guards(cx);
                    cx.notify();
                }));
            self.skin_preview = Some(preview);
            self.skins = Some(skins);
            if !self.load_in_flight && !self.account_busy {
                self.supply_skin_accounts(cx);
            }
        }
        if (self.page == 7) != (page == 7) {
            if let Some(skins) = &self.skins {
                skins.update(cx, |skins, cx| skins.set_active(page == 7, cx));
            }
            if let Some(preview) = &self.skin_preview {
                preview.update(cx, |preview, cx| preview.set_active(page == 7, cx));
            }
        }
        if page == 1 {
            if let Some(accounts) = &self.accounts_page {
                if self.page != page {
                    accounts.update(cx, |accounts, cx| accounts.refresh(cx));
                }
            } else {
                let accounts = cx.new(|cx| {
                    let mut accounts =
                        Accounts::new(self.host.clone(), self.rt.clone(), window, cx);
                    accounts.set_blocked(self.busy || self.skin_busy, cx);
                    accounts
                });
                self._subscriptions.push(cx.subscribe(
                    &accounts,
                    |this, _, event: &AccountsEvent, cx| {
                        match event {
                            AccountsEvent::Changed => this.reload_guarded(cx),
                            AccountsEvent::BusyChanged(busy) => {
                                this.account_busy = *busy;
                                if *busy {
                                    this.serial = this.serial.wrapping_add(1);
                                    this.readiness = None;
                                    if let Some(skins) = &this.skins {
                                        skins.update(cx, |skins, cx| {
                                            skins.set_accounts(&Default::default(), cx)
                                        });
                                        this.update_skin_preview(cx);
                                    }
                                }
                            }
                        }
                        this.sync_mutation_guards(cx);
                        cx.notify();
                    },
                ));
                self.accounts_page = Some(accounts);
            }
        }
        if page == 6 {
            if let Some(syncing) = &self.syncing {
                if self.page != page {
                    syncing.update(cx, |syncing, cx| syncing.refresh(cx));
                }
            } else {
                let syncing =
                    cx.new(|cx| Syncing::new(self.host.clone(), self.rt.clone(), window, cx));
                self._subscriptions.push(cx.subscribe(
                    &syncing,
                    |this, _, _: &SyncingEvent, cx| {
                        this.sync_error = None;
                        this.reload_guarded(cx);
                        if let Some(editor) = &this.instance_editor {
                            editor.update(cx, |editor, cx| editor.refresh(cx));
                        }
                        this.refresh_content_views(cx);
                        cx.notify();
                    },
                ));
                self.syncing = Some(syncing);
            }
        }
        if page == 5 || page == 8 {
            if let Some(discover) = &self.discover {
                if self.page != page {
                    discover.update(cx, |discover, cx| discover.refresh(cx));
                }
            } else {
                let discover =
                    cx.new(|cx| Discover::new(self.host.clone(), self.rt.clone(), window, cx));
                let downloads = discover.read(cx).downloads();
                self._subscriptions.push(cx.subscribe(
                    &downloads,
                    |this, downloads, event: &DownloadsEvent, cx| {
                        // The Slint host toasts a settled install wherever the
                        // user is; the status line is this host's equivalent.
                        if matches!(event, DownloadsEvent::Settled { .. }) {
                            if let Some(notice) = downloads.read(cx).notice() {
                                this.status = notice.to_owned();
                            }
                        }
                        this.downloads_busy = downloads.read(cx).is_busy();
                        this.sync_mutation_guards(cx);
                        cx.notify();
                    },
                ));
                self.downloads = Some(downloads);
                self._subscriptions.push(cx.subscribe_in(
                    &discover,
                    window,
                    |this, _, event: &DiscoverEvent, window, cx| {
                        match event {
                            DiscoverEvent::Changed => {
                                this.reload_guarded(cx);
                                if let Some(editor) = &this.instance_editor {
                                    editor.update(cx, |editor, cx| editor.refresh(cx));
                                }
                                if let Some(content) = &this.content {
                                    content.update(cx, |content, cx| content.refresh(cx));
                                }
                            }
                            DiscoverEvent::ReturnToInstance { instance, kind } => {
                                this.return_to_content(instance.clone(), *kind, window, cx);
                            }
                        }
                        cx.notify();
                    },
                ));
                discover.update(cx, |discover, cx| discover.refresh(cx));
                self.discover = Some(discover);
            }
        }
        if page == 4 {
            if let Some(content) = &self.content {
                if self.page != page {
                    content.update(cx, |content, cx| content.refresh(cx));
                }
            } else {
                let content =
                    cx.new(|cx| Content::new(self.host.clone(), self.rt.clone(), window, cx));
                self._subscriptions.push(cx.subscribe_in(
                    &content,
                    window,
                    |this, _, event: &ContentEvent, window, cx| {
                        match event {
                            ContentEvent::Changed => {
                                this.reload_guarded(cx);
                                if let Some(discover) = &this.discover {
                                    discover
                                        .update(cx, |discover, cx| discover.content_changed(cx));
                                }
                                if let Some(editor) = &this.instance_editor {
                                    editor.update(cx, |editor, cx| editor.refresh(cx));
                                }
                            }
                            ContentEvent::Discover { instance, kind } => {
                                this.open_discover_for(instance.clone(), *kind, window, cx);
                            }
                        }
                        cx.notify();
                    },
                ));
                self.content = Some(content);
            }
        }
        if page == 2 && self.settings.is_none() {
            let settings =
                cx.new(|cx| Settings::new(self.host.clone(), self.rt.clone(), window, cx));
            self._subscriptions.push(cx.subscribe(
                &settings,
                |this, settings, event: &SettingsEvent, cx| {
                    // The acknowledged state plus any live accent preview, so
                    // the root's `theme::apply` never overwrites a preview.
                    this.appearance = settings.read(cx).display_appearance();
                    let background = borealis_inputs(this.appearance.as_ref(), None).0;
                    if let Some(layer) = &this.borealis {
                        layer.update(cx, |layer, cx| match event {
                            SettingsEvent::Changed | SettingsEvent::Preview => {
                                layer.set_background(background, cx)
                            }
                            SettingsEvent::Motion(speed) => layer.set_motion(*speed, cx),
                        });
                    }
                    // Invalidate older root snapshots before they can restore an old theme.
                    this.reload_guarded(cx);
                    cx.notify();
                },
            ));
            if let (Some(layer), Some(speed)) = (&self.borealis, settings.read(cx).motion_speed()) {
                layer.update(cx, |layer, cx| layer.set_motion(speed, cx));
            }
            // The startup check usually finishes before Settings first exists.
            if let Some(payload) = &self.update_status {
                settings.update(cx, |settings, cx| {
                    settings.on_host_event("update-status", payload, cx)
                });
            }
            self.settings = Some(settings);
        }
        self.page = page;
        self.sync_mutation_guards(cx);
    }
    fn receive(&mut self, msg: Message) {
        match msg {
            Message::Loaded(n, s) => {
                let startup = s.status.is_some();
                self.load_in_flight = false;
                if self.load_pending {
                    // A superseded startup attempt hands its duties to the follow-up.
                    self.startup_pending |= startup;
                    self.reload();
                    return;
                }
                if n != self.serial {
                    self.startup_pending |= startup;
                    return;
                }
                if let Some(Err(error)) = s.status {
                    self.fatal_error = Some(error.message);
                    return;
                }
                if startup {
                    if let Err(error) = &s.launcher {
                        self.launcher = None;
                        self.fatal_error = Some(error.message.clone());
                        return;
                    }
                    self.fatal_error = None;
                    self.loaded = true;
                }
                self.load_error = None;
                match s.launcher {
                    Ok(v) => self.launcher = Some(v),
                    Err(e) => {
                        self.launcher = None;
                        self.load_error = Some(e.message);
                    }
                }
                match s.accounts {
                    Ok(v) => self.accounts = v,
                    Err(e) => {
                        self.accounts = Default::default();
                        self.load_error = Some(e.message);
                    }
                }
                match s.appearance {
                    Ok(v) => self.appearance = Some(v),
                    Err(e) => {
                        self.appearance = None;
                        self.load_error = Some(e.message);
                    }
                }
                self.readiness();
                if startup {
                    self.start_update_check();
                }
            }
            Message::Readiness(n, process_serial, result, process) => {
                self.readiness_in_flight = false;
                let current = n == self.serial;
                let mut stale = false;
                if current {
                    if process_serial == self.process_serial {
                        if let Some(process) = process {
                            self.processes.insert(process.instance_id.clone(), process);
                        }
                    }
                    match result {
                        Ok(v)
                            if self.selected_id() == Some(v.instance_id.as_str())
                                && self.accounts.selected_account_id == v.account_id =>
                        {
                            stale = stale_running(&v, self.processes.get(&v.instance_id));
                            self.readiness = Some(v)
                        }
                        Ok(_) => {}
                        Err(e) => self.status = e.message,
                    }
                    if !stale {
                        self.readiness_rechecks = 0;
                    }
                }
                if self.readiness_pending && !self.busy && !self.account_busy {
                    self.readiness_pending = false;
                    self.readiness();
                } else if stale && self.take_recheck() {
                    self.schedule_readiness_recheck();
                }
            }
            Message::RecheckReadiness(serial) => {
                if serial == self.serial {
                    self.begin_readiness();
                }
            }
            Message::StartupUpdates(result) => {
                // Failures are ignored like the Slint host; a newer event wins.
                if let (Ok(overview), None) = (result, &self.update_status) {
                    self.update_status = Some(update_overview_payload(&overview));
                }
            }
            Message::Launched(id, process_serial, result) => {
                self.busy = false;
                let failed = result.is_err();
                match result {
                    Ok(process) if process_serial == self.process_serial => {
                        self.processes.insert(id.clone(), process);
                        self.status.clear();
                    }
                    Ok(_) => {}
                    Err(error) => self.status = error.message,
                }
                self.reload();
                if failed
                    || self
                        .processes
                        .get(&id)
                        .is_some_and(|process| process.status == "failed")
                {
                    self.inspect_recovery(id);
                }
            }
            Message::Recovery(serial, id, result) => {
                if serial == self.serial && self.selected_id() == Some(id.as_str()) {
                    if let Ok(preview) = result {
                        self.recovery = Some(preview);
                    }
                }
            }
            Message::Done(result) => {
                self.busy = false;
                self.status = match result {
                    Ok(()) => String::new(),
                    Err(e) => e.message,
                };
                self.reload();
            }
            Message::SyncInitialized(result) => {
                self.sync_error = result
                    .err()
                    .map(|e| format!("Sync initialization: {}", e.message));
            }
            Message::Event(name, payload) => {
                if name == "instance-sync-state" {
                    if let Ok(status) = serde_json::from_value::<api::SyncStatus>(payload) {
                        let finished = self.sync_running && !status.running;
                        self.sync_running = status.running;
                        if finished {
                            self.reload();
                        }
                    }
                    return;
                }
                if name == "launch-state" {
                    if let Ok(process) = serde_json::from_value::<api::LaunchProcess>(payload) {
                        let id = process.instance_id.clone();
                        let failed = process.status == "failed";
                        let terminal = failed || process.status == "exited";
                        self.process_serial += 1;
                        self.processes.insert(id.clone(), process);
                        if self.selected_id() == Some(id.as_str()) {
                            self.readiness = None;
                            if terminal && !self.busy && !self.account_busy {
                                self.reload();
                                if failed {
                                    self.inspect_recovery(id);
                                }
                            }
                        }
                    }
                    return;
                }
                if name == "update-status" {
                    // Settings may not exist yet; keep the latest well-formed status.
                    if settings::update_status_event(&name, &payload).is_some() {
                        self.update_status = Some(payload);
                    }
                    return;
                }
                if payload
                    .get("phase")
                    .and_then(serde_json::Value::as_str)
                    .is_none()
                {
                    return;
                }
                self.status = match name.as_str() {
                    "launch-progress" if self.busy => "Preparing launch…",
                    "install-progress" | "instance-progress" if self.busy => "Installing…",
                    "runtime-progress" if self.busy => "Preparing Java…",
                    _ => return,
                }
                .into();
            }
        }
    }
    /// Home: the selected instance with Play as the one primary action, why it
    /// cannot play, and a compact picker for switching instance.
    fn home_children(
        &self,
        palette: &theme::AuroraTheme,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let muted = palette.muted_foreground;
        let mut out: Vec<gpui::AnyElement> = Vec::new();
        let Some(launcher) = &self.launcher else {
            let text = if self.host.is_some() && !self.loaded {
                "Loading launcher state…"
            } else {
                "Launcher data is not loaded."
            };
            out.push(div().text_color(muted).child(text).into_any_element());
            return out;
        };
        if launcher.instances.is_empty() {
            out.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_2xl().child("No instance yet"))
                    .child(div().text_sm().text_color(muted).child(
                        "Create an instance to install Minecraft and Aurora. Each instance keeps its own mods, worlds and settings.",
                    ))
                    .child(
                        div().flex().child(
                            self.button("create-instance", "Create instance")
                                .variant(ButtonVariant::Primary)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_page(3, window, cx);
                                    cx.notify();
                                })),
                        ),
                    )
                    .into_any_element(),
            );
            return out;
        }
        let selected = self.selected_id().map(str::to_owned);
        let instance = selected
            .as_deref()
            .and_then(|id| launcher.instances.iter().find(|i| i.id == id));
        let t = palette;
        let mut details = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .flex_1()
            .min_w(px(0.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(ui::eyebrow(t, "SELECTED INSTANCE"))
                    .child(
                        div()
                            .text_size(px(ui::TEXT_DISPLAY))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(t.foreground)
                            .text_ellipsis()
                            .child(instance.map_or_else(
                                || "Choose an instance".to_owned(),
                                |i| i.display_name.clone(),
                            )),
                    ),
            );
        if let Some(instance) = instance {
            let mut badges = div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .child(ui::badge(
                    t,
                    format!("Minecraft {}", instance.minecraft_version),
                    ui::Tone::Neutral,
                ))
                .child(ui::badge(t, instance.platform_label(), ui::Tone::Neutral));
            if let Some(aurora) = &instance.aurora {
                badges = badges.child(ui::badge(
                    t,
                    format!("Aurora {}", aurora.version),
                    ui::Tone::Accent,
                ));
            }
            if let Some(pack) = &instance.pack {
                badges = badges.child(ui::badge(
                    t,
                    format!("{} {}", pack.name, pack.pack_version),
                    ui::Tone::Neutral,
                ));
            }
            if let Some((label, tone)) = self.readiness_badge(&instance.id) {
                badges = badges.child(ui::status_badge(t, label, tone));
            }
            details = details.child(badges);
        }
        if let Some(id) = selected.clone() {
            let play_id = id.clone();
            details = details.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pt(px(4.))
                    .child(
                        self.button("play", self.play_label(&id))
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Large)
                            .icon("play_arrow")
                            .disabled(!self.can_play(&id))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.action("play", play_id.clone(), cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        self.button("instance-settings", "Instance settings")
                            .size(ButtonSize::Large)
                            .icon("tune")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_page(3, window, cx);
                                cx.notify();
                            })),
                    ),
            );
        }
        let hero_icon = instance.map(|instance| {
            instance_icon(
                stored_icon(&launcher.icons, &instance.id),
                112.,
                true,
                palette,
            )
        });
        out.push(
            div()
                .flex()
                .items_center()
                .gap(px(28.))
                .p(px(28.))
                .rounded(px(16.))
                .bg(gpui::linear_gradient(
                    100.,
                    gpui::linear_color_stop(t.raised, 0.),
                    gpui::linear_color_stop(t.card, 1.),
                ))
                .border_1()
                .border_color(t.border_subtle)
                .children(hero_icon)
                .child(details)
                .into_any_element(),
        );
        if let Some(id) = selected.clone() {
            if let Some(process) = self.processes.get(&id) {
                let label = match process.status.as_str() {
                    "starting" => "Starting Minecraft…".into(),
                    "running" => "Minecraft is running.".into(),
                    "exited" => format!(
                        "Minecraft closed (exit code {}).",
                        process
                            .exit_code
                            .map_or("unknown".into(), |code| code.to_string())
                    ),
                    "failed" => process
                        .diagnostic
                        .as_ref()
                        .map(|diagnostic| diagnostic.message.clone())
                        .or_else(|| process.message.clone())
                        .unwrap_or_else(|| {
                            "Minecraft failed to start or exited unsuccessfully.".into()
                        }),
                    _ => String::new(),
                };
                if !label.is_empty() {
                    out.push(ui::muted(t, label).into_any_element());
                }
            }
            if self.readiness.is_none()
                && self.host.is_some()
                && (self.readiness_in_flight || self.load_in_flight)
            {
                out.push(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("Checking readiness…")
                        .into_any_element(),
                );
            }
            for (i, blocker) in self.visible_blockers(&id).iter().enumerate() {
                let action_button = BlockerAction::for_code(&blocker.code).map(|action| {
                    let target = id.clone();
                    self.button(("blocker", i), action.label())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.run_blocker_action(action, target.clone(), window, cx);
                            cx.notify();
                        }))
                });
                out.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .px(px(16.))
                        .py(px(12.))
                        .rounded(t.radius_lg)
                        .bg(gpui::Hsla { a: 0.08, ..t.warning })
                        .border_1()
                        .border_color(gpui::Hsla { a: 0.25, ..t.warning })
                        .child(symbols::icon(Some("warning"), px(18.), t.warning))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(ui::TEXT_BODY))
                                .text_color(t.foreground)
                                .child(blocker.message.clone()),
                        )
                        .children(action_button)
                        .into_any_element(),
                );
            }
            if instance.is_some_and(|i| i.state == "installing" || i.state == "failed") {
                let retry_id = id.clone();
                out.push(
                    self.button("retry", "Retry installation")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.action("retry", retry_id.clone(), cx);
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
            if let Some(recovery) = self
                .recovery
                .as_ref()
                .filter(|preview| preview.instance_id == id)
            {
                out.push(div().child(recovery.summary.clone()).into_any_element());
                for blocker in &recovery.blockers {
                    out.push(div().child(blocker.clone()).into_any_element());
                }
            }
        }
        if launcher.instances.len() > 1 || selected.is_none() {
            let mut tiles = div().flex().flex_wrap().gap(px(12.));
            for (i, item) in launcher.instances.iter().enumerate() {
                let id = item.id.clone();
                let chosen = selected.as_deref() == Some(item.id.as_str());
                tiles = tiles.child(
                    ui::tile_button(t, ("instance", i), chosen)
                        .w(px(248.))
                        .disabled(self.controls_blocked())
                        .child(instance_icon(
                            stored_icon(&launcher.icons, &item.id),
                            40.,
                            chosen,
                            palette,
                        ))
                        .child(
                            ui::title_block(
                                t,
                                item.display_name.clone(),
                                Some(
                                    format!(
                                        "{} · {}",
                                        item.minecraft_version,
                                        item.platform_label()
                                    )
                                    .into(),
                                ),
                            )
                            .flex_1(),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.action("select", id.clone(), cx);
                            cx.notify();
                        })),
                );
            }
            out.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(ui::section_header(t, "Your instances", None))
                    .child(tiles)
                    .into_any_element(),
            );
        }
        out
    }

    fn controls_blocked(&self) -> bool {
        self.busy
            || self.account_busy
            || self.skin_busy
            || self.client_update_busy
            || self.downloads_busy
            || self.home_busy
            || self.host.is_none()
    }

    fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: impl Into<gpui::SharedString>,
    ) -> AuroraButton {
        AuroraButton::new(id, label).disabled(self.controls_blocked())
    }

    /// The hero's one-word readiness summary for the selected instance.
    fn readiness_badge(&self, id: &str) -> Option<(&'static str, ui::Tone)> {
        match self.game_phase(id) {
            GamePhase::Running => return Some(("Running", ui::Tone::Success)),
            GamePhase::Starting => return Some(("Starting", ui::Tone::Accent)),
            GamePhase::Idle => {}
        }
        let decision = self
            .readiness
            .as_ref()
            .filter(|decision| decision.instance_id == id);
        match decision {
            Some(decision) if decision.ready => Some(("Ready to play", ui::Tone::Success)),
            Some(decision) if decision.can_prepare => Some(("Needs preparation", ui::Tone::Warning)),
            Some(_) => Some(("Action needed", ui::Tone::Warning)),
            None if self.readiness_in_flight || self.load_in_flight => {
                Some(("Checking", ui::Tone::Neutral))
            }
            None => None,
        }
    }
}
impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        theme::apply(self.appearance.as_ref(), cx);
        let palette = theme::theme(cx).clone();
        let downloads_active = self
            .downloads
            .as_ref()
            .map_or(0, |downloads| downloads.read(cx).active_count());
        let nav = [
            ("Home", 0, "home"),
            ("Instances", 3, "grid_view"),
            ("Discover", 5, "explore"),
            ("Content", 4, "extension"),
            ("Downloads", 8, "download"),
            ("Skins", 7, "checkroom"),
            ("Accounts", 1, "account_circle"),
            ("Sync", 6, "sync"),
            ("Settings", 2, "settings"),
        ]
        .into_iter()
        .map(|(label, page, icon)| {
            // The Downloads item doubles as the activity indicator (Slint's
            // downloads-nav): lit while work is queued or running, with a
            // count badge. It only reads the shared queue's count.
            let active = if page == 8 { downloads_active } else { 0 };
            let button = self
                .button(label, label)
                .variant(ButtonVariant::Nav)
                .icon(icon)
                .selected(self.page == page)
                .disabled(self.client_update_running(cx) && self.page != page)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_page(page, window, cx);
                    cx.notify();
                }));
            if page != 8 {
                return button.into_any_element();
            }
            div()
                .id("downloads-nav")
                .role(Role::Group)
                .aria_label(downloads::nav_label(active))
                .relative()
                .flex()
                .flex_col()
                .child(button)
                .when(active > 0, |item| {
                    item.child(
                        div()
                            .absolute()
                            .top(px(8.))
                            .right(px(10.))
                            .min_w(px(18.))
                            .h(px(18.))
                            .px(px(5.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(palette.accent)
                            .text_color(palette.accent_foreground)
                            .text_size(px(10.))
                            .child(active.to_string()),
                    )
                })
                .into_any_element()
        })
        .collect::<Vec<_>>();
        let mut nav = nav.into_iter();
        let mut nav_group = |title: Option<&'static str>, count: usize| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .when_some(title, |group, title| {
                    group.child(ui::eyebrow(&palette, title).px(px(10.)).pt(px(16.)).pb(px(6.)))
                })
                .children(nav.by_ref().take(count).collect::<Vec<_>>())
        };
        let play_group = nav_group(Some("PLAY"), 2);
        let library_group = nav_group(Some("LIBRARY"), 3);
        let profile_group = nav_group(Some("PROFILE"), 2);
        let system_group = nav_group(None, 2);
        let brand = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(52.))
            .px(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(30.))
                    .rounded(px(9.))
                    .bg(gpui::linear_gradient(
                        135.,
                        gpui::linear_color_stop(palette.ring, 0.),
                        gpui::linear_color_stop(palette.accent, 1.),
                    ))
                    .child(symbols::icon(
                        Some("auto_awesome"),
                        px(17.),
                        palette.accent_foreground,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Aurora"),
                    )
                    .child(ui::caption(&palette, "Launcher")),
            );
        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(ui::PAGE_GAP))
            .px(px(32.))
            .pt(px(4.))
            .pb(px(32.))
            .flex_1()
            .id("content")
            .overflow_y_scroll();
        if showcase() {
            // Design screenshots can start part-way down a long page.
            if let Some(y) = std::env::var("AURORA_SHOWCASE_SCROLL")
                .ok()
                .and_then(|y| y.parse::<f32>().ok())
            {
                let scroll = window
                    .use_keyed_state("showcase-scroll", cx, |_, _| gpui::ScrollHandle::new())
                    .read(cx)
                    .clone();
                scroll.set_offset(gpui::point(px(0.), px(-y)));
                content = content.track_scroll(&scroll);
            }
        }
        match self.page {
            // The startup load failed: nothing else is trustworthy until it succeeds.
            _ if self.fatal_error.is_some() => {
                content = content
                    .child(
                        div()
                            .text_2xl()
                            .child("Aurora could not load its launcher state"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(palette.muted_foreground)
                            .child(self.fatal_error.clone().unwrap_or_default()),
                    )
                    .child(
                        div().flex().child(
                            self.button("retry-load", "Try again")
                                .variant(ButtonVariant::Primary)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.retry_load();
                                    this.sync_mutation_guards(cx);
                                    cx.notify();
                                })),
                        ),
                    );
            }
            0 => {
                content = content.children(self.home_children(&palette, cx));
                if let Some(updates) = &self.client_updates {
                    content = content.child(updates.clone());
                }
                if let Some(home) = &self.home_widgets {
                    content = content.child(home.clone());
                }
            }
            1 => {
                if let Some(page) = &self.accounts_page {
                    content = content.child(page.clone());
                }
            }
            7 => {
                let mut appearance = div().flex().flex_wrap().gap_6();
                if let Some(preview) = &self.skin_preview {
                    appearance = appearance.child(preview.clone());
                }
                if let Some(skins) = &self.skins {
                    appearance =
                        appearance.child(div().flex_1().min_w(px(320.)).child(skins.clone()));
                }
                content = content.child(appearance);
            }
            3 => {
                if let Some(import) = &self.pack_import {
                    content = content.child(import.clone());
                }
                if let Some(creator) = &self.instance_creator {
                    content = content.child(creator.clone());
                }
                if let Some(editor) = &self.instance_editor {
                    content = content.child(editor.clone());
                }
            }
            4 => {
                if let Some(page) = &self.content {
                    content = content.child(page.clone());
                }
            }
            5 => {
                if let Some(page) = &self.discover {
                    content = content.child(page.clone());
                }
            }
            8 => {
                if let Some(downloads) = &self.downloads {
                    content = content.child(downloads.clone());
                }
            }
            6 => {
                if let Some(page) = &self.syncing {
                    content = content.child(page.clone());
                }
            }
            _ => {
                if let Some(settings) = &self.settings {
                    content = content.child(settings.clone());
                }
            }
        }
        if self.page == 3 && self.fatal_error.is_none() && self.pack_import.is_some() {
            // The whole Instances page takes a dropped `.mrpack`; the import
            // row inside it handles drops on itself first. The outline says
            // whether the dragged file would be accepted, like the row's.
            let (accept, reject) = (palette.accent, palette.danger);
            content = content
                .border_1()
                .border_color(gpui::transparent_black())
                .rounded(palette.radius)
                .drag_over::<ExternalPaths>(move |style, paths, _, _| {
                    style.border_color(
                        if pack_import::classify_modpack_drop(paths.paths()).is_ok() {
                            accept
                        } else {
                            reject
                        },
                    )
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                    // Same guards as the import button: the view refuses (and
                    // explains) a drop while an import or root action runs.
                    if let Some(import) = &this.pack_import {
                        import.update(cx, |import, cx| import.import_dropped(paths.paths(), cx));
                    }
                }));
        }
        div()
            .size_full()
            .flex()
            // The Borealis layer is absolute inside this root. It is painted
            // first, behind the sidebar, title strip and page content, and has
            // no mouse handlers or window control area of its own.
            .relative()
            .bg(palette.background)
            .text_color(palette.foreground)
            .font_family("Segoe UI")
            .children(self.borealis.clone())
            .child(
                div()
                    .w(px(212.))
                    .flex_shrink_0()
                    .h_full()
                    .px(px(12.))
                    .pt(px(6.))
                    .pb(px(12.))
                    .bg(palette.sidebar)
                    .border_r_1()
                    .border_color(palette.border_subtle)
                    .flex()
                    .flex_col()
                    // The header also moves the window (the Slint logo area).
                    .child(chrome::drag_region(brand.id("brand")))
                    .child(play_group)
                    .child(library_group)
                    .child(profile_group)
                    .child(div().flex_1())
                    .child(system_group)
                    .child(
                        self.button("refresh", "Refresh")
                            .variant(ButtonVariant::Nav)
                            .icon("refresh")
                            .disabled(
                                self.busy
                                    || self.account_busy
                                    || self.skin_busy
                                    || self.client_update_running(cx)
                                    || self.load_in_flight
                                    || self.host.is_none(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reload_guarded(cx);
                                if this.page == 0 {
                                    if let Some(home) = &this.home_widgets {
                                        home.update(cx, |home, cx| home.refresh(cx));
                                    }
                                }
                                if let Some(updates) = &this.client_updates {
                                    updates.update(cx, |updates, cx| updates.refresh(cx));
                                }
                                if this.page == 2 {
                                    if let Some(settings) = &this.settings {
                                        settings.update(cx, |settings, cx| settings.refresh(cx));
                                    }
                                }
                                if this.page == 4 {
                                    if let Some(content) = &this.content {
                                        content.update(cx, |content, cx| content.refresh(cx));
                                    }
                                }
                                if this.page == 5 {
                                    if let Some(discover) = &this.discover {
                                        discover.update(cx, |discover, cx| discover.refresh(cx));
                                    }
                                }
                                if this.page == 6 {
                                    if let Some(syncing) = &this.syncing {
                                        syncing.update(cx, |syncing, cx| syncing.refresh(cx));
                                    }
                                }
                                if this.page == 7 {
                                    if let Some(skins) = &this.skins {
                                        skins.update(cx, |skins, cx| skins.refresh(cx));
                                    }
                                }
                                this.sync_mutation_guards(cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .when(chrome::custom(), |column| {
                        column.child(chrome::title_bar(window, cx, &palette))
                    })
                    .child(content)
                    .when_some(
                        self.load_error
                            .as_ref()
                            .or(self.sync_error.as_ref())
                            .or(Some(&self.status))
                            .filter(|status| !status.is_empty())
                            .cloned(),
                        |column, status| {
                            column.child(
                                div()
                                    .px(px(32.))
                                    .py(px(10.))
                                    .border_t_1()
                                    .border_color(palette.border_subtle)
                                    .text_size(px(ui::TEXT_CAPTION))
                                    .text_color(palette.muted_foreground)
                                    .child(status),
                            )
                        },
                    ),
            )
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let showcase_root = showcase_root();
    if std::env::args().any(|a| a == "--showcase") && showcase_root.is_none() {
        return Err("--showcase needs a directory containing AURORA_DISPOSABLE_DIAGNOSTIC".into());
    }
    SHOWCASE.store(showcase_root.is_some(), std::sync::atomic::Ordering::Relaxed);
    let diagnostic = std::env::args().any(|a| a == "--diagnostic") || showcase_root.is_some();
    if !diagnostic && aurora_launcher_lib::run_artwork_decoder_if_requested() {
        return Ok(());
    }
    if diagnostic && showcase_root.is_none() {
        let root = std::env::temp_dir().join(format!(
            "aurora-gpui-diagnostic-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::create_dir(&root)?;
        std::fs::write(
            root.join("AURORA_DISPOSABLE_DIAGNOSTIC"),
            b"Disposable diagnostic data; no production state.\n",
        )?;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .enable_all()
        .build()?;
    let rt = runtime.handle().clone();
    let (tx, rx) = async_channel::unbounded();
    let failed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let failed_window = failed.clone();
    let host = if diagnostic && showcase_root.is_none() {
        None
    } else {
        let events = tx.clone();
        Some(Host::native(
            showcase_root.map(Ok).unwrap_or_else(|| platform_local_data_dir(APPLICATION_IDENTIFIER)),
            env!("CARGO_PKG_VERSION"),
            rt.clone(),
            move |name, payload| {
                let _ = events.try_send(Message::Event(name.to_owned(), payload));
            },
        ))
    };
    gpui_kit::platform::application().run(move |cx: &mut App| {
        gpui_kit::base::init(cx);
        theme::init(cx);
        let display = if diagnostic {
            let primary = cx.primary_display().map(|d| d.id());
            let d = primary.and_then(|primary| {
                cx.displays()
                    .into_iter()
                    .find(|d| d.id() != primary && d.bounds().size.height > d.bounds().size.width)
            });
            if d.is_none() {
                eprintln!("Diagnostic mode requires a portrait secondary display.");
                failed_window.store(true, std::sync::atomic::Ordering::Relaxed);
                cx.quit();
                return;
            }
            d
        } else {
            None
        };
        let window_size = display
            .as_ref()
            .filter(|_| !showcase())
            .map(|d| size(d.bounds().size.width * 0.8, d.bounds().size.height * 0.8))
            .unwrap_or_else(chrome::default_size);
        let display_id = display.as_ref().map(|d| d.id());
        let bounds = Bounds::centered(display_id, window_size, cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id,
                focus: !diagnostic,
                titlebar: Some(chrome::titlebar()),
                window_min_size: Some(chrome::min_size()),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let borealis = cx.new(|cx| Borealis::new(rt.clone(), window, cx));
                    let editor =
                        cx.new(|cx| InstanceEditor::new(host.clone(), rt.clone(), window, cx));
                    let creator =
                        cx.new(|cx| InstanceCreator::new(host.clone(), rt.clone(), window, cx));
                    let pack_import =
                        cx.new(|cx| PackImport::new(host.clone(), rt.clone(), window, cx));
                    let client_updates =
                        cx.new(|cx| ClientUpdates::new(host.clone(), rt.clone(), window, cx));
                    let home_widgets =
                        cx.new(|cx| HomeWidgets::new(host.clone(), rt.clone(), window, cx));
                    let mut subscriptions = vec![
                        cx.subscribe(&home_widgets, |this: &mut Launcher, _, event: &HomeWidgetsEvent, cx| {
                            match event {
                                // Home owns acknowledged layout/favorite state;
                                // registry reloads would invalidate unrelated drafts.
                                HomeWidgetsEvent::Changed => {},
                                HomeWidgetsEvent::BusyChanged(busy) => this.home_busy = *busy,
                                HomeWidgetsEvent::QuickPlay { mode, target_id, instance_id } => {
                                    this.quick_play(mode.clone(), target_id.clone(), instance_id.clone(), cx);
                                }
                            }
                            this.sync_mutation_guards(cx);
                            cx.notify();
                        }),
                        cx.subscribe(&client_updates, |this: &mut Launcher, _, event: &ClientUpdatesEvent, cx| {
                            match event {
                                ClientUpdatesEvent::BusyChanged(busy) => this.client_update_busy = *busy,
                                ClientUpdatesEvent::Changed => {
                                    this.reload_guarded(cx);
                                    if let Some(editor) = &this.instance_editor {
                                        editor.update(cx, |editor, cx| editor.refresh(cx));
                                    }
                                    this.refresh_content_views(cx);
                                }
                            }
                            this.sync_mutation_guards(cx);
                            cx.notify();
                        }),
                        cx.subscribe(
                            &editor,
                            |this: &mut Launcher, _, _: &InstanceEditorEvent, cx| {
                                this.reload_guarded(cx);
                                this.refresh_content_views(cx);
                                if let Some(syncing) = &this.syncing {
                                    syncing.update(cx, |syncing, cx| syncing.refresh(cx));
                                }
                                cx.notify();
                            },
                        ),
                        cx.subscribe(
                            &creator,
                            |this: &mut Launcher, _, _: &InstanceCreatorEvent, cx| {
                                this.reload_guarded(cx);
                                if let Some(editor) = &this.instance_editor {
                                    editor.update(cx, |editor, cx| editor.refresh(cx));
                                }
                                if let Some(syncing) = &this.syncing {
                                    syncing.update(cx, |syncing, cx| syncing.refresh(cx));
                                }
                                cx.notify();
                            },
                        ),
                    ];
                    subscriptions.push(cx.subscribe_in(
                        &pack_import,
                        window,
                        |this: &mut Launcher, _, event: &PackImportEvent, window, cx| {
                            match event {
                                PackImportEvent::BusyChanged(busy) => this.pack_import_busy = *busy,
                                PackImportEvent::OpenLocalPack { project_id, version_id, note } => {
                                    this.open_page(5, window, cx);
                                    let opened = this.discover.as_ref().is_some_and(|discover| {
                                        discover.update(cx, |discover, cx| discover.open_local_pack(
                                            project_id.clone(), version_id.clone(), note.clone(), window, cx,
                                        ))
                                    });
                                    if !opened {
                                        this.status = "The recognized modpack could not be opened. Try importing it again.".into();
                                    }
                                }
                            }
                            this.sync_mutation_guards(cx);
                            cx.notify();
                        },
                    ));
                    let mut view = Launcher {
                        borealis: Some(borealis),
                        home_widgets: Some(home_widgets),
                        client_updates: Some(client_updates),
                        instance_editor: Some(editor),
                        instance_creator: Some(creator),
                        settings: None,
                        content: None,
                        discover: None,
                        downloads: None,
                        syncing: None,
                        accounts_page: None,
                        skins: None,
                        skin_preview: None,
                        pack_import: Some(pack_import),
                        _subscriptions: subscriptions,
                        host,
                        rt,
                        tx,
                        page: 0,
                        serial: 0,
                        busy: false,
                        account_busy: false,
                        skin_busy: false,
                        client_update_busy: false,
                        downloads_busy: false,
                        home_busy: false,
                        pack_import_busy: false,
                        load_in_flight: false,
                        load_pending: false,
                        startup_pending: false,
                        loaded: false,
                        fatal_error: None,
                        update_check_started: false,
                        update_status: None,
                        readiness_in_flight: false,
                        readiness_pending: false,
                        readiness_rechecks: 0,
                        launcher: None,
                        accounts: Default::default(),
                        appearance: None,
                        readiness: None,
                        processes: HashMap::new(),
                        process_serial: 0,
                        recovery: None,
                        load_error: None,
                        sync_error: None,
                        sync_running: false,
                        status: if diagnostic && !showcase() {
                            "Disposable diagnostic mode · production data and integrations disabled"
                                .into()
                        } else {
                            String::new()
                        },
                    };
                    view.start_load();
                    if let Some(page) = showcase()
                        .then(|| std::env::var("AURORA_SHOWCASE_PAGE").ok()?.parse().ok())
                        .flatten()
                    {
                        view.open_page(page, window, cx);
                    }
                    view.sync_mutation_guards(cx);
                    view.initialize_sync();
                    cx.spawn(async move |weak, cx| {
                        while let Ok(message) = rx.recv().await {
                            if weak
                                .update(cx, |view: &mut Launcher, cx| {
                                    if let Message::Event(name, payload) = &message {
                                        if let Some(home) = &view.home_widgets {
                                            home.update(cx, |home, cx| home.on_host_event(name, payload, cx));
                                        }
                                        if let Some(updates) = &view.client_updates {
                                            updates.update(cx, |updates, cx| updates.on_host_event(name, payload, cx));
                                        }
                                        if let Some(skins) = &view.skins {
                                            skins.update(cx, |skins, cx| skins.on_host_event(name, payload, cx));
                                        }
                                        if let Some(accounts) = &view.accounts_page {
                                            accounts.update(cx, |accounts, cx| {
                                                accounts.on_host_event(name, payload, cx)
                                            });
                                        }
                                        let sync_finished = name == "instance-sync-state"
                                            && view.sync_running
                                            && serde_json::from_value::<api::SyncStatus>(
                                                payload.clone(),
                                            )
                                            .is_ok_and(|status| !status.running);
                                        if let Some(editor) = &view.instance_editor {
                                            editor.update(cx, |editor, cx| {
                                                editor.on_host_event(name, payload, cx)
                                            });
                                        }
                                        if let Some(settings) = &view.settings {
                                            settings.update(cx, |settings, cx| {
                                                settings.on_host_event(name, payload, cx)
                                            });
                                        }
                                        if let Some(content) = &view.content {
                                            content.update(cx, |content, cx| {
                                                content.on_host_event(name, payload, cx)
                                            });
                                        }
                                        if let Some(discover) = &view.discover {
                                            discover.update(cx, |discover, cx| {
                                                discover.on_host_event(name, payload, cx)
                                            });
                                        }
                                        if let Some(syncing) = &view.syncing {
                                            syncing.update(cx, |syncing, cx| {
                                                syncing.on_host_event(name, payload, cx)
                                            });
                                        }
                                        if sync_finished {
                                            if let Some(editor) = &view.instance_editor {
                                                editor.update(cx, |editor, cx| editor.refresh(cx));
                                            }
                                            view.refresh_content_views(cx);
                                        }
                                    }
                                    if matches!(&message, Message::SyncInitialized(_)) {
                                        if let Some(syncing) = &view.syncing {
                                            syncing.update(cx, |syncing, cx| syncing.refresh(cx));
                                        }
                                    }
                                    let accounts_applied = matches!(&message, Message::Loaded(serial, _)
                                        if *serial == view.serial && !view.load_pending);
                                    let startup_updates = matches!(&message, Message::StartupUpdates(_));
                                    let loaded = matches!(&message, Message::Loaded(..));
                                    view.receive(message);
                                    if loaded {
                                        // A load that cleared the appearance turns the
                                        // background off and stops its motion.
                                        view.sync_borealis(cx);
                                    }
                                    if startup_updates {
                                        // The core also emits `update-status`; this covers a repeat check
                                        // that returns the stored overview without an event.
                                        if let (Some(settings), Some(payload)) = (&view.settings, view.update_status.clone()) {
                                            settings.update(cx, |settings, cx| {
                                                settings.on_host_event("update-status", &payload, cx)
                                            });
                                        }
                                    }
                                    if accounts_applied {
                                        if let Some(home) = &view.home_widgets {
                                            home.update(cx, |home, cx| home.set_state(view.launcher.clone(), view.serial, cx));
                                        }
                                    }
                                    view.sync_mutation_guards(cx);
                                    if accounts_applied && !view.account_busy {
                                        view.supply_skin_accounts(cx);
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
                    view
                })
            },
        )
        .expect("open Aurora Launcher window");
    });
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    if failed.load(std::sync::atomic::Ordering::Relaxed) {
        return Err("Diagnostic mode requires a portrait secondary display".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_icons_come_from_the_store_and_fall_back_when_unset() {
        let mut icons = BTreeMap::new();
        icons.insert("a".to_owned(), "swords".to_owned());
        assert_eq!(stored_icon(&icons, "a"), Some("swords"));
        // An instance without a stored icon, or an unknown one, gets the
        // fallback glyph through `symbols::icon(None, ..)`.
        assert_eq!(stored_icon(&icons, "b"), None);
        assert_eq!(
            symbols::path_or_fallback(stored_icon(&icons, "b")),
            symbols::path_or_fallback(None)
        );
        assert_ne!(
            symbols::path_or_fallback(stored_icon(&icons, "a")),
            symbols::path_or_fallback(None)
        );
    }

    #[test]
    fn instances_page_drop_outline_accepts_only_one_mrpack() {
        let pack = |name: &str| PathBuf::from(name);
        let is_single_modpack =
            |paths: &[PathBuf]| pack_import::classify_modpack_drop(paths).is_ok();
        assert!(is_single_modpack(&[pack("C:\\packs\\Fabulous.mrpack")]));
        assert!(is_single_modpack(&[pack("pack.MRPACK")]));
        assert!(!is_single_modpack(&[]));
        assert!(!is_single_modpack(&[pack("pack.zip")]));
        assert!(!is_single_modpack(&[pack("pack")]));
        assert!(!is_single_modpack(&[pack("a.mrpack"), pack("b.mrpack")]));
    }

    fn view() -> Launcher {
        let (tx, _) = async_channel::unbounded();
        Launcher {
            borealis: None,
            home_widgets: None,
            client_updates: None,
            instance_editor: None,
            instance_creator: None,
            settings: None,
            content: None,
            discover: None,
            downloads: None,
            syncing: None,
            accounts_page: None,
            skins: None,
            skin_preview: None,
            pack_import: None,
            _subscriptions: Vec::new(),
            host: None,
            rt: {
                static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> =
                    std::sync::OnceLock::new();
                RUNTIME
                    .get_or_init(|| tokio::runtime::Runtime::new().unwrap())
                    .handle()
                    .clone()
            },
            tx,
            page: 0,
            serial: 2,
            busy: false,
            account_busy: false,
            skin_busy: false,
            client_update_busy: false,
            downloads_busy: false,
            home_busy: false,
            pack_import_busy: false,
            load_in_flight: false,
            load_pending: false,
            startup_pending: false,
            loaded: false,
            fatal_error: None,
            update_check_started: false,
            update_status: None,
            readiness_in_flight: true,
            readiness_pending: false,
            readiness_rechecks: 0,
            launcher: None,
            accounts: Default::default(),
            appearance: None,
            readiness: None,
            processes: HashMap::new(),
            process_serial: 0,
            recovery: None,
            load_error: None,
            sync_error: None,
            sync_running: false,
            status: String::new(),
        }
    }
    fn readiness() -> api::PlayReadiness {
        serde_json::from_value(serde_json::json!({"ready": true, "instanceId":"old", "accountId":null, "accountName":null, "instanceStatus":"ready", "runtimeStatus":"ready", "accountStatus":"ready", "processStatus":"idle", "blockers":[]})).unwrap()
    }
    fn instance_json(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id":id, "displayName":"Fixture", "state":"ready", "aurora":null,
            "platform":{"kind":"vanilla"}, "minecraftVersion":"1.21.10",
            "configuration":{"minecraftVersion":"1.21.10", "auroraEnabled":false,
                "loader":{"kind":"vanilla"}, "memoryMib":4096,
                "additionalJvmArguments":"", "window":null}
        })
    }
    fn launcher_fixture() -> api::LauncherState {
        serde_json::from_value(serde_json::json!({
            "config": {"selectedInstanceId": "old"}, "instances": [instance_json("old")]
        }))
        .unwrap()
    }
    fn select_fixture(state: &mut Launcher) {
        state.launcher = Some(launcher_fixture());
    }
    fn decision(instance: &str, process: &str, blockers: serde_json::Value) -> api::PlayReadiness {
        serde_json::from_value(serde_json::json!({
            "ready": false, "canPrepare": false, "instanceId": instance, "accountId": null,
            "accountName": null, "instanceStatus": "ready", "runtimeStatus": "ready",
            "accountStatus": "ready", "processStatus": process, "blockers": blockers
        }))
        .unwrap()
    }
    fn child(instance: &str, status: &str) -> api::LaunchProcess {
        serde_json::from_value(serde_json::json!({
            "instanceId": instance, "status": status, "processId": null,
            "exitCode": null, "message": null
        }))
        .unwrap()
    }
    fn startup_snapshot(launcher: api::ApiResult<api::LauncherState>) -> Snapshot {
        Snapshot {
            status: Some(Ok(())),
            launcher,
            accounts: Ok(Default::default()),
            appearance: Err(api::ApiError::new("fixture", "no appearance")),
        }
    }

    #[test]
    fn play_requires_matching_target_and_account_and_no_active_process() {
        let mut state = view();
        select_fixture(&mut state);
        state.accounts.selected_account_id = Some("account".into());
        let mut decision = readiness();
        decision.account_id = Some("other-account".into());
        state.readiness = Some(decision.clone());
        assert!(!state.can_play("old"));
        decision.account_id = Some("account".into());
        state.readiness = Some(decision);
        assert!(state.can_play("old"));
        state.pack_import_busy = true;
        assert!(!state.can_play("old"));
        state.pack_import_busy = false;
        state.skin_busy = true;
        assert!(!state.can_play("old"));
        state.skin_busy = false;
        assert!(state.can_play("old"));
        state.client_update_busy = true;
        assert!(!state.can_play("old"));
        state.client_update_busy = false;
        assert!(state.can_play("old"));
        state.downloads_busy = true;
        assert!(!state.can_play("old"));
        state.downloads_busy = false;
        assert!(state.can_play("old"));
        state.receive(Message::Event("launch-state".into(), serde_json::json!({
            "instanceId":"old", "status":"running", "processId":123, "exitCode":null, "message":null
        })));
        assert!(!state.can_play("old"));
    }

    #[test]
    fn client_updates_require_the_exact_selected_registry_record() {
        let mut state = view();
        assert!(state.client_update_target().is_none());
        select_fixture(&mut state);
        assert_eq!(state.client_update_target().unwrap().id, "old");
        state.launcher.as_mut().unwrap().config.selected_instance_id = Some("missing".into());
        assert!(state.client_update_target().is_none());
    }

    #[test]
    fn quick_play_checks_its_own_instance_and_exact_signed_in_account() {
        let mut state = view();
        select_fixture(&mut state);
        state.accounts = serde_json::from_value(serde_json::json!({
            "selectedAccountId":"account", "accounts":[{"accountId":"account", "minecraftName":"Fixture", "status":"signedIn"}]
        })).unwrap();
        state.launcher.as_mut().unwrap().config.selected_instance_id = None;
        assert!(state.can_quick_launch("old"));
        assert!(!state.can_quick_launch("missing"));
        assert!(!state.can_quick_launch(""));
        state.accounts.accounts[0].status = "signedOut".into();
        assert!(!state.can_quick_launch("old"));
        state.accounts.accounts[0].status = "signedIn".into();
        state.receive(Message::Event("launch-state".into(), serde_json::json!({
            "instanceId":"old", "status":"running", "processId":123, "exitCode":null, "message":null
        })));
        assert!(!state.can_quick_launch("old"));
    }

    #[test]
    fn quick_play_messages_match_the_slint_host() {
        assert_eq!(
            quick_play_refusal("server", false, true),
            Some("The instance for this server no longer exists.")
        );
        assert_eq!(
            quick_play_refusal("server", true, false),
            Some("Sign in to quick play this server.")
        );
        assert_eq!(quick_play_refusal("server", true, true), None);
        assert_eq!(
            quick_play_refusal("world", true, false),
            Some("Select a Minecraft account to use Quick Play.")
        );
        assert_eq!(quick_play_refusal("saved-world", true, true), None);
        assert_eq!(
            quick_play_progress("server", Some("Hypixel")),
            "Preparing server: Hypixel"
        );
        assert_eq!(quick_play_progress("server", None), "Checking readiness…");
        assert_eq!(
            quick_play_progress("world", Some("x")),
            "Checking readiness…"
        );
    }

    #[test]
    fn quick_play_rejects_hidden_home_and_pending_mutations() {
        let mut state = view();
        select_fixture(&mut state);
        state.accounts = serde_json::from_value(serde_json::json!({
            "selectedAccountId":"account", "accounts":[{"accountId":"account", "minecraftName":"Fixture", "status":"signedIn"}]
        })).unwrap();
        assert!(state.can_quick_launch("old"));
        state.home_busy = true;
        assert!(!state.can_quick_launch("old"));
        assert!(!state.can_play("old"));
        state.home_busy = false;
        state.downloads_busy = true;
        assert!(!state.can_quick_launch("old"));
        state.downloads_busy = false;
        state.load_in_flight = true;
        assert!(!state.can_quick_launch("old"));
        state.load_in_flight = false;
        state.sync_running = true;
        assert!(!state.can_quick_launch("old"));
        state.sync_running = false;
        state.page = 1;
        assert!(!state.can_quick_launch("old"));
    }

    #[test]
    fn delayed_play_reply_cannot_overwrite_a_terminal_event() {
        let mut state = view();
        select_fixture(&mut state);
        state.busy = true;
        state.receive(Message::Event(
            "launch-state".into(),
            serde_json::json!({
                "instanceId":"old", "status":"failed", "processId":null, "exitCode":17,
                "message":"Synthetic failure", "diagnostic":{"message":"Fixture diagnostic"}
            }),
        ));
        let running = serde_json::from_value(serde_json::json!({
            "instanceId":"old", "status":"running", "processId":123, "exitCode":null, "message":null
        }))
        .unwrap();
        state.receive(Message::Launched("old".into(), 0, Ok(running)));
        assert_eq!(state.processes["old"].status, "failed");
        assert_eq!(state.processes["old"].exit_code, Some(17));
        assert!(!state.busy);
    }

    #[test]
    fn malformed_launch_events_are_ignored() {
        let mut state = view();
        state.receive(Message::Event(
            "launch-state".into(),
            serde_json::json!({"status":"running"}),
        ));
        assert!(state.processes.is_empty());
        assert_eq!(state.process_serial, 0);
    }
    #[test]
    fn stale_readiness_cannot_enable_play() {
        let mut state = view();
        state.receive(Message::Readiness(1, 0, Ok(readiness()), None));
        assert!(state.readiness.is_none());
        assert!(!state.readiness_in_flight);
    }
    #[test]
    fn current_readiness_is_published() {
        let mut state = view();
        select_fixture(&mut state);
        state.receive(Message::Readiness(2, 0, Ok(readiness()), None));
        assert_eq!(state.readiness.unwrap().instance_id, "old");
    }
    #[test]
    fn unknown_host_events_do_not_change_status() {
        let mut state = view();
        state.status = "kept".into();
        state.receive(Message::Event("unrelated".into(), serde_json::Value::Null));
        assert_eq!(state.status, "kept");
    }

    #[test]
    fn sync_initialization_cannot_clear_an_unrelated_operation() {
        let mut state = view();
        state.busy = true;
        state.account_busy = true;
        state.receive(Message::SyncInitialized(Err(api::ApiError::new(
            "fixture",
            "cannot read policy",
        ))));
        assert!(state.busy && state.account_busy);
        assert_eq!(state.serial, 2);
        assert!(state.sync_error.is_some());
        state.receive(Message::SyncInitialized(Ok(())));
        assert!(state.busy && state.account_busy);
        assert!(state.sync_error.is_none());
    }

    #[test]
    fn diagnostic_sync_initialization_submits_no_work() {
        let mut state = view();
        let (tx, rx) = async_channel::unbounded();
        state.tx = tx;
        state.initialize_sync();
        assert!(matches!(
            rx.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        ));
        state.receive(Message::Event(
            "instance-sync-state".into(),
            serde_json::json!({"bad": true}),
        ));
        assert!(!state.sync_running);
    }

    #[test]
    fn a_panicking_worker_returns_a_terminal_error() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result: api::ApiResult<()> = runtime.block_on(guarded(async {
            panic!("synthetic worker failure");
        }));
        assert_eq!(result.unwrap_err().code, "native_operation_failed");
    }

    #[test]
    fn blocker_codes_map_to_the_slint_labels_and_destinations() {
        let cases = [
            ("launch_authentication_required", "Sign in", Some(1)),
            ("launch_runtime_not_ready", "Install Java", None),
            ("launch_custom_java_invalid", "Java settings", Some(3)),
            ("launch_instance_not_ready", "Review recovery", Some(3)),
            ("launch_instance_damaged", "Review recovery", Some(3)),
            ("launch_configuration_changed", "Open instance", Some(3)),
            ("launch_mod_incompatible", "Review mods", Some(4)),
        ];
        for (code, label, page) in cases {
            let action = BlockerAction::for_code(code).unwrap_or_else(|| panic!("{code}"));
            assert_eq!(action.label(), label, "{code}");
            assert_eq!(action.page(), page, "{code}");
        }
        // Explanation only: the process-state blocker and codes from a newer core.
        assert_eq!(BlockerAction::for_code("launch_already_running"), None);
        assert_eq!(BlockerAction::for_code("launch_something_new"), None);
    }

    #[test]
    fn blockers_show_only_for_the_selected_instances_idle_decision() {
        let mut state = view();
        select_fixture(&mut state);
        let blockers =
            serde_json::json!([{"code":"launch_runtime_not_ready","message":"Java is missing."}]);
        state.readiness = Some(decision("old", "idle", blockers.clone()));
        assert_eq!(state.visible_blockers("old").len(), 1);
        assert!(state.visible_blockers("other").is_empty());
        // A running game hides them, but a finished child's snapshot outranks a
        // stale decision that still claims the process is alive.
        state.readiness = Some(decision("old", "running", blockers));
        assert!(state.visible_blockers("old").is_empty());
        state.processes.insert("old".into(), child("old", "exited"));
        assert_eq!(state.visible_blockers("old").len(), 1);
        state.readiness = None;
        assert!(state.visible_blockers("old").is_empty());
    }

    #[test]
    fn play_label_follows_the_slint_wording_and_the_childs_own_state() {
        let mut state = view();
        select_fixture(&mut state);
        assert_eq!(state.play_label("old"), "Play");
        let mut preparable = decision("old", "idle", serde_json::json!([]));
        preparable.can_prepare = true;
        state.readiness = Some(preparable);
        assert_eq!(state.play_label("old"), "Prepare & play");
        state.readiness = Some(decision("old", "starting", serde_json::json!([])));
        assert_eq!(state.play_label("old"), "Starting…");
        state.readiness = Some(decision("old", "running", serde_json::json!([])));
        assert_eq!(state.play_label("old"), "Running");
        state.processes.insert("old".into(), child("old", "failed"));
        assert_eq!(state.play_label("old"), "Play");
        state
            .processes
            .insert("old".into(), child("old", "starting"));
        assert_eq!(state.play_label("old"), "Running");
        state.readiness = None;
        assert_eq!(state.play_label("old"), "Starting…");
    }

    #[test]
    fn home_selection_needs_a_different_registered_instance_and_no_competing_work() {
        let mut state = view();
        state.launcher = Some(
            serde_json::from_value(serde_json::json!({
                "config": {"selectedInstanceId": "a"},
                "instances": [instance_json("a"), instance_json("b")]
            }))
            .unwrap(),
        );
        assert!(state.can_select("b"));
        assert!(!state.can_select("a"), "the current selection is a no-op");
        assert!(!state.can_select("missing"));
        for set in [
            |s: &mut Launcher, v| s.busy = v,
            |s: &mut Launcher, v| s.account_busy = v,
            |s: &mut Launcher, v| s.skin_busy = v,
            |s: &mut Launcher, v| s.client_update_busy = v,
            |s: &mut Launcher, v| s.downloads_busy = v,
            |s: &mut Launcher, v| s.home_busy = v,
            |s: &mut Launcher, v| s.pack_import_busy = v,
        ] {
            set(&mut state, true);
            assert!(!state.can_select("b"));
            set(&mut state, false);
            assert!(state.can_select("b"));
        }
    }

    #[test]
    fn stale_running_needs_a_live_decision_and_a_finished_child() {
        let running = decision("old", "running", serde_json::json!([]));
        assert!(stale_running(&running, Some(&child("old", "exited"))));
        assert!(stale_running(&running, Some(&child("old", "failed"))));
        assert!(!stale_running(&running, Some(&child("old", "running"))));
        assert!(!stale_running(&running, None));
        let idle = decision("old", "idle", serde_json::json!([]));
        assert!(!stale_running(&idle, Some(&child("old", "exited"))));
    }

    #[test]
    fn stale_running_rechecks_stop_at_the_limit_and_reset_on_a_settled_decision() {
        let mut state = view();
        select_fixture(&mut state);
        state.processes.insert("old".into(), child("old", "exited"));
        let stale = decision("old", "running", serde_json::json!([]));
        for attempt in 1..=7u8 {
            state.readiness_in_flight = true;
            state.receive(Message::Readiness(2, 0, Ok(stale.clone()), None));
            assert_eq!(
                state.readiness_rechecks,
                attempt.min(READINESS_RECHECK_LIMIT)
            );
        }
        // A superseded reply neither counts nor resets.
        state.receive(Message::Readiness(1, 0, Ok(stale), None));
        assert_eq!(state.readiness_rechecks, READINESS_RECHECK_LIMIT);
        state.receive(Message::Readiness(
            2,
            0,
            Ok(decision("old", "idle", serde_json::json!([]))),
            None,
        ));
        assert_eq!(state.readiness_rechecks, 0);
        assert!(state.take_recheck());
    }

    #[test]
    fn the_recheck_timer_reply_is_dropped_once_readiness_was_invalidated() {
        let mut state = view();
        state.receive(Message::RecheckReadiness(1));
        assert!(!state.readiness_pending, "an older serial starts nothing");
        // The current serial re-reads; with a check already running it coalesces.
        state.readiness_in_flight = true;
        state.receive(Message::RecheckReadiness(2));
        assert!(state.readiness_pending);
    }

    #[test]
    fn a_stale_startup_attempt_is_dropped_and_its_duty_survives() {
        let mut state = view();
        state.load_in_flight = true;
        state.receive(Message::Loaded(
            1,
            startup_snapshot(Err(api::ApiError::new("fixture", "old failure"))),
        ));
        assert!(!state.load_in_flight);
        assert!(
            state.fatal_error.is_none(),
            "an older attempt cannot raise the error"
        );
        assert!(state.launcher.is_none() && !state.loaded);
        assert!(matches!(
            state.plan_load(false),
            LoadPlan::Run { startup: true, .. }
        ));
        assert!(!state.startup_pending);
    }

    #[test]
    fn a_retry_supersedes_the_in_flight_attempt_and_keeps_the_startup_sequence() {
        let mut state = view();
        state.load_in_flight = true;
        assert!(matches!(state.plan_load(true), LoadPlan::Coalesced));
        assert_eq!(state.serial, 3);
        assert!(state.load_pending && state.startup_pending);
        // The superseded attempt's reply starts the follow-up instead of applying.
        state.receive(Message::Loaded(2, startup_snapshot(Ok(launcher_fixture()))));
        assert!(state.launcher.is_none() && !state.loaded);
        assert!(state.startup_pending);
    }

    #[test]
    fn a_failed_startup_shows_the_error_until_a_retry_succeeds() {
        let mut state = view();
        state.receive(Message::Loaded(
            2,
            startup_snapshot(Err(api::ApiError::new("fixture", "state unreadable"))),
        ));
        assert_eq!(state.fatal_error.as_deref(), Some("state unreadable"));
        assert!(state.launcher.is_none() && !state.loaded);
        state.retry_load();
        assert!(state.fatal_error.is_none() && state.load_error.is_none());
        state.receive(Message::Loaded(2, startup_snapshot(Ok(launcher_fixture()))));
        assert!(state.fatal_error.is_none() && state.loaded);
        assert_eq!(state.selected_id(), Some("old"));
    }

    #[test]
    fn an_unreadable_application_status_is_fatal_before_anything_else_is_read() {
        let mut state = view();
        let error = api::ApiError::new("fixture", "no data root");
        state.receive(Message::Loaded(
            2,
            Snapshot {
                status: Some(Err(error.clone())),
                launcher: Err(error.clone()),
                accounts: Err(error.clone()),
                appearance: Err(error),
            },
        ));
        assert_eq!(state.fatal_error.as_deref(), Some("no data root"));
        assert!(state.launcher.is_none() && state.appearance.is_none() && !state.loaded);
        assert!(state.load_error.is_none());
    }

    #[test]
    fn a_failed_ordinary_reload_is_not_the_fatal_startup_screen() {
        let mut state = view();
        state.receive(Message::Loaded(
            2,
            Snapshot {
                status: None,
                launcher: Err(api::ApiError::new("fixture", "busy")),
                accounts: Ok(Default::default()),
                appearance: Err(api::ApiError::new("fixture", "busy")),
            },
        ));
        assert!(state.fatal_error.is_none());
        assert_eq!(state.load_error.as_deref(), Some("busy"));
    }

    #[test]
    fn diagnostic_startup_never_checks_for_updates() {
        let mut state = view();
        let (tx, rx) = async_channel::unbounded();
        state.tx = tx;
        state.receive(Message::Loaded(2, startup_snapshot(Ok(launcher_fixture()))));
        state.start_update_check();
        assert!(state.loaded);
        assert!(!state.update_check_started);
        assert!(matches!(
            rx.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        ));
    }

    #[test]
    fn startup_update_status_is_kept_for_settings_and_a_newer_event_wins() {
        let original = serde_json::json!({
            "launcher": {"installedVersion": "2.1.1", "availability": {"kind": "upToDate"}},
            "client": {"kind": "updateAvailable"},
            "clientInstanceId": "old", "clientInstanceName": "Fixture",
            "clientInstalledVersion": "1.0.0"
        });
        let overview: api::UpdateOverview = serde_json::from_value(original.clone()).unwrap();
        let payload = update_overview_payload(&overview);
        assert_eq!(payload, original);
        assert!(settings::update_status_event("update-status", &payload).is_some());

        let mut state = view();
        state.receive(Message::StartupUpdates(Err(api::ApiError::new(
            "fixture", "offline",
        ))));
        assert!(state.update_status.is_none());
        state.receive(Message::StartupUpdates(Ok(overview.clone())));
        assert_eq!(state.update_status.as_ref(), Some(&payload));
        let newer = serde_json::json!({
            "launcher": {"installedVersion": "2.1.1", "availability": {"kind": "updateAvailable", "candidate": "2.2.0"}},
            "client": {"kind": "upToDate"}
        });
        state.receive(Message::Event("update-status".into(), newer.clone()));
        assert_eq!(state.update_status.as_ref(), Some(&newer));
        // A late or malformed startup result never replaces what Settings was told.
        state.receive(Message::StartupUpdates(Ok(overview)));
        state.receive(Message::Event(
            "update-status".into(),
            serde_json::json!({"bad": true}),
        ));
        assert_eq!(state.update_status.as_ref(), Some(&newer));
    }

    fn appearance_fixture(background: &str, motion: u8) -> api::Appearance {
        serde_json::from_value(serde_json::json!({
            "auroraMotionSpeed": motion,
            "background": background,
            "theme": "aurora-dark",
            "accent": { "type": "curated", "id": "violet" },
            "palette": {
                "accent": "#8b80ff", "accentStrong": "#6f5df2", "accentHover": "#6f63e8",
                "accentPressed": "#5b4ad6", "accentContrast": "#ffffff",
                "accentSoft": "rgba(139, 128, 255, 0.14)",
                "accentOutline": "rgba(139, 128, 255, 0.55)"
            },
            "themes": [],
            "accents": []
        }))
        .unwrap()
    }

    #[test]
    fn startup_borealis_state_comes_from_the_appearance_without_settings() {
        // Settings was never built: the saved background and speed alone decide.
        let saved = appearance_fixture("borealis", 35);
        assert_eq!(borealis_inputs(Some(&saved), None), (true, 35));
        let simple = appearance_fixture("simple", 80);
        assert_eq!(borealis_inputs(Some(&simple), None), (false, 80));
        // Reduce motion is saved as speed 0 and keeps the layer static.
        let still = appearance_fixture("borealis", 0);
        assert_eq!(borealis_inputs(Some(&still), None), (true, 0));
        // An unreleased slider draft or queued write wins over the saved speed,
        // but never changes which background is saved.
        assert_eq!(borealis_inputs(Some(&saved), Some(90)), (true, 90));
        assert_eq!(borealis_inputs(Some(&simple), Some(90)), (false, 90));
    }

    #[test]
    fn a_failed_appearance_load_leaves_the_background_plain_and_static() {
        let mut state = view();
        state.appearance = Some(appearance_fixture("borealis", 50));
        state.receive(Message::Loaded(
            2,
            Snapshot {
                status: None,
                launcher: Ok(launcher_fixture()),
                accounts: Ok(Default::default()),
                appearance: Err(api::ApiError::new("fixture", "unreadable")),
            },
        ));
        assert!(state.appearance.is_none());
        // Even a stale Settings speed cannot restart motion without a snapshot;
        // diagnostic mode (no Host, so no appearance) is the same state.
        assert_eq!(
            borealis_inputs(state.appearance.as_ref(), Some(70)),
            (false, 0)
        );
        assert_eq!(borealis_inputs(None, None), (false, 0));
    }

    #[test]
    fn every_accepted_navigation_cancels_settings_previews() {
        // Leaving Settings, entering it, and re-clicking the current page.
        for (from, to) in [(2, 0), (0, 2), (2, 2), (0, 0), (7, 3)] {
            assert!(navigation_accepted(from, to, false), "{from} -> {to}");
        }
        // A running client update refuses a change of page, but not a re-click.
        assert!(!navigation_accepted(2, 0, true));
        assert!(!navigation_accepted(0, 2, true));
        assert!(navigation_accepted(2, 2, true));
    }
}
