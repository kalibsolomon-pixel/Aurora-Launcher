//! Non-secret launch readiness and process-state vocabulary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchInstanceStatus {
    Ready,
    Missing,
    Installing,
    /// The desired configuration no longer matches the installed content;
    /// installing the new configuration restores launchability.
    Stale,
    Damaged,
}

impl LaunchInstanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::Installing => "installing",
            Self::Stale => "stale",
            Self::Damaged => "damaged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchRuntimeStatus {
    Ready,
    Missing,
    Damaged,
    Unresolved,
    /// The instance's selected custom Java failed validation. Never
    /// replaced by managed Java while the custom selection is active.
    CustomInvalid,
}

impl LaunchRuntimeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::Damaged => "damaged",
            Self::Unresolved => "unresolved",
            Self::CustomInvalid => "customInvalid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchAccountStatus {
    Ready,
    Missing,
    ReauthenticationRequired,
    ConfigurationMissing,
}

impl LaunchAccountStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::ReauthenticationRequired => "reauthenticationRequired",
            Self::ConfigurationMissing => "configurationMissing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchProcessStatus {
    Stopped,
    Starting,
    Running,
    Exited,
    Failed,
}

impl LaunchProcessStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Failed => "failed",
        }
    }

    pub fn blocks_launch(self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchBlocker {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayReadiness {
    pub instance: LaunchInstanceStatus,
    pub runtime: LaunchRuntimeStatus,
    pub account: LaunchAccountStatus,
    pub process: LaunchProcessStatus,
    pub blockers: Vec<LaunchBlocker>,
}

impl PlayReadiness {
    pub fn evaluate(
        instance: LaunchInstanceStatus,
        runtime: LaunchRuntimeStatus,
        account: LaunchAccountStatus,
        process: LaunchProcessStatus,
    ) -> Self {
        let mut blockers = Vec::new();
        match instance {
            LaunchInstanceStatus::Ready => {}
            LaunchInstanceStatus::Missing => blockers.push(LaunchBlocker {
                code: "launch_instance_not_ready",
                message: "The selected instance does not exist.".to_owned(),
            }),
            LaunchInstanceStatus::Installing => blockers.push(LaunchBlocker {
                code: "launch_instance_not_ready",
                message: "The selected instance has not finished installing.".to_owned(),
            }),
            LaunchInstanceStatus::Stale => blockers.push(LaunchBlocker {
                code: "launch_instance_not_ready",
                message: "The selected instance's configuration changed; install the new configuration before playing.".to_owned(),
            }),
            LaunchInstanceStatus::Damaged => blockers.push(LaunchBlocker {
                code: "launch_instance_damaged",
                message: "The selected instance failed complete content validation.".to_owned(),
            }),
        }
        match runtime {
            LaunchRuntimeStatus::Ready => {}
            LaunchRuntimeStatus::Missing => blockers.push(LaunchBlocker {
                code: "launch_runtime_not_ready",
                message: "The required managed Java runtime is not installed.".to_owned(),
            }),
            LaunchRuntimeStatus::Damaged => blockers.push(LaunchBlocker {
                code: "launch_runtime_not_ready",
                message: "The required managed Java runtime is damaged.".to_owned(),
            }),
            LaunchRuntimeStatus::Unresolved => blockers.push(LaunchBlocker {
                code: "launch_runtime_not_ready",
                message: "The required managed Java runtime could not be resolved.".to_owned(),
            }),
            LaunchRuntimeStatus::CustomInvalid => blockers.push(LaunchBlocker {
                code: "launch_custom_java_invalid",
                message: "The custom Java selected for this instance is not usable. Fix it in the instance's Settings or switch back to managed Java.".to_owned(),
            }),
        }
        match account {
            LaunchAccountStatus::Ready => {}
            LaunchAccountStatus::Missing => blockers.push(LaunchBlocker {
                code: "launch_authentication_required",
                message: "Select a Minecraft account before playing.".to_owned(),
            }),
            LaunchAccountStatus::ReauthenticationRequired => blockers.push(LaunchBlocker {
                code: "launch_authentication_required",
                message: "The selected Minecraft account must sign in again.".to_owned(),
            }),
            LaunchAccountStatus::ConfigurationMissing => blockers.push(LaunchBlocker {
                code: "launch_authentication_required",
                message: "Aurora's Microsoft application registration is not configured."
                    .to_owned(),
            }),
        }
        if process.blocks_launch() {
            blockers.push(LaunchBlocker {
                code: "launch_already_running",
                message: "This instance is already starting or running.".to_owned(),
            });
        }
        Self {
            instance,
            runtime,
            account,
            process,
            blockers,
        }
    }

    pub fn ready(&self) -> bool {
        self.blockers.is_empty()
    }

    /// Preparation is a separate decision from Ready. Only a wholly absent
    /// managed runtime is automatic; damaged, custom, unresolved and content
    /// problems retain their existing explicit recovery paths.
    pub fn can_prepare(&self) -> bool {
        self.instance == LaunchInstanceStatus::Ready
            && self.runtime == LaunchRuntimeStatus::Missing
            && self.account == LaunchAccountStatus::Ready
            && !self.process.blocks_launch()
            && self.blockers.len() == 1
            && self.blockers[0].code == "launch_runtime_not_ready"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_preparation_is_only_for_missing_managed_java() {
        let decision = |runtime| {
            PlayReadiness::evaluate(
                LaunchInstanceStatus::Ready,
                runtime,
                LaunchAccountStatus::Ready,
                LaunchProcessStatus::Stopped,
            )
        };
        assert!(decision(LaunchRuntimeStatus::Missing).can_prepare());
        for status in [
            LaunchRuntimeStatus::Ready,
            LaunchRuntimeStatus::Damaged,
            LaunchRuntimeStatus::Unresolved,
            LaunchRuntimeStatus::CustomInvalid,
        ] {
            assert!(!decision(status).can_prepare());
        }
        for account in [
            LaunchAccountStatus::Missing,
            LaunchAccountStatus::ReauthenticationRequired,
            LaunchAccountStatus::ConfigurationMissing,
        ] {
            assert!(
                !PlayReadiness::evaluate(
                    LaunchInstanceStatus::Ready,
                    LaunchRuntimeStatus::Missing,
                    account,
                    LaunchProcessStatus::Stopped
                )
                .can_prepare()
            );
        }
        for instance in [
            LaunchInstanceStatus::Missing,
            LaunchInstanceStatus::Installing,
            LaunchInstanceStatus::Stale,
            LaunchInstanceStatus::Damaged,
        ] {
            assert!(
                !PlayReadiness::evaluate(
                    instance,
                    LaunchRuntimeStatus::Missing,
                    LaunchAccountStatus::Ready,
                    LaunchProcessStatus::Stopped
                )
                .can_prepare()
            );
        }
        let mut with_mod_conflict = decision(LaunchRuntimeStatus::Missing);
        with_mod_conflict.blockers.push(LaunchBlocker {
            code: "launch_mod_incompatible",
            message: "conflict".into(),
        });
        assert!(!with_mod_conflict.can_prepare());
        assert!(
            !PlayReadiness::evaluate(
                LaunchInstanceStatus::Ready,
                LaunchRuntimeStatus::Missing,
                LaunchAccountStatus::Ready,
                LaunchProcessStatus::Running
            )
            .can_prepare()
        );
    }

    #[test]
    fn every_launch_precondition_is_rust_owned_and_explicit() {
        let ready = PlayReadiness::evaluate(
            LaunchInstanceStatus::Ready,
            LaunchRuntimeStatus::Ready,
            LaunchAccountStatus::Ready,
            LaunchProcessStatus::Stopped,
        );
        assert!(ready.ready());

        for (instance, runtime, account, process, code) in [
            (
                LaunchInstanceStatus::Ready,
                LaunchRuntimeStatus::CustomInvalid,
                LaunchAccountStatus::Ready,
                LaunchProcessStatus::Stopped,
                "launch_custom_java_invalid",
            ),
            (
                LaunchInstanceStatus::Missing,
                LaunchRuntimeStatus::Ready,
                LaunchAccountStatus::Ready,
                LaunchProcessStatus::Stopped,
                "launch_instance_not_ready",
            ),
            (
                LaunchInstanceStatus::Installing,
                LaunchRuntimeStatus::Ready,
                LaunchAccountStatus::Ready,
                LaunchProcessStatus::Stopped,
                "launch_instance_not_ready",
            ),
            (
                LaunchInstanceStatus::Damaged,
                LaunchRuntimeStatus::Ready,
                LaunchAccountStatus::Ready,
                LaunchProcessStatus::Stopped,
                "launch_instance_damaged",
            ),
            (
                LaunchInstanceStatus::Ready,
                LaunchRuntimeStatus::Missing,
                LaunchAccountStatus::Ready,
                LaunchProcessStatus::Stopped,
                "launch_runtime_not_ready",
            ),
            (
                LaunchInstanceStatus::Ready,
                LaunchRuntimeStatus::Damaged,
                LaunchAccountStatus::Ready,
                LaunchProcessStatus::Stopped,
                "launch_runtime_not_ready",
            ),
            (
                LaunchInstanceStatus::Ready,
                LaunchRuntimeStatus::Ready,
                LaunchAccountStatus::Missing,
                LaunchProcessStatus::Stopped,
                "launch_authentication_required",
            ),
            (
                LaunchInstanceStatus::Ready,
                LaunchRuntimeStatus::Ready,
                LaunchAccountStatus::ReauthenticationRequired,
                LaunchProcessStatus::Stopped,
                "launch_authentication_required",
            ),
            (
                LaunchInstanceStatus::Ready,
                LaunchRuntimeStatus::Ready,
                LaunchAccountStatus::ConfigurationMissing,
                LaunchProcessStatus::Stopped,
                "launch_authentication_required",
            ),
            (
                LaunchInstanceStatus::Ready,
                LaunchRuntimeStatus::Ready,
                LaunchAccountStatus::Ready,
                LaunchProcessStatus::Running,
                "launch_already_running",
            ),
        ] {
            let result = PlayReadiness::evaluate(instance, runtime, account, process);
            assert!(!result.ready());
            assert!(result.blockers.iter().any(|blocker| blocker.code == code));
        }
    }
}
