//! Instances: the list, selection, creation, and one instance workspace
//! (configuration, health, and installed content). Every mutation goes
//! through the validated application services; the UI keeps only drafts.

use slint::{ComponentHandle, Model};

use crate::api::{self, InstanceConfiguration, InstanceSummary, LauncherState, WindowSize};
use crate::ui;
use crate::{AppWindow, ContentRow, Instances, Nav, Page, Tone};

/// The open instance workspace and its editable drafts.
#[derive(Default, Clone)]
pub struct Workspace {
    pub id: String,
    pub base: Option<InstanceConfiguration>,
    pub content: Vec<ContentItem>,
    pub content_tab: Option<i32>,
    pub content_revision: Option<u64>,
    pub mod_icons: std::collections::HashMap<String, Option<slint::Image>>,
    pub client_preview: Option<api::AuroraTransition>,
    pub icons_pending: std::collections::HashSet<String>,
    pub content_serial: u64,
    pub icons_visible: std::collections::HashSet<String>,
    pub icons_loading: bool,
    pub icons_failed: std::collections::HashSet<String>,
    pub icon_order: std::collections::VecDeque<String>,
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

#[derive(Clone, PartialEq)]
pub struct ContentItem {
    pub icon_source: Option<ContentIconSource>,
    pub row: ContentRow,
    /// Modrinth project id when the file is provider-managed.
    pub project: Option<String>,
}

#[derive(Clone, PartialEq)]
pub enum ContentIconSource {
    Mod(api::ModIconRequest),
    Pack(api::PackIconRequest, String),
}
impl ContentIconSource {
    fn key(&self) -> String {
        match self {
            Self::Mod(source) => source.sha256.clone(),
            Self::Pack(_, key) => key.clone(),
        }
    }
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
        return if crate::activity::instance_busy(&instance.id) {
            ("Installing".into(), Tone::Working)
        } else {
            ("Incomplete".into(), Tone::Warning)
        };
    }
    ("Installed".into(), Tone::Success)
}

pub fn bind(window: &AppWindow) {
    let instances = window.global::<Instances>();
    instances.on_open(|id| open_workspace(id.to_string(), TAB_MODS));
    instances.on_create(open_create);
    instances.on_close_workspace(|| {
        ui::with(close_workspace);
    });
    instances.on_tab_changed(|tab| load_tab(tab));
    instances.on_play(|| {
        let id = ui::state(|s| s.workspace.as_ref().map(|w| w.id.clone()));
        if let Some(id) = id {
            crate::home::request_play(id);
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
    instances.on_memory_step(|direction| {
        ui::with(|window, state| {
            let instances = window.global::<Instances>();
            if state.workspace.is_none()
                || instances.get_config_busy()
                || instances.get_installing()
            {
                return;
            }
            if let Ok(value) = stepped_memory_mib(&instances.get_memory_mib_text(), direction) {
                instances.set_memory_mib_text(value.to_string().into());
            }
        });
        render_dirty();
    });
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
    instances.on_client_review(review_client);
    instances.on_client_confirm(confirm_client);
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
    instances.on_content_icon_visible(|instance_id, id, visible| {
        // Slint may report layout visibility while a model update still holds
        // AppState. Queue it onto the next event turn rather than reborrow it.
        let _ = slint::invoke_from_event_loop(move || {
            let serial = ui::with(|_, state| {
                let workspace = state.workspace.as_mut()?;
                if workspace.id != instance_id.as_str()
                    || !workspace.content.iter().any(|item| item.row.id == id)
                {
                    return None;
                }
                let changed = if visible {
                    workspace.icons_visible.insert(id.to_string())
                } else {
                    workspace.icons_visible.remove(id.as_str())
                };
                trim_icon_cache(workspace);
                changed.then_some(workspace.content_serial)
            })
            .flatten();
            if let Some(serial) = serial {
                render_content();
                load_mod_icons(serial);
            }
        });
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
        invalidate_content_cache();
        let tab = ui::with(|window, _| window.global::<Instances>().get_tab()).unwrap_or(0);
        load_tab(tab);
        crate::content_updates::check();
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
    instances.on_cfg_java_changed(|| {
        ui::with(|window, _| {
            let instances = window.global::<Instances>();
            instances.set_cfg_java_status("".into());
            instances.set_cfg_java_tone(Tone::Neutral);
        });
        render_dirty();
    });
    instances.on_cfg_java_check(check_custom_java);
    instances.on_cfg_java_choose(choose_custom_java);
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
    ui::with(|_, state| {
        state.pending_play = None;
        state.play_error = None;
    });
    ui::spawn(api::select_instance(ui::host(), id), |result| {
        if let Err(error) = result {
            ui::error_toast(&error);
        }
        reload(crate::home::refresh_readiness);
    });
}

/// Return to an existing settings draft without replacing it during navigation.
pub fn show_settings(id: String) {
    let resumed = ui::with(|window, state| {
        if !state.workspace.as_ref().is_some_and(|w| w.id == id) {
            return false;
        }
        window.global::<Instances>().set_tab(TAB_SETTINGS);
        window.global::<Nav>().set_page(Page::Instances);
        window.global::<Nav>().set_workspace_open(true);
        true
    })
    .unwrap_or(false);
    if !resumed {
        open_workspace(id, TAB_SETTINGS);
    }
}

pub fn open_workspace(id: String, tab: i32) {
    let found = ui::with(|window, state| {
        let instance = state.instance(&id)?.clone();
        remember_content(state);
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
        instances.set_client_review_open(false);
        instances.set_client_busy(false);
        instances.set_config_busy(false);
        instances.set_client_available(false);
        instances.set_client_note("Checking compatible release…".into());
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
    crate::syncing::load();
    seed_game_settings();
    render_workspace_header();
    render_dirty();
    refresh_client_option();
    load_memory_hint(id);
    load_tab(tab);
}

fn refresh_client_option() {
    let request = ui::with(|window, state| {
        let workspace = state.workspace.as_ref()?;
        let instance = state.instance(&workspace.id)?;
        let instances = window.global::<Instances>();
        instances.set_client_installed(instance.aurora.is_some());
        if let Some(pin) = &instance.aurora {
            instances.set_client_note(format!("Aurora Client {}", pin.version).into());
            return None;
        }
        if instance.platform.kind != "fabric" {
            instances.set_client_note("Aurora Client requires Fabric.".into());
            return None;
        }
        let version = instance.platform.version.as_ref()?;
        Some((
            workspace.id.clone(),
            instance.minecraft_version.clone(),
            serde_json::json!({"kind":"fabric", "policy":{"type":"pinned", "version":version}}),
        ))
    })
    .flatten();
    let Some((id, minecraft, loader)) = request else {
        return;
    };
    ui::spawn(
        api::aurora_compatibility(ui::host(), minecraft, loader),
        move |result| {
            ui::with(|window, state| {
                if state.workspace.as_ref().map(|w| w.id.as_str()) != Some(&id) {
                    return;
                }
                let instances = window.global::<Instances>();
                match result {
                    Ok(value) => {
                        instances.set_client_available(value.available);
                        instances.set_client_note(
                            if value.available {
                                format!("Latest compatible: {}", value.version.unwrap_or_default())
                            } else {
                                value.reason
                            }
                            .into(),
                        );
                    }
                    Err(error) => {
                        instances.set_client_available(false);
                        instances.set_client_note(error.message.into());
                    }
                }
            });
        },
    );
}

fn review_client() {
    let request = ui::with(|window, state| {
        let instances = window.global::<Instances>();
        if instances.get_client_busy()
            || instances.get_config_dirty()
            || instances.get_config_busy()
            || instances.get_installing()
            || instances.get_cfg_needs_install()
        {
            return None;
        }
        let workspace = state.workspace.as_mut()?;
        workspace.client_preview = None;
        instances.set_client_busy(true);
        instances.set_config_busy(true);
        Some((workspace.id.clone(), !instances.get_client_installed()))
    })
    .flatten();
    let Some((id, enabled)) = request else {
        return;
    };
    ui::spawn(
        api::preview_aurora(ui::host(), id.clone(), enabled),
        move |result| {
            ui::with(|window, state| {
                let Some(workspace) = state.workspace.as_mut().filter(|w| w.id == id) else {
                    return;
                };
                let instances = window.global::<Instances>();
                instances.set_client_busy(false);
                instances.set_config_busy(false);
                match result {
                    Ok(preview) => {
                        if preview.instance_id != id {
                            return;
                        }
                        let mut lines = preview
                            .requirements_added
                            .iter()
                            .map(|s| format!("Add: {s}"))
                            .chain(
                                preview
                                    .requirements_removed
                                    .iter()
                                    .map(|s| format!("Remove: {s}")),
                            )
                            .collect::<Vec<_>>();
                        lines.extend(preview.warnings.iter().cloned());
                        lines.extend(preview.blockers.iter().cloned());
                        instances.set_client_review_text(lines.join("\n\n").into());
                        instances.set_client_review_allowed(preview.blockers.is_empty());
                        workspace.client_preview = Some(preview);
                        instances.set_client_review_open(true);
                    }
                    Err(error) => instances.set_client_note(error.message.into()),
                }
            });
        },
    );
}

fn confirm_client() {
    let request = ui::with(|window, state| {
        let instances = window.global::<Instances>();
        if instances.get_client_busy()
            || !instances.get_client_review_open()
            || !instances.get_client_review_allowed()
            || instances.get_config_dirty()
            || instances.get_config_busy()
            || instances.get_installing()
        {
            return None;
        }
        let workspace = state.workspace.as_mut()?;
        let preview = workspace.client_preview.take()?;
        instances.set_client_review_open(false);
        instances.set_client_busy(true);
        instances.set_config_busy(true);
        Some((
            workspace.id.clone(),
            preview.target.aurora.is_some(),
            preview.fingerprint,
        ))
    })
    .flatten();
    let Some((id, enabled, fingerprint)) = request else {
        return;
    };
    ui::spawn(
        api::apply_aurora(ui::host(), id.clone(), enabled, fingerprint),
        move |result| {
            invalidate_content_cache();
            if let Err(error) = result {
                ui::error_toast(&error);
            }
            reload(move || {
                if ui::state(|s| s.workspace.as_ref().is_some_and(|w| w.id == id)) {
                    open_workspace(id, TAB_SETTINGS);
                }
                crate::home::refresh_readiness();
            });
        },
    );
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
        instances.set_installing(crate::activity::instance_busy(&instance.id));
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
    let custom = configuration.custom_java();
    instances.set_cfg_java_index(i32::from(custom.is_some()));
    instances.set_cfg_java_path(custom.unwrap_or_default().into());
    instances.set_cfg_java_status("".into());
    instances.set_cfg_java_tone(Tone::Neutral);
    instances.set_memory_mib_text(configuration.memory_mib.to_string().into());
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
fn memory_mib(text: &str) -> Result<u32, String> {
    use aurora_launcher_lib::instances::settings::{MAX_MEMORY_MIB, MIN_MEMORY_MIB};
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|m| (MIN_MEMORY_MIB..=MAX_MEMORY_MIB).contains(m))
        .ok_or_else(|| {
            format!("Enter a whole number from {MIN_MEMORY_MIB} to {MAX_MEMORY_MIB} MiB.")
        })
}

/// Step only a valid draft; invalid manual input remains intact for correction.
fn stepped_memory_mib(text: &str, direction: i32) -> Result<u32, String> {
    use aurora_launcher_lib::instances::settings::{MAX_MEMORY_MIB, MIN_MEMORY_MIB};
    let current = memory_mib(text)?;
    Ok(match direction {
        1 => current.saturating_add(1024).min(MAX_MEMORY_MIB),
        -1 => current.saturating_sub(1024).max(MIN_MEMORY_MIB),
        _ => current,
    })
}

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
    draft.java = draft_java(window, base)?;
    draft.memory_mib = memory_mib(&instances.get_memory_mib_text())?;
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

/// The edited Java selection; the exact persisted JSON when unchanged.
fn draft_java(
    window: &AppWindow,
    base: &InstanceConfiguration,
) -> Result<Option<serde_json::Value>, String> {
    let instances = window.global::<Instances>();
    if instances.get_cfg_java_index() != 1 {
        return Ok(None);
    }
    let path = instances.get_cfg_java_path().trim().to_owned();
    if path.is_empty() {
        return Err("Choose a Java executable, or switch back to managed Java.".into());
    }
    if base.custom_java() == Some(path.as_str()) {
        return Ok(base.java.clone());
    }
    Ok(Some(serde_json::json!({ "kind": "custom", "path": path })))
}

/// One line describing an executed custom Java check.
pub fn java_report_text(report: &api::JavaReport) -> (String, Tone) {
    match (&report.facts, &report.problem) {
        (Some(facts), _) if report.valid => (
            format!(
                "Java {} ({}{}{}, {}). Matches the Java {} this game requires.",
                facts.major_version,
                facts.vendor,
                if facts.vendor.is_empty() { "" } else { " " },
                facts.version,
                facts.architecture,
                report.required_major_version
            ),
            Tone::Success,
        ),
        (_, Some(problem)) => (problem.clone(), Tone::Error),
        _ => ("The Java check did not finish.".into(), Tone::Error),
    }
}

fn check_custom_java() {
    let Some((id, path, base)) = ui::with(|window, state| {
        let workspace = state.workspace.as_ref()?;
        let base = workspace.base.as_ref()?;
        let instances = window.global::<Instances>();
        let path = instances.get_cfg_java_path().trim().to_owned();
        if path.is_empty() || instances.get_cfg_java_busy() {
            return None;
        }
        let (version, loader) = draft_platform(window, workspace, base);
        if version != base.minecraft_version || loader != base.loader {
            instances
                .set_cfg_java_status("Save and install game changes before checking Java.".into());
            instances.set_cfg_java_tone(Tone::Warning);
            return None;
        }
        instances.set_cfg_java_busy(true);
        instances.set_cfg_java_status("Running the selected Java…".into());
        instances.set_cfg_java_tone(Tone::Working);
        Some((workspace.id.clone(), path, base.clone()))
    })
    .flatten() else {
        return;
    };
    let checked = path.clone();
    let checked_id = id.clone();
    ui::spawn(
        api::check_custom_java(ui::host(), id, path),
        move |result| {
            ui::with(|window, state| {
                let instances = window.global::<Instances>();
                instances.set_cfg_java_busy(false);
                let draft_matches = state.workspace.as_ref().is_some_and(|workspace| {
                    let (version, loader) = draft_platform(window, workspace, &base);
                    version == base.minecraft_version && loader == base.loader
                });
                // A path edited during the check gets no stale verdict.
                if state.workspace.as_ref().map(|w| w.id.as_str()) != Some(checked_id.as_str())
                    || state.workspace.as_ref().and_then(|w| w.base.as_ref()) != Some(&base)
                    || !draft_matches
                    || instances.get_cfg_java_index() != 1
                    || instances.get_cfg_java_path().trim() != checked
                {
                    instances.set_cfg_java_status("".into());
                    instances.set_cfg_java_tone(Tone::Neutral);
                    return;
                }
                let (text, tone) = match result {
                    Ok(report) => java_report_text(&report),
                    Err(error) => (error.message, Tone::Error),
                };
                instances.set_cfg_java_status(text.into());
                instances.set_cfg_java_tone(tone);
            });
        },
    );
}

fn choose_custom_java() {
    let owner = crate::chrome::dialog_owner();
    let target = ui::state(|state| state.workspace.as_ref().map(|w| w.id.clone()));
    ui::spawn(
        crate::skin_import::choose_java_path(owner),
        move |result| match result {
            Ok(Some(path)) => {
                ui::with(|window, state| {
                    if state.workspace.as_ref().map(|w| &w.id) != target.as_ref() {
                        return;
                    }
                    let instances = window.global::<Instances>();
                    instances.set_cfg_java_index(1);
                    instances.set_cfg_java_path(path.into());
                    instances.set_cfg_java_status(
                        "Not checked yet. Check runs the selected executable.".into(),
                    );
                    instances.set_cfg_java_tone(Tone::Neutral);
                });
                render_dirty();
            }
            Ok(None) => {}
            Err(error) => ui::error_toast(&error),
        },
    );
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
        api::update_configuration(ui::host(), id.clone(), base, draft),
        move |result| {
            apply_saved_configuration(&id, result);
            reload(|| {
                seed_game_settings();
                crate::home::refresh_readiness();
            });
        },
    );
}

fn apply_saved_configuration(id: &str, result: api::ApiResult<InstanceSummary>) {
    let message = ui::with(|window, state| {
        let workspace = state
            .workspace
            .as_mut()
            .filter(|workspace| workspace.id == id)?;
        let instances = window.global::<Instances>();
        instances.set_config_busy(false);
        match result {
            Ok(updated) => {
                workspace.base = Some(updated.configuration.clone());
                write_draft(window, &updated.configuration);
                instances.set_config_dirty(false);
                Some(if needs_install(&updated) {
                    "Settings saved. Install them to apply the new game version or loader."
                } else {
                    "Settings saved."
                })
            }
            Err(error) => {
                instances.set_config_error(error.message.into());
                None
            }
        }
    })
    .flatten();
    // Toast enters ui::with itself. Publish only after the state borrow ends.
    if let Some(message) = message {
        ui::toast(message, Tone::Success);
    }
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
                }
                Err(_) => instances.set_memory_hint("".into()),
            }
        });
    });
}

pub(crate) fn validate() {
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
                            "ready" if status.component == "Custom Java" =>
                                "checked and compatible".to_owned(),
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
        invalidate_content_cache();
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

fn remember_content(state: &mut ui::AppState) {
    let Some(workspace) = state.workspace.as_ref() else {
        return;
    };
    let Some(tab) = workspace.content_tab else {
        return;
    };
    // Enforce the limit before cloning rows, including transient allocations.
    if workspace.content.len() > crate::content_cache::MAX_ROWS {
        state.content_cache.invalidate(&workspace.id);
        return;
    }
    state.content_cache.put(
        &workspace.id,
        tab,
        crate::content_cache::Snapshot {
            items: workspace.content.clone(),
            icons: workspace.mod_icons.clone(),
            revision: workspace.content_revision,
        },
    );
}

/// Shared by the sidebar and the workspace back button. Explicit workspace
/// opening uses set_page directly, preserving Home's settings shortcut.
pub fn close_workspace(window: &AppWindow, state: &mut ui::AppState) {
    remember_content(state);
    state.workspace = None;
    window.global::<Nav>().set_workspace_open(false);
}

fn invalidate_content_cache() {
    ui::with(|_, state| {
        state.content_cache.clear();
        if let Some(workspace) = state.workspace.as_mut() {
            workspace.content_tab = None;
            workspace.content_revision = None;
        }
    });
}

/// Installs the cached snapshot synchronously, before any worker is scheduled.
fn begin_content_tab(tab: i32) -> Option<(String, u64)> {
    ui::with(|window, state| {
        remember_content(state);
        let id = state.workspace.as_ref()?.id.clone();
        let cached = state.content_cache.get(&id, tab);
        let hit = cached.is_some();
        let cached = cached.unwrap_or_default();
        state.content_serial += 1;
        let workspace = state.workspace.as_mut()?;
        let same_tab = workspace.content_tab == Some(tab);
        workspace.content_serial = state.content_serial;
        workspace.content_tab = hit.then_some(tab);
        workspace.content_revision = cached.revision;
        workspace.content = cached.items;
        workspace.mod_icons = cached.icons;
        workspace.icon_order = workspace.mod_icons.keys().cloned().collect();
        workspace.icons_pending.clear();
        if !same_tab {
            workspace.icons_visible.clear();
        }
        workspace.icons_loading = false;
        workspace.icons_failed.clear();
        if workspace.updates.tab != tab {
            workspace.updates = crate::content_updates::Updates::default();
        }
        let instances = window.global::<Instances>();
        instances.set_content_loading(!hit);
        instances.set_content_error("".into());
        Some((id, workspace.content_serial))
    })
    .flatten()
}

fn load_tab(tab: i32) {
    crate::perf_trace::mark("tab", tab);
    if !(TAB_MODS..TAB_SETTINGS).contains(&tab) {
        return;
    }
    let Some((id, serial)) = begin_content_tab(tab) else {
        return;
    };
    render_content();
    let host = ui::host();
    let work_id = id.clone();
    if tab == TAB_MODS {
        ui::spawn(
            async move {
                let revision =
                    crate::content_cache::folder_revision(host.clone(), work_id.clone(), tab).await;
                (revision, api::instance_mods(host, work_id).await)
            },
            move |(revision, result)| {
                apply_content(&id, tab, serial, revision, result.map(mod_items));
            },
        );
    } else {
        ui::spawn(
            async move {
                let revision =
                    crate::content_cache::folder_revision(host.clone(), work_id.clone(), tab).await;
                (
                    revision,
                    api::instance_content(host, work_id, CONTENT_KINDS[tab as usize]).await,
                )
            },
            move |(revision, result)| {
                apply_content(
                    &id,
                    tab,
                    serial,
                    revision,
                    result.map(|inventory| pack_items(inventory, tab == 1)),
                );
            },
        );
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
                icon_source: entry.sha256.as_ref().map(|hash| {
                    ContentIconSource::Mod(api::ModIconRequest {
                        file_name: entry.file_name.clone(),
                        sha256: hash.clone(),
                    })
                }),
                project: modrinth_project(entry.provenance.as_ref()),
                row: ContentRow {
                    is_mod: true,
                    letter: ui::letter(&entry.display_name).into(),
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

fn pack_items(inventory: api::ContentInventory, hide_bundled: bool) -> Vec<ContentItem> {
    inventory
        .entries
        .into_iter()
        .filter(|entry| !hide_bundled || !entry.pack_owned)
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
                icon_source: matches!(entry.file_type.as_str(), "zip" | "directory").then(|| {
                    ContentIconSource::Pack(
                        api::PackIconRequest {
                            file_name: entry.file_name.clone(),
                            sha256: entry.sha256.clone(),
                        },
                        format!("pack:{}:{}", hide_bundled, entry.entry_id),
                    )
                }),
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

fn apply_content(
    id: &str,
    tab: i32,
    serial: u64,
    revision: Option<u64>,
    result: api::ApiResult<Vec<ContentItem>>,
) {
    let applied = ui::with(|window, state| {
        let workspace = state.workspace.as_mut()?;
        if workspace.id != id
            || workspace.content_serial != serial
            || window.global::<Instances>().get_tab() != tab
        {
            return None;
        }
        let instances = window.global::<Instances>();
        instances.set_content_loading(false);
        match result {
            Ok(items) => {
                instances.set_content_error("".into());
                let changed = workspace.content_tab != Some(tab)
                    || workspace.content != items
                    || workspace.content_revision != revision;
                if changed {
                    // Discard any artwork batch started for the cached view
                    // before this authoritative refresh finished.
                    state.content_serial += 1;
                    workspace.content_serial = state.content_serial;
                    workspace.icons_pending.clear();
                    workspace.icons_loading = false;
                    workspace.icons_failed.clear();
                    let artwork_changed =
                        revision.is_none() || workspace.content_revision != revision;
                    workspace.content = items;
                    workspace.content_tab = Some(tab);
                    workspace.content_revision = revision;
                    let hashes: std::collections::HashSet<_> = workspace
                        .content
                        .iter()
                        .filter_map(|item| item.icon_source.as_ref().map(ContentIconSource::key))
                        .collect();
                    workspace.mod_icons.retain(|key, _| {
                        hashes.contains(key) && !(artwork_changed && key.starts_with("pack:"))
                    });
                    workspace
                        .icon_order
                        .retain(|key| workspace.mod_icons.contains_key(key));
                }
                Some((changed, workspace.content_serial))
            }
            Err(error) => {
                state.content_cache.invalidate(id);
                workspace.content_tab = None;
                instances.set_content_error(error.message.into());
                Some((false, workspace.content_serial))
            }
        }
    })
    .flatten();
    let Some((changed, icon_serial)) = applied else {
        return;
    };
    if changed {
        render_content();
    }
    load_mod_icons(icon_serial);
}

const ICON_BATCH: usize = 8;
const ICON_CACHE: usize = 64;

/// One decoded batch result: file-embedded pixels, or a provider artwork PNG.
enum LoadedIcon {
    Rgba((u32, u32, Vec<u8>)),
    Png(Vec<u8>),
}

fn trim_icon_cache(workspace: &mut Workspace) {
    let visible: std::collections::HashSet<_> = workspace
        .content
        .iter()
        .filter(|item| workspace.icons_visible.contains(item.row.id.as_str()))
        .filter_map(|item| item.icon_source.as_ref().map(ContentIconSource::key))
        .collect();
    let attempts = workspace.icon_order.len();
    for _ in 0..attempts {
        if workspace.mod_icons.len() <= ICON_CACHE {
            break;
        }
        let Some(key) = workspace.icon_order.pop_front() else {
            break;
        };
        if visible.contains(&key) {
            workspace.icon_order.push_back(key);
        } else {
            workspace.mod_icons.remove(&key);
        }
    }
}

fn artwork_row(workspace: &Workspace, item: &ContentItem) -> ContentRow {
    let mut row = item.row.clone();
    // Only the cache and currently visible model rows retain image references.
    row.picture = slint::Image::default();
    row.has_picture = false;
    if let Some(image) = item
        .icon_source
        .as_ref()
        .filter(|_| workspace.icons_visible.contains(item.row.id.as_str()))
        .and_then(|source| workspace.mod_icons.get(&source.key()))
        .and_then(|image| image.as_ref())
    {
        row.picture = image.clone();
        row.has_picture = true;
    }
    row
}

fn visible_icon_sources(workspace: &Workspace) -> Vec<ContentIconSource> {
    let filter = workspace.filter.to_lowercase();
    workspace
        .content
        .iter()
        .filter(|item| {
            workspace.icons_visible.contains(item.row.id.as_str())
                && (filter.is_empty()
                    || item.row.title.to_lowercase().contains(&filter)
                    || item.row.detail.to_lowercase().contains(&filter))
        })
        .filter_map(|item| item.icon_source.clone())
        .filter(|source| {
            let key = source.key();
            !workspace.mod_icons.contains_key(&key)
                && !workspace.icons_pending.contains(&key)
                && !workspace.icons_failed.contains(&key)
        })
        .take(ICON_BATCH)
        .collect()
}

fn load_mod_icons(serial: u64) {
    let request = ui::with(|window, state| {
        let tab = window.global::<Instances>().get_tab();
        if !(0..3).contains(&tab) {
            return None;
        }
        let workspace = state.workspace.as_mut()?;
        if workspace.content_serial != serial || workspace.icons_loading {
            return None;
        }
        let files = visible_icon_sources(workspace);
        if files.is_empty() {
            return None;
        }
        for source in &files {
            workspace.icons_pending.insert(source.key());
        }
        // Provider-managed rows fall back to their project's artwork when the file
        // carries none (shader packs never ship a pack.png).
        let projects: std::collections::HashMap<String, String> = workspace
            .content
            .iter()
            .filter_map(|item| Some((item.icon_source.as_ref()?.key(), item.project.clone()?)))
            .collect();
        workspace.icons_loading = true;
        Some((workspace.id.clone(), tab, files, projects))
    })
    .flatten();
    let Some((id, tab, files, projects)) = request else {
        return;
    };
    let host = ui::host();
    let artwork_host = host.clone();
    let expected_id = id.clone();
    let keys: Vec<_> = files.iter().map(ContentIconSource::key).collect();
    ui::spawn(
        async move {
            let bitmaps: Vec<_> = if tab == TAB_MODS {
                let requests = files
                    .iter()
                    .filter_map(|source| match source {
                        ContentIconSource::Mod(request) => Some(request.clone()),
                        _ => None,
                    })
                    .collect();
                let results = api::mod_icons(host, id, requests).await?;
                files
                    .iter()
                    .map(|source| {
                        let key = source.key();
                        let bitmap = results
                            .iter()
                            .find(|result| result.sha256 == key)
                            .and_then(|result| result.bitmap.as_ref());
                        (key, bitmap)
                    })
                    .map(|(key, bitmap)| (key, decode_icon(bitmap).map(LoadedIcon::Rgba)))
                    .collect()
            } else {
                let requests = files
                    .iter()
                    .filter_map(|source| match source {
                        ContentIconSource::Pack(request, _) => Some(request.clone()),
                        _ => None,
                    })
                    .collect();
                let results =
                    api::pack_icons(host, id, CONTENT_KINDS[tab as usize], requests).await?;
                files
                    .iter()
                    .map(|source| {
                        let bitmap = match source {
                            ContentIconSource::Pack(request, _) => results
                                .iter()
                                .find(|result| result.file_name == request.file_name)
                                .and_then(|result| result.bitmap.as_ref()),
                            _ => None,
                        };
                        (source.key(), decode_icon(bitmap).map(LoadedIcon::Rgba))
                    })
                    .collect()
            };
            // Cache-first in the core: a stored project icon is read from disk and
            // only an uncached project touches the provider once.
            let mut bitmaps = bitmaps;
            for (key, icon) in &mut bitmaps {
                if icon.is_none() {
                    if let Some(project) = projects.get(key) {
                        *icon = api::project_artwork(artwork_host.clone(), project.clone())
                            .await
                            .ok()
                            .flatten()
                            .map(LoadedIcon::Png);
                    }
                }
            }
            // Decoding/transferring only the current bounded batch avoids waiting for
            // the entire installed inventory before its first visible icons appear.
            fn decode_icon(bitmap: Option<&api::ModIconBitmap>) -> Option<(u32, u32, Vec<u8>)> {
                use base64::Engine;
                let bitmap = bitmap?;
                if bitmap.width == 0
                    || bitmap.height == 0
                    || bitmap.width > 96
                    || bitmap.height > 96
                {
                    return None;
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&bitmap.rgba_base64)
                    .ok()?;
                (bytes.len() == bitmap.width as usize * bitmap.height as usize * 4).then_some((
                    bitmap.width,
                    bitmap.height,
                    bytes,
                ))
            }
            Ok::<_, api::ApiError>(bitmaps)
        },
        move |result| {
            ui::with(|_, state| {
                let Some(workspace) = state.workspace.as_mut() else {
                    return;
                };
                if workspace.id != expected_id || workspace.content_serial != serial {
                    return;
                }
                workspace.icons_loading = false;
                for key in &keys {
                    workspace.icons_pending.remove(key);
                }
                match result {
                    Ok(icons) => {
                        for (key, icon) in icons {
                            let image = icon.and_then(|icon| match icon {
                                LoadedIcon::Rgba((w, h, bytes)) => {
                                    ui::rectangular_image(&bytes, w, h)
                                }
                                LoadedIcon::Png(bytes) => {
                                    slint::Image::load_from_data(&bytes, Some("png")).ok()
                                }
                            });
                            workspace.mod_icons.insert(key.clone(), image);
                            workspace.icon_order.retain(|old| old != &key);
                            workspace.icon_order.push_back(key);
                        }
                    }
                    // Do not retry a failed batch continuously; refresh clears this
                    // transient suppression, unlike a genuinely absent icon cache.
                    Err(_) => workspace.icons_failed.extend(keys),
                }
                trim_icon_cache(workspace);
            });
            render_content();
            load_mod_icons(serial);
        },
    );
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
                        let mut row = artwork_row(workspace, item);
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
        crate::perf_trace::mark(
            "content",
            format_args!(
                "rows={} loading={}",
                rows.len(),
                instances.get_content_loading()
            ),
        );
        instances.set_content(content_model(instances.get_content(), rows));
    });
}

fn content_model(
    current: slint::ModelRc<ContentRow>,
    rows: Vec<ContentRow>,
) -> slint::ModelRc<ContentRow> {
    if current.row_count() == rows.len()
        && rows
            .iter()
            .enumerate()
            .all(|(i, row)| current.row_data(i).is_some_and(|old| old.id == row.id))
        && let Some(model) = current
            .as_any()
            .downcast_ref::<slint::VecModel<ContentRow>>()
    {
        for (i, row) in rows.into_iter().enumerate() {
            if model.row_data(i).as_ref() != Some(&row) {
                model.set_row_data(i, row);
            }
        }
        current
    } else {
        ui::model(rows)
    }
}

/// After a queued install or update settles: reload the open content tab and
/// refresh update badges that described the superseded files.
pub fn after_content_change() {
    invalidate_content_cache();
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
        state.content_cache.invalidate(&workspace.id);
        workspace.content_tab = None;
        state.content_serial += 1;
        workspace.content_serial = state.content_serial;
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
            api::set_mod_enabled(host, id.clone(), entry, enabled),
            move |result| finish_mutation(&id, serial, result.map(mod_items)),
        );
    } else if (1..TAB_SETTINGS).contains(&tab) {
        let kind = CONTENT_KINDS[tab as usize];
        ui::spawn(
            api::set_pack_enabled(host, id.clone(), kind, entry, enabled),
            move |result| {
                finish_mutation(
                    &id,
                    serial,
                    result.map(|inventory| pack_items(inventory, tab == 1)),
                )
            },
        );
    }
}

fn remove_content() {
    let Some((id, tab, entry, serial)) = ui::with(|window, state| {
        let instances = window.global::<Instances>();
        instances.set_remove_open(false);
        let workspace = state.workspace.as_mut()?;
        let (entry, _) = workspace.pending_remove.take()?;
        state.content_cache.invalidate(&workspace.id);
        workspace.content_tab = None;
        state.content_serial += 1;
        workspace.content_serial = state.content_serial;
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
        ui::spawn(api::remove_mod(host, id.clone(), entry), move |result| {
            finish_mutation(&id, serial, result.map(mod_items))
        });
    } else if (1..TAB_SETTINGS).contains(&tab) {
        let kind = CONTENT_KINDS[tab as usize];
        ui::spawn(
            api::remove_content(host, id.clone(), kind, entry),
            move |result| {
                finish_mutation(
                    &id,
                    serial,
                    result.map(|inventory| pack_items(inventory, tab == 1)),
                )
            },
        );
    }
}

/// Applies the authoritative inventory returned by a content mutation. On
/// failure the list is re-read so the UI never shows an optimistic state.
fn finish_mutation(id: &str, serial: u64, result: api::ApiResult<Vec<ContentItem>>) {
    if !ui::state(|s| {
        s.workspace
            .as_ref()
            .is_some_and(|w| w.id == id && w.content_serial == serial)
    }) {
        return;
    }
    match result {
        Ok(items) => {
            let tab =
                ui::with(|window, _| window.global::<Instances>().get_tab()).unwrap_or(TAB_MODS);
            apply_content(id, tab, serial, None, Ok(items));
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
    let host = ui::host();
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
                api::aurora_releases(host).await,
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
    ui::spawn(
        api::aurora_compatibility(ui::host(), version, loader),
        |result| {
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
        },
    );
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

    fn test_ui(root: Option<std::path::PathBuf>) -> (AppWindow, tokio::runtime::Runtime) {
        struct TestPlatform;
        impl slint::platform::Platform for TestPlatform {
            fn create_window_adapter(
                &self,
            ) -> Result<std::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError>
            {
                Ok(
                    slint::platform::software_renderer::MinimalSoftwareWindow::new(
                        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
                    ),
                )
            }
        }
        let _ = slint::platform::set_platform(Box::new(TestPlatform));
        let window = AppWindow::new().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let host = aurora_launcher_lib::host::Host::native(
            root.ok_or_else(|| "headless UI regression".into()),
            "test",
            runtime.handle().clone(),
            |_, _| {},
        );
        ui::install(window.clone_strong(), host, runtime.handle().clone());
        bind(&window);
        (window, runtime)
    }

    #[test]
    fn saving_stepped_memory_publishes_success_without_reborrowing_state() {
        let (window, _runtime) = test_ui(None);
        let base: InstanceConfiguration = serde_json::from_value(serde_json::json!({
            "minecraftVersion":"1.21.1", "auroraEnabled":false,
            "loader":{"kind":"vanilla"}, "memoryMib":4096,
            "additionalJvmArguments":"", "window":null
        }))
        .unwrap();
        ui::with(|window, state| {
            state.workspace = Some(Workspace {
                id: "test-instance".into(),
                base: Some(base.clone()),
                ..Default::default()
            });
            write_draft(window, &base);
        });
        window.global::<Instances>().invoke_memory_step(-1);
        let draft = ui::with(|window, state| {
            read_draft(window, &base, state.workspace.as_ref().unwrap()).unwrap()
        })
        .unwrap();
        assert_eq!(draft.memory_mib, 3072);
        let updated = InstanceSummary {
            id: "test-instance".into(),
            display_name: "Test".into(),
            state: "ready".into(),
            aurora: None,
            platform: api::Platform {
                kind: "vanilla".into(),
                version: None,
            },
            pack: None,
            minecraft_version: "1.21.1".into(),
            configuration: draft.clone(),
            recovery: None,
        };
        apply_saved_configuration("test-instance", Ok(updated));
        assert_eq!(
            ui::state(|s| s.workspace.as_ref().unwrap().base.clone()),
            Some(draft)
        );
        assert!(!window.global::<Instances>().get_config_busy());
        assert!(!window.global::<Instances>().get_config_dirty());
        assert_eq!(
            window.global::<crate::Shell>().get_toast_text(),
            "Settings saved."
        );
        ui::release();
    }

    #[test]
    fn sidebar_instances_always_returns_to_selection_from_every_page() {
        let (window, _runtime) = test_ui(None);
        crate::bind_navigation(&window);
        for page in [
            Page::Home,
            Page::Discover,
            Page::Settings,
            Page::Skins,
            Page::Instances,
        ] {
            ui::with(|window, state| {
                state.workspace = Some(Workspace {
                    id: "test".into(),
                    ..Default::default()
                });
                window.global::<Nav>().set_workspace_open(true);
                window.global::<Nav>().set_page(page);
            });
            window.global::<Nav>().invoke_navigate(Page::Instances);
            assert_eq!(window.global::<Nav>().get_page(), Page::Instances);
            assert!(!window.global::<Nav>().get_workspace_open());
            assert!(ui::state(|state| state.workspace.is_none()));
        }
        ui::release();
    }

    #[test]
    fn content_reentry_restores_rows_and_icon_keys_and_rejects_stale_scans() {
        let (window, _runtime) = test_ui(None);
        let item = ContentItem {
            row: ContentRow {
                id: "mod".into(),
                title: "Installed mod".into(),
                ..Default::default()
            },
            project: Some("project".into()),
            icon_source: Some(ContentIconSource::Mod(api::ModIconRequest {
                file_name: "mod.jar".into(),
                sha256: "hash".into(),
            })),
        };
        ui::with(|window, state| {
            state.workspace = Some(Workspace {
                id: "one".into(),
                content: vec![item.clone()],
                content_tab: Some(TAB_MODS),
                content_revision: Some(7),
                ..Default::default()
            });
            state
                .workspace
                .as_mut()
                .unwrap()
                .mod_icons
                .insert("hash".into(), None);
            window.global::<Instances>().set_tab(TAB_MODS);
            close_workspace(window, state);
            state.workspace = Some(Workspace {
                id: "one".into(),
                ..Default::default()
            });
            window.global::<Nav>().set_workspace_open(true);
        });
        let (_, first) = begin_content_tab(TAB_MODS).unwrap();
        render_content();
        assert!(!window.global::<Instances>().get_content_loading());
        assert_eq!(window.global::<Instances>().get_content().row_count(), 1);
        assert!(ui::state(|state| state
            .workspace
            .as_ref()
            .unwrap()
            .mod_icons
            .contains_key("hash")));
        let model = window.global::<Instances>().get_content();
        apply_content("one", TAB_MODS, first, Some(7), Ok(vec![item.clone()]));
        assert!(std::ptr::eq(
            model.as_any(),
            window.global::<Instances>().get_content().as_any()
        ));
        ui::with(|window, state| {
            close_workspace(window, state);
            state.workspace = Some(Workspace {
                id: "one".into(),
                ..Default::default()
            });
        });
        let (_, second) = begin_content_tab(TAB_MODS).unwrap();
        assert_ne!(
            first, second,
            "request identities survive same-instance reopen"
        );
        apply_content("one", TAB_MODS, first, Some(8), Ok(vec![]));
        assert_eq!(
            ui::state(|state| state.workspace.as_ref().unwrap().content.len()),
            1
        );
        window.global::<Instances>().set_tab(1);
        let (_, pack_serial) = begin_content_tab(1).unwrap();
        assert!(window.global::<Instances>().get_content_loading());
        apply_content("one", TAB_MODS, second, Some(8), Ok(vec![]));
        assert!(ui::state(|state| state
            .workspace
            .as_ref()
            .unwrap()
            .content
            .is_empty()));
        apply_content("one", 1, pack_serial, Some(9), Ok(vec![item]));
        assert_eq!(window.global::<Instances>().get_content().row_count(), 1);
        invalidate_content_cache();
        assert!(
            ui::with(|_, state| state.content_cache.get("one", 0).is_none()
                && state.content_cache.get("one", 1).is_none())
            .unwrap()
        );
        assert!(ui::state(|state| state
            .workspace
            .as_ref()
            .unwrap()
            .content_tab
            .is_none()));
        ui::release();
    }

    #[test]
    #[ignore = "requires AURORA_CONTENT_TIMING_ROOT pointing at a disposable registered fixture"]
    fn installed_content_navigation_timing_harness() {
        let root =
            std::path::PathBuf::from(std::env::var_os("AURORA_CONTENT_TIMING_ROOT").unwrap());
        // Safety: the harness explicitly refuses the platform owner data root.
        assert!(root.join("launcher/instances.json").is_file());
        assert!(
            root.components()
                .any(|part| part.as_os_str() == "timing-fixture")
        );
        assert_eq!(
            aurora_launcher_lib::paths::ManagedPaths::from_app_local_data_dir(root.clone())
                .unwrap()
                .data_root(),
            root
        );
        let (window, runtime) = test_ui(Some(root));
        let host = ui::host();
        let scan = |tab| {
            if tab == TAB_MODS {
                mod_items(
                    runtime
                        .block_on(api::instance_mods(host.clone(), "timing-instance".into()))
                        .unwrap(),
                )
            } else {
                pack_items(
                    runtime
                        .block_on(api::instance_content(
                            host.clone(),
                            "timing-instance".into(),
                            CONTENT_KINDS[tab as usize],
                        ))
                        .unwrap(),
                    tab == 1,
                )
            }
        };
        for tab in 0..3 {
            let mut before = Vec::new();
            for _ in 0..2 {
                let start = std::time::Instant::now();
                let items = scan(tab);
                let rows = items.into_iter().map(|item| item.row).collect();
                window
                    .global::<Instances>()
                    .set_content(content_model(Default::default(), rows));
                before.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            ui::with(|window, state| {
                state.content_cache.clear();
                state.workspace = Some(Workspace {
                    id: "timing-instance".into(),
                    ..Default::default()
                });
                window.global::<Instances>().set_tab(tab);
            });
            let start = std::time::Instant::now();
            let (_, serial) = begin_content_tab(tab).unwrap();
            render_content();
            let revision = runtime.block_on(crate::content_cache::folder_revision(
                host.clone(),
                "timing-instance".into(),
                tab,
            ));
            let items = scan(tab);
            let count = items.len();
            assert!(count > 0);
            apply_content("timing-instance", tab, serial, revision, Ok(items.clone()));
            let first = start.elapsed().as_secs_f64() * 1000.0;
            ui::with(|window, state| {
                close_workspace(window, state);
                state.workspace = Some(Workspace {
                    id: "timing-instance".into(),
                    ..Default::default()
                });
            });
            let start = std::time::Instant::now();
            let (_, serial) = begin_content_tab(tab).unwrap();
            render_content();
            let second = start.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(
                window.global::<Instances>().get_content().row_count(),
                count
            );
            assert!(!window.global::<Instances>().get_content_loading());
            // The real background refresh follows publication; it must leave
            // identical rows and artwork keys intact.
            apply_content("timing-instance", tab, serial, revision, Ok(scan(tab)));
            println!(
                "CONTENT_TIMING kind={} rows={} before_first_ms={:.3} before_second_ms={:.3} after_first_ms={:.3} after_second_ms={:.3}",
                CONTENT_KINDS[tab as usize], count, before[0], before[1], first, second
            );
        }
        ui::release();
    }

    #[test]
    fn bundled_resource_packs_are_hidden_while_personal_packs_remain() {
        let entry = |name: &str, pack_owned: bool| serde_json::json!({"entryId":name,"fileName":name,"displayName":name,"packOwned":pack_owned,"management":{"canRemove":!pack_owned,"canToggle":true,"active":false,"activationManagedInGame":false}});
        let inventory: api::ContentInventory = serde_json::from_value(serde_json::json!({"entries":[entry("Mod Menu Helper.zip",true),entry("Personal texture pack.zip",false)]})).unwrap();
        assert_eq!(
            pack_items(inventory.clone(), false).len(),
            2,
            "bundled shader entries remain visible"
        );
        let items = pack_items(inventory, true);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].row.title.as_str(), "Personal texture pack.zip");
        assert!(items[0].row.removable);
    }

    #[test]
    fn memory_entry_rejects_invalid_and_out_of_core_range_values() {
        for value in [
            "",
            "-1",
            "NaN",
            "inf",
            "0",
            "511",
            "32769",
            "65536",
            "999999999999999",
            "4.5",
        ] {
            assert!(memory_mib(value).is_err(), "{value}");
        }
        for value in [512, 2048, 4096, 32768] {
            assert_eq!(memory_mib(&format!(" {value} ")).unwrap(), value);
        }
    }

    #[test]
    fn visible_icon_work_is_batched_filtered_and_excludes_cached_pending_and_failed() {
        let mut workspace = Workspace::default();
        for i in 0..100 {
            workspace.content.push(ContentItem {
                icon_source: Some(ContentIconSource::Mod(api::ModIconRequest {
                    file_name: format!("{i}.jar"),
                    sha256: format!("{i:064x}"),
                })),
                row: ContentRow {
                    id: i.to_string().into(),
                    title: format!("row{i}").into(),
                    ..Default::default()
                },
                project: None,
            });
        }
        assert!(visible_icon_sources(&workspace).is_empty());
        workspace
            .icons_visible
            .extend((0..20).map(|i| i.to_string()));
        workspace.mod_icons.insert(format!("{:064x}", 0), None);
        workspace.icons_pending.insert(format!("{:064x}", 1));
        workspace.icons_failed.insert(format!("{:064x}", 2));
        let keys: Vec<_> = visible_icon_sources(&workspace)
            .iter()
            .map(ContentIconSource::key)
            .collect();
        assert_eq!(
            keys,
            (3..11).map(|i| format!("{i:064x}")).collect::<Vec<_>>()
        );
        workspace.filter = "row9".into();
        assert_eq!(visible_icon_sources(&workspace).len(), 1);
        workspace.icons_visible.clear();
        assert!(visible_icon_sources(&workspace).is_empty());
    }
    #[test]
    fn artwork_publishes_batches_releases_offscreen_rows_and_trims_unpinned_cache() {
        let mut workspace = Workspace::default();
        let image = ui::rectangular_image(&[255, 0, 0, 255], 1, 1).unwrap();
        for i in 0..ICON_CACHE + ICON_BATCH {
            workspace.content.push(ContentItem {
                icon_source: Some(ContentIconSource::Mod(api::ModIconRequest {
                    file_name: format!("{i}.jar"),
                    sha256: format!("{i:064x}"),
                })),
                row: ContentRow {
                    id: i.to_string().into(),
                    letter: "M".into(),
                    ..Default::default()
                },
                project: None,
            });
            workspace.icons_visible.insert(i.to_string());
        }
        let first = visible_icon_sources(&workspace);
        assert_eq!(first.len(), ICON_BATCH);
        for source in first {
            let key = source.key();
            workspace.mod_icons.insert(key.clone(), Some(image.clone()));
            workspace.icon_order.push_back(key);
        }
        // The first batch is publishable while later visible rows still wait.
        let model = ui::model(
            workspace
                .content
                .iter()
                .map(|item| artwork_row(&workspace, item))
                .collect::<Vec<_>>(),
        );
        assert!(model.row_data(0).unwrap().has_picture);
        assert!(!model.row_data(ICON_BATCH).unwrap().has_picture);
        assert_eq!(
            visible_icon_sources(&workspace)[0].key(),
            format!("{:064x}", ICON_BATCH)
        );
        for item in &workspace.content[ICON_BATCH..] {
            let key = item.icon_source.as_ref().unwrap().key();
            workspace.mod_icons.insert(key.clone(), Some(image.clone()));
            workspace.icon_order.push_back(key);
        }
        // Visible rows are pinned. Once hidden, their cache entries are evictable
        // even when no further batch completes to trigger eviction.
        trim_icon_cache(&mut workspace);
        assert_eq!(workspace.mod_icons.len(), ICON_CACHE + ICON_BATCH);
        workspace.icons_visible.retain(|id| id == "0");
        trim_icon_cache(&mut workspace);
        assert_eq!(workspace.mod_icons.len(), ICON_CACHE);
        assert!(workspace.mod_icons.contains_key(&format!("{:064x}", 0)));
        let updated = content_model(
            model.clone(),
            workspace
                .content
                .iter()
                .map(|item| artwork_row(&workspace, item))
                .collect(),
        );
        assert!(std::ptr::eq(model.as_any(), updated.as_any()));
        for i in 1..updated.row_count() {
            let row = updated.row_data(i).unwrap();
            assert!(!row.has_picture);
            assert!(row.picture.to_rgba8().is_none());
        }
        // A genuine absence keeps the honest letter fallback without reloading.
        workspace.mod_icons.insert(format!("{:064x}", 0), None);
        let absent = artwork_row(&workspace, &workspace.content[0]);
        assert!(!absent.has_picture);
        assert!(absent.picture.to_rgba8().is_none());
        assert_eq!(absent.letter, "M");
        assert!(visible_icon_sources(&workspace).is_empty());
    }

    #[test]
    fn content_artwork_updates_keep_row_models_stable_until_ids_change() {
        let first = ContentRow {
            id: "a".into(),
            title: "before".into(),
            ..Default::default()
        };
        let model = ui::model(vec![first.clone()]);
        let before = model.as_any() as *const dyn std::any::Any;
        let updated = content_model(
            model,
            vec![ContentRow {
                title: "after".into(),
                ..first
            }],
        );
        assert!(std::ptr::eq(before, updated.as_any()));
        assert_eq!(updated.row_data(0).unwrap().title, "after");
        let replacement = content_model(
            updated.clone(),
            vec![ContentRow {
                id: "b".into(),
                ..Default::default()
            }],
        );
        assert!(!std::ptr::eq(updated.as_any(), replacement.as_any()));
    }

    #[test]
    fn memory_steps_preserve_validation_and_clamp_to_core_bounds() {
        assert_eq!(stepped_memory_mib("4096", 1).unwrap(), 5120);
        assert_eq!(stepped_memory_mib("4096", -1).unwrap(), 3072);
        assert_eq!(stepped_memory_mib(" 2048 ", 1).unwrap(), 3072);
        assert_eq!(stepped_memory_mib("512", -1).unwrap(), 512);
        assert_eq!(stepped_memory_mib("1000", -1).unwrap(), 512);
        assert_eq!(stepped_memory_mib("32000", 1).unwrap(), 32768);
        assert_eq!(stepped_memory_mib("32768", 1).unwrap(), 32768);
        for text in ["", "bad", "4096.0", "511", "32769"] {
            assert!(stepped_memory_mib(text, 1).is_err());
            assert!(stepped_memory_mib(text, -1).is_err());
        }
    }

    #[test]
    fn custom_java_reports_read_as_one_clear_line() {
        let valid = api::JavaReport {
            path: "C:/jdk/bin/javaw.exe".into(),
            required_major_version: 21,
            valid: true,
            facts: Some(api::JavaFacts {
                major_version: 21,
                version: "21.0.4".into(),
                vendor: "Eclipse Adoptium".into(),
                architecture: "amd64".into(),
            }),
            problem: None,
        };
        let (text, tone) = java_report_text(&valid);
        assert_eq!(
            text,
            "Java 21 (Eclipse Adoptium 21.0.4, amd64). Matches the Java 21 this game requires."
        );
        assert_eq!(tone, Tone::Success);
        let invalid = api::JavaReport {
            valid: false,
            facts: None,
            problem: Some("This Java is version 25.".into()),
            ..valid
        };
        assert_eq!(
            java_report_text(&invalid),
            ("This Java is version 25.".to_owned(), Tone::Error)
        );
    }

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
