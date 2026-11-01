//! NATIVE-10 live acceptance over the exact native API calls the UI makes.
//!
//! Only a marked `aurora-native10-*` direct child of the OS temp root is
//! accepted. Uses real Mojang, Fabric, Forge, NeoForge and Modrinth metadata
//! and the shared verified cache. No account, credential, Java launch,
//! production root, or deletion. Writes `native10-acceptance.json` in the root.
#![allow(dead_code)]
#[path = "../src/api.rs"]
mod api;
use aurora_launcher_lib::host::{Host, LAUNCHER_VERSION};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct Report {
    steps: Vec<Value>,
    failures: usize,
}

impl Report {
    fn record(&mut self, name: &str, pass: bool, detail: impl Into<String>) {
        let detail = detail.into();
        println!("{} {name}: {detail}", if pass { "PASS" } else { "FAIL" });
        if !pass {
            self.failures += 1;
        }
        self.steps
            .push(json!({ "step": name, "pass": pass, "detail": detail }));
    }
}

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
            .starts_with("aurora-native10-")
    );
    assert_eq!(
        root.parent().unwrap().canonicalize().unwrap(),
        std::env::temp_dir().canonicalize().unwrap()
    );
    assert!(
        root.join("NATIVE10-DISPOSABLE.txt").is_file(),
        "disposable marker required"
    );
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
    let mut report = Report {
        steps: Vec::new(),
        failures: 0,
    };
    let mode = std::env::args().nth(2).unwrap_or_default();
    match mode.as_str() {
        "followup" => runtime.block_on(followup(host, &root, &mut report)),
        "merge" => runtime.block_on(merge(host, &root, &mut report)),
        _ => runtime.block_on(run(host, &root, &mut report)),
    }
    std::fs::write(
        root.join(match mode.as_str() {
            "followup" => "native10-acceptance-followup.json",
            "merge" => "native10-acceptance-merge.json",
            _ => "native10-acceptance.json",
        }),
        serde_json::to_vec_pretty(&json!({ "failures": report.failures, "steps": report.steps }))
            .unwrap(),
    )
    .unwrap();
    if report.failures > 0 {
        std::process::exit(1);
    }
}

fn hash_tree(dir: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    out.push((entry.file_name().to_string_lossy().into_owned(), meta.len()));
                }
            }
        }
    }
    out.sort();
    out
}

async fn instance_named(host: &Host, name: &str) -> Option<api::InstanceSummary> {
    api::launcher_state(host.clone())
        .await
        .ok()?
        .instances
        .into_iter()
        .find(|i| i.display_name == name)
}

/// Only the four-component NeoForge generation (Minecraft 26.x onward) is
/// installable by this build; older Minecraft versions truthfully have none.
async fn neoforge_catalogs(report: &mut Report) {
    match api::loader_versions("neoForge".into(), "26.1.2".into()).await {
        Ok(v) => report.record(
            "loader catalog neoforge 26.1.2",
            !v.is_empty(),
            format!(
                "{} versions, newest {:?}",
                v.len(),
                v.first().map(|x| &x.version)
            ),
        ),
        Err(e) => report.record("loader catalog neoforge 26.1.2", false, e.message),
    }
    match api::loader_versions("neoForge".into(), "1.21.1".into()).await {
        Ok(v) => report.record(
            "loader catalog neoforge 1.21.1 (outside installable generation)",
            v.is_empty(),
            format!("{} versions, none invented", v.len()),
        ),
        Err(e) => report.record(
            "loader catalog neoforge 1.21.1 (outside installable generation)",
            false,
            e.message,
        ),
    }
}

async fn ready_vanilla(host: &Host) -> Option<api::InstanceSummary> {
    api::launcher_state(host.clone())
        .await
        .ok()?
        .instances
        .into_iter()
        .find(|i| i.platform.kind == "vanilla" && i.state == "ready")
}

/// Re-runs only read-only checks (catalogs, incompatible merge review).
async fn followup(host: Host, root: &Path, report: &mut Report) {
    neoforge_catalogs(report).await;
    let instances_dir = root.join("managed").join("instances");
    let local = root
        .join("fixtures")
        .join("Adrenaline 26.5.1 local copy.mrpack");
    if api::identify_local_modpack(host.clone(), local.to_string_lossy().into_owned())
        .await
        .is_err()
    {
        report.record("local mrpack recognized", false, "identification failed");
        return;
    }
    let Some(vanilla) = ready_vanilla(&host).await else {
        report.record(
            "local mrpack merge into incompatible instance blocked",
            false,
            "no ready vanilla instance",
        );
        return;
    };
    let tree = hash_tree(&instances_dir.join(&vanilla.id));
    let merge = json!({ "projectId": "BYN9yKrV", "versionId": "JBUK3TRN", "selectedOptional": [], "fingerprint": null, "instanceId": vanilla.id });
    let outcome = api::preview_pack(host.clone(), true, merge).await;
    let blocked = match &outcome {
        Ok(v) => !v["blockers"].as_array().map_or(true, Vec::is_empty),
        Err(_) => true,
    };
    report.record(
        "local mrpack merge into incompatible instance blocked",
        blocked && hash_tree(&instances_dir.join(&vanilla.id)) == tree,
        match outcome {
            Ok(v) => format!(
                "{} ({}): blockers {}",
                vanilla.display_name,
                vanilla.subtitle(),
                v["blockers"]
            ),
            Err(e) => format!(
                "{} ({}): refused: {} ({})",
                vanilla.display_name,
                vanilla.subtitle(),
                e.message,
                e.code
            ),
        },
    );
}

/// Compatible merge of the recognized local pack into an ordinary instance.
async fn merge(host: Host, root: &Path, report: &mut Report) {
    let instances_dir = root.join("managed").join("instances");
    let local = root
        .join("fixtures")
        .join("Adrenaline 26.5.1 local copy.mrpack");
    if api::identify_local_modpack(host.clone(), local.to_string_lossy().into_owned())
        .await
        .is_err()
    {
        report.record("local mrpack recognized", false, "identification failed");
        return;
    }
    let Some(mut target) = instance_named(&host, "N10 exact Fabric pin").await else {
        report.record(
            "local mrpack compatible merge",
            false,
            "target instance missing",
        );
        return;
    };
    // The pack requires Fabric Loader 0.19.5: re-pin the ordinary instance
    // through the same save-then-install flow the Settings tab uses.
    if target.platform.version.as_deref() != Some("0.19.5") {
        let mut draft = target.configuration.clone();
        draft.loader =
            json!({ "kind": "fabric", "policy": { "type": "pinned", "version": "0.19.5" } });
        let repinned = match api::update_configuration(
            host.clone(),
            target.id.clone(),
            target.configuration.clone(),
            draft,
        )
        .await
        {
            Ok(_) => api::install_configuration(host.clone(), target.id.clone()).await,
            Err(e) => Err(e),
        };
        match repinned {
            Ok(installed) => {
                report.record(
                    "existing exact pin changed 0.19.2 → 0.19.5",
                    installed.platform.version.as_deref() == Some("0.19.5")
                        && installed.state == "ready",
                    installed.subtitle(),
                );
                target = installed;
            }
            Err(e) => {
                report.record(
                    "existing exact pin changed 0.19.2 → 0.19.5",
                    false,
                    format!("{} ({})", e.message, e.code),
                );
                return;
            }
        }
    }
    let mods = instances_dir.join(&target.id).join("mods");
    let sentinel = instances_dir
        .join(&target.id)
        .join("config")
        .join("native10-user-sentinel.txt");
    std::fs::create_dir_all(sentinel.parent().unwrap()).unwrap();
    std::fs::write(&sentinel, b"user configuration must survive").unwrap();
    let before = hash_tree(&mods);
    let request = json!({ "projectId": "BYN9yKrV", "versionId": "JBUK3TRN", "selectedOptional": [], "fingerprint": null, "instanceId": target.id });
    let preview = match api::preview_pack(host.clone(), true, request.clone()).await {
        Ok(preview) => preview,
        Err(e) => {
            report.record(
                "local mrpack compatible merge review",
                false,
                format!("{} ({})", e.message, e.code),
            );
            return;
        }
    };
    let blockers = preview["blockers"].as_array().cloned().unwrap_or_default();
    report.record(
        "local mrpack compatible merge review",
        blockers.is_empty(),
        format!(
            "{} added · {} kept existing · blockers {:?}",
            preview["added"].as_array().map_or(0, Vec::len),
            preview["keptExisting"].as_array().map_or(0, Vec::len),
            blockers
        ),
    );
    if !blockers.is_empty() {
        return;
    }
    let mut approved = request;
    approved["fingerprint"] = preview["fingerprint"].clone();
    match api::install_pack(host.clone(), true, approved).await {
        Ok(_) => {
            let after = hash_tree(&mods);
            let state = instance_named(&host, "N10 exact Fabric pin").await;
            report.record(
                "local mrpack compatible merge installed",
                after.len() > before.len()
                    && std::fs::read(&sentinel).unwrap() == b"user configuration must survive"
                    && state
                        .as_ref()
                        .is_some_and(|i| i.state == "ready" && i.pack.is_none()),
                format!(
                    "mods {} → {}; user config kept; stays an ordinary (unmanaged) instance: {}",
                    before.len(),
                    after.len(),
                    state.as_ref().is_some_and(|i| i.pack.is_none())
                ),
            );
        }
        Err(e) => report.record(
            "local mrpack compatible merge installed",
            false,
            format!("{} ({})", e.message, e.code),
        ),
    }
}

async fn run(host: Host, root: &Path, report: &mut Report) {
    let fixtures = root.join("fixtures");
    let instances_dir = root.join("managed").join("instances");

    // ---------------------------------------------------------- loader catalogs
    match api::loader_versions("fabric".into(), "1.21.11".into()).await {
        Ok(v) => report.record(
            "loader catalog fabric 1.21.11",
            v.iter().any(|x| x.version == "0.19.3"),
            format!(
                "{} versions, newest {:?}",
                v.len(),
                v.first().map(|x| &x.version)
            ),
        ),
        Err(e) => report.record("loader catalog fabric 1.21.11", false, e.message),
    }
    match api::loader_versions("forge".into(), "1.20.1".into()).await {
        Ok(v) => report.record(
            "loader catalog forge 1.20.1",
            !v.is_empty(),
            format!(
                "{} versions, recommended {:?}",
                v.len(),
                v.iter().find(|x| x.stable).map(|x| &x.version)
            ),
        ),
        Err(e) => report.record("loader catalog forge 1.20.1", false, e.message),
    }
    neoforge_catalogs(report).await;
    match api::loader_versions("fabric".into(), "0.0-not-minecraft".into()).await {
        Ok(v) => report.record(
            "loader catalog unsupported version",
            v.is_empty(),
            format!("{} versions (no invented versions)", v.len()),
        ),
        Err(e) => report.record(
            "loader catalog unsupported version",
            true,
            format!("refused: {} ({})", e.message, e.code),
        ),
    }
    report.record(
        "loader catalog vanilla",
        api::loader_versions("vanilla".into(), "1.21.11".into())
            .await
            .is_ok_and(|v| v.is_empty()),
        "vanilla has no loader versions",
    );

    // ---------------------------------------------------------- local .mrpack
    let local = fixtures.join("Adrenaline 26.5.1 local copy.mrpack");
    let identified =
        api::identify_local_modpack(host.clone(), local.to_string_lossy().into_owned()).await;
    let matched = match &identified {
        Ok(found) => {
            let ok = found["projectId"] == "BYN9yKrV" && found["versionId"] == "JBUK3TRN";
            report.record(
                "local mrpack recognized",
                ok,
                format!(
                    "{} {} → {}/{}",
                    found["title"], found["versionNumber"], found["projectId"], found["versionId"]
                ),
            );
            ok
        }
        Err(e) => {
            report.record(
                "local mrpack recognized",
                false,
                format!("{} ({})", e.message, e.code),
            );
            false
        }
    };
    match api::identify_local_modpack(
        host.clone(),
        fixtures
            .join("Adrenaline privately edited.mrpack")
            .to_string_lossy()
            .into_owned(),
    )
    .await
    {
        Err(e) => report.record(
            "unpublished local mrpack refused",
            e.code == "local_pack_unrecognized",
            format!("{} ({})", e.message, e.code),
        ),
        Ok(v) => report.record(
            "unpublished local mrpack refused",
            false,
            format!("unexpectedly matched {v}"),
        ),
    }
    match api::identify_local_modpack(
        host.clone(),
        fixtures
            .join("not-a-pack.mrpack")
            .to_string_lossy()
            .into_owned(),
    )
    .await
    {
        Err(e) => report.record(
            "malformed local mrpack refused",
            e.code != "local_pack_unrecognized",
            format!("{} ({})", e.message, e.code),
        ),
        Ok(v) => report.record(
            "malformed local mrpack refused",
            false,
            format!("unexpectedly matched {v}"),
        ),
    }
    let before_ids: Vec<String> = api::launcher_state(host.clone())
        .await
        .map(|s| s.instances.into_iter().map(|i| i.id).collect())
        .unwrap_or_default();
    let before = before_ids.len();
    if matched {
        let request = json!({ "projectId": "BYN9yKrV", "versionId": "JBUK3TRN", "selectedOptional": [], "fingerprint": null });
        match api::preview_pack(host.clone(), false, request.clone()).await {
            Ok(preview) => {
                let fingerprint = preview["fingerprint"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                report.record(
                    "local mrpack new-instance review",
                    !fingerprint.is_empty(),
                    format!(
                        "{} recognized, {} external, {} overrides",
                        preview["recognized"].as_array().map_or(0, Vec::len),
                        preview["unresolved"].as_array().map_or(0, Vec::len),
                        preview["overrides"].as_array().map_or(0, Vec::len)
                    ),
                );
                let mut approved = request;
                approved["fingerprint"] = json!(fingerprint);
                match api::install_pack(host.clone(), false, approved).await {
                    Ok(_) => {
                        let state = api::launcher_state(host.clone()).await.unwrap();
                        let created = state
                            .instances
                            .iter()
                            .find(|i| i.pack.is_some() && !before_ids.contains(&i.id))
                            .cloned();
                        let ready = created.as_ref().is_some_and(|i| i.state == "ready");
                        let validation = match &created {
                            Some(i) => api::validate_instance(host.clone(), i.id.clone())
                                .await
                                .map(|v| v.status)
                                .unwrap_or_else(|e| e.code),
                            None => "missing".into(),
                        };
                        report.record(
                            "local mrpack installed as managed pack instance",
                            ready
                                && state.instances.len() == before + 1
                                && (validation == "valid" || validation == "ready"),
                            format!(
                                "{} · state {} · validation {validation}",
                                created.as_ref().map(|i| i.subtitle()).unwrap_or_default(),
                                created
                                    .as_ref()
                                    .map(|i| i.state.clone())
                                    .unwrap_or_default()
                            ),
                        );
                    }
                    Err(e) => report.record(
                        "local mrpack installed as managed pack instance",
                        false,
                        format!("{} ({})", e.message, e.code),
                    ),
                }
            }
            Err(e) => report.record(
                "local mrpack new-instance review",
                false,
                format!("{} ({})", e.message, e.code),
            ),
        }
        // Merge review into an incompatible ordinary instance must block and change nothing.
        if let Some(vanilla) = ready_vanilla(&host).await {
            let tree = hash_tree(&instances_dir.join(&vanilla.id));
            let merge = json!({ "projectId": "BYN9yKrV", "versionId": "JBUK3TRN", "selectedOptional": [], "fingerprint": null, "instanceId": vanilla.id });
            let outcome = api::preview_pack(host.clone(), true, merge).await;
            let blocked = match &outcome {
                Ok(v) => !v["blockers"].as_array().map_or(true, Vec::is_empty),
                Err(_) => true,
            };
            report.record(
                "local mrpack merge into incompatible instance blocked",
                blocked && hash_tree(&instances_dir.join(&vanilla.id)) == tree,
                match outcome {
                    Ok(v) => format!("blockers {}", v["blockers"]),
                    Err(e) => format!("refused: {} ({})", e.message, e.code),
                },
            );
        }
    }

    // ---------------------------------------------------------- content updates
    if let Some(target) = instance_named(&host, "N8 merge target").await {
        let mods_before = hash_tree(&instances_dir.join(&target.id).join("mods"));
        match api::check_updates(host.clone(), target.id.clone(), "mod").await {
            Ok(report_) => {
                let eligible: Vec<_> = report_
                    .entries
                    .iter()
                    .filter(|e| e.status == "updateAvailable" && !e.pinned && e.candidate.is_some())
                    .collect();
                let statuses: std::collections::BTreeMap<_, usize> =
                    report_.entries.iter().fold(Default::default(), |mut m, e| {
                        *m.entry(e.status.clone()).or_default() += 1;
                        m
                    });
                report.record(
                    "content update check (merged ordinary instance)",
                    true,
                    format!(
                        "{statuses:?}, {} dependency-managed",
                        report_.dependency_managed
                    ),
                );
                if let Some(first) = eligible.first() {
                    let project = first.project_id.clone();
                    match api::preview_updates(
                        host.clone(),
                        target.id.clone(),
                        "mod",
                        vec![project.clone()],
                    )
                    .await
                    {
                        Ok(preview) => {
                            let fingerprint = preview["previewFingerprint"]
                                .as_str()
                                .unwrap_or_default()
                                .to_owned();
                            match api::apply_updates(
                                host.clone(),
                                target.id.clone(),
                                "mod",
                                vec![project.clone()],
                                fingerprint.clone(),
                            )
                            .await
                            {
                                Ok(_) => {
                                    let after =
                                        hash_tree(&instances_dir.join(&target.id).join("mods"));
                                    let changed: Vec<_> =
                                        after.iter().filter(|f| !mods_before.contains(f)).collect();
                                    report.record(
                                        "content update applied",
                                        !changed.is_empty(),
                                        format!(
                                            "{project} → {:?}; changed files {changed:?}",
                                            first.candidate.as_ref().map(|c| &c.version_number)
                                        ),
                                    );
                                    let stale = api::apply_updates(
                                        host.clone(),
                                        target.id.clone(),
                                        "mod",
                                        vec![project.clone()],
                                        fingerprint,
                                    )
                                    .await;
                                    report.record(
                                        "reapplying a superseded review is refused",
                                        stale.is_err(),
                                        stale
                                            .err()
                                            .map(|e| format!("{} ({})", e.message, e.code))
                                            .unwrap_or_default(),
                                    );
                                    let recheck =
                                        api::check_updates(host.clone(), target.id.clone(), "mod")
                                            .await;
                                    let now = recheck
                                        .ok()
                                        .and_then(|r| {
                                            r.entries.into_iter().find(|e| e.project_id == project)
                                        })
                                        .map(|e| e.status)
                                        .unwrap_or_default();
                                    report.record(
                                        "updated item re-checks as current",
                                        now != "updateAvailable",
                                        now,
                                    );
                                }
                                Err(e) => report.record(
                                    "content update applied",
                                    false,
                                    format!("{} ({})", e.message, e.code),
                                ),
                            }
                        }
                        Err(e) => report.record(
                            "content update preview",
                            false,
                            format!("{} ({})", e.message, e.code),
                        ),
                    }
                } else {
                    report.record(
                        "content update applied",
                        true,
                        "no eligible update existed today (nothing to apply)",
                    );
                }
            }
            Err(e) => report.record(
                "content update check (merged ordinary instance)",
                false,
                format!("{} ({})", e.message, e.code),
            ),
        }
    }
    if let Some(pack) = instance_named(&host, "vulkan optimized").await {
        match api::check_updates(host.clone(), pack.id.clone(), "mod").await {
            Ok(r) => {
                let owned = r
                    .entries
                    .iter()
                    .filter(|e| e.block.as_deref() == Some("packOwned"))
                    .count();
                report.record(
                    "pack-owned content is never individually updated",
                    r.entries.iter().all(|e| {
                        e.block.as_deref() == Some("packOwned") || e.status != "updateAvailable"
                    }),
                    format!("{owned} pack-owned records of {}", r.entries.len()),
                );
            }
            Err(e) => report.record(
                "pack-owned content is never individually updated",
                false,
                format!("{} ({})", e.message, e.code),
            ),
        }
        match api::check_modpack_update(host.clone(), pack.id.clone()).await {
            Ok(check) => {
                report.record(
                    "managed modpack update check",
                    true,
                    format!(
                        "status {} · installed {} · candidate {}",
                        check["status"],
                        check["currentPackVersion"],
                        check["candidate"]["versionNumber"]
                    ),
                );
                if check["status"] == "updateAvailable" {
                    match api::preview_modpack_update(host.clone(), pack.id.clone()).await {
                        Ok(plan) => report.record(
                            "managed modpack update review",
                            !plan["fingerprint"].as_str().unwrap_or_default().is_empty(),
                            format!(
                                "counts {} · game transition {}",
                                plan["counts"], plan["gameTransition"]
                            ),
                        ),
                        Err(e) => report.record(
                            "managed modpack update review",
                            false,
                            format!("{} ({})", e.message, e.code),
                        ),
                    }
                }
            }
            Err(e) => report.record(
                "managed modpack update check",
                false,
                format!("{} ({})", e.message, e.code),
            ),
        }
    }

    // ---------------------------------------------------------- exact loader pins
    if let Some(target) = instance_named(&host, "N8 merge target").await {
        let mods_before = hash_tree(&instances_dir.join(&target.id).join("mods"));
        let mut draft = target.configuration.clone();
        draft.loader =
            json!({ "kind": "fabric", "policy": { "type": "pinned", "version": "0.19.2" } });
        match api::update_configuration(
            host.clone(),
            target.id.clone(),
            target.configuration.clone(),
            draft.clone(),
        )
        .await
        {
            Ok(saved) => {
                report.record(
                    "exact loader pin saved without installing",
                    saved.platform.version.as_deref() == Some("0.19.3"),
                    format!("installed still {:?}", saved.platform.version),
                );
                match api::install_configuration(host.clone(), target.id.clone()).await {
                    Ok(installed) => {
                        let validation = api::validate_instance(host.clone(), target.id.clone())
                            .await
                            .map(|v| v.status)
                            .unwrap_or_else(|e| e.code);
                        report.record(
                            "exact loader pin installed",
                            installed.platform.version.as_deref() == Some("0.19.2")
                                && installed.state == "ready"
                                && hash_tree(&instances_dir.join(&target.id).join("mods"))
                                    == mods_before,
                            format!(
                                "installed {:?}, validation {validation}, user mods unchanged",
                                installed.platform.version
                            ),
                        );
                    }
                    Err(e) => report.record(
                        "exact loader pin installed",
                        false,
                        format!("{} ({})", e.message, e.code),
                    ),
                }
            }
            Err(e) => report.record(
                "exact loader pin saved without installing",
                false,
                format!("{} ({})", e.message, e.code),
            ),
        }
        if let Some(current) = instance_named(&host, "N8 merge target").await {
            let mut bad = current.configuration.clone();
            bad.loader = json!({ "kind": "fabric", "policy": { "type": "pinned", "version": "0.0.1-not-a-loader" } });
            let saved = api::update_configuration(
                host.clone(),
                current.id.clone(),
                current.configuration.clone(),
                bad,
            )
            .await;
            let outcome = match saved {
                Err(e) => format!("save refused: {} ({})", e.message, e.code),
                Ok(_) => match api::install_configuration(host.clone(), current.id.clone()).await {
                    Err(e) => format!("install refused: {} ({})", e.message, e.code),
                    Ok(i) => format!("UNEXPECTED install {:?}", i.platform.version),
                },
            };
            let after = instance_named(&host, "N8 merge target").await;
            report.record(
                "unsupported loader pin refused without damage",
                !outcome.starts_with("UNEXPECTED")
                    && after.as_ref().is_some_and(|i| {
                        i.state == "ready" && i.platform.version.as_deref() == Some("0.19.2")
                    }),
                outcome,
            );
            // Restore the automatic policy so the instance stays usable.
            if let Some(after) = after {
                let mut restore = after.configuration.clone();
                restore.loader = json!({ "kind": "fabric", "policy": { "type": "automatic" } });
                let _ = api::update_configuration(
                    host.clone(),
                    after.id.clone(),
                    after.configuration.clone(),
                    restore,
                )
                .await;
            }
        }
    }
    match api::create_instance(
        host.clone(),
        "N10 exact Fabric pin".into(),
        "1.21.11".into(),
        json!({ "kind": "fabric", "policy": { "type": "pinned", "version": "0.19.2" } }),
        false,
    )
    .await
    {
        Ok(created) => report.record(
            "instance created with exact loader pin",
            created.platform.version.as_deref() == Some("0.19.2") && created.state == "ready",
            created.subtitle(),
        ),
        Err(e) => report.record(
            "instance created with exact loader pin",
            false,
            format!("{} ({})", e.message, e.code),
        ),
    }
}
