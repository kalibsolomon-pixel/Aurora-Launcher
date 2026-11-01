//! Tauri IPC adapter for the host-neutral application services.
//!
//! Generated once by the NATIVE-1 extraction: every command keeps its
//! original name, parameter names (the frontend's argument keys), sync/async
//! nature and return type, and delegates to the identical implementation in
//! `application`. Only the `AppHandle` parameter is converted into a
//! [`Host`]. Keep this file thin; behavior belongs in `application`.

use super::*;
use crate::host::Host;

#[tauri::command]
pub fn save_instance_sync_policy(
    app: tauri::AppHandle,
    request: crate::instance_sync::Policy,
) -> Result<crate::instance_sync::Policy, CommandError> {
    super::save_instance_sync_policy(Host::from_tauri(&app), request)
}
#[tauri::command]
pub fn get_instance_sync_status() -> crate::instance_sync::automatic::Status {
    super::get_instance_sync_status()
}

#[tauri::command]
pub fn get_instance_sync_policy(
    app: tauri::AppHandle,
) -> Result<crate::instance_sync::Policy, CommandError> {
    super::get_instance_sync_policy(Host::from_tauri(&app))
}
#[tauri::command]
pub fn set_instance_sync_enabled(
    app: tauri::AppHandle,
    request: SyncEnabledRequest,
) -> Result<crate::instance_sync::Policy, CommandError> {
    super::set_instance_sync_enabled(Host::from_tauri(&app), request)
}
#[tauri::command]
pub async fn preview_instance_sync(
    app: tauri::AppHandle,
    request: crate::instance_sync::Request,
) -> Result<crate::instance_sync::Preview, CommandError> {
    super::preview_instance_sync(Host::from_tauri(&app), request).await
}
#[tauri::command]
pub async fn apply_instance_sync(
    app: tauri::AppHandle,
    request: SyncApplyRequest,
) -> Result<crate::instance_sync::Receipt, CommandError> {
    super::apply_instance_sync(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn discard_local_content_review(
    app: tauri::AppHandle,
    request: LocalContentDiscardRequest,
) -> Result<(), CommandError> {
    super::discard_local_content_review(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn preview_local_content(
    app: tauri::AppHandle,
    request: LocalContentPreviewRequest,
) -> Result<crate::local_content::Preview, CommandError> {
    super::preview_local_content(Host::from_tauri(&app), request).await
}
#[tauri::command]
pub async fn install_local_content(
    app: tauri::AppHandle,
    request: LocalContentInstallRequest,
) -> Result<(), CommandError> {
    super::install_local_content(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn get_application_status(app: tauri::AppHandle) -> Result<ApplicationStatus, CommandError> {
    super::get_application_status(Host::from_tauri(&app))
}

#[tauri::command]
pub async fn get_launcher_state(app: tauri::AppHandle) -> Result<LauncherState, CommandError> {
    super::get_launcher_state(Host::from_tauri(&app)).await
}

#[tauri::command]
pub async fn delete_instance(
    app: tauri::AppHandle,
    request: DeleteInstanceRequest,
) -> Result<(), CommandError> {
    super::delete_instance(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn get_home_widgets(
    app: tauri::AppHandle,
) -> Result<crate::home_widgets::HomeLayout, CommandError> {
    super::get_home_widgets(Host::from_tauri(&app))
}

#[tauri::command]
pub fn get_playtime_summary(
    app: tauri::AppHandle,
    instance_id: Option<String>,
) -> Result<crate::gameplay_history::Playtime, CommandError> {
    super::get_playtime_summary(Host::from_tauri(&app), instance_id)
}

#[tauri::command]
pub fn get_daily_playtime(
    app: tauri::AppHandle,
    instance_id: Option<String>,
    days: usize,
) -> Result<Vec<crate::gameplay_history::DailyPlaytime>, CommandError> {
    super::get_daily_playtime(Host::from_tauri(&app), instance_id, days)
}

#[tauri::command]
pub fn get_recent_worlds(
    app: tauri::AppHandle,
    instance_id: Option<String>,
    limit: usize,
) -> Result<Vec<crate::gameplay_history::RecentTarget>, CommandError> {
    super::get_recent_worlds(Host::from_tauri(&app), instance_id, limit)
}

#[tauri::command]
pub fn get_recent_servers(
    app: tauri::AppHandle,
    instance_id: Option<String>,
    limit: usize,
) -> Result<Vec<crate::gameplay_history::RecentTarget>, CommandError> {
    super::get_recent_servers(Host::from_tauri(&app), instance_id, limit)
}

#[tauri::command]
pub fn set_recent_server_favorite(
    app: tauri::AppHandle,
    target_id: String,
    favorite: bool,
) -> Result<(), CommandError> {
    super::set_recent_server_favorite(Host::from_tauri(&app), target_id, favorite)
}

#[tauri::command]
pub async fn refresh_recent_server_status(
    app: tauri::AppHandle,
    request: ServerEnrichmentRequest,
) -> Result<Vec<crate::server_enrichment::RecentServerPresentation>, CommandError> {
    super::refresh_recent_server_status(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn set_home_widgets(
    app: tauri::AppHandle,
    request: crate::home_widgets::HomeLayout,
) -> Result<crate::home_widgets::HomeLayout, CommandError> {
    super::set_home_widgets(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn reset_home_widgets(
    app: tauri::AppHandle,
) -> Result<crate::home_widgets::HomeLayout, CommandError> {
    super::reset_home_widgets(Host::from_tauri(&app))
}

#[tauri::command]
pub fn get_discord_state() -> crate::discord::DiscordState {
    super::get_discord_state()
}

#[tauri::command]
pub async fn connect_discord() -> crate::discord::DiscordState {
    super::connect_discord().await
}

#[tauri::command]
pub fn set_discord_preferences(
    app: tauri::AppHandle,
    request: crate::discord::DiscordPreferences,
) -> Result<crate::discord::DiscordState, CommandError> {
    super::set_discord_preferences(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn get_appearance(app: tauri::AppHandle) -> Result<AppearanceDto, CommandError> {
    super::get_appearance(Host::from_tauri(&app))
}

#[tauri::command]
pub fn set_appearance(
    app: tauri::AppHandle,
    request: SetAppearanceRequest,
) -> Result<AppearanceDto, CommandError> {
    super::set_appearance(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn get_desktop_integration() -> Result<DesktopIntegrationDto, CommandError> {
    super::get_desktop_integration()
}

#[tauri::command]
pub fn create_desktop_shortcut() -> Result<DesktopIntegrationDto, CommandError> {
    super::create_desktop_shortcut()
}

#[tauri::command]
pub fn create_start_menu_shortcut() -> Result<DesktopIntegrationDto, CommandError> {
    super::create_start_menu_shortcut()
}

#[tauri::command]
pub fn open_launcher_folder(app: tauri::AppHandle) -> Result<(), CommandError> {
    super::open_launcher_folder(Host::from_tauri(&app))
}

#[tauri::command]
pub fn remove_desktop_shortcut() -> Result<DesktopIntegrationDto, CommandError> {
    super::remove_desktop_shortcut()
}

#[tauri::command]
pub async fn acquire_artifact(
    app: tauri::AppHandle,
    request: AcquireArtifactRequest,
) -> Result<AcquiredArtifact, CommandError> {
    super::acquire_artifact(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn plan_minecraft_install(
    request: PlanMinecraftInstallRequest,
) -> Result<MinecraftPlanSummary, CommandError> {
    super::plan_minecraft_install(request).await
}

#[tauri::command]
pub async fn plan_fabric_install(
    request: PlanFabricInstallRequest,
) -> Result<FabricPlanSummary, CommandError> {
    super::plan_fabric_install(request).await
}

#[tauri::command]
pub async fn plan_neoforge_install(
    app: tauri::AppHandle,
    request: PlanNeoForgeInstallRequest,
) -> Result<NeoForgePlanSummary, CommandError> {
    super::plan_neoforge_install(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn install_game(
    app: tauri::AppHandle,
    request: InstallGameRequest,
) -> Result<InstalledGameSummary, CommandError> {
    super::install_game(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn validate_installed_game(
    app: tauri::AppHandle,
    request: ValidateInstalledGameRequest,
) -> Result<InstalledGameValidationDto, CommandError> {
    super::validate_installed_game(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn list_aurora_releases() -> Result<Vec<AuroraReleaseSummary>, CommandError> {
    super::list_aurora_releases()
}

#[tauri::command]
pub async fn create_instance(
    app: tauri::AppHandle,
    request: CreateInstanceRequest,
) -> Result<InstanceSummary, CommandError> {
    super::create_instance(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn get_aurora_compatibility(
    request: AuroraCompatibilityRequest,
) -> Result<AuroraCompatibility, CommandError> {
    super::get_aurora_compatibility(request)
}

#[tauri::command]
pub fn preview_aurora_transition(
    app: tauri::AppHandle,
    request: AuroraTransitionRequest,
) -> Result<crate::instances::transition::TransitionPreview, CommandError> {
    super::preview_aurora_transition(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn apply_aurora_transition(
    app: tauri::AppHandle,
    request: AuroraTransitionRequest,
) -> Result<InstanceSummary, CommandError> {
    super::apply_aurora_transition(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn retry_instance_install(
    app: tauri::AppHandle,
    request: RetryInstanceRequest,
) -> Result<InstanceSummary, CommandError> {
    super::retry_instance_install(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn rename_instance(
    app: tauri::AppHandle,
    request: RenameInstanceRequest,
) -> Result<InstanceSummary, CommandError> {
    super::rename_instance(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn get_instance_icons(
    app: tauri::AppHandle,
) -> Result<std::collections::BTreeMap<String, String>, CommandError> {
    super::get_instance_icons(Host::from_tauri(&app))
}

#[tauri::command]
pub fn set_instance_icon(
    app: tauri::AppHandle,
    request: SetInstanceIconRequest,
) -> Result<std::collections::BTreeMap<String, String>, CommandError> {
    super::set_instance_icon(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn get_instance_performance_advice(
    app: tauri::AppHandle,
    request: PerformanceAdviceRequest,
) -> Result<crate::instances::performance::MemoryAdvice, CommandError> {
    super::get_instance_performance_advice(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn update_instance_configuration(
    app: tauri::AppHandle,
    request: UpdateInstanceConfigurationRequest,
) -> Result<InstanceSummary, CommandError> {
    super::update_instance_configuration(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn install_instance_configuration(
    app: tauri::AppHandle,
    request: RetryInstanceRequest,
) -> Result<InstanceSummary, CommandError> {
    super::install_instance_configuration(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn open_instance_folder(
    app: tauri::AppHandle,
    request: OpenInstanceFolderRequest,
) -> Result<(), CommandError> {
    super::open_instance_folder(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn get_installed_mod_icons(
    app: tauri::AppHandle,
    request: InstalledModIconsRequest,
) -> Result<Vec<crate::mod_icons::IconResult>, CommandError> {
    super::get_installed_mod_icons(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn get_instance_mods(
    app: tauri::AppHandle,
    request: InstanceModsRequest,
) -> Result<crate::instance_mods::ModInventory, CommandError> {
    super::get_instance_mods(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn set_instance_mod_enabled(
    app: tauri::AppHandle,
    request: SetInstanceModEnabledRequest,
) -> Result<crate::instance_mods::ModInventory, CommandError> {
    super::set_instance_mod_enabled(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn remove_instance_mod(
    app: tauri::AppHandle,
    request: RemoveInstanceModRequest,
) -> Result<crate::instance_mods::ModInventory, CommandError> {
    super::remove_instance_mod(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn open_instance_mods_folder(
    app: tauri::AppHandle,
    request: InstanceModsRequest,
) -> Result<(), CommandError> {
    super::open_instance_mods_folder(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn get_instance_content_context(
    app: tauri::AppHandle,
    request: InstanceModsRequest,
) -> Result<InstanceContentContext, CommandError> {
    super::get_instance_content_context(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn get_instance_content(
    app: tauri::AppHandle,
    request: InstanceContentRequest,
) -> Result<crate::instance_content::ContentInventory, CommandError> {
    super::get_instance_content(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn remove_instance_content(
    app: tauri::AppHandle,
    request: RemoveInstanceContentRequest,
) -> Result<crate::instance_content::ContentInventory, CommandError> {
    super::remove_instance_content(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn set_instance_pack_enabled(
    app: tauri::AppHandle,
    request: SetInstancePackEnabledRequest,
) -> Result<crate::instance_content::ContentInventory, CommandError> {
    super::set_instance_pack_enabled(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn open_instance_content_folder(
    app: tauri::AppHandle,
    request: InstanceContentRequest,
) -> Result<(), CommandError> {
    super::open_instance_content_folder(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn scan_instance_content(
    app: tauri::AppHandle,
    request: InstanceContentRequest,
) -> Result<crate::content_recognition::RecognitionScan, CommandError> {
    super::scan_instance_content(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn register_recovered_content(
    app: tauri::AppHandle,
    request: crate::content_recognition::RecoveredContentApproval,
) -> Result<Vec<crate::instance_content::ProviderRecord>, CommandError> {
    super::register_recovered_content(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn search_modrinth(
    app: tauri::AppHandle,
    request: ModrinthSearchRequest,
) -> Result<crate::modrinth::SearchPage, CommandError> {
    super::search_modrinth(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn browse_modrinth(
    app: tauri::AppHandle,
    request: ProviderBrowseRequest,
) -> Result<crate::modrinth::SearchPage, CommandError> {
    super::browse_modrinth(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn browse_modrinth_tags(
    request: ProviderBrowseTagsRequest,
) -> Result<Vec<crate::modrinth::ProviderCategory>, CommandError> {
    super::browse_modrinth_tags(request).await
}

#[tauri::command]
pub async fn get_modrinth_project(
    app: tauri::AppHandle,
    request: ModrinthProjectRequest,
) -> Result<crate::modrinth::ProjectDetails, CommandError> {
    super::get_modrinth_project(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn get_modrinth_project_artwork(
    app: tauri::AppHandle,
    request: ModrinthArtworkRequest,
) -> Option<String> {
    super::get_modrinth_project_artwork(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn resolve_project_artwork(
    app: tauri::AppHandle,
    request: ModrinthArtworkRequest,
) -> Result<crate::artwork::ArtworkResult, CommandError> {
    super::resolve_project_artwork(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn get_installed_artwork_identities(
    app: tauri::AppHandle,
    request: InstanceContentRequest,
) -> Result<crate::installed_artwork::Identities, CommandError> {
    super::get_installed_artwork_identities(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn preview_modrinth_pack(
    app: tauri::AppHandle,
    request: ModrinthPackRequest,
) -> Result<crate::modpacks::PackPreview, CommandError> {
    super::preview_modrinth_pack(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn install_modrinth_pack(
    app: tauri::AppHandle,
    request: ModrinthPackRequest,
) -> Result<InstanceSummary, CommandError> {
    super::install_modrinth_pack(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn preview_modrinth_pack_merge(
    app: tauri::AppHandle,
    request: ModrinthPackMergeRequest,
) -> Result<crate::pack_merge::PackMergePreview, CommandError> {
    super::preview_modrinth_pack_merge(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn install_modrinth_pack_merge(
    app: tauri::AppHandle,
    request: ModrinthPackMergeRequest,
) -> Result<crate::pack_merge::PackMergeResult, CommandError> {
    super::install_modrinth_pack_merge(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn check_modpack_update(
    app: tauri::AppHandle,
    request: ModpackInstanceRequest,
) -> Result<crate::pack_update::PackUpdateCheck, CommandError> {
    super::check_modpack_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn preview_modpack_update(
    app: tauri::AppHandle,
    request: ModpackInstanceRequest,
) -> Result<crate::pack_update::PackUpdatePlan, CommandError> {
    super::preview_modpack_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn apply_modpack_update(
    app: tauri::AppHandle,
    request: ModpackUpdateApproval,
) -> Result<InstanceSummary, CommandError> {
    super::apply_modpack_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn get_modpack_details(
    app: tauri::AppHandle,
    request: ModpackInstanceRequest,
) -> Result<ModpackDetails, CommandError> {
    super::get_modpack_details(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn preview_modrinth_install(
    app: tauri::AppHandle,
    request: ModrinthPreviewRequest,
) -> Result<ModrinthPreviewResponse, CommandError> {
    super::preview_modrinth_install(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn install_modrinth(
    app: tauri::AppHandle,
    request: ModrinthInstallRequest,
) -> Result<Vec<crate::instance_content::ProviderRecord>, CommandError> {
    super::install_modrinth(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn quick_install_modrinth(
    app: tauri::AppHandle,
    request: ModrinthQuickInstallRequest,
) -> Result<Vec<crate::instance_content::ProviderRecord>, CommandError> {
    super::quick_install_modrinth(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn get_provider_lifecycle(
    app: tauri::AppHandle,
    request: InstanceModsRequest,
) -> Result<Vec<ProviderLifecycleEntry>, CommandError> {
    super::get_provider_lifecycle(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn check_modrinth_update(
    app: tauri::AppHandle,
    request: ProviderLifecycleRequest,
) -> Result<Option<crate::modrinth::VersionChoice>, CommandError> {
    super::check_modrinth_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn check_instance_updates(
    app: tauri::AppHandle,
    request: CheckInstanceUpdatesRequest,
) -> Result<crate::content_updates::UpdatesReport, CommandError> {
    super::check_instance_updates(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn set_provider_update_policy(
    app: tauri::AppHandle,
    request: ProviderPolicyRequest,
) -> Result<crate::instance_content::ProviderRecord, CommandError> {
    super::set_provider_update_policy(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn preview_modrinth_update(
    app: tauri::AppHandle,
    request: ProviderLifecycleRequest,
) -> Result<ProviderUpdatePreview, CommandError> {
    super::preview_modrinth_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn preview_modrinth_bulk_update(
    app: tauri::AppHandle,
    request: ProviderBulkUpdateRequest,
) -> Result<BulkUpdatePreview, CommandError> {
    super::preview_modrinth_bulk_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn apply_modrinth_update(
    app: tauri::AppHandle,
    request: ProviderLifecycleApproval,
) -> Result<(), CommandError> {
    super::apply_modrinth_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn apply_modrinth_bulk_update(
    app: tauri::AppHandle,
    request: ProviderBulkUpdateApproval,
) -> Result<(), CommandError> {
    super::apply_modrinth_bulk_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn preview_provider_removal(
    app: tauri::AppHandle,
    request: ProviderLifecycleRequest,
) -> Result<ProviderRemovalPreview, CommandError> {
    super::preview_provider_removal(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn apply_provider_removal(
    app: tauri::AppHandle,
    request: ProviderLifecycleApproval,
) -> Result<(), CommandError> {
    super::apply_provider_removal(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn list_minecraft_versions(
    include_snapshots: bool,
) -> Result<Vec<MinecraftVersionDto>, CommandError> {
    super::list_minecraft_versions(include_snapshots).await
}

#[tauri::command]
pub async fn list_fabric_loader_versions(
    minecraft_version: String,
) -> Result<Vec<FabricLoaderVersionDto>, CommandError> {
    super::list_fabric_loader_versions(minecraft_version).await
}

#[tauri::command]
pub async fn list_neoforge_versions(
    minecraft_version: String,
) -> Result<Vec<NeoForgeVersionDto>, CommandError> {
    super::list_neoforge_versions(minecraft_version).await
}

#[tauri::command]
pub async fn list_forge_versions(
    minecraft_version: String,
) -> Result<Vec<ForgeVersionDto>, CommandError> {
    super::list_forge_versions(minecraft_version).await
}

#[tauri::command]
pub fn select_instance(
    app: tauri::AppHandle,
    request: SelectInstanceRequest,
) -> Result<(), CommandError> {
    super::select_instance(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn maintain_installation_backups(
    app: tauri::AppHandle,
    request: ValidateInstalledGameRequest,
) -> Result<crate::install::retention::RetentionReport, CommandError> {
    super::maintain_installation_backups(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn validate_instance(
    app: tauri::AppHandle,
    request: ValidateInstalledGameRequest,
) -> Result<InstanceValidationDto, CommandError> {
    super::validate_instance(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn get_instance_runtime_status(
    app: tauri::AppHandle,
    request: ValidateInstalledGameRequest,
) -> Result<RuntimeStatusDto, CommandError> {
    super::get_instance_runtime_status(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn ensure_instance_runtime(
    app: tauri::AppHandle,
    request: ValidateInstalledGameRequest,
) -> Result<RuntimeStatusDto, CommandError> {
    super::ensure_instance_runtime(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn get_accounts(app: tauri::AppHandle) -> Result<AccountsStateDto, CommandError> {
    super::get_accounts(Host::from_tauri(&app))
}

#[tauri::command]
pub async fn begin_microsoft_login(
    app: tauri::AppHandle,
) -> Result<AccountSummaryDto, CommandError> {
    super::begin_microsoft_login(Host::from_tauri(&app)).await
}

#[tauri::command]
pub fn cancel_microsoft_login() -> Result<(), CommandError> {
    super::cancel_microsoft_login()
}

#[tauri::command]
pub fn select_account(
    app: tauri::AppHandle,
    request: SelectAccountRequest,
) -> Result<(), CommandError> {
    super::select_account(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn remove_account(
    app: tauri::AppHandle,
    request: AccountIdRequest,
) -> Result<(), CommandError> {
    super::remove_account(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn refresh_account_session(
    app: tauri::AppHandle,
    request: AccountIdRequest,
) -> Result<AccountSessionDto, CommandError> {
    super::refresh_account_session(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn get_account_avatar(
    app: tauri::AppHandle,
    request: AccountAvatarRequest,
) -> Result<Option<crate::auth::avatar::HeadAvatar>, CommandError> {
    super::get_account_avatar(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn get_cached_account_avatars(
    app: tauri::AppHandle,
) -> Result<Vec<CachedAccountAvatarDto>, CommandError> {
    super::get_cached_account_avatars(Host::from_tauri(&app))
}

#[tauri::command]
pub async fn get_cosmetics(
    app: tauri::AppHandle,
    request: AccountIdRequest,
) -> Result<crate::cosmetics::CosmeticsState, CommandError> {
    super::get_cosmetics(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn list_skin_presets(
    app: tauri::AppHandle,
) -> Result<Vec<crate::cosmetics::SkinPreset>, CommandError> {
    super::list_skin_presets(Host::from_tauri(&app))
}

#[tauri::command]
pub fn set_skin_favorite(
    app: tauri::AppHandle,
    request: SkinFavoriteRequest,
) -> Result<Vec<crate::cosmetics::SkinPreset>, CommandError> {
    super::set_skin_favorite(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn skin_preset_preview(
    app: tauri::AppHandle,
    request: PresetIdRequest,
) -> Result<crate::auth::avatar::HeadAvatar, CommandError> {
    super::skin_preset_preview(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn import_skin_preset(
    app: tauri::AppHandle,
    request: ImportSkinPresetRequest,
) -> Result<SkinPresetImportDto, CommandError> {
    super::import_skin_preset(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn update_skin_preset(
    app: tauri::AppHandle,
    request: UpdateSkinPresetRequest,
) -> Result<Vec<crate::cosmetics::SkinPreset>, CommandError> {
    super::update_skin_preset(Host::from_tauri(&app), request)
}

#[tauri::command]
pub fn skin_preset_thumbnail(
    app: tauri::AppHandle,
    request: PresetIdRequest,
) -> Result<Option<PresetThumbnailDto>, CommandError> {
    super::skin_preset_thumbnail(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn save_current_skin(
    app: tauri::AppHandle,
    request: SaveCurrentSkinRequest,
) -> Result<SkinPresetImportDto, CommandError> {
    super::save_current_skin(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn remove_skin_preset(
    app: tauri::AppHandle,
    request: PresetIdRequest,
) -> Result<(), CommandError> {
    super::remove_skin_preset(Host::from_tauri(&app), request)
}

#[tauri::command]
pub async fn apply_skin_preset(
    app: tauri::AppHandle,
    request: ApplySkinPresetRequest,
) -> Result<crate::cosmetics::CosmeticsState, CommandError> {
    super::apply_skin_preset(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn select_cape(
    app: tauri::AppHandle,
    request: SelectCapeRequest,
) -> Result<crate::cosmetics::CosmeticsState, CommandError> {
    super::select_cape(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn disable_cape(
    app: tauri::AppHandle,
    request: AccountIdRequest,
) -> Result<crate::cosmetics::CosmeticsState, CommandError> {
    super::disable_cape(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn quick_play_history(
    app: tauri::AppHandle,
    request: QuickPlayRequest,
) -> Result<LaunchProcessDto, CommandError> {
    super::quick_play_history(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn get_play_readiness(
    app: tauri::AppHandle,
    request: LaunchReadinessRequest,
) -> Result<PlayReadinessDto, CommandError> {
    super::get_play_readiness(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn get_launch_state(
    request: ValidateInstalledGameRequest,
) -> Result<LaunchProcessDto, CommandError> {
    super::get_launch_state(request)
}

#[tauri::command]
pub async fn play_instance(
    app: tauri::AppHandle,
    request: PlayRequest,
) -> Result<LaunchProcessDto, CommandError> {
    super::play_instance(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn startup_update_check(
    app: tauri::AppHandle,
) -> Result<UpdateOverviewDto, CommandError> {
    super::startup_update_check(Host::from_tauri(&app)).await
}

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateOverviewDto, CommandError> {
    super::check_for_updates(Host::from_tauri(&app)).await
}

#[tauri::command]
pub fn get_update_overview(app: tauri::AppHandle) -> Result<UpdateOverviewDto, CommandError> {
    super::get_update_overview(Host::from_tauri(&app))
}

#[tauri::command]
pub async fn preview_client_update(
    app: tauri::AppHandle,
    request: PreviewClientUpdateRequest,
) -> Result<crate::updates::client::ClientUpdatePreview, CommandError> {
    super::preview_client_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn apply_client_update(
    app: tauri::AppHandle,
    request: ApplyClientUpdateRequest,
) -> Result<InstanceSummary, CommandError> {
    super::apply_client_update(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn launcher_update_download(
    app: tauri::AppHandle,
    request: LauncherUpdateDownloadRequest,
) -> Result<launcher_updates::LauncherUpdateStatus, CommandError> {
    super::launcher_update_download(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub async fn launcher_update_install(
    app: tauri::AppHandle,
    request: LauncherUpdateInstallRequest,
) -> Result<launcher_updates::LauncherUpdateStatus, CommandError> {
    super::launcher_update_install(Host::from_tauri(&app), request).await
}

#[tauri::command]
pub fn get_instance_sync_conflicts(
    app: tauri::AppHandle,
) -> Result<Vec<crate::instance_sync::Conflict>, super::CommandError> {
    super::get_instance_sync_conflicts(Host::from_tauri(&app))
}
#[tauri::command]
pub async fn resolve_instance_sync_conflict(
    app: tauri::AppHandle,
    request: super::SyncResolveRequest,
) -> Result<crate::instance_sync::Receipt, super::CommandError> {
    let host = Host::from_tauri(&app);
    tokio::task::spawn_blocking(move || super::resolve_instance_sync_conflict(host, request))
        .await
        .map_err(|_| {
            super::CommandError::new("sync_worker_failed", "The sync worker did not complete.")
        })?
}
