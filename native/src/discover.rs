//! Discover: Modrinth browsing through the launcher's provider service, with
//! installs into one explicitly chosen instance. Compatibility filtering,
//! dependency resolution and verification all stay in `application`.

use slint::ComponentHandle;

use crate::api::{self, ProjectSummary};
use crate::ui;
use crate::{AppWindow, Discover, Nav, Page, ProjectRow, Tone};

const KINDS: [&str; 3] = ["mod", "resourcePack", "shaderPack"];

#[derive(Default)]
pub struct DiscoverState {
    pub target: Option<String>,
    pub results: Vec<ProjectSummary>,
    pub installed: std::collections::HashSet<String>,
    pub busy: std::collections::HashSet<String>,
    pub total: u32,
    pub serial: u64,
}

pub fn bind(window: &AppWindow) {
    let discover = window.global::<Discover>();
    discover.on_search(|| search(false));
    discover.on_more(|| search(true));
    discover.on_kind_changed(|_| search(false));
    discover.on_instance_changed(|index| {
        let id = ui::state(|s| s.instances().get(index as usize).map(|i| i.id.clone()));
        ui::with(|_, state| state.discover.target = id);
        search(false);
    });
    discover.on_install(install);
}

/// Opens Discover for one instance and content kind (from a workspace).
pub fn open_for(instance_id: String, kind: i32) {
    ui::with(|window, state| {
        state.discover.target = Some(instance_id);
        window.global::<Discover>().set_kind(kind.clamp(0, 2));
        window.global::<Nav>().invoke_navigate(Page::Discover);
    });
    render_targets();
    search(false);
}

/// Called on navigation: runs a first search when nothing is shown yet.
pub fn on_shown() {
    let empty = ui::state(|s| s.discover.results.is_empty() && s.discover.serial == 0);
    render_targets();
    if empty {
        search(false);
    }
}

pub fn render_targets() {
    ui::with(|window, state| {
        let discover = window.global::<Discover>();
        let names: Vec<slint::SharedString> = state
            .instances()
            .iter()
            .map(|i| i.display_name.clone().into())
            .collect();
        discover.set_has_instances(!names.is_empty());
        let target = state
            .discover
            .target
            .clone()
            .filter(|id| state.instance(id).is_some())
            .or_else(|| state.selected_instance().map(|i| i.id.clone()))
            .or_else(|| state.instances().first().map(|i| i.id.clone()));
        let index = target
            .as_ref()
            .and_then(|id| state.instances().iter().position(|i| &i.id == id))
            .unwrap_or(0);
        state.discover.target = target;
        discover.set_instance_names(ui::model(names));
        discover.set_instance_index(index as i32);
    });
}

fn search(append: bool) {
    let Some((target, kind, query, offset, serial)) = ui::with(|window, state| {
        let discover = window.global::<Discover>();
        let target = state.discover.target.clone()?;
        state.discover.serial += 1;
        if !append {
            state.discover.results.clear();
            state.discover.installed.clear();
        }
        discover.set_loading(true);
        discover.set_error("".into());
        let kind = KINDS[discover.get_kind().clamp(0, 2) as usize];
        Some((
            target,
            kind,
            discover.get_query().to_string(),
            state.discover.results.len() as u32,
            state.discover.serial,
        ))
    })
    .flatten() else {
        return;
    };
    render();
    ui::spawn(
        api::browse(ui::host(), target, kind, query, offset),
        move |result| {
            ui::with(|window, state| {
                if state.discover.serial != serial {
                    return;
                }
                let discover = window.global::<Discover>();
                discover.set_loading(false);
                match result {
                    Ok(page) => {
                        state.discover.total = page.total_hits;
                        state.discover.results.extend(page.hits);
                    }
                    Err(error) => discover.set_error(error.message.into()),
                }
            });
            render();
        },
    );
}

fn install(project: slint::SharedString) {
    let Some((target, kind, project)) = ui::with(|window, state| {
        let target = state.discover.target.clone()?;
        let project = project.to_string();
        if !state.discover.busy.insert(project.clone()) {
            return None;
        }
        let kind = KINDS[window.global::<Discover>().get_kind().clamp(0, 2) as usize];
        Some((target, kind, project))
    })
    .flatten() else {
        return;
    };
    render();
    let done_project = project.clone();
    ui::spawn(
        api::quick_install(ui::host(), target, kind, project),
        move |result| {
            ui::with(|_, state| {
                state.discover.busy.remove(&done_project);
                if result.is_ok() {
                    state.discover.installed.insert(done_project.clone());
                }
            });
            match result {
                Ok(_) => ui::toast(
                    "Installed. Dependencies were resolved and verified.",
                    Tone::Success,
                ),
                Err(error) => ui::error_toast(&error),
            }
            render();
            crate::home::refresh_readiness();
        },
    );
}

fn downloads(count: u64) -> String {
    match count {
        0..1_000 => format!("{count} downloads"),
        1_000..1_000_000 => format!("{:.1}K downloads", count as f64 / 1_000.0),
        _ => format!("{:.1}M downloads", count as f64 / 1_000_000.0),
    }
}

fn render() {
    ui::with(|window, state| {
        let discover = window.global::<Discover>();
        let rows: Vec<ProjectRow> = state
            .discover
            .results
            .iter()
            .map(|project| ProjectRow {
                id: project.project_id.clone().into(),
                title: project.title.clone().into(),
                letter: ui::letter(&project.title).into(),
                author: project.author.clone().into(),
                description: project.summary.clone().into(),
                downloads: downloads(project.downloads).into(),
                installed: state.discover.installed.contains(&project.project_id),
                busy: state.discover.busy.contains(&project.project_id),
            })
            .collect();
        let shown = rows.len() as u32;
        discover.set_has_more(shown < state.discover.total);
        discover.set_summary(
            if state.discover.total > 0 {
                format!("{shown} of {} results", state.discover.total)
            } else {
                String::new()
            }
            .into(),
        );
        discover.set_results(ui::model(rows));
    });
}
