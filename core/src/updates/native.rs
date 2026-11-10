//! Signed, native-only preview distribution. This never reads the WebView
//! authority. Publication/enablement is a separate owner operation.
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use base64ct::{Base64, Encoding};
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "native-update-acceptance"))]
use super::launcher::UPDATER_PUBKEY;
use super::launcher::{LauncherUpdateError as Error, LauncherUpdateStatus};
use super::{LauncherPhase, UpdateAvailability};
use crate::host::Host;
use crate::integrity::{ArtifactDigest, verify_file};

// Proposed discovery location only. No ref is initialized or published by a
// build. Both owner-supplied compile-time opt-in and the existing trust key are
// required; environment/runtime/frontend overrides are deliberately absent.
pub const AUTHORITY: &str = "https://raw.githubusercontent.com/kalibsolomon-pixel/Aurora-Launcher/launcher-native-update-authority/native-update.json";
#[cfg(not(feature = "native-update-acceptance"))]
const PRODUCT: &str = "com.aurora.launcher.native-preview";
#[cfg(feature = "native-update-acceptance")]
const PRODUCT: &str = concat!(
    "com.aurora.launcher.native-preview.acceptance.",
    env!("AURORA_NATIVE_ACCEPTANCE_ID")
);
#[cfg(not(feature = "native-update-acceptance"))]
const CHANNEL: &str = "native-preview";
#[cfg(feature = "native-update-acceptance")]
const CHANNEL: &str = "native-acceptance";

fn discovery_url() -> &'static str {
    #[cfg(feature = "native-update-acceptance")]
    {
        concat!(
            env!("AURORA_NATIVE_ACCEPTANCE_ORIGIN"),
            "/native-update.json"
        )
    }
    #[cfg(not(feature = "native-update-acceptance"))]
    {
        AUTHORITY
    }
}
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
    #[cfg(not(feature = "native-update-acceptance"))]
    if self::version(version).is_ok_and(|v| v < semver::Version::new(2, 1, 1)) {
        return format!("Aurora.Native.Preview_{version}_x64-setup.exe");
    }
    format!("Aurora.Launcher_{version}_x64-setup.exe")
}

impl Offer {
    fn validate(&self, installed: &str) -> Result<(), Error> {
        let candidate = version(&self.version)?;
        let minimum = version(&self.minimum_installed_version)?;
        let current = version(installed)?;
        #[cfg(not(feature = "native-update-acceptance"))]
        let url = format!(
            "https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v{}/{}",
            self.version,
            installer_name(&self.version)
        );
        #[cfg(feature = "native-update-acceptance")]
        let url = format!(
            "{}/native-v{}/{}",
            env!("AURORA_NATIVE_ACCEPTANCE_ORIGIN"),
            self.version,
            installer_name(&self.version)
        );
        if self.schema_version != 1
            || self.product != PRODUCT
            || self.channel != CHANNEL
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
    #[cfg(feature = "native-update-acceptance")]
    {
        let value = env!("AURORA_NATIVE_ACCEPTANCE_KEY");
        key(value)?;
        return Ok(value);
    }
    #[cfg(not(feature = "native-update-acceptance"))]
    {
        if !cfg!(all(target_os = "windows", target_arch = "x86_64"))
            || option_env!("AURORA_NATIVE_UPDATE_AUTHORITY") != Some(AUTHORITY)
        {
            return Err(Error::new(
                "updater_unconfigured",
                "Aurora Launcher updates are not enabled. Install the latest Aurora Launcher release.",
            ));
        }
        let value = UPDATER_PUBKEY.ok_or_else(|| {
            Error::new(
                "updater_unconfigured",
                "Aurora Launcher update verification is not configured.",
            )
        })?;
        key(value)?;
        Ok(value)
    }
}

fn client(artifacts: bool) -> Result<reqwest::Client, Error> {
    crate::downloads::ensure_rustls_crypto_provider();
    reqwest::Client::builder()
        .user_agent(concat!("aurora-launcher/", env!("CARGO_PKG_VERSION")))
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
        .get(discovery_url())
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

/// Only a strictly newer offer can be shown, downloaded or installed. Equal,
/// older or unparsable versions (on either side) never count as an upgrade.
fn is_upgrade(candidate: &str, installed: &str) -> bool {
    matches!((version(candidate), version(installed)), (Ok(c), Ok(i)) if c > i)
}

pub async fn check(app: &Host) -> UpdateAvailability {
    match discover(app).await {
        Ok(offer) if is_upgrade(&offer.version, &app.version()) => {
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
    progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<LauncherUpdateStatus, Error> {
    let offer = discover(app).await?;
    if offer.version != presented || !is_upgrade(presented, &app.version()) {
        return Err(Error::gone("The native update changed. Check again."));
    }
    let managed = crate::paths::ManagedPaths::from_app_local_data_dir(
        app.local_data_dir().map_err(|_| failure())?,
    )
    .map_err(|_| failure())?;
    let path = stage_installer(&managed, &offer, configured_key()?, &offer.url, progress).await?;
    let status = status(app, &offer, LauncherPhase::ReadyToInstall);
    *pending().lock().expect("native update slot") = Some(Pending { offer, path });
    Ok(status)
}

// One transfer/verification implementation. The only production caller supplies
// the authenticated, pinned offer URL. Tests can exercise it over loopback;
// there is no runtime endpoint/key override or production signature bypass.
async fn stage_installer(
    managed: &crate::paths::ManagedPaths,
    offer: &Offer,
    public_key: &str,
    url: &str,
    mut progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<PathBuf, Error> {
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
        .get(url)
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
    verify_installer(&staging.path, offer, public_key)?;
    let executable = staging.root.join("verified-setup.exe");
    std::fs::rename(&staging.path, &executable).map_err(|_| failure())?;
    staging.path = executable;
    staging.retained = true;
    Ok(staging.path.clone())
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
            "Close running games before updating Aurora Launcher.",
        ));
    }
    let mut slot = pending().lock().expect("native update slot");
    let candidate = slot.as_ref().ok_or_else(|| {
        Error::new(
            "update_not_downloaded",
            "Download the Aurora Launcher update first.",
        )
    })?;
    if candidate.offer != offer
        || presented != offer.version
        || !is_upgrade(presented, &app.version())
    {
        return Err(Error::gone("The native update changed. Download it again."));
    }
    let executable = std::env::current_exe().map_err(|_| failure())?;
    #[cfg(not(feature = "native-update-acceptance"))]
    let expected = crate::host::platform_local_data_dir("Aurora Launcher Native Preview")
        .map_err(|_| failure())?
        .join("aurora-launcher.exe");
    #[cfg(feature = "native-update-acceptance")]
    let expected = PathBuf::from(env!("AURORA_NATIVE_ACCEPTANCE_ROOT"))
        .join("installation/aurora-launcher.exe");
    safe_directory(executable.parent().ok_or_else(failure)?)?;
    if executable.canonicalize().ok() != expected.canonicalize().ok() || !expected.is_file() {
        return Err(Error::new(
            "native_update_installation_required",
            "Install Aurora Launcher before applying updates.",
        ));
    }
    safe_directory(candidate.path.parent().ok_or_else(failure)?)?;
    verify_installer(&candidate.path, &candidate.offer, configured_key()?)?;
    // The installer opens this exact live process before waiting for it. No
    // process-name discovery/termination and no in-place running-exe overwrite.
    std::process::Command::new(&candidate.path).args(["/S", "/RESTART", &format!("/UPDATEPID={}", std::process::id())]).spawn().map_err(|_| Error::new("update_install_failed", "The verified Aurora Launcher installer could not start. The installed version is unchanged."))?;
    *slot = None;
    Ok(status(app, &offer, LauncherPhase::Installing))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(not(feature = "native-update-acceptance"))]
    fn launcher_names_keep_one_exact_filename_per_release_version() {
        assert_eq!(
            installer_name("2.1.0"),
            "Aurora.Native.Preview_2.1.0_x64-setup.exe"
        );
        assert_eq!(
            installer_name("2.1.1"),
            "Aurora.Launcher_2.1.1_x64-setup.exe"
        );
        assert_eq!(
            installer_name("3.0.0"),
            "Aurora.Launcher_3.0.0_x64-setup.exe"
        );
        let fixture = fixture();
        let payload = decode(fixture["valid"]["payload"].as_str().unwrap()).unwrap();
        let mut offer: Offer = serde_json::from_slice(&payload).unwrap();
        offer.version = "2.1.1".into();
        offer.url = format!(
            "https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.1/{}",
            installer_name("2.1.1")
        );
        assert!(offer.validate("2.1.0").is_ok());
        offer.url = offer
            .url
            .replace("Aurora.Launcher_", "Aurora.Native.Preview_");
        assert!(offer.validate("2.1.0").is_err());
        offer.version = "2.1.0".into();
        offer.url = "https://github.com/kalibsolomon-pixel/Aurora-Launcher/releases/download/native-v2.1.0/Aurora.Launcher_2.1.0_x64-setup.exe".into();
        assert!(offer.validate("2.0.1").is_err());
    }

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

    #[test]
    fn native9_only_strictly_newer_offers_are_upgrades() {
        assert!(is_upgrade("1.4.3", "1.4.2"));
        assert!(is_upgrade("2.0.0", "1.9.9"));
        assert!(!is_upgrade("1.4.2", "1.4.2"));
        assert!(!is_upgrade("1.4.1", "1.4.2"));
        assert!(!is_upgrade("1.4.3+build", "1.4.2"));
        assert!(!is_upgrade("1.4.3-rc.1", "1.4.2"));
        assert!(!is_upgrade("1.4.3", "not-a-version"));
        assert!(!is_upgrade("garbage", "1.4.2"));
        // A validly signed offer for the installed version or older passes
        // field validation; the upgrade rule is what refuses it.
        let fixture = fixture();
        let valid = serde_json::to_vec(&fixture["valid"]).unwrap();
        let offer = verify_offer(&valid, fixture["key"].as_str().unwrap(), "1.4.3").unwrap();
        assert!(!is_upgrade(&offer.version, "1.4.3"));
        assert!(!is_upgrade(&offer.version, "1.4.4"));
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

    #[tokio::test]
    async fn release_signed_transfer_progress_and_failure_preserve_existing_data() {
        use crate::test_support::{TestResponse, TestServer};
        let fixture = fixture();
        let key = fixture["key"].as_str().unwrap();
        let offer = verify_offer(
            &serde_json::to_vec(&fixture["valid"]).unwrap(),
            key,
            "1.4.2",
        )
        .unwrap();
        let bytes = decode(fixture["installer"].as_str().unwrap()).unwrap();
        let served = bytes.clone();
        let server = TestServer::spawn(std::sync::Arc::new(move |request| {
            match request.path.as_str() {
                "/ok" => TestResponse::ok(&served),
                "/corrupt" => TestResponse::ok(&vec![b'x'; served.len()]),
                "/interrupted" => TestResponse::ok(&served).with_truncated_body(3),
                _ => TestResponse::status(404),
            }
        }));
        let root = std::env::temp_dir().join(format!(
            "aurora-native-release-transfer-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let managed = crate::paths::ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let sentinel = root.join("owner-world-sentinel");
        std::fs::write(&sentinel, b"preserve").unwrap();
        let mut progress = Vec::new();
        let path = stage_installer(
            &managed,
            &offer,
            key,
            &format!("{}/ok", server.base_url()),
            |done, total| progress.push((done, total)),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            progress.last(),
            Some(&(offer.size_bytes, Some(offer.size_bytes)))
        );
        assert!(progress.windows(2).all(|p| p[0].0 <= p[1].0));
        for route in ["corrupt", "interrupted", "missing"] {
            assert!(
                stage_installer(
                    &managed,
                    &offer,
                    key,
                    &format!("{}/{route}", server.base_url()),
                    |_, _| {}
                )
                .await
                .is_err()
            );
            assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserve");
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert_eq!(
                std::fs::read_dir(root.join("cache/native-updates"))
                    .unwrap()
                    .count(),
                1,
                "failed operation staging must be removed without touching verified data"
            );
        }
        std::fs::write(
            path.parent().unwrap().join("installer.part"),
            b"interrupted staging evidence",
        )
        .unwrap();
        assert!(
            pending_version().is_none(),
            "on-disk staging cannot create process-local installation authority"
        );
        assert!(
            verify_offer(
                &serde_json::to_vec(&fixture["valid"]).unwrap(),
                "unknown identity",
                "1.4.2"
            )
            .is_err()
        );
        // Exact disposable root positively created by this test.
        std::fs::remove_dir_all(root).unwrap();
    }
}
