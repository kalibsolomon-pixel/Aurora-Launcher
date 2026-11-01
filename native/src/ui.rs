//! UI-thread state and async dispatch.
//!
//! Launcher operations run on the Tokio runtime; their results return to the
//! Slint event loop through `slint::invoke_from_event_loop`. All mutable UI
//! state lives in one thread-local [`AppState`] owned by the UI thread, so
//! completions only need to capture `Send` data.

use std::cell::RefCell;
use std::future::Future;
use std::time::Duration;

use aurora_launcher_lib::host::Host;
use slint::ComponentHandle;

use crate::api::{
    AccountsState, ApiError, Appearance, InstanceSummary, LaunchProcess, LauncherState,
    PlayReadiness,
};
use crate::{AppWindow, Shell, Tone};

/// Everything the native UI knows, mirrored from Rust-owned launcher state.
/// Nothing here is an authority: every action re-validates in `application`.
#[derive(Default)]
pub struct AppState {
    pub launcher: Option<LauncherState>,
    pub accounts: AccountsState,
    pub appearance: Option<Appearance>,
    pub readiness: Option<PlayReadiness>,
    pub process: Option<LaunchProcess>,
    /// Bumped whenever readiness inputs change; stale responses are dropped.
    pub readiness_serial: u64,
    pub readiness_rechecks: u8,
    pub readiness_in_flight: bool,
    pub readiness_pending: bool,
    pub play_busy: bool,
    pub play_progress: Option<String>,
    pub play_error: Option<String>,
    pub install_progress: Option<(String, f32)>,
    pub avatars: std::collections::HashMap<String, slint::Image>,
    pub workspace: Option<crate::instances::Workspace>,
    pub discover: crate::discover::DiscoverState,
    pub toast_serial: u64,
    pub skin_presets: Vec<crate::api::SkinPreset>,
    pub skin_thumbnails: std::collections::HashMap<String, slint::Image>,
    pub skin_preview: crate::skins::PreviewState,
    pub client_update: Option<(String, String)>,
    pub icon_picker: crate::icons::PickerState,
    pub sync_policy: Option<crate::api::SyncPolicy>,
    pub sync_instance_ids: Vec<String>,
    pub sync_conflicts: Vec<crate::api::SyncConflict>,
}

impl AppState {
    pub fn instances(&self) -> &[InstanceSummary] {
        self.launcher
            .as_ref()
            .map(|state| state.instances.as_slice())
            .unwrap_or(&[])
    }

    pub fn selected_instance(&self) -> Option<&InstanceSummary> {
        let state = self.launcher.as_ref()?;
        let id = state.config.selected_instance_id.as_deref()?;
        state.instances.iter().find(|instance| instance.id == id)
    }

    pub fn instance(&self, id: &str) -> Option<&InstanceSummary> {
        self.instances().iter().find(|instance| instance.id == id)
    }

    /// The stored Material Symbols name for an instance, if any.
    pub fn icon_name(&self, id: &str) -> Option<&str> {
        self.launcher.as_ref()?.icons.get(id).map(String::as_str)
    }

    /// The SVG path rendered for an instance (fallback when unset/unknown).
    pub fn icon_path(&self, id: &str) -> &'static str {
        crate::symbols::path_or_fallback(self.icon_name(id))
    }
}

type Continuation = Box<dyn FnOnce()>;

thread_local! {
    static PARKED: RefCell<(u64, std::collections::HashMap<u64, Continuation>)> = RefCell::default();
    static WINDOW: RefCell<Option<AppWindow>> = const { RefCell::new(None) };
    static STATE: RefCell<AppState> = RefCell::new(AppState::default());
    static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) };
}

#[derive(Clone)]
struct Context {
    host: Host,
    runtime: tokio::runtime::Handle,
}

/// Drops the UI-thread window handle and state before the process exits.
pub fn release() {
    WINDOW.with(|slot| slot.borrow_mut().take());
    STATE.with(|state| *state.borrow_mut() = AppState::default());
    PARKED.with(|parked| parked.borrow_mut().1.clear());
}

pub fn install(window: AppWindow, host: Host, runtime: tokio::runtime::Handle) {
    WINDOW.with(|slot| *slot.borrow_mut() = Some(window));
    CONTEXT.with(|slot| *slot.borrow_mut() = Some(Context { host, runtime }));
}

/// Runs `f` with the window and state. Must be called on the UI thread.
pub fn with<R>(f: impl FnOnce(&AppWindow, &mut AppState) -> R) -> Option<R> {
    WINDOW.with(|window| {
        let window = window.borrow();
        let window = window.as_ref()?;
        STATE.with(|state| Some(f(window, &mut state.borrow_mut())))
    })
}

pub fn state<R>(f: impl FnOnce(&AppState) -> R) -> R {
    STATE.with(|state| f(&state.borrow()))
}

pub fn host() -> Host {
    CONTEXT.with(|context| {
        context
            .borrow()
            .as_ref()
            .expect("ui context installed")
            .host
            .clone()
    })
}

/// Runs `work` on the async runtime and hands its output to `done` on the UI thread.
pub fn spawn<F, T>(work: F, done: impl FnOnce(T) + Send + 'static)
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let runtime = CONTEXT.with(|context| {
        context
            .borrow()
            .as_ref()
            .expect("ui context installed")
            .runtime
            .clone()
    });
    runtime.spawn(async move {
        let value = work.await;
        let _ = slint::invoke_from_event_loop(move || done(value));
    });
}

/// Parks a UI-thread continuation (which need not be `Send`) until a
/// background request completes; [`resume`] runs it exactly once.
pub fn park(then: impl FnOnce() + 'static) -> u64 {
    PARKED.with(|parked| {
        let mut parked = parked.borrow_mut();
        parked.0 += 1;
        let token = parked.0;
        parked.1.insert(token, Box::new(then));
        token
    })
}

pub fn resume(token: u64) {
    let then = PARKED.with(|parked| parked.borrow_mut().1.remove(&token));
    if let Some(then) = then {
        then();
    }
}

/// Shows a transient notification; it hides itself after a few seconds
/// unless it reports an error, which stays until dismissed.
pub fn toast(text: impl Into<String>, tone: Tone) {
    let text = text.into();
    let serial = with(|window, state| {
        state.toast_serial += 1;
        let shell = window.global::<Shell>();
        shell.set_toast_text(text.into());
        shell.set_toast_tone(tone);
        shell.set_toast_visible(true);
        state.toast_serial
    });
    if tone != Tone::Error {
        if let Some(serial) = serial {
            slint::Timer::single_shot(Duration::from_secs(5), move || {
                with(|window, state| {
                    if state.toast_serial == serial {
                        window.global::<Shell>().set_toast_visible(false);
                    }
                });
            });
        }
    }
}

pub fn error_toast(error: &ApiError) {
    toast(error.message.clone(), Tone::Error);
}

/// First letter of a display name for glyph fallbacks.
pub fn letter(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into())
}

pub fn model<T: Clone + 'static>(items: Vec<T>) -> slint::ModelRc<T> {
    slint::ModelRc::new(slint::VecModel::from(items))
}

/// Converts square RGBA bytes (e.g. an 8×8 head) into an image.
pub fn rgba_image(rgba: &[u8]) -> Option<slint::Image> {
    let pixels = rgba.len() / 4;
    let side = (pixels as f64).sqrt() as u32;
    if side == 0 || (side * side * 4) as usize != rgba.len() {
        return None;
    }
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(rgba, side, side);
    Some(slint::Image::from_rgba8(buffer))
}

pub fn rectangular_image(rgba: &[u8], width: u32, height: u32) -> Option<slint::Image> {
    if width == 0
        || height == 0
        || width > 1024
        || height > 1024
        || rgba.len() != width as usize * height as usize * 4
    {
        return None;
    }
    Some(slint::Image::from_rgba8(slint::SharedPixelBuffer::<
        slint::Rgba8Pixel,
    >::clone_from_slice(
        rgba, width, height
    )))
}
