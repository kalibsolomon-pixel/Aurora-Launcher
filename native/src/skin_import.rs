//! Native PNG chooser. Only the explicitly selected file is read; the core
//! owns format/dimension validation, hashing and library persistence.
use crate::api::{ApiError, ApiResult};

pub async fn choose(owner: Option<isize>) -> ApiResult<Option<Vec<u8>>> {
    tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let Some(path) = choose_path_blocking(true, owner)? else {
            return Ok(None);
        };
        let file = std::fs::File::open(path).map_err(|_| {
            ApiError::new("skin_import_failed", "The selected skin could not be read.")
        })?;
        if !file
            .metadata()
            .is_ok_and(|m| m.is_file() && m.len() <= 128 * 1024)
        {
            return Err(ApiError::new(
                "skin_import_failed",
                "Choose a skin up to 128 KiB.",
            ));
        }
        let mut bytes = Vec::new();
        file.take(128 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                ApiError::new("skin_import_failed", "The selected skin could not be read.")
            })?;
        if bytes.len() > 128 * 1024 {
            return Err(ApiError::new(
                "skin_import_failed",
                "Choose a skin up to 128 KiB.",
            ));
        }
        Ok(Some(bytes))
    })
    .await
    .map_err(|_| ApiError::new("skin_import_failed", "The skin chooser did not complete."))?
}

pub async fn choose_content_path(owner: Option<isize>) -> ApiResult<Option<String>> {
    tokio::task::spawn_blocking(move || choose_path_blocking(false, owner))
        .await
        .map_err(|_| ApiError::new("local_import_failed", "The file chooser did not complete."))?
}

#[cfg(not(windows))]
fn choose_path_blocking(_skin: bool, _owner: Option<isize>) -> ApiResult<Option<String>> {
    Err(ApiError::new(
        "skin_import_unsupported",
        "Native file import is not available on this platform yet.",
    ))
}

#[cfg(windows)]
fn choose_path_blocking(skin: bool, owner: Option<isize>) -> ApiResult<Option<String>> {
    use windows::{
        Win32::{
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, CoUninitialize,
            },
            UI::Shell::{
                Common::COMDLG_FILTERSPEC, FOS_DONTADDTORECENT, FOS_FILEMUSTEXIST,
                FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_PATHMUSTEXIST, FileOpenDialog,
                IFileOpenDialog, SIGDN_FILESYSPATH,
            },
        },
        core::w,
    };
    let failed = || {
        ApiError::new(
            "skin_import_failed",
            "The native file chooser could not select the file.",
        )
    };
    static OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if OPEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(ApiError::new(
            "file_picker_busy",
            "Finish the open file picker first.",
        ));
    }
    struct Picker;
    impl Drop for Picker {
        fn drop(&mut self) {
            OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _picker = Picker;
    let owner =
        windows::Win32::Foundation::HWND(owner.ok_or_else(failed)? as *mut std::ffi::c_void);
    let mut owner_pid = 0;
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
            owner,
            Some(&mut owner_pid),
        );
    }
    if owner_pid != std::process::id()
        || !unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(owner)) }.as_bool()
    {
        return Err(failed());
    }
    // This is a dedicated blocking thread with a balanced COM apartment.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    initialized.ok().map_err(|_| failed())?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() }
        }
    }
    let _apartment = Apartment;
    let dialog: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| failed())?;
    unsafe {
        dialog
            .SetTitle(if skin {
                w!("Import Minecraft skin (PNG)")
            } else {
                w!("Import local mod, resource pack or shader")
            })
            .map_err(|_| failed())?;
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: if skin {
                    w!("PNG skin")
                } else {
                    w!("Content archive")
                },
                pszSpec: if skin { w!("*.png") } else { w!("*.jar;*.zip") },
            }])
            .map_err(|_| failed())?;
        dialog
            .SetOptions(
                FOS_FORCEFILESYSTEM
                    | FOS_FILEMUSTEXIST
                    | FOS_PATHMUSTEXIST
                    | FOS_NOCHANGEDIR
                    | FOS_DONTADDTORECENT,
            )
            .map_err(|_| failed())?;
        if let Err(e) = dialog.Show(Some(owner)) {
            if e.code().0 == 0x800704c7u32 as i32 {
                return Ok(None);
            }
            return Err(failed());
        }
        let item = dialog.GetResult().map_err(|_| failed())?;
        let path = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|_| failed())?;
        let value = path.to_string();
        CoTaskMemFree(Some(path.0.cast()));
        Ok(Some(value.map_err(|_| failed())?))
    }
}
