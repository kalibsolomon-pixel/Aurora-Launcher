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

/// Separate opens contend even in one process. The descriptor is never cloned
/// or inherited by children, and closing it releases OS ownership after a crash.
#[derive(Debug)]
pub(crate) struct TargetLock {
    _file: File,
    parent: PathBuf,
}

impl TargetLock {
    pub(crate) fn acquire(root: &Path, parent: &Path, name: &str) -> Result<Self, LockError> {
        let parent = ensure_directory(root, parent)?;
        let path = parent.join(name);
        require_regular_or_missing(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
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
        match file.try_lock() {
            Ok(()) => Ok(Self {
                _file: file,
                parent,
            }),
            Err(std::fs::TryLockError::WouldBlock) => Err(LockError::Busy),
            Err(std::fs::TryLockError::Error(error)) => Err(LockError::Io(error)),
        }
    }

    pub(crate) fn parent(&self) -> &Path {
        &self.parent
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
        let guard = TargetLock::acquire(&root, &root.join("target"), ".lock").unwrap();
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
}
