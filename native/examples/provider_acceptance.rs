//! Explicit live acceptance. Only a marked, direct child of the OS temp root.
//! No account, credential, Java launch, production root or deletion operations.
#![allow(dead_code)]
#[path = "../src/api.rs"]
mod api;
use aurora_launcher_lib::host::{Host, LAUNCHER_VERSION};
use serde_json::json;
use std::path::PathBuf;

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
            .starts_with("aurora-native7-")
    );
    assert_eq!(
        root.parent().unwrap().canonicalize().unwrap(),
        std::env::temp_dir().canonicalize().unwrap()
    );
    std::fs::create_dir_all(&root).unwrap();
    let marker = root.join("NATIVE7-DISPOSABLE.txt");
    if marker.exists() {
        assert_eq!(
            std::fs::read(&marker).unwrap(),
            b"Native 7 disposable acceptance\n"
        );
    } else {
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::write(&marker, b"Native 7 disposable acceptance\n").unwrap();
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
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
        if let Err(e) = run(host, &root).await {
            eprintln!("{}: {}", e.code, e.message);
            std::process::exit(1);
        }
    });
}
async fn run(host: Host, root: &std::path::Path) -> api::ApiResult<()> {
    let state = api::launcher_state(host.clone()).await?;
    let instance = match state
        .instances
        .iter()
        .find(|i| i.display_name == "Native7 content acceptance")
    {
        Some(i) => i.clone(),
        None => {
            api::create_instance(
                host.clone(),
                "Native7 content acceptance".into(),
                "1.21.11".into(),
                json!({"kind":"fabric","policy":{"type":"automatic"}}),
                false,
            )
            .await?
        }
    };
    println!("INSTANCE {} {}", instance.id, instance.subtitle());
    let mut results = Vec::new();
    for (kind, query) in [
        ("mod", "Sodium"),
        ("resourcePack", "Fresh Animations"),
        ("shaderPack", "Complementary Reimagined"),
        ("modpack", "Adrenaline"),
    ] {
        let target = if kind == "modpack" {
            String::new()
        } else {
            instance.id.clone()
        };
        let initial = api::browse(host.clone(), target.clone(), kind, String::new(), 0).await?;
        let page = api::browse(host.clone(), target.clone(), kind, query.into(), 0).await?;
        let second = api::browse(host.clone(), target.clone(), kind, query.into(), 20).await?;
        let first = page
            .hits
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case(query))
            .or_else(|| page.hits.first())
            .ok_or_else(|| {
                api::ApiError::new("acceptance_empty", "Expected a real search result")
            })?;
        let project = api::project(host.clone(), target, kind, first.project_id.clone()).await?;
        let icon = api::project_artwork(host.clone(), first.project_id.clone())
            .await?
            .map(|b| b.len());
        println!(
            "BROWSE {kind}: initial={} searched={} page2={} project={} versions={} icon={icon:?}",
            initial.hits.len(),
            page.hits.len(),
            second.hits.len(),
            project.title,
            project.versions.len()
        );
        if kind != "modpack" {
            api::quick_install(
                host.clone(),
                instance.id.clone(),
                kind,
                first.project_id.clone(),
            )
            .await?;
            let inventory = api::provider_inventory(host.clone(), instance.id.clone()).await?;
            assert!(
                inventory
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["record"]["projectId"] == first.project_id
                        && r["record"]["contentType"] == kind)
            );
            println!("INSTALLED {kind} {}", project.title);
        } else {
            let candidate = project
                .versions
                .iter()
                .find(|v| v.name.contains("1.21.11") || v.version_number.contains("1.21.11"))
                .or_else(|| project.versions.first())
                .ok_or_else(|| {
                    api::ApiError::new("acceptance_no_pack_version", "No supported pack version")
                })?;
            let mut request = json!({"projectId":project.project_id,"versionId":candidate.id,"selectedOptional":[],"fingerprint":null});
            let preview = api::preview_pack(host.clone(), false, request.clone()).await?;
            println!(
                "PACK REVIEW {} MC={} loader={}",
                project.title, preview["minecraftVersion"], preview["fabricLoaderVersion"]
            );
            let bad = api::install_pack(host.clone(), false, {
                let mut r = request.clone();
                r["fingerprint"] = json!("stale");
                r
            })
            .await;
            assert!(bad.is_err(), "stale fingerprint must refuse installation");
            request["fingerprint"] = preview["fingerprint"].clone();
            let installed = api::install_pack(host.clone(), false, request).await?;
            println!("PACK INSTALLED {}", installed["id"]);
            // Merge into a fresh ordinary instance of the pack's exact Minecraft.
            let merge_target=api::create_instance(host.clone(),"Native7 merge acceptance".into(),preview["minecraftVersion"].as_str().unwrap().into(),json!({"kind":"fabric","policy":{"type":"pinned","version":preview["fabricLoaderVersion"]}}),false).await?;
            let mut merge = json!({"instanceId":merge_target.id,"projectId":project.project_id,"versionId":candidate.id,"selectedOptional":[],"fingerprint":null});
            let reviewed = api::preview_pack(host.clone(), true, merge.clone()).await?;
            assert!(
                reviewed["blockers"].as_array().unwrap().is_empty(),
                "merge blockers: {}",
                reviewed["blockers"]
            );
            merge["fingerprint"] = reviewed["fingerprint"].clone();
            let merged = api::install_pack(host.clone(), true, merge).await?;
            println!("MERGED {}", merged);
            results
                .push(json!({"packPreview":preview,"mergeReview":reviewed,"mergeResult":merged}));
        }
        results.push(json!({"kind":kind,"project":project.title,"initial":initial.hits.len(),"searched":page.hits.len(),"page2":second.hits.len(),"iconBytes":icon}));
    }
    std::fs::write(
        root.join("live-results.json"),
        serde_json::to_vec_pretty(&results).unwrap(),
    )
    .unwrap();
    Ok(())
}
