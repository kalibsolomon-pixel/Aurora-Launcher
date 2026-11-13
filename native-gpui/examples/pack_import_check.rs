//! Non-launching compile/test harness. Fixtures live with the real module.
extern crate gpui_kit as gpui;
#[path = "../../native/src/api.rs"]
mod api;
#[path = "../src/backend.rs"]
mod backend;
#[path = "../src/borealis.rs"]
mod borealis;
#[path = "../src/controls.rs"]
mod controls;
#[path = "../src/file_picker.rs"]
mod file_picker;
#[path = "../src/images.rs"]
mod images;
#[path = "../src/pack_import.rs"]
mod pack_import;
#[path = "../src/theme.rs"]
mod theme;
fn main() {}
