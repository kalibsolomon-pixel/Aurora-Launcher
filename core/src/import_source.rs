//! Bounded snapshots of explicitly selected local directories.
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use crate::instance_content::ContentError;

pub(crate) const MAX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_DEPTH: usize = 24;

fn limit() -> ContentError {
    ContentError::UnsupportedActionWith("The selected folder exceeds safe import limits.".into())
}

fn checked(meta: Metadata) -> Result<Metadata, ContentError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(ContentError::UnsafePath);
        }
    }
    if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
        return Err(ContentError::UnsafePath);
    }
    Ok(meta)
}

pub(crate) fn metadata(path: &Path) -> Result<Metadata, ContentError> {
    if !path.is_absolute()
        || path.components().any(|part| match part {
            Component::ParentDir => true,
            Component::Normal(name) => name.to_str().is_none_or(|name| {
                name.eq_ignore_ascii_case(".minecraft")
                    || crate::instance_content::validate_file_name(name).is_err()
            }),
            _ => false,
        })
    {
        return Err(ContentError::UnsafePath);
    }
    let ancestors: Vec<_> = path.ancestors().collect();
    for ancestor in ancestors.iter().skip(1).rev() {
        if !checked(fs::symlink_metadata(ancestor).map_err(ContentError::Io)?)?.is_dir() {
            return Err(ContentError::UnsafePath);
        }
    }
    checked(fs::symlink_metadata(path).map_err(ContentError::Io)?)
}

/// Files are pinned against writes, rename and delete while they are read.
#[cfg(windows)]
const SHARE_FILE: u32 = 1;
/// Directories are pinned only against rename and delete: other programs may
/// still add or rename entries inside every ancestor (up to the drive root).
#[cfg(windows)]
const SHARE_DIRECTORY: u32 = 1 | 2;

#[cfg(windows)]
fn open_checked(path: &Path, share: u32) -> Result<File, ContentError> {
    use std::os::windows::fs::OpenOptionsExt;
    // Open reparse points themselves so `checked` can reject them.
    let file = OpenOptions::new()
        .read(true)
        .share_mode(share)
        .custom_flags(0x0220_0000)
        .open(path)
        .map_err(ContentError::Io)?;
    checked(file.metadata().map_err(ContentError::Io)?)?;
    Ok(file)
}

#[cfg(unix)]
fn open_at(parent: libc::c_int, path: &Path) -> Result<File, ContentError> {
    use std::os::fd::FromRawFd;
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| ContentError::UnsafePath)?;
    // Each child is opened relative to a pinned directory and never follows a symlink.
    let fd = unsafe {
        libc::openat(
            parent,
            path.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(ContentError::Io(io::Error::last_os_error()));
    }
    let file = unsafe { File::from_raw_fd(fd) };
    checked(file.metadata().map_err(ContentError::Io)?)?;
    Ok(file)
}

struct Directory {
    handle: File,
    #[cfg(windows)]
    path: PathBuf,
}

impl Directory {
    fn root(path: &Path) -> Result<Self, ContentError> {
        #[cfg(windows)]
        let handle = open_checked(path, SHARE_DIRECTORY)?;
        #[cfg(unix)]
        let handle = open_at(libc::AT_FDCWD, path)?;
        Self::new(handle, path)
    }

    fn new(handle: File, _path: &Path) -> Result<Self, ContentError> {
        if !handle.metadata().map_err(ContentError::Io)?.is_dir() {
            return Err(ContentError::UnsafePath);
        }
        Ok(Self {
            handle,
            #[cfg(windows)]
            path: _path.to_owned(),
        })
    }

    fn open(&self, name: &Path) -> Result<File, ContentError> {
        #[cfg(windows)]
        {
            open_checked(&self.path.join(name), SHARE_FILE)
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            open_at(self.handle.as_raw_fd(), name)
        }
    }

    fn child(&self, name: &Path) -> Result<Self, ContentError> {
        #[cfg(windows)]
        {
            let path = self.path.join(name);
            Self::new(open_checked(&path, SHARE_DIRECTORY)?, &path)
        }
        #[cfg(unix)]
        Self::new(self.open(name)?, name)
    }

    fn walk(
        &self,
        mut visitor: impl FnMut(&std::ffi::OsStr, Metadata) -> Result<(), ContentError>,
    ) -> Result<(), ContentError> {
        #[cfg(windows)]
        {
            let _pin = &self.handle;
            for entry in fs::read_dir(&self.path).map_err(ContentError::Io)? {
                let entry = entry.map_err(ContentError::Io)?;
                let info = checked(fs::symlink_metadata(entry.path()).map_err(ContentError::Io)?)?;
                visitor(&entry.file_name(), info)?;
            }
        }
        #[cfg(unix)]
        {
            use std::os::fd::{AsRawFd, IntoRawFd};
            use std::os::unix::ffi::OsStrExt;
            struct Stream(*mut libc::DIR);
            impl Drop for Stream {
                fn drop(&mut self) {
                    unsafe { libc::closedir(self.0) };
                }
            }
            // A fresh open file description gives each walk an independent cursor.
            let directory = open_at(self.handle.as_raw_fd(), Path::new("."))?;
            let raw = unsafe { libc::fdopendir(directory.as_raw_fd()) };
            if raw.is_null() {
                return Err(ContentError::Io(io::Error::last_os_error()));
            }
            let _ = directory.into_raw_fd(); // fdopendir now owns it
            let stream = Stream(raw);
            loop {
                #[cfg(target_os = "linux")]
                let errno = unsafe { libc::__errno_location() };
                #[cfg(any(target_vendor = "apple", target_os = "freebsd"))]
                let errno = unsafe { libc::__error() };
                unsafe { *errno = 0 };
                let entry = unsafe { libc::readdir(stream.0) };
                if entry.is_null() {
                    if unsafe { *errno } != 0 {
                        return Err(ContentError::Io(io::Error::last_os_error()));
                    }
                    break;
                }
                let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
                if matches!(name, b"." | b"..") {
                    continue;
                }
                let name = std::ffi::OsStr::from_bytes(name);
                let file = self.open(Path::new(name))?;
                visitor(name, file.metadata().map_err(ContentError::Io)?)?;
            }
        }
        Ok(())
    }
}

pub(crate) struct Folder(Vec<Directory>);

impl Folder {
    pub(crate) fn open(path: &Path) -> Result<Self, ContentError> {
        if !metadata(path)?.is_dir() {
            return Err(ContentError::UnsafePath);
        }
        let mut ancestors = path.ancestors().collect::<Vec<_>>();
        let mut directories = vec![Directory::root(
            ancestors.pop().ok_or(ContentError::UnsafePath)?,
        )?];
        for ancestor in ancestors.into_iter().rev() {
            let name = ancestor.file_name().ok_or(ContentError::UnsafePath)?;
            directories.push(directories.last().unwrap().child(Path::new(name))?);
        }
        Ok(Self(directories))
    }

    pub(crate) fn files(&self, recursive: bool) -> Result<Vec<Entry>, ContentError> {
        let mut files = Vec::new();
        visit(self.0.last().unwrap(), "", recursive, 0, &mut 0, &mut files)?;
        files.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }

    pub(crate) fn selected_file(path: &Path) -> Result<(Self, Entry), ContentError> {
        let info = metadata(path)?;
        if !info.is_file() || info.len() == 0 || info.len() > MAX_BYTES {
            return Err(limit());
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(ContentError::UnsafePath)?;
        crate::instance_content::validate_file_name(name)?;
        let folder = Self::open(path.parent().ok_or(ContentError::UnsafePath)?)?;
        Ok((
            folder,
            Entry {
                name: name.to_owned(),
                relative: name.to_owned(),
                size: info.len(),
            },
        ))
    }

    pub(crate) fn copy(&self, entry: &Entry, writer: &mut impl Write) -> Result<(), ContentError> {
        if entry.size > MAX_BYTES {
            return Err(limit());
        }
        let mut directories = Vec::new();
        let mut current = self.0.last().unwrap();
        let mut segments = entry.relative.split('/').peekable();
        let mut file = loop {
            let part = segments.next().ok_or(ContentError::UnsafePath)?;
            let name = Path::new(part);
            if segments.peek().is_none() {
                break current.open(name)?;
            }
            directories.push(current.child(name)?);
            current = directories.last().unwrap();
        };
        let info = file.metadata().map_err(ContentError::Io)?;
        if !info.is_file() || info.len() != entry.size {
            return Err(ContentError::ChangedSinceScan);
        }
        let copied = io::copy(&mut Read::by_ref(&mut file).take(entry.size + 1), writer)
            .map_err(ContentError::Io)?;
        if copied != entry.size {
            return Err(ContentError::ChangedSinceScan);
        }
        Ok(())
    }
}

fn visit(
    directory: &Directory,
    prefix: &str,
    recursive: bool,
    depth: usize,
    count: &mut usize,
    files: &mut Vec<Entry>,
) -> Result<(), ContentError> {
    if depth >= MAX_DEPTH {
        return Err(limit());
    }
    directory.walk(|part, info| {
        crate::content_jobs::checkpoint().map_err(|_| ContentError::Cancelled)?;
        *count += 1;
        if *count > MAX_ENTRIES {
            return Err(limit());
        }
        let part = part.to_str().ok_or(ContentError::UnsafePath)?;
        crate::instance_content::validate_file_name(part)?;
        if part.eq_ignore_ascii_case(".minecraft") {
            return Err(ContentError::UnsafePath);
        }
        let name = format!("{prefix}{part}");
        if info.is_dir() {
            if recursive {
                let child = directory.child(Path::new(part))?;
                visit(
                    &child,
                    &format!("{name}/"),
                    recursive,
                    depth + 1,
                    count,
                    files,
                )?;
            }
        } else {
            files.push(Entry {
                relative: name.clone(),
                name,
                size: info.len(),
            });
        }
        Ok(())
    })
}

pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) size: u64,
    relative: String,
}

/// An owned working directory containing only snapshots created by this import.
pub(crate) struct Scratch(PathBuf);
impl Scratch {
    pub(crate) fn new() -> Result<Self, ContentError> {
        let parent = std::env::temp_dir()
            .canonicalize()
            .map_err(ContentError::Io)?;
        if !metadata(&parent)?.is_dir() {
            return Err(ContentError::UnsafePath);
        }
        let root = parent.join(format!("aurora-import-{}", uuid::Uuid::new_v4()));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&root).map_err(ContentError::Io)?;
        Ok(Self(root))
    }
    pub(crate) fn file(&self, name: &str) -> Result<(PathBuf, File), ContentError> {
        crate::instance_content::validate_file_name(name)?;
        let path = self.0.join(name);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(ContentError::Io)?;
        Ok((path, file))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Normalize a directory to the same reader input as a manually zipped pack.
pub(crate) fn zip(
    folder: &Folder,
    entries: &[Entry],
    supplemental: Option<(&str, &[u8])>,
    writer: impl Write + Seek,
) -> Result<(), ContentError> {
    let mut zip = zip::ZipWriter::new(writer);
    let options = zip::write::SimpleFileOptions::default()
        .last_modified_time(zip::DateTime::default())
        .compression_method(zip::CompressionMethod::Deflated);
    let mut size = 0;
    if let Some((name, bytes)) = supplemental {
        zip.start_file(name, options)
            .map_err(|_| ContentError::InvalidProviderArtifact)?;
        zip.write_all(bytes).map_err(ContentError::Io)?;
        size = bytes.len() as u64;
    }
    for entry in entries {
        crate::content_jobs::checkpoint().map_err(|_| ContentError::Cancelled)?;
        size += entry.size.min(MAX_BYTES + 1);
        if size > MAX_BYTES {
            return Err(limit());
        }
        zip.start_file(&entry.name, options)
            .map_err(|_| ContentError::InvalidProviderArtifact)?;
        folder.copy(entry, &mut zip)?;
    }
    zip.finish()
        .map_err(|_| ContentError::InvalidProviderArtifact)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracted_folder_rejects_links_and_minecraft_paths() {
        let scratch = Scratch::new().unwrap();
        let source = scratch.0.join("source");
        let outside = scratch.0.join("outside");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"outside").unwrap();
        let link = source.join("escape");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(windows)]
        {
            let joined = std::process::Command::new("cmd.exe")
                .args(["/c", "mklink", "/J"])
                .arg(&link)
                .arg(&outside)
                .output()
                .unwrap();
            assert!(joined.status.success(), "{:?}", joined);
        }
        let folder = Folder::open(&source).unwrap();
        assert!(folder.files(true).is_err());
        assert!(Folder::open(&link).is_err());
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"outside");
        #[cfg(windows)]
        fs::remove_dir(link).unwrap();
        #[cfg(unix)]
        fs::remove_file(link).unwrap();
        fs::create_dir(source.join(".minecraft")).unwrap();
        assert!(folder.files(true).is_err());
        assert!(Folder::open(&source.join(".minecraft")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn open_folder_blocks_rename_but_not_sibling_writes() {
        let scratch = Scratch::new().unwrap();
        let selected = scratch.0.join("pack");
        fs::create_dir(&selected).unwrap();
        let folder = Folder::open(&selected).unwrap();
        assert!(fs::rename(&selected, scratch.0.join("moved")).is_err());
        fs::write(scratch.0.join("sibling.txt"), b"ok").unwrap();
        fs::rename(scratch.0.join("sibling.txt"), scratch.0.join("renamed.txt")).unwrap();
        drop(folder);
    }

    #[test]
    fn extracted_folder_refuses_oversized_input_before_copying() {
        let scratch = Scratch::new().unwrap();
        let _ = scratch.file("huge.bin").unwrap();
        let folder = Folder::open(&scratch.0).unwrap();
        let mut entries = folder.files(true).unwrap();
        entries[0].size = MAX_BYTES + 1;
        let mut output = Vec::new();
        assert!(folder.copy(&entries[0], &mut output).is_err());
        assert!(output.is_empty());
    }
}
