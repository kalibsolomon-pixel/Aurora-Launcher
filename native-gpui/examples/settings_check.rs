//! Compile-only harness for the GPUI Settings module.
//!
//! It imports the shared modules the same way the host binary does (the
//! shared typed API straight out of the preserved Slint crate) plus the
//! settings module under development, and nothing else. It never runs:
//! this exists so `cargo check --example settings_check` exercises the
//! module's real dependency graph without registering it in `main.rs` or
//! launching any interface.

extern crate gpui_kit as gpui;

#[path = "../../native/src/api.rs"]
mod api;

#[path = "../src/backend.rs"]
mod backend;

#[path = "../src/controls.rs"]
mod controls;

// The Borealis view settings previews; its pose, composition and gate
// fixtures run with this harness.
#[path = "../src/images.rs"]
mod images;

#[path = "../src/borealis.rs"]
mod borealis;
#[path = "../src/theme.rs"]
mod theme;

#[path = "../src/settings.rs"]
mod settings;

fn main() {
    let _ = (
        api::ApiError::new("unused", "compile-only"),
        controls::AuroraButton::new("probe", "unused"),
        theme::AuroraTheme::default(),
        settings::SettingsEvent::Changed,
        settings::SettingsEvent::Motion(0),
        settings::AppearanceState::default().reduce_motion(),
        settings::AppearanceState::default().motion_speed(),
        settings::AccentPickerState::open("#8b80ff"),
        borealis::should_animate(0, false, false, false),
    );
}
