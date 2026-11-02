//! Shared dialog keyboard modality, exercised with real Slint key delivery.
use slint::ComponentHandle;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Key, Platform, WindowAdapter, WindowEvent};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
thread_local! {static ADAPTER: RefCell<Option<Rc<MinimalSoftwareWindow>>> = const {RefCell::new(None)};}
slint::slint! {
    import { Button, Dialog, TextField } from "ui/components.slint";
    export component ModalFixture inherits Window {
        width: 720px; height: 520px;
        callback background-action();
        callback dialog-action();
        callback dismissed();
        in-out property <bool> opened: false;
        in property <bool> with-field: false;
        in-out property <string> entered;
        public function focus-background() { background.focus(); }
        background := Button { text: "Background"; clicked => { root.background-action(); } }
        if opened: Dialog {
            title: "Modal acceptance";
            dismissed => { root.opened = false; root.dismissed(); }
            if root.with-field: TextField { label: "Confirmation"; initial-focus: true; text <=> root.entered; }
            Button { text: "Inside dialog"; clicked => { root.dialog-action(); } }
        }
    }
}
struct TestPlatform;
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        ADAPTER.with(|adapter| *adapter.borrow_mut() = Some(window.clone()));
        Ok(window)
    }
}

#[test]
fn modal_preserves_geometry_and_visible_focus_at_supported_scales() {
    slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let app = ModalFixture::new().unwrap();
        let adapter = ADAPTER.with(|a| a.borrow().as_ref().unwrap().clone());
        app.window()
            .dispatch_event(WindowEvent::ScaleFactorChanged {
                scale_factor: scale,
            });
        app.window().set_size(slint::LogicalSize::new(720., 520.));
        app.show().unwrap();
        app.set_opened(true);
        key(app.window(), Key::Tab);
        slint::platform::update_timers_and_animations();
        let size = app.window().size();
        let (width, height) = (size.width as usize, size.height as usize);
        let mut pixels = vec![slint::Rgb8Pixel::default(); width * height];
        adapter.request_redraw();
        adapter.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, width);
        });
        assert!(
            pixels
                .iter()
                .any(|p| p.r == 0x1d && p.g == 0x1f && p.b == 0x27),
            "dialog panel must render at {scale}"
        );
        assert!(
            pixels.iter().any(|p| p.r > 100 && p.b > 150 && p.b > p.g),
            "keyboard focus accent must render at {scale}"
        );
        if let Some(dir) = std::env::var_os("AURORA_DESKTOP_EVIDENCE") {
            let dir = std::path::PathBuf::from(dir);
            assert!(dir.is_dir());
            let row = (width * 3).div_ceil(4) * 4;
            let image = row * height;
            let mut out = Vec::with_capacity(54 + image);
            out.extend_from_slice(b"BM");
            out.extend_from_slice(&((54 + image) as u32).to_le_bytes());
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&54u32.to_le_bytes());
            out.extend_from_slice(&40u32.to_le_bytes());
            out.extend_from_slice(&(width as i32).to_le_bytes());
            out.extend_from_slice(&(height as i32).to_le_bytes());
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&24u16.to_le_bytes());
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&(image as u32).to_le_bytes());
            out.extend_from_slice(&[0; 16]);
            for y in (0..height).rev() {
                let start = out.len();
                for pixel in &pixels[y * width..(y + 1) * width] {
                    out.extend_from_slice(&[pixel.b, pixel.g, pixel.r]);
                }
                out.resize(start + row, 0);
            }
            std::fs::write(
                dir.join(format!("modal-focus-{}.bmp", (scale * 100.) as u32)),
                out,
            )
            .unwrap();
        }
        app.hide().unwrap();
    }
}

#[test]
fn modal_confirmation_field_receives_initial_keyboard_input() {
    slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
    let app = ModalFixture::new().unwrap();
    app.set_with_field(true);
    app.window().set_size(slint::LogicalSize::new(720., 520.));
    app.show().unwrap();
    app.set_opened(true);
    let adapter = ADAPTER.with(|a| a.borrow().as_ref().unwrap().clone());
    let size = app.window().size();
    let mut pixels = vec![slint::Rgb8Pixel::default(); (size.width * size.height) as usize];
    adapter.request_redraw();
    adapter.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, size.width as usize);
    });
    slint::platform::update_timers_and_animations();
    std::thread::sleep(std::time::Duration::from_millis(2));
    slint::platform::update_timers_and_animations();
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "x".into() });
    assert_eq!(app.get_entered().as_str(), "x");
}
fn key(window: &slint::Window, key: Key) {
    window.dispatch_event(WindowEvent::KeyPressed { text: key.into() });
    window.dispatch_event(WindowEvent::KeyReleased { text: key.into() });
}
#[test]
fn modal_tab_navigation_never_activates_background_and_escape_is_available() {
    slint::platform::set_platform(Box::new(TestPlatform)).unwrap();
    let background = Rc::new(Cell::new(0));
    let inside = Rc::new(Cell::new(0));
    for direction in [Key::Tab, Key::Backtab] {
        for tabs in 1..24 {
            let app = ModalFixture::new().unwrap();
            let seen = background.clone();
            app.on_background_action(move || seen.set(seen.get() + 1));
            let seen = inside.clone();
            app.on_dialog_action(move || seen.set(seen.get() + 1));
            let dismissed = Rc::new(Cell::new(0));
            let seen = dismissed.clone();
            app.on_dismissed(move || seen.set(seen.get() + 1));
            app.show().unwrap();
            app.invoke_focus_background();
            app.set_opened(true);
            for _ in 0..tabs {
                key(app.window(), direction);
            }
            key(app.window(), Key::Return);
            assert_eq!(
                background.get(),
                0,
                "Tab escaped the modal after {tabs} steps"
            );
            if !app.get_opened() {
                app.set_opened(true);
            }
            let before = dismissed.get();
            key(app.window(), Key::Escape);
            assert_eq!(dismissed.get(), before + 1, "Escape must stay available");
            assert!(!app.get_opened());
            let before = background.get();
            key(app.window(), Key::Return);
            assert_eq!(
                background.get(),
                before + 1,
                "Dismissal restores background focus"
            );
            background.set(0);
            app.hide().unwrap();
        }
    }
    assert!(
        inside.get() > 0,
        "The dialog action must remain keyboard reachable"
    );
}
