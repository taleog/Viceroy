use crate::sync;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_HOTKEY: &str = "Alt+Space";
const MIN_RESULTS: usize = 10;
const MAX_RESULTS: usize = 200;
const MIN_POLL_INTERVAL_SECONDS: u64 = 5;
const MAX_POLL_INTERVAL_SECONDS: u64 = 3600;

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
    pub paste_after_restore: bool,
    pub sync: SyncSettings,
    pub obsidian: ObsidianSettings,
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
    pub mirror_clipboard: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObsidianSettings {
    pub enabled: bool,
    pub vault_path: Option<String>,
    pub vault_name: Option<String>,
    pub open_in_obsidian: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSyncSettings {
    pub device_name: String,
    pub server_url: Option<String>,
    pub auth_token: Option<String>,
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
            hotkey: DEFAULT_HOTKEY.to_string(),
            dismiss_on_escape: true,
            dismiss_on_click_away: true,
            paste_after_restore: true,
            sync: SyncSettings::default(),
            obsidian: ObsidianSettings::default(),
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
            mirror_clipboard: false,
        }
    }
}

impl Default for ObsidianSettings {
    fn default() -> Self {
        ObsidianSettings {
            enabled: false,
            vault_path: None,
            vault_name: None,
            open_in_obsidian: true,
        }
    }
}

pub fn load() -> Result<Settings> {
    load_from_path(&get_settings_path())
}

pub fn save(settings: &Settings) -> Result<()> {
    save_to_path(&get_settings_path(), settings)
}

pub fn prepare_sync_settings(
    enabled: bool,
    device_name_input: &str,
    server_url_input: &str,
    auth_token_input: &str,
) -> Result<PreparedSyncSettings> {
    let device_name = normalize_device_name(device_name_input);
    let auth_token = normalize_optional_text(Some(auth_token_input));
    let server_url = match normalize_optional_text(Some(server_url_input)) {
        Some(server_url) => {
            let normalized =
                sync::normalize_server_url(&server_url).context("invalid sync server URL")?;
            if enabled {
                sync::validate_server_url_for_local_device(&normalized)
                    .context("invalid sync server URL")?;
            }
            Some(normalized)
        }
        None if enabled => {
            return Err(anyhow!("Enter a sync server URL before enabling sync."));
        }
        None => None,
    };

    Ok(PreparedSyncSettings {
        device_name,
        server_url,
        auth_token,
    })
}

fn load_from_path(path: &Path) -> Result<Settings> {
    if path.exists() {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read settings file at {}", path.display()))?;
        match parse_settings_content(&content) {
            Ok((settings, migrated)) => {
                if migrated {
                    save_to_path(path, &settings)?;
                }
                Ok(settings)
            }
            Err(err) => recover_invalid_settings_file(path, &content, err),
        }
    } else {
        let settings = Settings::default();
        save_to_path(path, &settings)?;
        Ok(settings)
    }
}

fn save_to_path(path: &Path, settings: &Settings) -> Result<()> {
    let mut normalized = settings.clone();
    normalize_settings(&mut normalized);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_vec_pretty(&normalized)?;
    let tmp_path = temporary_settings_path(path);
    fs::write(&tmp_path, content).with_context(|| {
        format!(
            "failed to write temporary settings file at {}",
            tmp_path.display()
        )
    })?;
    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to replace settings file {} with {}",
            path.display(),
            tmp_path.display()
        )
    })?;

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
    let mut migrated = migrate_legacy_sync_fields(&mut value);
    let mut settings: Settings = serde_json::from_value(value)?;
    migrated |= normalize_settings(&mut settings);
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

fn normalize_settings(settings: &mut Settings) -> bool {
    let mut changed = false;

    let trimmed_hotkey = settings.hotkey.trim();
    let normalized_hotkey = if trimmed_hotkey.is_empty() {
        DEFAULT_HOTKEY.to_string()
    } else {
        trimmed_hotkey.to_string()
    };
    if settings.hotkey != normalized_hotkey {
        settings.hotkey = normalized_hotkey;
        changed = true;
    }

    let clamped_results = settings.max_results.clamp(MIN_RESULTS, MAX_RESULTS);
    if settings.max_results != clamped_results {
        settings.max_results = clamped_results;
        changed = true;
    }

    let normalized_patterns = settings
        .file_hiding_patterns
        .iter()
        .map(|pattern| pattern.trim())
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| pattern.to_string())
        .collect::<Vec<_>>();
    if settings.file_hiding_patterns != normalized_patterns {
        settings.file_hiding_patterns = normalized_patterns;
        changed = true;
    }
    if settings.file_hiding_patterns.is_empty() {
        settings.file_hiding_patterns = Settings::default().file_hiding_patterns;
        changed = true;
    }

    let normalized_device_name = normalize_device_name(&settings.sync.device_name);
    if settings.sync.device_name != normalized_device_name {
        settings.sync.device_name = normalized_device_name;
        changed = true;
    }

    let trimmed_device_id = settings.sync.device_id.trim().to_string();
    if settings.sync.device_id != trimmed_device_id {
        settings.sync.device_id = trimmed_device_id;
        changed = true;
    }

    let normalized_server_url = normalize_optional_text(settings.sync.server_url.as_deref());
    if settings.sync.server_url != normalized_server_url {
        settings.sync.server_url = normalized_server_url;
        changed = true;
    }

    let normalized_auth_token = normalize_optional_text(settings.sync.auth_token.as_deref());
    if settings.sync.auth_token != normalized_auth_token {
        settings.sync.auth_token = normalized_auth_token;
        changed = true;
    }

    let clamped_poll_interval = settings
        .sync
        .poll_interval_seconds
        .clamp(MIN_POLL_INTERVAL_SECONDS, MAX_POLL_INTERVAL_SECONDS);
    if settings.sync.poll_interval_seconds != clamped_poll_interval {
        settings.sync.poll_interval_seconds = clamped_poll_interval;
        changed = true;
    }

    let normalized_vault_path = normalize_optional_text(settings.obsidian.vault_path.as_deref());
    if settings.obsidian.vault_path != normalized_vault_path {
        settings.obsidian.vault_path = normalized_vault_path;
        changed = true;
    }

    let normalized_vault_name = normalize_optional_text(settings.obsidian.vault_name.as_deref());
    if settings.obsidian.vault_name != normalized_vault_name {
        settings.obsidian.vault_name = normalized_vault_name;
        changed = true;
    }

    if settings.obsidian.enabled && settings.obsidian.vault_path.is_none() {
        settings.obsidian.enabled = false;
        changed = true;
    }

    changed
}

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn normalize_device_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default_device_name()
    } else {
        trimmed.to_string()
    }
}

fn recover_invalid_settings_file(
    path: &Path,
    original_content: &str,
    err: anyhow::Error,
) -> Result<Settings> {
    let backup_path = invalid_settings_backup_path(path);
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&backup_path, original_content).with_context(|| {
        format!(
            "failed to preserve invalid settings file at {}",
            backup_path.display()
        )
    })?;
    eprintln!(
        "Invalid settings file at {} was preserved as {}: {err:#}",
        path.display(),
        backup_path.display()
    );

    let settings = Settings::default();
    save_to_path(path, &settings)?;
    Ok(settings)
}

fn temporary_settings_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    path.with_file_name(format!("{file_name}.tmp"))
}

fn invalid_settings_backup_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("settings");
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("json");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    path.with_file_name(format!("{stem}.invalid-{timestamp}.{ext}"))
}

#[cfg(test)]
mod tests {
    use super::{
        load_from_path, parse_settings_content, prepare_sync_settings, save_to_path, Settings,
        DEFAULT_HOTKEY,
    };
    use std::fs;
    use tempfile::tempdir;

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
        assert!(settings.paste_after_restore);
        assert!(!settings.sync.mirror_clipboard);
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
        assert!(settings.paste_after_restore);
        assert!(!settings.sync.mirror_clipboard);
    }

    #[test]
    fn normalizes_trimmed_and_out_of_range_settings() {
        let raw = r#"{
  "hotkey": "   ",
  "max_results": 500,
  "file_hiding_patterns": ["  ", " node_modules "],
  "sync": {
    "device_name": "   ",
    "device_id": "  device-1  ",
    "server_url": "   ",
    "auth_token": "  ",
    "poll_interval_seconds": 0
  }
}"#;

        let (settings, migrated) = parse_settings_content(raw).expect("load settings");

        assert!(migrated);
        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
        assert_eq!(settings.max_results, 200);
        assert_eq!(settings.file_hiding_patterns, vec!["node_modules"]);
        assert!(!settings.sync.device_name.trim().is_empty());
        assert_eq!(settings.sync.device_id, "device-1");
        assert_eq!(settings.sync.server_url, None);
        assert_eq!(settings.sync.auth_token, None);
        assert_eq!(settings.sync.poll_interval_seconds, 5);
        assert!(settings.paste_after_restore);
        assert!(!settings.sync.mirror_clipboard);
    }

    #[test]
    fn load_from_path_recovers_invalid_json_with_backup() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ invalid json").expect("write invalid settings");

        let settings = load_from_path(&path).expect("recover settings");

        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
        let files = fs::read_dir(dir.path())
            .expect("read dir")
            .map(|entry| {
                entry
                    .expect("dir entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert!(files
            .iter()
            .any(|name| name.starts_with("settings.invalid-")));
        let saved = fs::read_to_string(&path).expect("read recovered settings");
        assert!(saved.contains(DEFAULT_HOTKEY));
    }

    #[test]
    fn save_to_path_writes_normalized_settings_atomically() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        let settings = Settings {
            hotkey: "   ".to_string(),
            sync: super::SyncSettings {
                device_name: "   ".to_string(),
                server_url: Some("   ".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        save_to_path(&path, &settings).expect("save settings");
        let saved = load_from_path(&path).expect("reload settings");

        assert_eq!(saved.hotkey, DEFAULT_HOTKEY);
        assert!(!saved.sync.device_name.trim().is_empty());
        assert_eq!(saved.sync.server_url, None);
        assert!(!path.with_file_name("settings.json.tmp").exists());
    }

    #[test]
    fn prepare_sync_settings_requires_server_url_when_enabled() {
        let err = prepare_sync_settings(true, " Laptop ", "   ", " secret ")
            .expect_err("missing server URL should fail");

        assert!(err.to_string().contains("Enter a sync server URL"));
    }

    #[test]
    fn prepare_sync_settings_trims_and_normalizes_values() {
        let prepared = prepare_sync_settings(
            false,
            "  Laptop  ",
            " sync.example.com ",
            "  secret-token  ",
        )
        .expect("prepare sync settings");

        assert_eq!(prepared.device_name, "Laptop");
        assert_eq!(
            prepared.server_url.as_deref(),
            Some("http://sync.example.com")
        );
        assert_eq!(prepared.auth_token.as_deref(), Some("secret-token"));
    }
}
