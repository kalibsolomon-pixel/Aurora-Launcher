pub mod appearance;
pub mod application;
pub mod artwork;
pub mod aurora;
pub mod auth;
pub mod cache;
pub mod config;
pub mod content_recognition;
pub mod content_updates;
mod cosmetic_image;
#[cfg(all(debug_assertions, feature = "f3ra-desktop-acceptance"))]
mod desktop_acceptance;
pub use cosmetic_image::run_artwork_decoder_if_requested;
pub mod cosmetics;
pub mod discord;
pub mod distribution;
pub mod downloads;
pub mod fabric;
pub mod forge;
pub mod game;
pub mod gameplay_history;
pub mod home_widgets;
pub mod host;
pub mod install;
mod installed_artwork;
pub mod instance_content;
pub mod instance_icons;
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
pub mod pack_merge;
pub mod pack_state;
pub mod pack_update;
pub mod paths;
mod performance;
pub mod runtime;
pub mod server_enrichment;
pub mod shortcuts;
pub mod transfer;
pub mod updates;
#[cfg(feature = "tauri-host")]
pub mod window_activity;

#[cfg(test)]
mod test_support;

#[cfg(feature = "tauri-host")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(all(debug_assertions, feature = "f3ra-desktop-acceptance"))]
    desktop_acceptance::authorize().expect("desktop acceptance requires a marked disposable root");
    let initialization = performance::scope(performance::Event::Initialization);
    eprintln!("[aurora-launcher] starting native backend");

    use tauri::Manager;
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let _setup = performance::scope(performance::Event::NativeSetup);
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
                            crate::host::Host::from_tauri(app.handle()),
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
            application::tauri_commands::get_application_status,
            application::tauri_commands::maintain_installation_backups,
            application::tauri_commands::get_launcher_state,
            application::tauri_commands::get_home_widgets,
            application::tauri_commands::get_playtime_summary,
            application::tauri_commands::get_daily_playtime,
            application::tauri_commands::get_recent_worlds,
            application::tauri_commands::get_recent_servers,
            application::tauri_commands::set_recent_server_favorite,
            application::tauri_commands::refresh_recent_server_status,
            application::tauri_commands::quick_play_history,
            application::tauri_commands::set_home_widgets,
            application::tauri_commands::reset_home_widgets,
            application::tauri_commands::get_discord_state,
            application::tauri_commands::connect_discord,
            application::tauri_commands::set_discord_preferences,
            application::tauri_commands::get_appearance,
            application::tauri_commands::set_appearance,
            application::tauri_commands::get_desktop_integration,
            application::tauri_commands::create_desktop_shortcut,
            application::tauri_commands::create_start_menu_shortcut,
            application::tauri_commands::open_launcher_folder,
            application::tauri_commands::remove_desktop_shortcut,
            application::tauri_commands::acquire_artifact,
            application::tauri_commands::plan_minecraft_install,
            application::tauri_commands::plan_fabric_install,
            application::tauri_commands::plan_neoforge_install,
            application::tauri_commands::install_game,
            application::tauri_commands::validate_installed_game,
            application::tauri_commands::list_aurora_releases,
            application::tauri_commands::create_instance,
            application::tauri_commands::get_aurora_compatibility,
            application::tauri_commands::preview_aurora_transition,
            application::tauri_commands::apply_aurora_transition,
            application::tauri_commands::retry_instance_install,
            application::tauri_commands::rename_instance,
            application::tauri_commands::get_instance_icons,
            application::tauri_commands::set_instance_icon,
            application::tauri_commands::update_instance_configuration,
            application::tauri_commands::get_instance_performance_advice,
            application::tauri_commands::install_instance_configuration,
            application::tauri_commands::list_minecraft_versions,
            application::tauri_commands::list_fabric_loader_versions,
            application::tauri_commands::list_neoforge_versions,
            application::tauri_commands::list_forge_versions,
            application::tauri_commands::select_instance,
            application::tauri_commands::open_instance_folder,
            application::tauri_commands::get_instance_mods,
            application::tauri_commands::set_instance_mod_enabled,
            application::tauri_commands::remove_instance_mod,
            application::tauri_commands::open_instance_mods_folder,
            application::tauri_commands::get_instance_content_context,
            application::tauri_commands::get_instance_content,
            application::tauri_commands::remove_instance_content,
            application::tauri_commands::set_instance_pack_enabled,
            application::tauri_commands::open_instance_content_folder,
            application::tauri_commands::scan_instance_content,
            application::tauri_commands::register_recovered_content,
            application::tauri_commands::search_modrinth,
            application::tauri_commands::browse_modrinth,
            application::tauri_commands::browse_modrinth_tags,
            application::tauri_commands::get_modrinth_project,
            application::tauri_commands::get_modrinth_project_artwork,
            application::tauri_commands::resolve_project_artwork,
            application::tauri_commands::get_installed_artwork_identities,
            application::tauri_commands::preview_modrinth_pack,
            application::tauri_commands::install_modrinth_pack,
            application::tauri_commands::preview_modrinth_pack_merge,
            application::tauri_commands::install_modrinth_pack_merge,
            application::tauri_commands::check_modpack_update,
            application::tauri_commands::preview_modpack_update,
            application::tauri_commands::apply_modpack_update,
            application::tauri_commands::get_modpack_details,
            application::tauri_commands::preview_modrinth_install,
            application::tauri_commands::install_modrinth,
            application::tauri_commands::quick_install_modrinth,
            application::tauri_commands::get_provider_lifecycle,
            application::tauri_commands::check_modrinth_update,
            application::tauri_commands::preview_modrinth_update,
            application::tauri_commands::apply_modrinth_update,
            application::tauri_commands::check_instance_updates,
            application::tauri_commands::set_provider_update_policy,
            application::tauri_commands::preview_modrinth_bulk_update,
            application::tauri_commands::apply_modrinth_bulk_update,
            application::tauri_commands::preview_provider_removal,
            application::tauri_commands::apply_provider_removal,
            application::tauri_commands::validate_instance,
            application::tauri_commands::delete_instance,
            application::tauri_commands::get_instance_runtime_status,
            application::tauri_commands::ensure_instance_runtime,
            application::tauri_commands::get_accounts,
            application::tauri_commands::get_account_avatar,
            application::tauri_commands::get_cached_account_avatars,
            application::tauri_commands::get_cosmetics,
            application::tauri_commands::list_skin_presets,
            application::tauri_commands::set_skin_favorite,
            application::tauri_commands::skin_preset_preview,
            application::tauri_commands::import_skin_preset,
            application::tauri_commands::update_skin_preset,
            application::tauri_commands::skin_preset_thumbnail,
            application::tauri_commands::save_current_skin,
            application::tauri_commands::remove_skin_preset,
            application::tauri_commands::apply_skin_preset,
            application::tauri_commands::select_cape,
            application::tauri_commands::disable_cape,
            application::tauri_commands::begin_microsoft_login,
            application::tauri_commands::cancel_microsoft_login,
            application::tauri_commands::select_account,
            application::tauri_commands::remove_account,
            application::tauri_commands::refresh_account_session,
            application::tauri_commands::get_play_readiness,
            application::tauri_commands::get_launch_state,
            application::tauri_commands::play_instance,
            application::tauri_commands::get_update_overview,
            application::tauri_commands::startup_update_check,
            application::tauri_commands::check_for_updates,
            application::tauri_commands::preview_client_update,
            application::tauri_commands::apply_client_update,
            application::tauri_commands::launcher_update_download,
            application::tauri_commands::launcher_update_install
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
