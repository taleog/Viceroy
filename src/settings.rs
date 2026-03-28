use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub file_hiding_patterns: Vec<String>,
    pub retype_delay_enabled: bool,
    pub max_results: usize,
    pub hotkey: String,
    pub dismiss_on_escape: bool,
    pub dismiss_on_click_away: bool,
    pub sync: SyncSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub background_color: String,
    pub text_color: String,
    pub accent_color: String,
    pub selection_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncSettings {
    pub enabled: bool,
    pub device_id: String,
    pub device_name: String,
    pub server_url: Option<String>,
    pub auth_token: Option<String>,
    pub poll_interval_seconds: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::default(),
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
            sync: SyncSettings::default(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            background_color: "#1e1e1e".to_string(),
            text_color: "#d4d4d4".to_string(),
            accent_color: "#007acc".to_string(),
            selection_color: "#264f78".to_string(),
        }
    }
}

impl Default for SyncSettings {
    fn default() -> Self {
        SyncSettings {
            enabled: false,
            device_id: String::new(),
            device_name: default_device_name(),
            server_url: None,
            auth_token: None,
            poll_interval_seconds: 15,
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

fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Viceroy Device".to_string())
}
