//! Custom window chrome. On Windows the window is frameless; this module
//! hands drag and edge resizing to the OS through winit (so Aero Snap,
//! monitor bounds, DPI and the taskbar behave natively), implements
//! double-click maximize, and tracks focus/maximize state. Other platforms
//! keep their native frame (`Chrome.custom` stays false).

use std::cell::Cell;
use std::time::{Duration, Instant};

use slint::ComponentHandle;
use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

use crate::{AppWindow, Chrome};

const DOUBLE_CLICK: Duration = Duration::from_millis(450);

thread_local! {
    // Unfocused until Windows reports focus: a window that never gains it
    // never reports losing it.
    static FOCUSED: Cell<bool> = const { Cell::new(false) };
    // Minimized or fully covered (winit reports both as occlusion).
    static OCCLUDED: Cell<bool> = const { Cell::new(false) };
    // The window handle once known (Windows), for authoritative state.
    static HWND_VALUE: Cell<isize> = const { Cell::new(0) };
    static LAST_PRESS: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Whether the window currently has keyboard focus.
#[allow(dead_code)]
pub fn is_focused() -> bool {
    FOCUSED.with(Cell::get)
}

/// Capture on the UI thread, then revalidate before a blocking native dialog.
pub fn dialog_owner() -> Option<isize> {
    #[cfg(windows)]
    {
        hwnd().map(|handle| handle.0 as isize)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Whether the window is minimized or fully covered.
#[allow(dead_code)]
pub fn is_occluded() -> bool {
    OCCLUDED.with(Cell::get)
}

/// The window's handle when known (Windows), for authoritative OS state.
/// winit's focus and occlusion events can arrive in an order that leaves
/// cached state stale (observed: a minimize reported as gaining focus), so
/// these checks ask the OS at call time where possible.
#[cfg(windows)]
fn hwnd() -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow};
    let raw = HWND_VALUE.with(Cell::get);
    let handle = windows::Win32::Foundation::HWND(raw as *mut core::ffi::c_void);
    let mut process = 0;
    if raw != 0 && unsafe { IsWindow(Some(handle)) }.as_bool() {
        unsafe { GetWindowThreadProcessId(handle, Some(&mut process)) };
    }
    (process == std::process::id()).then_some(handle)
}

/// Visible, not minimized, and not reported occluded. Unknown Windows handles
/// fail closed. Windows does not report arbitrary overlap as winit occlusion.
#[allow(dead_code)]
pub fn is_visible() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
        use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindowVisible};
        return hwnd().is_some_and(|hwnd| {
            let mut cloaked = 0u32;
            let _ = unsafe {
                DwmGetWindowAttribute(
                    hwnd,
                    DWMWA_CLOAKED,
                    (&mut cloaked as *mut u32).cast(),
                    std::mem::size_of::<u32>() as u32,
                )
            };
            !is_occluded()
                && cloaked == 0
                && unsafe { IsWindowVisible(hwnd) }.as_bool()
                && !unsafe { IsIconic(hwnd) }.as_bool()
        });
    }
    #[cfg(not(windows))]
    !is_occluded()
}

/// The foreground window (or, without an OS handle, last reported focused).
#[allow(dead_code)]
pub fn is_foreground() -> bool {
    #[cfg(windows)]
    if let Some(hwnd) = hwnd() {
        return unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() } == hwnd;
    }
    is_focused()
}

pub fn install(window: &AppWindow) {
    let chrome = window.global::<Chrome>();
    chrome.set_custom(cfg!(windows));

    let weak = window.as_weak();
    chrome.on_drag_pressed(move || {
        let Some(window) = weak.upgrade() else { return };
        let now = Instant::now();
        let double = LAST_PRESS
            .with(|last| last.replace(Some(now)))
            .is_some_and(|previous| now.duration_since(previous) <= DOUBLE_CLICK);
        if double {
            LAST_PRESS.with(|last| last.set(None));
            toggle_maximize(&window);
        } else {
            window.window().with_winit_window(|w| {
                let _ = w.drag_window();
            });
        }
    });
    let weak = window.as_weak();
    chrome.on_minimize(move || {
        if let Some(window) = weak.upgrade() {
            window.window().set_minimized(true);
        }
    });
    let weak = window.as_weak();
    chrome.on_toggle_maximize(move || {
        if let Some(window) = weak.upgrade() {
            toggle_maximize(&window);
        }
    });
    let weak = window.as_weak();
    chrome.on_close(move || {
        if let Some(window) = weak.upgrade() {
            // The same path as the system close button.
            window
                .window()
                .dispatch_event(slint::platform::WindowEvent::CloseRequested);
        }
    });
    let weak = window.as_weak();
    chrome.on_resize(move |direction| {
        use winit::window::ResizeDirection as D;
        let direction = match direction.as_str() {
            "n" => D::North,
            "s" => D::South,
            "e" => D::East,
            "w" => D::West,
            "ne" => D::NorthEast,
            "nw" => D::NorthWest,
            "se" => D::SouthEast,
            "sw" => D::SouthWest,
            _ => return,
        };
        if let Some(window) = weak.upgrade() {
            window.window().with_winit_window(|w| {
                let _ = w.drag_resize_window(direction);
            });
        }
    });

    let weak = window.as_weak();
    window
        .window()
        .on_winit_window_event(move |slint_window, event| {
            use winit::event::WindowEvent;
            // Refresh from the actual winit window, including after recreation.
            slint_window.with_winit_window(style_frameless);
            match event {
                WindowEvent::Focused(focused) => {
                    FOCUSED.with(|f| f.set(*focused));
                }
                WindowEvent::Occluded(occluded) => {
                    OCCLUDED.with(|o| o.set(*occluded));
                }
                WindowEvent::Resized(_) => {
                    if let Some(window) = weak.upgrade() {
                        window
                            .global::<Chrome>()
                            .set_maximized(window.window().is_maximized());
                    }
                }
                WindowEvent::Destroyed => {
                    HWND_VALUE.with(|value| value.set(0));
                    FOCUSED.with(|value| value.set(false));
                    OCCLUDED.with(|value| value.set(true));
                }
                _ => {}
            }
            EventResult::Propagate
        });
}

fn toggle_maximize(window: &AppWindow) {
    let maximized = !window.window().is_maximized();
    window.window().set_maximized(maximized);
    window.global::<Chrome>().set_maximized(maximized);
}

/// A frameless window keeps the system drop shadow and, on Windows 11,
/// rounded corners like other top-level windows.
#[cfg(windows)]
fn style_frameless(window: &winit::window::Window) {
    use winit::platform::windows::WindowExtWindows;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return;
    };
    if let RawWindowHandle::Win32(handle) = handle.as_raw() {
        if HWND_VALUE.with(|value| value.replace(handle.hwnd.get())) == handle.hwnd.get() {
            return;
        }
        OCCLUDED.with(|value| value.set(false));
        window.set_undecorated_shadow(true);
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::{
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
        };
        let hwnd = HWND(handle.hwnd.get() as *mut core::ffi::c_void);
        let preference = DWMWCP_ROUND;
        // Ignored on Windows 10, where corners are square.
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&preference as *const _) as *const core::ffi::c_void,
                std::mem::size_of_val(&preference) as u32,
            )
        };
    }
}

#[cfg(not(windows))]
fn style_frameless(_window: &winit::window::Window) {}
