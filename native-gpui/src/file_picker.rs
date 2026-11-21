//! Reuse the supported host's native file dialogs with the GPUI window owner.
#![allow(dead_code)]

#[path = "../../native/src/skin_import.rs"]
mod native;

#[allow(unused_imports)]
pub use native::{choose, choose_content_path, choose_java_path, choose_modpack_path};

#[cfg(windows)]
pub fn owner(window: &gpui::Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

#[cfg(not(windows))]
pub fn owner(_window: &gpui::Window) -> Option<isize> {
    None
}

#[derive(Clone, Copy)]
pub enum LocalImportKind {
    Content,
    Modpack,
}

/// The Windows file dialogs retain their format filters. GPUI owns folder
/// selection and the file dialog on platforms without the custom picker.
pub fn import_path(
    kind: LocalImportKind,
    folder: bool,
    window: &gpui::Window,
    cx: &gpui::App,
) -> impl std::future::Future<Output = crate::api::ApiResult<Option<String>>> + Send + use<> {
    let owner = owner(window);
    let prompt = (folder || !cfg!(windows)).then(|| {
        cx.prompt_for_paths(gpui::PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: false,
            prompt: Some("Choose".into()),
        })
    });
    async move {
        if let Some(prompt) = prompt {
            let failed = || {
                crate::api::ApiError::new(
                    "local_import_failed",
                    "The native import picker could not select a path.",
                )
            };
            let selected = prompt.await.map_err(|_| failed())?.map_err(|_| failed())?;
            return selected
                .and_then(|paths| paths.into_iter().next())
                .map(|path| path.into_os_string().into_string().map_err(|_| failed()))
                .transpose();
        }
        match kind {
            LocalImportKind::Content => choose_content_path(owner).await,
            LocalImportKind::Modpack => choose_modpack_path(owner).await,
        }
    }
}

/// Native Save-File dialog using GPUI's platform prompt.
pub fn save_path(
    suggested_name: &str,
    cx: &gpui::App,
) -> impl std::future::Future<Output = crate::api::ApiResult<Option<String>>> + Send + use<> {
    let dir = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let prompt = cx.prompt_for_new_path(&dir, Some(suggested_name));
    async move {
        let failed = || {
            crate::api::ApiError::new("save_dialog_failed", "Could not open the save file dialog.")
        };
        let selected = prompt.await.map_err(|_| failed())?.map_err(|_| failed())?;
        selected
            .map(|p| p.into_os_string().into_string().map_err(|_| failed()))
            .transpose()
    }
}
