pub mod appearance;
mod application;
pub mod aurora;
pub mod auth;
pub mod cache;
pub mod config;
pub mod discord;
pub mod distribution;
pub mod downloads;
pub mod fabric;
pub mod gameplay_history;
pub mod home_widgets;
pub mod install;
pub mod instance_content;
pub mod instance_mods;
pub mod instances;
pub mod integrity;
pub mod launch;
pub mod minecraft;
pub mod mod_compatibility;
pub mod modrinth;
pub mod paths;
pub mod runtime;
pub mod shortcuts;

#[cfg(test)]
mod test_support;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("[aurora-launcher] starting native backend");

    use tauri::Manager;
    let app = tauri::Builder::default()
        .setup(|app| {
            // Optional integration failure never changes startup or Play authority.
            if let Ok(root) = app.path().app_local_data_dir() {
                if let Ok(paths) = crate::paths::ManagedPaths::from_app_local_data_dir(root) {
                    if let Ok(config) = crate::config::load(&paths.config_file()) {
                        crate::discord::initialize(
                            app.handle().clone(),
                            config.unwrap_or_default().discord().clone(),
                        );
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            application::get_application_status,
            application::get_launcher_state,
            application::get_home_widgets,
            application::get_playtime_summary,
            application::get_daily_playtime,
            application::get_recent_worlds,
            application::get_recent_servers,
            application::quick_play_history,
            application::set_home_widgets,
            application::reset_home_widgets,
            application::get_discord_state,
            application::connect_discord,
            application::set_discord_preferences,
            application::get_appearance,
            application::set_appearance,
            application::get_desktop_integration,
            application::create_desktop_shortcut,
            application::remove_desktop_shortcut,
            application::acquire_artifact,
            application::plan_minecraft_install,
            application::plan_fabric_install,
            application::install_game,
            application::validate_installed_game,
            application::list_aurora_releases,
            application::create_instance,
            application::get_aurora_compatibility,
            application::preview_aurora_transition,
            application::apply_aurora_transition,
            application::retry_instance_install,
            application::rename_instance,
            application::update_instance_configuration,
            application::install_instance_configuration,
            application::list_minecraft_versions,
            application::list_fabric_loader_versions,
            application::select_instance,
            application::open_instance_folder,
            application::get_instance_mods,
            application::set_instance_mod_enabled,
            application::remove_instance_mod,
            application::open_instance_mods_folder,
            application::get_instance_content_context,
            application::get_instance_content,
            application::remove_instance_content,
            application::open_instance_content_folder,
            application::search_modrinth,
            application::get_modrinth_project,
            application::get_modrinth_project_artwork,
            application::preview_modrinth_install,
            application::install_modrinth,
            application::quick_install_modrinth,
            application::get_provider_lifecycle,
            application::check_modrinth_update,
            application::preview_modrinth_update,
            application::apply_modrinth_update,
            application::preview_provider_removal,
            application::apply_provider_removal,
            application::validate_instance,
            application::delete_instance,
            application::get_instance_runtime_status,
            application::ensure_instance_runtime,
            application::get_accounts,
            application::get_account_avatar,
            application::begin_microsoft_login,
            application::cancel_microsoft_login,
            application::select_account,
            application::remove_account,
            application::refresh_account_session,
            application::get_play_readiness,
            application::get_launch_state,
            application::play_instance
        ])
        .build(tauri::generate_context!())
        .expect("failed to run Aurora Launcher");
    app.run(|_, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            tauri::async_runtime::block_on(crate::discord::shutdown());
        }
    });
}
