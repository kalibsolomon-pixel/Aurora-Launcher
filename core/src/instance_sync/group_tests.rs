use super::*;
use crate::instances::InstanceRecord;
use crate::instances::platform::{InstalledConfiguration, PlatformPin};
use crate::instances::settings::InstanceConfiguration;

struct Fixture {
    managed: ManagedPaths,
    ids: Vec<InstanceId>,
}

fn fixture(count: usize) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "aurora-sync-group-{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir(&root).unwrap();
    let managed = ManagedPaths::from_app_local_data_dir(root).unwrap();
    let mut registry = InstanceRegistry::empty();
    let mut ids = Vec::new();
    for (index, version) in ["26.3", "26.2", "26.1.2", "26.3"]
        .iter()
        .take(count)
        .enumerate()
    {
        let id = crate::instances::generate_instance_id();
        let mut configuration = InstanceConfiguration::for_minecraft_version(*version);
        configuration.set_aurora_enabled(false);
        let record = InstanceRecord::from_installed(
            id.clone(),
            format!("Instance {index}"),
            InstanceState::Ready,
            InstalledConfiguration {
                minecraft_version: (*version).into(),
                platform: PlatformPin::Vanilla {},
                aurora: None,
            },
            configuration,
        )
        .unwrap();
        registry.instances_mut().push(record);
        fs::create_dir_all(managed.instance_paths(&id).root()).unwrap();
        ids.push(id);
    }
    registry.save(&managed.instance_registry_file()).unwrap();
    Fixture { managed, ids }
}

fn put(f: &Fixture, i: usize, relative: &str, text: &str) {
    let path = f.managed.instance_paths(&f.ids[i]).root().join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn get(f: &Fixture, i: usize, relative: &str) -> String {
    fs::read_to_string(f.managed.instance_paths(&f.ids[i]).root().join(relative)).unwrap()
}
fn aurora(f: &Fixture, i: usize) -> serde_json::Map<String, Value> {
    serde_json::from_str(&get(f, i, "config/aurora.json")).unwrap()
}

/// Main is instance 0; every listed instance is a member.
fn enable(f: &Fixture, members: &[usize]) -> GroupConfig {
    save_config(
        &f.managed,
        GroupConfig {
            enabled: true,
            main_instance_id: Some(f.ids[0].clone()),
            members: members.iter().map(|i| f.ids[*i].clone()).collect(),
            ..GroupConfig::default()
        },
    )
    .unwrap()
}

const MAIN_OPTIONS: &str = "version:4790\nfov:0.75\nmouseSensitivity:0.6\nsoundDevice:\"Headset\"\nrenderDistance:16\nmodOnly:main\n";
const MAIN_AURORA: &str = r#"{"armorHudEnabled":true,"playtimeTotalMs":500,"waypointsByWorld":{"a":1},"keybindVersion":1,"freeLookKey":42}"#;

#[test]
fn joining_member_is_seeded_from_main_and_local_keys_stay() {
    let f = fixture(2);
    put(&f, 0, "options.txt", MAIN_OPTIONS);
    put(&f, 0, "config/aurora.json", MAIN_AURORA);
    put(
        &f,
        1,
        "options.txt",
        "version:4903\nfov:0.25\nmouseSensitivity:0.1\nsoundDevice:\"Speakers\"\nrenderDistance:8\nmodOnly:member\n",
    );
    put(
        &f,
        1,
        "config/aurora.json",
        r#"{"armorHudEnabled":false,"playtimeTotalMs":9,"waypointsByWorld":{"b":2},"keybindVersion":1,"freeLookKey":7}"#,
    );
    let group = enable(&f, &[0, 1]);
    assert_eq!(group.pending_seed, vec![f.ids[1].clone()]);

    let report = sync_now(&f.managed).unwrap();
    assert!(report.applied.contains(&f.ids[1].to_string()));
    let options = get(&f, 1, "options.txt");
    for line in ["fov:0.75", "mouseSensitivity:0.6", "renderDistance:16"] {
        assert!(options.contains(line), "{line} missing in {options}");
    }
    // Version, device and unknown mod keys stay per instance.
    for line in ["version:4903", "soundDevice:\"Speakers\"", "modOnly:member"] {
        assert!(options.contains(line), "{line} changed in {options}");
    }
    let config = aurora(&f, 1);
    assert_eq!(config["armorHudEnabled"], true);
    assert_eq!(config["freeLookKey"], 42);
    assert_eq!(config["playtimeTotalMs"], 9);
    assert_eq!(config["waypointsByWorld"], serde_json::json!({"b": 2}));
    assert!(super::config(&f.managed).unwrap().pending_seed.is_empty());
    // The member's previous files are backed up and listed in history.
    let entry = &history(&f.managed).unwrap()[0];
    assert_eq!(entry.kind, "seed");
    assert!(entry.files.contains(&"options.txt".to_owned()));
}

#[test]
fn an_edit_in_any_member_reaches_the_others_including_main() {
    let f = fixture(3);
    for i in 0..3 {
        put(&f, i, "options.txt", MAIN_OPTIONS);
    }
    enable(&f, &[0, 1, 2]);
    sync_now(&f.managed).unwrap();
    put(
        &f,
        2,
        "options.txt",
        &MAIN_OPTIONS.replace("fov:0.75", "fov:0.9"),
    );
    after_exit(&f.managed, &f.ids[2]).unwrap();
    sync_now(&f.managed).unwrap();
    assert!(get(&f, 0, "options.txt").contains("fov:0.9"));
    assert!(get(&f, 1, "options.txt").contains("fov:0.9"));
}

#[test]
fn newest_capture_wins_and_main_breaks_exact_ties() {
    let f = fixture(3);
    for i in 0..3 {
        put(&f, i, "options.txt", MAIN_OPTIONS);
    }
    let group = enable(&f, &[0, 1, 2]);
    sync_now(&f.managed).unwrap();
    let group = config(&f.managed).unwrap_or(group);
    let mut state: GroupState = read_json(&f.managed, &state_file(&f.managed)).unwrap();
    put(
        &f,
        1,
        "options.txt",
        &MAIN_OPTIONS.replace("fov:0.75", "fov:0.1"),
    );
    put(
        &f,
        2,
        "options.txt",
        &MAIN_OPTIONS.replace("fov:0.75", "fov:0.2"),
    );
    capture(&f.managed, &group, &mut state, &f.ids[2], 2_000).unwrap();
    capture(&f.managed, &group, &mut state, &f.ids[1], 1_000).unwrap();
    assert_eq!(state.values["o:fov"].value, "0.2");
    put(
        &f,
        0,
        "options.txt",
        &MAIN_OPTIONS.replace("fov:0.75", "fov:0.3"),
    );
    capture(&f.managed, &group, &mut state, &f.ids[0], 2_000).unwrap();
    assert_eq!(state.values["o:fov"].value, "0.3");
    put(
        &f,
        2,
        "options.txt",
        &MAIN_OPTIONS.replace("fov:0.75", "fov:0.4"),
    );
    capture(&f.managed, &group, &mut state, &f.ids[2], 2_000).unwrap();
    assert_eq!(
        state.values["o:fov"].value, "0.3",
        "an exact tie keeps the main's value"
    );
}

#[test]
fn keys_a_version_does_not_write_are_not_injected() {
    let f = fixture(2);
    put(&f, 0, "options.txt", MAIN_OPTIONS);
    put(&f, 1, "options.txt", "version:4790\nfov:0.5\n");
    enable(&f, &[0, 1]);
    sync_now(&f.managed).unwrap();
    assert_eq!(get(&f, 1, "options.txt"), "version:4790\nfov:0.75\n");
}

#[test]
fn key_codes_need_a_matching_codec_marker() {
    let f = fixture(2);
    put(&f, 0, "config/aurora.json", MAIN_AURORA);
    put(
        &f,
        1,
        "config/aurora.json",
        r#"{"armorHudEnabled":false,"freeLookKey":7}"#,
    );
    enable(&f, &[0, 1]);
    sync_now(&f.managed).unwrap();
    let config = aurora(&f, 1);
    assert_eq!(config["armorHudEnabled"], true);
    assert_eq!(
        config["freeLookKey"], 7,
        "legacy key codes are not overwritten"
    );
}

#[test]
fn profiles_are_synced_and_created_with_only_synced_fields() {
    let f = fixture(2);
    put(
        &f,
        0,
        "config/aurora.json",
        r#"{"armorHudEnabled":true,"activeProfile":"PvP","playtimeTotalMs":1}"#,
    );
    put(
        &f,
        0,
        "config/profiles/PvP.json",
        r#"{"name":"PvP","createdAtMs":5,"icon":"sword","fields":{"armorHudEnabled":true,"waypointsByWorld":{"x":1}}}"#,
    );
    put(
        &f,
        1,
        "config/aurora.json",
        r#"{"armorHudEnabled":false,"activeProfile":"default","playtimeTotalMs":2}"#,
    );
    enable(&f, &[0, 1]);
    sync_now(&f.managed).unwrap();
    assert_eq!(aurora(&f, 1)["activeProfile"], "PvP");
    let profile: Value = serde_json::from_str(&get(&f, 1, "config/profiles/PvP.json")).unwrap();
    assert_eq!(profile["name"], "PvP");
    assert_eq!(profile["icon"], "sword");
    assert_eq!(profile["fields"]["armorHudEnabled"], true);
    assert!(profile["fields"].get("waypointsByWorld").is_none());
}

#[test]
fn packs_are_pooled_without_replacing_anything() {
    let f = fixture(3);
    enable(&f, &[0, 1, 2]);
    put(&f, 1, "resourcepacks/Shared.zip", "from-1");
    put(&f, 2, "resourcepacks/Shared.zip", "kept-2");
    put(&f, 2, "shaderpacks/Folder/shaders/a.fsh", "shader");
    sync_now(&f.managed).unwrap();
    assert_eq!(get(&f, 0, "resourcepacks/Shared.zip"), "from-1");
    assert_eq!(get(&f, 2, "resourcepacks/Shared.zip"), "kept-2");
    assert_eq!(get(&f, 1, "shaderpacks/Folder/shaders/a.fsh"), "shader");
}

#[cfg(windows)]
#[test]
fn a_running_game_is_never_written() {
    let f = fixture(2);
    put(&f, 0, "options.txt", MAIN_OPTIONS);
    put(&f, 1, "options.txt", "version:4790\nfov:0.5\n");
    put(&f, 1, "logs/latest.log", "game running");
    enable(&f, &[0, 1]);
    let open = File::open(
        f.managed
            .instance_paths(&f.ids[1])
            .logs()
            .join("latest.log"),
    )
    .unwrap();
    let report = sync_now(&f.managed).unwrap();
    assert!(report.deferred.contains(&f.ids[1].to_string()));
    assert_eq!(get(&f, 1, "options.txt"), "version:4790\nfov:0.5\n");
    drop(open);
    sync_now(&f.managed).unwrap();
    assert!(get(&f, 1, "options.txt").contains("fov:0.75"));
}

#[test]
fn restore_puts_files_back_and_makes_the_instance_independent() {
    let f = fixture(2);
    put(&f, 0, "options.txt", MAIN_OPTIONS);
    put(&f, 1, "options.txt", "version:4790\nfov:0.5\n");
    enable(&f, &[0, 1]);
    sync_now(&f.managed).unwrap();
    let entry = history(&f.managed).unwrap()[0].clone();
    let group = restore(&f.managed, &entry.id).unwrap();
    assert_eq!(get(&f, 1, "options.txt"), "version:4790\nfov:0.5\n");
    assert!(!group.members.contains(&f.ids[1]));
    sync_now(&f.managed).unwrap();
    assert_eq!(get(&f, 1, "options.txt"), "version:4790\nfov:0.5\n");
}

#[test]
fn a_new_instance_joins_on_first_launch_and_is_seeded() {
    let f = fixture(2);
    put(&f, 0, "options.txt", MAIN_OPTIONS);
    put(&f, 1, "options.txt", "version:4790\nfov:0.5\n");
    let mut group = enable(&f, &[0]);
    // Instance 1 existed before the group, so it stays independent...
    assert!(before_launch(&f.managed, &f.ids[1]).unwrap().is_none());
    // ...but an instance created afterwards joins.
    group = config(&f.managed).unwrap_or(group);
    group.known.retain(|id| *id != f.ids[1]);
    write_json(&config_file(&f.managed), &group).unwrap();
    let report = before_launch(&f.managed, &f.ids[1]).unwrap().unwrap();
    assert!(report.applied.contains(&f.ids[1].to_string()));
    assert!(get(&f, 1, "options.txt").contains("fov:0.75"));
    assert!(config(&f.managed).unwrap().members.contains(&f.ids[1]));
}

#[test]
fn members_wait_until_the_main_has_been_captured() {
    let f = fixture(2);
    put(&f, 0, "options.txt", MAIN_OPTIONS);
    put(&f, 0, "logs/latest.log", "main running");
    put(&f, 1, "options.txt", "version:4790\nfov:0.5\n");
    enable(&f, &[0, 1]);
    let open = File::open(
        f.managed
            .instance_paths(&f.ids[0])
            .logs()
            .join("latest.log"),
    )
    .unwrap();
    let report = sync_now(&f.managed).unwrap();
    if cfg!(windows) {
        assert!(report.deferred.contains(&f.ids[1].to_string()));
        assert_eq!(get(&f, 1, "options.txt"), "version:4790\nfov:0.5\n");
    }
    drop(open);
}
