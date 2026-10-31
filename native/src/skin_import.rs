//! Native PNG chooser. Only the explicitly selected file is read; the core
//! owns format/dimension validation, hashing and library persistence.
use crate::api::{ApiError, ApiResult};

pub async fn choose() -> ApiResult<Option<Vec<u8>>> {
    tokio::task::spawn_blocking(choose_blocking)
        .await
        .map_err(|_| ApiError::new("skin_import_failed", "The skin chooser did not complete."))?
}

#[cfg(not(windows))]
fn choose_blocking() -> ApiResult<Option<Vec<u8>>> {
    Err(ApiError::new(
        "skin_import_unsupported",
        "Native file import is not available on this platform yet.",
    ))
}

#[cfg(windows)]
fn choose_blocking() -> ApiResult<Option<Vec<u8>>> {
    use std::io::Read;
    use windows::{
        Win32::{
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoTaskMemFree, CoUninitialize,
            },
            UI::Shell::{
                Common::COMDLG_FILTERSPEC, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR,
                FOS_PATHMUSTEXIST, FileOpenDialog, IFileOpenDialog, SIGDN_FILESYSPATH,
            },
        },
        core::w,
    };
    let failed = || {
        ApiError::new(
            "skin_import_failed",
            "The selected PNG could not be read. Choose a skin file up to 128 KiB.",
        )
    };
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
            .SetTitle(w!("Import Minecraft skin (PNG)"))
            .map_err(|_| failed())?;
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("PNG skin"),
                pszSpec: w!("*.png"),
            }])
            .map_err(|_| failed())?;
        dialog
            .SetOptions(
                FOS_FORCEFILESYSTEM | FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR,
            )
            .map_err(|_| failed())?;
        if let Err(e) = dialog.Show(None) {
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
        let file = std::fs::File::open(value.map_err(|_| failed())?).map_err(|_| failed())?;
        let metadata = file.metadata().map_err(|_| failed())?;
        if !metadata.is_file() || metadata.len() > 128 * 1024 {
            return Err(failed());
        }
        let mut bytes = Vec::new();
        file.take(128 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| failed())?;
        if bytes.len() > 128 * 1024 {
            return Err(failed());
        }
        Ok(Some(bytes))
    }
}
