//! Signed, native-only preview distribution. This never reads the WebView
//! authority. Publication/enablement is a separate owner operation.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use base64ct::{Base64, Encoding};
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};

use super::launcher::{LauncherUpdateError as Error, LauncherUpdateStatus, UPDATER_PUBKEY};
use super::{LauncherPhase, UpdateAvailability};
use crate::host::Host;
use crate::integrity::{ArtifactDigest, verify_file};

// Proposed discovery location only. No ref is initialized or published by a
// build. Both owner-supplied compile-time opt-in and the existing trust key are
// required; environment/runtime/frontend overrides are deliberately absent.
pub const AUTHORITY: &str = "https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-native-update-authority/native-update.json";
const PRODUCT: &str = "com.aurora.launcher.native-preview";
const MAX_INSTALLER: u64 = 256 * 1024 * 1024;
const MAX_ENVELOPE: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    payload: String,
    signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Offer {
    schema_version: u32,
    product: String,
    channel: String,
    runtime: String,
    platform: String,
    architecture: String,
    package_type: String,
    version: String,
    minimum_installed_version: String,
    source_sha: String,
    url: String,
    size_bytes: u64,
    sha256: String,
    installer_signature: String,
}

fn failure() -> Error {
    Error::new(
        "native_update_invalid",
        "The native update failed authenticity, integrity or compatibility validation.",
    )
}

fn decode(value: &str) -> Result<Vec<u8>, Error> {
    if value.is_empty() || value.len() > MAX_ENVELOPE {
        return Err(failure());
    }
    let bytes = Base64::decode_vec(value).map_err(|_| failure())?;
    if Base64::encode_string(&bytes) != value {
        return Err(failure());
    }
    Ok(bytes)
}

fn key(encoded: &str) -> Result<PublicKey, Error> {
    PublicKey::decode(std::str::from_utf8(&decode(encoded)?).map_err(|_| failure())?)
        .map_err(|_| failure())
}

fn signature(encoded: &str) -> Result<Signature, Error> {
    Signature::decode(std::str::from_utf8(&decode(encoded)?).map_err(|_| failure())?)
        .map_err(|_| failure())
}

fn version(value: &str) -> Result<semver::Version, Error> {
    let parsed = semver::Version::parse(value).map_err(|_| failure())?;
    if parsed.to_string() != value || !parsed.pre.is_empty() || !parsed.build.is_empty() {
        return Err(failure());
    }
    Ok(parsed)
}

fn installer_name(version: &str) -> String {
    format!("Aurora.Native.Preview_{version}_x64-setup.exe")
}

impl Offer {
    fn validate(&self, installed: &str) -> Result<(), Error> {
        let candidate = version(&self.version)?;
        let minimum = version(&self.minimum_installed_version)?;
        let current = version(installed)?;
        let url = format!(
            "https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v{}/{}",
            self.version,
            installer_name(&self.version)
        );
        if self.schema_version != 1
            || self.product != PRODUCT
            || self.channel != "native-preview"
            || self.runtime != "slint"
            || self.platform != "windows"
            || self.architecture != "x86_64"
            || self.package_type != "nsis-per-user"
            || minimum > current
            || minimum > candidate
            || self.url != url
            || self.size_bytes == 0
            || self.size_bytes > MAX_INSTALLER
            || self.source_sha.len() != 40
            || !self
                .source_sha
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || ArtifactDigest::parse(&self.sha256).is_err()
            || self.sha256 != self.sha256.to_ascii_lowercase()
        {
            return Err(failure());
        }
        signature(&self.installer_signature)?;
        Ok(())
    }
}

/// Verify the signature before interpreting any payload field as authority.
/// Public for the offline release/acceptance driver; it cannot execute bytes.
pub fn verify_offer(bytes: &[u8], public_key: &str, installed: &str) -> Result<Offer, Error> {
    if bytes.len() > MAX_ENVELOPE {
        return Err(failure());
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| failure())?;
    let payload = decode(&envelope.payload)?;
    key(public_key)?
        .verify(&payload, &signature(&envelope.signature)?, false)
        .map_err(|_| failure())?;
    let offer: Offer = serde_json::from_slice(&payload).map_err(|_| failure())?;
    offer.validate(installed)?;
    Ok(offer)
}

/// Rehash and stream-verify the exact file, including the authenticated filename.
pub fn verify_installer(path: &Path, offer: &Offer, public_key: &str) -> Result<(), Error> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| failure())?;
    if !metadata.is_file() || redirected(&metadata) {
        return Err(failure());
    }
    verify_file(
        path,
        &ArtifactDigest::parse(&offer.sha256).map_err(|_| failure())?,
        Some(offer.size_bytes),
    )
    .map_err(|_| failure())?;
    let public_key = key(public_key)?;
    let signature = signature(&offer.installer_signature)?;
    let mut verifier = public_key
        .verify_stream(&signature)
        .map_err(|_| failure())?;
    let mut file = std::fs::File::open(path).map_err(|_| failure())?;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|_| failure())?;
        if count == 0 {
            break;
        }
        verifier.update(&buffer[..count]);
    }
    verifier.finalize().map_err(|_| failure())?;
    let files: Vec<_> = signature
        .trusted_comment()
        .split('\t')
        .filter_map(|f| f.strip_prefix("file:"))
        .collect();
    let versions: Vec<_> = signature
        .trusted_comment()
        .split('\t')
        .filter_map(|f| f.strip_prefix("version:"))
        .collect();
    if files != [installer_name(&offer.version).as_str()]
        || versions.len() > 1
        || versions.first().is_some_and(|v| *v != offer.version)
    {
        return Err(failure());
    }
    Ok(())
}

fn configured_key() -> Result<&'static str, Error> {
    if !cfg!(all(target_os = "windows", target_arch = "x86_64"))
        || option_env!("AURORA_NATIVE_UPDATE_AUTHORITY") != Some(AUTHORITY)
    {
        return Err(Error::new(
            "updater_unconfigured",
            "Native preview updates are not enabled. Use the separately accepted native installer.",
        ));
    }
    let value = UPDATER_PUBKEY.ok_or_else(|| {
        Error::new(
            "updater_unconfigured",
            "Native update verification is not configured.",
        )
    })?;
    key(value)?;
    Ok(value)
}

fn client(artifacts: bool) -> Result<reqwest::Client, Error> {
    crate::downloads::ensure_rustls_crypto_provider();
    reqwest::Client::builder()
        .user_agent(concat!("aurora-native/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(300))
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            let valid = artifacts
                && attempt.previous().len() < 5
                && attempt.previous().first().is_some_and(|u| {
                    u.host_str() == Some("github.com")
                        && u.path().starts_with(
                            "/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v",
                        )
                })
                && attempt.url().scheme() == "https"
                && attempt.url().host_str() == Some("release-assets.githubusercontent.com")
                && attempt.url().username().is_empty()
                && attempt.url().password().is_none()
                && attempt.url().port_or_known_default() == Some(443);
            if valid {
                attempt.follow()
            } else {
                attempt.error("Native updater redirect refused")
            }
        }))
        .build()
        .map_err(|_| failure())
}

async fn discover(app: &Host) -> Result<Offer, Error> {
    let public_key = configured_key()?;
    let mut response = client(false)?
        .get(AUTHORITY)
        .header("Cache-Control", "no-cache")
        .send()
        .await
        .map_err(|_| {
            Error::new(
                "update_check_unavailable",
                "Could not check native updates.",
            )
        })?
        .error_for_status()
        .map_err(|_| {
            Error::new(
                "update_check_unavailable",
                "The native update authority is unavailable.",
            )
        })?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| failure())? {
        if bytes.len() + chunk.len() > MAX_ENVELOPE {
            return Err(failure());
        }
        bytes.extend_from_slice(&chunk);
    }
    verify_offer(&bytes, public_key, &app.version())
}

fn status(app: &Host, offer: &Offer, phase: LauncherPhase) -> LauncherUpdateStatus {
    LauncherUpdateStatus {
        installed_version: app.version(),
        availability: UpdateAvailability::UpdateAvailable {
            current: app.version(),
            candidate: offer.version.clone(),
            notes: None,
        },
        phase: Some(phase),
    }
}

pub async fn check(app: &Host) -> UpdateAvailability {
    match discover(app).await {
        Ok(offer) if version(&offer.version).ok() > version(&app.version()).ok() => {
            status(app, &offer, LauncherPhase::Checking).availability
        }
        Ok(_) => UpdateAvailability::UpToDate,
        Err(error) => UpdateAvailability::unavailable(error.message),
    }
}

struct Pending {
    offer: Offer,
    path: PathBuf,
}
fn pending() -> &'static Mutex<Option<Pending>> {
    static SLOT: OnceLock<Mutex<Option<Pending>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}
pub fn pending_version() -> Option<String> {
    pending()
        .lock()
        .expect("native update slot")
        .as_ref()
        .map(|p| p.offer.version.clone())
}

fn redirected(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn safe_directory(path: &Path) -> Result<(), Error> {
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor).map_err(|_| failure())?;
        if !metadata.is_dir() || redirected(&metadata) {
            return Err(failure());
        }
    }
    Ok(())
}

// Only exact files/directories created by this operation are removed on error.
struct Staging {
    root: PathBuf,
    path: PathBuf,
    retained: bool,
}
impl Drop for Staging {
    fn drop(&mut self) {
        if !self.retained {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_dir(&self.root);
        }
    }
}

pub async fn download(
    app: &Host,
    presented: &str,
    mut progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<LauncherUpdateStatus, Error> {
    let offer = discover(app).await?;
    if offer.version != presented || version(presented)? <= version(&app.version())? {
        return Err(Error::gone("The native update changed. Check again."));
    }
    let managed = crate::paths::ManagedPaths::from_app_local_data_dir(
        app.local_data_dir().map_err(|_| failure())?,
    )
    .map_err(|_| failure())?;
    safe_directory(managed.data_root())?;
    let mut directory = managed.data_root().to_path_buf();
    for name in ["cache", "native-updates"] {
        directory.push(name);
        if !directory.exists() {
            std::fs::create_dir(&directory).map_err(|_| failure())?;
        }
        safe_directory(&directory)?;
    }
    directory.push(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir(&directory).map_err(|_| failure())?;
    let mut staging = Staging {
        path: directory.join("installer.part"),
        root: directory,
        retained: false,
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging.path)
        .map_err(|_| failure())?;
    let mut response = client(true)?
        .get(&offer.url)
        .send()
        .await
        .map_err(|_| failure())?
        .error_for_status()
        .map_err(|_| failure())?;
    if response
        .content_length()
        .is_some_and(|n| n != offer.size_bytes)
    {
        return Err(failure());
    }
    let mut done = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|_| failure())? {
        done = done.checked_add(chunk.len() as u64).ok_or_else(failure)?;
        if done > offer.size_bytes {
            return Err(failure());
        }
        file.write_all(&chunk).map_err(|_| failure())?;
        progress(done, Some(offer.size_bytes));
    }
    file.sync_all().map_err(|_| failure())?;
    drop(file);
    if done != offer.size_bytes {
        return Err(failure());
    }
    verify_installer(&staging.path, &offer, configured_key()?)?;
    let executable = staging.root.join("verified-setup.exe");
    std::fs::rename(&staging.path, &executable).map_err(|_| failure())?;
    staging.path = executable;
    let status = status(app, &offer, LauncherPhase::ReadyToInstall);
    *pending().lock().expect("native update slot") = Some(Pending {
        offer,
        path: staging.path.clone(),
    });
    staging.retained = true;
    Ok(status)
}

pub async fn install(app: &Host, presented: &str) -> Result<LauncherUpdateStatus, Error> {
    let offer = discover(app).await?;
    let managed = crate::paths::ManagedPaths::from_app_local_data_dir(
        app.local_data_dir().map_err(|_| failure())?,
    )
    .map_err(|_| failure())?;
    let registry = crate::instances::InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|_| failure())?;
    if registry.instances().iter().any(|instance| {
        matches!(
            crate::launch::process::snapshot(instance.id().as_str()).status,
            crate::launch::state::LaunchProcessStatus::Starting
                | crate::launch::state::LaunchProcessStatus::Running
        )
    }) {
        return Err(Error::new(
            "native_update_busy",
            "Close running games before installing a native update.",
        ));
    }
    let mut slot = pending().lock().expect("native update slot");
    let candidate = slot
        .as_ref()
        .ok_or_else(|| Error::new("update_not_downloaded", "Download the native update first."))?;
    if candidate.offer != offer
        || presented != offer.version
        || version(presented)? <= version(&app.version())?
    {
        return Err(Error::gone("The native update changed. Download it again."));
    }
    let executable = std::env::current_exe().map_err(|_| failure())?;
    let expected = crate::host::platform_local_data_dir("Aurora Launcher Native Preview")
        .map_err(|_| failure())?
        .join("aurora-native.exe");
    safe_directory(executable.parent().ok_or_else(failure)?)?;
    if executable.canonicalize().ok() != expected.canonicalize().ok() || !expected.is_file() {
        return Err(Error::new(
            "native_update_installation_required",
            "Install the native preview package before applying updates.",
        ));
    }
    safe_directory(candidate.path.parent().ok_or_else(failure)?)?;
    verify_installer(&candidate.path, &candidate.offer, configured_key()?)?;
    // The installer opens this exact live process before waiting for it. No
    // process-name discovery/termination and no in-place running-exe overwrite.
    std::process::Command::new(&candidate.path).args(["/S", "/RESTART", &format!("/UPDATEPID={}", std::process::id())]).spawn().map_err(|_| Error::new("update_install_failed", "The verified native installer could not start. The installed version is unchanged."))?;
    *slot = None;
    Ok(status(app, &offer, LauncherPhase::Installing))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("fixtures/native-test.json")).unwrap()
    }

    #[test]
    fn native9_signed_native_identity_is_required_even_with_a_valid_signing_key() {
        let fixture = fixture();
        let public_key = fixture["key"].as_str().unwrap();
        let valid = serde_json::to_vec(&fixture["valid"]).unwrap();
        assert!(verify_offer(&valid, public_key, "1.4.2").is_ok());
        assert!(verify_offer(&valid, public_key, "1.4.1").is_err());
        for invalid in fixture["invalid"].as_array().unwrap() {
            assert!(
                verify_offer(&serde_json::to_vec(invalid).unwrap(), public_key, "1.4.2").is_err()
            );
        }
        let mut changed = fixture["valid"].clone();
        let payload = decode(changed["payload"].as_str().unwrap()).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        value["version"] = "9.9.9".into();
        changed["payload"] = Base64::encode_string(&serde_json::to_vec(&value).unwrap()).into();
        assert!(verify_offer(&serde_json::to_vec(&changed).unwrap(), public_key, "1.4.2").is_err());
        assert!(
            verify_offer(
                br#"{"version":"9.9.9","platforms":{}}"#,
                public_key,
                "1.4.2"
            )
            .is_err()
        );
        assert!(verify_offer(&valid[..valid.len() / 2], public_key, "1.4.2").is_err());
        assert!(verify_offer(&vec![b' '; MAX_ENVELOPE + 1], public_key, "1.4.2").is_err());
    }

    #[test]
    fn native9_installer_requires_signature_hash_size_and_authenticated_filename() {
        let fixture = fixture();
        let public_key = fixture["key"].as_str().unwrap();
        let offer = verify_offer(
            &serde_json::to_vec(&fixture["valid"]).unwrap(),
            public_key,
            "1.4.2",
        )
        .unwrap();
        let root =
            std::env::temp_dir().join(format!("aurora-native9-signature-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("harmless-fixture.bin");
        let bytes = decode(fixture["installer"].as_str().unwrap()).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        verify_installer(&path, &offer, public_key).unwrap();
        let mut changed = offer.clone();
        changed.version = "1.4.4".into();
        assert!(verify_installer(&path, &changed, public_key).is_err());
        changed = offer.clone();
        changed.sha256 = "0".repeat(64);
        assert!(verify_installer(&path, &changed, public_key).is_err());
        changed = offer.clone();
        changed.size_bytes += 1;
        assert!(verify_installer(&path, &changed, public_key).is_err());
        let mut bytes = bytes;
        bytes[0] ^= 1;
        std::fs::write(&path, &bytes).unwrap();
        assert!(verify_installer(&path, &offer, public_key).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }

    #[tokio::test]
    async fn native9_discovery_never_follows_a_redirect() {
        let server = crate::test_support::TestServer::spawn(std::sync::Arc::new(|_| {
            crate::test_support::TestResponse::status(302).with_header("Location", "/other")
        }));
        let error = client(false)
            .unwrap()
            .get(format!("{}/manifest", server.base_url()))
            .send()
            .await
            .unwrap_err();
        assert!(error.is_redirect());
        assert_eq!(server.request_count(), 1);
    }
}
