//! Compile-only harness for the GPUI Accounts module.
//!
//! It imports the shared modules the same way the host binary does (the
//! shared typed API straight out of the preserved Slint crate) plus the
//! accounts module under development, and nothing else. It never runs and
//! never opens a window: this exists so `cargo check --example
//! accounts_check` exercises the module's real dependency graph without
//! registering it in `main.rs` or launching any interface.

extern crate gpui_kit as gpui;

#[path = "../../native/src/api.rs"]
mod api;

#[path = "../src/backend.rs"]
mod backend;

#[path = "../src/images.rs"]
mod images;

#[path = "../src/controls.rs"]
mod controls;

#[path = "../src/borealis.rs"]
mod borealis;
#[path = "../src/theme.rs"]
mod theme;

#[path = "../src/accounts.rs"]
mod accounts;

fn main() {
    let _ = (
        api::ApiError::new("unused", "compile-only"),
        controls::AuroraButton::new("probe", "unused"),
        theme::AuroraTheme::default(),
        accounts::AccountsEvent::Changed,
        accounts::AccountsEvent::BusyChanged(false),
    );
}
