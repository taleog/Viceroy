use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: Theme,
    pub file_hiding_patterns: Vec<String>,
    pub retype_delay_enabled: bool,
    pub max_results: usize,
    pub hotkey: String,
    pub dismiss_on_escape: bool,
    pub dismiss_on_click_away: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub background_color: String,
    pub text_color: String,
    pub accent_color: String,
    pub selection_color: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme {
                background_color: "#1e1e1e".to_string(),
                text_color: "#d4d4d4".to_string(),
                accent_color: "#007acc".to_string(),
                selection_color: "#264f78".to_string(),
            },
            file_hiding_patterns: vec![
                r"\.git".to_string(),
                r"node_modules".to_string(),
                r"\.DS_Store".to_string(),
            ],
            retype_delay_enabled: false,
            max_results: 50,
            hotkey: "Alt+Space".to_string(),
            dismiss_on_escape: true,
            dismiss_on_click_away: true,
        }
    }
}

pub fn load() -> Result<Settings> {
    let path = get_settings_path();

    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let settings: Settings = serde_json::from_str(&content)?;
        Ok(settings)
    } else {
        let settings = Settings::default();
        save(&settings)?;
        Ok(settings)
    }
}

pub fn save(settings: &Settings) -> Result<()> {
    let path = get_settings_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(settings)?;
    fs::write(&path, content)?;

    Ok(())
}

fn get_settings_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("viceroy");
    path.push("settings.json");
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test default values
    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.max_results, 50);
        assert_eq!(settings.hotkey, "Alt+Space");
        assert!(settings.dismiss_on_escape);
        assert!(settings.dismiss_on_click_away);
        assert!(!settings.retype_delay_enabled);
    }

    #[test]
    fn test_default_theme() {
        let settings = Settings::default();
        assert_eq!(settings.theme.background_color, "#1e1e1e");
        assert_eq!(settings.theme.text_color, "#d4d4d4");
        assert_eq!(settings.theme.accent_color, "#007acc");
        assert_eq!(settings.theme.selection_color, "#264f78");
    }

    #[test]
    fn test_default_file_hiding_patterns() {
        let settings = Settings::default();
        assert!(settings
            .file_hiding_patterns
            .contains(&r"\.git".to_string()));
        assert!(settings
            .file_hiding_patterns
            .contains(&r"node_modules".to_string()));
        assert!(settings
            .file_hiding_patterns
            .contains(&r"\.DS_Store".to_string()));
    }

    // Test JSON serialization/deserialization
    #[test]
    fn test_settings_serialization() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(settings.max_results, deserialized.max_results);
        assert_eq!(settings.hotkey, deserialized.hotkey);
        assert_eq!(settings.dismiss_on_escape, deserialized.dismiss_on_escape);
    }

    #[test]
    fn test_settings_serialization_pretty() {
        let settings = Settings::default();
        let json = serde_json::to_string_pretty(&settings).unwrap();

        // Pretty print should have newlines and indentation
        assert!(json.contains('\n'));
        assert!(json.contains("  ")); // Indentation
    }

    #[test]
    fn test_theme_serialization() {
        let theme = Theme {
            background_color: "#000000".to_string(),
            text_color: "#ffffff".to_string(),
            accent_color: "#ff0000".to_string(),
            selection_color: "#00ff00".to_string(),
        };
        let json = serde_json::to_string(&theme).unwrap();
        let deserialized: Theme = serde_json::from_str(&json).unwrap();

        assert_eq!(theme.background_color, deserialized.background_color);
        assert_eq!(theme.text_color, deserialized.text_color);
        assert_eq!(theme.accent_color, deserialized.accent_color);
        assert_eq!(theme.selection_color, deserialized.selection_color);
    }

    #[test]
    fn test_settings_with_custom_values() {
        // Build settings programmatically instead of parsing JSON
        let settings = Settings {
            theme: Theme {
                background_color: "#ffffff".to_string(),
                text_color: "#000000".to_string(),
                accent_color: "#0000ff".to_string(),
                selection_color: "#ff00ff".to_string(),
            },
            file_hiding_patterns: vec!["*.tmp".to_string(), "*.bak".to_string()],
            retype_delay_enabled: true,
            max_results: 100,
            hotkey: "Ctrl+Space".to_string(),
            dismiss_on_escape: false,
            dismiss_on_click_away: false,
        };

        // Serialize and deserialize to test round-trip
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.max_results, 100);
        assert_eq!(deserialized.hotkey, "Ctrl+Space");
        assert!(!deserialized.dismiss_on_escape);
        assert!(!deserialized.dismiss_on_click_away);
        assert!(deserialized.retype_delay_enabled);
        assert_eq!(deserialized.theme.background_color, "#ffffff");
    }

    // Test Theme struct
    #[test]
    fn test_theme_clone() {
        let theme = Theme {
            background_color: "#123456".to_string(),
            text_color: "#abcdef".to_string(),
            accent_color: "#fedcba".to_string(),
            selection_color: "#654321".to_string(),
        };
        let cloned = theme.clone();

        assert_eq!(theme.background_color, cloned.background_color);
        assert_eq!(theme.text_color, cloned.text_color);
        assert_eq!(theme.accent_color, cloned.accent_color);
        assert_eq!(theme.selection_color, cloned.selection_color);
    }

    #[test]
    fn test_settings_clone() {
        let settings = Settings::default();
        let cloned = settings.clone();

        assert_eq!(settings.max_results, cloned.max_results);
        assert_eq!(settings.hotkey, cloned.hotkey);
        assert_eq!(settings.file_hiding_patterns, cloned.file_hiding_patterns);
    }

    // Test file hiding patterns
    #[test]
    fn test_empty_file_hiding_patterns() {
        let settings = Settings {
            theme: Settings::default().theme,
            file_hiding_patterns: vec![],
            retype_delay_enabled: false,
            max_results: 50,
            hotkey: "Alt+Space".to_string(),
            dismiss_on_escape: true,
            dismiss_on_click_away: true,
        };

        // Serialize and deserialize to test round-trip
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();

        assert!(deserialized.file_hiding_patterns.is_empty());
    }

    #[test]
    fn test_multiple_file_hiding_patterns() {
        let settings = Settings {
            theme: Settings::default().theme,
            file_hiding_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                ".DS_Store".to_string(),
                "target".to_string(),
                "build".to_string(),
            ],
            retype_delay_enabled: false,
            max_results: 50,
            hotkey: "Alt+Space".to_string(),
            dismiss_on_escape: true,
            dismiss_on_click_away: true,
        };

        // Serialize and deserialize to test round-trip
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.file_hiding_patterns.len(), 5);
        assert!(deserialized
            .file_hiding_patterns
            .contains(&"target".to_string()));
        assert!(deserialized
            .file_hiding_patterns
            .contains(&"build".to_string()));
    }
}
