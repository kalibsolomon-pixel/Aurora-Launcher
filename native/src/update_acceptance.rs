//! Absent from distribution: exact compile-time loopback, test key and root.
use crate::{AppWindow, Shell, api, ui};
use aurora_launcher_lib::host::Host;
use slint::ComponentHandle;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("AURORA_NATIVE_ACCEPTANCE_ROOT"))
}
pub fn record_progress(name: &str, payload: &serde_json::Value) {
    if name == "launcher-update-progress" {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root().join("progress.jsonl"))
            .expect("acceptance progress");
        writeln!(file, "{payload}").expect("acceptance progress");
    }
}
pub fn start(window: &AppWindow, host: Host) {
    window.set_window_title("Aurora local signed-update acceptance".into());
    ui::spawn(
        async move {
            let state = api::launcher_state(host.clone()).await?;
            let version = host.version();
            if version == "1.4.3" {
                let receipt = serde_json::json!({"version":version,"pid":std::process::id(),"executable":std::env::current_exe().unwrap(),"dataRoot":host.local_data_dir().unwrap(),"stateLoaded":true});
                std::fs::write(
                    root().join("restarted-ready.json"),
                    serde_json::to_vec_pretty(&receipt).unwrap(),
                )
                .unwrap();
                return Ok(state);
            }
            let discovered = aurora_launcher_lib::updates::native::check(&host).await;
            if !matches!(discovered, aurora_launcher_lib::updates::UpdateAvailability::UpdateAvailable { ref candidate,.. } if candidate=="1.4.3")
            {
                return Err(api::ApiError::new(
                    "acceptance_discovery_failed",
                    "Local signed candidate not discovered",
                ));
            }
            std::fs::write(
                root().join("discovered.json"),
                serde_json::to_vec(&discovered).unwrap(),
            )
            .unwrap();
            api::download_launcher_update(host.clone(), "1.4.3".into()).await?;
            api::install_launcher_update(host, "1.4.3".into()).await?;
            std::fs::write(
                root().join("handoff.json"),
                b"verified installer spawned; parent exiting",
            )
            .unwrap();
            Ok(state)
        },
        |result| {
            match result {
                Ok(state) => {
                    crate::instances::apply_state(state);
                    ui::with(|w, _| w.global::<Shell>().set_loaded(true));
                }
                Err(error) => {
                    std::fs::write(
                        root().join("failed.json"),
                        serde_json::json!({"code":error.code,"message":error.message}).to_string(),
                    )
                    .unwrap();
                }
            }
            slint::Timer::single_shot(std::time::Duration::from_millis(500), || {
                let _ = slint::quit_event_loop();
            });
        },
    );
}
