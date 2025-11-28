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
