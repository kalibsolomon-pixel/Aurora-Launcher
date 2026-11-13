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
