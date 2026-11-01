// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Aurora Launcher native host (NATIVE initiative).
//!
//! A Slint window over the same Rust launcher core the Tauri/WebView host
//! uses (`aurora_launcher_lib`, built without Tauri). No WebView, browser
//! engine or localhost server is involved; the system browser is used only
//! for Microsoft sign-in, exactly as before.

slint::include_modules!();

mod accounts;
mod activity;
mod api;
mod borealis;
mod chrome;
mod content_import;
mod content_updates;
mod discover;
mod events;
mod home;
mod icons;
mod instances;
#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod layout_acceptance;
mod media;
mod player;
mod preview;
#[cfg(feature = "disposable-data-root")]
mod preview_probe;
mod settings;
mod skin_import;
mod skins;
mod symbols;
mod syncing;
mod theme;
mod ui;
mod widgets;

use aurora_launcher_lib::host::{
    APPLICATION_IDENTIFIER, Host, LAUNCHER_VERSION, platform_local_data_dir,
};

fn main() -> Result<(), slint::PlatformError> {
    // The isolated cosmetic image worker re-executes this binary.
    if aurora_launcher_lib::run_artwork_decoder_if_requested() {
        return Ok(());
    }

    borealis::diagnostic("process-entry");
    select_renderer()?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .thread_name("aurora-worker")
        .enable_all()
        .build()
        .map_err(|error| {
            slint::PlatformError::Other(format!("the async runtime could not start: {error}"))
        })?;

    let window = AppWindow::new()?;
    #[cfg(feature = "disposable-data-root")]
    preview_probe::configure_window(&window);
    let host = Host::native(
        platform_local_data_dir(APPLICATION_IDENTIFIER),
        LAUNCHER_VERSION,
        runtime.handle().clone(),
        |name, payload| {
            let name = name.to_owned();
            let _ = slint::invoke_from_event_loop(move || events::dispatch(&name, payload));
        },
    );
    ui::install(
        window.clone_strong(),
        host.clone(),
        runtime.handle().clone(),
    );

    chrome::install(&window);
    bind_navigation(&window);
    home::bind(&window);
    instances::bind(&window);
    icons::bind(&window);
    widgets::bind(&window);
    activity::bind(&window);
    content_import::bind(&window);
    content_updates::bind(&window);
    discover::bind(&window);
    accounts::bind(&window);
    settings::bind(&window);
    skins::bind(&window);
    window.global::<Shell>().on_dismiss_toast(|| {
        ui::with(|window, _| window.global::<Shell>().set_toast_visible(false));
    });
    window.global::<Shell>().on_retry_load(load);

    // Opt-in local diagnostics: synthetic pixels, no launcher loading,
    // credentials, provider requests, persistence or account mutations.
    #[cfg(feature = "disposable-data-root")]
    if preview_probe::start(&window) {
        window.run()?;
        ui::release();
        runtime.shutdown_timeout(std::time::Duration::from_secs(2));
        return Ok(());
    }

    {
        let _guard = runtime.enter();
        start_discord(&host);
    }
    load();
    window.run()?;
    eprintln!("[aurora-native] window closed; stopping background work");

    // Bounded shutdown: Discord presence is cleared (≤ 3 s inside), then
    // background tasks get 2 s. A supervised game keeps running; it is never
    // terminated by the launcher closing.
    runtime.block_on(aurora_launcher_lib::discord::shutdown());
    eprintln!("[aurora-native] integrations stopped");
    ui::release();
    drop(window);
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    eprintln!("[aurora-native] exited cleanly");
    Ok(())
}

/// The software renderer is the measured default (docs/native-ui/BENCHMARKS.md).
/// `SLINT_BACKEND` still overrides it for diagnostics.
fn select_renderer() -> Result<(), slint::PlatformError> {
    if std::env::var_os("SLINT_BACKEND").is_some() {
        return Ok(());
    }
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()
}

fn bind_navigation(window: &AppWindow) {
    window.global::<Nav>().on_navigate(|page| {
        ui::with(|window, state| {
            let nav = window.global::<Nav>();
            if page == Page::Instances && nav.get_page() == Page::Instances {
                // Activating Instances again returns to the list.
                state.workspace = None;
                nav.set_workspace_open(false);
            }
            nav.set_page(page);
        });
        // Navigation never triggers Play readiness: it deep-verifies installed
        // files, so it runs only when its inputs change (selection, account,
        // installation, content, process exit) or on an explicit refresh.
        match page {
            Page::Discover => discover::on_shown(),
            Page::Settings => settings::load(),
            Page::Skins => skins::on_shown(),
            _ => {}
        }
    });
}

fn start_discord(host: &Host) {
    // Optional integration: failures never affect startup or Play authority.
    let Ok(root) = host.local_data_dir() else {
        return;
    };
    let Ok(paths) = aurora_launcher_lib::paths::ManagedPaths::from_app_local_data_dir(root) else {
        return;
    };
    if let Ok(config) = aurora_launcher_lib::config::load(&paths.config_file()) {
        aurora_launcher_lib::discord::initialize(
            host.clone(),
            config.unwrap_or_default().discord().clone(),
        );
    }
}

/// Startup: status, appearance, then launcher state and accounts together,
/// then Rust's Play decision, then background update discovery.
fn load() {
    let host = ui::host();
    match api::application_status(&host) {
        Ok(status) => {
            ui::with(|window, _| {
                let settings = window.global::<Settings>();
                settings.set_version(status.launcher_version.clone().into());
                settings.set_data_root(status.managed_data_root.clone().into());
                settings.set_renderer(renderer_label().into());
                window
                    .global::<Shell>()
                    .set_version(status.launcher_version.into());
            });
        }
        Err(error) => {
            ui::with(|window, _| {
                window
                    .global::<Shell>()
                    .set_fatal_error(error.message.into())
            });
            return;
        }
    }
    ui::with(|window, _| window.global::<Shell>().set_fatal_error("".into()));

    ui::spawn(api::appearance(host.clone()), |result| {
        if let Ok(appearance) = result {
            settings::apply_appearance(appearance);
        }
    });

    let accounts_host = host.clone();
    ui::spawn(
        async move { tokio::join!(api::launcher_state(host), api::accounts(accounts_host)) },
        |(state, accounts)| {
            match state {
                Ok(state) => {
                    instances::apply_state(state);
                    ui::with(|window, _| window.global::<Shell>().set_loaded(true));
                    borealis::diagnostic("home-state-loaded");
                    widgets::load();
                }
                Err(error) => {
                    ui::with(|window, _| {
                        window
                            .global::<Shell>()
                            .set_fatal_error(error.message.into())
                    });
                    return;
                }
            }
            match accounts {
                Ok(accounts) => accounts::apply(accounts),
                Err(error) => ui::error_toast(&error),
            }
            home::refresh_readiness();
            ui::spawn(api::startup_update_check(ui::host()), |result| {
                if let Ok(overview) = result {
                    settings::apply_updates(&overview);
                }
            });
        },
    );
}

fn renderer_label() -> String {
    match std::env::var("SLINT_BACKEND") {
        Ok(backend) => format!("Slint {backend}"),
        Err(_) => "Slint software renderer".into(),
    }
}
