//! Test-only, fail-closed boundary for the disposable Windows symlink gate.
//! There is deliberately no cleanup operation: even successful fixtures remain
//! evidence. Neither a supplied path, a prefix, nor a marker grants deletion.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;

const PREFIX: &str = "aurora-f3rb1-symlink-";
const MARKER: &str = "F3RB1-ISOLATED.txt";
const MANIFEST: &str = "gate-manifest.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Fixture {
    schema_version: u32,
    fixture: String,
    slot: String,
    sentinel_sha256: String,
    authority_sha256: String,
    inventory: BTreeMap<String, Entry>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "camelCase")]
enum Entry {
    Directory,
    File { sha256: String },
    Link { target: PathBuf },
}

fn invalid() -> io::Error {
    io::Error::other("invalid, redirected, or incomplete isolated symlink fixture; preserved")
}

fn root_identity(root: &Path) -> io::Result<String> {
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(invalid)?;
    if !name.strip_prefix(PREFIX).is_some_and(journal::safe_uuid)
        || !root.is_absolute()
        || root.components().any(|p| {
            matches!(
                p,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
        || root != std::env::temp_dir().join(name)
        || root.parent().map(fs::canonicalize).transpose()?
            != Some(fs::canonicalize(std::env::temp_dir())?)
    {
        return Err(invalid());
    }
    Ok(name.to_owned())
}

fn marker(name: &str) -> Vec<u8> {
    format!("F3RB2a isolated file-symlink acceptance\nfixture={name}\n").into_bytes()
}

fn regular(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if redirected(&meta) || !meta.is_file() {
        return Err(invalid());
    }
    Ok(())
}

fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn inventory(root: &Path, slot: &str) -> io::Result<BTreeMap<String, Entry>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![(root.to_path_buf(), 0)];
    let backup = root
        .join("world")
        .join(journal::HISTORY_DIR)
        .join(slot)
        .join("previous-game");
    while let Some((directory, depth)) = pending.pop() {
        if depth > 32 {
            return Err(invalid());
        }
        let meta = fs::symlink_metadata(&directory)?;
        if redirected(&meta) || !meta.is_dir() {
            return Err(invalid());
        }
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| invalid())?
                .to_str()
                .ok_or_else(invalid)?
                .replace('\\', "/");
            let meta = fs::symlink_metadata(&path)?;
            // The manifest itself must be checked even though it cannot include
            // its own hash. Nothing else is omitted from the inventory.
            if relative == MANIFEST {
                regular(&path)?;
                continue;
            }
            let value = if redirected(&meta) {
                let target = if path == backup.join("redirected-file.txt") {
                    root.join("sentinel-target.bin")
                } else if path == backup.join("dangling-file.txt") {
                    root.join("sentinel-target.bin.absent")
                } else {
                    return Err(invalid());
                };
                if !meta.file_type().is_symlink() || fs::read_link(&path)? != target {
                    return Err(invalid());
                }
                Entry::Link { target }
            } else if meta.is_dir() {
                pending.push((path, depth + 1));
                Entry::Directory
            } else if meta.is_file() {
                if meta.len() > 4 * 1024 * 1024 {
                    return Err(invalid());
                }
                Entry::File {
                    sha256: hash(&fs::read(path)?),
                }
            } else {
                return Err(invalid());
            };
            result.insert(relative, value);
            if result.len() > 20_000 {
                return Err(invalid());
            }
        }
    }
    Ok(result)
}

fn validate(root: &Path) -> io::Result<Fixture> {
    let name = root_identity(root)?;
    let meta = fs::symlink_metadata(root)?;
    if redirected(&meta) || !meta.is_dir() {
        return Err(invalid());
    }
    for control in [MARKER, MANIFEST] {
        regular(&root.join(control))?;
    }
    if fs::read(root.join(MARKER))? != marker(&name) {
        return Err(invalid());
    }
    if fs::metadata(root.join(MANIFEST))?.len() > 4 * 1024 * 1024 {
        return Err(invalid());
    }
    let fixture: Fixture =
        serde_json::from_slice(&fs::read(root.join(MANIFEST))?).map_err(|_| invalid())?;
    if fixture.schema_version != 1
        || fixture.fixture != name
        || !journal::safe_uuid(&fixture.slot)
        || !digest_valid(&fixture.sentinel_sha256)
        || !digest_valid(&fixture.authority_sha256)
    {
        return Err(invalid());
    }
    // Enumerate without following any reparse point (except observing the two
    // exact intentional file links). This covers world/history/slot/control
    // ancestors before reading or mutating anything through them.
    if inventory(root, &fixture.slot)? != fixture.inventory {
        return Err(invalid());
    }
    if fs::read(root.join("world/owner-sentinel"))? != b"owner"
        || hash(&fs::read(root.join("sentinel-target.bin"))?) != fixture.sentinel_sha256
        || hash(&fs::read(root.join("world").join(journal::JOURNAL_FILE))?)
            != fixture.authority_sha256
    {
        return Err(invalid());
    }
    Ok(fixture)
}

fn elevated() -> io::Result<bool> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    let mut token = std::ptr::null_mut();
    // Query the actual process token; group membership is not elevation.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut information = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut length = 0;
    let size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&mut information as *mut TOKEN_ELEVATION).cast(),
            size,
            &mut length,
        )
    };
    let error = io::Error::last_os_error();
    unsafe {
        CloseHandle(token);
    }
    if ok == 0 {
        return Err(error);
    }
    if length != size {
        return Err(io::Error::other(
            "unexpected process token elevation result",
        ));
    }
    Ok(information.TokenIsElevated != 0)
}

fn require_mode(phase: &str, elevation: io::Result<bool>) -> io::Result<&'static str> {
    let is_elevated = elevation?;
    if phase == "verify" && is_elevated {
        return Err(io::Error::other(
            "elevated verify rejected; fixture preserved",
        ));
    }
    Ok(if is_elevated {
        "ELEVATED"
    } else {
        "UNELEVATED"
    })
}

fn world(root: &Path) -> World {
    World {
        root: root.join("world"),
        id: InstanceId::new("f3rb1-symlink").unwrap(),
        cleanup: false,
    }
}

fn prepare(root: &Path) -> io::Result<()> {
    let name = root_identity(root)?;
    // create_dir is exclusive, including for dangling roots. No overwrite.
    fs::create_dir(root)?;
    fs::write(root.join(MARKER), marker(&name))?;
    let world = world(root);
    fs::create_dir(&world.root)?;
    fs::write(world.root.join("owner-sentinel"), b"owner")?;
    let (slot, _, _) = world.three();
    let sentinel = root.join("sentinel-target.bin");
    fs::write(&sentinel, b"sentinel-bytes-f3rb1")?;
    let backup = world.slot(&slot).join("previous-game");
    for (name, target) in [
        ("redirected-file.txt", sentinel.clone()),
        ("dangling-file.txt", root.join("sentinel-target.bin.absent")),
    ] {
        std::os::windows::fs::symlink_file(target, backup.join(name)).map_err(|e| {
            io::Error::other(format!(
                "BLOCKED: real file symbolic-link creation failed: {e}; fixture preserved"
            ))
        })?;
    }
    let fixture = Fixture {
        schema_version: 1,
        fixture: name,
        slot: slot.clone(),
        sentinel_sha256: hash(&fs::read(sentinel)?),
        authority_sha256: hash(&fs::read(world.root.join(journal::JOURNAL_FILE))?),
        inventory: inventory(root, &slot)?,
    };
    fs::write(root.join(MANIFEST), serde_json::to_vec_pretty(&fixture)?)?;
    validate(root)?;
    eprintln!(
        "SYMLINK-GATE-HELPER: PREPARED links=real root={}",
        root.display()
    );
    Ok(())
}

fn verify(root: &Path) -> io::Result<()> {
    let fixture = validate(root)?;
    let world = world(root);
    let backup = world.slot(&fixture.slot).join("previous-game");
    for name in ["redirected-file.txt", "dangling-file.txt"] {
        if !fs::symlink_metadata(backup.join(name))?
            .file_type()
            .is_symlink()
        {
            return Err(invalid());
        }
    }
    if root.join("sentinel-target.bin.absent").try_exists()? {
        return Err(invalid());
    }
    let before = inventory(root, &fixture.slot)?;
    for attempt in 1..=2 {
        let result = world.run(u64::MAX);
        assert_eq!(result.reclaimed_bytes, 0);
        assert_eq!(result.retired_backups, 0);
        assert!(
            result
                .entries
                .iter()
                .any(|e| e.transaction_id == fixture.slot
                    && e.outcome == Outcome::RetainedUnknownOwnership)
        );
        assert_eq!(
            inventory(root, &fixture.slot)?,
            before,
            "attempt {attempt}: inventory, sentinel hashes and authority must remain unchanged"
        );
        eprintln!(
            "[f3rb1-symlink] attempt={attempt} refused=true reclaimed=0 inventoryUnchanged=true authorityUnchanged=true"
        );
    }
    assert!(
        matches!(super::super::super::validate_game_directory(&world.root.join("game")), Ok(ValidationOutcome::Installed(result)) if result.status == ValidationStatus::Valid)
    );
    Ok(())
}

pub(super) fn run_from_environment() -> io::Result<()> {
    let phase = std::env::var("AURORA_F3RB1_SYMLINK_PHASE").unwrap_or_default();
    if !["", "prepare", "verify"].contains(&phase.as_str()) {
        return Err(invalid());
    }
    // Token errors/elevated verify fail before root access, retention, or cleanup.
    let mode = require_mode(&phase, elevated())?;
    eprintln!("SYMLINK-GATE-HELPER: MODE={mode} phase={phase}");
    let root = PathBuf::from(std::env::var_os("AURORA_F3RB1_SYMLINK_ROOT").ok_or_else(invalid)?);
    if phase != "verify" {
        prepare(&root)?;
    }
    if phase != "prepare" {
        verify(&root)?;
        eprintln!(
            "SYMLINK-GATE-HELPER: VERIFIED mode={mode} links=real dangling=real attempts=2 reclaimed=0 currentValid=true"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> PathBuf {
        let root = std::env::temp_dir().join(format!("{PREFIX}{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("sentinel-target.bin"),
            b"negative-test sentinel\0bytes",
        )
        .unwrap();
        root
    }

    // This is an ownership-validation fixture, never security acceptance:
    // it intentionally has no links or real game; verify must reject it.
    fn controls(root: &Path, slot: &str) {
        let name = root_identity(root).unwrap();
        fs::write(root.join(MARKER), marker(&name)).unwrap();
        fs::create_dir(root.join("world")).unwrap();
        fs::write(root.join("world/owner-sentinel"), b"owner").unwrap();
        fs::write(
            root.join("world").join(journal::JOURNAL_FILE),
            b"authority bytes",
        )
        .unwrap();
        let fixture = Fixture {
            schema_version: 1,
            fixture: name,
            slot: slot.into(),
            sentinel_sha256: hash(&fs::read(root.join("sentinel-target.bin")).unwrap()),
            authority_sha256: hash(b"authority bytes"),
            inventory: inventory(root, slot).unwrap(),
        };
        fs::write(root.join(MANIFEST), serde_json::to_vec(&fixture).unwrap()).unwrap();
    }

    // Observation only, never follows a redirected entry. Compare actual bytes
    // (not just hashes) and directory inventory across every rejection.
    fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut result = BTreeMap::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(path) = pending.pop() {
            let meta = fs::symlink_metadata(&path).unwrap();
            let value = if redirected(&meta) {
                format!("redirect:{:?}", fs::read_link(&path).unwrap()).into_bytes()
            } else if meta.is_dir() {
                pending.extend(fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
                b"directory".to_vec()
            } else {
                fs::read(&path).unwrap()
            };
            result.insert(path, value);
        }
        result
    }

    fn rejected_unchanged(root: &Path) {
        let before = snapshot(root);
        assert!(verify(root).is_err());
        assert_eq!(
            snapshot(root),
            before,
            "all sentinel bytes and inventory preserved"
        );
    }

    #[test]
    fn unrelated_and_prefix_only_roots_are_preserved() {
        let unrelated =
            std::env::temp_dir().join(format!("aurora-f3rb2a-unrelated-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&unrelated).unwrap();
        fs::write(unrelated.join("sentinel"), b"unrelated sentinel").unwrap();
        rejected_unchanged(&unrelated);
        let root = fresh();
        rejected_unchanged(&root);
        let before = snapshot(&root);
        assert!(prepare(&root).is_err());
        assert_eq!(
            snapshot(&root),
            before,
            "prepare never overwrites an existing prefix root"
        );
        // Positively created exact disposable roots; no caller-root cleanup.
        fs::remove_dir_all(unrelated).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn marker_manifest_and_slot_rejections_preserve_every_byte() {
        for defect in [
            "marker-missing",
            "marker-wrong",
            "manifest-missing",
            "manifest-malformed",
            "schema",
            "identity",
            "digest",
            "slot-empty",
            "slot-traversal",
            "slot-absolute",
            "slot-backslash",
            "slot-ads",
            "inventory",
        ] {
            let root = fresh();
            controls(&root, "0123456789abcdef0123456789abcdef");
            match defect {
                "marker-missing" => fs::remove_file(root.join(MARKER)).unwrap(),
                "marker-wrong" => fs::write(root.join(MARKER), b"wrong marker").unwrap(),
                "manifest-missing" => fs::remove_file(root.join(MANIFEST)).unwrap(),
                "manifest-malformed" => fs::write(root.join(MANIFEST), b"{broken").unwrap(),
                "inventory" => {
                    fs::write(root.join("unexpected-sentinel"), b"extra owner bytes").unwrap()
                }
                other => {
                    let mut doc: serde_json::Value =
                        serde_json::from_slice(&fs::read(root.join(MANIFEST)).unwrap()).unwrap();
                    let (field, value) = match other {
                        "schema" => ("schemaVersion", serde_json::json!(2)),
                        "identity" => ("fixture", serde_json::json!("different fixture")),
                        "digest" => ("sentinelSha256", serde_json::json!("invalid")),
                        "slot-empty" => ("slot", serde_json::json!("")),
                        "slot-traversal" => ("slot", serde_json::json!("../../outside")),
                        "slot-absolute" => ("slot", serde_json::json!("C:/outside")),
                        "slot-backslash" => ("slot", serde_json::json!("..\\outside")),
                        _ => ("slot", serde_json::json!("slot:stream")),
                    };
                    doc[field] = value;
                    fs::write(root.join(MANIFEST), serde_json::to_vec(&doc).unwrap()).unwrap();
                }
            }
            rejected_unchanged(&root);
            fs::remove_dir_all(root).unwrap();
        }
    }

    fn junction(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(status.status.success(), "junction creation must execute");
    }

    #[test]
    fn redirected_root_and_control_ancestors_are_preserved() {
        for control in ["root", "world", MARKER, MANIFEST] {
            let root = fresh();
            let target = fresh();
            controls(&root, "0123456789abcdef0123456789abcdef");
            let path = if control == "root" {
                root.clone()
            } else {
                root.join(control)
            };
            let saved = if control == "root" {
                root.with_extension("saved")
            } else {
                root.join(format!("{control}.saved"))
            };
            fs::rename(&path, &saved).unwrap();
            junction(&path, &target);
            let before = snapshot(&target);
            rejected_unchanged(&root);
            assert_eq!(
                snapshot(&target),
                before,
                "junction target sentinel bytes/inventory preserved"
            );
            fs::remove_dir(&path).unwrap(); // junction only, never its target
            fs::rename(saved, path).unwrap();
            fs::remove_dir_all(root).unwrap();
            fs::remove_dir_all(target).unwrap();
        }
    }

    #[test]
    fn failed_verification_and_world_unwinding_preserve_fixture() {
        let root = fresh();
        controls(&root, "0123456789abcdef0123456789abcdef");
        assert!(validate(&root).is_ok());
        rejected_unchanged(&root); // missing REAL links fails before retention
        let before = snapshot(&root);
        let result = std::panic::catch_unwind(|| {
            let _world = world(&root);
            panic!("injected verification assertion failure");
        });
        assert!(result.is_err());
        assert_eq!(
            snapshot(&root),
            before,
            "World::drop cannot delete a gate fixture during unwind"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn token_decision_fails_closed_and_observes_current_process() {
        assert_eq!(require_mode("verify", Ok(false)).unwrap(), "UNELEVATED");
        assert!(require_mode("verify", Ok(true)).is_err());
        assert!(require_mode("verify", Err(io::Error::other("injected token failure"))).is_err());
        assert_eq!(require_mode("", Ok(true)).unwrap(), "ELEVATED");
        assert_eq!(require_mode("prepare", Ok(true)).unwrap(), "ELEVATED");
        eprintln!(
            "[f3rb2a-token] actual_mode={}",
            require_mode("", elevated()).unwrap()
        );
    }
}
