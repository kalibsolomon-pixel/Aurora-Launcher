//! Modrinth Discover for the GPUI host.
//!
//! Behavior authority is the Slint controller `native/src/discover.rs` over the
//! shared typed boundary `native/src/api.rs`. The core owns installability,
//! compatibility, destinations and review fingerprints; this module only
//! presents them and freezes the target, kind, category, version, destination
//! and optional-file choices into typed requests before work leaves the UI
//! thread. Every asynchronous call goes through [`crate::api`] wrapped in
//! [`crate::backend::guarded`]; this module never performs downloads,
//! filesystem mutation or credential work itself.
//!
//! Installs run in the retained [`crate::downloads::Downloads`] queue this
//! view owns and exposes through [`Discover::downloads`]; the parent embeds
//! that entity wherever Downloads is presented. Submission is by typed
//! prepared jobs carrying the reviewed fingerprints and exact local-pack
//! pins; per-task retry, cancel and bounded history live in the queue, and a
//! settlement there drives this view's registry/inventory refresh and
//! `Changed` event. Searching, browsing and reviewing continue while
//! unrelated queued jobs run; busy projects and the busy target are marked
//! honestly.
//!
//! Result cards and the project details header show the project's icon from
//! `api::project_artwork` (the core's bounded, normalized PNG; the UI never
//! fetches an image itself). Requests run on the runtime behind a four-wide
//! gate, carry the result list's epoch so a replaced list drops both queued
//! work and late replies, and land in a capped retained cache; each image is
//! converted for GPUI once. Rows show a letter glyph until their icon arrives
//! or when the project has none.
//!
//! Differences from the Slint host are deliberate and disclosed in the module
//! report: review download progress shows the static preparation wording
//! instead of a polled byte counter, and aggregate live transfer byte
//! counters are not presented (see `crate::downloads`).

use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use aurora_launcher_lib::content_jobs::Control;
use aurora_launcher_lib::host::Host;
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::base::{Dialog, DialogChangeReason, DialogClose, DialogHandle};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Bounds, Context, Entity, EventEmitter, FocusHandle, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, SharedString,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div, px,
};
use serde_json::{Value, json};

use crate::artwork::{self, ArtworkCache, RawArtwork};
use crate::backend::guarded;
use crate::controls::{AuroraButton, AuroraInput, AuroraSelect, AuroraToggle, ButtonVariant};
use crate::downloads::{Downloads, DownloadsEvent, JobRequest, PreparedJob};
use crate::{api, theme};

/// Modrinth content kinds, in UI order. The third value is the modpack flow.
const KINDS: [&str; 4] = ["mod", "resourcePack", "shaderPack", "modpack"];
const KIND_LABELS: [&str; 4] = ["Mods", "Resource packs", "Shaders", "Modpacks"];
/// Modrinth tag vocabulary per UI kind (lowercase tag names for requests).
const CATEGORY_TYPES: [&str; 4] = ["mod", "resourcepack", "shader", "modpack"];
const SORTS: [&str; 5] = ["relevance", "downloads", "follows", "newest", "updated"];
const SORT_LABELS: [&str; 5] = [
    "Relevance",
    "Most downloaded",
    "Most followed",
    "Newest",
    "Recently updated",
];
const PACK_LOADERS: [&str; 4] = ["", "fabric", "forge", "neoforge"];
const PACK_LOADER_LABELS: [&str; 4] = ["Any loader", "Fabric", "Forge", "NeoForge"];
const MAX_RESULTS: usize = 200;
const PAGE_SIZE: u32 = 20;
/// Retained project icons and the most requested at once (the Slint host's
/// `MAX_ICONS`); concurrent core artwork calls are gated at [`ARTWORK_GATE`].
const MAX_ICONS: usize = 128;
const ARTWORK_GATE: usize = 4;

/// Emitted after any install settles (or fails) so the root reloads the
/// instance list, readiness and other pages that depend on instance content.
///
/// `ReturnToInstance` asks the root to show the instance Discover was opened
/// from on the content tab (`kind`: 0 mods, 1 resource packs, 2 shaders).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoverEvent {
    Changed,
    ReturnToInstance { instance: String, kind: i32 },
}

/// One completion arriving from the Tokio runtime. Serials and keys carry the
/// guards: a message is applied only while it still describes current state.
enum Message {
    Instances(u64, api::ApiResult<api::LauncherState>),
    Categories(api::ApiResult<Vec<api::ProviderCategory>>),
    Search {
        serial: u64,
        result: api::ApiResult<api::SearchPage>,
    },
    Inventory {
        request: InventoryRequest,
        result: api::ApiResult<Value>,
    },
    Details {
        serial: u64,
        preferred: Option<String>,
        result: api::ApiResult<api::ProjectDetails>,
    },
    Destinations {
        serial: u64,
        key: (String, String),
        result: api::ApiResult<Vec<api::PackDestination>>,
    },
    Review {
        serial: u64,
        snapshot: Review,
        observed_cancellation: bool,
        result: api::ApiResult<Value>,
    },
    Artwork {
        epoch: u64,
        project: String,
        outcome: ArtworkOutcome,
    },
}

/// What the core said about one project's artwork. `Failed` is transient and
/// is requested again with the next result list; `Absent` is remembered.
enum ArtworkOutcome {
    Image(RawArtwork),
    Absent,
    Failed,
}

/// Follow-up work a state transition asks the view to perform.
#[derive(Default, Debug, PartialEq)]
struct Effects {
    notify: bool,
    changed: bool,
    reload_instances: bool,
    reload_installed: bool,
    search_now: bool,
    /// New results or details need their artwork requested.
    artwork: bool,
}

/// Retained project artwork and its request bookkeeping.
struct ArtworkState {
    cache: ArtworkCache<String>,
    /// Projects with a request in flight for the current epoch.
    pending: HashSet<String>,
    /// Bumped whenever the result list is replaced. Replies from an older
    /// epoch are dropped; queued work observes it through [`Self::live`].
    epoch: u64,
    live: Arc<AtomicU64>,
}

impl Default for ArtworkState {
    fn default() -> Self {
        Self {
            cache: ArtworkCache::new(MAX_ICONS),
            pending: HashSet::new(),
            epoch: 0,
            live: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl ArtworkState {
    fn supersede(&mut self) {
        self.epoch += 1;
        self.pending.clear();
        self.live.store(self.epoch, Ordering::Release);
    }
}

/// The frozen reviewed snapshot: display is never authority; the commit
/// re-resolves this request and its fingerprint in Rust (in the core).
#[derive(Clone)]
struct Review {
    target: String,
    kind: &'static str,
    project: String,
    version: String,
    merge: bool,
    request: Value,
    fingerprint: String,
}

// ---------------------------------------------------------------- pure labels

fn loader_name(tag: &str) -> String {
    match tag.to_ascii_lowercase().as_str() {
        "fabric" => "Fabric".into(),
        "forge" => "Forge".into(),
        "neoforge" => "NeoForge".into(),
        "quilt" => "Quilt".into(),
        "vanilla" => "Vanilla".into(),
        _ => tag.into(),
    }
}

fn channel(version_type: &str) -> String {
    match version_type {
        "release" => "Release".into(),
        "beta" => "Beta".into(),
        "alpha" => "Alpha".into(),
        other => other.into(),
    }
}

fn bounded_list(values: &[String], shown: usize) -> String {
    let mut text = values
        .iter()
        .take(shown)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if values.len() > shown {
        text.push_str(&format!(" +{} more", values.len() - shown));
    }
    text
}

fn size_label(bytes: u64) -> String {
    let mib = bytes as f64 / (1024.0 * 1024.0);
    if mib >= 0.1 {
        format!("{mib:.1} MB")
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}

fn download_label(count: u64) -> String {
    match count {
        0..1_000 => format!("{count} downloads"),
        1_000..1_000_000 => format!("{:.1}K downloads", count as f64 / 1_000.),
        _ => format!("{:.1}M downloads", count as f64 / 1_000_000.),
    }
}

/// One dropdown row: "Pack 6.4.0 · Release · Minecraft 1.21.1 · Fabric",
/// marked when the core says Aurora can't install it.
fn version_label(v: &api::VersionChoice, pack: bool) -> String {
    if !pack {
        return format!(
            "{} · {} · {}",
            v.name,
            v.version_number,
            v.loaders.join(", ")
        );
    }
    let title = if v.name.contains(&v.version_number) {
        v.name.clone()
    } else {
        format!("{} {}", v.name, v.version_number)
    };
    let loaders = v.loaders.iter().map(|l| loader_name(l)).collect::<Vec<_>>();
    let mut label = format!(
        "{title} · {} · Minecraft {} · {}",
        channel(&v.version_type),
        bounded_list(&v.game_versions, 2),
        loaders.join(", ")
    );
    if v.unsupported_reason.is_some() {
        label.push_str(" · can't install");
    }
    label
}

/// What the selected pack version installs (its own metadata).
fn version_detail(v: &api::VersionChoice) -> String {
    let loaders = v.loaders.iter().map(|l| loader_name(l)).collect::<Vec<_>>();
    let mut text = format!(
        "{} · Minecraft {} · {}",
        channel(&v.version_type),
        bounded_list(&v.game_versions, 6),
        loaders.join(", ")
    );
    if let Some(size) = v.archive_size {
        text.push_str(&format!(" · {} archive", size_label(size)));
    }
    if v.unsupported_reason.is_none() {
        text.push_str(
            "\nThis version sets the Minecraft version and mod loader; they are not chosen here.",
        );
    }
    text
}

/// Project-level metadata, labelled as publisher lists rather than what any
/// one version installs.
fn publisher_lists(p: &api::ProjectDetails, pack: bool) -> String {
    let loaders = p.loaders.iter().map(|l| loader_name(l)).collect::<Vec<_>>();
    let mut text = format!(
        "Publisher lists Minecraft {} · {}",
        bounded_list(&p.game_versions, 12),
        loaders.join(", ")
    );
    if pack {
        text.push_str(". Each version shows what it installs.");
    }
    text
}

fn destination_detail(d: &api::PackDestination) -> String {
    let mut text = format!(
        "Minecraft {} · {}{}",
        d.minecraft_version,
        loader_name(&d.loader),
        d.loader_version
            .as_deref()
            .map(|v| format!(" {v}"))
            .unwrap_or_default()
    );
    if !d.compatible {
        text.push_str(" · ");
        text.push_str(&d.reasons.join(" "));
    }
    text
}

/// "Forge 47.4.10" from a pack DTO's `loader` kind and `loaderVersion`.
fn pack_loader_label(pack: &Value) -> String {
    let name = match pack["loader"].as_str().unwrap_or("") {
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoForge" => "NeoForge",
        _ => "Mod loader",
    };
    format!("{name} {}", pack["loaderVersion"].as_str().unwrap_or(""))
}

/// The labelled summary of a pack review: the exact version, the Minecraft
/// version and loader the pack sets (new instance) or requires (merge), the
/// destination, the published archive size, and for a merge the component
/// bound and that the instance will not follow pack updates.
fn pack_review_facts(
    value: &Value,
    merge: bool,
    version: Option<&api::VersionChoice>,
    destination: Option<&api::PackDestination>,
) -> Vec<(String, String)> {
    let pack = if merge { &value["pack"] } else { value };
    let text = |v: &Value, key: &str| v[key].as_str().unwrap_or("").to_owned();
    let mut pack_version = text(pack, "packVersion");
    if let Some(v) = version {
        pack_version.push_str(&format!(" · {}", channel(&v.version_type)));
    }
    let minecraft = text(pack, "minecraftVersion");
    let loader = pack_loader_label(pack).trim_end().to_owned();
    let mut facts = vec![("Pack version".to_owned(), pack_version)];
    if merge {
        let instance_loader = pack_loader_label(&json!({
            "loader": value["instanceLoader"],
            "loaderVersion": value["instanceLoaderVersion"].as_str().unwrap_or("")
        }));
        facts.extend([
            (
                "Minecraft".to_owned(),
                format!(
                    "{minecraft} · required by the pack; this instance has {}",
                    text(value, "instanceMinecraftVersion")
                ),
            ),
            (
                "Mod loader".to_owned(),
                format!(
                    "{loader} or newer · required by the pack; this instance has {}",
                    instance_loader.trim_end()
                ),
            ),
            (
                "Destination".to_owned(),
                format!(
                    "{} (existing instance)",
                    destination.map_or("The chosen instance", |d| d.display_name.as_str())
                ),
            ),
            (
                "Components".to_owned(),
                format!(
                    "{} added, at most {} per merge",
                    value["componentCount"].as_u64().unwrap_or(0),
                    value["componentLimit"].as_u64().unwrap_or(0)
                ),
            ),
            (
                "Pack updates".to_owned(),
                "This instance will not follow future updates of this pack.".to_owned(),
            ),
        ]);
    } else {
        facts.extend([
            (
                "Minecraft".to_owned(),
                format!("{minecraft} · set by the pack"),
            ),
            (
                "Mod loader".to_owned(),
                format!("{loader} · set by the pack"),
            ),
            (
                "Destination".to_owned(),
                "A new instance named after the pack".to_owned(),
            ),
        ]);
    }
    if let Some(size) = version.and_then(|v| v.archive_size) {
        facts.push((
            "Pack archive".to_owned(),
            format!(
                "{} from Modrinth, verified against its published SHA-512",
                size_label(size)
            ),
        ));
    }
    facts
}

/// Display is never authority; the core re-resolves the request and
/// fingerprint at commit. Returns (fingerprint, install allowed, display).
fn review_description(value: &Value, kind: &str, merge: bool) -> (String, bool, String) {
    let fingerprint = value[if kind == "modpack" {
        "fingerprint"
    } else {
        "previewFingerprint"
    }]
    .as_str()
    .unwrap_or_default()
    .to_owned();
    let mut text = String::new();
    let array = |v: &Value, key: &str| v[key].as_array().cloned().unwrap_or_default();
    let mut allowed = !fingerprint.is_empty();
    if kind == "modpack" {
        let pack = if merge { &value["pack"] } else { value };
        text.push_str(if merge {
            "Adds the pack's files to this instance as ordinary content. The instance keeps its own Minecraft version and mod loader.\n"
        } else {
            "Creates a separate modpack instance. Aurora Client is not added, and your other instances are unchanged.\n"
        });
        let not_for_client = "Not installed: not for this client, or optional and not selected";
        let outside = "Not installed: pack files outside game content";
        let groups = if merge {
            vec![
                ("Added", array(value, "added")),
                (
                    "New configuration files written",
                    array(value, "overridesWritten"),
                ),
                (
                    "Already present, left as they are",
                    array(value, "alreadyPresent"),
                ),
                (
                    "Your differing config files are kept; the pack's versions are not applied",
                    array(value, "keptExisting"),
                ),
                (
                    "Your existing mods, left untouched",
                    array(value, "untouchedMods"),
                ),
                (not_for_client, array(pack, "excluded")),
                (outside, array(pack, "skippedOverrides")),
                ("Blocked", array(value, "blockers")),
            ]
        } else {
            vec![
                ("Modrinth components installed", array(pack, "recognized")),
                ("Other verified files installed", array(pack, "unresolved")),
                ("Pack overrides written", array(pack, "overrides")),
                ("Optional files (choose below)", array(pack, "optional")),
                (not_for_client, array(pack, "excluded")),
                (outside, array(pack, "skippedOverrides")),
            ]
        };
        allowed &= array(value, "blockers").is_empty();
        for (index, (label, rows)) in groups.into_iter().enumerate() {
            if index > 0 && rows.is_empty() {
                continue;
            }
            text.push_str(&format!("\n{label} ({})\n", rows.len()));
            for row in rows.iter().take(200) {
                text.push_str(
                    row.as_str()
                        .or_else(|| row["path"].as_str())
                        .unwrap_or("File"),
                );
                text.push('\n');
            }
            if rows.len() > 200 {
                text.push_str("Additional files omitted from this list.\n");
            }
        }
    } else {
        let items = array(&value["preview"], "items");
        let conflicts = array(value, "conflicts");
        allowed &= conflicts.is_empty();
        text.push_str("Dependencies and destination files\n");
        for item in items {
            text.push_str(&format!(
                "{} · {}{}\n",
                item["title"].as_str().unwrap_or("Content"),
                item["fileName"].as_str().unwrap_or(""),
                if item["alreadyInstalled"] == true {
                    " (already installed)"
                } else {
                    ""
                }
            ));
        }
        for warning in array(&value["preview"], "warnings") {
            text.push_str(&format!(
                "\n{}",
                warning.as_str().unwrap_or("Review warning")
            ));
        }
        if !conflicts.is_empty() {
            text.push_str(
                "\nInstallation blocked by file/mod conflicts. Review installed content first.\n",
            );
            for conflict in conflicts {
                text.push_str(&format!("{conflict}\n"));
            }
        }
    }
    if fingerprint.is_empty() {
        text.push_str("\nThe service did not provide a valid review fingerprint.");
    }
    (fingerprint, allowed, text)
}

/// Review preparation status. The archive byte counter exists in the Slint
/// host through a 250 ms UI timer; this host has no polling, so only the
/// initial (pre-transfer) wording is presented while a review prepares.
fn review_progress(pack: bool, streamed: u64, archive: Option<u64>) -> (String, f32) {
    if !pack {
        return ("Checking dependencies and destination files…".into(), -1.);
    }
    match archive {
        _ if streamed == 0 => (
            match archive {
                Some(size) => format!(
                    "Getting the pack archive ({}) to review it…",
                    size_label(size)
                ),
                None => "Getting the pack archive to review it…".into(),
            },
            -1.,
        ),
        Some(size) if streamed < size => (
            format!(
                "Downloading the pack archive to review it: {} of {}",
                size_label(streamed),
                size_label(size)
            ),
            streamed as f32 / size as f32,
        ),
        Some(_) => (
            "Reading the pack and looking up its files on Modrinth…".into(),
            -1.,
        ),
        None => (
            format!(
                "Downloading the pack archive to review it: {} received",
                size_label(streamed)
            ),
            -1.,
        ),
    }
}

/// Project ids the core already installed for this kind in the target.
fn installed_from_inventory(rows: &Value, kind: &str) -> HashSet<String> {
    rows.as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["record"]["contentType"] == kind)
        .filter_map(|row| row["record"]["projectId"].as_str().map(str::to_owned))
        .collect()
}

/// Pagination helper: provider ids are unique across pages and retention is
/// bounded at [`MAX_RESULTS`].
fn append_unique(results: &mut Vec<api::ProjectSummary>, hits: Vec<api::ProjectSummary>) {
    let mut known: HashSet<String> = results.iter().map(|p| p.project_id.clone()).collect();
    results.extend(
        hits.into_iter()
            .filter(|p| known.insert(p.project_id.clone()))
            .take(MAX_RESULTS.saturating_sub(results.len())),
    );
}

/// The version row opened by default: a local archive's exact version, the
/// core's annotated default, or (non-pack only) the first listed version.
fn default_version_index(
    p: &api::ProjectDetails,
    pack: bool,
    preferred: Option<&str>,
) -> Option<usize> {
    match preferred {
        Some(version) => p.versions.iter().position(|v| v.id == version),
        None => p
            .default_version_id
            .as_ref()
            .and_then(|id| p.versions.iter().position(|v| &v.id == id))
            .or_else(|| (!pack && !p.versions.is_empty()).then_some(0)),
    }
}

// ------------------------------------------------------------ pure state

/// Everything Discover presents, with every transition executable without a
/// GPUI context so stale/in-flight/review guards are unit-testable.
#[derive(Default)]
struct DiscoverState {
    /// `true` when constructed without a host: an empty diagnostic view that
    /// makes no external calls.
    diagnostic: bool,

    // browsing
    kind: usize,
    category_index: usize,
    sort_index: usize,
    loader_index: usize,
    categories: Vec<api::ProviderCategory>,
    category_values: Vec<String>,
    categories_requested: bool,
    categories_error: Option<String>,
    instances: Vec<api::InstanceSummary>,
    selected_instance: Option<String>,
    instances_error: Option<String>,
    instances_serial: u64,
    target: Option<String>,

    // results
    results: Vec<api::ProjectSummary>,
    artwork: ArtworkState,
    installed: HashSet<String>,
    inventory_generation: u64,
    inventory_request: Option<InventoryRequest>,
    total: u32,
    serial: u64,
    next_offset: u32,
    searching: bool,
    error: Option<String>,

    // details
    details: Option<api::ProjectDetails>,
    details_open: bool,
    details_loading: bool,
    detail_error: Option<String>,
    detail_serial: u64,
    version_index: Option<usize>,
    pack_mode: usize,
    local_pack_note: String,

    // destinations
    destinations: Vec<api::PackDestination>,
    destinations_for: Option<(String, String)>,
    destinations_loading: bool,
    destinations_error: Option<String>,
    destination_serial: u64,
    destination: Option<String>,

    // review
    review: Option<Review>,
    review_job: Option<Control>,
    review_cancellable: bool,
    review_progress: String,
    review_text: String,
    review_facts: Vec<(String, String)>,
    optional_files: Vec<String>,
    selected_optional: HashSet<String>,
    /// The instance and content tab Discover was last opened from
    /// ([`Discover::open_for`]). Like the Slint host it stays until the next
    /// `open_for`; ordinary navigation does not clear it.
    origin: Option<(String, i32)>,
    /// A recognized local `.mrpack` (exact version id, banner text) taken by
    /// the next details load so it never leaks into later browsing.
    local_pack: Option<(String, String)>,

    // queue markers (mirrored from the retained Downloads queue by the view;
    // the queue itself owns the tasks)
    busy_projects: HashSet<String>,
    busy_instances: HashSet<String>,
}

/// Frozen data for one search request.
struct SearchRequest {
    target: String,
    kind: &'static str,
    query: String,
    offset: u32,
    categories: Vec<String>,
    sort: String,
    serial: u64,
}

#[derive(Clone, PartialEq, Eq)]
struct InventoryRequest {
    generation: u64,
    target: String,
    kind: &'static str,
}

/// Frozen data for one details request.
struct DetailsRequest {
    target: String,
    kind: &'static str,
    project: String,
    serial: u64,
    preferred: Option<String>,
}

/// Frozen data for one destinations request.
struct DestinationRequest {
    project: String,
    version: String,
    serial: u64,
}

/// Everything needed to spawn a review and to present its preparation.
struct ReviewStart {
    snapshot: Review,
    serial: u64,
    control: Control,
}

impl DiscoverState {
    fn kind_name(&self) -> &'static str {
        KINDS[self.kind.min(3)]
    }

    fn is_pack(&self) -> bool {
        self.kind == 3
    }

    fn merging(&self) -> bool {
        self.is_pack() && self.pack_mode == 1
    }

    fn selected_version(&self) -> Option<&api::VersionChoice> {
        let index = self.version_index?;
        self.details.as_ref()?.versions.get(index)
    }

    fn chosen_destination(&self) -> Option<&api::PackDestination> {
        let id = self.destination.as_ref()?;
        self.destinations
            .iter()
            .find(|d| &d.instance_id == id && d.compatible)
    }

    /// The selected version can drive a review (packs must be installable).
    fn version_installable(&self) -> bool {
        self.selected_version()
            .is_some_and(|v| !self.is_pack() || v.unsupported_reason.is_none())
    }

    /// Reviewing continues while queued jobs run: only this view's own loads
    /// and choices gate it, never unrelated queue work.
    fn can_review(&self) -> bool {
        !self.diagnostic
            && !self.details_loading
            && self.version_installable()
            && (!self.merging() || self.chosen_destination().is_some())
    }

    /// A completed, still-valid review is installable exactly once: the
    /// confirm path consumes it into a prepared job.
    fn can_install(&self) -> bool {
        self.review.is_some()
    }

    fn confirm_label(&self) -> &'static str {
        match (self.is_pack(), self.merging()) {
            (true, false) => "Create instance",
            (true, true) => "Add to instance",
            _ => "Install reviewed content",
        }
    }

    fn has_more(&self) -> bool {
        self.next_offset < self.total && self.results.len() < MAX_RESULTS
    }

    fn summary(&self) -> String {
        if self.results.len() >= MAX_RESULTS {
            format!("Showing {MAX_RESULTS} results. Refine your search for more.")
        } else if self.total > 0 {
            format!("{} shown · {} matches", self.results.len(), self.total)
        } else if !self.searching && self.error.is_none() {
            "No matching projects. Try another search.".into()
        } else {
            String::new()
        }
    }

    /// Whether the retained Downloads queue has this project queued or
    /// running (mirrored from the queue by the view).
    fn busy_project(&self, project: &str) -> bool {
        self.busy_projects.contains(project)
    }

    /// Whether the target instance has queued or running work (busy targets
    /// are marked honestly, never hidden).
    fn target_busy(&self) -> bool {
        self.target
            .as_ref()
            .is_some_and(|id| self.busy_instances.contains(id))
    }

    /// Options and guard state for a search; `None` means no request. The
    /// serial is bumped first so any in-flight result becomes stale.
    fn begin_search(&mut self, append: bool, query: String) -> Option<SearchRequest> {
        if self.diagnostic {
            return None;
        }
        if append && (self.searching || self.results.len() >= MAX_RESULTS) {
            return None;
        }
        let kind = self.kind_name();
        self.serial += 1;
        self.inventory_request = None;
        if !append {
            self.results.clear();
            self.next_offset = 0;
            self.total = 0;
            self.installed.clear();
            self.artwork.supersede();
        }
        self.error = None;
        let target = if kind == "modpack" {
            String::new()
        } else {
            self.target.clone().unwrap_or_default()
        };
        if kind != "modpack" && target.is_empty() {
            self.searching = false;
            self.error = Some("Choose a target instance to browse compatible content.".into());
            return None;
        }
        self.searching = true;
        let categories = self
            .category_values
            .get(self.category_index.wrapping_sub(1))
            .cloned()
            .into_iter()
            .chain(
                (kind == "modpack")
                    .then(|| PACK_LOADERS[self.loader_index.min(3)].to_owned())
                    .filter(|v| !v.is_empty()),
            )
            .collect();
        Some(SearchRequest {
            target,
            kind,
            query,
            offset: self.next_offset,
            categories,
            sort: SORTS[self.sort_index.min(4)].to_owned(),
            serial: self.serial,
        })
    }

    fn begin_inventory(&mut self) -> Option<InventoryRequest> {
        self.inventory_request = None;
        if self.diagnostic || self.serial == 0 || self.is_pack() {
            return None;
        }
        let target = self.target.clone()?;
        self.inventory_generation = self.inventory_generation.wrapping_add(1);
        let request = InventoryRequest {
            generation: self.inventory_generation,
            target,
            kind: self.kind_name(),
        };
        self.inventory_request = Some(request.clone());
        Some(request)
    }

    fn content_changed(&mut self) -> Effects {
        if self.diagnostic {
            return Effects::default();
        }
        self.inventory_request = None;
        self.invalidate_review();
        Effects {
            reload_installed: true,
            notify: true,
            ..Effects::default()
        }
    }

    /// Category tag names for the active kind, reset to "all".
    fn rebuild_categories(&mut self) {
        let kind = CATEGORY_TYPES[self.kind.min(3)];
        let mut values: Vec<String> = self
            .categories
            .iter()
            .filter(|c| c.project_type == kind)
            .map(|c| c.name.clone())
            .collect();
        values.sort();
        values.dedup();
        self.category_values = values;
        self.category_index = 0;
    }

    fn begin_instances(&mut self) -> u64 {
        self.instances_serial += 1;
        self.instances_serial
    }

    /// Applies a fresh instance list; keeps the explicit target only while it
    /// still exists, otherwise falls back to the launcher's selection.
    fn apply_instances(&mut self, launcher: api::LauncherState) -> bool {
        let previous = self.target.clone();
        self.selected_instance = launcher.config.selected_instance_id.clone();
        self.instances = launcher.instances;
        let present = |id: &str| self.instances.iter().any(|i| i.id == id);
        self.target = self
            .target
            .take()
            .filter(|id| present(id))
            .or_else(|| self.selected_instance.clone().filter(|id| present(id)));
        if previous.as_ref() != self.target.as_ref() {
            self.inventory_request = None;
            self.installed.clear();
            // Pack details do not depend on the content-browser target.
            // Merge destinations have their own exact project/version guard.
            if !self.is_pack() {
                self.invalidate_review();
            }
        }
        previous != self.target
    }

    /// Records the origin and makes it the install target and content kind
    /// (Slint's `open_for`). The caller then starts the search, whose serial
    /// bump drops any result requested before this navigation. Without a
    /// host nothing changes.
    fn begin_open_for(&mut self, instance: String, kind: i32) -> bool {
        if self.diagnostic {
            return false;
        }
        let kind = kind.clamp(0, 3);
        self.origin = Some((instance.clone(), kind));
        if self.target.as_ref() != Some(&instance) {
            self.target = Some(instance);
            self.installed.clear();
            self.invalidate_review();
        }
        self.kind = kind as usize;
        self.rebuild_categories();
        true
    }

    /// The origin to return to, only while that instance still exists
    /// (Slint's `can_return`).
    fn return_request(&self) -> Option<(String, i32)> {
        self.origin
            .clone()
            .filter(|(id, _)| self.instances.iter().any(|i| &i.id == id))
    }

    fn begin_local_pack(
        &mut self,
        project: String,
        version: String,
        note: String,
    ) -> Option<DetailsRequest> {
        // Queue installs are retained elsewhere, so navigation never replaces
        // or abandons running work; only identity and diagnostics gate here.
        if self.diagnostic || project.trim().is_empty() || version.trim().is_empty() {
            return None;
        }
        self.close_details();
        self.kind = 3;
        self.rebuild_categories();
        self.serial = self.serial.wrapping_add(1);
        self.searching = false;
        self.results.clear();
        self.artwork.supersede();
        self.total = 0;
        self.next_offset = 0;
        self.error = None;
        self.installed.clear();
        self.inventory_request = None;
        self.local_pack = Some((version, note));
        self.begin_details(project)
    }

    /// Freezes a details request and resets every per-project choice. Any
    /// in-flight review is cancelled: its result could never apply here.
    /// Queued installs keep running; browsing and review are never blocked.
    fn begin_details(&mut self, project: String) -> Option<DetailsRequest> {
        if self.diagnostic {
            return None;
        }
        let local = self.local_pack.take();
        self.local_pack_note = local
            .as_ref()
            .map(|(_, note)| note.clone())
            .unwrap_or_default();
        let preferred = local.map(|(version, _)| version);
        self.stop_review();
        self.detail_serial += 1;
        self.destination_serial += 1;
        self.details = None;
        self.review = None;
        self.review_text.clear();
        self.review_facts.clear();
        self.optional_files.clear();
        self.selected_optional.clear();
        self.destinations.clear();
        self.destinations_for = None;
        self.destinations_loading = false;
        self.destinations_error = None;
        self.destination = None;
        self.version_index = None;
        self.pack_mode = 0;
        self.details_open = true;
        self.details_loading = true;
        self.detail_error = None;
        self.review_progress = "Loading project details…".into();
        let kind = self.kind_name();
        let target = if kind == "modpack" {
            String::new()
        } else {
            self.target.clone().unwrap_or_default()
        };
        Some(DetailsRequest {
            target,
            kind,
            project,
            serial: self.detail_serial,
            preferred,
        })
    }

    /// Presents loaded project details: labelled publisher metadata, the
    /// core's default version (never a substitute for a preferred one).
    fn apply_details(&mut self, p: api::ProjectDetails, preferred: Option<&str>) {
        let pack = self.is_pack();
        let index = default_version_index(&p, pack, preferred);
        if p.versions.is_empty() {
            self.detail_error =
                Some("This project has no published versions Aurora can show.".into());
        } else if preferred.is_some() && index.is_none() {
            self.detail_error =
                Some("This archive's exact pack version is no longer listed by Modrinth.".into());
        }
        self.version_index = index;
        self.details = Some(p);
    }

    fn close_details(&mut self) {
        self.stop_review();
        self.detail_serial += 1;
        self.destination_serial += 1;
        self.review = None;
        self.review_text.clear();
        self.review_facts.clear();
        self.optional_files.clear();
        self.selected_optional.clear();
        self.details = None;
        self.details_open = false;
        self.details_loading = false;
        self.detail_error = None;
        self.version_index = None;
        self.destinations.clear();
        self.destinations_for = None;
        self.destinations_loading = false;
        self.destinations_error = None;
        self.destination = None;
        self.local_pack_note.clear();
        self.local_pack = None;
    }

    /// Cancels the in-flight review's content job (its download stops at the
    /// next streamed chunk and its staging file is removed).
    fn stop_review(&mut self) {
        if let Some(job) = self.review_job.take() {
            job.cancel();
        }
        self.review_cancellable = false;
        self.review_progress.clear();
    }

    /// Discards the current review (and stops one still being prepared). Any
    /// change to the version, destination or optional files lands here, so the
    /// reviewed fingerprint can never outlive the choices it was made for.
    fn invalidate_review(&mut self) {
        self.stop_review();
        self.detail_serial += 1;
        self.review = None;
        self.optional_files.clear();
        self.selected_optional.clear();
        self.review_text.clear();
        self.review_facts.clear();
        self.details_loading = false;
        self.detail_error = None;
    }

    /// Explicit cancel of an in-flight review; bumps the serial so the late
    /// result of the cancelled job is discarded.
    fn cancel_review(&mut self) -> bool {
        if self.review_job.is_none() {
            return false;
        }
        self.stop_review();
        self.detail_serial += 1;
        self.review = None;
        self.review_facts.clear();
        self.review_text = "Review cancelled. Nothing was installed.".into();
        self.details_loading = false;
        true
    }
    fn toggle_optional(&mut self, path: &str) {
        if !self.optional_files.iter().any(|p| p == path) {
            return;
        }
        if !self.selected_optional.remove(path) {
            self.selected_optional.insert(path.to_owned());
        }
        self.detail_serial += 1;
        self.review = None;
        self.review_facts.clear();
        self.review_text =
            "Optional selection changed. Review installation again before installing.".into();
    }

    /// Selects a listed version; clears the review and destination state that
    /// belonged to the previous version.
    fn version_changed(&mut self, index: usize) {
        let valid = self
            .details
            .as_ref()
            .is_some_and(|d| index < d.versions.len());
        if !valid {
            return;
        }
        self.invalidate_review();
        self.version_index = Some(index);
        self.destination_serial += 1;
        self.destinations.clear();
        self.destinations_for = None;
        self.destinations_loading = false;
        self.destinations_error = None;
    }

    fn set_pack_mode(&mut self, mode: usize) {
        let mode = mode.min(1);
        if self.pack_mode == mode {
            return;
        }
        self.invalidate_review();
        self.pack_mode = mode;
        if mode == 0 {
            self.destination_serial += 1;
            self.destinations.clear();
            self.destinations_for = None;
            self.destinations_loading = false;
            self.destinations_error = None;
        }
    }

    /// Only a compatible instance can be chosen. Returns whether the choice
    /// changed (and therefore invalidated any review).
    fn choose_destination(&mut self, index: usize) -> bool {
        let chosen = self
            .destinations
            .get(index)
            .filter(|row| row.compatible)
            .map(|row| row.instance_id.clone());
        match chosen {
            Some(id) if self.destination.as_ref() != Some(&id) => {
                self.invalidate_review();
                self.destination = Some(id);
                true
            }
            _ => false,
        }
    }

    /// Asks the core which instances can take the selected pack version.
    fn begin_destinations(&mut self) -> Option<DestinationRequest> {
        if self.diagnostic || !self.is_pack() || self.pack_mode != 1 {
            return None;
        }
        let p = self.details.as_ref()?;
        let v = self.selected_version()?;
        if v.unsupported_reason.is_some() {
            return None;
        }
        let key = (p.project_id.clone(), v.id.clone());
        if self.destinations_for.as_ref() == Some(&key) {
            return None;
        }
        self.destination_serial += 1;
        self.destinations_for = Some(key.clone());
        self.destinations.clear();
        self.destinations_loading = true;
        self.destinations_error = None;
        Some(DestinationRequest {
            project: key.0,
            version: key.1,
            serial: self.destination_serial,
        })
    }

    /// Keeps the chosen instance only while the core still calls it
    /// compatible; otherwise preselects the instance Discover was opened from,
    /// if it is.
    fn apply_destinations(&mut self, rows: Vec<api::PackDestination>) {
        self.destinations = rows;
        let compatible = |id: &String| {
            self.destinations
                .iter()
                .any(|d| &d.instance_id == id && d.compatible)
        };
        self.destination = self
            .destination
            .clone()
            .filter(|id| compatible(id))
            .or_else(|| {
                self.origin
                    .as_ref()
                    .map(|(id, _)| id.clone())
                    .filter(|id| compatible(id))
            });
    }

    fn destinations_note(&self) -> String {
        if self.destinations_loading {
            return "Checking which instances can take this version…".into();
        }
        if let Some(error) = &self.destinations_error {
            return error.clone();
        }
        if self.destinations_for.is_none() {
            return String::new();
        }
        if self.destinations.is_empty() {
            return "You have no instances yet. Choose New instance instead.".into();
        }
        if !self.destinations.iter().any(|d| d.compatible) {
            return "None of your instances can take this version. Choose New instance, or another version.".into();
        }
        "Only compatible instances can be chosen. Review checks the exact loader version too."
            .into()
    }

    /// Freezes the reviewed request and its cancellable job. The serial bump
    /// discards both late details results and late prior reviews.
    fn begin_review(&mut self) -> Option<ReviewStart> {
        if !self.can_review() {
            return None;
        }
        let p = self.details.as_ref()?;
        let v = self.selected_version()?;
        let kind = self.kind_name();
        let pack = kind == "modpack";
        let merge = self.merging();
        let target = if merge {
            match self.chosen_destination() {
                Some(destination) => destination.instance_id.clone(),
                None => {
                    self.detail_error = Some("Choose a compatible instance first.".into());
                    return None;
                }
            }
        } else if pack {
            String::new()
        } else {
            match self.target.clone() {
                Some(target) => target,
                None => {
                    self.detail_error = Some("Choose a target instance first.".into());
                    return None;
                }
            }
        };
        let archive = v.archive_size;
        let (project, version) = (p.project_id.clone(), v.id.clone());
        self.stop_review();
        self.detail_serial += 1;
        self.review = None;
        self.review_text.clear();
        self.review_facts.clear();
        self.details_loading = true;
        self.detail_error = None;
        let selected: Vec<_> = self
            .optional_files
            .iter()
            .filter(|path| self.selected_optional.contains(*path))
            .cloned()
            .collect();
        let mut request = json!({"projectId":project,"versionId":version,"selectedOptional":selected,"fingerprint":null});
        if merge {
            request["instanceId"] = json!(target);
        }
        // The review runs as its own content job: Cancel stops its archive
        // download between streamed chunks.
        let control = Control::interrupting_transfers();
        self.review_job = Some(control.clone());
        self.review_cancellable = true;
        self.review_progress = review_progress(pack, 0, archive).0;
        Some(ReviewStart {
            snapshot: Review {
                target,
                kind,
                project,
                version,
                merge,
                request,
                fingerprint: String::new(),
            },
            serial: self.detail_serial,
            control,
        })
    }

    /// Presents a completed review and keeps it as the only installable
    /// snapshot when the core allows it.
    fn apply_review(&mut self, mut snapshot: Review, value: &Value) {
        if snapshot.kind == "modpack" {
            let pack = if snapshot.merge {
                &value["pack"]
            } else {
                value
            };
            self.optional_files = pack["optional"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            let version = self.selected_version().cloned();
            let destination = self.chosen_destination().cloned();
            self.review_facts = pack_review_facts(
                value,
                snapshot.merge,
                version.as_ref(),
                destination.as_ref(),
            );
        }
        let (fingerprint, allowed, text) = review_description(value, snapshot.kind, snapshot.merge);
        snapshot.fingerprint = fingerprint;
        snapshot.request["fingerprint"] = json!(snapshot.fingerprint);
        self.review_text = text;
        // A completed review that blocks installation invalidates any prior
        // installable snapshot; display is never authority.
        self.review = allowed.then_some(snapshot);
    }

    /// Quick install from a result row (content kinds only): the core
    /// resolves the version; review is not required for this path. The
    /// prepared job is queued, never executed here.
    fn begin_quick_install(&mut self, project: &str) -> Option<PreparedJob> {
        if self.diagnostic || self.is_pack() {
            return None;
        }
        let target = self.target.clone()?;
        if self.installed.contains(project) || self.busy_project(project) {
            return None;
        }
        let title = self
            .results
            .iter()
            .find(|p| p.project_id == project)
            .map(|p| p.title.clone())
            .unwrap_or_else(|| "Content".into());
        let instance = self
            .instances
            .iter()
            .find(|i| i.id == target)
            .map(|i| i.display_name.clone())
            .unwrap_or_default();
        Some(PreparedJob {
            title,
            detail: format!("Into {instance}"),
            request: JobRequest::Quick {
                target,
                kind: self.kind_name(),
                project: project.to_owned(),
            },
        })
    }

    /// Consumes the completed review into the exact prepared job the core
    /// will re-resolve and fingerprint-check. The snapshot is taken, never
    /// reused: one review installs once.
    fn confirm_install(&mut self) -> Option<PreparedJob> {
        if !self.can_install() {
            return None;
        }
        let snapshot = self.review.take()?;
        let title = self
            .details
            .as_ref()
            .map(|p| p.title.clone())
            .unwrap_or_else(|| "Content".into());
        let detail = if snapshot.kind == "modpack" && !snapshot.merge {
            "New modpack instance".to_owned()
        } else {
            let name = self
                .instances
                .iter()
                .find(|i| i.id == snapshot.target)
                .map(|i| i.display_name.clone())
                .unwrap_or_default();
            format!("Into {name}")
        };
        let request = if snapshot.kind == "modpack" {
            JobRequest::Pack {
                merge: snapshot.merge,
                request: snapshot.request.clone(),
            }
        } else {
            JobRequest::Content {
                target: snapshot.target.clone(),
                kind: snapshot.kind,
                project: snapshot.project.clone(),
                version: snapshot.version.clone(),
                fingerprint: snapshot.fingerprint.clone(),
            }
        };
        self.detail_serial += 1;
        Some(PreparedJob {
            title,
            detail,
            request,
        })
    }

    /// What a Downloads settlement asks this view to do: the registry and
    /// installed markers are refreshed and the parent is told (a kept
    /// instance can appear on failure or cancellation too), with the
    /// existing serial/generation guards applied by the loaders themselves.
    fn settlement_effects() -> Effects {
        Effects {
            notify: true,
            changed: true,
            reload_instances: true,
            reload_installed: true,
            search_now: false,
            artwork: false,
        }
    }

    /// Projects whose icon is on screen: the current results plus the open
    /// project's details.
    fn artwork_wanted(&self, project: &str) -> bool {
        self.results.iter().any(|p| p.project_id == project)
            || self
                .details
                .as_ref()
                .is_some_and(|d| d.project_id == project)
    }

    /// The next artwork requests: wanted projects without a retained or
    /// in-flight icon, within the [`MAX_ICONS`] budget. Marks them pending
    /// and returns the epoch they belong to.
    fn begin_artwork(&mut self) -> (u64, Vec<String>) {
        if self.diagnostic {
            return (self.artwork.epoch, Vec::new());
        }
        let budget = MAX_ICONS.saturating_sub(self.artwork.pending.len());
        // Result order is screen order, so the first cards load first.
        let mut seen = HashSet::new();
        let ids: Vec<String> = self
            .results
            .iter()
            .map(|p| p.project_id.as_str())
            .chain(self.details.iter().map(|d| d.project_id.as_str()))
            .filter(|id| {
                !self.artwork.cache.contains(&(*id).to_owned())
                    && !self.artwork.pending.contains(*id)
                    && seen.insert(*id)
            })
            .map(str::to_owned)
            .take(budget)
            .collect();
        self.artwork.pending.extend(ids.iter().cloned());
        (self.artwork.epoch, ids)
    }

    /// Applies one artwork reply. Replies for a replaced result list, or for
    /// a project no longer shown, change nothing. Returns whether to redraw.
    fn receive_artwork(&mut self, epoch: u64, project: String, outcome: ArtworkOutcome) -> bool {
        if epoch != self.artwork.epoch {
            return false;
        }
        self.artwork.pending.remove(&project);
        if !self.artwork_wanted(&project) {
            return false;
        }
        let image = match outcome {
            ArtworkOutcome::Image(raw) => raw.into_image(),
            ArtworkOutcome::Absent => None,
            // Not cached: the next result list or details view asks again.
            ArtworkOutcome::Failed => return false,
        };
        let wanted: HashSet<String> = self
            .results
            .iter()
            .map(|p| p.project_id.clone())
            .chain(self.details.iter().map(|d| d.project_id.clone()))
            .collect();
        self.artwork
            .cache
            .insert(project, image, |old| wanted.contains(old));
        true
    }

    /// Applies one async completion; stale messages change nothing.
    fn receive(&mut self, message: Message) -> Effects {
        let mut fx = Effects::default();
        match message {
            Message::Instances(serial, result) => {
                if serial != self.instances_serial {
                    return fx;
                }
                match result {
                    Ok(launcher) => {
                        self.instances_error = None;
                        let target_changed = self.apply_instances(launcher);
                        if !self.diagnostic {
                            if self.serial == 0 || target_changed {
                                fx.search_now = true;
                            } else {
                                fx.reload_installed = true;
                            }
                        }
                    }
                    Err(e) => self.instances_error = Some(format!("{} ({})", e.message, e.code)),
                }
                fx.notify = true;
            }
            Message::Categories(result) => match result {
                Ok(categories) => {
                    self.categories = categories;
                    self.categories_error = None;
                    self.rebuild_categories();
                    fx.notify = true;
                }
                Err(e) => {
                    // Retry on the next refresh, like the Slint host.
                    self.categories_requested = false;
                    self.categories_error = Some(format!("{} ({})", e.message, e.code));
                    fx.notify = true;
                }
            },
            Message::Search { serial, result } => {
                if serial != self.serial {
                    return fx;
                }
                self.searching = false;
                match result {
                    Ok(page) => {
                        self.next_offset = page.offset.saturating_add(PAGE_SIZE);
                        self.total = page.total_hits;
                        append_unique(&mut self.results, page.hits);
                        fx.artwork = true;
                    }
                    Err(e) => self.error = Some(format!("{} ({})", e.message, e.code)),
                }
                fx.notify = true;
            }
            Message::Inventory { request, result } => {
                if self.inventory_request.as_ref() != Some(&request)
                    || self.target.as_deref() != Some(request.target.as_str())
                    || self.kind_name() != request.kind
                {
                    return fx;
                }
                self.inventory_request = None;
                if let Ok(rows) = result {
                    self.installed = installed_from_inventory(&rows, request.kind);
                    fx.notify = true;
                }
            }
            Message::Details {
                serial,
                preferred,
                result,
            } => {
                if serial != self.detail_serial {
                    return fx;
                }
                self.details_loading = false;
                self.review_progress.clear();
                match result {
                    Ok(p) => {
                        self.detail_error = None;
                        self.apply_details(p, preferred.as_deref());
                        fx.artwork = true;
                    }
                    Err(e) => self.detail_error = Some(format!("{} ({})", e.message, e.code)),
                }
                fx.notify = true;
            }
            Message::Destinations {
                serial,
                key,
                result,
            } => {
                if serial != self.destination_serial || self.destinations_for.as_ref() != Some(&key)
                {
                    return fx;
                }
                self.destinations_loading = false;
                match result {
                    Ok(rows) => self.apply_destinations(rows),
                    Err(e) => {
                        self.destinations_for = None;
                        self.destinations_error = Some(format!("{} ({})", e.message, e.code));
                    }
                }
                fx.notify = true;
            }
            Message::Review {
                serial,
                snapshot,
                observed_cancellation,
                result,
            } => {
                if serial != self.detail_serial {
                    return fx;
                }
                self.review_job = None;
                self.review_cancellable = false;
                self.review_progress.clear();
                self.details_loading = false;
                if observed_cancellation {
                    self.review_text = "Review cancelled. Nothing was installed.".into();
                    fx.notify = true;
                    return fx;
                }
                match result {
                    Ok(value) => self.apply_review(snapshot, &value),
                    Err(e) => self.detail_error = Some(format!("{} ({})", e.message, e.code)),
                }
                fx.notify = true;
            }
            Message::Artwork {
                epoch,
                project,
                outcome,
            } => fx.notify = self.receive_artwork(epoch, project, outcome),
        }
        fx
    }
}

// ---------------------------------------------------------------- view state

/// Open flag, highlight index and the two focus handles one controlled
/// `AuroraSelect` needs, plus its trigger-bounds cell.
struct SelectUi {
    open: bool,
    highlight: usize,
    trigger: FocusHandle,
    content: FocusHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl SelectUi {
    fn new(cx: &mut App) -> Self {
        Self {
            open: false,
            highlight: 0,
            trigger: cx.focus_handle(),
            content: cx.focus_handle(),
            bounds: Rc::new(Cell::new(None)),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SelectSlot {
    Category,
    Sort,
    Loader,
    Target,
    Version,
}

struct Selects {
    category: SelectUi,
    sort: SelectUi,
    loader: SelectUi,
    target: SelectUi,
    version: SelectUi,
}

impl Selects {
    fn new(cx: &mut App) -> Self {
        Self {
            category: SelectUi::new(cx),
            sort: SelectUi::new(cx),
            loader: SelectUi::new(cx),
            target: SelectUi::new(cx),
            version: SelectUi::new(cx),
        }
    }
}

/// The GPUI Discover view. Async work runs on the shared Tokio runtime and
/// lands on the UI thread through one awaited channel (no polling, no idle
/// animations); the view notifies only when a message changed state. Installs
/// are submitted to the retained [`Downloads`] queue this view owns; the
/// parent embeds that entity through [`Discover::downloads`].
pub struct Discover {
    state: DiscoverState,
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    tx: async_channel::Sender<Message>,
    query: Entity<InputState>,
    _query_events: Subscription,
    dialog: DialogHandle,
    selects: Selects,
    downloads: Entity<Downloads>,
    _downloads_events: Subscription,
    /// Mirrored queue activity for the parent's conservative guards; kept in
    /// step with the queue by every submission and queue event.
    queue_busy: bool,
}

impl EventEmitter<DiscoverEvent> for Discover {}

impl Discover {
    /// Builds the view. `host: None` constructs the empty diagnostic view that
    /// makes no external calls.
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let diagnostic = host.is_none();
        let (tx, rx) = async_channel::unbounded();
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search Modrinth…"));
        let _query_events = cx.subscribe(&query, |this, _, event: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.search(false, cx);
            }
        });
        // The retained Downloads queue: submissions from this view settle
        // there and drive this view's refresh through its events.
        let downloads = cx.new(|cx| Downloads::new(host.clone(), runtime.clone(), window, cx));
        let _downloads_events = cx.subscribe(&downloads, |this, _, event, cx| {
            this.sync_queue_markers(cx);
            match event {
                DownloadsEvent::Changed => cx.notify(),
                DownloadsEvent::Settled { .. } => {
                    this.apply_effects(DiscoverState::settlement_effects(), cx)
                }
            }
        });
        // Event-driven pump: results arrive as messages; nothing polls.
        cx.spawn(async move |weak, cx| {
            while let Ok(message) = rx.recv().await {
                if weak
                    .update(cx, |view, cx| view.receive(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Discover {
            state: DiscoverState {
                diagnostic,
                ..Default::default()
            },
            host,
            rt: runtime,
            tx,
            query,
            _query_events,
            dialog: DialogHandle::new(false),
            selects: Selects::new(cx),
            downloads,
            _downloads_events,
            queue_busy: false,
        }
    }

    /// The retained Downloads queue entity for the parent to embed wherever
    /// it presents downloads. The queue is shared with this view's
    /// submissions; the parent only renders/routes, never re-creates it.
    pub fn downloads(&self) -> Entity<Downloads> {
        self.downloads.clone()
    }

    /// Reloads launcher data: instances (target choices), Modrinth categories
    /// (once, retried after failure) and the first search.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.state.diagnostic {
            return;
        }
        self.load_categories(cx);
        self.load_instances(cx);
        cx.notify();
    }

    /// Invalidates reviews and inventory after another page changes content.
    pub fn content_changed(&mut self, cx: &mut Context<Self>) {
        let effects = self.state.content_changed();
        self.apply_effects(effects, cx);
    }

    /// Open the explicit hosted review for a recognized local archive's
    /// exact version. Queue installs are retained, so navigation never
    /// replaces or abandons running work.
    pub fn open_local_pack(
        &mut self,
        project: String,
        version: String,
        note: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(request) = self.state.begin_local_pack(project, version, note) else {
            return false;
        };
        self.load_categories(cx);
        self.dispatch_details(request, window, cx);
        true
    }

    /// Opens Discover from an instance's content tab: that instance becomes
    /// the target, `kind` (0 mods, 1 resource packs, 2 shaders) the browsed
    /// kind, and the search runs under a fresh serial.
    pub fn open_for(&mut self, instance: String, kind: i32, cx: &mut Context<Self>) -> bool {
        if !self.state.begin_open_for(instance, kind) {
            return false;
        }
        self.load_categories(cx);
        self.search(false, cx);
        true
    }

    /// Asks the root to show the origin instance again; a no-op once that
    /// instance is gone.
    fn return_to_instance(&mut self, cx: &mut Context<Self>) {
        if let Some((instance, kind)) = self.state.return_request() {
            cx.emit(DiscoverEvent::ReturnToInstance { instance, kind });
        }
    }

    /// True while any prepared job is queued or running. Conservative for
    /// the parent's mutation guards: it stays busy through retries and
    /// cooperative cancellations until the queue drains.
    pub fn is_busy(&self) -> bool {
        self.queue_busy
    }

    /// Routes one host event. `modpack-progress` carries no task identity, so
    /// it is advisory: it only refreshes the queue's presentation (each
    /// running row reads its own control's stage at render). Unknown names
    /// and malformed payloads are ignored, never fatal.
    pub fn on_host_event(&mut self, name: &str, payload: &Value, cx: &mut Context<Self>) {
        self.downloads.update(cx, |downloads, cx| {
            downloads.on_host_event(name, payload, cx)
        });
    }

    /// Re-reads the queue's honest busy markers (projects, instances, and the
    /// parent-guard busy flag).
    fn sync_queue_markers(&mut self, cx: &mut Context<Self>) {
        let queue = self.downloads.read(cx);
        self.queue_busy = queue.is_busy();
        self.state.busy_projects = queue.busy_projects().into_iter().collect();
        self.state.busy_instances = queue.busy_instances().into_iter().collect();
    }

    /// Queues one prepared job in the retained Downloads entity.
    fn enqueue(&mut self, job: PreparedJob, cx: &mut Context<Self>) {
        let queued = self
            .downloads
            .update(cx, |downloads, cx| downloads.submit(job, cx));
        if !queued && !self.state.diagnostic {
            self.state.error =
                Some("Finish the launcher operation, then review and install again.".into());
        }
        self.sync_queue_markers(cx);
        cx.notify();
    }

    // ------------------------------------------------------------ loaders

    fn load_categories(&mut self, cx: &mut Context<Self>) {
        if self.state.categories_requested {
            return;
        }
        self.state.categories_requested = true;
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let result = guarded(api::browse_categories()).await;
            let _ = tx.send(Message::Categories(result)).await;
        });
        cx.notify();
    }

    fn load_instances(&mut self, cx: &mut Context<Self>) {
        let serial = self.state.begin_instances();
        let Some(host) = self.host.clone() else {
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let result = guarded(api::launcher_state(host)).await;
            let _ = tx.send(Message::Instances(serial, result)).await;
        });
        cx.notify();
    }

    fn load_installed(&mut self, _cx: &mut Context<Self>) {
        let Some(request) = self.state.begin_inventory() else {
            return;
        };
        let Some(host) = self.host.clone() else {
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let result = guarded(api::provider_inventory(host, request.target.clone())).await;
            let _ = tx.send(Message::Inventory { request, result }).await;
        });
    }

    /// Starts the artwork requests the current results and details still
    /// need. Each runs behind the shared gate and skips the provider when its
    /// result list was replaced while it waited. The view never touches the
    /// network: the core returns normalized PNG bytes.
    fn load_artwork(&mut self) {
        let Some(host) = self.host.clone() else {
            return;
        };
        let (epoch, ids) = self.state.begin_artwork();
        if ids.is_empty() {
            return;
        }
        static GATE: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> = std::sync::OnceLock::new();
        let gate = GATE
            .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(ARTWORK_GATE)))
            .clone();
        let live = self.state.artwork.live.clone();
        for id in ids {
            let (host, gate, live, tx) =
                (host.clone(), gate.clone(), live.clone(), self.tx.clone());
            self.rt.spawn(async move {
                let outcome = match gate.acquire_owned().await {
                    Ok(_permit) if live.load(Ordering::Acquire) == epoch => {
                        match guarded(api::project_artwork(host, id.clone())).await {
                            Ok(Some(png)) => artwork::raw_from_png(&png)
                                .map_or(ArtworkOutcome::Absent, ArtworkOutcome::Image),
                            Ok(None) => ArtworkOutcome::Absent,
                            Err(_) => ArtworkOutcome::Failed,
                        }
                    }
                    // Superseded while queued: the reply is dropped by epoch.
                    _ => ArtworkOutcome::Failed,
                };
                let _ = tx
                    .send(Message::Artwork {
                        epoch,
                        project: id,
                        outcome,
                    })
                    .await;
            });
        }
    }

    fn search(&mut self, append: bool, cx: &mut Context<Self>) {
        let query = self.query.read(cx).value().to_string();
        let Some(request) = self.state.begin_search(append, query) else {
            cx.notify();
            return;
        };
        let Some(host) = self.host.clone() else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let result = guarded(api::browse_filtered(
                host,
                request.target,
                request.kind,
                request.query,
                request.offset,
                request.categories,
                request.sort,
            ))
            .await;
            let _ = tx
                .send(Message::Search {
                    serial: request.serial,
                    result,
                })
                .await;
        });
        self.load_installed(cx);
        cx.notify();
    }

    fn open_details(&mut self, project: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(request) = self.state.begin_details(project) else {
            cx.notify();
            return;
        };
        self.dispatch_details(request, window, cx);
    }

    fn dispatch_details(
        &mut self,
        request: DetailsRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(host) = self.host.clone() else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let result = guarded(api::project(
                host,
                request.target,
                request.kind,
                request.project,
            ))
            .await;
            let _ = tx
                .send(Message::Details {
                    serial: request.serial,
                    preferred: request.preferred,
                    result,
                })
                .await;
        });
        self.dialog.open(window, cx);
        self.selects.version.trigger.focus(window, cx);
        cx.notify();
    }

    fn load_destinations(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.state.begin_destinations() else {
            cx.notify();
            return;
        };
        let Some(host) = self.host.clone() else {
            cx.notify();
            return;
        };
        let key = (request.project.clone(), request.version.clone());
        let tx = self.tx.clone();
        self.rt.spawn(async move {
            let result = guarded(api::pack_destinations(
                host,
                request.project,
                request.version,
            ))
            .await;
            let _ = tx
                .send(Message::Destinations {
                    serial: request.serial,
                    key,
                    result,
                })
                .await;
        });
        cx.notify();
    }

    // ------------------------------------------------------------ actions

    fn choose_kind(&mut self, kind: usize, cx: &mut Context<Self>) {
        let kind = kind.min(3);
        if self.state.kind == kind {
            return;
        }
        self.state.kind = kind;
        self.state.rebuild_categories();
        self.search(false, cx);
    }

    fn set_category(&mut self, index: usize, cx: &mut Context<Self>) {
        if index <= self.state.category_values.len() {
            self.state.category_index = index;
        }
        self.search(false, cx);
    }

    fn set_sort(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < SORTS.len() {
            self.state.sort_index = index;
        }
        self.search(false, cx);
    }

    fn set_loader(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < PACK_LOADERS.len() && self.state.loader_index != index {
            self.state.loader_index = index;
        }
        self.search(false, cx);
    }

    fn choose_target(&mut self, index: usize, cx: &mut Context<Self>) {
        let chosen = self.state.instances.get(index).map(|i| i.id.clone());
        if let Some(id) = chosen {
            if self.state.target.as_ref() != Some(&id) {
                self.state.target = Some(id);
                self.state.invalidate_review();
            }
        }
        self.search(false, cx);
    }

    fn set_select_open(&mut self, slot: SelectSlot, open: bool, cx: &mut Context<Self>) {
        let select = self.select(slot);
        if select.open != open {
            select.open = open;
            if open {
                select.highlight = 0;
            }
            cx.notify();
        }
    }

    fn set_select_highlight(&mut self, slot: SelectSlot, index: usize, cx: &mut Context<Self>) {
        if self.select(slot).highlight != index {
            self.select(slot).highlight = index;
            cx.notify();
        }
    }

    fn select(&mut self, slot: SelectSlot) -> &mut SelectUi {
        match slot {
            SelectSlot::Category => &mut self.selects.category,
            SelectSlot::Sort => &mut self.selects.sort,
            SelectSlot::Loader => &mut self.selects.loader,
            SelectSlot::Target => &mut self.selects.target,
            SelectSlot::Version => &mut self.selects.version,
        }
    }

    fn version_selected(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.version_changed(index);
        self.load_destinations(cx);
    }

    fn pack_mode_selected(&mut self, mode: usize, cx: &mut Context<Self>) {
        self.state.set_pack_mode(mode);
        self.load_destinations(cx);
    }

    fn destination_selected(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.state.choose_destination(index) {
            cx.notify();
        }
    }

    fn review(&mut self, cx: &mut Context<Self>) {
        let Some(start) = self.state.begin_review() else {
            cx.notify();
            return;
        };
        let Some(host) = self.host.clone() else {
            cx.notify();
            return;
        };
        let tx = self.tx.clone();
        let call_snapshot = start.snapshot.clone();
        let control = start.control.clone();
        self.rt.spawn(async move {
            let run = control.clone();
            let result = guarded(async move {
                run.run(async move {
                    if call_snapshot.kind == "modpack" {
                        api::preview_pack(host, call_snapshot.merge, call_snapshot.request.clone())
                            .await
                    } else {
                        api::preview_content(
                            host,
                            call_snapshot.target.clone(),
                            call_snapshot.kind,
                            call_snapshot.project.clone(),
                            call_snapshot.version.clone(),
                        )
                        .await
                    }
                })
                .await
            })
            .await;
            let observed_cancellation = control.cancellation_observed();
            let _ = tx
                .send(Message::Review {
                    serial: start.serial,
                    snapshot: start.snapshot,
                    observed_cancellation,
                    result,
                })
                .await;
        });
        cx.notify();
    }

    fn cancel_review(&mut self, cx: &mut Context<Self>) {
        if self.state.cancel_review() {
            cx.notify();
        }
    }

    fn toggle_optional(&mut self, path: String, cx: &mut Context<Self>) {
        self.state.toggle_optional(&path);
        cx.notify();
    }

    /// Quick install queues in Downloads; browsing stays available.
    fn quick_install(&mut self, project: String, cx: &mut Context<Self>) {
        let Some(job) = self.state.begin_quick_install(&project) else {
            cx.notify();
            return;
        };
        self.enqueue(job, cx);
    }

    /// Queues the reviewed install and closes the review. The frozen request
    /// (with its fingerprint) leaves with the job; the consumed review can
    /// never be installed twice.
    fn confirm_install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(job) = self.state.confirm_install() else {
            cx.notify();
            return;
        };
        self.enqueue(job, cx);
        self.state.close_details();
        self.dialog.close(window, cx);
        cx.notify();
    }

    /// Dialog Escape/backdrop decision: an in-flight review is cancelled but
    /// stays open; installs live in the queue, so nothing else vetoes.
    fn dialog_veto(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.state.review_job.is_some() {
            self.cancel_review(cx);
            return false;
        }
        true
    }

    fn details_open_changed(
        &mut self,
        open: bool,
        _reason: DialogChangeReason,
        cx: &mut Context<Self>,
    ) {
        if !open && self.state.details_open {
            self.state.close_details();
            cx.notify();
        }
    }

    fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
        let effects = self.state.receive(message);
        self.apply_effects(effects, cx);
    }

    fn apply_effects(&mut self, effects: Effects, cx: &mut Context<Self>) {
        if effects.search_now {
            self.search(false, cx);
        }
        if effects.reload_instances {
            self.load_instances(cx);
        }
        if effects.reload_installed {
            self.load_installed(cx);
        }
        if effects.artwork {
            self.load_artwork();
        }
        if effects.changed {
            cx.emit(DiscoverEvent::Changed);
        }
        if effects.notify {
            cx.notify();
        }
    }
}

// ---------------------------------------------------------------- render

/// Multi-line text as one child per line (no rich formatting invented).
fn text_block(text: &str, color: Hsla, size: Pixels) -> impl IntoElement + use<> {
    let block = div().flex().flex_col().text_size(size).text_color(color);
    text.lines()
        .fold(block, |block, line| block.child(line.to_owned()))
}

impl Render for Discover {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx).clone();

        // Built before the immutable state borrow below; a dialog close that
        // slips past the veto (or an imperative close) still resets
        // per-project state through `details_open_changed`.
        let details_panel = self.details_panel(cx);

        let s = &self.state;
        if s.diagnostic {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .p_6()
                .gap_3()
                .child(div().text_2xl().child("Discover"))
                .child(
                    div()
                        .text_color(palette.muted_foreground)
                        .child("Discover is disabled in diagnostic mode. No catalog requests are made from this view."),
                )
                .child(details_panel);
        }

        let kind_row = div()
            .flex()
            .gap_2()
            .children(KIND_LABELS.iter().enumerate().map(|(index, label)| {
                AuroraButton::new(("kind", index), *label)
                    .variant(ButtonVariant::Ghost)
                    .selected(s.kind == index)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose_kind(index, cx);
                    }))
            }));

        let instance_names: Vec<SharedString> = s
            .instances
            .iter()
            .map(|i| i.display_name.clone().into())
            .collect();
        let target_options: Vec<SharedString> = if instance_names.is_empty() {
            vec!["No instances yet".into()]
        } else {
            instance_names
        };
        let target_selected = s
            .target
            .as_ref()
            .and_then(|id| s.instances.iter().position(|i| i.id == *id));

        let mut filters = div().flex().flex_wrap().gap_2();
        if s.is_pack() {
            filters = filters.child(
                AuroraSelect::new(
                    "loader",
                    "Mod loader",
                    PACK_LOADER_LABELS.iter().map(|l| (*l).into()).collect(),
                    &self.selects.loader.trigger,
                    &self.selects.loader.content,
                    self.selects.loader.bounds.clone(),
                )
                .selected(Some(s.loader_index))
                .highlight(self.selects.loader.highlight)
                .open(self.selects.loader.open)
                .on_open_change(cx.processor(|view, open, _window, cx| {
                    view.set_select_open(SelectSlot::Loader, open, cx);
                }))
                .on_highlight(cx.processor(|view, index, _window, cx| {
                    view.set_select_highlight(SelectSlot::Loader, index, cx);
                }))
                .on_confirm(cx.processor(|view, index, _window, cx| {
                    view.set_loader(index, cx);
                })),
            );
        } else {
            filters = filters.child(
                AuroraSelect::new(
                    "target",
                    "Target instance",
                    target_options,
                    &self.selects.target.trigger,
                    &self.selects.target.content,
                    self.selects.target.bounds.clone(),
                )
                .selected(target_selected)
                .highlight(self.selects.target.highlight)
                .open(self.selects.target.open)
                .on_open_change(cx.processor(|view, open, _window, cx| {
                    view.set_select_open(SelectSlot::Target, open, cx);
                }))
                .on_highlight(cx.processor(|view, index, _window, cx| {
                    view.set_select_highlight(SelectSlot::Target, index, cx);
                }))
                .on_confirm(cx.processor(|view, index, _window, cx| {
                    if !view.state.instances.is_empty() {
                        view.choose_target(index, cx);
                    }
                })),
            );
        }
        let mut category_names = vec!["All categories".to_owned()];
        category_names.extend(s.category_values.iter().map(|v| v.replace('-', " ")));
        filters = filters.child(
            AuroraSelect::new(
                "category",
                "Category",
                category_names.into_iter().map(SharedString::from).collect(),
                &self.selects.category.trigger,
                &self.selects.category.content,
                self.selects.category.bounds.clone(),
            )
            .selected(Some(s.category_index))
            .highlight(self.selects.category.highlight)
            .open(self.selects.category.open)
            .on_open_change(cx.processor(|view, open, _window, cx| {
                view.set_select_open(SelectSlot::Category, open, cx);
            }))
            .on_highlight(cx.processor(|view, index, _window, cx| {
                view.set_select_highlight(SelectSlot::Category, index, cx);
            }))
            .on_confirm(cx.processor(|view, index, _window, cx| {
                view.set_category(index, cx);
            })),
        );
        filters = filters.child(
            AuroraSelect::new(
                "sort",
                "Sort",
                SORT_LABELS.iter().map(|l| (*l).into()).collect(),
                &self.selects.sort.trigger,
                &self.selects.sort.content,
                self.selects.sort.bounds.clone(),
            )
            .selected(Some(s.sort_index))
            .highlight(self.selects.sort.highlight)
            .open(self.selects.sort.open)
            .on_open_change(cx.processor(|view, open, _window, cx| {
                view.set_select_open(SelectSlot::Sort, open, cx);
            }))
            .on_highlight(cx.processor(|view, index, _window, cx| {
                view.set_select_highlight(SelectSlot::Sort, index, cx);
            }))
            .on_confirm(cx.processor(|view, index, _window, cx| {
                view.set_sort(index, cx);
            })),
        );

        let header = div()
            .flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .w(px(320.))
                    .child(AuroraInput::new(&self.query).label("Search Modrinth projects")),
            )
            .child(
                AuroraButton::new("search", "Search")
                    .variant(ButtonVariant::Primary)
                    .disabled(s.searching)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.search(false, cx);
                    })),
            )
            .when(s.has_more() && !s.searching, |this| {
                this.child(AuroraButton::new("more", "More").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.search(true, cx);
                    },
                )))
            });

        let mut status = div().flex().flex_col().text_size(px(12.));
        if let Some(error) = &s.error {
            status = status.text_color(palette.danger).child(error.clone());
        } else if s.searching {
            status = status
                .text_color(palette.muted_foreground)
                .child("Searching Modrinth…");
        } else {
            status = status
                .text_color(palette.muted_foreground)
                .child(s.summary());
        }
        if let Some(error) = &s.instances_error {
            status = status.text_color(palette.danger).child(error.clone());
        }
        if let Some(error) = &s.categories_error {
            status = status.text_color(palette.danger).child(error.clone());
        }
        // The busy target is marked honestly; installs into one instance
        // take turns in Downloads.
        if !s.is_pack() && s.target_busy() {
            status = status.text_color(palette.muted_foreground).child(
                "This instance has queued or running installs; they take turns in Downloads.",
            );
        }

        let rows = s.results.iter().enumerate().map(|(index, project)| {
            let id = project.project_id.clone();
            let installed = s.installed.contains(&project.project_id);
            let row_busy = s.busy_project(&project.project_id);
            let badges = [
                installed.then(|| "Installed".to_owned()),
                row_busy.then(|| "Installing…".to_owned()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
            div()
                .id(("result", index))
                .flex()
                .items_center()
                .gap_3()
                .py_2()
                .border_b_1()
                .border_color(palette.border)
                .child(artwork::tile(
                    ("art", index),
                    s.artwork.cache.image(&project.project_id),
                    &artwork::letter(&project.title),
                    px(48.),
                    &palette,
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(14.))
                                .child(format!("{} · {}", project.title, project.author)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette.muted_foreground)
                                .child(project.summary.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette.muted_foreground)
                                .child(format!(
                                    "{}{}{}",
                                    download_label(project.downloads),
                                    if badges.is_empty() { "" } else { " · " },
                                    badges
                                )),
                        ),
                )
                .child({
                    let id = id.clone();
                    AuroraButton::new(("details", index), "Details").on_click(cx.listener(
                        move |this, _, window, cx| {
                            this.open_details(id.clone(), window, cx);
                        },
                    ))
                })
                .when(!s.is_pack(), |row| {
                    row.child({
                        let id = id.clone();
                        AuroraButton::new(("install", index), "Install")
                            .disabled(installed || row_busy || s.target.is_none())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.quick_install(id.clone(), cx);
                            }))
                    })
                })
        });

        let content = div()
            .id("discover-results")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll()
            .children(rows);

        div()
            .size_full()
            .flex()
            .flex_col()
            .p_6()
            .gap_3()
            .child(div().text_2xl().child("Discover"))
            .when(s.return_request().is_some(), |page| {
                page.child(
                    div().flex().child(
                        AuroraButton::new("return-to-instance", "Back to instance")
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.return_to_instance(cx);
                            })),
                    ),
                )
            })
            .child(header)
            .child(kind_row)
            .child(filters)
            .child(status)
            .child(content)
            .child(details_panel)
    }
}

impl Discover {
    /// The details/review dialog. The base `Dialog` owns the focus trap and
    /// Escape/Enter bindings; `on_cancel` vetoes while a review prepares
    /// (cancelling it instead). Installs live in the Downloads queue, so
    /// confirming never blocks the dialog.
    fn details_panel(&self, cx: &mut Context<Self>) -> Dialog {
        let palette = theme::theme(cx).clone();
        let s = &self.state;
        let pack = s.is_pack();
        let merging = s.merging();
        let details = s.details.as_ref();
        let version = s.selected_version();

        let mut panel =
            div()
                .id("discover-details")
                .w(px(640.))
                .max_h(px(560.))
                .overflow_y_scroll()
                .p(px(16.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .rounded(palette.radius)
                .bg(palette.surface)
                .border_1()
                .border_color(palette.border)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .children(details.map(|p| {
                            artwork::tile(
                                "details-art",
                                s.artwork.cache.image(&p.project_id),
                                &artwork::letter(&p.title),
                                px(48.),
                                &palette,
                            )
                        }))
                        .child(
                            div().text_size(px(16.)).child(
                                details
                                    .map(|p| p.title.clone())
                                    .unwrap_or_else(|| "Project details".into()),
                            ),
                        ),
                )
                .children(details.map(|p| {
                    div()
                        .text_size(px(12.))
                        .text_color(palette.muted_foreground)
                        .child(p.summary.clone())
                }))
                .children(details.map(|p| {
                    text_block(&publisher_lists(p, pack), palette.muted_foreground, px(12.))
                }))
                .when(!s.local_pack_note.is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(12.))
                            .text_color(palette.muted_foreground)
                            .child(s.local_pack_note.clone()),
                    )
                });

        // Version choice: one annotated row per version, core-annotated.
        let version_options: Vec<SharedString> = details
            .map(|p| {
                p.versions
                    .iter()
                    .map(|v| version_label(v, pack).into())
                    .collect()
            })
            .unwrap_or_default();
        panel = panel.child(
            AuroraSelect::new(
                "version",
                if pack { "Pack version" } else { "Version" },
                version_options,
                &self.selects.version.trigger,
                &self.selects.version.content,
                self.selects.version.bounds.clone(),
            )
            .selected(s.version_index)
            .highlight(self.selects.version.highlight)
            .open(self.selects.version.open)
            .on_open_change(cx.processor(|view, open, _window, cx| {
                view.set_select_open(SelectSlot::Version, open, cx);
            }))
            .on_highlight(cx.processor(|view, index, _window, cx| {
                view.set_select_highlight(SelectSlot::Version, index, cx);
            }))
            .on_confirm(cx.processor(|view, index, _window, cx| {
                view.version_selected(index, cx);
            })),
        );

        if pack {
            if let Some(v) = version {
                panel = panel.child(text_block(&version_detail(v), palette.foreground, px(12.)));
                if let Some(reason) = &v.unsupported_reason {
                    panel = panel.child(
                        div()
                            .text_size(px(12.))
                            .text_color(palette.danger)
                            .child(reason.clone()),
                    );
                }
            } else if details.is_some_and(|p| !p.versions.is_empty()) {
                let all_unsupported = details
                    .is_some_and(|p| p.versions.iter().all(|v| v.unsupported_reason.is_some()));
                panel = panel.child(div().text_size(px(12.)).text_color(palette.danger).child(
                    if all_unsupported {
                        "Aurora can't install any version of this pack. Each version says why."
                    } else {
                        "Choose a pack version."
                    },
                ));
            }
            // Destination mode: a new instance, or adding to a compatible one.
            panel = panel.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        AuroraButton::new("pack-new", "New instance")
                            .variant(ButtonVariant::Ghost)
                            .selected(!merging)
                            .disabled(s.details_loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.pack_mode_selected(0, cx);
                            })),
                    )
                    .child(
                        AuroraButton::new("pack-existing", "Add to instance")
                            .variant(ButtonVariant::Ghost)
                            .selected(merging)
                            .disabled(s.details_loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.pack_mode_selected(1, cx);
                            })),
                    ),
            );
            if merging {
                panel = panel.child(
                    div()
                        .text_size(px(12.))
                        .text_color(palette.muted_foreground)
                        .child(s.destinations_note()),
                );
                panel = panel.children(s.destinations.iter().enumerate().map(|(index, row)| {
                    let selected =
                        row.compatible && s.destination.as_ref() == Some(&row.instance_id);
                    AuroraButton::new(("destination", index), destination_detail(row))
                        .variant(ButtonVariant::Ghost)
                        .selected(selected)
                        .disabled(!row.compatible)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.destination_selected(index, cx);
                        }))
                }));
            }
        }

        if !s.review_progress.is_empty() {
            panel = panel.child(
                div()
                    .text_size(px(12.))
                    .text_color(palette.muted_foreground)
                    .child(s.review_progress.clone()),
            );
        }
        if let Some(error) = &s.detail_error {
            panel = panel.child(
                div()
                    .text_size(px(12.))
                    .text_color(palette.danger)
                    .child(error.clone()),
            );
        }

        // Optional pack files, from the completed review only.
        if !s.optional_files.is_empty() {
            panel = panel
                .child(div().text_size(px(12.)).child("Optional files"))
                .children(s.optional_files.iter().enumerate().map(|(index, path)| {
                    let label: SharedString = format!("Install optional file {path}").into();
                    let toggle_view = cx.entity();
                    let toggle_path = path.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            AuroraToggle::new(("optional", index))
                                .label(label)
                                .checked(s.selected_optional.contains(path))
                                .on_change(move |_next, _event, _window, cx| {
                                    toggle_view.update(cx, |view, cx| {
                                        view.toggle_optional(toggle_path.clone(), cx)
                                    });
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(palette.muted_foreground)
                                .child(path.clone()),
                        )
                }));
        }

        panel = panel.children(
            s.review_facts
                .iter()
                .map(|(label, value)| div().text_size(px(12.)).child(format!("{label}: {value}"))),
        );
        if !s.review_text.is_empty() {
            panel = panel.child(text_block(&s.review_text, palette.foreground, px(12.)));
        }

        let reviewing = s.review_job.is_some();
        panel = panel.child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .pt(px(6.))
                .child(
                    AuroraButton::new("review", "Review install")
                        .disabled(!s.can_review())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.review(cx);
                        })),
                )
                .when(reviewing, |row| {
                    row.child(
                        AuroraButton::new("cancel-review", "Cancel review").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.cancel_review(cx);
                            },
                        )),
                    )
                })
                .child(
                    AuroraButton::new("confirm-install", s.confirm_label())
                        .variant(ButtonVariant::Primary)
                        .disabled(!s.can_install())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.confirm_install(window, cx);
                        })),
                )
                .child(DialogClose::new().trigger(|button| {
                    button
                        .px(px(12.))
                        .py(px(6.))
                        .rounded(palette.radius)
                        .text_size(px(13.))
                        .border_1()
                        .border_color(palette.border)
                        .hover(|style| style.bg(palette.surface_hover))
                        .child("Close")
                })),
        );

        let entity = cx.entity();
        let dialog_view = entity.clone();
        Dialog::new(cx)
            .handle(self.dialog.clone())
            .on_open_change(move |open, reason, _window, cx| {
                dialog_view.update(cx, |view, cx| {
                    view.details_open_changed(open, reason, cx);
                });
            })
            // Enter must not confirm an install implicitly; buttons own their
            // own keyboard activation.
            .on_ok(|_, _, _| false)
            .on_cancel(move |_, window, cx| {
                entity.update(cx, |view, cx| view.dialog_veto(window, cx))
            })
            .close_on_backdrop_press(!reviewing)
            .backdrop(div().absolute().size_full().bg(gpui_kit::rgba(0x000000B3)))
            .popup(panel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurora_launcher_lib::content_jobs::Phase as JobPhase;

    fn project(id: &str) -> api::ProjectSummary {
        api::ProjectSummary {
            project_id: id.into(),
            title: id.into(),
            summary: String::new(),
            author: String::new(),
            downloads: 0,
        }
    }

    fn version(
        id: &str,
        channel: &str,
        loader: &str,
        game: &str,
        reason: Option<&str>,
    ) -> api::VersionChoice {
        api::VersionChoice {
            id: id.into(),
            name: format!("Pack {id}"),
            version_number: id.into(),
            loaders: vec![loader.into()],
            version_type: channel.into(),
            game_versions: vec![game.into()],
            archive_size: Some(12 * 1024 * 1024),
            unsupported_reason: reason.map(str::to_owned),
        }
    }

    /// As the core reports it: Quilt and an old Forge annotated, the newest
    /// installable release chosen as the default.
    fn pack_details() -> api::ProjectDetails {
        api::ProjectDetails {
            project_id: "PACK0001".into(),
            title: "Fixture Pack".into(),
            summary: "A fixture".into(),
            game_versions: vec!["1.12.2".into(), "1.21.1".into()],
            loaders: vec!["fabric".into(), "forge".into(), "quilt".into()],
            versions: vec![
                version("3.0-beta", "beta", "fabric", "1.21.1", None),
                version(
                    "2.9",
                    "release",
                    "quilt",
                    "1.21.1",
                    Some(
                        "This Modrinth version is a Quilt modpack. Aurora does not support Quilt.",
                    ),
                ),
                version("2.8", "release", "fabric", "1.21.1", None),
                version(
                    "1.0",
                    "release",
                    "forge",
                    "1.12.2",
                    Some("This Modrinth version is a Forge modpack for Minecraft 1.12.2."),
                ),
            ],
            default_version_id: Some("2.8".into()),
        }
    }

    fn content_details() -> api::ProjectDetails {
        api::ProjectDetails {
            project_id: "MOD0001".into(),
            title: "Fixture Mod".into(),
            summary: String::new(),
            game_versions: vec!["1.21.1".into()],
            loaders: vec!["fabric".into()],
            versions: vec![version("1", "release", "fabric", "1.21.1", None)],
            default_version_id: None,
        }
    }

    fn new_pack_preview() -> Value {
        json!({"fingerprint":"new-fp","name":"Fixture Pack","packVersion":"2.8","minecraftVersion":"1.21.1",
            "loader":"fabric","loaderVersion":"0.16.10",
            "recognized":[{"path":"mods/sodium.jar","projectId":"AANobbMI","versionId":"x","title":"Sodium"}],
            "unresolved":["mods/external.jar"],"overrides":["config/a.toml"],"optional":["mods/opt.jar"],
            "excluded":["mods/server-only.jar","mods/opt.jar"],"skippedOverrides":["server.properties"]})
    }

    fn merge_preview() -> Value {
        json!({"fingerprint":"merge-fp","pack":{"name":"Fixture Pack","packVersion":"2.8","minecraftVersion":"1.21.1",
                "loader":"fabric","loaderVersion":"0.16.10","optional":[],"excluded":[],"skippedOverrides":[]},
            "instanceId":"good0001","instanceMinecraftVersion":"1.21.1","instanceLoader":"fabric","instanceLoaderVersion":"0.19.5",
            "added":["mods/sodium.jar"],"alreadyPresent":["mods/same.jar"],"keptExisting":["config/user.toml"],
            "overridesWritten":["config/new.toml"],"untouchedMods":["user.jar"],
            "componentCount":1,"componentLimit":64,"blockers":[]})
    }

    fn destinations() -> Vec<api::PackDestination> {
        let row = |id: &str, name: &str, game: &str, loader: &str, reasons: &[&str]| {
            api::PackDestination {
                instance_id: id.into(),
                display_name: name.into(),
                minecraft_version: game.into(),
                loader: loader.into(),
                loader_version: Some("0.19.5".into()),
                compatible: reasons.is_empty(),
                reasons: reasons.iter().map(|r| r.to_string()).collect(),
            }
        };
        vec![
            row(
                "bad00001",
                "Forge world",
                "1.21.1",
                "forge",
                &["This instance uses Forge."],
            ),
            row("good0001", "Fabric world", "1.21.1", "fabric", &[]),
            row(
                "old00001",
                "Old world",
                "1.20.1",
                "fabric",
                &["This instance has Minecraft 1.20.1."],
            ),
        ]
    }

    fn state_with_pack_details() -> DiscoverState {
        let mut state = DiscoverState {
            kind: 3,
            ..Default::default()
        };
        state.apply_details(pack_details(), None);
        state
    }

    #[test]
    fn local_pack_navigation_selects_the_exact_version_without_reviewing_or_installing() {
        let mut state = DiscoverState::default();
        state.target = Some("content-target".into());
        let old_search = state.begin_search(false, "old query".into()).unwrap();
        let request = state
            .begin_local_pack(
                "PACK0001".into(),
                "3.0-beta".into(),
                "Exact local file".into(),
            )
            .unwrap();
        assert_eq!(request.kind, "modpack");
        assert_eq!(request.project, "PACK0001");
        assert!(request.target.is_empty());
        assert_eq!(request.preferred.as_deref(), Some("3.0-beta"));
        assert!(
            !state
                .receive(Message::Search {
                    serial: old_search.serial,
                    result: Ok(api::SearchPage {
                        offset: 0,
                        total_hits: 1,
                        hits: vec![project("old")]
                    }),
                })
                .notify
        );
        state.receive(Message::Details {
            serial: request.serial,
            preferred: request.preferred,
            result: Ok(pack_details()),
        });
        assert_eq!(state.selected_version().unwrap().id, "3.0-beta");
        assert_eq!(state.local_pack_note, "Exact local file");
        assert!(state.review.is_none() && state.review_job.is_none());

        let next = state.begin_details("another-project".into()).unwrap();
        assert!(next.preferred.is_none());
        assert!(
            state.local_pack_note.is_empty(),
            "local identity never leaks into later browsing"
        );
    }

    #[test]
    fn a_missing_local_version_never_falls_back_to_the_provider_default() {
        let mut state = DiscoverState::default();
        let request = state
            .begin_local_pack("PACK0001".into(), "missing".into(), String::new())
            .unwrap();
        state.receive(Message::Details {
            serial: request.serial,
            preferred: request.preferred,
            result: Ok(pack_details()),
        });
        assert!(state.version_index.is_none());
        assert!(
            state
                .detail_error
                .as_deref()
                .unwrap()
                .contains("exact pack version")
        );
        assert!(!state.can_review());
        assert!(state.begin_review().is_none());
    }

    #[test]
    fn a_late_instance_bootstrap_does_not_supersede_local_pack_details() {
        let mut state = DiscoverState::default();
        let serial = state.begin_instances();
        let request = state
            .begin_local_pack("PACK0001".into(), "2.8".into(), "Local".into())
            .unwrap();
        let fx = state.receive(Message::Instances(
            serial,
            Ok(api::LauncherState {
                config: api::ConfigSummary {
                    selected_instance_id: Some("selected".into()),
                },
                instances: vec![instance("selected", "Selected instance")],
                platform_capabilities: vec![],
                icons: Default::default(),
            }),
        ));
        assert!(fx.notify);
        assert_eq!(state.detail_serial, request.serial);
        assert!(state.details_loading);
        state.receive(Message::Details {
            serial: request.serial,
            preferred: request.preferred,
            result: Ok(pack_details()),
        });
        assert_eq!(state.selected_version().unwrap().id, "2.8");
        assert_eq!(state.local_pack_note, "Local");
    }

    #[test]
    fn local_pack_navigation_never_starts_from_invalid_identity() {
        let mut state = DiscoverState::default();
        assert!(
            state
                .begin_local_pack(" ".into(), "v".into(), String::new())
                .is_none()
        );
        assert!(
            state
                .begin_local_pack("p".into(), " ".into(), String::new())
                .is_none()
        );
        state.diagnostic = true;
        assert!(
            state
                .begin_local_pack("p".into(), "v".into(), String::new())
                .is_none()
        );
        state.diagnostic = false;
        // Queue installs are retained elsewhere, so navigation continues
        // honestly while queued work runs; only this view's markers show it.
        state.busy_projects.insert("OTHER".into());
        state.busy_instances.insert("busy0001".into());
        assert!(
            state
                .begin_local_pack("p".into(), "v".into(), String::new())
                .is_some()
        );
        assert_eq!(state.kind, 3, "local pack navigation selects the pack flow");
    }

    /// The fixture pack opened, destination chosen (merge), and reviewed. The
    /// request is exactly what `begin_review` freezes for that mode.
    fn reviewed(merge: bool) -> DiscoverState {
        let mut state = state_with_pack_details();
        let mut request = json!({"projectId":"PACK0001","versionId":"2.8","selectedOptional":[],"fingerprint":null});
        if merge {
            state.set_pack_mode(1);
            state.destinations_for = Some(("PACK0001".into(), "2.8".into()));
            state.apply_destinations(destinations());
            state.destination = Some("good0001".into());
            request["instanceId"] = json!("good0001");
        }
        let value = if merge {
            merge_preview()
        } else {
            new_pack_preview()
        };
        let snapshot = Review {
            target: if merge {
                "good0001".into()
            } else {
                String::new()
            },
            kind: "modpack",
            project: "PACK0001".into(),
            version: "2.8".into(),
            merge,
            request,
            fingerprint: String::new(),
        };
        state.apply_review(snapshot, &value);
        state
    }

    // ------------------------------------------------------- label helpers

    #[test]
    fn pagination_deduplicates_provider_ids_and_bounds_retention() {
        let mut rows = vec![project("one")];
        append_unique(
            &mut rows,
            vec![project("one"), project("two"), project("two")],
        );
        assert_eq!(
            rows.iter()
                .map(|p| p.project_id.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
        append_unique(
            &mut rows,
            (0..300).map(|i| project(&i.to_string())).collect(),
        );
        assert_eq!(rows.len(), MAX_RESULTS);
    }

    #[test]
    fn review_blocks_conflicts_and_never_invents_an_install_fingerprint() {
        assert!(
            !review_description(
                &json!({"preview":{"items":[]},"conflicts":[]}),
                "mod",
                false
            )
            .1
        );
        let blocked = json!({"previewFingerprint":"abc","preview":{"items":[]},"conflicts":[{"fileName":"user.jar"}]});
        assert!(!review_description(&blocked, "mod", false).1);
        let merge = json!({"fingerprint":"abc","pack":{"name":"Pack","minecraftVersion":"1.21.11"},
            "blockers":["Wrong Minecraft"],"keptExisting":["config/user.json"]});
        let (_, allowed, display) = review_description(&merge, "modpack", true);
        assert!(!allowed);
        assert!(display.contains("Blocked (1)\nWrong Minecraft\n"));
        assert!(display.contains(
            "Your differing config files are kept; the pack's versions are not applied (1)\nconfig/user.json\n"
        ));
        let (_, allowed, display) = review_description(&merge_preview(), "modpack", true);
        assert!(allowed);
        assert!(display.contains("Your existing mods, left untouched (1)\nuser.jar\n"));
        assert!(!display.contains("Blocked"), "empty groups are omitted");
    }

    #[test]
    fn pack_review_facts_label_core_decisions() {
        let state = reviewed(false);
        let version = state.selected_version().cloned();
        let facts = pack_review_facts(&new_pack_preview(), false, version.as_ref(), None);
        let fact = |label: &str| {
            facts
                .iter()
                .find(|(l, _)| l == label)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| panic!("no {label} fact"))
        };
        assert_eq!(fact("Pack version"), "2.8 · Release");
        assert_eq!(fact("Minecraft"), "1.21.1 · set by the pack");
        assert_eq!(fact("Mod loader"), "Fabric 0.16.10 · set by the pack");
        assert_eq!(fact("Destination"), "A new instance named after the pack");
        assert!(fact("Pack archive").starts_with("12.0 MB from Modrinth"));

        let merge = reviewed(true);
        let version = merge.selected_version().cloned();
        let destination = merge.chosen_destination().cloned();
        let facts = pack_review_facts(
            &merge_preview(),
            true,
            version.as_ref(),
            destination.as_ref(),
        );
        let fact = |label: &str| {
            facts
                .iter()
                .find(|(l, _)| l == label)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| panic!("no {label} fact"))
        };
        assert_eq!(fact("Destination"), "Fabric world (existing instance)");
        assert_eq!(fact("Components"), "1 added, at most 64 per merge");
        assert_eq!(
            fact("Pack updates"),
            "This instance will not follow future updates of this pack."
        );
    }

    #[test]
    fn review_progress_is_honest_about_what_is_known() {
        let mb = 1024 * 1024;
        assert_eq!(
            review_progress(true, 0, Some(123 * mb)).0,
            "Getting the pack archive (123.0 MB) to review it…"
        );
        let (text, fraction) = review_progress(true, 41 * mb, Some(123 * mb));
        assert_eq!(
            text,
            "Downloading the pack archive to review it: 41.0 MB of 123.0 MB"
        );
        assert!((fraction - 41. / 123.).abs() < 1e-6);
        assert_eq!(review_progress(false, 0, None).1, -1.);
    }

    #[test]
    fn inventory_markers_only_count_the_active_kind() {
        let rows = json!([
            {"record": {"contentType": "mod", "projectId": "A"}},
            {"record": {"contentType": "shaderPack", "projectId": "B"}}
        ]);
        let installed = installed_from_inventory(&rows, "mod");
        assert!(installed.contains("A"));
        assert!(!installed.contains("B"));
    }

    // ----------------------------------------------- version selection

    #[test]
    fn version_selection_follows_the_core_default_and_local_pinnings() {
        let pack = pack_details();
        assert_eq!(default_version_index(&pack, true, None), Some(2));
        assert_eq!(default_version_index(&pack, true, Some("1.0")), Some(3));
        // A local archive installs exactly its own version, never a substitute.
        assert_eq!(default_version_index(&pack, true, Some("gone")), None);
        let content = content_details();
        assert_eq!(default_version_index(&content, false, None), Some(0));
        let mut unsupported_only = pack_details();
        unsupported_only
            .versions
            .retain(|v| v.unsupported_reason.is_some());
        unsupported_only.default_version_id = None;
        assert_eq!(default_version_index(&unsupported_only, true, None), None);
    }

    #[test]
    fn apply_details_reports_uninstallable_and_missing_versions() {
        let mut state = state_with_pack_details();
        assert_eq!(state.version_index, Some(2));
        assert!(state.detail_error.is_none());
        state.apply_details(pack_details(), Some("gone"));
        assert_eq!(state.version_index, None);
        assert!(
            state
                .detail_error
                .as_deref()
                .is_some_and(|e| e.contains("no longer listed"))
        );
        let mut empty = pack_details();
        empty.versions.clear();
        state.apply_details(empty, None);
        assert!(
            state
                .detail_error
                .as_deref()
                .is_some_and(|e| e.contains("no published versions"))
        );
    }

    // ------------------------------------------------------ stale guards

    fn page(ids: &[&str]) -> api::SearchPage {
        api::SearchPage {
            offset: 0,
            total_hits: ids.len() as u32,
            hits: ids.iter().map(|id| project(id)).collect(),
        }
    }

    #[test]
    fn only_the_newest_search_result_applies() {
        let mut state = DiscoverState::default();
        state.kind = 3; // modpack browses without a target
        let first = state.begin_search(false, "a".into()).unwrap();
        let second = state.begin_search(false, "b".into()).unwrap();
        assert!(state.searching);
        let fx = state.receive(Message::Search {
            serial: first.serial,
            result: Ok(page(&["stale"])),
        });
        assert!(!fx.notify, "a superseded search result changes nothing");
        assert!(state.results.is_empty());
        let fx = state.receive(Message::Search {
            serial: second.serial,
            result: Ok(page(&["fresh"])),
        });
        assert!(fx.notify);
        assert_eq!(state.results.len(), 1);
        assert_eq!(state.results[0].project_id, "fresh");
        assert!(!state.searching);
    }

    #[test]
    fn append_searches_are_blocked_while_one_is_in_flight() {
        let mut state = DiscoverState::default();
        state.kind = 3;
        state.begin_search(false, String::new()).unwrap();
        assert!(state.begin_search(true, String::new()).is_none());
        state.receive(Message::Search {
            serial: state.serial,
            result: Ok(api::SearchPage {
                offset: 20,
                total_hits: 100,
                hits: vec![project("one")],
            }),
        });
        assert_eq!(state.next_offset, 40);
        assert!(state.has_more());
        let request = state.begin_search(true, String::new()).unwrap();
        assert_eq!(request.offset, 40);
    }

    #[test]
    fn late_details_and_destination_results_are_discarded() {
        let mut state = state_with_pack_details();
        let first = state.begin_details("PACK0001".into()).unwrap();
        let second = state.begin_details("PACK0001".into()).unwrap();
        let fx = state.receive(Message::Details {
            serial: first.serial,
            preferred: None,
            result: Ok(pack_details()),
        });
        assert!(!fx.notify);
        assert!(state.details.is_none(), "only the newest details apply");
        state.receive(Message::Details {
            serial: second.serial,
            preferred: None,
            result: Ok(pack_details()),
        });
        assert!(state.details.is_some());

        state.set_pack_mode(1);
        let request = state.begin_destinations().unwrap();
        // A version change supersedes the destination request.
        state.version_changed(0);
        let fx = state.receive(Message::Destinations {
            serial: request.serial,
            key: request_key(&request),
            result: Ok(destinations()),
        });
        assert!(!fx.notify);
        assert!(state.destinations.is_empty());
        let fresh = state.begin_destinations().unwrap();
        let fx = state.receive(Message::Destinations {
            serial: fresh.serial,
            key: request_key(&fresh),
            result: Ok(destinations()),
        });
        assert!(fx.notify);
        assert_eq!(state.destinations.len(), 3);
    }

    fn request_key(request: &DestinationRequest) -> (String, String) {
        (request.project.clone(), request.version.clone())
    }

    #[test]
    fn stale_inventory_results_are_discarded() {
        let mut state = DiscoverState::default();
        state.kind = 0;
        state.serial = 5;
        state.target = Some("i1".into());
        let stale = state.begin_inventory().unwrap();
        state.begin_search(false, "new search".into()).unwrap();
        let fx = state.receive(Message::Inventory {
            request: stale,
            result: Ok(json!([
                {"record": {"contentType": "mod", "projectId": "A"}}
            ])),
        });
        assert!(!fx.notify);
        assert!(state.installed.is_empty());
        let request = state.begin_inventory().unwrap();
        let fx = state.receive(Message::Inventory {
            request,
            result: Ok(json!([
                {"record": {"contentType": "mod", "projectId": "A"}}
            ])),
        });
        assert!(fx.notify);
        assert!(state.installed.contains("A"));
    }

    #[test]
    fn older_inventory_cannot_overwrite_a_post_install_refresh_for_the_same_search() {
        let mut state = DiscoverState {
            serial: 5,
            target: Some("i1".into()),
            ..Default::default()
        };
        let before_install = state.begin_inventory().unwrap();
        let after_install = state.begin_inventory().unwrap();
        state.receive(Message::Inventory {
            request: after_install,
            result: Ok(json!([{"record": {"contentType": "mod", "projectId": "A"}}])),
        });
        let fx = state.receive(Message::Inventory {
            request: before_install,
            result: Ok(json!([])),
        });
        assert_eq!(state.serial, 5, "both reads belong to the same search");
        assert!(!fx.notify);
        assert!(state.installed.contains("A"));
    }

    #[test]
    fn content_changes_refresh_installed_markers_without_restarting_search() {
        let mut state = DiscoverState {
            serial: 5,
            target: Some("i1".into()),
            installed: HashSet::from(["A".into()]),
            ..Default::default()
        };
        assert!(state.begin_quick_install("A").is_none());
        let stale = state.begin_inventory().unwrap();
        let fx = state.content_changed();
        assert!(fx.reload_installed && !fx.search_now);
        let request = state.begin_inventory().unwrap();
        state.receive(Message::Inventory {
            request,
            result: Ok(json!([])),
        });
        state.receive(Message::Inventory {
            request: stale,
            result: Ok(json!([{"record": {"contentType": "mod", "projectId": "A"}}])),
        });
        assert!(state.installed.is_empty());
        assert!(state.begin_quick_install("A").is_some());
    }

    #[test]
    fn returning_to_discover_refreshes_inventory_for_an_existing_target() {
        let mut state = DiscoverState {
            serial: 5,
            target: Some("i1".into()),
            ..Default::default()
        };
        let serial = state.begin_instances();
        let fx = state.receive(Message::Instances(
            serial,
            Ok(api::LauncherState {
                config: api::ConfigSummary {
                    selected_instance_id: Some("i1".into()),
                },
                instances: vec![instance("i1", "World")],
                platform_capabilities: vec![],
                icons: Default::default(),
            }),
        ));
        assert!(fx.reload_installed && !fx.search_now);
        assert_eq!(state.serial, 5);
    }

    // -------------------------------------------- destinations & choice

    #[test]
    fn only_compatible_instances_can_be_chosen_as_destination() {
        let mut state = state_with_pack_details();
        state.set_pack_mode(1);
        state.destinations_for = Some(("PACK0001".into(), "2.8".into()));
        state.apply_destinations(destinations());
        assert!(state.chosen_destination().is_none());
        assert!(!state.can_review(), "a destination must be chosen first");
        assert!(!state.choose_destination(0), "incompatible choice ignored");
        assert_eq!(state.destination, None);
        assert!(state.choose_destination(1));
        assert_eq!(state.destination.as_deref(), Some("good0001"));
        assert!(state.chosen_destination().is_some());
    }

    #[test]
    fn a_kept_destination_survives_only_while_the_core_says_compatible() {
        let mut state = DiscoverState {
            destination: Some("good0001".into()),
            ..Default::default()
        };
        state.apply_destinations(destinations());
        assert_eq!(state.destination.as_deref(), Some("good0001"));
        let mut changed = destinations();
        changed[1].compatible = false;
        changed[1].reasons = vec!["Finish installing this instance first.".into()];
        state.apply_destinations(changed);
        assert_eq!(state.destination, None);
        // Opened from an instance: that instance is preselected when compatible.
        let mut state = DiscoverState {
            origin: Some(("good0001".into(), 3)),
            ..Default::default()
        };
        state.apply_destinations(destinations());
        assert_eq!(state.destination.as_deref(), Some("good0001"));
        let mut state = DiscoverState {
            origin: Some(("bad00001".into(), 3)),
            ..Default::default()
        };
        state.apply_destinations(destinations());
        assert_eq!(state.destination, None);
    }

    #[test]
    fn an_unsupported_version_is_never_reviewed() {
        let mut state = state_with_pack_details();
        state.version_changed(1); // Quilt: unsupported
        assert!(
            state
                .selected_version()
                .unwrap()
                .unsupported_reason
                .is_some()
        );
        assert!(!state.version_installable());
        assert!(state.begin_review().is_none());
        state.version_changed(3); // old Forge: unsupported
        assert!(state.begin_review().is_none());
        state.version_changed(2);
        assert!(state.begin_review().is_some());
    }

    #[test]
    fn review_requires_a_target_instance_for_content() {
        let mut state = DiscoverState::default();
        state.apply_details(content_details(), None);
        assert!(state.begin_review().is_none());
        assert!(
            state
                .detail_error
                .as_deref()
                .is_some_and(|e| e.contains("target instance"))
        );
        state.target = Some("i1".into());
        assert!(state.begin_review().is_some());
    }

    #[test]
    fn review_requires_a_chosen_destination_for_merges() {
        let mut state = state_with_pack_details();
        state.set_pack_mode(1);
        state.destinations_for = Some(("PACK0001".into(), "2.8".into()));
        state.apply_destinations(destinations());
        // The Review control stays disabled while no compatible destination
        // is chosen; `begin_review` is a no-op and the destinations note
        // explains why (the core decides compatibility, not this view).
        assert!(!state.can_review());
        assert!(state.begin_review().is_none());
        assert!(state.detail_error.is_none());
        assert!(
            state
                .destinations_note()
                .contains("Only compatible instances can be chosen")
        );
    }

    // ------------------------------------------------- review lifecycle

    #[test]
    fn a_started_review_freezes_its_request_and_is_cancellable() {
        let mut state = state_with_pack_details();
        let start = state.begin_review().unwrap();
        assert_eq!(start.serial, state.detail_serial);
        assert_eq!(
            start.snapshot.request,
            json!({"projectId":"PACK0001","versionId":"2.8","selectedOptional":[],"fingerprint":null})
        );
        assert!(state.review_cancellable);
        assert!(state.review_job.is_some());
        assert!(state.details_loading);
        assert_eq!(
            state.review_progress,
            "Getting the pack archive (12.0 MB) to review it…"
        );

        // Explicit cancel stops the job; its late result is discarded.
        let job = start.control.clone();
        assert!(state.cancel_review());
        assert_eq!(job.phase(), JobPhase::Cancelling);
        assert!(state.review_job.is_none());
        assert_eq!(
            state.review_text,
            "Review cancelled. Nothing was installed."
        );
        let fx = state.receive(Message::Review {
            serial: start.serial,
            snapshot: start.snapshot,
            observed_cancellation: false,
            result: Ok(new_pack_preview()),
        });
        assert!(!fx.notify, "the cancelled review's result is stale");
        assert!(state.review.is_none());
    }

    #[test]
    fn an_observed_cancellation_result_never_becomes_a_review() {
        let mut state = state_with_pack_details();
        let start = state.begin_review().unwrap();
        let fx = state.receive(Message::Review {
            serial: start.serial,
            snapshot: start.snapshot,
            observed_cancellation: true,
            result: Ok(new_pack_preview()),
        });
        assert!(fx.notify);
        assert!(state.review_job.is_none());
        assert!(state.review.is_none());
        assert_eq!(
            state.review_text,
            "Review cancelled. Nothing was installed."
        );
    }

    #[test]
    fn changing_version_destination_or_optional_files_clears_the_review() {
        let cleared = |state: &DiscoverState, why: &str| {
            assert!(state.review.is_none(), "{why}");
            assert!(state.review_facts.is_empty(), "{why}");
            assert!(!state.can_install(), "{why}");
        };

        let mut state = reviewed(false);
        assert!(state.can_install());
        state.version_changed(0);
        cleared(&state, "version");

        let mut state = reviewed(true);
        assert!(state.can_install());
        state.set_pack_mode(0);
        cleared(&state, "destination mode");

        let mut state = reviewed(true);
        state.destinations[2].compatible = true;
        state.destinations[2].reasons.clear();
        assert!(state.choose_destination(2));
        cleared(&state, "destination instance");

        // Optional files: the selection is part of the fingerprinted request.
        let mut state = reviewed(false);
        assert_eq!(state.optional_files, vec!["mods/opt.jar".to_owned()]);
        state.toggle_optional("mods/opt.jar");
        cleared(&state, "optional");
        assert!(state.selected_optional.contains("mods/opt.jar"));
        assert!(state.review_text.contains("Review installation again"));
        // A path the review never offered changes nothing.
        let mut state = reviewed(false);
        state.toggle_optional("mods/unknown.jar");
        assert!(state.can_install());
    }

    #[test]
    fn a_completed_review_is_presented_and_gates_confirmation() {
        let state = reviewed(false);
        assert_eq!(state.review_facts.len(), 5);
        assert!(
            state
                .review_text
                .contains("Modrinth components installed (1)\nmods/sodium.jar\n")
        );
        assert!(
            state.review_text.contains(
                "Not installed: pack files outside game content (1)\nserver.properties\n"
            )
        );
        let snapshot = state.review.clone().unwrap();
        assert_eq!(snapshot.fingerprint, "new-fp");
        assert_eq!(snapshot.request["fingerprint"], json!("new-fp"));
        assert_eq!(state.confirm_label(), "Create instance");

        // Conflicts in a merge keep confirmation disabled.
        let mut state = reviewed(true);
        assert_eq!(state.confirm_label(), "Add to instance");
        let mut blocked = merge_preview();
        blocked["blockers"] = json!(["This pack needs Fabric Loader 0.17.0 or newer."]);
        state.apply_review(
            Review {
                target: "good0001".into(),
                kind: "modpack",
                project: "PACK0001".into(),
                version: "2.8".into(),
                merge: true,
                request: json!({}),
                fingerprint: String::new(),
            },
            &blocked,
        );
        assert!(state.review.is_none());
        assert!(state.review_text.contains("Blocked (1)"));
    }

    // ----------------------------------------------------- submissions

    fn instance(id: &str, name: &str) -> api::InstanceSummary {
        serde_json::from_value(json!({
            "id": id, "displayName": name, "state": "ready", "aurora": null,
            "platform": {"kind": "vanilla"}, "minecraftVersion": "1.21.1",
            "configuration": {"minecraftVersion": "1.21.1", "auroraEnabled": false,
                "loader": {"kind": "vanilla"}, "memoryMib": 4096,
                "additionalJvmArguments": "", "window": null}
        }))
        .unwrap()
    }

    #[test]
    fn confirm_install_consumes_the_review_into_a_fingerprinted_request() {
        let mut state = reviewed(false);
        let job = state.confirm_install().unwrap();
        assert_eq!(job.title, "Fixture Pack");
        assert_eq!(job.detail, "New modpack instance");
        match &job.request {
            JobRequest::Pack { merge, request } => {
                assert!(!merge);
                assert_eq!(request["projectId"], json!("PACK0001"));
                assert_eq!(request["fingerprint"], json!("new-fp"));
                assert_eq!(request["versionId"], json!("2.8"));
            }
            _ => panic!("pack review must queue as a pack"),
        }
        // A new pack instance touches no existing instance, so it carries no
        // serialization key and can start immediately.
        assert_eq!(job.request.key(), None);
        assert!(state.review.is_none());
        assert!(!state.can_install(), "one review installs once");
        assert!(state.confirm_install().is_none());

        let mut state = reviewed(true);
        let job = state.confirm_install().unwrap();
        match &job.request {
            JobRequest::Pack { merge, request } => {
                assert!(merge);
                assert_eq!(request["instanceId"], json!("good0001"));
            }
            _ => panic!("merge review must queue as a merge"),
        }
        assert_eq!(job.request.key().as_deref(), Some("good0001"));
        assert_eq!(job.detail, "Into ");
    }

    #[test]
    fn quick_install_requires_a_target_and_a_content_kind() {
        let mut state = DiscoverState::default();
        state.kind = 0;
        state.results = vec![project("MOD0001")];
        assert!(state.begin_quick_install("MOD0001").is_none(), "no target");
        state.target = Some("i1".into());
        state.instances = vec![instance("i1", "World")];
        let job = state.begin_quick_install("MOD0001").unwrap();
        assert!(matches!(
            &job.request,
            JobRequest::Quick { kind: "mod", target, project }
                if target == "i1" && project == "MOD0001"
        ));
        assert_eq!(job.title, "MOD0001");
        assert_eq!(job.detail, "Into World");
        assert_eq!(job.request.key().as_deref(), Some("i1"));
        state.installed.insert("MOD0001".into());
        assert!(state.begin_quick_install("MOD0001").is_none(), "installed");
        state.kind = 3;
        assert!(
            state.begin_quick_install("PACK0001").is_none(),
            "packs review first"
        );
    }

    #[test]
    fn queued_work_blocks_only_its_own_project_and_marks_targets_honestly() {
        // A reviewed job for the same project is queued; a different project
        // stays installable, and browsing/search/review continue (the queue,
        // not this view, serializes per instance).
        let mut state = reviewed(false);
        state.kind = 0;
        state.apply_details(content_details(), None);
        state.target = Some("i1".into());
        state.instances = vec![instance("i1", "World")];
        state.results = vec![project("MOD0001"), project("MOD0002")];
        state.busy_projects.insert("MOD0001".into());
        state.busy_instances.insert("i1".into());
        assert!(state.busy_project("MOD0001"));
        assert!(state.target_busy());
        assert!(
            state.begin_quick_install("MOD0001").is_none(),
            "same project"
        );
        assert!(
            state.begin_quick_install("MOD0002").is_some(),
            "other project"
        );
        assert!(state.begin_search(false, String::new()).is_some());
        // Reviewing is never blocked by queue work (the queue, not review,
        // serializes per instance) — for the same or an unrelated project.
        assert!(state.begin_review().is_some());
        // Browsing other projects continues too; it resets per-project state.
        assert!(state.begin_details("MOD0002".into()).is_some());
    }

    #[test]
    fn settlement_refreshes_registry_inventory_and_notifies_the_parent() {
        // Success, failure and running cancellation all settle through the
        // queue; each drives this view's refresh with its existing guards.
        assert_eq!(
            DiscoverState::settlement_effects(),
            Effects {
                notify: true,
                changed: true,
                reload_instances: true,
                reload_installed: true,
                search_now: false,
                artwork: false,
            }
        );
    }

    fn art() -> ArtworkOutcome {
        ArtworkOutcome::Image(RawArtwork {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 255],
        })
    }

    /// A modpack view (no target needed) showing the named results.
    fn showing(ids: &[&str]) -> DiscoverState {
        let mut state = DiscoverState::default();
        state.kind = 3;
        let request = state.begin_search(false, String::new()).unwrap();
        let fx = state.receive(Message::Search {
            serial: request.serial,
            result: Ok(page(ids)),
        });
        assert!(fx.artwork, "new results ask for their artwork");
        state
    }

    #[test]
    fn artwork_is_requested_once_per_project_in_screen_order() {
        let mut state = showing(&["b", "a", "c"]);
        let (epoch, ids) = state.begin_artwork();
        assert_eq!(ids, ["b", "a", "c"]);
        assert_eq!(epoch, state.artwork.epoch);
        // In-flight projects are not requested again.
        assert!(state.begin_artwork().1.is_empty());
        assert!(state.receive_artwork(epoch, "b".into(), art()));
        assert!(state.artwork.cache.image(&"b".to_owned()).is_some());
        assert!(!state.artwork.pending.contains("b"));
        assert!(
            state.begin_artwork().1.is_empty(),
            "retained, so never refetched"
        );
    }

    #[test]
    fn artwork_for_a_replaced_result_list_is_dropped() {
        let mut state = showing(&["old"]);
        let (stale, ids) = state.begin_artwork();
        assert_eq!(ids, ["old"]);
        // A new search replaces the list and its epoch before the reply lands.
        let request = state.begin_search(false, "q".into()).unwrap();
        assert!(!state.artwork.pending.contains("old"));
        assert_eq!(
            state.artwork.live.load(Ordering::Acquire),
            state.artwork.epoch
        );
        state.receive(Message::Search {
            serial: request.serial,
            result: Ok(page(&["old", "new"])),
        });
        let fx = state.receive(Message::Artwork {
            epoch: stale,
            project: "old".into(),
            outcome: art(),
        });
        assert!(!fx.notify, "a superseded reply changes nothing");
        assert!(!state.artwork.cache.contains(&"old".to_owned()));
        // The same project is requested again under the current epoch.
        let (epoch, ids) = state.begin_artwork();
        assert_eq!(ids, ["old", "new"]);
        assert!(state.receive_artwork(epoch, "old".into(), art()));
    }

    #[test]
    fn artwork_for_a_project_no_longer_shown_is_not_retained() {
        let mut state = showing(&["a"]);
        let (epoch, _) = state.begin_artwork();
        assert!(!state.receive_artwork(epoch, "elsewhere".into(), art()));
        assert!(!state.artwork.cache.contains(&"elsewhere".to_owned()));
    }

    #[test]
    fn absent_artwork_is_remembered_but_failures_are_retried() {
        let mut state = showing(&["none", "flaky"]);
        let (epoch, _) = state.begin_artwork();
        assert!(state.receive_artwork(epoch, "none".into(), ArtworkOutcome::Absent));
        assert!(!state.receive_artwork(epoch, "flaky".into(), ArtworkOutcome::Failed));
        assert!(state.artwork.cache.image(&"none".to_owned()).is_none());
        // Only the transient failure is asked for again.
        assert_eq!(state.begin_artwork().1, ["flaky"]);
        // Malformed pixels read as absent rather than as an image.
        let bad = ArtworkOutcome::Image(RawArtwork {
            width: 2,
            height: 2,
            rgba: vec![0; 3],
        });
        assert!(state.receive_artwork(epoch, "flaky".into(), bad));
        assert!(state.artwork.cache.image(&"flaky".to_owned()).is_none());
        assert!(state.begin_artwork().1.is_empty());
    }

    #[test]
    fn artwork_cache_is_capped_and_keeps_projects_on_screen() {
        let ids: Vec<String> = (0..MAX_ICONS + 20).map(|i| format!("p{i}")).collect();
        let mut state = showing(&[]);
        state.results = ids.iter().map(|id| project(id)).collect();
        // The request budget never exceeds the cache size.
        let (epoch, wanted) = state.begin_artwork();
        assert_eq!(wanted.len(), MAX_ICONS);
        for id in &wanted {
            assert!(state.receive_artwork(epoch, id.clone(), art()));
        }
        assert_eq!(state.artwork.cache.len(), MAX_ICONS);
        // The remaining cards load as budget frees up; the cap still holds and
        // the newest projects, which are on screen, are retained.
        let (_, rest) = state.begin_artwork();
        assert_eq!(rest.len(), 20);
        for id in &rest {
            assert!(state.receive_artwork(epoch, id.clone(), art()));
        }
        assert_eq!(state.artwork.cache.len(), MAX_ICONS);
        for id in &rest {
            assert!(state.artwork.cache.contains(id));
        }
    }

    #[test]
    fn details_artwork_is_requested_for_the_open_project_only_once() {
        let mut state = state_with_pack_details();
        let id = state.details.as_ref().unwrap().project_id.clone();
        let (epoch, ids) = state.begin_artwork();
        assert_eq!(ids, [id.clone()]);
        assert!(state.receive_artwork(epoch, id.clone(), art()));
        assert!(state.artwork.cache.image(&id).is_some());
        assert!(state.begin_artwork().1.is_empty());
    }

    #[test]
    fn a_diagnostic_view_requests_no_artwork() {
        let mut state = DiscoverState {
            diagnostic: true,
            ..Default::default()
        };
        state.results = vec![project("a")];
        assert!(state.begin_artwork().1.is_empty());
    }

    #[test]
    fn a_consumed_review_request_is_never_reused_or_altered() {
        // The confirm path takes the frozen snapshot once; the queued job
        // carries its own clone of the exact fingerprinted request.
        let mut state = reviewed(false);
        let first = state.confirm_install().unwrap();
        let request = match first.request {
            JobRequest::Pack { request, .. } => request,
            _ => panic!("pack"),
        };
        assert_eq!(request["fingerprint"], json!("new-fp"));
        // The review is gone: a second confirm cannot rebuild or mutate it.
        assert!(state.confirm_install().is_none());
        assert!(state.review.is_none());
    }

    // ------------------------------------------------------ diagnostic

    #[test]
    fn a_diagnostic_view_begins_nothing() {
        let mut state = DiscoverState {
            diagnostic: true,
            kind: 3,
            ..Default::default()
        };
        assert!(state.begin_search(false, String::new()).is_none());
        assert!(state.begin_details("P".into()).is_none());
        assert!(state.begin_destinations().is_none());
        assert!(state.begin_review().is_none());
        assert!(state.begin_quick_install("P").is_none());
        assert!(!state.can_review());
        // Even an applied instance reload never triggers the first search.
        let serial = state.begin_instances();
        let fx = state.receive(Message::Instances(
            serial,
            Ok(api::LauncherState {
                config: api::ConfigSummary {
                    selected_instance_id: None,
                },
                instances: vec![],
                platform_capabilities: vec![],
                icons: Default::default(),
            }),
        ));
        assert!(fx.notify);
        assert!(!fx.search_now, "diagnostic never auto-searches");
    }

    // ---------------------------------------------------- instances/target

    #[test]
    fn instances_reload_keeps_a_valid_target_or_falls_back_to_selection() {
        let mut state = DiscoverState::default();
        let launcher = |ids: &[&str], selected: Option<&str>| api::LauncherState {
            config: api::ConfigSummary {
                selected_instance_id: selected.map(str::to_owned),
            },
            instances: ids.iter().map(|i| instance(i, i)).collect(),
            platform_capabilities: vec![],
            icons: Default::default(),
        };
        state.apply_instances(launcher(&["a", "b"], Some("b")));
        assert_eq!(state.target.as_deref(), Some("b"));
        state.target = Some("a".into());
        state.apply_instances(launcher(&["b"], Some("b")));
        assert_eq!(
            state.target.as_deref(),
            Some("b"),
            "dangling target dropped"
        );
    }

    fn launcher_with(ids: &[&str]) -> api::LauncherState {
        api::LauncherState {
            config: api::ConfigSummary {
                selected_instance_id: None,
            },
            instances: ids.iter().map(|i| instance(i, i)).collect(),
            platform_capabilities: vec![],
            icons: Default::default(),
        }
    }

    #[test]
    fn open_for_sets_origin_target_and_kind_and_searches_that_instance() {
        let mut state = DiscoverState::default();
        state.apply_instances(launcher_with(&["a", "b"]));
        state.target = Some("a".into());
        assert!(state.begin_open_for("b".into(), 2));
        assert_eq!(state.origin, Some(("b".into(), 2)));
        assert_eq!(state.target.as_deref(), Some("b"));
        assert_eq!(state.kind_name(), "shaderPack");
        let request = state.begin_search(false, String::new()).unwrap();
        assert_eq!((request.target.as_str(), request.kind), ("b", "shaderPack"));
        // Content tabs only reach 0..=2; anything else clamps like Slint.
        assert!(state.begin_open_for("b".into(), 9));
        assert_eq!(state.origin, Some(("b".into(), 3)));
        assert!(state.begin_open_for("b".into(), -1));
        assert_eq!(state.kind_name(), "mod");
    }

    #[test]
    fn open_for_resets_the_per_target_view_but_keeps_an_unchanged_target() {
        let mut state = DiscoverState::default();
        state.apply_instances(launcher_with(&["a", "b"]));
        state.target = Some("a".into());
        state.installed.insert("fabric-api".into());
        state.category_index = 2;
        assert!(state.begin_open_for("a".into(), 0));
        assert!(
            state.installed.contains("fabric-api"),
            "same target keeps its inventory markers"
        );
        assert_eq!(state.category_index, 0, "categories reset for the kind");
        assert!(state.begin_open_for("b".into(), 0));
        assert!(
            state.installed.is_empty(),
            "another target's markers never carry over"
        );
    }

    #[test]
    fn a_search_from_before_open_for_is_dropped() {
        let mut state = DiscoverState::default();
        state.apply_instances(launcher_with(&["a", "b"]));
        state.target = Some("a".into());
        state.kind = 0;
        let before = state.begin_search(false, "old".into()).unwrap();
        assert!(state.begin_open_for("b".into(), 1));
        let after = state.begin_search(false, String::new()).unwrap();
        let fx = state.receive(Message::Search {
            serial: before.serial,
            result: Ok(page(&["stale"])),
        });
        assert!(!fx.notify);
        assert!(state.results.is_empty());
        assert!(state.searching, "the new search is still the live one");
        state.receive(Message::Search {
            serial: after.serial,
            result: Ok(page(&["fresh"])),
        });
        assert_eq!(state.results[0].project_id, "fresh");
    }

    #[test]
    fn a_diagnostic_view_ignores_open_for() {
        let mut state = DiscoverState {
            diagnostic: true,
            ..Default::default()
        };
        assert!(!state.begin_open_for("a".into(), 0));
        assert_eq!(state.origin, None);
        assert_eq!(state.target, None);
    }

    #[test]
    fn returning_is_offered_only_while_the_origin_instance_exists() {
        let mut state = DiscoverState::default();
        assert_eq!(
            state.return_request(),
            None,
            "never opened from an instance"
        );
        state.apply_instances(launcher_with(&["a", "b"]));
        state.begin_open_for("a".into(), 1);
        assert_eq!(state.return_request(), Some(("a".into(), 1)));
        // Browsing elsewhere keeps the origin, as in Slint.
        state.target = Some("b".into());
        state.kind = 0;
        assert_eq!(state.return_request(), Some(("a".into(), 1)));
        state.apply_instances(launcher_with(&["b"]));
        assert_eq!(state.return_request(), None, "origin instance was removed");
        state.apply_instances(launcher_with(&["a", "b"]));
        assert_eq!(state.return_request(), Some(("a".into(), 1)));
    }

    #[test]
    fn an_explicit_destination_wins_over_the_origin() {
        let mut rows = destinations();
        rows[2].compatible = true;
        rows[2].reasons.clear();
        let mut state = DiscoverState {
            origin: Some(("good0001".into(), 0)),
            destination: Some("old00001".into()),
            ..Default::default()
        };
        state.apply_destinations(rows.clone());
        assert_eq!(state.destination.as_deref(), Some("old00001"));
        // With no choice yet the compatible origin is preselected.
        state.destination = None;
        state.apply_destinations(rows);
        assert_eq!(state.destination.as_deref(), Some("good0001"));
    }

    #[test]
    fn content_search_without_a_target_is_an_error_not_a_request() {
        let mut state = DiscoverState::default();
        state.kind = 0;
        assert!(state.begin_search(false, "query".into()).is_none());
        assert_eq!(
            state.error.as_deref(),
            Some("Choose a target instance to browse compatible content.")
        );
        assert!(!state.searching);
        state.target = Some("a".into());
        let request = state.begin_search(false, "query".into()).unwrap();
        assert_eq!(request.target, "a");
        assert_eq!(request.kind, "mod");
        assert_eq!(request.sort, "relevance");
        // Packs browse without a target but carry the loader filter.
        let mut state = DiscoverState {
            kind: 3,
            loader_index: 1,
            ..Default::default()
        };
        let request = state.begin_search(false, String::new()).unwrap();
        assert_eq!(request.target, "");
        assert_eq!(request.kind, "modpack");
        assert_eq!(request.categories, vec!["fabric".to_owned()]);
    }
}
