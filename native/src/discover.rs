//! Modrinth presentation over the existing provider and reviewed pack engines.
//! Requests freeze target, category and exact version before leaving the UI thread.
use crate::api::{self, ProjectDetails, ProjectSummary};
use crate::ui;
use crate::{AppWindow, Discover, Nav, Page, ProjectRow, Tone};
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
    pub origin: Option<(String, i32)>,
}

pub fn bind(window: &AppWindow) {
    let d = window.global::<Discover>();
    d.on_search(|| search(false));
    d.on_more(|| search(true));
    d.on_kind_changed(|_| search(false));
    d.on_instance_changed(|index| {
        let id = ui::state(|s| s.instances().get(index as usize).map(|i| i.id.clone()));
        ui::with(|_, state| state.discover.target = id);
        invalidate_review();
        search(false);
    });
    d.on_install(install);
    d.on_details(details);
    d.on_version_changed(invalidate_review);
    d.on_review(review);
    d.on_confirm_install(confirm_install);
    d.on_close_details(|| {
        ui::with(|w, s| {
            if w.global::<Discover>().get_install_busy() {
                return;
            }
            s.discover.detail_serial += 1;
            s.discover.review = None;
            let d = w.global::<Discover>();
            d.set_details_loading(false);
            d.set_details_open(false);
        });
    });
    d.on_return_to_instance(|| {
        if let Some((id, tab)) = ui::state(|s| s.discover.origin.clone()) {
            crate::instances::open_workspace(id, tab);
        }
    });
}

pub fn open_for(instance_id: String, kind: i32) {
    ui::with(|window, state| {
        state.discover.origin = Some((instance_id.clone(), kind + 1));
        state.discover.target = Some(instance_id);
        window.global::<Discover>().set_kind(kind.clamp(0, 3));
        window.global::<Nav>().set_page(Page::Discover);
    });
    render_targets();
    search(false);
}

pub fn on_shown() {
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

fn search(append: bool) {
    let Some((target, kind, query, offset, serial)) = ui::with(|window, state| {
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
        ))
    })
    .flatten() else {
        render();
        return;
    };
    render();
    load_installed();
    ui::spawn(
        api::browse(ui::host(), target, kind, query, offset),
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

fn invalidate_review() {
    ui::with(|w, s| {
        s.discover.detail_serial += 1;
        s.discover.review = None;
        let d = w.global::<Discover>();
        d.set_details_loading(false);
        d.set_can_install(false);
        d.set_review_text("".into());
        d.set_detail_error("".into());
    });
}

fn details(project: slint::SharedString) {
    let Some((target, kind, serial)) = ui::with(|w, s| {
        let d = w.global::<Discover>();
        if d.get_install_busy() {
            return None;
        }
        s.discover.detail_serial += 1;
        s.discover.details = None;
        s.discover.review = None;
        d.set_details_open(true);
        d.set_details_loading(true);
        d.set_detail_error("".into());
        d.set_project_title("Project details".into());
        d.set_project_summary("".into());
        d.set_compatibility("".into());
        d.set_versions(ui::model(Vec::new()));
        d.set_version_index(-1);
        d.set_review_text("".into());
        d.set_can_install(false);
        let kind = KINDS[d.get_kind().clamp(0, 3) as usize];
        Some((
            if kind == "modpack" {
                String::new()
            } else {
                s.discover.target.clone().unwrap_or_default()
            },
            kind,
            s.discover.detail_serial,
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
                match result {
                    Ok(p) => {
                        d.set_project_title(p.title.clone().into());
                        d.set_project_summary(p.summary.clone().into());
                        d.set_compatibility(
                            format!(
                                "Minecraft: {}\nLoaders: {}",
                                p.game_versions
                                    .iter()
                                    .take(16)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", "),
                                p.loaders.join(", ")
                            )
                            .into(),
                        );
                        d.set_versions(ui::model(
                            p.versions
                                .iter()
                                .map(|v| {
                                    format!(
                                        "{} · {} · {}",
                                        v.name,
                                        v.version_number,
                                        v.loaders.join(", ")
                                    )
                                    .into()
                                })
                                .collect(),
                        ));
                        let index = p
                            .default_version_id
                            .as_ref()
                            .and_then(|id| p.versions.iter().position(|v| &v.id == id))
                            .or_else(|| (!p.versions.is_empty()).then_some(0));
                        d.set_version_index(index.map(|i| i as i32).unwrap_or(-1));
                        if index.is_none() {
                            d.set_detail_error("No supported installable versions. Modpacks require Fabric and a supported .mrpack archive.".into());
                        }
                        s.discover.details = Some(p);
                    }
                    Err(e) => d.set_detail_error(format!("{} ({})", e.message, e.code).into()),
                }
            });
        },
    );
}

fn review() {
    let Some((mut snapshot,serial))=ui::with(|w,s| {
        let d=w.global::<Discover>();
        if d.get_install_busy() || d.get_details_loading(){return None;}
        let p=s.discover.details.as_ref()?;
        let v=p.versions.get(d.get_version_index() as usize)?;
        let kind=KINDS[d.get_kind().clamp(0,3) as usize];
        let merge=kind=="modpack" && d.get_pack_mode()==1;
        let target=s.discover.target.clone().unwrap_or_default();
        if (kind!="modpack" || merge) && target.is_empty(){d.set_detail_error("Choose a target instance first.".into());return None;}
        s.discover.detail_serial+=1;
        s.discover.review=None;
        d.set_can_install(false); d.set_details_loading(true);d.set_detail_error("".into());
        let mut request=json!({"projectId":p.project_id,"versionId":v.id,"selectedOptional":[],"fingerprint":null});
        if merge {request["instanceId"]=json!(target);}
        Some((Review{target,kind,project:p.project_id.clone(),version:v.id.clone(),merge,request,fingerprint:String::new()},s.discover.detail_serial))
    }).flatten() else {return};
    let work = snapshot.clone();
    let host = ui::host();
    ui::spawn(
        async move {
            if work.kind == "modpack" {
                api::preview_pack(host, work.merge, work.request).await
            } else {
                api::preview_content(host, work.target, work.kind, work.project, work.version).await
            }
        },
        move |result| {
            ui::with(|w, s| {
                if s.discover.detail_serial != serial {
                    return;
                }
                let d = w.global::<Discover>();
                d.set_details_loading(false);
                match result {
                    Ok(value) => {
                        let (fingerprint, allowed, text) =
                            review_description(&value, snapshot.kind, snapshot.merge);
                        snapshot.fingerprint = fingerprint;
                        snapshot.request["fingerprint"] = json!(snapshot.fingerprint);
                        d.set_review_text(text.into());
                        d.set_can_install(allowed);
                        if allowed {
                            s.discover.review = Some(snapshot);
                        }
                    }
                    Err(e) => d.set_detail_error(format!("{} ({})", e.message, e.code).into()),
                }
            });
        },
    );
}

fn install(project: slint::SharedString) {
    // Pack installs always need version selection and a reviewed fingerprint.
    if ui::with(|w, _| w.global::<Discover>().get_kind() == 3).unwrap_or(false) {
        details(project);
        return;
    }
    let Some((target, kind)) = ui::with(|w, s| {
        let d = w.global::<Discover>();
        if d.get_install_busy() {
            return None;
        }
        let target = s.discover.target.clone()?;
        d.set_install_busy(true);
        d.set_progress("Resolving dependencies and verifying content…".into());
        Some((target, KINDS[d.get_kind().clamp(0, 2) as usize]))
    })
    .flatten() else {
        return;
    };
    ui::spawn(
        api::quick_install(ui::host(), target, kind, project.to_string()),
        finish_install,
    );
}

fn confirm_install() {
    let Some(snapshot) = ui::with(|w, s| {
        let d = w.global::<Discover>();
        if d.get_install_busy() || !d.get_can_install() {
            return None;
        }
        let snapshot = s.discover.review.take()?;
        d.set_install_busy(true);
        d.set_can_install(false);
        d.set_progress("Installing reviewed content…".into());
        Some(snapshot)
    })
    .flatten() else {
        return;
    };
    let host = ui::host();
    ui::spawn(
        async move {
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
        },
        finish_install,
    );
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
        text.push_str(&format!("{} · {}\nMinecraft {} · Fabric {}\n{}\n",pack["name"].as_str().unwrap_or("Pack"),pack["packVersion"].as_str().unwrap_or(""),pack["minecraftVersion"].as_str().unwrap_or(""),pack["fabricLoaderVersion"].as_str().unwrap_or(""),if merge {"Adds ordinary content to the chosen instance. It will not follow modpack updates. Existing differing configuration is kept."}else{"Creates a separate managed modpack instance. Aurora is not added. The current instance stays unchanged."}));
        let groups = if merge {
            vec![
                ("Added files", array(value, "added")),
                ("Already present", array(value, "alreadyPresent")),
                ("Existing files kept", array(value, "keptExisting")),
                ("Overrides written", array(value, "overridesWritten")),
                ("Blockers", array(value, "blockers")),
            ]
        } else {
            vec![
                ("Recognized files", array(pack, "recognized")),
                ("Verified external files", array(pack, "unresolved")),
                ("Overrides", array(pack, "overrides")),
                ("Optional files omitted", array(pack, "optional")),
                ("Excluded files", array(pack, "excluded")),
            ]
        };
        allowed &= array(value, "blockers").is_empty();
        for (label, rows) in groups {
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

fn finish_install(result: api::ApiResult<Value>) {
    ui::with(|w, s| {
        let d = w.global::<Discover>();
        d.set_install_busy(false);
        d.set_progress("".into());
        if result.is_ok() {
            d.set_details_open(false);
            s.discover.review = None;
        } else if let Err(e) = &result {
            d.set_detail_error(format!("{} ({})", e.message, e.code).into());
        }
    });
    match result {
        Ok(_) => ui::toast("Installation completed and verified.", Tone::Success),
        Err(e) => ui::error_toast(&e),
    }
    load_installed();
    crate::instances::reload(crate::home::refresh_readiness);
}

pub fn progress(phase: &str) {
    let label = match phase {
        "resolvingPack" => "Verifying the exact modpack",
        "acquiringComponents" => "Downloading and verifying pack files",
        "creatingInstance" => "Creating the isolated instance",
        "installingComponents" => "Installing verified content",
        "installingOverrides" => "Applying reviewed overrides",
        "validating" => "Validating the installation",
        "completing" => "Finishing the installation",
        _ => "Installing reviewed modpack content",
    };
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
    ui::with(|w, s| {
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
                        busy: false,
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
mod tests {
    use super::*;
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
        assert!(display.contains("Wrong Minecraft"));
        assert!(display.contains("config/user.json"));
        assert!(display.contains("will not follow modpack updates"));
    }
}
