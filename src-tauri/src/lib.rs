pub mod appearance;
mod application;
pub mod artwork;
pub mod aurora;
pub mod auth;
pub mod cache;
pub mod config;
pub mod content_recognition;
pub mod content_updates;
mod cosmetic_image;
pub use cosmetic_image::run_artwork_decoder_if_requested;
pub mod cosmetics;
pub mod discord;
pub mod distribution;
pub mod downloads;
pub mod fabric;
pub mod gameplay_history;
pub mod home_widgets;
pub mod install;
mod installed_artwork;
pub mod instance_content;
pub mod instance_mods;
pub mod instances;
pub mod integrity;
pub mod launch;
pub mod minecraft;
pub mod mod_compatibility;
pub mod modpacks;
pub mod modrinth;
pub mod mrpack;
pub mod neoforge;
pub mod pack_activation;
pub mod pack_state;
pub mod pack_update;
pub mod paths;
mod performance;
pub mod runtime;
pub mod server_enrichment;
pub mod shortcuts;
pub mod updates;
pub mod window_activity;

#[cfg(test)]
mod test_support;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initialization = performance::scope(performance::Event::Initialization);
    eprintln!("[aurora-launcher] starting native backend");

    use tauri::Manager;
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "windows")]
                window.set_decorations(false)?;
                // WebView2 never reports a minimized host window as hidden and
                // its blur can race minimization, so native window state owns
                // pausing decorative playback.
                window_activity::attach(app.handle(), &window);
            }
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
            performance::performance_mark,
            performance::performance_readiness,
            performance::performance_runtime_status,
            performance::performance_play,
            application::get_application_status,
            application::get_launcher_state,
            application::get_home_widgets,
            application::get_playtime_summary,
            application::get_daily_playtime,
            application::get_recent_worlds,
            application::get_recent_servers,
            application::set_recent_server_favorite,
            application::refresh_recent_server_status,
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
            application::plan_neoforge_install,
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
            application::list_neoforge_versions,
            application::select_instance,
            application::open_instance_folder,
            application::get_instance_mods,
            application::set_instance_mod_enabled,
            application::remove_instance_mod,
            application::open_instance_mods_folder,
            application::get_instance_content_context,
            application::get_instance_content,
            application::remove_instance_content,
            application::set_instance_pack_enabled,
            application::open_instance_content_folder,
            application::scan_instance_content,
            application::register_recovered_content,
            application::search_modrinth,
            application::browse_modrinth,
            application::browse_modrinth_tags,
            application::get_modrinth_project,
            application::get_modrinth_project_artwork,
            application::resolve_project_artwork,
            application::get_installed_artwork_identities,
            application::preview_modrinth_pack,
            application::install_modrinth_pack,
            application::check_modpack_update,
            application::preview_modpack_update,
            application::apply_modpack_update,
            application::get_modpack_details,
            application::preview_modrinth_install,
            application::install_modrinth,
            application::quick_install_modrinth,
            application::get_provider_lifecycle,
            application::check_modrinth_update,
            application::preview_modrinth_update,
            application::apply_modrinth_update,
            application::check_instance_updates,
            application::set_provider_update_policy,
            application::preview_modrinth_bulk_update,
            application::apply_modrinth_bulk_update,
            application::preview_provider_removal,
            application::apply_provider_removal,
            application::validate_instance,
            application::delete_instance,
            application::get_instance_runtime_status,
            application::ensure_instance_runtime,
            application::get_accounts,
            application::get_account_avatar,
            application::get_cached_account_avatars,
            application::get_cosmetics,
            application::list_skin_presets,
            application::import_skin_preset,
            application::update_skin_preset,
            application::skin_preset_thumbnail,
            application::save_current_skin,
            application::remove_skin_preset,
            application::apply_skin_preset,
            application::select_cape,
            application::disable_cape,
            application::begin_microsoft_login,
            application::cancel_microsoft_login,
            application::select_account,
            application::remove_account,
            application::refresh_account_session,
            application::get_play_readiness,
            application::get_launch_state,
            application::play_instance,
            application::get_update_overview,
            application::startup_update_check,
            application::check_for_updates,
            application::preview_client_update,
            application::apply_client_update,
            application::launcher_update_download,
            application::launcher_update_install
        ])
        .build(tauri::generate_context!())
        .expect("failed to run Aurora Launcher");
    drop(initialization);
    app.run(|_, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            tauri::async_runtime::block_on(crate::discord::shutdown());
        }
        if matches!(event, tauri::RunEvent::Exit) {
            performance::flush();
        }
    });
}
