use serde::{Deserialize, Serialize};
use std::fs;

use crate::app::{get_config_path, Priority};

pub mod theme;
pub use theme::Theme;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Default)]
pub enum ColorTheme {
    #[default]
    Default,
    Dracula,
    Solarized,
    Nord,
    GruvboxDark,
    Cyberpunk,
    Custom,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CustomThemeColors {
    pub high_color: Option<String>,
    pub medium_color: Option<String>,
    pub low_color: Option<String>,
    pub done_color: Option<String>,
    pub high_bg: Option<String>,
    pub medium_bg: Option<String>,
    pub low_bg: Option<String>,
    pub accent_color: Option<String>,
    pub base_fg: Option<String>,
    pub base_bg: Option<String>,
    pub highlight_bg: Option<String>,
    pub help_text_fg: Option<String>,
}

fn default_priority() -> Priority {
    Priority::Medium
}
fn default_notifications() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SerializableSettings {
    #[serde(default)]
    theme: ColorTheme,
    #[serde(default = "default_notifications")]
    desktop_notifications: bool,
    #[serde(default = "default_priority")]
    default_priority: Priority,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_theme: Option<CustomThemeColors>,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub theme: ColorTheme,
    pub desktop_notifications: bool,
    pub default_priority: Priority,
    pub custom_theme: Option<CustomThemeColors>,
}

impl From<SerializableSettings> for Settings {
    fn from(s: SerializableSettings) -> Self {
        Self {
            theme: s.theme,
            desktop_notifications: s.desktop_notifications,
            default_priority: s.default_priority,
            custom_theme: s.custom_theme,
        }
    }
}

impl From<&Settings> for SerializableSettings {
    fn from(s: &Settings) -> Self {
        Self {
            theme: s.theme,
            desktop_notifications: s.desktop_notifications,
            default_priority: s.default_priority,
            custom_theme: s.custom_theme.clone(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ColorTheme::Default,
            desktop_notifications: true,
            default_priority: Priority::Medium,
            custom_theme: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        if let Some(path) = get_config_path() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(serializable) = toml::from_str::<SerializableSettings>(&content) {
                    return serializable.into();
                }
            }
        }
        let default_settings = Settings::default();
        default_settings.save();
        default_settings
    }

    pub fn save(&self) {
        if let Some(path) = get_config_path() {
            if let Some(parent) = path.parent() {
                if fs::create_dir_all(parent).is_ok() {
                    let serializable = SerializableSettings::from(self);
                    if let Ok(toml_string) = toml::to_string_pretty(&serializable) {
                        let _ = fs::write(path, toml_string);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_with_custom_theme() {
        let toml = r##"
theme = "Default"
desktop_notifications = true
default_priority = "High"

[custom_theme]
high_color = "#fb4934"
base_bg = "#282828"
"##;
        let s: SerializableSettings = toml::from_str(toml).expect("parse failed");
        assert!(s.custom_theme.is_some(), "custom_theme should be Some");
        let ct = s.custom_theme.unwrap();
        assert_eq!(ct.high_color.as_deref(), Some("#fb4934"));
        assert!(ct.medium_color.is_none(), "unset field should be None");
    }

    #[test]
    fn deserialize_without_priority_defaults_medium() {
        let toml = r##"
theme = "Default"
desktop_notifications = true
"##;
        let s: SerializableSettings =
            toml::from_str(toml).expect("parse should not fail without default_priority");
        assert_eq!(s.default_priority, Priority::Medium);
    }
}
