//! Opt-in native acceptance runner. Never uses the owner's application root.
//! Run only against a marked, direct temporary child created for this phase.
use aurora_launcher_lib::{install, instances, paths::ManagedPaths};
use install::journal::TransactionPoint;
use instances::{InstanceId, InstanceRegistry, lifecycle};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

static OBSERVER: OnceLock<(PathBuf, String)> = OnceLock::new();

fn observe(point: TransactionPoint) {
    let (root, selected) = OBSERVER.get().expect("runner observer initialized");
    let label = format!("{point:?}");
    use std::io::Write;
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join(format!("transitions-{}.jsonl", std::process::id())))
        .unwrap();
    writeln!(log, "{}", serde_json::json!({"point": label,
        "unixMicros": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros()})).unwrap();
    if &label == selected {
        std::fs::write(
            root.join(format!("transition-marker-{}.json", std::process::id())),
            serde_json::json!({
                "pid": std::process::id(), "point": label,
            })
            .to_string(),
        )
        .unwrap();
        // The driver terminates this exact process. No destructor or injected
        // error simulates process death at the native acceptance boundary.
        loop {
            std::thread::park();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let requested_root = PathBuf::from(args.get(1).ok_or("root required")?);
    let root = requested_root.canonicalize()?;
    let temp = std::env::temp_dir().canonicalize()?;
    if root.parent() != Some(temp.as_path())
        || !root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("aurora-f3r-acceptance-")
        || std::fs::read(root.join("f3r-owned-root"))? != b"F3R isolated native acceptance"
    {
        return Err("not a marked F3R acceptance root".into());
    }
    // Verify physical containment above, but keep the ordinary Windows path
    // spelling for Java classpath arguments (Java does not accept \\?\ there).
    let managed = ManagedPaths::from_app_local_data_dir(requested_root.join("managed"))?;
    let command = args.get(2).ok_or("command required")?;
    let started = Instant::now();
    println!("pid={} command={command}", std::process::id());
    if command == "validate-all" {
        let registry = InstanceRegistry::load(&managed.instance_registry_file())?;
        for record in registry.instances() {
            let validation = lifecycle::validate_instance(&managed, &registry, record.id())?;
            println!(
                "id={} name={} status={:?} problems={:?}",
                record.id(),
                record.display_name(),
                validation.status,
                validation.problems
            );
            if validation.status != lifecycle::InstanceStatus::Ready {
                return Err("instance is not Ready".into());
            }
        }
    } else {
        let id = InstanceId::new(args.get(3).ok_or("instance required")?.clone())?;
        OBSERVER
            .set((root.clone(), args.get(4).cloned().unwrap_or_default()))
            .unwrap();
        let faults = install::InstallFaults {
            transition_observer: Some(observe),
            ..Default::default()
        };
        if command == "recover" {
            println!(
                "recovery={:?}",
                install::recover_installation_with_faults(&managed, &id, faults)?
            );
        } else if command == "replace" {
            let executor = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            executor.block_on(async {
                let endpoints = lifecycle::InstanceEndpoints::creation()?;
                let plan = lifecycle::resolve_instance_game_plan(
                    &managed,
                    &managed.instance_registry_file(),
                    &endpoints,
                    &id,
                )
                .await?;
                let runtime = lifecycle::ensure_instance_runtime(
                    &managed,
                    &managed.instance_registry_file(),
                    &endpoints,
                    &aurora_launcher_lib::runtime::metadata::RuntimeMetadataEndpoints::official(),
                    &id,
                    &mut |_| {},
                )
                .await?;
                let context = install::InstallContext::official()
                    .with_processor_java(runtime.diagnostic_executable().to_path_buf());
                install::install_game(
                    &managed,
                    &id,
                    &plan,
                    &context,
                    &mut |event| {
                        if event.phase == install::InstallPhase::Processing {
                            println!("processing={:?}", event.current_item);
                        }
                    },
                    faults,
                )
                .await?;
                Ok::<(), Box<dyn std::error::Error>>(())
            })?;
        } else {
            return Err("unsupported acceptance command".into());
        }
        let registry = InstanceRegistry::load(&managed.instance_registry_file())?;
        let validation = lifecycle::validate_instance(&managed, &registry, &id)?;
        println!(
            "id={id} status={:?} problems={:?}",
            validation.status, validation.problems
        );
        if validation.status != lifecycle::InstanceStatus::Ready {
            return Err("instance is not Ready".into());
        }
    }
    println!("elapsed_ms={}", started.elapsed().as_millis());
    Ok(())
}
