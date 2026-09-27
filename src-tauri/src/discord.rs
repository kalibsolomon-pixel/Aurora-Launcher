//! Optional local desktop presence. No account linking, tokens, or gameplay inference.
use crate::launch::state::LaunchProcessStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscordPreferences {
    pub enabled: bool,
    pub instance_name: bool,
    pub minecraft_version: bool,
    pub platform: bool,
    pub aurora_active: bool,
    pub elapsed_time: bool,
}

/// Only explicitly permitted non-secret facts can enter the presence builder.
/// This type cannot receive LaunchSpec, sessions, arguments, paths or logs.
#[derive(Clone)]
pub struct GameActivity {
    pub instance_id: String,
    pub instance_name: String,
    pub minecraft_version: String,
    pub platform: String,
    pub aurora_active: bool,
    pub status: LaunchProcessStatus,
    pub started_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Activity {
    pub details: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Timestamps>,
    pub assets: Assets,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Timestamps {
    pub start: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assets {
    pub large_image: &'static str,
    pub large_text: &'static str,
}

fn display_text(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(100)
        .collect()
}
pub fn activity(preferences: &DiscordPreferences, game: Option<&GameActivity>) -> Option<Activity> {
    if !preferences.enabled {
        return None;
    }
    let game = game.filter(|game| game.status.blocks_launch());
    let details = match game.map(|game| game.status) {
        Some(LaunchProcessStatus::Starting) => "Starting Minecraft",
        Some(LaunchProcessStatus::Running) => "Playing Minecraft",
        _ => "In Launcher",
    };
    let mut fields = Vec::new();
    if let Some(game) = game {
        if preferences.minecraft_version {
            fields.push(format!(
                "Minecraft {}",
                display_text(&game.minecraft_version)
            ));
        }
        if preferences.platform {
            fields.push(display_text(&game.platform));
        }
        if preferences.aurora_active && game.aurora_active {
            fields.push("Aurora Client active at launch".into());
        }
        if preferences.instance_name {
            fields.push(display_text(&game.instance_name));
        }
    }
    Some(Activity {
        details: details.into(),
        state: (!fields.is_empty()).then(|| fields.join(" · ").chars().take(128).collect()),
        timestamps: game
            .filter(|g| preferences.elapsed_time && g.status == LaunchProcessStatus::Running)
            .and_then(|g| g.started_at)
            .map(|start| Timestamps { start }),
        assets: Assets {
            large_image: "aurora-logo",
            large_text: "Aurora Client",
        },
    })
}

pub fn application_id() -> Option<&'static str> {
    option_env!("AURORA_DISCORD_APPLICATION_ID")
        .map(str::trim)
        .filter(|id| {
            (17..=20).contains(&id.len())
                && id.bytes().all(|b| b.is_ascii_digit())
                && id.parse::<u64>().is_ok_and(|id| id > 0)
        })
}

mod transport;
mod worker;
pub use worker::{
    DiscordState, connect, initialize, preferences_changed, process_changed, shutdown, state,
};

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> GameActivity {
        GameActivity {
            instance_id: "fixture".into(),
            instance_name: "Private instance".into(),
            minecraft_version: "1.21.11".into(),
            platform: "Fabric".into(),
            aurora_active: true,
            status: LaunchProcessStatus::Running,
            started_at: Some(123),
        }
    }
    #[test]
    fn defaults_publish_nothing() {
        assert!(activity(&DiscordPreferences::default(), Some(&game())).is_none());
    }
    #[test]
    fn opt_in_generic_presence_omits_private_fields_and_time() {
        let prefs = DiscordPreferences {
            enabled: true,
            ..Default::default()
        };
        let value = activity(&prefs, Some(&game())).unwrap();
        assert_eq!(value.details, "Playing Minecraft");
        assert!(value.state.is_none());
        assert!(value.timestamps.is_none());
    }
    #[test]
    fn each_supported_field_is_independent() {
        for (field, expected) in [
            (0, "Private instance"),
            (1, "Minecraft 1.21.11"),
            (2, "Fabric"),
            (3, "Aurora Client active at launch"),
        ] {
            let mut prefs = DiscordPreferences {
                enabled: true,
                ..Default::default()
            };
            match field {
                0 => prefs.instance_name = true,
                1 => prefs.minecraft_version = true,
                2 => prefs.platform = true,
                _ => prefs.aurora_active = true,
            }
            assert_eq!(
                activity(&prefs, Some(&game())).unwrap().state.as_deref(),
                Some(expected)
            );
        }
        let prefs = DiscordPreferences {
            enabled: true,
            elapsed_time: true,
            ..Default::default()
        };
        assert_eq!(
            activity(&prefs, Some(&game()))
                .unwrap()
                .timestamps
                .unwrap()
                .start,
            123
        );
    }
    #[test]
    fn starting_running_exit_and_launcher_use_exact_supervisor_state() {
        let prefs = DiscordPreferences {
            enabled: true,
            elapsed_time: true,
            ..Default::default()
        };
        let mut game = game();
        game.status = LaunchProcessStatus::Starting;
        let value = activity(&prefs, Some(&game)).unwrap();
        assert_eq!(value.details, "Starting Minecraft");
        assert!(value.timestamps.is_none());
        game.status = LaunchProcessStatus::Exited;
        let value = activity(&prefs, Some(&game)).unwrap();
        assert_eq!(value.details, "In Launcher");
        assert!(value.timestamps.is_none());
    }
    #[test]
    fn payload_has_no_gameplay_or_secret_fields() {
        let prefs = DiscordPreferences {
            enabled: true,
            ..Default::default()
        };
        let json = serde_json::to_value(activity(&prefs, Some(&game())).unwrap()).unwrap();
        for key in [
            "secrets",
            "party",
            "world",
            "server",
            "arguments",
            "token",
            "accountId",
        ] {
            assert!(json.get(key).is_none());
        }
    }
}
