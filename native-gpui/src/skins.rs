//! GPUI skins page: the non-raster workflows around the local skin library
//! and the selected account's cosmetics. The preserved Slint controller
//! (`native/src/skins.rs`) is the behavior authority and the typed API in
//! `native/src/api.rs` stays the only backend boundary; the core alone
//! validates images, credentials and mutations.
//!
//! Contract kept here:
//! - Account identity is supplied by the root through [`Skins::set_accounts`]
//!   (acknowledged snapshot); there is no second account selector. The local
//!   library works signed out; every remote call requires the exact selected
//!   `signedIn` identity, frozen at write start and re-checked at settlement.
//! - One write at a time, `blocked` is checked before starting work, and all
//!   API/picker work runs through `backend::guarded` plus an awaited oneshot —
//!   never polling, timers, or work started in `render`. With no host the page
//!   performs zero API calls and opens no pickers.
//! - Read generations (library serial, cosmetics epoch, write generation)
//!   reject stale and duplicate replies, so an old read can never overwrite an
//!   acknowledged write, and a removed or replaced library entry never returns
//!   through a late thumbnail or list reply.
//! - An account/status epoch change synchronously invalidates cosmetics,
//!   reviews and the inspected preview, and rejects remote replies; a pending
//!   write still settles terminally — busy clears exactly once — but its
//!   result is only applied when the exact identity still matches.
//! - A deliberately inspected library skin or cape (local revision) survives a
//!   cosmetics refresh that was in flight when the inspection happened.
//! - Apply and removal are reviewed with the exact frozen identity, preset
//!   name and classic/slim model; Enter never confirms, and Escape or the
//!   backdrop can only veto while this page's own write is committed. Focus is
//!   restored to the page when a dialog closes.
//! - `SkinsEvent::Changed` is emitted only for acknowledged persisted
//!   mutations (or a documented drift side effect that requires a reload);
//!   `PreviewChanged` is emitted only when the preview appearance actually
//!   changed.
//!
//! Honest parity limits, kept deliberate:
//! - No CPU rasterization or pose controls here: the parent owns a separate
//!   preview entity and consumes [`SkinsEvent::PreviewChanged`] plus
//!   [`Skins::preview_appearance`]. This page only selects what is previewed.
//! - Cape rows show the account's cape texture (the service-provided pixels),
//!   not a posed render.

#![allow(dead_code)]

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use aurora_launcher_lib::host::Host;
use gpui_kit::base::Dialog;
use gpui_kit::base::input::InputState;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext, Bounds, Context, Entity, EventEmitter, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, Render, RenderImage, StatefulInteractiveElement as _,
    Styled, Window, div, img, px, rgba,
};

use crate::api::{self, ApiResult};
use crate::backend::guarded;
use crate::controls::{
    AuroraButton, AuroraInput, AuroraSelect, AuroraToggle, ButtonSize, ButtonVariant,
};
use crate::{file_picker, images};
use crate::{theme, ui};

/// Events for the integrating parent view.
#[derive(Debug)]
pub enum SkinsEvent {
    OpenAccounts,
    /// A mutation settled and may have changed persisted state (the local
    /// library or account cosmetics). Never a read, preview selection or
    /// cancelled picker alone.
    Changed,
    /// The page's busy state changed. Each transition is reported once.
    BusyChanged(bool),
    /// The selected preview appearance actually changed (skin, cape, or the
    /// rear-view request). The parent updates its independent preview entity.
    PreviewChanged,
}

// ---------------------------------------------------------------------------
// Pure page core (testable without a host or window)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    id: String,
    name: String,
    status: String,
}

fn identity(state: &api::AccountsState) -> Option<Identity> {
    let id = state.selected_account_id.as_ref()?;
    let account = state.accounts.iter().find(|a| &a.account_id == id)?;
    Some(Identity {
        id: account.account_id.clone(),
        name: account.minecraft_name.clone(),
        status: account.status.clone(),
    })
}

/// One acknowledged write, frozen at start.
#[derive(Clone, Debug, PartialEq)]
enum Write {
    Import {
        name: String,
        model: String,
    },
    SaveCurrent {
        account: String,
    },
    Favorite {
        preset: String,
        value: bool,
    },
    Rename {
        preset: String,
        name: String,
    },
    Remove {
        preset: String,
    },
    Apply {
        account: String,
        preset: String,
        model: String,
    },
    SelectCape {
        account: String,
        cape: String,
    },
    DisableCape {
        account: String,
    },
}

impl Write {
    fn remote_account(&self) -> Option<&str> {
        match self {
            Write::SaveCurrent { account }
            | Write::Apply { account, .. }
            | Write::SelectCape { account, .. }
            | Write::DisableCape { account } => Some(account),
            _ => None,
        }
    }
}

/// The terminal reply of an acknowledged write.
#[derive(Debug)]
enum WriteResult {
    Failure(api::ApiError),
    /// The user cancelled the owned picker; a quiet terminal settlement.
    Cancelled,
    /// Local write acknowledged with the fresh authoritative list.
    Library(Vec<api::SkinPreset>),
    /// Local write acknowledged; the list is re-read.
    LibraryReload,
    /// Remote cosmetics mutation acknowledged with the new snapshot.
    Cosmetics(api::Cosmetics),
}

/// What the view must do after a write settles. `changed` means persisted
/// state may have changed (emit `Changed`); a drift success also sets
/// `changed` because the server-side mutation may have persisted under the
/// previous identity and the parent must reload.
#[derive(Debug, Default, PartialEq)]
struct WriteOutcome {
    ok: bool,
    changed: bool,
    preview_changed: bool,
    reload_library: bool,
    reload_cosmetics: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LibraryRead {
    serial: u64,
    generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CosmeticsRead {
    generation: u64,
    epoch: u64,
    revision: u64,
    account: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ApplyReview {
    account: String,
    account_name: String,
    preset: String,
    name: String,
    model: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RemoveReview {
    preset: String,
    name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum SkinChoice {
    #[default]
    Current,
    Preset(String),
}

#[derive(Debug, Default, PartialEq)]
struct RefreshReads {
    library: Option<LibraryRead>,
    cosmetics: Option<CosmeticsRead>,
    preview_changed: bool,
}

#[derive(Debug, Default, PartialEq)]
struct ReadSettlement {
    redispatch: bool,
    applied: bool,
    preview_changed: bool,
}

#[derive(Debug, Default, PartialEq)]
struct PreviewArrival {
    applied: bool,
    preview_changed: bool,
}

#[derive(Default)]
struct Core {
    account: Option<Identity>,
    /// Bumped by every account/status change and every refresh; rejects stale
    /// remote replies.
    epoch: u64,
    /// Bumped by every write start and settlement; rejects stale reads.
    generation: u64,
    // Library (local, works signed out)
    library: Option<Vec<api::SkinPreset>>,
    library_error: Option<String>,
    library_read: Option<LibraryRead>,
    library_pending: bool,
    /// Bumped on refresh and on each accepted list change; guards reads and
    /// thumbnails so removed or replaced entries cannot return.
    library_serial: u64,
    favorites_only: bool,
    // Cosmetics (remote, exact signed-in identity)
    cosmetics: Option<api::Cosmetics>,
    remote_error: Option<String>,
    cosmetics_read: Option<CosmeticsRead>,
    cosmetics_pending: bool,
    // One write at a time
    write: Option<(u64, Write)>,
    write_serial: u64,
    error: Option<String>,
    notice: Option<String>,
    // Reviewed decisions
    apply_review: Option<ApplyReview>,
    apply_open: bool,
    remove_review: Option<RemoveReview>,
    remove_open: bool,
    rename_review: Option<RemoveReview>,
    rename_open: bool,
    // Preview selection (local inspection only)
    preview_serial: u64,
    local_revision: u64,
    choice: SkinChoice,
    skin: Option<Arc<api::HeadAvatar>>,
    cape: Option<Arc<api::Texture>>,
    cape_choice: Option<String>,
    rear: bool,
}

fn valid_model(model: &str) -> bool {
    matches!(model, "classic" | "slim")
}

fn model_label(model: &str) -> &'static str {
    if model == "slim" { "Slim" } else { "Classic" }
}

/// A form field: a small label above its control.
fn field(t: &theme::AuroraTheme, label: &'static str) -> gpui_kit::Div {
    div().flex().flex_col().gap(px(6.)).child(
        div()
            .text_size(px(ui::TEXT_CAPTION))
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(t.muted_foreground)
            .child(label),
    )
}

/// An inline status strip for notices and errors.
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
        .bg(gpui_kit::Hsla { a: 0.10, ..color })
        .border_1()
        .border_color(gpui_kit::Hsla { a: 0.22, ..color })
        .text_size(px(ui::TEXT_BODY))
        .text_color(color)
        .child(crate::symbols::icon(Some(icon), px(16.), color))
        .child(div().flex_1().child(text.into()))
}

/// A quiet icon-only row action. The label is the accessible name.
fn icon_button(
    t: &theme::AuroraTheme,
    id: impl Into<gpui_kit::ElementId>,
    icon: &'static str,
    label: &'static str,
    color: gpui_kit::Hsla,
    danger: bool,
) -> gpui_kit::base::Button {
    let clear = gpui_kit::transparent_black();
    let ring = t.ring;
    let hover = if danger {
        gpui_kit::Hsla {
            a: 0.16,
            ..t.danger
        }
    } else {
        t.control_hover
    };
    gpui_kit::base::Button::new(id)
        .aria_label(label)
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
        .child(crate::symbols::icon(Some(icon), px(16.), color))
}

/// Value equality: repeated selections of the same decoded artwork must not
/// report an appearance change just because a fresh `Arc` was allocated.
fn same_skin(a: &Option<Arc<api::HeadAvatar>>, b: &Option<Arc<api::HeadAvatar>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            x.rgba == y.rgba
                && x.skin_rgba == y.skin_rgba
                && x.skin_height == y.skin_height
                && x.model == y.model
        }
        _ => false,
    }
}

fn same_cape(a: &Option<Arc<api::Texture>>, b: &Option<Arc<api::Texture>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x.width == y.width && x.height == y.height && x.rgba == y.rgba,
        _ => false,
    }
}

impl Core {
    fn busy(&self) -> bool {
        self.write.is_some()
    }

    fn signed_in(&self) -> Option<&Identity> {
        self.account.as_ref().filter(|a| a.status == "signedIn")
    }

    /// Shared by the visible Apply control and its activation guard.
    fn apply_unavailable(&self, blocked: bool) -> Option<&'static str> {
        if self.busy() || blocked {
            Some("Wait for the current task to finish before applying a skin.")
        } else if self.account.is_none() {
            Some("Sign in to apply a saved skin to your Minecraft account.")
        } else if self.signed_in().is_none() {
            Some("Sign in again in Manage accounts to apply skins.")
        } else {
            None
        }
    }

    fn applied(&self, preset: &api::SkinPreset) -> bool {
        self.signed_in().is_some()
            && self.cosmetics.as_ref().is_some_and(|cosmetics| {
                cosmetics.has_current_skin
                    && cosmetics.current_skin.as_ref().is_some_and(|skin| {
                        !preset.sha256.is_empty()
                            && preset.sha256 == skin.sha256
                            && preset.model
                                == cosmetics
                                    .current_skin_model
                                    .as_deref()
                                    .unwrap_or(&skin.model)
                    })
            })
    }

    fn apply_label(&self) -> String {
        self.signed_in()
            .map(|account| format!("Apply to {}", account.name))
            .unwrap_or_else(|| "Apply".into())
    }

    fn preset(&self, id: &str) -> Option<&api::SkinPreset> {
        self.library.as_ref()?.iter().find(|p| p.id == id)
    }

    fn cape(&self, id: &str) -> Option<&api::Cape> {
        self.cosmetics.as_ref()?.capes.iter().find(|c| c.id == id)
    }

    fn worn_cape(&self) -> Option<&api::Cape> {
        self.cosmetics.as_ref()?.capes.iter().find(|c| c.selected)
    }

    /// The visible rows after the local favorites filter.
    fn filtered_library(&self) -> Vec<&api::SkinPreset> {
        self.library
            .as_ref()
            .map(|list| {
                list.iter()
                    .filter(|p| !self.favorites_only || p.favorite)
                    .collect()
            })
            .unwrap_or_default()
    }

    // -- account ---------------------------------------------------------

    /// Installs the acknowledged account snapshot. A change invalidates
    /// cosmetics, reviews and the inspected preview, and rejects remote
    /// replies through the epoch. Pending writes settle terminally.
    fn set_account(&mut self, account: Option<Identity>) -> bool {
        if self.account == account {
            return false;
        }
        self.account = account;
        self.epoch = self.epoch.wrapping_add(1);
        self.cosmetics = None;
        self.remote_error = None;
        self.apply_review = None;
        self.apply_open = false;
        self.remove_review = None;
        self.remove_open = false;
        self.rename_review = None;
        self.rename_open = false;
        self.error = None;
        self.notice = None;
        self.reset_preview(None, None);
        true
    }

    // -- preview ---------------------------------------------------------

    fn set_preview(
        &mut self,
        skin: Option<Arc<api::HeadAvatar>>,
        cape: Option<Arc<api::Texture>>,
        rear: bool,
    ) -> bool {
        let changed =
            !same_skin(&self.skin, &skin) || !same_cape(&self.cape, &cape) || self.rear != rear;
        self.skin = skin;
        self.cape = cape;
        self.rear = rear;
        changed
    }

    fn reset_preview(
        &mut self,
        skin: Option<Arc<api::HeadAvatar>>,
        cape: Option<Arc<api::Texture>>,
    ) -> bool {
        self.preview_serial = self.preview_serial.wrapping_add(1);
        self.local_revision = self.local_revision.wrapping_add(1);
        self.choice = SkinChoice::Current;
        self.cape_choice = None;
        self.set_preview(skin, cape, false)
    }

    /// Selects the account's current appearance for preview. Purely local.
    fn preview_current(&mut self) -> bool {
        self.preview_serial = self.preview_serial.wrapping_add(1);
        self.local_revision = self.local_revision.wrapping_add(1);
        self.choice = SkinChoice::Current;
        let worn = self.worn_cape();
        let worn_id = worn.map(|cape| cape.id.clone());
        let skin = self
            .cosmetics
            .as_ref()
            .and_then(|cosmetics| cosmetics.current_skin.clone())
            .map(Arc::new);
        let cape = worn.and_then(|cape| cape.preview.clone()).map(Arc::new);
        self.cape_choice = worn_id;
        self.set_preview(skin, cape, false)
    }

    /// Selects a listed library skin for preview; the avatar loads
    /// asynchronously under the returned serial. Inspecting a skin keeps the
    /// inspected cape but returns to the front view.
    fn preview_preset(&mut self, id: &str) -> Option<(u64, String, bool)> {
        self.preset(id)?;
        self.preview_serial = self.preview_serial.wrapping_add(1);
        self.local_revision = self.local_revision.wrapping_add(1);
        let changed_rear = self.rear;
        self.choice = SkinChoice::Preset(id.to_owned());
        self.rear = false;
        Some((self.preview_serial, id.to_owned(), changed_rear))
    }

    /// A preset avatar load settled; only the newest selection applies.
    fn preview_arrived(
        &mut self,
        serial: u64,
        id: &str,
        result: ApiResult<api::HeadAvatar>,
    ) -> PreviewArrival {
        if self.preview_serial != serial || self.choice != SkinChoice::Preset(id.to_owned()) {
            return PreviewArrival::default();
        }
        match result {
            Ok(avatar) => {
                let changed =
                    self.set_preview(Some(Arc::new(avatar)), self.cape.clone(), self.rear);
                PreviewArrival {
                    applied: true,
                    preview_changed: changed,
                }
            }
            Err(error) => {
                self.error = Some(error.message);
                PreviewArrival {
                    applied: true,
                    preview_changed: false,
                }
            }
        }
    }

    /// Inspects an owned cape from the rear without changing the account.
    fn inspect_cape(&mut self, id: &str) -> Option<bool> {
        let texture = self.cape(id)?.preview.clone().map(Arc::new);
        self.local_revision = self.local_revision.wrapping_add(1);
        let changed = !same_cape(&self.cape, &texture) || !self.rear;
        self.cape_choice = Some(id.to_owned());
        self.cape = texture;
        self.rear = true;
        Some(changed)
    }

    /// The selected preview appearance for the parent's preview entity.
    fn preview_appearance(
        &self,
    ) -> (
        Option<Arc<api::HeadAvatar>>,
        Option<Arc<api::Texture>>,
        bool,
    ) {
        (self.skin.clone(), self.cape.clone(), self.rear)
    }

    // -- reads ------------------------------------------------------------

    /// Begins an explicit refresh: preview and remote state reset
    /// synchronously (a deliberate inspection during the flight is still
    /// preserved), stale reads are rejected, fresh reads are registered.
    fn begin_refresh(&mut self) -> RefreshReads {
        self.epoch = self.epoch.wrapping_add(1);
        self.library_serial = self.library_serial.wrapping_add(1);
        self.library_error = None;
        self.remote_error = None;
        self.cosmetics = None;
        self.error = None;
        self.notice = None;
        let preview_changed = self.reset_preview(None, None);
        RefreshReads {
            library: self.request_library(),
            cosmetics: self.request_cosmetics(),
            preview_changed,
        }
    }

    fn request_library(&mut self) -> Option<LibraryRead> {
        if self.library_read.is_some() || self.write.is_some() {
            self.library_pending = true;
            return None;
        }
        let read = LibraryRead {
            serial: self.library_serial,
            generation: self.generation,
        };
        self.library_pending = false;
        self.library_read = Some(read);
        Some(read)
    }

    /// A library read settled. A stale generation, serial or duplicate reply
    /// is never applied, so an old read cannot overwrite an acknowledged
    /// write or a newer list.
    fn library_settled(
        &mut self,
        read: LibraryRead,
        result: ApiResult<Vec<api::SkinPreset>>,
    ) -> ReadSettlement {
        if self.library_read != Some(read) {
            return ReadSettlement::default();
        }
        self.library_read = None;
        let redispatch = self.library_pending;
        if read.serial != self.library_serial || read.generation != self.generation || self.busy() {
            return ReadSettlement {
                redispatch,
                ..ReadSettlement::default()
            };
        }
        match result {
            Ok(list) => {
                self.library = Some(list);
                self.library_error = None;
                self.library_serial = self.library_serial.wrapping_add(1);
            }
            // A failed read never fabricates a writable snapshot.
            Err(error) => {
                self.library = None;
                self.library_error = Some(error.message);
            }
        }
        ReadSettlement {
            redispatch,
            applied: true,
            preview_changed: false,
        }
    }

    fn request_cosmetics(&mut self) -> Option<CosmeticsRead> {
        let account = self.signed_in()?.id.clone();
        if self.cosmetics_read.is_some() || self.write.is_some() {
            self.cosmetics_pending = true;
            return None;
        }
        let read = CosmeticsRead {
            generation: self.generation,
            epoch: self.epoch,
            revision: self.local_revision,
            account,
        };
        self.cosmetics_pending = false;
        self.cosmetics_read = Some(read.clone());
        Some(read)
    }

    /// Drain only explicitly requested reloads. A reply for one domain must
    /// never enqueue a new read in the other domain merely because it is busy.
    fn pending_reads(&mut self) -> RefreshReads {
        let mut reads = RefreshReads::default();
        if self.busy() {
            return reads;
        }
        if self.library_pending && self.library_read.is_none() {
            self.library_pending = false;
            reads.library = self.request_library();
        }
        if self.cosmetics_pending && self.cosmetics_read.is_none() {
            self.cosmetics_pending = false;
            reads.cosmetics = self.request_cosmetics();
        }
        reads
    }

    /// A cosmetics read settled. The account epoch, write generation and a
    /// deliberate local inspection during the flight decide whether the
    /// snapshot applies and whether the inspected preview is preserved.
    fn cosmetics_settled(
        &mut self,
        read: CosmeticsRead,
        result: ApiResult<api::Cosmetics>,
    ) -> ReadSettlement {
        if self.cosmetics_read.as_ref() != Some(&read) {
            return ReadSettlement::default();
        }
        self.cosmetics_read = None;
        let redispatch = self.cosmetics_pending;
        let identity_current = self.signed_in().is_some_and(|i| i.id == read.account);
        if read.epoch != self.epoch
            || read.generation != self.generation
            || !identity_current
            || self.busy()
        {
            return ReadSettlement {
                redispatch,
                ..ReadSettlement::default()
            };
        }
        match result {
            Ok(cosmetics) => {
                let preserve = read.revision != self.local_revision;
                let preview_changed = self.apply_cosmetics_snapshot(cosmetics, preserve);
                ReadSettlement {
                    redispatch,
                    applied: true,
                    preview_changed,
                }
            }
            Err(error) => {
                self.cosmetics = None;
                self.remote_error = Some(error.message);
                ReadSettlement {
                    redispatch,
                    applied: true,
                    preview_changed: false,
                }
            }
        }
    }

    /// Applies an authoritative cosmetics snapshot. Without preservation the
    /// preview returns to the current appearance.
    fn apply_cosmetics_snapshot(&mut self, cosmetics: api::Cosmetics, preserve: bool) -> bool {
        let mut preview_changed = false;
        if !preserve {
            let worn = cosmetics.capes.iter().find(|cape| cape.selected);
            self.choice = SkinChoice::Current;
            self.cape_choice = worn.map(|cape| cape.id.clone());
            let skin = cosmetics.current_skin.clone().map(Arc::new);
            let cape = worn.and_then(|cape| cape.preview.clone()).map(Arc::new);
            preview_changed = self.set_preview(skin, cape, false);
        }
        self.cosmetics = Some(cosmetics);
        preview_changed
    }

    /// A thumbnail read is accepted only for a preset that still exists in
    /// the same library revision.
    fn thumbnail_accept(&self, serial: u64, id: &str) -> bool {
        serial == self.library_serial && self.preset(id).is_some()
    }

    // -- writes -----------------------------------------------------------

    fn request_write(&mut self, write: Write) -> Option<u64> {
        if self.busy() || !self.valid(&write) {
            return None;
        }
        // This write invalidates every outstanding read's generation. Keep
        // their reload intents even if a stale reply arrives during the write.
        self.library_pending |= self.library_read.is_some();
        self.cosmetics_pending |= self.cosmetics_read.is_some();
        self.generation = self.generation.wrapping_add(1);
        self.error = None;
        self.notice = None;
        self.write_serial = self.write_serial.wrapping_add(1);
        let tag = self.write_serial;
        self.write = Some((tag, write));
        Some(tag)
    }

    /// Every write target is validated against current state before it
    /// starts; malformed or stale targets never start a write.
    fn valid(&self, write: &Write) -> bool {
        match write {
            Write::Import { name, model } => !name.trim().is_empty() && valid_model(model),
            Write::Favorite { preset, value } => {
                self.preset(preset).is_some_and(|p| p.favorite != *value)
            }
            Write::Rename { preset, name } => {
                !name.trim().is_empty() && self.preset(preset).is_some()
            }
            Write::Remove { preset } => self.preset(preset).is_some(),
            Write::SaveCurrent { account } => {
                self.signed_in().is_some_and(|i| &i.id == account)
                    && self.cosmetics.as_ref().is_some_and(|c| c.has_current_skin)
            }
            Write::Apply {
                account,
                preset,
                model,
            } => {
                valid_model(model)
                    && self.signed_in().is_some_and(|i| &i.id == account)
                    && self.preset(preset).is_some()
            }
            Write::SelectCape { account, cape } => {
                self.signed_in().is_some_and(|i| &i.id == account) && self.cape(cape).is_some()
            }
            Write::DisableCape { account } => {
                self.signed_in().is_some_and(|i| &i.id == account) && self.worn_cape().is_some()
            }
        }
    }

    /// A write settled terminally. Stale and duplicate terminals are ignored;
    /// busy clears exactly once. A remote write whose exact identity drifted
    /// clears busy and reports a possible persisted side effect but its
    /// snapshot is never applied.
    fn write_settled(&mut self, tag: u64, result: WriteResult) -> Option<WriteOutcome> {
        let (current, write) = self.write.clone()?;
        if current != tag {
            return None;
        }
        self.write = None;
        self.generation = self.generation.wrapping_add(1);
        let drifted = write
            .remote_account()
            .is_some_and(|account| self.signed_in().map(|i| i.id.as_str()) != Some(account));
        match result {
            WriteResult::Cancelled => Some(WriteOutcome {
                ok: false,
                ..Default::default()
            }),
            WriteResult::Failure(error) => {
                if drifted {
                    self.notice = Some(ACCOUNT_CHANGED.to_owned());
                    Some(WriteOutcome::default())
                } else {
                    self.error = Some(error.message);
                    Some(WriteOutcome::default())
                }
            }
            WriteResult::Library(list) => {
                self.library = Some(list);
                self.library_error = None;
                self.library_serial = self.library_serial.wrapping_add(1);
                Some(WriteOutcome {
                    ok: true,
                    changed: true,
                    ..Default::default()
                })
            }
            WriteResult::LibraryReload => {
                match &write {
                    Write::Import { .. } => self.notice = Some(SKIN_IMPORTED.to_owned()),
                    Write::SaveCurrent { .. } => self.notice = Some(SKIN_SAVED.to_owned()),
                    _ => {}
                }
                Some(WriteOutcome {
                    ok: true,
                    changed: true,
                    reload_library: true,
                    ..Default::default()
                })
            }
            WriteResult::Cosmetics(cosmetics) => {
                if drifted {
                    // Documented partial side effect: the mutation may have
                    // persisted under the previous identity; reload instead
                    // of applying the snapshot to a different account.
                    self.notice = Some(ACCOUNT_CHANGED.to_owned());
                    Some(WriteOutcome {
                        ok: true,
                        changed: true,
                        reload_cosmetics: true,
                        ..Default::default()
                    })
                } else {
                    let preview_changed = self.apply_cosmetics_snapshot(cosmetics, false);
                    if matches!(write, Write::Apply { .. }) {
                        self.notice = Some("Skin applied to your Minecraft account.".into());
                    }
                    Some(WriteOutcome {
                        ok: true,
                        changed: true,
                        preview_changed,
                        ..Default::default()
                    })
                }
            }
        }
    }

    // -- reviewed decisions -------------------------------------------------

    /// The library button's activation path, also used by keyboard clicks.
    fn activate_apply(&mut self, preset: &str, blocked: bool) -> bool {
        if self.apply_unavailable(blocked).is_some()
            || self
                .preset(preset)
                .is_some_and(|preset| self.applied(preset))
        {
            return false;
        }
        self.open_apply_review(preset)
    }

    fn open_apply_review(&mut self, preset: &str) -> bool {
        if self.apply_unavailable(false).is_some() {
            return false;
        }
        let Some(account) = self.signed_in() else {
            return false;
        };
        let Some(target) = self.preset(preset) else {
            return false;
        };
        self.apply_review = Some(ApplyReview {
            account: account.id.clone(),
            account_name: account.name.clone(),
            preset: target.id.clone(),
            name: target.name.clone(),
            model: target.model.clone(),
        });
        self.apply_open = true;
        true
    }

    /// Confirms the frozen apply review. The exact account, preset name and
    /// classic/slim model must still match; the frozen model is what is
    /// applied and the local persisted library is never implicitly rewritten.
    fn confirm_apply(&mut self, expected: &ApplyReview) -> Option<Write> {
        let review = self.apply_review.clone()?;
        if self.busy() || !self.apply_open || &review != expected {
            return None;
        }
        self.apply_open = false;
        self.apply_review = None;
        let still_exact = self.signed_in().is_some_and(|i| {
            i.id == review.account
                && self
                    .preset(&review.preset)
                    .is_some_and(|p| p.name == review.name && p.model == review.model)
        });
        if !still_exact {
            self.error = Some(STALE_REVIEW.to_owned());
            return None;
        }
        Some(Write::Apply {
            account: review.account,
            preset: review.preset,
            model: review.model,
        })
    }

    fn dismiss_apply(&mut self, expected: Option<&ApplyReview>) {
        if self.apply_review.as_ref() == expected && !self.busy() {
            self.apply_open = false;
            self.apply_review = None;
        }
    }

    fn open_remove_review(&mut self, preset: &str) -> bool {
        if self.busy() {
            return false;
        }
        let Some(target) = self.preset(preset) else {
            return false;
        };
        self.remove_review = Some(RemoveReview {
            preset: target.id.clone(),
            name: target.name.clone(),
        });
        self.remove_open = true;
        true
    }

    fn confirm_remove(&mut self, expected: &RemoveReview) -> Option<Write> {
        let review = self.remove_review.clone()?;
        if self.busy() || !self.remove_open || &review != expected {
            return None;
        }
        self.remove_open = false;
        self.remove_review = None;
        let still_exact = self
            .preset(&review.preset)
            .is_some_and(|p| p.name == review.name);
        if !still_exact {
            self.error = Some(STALE_REVIEW.to_owned());
            return None;
        }
        Some(Write::Remove {
            preset: review.preset,
        })
    }

    fn dismiss_remove(&mut self, expected: Option<&RemoveReview>) {
        if self.remove_review.as_ref() == expected && !self.busy() {
            self.remove_open = false;
            self.remove_review = None;
        }
    }

    fn open_rename(&mut self, preset: &str) -> bool {
        if self.busy() {
            return false;
        }
        let Some(target) = self.preset(preset) else {
            return false;
        };
        self.rename_review = Some(RemoveReview {
            preset: target.id.clone(),
            name: target.name.clone(),
        });
        self.rename_open = true;
        true
    }

    fn confirm_rename(&mut self, expected: &RemoveReview, name: &str) -> Option<Write> {
        let review = self.rename_review.clone()?;
        if !self.rename_open || &review != expected || name.trim().is_empty() {
            return None;
        }
        self.rename_open = false;
        self.rename_review = None;
        Some(Write::Rename {
            preset: review.preset,
            name: name.trim().to_owned(),
        })
    }

    fn dismiss_rename(&mut self, expected: Option<&RemoveReview>) {
        if self.rename_review.as_ref() == expected {
            self.rename_open = false;
            self.rename_review = None;
        }
    }
}

const SKIN_IMPORTED: &str = "Skin imported to your library.";
const SKIN_SAVED: &str = "Current skin saved to your library.";
const ACCOUNT_CHANGED: &str =
    "The selected account changed while your appearance was updating. Review and try again.";
const STALE_REVIEW: &str = "The skin list changed. Review the decision again.";

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

struct SelectState {
    trigger: FocusHandle,
    content: FocusHandle,
    bounds: Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
    open: bool,
    highlight: usize,
}

impl SelectState {
    fn new(cx: &mut Context<Skins>) -> Self {
        Self {
            trigger: cx.focus_handle(),
            content: cx.focus_handle(),
            bounds: Rc::new(std::cell::Cell::new(None)),
            open: false,
            highlight: 0,
        }
    }
}

enum Message {
    Library(LibraryRead, ApiResult<Vec<api::SkinPreset>>),
    Cosmetics(CosmeticsRead, ApiResult<api::Cosmetics>),
    Thumbnail(u64, String, ApiResult<Option<api::Thumbnail>>),
    PreviewSkin(u64, String, ApiResult<api::HeadAvatar>),
}

pub struct Skins {
    preview: Option<Entity<crate::skin_preview::SkinPreview>>,
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    core: Core,
    focus: FocusHandle,
    active: bool,
    library_actions_open: bool,
    import_open: bool,
    showcase_ready: bool,
    blocked: bool,
    import_name: Entity<InputState>,
    import_model: SelectState,
    rename_input: Entity<InputState>,
    thumbnails: HashMap<String, Arc<RenderImage>>,
    cape_images: HashMap<String, Arc<RenderImage>>,
    retired_images: Vec<Arc<RenderImage>>,
}

async fn read_cosmetics(host: Host, account: String) -> ApiResult<api::Cosmetics> {
    if crate::showcase() {
        crate::skins_showcase::cosmetics(host).await
    } else {
        api::cosmetics(host, account).await
    }
}

impl Skins {
    /// Builds the page. With a host, the local library loads; with no host
    /// the page stays in an unavailable diagnostic state and initiates zero
    /// API calls or pickers.
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let import_name = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Winter coat"));
        let rename_input = cx.new(|cx| InputState::new(window, cx).placeholder("New name"));
        let mut view = Self {
            preview: None,
            host,
            rt: runtime,
            core: Core::default(),
            focus: cx.focus_handle(),
            active: true,
            library_actions_open: false,
            import_open: false,
            showcase_ready: false,
            blocked: false,
            import_name,
            import_model: SelectState::new(cx),
            rename_input,
            thumbnails: HashMap::new(),
            cape_images: HashMap::new(),
            retired_images: Vec::new(),
        };
        view.refresh(cx);
        view
    }

    /// Compose the parent's independent renderer beside appearance controls.
    pub fn set_preview(&mut self, preview: Entity<crate::skin_preview::SkinPreview>) {
        self.preview = Some(preview);
    }

    /// Installs the acknowledged account snapshot from the root. A changed
    /// identity or status invalidates remote state; the local library and a
    /// pending write are untouched. While blocked the reload is only pended.
    pub fn set_accounts(&mut self, state: &api::AccountsState, cx: &mut Context<Self>) {
        if !self.core.set_account(identity(state)) {
            return;
        }
        if self.blocked {
            self.core.cosmetics_pending = true;
        } else if let Some(host) = self.host.clone()
            && let Some(read) = self.core.request_cosmetics()
        {
            let account = read.account.clone();
            self.run(
                cx,
                async move { read_cosmetics(host, account).await },
                move |view, result, cx| view.receive(Message::Cosmetics(read.clone(), result), cx),
            );
        }
        cx.notify();
    }

    /// Explicit reload of the library and (signed in) cosmetics.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked || self.core.busy() {
            return;
        }
        let reads = self.core.begin_refresh();
        if let Some(read) = reads.library {
            let h = host.clone();
            self.run(
                cx,
                async move { api::skin_presets(h).await },
                move |view, result, cx| view.receive(Message::Library(read, result), cx),
            );
        }
        if let Some(read) = reads.cosmetics {
            let account = read.account.clone();
            self.run(
                cx,
                async move { read_cosmetics(host, account).await },
                move |view, result, cx| view.receive(Message::Cosmetics(read.clone(), result), cx),
            );
        }
        if reads.preview_changed {
            cx.emit(SkinsEvent::PreviewChanged);
        }
        cx.notify();
    }

    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active != active {
            self.active = active;
            cx.notify();
        }
    }

    /// The parent's gate on new work; a running write still completes.
    pub fn set_blocked(&mut self, blocked: bool, cx: &mut Context<Self>) {
        if self.blocked == blocked {
            return;
        }
        self.blocked = blocked;
        if !blocked {
            self.dispatch_pending_reads(cx);
        }
        cx.notify();
    }

    pub fn is_busy(&self) -> bool {
        self.core.busy()
    }

    /// The selected preview appearance: skin, cape and whether a cape
    /// inspection requests the rear view.
    pub fn preview_appearance(
        &self,
    ) -> (
        Option<Arc<api::HeadAvatar>>,
        Option<Arc<api::Texture>>,
        bool,
    ) {
        self.core.preview_appearance()
    }

    /// Routes one host event. Skins has no page-specific host events; every
    /// cosmetics change arrives through this page's own acknowledged writes.
    pub fn on_host_event(
        &mut self,
        _name: &str,
        _payload: &serde_json::Value,
        _cx: &mut Context<Self>,
    ) {
    }

    // -- dispatch -----------------------------------------------------------

    /// Runs one backend call through the panic-safe boundary and delivers the
    /// result through an awaited oneshot channel, never by polling.
    fn run<T, F>(
        &self,
        cx: &mut Context<Self>,
        work: F,
        deliver: impl Fn(&mut Self, ApiResult<T>, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
        F: std::future::Future<Output = ApiResult<T>> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let _ = tx.send(guarded(work).await);
        });
        cx.spawn(async move |weak, cx| {
            let result = rx.await.unwrap_or_else(|_| {
                Err(api::ApiError::new(
                    "native_operation_failed",
                    "The skin operation stopped unexpectedly. Refresh and try again.",
                ))
            });
            let _ = weak.update(cx, |view, cx| deliver(view, result, cx));
        })
        .detach();
    }

    fn dispatch_pending_reads(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked || self.core.busy() {
            return;
        }
        let reads = self.core.pending_reads();
        if let Some(read) = reads.library {
            let h = host.clone();
            self.run(
                cx,
                async move { api::skin_presets(h).await },
                move |view, result, cx| view.receive(Message::Library(read, result), cx),
            );
        }
        if let Some(read) = reads.cosmetics {
            let account = read.account.clone();
            self.run(
                cx,
                async move { read_cosmetics(host, account).await },
                move |view, result, cx| view.receive(Message::Cosmetics(read.clone(), result), cx),
            );
        }
    }

    /// Requests thumbnails for listed presets that have no cached image,
    /// bounded to avoid flooding the blocking pool on large skin libraries.
    fn request_missing_thumbnails(&mut self, cx: &mut Context<Self>) {
        static THUMB_SLOTS: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> =
            std::sync::OnceLock::new();
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(list) = self.core.library.clone() else {
            return;
        };
        let serial = self.core.library_serial;
        let slots =
            Arc::clone(THUMB_SLOTS.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4))));
        for preset in list {
            if self.thumbnails.contains_key(&preset.id) {
                continue;
            }
            let id = preset.id.clone();
            let fetch_id = id.clone();
            let h = host.clone();
            let slots = Arc::clone(&slots);
            self.run(
                cx,
                async move {
                    let _permit = slots.acquire_owned().await.ok();
                    api::skin_thumbnail(h, fetch_id).await
                },
                move |view, result, cx| {
                    view.receive(Message::Thumbnail(serial, id.clone(), result), cx)
                },
            );
        }
    }

    /// Prunes the converted image caches to the acknowledged lists and
    /// converts new cape textures once, outside render.
    fn sync_caches(&mut self) {
        if let Some(list) = &self.core.library {
            self.thumbnails.retain(|id, image| {
                let kept = list.iter().any(|p| &p.id == id);
                if !kept {
                    self.retired_images.push(Arc::clone(image));
                }
                kept
            });
        }
        if let Some(cosmetics) = &self.core.cosmetics {
            self.cape_images.retain(|id, image| {
                let kept = cosmetics.capes.iter().any(|c| &c.id == id);
                if !kept {
                    self.retired_images.push(Arc::clone(image));
                }
                kept
            });
            for cape in &cosmetics.capes {
                if let Some(texture) = &cape.preview
                    && !self.cape_images.contains_key(&cape.id)
                    && let Some(pixels) = crate::player::cape_artwork(texture)
                    && let Some(pixels) = image::RgbaImage::from_raw(10, 16, pixels)
                    && let Some(image) = images::rgba_image(
                        120,
                        192,
                        image::imageops::resize(
                            &pixels,
                            120,
                            192,
                            image::imageops::FilterType::Nearest,
                        )
                        .into_raw(),
                    )
                {
                    if let Some(old) = self.cape_images.insert(cape.id.clone(), image) {
                        self.retired_images.push(old);
                    }
                }
            }
        }
    }

    // -- actions ------------------------------------------------------------

    fn open_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked || self.core.busy() {
            return;
        }
        self.import_open = true;
        self.import_name
            .read(cx)
            .presentation()
            .focus_handle()
            .focus(window, cx);
        cx.notify();
    }

    fn start_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked {
            return;
        }
        let name = self
            .import_name
            .read(cx)
            .text()
            .to_string()
            .trim()
            .to_owned();
        if name.is_empty() {
            self.core.error = Some("Name the skin before importing.".into());
            cx.notify();
            return;
        }
        let model = if self.import_model_highlight_slim() {
            "slim"
        } else {
            "classic"
        }
        .to_owned();
        let Some(tag) = self.core.request_write(Write::Import {
            name: name.clone(),
            model: model.clone(),
        }) else {
            cx.notify();
            return;
        };
        cx.emit(SkinsEvent::BusyChanged(true));
        cx.notify();
        // The owned picker needs the window owner, captured on this click.
        let Some(owner) = file_picker::owner(window) else {
            self.receive_write(
                tag,
                WriteResult::Failure(api::ApiError::new(
                    "skin_import_failed",
                    "The file picker is unavailable in this window.",
                )),
                cx,
            );
            return;
        };
        self.run(
            cx,
            async move {
                let Some(bytes) = file_picker::choose(Some(owner)).await? else {
                    return Ok(WriteResult::Cancelled);
                };
                api::import_skin(host, name, model, bytes)
                    .await
                    .map(|_| WriteResult::LibraryReload)
            },
            move |view, result, cx| {
                let result = result.unwrap_or_else(WriteResult::Failure);
                view.receive_write(tag, result, cx);
            },
        );
    }

    fn import_model_highlight_slim(&self) -> bool {
        self.import_model.highlight == 1
    }

    fn favorite(&mut self, preset: String, value: bool, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked {
            return;
        }
        let Some(tag) = self.core.request_write(Write::Favorite {
            preset: preset.clone(),
            value,
        }) else {
            cx.notify();
            return;
        };
        cx.emit(SkinsEvent::BusyChanged(true));
        cx.notify();
        self.run(
            cx,
            async move { api::set_skin_favorite(host, preset, value).await },
            move |view, result, cx| {
                let result = result
                    .map(WriteResult::Library)
                    .unwrap_or_else(WriteResult::Failure);
                view.receive_write(tag, result, cx);
            },
        );
    }

    fn open_rename(&mut self, preset: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked || !self.core.open_rename(&preset) {
            cx.notify();
            return;
        }
        let name = self
            .core
            .rename_review
            .as_ref()
            .map(|r| r.name.clone())
            .unwrap_or_default();
        self.rename_input.update(cx, |state, cx| {
            state.set_value(name, window, cx);
        });
        self.rename_input
            .read(cx)
            .presentation()
            .focus_handle()
            .clone()
            .focus(window, cx);
        cx.notify();
    }

    fn confirm_rename(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked {
            return;
        }
        let expected = self.core.rename_review.clone();
        let name = self
            .rename_input
            .read(cx)
            .text()
            .to_string()
            .trim()
            .to_owned();
        let Some(write) = expected
            .as_ref()
            .and_then(|expected| self.core.confirm_rename(expected, &name))
        else {
            cx.notify();
            return;
        };
        let Write::Rename { preset, name } = write.clone() else {
            return;
        };
        let Some(tag) = self.core.request_write(write) else {
            cx.notify();
            return;
        };
        cx.emit(SkinsEvent::BusyChanged(true));
        cx.notify();
        self.run(
            cx,
            async move { api::rename_skin_preset(host, preset, name).await },
            move |view, result, cx| {
                let result = result
                    .map(WriteResult::Library)
                    .unwrap_or_else(WriteResult::Failure);
                view.receive_write(tag, result, cx);
            },
        );
    }

    fn dismiss_rename(&mut self, expected: Option<&RemoveReview>, cx: &mut Context<Self>) {
        self.core.dismiss_rename(expected);
        cx.notify();
    }

    fn open_remove_review(&mut self, preset: String, cx: &mut Context<Self>) {
        if self.blocked || !self.core.open_remove_review(&preset) {
            cx.notify();
            return;
        }
        cx.notify();
    }

    fn confirm_remove(&mut self, expected: &RemoveReview, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked {
            return;
        }
        let Some(write) = self.core.confirm_remove(expected) else {
            cx.notify();
            return;
        };
        let Write::Remove { preset } = write.clone() else {
            return;
        };
        let Some(tag) = self.core.request_write(write) else {
            cx.notify();
            return;
        };
        cx.emit(SkinsEvent::BusyChanged(true));
        cx.notify();
        self.run(
            cx,
            async move { api::remove_skin_preset(host, preset).await },
            move |view, result, cx| {
                let result = result
                    .map(|_| WriteResult::LibraryReload)
                    .unwrap_or_else(WriteResult::Failure);
                view.receive_write(tag, result, cx);
            },
        );
    }

    fn dismiss_remove(&mut self, expected: Option<&RemoveReview>, cx: &mut Context<Self>) {
        self.core.dismiss_remove(expected);
        cx.notify();
    }

    fn open_apply_review(&mut self, preset: String, cx: &mut Context<Self>) {
        if !self.core.activate_apply(&preset, self.blocked) {
            cx.notify();
            return;
        }
        cx.notify();
    }

    fn confirm_apply(&mut self, expected: &ApplyReview, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked {
            return;
        }
        let Some(write) = self.core.confirm_apply(expected) else {
            cx.notify();
            return;
        };
        let Write::Apply {
            account,
            preset,
            model,
        } = write.clone()
        else {
            return;
        };
        let Some(tag) = self.core.request_write(write) else {
            cx.notify();
            return;
        };
        cx.emit(SkinsEvent::BusyChanged(true));
        cx.notify();
        self.run(
            cx,
            async move {
                if crate::showcase() {
                    crate::skins_showcase::apply(host, preset).await
                } else {
                    api::apply_skin_preset(host, account, preset, model).await
                }
            },
            move |view, result, cx| {
                let result = result
                    .map(WriteResult::Cosmetics)
                    .unwrap_or_else(WriteResult::Failure);
                view.receive_write(tag, result, cx);
            },
        );
    }

    fn dismiss_apply(&mut self, expected: Option<&ApplyReview>, cx: &mut Context<Self>) {
        self.core.dismiss_apply(expected);
        cx.notify();
    }

    fn save_current(&mut self, cx: &mut Context<Self>) {
        if crate::showcase() {
            self.core.notice = Some("This showcase previews account changes locally.".into());
            cx.notify();
            return;
        }

        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked {
            return;
        }
        let Some(account) = self.core.signed_in().map(|i| i.id.clone()) else {
            return;
        };
        let Some(tag) = self.core.request_write(Write::SaveCurrent {
            account: account.clone(),
        }) else {
            cx.notify();
            return;
        };
        cx.emit(SkinsEvent::BusyChanged(true));
        cx.notify();
        self.run(
            cx,
            async move { api::save_current_skin(host, account).await },
            move |view, result, cx| {
                let result = result
                    .map(|_| WriteResult::LibraryReload)
                    .unwrap_or_else(WriteResult::Failure);
                view.receive_write(tag, result, cx);
            },
        );
    }

    fn select_cape(&mut self, cape: String, cx: &mut Context<Self>) {
        if crate::showcase() {
            self.core.notice = Some("This showcase previews account changes locally.".into());
            cx.notify();
            return;
        }

        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked {
            return;
        }
        let Some(account) = self.core.signed_in().map(|i| i.id.clone()) else {
            return;
        };
        let Some(tag) = self.core.request_write(Write::SelectCape {
            account: account.clone(),
            cape: cape.clone(),
        }) else {
            cx.notify();
            return;
        };
        cx.emit(SkinsEvent::BusyChanged(true));
        cx.notify();
        self.run(
            cx,
            async move { api::select_cape(host, account, cape).await },
            move |view, result, cx| {
                let result = result
                    .map(WriteResult::Cosmetics)
                    .unwrap_or_else(WriteResult::Failure);
                view.receive_write(tag, result, cx);
            },
        );
    }

    fn disable_cape(&mut self, cx: &mut Context<Self>) {
        if crate::showcase() {
            self.core.notice = Some("This showcase previews account changes locally.".into());
            cx.notify();
            return;
        }

        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked {
            return;
        }
        let Some(account) = self.core.signed_in().map(|i| i.id.clone()) else {
            return;
        };
        let Some(tag) = self.core.request_write(Write::DisableCape {
            account: account.clone(),
        }) else {
            cx.notify();
            return;
        };
        cx.emit(SkinsEvent::BusyChanged(true));
        cx.notify();
        self.run(
            cx,
            async move { api::disable_cape(host, account).await },
            move |view, result, cx| {
                let result = result
                    .map(WriteResult::Cosmetics)
                    .unwrap_or_else(WriteResult::Failure);
                view.receive_write(tag, result, cx);
            },
        );
    }

    fn preview_preset(&mut self, preset: String, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            return;
        };
        if self.blocked {
            return;
        }
        let Some((serial, id, changed_rear)) = self.core.preview_preset(&preset) else {
            return;
        };
        let fetch_id = id.clone();
        if changed_rear {
            cx.emit(SkinsEvent::PreviewChanged);
        }
        cx.notify();
        self.run(
            cx,
            async move { api::skin_preview(host, fetch_id).await },
            move |view, result, cx| {
                view.receive(Message::PreviewSkin(serial, id.clone(), result), cx)
            },
        );
    }

    fn preview_current(&mut self, cx: &mut Context<Self>) {
        if self.core.preview_current() {
            cx.emit(SkinsEvent::PreviewChanged);
        }
        cx.notify();
    }

    fn inspect_cape(&mut self, cape: String, cx: &mut Context<Self>) {
        if let Some(changed) = self.core.inspect_cape(&cape)
            && changed
        {
            cx.emit(SkinsEvent::PreviewChanged);
        }
        cx.notify();
    }

    // -- message routing ----------------------------------------------------

    fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Library(read, result) => {
                let settlement = self.core.library_settled(read, result);
                if settlement.applied {
                    self.sync_caches();
                    self.request_missing_thumbnails(cx);
                }
                if settlement.redispatch {
                    self.dispatch_pending_reads(cx);
                }
                cx.notify();
            }
            Message::Cosmetics(read, result) => {
                let settlement = self.core.cosmetics_settled(read, result);
                if settlement.applied {
                    self.sync_caches();
                    if settlement.preview_changed {
                        cx.emit(SkinsEvent::PreviewChanged);
                    }
                }
                if settlement.redispatch {
                    self.dispatch_pending_reads(cx);
                }
                cx.notify();
            }
            Message::Thumbnail(serial, id, result) => {
                if !self.core.thumbnail_accept(serial, &id) {
                    return;
                }
                if let Ok(Some(thumbnail)) = result
                    && let Some(pixels) = image::RgbaImage::from_raw(8, 8, thumbnail.rgba)
                    && let Some(image) = images::rgba_image(
                        64,
                        64,
                        image::imageops::resize(
                            &pixels,
                            64,
                            64,
                            image::imageops::FilterType::Nearest,
                        )
                        .into_raw(),
                    )
                {
                    if let Some(old) = self.thumbnails.insert(id, image) {
                        self.retired_images.push(old);
                    }
                    cx.notify();
                }
            }
            Message::PreviewSkin(serial, id, result) => {
                let arrival = self.core.preview_arrived(serial, &id, result);
                if !arrival.applied {
                    return;
                }
                if arrival.preview_changed {
                    cx.emit(SkinsEvent::PreviewChanged);
                }
                cx.notify();
            }
        }
        if crate::showcase()
            && !self.showcase_ready
            && self.core.library.is_some()
            && self.core.cosmetics.is_some()
        {
            self.showcase_ready = true;
            match crate::skins_showcase::state().as_str() {
                "review" => {
                    self.open_apply_review("00000000-0000-0000-0000-000000000002".into(), cx);
                }
                "manage" => {
                    self.library_actions_open = true;
                }
                "success" => {
                    self.core.notice = Some("Skin applied to your Minecraft account.".into());
                }
                _ => {}
            }
        }
    }

    fn receive_write(&mut self, tag: u64, result: WriteResult, cx: &mut Context<Self>) {
        let Some(outcome) = self.core.write_settled(tag, result) else {
            return;
        };
        cx.emit(SkinsEvent::BusyChanged(false));
        if outcome.changed {
            cx.emit(SkinsEvent::Changed);
        }
        if outcome.preview_changed {
            cx.emit(SkinsEvent::PreviewChanged);
        }
        if outcome.reload_library {
            self.core.library_pending = true;
        }
        if outcome.reload_cosmetics {
            self.core.cosmetics_pending = true;
        }
        self.sync_caches();
        self.dispatch_pending_reads(cx);
        self.request_missing_thumbnails(cx);
        cx.notify();
    }
}

impl EventEmitter<SkinsEvent> for Skins {}

impl Render for Skins {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for image in self.retired_images.drain(..) {
            let _ = window.drop_image(image);
        }
        let palette = theme::theme(cx).clone();
        let view = cx.entity();
        let busy = self.is_busy();
        let locked = busy || self.blocked;
        let mut content = div()
            .id("skins")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .gap(px(ui::PAGE_GAP))
            .w_full()
            .text_color(palette.foreground);
        if self.host.is_none() {
            return div().w_full().child(content.child(ui::empty_state(
                &palette,
                "checkroom",
                "Skins unavailable",
                "This window has no launcher data.",
            )));
        }
        if let Some(notice) = &self.core.notice {
            content = content.child(notice_bar(&palette, notice.clone(), ui::Tone::Accent));
        }
        if let Some(error) = &self.core.error {
            content = content.child(notice_bar(&palette, error.clone(), ui::Tone::Danger));
        }

        let cosmetics = self.core.cosmetics.as_ref();
        let account = self.core.account.as_ref();
        let signed_in = self.core.signed_in();
        let active_skin_name = self
            .core
            .library
            .as_ref()
            .and_then(|list| list.iter().find(|p| self.core.applied(p)))
            .map(|p| p.name.clone());
        let preview_name = match &self.core.choice {
            SkinChoice::Current => None,
            SkinChoice::Preset(id) => Some(format!(
                "Previewing {}",
                self.core
                    .preset(id)
                    .map(|p| p.name.as_str())
                    .unwrap_or("skin")
            )),
        };
        let mut details = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(220.))
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(ui::TEXT_DISPLAY))
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(palette.foreground)
                            .child(
                                account
                                    .map(|a| a.name.clone())
                                    .unwrap_or_else(|| "Skins".into()),
                            ),
                    )
                    .when_some(
                        match signed_in {
                            Some(_) => preview_name
                                .or_else(|| active_skin_name.map(|name| format!("Wearing {name}")))
                                .map(gpui_kit::SharedString::from),
                            None if account.is_some() => {
                                Some("Sign in again to apply skins.".into())
                            }
                            None => Some("Sign in to wear skins and capes.".into()),
                        },
                        |d, subtitle| d.child(ui::muted(&palette, subtitle)),
                    ),
            )
            .when_some(cosmetics, |d, cosmetics| {
                d.child(div().flex().flex_wrap().gap(px(8.)).child(ui::badge(
                    &palette,
                    format!(
                        "{} model",
                        model_label(cosmetics.current_skin_model.as_deref().unwrap_or("classic"),)
                    ),
                    ui::Tone::Neutral,
                )))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("skins-import-open", "Import PNG")
                            .variant(ButtonVariant::Primary)
                            .icon("upload")
                            .disabled(locked)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_import(window, cx)),
                            ),
                    )
                    .when(cosmetics.is_some_and(|c| c.has_current_skin), |row| {
                        row.child(
                            AuroraButton::new("skins-save-current", "Save to library")
                                .variant(ButtonVariant::Default)
                                .disabled(locked)
                                .on_click(cx.listener(|this, _, _, cx| this.save_current(cx))),
                        )
                    })
                    .when(
                        signed_in.is_some()
                            && (self.core.choice != SkinChoice::Current || self.core.rear),
                        |row| {
                            row.child(
                                AuroraButton::new("skins-preview-current", "Reset preview")
                                    .variant(ButtonVariant::Ghost)
                                    .icon("person")
                                    .disabled(locked)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.preview_current(cx)),
                                    ),
                            )
                        },
                    ),
            );
        if let Some(error) = &self.core.remote_error {
            details = details.child(notice_bar(&palette, error.clone(), ui::Tone::Danger));
        } else if signed_in.is_some() && cosmetics.is_none() {
            details = details.child(ui::muted(&palette, "Loading your current look…"));
        }

        // Owned Capes live right inside the hero card so the right column feels balanced & complete.
        let mut capes = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .pt(px(14.))
            .border_t_1()
            .border_color(palette.border_subtle);
        let mut cape_header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(ui::TEXT_CAPTION))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(palette.muted_foreground)
                    .child("Capes"),
            );
        if let Some(cosmetics) = cosmetics {
            if cosmetics.capes.iter().any(|c| c.selected) {
                cape_header = cape_header.child(
                    AuroraButton::new("skins-cape-disable", "Unequip")
                        .size(ButtonSize::Small)
                        .variant(ButtonVariant::Default)
                        .disabled(locked)
                        .on_click(cx.listener(|this, _, _, cx| this.disable_cape(cx))),
                );
            }
            capes = capes.child(cape_header);
            if cosmetics.capes.is_empty() {
                capes = capes.child(ui::caption(&palette, "No capes on this account yet."));
            } else {
                let mut cape_grid = div().flex().flex_wrap().gap(px(10.));
                let hover_cape = palette.control_hover;
                for cape in &cosmetics.capes {
                    let inspect_id = cape.id.clone();
                    let wear_id = cape.id.clone();
                    let previewing = self.core.cape_choice.as_deref() == Some(cape.id.as_str())
                        && self.core.rear;
                    cape_grid = cape_grid.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .px(px(12.))
                            .py(px(10.))
                            .rounded(palette.radius)
                            .bg(palette.control)
                            .border_1()
                            .border_color(if cape.selected || previewing {
                                palette.ring
                            } else {
                                palette.border_subtle
                            })
                            .child(
                                gpui_kit::base::Button::new(format!("skins-cape-view-{}", cape.id))
                                    .accessibility_label(format!("Preview {} cape", cape.name))
                                    .selected(previewing)
                                    .disabled(locked)
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .rounded(px(6.))
                                    .hover(move |s| s.bg(hover_cape))
                                    .child(
                                        div()
                                            .w(px(30.))
                                            .h(px(46.))
                                            .flex_shrink_0()
                                            .rounded(px(4.))
                                            .bg(palette.card)
                                            .when_some(
                                                self.cape_images.get(&cape.id).cloned(),
                                                |d, image| {
                                                    d.child(img(image).w(px(30.)).h(px(46.)))
                                                },
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(ui::TEXT_BODY))
                                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                            .text_color(palette.foreground)
                                            .child(cape.name.clone()),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.inspect_cape(inspect_id.clone(), cx)
                                    })),
                            )
                            .child(if cape.selected {
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .text_size(px(ui::TEXT_CAPTION))
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                    .text_color(palette.success)
                                    .child(div().size(px(6.)).rounded_full().bg(palette.success))
                                    .child("Equipped")
                                    .into_any_element()
                            } else {
                                AuroraButton::new(format!("skins-cape-wear-{}", cape.id), "Equip")
                                    .accessibility_label(format!("Equip {} cape", cape.name))
                                    .size(ButtonSize::Small)
                                    .variant(ButtonVariant::Default)
                                    .disabled(locked)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_cape(wear_id.clone(), cx)
                                    }))
                                    .into_any_element()
                            }),
                    );
                }
                capes = capes.child(cape_grid);
            }
        } else {
            capes = capes.child(cape_header).child(ui::caption(
                &palette,
                if signed_in.is_none() {
                    "Sign in to browse and wear owned capes."
                } else if self.core.remote_error.is_some() {
                    "Refresh to load your capes."
                } else {
                    "Loading capes…"
                },
            ));
        }
        details = details.child(capes);

        let mut appearance = ui::card(&palette)
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(ui::PAGE_GAP));
        // Keep the 3D skin preview visible first at every window width.
        if let Some(preview) = &self.preview {
            appearance = appearance.child(preview.clone());
        }
        appearance = appearance.child(details);
        content = content.child(appearance);

        let import_select = &self.import_model;
        let select_view = view.clone();
        let highlight_view = view.clone();
        let confirm_view = view.clone();
        let import_form = div()
            .flex()
            .flex_wrap()
            .items_end()
            .gap(px(12.))
            .child(
                field(&palette, "Skin name")
                    .flex_1()
                    .min_w(px(200.))
                    .child(AuroraInput::new(&self.import_name).label("Skin name")),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .w_full()
                    .items_end()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        // The select carries a fixed 220px width; a flex row lets it
                        // shrink to this field instead of overlapping the button.
                        field(&palette, "Model").w(px(180.)).flex_shrink_0().child(
                            div().flex().w_full().min_w(px(0.)).child(
                                AuroraSelect::new(
                                    "skins-import-model",
                                    "Model",
                                    vec!["Classic".into(), "Slim".into()],
                                    &import_select.trigger,
                                    &import_select.content,
                                    import_select.bounds.clone(),
                                )
                                .selected(Some(if self.import_model_highlight_slim() {
                                    1
                                } else {
                                    0
                                }))
                                .open(import_select.open)
                                .highlight(import_select.highlight)
                                .disabled(locked)
                                .on_open_change(move |open, _, cx| {
                                    select_view.update(cx, |this, cx| {
                                        this.import_model.open = open;
                                        cx.notify();
                                    });
                                })
                                .on_highlight(move |highlight, _, cx| {
                                    highlight_view.update(cx, |this, cx| {
                                        this.import_model.highlight = highlight;
                                        cx.notify();
                                    });
                                })
                                .on_confirm(
                                    move |selected, _, cx| {
                                        confirm_view.update(cx, |this, cx| {
                                            this.import_model.highlight = selected;
                                            cx.notify();
                                        });
                                    },
                                ),
                            ),
                        ),
                    )
                    .child(
                        div().flex_1().min_w(px(160.)).child(
                            AuroraButton::new("skins-import", "Import PNG…")
                                .variant(ButtonVariant::Primary)
                                .icon("upload")
                                .full_width()
                                .disabled(locked)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.start_import(window, cx);
                                })),
                        ),
                    ),
            );

        if self.import_open {
            content = content.child(
                ui::card(&palette)
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .child(ui::section_header(
                                &palette,
                                "Import a skin",
                                Some("Choose a name and the arm model for your PNG.".into()),
                            ))
                            .child(
                                AuroraButton::new("skins-import-close", "Done")
                                    .size(ButtonSize::Small)
                                    .variant(ButtonVariant::Ghost)
                                    .disabled(locked)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.import_open = false;
                                        this.focus.focus(window, cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(import_form),
            );
        }

        let favorites_view = view.clone();
        let count = self.core.filtered_library().len();
        let mut library_header = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(ui::section_header(&palette, "Library", None))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new(
                            "skins-manage-library",
                            if self.library_actions_open {
                                "Done"
                            } else {
                                "Edit"
                            },
                        )
                        .size(ButtonSize::Small)
                        .variant(ButtonVariant::Default)
                        .selected(self.library_actions_open)
                        .disabled(locked || count == 0 && !self.library_actions_open)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library_actions_open = !this.library_actions_open;
                            cx.notify();
                        })),
                    )
                    .child(ui::caption(&palette, "Favorites only"))
                    .child(
                        AuroraToggle::new("skins-favorites-only")
                            .label("Favorites only")
                            .checked(self.core.favorites_only)
                            .disabled(locked)
                            .on_change(move |checked, _, _, cx| {
                                favorites_view.update(cx, |this, cx| {
                                    this.core.favorites_only = checked;
                                    cx.notify();
                                });
                            }),
                    ),
            );
        if let Some(reason) = self.core.apply_unavailable(self.blocked) {
            library_header = div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(library_header)
                .child(ui::muted(&palette, reason));
        }
        let mut library_section = div().flex().flex_col().gap(px(16.)).child(library_header);
        if let Some(error) = &self.core.library_error {
            library_section = library_section
                .child(notice_bar(&palette, error.clone(), ui::Tone::Danger))
                .child(
                    div().flex().child(
                        AuroraButton::new("library-retry", "Try again")
                            .disabled(locked)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
                );
        } else if self.core.library.is_none() {
            library_section = library_section.child(ui::muted(&palette, "Loading your library…"));
        } else if count == 0 {
            let filtered = self.core.favorites_only
                && self.core.library.as_ref().is_some_and(|l| !l.is_empty());
            library_section = library_section.child(
                ui::empty_state(
                    &palette,
                    if filtered { "star" } else { "checkroom" },
                    if filtered {
                        "No favorite skins"
                    } else {
                        "No saved skins"
                    },
                    if filtered {
                        "Turn off Favorites only to see all skins."
                    } else {
                        "Import a skin PNG to start your library."
                    },
                )
                .when(!filtered, |d| {
                    d.child(
                        AuroraButton::new("skins-empty-import", "Import PNG…")
                            .variant(ButtonVariant::Default)
                            .icon("upload")
                            .disabled(locked)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_import(window, cx)),
                            ),
                    )
                }),
            );
        } else {
            // The parent reserves 212px for navigation and 32px page padding
            // on either side. Recompute on resize; no saved layout is changed.
            let available = (f32::from(window.viewport_size().width)
                - crate::navigation::PAGE_CHROME)
                .min(ui::MAX_CONTENT_WIDTH);
            let columns = ((available + 14.) / 215.).floor().clamp(1., 4.) as u16;
            let mut grid = div()
                .id("skins-library")
                .grid()
                .grid_cols(columns)
                .gap(px(14.))
                .w_full();
            for preset in self.core.filtered_library() {
                let preview_id = preset.id.clone();
                let apply_id = preset.id.clone();
                let rename_id = preset.id.clone();
                let remove_id = preset.id.clone();
                let favorite_id = preset.id.clone();
                let next_favorite = !preset.favorite;
                let previewed = self.core.choice == SkinChoice::Preset(preset.id.clone());
                let applied = self.core.applied(preset);
                let thumbnail = self.thumbnails.get(&preset.id).cloned();
                let hover_bg = palette.control_hover;
                let mut card = ui::card(&palette)
                    .id(format!("skins-card-{}", preset.id))
                    .justify_center()
                    .min_w(px(0.))
                    .p(px(12.))
                    .gap(px(8.))
                    .when(previewed || applied, |d| d.border_color(palette.ring))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .w_full()
                            .min_w(px(0.))
                            .child(
                                gpui_kit::base::Button::new(format!("skins-preview-{}", preset.id))
                                    .accessibility_label(format!("Preview {}", preset.name))
                                    .disabled(locked)
                                    .flex()
                                    .flex_1()
                                    .items_center()
                                    .gap(px(12.))
                                    .min_w(px(0.))
                                    .p(px(4.))
                                    .m(px(-4.))
                                    .rounded(palette.radius)
                                    .hover(move |s| s.bg(hover_bg))
                                    .child(
                                        div()
                                            .size(px(48.))
                                            .flex_shrink_0()
                                            .rounded(palette.radius)
                                            .overflow_hidden()
                                            .bg(palette.control)
                                            .when_some(thumbnail, |d, image| {
                                                d.child(img(image).size(px(48.)))
                                            }),
                                    )
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
                                                    .overflow_hidden()
                                                    .text_ellipsis()
                                                    .whitespace_nowrap()
                                                    .text_size(px(ui::TEXT_BODY + 1.))
                                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                                    .text_color(palette.foreground)
                                                    .child(preset.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .h(px(22.))
                                                    .gap(px(6.))
                                                    .child(ui::caption(
                                                        &palette,
                                                        model_label(&preset.model),
                                                    ))
                                                    .when(applied, |row| {
                                                        row.child(
                                                            div()
                                                                .text_size(px(ui::TEXT_CAPTION))
                                                                .font_weight(
                                                                    gpui_kit::FontWeight::MEDIUM,
                                                                )
                                                                .text_color(palette.success)
                                                                .child("· Equipped"),
                                                        )
                                                    }),
                                            ),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.preview_preset(preview_id.clone(), cx)
                                    })),
                            )
                            .when(previewed && !applied, |row| {
                                row.child(
                                    AuroraButton::new(
                                        format!("skins-apply-{}", preset.id),
                                        "Equip",
                                    )
                                    .accessibility_label(format!(
                                        "Apply {} to {}{}",
                                        preset.name,
                                        signed_in
                                            .map(|a| a.name.as_str())
                                            .unwrap_or("your account"),
                                        self.core
                                            .apply_unavailable(self.blocked)
                                            .map(|reason| format!(". {reason}"))
                                            .unwrap_or_default()
                                    ))
                                    .variant(ButtonVariant::Soft)
                                    .size(ButtonSize::Small)
                                    .disabled(self.core.apply_unavailable(self.blocked).is_some())
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.open_apply_review(apply_id.clone(), cx)
                                        },
                                    )),
                                )
                            })
                            .child(
                                gpui_kit::base::Button::new(format!(
                                    "skins-favorite-{}",
                                    preset.id
                                ))
                                .accessibility_label(format!(
                                    "{} {}",
                                    if preset.favorite {
                                        "Unfavorite"
                                    } else {
                                        "Favorite"
                                    },
                                    preset.name
                                ))
                                .selected(preset.favorite)
                                .disabled(locked)
                                .self_start()
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(24.))
                                .rounded(palette.radius)
                                .when(!preset.favorite && !self.library_actions_open, |b| {
                                    b.opacity(0.28)
                                })
                                .hover(move |s| s.bg(hover_bg).opacity(1.0))
                                .child(crate::symbols::icon(
                                    Some("star"),
                                    px(16.),
                                    if preset.favorite {
                                        palette.ring
                                    } else {
                                        palette.subtle_foreground
                                    },
                                ))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.favorite(favorite_id.clone(), next_favorite, cx)
                                    },
                                )),
                            ),
                    );
                if self.library_actions_open {
                    card = card.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(4.))
                            .child(
                                AuroraButton::new(format!("skins-rename-{}", preset.id), "Rename")
                                    .accessibility_label(format!("Rename {}", preset.name))
                                    .size(ButtonSize::Small)
                                    .variant(ButtonVariant::Ghost)
                                    .disabled(locked)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_rename(rename_id.clone(), window, cx)
                                    })),
                            )
                            .child(
                                AuroraButton::new(format!("skins-remove-{}", preset.id), "Remove")
                                    .accessibility_label(format!("Remove {}", preset.name))
                                    .size(ButtonSize::Small)
                                    .variant(ButtonVariant::DangerSubtle)
                                    .disabled(locked)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_remove_review(remove_id.clone(), cx)
                                    })),
                            ),
                    );
                }
                grid = grid.child(card);
            }
            library_section = library_section.child(grid);
        }
        content = content.child(library_section);

        div()
            .w_full()
            .flex_shrink_0()
            .child(content)
            .child(self.rename_dialog(cx, &palette, busy))
            .child(self.apply_dialog(cx, &palette, busy))
            .child(self.remove_dialog(cx, &palette, busy))
    }
}

impl Skins {
    fn dialog_panel(&self, palette: &crate::theme::AuroraTheme, title: &str) -> gpui_kit::Div {
        div()
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
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(title.to_owned()),
            )
    }

    fn rename_dialog(
        &self,
        cx: &mut Context<Self>,
        palette: &crate::theme::AuroraTheme,
        busy: bool,
    ) -> gpui_kit::base::Dialog {
        let review = self.core.rename_review.clone();
        let cancel_review = review.clone();
        let change_review = review.clone();
        let view = cx.entity();
        let weak = view.downgrade();
        let page_focus = self.focus.clone();
        let name = review
            .as_ref()
            .map(|review| review.name.clone())
            .unwrap_or_default();
        let panel = self
            .dialog_panel(palette, "Rename skin")
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(palette.muted_foreground)
                    .child(format!("Rename “{name}” in your skin library.")),
            )
            .child(AuroraInput::new(&self.rename_input).label("New name"))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("skins-rename-cancel", "Cancel")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dismiss_rename(cancel_review.as_ref(), cx);
                            })),
                    )
                    .child(
                        AuroraButton::new("skins-rename-save", "Save")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.confirm_rename(cx);
                            })),
                    ),
            );
        Dialog::new(cx)
            .open(self.core.rename_open && self.core.rename_review.is_some())
            .accessibility_label("Rename skin")
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_cancel(move |_, _, app| view.read(app).core.write.is_none())
            .on_open_change(move |open, _reason, window, app| {
                if !open {
                    page_focus.focus(window, app);
                }
                let _ = weak.update(app, |this, cx| {
                    if !open {
                        this.core.dismiss_rename(change_review.as_ref());
                    }
                    cx.notify();
                });
            })
            .backdrop(div().absolute().size_full().bg(rgba(0x000000a6)))
            .popup(panel)
    }

    fn apply_dialog(
        &self,
        cx: &mut Context<Self>,
        palette: &crate::theme::AuroraTheme,
        busy: bool,
    ) -> gpui_kit::base::Dialog {
        let review = self.core.apply_review.clone();
        let cancel_review = review.clone();
        let confirm_review = review.clone();
        let change_review = review.clone();
        let view = cx.entity();
        let weak = view.downgrade();
        let page_focus = self.focus.clone();
        let text = review
            .as_ref()
            .map(|review| {
                format!(
                    "Apply “{}” to {} using the {} model? This changes the skin on your Minecraft account.",
                    review.name,
                    review.account_name,
                    model_label(&review.model).to_lowercase()
                )
            })
            .unwrap_or_default();
        let panel = self
            .dialog_panel(palette, "Apply skin")
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(palette.muted_foreground)
                    .child(text),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("skins-apply-cancel", "Cancel")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dismiss_apply(cancel_review.as_ref(), cx);
                            })),
                    )
                    .child(
                        AuroraButton::new("skins-apply-confirm", "Apply skin")
                            .variant(ButtonVariant::Primary)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(expected) = confirm_review.as_ref() {
                                    this.confirm_apply(expected, cx);
                                }
                            })),
                    ),
            );
        Dialog::new(cx)
            .open(self.core.apply_open && self.core.apply_review.is_some())
            .accessibility_label("Apply skin")
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_cancel(move |_, _, app| view.read(app).core.write.is_none())
            .on_open_change(move |open, _reason, window, app| {
                if !open {
                    page_focus.focus(window, app);
                }
                let _ = weak.update(app, |this, cx| {
                    if !open {
                        this.core.dismiss_apply(change_review.as_ref());
                    }
                    cx.notify();
                });
            })
            .backdrop(div().absolute().size_full().bg(rgba(0x000000a6)))
            .popup(panel)
    }

    fn remove_dialog(
        &self,
        cx: &mut Context<Self>,
        palette: &crate::theme::AuroraTheme,
        busy: bool,
    ) -> gpui_kit::base::Dialog {
        let review = self.core.remove_review.clone();
        let cancel_review = review.clone();
        let confirm_review = review.clone();
        let change_review = review.clone();
        let view = cx.entity();
        let weak = view.downgrade();
        let page_focus = self.focus.clone();
        let text = review
            .as_ref()
            .map(|review| {
                format!(
                    "Remove “{}” from your skin library? This cannot be undone.",
                    review.name
                )
            })
            .unwrap_or_default();
        let panel = self
            .dialog_panel(palette, "Remove skin")
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(palette.muted_foreground)
                    .child(text),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("skins-remove-cancel", "Cancel")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.dismiss_remove(cancel_review.as_ref(), cx);
                            })),
                    )
                    .child(
                        AuroraButton::new("skins-remove-confirm", "Remove skin")
                            .variant(ButtonVariant::Danger)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(expected) = confirm_review.as_ref() {
                                    this.confirm_remove(expected, cx);
                                }
                            })),
                    ),
            );
        Dialog::new(cx)
            .open(self.core.remove_open && self.core.remove_review.is_some())
            .accessibility_label("Remove skin")
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_cancel(move |_, _, app| view.read(app).core.write.is_none())
            .on_open_change(move |open, _reason, window, app| {
                if !open {
                    page_focus.focus(window, app);
                }
                let _ = weak.update(app, |this, cx| {
                    if !open {
                        this.core.dismiss_remove(change_review.as_ref());
                    }
                    cx.notify();
                });
            })
            .backdrop(div().absolute().size_full().bg(rgba(0x000000a6)))
            .popup(panel)
    }
}

// ---------------------------------------------------------------------------
// Tests: the pure page core, exercised through its real state methods
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(id: &str, name: &str, model: &str, favorite: bool) -> api::SkinPreset {
        api::SkinPreset {
            id: id.to_owned(),
            sha256: String::new(),
            name: name.to_owned(),
            model: model.to_owned(),
            favorite,
        }
    }

    fn texture(tag: u8) -> api::Texture {
        api::Texture {
            width: 2,
            height: 1,
            rgba: vec![tag; 8],
        }
    }

    fn avatar(tag: u8) -> api::HeadAvatar {
        api::HeadAvatar {
            sha256: String::new(),
            rgba: vec![tag; 4],
            skin_rgba: vec![tag; 64 * 64 * 4],
            skin_height: 64,
            model: "classic".into(),
        }
    }

    fn cosmetics(current: Option<api::HeadAvatar>, capes: Vec<api::Cape>) -> api::Cosmetics {
        api::Cosmetics {
            current_skin_model: Some("classic".into()),
            has_current_skin: current.is_some(),
            current_skin: current,
            capes,
        }
    }

    fn cape(id: &str, name: &str, selected: bool, preview: bool) -> api::Cape {
        api::Cape {
            id: id.to_owned(),
            name: name.to_owned(),
            selected,
            preview: preview.then(|| texture(7)),
        }
    }

    fn signed_in(id: &str, name: &str) -> Identity {
        Identity {
            id: id.into(),
            name: name.into(),
            status: "signedIn".into(),
        }
    }

    fn library_ready() -> Core {
        let mut core = Core::default();
        core.library = Some(vec![
            preset("a", "Alpha", "classic", false),
            preset("b", "Beta", "slim", true),
        ]);
        core
    }

    fn appearance_ready() -> Core {
        let mut core = library_ready();
        core.account = Some(signed_in("acc-1", "Ann"));
        core.cosmetics = Some(cosmetics(
            Some(avatar(1)),
            vec![
                cape("migrator", "Migrator", true, true),
                cape("vanilla", "Vanilla", false, true),
            ],
        ));
        core
    }

    #[test]
    fn write_invalidated_reads_reload_after_either_reply_order() {
        for reads_finish_first in [true, false] {
            for outcome in 0..3 {
                let terminal = || match outcome {
                    0 => WriteResult::Library(vec![preset("a", "Alpha", "classic", true)]),
                    1 => WriteResult::Failure(api::ApiError::new("fixture", "Failed write")),
                    _ => WriteResult::Cancelled,
                };
                let mut core = appearance_ready();
                let library = core.request_library().unwrap();
                let cosmetics_read = core.request_cosmetics().unwrap();
                let write = core
                    .request_write(Write::Favorite {
                        preset: "a".into(),
                        value: true,
                    })
                    .unwrap();
                if !reads_finish_first {
                    core.write_settled(write, terminal()).unwrap();
                }
                assert!(!core.library_settled(library, Ok(vec![])).applied);
                assert!(
                    !core
                        .cosmetics_settled(cosmetics_read, Ok(cosmetics(None, vec![])))
                        .applied
                );
                if reads_finish_first {
                    assert_eq!(core.pending_reads(), RefreshReads::default());
                    core.write_settled(write, terminal()).unwrap();
                }
                let fresh = core.pending_reads();
                assert!(
                    core.library_settled(fresh.library.unwrap(), Ok(vec![]))
                        .applied
                );
                assert!(
                    core.cosmetics_settled(fresh.cosmetics.unwrap(), Ok(cosmetics(None, vec![])))
                        .applied
                );
                assert_eq!(core.pending_reads(), RefreshReads::default());
                assert!(core.library.is_some() && core.cosmetics.is_some());
            }
        }
    }

    #[test]
    fn crossed_reload_replies_drain_once_and_return_to_idle() {
        for library_first in [true, false] {
            let mut core = appearance_ready();
            assert_eq!(core.pending_reads(), RefreshReads::default());
            let original = core.begin_refresh();
            let queued = core.begin_refresh();
            assert!(queued.library.is_none() && queued.cosmetics.is_none());
            let old_library = original.library.unwrap();
            let old_cosmetics = original.cosmetics.unwrap();
            let mut fresh_library = None;
            let mut fresh_cosmetics = None;
            for is_library in [library_first, !library_first] {
                let settlement = if is_library {
                    core.library_settled(old_library, Ok(vec![]))
                } else {
                    core.cosmetics_settled(old_cosmetics.clone(), Ok(cosmetics(None, vec![])))
                };
                assert!(!settlement.applied && settlement.redispatch);
                let reads = core.pending_reads();
                if is_library {
                    fresh_library = reads.library;
                    assert!(reads.cosmetics.is_none());
                } else {
                    fresh_cosmetics = reads.cosmetics;
                    assert!(reads.library.is_none());
                }
            }
            let settled = core.library_settled(fresh_library.unwrap(), Ok(vec![]));
            assert!(settled.applied && !settled.redispatch);
            assert_eq!(core.pending_reads(), RefreshReads::default());
            let settled =
                core.cosmetics_settled(fresh_cosmetics.unwrap(), Ok(cosmetics(None, vec![])));
            assert!(settled.applied && !settled.redispatch);
            assert_eq!(core.pending_reads(), RefreshReads::default());
            assert!(core.library_read.is_none() && core.cosmetics_read.is_none());
        }
    }

    #[test]
    fn stale_library_reads_cannot_overwrite_acknowledged_writes() {
        let mut core = library_ready();
        let stale = core.request_library().expect("read dispatched");
        let start = core
            .request_write(Write::Favorite {
                preset: "a".into(),
                value: true,
            })
            .expect("write accepted");
        assert!(core.busy());

        // The pre-write read replies with the old list: never applied.
        let settlement =
            core.library_settled(stale, Ok(vec![preset("a", "Alpha", "classic", false)]));
        assert!(!settlement.applied);

        // The write acknowledges the fresh list.
        let outcome = core
            .write_settled(
                start,
                WriteResult::Library(vec![preset("a", "Alpha", "classic", true)]),
            )
            .expect("write settles");
        assert!(outcome.ok && outcome.changed);
        assert!(
            core.library
                .as_ref()
                .is_some_and(|list| list.iter().any(|p| p.id == "a" && p.favorite))
        );

        // A duplicate terminal and a late stale read are both ignored.
        assert!(core.write_settled(start, WriteResult::Cancelled).is_none());
        assert!(
            !core
                .library_settled(stale, Ok(vec![preset("a", "Alpha", "classic", false)]),)
                .applied
        );
    }

    #[test]
    fn account_change_invalidates_remote_state_and_drifted_writes_settle_terminally() {
        let mut core = appearance_ready();
        // A remote write starts under the exact frozen identity, and a
        // cosmetics read is in flight.
        let read = core.request_cosmetics().expect("cosmetics read");
        let start = core
            .request_write(Write::SelectCape {
                account: "acc-1".into(),
                cape: "vanilla".into(),
            })
            .expect("write starts against the frozen identity");
        assert!(core.busy());

        // The account changed: the in-flight reply is rejected, reviews and
        // the inspected preview are invalidated synchronously.
        assert!(core.set_account(Some(signed_in("acc-2", "Bee"))));
        let settlement = core.cosmetics_settled(read, Ok(cosmetics(None, vec![])));
        assert!(!settlement.applied);
        assert!(core.cosmetics.is_none());

        // The pending write settles terminally: busy clears exactly once, the
        // snapshot is never applied under the new identity, and the possible
        // persisted side effect demands a reload.
        let outcome = core
            .write_settled(start, WriteResult::Cosmetics(cosmetics(None, vec![])))
            .expect("terminal settlement");
        assert!(outcome.ok && outcome.changed && outcome.reload_cosmetics);
        assert!(!outcome.preview_changed);
        assert!(core.cosmetics.is_none());
        assert!(!core.busy());

        // A drifted failure is quiet but still terminal. The new identity
        // reloaded its cosmetics first, so the write target is valid.
        let read = core.request_cosmetics().expect("reload read");
        assert!(
            core.cosmetics_settled(
                read,
                Ok(cosmetics(
                    None,
                    vec![cape("migrator", "Migrator", true, false)]
                )),
            )
            .applied
        );
        let start = core
            .request_write(Write::DisableCape {
                account: "acc-2".into(),
            })
            .expect("write starts");
        core.set_account(None);
        let outcome = core
            .write_settled(
                start,
                WriteResult::Failure(api::ApiError::new("fixture", "late failure")),
            )
            .expect("terminal settlement");
        assert!(!outcome.ok && !outcome.changed);
        assert!(!core.busy());
        assert!(core.error.is_none());
    }

    #[test]
    fn malformed_or_stale_targets_never_start_writes() {
        let mut core = appearance_ready();
        assert!(
            core.request_write(Write::Import {
                name: "   ".into(),
                model: "classic".into(),
            })
            .is_none()
        );
        assert!(
            core.request_write(Write::Import {
                name: "Named".into(),
                model: "square".into(),
            })
            .is_none()
        );
        assert!(
            core.request_write(Write::Favorite {
                preset: "missing".into(),
                value: true,
            })
            .is_none()
        );
        // A favorite write with the current value is a no-op, not a write.
        assert!(
            core.request_write(Write::Favorite {
                preset: "b".into(),
                value: true,
            })
            .is_none()
        );
        assert!(
            core.request_write(Write::Rename {
                preset: "a".into(),
                name: "  ".into(),
            })
            .is_none()
        );
        assert!(
            core.request_write(Write::Remove {
                preset: "missing".into(),
            })
            .is_none()
        );
        assert!(
            core.request_write(Write::SelectCape {
                account: "acc-1".into(),
                cape: "not-owned".into(),
            })
            .is_none()
        );
        assert!(
            core.request_write(Write::DisableCape {
                account: "other".into(),
            })
            .is_none()
        );

        // Save requires an actual current skin on the exact identity.
        core.cosmetics = Some(cosmetics(
            None,
            vec![cape("migrator", "Migrator", true, false)],
        ));
        assert!(
            core.request_write(Write::SaveCurrent {
                account: "acc-1".into(),
            })
            .is_none()
        );

        // One write at a time vetoes everything else.
        let start = core
            .request_write(Write::Favorite {
                preset: "a".into(),
                value: true,
            })
            .expect("first write");
        assert!(
            core.request_write(Write::Favorite {
                preset: "b".into(),
                value: false,
            })
            .is_none()
        );
        assert!(core.request_library().is_none());
        assert!(core.library_pending);
        let outcome = core
            .write_settled(
                start,
                WriteResult::Library(vec![preset("a", "Alpha", "classic", true)]),
            )
            .unwrap();
        assert!(outcome.reload_library || outcome.changed);
    }

    #[test]
    fn local_inspection_survives_an_arriving_cosmetics_refresh() {
        let mut core = appearance_ready();
        let read = core.request_cosmetics().expect("read dispatched");

        // The user deliberately inspects a library skin while the refresh is
        // in flight: the arriving snapshot must not steal the preview.
        let (serial, id, _) = core.preview_preset("b").expect("preset listed");
        let arrival = core.preview_arrived(serial, &id, Ok(avatar(9)));
        assert!(arrival.applied && arrival.preview_changed);

        let settlement = core.cosmetics_settled(read, Ok(cosmetics(None, vec![])));
        assert!(settlement.applied);
        assert!(!settlement.preview_changed);
        assert!(matches!(core.choice, SkinChoice::Preset(_)));
        assert!(core.preview_appearance().0.is_some());

        // Without an inspection the same refresh resets the preview.
        let read = core.request_cosmetics().expect("next read");
        let settlement = core.cosmetics_settled(read, Ok(cosmetics(None, vec![])));
        assert!(settlement.applied);
        assert!(settlement.preview_changed);
        assert!(core.preview_appearance().0.is_none());

        // An explicit refresh resets synchronously and cancels preset loads.
        let (serial, id, _) = core.preview_preset("a").expect("preset listed");
        let _ = core.begin_refresh();
        let arrival = core.preview_arrived(serial, &id, Ok(avatar(3)));
        assert!(!arrival.applied);
    }

    #[test]
    fn duplicate_and_stale_write_terminals_are_ignored() {
        let mut core = appearance_ready();
        let first = core
            .request_write(Write::Rename {
                preset: "a".into(),
                name: "Renamed".into(),
            })
            .expect("first write");
        assert!(
            core.write_settled(
                first,
                WriteResult::Library(vec![preset("a", "Renamed", "classic", false)])
            )
            .is_some()
        );
        // Duplicate terminal after settlement.
        assert!(
            core.write_settled(
                first,
                WriteResult::Library(vec![preset("a", "Evil", "classic", false)])
            )
            .is_none()
        );
        assert!(
            core.library
                .as_ref()
                .is_some_and(|list| list[0].name == "Renamed")
        );

        // A terminal carrying an older tag cannot settle a newer write.
        let second = core
            .request_write(Write::Remove { preset: "a".into() })
            .expect("second write");
        assert!(core.write_settled(first, WriteResult::Cancelled).is_none());
        assert!(core.busy());
        assert!(
            core.write_settled(second, WriteResult::LibraryReload)
                .is_some()
        );
        assert!(!core.busy());
    }

    #[test]
    fn signed_out_local_actions_work_but_remote_calls_are_refused() {
        let mut core = library_ready();
        assert!(core.account.is_none());
        assert!(core.request_cosmetics().is_none());
        assert!(
            core.request_write(Write::SaveCurrent {
                account: "acc-1".into()
            })
            .is_none()
        );
        assert!(
            core.request_write(Write::Apply {
                account: "acc-1".into(),
                preset: "a".into(),
                model: "classic".into(),
            })
            .is_none()
        );
        assert!(
            core.request_write(Write::SelectCape {
                account: "acc-1".into(),
                cape: "migrator".into(),
            })
            .is_none()
        );
        assert!(
            core.request_write(Write::DisableCape {
                account: "acc-1".into()
            })
            .is_none()
        );
        assert!(!core.open_apply_review("a"));

        // The local library keeps working.
        assert!(
            core.request_write(Write::Import {
                name: "Imported".into(),
                model: "slim".into(),
            })
            .is_some()
        );
        let mut core = library_ready();
        assert!(core.preview_preset("a").is_some());
        assert!(core.preview_current() || core.preview_appearance().0.is_none());
        assert!(core.open_rename("a"));
        assert!(core.open_remove_review("a"));
    }

    #[test]
    fn apply_review_freezes_exact_identity_and_model() {
        let mut core = appearance_ready();
        assert!(core.open_apply_review("b"));
        let review = core.apply_review.clone().expect("frozen review");
        assert_eq!(review.account, "acc-1");
        assert_eq!(review.name, "Beta");
        assert_eq!(review.model, "slim");

        // A rename that changes the frozen name rejects the confirmation.
        core.library = Some(vec![
            preset("a", "Alpha", "classic", false),
            preset("b", "Beta II", "slim", true),
        ]);
        assert!(core.confirm_apply(&review).is_none());
        assert_eq!(core.error.as_deref(), Some(STALE_REVIEW));

        // An account change rejects the frozen identity (and closed the
        // review synchronously).
        core = appearance_ready();
        assert!(core.open_apply_review("b"));
        let review = core.apply_review.clone().unwrap();
        core.set_account(Some(signed_in("acc-2", "Bee")));
        assert!(!core.apply_open);
        assert!(core.confirm_apply(&review).is_none());

        // The exact frozen review applies the frozen model; the persisted
        // library model is never implicitly rewritten.
        core = appearance_ready();
        assert!(core.open_apply_review("b"));
        let review = core.apply_review.clone().unwrap();
        let write = core.confirm_apply(&review).expect("confirmed");
        assert_eq!(
            write,
            Write::Apply {
                account: "acc-1".into(),
                preset: "b".into(),
                model: "slim".into(),
            }
        );
        assert_eq!(
            core.preset("b").map(|p| p.model.clone()),
            Some("slim".into())
        );

        // A stale apply target cannot start the write at all.
        core = appearance_ready();
        assert!(
            core.request_write(Write::Apply {
                account: "acc-1".into(),
                preset: "gone".into(),
                model: "classic".into(),
            })
            .is_none()
        );
    }

    #[test]
    fn library_apply_availability_and_activation_follow_account_and_busy_state() {
        let mut core = library_ready();
        assert!(core.apply_unavailable(false).unwrap().contains("Sign in"));
        assert_eq!(core.apply_label(), "Apply");
        assert!(!core.activate_apply("a", false));
        core.set_account(Some(Identity {
            status: "reauthenticationRequired".into(),
            ..signed_in("acc", "Ann")
        }));
        assert!(
            core.apply_unavailable(false)
                .unwrap()
                .contains("Sign in again")
        );
        assert!(!core.activate_apply("a", false));
        core.set_account(Some(signed_in("acc", "Ann")));
        assert_eq!(core.apply_label(), "Apply to Ann");
        assert!(core.apply_unavailable(false).is_none());
        assert!(!core.activate_apply("a", true));
        assert!(!core.activate_apply("missing", false));
        let tag = core
            .request_write(Write::Favorite {
                preset: "a".into(),
                value: true,
            })
            .unwrap();
        assert!(core.apply_unavailable(false).unwrap().contains("Wait"));
        assert!(!core.activate_apply("a", false));
        core.write_settled(tag, WriteResult::Cancelled).unwrap();
        assert!(core.activate_apply("b", false));
        assert_eq!(
            core.apply_review,
            Some(ApplyReview {
                account: "acc".into(),
                account_name: "Ann".into(),
                preset: "b".into(),
                name: "Beta".into(),
                model: "slim".into()
            })
        );
        assert!(core.apply_open);
    }

    #[test]
    fn applied_state_uses_current_content_and_model_never_the_preview_choice() {
        let mut core = appearance_ready();
        core.library.as_mut().unwrap()[0].sha256 = "a".repeat(64);
        core.cosmetics
            .as_mut()
            .unwrap()
            .current_skin
            .as_mut()
            .unwrap()
            .sha256 = "a".repeat(64);
        assert!(core.applied(core.preset("a").unwrap()));
        assert!(!core.activate_apply("a", false));
        core.preview_preset("b").unwrap();
        assert!(core.applied(core.preset("a").unwrap()));
        assert!(!core.applied(core.preset("b").unwrap()));
        core.library.as_mut().unwrap()[0].model = "slim".into();
        assert!(!core.applied(core.preset("a").unwrap()));
        core.library.as_mut().unwrap()[0].model = "classic".into();
        core.cosmetics
            .as_mut()
            .unwrap()
            .current_skin
            .as_mut()
            .unwrap()
            .sha256
            .clear();
        core.library.as_mut().unwrap()[0].sha256.clear();
        assert!(!core.applied(core.preset("a").unwrap()));
        core.account = None;
        assert!(!core.applied(core.preset("a").unwrap()));
    }

    #[test]
    fn library_apply_success_returns_to_current_preview_and_reports_success() {
        let mut core = appearance_ready();
        core.library.as_mut().unwrap()[1].sha256 = "b".repeat(64);
        let (serial, id, _) = core.preview_preset("a").unwrap();
        assert!(core.activate_apply("b", false));
        let review = core.apply_review.clone().unwrap();
        let write = core.confirm_apply(&review).unwrap();
        let tag = core.request_write(write).unwrap();
        let mut skin = avatar(9);
        skin.model = "slim".into();
        skin.sha256 = "b".repeat(64);
        let mut snapshot = cosmetics(Some(skin), vec![cape("migrator", "Migrator", true, true)]);
        snapshot.current_skin_model = Some("slim".into());
        let outcome = core
            .write_settled(tag, WriteResult::Cosmetics(snapshot))
            .unwrap();
        assert!(outcome.ok && outcome.changed && outcome.preview_changed);
        assert!(!core.busy());
        assert_eq!(core.choice, SkinChoice::Current);
        assert_eq!(core.skin.as_ref().unwrap().model, "slim");
        assert_eq!(core.skin.as_ref().unwrap().skin_rgba[0], 9);
        assert_eq!(core.cape_choice.as_deref(), Some("migrator"));
        assert!(core.applied(core.preset("b").unwrap()));
        assert_eq!(
            core.notice.as_deref(),
            Some("Skin applied to your Minecraft account.")
        );
        assert!(!core.preview_arrived(serial, &id, Ok(avatar(3))).applied);
        assert!(core.write_settled(tag, WriteResult::Cancelled).is_none());
    }

    #[test]
    fn library_apply_errors_keep_the_current_skin_and_use_the_existing_error_surface() {
        for (code, message) in [
            ("skin_rate_limited", "Try again in a minute."),
            ("minecraft_profile_missing", "Minecraft profile missing."),
        ] {
            let mut core = appearance_ready();
            assert!(core.activate_apply("b", false));
            let review = core.apply_review.clone().unwrap();
            let write = core.confirm_apply(&review).unwrap();
            let tag = core.request_write(write).unwrap();
            let outcome = core
                .write_settled(tag, WriteResult::Failure(api::ApiError::new(code, message)))
                .unwrap();
            assert!(!outcome.ok && !outcome.changed);
            assert_eq!(core.error.as_deref(), Some(message));
            assert!(core.notice.is_none());
            assert_eq!(
                core.cosmetics
                    .as_ref()
                    .unwrap()
                    .current_skin
                    .as_ref()
                    .unwrap()
                    .skin_rgba[0],
                1
            );
        }
    }

    #[test]
    fn remove_review_freezes_identity_and_rejects_drift() {
        let mut core = library_ready();
        assert!(core.open_remove_review("a"));
        let review = core.remove_review.clone().expect("frozen review");
        assert_eq!(review.name, "Alpha");

        // A rename underneath the open review rejects the removal.
        core.library = Some(vec![preset("a", "Alpha II", "classic", false)]);
        assert!(core.confirm_remove(&review).is_none());
        assert_eq!(core.error.as_deref(), Some(STALE_REVIEW));

        // The exact review confirms the frozen preset.
        core = library_ready();
        assert!(core.open_remove_review("a"));
        let review = core.remove_review.clone().unwrap();
        assert_eq!(
            core.confirm_remove(&review),
            Some(Write::Remove { preset: "a".into() })
        );
        // A queued confirm cannot remove twice or a dismissed target.
        assert!(core.confirm_remove(&review).is_none());
        assert!(core.open_remove_review("b"));
        assert!(core.confirm_remove(&review).is_none());
    }

    #[test]
    fn favorites_filter_and_thumbnail_guards_prevent_removed_entries_returning() {
        let mut core = library_ready();
        core.favorites_only = true;
        assert_eq!(core.filtered_library().len(), 1);
        assert_eq!(core.filtered_library()[0].id, "b");
        core.favorites_only = false;
        assert_eq!(core.filtered_library().len(), 2);

        let serial = core.library_serial;
        assert!(core.thumbnail_accept(serial, "a"));
        assert!(!core.thumbnail_accept(serial, "missing"));

        // The removal write is acknowledged and the list is re-read; the
        // fresh revision drops the removed entry so its late thumbnail can
        // never return.
        let start = core
            .request_write(Write::Remove { preset: "a".into() })
            .unwrap();
        assert!(
            core.write_settled(start, WriteResult::LibraryReload)
                .is_some()
        );
        let read = core.request_library().expect("reload read");
        assert!(
            core.library_settled(read, Ok(vec![preset("b", "Beta", "slim", true)]))
                .applied
        );
        assert!(!core.thumbnail_accept(serial, "a"));
        assert!(core.thumbnail_accept(core.library_serial, "b"));

        // A replacement list also invalidates the previous serial.
        let read = core.request_library().unwrap();
        core.library_settled(read, Ok(vec![preset("b", "Beta II", "slim", false)]))
            .applied;
        assert!(!core.thumbnail_accept(serial, "b"));
    }

    #[test]
    fn preview_selections_change_the_appearance_only_when_it_really_changes() {
        let mut core = appearance_ready();
        // Arriving at the current appearance from the initial neutral state
        // is a real change; repeating it is not.
        assert!(core.preview_current());
        assert!(!core.preview_current());

        // Inspecting the worn cape requests the rear view once.
        assert!(core.inspect_cape("migrator").expect("owned cape"));
        assert!(!core.inspect_cape("migrator").expect("owned cape"));
        let (skin, cape, rear) = core.preview_appearance();
        assert!(skin.is_some() && cape.is_some() && rear);

        // Inspecting the same cape again after returning front changes rear.
        assert!(core.preview_current());
        assert!(core.inspect_cape("migrator").is_some());
        assert!(core.inspect_cape("not-owned").is_none());

        // A preset avatar load replaces the skin; a stale serial is ignored.
        let (serial, id, _) = core.preview_preset("a").expect("listed");
        let other = core.preview_preset("b").expect("listed");
        let arrival = core.preview_arrived(serial, &id, Ok(avatar(5)));
        assert!(!arrival.applied, "an older selection never applies");
        let arrival = core.preview_arrived(other.0, &other.1, Ok(avatar(6)));
        assert!(arrival.applied && arrival.preview_changed);
        let (_, cape, rear) = core.preview_appearance();
        assert!(
            cape.is_some() && !rear,
            "the inspected cape survives while a skin preview returns to the front"
        );
    }

    #[test]
    fn refresh_resets_preview_and_rejects_stale_reads() {
        let mut core = appearance_ready();
        let old_cosmetics = core.request_cosmetics().unwrap();
        let old_library = core.request_library().unwrap();
        let reads = core.begin_refresh();
        assert!(reads.preview_changed || core.preview_appearance().0.is_none());
        // The pre-refresh reads are still in flight: the refresh pends until
        // their stale replies are rejected.
        assert!(reads.library.is_none() && reads.cosmetics.is_none());

        // Replies from before the refresh are rejected and re-armed.
        let settlement = core.cosmetics_settled(old_cosmetics, Ok(cosmetics(None, vec![])));
        assert!(!settlement.applied && settlement.redispatch);
        let settlement =
            core.library_settled(old_library, Ok(vec![preset("z", "Z", "classic", false)]));
        assert!(!settlement.applied && settlement.redispatch);

        // The fresh reads apply and reset the preview to the new snapshot.
        let fresh_cosmetics = core.request_cosmetics().unwrap();
        let settlement = core.cosmetics_settled(
            fresh_cosmetics,
            Ok(cosmetics(
                Some(avatar(2)),
                vec![cape("vanilla", "Vanilla", true, true)],
            )),
        );
        assert!(settlement.applied);
        let (skin, cape, rear) = core.preview_appearance();
        assert!(skin.is_some() && cape.is_some() && !rear);

        // A status change also invalidates through the epoch.
        let mut drifted = core.account.clone().unwrap();
        drifted.status = "reauthenticationRequired".into();
        assert!(core.set_account(Some(drifted)));
        assert!(core.cosmetics.is_none());
        assert!(core.request_cosmetics().is_none());
    }

    #[test]
    fn rename_review_confirms_the_edited_name() {
        let mut core = library_ready();
        assert!(core.open_rename("a"));
        let review = core.rename_review.clone().unwrap();
        assert!(core.confirm_rename(&review, "  ").is_none());
        let write = core
            .confirm_rename(&review, "  Alpha II  ")
            .expect("trimmed");
        assert_eq!(
            write,
            Write::Rename {
                preset: "a".into(),
                name: "Alpha II".into(),
            }
        );
        // A queued confirm after a dismissal cannot rename.
        assert!(core.confirm_rename(&review, "Again").is_none());
    }
}
