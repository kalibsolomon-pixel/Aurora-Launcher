//! Opt-in live Forge acceptance: creates real Forge instances through the
//! ordinary lifecycle in a disposable root, then (optionally) boots each one
//! to Minecraft's title-screen resources with an offline session.
//!
//! `AURORA_FORGE_LIVE_ROOT` must be a direct child of the OS temp directory
//! named `aurora-forge-live-*`; `AURORA_FORGE_LIVE_PINS` is a comma list of
//! exact Forge pins. `AURORA_FORGE_LIVE_BOOT=1` boots each instance;
//! `AURORA_FORGE_LIVE_MOVER` names a script that keeps the game window on
//! the owner's chosen monitor.
use crate::auth::credentials::SecretString;
use crate::auth::session::{MinecraftProfile, MinecraftSession};
use crate::instances::lifecycle::{self, CreateInstanceRequest, InstanceEndpoints};
use crate::instances::settings::{
    DEFAULT_MEMORY_MIB, InstanceConfiguration, LoaderConfiguration, LoaderKind, LoaderPolicy,
};
use crate::paths::ManagedPaths;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct LiveError(String);

impl From<crate::launch::boundary::BoundaryError> for LiveError {
    fn from(error: crate::launch::boundary::BoundaryError) -> Self {
        Self(error.to_string())
    }
}

fn live_root() -> PathBuf {
    let root = PathBuf::from(std::env::var("AURORA_FORGE_LIVE_ROOT").unwrap());
    std::fs::create_dir_all(&root).unwrap();
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("aurora-forge-live-")
    );
    assert_eq!(
        root.parent().unwrap().canonicalize().unwrap(),
        std::env::temp_dir().canonicalize().unwrap(),
        "the live root must be a direct child of the OS temp directory"
    );
    root
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "downloads and installs real Forge instances into AURORA_FORGE_LIVE_ROOT"]
async fn live_forge_install_and_boot() {
    let root = live_root();
    let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
    let boot = std::env::var("AURORA_FORGE_LIVE_BOOT").is_ok_and(|v| v == "1");
    let pins = std::env::var("AURORA_FORGE_LIVE_PINS").unwrap();
    let mut report = Vec::new();
    for pin in pins.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let started = Instant::now();
        let outcome = install_and_boot(&managed, pin, boot).await;
        let line = match outcome {
            Ok(detail) => format!("PASS {pin} ({}s) {detail}", started.elapsed().as_secs()),
            Err(reason) => format!("FAIL {pin} ({}s) {reason}", started.elapsed().as_secs()),
        };
        eprintln!("{line}");
        report.push(line);
        std::fs::write(root.join("report.txt"), report.join("\n")).unwrap();
    }
    assert!(
        report.iter().all(|line| line.starts_with("PASS")),
        "{}",
        report.join("\n")
    );
}

async fn install_and_boot(managed: &ManagedPaths, pin: &str, boot: bool) -> Result<String, String> {
    let minecraft = pin.split('-').next().unwrap();
    let endpoints = InstanceEndpoints::creation().map_err(|e| e.to_string())?;
    let mut configuration = InstanceConfiguration::from_parts(
        minecraft,
        LoaderConfiguration::from_parts(
            LoaderKind::Forge,
            LoaderPolicy::Pinned {
                version: pin.to_owned(),
            },
        ),
        DEFAULT_MEMORY_MIB,
        String::new(),
        None,
    );
    configuration.set_aurora_enabled(false);
    let mut last_phase = String::new();
    let record = lifecycle::create_instance(
        managed,
        &managed.instance_registry_file(),
        &managed.config_file(),
        &endpoints,
        CreateInstanceRequest::new(format!("Forge {pin}"), configuration),
        &mut |progress| {
            let phase = progress.phase.as_str().to_owned();
            if phase != last_phase {
                eprintln!("  {pin}: {phase}");
                last_phase = phase;
            }
        },
        Default::default(),
    )
    .await
    .map_err(|e| format!("install: {e}"))?;
    let id = record.id().clone();
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|e| e.to_string())?;
    let validation =
        lifecycle::validate_instance(managed, &registry, &id).map_err(|e| e.to_string())?;
    if validation.status != lifecycle::InstanceStatus::Ready {
        return Err(format!("validation: {:?}", validation.problems));
    }
    if !boot {
        return Ok("installed and validated".into());
    }

    let operational = InstanceEndpoints::operational().map_err(|e| e.to_string())?;
    let runtime_endpoints = crate::runtime::metadata::RuntimeMetadataEndpoints::official();
    let (game_plan, _) = lifecycle::resolve_instance_launch_plans(
        managed,
        &managed.instance_registry_file(),
        &operational,
        &runtime_endpoints,
        &id,
    )
    .await
    .map_err(|e| format!("launch plans: {e}"))?;
    let runtime = lifecycle::ensure_instance_runtime(
        managed,
        &managed.instance_registry_file(),
        &operational,
        &runtime_endpoints,
        &id,
        &mut |_| {},
    )
    .await
    .map_err(|e| format!("runtime: {e}"))?;
    let launch_plan = crate::launch::resolve::LaunchPlan::from_game_plan(
        &game_plan,
        crate::minecraft::rules::PlatformProfile::current().unwrap(),
    );
    let session = MinecraftSession::new(
        "00000000000000000000000000000001",
        MinecraftProfile::new("00000000000000000000000000000001", "AuroraForgeTest"),
        SecretString::new("0"),
        Duration::from_secs(3600),
    );
    let options = crate::launch::resolve::LaunchOptions::new(2048, Vec::new(), None);
    let snapshot = crate::launch::boundary::LaunchSnapshot::capture(managed, &id)
        .map_err(|e| e.to_string())?;
    let spec = snapshot
        .with_validated(managed, |installed| {
            crate::launch::resolve::resolve_launch_spec(
                managed,
                &id,
                &launch_plan,
                installed,
                runtime.launch_executable(),
                &session,
                &options.feature_profile(),
                &options,
            )
            .map_err(|e| LiveError(e.to_string()))
        })
        .map_err(|e| format!("launch spec: {}", e.0))?;

    // Exactly the launcher's own argument list (process.rs spawns this).
    let arguments = spec.command_arguments();
    let command_line_chars: usize = arguments.iter().map(|a| a.expose().len() + 3).sum();
    eprintln!("  {pin}: command line ~{command_line_chars} characters");
    let mut command = std::process::Command::new(spec.java_executable());
    command
        .current_dir(spec.working_directory())
        .args(arguments.iter().map(|a| a.expose()))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("spawn: {e}"))?;
    if let Ok(mover) = std::env::var("AURORA_FORGE_LIVE_MOVER") {
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &mover])
            .arg("-TargetPid")
            .arg(child.id().to_string())
            .spawn();
    }
    let log_path = managed
        .instance_paths(&id)
        .root()
        .join(format!("forge-live-{pin}.log"));
    let (sender, receiver) = std::sync::mpsc::channel::<String>();
    for stream in [
        Box::new(child.stdout.take().unwrap()) as Box<dyn std::io::Read + Send>,
        Box::new(child.stderr.take().unwrap()),
    ] {
        let sender = sender.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let _ = sender.send(line);
            }
        });
    }
    drop(sender);
    let deadline = Instant::now() + Duration::from_secs(240);
    let mut log = Vec::new();
    let mut forge_seen = false;
    let result = loop {
        match receiver.recv_timeout(Duration::from_millis(500)) {
            Ok(line) => {
                forge_seen |= line.contains("Forge mod loading")
                    || line.contains("ModLauncher running")
                    || line.contains("FML");
                let done = line.contains("Sound engine started")
                    || line.contains("Created: 1024x")
                    || line.contains("Created: 2048x")
                    || line.contains("Created: 4096x");
                let crashed =
                    line.contains("Crash report saved") || line.contains("#@!@# Game crashed!");
                log.push(line);
                if crashed {
                    break Err("the game crashed".to_owned());
                }
                if done {
                    break if forge_seen {
                        Ok("booted to title resources".to_owned())
                    } else {
                        Err("booted without Forge loading".to_owned())
                    };
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() > deadline {
                    break Err("no title-screen marker within 240s".to_owned());
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break Err(format!("the game exited: {:?}", child.wait().ok()));
            }
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    std::fs::write(&log_path, log.join("\n")).unwrap();
    result.map_err(|reason| format!("boot: {reason} (log {})", log_path.display()))
}
