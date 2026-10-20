//! Explicit live acceptance of official Client discovery in a new disposable root.
use aurora_launcher_lib::{distribution::github, downloads::DownloadOptions, paths::ManagedPaths};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let existing = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let root = existing.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("aurora-client-discovery-{}", uuid::Uuid::new_v4()))
    });
    if existing.is_none() {
        std::fs::create_dir(&root)?;
    }
    let managed = ManagedPaths::from_app_local_data_dir(root)?;
    let manifest = github::discover(&managed, &DownloadOptions::default())
        .await
        .map_err(|error| std::io::Error::other(error.message()))?;
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    if existing.is_some() {
        let config = aurora_launcher_lib::config::load(&managed.config_file())?
            .ok_or("No launcher configuration")?;
        let id = config
            .selected_instance_id()
            .ok_or("No selected instance")?;
        let endpoints =
            aurora_launcher_lib::instances::lifecycle::InstanceEndpoints::operational()?;
        let preview = aurora_launcher_lib::updates::client::preview_update(
            &managed, &endpoints, id, &manifest,
        )
        .map_err(|error| std::io::Error::other(error.message()))?;
        println!(
            "{}",
            serde_json::json!({"outcome":preview.outcome,"installedVersion":preview.installed_version,"candidateVersion":preview.candidate.map(|candidate|candidate.version),"blockers":preview.blockers})
        );
    } else {
        assert!(!managed.instance_registry_file().exists());
    }
    println!("Verified live GitHub Client discovery; no instance was modified.");
    Ok(())
}
