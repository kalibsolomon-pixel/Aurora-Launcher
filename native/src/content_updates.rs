//! Native controls over the shared update engines.
//!
//! Content: an explicit availability check, then per-item or all-eligible
//! updates through the reviewed bulk preview. A preview that only replaces the
//! chosen items is queued directly; one that adds or removes dependencies or
//! carries warnings is shown for review first. Managed modpacks: check, review
//! the three-way plan, and resolve conflicts only by keeping local copies.
//! Every apply re-resolves in Rust and must match the reviewed fingerprint;
//! the UI holds display state only.

use std::cell::RefCell;
use std::collections::HashMap;

use serde_json::Value;
use slint::ComponentHandle;

use crate::api::{self, UpdateEntry};
use crate::instances::{CONTENT_KINDS, ContentItem};
use crate::{AppWindow, ContentRow, Instances, Tone, ui};

/// The last explicit check for one content tab of the open workspace.
#[derive(Clone, Default)]
pub struct Updates {
    pub tab: i32,
    pub entries: HashMap<String, UpdateEntry>,
    pub summary: String,
    pub checked: bool,
}

struct PendingUpdate {
    instance: String,
    kind: &'static str,
    projects: Vec<String>,
    fingerprint: String,
    title: String,
    detail: String,
}

struct PackReview {
    instance: String,
    name: String,
    fingerprint: String,
    conflicts: Vec<String>,
}

thread_local! {
    static PENDING: RefCell<Option<PendingUpdate>> = const { RefCell::new(None) };
    static PACK: RefCell<Option<PackReview>> = const { RefCell::new(None) };
}

pub fn bind(window: &AppWindow) {
    let instances = window.global::<Instances>();
    instances.on_check_updates(check);
    instances.on_update_all(|| start(None));
    instances.on_content_update(|entry| start(Some(entry.to_string())));
    instances.on_content_unpin(|entry| unpin(entry.to_string()));
    instances.on_update_review_confirm(|| {
        ui::with(|w, _| w.global::<Instances>().set_update_review_open(false));
        if let Some(pending) = PENDING.with(|p| p.borrow_mut().take()) {
            submit(pending);
        }
    });
    instances.on_update_review_dismiss(|| {
        PENDING.with(|p| p.borrow_mut().take());
        ui::with(|w, _| w.global::<Instances>().set_update_review_open(false));
    });
    instances.on_pack_check_update(pack_check);
    instances.on_pack_review_update(pack_review);
    instances.on_pack_review_confirm(pack_confirm);
}

/// Updatable now: a compatible candidate under policy, not pinned.
pub fn eligible(entry: &UpdateEntry) -> bool {
    entry.status == "updateAvailable" && !entry.pinned && entry.candidate.is_some()
}

/// Badge text, tone, and the actions a row offers after a check.
pub fn decorate(row: &mut ContentRow, project: Option<&str>, updates: &Updates) {
    row.update_text = "".into();
    row.update_tone = Tone::Neutral;
    row.can_update = false;
    row.can_unpin = false;
    if !updates.checked {
        return;
    }
    let Some(project) = project else {
        return;
    };
    let Some(entry) = updates.entries.get(project) else {
        // Provider-managed dependencies advance with their parents.
        row.update_text = "Updated with its parent".into();
        return;
    };
    let candidate = entry
        .candidate
        .as_ref()
        .map(|c| c.version_number.clone())
        .unwrap_or_default();
    let (text, tone) = match entry.status.as_str() {
        "updateAvailable" if entry.pinned => {
            (format!("Pinned · {candidate} available"), Tone::Neutral)
        }
        "updateAvailable" => (format!("Update: {candidate}"), Tone::Accent),
        "pinnedUpdateAvailable" => (format!("Pinned · {candidate} available"), Tone::Neutral),
        "upToDate" => ("Up to date".to_owned(), Tone::Success),
        "noNewerUnderPolicy" => ("No update in its release channel".to_owned(), Tone::Neutral),
        "currentVersionUnknown" => ("Version unknown to Modrinth".to_owned(), Tone::Warning),
        "providerUnavailable" => ("Check failed".to_owned(), Tone::Error),
        "blocked" => (
            block_label(entry.block.as_deref()).to_owned(),
            Tone::Warning,
        ),
        _ => ("Update status unknown".to_owned(), Tone::Neutral),
    };
    row.update_text = text.into();
    row.update_tone = tone;
    row.can_update = eligible(entry);
    row.can_unpin = entry.pinned && entry.candidate.is_some();
}

fn block_label(block: Option<&str>) -> &'static str {
    match block {
        Some("packOwned") => "Managed by the modpack",
        Some("disabled") => "Disabled — enable to update",
        Some("locallyModified") => "Changed locally — not updated",
        Some("missing") => "File missing",
        Some("conflict") => "File conflict",
        _ => "Update blocked",
    }
}

/// "3 updates available · 1 pinned · 8 up to date".
pub fn summarize(entries: &[UpdateEntry], dependencies: usize) -> (String, i32) {
    let count = |status: &str| entries.iter().filter(|e| e.status == status).count();
    let available = entries.iter().filter(|e| eligible(e)).count();
    let pinned = entries
        .iter()
        .filter(|e| e.pinned && e.candidate.is_some())
        .count();
    let current = count("upToDate") + count("noNewerUnderPolicy");
    let blocked = count("blocked") + count("currentVersionUnknown");
    let failed = count("providerUnavailable");
    let mut parts = Vec::new();
    if entries.is_empty() && dependencies == 0 {
        return (
            "Nothing here is managed from Modrinth, so there is nothing to update.".into(),
            0,
        );
    }
    parts.push(match available {
        0 => "No updates available".to_owned(),
        1 => "1 update available".to_owned(),
        n => format!("{n} updates available"),
    });
    if pinned > 0 {
        parts.push(format!("{pinned} pinned"));
    }
    if current > 0 {
        parts.push(format!("{current} up to date"));
    }
    if blocked > 0 {
        parts.push(format!("{blocked} need attention"));
    }
    if failed > 0 {
        parts.push(format!("{failed} could not be checked"));
    }
    if dependencies > 0 {
        parts.push(format!("{dependencies} dependencies follow their parents"));
    }
    (parts.join(" · "), available as i32)
}

fn check() {
    let Some((id, tab, kind)) = ui::with(|w, s| {
        let workspace = s.workspace.as_ref()?;
        let instances = w.global::<Instances>();
        let tab = instances.get_tab();
        let kind = *CONTENT_KINDS.get(tab as usize)?;
        instances.set_updates_busy(true);
        Some((workspace.id.clone(), tab, kind))
    })
    .flatten() else {
        return;
    };
    let instance = id.clone();
    ui::spawn(api::check_updates(ui::host(), id, kind), move |result| {
        ui::with(|w, s| {
            w.global::<Instances>().set_updates_busy(false);
            let Some(workspace) = s.workspace.as_mut().filter(|w| w.id == instance) else {
                return;
            };
            workspace.updates = match result {
                Ok(report) => {
                    let (summary, _) = summarize(&report.entries, report.dependency_managed);
                    Updates {
                        tab,
                        summary,
                        checked: true,
                        entries: report
                            .entries
                            .into_iter()
                            .map(|e| (e.project_id.clone(), e))
                            .collect(),
                    }
                }
                Err(error) => Updates {
                    tab,
                    summary: format!("Update check failed: {}", error.message),
                    ..Default::default()
                },
            };
        });
        crate::instances::render_content();
    });
}

/// Starts one item (`entry` = its row id) or every eligible item of the tab.
fn start(entry: Option<String>) {
    let Some((instance, kind, projects, title, detail)) = ui::with(|w, s| {
        let workspace = s.workspace.as_ref()?;
        let updates = &workspace.updates;
        let tab = w.global::<Instances>().get_tab();
        if !updates.checked || updates.tab != tab {
            return None;
        }
        let kind = *CONTENT_KINDS.get(tab as usize)?;
        let instance_name = s
            .instance(&workspace.id)
            .map(|i| i.display_name.clone())
            .unwrap_or_default();
        let (projects, title) = match &entry {
            Some(entry_id) => {
                let item: &ContentItem = workspace
                    .content
                    .iter()
                    .find(|i| i.row.id == entry_id.as_str())?;
                let project = item.project.clone()?;
                updates.entries.get(&project).filter(|e| eligible(e))?;
                (vec![project], format!("Update {}", item.row.title))
            }
            None => {
                let mut projects: Vec<String> = updates
                    .entries
                    .values()
                    .filter(|e| eligible(e))
                    .map(|e| e.project_id.clone())
                    .collect();
                projects.sort();
                let noun = match kind {
                    "mod" => "mods",
                    "resourcePack" => "resource packs",
                    _ => "shaders",
                };
                let title = format!("Update {} {noun}", projects.len());
                (projects, title)
            }
        };
        if projects.is_empty() {
            return None;
        }
        w.global::<Instances>().set_updates_busy(true);
        Some((
            workspace.id.clone(),
            kind,
            projects,
            title,
            format!("In {instance_name}"),
        ))
    })
    .flatten() else {
        return;
    };
    let request = (instance.clone(), projects.clone());
    ui::spawn(
        api::preview_updates(ui::host(), request.0, kind, request.1),
        move |result| {
            ui::with(|w, _| w.global::<Instances>().set_updates_busy(false));
            match result {
                Ok(preview) => {
                    let fingerprint = preview["previewFingerprint"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned();
                    if fingerprint.is_empty() {
                        ui::toast(
                            "The update service did not provide a valid review.",
                            Tone::Error,
                        );
                        return;
                    }
                    let (needs_review, text) = describe(&preview);
                    let pending = PendingUpdate {
                        instance,
                        kind,
                        projects,
                        fingerprint,
                        title,
                        detail,
                    };
                    if needs_review {
                        ui::with(|w, _| {
                            let instances = w.global::<Instances>();
                            instances.set_update_review_title(pending.title.clone().into());
                            instances.set_update_review_text(text.into());
                            instances.set_update_review_open(true);
                        });
                        PENDING.with(|p| *p.borrow_mut() = Some(pending));
                    } else {
                        submit(pending);
                    }
                }
                Err(error) => ui::error_toast(&error),
            }
        },
    );
}

fn item_name(item: &Value) -> String {
    let file = item["fileName"].as_str().unwrap_or("Content");
    match item["displayVersion"].as_str() {
        Some(version) => format!("{file} ({version})"),
        None => file.to_owned(),
    }
}

/// Review text, and whether the change goes beyond replacing the chosen items.
pub fn describe(preview: &Value) -> (bool, String) {
    let list = |value: &Value| value.as_array().cloned().unwrap_or_default();
    let mut text = String::from("Updates\n");
    for target in list(&preview["targets"]) {
        text.push_str(&format!(
            "{} → {}\n",
            item_name(&target["current"]),
            target["candidate"]["versionNumber"]
                .as_str()
                .unwrap_or("new version"),
        ));
    }
    let added = list(&preview["delta"]["newRequirements"]);
    let removed = list(&preview["delta"]["removedRequirements"]);
    let warnings = list(&preview["warnings"]);
    if !added.is_empty() {
        text.push_str("\nNew dependencies installed\n");
        for item in &added {
            text.push_str(&format!("{}\n", item_name(item)));
        }
    }
    if !removed.is_empty() {
        text.push_str("\nDependencies no longer required (removed)\n");
        for item in &removed {
            text.push_str(&format!("{}\n", item_name(item)));
        }
    }
    if !warnings.is_empty() {
        text.push_str("\nWarnings\n");
        for warning in &warnings {
            text.push_str(&format!(
                "{}\n",
                warning.as_str().unwrap_or("Review warning")
            ));
        }
    }
    (
        !added.is_empty() || !removed.is_empty() || !warnings.is_empty(),
        text,
    )
}

fn submit(pending: PendingUpdate) {
    let PendingUpdate {
        instance,
        kind,
        projects,
        fingerprint,
        title,
        detail,
    } = pending;
    let key = instance.clone();
    let project = format!("update:{kind}:{}", projects.join(","));
    crate::activity::submit(title.clone(), detail, Some(key), project, move || {
        Box::pin(api::apply_updates(
            ui::host(),
            instance.clone(),
            kind,
            projects.clone(),
            fingerprint.clone(),
        ))
    });
    ui::toast(format!("{title} added to Downloads."), Tone::Neutral);
}

fn unpin(entry: String) {
    let Some((instance, kind, project)) = ui::with(|w, s| {
        let workspace = s.workspace.as_ref()?;
        let kind = *CONTENT_KINDS.get(w.global::<Instances>().get_tab() as usize)?;
        let project = workspace
            .content
            .iter()
            .find(|i| i.row.id == entry.as_str())?
            .project
            .clone()?;
        Some((workspace.id.clone(), kind, project))
    })
    .flatten() else {
        return;
    };
    ui::spawn(
        api::set_update_pinned(ui::host(), instance, kind, project, false),
        |result| match result {
            Ok(_) => {
                ui::toast("Unpinned. It can now be updated.", Tone::Success);
                check();
            }
            Err(error) => ui::error_toast(&error),
        },
    );
}

/// After any queued change to the open instance settles: refresh a tab whose
/// updates were checked so badges never describe superseded files.
pub fn after_change() {
    let recheck = ui::with(|w, s| {
        let workspace = s.workspace.as_mut()?;
        let checked =
            workspace.updates.checked && workspace.updates.tab == w.global::<Instances>().get_tab();
        workspace.updates = Updates::default();
        Some(checked)
    })
    .flatten()
    .unwrap_or(false);
    if recheck {
        check();
    }
}

// ------------------------------------------------- managed modpacks

fn pack_check() {
    let Some(id) = ui::with(|w, s| {
        let id = s.workspace.as_ref()?.id.clone();
        let instances = w.global::<Instances>();
        instances.set_pack_update_busy(true);
        instances.set_pack_update_available(false);
        Some(id)
    })
    .flatten() else {
        return;
    };
    ui::spawn(api::check_modpack_update(ui::host(), id), |result| {
        ui::with(|w, _| {
            let instances = w.global::<Instances>();
            instances.set_pack_update_busy(false);
            let (text, available) = match result {
                Ok(check) => pack_check_text(&check),
                Err(error) => (error.message, false),
            };
            instances.set_pack_update_text(text.into());
            instances.set_pack_update_available(available);
        });
    });
}

pub fn pack_check_text(check: &Value) -> (String, bool) {
    let current = check["currentPackVersion"].as_str().unwrap_or("");
    let modified = match check["divergences"].as_u64().unwrap_or(0) {
        0 => String::new(),
        n => format!(" ({n} local change(s) recorded)"),
    };
    match check["status"].as_str().unwrap_or("") {
        "updateAvailable" => (
            format!(
                "{} available (installed {current}){modified}",
                check["candidate"]["versionNumber"]
                    .as_str()
                    .unwrap_or("A new version")
            ),
            true,
        ),
        "upToDate" => (format!("{current} is the newest version{modified}"), false),
        "blocked" => (
            check["blockedReason"]
                .as_str()
                .unwrap_or("This modpack cannot be updated right now.")
                .to_owned(),
            false,
        ),
        "currentVersionUnknown" => (
            "Modrinth no longer recognizes the installed pack version.".into(),
            false,
        ),
        _ => (
            "The modpack update check could not reach Modrinth.".into(),
            false,
        ),
    }
}

fn pack_review() {
    let Some((id, name)) = ui::with(|w, s| {
        let workspace = s.workspace.as_ref()?;
        let name = s.instance(&workspace.id)?.pack.as_ref()?.name.clone();
        w.global::<Instances>().set_pack_update_busy(true);
        Some((workspace.id.clone(), name))
    })
    .flatten() else {
        return;
    };
    let instance = id.clone();
    ui::spawn(api::preview_modpack_update(ui::host(), id), move |result| {
        ui::with(|w, _| w.global::<Instances>().set_pack_update_busy(false));
        match result {
            Ok(plan) => {
                let (text, conflicts) = pack_plan_text(&plan);
                let fingerprint = plan["fingerprint"].as_str().unwrap_or_default().to_owned();
                ui::with(|w, _| {
                    let instances = w.global::<Instances>();
                    instances.set_pack_review_text(text.into());
                    instances.set_pack_review_conflicts(conflicts.len() as i32);
                    instances.set_pack_keep_local(false);
                    instances.set_pack_review_allowed(!fingerprint.is_empty());
                    instances.set_pack_review_open(true);
                });
                PACK.with(|p| {
                    *p.borrow_mut() = Some(PackReview {
                        instance,
                        name,
                        fingerprint,
                        conflicts,
                    })
                });
            }
            Err(error) => ui::error_toast(&error),
        }
    });
}

/// Review text and the conflicting paths the user must resolve.
pub fn pack_plan_text(plan: &Value) -> (String, Vec<String>) {
    let s = |key: &str| plan[key].as_str().unwrap_or("").to_owned();
    let n = |key: &str| plan["counts"][key].as_u64().unwrap_or(0);
    let mut text = format!(
        "{} {} → {}\n",
        s("name"),
        s("currentPackVersion"),
        s("candidatePackVersion")
    );
    if plan["gameTransition"] == true {
        text.push_str(&format!(
            "Minecraft {} → {} · loader {} → {}. The game is reinstalled for the new version.\n",
            s("minecraftCurrent"),
            s("minecraftCandidate"),
            s("loaderCurrent"),
            s("loaderCandidate")
        ));
    } else {
        text.push_str(&format!(
            "Minecraft {} · loader {} (unchanged)\n",
            s("minecraftCurrent"),
            s("loaderCurrent")
        ));
    }
    text.push_str(&format!(
        "\n{} added · {} updated · {} removed · {} kept as is\n",
        n("added"),
        n("updated"),
        n("removed"),
        n("preserved")
    ));
    let rows = plan["rows"].as_array().cloned().unwrap_or_default();
    let conflicts: Vec<String> = rows
        .iter()
        .filter(|row| row["action"] == "conflict")
        .filter_map(|row| row["path"].as_str().map(str::to_owned))
        .collect();
    if !conflicts.is_empty() {
        text.push_str("\nYou changed these files and the new version changes them too. Your copies are kept; the pack's new copy is not written over them.\n");
        for path in &conflicts {
            text.push_str(&format!("{path}\n"));
        }
    }
    let unselected = plan["newOptionalUnselected"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !unselected.is_empty() {
        text.push_str("\nNew optional files (not installed)\n");
        for path in unselected {
            text.push_str(&format!("{}\n", path.as_str().unwrap_or("")));
        }
    }
    if let Some(changelog) = plan["changelog"].as_str().filter(|c| !c.trim().is_empty()) {
        let short: String = changelog.chars().take(1200).collect();
        text.push_str(&format!("\nChangelog\n{short}\n"));
    }
    (text, conflicts)
}

fn pack_confirm() {
    let keep = ui::with(|w, _| {
        let instances = w.global::<Instances>();
        instances.get_pack_review_conflicts() == 0 || instances.get_pack_keep_local()
    })
    .unwrap_or(false);
    if !keep {
        return;
    }
    let Some(review) = PACK.with(|p| p.borrow_mut().take()) else {
        return;
    };
    ui::with(|w, _| {
        let instances = w.global::<Instances>();
        instances.set_pack_review_open(false);
        instances.set_pack_update_available(false);
        instances.set_pack_update_text("".into());
    });
    // Keeping local copies is the only resolution offered: it is allowed for
    // every conflict kind and never deletes or overwrites user bytes.
    let resolutions: Vec<(String, String)> = review
        .conflicts
        .iter()
        .map(|path| (path.clone(), "keepLocal".to_owned()))
        .collect();
    let PackReview {
        instance,
        name,
        fingerprint,
        ..
    } = review;
    let key = instance.clone();
    crate::activity::submit(
        format!("Update {name}"),
        "Modpack update".into(),
        Some(key),
        format!("pack-update:{instance}"),
        move || {
            Box::pin(api::apply_modpack_update(
                ui::host(),
                instance.clone(),
                fingerprint.clone(),
                resolutions.clone(),
            ))
        },
    );
    ui::toast(format!("Update {name} added to Downloads."), Tone::Neutral);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(project: &str, status: &str, pinned: bool, candidate: Option<&str>) -> UpdateEntry {
        UpdateEntry {
            content_type: "mod".into(),
            project_id: project.into(),
            current_version: "v1".into(),
            status: status.into(),
            block: None,
            candidate: candidate.map(|version| api::UpdateCandidate {
                id: format!("{project}-{version}"),
                version_number: version.into(),
                version_type: "release".into(),
            }),
            pinned,
            detail: None,
        }
    }

    #[test]
    fn only_unpinned_compatible_candidates_are_updatable() {
        assert!(eligible(&entry("a", "updateAvailable", false, Some("2.0"))));
        assert!(!eligible(&entry("a", "updateAvailable", true, Some("2.0"))));
        assert!(!eligible(&entry(
            "a",
            "pinnedUpdateAvailable",
            true,
            Some("2.0")
        )));
        assert!(!eligible(&entry("a", "upToDate", false, None)));
        assert!(!eligible(&entry("a", "blocked", false, Some("2.0"))));
        assert!(!eligible(&entry("a", "noNewerUnderPolicy", false, None)));
        assert!(!eligible(&entry("a", "updateAvailable", false, None)));
    }

    #[test]
    fn summary_counts_each_state_honestly() {
        let entries = vec![
            entry("a", "updateAvailable", false, Some("2")),
            entry("b", "updateAvailable", false, Some("3")),
            entry("c", "pinnedUpdateAvailable", true, Some("4")),
            entry("d", "upToDate", false, None),
            entry("e", "blocked", false, None),
            entry("f", "providerUnavailable", false, None),
        ];
        let (text, count) = summarize(&entries, 2);
        assert_eq!(count, 2);
        assert_eq!(
            text,
            "2 updates available · 1 pinned · 1 up to date · 1 need attention · 1 could not be checked · 2 dependencies follow their parents"
        );
        assert_eq!(summarize(&[], 0).1, 0);
    }

    #[test]
    fn rows_offer_actions_only_for_their_own_state() {
        let mut updates = Updates {
            tab: 0,
            checked: true,
            ..Default::default()
        };
        for e in [
            entry("up", "updateAvailable", false, Some("2.0")),
            entry("pin", "pinnedUpdateAvailable", true, Some("3.0")),
            entry("cur", "upToDate", false, None),
        ] {
            updates.entries.insert(e.project_id.clone(), e);
        }
        let mut row = ContentRow::default();
        decorate(&mut row, Some("up"), &updates);
        assert!(row.can_update && !row.can_unpin);
        assert_eq!(row.update_text, "Update: 2.0");
        decorate(&mut row, Some("pin"), &updates);
        assert!(!row.can_update && row.can_unpin);
        decorate(&mut row, Some("cur"), &updates);
        assert!(!row.can_update && !row.can_unpin);
        assert_eq!(row.update_text, "Up to date");
        decorate(&mut row, Some("dependency"), &updates);
        assert_eq!(row.update_text, "Updated with its parent");
        decorate(&mut row, None, &updates);
        assert_eq!(row.update_text, "");
        decorate(&mut row, Some("up"), &Updates::default());
        assert!(!row.can_update && row.update_text.is_empty());
    }

    #[test]
    fn a_plain_replacement_needs_no_review_but_dependency_changes_do() {
        let plain = json!({
            "targets": [{"current": {"fileName": "a.jar", "displayVersion": "1.0"}, "candidate": {"versionNumber": "1.1"}}],
            "delta": {"newRequirements": [], "removedRequirements": []}, "warnings": [], "previewFingerprint": "f"
        });
        let (review, text) = describe(&plain);
        assert!(!review);
        assert!(text.contains("a.jar (1.0) → 1.1"));
        let mut added = plain.clone();
        added["delta"]["newRequirements"] = json!([{"fileName": "lib.jar"}]);
        assert!(describe(&added).0);
        let mut removed = plain.clone();
        removed["delta"]["removedRequirements"] = json!([{"fileName": "old.jar"}]);
        assert!(describe(&removed).0);
        let mut warned = plain;
        warned["warnings"] = json!(["Changes the loader requirement"]);
        assert!(describe(&warned).0);
    }

    #[test]
    fn pack_review_lists_conflicts_and_game_transitions() {
        let plan = json!({
            "name": "Pack", "currentPackVersion": "1", "candidatePackVersion": "2", "gameTransition": true,
            "minecraftCurrent": "1.21.1", "minecraftCandidate": "1.21.4", "loaderCurrent": "0.16.0", "loaderCandidate": "0.16.9",
            "counts": {"added": 1, "updated": 2, "removed": 0, "preserved": 5, "conflicts": 1},
            "rows": [{"path": "config/a.json", "action": "conflict"}, {"path": "mods/b.jar", "action": "replace"}],
            "newOptionalUnselected": [], "fingerprint": "x"
        });
        let (text, conflicts) = pack_plan_text(&plan);
        assert_eq!(conflicts, ["config/a.json"]);
        assert!(text.contains("Minecraft 1.21.1 → 1.21.4"));
        assert!(text.contains("1 added · 2 updated · 0 removed · 5 kept as is"));
        let check = json!({"status": "updateAvailable", "currentPackVersion": "1", "candidate": {"versionNumber": "2"}, "divergences": 0});
        assert_eq!(
            pack_check_text(&check),
            ("2 available (installed 1)".to_owned(), true)
        );
        assert!(!pack_check_text(&json!({"status": "upToDate", "currentPackVersion": "1"})).1);
    }
}
