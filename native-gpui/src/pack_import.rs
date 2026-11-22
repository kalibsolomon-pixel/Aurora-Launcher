//! Review local archives and extracted modpack folders before installation.

#![allow(dead_code)]

use aurora_launcher_lib::host::Host;
use std::path::PathBuf;

use gpui_kit::{
    Context, EventEmitter, ExternalPaths, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use serde_json::Value;

use crate::api::{self, ApiError, ApiResult};
use crate::backend::guarded;
use crate::controls::AuroraButton;
use crate::downloads::{JobRequest, PreparedJob};
use crate::file_picker;
use crate::theme;
use crate::ui;

/// Emitted for the integrating parent view.
#[derive(Debug)]
pub enum PackImportEvent {
    /// The module's busy state changed. Each transition is reported once.
    BusyChanged(bool),
    InstallFolder(PreparedJob),
    OpenSync,
    /// An exact published Modrinth pack version was recognized for the
    /// selected local file. Both IDs are nonempty; the parent opens the
    /// reviewed Discover pack workflow pinned to this exact version.
    OpenLocalPack {
        project_id: String,
        version_id: String,
        note: String,
    },
}

// ---------------------------------------------------------------------------
// Pure import core (testable without a host, window or picker)
// ---------------------------------------------------------------------------

/// The exact published identity of the selected archive, ready to navigate.
#[derive(Clone, Debug, PartialEq)]
struct PackIdentity {
    project_id: String,
    version_id: String,
    note: String,
}

/// One terminal reply. `Ignored` is a stale or duplicate reply: it touches
/// nothing, including busy state owned by a newer operation.
#[derive(Debug, PartialEq)]
enum PackImportCompletion {
    Ignored,
    /// The user cancelled the chooser; a quiet terminal settlement.
    Cancelled,
    /// An exact recognized published version, with nonempty IDs.
    Opened(PackIdentity),
    Folder(Value),
    /// The chooser or identification failed; `String` is the actionable
    /// message shown to the user.
    Failed(String),
}

/// The import state machine, shared by the view and the pure-state fixtures.
#[derive(Debug, PartialEq)]
struct PackImportCore {
    /// Monotonic operation tag; only the running operation's reply settles.
    serial: u64,
    /// The serial whose reply may still settle, if an operation is running.
    pending: Option<u64>,
    busy: bool,
}

impl PackImportCore {
    fn new() -> Self {
        Self {
            serial: 0,
            pending: None,
            busy: false,
        }
    }

    /// Starts one identification. Requires a host and no parent gate, and
    /// never overlaps another running operation.
    fn start(&mut self, host_present: bool, blocked: bool) -> Option<u64> {
        if !host_present || blocked || self.busy {
            return None;
        }
        self.serial += 1;
        self.pending = Some(self.serial);
        self.busy = true;
        Some(self.serial)
    }

    /// Settles the reply of the running operation. A reply that does not
    /// belong to the running operation is ignored without touching busy.
    fn receive(&mut self, serial: u64, result: ApiResult<Option<Value>>) -> PackImportCompletion {
        if self.pending != Some(serial) {
            return PackImportCompletion::Ignored;
        }
        self.pending = None;
        self.busy = false;
        match result {
            Ok(None) => PackImportCompletion::Cancelled,
            Ok(Some(found)) if found["provider"] == "local" => PackImportCompletion::Folder(found),
            Ok(Some(found)) => {
                let text = |key: &str| found[key].as_str().unwrap_or("").to_owned();
                let note = local_pack_note(&found);
                let project_id = text("projectId");
                let version_id = text("versionId");
                if project_id.trim().is_empty() || version_id.trim().is_empty() {
                    return PackImportCompletion::Failed(
                        "The selected file matched Modrinth, but its published identity is \
                         incomplete. Choose the pack again, or find it in Discover."
                            .to_owned(),
                    );
                }
                PackImportCompletion::Opened(PackIdentity {
                    project_id,
                    version_id,
                    note,
                })
            }
            Err(error) => PackImportCompletion::Failed(error.message),
        }
    }
}

/// Where the archive comes from: the owned picker or a dropped file.
enum ModpackSource {
    Picker(std::pin::Pin<Box<dyn std::future::Future<Output = ApiResult<Option<String>>> + Send>>),
    Path(String),
}

/// Exactly one supported archive or directory enters core review.
pub(crate) fn classify_modpack_drop(paths: &[PathBuf]) -> Result<String, &'static str> {
    let is_mrpack = |path: &PathBuf| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mrpack"))
    };
    match paths {
        [path]
            if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
                || is_mrpack(path) =>
        {
            path.to_str()
                .map(str::to_owned)
                .ok_or("The selected path can't be read.")
        }
        [] | [_] => Err("Choose a .mrpack file or extracted modpack folder."),
        _ => Err("Drop one modpack at a time."),
    }
}

/// Reader-facing name for the pack's one mod loader and version.
fn pack_loader_label(pack: &Value) -> String {
    let name = match pack["loader"].as_str().unwrap_or("") {
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoForge" => "NeoForge",
        _ => "Mod loader",
    };
    format!("{name} {}", pack["loaderVersion"].as_str().unwrap_or(""))
}

/// Small archives would otherwise read as "0.0 MiB".
fn size_label(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if bytes < MIB {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    }
}

/// The note shown above the hosted review for a recognized local archive.
/// Exact SHA-512 match only: the verified local bytes are used for this
/// exact version, while any other version is downloaded from Modrinth.
fn local_pack_note(found: &Value) -> String {
    let text = |key: &str| found[key].as_str().unwrap_or("");
    format!(
        "Local file {} is {} {} on Modrinth (exact SHA-512 match). Its verified local bytes are used for this version; choosing another version downloads that one from Modrinth instead.\nPack: {} {} · Minecraft {} · {} · {}",
        text("fileName"),
        text("title"),
        text("versionNumber"),
        text("name"),
        text("packVersion"),
        text("minecraftVersion"),
        pack_loader_label(found),
        size_label(found["sizeBytes"].as_u64().unwrap_or(0)),
    )
}

// ---------------------------------------------------------------------------
// Import view
// ---------------------------------------------------------------------------

pub struct PackImport {
    host: Option<Host>,
    rt: tokio::runtime::Handle,
    core: PackImportCore,
    /// The parent's gate on new starts; never discards a running completion.
    blocked: bool,
    import_menu_open: bool,
    error: Option<String>,
    folder_review: Option<Value>,
}

impl PackImport {
    /// Builds the import entry. The constructor does no API, picker or file
    /// work; with no host nothing ever runs.
    pub fn new(
        host: Option<Host>,
        runtime: tokio::runtime::Handle,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            host,
            rt: runtime,
            core: PackImportCore::new(),
            blocked: false,
            import_menu_open: false,
            error: None,
            folder_review: None,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.core.busy
    }

    /// Prevents starting an import during a root launch/preparation action.
    /// An already-running identification still completes normally.
    pub fn set_blocked(&mut self, blocked: bool, cx: &mut Context<Self>) {
        if self.blocked != blocked {
            self.blocked = blocked;
            cx.notify();
        }
    }

    /// The explicit action: owned file picker, then exact identification.
    /// A diagnostic run without a host performs no action.
    fn import(&mut self, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        let Some(serial) = self.core.start(true, self.blocked) else {
            return;
        };
        self.error = None;
        cx.emit(PackImportEvent::BusyChanged(true));
        cx.notify();
        let picker =
            file_picker::import_path(file_picker::LocalImportKind::Modpack, folder, window, cx);
        self.identify(serial, ModpackSource::Picker(Box::pin(picker)), cx);
    }

    /// Drops enter the same review as selections from the picker.
    pub fn import_dropped(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        let path = match classify_modpack_drop(paths) {
            Ok(path) => path,
            Err(reason) => {
                self.error = Some(reason.to_owned());
                cx.notify();
                return;
            }
        };
        let Some(serial) = self.core.start(true, self.blocked) else {
            self.error = Some(
                "Finish the current import or launcher operation, then drop the file again."
                    .to_owned(),
            );
            cx.notify();
            return;
        };
        self.error = None;
        cx.emit(PackImportEvent::BusyChanged(true));
        cx.notify();
        self.identify(serial, ModpackSource::Path(path), cx);
    }

    /// Runs one identification on the runtime and delivers its single reply
    /// through an awaited channel under `serial`.
    fn identify(&mut self, serial: u64, source: ModpackSource, cx: &mut Context<Self>) {
        let host = self.host.clone().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.rt.spawn(async move {
            let result = guarded(async move {
                let path = match source {
                    ModpackSource::Picker(picker) => match picker.await? {
                        Some(path) => path,
                        None => return Ok(None),
                    },
                    ModpackSource::Path(path) => path,
                };
                api::review_local_modpack(host, path).await.map(Some)
            })
            .await;
            let _ = tx.send(result);
        });
        cx.spawn(async move |weak, cx| {
            let result = rx.await.unwrap_or_else(|_| {
                Err(ApiError::new(
                    "native_operation_failed",
                    "The modpack import stopped unexpectedly. Try again.",
                ))
            });
            let _ = weak.update(cx, |view, cx| view.receive(serial, result, cx));
        })
        .detach();
    }

    fn receive(&mut self, serial: u64, result: ApiResult<Option<Value>>, cx: &mut Context<Self>) {
        let completion = self.core.receive(serial, result);
        match completion {
            PackImportCompletion::Ignored => return,
            PackImportCompletion::Cancelled => {}
            PackImportCompletion::Opened(identity) => {
                self.error = None;
                self.folder_review = None;
                cx.emit(PackImportEvent::OpenLocalPack {
                    project_id: identity.project_id,
                    version_id: identity.version_id,
                    note: identity.note,
                });
            }
            PackImportCompletion::Folder(review) => self.folder_review = Some(review),
            PackImportCompletion::Failed(message) => {
                self.folder_review = None;
                self.error = Some(message);
            }
        }
        cx.emit(PackImportEvent::BusyChanged(false));
        cx.notify();
    }
}

impl EventEmitter<PackImportEvent> for PackImport {}

impl Render for PackImport {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::theme(cx).clone();
        let busy = self.is_busy();
        let disabled = busy || self.blocked || self.host.is_none();

        // The whole header is a drop target for a `.mrpack`; the wash says
        // whether the dragged file would be accepted.
        let (accept, reject) = (palette.accent, palette.danger);
        // Main page actions stay to the left (primary action first), matching
        // Pandora's action-first layout.
        let mut actions = div().flex().flex_shrink_0().items_center().gap(px(8.));
        if let Some(new_instance) = crate::instance_creator::new_instance_button(cx) {
            actions = actions.child(new_instance);
        }
        actions = actions
            .child(
                AuroraButton::new("pack-import-toggle", "Import")
                    .selected(self.import_menu_open)
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.import_menu_open = !this.import_menu_open;
                        cx.notify();
                    })),
            )
            .child(
                AuroraButton::new("instances-open-sync", "Sync")
                    .variant(crate::controls::ButtonVariant::Default)
                    .disabled(busy)
                    .on_click(cx.listener(|_, _, _, cx| {
                        cx.emit(PackImportEvent::OpenSync);
                    })),
            );

        let hint = if busy {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(crate::symbols::icon(Some("sync"), px(14.), palette.ring))
                .child(
                    div()
                        .text_size(px(ui::TEXT_CAPTION))
                        .text_color(palette.muted_foreground)
                        .child("Reviewing modpack…"),
                )
        } else {
            div()
        };

        let mut root = div()
            .id("pack-import")
            .flex()
            .flex_col()
            .gap(px(12.))
            .rounded(palette.radius)
            .drag_over::<ExternalPaths>(move |style, paths, _, _| {
                style.bg(gpui_kit::Hsla {
                    a: 0.10,
                    ..if classify_modpack_drop(paths.paths()).is_ok() {
                        accept
                    } else {
                        reject
                    }
                })
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                this.import_dropped(paths.paths(), cx);
            }))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(16.))
                    .child(ui::page_header(&palette, "Instances", None))
                    .child(actions),
            )
            .when(self.import_menu_open && !busy, |col| {
                col.child(
                    ui::card(&palette)
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .p(px(14.))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    AuroraButton::new("pack-import-open", "Import .mrpack")
                                        .variant(crate::controls::ButtonVariant::Soft)
                                        .size(crate::controls::ButtonSize::Small)
                                        .icon("upload")
                                        .disabled(disabled)
                                        .on_click(cx.listener(|this, _event, window, cx| {
                                            this.import_menu_open = false;
                                            this.import(false, window, cx);
                                        })),
                                )
                                .child(
                                    AuroraButton::new("pack-import-folder", "Import folder")
                                        .size(crate::controls::ButtonSize::Small)
                                        .icon("folder_open")
                                        .disabled(disabled)
                                        .on_click(cx.listener(|this, _event, window, cx| {
                                            this.import_menu_open = false;
                                            this.import(true, window, cx);
                                        })),
                                ),
                        )
                        .child(ui::caption(&palette, "Or drop a .mrpack anywhere here")),
                )
            })
            .children(busy.then_some(hint));

        if let Some(review) = &self.folder_review {
            let title = review["name"].as_str().unwrap_or("Local modpack");
            let count = |key: &str| review[key].as_array().map_or(0, Vec::len);
            let detail = format!(
                "Version {} · Minecraft {} · {}",
                review["packVersion"].as_str().unwrap_or(""),
                review["minecraftVersion"].as_str().unwrap_or(""),
                pack_loader_label(review)
            );
            let snapshot = review["projectId"].as_str().unwrap_or("").to_owned();
            let fingerprint = review["fingerprint"].as_str().unwrap_or("").to_owned();
            let job = PreparedJob {
                title: title.to_owned(),
                detail: detail.clone(),
                request: JobRequest::LocalPack {
                    snapshot,
                    fingerprint,
                },
            };
            root = root.child(ui::card(&palette)
                .gap(px(8.))
                .child(ui::section_header(&palette, title.to_owned(), Some(detail.into())))
                .child(ui::muted(&palette, format!(
                    "Required downloads {} · Overrides {} · Excluded files {} · Skipped overrides {}",
                    count("unresolved"), count("overrides"), count("excluded"), count("skippedOverrides"),
                )))
                .child(ui::caption(&palette, "This folder creates a new instance. Its publisher is unverified."))
                .child(div().flex().gap(px(8.))
                    .child(AuroraButton::new("install-pack-folder", "Install modpack")
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.folder_review = None;
                            cx.emit(PackImportEvent::InstallFolder(job.clone()));
                            cx.notify();
                        })))
                    .child(AuroraButton::new("cancel-pack-folder", "Cancel")
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.folder_review = None;
                            cx.notify();
                        })))));
        }

        if let Some(error) = &self.error {
            root = root.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(palette.radius)
                    .bg(gpui_kit::Hsla {
                        a: 0.10,
                        ..palette.danger
                    })
                    .border_1()
                    .border_color(gpui_kit::Hsla {
                        a: 0.28,
                        ..palette.danger
                    })
                    .child(crate::symbols::icon(Some("error"), px(16.), palette.danger))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .max_w(px(720.))
                            .text_size(px(ui::TEXT_BODY))
                            .text_color(palette.danger)
                            .child(error.clone()),
                    ),
            );
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn found() -> Value {
        json!({
            "projectId": "proj-1", "versionId": "ver-1",
            "fileName": "pack.mrpack", "title": "Fixture Pack", "versionNumber": "1.0.0",
            "name": "Fixture", "packVersion": "1.0.0", "minecraftVersion": "1.21.1",
            "loader": "fabric", "loaderVersion": "0.16.0", "sizeBytes": 2048
        })
    }

    #[test]
    fn note_names_the_exact_match() {
        let note = local_pack_note(&found());
        assert!(note.starts_with("Local file pack.mrpack is Fixture Pack 1.0.0 on Modrinth"));
        assert!(note.ends_with("Minecraft 1.21.1 · Fabric 0.16.0 · 2.0 KiB"));
        assert_eq!(
            pack_loader_label(&json!({"loader": "forge", "loaderVersion": "47.4.10"})),
            "Forge 47.4.10"
        );
        assert_eq!(
            pack_loader_label(&json!({"loader": "neoForge", "loaderVersion": "26.2.0.88"})),
            "NeoForge 26.2.0.88"
        );
        assert_eq!(pack_loader_label(&json!({})), "Mod loader ");
    }

    fn drop_of(names: &[&str]) -> Result<String, &'static str> {
        classify_modpack_drop(&names.iter().map(PathBuf::from).collect::<Vec<_>>())
    }

    #[test]
    fn dropped_archive_must_be_exactly_one_mrpack() {
        assert_eq!(
            drop_of(&["C:\\packs\\Fixture.mrpack"]).unwrap(),
            "C:\\packs\\Fixture.mrpack"
        );
        assert!(
            drop_of(&["PACK.MRPACK"]).is_ok(),
            "extension is case-insensitive"
        );
        assert_eq!(
            drop_of(&["a.zip"]),
            Err("Choose a .mrpack file or extracted modpack folder.")
        );
        assert_eq!(
            drop_of(&["a.jar"]),
            Err("Choose a .mrpack file or extracted modpack folder.")
        );
        assert_eq!(
            drop_of(&["mrpack"]),
            Err("Choose a .mrpack file or extracted modpack folder.")
        );
        assert_eq!(
            drop_of(&["a.mrpack", "b.mrpack"]),
            Err("Drop one modpack at a time.")
        );
        assert_eq!(
            drop_of(&["a.mrpack", "notes.txt"]),
            Err("Drop one modpack at a time.")
        );
        assert_eq!(
            drop_of(&[]),
            Err("Choose a .mrpack file or extracted modpack folder.")
        );
    }

    #[test]
    fn a_drop_starts_only_when_the_picker_could() {
        // A drop shares the picker's start gate: no overlap with a running
        // identification, none while the parent has blocked new starts.
        let mut core = PackImportCore::new();
        let serial = core.start(true, false).unwrap();
        assert_eq!(core.start(true, false), None);
        assert!(matches!(
            core.receive(serial, Ok(Some(found()))),
            PackImportCompletion::Opened(_)
        ));
        assert_eq!(core.start(true, true), None);
        // A dropped archive that is not published never opens Discover.
        let serial = core.start(true, false).unwrap();
        assert!(matches!(
            core.receive(
                serial,
                Err(ApiError::new("local_pack_unknown", "Not a published pack."))
            ),
            PackImportCompletion::Failed(_)
        ));
        assert!(!core.busy);
    }

    #[test]
    fn start_gating() {
        let mut core = PackImportCore::new();
        assert_eq!(core.start(false, false), None);
        assert_eq!(core.start(true, true), None);
        assert!(!core.busy);
        let serial = core.start(true, false);
        assert!(serial.is_some() && core.busy);
        assert_eq!(core.start(true, false), None);
    }

    #[test]
    fn cancelled_chooser_is_quiet() {
        let mut core = PackImportCore::new();
        let serial = core.start(true, false).unwrap();
        assert_eq!(
            core.receive(serial, Ok(None)),
            PackImportCompletion::Cancelled
        );
        assert!(!core.busy && core.pending.is_none());
    }

    #[test]
    fn failure_shows_actionable_error() {
        let mut core = PackImportCore::new();
        let serial = core.start(true, false).unwrap();
        assert_eq!(
            core.receive(
                serial,
                Err(ApiError::new(
                    "local_pack_unknown",
                    "Not a recognized pack."
                ))
            ),
            PackImportCompletion::Failed("Not a recognized pack.".to_owned())
        );
        assert!(!core.busy);
    }

    #[test]
    fn success_carries_exact_identity_and_note() {
        let mut core = PackImportCore::new();
        let serial = core.start(true, false).unwrap();
        match core.receive(serial, Ok(Some(found()))) {
            PackImportCompletion::Opened(identity) => {
                assert_eq!(identity.project_id, "proj-1");
                assert_eq!(identity.version_id, "ver-1");
                assert!(
                    identity
                        .note
                        .starts_with("Local file pack.mrpack is Fixture Pack 1.0.0 on Modrinth")
                );
            }
            other => panic!("{other:?}"),
        }
        assert!(!core.busy);
    }

    #[test]
    fn malformed_identity_never_opens() {
        let mut core = PackImportCore::new();
        let serial = core.start(true, false).unwrap();
        let mut broken = found();
        broken["versionId"] = json!("");
        assert!(matches!(
            core.receive(serial, Ok(Some(broken))),
            PackImportCompletion::Failed(_)
        ));
        let mut minimal = found();
        minimal.as_object_mut().unwrap().remove("projectId");
        let serial = core.start(true, false).unwrap();
        assert!(matches!(
            core.receive(serial, Ok(Some(minimal))),
            PackImportCompletion::Failed(_)
        ));
    }

    #[test]
    fn duplicate_and_stale_replies_touch_nothing() {
        let mut core = PackImportCore::new();
        let serial = core.start(true, false).unwrap();
        assert_eq!(
            core.receive(serial, Ok(Some(found()))),
            PackImportCompletion::Opened(PackIdentity {
                project_id: "proj-1".to_owned(),
                version_id: "ver-1".to_owned(),
                note: local_pack_note(&found()),
            })
        );
        assert_eq!(
            core.receive(serial, Ok(None)),
            PackImportCompletion::Ignored
        );
        assert_eq!(
            core.receive(serial, Ok(Some(found()))),
            PackImportCompletion::Ignored
        );
        let second = core.start(true, false).unwrap();
        let stale = core.receive(serial, Ok(None));
        assert_eq!(stale, PackImportCompletion::Ignored);
        assert!(core.busy);
        assert_eq!(
            core.receive(second, Ok(None)),
            PackImportCompletion::Cancelled
        );
    }

    #[test]
    fn blocked_gate_does_not_discard_running_completion() {
        let mut core = PackImportCore::new();
        let serial = core.start(true, false).unwrap();
        // A parent gate that arrives mid-operation never invalidates it.
        let result = core.receive(serial, Ok(Some(found())));
        assert!(matches!(result, PackImportCompletion::Opened(_)));
        assert!(!core.busy);
        // After settling, the gate blocks only new starts.
        assert_eq!(core.start(true, true), None);
    }
}
