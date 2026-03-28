use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
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
        let (settings, migrated) = parse_settings_content(&content)?;
        if migrated {
            save(&settings)?;
        }
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

fn parse_settings_content(content: &str) -> Result<(Settings, bool)> {
    let mut value: Value = serde_json::from_str(content)?;
    let migrated = migrate_legacy_sync_fields(&mut value);
    let settings: Settings = serde_json::from_value(value)?;
    Ok((settings, migrated))
}

fn migrate_legacy_sync_fields(root: &mut Value) -> bool {
    let Some(object) = root.as_object_mut() else {
        return false;
    };

    let mut migrated = false;
    let sync_enabled = object.remove("sync_enabled");
    let sync_device_id = object.remove("sync_device_id");
    let sync_device_name = object.remove("sync_device_name");
    let sync_server_url = object.remove("sync_server_url");
    let sync_auth_token = object.remove("sync_auth_token");

    migrated |= sync_enabled.is_some();
    migrated |= sync_device_id.is_some();
    migrated |= sync_device_name.is_some();
    migrated |= sync_server_url.is_some();
    migrated |= sync_auth_token.is_some();

    let sync = object
        .entry("sync".to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    if !sync.is_object() {
        *sync = Value::Object(Map::new());
        migrated = true;
    }

    let sync_object = sync
        .as_object_mut()
        .expect("sync settings should always be an object after normalization");

    insert_legacy_value(sync_object, "enabled", sync_enabled);
    insert_legacy_value(sync_object, "device_id", sync_device_id);
    insert_legacy_value(sync_object, "device_name", sync_device_name);
    insert_legacy_value_with_empty_as_null(sync_object, "server_url", sync_server_url);
    insert_legacy_value_with_empty_as_null(sync_object, "auth_token", sync_auth_token);

    migrated
}

fn insert_legacy_value(sync: &mut Map<String, Value>, sync_key: &str, value: Option<Value>) {
    let Some(value) = value else {
        return;
    };

    if !sync.contains_key(sync_key) {
        sync.insert(sync_key.to_string(), value);
    }
}

fn insert_legacy_value_with_empty_as_null(
    sync: &mut Map<String, Value>,
    sync_key: &str,
    value: Option<Value>,
) {
    let Some(value) = value else {
        return;
    };

    if !sync.contains_key(sync_key) {
        let mapped = match value {
            Value::String(text) if text.trim().is_empty() => Value::Null,
            other => other,
        };
        sync.insert(sync_key.to_string(), mapped);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_settings_content;

    #[test]
    fn loads_legacy_flat_sync_settings() {
        let legacy = r#"{
  "max_results": 52,
  "sync_enabled": true,
  "sync_server_url": "http://127.0.0.1:8787",
  "sync_device_name": "This Mac",
  "sync_auth_token": ""
}"#;

        let (settings, migrated) = parse_settings_content(legacy).expect("load settings");

        assert!(migrated);
        assert_eq!(settings.max_results, 52);
        assert!(settings.sync.enabled);
        assert_eq!(
            settings.sync.server_url.as_deref(),
            Some("http://127.0.0.1:8787")
        );
        assert_eq!(settings.sync.device_name, "This Mac");
        assert_eq!(settings.sync.auth_token, None);
    }

    #[test]
    fn nested_sync_settings_stay_as_is() {
        let nested = r#"{
  "sync": {
    "enabled": true,
    "server_url": "https://sync.example.com",
    "device_name": "Office Laptop"
  }
}"#;

        let (settings, migrated) = parse_settings_content(nested).expect("load settings");

        assert!(!migrated);
        assert!(settings.sync.enabled);
        assert_eq!(
            settings.sync.server_url.as_deref(),
            Some("https://sync.example.com")
        );
        assert_eq!(settings.sync.device_name, "Office Laptop");
    }
}
