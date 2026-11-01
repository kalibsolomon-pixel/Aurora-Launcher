//! Native file selection and explicit review over the shared local importer.
use crate::{ContentImport, Instances, Tone, api, ui};
use slint::ComponentHandle;
thread_local! { static REVIEW: std::cell::RefCell<Option<(String, serde_json::Value)>> = const { std::cell::RefCell::new(None) }; }

/// Reader-facing names for the core's camelCase `ContentType` values.
fn type_label(kind: &str) -> &'static str {
    match kind {
        "mod" => "Mod",
        "resourcePack" => "Resource pack",
        "shaderPack" => "Shader pack",
        _ => "Content",
    }
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

/// Local `.mrpack`: owned picker, then exact identification by Rust. A
/// recognized archive opens the reviewed hosted pack workflow for its exact
/// version (new isolated instance or merge); nothing installs before review.
fn import_modpack() {
    let busy = ui::with(|w, _| {
        let instances = w.global::<Instances>();
        let busy = instances.get_pack_import_busy();
        instances.set_pack_import_busy(true);
        busy
    })
    .unwrap_or(true);
    if busy {
        return;
    }
    let owner = crate::chrome::dialog_owner();
    let host = ui::host();
    ui::spawn(
        async move {
            let Some(path) = crate::skin_import::choose_modpack_path(owner).await? else {
                return Ok(None);
            };
            api::identify_local_modpack(host, path).await.map(Some)
        },
        |result| {
            ui::with(|w, _| w.global::<Instances>().set_pack_import_busy(false));
            match result {
                Ok(Some(found)) => {
                    let text = |key: &str| found[key].as_str().unwrap_or("").to_owned();
                    let note = local_pack_note(&found);
                    crate::discover::open_local_pack(text("projectId"), text("versionId"), note);
                }
                Ok(None) => {}
                Err(error) => ui::error_toast(&error),
            }
        },
    );
}

/// The banner shown above the hosted review for a recognized local archive.
pub fn local_pack_note(found: &serde_json::Value) -> String {
    let text = |key: &str| found[key].as_str().unwrap_or("");
    format!(
        "Local file {} is {} {} on Modrinth (exact SHA-512 match). Its verified local bytes are used for this version; choosing another version downloads that one from Modrinth instead.\nPack: {} {} · Minecraft {} · Fabric {} · {}",
        text("fileName"),
        text("title"),
        text("versionNumber"),
        text("name"),
        text("packVersion"),
        text("minecraftVersion"),
        text("fabricLoaderVersion"),
        size_label(found["sizeBytes"].as_u64().unwrap_or(0)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_labels_are_readable() {
        assert_eq!(type_label("resourcePack"), "Resource pack");
        assert_eq!(type_label("shaderPack"), "Shader pack");
        assert_eq!(type_label("mod"), "Mod");
        assert_eq!(type_label("unexpected"), "Content");
        assert_eq!(size_label(192), "0.2 KiB");
        assert_eq!(size_label(3 * 1024 * 1024 + 512 * 1024), "3.5 MiB");
    }

    #[test]
    fn local_pack_banner_names_the_exact_match() {
        let note = local_pack_note(&serde_json::json!({
            "fileName": "pack.mrpack", "title": "Fixture Pack", "versionNumber": "1.0.0",
            "name": "Fixture", "packVersion": "1.0.0", "minecraftVersion": "1.21.1",
            "fabricLoaderVersion": "0.16.0", "sizeBytes": 2048
        }));
        assert!(note.starts_with("Local file pack.mrpack is Fixture Pack 1.0.0 on Modrinth"));
        assert!(note.ends_with("Minecraft 1.21.1 · Fabric 0.16.0 · 2.0 KiB"));
    }
}

pub fn bind(window: &crate::AppWindow) {
    window
        .global::<Instances>()
        .on_import_modpack(import_modpack);
    window.global::<ContentImport>().on_dismiss(|| {
        ui::with(|w, _| w.global::<ContentImport>().set_open(false));
        if let Some((instance, preview)) = REVIEW.with(|s| s.borrow_mut().take()) {
            let ticket = preview["ticket"].as_str().unwrap_or_default().to_owned();
            ui::spawn(
                api::discard_local_content_review(ui::host(), instance, ticket),
                |_| {},
            );
        }
    });
    window.global::<Instances>().on_content_import(|| {
        let Some(instance) = ui::state(|s| s.workspace.as_ref().map(|w| w.id.clone())) else {
            return;
        };
        let host = ui::host();
        let owner = crate::chrome::dialog_owner();
        let work_instance = instance.clone();
        ui::spawn(
            async move {
                let Some(path) = crate::skin_import::choose_content_path(owner).await? else {
                    return Ok(None);
                };
                api::preview_local_content(host, work_instance, path)
                    .await
                    .map(Some)
            },
            move |result| match result {
                Ok(Some(preview)) => {
                    let mut text = format!(
                        "{}\nTarget: {}\nType: {} · {}\n{}",
                        preview["fileName"].as_str().unwrap_or("File"),
                        preview["instanceName"].as_str().unwrap_or("Instance"),
                        type_label(preview["contentType"].as_str().unwrap_or_default()),
                        size_label(preview["sizeBytes"].as_u64().unwrap_or(0)),
                        preview["note"].as_str().unwrap_or("")
                    );
                    let blocked = preview["blockers"].as_array().is_none_or(|b| !b.is_empty());
                    if let Some(blockers) = preview["blockers"].as_array() {
                        for blocker in blockers {
                            text.push_str(&format!(
                                "\nBlocked: {}",
                                blocker.as_str().unwrap_or("File conflict")
                            ));
                        }
                    }
                    REVIEW.with(|s| *s.borrow_mut() = Some((instance, preview)));
                    ui::with(|w, _| {
                        let state = w.global::<ContentImport>();
                        state.set_description(text.into());
                        state.set_allowed(!blocked);
                        state.set_open(true);
                    });
                }
                Ok(None) => {}
                Err(error) => ui::error_toast(&error),
            },
        );
    });
    window.global::<ContentImport>().on_confirm(|| {
        let Some((instance, preview)) = REVIEW.with(|s| s.borrow_mut().take()) else {
            return;
        };
        if preview["blockers"].as_array().is_none_or(|b| !b.is_empty()) {
            return;
        }
        ui::with(|w, _| w.global::<ContentImport>().set_open(false));
        let title = preview["fileName"]
            .as_str()
            .unwrap_or("Local content")
            .to_owned();
        let ticket = preview["ticket"].as_str().unwrap_or_default().to_owned();
        let fingerprint = preview["fingerprint"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let project = format!("local:{ticket}");
        crate::activity::submit(
            title,
            "Local file import".into(),
            Some(instance.clone()),
            project,
            move || {
                Box::pin(api::install_local_content(
                    ui::host(),
                    instance.clone(),
                    ticket.clone(),
                    fingerprint.clone(),
                ))
            },
        );
        ui::toast("Local import added to Downloads.", Tone::Neutral);
    });
}
