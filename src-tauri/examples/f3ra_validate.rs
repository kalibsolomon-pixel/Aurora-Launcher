//! Read-only, network-free seven-loader regression on the new desktop root.
use aurora_launcher_lib::{
    instances::{InstanceRegistry, lifecycle},
    paths::ManagedPaths,
};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::args_os().nth(1).ok_or("root required")?);
    let physical = root.canonicalize()?;
    if physical.parent() != Some(std::env::temp_dir().canonicalize()?.as_path())
        || !root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("aurora-f3ra-")
        || std::fs::read(root.join("F3RA-ISOLATED.txt"))? != b"F3RA isolated desktop acceptance\n"
    {
        return Err("not a marked desktop acceptance root".into());
    }
    let managed = ManagedPaths::from_app_local_data_dir(root.join("managed"))?;
    let registry = InstanceRegistry::load(&managed.instance_registry_file())?;
    let selected = std::env::args()
        .nth(2)
        .map(aurora_launcher_lib::instances::InstanceId::new)
        .transpose()?;
    if registry.instances().is_empty()
        || selected
            .as_ref()
            .is_some_and(|id| registry.find(id).is_none())
    {
        return Err("requested validation target is absent".into());
    }
    let mut all_ready = true;
    for record in registry.instances() {
        if selected.as_ref().is_some_and(|id| id != record.id()) {
            continue;
        }
        let start = std::time::Instant::now();
        let report = match lifecycle::validate_instance(&managed, &registry, record.id()) {
            Ok(report) => report,
            Err(error) => {
                println!("id={} validation_error={error}", record.id());
                all_ready = false;
                continue;
            }
        };
        println!(
            "id={} name={} status={:?} problems={:?} elapsed_ms={}",
            record.id(),
            record.display_name(),
            report.status,
            report.problems,
            start.elapsed().as_millis()
        );
        if report.status != lifecycle::InstanceStatus::Ready {
            all_ready = false;
        }
    }
    if !all_ready {
        return Err("one or more instances not deeply Ready".into());
    }
    Ok(())
}
