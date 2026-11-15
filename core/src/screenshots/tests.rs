use super::*;
use std::time::{Duration, SystemTime};

struct Fixture {
    root: PathBuf,
    paths: ManagedPaths,
    instance: InstanceId,
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("aurora-screenshots-{}", uuid::Uuid::new_v4()));
        let paths = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let instance = InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        let dir = paths.instance_paths(&instance).root().join("screenshots");
        fs::create_dir_all(&dir).unwrap();
        Self {
            root,
            paths,
            instance,
            dir,
        }
    }
    fn image(&self, name: &str) -> PathBuf {
        let path = self.dir.join(name);
        image::RgbaImage::from_pixel(640, 360, image::Rgba([30, 80, 150, 255]))
            .save(&path)
            .unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(
            self.root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-screenshots-")
        );
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn validates_windows_leaf_names_and_image_extensions() {
    for name in [
        "../escape.png",
        "..\\escape.png",
        "C:\\image.png",
        "stream.png:ads",
        "a/b.jpg",
        "CON.png",
        "aux.jpg",
        "COM1.png",
        "LPT¹.png",
        ".png",
        "a.png.",
        "a.png ",
        "a\0.png",
        "a.txt",
        "a?.png",
        "",
    ] {
        assert!(validate_name(name).is_err(), "{name:?}");
    }
    for name in [
        "scoreboard.png",
        "practice 2.JPG",
        "勝利.jpeg",
        "2026-11-15_12.30.00.PNG",
    ] {
        assert!(validate_name(name).is_ok(), "{name:?}");
    }
}
#[test]
fn lists_only_images_newest_first_with_stable_ties_and_missing_directory_is_empty() {
    let f = Fixture::new();
    let a = f.image("a.png");
    let b = f.image("b.png");
    let c = f.image("c.png");
    let stamp = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    fs::File::options()
        .write(true)
        .open(a)
        .unwrap()
        .set_modified(stamp)
        .unwrap();
    for path in [b, c] {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(stamp + Duration::from_secs(10))
            .unwrap();
    }
    fs::write(f.dir.join("ignored.txt"), "text").unwrap();
    fs::create_dir(f.dir.join("directory.png")).unwrap();
    let shots = list(&f.paths, &f.instance).unwrap();
    assert_eq!(
        shots.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["b.png", "c.png", "a.png"]
    );
    assert!(shots.iter().all(|s| s.bytes > 0));
    let other = InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
    assert!(list(&f.paths, &other).unwrap().is_empty());
}
#[test]
fn containment_and_rename_no_clobber_and_changed_files() {
    let f = Fixture::new();
    f.image("old.png");
    f.image("taken.png");
    let old = list(&f.paths, &f.instance)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "old.png")
        .unwrap();
    for name in ["../escape.png", "taken.png", "new.jpg"] {
        assert!(rename(&f.paths, &f.instance, &old, name).is_err());
    }
    let mut traversal = old.clone();
    traversal.name = "../outside.png".into();
    assert!(bitmap(&f.paths, &f.instance, &traversal, false).is_err());
    assert!(action(&f.paths, &f.instance, &traversal, Action::Delete, true).is_err());
    rename(&f.paths, &f.instance, &old, "new.png").unwrap();
    assert!(!f.dir.join("old.png").exists());
    assert!(f.dir.join("new.png").is_file());
    let current = list(&f.paths, &f.instance)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "new.png")
        .unwrap();
    fs::write(f.dir.join("new.png"), b"changed").unwrap();
    assert!(bitmap(&f.paths, &f.instance, &current, false).is_err());
    assert!(rename(&f.paths, &f.instance, &current, "again.png").is_err());
}
#[test]
fn thumbnails_are_bounded_and_cache_identity_invalidates_on_path_size_time_and_side() {
    let f = Fixture::new();
    let path = f.image("a.png");
    let shot = list(&f.paths, &f.instance).unwrap().remove(0);
    let bitmap = bitmap(&f.paths, &f.instance, &shot, false).unwrap();
    assert_eq!((bitmap.width, bitmap.height), (256, 144));
    assert_eq!(bitmap.rgba.len(), 256 * 144 * 4);
    let key = CacheKey::new(path.clone(), &shot, 256);
    let mut cache = ThumbnailCache::default();
    cache.insert(key.clone(), Arc::new(bitmap));
    assert!(cache.get(&key).is_some());
    let mut changed = shot.clone();
    changed.bytes += 1;
    assert!(
        cache
            .get(&CacheKey::new(path.clone(), &changed, 256))
            .is_none()
    );
    changed = shot.clone();
    changed.modified_ns += 1;
    assert!(
        cache
            .get(&CacheKey::new(path.clone(), &changed, 256))
            .is_none()
    );
    assert!(
        cache
            .get(&CacheKey::new(path.with_file_name("b.png"), &shot, 256))
            .is_none()
    );
    assert!(
        cache
            .get(&CacheKey::new(path.clone(), &shot, 1920))
            .is_none()
    );
    cache.insert(
        CacheKey::new(path, &changed, 256),
        Arc::new(Bitmap {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        }),
    );
    assert!(cache.get(&key).is_none());
    for n in 0..110 {
        cache.insert(
            CacheKey::new(PathBuf::from(n.to_string()), &shot, 256),
            Arc::new(Bitmap {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            }),
        );
    }
    assert_eq!(cache.0.len(), 96);
}
#[test]
fn in_memory_cache_refreshes_pixels_after_a_file_change() {
    let f = Fixture::new();
    let path = f.image("a.png");
    let original = list(&f.paths, &f.instance).unwrap().remove(0);
    let before = bitmap(&f.paths, &f.instance, &original, false).unwrap();
    image::RgbaImage::from_pixel(640, 360, image::Rgba([220, 10, 20, 255]))
        .save(&path)
        .unwrap();
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(5))
        .unwrap();
    let changed = list(&f.paths, &f.instance).unwrap().remove(0);
    assert_ne!(original, changed);
    assert!(bitmap(&f.paths, &f.instance, &original, false).is_err());
    let after = bitmap(&f.paths, &f.instance, &changed, false).unwrap();
    assert_ne!(before.rgba, after.rgba);
}
#[test]
fn malformed_missing_and_oversized_images_fail_without_mutation() {
    let f = Fixture::new();
    fs::write(f.dir.join("partial.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    let shot = list(&f.paths, &f.instance).unwrap().remove(0);
    assert!(bitmap(&f.paths, &f.instance, &shot, false).is_err());
    fs::remove_file(f.dir.join("partial.png")).unwrap();
    assert!(bitmap(&f.paths, &f.instance, &shot, false).is_err());
    let file = fs::File::create(f.dir.join("large.png")).unwrap();
    file.set_len(MAX_FILE + 1).unwrap();
    let shot = list(&f.paths, &f.instance).unwrap().remove(0);
    assert!(bitmap(&f.paths, &f.instance, &shot, false).is_err());
    let wide = f.dir.join("too-wide.png");
    image::RgbaImage::from_pixel(16_385, 1, image::Rgba([0, 0, 0, 255]))
        .save(&wide)
        .unwrap();
    let shot = describe(&wide, "too-wide.png".into()).unwrap();
    assert!(bitmap(&f.paths, &f.instance, &shot, false).is_err());
}
#[test]
fn jpeg_is_supported_and_preview_is_bounded() {
    let f = Fixture::new();
    image::RgbImage::from_pixel(2200, 1200, image::Rgb([90, 40, 20]))
        .save(f.dir.join("a.jpg"))
        .unwrap();
    let shot = list(&f.paths, &f.instance).unwrap().remove(0);
    let preview = bitmap(&f.paths, &f.instance, &shot, true).unwrap();
    assert_eq!(preview.width, 1920);
    assert!(preview.height <= 1920);
}
#[test]
fn delete_requires_explicit_confirmation_before_any_file_access() {
    let f = Fixture::new();
    f.image("a.png");
    let shot = list(&f.paths, &f.instance).unwrap().remove(0);
    assert!(action(&f.paths, &f.instance, &shot, Action::Delete, false).is_err());
    assert!(f.dir.join("a.png").is_file());
}
#[test]
fn clipboard_file_drop_is_utf16_double_terminated_and_never_a_text_path() {
    let f = Fixture::new();
    let path = f.dir.join("勝利.png");
    let payload = file_drop_payload(&path).unwrap();
    assert_eq!(&payload[..4], &20u32.to_le_bytes());
    assert_eq!(&payload[16..20], &1u32.to_le_bytes());
    let wide: Vec<_> = payload[20..]
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
    assert_eq!(&wide[wide.len() - 2..], &[0, 0]);
    assert_eq!(
        String::from_utf16(&wide[..wide.len() - 2]).unwrap(),
        path.to_str().unwrap()
    );
    assert!(file_drop_payload(Path::new("relative.png")).is_err());
}

#[cfg(windows)]
#[test]
fn shell_paths_preserve_unicode_and_normalize_canonical_disk_and_unc_prefixes() {
    assert_eq!(
        windows::shell_path(Path::new(r"\\?\C:\fixture\勝利.png")),
        PathBuf::from(r"C:\fixture\勝利.png")
    );
    assert_eq!(
        windows::shell_path(Path::new(r"\\?\UNC\server\share\a.png")),
        PathBuf::from(r"\\server\share\a.png")
    );
}

#[cfg(windows)]
#[test]
#[ignore = "Windows shell acceptance: recycles only a generated temp fixture"]
fn recycle_bin_acceptance_preserves_unconfirmed_file_then_recycles_confirmed_fixture() {
    let f = Fixture::new();
    f.image("recycle-fixture.png");
    let shot = list(&f.paths, &f.instance).unwrap().remove(0);
    assert!(action(&f.paths, &f.instance, &shot, Action::Delete, false).is_err());
    assert!(f.dir.join(&shot.name).is_file());
    action(&f.paths, &f.instance, &shot, Action::Delete, true).unwrap();
    assert!(!f.dir.join(&shot.name).exists());
}
#[test]
fn clipboard_bitmap_has_v5_header_top_down_bgra_and_alpha_masks() {
    let bytes = dib_payload(2, 1, &[10, 20, 30, 255, 40, 50, 60, 128]).unwrap();
    assert_eq!(bytes.len(), 132);
    assert_eq!(&bytes[..4], &124u32.to_le_bytes());
    assert_eq!(&bytes[8..12], &(-1i32).to_le_bytes());
    assert_eq!(&bytes[14..16], &32u16.to_le_bytes());
    assert_eq!(&bytes[52..56], &0xff000000u32.to_le_bytes());
    assert_eq!(&bytes[124..], &[30, 20, 10, 255, 60, 50, 40, 128]);
    assert!(dib_payload(2, 1, &[0; 4]).is_err());
    assert!(dib_payload(0, 0, &[]).is_err());
}
#[test]
fn utc_date_is_readable() {
    assert_eq!(date(0), "1970-01-01 00:00 UTC");
    assert_eq!(date(1_794_700_800_000_000_000), "2026-11-15 00:00 UTC");
}

#[cfg(windows)]
#[test]
fn refuses_junction_ancestors_and_symlink_leaves() {
    use std::os::windows::fs::symlink_file;
    let f = Fixture::new();
    let target = f.root.join("outside");
    fs::create_dir(&target).unwrap();
    let sentinel = target.join("sentinel.png");
    fs::write(&sentinel, b"sentinel").unwrap();
    // A junction needs no symlink privilege; all paths belong to this fixture.
    fs::remove_dir(&f.dir).unwrap();
    let script = "New-Item -ItemType Junction -Path $env:AURORA_SHOT_LINK -Target $env:AURORA_SHOT_TARGET | Out-Null";
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("AURORA_SHOT_LINK", &f.dir)
        .env("AURORA_SHOT_TARGET", &target)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(list(&f.paths, &f.instance).is_err());
    let shot = Screenshot {
        name: "sentinel.png".into(),
        bytes: 8,
        modified_ns: 0,
    };
    assert!(rename(&f.paths, &f.instance, &shot, "renamed.png").is_err());
    assert!(action(&f.paths, &f.instance, &shot, Action::Delete, true).is_err());
    assert_eq!(fs::read(&sentinel).unwrap(), b"sentinel");
    fs::remove_dir(&f.dir).unwrap();
    fs::create_dir(&f.dir).unwrap();
    if symlink_file(&sentinel, f.dir.join("link.png")).is_ok() {
        assert!(list(&f.paths, &f.instance).unwrap().is_empty());
        let shot = Screenshot {
            name: "link.png".into(),
            bytes: 8,
            modified_ns: 0,
        };
        assert!(bitmap(&f.paths, &f.instance, &shot, false).is_err());
        fs::remove_file(f.dir.join("link.png")).unwrap();
    }
}
