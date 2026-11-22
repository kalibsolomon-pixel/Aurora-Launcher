#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate gpui_kit as gpui;

mod accounts;
mod artwork;
mod backend;
mod borealis;
mod chrome;
mod client_updates;
mod command_palette;
mod content;
mod content_updates;
mod controls;
mod discover;
mod downloads;
mod file_picker;
mod game_log;
mod home_widgets;
mod images;
mod instance_creator;
mod instance_editor;
mod media;
mod navigation;
mod pack_import;
mod pulsar;
mod screenshots;
mod server_editor;
mod settings;
mod skin_preview;
mod skins;
mod skins_showcase;
mod storage;
mod symbols;
mod syncing;
mod theme;
mod tray;
mod ui;
mod worlds;

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
    App, Bounds, Context, ExternalPaths, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    size,
};
use home_widgets::{HomeWidgets, HomeWidgetsEvent};
use instance_creator::{InstanceCreator, InstanceCreatorEvent};
use instance_editor::{InstanceEditor, InstanceEditorEvent};
use pack_import::{PackImport, PackImportEvent};
use pulsar::Pulsar;
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
    MaybeOpenLog(String, bool),
    StopResult(api::ApiResult<()>),
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
    RailAvatar(Option<Vec<u8>>),
    HeroActivity(String, Vec<(gpui::SharedString, gpui::SharedString)>),
    Organization(api::ApiResult<api::InstanceOrganization>),
    OnLaunchBehavior(api::OnLaunchBehavior),
    PulsarCount(Option<usize>, bool),
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
#[cfg(debug_assertions)]
fn showcase_root() -> Option<PathBuf> {
    let mut args = std::env::args().skip_while(|a| a != "--showcase").skip(1);
    let root = PathBuf::from(args.next()?);
    root.join("AURORA_DISPOSABLE_DIAGNOSTIC")
        .is_file()
        .then_some(root)
}
#[cfg(not(debug_assertions))]
fn showcase_root() -> Option<PathBuf> {
    None
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
        "clients": overview.clients,
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

/// Single-theme Aurora icon palette: Aurora/active tiles glow in the active theme
/// accent; standard and non-Aurora tiles stay sleek dark neutral surfaces.
pub(crate) fn icon_palette(
    _name: Option<&str>,
    active: bool,
    palette: &theme::AuroraTheme,
) -> (gpui::Hsla, gpui::Hsla, gpui::Hsla) {
    if active {
        let h1 = palette.ring.h;
        let start = gpui::Hsla {
            h: h1,
            s: palette.ring.s.clamp(0.45, 0.90),
            l: 0.56,
            a: 0.28,
        };
        let end = gpui::Hsla {
            h: h1,
            s: palette.ring.s.clamp(0.40, 0.86),
            l: 0.46,
            a: 0.14,
        };
        let ink = gpui::Hsla {
            h: h1,
            s: palette.ring.s.clamp(0.35, 0.92),
            l: 0.84,
            a: 1.0,
        };
        (start, end, ink)
    } else {
        (
            palette.raised,
            palette.card,
            gpui::Hsla {
                a: 0.84,
                ..palette.foreground
            },
        )
    }
}

/// Rounded icon well with flat solid fill and 4pt corner radius.
pub(crate) fn instance_icon(
    name: Option<&str>,
    size: f32,
    active: bool,
    palette: &theme::AuroraTheme,
) -> gpui::Div {
    let (_start, _end, ink) = icon_palette(name, active, palette);
    let bg = if active {
        palette.accent_soft
    } else {
        palette.surface
    };
    let border = if active {
        palette.border
    } else {
        palette.border_subtle
    };
    let radius = if size >= 48. { px(12.) } else { px(8.) };
    div()
        .relative()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(px(size))
        .h(px(size))
        .rounded(radius)
        .bg(bg)
        .border_1()
        .border_color(border)
        .child(symbols::icon(name, px((size * 0.5).round()), ink))
}

/// Instance icon with a flat solid Aurora sparkle corner badge for Aurora Client instances.
pub(crate) fn aurora_instance_icon(
    name: Option<&str>,
    size: f32,
    active: bool,
    has_aurora: bool,
    palette: &theme::AuroraTheme,
) -> gpui::Div {
    let tile = instance_icon(name, size, active, palette);
    if !has_aurora {
        return tile;
    }
    let badge_size = if size >= 56. { 24. } else { 20. };
    tile.child(
        div()
            .absolute()
            .bottom(px(-4.))
            .right(px(-4.))
            .size(px(badge_size))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(palette.accent)
            .border_2()
            .border_color(palette.card)
            .child(symbols::icon(
                Some("auto_awesome"),
                px(12.),
                palette.accent_foreground,
            )),
    )
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstanceTab {
    Overview,
    Content,
    Worlds,
    Screenshots,
    Settings,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NavLocation {
    page: u8,
    instance_detail: Option<(String, InstanceTab)>,
    content_kind: i32,
}

impl NavLocation {
    fn home() -> Self {
        Self {
            page: 0,
            instance_detail: None,
            content_kind: 0,
        }
    }
}

struct Launcher {
    root_focus: Option<gpui::FocusHandle>,
    nav_history: Vec<NavLocation>,
    nav_index: usize,
    nav_restoring: bool,
    /// The window background behind the chrome and pages; absent only in
    /// fixtures that have no window.
    borealis: Option<gpui::Entity<Borealis>>,
    home_widgets: Option<gpui::Entity<HomeWidgets>>,
    client_updates: Option<gpui::Entity<ClientUpdates>>,
    instance_editor: Option<gpui::Entity<InstanceEditor>>,
    instance_creator: Option<gpui::Entity<InstanceCreator>>,
    settings: Option<gpui::Entity<Settings>>,
    content: Option<gpui::Entity<Content>>,
    worlds: Option<gpui::Entity<worlds::Worlds>>,
    command_palette: Option<gpui::Entity<command_palette::CommandPalette>>,
    pulsar: Option<gpui::Entity<Pulsar>>,
    discover: Option<gpui::Entity<Discover>>,
    downloads: Option<gpui::Entity<Downloads>>,
    syncing: Option<gpui::Entity<Syncing>>,
    accounts_page: Option<gpui::Entity<Accounts>>,
    skins: Option<gpui::Entity<Skins>>,
    skin_preview: Option<gpui::Entity<SkinPreview>>,
    pack_import: Option<gpui::Entity<PackImport>>,
    instances_search: Option<gpui::Entity<gpui_kit::base::input::InputState>>,
    instances_sort: u8,
    instances_tag_filter: Option<String>,
    organization: api::InstanceOrganization,
    on_launch_behavior: api::OnLaunchBehavior,
    cli_launch_instance: Option<String>,
    last_pulsar_count: Option<usize>,
    main_hwnd: Option<isize>,
    tray_active: bool,
    _subscriptions: Vec<gpui::Subscription>,
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    tx: async_channel::Sender<Message>,
    page: u8,
    sidebar_collapsed: bool,
    instances_filter: u8,
    instance_detail: Option<(String, InstanceTab)>,
    reset_instance_scroll: bool,
    showcase_instance_opened: bool,
    reset_discover_scroll: bool,
    serial: u64,
    busy: bool,
    account_busy: bool,
    skin_busy: bool,
    client_update_busy: bool,
    client_update_instance: Option<String>,
    launcher_update_pending: bool,
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
    rail_avatar: Option<Arc<gpui::RenderImage>>,
    rail_avatar_account: Option<String>,
    hero_activity: HashMap<String, Vec<(gpui::SharedString, gpui::SharedString)>>,
    appearance: Option<api::Appearance>,
    readiness: Option<api::PlayReadiness>,
    processes: HashMap<String, api::LaunchProcess>,
    process_serial: u64,
    game_showcase_applied: bool,
    launching: Option<String>,
    pending_stop: Option<String>,
    kill_confirmation: Option<(String, u32)>,
    log_windows: HashMap<String, gpui::WindowHandle<game_log::GameLog>>,
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
        let state = self.launcher.as_ref()?;
        self.client_update_instance
            .as_ref()
            .and_then(|id| state.instances.iter().find(|instance| &instance.id == id))
            .or_else(|| {
                state
                    .instances
                    .iter()
                    .find(|instance| self.client_offer(instance).is_some())
            })
            .or_else(|| {
                let id = self.selected_id()?;
                state.instances.iter().find(|instance| instance.id == id)
            })
            .cloned()
    }

    fn client_offer<'a>(&'a self, instance: &api::InstanceSummary) -> Option<&'a str> {
        if instance.state != "ready" {
            return None;
        }
        let availability = self
            .update_status
            .as_ref()?
            .get("clients")?
            .get(&instance.id)?;
        if availability.get("kind")?.as_str()? != "updateAvailable"
            || availability.get("current")?.as_str()? != instance.aurora.as_ref()?.version
        {
            return None;
        }
        availability
            .get("candidate")?
            .as_str()
            .filter(|version| !version.is_empty())
    }

    fn updates_pending(&self) -> bool {
        self.launcher_update_pending
            || self.launcher.as_ref().is_some_and(|state| {
                state
                    .instances
                    .iter()
                    .any(|instance| self.client_offer(instance).is_some())
            })
    }

    fn set_update_status(&mut self, payload: serde_json::Value) {
        let availability = &payload["launcher"]["availability"];
        match availability["kind"].as_str() {
            Some("updateAvailable") => {
                self.launcher_update_pending = settings::launcher_offer(availability).is_some()
            }
            Some("upToDate" | "notApplicable") => self.launcher_update_pending = false,
            _ => {}
        }
        self.update_status = Some(payload);
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
                    .instance_editor
                    .as_ref()
                    .is_some_and(|editor| editor.read(cx).is_busy())
                || self
                    .content
                    .as_ref()
                    .is_some_and(|content| content.read(cx).is_busy(cx))
                || self
                    .pack_import
                    .as_ref()
                    .is_some_and(|view| view.read(cx).is_busy())
                || self
                    .discover
                    .as_ref()
                    .is_some_and(|view| view.read(cx).is_busy());
            let choices = self
                .launcher
                .as_ref()
                .into_iter()
                .flat_map(|state| &state.instances)
                .filter(|instance| instance.aurora.is_some() && instance.state == "ready")
                .map(|instance| {
                    (
                        instance.id.clone(),
                        instance.display_name.clone(),
                        self.client_offer(instance).map(str::to_owned),
                    )
                })
                .collect();
            updates.update(cx, |updates, cx| {
                updates.set_choices(choices);
                updates.set_target(target, self.serial, cx);
                updates.set_blocked(blocked, cx);
            });
        }
        if let Some(settings) = &self.settings {
            let blocked = self.busy
                || account_busy
                || skin_busy
                || client_update_busy
                || home_busy
                || self.load_in_flight
                || self.readiness_in_flight
                || self.sync_running
                || self.downloads_running(cx)
                || self.pack_import_busy
                || self
                    .processes
                    .values()
                    .any(|p| matches!(p.status.as_str(), "starting" | "running"))
                || self
                    .pack_import
                    .as_ref()
                    .is_some_and(|v| v.read(cx).is_busy())
                || self.discover.as_ref().is_some_and(|v| v.read(cx).is_busy())
                || self
                    .content
                    .as_ref()
                    .is_some_and(|v| v.read(cx).is_busy(cx))
                || self
                    .instance_editor
                    .as_ref()
                    .is_some_and(|v| v.read(cx).is_busy());
            let pending = self.updates_pending();
            settings.update(cx, |settings, cx| {
                settings.set_install_blocked(blocked, cx);
                settings.set_updates_pending(pending, cx);
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
            let needs_sign_in = self.selected_id().is_some_and(|id| {
                self.visible_blockers(id)
                    .iter()
                    .any(|blocker| blocker.code == "launch_authentication_required")
            });
            home.update(cx, |home, cx| {
                if self.page != 0 {
                    home.set_active(false, cx);
                }
                home.set_blocked(blocked, cx);
                home.set_needs_sign_in(needs_sign_in, cx);
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
    /// load starts now or coalesces into a single follow-up. Preserves the
    /// currently displayed readiness decision for the same selected instance
    /// so reloads do not flash the Play button or jump the Home layout.
    fn plan_load(&mut self, startup: bool) -> LoadPlan {
        self.serial += 1;
        if self
            .readiness
            .as_ref()
            .is_some_and(|r| Some(r.instance_id.as_str()) != self.selected_id())
        {
            self.readiness = None;
        }
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
                let launcher_fut = guarded(api::launcher_state(host.clone()));
                let accounts_fut = {
                    let host = host.clone();
                    async move {
                        if showcase() {
                            Ok(skins_showcase::accounts())
                        } else {
                            guarded(api::accounts(host)).await
                        }
                    }
                };
                let appearance_fut = guarded(api::appearance(host));
                let (launcher, accounts, appearance) =
                    tokio::join!(launcher_fut, accounts_fut, appearance_fut);
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
        if self.readiness.as_ref().is_some_and(|r| {
            Some(r.instance_id.as_str()) != self.selected_id()
                || r.account_id != self.accounts.selected_account_id
        }) {
            self.readiness = None;
        }
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
            // This explicit capture state previews Play without credentials.
            let signed_in = std::env::var("AURORA_SHOWCASE_GAME_STATE")
                .is_ok_and(|state| state == "play")
                || self.accounts.accounts.iter().any(|saved| {
                    Some(&saved.account_id) == account.as_ref() && saved.status == "signedIn"
                });
            let decision = api::PlayReadiness {
                ready: signed_in,
                can_prepare: false,
                can_launch_offline: false,
                instance_id: id,
                account_name: self.accounts.selected_account_id.as_ref().and_then(|_| {
                    self.accounts
                        .accounts
                        .first()
                        .map(|a| a.minecraft_name.clone())
                }),
                account_id: account,
                instance_status: "ready".into(),
                runtime_status: "ready".into(),
                account_status: if signed_in {
                    "ready"
                } else {
                    "reauthenticationRequired"
                }
                .into(),
                process_status: "idle".into(),
                blockers: if signed_in {
                    Vec::new()
                } else {
                    vec![api::PlayBlocker {
                        code: "launch_authentication_required".into(),
                        message: "Sign in to your Minecraft account before playing.".into(),
                    }]
                },
            };
            self.rt.spawn(async move {
                let _ = tx
                    .send(Message::Readiness(
                        serial,
                        process_serial,
                        Ok(decision),
                        None,
                    ))
                    .await;
            });
            return;
        }
        self.rt.spawn(async move {
            let (result, process) = tokio::join!(
                guarded(api::play_readiness(host, id.clone(), account)),
                guarded(api::launch_state(id)),
            );
            let _ = tx
                .send(Message::Readiness(
                    serial,
                    process_serial,
                    result,
                    process.ok(),
                ))
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
        let finished = matches!(child, Some("exited" | "failed" | "stopped"));
        let decision = self
            .readiness
            .as_ref()
            .filter(|decision| decision.instance_id == id && !finished)
            .map(|decision| decision.process_status.as_str());
        if child == Some("running") || decision == Some("running") {
            GamePhase::Running
        } else if self.launching.as_deref() == Some(id)
            || child == Some("starting")
            || decision == Some("starting")
        {
            GamePhase::Starting
        } else {
            GamePhase::Idle
        }
    }

    fn play_label(&self, id: &str) -> &'static str {
        match self.game_phase(id) {
            GamePhase::Running => {
                if self.can_kill(id) {
                    "Kill"
                } else {
                    "Running"
                }
            }
            GamePhase::Starting => {
                if self.can_kill(id) {
                    "Kill"
                } else {
                    "Starting\u{2026}"
                }
            }
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
                self.open_instance(id.clone(), InstanceTab::Settings, 0, window, cx);
                let taken = self.instance_detail.as_ref().is_some_and(|(target, tab)| {
                    self.page == 3 && target == &id && *tab == InstanceTab::Settings
                }) && self.instance_editor.as_ref().is_some_and(|editor| {
                    editor.update(cx, |editor, cx| editor.review_recovery_on_open(&id, cx))
                });
                if !taken {
                    self.inspect_recovery(id);
                }
            }
            BlockerAction::ReviewMods => {
                self.open_instance(id, InstanceTab::Content, 0, window, cx);
            }
            BlockerAction::JavaSettings => {
                self.open_instance(id, InstanceTab::Settings, 0, window, cx);
            }
            BlockerAction::OpenInstance => {
                self.open_instance(id, InstanceTab::Overview, 0, window, cx);
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
        self.launching = Some(instance_id.clone());
        self.maybe_open_log(instance_id.clone());
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
            // Servers launch from the current instance; worlds from their own.
            let current = mode.ends_with("server").then(|| instance_id.clone());
            let result = guarded(api::quick_play(host, mode, target_id, account, current)).await;
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
        if op == "play" {
            self.launching = Some(id.clone());
            self.maybe_open_log(id.clone());
        }
        let tx = self.tx.clone();
        let account = self.effective_play_account(&id);
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
    fn open_instance(
        &mut self,
        instance: String,
        tab: InstanceTab,
        kind: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .launcher
            .as_ref()
            .is_some_and(|state| state.instances.iter().any(|record| record.id == instance))
            || self
                .instance_editor
                .as_ref()
                .is_some_and(|editor| editor.read(cx).is_busy())
        {
            return;
        }
        if tab == InstanceTab::Content
            && self.content.as_ref().is_some_and(|content| {
                let content = content.read(cx);
                content.is_busy(cx) && !content.shows(&instance)
            })
        {
            self.status = "Finish the current content operation before switching instances.".into();
            return;
        }
        if self.page != 3 {
            self.open_page(3, window, cx);
        }
        if self.page != 3 {
            return;
        }
        if tab == InstanceTab::Content {
            self.ensure_content(window, cx);
            if let Some(content) = &self.content {
                content.update(cx, |content, cx| {
                    if !content.is_busy(cx) {
                        content.show(instance.clone(), kind, cx);
                    }
                });
            }
        } else if tab == InstanceTab::Worlds {
            self.ensure_worlds(window, cx);
            let qp_supported = self.launcher.as_ref().is_some_and(|s| {
                s.instances
                    .iter()
                    .find(|i| i.id == instance)
                    .is_some_and(|i| home_widgets::quick_play_supported(&i.minecraft_version))
            });
            if let Some(worlds) = &self.worlds {
                worlds.update(cx, |worlds, cx| {
                    worlds.set_instance(Some(instance.clone()), qp_supported, cx);
                });
            }
        } else if tab == InstanceTab::Screenshots {
            if let Some(editor) = &self.instance_editor {
                editor.update(cx, |editor, cx| {
                    editor.open_screenshots(instance.clone(), cx)
                });
            }
        } else if tab == InstanceTab::Settings {
            if let Some(editor) = &self.instance_editor {
                editor.update(cx, |editor, cx| {
                    editor.open_instance(instance.clone(), window, cx)
                });
            }
        }
        self.instance_detail = Some((instance.clone(), tab));
        self.reset_instance_scroll = true;
        self.record_nav(NavLocation {
            page: 3,
            instance_detail: Some((instance, tab)),
            content_kind: kind,
        });
    }

    fn record_nav(&mut self, loc: NavLocation) {
        if self.nav_restoring {
            return;
        }
        if self.nav_history.get(self.nav_index) == Some(&loc) {
            return;
        }
        if !self.nav_history.is_empty() {
            self.nav_history.truncate(self.nav_index + 1);
        }
        self.nav_history.push(loc);
        const MAX_NAV_HISTORY: usize = 64;
        if self.nav_history.len() > MAX_NAV_HISTORY {
            let excess = self.nav_history.len() - MAX_NAV_HISTORY;
            self.nav_history.drain(0..excess);
        }
        self.nav_index = self.nav_history.len().saturating_sub(1);
    }

    fn can_go_back(&self) -> bool {
        self.nav_index > 0
    }

    fn can_go_forward(&self) -> bool {
        self.nav_index + 1 < self.nav_history.len()
    }

    fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_go_back() || self.client_update_running(cx) {
            return;
        }
        self.nav_index -= 1;
        let loc = self.nav_history[self.nav_index].clone();
        self.restore_nav(loc, window, cx);
    }

    fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_go_forward() || self.client_update_running(cx) {
            return;
        }
        self.nav_index += 1;
        let loc = self.nav_history[self.nav_index].clone();
        self.restore_nav(loc, window, cx);
    }

    fn restore_nav(&mut self, loc: NavLocation, window: &mut Window, cx: &mut Context<Self>) {
        self.nav_restoring = true;
        if let Some((instance, tab)) = loc.instance_detail {
            self.open_instance(instance, tab, loc.content_kind, window, cx);
        } else {
            self.open_page(loc.page, window, cx);
        }
        self.nav_restoring = false;
        cx.notify();
    }

    fn select_and_play(&mut self, id: String, cx: &mut Context<Self>) {
        if self.selected_id() == Some(id.as_str()) && self.can_play(&id) {
            self.action("play", id, cx);
        } else {
            self.cli_launch_instance = Some(id.clone());
            self.action("select", id, cx);
        }
    }

    fn return_to_content(
        &mut self,
        instance: String,
        kind: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_instance(instance, InstanceTab::Content, kind, window, cx);
    }
    fn ensure_content(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.content.is_some() {
            return;
        }
        let content = cx.new(|cx| Content::new(self.host.clone(), self.rt.clone(), window, cx));
        self._subscriptions.push(cx.subscribe_in(
            &content,
            window,
            |this, _, event: &ContentEvent, window, cx| {
                match event {
                    ContentEvent::Changed => {
                        this.reload_guarded(cx);
                        if let Some(discover) = &this.discover {
                            discover.update(cx, |discover, cx| discover.content_changed(cx));
                        }
                        if let Some(editor) = &this.instance_editor {
                            editor.update(cx, |editor, cx| editor.refresh(cx));
                        }
                    }
                    ContentEvent::Discover { instance, kind } => {
                        this.open_discover_for(instance.clone(), *kind, window, cx);
                    }
                    ContentEvent::Project {
                        instance,
                        kind,
                        project_id,
                        title,
                    } => {
                        this.open_page(5, window, cx);
                        if let Some(discover) = &this.discover {
                            discover.update(cx, |discover, cx| {
                                discover.open_content_project(
                                    (**instance).clone(),
                                    *kind,
                                    project_id.clone(),
                                    title.clone(),
                                    cx,
                                );
                            });
                        }
                    }
                }
                cx.notify();
            },
        ));
        self.content = Some(content);
    }

    fn effective_play_account(&self, instance_id: &str) -> Option<String> {
        if let Some(pref) = self.launcher.as_ref().and_then(|s| {
            s.instances
                .iter()
                .find(|i| i.id == instance_id)
                .and_then(|i| i.configuration.preferred_account_id.as_deref())
        }) && self
            .accounts
            .accounts
            .iter()
            .any(|a| a.account_id == pref && a.status == "signedIn")
        {
            return Some(pref.to_owned());
        }
        self.accounts.selected_account_id.clone()
    }

    pub(crate) fn toggle_instance_pin(
        &mut self,
        id: String,
        pinned: bool,
        tags: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let r = guarded(api::set_instance_organization(host, id, pinned, tags)).await;
            let _ = tx.send(Message::Organization(r)).await;
        });
        cx.notify();
    }

    fn ensure_worlds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.worlds.is_some() {
            return;
        }
        let worlds =
            cx.new(|cx| worlds::Worlds::new(self.host.clone(), self.rt.clone(), window, cx));
        self._subscriptions.push(cx.subscribe(
            &worlds,
            |this, _, event: &worlds::WorldsEvent, cx| match event {
                worlds::WorldsEvent::QuickPlayWorld {
                    instance_id,
                    world_dir,
                } => {
                    this.quick_play_world_direct(instance_id.clone(), world_dir.clone(), cx);
                }
                worlds::WorldsEvent::Changed => {
                    this.reload_guarded(cx);
                }
            },
        ));
        self.worlds = Some(worlds);
    }

    fn quick_play_world_direct(
        &mut self,
        instance_id: String,
        world_dir: String,
        cx: &mut Context<Self>,
    ) {
        if self.controls_blocked() {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(account) = self.effective_play_account(&instance_id) else {
            self.status = "Select a signed-in Minecraft account first.".into();
            cx.notify();
            return;
        };
        self.launching = Some(instance_id.clone());
        self.maybe_open_log(instance_id.clone());
        self.busy = true;
        self.serial = self.serial.wrapping_add(1);
        self.readiness = None;
        self.status = format!("Launching world {world_dir}…");
        self.sync_mutation_guards(cx);
        let tx = self.tx.clone();
        let process_serial = self.process_serial;
        self.rt.spawn(async move {
            let target = format!("{instance_id}:{world_dir}");
            let result = guarded(api::quick_play(
                host,
                "saved-world".into(),
                target,
                account,
                None,
            ))
            .await;
            let _ = tx
                .send(Message::Launched(instance_id, process_serial, result))
                .await;
        });
        cx.notify();
    }

    fn play_offline(&mut self, id: String, cx: &mut Context<Self>) {
        if self.controls_blocked() {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(account) = self
            .effective_play_account(&id)
            .or_else(|| self.accounts.accounts.first().map(|a| a.account_id.clone()))
        else {
            self.status = "Sign in at least once online before playing offline.".into();
            cx.notify();
            return;
        };
        self.launching = Some(id.clone());
        self.maybe_open_log(id.clone());
        self.busy = true;
        self.serial = self.serial.wrapping_add(1);
        self.readiness = None;
        self.status = "Launching offline…".into();
        self.sync_mutation_guards(cx);
        let tx = self.tx.clone();
        let process_serial = self.process_serial;
        self.rt.spawn(async move {
            let result = guarded(api::play_offline(host, id.clone(), account)).await;
            let _ = tx.send(Message::Launched(id, process_serial, result)).await;
        });
        cx.notify();
    }

    fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use command_palette::{PaletteAction, PaletteItem};
        let mut items = vec![
            PaletteItem {
                title: "Home".into(),
                subtitle: "Launch hub and widgets (Ctrl+1)".into(),
                group: "Navigation",
                action: PaletteAction::NavigatePage(0),
            },
            PaletteItem {
                title: "Instances".into(),
                subtitle: "Browse and manage instances (Ctrl+2)".into(),
                group: "Navigation",
                action: PaletteAction::NavigatePage(3),
            },
            PaletteItem {
                title: "Discover".into(),
                subtitle: "Browse Modrinth mods and packs (Ctrl+3)".into(),
                group: "Navigation",
                action: PaletteAction::NavigatePage(5),
            },
            PaletteItem {
                title: "Pulsar".into(),
                subtitle: "Combat analytics and fight history (Ctrl+4)".into(),
                group: "Navigation",
                action: PaletteAction::NavigatePage(9),
            },
            PaletteItem {
                title: "Skins".into(),
                subtitle: "Skin library and 3D preview (Ctrl+5)".into(),
                group: "Navigation",
                action: PaletteAction::NavigatePage(7),
            },
            PaletteItem {
                title: "Accounts".into(),
                subtitle: "Manage Microsoft accounts".into(),
                group: "Navigation",
                action: PaletteAction::NavigatePage(1),
            },
            PaletteItem {
                title: "Settings".into(),
                subtitle: "Launcher preferences (Ctrl+,)".into(),
                group: "Settings",
                action: PaletteAction::NavigatePage(2),
            },
            PaletteItem {
                title: "Storage & Cache Manager".into(),
                subtitle: "Inspect disk usage and prune unused cache or Java runtimes".into(),
                group: "Settings",
                action: PaletteAction::OpenSettingsSection(5),
            },
            PaletteItem {
                title: "Game & Launch Settings".into(),
                subtitle: "Game log and on-launch window behavior".into(),
                group: "Settings",
                action: PaletteAction::OpenSettingsSection(4),
            },
            PaletteItem {
                title: "Check Updates".into(),
                subtitle: "Aurora Launcher & Aurora Client updates".into(),
                group: "Settings",
                action: PaletteAction::OpenSettingsSection(3),
            },
        ];
        if self.selected_id().is_some() {
            items.push(PaletteItem {
                title: "Open Game Log".into(),
                subtitle: "Live stdout/stderr and crash diagnostics (Ctrl+L)".into(),
                group: "Actions",
                action: PaletteAction::OpenGameLog,
            });
        }
        if let Some(state) = &self.launcher {
            for inst in &state.instances {
                let sub = format!(
                    "Minecraft {} · {}",
                    inst.minecraft_version,
                    navigation::short_platform_label(inst)
                );
                items.push(PaletteItem {
                    title: format!("Open {}", inst.display_name),
                    subtitle: sub.clone(),
                    group: "Instances",
                    action: PaletteAction::OpenInstance(inst.id.clone()),
                });
                items.push(PaletteItem {
                    title: format!("Play {}", inst.display_name),
                    subtitle: sub,
                    group: "Launch",
                    action: PaletteAction::PlayInstance(inst.id.clone()),
                });
            }
        }
        if let Some(palette) = &self.command_palette {
            palette.update(cx, |p, cx| p.open_with(items, window, cx));
        }
    }

    fn execute_palette_action(
        &mut self,
        action: command_palette::PaletteAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use command_palette::PaletteAction;
        match action {
            PaletteAction::NavigatePage(p) => self.open_page(p, window, cx),
            PaletteAction::OpenInstance(id) => {
                self.open_instance(id, InstanceTab::Overview, 0, window, cx);
            }
            PaletteAction::PlayInstance(id) => {
                self.select_and_play(id, cx);
            }
            PaletteAction::OpenSettingsSection(sec) => {
                self.open_page(2, window, cx);
                if let Some(settings) = &self.settings {
                    settings.update(cx, |s, cx| s.select_section(sec, cx));
                }
            }
            PaletteAction::OpenGameLog => {
                if let Some(id) = self.selected_id().map(str::to_owned) {
                    self.open_game_log(id, cx);
                }
            }
            PaletteAction::CreateInstance => {
                self.open_page(3, window, cx);
            }
        }
        cx.notify();
    }

    fn refresh_organization_and_behavior(&self) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let org = guarded(api::instance_organization(host.clone())).await;
            let _ = tx.send(Message::Organization(org)).await;
            if let Ok(beh) = guarded(api::on_launch_behavior(host, None)).await {
                let _ = tx.send(Message::OnLaunchBehavior(beh)).await;
            }
        });
    }

    fn check_pulsar_recap(&self, notify_if_increased: bool) {
        let Some(host) = self.host.clone().filter(|_| !showcase()) else {
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let count = guarded(api::pulsar_recent_widget(host, None, 1))
                .await
                .ok()
                .map(|w| w.total_fights);
            let _ = tx
                .send(Message::PulsarCount(count, notify_if_increased))
                .await;
        });
    }

    fn open_pulsar_for(&mut self, instance: String, window: &mut Window, cx: &mut Context<Self>) {
        self.open_page(9, window, cx);
        if self.page == 9 {
            if let Some(pulsar) = &self.pulsar {
                pulsar.update(cx, |pulsar, cx| pulsar.show(instance, cx));
            }
        }
    }
    fn open_page(&mut self, page: u8, window: &mut Window, cx: &mut Context<Self>) {
        if !navigation_accepted(self.page, page, self.client_update_running(cx)) {
            return;
        }
        if page == 3 {
            self.instance_detail = None;
            self.reset_instance_scroll = true;
        }
        // An unreleased Motion draft and an open accent picker never outlive a
        // navigation, including the one that leaves Settings.
        if let Some(settings) = &self.settings {
            settings.update(cx, |settings, cx| settings.cancel_previews(cx));
        }
        if page == 9 {
            if let Some(pulsar) = &self.pulsar {
                if self.page != 9 {
                    pulsar.update(cx, |pulsar, cx| pulsar.refresh(cx));
                }
            } else {
                self.pulsar =
                    Some(cx.new(|cx| Pulsar::new(self.host.clone(), self.rt.clone(), window, cx)));
                self._subscriptions.push(cx.subscribe_in(
                    self.pulsar.as_ref().unwrap(),
                    window,
                    |this, _, event: &pulsar::PulsarEvent, window, cx| match event {
                        pulsar::PulsarEvent::OpenInstances => this.open_page(3, window, cx),
                        pulsar::PulsarEvent::Launch(target) => {
                            let id = target
                                .clone()
                                .or_else(|| this.selected_id().map(str::to_owned))
                                .or_else(|| {
                                    this.launcher.as_ref().and_then(|s| {
                                        s.instances
                                            .iter()
                                            .find(|i| i.aurora.is_some())
                                            .or_else(|| s.instances.first())
                                            .map(|i| i.id.clone())
                                    })
                                });
                            if let Some(id) = id {
                                if this.can_play(&id) {
                                    this.action("play", id, cx);
                                } else {
                                    this.open_instance(id, InstanceTab::Overview, 0, window, cx);
                                }
                            } else {
                                this.open_page(3, window, cx);
                            }
                            cx.notify();
                        }
                    },
                ));
            }
        }
        if (self.page == 9) != (page == 9) {
            if let Some(pulsar) = &self.pulsar {
                pulsar.update(cx, |pulsar, cx| pulsar.set_active(page == 9, cx));
            }
        }
        if page == 7 && self.skins.is_none() {
            let preview = cx.new(|cx| SkinPreview::new(self.rt.clone(), window, cx));
            let skins = cx.new(|cx| Skins::new(self.host.clone(), self.rt.clone(), window, cx));
            skins.update(cx, |skins, _| skins.set_preview(preview.clone()));
            self._subscriptions.push(cx.subscribe_in(
                &skins,
                window,
                |this, _, event: &SkinsEvent, window, cx| {
                    match event {
                        SkinsEvent::OpenAccounts => this.open_page(1, window, cx),
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
                },
            ));
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
                self._subscriptions.push(cx.subscribe_in(
                    &syncing,
                    window,
                    |this, _, event: &SyncingEvent, window, cx| match event {
                        SyncingEvent::Changed => {
                            this.sync_error = None;
                            this.reload_guarded(cx);
                            if let Some(editor) = &this.instance_editor {
                                editor.update(cx, |editor, cx| editor.refresh(cx));
                            }
                            this.refresh_content_views(cx);
                            cx.notify();
                        }
                        SyncingEvent::OpenInstances => {
                            this.open_page(3, window, cx);
                            cx.notify();
                        }
                    },
                ));
                self.syncing = Some(syncing);
            }
        }
        if page == 5 || page == 8 || (page == 3 && self.discover.is_none()) {
            if let Some(discover) = &self.discover {
                if self.page != page {
                    discover.update(cx, |discover, cx| discover.refresh(cx));
                }
            } else {
                let discover =
                    cx.new(|cx| Discover::new(self.host.clone(), self.rt.clone(), window, cx));
                let downloads = discover.read(cx).downloads();
                self._subscriptions.push(cx.subscribe_in(
                    &downloads,
                    window,
                    |this, downloads, event: &DownloadsEvent, window, cx| {
                        if matches!(event, DownloadsEvent::OpenDiscover) {
                            this.open_page(5, window, cx);
                        }
                        // The Slint host toasts a settled install wherever the
                        // user is; the status line is this host's equivalent.
                        if matches!(event, DownloadsEvent::Settled { .. }) {
                            if let Some(notice) = downloads.read(cx).notice() {
                                let notice = notice.to_owned();
                                this.status = notice.clone();
                                cx.spawn(async move |this, cx| {
                                    cx.background_executor()
                                        .timer(std::time::Duration::from_secs(8))
                                        .await;
                                    let _ = this.update(cx, |this, cx| {
                                        if this.status == notice {
                                            this.status.clear();
                                            cx.notify();
                                        }
                                    });
                                })
                                .detach();
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
                            DiscoverEvent::BrowsePageChanged => {
                                this.reset_discover_scroll = true;
                                cx.notify();
                            }
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
                if page != 3 {
                    discover.update(cx, |discover, cx| discover.refresh(cx));
                }
                self.discover = Some(discover);
            }
        }
        if page == 2 && self.settings.is_none() {
            let settings =
                cx.new(|cx| Settings::new(self.host.clone(), self.rt.clone(), window, cx));
            self._subscriptions.push(cx.subscribe(
                &settings,
                |this, settings, event: &SettingsEvent, cx| {
                    if let SettingsEvent::Installing(busy) = event {
                        this.busy = *busy;
                        this.sync_mutation_guards(cx);
                        cx.notify();
                        return;
                    }
                    if let SettingsEvent::LaunchBehavior(behavior) = event {
                        this.on_launch_behavior = *behavior;
                        cx.notify();
                        return;
                    }
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
                            SettingsEvent::Installing(_) | SettingsEvent::LaunchBehavior(_) => {}
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
            settings.update(cx, |settings, _| {
                settings.set_client_updates(self.client_updates.clone())
            });
            self.settings = Some(settings);
        }
        self.page = page;
        self.record_nav(NavLocation {
            page,
            instance_detail: if page == 3 {
                self.instance_detail.clone()
            } else {
                None
            },
            content_kind: 0,
        });
        self.sync_mutation_guards(cx);
    }
    fn refresh_rail_and_activity(&mut self) {
        let Some(host) = self.host.clone() else {
            return;
        };
        // Load player head avatar only when missing or when the active signed-in
        // account changes, avoiding repeated disk reads and GPU atlas churn.
        let account_id = self.accounts.selected_account_id.clone().filter(|id| {
            self.accounts
                .accounts
                .iter()
                .any(|a| &a.account_id == id && a.status == "signedIn")
        });
        let avatar_stale = self.rail_avatar.is_none() || self.rail_avatar_account != account_id;
        if avatar_stale && (account_id.is_some() || showcase()) {
            self.rail_avatar_account = account_id.clone();
            let host_avatar = host.clone();
            let tx = self.tx.clone();
            self.rt.spawn(async move {
                let rgba = if showcase() {
                    api::skin_thumbnail(host_avatar, "00000000-0000-0000-0000-000000000001".into())
                        .await
                        .ok()
                        .flatten()
                        .map(|t| t.rgba)
                } else if let Some(id) = account_id {
                    api::account_avatar(host_avatar, id)
                        .await
                        .ok()
                        .flatten()
                        .map(|a| a.rgba)
                } else {
                    None
                };
                let _ = tx.send(Message::RailAvatar(rgba)).await;
            });
        }
        // Only query gameplay history summary when viewing Instance Detail or
        // before the active instance has an initial summary cached.
        if let Some(instance_id) = self.selected_id().map(str::to_owned)
            && ((self.page == 3 && self.instance_detail.is_some())
                || !self.hero_activity.contains_key(&instance_id))
        {
            let tx = self.tx.clone();
            self.rt.spawn(async move {
                let (playtime, servers) = tokio::join!(
                    api::playtime(host.clone(), Some(instance_id.clone())),
                    api::recent_servers(host, Some(instance_id.clone()), 1)
                );
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let mut stats: Vec<(gpui::SharedString, gpui::SharedString)> = Vec::new();
                if let Ok(list) = &servers {
                    if let Some(top) = list.first() {
                        let at = if top.last_played_at > 100_000_000_000 {
                            top.last_played_at / 1000
                        } else {
                            top.last_played_at
                        };
                        if at > 0 {
                            let label = match now.saturating_sub(at) {
                                0..=59 => "Just now".into(),
                                s @ 60..=3599 => format!("{}m ago", s / 60),
                                s @ 3600..=86399 => format!("{}h ago", s / 3600),
                                s => format!("{}d ago", s / 86400),
                            };
                            stats.push(("Last played".into(), label.into()));
                        }
                    }
                }
                if let Ok(pt) = &playtime {
                    if pt.last_7_days_ms > 0 {
                        let mins = pt.last_7_days_ms / 60_000;
                        let text = if mins >= 60 {
                            format!("{}h {}m", mins / 60, mins % 60)
                        } else {
                            format!("{mins}m")
                        };
                        stats.push(("This week".into(), text.into()));
                    }
                }
                if let Ok(list) = &servers {
                    if let Some(top) = list.first().filter(|s| !s.display_name.trim().is_empty()) {
                        stats.push(("Server".into(), top.display_name.clone().into()));
                    }
                }
                let _ = tx.send(Message::HeroActivity(instance_id, stats)).await;
            });
        }
    }

    fn receive(&mut self, msg: Message) {
        match msg {
            Message::MaybeOpenLog(_, _) => {}
            Message::StopResult(result) => {
                if let Err(error) = result {
                    self.status = error.message;
                }
            }
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
                    Ok(state) => {
                        if self.instance_detail.as_ref().is_some_and(|(id, _)| {
                            !state.instances.iter().any(|instance| &instance.id == id)
                        }) {
                            self.instance_detail = None;
                            self.reset_instance_scroll = true;
                        }
                        self.launcher = Some(state);
                    }
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
                self.refresh_rail_and_activity();
                self.refresh_organization_and_behavior();
                if startup {
                    self.start_update_check();
                    self.check_pulsar_recap(false);
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
                    self.set_update_status(update_overview_payload(&overview));
                }
            }
            Message::Launched(id, process_serial, result) => {
                self.launching = None;
                self.pending_stop = None;
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
            Message::RailAvatar(rgba) => {
                self.rail_avatar = rgba.and_then(|bytes| {
                    let pixels_count = bytes.len() / 4;
                    let side = (pixels_count as f64).sqrt() as u32;
                    if side == 0 || (side as usize) * (side as usize) * 4 != bytes.len() {
                        return None;
                    }
                    if side < 64 {
                        let img = image::RgbaImage::from_raw(side, side, bytes)?;
                        let scaled = image::imageops::resize(
                            &img,
                            64,
                            64,
                            image::imageops::FilterType::Nearest,
                        );
                        images::rgba_image(64, 64, scaled.into_raw())
                    } else {
                        images::rgba_image(side, side, bytes)
                    }
                });
            }
            Message::HeroActivity(id, stats) => {
                if stats.is_empty() {
                    self.hero_activity.remove(&id);
                } else {
                    self.hero_activity.insert(id, stats);
                }
            }
            Message::Organization(result) => {
                if let Ok(org) = result {
                    self.organization = org;
                }
            }
            Message::OnLaunchBehavior(behavior) => {
                self.on_launch_behavior = behavior;
            }
            Message::PulsarCount(count, notify_if_increased) => {
                if let Some(new_total) = count {
                    if notify_if_increased
                        && let Some(prev) = self.last_pulsar_count
                        && new_total > prev
                    {
                        let added = new_total - prev;
                        self.status = format!(
                            "Pulsar recorded {added} new {} this session — press Ctrl+4 to review.",
                            if added == 1 { "fight" } else { "fights" }
                        );
                    }
                    self.last_pulsar_count = Some(new_total);
                }
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
                        let terminal =
                            failed || process.status == "exited" || process.status == "stopped";
                        if (terminal || process.status == "running")
                            && self.launching.as_deref() == Some(id.as_str())
                        {
                            self.launching = None;
                            if terminal {
                                self.pending_stop = None;
                            }
                        }
                        self.process_serial += 1;
                        let is_running = process.status == "running";
                        self.processes.insert(id.clone(), process);
                        if is_running {
                            if let Some(hwnd) = self.main_hwnd {
                                match self.on_launch_behavior {
                                    api::OnLaunchBehavior::KeepOpen => {}
                                    api::OnLaunchBehavior::Minimize => tray::minimize_window(hwnd),
                                    api::OnLaunchBehavior::HideToTray => {
                                        self.tray_active = true;
                                        tray::hide_to_tray(hwnd);
                                    }
                                }
                            }
                        }
                        if terminal {
                            let any_alive = self
                                .processes
                                .values()
                                .any(|p| matches!(p.status.as_str(), "starting" | "running"));
                            if !any_alive && self.tray_active {
                                self.tray_active = false;
                                if let Some(hwnd) = self.main_hwnd {
                                    tray::restore_window(hwnd);
                                }
                            }
                            self.check_pulsar_recap(true);
                        }
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
                        self.set_update_status(payload);
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
        window: &mut Window,
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
        let has_aurora = instance.is_some_and(|i| i.aurora.is_some());
        // Balanced, breathable two-line identity stack inside the Home instance card:
        // condensed enough to leave prime room for widgets, spacious enough to feel primary.
        let mut head = div().flex().flex_col().gap(px(10.)).flex_1().min_w(px(0.));
        if let Some(instance) = instance {
            head = head.child(div().flex().items_center().gap(px(10.)).child(
                if launcher.instances.len() > 1 {
                    self.instance_switcher(launcher, selected.as_deref(), window, cx)
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(ui::TEXT_DISPLAY + 2.))
                        .line_height(px(34.))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(t.foreground)
                        .text_ellipsis()
                        .child(instance.display_name.clone())
                        .into_any_element()
                },
            ));
            let mut metadata = div().flex().items_center().flex_wrap().gap(px(8.));
            if let Some(pin) = &instance.aurora {
                metadata = metadata.child(ui::aurora_badge(
                    t,
                    format!("Aurora Client {}", pin.version),
                ));
            }
            metadata = metadata.child(ui::badge(
                t,
                format!(
                    "{} · {}",
                    instance.minecraft_version,
                    navigation::short_platform_label(instance)
                ),
                ui::Tone::Neutral,
            ));
            if let Some((label, tone)) = self.readiness_badge(&instance.id) {
                metadata = metadata.child(ui::status_badge(t, label, tone));
            }
            head = head.child(metadata);
        } else {
            head = head.child(self.instance_switcher(launcher, selected.as_deref(), window, cx));
        }
        let hero_icon = instance.map(|instance| {
            aurora_instance_icon(
                stored_icon(&launcher.icons, &instance.id),
                58.,
                true,
                instance.aurora.is_some(),
                palette,
            )
        });
        let details = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(18.))
            .flex_1()
            .min_w(px(0.))
            .children(hero_icon)
            .child(head);
        let mut primary_cta: Option<gpui::AnyElement> = None;
        if let Some(id) = selected.clone() {
            let play_id = id.clone();
            let can_kill = self.can_kill(&id);
            let needs_sign_in = !can_kill
                && self
                    .visible_blockers(&id)
                    .iter()
                    .any(|blocker| blocker.code == "launch_authentication_required");
            let can_offline = !can_kill
                && self
                    .readiness
                    .as_ref()
                    .is_some_and(|r| r.instance_id == id && !r.ready && r.can_launch_offline);
            let offline_id = id.clone();
            primary_cta = Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .when(can_offline, |row| {
                        row.child(
                            self.button("play-offline", "Play Offline")
                                .variant(ButtonVariant::Default)
                                .size(ButtonSize::Large)
                                .icon("wifi_off")
                                .disabled(self.controls_blocked())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.play_offline(offline_id.clone(), cx);
                                })),
                        )
                    })
                    .child(
                        self.button(
                            "play",
                            if needs_sign_in {
                                "Sign in to play"
                            } else {
                                self.play_label(&id)
                            },
                        )
                        .variant(if can_kill {
                            ButtonVariant::Danger
                        } else {
                            ButtonVariant::Primary
                        })
                        .size(ButtonSize::Large)
                        .icon(if can_kill {
                            "stop"
                        } else if needs_sign_in {
                            "login"
                        } else {
                            "play_arrow"
                        })
                        .disabled(if needs_sign_in {
                            self.controls_blocked()
                        } else {
                            !can_kill && !self.can_play(&id)
                        })
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                if can_kill {
                                    this.request_kill(play_id.clone(), cx);
                                } else if needs_sign_in {
                                    this.open_page(1, window, cx);
                                } else {
                                    this.action("play", play_id.clone(), cx);
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .into_any_element(),
            );
        }
        // Spacious yet condensed Home launch card leaving prime canvas for widgets.
        let stacked = window.viewport_size().width < px(900.);
        let hero_card = if has_aurora {
            ui::aurora_card(t)
        } else {
            ui::card(t)
        };
        out.push(
            hero_card
                .px(px(22.))
                .py(px(18.))
                .when(!stacked, |card| {
                    card.flex_row().items_center().justify_between()
                })
                .gap(px(20.))
                .child(details)
                .when_some(primary_cta, |card, cta| {
                    card.child(div().relative().flex().flex_shrink_0().child(cta))
                })
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
                    "stopped" if process.message.is_some() => "Stopped by you.".into(),
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
            for (i, blocker) in self
                .visible_blockers(&id)
                .iter()
                .filter(|blocker| blocker.code != "launch_authentication_required")
                .enumerate()
            {
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
                        .bg(gpui::Hsla {
                            a: 0.08,
                            ..t.warning
                        })
                        .border_1()
                        .border_color(gpui::Hsla {
                            a: 0.25,
                            ..t.warning
                        })
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
        if self
            .processes
            .get(id)
            .is_some_and(|p| p.status == "stopped" && p.message.is_some())
        {
            return Some(("Stopped", ui::Tone::Neutral));
        }
        let decision = self
            .readiness
            .as_ref()
            .filter(|decision| decision.instance_id == id);
        match decision {
            Some(decision) if decision.ready => Some(("Ready", ui::Tone::Success)),
            Some(decision) if decision.can_prepare => {
                Some(("Needs preparation", ui::Tone::Warning))
            }
            Some(decision)
                if decision
                    .blockers
                    .iter()
                    .any(|blocker| blocker.code == "launch_authentication_required") =>
            {
                Some(("Sign in needed", ui::Tone::Warning))
            }
            Some(_) => Some(("Action needed", ui::Tone::Warning)),
            None if self.readiness_in_flight || self.load_in_flight => {
                Some(("Checking", ui::Tone::Neutral))
            }
            None => None,
        }
    }
}
fn instance_metadata(instance: &api::InstanceSummary) -> String {
    let mut text = format!(
        "Minecraft {} · {}",
        instance.minecraft_version,
        instance.platform_label()
    );
    if let Some(aurora) = &instance.aurora {
        text.push_str(&format!(" · Aurora {}", aurora.version));
    }
    text
}

impl Launcher {
    fn refresh_page(&mut self, cx: &mut Context<Self>) {
        self.reload_guarded(cx);
        if self.page == 0 {
            if let Some(home) = &self.home_widgets {
                home.update(cx, |home, cx| home.refresh(cx));
            }
        }
        if let Some(updates) = &self.client_updates {
            updates.update(cx, |updates, cx| updates.refresh(cx));
        }
        if self.page == 2 {
            if let Some(settings) = &self.settings {
                settings.update(cx, |settings, cx| settings.refresh(cx));
            }
        }
        if self.page == 3 {
            if let Some(editor) = &self.instance_editor {
                editor.update(cx, |editor, cx| editor.refresh(cx));
            }
            if let Some(content) = &self.content {
                content.update(cx, |content, cx| content.refresh(cx));
            }
            if let Some(worlds) = &self.worlds {
                worlds.update(cx, |worlds, cx| worlds.refresh(cx));
            }
        }
        if self.page == 9 {
            if let Some(pulsar) = &self.pulsar {
                pulsar.update(cx, |pulsar, cx| pulsar.refresh(cx));
            }
        }
        if self.page == 5 {
            if let Some(discover) = &self.discover {
                discover.update(cx, |discover, cx| discover.refresh(cx));
            }
        }
        if self.page == 6 {
            if let Some(syncing) = &self.syncing {
                syncing.update(cx, |syncing, cx| syncing.refresh(cx));
            }
        }
        if self.page == 7 {
            if let Some(skins) = &self.skins {
                skins.update(cx, |skins, cx| skins.refresh(cx));
            }
        }
        self.sync_mutation_guards(cx);
        cx.notify();
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
        let page_scroll = window
            .use_keyed_state(("page-scroll", self.page as usize), cx, |_, _| {
                gpui::ScrollHandle::new()
            })
            .read(cx)
            .clone();
        if self.page == 5 && std::mem::take(&mut self.reset_discover_scroll) {
            page_scroll.set_offset(gpui::point(px(0.), px(0.)));
        }
        if self.page == 3
            && showcase()
            && !self.showcase_instance_opened
            && self.instance_detail.is_none()
        {
            let section_env = std::env::var("AURORA_SHOWCASE_SECTION").ok();
            let target_tab = match section_env.as_deref() {
                Some("screenshots") => Some(InstanceTab::Screenshots),
                Some("overview") => Some(InstanceTab::Overview),
                Some("settings") => Some(InstanceTab::Settings),
                Some("content") => Some(InstanceTab::Content),
                _ => None,
            };
            if let Some(tab) = target_tab {
                let wanted = std::env::var("AURORA_SHOWCASE_INSTANCE").unwrap_or_default();
                if wanted != "none" {
                    let index = wanted.parse::<usize>().unwrap_or(0);
                    if let Some(id) = self
                        .launcher
                        .as_ref()
                        .and_then(|state| state.instances.get(index))
                        .map(|instance| instance.id.clone())
                    {
                        self.showcase_instance_opened = true;
                        self.open_instance(id, tab, 0, window, cx);
                    }
                }
            }
        }
        if self.page == 3 && std::mem::take(&mut self.reset_instance_scroll) {
            page_scroll.set_offset(gpui::point(px(0.), px(0.)));
        }
        let mut inner = div()
            .flex()
            .flex_col()
            .gap(px(ui::PAGE_GAP))
            .w_full()
            .max_w(px(ui::MAX_CONTENT_WIDTH))
            .mx_auto()
            .min_w(px(0.));
        if showcase() {
            // Design screenshots can start part-way down a long page.
            if let Some(y) = std::env::var("AURORA_SHOWCASE_SCROLL")
                .ok()
                .and_then(|y| y.parse::<f32>().ok())
            {
                page_scroll.set_offset(gpui::point(px(0.), px(-y)));
            }
        }
        match self.page {
            // The startup load failed: nothing else is trustworthy until it succeeds.
            _ if self.fatal_error.is_some() => {
                inner = inner
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
                inner = inner
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(16.))
                            .child(ui::page_header(&palette, "Home", None))
                            .when_some(self.home_widgets.as_ref(), |header, home| {
                                header.child(home.read(cx).customize_button(home.clone()))
                            }),
                    )
                    .children(self.home_children(&palette, window, cx));
                if let Some(home) = &self.home_widgets {
                    inner = inner.child(home.clone());
                }
            }
            1 => {
                if let Some(page) = &self.accounts_page {
                    inner = inner.child(page.clone());
                }
            }
            7 => {
                inner = inner.child(ui::page_header(&palette, "Skins", None));
                if let Some(skins) = &self.skins {
                    inner = inner.child(skins.clone());
                }
            }
            3 => {
                inner = inner.children(self.instances_children(&palette, window, cx));
            }
            5 => {
                if let Some(page) = &self.discover {
                    inner = inner.child(page.clone());
                }
            }
            9 => {
                if let Some(pulsar) = &self.pulsar {
                    // Pulsar's PageUp/PageDown/Home/End drive this page's scroll.
                    pulsar.update(cx, |pulsar, _| {
                        pulsar.page_scroll = Some(page_scroll.clone())
                    });
                    inner = inner.child(pulsar.clone());
                }
            }
            8 => {
                // Downloads are now floating fade-out popup notifications; fall back to Home.
                inner = inner
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(16.))
                            .child(ui::page_header(&palette, "Home", None))
                            .when_some(self.home_widgets.as_ref(), |header, home| {
                                header.child(home.read(cx).customize_button(home.clone()))
                            }),
                    )
                    .children(self.home_children(&palette, window, cx));
                if let Some(home) = &self.home_widgets {
                    inner = inner.child(home.clone());
                }
            }
            6 => {
                if let Some(page) = &self.syncing {
                    inner = inner.child(page.clone());
                }
            }
            _ => {
                if let Some(settings) = &self.settings {
                    inner = inner.child(settings.clone());
                }
            }
        }
        let mut content = div()
            .flex()
            .flex_col()
            .px(px(navigation::PAGE_GUTTER))
            .pt(px(8.))
            .pb(px(40.))
            .flex_1()
            .id("content")
            .min_w(px(0.))
            .min_h(px(0.))
            .track_scroll(&page_scroll)
            .overflow_y_scroll()
            .child(inner);
        if self.page == 3
            && self.instance_detail.is_none()
            && self.fatal_error.is_none()
            && self.pack_import.is_some()
        {
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
        // Clean browser-style back/forward navigation + contextual status chips.
        let mut title_left = div().flex().items_center().gap(px(4.));
        let nav_busy = self.client_update_running(cx);
        let back_enabled = self.can_go_back() && !nav_busy;
        let forward_enabled = self.can_go_forward() && !nav_busy;
        let nav_hover = palette.control_hover;
        title_left = title_left
            .child(
                gpui_kit::base::Button::new("title-nav-back")
                    .accessibility_label("Back (Alt+Left or Mouse Back)")
                    .disabled(!back_enabled)
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(palette.radius)
                    .hover(move |s| s.bg(nav_hover))
                    .child(symbols::icon(
                        Some("arrow_back"),
                        px(16.),
                        if back_enabled {
                            palette.foreground
                        } else {
                            palette.subtle_foreground
                        },
                    ))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.go_back(window, cx);
                    })),
            )
            .child(
                gpui_kit::base::Button::new("title-nav-forward")
                    .accessibility_label("Forward (Alt+Right or Mouse Forward)")
                    .disabled(!forward_enabled)
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .mr(px(4.))
                    .rounded(palette.radius)
                    .hover(move |s| s.bg(nav_hover))
                    .child(symbols::icon(
                        Some("arrow_forward"),
                        px(16.),
                        if forward_enabled {
                            palette.foreground
                        } else {
                            palette.subtle_foreground
                        },
                    ))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.go_forward(window, cx);
                    })),
            );
        // Global Launcher update chip: surfaces only when a signed launcher update is offered.
        if let Some(ver) = self
            .update_status
            .as_ref()
            .filter(|v| v["launcher"]["availability"]["status"].as_str() == Some("updateAvailable"))
            .and_then(|v| v["launcher"]["availability"]["version"].as_str())
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
        {
            title_left = title_left.child(
                AuroraButton::new(
                    "title-launcher-update",
                    format!("Launcher {ver} update available"),
                )
                .variant(ButtonVariant::Soft)
                .size(ButtonSize::Small)
                .icon("system_update")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_page(2, window, cx);
                    if let Some(settings) = &this.settings {
                        settings.update(cx, |s, cx| s.select_section(3, cx));
                    }
                    cx.notify();
                })),
            );
        }
        // Global Aurora Client update chip: surfaces only when a real Client update is ready to review.
        if let Some(ver) = self.launcher.as_ref().and_then(|s| {
            s.instances
                .iter()
                .find_map(|i| self.client_offer(i).map(str::to_owned))
        }) {
            title_left = title_left.child(
                AuroraButton::new(
                    "title-aurora-update",
                    format!("Aurora Client {ver} update available"),
                )
                .variant(ButtonVariant::Soft)
                .size(ButtonSize::Small)
                .icon("system_update")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_page(2, window, cx);
                    if let Some(settings) = &this.settings {
                        settings.update(cx, |s, cx| s.select_section(3, cx));
                    }
                    cx.notify();
                })),
            );
        }
        // Live "Running now" strip across all concurrent active instances.
        if let Some(state) = &self.launcher {
            let active_runs: Vec<(String, String, GamePhase)> = state
                .instances
                .iter()
                .filter_map(|i| {
                    let phase = self.game_phase(&i.id);
                    matches!(phase, GamePhase::Running | GamePhase::Starting)
                        .then(|| (i.id.clone(), i.display_name.clone(), phase))
                })
                .collect();
            for (idx, (run_id, run_name, phase)) in active_runs.iter().take(2).enumerate() {
                let state_word = if *phase == GamePhase::Running {
                    "Running"
                } else {
                    "Starting…"
                };
                let log_id = run_id.clone();
                let stop_id = run_id.clone();
                title_left = title_left.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(ui::status_badge(
                            &palette,
                            format!("{run_name} · {state_word}"),
                            if *phase == GamePhase::Running {
                                ui::Tone::Success
                            } else {
                                ui::Tone::Accent
                            },
                        ))
                        .child(
                            AuroraButton::new(("title-log-game", idx), "Log")
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Small)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_game_log(log_id.clone(), cx);
                                })),
                        )
                        .when(self.can_kill(run_id), |row| {
                            row.child(
                                AuroraButton::new(("title-stop-game", idx), "Stop")
                                    .variant(ButtonVariant::DangerSubtle)
                                    .size(ButtonSize::Small)
                                    .icon("stop")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.request_kill(stop_id.clone(), cx);
                                        cx.notify();
                                    })),
                            )
                        }),
                );
            }
            if active_runs.len() > 2 {
                title_left = title_left.child(ui::badge(
                    &palette,
                    format!("+{} more", active_runs.len() - 2),
                    ui::Tone::Success,
                ));
            }
        }

        let refresh_disabled = self.busy
            || self.account_busy
            || self.skin_busy
            || self.client_update_running(cx)
            || self.load_in_flight
            || self.host.is_none();
        let hover_refresh = palette.control_hover;
        let title_right = gpui_kit::base::Button::new("title-refresh-launcher")
            .accessibility_label("Refresh")
            .disabled(refresh_disabled)
            .flex()
            .items_center()
            .justify_center()
            .size(px(30.))
            .mr(px(6.))
            .rounded(palette.radius)
            .hover(move |s| s.bg(hover_refresh))
            .child(symbols::icon(
                Some("refresh"),
                px(18.),
                if refresh_disabled {
                    palette.subtle_foreground
                } else {
                    palette.muted_foreground
                },
            ))
            .on_click(cx.listener(|this, _, _, cx| this.refresh_page(cx)));

        div()
            .size_full()
            .flex()
            // The Borealis layer is absolute inside this root. It is painted
            // first, behind the sidebar, title strip and page content, and has
            // no mouse handlers or window control area of its own.
            .relative()
            .bg(palette.background)
            .text_color(palette.foreground)
            .font_family(palette.font_family)
            .when_some(self.root_focus.clone(), |d, focus| d.track_focus(&focus))
            .on_mouse_down(
                gpui::MouseButton::Navigate(gpui::NavigationDirection::Back),
                cx.listener(|this, _, window, cx| {
                    this.go_back(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(
                gpui::MouseButton::Navigate(gpui::NavigationDirection::Forward),
                cx.listener(|this, _, window, cx| {
                    this.go_forward(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                let m = event.keystroke.modifiers;
                if m.alt && !m.control && !m.platform && !m.shift {
                    match event.keystroke.key.as_str() {
                        "left" => {
                            this.go_back(window, cx);
                            cx.stop_propagation();
                            return;
                        }
                        "right" => {
                            this.go_forward(window, cx);
                            cx.stop_propagation();
                            return;
                        }
                        _ => {}
                    }
                }
                if !(m.control || m.platform) || m.alt {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "k" | "K" => {
                        this.open_command_palette(window, cx);
                        cx.stop_propagation();
                    }
                    "1" => {
                        this.open_page(0, window, cx);
                        cx.stop_propagation();
                    }
                    "2" => {
                        this.open_page(3, window, cx);
                        cx.stop_propagation();
                    }
                    "3" => {
                        this.open_page(5, window, cx);
                        cx.stop_propagation();
                    }
                    "4" => {
                        this.open_page(9, window, cx);
                        cx.stop_propagation();
                    }
                    "5" => {
                        this.open_page(7, window, cx);
                        cx.stop_propagation();
                    }
                    "," => {
                        this.open_page(2, window, cx);
                        cx.stop_propagation();
                    }
                    "l" | "L" => {
                        if let Some(id) = this.selected_id().map(str::to_owned) {
                            this.open_game_log(id, cx);
                            cx.stop_propagation();
                        }
                    }
                    _ => {}
                }
            }))
            .children(self.borealis.clone())
            .child(self.navigation_rail(&palette, downloads_active, window, cx))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .when(chrome::custom(), |column| {
                        column.child(chrome::title_bar_with_context(
                            window,
                            cx,
                            &palette,
                            title_left,
                            title_right,
                        ))
                    })
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w(px(0.))
                            .min_h(px(0.))
                            .flex()
                            .flex_col()
                            .child(content)
                            .child(ui::scrollbar(&palette, "page-scrollbar", &page_scroll)),
                    )
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
            .children(self.downloads.clone())
            .children(self.command_palette.clone())
            .child(self.kill_dialog(cx))
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_launch_instance: Option<String> = {
        let mut args = std::env::args().skip(1);
        let mut found = None;
        while let Some(arg) = args.next() {
            if arg == "--launch-instance" {
                found = args.next();
                break;
            }
        }
        found
    };
    let showcase_root = showcase_root();
    #[cfg(debug_assertions)]
    if std::env::args().any(|a| a == "--showcase") && showcase_root.is_none() {
        return Err("--showcase needs a directory containing AURORA_DISPOSABLE_DIAGNOSTIC".into());
    }
    SHOWCASE.store(
        showcase_root.is_some(),
        std::sync::atomic::Ordering::Relaxed,
    );
    let diagnostic = cfg!(debug_assertions)
        && (std::env::args().any(|a| a == "--diagnostic") || showcase_root.is_some());
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
        .worker_threads(4)
        .max_blocking_threads(16)
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
            showcase_root
                .map(Ok)
                .unwrap_or_else(|| platform_local_data_dir(APPLICATION_IDENTIFIER)),
            env!("CARGO_PKG_VERSION"),
            rt.clone(),
            move |name, payload| {
                let _ = events.try_send(Message::Event(name.to_owned(), payload));
            },
        ))
    };
    if let Some(host) = host.as_ref().filter(|_| !diagnostic).cloned() {
        rt.spawn_blocking(move || {
            if api::initialize_discord(host).is_err() {
                eprintln!("[aurora-launcher] Discord preferences unavailable; see Settings");
            }
        });
    }
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
            .unwrap_or_else(|| {
                if showcase() && skins_showcase::state() == "narrow" {
                    chrome::min_size()
                } else {
                    chrome::default_size()
                }
            });
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
                    let command_palette =
                        cx.new(|cx| command_palette::CommandPalette::new(window, cx));
                    let instances_search = cx.new(|cx| {
                        gpui_kit::base::input::InputState::new(window, cx)
                            .placeholder("Search instances or #tag…")
                    });
                    let main_hwnd = file_picker::owner(window);
                    let mut subscriptions = vec![
                        cx.subscribe_in(
                            &command_palette,
                            window,
                            |this: &mut Launcher, _, event: &command_palette::CommandPaletteEvent, window, cx| {
                                if let command_palette::CommandPaletteEvent::Execute(action) = event {
                                    this.execute_palette_action(action.clone(), window, cx);
                                }
                            },
                        ),
                        cx.subscribe(&instances_search, |_this: &mut Launcher, _, _: &gpui_kit::base::input::InputEvent, cx| {
                            cx.notify();
                        }),
                        cx.subscribe_in(&home_widgets, window, |this: &mut Launcher, _, event: &HomeWidgetsEvent, window, cx| {
                            match event {
                                // Home owns acknowledged layout/favorite state;
                                // registry reloads would invalidate unrelated drafts.
                                HomeWidgetsEvent::Changed => {},
                                HomeWidgetsEvent::BusyChanged(busy) => this.home_busy = *busy,
                                HomeWidgetsEvent::QuickPlay { mode, target_id, instance_id } => {
                                    this.quick_play(mode.clone(), target_id.clone(), instance_id.clone(), cx);
                                }
                                HomeWidgetsEvent::OpenPulsar => {
                                    this.open_page(9, window, cx);
                                }
                            }
                            this.sync_mutation_guards(cx);
                            cx.notify();
                        }),
                        cx.subscribe(&client_updates, |this: &mut Launcher, _, event: &ClientUpdatesEvent, cx| {
                            match event {
                                ClientUpdatesEvent::BusyChanged(busy) => {
                                    this.client_update_busy = *busy;
                                    if *busy && this.client_update_instance.is_none() {
                                        this.client_update_instance = this.client_update_target().map(|target| target.id);
                                    }
                                },
                                ClientUpdatesEvent::SelectInstance(id) => this.client_update_instance = Some(id.clone()),
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
                        cx.subscribe_in(
                            &editor,
                            window,
                            |this: &mut Launcher, _, event: &InstanceEditorEvent, window, cx| {
                                match event {
                                    InstanceEditorEvent::Changed => {
                                        this.reload_guarded(cx);
                                        this.refresh_content_views(cx);
                                        if let Some(syncing) = &this.syncing {
                                            syncing.update(cx, |syncing, cx| syncing.refresh(cx));
                                        }
                                    }
                                    InstanceEditorEvent::OpenPulsar { instance } => {
                                        this.open_pulsar_for(instance.clone(), window, cx);
                                    }
                                    InstanceEditorEvent::OpenContent { instance } => {
                                        this.return_to_content(instance.clone(), 0, window, cx);
                                    }
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
                                PackImportEvent::InstallFolder(job) => {
                                    if this.downloads.is_none() {
                                        let prev = this.page;
                                        this.open_page(5, window, cx);
                                        if prev != 5 {
                                            this.open_page(prev, window, cx);
                                        }
                                    }
                                    let queued = this.downloads.as_ref().is_some_and(|downloads| {
                                        downloads.update(cx, |downloads, cx| downloads.submit(job.clone(), cx))
                                    });
                                    if !queued {
                                        this.status = "The installation could not be queued. Review the folder again.".into();
                                    }
                                }
                                PackImportEvent::OpenSync => {
                                    this.open_page(6, window, cx);
                                }
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
                    let root_focus = cx.focus_handle();
                    if !diagnostic {
                        window.focus(&root_focus, cx);
                    }
                    let mut view = Launcher {
                        root_focus: Some(root_focus),
                        nav_history: vec![NavLocation::home()],
                        nav_index: 0,
                        nav_restoring: false,
                        borealis: Some(borealis),
                        home_widgets: Some(home_widgets),
                        client_updates: Some(client_updates),
                        instance_editor: Some(editor),
                        instance_creator: Some(creator),
                        settings: None,
                        content: None,
                        worlds: None,
                        command_palette: Some(command_palette),
                        pulsar: None,
                        discover: None,
                        downloads: None,
                        syncing: None,
                        accounts_page: None,
                        skins: None,
                        skin_preview: None,
                        pack_import: Some(pack_import),
                        instances_search: Some(instances_search),
                        instances_sort: 0,
                        instances_tag_filter: None,
                        organization: Default::default(),
                        on_launch_behavior: api::OnLaunchBehavior::KeepOpen,
                        cli_launch_instance: cli_launch_instance.clone(),
                        last_pulsar_count: None,
                        main_hwnd,
                        tray_active: false,
                        _subscriptions: subscriptions,
                        host,
                        rt,
                        tx,
                        page: 0,
                        sidebar_collapsed: true,
                        instances_filter: 0,
                        instance_detail: None,
                        reset_instance_scroll: false,
                        showcase_instance_opened: false,
                        reset_discover_scroll: false,
                        serial: 0,
                        busy: false,
                        account_busy: false,
                        skin_busy: false,
                        client_update_busy: false,
                        client_update_instance: None,
                        launcher_update_pending: false,
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
                        rail_avatar: None,
                        rail_avatar_account: None,
                        hero_activity: HashMap::new(),
                        appearance: None,
                        readiness: None,
                        processes: HashMap::new(),
                        game_showcase_applied: false,
                        launching: None,
                        pending_stop: None,
                        kill_confirmation: None,
                        log_windows: HashMap::new(),
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
                                        if let Some(pulsar) = &view.pulsar {
                                            pulsar.update(cx, |pulsar, cx| pulsar.on_host_event(name, payload, cx));
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
                                    let readiness_settled = matches!(&message, Message::Readiness(..));
                                    if let Message::MaybeOpenLog(id, true) = &message { view.open_game_log(id.clone(), cx); }
                                    if let Message::Event(name, payload) = &message {
                                        if name == "launch-state" && payload.get("status").and_then(|v| v.as_str()) == Some("starting") {
                                            if payload.get("instanceId").and_then(|v| v.as_str()) == view.pending_stop.as_deref() {
                                                if let Some(id) = view.pending_stop.take() { view.send_stop(id, None); }
                                            }
                                        }
                                    }
                                    view.receive(message);
                                    view.apply_game_showcase(cx);
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
                                        if let Some(editor) = &view.instance_editor {
                                            let accounts = view.accounts.accounts.clone();
                                            editor.update(cx, |ed, _| ed.set_accounts(accounts));
                                        }
                                        if let Some(target_id) = view.cli_launch_instance.clone()
                                            && view.launcher.as_ref().is_some_and(|s| s.instances.iter().any(|i| i.id == target_id))
                                        {
                                            if view.selected_id() != Some(target_id.as_str()) {
                                                view.action("select", target_id, cx);
                                            } else if view.can_play(&target_id) {
                                                view.cli_launch_instance = None;
                                                view.action("play", target_id, cx);
                                            }
                                        }
                                    }
                                    if readiness_settled {
                                        if let Some(target_id) = view.cli_launch_instance.clone() {
                                            if view.selected_id() == Some(target_id.as_str())
                                                && view.can_play(&target_id)
                                            {
                                                view.cli_launch_instance = None;
                                                view.action("play", target_id, cx);
                                            }
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
    runtime.block_on(aurora_launcher_lib::discord::shutdown());
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
    fn instances_page_drop_outline_accepts_one_mrpack_or_folder() {
        let pack = |name: &str| PathBuf::from(name);
        let is_single_modpack =
            |paths: &[PathBuf]| pack_import::classify_modpack_drop(paths).is_ok();
        assert!(is_single_modpack(&[pack("C:\\packs\\Fabulous.mrpack")]));
        assert!(is_single_modpack(&[pack("pack.MRPACK")]));
        let folder = std::env::temp_dir().join(format!("aurora-pack-drop-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let accepts_folder = is_single_modpack(std::slice::from_ref(&folder));
        std::fs::remove_dir(&folder).unwrap();
        assert!(accepts_folder);
        assert!(!is_single_modpack(&[]));
        assert!(!is_single_modpack(&[pack("pack.zip")]));
        assert!(!is_single_modpack(&[pack("pack")]));
        assert!(!is_single_modpack(&[pack("a.mrpack"), pack("b.mrpack")]));
    }

    #[test]
    fn nav_history_tracks_back_forward_dedup_and_truncates_tail() {
        let mut v = view();
        assert!(!v.can_go_back());
        assert!(!v.can_go_forward());

        let instances = NavLocation {
            page: 3,
            instance_detail: None,
            content_kind: 0,
        };
        let detail = NavLocation {
            page: 3,
            instance_detail: Some(("inst-1".into(), InstanceTab::Content)),
            content_kind: 0,
        };
        v.record_nav(instances.clone());
        v.record_nav(instances.clone()); // duplicate ignored
        v.record_nav(detail.clone());
        assert_eq!(v.nav_history.len(), 3);
        assert!(v.can_go_back());
        assert!(!v.can_go_forward());

        v.nav_index = 1; // stepped back to instances
        assert!(v.can_go_back());
        assert!(v.can_go_forward());

        // Navigating to a new page truncates forward history.
        let settings = NavLocation {
            page: 2,
            instance_detail: None,
            content_kind: 0,
        };
        v.record_nav(settings.clone());
        assert_eq!(v.nav_history.len(), 3);
        assert_eq!(v.nav_history[2], settings);
        assert!(!v.can_go_forward());
    }

    fn view() -> Launcher {
        let (tx, _) = async_channel::unbounded();
        Launcher {
            root_focus: None,
            nav_history: vec![NavLocation::home()],
            nav_index: 0,
            nav_restoring: false,
            borealis: None,
            home_widgets: None,
            client_updates: None,
            instance_editor: None,
            instance_creator: None,
            settings: None,
            content: None,
            worlds: None,
            command_palette: None,
            pulsar: None,
            discover: None,
            downloads: None,
            syncing: None,
            accounts_page: None,
            skins: None,
            skin_preview: None,
            pack_import: None,
            instances_search: None,
            instances_sort: 0,
            instances_tag_filter: None,
            organization: Default::default(),
            on_launch_behavior: api::OnLaunchBehavior::KeepOpen,
            cli_launch_instance: None,
            last_pulsar_count: None,
            main_hwnd: None,
            tray_active: false,
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
            sidebar_collapsed: false,
            instances_filter: 0,
            instance_detail: None,
            reset_instance_scroll: false,
            showcase_instance_opened: false,
            reset_discover_scroll: false,
            serial: 2,
            busy: false,
            account_busy: false,
            skin_busy: false,
            client_update_busy: false,
            client_update_instance: None,
            launcher_update_pending: false,
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
            rail_avatar: None,
            rail_avatar_account: None,
            hero_activity: HashMap::new(),
            appearance: None,
            readiness: None,
            processes: HashMap::new(),
            game_showcase_applied: false,
            launching: None,
            pending_stop: None,
            kill_confirmation: None,
            log_windows: HashMap::new(),
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

    #[test]
    fn authentication_blocker_has_a_recovery_status_without_claiming_readiness() {
        let mut state = view();
        state.readiness_in_flight = false;
        state.load_in_flight = false;
        state.readiness = Some(decision(
            "old",
            "idle",
            serde_json::json!([{
                "code": "launch_authentication_required", "message": "Sign in first."
            }]),
        ));
        assert_eq!(
            state.readiness_badge("old").map(|badge| badge.0),
            Some("Sign in needed")
        );
        assert!(!state.can_play("old"));
        assert!(state.readiness_badge("unrelated").is_none());
        // A running process keeps its own status when a saved session expires.
        state.readiness.as_mut().unwrap().process_status = "running".into();
        assert_eq!(
            state.readiness_badge("old").map(|badge| badge.0),
            Some("Running")
        );
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
    fn preparation_stop_updates_the_primary_action_before_cleanup_finishes() {
        let mut state = view();
        select_fixture(&mut state);
        state.launching = Some("old".into());
        state.busy = true;
        state.receive(Message::Event(
            "launch-state".into(),
            serde_json::json!({
                "instanceId": "old", "status": "stopped", "canStop": false,
                "processId": null, "exitCode": null, "message": "Stopped by you"
            }),
        ));
        assert_eq!(state.play_label("old"), "Play");
        assert!(!state.can_play("old"));
        assert!(matches!(
            state.readiness_badge("old"),
            Some(("Stopped", ui::Tone::Neutral))
        ));
    }

    #[test]
    fn play_kill_labels_require_a_supervised_capability() {
        let mut state = view();
        select_fixture(&mut state);
        assert_eq!(state.play_label("old"), "Play");
        state.launching = Some("old".into());
        assert_eq!(state.play_label("old"), "Kill");
        state.launching = None;
        for status in ["starting", "running"] {
            let mut process = child("old", status);
            process.can_stop = true;
            process.process_id = Some(123);
            state.processes.insert("old".into(), process);
            assert_eq!(state.play_label("old"), "Kill");
        }
        state.processes.get_mut("old").unwrap().can_stop = false;
        assert_eq!(state.play_label("old"), "Running");
        state
            .processes
            .insert("old".into(), child("old", "stopped"));
        assert_eq!(state.play_label("old"), "Play");
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
            "clients": {},
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

    #[test]
    fn offers_for_any_ready_instance_keep_the_settings_indicator_active() {
        let mut state = view();
        select_fixture(&mut state);
        let mut other = state.launcher.as_ref().unwrap().instances[0].clone();
        other.id = "other".into();
        other.aurora = Some(
            serde_json::from_value(serde_json::json!({
                "version": "3.5.1", "channel": "stable"
            }))
            .unwrap(),
        );
        state.launcher.as_mut().unwrap().instances.push(other);
        state.set_update_status(serde_json::json!({
            "launcher": {"availability": {"kind": "updateAvailable", "candidate": "2.4.0"}},
            "clients": {"other": {"kind": "updateAvailable", "current": "3.5.1", "candidate": "3.6.0"}},
        }));
        assert!(state.updates_pending());
        assert_eq!(state.client_update_target().unwrap().id, "other");
        state.set_update_status(serde_json::json!({
            "launcher": {"availability": {"kind": "upToDate"}},
            "clients": {"other": {"kind": "updateAvailable", "current": "3.5.1", "candidate": "3.6.0"}},
        }));
        assert!(state.updates_pending());
        state.launcher.as_mut().unwrap().instances[1]
            .aurora
            .as_mut()
            .unwrap()
            .version = "3.6.0".into();
        assert!(
            !state.updates_pending(),
            "an applied offer cannot survive the pin change"
        );
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
