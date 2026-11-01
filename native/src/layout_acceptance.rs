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
use slint::{ComponentHandle, LogicalPosition, LogicalSize, ModelRc, SharedString, VecModel};

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

type Scene = fn(&AppWindow);

fn scenes() -> Vec<(&'static str, Scene)> {
    vec![
        ("mods-updates", |app| workspace(app, 0)),
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
    click(&app, 951., 294.);
    render(&app, &adapter, "refinement-skin-name-editor");
    click(&app, 820., 294.);
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
        instances.on_check_updates(move || log.borrow_mut().push("check".into()));
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
    click(&app, 180., 344.);
    click(&app, 390., 344.);
    assert_eq!(*choices.borrow(), [(0, true), (0, false)]);
    // Tab transfers focus between native buttons and Return activates the next.
    click(&app, 180., 344.);
    key(&app, Key::Tab);
    key(&app, Key::Return);
    assert_eq!(choices.borrow().last(), Some(&(0, false)));
    render(&app, &adapter, "pandora4-conflict-keyboard-focus");
}

/// Logical click targets measured from the 1120×760 `input-baseline` render.
mod targets {
    // Check for updates, Update all, Sodium Update, Lithium Unpin.
    pub const MODS: [(f32, f32); 4] = [
        (855.0, 224.0),
        (1018.0, 224.0),
        (930.0, 280.0),
        (948.0, 341.0),
    ];
}
