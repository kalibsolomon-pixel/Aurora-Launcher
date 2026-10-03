//! Versioned launcher configuration and its persistence.
//!
//! The configuration is a small, human-inspectable JSON document stored under
//! the managed-data root. Malformed or unsupported files fail deliberately and
//! are never silently replaced; writes go through a temporary file and a
//! rename so a partially written configuration can never be observed.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::appearance::AppearancePreferences;
use crate::instances::InstanceId;

/// Schema 7 remains current. Its former update-channel preference is
/// validated and discarded on load; new documents omit it. Schemas 1–6
/// retain their explicit migrations for the preferences that still exist.
/// Schema 6 added bounded Aurora motion speed.
/// Schema 5 added the independent bundled background selection.
/// Schema 4 added independently opt-in world/server/address preferences.
/// Schemas 1, 2 and 3 migrate explicitly; malformed documents remain untouched.
/// The current launcher-configuration schema version.
///
/// Version 2 added the launcher-wide appearance preferences. Version 1 files
/// (selected instance only) migrate deterministically on load with the
/// default appearance; anything else fails deliberately.
pub const CONFIG_SCHEMA_VERSION: u32 = 7;
/// The schema version before appearance preferences existed.
const LEGACY_CONFIG_SCHEMA_VERSION: u32 = 1;

/// Launcher preferences that are genuinely required now.
///
/// Secrets never belong here. Settings for future features are added by later
/// phases together with an explicit schema-version decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    schema_version: u32,
    selected_instance_id: Option<InstanceId>,
    appearance: AppearancePreferences,
    home_widgets: crate::home_widgets::HomeLayout,
    discord: crate::discord::DiscordPreferences,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            selected_instance_id: None,
            appearance: AppearancePreferences::new(),
            home_widgets: Default::default(),
            discord: Default::default(),
        }
    }
}

impl LauncherConfig {
    /// The configuration schema version this launcher writes and enforces.
    pub const SCHEMA_VERSION: u32 = CONFIG_SCHEMA_VERSION;

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn selected_instance_id(&self) -> Option<&InstanceId> {
        self.selected_instance_id.as_ref()
    }

    /// Sets (or clears) the selected instance. The value is validated as an
    /// identifier shape by the type system; referential integrity against
    /// the registry is enforced by the lifecycle operations that call this.
    pub fn set_selected_instance_id(&mut self, id: Option<InstanceId>) {
        self.selected_instance_id = id;
    }

    pub fn appearance(&self) -> &AppearancePreferences {
        &self.appearance
    }

    pub fn set_appearance(&mut self, appearance: AppearancePreferences) {
        self.appearance = appearance;
    }

    pub fn home_widgets(&self) -> &crate::home_widgets::HomeLayout {
        &self.home_widgets
    }
    pub fn set_home_widgets(&mut self, layout: crate::home_widgets::HomeLayout) {
        self.home_widgets = layout;
    }
    pub fn discord(&self) -> &crate::discord::DiscordPreferences {
        &self.discord
    }
    pub fn set_discord(&mut self, preferences: crate::discord::DiscordPreferences) {
        self.discord = preferences;
    }
    /// Parses and validates a configuration from JSON text.
    ///
    /// Schema 1 (the pre-appearance shape) migrates deterministically: the
    /// selection survives and the appearance defaults. Within a known
    /// schema, unknown theme ids and unusable accent colors normalize to the
    /// default look — appearance is cosmetic launcher-wide state and must
    /// never block startup.
    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let mut document: serde_json::Value = serde_json::from_str(json)
            .map_err(|error| ConfigError::Malformed(error.to_string()))?;

        let schema_version = document
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                ConfigError::Malformed("the schemaVersion field is missing".to_owned())
            })?;

        // Explicit Phase F migration: previous appearance documents have no
        // background dimension. Preserve every existing choice and use Simple.
        // Schema 5 requires the field; malformed/unknown values fail closed.
        if matches!(schema_version, 2..=4) {
            let appearance = document
                .get_mut("appearance")
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| {
                    ConfigError::Malformed("the appearance preferences are missing".into())
                })?;
            appearance.insert("background".into(), "simple".into());
        }

        if matches!(schema_version, 2..=5) {
            let appearance = document
                .get_mut("appearance")
                .and_then(|d| d.as_object_mut())
                .ok_or_else(|| {
                    ConfigError::Malformed("the appearance preferences are missing".into())
                })?;
            appearance.insert("auroraMotionSpeed".into(), 50.into());
        }

        // Both the original and corrected schema-7 shapes are known. Validate
        // legacy state before discarding it; malformed preferences are never
        // guessed into a usable configuration. Loading remains read-only.
        if schema_version == 7 {
            if let Some(legacy) = document.get("updates") {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct LegacyUpdates {
                    #[allow(dead_code)]
                    channel: crate::distribution::ReleaseChannel,
                }
                serde_json::from_value::<LegacyUpdates>(legacy.clone())
                    .map_err(|error| ConfigError::Malformed(error.to_string()))?;
                document.as_object_mut().unwrap().remove("updates");
            }
        }

        let config = match schema_version {
            version if version == u64::from(CONFIG_SCHEMA_VERSION) => {
                serde_json::from_value::<Self>(document)
                    .map_err(|error| ConfigError::Malformed(error.to_string()))?
            }
            4 | 5 | 6 => {
                document["schemaVersion"] = CONFIG_SCHEMA_VERSION.into();
                serde_json::from_value::<Self>(document)
                    .map_err(|error| ConfigError::Malformed(error.to_string()))?
            }
            3 => {
                let preferences = document
                    .get_mut("discord")
                    .and_then(|d| d.as_object_mut())
                    .ok_or_else(|| {
                        ConfigError::Malformed("the Discord preferences are missing".into())
                    })?;
                // Schema 3 has exactly six required booleans. Never normalize damage.
                let old_fields = [
                    "enabled",
                    "instanceName",
                    "minecraftVersion",
                    "platform",
                    "auroraActive",
                    "elapsedTime",
                ];
                if preferences.len() != old_fields.len()
                    || old_fields
                        .iter()
                        .any(|key| !preferences.get(*key).is_some_and(|v| v.is_boolean()))
                {
                    return Err(ConfigError::Malformed(
                        "the schema-3 Discord preferences are malformed".into(),
                    ));
                }
                for key in ["world", "server", "serverAddress"] {
                    preferences.insert(key.into(), false.into());
                }
                document["schemaVersion"] = CONFIG_SCHEMA_VERSION.into();
                serde_json::from_value::<Self>(document)
                    .map_err(|error| ConfigError::Malformed(error.to_string()))?
            }
            2 => {
                let legacy: AppearanceLauncherConfig = serde_json::from_value(document)
                    .map_err(|error| ConfigError::Malformed(error.to_string()))?;
                Self {
                    schema_version: CONFIG_SCHEMA_VERSION,
                    selected_instance_id: legacy.selected_instance_id,
                    appearance: legacy.appearance,
                    home_widgets: Default::default(),
                    discord: Default::default(),
                }
            }
            version if version == u64::from(LEGACY_CONFIG_SCHEMA_VERSION) => {
                let legacy: LegacyLauncherConfig = serde_json::from_value(document)
                    .map_err(|error| ConfigError::Malformed(error.to_string()))?;
                Self {
                    schema_version: CONFIG_SCHEMA_VERSION,
                    selected_instance_id: legacy.selected_instance_id,
                    appearance: AppearancePreferences::new(),
                    home_widgets: Default::default(),
                    discord: Default::default(),
                }
            }
            found => {
                return Err(ConfigError::UnsupportedSchema {
                    found: u32::try_from(found).unwrap_or(u32::MAX),
                    supported: CONFIG_SCHEMA_VERSION,
                });
            }
        };

        if config.schema_version != Self::SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchema {
                found: config.schema_version,
                supported: Self::SCHEMA_VERSION,
            });
        }

        if config.appearance.aurora_motion_speed > 100 {
            return Err(ConfigError::Malformed(
                "Aurora motion speed must be between 0 and 100".into(),
            ));
        }
        config
            .home_widgets
            .validate()
            .map_err(|detail| ConfigError::Malformed(detail.into()))?;
        Ok(Self {
            appearance: config.appearance.normalized(),
            ..config
        })
    }

    /// Serializes the configuration as pretty, human-inspectable JSON.
    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self)
            .expect("launcher configuration serialization cannot fail");
        json.push('\n');
        json
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppearanceLauncherConfig {
    selected_instance_id: Option<InstanceId>,
    appearance: AppearancePreferences,
}

/// The schema-1 configuration shape (before appearance preferences).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyLauncherConfig {
    #[allow(dead_code)]
    schema_version: u32,
    selected_instance_id: Option<InstanceId>,
}

/// The outcome of [`load_or_initialize`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLoad {
    Existing(LauncherConfig),
    Initialized(LauncherConfig),
}

impl ConfigLoad {
    pub fn into_config(self) -> LauncherConfig {
        match self {
            Self::Existing(config) | Self::Initialized(config) => config,
        }
    }

    pub fn config(&self) -> &LauncherConfig {
        match self {
            Self::Existing(config) | Self::Initialized(config) => config,
        }
    }
}

/// Loads the configuration. Returns `Ok(None)` when no file exists yet.
pub fn load(path: &Path) -> Result<Option<LauncherConfig>, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
            return Err(ConfigError::Malformed(
                "the launcher configuration file is not valid UTF-8 and may be corrupted"
                    .to_owned(),
            ));
        }
        Err(error) => return Err(ConfigError::Read(error)),
    };

    LauncherConfig::from_json(&text).map(Some)
}

/// Loads the configuration, writing safe defaults when no file exists yet.
///
/// A malformed or unsupported existing file is returned as an error and left
/// untouched, so a damaged configuration is never silently replaced.
pub fn load_or_initialize(path: &Path) -> Result<ConfigLoad, ConfigError> {
    match load(path)? {
        Some(config) => Ok(ConfigLoad::Existing(config)),
        None => {
            let config = LauncherConfig::default();
            save(path, &config)?;
            Ok(ConfigLoad::Initialized(config))
        }
    }
}

/// Persists the configuration atomically: write to a sibling temporary file,
/// then rename over the target.
pub fn save(path: &Path, config: &LauncherConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(ConfigError::Write)?;
    }

    let temporary_path = temporary_sibling(path);
    std::fs::write(&temporary_path, config.to_json()).map_err(|error| {
        let _ = std::fs::remove_file(&temporary_path);
        ConfigError::Write(error)
    })?;

    match std::fs::rename(&temporary_path, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&temporary_path);
            Err(ConfigError::Write(error))
        }
    }
}

fn temporary_sibling(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

#[derive(Debug)]
pub enum ConfigError {
    Read(std::io::Error),
    Write(std::io::Error),
    Malformed(String),
    UnsupportedSchema { found: u32, supported: u32 },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(
                formatter,
                "the launcher configuration could not be read: {error}"
            ),
            Self::Write(error) => write!(
                formatter,
                "the launcher configuration could not be written: {error}"
            ),
            Self::Malformed(detail) => write!(
                formatter,
                "the launcher configuration is malformed and must be corrected manually: {detail}"
            ),
            Self::UnsupportedSchema { found, supported } => write!(
                formatter,
                "the launcher configuration uses schema version {found}, which is not supported; this launcher supports version {supported}"
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) | Self::Write(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_configuration_has_no_update_preference() {
        let document = serde_json::to_value(LauncherConfig::default()).unwrap();
        assert!(document.get("updates").is_none());
        assert_eq!(document["schemaVersion"], 7);
    }

    #[test]
    fn every_legacy_channel_becomes_identical_production_behavior_without_state_loss() {
        let mut expected = LauncherConfig::default();
        expected.set_selected_instance_id(Some(InstanceId::new("selected").unwrap()));
        expected.appearance.theme = "oled".into();
        expected.appearance.background = crate::appearance::BackgroundId::Borealis;
        expected.appearance.aurora_motion_speed = 37;
        expected.discord.enabled = true;
        expected.discord.server = true;
        expected.home_widgets.widgets.swap(0, 1);
        expected.home_widgets.widgets[0].enabled = false;
        for channel in ["stable", "beta", "nightly"] {
            let directory = test_directory("aurora-config-single-stream");
            let path = directory.join("config.json");
            let mut legacy = serde_json::to_value(&expected).unwrap();
            legacy["updates"] = serde_json::json!({"channel": channel});
            let original = legacy.to_string();
            std::fs::write(&path, &original).unwrap();
            let migrated = load(&path).unwrap().unwrap();
            assert_eq!(migrated, expected, "{channel} must have no residual effect");
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                original,
                "load is read-only"
            );
            save(&path, &migrated).unwrap();
            let saved: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert!(saved.get("updates").is_none());
            for key in ["selectedInstanceId", "appearance", "homeWidgets", "discord"] {
                assert_eq!(saved[key], legacy[key], "preserve {key} for {channel}");
            }
            assert_eq!(load(&path).unwrap().unwrap(), expected);
        }
    }

    #[test]
    fn malformed_legacy_update_preferences_are_never_overwritten() {
        for bad in [
            serde_json::json!({"channel":"future"}),
            serde_json::json!({}),
            serde_json::json!({"channel":"beta","url":"http://other"}),
            serde_json::Value::Null,
        ] {
            let directory = test_directory("aurora-config-malformed-legacy-updates");
            let path = directory.join("config.json");
            let mut document = serde_json::to_value(LauncherConfig::default()).unwrap();
            document["updates"] = bad;
            let original = document.to_string();
            std::fs::write(&path, &original).unwrap();
            assert!(matches!(
                load_or_initialize(&path),
                Err(ConfigError::Malformed(_))
            ));
            assert_eq!(std::fs::read_to_string(path).unwrap(), original);
        }
    }

    #[test]
    fn schema_two_migration_preserves_unrelated_preferences_and_is_idempotent() {
        let json = r##"{"schemaVersion":2,"selectedInstanceId":"aurora-default","appearance":{"theme":"oled","accent":{"type":"custom","hex":"#7300d1"}}}"##;
        let config = LauncherConfig::from_json(json).unwrap();
        assert_eq!(
            config.selected_instance_id().unwrap().as_str(),
            "aurora-default"
        );
        assert_eq!(config.appearance().theme, "oled");
        assert_eq!(
            config.home_widgets(),
            &crate::home_widgets::HomeLayout::default()
        );
        assert!(!config.discord().enabled);
        assert_eq!(
            LauncherConfig::from_json(&config.to_json()).unwrap(),
            config
        );
    }

    #[test]
    fn layout_and_privacy_changes_preserve_selection_appearance_and_unknown_ids() {
        let mut config = LauncherConfig::default();
        config.set_selected_instance_id(Some(InstanceId::new("selected").unwrap()));
        let appearance = config.appearance().clone();
        let mut layout = config.home_widgets().clone();
        layout.widgets.swap(0, 1);
        layout.widgets[0].size = crate::home_widgets::WidgetSize::Large;
        layout.widgets[1].enabled = false;
        layout.widgets.push(crate::home_widgets::WidgetPlacement {
            id: "future-widget".into(),
            enabled: true,
            size: crate::home_widgets::WidgetSize::Small,
        });
        config.set_home_widgets(layout.clone());
        config.set_discord(crate::discord::DiscordPreferences {
            enabled: true,
            instance_name: true,
            ..Default::default()
        });
        let loaded = LauncherConfig::from_json(&config.to_json()).unwrap();
        assert_eq!(loaded.home_widgets(), &layout);
        assert_eq!(loaded.appearance(), &appearance);
        assert_eq!(loaded.selected_instance_id().unwrap().as_str(), "selected");
        assert!(loaded.discord().instance_name);
        assert!(!loaded.discord().elapsed_time);
    }

    #[test]
    fn malformed_widget_document_is_never_reset_or_overwritten() {
        let directory = test_directory("aurora-config-malformed-widget");
        let path = directory.join("config.json");
        let mut document = serde_json::to_value(LauncherConfig::default()).unwrap();
        document["homeWidgets"]["widgets"][2]["size"] = "large".into();
        let damaged = document.to_string();
        std::fs::write(&path, &damaged).unwrap();
        assert!(matches!(
            load_or_initialize(&path),
            Err(ConfigError::Malformed(_))
        ));
        assert_eq!(std::fs::read_to_string(path).unwrap(), damaged);
    }

    fn test_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir()
            .join(name)
            .join(std::process::id().to_string())
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn phase_f_migration_preserves_existing_choices_and_requires_current_background() {
        let mut original = LauncherConfig::default();
        original.set_selected_instance_id(Some(InstanceId::new("selected").unwrap()));
        original.appearance.theme = "oled".into();
        original.appearance.accent =
            crate::appearance::AccentSelection::Preset { id: "cyan".into() };
        original.discord.server = true;
        let mut legacy = serde_json::to_value(&original).unwrap();
        legacy["schemaVersion"] = 4.into();
        legacy["appearance"]
            .as_object_mut()
            .unwrap()
            .remove("background");
        let migrated = LauncherConfig::from_json(&legacy.to_string()).unwrap();
        assert_eq!(migrated, original);
        assert_eq!(
            LauncherConfig::from_json(&migrated.to_json()).unwrap(),
            migrated
        );
        let mut current = serde_json::to_value(&migrated).unwrap();
        for bad in [
            serde_json::Value::Null,
            serde_json::json!(17),
            serde_json::json!("file:///other"),
        ] {
            current["appearance"]["background"] = bad;
            assert!(matches!(
                LauncherConfig::from_json(&current.to_string()),
                Err(ConfigError::Malformed(_))
            ));
        }
        current["appearance"]
            .as_object_mut()
            .unwrap()
            .remove("background");
        assert!(matches!(
            LauncherConfig::from_json(&current.to_string()),
            Err(ConfigError::Malformed(_))
        ));
        original.appearance.background = crate::appearance::BackgroundId::Borealis;
        assert_eq!(
            LauncherConfig::from_json(&original.to_json()).unwrap(),
            original
        );
    }

    #[test]
    fn schema_five_migrates_speed_only_and_current_speed_is_strict() {
        let mut original = LauncherConfig::default();
        original.appearance.background = crate::appearance::BackgroundId::Borealis;
        original.appearance.theme = "oled".into();
        original.discord.enabled = true;
        let mut legacy = serde_json::to_value(&original).unwrap();
        legacy["schemaVersion"] = 5.into();
        legacy["appearance"]
            .as_object_mut()
            .unwrap()
            .remove("auroraMotionSpeed");
        let migrated = LauncherConfig::from_json(&legacy.to_string()).unwrap();
        assert_eq!(migrated, original);
        for speed in [0, 37, 50, 100] {
            original.appearance.aurora_motion_speed = speed;
            assert_eq!(
                LauncherConfig::from_json(&original.to_json()).unwrap(),
                original
            );
        }
        let current = serde_json::to_value(&original).unwrap();
        for invalid in [
            serde_json::json!(-1),
            serde_json::json!(101),
            serde_json::json!(1.5),
            serde_json::json!("fast"),
            serde_json::Value::Null,
        ] {
            let mut damaged = current.clone();
            damaged["appearance"]["auroraMotionSpeed"] = invalid;
            assert!(matches!(
                LauncherConfig::from_json(&damaged.to_string()),
                Err(ConfigError::Malformed(_))
            ));
        }
        let mut missing = current.clone();
        missing["appearance"]
            .as_object_mut()
            .unwrap()
            .remove("auroraMotionSpeed");
        assert!(LauncherConfig::from_json(&missing.to_string()).is_err());
        assert_eq!(crate::appearance::clamp_motion_speed(-1), 0);
        assert_eq!(crate::appearance::clamp_motion_speed(37), 37);
        assert_eq!(crate::appearance::clamp_motion_speed(101), 100);
    }

    #[test]
    fn default_uses_the_current_schema_version_and_no_selection() {
        let config = LauncherConfig::default();

        assert_eq!(config.schema_version(), CONFIG_SCHEMA_VERSION);
        assert_eq!(config.selected_instance_id(), None);
        assert_eq!(config.appearance(), &AppearancePreferences::new());
    }

    #[test]
    fn json_round_trip_preserves_the_selection_and_appearance() {
        let config = LauncherConfig {
            schema_version: CONFIG_SCHEMA_VERSION,
            selected_instance_id: Some(InstanceId::new("aurora-default").unwrap()),
            home_widgets: Default::default(),
            discord: Default::default(),
            appearance: AppearancePreferences {
                aurora_motion_speed: 50,
                background: crate::appearance::BackgroundId::Simple,
                theme: "oled".to_owned(),
                accent: crate::appearance::AccentSelection::Custom {
                    hex: "#FF5533".to_owned(),
                },
            },
        };

        let parsed = LauncherConfig::from_json(&config.to_json()).unwrap();

        assert_eq!(parsed, config);
        assert_eq!(
            parsed.selected_instance_id().map(InstanceId::as_str),
            Some("aurora-default")
        );
        assert_eq!(
            parsed.appearance().theme_id(),
            crate::appearance::ThemeId::Oled
        );
    }

    #[test]
    fn serializes_to_inspectable_camel_case_json() {
        let json = LauncherConfig::default().to_json();

        assert!(json.contains("\"schemaVersion\": 7"));
        assert!(json.contains("\"selectedInstanceId\": null"));
        assert!(json.contains("\"appearance\": {"));
        assert!(json.contains("\"theme\": \"aurora-dark\""));
    }

    #[test]
    fn malformed_configurations_fail_and_are_reported_verbatim() {
        for json in [
            "{ not json",
            "{}",
            r#"{ "schemaVersion": "one" }"#,
            // Known schema, structurally broken appearance.
            r#"{ "schemaVersion": 2, "selectedInstanceId": null, "appearance": [] }"#,
            r#"{ "schemaVersion": 2, "selectedInstanceId": null }"#,
        ] {
            let error = LauncherConfig::from_json(json).expect_err("must be rejected");
            assert!(matches!(error, ConfigError::Malformed(_)), "got: {error}");
        }
    }

    #[test]
    fn unsupported_schema_versions_fail_deliberately() {
        let json = r#"{ "schemaVersion": 8, "selectedInstanceId": null }"#;

        let error = LauncherConfig::from_json(json).unwrap_err();

        assert!(matches!(
            error,
            ConfigError::UnsupportedSchema {
                found: 8,
                supported: 7
            }
        ));
    }

    #[test]
    fn schema_one_configurations_migrate_deterministically() {
        let json = r#"{ "schemaVersion": 1, "selectedInstanceId": "aurora-default" }"#;

        let config = LauncherConfig::from_json(json).unwrap();

        assert_eq!(config.schema_version(), CONFIG_SCHEMA_VERSION);
        assert_eq!(
            config.selected_instance_id().map(InstanceId::as_str),
            Some("aurora-default")
        );
        // The migrated document carries the default appearance and persists
        // as schema 2 on its next save.
        assert_eq!(config.appearance(), &AppearancePreferences::new());
        let reserialized = LauncherConfig::from_json(&config.to_json()).unwrap();
        assert_eq!(reserialized, config);
        assert!(config.to_json().contains("\"schemaVersion\": 7"));
    }

    #[test]
    fn unknown_appearance_values_normalize_instead_of_failing_startup() {
        let json = r#"{
            "schemaVersion": 2,
            "selectedInstanceId": null,
            "appearance": {
                "theme": "neon",
                "accent": { "type": "preset", "id": "hotdog" }
            }
        }"#;

        let config = LauncherConfig::from_json(json).unwrap();

        assert_eq!(config.appearance(), &AppearancePreferences::new());
    }

    #[test]
    fn an_unusable_custom_accent_normalizes_to_the_default_accent() {
        let json = r##"{
            "schemaVersion": 2,
            "selectedInstanceId": null,
            "appearance": {
                "theme": "midnight",
                "accent": { "type": "custom", "hex": "#000000" }
            }
        }"##;

        let config = LauncherConfig::from_json(json).unwrap();

        assert_eq!(
            config.appearance().theme_id(),
            crate::appearance::ThemeId::Midnight
        );
        assert_eq!(
            config.appearance().accent,
            crate::appearance::AccentSelection::default()
        );
    }

    #[test]
    fn load_reports_a_missing_file_instead_of_inventing_one() {
        let directory = test_directory("aurora-config-test-missing");
        let path = directory.join("launcher").join("config.json");

        assert_eq!(load(&path).unwrap(), None);
        assert!(!path.exists());
    }

    #[test]
    fn load_or_initialize_materializes_safe_defaults_once() {
        let directory = test_directory("aurora-config-test-initialize");
        let path = directory.join("launcher").join("config.json");

        let first = load_or_initialize(&path).unwrap();
        assert!(matches!(first, ConfigLoad::Initialized(_)));
        assert_eq!(first.config(), &LauncherConfig::default());
        assert!(path.exists());

        let persisted = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            LauncherConfig::from_json(&persisted).unwrap(),
            LauncherConfig::default()
        );

        let second = load_or_initialize(&path).unwrap();
        assert!(matches!(second, ConfigLoad::Existing(_)));
        assert_eq!(second.config(), &LauncherConfig::default());
    }

    #[test]
    fn malformed_configurations_are_never_overwritten() {
        let directory = test_directory("aurora-config-test-malformed");
        let path = directory.join("launcher").join("config.json");
        let damaged = "{ this is not json";
        std::fs::create_dir_all(directory.join("launcher")).unwrap();
        std::fs::write(&path, damaged).unwrap();

        let result = load_or_initialize(&path);

        assert!(matches!(result, Err(ConfigError::Malformed(_))));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), damaged);
    }

    #[test]
    fn save_replaces_existing_configuration_and_leaves_no_temporary_files() {
        let directory = test_directory("aurora-config-test-save");
        let path = directory.join("launcher").join("config.json");
        save(&path, &LauncherConfig::default()).unwrap();

        let updated = LauncherConfig {
            schema_version: CONFIG_SCHEMA_VERSION,
            selected_instance_id: Some(InstanceId::new("beta-playground").unwrap()),
            home_widgets: Default::default(),
            discord: Default::default(),
            appearance: AppearancePreferences {
                aurora_motion_speed: 50,
                background: crate::appearance::BackgroundId::Simple,
                theme: "midnight".to_owned(),
                accent: crate::appearance::AccentSelection::Preset {
                    id: "blue".to_owned(),
                },
            },
        };
        save(&path, &updated).unwrap();

        assert_eq!(load(&path).unwrap(), Some(updated));
        let names: Vec<_> = std::fs::read_dir(directory.join("launcher"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["config.json".to_owned()]);
    }

    #[test]
    fn save_creates_missing_parent_directories() {
        let directory = test_directory("aurora-config-test-dirs");
        let path = directory.join("launcher").join("config.json");

        save(&path, &LauncherConfig::default()).unwrap();

        assert!(path.is_file());
    }
    #[test]
    fn schema_three_privacy_migration_preserves_every_existing_value() {
        for mask in 0..64 {
            let mut config = LauncherConfig::default();
            config.set_selected_instance_id(Some(InstanceId::new("selected").unwrap()));
            let mut document = serde_json::to_value(&config).unwrap();
            document["schemaVersion"] = 3.into();
            let prefs = document["discord"].as_object_mut().unwrap();
            for key in ["world", "server", "serverAddress"] {
                prefs.remove(key);
            }
            for (bit, key) in [
                "enabled",
                "instanceName",
                "minecraftVersion",
                "platform",
                "auroraActive",
                "elapsedTime",
            ]
            .iter()
            .enumerate()
            {
                prefs.insert((*key).into(), (mask & (1 << bit) != 0).into());
            }
            let original = document.clone();
            let migrated = LauncherConfig::from_json(&document.to_string()).unwrap();
            let output = serde_json::to_value(&migrated).unwrap();
            for key in ["selectedInstanceId", "appearance", "homeWidgets"] {
                assert_eq!(output[key], original[key]);
            }
            for key in [
                "enabled",
                "instanceName",
                "minecraftVersion",
                "platform",
                "auroraActive",
                "elapsedTime",
            ] {
                assert_eq!(output["discord"][key], original["discord"][key]);
            }
            assert!(
                !migrated.discord().world
                    && !migrated.discord().server
                    && !migrated.discord().server_address
            );
            assert_eq!(
                migrated,
                LauncherConfig::from_json(&migrated.to_json()).unwrap()
            );
        }
    }
    #[test]
    fn missing_new_current_schema_privacy_is_damage_not_a_migration() {
        let mut document = serde_json::to_value(LauncherConfig::default()).unwrap();
        document["discord"].as_object_mut().unwrap().remove("world");
        assert!(LauncherConfig::from_json(&document.to_string()).is_err());
        document["schemaVersion"] = 3.into();
        document["discord"]["enabled"] = "yes".into();
        assert!(LauncherConfig::from_json(&document.to_string()).is_err());
    }
}
