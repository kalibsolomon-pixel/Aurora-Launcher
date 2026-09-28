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
    pub world: bool,
    pub server: bool,
    pub server_address: bool,
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
    pub(crate) gameplay: Option<crate::launch::activity_bridge::ActivityHandle>,
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
    crate::launch::activity_bridge::sanitize(value)
        .chars()
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
        if game.status == LaunchProcessStatus::Running {
            if let Some(snapshot) = game.gameplay.as_ref().and_then(|h| h.snapshot()) {
                use crate::launch::activity_bridge::GameplayState;
                match snapshot.state {
                    GameplayState::Singleplayer if preferences.world => {
                        if let Some(world) = snapshot.world {
                            fields.push(format!("World: {}", display_text(&world)));
                        }
                    }
                    GameplayState::Multiplayer if preferences.server => {
                        // Minecraft display names may themselves equal/contain the raw address.
                        // Never send such a name without the separate address consent.
                        if let Some(name) = snapshot.server_name {
                            if preferences.server_address
                                || !snapshot.server_address.as_ref().is_some_and(|address| {
                                    let name = crate::launch::activity_bridge::sanitize(&name)
                                        .to_lowercase();
                                    let address = crate::launch::activity_bridge::sanitize(address)
                                        .to_lowercase();
                                    let host = address
                                        .strip_prefix('[')
                                        .and_then(|s| s.split_once(']').map(|(host, _)| host))
                                        .or_else(|| {
                                            address
                                                .rsplit_once(':')
                                                .filter(|(_, port)| {
                                                    port.bytes().all(|b| b.is_ascii_digit())
                                                })
                                                .map(|(host, _)| host)
                                        })
                                        .unwrap_or(&address);
                                    name.contains(&address)
                                        || (!host.is_empty() && name.contains(host))
                                })
                            {
                                fields.push(format!("Server: {}", display_text(&name)));
                            }
                        }
                        if preferences.server_address {
                            if let Some(address) = snapshot.server_address {
                                fields.push(format!("Address: {}", display_text(&address)));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
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
    DiscordState, connect, gameplay_changed, initialize, preferences_changed, process_changed,
    shutdown, state,
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
            gameplay: None,
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
        assert_eq!(value.assets.large_image, "aurora-logo");
        assert_eq!(value.assets.large_text, "Aurora Client");
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
    #[test]
    fn all_gameplay_privacy_combinations_filter_before_serialization() {
        use crate::launch::activity_bridge::{ActivityHandle, GameplayState, Snapshot};
        for state in [
            GameplayState::MainMenu,
            GameplayState::Singleplayer,
            GameplayState::Multiplayer,
        ] {
            for mask in 0..16 {
                let prefs = DiscordPreferences {
                    enabled: mask & 1 != 0,
                    world: mask & 2 != 0,
                    server: mask & 4 != 0,
                    server_address: mask & 8 != 0,
                    ..Default::default()
                };
                let mut game = game();
                game.gameplay = Some(ActivityHandle::fixture(Snapshot {
                    state,
                    world: (state == GameplayState::Singleplayer).then(|| "Fixture World".into()),
                    server_name: (state == GameplayState::Multiplayer)
                        .then(|| "Fixture SMP".into()),
                    server_address: (state == GameplayState::Multiplayer)
                        .then(|| "example.invalid".into()),
                    world_save_id: None,
                    server_target: None,
                }));
                let projected = activity(&prefs, Some(&game));
                assert_eq!(projected.is_some(), prefs.enabled);
                let json = serde_json::to_string(&projected).unwrap();
                assert_eq!(
                    json.contains("Fixture World"),
                    prefs.enabled && prefs.world && state == GameplayState::Singleplayer
                );
                assert_eq!(
                    json.contains("Fixture SMP"),
                    prefs.enabled && prefs.server && state == GameplayState::Multiplayer
                );
                assert_eq!(
                    json.contains("example.invalid"),
                    prefs.enabled
                        && prefs.server
                        && prefs.server_address
                        && state == GameplayState::Multiplayer
                );
            }
        }
    }
    #[test]
    fn validated_history_targets_never_enter_discord_projection() {
        use crate::launch::activity_bridge::{ActivityHandle, GameplayState, Snapshot};
        let mut game = game();
        game.gameplay = Some(ActivityHandle::fixture(Snapshot {
            state: GameplayState::Singleplayer,
            world: Some("Friendly title".into()),
            server_name: None,
            server_address: None,
            world_save_id: Some(
                crate::gameplay_history::WorldSaveId::parse("private-save".into()).unwrap(),
            ),
            server_target: None,
        }));
        let prefs = DiscordPreferences {
            enabled: true,
            world: true,
            server: true,
            server_address: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&activity(&prefs, Some(&game))).unwrap();
        assert!(json.contains("Friendly title"));
        assert!(!json.contains("private-save"));
        game.gameplay = Some(ActivityHandle::fixture(Snapshot {
            state: GameplayState::Multiplayer,
            world: None,
            server_name: Some("Friendly server".into()),
            server_address: None,
            world_save_id: None,
            server_target: Some(
                crate::gameplay_history::ServerTarget::parse("private.invalid".into()).unwrap(),
            ),
        }));
        let json = serde_json::to_string(&activity(&prefs, Some(&game))).unwrap();
        assert!(json.contains("Friendly server"));
        assert!(!json.contains("private.invalid"));
    }
    #[test]
    fn address_is_never_a_name_fallback_or_hidden_in_the_name() {
        use crate::launch::activity_bridge::{ActivityHandle, GameplayState, Snapshot};
        let prefs = DiscordPreferences {
            enabled: true,
            server: true,
            ..Default::default()
        };
        for (name, address) in [
            (None, "example.invalid"),
            (Some("example.invalid"), "example.invalid"),
            (Some("EXAMPLE.INVALID"), "example.invalid"),
            (Some("My example.invalid server"), "example.invalid"),
            (Some("example.invalid"), "example.invalid:25565"),
            (Some("My 127.0.0.1 server"), "127.0.0.1:25565"),
            (Some("My ::1 server"), "[::1]:25565"),
            (Some("example\u{200b}.invalid"), "example.invalid:25565"),
        ] {
            let mut game = game();
            game.gameplay = Some(ActivityHandle::fixture(Snapshot {
                state: GameplayState::Multiplayer,
                world: None,
                server_name: name.map(str::to_owned),
                server_address: Some(address.into()),
                world_save_id: None,
                server_target: None,
            }));
            assert!(activity(&prefs, Some(&game)).unwrap().state.is_none());
            game.status = LaunchProcessStatus::Starting;
            assert!(
                activity(
                    &DiscordPreferences {
                        server_address: true,
                        ..prefs.clone()
                    },
                    Some(&game)
                )
                .unwrap()
                .state
                .is_none()
            );
        }
    }
}
