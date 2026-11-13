//! Compile-only harness for the GPUI Media module.
//!
//! It imports the shared modules the same way the host binary does (the
//! shared typed API straight out of the preserved Slint crate) plus the
//! media module under development, and nothing else. It never runs and
//! never opens a window: this exists so `cargo check --example
//! media_check` exercises the module's real dependency graph without
//! registering it in `main.rs` or launching any interface. The
//! deterministic fixture tests for the module's pure lifecycle guards live
//! inside the module itself and run with `cargo test --example
//! media_check`.

extern crate gpui_kit as gpui;

#[allow(dead_code)]
#[path = "../../native/src/api.rs"]
mod api;

#[path = "../src/controls.rs"]
mod controls;

#[path = "../src/images.rs"]
mod images;

#[path = "../src/media.rs"]
mod media;

#[path = "../src/borealis.rs"]
mod borealis;
#[path = "../src/theme.rs"]
mod theme;

fn main() {
    let _ = (
        controls::AuroraButton::new("probe", "unused"),
        theme::AuroraTheme::default(),
        media::supported(),
    );
}
