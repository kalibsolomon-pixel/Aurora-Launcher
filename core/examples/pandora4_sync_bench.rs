//! Offline, explicit disposable-root fixture; never opens existing launcher data.
//! Source-compatible with the accepted pre-PANDORA-4 Engine for matched probes.
use aurora_launcher_lib::{
    instance_sync::{self, Category, Policy, automatic::Engine},
    instances::{
        InstanceRecord, InstanceRegistry, InstanceState, generate_instance_id,
        platform::{InstalledConfiguration, PlatformPin},
        settings::InstanceConfiguration,
    },
    paths::ManagedPaths,
};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::args_os().nth(1).ok_or("root required")?);
    let n: usize = std::env::args().nth(2).ok_or("count required")?.parse()?;
    if !root.is_absolute()
        || root.exists()
        || !root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("pandora4-")
        || ![0, 2, 8, 32, 64].contains(&n)
    {
        return Err("new absolute pandora4-* root and count 0/2/8/32/64 required".into());
    }
    std::fs::create_dir(&root)?;
    std::fs::write(
        root.join("PANDORA-DISPOSABLE.txt"),
        b"PANDORA-4 synthetic fixture, no accounts or game installation.\n",
    )?;
    let paths = ManagedPaths::from_app_local_data_dir(root.join("managed"))?;
    let mut registry = InstanceRegistry::empty();
    let mut ids = Vec::new();
    for index in 0..n.max(2) {
        let id = generate_instance_id();
        let mut config = InstanceConfiguration::for_minecraft_version("1.21.11");
        config.set_aurora_enabled(false);
        registry
            .instances_mut()
            .push(InstanceRecord::from_installed(
                id.clone(),
                &format!("Synthetic {index}"),
                InstanceState::Ready,
                InstalledConfiguration {
                    minecraft_version: "1.21.11".into(),
                    platform: PlatformPin::Vanilla {},
                    aurora: None,
                },
                config,
            )?);
        let dir = paths.instance_paths(&id).root().join("resourcepacks");
        std::fs::create_dir_all(&dir)?;
        for file in 0..32 {
            std::fs::write(
                dir.join(format!("small-{file:02}.zip")),
                vec![file as u8; 1024],
            )?;
        }
        std::fs::write(
            paths.instance_paths(&id).root().join("options.txt"),
            b"mouseSensitivity:0.5\nkey_jump:key.keyboard.space\n",
        )?;
        ids.push(id);
    }
    registry.save(&paths.instance_registry_file())?;
    instance_sync::save_policy(
        &paths,
        Policy {
            enabled: n > 0,
            instances: ids.iter().take(n).cloned().collect(),
            categories: vec![
                Category::Settings,
                Category::Controls,
                Category::ResourcePacks,
            ],
            ..Policy::default()
        },
    )?;
    let mut engine = Engine::default();
    let mut emit = |phase: &str| -> Result<(), Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();
        let status = engine.cycle(&paths)?;
        println!(
            "{}",
            serde_json::json!({"phase":phase,"elapsedMicros":start.elapsed().as_micros(),"status":status})
        );
        io::stdout().flush()?;
        Ok(())
    };
    emit("cold")?;
    println!("{{\"ready\":true}}");
    io::stdout().flush()?;
    for line in io::stdin().lock().lines() {
        let phase = line?;
        match phase.as_str() {
            "prepare-edit" => std::fs::write(
                paths
                    .instance_paths(&ids[0])
                    .root()
                    .join("resourcepacks/small-00.zip"),
                b"one sided edit",
            )?,
            "prepare-conflict" => {
                for (i, value) in [
                    (0, b"first edit".as_slice()),
                    (1, b"second edit".as_slice()),
                ] {
                    std::fs::write(
                        paths
                            .instance_paths(&ids[i])
                            .root()
                            .join("resourcepacks/small-01.zip"),
                        value,
                    )?;
                }
            }
            "prepare-copy" => std::fs::write(
                paths
                    .instance_paths(&ids[0])
                    .root()
                    .join("resourcepacks/moderate.zip"),
                vec![17u8; 2 * 1024 * 1024],
            )?,
            "exit" => break,
            "fixture" => {
                println!("{{\"fixture\":true}}");
                io::stdout().flush()?;
                continue;
            }
            "converged" | "repeat" | "edit" | "conflict" | "copy" => {}
            _ => return Err("unknown phase".into()),
        }
        if phase.starts_with("prepare-") {
            println!("{{\"prepared\":true}}");
            io::stdout().flush()?;
            continue;
        }
        emit(&phase)?;
    }
    Ok(())
}
