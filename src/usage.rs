use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct UsageEntry {
    last_used: i64,
    launch_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct UsageData {
    apps: HashMap<String, UsageEntry>,
}

lazy_static::lazy_static! {
    static ref USAGE: Mutex<UsageData> = Mutex::new(load_usage().unwrap_or_default());
}

fn usage_path() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("viceroy");
    let _ = fs::create_dir_all(&dir);
    dir.push("usage.json");
    dir
}

fn load_usage() -> Result<UsageData> {
    let path = usage_path();
    if !path.exists() {
        return Ok(UsageData::default());
    }
    let data = fs::read_to_string(path)?;
    let parsed: UsageData = serde_json::from_str(&data)?;
    Ok(parsed)
}

fn save_usage(data: &UsageData) -> Result<()> {
    let path = usage_path();
    let json = serde_json::to_string_pretty(data)?;
    fs::write(path, json)?;
    Ok(())
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn record_app_launch(path: &str) {
    let mut usage = USAGE.lock().unwrap();
    let entry = usage.apps.entry(path.to_string()).or_default();
    entry.last_used = now_ts();
    entry.launch_count = entry.launch_count.saturating_add(1);
    let _ = save_usage(&usage);
}

pub fn get_app_usage(path: &str) -> Option<(i64, u32)> {
    let usage = USAGE.lock().unwrap();
    usage
        .apps
        .get(path)
        .map(|e| (e.last_used, e.launch_count))
}
