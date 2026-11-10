//! Modrinth presentation over the existing provider and reviewed pack engines.
//! Requests freeze target, category and exact version before leaving the UI thread.
use crate::api::{self, PackDestination, ProjectDetails, ProjectSummary, VersionChoice};
use crate::ui;
use crate::{AppWindow, Discover, Nav, PackDestinationRow, Page, ProjectRow, ReviewFact, Tone};
use aurora_launcher_lib::content_jobs::Control;
use serde_json::{Value, json};
use slint::ComponentHandle;
use std::collections::{HashMap, HashSet};

pub const KINDS: [&str; 4] = ["mod", "resourcePack", "shaderPack", "modpack"];
const MAX_RESULTS: usize = 200;
const MAX_ICONS: usize = 128;

#[derive(Clone)]
pub struct Review {
    target: String,
    kind: &'static str,
    project: String,
    version: String,
    merge: bool,
    request: Value,
    fingerprint: String,
}

#[derive(Default)]
pub struct DiscoverState {
    pub categories: Vec<api::ProviderCategory>,
    pub category_values: Vec<String>,
    pub categories_requested: bool,
    pub target: Option<String>,
    pub results: Vec<ProjectSummary>,
    pub installed: HashSet<String>,
    pub total: u32,
    pub serial: u64,
    pub next_offset: u32,
    pub icons: HashMap<String, slint::Image>,
    pub icon_pending: HashSet<String>,
    pub details: Option<ProjectDetails>,
    pub detail_serial: u64,
    pub review: Option<Review>,
    pub optional_files: Vec<String>,
    pub selected_optional: HashSet<String>,
    pub origin: Option<(String, i32)>,
    /// A recognized local `.mrpack`: (exact version id, banner text). Taken
    /// by the next details load so it never leaks into later browsing.
    pub local_pack: Option<(String, String)>,
    /// Every instance as the core annotated it for the selected pack version.
    pub destinations: Vec<PackDestination>,
    /// The (project, version) `destinations` belongs to, once requested.
    pub destinations_for: Option<(String, String)>,
    pub destinations_loading: bool,
    pub destinations_error: Option<String>,
    pub destination_serial: u64,
    /// The chosen existing instance; only a compatible one is ever kept.
    pub destination: Option<String>,
    /// The in-flight review's own content job, so Cancel can stop it.
    pub review_job: Option<Control>,
    pub review_timer: Option<slint::Timer>,
}

pub fn bind(window: &AppWindow) {
    let d = window.global::<Discover>();
    d.on_search(|| search(false));
    d.on_more(|| search(true));
    d.on_kind_changed(|_| {
        render_categories();
        search(false);
    });
    d.on_filters_changed(|| search(false));
    d.on_instance_changed(|index| {
        let id = ui::state(|s| s.instances().get(index as usize).map(|i| i.id.clone()));
        ui::with(|_, state| state.discover.target = id);
        invalidate_review();
        search(false);
    });
    d.on_install(install);
    d.on_details(details);
    d.on_version_changed(version_changed);
    d.on_destination_mode_changed(destination_mode_changed);
    d.on_choose_destination(choose_destination);
    d.on_review(review);
    d.on_cancel_review(|| {
        ui::with(|w, s| cancel_review(w, &mut s.discover));
    });
    d.on_toggle_optional(|path| {
        ui::with(|w, s| toggle_optional(w, &mut s.discover, path.as_str()));
    });
    d.on_confirm_install(confirm_install);
    d.on_close_details(|| {
        ui::with(|w, s| {
            if w.global::<Discover>().get_install_busy() {
                return;
            }
            stop_review(w, &mut s.discover);
            s.discover.detail_serial += 1;
            s.discover.destination_serial += 1;
            s.discover.review = None;
            s.discover.local_pack = None;
            let d = w.global::<Discover>();
            d.set_details_loading(false);
            d.set_details_open(false);
            d.set_local_pack_note("".into());
        });
    });
    d.on_return_to_instance(|| {
        if let Some((id, tab)) = ui::state(|s| s.discover.origin.clone()) {
            crate::instances::open_workspace(id, tab);
        }
    });
}

pub fn open_for(instance_id: String, kind: i32) {
    load_categories();
    ui::with(|window, state| {
        // Content tabs share the content kind index, so the origin tab is the kind.
        state.discover.origin = Some((instance_id.clone(), kind));
        state.discover.target = Some(instance_id);
        window.global::<Discover>().set_kind(kind.clamp(0, 3));
        window.global::<Nav>().set_page(Page::Discover);
    });
    render_categories();
    render_targets();
    search(false);
}

/// Opens the reviewed modpack workflow for a local archive that Modrinth
/// recognized as `project`/`version`. Review, optional files, new-instance or
/// merge choice and the queued install are the hosted workflow unchanged.
pub fn open_local_pack(project: String, version: String, note: String) {
    ui::with(|window, state| {
        state.discover.local_pack = Some((version, note));
        window.global::<Discover>().set_kind(3);
        window.global::<Nav>().set_page(Page::Discover);
    });
    render_targets();
    details(project.into());
}

fn load_categories() {
    let load = ui::with(|_, state| {
        let load = !state.discover.categories_requested;
        state.discover.categories_requested = true;
        load
    })
    .unwrap_or(false);
    if load {
        ui::spawn(api::browse_categories(), |result| match result {
            Ok(categories) => {
                ui::with(|_, s| s.discover.categories = categories);
                render_categories();
            }
            Err(error) => {
                ui::with(|_, s| s.discover.categories_requested = false);
                ui::error_toast(&error);
            }
        });
    }
}

pub fn on_shown() {
    load_categories();
    render_targets();
    if ui::state(|s| s.discover.serial == 0) {
        search(false);
    }
}

pub fn render_targets() {
    ui::with(|window, state| {
        let d = window.global::<Discover>();
        let names = state
            .instances()
            .iter()
            .map(|i| i.display_name.clone().into())
            .collect::<Vec<_>>();
        d.set_has_instances(!names.is_empty());
        // The install target is an explicit UI choice, independent of Home selection.
        let target = state
            .discover
            .target
            .clone()
            .filter(|id| state.instance(id).is_some())
            .or_else(|| state.selected_instance().map(|i| i.id.clone()));
        let index = target
            .as_ref()
            .and_then(|id| state.instances().iter().position(|i| &i.id == id));
        state.discover.target = target;
        d.set_instance_names(ui::model(names));
        d.set_instance_index(index.map(|i| i as i32).unwrap_or(-1));
        d.set_can_return(
            state
                .discover
                .origin
                .as_ref()
                .is_some_and(|(id, _)| state.instance(id).is_some()),
        );
    });
}

fn render_categories() {
    ui::with(|w, s| {
        let d = w.global::<Discover>();
        let kind = ["mod", "resourcepack", "shader", "modpack"][d.get_kind().clamp(0, 3) as usize];
        let mut values: Vec<String> = s
            .discover
            .categories
            .iter()
            .filter(|c| c.project_type == kind)
            .map(|c| c.name.clone())
            .collect();
        values.sort();
        values.dedup();
        let mut names = vec!["All categories".into()];
        names.extend(values.iter().map(|v| v.replace('-', " ").into()));
        s.discover.category_values = values;
        d.set_category_index(0);
        d.set_category_names(ui::model(names));
    });
}

fn search(append: bool) {
    let Some((target, kind, query, offset, serial, categories, sort)) =
        ui::with(|window, state| {
            let d = window.global::<Discover>();
            if append && (d.get_loading() || state.discover.results.len() >= MAX_RESULTS) {
                return None;
            }
            let kind = KINDS[d.get_kind().clamp(0, 3) as usize];
            state.discover.serial += 1;
            if !append {
                state.discover.results.clear();
                state.discover.next_offset = 0;
                state.discover.total = 0;
                state.discover.installed.clear();
            }
            d.set_error("".into());
            let target = if kind == "modpack" {
                String::new()
            } else {
                state.discover.target.clone().unwrap_or_default()
            };
            if kind != "modpack" && target.is_empty() {
                d.set_loading(false);
                d.set_error("Choose a target instance to browse compatible content.".into());
                return None;
            }
            d.set_loading(true);
            Some((
                target,
                kind,
                d.get_query().to_string(),
                state.discover.next_offset,
                state.discover.serial,
                state
                    .discover
                    .category_values
                    .get(d.get_category_index().wrapping_sub(1) as usize)
                    .cloned()
                    .into_iter()
                    .chain(
                        (kind == "modpack")
                            .then(|| {
                                ["", "fabric", "forge", "neoforge"]
                                    [d.get_loader_index().clamp(0, 3) as usize]
                                    .to_owned()
                            })
                            .filter(|v| !v.is_empty()),
                    )
                    .collect(),
                ["relevance", "downloads", "follows", "newest", "updated"]
                    [d.get_sort_index().clamp(0, 4) as usize]
                    .to_owned(),
            ))
        })
        .flatten()
    else {
        render();
        return;
    };
    render();
    load_installed();
    ui::spawn(
        api::browse_filtered(ui::host(), target, kind, query, offset, categories, sort),
        move |result| {
            let current = ui::with(|window, state| {
                if state.discover.serial != serial {
                    return false;
                }
                let d = window.global::<Discover>();
                d.set_loading(false);
                match result {
                    Ok(page) => {
                        state.discover.next_offset = page.offset.saturating_add(20);
                        state.discover.total = page.total_hits;
                        append_unique(&mut state.discover.results, page.hits);
                    }
                    Err(error) => d.set_error(format!("{} ({})", error.message, error.code).into()),
                }
                true
            })
            .unwrap_or(false);
            if current {
                render();
                load_icons();
            }
        },
    );
}

fn append_unique(results: &mut Vec<ProjectSummary>, hits: Vec<ProjectSummary>) {
    let mut known: HashSet<String> = results.iter().map(|p| p.project_id.clone()).collect();
    results.extend(
        hits.into_iter()
            .filter(|p| known.insert(p.project_id.clone()))
            .take(MAX_RESULTS.saturating_sub(results.len())),
    );
}

fn load_installed() {
    let Some((id, serial, kind)) = ui::with(|w, s| {
        Some((
            s.discover.target.clone()?,
            s.discover.serial,
            KINDS[w.global::<Discover>().get_kind().clamp(0, 3) as usize],
        ))
    })
    .flatten() else {
        return;
    };
    ui::spawn(api::provider_inventory(ui::host(), id), move |result| {
        ui::with(|_, s| {
            if s.discover.serial != serial {
                return;
            }
            if let Ok(rows) = result {
                s.discover.installed = rows
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|row| row["record"]["contentType"] == kind)
                    .filter_map(|row| row["record"]["projectId"].as_str().map(str::to_owned))
                    .collect();
            }
        });
        render();
    });
}

fn load_icons() {
    let ids = ui::with(|_, s| {
        let ids = s
            .discover
            .results
            .iter()
            .filter(|p| {
                !s.discover.icons.contains_key(&p.project_id)
                    && !s.discover.icon_pending.contains(&p.project_id)
            })
            .map(|p| p.project_id.clone())
            .take(MAX_ICONS.saturating_sub(s.discover.icon_pending.len()))
            .collect::<Vec<_>>();
        s.discover.icon_pending.extend(ids.iter().cloned());
        ids
    })
    .unwrap_or_default();
    static GATE: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    let gate = GATE
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(4)))
        .clone();
    for id in ids {
        let gate = gate.clone();
        let host = ui::host();
        let key = id.clone();
        ui::spawn(
            async move {
                let _permit = gate.acquire_owned().await.expect("image gate");
                api::project_artwork(host, id).await
            },
            move |result| {
                let image = result
                    .ok()
                    .flatten()
                    .and_then(|bytes| slint::Image::load_from_data(&bytes, Some("png")).ok());
                ui::with(|_, s| {
                    s.discover.icon_pending.remove(&key);
                    if s.discover.results.iter().any(|p| p.project_id == key) {
                        if let Some(image) = image {
                            if s.discover.icons.len() >= MAX_ICONS {
                                let keep: HashSet<_> = s
                                    .discover
                                    .results
                                    .iter()
                                    .map(|p| p.project_id.clone())
                                    .collect();
                                s.discover.icons.retain(|id, _| keep.contains(id));
                            }
                            if s.discover.icons.len() < MAX_ICONS {
                                s.discover.icons.insert(key, image);
                            }
                        }
                    }
                });
                render();
            },
        );
    }
}

/// Discards the current review (and stops one still being prepared). Any
/// change to the version, destination or optional files lands here, so the
/// reviewed fingerprint can never outlive the choices it was made for.
fn invalidate_review() {
    ui::with(|w, s| {
        stop_review(w, &mut s.discover);
        s.discover.detail_serial += 1;
        s.discover.review = None;
        s.discover.optional_files.clear();
        s.discover.selected_optional.clear();
        render_optional(w, &s.discover);
        clear_review(w);
        let d = w.global::<Discover>();
        d.set_details_loading(false);
        d.set_detail_error("".into());
        render_selection(w, &s.discover);
    });
}

fn clear_review(w: &AppWindow) {
    let d = w.global::<Discover>();
    d.set_can_install(false);
    d.set_review_text("".into());
    d.set_review_facts(ui::model(Vec::new()));
}

/// Cancels the in-flight review's content job (its download stops at the
/// next streamed chunk and its staging file is removed) and its progress.
fn stop_review(w: &AppWindow, state: &mut DiscoverState) {
    if let Some(job) = state.review_job.take() {
        job.cancel();
    }
    state.review_timer = None;
    let d = w.global::<Discover>();
    d.set_review_cancellable(false);
    d.set_review_progress("".into());
    d.set_review_fraction(-1.);
}

fn cancel_review(w: &AppWindow, state: &mut DiscoverState) {
    if state.review_job.is_none() {
        return;
    }
    stop_review(w, state);
    state.detail_serial += 1;
    state.review = None;
    clear_review(w);
    let d = w.global::<Discover>();
    d.set_details_loading(false);
    d.set_review_text("Review cancelled. Nothing was installed.".into());
    render_selection(w, state);
}

fn toggle_optional(w: &AppWindow, state: &mut DiscoverState, path: &str) {
    if !state.optional_files.iter().any(|p| p == path) {
        return;
    }
    if !state.selected_optional.remove(path) {
        state.selected_optional.insert(path.to_owned());
    }
    state.detail_serial += 1;
    state.review = None;
    clear_review(w);
    w.global::<Discover>().set_review_text(
        "Optional selection changed. Review installation again before installing.".into(),
    );
    render_optional(w, state);
}

fn version_changed() {
    invalidate_review();
    ui::with(|w, s| {
        s.discover.destination_serial += 1;
        s.discover.destinations.clear();
        s.discover.destinations_for = None;
        s.discover.destinations_loading = false;
        s.discover.destinations_error = None;
        render_selection(w, &s.discover);
    });
    load_destinations();
}

fn destination_mode_changed() {
    invalidate_review();
    load_destinations();
}

fn choose_destination(index: i32) {
    let chosen = ui::state(|s| {
        usize::try_from(index)
            .ok()
            .and_then(|i| s.discover.destinations.get(i))
            .filter(|row| row.compatible)
            .map(|row| row.instance_id.clone())
    });
    if chosen.is_some() {
        invalidate_review();
    }
    ui::with(|w, s| {
        if chosen.is_some() {
            s.discover.destination = chosen;
        }
        // Re-rendering also restores a row an incompatible click toggled.
        render_selection(w, &s.discover);
    });
}

/// Asks the core which instances can take the selected pack version.
fn load_destinations() {
    let Some((project, version, serial)) = ui::with(|w, s| {
        let d = w.global::<Discover>();
        if d.get_kind() != 3 || d.get_pack_mode() != 1 {
            return None;
        }
        let p = s.discover.details.as_ref()?;
        let v = selected_version(w, &s.discover)?;
        if v.unsupported_reason.is_some() {
            return None;
        }
        let key = (p.project_id.clone(), v.id.clone());
        if s.discover.destinations_for.as_ref() == Some(&key) {
            return None;
        }
        s.discover.destination_serial += 1;
        s.discover.destinations_for = Some(key.clone());
        s.discover.destinations.clear();
        s.discover.destinations_loading = true;
        s.discover.destinations_error = None;
        render_selection(w, &s.discover);
        Some((key.0, key.1, s.discover.destination_serial))
    })
    .flatten() else {
        return;
    };
    ui::spawn(
        api::pack_destinations(ui::host(), project, version),
        move |result| {
            ui::with(|w, s| {
                if s.discover.destination_serial != serial {
                    return;
                }
                s.discover.destinations_loading = false;
                match result {
                    Ok(rows) => apply_destinations(&mut s.discover, rows),
                    Err(e) => {
                        s.discover.destinations_for = None;
                        s.discover.destinations_error = Some(format!("{} ({})", e.message, e.code));
                    }
                }
                render_selection(w, &s.discover);
            });
        },
    );
}

/// Keeps the chosen instance only while the core still calls it compatible;
/// otherwise preselects the instance Discover was opened from, if it is.
fn apply_destinations(state: &mut DiscoverState, rows: Vec<PackDestination>) {
    state.destinations = rows;
    let compatible = |id: &String| {
        state
            .destinations
            .iter()
            .any(|d| &d.instance_id == id && d.compatible)
    };
    state.destination = state.destination.clone().filter(compatible).or_else(|| {
        state
            .origin
            .as_ref()
            .map(|(id, _)| id.clone())
            .filter(compatible)
    });
}

fn selected_version<'a>(w: &AppWindow, state: &'a DiscoverState) -> Option<&'a VersionChoice> {
    let index = usize::try_from(w.global::<Discover>().get_version_index()).ok()?;
    state.details.as_ref()?.versions.get(index)
}

fn chosen_destination(state: &DiscoverState) -> Option<&PackDestination> {
    let id = state.destination.as_ref()?;
    state
        .destinations
        .iter()
        .find(|d| &d.instance_id == id && d.compatible)
}

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

/// `values` joined, the first `shown` of them, then "+N more".
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

/// One dropdown row: "Pack 6.4.0 · Release · Minecraft 1.21.1 · Fabric",
/// marked when the core says Aurora can't install it.
pub(crate) fn version_label(v: &VersionChoice, pack: bool) -> String {
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
fn version_detail(v: &VersionChoice) -> String {
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

/// Project-level metadata, labelled as what the publisher lists rather than
/// what any one version installs.
fn publisher_lists(p: &ProjectDetails, pack: bool) -> String {
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

fn destination_detail(d: &PackDestination) -> String {
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

fn destinations_note(state: &DiscoverState) -> String {
    if state.destinations_loading {
        return "Checking which instances can take this version…".into();
    }
    if let Some(error) = &state.destinations_error {
        return error.clone();
    }
    if state.destinations_for.is_none() {
        return String::new();
    }
    if state.destinations.is_empty() {
        return "You have no instances yet. Choose New instance instead.".into();
    }
    if !state.destinations.iter().any(|d| d.compatible) {
        return "None of your instances can take this version. Choose New instance, or another version.".into();
    }
    "Only compatible instances can be chosen. Review checks the exact loader version too.".into()
}

/// Presents the version and destination choices from Rust state. The core
/// has decided installability and compatibility; this only displays them.
pub(crate) fn render_selection(w: &AppWindow, state: &DiscoverState) {
    let d = w.global::<Discover>();
    let pack = d.get_kind() == 3;
    let merge = pack && d.get_pack_mode() == 1;
    let version = selected_version(w, state);
    let versions = state
        .details
        .as_ref()
        .map(|p| p.versions.as_slice())
        .unwrap_or(&[]);
    let (detail, problem) = match version {
        Some(v) if pack => (
            version_detail(v),
            v.unsupported_reason.clone().unwrap_or_default(),
        ),
        None if pack && !versions.is_empty() => (
            String::new(),
            if versions.iter().all(|v| v.unsupported_reason.is_some()) {
                "Aurora can't install any version of this pack. Each version says why.".into()
            } else {
                "Choose a pack version.".into()
            },
        ),
        _ => (String::new(), String::new()),
    };
    let installable = version.is_some_and(|v| !pack || v.unsupported_reason.is_none());
    d.set_version_detail(detail.into());
    d.set_version_problem(problem.into());
    d.set_version_installable(installable);
    d.set_destinations(ui::model(
        state
            .destinations
            .iter()
            .map(|row| PackDestinationRow {
                name: row.display_name.clone().into(),
                detail: destination_detail(row).into(),
                compatible: row.compatible,
                selected: row.compatible && state.destination.as_ref() == Some(&row.instance_id),
            })
            .collect(),
    ));
    d.set_destinations_note(destinations_note(state).into());
    d.set_can_review(installable && (!merge || chosen_destination(state).is_some()));
    d.set_confirm_label(
        match (pack, merge) {
            (true, false) => "Create instance",
            (true, true) => "Add to instance",
            _ => "Install reviewed content",
        }
        .into(),
    );
}

fn details(project: slint::SharedString) {
    let Some((target, kind, serial, preferred)) = ui::with(|w, s| {
        let d = w.global::<Discover>();
        if d.get_install_busy() {
            return None;
        }
        let local = s.discover.local_pack.take();
        d.set_local_pack_note(
            local
                .as_ref()
                .map(|(_, note)| note.clone())
                .unwrap_or_default()
                .into(),
        );
        let preferred = local.map(|(version, _)| version);
        stop_review(w, &mut s.discover);
        s.discover.detail_serial += 1;
        s.discover.destination_serial += 1;
        s.discover.details = None;
        s.discover.review = None;
        s.discover.optional_files.clear();
        s.discover.selected_optional.clear();
        s.discover.destinations.clear();
        s.discover.destinations_for = None;
        s.discover.destinations_loading = false;
        s.discover.destinations_error = None;
        s.discover.destination = None;
        render_optional(w, &s.discover);
        clear_review(w);
        d.set_details_open(true);
        d.set_details_loading(true);
        d.set_review_progress("Loading project details…".into());
        d.set_detail_error("".into());
        d.set_project_title("Project details".into());
        d.set_project_summary("".into());
        d.set_compatibility("".into());
        d.set_versions(ui::model(Vec::new()));
        d.set_version_index(-1);
        d.set_pack_mode(0);
        render_selection(w, &s.discover);
        let kind = KINDS[d.get_kind().clamp(0, 3) as usize];
        Some((
            if kind == "modpack" {
                String::new()
            } else {
                s.discover.target.clone().unwrap_or_default()
            },
            kind,
            s.discover.detail_serial,
            preferred,
        ))
    })
    .flatten() else {
        return;
    };
    ui::spawn(
        api::project(ui::host(), target, kind, project.to_string()),
        move |result| {
            ui::with(|w, s| {
                if s.discover.detail_serial != serial {
                    return;
                }
                let d = w.global::<Discover>();
                d.set_details_loading(false);
                d.set_review_progress("".into());
                match result {
                    Ok(p) => show_details(w, &mut s.discover, p, kind, preferred.as_deref()),
                    Err(e) => d.set_detail_error(format!("{} ({})", e.message, e.code).into()),
                }
            });
        },
    );
}

/// Presents loaded project details: labelled publisher metadata, one
/// annotated row per version, and the core's default version.
pub(crate) fn show_details(
    w: &AppWindow,
    state: &mut DiscoverState,
    p: ProjectDetails,
    kind: &str,
    preferred: Option<&str>,
) {
    let d = w.global::<Discover>();
    let pack = kind == "modpack";
    d.set_project_title(p.title.clone().into());
    d.set_project_summary(p.summary.clone().into());
    d.set_compatibility(publisher_lists(&p, pack).into());
    d.set_versions(ui::model(
        p.versions
            .iter()
            .map(|v| version_label(v, pack).into())
            .collect(),
    ));
    let index = match preferred {
        // A local archive installs exactly its own version, never a substitute.
        Some(version) => p.versions.iter().position(|v| v.id == version),
        None => p
            .default_version_id
            .as_ref()
            .and_then(|id| p.versions.iter().position(|v| &v.id == id))
            .or_else(|| (!pack && !p.versions.is_empty()).then_some(0)),
    };
    d.set_version_index(index.map(|i| i as i32).unwrap_or(-1));
    if p.versions.is_empty() {
        d.set_detail_error("This project has no published versions Aurora can show.".into());
    } else if preferred.is_some() && index.is_none() {
        d.set_detail_error(
            "This archive's exact pack version is no longer listed by Modrinth.".into(),
        );
    }
    state.details = Some(p);
    render_selection(w, state);
}

fn render_optional(window: &AppWindow, state: &DiscoverState) {
    window.global::<Discover>().set_optional_files(ui::model(
        state
            .optional_files
            .iter()
            .map(|path| crate::OptionalPackFile {
                path: path.clone().into(),
                selected: state.selected_optional.contains(path),
            })
            .collect(),
    ));
}

/// Review preparation status from the review job's streamed bytes and the
/// archive size Modrinth publishes. A fraction only exists while the
/// archive is actually streaming toward a known size.
pub(crate) fn review_progress(pack: bool, streamed: u64, archive: Option<u64>) -> (String, f32) {
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

fn show_review_progress(w: &AppWindow, pack: bool, streamed: u64, archive: Option<u64>) {
    let (text, fraction) = review_progress(pack, streamed, archive);
    let d = w.global::<Discover>();
    d.set_review_progress(text.into());
    d.set_review_fraction(fraction);
}

fn review() {
    let Some((snapshot, serial, control)) = ui::with(|w, s| {
        let d = w.global::<Discover>();
        if d.get_install_busy() || d.get_details_loading() {
            return None;
        }
        let p = s.discover.details.as_ref()?;
        let v = selected_version(w, &s.discover)?;
        let kind = KINDS[d.get_kind().clamp(0, 3) as usize];
        let pack = kind == "modpack";
        if pack && v.unsupported_reason.is_some() {
            return None;
        }
        let merge = pack && d.get_pack_mode() == 1;
        let target = if merge {
            match chosen_destination(&s.discover) {
                Some(destination) => destination.instance_id.clone(),
                None => {
                    d.set_detail_error("Choose a compatible instance first.".into());
                    return None;
                }
            }
        } else if pack {
            String::new()
        } else {
            match s.discover.target.clone() {
                Some(target) => target,
                None => {
                    d.set_detail_error("Choose a target instance first.".into());
                    return None;
                }
            }
        };
        let archive = v.archive_size;
        let (project, version) = (p.project_id.clone(), v.id.clone());
        stop_review(w, &mut s.discover);
        s.discover.detail_serial += 1;
        s.discover.review = None;
        clear_review(w);
        d.set_details_loading(true);
        d.set_detail_error("".into());
        let selected: Vec<_> = s
            .discover
            .optional_files
            .iter()
            .filter(|p| s.discover.selected_optional.contains(*p))
            .cloned()
            .collect();
        let mut request = json!({"projectId":project,"versionId":version,"selectedOptional":selected,"fingerprint":null});
        if merge {
            request["instanceId"] = json!(target);
        }
        // The review runs as its own content job: Cancel stops its archive
        // download between streamed chunks, and its byte count is progress.
        let control = Control::interrupting_transfers();
        s.discover.review_job = Some(control.clone());
        d.set_review_cancellable(true);
        show_review_progress(w, pack, 0, archive);
        if pack {
            let watched = control.clone();
            let timer = slint::Timer::default();
            timer.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(250),
                move || {
                    let streamed = watched.streamed_bytes();
                    ui::with(|w, _| show_review_progress(w, true, streamed, archive));
                },
            );
            s.discover.review_timer = Some(timer);
        }
        Some((
            Review {
                target,
                kind,
                project,
                version,
                merge,
                request,
                fingerprint: String::new(),
            },
            s.discover.detail_serial,
            control,
        ))
    })
    .flatten() else {
        return;
    };
    let work = snapshot.clone();
    let host = ui::host();
    let job = control.clone();
    ui::spawn(
        async move {
            job.run(async move {
                if work.kind == "modpack" {
                    api::preview_pack(host, work.merge, work.request).await
                } else {
                    api::preview_content(host, work.target, work.kind, work.project, work.version)
                        .await
                }
            })
            .await
        },
        move |result| {
            ui::with(|w, s| {
                if s.discover.detail_serial != serial {
                    return;
                }
                s.discover.review_job = None;
                stop_review(w, &mut s.discover);
                let d = w.global::<Discover>();
                d.set_details_loading(false);
                if control.cancellation_observed() {
                    d.set_review_text("Review cancelled. Nothing was installed.".into());
                    return;
                }
                match result {
                    Ok(value) => {
                        apply_review(w, &mut s.discover, snapshot, &value);
                    }
                    Err(e) => d.set_detail_error(format!("{} ({})", e.message, e.code).into()),
                }
            });
        },
    );
}

/// Presents a completed review and keeps it as the only installable
/// snapshot when the core allows it.
fn apply_review(w: &AppWindow, state: &mut DiscoverState, mut snapshot: Review, value: &Value) {
    let d = w.global::<Discover>();
    if snapshot.kind == "modpack" {
        let pack = if snapshot.merge {
            &value["pack"]
        } else {
            value
        };
        state.optional_files = pack["optional"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        render_optional(w, state);
        let version = selected_version(w, state);
        let facts = pack_review_facts(value, snapshot.merge, version, chosen_destination(state));
        d.set_review_facts(ui::model(
            facts
                .into_iter()
                .map(|(label, value)| ReviewFact {
                    label: label.into(),
                    value: value.into(),
                })
                .collect(),
        ));
    }
    let (fingerprint, allowed, text) = review_description(value, snapshot.kind, snapshot.merge);
    snapshot.fingerprint = fingerprint;
    snapshot.request["fingerprint"] = json!(snapshot.fingerprint);
    d.set_review_text(text.into());
    d.set_can_install(allowed);
    if allowed {
        state.review = Some(snapshot);
    }
}

fn install(project: slint::SharedString) {
    // Pack installs always need version selection and a reviewed fingerprint.
    if ui::with(|w, _| w.global::<Discover>().get_kind() == 3).unwrap_or(false) {
        details(project);
        return;
    }
    // Queued in Downloads: the dialog never blocks further browsing, and
    // installs into one instance take turns (see activity.rs).
    let Some((target, kind, title, instance)) = ui::with(|w, s| {
        let d = w.global::<Discover>();
        let target = s.discover.target.clone()?;
        let title = s
            .discover
            .results
            .iter()
            .find(|p| p.project_id == project.as_str())
            .map(|p| p.title.clone())
            .unwrap_or_else(|| "Content".into());
        let instance = s
            .instance(&target)
            .map(|i| i.display_name.clone())
            .unwrap_or_default();
        Some((
            target,
            KINDS[d.get_kind().clamp(0, 2) as usize],
            title,
            instance,
        ))
    })
    .flatten() else {
        return;
    };
    let project = project.to_string();
    crate::activity::submit(
        title.clone(),
        format!("Into {instance}"),
        Some(target.clone()),
        project.clone(),
        move || {
            Box::pin(api::quick_install(
                ui::host(),
                target.clone(),
                kind,
                project.clone(),
            ))
        },
    );
    ui::toast(format!("{title} added to Downloads."), Tone::Neutral);
}

/// Queues the reviewed install and closes the review. The commit still
/// re-resolves and checks the frozen fingerprint in Rust.
fn confirm_install() {
    let Some((snapshot, title, detail)) = ui::with(|w, s| {
        let d = w.global::<Discover>();
        if !d.get_can_install() {
            return None;
        }
        let snapshot = s.discover.review.take()?;
        let title = d.get_project_title().to_string();
        let detail = if snapshot.kind == "modpack" && !snapshot.merge {
            "New modpack instance".to_owned()
        } else {
            format!(
                "Into {}",
                s.instance(&snapshot.target)
                    .map(|i| i.display_name.clone())
                    .unwrap_or_default()
            )
        };
        s.discover.detail_serial += 1;
        d.set_can_install(false);
        d.set_details_open(false);
        Some((snapshot, title, detail))
    })
    .flatten() else {
        return;
    };
    // A new pack instance touches no existing instance, so it runs at once.
    let key = (snapshot.kind != "modpack" || snapshot.merge).then(|| snapshot.target.clone());
    let project = snapshot.project.clone();
    crate::activity::submit(title.clone(), detail, key, project, move || {
        let snapshot = snapshot.clone();
        let host = ui::host();
        Box::pin(async move {
            if snapshot.kind == "modpack" {
                api::install_pack(host, snapshot.merge, snapshot.request).await
            } else {
                api::install_content(
                    host,
                    snapshot.target,
                    snapshot.kind,
                    snapshot.project,
                    snapshot.version,
                    snapshot.fingerprint,
                )
                .await
            }
        })
    });
    ui::toast(format!("{title} added to Downloads."), Tone::Neutral);
}

/// "Forge 47.4.10" from a pack DTO's `loader` kind and `loaderVersion`.
pub(crate) fn pack_loader_label(pack: &Value) -> String {
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
pub(crate) fn pack_review_facts(
    value: &Value,
    merge: bool,
    version: Option<&VersionChoice>,
    destination: Option<&PackDestination>,
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

/// Display is never authority; commit re-resolves the request and fingerprint.
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
        // The first group always shows; the others only when non-empty.
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

/// After any queued install settles: refresh installed markers and state.
pub fn after_install() {
    load_installed();
    crate::instances::reload(crate::home::refresh_readiness);
    crate::instances::after_content_change();
}

/// Re-renders result rows so queued/running projects show as busy.
pub fn render_busy() {
    render();
}

/// Labels for the core's `modpack-progress` vocabulary
/// (`aurora_launcher_lib::modpacks::progress_phase`): a short Downloads-row
/// stage and a sentence. `None` for a name the core does not emit.
pub(crate) fn modpack_phase(phase: &str) -> Option<(&'static str, &'static str)> {
    use aurora_launcher_lib::modpacks::progress_phase as p;
    Some(match phase {
        p::RESOLVING_PACK => ("Resolving", "Verifying the exact modpack version"),
        p::DOWNLOADING => ("Downloading", "Downloading and verifying pack files"),
        p::PREPARING_INSTANCE => ("Preparing", "Preparing the new instance"),
        p::INSTALLING_GAME => ("Installing game", "Installing Minecraft and the mod loader"),
        p::INSTALLING_OVERRIDES => ("Writing files", "Writing pack files and reviewed overrides"),
        p::INSTALLING_COMPONENTS => ("Installing mods", "Installing verified pack components"),
        p::VALIDATING => ("Validating", "Validating the installation"),
        p::COMPLETE => ("Finishing", "Finishing the installation"),
        _ => return None,
    })
}

pub fn progress(phase: &str) {
    let label =
        modpack_phase(phase).map_or("Installing reviewed modpack content", |(_, label)| label);
    ui::with(|w, _| {
        if w.global::<Discover>().get_install_busy() {
            w.global::<Discover>().set_progress(label.into());
        }
    });
}

fn downloads(count: u64) -> String {
    match count {
        0..1_000 => format!("{count} downloads"),
        1_000..1_000_000 => format!("{:.1}K downloads", count as f64 / 1_000.),
        _ => format!("{:.1}M downloads", count as f64 / 1_000_000.),
    }
}
fn render() {
    let busy = crate::activity::busy_projects();
    ui::with(|w, s| {
        crate::perf_trace::mark(
            "discover",
            format_args!("rows={}", s.discover.results.len()),
        );
        let d = w.global::<Discover>();
        d.set_results(ui::model(
            s.discover
                .results
                .iter()
                .map(|p| {
                    let icon = s.discover.icons.get(&p.project_id).cloned();
                    ProjectRow {
                        id: p.project_id.clone().into(),
                        title: p.title.clone().into(),
                        letter: ui::letter(&p.title).into(),
                        author: p.author.clone().into(),
                        description: p.summary.clone().into(),
                        downloads: downloads(p.downloads).into(),
                        installed: s.discover.installed.contains(&p.project_id),
                        busy: busy.contains(&p.project_id),
                        has_picture: icon.is_some(),
                        picture: icon.unwrap_or_default(),
                    }
                })
                .collect(),
        ));
        d.set_has_more(
            s.discover.next_offset < s.discover.total && s.discover.results.len() < MAX_RESULTS,
        );
        d.set_summary(
            if s.discover.results.len() >= MAX_RESULTS {
                format!("Showing {MAX_RESULTS} results. Refine your search for more.")
            } else if s.discover.total > 0 {
                format!(
                    "{} shown · {} matches",
                    s.discover.results.len(),
                    s.discover.total
                )
            } else if !d.get_loading() && d.get_error().is_empty() {
                "No matching projects. Try another search.".into()
            } else {
                String::new()
            }
            .into(),
        );
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use slint::Model;
    fn project(id: &str) -> ProjectSummary {
        ProjectSummary {
            project_id: id.into(),
            title: id.into(),
            summary: String::new(),
            author: String::new(),
            downloads: 0,
        }
    }
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
    fn every_core_modpack_phase_has_its_own_label() {
        let all = aurora_launcher_lib::modpacks::progress_phase::ALL;
        let mut stages = std::collections::HashSet::new();
        let mut sentences = std::collections::HashSet::new();
        for phase in all {
            let (stage, sentence) =
                modpack_phase(phase).unwrap_or_else(|| panic!("{phase} has no label"));
            assert!(stages.insert(stage), "{phase}: duplicate stage {stage}");
            assert!(sentences.insert(sentence), "{phase}: duplicate label");
            assert_ne!(sentence, "Installing reviewed modpack content");
        }
        assert_eq!(
            modpack_phase("installingGame").unwrap().1,
            "Installing Minecraft and the mod loader"
        );
        for retired in ["acquiringComponents", "creatingInstance", "completing"] {
            assert!(modpack_phase(retired).is_none(), "{retired}");
        }
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
        let merge = json!({"fingerprint":"abc","pack":{"name":"Pack","minecraftVersion":"1.21.11"},"blockers":["Wrong Minecraft"],"keptExisting":["config/user.json"]});
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

    // ---- interaction tests over the real AppWindow and bound callbacks ----

    struct TestPlatform;
    impl slint::platform::Platform for TestPlatform {
        fn create_window_adapter(
            &self,
        ) -> Result<std::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            Ok(
                slint::platform::software_renderer::MinimalSoftwareWindow::new(
                    slint::platform::software_renderer::RepaintBufferType::NewBuffer,
                ),
            )
        }
    }

    /// A headless window with the Discover controller bound. No network is
    /// reached: tests never trigger a request whose result they depend on.
    fn test_ui() -> (AppWindow, tokio::runtime::Runtime) {
        let _ = slint::platform::set_platform(Box::new(TestPlatform));
        let window = AppWindow::new().unwrap();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let host = aurora_launcher_lib::host::Host::native(
            Err("headless discover test".into()),
            "test",
            runtime.handle().clone(),
            |_, _| {},
        );
        ui::install(window.clone_strong(), host, runtime.handle().clone());
        bind(&window);
        window.global::<Discover>().set_kind(3);
        window.global::<Discover>().set_details_open(true);
        (window, runtime)
    }

    fn version(
        id: &str,
        channel: &str,
        loader: &str,
        game: &str,
        reason: Option<&str>,
    ) -> VersionChoice {
        VersionChoice {
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
    fn pack_details() -> ProjectDetails {
        ProjectDetails {
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
                    Some(
                        "This Modrinth version is a Forge modpack for Minecraft 1.12.2. Aurora installs Forge for Minecraft 1.15 and newer, and for 1.14.4 from Forge 28.1.16.",
                    ),
                ),
            ],
            default_version_id: Some("2.8".into()),
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

    fn destinations() -> Vec<PackDestination> {
        let row =
            |id: &str, name: &str, game: &str, loader: &str, reasons: &[&str]| PackDestination {
                instance_id: id.into(),
                display_name: name.into(),
                minecraft_version: game.into(),
                loader: loader.into(),
                loader_version: Some("0.19.5".into()),
                compatible: reasons.is_empty(),
                reasons: reasons.iter().map(|r| r.to_string()).collect(),
            };
        vec![
            row(
                "bad00001",
                "Forge world",
                "1.21.1",
                "forge",
                &["This Modrinth version is a Fabric modpack; this instance uses Forge."],
            ),
            row("good0001", "Fabric world", "1.21.1", "fabric", &[]),
            row(
                "old00001",
                "Old world",
                "1.20.1",
                "fabric",
                &[
                    "This Modrinth version is for Minecraft 1.21.1; this instance has Minecraft 1.20.1.",
                ],
            ),
        ]
    }

    fn snapshot(merge: bool, target: &str) -> Review {
        Review {
            target: target.into(),
            kind: "modpack",
            project: "PACK0001".into(),
            version: "2.8".into(),
            merge,
            request: json!({"projectId":"PACK0001","versionId":"2.8","selectedOptional":[],"fingerprint":null}),
            fingerprint: String::new(),
        }
    }

    fn facts(window: &AppWindow) -> Vec<(String, String)> {
        window
            .global::<Discover>()
            .get_review_facts()
            .iter()
            .map(|f| (f.label.to_string(), f.value.to_string()))
            .collect()
    }

    fn fact(window: &AppWindow, label: &str) -> String {
        facts(window)
            .into_iter()
            .find(|(l, _)| l == label)
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("no {label} fact"))
    }

    /// Opens the fixture pack and applies a completed review of `merge`.
    fn reviewed(merge: bool) {
        ui::with(|w, s| {
            show_details(w, &mut s.discover, pack_details(), "modpack", None);
            w.global::<Discover>().set_pack_mode(i32::from(merge));
            if merge {
                s.discover.destinations_for = Some(("PACK0001".into(), "2.8".into()));
                apply_destinations(&mut s.discover, destinations());
                s.discover.destination = Some("good0001".into());
            }
            render_selection(w, &s.discover);
            let value = if merge {
                merge_preview()
            } else {
                new_pack_preview()
            };
            apply_review(
                w,
                &mut s.discover,
                snapshot(merge, if merge { "good0001" } else { "" }),
                &value,
            );
        });
    }

    /// The pack dialog as the presenter renders it for one step, without
    /// the UI thread-locals (layout acceptance renders it at every size).
    pub(crate) fn layout_scene(window: &AppWindow, merge: bool, review: bool) {
        window.global::<Nav>().set_page(Page::Discover);
        let d = window.global::<Discover>();
        d.set_kind(3);
        d.set_details_open(true);
        let mut state = DiscoverState::default();
        show_details(window, &mut state, pack_details(), "modpack", None);
        d.set_pack_mode(i32::from(merge));
        if merge {
            state.destinations_for = Some(("PACK0001".into(), "2.8".into()));
            apply_destinations(&mut state, destinations());
            if review {
                state.destination = Some("good0001".into());
            }
        }
        render_selection(window, &state);
        if review {
            let value = if merge {
                merge_preview()
            } else {
                new_pack_preview()
            };
            let target = if merge { "good0001" } else { "" };
            apply_review(window, &mut state, snapshot(merge, target), &value);
        }
    }

    #[test]
    fn versions_show_core_annotations_and_the_core_default() {
        let (window, _runtime) = test_ui();
        ui::with(|w, s| show_details(w, &mut s.discover, pack_details(), "modpack", None));
        let d = window.global::<Discover>();
        let labels: Vec<String> = d.get_versions().iter().map(|l| l.to_string()).collect();
        assert_eq!(labels.len(), 4, "unsupported versions stay listed");
        assert_eq!(
            labels[0],
            "Pack 3.0-beta · Beta · Minecraft 1.21.1 · Fabric"
        );
        assert_eq!(
            labels[1],
            "Pack 2.9 · Release · Minecraft 1.21.1 · Quilt · can't install"
        );
        assert!(
            labels[3].ends_with("Forge · can't install"),
            "{}",
            labels[3]
        );
        // The core's default (newest installable release), not the newest row.
        assert_eq!(d.get_version_index(), 2);
        assert!(d.get_version_installable());
        assert!(d.get_can_review());
        assert_eq!(d.get_version_problem(), "");
        let detail = d.get_version_detail().to_string();
        assert!(
            detail.starts_with("Release · Minecraft 1.21.1 · Fabric · 12.0 MB archive"),
            "{detail}"
        );
        assert!(
            detail.contains("sets the Minecraft version and mod loader"),
            "{detail}"
        );
        assert!(
            d.get_compatibility()
                .starts_with("Publisher lists Minecraft 1.12.2, 1.21.1 · Fabric, Forge, Quilt")
        );
        assert_eq!(d.get_confirm_label(), "Create instance");

        // Choosing an unsupported version shows why and allows no review.
        d.set_version_index(1);
        d.invoke_version_changed();
        assert!(!d.get_version_installable());
        assert!(!d.get_can_review());
        assert!(d.get_version_problem().contains("does not support Quilt"));
        d.invoke_review();
        assert!(
            !d.get_details_loading(),
            "an unsupported version is never reviewed"
        );
        assert!(ui::state(|s| s.discover.review_job.is_none()));
    }

    #[test]
    fn no_installable_version_is_explained_without_a_default() {
        let (window, _runtime) = test_ui();
        let mut details = pack_details();
        details.versions.retain(|v| v.unsupported_reason.is_some());
        details.default_version_id = None;
        ui::with(|w, s| show_details(w, &mut s.discover, details, "modpack", None));
        let d = window.global::<Discover>();
        assert_eq!(d.get_version_index(), -1);
        assert!(!d.get_can_review());
        assert!(
            d.get_version_problem()
                .contains("can't install any version")
        );
    }

    #[test]
    fn new_instance_review_marks_pack_set_values_and_can_confirm() {
        let (window, _runtime) = test_ui();
        reviewed(false);
        let d = window.global::<Discover>();
        assert_eq!(fact(&window, "Pack version"), "2.8 · Release");
        assert_eq!(fact(&window, "Minecraft"), "1.21.1 · set by the pack");
        assert_eq!(
            fact(&window, "Mod loader"),
            "Fabric 0.16.10 · set by the pack"
        );
        assert_eq!(
            fact(&window, "Destination"),
            "A new instance named after the pack"
        );
        assert!(fact(&window, "Pack archive").starts_with("12.0 MB from Modrinth"));
        let text = d.get_review_text().to_string();
        for expected in [
            "Modrinth components installed (1)\nmods/sodium.jar\n",
            "Other verified files installed (1)\nmods/external.jar\n",
            "Pack overrides written (1)\nconfig/a.toml\n",
            "Not installed: not for this client, or optional and not selected (2)\n",
            "Not installed: pack files outside game content (1)\nserver.properties\n",
        ] {
            assert!(text.contains(expected), "{expected}\n---\n{text}");
        }
        assert!(d.get_can_install());
        assert_eq!(d.get_confirm_label(), "Create instance");
        assert!(ui::state(|s| s
            .discover
            .review
            .as_ref()
            .is_some_and(|r| !r.merge && r.fingerprint == "new-fp")));
        assert_eq!(d.get_optional_files().row_count(), 1);
    }

    #[test]
    fn existing_instance_choice_lists_core_compatibility_and_merge_review() {
        let (window, _runtime) = test_ui();
        let d = window.global::<Discover>();
        ui::with(|w, s| {
            show_details(w, &mut s.discover, pack_details(), "modpack", None);
            w.global::<Discover>().set_pack_mode(1);
            s.discover.destinations_for = Some(("PACK0001".into(), "2.8".into()));
            apply_destinations(&mut s.discover, destinations());
            render_selection(w, &s.discover);
        });
        let rows: Vec<_> = d.get_destinations().iter().collect();
        assert_eq!(rows.len(), 3);
        assert!(!rows[0].compatible && rows[0].detail.contains("this instance uses Forge"));
        assert!(
            rows[1].compatible && rows[1].detail.as_str() == "Minecraft 1.21.1 · Fabric 0.19.5"
        );
        assert!(!rows[2].compatible && rows[2].detail.contains("has Minecraft 1.20.1"));
        assert!(rows.iter().all(|r| !r.selected));
        assert!(!d.get_can_review(), "a destination must be chosen first");
        assert_eq!(d.get_confirm_label(), "Add to instance");

        // An incompatible instance cannot be chosen; a compatible one can.
        d.invoke_choose_destination(0);
        assert_eq!(ui::state(|s| s.discover.destination.clone()), None);
        assert!(!d.get_can_review());
        d.invoke_choose_destination(1);
        assert_eq!(
            ui::state(|s| s.discover.destination.clone()).as_deref(),
            Some("good0001")
        );
        assert!(d.get_destinations().row_data(1).unwrap().selected);
        assert!(d.get_can_review());

        ui::with(|w, s| {
            apply_review(
                w,
                &mut s.discover,
                snapshot(true, "good0001"),
                &merge_preview(),
            )
        });
        assert_eq!(
            fact(&window, "Destination"),
            "Fabric world (existing instance)"
        );
        assert_eq!(
            fact(&window, "Minecraft"),
            "1.21.1 · required by the pack; this instance has 1.21.1"
        );
        assert_eq!(
            fact(&window, "Mod loader"),
            "Fabric 0.16.10 or newer · required by the pack; this instance has Fabric 0.19.5"
        );
        assert_eq!(fact(&window, "Components"), "1 added, at most 64 per merge");
        assert_eq!(
            fact(&window, "Pack updates"),
            "This instance will not follow future updates of this pack."
        );
        let text = d.get_review_text().to_string();
        assert!(text.contains("Your differing config files are kept; the pack's versions are not applied (1)\nconfig/user.toml\n"), "{text}");
        assert!(text.contains("Already present, left as they are (1)\nmods/same.jar\n"));
        assert!(text.contains("New configuration files written (1)\nconfig/new.toml\n"));
        assert!(d.get_can_install());

        // Core-reported blockers keep confirm disabled.
        let mut blocked = merge_preview();
        blocked["blockers"] = json!([
            "This pack needs Fabric Loader 0.17.0 or newer; this instance has Fabric Loader 0.16.0."
        ]);
        ui::with(|w, s| apply_review(w, &mut s.discover, snapshot(true, "good0001"), &blocked));
        assert!(!d.get_can_install());
        assert!(d.get_review_text().contains("Blocked (1)"));
    }

    #[test]
    fn a_kept_destination_survives_only_while_the_core_says_compatible() {
        let mut state = DiscoverState {
            destination: Some("good0001".into()),
            ..Default::default()
        };
        apply_destinations(&mut state, destinations());
        assert_eq!(state.destination.as_deref(), Some("good0001"));
        let mut changed = destinations();
        changed[1].compatible = false;
        changed[1].reasons =
            vec!["Finish installing this instance before adding a modpack.".into()];
        apply_destinations(&mut state, changed);
        assert_eq!(state.destination, None);
        // Opened from an instance: that instance is preselected when compatible.
        let mut state = DiscoverState {
            origin: Some(("good0001".into(), 3)),
            ..Default::default()
        };
        apply_destinations(&mut state, destinations());
        assert_eq!(state.destination.as_deref(), Some("good0001"));
        let mut state = DiscoverState {
            origin: Some(("bad00001".into(), 3)),
            ..Default::default()
        };
        apply_destinations(&mut state, destinations());
        assert_eq!(state.destination, None);
    }

    #[test]
    fn changing_version_destination_or_optional_files_clears_the_review() {
        let (window, _runtime) = test_ui();
        let d = window.global::<Discover>();
        let cleared = |why: &str| {
            assert!(!d.get_can_install(), "{why}");
            assert!(ui::state(|s| s.discover.review.is_none()), "{why}");
            assert_eq!(
                window.global::<Discover>().get_review_facts().row_count(),
                0,
                "{why}"
            );
        };

        reviewed(false);
        assert!(d.get_can_install());
        d.set_version_index(0);
        d.invoke_version_changed();
        cleared("version");

        // Destination: existing → new (no destination request is made).
        reviewed(true);
        assert!(d.get_can_install());
        d.set_pack_mode(0);
        d.invoke_destination_mode_changed();
        cleared("destination mode");

        // Choosing another compatible instance also invalidates.
        reviewed(true);
        ui::with(|_, s| {
            s.discover.destinations[2].compatible = true;
            s.discover.destinations[2].reasons.clear();
        });
        d.invoke_choose_destination(2);
        cleared("destination instance");

        // Optional files: the selection is part of the fingerprinted request.
        reviewed(false);
        assert!(d.get_can_install());
        d.invoke_toggle_optional("mods/opt.jar".into());
        cleared("optional");
        assert!(ui::state(|s| s
            .discover
            .selected_optional
            .contains("mods/opt.jar")));
        assert!(d.get_review_text().contains("Review installation again"));
        // A path the review never offered changes nothing.
        reviewed(false);
        d.invoke_toggle_optional("mods/unknown.jar".into());
        assert!(d.get_can_install());
    }

    #[test]
    fn cancelling_a_review_cancels_its_job_and_says_nothing_was_installed() {
        let (window, _runtime) = test_ui();
        let d = window.global::<Discover>();
        let job = Control::default();
        ui::with(|w, s| {
            show_details(w, &mut s.discover, pack_details(), "modpack", None);
            s.discover.review_job = Some(job.clone());
            w.global::<Discover>().set_details_loading(true);
            w.global::<Discover>().set_review_cancellable(true);
        });
        let serial = ui::state(|s| s.discover.detail_serial);
        d.invoke_cancel_review();
        assert_eq!(
            job.phase(),
            aurora_launcher_lib::content_jobs::Phase::Cancelling
        );
        assert!(!d.get_details_loading());
        assert!(!d.get_review_cancellable());
        assert!(!d.get_can_install());
        assert_eq!(
            d.get_review_text(),
            "Review cancelled. Nothing was installed."
        );
        // A late result of the cancelled review is discarded by serial.
        assert!(ui::state(|s| s.discover.detail_serial) > serial);
        assert!(ui::state(|s| s.discover.review_job.is_none()));
    }

    #[test]
    fn review_progress_is_honest_about_what_is_known() {
        let mb = 1024 * 1024;
        assert_eq!(
            review_progress(true, 0, Some(123 * mb)),
            (
                "Getting the pack archive (123.0 MB) to review it…".into(),
                -1.
            )
        );
        let (text, fraction) = review_progress(true, 41 * mb, Some(123 * mb));
        assert_eq!(
            text,
            "Downloading the pack archive to review it: 41.0 MB of 123.0 MB"
        );
        assert!((fraction - 41. / 123.).abs() < 1e-6);
        // Fully received (or more, after a retry): no invented percentage.
        for streamed in [123 * mb, 200 * mb] {
            assert_eq!(review_progress(true, streamed, Some(123 * mb)).1, -1.);
        }
        assert_eq!(
            review_progress(true, 5 * mb, None).0,
            "Downloading the pack archive to review it: 5.0 MB received"
        );
        assert_eq!(review_progress(false, 0, None).1, -1.);
    }
}
