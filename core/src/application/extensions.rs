//! Typed application commands for 2.6 launcher capabilities:
//! instance duplication, organization, pack export, worlds/backups/datapacks,
//! storage inspection & pruning, Mod Bisect, offline launch, bulk mod ops,
//! launch behavior, instance desktop shortcuts, and Pulsar annotations/widgets.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[cfg(windows)]
use super::shortcut_context_for_management;
use super::{
    CommandError, InstanceSummary, LaunchProcessDto, PlayRequest, emit_launch_phase, managed_paths,
    pulsar_worker,
};
use crate::gameplay_history::WorldSaveId;
use crate::host::Host;

// ---------------------------------------------------------------------------
// Instance duplication & organization
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DuplicateInstanceRequest {
    pub source_instance_id: String,
    pub display_name: String,
    #[serde(default)]
    pub options: crate::instances::duplicate::DuplicateOptions,
}

pub async fn duplicate_instance(
    app: Host,
    request: DuplicateInstanceRequest,
) -> Result<InstanceSummary, CommandError> {
    let managed = managed_paths(&app)?;
    let source = crate::instances::InstanceId::new(request.source_instance_id.trim())?;
    let name = request.display_name;
    let options = request.options;
    let record = tokio::task::spawn_blocking(move || {
        crate::instances::duplicate::duplicate(&managed, &source, &name, &options)
    })
    .await
    .map_err(|_| CommandError::new("instance_duplicate_failed", "Duplication did not complete."))?
    .map_err(|err| CommandError::new(err.0, err.1))?;
    Ok(InstanceSummary::from_record(&record))
}

pub fn get_instance_organization(
    app: Host,
) -> Result<crate::instances::organization::InstanceOrganization, CommandError> {
    let managed = managed_paths(&app)?;
    crate::instances::organization::load(&managed)
        .map_err(|e| CommandError::new("instance_organization_error", e.to_string()))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetInstanceOrganizationRequest {
    pub instance_id: String,
    pub pinned: bool,
    pub tags: Vec<String>,
}

pub fn set_instance_organization(
    app: Host,
    request: SetInstanceOrganizationRequest,
) -> Result<crate::instances::organization::InstanceOrganization, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(request.instance_id.trim())?;
    crate::instances::organization::set_meta(&managed, &id, request.pinned, &request.tags)
        .map_err(|e| CommandError::new("instance_organization_error", e.to_string()))
}

// ---------------------------------------------------------------------------
// Reviewed .mrpack export
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackExportPreviewRequest {
    pub instance_id: String,
    #[serde(default)]
    pub options: crate::pack_export::ExportOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackExportRequest {
    pub instance_id: String,
    #[serde(default)]
    pub options: crate::pack_export::ExportOptions,
    pub destination: String,
}

pub async fn preview_pack_export(
    app: Host,
    request: PackExportPreviewRequest,
) -> Result<crate::pack_export::ExportPreview, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(request.instance_id.trim())?;
    let options = request.options;
    tokio::task::spawn_blocking(move || crate::pack_export::preview_export(&managed, &id, &options))
        .await
        .map_err(|_| CommandError::new("pack_export_failed", "Export preview did not complete."))?
        .map_err(|e| CommandError::new(e.0, e.1))
}

pub async fn export_instance_pack(
    app: Host,
    request: PackExportRequest,
) -> Result<crate::pack_export::ExportReceipt, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(request.instance_id.trim())?;
    let options = request.options;
    let destination = PathBuf::from(request.destination);
    tokio::task::spawn_blocking(move || {
        crate::pack_export::export_mrpack(&managed, &id, &options, &destination)
    })
    .await
    .map_err(|_| CommandError::new("pack_export_failed", "Pack export did not complete."))?
    .map_err(|e| CommandError::new(e.0, e.1))
}

// ---------------------------------------------------------------------------
// Worlds, local backups & datapacks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceWorldDto {
    pub directory: String,
    pub display_name: String,
    pub last_played_ms: u64,
    pub game_mode: Option<String>,
    pub version_name: Option<String>,
    pub size_bytes: u64,
    pub datapack_count: usize,
    pub backup_count: usize,
}

pub async fn list_instance_worlds(
    app: Host,
    instance_id: String,
) -> Result<Vec<InstanceWorldDto>, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    tokio::task::spawn_blocking(move || {
        let mut worlds = crate::saved_worlds::read(&managed, &id).map_err(|_| {
            CommandError::new("worlds_read_failed", "Could not read instance worlds.")
        })?;
        worlds.sort_by(|a, b| b.last_played_ms.cmp(&a.last_played_ms));
        Ok(worlds
            .into_iter()
            .map(|w| {
                let backup_count = crate::world_backups::list_backups(&managed, &id, &w.directory)
                    .map(|v| v.len())
                    .unwrap_or(0);
                InstanceWorldDto {
                    display_name: w
                        .display_name
                        .unwrap_or_else(|| w.directory.as_str().to_owned()),
                    directory: w.directory.as_str().to_owned(),
                    last_played_ms: w.last_played_ms,
                    game_mode: w.game_mode.map(str::to_owned),
                    version_name: w.version_name,
                    size_bytes: w.size_bytes,
                    datapack_count: w.datapack_count,
                    backup_count,
                }
            })
            .collect())
    })
    .await
    .map_err(|_| CommandError::new("worlds_read_failed", "World scan did not complete."))?
}

pub async fn list_world_backups(
    app: Host,
    instance_id: String,
    world_directory: String,
) -> Result<Vec<crate::world_backups::WorldBackupSummary>, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let world = WorldSaveId::parse(world_directory)
        .map_err(|_| CommandError::new("world_invalid", "Invalid world directory name."))?;
    tokio::task::spawn_blocking(move || crate::world_backups::list_backups(&managed, &id, &world))
        .await
        .map_err(|_| CommandError::new("world_backup_failed", "Backup listing did not complete."))?
        .map_err(|e| CommandError::new(e.0, e.1))
}

pub async fn create_world_backup(
    app: Host,
    instance_id: String,
    world_directory: String,
    label: Option<String>,
) -> Result<crate::world_backups::WorldBackupSummary, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let world = WorldSaveId::parse(world_directory)
        .map_err(|_| CommandError::new("world_invalid", "Invalid world directory name."))?;
    tokio::task::spawn_blocking(move || {
        crate::world_backups::create_backup(&managed, &id, &world, label.as_deref())
    })
    .await
    .map_err(|_| CommandError::new("world_backup_failed", "Backup creation did not complete."))?
    .map_err(|e| CommandError::new(e.0, e.1))
}

pub async fn restore_world_backup(
    app: Host,
    instance_id: String,
    world_directory: String,
    backup_id: String,
) -> Result<(), CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let world = WorldSaveId::parse(world_directory)
        .map_err(|_| CommandError::new("world_invalid", "Invalid world directory name."))?;
    tokio::task::spawn_blocking(move || {
        crate::world_backups::restore_backup(&managed, &id, &world, &backup_id)
    })
    .await
    .map_err(|_| CommandError::new("world_backup_failed", "Backup restore did not complete."))?
    .map_err(|e| CommandError::new(e.0, e.1))
}

pub async fn delete_world_backup(
    app: Host,
    instance_id: String,
    world_directory: String,
    backup_id: String,
) -> Result<(), CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let world = WorldSaveId::parse(world_directory)
        .map_err(|_| CommandError::new("world_invalid", "Invalid world directory name."))?;
    tokio::task::spawn_blocking(move || {
        crate::world_backups::delete_backup(&managed, &id, &world, &backup_id)
    })
    .await
    .map_err(|_| CommandError::new("world_backup_failed", "Backup deletion did not complete."))?
    .map_err(|e| CommandError::new(e.0, e.1))
}

pub async fn list_world_datapacks(
    app: Host,
    instance_id: String,
    world_directory: String,
) -> Result<Vec<crate::saved_worlds::DatapackEntry>, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let world = WorldSaveId::parse(world_directory)
        .map_err(|_| CommandError::new("world_invalid", "Invalid world directory name."))?;
    tokio::task::spawn_blocking(move || crate::saved_worlds::list_datapacks(&managed, &id, &world))
        .await
        .map_err(|_| CommandError::new("datapacks_failed", "Datapack scan did not complete."))?
        .map_err(|_| CommandError::new("datapacks_failed", "Could not read world datapacks."))
}

pub fn set_world_datapack_enabled(
    app: Host,
    instance_id: String,
    world_directory: String,
    file_name: String,
    enabled: bool,
) -> Result<(), CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let world = WorldSaveId::parse(world_directory)
        .map_err(|_| CommandError::new("world_invalid", "Invalid world directory name."))?;
    crate::saved_worlds::set_datapack_enabled(&managed, &id, &world, &file_name, enabled)
        .map_err(|msg| CommandError::new("datapacks_failed", msg))
}

pub fn remove_world_datapack(
    app: Host,
    instance_id: String,
    world_directory: String,
    file_name: String,
) -> Result<(), CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let world = WorldSaveId::parse(world_directory)
        .map_err(|_| CommandError::new("world_invalid", "Invalid world directory name."))?;
    crate::saved_worlds::remove_datapack(&managed, &id, &world, &file_name)
        .map_err(|msg| CommandError::new("datapacks_failed", msg))
}

pub async fn browse_datapacks(
    app: Host,
    instance_id: String,
    query: String,
    offset: u32,
) -> Result<crate::modrinth::SearchPage, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())?;
    let record = registry
        .find(&id)
        .ok_or_else(|| CommandError::new("instance_not_found", "Instance not found."))?;
    let context = crate::modrinth::Context {
        minecraft_version: record.installed().minecraft_version.clone(),
        loader: "datapack".into(),
        fabric_api_protected: false,
    };
    let client = crate::modrinth::Client::official();
    client
        .search_browse(
            &context,
            crate::modrinth::BrowseKind::DataPack,
            &query,
            &[],
            crate::modrinth::BrowseSort::Relevance,
            offset,
        )
        .await
        .map_err(|e| CommandError::new(e.code(), e.to_string()))
}

pub async fn install_world_datapack(
    app: Host,
    instance_id: String,
    world_directory: String,
    project_id: String,
) -> Result<crate::saved_worlds::DatapackEntry, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let world = WorldSaveId::parse(world_directory)
        .map_err(|_| CommandError::new("world_invalid", "Invalid world directory name."))?;
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())?;
    let record = registry
        .find(&id)
        .ok_or_else(|| CommandError::new("instance_not_found", "Instance not found."))?;
    let context = crate::modrinth::Context {
        minecraft_version: record.installed().minecraft_version.clone(),
        loader: "datapack".into(),
        fabric_api_protected: false,
    };
    let client = crate::modrinth::Client::official();
    let details = client
        .details_browse(&context, crate::modrinth::BrowseKind::DataPack, &project_id)
        .await
        .map_err(|e| CommandError::new(e.code(), e.to_string()))?;
    let version_id = details.default_version_id.ok_or_else(|| {
        CommandError::new(
            "datapack_incompatible",
            "No compatible datapack version was found for this Minecraft version.",
        )
    })?;
    let artifact = client
        .datapack_artifact(&project_id, &version_id)
        .await
        .map_err(|e| CommandError::new(e.code(), e.to_string()))?;
    let cache = crate::cache::ArtifactCache::new(managed.clone());
    let verified = cache
        .acquire_sha512(&artifact.source)
        .await
        .map_err(|e| CommandError::new("datapack_download_failed", e.to_string()))?;
    let file_name = artifact.file_name;
    let verified_path = verified.path;
    tokio::task::spawn_blocking(move || {
        crate::saved_worlds::install_datapack_from_verified_file(
            &managed,
            &id,
            &world,
            &file_name,
            &verified_path,
        )
    })
    .await
    .map_err(|_| CommandError::new("datapacks_failed", "Datapack install did not complete."))?
    .map_err(|msg| CommandError::new("datapacks_failed", msg))
}

// ---------------------------------------------------------------------------
// Storage & Cache Manager
// ---------------------------------------------------------------------------

pub async fn inspect_storage(app: Host) -> Result<crate::storage::StorageOverview, CommandError> {
    let managed = managed_paths(&app)?;
    tokio::task::spawn_blocking(move || crate::storage::inspect_storage(&managed))
        .await
        .map_err(|_| CommandError::new("storage_failed", "Storage inspection did not complete."))?
        .map_err(|e| CommandError::new(e.0, e.1))
}

pub async fn prune_storage(
    app: Host,
    options: crate::storage::PruneOptions,
) -> Result<crate::storage::PruneReport, CommandError> {
    let managed = managed_paths(&app)?;
    tokio::task::spawn_blocking(move || crate::storage::prune_storage(&managed, &options))
        .await
        .map_err(|_| CommandError::new("storage_failed", "Storage prune did not complete."))?
        .map_err(|e| CommandError::new(e.0, e.1))
}

// ---------------------------------------------------------------------------
// Built-in Mod Bisect Crash Troubleshooter
// ---------------------------------------------------------------------------

pub fn get_mod_bisect(
    app: Host,
    instance_id: String,
) -> Result<Option<crate::mod_bisect::BisectView>, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    crate::mod_bisect::load(&managed, &id)
        .map(|opt| opt.map(|s| s.to_view()))
        .map_err(|e| CommandError::new(e.0, e.1))
}

pub fn start_mod_bisect(
    app: Host,
    instance_id: String,
) -> Result<crate::mod_bisect::BisectView, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    crate::mod_bisect::start(&managed, &id)
        .map(|s| s.to_view())
        .map_err(|e| CommandError::new(e.0, e.1))
}

pub fn record_mod_bisect_outcome(
    app: Host,
    instance_id: String,
    crashed: bool,
) -> Result<crate::mod_bisect::BisectView, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    crate::mod_bisect::record_outcome(&managed, &id, crashed)
        .map(|s| s.to_view())
        .map_err(|e| CommandError::new(e.0, e.1))
}

pub fn undo_mod_bisect(
    app: Host,
    instance_id: String,
) -> Result<crate::mod_bisect::BisectView, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    crate::mod_bisect::undo(&managed, &id)
        .map(|s| s.to_view())
        .map_err(|e| CommandError::new(e.0, e.1))
}

pub fn finish_mod_bisect(
    app: Host,
    instance_id: String,
    keep_culprit_disabled: bool,
) -> Result<(), CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    crate::mod_bisect::finish(&managed, &id, keep_culprit_disabled)
        .map_err(|e| CommandError::new(e.0, e.1))
}

// ---------------------------------------------------------------------------
// Launch Behavior, Instance Shortcuts, Bulk Mod Ops & Quick Diagnostic Fixes
// ---------------------------------------------------------------------------

pub fn on_launch_behavior(
    app: Host,
    behavior: Option<crate::config::OnLaunchBehavior>,
) -> Result<crate::config::OnLaunchBehavior, CommandError> {
    let paths = managed_paths(&app)?;
    let _guard = crate::instances::lifecycle::registry_lock();
    let mut config = crate::config::load(&paths.config_file())?.unwrap_or_default();
    if let Some(behavior) = behavior {
        config.set_on_launch_behavior(behavior);
        crate::config::save(&paths.config_file(), &config)?;
    }
    Ok(config.on_launch_behavior())
}

pub fn create_instance_desktop_shortcut(
    app: Host,
    instance_id: String,
) -> Result<String, CommandError> {
    #[cfg(windows)]
    {
        let managed = managed_paths(&app)?;
        let id = crate::instances::InstanceId::new(instance_id.trim())?;
        let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())?;
        let record = registry
            .find(&id)
            .ok_or_else(|| CommandError::new("instance_not_found", "Instance not found."))?;
        let context = shortcut_context_for_management()?;
        let path = context
            .create_instance_desktop_shortcut(id.as_str(), record.display_name())
            .map_err(CommandError::from)?;
        Ok(path.display().to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (app, instance_id);
        Err(CommandError::from(
            crate::shortcuts::ShortcutError::UnsupportedPlatform,
        ))
    }
}

pub fn list_active_launches() -> Vec<LaunchProcessDto> {
    crate::launch::process::active_snapshots()
        .into_iter()
        .map(LaunchProcessDto::from)
        .collect()
}

pub fn set_mods_enabled_bulk(
    app: Host,
    instance_id: String,
    entry_ids: Vec<String>,
    enabled: bool,
) -> Result<usize, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let mut changed = 0usize;
    for entry_id in entry_ids {
        if crate::instance_mods::set_enabled(&managed, &id, &entry_id, enabled).is_ok() {
            changed += 1;
        }
    }
    Ok(changed)
}

pub fn remove_mods_bulk(
    app: Host,
    instance_id: String,
    entry_ids: Vec<String>,
) -> Result<usize, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let mut removed = 0usize;
    for entry_id in entry_ids {
        if crate::instance_mods::remove(&managed, &id, &entry_id).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

pub fn open_instance_config_folder(app: Host, instance_id: String) -> Result<(), CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let dir = managed.instance_paths(&id).config().to_path_buf();
    std::fs::create_dir_all(&dir)
        .map_err(|e| CommandError::new("open_folder_failed", e.to_string()))?;
    crate::host::open_path(&dir)
        .map_err(|_| CommandError::new("open_folder_failed", "Could not open the config folder."))
}

/// Quick one-click action for OutOfMemoryError diagnosis: increases memory by
/// `delta_mib` (clamped to `MAX_MEMORY_MIB`) and persists atomically.
pub fn increase_instance_memory_quick(
    app: Host,
    instance_id: String,
    delta_mib: u32,
) -> Result<InstanceSummary, CommandError> {
    let managed = managed_paths(&app)?;
    let id = crate::instances::InstanceId::new(instance_id.trim())?;
    let endpoints = crate::instances::lifecycle::InstanceEndpoints::operational()
        .map_err(|error| CommandError::new("aurora_manifest_invalid", error.to_string()))?;
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())?;
    let existing = registry
        .find(&id)
        .ok_or_else(|| CommandError::new("instance_not_found", "Instance not found."))?;
    let mut next = existing.configuration().clone();
    let new_mem = next
        .memory_mib()
        .saturating_add(delta_mib)
        .min(crate::instances::settings::MAX_MEMORY_MIB);
    next.set_memory_mib(new_mem);
    let record = crate::instances::lifecycle::update_instance_configuration_checked(
        &managed.instance_registry_file(),
        &endpoints,
        &id,
        next,
        Some(existing.configuration()),
    )?;
    Ok(InstanceSummary::from_record(&record))
}

// ---------------------------------------------------------------------------
// Explicit Offline Launch (using verified cached plan receipt)
// ---------------------------------------------------------------------------

pub async fn launch_instance_offline(
    app: Host,
    request: PlayRequest,
) -> Result<LaunchProcessDto, CommandError> {
    let managed = managed_paths(&app)?;
    let instance = crate::instances::InstanceId::new(request.instance_id.trim())?;
    let account_id = request.account_id.trim().to_owned();
    if account_id.is_empty() {
        return Err(CommandError::new(
            "launch_authentication_required",
            "Select a previously signed-in Minecraft account for offline play.",
        ));
    }
    let _preparation = crate::launch::process::PreparationGuard::acquire(instance.as_str())?;

    emit_launch_phase(&app, "validatingInstance");
    let prepared = crate::launch::boundary::LaunchSnapshot::capture(&managed, &instance)?;
    let record = prepared.record().clone();
    let receipt = crate::launch::offline::load_verified_receipt(&managed, &instance).ok_or_else(
        || {
            CommandError::new(
                "launch_offline_unavailable",
                "This instance has not been launched online yet, so no verified offline launch plan is cached. Launch once while online first.",
            )
        },
    )?;

    let accounts = crate::auth::accounts::load(&managed.accounts_file())
        .map_err(|e| CommandError::new("launch_authentication_required", e.to_string()))?;
    let account = accounts.find(&account_id).ok_or_else(|| {
        CommandError::new(
            "launch_authentication_required",
            "The selected Minecraft account was not found.",
        )
    })?;
    let offline_session = crate::auth::MinecraftSession::new(
        account.account_id(),
        crate::auth::MinecraftProfile::new(account.account_id(), account.minecraft_name()),
        crate::auth::credentials::SecretString::new("0"),
        std::time::Duration::from_secs(3600),
    );

    let configuration = record.configuration();
    configuration
        .validate()
        .map_err(|error| CommandError::new("launch_metadata_invalid", error.to_string()))?;
    let additional_jvm_arguments = configuration
        .validated_jvm_arguments()
        .map_err(|error| CommandError::new("launch_jvm_arguments_invalid", error.to_string()))?;
    let performance_mode = configuration.performance_mode()
        == Some(crate::instances::performance::PerformanceMode::Performance);
    let launch_options = crate::launch::resolve::LaunchOptions::new(
        configuration.memory_mib(),
        additional_jvm_arguments,
        configuration
            .window()
            .map(|window| (window.width(), window.height())),
    )
    .with_fullscreen(configuration.fullscreen())
    .with_eco_qos_opt_out(performance_mode);
    let features = launch_options.feature_profile();

    // Validate local custom Java or local managed Java without network access.
    let custom_java = match configuration.java() {
        Some(java) => Some(
            crate::runtime::external::validate(
                java.custom_path(),
                receipt.launch_plan.required_java_major(),
            )
            .await
            .map_err(|error| CommandError::new("launch_custom_java_invalid", error.message))?,
        ),
        None => None,
    };
    let managed_java = if custom_java.is_some() {
        None
    } else {
        Some(find_local_managed_java(
            &managed,
            &receipt.runtime_component,
        )?)
    };

    emit_launch_phase(&app, "assemblingArguments");
    let snapshot = prepared.with_validated(&managed, |installed| {
        let _custom_java_guard = custom_java
            .as_ref()
            .map(|java| java.lock_for_launch())
            .transpose()
            .map_err(|error| CommandError::new("launch_custom_java_invalid", error.message))?;
        let java = match (&custom_java, &managed_java) {
            (Some(external), _) => crate::launch::resolve::LaunchJava::External(external),
            (None, Some(path)) => crate::launch::resolve::LaunchJava::Managed(path),
            (None, None) => {
                return Err(CommandError::new(
                    "launch_runtime_not_ready",
                    "No validated local Java runtime is available for offline play.",
                ));
            }
        };
        let spec = crate::launch::resolve::resolve_launch_spec_with_java(
            &managed,
            &instance,
            &receipt.launch_plan,
            installed,
            java,
            &offline_session,
            &features,
            &launch_options,
        )?;
        emit_launch_phase(&app, "startingProcess");
        let listener_app = app.clone();
        let snapshot = crate::launch::process::spawn_supervised_with_bridge(
            spec,
            managed.instance_paths(&instance).logs(),
            Arc::new(move |snapshot| {
                let _ = listener_app.emit("launch-state", LaunchProcessDto::from(snapshot));
            }),
            None,
            None,
        )?;
        Ok::<_, CommandError>(snapshot)
    })?;
    Ok(LaunchProcessDto::from(snapshot))
}

fn find_local_managed_java(
    managed: &crate::paths::ManagedPaths,
    component: &str,
) -> Result<PathBuf, CommandError> {
    let comp_dir = managed.runtimes_dir().join(component);
    let entries = std::fs::read_dir(&comp_dir).map_err(|_| {
        CommandError::new(
            "launch_runtime_not_ready",
            format!(
                "Managed Java runtime '{component}' is not installed locally for offline play."
            ),
        )
    })?;
    let exe_name = if cfg!(windows) { "javaw.exe" } else { "java" };
    for entry in entries.flatten() {
        let slot = entry.path();
        if !slot.join("runtime-installed.json").is_file() {
            continue;
        }
        let candidate = slot.join("bin").join(exe_name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(CommandError::new(
        "launch_runtime_not_ready",
        format!("Managed Java runtime '{component}' is missing its local executable."),
    ))
}

// ---------------------------------------------------------------------------
// Pulsar Annotations, Home Widget & Stats Export
// ---------------------------------------------------------------------------

pub async fn set_pulsar_annotation(
    app: Host,
    request: crate::pulsar::view::AnnotationRequest,
) -> Result<crate::pulsar::annotations::FightAnnotation, CommandError> {
    let result = pulsar_worker(app.clone(), move |paths| {
        crate::pulsar::set_annotation(paths, request)
    })
    .await?;
    let _ = app.emit("pulsar-changed", serde_json::json!({"annotated": true}));
    Ok(result)
}

pub async fn get_pulsar_recent_widget(
    app: Host,
    instance_id: Option<String>,
    limit: usize,
) -> Result<crate::pulsar::view::RecentFightsWidgetSummary, CommandError> {
    pulsar_worker(app, move |paths| {
        crate::pulsar::recent_widget_summary(paths, instance_id, limit)
    })
    .await
}

pub async fn export_pulsar_stats(
    app: Host,
    request: crate::pulsar::view::StatsExportRequest,
) -> Result<crate::pulsar::view::ActionReceipt, CommandError> {
    pulsar_worker(app, move |paths| {
        crate::pulsar::export_stats(paths, request)
    })
    .await
}
