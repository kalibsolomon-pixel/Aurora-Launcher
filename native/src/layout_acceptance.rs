//! Headless layout acceptance for the NATIVE-10 surfaces.
//!
//! Renders the real `AppWindow` with Slint's software renderer (the same
//! renderer the native host ships) at several window sizes and scale factors,
//! with the new update, loader, modpack and import states filled in, and
//! drives the new controls with real pointer and key events. Images are
//! written only when `AURORA_LAYOUT_OUT` names an existing directory. These
//! are renders of the real components, not screenshots of a desktop window.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use slint::Rgb8Pixel;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Key, Platform, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{
    ComponentHandle, LogicalPosition, LogicalSize, Model, ModelRc, SharedString, VecModel,
};

use crate::{AppWindow, ContentImport, ContentRow, Discover, Instances, Nav, Page, Shell, Tone};

thread_local! {
    static ADAPTERS: RefCell<Vec<Rc<MinimalSoftwareWindow>>> = const { RefCell::new(Vec::new()) };
}

struct RenderPlatform;
impl Platform for RenderPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        ADAPTERS.with(|all| all.borrow_mut().push(window.clone()));
        Ok(window)
    }
}

fn strings(values: &[&str]) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        values
            .iter()
            .map(|v| SharedString::from(*v))
            .collect::<Vec<_>>(),
    ))
}

/// A fresh window at `logical` size and `scale`, returning its adapter.
fn window(logical: (f32, f32), scale: f32) -> (AppWindow, Rc<MinimalSoftwareWindow>) {
    let app = AppWindow::new().unwrap();
    let adapter = ADAPTERS.with(|all| all.borrow().last().cloned()).unwrap();
    app.window()
        .dispatch_event(WindowEvent::ScaleFactorChanged {
            scale_factor: scale,
        });
    app.window()
        .set_size(LogicalSize::new(logical.0, logical.1));
    app.global::<Shell>().set_loaded(true);
    app.show().unwrap();
    (app, adapter)
}

/// Renders the current frame; writes a 24-bit BMP when an output dir is set.
fn render(app: &AppWindow, adapter: &MinimalSoftwareWindow, name: &str) -> Vec<Rgb8Pixel> {
    slint::platform::update_timers_and_animations();
    let size = app.window().size();
    let (width, height) = (size.width as usize, size.height as usize);
    let mut buffer = vec![Rgb8Pixel::default(); width * height];
    adapter.request_redraw();
    adapter.draw_if_needed(|renderer| {
        renderer.render(&mut buffer, width);
    });
    if let Some(dir) = std::env::var_os("AURORA_LAYOUT_OUT") {
        let dir = Path::new(&dir);
        if dir.is_dir() {
            write_bmp(&dir.join(format!("{name}.bmp")), width, height, &buffer);
        }
    }
    buffer
}

fn write_bmp(path: &Path, width: usize, height: usize, pixels: &[Rgb8Pixel]) {
    let row = (width * 3).div_ceil(4) * 4;
    let image = row * height;
    let mut out = Vec::with_capacity(54 + image);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((54 + image) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(image as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    for y in (0..height).rev() {
        let start = out.len();
        for pixel in &pixels[y * width..(y + 1) * width] {
            out.extend_from_slice(&[pixel.b, pixel.g, pixel.r]);
        }
        out.resize(start + row, 0);
    }
    std::fs::write(path, out).unwrap();
}

fn click(app: &AppWindow, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    let window = app.window();
    window.dispatch_event(WindowEvent::PointerMoved { position });
    window.dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}

fn key(app: &AppWindow, key: Key) {
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
}

fn row(
    id: &str,
    title: &str,
    detail: &str,
    update: &str,
    tone: Tone,
    can_update: bool,
    can_unpin: bool,
) -> ContentRow {
    ContentRow {
        id: id.into(),
        title: title.into(),
        detail: detail.into(),
        enabled: true,
        toggleable: true,
        removable: true,
        note: "".into(),
        update_text: update.into(),
        update_tone: tone,
        can_update,
        can_unpin,
        is_mod: true,
        letter: crate::ui::letter(title).into(),
        ..Default::default()
    }
}

const LONG_FILE: &str = "Synthetic Aurora Extremely Long Resource Pack Name Used To Check Wrapping In Narrow Windows And Large Scale Factors v12.4.0-final.zip";

fn settings_toggle_scene(app: &AppWindow, theme: &str, background: &str, accent: &str) {
    app.global::<Nav>().set_page(Page::Settings);
    app.global::<crate::Settings>()
        .set_section(crate::SettingsSection::Accessibility);
    let palette = &aurora_launcher_lib::appearance::accent_preset(accent)
        .unwrap()
        .palette;
    crate::theme::apply(
        app,
        &crate::api::Appearance {
            aurora_motion_speed: 0,
            background: background.into(),
            theme: theme.into(),
            accent: serde_json::Value::Null,
            palette: crate::api::AccentPalette {
                accent: palette.base.as_hex(),
                accent_strong: palette.strong.as_hex(),
                accent_hover: palette.hover.as_hex(),
                accent_pressed: palette.pressed.as_hex(),
                accent_contrast: palette.on_accent.as_hex(),
                accent_soft: palette.soft_css(),
                accent_outline: palette.outline_css(),
            },
            themes: vec![],
            accents: vec![],
        },
    );
}

fn workspace(app: &AppWindow, tab: i32) {
    let nav = app.global::<Nav>();
    nav.set_page(Page::Instances);
    nav.set_workspace_open(true);
    let instances = app.global::<Instances>();
    instances.set_name("Synthetic layout instance".into());
    instances.set_subtitle("Minecraft 1.21.11 · Fabric 0.19.3 · Adrenaline 26.5.1".into());
    instances.set_status("Installed".into());
    instances.set_tone(Tone::Success);
    instances.set_tab(tab);
    instances.set_content(ModelRc::new(VecModel::from(vec![
        row(
            "a",
            "Sodium",
            "0.6.13 · CaffeineMC",
            "Update: 0.6.14",
            Tone::Accent,
            true,
            false,
        ),
        row(
            "b",
            "Lithium",
            "0.15.1 · CaffeineMC",
            "Pinned · 0.15.2 available",
            Tone::Neutral,
            false,
            true,
        ),
        row(
            "c",
            "Fabric API",
            "0.141.1 · FabricMC",
            "Up to date",
            Tone::Success,
            false,
            false,
        ),
        row(
            "d",
            "Entity Culling",
            "1.9.4 · tr7zw",
            "Managed by the modpack",
            Tone::Warning,
            false,
            false,
        ),
        row(
            "e",
            "Cloth Config API",
            "19.0.147 · shedaniel",
            "Updated with its parent",
            Tone::Neutral,
            false,
            false,
        ),
        row(
            "f",
            LONG_FILE,
            "Local file · 12.4 MB",
            "",
            Tone::Neutral,
            false,
            false,
        ),
    ])));
    instances.set_updates_summary(
        "1 update available · 1 pinned · 1 up to date · 1 need attention · 1 dependencies follow their parents".into(),
    );
    instances.set_updates_count(2);
    instances.set_cfg_versions(strings(&["1.21.11", "1.21.10", "1.21.4", "1.21.1"]));
    instances.set_cfg_version_index(0);
    instances.set_cfg_loaders(strings(&["Fabric", "Forge", "NeoForge", "Vanilla"]));
    instances.set_cfg_loader_index(0);
    instances
        .set_cfg_loader_kind_note("Changing the loader requires installing new content.".into());
    instances.set_cfg_has_loader(true);
    instances.set_cfg_loader_label("Fabric Loader version".into());
    instances.set_cfg_loader_versions(strings(&[
        "Automatic compatible version (recommended)",
        "0.19.3",
        "0.19.2",
        "0.20.0-beta.1 (unstable)",
    ]));
    instances.set_cfg_loader_version_index(1);
    instances.set_cfg_loader_note(
        "Automatic resolves a compatible loader when installed; an exact version stays pinned."
            .into(),
    );
    instances.set_cfg_needs_install(true);
    instances.set_pack_name("Adrenaline 26.5.1+mc1.21.11.fabric".into());
    instances.set_pack_update_text("26.6.0 available (installed 26.5.1+mc1.21.11.fabric)".into());
    instances.set_pack_update_available(true);
    instances.set_memory_hint(
        "Recommended: 4.0 GB. Sized for this instance's installed content.".into(),
    );
    instances.set_health_text("All installed files match their verified records.".into());
}

/// Settings scrolled to the Java group, with a custom selection.
fn java_settings(app: &AppWindow, valid: bool) {
    workspace(app, 3);
    let instances = app.global::<Instances>();
    instances.set_cfg_java_index(1);
    if valid {
        instances.set_cfg_java_path(
            r"C:\Program Files\Eclipse Adoptium\jdk-21.0.4.7-hotspot\bin\javaw.exe".into(),
        );
        instances.set_cfg_java_status(
            "Java 21 (Eclipse Adoptium 21.0.4, amd64). Matches the Java 21 this game requires."
                .into(),
        );
        instances.set_cfg_java_tone(Tone::Success);
    } else {
        instances.set_cfg_java_path(
            r"C:\Users\Example\Very Long Folder Name For Testing Narrow Layouts\Java Installations\jdk-25.0.4.101-hotspot\bin\java.exe".into(),
        );
        instances.set_cfg_java_status(
            "This Java is version 25; this Minecraft version requires Java 21. Choose a Java 21 installation or use managed Java.".into(),
        );
        instances.set_cfg_java_tone(Tone::Error);
    }
}

fn scroll_down(app: &AppWindow, pixels: f32) {
    let size = app.window().size().to_logical(app.window().scale_factor());
    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position: LogicalPosition::new(size.width / 2.0, size.height / 2.0),
        delta_x: 0.0,
        delta_y: -pixels,
    });
}

fn update_review(app: &AppWindow) {
    let instances = app.global::<Instances>();
    instances.set_update_review_title("Update 3 mods".into());
    instances.set_update_review_text(
        "Updates\nsodium-fabric-0.6.13.jar (0.6.13) → 0.6.14\nlithium-fabric-0.15.1.jar (0.15.1) → 0.15.2\niris-1.8.8.jar (1.8.8) → 1.9.0\n\nNew dependencies installed\nfabric-language-kotlin-1.13.7.jar (1.13.7)\n\nDependencies no longer required (removed)\nindium-1.0.36.jar (1.0.36)\n\nWarnings\nThis update changes the loader requirement of one item."
            .into(),
    );
    instances.set_update_review_open(true);
}

fn pack_review(app: &AppWindow) {
    let instances = app.global::<Instances>();
    instances.set_pack_review_text(
        "Adrenaline 26.5.1+mc1.21.11.fabric → 26.6.0\nMinecraft 1.21.11 → 1.21.12 · loader 0.19.5 → 0.20.1. The game is reinstalled for the new version.\n\n3 added · 14 updated · 1 removed · 22 kept as is\n\nYou changed these files and the new version changes them too. Your copies are kept; the pack's new copy is not written over them.\nconfig/sodium-options.json\nconfig/a-very-long-configuration-directory-name/with/nested/settings-file-name.properties"
            .into(),
    );
    instances.set_pack_review_conflicts(2);
    instances.set_pack_keep_local(false);
    instances.set_pack_review_allowed(true);
    instances.set_pack_review_open(true);
}

fn import_review(app: &AppWindow, blocked: bool) {
    let import = app.global::<ContentImport>();
    let mut text = format!(
        "{LONG_FILE}\nTarget: Synthetic layout instance with a long descriptive name\nType: Resource pack · 12.4 MiB\nInstalls the reviewed local bytes as user-managed content. Publisher authenticity is not established. Mods must satisfy the current loader/dependency checks; resource-pack format and shader activation are selected in Minecraft."
    );
    if blocked {
        text.push_str(
            "\nBlocked: A file with that name already exists. Nothing will be overwritten.",
        );
    }
    import.set_description(text.into());
    import.set_allowed(!blocked);
    import.set_open(true);
}

fn create_dialog(app: &AppWindow) {
    let instances = app.global::<Instances>();
    instances.set_versions(strings(&["1.21.11", "1.21.10", "1.21.4"]));
    instances.set_version_index(0);
    instances.set_loaders(strings(&["Fabric", "Forge", "NeoForge", "Vanilla"]));
    instances.set_loader_index(0);
    instances.set_create_name("Instance 13".into());
    instances.set_create_has_loader(true);
    instances.set_create_loader_label("Fabric Loader version".into());
    instances.set_create_loader_versions(strings(&[
        "Automatic compatible version (recommended)",
        "0.19.3",
        "0.19.2",
    ]));
    instances.set_create_loader_index(1);
    instances.set_aurora_note("Aurora 3.0.0 will be installed and kept with this instance.".into());
    instances.set_create_open(true);
}

fn local_pack(app: &AppWindow) {
    app.global::<Nav>().set_page(Page::Discover);
    let discover = app.global::<Discover>();
    discover.set_kind(3);
    discover.set_details_open(true);
    discover.set_project_title("Adrenaline".into());
    discover.set_project_summary("A performance-focused Fabric modpack.".into());
    discover.set_local_pack_note(
        "Local file Adrenaline 26.5.1 local copy.mrpack is Adrenaline 26.5.1+mc1.21.11.fabric on Modrinth (exact SHA-512 match). Its verified local bytes are used for this version; choosing another version downloads that one from Modrinth instead.\nPack: Adrenaline 26.5.1+mc1.21.11.fabric · Minecraft 1.21.11 · Fabric 0.19.5 · 6.9 KiB"
            .into(),
    );
    discover.set_versions(strings(&[
        "Adrenaline 26.5.1 · 26.5.1+mc1.21.11.fabric · fabric",
    ]));
    discover.set_version_index(0);
}

const SIZES: [(&str, (f32, f32)); 3] = [
    ("min-720x520", (720.0, 520.0)),
    ("typical-1120x760", (1120.0, 760.0)),
    ("large-1600x1000", (1600.0, 1000.0)),
];
const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

fn home_card(app: &AppWindow, attention: bool) {
    app.global::<Nav>().set_page(Page::Home);
    app.global::<Nav>().set_workspace_open(false);
    let home = app.global::<crate::Home>();
    home.set_instances(ModelRc::new(VecModel::from(vec![crate::InstanceRow {
        id: "home-instance".into(),
        name: "Vulkan 11".into(),
        ..Default::default()
    }])));
    home.set_has_instance(true);
    home.set_name("Vulkan 11".into());
    home.set_instance_names(strings(&["Vulkan 11"]));
    home.set_instance_index(0);
    home.set_status(
        if attention {
            "Needs Attention"
        } else {
            "Ready"
        }
        .into(),
    );
    home.set_tone(if attention {
        Tone::Warning
    } else {
        Tone::Success
    });
    home.set_can_play(!attention);
    home.set_message(
        if attention {
            "A detailed launch failure belongs in instance settings."
        } else {
            "Last game exited with code 0."
        }
        .into(),
    );
    home.set_blockers(ModelRc::new(VecModel::from(vec![crate::BlockerRow {
        code: "launch_instance_damaged".into(),
        message: "Detailed repair warning belongs in instance settings.".into(),
        action: "Review recovery".into(),
    }])));
    home.set_progress_text("Detailed installation status belongs in settings.".into());
}

type Scene = fn(&AppWindow);

fn dashboard(app: &AppWindow, editing: bool, enlarged: bool) {
    home_card(app, false);
    let widgets = app.global::<crate::Widgets>();
    widgets.set_editing(editing);
    let columns = (((app.window().size().width as f32 / app.window().scale_factor() - 120.0 + 14.0)
        / 164.0)
        .floor() as i32)
        .clamp(1, 6);
    let layout = crate::api::HomeLayout {
        widgets: vec![
            crate::api::WidgetPlacement {
                id: "playtime".into(),
                enabled: true,
                size: "small".into(),
                grid: Some(crate::api::WidgetGrid {
                    columns: if enlarged { 4 } else { 2 },
                    rows: if enlarged { 2 } else { 1 },
                    position: None,
                }),
            },
            crate::api::WidgetPlacement {
                id: "media".into(),
                enabled: true,
                size: "small".into(),
                grid: None,
            },
        ],
    };
    let positions = crate::widgets::grid_positions(&layout, columns);
    widgets.set_grid_rows(
        positions
            .iter()
            .map(|(_, r, _, h)| r + h)
            .max()
            .unwrap_or(0),
    );
    widgets.set_items(ui_model(
        layout
            .widgets
            .iter()
            .zip(&positions)
            .map(|(w, (col, row, width, height))| crate::WidgetView {
                id: w.id.clone().into(),
                title: if w.id == "playtime" {
                    "Playtime"
                } else {
                    "Media"
                }
                .into(),
                column: *col,
                row: *row,
                width: *width,
                height: *height,
                requested_width: w.grid.as_ref().map_or(2, |g| g.columns as i32),
                requested_height: w.grid.as_ref().map_or(1, |g| g.rows as i32),
                lines: ui_model(
                    ["12 min", "2 h", "8 h", "24 h"]
                        .into_iter()
                        .map(|value| crate::WidgetLine {
                            value: value.into(),
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            })
            .collect(),
    ));
    widgets.set_names(strings(&["Playtime", "Media"]));
    widgets.set_positions(strings(&["Position 1", "Position 2"]));
    widgets.set_available_names(strings(&["Servers"]));
    widgets.set_available(ui_model(vec![crate::WidgetChoice {
        id: "recent-servers".into(),
        title: "Servers".into(),
        ..Default::default()
    }]));
    let history: Vec<_> = (0..30)
        .map(|day| crate::api::DailyPlaytime {
            day: 20000 + day,
            duration_ms: if day % 6 == 0 {
                0
            } else {
                (day % 4 + 1) * 600_000
            },
        })
        .collect();
    widgets.set_week(ui_model(crate::widgets::chart_points(&history, 7)));
    widgets.set_month(ui_model(crate::widgets::chart_points(&history, 30)));
    widgets.set_week_peak("40 min".into());
    widgets.set_month_peak("40 min".into());
    widgets.set_media_supported(true);
    widgets.set_media_active(true);
    widgets.set_media_title("Synthetic track".into());
    widgets.set_media_artist("Synthetic artist".into());
    widgets.set_media_source("Media app".into());
}

fn ui_model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn instance_cards(app: &AppWindow) {
    app.global::<Nav>().set_page(Page::Instances);
    app.global::<Nav>().set_workspace_open(false);
    app.global::<Instances>()
        .set_rows(ModelRc::new(VecModel::from(vec![
            crate::InstanceRow {
                id: "vulkan".into(),
                name: "Vulkan 11".into(),
                subtitle: "Minecraft 1.21.11".into(),
                status: "Installed".into(),
                icon: crate::symbols::path_or_fallback(Some("bolt")).into(),
                selected: true,
                ..Default::default()
            },
            crate::InstanceRow {
                id: "optimized".into(),
                name: "Fabulously Optimized".into(),
                subtitle: "Minecraft 1.21.11".into(),
                status: "Needs repair".into(),
                icon: crate::symbols::path_or_fallback(None).into(),
                ..Default::default()
            },
        ])));
}

fn responsive_widget(app: &AppWindow, id: &str, columns: i32, rows: i32) {
    dashboard(app, false, false);
    let widgets = app.global::<crate::Widgets>();
    let available = (((app.window().size().width as f32 / app.window().scale_factor() - 106.0)
        / 164.0)
        .floor() as i32)
        .clamp(1, 6);
    let mut item = widgets
        .get_items()
        .iter()
        .find(|w| w.id == id)
        .unwrap_or(crate::WidgetView {
            id: id.into(),
            title: "Servers".into(),
            empty: "No servers yet.".into(),
            ..Default::default()
        });
    if matches!(id, "recent-worlds" | "recent-servers") {
        let world = id == "recent-worlds";
        item.title = if world { "Worlds" } else { "Servers" }.into();
        item.lines = ui_model(
            (0..8)
                .map(|i| crate::WidgetLine {
                    label: if world {
                        format!("Workshop world {i}")
                    } else {
                        format!("Workshop server {i}")
                    }
                    .into(),
                    detail: "Minecraft 1.21.11".into(),
                    value: if !world && i == 0 {
                        "Today · Online"
                    } else {
                        "Today"
                    }
                    .into(),
                    target_id: format!("fixture-{i}").into(),
                    playable: i != 2,
                    favorite: !world && i == 0,
                    ..Default::default()
                })
                .collect(),
        );
    }
    item.column = 0;
    item.row = 0;
    item.width = columns.min(available);
    item.height = rows;
    item.requested_width = columns;
    item.requested_height = rows;
    widgets.set_items(ui_model(vec![item]));
    widgets.set_grid_rows(rows);
    scroll_down(app, 280.);
}

fn saved_destination_scene(app: &AppWindow, worlds: bool) {
    let id = if worlds {
        "recent-worlds"
    } else {
        "recent-servers"
    };
    responsive_widget(app, id, 3, 2);
    let widgets = app.global::<crate::Widgets>();
    let mut item = widgets.get_items().row_data(0).unwrap();
    item.icon =
        crate::symbols::path_or_fallback(Some(if worlds { "landscape" } else { "dns" })).into();
    item.lines = ui_model(
        item.lines
            .iter()
            .enumerate()
            .map(|(index, mut row)| {
                row.value = if worlds {
                    format!("Workshop {index} · {} days ago", index + 1)
                } else {
                    format!("server{index}.invalid:25565")
                }
                .into();
                row.detail = if worlds {
                    "Vulkan 11 · Survival · 1.21.11"
                } else {
                    "Vulkan 11"
                }
                .into();
                row
            })
            .collect(),
    );
    widgets.set_items(ui_model(vec![item]));
    widgets.set_saved_servers(!worlds);
    widgets.set_saved_worlds(worlds);
}

fn scenes() -> Vec<(&'static str, Scene)> {
    vec![
        ("responsive-media-small", |app| {
            responsive_widget(app, "media", 1, 1)
        }),
        ("responsive-media-wide", |app| {
            responsive_widget(app, "media", 4, 1)
        }),
        ("responsive-media-large", |app| {
            responsive_widget(app, "media", 4, 2)
        }),
        ("responsive-media-tall", |app| {
            responsive_widget(app, "media", 2, 3)
        }),
        ("responsive-empty-tall", |app| {
            responsive_widget(app, "recent-servers", 2, 3)
        }),
        ("responsive-list-small", |app| {
            responsive_widget(app, "recent-worlds", 1, 1)
        }),
        ("responsive-list-tall", |app| {
            responsive_widget(app, "recent-worlds", 2, 3)
        }),
        ("responsive-chart-narrow", |app| {
            responsive_widget(app, "playtime", 1, 3)
        }),
        ("ui2-servers", |app| {
            responsive_widget(app, "recent-servers", 3, 2)
        }),
        ("ui2-worlds", |app| {
            responsive_widget(app, "recent-worlds", 3, 2)
        }),
        ("ui2-saved-servers", |app| {
            saved_destination_scene(app, false);
        }),
        ("ui2-saved-worlds", |app| {
            saved_destination_scene(app, true);
        }),
        ("ui2-playtime-30d", |app| {
            responsive_widget(app, "playtime", 3, 1);
            app.global::<crate::Widgets>().set_playtime_period(1);
        }),
        ("ui2-playtime-30d-narrow", |app| {
            responsive_widget(app, "playtime", 1, 2);
            let widgets = app.global::<crate::Widgets>();
            let points = [0, 2, 1, 0, 3, 2]
                .into_iter()
                .enumerate()
                .map(|(index, hours)| {
                    let back = (5 - index) * 5;
                    let label = if back == 0 {
                        "Last 5 days".into()
                    } else {
                        format!("{}–{} days ago", back + 4, back)
                    };
                    crate::PlaytimePoint {
                        label: label.clone().into(),
                        detail: format!("{label} (UTC), {hours} h").into(),
                        value: format!("{hours} h").into(),
                        fraction: hours as f32 / 3.,
                    }
                })
                .collect();
            widgets.set_month_binned(ui_model(points));
            widgets.set_month_binned_peak("3 h".into());
            widgets.set_playtime_range(1);
        }),
        ("ui2-playtime-all", |app| {
            responsive_widget(app, "playtime", 3, 1);
            let widgets = app.global::<crate::Widgets>();
            widgets.set_month_binned(ui_model(
                [4, 0, 2, 1, 6, 3, 8]
                    .into_iter()
                    .enumerate()
                    .map(|(index, hours)| {
                        let ago = (6 - index) * 4;
                        let label = if ago == 0 {
                            "Last 4 days".into()
                        } else {
                            format!("{}–{} days ago", ago + 3, ago)
                        };
                        crate::PlaytimePoint {
                            label: label.clone().into(),
                            detail: format!("{label} (UTC), {hours} h").into(),
                            value: format!("{hours} h").into(),
                            fraction: hours as f32 / 8.,
                        }
                    })
                    .collect(),
            ));
            widgets.set_month_binned_peak("8 h".into());
            widgets.set_lifetime_total("32 h".into());
            widgets
                .set_lifetime_detail("32 h recorded in total; chart shows the last 30 days".into());
            widgets.set_playtime_range(2);
        }),
        ("dashboard-compact", |app| dashboard(app, false, false)),
        ("dashboard-large", |app| dashboard(app, false, true)),
        ("dashboard-editor", |app| dashboard(app, true, true)),
        ("window-controls", |app| {
            app.global::<crate::Chrome>().set_custom(true);
            home_card(app, false);
        }),
        ("modpack-loader-filter", |app| {
            app.global::<Nav>().set_page(Page::Discover);
            app.global::<Discover>().set_kind(3);
            app.global::<Discover>().set_loader_index(3);
        }),
        ("instance-cards-simple", instance_cards),
        ("mods-updates", |app| workspace(app, 0)),
        ("home-ready-simple", |app| home_card(app, false)),
        ("home-attention-simple", |app| home_card(app, true)),
        ("settings-launch-details", |app| {
            home_card(app, true);
            workspace(app, 3);
            app.global::<Instances>().set_selected(true);
            scroll_down(app, 1050.);
        }),
        ("recovery-local-edits", |app| {
            workspace(app, 3);
            let recovery = app.global::<crate::Recovery>();
            recovery.set_title("Recovery: Vulkan 11".into());
            recovery.set_text("Keep 10 changed settings file(s), review 1 removed mod(s), then verify the installation. No game files will be overwritten.\n\nPreserve local settings:\nconfig/asyncparticles/asyncparticles-mixin.properties\nconfig/c2me.toml\nconfig/fabric/indigo-renderer.properties\nconfig/ferritecore.mixin.properties\nconfig/ixeris.toml\nconfig/packetfixer.properties\nconfig/vmp.properties\nconfig/vulkanmod_settings.json\nconfig/zfastnoise.mixin.toml\noptions.txt\n\nRemoved mods (kept removed only if you confirm below):\nmods/clientsort-fabric-2.2.2+1.21.11.jar".into());
            recovery.set_removed_count(1);
            recovery.set_keep_removed(false);
            recovery.set_allowed(true);
            recovery.set_open(true);
        }),
        ("recovery-blocked", |app| {
            workspace(app, 0);
            let recovery = app.global::<crate::Recovery>();
            recovery.set_title("Recovery: test instance".into());
            recovery.set_text("Recovery found issues requiring attention. Your files remain unchanged.\n\nNeeds attention:\nmods/changed.jar: installed bytes changed. Restore or review this file explicitly; recovery will not trust changed mod artifacts.".into());
            recovery.set_allowed(false);
            recovery.set_open(true);
        }),
        ("mods-icons-controls", |app| {
            workspace(app, 0);
            let mut icon_row = row(
                "icon",
                "Fabric API",
                "0.141.6 · FabricMC",
                "",
                Tone::Neutral,
                false,
                false,
            );
            icon_row.picture =
                crate::ui::rectangular_image(&[96, 160, 220, 255].repeat(32 * 32), 32, 32).unwrap();
            icon_row.has_picture = true;
            let mut local = row(
                "fallback",
                "Local mod without icon",
                "1.0",
                "",
                Tone::Neutral,
                false,
                false,
            );
            local.note =
                "Another mod requires this dependency. Disabling it may prevent launch.".into();
            app.global::<Instances>()
                .set_content(ModelRc::new(VecModel::from(vec![icon_row, local])));
        }),
        ("aurora-add-settings", |app| {
            workspace(app, 3);
            app.global::<Instances>().set_client_available(true);
            app.global::<Instances>().set_cfg_needs_install(false);
            app.global::<Instances>()
                .set_client_note("Latest compatible: 3.0.0".into());
            scroll_down(app, 650.);
        }),
        ("aurora-review", |app| {
            workspace(app, 3);
            let instances = app.global::<Instances>();
            instances.set_client_review_text("Add: Aurora Client 3.0.0\n\nThe existing verified Fabric API remains provider-managed.".into());
            instances.set_client_review_allowed(true);
            instances.set_client_review_open(true);
        }),
        ("refinement-discover-filters", |app| {
            app.global::<Nav>().set_page(Page::Discover);
            let discover = app.global::<Discover>();
            discover.set_kind(3);
            discover.set_category_names(strings(&["All categories", "optimization", "adventure"]));
            discover.set_category_index(1);
            discover.set_sort_index(4);
        }),
        ("refinement-saved-skin", |app| {
            app.global::<Nav>().set_page(Page::Skins);
            app.global::<crate::Skins>()
                .set_presets(ModelRc::new(VecModel::from(vec![crate::SkinRow {
                    id: "saved-one".into(),
                    name: "Saved skin".into(),
                    edit_name: "Saved skin".into(),
                    model: "Slim".into(),
                    ..Default::default()
                }])));
        }),
        ("pandora4-instance-overrides", |app| {
            workspace(app, 3);
            let sync = app.global::<crate::InstanceSync>();
            sync.set_loaded(true);
            sync.set_instance_participating(true);
            sync.set_instance_categories(ModelRc::new(VecModel::from(vec![
                crate::SyncOverrideChoice {
                    name: "Resource packs".into(),
                    mode: 0,
                },
                crate::SyncOverrideChoice {
                    name: "Controls".into(),
                    mode: 1,
                },
                crate::SyncOverrideChoice {
                    name: "Server list".into(),
                    mode: 2,
                },
            ])));
            sync.set_instance_exclusions("resourcepacks/private; config/local".into());
            scroll_down(app, 1150.);
        }),
        ("pandora4-conflicts", |app| {
            app.global::<Nav>().set_page(Page::Settings);
            app.global::<crate::Settings>()
                .set_section(crate::SettingsSection::Syncing);
            let sync = app.global::<crate::InstanceSync>();
            sync.set_loaded(true);
            sync.set_enabled(true);
            sync.set_message("Synced 2 files. 2 conflicts remain pending.".into());
            sync.set_conflicts(ModelRc::new(VecModel::from(vec![
                crate::SyncConflictRow {
                    label: "resourcepacks/Shared-Pack.zip".into(),
                    peers: "First: Survival · Second: Creative".into(),
                    resolvable: true,
                },
                crate::SyncConflictRow {
                    label: "servers.dat".into(),
                    peers: "First: Survival · Second: Creative".into(),
                    resolvable: false,
                },
            ])));
            scroll_down(app, 1000.);
        }),
        ("ui1-settings", |app| {
            app.global::<Nav>().set_page(Page::Settings)
        }),
        ("ui1-toggle-simple", |app| {
            settings_toggle_scene(app, "aurora-dark", "simple", "violet")
        }),
        ("ui1-toggle-oled", |app| {
            settings_toggle_scene(app, "oled", "simple", "violet")
        }),
        ("ui1-toggle-borealis", |app| {
            settings_toggle_scene(app, "aurora-dark", "borealis", "violet")
        }),
        ("ui1-accounts", |app| {
            let accounts = app.global::<crate::Accounts>();
            accounts.set_has_account(true);
            accounts.set_rows(ModelRc::new(VecModel::from(vec![
                crate::AccountRow {
                    name: "Active fixture".into(),
                    status: "Ready to play".into(),
                    selected: true,
                    initial: "A".into(),
                    ..Default::default()
                },
                crate::AccountRow {
                    name: "Second fixture".into(),
                    status: "Sign in again".into(),
                    initial: "S".into(),
                    ..Default::default()
                },
            ])));
            accounts.set_dialog_open(true);
        }),
        ("ui1-downloads", |app| {
            let activity = app.global::<crate::Activity>();
            activity.set_summary("1 active · 1 queued".into());
            activity.set_active(2);
            activity.set_rows(ModelRc::new(VecModel::from(
                ["Downloading", "Queued", "Completed", "Failed"]
                    .iter()
                    .map(|phase| crate::ActivityRow {
                        title: format!("{phase} fixture").into(),
                        phase: (*phase).into(),
                        message: "Synthetic download activity".into(),
                        running: *phase == "Downloading",
                        can_cancel: matches!(*phase, "Downloading" | "Queued"),
                        can_retry: *phase == "Failed",
                        ..Default::default()
                    })
                    .collect::<Vec<_>>(),
            )));
            activity.set_open(true);
        }),
        ("ui1-long-mods", |app| {
            workspace(app, 0);
            app.global::<Instances>()
                .set_content(ModelRc::new(VecModel::from(
                    (0..120)
                        .map(|i| ContentRow {
                            id: format!("fixture-{i}").into(),
                            title: format!("Mod {i}").into(),
                            detail: LONG_FILE.into(),
                            letter: "M".into(),
                            is_mod: true,
                            enabled: true,
                            toggleable: true,
                            removable: true,
                            ..Default::default()
                        })
                        .collect::<Vec<_>>(),
                )));
        }),
        ("ui1-long-skins", |app| {
            app.global::<Nav>().set_page(Page::Skins);
            app.global::<crate::Skins>()
                .set_presets(ModelRc::new(VecModel::from(
                    (0..120)
                        .map(|i| crate::SkinRow {
                            id: format!("fixture-{i}").into(),
                            name: "Duplicate name".into(),
                            model: "Classic".into(),
                            favorite: i % 2 == 0,
                            ..Default::default()
                        })
                        .collect::<Vec<_>>(),
                )));
        }),
        ("settings-game", |app| workspace(app, 3)),
        ("create-loader", |app| create_dialog(app)),
        ("update-review", |app| {
            workspace(app, 0);
            update_review(app);
        }),
        ("pack-review", |app| {
            workspace(app, 3);
            pack_review(app);
        }),
        ("import-review-blocked", |app| {
            workspace(app, 1);
            import_review(app, true);
        }),
        ("import-review-allowed", |app| {
            workspace(app, 1);
            import_review(app, false);
        }),
        ("local-mrpack-review", |app| local_pack(app)),
        ("pack-version-choice", |app| {
            crate::discover::tests::layout_scene(app, false, false)
        }),
        ("pack-review-new-instance", |app| {
            crate::discover::tests::layout_scene(app, false, true)
        }),
        ("pack-destination-existing", |app| {
            crate::discover::tests::layout_scene(app, true, false)
        }),
        ("pack-review-existing", |app| {
            crate::discover::tests::layout_scene(app, true, true)
        }),
        ("settings-java-valid", |app| {
            java_settings(app, true);
            scroll_down(app, 330.0);
        }),
        ("settings-java-invalid", |app| {
            java_settings(app, false);
            scroll_down(app, 330.0);
        }),
    ]
}

#[test]
fn discover_download_notification_clears_pagination_at_every_size_and_scale() {
    let _ = slint::platform::set_platform(Box::new(RenderPlatform));
    for (width, height) in [(720., 520.), (1120., 760.), (1600., 1000.)] {
        for scale in [1., 1.25, 1.5, 2.] {
            let (app, adapter) = window((width, height), scale);
            app.global::<Nav>().set_page(Page::Discover);
            let discover = app.global::<Discover>();
            discover.set_kind(1);
            discover.set_has_instances(true);
            discover.set_instance_names(strings(&["Disposable fixture"]));
            discover.set_has_more(true);
            discover.set_results(ModelRc::new(VecModel::from(
                (0..30)
                    .map(|i| crate::ProjectRow {
                        id: format!("fixture-{i}").into(),
                        title: format!("Resource pack {i}").into(),
                        description: "Synthetic download layout fixture".into(),
                        letter: "R".into(),
                        ..Default::default()
                    })
                    .collect::<Vec<_>>(),
            )));
            let more = Rc::new(RefCell::new(0));
            let observed = more.clone();
            discover.on_more(move || *observed.borrow_mut() += 1);
            let shell = app.global::<Shell>();
            let started = Rc::new(RefCell::new(Vec::new()));
            let observed = started.clone();
            let weak = app.as_weak();
            // Synthetic operation: real Install pointer route, no network or owner data.
            discover.on_install(move |id| {
                observed.borrow_mut().push(id.to_string());
                let app = weak.unwrap();
                app.global::<Shell>()
                    .set_toast_text("Resource pack added to Downloads.".into());
                app.global::<Shell>().set_toast_visible(true);
            });
            render(&app, &adapter, &format!("download-before-{width}-{scale}"));
            click(&app, width - 94., 280.);
            assert_eq!(*started.borrow(), ["fixture-0"]);
            let weak = app.as_weak();
            shell
                .on_dismiss_toast(move || weak.unwrap().global::<Shell>().set_toast_visible(false));
            let pixels = render(&app, &adapter, &format!("download-right-{width}-{scale}"));
            let physical_width = app.window().size().width as usize;
            // Flood just the toast surface: pagination/buttons share its color.
            let raised = |i: usize| (pixels[i].r, pixels[i].g, pixels[i].b) == (29, 31, 39);
            let seed = (0..pixels.len())
                .rev()
                .find(|&i| {
                    i % physical_width > physical_width - (40. * scale) as usize && raised(i)
                })
                .expect("notification must render at bottom-right");
            let mut pending = vec![seed];
            let mut visited = vec![false; pixels.len()];
            let mut bounds = (usize::MAX, usize::MAX, 0, 0);
            while let Some(i) = pending.pop() {
                if visited[i] || !raised(i) {
                    continue;
                }
                visited[i] = true;
                let (x, y) = (i % physical_width, i / physical_width);
                bounds = (
                    bounds.0.min(x),
                    bounds.1.min(y),
                    bounds.2.max(x),
                    bounds.3.max(y),
                );
                if x > 0 {
                    pending.push(i - 1);
                }
                if x + 1 < physical_width {
                    pending.push(i + 1);
                }
                if i >= physical_width {
                    pending.push(i - physical_width);
                }
                if i + physical_width < pixels.len() {
                    pending.push(i + physical_width);
                }
            }
            assert_ne!(bounds.0, usize::MAX, "notification must render");
            let (left, top, right, bottom) = (
                bounds.0 as f32 / scale,
                bounds.1 as f32 / scale,
                (bounds.2 + 1) as f32 / scale,
                (bounds.3 + 1) as f32 / scale,
            );
            assert!((width - right - 16.).abs() < 3., "right: {bounds:?}");
            assert!((height - bottom - 16.).abs() < 3., "bottom: {bounds:?}");
            assert!(
                left > (width + 72.) / 2. + 60.,
                "pagination must stay clear"
            );
            assert!(top > 44. && left >= 88.);
            // Scroll the real results viewport with the notification visible.
            scroll_down(&app, 4000.);
            render(&app, &adapter, "download-scrolled");
            // Actual pointer dispatch through the transparent space beside the toast.
            click(&app, (width + 72.) / 2., height - 50.);
            assert_eq!(*more.borrow(), 1, "Show more blocked at {width} / {scale}");
            // Replacing a notification uses the existing single slot, not a growing stack.
            shell.set_toast_text("Installation completed and verified.".into());
            shell.set_toast_tone(Tone::Success);
            render(&app, &adapter, "download-completed");
            click(&app, width - 39., height - 39.);
            assert!(!shell.get_toast_visible(), "Dismiss must remain clickable");
            // Resize/restore and change DPI on the same window, without a desktop origin.
            app.window()
                .dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.5 });
            app.window().set_size(LogicalSize::new(1120., 760.));
            shell.set_toast_visible(true);
            render(&app, &adapter, "download-restored");
            click(&app, 1081., 721.);
            assert!(!shell.get_toast_visible());
        }
    }
    let (app, adapter) = window((1120., 760.), 1.);
    let activity = app.global::<crate::Activity>();
    activity.set_active(1);
    activity.set_summary("1 active · 50% downloaded".into());
    activity.set_rows(ModelRc::new(VecModel::from(vec![
        crate::ActivityRow {
            id: 1,
            title: "Resource pack".into(),
            phase: "Downloading".into(),
            message: "50% downloaded".into(),
            running: true,
            can_cancel: true,
            ..Default::default()
        },
        crate::ActivityRow {
            id: 2,
            title: "Second download".into(),
            phase: "Failed".into(),
            message: "Retry available".into(),
            can_retry: true,
            ..Default::default()
        },
    ])));
    let cancelled = Rc::new(RefCell::new(Vec::new()));
    let observed = cancelled.clone();
    activity.on_cancel(move |id| observed.borrow_mut().push(id));
    let retried = Rc::new(RefCell::new(Vec::new()));
    let observed = retried.clone();
    activity.on_retry(move |id| observed.borrow_mut().push(id));
    activity.set_open(true);
    render(&app, &adapter, "download-progress-controls");
    key(&app, Key::Tab); // Close
    key(&app, Key::Tab); // Cancel first download
    key(&app, Key::Return);
    assert_eq!(*cancelled.borrow(), [1]);
    key(&app, Key::Tab); // Retry second download
    key(&app, Key::Return);
    assert_eq!(*retried.borrow(), [2]);
}

/// Each step of the Modrinth pack dialog, from the real Discover presenter,
/// also scrolled to its review lists (named BMPs with `AURORA_LAYOUT_OUT`).
#[test]
fn pack_flow_dialog_renders_each_step() {
    let _ = slint::platform::set_platform(Box::new(RenderPlatform));
    for (scene, merge, review) in [
        ("pack-version-choice", false, false),
        ("pack-review-new-instance", false, true),
        ("pack-destination-existing", true, false),
        ("pack-review-existing", true, true),
    ] {
        let (app, adapter) = window((1120., 1000.), 1.);
        crate::discover::tests::layout_scene(&app, merge, review);
        let pixels = render(&app, &adapter, &format!("{scene}-1120x1000"));
        assert!(
            pixels
                .iter()
                .any(|p| p.r != pixels[0].r || p.g != pixels[0].g),
            "{scene} rendered blank"
        );
        let discover = app.global::<Discover>();
        assert_eq!(discover.get_can_install(), review, "{scene}");
        if review {
            scroll_down(&app, 320.);
            render(&app, &adapter, &format!("{scene}-1120x1000-scrolled"));
        }
    }
}

#[test]
fn skin_favorite_controls_use_stable_identity_for_pointer_and_keyboard() {
    let _ = slint::platform::set_platform(Box::new(RenderPlatform));
    let (app, adapter) = window((1120., 760.), 1.);
    scenes()
        .into_iter()
        .find(|(name, _)| *name == "refinement-saved-skin")
        .unwrap()
        .1(&app);
    render(&app, &adapter, "ui1-skin-before-favorite");
    // Exercise the star through both input paths, retaining stable identity.
    let presets = app.global::<crate::Skins>().get_presets();
    let favorites: Rc<RefCell<Vec<(String, bool)>>> = Rc::default();
    let observed = favorites.clone();
    let weak = app.as_weak();
    app.global::<crate::Skins>()
        .on_set_favorite(move |id, favorite| {
            observed.borrow_mut().push((id.to_string(), favorite));
            let app = weak.unwrap();
            let model = app.global::<crate::Skins>().get_presets();
            let mut row = model.row_data(0).unwrap();
            row.favorite = favorite;
            model.set_row_data(0, row);
        });
    click(&app, 480., 334.);
    render(&app, &adapter, "ui1-skin-favorited");
    assert!(presets.row_data(0).unwrap().favorite);
    key(&app, Key::Return);
    assert_eq!(
        *favorites.borrow(),
        [("saved-one".into(), true), ("saved-one".into(), false)]
    );
    click(&app, 457., 247.);
    assert!(app.global::<crate::Skins>().get_favorites_only());
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: " ".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: " ".into() });
    assert!(!app.global::<crate::Skins>().get_favorites_only());
}

#[test]
fn content_artwork_tracks_the_visible_window_on_long_lists() {
    let _ = slint::platform::set_platform(Box::new(RenderPlatform));
    let (app, adapter) = window((720., 520.), 1.);
    let ranges: Rc<RefCell<Vec<(String, bool)>>> = Rc::default();
    let observed = ranges.clone();
    app.global::<Instances>()
        .on_content_icon_visible(move |_, id, visible| {
            observed.borrow_mut().push((id.to_string(), visible));
        });
    scenes()
        .into_iter()
        .find(|(name, _)| *name == "ui1-long-mods")
        .unwrap()
        .1(&app);
    render(&app, &adapter, "ui1-artwork-top");
    assert!(
        ranges
            .borrow()
            .iter()
            .any(|(id, visible)| id == "fixture-0" && *visible)
    );
    scroll_down(&app, 5_000.);
    render(&app, &adapter, "ui1-artwork-scrolled");
    render(&app, &adapter, "ui1-artwork-scrolled-settled");
    assert!(ranges.borrow().iter().any(|(id, visible)| {
        *visible
            && id
                .strip_prefix("fixture-")
                .and_then(|id| id.parse::<usize>().ok())
                .is_some_and(|index| index > 30)
    }));
}

#[test]
fn anchored_drawers_dismiss_by_escape_and_close_control() {
    let _ = slint::platform::set_platform(Box::new(RenderPlatform));
    let (app, adapter) = window((720., 520.), 1.);
    scenes()
        .into_iter()
        .find(|(name, _)| *name == "ui1-accounts")
        .unwrap()
        .1(&app);
    render(&app, &adapter, "ui1-account-dismissal");
    key(&app, Key::Escape);
    assert!(!app.global::<crate::Accounts>().get_dialog_open());
    scenes()
        .into_iter()
        .find(|(name, _)| *name == "ui1-downloads")
        .unwrap()
        .1(&app);
    render(&app, &adapter, "ui1-download-dismissal");
    click(&app, 492., 55.);
    assert!(!app.global::<crate::Activity>().get_open());
}

#[test]
fn settings_sections_accept_keyboard_navigation_and_preserve_theme_accents() {
    let _ = slint::platform::set_platform(Box::new(RenderPlatform));
    let (app, adapter) = window((720., 520.), 1.);
    app.global::<Nav>().set_page(Page::Settings);
    render(&app, &adapter, "ui1-settings-navigation");
    click(&app, 112., 95.);
    assert_eq!(
        app.global::<crate::Settings>().get_section(),
        crate::SettingsSection::Account
    );
    key(&app, Key::Tab);
    key(&app, Key::Return);
    assert_eq!(
        app.global::<crate::Settings>().get_section(),
        crate::SettingsSection::Discord
    );
    for (theme, background) in [
        ("aurora-dark", "simple"),
        ("oled", "simple"),
        ("aurora-dark", "borealis"),
    ] {
        for accent in aurora_launcher_lib::appearance::accent_presets() {
            settings_toggle_scene(&app, theme, background, accent.id);
            let palette = app.global::<crate::Theme>();
            assert!(palette.get_control_track().red() > palette.get_border_strong().red());
            assert_eq!(
                palette.get_accent_strong(),
                crate::theme::parse_color(&accent.palette.strong.as_hex()).unwrap()
            );
        }
    }
}

#[test]
fn ui2_saved_tab_and_quick_play_route_the_saved_row_identity() {
    let _ = slint::platform::set_platform(Box::new(RenderPlatform));
    let (app, adapter) = window((1120., 760.), 1.);
    saved_destination_scene(&app, false);
    let widgets = app.global::<crate::Widgets>();
    widgets.set_saved_servers(false);
    let views = Rc::new(RefCell::new(Vec::new()));
    let view_events = views.clone();
    widgets.on_saved_view(move |worlds, saved| view_events.borrow_mut().push((worlds, saved)));
    let plays = Rc::new(RefCell::new(Vec::new()));
    let play_events = plays.clone();
    widgets.on_quick_play(move |mode, id| {
        play_events
            .borrow_mut()
            .push((mode.to_string(), id.to_string()));
    });
    let _ = render(&app, &adapter, "ui2-saved-bindings");
    click(&app, 422., 372.);
    assert_eq!(*views.borrow(), [(false, true)]);
    click(&app, 560., 416.);
    assert_eq!(
        *plays.borrow(),
        [("saved-server".into(), "fixture-0".into())]
    );
}

/// One scenario per process so working set and CPU can be sampled externally.
#[test]
#[ignore = "opt-in optimized UI-2 resource sample"]
fn ui2_resource_scene() {
    let scenario = std::env::var("AURORA_UI2_RESOURCE_SCENE").unwrap();
    let ready = std::env::var_os("AURORA_UI2_RESOURCE_READY").unwrap();
    slint::platform::set_platform(Box::new(RenderPlatform)).unwrap();
    let (app, adapter) = window((1120., 760.), 1.);
    let scene = if scenario == "navigation" {
        "ui2-saved-servers"
    } else {
        &scenario
    };
    let (_, setup) = scenes()
        .into_iter()
        .find(|(name, _)| *name == scene)
        .unwrap();
    setup(&app);
    let _ = render(&app, &adapter, "resource-setup");
    std::fs::write(ready, std::process::id().to_string()).unwrap();
    if scenario == "navigation" {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let nav = app.global::<Nav>();
        for _ in 0..40 {
            nav.set_page(Page::Instances);
            let _ = render(&app, &adapter, "resource-away");
            nav.set_page(Page::Home);
            let _ = render(&app, &adapter, "resource-home");
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(12));
}

#[test]
fn native10_surfaces_render_at_every_size_and_scale_and_respond_to_input() {
    let _ = slint::platform::set_platform(Box::new(RenderPlatform));
    // Every scene at every size (100%) and every scale (minimum size).
    for (scene, setup) in scenes() {
        for (label, size) in SIZES {
            let (app, adapter) = window(size, 1.0);
            setup(&app);
            let pixels = render(&app, &adapter, &format!("{scene}-{label}-100"));
            assert!(
                pixels
                    .iter()
                    .any(|p| p.r != pixels[0].r || p.g != pixels[0].g),
                "{scene} {label} rendered blank"
            );
        }
        for scale in SCALES {
            let (app, adapter) = window((720.0, 520.0), scale);
            setup(&app);
            render(
                &app,
                &adapter,
                &format!("{scene}-min-720x520-{}", (scale * 100.0) as u32),
            );
        }
    }

    // Real pointer capture: no writes while moving, snap once on release, Escape cancels.
    let (app, adapter) = window((1120., 760.), 1.);
    dashboard(&app, true, false);
    let widgets = app.global::<crate::Widgets>();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed = changes.clone();
    widgets.on_arrange(move |id, c, r, w, h, columns| {
        assert_eq!(columns, 6);
        observed.borrow_mut().push((id.to_string(), c, r, w, h));
    });
    render(&app, &adapter, "drag-editor-before");
    // Home's selected-instance section can change height. Find the actual
    // Playtime drag handle through its interaction contract, rather than
    // pressing an old page coordinate that may now hit a different control.
    let y = (0..760)
        .step_by(4)
        .find_map(|y| {
            widgets.set_editing(true);
            let position = LogicalPosition::new(200., y as f32);
            app.window().dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            let found = widgets.get_active_id() == "playtime";
            app.window().dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
            found.then_some(y as f32)
        })
        .expect("visible Playtime header must accept a pointer press");
    assert!(changes.borrow().is_empty(), "hit testing must not persist");
    let from = LogicalPosition::new(200., y);
    let to = LogicalPosition::new(380., y + 210.);
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: from,
        button: PointerEventButton::Left,
    });
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: to });
    assert_eq!(widgets.get_active_id(), "playtime");
    assert_eq!(
        (widgets.get_preview_column(), widgets.get_preview_row()),
        (1, 1)
    );
    assert!(changes.borrow().is_empty(), "preview must not persist");
    render(&app, &adapter, "drag-editor-preview");
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: to,
        button: PointerEventButton::Left,
    });
    assert_eq!(*changes.borrow(), vec![("playtime".into(), 1, 1, 2, 1)]);
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: from });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: from,
        button: PointerEventButton::Left,
    });
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: to });
    key(&app, Key::Escape);
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: to,
        button: PointerEventButton::Left,
    });
    assert_eq!(changes.borrow().len(), 1, "Escape cancels without saving");
    assert_eq!(widgets.get_active_id(), "");
    // Focus remains on the move handle for keyboard equivalents.
    key(&app, Key::RightArrow);
    assert_eq!(
        changes.borrow().last().unwrap(),
        &("playtime".into(), 1, 0, 2, 1)
    );

    // The actual corner handle snaps both dimensions and emits one final edit.
    let corner_y = ((y as usize + 44)..760)
        .step_by(4)
        .find_map(|y| {
            let position = LogicalPosition::new(407., y as f32);
            app.window().dispatch_event(WindowEvent::PointerPressed {
                position,
                button: PointerEventButton::Left,
            });
            let found = widgets.get_active_id() == "playtime";
            app.window().dispatch_event(WindowEvent::PointerReleased {
                position,
                button: PointerEventButton::Left,
            });
            found.then_some(y as f32)
        })
        .expect("visible Playtime resize handle must accept a pointer press");
    let corner = LogicalPosition::new(407., corner_y);
    let larger = LogicalPosition::new(573., corner_y + 110.);
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: corner });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: corner,
        button: PointerEventButton::Left,
    });
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: larger });
    assert_eq!(
        (widgets.get_preview_width(), widgets.get_preview_height()),
        (3, 2)
    );
    assert_eq!(changes.borrow().len(), 2);
    render(&app, &adapter, "drag-editor-resize");
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: larger,
        button: PointerEventButton::Left,
    });
    assert_eq!(
        changes.borrow().last().unwrap(),
        &("playtime".into(), 0, 0, 3, 2)
    );
    assert_eq!(changes.borrow().len(), 3);

    // Playback remains usable after changing the card footprint and orientation.
    for (columns, rows, x) in [(4, 2, 430.), (1, 3, 180.)] {
        let (app, adapter) = window((1120., 1000.), 1.);
        responsive_widget(&app, "media", columns, rows);
        let toggles = Rc::new(RefCell::new(0));
        let observed = toggles.clone();
        app.global::<crate::Widgets>().set_media_can_toggle(true);
        app.global::<crate::Widgets>()
            .on_media_toggle(move || *observed.borrow_mut() += 1);
        render(
            &app,
            &adapter,
            &format!("responsive-media-input-{columns}-{rows}"),
        );
        for y in (100..980).step_by(8) {
            click(&app, x, y as f32);
            if *toggles.borrow() > 0 {
                break;
            }
        }
        assert_eq!(
            *toggles.borrow(),
            1,
            "playback control remains reachable after resize"
        );
    }

    // Rename a saved skin through the real editor and Enter action.
    let (app, adapter) = window((1120., 760.), 1.);
    scenes()
        .into_iter()
        .find(|(name, _)| *name == "refinement-saved-skin")
        .unwrap()
        .1(&app);
    let renamed: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let observed = renamed.clone();
    app.global::<crate::Skins>().on_rename(move |id, name| {
        observed
            .borrow_mut()
            .push((id.to_string(), name.to_string()))
    });
    render(&app, &adapter, "refinement-skin-before-rename");
    click(&app, 960., 334.);
    render(&app, &adapter, "refinement-skin-name-editor");
    click(&app, 820., 334.);
    key(&app, Key::End);
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: " New".into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: " New".into(),
    });
    key(&app, Key::Return);
    assert_eq!(
        *renamed.borrow(),
        [("saved-one".into(), "Saved skin New".into())]
    );

    // Input: the new actions fire exactly once from real pointer events.
    let (app, adapter) = window((1120.0, 760.0), 1.0);
    workspace(&app, 0);
    render(&app, &adapter, "input-baseline");
    let fired: Rc<RefCell<Vec<String>>> = Rc::default();
    let instances = app.global::<Instances>();
    {
        let log = fired.clone();
        instances.on_content_refresh(move || log.borrow_mut().push("check".into()));
        let log = fired.clone();
        instances.on_update_all(move || log.borrow_mut().push("all".into()));
        let log = fired.clone();
        instances.on_content_update(move |id| log.borrow_mut().push(format!("update:{id}")));
        let log = fired.clone();
        instances.on_content_unpin(move |id| log.borrow_mut().push(format!("unpin:{id}")));
        let log = fired.clone();
        instances.on_update_review_dismiss(move || log.borrow_mut().push("dismiss".into()));
    }
    for (x, y) in crate::layout_acceptance::targets::MODS {
        click(&app, x, y);
    }
    assert_eq!(
        *fired.borrow(),
        ["check", "all", "update:a", "unpin:b"],
        "each new content-update control dispatches once"
    );
    // Escape dismisses the review dialog without confirming it.
    update_review(&app);
    render(&app, &adapter, "input-review-open");
    key(&app, Key::Escape);
    assert_eq!(fired.borrow().last().map(String::as_str), Some("dismiss"));

    // The optional Client action and review are keyboard/pointer reachable.
    let (app, adapter) = window((1120.0, 760.0), 1.0);
    scenes()
        .into_iter()
        .find(|(name, _)| *name == "aurora-add-settings")
        .unwrap()
        .1(&app);
    render(&app, &adapter, "aurora-add-input");
    let client_actions = Rc::new(RefCell::new(0));
    let actions = client_actions.clone();
    app.global::<Instances>()
        .on_client_review(move || *actions.borrow_mut() += 1);
    click(&app, 1000., 377.);
    assert_eq!(*client_actions.borrow(), 1);
    scenes()
        .into_iter()
        .find(|(name, _)| *name == "aurora-review")
        .unwrap()
        .1(&app);
    render(&app, &adapter, "aurora-review-input");
    key(&app, Key::Escape);
    assert!(!app.global::<Instances>().get_client_review_open());

    let (app, adapter) = window((1120., 760.), 1.);
    scenes()
        .into_iter()
        .find(|(name, _)| *name == "recovery-local-edits")
        .unwrap()
        .1(&app);
    render(&app, &adapter, "recovery-input-baseline");
    assert!(!app.global::<crate::Recovery>().get_keep_removed());
    let recovery_actions = Rc::new(RefCell::new(0));
    let seen = recovery_actions.clone();
    app.global::<crate::Recovery>()
        .on_confirm(move || *seen.borrow_mut() += 1);
    click(&app, 695., 581.);
    assert_eq!(
        *recovery_actions.borrow(),
        0,
        "removed mods require explicit confirmation"
    );
    click(&app, 353., 534.);
    assert!(app.global::<crate::Recovery>().get_keep_removed());
    click(&app, 695., 581.);
    assert_eq!(
        *recovery_actions.borrow(),
        1,
        "one approval dispatches one recovery"
    );
    render(&app, &adapter, "recovery-input-confirmed");
    key(&app, Key::Escape);
    assert!(!app.global::<crate::Recovery>().get_open());

    // Home's visible frame must not depend on diagnostic strings. Settings
    // retains those same fields; this catches accidental warning/exit leakage.
    let (app, adapter) = window((1120., 760.), 1.);
    home_card(&app, false);
    let diagnostic_frame = render(&app, &adapter, "home-diagnostics-hidden");
    let home = app.global::<crate::Home>();
    home.set_message("".into());
    home.set_blockers(ModelRc::default());
    home.set_progress_text("".into());
    let clean_frame = render(&app, &adapter, "home-clean-frame");
    assert!(
        diagnostic_frame == clean_frame,
        "Home must not render launch details, warnings or exit messages"
    );
    let opens = Rc::new(RefCell::new(0));
    let seen = opens.clone();
    app.global::<crate::Home>()
        .on_open_settings(move || *seen.borrow_mut() += 1);
    // The gear follows the responsive launch controls; locate it by its
    // keyboard action rather than an obsolete absolute pointer position.
    for _ in 0..32 {
        key(&app, Key::Tab);
        key(&app, Key::Return);
        if *opens.borrow() > 0 {
            break;
        }
        key(&app, Key::Escape);
    }
    assert_eq!(
        *opens.borrow(),
        1,
        "the Home gear opens instance settings once"
    );

    // Exercise the Java controls on the real layout, including keyboard
    // inspection/editing of a long path. The chooser callback is observed;
    // this headless test does not pretend to operate the Windows dialog.
    let (app, adapter) = window((1120.0, 760.0), 1.0);
    java_settings(&app, true);
    scroll_down(&app, 330.0);
    render(&app, &adapter, "java-input-baseline");
    let java_fired: Rc<RefCell<Vec<&str>>> = Rc::default();
    let instances = app.global::<Instances>();
    let log = java_fired.clone();
    instances.on_cfg_java_choose(move || log.borrow_mut().push("choose"));
    let log = java_fired.clone();
    instances.on_cfg_java_check(move || log.borrow_mut().push("check"));
    let log = java_fired.clone();
    instances.on_cfg_java_changed(move || log.borrow_mut().push("changed"));
    click(&app, 175.0, 320.0);
    click(&app, 290.0, 320.0);
    assert_eq!(*java_fired.borrow(), ["choose", "check"]);
    click(&app, 350.0, 276.0);
    key(&app, Key::End);
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "x".into() });
    assert!(instances.get_cfg_java_path().ends_with('x'));
    // The real numeric memory control accepts pointer focus and keyboard edits.
    click(&app, 975.0, 484.0);
    key(&app, Key::End);
    key(&app, Key::Backspace);
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "8".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "8".into() });
    assert_eq!(instances.get_memory_mib_text().as_str(), "4098");
    click(&app, 430.0, 320.0);
    assert_eq!(
        instances.get_cfg_java_index(),
        0,
        "reset works with one click"
    );
    // The conflict actions use the real scrollable layout at minimum size.
    let (app, adapter) = window((720., 520.), 1.);
    scenes()
        .into_iter()
        .find(|(name, _)| *name == "pandora4-conflicts")
        .unwrap()
        .1(&app);
    render(&app, &adapter, "pandora4-conflict-input-baseline");
    let choices: Rc<RefCell<Vec<(i32, bool)>>> = Rc::default();
    let log = choices.clone();
    app.global::<crate::InstanceSync>()
        .on_resolve_conflict(move |index, use_source| log.borrow_mut().push((index, use_source)));
    // Settings descriptions wrap differently at minimum width. Find the real
    // pointer target in the scrollable page instead of pinning its old y.
    let mut conflict_y = None;
    for _ in 0..8 {
        for y in (100..500).step_by(8) {
            click(&app, 180., y as f32);
            if !choices.borrow().is_empty() {
                conflict_y = Some(y as f32);
                break;
            }
        }
        if conflict_y.is_some() {
            break;
        }
        scroll_down(&app, 240.);
        render(&app, &adapter, "pandora4-conflict-input-scrolled");
    }
    let conflict_y = conflict_y.expect("the first conflict action remains pointer-reachable");
    assert_eq!(*choices.borrow(), [(0, true)]);
    click(&app, 390., conflict_y);
    assert_eq!(*choices.borrow(), [(0, true), (0, false)]);
    // Tab transfers focus between native buttons and Return activates the next.
    click(&app, 180., conflict_y);
    key(&app, Key::Tab);
    key(&app, Key::Return);
    assert_eq!(choices.borrow().last(), Some(&(0, false)));
    render(&app, &adapter, "pandora4-conflict-keyboard-focus");
}

/// Logical click targets measured from the 1120×760 `input-baseline` render.
mod targets {
    // Check for updates, Update all, Sodium Update, Lithium Unpin.
    pub const MODS: [(f32, f32); 4] = [
        (1070.0, 178.0),
        (1018.0, 224.0),
        (930.0, 280.0),
        (944.0, 354.0),
    ];
}
