//! Compile-only player preview harness; never opens a window.
extern crate gpui_kit as gpui;
#[path = "../../native/src/api.rs"]
mod api;
#[path = "../src/backend.rs"]
mod backend;
#[path = "../src/borealis.rs"]
mod borealis;
#[path = "../src/controls.rs"]
mod controls;
#[path = "../src/images.rs"]
mod images;
#[path = "../../native/src/player.rs"]
mod player;
#[path = "../../native/src/preview.rs"]
mod preview;
#[path = "../src/skin_preview.rs"]
mod skin_preview;
#[path = "../src/theme.rs"]
mod theme;
fn main() {}
