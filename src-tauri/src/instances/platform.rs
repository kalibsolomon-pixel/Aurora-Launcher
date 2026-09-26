//! Rust-owned installed configuration and capability authority.
//! Vanilla and future loaders are representable; only Fabric has an executor.
use crate::distribution::ReleaseChannel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum PlatformPin {
    Vanilla {},
    Fabric { version: String },
    Forge { version: String },
    NeoForge { version: String },
    Quilt { version: String },
}
impl PlatformPin {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Vanilla {} => "vanilla",
            Self::Fabric { .. } => "fabric",
            Self::Forge { .. } => "forge",
            Self::NeoForge { .. } => "neoForge",
            Self::Quilt { .. } => "quilt",
        }
    }
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Vanilla {} => None,
            Self::Fabric { version }
            | Self::Forge { version }
            | Self::NeoForge { version }
            | Self::Quilt { version } => Some(version),
        }
    }
    pub fn require_fabric(&self) -> Result<&str, String> {
        match self {
            Self::Fabric { version } => Ok(version),
            _ => Err(format!(
                "{} installation and launch are not implemented in this build",
                self.kind()
            )),
        }
    }
    pub fn provider_loader(&self) -> Result<&'static str, String> {
        self.require_fabric()?;
        Ok("fabric")
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuroraPin {
    pub channel: ReleaseChannel,
    pub version: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledConfiguration {
    pub minecraft_version: String,
    pub platform: PlatformPin,
    pub aurora: Option<AuroraPin>,
}
impl InstalledConfiguration {
    pub fn validate(&self) -> Result<(), String> {
        crate::minecraft::metadata::MinecraftVersionId::new(&self.minecraft_version)
            .map_err(|e| e.to_string())?;
        if let Some(version) = self.platform.version() {
            if version.len() > 128
                || !version.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
                })
            {
                return Err("invalid platform version".into());
            }
            if version.is_empty()
                || version.trim() != version
                || version.chars().any(char::is_control)
            {
                return Err("invalid platform version".into());
            }
        }
        if let Some(aurora) = &self.aurora {
            self.platform.require_fabric()?;
            if aurora.version.is_empty()
                || aurora.version.trim() != aurora.version
                || aurora.version.chars().any(char::is_control)
            {
                return Err("invalid Aurora version".into());
            }
        }
        Ok(())
    }
    pub fn from_release(release: &super::PinnedRelease) -> Self {
        Self {
            minecraft_version: release.minecraft_version().into(),
            platform: PlatformPin::Fabric {
                version: release.fabric_loader_version().into(),
            },
            aurora: Some(AuroraPin {
                channel: release.channel(),
                version: release.aurora_version().into(),
            }),
        }
    }
    /// Compatibility adapter for the proven Aurora artifact layer only.
    pub fn aurora_release(&self) -> Result<super::PinnedRelease, String> {
        let aurora = self
            .aurora
            .as_ref()
            .ok_or("Aurora is not configured for this instance")?;
        super::PinnedRelease::new(
            aurora.channel,
            &aurora.version,
            &self.minecraft_version,
            self.platform.require_fabric()?,
        )
        .map_err(|e| e.to_string())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapability {
    pub kind: &'static str,
    pub can_create: bool,
    pub can_install: bool,
    pub can_validate: bool,
    pub can_launch: bool,
    pub aurora_supported: bool,
}
/// UI choices are this list, never the serialization enum's vocabulary.
pub fn capabilities() -> Vec<PlatformCapability> {
    vec![PlatformCapability {
        kind: "fabric",
        can_create: true,
        can_install: true,
        can_validate: true,
        can_launch: true,
        aurora_supported: true,
    }]
}

/// Requirements are properties of a resolved optional capability, not of Fabric itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequiredContent {
    pub aurora: bool,
    pub fabric_api: bool,
}
pub fn required_content(
    installed: &InstalledConfiguration,
    release: Option<&crate::distribution::AuroraRelease>,
) -> Result<RequiredContent, String> {
    let Some(pin) = &installed.aurora else {
        return Ok(RequiredContent {
            aurora: false,
            fabric_api: false,
        });
    };
    let release = release.ok_or("configured Aurora release metadata is missing")?;
    if pin.version != release.aurora_version()
        || pin.channel != release.channel()
        || installed.minecraft_version != release.minecraft_version()
        || installed.platform.require_fabric()? != release.fabric_loader_version()
    {
        return Err("Aurora is incompatible with the configured Minecraft/platform pin".into());
    }
    Ok(RequiredContent {
        aurora: true,
        fabric_api: release.fabric_api().is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn production() -> InstalledConfiguration {
        InstalledConfiguration {
            minecraft_version: "1.21.11".into(),
            platform: PlatformPin::Fabric {
                version: "0.19.5".into(),
            },
            aurora: Some(AuroraPin {
                channel: ReleaseChannel::Stable,
                version: "2.1.2".into(),
            }),
        }
    }
    #[test]
    fn vanilla_has_no_loader_metadata() {
        let value = serde_json::to_value(PlatformPin::Vanilla {}).unwrap();
        assert_eq!(value, serde_json::json!({"kind":"vanilla"}));
        assert!(
            serde_json::from_value::<PlatformPin>(
                serde_json::json!({"kind":"vanilla","version":"fake"})
            )
            .is_err()
        );
    }
    #[test]
    fn fabric_keeps_exact_loader_version() {
        let pin = production();
        assert_eq!(pin.platform.require_fabric().unwrap(), "0.19.5");
        assert_eq!(
            serde_json::from_str::<InstalledConfiguration>(&serde_json::to_string(&pin).unwrap())
                .unwrap(),
            pin
        );
    }
    #[test]
    fn unimplemented_platforms_are_not_executable_or_provider_compatible() {
        for platform in [
            PlatformPin::Vanilla {},
            PlatformPin::Forge {
                version: "1".into(),
            },
            PlatformPin::NeoForge {
                version: "1".into(),
            },
            PlatformPin::Quilt {
                version: "1".into(),
            },
        ] {
            assert!(platform.require_fabric().is_err());
            assert!(platform.provider_loader().is_err());
        }
        assert_eq!(
            capabilities().iter().map(|c| c.kind).collect::<Vec<_>>(),
            vec!["fabric"]
        );
        assert!(
            capabilities()
                .iter()
                .all(|c| c.can_install && c.can_validate && c.can_launch)
        );
    }
    #[test]
    fn aurora_cannot_change_platform_to_satisfy_compatibility() {
        let mut pin = production();
        pin.platform = PlatformPin::Vanilla {};
        assert!(pin.validate().is_err());
        assert!(pin.aurora_release().is_err());
        assert_eq!(pin.platform, PlatformPin::Vanilla {});
    }
    #[test]
    fn production_aurora_requires_and_protects_both_artifacts() {
        let manifest = crate::distribution::production_manifest().unwrap();
        let release = manifest
            .resolve_exact("2.1.2", Some(ReleaseChannel::Stable))
            .unwrap();
        let pin = production();
        assert_eq!(
            required_content(&pin, Some(release)).unwrap(),
            RequiredContent {
                aurora: true,
                fabric_api: true
            }
        );
        let mut wrong = pin.clone();
        wrong.minecraft_version = "1.20.1".into();
        assert!(required_content(&wrong, Some(release)).is_err());
    }
    #[test]
    fn absence_of_aurora_derives_no_aurora_or_fabric_api() {
        let mut pin = production();
        pin.aurora = None;
        assert_eq!(
            required_content(&pin, None).unwrap(),
            RequiredContent {
                aurora: false,
                fabric_api: false
            }
        );
        pin.platform = PlatformPin::Vanilla {};
        assert!(pin.validate().is_ok());
        assert_eq!(
            required_content(&pin, None).unwrap(),
            RequiredContent {
                aurora: false,
                fabric_api: false
            }
        );
    }
}
