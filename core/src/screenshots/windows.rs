//! Owned Win32 clipboard allocations and shell operations on checked paths only.
use ::windows::{
    Win32::{
        Foundation::{GlobalFree, HANDLE},
        Storage::FileSystem::MoveFileW,
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, CoUninitialize,
            },
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        },
        UI::Shell::{
            FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOFX_RECYCLEONDELETE,
            FileOperation, IFileOperation, IFileOperationProgressSink, IShellItem,
            SHCreateItemFromParsingName, SHOpenFolderAndSelectItems, SHParseDisplayName,
        },
    },
    core::{HSTRING, PCWSTR},
};
use std::{io, path::Path};

/// Shell items and clipboard consumers expect ordinary DOS/UNC names rather
/// than Rust canonicalize's Win32 extended-length prefix. This changes only
/// the spelling of an already validated, pinned path.
pub(super) fn shell_path(path: &Path) -> std::path::PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    let ordinary = if wide.starts_with(&[92, 92, 63, 92, 85, 78, 67, 92]) {
        [vec![92, 92], wide[8..].to_vec()].concat()
    } else if wide.starts_with(&[92, 92, 63, 92]) {
        wide[4..].to_vec()
    } else {
        wide
    };
    std::ffi::OsString::from_wide(&ordinary).into()
}

struct Apartment(bool);
impl Apartment {
    fn enter() -> Self {
        Self(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub(super) unsafe fn move_file(from: &[u16], to: &[u16]) -> io::Result<()> {
    // Both strings are checked, NUL-terminated paths in the same pinned directory.
    unsafe { MoveFileW(PCWSTR(from.as_ptr()), PCWSTR(to.as_ptr())) }.map_err(io::Error::other)
}

pub(super) fn clipboard(format: u32, bytes: &[u8]) -> io::Result<()> {
    // Allocate before opening/emptying the clipboard. The system takes ownership
    // only after SetClipboardData succeeds; every failure frees our allocation.
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(io::Error::other)?;
        let pointer = GlobalLock(memory);
        if pointer.is_null() {
            let _ = GlobalFree(Some(memory));
            return Err(io::Error::last_os_error());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast(), bytes.len());
        let _ = GlobalUnlock(memory);
        // A temporary native window gives EmptyClipboard a valid owner even
        // though this runs on a worker with no GPUI window/thread affinity.
        use ::windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
        };
        let owner = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            ::windows::core::w!("STATIC"),
            ::windows::core::w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            None,
        ) {
            Ok(owner) => owner,
            Err(e) => {
                let _ = GlobalFree(Some(memory));
                return Err(io::Error::other(e));
            }
        };
        let result = (|| {
            OpenClipboard(Some(owner)).map_err(io::Error::other)?;
            let result = EmptyClipboard()
                .and_then(|_| SetClipboardData(format, Some(HANDLE(memory.0))))
                .map(|_| ())
                .map_err(io::Error::other);
            let _ = CloseClipboard();
            result
        })();
        let _ = DestroyWindow(owner);
        if result.is_err() {
            let _ = GlobalFree(Some(memory));
        }
        result
    }
}

pub(super) fn reveal(path: &Path) -> io::Result<()> {
    let _apartment = Apartment::enter();
    let name = HSTRING::from(shell_path(path).as_os_str());
    unsafe {
        let mut item = std::ptr::null_mut();
        SHParseDisplayName(&name, None, &mut item, 0, None).map_err(io::Error::other)?;
        let result = SHOpenFolderAndSelectItems(item, None, 0).map_err(io::Error::other);
        CoTaskMemFree(Some(item.cast()));
        result
    }
}

pub(super) fn recycle(path: &Path) -> io::Result<()> {
    let _apartment = Apartment::enter();
    let name = HSTRING::from(shell_path(path).as_os_str());
    unsafe {
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)
                .map_err(io::Error::other)?;
        // RECYCLEONDELETE explicitly requests recycling. Never fall back to a
        // permanent unlink when the shell cannot recycle this volume/file.
        operation
            .SetOperationFlags(
                FOF_ALLOWUNDO
                    | FOFX_RECYCLEONDELETE
                    | FOF_NOCONFIRMATION
                    | FOF_NOERRORUI
                    | FOF_SILENT,
            )
            .map_err(io::Error::other)?;
        let item: IShellItem =
            SHCreateItemFromParsingName(&name, None).map_err(io::Error::other)?;
        operation
            .DeleteItem(&item, None::<&IFileOperationProgressSink>)
            .map_err(io::Error::other)?;
        operation.PerformOperations().map_err(io::Error::other)?;
        if operation
            .GetAnyOperationsAborted()
            .map_err(io::Error::other)?
            .as_bool()
        {
            return Err(io::Error::other("The screenshot was not recycled."));
        }
    }
    if path.exists() {
        return Err(io::Error::other(
            "The screenshot could not be moved to the Recycle Bin.",
        ));
    }
    Ok(())
}
