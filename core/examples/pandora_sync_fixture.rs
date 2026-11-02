//! Synthetic native UI fixture. No installation, accounts, credentials,
//! game launch or public network. Refuses an existing managed root.
use aurora_launcher_lib::{
    instances::{
        InstanceRecord, InstanceRegistry, InstanceState, generate_instance_id,
        platform::{InstalledConfiguration, PlatformPin},
        settings::InstanceConfiguration,
    },
    paths::ManagedPaths,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        std::path::PathBuf::from(std::env::args_os().nth(1).ok_or("explicit root required")?);
    if !root.is_absolute()
        || !root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("pandora-ui-")
        || root.exists()
    {
        return Err("requires a new absolute pandora-ui-* disposable root".into());
    }
    std::fs::create_dir(&root)?;
    std::fs::write(
        root.join("PANDORA-DISPOSABLE.txt"),
        b"Synthetic UI fixture: no installed game or accounts.\n",
    )?;
    let managed = ManagedPaths::from_app_local_data_dir(root.join("managed"))?;
    let mut registry = InstanceRegistry::empty();
    for (name, value) in [("Sync source", "0.75"), ("Sync destination", "0.25")] {
        let id = generate_instance_id();
        let mut configuration = InstanceConfiguration::for_minecraft_version("1.21.11");
        configuration.set_aurora_enabled(false);
        let record = InstanceRecord::from_installed(
            id.clone(),
            name,
            InstanceState::Ready,
            InstalledConfiguration {
                minecraft_version: "1.21.11".into(),
                platform: PlatformPin::Vanilla {},
                aurora: None,
            },
            configuration,
        )?;
        registry.instances_mut().push(record);
        let instance = managed.instance_paths(&id);
        std::fs::create_dir_all(instance.root())?;
        std::fs::write(
            instance.root().join("options.txt"),
            format!(
                "version:1\nmouseSensitivity:{value}\nkey_jump:key.keyboard.space\nresourcePacks:[]\n"
            ),
        )?;
    }
    registry.save(&managed.instance_registry_file())?;
    println!("{}", root.display());
    Ok(())
}
