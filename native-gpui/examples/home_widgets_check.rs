//! Compile-only harness for the GPUI Home widgets module.
//!
//! It imports the shared modules the same way the host binary does (the
//! shared typed API straight out of the preserved Slint crate) plus the
//! home-widgets module under development and the Media entity it embeds, and
//! nothing else. It never runs
//! and never opens a window: this exists so `cargo check --example
//! home_widgets_check` exercises the module's real dependency graph without
//! registering it in `main.rs` or launching any interface. The deterministic
//! fixture tests for the module's pure state machine live inside the module
//! itself and run with `cargo test --example home_widgets_check`.

extern crate gpui_kit as gpui;

#[allow(dead_code)]
#[path = "../../native/src/api.rs"]
mod api;

#[path = "../src/backend.rs"]
mod backend;

#[path = "../src/controls.rs"]
mod controls;

#[path = "../src/home_widgets.rs"]
mod home_widgets;

#[path = "../src/images.rs"]
mod images;

#[path = "../src/media.rs"]
mod media;

#[path = "../src/borealis.rs"]
mod borealis;
#[allow(dead_code)]
#[path = "../src/theme.rs"]
mod theme;

fn main() {
    let _ = (
        api::ApiError::new("unused", "compile-only"),
        controls::AuroraButton::new("probe", "unused"),
        theme::AuroraTheme::default(),
        home_widgets::HomeWidgetsEvent::Changed,
        home_widgets::HomeWidgetsEvent::BusyChanged(false),
    );
}
