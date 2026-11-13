//! Sync preferences and conflict resolution for the GPUI host.
//!
//! The preserved Slint host (`native/src/syncing.rs`) stays the behavioral
//! reference: participant checkboxes keyed by stable instance ids (saved
//! participants missing from the registry stay visible), seven sync
//! categories, the `;` path vocabulary, and per-participant overrides with
//! the same clearing rule — an override disappears only when it carries no
//! information, and removing a participant explicitly clears its override.
//!
//! This page batches every edit into one typed draft that is saved through
//! `api::sync_save` only on an explicit Apply. The acknowledged policy stays
//! the baseline; a failed write keeps the draft and error for retry; a
//! malformed policy read never synthesizes a writable policy; and a late
//! policy read never overwrites a dirty draft. Conflicts are read-only until
//! the user opens an explicit review that freezes the exact conflict
//! identity; stale results are rejected and at most one resolve runs at a
//! time. Server-list conflicts stay deferred in the game and are never
//! resolved here.
//!
//! Deliberate limits, matching the standing contract: no `initialize_sync`
//! call, no minute worker, no polling — `sync_save` itself triggers the core
//! reconcile, and `instance-sync-state` host events drive status and conflict
//! refreshes. Nothing here touches instance files or invents backup receipts.

#![allow(dead_code)]

use std::cell::Cell;
use std::rc::Rc;

use aurora_launcher_lib::host::Host;
use gpui::{
    Bounds, Context, Div, Entity, EventEmitter, FocusHandle, Pixels, Render, SharedString,
    Subscription, Window, div, prelude::*, px,
};
use gpui_kit::base::{
    Dialog,
    input::{InputEvent, InputState},
};
use tokio::runtime::Handle;

use crate::api::{self, ApiResult};
use crate::backend::guarded;
use crate::controls::{
    AuroraButton, AuroraInput, AuroraSelect, AuroraToggle, ButtonSize, ButtonVariant,
};
use crate::theme::theme;
use crate::ui;

pub enum SyncingEvent {
    Changed,
}
impl EventEmitter<SyncingEvent> for Syncing {}

const CATEGORIES: [(&str, &str); 7] = [
    ("settings", "Game settings"),
    ("controls", "Controls"),
    ("resourcePacks", "Resource packs"),
    ("shaderPacks", "Shader packs"),
    ("screenshots", "Screenshots"),
    ("configuration", "Selected configuration"),
    ("servers", "Server list"),
];
const OVERRIDE_MODES: [&str; 3] = ["Inherit", "Include", "Exclude"];

type BoundsCell = Rc<Cell<Option<Bounds<Pixels>>>>;

// ---------------------------------------------------------------------------
// Pure policy editing (ported from native/src/syncing.rs)
// ---------------------------------------------------------------------------

/// Splits the existing `;` path vocabulary. Empty entries are dropped.
fn split_paths(text: &str) -> Vec<String> {
    text.split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Checks or unchecks one participant by stable id. Removing a participant
/// explicitly clears its override, exactly like the Slint host; adding one
/// creates no override, so it inherits.
fn set_participant(policy: &mut api::SyncPolicy, id: &str, checked: bool) {
    policy.instances.retain(|current| current != id);
    if checked {
        policy.instances.push(id.to_owned());
    } else {
        policy.overrides.remove(id);
    }
}

/// Checks or unchecks one category by its existing id.
fn set_category(policy: &mut api::SyncPolicy, id: &str, checked: bool) {
    policy.categories.retain(|current| current != id);
    if checked {
        policy.categories.push(id.to_owned());
    }
}

/// Edits one participant override. Only participating instances can carry an
/// override, and an override that carries no information (not disabled, no
/// category choices, no exclusions) is removed again — the native clearing
/// behavior. Unrelated overrides are never touched.
fn edit_override(
    policy: &mut api::SyncPolicy,
    id: &str,
    edit: impl FnOnce(&mut api::SyncOverride),
) {
    if !policy.instances.iter().any(|current| current == id) {
        return;
    }
    let value = policy.overrides.entry(id.to_owned()).or_default();
    edit(value);
    if !value.disabled && value.categories.is_empty() && value.exclusions.is_empty() {
        policy.overrides.remove(id);
    }
}

/// `0` inherit, `1` include, `2` exclude — the saved choice per category.
fn override_mode(value: Option<&api::SyncOverride>, category: &str) -> usize {
    match value.and_then(|value| value.categories.get(category)) {
        Some(true) => 1,
        Some(false) => 2,
        None => 0,
    }
}

/// Applies one mode choice. Inherit removes the entry again.
fn set_override_mode(value: &mut api::SyncOverride, category: &str, mode: usize) {
    value.categories.remove(category);
    if mode == 1 || mode == 2 {
        value.categories.insert(category.to_owned(), mode == 1);
    }
}

struct ParticipantRow {
    id: String,
    name: String,
    detail: String,
    checked: bool,
    /// Whether the registry still knows this instance.
    available: bool,
}

/// Participants keyed by stable instance id: the registry first, then saved
/// participants the registry no longer knows, shown as unavailable.
fn participant_rows(
    policy: &api::SyncPolicy,
    state: Option<&api::LauncherState>,
) -> Vec<ParticipantRow> {
    let mut rows = Vec::new();
    if let Some(state) = state {
        for instance in &state.instances {
            rows.push(ParticipantRow {
                id: instance.id.clone(),
                name: instance.display_name.clone(),
                detail: instance.state.clone(),
                checked: policy
                    .instances
                    .iter()
                    .any(|current| current == &instance.id),
                available: true,
            });
        }
    }
    for id in &policy.instances {
        if rows.iter().any(|row| row.id == *id) {
            continue;
        }
        rows.push(ParticipantRow {
            id: id.clone(),
            name: "Unavailable instance".into(),
            detail: format!("Saved participant {id} is no longer in the registry"),
            checked: true,
            available: false,
        });
    }
    rows
}

fn instance_name(state: Option<&api::LauncherState>, id: &str) -> String {
    state
        .and_then(|state| state.instances.iter().find(|i| i.id == id))
        .map(|instance| instance.display_name.clone())
        .unwrap_or_else(|| "Unavailable".into())
}

// ---------------------------------------------------------------------------
// Policy baseline/draft logic
// ---------------------------------------------------------------------------

/// The acknowledged policy plus the working draft, as pure logic. Reads only
/// ever replace the acknowledged state; a save failure keeps the draft.
#[derive(Default)]
struct PolicyState {
    guard: PolicyGuard,
    baseline: Option<api::SyncPolicy>,
    draft: Option<api::SyncPolicy>,
    read_valid: bool,
    dirty: bool,
    error: Option<String>,
}
impl PolicyState {
    fn begin_read(&mut self, writing: bool) -> Option<PolicyGuard> {
        if self.dirty || writing {
            return None;
        }
        self.guard.serial = self.guard.serial.wrapping_add(1);
        Some(self.guard)
    }

    fn complete_read(&mut self, guard: PolicyGuard, result: ApiResult<api::SyncPolicy>) -> bool {
        if guard != self.guard || self.dirty {
            return false;
        }
        self.guard.serial = self.guard.serial.wrapping_add(1);
        self.loaded(true, result);
        true
    }

    fn invalidate_reads(&mut self) {
        self.guard.epoch = self.guard.epoch.wrapping_add(1);
    }

    fn edit(&mut self, edit: impl FnOnce(&mut api::SyncPolicy)) {
        if !self.writable() {
            return;
        }
        self.invalidate_reads();
        edit(self.draft.as_mut().unwrap());
        self.dirty = true;
    }

    fn begin_write(&mut self) -> Option<api::SyncPolicy> {
        if !self.can_apply() {
            return None;
        }
        self.invalidate_reads();
        self.draft.clone()
    }

    fn revert(&mut self) -> bool {
        let Some(baseline) = self.baseline.clone() else {
            return false;
        };
        self.invalidate_reads();
        self.draft = Some(baseline);
        self.dirty = false;
        if self.read_valid {
            self.error = None;
        }
        true
    }

    /// Applies a read result. When a dirty draft or an in-flight write
    /// exists (`allow` false) the result is dropped entirely. A malformed
    /// read (`Err`) never synthesizes a writable policy: without an
    /// acknowledged baseline nothing can be saved.
    fn loaded(&mut self, allow: bool, result: ApiResult<api::SyncPolicy>) {
        if !allow {
            return;
        }
        match result {
            Ok(policy) => {
                self.read_valid = true;
                self.baseline = Some(policy.clone());
                self.draft = Some(policy);
                self.dirty = false;
                self.error = None;
            }
            Err(error) => {
                self.read_valid = false;
                self.error = Some(error.message);
            }
        }
    }

    /// Completes a write. The acknowledged response becomes the baseline and
    /// the draft; a failure keeps the draft and error so Apply can retry.
    /// Returns whether the change was acknowledged.
    fn saved(&mut self, result: ApiResult<api::SyncPolicy>) -> bool {
        self.invalidate_reads();
        match result {
            Ok(policy) => {
                self.baseline = Some(policy.clone());
                self.draft = Some(policy);
                self.read_valid = true;
                self.dirty = false;
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error.message);
                false
            }
        }
    }

    fn writable(&self) -> bool {
        self.read_valid && self.baseline.is_some() && self.draft.is_some()
    }

    fn can_apply(&self) -> bool {
        self.dirty && self.writable()
    }
}

// ---------------------------------------------------------------------------
// Conflict review and generation guards
// ---------------------------------------------------------------------------

/// Exact conflict identity frozen when the review opens. Resolution only
/// proceeds through this frozen copy, never through a row index.
#[derive(Clone)]
struct ConflictReview {
    conflict: api::SyncConflict,
    source: String,
    destination: String,
}

/// Identifies the active resolve. Its revision invalidates preceding reads;
/// a completion must match this exact operation before clearing busy state.
#[derive(Clone, PartialEq)]
struct ResolveGuard {
    id: String,
    revision: u64,
}

/// Reads started before a resolution cannot restore its old conflict list.
/// While resolving, refresh requests coalesce into the mandatory settle read.
#[derive(Default)]
struct ConflictRequests {
    serial: u64,
    pending: Option<ResolveGuard>,
}
impl ConflictRequests {
    fn begin_read(&mut self) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        self.serial = self.serial.wrapping_add(1);
        Some(self.serial)
    }

    fn accepts_read(&self, serial: u64) -> bool {
        self.pending.is_none() && self.serial == serial
    }

    fn begin_resolve(&mut self, id: String) -> Option<ResolveGuard> {
        if self.pending.is_some() {
            return None;
        }
        self.serial = self.serial.wrapping_add(1);
        let guard = ResolveGuard {
            id,
            revision: self.serial,
        };
        self.pending = Some(guard.clone());
        Some(guard)
    }

    fn finish_resolve(&mut self, guard: &ResolveGuard) -> bool {
        if self.pending.as_ref() != Some(guard) {
            return false;
        }
        self.pending = None;
        self.serial = self.serial.wrapping_add(1);
        true
    }
}

#[derive(Default)]
struct StatusState {
    serial: u64,
    value: Option<api::SyncStatus>,
}
impl StatusState {
    fn begin_read(&mut self) -> u64 {
        self.serial = self.serial.wrapping_add(1);
        self.serial
    }

    fn apply(&mut self, status: api::SyncStatus) -> bool {
        let finished = self.value.as_ref().is_some_and(|s| s.running) && !status.running;
        self.value = Some(status);
        finished
    }

    fn event(&mut self, status: api::SyncStatus) -> bool {
        self.begin_read(); // live events supersede outstanding snapshot reads
        self.apply(status)
    }

    fn complete_read(&mut self, serial: u64, status: api::SyncStatus) -> bool {
        if serial != self.serial {
            return false;
        }
        self.begin_read();
        self.apply(status)
    }
}

fn conflict_identity_matches(a: &api::SyncConflict, b: &api::SyncConflict) -> bool {
    a.id == b.id
        && a.source_id == b.source_id
        && a.destination_id == b.destination_id
        && a.category == b.category
        && a.path == b.path
        && a.key == b.key
}

fn conflict_resolvable(conflict: &api::SyncConflict) -> bool {
    conflict.category != "servers"
}

fn conflict_label(conflict: &api::SyncConflict) -> String {
    match conflict.key.as_ref() {
        Some(key) => format!("{} · {key}", conflict.path),
        None => conflict.path.clone(),
    }
}

// ---------------------------------------------------------------------------
// Async replies
// ---------------------------------------------------------------------------

/// Guards a policy read against superseding acknowledged changes.
#[derive(Clone, Copy, Default, PartialEq)]
struct PolicyGuard {
    serial: u64,
    epoch: u64,
}

enum Reply {
    Policy(PolicyGuard, ApiResult<api::SyncPolicy>),
    Saved(u64, ApiResult<api::SyncPolicy>),
    Conflicts(u64, ApiResult<Vec<api::SyncConflict>>),
    Instances(u64, ApiResult<api::LauncherState>),
    Status(u64, ApiResult<api::SyncStatus>),
    Resolved(ResolveGuard, ApiResult<serde_json::Value>),
}

fn error(error: api::ApiError) -> String {
    format!("{}: {}", error.code, error.message)
}

fn load_input(
    field: &Entity<InputState>,
    text: String,
    window: &mut Window,
    cx: &mut Context<Syncing>,
) {
    field.update(cx, |field, cx| field.set_value(text, window, cx));
}

// ---------------------------------------------------------------------------
// Syncing view
// ---------------------------------------------------------------------------

pub struct Syncing {
    section: usize,
    host: Option<Host>,
    runtime: Handle,
    tx: async_channel::Sender<Reply>,
    // Generation guards
    write_serial: u64,
    instances_serial: u64,
    conflict_requests: ConflictRequests,
    // Policy
    policy: PolicyState,
    write_busy: bool,
    // Reference data and live status
    instances: Option<api::LauncherState>,
    status: StatusState,
    conflicts: Vec<api::SyncConflict>,
    conflicts_loading: bool,
    // Conflict review
    review: Option<ConflictReview>,
    // Draft editing
    selected: Option<String>,
    exclusions: Entity<InputState>,
    paths: Entity<InputState>,
    override_exclusions: Entity<InputState>,
    input_reload: bool,
    override_loaded_for: Option<String>,
    category_open: [bool; 7],
    category_highlight: [usize; 7],
    category_focus: [(FocusHandle, FocusHandle); 7],
    category_bounds: [BoundsCell; 7],
    _input_events: Vec<Subscription>,
    message: String,
}

impl Syncing {
    /// Builds the page. With a host it reads the saved policy, the instance
    /// registry, the sync status and pending conflicts; nothing is saved, no
    /// sync starts and nothing resolves. Without a host the page stays in an
    /// unavailable diagnostic state and makes no calls at all.
    pub fn new(
        host: Option<Host>,
        runtime: Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded();
        let mut input = |placeholder: &'static str| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let exclusions = input("Exclusions, separated by ;");
        let paths = input("Selected configuration paths, separated by ;");
        let override_exclusions = input("Exclusions for this instance, separated by ;");
        let mut input_events = Vec::new();
        input_events.push(
            cx.subscribe(&exclusions, |this, input, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) || this.busy() || this.input_reload {
                    return;
                }
                let value = split_paths(&input.read(cx).value().to_string());
                if let Some(draft) = this.policy.draft.as_ref() {
                    if draft.exclusions != value {
                        this.policy.edit(|draft| draft.exclusions = value);
                        cx.notify();
                    }
                }
            }),
        );
        input_events.push(cx.subscribe(&paths, |this, input, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change) || this.busy() || this.input_reload {
                return;
            }
            let value = split_paths(&input.read(cx).value().to_string());
            if let Some(draft) = this.policy.draft.as_ref() {
                if draft.selected_paths != value {
                    this.policy.edit(|draft| draft.selected_paths = value);
                    cx.notify();
                }
            }
        }));
        input_events.push(cx.subscribe(
            &override_exclusions,
            |this, input, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change)
                    || this.busy()
                    || this.input_reload
                    || this.override_loaded_for != this.selected
                {
                    return;
                }
                let Some(selected) = this.selected.clone() else {
                    return;
                };
                let Some(draft) = this.policy.draft.as_ref() else {
                    return;
                };
                let value = split_paths(&input.read(cx).value().to_string());
                let current = draft
                    .overrides
                    .get(&selected)
                    .map(|value| value.exclusions.clone())
                    .unwrap_or_default();
                if current != value {
                    this.policy.edit(|draft| {
                        edit_override(draft, &selected, |entry| entry.exclusions = value)
                    });
                    cx.notify();
                }
            },
        ));
        cx.spawn(async move |weak, cx| {
            while let Ok(reply) = rx.recv().await {
                if weak
                    .update(cx, |this: &mut Self, cx| {
                        this.receive(reply, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let mut this = Self {
            section: 0,
            host,
            runtime,
            tx,
            write_serial: 0,
            instances_serial: 0,
            conflict_requests: ConflictRequests::default(),
            policy: PolicyState::default(),
            write_busy: false,
            instances: None,
            status: StatusState::default(),
            conflicts: Vec::new(),
            conflicts_loading: false,
            review: None,
            selected: None,
            exclusions,
            paths,
            override_exclusions,
            input_reload: false,
            override_loaded_for: None,
            category_open: [false; 7],
            category_highlight: [0; 7],
            category_focus: std::array::from_fn(|_| (cx.focus_handle(), cx.focus_handle())),
            category_bounds: std::array::from_fn(|_| Rc::new(Cell::new(None))),
            _input_events: input_events,
            message: String::new(),
        };
        this.refresh(cx);
        this
    }

    /// Re-reads read-only state. Reads only: no writes, no worker, no
    /// resolves. A policy read is skipped while the draft is dirty or a
    /// write is in flight, so background refresh never replaces user edits.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else {
            self.message =
                "Sync preferences are unavailable in this diagnostic window: there is no launcher host, so nothing is read or changed."
                    .into();
            cx.notify();
            return;
        };
        self.read_status();
        self.read_conflicts();
        let instances_host = host.clone();
        self.instances_serial = self.instances_serial.wrapping_add(1);
        let instances_serial = self.instances_serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::launcher_state(instances_host)).await;
            let _ = tx.send(Reply::Instances(instances_serial, r)).await;
        });
        if let Some(guard) = self.policy.begin_read(self.write_busy) {
            let tx = self.tx.clone();
            self.runtime.spawn(async move {
                let r = guarded(api::sync_policy(host)).await;
                let _ = tx.send(Reply::Policy(guard, r)).await;
            });
        }
        cx.notify();
    }

    fn read_status(&mut self) {
        if self.host.is_none() {
            return;
        }
        let serial = self.status.begin_read();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::sync_status()).await;
            let _ = tx.send(Reply::Status(serial, r)).await;
        });
    }

    fn read_conflicts(&mut self) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(serial) = self.conflict_requests.begin_read() else {
            return;
        };
        self.conflicts_loading = true;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::sync_conflicts(host)).await;
            let _ = tx.send(Reply::Conflicts(serial, r)).await;
        });
    }

    /// Applies the typed draft. One write at a time; the form stays disabled
    /// until the write settles, and a failure keeps the draft for retry.
    fn apply(&mut self, cx: &mut Context<Self>) {
        if self.busy() || !self.policy.can_apply() {
            return;
        }
        let Some(host) = self.host.clone() else {
            return;
        };
        let Some(draft) = self.policy.begin_write() else {
            return;
        };
        self.write_busy = true;
        self.write_serial = self.write_serial.wrapping_add(1);
        let serial = self.write_serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::sync_save(host, draft)).await;
            let _ = tx.send(Reply::Saved(serial, r)).await;
        });
        cx.notify();
    }

    /// Discards the draft back to the acknowledged baseline.
    fn revert(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.input_reload |= self.policy.revert();
        self.category_open = [false; 7];
        cx.notify();
    }

    /// Opens the explicit review for one conflict, freezing its exact
    /// identity together with the peer names at review time.
    fn open_review(&mut self, conflict: api::SyncConflict, cx: &mut Context<Self>) {
        if self.busy() || !conflict_resolvable(&conflict) {
            return;
        }
        if !self
            .conflicts
            .iter()
            .any(|current| conflict_identity_matches(current, &conflict))
        {
            self.message = "This conflict is no longer pending; refresh the list.".into();
            self.review = None;
            cx.notify();
            return;
        }
        self.review = Some(ConflictReview {
            source: instance_name(self.instances.as_ref(), &conflict.source_id),
            destination: instance_name(self.instances.as_ref(), &conflict.destination_id),
            conflict,
        });
        self.message.clear();
        cx.notify();
    }

    /// Resolves the reviewed conflict. One mutation at a time; the exact
    /// frozen identity and the conflict revision at issue are guarded, and
    /// deferred conflicts are never touched.
    fn resolve(&mut self, expected: &api::SyncConflict, use_source: bool, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let (Some(host), Some(review)) = (self.host.clone(), self.review.clone()) else {
            return;
        };
        if !conflict_identity_matches(expected, &review.conflict) {
            return;
        }
        if !self
            .conflicts
            .iter()
            .any(|current| conflict_identity_matches(current, &review.conflict))
        {
            self.message = "This conflict is no longer pending; refresh the list.".into();
            self.review = None;
            cx.notify();
            return;
        }
        let Some(guard) = self
            .conflict_requests
            .begin_resolve(review.conflict.id.clone())
        else {
            return;
        };
        self.conflicts_loading = false;
        let id = review.conflict.id.clone();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::sync_resolve(host, id, use_source)).await;
            let _ = tx.send(Reply::Resolved(guard, r)).await;
        });
        cx.notify();
    }

    fn close_review(&mut self, expected: &api::SyncConflict, cx: &mut Context<Self>) {
        if self.busy()
            || !self
                .review
                .as_ref()
                .is_some_and(|review| conflict_identity_matches(expected, &review.conflict))
        {
            return;
        }
        self.review = None;
        self.message.clear();
        cx.notify();
    }

    fn busy(&self) -> bool {
        self.write_busy || self.conflict_requests.pending.is_some()
    }

    fn set_participant_checked(&mut self, id: String, checked: bool, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if self.policy.writable() {
            self.policy
                .edit(|draft| set_participant(draft, &id, checked));
            if !checked && self.selected.as_deref() == Some(&id) {
                self.selected = None;
                self.category_open = [false; 7];
            }
            cx.notify();
        }
    }

    fn set_category_checked(&mut self, id: &'static str, checked: bool, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if self.policy.writable() {
            self.policy.edit(|draft| set_category(draft, id, checked));
            cx.notify();
        }
    }

    fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if self.policy.writable() {
            self.policy.edit(|draft| draft.enabled = enabled);
            cx.notify();
        }
    }

    fn set_override_disabled(&mut self, id: String, disabled: bool, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if self.policy.writable() {
            self.policy
                .edit(|draft| edit_override(draft, &id, |value| value.disabled = disabled));
            cx.notify();
        }
    }

    fn choose_override_mode(
        &mut self,
        id: String,
        category: &'static str,
        mode: usize,
        cx: &mut Context<Self>,
    ) {
        if self.busy() || self.selected.as_deref() != Some(&id) {
            return;
        }
        if self.policy.writable() {
            self.policy.edit(|draft| {
                edit_override(draft, &id, |value| set_override_mode(value, category, mode))
            });
            cx.notify();
        }
    }

    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Status(serial, result) => {
                if let Ok(status) = result {
                    if self.status.complete_read(serial, status) {
                        self.read_conflicts();
                    }
                }
            }
            Reply::Instances(serial, result) if serial == self.instances_serial => match result {
                Ok(state) => self.instances = Some(state),
                Err(e) => {
                    if self.instances.is_none() {
                        self.message = error(e);
                    }
                }
            },
            Reply::Conflicts(serial, result) => {
                if self.conflict_requests.accepts_read(serial) {
                    self.conflicts_loading = false;
                    match result {
                        Ok(list) => {
                            self.conflicts = list;
                            // A resolved conflict must not keep an open
                            // review once it left the pending list.
                            if let Some(review) = self.review.clone() {
                                if !self.conflicts.iter().any(|current| {
                                    conflict_identity_matches(current, &review.conflict)
                                }) {
                                    self.review = None;
                                }
                            }
                        }
                        Err(e) => self.message = error(e),
                    }
                }
            }
            Reply::Policy(guard, result) => {
                if !self.write_busy && self.policy.complete_read(guard, result) {
                    self.input_reload = true;
                }
            }
            Reply::Saved(serial, result) => {
                if serial != self.write_serial || !self.write_busy {
                    return;
                }
                self.write_busy = false;
                let acknowledged = self.policy.saved(result);
                self.input_reload = true;
                if acknowledged {
                    // sync_save already triggered the core reconcile.
                    cx.emit(SyncingEvent::Changed);
                    self.read_conflicts();
                    self.read_status();
                }
            }
            Reply::Resolved(guard, result) => {
                if !self.conflict_requests.finish_resolve(&guard) {
                    return;
                }
                match result {
                    Ok(_) => {
                        self.message =
                            "Conflict resolved. The replaced version is backed up.".into();
                        self.review = None;
                        cx.emit(SyncingEvent::Changed);
                    }
                    Err(e) => self.message = error(e),
                }
                self.read_conflicts();
                self.read_status();
            }
            Reply::Instances(_, _) => {}
        }
    }

    /// Routes one host event. Unknown names and unparseable payloads are
    /// ignored, never fatal. A running sync finishing reloads the conflict
    /// list; policy draft fields are never replaced by events.
    pub fn on_host_event(
        &mut self,
        name: &str,
        payload: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        if self.host.is_none() || name != "instance-sync-state" {
            return;
        }
        let Ok(status) = serde_json::from_value::<api::SyncStatus>(payload.clone()) else {
            return;
        };
        let finished = self.status.event(status);
        if finished {
            self.read_conflicts();
        }
        cx.notify();
    }

    fn sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.selected.clone();
        let reload_all = std::mem::take(&mut self.input_reload);
        if reload_all {
            if let Some(draft) = self.policy.draft.clone() {
                load_input(&self.exclusions, draft.exclusions.join("; "), window, cx);
                load_input(&self.paths, draft.selected_paths.join("; "), window, cx);
            }
        }
        let reload_override =
            reload_all || self.override_loaded_for.as_deref() != selected.as_deref();
        if reload_override {
            let text = selected
                .as_deref()
                .and_then(|id| self.policy.draft.as_ref().and_then(|d| d.overrides.get(id)))
                .map(|value| value.exclusions.join("; "))
                .unwrap_or_default();
            self.override_loaded_for = selected;
            load_input(&self.override_exclusions, text, window, cx);
        }
    }
}

/// Width of the right-hand control column in settings rows.
const CONTROL_WIDTH: f32 = 220.;

/// One line saying what a sync category covers.
fn category_detail(id: &str) -> &'static str {
    match id {
        "settings" => "Video, audio and gameplay options.",
        "controls" => "Key bindings and mouse settings.",
        "resourcePacks" => "Installed resource packs and their order.",
        "shaderPacks" => "Shader packs and their settings.",
        "screenshots" => "Pictures taken in the game.",
        "configuration" => "Paths chosen in Paths & overrides.",
        "servers" => "Saved multiplayer servers.",
        _ => "",
    }
}

/// Turns an instance state such as `ready` or `needsInstall` into a badge.
fn instance_status(state: &str) -> (SharedString, ui::Tone) {
    let mut label = String::new();
    for (index, ch) in state.chars().enumerate() {
        if ch.is_uppercase() && index > 0 {
            label.push(' ');
            label.push(ch.to_ascii_lowercase());
        } else if index == 0 {
            label.extend(ch.to_uppercase());
        } else {
            label.push(ch);
        }
    }
    let lower = state.to_ascii_lowercase();
    let tone = if lower == "ready" {
        ui::Tone::Success
    } else if lower.contains("fail") || lower.contains("error") || lower.contains("broken") {
        ui::Tone::Danger
    } else if lower.contains("install") || lower.contains("updat") || lower.contains("running") {
        ui::Tone::Accent
    } else {
        ui::Tone::Neutral
    };
    (label.into(), tone)
}

/// A labelled form field: title and caption above the input.
fn field(
    t: &crate::theme::AuroraTheme,
    title: &'static str,
    caption: &'static str,
    input: impl IntoElement,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(ui::title_block(t, title, Some(caption.into())))
        .child(input)
}

/// A card whose rows run inside it, separated by hairlines.
fn section_card(t: &crate::theme::AuroraTheme) -> Div {
    // Rows carry their own vertical padding, so the card's bottom is trimmed.
    ui::card(t).gap(px(0.)).pb(px(6.))
}

/// Saved choices stay visible but inert while the shared policy is paused.
fn saved_choice(
    t: &crate::theme::AuroraTheme,
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
) -> impl IntoElement {
    let label: SharedString = label.into();
    let value = if checked { "Included" } else { "Excluded" };
    div()
        .id(id)
        .role(gpui::Role::Group)
        .aria_label(format!("{label}: {value}; syncing is off"))
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_end()
        .gap(px(6.))
        .text_size(px(ui::TEXT_CAPTION))
        .text_color(t.foreground)
        .child(value)
}

/// A right-aligned slot for one row's control.
fn control_slot(child: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_end()
        .w(px(CONTROL_WIDTH))
        .child(child)
}

impl Render for Syncing {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme(cx).clone();
        let t = &palette;
        let busy = self.busy();
        let editing_disabled = busy || !self.policy.writable();
        for field in [&self.exclusions, &self.paths, &self.override_exclusions] {
            field.update(cx, |field, cx| field.set_disabled(editing_disabled, cx));
        }
        self.sync_inputs(window, cx);

        let mut root = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            // Bound intrinsic text widths to the actual content viewport.
            .w((window.viewport_size().width - px(276.)).max(px(560.)))
            .min_w(px(0.))
            .text_color(palette.foreground)
            .child(ui::page_header(
                t,
                "Sync",
                Some(
                    "Keep game settings, packs and controls identical across the instances you choose."
                        .into(),
                ),
            ));

        if self.host.is_none() {
            return root.child(ui::empty_state(
                t,
                "sync",
                "Sync is unavailable",
                self.message.clone(),
            ));
        }

        // --- Status ---------------------------------------------------------
        let (status_label, status_tone, status_text) = {
            let ((label, tone), text) = match &self.status.value {
                Some(status) if status.running => (
                    ("Syncing", ui::Tone::Accent),
                    "Syncing selected instances…".to_owned(),
                ),
                Some(status) => (("Idle", ui::Tone::Neutral), status.message.clone()),
                None => (
                    ("Unknown", ui::Tone::Neutral),
                    "Sync status not loaded yet.".to_owned(),
                ),
            };
            (label, tone, text)
        };

        // --- Preferences ----------------------------------------------------
        if let Some(draft) = self.policy.draft.clone() {
            let view = cx.entity();
            let enabled = draft.enabled;
            // The master switch is stated in words, not only by the toggle.
            let status_tone = if enabled {
                status_tone
            } else {
                ui::Tone::Neutral
            };
            root = root.child(
                section_card(t)
                    .pb(px(20.))
                    .child(
                        ui::setting_row(
                            t,
                            "Sync shared files between instances",
                            Some(
                                if enabled {
                                    "Selected instances automatically share the categories below."
                                } else {
                                    "Turned off. Your choices below are kept, but nothing is synced."
                                }
                                .into(),
                            ),
                            true,
                        )
                        .py(px(0.))
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap(px(16.))
                                .when(enabled, |state| state.child(ui::status_badge(t, status_label, status_tone)))
                                .child(
                                    AuroraToggle::new("sync-enabled")
                                        .label("Sync selected instances automatically")
                                        .checked(enabled)
                                        .disabled(editing_disabled)
                                        .on_change({
                                            let view = view.clone();
                                            move |next: bool, _, _, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.set_enabled(next, cx)
                                                });
                                            }
                                        }),
                                ),
                        ),
                    )
                    .when(!status_text.trim().is_empty(), |card| {
                        card.child(
                            div()
                                .pt(px(12.))
                                .text_size(px(ui::TEXT_CAPTION))
                                .text_color(palette.muted_foreground)
                                .child(status_text),
                        )
                    }),
            );

            // While the master switch is off the cards keep full contrast; only
            // their toggles go inert and a notice in each header says why.
            let child_locked = editing_disabled || !enabled;
            let off_notice = |t: &crate::theme::AuroraTheme| {
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(ui::TEXT_CAPTION))
                    .text_color(t.muted_foreground)
                    .child(crate::symbols::icon(
                        Some("lock"),
                        px(14.),
                        t.muted_foreground,
                    ))
                    .child("Saved choices · turn on Sync to edit")
            };
            let mut body = div().flex().flex_col().gap(px(ui::PAGE_GAP));

            let mut tabs = div()
                .flex()
                .flex_wrap()
                .gap(px(4.))
                .pb(px(8.))
                .border_b_1()
                .border_color(t.border_subtle);
            for (index, label) in [
                "Shared files".to_owned(),
                "Participating instances".to_owned(),
                "Paths & overrides".to_owned(),
                format!("Conflicts ({})", self.conflicts.len()),
            ]
            .into_iter()
            .enumerate()
            {
                let view = view.clone();
                tabs = tabs.child(
                    AuroraButton::new(("sync-section", index), label)
                        .variant(ButtonVariant::Ghost)
                        .size(ButtonSize::Small)
                        .selected(self.section == index)
                        .on_click(move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                this.section = index;
                                cx.notify();
                            });
                        }),
                );
            }
            root = root.child(tabs);

            // What to sync.
            let categories = section_card(t).child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .pb(px(6.))
                    .child(ui::section_header(
                        t,
                        "What to sync",
                        Some("The kinds of files that stay in step across instances.".into()),
                    ))
                    .when(!enabled, |this| this.child(off_notice(t))),
            );
            let mut category_grid = div().grid().grid_cols(2).gap_x(px(24.));
            for (index, (id, label)) in CATEGORIES.iter().enumerate() {
                let checked = draft.categories.iter().any(|current| current == id);
                let id = *id;
                let choice_id = SharedString::from(format!("sync-category-{id}"));
                let row = if !enabled {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .py(px(8.))
                        .border_b_1()
                        .border_color(t.border_subtle)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .text_size(px(ui::TEXT_BODY))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(*label),
                                )
                                .child(saved_choice(t, choice_id, *label, checked)),
                        )
                        .child(ui::muted(t, category_detail(id)))
                } else {
                    ui::setting_row(
                        t,
                        *label,
                        Some(category_detail(id).into()),
                        index + 1 == CATEGORIES.len(),
                    )
                    .py(px(8.))
                    .child(
                        AuroraToggle::new(choice_id)
                            .label(*label)
                            .checked(checked)
                            .disabled(child_locked)
                            .on_change({
                                let view = view.clone();
                                move |next: bool, _, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.set_category_checked(id, next, cx)
                                    });
                                }
                            }),
                    )
                };
                category_grid = category_grid.child(row);
            }
            if self.section == 0 {
                body = body.child(categories.child(category_grid));
            }

            // Instances.
            let rows = participant_rows(&draft, self.instances.as_ref());
            let total = rows.len();
            let syncing = rows.iter().filter(|row| row.checked).count();
            let mut list = ui::list_card(t).child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .px(px(20.))
                    .py(px(16.))
                    .border_b_1()
                    .border_color(palette.border_subtle)
                    .child(ui::section_header(
                        t,
                        "Instances",
                        Some("Choose which instances take part in syncing.".into()),
                    ))
                    .child(if enabled {
                        ui::badge(
                            t,
                            format!("{syncing} of {total} syncing"),
                            ui::Tone::Neutral,
                        )
                        .into_any_element()
                    } else {
                        off_notice(t).into_any_element()
                    }),
            );
            if total == 0 {
                list = list.child(
                    div()
                        .px(px(16.))
                        .py(px(20.))
                        .child(ui::muted(t, "No instances yet.")),
                );
            }
            for (index, row) in rows.into_iter().enumerate() {
                let toggle_view = view.clone();
                let select_view = view.clone();
                let toggle_id = row.id.clone();
                let select_id = row.id.clone();
                let (status_label, tone) = if row.available {
                    instance_status(&row.detail)
                } else {
                    ("Unavailable".into(), ui::Tone::Warning)
                };
                let detail: SharedString = if !row.available {
                    row.detail.clone().into()
                } else if !row.checked {
                    "Not syncing".into()
                } else if draft.overrides.contains_key(&row.id) {
                    "Syncing with custom overrides".into()
                } else {
                    "Syncing with the shared settings".into()
                };
                list = list.child(
                    ui::row(t, index + 1 == total)
                        .px(px(20.))
                        .child(ui::icon_tile(t, Some("deployed_code"), 36., row.checked))
                        .child(
                            ui::title_block(t, row.name.clone(), Some(detail))
                                .flex_1()
                                .min_w(px(0.)),
                        )
                        .child(ui::status_badge(t, status_label, tone))
                        .child(
                            AuroraButton::new(
                                SharedString::from(format!("sync-overrides-{}", row.id)),
                                "Overrides…",
                            )
                            .size(ButtonSize::Small)
                            .disabled(busy || !row.checked)
                            .selected(self.selected.as_deref() == Some(&row.id))
                            .on_click(move |_, _, cx| {
                                select_view.update(cx, |this, cx| {
                                    if this.busy() {
                                        return;
                                    }
                                    this.category_open = [false; 7];
                                    this.selected = Some(select_id.clone());
                                    cx.notify();
                                });
                            }),
                        )
                        .child(if !enabled {
                            saved_choice(
                                t,
                                SharedString::from(format!("sync-participant-{}", row.id)),
                                row.name.clone(),
                                row.checked,
                            )
                            .into_any_element()
                        } else {
                            AuroraToggle::new(SharedString::from(format!(
                                "sync-participant-{}",
                                row.id
                            )))
                            .label(row.name.clone())
                            .checked(row.checked)
                            .disabled(child_locked)
                            .on_change(move |next: bool, _, _, cx| {
                                toggle_view.update(cx, |this, cx| {
                                    this.set_participant_checked(toggle_id.clone(), next, cx)
                                });
                            })
                            .into_any_element()
                        }),
                );
            }
            if self.section == 1 {
                body = body.child(list);
            }

            // Per-participant overrides.
            let selected = self.selected.clone();
            let participating = selected
                .as_deref()
                .is_some_and(|id| draft.instances.iter().any(|current| current == id));
            if let Some(id) = selected.filter(|_| participating && self.section == 2) {
                let value = draft.overrides.get(&id);
                let name = instance_name(self.instances.as_ref(), &id);
                let mut overrides = section_card(t)
                    .child(
                        ui::section_header(
                            t,
                            format!("Overrides for {name}"),
                            Some("How this instance differs from the shared settings.".into()),
                        )
                        .pb(px(6.)),
                    )
                    .child(
                        ui::setting_row(
                            t,
                            "Exclude this instance from syncing",
                            Some("It keeps its own files and shares nothing.".into()),
                            false,
                        )
                        .child(
                            AuroraToggle::new("override-disabled")
                                .label("Exclude this instance from syncing")
                                .checked(value.is_some_and(|value| value.disabled))
                                .disabled(child_locked)
                                .on_change({
                                    let view = view.clone();
                                    let id = id.clone();
                                    move |next: bool, _, _, cx| {
                                        view.update(cx, |this, cx| {
                                            this.set_override_disabled(id.clone(), next, cx)
                                        });
                                    }
                                }),
                        ),
                    )
                    .child(
                        div()
                            .py(px(14.))
                            .border_b_1()
                            .border_color(palette.border_subtle)
                            .child(field(
                                t,
                                "Exclusions",
                                "Paths this instance never syncs, separated by ;",
                                AuroraInput::new(&self.override_exclusions)
                                    .label("Exclusions for this instance, separated by ;"),
                            )),
                    );
                for (index, (category, label)) in CATEGORIES.iter().enumerate() {
                    let mode = override_mode(value, category);
                    let (trigger, content) = self.category_focus[index].clone();
                    let bounds = self.category_bounds[index].clone();
                    let options = OVERRIDE_MODES
                        .iter()
                        .map(|mode| SharedString::from(*mode))
                        .collect();
                    let select = AuroraSelect::new(
                        SharedString::from(format!("sync-override-mode-{category}")),
                        *label,
                        options,
                        &trigger,
                        &content,
                        bounds,
                    )
                    .selected(Some(mode))
                    .disabled(child_locked)
                    .highlight(self.category_highlight[index])
                    .open(self.category_open[index] && !busy)
                    .on_open_change({
                        let view = view.clone();
                        move |open, _, cx| {
                            view.update(cx, |this, cx| {
                                this.category_open[index] =
                                    open && !this.busy() && this.policy.writable();
                                cx.notify();
                            });
                        }
                    })
                    .on_highlight({
                        let view = view.clone();
                        move |index_choice, _, cx| {
                            view.update(cx, |this, cx| {
                                this.category_highlight[index] = index_choice;
                                cx.notify();
                            });
                        }
                    })
                    .on_confirm({
                        let view = view.clone();
                        let id = id.clone();
                        let category = *category;
                        move |mode, _, cx| {
                            view.update(cx, |this, cx| {
                                this.category_open[index] = false;
                                this.choose_override_mode(id.clone(), category, mode, cx);
                            });
                        }
                    });
                    overrides = overrides.child(
                        ui::setting_row(
                            t,
                            *label,
                            Some(category_detail(category).into()),
                            index + 1 == CATEGORIES.len(),
                        )
                        .child(control_slot(select)),
                    );
                }
                body = body.child(overrides);
            }

            // Global paths and exclusions.
            if self.section == 2 {
                body = body.child(
                ui::card(t)
                    .gap(px(0.))
                    .child(
                        ui::section_header(
                            t,
                            "Paths",
                            Some("Applied to every participating instance.".into()),
                        )
                        .pb(px(14.)),
                    )
                    .child(
                        div().flex().flex_col().gap(px(16.))
                            .child(field(
                                t,
                                "Selected configuration paths",
                                "Files and folders the Selected configuration category syncs, separated by ;",
                                AuroraInput::new(&self.paths)
                                    .label("Selected configuration paths, separated by ;"),
                            ))
                            .child(field(
                                t,
                                "Exclusions",
                                "Paths that are never synced, separated by ;",
                                AuroraInput::new(&self.exclusions)
                                    .label("Exclusions, separated by ;"),
                            )),
                    ),
            );
            }
            root = root.child(body);

            // Apply / Revert.
            let dirty = self.policy.dirty;
            let (save_title, save_detail) = if self.write_busy {
                ("Saving…", "Applying your sync preferences.")
            } else if dirty {
                ("Unsaved changes", "Apply to save them and start syncing.")
            } else {
                ("All changes saved", "Your sync preferences are up to date.")
            };
            let mut actions = ui::card(t).gap(px(12.)).child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(
                        ui::title_block(t, save_title, Some(save_detail.into()))
                            .flex_1()
                            .min_w(px(0.)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                AuroraButton::new("sync-refresh", "Refresh sync")
                                    .icon("refresh")
                                    .disabled(busy)
                                    .on_click({
                                        let view = view.clone();
                                        move |_, _, cx| view.update(cx, |this, cx| this.refresh(cx))
                                    }),
                            )
                            .when(dirty || self.write_busy, |buttons| {
                                buttons.child(
                                    AuroraButton::new("sync-revert", "Revert draft")
                                        .disabled(busy || !self.policy.dirty)
                                        .on_click({
                                            let view = view.clone();
                                            move |_, _, cx| {
                                                view.update(cx, |this, cx| this.revert(cx))
                                            }
                                        }),
                                )
                            })
                            .when(dirty || self.write_busy, |buttons| {
                                buttons.child(
                                    AuroraButton::new("sync-apply", "Apply")
                                        .variant(ButtonVariant::Primary)
                                        .disabled(busy || !self.policy.can_apply())
                                        .on_click({
                                            let view = view.clone();
                                            move |_, _, cx| {
                                                view.update(cx, |this, cx| this.apply(cx))
                                            }
                                        }),
                                )
                            }),
                    ),
            );
            if let Some(error) = &self.policy.error {
                actions = actions.child(
                    div()
                        .text_size(px(ui::TEXT_BODY))
                        .text_color(palette.danger)
                        .child(error.clone()),
                );
            }
            root = root.child(actions);
        } else {
            let view = cx.entity();
            let status_strip = div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(ui::status_badge(t, status_label, status_tone))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(px(ui::TEXT_BODY))
                        .text_color(palette.muted_foreground)
                        .child(status_text.clone()),
                );
            root = root.child(status_strip_card(t, status_strip)).child(
                ui::card(t)
                    .child(
                        div()
                            .text_size(px(ui::TEXT_BODY))
                            .text_color(palette.danger)
                            .child(self.policy.error.clone().unwrap_or_else(|| {
                                "The saved sync policy could not be read; nothing can be saved until it loads.".into()
                            })),
                    )
                    .child(
                        div().flex().child(
                            AuroraButton::new("sync-retry", "Retry loading sync")
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |this, cx| this.refresh(cx))
                                }),
                        ),
                    ),
            );
        }

        if !self.message.is_empty() {
            let resolved = self.message.starts_with("Conflict resolved");
            let tone = if resolved {
                palette.success
            } else {
                palette.danger
            };
            root = root.child(
                div()
                    .px(px(16.))
                    .py(px(12.))
                    .rounded(palette.radius)
                    .bg(gpui::Hsla { a: 0.12, ..tone })
                    .text_size(px(ui::TEXT_BODY))
                    .text_color(tone)
                    .child(self.message.clone()),
            );
        }

        // --- Conflicts ------------------------------------------------------
        let mut conflicts = ui::list_card(t).child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(16.))
                .px(px(20.))
                .py(px(16.))
                .border_b_1()
                .border_color(palette.border_subtle)
                .child(
                    ui::section_header(
                        t,
                        "Conflicts",
                        Some(
                            "Resolving keeps one version and replaces the other; the replaced version is backed up. Server-list conflicts stay pending and are resolved in the game."
                                .into(),
                        ),
                    )
                    .flex_1()
                    .min_w(px(0.)),
                )
                .child(if self.conflicts.is_empty() {
                    ui::badge(t, "None pending", ui::Tone::Neutral)
                } else {
                    ui::badge(
                        t,
                        format!("{} pending", self.conflicts.len()),
                        ui::Tone::Warning,
                    )
                }),
        );
        if self.conflicts_loading && self.conflicts.is_empty() {
            conflicts = conflicts.child(
                div()
                    .px(px(16.))
                    .py(px(16.))
                    .child(ui::muted(t, "Loading conflicts…")),
            );
        } else if self.conflicts.is_empty() {
            conflicts = conflicts.child(
                ui::row(t, true)
                    .px(px(20.))
                    .child(ui::icon_tile(t, Some("check_circle"), 36., false))
                    .child(ui::muted(t, "No pending conflicts.")),
            );
        }
        let total_conflicts = self.conflicts.len();
        for (index, conflict) in self.conflicts.clone().into_iter().enumerate() {
            let view = cx.entity();
            let resolvable = conflict_resolvable(&conflict);
            let label = conflict_label(&conflict);
            let peers = format!(
                "First: {} · Second: {}",
                instance_name(self.instances.as_ref(), &conflict.source_id),
                instance_name(self.instances.as_ref(), &conflict.destination_id)
            );
            let mut row = ui::row(t, index + 1 == total_conflicts)
                .px(px(20.))
                .child(ui::icon_tile(t, Some("warning"), 36., false))
                .child(
                    ui::title_block(t, label, Some(peers.into()))
                        .flex_1()
                        .min_w(px(0.)),
                );
            if resolvable {
                row = row.child(
                    AuroraButton::new(
                        SharedString::from(format!("sync-review-{}", conflict.id)),
                        "Review…",
                    )
                    .size(ButtonSize::Small)
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        view.update(cx, |this, cx| this.open_review(conflict.clone(), cx));
                    }),
                );
            } else {
                row = row.child(ui::badge(t, "Resolve in game", ui::Tone::Neutral));
            }
            conflicts = conflicts.child(row);
        }
        if self.section == 3 || self.policy.draft.is_none() {
            root = root.child(conflicts);
        }

        // Keep the dialog mounted so base focus restoration observes closing.
        let mut panel = div()
            .w(px(480.))
            .max_w_full()
            .p(px(24.))
            .bg(palette.raised)
            .border_1()
            .border_color(palette.border_subtle)
            .text_color(palette.foreground)
            .rounded(palette.radius_lg)
            .flex()
            .flex_col()
            .gap(px(16.));
        if let Some(review) = self.review.clone() {
            let source_view = cx.entity();
            let destination_view = cx.entity();
            let cancel_view = cx.entity();
            let source_identity = review.conflict.clone();
            let destination_identity = review.conflict.clone();
            let cancel_identity = review.conflict.clone();
            panel = panel
                .child(ui::section_header(
                    t,
                    "Resolve conflict",
                    Some(
                        format!(
                            "{} · {}",
                            conflict_label(&review.conflict),
                            review.conflict.category
                        )
                        .into(),
                    ),
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .p(px(14.))
                        .rounded(palette.radius)
                        .bg(palette.sunken)
                        .border_1()
                        .border_color(palette.border_subtle)
                        .child(ui::stat(
                            t,
                            "First version",
                            format!("{} ({})", review.source, review.conflict.source_id),
                        ))
                        .child(ui::stat(
                            t,
                            "Second version",
                            format!(
                                "{} ({})",
                                review.destination, review.conflict.destination_id
                            ),
                        )),
                )
                .child(ui::muted(
                    t,
                    "The chosen version replaces the other. The core backs up the replaced version.",
                ))
                .when(!self.message.is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(ui::TEXT_BODY))
                            .text_color(palette.danger)
                            .child(self.message.clone()),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            AuroraButton::new("sync-review-cancel", "Leave pending")
                                .variant(ButtonVariant::Ghost)
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    cancel_view.update(cx, |this, cx| {
                                        this.close_review(&cancel_identity, cx);
                                    })
                                }),
                        )
                        .child(
                            AuroraButton::new("sync-resolve-destination", "Keep second version")
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    destination_view.update(cx, |this, cx| {
                                        this.resolve(&destination_identity, false, cx);
                                    })
                                }),
                        )
                        .child(
                            AuroraButton::new("sync-resolve-source", "Keep first version")
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    source_view.update(cx, |this, cx| {
                                        this.resolve(&source_identity, true, cx);
                                    })
                                }),
                        ),
                );
        }
        let veto = cx.entity();
        let changed = cx.entity();
        let expected = self.review.as_ref().map(|review| review.conflict.clone());
        root = root.child(
            Dialog::new(cx)
                .open(self.review.is_some())
                .accessibility_label("Resolve sync conflict")
                .close_on_backdrop_press(false)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !veto.read(cx).busy())
                .on_open_change(move |open, _, _, cx| {
                    if !open {
                        if let Some(expected) = &expected {
                            changed.update(cx, |this, cx| this.close_review(expected, cx));
                        }
                    }
                })
                .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                .popup(panel),
        );

        root
    }
}

/// The status line alone in a card, for when no policy is loaded.
fn status_strip_card(t: &crate::theme::AuroraTheme, strip: Div) -> Div {
    section_card(t).child(strip.pt(px(0.)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn policy(value: serde_json::Value) -> api::SyncPolicy {
        serde_json::from_value(value).unwrap()
    }

    fn base_policy() -> api::SyncPolicy {
        policy(json!({
            "schemaVersion": 2,
            "enabled": true,
            "instances": ["a", "b"],
            "categories": ["settings", "controls", "resourcePacks", "shaderPacks", "screenshots"],
            "exclusions": ["cache"],
            "selectedPaths": ["options.txt"],
            "overrides": {
                "a": {"disabled": true, "categories": {"settings": false, "future": true}, "exclusions": ["a.txt"]},
                "b": {"disabled": false, "categories": {"controls": true}, "exclusions": ["b.txt"]}
            }
        }))
    }

    fn err(message: &str) -> ApiResult<api::SyncPolicy> {
        Err(api::ApiError::new("policy_unreadable", message))
    }

    #[test]
    fn participant_and_category_edits_preserve_untouched_policy() {
        let mut saved = base_policy();
        set_participant(&mut saved, "c", true);
        set_category(&mut saved, "servers", true);
        set_category(&mut saved, "controls", false);
        assert_eq!(saved.schema_version, 2);
        assert!(saved.enabled);
        assert_eq!(saved.instances, vec!["a", "b", "c"]);
        assert_eq!(
            saved.categories,
            vec![
                "settings",
                "resourcePacks",
                "shaderPacks",
                "screenshots",
                "servers"
            ]
        );
        assert_eq!(saved.exclusions, vec!["cache"]);
        assert_eq!(saved.selected_paths, vec!["options.txt"]);
        // Unrelated overrides survive untouched, including unknown entries.
        assert_eq!(
            saved.overrides.get("a").unwrap().categories.get("future"),
            Some(&true)
        );
        assert_eq!(saved.overrides.get("b").unwrap().exclusions, vec!["b.txt"]);
    }

    #[test]
    fn removing_participant_clears_its_override_only() {
        let mut saved = base_policy();
        set_participant(&mut saved, "a", false);
        assert_eq!(saved.instances, vec!["b"]);
        assert!(saved.overrides.get("a").is_none());
        assert!(saved.overrides.get("b").is_some());
        // Re-checking creates no override: the participant inherits again.
        set_participant(&mut saved, "a", true);
        assert!(saved.overrides.get("a").is_none());
        assert_eq!(saved.instances, vec!["b", "a"]);
    }

    #[test]
    fn override_edits_match_native_clearing_behavior() {
        let mut saved = base_policy();
        // Only participating instances can carry an override.
        edit_override(&mut saved, "ghost", |value| value.disabled = true);
        assert!(saved.overrides.get("ghost").is_none());
        // The entry stays while it still carries exclusions.
        edit_override(&mut saved, "b", |value| {
            set_override_mode(value, "controls", 0)
        });
        assert!(saved.overrides.get("b").is_some());
        // Once nothing meaningful remains, choosing Inherit clears the
        // entry, exactly like the native clearing behavior.
        edit_override(&mut saved, "b", |value| value.exclusions.clear());
        edit_override(&mut saved, "b", |value| {
            set_override_mode(value, "controls", 0)
        });
        assert!(saved.overrides.get("b").is_none());
        // A disabled flag or exclusions keep the override alive.
        edit_override(&mut saved, "b", |value| value.disabled = true);
        assert!(saved.overrides.get("b").is_some());
        edit_override(&mut saved, "b", |value| {
            value.exclusions = vec!["keep".to_owned()]
        });
        assert_eq!(saved.overrides.get("b").unwrap().exclusions, vec!["keep"]);
        assert_eq!(override_mode(saved.overrides.get("b"), "controls"), 0);
        edit_override(&mut saved, "b", |value| {
            set_override_mode(value, "controls", 2)
        });
        assert_eq!(override_mode(saved.overrides.get("b"), "controls"), 2);
    }

    #[test]
    fn malformed_read_never_enables_save() {
        let mut state = PolicyState::default();
        state.loaded(true, err("damaged"));
        assert!(state.baseline.is_none());
        assert!(!state.writable());
        assert!(!state.can_apply());
        // A later good read recovers the page honestly.
        state.loaded(true, Ok(base_policy()));
        assert!(state.writable());
        // A malformed refresh with no acknowledged baseline never synthesizes
        // a writable policy even after edits.
        let mut fresh = PolicyState::default();
        fresh.loaded(true, err("damaged"));
        fresh.loaded(true, Ok(base_policy()));
        if let Some(draft) = fresh.draft.as_mut() {
            draft.enabled = false;
            fresh.dirty = true;
        }
        assert!(fresh.can_apply());
        fresh.loaded(true, err("damaged again"));
        assert!(!fresh.can_apply());
    }

    #[test]
    fn write_failure_keeps_draft_for_retry() {
        let mut state = PolicyState::default();
        state.loaded(true, Ok(base_policy()));
        if let Some(draft) = state.draft.as_mut() {
            draft.enabled = false;
        }
        state.dirty = true;
        assert!(!state.saved(err("rejected")));
        assert!(state.dirty);
        assert!(state.can_apply());
        assert_eq!(state.draft.as_ref().unwrap().enabled, false);
        assert_eq!(state.baseline.as_ref().unwrap().enabled, true);
        let mut acked = base_policy();
        acked.enabled = false;
        assert!(state.saved(Ok(acked)));
        assert!(!state.dirty);
        assert_eq!(state.baseline.as_ref().unwrap().enabled, false);
    }

    #[test]
    fn dirty_draft_survives_late_policy_reads() {
        let mut state = PolicyState::default();
        state.loaded(true, Ok(base_policy()));
        if let Some(draft) = state.draft.as_mut() {
            draft.exclusions = vec!["edited".to_owned()];
        }
        state.dirty = true;
        // A late clean read must not replace the dirty draft.
        let mut newer = base_policy();
        newer.exclusions = vec!["disk".to_owned()];
        state.loaded(false, Ok(newer));
        assert_eq!(state.draft.as_ref().unwrap().exclusions, vec!["edited"]);
        // A clean page still accepts a fresh read.
        let mut clean = PolicyState::default();
        clean.loaded(true, Ok(base_policy()));
        let mut disk = base_policy();
        disk.exclusions = vec!["disk".to_owned()];
        clean.loaded(true, Ok(disk));
        assert_eq!(clean.draft.as_ref().unwrap().exclusions, vec!["disk"]);
        assert!(!clean.dirty);
    }

    #[test]
    fn a_pre_edit_read_cannot_undo_a_save_or_revert() {
        let mut state = PolicyState::default();
        let initial = state.begin_read(false).unwrap();
        assert!(state.complete_read(initial, Ok(base_policy())));
        let late = state.begin_read(false).unwrap();
        state.edit(|draft| draft.overrides.get_mut("a").unwrap().exclusions = vec!["new".into()]);
        let submitted = state.begin_write().unwrap();
        assert!(state.saved(Ok(submitted)));
        assert!(!state.complete_read(late, Ok(base_policy())));
        assert_eq!(
            state.draft.as_ref().unwrap().overrides["a"].exclusions,
            ["new"]
        );

        let late = state.begin_read(false).unwrap();
        state.edit(|draft| draft.overrides.get_mut("a").unwrap().exclusions.clear());
        assert!(state.revert());
        assert!(!state.complete_read(late, Ok(base_policy())));
        assert_eq!(
            state.draft.as_ref().unwrap().overrides["a"].exclusions,
            ["new"]
        );
        assert!(!state.dirty);
    }

    #[test]
    fn unreadable_refresh_blocks_writes_without_discarding_acknowledged_data() {
        let mut state = PolicyState::default();
        let initial = state.begin_read(false).unwrap();
        state.complete_read(initial, Ok(base_policy()));
        let read = state.begin_read(false).unwrap();
        state.complete_read(read, err("damaged on disk"));
        state.edit(|draft| draft.enabled = false);
        assert!(state.begin_write().is_none());
        assert!(state.baseline.as_ref().unwrap().enabled);
        assert!(state.draft.as_ref().unwrap().enabled);
        let retry = state.begin_read(false).unwrap();
        assert!(state.complete_read(retry, Ok(base_policy())));
        state.edit(|draft| draft.enabled = false);
        assert!(state.begin_write().is_some());
    }

    #[test]
    fn conflict_resolve_settle_rejects_old_lists_and_duplicate_completions() {
        let mut requests = ConflictRequests::default();
        let before = requests.begin_read().unwrap();
        let first = requests.begin_resolve("c1".into()).unwrap();
        assert!(!requests.accepts_read(before));
        assert!(requests.begin_read().is_none());
        assert!(requests.finish_resolve(&first)); // both success and failure settle here
        let after = requests.begin_read().unwrap();
        assert!(!requests.accepts_read(before));
        assert!(requests.accepts_read(after));
        let second = requests.begin_resolve("c2".into()).unwrap();
        assert!(!requests.finish_resolve(&first));
        assert!(requests.pending.as_ref() == Some(&second));
        assert!(!requests.accepts_read(after));
        assert!(requests.finish_resolve(&second));
    }

    #[test]
    fn status_events_supersede_reads_and_read_completions_detect_finish() {
        let mut status = StatusState::default();
        let old = status.begin_read();
        assert!(!status.event(api::SyncStatus {
            running: true,
            message: "running".into()
        }));
        assert!(!status.complete_read(
            old,
            api::SyncStatus {
                running: false,
                message: "old".into()
            }
        ));
        assert!(status.value.as_ref().unwrap().running);
        let fresh = status.begin_read();
        assert!(status.complete_read(
            fresh,
            api::SyncStatus {
                running: false,
                message: "done".into()
            }
        ));
        assert_eq!(status.value.as_ref().unwrap().message, "done");
    }

    #[test]
    fn stale_conflict_identities_are_rejected() {
        let guard = ResolveGuard {
            id: "c1".into(),
            revision: 3,
        };
        let mut requests = ConflictRequests {
            serial: 3,
            pending: Some(guard.clone()),
        };
        assert!(!requests.finish_resolve(&ResolveGuard {
            id: "c2".into(),
            revision: 3
        }));
        assert!(!requests.finish_resolve(&ResolveGuard {
            id: "c1".into(),
            revision: 4
        }));
        assert!(requests.pending.as_ref() == Some(&guard));
        assert!(requests.finish_resolve(&guard));
        let a = api::SyncConflict {
            id: "c1".into(),
            source_id: "a".into(),
            destination_id: "b".into(),
            category: "settings".into(),
            path: "options.txt".into(),
            key: Some("key".into()),
        };
        let mut b = a.clone();
        assert!(conflict_identity_matches(&a, &b));
        b.key = None;
        assert!(!conflict_identity_matches(&a, &b));
        b.key = Some("key".into());
        b.path = "other.txt".into();
        assert!(!conflict_identity_matches(&a, &b));
    }

    #[test]
    fn conflict_rows_label_and_resolvability() {
        let with_key = api::SyncConflict {
            id: "c1".into(),
            source_id: "a".into(),
            destination_id: "b".into(),
            category: "configuration".into(),
            path: "options.txt".into(),
            key: Some("fov".into()),
        };
        assert_eq!(conflict_label(&with_key), "options.txt · fov");
        assert!(conflict_resolvable(&with_key));
        let servers = api::SyncConflict {
            category: "servers".into(),
            key: None,
            ..with_key
        };
        assert_eq!(conflict_label(&servers), "options.txt");
        assert!(!conflict_resolvable(&servers));
    }

    #[test]
    fn path_vocabulary_splits_semicolons() {
        assert_eq!(split_paths(" a ; ;b;; "), vec!["a", "b"]);
        assert!(split_paths(" ; ").is_empty());
    }

    #[test]
    fn participant_rows_keep_unavailable_saved_participants_visible() {
        let state: Option<api::LauncherState> = serde_json::from_value(json!({
            "config": {"selectedInstanceId": null},
            "instances": [
                {"id": "a", "displayName": "Alpha", "state": "ready", "aurora": null,
                 "platform": {"kind": "vanilla"}, "minecraftVersion": "1.21", "configuration": {
                    "minecraftVersion": "1.21", "auroraEnabled": false, "loader": {"kind": "vanilla"},
                    "memoryMib": 2048, "additionalJvmArguments": "", "window": null}}
            ],
            "platformCapabilities": []
        }))
        .unwrap();
        let rows = participant_rows(&base_policy(), state.as_ref());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "a");
        assert_eq!(rows[0].name, "Alpha");
        assert!(rows[0].checked);
        assert_eq!(rows[1].id, "b");
        assert_eq!(rows[1].name, "Unavailable instance");
        assert!(rows[1].checked);
    }
}
