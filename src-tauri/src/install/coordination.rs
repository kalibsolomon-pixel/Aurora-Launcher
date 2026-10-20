//! Kernel-owned resource locks. Lock files are permanent rendezvous objects,
//! not ownership records: never unlink one to "clear" a stale lock.
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub(crate) enum LockError {
    Busy,
    Io(io::Error),
}

impl From<io::Error> for LockError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Exclusive opens contend even in one process; explicit shared readers may
/// coexist. Descriptors are never cloned or inherited by children, and closing
/// them releases OS ownership after a crash.
#[derive(Debug)]
pub(crate) struct TargetLock {
    _file: File,
    parent: PathBuf,
}

impl TargetLock {
    pub(crate) fn acquire(root: &Path, parent: &Path, name: &str) -> Result<Self, LockError> {
        Self::open(root, parent, name, true, false)
    }

    /// Observe existing exclusive ownership without creating a rendezvous file.
    /// Lifecycle callers can perform recovery while holding this lock.
    pub(crate) fn existing(
        root: &Path,
        parent: &Path,
        name: &str,
    ) -> Result<Option<Self>, LockError> {
        match Self::open(root, parent, name, false, false) {
            Ok(guard) => Ok(Some(guard)),
            Err(LockError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Only byte-validation readers use this mode. It excludes every writer
    /// while allowing other read-only validations of the same installed tree.
    pub(crate) fn existing_shared(
        root: &Path,
        parent: &Path,
        name: &str,
    ) -> Result<Option<Self>, LockError> {
        match Self::open(root, parent, name, false, true) {
            Ok(guard) => Ok(Some(guard)),
            Err(LockError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn open(
        root: &Path,
        parent: &Path,
        name: &str,
        create: bool,
        shared: bool,
    ) -> Result<Self, LockError> {
        let parent = if create {
            ensure_directory(root, parent)?
        } else {
            checked_directory(root, parent)?
        };
        let path = parent.join(name);
        require_regular_or_missing(&path)?;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(create)
            .truncate(false);
        // Keeping the file open must also prevent deletion/replacement of the
        // rendezvous object on Windows. Do not use FILE_SHARE_DELETE.
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0x1 | 0x2);
            options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(&path)?;
        let metadata = file.metadata()?;
        if redirected(&metadata) || !metadata.is_file() {
            return Err(unsafe_path().into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() != 1 {
                return Err(unsafe_path().into());
            }
        }
        let locked = if shared {
            file.try_lock_shared()
        } else {
            file.try_lock()
        };
        match locked {
            Ok(()) => Ok(Self {
                _file: file,
                parent,
            }),
            Err(std::fs::TryLockError::WouldBlock) => Err(LockError::Busy),
            Err(std::fs::TryLockError::Error(error)) => Err(LockError::Io(error)),
        }
    }

    pub(crate) fn recheck(&self, root: &Path, parent: &Path) -> io::Result<()> {
        if checked_directory(root, parent)? != self.parent {
            return Err(unsafe_path());
        }
        Ok(())
    }
}

pub(crate) fn redirected(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

pub(crate) fn unsafe_path() -> io::Error {
    io::Error::other(
        "a managed operation path is redirected, conflicting, or outside its authorized root",
    )
}

pub(crate) fn require_regular_or_missing(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !redirected(&meta) => Ok(()),
        Ok(_) => Err(unsafe_path()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Canonicalize only the platform-granted root; managed descendants may never
/// redirect. Aliases of that root still meet at the same physical lock file.
pub(crate) fn ensure_directory(root: &Path, path: &Path) -> io::Result<PathBuf> {
    directory(root, path, true)
}

pub(crate) fn checked_directory(root: &Path, path: &Path) -> io::Result<PathBuf> {
    directory(root, path, false)
}

fn directory(root: &Path, path: &Path, create: bool) -> io::Result<PathBuf> {
    if !root.is_absolute() {
        return Err(unsafe_path());
    }
    let relative = path.strip_prefix(root).map_err(|_| unsafe_path())?;
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(unsafe_path());
    }
    if create {
        std::fs::create_dir_all(root)?;
    }
    let canonical_root = std::fs::canonicalize(root)?;
    #[cfg(unix)]
    let root_device = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&canonical_root)?.dev()
    };
    let mut current = canonical_root.clone();
    for part in relative.components() {
        current.push(part);
        if create {
            match std::fs::create_dir(&current) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        let metadata = std::fs::symlink_metadata(&current)?;
        if !metadata.is_dir() || redirected(&metadata) {
            return Err(unsafe_path());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != root_device {
                return Err(unsafe_path());
            }
        }
    }
    let canonical = std::fs::canonicalize(&current)?;
    if !canonical.starts_with(&canonical_root) {
        return Err(unsafe_path());
    }
    Ok(canonical)
}

/// Refuse links anywhere before recursive disposal or tree hashing. Bounded
/// depth/count prevents untrusted filesystem evidence exhausting the stack.
pub(crate) fn checked_tree(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![(path.to_path_buf(), 0)];
    let mut count = 0usize;
    while let Some((current, depth)) = pending.pop() {
        count += 1;
        if depth > 64 || count > 200_000 {
            return Err(unsafe_path());
        }
        let meta = std::fs::symlink_metadata(&current)?;
        if redirected(&meta) {
            return Err(unsafe_path());
        }
        if meta.is_dir() {
            for child in std::fs::read_dir(current)? {
                pending.push((child?.path(), depth + 1));
            }
        } else if meta.is_file() {
            files.push(current);
        } else {
            return Err(unsafe_path());
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[test]
    fn process_lock_helper() {
        let Some(root) = std::env::var_os("AURORA_F3R_LOCK_TEST_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let guard = if std::env::var_os("AURORA_F3R_LOCK_TEST_SHARED").is_some() {
            TargetLock::existing_shared(&root, &root.join("target"), ".lock")
                .unwrap()
                .unwrap()
        } else {
            TargetLock::acquire(&root, &root.join("target"), ".lock").unwrap()
        };
        std::fs::write(root.join("held"), std::process::id().to_string()).unwrap();
        // Parent kills this exact independent process, not its executable name.
        loop {
            std::thread::sleep(Duration::from_millis(20));
            std::hint::black_box(&guard);
        }
    }

    #[test]
    fn independent_process_contention_death_and_independent_targets() {
        let root = std::env::temp_dir().join(format!("aurora-f3r-lock-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "install::coordination::tests::process_lock_helper",
                "--nocapture",
            ])
            .env("AURORA_F3R_LOCK_TEST_ROOT", &root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !root.join("held").exists() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let held = root.join("held").exists();
        let blocked = matches!(
            TargetLock::acquire(&root, &root.join("target"), ".lock"),
            Err(LockError::Busy)
        );
        let other = TargetLock::acquire(&root, &root.join("other"), ".lock");
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(held && blocked && other.is_ok());
        let after_death = TargetLock::acquire(&root, &root.join("target"), ".lock").unwrap();
        drop(after_death);
        drop(other);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn shared_readers_coexist_without_creating_paths_and_still_exclude_writers() {
        let root =
            std::env::temp_dir().join(format!("aurora-shared-read-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("target");
        assert!(
            TargetLock::existing_shared(&root, &target, ".lock")
                .unwrap()
                .is_none()
        );
        assert!(!target.exists());
        let writer = TargetLock::acquire(&root, &target, ".lock").unwrap();
        assert!(matches!(
            TargetLock::existing_shared(&root, &target, ".lock"),
            Err(LockError::Busy)
        ));
        drop(writer);
        let first = TargetLock::existing_shared(&root, &target, ".lock")
            .unwrap()
            .unwrap();
        let second = TargetLock::existing_shared(&root, &target, ".lock")
            .unwrap()
            .unwrap();
        assert!(matches!(
            TargetLock::acquire(&root, &target, ".lock"),
            Err(LockError::Busy)
        ));
        drop(first);
        assert!(matches!(
            TargetLock::acquire(&root, &target, ".lock"),
            Err(LockError::Busy)
        ));
        drop(second);
        drop(TargetLock::acquire(&root, &target, ".lock").unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn independent_shared_reader_releases_exclusion_after_process_death() {
        let root =
            std::env::temp_dir().join(format!("aurora-shared-process-{}", uuid::Uuid::new_v4()));
        let target = root.join("target");
        drop(TargetLock::acquire(&root, &target, ".lock").unwrap());
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "install::coordination::tests::process_lock_helper",
                "--nocapture",
            ])
            .env("AURORA_F3R_LOCK_TEST_ROOT", &root)
            .env("AURORA_F3R_LOCK_TEST_SHARED", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !root.join("held").exists() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let held = root.join("held").exists();
        let reader = TargetLock::existing_shared(&root, &target, ".lock");
        let concurrent = matches!(reader, Ok(Some(_)));
        let blocked = matches!(
            TargetLock::acquire(&root, &target, ".lock"),
            Err(LockError::Busy)
        );
        drop(reader);
        let still_blocked = matches!(
            TargetLock::acquire(&root, &target, ".lock"),
            Err(LockError::Busy)
        );
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(held && concurrent && blocked && still_blocked);
        drop(TargetLock::acquire(&root, &target, ".lock").unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn handles_release_on_error_and_root_aliases_contend() {
        let root = std::env::temp_dir().join(format!("aurora-f3r-lock-{}", uuid::Uuid::new_v4()));
        let guard = TargetLock::acquire(&root, &root.join("target"), ".lock").unwrap();
        let alias = std::fs::canonicalize(&root).unwrap();
        assert!(matches!(
            TargetLock::acquire(&alias, &alias.join("target"), ".lock"),
            Err(LockError::Busy)
        ));
        assert!(TargetLock::acquire(&root, &root.join("target/../escape"), ".lock").is_err());
        drop(guard);
        drop(TargetLock::acquire(&root, &root.join("target"), ".lock").unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_case_and_junction_aliases_contend_but_redirected_descendants_are_rejected() {
        use std::os::windows::process::CommandExt;
        let root =
            std::env::temp_dir().join(format!("aurora-f3r-junction-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("physical")).unwrap();
        std::fs::create_dir(root.join("external")).unwrap();
        std::fs::write(root.join("external/sentinel"), b"owner").unwrap();
        let status = Command::new("powershell.exe").creation_flags(0x08000000)
            .env("F3R_JUNCTION_ROOT", &root)
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path ($env:F3R_JUNCTION_ROOT + '\\alias') -Target ($env:F3R_JUNCTION_ROOT + '\\physical') | Out-Null; New-Item -ItemType Junction -Path ($env:F3R_JUNCTION_ROOT + '\\physical\\redirected') -Target ($env:F3R_JUNCTION_ROOT + '\\external') | Out-Null"])
            .status().unwrap();
        assert!(status.success());
        let physical = root.join("physical");
        let held = TargetLock::acquire(&physical, &physical.join("target"), ".lock").unwrap();
        for alias in [
            root.join("alias"),
            PathBuf::from(physical.to_string_lossy().to_lowercase()),
        ] {
            assert!(matches!(
                TargetLock::acquire(&alias, &alias.join("target"), ".lock"),
                Err(LockError::Busy)
            ));
        }
        assert!(TargetLock::acquire(&physical, &physical.join("redirected"), ".lock").is_err());
        assert_eq!(
            std::fs::read(root.join("external/sentinel")).unwrap(),
            b"owner"
        );
        drop(held);
        // Remove only the two positively-created junction names before the
        // unique disposable root. Neither operation traverses their targets.
        std::fs::remove_dir(root.join("alias")).unwrap();
        std::fs::remove_dir(physical.join("redirected")).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
