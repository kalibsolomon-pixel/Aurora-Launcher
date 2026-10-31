//! Windows-only neutral foreground fixture for the native acceptance probe.
//! rustc --edition=2024 -O native/tools/focus-sink.rs -o <temp>/focus-sink.exe
//! No launcher state, network, credentials or user content is opened.
#![windows_subsystem = "windows"]
#[cfg(windows)]
fn main() {
    use std::ffi::c_void;
    type Handle = *mut c_void;
    #[repr(C)]
    struct Class {
        style: u32,
        proc: Option<unsafe extern "system" fn(Handle, u32, usize, isize) -> isize>,
        cls_extra: i32,
        win_extra: i32,
        instance: Handle,
        icon: Handle,
        cursor: Handle,
        background: Handle,
        menu: *const u16,
        name: *const u16,
    }
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    struct Message {
        hwnd: Handle,
        message: u32,
        w: usize,
        l: isize,
        time: u32,
        point: Point,
        private: u32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassW(c: *const Class) -> u16;
        fn CreateWindowExW(
            ex: u32,
            class: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: Handle,
            menu: Handle,
            instance: Handle,
            param: Handle,
        ) -> Handle;
        fn DefWindowProcW(h: Handle, m: u32, w: usize, l: isize) -> isize;
        fn ShowWindow(h: Handle, cmd: i32) -> i32;
        fn GetMessageW(m: *mut Message, h: Handle, min: u32, max: u32) -> i32;
        fn TranslateMessage(m: *const Message) -> i32;
        fn DispatchMessageW(m: *const Message) -> isize;
        fn PostQuitMessage(code: i32);
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> Handle;
    }
    unsafe extern "system" fn proc(h: Handle, m: u32, w: usize, l: isize) -> isize {
        if m == 2 {
            unsafe { PostQuitMessage(0) };
            return 0;
        }
        unsafe { DefWindowProcW(h, m, w, l) }
    }
    let text: Vec<u16> = "Aurora benchmark focus sink\0".encode_utf16().collect();
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = Class {
            style: 0,
            proc: Some(proc),
            cls_extra: 0,
            win_extra: 0,
            instance,
            icon: std::ptr::null_mut(),
            cursor: std::ptr::null_mut(),
            background: 6usize as Handle,
            menu: std::ptr::null(),
            name: text.as_ptr(),
        };
        assert_ne!(RegisterClassW(&class), 0);
        let window = CreateWindowExW(
            0,
            text.as_ptr(),
            text.as_ptr(),
            0x00cf0000,
            20,
            20,
            320,
            100,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );
        assert!(!window.is_null());
        ShowWindow(window, 4);
        let mut message: Message = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
#[cfg(not(windows))]
fn main() {
    panic!("The foreground fixture is Windows-only");
}
