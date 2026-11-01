//! Native file selection and explicit review over the shared local importer.
use crate::{ContentImport, Instances, Tone, api, ui};
use slint::ComponentHandle;
thread_local! { static REVIEW: std::cell::RefCell<Option<(String, serde_json::Value)>> = const { std::cell::RefCell::new(None) }; }

pub fn bind(window: &crate::AppWindow) {
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
                        "{}\nTarget: {}\nType: {} · {:.1} MiB\n{}",
                        preview["fileName"].as_str().unwrap_or("File"),
                        preview["instanceName"].as_str().unwrap_or("Instance"),
                        preview["contentType"].as_str().unwrap_or("Content"),
                        preview["sizeBytes"].as_u64().unwrap_or(0) as f64 / 1048576.0,
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
