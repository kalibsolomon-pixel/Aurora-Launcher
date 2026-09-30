//! Native window-state signals for decorative frontend work.
//!
//! A minimized WebView2 host window keeps reporting the page as visible
//! (`document.hidden` stays false because the controller visibility never
//! changes), and the page blur that normally accompanies minimization can be
//! lost when focus transitions race the minimize. The native window is
//! therefore the only authority for "the launcher is minimized", and this
//! module forwards exactly that one boolean across the boundary.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, WebviewWindow, WindowEvent};

/// The single window-state event the frontend may observe. Payload is the
/// current minimized state as a plain boolean.
pub const WINDOW_MINIMIZED_EVENT: &str = "window-minimized";

/// Window events that can accompany a minimize or restore transition and
/// therefore justify re-reading native minimize state.
pub fn probes_minimize(event: &WindowEvent) -> bool {
    matches!(event, WindowEvent::Focused(_) | WindowEvent::Resized(_))
}

/// Pure transition decision: `Some(new)` when native state differs from the
/// tracked state, `None` when nothing changed (no event churn).
pub fn transition(tracked: bool, native: bool) -> Option<bool> {
    (tracked != native).then_some(native)
}

/// Emits [`WINDOW_MINIMIZED_EVENT`] whenever the window's native minimize
/// state changes. Checks run only on focus/resize window events; there is no
/// polling. A failed minimize query keeps the previous state.
pub fn attach(app: &AppHandle, window: &WebviewWindow) {
    let app = app.clone();
    let listener = window.clone();
    let minimized = AtomicBool::new(window.is_minimized().unwrap_or(false));
    window.on_window_event(move |event| {
        if !probes_minimize(event) {
            return;
        }
        let tracked = minimized.load(Ordering::Relaxed);
        let native = listener.is_minimized().unwrap_or(tracked);
        if let Some(next) = transition(tracked, native) {
            minimized.store(next, Ordering::Relaxed);
            let _ = app.emit(WINDOW_MINIMIZED_EVENT, next);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_focus_and_resize_probe_minimize_state() {
        assert!(probes_minimize(&WindowEvent::Focused(true)));
        assert!(probes_minimize(&WindowEvent::Focused(false)));
        assert!(probes_minimize(&WindowEvent::Resized(
            tauri::PhysicalSize::new(0, 0)
        )));
        assert!(!probes_minimize(&WindowEvent::Moved(
            tauri::PhysicalPosition::new(0, 0)
        )));
        assert!(!probes_minimize(&WindowEvent::Destroyed));
    }

    #[test]
    fn transition_emits_only_on_state_change() {
        assert_eq!(transition(true, true), None);
        assert_eq!(transition(false, false), None);
        assert_eq!(transition(false, true), Some(true));
        assert_eq!(transition(true, false), Some(false));
    }
}
