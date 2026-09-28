//! Declarative Home layout. Unknown IDs survive, but cannot execute anything.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WidgetSize {
    Small,
    Wide,
    Large,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WidgetPlacement {
    pub id: String,
    pub enabled: bool,
    pub size: WidgetSize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HomeLayout {
    pub widgets: Vec<WidgetPlacement>,
}

pub fn supported_sizes(id: &str) -> Option<&'static [WidgetSize]> {
    use WidgetSize::*;
    match id {
        "instance-details" | "content-summary" => Some(&[Small, Wide, Large]),
        "session" => Some(&[Small, Wide]),
        "playtime" | "recent-worlds" | "recent-servers" => Some(&[Small, Wide, Large]),
        _ => None,
    }
}

impl Default for HomeLayout {
    fn default() -> Self {
        Self {
            widgets: vec![
                WidgetPlacement {
                    id: "instance-details".into(),
                    enabled: true,
                    size: WidgetSize::Small,
                },
                WidgetPlacement {
                    id: "content-summary".into(),
                    enabled: true,
                    size: WidgetSize::Small,
                },
                WidgetPlacement {
                    id: "session".into(),
                    enabled: false,
                    size: WidgetSize::Small,
                },
                WidgetPlacement {
                    id: "playtime".into(),
                    enabled: true,
                    size: WidgetSize::Small,
                },
                WidgetPlacement {
                    id: "recent-worlds".into(),
                    enabled: true,
                    size: WidgetSize::Small,
                },
                WidgetPlacement {
                    id: "recent-servers".into(),
                    enabled: true,
                    size: WidgetSize::Small,
                },
            ],
        }
    }
}

impl HomeLayout {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.widgets.len() > 64 {
            return Err("Home supports at most 64 saved widget entries.");
        }
        let mut ids = HashSet::new();
        for widget in &self.widgets {
            if widget.id.is_empty()
                || widget.id.len() > 64
                || !widget
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                || !ids.insert(&widget.id)
            {
                return Err("Widget IDs must be unique bounded lowercase identifiers.");
            }
            if supported_sizes(&widget.id).is_some_and(|sizes| !sizes.contains(&widget.size)) {
                return Err("The requested widget size is outside its supported bounds.");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_restrained_and_valid() {
        let layout = HomeLayout::default();
        layout.validate().unwrap();
        assert_eq!(layout.widgets.iter().filter(|w| w.enabled).count(), 5);
    }
    #[test]
    fn declarative_changes_round_trip_and_reset() {
        let mut layout = HomeLayout::default();
        layout.widgets[0].enabled = false;
        layout.widgets[2].enabled = true;
        layout.widgets.swap(0, 2);
        layout.widgets[2].size = WidgetSize::Large;
        layout.validate().unwrap();
        let loaded: HomeLayout =
            serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
        assert_eq!(loaded, layout);
        assert_ne!(layout, HomeLayout::default());
        layout = HomeLayout::default();
        assert!(layout.widgets[0].enabled);
    }
    #[test]
    fn unknown_widgets_survive_without_registration() {
        let mut layout = HomeLayout::default();
        layout.widgets.push(WidgetPlacement {
            id: "future-cosmetics".into(),
            enabled: true,
            size: WidgetSize::Wide,
        });
        layout.validate().unwrap();
        assert!(supported_sizes("future-cosmetics").is_none());
    }
    #[test]
    fn malformed_and_out_of_bounds_layouts_fail() {
        let mut layout = HomeLayout::default();
        layout.widgets[2].size = WidgetSize::Large;
        assert!(layout.validate().is_err());
        layout.widgets[2].size = WidgetSize::Small;
        layout.widgets.push(layout.widgets[0].clone());
        assert!(layout.validate().is_err());
        assert!(
            serde_json::from_str::<HomeLayout>(
                r#"{"widgets":[{"id":"session","enabled":true,"size":"pixels"}]}"#
            )
            .is_err()
        );
    }
}
