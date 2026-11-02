//! Explicit live publication diagnostic. Writes verified cache objects only to
//! a new, caller-specified disposable directory; never opens launcher instances.
use aurora_launcher_lib::{
    distribution::ReleaseManifest, downloads::DownloadOptions, paths::ManagedPaths,
    updates::client::fetch_github_release_manifest,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("Provide a new disposable cache directory")?;
    let directory = std::path::PathBuf::from(directory);
    if !directory.is_absolute() || directory.exists() {
        return Err("The probe requires a new absolute directory".into());
    }
    std::fs::create_dir(&directory)?;
    let managed = ManagedPaths::from_app_local_data_dir(directory.clone())?;
    if managed.data_root() != directory {
        return Err("Data-root overrides are not accepted by this probe".into());
    }
    let embedded = ReleaseManifest::from_json(include_str!("../production/aurora-releases.json"))?;
    let installed = embedded
        .resolve_exact("3.0.0", None)
        .ok_or("Missing probe baseline")?;
    let releases = fetch_github_release_manifest(&managed, &DownloadOptions::default(), installed)
        .await
        .map_err(|e| std::io::Error::other(e.message()))?;
    for release in releases.releases() {
        println!(
            "Published Aurora {} · Minecraft {} · Fabric {} · SHA-256 {}",
            release.aurora_version(),
            release.minecraft_version(),
            release.fabric_loader_version(),
            release.artifact().sha256()
        );
    }
    Ok(())
}
