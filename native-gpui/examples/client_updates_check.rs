//! Compile-only harness for the GPUI client-updates module.
//!
//! It imports the shared modules the same way the host binary does (the
//! shared typed API straight out of the preserved Slint crate) plus the
//! client-updates module under development, and nothing else. It never runs
//! and never opens a window: `cargo check --example client_updates_check`
//! exercises the module's real dependency graph without registering it in
//! `main.rs` or launching any interface. The fixture tests for the module's
//! pure state machine live inside the module itself.

extern crate gpui_kit as gpui;

#[path = "../../native/src/api.rs"]
mod api;

#[path = "../src/backend.rs"]
mod backend;

#[path = "../src/client_updates.rs"]
mod client_updates;

#[path = "../src/controls.rs"]
mod controls;

#[path = "../src/borealis.rs"]
mod borealis;
#[path = "../src/images.rs"]
mod images;
#[path = "../src/theme.rs"]
mod theme;

fn main() {
    let _ = (
        api::ApiError::new("unused", "compile-only"),
        controls::AuroraButton::new("probe", "unused"),
        theme::AuroraTheme::default(),
        client_updates::ClientUpdatesEvent::Changed,
        client_updates::ClientUpdatesEvent::BusyChanged(false),
    );
}
