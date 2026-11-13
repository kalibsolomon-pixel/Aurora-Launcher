//! Compile-only harness for the GPUI Syncing module.
//!
//! It imports the shared modules the same way the host binary does (the
//! shared typed API straight out of the preserved Slint crate) plus the
//! syncing module under development, and nothing else. It never runs: this
//! exists so `cargo check --example syncing_check` exercises the module's
//! real dependency graph without registering it in `main.rs` or launching
//! any interface.

extern crate gpui_kit as gpui;

#[path = "../../native/src/api.rs"]
mod api;

#[path = "../src/backend.rs"]
mod backend;

#[path = "../src/controls.rs"]
mod controls;

#[path = "../src/borealis.rs"]
mod borealis;
#[path = "../src/images.rs"]
mod images;
#[path = "../src/theme.rs"]
mod theme;

#[path = "../src/syncing.rs"]
mod syncing;

fn main() {}
