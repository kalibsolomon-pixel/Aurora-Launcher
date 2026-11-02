//! Native launcher update facade. All acquisition, signature and install
//! behavior lives in the existing native updater.
use crate::host::Host;
use crate::updates::{LauncherPhase, UpdateAvailability};
use serde::Serialize;
pub const UPDATER_PUBKEY: Option<&str> = option_env!("AURORA_UPDATER_PUBKEY");
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdateStatus {
    pub installed_version: String,
    pub availability: UpdateAvailability,
    pub phase: Option<LauncherPhase>,
}
/// A failed launcher update operation. Codes stay user-safe; messages never
/// include signing secrets or endpoint bodies.
#[derive(Debug)]
pub struct LauncherUpdateError {
    pub code: &'static str,
    pub message: String,
}

impl LauncherUpdateError {
    pub(super) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(super) fn gone(message: &str) -> Self {
        Self::new("update_stale", message.to_owned())
    }
}

pub async fn check_launcher_update(app: &Host) -> UpdateAvailability {
    super::native::check(app).await
}
pub async fn run_check(app: &Host) -> Option<UpdateAvailability> {
    if crate::updates::begin_check(crate::updates::CheckDomain::Launcher)
        == crate::updates::CheckStart::Duplicate
    {
        return None;
    }
    let availability = check_launcher_update(app).await;
    crate::updates::finish_check(crate::updates::CheckDomain::Launcher, availability.clone());
    Some(availability)
}
pub async fn download(
    app: &Host,
    presented_version: &str,
    progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<LauncherUpdateStatus, LauncherUpdateError> {
    super::native::download(app, presented_version, progress).await
}
pub async fn install(
    app: &Host,
    presented_version: &str,
) -> Result<LauncherUpdateStatus, LauncherUpdateError> {
    super::native::install(app, presented_version).await
}
pub fn pending_version() -> Option<String> {
    super::native::pending_version()
}
pub fn set_phase(phase: Option<LauncherPhase>) {
    crate::updates::center()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .launcher_phase = phase;
}
