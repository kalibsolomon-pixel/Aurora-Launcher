//! Windows window-visibility & notification-area tray icon helper for
//! OnLaunchBehavior (`Minimize` and `HideToTray`).

#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, IDI_APPLICATION, LoadIconW,
        MSG, PostMessageW, SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SetForegroundWindow,
        ShowWindow, TranslateMessage, WM_APP, WM_CLOSE, WM_LBUTTONDBLCLK, WM_LBUTTONUP,
        WM_RBUTTONUP,
    };
    use windows::core::PCWSTR;

    const WM_TRAY: u32 = WM_APP + 42;
    static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
    static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
    static TRAY_ACTIVE: AtomicBool = AtomicBool::new(false);

    pub fn minimize_window(hwnd: isize) {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), SW_MINIMIZE);
        }
    }

    pub fn restore_window(hwnd: isize) {
        remove_tray();
        unsafe {
            let h = HWND(hwnd as *mut _);
            let _ = ShowWindow(h, SW_SHOW);
            let _ = ShowWindow(h, SW_RESTORE);
            let _ = SetForegroundWindow(h);
        }
    }

    pub fn hide_to_tray(hwnd: isize) {
        MAIN_HWND.store(hwnd, Ordering::SeqCst);
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), SW_HIDE);
        }
        if TRAY_ACTIVE.swap(true, Ordering::SeqCst) {
            return;
        }
        std::thread::spawn(|| {
            run_tray_thread();
        });
    }

    pub fn remove_tray() {
        if !TRAY_ACTIVE.swap(false, Ordering::SeqCst) {
            return;
        }
        let tray = TRAY_HWND.swap(0, Ordering::SeqCst);
        if tray != 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(tray as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
    }

    fn run_tray_thread() {
        let class_name: Vec<u16> = "STATIC\0".encode_utf16().collect();
        unsafe {
            let Ok(hwnd) = CreateWindowExW(
                Default::default(),
                PCWSTR(class_name.as_ptr()),
                PCWSTR::null(),
                Default::default(),
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            ) else {
                TRAY_ACTIVE.store(false, Ordering::SeqCst);
                return;
            };
            TRAY_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
                ..Default::default()
            };
            let tip = "Aurora Launcher (click to restore)";
            for (dst, src) in nid.szTip.iter_mut().zip(tip.encode_utf16()) {
                *dst = src;
            }
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if msg.message == WM_TRAY {
                    let ev = (msg.lParam.0 & 0xFFFF) as u32;
                    if matches!(ev, WM_LBUTTONUP | WM_LBUTTONDBLCLK | WM_RBUTTONUP) {
                        let main = MAIN_HWND.load(Ordering::SeqCst);
                        if main != 0 {
                            restore_window(main);
                        }
                        break;
                    }
                }
                if msg.message == WM_CLOSE {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
                if !TRAY_ACTIVE.load(Ordering::SeqCst) {
                    break;
                }
            }
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            let _ = DestroyWindow(hwnd);
            TRAY_HWND.store(0, Ordering::SeqCst);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn minimize_window(_hwnd: isize) {}
    pub fn restore_window(_hwnd: isize) {}
    pub fn hide_to_tray(_hwnd: isize) {}
    #[allow(dead_code)]
    pub fn remove_tray() {}
}

pub use imp::{hide_to_tray, minimize_window, restore_window};
