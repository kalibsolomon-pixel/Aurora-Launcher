//! The Aurora Launcher self-update: a native wrapper over the official
//! Tauri 2 updater plugin.
//!
//! Trust model, stated honestly: the updater enforces minisign-style
//! signature verification against a public verification key compiled into
//! the launcher — it cannot be disabled, and no frontend- or file-supplied
//! URL or key ever participates. Builds without a compiled public key report
//! honestly unconfigured rather than weakening verification. The production
//! private key remains solely in the owner's protected signing environment.
//!
//! The updater endpoint serves one owner-published production manifest; nothing about a commit, branch, or CI build is ever an
//! update candidate. On Windows the install step launches the verified
//! NSIS installer and the application exits — that platform behavior is
//! surfaced truthfully instead of being hidden behind a fabricated restart.

use std::sync::Mutex;
use std::sync::OnceLock;

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::updates::{CheckDomain, LauncherPhase, UpdateAvailability, begin_check, finish_check};

/// The minisign-style public verification key compiled into this build.
/// `None` means this build was produced without updater trust material:
/// every check reports unconfigured, and nothing is ever downloaded or run.
pub const UPDATER_PUBKEY: Option<&str> = option_env!("AURORA_UPDATER_PUBKEY");

/// One authoritative production manifest, published only through the owner's
/// manual release gate. No environment or runtime endpoint override exists.
pub const PRODUCTION_MANIFEST_URL: &str = "https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-update-authority/launcher-update.json";

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
    update: tauri_plugin_updater::Update,
    installer: VerifiedInstaller,
}

struct VerifiedInstaller {
    version: String,
    url: url::Url,
    signature: String,
    /// The signature-verified installer bytes returned by `download`;
    /// `install` consumes exactly these bytes, never a re-download.
    bytes: Vec<u8>,
}

impl VerifiedInstaller {
    // The production caller supplies only the result of official Update::download.
    // An error (including failed signature verification) cannot become retained bytes.
    fn from_download(
        version: String,
        url: url::Url,
        signature: String,
        download: Result<Vec<u8>, tauri_plugin_updater::Error>,
    ) -> Result<Self, LauncherUpdateError> {
        let bytes = download.map_err(|error| {
            LauncherUpdateError::new(
                "update_download_failed",
                format!("The update download failed: {error}"),
            )
        })?;
        Ok(Self {
            version,
            url,
            signature,
            bytes,
        })
    }

    fn bytes_for_install(
        &self,
        presented: &str,
        version: &str,
        url: &url::Url,
        signature: &str,
    ) -> Result<&[u8], LauncherUpdateError> {
        if presented != self.version
            || version != self.version
            || url != &self.url
            || signature != self.signature
        {
            return Err(LauncherUpdateError::gone(
                "The launcher update changed. Check again.",
            ));
        }
        validate_artifact(url, version)?;
        Ok(&self.bytes)
    }
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

/// Builds the configured updater for production. Fails closed when this
/// build carries no trust material.
fn build_updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, LauncherUpdateError> {
    let pubkey = UPDATER_PUBKEY.ok_or_else(|| {
        LauncherUpdateError::new(
            "updater_unconfigured",
            "Launcher updates are not configured in this build.",
        )
    })?;
    let endpoint = url::Url::parse(PRODUCTION_MANIFEST_URL).map_err(|_| {
        LauncherUpdateError::new("updater_unconfigured", "The update endpoint is not valid.")
    })?;
    validate_authority(&endpoint)?;
    build_configured_updater(app, pubkey, endpoint)
}

/// Production construction with the compiled key and fixed endpoint. Boundary
/// tests exercise the shared HTTP policy and install identity checks, not this
/// desktop plugin builder or a full official download/install transaction.
fn build_configured_updater<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pubkey: &str,
    endpoint: url::Url,
) -> Result<tauri_plugin_updater::Updater, LauncherUpdateError> {
    validate_authority(&endpoint)?;
    app.updater_builder()
        .pubkey(pubkey.to_owned())
        .timeout(std::time::Duration::from_secs(30))
        .header("Cache-Control", "no-cache")
        .map_err(|_| LauncherUpdateError::new("updater_unconfigured", "Invalid cache policy."))?
        .configure_client(configure_updater_client)
        .endpoints(vec![endpoint])
        .map_err(|error| LauncherUpdateError::new("updater_unconfigured", error.to_string()))?
        .build()
        .map_err(|error| LauncherUpdateError::new("updater_unconfigured", error.to_string()))
}

/// The one-shot read-only launcher update check for production. The check
/// itself never installs anything; failures are honest unavailability.
pub async fn check_launcher_update(app: &AppHandle) -> UpdateAvailability {
    let Some(_) = UPDATER_PUBKEY else {
        return UpdateAvailability::unavailable(
            "Launcher updates are not configured in this build.",
        );
    };
    let updater = match build_updater(app) {
        Ok(updater) => updater,
        Err(error) => return UpdateAvailability::unavailable(error.message),
    };
    match updater.check().await {
        Ok(None) => UpdateAvailability::UpToDate,
        Ok(Some(update)) => UpdateAvailability::UpdateAvailable {
            current: installed_version(app),
            candidate: update.version.clone(),
            notes: update.body.clone(),
        },
        Err(error) => {
            UpdateAvailability::unavailable(format!("Could not check for updates: {error}"))
        }
    }
}

/// Runs the de-duplicated launcher check and records it in the update
/// center. `None` means a check is already in flight.
pub async fn run_check(app: &AppHandle) -> Option<UpdateAvailability> {
    if begin_check(CheckDomain::Launcher) == crate::updates::CheckStart::Duplicate {
        return None;
    }
    let availability = check_launcher_update(app).await;
    finish_check(CheckDomain::Launcher, availability.clone());
    Some(availability)
}

/// Downloads the currently offered update after re-checking that it is
/// still the presented one. Progress is reported through real updater
/// callbacks; nothing is fabricated.
pub async fn download(
    app: &AppHandle,
    presented_version: &str,
    mut progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<LauncherUpdateStatus, LauncherUpdateError> {
    let updater = build_updater(app)?;
    // Stale defense: revalidate the offer before any bytes are accepted.
    let fresh = updater.check().await.map_err(|error| {
        LauncherUpdateError::new(
            "update_check_unavailable",
            format!("Could not check for updates: {error}"),
        )
    })?;
    let Some(mut update) = fresh else {
        return Err(LauncherUpdateError::gone(
            "The update is no longer offered.",
        ));
    };
    if update.version != presented_version {
        return Err(LauncherUpdateError::gone(
            "Update changed while you were reviewing it.",
        ));
    }
    validate_artifact(&update.download_url, &update.version)?;
    update.timeout = Some(std::time::Duration::from_secs(300));
    let version = update.version.clone();
    let mut downloaded: u64 = 0;
    let mut total: Option<u64> = None;
    // `download` verifies the minisign signature over the complete buffer
    // before returning it; only verified bytes are ever retained.
    let downloaded = update
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
        .await;
    let installer = VerifiedInstaller::from_download(
        version.clone(),
        update.download_url.clone(),
        update.signature.clone(),
        downloaded,
    )?;
    let mut pending = pending()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *pending = Some(PendingUpdate { update, installer });
    Ok(LauncherUpdateStatus {
        installed_version: installed_version(app),
        availability: UpdateAvailability::UpdateAvailable {
            current: installed_version(app),
            candidate: version,
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
    if pending.installer.version != presented_version {
        return Err(LauncherUpdateError::gone(
            "Update changed while you were reviewing it.",
        ));
    }
    // The one-click UI still crosses two native steps. Revalidate the exact
    // offered installer before installation, including same-version drift.
    let fresh = build_updater(app)?
        .check()
        .await
        .map_err(|_| {
            LauncherUpdateError::new(
                "update_check_unavailable",
                "Could not revalidate the launcher update.",
            )
        })?
        .ok_or_else(|| LauncherUpdateError::gone("The update is no longer offered."))?;
    let bytes = pending.installer.bytes_for_install(
        presented_version,
        &fresh.version,
        &fresh.download_url,
        &fresh.signature,
    )?;
    pending.update.install(bytes).map_err(|error| {
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
        .map(|pending| pending.installer.version.clone())
}

/// Records a phase transition in the update center.
pub fn set_phase(phase: Option<LauncherPhase>) {
    let mut state = crate::updates::center()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.launcher_phase = phase;
}

fn secure_url(endpoint: &url::Url) -> bool {
    endpoint.scheme() == "https"
        && endpoint.username().is_empty()
        && endpoint.password().is_none()
        && endpoint.port_or_known_default() == Some(443)
        && endpoint.query().is_none()
        && endpoint.fragment().is_none()
}

fn validate_authority(endpoint: &url::Url) -> Result<(), LauncherUpdateError> {
    if secure_url(endpoint) && endpoint.as_str() == PRODUCTION_MANIFEST_URL {
        Ok(())
    } else {
        Err(LauncherUpdateError::new(
            "updater_unconfigured",
            "Unexpected launcher update authority.",
        ))
    }
}

fn validate_artifact(endpoint: &url::Url, version: &str) -> Result<(), LauncherUpdateError> {
    #[cfg(test)]
    if endpoint.scheme() == "http"
        && crate::downloads::is_loopback_host(endpoint)
        && endpoint.username().is_empty()
        && endpoint.password().is_none()
    {
        return Ok(());
    }
    validate_production_artifact(endpoint, version)
}

fn validate_production_artifact(
    endpoint: &url::Url,
    version: &str,
) -> Result<(), LauncherUpdateError> {
    let parsed = semver::Version::parse(version).ok();
    let valid_version =
        parsed.is_some_and(|v| v.to_string() == version && v.pre.is_empty() && v.build.is_empty());
    let expected = format!(
        "https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/v{version}/Aurora.Launcher_{version}_x64-setup.exe"
    );
    if valid_version && secure_url(endpoint) && endpoint.as_str() == expected {
        Ok(())
    } else {
        Err(LauncherUpdateError::new(
            "update_stale",
            "Unexpected signed launcher artifact location.",
        ))
    }
}

fn configure_updater_client(client: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    client.redirect(reqwest::redirect::Policy::custom(|attempt| {
        if redirect_allowed(attempt.previous(), attempt.url()) {
            attempt.follow()
        } else {
            attempt.error("Unexpected updater redirect")
        }
    }))
}

// The raw authority never redirects. GitHub installers require its asset CDN;
// redirect transport remains HTTPS, while the compiled key verifies the payload.
fn redirect_allowed(previous: &[url::Url], next: &url::Url) -> bool {
    previous.len() < 5
        && previous.first().is_some_and(|first| {
            first.host_str() == Some("github.com")
                && first
                    .path()
                    .starts_with("/kalibsolomon-pixel/Aurora-Launcher/releases/download/")
        })
        && next.scheme() == "https"
        && next.username().is_empty()
        && next.password().is_none()
        && next.port_or_known_default() == Some(443)
        && next.host_str() == Some("release-assets.githubusercontent.com")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_production_manifest_is_the_only_launcher_source() {
        assert_eq!(
            PRODUCTION_MANIFEST_URL,
            "https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-update-authority/launcher-update.json"
        );
        assert!(validate_authority(&url::Url::parse(PRODUCTION_MANIFEST_URL).unwrap()).is_ok());
    }
    #[test]
    fn without_compiled_trust_material_the_updater_is_unconfigured() {
        if option_env!("AURORA_UPDATER_PUBKEY").is_none() {
            assert!(UPDATER_PUBKEY.is_none());
        }
    }
    #[test]
    fn authority_is_exact_and_has_no_runtime_override() {
        for invalid in [
            "http://localhost/update.json",
            "https://example.com/update.json",
            "https://raw.githubusercontent.com/foreign/repo/launcher-update-authority/launcher-update.json",
            "https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/main/launcher-update.json",
            "https://user:secret@raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-update-authority/launcher-update.json",
        ] {
            assert!(validate_authority(&url::Url::parse(invalid).unwrap()).is_err());
        }
        for suffix in ["?x=1", "#fragment"] {
            assert!(
                validate_authority(
                    &url::Url::parse(&format!("{PRODUCTION_MANIFEST_URL}{suffix}")).unwrap()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn installer_is_pinned_to_version_repository_and_nsis() {
        let valid = "https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/v1.4.1/Aurora.Launcher_1.4.1_x64-setup.exe";
        assert!(validate_artifact(&url::Url::parse(valid).unwrap(), "1.4.1").is_ok());
        for wrong in [
            valid.replace("1.4.1", "1.4.0"),
            valid.replace("kalibsolomon-pixel", "foreign"),
            valid.replace("https:", "http:"),
            format!("{valid}?x=1"),
            valid.replace("setup.exe", "setup.msi"),
        ] {
            assert!(validate_artifact(&url::Url::parse(&wrong).unwrap(), "1.4.1").is_err());
        }
    }
    #[test]
    fn redirects_cannot_replace_authority_or_leave_asset_cdn() {
        let raw = url::Url::parse(PRODUCTION_MANIFEST_URL).unwrap();
        let cdn = url::Url::parse("https://release-assets.githubusercontent.com/github-production-release-asset/1?token=opaque").unwrap();
        assert!(!redirect_allowed(&[raw], &cdn));
        let github = url::Url::parse("https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/v1.4.1/Aurora.Launcher_1.4.1_x64-setup.exe").unwrap();
        assert!(redirect_allowed(&[github.clone()], &cdn));
        assert!(!redirect_allowed(
            &[github.clone()],
            &url::Url::parse("https://evil.example/payload").unwrap()
        ));
        assert!(!redirect_allowed(
            &[github],
            &url::Url::parse("http://release-assets.githubusercontent.com/payload").unwrap()
        ));
    }

    #[test]
    fn production_artifact_policy_is_tested_without_loopback_exception() {
        let valid = "https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/v1.4.1/Aurora.Launcher_1.4.1_x64-setup.exe";
        assert!(validate_production_artifact(&url::Url::parse(valid).unwrap(), "1.4.1").is_ok());
        for wrong in [
            valid.replace("Aurora-Launcher", "foreign"),
            valid.replace("github.com", "github.com.evil.example"),
            valid.replace("github.com", "user:password@github.com"),
            valid.replace("github.com", "github.com:8443"),
            valid.replace("github.com", "raw.githubusercontent.com"),
            valid.replace("download/v1.4.1", "latest/download"),
            valid.replace("Aurora.Launcher_", "%41urora.Launcher_"),
            valid.replace("Aurora.Launcher_", "%zzAurora.Launcher_"),
            format!("{valid}#fragment"),
            "http://127.0.0.1/installer.exe".to_owned(),
        ] {
            if let Ok(url) = url::Url::parse(&wrong) {
                assert!(
                    validate_production_artifact(&url, "1.4.1").is_err(),
                    "{wrong}"
                );
            }
        }
        for version in ["01.4.1", "1.4.1-beta", "1.4.1+build"] {
            assert!(
                validate_production_artifact(&url::Url::parse(valid).unwrap(), version).is_err()
            );
        }
        for wrong in [
            PRODUCTION_MANIFEST_URL.replace(
                "raw.githubusercontent.com",
                "raw.githubusercontent.com:8443",
            ),
            PRODUCTION_MANIFEST_URL.replace("launcher-update.json", "%6cauncher-update.json"),
            PRODUCTION_MANIFEST_URL
                .replace("launcher-update.json", "launcher-update.json?x=1#fragment"),
        ] {
            assert!(validate_authority(&url::Url::parse(&wrong).unwrap()).is_err());
        }
    }

    #[test]
    fn failed_verified_download_cannot_create_retained_installer() {
        let result = VerifiedInstaller::from_download(
            "1.4.1".to_owned(),
            url::Url::parse("https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/v1.4.1/Aurora.Launcher_1.4.1_x64-setup.exe").unwrap(),
            "diagnostic signature".to_owned(),
            Err(tauri_plugin_updater::Error::Network("diagnostic verification failure".to_owned())),
        );
        assert!(matches!(result, Err(error) if error.code == "update_download_failed"));
    }

    #[test]
    fn install_recheck_returns_original_bytes_only_for_exact_identity() {
        let url = url::Url::parse("https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/v1.4.1/Aurora.Launcher_1.4.1_x64-setup.exe").unwrap();
        let bytes = b"harmless diagnostic bytes, not an installer".to_vec();
        let address = bytes.as_ptr();
        let installer = VerifiedInstaller::from_download(
            "1.4.1".to_owned(),
            url.clone(),
            "signature".to_owned(),
            Ok(bytes),
        )
        .unwrap();
        let retained = installer
            .bytes_for_install("1.4.1", "1.4.1", &url, "signature")
            .unwrap();
        assert_eq!(retained, b"harmless diagnostic bytes, not an installer");
        assert_eq!(retained.as_ptr(), address);
        let changed = url::Url::parse("https://example.invalid/changed.exe").unwrap();
        for (presented, version, location, signature) in [
            ("1.4.0", "1.4.1", &url, "signature"),
            ("1.4.1", "1.4.2", &url, "signature"),
            ("1.4.1", "1.4.1", &changed, "signature"),
            ("1.4.1", "1.4.1", &url, "changed signature"),
        ] {
            assert!(
                installer
                    .bytes_for_install(presented, version, location, signature)
                    .is_err()
            );
        }
        assert_eq!(installer.bytes.as_ptr(), address);
    }

    #[tokio::test]
    async fn configured_http_client_rejects_redirect_before_second_request() {
        use crate::test_support::{TestResponse, TestServer};
        use std::sync::Arc;
        // Standalone execution must not rely on another test installing TLS.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let server = TestServer::spawn(Arc::new(|_| {
            TestResponse::status(302).with_header("Location", "/forbidden")
        }));
        let client = configure_updater_client(reqwest::Client::builder())
            .no_proxy()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap();
        let error = client
            .get(format!("{}/manifest", server.base_url()))
            .send()
            .await
            .unwrap_err();
        assert!(error.is_redirect());
        assert_eq!(server.request_count(), 1);
    }

    #[test]
    fn redirect_hops_credentials_ports_and_cdn_lookalikes_fail() {
        let github = url::Url::parse("https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/v1.4.1/Aurora.Launcher_1.4.1_x64-setup.exe").unwrap();
        let cdn =
            url::Url::parse("https://release-assets.githubusercontent.com/asset?token=opaque")
                .unwrap();
        assert!(!redirect_allowed(&vec![github.clone(); 5], &cdn));
        for wrong in [
            "https://user:password@release-assets.githubusercontent.com/asset",
            "https://release-assets.githubusercontent.com:8443/asset",
            "https://release-assets.githubusercontent.com.evil.example/asset",
        ] {
            assert!(!redirect_allowed(
                std::slice::from_ref(&github),
                &url::Url::parse(wrong).unwrap()
            ));
        }
    }
}
