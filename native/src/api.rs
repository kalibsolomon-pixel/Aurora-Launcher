//! The native host's view of the launcher application services.
//!
//! Every operation calls the same `aurora_launcher_lib::application`
//! function the WebView frontend reaches over IPC, with requests built from
//! the same camelCase JSON contract (`src/lib/backend.ts`). Responses are
//! read back through mirror types below, so the native UI depends only on the
//! documented DTO shapes — never on private Rust internals — and cannot skip
//! any validation the application layer performs.

use aurora_launcher_lib::application as app;
use aurora_launcher_lib::host::Host;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A structured operation failure (`CommandError` in Rust, `LauncherBackendError` in TS).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
pub async fn initialize_sync(host: Host) -> ApiResult<()> {
    blocking(move || convert(app::initialize_instance_sync(host))).await?
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPolicy {
    pub schema_version: u32,
    pub enabled: bool,
    pub instances: Vec<String>,
    pub categories: Vec<String>,
    pub exclusions: Vec<String>,
    pub selected_paths: Vec<String>,
    #[serde(default)]
    pub overrides: std::collections::BTreeMap<String, SyncOverride>,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOverride {
    pub disabled: bool,
    pub categories: std::collections::BTreeMap<String, bool>,
    pub exclusions: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflict {
    pub id: String,
    pub source_id: String,
    pub destination_id: String,
    pub category: String,
    pub path: String,
    pub key: Option<String>,
}
pub async fn sync_conflicts(host: Host) -> ApiResult<Vec<SyncConflict>> {
    blocking(move || convert(app::get_instance_sync_conflicts(host))).await?
}
pub async fn sync_resolve(host: Host, id: String, use_source: bool) -> ApiResult<Value> {
    blocking(move || {
        convert(app::resolve_instance_sync_conflict(
            host,
            request(json!({"id":id,"useSource":use_source}))?,
        ))
    })
    .await?
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub message: String,
    pub running: bool,
}
pub async fn sync_save(host: Host, policy: SyncPolicy) -> ApiResult<SyncPolicy> {
    let request = request(serde_json::to_value(policy).map_err(contract)?)?;
    blocking(move || convert(app::save_instance_sync_policy(host, request))).await?
}
pub async fn sync_status() -> ApiResult<SyncStatus> {
    convert(Ok(app::get_instance_sync_status()))
}
pub async fn sync_policy(host: Host) -> ApiResult<SyncPolicy> {
    blocking(move || convert(app::get_instance_sync_policy(host))).await?
}

fn contract(error: serde_json::Error) -> ApiError {
    ApiError::new(
        "native_contract_mismatch",
        format!("Unexpected launcher response: {error}"),
    )
}

fn request<R: DeserializeOwned>(value: Value) -> ApiResult<R> {
    serde_json::from_value(value).map_err(contract)
}

fn convert<T: Serialize, U: DeserializeOwned>(
    result: Result<T, app::CommandError>,
) -> ApiResult<U> {
    match result {
        Ok(value) => serde_json::to_value(value)
            .and_then(serde_json::from_value)
            .map_err(contract),
        Err(error) => Err(serde_json::to_value(error)
            .and_then(serde_json::from_value)
            .unwrap_or_else(|_| {
                ApiError::new("backend_unavailable", "The launcher operation failed.")
            })),
    }
}

/// Runs a synchronous (filesystem-bound) application function off the UI
/// and async worker threads.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> ApiResult<T> {
    tokio::task::spawn_blocking(work).await.map_err(|_| {
        ApiError::new(
            "backend_unavailable",
            "The launcher worker did not complete.",
        )
    })
}

// ---------------------------------------------------------------- mirrors

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationStatus {
    pub launcher_version: String,
    pub managed_data_root: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherState {
    pub config: ConfigSummary,
    pub instances: Vec<InstanceSummary>,
    #[serde(default)]
    pub platform_capabilities: Vec<PlatformCapability>,
    /// Instance id → Material Symbols name, read from the separate icon
    /// store with the state. A damaged store degrades to no icons.
    #[serde(skip)]
    pub icons: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSummary {
    pub selected_instance_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapability {
    pub kind: String,
    pub can_create: bool,
    #[serde(default)]
    pub can_install: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSummary {
    pub id: String,
    pub display_name: String,
    pub state: String,
    pub aurora: Option<AuroraPin>,
    pub platform: Platform,
    #[serde(default)]
    pub pack: Option<PackPin>,
    pub minecraft_version: String,
    pub configuration: InstanceConfiguration,
    #[serde(default)]
    pub recovery: Option<Recovery>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuroraPin {
    pub channel: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    pub kind: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackPin {
    pub name: String,
    pub pack_version: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Recovery {
    pub status: String,
    pub message: String,
}

/// The desired configuration. `loader` and unknown fields round-trip as JSON
/// so an edit never drops configuration the native UI does not render.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance_mode: Option<String>,
    pub minecraft_version: String,
    pub aurora_enabled: bool,
    pub loader: Value,
    pub memory_mib: u32,
    pub additional_jvm_arguments: String,
    pub window: Option<WindowSize>,
    /// Absent: managed Java. `{ "kind": "custom", "path": … }` otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java: Option<Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, Value>,
}

impl InstanceConfiguration {
    /// The selected custom Java path, if any.
    pub fn custom_java(&self) -> Option<&str> {
        self.java
            .as_ref()
            .filter(|java| java["kind"] == "custom")
            .and_then(|java| java["path"].as_str())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl InstanceSummary {
    /// "Fabric 0.17.3", "Vanilla", … as the WebView UI words it.
    pub fn platform_label(&self) -> String {
        let name = match self.platform.kind.as_str() {
            "vanilla" => "Vanilla",
            "fabric" => "Fabric",
            "forge" => "Forge",
            "neoForge" => "NeoForge",
            "quilt" => "Quilt",
            other => other,
        };
        match &self.platform.version {
            Some(version) => format!("{name} {version}"),
            None => name.to_owned(),
        }
    }

    pub fn subtitle(&self) -> String {
        let mut parts = vec![
            format!("Minecraft {}", self.minecraft_version),
            self.platform_label(),
        ];
        if let Some(aurora) = &self.aurora {
            parts.push(format!("Aurora {}", aurora.version));
        }
        if let Some(pack) = &self.pack {
            parts.push(format!("{} {}", pack.name, pack.pack_version));
        }
        parts.join(" · ")
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayReadiness {
    pub ready: bool,
    #[serde(default)]
    pub can_prepare: bool,
    pub instance_id: String,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub instance_status: String,
    pub runtime_status: String,
    pub account_status: String,
    pub process_status: String,
    pub blockers: Vec<PlayBlocker>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PlayBlocker {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProcess {
    pub instance_id: String,
    pub status: String,
    pub process_id: Option<u32>,
    pub exit_code: Option<i32>,
    pub message: Option<String>,
    #[serde(default)]
    pub diagnostic: Option<LaunchDiagnostic>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LaunchDiagnostic {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub status: String,
    pub component: String,
    pub required_major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceValidation {
    pub status: String,
    pub problems: Vec<InstanceProblem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstanceProblem {
    pub component: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountsState {
    pub accounts: Vec<AccountSummary>,
    pub selected_account_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub account_id: String,
    pub minecraft_name: String,
    pub status: String,
}

/// A decoded 8×8 (or larger) head avatar as RGBA bytes.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadAvatar {
    pub rgba: Vec<u8>,
    #[serde(default)]
    pub skin_rgba: Vec<u8>,
    #[serde(default)]
    pub skin_height: u32,
    #[serde(default)]
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedAvatar {
    pub account_id: String,
    pub avatar: HeadAvatar,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub aurora_motion_speed: u8,
    pub background: String,
    pub theme: String,
    pub accent: Value,
    pub palette: AccentPalette,
    pub themes: Vec<ThemeOption>,
    pub accents: Vec<AccentOption>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccentPalette {
    pub accent: String,
    pub accent_strong: String,
    pub accent_hover: String,
    pub accent_pressed: String,
    pub accent_contrast: String,
    pub accent_soft: String,
    pub accent_outline: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeOption {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccentOption {
    pub id: String,
    pub label: String,
    pub hex: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopIntegration {
    pub supported: bool,
    pub manageable: bool,
    pub desktop_shortcut: ShortcutStatus,
    pub start_menu_shortcut: ShortcutStatus,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub state: String,
    pub managed_by: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOverview {
    pub launcher: LauncherUpdateStatus,
    pub client: Value,
    pub client_instance_id: Option<String>,
    pub client_instance_name: Option<String>,
    pub client_installed_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdateStatus {
    pub installed_version: String,
    pub availability: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersion {
    pub id: String,
    pub version_type: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuroraCompatibility {
    pub available: bool,
    pub reason: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAdvice {
    pub recommended_mib: Option<u32>,
    pub explanation: String,
    pub facts: HostFacts,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostFacts {
    pub total_mib: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInventory {
    pub entries: Vec<ModEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    #[serde(default)]
    pub sha256: Option<String>,
    pub entry_id: String,
    pub file_name: String,
    pub display_name: String,
    pub enabled: bool,
    pub ownership: String,
    #[serde(default)]
    pub metadata: Option<ModMetadata>,
    #[serde(default)]
    pub warnings: Vec<Warning>,
    pub can_toggle: bool,
    pub can_remove: bool,
    #[serde(default)]
    pub removal_blocked_reason: Option<String>,
    #[serde(default)]
    pub action_blocked_reason: Option<String>,
    #[serde(default)]
    pub provenance: Option<Provenance>,
}

/// The managed provider identity of an installed file, when it has one.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub provider: String,
    pub project_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModMetadata {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Warning {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentInventory {
    pub entries: Vec<ContentEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentEntry {
    #[serde(default)]
    pub pack_owned: bool,
    pub entry_id: String,
    pub file_name: String,
    pub display_name: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub warnings: Vec<Warning>,
    pub management: ContentManagement,
    #[serde(default)]
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentManagement {
    pub can_remove: bool,
    pub can_toggle: bool,
    pub active: Option<bool>,
    pub activation_managed_in_game: bool,
    #[serde(default)]
    pub removal_blocked_reason: Option<String>,
    #[serde(default)]
    pub toggle_blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub offset: u32,
    pub total_hits: u32,
    pub hits: Vec<ProjectSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub project_id: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetails {
    pub project_id: String,
    pub title: String,
    pub summary: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub versions: Vec<VersionChoice>,
    pub default_version_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionChoice {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub loaders: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cosmetics {
    pub current_skin_model: Option<String>,
    pub has_current_skin: bool,
    pub current_skin: Option<HeadAvatar>,
    pub capes: Vec<Cape>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Cape {
    pub id: String,
    pub name: String,
    pub selected: bool,
    pub preview: Option<Texture>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkinPreset {
    pub id: String,
    pub name: String,
    pub model: String,
    #[serde(default)]
    pub favorite: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Thumbnail {
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUpdatePreview {
    pub installed_version: String,
    pub outcome: String,
    pub candidate: Option<ClientUpdateCandidate>,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientUpdateCandidate {
    pub version: String,
    pub notes: Option<String>,
    pub minecraft_version: String,
}

// ------------------------------------------------------------- operations

pub fn application_status(host: &Host) -> ApiResult<ApplicationStatus> {
    convert(app::get_application_status(host.clone()))
}

pub async fn launcher_state(host: Host) -> ApiResult<LauncherState> {
    let mut state: LauncherState = convert(app::get_launcher_state(host.clone()).await)?;
    state.icons = blocking(move || app::get_instance_icons(host).unwrap_or_default())
        .await
        .unwrap_or_default();
    Ok(state)
}

/// Sets (or clears, with `None`) one instance's Material Symbols icon.
pub async fn set_instance_icon(
    host: Host,
    instance_id: String,
    icon: Option<String>,
) -> ApiResult<std::collections::BTreeMap<String, String>> {
    let request = request(json!({ "instanceId": instance_id, "icon": icon }))?;
    blocking(move || convert(app::set_instance_icon(host, request))).await?
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WidgetGrid {
    pub columns: u8,
    pub rows: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[u8; 2]>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WidgetPlacement {
    pub id: String,
    pub enabled: bool,
    pub size: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<WidgetGrid>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HomeLayout {
    pub widgets: Vec<WidgetPlacement>,
}

pub async fn home_widgets(host: Host) -> ApiResult<HomeLayout> {
    blocking(move || convert(app::get_home_widgets(host))).await?
}

pub async fn set_home_widgets(host: Host, layout: HomeLayout) -> ApiResult<HomeLayout> {
    let request = request(serde_json::to_value(layout).map_err(contract)?)?;
    blocking(move || convert(app::set_home_widgets(host, request))).await?
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playtime {
    pub today_ms: u64,
    pub last_7_days_ms: u64,
    pub last_30_days_ms: u64,
    pub all_time_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPlaytime {
    pub day: u64,
    pub duration_ms: u64,
}

pub async fn daily_playtime(
    host: Host,
    instance_id: Option<String>,
) -> ApiResult<Vec<DailyPlaytime>> {
    blocking(move || convert(app::get_daily_playtime(host, instance_id, 30))).await?
}

pub async fn playtime(host: Host, instance_id: Option<String>) -> ApiResult<Playtime> {
    blocking(move || convert(app::get_playtime_summary(host, instance_id))).await?
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTarget {
    pub instance_id: String,
    pub display_name: String,
    pub last_played_at: u64,
    pub duration_ms: u64,
    pub available: bool,
    pub favorite: bool,
}

pub async fn recent_servers(
    host: Host,
    instance_id: Option<String>,
    limit: usize,
) -> ApiResult<Vec<RecentTarget>> {
    blocking(move || convert(app::get_recent_servers(host, instance_id, limit))).await?
}

pub async fn recent_worlds(
    host: Host,
    instance_id: Option<String>,
    limit: usize,
) -> ApiResult<Vec<RecentTarget>> {
    blocking(move || convert(app::get_recent_worlds(host, instance_id, limit))).await?
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscordPreferences {
    pub enabled: bool,
    pub instance_name: bool,
    pub minecraft_version: bool,
    pub platform: bool,
    pub aurora_active: bool,
    pub elapsed_time: bool,
    pub world: bool,
    pub server: bool,
    pub server_address: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordState {
    pub connection: String,
    pub configured: bool,
    pub preferences: DiscordPreferences,
}

pub fn discord_state() -> ApiResult<DiscordState> {
    convert::<_, DiscordState>(Ok::<_, app::CommandError>(app::get_discord_state()))
}

pub async fn set_discord_preferences(
    host: Host,
    preferences: DiscordPreferences,
) -> ApiResult<DiscordState> {
    let request = request(serde_json::to_value(preferences).map_err(contract)?)?;
    blocking(move || convert(app::set_discord_preferences(host, request))).await?
}

pub async fn connect_discord() -> ApiResult<DiscordState> {
    convert::<_, DiscordState>(Ok::<_, app::CommandError>(app::connect_discord().await))
}

pub async fn select_instance(host: Host, instance_id: String) -> ApiResult<()> {
    let request = request(json!({ "instanceId": instance_id }))?;
    blocking(move || convert::<_, ()>(app::select_instance(host, request))).await?
}

pub async fn play_readiness(
    host: Host,
    instance_id: String,
    account_id: Option<String>,
) -> ApiResult<PlayReadiness> {
    let request = request(json!({ "instanceId": instance_id, "accountId": account_id }))?;
    convert(app::get_play_readiness(host, request).await)
}

pub async fn launch_state(instance_id: String) -> ApiResult<LaunchProcess> {
    let request = request(json!({ "instanceId": instance_id }))?;
    blocking(move || convert(app::get_launch_state(request))).await?
}

pub async fn play(host: Host, instance_id: String, account_id: String) -> ApiResult<LaunchProcess> {
    let request = request(json!({ "instanceId": instance_id, "accountId": account_id }))?;
    convert(app::play_instance(host, request).await)
}

pub async fn runtime_status(host: Host, instance_id: String) -> ApiResult<RuntimeStatus> {
    let request = request(json!({ "instanceId": instance_id }))?;
    convert(app::get_instance_runtime_status(host, request).await)
}

pub async fn ensure_runtime(host: Host, instance_id: String) -> ApiResult<RuntimeStatus> {
    let request = request(json!({ "instanceId": instance_id }))?;
    convert(app::ensure_instance_runtime(host, request).await)
}

pub async fn validate_instance(host: Host, instance_id: String) -> ApiResult<InstanceValidation> {
    let request = request(json!({ "instanceId": instance_id }))?;
    blocking(move || convert(app::validate_instance(host, request))).await?
}

pub async fn retry_install(host: Host, instance_id: String) -> ApiResult<InstanceSummary> {
    let request = request(json!({ "instanceId": instance_id }))?;
    convert(app::retry_instance_install(host, request).await)
}

pub async fn install_configuration(host: Host, instance_id: String) -> ApiResult<InstanceSummary> {
    let request = request(json!({ "instanceId": instance_id }))?;
    convert(app::install_instance_configuration(host, request).await)
}

pub async fn rename_instance(
    host: Host,
    instance_id: String,
    expected: String,
    name: String,
) -> ApiResult<InstanceSummary> {
    let request = request(
        json!({ "instanceId": instance_id, "newDisplayName": name, "expectedDisplayName": expected }),
    )?;
    blocking(move || convert(app::rename_instance(host, request))).await?
}

pub async fn update_configuration(
    host: Host,
    instance_id: String,
    expected: InstanceConfiguration,
    configuration: InstanceConfiguration,
) -> ApiResult<InstanceSummary> {
    let request = request(json!({
        "instanceId": instance_id,
        "configuration": configuration,
        "expectedConfiguration": expected,
    }))?;
    blocking(move || convert(app::update_instance_configuration(host, request))).await?
}

pub async fn performance_advice(host: Host, instance_id: String) -> ApiResult<MemoryAdvice> {
    let request = request(json!({ "instanceId": instance_id, "mode": Value::Null }))?;
    blocking(move || convert(app::get_instance_performance_advice(host, request))).await?
}

pub async fn delete_instance(
    host: Host,
    instance_id: String,
    confirmed_name: String,
) -> ApiResult<()> {
    let request = request(json!({ "instanceId": instance_id, "confirmedName": confirmed_name }))?;
    convert::<_, ()>(app::delete_instance(host, request).await)
}

pub async fn open_instance_folder(host: Host, instance_id: String) -> ApiResult<()> {
    let request = request(json!({ "instanceId": instance_id }))?;
    blocking(move || convert::<_, ()>(app::open_instance_folder(host, request))).await?
}

pub async fn create_instance(
    host: Host,
    name: String,
    minecraft: String,
    loader: Value,
    aurora: bool,
) -> ApiResult<InstanceSummary> {
    let request = request(json!({
        "displayName": name,
        "minecraftVersion": minecraft,
        "loader": loader,
        "auroraEnabled": aurora,
    }))?;
    convert(app::create_instance(host, request).await)
}

pub async fn aurora_compatibility(
    host: Host,
    minecraft: String,
    loader: Value,
) -> ApiResult<AuroraCompatibility> {
    let request = request(json!({ "minecraftVersion": minecraft, "loader": loader }))?;
    convert(app::get_aurora_compatibility(host, request).await)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuroraRelease {
    pub minecraft_version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuroraTransition {
    pub instance_id: String,
    pub target: AuroraTarget,
    pub requirements_added: Vec<String>,
    pub requirements_removed: Vec<String>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
    pub fingerprint: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct AuroraTarget {
    pub aurora: Option<AuroraPin>,
}
pub async fn preview_aurora(
    host: Host,
    instance: String,
    enabled: bool,
) -> ApiResult<AuroraTransition> {
    let request = request(json!({"instanceId": instance, "enabled": enabled}))?;
    convert(app::preview_aurora_transition(host, request).await)
}
pub async fn apply_aurora(
    host: Host,
    instance: String,
    enabled: bool,
    fingerprint: String,
) -> ApiResult<InstanceSummary> {
    let request =
        request(json!({"instanceId": instance, "enabled": enabled, "fingerprint": fingerprint}))?;
    convert(app::apply_aurora_transition(host, request).await)
}

pub async fn aurora_releases(host: Host) -> ApiResult<Vec<AuroraRelease>> {
    convert(app::list_aurora_releases(host).await)
}

pub async fn minecraft_versions(include_snapshots: bool) -> ApiResult<Vec<MinecraftVersion>> {
    convert(app::list_minecraft_versions(include_snapshots).await)
}

pub async fn instance_mods(host: Host, instance_id: String) -> ApiResult<ModInventory> {
    let request = request(json!({ "instanceId": instance_id }))?;
    convert(app::get_instance_mods(host, request).await)
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModIconRequest {
    pub file_name: String,
    pub sha256: String,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModIconBitmap {
    pub width: u32,
    pub height: u32,
    pub rgba_base64: String,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModIconResult {
    pub sha256: String,
    pub bitmap: Option<ModIconBitmap>,
}
pub async fn mod_icons(
    host: Host,
    id: String,
    files: Vec<ModIconRequest>,
) -> ApiResult<Vec<ModIconResult>> {
    let request = request(json!({"instanceId": id, "files": files}))?;
    convert(app::get_installed_mod_icons(host, request).await)
}

pub async fn set_mod_enabled(
    host: Host,
    instance_id: String,
    entry_id: String,
    enabled: bool,
) -> ApiResult<ModInventory> {
    let request =
        request(json!({ "instanceId": instance_id, "entryId": entry_id, "enabled": enabled }))?;
    convert(app::set_instance_mod_enabled(host, request).await)
}

pub async fn remove_mod(
    host: Host,
    instance_id: String,
    entry_id: String,
) -> ApiResult<ModInventory> {
    let request = request(json!({ "instanceId": instance_id, "entryId": entry_id }))?;
    convert(app::remove_instance_mod(host, request).await)
}

pub async fn open_mods_folder(host: Host, instance_id: String) -> ApiResult<()> {
    let request = request(json!({ "instanceId": instance_id }))?;
    blocking(move || convert::<_, ()>(app::open_instance_mods_folder(host, request))).await?
}

pub async fn instance_content(
    host: Host,
    instance_id: String,
    kind: &'static str,
) -> ApiResult<ContentInventory> {
    let request = request(json!({ "instanceId": instance_id, "contentType": kind }))?;
    convert(app::get_instance_content(host, request).await)
}

pub async fn set_pack_enabled(
    host: Host,
    instance_id: String,
    kind: &'static str,
    entry_id: String,
    enabled: bool,
) -> ApiResult<ContentInventory> {
    let request = request(
        json!({ "instanceId": instance_id, "contentType": kind, "entryId": entry_id, "enabled": enabled }),
    )?;
    convert(app::set_instance_pack_enabled(host, request).await)
}

pub async fn remove_content(
    host: Host,
    instance_id: String,
    kind: &'static str,
    entry_id: String,
) -> ApiResult<ContentInventory> {
    let request =
        request(json!({ "instanceId": instance_id, "contentType": kind, "entryId": entry_id }))?;
    convert(app::remove_instance_content(host, request).await)
}

pub async fn open_content_folder(
    host: Host,
    instance_id: String,
    kind: &'static str,
) -> ApiResult<()> {
    let request = request(json!({ "instanceId": instance_id, "contentType": kind }))?;
    blocking(move || convert::<_, ()>(app::open_instance_content_folder(host, request))).await?
}

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCategory {
    pub name: String,
    pub project_type: String,
}
pub async fn browse_categories() -> ApiResult<Vec<ProviderCategory>> {
    let request = request(json!({"provider": "modrinth"}))?;
    convert(app::browse_modrinth_tags(request).await)
}

#[allow(dead_code)]
pub async fn browse(
    host: Host,
    instance_id: String,
    kind: &'static str,
    search: String,
    offset: u32,
) -> ApiResult<SearchPage> {
    let sort = if search.trim().is_empty() {
        "downloads"
    } else {
        "relevance"
    }
    .to_owned();
    browse_filtered(host, instance_id, kind, search, offset, Vec::new(), sort).await
}

pub async fn browse_filtered(
    host: Host,
    instance_id: String,
    kind: &'static str,
    search: String,
    offset: u32,
    categories: Vec<String>,
    sort: String,
) -> ApiResult<SearchPage> {
    let request = request(json!({
        "instanceId": instance_id,
        "provider": "modrinth",
        "contentType": kind,
        "search": search,
        "categories": categories,
        "sort": sort,
        "offset": offset,
    }))?;
    convert(app::browse_modrinth(host, request).await)
}

pub async fn quick_install(
    host: Host,
    instance_id: String,
    kind: &'static str,
    project_id: String,
) -> ApiResult<Value> {
    let request = request(
        json!({ "instanceId": instance_id, "contentType": kind, "projectId": project_id }),
    )?;
    convert(app::quick_install_modrinth(host, request).await)
}

pub async fn project(
    host: Host,
    instance: String,
    kind: &'static str,
    project: String,
) -> ApiResult<ProjectDetails> {
    let request =
        request(json!({"instanceId": instance, "contentType": kind, "projectId": project}))?;
    convert(app::get_modrinth_project(host, request).await)
}

/// Only normalized, bounded PNGs from the core's isolated artwork decoder.
pub async fn project_artwork(host: Host, project: String) -> ApiResult<Option<Vec<u8>>> {
    use base64::Engine;
    let request = request(json!({"projectId": project}))?;
    let value: Value = convert(app::resolve_project_artwork(host, request).await)?;
    Ok(value["source"]
        .as_str()
        .and_then(|s| s.strip_prefix("data:image/png;base64,"))
        .filter(|s| s.len() <= 2 * 1024 * 1024)
        .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok()))
}

pub async fn provider_inventory(host: Host, instance: String) -> ApiResult<Value> {
    let request = request(json!({"instanceId": instance}))?;
    blocking(move || convert(app::get_provider_lifecycle(host, request))).await?
}

pub async fn discard_local_content_review(
    host: Host,
    instance: String,
    ticket: String,
) -> ApiResult<()> {
    let request = request(json!({"instanceId":instance,"ticket":ticket}))?;
    blocking(move || convert(app::discard_local_content_review(host, request))).await?
}

pub async fn preview_local_content(host: Host, instance: String, path: String) -> ApiResult<Value> {
    convert(
        app::preview_local_content(
            host,
            request(json!({"instanceId":instance,"sourcePath":path}))?,
        )
        .await,
    )
}
pub async fn install_local_content(
    host: Host,
    instance: String,
    ticket: String,
    fingerprint: String,
) -> ApiResult<Value> {
    convert(
        app::install_local_content(
            host,
            request(json!({"instanceId":instance,"ticket":ticket,"fingerprint":fingerprint}))?,
        )
        .await,
    )
}

pub async fn preview_content(
    host: Host,
    instance: String,
    kind: &'static str,
    project: String,
    version: String,
) -> ApiResult<Value> {
    let request = request(
        json!({"instanceId": instance, "contentType": kind, "projectId": project, "versionId": version}),
    )?;
    convert(app::preview_modrinth_install(host, request).await)
}

pub async fn install_content(
    host: Host,
    instance: String,
    kind: &'static str,
    project: String,
    version: String,
    fingerprint: String,
) -> ApiResult<Value> {
    let request = request(
        json!({"instanceId": instance, "contentType": kind, "projectId": project, "versionId": version, "previewFingerprint": fingerprint}),
    )?;
    convert(app::install_modrinth(host, request).await)
}

/// The same reviewed pack engines used by the WebView host. The frozen
/// request is produced by preview; commit only adds its returned fingerprint.
pub async fn preview_pack(host: Host, merge: bool, value: Value) -> ApiResult<Value> {
    if merge {
        convert(app::preview_modrinth_pack_merge(host, request(value)?).await)
    } else {
        convert(app::preview_modrinth_pack(host, request(value)?).await)
    }
}

pub async fn install_pack(host: Host, merge: bool, value: Value) -> ApiResult<Value> {
    if merge {
        convert(Box::pin(app::install_modrinth_pack_merge(host, request(value)?)).await)
    } else {
        convert(Box::pin(app::install_modrinth_pack(host, request(value)?)).await)
    }
}

// ------------------------------------------------- loader versions

/// One installable loader version for an exact Minecraft version.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

/// The official catalog for one platform, newest first. Vanilla has none.
pub async fn loader_versions(kind: String, minecraft: String) -> ApiResult<Vec<LoaderVersion>> {
    match kind.as_str() {
        "fabric" => convert(app::list_fabric_loader_versions(minecraft).await),
        "forge" => convert(app::list_forge_versions(minecraft).await),
        "neoForge" => convert(app::list_neoforge_versions(minecraft).await),
        _ => Ok(Vec::new()),
    }
}

// ------------------------------------------------- content updates

/// The candidate version an update would install.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCandidate {
    pub id: String,
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
}

/// One managed record's update availability (`content_updates.rs`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEntry {
    pub content_type: String,
    pub project_id: String,
    pub current_version: String,
    pub status: String,
    #[serde(default)]
    pub block: Option<String>,
    #[serde(default)]
    pub candidate: Option<UpdateCandidate>,
    pub pinned: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesReport {
    pub entries: Vec<UpdateEntry>,
    #[serde(default)]
    pub dependency_managed: usize,
}

/// Explicit, read-only availability check for one content kind.
pub async fn check_updates(
    host: Host,
    instance: String,
    kind: &'static str,
) -> ApiResult<UpdatesReport> {
    let request = request(json!({ "instanceId": instance, "contentType": kind }))?;
    convert(app::check_instance_updates(host, request).await)
}

fn update_targets(kind: &str, projects: &[String]) -> Value {
    Value::Array(
        projects
            .iter()
            .map(|project| json!({ "contentType": kind, "projectId": project }))
            .collect(),
    )
}

/// One coherent reviewed preview for one or more roots of one kind.
pub async fn preview_updates(
    host: Host,
    instance: String,
    kind: &'static str,
    projects: Vec<String>,
) -> ApiResult<Value> {
    let request =
        request(json!({ "instanceId": instance, "targets": update_targets(kind, &projects) }))?;
    convert(app::preview_modrinth_bulk_update(host, request).await)
}

/// Re-resolves the same targets and applies them only if the preview is unchanged.
pub async fn apply_updates(
    host: Host,
    instance: String,
    kind: &'static str,
    projects: Vec<String>,
    fingerprint: String,
) -> ApiResult<Value> {
    let request = request(json!({
        "instanceId": instance,
        "targets": update_targets(kind, &projects),
        "previewFingerprint": fingerprint,
    }))?;
    convert(
        app::apply_modrinth_bulk_update(host, request)
            .await
            .map(|()| Value::Null),
    )
}

/// Changes only the persisted pin of one managed record.
pub async fn set_update_pinned(
    host: Host,
    instance: String,
    kind: &'static str,
    project: String,
    pinned: bool,
) -> ApiResult<Value> {
    let request = request(json!({
        "instanceId": instance, "contentType": kind, "projectId": project, "pinned": pinned, "channel": null,
    }))?;
    blocking(move || convert(app::set_provider_update_policy(host, request))).await?
}

// ------------------------------------------------- managed modpack updates

pub async fn check_modpack_update(host: Host, instance: String) -> ApiResult<Value> {
    let request = request(json!({ "instanceId": instance }))?;
    convert(app::check_modpack_update(host, request).await)
}

pub async fn preview_modpack_update(host: Host, instance: String) -> ApiResult<Value> {
    let request = request(json!({ "instanceId": instance }))?;
    convert(app::preview_modpack_update(host, request).await)
}

/// `resolutions` are (path, resolution) pairs re-validated by Rust.
pub async fn apply_modpack_update(
    host: Host,
    instance: String,
    fingerprint: String,
    resolutions: Vec<(String, String)>,
) -> ApiResult<Value> {
    let resolutions: Vec<Value> = resolutions
        .into_iter()
        .map(|(path, resolution)| json!({ "path": path, "resolution": resolution }))
        .collect();
    let request = request(
        json!({ "instanceId": instance, "fingerprint": fingerprint, "resolutions": resolutions }),
    )?;
    convert(app::apply_modpack_update(host, request).await)
}

// ------------------------------------------------- custom Java

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JavaFacts {
    pub major_version: u32,
    pub version: String,
    pub vendor: String,
    pub architecture: String,
}

/// The executed check of one candidate custom Java (`runtime::external`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JavaReport {
    pub path: String,
    pub required_major_version: u32,
    pub valid: bool,
    #[serde(default)]
    pub facts: Option<JavaFacts>,
    #[serde(default)]
    pub problem: Option<String>,
}

/// Executes and checks a candidate without saving anything.
pub async fn check_custom_java(
    host: Host,
    instance: String,
    path: String,
) -> ApiResult<JavaReport> {
    let request = request(json!({ "instanceId": instance, "path": path }))?;
    convert(app::check_custom_java(host, request).await)
}

// ------------------------------------------------- local .mrpack

/// Identifies a selected local archive as an exact published pack version.
pub async fn identify_local_modpack(host: Host, path: String) -> ApiResult<Value> {
    let request = request(json!({ "sourcePath": path }))?;
    convert(app::identify_local_modpack(host, request).await)
}

pub async fn skin_preview(host: Host, preset: String) -> ApiResult<HeadAvatar> {
    let request = request(json!({"presetId": preset}))?;
    blocking(move || convert(app::skin_preset_preview(host, request))).await?
}

pub async fn import_skin(
    host: Host,
    name: String,
    model: String,
    bytes: Vec<u8>,
) -> ApiResult<Value> {
    let request = request(json!({"name":name,"model":model,"bytes":bytes}))?;
    blocking(move || convert(app::import_skin_preset(host, request))).await?
}

pub async fn accounts(host: Host) -> ApiResult<AccountsState> {
    blocking(move || convert(app::get_accounts(host))).await?
}

pub async fn cached_avatars(host: Host) -> ApiResult<Vec<CachedAvatar>> {
    blocking(move || convert(app::get_cached_account_avatars(host))).await?
}

pub async fn account_avatar(host: Host, account_id: String) -> ApiResult<Option<HeadAvatar>> {
    let request = request(json!({ "accountId": account_id, "refresh": false }))?;
    convert(app::get_account_avatar(host, request).await)
}

pub async fn begin_login(host: Host) -> ApiResult<AccountSummary> {
    convert(app::begin_microsoft_login(host).await)
}

pub async fn cancel_login() -> ApiResult<()> {
    blocking(|| convert::<_, ()>(app::cancel_microsoft_login())).await?
}

pub async fn select_account(host: Host, account_id: String) -> ApiResult<()> {
    let request = request(json!({ "accountId": account_id }))?;
    blocking(move || convert::<_, ()>(app::select_account(host, request))).await?
}

pub async fn remove_account(host: Host, account_id: String) -> ApiResult<()> {
    let request = request(json!({ "accountId": account_id }))?;
    convert(app::remove_account(host, request).await)
}

pub async fn refresh_session(host: Host, account_id: String) -> ApiResult<Value> {
    let request = request(json!({ "accountId": account_id }))?;
    convert(app::refresh_account_session(host, request).await)
}

pub async fn appearance(host: Host) -> ApiResult<Appearance> {
    blocking(move || convert(app::get_appearance(host))).await?
}

pub async fn set_appearance(
    host: Host,
    theme: String,
    accent: Value,
    background: String,
    motion: u8,
) -> ApiResult<Appearance> {
    let request = request(json!({
        "theme": theme,
        "accent": accent,
        "background": background,
        "auroraMotionSpeed": motion,
    }))?;
    blocking(move || convert(app::set_appearance(host, request))).await?
}

pub async fn desktop_integration() -> ApiResult<DesktopIntegration> {
    blocking(|| convert(app::get_desktop_integration())).await?
}

pub async fn create_desktop_shortcut() -> ApiResult<DesktopIntegration> {
    blocking(|| convert(app::create_desktop_shortcut())).await?
}

pub async fn remove_desktop_shortcut() -> ApiResult<DesktopIntegration> {
    blocking(|| convert(app::remove_desktop_shortcut())).await?
}

pub async fn create_start_menu_shortcut() -> ApiResult<DesktopIntegration> {
    blocking(|| convert(app::create_start_menu_shortcut())).await?
}

pub async fn open_launcher_folder(host: Host) -> ApiResult<()> {
    blocking(move || convert::<_, ()>(app::open_launcher_folder(host))).await?
}

pub async fn startup_update_check(host: Host) -> ApiResult<UpdateOverview> {
    convert(app::startup_update_check(host).await)
}

pub async fn check_for_updates(host: Host) -> ApiResult<UpdateOverview> {
    convert(app::check_for_updates(host).await)
}

pub async fn download_launcher_update(host: Host, version: String) -> ApiResult<Value> {
    convert(app::launcher_update_download(host, request(json!({"version":version}))?).await)
}

pub async fn install_launcher_update(host: Host, version: String) -> ApiResult<Value> {
    convert(app::launcher_update_install(host, request(json!({"version":version}))?).await)
}

pub async fn cosmetics(host: Host, account_id: String) -> ApiResult<Cosmetics> {
    let request = request(json!({ "accountId": account_id }))?;
    convert(app::get_cosmetics(host, request).await)
}

pub async fn skin_presets(host: Host) -> ApiResult<Vec<SkinPreset>> {
    blocking(move || convert(app::list_skin_presets(host))).await?
}

pub async fn skin_thumbnail(host: Host, preset_id: String) -> ApiResult<Option<Thumbnail>> {
    let request = request(json!({ "presetId": preset_id }))?;
    blocking(move || convert(app::skin_preset_thumbnail(host, request))).await?
}

pub async fn save_current_skin(host: Host, account_id: String) -> ApiResult<Value> {
    let request = request(json!({ "accountId": account_id }))?;
    convert(app::save_current_skin(host, request).await)
}

pub async fn rename_skin_preset(
    host: Host,
    preset_id: String,
    name: String,
) -> ApiResult<Vec<SkinPreset>> {
    let request = request(json!({"presetId": preset_id, "changes": {"name": name}}))?;
    blocking(move || convert(app::update_skin_preset(host, request))).await?
}

pub async fn remove_skin_preset(host: Host, preset_id: String) -> ApiResult<()> {
    let request = request(json!({ "presetId": preset_id }))?;
    blocking(move || convert::<_, ()>(app::remove_skin_preset(host, request))).await?
}

pub async fn apply_skin_preset(
    host: Host,
    account_id: String,
    preset_id: String,
    model: String,
) -> ApiResult<Cosmetics> {
    let request =
        request(json!({ "accountId": account_id, "presetId": preset_id, "model": model }))?;
    convert(app::apply_skin_preset(host, request).await)
}

pub async fn select_cape(host: Host, account_id: String, cape_id: String) -> ApiResult<Cosmetics> {
    let request = request(json!({ "accountId": account_id, "capeId": cape_id }))?;
    convert(app::select_cape(host, request).await)
}

pub async fn disable_cape(host: Host, account_id: String) -> ApiResult<Cosmetics> {
    let request = request(json!({ "accountId": account_id }))?;
    convert(app::disable_cape(host, request).await)
}

pub async fn preview_client_update(
    host: Host,
    instance_id: String,
) -> ApiResult<ClientUpdatePreview> {
    let request = request(json!({ "instanceId": instance_id }))?;
    convert(app::preview_client_update(host, request).await)
}

pub async fn apply_client_update(
    host: Host,
    instance_id: String,
    fingerprint: String,
) -> ApiResult<InstanceSummary> {
    let request = request(json!({ "instanceId": instance_id, "fingerprint": fingerprint }))?;
    convert(app::apply_client_update(host, request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_skin_pixels_and_model_survive_the_camel_case_contract() {
        let avatar: HeadAvatar = serde_json::from_value(
            json!({"rgba":[1,2,3,255],"skinRgba":[9,8,7,255],"skinHeight":64,"model":"slim"}),
        )
        .unwrap();
        assert_eq!(avatar.skin_rgba, vec![9, 8, 7, 255]);
        assert_eq!(avatar.skin_height, 64);
        assert_eq!(avatar.model, "slim");
    }

    #[test]
    fn configuration_round_trips_unknown_fields() {
        let original = json!({
            "performanceMode": "balanced",
            "java": { "kind": "custom", "path": "C:\\Java with spaces 日本語\\bin\\java.exe" },
            "minecraftVersion": "1.21.10",
            "auroraEnabled": true,
            "loader": { "kind": "fabric", "policy": { "type": "automatic" } },
            "memoryMib": 4096,
            "additionalJvmArguments": "",
            "window": null,
            "futureField": { "kept": true }
        });
        let parsed: InstanceConfiguration = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(serde_json::to_value(&parsed).unwrap(), original);
    }

    #[test]
    fn command_errors_keep_code_and_message() {
        let error: ApiError =
            serde_json::from_value(json!({ "code": "instance_not_found", "message": "gone" }))
                .unwrap();
        assert_eq!(error.code, "instance_not_found");
        assert_eq!(error.message, "gone");
    }

    #[test]
    fn subtitle_matches_launcher_wording() {
        let instance: InstanceSummary = serde_json::from_value(json!({
            "id": "a", "displayName": "Aurora", "state": "ready",
            "aurora": { "channel": "stable", "version": "3.0.0" },
            "platform": { "kind": "fabric", "version": "0.17.3" },
            "minecraftVersion": "1.21.10",
            "configuration": {
                "minecraftVersion": "1.21.10", "auroraEnabled": true,
                "loader": { "kind": "fabric", "policy": { "type": "automatic" } },
                "memoryMib": 4096, "additionalJvmArguments": "", "window": null
            }
        }))
        .unwrap();
        assert_eq!(
            instance.subtitle(),
            "Minecraft 1.21.10 · Fabric 0.17.3 · Aurora 3.0.0"
        );
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPreview {
    pub instance_id: String,
    pub summary: String,
    pub kept_settings: Vec<String>,
    pub removed_mods: Vec<String>,
    pub blockers: Vec<String>,
    pub can_apply: bool,
    pub fingerprint: String,
}
pub async fn inspect_recovery(host: Host, id: String) -> ApiResult<RecoveryPreview> {
    let request = request(json!({"instanceId":id}))?;
    convert(app::inspect_instance_recovery(host, request).await)
}
pub async fn apply_recovery(
    host: Host,
    id: String,
    fingerprint: String,
    keep_removed_mods: bool,
) -> ApiResult<RecoveryPreview> {
    let request = request(
        json!({"instanceId":id,"fingerprint":fingerprint,"keepRemovedMods":keep_removed_mods}),
    )?;
    convert(app::apply_instance_recovery(host, request).await)
}
