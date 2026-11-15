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
