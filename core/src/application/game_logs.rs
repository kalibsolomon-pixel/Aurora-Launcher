//! Typed game-output operations. All paths derive from validated instance IDs.
use super::*;
use crate::launch::logs;

pub fn game_log_preference(app: Host, show: Option<bool>) -> Result<bool, CommandError> {
    let paths = managed_paths(&app)?;
    let _guard = crate::instances::lifecycle::registry_lock();
    let mut config = crate::config::load(&paths.config_file())?.unwrap_or_default();
    if let Some(show) = show {
        config.set_show_game_log(show);
        crate::config::save(&paths.config_file(), &config)?;
    }
    Ok(config.show_game_log())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StopGameRequest {
    pub instance_id: String,
    pub process_id: Option<u32>,
}

pub fn stop_game(request: StopGameRequest) -> Result<(), CommandError> {
    let instance = crate::instances::InstanceId::new(request.instance_id)?;
    crate::launch::process::stop(instance.as_str(), request.process_id)
        .map_err(|m| CommandError::new("game_stop_unavailable", m))
}

pub fn game_log(request: ValidateInstalledGameRequest) -> Result<logs::LogSnapshot, CommandError> {
    let instance = crate::instances::InstanceId::new(request.instance_id)?;
    Ok(logs::session(instance.as_str()).snapshot())
}

pub fn subscribe_game_log(
    request: ValidateInstalledGameRequest,
) -> Result<tokio::sync::watch::Receiver<u64>, CommandError> {
    let instance = crate::instances::InstanceId::new(request.instance_id)?;
    Ok(logs::session(instance.as_str()).subscribe())
}

pub fn diagnose_game_log(
    app: Host,
    request: ValidateInstalledGameRequest,
) -> Result<crate::launch::diagnostics::LaunchFailureDiagnostic, CommandError> {
    use std::io::Read;
    let paths = managed_paths(&app)?;
    let instance = crate::instances::InstanceId::new(request.instance_id)?;
    let snapshot = logs::session(instance.as_str()).snapshot();
    let log = snapshot
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let directory = paths.instance_paths(&instance).root().join("crash-reports");
    let newest = std::fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().extension().is_some_and(|e| e == "txt")
                && entry.file_type().is_ok_and(|t| t.is_file())
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max_by_key(|(time, _)| *time)
        .map(|(_, path)| path);
    let mut crash = Vec::new();
    if let Some(path) = newest {
        if let Ok(file) = std::fs::File::open(path) {
            let _ = file.take(1024 * 1024).read_to_end(&mut crash);
        }
    }
    Ok(crate::launch::diagnostics::diagnose(
        &log,
        &logs::redact(&String::from_utf8_lossy(&crash), &[]),
        None,
    ))
}

pub fn game_log_folders(
    app: Host,
    request: ValidateInstalledGameRequest,
) -> Result<(bool, bool), CommandError> {
    let paths = managed_paths(&app)?;
    let instance = crate::instances::InstanceId::new(request.instance_id)?;
    let root = paths.instance_paths(&instance).root().to_path_buf();
    Ok((
        root.join("logs").is_dir(),
        root.join("crash-reports").is_dir(),
    ))
}

pub fn open_game_log_folder(
    app: Host,
    request: ValidateInstalledGameRequest,
    crash: bool,
) -> Result<(), CommandError> {
    let paths = managed_paths(&app)?;
    let instance = crate::instances::InstanceId::new(request.instance_id)?;
    let folder =
        paths
            .instance_paths(&instance)
            .root()
            .join(if crash { "crash-reports" } else { "logs" });
    if !folder.is_dir() {
        return Err(CommandError::new(
            "game_log_folder_missing",
            "This folder has not been created by the game yet.",
        ));
    }
    crate::host::open_path(&folder)
        .map_err(|_| CommandError::new("folder_open_failed", "The folder could not be opened."))
}

/// The host must obtain explicit confirmation before this command is dispatched.
pub async fn upload_game_log(
    request: ValidateInstalledGameRequest,
    confirmed_public: bool,
) -> Result<logs::UploadResult, CommandError> {
    if !confirmed_public {
        return Err(CommandError::new(
            "game_log_confirmation_required",
            "Confirm that the uploaded log will be public.",
        ));
    }
    let snapshot = game_log(request)?;
    if snapshot.lines.is_empty() {
        return Err(CommandError::new(
            "game_log_empty",
            "There is no game output to upload yet.",
        ));
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok();
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok();
    let upload = logs::UploadRequest::build(&snapshot, home.as_deref(), username.as_deref());
    let client = crate::downloads::build_direct_client(
        Some(std::time::Duration::from_secs(10)),
        None,
        Some(std::time::Duration::from_secs(30)),
        false,
        true,
    );
    logs::upload_using(upload, &client, |request| async {
        let mut response = client
            .execute(request)
            .await
            .map_err(|_| "Could not reach mclo.gs. Your log is still available locally.")?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "The upload response was interrupted.")?
        {
            if bytes.len() + chunk.len() > 64 * 1024 {
                return Err("mclo.gs returned an oversized response.");
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok((status, bytes))
    })
    .await
    .map_err(|m| CommandError::new("game_log_upload_failed", m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_previous_stop_does_not_hide_a_new_preparation() {
        use crate::launch::process::{PreparationGuard, ProcessSnapshot, complete_preparation};
        let id = "game-log-preparation-state";
        let mut stopped = ProcessSnapshot::stopped(id);
        stopped.message = Some("Stopped by you".into());
        complete_preparation(stopped.clone());
        let preparation = PreparationGuard::acquire(id).unwrap();
        let state = || {
            get_launch_state(ValidateInstalledGameRequest {
                instance_id: id.into(),
            })
            .unwrap()
        };
        assert_eq!(state().status, "starting");
        assert!(state().can_stop);
        crate::launch::process::stop(id, None).unwrap();
        complete_preparation(stopped);
        assert_eq!(state().status, "stopped");
        drop(preparation);
        assert!(!state().can_stop);
    }
}
