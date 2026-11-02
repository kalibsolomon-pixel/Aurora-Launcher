//! Offline acceptance probe, deliberately compatible with the pre-initiative
//! core so the exact same harness can measure the pinned baseline source.
use std::path::Path;
use std::time::Instant;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let started = Instant::now();
    match args.get(1).map(String::as_str) {
        Some("archive") => {
            let path = Path::new(args.get(2).expect("explicit fixture path required"));
            assert!(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("pandora-bench-")
            );
            for _ in 0..5 {
                let plan = aurora_launcher_lib::mrpack::parse_verified_file(path).unwrap();
                assert_eq!(plan.overrides.len(), 1);
                assert_eq!(plan.overrides[0].size, 32 * 1024 * 1024);
                std::hint::black_box(plan);
            }
        }
        Some("hosts") => {
            for index in 0..50_000 {
                drop(
                    aurora_launcher_lib::transfer::permit(&format!("disposable-{index}.invalid"))
                        .await,
                );
            }
        }
        _ => panic!("archive <pandora-bench-*.mrpack> | hosts"),
    }
    println!(
        "{{\"elapsedMs\":{}}}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    // Let the external process-metrics sampler observe steady memory after
    // the operation. No network, credentials or instance paths are involved.
    std::thread::sleep(std::time::Duration::from_millis(500));
}
