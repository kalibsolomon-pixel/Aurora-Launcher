//! Final, network-free launch validation serialized with C2 mutation commit.
use crate::aurora::AuroraInstalledState;
use crate::install::state::InstalledGameManifest;
use crate::instances::lifecycle::{InstanceStatus, registry_lock, validate_instance};
use crate::instances::{InstanceId, InstanceRecord, InstanceRegistry};
use crate::paths::ManagedPaths;

/// Rust-only preparation snapshot; cosmetic state never authorizes a launch.
#[derive(PartialEq, Eq)]
pub(crate) struct LaunchSnapshot {
    record: InstanceRecord,
    game: Option<InstalledGameManifest>,
    aurora: Option<AuroraInstalledState>,
}

#[derive(Debug)]
pub(crate) enum BoundaryError {
    Busy,
    Stale,
    Invalid,
}

impl BoundaryError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Busy => "launch_instance_busy",
            Self::Stale => "launch_configuration_changed",
            Self::Invalid => "launch_instance_not_ready",
        }
    }
}

impl std::fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Busy => "This instance is being changed. Try Play again when it finishes.",
            Self::Stale => "The instance changed while Play was preparing. Try Play again.",
            Self::Invalid => {
                "The instance did not pass final launch validation. Check its readiness."
            }
        })
    }
}
impl std::error::Error for BoundaryError {}

impl LaunchSnapshot {
    pub(crate) fn capture(managed: &ManagedPaths, id: &InstanceId) -> Result<Self, BoundaryError> {
        let registry = InstanceRegistry::load(&managed.instance_registry_file())
            .map_err(|_| BoundaryError::Invalid)?;
        let record = registry.find(id).cloned().ok_or(BoundaryError::Invalid)?;
        let game = crate::install::state::load_installed_state(managed.instance_paths(id).game())
            .map_err(|_| BoundaryError::Invalid)?;
        let aurora =
            crate::aurora::load_installed_state(managed, id).map_err(|_| BoundaryError::Invalid)?;
        Ok(Self {
            record,
            game,
            aurora,
        })
    }

    pub(crate) fn record(&self) -> &InstanceRecord {
        &self.record
    }

    /// The callback must be synchronous and finish process reservation/spawn
    /// before returning. No runtime/session/network awaits occur under these
    /// locks. Ordering matches Aurora commit: content -> registry -> process.
    pub(crate) fn with_validated<T, E: From<BoundaryError>>(
        &self,
        managed: &ManagedPaths,
        spawn: impl FnOnce(&InstalledGameManifest) -> Result<T, E>,
    ) -> Result<T, E> {
        let id = self.record.id();
        crate::instance_content::with_instance_lock(id, || {
            let _registry_guard = registry_lock();
            Ok((|| {
                let current = Self::capture(managed, id)?;
                if current != *self {
                    return Err(BoundaryError::Stale.into());
                }
                let registry = InstanceRegistry::load(&managed.instance_registry_file())
                    .map_err(|_| BoundaryError::Invalid)?;
                let validation = validate_instance(managed, &registry, id)
                    .map_err(|_| BoundaryError::Invalid)?;
                if validation.status != InstanceStatus::Ready {
                    return Err(BoundaryError::Invalid.into());
                }
                spawn(current.game.as_ref().ok_or(BoundaryError::Invalid)?)
            })())
        })
        .map_err(|_| E::from(BoundaryError::Busy))?
    }
}
