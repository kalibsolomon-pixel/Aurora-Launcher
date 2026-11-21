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
    /// The instance a failed or cancelled new-pack install created and kept
    /// (still installing); `resume_pack` finishes it. Absent otherwise.
    #[serde(default, rename = "instanceId")]
    pub instance_id: Option<String>,
}

impl ApiError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            instance_id: None,
        }
    }
}

impl From<app::CommandError> for ApiError {
    fn from(error: app::CommandError) -> Self {
        convert::<(), ()>(Err(error)).unwrap_err()
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

/// The main-instance synced group. Members are seeded from the main when they
/// join; afterwards settings edits from any member reach the others.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncGroupConfig {
    pub schema_version: u32,
    pub enabled: bool,
    pub main_instance_id: Option<String>,
    pub members: Vec<String>,
    pub new_instances_join: bool,
    pub vanilla: bool,
    pub aurora: bool,
    pub packs: bool,
    #[serde(default)]
    pub pending_seed: Vec<String>,
    #[serde(default)]
    pub known: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncGroupMember {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub member: bool,
    pub main: bool,
    pub pending_seed: bool,
    pub running: bool,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncGroupHistory {
    pub id: String,
    pub instance_id: String,
    pub at: u64,
    pub kind: String,
    pub keys: Vec<String>,
    pub files: Vec<String>,
    pub packs: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncGroupOverview {
    pub config: SyncGroupConfig,
    pub instances: Vec<SyncGroupMember>,
    pub history: Vec<SyncGroupHistory>,
    pub suggested_main: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncGroupReport {
    pub applied: Vec<String>,
    pub deferred: Vec<String>,
    pub changed_settings: usize,
    pub copied_packs: usize,
    pub message: String,
}
pub async fn sync_group(host: Host) -> ApiResult<SyncGroupOverview> {
    blocking(move || convert(app::get_sync_group(host))).await?
}
pub async fn sync_group_save(host: Host, config: SyncGroupConfig) -> ApiResult<SyncGroupOverview> {
    let request = request(serde_json::to_value(config).map_err(contract)?)?;
    blocking(move || convert(app::save_sync_group(host, request))).await?
}
pub async fn sync_group_now(host: Host) -> ApiResult<SyncGroupReport> {
    blocking(move || convert(app::sync_group_now(host))).await?
}
pub async fn sync_group_restore(host: Host, id: String) -> ApiResult<SyncGroupOverview> {
    blocking(move || convert(app::restore_sync_group(host, request(json!({ "id": id }))?))).await?
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub name: String,
    pub bytes: u64,
    pub modified_ns: u128,
}
#[derive(Clone, Deserialize)]
pub struct ScreenshotBitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub async fn screenshots(host: Host, instance_id: String) -> ApiResult<Vec<Screenshot>> {
    blocking(move || {
        convert(app::get_screenshots(
            host,
            request(json!({"instanceId":instance_id}))?,
        ))
    })
    .await?
}
pub async fn screenshot_image(
    host: Host,
    instance_id: String,
    screenshot: Screenshot,
    preview: bool,
) -> ApiResult<ScreenshotBitmap> {
    blocking(move || {
        match app::get_screenshot_image(
            host,
            request(json!({"instanceId":instance_id,"screenshot":screenshot,"preview":preview}))?,
        ) {
            // Pixel buffers do not go through a JSON Value per channel. Preserve
            // the typed boundary without millions of temporary numeric values.
            Ok(bitmap) => Ok(ScreenshotBitmap {
                width: bitmap.width,
                height: bitmap.height,
                rgba: bitmap.rgba,
            }),
            Err(error) => convert::<(), ScreenshotBitmap>(Err(error)),
        }
    })
    .await?
}
pub async fn screenshot_rename(
    host: Host,
    instance_id: String,
    screenshot: Screenshot,
    name: String,
) -> ApiResult<()> {
    blocking(move || {
        convert(app::rename_screenshot(
            host,
            request(json!({"instanceId":instance_id,"screenshot":screenshot,"name":name}))?,
        ))
    })
    .await?
}
pub async fn screenshot_action(
    host: Host,
    instance_id: String,
    screenshot: Screenshot,
    action: String,
    confirmed: bool,
) -> ApiResult<()> {
    blocking(move || convert(app::screenshot_action(host, request(json!({"instanceId":instance_id,"screenshot":screenshot,"action":action,"confirmed":confirmed}))?))).await?
}

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
    pub provider: Option<String>,
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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fullscreen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_account_id: Option<String>,
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
    #[serde(default)]
    pub can_launch_offline: bool,
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
    #[serde(default)]
    pub can_stop: bool,
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
    pub sha256: String,
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
    #[serde(default)]
    pub clients: std::collections::BTreeMap<String, Value>,
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
    #[serde(default)]
    pub display_version: Option<String>,
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
    /// Stable machine code; absent from older payloads.
    #[serde(default)]
    pub code: String,
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
    #[serde(default)]
    pub file_type: String,
    #[serde(default)]
    pub sha256: Option<String>,
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
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    /// Published `.mrpack` byte size (modpacks only), known before download.
    #[serde(default)]
    pub archive_size: Option<u64>,
    /// The core's reason Aurora cannot install this modpack version.
    #[serde(default)]
    pub unsupported_reason: Option<String>,
}

/// One instance as a destination for adding an exact pack version
/// (`pack_merge::PackDestination`). The core decides `compatible`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackDestination {
    pub instance_id: String,
    pub display_name: String,
    pub minecraft_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub compatible: bool,
    #[serde(default)]
    pub reasons: Vec<String>,
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
    pub sha256: String,
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
    let icons_host = host.clone();
    let (state_res, icons_res) = tokio::join!(
        app::get_launcher_state(host),
        blocking(move || app::get_instance_icons(icons_host).unwrap_or_default()),
    );
    let mut state: LauncherState = convert(state_res)?;
    state.icons = icons_res.unwrap_or_default();
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
    /// Subcells per legacy cell; absent means the original six-column grid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdivisions: Option<u8>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaytimeInterval {
    pub start_day: u64,
    pub end_day: u64,
    pub duration_ms: u64,
}

pub async fn historical_playtime(
    host: Host,
    instance_id: Option<String>,
) -> ApiResult<Vec<PlaytimeInterval>> {
    blocking(move || convert(app::get_historical_playtime(host, instance_id))).await?
}

pub async fn playtime(host: Host, instance_id: Option<String>) -> ApiResult<Playtime> {
    blocking(move || convert(app::get_playtime_summary(host, instance_id))).await?
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTarget {
    pub id: String,
    pub instance_id: String,
    pub display_name: String,
    #[serde(default)]
    pub destination: String,
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
    blocking(move || convert(app::get_home_servers(host, instance_id, limit))).await?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedDestination {
    pub id: String,
    pub display_name: String,
    pub identity: String,
    pub last_played_ms: u64,
    pub detail: String,
    pub icon: Option<Vec<u8>>,
    pub favorite: bool,
}

pub async fn saved_destinations(
    host: Host,
    instance: String,
    worlds: bool,
) -> ApiResult<Vec<SavedDestination>> {
    blocking(move || convert(app::get_saved_destinations(host, instance, worlds))).await?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub target_id: String,
    pub status: String,
    pub favicon: Option<String>,
}

pub async fn refresh_server_status(
    host: Host,
    target_ids: Vec<String>,
) -> ApiResult<Vec<ServerStatus>> {
    let request = request(json!({ "targetIds": target_ids }))?;
    convert(app::refresh_recent_server_status(host, request).await)
}

pub async fn refresh_saved_server_status(
    host: Host,
    instance: String,
    ids: Vec<String>,
) -> ApiResult<Vec<ServerStatus>> {
    convert(app::refresh_saved_server_status(host, instance, ids).await)
}

pub async fn server_favorite(host: Host, target_id: String, favorite: bool) -> ApiResult<()> {
    blocking(move || convert(app::set_recent_server_favorite(host, target_id, favorite))).await?
}

/// The core resolves the stored target's instance and server, then uses the
/// normal authenticated, deeply validated launch pipeline.
pub async fn quick_play_server(
    host: Host,
    target_id: String,
    account_id: String,
) -> ApiResult<LaunchProcess> {
    let request =
        request(json!({ "targetId": target_id, "mode": "server", "accountId": account_id }))?;
    convert(app::quick_play_history(host, request).await)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerMotdSegment {
    pub text: String,
}

/// Display-only mirror of the core's sanitized, cache-first status response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPresentation {
    pub target_id: String,
    pub name: Option<String>,
    pub motd: Vec<Vec<ServerMotdSegment>>,
    pub favicon: Option<String>,
}

pub async fn server_presentations(
    host: Host,
    target_ids: Vec<String>,
) -> ApiResult<Vec<ServerPresentation>> {
    convert(
        app::refresh_recent_server_status(host, app::ServerEnrichmentRequest { target_ids }).await,
    )
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

pub fn initialize_discord(host: Host) -> ApiResult<()> {
    app::initialize_discord(host).map_err(Into::into)
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

pub use aurora_launcher_lib::launch::logs::{LogSnapshot, UploadResult};

pub async fn game_log_preference(host: Host, show: Option<bool>) -> ApiResult<bool> {
    blocking(move || app::game_log_preference(host, show).map_err(ApiError::from)).await?
}
pub async fn game_log(instance_id: String) -> ApiResult<LogSnapshot> {
    let request = request(json!({"instanceId": instance_id}))?;
    blocking(move || app::game_log(request).map_err(ApiError::from)).await?
}
pub fn subscribe_game_log(instance_id: String) -> ApiResult<tokio::sync::watch::Receiver<u64>> {
    app::subscribe_game_log(request(json!({"instanceId": instance_id}))?).map_err(ApiError::from)
}
pub async fn stop_game(instance_id: String, process_id: Option<u32>) -> ApiResult<()> {
    blocking(move || {
        app::stop_game(app::StopGameRequest {
            instance_id,
            process_id,
        })
        .map_err(ApiError::from)
    })
    .await?
}
pub async fn diagnose_game_log(
    host: Host,
    instance_id: String,
) -> ApiResult<aurora_launcher_lib::launch::diagnostics::LaunchFailureDiagnostic> {
    let request = request(json!({"instanceId": instance_id}))?;
    blocking(move || app::diagnose_game_log(host, request).map_err(ApiError::from)).await?
}
pub async fn game_log_folders(host: Host, instance_id: String) -> ApiResult<(bool, bool)> {
    let request = request(json!({"instanceId": instance_id}))?;
    blocking(move || app::game_log_folders(host, request).map_err(ApiError::from)).await?
}
pub async fn open_game_log_folder(host: Host, instance_id: String, crash: bool) -> ApiResult<()> {
    let request = request(json!({"instanceId": instance_id}))?;
    blocking(move || app::open_game_log_folder(host, request, crash).map_err(ApiError::from))
        .await?
}
pub async fn upload_game_log(
    instance_id: String,
    confirmed_public: bool,
) -> ApiResult<UploadResult> {
    app::upload_game_log(
        request(json!({"instanceId": instance_id}))?,
        confirmed_public,
    )
    .await
    .map_err(ApiError::from)
}
pub async fn open_game_log_link(url: String) -> ApiResult<()> {
    if !url.starts_with("https://mclo.gs/") {
        return Err(ApiError::new("invalid_log_link", "Invalid log link."));
    }
    blocking(move || {
        aurora_launcher_lib::host::open_url(&url)
            .map_err(|_| ApiError::new("open_failed", "The link could not be opened."))
    })
    .await?
}

pub async fn launch_state(instance_id: String) -> ApiResult<LaunchProcess> {
    let request = request(json!({ "instanceId": instance_id }))?;
    blocking(move || convert(app::get_launch_state(request))).await?
}

pub async fn play(host: Host, instance_id: String, account_id: String) -> ApiResult<LaunchProcess> {
    let request = request(json!({ "instanceId": instance_id, "accountId": account_id }))?;
    convert(app::play_instance(host, request).await)
}

pub async fn quick_play(
    host: Host,
    mode: String,
    target_id: String,
    account_id: String,
    instance_id: Option<String>,
) -> ApiResult<LaunchProcess> {
    let request = request(json!({
        "mode": mode,
        "targetId": target_id,
        "accountId": account_id,
        "instanceId": instance_id,
    }))?;
    convert(app::quick_play_history(host, request).await)
}

pub async fn favorite_server(host: Host, target_id: String, favorite: bool) -> ApiResult<()> {
    blocking(move || {
        if target_id.contains(':') {
            convert(app::set_saved_server_favorite(host, target_id, favorite))
        } else {
            convert(app::set_recent_server_favorite(host, target_id, favorite))
        }
    })
    .await?
}

pub use aurora_launcher_lib::saved_servers::editor::{
    Edit as ServerEdit, ResourcePacks, ServerInput, ServerList,
};

pub async fn server_list(host: Host, instance_id: String) -> ApiResult<ServerList> {
    blocking(move || app::get_server_list(host, instance_id).map_err(ApiError::from)).await?
}

pub async fn edit_server_list(
    host: Host,
    instance_id: String,
    revision: String,
    edit: ServerEdit,
) -> ApiResult<ServerList> {
    blocking(move || {
        app::edit_server_list(host, instance_id, revision, edit).map_err(ApiError::from)
    })
    .await?
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

#[derive(Clone, PartialEq, serde::Serialize)]
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
pub async fn pack_icon(
    host: Host,
    instance: String,
    resource_pack: bool,
    file_name: String,
    project: Option<String>,
) -> ApiResult<Option<ModIconBitmap>> {
    blocking(move || {
        convert(Ok(app::get_installed_pack_icon(
            host,
            &instance,
            resource_pack,
            &file_name,
            project.as_deref(),
        )))
    })
    .await?
}

pub async fn mod_icons(
    host: Host,
    id: String,
    files: Vec<ModIconRequest>,
) -> ApiResult<Vec<ModIconResult>> {
    let request = request(json!({"instanceId": id, "files": files}))?;
    convert(app::get_installed_mod_icons(host, request).await)
}

#[derive(Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIconRequest {
    pub file_name: String,
    pub sha256: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIconResult {
    pub file_name: String,
    pub bitmap: Option<ModIconBitmap>,
}
pub async fn pack_icons(
    host: Host,
    id: String,
    kind: &str,
    files: Vec<PackIconRequest>,
) -> ApiResult<Vec<PackIconResult>> {
    let request = request(json!({"instanceId":id,"contentType":kind,"files":files}))?;
    convert(app::get_installed_pack_icons(host, request).await)
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

pub async fn project_page(
    kind: &'static str,
    project: String,
) -> ApiResult<aurora_launcher_lib::modrinth::ProjectPage> {
    let request = request(json!({"contentType": kind, "projectId": project}))?;
    app::get_modrinth_project_page(request)
        .await
        .map_err(Into::into)
}

pub async fn project_changelog(project: String, version: String) -> ApiResult<Option<String>> {
    let request = request(json!({"projectId": project, "versionId": version}))?;
    convert(app::get_modrinth_changelog(request).await)
}

pub async fn project_image(host: Host, url: String) -> Option<Vec<u8>> {
    let source = app::get_modrinth_project_image(host, &url).await;
    decode_project_png(source.as_deref())
}

/// Only normalized, bounded PNGs from the core's isolated artwork decoder.
pub async fn project_artwork(host: Host, project: String) -> ApiResult<Option<Vec<u8>>> {
    let request = request(json!({"projectId": project}))?;
    let value: Value = convert(app::resolve_project_artwork(host, request).await)?;
    Ok(decode_project_png(value["source"].as_str()))
}

fn decode_project_png(source: Option<&str>) -> Option<Vec<u8>> {
    use base64::Engine;
    source
        .and_then(|s| s.strip_prefix("data:image/png;base64,"))
        .filter(|s| s.len() <= (2 * 1024 * 1024_usize).div_ceil(3) * 4)
        .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
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

/// Every instance marked as a compatible or incompatible destination for
/// adding this exact pack version. Read-only; nothing is downloaded.
pub async fn pack_destinations(
    host: Host,
    project: String,
    version: String,
) -> ApiResult<Vec<PackDestination>> {
    let request = request(json!({"projectId": project, "versionId": version}))?;
    convert(app::get_modrinth_pack_destinations(host, request).await)
}

pub async fn install_pack(host: Host, merge: bool, value: Value) -> ApiResult<Value> {
    if merge {
        convert(Box::pin(app::install_modrinth_pack_merge(host, request(value)?)).await)
    } else {
        convert(Box::pin(app::install_modrinth_pack(host, request(value)?)).await)
    }
}

/// Finishes the kept, still-installing instance of an interrupted new-pack
/// install (`ApiError::instance_id`) instead of creating another instance.
pub async fn resume_pack(host: Host, instance_id: String) -> ApiResult<Value> {
    let request = request(json!({ "instanceId": instance_id }))?;
    convert(Box::pin(app::resume_modrinth_pack(host, request)).await)
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
pub async fn review_local_modpack(host: Host, path: String) -> ApiResult<Value> {
    let request = request(json!({ "sourcePath": path }))?;
    convert(app::review_local_modpack(host, request).await)
}

pub async fn install_local_modpack(
    host: Host,
    snapshot: String,
    fingerprint: String,
) -> ApiResult<Value> {
    let request = request(json!({ "snapshot": snapshot, "fingerprint": fingerprint }))?;
    let summary = Box::pin(app::install_local_modpack(host, request)).await;
    convert(summary)
}

pub async fn identify_local_modpack(host: Host, path: String) -> ApiResult<Value> {
    let request = request(json!({ "sourcePath": path }))?;
    convert(app::identify_local_modpack(host, request).await)
}

fn map_head_avatar(avatar: aurora_launcher_lib::auth::avatar::HeadAvatar) -> HeadAvatar {
    HeadAvatar {
        rgba: avatar.rgba,
        sha256: avatar.sha256,
        skin_rgba: avatar.skin_rgba,
        skin_height: avatar.skin_height,
        model: avatar.model,
    }
}

pub async fn skin_preview(host: Host, preset: String) -> ApiResult<HeadAvatar> {
    let request = request(json!({"presetId": preset}))?;
    blocking(move || {
        app::skin_preset_preview(host, request)
            .map(map_head_avatar)
            .map_err(ApiError::from)
    })
    .await?
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
    blocking(move || {
        app::get_cached_account_avatars(host)
            .map(|items| {
                items
                    .into_iter()
                    .map(|item| CachedAvatar {
                        account_id: item.account_id,
                        avatar: map_head_avatar(item.avatar),
                    })
                    .collect()
            })
            .map_err(ApiError::from)
    })
    .await?
}

pub async fn account_avatar(host: Host, account_id: String) -> ApiResult<Option<HeadAvatar>> {
    let request = request(json!({ "accountId": account_id, "refresh": false }))?;
    app::get_account_avatar(host, request)
        .await
        .map(|avatar| avatar.map(map_head_avatar))
        .map_err(ApiError::from)
}

pub async fn begin_login(host: Host) -> ApiResult<AccountSummary> {
    convert(app::begin_microsoft_login(host).await)
}

pub use app::{LoginCancelOutcome, LoginCancellation};

/// A non-serializable, single-use backend request, bound before async dispatch.
pub fn prepare_login() -> app::LoginRequest {
    app::prepare_microsoft_login()
}

pub async fn begin_prepared_login(
    host: Host,
    login: app::LoginRequest,
) -> ApiResult<AccountSummary> {
    convert(app::begin_prepared_microsoft_login(host, login).await)
}

pub async fn cancel_prepared_login(
    cancellation: LoginCancellation,
) -> ApiResult<LoginCancelOutcome> {
    Ok(app::cancel_prepared_microsoft_login(&cancellation))
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
    app::get_cosmetics(host, request)
        .await
        .map(|state| Cosmetics {
            current_skin_model: state.current_skin_model.map(|model| match model {
                aurora_launcher_lib::cosmetics::SkinModel::Classic => "classic".into(),
                aurora_launcher_lib::cosmetics::SkinModel::Slim => "slim".into(),
            }),
            has_current_skin: state.has_current_skin,
            current_skin: state.current_skin.map(map_head_avatar),
            capes: state
                .capes
                .into_iter()
                .map(|cape| Cape {
                    id: cape.id,
                    name: cape.name,
                    selected: cape.selected,
                    preview: cape.preview.map(|preview| Texture {
                        width: preview.width,
                        height: preview.height,
                        rgba: preview.rgba,
                    }),
                })
                .collect(),
        })
        .map_err(ApiError::from)
}

pub async fn skin_presets(host: Host) -> ApiResult<Vec<SkinPreset>> {
    blocking(move || convert(app::list_skin_presets(host))).await?
}

pub async fn skin_thumbnail(host: Host, preset_id: String) -> ApiResult<Option<Thumbnail>> {
    let request = request(json!({ "presetId": preset_id }))?;
    blocking(move || {
        app::skin_preset_thumbnail(host, request)
            .map(|thumb| thumb.map(|t| Thumbnail { rgba: t.rgba }))
            .map_err(ApiError::from)
    })
    .await?
}

pub async fn save_current_skin(host: Host, account_id: String) -> ApiResult<Value> {
    let request = request(json!({ "accountId": account_id }))?;
    convert(app::save_current_skin(host, request).await)
}

pub async fn set_skin_favorite(
    host: Host,
    preset_id: String,
    favorite: bool,
) -> ApiResult<Vec<SkinPreset>> {
    let request = request(json!({"presetId": preset_id, "favorite": favorite}))?;
    blocking(move || convert(app::set_skin_favorite(host, request))).await?
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
            json!({"rgba":[1,2,3,255],"skinRgba":[9,8,7,255],"skinHeight":64,"model":"slim","sha256":"skin-content"}),
        )
        .unwrap();
        assert_eq!(avatar.skin_rgba, vec![9, 8, 7, 255]);
        assert_eq!(avatar.skin_height, 64);
        assert_eq!(avatar.model, "slim");
        assert_eq!(avatar.sha256, "skin-content");
    }

    #[test]
    fn library_content_identity_survives_the_native_boundary() {
        let preset: SkinPreset = serde_json::from_value(
            json!({"id":"skin", "name":"Coat", "model":"slim", "sha256":"skin-content"}),
        )
        .unwrap();
        assert_eq!(preset.sha256, "skin-content");
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

// Pulsar uses the shared, serde camelCase presentation contract.
pub use aurora_launcher_lib::pulsar::view::*;
pub async fn pulsar_library(host: Host, request: LibraryQuery) -> ApiResult<LibraryView> {
    app::list_pulsar_library(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_refresh(host: Host, request: LibraryQuery) -> ApiResult<LibraryView> {
    app::refresh_pulsar_library(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_analysis(host: Host, request: AnalysisRequest) -> ApiResult<AnalysisView> {
    app::analyze_pulsar(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_detail(host: Host, request: DetailRequest) -> ApiResult<FightDetailView> {
    app::get_pulsar_fight(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_playback(host: Host, request: PlaybackRequest) -> ApiResult<PlaybackView> {
    app::get_pulsar_playback(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_detail_analysis(
    host: Host,
    request: DetailAnalysisRequest,
) -> ApiResult<FightDetailView> {
    app::analyze_pulsar_fight(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_settings(host: Host, request: FolderRequest) -> ApiResult<SettingsView> {
    app::get_pulsar_settings(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_opponents(host: Host, request: LibraryQuery) -> ApiResult<Vec<OpponentView>> {
    app::get_pulsar_opponents(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_records(host: Host, request: LibraryQuery) -> ApiResult<Vec<RecordView>> {
    app::get_pulsar_records(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_compare(host: Host, request: CompareRequest) -> ApiResult<CompareView> {
    app::compare_pulsar_fights(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_open_folder(host: Host, request: FolderRequest) -> ApiResult<ActionReceipt> {
    app::open_pulsar_folder(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_view_report(host: Host, request: FightKey) -> ApiResult<ActionReceipt> {
    app::view_pulsar_report(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_export(host: Host, request: ExportRequest) -> ApiResult<ActionReceipt> {
    app::export_pulsar_report(host, request)
        .await
        .map_err(ApiError::from)
}
pub async fn pulsar_delete(host: Host, request: DeleteRequest) -> ApiResult<ActionReceipt> {
    app::delete_pulsar_fight(host, request)
        .await
        .map_err(ApiError::from)
}

// ---------------------------------------------------------------------------
// 2.6 Launcher Extensions (duplicate, organization, export, worlds, storage,
// bisect, offline play, launch behavior, bulk content, Pulsar annotations)
// ---------------------------------------------------------------------------

pub use aurora_launcher_lib::config::OnLaunchBehavior;
pub use aurora_launcher_lib::instances::duplicate::DuplicateOptions;
pub use aurora_launcher_lib::instances::organization::{InstanceMeta, InstanceOrganization};
pub use aurora_launcher_lib::mod_bisect::{BisectStatus, BisectView};
pub use aurora_launcher_lib::pack_export::{ExportOptions, ExportPreview, ExportReceipt};
pub use aurora_launcher_lib::pulsar::annotations::FightAnnotation;
pub use aurora_launcher_lib::saved_worlds::DatapackEntry;
pub use aurora_launcher_lib::storage::{
    InstanceStorageEntry, PruneOptions, PruneReport, RuntimeStorageEntry, StorageOverview,
};
pub use aurora_launcher_lib::world_backups::WorldBackupSummary;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceWorld {
    pub directory: String,
    pub display_name: String,
    pub last_played_ms: u64,
    pub game_mode: Option<String>,
    pub version_name: Option<String>,
    pub size_bytes: u64,
    pub datapack_count: usize,
    pub backup_count: usize,
}

pub async fn duplicate_instance(
    host: Host,
    source_instance_id: String,
    display_name: String,
    options: DuplicateOptions,
) -> ApiResult<InstanceSummary> {
    let req = app::DuplicateInstanceRequest {
        source_instance_id,
        display_name,
        options,
    };
    convert(app::duplicate_instance(host, req).await)
}

pub async fn instance_organization(host: Host) -> ApiResult<InstanceOrganization> {
    blocking(move || app::get_instance_organization(host).map_err(ApiError::from)).await?
}

pub async fn set_instance_organization(
    host: Host,
    instance_id: String,
    pinned: bool,
    tags: Vec<String>,
) -> ApiResult<InstanceOrganization> {
    blocking(move || {
        app::set_instance_organization(
            host,
            app::SetInstanceOrganizationRequest {
                instance_id,
                pinned,
                tags,
            },
        )
        .map_err(ApiError::from)
    })
    .await?
}

pub async fn preview_pack_export(
    host: Host,
    instance_id: String,
    options: ExportOptions,
) -> ApiResult<ExportPreview> {
    app::preview_pack_export(
        host,
        app::PackExportPreviewRequest {
            instance_id,
            options,
        },
    )
    .await
    .map_err(ApiError::from)
}

pub async fn export_instance_pack(
    host: Host,
    instance_id: String,
    options: ExportOptions,
    destination: String,
) -> ApiResult<ExportReceipt> {
    app::export_instance_pack(
        host,
        app::PackExportRequest {
            instance_id,
            options,
            destination,
        },
    )
    .await
    .map_err(ApiError::from)
}

pub async fn instance_worlds(host: Host, instance_id: String) -> ApiResult<Vec<InstanceWorld>> {
    convert(app::list_instance_worlds(host, instance_id).await)
}

pub async fn world_backups(
    host: Host,
    instance_id: String,
    world_directory: String,
) -> ApiResult<Vec<WorldBackupSummary>> {
    app::list_world_backups(host, instance_id, world_directory)
        .await
        .map_err(ApiError::from)
}

pub async fn create_world_backup(
    host: Host,
    instance_id: String,
    world_directory: String,
    label: Option<String>,
) -> ApiResult<WorldBackupSummary> {
    app::create_world_backup(host, instance_id, world_directory, label)
        .await
        .map_err(ApiError::from)
}

pub async fn restore_world_backup(
    host: Host,
    instance_id: String,
    world_directory: String,
    backup_id: String,
) -> ApiResult<()> {
    app::restore_world_backup(host, instance_id, world_directory, backup_id)
        .await
        .map_err(ApiError::from)
}

pub async fn delete_world_backup(
    host: Host,
    instance_id: String,
    world_directory: String,
    backup_id: String,
) -> ApiResult<()> {
    app::delete_world_backup(host, instance_id, world_directory, backup_id)
        .await
        .map_err(ApiError::from)
}

pub async fn world_datapacks(
    host: Host,
    instance_id: String,
    world_directory: String,
) -> ApiResult<Vec<DatapackEntry>> {
    app::list_world_datapacks(host, instance_id, world_directory)
        .await
        .map_err(ApiError::from)
}

pub async fn set_world_datapack_enabled(
    host: Host,
    instance_id: String,
    world_directory: String,
    file_name: String,
    enabled: bool,
) -> ApiResult<()> {
    blocking(move || {
        app::set_world_datapack_enabled(host, instance_id, world_directory, file_name, enabled)
            .map_err(ApiError::from)
    })
    .await?
}

pub async fn remove_world_datapack(
    host: Host,
    instance_id: String,
    world_directory: String,
    file_name: String,
) -> ApiResult<()> {
    blocking(move || {
        app::remove_world_datapack(host, instance_id, world_directory, file_name)
            .map_err(ApiError::from)
    })
    .await?
}

pub async fn browse_datapacks(
    host: Host,
    instance_id: String,
    query: String,
    offset: u32,
) -> ApiResult<SearchPage> {
    convert(app::browse_datapacks(host, instance_id, query, offset).await)
}

pub async fn install_world_datapack(
    host: Host,
    instance_id: String,
    world_directory: String,
    project_id: String,
) -> ApiResult<DatapackEntry> {
    app::install_world_datapack(host, instance_id, world_directory, project_id)
        .await
        .map_err(ApiError::from)
}

pub async fn inspect_storage(host: Host) -> ApiResult<StorageOverview> {
    app::inspect_storage(host).await.map_err(ApiError::from)
}

pub async fn prune_storage(host: Host, options: PruneOptions) -> ApiResult<PruneReport> {
    app::prune_storage(host, options)
        .await
        .map_err(ApiError::from)
}

pub async fn mod_bisect(host: Host, instance_id: String) -> ApiResult<Option<BisectView>> {
    blocking(move || app::get_mod_bisect(host, instance_id).map_err(ApiError::from)).await?
}

pub async fn start_mod_bisect(host: Host, instance_id: String) -> ApiResult<BisectView> {
    blocking(move || app::start_mod_bisect(host, instance_id).map_err(ApiError::from)).await?
}

pub async fn record_mod_bisect_outcome(
    host: Host,
    instance_id: String,
    crashed: bool,
) -> ApiResult<BisectView> {
    blocking(move || {
        app::record_mod_bisect_outcome(host, instance_id, crashed).map_err(ApiError::from)
    })
    .await?
}

pub async fn undo_mod_bisect(host: Host, instance_id: String) -> ApiResult<BisectView> {
    blocking(move || app::undo_mod_bisect(host, instance_id).map_err(ApiError::from)).await?
}

pub async fn finish_mod_bisect(
    host: Host,
    instance_id: String,
    keep_culprit_disabled: bool,
) -> ApiResult<()> {
    blocking(move || {
        app::finish_mod_bisect(host, instance_id, keep_culprit_disabled).map_err(ApiError::from)
    })
    .await?
}

pub async fn on_launch_behavior(
    host: Host,
    behavior: Option<OnLaunchBehavior>,
) -> ApiResult<OnLaunchBehavior> {
    blocking(move || app::on_launch_behavior(host, behavior).map_err(ApiError::from)).await?
}

pub async fn create_instance_desktop_shortcut(
    host: Host,
    instance_id: String,
) -> ApiResult<String> {
    blocking(move || {
        app::create_instance_desktop_shortcut(host, instance_id).map_err(ApiError::from)
    })
    .await?
}

pub async fn active_launches() -> ApiResult<Vec<LaunchProcess>> {
    blocking(move || convert(Ok(app::list_active_launches()))).await?
}

pub async fn set_mods_enabled_bulk(
    host: Host,
    instance_id: String,
    entry_ids: Vec<String>,
    enabled: bool,
) -> ApiResult<usize> {
    blocking(move || {
        app::set_mods_enabled_bulk(host, instance_id, entry_ids, enabled).map_err(ApiError::from)
    })
    .await?
}

pub async fn remove_mods_bulk(
    host: Host,
    instance_id: String,
    entry_ids: Vec<String>,
) -> ApiResult<usize> {
    blocking(move || app::remove_mods_bulk(host, instance_id, entry_ids).map_err(ApiError::from))
        .await?
}

pub async fn open_instance_config_folder(host: Host, instance_id: String) -> ApiResult<()> {
    blocking(move || app::open_instance_config_folder(host, instance_id).map_err(ApiError::from))
        .await?
}

pub async fn increase_instance_memory_quick(
    host: Host,
    instance_id: String,
    delta_mib: u32,
) -> ApiResult<InstanceSummary> {
    blocking(move || {
        convert(app::increase_instance_memory_quick(
            host,
            instance_id,
            delta_mib,
        ))
    })
    .await?
}

pub async fn play_offline(
    host: Host,
    instance_id: String,
    account_id: String,
) -> ApiResult<LaunchProcess> {
    let req = request(json!({"instanceId": instance_id, "accountId": account_id}))?;
    convert(app::launch_instance_offline(host, req).await)
}

pub async fn pulsar_set_annotation(
    host: Host,
    request: AnnotationRequest,
) -> ApiResult<FightAnnotation> {
    app::set_pulsar_annotation(host, request)
        .await
        .map_err(ApiError::from)
}

pub async fn pulsar_recent_widget(
    host: Host,
    instance_id: Option<String>,
    limit: usize,
) -> ApiResult<RecentFightsWidgetSummary> {
    app::get_pulsar_recent_widget(host, instance_id, limit)
        .await
        .map_err(ApiError::from)
}

pub async fn pulsar_export_stats(
    host: Host,
    request: StatsExportRequest,
) -> ApiResult<ActionReceipt> {
    app::export_pulsar_stats(host, request)
        .await
        .map_err(ApiError::from)
}
