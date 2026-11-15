//! Offline showcase fixture. Requires a marked, otherwise uninitialized root.
//! Never installs or launches a game, provisions Java, or contacts a server.
use aurora_launcher_lib::{
    config::{self, LauncherConfig},
    instances::{
        InstanceRecord, InstanceRegistry, InstanceState, generate_instance_id,
        platform::{InstalledConfiguration, PlatformPin},
        settings::InstanceConfiguration,
    },
    paths::ManagedPaths,
    saved_servers::editor::{self, Edit, ResourcePacks, ServerInput},
};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Expected disposable fixture root")?,
    )
    .canonicalize()?;
    if !fs::read(root.join("AURORA_DISPOSABLE_DIAGNOSTIC"))?.starts_with(b"Disposable")
        || root.join("launcher").exists()
        || root.join("instances").exists()
    {
        return Err(
            "Requires a marked disposable root with no existing launcher or instances".into(),
        );
    }
    let managed = ManagedPaths::from_app_local_data_dir(root)?;
    let id = generate_instance_id();
    fs::create_dir_all(managed.instance_paths(&id).root())?;
    let mut configuration = InstanceConfiguration::for_minecraft_version("1.21.11");
    configuration.set_aurora_enabled(false);
    let mut registry = InstanceRegistry::empty();
    registry
        .instances_mut()
        .push(InstanceRecord::from_installed(
            id.clone(),
            "Server editor showcase",
            InstanceState::Ready,
            InstalledConfiguration {
                minecraft_version: "1.21.11".into(),
                platform: PlatformPin::Vanilla {},
                aurora: None,
            },
            configuration,
        )?);
    registry.save(&managed.instance_registry_file())?;
    let mut config = LauncherConfig::default();
    config.set_selected_instance_id(Some(id.clone()));
    config::save(&managed.config_file(), &config)?;
    for (name, address, resource_packs) in [
        (
            "Practice",
            "practice.example.invalid",
            ResourcePacks::Prompt,
        ),
        (
            "Tournament",
            "tournament.example.invalid:25566",
            ResourcePacks::Enabled,
        ),
        (
            "Survival",
            "survival.example.invalid",
            ResourcePacks::Disabled,
        ),
    ] {
        let list = editor::load(&managed, &id).map_err(|e| e.message())?;
        editor::apply(
            &managed,
            &id,
            &list.revision,
            &Edit::Add {
                server: ServerInput {
                    name: name.into(),
                    address: address.into(),
                    resource_packs,
                },
            },
        )
        .map_err(|e| e.message())?;
    }
    println!(
        "Disposable server-editor fixture: {}",
        managed.data_root().display()
    );
    Ok(())
}
