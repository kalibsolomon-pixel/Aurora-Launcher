//! Explicit local OS actions and copies of existing reports. Never regenerates game-owned data.
use super::{
    Error, parse,
    store::{self, Entry},
    view::*,
};
use crate::{instances::InstanceId, paths::ManagedPaths};
use serde::Deserialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    id: String,
    folder: String,
    html_digest: String,
    json_digest: String,
}
fn single_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 240
        && !name.contains(['/', '\\', ':'])
        && !matches!(name, "." | "..")
        && !name.ends_with(['.', ' '])
}
fn root(paths: &ManagedPaths, e: &Entry) -> Result<PathBuf, Error> {
    let instance = store::instance_root(paths, &e.key.instance_id)?;
    let root = instance.join(if e.legacy { "kwamster" } else { "phenomena" });
    // An imported index is never an arbitrary-path capability.
    if Path::new(&e.provenance.source_root) != root {
        return Err(Error::new(
            "pulsar_source_unavailable",
            "This source root is no longer registered at its original location.",
        ));
    }
    Ok(root)
}
fn receipt_folder(paths: &ManagedPaths, e: &Entry, verify: bool) -> Result<PathBuf, Error> {
    let root = root(paths, e)?;
    let instance = store::instance_root(paths, &e.key.instance_id)?;
    let receipt = root
        .join("report-state")
        .join(format!("{}.json", e.key.fight_id));
    if !receipt.try_exists()? {
        return Err(Error::new(
            "pulsar_report_missing",
            "No completed report receipt exists yet. Refresh after the client finishes exporting.",
        ));
    }
    let bytes=parse::read_bytes(&instance,&receipt,16*1024).map_err(|_|Error::new("pulsar_report_pending","A completed report receipt is not available yet. Refresh after the client finishes exporting."))?;
    let r: Receipt = serde_json::from_slice(&bytes).map_err(|_| {
        Error::new(
            "pulsar_report_pending",
            "The report receipt is incomplete or invalid; source files were preserved.",
        )
    })?;
    if r.id != e.key.fight_id || !single_name(&r.folder) {
        return Err(Error::new(
            "pulsar_report_invalid",
            "The report receipt identity or child folder is invalid.",
        ));
    }
    let folder = root.join("exports").join(r.folder);
    parse::safe_path(&instance, &folder)?;
    let marker = folder.join(".phenomena-owner");
    let marker = if marker.exists() {
        marker
    } else {
        folder.join(".kwamster-owner")
    };
    let owner = parse::read_bytes(&instance, &marker, 128)?;
    if owner != e.key.fight_id.as_bytes() {
        return Err(Error::new(
            "pulsar_report_invalid",
            "Report ownership does not match the fight UUID.",
        ));
    }
    for (name, hash) in [
        ("index.html", r.html_digest),
        ("report.json", r.json_digest),
    ] {
        let file = folder.join(name);
        parse::safe_path(&instance, &file)?;
        let meta = fs::symlink_metadata(&file)?;
        if !meta.is_file() || meta.len() > parse::MAX_FILE_BYTES {
            return Err(Error::new(
                "pulsar_report_invalid",
                "The completed report file is unavailable or too large.",
            ));
        }
        if verify
            && store::digest(&parse::read_bytes(&instance, &file, parse::MAX_FILE_BYTES)?) != hash
        {
            return Err(Error::new(
                "pulsar_report_pending",
                "The report and receipt do not match yet. Refresh after export completes.",
            ));
        }
    }
    Ok(folder)
}
pub(crate) fn available(paths: &ManagedPaths, e: &Entry) -> bool {
    receipt_folder(paths, e, false).is_ok()
}
fn existing(paths: &ManagedPaths, e: &Entry) -> Result<PathBuf, Error> {
    match receipt_folder(paths, e, true) {
        Ok(p) => Ok(p),
        Err(error) if error.code == "pulsar_report_missing" => {
            // Manual exports may not have a receipt. Require a UUID ownership marker and both files.
            let instance = store::instance_root(paths, &e.key.instance_id)?;
            let exports = root(paths, e)?.join("exports");
            if !exports.exists() {
                return Err(error);
            }
            parse::safe_path(&instance, &exports)?;
            let mut folders = Vec::new();
            for child in fs::read_dir(&exports)?.take(2000) {
                let folder = child?.path();
                if parse::safe_path(&instance, &folder).is_err() || !folder.is_dir() {
                    continue;
                }
                let marker = if folder.join(".phenomena-owner").exists() {
                    folder.join(".phenomena-owner")
                } else {
                    folder.join(".kwamster-owner")
                };
                if parse::read_bytes(&instance, &marker, 128).ok().as_deref()
                    != Some(e.key.fight_id.as_bytes())
                {
                    continue;
                }
                let valid = ["index.html", "report.json"].iter().all(|name| {
                    let p = folder.join(name);
                    parse::safe_path(&instance, &p).is_ok()
                        && fs::symlink_metadata(p)
                            .is_ok_and(|m| m.is_file() && m.len() <= parse::MAX_FILE_BYTES)
                });
                if valid {
                    folders.push(folder);
                }
            }
            folders.sort();
            folders.into_iter().next().ok_or(error)
        }
        Err(e) => Err(e),
    }
}
pub(crate) fn view(paths: &ManagedPaths, key: &FightKey) -> Result<ActionReceipt, Error> {
    let index = store::read_index(paths)?;
    let entry = store::entry(&index, key)?;
    let folder = existing(paths, entry)?;
    let page = folder.join("index.html");
    crate::host::open_path(&page).map_err(|_| {
        Error::new(
            "pulsar_open_failed",
            "The system could not open the existing report.",
        )
    })?;
    Ok(ActionReceipt {
        path: Some(page.to_string_lossy().into_owned()),
        revision: index.revision,
        message: "Opened the existing local report.".into(),
    })
}
pub(crate) fn open_folder(
    paths: &ManagedPaths,
    request: FolderRequest,
) -> Result<ActionReceipt, Error> {
    let id = InstanceId::new(&request.instance_id)
        .map_err(|_| Error::new("pulsar_invalid_instance", "Invalid instance identity."))?;
    let folder = match request.kind {
        FolderKind::Library => store::library_root(paths).join("library").join(id.as_str()),
        _ => {
            let instance = paths.instance_paths(&id);
            let root = if instance.root().join("phenomena").is_dir() {
                instance.root().join("phenomena")
            } else {
                instance.root().join("kwamster")
            };
            root.join(if matches!(request.kind, FolderKind::Exports) {
                "exports"
            } else {
                "fights"
            })
        }
    };
    parse::safe_path(paths.data_root(), &folder).map_err(|_| {
        Error::new(
            "pulsar_folder_unavailable",
            "The requested recordings, exports or library folder does not exist safely.",
        )
    })?;
    if !folder.is_dir() {
        return Err(Error::new(
            "pulsar_folder_unavailable",
            "Expected an existing folder.",
        ));
    }
    crate::host::open_path(&folder).map_err(|_| {
        Error::new(
            "pulsar_open_failed",
            "The system could not open the folder.",
        )
    })?;
    Ok(ActionReceipt {
        path: Some(folder.to_string_lossy().into_owned()),
        revision: 0,
        message: "Opened the local folder.".into(),
    })
}
pub(crate) fn export(
    paths: &ManagedPaths,
    request: &ExportRequest,
) -> Result<ActionReceipt, Error> {
    if !request.confirmed {
        return Err(Error::new(
            "pulsar_confirmation_required",
            "Confirm copying this existing report to the selected destination.",
        ));
    }
    let index = store::read_index(paths)?;
    let entry = store::entry(&index, &request.key)?;
    let source = existing(paths, entry)?;
    let destination = PathBuf::from(&request.destination);
    if !destination.is_absolute() || destination.as_os_str().len() > 32760 {
        return Err(Error::new(
            "pulsar_invalid_destination",
            "Choose an absolute existing export destination folder.",
        ));
    }
    // Reject every linked ancestor before writing to a user-chosen directory.
    for ancestor in destination.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        let m = fs::symlink_metadata(ancestor)?;
        if crate::install::coordination::redirected(&m) || !m.is_dir() {
            return Err(Error::new(
                "pulsar_unsafe_path",
                "Export destinations must be existing non-link directories.",
            ));
        }
    }
    let canonical = fs::canonicalize(&destination)?;
    let managed = fs::canonicalize(paths.data_root())?;
    if canonical.starts_with(&managed) {
        return Err(Error::new(
            "pulsar_invalid_destination",
            "Choose an export destination outside managed instance and launcher data.",
        ));
    }
    let name = format!("Pulsar report {}", request.key.fight_id);
    let target = destination.join(name);
    if target.exists() {
        return Err(Error::new(
            "pulsar_export_exists",
            "The destination report folder already exists; nothing was overwritten.",
        ));
    }
    let staging = destination.join(format!(".pulsar-export-{}.tmp", uuid::Uuid::new_v4()));
    fs::create_dir(&staging)?;
    let instance = store::instance_root(paths, &request.key.instance_id)?;
    let result = (|| {
        // Existing self-contained report folders have three files. Do not copy arbitrary attachments.
        for name in ["index.html", "report.json"] {
            let bytes = parse::read_bytes(&instance, &source.join(name), parse::MAX_FILE_BYTES)?;
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(staging.join(name))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        let marker = staging.join(".phenomena-owner");
        fs::write(&marker, request.key.fight_id.as_bytes())?;
        // Recheck the parent immediately before publication; no non-atomic copy fallback.
        if fs::canonicalize(&destination)? != canonical {
            return Err(Error::new(
                "pulsar_unsafe_path",
                "The destination changed during export.",
            ));
        }
        if target.exists() {
            return Err(Error::new(
                "pulsar_export_exists",
                "The destination report folder already exists.",
            ));
        }
        fs::rename(&staging, &target)?;
        Ok(())
    })();
    if result.is_err() {
        for name in ["index.html", "report.json", ".phenomena-owner"] {
            let _ = fs::remove_file(staging.join(name));
        }
        let _ = fs::remove_dir(&staging);
    }
    result?;
    Ok(ActionReceipt {
        path: Some(target.to_string_lossy().into_owned()),
        revision: index.revision,
        message: "Copied the existing report unchanged, preserving its privacy and motion options."
            .into(),
    })
}
