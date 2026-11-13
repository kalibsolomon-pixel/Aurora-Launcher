//! Instance content management through the native core.
//!
//! Row artwork follows the Slint host (`native/src/instances.rs`): the core
//! returns icons in batches (`api::mod_icons`, `api::pack_icons`, with the
//! provider project icon as the fallback), one batch in flight, and the UI
//! thread converts each result once into a retained image. Rows show a letter
//! glyph until their icon arrives or when none exists. Every batch carries
//! the icon serial and the exact instance/kind/revision, so a result for
//! superseded inventory is dropped.
//!
//! Local files can also be dropped onto the page. A dropped path feeds the
//! same preview-ticket review as the path field; nothing installs before the
//! reviewed install action.
use crate::{
    api,
    artwork::{self, ArtworkCache, RawArtwork},
    backend::guarded,
    content_updates::{ContentUpdates, ContentUpdatesEvent},
    controls::{AuroraButton, AuroraInput, ButtonVariant},
};
use aurora_launcher_lib::host::Host;
use gpui::{
    Context, Entity, EventEmitter, ExternalPaths, Render, Subscription, Window, div, prelude::*, px,
};
use gpui_kit::base::{
    Dialog,
    input::{InputEvent, InputState},
};
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::runtime::Handle;

/// Icons requested per core call (the Slint host's `ICON_BATCH`).
const ICON_BATCH: usize = 8;
/// Retained icons, and how many leading rows may request one. The page lists
/// every row without virtualization, so the leading rows stand in for the
/// Slint host's visible set; later rows keep their placeholder.
const ICON_CACHE: usize = 64;

pub enum ContentEvent {
    Changed,
    /// "Find more": open Discover for this instance and kind index (0 mods,
    /// 1 resource packs, 2 shaders; the Slint content tab index).
    Discover {
        instance: String,
        kind: i32,
    },
}
impl EventEmitter<ContentEvent> for Content {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Mod,
    Resource,
    Shader,
}
impl Kind {
    fn index(self) -> i32 {
        match self {
            Self::Mod => 0,
            Self::Resource => 1,
            Self::Shader => 2,
        }
    }
    /// Out-of-range indexes clamp to the nearest tab, like Slint's `clamp(0, 2)`.
    fn from_index(index: i32) -> Self {
        match index.clamp(0, 2) {
            0 => Self::Mod,
            1 => Self::Resource,
            _ => Self::Shader,
        }
    }
    fn api(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Resource => "resourcePack",
            Self::Shader => "shaderPack",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Mod => "Mods",
            Self::Resource => "Resource packs",
            Self::Shader => "Shader packs",
        }
    }
}
#[derive(Clone)]
struct Entry {
    id: String,
    name: String,
    file_name: String,
    details: String,
    enabled: Option<bool>,
    toggle: bool,
    remove: bool,
    /// Placeholder glyph shown until (or unless) an icon loads.
    letter: String,
    icon: Option<IconSource>,
    /// Modrinth project id when provenance names one; its artwork is the
    /// fallback for a file that carries no icon of its own.
    project: Option<String>,
}

/// What the core is asked for to draw one row's icon.
#[derive(Clone, PartialEq)]
enum IconSource {
    Mod(api::ModIconRequest),
    Pack(api::PackIconRequest, String),
}
impl IconSource {
    /// Cache key: a mod's content hash, or the pack row identity plus its hash.
    fn key(&self) -> String {
        match self {
            Self::Mod(request) => request.sha256.clone(),
            Self::Pack(_, key) => key.clone(),
        }
    }
    /// Content-addressed icons stay valid across inventory reads; a pack
    /// without a hash (a directory pack) can change artwork in place.
    fn stable(&self) -> bool {
        match self {
            Self::Mod(_) => true,
            Self::Pack(request, _) => request.sha256.is_some(),
        }
    }
}
fn modrinth_project(p: &Option<api::Provenance>) -> Option<String> {
    p.as_ref()
        .filter(|p| p.provider == "modrinth")
        .map(|p| p.project_id.clone())
}
fn provenance(p: &Option<api::Provenance>) -> String {
    p.as_ref()
        .map(|p| {
            format!(
                "{} · {}{}",
                p.provider,
                p.project_id,
                p.display_version
                    .as_ref()
                    .map(|v| format!(" · {v}"))
                    .unwrap_or_default()
            )
        })
        .unwrap_or_else(|| "No provider provenance".into())
}
fn mods(inv: api::ModInventory) -> Vec<Entry> {
    inv.entries
        .into_iter()
        .map(|e| {
            let icon = e.sha256.as_ref().map(|hash| {
                IconSource::Mod(api::ModIconRequest {
                    file_name: e.file_name.clone(),
                    sha256: hash.clone(),
                })
            });
            let project = modrinth_project(&e.provenance);
            let version = e
                .metadata
                .as_ref()
                .and_then(|m| m.version.as_deref())
                .unwrap_or("Version unknown");
            let mut details = format!(
                "{} · {} · {} · {}",
                version,
                if e.enabled { "Enabled" } else { "Disabled" },
                e.ownership,
                provenance(&e.provenance)
            );
            for w in e.warnings {
                details.push_str(&format!("\nWarning: {}", w.message));
            }
            for reason in [e.action_blocked_reason, e.removal_blocked_reason]
                .into_iter()
                .flatten()
            {
                details.push_str(&format!("\n{reason}"));
            }
            Entry {
                id: e.entry_id,
                letter: artwork::letter(&e.display_name),
                name: e.display_name,
                file_name: e.file_name,
                details,
                enabled: Some(e.enabled),
                toggle: e.can_toggle,
                remove: e.can_remove,
                icon,
                project,
            }
        })
        .collect()
}
fn packs(inv: api::ContentInventory) -> Vec<Entry> {
    inv.entries
        .into_iter()
        .map(|e| {
            // Directories and archives can carry a pack.png; other files cannot.
            let icon = matches!(e.file_type.as_str(), "zip" | "directory").then(|| {
                IconSource::Pack(
                    api::PackIconRequest {
                        file_name: e.file_name.clone(),
                        sha256: e.sha256.clone(),
                    },
                    format!("pack:{}:{}", e.entry_id, e.sha256.as_deref().unwrap_or("")),
                )
            });
            let project = modrinth_project(&e.provenance);
            let m = e.management;
            let status = if m.activation_managed_in_game {
                "Activation managed in game"
            } else {
                match m.active {
                    Some(true) => "Enabled",
                    Some(false) => "Disabled",
                    None => "Activation unknown",
                }
            };
            let mut details = format!(
                "{} · {} · {}",
                status,
                if e.pack_owned {
                    "Pack owned"
                } else {
                    "Not pack owned"
                },
                provenance(&e.provenance)
            );
            for w in e.warnings {
                details.push_str(&format!("\nWarning: {}", w.message));
            }
            for reason in [m.toggle_blocked_reason, m.removal_blocked_reason]
                .into_iter()
                .flatten()
            {
                details.push_str(&format!("\n{reason}"));
            }
            Entry {
                id: e.entry_id,
                letter: artwork::letter(&e.display_name),
                name: e.display_name,
                file_name: e.file_name,
                details,
                enabled: m.active,
                toggle: m.can_toggle && !m.activation_managed_in_game && m.active.is_some(),
                remove: m.can_remove,
                icon,
                project,
            }
        })
        .collect()
}
#[derive(Clone, PartialEq, Eq)]
struct Target {
    instance: String,
    kind: Kind,
    revision: u64,
}
#[derive(Clone)]
struct Review {
    target: Target,
    path: String,
    ticket: String,
    fingerprint: String,
    value: Value,
}
/// One icon the core is asked to produce, with its provider fallback.
struct IconRequest {
    source: IconSource,
    project: Option<String>,
}
type IconResults = Vec<(String, Option<RawArtwork>)>;

/// Retained row icons and the request bookkeeping that keeps them honest.
/// Pure state: every transition runs without a window or host.
struct IconBook {
    cache: ArtworkCache<String>,
    /// Keys whose batch failed; retried only after the next inventory read.
    failed: HashSet<String>,
    /// Bumped by every inventory read and target change; a batch result for
    /// an older serial is dropped.
    serial: u64,
    /// At most one batch is in flight, like the Slint host.
    loading: bool,
}
impl IconBook {
    fn new() -> Self {
        Self {
            cache: ArtworkCache::new(ICON_CACHE),
            failed: HashSet::new(),
            serial: 0,
            loading: false,
        }
    }
    /// A new inventory read supersedes any batch in flight and forgives
    /// earlier failures. Retained icons stay until the read settles.
    fn supersede(&mut self) {
        self.serial = self.serial.wrapping_add(1);
        self.loading = false;
        self.failed.clear();
    }
    /// A different instance or kind shares no icons with the previous one.
    fn reset(&mut self) {
        self.supersede();
        self.cache.clear();
    }
    /// After an inventory read: keep icons of rows that are still listed and
    /// content-addressed; an archive without a hash (or a directory pack) may
    /// have changed in place, so its icon is fetched again.
    fn retain_for(&mut self, entries: &[Entry]) {
        let keep: HashSet<String> = entries
            .iter()
            .filter_map(|e| e.icon.as_ref())
            .filter(|icon| icon.stable())
            .map(IconSource::key)
            .collect();
        self.cache.retain(|key| keep.contains(key));
    }
    /// The next batch to request, if none is in flight: leading rows without
    /// a retained or failed icon, at most [`ICON_BATCH`], one per key.
    fn next_batch(&mut self, entries: &[Entry]) -> Option<(u64, Vec<IconRequest>)> {
        if self.loading {
            return None;
        }
        let mut seen = HashSet::new();
        let batch: Vec<IconRequest> = entries
            .iter()
            .take(ICON_CACHE)
            .filter_map(|e| {
                let source = e.icon.clone()?;
                let key = source.key();
                (!self.cache.contains(&key) && !self.failed.contains(&key) && seen.insert(key))
                    .then(|| IconRequest {
                        source,
                        project: e.project.clone(),
                    })
            })
            .take(ICON_BATCH)
            .collect();
        if batch.is_empty() {
            return None;
        }
        self.loading = true;
        Some((self.serial, batch))
    }
    /// Applies a batch result. Returns false (changing nothing) when the
    /// batch belongs to superseded inventory.
    fn apply(
        &mut self,
        serial: u64,
        entries: &[Entry],
        keys: Vec<String>,
        result: Result<IconResults, ()>,
    ) -> bool {
        if serial != self.serial {
            return false;
        }
        self.loading = false;
        match result {
            Ok(icons) => {
                let wanted: HashSet<String> = entries
                    .iter()
                    .take(ICON_CACHE)
                    .filter_map(|e| e.icon.as_ref())
                    .map(IconSource::key)
                    .collect();
                for (key, raw) in icons {
                    // The one GPUI conversion; an unusable bitmap is an absent icon.
                    let image = raw.and_then(RawArtwork::into_image);
                    self.cache.insert(key, image, |old| wanted.contains(old));
                }
            }
            // Not retried until the next inventory read, so a failing core
            // call never loops.
            Err(()) => self.failed.extend(keys),
        }
        true
    }
}

/// Fetches one batch: archive icons from the core, then the provider's
/// project artwork for rows whose file carries none (shader packs never ship
/// a pack.png). The core reads the artwork cache first.
async fn fetch_icons(
    host: Host,
    target: Target,
    batch: Vec<IconRequest>,
) -> api::ApiResult<IconResults> {
    let mut icons: IconResults = if target.kind == Kind::Mod {
        let requests = batch
            .iter()
            .filter_map(|r| match &r.source {
                IconSource::Mod(request) => Some(request.clone()),
                IconSource::Pack(..) => None,
            })
            .collect();
        let results = api::mod_icons(host.clone(), target.instance.clone(), requests).await?;
        batch
            .iter()
            .map(|r| {
                let key = r.source.key();
                let raw = results
                    .iter()
                    .find(|result| result.sha256 == key)
                    .and_then(|result| result.bitmap.as_ref())
                    .and_then(artwork::raw_from_bitmap);
                (key, raw)
            })
            .collect()
    } else {
        let requests = batch
            .iter()
            .filter_map(|r| match &r.source {
                IconSource::Pack(request, _) => Some(request.clone()),
                IconSource::Mod(_) => None,
            })
            .collect();
        let results = api::pack_icons(
            host.clone(),
            target.instance.clone(),
            target.kind.api(),
            requests,
        )
        .await?;
        batch
            .iter()
            .map(|r| {
                let raw = match &r.source {
                    IconSource::Pack(request, _) => results
                        .iter()
                        .find(|result| result.file_name == request.file_name)
                        .and_then(|result| result.bitmap.as_ref())
                        .and_then(artwork::raw_from_bitmap),
                    IconSource::Mod(_) => None,
                };
                (r.source.key(), raw)
            })
            .collect()
    };
    for ((_, raw), request) in icons.iter_mut().zip(&batch) {
        if raw.is_none()
            && let Some(project) = &request.project
        {
            *raw = api::project_artwork(host.clone(), project.clone())
                .await
                .ok()
                .flatten()
                .and_then(|png| artwork::raw_from_png(&png));
        }
    }
    Ok(icons)
}

/// Extensions the core's content importer and the Slint picker accept.
const CONTENT_EXTENSIONS: [&str; 2] = ["jar", "zip"];

fn extension_is(path: &std::path::Path, wanted: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| wanted.iter().any(|w| extension.eq_ignore_ascii_case(w)))
}

/// Classifies files dropped onto the Content page. Exactly one `.jar` or
/// `.zip` (the Slint picker's filter) becomes a preview path; the core still
/// validates the archive itself. Anything else is refused with the reason.
fn classify_content_drop(paths: &[PathBuf]) -> Result<String, &'static str> {
    match paths {
        [] => Err("Drop a .jar or .zip file to import it."),
        [path] if extension_is(path, &CONTENT_EXTENSIONS) => path
            .to_str()
            .map(str::to_owned)
            .ok_or("That file's path can't be read. Move the file and try again."),
        [path] if extension_is(path, &["mrpack"]) => {
            Err("Modpacks (.mrpack) are imported from the Instances page.")
        }
        [_] => Err("Only .jar and .zip files can be imported as content."),
        _ => Err("Drop one file at a time."),
    }
}

fn review_allowed(v: &Value) -> bool {
    v["blockers"].as_array().is_some_and(|b| b.is_empty())
        && v["ticket"].as_str().is_some_and(|s| !s.is_empty())
        && v["fingerprint"].as_str().is_some_and(|s| !s.is_empty())
}
fn current(a: &Target, b: &Target) -> bool {
    a == b
}
fn inventory_current(serial: u64, active: u64, now: Option<&Target>, target: &Target) -> bool {
    serial == active && now == Some(target)
}
fn preview_current(
    now: Option<&Target>,
    target: &Target,
    serial: u64,
    expected: u64,
    path: &str,
    expected_path: &str,
    busy: bool,
) -> bool {
    now.is_some_and(|n| current(n, target)) && serial == expected && path == expected_path && !busy
}
enum Reply {
    State(u64, api::ApiResult<api::LauncherState>),
    Inventory(u64, Target, api::ApiResult<Vec<Entry>>),
    Preview(Target, u64, String, api::ApiResult<Value>),
    Mutation(Target, api::ApiResult<()>),
    Folder(Target, api::ApiResult<()>),
    /// One icon batch: the serial it was requested under, its keys, results.
    Icons(u64, Target, Vec<String>, Result<IconResults, ()>),
}

pub struct Content {
    host: Option<Host>,
    runtime: Handle,
    tx: async_channel::Sender<Reply>,
    state_serial: u64,
    inventory_serial: u64,
    state: Option<api::LauncherState>,
    selected: Option<String>,
    kind: Kind,
    revision: u64,
    entries: Vec<Entry>,
    icons: IconBook,
    review_serial: u64,
    preview_request: Option<u64>,
    busy: bool,
    loading: bool,
    message: String,
    path: Entity<InputState>,
    path_value: String,
    _path_events: Subscription,
    review: Option<Review>,
    removing: Option<(Target, Entry)>,
    updates: Entity<ContentUpdates>,
    _update_events: Subscription,
}
impl Content {
    pub fn new(
        host: Option<Host>,
        runtime: Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (tx, rx) = async_channel::unbounded();
        let path =
            cx.new(|cx| InputState::new(window, cx).placeholder("Path to a local content file"));
        let subscription = cx.subscribe(&path, |this: &mut Self, input, _: &InputEvent, cx| {
            let value = input.read(cx).value().to_string();
            if value != this.path_value {
                this.path_value = value;
                this.invalidate_review();
                cx.notify();
            }
        });
        let updates = cx.new(|cx| ContentUpdates::new(host.clone(), runtime.clone(), window, cx));
        let update_events = cx.subscribe(
            &updates,
            |this: &mut Self, _, event: &ContentUpdatesEvent, cx| {
                match event {
                    ContentUpdatesEvent::BusyChanged(true) => {
                        this.invalidate_review();
                        this.removing = None;
                    }
                    ContentUpdatesEvent::Changed => {
                        // The child rechecks its own settled mutation. Preserve its
                        // target revision while giving inventories a new read serial.
                        this.invalidate_review();
                        this.removing = None;
                        this.load();
                        this.load_registry();
                        cx.emit(ContentEvent::Changed);
                    }
                    ContentUpdatesEvent::BusyChanged(false) => {}
                }
                cx.notify();
            },
        );
        let cleanup_host = host.clone();
        let cleanup_runtime = runtime.clone();
        cx.spawn(async move |weak, cx| {
            while let Ok(reply) = rx.recv().await {
                let cleanup = match &reply {
                    Reply::Preview(t, _, _, Ok(v)) => v["ticket"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(|ticket| (t.instance.clone(), ticket.to_owned())),
                    _ => None,
                };
                if weak
                    .update(cx, |this: &mut Self, cx| {
                        this.receive(reply, cx);
                        cx.notify();
                    })
                    .is_err()
                {
                    if let (Some(host), Some((instance, ticket))) = (cleanup_host.clone(), cleanup)
                    {
                        cleanup_runtime.spawn(async move {
                            let _ =
                                guarded(api::discard_local_content_review(host, instance, ticket))
                                    .await;
                        });
                    }
                    // Drain any successful previews already queued, revoking every ticket.
                }
            }
        })
        .detach();
        let mut this = Self {
            host,
            runtime,
            tx,
            state_serial: 0,
            inventory_serial: 0,
            state: None,
            selected: None,
            kind: Kind::Mod,
            revision: 0,
            entries: Vec::new(),
            icons: IconBook::new(),
            review_serial: 0,
            preview_request: None,
            busy: false,
            loading: false,
            message: String::new(),
            path,
            path_value: String::new(),
            _path_events: subscription,
            review: None,
            removing: None,
            updates,
            _update_events: update_events,
        };
        this.refresh(cx);
        this
    }
    fn target(&self) -> Option<Target> {
        Some(Target {
            instance: self.selected.clone()?,
            kind: self.kind,
            revision: self.revision,
        })
    }
    fn is_busy(&self, cx: &gpui::App) -> bool {
        self.busy || self.updates.read(cx).is_busy()
    }
    fn sync_updates(&self, cx: &mut Context<Self>) {
        let instance = self
            .state
            .as_ref()
            .and_then(|state| {
                state
                    .instances
                    .iter()
                    .find(|instance| Some(&instance.id) == self.selected.as_ref())
            })
            .cloned();
        self.updates.update(cx, |updates, cx| {
            updates.set_target(instance, self.kind.api(), self.revision, cx);
            updates.set_blocked(self.busy || self.preview_request.is_some(), cx);
        });
    }
    fn discard(&self, instance: String, ticket: String) {
        if ticket.is_empty() {
            return;
        }
        if let Some(host) = self.host.clone() {
            self.runtime.spawn(async move {
                let _ = guarded(api::discard_local_content_review(host, instance, ticket)).await;
            });
        }
    }
    fn invalidate_review(&mut self) {
        self.review_serial = self.review_serial.wrapping_add(1);
        if let Some(r) = self.review.take() {
            self.discard(r.target.instance, r.ticket);
        }
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.host.is_none() {
            self.message = "Content management is unavailable in diagnostic mode.".into();
            cx.notify();
            return;
        }
        self.load_registry();
        if !self.busy {
            self.invalidate_review();
            self.removing = None;
            self.revision = self.revision.wrapping_add(1);
            self.load();
            self.sync_updates(cx);
        }
    }
    fn load_registry(&mut self) {
        let Some(host) = self.host.clone() else {
            return;
        };
        self.state_serial = self.state_serial.wrapping_add(1);
        let serial = self.state_serial;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = guarded(api::launcher_state(host)).await;
            let _ = tx.send(Reply::State(serial, r)).await;
        });
    }
    pub fn on_host_event(&mut self, name: &str, payload: &Value, cx: &mut Context<Self>) {
        self.updates
            .update(cx, |updates, cx| updates.on_host_event(name, payload, cx));
        if name == "launch-state"
            && matches!(payload["status"].as_str(), Some("exited" | "failed"))
            && !self.busy
        {
            self.refresh(cx);
        }
    }
    fn load(&mut self) {
        let (Some(host), Some(target)) = (self.host.clone(), self.target()) else {
            return;
        };
        self.loading = true;
        self.entries.clear();
        self.icons.supersede();
        self.inventory_serial = self.inventory_serial.wrapping_add(1);
        let serial = self.inventory_serial;
        let tx = self.tx.clone();
        let t = target.clone();
        self.runtime.spawn(async move {
            let result = if t.kind == Kind::Mod {
                guarded(api::instance_mods(host, t.instance.clone()))
                    .await
                    .map(mods)
            } else {
                guarded(api::instance_content(
                    host,
                    t.instance.clone(),
                    t.kind.api(),
                ))
                .await
                .map(packs)
            };
            let _ = tx.send(Reply::Inventory(serial, t, result)).await;
        });
    }
    fn receive(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::State(s, result) if s == self.state_serial && !self.busy => match result {
                Ok(state) => {
                    if self
                        .selected
                        .as_ref()
                        .is_some_and(|id| !state.instances.iter().any(|i| &i.id == id))
                    {
                        self.invalidate_review();
                        self.selected = None;
                        self.entries.clear();
                        self.icons.reset();
                        self.removing = None;
                        self.loading = false;
                        self.revision = self.revision.wrapping_add(1);
                    }
                    self.state = Some(state);
                }
                Err(e) => self.message = error(e),
            },
            Reply::State(..) => (),
            Reply::Inventory(serial, t, r)
                if inventory_current(serial, self.inventory_serial, self.target().as_ref(), &t) =>
            {
                self.loading = false;
                match r {
                    Ok(entries) => {
                        self.entries = entries;
                        self.icons.retain_for(&self.entries);
                        self.load_icons();
                    }
                    Err(e) => self.message = error(e),
                }
            }
            Reply::Inventory(..) => (),
            Reply::Icons(serial, t, keys, result) => {
                if self.target().as_ref() == Some(&t)
                    && self.icons.apply(serial, &self.entries, keys, result)
                {
                    self.load_icons();
                }
            }
            Reply::Preview(t, serial, path, r) => {
                if self.preview_request == Some(serial) {
                    self.preview_request = None;
                }
                match r {
                    Ok(value) => {
                        let ticket = value["ticket"].as_str().unwrap_or("").to_owned();
                        if !preview_current(
                            self.target().as_ref(),
                            &t,
                            self.review_serial,
                            serial,
                            &self.path_value,
                            &path,
                            self.busy,
                        ) {
                            self.discard(t.instance, ticket);
                        } else {
                            let fingerprint =
                                value["fingerprint"].as_str().unwrap_or("").to_owned();
                            self.review = Some(Review {
                                target: t,
                                path,
                                ticket,
                                fingerprint,
                                value,
                            });
                            self.message.clear();
                        }
                    }
                    Err(e) => {
                        if self.target().as_ref().is_some_and(|now| current(now, &t))
                            && self.review_serial == serial
                            && self.path_value == path
                        {
                            self.message = error(e);
                        }
                    }
                }
            }
            Reply::Mutation(t, r) => {
                self.busy = false;
                self.removing = None;
                if self.target().as_ref().is_some_and(|now| current(now, &t)) {
                    match r {
                        Ok(()) => {
                            self.message = "Content updated.".into();
                            self.removing = None;
                            self.invalidate_review();
                            self.revision = self.revision.wrapping_add(1);
                            self.load();
                        }
                        Err(e) => {
                            self.message = error(e);
                            self.invalidate_review();
                            self.revision = self.revision.wrapping_add(1);
                            self.load();
                        }
                    }
                }
                cx.emit(ContentEvent::Changed);
            }
            Reply::Folder(t, r) => {
                if self.target().as_ref().is_some_and(|now| current(now, &t)) {
                    self.message = match r {
                        Ok(()) => "Folder opened.".into(),
                        Err(e) => error(e),
                    };
                }
            }
        }
        self.sync_updates(cx);
    }
    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        if self.is_busy(cx) {
            return;
        }
        self.invalidate_review();
        self.revision = self.revision.wrapping_add(1);
        self.selected = Some(id);
        self.removing = None;
        self.icons.reset();
        self.load();
        self.sync_updates(cx);
    }
    /// Shows `instance` on the `kind` tab (Discover's "Back to instance").
    /// Like `select`, it waits out a running mutation and says so instead of
    /// silently keeping the old selection.
    pub fn show(&mut self, instance: String, kind: i32, cx: &mut Context<Self>) {
        if self.is_busy(cx) {
            self.message =
                "Finish the current content operation, then choose the instance again.".into();
            cx.notify();
            return;
        }
        self.invalidate_review();
        self.revision = self.revision.wrapping_add(1);
        self.selected = Some(instance);
        self.kind = Kind::from_index(kind);
        self.removing = None;
        self.icons.reset();
        self.load();
        self.sync_updates(cx);
        cx.notify();
    }
    fn find_more(&mut self, cx: &mut Context<Self>) {
        if let Some(instance) = self.selected.clone() {
            cx.emit(ContentEvent::Discover {
                instance,
                kind: self.kind.index(),
            });
        }
    }
    fn select_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        if self.is_busy(cx) || self.kind == kind {
            return;
        }
        self.invalidate_review();
        self.revision = self.revision.wrapping_add(1);
        self.kind = kind;
        self.removing = None;
        self.icons.reset();
        self.load();
        self.sync_updates(cx);
    }
    fn preview(&mut self, cx: &mut Context<Self>) {
        if self.is_busy(cx) || self.preview_request.is_some() || self.path_value.trim().is_empty() {
            return;
        }
        self.invalidate_review();
        let (Some(host), Some(t)) = (self.host.clone(), self.target()) else {
            return;
        };
        let path = self.path_value.clone();
        let serial = self.review_serial;
        self.preview_request = Some(serial);
        self.sync_updates(cx);
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let result = guarded(api::preview_local_content(
                host.clone(),
                t.instance.clone(),
                path.clone(),
            ))
            .await;
            if let Err(e) = tx
                .send(Reply::Preview(t.clone(), serial, path, result))
                .await
            {
                if let Reply::Preview(_, _, _, Ok(v)) = e.0 {
                    if let Some(ticket) = v["ticket"].as_str().filter(|s| !s.is_empty()) {
                        let _ = guarded(api::discard_local_content_review(
                            host,
                            t.instance,
                            ticket.to_owned(),
                        ))
                        .await;
                    }
                }
            }
        });
    }
    fn mutate(&mut self, entry: Option<Entry>, install: bool, cx: &mut Context<Self>) {
        if self.is_busy(cx) || (!install && entry.is_none()) {
            return;
        }
        let (Some(host), Some(t)) = (self.host.clone(), self.target()) else {
            return;
        };
        let review = if install {
            let Some(r) = self.review.take() else {
                return;
            };
            if !current(&r.target, &t) || r.path != self.path_value || !review_allowed(&r.value) {
                self.discard(r.target.instance, r.ticket);
                return;
            }
            Some(r)
        } else {
            None
        };
        if let Some(e) = &entry {
            if !e.remove
                || !self
                    .entries
                    .iter()
                    .any(|current| current.id == e.id && current.remove)
            {
                return;
            }
        }
        self.invalidate_review();
        self.revision = self.revision.wrapping_add(1);
        let t = self.target().unwrap();
        self.busy = true;
        self.sync_updates(cx);
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let result = if let Some(r) = review {
                let result = guarded(api::install_local_content(
                    host.clone(),
                    t.instance.clone(),
                    r.ticket.clone(),
                    r.fingerprint,
                ))
                .await
                .map(|_| ());
                let _ = guarded(api::discard_local_content_review(
                    host,
                    t.instance.clone(),
                    r.ticket,
                ))
                .await;
                result
            } else if let Some(e) = entry {
                if t.kind == Kind::Mod {
                    guarded(api::remove_mod(host, t.instance.clone(), e.id))
                        .await
                        .map(|_| ())
                } else {
                    guarded(api::remove_content(
                        host,
                        t.instance.clone(),
                        t.kind.api(),
                        e.id,
                    ))
                    .await
                    .map(|_| ())
                }
            } else {
                return;
            };
            let _ = tx.send(Reply::Mutation(t, result)).await;
        });
    }
    fn toggle(&mut self, target: &Target, entry: Entry, cx: &mut Context<Self>) {
        if self.is_busy(cx)
            || self.target().as_ref() != Some(target)
            || !entry.toggle
            || entry.enabled.is_none()
            || !self.entries.iter().any(|current| {
                current.id == entry.id && current.toggle && current.enabled == entry.enabled
            })
        {
            return;
        }
        self.invalidate_review();
        self.revision = self.revision.wrapping_add(1);
        let (Some(host), Some(t)) = (self.host.clone(), self.target()) else {
            return;
        };
        self.busy = true;
        self.sync_updates(cx);
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let enabled = !entry.enabled.unwrap();
            let r = if t.kind == Kind::Mod {
                guarded(api::set_mod_enabled(
                    host,
                    t.instance.clone(),
                    entry.id,
                    enabled,
                ))
                .await
                .map(|_| ())
            } else {
                guarded(api::set_pack_enabled(
                    host,
                    t.instance.clone(),
                    t.kind.api(),
                    entry.id,
                    enabled,
                ))
                .await
                .map(|_| ())
            };
            let _ = tx.send(Reply::Mutation(t, r)).await;
        });
    }
    /// Requests the next icon batch for the settled inventory, if any rows
    /// still lack one. Runs after an inventory read and after each batch, so
    /// there is never more than one core call in flight and nothing polls.
    fn load_icons(&mut self) {
        let (Some(host), Some(target)) = (self.host.clone(), self.target()) else {
            return;
        };
        let Some((serial, batch)) = self.icons.next_batch(&self.entries) else {
            return;
        };
        let keys: Vec<String> = batch.iter().map(|r| r.source.key()).collect();
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let work = target.clone();
            let result = guarded(fetch_icons(host, work, batch))
                .await
                .map_err(|_| ());
            let _ = tx.send(Reply::Icons(serial, target, keys, result)).await;
        });
    }
    /// A file dropped from the file manager: the same preview-ticket review
    /// as the path field, never an install. The core validates the archive.
    fn drop_files(&mut self, paths: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        self.message = if self.selected.is_none() {
            "Choose an instance before importing a file.".into()
        } else {
            match classify_content_drop(paths) {
                Err(reason) => reason.into(),
                Ok(_) if self.is_busy(cx) || self.preview_request.is_some() => {
                    "Finish the current operation, then drop the file again.".into()
                }
                Ok(path) => {
                    self.message.clear();
                    // Keep the field and the reviewed path in step so the
                    // preview guard accepts this review.
                    self.path_value = path.clone();
                    self.path.update(cx, |p, cx| p.set_value(path, window, cx));
                    self.preview(cx);
                    cx.notify();
                    return;
                }
            }
        };
        cx.notify();
    }
    fn folder(&self) {
        let (Some(host), Some(t)) = (self.host.clone(), self.target()) else {
            return;
        };
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let r = if t.kind == Kind::Mod {
                guarded(api::open_mods_folder(host, t.instance.clone())).await
            } else {
                guarded(api::open_content_folder(
                    host,
                    t.instance.clone(),
                    t.kind.api(),
                ))
                .await
            };
            let _ = tx.send(Reply::Folder(t, r)).await;
        });
    }
}
impl Drop for Content {
    fn drop(&mut self) {
        if let Some(r) = self.review.take() {
            self.discard(r.target.instance, r.ticket);
        }
    }
}
fn error(e: api::ApiError) -> String {
    format!("{}: {}", e.code, e.message)
}
impl Render for Content {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let busy = self.is_busy(cx);
        let previewing = self.preview_request.is_some();
        let palette = crate::theme::theme(cx).clone();
        let available = self.host.is_some();
        self.path.update(cx, |p, cx| p.set_disabled(busy, cx));
        // Files dropped from the file manager feed the local import review.
        // The outline shows whether the dragged file would be accepted.
        let (accept, reject) = (palette.accent, palette.danger);
        let mut root = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .border_1()
            .border_color(gpui::transparent_black())
            .rounded(palette.radius)
            .drag_over::<ExternalPaths>(move |style, paths, _, _| {
                style.border_color(if classify_content_drop(paths.paths()).is_ok() {
                    accept
                } else {
                    reject
                })
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.drop_files(paths.paths(), window, cx);
            }))
            .child(div().text_2xl().child("Content"))
            .child(self.message.clone());
        if let Some(state) = &self.state {
            let mut instances = div().flex().flex_wrap().gap(px(8.));
            for instance in &state.instances {
                let id = instance.id.clone();
                let v = view.clone();
                instances = instances.child(
                    AuroraButton::new(
                        gpui::SharedString::from(format!("content-instance-{id}")),
                        instance.display_name.clone(),
                    )
                    .selected(self.selected.as_deref() == Some(&id))
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        v.update(cx, |s, cx| {
                            s.select(id.clone(), cx);
                            cx.notify();
                        });
                    }),
                );
            }
            root = root.child(instances);
        }
        if self.selected.is_some() {
            let mut toolbar = div().flex().flex_wrap().gap(px(8.));
            for kind in [Kind::Mod, Kind::Resource, Kind::Shader] {
                let v = view.clone();
                toolbar = toolbar.child(
                    AuroraButton::new(kind.api(), kind.label())
                        .selected(self.kind == kind)
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            v.update(cx, |s, cx| {
                                s.select_kind(kind, cx);
                                cx.notify();
                            });
                        }),
                );
            }
            let v = view.clone();
            toolbar = toolbar.child(
                AuroraButton::new("content-find-more", "Find more").on_click(move |_, _, cx| {
                    v.update(cx, |s, cx| s.find_more(cx));
                }),
            );
            let v = view.clone();
            toolbar = toolbar.child(
                AuroraButton::new("content-folder", "Open folder")
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        v.read(cx).folder();
                    }),
            );
            let v = view.clone();
            toolbar = toolbar.child(
                AuroraButton::new("content-retry", "Refresh content")
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        v.update(cx, |s, cx| s.refresh(cx));
                    }),
            );
            root = root.child(toolbar);
            if self.loading {
                root = root.child("Loading content…");
            } else if self.entries.is_empty() {
                root = root.child("No content found.");
            }
            for entry in &self.entries {
                let e = entry.clone();
                let v = view.clone();
                let target = self.target().unwrap();
                let icon = entry
                    .icon
                    .as_ref()
                    .and_then(|icon| self.icons.cache.image(&icon.key()));
                let mut row = div().flex().flex_col().gap(px(4.)).child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(12.))
                        .child(artwork::tile(
                            gpui::SharedString::from(format!("icon-{}", entry.id)),
                            icon,
                            &entry.letter,
                            px(40.),
                            &palette,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(entry.name.clone())
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(palette.muted_foreground)
                                        .child(entry.file_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(palette.muted_foreground)
                                        .child(entry.details.clone()),
                                ),
                        ),
                );
                let mut actions = div().flex().gap(px(8.)).child(
                    AuroraButton::new(
                        gpui::SharedString::from(format!("toggle-{}", entry.id)),
                        if entry.enabled == Some(true) {
                            "Disable"
                        } else {
                            "Enable"
                        },
                    )
                    .disabled(busy || !entry.toggle)
                    .on_click(move |_, _, cx| {
                        v.update(cx, |s, cx| {
                            s.toggle(&target, e.clone(), cx);
                            cx.notify();
                        });
                    }),
                );
                let e = entry.clone();
                let v = view.clone();
                let target = self.target().unwrap();
                actions = actions.child(
                    AuroraButton::new(
                        gpui::SharedString::from(format!("remove-{}", entry.id)),
                        "Remove…",
                    )
                    .disabled(busy || !entry.remove)
                    .on_click(move |_, _, cx| {
                        v.update(cx, |s, cx| {
                            if !s.is_busy(cx)
                                && s.target().as_ref() == Some(&target)
                                && s.entries
                                    .iter()
                                    .any(|current| current.id == e.id && current.remove)
                            {
                                s.removing = Some((target.clone(), e.clone()));
                            }
                            cx.notify();
                        });
                    }),
                );
                row = row.child(actions);
                root = root.child(row);
            }
            root = root
                .child("Import local content")
                .child(AuroraInput::new(&self.path).label("Local file path"));
            let v = view.clone();
            root = root.child(
                AuroraButton::new(
                    "preview-local",
                    if previewing {
                        "Reading file…"
                    } else {
                        "Preview file"
                    },
                )
                .disabled(busy || previewing || self.path_value.trim().is_empty())
                .on_click(move |_, _, cx| {
                    v.update(cx, |s, cx| {
                        s.preview(cx);
                        cx.notify();
                    });
                }),
            );
            if let Some(review) = &self.review {
                let value = &review.value;
                let mut description = format!(
                    "{}\nTarget: {}\nType: {} · {} bytes\n{}",
                    value["fileName"].as_str().unwrap_or("File"),
                    value["instanceName"].as_str().unwrap_or("Instance"),
                    value["contentType"].as_str().unwrap_or("Unknown"),
                    value["sizeBytes"].as_u64().unwrap_or(0),
                    value["note"].as_str().unwrap_or("")
                );
                for key in ["warnings", "blockers"] {
                    if let Some(items) = value[key].as_array() {
                        for item in items {
                            description.push_str(&format!(
                                "\n{}: {}",
                                if key == "blockers" {
                                    "Blocked"
                                } else {
                                    "Warning"
                                },
                                item.as_str()
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| item.to_string())
                            ));
                        }
                    }
                }
                let allowed = review_allowed(value)
                    && review.path == self.path_value
                    && self.target().as_ref() == Some(&review.target);
                root = root.child(description);
                let v = view.clone();
                root = root.child(
                    AuroraButton::new("install-local", "Install reviewed file")
                        .disabled(busy || !allowed)
                        .on_click(move |_, _, cx| {
                            v.update(cx, |s, cx| {
                                s.mutate(None, true, cx);
                                cx.notify();
                            });
                        }),
                );
                let v = view.clone();
                root = root.child(
                    AuroraButton::new("discard-local", "Close review")
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            v.update(cx, |s, cx| {
                                s.invalidate_review();
                                cx.notify();
                            });
                        }),
                );
            }
            root = root.child(self.updates.clone());
        } else if available {
            root = root.child("Choose an instance to manage its content.");
        }
        let cancel = view.clone();
        let confirm = view.clone();
        let changed = view.clone();
        let veto = view.clone();
        let name = self
            .removing
            .as_ref()
            .map(|(_, e)| format!("{} ({})", e.name, e.file_name))
            .unwrap_or_default();
        let panel = div()
            .w(px(440.))
            .p(px(24.))
            .bg(palette.surface)
            .text_color(palette.foreground)
            .rounded(palette.radius)
            .flex()
            .flex_col()
            .gap(px(12.))
            .child("Remove content")
            .child(format!(
                "Remove {name}? This removes the selected entry from this instance."
            ))
            .child(self.message.clone())
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        AuroraButton::new("remove-cancel", "Cancel")
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                cancel.update(cx, |s, cx| {
                                    s.removing = None;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        AuroraButton::new("remove-confirm", "Remove")
                            .variant(ButtonVariant::Danger)
                            .disabled(busy)
                            .on_click(move |_, _, cx| {
                                confirm.update(cx, |s, cx| {
                                    if let Some((target, e)) = s.removing.clone() {
                                        if s.target().as_ref() == Some(&target) {
                                            s.mutate(Some(e), false, cx);
                                        }
                                    }
                                    cx.notify();
                                });
                            }),
                    ),
            );
        root.child(
            Dialog::new(cx)
                .open(self.removing.is_some())
                .accessibility_label("Remove content")
                .close_on_backdrop_press(false)
                .on_ok(|_, _, _| false)
                .on_cancel(move |_, _, cx| !veto.read(cx).is_busy(cx))
                .on_open_change(move |open, _, _, cx| {
                    if !open {
                        changed.update(cx, |s, cx| {
                            s.removing = None;
                            cx.notify();
                        });
                    }
                })
                .backdrop(div().absolute().size_full().bg(gpui::rgba(0x000000a6)))
                .popup(panel),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn content_tab_indexes_round_trip_and_clamp_like_slint() {
        for kind in [Kind::Mod, Kind::Resource, Kind::Shader] {
            assert!(Kind::from_index(kind.index()) == kind);
        }
        assert!(Kind::from_index(-4) == Kind::Mod);
        assert!(Kind::from_index(3) == Kind::Shader);
    }
    #[test]
    fn targets_reject_changed_instance_kind_and_revision() {
        let t = Target {
            instance: "a".into(),
            kind: Kind::Mod,
            revision: 1,
        };
        let mut n = t.clone();
        assert!(current(&t, &n));
        n.revision += 1;
        assert!(!current(&t, &n));
        n = t.clone();
        n.instance = "b".into();
        assert!(!current(&t, &n));
        n = t.clone();
        n.kind = Kind::Shader;
        assert!(!current(&t, &n));
    }
    #[test]
    fn a_late_inventory_cannot_undo_a_new_read_for_the_same_target() {
        let target = Target {
            instance: "a".into(),
            kind: Kind::Mod,
            revision: 4,
        };
        assert!(!inventory_current(1, 2, Some(&target), &target));
        assert!(inventory_current(2, 2, Some(&target), &target));
        let changed = Target {
            revision: 5,
            ..target.clone()
        };
        assert!(!inventory_current(2, 2, Some(&changed), &target));
    }
    #[test]
    fn preview_guard_rejects_superseded_tickets() {
        let t = Target {
            instance: "a".into(),
            kind: Kind::Mod,
            revision: 2,
        };
        assert!(preview_current(Some(&t), &t, 3, 3, "a.jar", "a.jar", false));
        assert!(!preview_current(
            Some(&t),
            &t,
            4,
            3,
            "a.jar",
            "a.jar",
            false
        ));
        assert!(!preview_current(
            Some(&t),
            &t,
            3,
            3,
            "b.jar",
            "a.jar",
            false
        ));
        assert!(!preview_current(Some(&t), &t, 3, 3, "a.jar", "a.jar", true));
        assert!(!preview_current(None, &t, 3, 3, "a.jar", "a.jar", false));
    }
    #[test]
    fn review_requires_explicit_empty_blockers_and_credentials() {
        let mut v = serde_json::json!({"ticket":"t","fingerprint":"f","blockers":[]});
        assert!(review_allowed(&v));
        v["blockers"] = serde_json::json!(["conflict"]);
        assert!(!review_allowed(&v));
        v["blockers"] = Value::Null;
        assert!(!review_allowed(&v));
        v["blockers"] = serde_json::json!([]);
        v["ticket"] = serde_json::json!("");
        assert!(!review_allowed(&v));
    }
    fn png_entry(id: &str, hash: Option<&str>, project: Option<&str>) -> Entry {
        Entry {
            id: id.into(),
            name: id.into(),
            file_name: format!("{id}.zip"),
            details: String::new(),
            enabled: Some(true),
            toggle: true,
            remove: true,
            letter: artwork::letter(id),
            icon: Some(IconSource::Pack(
                api::PackIconRequest {
                    file_name: format!("{id}.zip"),
                    sha256: hash.map(str::to_owned),
                },
                format!("pack:{id}:{}", hash.unwrap_or("")),
            )),
            project: project.map(str::to_owned),
        }
    }
    fn mod_entry(id: &str) -> Entry {
        Entry {
            icon: Some(IconSource::Mod(api::ModIconRequest {
                file_name: format!("{id}.jar"),
                sha256: format!("hash-{id}"),
            })),
            ..png_entry(id, None, None)
        }
    }
    fn art() -> Option<RawArtwork> {
        Some(RawArtwork {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 255],
        })
    }
    fn keys(batch: &[IconRequest]) -> Vec<String> {
        batch.iter().map(|r| r.source.key()).collect()
    }

    #[test]
    fn icon_batches_are_bounded_unique_and_one_at_a_time() {
        let mut book = IconBook::new();
        let mut entries: Vec<Entry> = (0..12).map(|i| mod_entry(&format!("m{i}"))).collect();
        entries.push(mod_entry("m0"));
        entries.push(Entry {
            icon: None,
            ..mod_entry("plain")
        });
        let (serial, batch) = book.next_batch(&entries).unwrap();
        assert_eq!(batch.len(), ICON_BATCH);
        assert_eq!(keys(&batch)[0], "hash-m0");
        assert!(book.next_batch(&entries).is_none(), "one batch in flight");
        assert!(book.apply(serial, &entries, keys(&batch), Ok(vec![])));
        // Cached keys are excluded and the remainder follows in the next batch.
        let done: IconResults = keys(&batch).into_iter().map(|k| (k, art())).collect();
        assert!(book.apply(serial, &entries, vec![], Ok(done)));
        let (_, next) = book.next_batch(&entries).unwrap();
        assert_eq!(keys(&next), ["hash-m8", "hash-m9", "hash-m10", "hash-m11"]);
    }

    #[test]
    fn a_batch_for_superseded_inventory_is_dropped() {
        let mut book = IconBook::new();
        let entries = vec![mod_entry("a")];
        let (serial, batch) = book.next_batch(&entries).unwrap();
        // A new inventory read (refresh, instance or kind change) lands first.
        book.supersede();
        assert!(!book.apply(
            serial,
            &entries,
            keys(&batch),
            Ok(vec![("hash-a".into(), art())])
        ));
        assert!(!book.cache.contains(&"hash-a".to_owned()));
        // The stale reply did not clear the new read's in-flight state either.
        let (fresh, _) = book.next_batch(&entries).unwrap();
        assert_ne!(fresh, serial);
        assert!(book.next_batch(&entries).is_none());
        assert!(!book.apply(serial, &entries, vec![], Err(())));
        assert!(book.next_batch(&entries).is_none(), "still loading");
    }

    #[test]
    fn failures_and_absent_icons_do_not_retry_until_the_next_read() {
        let mut book = IconBook::new();
        let entries = vec![mod_entry("a"), mod_entry("b")];
        let (serial, batch) = book.next_batch(&entries).unwrap();
        assert!(book.apply(
            serial,
            &entries,
            keys(&batch),
            Ok(vec![("hash-a".into(), None), ("hash-b".into(), art())])
        ));
        // Absent is remembered as absent, so it is neither shown nor requested.
        assert!(book.cache.image(&"hash-a".to_owned()).is_none());
        assert!(book.cache.image(&"hash-b".to_owned()).is_some());
        assert!(book.next_batch(&entries).is_none());

        let mut failing = IconBook::new();
        let (serial, batch) = failing.next_batch(&entries).unwrap();
        assert!(failing.apply(serial, &entries, keys(&batch), Err(())));
        assert!(failing.next_batch(&entries).is_none());
        failing.supersede();
        assert!(failing.next_batch(&entries).is_some());
    }

    #[test]
    fn reads_keep_hashed_icons_and_refetch_directory_packs() {
        let mut book = IconBook::new();
        let entries = vec![
            png_entry("hashed", Some("h1"), None),
            png_entry("folder", None, None),
            png_entry("gone", Some("h2"), None),
        ];
        let (serial, batch) = book.next_batch(&entries).unwrap();
        let all: IconResults = keys(&batch).into_iter().map(|k| (k, art())).collect();
        assert!(book.apply(serial, &entries, vec![], Ok(all)));
        let kept = book.cache.image(&"pack:hashed:h1".to_owned()).unwrap();
        book.supersede();
        book.retain_for(&entries[..2]);
        assert!(Arc::ptr_eq(
            &kept,
            &book.cache.image(&"pack:hashed:h1".to_owned()).unwrap()
        ));
        assert!(!book.cache.contains(&"pack:folder:".to_owned()));
        assert!(!book.cache.contains(&"pack:gone:h2".to_owned()));
        // Switching instance or kind forgets everything.
        book.reset();
        assert_eq!(book.cache.len(), 0);
    }

    #[test]
    fn only_leading_rows_request_icons_and_the_cache_is_capped() {
        let mut book = IconBook::new();
        let entries: Vec<Entry> = (0..ICON_CACHE + 10)
            .map(|i| mod_entry(&format!("m{i}")))
            .collect();
        for _ in 0..ICON_CACHE / ICON_BATCH {
            let (serial, batch) = book.next_batch(&entries).unwrap();
            let done: IconResults = keys(&batch).into_iter().map(|k| (k, art())).collect();
            assert!(book.apply(serial, &entries, vec![], Ok(done)));
        }
        assert_eq!(book.cache.len(), ICON_CACHE);
        assert!(
            book.next_batch(&entries).is_none(),
            "trailing rows keep their glyph"
        );
        // A shorter list after a read keeps the bound even if more arrive.
        assert!(book.cache.len() <= ICON_CACHE);
    }

    #[test]
    fn mod_and_pack_rows_request_icons_from_the_expected_sources() {
        let inv: api::ContentInventory = serde_json::from_value(serde_json::json!({"entries":[
            {"entryId":"a","fileName":"a.zip","displayName":"A","fileType":"zip","sha256":"h",
             "provenance":{"provider":"modrinth","projectId":"P1","versionId":"v"},
             "management":{"canRemove":true,"canToggle":true,"active":true,"activationManagedInGame":false}},
            {"entryId":"b","fileName":"b.txt","displayName":"B","fileType":"file",
             "management":{"canRemove":true,"canToggle":true,"active":true,"activationManagedInGame":false}},
            {"entryId":"c","fileName":"c","displayName":"C","fileType":"directory",
             "provenance":{"provider":"other","projectId":"X","versionId":"v"},
             "management":{"canRemove":true,"canToggle":true,"active":true,"activationManagedInGame":false}}
        ]}))
        .unwrap();
        let rows = packs(inv);
        assert_eq!(rows[0].icon.as_ref().unwrap().key(), "pack:a:h");
        assert!(rows[0].icon.as_ref().unwrap().stable());
        assert_eq!(rows[0].project.as_deref(), Some("P1"));
        assert!(
            rows[1].icon.is_none(),
            "only archives and directories carry pack.png"
        );
        assert!(!rows[2].icon.as_ref().unwrap().stable());
        assert!(
            rows[2].project.is_none(),
            "only Modrinth provenance falls back to artwork"
        );
        assert_eq!(rows[0].letter, "A");
    }

    fn drop_of(names: &[&str]) -> Result<String, &'static str> {
        classify_content_drop(&names.iter().map(PathBuf::from).collect::<Vec<_>>())
    }

    #[test]
    fn dropped_content_accepts_one_jar_or_zip_case_insensitively() {
        assert_eq!(
            drop_of(&["C:\\mods\\sodium.jar"]).unwrap(),
            "C:\\mods\\sodium.jar"
        );
        assert!(drop_of(&["pack.ZIP"]).is_ok());
        assert!(drop_of(&["Mod.Jar"]).is_ok());
    }

    #[test]
    fn dropped_content_rejects_everything_else_with_a_reason() {
        assert_eq!(drop_of(&[]), Err("Drop a .jar or .zip file to import it."));
        assert_eq!(
            drop_of(&["a.jar", "b.jar"]),
            Err("Drop one file at a time.")
        );
        assert_eq!(
            drop_of(&["a.txt"]),
            Err("Only .jar and .zip files can be imported as content.")
        );
        assert_eq!(
            drop_of(&["no-extension"]),
            Err("Only .jar and .zip files can be imported as content.")
        );
        assert_eq!(
            drop_of(&["pack.mrpack"]),
            Err("Modpacks (.mrpack) are imported from the Instances page.")
        );
        assert_eq!(drop_of(&["a.jar.exe"]).is_err(), true);
    }

    #[test]
    fn permissions_are_core_owned_and_in_game_activation_is_not_toggleable() {
        let inv: api::ContentInventory = serde_json::from_value(serde_json::json!({"entries":[{"entryId":"id","fileName":"a.zip","displayName":"A","management":{"canRemove":false,"canToggle":true,"active":true,"activationManagedInGame":true}}]})).unwrap();
        let e = packs(inv).pop().unwrap();
        assert!(!e.toggle && !e.remove);
        assert!(e.details.contains("managed in game"));
    }
}
