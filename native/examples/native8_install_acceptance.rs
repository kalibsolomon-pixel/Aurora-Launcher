//! NATIVE-8 live installation acceptance. Runs only against a marked,
//! direct child of the OS temp directory named `aurora-native8-*`. It never
//! signs in, launches Minecraft, or touches the owner's launcher data.
//!
//! Scenarios (each timed, with process-wide transfer counters):
//! 1. Vulkan Optimized 1.21.11-4.1 (Modrinth u76hn1pF / aRFc5ttQ), the pack
//!    that failed with `pack_unsafe_override`: review + new pack instance.
//! 2. The same pack merged into an ordinary Fabric instance.
//! 3. An ordinary Fabric pack (Adrenaline) on the now-warm shared cache.
//! 4. Two cold installs of a different Minecraft version into separate
//!    instances, concurrently (shared-artifact deduplication + permits).
//! 5. Java runtime preparation for two instances needing the same runtime.
#![allow(dead_code)]
#[path = "../src/api.rs"]
mod api;
use aurora_launcher_lib::host::{Host, LAUNCHER_VERSION};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Instant;

const MARKER: &[u8] = b"Native 8 disposable acceptance\n";

fn main() {
    if aurora_launcher_lib::run_artwork_decoder_if_requested() {
        return;
    }
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("disposable root required"),
    );
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("aurora-native8-"),
        "root must be named aurora-native8-*"
    );
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(
        root.parent().unwrap().canonicalize().unwrap(),
        std::env::temp_dir().canonicalize().unwrap(),
        "root must be a direct child of the OS temp directory"
    );
    let marker = root.join("NATIVE8-DISPOSABLE.txt");
    if marker.exists() {
        assert_eq!(std::fs::read(&marker).unwrap(), MARKER);
    } else {
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::write(&marker, MARKER).unwrap();
    }
    let scenarios: Vec<String> = std::env::args().skip(2).collect();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let host = Host::native(
        Ok(root.join("managed")),
        LAUNCHER_VERSION,
        runtime.handle().clone(),
        |_, _| {},
    );
    runtime.block_on(async move {
        let mut results = Vec::new();
        let outcome = run(host, &scenarios, &mut results).await;
        std::fs::write(
            root.join(format!(
                "native8-results-{}.json",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            )),
            serde_json::to_vec_pretty(&results).unwrap(),
        )
        .unwrap();
        if let Err(e) = outcome {
            eprintln!("FAILED {}: {}", e.code, e.message);
            std::process::exit(1);
        }
    });
}

fn stats() -> Value {
    serde_json::to_value(aurora_launcher_lib::transfer::snapshot()).unwrap()
}

fn delta(before: &Value, after: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for (key, value) in after.as_object().unwrap() {
        if key == "activeTransfers" {
            continue;
        }
        out.insert(
            key.clone(),
            json!(value.as_u64().unwrap() - before[key].as_u64().unwrap()),
        );
    }
    Value::Object(out)
}

fn enabled(scenarios: &[String], name: &str) -> bool {
    scenarios.is_empty() || scenarios.iter().any(|s| s == name)
}

async fn timed<T>(
    results: &mut Vec<Value>,
    name: &str,
    future: impl std::future::Future<Output = api::ApiResult<T>>,
    describe: impl FnOnce(&T) -> Value,
) -> api::ApiResult<T> {
    let before = stats();
    let started = Instant::now();
    let outcome = future.await;
    let elapsed = started.elapsed().as_secs_f64();
    let after = stats();
    let mut entry = json!({
        "scenario": name,
        "seconds": (elapsed * 100.0).round() / 100.0,
        "transfers": delta(&before, &after),
    });
    match &outcome {
        Ok(value) => entry["result"] = describe(value),
        Err(e) => entry["error"] = json!({"code": e.code, "message": e.message}),
    }
    println!("{}", serde_json::to_string(&entry).unwrap());
    results.push(entry);
    outcome
}

const VULKAN_PROJECT: &str = "u76hn1pF";
const VULKAN_VERSION: &str = "aRFc5ttQ";

async fn review_and_install(
    host: &Host,
    results: &mut Vec<Value>,
    label: &str,
    merge_into: Option<String>,
    project: &str,
    version: &str,
) -> api::ApiResult<Value> {
    let merge = merge_into.is_some();
    let mut request = json!({"projectId": project, "versionId": version, "selectedOptional": [], "fingerprint": null});
    if let Some(instance) = &merge_into {
        request["instanceId"] = json!(instance);
    }
    let preview = timed(
        results,
        &format!("{label}: review"),
        api::preview_pack(host.clone(), merge, request.clone()),
        |p| {
            let pack = if merge { &p["pack"] } else { p };
            json!({
                "name": pack["name"], "minecraft": pack["minecraftVersion"],
                "loader": pack["fabricLoaderVersion"],
                "recognized": pack["recognized"].as_array().map(Vec::len),
                "unresolved": pack["unresolved"].as_array().map(Vec::len),
                "overrides": pack["overrides"].as_array().map(Vec::len),
                "skippedOverrides": pack["skippedOverrides"],
                "blockers": p.get("blockers"),
            })
        },
    )
    .await?;
    request["fingerprint"] = preview["fingerprint"].clone();
    timed(
        results,
        &format!("{label}: install"),
        api::install_pack(host.clone(), merge, request),
        |v| v.clone(),
    )
    .await
}

async fn fabric_instance(
    host: &Host,
    results: &mut Vec<Value>,
    name: &str,
    minecraft: &str,
    loader: Option<&str>,
) -> api::ApiResult<api::InstanceSummary> {
    let loader_json = match loader {
        Some("vanilla") => json!({"kind": "vanilla"}),
        Some(version) => {
            json!({"kind": "fabric", "policy": {"type": "pinned", "version": version}})
        }
        None => json!({"kind": "fabric", "policy": {"type": "automatic"}}),
    };
    timed(
        results,
        &format!("create {name}"),
        api::create_instance(
            host.clone(),
            name.into(),
            minecraft.into(),
            loader_json,
            false,
        ),
        |i| json!({"id": i.id, "subtitle": i.subtitle()}),
    )
    .await
}

async fn run(host: Host, scenarios: &[String], results: &mut Vec<Value>) -> api::ApiResult<()> {
    if enabled(scenarios, "fabulously") {
        let installed = review_and_install(
            &host,
            results,
            "Fabulously Optimized 12.3.0 new instance",
            None,
            "1KVo5zza",
            "K0lc692U",
        )
        .await?;
        let id = installed["id"].as_str().unwrap_or_default().to_owned();
        timed(
            results,
            "Fabulously Optimized instance validation",
            api::validate_instance(host.clone(), id),
            |v| json!({"status": v.status, "problems": v.problems.len()}),
        )
        .await?;
    }
    if enabled(scenarios, "pack-controls") {
        let state = api::launcher_state(host.clone()).await?;
        let instance = state
            .instances
            .iter()
            .rev()
            .find(|i| i.display_name == "Fabulously Optimized" && i.state == "ready")
            .expect("finished disposable FO instance");
        let id = instance.id.clone();
        let mut memory = instance.configuration.clone();
        memory.memory_mib = 8192;
        api::update_configuration(
            host.clone(),
            id.clone(),
            instance.configuration.clone(),
            memory.clone(),
        )
        .await?;
        let mut invalid = memory.clone();
        invalid.memory_mib = 32769;
        assert!(
            api::update_configuration(host.clone(), id.clone(), memory.clone(), invalid)
                .await
                .is_err()
        );
        api::update_configuration(
            host.clone(),
            id.clone(),
            memory,
            instance.configuration.clone(),
        )
        .await?;
        for failed in state.instances.iter().filter(|i| i.state == "installing") {
            timed(
                results,
                "Delete stopped failed FO installation",
                api::delete_instance(host.clone(), failed.id.clone(), failed.display_name.clone()),
                |_| json!({"deleted":true}),
            )
            .await?;
        }
        let inventory = api::instance_mods(host.clone(), id.clone()).await?;
        let entry = inventory
            .entries
            .iter()
            .find(|e| e.display_name == "BetterGrassify")
            .expect("FO grass component");
        assert!(entry.can_toggle && entry.can_remove);
        let disabled =
            api::set_mod_enabled(host.clone(), id.clone(), entry.entry_id.clone(), false).await?;
        timed(
            results,
            "FO validation after disabling pack mod",
            api::validate_instance(host.clone(), id.clone()),
            |v| json!({"status": v.status, "problems": v.problems.len()}),
        )
        .await?;
        let entry = disabled
            .entries
            .iter()
            .find(|e| e.display_name == "BetterGrassify")
            .unwrap();
        let active =
            api::set_mod_enabled(host.clone(), id.clone(), entry.entry_id.clone(), true).await?;
        let entry = active
            .entries
            .iter()
            .find(|e| e.display_name == "BetterGrassify")
            .unwrap();
        api::remove_mod(host.clone(), id.clone(), entry.entry_id.clone()).await?;
        timed(
            results,
            "FO validation after deleting pack mod",
            api::validate_instance(host.clone(), id),
            |v| json!({"status": v.status, "problems": v.problems.len()}),
        )
        .await?;
    }
    if enabled(scenarios, "vulkan") {
        let installed = review_and_install(
            &host,
            results,
            "Vulkan Optimized new instance",
            None,
            VULKAN_PROJECT,
            VULKAN_VERSION,
        )
        .await?;
        let id = installed["id"].as_str().unwrap_or_default().to_owned();
        timed(
            results,
            "Vulkan Optimized instance validation",
            api::validate_instance(host.clone(), id),
            |v| json!({"status": v.status, "problems": v.problems.len()}),
        )
        .await?;
    }
    if enabled(scenarios, "merge") {
        let target =
            fabric_instance(&host, results, "N8 merge target", "1.21.11", Some("0.19.3")).await?;
        review_and_install(
            &host,
            results,
            "Vulkan Optimized merge",
            Some(target.id.clone()),
            VULKAN_PROJECT,
            VULKAN_VERSION,
        )
        .await?;
        // A second review of the same merge must now report the content as
        // already present rather than writing it again.
        timed(
            results,
            "Vulkan Optimized merge re-review",
            api::preview_pack(
                host.clone(),
                true,
                json!({"instanceId": target.id, "projectId": VULKAN_PROJECT, "versionId": VULKAN_VERSION, "selectedOptional": [], "fingerprint": null}),
            ),
            |p| json!({
                "added": p["added"].as_array().map(Vec::len),
                "alreadyPresent": p["alreadyPresent"].as_array().map(Vec::len),
                "overridesWritten": p["overridesWritten"].as_array().map(Vec::len),
                "blockers": p["blockers"],
            }),
        )
        .await?;
    }
    if enabled(scenarios, "ordinary") {
        let page = api::browse(
            host.clone(),
            String::new(),
            "modpack",
            "Adrenaline".into(),
            0,
        )
        .await?;
        let hit = page
            .hits
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case("Adrenaline"))
            .ok_or_else(|| api::ApiError::new("acceptance_empty", "Adrenaline not found"))?;
        let project = api::project(
            host.clone(),
            String::new(),
            "modpack",
            hit.project_id.clone(),
        )
        .await?;
        let version = project
            .versions
            .iter()
            .find(|v| v.version_number.contains("1.21.11") || v.name.contains("1.21.11"))
            .ok_or_else(|| api::ApiError::new("acceptance_no_version", "No 1.21.11 Adrenaline"))?;
        review_and_install(
            &host,
            results,
            "Adrenaline new instance (warm cache)",
            None,
            &project.project_id,
            &version.id,
        )
        .await?;
    }
    if enabled(scenarios, "concurrent") {
        let (mut log_a, mut log_b) = (Vec::new(), Vec::new());
        let before = stats();
        let started = Instant::now();
        let (ra, rb) = tokio::join!(
            fabric_instance(
                &host,
                &mut log_a,
                "N8 concurrent A",
                "1.21.10",
                Some("vanilla")
            ),
            fabric_instance(
                &host,
                &mut log_b,
                "N8 concurrent B",
                "1.21.10",
                Some("vanilla")
            )
        );
        results.extend(log_a);
        results.extend(log_b);
        let entry = json!({
            "scenario": "two cold Vanilla 1.21.10 instances concurrently",
            "seconds": (started.elapsed().as_secs_f64() * 100.0).round() / 100.0,
            "transfers": delta(&before, &stats()),
            "a": ra.as_ref().map(|i| i.id.clone()).map_err(|e| e.code.clone()),
            "b": rb.as_ref().map(|i| i.id.clone()).map_err(|e| e.code.clone()),
        });
        println!("{}", serde_json::to_string(&entry).unwrap());
        results.push(entry);
        let (a, b) = (ra?, rb?);
        let (sa, sb) = tokio::join!(
            api::ensure_runtime(host.clone(), a.id.clone()),
            api::ensure_runtime(host.clone(), b.id.clone())
        );
        let entry = json!({
            "scenario": "runtime for both instances concurrently",
            "a": sa.as_ref().map(|s| format!("{} {} {}", s.status, s.component, s.required_major_version)).map_err(|e| e.message.clone()),
            "b": sb.as_ref().map(|s| format!("{} {} {}", s.status, s.component, s.required_major_version)).map_err(|e| e.message.clone()),
            "transfers": stats(),
        });
        println!("{}", serde_json::to_string(&entry).unwrap());
        results.push(entry);
        sa?;
        sb?;
    }
    if enabled(scenarios, "runtime") {
        let state = api::launcher_state(host.clone()).await?;
        for instance in state
            .instances
            .iter()
            .filter(|i| i.state == "ready")
            .take(6)
        {
            timed(
                results,
                &format!("runtime for {}", instance.display_name),
                api::ensure_runtime(host.clone(), instance.id.clone()),
                |s| json!({"status": s.status, "component": s.component, "major": s.required_major_version}),
            )
            .await?;
        }
    }
    Ok(())
}
