//! Instances: the list, selection, creation, and one instance workspace
//! (configuration, health, and installed content). Every mutation goes
//! through the validated application services; the UI keeps only drafts.

use slint::ComponentHandle;

use crate::api::{self, InstanceConfiguration, InstanceSummary, LauncherState, WindowSize};
use crate::ui;
use crate::{AppWindow, ContentRow, Instances, Nav, Page, Tone};

/// The open instance workspace and its editable drafts.
#[derive(Default, Clone)]
pub struct Workspace {
    pub id: String,
    pub base: Option<InstanceConfiguration>,
    pub content: Vec<ContentItem>,
    pub content_serial: u64,
    pub filter: String,
    pub pending_remove: Option<(String, String)>,
    /// The last explicit update check of the current content tab.
    pub updates: crate::content_updates::Updates,
    /// Option values behind the Game settings selectors, in display order.
    /// Loader versions start with "" (automatic) then exact versions.
    pub cfg_versions: Vec<String>,
    pub cfg_kinds: Vec<String>,
    pub cfg_loader_versions: Vec<String>,
}

#[derive(Clone)]
pub struct ContentItem {
    pub row: ContentRow,
    /// Modrinth project id when the file is provider-managed.
    pub project: Option<String>,
}

/// Workspace tabs. Content tabs come first and their index equals the
/// content kind index; Settings is always last.
pub const TAB_MODS: i32 = 0;
pub const TAB_SETTINGS: i32 = 3;
pub const CONTENT_KINDS: [&str; 3] = ["mod", "resourcePack", "shaderPack"];

/// The same content-status wording every surface shows (instanceStatus.ts).
pub fn content_status(instance: &InstanceSummary) -> (String, Tone) {
    if let Some(recovery) = &instance.recovery {
        if recovery.status != "completed" || instance.state != "ready" {
            let label = match recovery.status.as_str() {
                "checking" => "Checking recovery…",
                "busy" => "Installation busy",
                "interrupted" => "Installation interrupted",
                "previousRestored" => "Previous installation restored",
                "manualRepair" => "Recovery required",
                _ => "Recovery completed",
            };
            let tone = match recovery.status.as_str() {
                "manualRepair" => Tone::Error,
                "checking" | "busy" => Tone::Working,
                _ => Tone::Warning,
            };
            return (label.into(), tone);
        }
    }
    if instance.state == "installing" {
        return ("Installing".into(), Tone::Working);
    }
    ("Installed".into(), Tone::Success)
}

pub fn bind(window: &AppWindow) {
    let instances = window.global::<Instances>();
    instances.on_open(|id| open_workspace(id.to_string(), TAB_MODS));
    instances.on_create(open_create);
    instances.on_close_workspace(|| {
        ui::with(|window, state| {
            state.workspace = None;
            window.global::<Nav>().set_workspace_open(false);
        });
    });
    instances.on_tab_changed(|tab| load_tab(tab));
    instances.on_play(|| {
        let id = ui::state(|s| s.workspace.as_ref().map(|w| w.id.clone()));
        if let Some(id) = id {
            select(id);
            ui::with(|window, _| window.global::<Nav>().invoke_navigate(Page::Home));
        }
    });
    instances.on_make_selected(|| {
        if let Some(id) = ui::state(|s| s.workspace.as_ref().map(|w| w.id.clone())) {
            select(id);
        }
    });
    instances.on_open_folder(|| {
        if let Some(id) = ui::state(|s| s.workspace.as_ref().map(|w| w.id.clone())) {
            ui::spawn(api::open_instance_folder(ui::host(), id), report_error);
        }
    });
    instances.on_config_edited(render_dirty);
    instances.on_revert_config(|| {
        ui::with(|window, state| {
            if let Some(base) = state.workspace.as_ref().and_then(|w| w.base.clone()) {
                write_draft(window, &base);
            }
        });
        seed_game_settings();
        render_dirty();
    });
    instances.on_save_config(save_configuration);
    instances.on_validate(validate);
    instances.on_repair(|| install_or_repair(false));
    instances.on_content_filter_changed(|text| {
        ui::with(|_, state| {
            if let Some(workspace) = state.workspace.as_mut() {
                workspace.filter = text.to_string();
            }
        });
        render_content();
    });
    instances.on_content_toggle(toggle_content);
    instances.on_content_remove(|id, title| {
        ui::with(|window, state| {
            let Some(workspace) = state.workspace.as_mut() else { return };
            workspace.pending_remove = Some((id.to_string(), title.to_string()));
            let instances = window.global::<Instances>();
            instances.set_remove_title(format!("Remove {title}?").into());
            instances.set_remove_detail(
                "The file is removed from this instance only. Content installed from Modrinth may also remove dependencies it alone required."
                    .into(),
            );
            instances.set_remove_open(true);
        });
    });
    instances.on_remove_confirm(remove_content);
    instances.on_content_open_folder(|| {
        let Some((id, tab)) = ui::with(|window, state| {
            state
                .workspace
                .as_ref()
                .map(|w| (w.id.clone(), window.global::<Instances>().get_tab()))
        })
        .flatten() else {
            return;
        };
        let host = ui::host();
        if tab == TAB_MODS {
            ui::spawn(api::open_mods_folder(host, id), report_error);
        } else if let Some(kind) = CONTENT_KINDS.get(tab as usize) {
            ui::spawn(api::open_content_folder(host, id, kind), report_error);
        }
    });
    instances.on_content_refresh(|| {
        let tab = ui::with(|window, _| window.global::<Instances>().get_tab()).unwrap_or(0);
        load_tab(tab);
    });
    instances.on_content_discover(|| {
        let Some((id, tab)) = ui::with(|window, state| {
            state
                .workspace
                .as_ref()
                .map(|w| (w.id.clone(), window.global::<Instances>().get_tab()))
        })
        .flatten() else {
            return;
        };
        crate::discover::open_for(id, tab.clamp(0, 2));
    });
    instances.on_rename_start(|| {
        let Some(current) = ui::with(|window, state| {
            let workspace = state.workspace.as_ref()?;
            let instance = state.instance(&workspace.id)?;
            let instances = window.global::<Instances>();
            instances.set_rename_text(instance.display_name.clone().into());
            instances.set_rename_error("".into());
            Some(state.icon_name(&instance.id).map(str::to_owned))
        })
        .flatten() else {
            return;
        };
        crate::icons::begin(crate::icons::Target::Identity, current);
        ui::with(|window, _| window.global::<Instances>().set_rename_open(true));
    });
    instances.on_rename_confirm(rename);
    instances.on_delete_start(|| {
        ui::with(|window, _| {
            let instances = window.global::<Instances>();
            instances.set_delete_text("".into());
            instances.set_delete_error("".into());
            instances.set_delete_open(true);
        });
    });
    instances.on_delete_confirm(delete_instance);
    instances.on_create_changed(refresh_aurora_compatibility);
    instances.on_create_platform_changed(|| {
        load_create_loader_versions();
        refresh_aurora_compatibility();
    });
    instances.on_snapshots_changed(|_| load_versions());
    instances.on_create_confirm(create);
    instances.on_cfg_platform_changed(cfg_platform_changed);
    instances.on_install_configuration(|| install_or_repair(true));
}

fn report_error(result: api::ApiResult<()>) {
    if let Err(error) = result {
        ui::error_toast(&error);
    }
}

/// Loads launcher state and re-renders every dependent surface.
pub fn reload(then: impl FnOnce() + 'static) {
    let token = ui::park(then);
    ui::spawn(api::launcher_state(ui::host()), move |result| {
        match result {
            Ok(state) => apply_state(state),
            Err(error) => ui::error_toast(&error),
        }
        ui::resume(token);
    });
}

pub fn apply_state(state: LauncherState) {
    ui::with(|_, app| app.launcher = Some(state));
    render_list();
    crate::home::render();
    render_workspace_header();
    crate::discover::render_targets();
    crate::widgets::refresh();
}

pub fn render_list() {
    ui::with(|window, state| {
        window
            .global::<Instances>()
            .set_rows(ui::model(crate::home::instance_rows(state)));
    });
}

/// Persists the Home selection, then re-reads state and readiness.
pub fn select(id: String) {
    ui::spawn(api::select_instance(ui::host(), id), |result| {
        if let Err(error) = result {
            ui::error_toast(&error);
        }
        reload(crate::home::refresh_readiness);
    });
}

pub fn open_workspace(id: String, tab: i32) {
    let found = ui::with(|window, state| {
        let instance = state.instance(&id)?.clone();
        state.workspace = Some(Workspace {
            id: id.clone(),
            base: Some(instance.configuration.clone()),
            ..Default::default()
        });
        let instances = window.global::<Instances>();
        write_draft(window, &instance.configuration);
        instances.set_pack_update_text("".into());
        instances.set_pack_update_available(false);
        instances.set_pack_review_open(false);
        instances.set_tab(tab);
        instances.set_config_error("".into());
        instances.set_health_text("Not checked yet.".into());
        instances.set_health_tone(Tone::Neutral);
        instances.set_can_repair(instance.state == "installing");
        instances.set_runtime_text("".into());
        instances.set_content_filter("".into());
        let nav = window.global::<Nav>();
        nav.set_page(Page::Instances);
        nav.set_workspace_open(true);
        Some(())
    })
    .flatten();
    if found.is_none() {
        return;
    }
    seed_game_settings();
    render_workspace_header();
    render_dirty();
    load_memory_hint(id);
    load_tab(tab);
}

fn render_workspace_header() {
    ui::with(|window, state| {
        let Some(workspace) = &state.workspace else {
            return;
        };
        let instances = window.global::<Instances>();
        let Some(instance) = state.instance(&workspace.id) else {
            // The instance disappeared from the registry: close honestly.
            window.global::<Nav>().set_workspace_open(false);
            return;
        };
        let (status, tone) = content_status(instance);
        instances.set_id(instance.id.clone().into());
        instances.set_name(instance.display_name.clone().into());
        instances.set_icon(state.icon_path(&instance.id).into());
        instances.set_icon_label(
            crate::symbols::label(
                state
                    .icon_name(&instance.id)
                    .unwrap_or(crate::symbols::FALLBACK_ICON),
            )
            .into(),
        );
        instances.set_subtitle(instance.subtitle().into());
        instances.set_letter(ui::letter(&instance.display_name).into());
        instances.set_status(status.into());
        instances.set_tone(tone);
        instances.set_installing(instance.state == "installing");
        instances.set_cfg_needs_install(needs_install(instance));
        instances.set_pack_name(
            instance
                .pack
                .as_ref()
                .map(|pack| format!("{} {}", pack.name, pack.pack_version))
                .unwrap_or_default()
                .into(),
        );
        let selected = state
            .launcher
            .as_ref()
            .and_then(|l| l.config.selected_instance_id.as_deref())
            == Some(instance.id.as_str());
        instances.set_selected(selected);
    });
}

fn write_draft(window: &AppWindow, configuration: &InstanceConfiguration) {
    let instances = window.global::<Instances>();
    instances.set_memory_gib(configuration.memory_mib as f32 / 1024.0);
    instances.set_jvm_args(configuration.additional_jvm_arguments.clone().into());
    instances.set_custom_window(configuration.window.is_some());
    let (width, height) = configuration
        .window
        .map(|w| (w.width, w.height))
        .unwrap_or((1280, 720));
    instances.set_window_width(width.to_string().into());
    instances.set_window_height(height.to_string().into());
}

/// Builds the edited configuration from the drafts. Unrendered fields
/// (loader, version, Aurora, unknown future fields) are carried unchanged.
fn read_draft(
    window: &AppWindow,
    base: &InstanceConfiguration,
    workspace: &Workspace,
) -> Result<InstanceConfiguration, String> {
    let instances = window.global::<Instances>();
    let mut draft = base.clone();
    let (version, loader) = draft_platform(window, workspace, base);
    draft.minecraft_version = version;
    draft.loader = loader;
    draft.memory_mib = ((instances.get_memory_gib() * 2.0).round() / 2.0 * 1024.0) as u32;
    draft.additional_jvm_arguments = instances.get_jvm_args().to_string();
    draft.window = if instances.get_custom_window() {
        let parse = |text: &str, what: &str| {
            text.trim()
                .parse::<u32>()
                .ok()
                .filter(|value| (100..=7680).contains(value))
                .ok_or_else(|| {
                    format!("The window {what} must be a whole number from 100 to 7680.")
                })
        };
        Some(WindowSize {
            width: parse(&instances.get_window_width(), "width")?,
            height: parse(&instances.get_window_height(), "height")?,
        })
    } else {
        None
    };
    Ok(draft)
}

fn render_dirty() {
    ui::with(|window, state| {
        let Some(workspace) = state.workspace.as_ref() else {
            return;
        };
        let Some(base) = workspace.base.clone() else {
            return;
        };
        let instances = window.global::<Instances>();
        match read_draft(window, &base, workspace) {
            Ok(draft) => {
                instances.set_config_dirty(draft != base);
                instances.set_config_error("".into());
            }
            Err(message) => {
                instances.set_config_dirty(true);
                instances.set_config_error(message.into());
            }
        }
    });
}

fn save_configuration() {
    let request = ui::with(|window, state| {
        let workspace = state.workspace.as_ref()?;
        let base = workspace.base.clone()?;
        let instances = window.global::<Instances>();
        match read_draft(window, &base, workspace) {
            Ok(draft) => {
                instances.set_config_busy(true);
                Some((workspace.id.clone(), base, draft))
            }
            Err(message) => {
                instances.set_config_error(message.into());
                None
            }
        }
    })
    .flatten();
    let Some((id, base, draft)) = request else {
        return;
    };
    ui::spawn(
        api::update_configuration(ui::host(), id, base, draft),
        |result| {
            ui::with(|window, state| {
                let instances = window.global::<Instances>();
                instances.set_config_busy(false);
                match result {
                    Ok(updated) => {
                        if let Some(workspace) = state.workspace.as_mut() {
                            workspace.base = Some(updated.configuration.clone());
                        }
                        write_draft(window, &updated.configuration);
                        instances.set_config_dirty(false);
                        ui::toast(
                            if needs_install(&updated) {
                                "Settings saved. Install them to apply the new game version or loader."
                            } else {
                                "Settings saved."
                            },
                            Tone::Success,
                        );
                    }
                    Err(error) => instances.set_config_error(error.message.into()),
                }
            });
            reload(|| {
                seed_game_settings();
                crate::home::refresh_readiness();
            });
        },
    );
}

fn load_memory_hint(id: String) {
    ui::spawn(api::performance_advice(ui::host(), id), |result| {
        ui::with(|window, _| {
            let instances = window.global::<Instances>();
            match result {
                Ok(advice) => {
                    let hint = match advice.recommended_mib {
                        Some(mib) => format!(
                            "Recommended: {:.1} GB. {}",
                            mib as f32 / 1024.0,
                            advice.explanation
                        ),
                        None => advice.explanation,
                    };
                    instances.set_memory_hint(hint.into());
                    if let Some(total) = advice.facts.total_mib {
                        instances.set_memory_max_gib(
                            ((total as f32 / 1024.0) * 0.75).floor().clamp(2.0, 64.0),
                        );
                    }
                }
                Err(_) => instances.set_memory_hint("".into()),
            }
        });
    });
}

fn validate() {
    let Some(id) = ui::state(|s| s.workspace.as_ref().map(|w| w.id.clone())) else {
        return;
    };
    ui::with(|window, _| window.global::<Instances>().set_health_busy(true));
    let host = ui::host();
    let runtime_host = host.clone();
    let runtime_id = id.clone();
    ui::spawn(
        async move {
            let validation = api::validate_instance(host, id).await;
            let runtime = api::runtime_status(runtime_host, runtime_id).await;
            (validation, runtime)
        },
        |(validation, runtime)| {
            ui::with(|window, state| {
                let instances = window.global::<Instances>();
                instances.set_health_busy(false);
                match validation {
                    Ok(report) if report.status == "valid" || report.status == "ready" => {
                        instances.set_health_text(
                            "All installed files match their verified records.".into(),
                        );
                        instances.set_health_tone(Tone::Success);
                        instances.set_can_repair(false);
                    }
                    Ok(report) => {
                        let detail = report
                            .problems
                            .iter()
                            .take(3)
                            .map(|p| format!("{}: {}", p.component, p.reason))
                            .collect::<Vec<_>>()
                            .join("; ");
                        instances.set_health_text(format!("Needs repair. {detail}").into());
                        instances.set_health_tone(Tone::Warning);
                        instances.set_can_repair(true);
                    }
                    Err(error) => {
                        instances.set_health_text(error.message.into());
                        instances.set_health_tone(Tone::Error);
                    }
                }
                let runtime_text = match runtime {
                    Ok(status) => format!(
                        "{} (Java {}) · {}",
                        status.component,
                        status.required_major_version,
                        match status.status.as_str() {
                            "ready" => "installed and verified".to_owned(),
                            "missing" =>
                                "not installed yet; installed automatically when needed".to_owned(),
                            other => other.to_owned(),
                        }
                    ),
                    Err(error) => error.message,
                };
                instances.set_runtime_text(runtime_text.into());
                let _ = state;
            });
        },
    );
}

/// `install`: apply saved settings that differ from the installed game.
/// Otherwise repair (or retry an interrupted install). Same Rust services.
fn install_or_repair(install: bool) {
    let Some((id, installing)) = ui::state(|s| {
        let workspace = s.workspace.as_ref()?;
        let instance = s.instance(&workspace.id)?;
        Some((instance.id.clone(), instance.state == "installing"))
    }) else {
        return;
    };
    ui::with(|window, _| window.global::<Instances>().set_health_busy(true));
    let host = ui::host();
    let work = async move {
        if installing {
            api::retry_install(host, id).await
        } else {
            api::install_configuration(host, id).await
        }
    };
    ui::spawn(work, move |result| {
        crate::home::clear_progress();
        ui::with(|window, _| window.global::<Instances>().set_health_busy(false));
        match result {
            Ok(_) => {
                ui::toast(
                    if install {
                        "Saved settings installed and verified."
                    } else {
                        "Installation repaired."
                    },
                    Tone::Success,
                );
                reload(|| {
                    validate();
                    seed_game_settings();
                    crate::home::refresh_readiness();
                });
            }
            Err(error) => {
                ui::error_toast(&error);
                reload(|| {});
            }
        }
    });
}

fn load_tab(tab: i32) {
    if !(TAB_MODS..TAB_SETTINGS).contains(&tab) {
        return;
    }
    let Some((id, serial)) = ui::with(|window, state| {
        let workspace = state.workspace.as_mut()?;
        workspace.content_serial += 1;
        workspace.content.clear();
        if workspace.updates.tab != tab {
            workspace.updates = crate::content_updates::Updates::default();
        }
        let instances = window.global::<Instances>();
        instances.set_content_loading(true);
        instances.set_content_error("".into());
        Some((workspace.id.clone(), workspace.content_serial))
    })
    .flatten() else {
        return;
    };
    render_content();
    let host = ui::host();
    if tab == TAB_MODS {
        ui::spawn(api::instance_mods(host, id), move |result| {
            apply_content(serial, result.map(mod_items))
        });
    } else {
        let kind = CONTENT_KINDS[tab as usize];
        ui::spawn(api::instance_content(host, id, kind), move |result| {
            apply_content(serial, result.map(pack_items))
        });
    }
}

fn mod_items(inventory: api::ModInventory) -> Vec<ContentItem> {
    inventory
        .entries
        .into_iter()
        .map(|entry| {
            let version = entry.metadata.as_ref().and_then(|m| m.version.clone());
            let authors = entry
                .metadata
                .as_ref()
                .map(|m| m.authors.join(", "))
                .unwrap_or_default();
            let mut detail = vec![];
            if let Some(version) = version {
                detail.push(version);
            }
            if !authors.is_empty() {
                detail.push(authors);
            }
            if entry.ownership == "launcherBootstrap"
                || entry.ownership.starts_with("launcherManaged")
            {
                detail.push("Managed by Aurora".into());
            }
            if detail.is_empty() {
                detail.push(entry.file_name.clone());
            }
            let note = entry
                .warnings
                .first()
                .map(|w| w.message.clone())
                .or_else(|| {
                    entry
                        .action_blocked_reason
                        .clone()
                        .filter(|_| !entry.can_toggle)
                })
                .or_else(|| {
                    entry
                        .removal_blocked_reason
                        .clone()
                        .filter(|_| !entry.can_remove && entry.can_toggle)
                })
                .unwrap_or_default();
            ContentItem {
                project: modrinth_project(entry.provenance.as_ref()),
                row: ContentRow {
                    id: entry.entry_id.into(),
                    title: entry.display_name.into(),
                    detail: detail.join(" · ").into(),
                    enabled: entry.enabled,
                    toggleable: entry.can_toggle,
                    removable: entry.can_remove,
                    note: note.into(),
                    ..Default::default()
                },
            }
        })
        .collect()
}

fn pack_items(inventory: api::ContentInventory) -> Vec<ContentItem> {
    inventory
        .entries
        .into_iter()
        .map(|entry| {
            let size = entry
                .size_bytes
                .map(|bytes| format!("{:.1} MB", bytes as f64 / 1_048_576.0))
                .unwrap_or_default();
            let mut detail = vec![entry.file_name.clone()];
            if !size.is_empty() {
                detail.push(size);
            }
            if entry.management.activation_managed_in_game {
                detail.push("Selected in game".into());
            }
            ContentItem {
                project: modrinth_project(entry.provenance.as_ref()),
                row: ContentRow {
                    id: entry.entry_id.into(),
                    title: entry.display_name.into(),
                    detail: detail.join(" · ").into(),
                    enabled: entry.management.active.unwrap_or(true),
                    toggleable: entry.management.can_toggle,
                    removable: entry.management.can_remove,
                    note: entry
                        .warnings
                        .first()
                        .map(|w| w.message.clone())
                        .or_else(|| {
                            entry
                                .management
                                .removal_blocked_reason
                                .clone()
                                .filter(|_| !entry.management.can_remove)
                        })
                        .or_else(|| {
                            entry.management.toggle_blocked_reason.clone().filter(|_| {
                                !entry.management.can_toggle
                                    && !entry.management.activation_managed_in_game
                            })
                        })
                        .unwrap_or_default()
                        .into(),
                    ..Default::default()
                },
            }
        })
        .collect()
}

fn modrinth_project(provenance: Option<&api::Provenance>) -> Option<String> {
    provenance
        .filter(|p| p.provider == "modrinth")
        .map(|p| p.project_id.clone())
}

fn apply_content(serial: u64, result: api::ApiResult<Vec<ContentItem>>) {
    ui::with(|window, state| {
        let Some(workspace) = state.workspace.as_mut() else {
            return;
        };
        if workspace.content_serial != serial {
            return;
        }
        let instances = window.global::<Instances>();
        instances.set_content_loading(false);
        match result {
            Ok(items) => workspace.content = items,
            Err(error) => instances.set_content_error(error.message.into()),
        }
    });
    render_content();
}

pub fn render_content() {
    ui::with(|window, state| {
        let instances = window.global::<Instances>();
        let tab = instances.get_tab();
        let empty = crate::content_updates::Updates::default();
        let updates = state
            .workspace
            .as_ref()
            .map(|w| &w.updates)
            .filter(|u| u.tab == tab)
            .unwrap_or(&empty);
        let rows: Vec<ContentRow> = state
            .workspace
            .as_ref()
            .map(|workspace| {
                let filter = workspace.filter.to_lowercase();
                workspace
                    .content
                    .iter()
                    .filter(|item| {
                        filter.is_empty()
                            || item.row.title.to_lowercase().contains(&filter)
                            || item.row.detail.to_lowercase().contains(&filter)
                    })
                    .map(|item| {
                        let mut row = item.row.clone();
                        crate::content_updates::decorate(
                            &mut row,
                            item.project.as_deref(),
                            updates,
                        );
                        row
                    })
                    .collect()
            })
            .unwrap_or_default();
        let count = updates
            .entries
            .values()
            .filter(|e| crate::content_updates::eligible(e))
            .count();
        instances.set_updates_summary(updates.summary.clone().into());
        instances.set_updates_count(count as i32);
        instances.set_content(ui::model(rows));
    });
}

/// After a queued install or update settles: reload the open content tab and
/// refresh update badges that described the superseded files.
pub fn after_content_change() {
    let tab = ui::with(|window, state| {
        state.workspace.as_ref()?;
        Some(window.global::<Instances>().get_tab())
    })
    .flatten();
    if let Some(tab) = tab.filter(|tab| (TAB_MODS..TAB_SETTINGS).contains(tab)) {
        load_tab(tab);
    }
    crate::content_updates::after_change();
}

fn toggle_content(entry: slint::SharedString, enabled: bool) {
    let Some((id, tab, serial)) = ui::with(|window, state| {
        let workspace = state.workspace.as_mut()?;
        workspace.content_serial += 1;
        Some((
            workspace.id.clone(),
            window.global::<Instances>().get_tab(),
            workspace.content_serial,
        ))
    })
    .flatten() else {
        return;
    };
    let host = ui::host();
    let entry = entry.to_string();
    if tab == TAB_MODS {
        ui::spawn(
            api::set_mod_enabled(host, id, entry, enabled),
            move |result| finish_mutation(serial, result.map(mod_items)),
        );
    } else if (1..TAB_SETTINGS).contains(&tab) {
        let kind = CONTENT_KINDS[tab as usize];
        ui::spawn(
            api::set_pack_enabled(host, id, kind, entry, enabled),
            move |result| finish_mutation(serial, result.map(pack_items)),
        );
    }
}

fn remove_content() {
    let Some((id, tab, entry, serial)) = ui::with(|window, state| {
        let instances = window.global::<Instances>();
        instances.set_remove_open(false);
        let workspace = state.workspace.as_mut()?;
        let (entry, _) = workspace.pending_remove.take()?;
        workspace.content_serial += 1;
        Some((
            workspace.id.clone(),
            instances.get_tab(),
            entry,
            workspace.content_serial,
        ))
    })
    .flatten() else {
        return;
    };
    let host = ui::host();
    if tab == TAB_MODS {
        ui::spawn(api::remove_mod(host, id, entry), move |result| {
            finish_mutation(serial, result.map(mod_items))
        });
    } else if (1..TAB_SETTINGS).contains(&tab) {
        let kind = CONTENT_KINDS[tab as usize];
        ui::spawn(api::remove_content(host, id, kind, entry), move |result| {
            finish_mutation(serial, result.map(pack_items))
        });
    }
}

/// Applies the authoritative inventory returned by a content mutation. On
/// failure the list is re-read so the UI never shows an optimistic state.
fn finish_mutation(serial: u64, result: api::ApiResult<Vec<ContentItem>>) {
    match result {
        Ok(items) => {
            apply_content(serial, Ok(items));
            crate::home::refresh_readiness();
        }
        Err(error) => {
            ui::error_toast(&error);
            let tab = ui::with(|window, _| window.global::<Instances>().get_tab()).unwrap_or(0);
            load_tab(tab);
        }
    }
}

/// Saves the identity dialog: the display name (only when it changed) and
/// then the icon (only when it changed). Each goes through its own
/// validated application service.
fn rename() {
    let Some((id, expected, name, current_icon)) = ui::with(|window, state| {
        let workspace = state.workspace.as_ref()?;
        let instance = state.instance(&workspace.id)?;
        let name = window
            .global::<Instances>()
            .get_rename_text()
            .trim()
            .to_owned();
        Some((
            instance.id.clone(),
            instance.display_name.clone(),
            name,
            state.icon_name(&instance.id).map(str::to_owned),
        ))
    })
    .flatten() else {
        return;
    };
    let icon = crate::icons::chosen();
    let save_icon = move |id: String| {
        if icon != current_icon {
            crate::icons::persist(id, icon, || {});
        }
    };
    if name.is_empty() {
        ui::with(|window, _| {
            window
                .global::<Instances>()
                .set_rename_error("Enter a name.".into())
        });
        return;
    }
    if name == expected {
        ui::with(|window, _| window.global::<Instances>().set_rename_open(false));
        save_icon(id);
        return;
    }
    let renamed = id.clone();
    ui::spawn(
        api::rename_instance(ui::host(), id, expected, name),
        move |result| match result {
            Ok(_) => {
                ui::with(|window, _| window.global::<Instances>().set_rename_open(false));
                reload(|| {});
                save_icon(renamed);
            }
            Err(error) => {
                ui::with(|window, _| {
                    window
                        .global::<Instances>()
                        .set_rename_error(error.message.into())
                });
            }
        },
    );
}

fn delete_instance() {
    let Some((id, typed)) = ui::with(|window, state| {
        let workspace = state.workspace.as_ref()?;
        let instances = window.global::<Instances>();
        instances.set_delete_busy(true);
        instances.set_delete_error("".into());
        Some((
            workspace.id.clone(),
            instances.get_delete_text().to_string(),
        ))
    })
    .flatten() else {
        return;
    };
    ui::spawn(api::delete_instance(ui::host(), id, typed), |result| {
        ui::with(|window, _| window.global::<Instances>().set_delete_busy(false));
        match result {
            Ok(()) => {
                ui::with(|window, state| {
                    window.global::<Instances>().set_delete_open(false);
                    state.workspace = None;
                    window.global::<Nav>().set_workspace_open(false);
                });
                ui::toast("Instance deleted.", Tone::Success);
                reload(crate::home::refresh_readiness);
            }
            Err(error) => {
                ui::with(|window, _| {
                    window
                        .global::<Instances>()
                        .set_delete_error(error.message.into())
                });
            }
        }
    });
}

// ------------------------------------------------------------ creation

thread_local! {
    static LOADER_KINDS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    static VERSIONS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    /// (request serial, option values): "" is the automatic policy.
    static CREATE_LOADER: std::cell::RefCell<(u64, Vec<String>)> = const { std::cell::RefCell::new((0, Vec::new())) };
    /// Official release catalog for the Game settings selector.
    static RELEASES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn open_create() {
    crate::icons::reset_create_choice();
    ui::with(|window, state| {
        let instances = window.global::<Instances>();
        instances.set_create_name(format!("Instance {}", state.instances().len() + 1).into());
        instances.set_create_error("".into());
        instances.set_create_progress("".into());
        instances.set_create_busy(false);
        // Platforms come from Rust's capability list, in its order.
        let kinds: Vec<String> = state
            .launcher
            .as_ref()
            .map(|l| {
                l.platform_capabilities
                    .iter()
                    .filter(|c| c.can_create)
                    .map(|c| c.kind.clone())
                    .collect()
            })
            .unwrap_or_else(|| vec!["fabric".into(), "vanilla".into()]);
        let labels: Vec<slint::SharedString> =
            kinds.iter().map(|k| platform_label(k).into()).collect();
        instances.set_loader_index(kinds.iter().position(|k| k == "fabric").unwrap_or(0) as i32);
        instances.set_loaders(ui::model(labels));
        LOADER_KINDS.with(|slot| *slot.borrow_mut() = kinds);
        instances.set_create_open(true);
    });
    let empty = VERSIONS.with(|versions| versions.borrow().is_empty());
    if empty {
        load_versions();
    } else {
        load_create_loader_versions();
        refresh_aurora_compatibility();
    }
}

fn platform_label(kind: &str) -> &'static str {
    match kind {
        "vanilla" => "Vanilla",
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoForge" => "NeoForge",
        "quilt" => "Quilt",
        _ => "Other",
    }
}

fn load_versions() {
    let include = ui::with(|window, _| {
        let instances = window.global::<Instances>();
        instances.set_versions_loading(true);
        instances.get_include_snapshots()
    })
    .unwrap_or(false);
    ui::spawn(
        async move {
            (
                api::minecraft_versions(include).await,
                api::aurora_releases().await,
            )
        },
        |(result, releases)| {
            ui::with(|window, _| {
                let instances = window.global::<Instances>();
                instances.set_versions_loading(false);
                match result {
                    Ok(versions) => {
                        let ids: Vec<String> = versions
                            .iter()
                            .map(|v| {
                                if v.version_type == "snapshot" {
                                    format!("{} (snapshot)", v.id)
                                } else {
                                    v.id.clone()
                                }
                            })
                            .collect();
                        let raw: Vec<String> = versions.into_iter().map(|v| v.id).collect();
                        // Default to the newest Aurora release's Minecraft version,
                        // like the WebView creation form.
                        let preferred = releases
                            .ok()
                            .and_then(|r| r.first().map(|r| r.minecraft_version.clone()))
                            .and_then(|version| raw.iter().position(|id| *id == version))
                            .unwrap_or(0);
                        VERSIONS.with(|slot| *slot.borrow_mut() = raw);
                        instances
                            .set_versions(ui::model(ids.into_iter().map(Into::into).collect()));
                        instances.set_version_index(preferred as i32);
                    }
                    Err(error) => instances.set_create_error(error.message.into()),
                }
            });
            load_create_loader_versions();
            refresh_aurora_compatibility();
        },
    );
}

fn creation_choice() -> Option<(String, serde_json::Value)> {
    ui::with(|window, _| {
        let instances = window.global::<Instances>();
        let version = VERSIONS.with(|v| {
            v.borrow()
                .get(instances.get_version_index() as usize)
                .cloned()
        })?;
        let kind = LOADER_KINDS.with(|k| {
            k.borrow()
                .get(instances.get_loader_index() as usize)
                .cloned()
        })?;
        let pinned = CREATE_LOADER.with(|slot| {
            slot.borrow()
                .1
                .get(instances.get_create_loader_index() as usize)
                .cloned()
        });
        Some((version, loader_json(&kind, pinned.as_deref())))
    })
    .flatten()
}

fn refresh_aurora_compatibility() {
    let Some((version, loader)) = creation_choice() else {
        return;
    };
    ui::spawn(api::aurora_compatibility(version, loader), |result| {
        ui::with(|window, _| {
            let instances = window.global::<Instances>();
            match result {
                Ok(compatibility) => {
                    instances.set_aurora_available(compatibility.available);
                    instances.set_create_aurora(compatibility.available);
                    let note = match (&compatibility.version, compatibility.available) {
                        (Some(version), true) => format!(
                            "Aurora {version} will be installed and kept with this instance."
                        ),
                        _ => compatibility.reason,
                    };
                    instances.set_aurora_note(note.into());
                }
                Err(error) => {
                    instances.set_aurora_available(false);
                    instances.set_create_aurora(false);
                    instances.set_aurora_note(error.message.into());
                }
            }
        });
    });
}

fn create() {
    let Some((version, loader)) = creation_choice() else {
        return;
    };
    let Some((name, aurora)) = ui::with(|window, _| {
        let instances = window.global::<Instances>();
        let name = instances.get_create_name().trim().to_owned();
        if name.is_empty() {
            instances.set_create_error("Enter a name for the instance.".into());
            return None;
        }
        instances.set_create_error("".into());
        instances.set_create_busy(true);
        instances.set_create_progress("Preparing…".into());
        instances.set_create_fraction(-1.0);
        Some((
            name,
            instances.get_create_aurora() && instances.get_aurora_available(),
        ))
    })
    .flatten() else {
        return;
    };
    ui::spawn(
        api::create_instance(ui::host(), name, version, loader, aurora),
        |result| {
            crate::home::clear_progress();
            ui::with(|window, _| window.global::<Instances>().set_create_busy(false));
            match result {
                Ok(instance) => {
                    ui::with(|window, _| window.global::<Instances>().set_create_open(false));
                    ui::toast(
                        format!("{} is installed.", instance.display_name),
                        Tone::Success,
                    );
                    let id = instance.id.clone();
                    if let Some(icon) = crate::icons::take_create_choice() {
                        crate::icons::persist(id.clone(), Some(icon), || {});
                    }
                    reload(move || {
                        // A first instance becomes the Home selection like the WebView UI.
                        let has_selection = ui::state(|s| s.selected_instance().is_some());
                        if !has_selection {
                            select(id);
                        } else {
                            crate::home::refresh_readiness();
                        }
                    });
                }
                Err(error) => {
                    ui::with(|window, _| {
                        window
                            .global::<Instances>()
                            .set_create_error(error.message.into())
                    });
                    // A failed creation leaves a retryable record; show it.
                    reload(|| {});
                }
            }
        },
    );
}

/// `instance-progress` / `install-progress` while the creation dialog is open.
pub fn on_create_progress(text: &str, fraction: f32) {
    ui::with(|window, _| {
        let instances = window.global::<Instances>();
        if instances.get_create_busy() {
            instances.set_create_progress(text.into());
            instances.set_create_fraction(fraction);
        }
    });
}

// ------------------------------------------------------------ loader versions

thread_local! {
    /// Latest Game-settings catalog request; older responses are ignored.
    static CFG_LOADER_SERIAL: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The persisted loader shape. Vanilla has no policy; no pin is automatic.
pub fn loader_json(kind: &str, pinned: Option<&str>) -> serde_json::Value {
    match (kind, pinned.filter(|version| !version.is_empty())) {
        ("vanilla", _) => serde_json::json!({ "kind": "vanilla" }),
        (kind, Some(version)) => {
            serde_json::json!({ "kind": kind, "policy": { "type": "pinned", "version": version } })
        }
        (kind, None) => serde_json::json!({ "kind": kind, "policy": { "type": "automatic" } }),
    }
}

fn loader_kind(loader: &serde_json::Value) -> String {
    loader["kind"].as_str().unwrap_or("vanilla").to_owned()
}

fn loader_pin(loader: &serde_json::Value) -> Option<String> {
    if loader["policy"]["type"] == "pinned" {
        loader["policy"]["version"].as_str().map(str::to_owned)
    } else {
        None
    }
}

fn loader_version_label(kind: &str) -> &'static str {
    match kind {
        "forge" => "Forge version",
        "neoForge" => "NeoForge version",
        "quilt" => "Quilt Loader version",
        _ => "Fabric Loader version",
    }
}

/// Option values and labels: automatic first, a saved pin missing from the
/// catalog kept visible, then the official versions newest first.
pub fn loader_options(
    versions: &[api::LoaderVersion],
    keep: Option<&str>,
    aurora: bool,
) -> (Vec<String>, Vec<slint::SharedString>) {
    let mut values = vec![String::new()];
    let mut labels: Vec<slint::SharedString> = vec![
        if aurora {
            "Aurora release version (automatic)"
        } else {
            "Automatic compatible version (recommended)"
        }
        .into(),
    ];
    if let Some(keep) = keep.filter(|k| !k.is_empty() && !versions.iter().any(|v| v.version == *k))
    {
        values.push(keep.to_owned());
        labels.push(format!("{keep} (saved)").into());
    }
    for version in versions {
        values.push(version.version.clone());
        labels.push(if version.stable {
            version.version.clone().into()
        } else {
            format!("{} (unstable)", version.version).into()
        });
    }
    (values, labels)
}

fn selected_index(values: &[String], pin: Option<&str>) -> i32 {
    pin.and_then(|pin| values.iter().position(|v| v == pin))
        .unwrap_or(0) as i32
}

/// The saved configuration differs from the installed game (instanceStatus.ts).
pub fn needs_install(instance: &InstanceSummary) -> bool {
    let configuration = &instance.configuration;
    instance.state == "ready"
        && (configuration.minecraft_version != instance.minecraft_version
            || loader_kind(&configuration.loader) != instance.platform.kind
            || configuration.aurora_enabled != instance.aurora.is_some()
            || loader_pin(&configuration.loader)
                .is_some_and(|pin| Some(&pin) != instance.platform.version.as_ref()))
}

fn load_create_loader_versions() {
    let Some((version, kind, serial)) = ui::with(|window, _| {
        let instances = window.global::<Instances>();
        let version = VERSIONS.with(|v| {
            v.borrow()
                .get(instances.get_version_index() as usize)
                .cloned()
        })?;
        let kind = LOADER_KINDS.with(|k| {
            k.borrow()
                .get(instances.get_loader_index() as usize)
                .cloned()
        })?;
        let has = kind != "vanilla";
        instances.set_create_has_loader(has);
        instances.set_create_loader_label(loader_version_label(&kind).into());
        instances.set_create_loader_note("".into());
        let (values, labels) = loader_options(&[], None, false);
        let serial = CREATE_LOADER.with(|slot| {
            let mut slot = slot.borrow_mut();
            slot.0 += 1;
            slot.1 = values;
            slot.0
        });
        instances.set_create_loader_versions(ui::model(labels));
        instances.set_create_loader_index(0);
        instances.set_create_loader_loading(has);
        Some((version, kind, serial))
    })
    .flatten() else {
        return;
    };
    if kind == "vanilla" {
        return;
    }
    let label = loader_version_label(&kind);
    let minecraft = version.clone();
    ui::spawn(api::loader_versions(kind, version), move |result| {
        ui::with(|window, _| {
            if CREATE_LOADER.with(|slot| slot.borrow().0) != serial {
                return;
            }
            let instances = window.global::<Instances>();
            instances.set_create_loader_loading(false);
            match result {
                Ok(versions) => {
                    if versions.is_empty() {
                        instances.set_create_loader_note(
                            format!(
                                "No {label} supports Minecraft {minecraft}. Choose another version or loader."
                            )
                            .into(),
                        );
                    }
                    let (values, labels) = loader_options(&versions, None, false);
                    CREATE_LOADER.with(|slot| slot.borrow_mut().1 = values);
                    instances.set_create_loader_versions(ui::model(labels));
                }
                Err(error) => instances.set_create_loader_note(
                    format!(
                        "Exact versions are unavailable ({}). The automatic version is resolved during installation.",
                        error.message
                    )
                    .into(),
                ),
            }
        });
    });
}

/// Seeds the Game selectors from the saved configuration and loads catalogs.
fn seed_game_settings() {
    let request = ui::with(|window, state| {
        let base = state.workspace.as_ref()?.base.clone()?;
        let kind = loader_kind(&base.loader);
        let mut kinds: Vec<String> = state
            .launcher
            .as_ref()
            .map(|l| {
                l.platform_capabilities
                    .iter()
                    .filter(|c| c.can_install || c.can_create)
                    .map(|c| c.kind.clone())
                    .collect()
            })
            .unwrap_or_default();
        if !kinds.contains(&kind) {
            kinds.push(kind.clone());
        }
        let instances = window.global::<Instances>();
        instances.set_cfg_loaders(ui::model(
            kinds.iter().map(|k| platform_label(k).into()).collect(),
        ));
        instances.set_cfg_loader_index(
            kinds
                .iter()
                .position(|k| *k == kind)
                .map(|i| i as i32)
                .unwrap_or(-1),
        );
        instances.set_cfg_loader_kind_enabled(!base.aurora_enabled);
        instances.set_cfg_loader_kind_note(
            if base.aurora_enabled {
                "Aurora Client requires Fabric. Manage Aurora content in Mods."
            } else {
                "Changing the loader requires installing new content."
            }
            .into(),
        );
        let workspace = state.workspace.as_mut()?;
        workspace.cfg_kinds = kinds;
        workspace.cfg_versions.clear();
        Some((
            base.minecraft_version.clone(),
            kind,
            loader_pin(&base.loader),
            base.aurora_enabled,
        ))
    })
    .flatten();
    let Some((version, kind, pin, aurora)) = request else {
        return;
    };
    refresh_version_list(Some(version.clone()));
    load_cfg_loader_versions(version, kind, pin, aurora);
    if RELEASES.with(|r| r.borrow().is_empty()) {
        ui::spawn(api::minecraft_versions(false), |result| {
            if let Ok(versions) = result {
                RELEASES.with(|r| *r.borrow_mut() = versions.into_iter().map(|v| v.id).collect());
                refresh_version_list(None);
            }
        });
    }
}

/// Rebuilds the version list from the release catalog, keeping the saved
/// version and the current draft choice (`select` overrides it) visible.
fn refresh_version_list(select: Option<String>) {
    ui::with(|window, state| {
        let Some(workspace) = state.workspace.as_mut() else {
            return;
        };
        let Some(base) = workspace.base.clone() else {
            return;
        };
        let instances = window.global::<Instances>();
        let current = select
            .or_else(|| {
                workspace
                    .cfg_versions
                    .get(instances.get_cfg_version_index() as usize)
                    .cloned()
            })
            .unwrap_or_else(|| base.minecraft_version.clone());
        let catalog = RELEASES.with(|r| r.borrow().clone());
        let mut versions = catalog.clone();
        for keep in [&current, &base.minecraft_version] {
            if !versions.contains(keep) {
                versions.insert(0, keep.clone());
            }
        }
        let labels = versions
            .iter()
            .map(|v| {
                if !catalog.is_empty() && !catalog.contains(v) {
                    format!("{v} (saved)").into()
                } else {
                    v.clone().into()
                }
            })
            .collect();
        instances.set_cfg_versions(ui::model(labels));
        instances.set_cfg_version_index(
            versions
                .iter()
                .position(|v| *v == current)
                .map(|i| i as i32)
                .unwrap_or(0),
        );
        workspace.cfg_versions = versions;
    });
}

fn cfg_platform_changed() {
    let request = ui::with(|window, state| {
        let workspace = state.workspace.as_ref()?;
        let base = workspace.base.as_ref()?;
        let instances = window.global::<Instances>();
        let version = workspace
            .cfg_versions
            .get(instances.get_cfg_version_index() as usize)?
            .clone();
        let kind = workspace
            .cfg_kinds
            .get(instances.get_cfg_loader_index() as usize)?
            .clone();
        // A saved pin applies only to the saved version and loader.
        let pin = (version == base.minecraft_version && kind == loader_kind(&base.loader))
            .then(|| loader_pin(&base.loader))
            .flatten();
        Some((version, kind, pin, base.aurora_enabled))
    })
    .flatten();
    if let Some((version, kind, pin, aurora)) = request {
        load_cfg_loader_versions(version, kind, pin, aurora);
    }
    render_dirty();
}

fn load_cfg_loader_versions(version: String, kind: String, pin: Option<String>, aurora: bool) {
    let serial = CFG_LOADER_SERIAL.with(|s| {
        s.set(s.get() + 1);
        s.get()
    });
    let started = ui::with(|window, state| {
        let workspace = state.workspace.as_mut()?;
        let instances = window.global::<Instances>();
        let has = kind != "vanilla";
        instances.set_cfg_has_loader(has);
        instances.set_cfg_loader_label(loader_version_label(&kind).into());
        instances.set_cfg_loader_note(
            if aurora {
                "Automatic uses the exact loader the Aurora release requires."
            } else {
                "Automatic resolves a compatible loader when installed; an exact version stays pinned."
            }
            .into(),
        );
        let (values, labels) = loader_options(&[], pin.as_deref(), aurora);
        instances.set_cfg_loader_versions(ui::model(labels));
        instances.set_cfg_loader_version_index(selected_index(&values, pin.as_deref()));
        workspace.cfg_loader_versions = values;
        instances.set_cfg_loader_loading(has);
        Some(())
    })
    .flatten();
    if started.is_none() || kind == "vanilla" {
        return;
    }
    let label = loader_version_label(&kind);
    let minecraft = version.clone();
    ui::spawn(api::loader_versions(kind, version), move |result| {
        ui::with(|window, state| {
            if CFG_LOADER_SERIAL.with(|s| s.get()) != serial {
                return;
            }
            let Some(workspace) = state.workspace.as_mut() else {
                return;
            };
            let instances = window.global::<Instances>();
            instances.set_cfg_loader_loading(false);
            let current = workspace
                .cfg_loader_versions
                .get(instances.get_cfg_loader_version_index() as usize)
                .cloned()
                .filter(|v| !v.is_empty())
                .or(pin);
            match result {
                Ok(versions) => {
                    if versions.is_empty() {
                        instances.set_cfg_loader_note(
                            format!("No {label} supports Minecraft {minecraft}.").into(),
                        );
                    }
                    let (values, labels) = loader_options(&versions, current.as_deref(), aurora);
                    instances.set_cfg_loader_versions(ui::model(labels));
                    instances
                        .set_cfg_loader_version_index(selected_index(&values, current.as_deref()));
                    workspace.cfg_loader_versions = values;
                }
                Err(error) => instances.set_cfg_loader_note(
                    format!("Exact versions are unavailable: {}", error.message).into(),
                ),
            }
        });
        render_dirty();
    });
}

/// Minecraft version and loader as edited, keeping the exact persisted JSON
/// when the choice is unchanged. Unloaded selectors keep the saved values.
fn draft_platform(
    window: &AppWindow,
    workspace: &Workspace,
    base: &InstanceConfiguration,
) -> (String, serde_json::Value) {
    let instances = window.global::<Instances>();
    let version = workspace
        .cfg_versions
        .get(instances.get_cfg_version_index() as usize)
        .cloned()
        .unwrap_or_else(|| base.minecraft_version.clone());
    let kind = workspace
        .cfg_kinds
        .get(instances.get_cfg_loader_index() as usize)
        .cloned()
        .unwrap_or_else(|| loader_kind(&base.loader));
    let pin = match workspace
        .cfg_loader_versions
        .get(instances.get_cfg_loader_version_index() as usize)
    {
        Some(value) => Some(value.clone()).filter(|v| !v.is_empty()),
        None => loader_pin(&base.loader).filter(|_| kind == loader_kind(&base.loader)),
    };
    let loader = loader_json(&kind, pin.as_deref());
    let unchanged = loader_kind(&loader) == loader_kind(&base.loader)
        && loader_pin(&loader) == loader_pin(&base.loader);
    (
        version,
        if unchanged {
            base.loader.clone()
        } else {
            loader
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_shapes_match_the_persisted_schema() {
        assert_eq!(
            loader_json("vanilla", Some("1")),
            serde_json::json!({"kind": "vanilla"})
        );
        assert_eq!(
            loader_json("fabric", None),
            serde_json::json!({"kind": "fabric", "policy": {"type": "automatic"}})
        );
        assert_eq!(
            loader_json("fabric", Some("")),
            serde_json::json!({"kind": "fabric", "policy": {"type": "automatic"}})
        );
        let pinned = loader_json("neoForge", Some("21.1.200"));
        assert_eq!(loader_pin(&pinned).as_deref(), Some("21.1.200"));
        assert_eq!(loader_kind(&pinned), "neoForge");
    }

    #[test]
    fn options_keep_automatic_first_and_never_drop_a_saved_pin() {
        let catalog = vec![
            api::LoaderVersion {
                version: "0.17.3".into(),
                stable: true,
            },
            api::LoaderVersion {
                version: "0.17.4-beta".into(),
                stable: false,
            },
        ];
        let (values, labels) = loader_options(&catalog, Some("0.16.0"), false);
        assert_eq!(values, ["", "0.16.0", "0.17.3", "0.17.4-beta"]);
        assert_eq!(labels[0], "Automatic compatible version (recommended)");
        assert_eq!(labels[1], "0.16.0 (saved)");
        assert_eq!(labels[3], "0.17.4-beta (unstable)");
        assert_eq!(selected_index(&values, Some("0.17.3")), 2);
        assert_eq!(selected_index(&values, None), 0);
        let (values, _) = loader_options(&catalog, Some("0.17.3"), true);
        assert_eq!(values, ["", "0.17.3", "0.17.4-beta"]);
        // Network failure: the saved pin stays selectable and selected.
        let (values, _) = loader_options(&[], Some("0.16.0"), false);
        assert_eq!(selected_index(&values, Some("0.16.0")), 1);
    }
}
