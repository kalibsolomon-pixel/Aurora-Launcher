//! Compile-only harness for the GPUI Skins module.
//!
//! It imports the shared modules the same way the host binary does (the
//! shared typed API straight out of the preserved Slint crate) plus the
//! skins module under development, and nothing else. It never runs and never
//! opens a window: `cargo check --example skins_check` exercises the
//! module's real dependency graph without registering it in `main.rs` or
//! launching any interface.

extern crate gpui_kit as gpui;

#[path = "../../native/src/api.rs"]
mod api;

#[path = "../src/backend.rs"]
mod backend;

#[path = "../src/controls.rs"]
mod controls;

#[path = "../src/file_picker.rs"]
mod file_picker;

#[path = "../src/images.rs"]
mod images;

#[path = "../../native/src/player.rs"]
mod player;

#[path = "../../native/src/preview.rs"]
mod preview;

#[path = "../src/skins.rs"]
mod skins;

#[path = "../src/borealis.rs"]
mod borealis;
#[path = "../src/theme.rs"]
mod theme;

fn main() {
    let _ = (
        api::ApiError::new("unused", "compile-only"),
        controls::AuroraButton::new("probe", "unused"),
        theme::AuroraTheme::default(),
        skins::SkinsEvent::Changed,
        skins::SkinsEvent::BusyChanged(false),
        skins::SkinsEvent::PreviewChanged,
    );
}
