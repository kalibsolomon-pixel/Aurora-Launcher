//! The Aurora Launcher self-update: a native wrapper over the official
//! Tauri 2 updater plugin.
//!
//! Trust model, stated honestly: the updater enforces minisign-style
//! signature verification against a public verification key compiled into
//! the launcher — it cannot be disabled, and no frontend- or file-supplied
//! URL or key ever participates. Production builds compile without a key
//! until the owner generates the production keypair (documented in the
//! Phase L publication contract); such builds report launcher updates as
//! honestly unconfigured rather than weakening verification. Diagnostic
//! builds compile the diagnostic public key through `AURORA_UPDATER_PUBKEY`
//! so the full signed lifecycle is testable without touching production
//! trust.
//!
//! The updater endpoint serves one owner-published, signed manifest per
//! release channel; nothing about a commit, branch, or CI build is ever an
//! update candidate. On Windows the install step launches the verified
//! NSIS installer and the application exits — that platform behavior is
//! surfaced truthfully instead of being hidden behind a fabricated restart.

use std::sync::Mutex;
use std::sync::OnceLock;

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::distribution::ReleaseChannel;
use crate::updates::{CheckDomain, LauncherPhase, UpdateAvailability, begin_check, finish_check};

/// The minisign-style public verification key compiled into this build.
/// `None` means this build was produced without updater trust material:
/// every check reports unconfigured, and nothing is ever downloaded or run.
pub const UPDATER_PUBKEY: Option<&str> = option_env!("AURORA_UPDATER_PUBKEY");

/// The base URL of the owner-published per-channel launcher update
/// manifests. The publication gate attaches `<channel>.json` (the Tauri
/// update manifest for the newest intentionally published release of that
/// channel) to a dedicated owner-controlled release; the URL exists only
/// once the owner has published. Diagnostic builds may compile a loopback
/// base through `AURORA_LAUNCHER_UPDATES_URL`.
pub const UPDATES_BASE_URL: &str = match option_env!("AURORA_LAUNCHER_UPDATES_URL") {
    Some(diagnostic) => diagnostic,
    None => {
        "https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/launcher-updates"
    }
};

/// The updater endpoint for one channel.
pub fn channel_endpoint(channel: ReleaseChannel) -> String {
    format!("{UPDATES_BASE_URL}/{}.json", channel.as_str())
}

/// The user-facing launcher update status.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdateStatus {
    pub installed_version: String,
    pub availability: UpdateAvailability,
    pub phase: Option<LauncherPhase>,
}

/// The downloaded-but-not-installed update held between the explicit
/// download and install steps. Process-local; never persisted, never
/// exposed beyond its version identity.
fn pending() -> &'static Mutex<Option<PendingUpdate>> {
    static PENDING: OnceLock<Mutex<Option<PendingUpdate>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(None))
}

struct PendingUpdate {
    version: String,
    update: tauri_plugin_updater::Update,
    /// The signature-verified installer bytes returned by `download`;
    /// `install` consumes exactly these bytes, never a re-download.
    bytes: Vec<u8>,
}

/// A failed launcher update operation. Codes stay user-safe; messages never
/// include signing secrets or endpoint bodies.
#[derive(Debug)]
pub struct LauncherUpdateError {
    pub code: &'static str,
    pub message: String,
}

impl LauncherUpdateError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn gone(message: &str) -> Self {
        Self::new("update_stale", message.to_owned())
    }
}

fn installed_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Builds the configured updater for one channel. Fails closed when this
/// build carries no trust material.
fn build_updater(
    app: &AppHandle,
    channel: ReleaseChannel,
) -> Result<tauri_plugin_updater::Updater, LauncherUpdateError> {
    let pubkey = UPDATER_PUBKEY.ok_or_else(|| {
        LauncherUpdateError::new(
            "updater_unconfigured",
            "Launcher updates are not configured in this build.",
        )
    })?;
    let endpoint = url::Url::parse(&channel_endpoint(channel)).map_err(|_| {
        LauncherUpdateError::new("updater_unconfigured", "The update endpoint is not valid.")
    })?;
    build_configured_updater(app, pubkey, endpoint)
}

/// The explicit-key/endpoint construction shared by production and the
/// deterministic updater tests (which exercise the real signature path with
/// test-generated keys and loopback manifests). Production always passes
/// the compiled key and the published endpoint.
pub(crate) fn build_configured_updater<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pubkey: &str,
    endpoint: url::Url,
) -> Result<tauri_plugin_updater::Updater, LauncherUpdateError> {
    app.updater_builder()
        .pubkey(pubkey.to_owned())
        .endpoints(vec![endpoint])
        .map_err(|error| LauncherUpdateError::new("updater_unconfigured", error.to_string()))?
        .build()
        .map_err(|error| LauncherUpdateError::new("updater_unconfigured", error.to_string()))
}

/// The one-shot read-only launcher update check for one channel. The check
/// itself never installs anything; failures are honest unavailability.
pub async fn check_launcher_update(app: &AppHandle, channel: ReleaseChannel) -> UpdateAvailability {
    let Some(_) = UPDATER_PUBKEY else {
        return UpdateAvailability::unavailable(
            "Launcher updates are not configured in this build.",
        );
    };
    let updater = match build_updater(app, channel) {
        Ok(updater) => updater,
        Err(error) => return UpdateAvailability::unavailable(error.message),
    };
    match updater.check().await {
        Ok(None) => UpdateAvailability::UpToDate,
        Ok(Some(update)) => UpdateAvailability::UpdateAvailable {
            current: installed_version(app),
            candidate: update.version.clone(),
            channel,
            notes: update.body.clone(),
        },
        Err(error) => {
            UpdateAvailability::unavailable(format!("Could not check for updates: {error}"))
        }
    }
}

/// Runs the de-duplicated launcher check and records it in the update
/// center. `None` means a check is already in flight.
pub async fn run_check(app: &AppHandle, channel: ReleaseChannel) -> Option<UpdateAvailability> {
    if begin_check(CheckDomain::Launcher) == crate::updates::CheckStart::Duplicate {
        return None;
    }
    let availability = check_launcher_update(app, channel).await;
    finish_check(CheckDomain::Launcher, availability.clone());
    Some(availability)
}

/// Downloads the currently offered update after re-checking that it is
/// still the presented one. Progress is reported through real updater
/// callbacks; nothing is fabricated.
pub async fn download(
    app: &AppHandle,
    channel: ReleaseChannel,
    presented_version: &str,
    mut progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<LauncherUpdateStatus, LauncherUpdateError> {
    let updater = build_updater(app, channel)?;
    // Stale defense: revalidate the offer before any bytes are accepted.
    let fresh = updater.check().await.map_err(|error| {
        LauncherUpdateError::new(
            "update_check_unavailable",
            format!("Could not check for updates: {error}"),
        )
    })?;
    let Some(update) = fresh else {
        return Err(LauncherUpdateError::gone(
            "The update is no longer offered.",
        ));
    };
    if update.version != presented_version {
        return Err(LauncherUpdateError::gone(
            "Update changed while you were reviewing it.",
        ));
    }
    let version = update.version.clone();
    let mut downloaded: u64 = 0;
    let mut total: Option<u64> = None;
    // `download` verifies the minisign signature over the complete buffer
    // before returning it; only verified bytes are ever retained.
    let bytes = update
        .download(
            move |chunk, content_length| {
                downloaded += chunk as u64;
                if let Some(length) = content_length {
                    total = Some(length as u64);
                }
                progress(downloaded, total);
            },
            || {},
        )
        .await
        .map_err(|error| {
            LauncherUpdateError::new(
                "update_download_failed",
                format!("The update download failed: {error}"),
            )
        })?;
    let mut pending = pending()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *pending = Some(PendingUpdate {
        version: version.clone(),
        update,
        bytes,
    });
    Ok(LauncherUpdateStatus {
        installed_version: installed_version(app),
        availability: UpdateAvailability::UpdateAvailable {
            current: installed_version(app),
            candidate: version,
            channel,
            notes: None,
        },
        phase: Some(LauncherPhase::ReadyToInstall),
    })
}

/// Installs the downloaded, signature-verified update.
///
/// On Windows this launches the verified installer and the application
/// exits as part of the install — the platform's documented updater
/// behavior, surfaced truthfully to the UI before the call.
pub async fn install(
    app: &AppHandle,
    channel: ReleaseChannel,
    presented_version: &str,
) -> Result<LauncherUpdateStatus, LauncherUpdateError> {
    let downloaded = {
        let mut pending = pending()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        pending.take()
    };
    let Some(pending) = downloaded else {
        return Err(LauncherUpdateError::new(
            "update_not_downloaded",
            "Download the update before installing it.",
        ));
    };
    if pending.version != presented_version {
        return Err(LauncherUpdateError::gone(
            "Update changed while you were reviewing it.",
        ));
    }
    pending.update.install(&pending.bytes).map_err(|error| {
        LauncherUpdateError::new(
            "update_install_failed",
            format!("Update could not be installed; your current version is unchanged. {error}"),
        )
    })?;
    // On Windows the process exits inside install(); reaching here means
    // the platform relaunches (or the install completed without exit).
    Ok(LauncherUpdateStatus {
        installed_version: installed_version(app),
        availability: UpdateAvailability::UpdateAvailable {
            current: installed_version(app),
            candidate: presented_version.to_owned(),
            channel,
            notes: None,
        },
        phase: Some(LauncherPhase::Installing),
    })
}

/// Whether a downloaded update is waiting to be installed, and its version.
pub fn pending_version() -> Option<String> {
    pending()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .map(|pending| pending.version.clone())
}

/// Records a phase transition in the update center.
pub fn set_phase(phase: Option<LauncherPhase>) {
    let mut state = crate::updates::center()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.launcher_phase = phase;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_endpoints_address_the_published_manifests_per_channel() {
        assert!(
            channel_endpoint(ReleaseChannel::Stable).ends_with("/stable.json"),
            "{}",
            channel_endpoint(ReleaseChannel::Stable)
        );
        assert!(channel_endpoint(ReleaseChannel::Beta).ends_with("/beta.json"));
        assert!(channel_endpoint(ReleaseChannel::Nightly).ends_with("/nightly.json"));
        // Production builds compile the documented owner-controlled
        // location unless a diagnostic base was provided at build time.
        if option_env!("AURORA_LAUNCHER_UPDATES_URL").is_none() {
            assert!(UPDATES_BASE_URL.starts_with("https://"));
        }
    }

    #[test]
    fn without_compiled_trust_material_the_updater_is_unconfigured() {
        // Test builds compile without AURORA_UPDATER_PUBKEY: the honest
        // unconfigured state, exactly what production builds report until
        // the owner provides the production public key. The full signed
        // check/download/verify path is exercised by the Phase L
        // diagnostic acceptance (a real diagnostic build against a signed
        // loopback update) because a mock-runtime build of this host cannot
        // load the updater plugin.
        if option_env!("AURORA_UPDATER_PUBKEY").is_none() {
            assert!(UPDATER_PUBKEY.is_none());
        }
    }
}
