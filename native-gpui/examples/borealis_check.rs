//! Compile-only harness for the GPUI Borealis background.
//!
//! It imports the shared modules the way the host binary does (the shared
//! typed API straight out of the preserved Slint crate and the image adapter)
//! plus the Borealis module under development, and nothing else. It never
//! runs and never opens a window: this exists so `cargo check --example
//! borealis_check` exercises the module's real dependency graph without
//! registering it in `main.rs`. The pure fixtures (pose math, animation gate
//! truth table, composition, state machine) live inside the module and run
//! with `cargo test --example borealis_check`.

extern crate gpui_kit as gpui;

#[allow(dead_code)]
#[path = "../../native/src/api.rs"]
mod api;

#[path = "../src/images.rs"]
mod images;

#[path = "../src/borealis.rs"]
mod borealis;

fn main() {
    let _ = (
        borealis::should_animate(0, false, false, false),
        borealis::cadence(1),
        borealis::Motion::default().run(),
    );
}
