use super::*;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const IDS: [&str; 8] = [
    "02100001-0000-4000-8000-000000000021",
    "02100002-0000-4000-8000-000000000021",
    "02100003-0000-4000-8000-000000000021",
    "02100004-0000-4000-8000-000000000021",
    "02100005-0000-4000-8000-000000000021",
    "02100006-0000-4000-8000-000000000021",
    "02100007-0000-4000-8000-000000000021",
    "02100008-0000-4000-8000-000000000021",
];
fn fixture(n: usize) -> serde_json::Value {
    let bytes: [&[u8]; 8] = [
        include_bytes!("testdata/02100001-0000-4000-8000-000000000021.json.gz"),
        include_bytes!("testdata/02100002-0000-4000-8000-000000000021.json.gz"),
        include_bytes!("testdata/02100003-0000-4000-8000-000000000021.json.gz"),
        include_bytes!("testdata/02100004-0000-4000-8000-000000000021.json.gz"),
        include_bytes!("testdata/02100005-0000-4000-8000-000000000021.json.gz"),
        include_bytes!("testdata/02100006-0000-4000-8000-000000000021.json.gz"),
        include_bytes!("testdata/02100007-0000-4000-8000-000000000021.json.gz"),
        include_bytes!("testdata/02100008-0000-4000-8000-000000000021.json.gz"),
    ];
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(bytes[n])
        .read_to_end(&mut raw)
        .unwrap();
    serde_json::from_slice(&raw).unwrap()
}
fn fight(n: usize) -> wire::FightRecord {
    parse::decode(&serde_json::to_vec(&fixture(n)["raw"]).unwrap()).unwrap()
}
struct Lab {
    root: PathBuf,
    paths: crate::paths::ManagedPaths,
    source: store::Source,
}
impl Lab {
    fn register(&self) {
        use crate::{
            distribution::ReleaseChannel,
            instances::{settings::InstanceConfiguration, *},
        };
        let mut registry = InstanceRegistry::empty();
        registry.instances_mut().push(
            InstanceRecord::new(
                InstanceId::new(&self.source.id).unwrap(),
                self.source.name.clone(),
                InstanceState::Installing,
                PinnedRelease::new(ReleaseChannel::Stable, "0.3.0", "26.2", "0.19.5").unwrap(),
                InstanceConfiguration::for_minecraft_version("26.2"),
            )
            .unwrap(),
        );
        registry.save(&self.paths.instance_registry_file()).unwrap();
    }
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("aurora-pulsar-{}", uuid::Uuid::new_v4()));
        let paths = crate::paths::ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let id = "311ec473933d43d09d5904b9fa17c763".to_owned();
        let source = store::Source {
            root: store::instance_root(&paths, &id).unwrap(),
            id,
            name: "Fixture instance".into(),
        };
        fs::create_dir_all(source.root.join("phenomena/fights")).unwrap();
        Self {
            root,
            paths,
            source,
        }
    }
    fn write(&self, n: usize) -> PathBuf {
        let path = self
            .source
            .root
            .join("phenomena/fights")
            .join(format!("{}.json", IDS[n]));
        fs::write(&path, serde_json::to_vec(&fixture(n)["raw"]).unwrap()).unwrap();
        path
    }
    fn refresh(&self) -> store::Index {
        store::refresh(&self.paths, std::slice::from_ref(&self.source)).unwrap()
    }
}
impl Drop for Lab {
    fn drop(&mut self) {
        assert!(
            self.root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-pulsar-")
        );
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn all_eight_raw_fixtures_decode_with_exact_counts() {
    for (n, count) in [218, 30, 31, 228, 10, 18, 3, 880].into_iter().enumerate() {
        let f = fight(n);
        assert_eq!(f.id, IDS[n]);
        assert_eq!(f.events.len(), count);
    }
}
#[test]
fn historical_defaults_and_integer_bounds() {
    let mut raw = fixture(5)["raw"].clone();
    raw.as_object_mut().unwrap().remove("policy");
    raw.as_object_mut().unwrap().remove("resolution");
    raw["unknownAdditive"] = serde_json::json!({"preserved":true});
    let decoded = parse::decode(&serde_json::to_vec(&raw).unwrap()).unwrap();
    assert_eq!(decoded.policy().perfect_reach_target, 3.0);
    assert_eq!(decoded.statistics.exchange_timeout_seconds, 3.0);
    assert!(decoded.extra.contains_key("unknownAdditive"));
    raw["statistics"]["attempts"] = serde_json::json!(i64::MAX);
    assert!(parse::decode(&serde_json::to_vec(&raw).unwrap()).is_err());
}
#[test]
fn malformed_future_order_and_missing_required_files_are_preserved() {
    let lab = Lab::new();
    let path = lab.write(0);
    let original = fs::read(&path).unwrap();
    for mutate in 0..5 {
        let mut raw = fixture(0)["raw"].clone();
        match mutate {
            0 => raw["schemaVersion"] = 99.into(),
            1 => raw["events"][1]["sequence"] = 0.into(),
            2 => raw["events"][1]["nanoTime"] = (-1).into(),
            3 => {
                raw.as_object_mut().unwrap().remove("statistics");
            }
            _ => raw["events"] = serde_json::Value::Null,
        }
        let bytes = serde_json::to_vec(&raw).unwrap();
        fs::write(&path, &bytes).unwrap();
        let index = lab.refresh();
        assert!(index.entries.is_empty());
        assert!(!index.issues[&lab.source.id].is_empty());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::write(&path, &original).unwrap();
    assert_eq!(lab.refresh().entries.len(), 1);
}
#[test]
fn imports_are_idempotent_retained_and_tombstoned() {
    let lab = Lab::new();
    let source = lab.write(0);
    let first = lab.refresh();
    let second = lab.refresh();
    assert_eq!(first.entries.len(), 1);
    assert_eq!(second.entries.len(), 1);
    assert_eq!(
        first.entries[0].provenance.imported_epoch_millis,
        second.entries[0].provenance.imported_epoch_millis
    );
    assert_eq!(
        store::load_record(&lab.paths, &second.entries[0])
            .unwrap()
            .events
            .len(),
        218
    );
    fs::remove_file(&source).unwrap();
    assert_eq!(lab.refresh().entries.len(), 1);
    lab.write(0);
    let request = view::DeleteRequest {
        key: first.entries[0].key.clone(),
        confirmed: true,
        also_delete_from_instance: false,
    };
    store::delete(&lab.paths, &request).unwrap();
    assert!(source.exists());
    assert!(lab.refresh().entries.is_empty());
    assert_eq!(store::read_index(&lab.paths).unwrap().tombstones.len(), 1);
}
#[test]
fn temp_checkpoint_export_depth_and_uuid_rules() {
    let lab = Lab::new();
    let source = lab.write(0);
    let bytes = fs::read(source).unwrap();
    for rel in [
        "phenomena/fights/interrupted.json.tmp",
        "phenomena/fights/interrupted.json.migrating",
        "phenomena/fights/not-a-uuid.json",
        "phenomena/pending-sessions/checkpoint.json",
        "phenomena/exports/report.json",
        "phenomena/fights/a/b/deep.json",
    ] {
        let path = lab.source.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &bytes).unwrap();
    }
    let index = lab.refresh();
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].key.fight_id, IDS[0]);
    let wrong = lab
        .source
        .root
        .join("phenomena/fights")
        .join(format!("{}.json", IDS[1]));
    fs::write(&wrong, &bytes).unwrap();
    assert_eq!(
        parse::read_fight(&lab.source.root, &wrong)
            .unwrap_err()
            .code,
        "pulsar_id_mismatch"
    );
}
#[test]
fn oversize_file_is_rejected_before_read() {
    let lab = Lab::new();
    let path = lab.write(0);
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(parse::MAX_FILE_BYTES + 1)
        .unwrap();
    assert_eq!(
        parse::read_fight(&lab.source.root, &path).unwrap_err().code,
        "pulsar_oversize"
    );
}
#[test]
fn conflicts_prefer_current_root_and_never_double_count_manual_children() {
    let lab = Lab::new();
    lab.write(3);
    let mut old = fixture(3)["raw"].clone();
    old["reason"] = "conflicting legacy".into();
    let folder = lab.source.root.join("kwamster/fights");
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join(format!("{}.json", IDS[3])),
        serde_json::to_vec(&old).unwrap(),
    )
    .unwrap();
    let index = lab.refresh();
    assert_eq!(index.entries.len(), 1);
    assert!(!index.entries[0].legacy);
    assert!(!index.entries[0].provenance.conflicts.is_empty());
    assert_eq!(
        store::load_record(&lab.paths, &index.entries[0])
            .unwrap()
            .session
            .unwrap()
            .exchanges
            .len(),
        2
    );
}
#[test]
fn last_valid_import_survives_a_transient_or_malformed_source() {
    let lab = Lab::new();
    let path = lab.write(0);
    lab.refresh();
    fs::write(&path, b"{").unwrap();
    let index = lab.refresh();
    assert_eq!(index.entries.len(), 1);
    assert!(index.entries[0].stale);
    assert_eq!(
        store::load_record(&lab.paths, &index.entries[0])
            .unwrap()
            .id,
        IDS[0]
    );
}
#[test]
fn malformed_or_future_index_is_never_overwritten() {
    let lab = Lab::new();
    lab.write(0);
    lab.refresh();
    let path = store::library_root(&lab.paths).join("index.json");
    for bytes in [
        b"{".as_slice(),
        b"{\"schemaVersion\":999,\"revision\":1,\"entries\":[],\"tombstones\":[],\"issues\":{}}",
    ] {
        fs::write(&path, bytes).unwrap();
        assert!(store::refresh(&lab.paths, std::slice::from_ref(&lab.source)).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
#[ignore = "Reads the owner's real stores into a disposable launcher library; never writes to sources."]
fn real_stores_import_read_only_without_parse_errors() {
    let lab = Lab::new();
    let sources = [
        store::Source {
            id: "044b250279dc48de822f1684a2a3941e".into(),
            name: "Real legacy store".into(),
            root: PathBuf::from(
                "C:/Users/kalib/AppData/Local/com.aurora.launcher/instances/044b250279dc48de822f1684a2a3941e",
            ),
        },
        store::Source {
            id: "311ec473933d43d09d5904b9fa17c763".into(),
            name: "Real current store".into(),
            root: PathBuf::from(
                "C:/Users/kalib/AppData/Local/com.aurora.launcher.native-preview/instances/311ec473933d43d09d5904b9fa17c763",
            ),
        },
    ];
    let index = store::refresh(&lab.paths, &sources).unwrap();
    for (id, issues) in &index.issues {
        assert!(issues.is_empty(), "{id}: {issues:?}");
    }
    assert_eq!(index.entries.len(), 369);
    eprintln!(
        "REAL STORE IMPORT: {} recordings, zero parse errors",
        index.entries.len()
    );
}
#[test]
fn full_large_fixture_path_when_available() {
    let root = Path::new("C:/Users/kalib/Aurora Launcher/work/pulsar/sample-store");
    if !root.exists() {
        return;
    }
    let folder = root.join("phenomena/fights");
    let mut found = false;
    for dir in fs::read_dir(folder).unwrap() {
        let path = dir.unwrap().path().join(format!("{}.json", IDS[7]));
        if path.exists() {
            assert_eq!(parse::read_fight(root, &path).unwrap().0.events.len(), 880);
            found = true;
        }
    }
    assert!(found);
}

fn compare_oracle(
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    use serde_json::Value;
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => {
            let keys: std::collections::BTreeSet<_> = a.keys().chain(e.keys()).collect();
            for key in keys {
                compare_oracle(
                    a.get(key).unwrap_or(&Value::Null),
                    e.get(key).unwrap_or(&Value::Null),
                    &format!("{path}.{key}"),
                    errors,
                );
            }
        }
        (Value::Array(a), Value::Array(e)) => {
            if a.len() != e.len() {
                errors.push(format!("{path}: array length {} != {}", a.len(), e.len()));
            }
            for (i, (a, e)) in a.iter().zip(e).enumerate() {
                compare_oracle(a, e, &format!("{path}[{i}]"), errors);
            }
        }
        (Value::Number(a), Value::Number(e)) => {
            let (a, e) = (a.as_f64().unwrap(), e.as_f64().unwrap());
            let tolerance = if path.contains("dominance.values[") {
                1e-4
            } else {
                1e-9
            };
            if (a - e).abs() > tolerance {
                errors.push(format!("{path}: {a} != {e}"));
            }
        }
        _ => {
            if actual != expected {
                errors.push(format!("{path}: {actual} != {expected}"));
            }
        }
    }
}
fn oracle_case(n: usize) {
    let data = fixture(n);
    let raw = fight(n);
    let a = analytics::analyze(&raw);
    let mut errors = Vec::new();
    let mut projected = a.derived.clone();
    // Production exports replace announcement text only in the manual parent payload.
    // Core retains the original evidence; compare the same explicit privacy projection.
    if data["oracle"]["anonymized"] == true && raw.session.is_some() {
        for entry in projected["boundaries"].as_array_mut().unwrap() {
            entry["text"] = "Announcement text redacted".into();
        }
    }
    compare_oracle(
        &projected,
        &data["oracle"]["derived"],
        "derived",
        &mut errors,
    );
    compare_oracle(
        &serde_json::to_value(analytics::metrics::definitions()).unwrap(),
        &data["oracle"]["metrics"],
        "metrics",
        &mut errors,
    );
    if let Some(session) = &raw.session {
        for (i, ex) in session.exchanges.iter().enumerate() {
            compare_oracle(
                &analytics::analyze(ex).derived,
                &data["oracle"]["scopes"][i]["derived"],
                &format!("scopes[{i}]"),
                &mut errors,
            );
        }
    }
    for e in errors.iter().take(30) {
        eprintln!("{} {e}", IDS[n]);
    }
    assert!(
        errors.is_empty(),
        "{}: {} oracle mismatches",
        IDS[n],
        errors.len()
    );
}
#[test]
fn oracle_completed_multiweapon() {
    oracle_case(0);
}
#[test]
fn oracle_multiround() {
    oracle_case(1);
}
#[test]
fn oracle_separated_exchanges() {
    oracle_case(2);
}
#[test]
fn oracle_manual_participants() {
    oracle_case(3);
}
#[test]
fn oracle_interrupted() {
    oracle_case(4);
}
#[test]
fn oracle_historical() {
    oracle_case(5);
}
#[test]
fn oracle_no_hit_incomplete() {
    oracle_case(6);
}
#[test]
fn oracle_long_forty_round_match() {
    oracle_case(7);
}

#[test]
fn nullable_constructor_collections_and_negative_measurements_normalize() {
    let mut raw = fixture(0)["raw"].clone();
    for key in [
        "reachDistribution",
        "recentReach",
        "attackCanonicalIds",
        "attackSamples",
        "attackClassifications",
        "attackTimings",
        "combatExchanges",
    ] {
        raw["statistics"][key] = serde_json::Value::Null;
    }
    raw["resolution"]["notes"] = serde_json::Value::Null;
    let decoded = parse::decode(&serde_json::to_vec(&raw).unwrap()).unwrap();
    assert!(decoded.statistics.reach_distribution.is_empty());
    assert!(decoded.resolution.unwrap().notes.is_empty());
}

#[test]
fn transient_partial_recording_replacement_is_retried() {
    let lab = Lab::new();
    let path = lab.write(0);
    let valid = fs::read(&path).unwrap();
    fs::write(&path, b"{").unwrap();
    let replacement = path.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(15));
        fs::write(replacement, valid).unwrap();
    });
    assert_eq!(
        parse::read_fight(&lab.source.root, &path).unwrap().0.id,
        IDS[0]
    );
    writer.join().unwrap();
}

#[test]
fn event_depth_and_nested_session_budgets_are_enforced() {
    let mut raw = fixture(6)["raw"].clone();
    let event = raw["events"][0].clone();
    raw["events"] = serde_json::json!(vec![event; 10_001]);
    assert_eq!(
        parse::decode(&serde_json::to_vec(&raw).unwrap())
            .unwrap_err()
            .code,
        "pulsar_event_limit"
    );
    let mut manual = fixture(3)["raw"].clone();
    manual["session"]["exchanges"][0]["session"] = manual["session"].clone();
    assert!(parse::decode(&serde_json::to_vec(&manual).unwrap()).is_err());
    assert!(parse::decode(format!("{}0{}", "[".repeat(140), "]".repeat(140)).as_bytes()).is_err());
}

#[cfg(windows)]
#[test]
fn junction_ancestors_are_rejected_without_touching_the_target() {
    let lab = Lab::new();
    let target = lab.root.join("link-target");
    fs::create_dir(&target).unwrap();
    let link = lab.source.root.join("phenomena/fights/linked");
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link.to_string_lossy().replace('/', "\\"))
        .arg(target.to_string_lossy().replace('/', "\\"))
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let file = target.join(format!("{}.json", IDS[0]));
    let bytes = serde_json::to_vec(&fixture(0)["raw"]).unwrap();
    fs::write(&file, &bytes).unwrap();
    assert_eq!(
        parse::read_fight(&lab.source.root, &link.join(file.file_name().unwrap()))
            .unwrap_err()
            .code,
        "pulsar_unsafe_path"
    );
    assert!(lab.refresh().entries.is_empty());
    assert_eq!(fs::read(file).unwrap(), bytes);
    fs::remove_dir(link).unwrap();
}

#[test]
fn canonical_duplicates_exclusions_and_frozen_timings_survive_preferences() {
    let mut f = fight(2);
    let a = analytics::analyze(&f);
    assert_eq!(a.derived["summary"]["deduplicatedAttacks"], 1);
    assert_eq!(a.derived["summary"]["excludedSwings"], 1);
    assert_eq!(a.events.iter().filter(|e| e.kind == "ATTACK").count(), 14);
    f.policy.as_mut().unwrap().timing_tolerance_ticks = 0.0;
    let changed = analytics::analyze(&f);
    assert_eq!(changed.derived["timingSamples"], a.derived["timingSamples"]);
    let multi = analytics::analyze(&fight(0));
    assert_eq!(multi.derived["timingSamples"][5]["grade"], "OPENER");
    assert_eq!(multi.derived["timingSamples"][8]["grade"], "OPENER");
}

#[test]
fn same_tick_trades_deduplicate_opponent_observations() {
    let mut f = fight(0);
    let local = analytics::analyze(&f)
        .combos
        .marks
        .into_iter()
        .find(|m| m.local)
        .unwrap();
    let template = f
        .events
        .iter()
        .find(|e| e.kind == "DAMAGE_IN" && analytics::attributed(e, f.opponent.as_deref()))
        .unwrap()
        .clone();
    f.events.retain(|e| e.kind != "DAMAGE_IN");
    for seq in [10001, 10002] {
        let mut event = template.clone();
        event.sequence = seq;
        event.nano_time = local.nanos;
        event.client_tick = local.tick;
        f.events.push(event);
    }
    let c = analytics::analyze(&f).combos;
    assert_eq!(c.marks.iter().filter(|m| !m.local).count(), 1);
    assert_eq!(c.marks.iter().filter(|m| m.traded).count(), 2);
}

#[test]
fn creative_and_nearest_box_hits_do_not_enter_eligible_ray_reach() {
    let mut f = fight(0);
    let baseline = analytics::analyze(&f);
    for e in &mut f.events {
        if let Some(a) = &mut e.attack {
            if let Some(m) = &mut a.measurement {
                m.mode = Some("MELEE_ATTEMPT_CREATIVE".into());
            }
        }
    }
    let creative = analytics::analyze(&f);
    assert_eq!(creative.derived["reach"]["eligible"], 0);
    assert_eq!(
        creative.derived["summary"]["hits"],
        baseline.derived["summary"]["hits"]
    );
    let mut f = fight(0);
    for e in &mut f.events {
        if let Some(a) = &mut e.attack {
            a.ray_contact = false;
        }
    }
    let nearest = analytics::analyze(&f);
    assert_eq!(nearest.derived["reach"]["eligible"], 0);
    assert!(!nearest.derived["summary"]["averageReach"].is_null());
}

#[test]
fn shield_disable_and_block_remain_distinct_with_bounded_follow_up() {
    let a = analytics::analyze(&fight(0));
    assert_eq!(a.derived["shields"]["confirmedBlocks"], 2);
    assert_eq!(a.derived["shields"]["disables"], 1);
    assert_eq!(a.derived["shields"]["hitsAfterDisable"], 4);
    assert_eq!(a.derived["summary"]["accuracyDenominator"], 14);
}

#[test]
fn health_baseline_same_tick_and_jump_sequence_ties() {
    use wire::*;
    let mut f = fight(0);
    f.events.clear();
    f.evaluations.clear();
    let event = |seq, tick, kind: &str, health| CombatEvent {
        sequence: seq,
        nano_time: seq * 1_000_000,
        client_tick: Some(tick),
        kind: kind.into(),
        actor: Some("B".into()),
        confidence: "OBSERVED".into(),
        context: Some(Context {
            health,
            attribution: Some("OPPONENT_DIRECT".into()),
            ground: Some(true),
            movement_valid: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    };
    f.events = vec![
        event(1, 100, "HEALTH", Some(20.0)),
        event(2, 100, "HEALTH", Some(18.0)),
        event(3, 100, "DAMAGE_IN", None),
        event(4, 101, "HEALTH", Some(15.0)),
        event(5, 99, "JUMP", None),
        event(6, 101, "JUMP", None),
    ];
    let result = analytics::analyze(&f).derived["incomingDetail"].clone();
    assert_eq!(result[0]["damage"], 3.0);
    assert_eq!(result[0]["jumpOffsetTicks"], -1);
    assert_eq!(result[0]["jumpClass"], "ATTEMPT_CANDIDATE");
}

#[test]
fn manual_scope_no_hit_parent_and_child_selection_do_not_double_count() {
    let lab = Lab::new();
    lab.write(3);
    let index = lab.refresh();
    let key = index.entries[0].key.clone();
    let parent = service::detail(
        &lab.paths,
        view::DetailRequest {
            key: key.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(parent.exchanges.len(), 2);
    let ex = &parent.exchanges[0];
    let child = service::detail(
        &lab.paths,
        view::DetailRequest {
            key: key.clone(),
            participant: ex.participant_id.clone(),
            exchange_id: Some(ex.id.clone()),
        },
    )
    .unwrap();
    assert_eq!(child.selected_exchange.as_deref(), Some(ex.id.as_str()));
    assert!(
        service::detail(
            &lab.paths,
            view::DetailRequest {
                key: key.clone(),
                participant: Some("missing".into()),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(
        service::analyze_library(&lab.paths, view::AnalysisRequest::default())
            .unwrap()
            .coverage
            .recordings,
        1
    );
    let mut raw = fixture(3)["raw"].clone();
    raw["events"] = serde_json::json!([]);
    raw["evaluations"] = serde_json::json!([]);
    raw["session"]["exchanges"] = serde_json::json!([]);
    raw["statistics"] = serde_json::json!({});
    let empty = parse::decode(&serde_json::to_vec(&raw).unwrap()).unwrap();
    assert!(analytics::analyze(&empty).derived["summary"]["accuracy"].is_null());
}

#[test]
fn pooled_ratios_mixed_policy_and_summary_only_coverage() {
    let lab = Lab::new();
    lab.write(0);
    let second = lab.write(2);
    let mut raw = fixture(2)["raw"].clone();
    raw["policy"]["perfectReachTarget"] = 2.5.into();
    fs::write(second, serde_json::to_vec(&raw).unwrap()).unwrap();
    let index = lab.refresh();
    let a = service::analyze_library(&lab.paths, view::AnalysisRequest::default()).unwrap();
    assert!((a.overview.metrics["accuracy"].value.unwrap() - 100.0 * 23.0 / 26.0).abs() < 1e-9);
    assert!(a.coverage.mixed_reach_policy);
    let entries: Vec<_> = index.entries.iter().collect();
    let summary = projection::pool(&entries, &[], &None, &None);
    assert_eq!(summary.0.metrics["hits"].value, Some(23.0));
    assert_eq!(summary.1.metrics["reachWithin"].value, None);
    let empty = analytics::metrics::metric("accuracy", analytics::ratio(0.0, 0.0));
    assert_eq!(empty.availability, view::Availability::Unavailable);
    assert_eq!(
        serde_json::to_value(empty).unwrap()["value"],
        serde_json::Value::Null
    );
}

#[test]
fn history_ties_filters_pagination_and_unavailable_last() {
    let lab = Lab::new();
    for n in [0, 2, 6] {
        lab.write(n);
    }
    let mut index = lab.refresh();
    for e in &mut index.entries {
        e.record.start_epoch_millis = 100;
    }
    let q = view::LibraryQuery {
        sort: view::HistorySort::Accuracy,
        ..Default::default()
    };
    let rows = service::filtered(&index, &q).unwrap();
    assert_eq!(rows[0].key.fight_id, IDS[0]);
    assert_eq!(rows[2].key.fight_id, IDS[6]);
    let q = view::LibraryQuery {
        sort: view::HistorySort::Newest,
        ..Default::default()
    };
    assert_eq!(
        service::filtered(&index, &q).unwrap()[0].key.fight_id,
        IDS[0]
    );
    let q = view::LibraryQuery {
        outcome: view::OutcomeFilter::Win,
        hits_only: true,
        ..Default::default()
    };
    assert_eq!(service::filtered(&index, &q).unwrap().len(), 2);
    let q = view::LibraryQuery {
        limit: 1,
        offset: 1,
        ..Default::default()
    };
    let page = service::list_library(&lab.paths, q).unwrap();
    assert_eq!(page.rows.len(), 1);
    assert!(page.has_more);
    assert!(
        service::list_library(
            &lab.paths,
            view::LibraryQuery {
                limit: 1001,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn opponents_records_and_compare_link_to_fights_and_gate_accuracy() {
    let lab = Lab::new();
    for n in [0, 2, 6] {
        lab.write(n);
    }
    let index = lab.refresh();
    let opponents = service::opponents(&lab.paths, view::LibraryQuery::default()).unwrap();
    let b = opponents.iter().find(|o| o.id == "B").unwrap();
    assert_eq!(b.wins, 2);
    assert_eq!(b.fights.len(), 2);
    assert!((b.metrics["accuracy"].value.unwrap() - 100.0 * 23.0 / 26.0).abs() < 1e-9);
    let records = service::records(&lab.paths, view::LibraryQuery::default()).unwrap();
    let accuracy = records.iter().find(|r| r.id == "bestAccuracy").unwrap();
    assert_eq!(accuracy.minimum_attempts, Some(10));
    assert_eq!(accuracy.key.as_ref().unwrap().fight_id, IDS[0]);
    let left = view::DetailRequest {
        key: index
            .entries
            .iter()
            .find(|e| e.key.fight_id == IDS[0])
            .unwrap()
            .key
            .clone(),
        ..Default::default()
    };
    let right = view::DetailRequest {
        key: index
            .entries
            .iter()
            .find(|e| e.key.fight_id == IDS[6])
            .unwrap()
            .key
            .clone(),
        ..Default::default()
    };
    let compare = service::compare(&lab.paths, view::CompareRequest { left, right }).unwrap();
    assert_eq!(
        compare
            .metrics
            .iter()
            .find(|m| m.key == "reachWithin")
            .unwrap()
            .delta,
        None
    );
    assert_eq!(
        compare
            .metrics
            .iter()
            .find(|m| m.key == "hits")
            .unwrap()
            .delta,
        Some(-13.0)
    );
}

#[test]
fn settings_defaults_migration_future_and_malformed_are_read_only() {
    let lab = Lab::new();
    lab.register();
    let defaults = settings::inspect(&lab.paths, &lab.source.id).unwrap();
    assert!(defaults.read_only);
    assert!(defaults.settings.iter().all(|s| s.runtime_only));
    assert_eq!(
        defaults
            .settings
            .iter()
            .find(|s| s.key == "forwardColor")
            .unwrap()
            .value,
        -7812950
    );
    let path = lab.source.root.join("config/aurora.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for raw in [
        serde_json::json!({"pulsarSettings":{"schemaVersion":1,"display":"BOTH","trendWindow":37}}),
        serde_json::json!({"pulsarSettings":{"schemaVersion":999,"tracking":false}}),
    ] {
        let bytes = serde_json::to_vec(&raw).unwrap();
        fs::write(&path, &bytes).unwrap();
        let settings = settings::inspect(&lab.paths, &lab.source.id).unwrap();
        assert!(!settings.warnings.is_empty());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let chat = settings
            .settings
            .iter()
            .find(|s| s.key == "chatSummary")
            .unwrap();
        assert_eq!(chat.value, raw["pulsarSettings"]["schemaVersion"] == 1);
    }
    fs::write(&path, b"{").unwrap();
    assert_eq!(
        settings::inspect(&lab.paths, &lab.source.id)
            .unwrap_err()
            .code,
        "pulsar_settings_invalid"
    );
    assert_eq!(fs::read(&path).unwrap(), b"{");
}

#[test]
fn report_copy_receipt_integrity_confirmation_and_source_preservation() {
    let lab = Lab::new();
    let original = lab.write(0);
    let index = lab.refresh();
    let key = index.entries[0].key.clone();
    let folder = lab.source.root.join("phenomena/exports/fixture");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(".phenomena-owner"), IDS[0]).unwrap();
    fs::write(folder.join("index.html"), b"<html>anonymized</html>").unwrap();
    fs::write(folder.join("report.json"), b"{}").unwrap();
    let destination = Lab::new();
    let mut request = view::ExportRequest {
        key: key.clone(),
        destination: destination.root.to_string_lossy().into_owned(),
        confirmed: false,
    };
    assert_eq!(
        reports::export(&lab.paths, &request).unwrap_err().code,
        "pulsar_confirmation_required"
    );
    request.confirmed = true;
    let copy = reports::export(&lab.paths, &request).unwrap();
    assert_eq!(
        fs::read(Path::new(copy.path.as_ref().unwrap()).join("index.html")).unwrap(),
        fs::read(folder.join("index.html")).unwrap()
    );
    assert_eq!(
        reports::export(&lab.paths, &request).unwrap_err().code,
        "pulsar_export_exists"
    );
    let receipt = lab
        .source
        .root
        .join(format!("phenomena/report-state/{}.json", IDS[0]));
    fs::create_dir_all(receipt.parent().unwrap()).unwrap();
    fs::write(&receipt,serde_json::to_vec(&serde_json::json!({"id":IDS[0],"folder":"fixture","htmlDigest":"wrong","jsonDigest":"wrong"})).unwrap()).unwrap();
    assert_eq!(
        reports::export(&lab.paths, &request).unwrap_err().code,
        "pulsar_report_pending"
    );
    assert!(original.exists());
    assert!(folder.join("index.html").exists());
}

#[test]
fn source_delete_requires_confirmation_and_stopped_instance_preserves_reports() {
    let lab = Lab::new();
    let path = lab.write(0);
    let index = lab.refresh();
    let mut request = view::DeleteRequest {
        key: index.entries[0].key.clone(),
        confirmed: false,
        also_delete_from_instance: true,
    };
    assert_eq!(
        store::delete(&lab.paths, &request).unwrap_err().code,
        "pulsar_confirmation_required"
    );
    request.confirmed = true;
    {
        let _running = crate::launch::process::force_running(&lab.source.id);
        assert!(store::delete(&lab.paths, &request).is_err());
        assert!(path.exists());
    }
    let sentinel = lab.source.root.join("phenomena/report-sentinel");
    fs::write(&sentinel, b"keep").unwrap();
    store::delete(&lab.paths, &request).unwrap();
    assert!(!path.exists());
    assert!(sentinel.exists());
    assert!(lab.refresh().entries.is_empty());
}

#[tokio::test]
async fn application_commands_use_native_host_and_serialize_contract_end_to_end() {
    use crate::{application as app, host::Host};
    let lab = Lab::new();
    lab.register();
    lab.write(0);
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let collected = events.clone();
    let host = Host::native(
        Ok(lab.root.clone()),
        "1.5.1",
        tokio::runtime::Handle::current(),
        move |name, _| collected.lock().unwrap().push(name.to_owned()),
    );
    let library = app::refresh_pulsar_library(host.clone(), view::LibraryQuery::default())
        .await
        .unwrap();
    assert_eq!(library.total, 1);
    let detail = view::DetailRequest {
        key: library.rows[0].key.clone(),
        ..Default::default()
    };
    let fight = app::get_pulsar_fight(host.clone(), detail.clone())
        .await
        .unwrap();
    let json = serde_json::to_value(&fight).unwrap();
    assert!(json["summary"]["timelineLoaded"] == true);
    let roundtrip: view::FightDetailView = serde_json::from_value(json).unwrap();
    assert_eq!(roundtrip.summary.key, detail.key);
    let playback = app::get_pulsar_playback(
        host.clone(),
        view::PlaybackRequest {
            detail,
            playhead_seconds: 9.0,
            event_filter: Some("ATTACK".into()),
        },
    )
    .await
    .unwrap();
    assert!(!playback.checkpoints.is_empty());
    assert!(playback.visible_events.iter().all(|e| e.kind == "ATTACK"));
    assert_eq!(
        app::get_pulsar_records(host.clone(), view::LibraryQuery::default())
            .await
            .unwrap()
            .len(),
        5
    );
    assert!(
        app::get_pulsar_settings(
            host.clone(),
            view::FolderRequest {
                instance_id: lab.source.id.clone(),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .read_only
    );
    assert!(events.lock().unwrap().iter().any(|e| e == "pulsar-changed"));
    let error = app::list_pulsar_library(
        host,
        view::LibraryQuery {
            limit: 1001,
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        serde_json::to_value(error).unwrap()["code"],
        "pulsar_query_limit"
    );
}
