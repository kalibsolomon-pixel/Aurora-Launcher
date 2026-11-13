//! Accounts for the GPUI host: Microsoft sign-in through the system browser,
//! account selection, explicit per-account session refresh and reviewed
//! sign-out. The preserved Slint host (`native/src/accounts.rs`) and the
//! provisional root (`native-gpui/src/main.rs`, page 1) are the behavior
//! references; the typed API in `native/src/api.rs` stays the only backend
//! boundary and `AccountsState`/`AccountSummary` remain authoritative for
//! status and identity.
//!
//! Contract kept here:
//! - Constructor and navigation refreshes load account listings and cached
//!   artwork, then ask the existing cache-first avatar API for missing heads.
//!   That API may restore a saved account's session on a cache miss.
//! - One active account/auth mutation at a time; sign-in runs in the user's
//!   system browser (no webview, no password, no auto sign-in).
//!   Cancellation is acknowledged on its own and never clears busy;
//!   busy clears only when the underlying login settles.
//! - Tagged operations plus a read generation counter reject stale and
//!   duplicate terminal replies, so a pre-mutation listing can never restore
//!   a removed or superseded selection. Core remains authoritative.
//! - A failed read never fabricates a writable account snapshot; retry stays
//!   reachable. Removal requires a review that names the exact username and
//!   account id, frozen at open time and rejected when the listing no longer
//!   matches.
//! - Host events carry coarse `auth-progress` phases only; no credential,
//!   token, or raw `refresh_session` payload is ever rendered or logged.
//!
//! Honest parity limits, kept deliberate:
//! - The busy state applies to row buttons and the removal dialog; there is
//!   no text input on this page, so input disabling is not applicable here.

#![allow(dead_code)]

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use aurora_launcher_lib::host::Host;
use gpui_kit::base::Dialog;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Context, EventEmitter, FocusHandle, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled, Window, div, img, px,
    rgba,
};

use crate::api::{self, ApiResult};
use crate::backend::guarded;
use crate::controls::{AuroraButton, ButtonSize, ButtonVariant};
use crate::{theme, ui};

/// Emitted for the integrating parent view.
#[derive(Debug)]
pub enum AccountsEvent {
    /// A mutation or login settled and may have changed persisted state,
    /// including a failed refresh marking an account for reauthentication.
    /// Never a read or cancellation acknowledgment alone.
    Changed,
    /// The page's busy state changed. Each transition is reported once.
    BusyChanged(bool),
}

// ---------------------------------------------------------------------------
// Pure page core (testable without a host or window)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Mutation {
    Select(String),
    Remove(String),
    Refresh(String),
}

impl Mutation {
    fn account_id(&self) -> &str {
        match self {
            Mutation::Select(id) | Mutation::Remove(id) | Mutation::Refresh(id) => id,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Review {
    id: String,
    name: String,
}

#[derive(Debug, PartialEq)]
struct Start {
    tag: u64,
    busy: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LoadGuard {
    serial: u64,
    epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CancelGuard {
    login: u64,
    serial: u64,
}

#[derive(Debug, PartialEq)]
enum MutationOutcome {
    /// A stale or duplicate terminal reply was ignored.
    Ignored,
    Acknowledged {
        ok: bool,
        busy: Option<bool>,
    },
}

#[derive(Debug, PartialEq)]
enum LoginOutcome {
    Ignored,
    Settled {
        ok: bool,
        cancelled: bool,
        busy: Option<bool>,
    },
}

#[derive(Default)]
struct AccountsCore {
    accounts: Option<api::AccountsState>,
    load_error: Option<String>,
    load_in_flight: Option<LoadGuard>,
    load_serial: u64,
    load_pending: bool,
    read_tag: u64,
    mutation: Option<(u64, Mutation)>,
    login: Option<u64>,
    cancel_requested: bool,
    cancel_unavailable: bool,
    cancel_serial: u64,
    cancel: Option<CancelGuard>,
    phase: Option<String>,
    error: Option<String>,
    notice: Option<String>,
    review: Option<Review>,
    review_open: bool,
    busy: bool,
}

impl AccountsCore {
    fn active(&self) -> bool {
        self.mutation.is_some() || self.login.is_some()
    }

    /// The busy transition, if any, exactly once per actual change.
    fn sync_busy(&mut self) -> Option<bool> {
        let active = self.active();
        if self.busy != active {
            self.busy = active;
            Some(active)
        } else {
            None
        }
    }

    /// Registers a read-only listing request; `None` while one is already in
    /// flight (the later completion re-arms it).
    fn request_load(&mut self) -> Option<LoadGuard> {
        if self.load_in_flight.is_some() || self.active() {
            self.load_pending = true;
            return None;
        }
        self.load_pending = false;
        self.load_serial = self.load_serial.wrapping_add(1);
        let guard = LoadGuard {
            serial: self.load_serial,
            epoch: self.read_tag,
        };
        self.load_in_flight = Some(guard);
        Some(guard)
    }

    /// Completes a listing read. Returns whether another read must be
    /// dispatched (a pending request). A pre-mutation result — any tag older
    /// than the current generation — is never applied, so it cannot restore
    /// a removed or superseded account state.
    fn read_completed(&mut self, tag: LoadGuard, result: ApiResult<api::AccountsState>) -> bool {
        if self.load_in_flight != Some(tag) {
            return false;
        }
        let pending = self.load_pending;
        self.load_in_flight = None;
        self.load_pending = false;
        if tag.epoch != self.read_tag || self.active() {
            return pending;
        }
        match result {
            Ok(state) => {
                self.accounts = Some(state);
                self.load_error = None;
            }
            // A failed read never fabricates a writable snapshot.
            Err(error) => {
                self.accounts = None;
                self.load_error = Some(error.message);
            }
        }
        pending
    }

    fn request_mutation(&mut self, mutation: Mutation) -> Option<Start> {
        if self.active()
            || !self.accounts.as_ref().is_some_and(|state| {
                state
                    .accounts
                    .iter()
                    .any(|account| account.account_id == mutation.account_id())
            })
        {
            return None;
        }
        self.error = None;
        self.notice = None;
        self.read_tag = self.read_tag.wrapping_add(1);
        let tag = self.read_tag;
        self.phase = match &mutation {
            Mutation::Refresh(_) => Some(auth_phase_text("restoringSession").to_owned()),
            _ => None,
        };
        self.mutation = Some((tag, mutation));
        let busy = self.sync_busy();
        Some(Start { tag, busy })
    }

    fn mutation_settled(&mut self, tag: u64, result: ApiResult<()>) -> MutationOutcome {
        let Some((current_tag, mutation)) = self.mutation.clone() else {
            return MutationOutcome::Ignored;
        };
        if current_tag != tag {
            return MutationOutcome::Ignored;
        }
        self.mutation = None;
        self.read_tag = self.read_tag.wrapping_add(1);
        self.accounts = None;
        self.phase = None;
        let busy = self.sync_busy();
        match result {
            Ok(()) => {
                if matches!(mutation, Mutation::Remove(_)) {
                    self.notice = Some(SIGNED_OUT.to_owned());
                }
                MutationOutcome::Acknowledged { ok: true, busy }
            }
            Err(error) => {
                self.error = Some(error.message);
                MutationOutcome::Acknowledged { ok: false, busy }
            }
        }
    }

    fn login_started(&mut self) -> Option<Start> {
        if self.active() || self.accounts.is_none() || self.cancel.is_some() {
            return None;
        }
        self.error = None;
        self.notice = None;
        self.cancel_requested = false;
        self.cancel_unavailable = false;
        self.cancel = None;
        self.read_tag = self.read_tag.wrapping_add(1);
        let tag = self.read_tag;
        self.phase = Some(auth_phase_text("waitingForMicrosoft").to_owned());
        self.login = Some(tag);
        let busy = self.sync_busy();
        Some(Start { tag, busy })
    }

    fn login_settled(&mut self, tag: u64, result: ApiResult<api::AccountSummary>) -> LoginOutcome {
        if self.login != Some(tag) {
            return LoginOutcome::Ignored;
        }
        self.login = None;
        self.read_tag = self.read_tag.wrapping_add(1);
        self.accounts = None;
        self.cancel_requested = false;
        self.cancel_unavailable = false;
        self.phase = None;
        self.notice = None;
        self.error = None;
        let busy = self.sync_busy();
        match result {
            Ok(account) => {
                self.notice = Some(format!("Signed in as {}.", account.minecraft_name));
                LoginOutcome::Settled {
                    ok: true,
                    cancelled: false,
                    busy,
                }
            }
            Err(error) => {
                let cancelled = matches!(
                    error.code.as_str(),
                    "auth_cancelled" | "auth_login_cancelled"
                );
                if !cancelled {
                    self.error = Some(error.message);
                }
                LoginOutcome::Settled {
                    ok: false,
                    cancelled,
                    busy,
                }
            }
        }
    }

    /// Requests cancellation. Its acknowledgment never clears busy.
    fn request_cancel(&mut self) -> Option<CancelGuard> {
        let login = self.login?;
        if self.cancel_requested || self.cancel_unavailable {
            return None;
        }
        self.error = None;
        self.cancel_requested = true;
        self.cancel_serial = self.cancel_serial.wrapping_add(1);
        let guard = CancelGuard {
            login,
            serial: self.cancel_serial,
        };
        self.cancel = Some(guard);
        Some(guard)
    }

    /// Request identity rejects stale completions. Busy clears only when the
    /// sign-in settles; TooLate reports the non-cancellable persistence boundary.
    fn cancel_acknowledged(
        &mut self,
        guard: CancelGuard,
        result: ApiResult<api::LoginCancelOutcome>,
    ) -> Option<String> {
        if self.cancel != Some(guard) {
            return None;
        }
        self.cancel = None;
        if self.login == Some(guard.login) {
            match result {
                Ok(api::LoginCancelOutcome::Requested) => {
                    self.cancel_requested = true;
                    None
                }
                Ok(api::LoginCancelOutcome::TooLate) => {
                    self.cancel_requested = false;
                    self.cancel_unavailable = true;
                    self.notice = Some(
                        "Sign-in is already finishing. Its account result will appear shortly."
                            .into(),
                    );
                    None
                }
                Err(error) => {
                    self.cancel_requested = false;
                    Some(error.message)
                }
            }
        } else {
            None
        }
    }

    /// Live coarse phase text while a login or session refresh is active.
    fn auth_progress(&mut self, phase: &str) -> bool {
        let auth_related =
            self.login.is_some() || matches!(&self.mutation, Some((_, Mutation::Refresh(_))));
        if !auth_related {
            return false;
        }
        let text = auth_phase_text(phase);
        if self.phase.as_deref() == Some(text) {
            return false;
        }
        self.phase = Some(text.to_owned());
        true
    }

    /// Opens the removal review with a frozen identity that must still be
    /// listed exactly as named.
    fn open_review(&mut self, id: String, name: String) -> bool {
        if self.active() {
            return false;
        }
        let listed = self.accounts.as_ref().is_some_and(|state| {
            state
                .accounts
                .iter()
                .any(|account| account.account_id == id && account.minecraft_name == name)
        });
        if !listed {
            return false;
        }
        self.review = Some(Review { id, name });
        self.review_open = true;
        true
    }

    /// Confirms the frozen review. A stale review — the listing no longer
    /// contains that exact username and account id — is rejected.
    fn confirm_review(&mut self, expected: &Review) -> Option<Mutation> {
        let review = self.review.clone()?;
        if self.active() || !self.review_open || &review != expected {
            return None;
        }
        self.review_open = false;
        let stale = !self.accounts.as_ref().is_some_and(|state| {
            state.accounts.iter().any(|account| {
                account.account_id == review.id && account.minecraft_name == review.name
            })
        });
        self.review = None;
        if stale {
            self.error = Some(STALE_REVIEW.to_owned());
            return None;
        }
        Some(Mutation::Remove(review.id))
    }

    fn dismiss_review(&mut self, expected: Option<&Review>) {
        if self.active() || self.review.as_ref() != expected {
            return;
        }
        self.review_open = false;
        self.review = None;
    }
}

const SIGNED_OUT: &str = "Signed out. The stored credential was removed.";
const STALE_REVIEW: &str = "The account list changed. Refresh and review the removal again.";

/// Coarse phases only, never credentials (same wording as the Slint host).
fn auth_phase_text(phase: &str) -> &'static str {
    match phase {
        "waitingForMicrosoft" => {
            "Continue in your browser. Sign in with Microsoft, then return here."
        }
        "exchangingMicrosoftToken" => "Confirming your Microsoft sign-in…",
        "authenticatingWithXbox" => "Signing in to Xbox services…",
        "authorizingXsts" => "Authorizing with Xbox services…",
        "authenticatingMinecraft" => "Signing in to Minecraft…",
        "checkingEntitlement" => "Checking that you own Minecraft…",
        "fetchingProfile" => "Loading your Minecraft profile…",
        "savingAccount" => "Saving your account…",
        "restoringSession" => "Restoring your session…",
        _ => "Signing in…",
    }
}

fn status_text(status: &str) -> &'static str {
    match status {
        "signedIn" => "Signed in",
        "reauthenticationRequired" => "Sign in again to play",
        _ => "Unavailable",
    }
}

/// Quiet session state: a colored dot plus caption text, never a pill.
/// "Sign in again" is reserved for the single call to action on the page.
fn status_line(t: &theme::AuroraTheme, status: &str, extra: Option<&'static str>) -> gpui_kit::Div {
    let (label, color, text_color) = match status {
        "signedIn" => ("Signed in", t.success, t.subtle_foreground),
        "reauthenticationRequired" => ("Session expired", t.warning, t.warning),
        _ => ("Unavailable", t.subtle_foreground, t.subtle_foreground),
    };
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(ui::TEXT_CAPTION))
        .child(div().flex_shrink_0().size(px(6.)).rounded_full().bg(color))
        .child(div().text_color(text_color).child(label))
        .when_some(extra, |row, extra| {
            row.child(
                div()
                    .text_color(t.subtle_foreground)
                    .child(format!("· {extra}")),
            )
        })
}

/// An inline status strip for progress, notices and errors.
fn notice_bar(
    t: &theme::AuroraTheme,
    text: impl Into<gpui_kit::SharedString>,
    tone: ui::Tone,
) -> gpui_kit::Div {
    let (color, icon) = match tone {
        ui::Tone::Danger => (t.danger, "error"),
        ui::Tone::Accent => (t.ring, "info"),
        _ => (t.muted_foreground, "info"),
    };
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(14.))
        .py(px(10.))
        .rounded(t.radius)
        .bg(Hsla {
            a: 0.10,
            ..if tone == ui::Tone::Neutral {
                t.foreground
            } else {
                color
            }
        })
        .border_1()
        .border_color(Hsla { a: 0.22, ..color })
        .text_size(px(ui::TEXT_BODY))
        .text_color(if tone == ui::Tone::Neutral {
            t.foreground
        } else {
            color
        })
        .child(crate::symbols::icon(Some(icon), px(16.), color))
        .child(div().flex_1().child(text.into()))
}

fn initial_letter(name: &str) -> String {
    name.chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into())
}

/// A quiet icon-only row action. The label is the accessible name.
fn icon_button(
    t: &theme::AuroraTheme,
    id: impl Into<gpui_kit::ElementId>,
    icon: &'static str,
    label: impl Into<gpui_kit::SharedString>,
) -> gpui_kit::base::Button {
    let clear = gpui_kit::transparent_black();
    let ring = t.ring;
    let hover = Hsla {
        a: 0.16,
        ..t.danger
    };
    gpui_kit::base::Button::new(id)
        .aria_label(label.into())
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded(t.radius)
        .bg(clear)
        .border_1()
        .border_color(clear)
        .hover(move |style| style.bg(hover))
        .active(|style| style.opacity(0.88))
        .focus(move |style| style.border_color(ring))
        .styles(|styles| styles.disabled(|style| style.opacity(0.42)))
        .child(crate::symbols::icon(
            Some(icon),
            px(16.),
            t.muted_foreground,
        ))
}

// ---------------------------------------------------------------------------
// Accounts view
// ---------------------------------------------------------------------------

enum Message {
    Read(LoadGuard, ApiResult<api::AccountsState>),
    Login(u64, ApiResult<api::AccountSummary>),
    Cancel(CancelGuard, ApiResult<api::LoginCancelOutcome>),
    Mutation(u64, ApiResult<()>),
}

// Artwork has its own generations; it never participates in account mutations.
#[derive(Default)]
struct Avatars {
    generation: u64,
    epoch: u64,
    identities: HashMap<String, String>,
    images: HashMap<String, Arc<gpui::RenderImage>>,
    image_generations: HashMap<String, u64>,
    cache_pending: Option<u64>,
    cache_ready: bool,
    pending: HashMap<String, u64>,
    attempted: HashSet<String>,
}

impl Avatars {
    fn listing(&mut self, state: &api::AccountsState, epoch: u64) {
        self.generation = self.generation.wrapping_add(1);
        self.epoch = epoch;
        let identities: HashMap<_, _> = state
            .accounts
            .iter()
            .map(|a| (a.account_id.clone(), a.status.clone()))
            .collect();
        self.images.retain(|id, _| {
            identities
                .get(id)
                .is_some_and(|status| self.identities.get(id) == Some(status))
        });
        self.image_generations
            .retain(|id, _| self.images.contains_key(id));
        self.identities = identities;
        self.attempted.clear();
        self.cache_ready = false;
    }

    fn valid(&self, generation: u64, epoch: u64) -> bool {
        self.generation == generation && self.epoch == epoch
    }

    fn missing(&self, epoch: u64) -> Vec<String> {
        if !self.cache_ready || self.epoch != epoch {
            return Vec::new();
        }
        self.identities
            .iter()
            .filter(|(id, status)| {
                *status == "signedIn"
                    && !self.images.contains_key(*id)
                    && !self.pending.contains_key(*id)
                    && !self.attempted.contains(*id)
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn accept_cache(&mut self, generation: u64, epoch: u64, entries: Vec<api::CachedAvatar>) {
        if !self.valid(generation, epoch) {
            return;
        }
        for entry in entries {
            if self.identities.contains_key(&entry.account_id)
                && self.image_generations.get(&entry.account_id) != Some(&generation)
            {
                if let Some(image) = Self::image(entry.avatar) {
                    self.image_generations
                        .insert(entry.account_id.clone(), generation);
                    self.images.insert(entry.account_id, image);
                }
            }
        }
    }

    fn accept_fetched(&mut self, generation: u64, epoch: u64, id: &str, avatar: api::HeadAvatar) {
        if self.valid(generation, epoch)
            && self
                .identities
                .get(id)
                .is_some_and(|status| status == "signedIn")
            && let Some(image) = Self::image(avatar)
        {
            self.image_generations.insert(id.to_owned(), generation);
            self.images.insert(id.to_owned(), image);
        }
    }

    fn image(avatar: api::HeadAvatar) -> Option<Arc<gpui::RenderImage>> {
        let len = avatar.rgba.len();
        if len == 0 || len % 4 != 0 {
            return None;
        }
        let pixels = len / 4;
        let side = (pixels as f64).sqrt() as usize;
        if side == 0 || side > 1024 || side.checked_mul(side) != Some(pixels) {
            return None;
        }
        crate::images::rgba_image(side as u32, side as u32, avatar.rgba)
    }
}

pub struct Accounts {
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    core: AccountsCore,
    avatars: Avatars,
    page_focus: FocusHandle,
    blocked: bool,
    login_cancellation: Option<(u64, api::LoginCancellation)>,
}

impl Accounts {
    /// Builds the page. With a host, a read-only account listing is loaded.
    /// Without a host the page stays in an unavailable diagnostic state and
    /// initiates zero API, auth, credential or integration calls.
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            host,
            rt: runtime,
            core: AccountsCore::default(),
            avatars: Avatars::default(),
            page_focus: cx.focus_handle(),
            blocked: false,
            login_cancellation: None,
        };
        view.refresh(cx);
        view
    }

    /// Read-only account listing; never mutates account state.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        let Some(tag) = self.core.request_load() else {
            return;
        };
        let host = self.host.clone().unwrap();
        cx.notify();
        self.run(
            cx,
            async move { api::accounts(host).await },
            move |view, result, cx| view.receive(Message::Read(tag, result), cx),
        );
    }

    /// Routes one host event. Only coarse `auth-progress` phases matter;
    /// unknown names, unparseable payloads and events without an active auth
    /// operation are ignored, never fatal.
    pub fn on_host_event(
        &mut self,
        name: &str,
        payload: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if self.host.is_none() || name != "auth-progress" {
            return;
        }
        let Some(phase) = payload.get("phase").and_then(serde_json::Value::as_str) else {
            return;
        };
        if self.core.auth_progress(phase) {
            cx.notify();
        }
    }

    pub fn is_busy(&self) -> bool {
        self.core.busy
    }

    /// Prevents starting account work during a root launch/preparation action.
    /// An already-started sign-in remains cancellable until it settles.
    pub fn set_blocked(&mut self, blocked: bool, cx: &mut Context<Self>) {
        if self.blocked != blocked {
            self.blocked = blocked;
            cx.notify();
        }
    }

    // -- dispatch -----------------------------------------------------------

    /// Runs one backend call through the panic-safe boundary and delivers the
    /// result through an awaited oneshot channel, never by polling.
    fn run<T, A>(
        &self,
        cx: &mut Context<Self>,
        work: A,
        deliver: impl Fn(&mut Self, ApiResult<T>, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
        A: std::future::Future<Output = ApiResult<T>> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let result = guarded(work).await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            let result = rx.await.unwrap_or_else(|_| {
                Err(api::ApiError::new(
                    "native_operation_failed",
                    "The account operation stopped unexpectedly. Refresh and try again.",
                ))
            });
            let _ = weak.update(cx, |view, cx| deliver(view, result, cx));
        })
        .detach();
    }

    fn load_avatar_cache(&mut self, cx: &mut Context<Self>) {
        if self.avatars.cache_pending.is_some() {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let generation = self.avatars.generation;
        self.avatars.cache_pending = Some(generation);
        self.run(
            cx,
            async move { api::cached_avatars(host).await },
            move |view, result, cx| {
                if view.avatars.cache_pending != Some(generation) {
                    return;
                }
                view.avatars.cache_pending = None;
                if !view.avatars.valid(generation, view.core.read_tag) {
                    if view.avatars.epoch == view.core.read_tag {
                        view.load_avatar_cache(cx);
                    }
                    return;
                }
                if let Ok(entries) = result {
                    view.avatars
                        .accept_cache(generation, view.core.read_tag, entries);
                }
                view.avatars.cache_ready = true;
                view.load_missing_avatars(cx);
                cx.notify();
            },
        );
    }

    fn load_missing_avatars(&mut self, cx: &mut Context<Self>) {
        if !self.avatars.cache_ready || self.avatars.epoch != self.core.read_tag {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let ids = self.avatars.missing(self.core.read_tag);
        for id in ids {
            let generation = self.avatars.generation;
            self.avatars.attempted.insert(id.clone());
            self.avatars.pending.insert(id.clone(), generation);
            let request_id = id.clone();
            let host = host.clone();
            self.run(
                cx,
                async move { api::account_avatar(host, request_id).await },
                move |view, result, cx| {
                    if view.avatars.pending.get(&id) != Some(&generation) {
                        return;
                    }
                    view.avatars.pending.remove(&id);
                    if let Ok(Some(avatar)) = result {
                        view.avatars
                            .accept_fetched(generation, view.core.read_tag, &id, avatar);
                    }
                    view.load_missing_avatars(cx);
                    cx.notify();
                },
            );
        }
    }

    fn emit_start(&mut self, start: &Start, cx: &mut Context<Self>) {
        if let Some(busy) = start.busy {
            cx.emit(AccountsEvent::BusyChanged(busy));
        }
        cx.notify();
    }

    fn load_after_settle(&mut self, cx: &mut Context<Self>) {
        self.refresh(cx);
    }

    /// A rounded avatar tile: the cached head when present, else the initial.
    #[allow(clippy::too_many_arguments)]
    fn avatar_tile(
        &self,
        t: &theme::AuroraTheme,
        id: &str,
        name: &str,
        selected: bool,
        size: f32,
        radius: f32,
        letter: f32,
    ) -> gpui_kit::Div {
        let image = self.avatars.images.get(id).cloned();
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .size(px(size))
            .rounded(px(radius))
            .overflow_hidden()
            .bg(if selected { t.accent_soft } else { t.control })
            .border_1()
            .border_color(if selected {
                Hsla { a: 0.40, ..t.ring }
            } else {
                t.border_subtle
            })
            .text_size(px(letter))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if selected { t.ring } else { t.muted_foreground })
            .map(|d| match image {
                Some(image) => d.child(img(image).size(px(size))),
                None => d.child(initial_letter(name)),
            })
    }

    // -- actions ------------------------------------------------------------

    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        if self.host.is_none() || self.blocked {
            return;
        }
        let Some(start) = self.core.request_mutation(Mutation::Select(id.clone())) else {
            return;
        };
        self.emit_start(&start, cx);
        let host = self.host.clone().unwrap();
        let tag = start.tag;
        self.run(
            cx,
            async move { api::select_account(host, id).await },
            move |view, result, cx| view.receive(Message::Mutation(tag, result), cx),
        );
    }

    fn refresh_session(&mut self, id: String, cx: &mut Context<Self>) {
        if self.host.is_none() || self.blocked {
            return;
        }
        let Some(start) = self.core.request_mutation(Mutation::Refresh(id.clone())) else {
            return;
        };
        self.emit_start(&start, cx);
        let host = self.host.clone().unwrap();
        let tag = start.tag;
        // The raw session payload is never rendered or logged; only the
        // acknowledged outcome is.
        self.run(
            cx,
            async move { api::refresh_session(host, id).await.map(|_| ()) },
            move |view, result, cx| view.receive(Message::Mutation(tag, result), cx),
        );
    }

    fn sign_in(&mut self, cx: &mut Context<Self>) {
        if self.host.is_none() || self.blocked {
            return;
        }
        let Some(start) = self.core.login_started() else {
            return;
        };
        self.emit_start(&start, cx);
        let host = self.host.clone().unwrap();
        let tag = start.tag;
        let login = api::prepare_login();
        self.login_cancellation = Some((tag, login.cancellation()));
        self.run(
            cx,
            async move { api::begin_prepared_login(host, login).await },
            move |view, result, cx| view.receive(Message::Login(tag, result), cx),
        );
    }

    fn cancel_sign_in(&mut self, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        let Some((_, cancellation)) = self
            .login_cancellation
            .as_ref()
            .filter(|(tag, _)| self.core.login == Some(*tag))
            .cloned()
        else {
            return;
        };
        let Some(guard) = self.core.request_cancel() else {
            return;
        };
        cx.notify();
        self.run(
            cx,
            async move { api::cancel_prepared_login(cancellation).await },
            move |view, result, cx| view.receive(Message::Cancel(guard, result), cx),
        );
    }

    fn open_remove_review(&mut self, id: String, name: String, cx: &mut Context<Self>) {
        if self.host.is_none() || self.blocked {
            return;
        }
        if self.core.open_review(id, name) {
            cx.notify();
        }
    }

    fn dismiss_remove(&mut self, expected: Option<&Review>, cx: &mut Context<Self>) {
        self.core.dismiss_review(expected);
        cx.notify();
    }

    fn confirm_remove(&mut self, expected: Option<&Review>, cx: &mut Context<Self>) {
        if self.host.is_none() || self.blocked {
            return;
        }
        let Some(expected) = expected else {
            return;
        };
        let Some(mutation) = self.core.confirm_review(expected) else {
            cx.notify();
            return;
        };
        let id = mutation.account_id().to_owned();
        let Some(start) = self.core.request_mutation(mutation) else {
            cx.notify();
            return;
        };
        self.emit_start(&start, cx);
        let host = self.host.clone().unwrap();
        let tag = start.tag;
        self.run(
            cx,
            async move { api::remove_account(host, id).await },
            move |view, result, cx| view.receive(Message::Mutation(tag, result), cx),
        );
    }

    // -- message routing ----------------------------------------------------

    fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Read(tag, result) => {
                // A stale result emits nothing; only the acknowledged listing
                // of the current generation is applied.
                let accepted = self.core.load_in_flight == Some(tag)
                    && tag.epoch == self.core.read_tag
                    && !self.core.active()
                    && result.is_ok();
                let pending = self.core.read_completed(tag, result);
                if accepted {
                    if let Some(state) = self.core.accounts.as_ref() {
                        self.avatars.listing(state, self.core.read_tag);
                        self.load_avatar_cache(cx);
                    }
                }
                if pending {
                    self.refresh(cx);
                }
            }
            Message::Login(tag, result) => {
                if let LoginOutcome::Settled { busy, .. } = self.core.login_settled(tag, result) {
                    self.login_cancellation = None;
                    if let Some(busy) = busy {
                        cx.emit(AccountsEvent::BusyChanged(busy));
                    }
                    cx.emit(AccountsEvent::Changed);
                    self.load_after_settle(cx);
                }
            }
            Message::Cancel(guard, result) => {
                // The cancellation acknowledgment never clears busy and never
                // emits Changed; the outstanding login settles both.
                if let Some(message) = self.core.cancel_acknowledged(guard, result) {
                    self.core.error = Some(message);
                }
            }
            Message::Mutation(tag, result) => {
                if let MutationOutcome::Acknowledged { busy, .. } =
                    self.core.mutation_settled(tag, result)
                {
                    if let Some(busy) = busy {
                        cx.emit(AccountsEvent::BusyChanged(busy));
                    }
                    // A failed refresh can persist reauthenticationRequired.
                    cx.emit(AccountsEvent::Changed);
                    self.load_after_settle(cx);
                }
            }
        }
        cx.notify();
    }
}

impl EventEmitter<AccountsEvent> for Accounts {}

impl Render for Accounts {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx).clone();
        let busy = self.is_busy() || self.blocked;
        let no_host = self.host.is_none();
        let auth_busy = self.core.login.is_some()
            || matches!(&self.core.mutation, Some((_, Mutation::Refresh(_))));

        let mut content = div()
            .id("accounts")
            .track_focus(&self.page_focus)
            .flex()
            .flex_col()
            .gap(px(ui::PAGE_GAP))
            .text_color(palette.foreground);

        let heading = ui::page_header(
            &palette,
            "Accounts",
            Some("The Microsoft accounts you play Minecraft with.".into()),
        );

        if no_host {
            content = content.child(heading).child(ui::empty_state(
                &palette,
                "person_off",
                "Accounts unavailable",
                "Accounts are unavailable: this diagnostic window has no launcher host, so no sign-in, credentials or account state are touched.",
            ));
            return div().size_full().child(content);
        }

        // Header action: one primary action, or the cancel affordance while a
        // browser sign-in is running.
        let mut notices: Vec<(String, ui::Tone)> = Vec::new();
        let action: Option<gpui_kit::AnyElement> = if self.core.login.is_some() {
            if self.core.cancel_unavailable {
                notices.push(("Finishing sign-in…".into(), ui::Tone::Neutral));
                None
            } else if self.core.cancel_requested {
                notices.push(("Cancelling sign-in…".into(), ui::Tone::Neutral));
                None
            } else {
                Some(
                    AuroraButton::new("cancel-sign-in", "Cancel sign-in")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_sign_in(cx)))
                        .into_any_element(),
                )
            }
        } else {
            if self.core.cancel.is_some() {
                notices.push(("Finishing sign-in cancellation…".into(), ui::Tone::Neutral));
            }
            Some(
                AuroraButton::new("sign-in", "Add account")
                    .variant(ButtonVariant::Default)
                    .icon("person_add")
                    .disabled(busy || self.core.accounts.is_none() || self.core.cancel.is_some())
                    .on_click(cx.listener(|this, _, _, cx| this.sign_in(cx)))
                    .into_any_element(),
            )
        };
        content = content.child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(16.))
                .child(heading)
                .when_some(action, |row, action| {
                    row.child(div().flex_shrink_0().child(action))
                }),
        );

        if auth_busy {
            if let Some(phase) = &self.core.phase {
                notices.push((phase.to_string(), ui::Tone::Neutral));
            }
        }
        if let Some(notice) = &self.core.notice {
            notices.push((notice.to_string(), ui::Tone::Accent));
        }
        if let Some(error) = &self.core.error {
            notices.push((error.to_string(), ui::Tone::Danger));
        }
        for (text, tone) in notices {
            content = content.child(notice_bar(&palette, text, tone));
        }

        if self.core.accounts.is_none() {
            if let Some(error) = &self.core.load_error {
                content = content.child(
                    ui::card(&palette)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .text_size(px(ui::TEXT_BODY))
                                        .text_color(palette.danger)
                                        .child(error.clone()),
                                )
                                .child(ui::muted(&palette, "Accounts could not be loaded.")),
                        )
                        .child(
                            div().flex().child(
                                AuroraButton::new("accounts-retry", "Retry")
                                    .icon("refresh")
                                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                            ),
                        ),
                );
            } else {
                content = content.child(
                    ui::list_card(&palette).child(
                        div()
                            .px(px(16.))
                            .py(px(20.))
                            .child(ui::muted(&palette, "Loading accounts…")),
                    ),
                );
            }
        }

        if let Some(state) = self.core.accounts.as_ref() {
            let selected_id = state.selected_account_id.clone();
            if state.accounts.is_empty() {
                content = content.child(ui::empty_state(
                    &palette,
                    "account_circle",
                    "No accounts yet",
                    "No accounts yet. Sign in with Microsoft to add one.",
                ));
            } else {
                let mut rows: Vec<&api::AccountSummary> = state.accounts.iter().collect();
                rows.sort_by_key(|account| {
                    selected_id.as_deref() != Some(account.account_id.as_str())
                });
                let count = rows.len();
                let mut list = ui::list_card(&palette).child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .px(px(16.))
                        .py(px(14.))
                        .border_b_1()
                        .border_color(palette.border_subtle)
                        .child(ui::section_header(
                            &palette,
                            "Your accounts",
                            Some("Choose which account launches the game.".into()),
                        ))
                        .child(ui::badge(&palette, count.to_string(), ui::Tone::Neutral)),
                );
                for (index, account) in rows.iter().copied().enumerate() {
                    // Row identity is frozen at render time so a queued event can
                    // never act on a different account.
                    let id = account.account_id.clone();
                    let name = account.minecraft_name.clone();
                    let selected = selected_id.as_deref() == Some(id.as_str());
                    let select_id = id.clone();
                    let review_id = id.clone();
                    let avatar = self.avatar_tile(
                        &palette,
                        &id,
                        &account.minecraft_name,
                        selected,
                        40.,
                        10.,
                        16.,
                    );
                    // One trailing set for every row: a fixed slot holding either
                    // "Use" or the Active badge, then a quiet remove icon.
                    let slot = div()
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .justify_end()
                        .w(px(72.))
                        .child(if selected {
                            ui::badge(&palette, "Active", ui::Tone::Accent).into_any_element()
                        } else {
                            AuroraButton::new(
                                gpui_kit::SharedString::from(format!(
                                    "account-{}",
                                    account.account_id
                                )),
                                "Use",
                            )
                            .variant(ButtonVariant::Soft)
                            .size(ButtonSize::Small)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select(select_id.clone(), cx);
                            }))
                            .into_any_element()
                        });
                    let actions = div()
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap(px(6.))
                        .child(slot)
                        .child(
                            icon_button(
                                &palette,
                                gpui_kit::SharedString::from(format!(
                                    "remove-account-{}",
                                    account.account_id
                                )),
                                "delete",
                                gpui_kit::SharedString::from(format!(
                                    "Remove {}",
                                    account.minecraft_name
                                )),
                            )
                            .disabled(busy)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.open_remove_review(review_id.clone(), name.clone(), cx);
                                },
                            )),
                        );
                    list = list.child(
                        ui::row(&palette, index + 1 == count)
                            .id(gpui_kit::SharedString::from(format!("account-row-{id}")))
                            .when(selected, |row| {
                                // Quiet wash: the Active badge carries the state.
                                row.bg(Hsla {
                                    a: palette.accent_soft.a * 0.5,
                                    ..palette.accent_soft
                                })
                            })
                            .child(avatar)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .gap(px(3.))
                                    .child(
                                        div()
                                            .min_w(px(0.))
                                            .text_size(px(ui::TEXT_BODY + 1.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(palette.foreground)
                                            .text_ellipsis()
                                            .child(account.minecraft_name.clone()),
                                    )
                                    .child(status_line(&palette, &account.status, None)),
                            )
                            .child(actions),
                    );
                }

                // One contextual session action below the account list.
                let mut side = div().flex().flex_col().gap(px(12.)).w_full();
                let active = selected_id
                    .as_deref()
                    .and_then(|id| rows.iter().copied().find(|a| a.account_id == id));
                if let Some(account) = active {
                    let id = account.account_id.clone();
                    let refresh_id = id.clone();
                    let reauth = account.status == "reauthenticationRequired";
                    let explanation = match account.status.as_str() {
                        "signedIn" => "Your session is valid and ready to play.",
                        "reauthenticationRequired" => "Sign in with Microsoft to keep playing.",
                        _ => "This account's session status is unavailable right now.",
                    };
                    let sign_in_blocked =
                        busy || self.core.accounts.is_none() || self.core.cancel.is_some();
                    let mut actions = div().flex().gap(px(8.));
                    if reauth {
                        actions = actions.child(
                            AuroraButton::new("profile-sign-in-again", "Sign in again")
                                .variant(ButtonVariant::Primary)
                                .icon("login")
                                .disabled(sign_in_blocked)
                                .on_click(cx.listener(|this, _, _, cx| this.sign_in(cx))),
                        );
                    }
                    actions = actions.child(
                        AuroraButton::new(
                            gpui_kit::SharedString::from(format!("refresh-session-{id}")),
                            if reauth {
                                "Try saved session"
                            } else {
                                "Refresh session"
                            },
                        )
                        .variant(if reauth {
                            ButtonVariant::Default
                        } else {
                            ButtonVariant::Primary
                        })
                        .icon("sync")
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.refresh_session(refresh_id.clone(), cx);
                        })),
                    );
                    side = side.child(
                        ui::card(&palette).child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(20.))
                                .child(
                                    ui::section_header(
                                        &palette,
                                        "Session",
                                        Some(explanation.into()),
                                    )
                                    .flex_1()
                                    .min_w(px(0.)),
                                )
                                .child(actions.flex_shrink_0()),
                        ),
                    );
                }
                let info = ui::card(&palette).child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .child(ui::icon_tile(&palette, Some("verified_user"), 32., true))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w(px(0.))
                                .gap(px(3.))
                                .child(
                                    div()
                                        .text_size(px(ui::TEXT_BODY))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(palette.foreground)
                                        .child("Secure Microsoft sign-in"),
                                )
                                .child(ui::caption(
                                    &palette,
                                    "Sign-in happens in your browser, and Aurora never sees your password. Sessions are stored in Windows Credential Manager.",
                                )),
                        ),
                );
                content = content
                    .child(div().flex().flex_col().gap(px(16.)).child(list).child(side))
                    .child(info);
            }
        }

        // Removal review: the base dialog owns Escape (vetoed while busy) and
        // the focus trap; Enter never confirms a removal.
        {
            let review = self.core.review.clone();
            let cancel_review = review.clone();
            let confirm_review = review.clone();
            let change_review = review.clone();
            let view = cx.entity();
            let weak = view.downgrade();
            let page_focus = self.page_focus.clone();
            let panel = div()
                .w(px(440.))
                .max_w(px(560.))
                .p(px(24.))
                .bg(palette.raised)
                .border_1()
                .border_color(palette.border_subtle)
                .text_color(palette.foreground)
                .rounded(palette.radius_lg)
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(px(ui::TEXT_SECTION))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Remove account"),
                )
                .child(
                    div()
                        .text_size(px(ui::TEXT_BODY))
                        .text_color(palette.muted_foreground)
                        .child(format!(
                            "Remove {} (account {}) from Aurora Launcher? Its saved sign-in credential will be deleted. All instances are preserved.",
                            review.as_ref().map(|r| r.name.as_str()).unwrap_or(""),
                            review.as_ref().map(|r| r.id.as_str()).unwrap_or("")
                        )),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .pt(px(4.))
                        .child(
                            AuroraButton::new("remove-cancel", "Cancel").disabled(busy).on_click(
                                cx.listener(move |this, _, _, cx| this.dismiss_remove(cancel_review.as_ref(), cx)),
                            ),
                        )
                        .child(
                            AuroraButton::new("remove-confirm", "Remove account")
                                .variant(ButtonVariant::Danger)
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.confirm_remove(confirm_review.as_ref(), cx);
                                })),
                        ),
                );
            // Outside the gapped column so the closed dialog adds no space.
            let dialog = Dialog::new(cx)
                .open(self.core.review_open && self.core.review.is_some())
                .accessibility_label("Remove account")
                .close_on_backdrop_press(false)
                // Enter must never bypass the explicit removal button.
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, app| !view.read(app).core.busy)
                .on_open_change(move |open, _reason, window, app| {
                    if !open {
                        page_focus.focus(window, app);
                    }
                    let _ = weak.update(app, |this, cx| {
                        if !open {
                            this.core.dismiss_review(change_review.as_ref());
                        }
                        cx.notify();
                    });
                })
                .backdrop(div().absolute().size_full().bg(rgba(0x000000a6)))
                .popup(panel);
            return div().size_full().child(content).child(dialog);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests: the pure page core, exercised through its real state methods
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(accounts: &[(&str, &str, &str)], selected: Option<&str>) -> api::AccountsState {
        api::AccountsState {
            accounts: accounts
                .iter()
                .map(|(id, name, status)| api::AccountSummary {
                    account_id: id.to_string(),
                    minecraft_name: name.to_string(),
                    status: status.to_string(),
                })
                .collect(),
            selected_account_id: selected.map(|id| id.to_string()),
        }
    }

    fn ready() -> AccountsCore {
        AccountsCore {
            accounts: Some(fixture(
                &[("a", "Ann", "signedIn"), ("b", "Bee", "signedIn")],
                Some("a"),
            )),
            ..AccountsCore::default()
        }
    }

    fn head(bytes: Vec<u8>) -> api::HeadAvatar {
        api::HeadAvatar {
            rgba: bytes,
            skin_rgba: vec![],
            skin_height: 0,
            model: String::new(),
        }
    }

    #[test]
    fn avatar_listing_rejects_stale_cache_after_removal_or_status_change() {
        let mut avatars = Avatars::default();
        avatars.listing(&fixture(&[("a", "Ann", "signedIn")], None), 0);
        let old = avatars.generation;
        avatars.listing(
            &fixture(&[("a", "Ann", "reauthenticationRequired")], None),
            0,
        );
        avatars.accept_cache(
            old,
            0,
            vec![api::CachedAvatar {
                account_id: "a".into(),
                avatar: head(vec![1, 2, 3, 4]),
            }],
        );
        assert!(avatars.images.is_empty());
        avatars.listing(&fixture(&[], None), 0);
        avatars.accept_cache(
            old,
            0,
            vec![api::CachedAvatar {
                account_id: "a".into(),
                avatar: head(vec![1, 2, 3, 4]),
            }],
        );
        assert!(avatars.images.is_empty());
    }

    #[test]
    fn avatar_refresh_deduplicates_inflight_and_retries_only_after_new_listing() {
        let mut avatars = Avatars::default();
        let state = fixture(&[("a", "Ann", "signedIn")], None);
        avatars.listing(&state, 0);
        avatars.cache_ready = true;
        assert_eq!(avatars.missing(0), vec!["a"]);
        avatars.attempted.insert("a".into());
        avatars.pending.insert("a".into(), avatars.generation);
        assert!(avatars.missing(0).is_empty());
        avatars.listing(&state, 0);
        avatars.cache_ready = true;
        assert!(avatars.missing(0).is_empty());
        avatars.pending.remove("a");
        assert_eq!(avatars.missing(0), vec!["a"]);
        avatars.attempted.insert("a".into());
        assert!(avatars.missing(0).is_empty());
    }

    #[test]
    fn cached_pixels_do_not_replace_existing_head_and_malformed_heads_fall_back() {
        let mut avatars = Avatars::default();
        avatars.listing(&fixture(&[("a", "Ann", "signedIn")], None), 0);
        avatars.accept_fetched(avatars.generation, 0, "a", head(vec![1, 2, 3, 4]));
        let fetched = avatars.images["a"].clone();
        avatars.accept_cache(
            avatars.generation,
            0,
            vec![api::CachedAvatar {
                account_id: "a".into(),
                avatar: head(vec![5, 6, 7, 8]),
            }],
        );
        assert!(Arc::ptr_eq(&fetched, &avatars.images["a"]));
        assert!(Avatars::image(head(vec![])).is_none());
        assert!(Avatars::image(head(vec![0; 12])).is_none());
        assert!(Avatars::image(head(vec![0; 1025 * 1025 * 4])).is_none());
    }

    #[test]
    fn a_fresh_listing_can_replace_an_old_head_with_newer_cached_pixels() {
        let mut avatars = Avatars::default();
        let state = fixture(&[("a", "Ann", "signedIn")], None);
        avatars.listing(&state, 0);
        let old = avatars.generation;
        avatars.accept_fetched(old, 0, "a", head(vec![1, 2, 3, 4]));
        avatars.listing(&state, 0);
        avatars.accept_cache(
            avatars.generation,
            0,
            vec![api::CachedAvatar {
                account_id: "a".into(),
                avatar: head(vec![5, 6, 7, 8]),
            }],
        );
        let fresh = avatars.images["a"].clone();
        assert_eq!(fresh.as_bytes(0).unwrap(), &[7, 6, 5, 8]);
        avatars.accept_fetched(old, 0, "a", head(vec![1, 2, 3, 4]));
        assert!(Arc::ptr_eq(&fresh, &avatars.images["a"]));
    }

    #[test]
    fn read_after_mutation_cannot_restore_superseded_account_state() {
        let mut core = AccountsCore::default();
        core.accounts = Some(fixture(
            &[("a", "Ann", "signedIn"), ("b", "Bee", "signedIn")],
            Some("a"),
        ));
        let stale_tag = core.request_load().expect("first listing read");
        let start = core
            .request_mutation(Mutation::Select("b".into()))
            .expect("mutation accepted");
        assert!(matches!(start.busy, Some(true)));

        // The pre-mutation read arrives, claiming the old selection.
        let applied = core.read_completed(
            stale_tag,
            Ok(fixture(&[("a", "Ann", "signedIn")], Some("a"))),
        );
        assert!(!applied);
        // Rejected: the stale snapshot was never applied.
        assert_eq!(
            core.accounts
                .as_ref()
                .unwrap()
                .selected_account_id
                .as_deref(),
            Some("a")
        );

        let outcome = core.mutation_settled(start.tag, Ok(()));
        assert!(matches!(
            outcome,
            MutationOutcome::Acknowledged { ok: true, .. }
        ));
        // After settlement a fresh read of the current generation dispatches.
        let fresh = core.request_load().expect("post-mutation read");
        assert_ne!(fresh.epoch, stale_tag.epoch);
        assert_ne!(fresh.serial, stale_tag.serial);
        assert!(core.accounts.is_none());
    }

    #[test]
    fn failed_read_never_fabricates_a_snapshot_and_retry_stays_reachable() {
        let mut core = AccountsCore::default();
        core.accounts = Some(fixture(&[("a", "Ann", "signedIn")], None));
        let tag = core.request_load().expect("read");
        core.read_completed(tag, Err(api::ApiError::new("fixture", "Listing failed.")));
        assert!(core.accounts.is_none());
        assert_eq!(core.load_error.as_deref(), Some("Listing failed."));
        let retry = core.request_load().expect("retry read");
        assert_ne!(retry.serial, tag.serial);
        assert_eq!(retry.epoch, tag.epoch);
    }

    #[test]
    fn duplicate_terminal_reply_is_ignored() {
        let mut core = ready();
        let start = core
            .request_mutation(Mutation::Select("a".into()))
            .expect("mutation accepted");
        assert!(matches!(
            core.mutation_settled(start.tag, Ok(())),
            MutationOutcome::Acknowledged { ok: true, .. }
        ));
        assert!(matches!(
            core.mutation_settled(start.tag, Ok(())),
            MutationOutcome::Ignored
        ));
        assert!(matches!(
            core.mutation_settled(start.tag, Err(api::ApiError::new("fixture", "late"))),
            MutationOutcome::Ignored
        ));
    }

    #[test]
    fn cancelled_login_clears_busy_only_when_it_settles() {
        let mut core = ready();
        let start = core.login_started().expect("login accepted");
        assert!(matches!(start.busy, Some(true)));
        let cancel = core.request_cancel().expect("cancellation accepted");
        assert!(core.cancel_requested);

        // The cancellation acknowledgment alone never clears busy.
        assert_eq!(
            core.cancel_acknowledged(cancel, Ok(api::LoginCancelOutcome::Requested)),
            None
        );
        assert!(core.login.is_some());

        // Only the settled login clears busy.
        assert!(matches!(
            core.login_settled(
                start.tag,
                Err(api::ApiError::new(
                    "auth_cancelled",
                    "Sign-in was cancelled."
                ))
            ),
            LoginOutcome::Settled {
                ok: false,
                cancelled: true,
                busy: Some(false)
            }
        ));
        assert!(!core.busy);
        assert!(core.error.is_none());

        // A duplicate terminal reply is ignored.
        assert!(matches!(
            core.login_settled(
                start.tag,
                Ok(api::AccountSummary {
                    account_id: "new".into(),
                    minecraft_name: "New".into(),
                    status: "signedIn".into(),
                })
            ),
            LoginOutcome::Ignored
        ));
    }

    #[test]
    fn failed_login_and_failed_mutation_report_and_clear_busy() {
        let mut core = ready();
        let login = core.login_started().expect("login accepted");
        assert!(matches!(
            core.login_settled(
                login.tag,
                Err(api::ApiError::new("fixture", "Sign-in failed."))
            ),
            LoginOutcome::Settled { ok: false, .. }
        ));
        assert_eq!(core.error.as_deref(), Some("Sign-in failed."));
        assert!(!core.busy);

        assert!(core.accounts.is_none());
        let read = core.request_load().unwrap();
        core.read_completed(read, Ok(fixture(&[("a", "Ann", "signedIn")], Some("a"))));
        let mutation = core
            .request_mutation(Mutation::Refresh("a".into()))
            .expect("mutation accepted");
        assert!(matches!(
            core.mutation_settled(
                mutation.tag,
                Err(api::ApiError::new("fixture", "Refresh failed."))
            ),
            MutationOutcome::Acknowledged { ok: false, .. }
        ));
        assert_eq!(core.error.as_deref(), Some("Refresh failed."));
        assert!(!core.busy);
    }

    #[test]
    fn stale_review_is_rejected_and_a_matching_review_freezes_identity() {
        let mut core = AccountsCore::default();
        core.accounts = Some(fixture(&[("x", "Player", "signedIn")], None));
        assert!(core.open_review("x".into(), "Player".into()));

        // The listing changed underneath the open review.
        core.accounts = Some(fixture(&[("y", "Other", "signedIn")], None));
        let review = core.review.clone().unwrap();
        assert!(core.confirm_review(&review).is_none());
        assert!(!core.review_open);
        assert_eq!(core.error.as_deref(), Some(STALE_REVIEW));

        // A review matching the current listing confirms the frozen id.
        assert!(core.open_review("y".into(), "Other".into()));
        let review = core.review.clone().unwrap();
        assert_eq!(
            core.confirm_review(&review),
            Some(Mutation::Remove("y".into()))
        );

        // No listing at all (failed read) can never open a review.
        core.accounts = None;
        assert!(!core.open_review("y".into(), "Other".into()));
    }

    #[test]
    fn single_mutation_vetoes_a_second_operation() {
        let mut core = ready();
        assert!(
            core.request_mutation(Mutation::Select("a".into()))
                .is_some()
        );
        assert!(
            core.request_mutation(Mutation::Refresh("b".into()))
                .is_none()
        );
        assert!(core.login_started().is_none());
        // Same for an in-flight login.
        let mut core = ready();
        assert!(core.login_started().is_some());
        assert!(
            core.request_mutation(Mutation::Select("a".into()))
                .is_none()
        );
    }

    #[test]
    fn auth_progress_is_live_only_during_auth_work() {
        let mut core = ready();
        assert!(!core.auth_progress("waitingForMicrosoft"));

        let start = core.login_started().expect("login accepted");
        assert!(core.auth_progress("exchangingMicrosoftToken"));
        assert_eq!(
            core.phase.as_deref(),
            Some("Confirming your Microsoft sign-in…")
        );
        assert!(core.auth_progress("unknownPhase"));
        assert_eq!(core.phase.as_deref(), Some("Signing in…"));
        let _ = start;

        // Session refresh shows the restoring phase.
        let mut refresh = ready();
        let start = refresh
            .request_mutation(Mutation::Refresh("a".into()))
            .expect("mutation accepted");
        assert_eq!(refresh.phase.as_deref(), Some("Restoring your session…"));
        assert!(refresh.auth_progress("fetchingProfile"));
        let _ = start;
    }

    #[test]
    fn duplicate_read_cannot_clear_a_newer_read_or_apply_old_state() {
        let mut core = ready();
        let old = core.request_load().unwrap();
        assert!(!core.read_completed(old, Ok(fixture(&[], None))));
        let current = core.request_load().unwrap();
        assert!(!core.read_completed(old, Ok(fixture(&[("old", "Old", "signedIn")], Some("old")))));
        assert_eq!(core.load_in_flight, Some(current));
        assert!(core.accounts.as_ref().unwrap().accounts.is_empty());
        assert!(!core.read_completed(
            current,
            Ok(fixture(&[("new", "New", "signedIn")], Some("new")))
        ));
        assert_eq!(
            core.accounts
                .as_ref()
                .unwrap()
                .selected_account_id
                .as_deref(),
            Some("new")
        );
    }

    #[test]
    fn reads_requested_during_mutation_wait_for_settlement_and_failed_refresh_invalidates() {
        let mut core = ready();
        let mutation = core
            .request_mutation(Mutation::Refresh("a".into()))
            .unwrap();
        assert!(core.request_load().is_none());
        assert!(core.load_pending);
        core.mutation_settled(
            mutation.tag,
            Err(api::ApiError::new("fixture", "Sign in again.")),
        );
        assert!(core.accounts.is_none());
        assert!(
            core.request_mutation(Mutation::Select("a".into()))
                .is_none()
        );
        assert!(core.login_started().is_none());
        let read = core.request_load().unwrap();
        core.read_completed(
            read,
            Ok(fixture(
                &[("a", "Ann", "reauthenticationRequired")],
                Some("a"),
            )),
        );
        assert_eq!(
            core.accounts.as_ref().unwrap().accounts[0].status,
            "reauthenticationRequired"
        );
    }

    #[test]
    fn old_cancellation_reply_cannot_modify_a_retry_or_a_new_login() {
        let mut core = ready();
        let login = core.login_started().unwrap();
        let old = core.request_cancel().unwrap();
        assert!(
            core.cancel_acknowledged(old, Err(api::ApiError::new("fixture", "Retry cancel.")))
                .is_some()
        );
        let retry = core.request_cancel().unwrap();
        assert_ne!(old, retry);
        assert!(
            core.cancel_acknowledged(old, Ok(api::LoginCancelOutcome::Requested))
                .is_none()
        );
        assert_eq!(core.cancel, Some(retry));
        core.login_settled(
            login.tag,
            Err(api::ApiError::new("auth_cancelled", "Cancelled.")),
        );
        let read = core.request_load().unwrap();
        core.read_completed(read, Ok(fixture(&[], None)));
        assert!(
            core.login_started().is_none(),
            "an outstanding cancellation must settle before a new sign-in starts"
        );
        assert!(
            core.cancel_acknowledged(retry, Ok(api::LoginCancelOutcome::Requested))
                .is_none()
        );
        let next_login = core.login_started().unwrap();
        let next_cancel = core.request_cancel().unwrap();
        assert!(
            core.cancel_acknowledged(retry, Err(api::ApiError::new("fixture", "Old failure.")))
                .is_none()
        );
        assert_eq!(core.login, Some(next_login.tag));
        assert_eq!(core.cancel, Some(next_cancel));
        assert!(core.cancel_requested && core.busy);
    }

    #[test]
    fn queued_review_actions_cannot_confirm_or_dismiss_a_different_account() {
        let mut core = ready();
        assert!(core.open_review("a".into(), "Ann".into()));
        let old = core.review.clone().unwrap();
        core.dismiss_review(Some(&old));
        assert!(core.open_review("b".into(), "Bee".into()));
        assert!(core.confirm_review(&old).is_none());
        core.dismiss_review(Some(&old));
        let current = core.review.clone().unwrap();
        assert_eq!(current.id, "b");
        assert!(core.review_open);
        assert_eq!(
            core.confirm_review(&current),
            Some(Mutation::Remove("b".into()))
        );
        assert!(core.confirm_review(&current).is_none());
        assert!(AccountsCore::default().login_started().is_none());
    }

    #[test]
    fn cancellation_after_persistence_boundary_reports_finishing_and_awaits_login() {
        let mut core = ready();
        let login = core.login_started().unwrap();
        let cancel = core.request_cancel().unwrap();
        assert!(
            core.cancel_acknowledged(cancel, Ok(api::LoginCancelOutcome::TooLate))
                .is_none()
        );
        assert!(core.busy && core.cancel_unavailable);
        assert!(!core.cancel_requested);
        assert!(core.notice.as_ref().unwrap().contains("finishing"));
        assert!(core.request_cancel().is_none());
        assert_eq!(core.login, Some(login.tag));
        assert!(matches!(
            core.login_settled(
                login.tag,
                Ok(api::AccountSummary {
                    account_id: "a".into(),
                    minecraft_name: "Ann".into(),
                    status: "signedIn".into(),
                })
            ),
            LoginOutcome::Settled {
                ok: true,
                busy: Some(false),
                ..
            }
        ));
        assert!(!core.cancel_unavailable && !core.busy);
        assert_eq!(core.notice.as_deref(), Some("Signed in as Ann."));
    }
}
