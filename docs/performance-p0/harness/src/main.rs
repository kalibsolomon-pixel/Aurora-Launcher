//! Isolated research harness. No install/auth/game-spawn APIs are called.
//! Online plan resolution may populate metadata caches; instance content is read-only.
use aurora_launcher_lib as a;
use serde_json::json;
use std::{path::PathBuf, time::Instant};

fn row(alias: &str, stage: &str, sample: usize, start: Instant, facts: serde_json::Value) {
    println!(
        "{}",
        json!({"instance":alias,"stage":stage,"sample":sample,
        "milliseconds":start.elapsed().as_secs_f64()*1000.0,"facts":facts})
    );
}
fn main() {
    let trials = std::env::var("AURORA_P0_TRIALS")
        .ok()
        .map(|value| value.parse::<usize>().expect("integer trial count"))
        .unwrap_or(21);
    assert!((2..=100).contains(&trials));
    // Only a platform-derived app-data root is admitted. No user-supplied instance paths.
    let root = if let Some(value) = std::env::var_os("AURORA_P0_BENCHMARK_ROOT") {
        let root = PathBuf::from(value).canonicalize().unwrap();
        let temporary = std::env::temp_dir().canonicalize().unwrap();
        assert!(root.starts_with(temporary));
        assert!(
            root.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("aurora-p0-small-")
        );
        root
    } else {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("Windows app data"))
            .join("com.aurora.launcher")
    };
    let managed = a::paths::ManagedPaths::from_app_local_data_dir(root).unwrap();
    let registry = a::instances::InstanceRegistry::load(&managed.instance_registry_file()).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    for (index, record) in registry.instances().iter().enumerate() {
        if record.state() != a::instances::InstanceState::Ready {
            continue;
        }
        let alias = format!("instance-{}", index + 1);
        let id = record.id();
        println!(
            "{}",
            json!({"instance":alias,"stage":"configuration",
            "minecraft":record.installed().minecraft_version,
            "loader":record.installed().platform.kind(),
            "aurora":record.installed().aurora.as_ref().map(|p|p.version.clone())})
        );
        // First observed call is not called disk-cold. Subsequent trials are warm.
        for sample in 0..trials {
            let start = Instant::now();
            let result = a::install::validate_installed_game(&managed, id);
            let facts = match result {
                Ok(a::install::ValidationOutcome::Installed(v)) => {
                    json!({"valid":v.status==a::install::ValidationStatus::Valid,
                    "files":v.checked_files,"bytes":v.verified_bytes,"problems":v.problems.len()})
                }
                Ok(_) => json!({"installed":false}),
                Err(_) => json!({"error":true}),
            };
            row(&alias, "game-integrity", sample, start, facts);
            let start = Instant::now();
            let result = a::instances::lifecycle::validate_instance(&managed, &registry, id);
            row(
                &alias,
                "complete-instance",
                sample,
                start,
                match result {
                    Ok(v) => json!({"status":v.status.as_str(),"problems":v.problems.len()}),
                    Err(_) => json!({"error":true}),
                },
            );
            let start = Instant::now();
            let result = a::instance_mods::scan(&managed, id);
            row(
                &alias,
                "mods-scan",
                sample,
                start,
                match result {
                    Ok(v) => {
                        json!({"entries":v.entries.len(),"bytes":v.entries.iter().filter_map(|e|e.size_bytes).sum::<u64>(),
                    "missing":v.missing_managed.len()})
                    }
                    Err(_) => json!({"error":true}),
                },
            );
            for (kind, stage) in [
                (
                    a::instance_content::ContentType::ResourcePack,
                    "resourcepacks-scan",
                ),
                (a::instance_content::ContentType::ShaderPack, "shaders-scan"),
            ] {
                let start = Instant::now();
                let result = a::instance_content::scan(&managed, id, kind);
                row(
                    &alias,
                    stage,
                    sample,
                    start,
                    match result {
                        Ok(v) => {
                            json!({"entries":v.entries.len(),"missing":v.missing_managed.len()})
                        }
                        Err(_) => json!({"error":true}),
                    },
                );
            }
        }
        // Network metadata is deliberately separate from local validation. No installs.
        if std::env::var_os("AURORA_P0_ONLINE").is_some() {
            let endpoints = a::instances::lifecycle::InstanceEndpoints::operational().unwrap();
            let runtime_endpoints = a::runtime::metadata::RuntimeMetadataEndpoints::official();
            for sample in 0..5 {
                let start = Instant::now();
                let result = rt.block_on(a::instances::lifecycle::resolve_instance_launch_plans(
                    &managed,
                    &managed.instance_registry_file(),
                    &endpoints,
                    &runtime_endpoints,
                    id,
                ));
                row(
                    &alias,
                    "launch-plan-resolution-including-integrity",
                    sample,
                    start,
                    json!({"ok":result.is_ok()}),
                );
                if let Ok((_, plan)) = result {
                    let start = Instant::now();
                    let result = rt.block_on(a::runtime::install::validate_runtime(
                        &managed,
                        &plan,
                        false,
                        a::runtime::install::DEFAULT_JAVA_DIAGNOSTIC_TIMEOUT,
                    ));
                    row(
                        &alias,
                        "runtime-integrity-no-java-diagnostic",
                        sample,
                        start,
                        match result {
                            Ok(v) => {
                                json!({"status":v.status.as_str(),"files":v.checked_files,"bytes":v.verified_bytes})
                            }
                            Err(_) => json!({"error":true}),
                        },
                    );
                }
            }
        }
    }
    // Positive disk artwork decode cost; read_cached never acquires missing artwork.
    let directory = managed.cache_dir().join("artwork/modrinth");
    if let Ok(entries) = std::fs::read_dir(directory) {
        // PNG-only avoids read_cached's one-time legacy WebP conversion write.
        let ids: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|e| {
                use std::io::Read;
                let mut magic = [0u8; 8];
                std::fs::File::open(e.path()).is_ok_and(|mut f| {
                    f.read_exact(&mut magic).is_ok() && magic == *b"\x89PNG\r\n\x1a\n"
                })
            })
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_owned)
            })
            .collect();
        for sample in 0..trials {
            let start = Instant::now();
            let available = ids
                .iter()
                .filter(|id| a::artwork::read_cached(&managed, id).is_some())
                .count();
            row(
                "shared",
                "artwork-disk-decode-batch",
                sample,
                start,
                json!({"objects":ids.len(),"available":available}),
            );
        }
    }
}
