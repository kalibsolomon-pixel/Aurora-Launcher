//! Aurora's production update domain (Phase L).
//!
//! Two deliberately separate update lives here:
//!
//! * the **Aurora Client** update — a versioned, digested mod artifact the
//!   launcher already owns through the verified-store/staged-activation
//!   pipeline ([`client`]);
//! * the **launcher self-update** — the official Tauri 2 updater with its
//!   signature-verified install lifecycle, wrapped natively ([`launcher`]).
//!
//! They share presentation concepts and this
//! module's in-process status center, and nothing else: artifacts, trust
//! roots, activation, and rollback are distinct by design.
//!
//! Updates are release-driven, never commit-driven: only intentionally
//! published release metadata (the manually gated public Aurora Client
//! manifest, the owner-published launcher update manifests) may become
//! update candidates. A pushed commit or a CI build is never an update.
//! Checking is bounded and explicit — one bounded check after startup, one
//! per explicit user request — with no polling, watchers, or background
//! refresh anywhere in this module.

pub mod client;
pub mod launcher;

use std::sync::Mutex;
use std::sync::OnceLock;

use serde::Serialize;

/// Parses one Aurora version string with semantic-version precedence.
///
/// Malformed versions never become update candidates: discovery skips them
/// (fail closed) rather than comparing lexically or guessing.
pub fn parse_release_version(version: &str) -> Option<semver::Version> {
    semver::Version::parse(version).ok()
}

/// One domain of the update overview shown by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum UpdateAvailability {
    /// No check has run in this session yet.
    NotChecked,
    /// The check succeeded and nothing eligible is newer.
    UpToDate,
    /// An intentionally published, eligible, compatible newer release exists.
    UpdateAvailable {
        current: String,
        candidate: String,
        notes: Option<String>,
    },
    /// The checked subject has no update domain (for example an instance
    /// without the Aurora Client). Honest neutrality, not health.
    NotApplicable { reason: String },
    /// The check itself failed (offline, endpoint unavailable, malformed or
    /// untrusted metadata). Never a launcher- or instance-level failure.
    Unavailable { reason: String },
}

impl UpdateAvailability {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable {
            reason: reason.into(),
        }
    }
}

/// Session-scoped state for update presentation: the last availability per
/// domain, in-flight de-duplication, and dismissed notices.
///
/// All fields are process-local memory. Nothing here is authority, persisted
/// state, or a download; installed software changes only through the explicit
/// client update transaction or the launcher updater's install lifecycle.
#[derive(Debug, Default)]
pub struct UpdateCenter {
    pub launcher: Option<UpdateAvailability>,
    pub client: Option<UpdateAvailability>,
    pub client_instance_id: Option<String>,
    pub launcher_phase: Option<LauncherPhase>,
    pub client_phase: Option<ClientPhase>,
    launcher_check_running: bool,
    client_check_running: bool,
    launcher_transaction_running: bool,
    client_transaction_running: bool,
    startup_check_done: bool,
}

impl UpdateCenter {
    /// Records that the one-per-process startup check has run.
    pub fn startup_check_done(&self) -> bool {
        self.startup_check_done
    }

    pub fn mark_startup_check_done(&mut self) {
        self.startup_check_done = true;
    }
}

/// The launcher self-update lifecycle phases surfaced to the UI. They reflect
/// real updater state; percentages are never fabricated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LauncherPhase {
    Checking,
    Downloading,
    ReadyToInstall,
    Installing,
}

/// The Aurora Client update transaction phases surfaced to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientPhase {
    Acquiring,
    Staging,
    Activating,
    Validating,
    Committing,
}

/// The process-global update status center. One instance per process; UI
/// state is derived from it, never the other way around.
pub fn center() -> &'static Mutex<UpdateCenter> {
    static CENTER: OnceLock<Mutex<UpdateCenter>> = OnceLock::new();
    CENTER.get_or_init(|| Mutex::new(UpdateCenter::default()))
}

/// The outcome of trying to begin one domain's bounded check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStart {
    /// This caller owns the check; it must finish with [`finish_check`].
    Started,
    /// A check of this domain is already in flight; do not duplicate it.
    Duplicate,
}

/// Begins one domain's transaction exclusivity. The caller owns the work
/// and must release it through [`finish_transaction`], including on error
/// paths.
pub fn begin_transaction(domain: CheckDomain) -> CheckStart {
    let mut center = center()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let running = match domain {
        CheckDomain::Launcher => &mut center.launcher_transaction_running,
        CheckDomain::Client => &mut center.client_transaction_running,
    };
    if *running {
        return CheckStart::Duplicate;
    }
    *running = true;
    CheckStart::Started
}

/// Releases one domain's transaction exclusivity.
pub fn finish_transaction(domain: CheckDomain) {
    let mut center = center()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match domain {
        CheckDomain::Launcher => center.launcher_transaction_running = false,
        CheckDomain::Client => center.client_transaction_running = false,
    }
}

/// Begins one domain's check de-duplication. The caller owns the network
/// work and records the outcome through [`finish_check`].
pub fn begin_check(domain: CheckDomain) -> CheckStart {
    let mut center = center()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let running = match domain {
        CheckDomain::Launcher => &mut center.launcher_check_running,
        CheckDomain::Client => &mut center.client_check_running,
    };
    if *running {
        return CheckStart::Duplicate;
    }
    *running = true;
    CheckStart::Started
}

/// Records one domain's check outcome and releases its de-duplication.
pub fn finish_check(domain: CheckDomain, availability: UpdateAvailability) {
    let mut center = center()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match domain {
        CheckDomain::Launcher => {
            center.launcher_check_running = false;
            center.launcher = Some(availability);
        }
        CheckDomain::Client => {
            center.client_check_running = false;
            center.client = Some(availability);
        }
    }
}

/// Which update domain a de-duplication guard protects. The two domains
/// never share activation assumptions; they also never block each other's
/// checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckDomain {
    Launcher,
    Client,
}

/// Resets the process-global center. Test-only: production code never clears
/// session update state.
#[cfg(test)]
pub(crate) fn reset_center_for_tests() {
    let mut center = center()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *center = UpdateCenter::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_versions_parse_with_semver_precedence() {
        let older = parse_release_version("2.1.5").expect("plain versions parse");
        let newer = parse_release_version("2.2.0").expect("plain versions parse");
        let prerelease = parse_release_version("2.2.0-beta.1").expect("prereleases parse");
        assert!(newer > older);
        // Semver precedence: a prerelease of 2.2.0 sorts before its release.
        assert!(prerelease < newer);
        assert!(prerelease > older);
    }

    #[test]
    fn malformed_versions_never_parse() {
        for malformed in ["", "latest", "2.x", "2.1", "v2.1.5!", "2.1.5 beta"] {
            assert!(
                parse_release_version(malformed).is_none(),
                "'{malformed}' must never become an update candidate"
            );
        }
    }

    #[test]
    fn check_and_transaction_guards_deduplicate() {
        reset_center_for_tests();
        // One caller owns a domain's check; a second simultaneous check of
        // the same domain is refused instead of duplicated.
        assert_eq!(begin_check(CheckDomain::Launcher), CheckStart::Started);
        assert_eq!(begin_check(CheckDomain::Launcher), CheckStart::Duplicate);
        finish_check(CheckDomain::Launcher, UpdateAvailability::UpToDate);
        {
            // The outcome is recorded and the de-duplication released.
            let state = center()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            assert_eq!(state.launcher, Some(UpdateAvailability::UpToDate));
            assert!(!state.launcher_check_running);
        }
        // A released check may begin again.
        assert_eq!(begin_check(CheckDomain::Launcher), CheckStart::Started);
        // Transaction exclusivity refuses an overlapping transaction of
        // the same domain and releases cleanly.
        assert_eq!(begin_transaction(CheckDomain::Client), CheckStart::Started);
        assert_eq!(
            begin_transaction(CheckDomain::Client),
            CheckStart::Duplicate
        );
        finish_transaction(CheckDomain::Client);
        assert_eq!(begin_transaction(CheckDomain::Client), CheckStart::Started);
        reset_center_for_tests();
    }
}
