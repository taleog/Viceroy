use crate::app_launcher;
use crate::database;
use anyhow::{anyhow, Context, Result};
use arboard::{Clipboard, ImageData};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::Utc;
use lazy_static::lazy_static;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use tokio::task;
use tokio::time::{sleep, Duration};

lazy_static! {
    static ref MONITOR_PAUSED: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref WS_RE: Regex = Regex::new(r"\s+").expect("failed to compile whitespace regex");
    static ref LAST_PROGRAMMATIC_TEXT: Arc<Mutex<Option<(String, i64)>>> =
        Arc::new(Mutex::new(None));
    static ref LAST_PROGRAMMATIC_IMAGE: Arc<Mutex<Option<(blake3::Hash, i64)>>> =
        Arc::new(Mutex::new(None));
}

const PASSWORD_MANAGERS: &[&str] = &[
    "Keychain Access",
    "1Password",
    "Bitwarden",
    "LastPass",
    "Dashlane",
    "KeePassXC",
    "Enpass",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content: String,
    pub content_type: String,
    pub app_name: Option<String>,
    pub timestamp: i64,
    pub custom_name: Option<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
}

struct ClipboardMonitorPauseGuard;

impl ClipboardMonitorPauseGuard {
    fn new() -> Self {
        if let Ok(mut paused) = MONITOR_PAUSED.lock() {
            *paused = true;
        }
        ClipboardMonitorPauseGuard
    }
}

impl Drop for ClipboardMonitorPauseGuard {
    fn drop(&mut self) {
        if let Ok(mut paused) = MONITOR_PAUSED.lock() {
            *paused = false;
        }
    }
}

pub async fn start_monitor() -> Result<()> {
    let mut clipboard = Clipboard::new()?;
    let mut last_text = String::new();
    let mut last_image_hash: Option<blake3::Hash> = None;

    loop {
        sleep(Duration::from_millis(200)).await;
        if *MONITOR_PAUSED.lock().unwrap() {
            continue;
        }
        let app_name = app_launcher::get_frontmost_app_name();

        // Text capture
        if let Ok(content) = clipboard.get_text() {
            if content != last_text && !content.trim().is_empty() {
                if should_skip_programmatic_text(&content) {
                    last_text = content;
                    continue;
                }
                if should_skip_app(&app_name) {
                    last_text = content;
                    continue;
                }
                if let Err(e) = save_clipboard_entry(&content, &app_name).await {
                    eprintln!("Failed to save clipboard entry: {}", e);
                }
                last_text = content;
            }
        }
        // Image capture
        if let Ok(image) = clipboard.get_image() {
            let hash = blake3::hash(&image.bytes);
            let changed = last_image_hash.map(|h| h != hash).unwrap_or(true);
            if changed {
                if should_skip_programmatic_image(&hash) {
                    last_image_hash = Some(hash);
                    continue;
                }
                if should_skip_app(&app_name) {
                    last_image_hash = Some(hash);
                    continue;
                }
                if let Err(e) = save_clipboard_image(&image, &app_name).await {
                    eprintln!("Failed to save clipboard image: {}", e);
                }
                last_image_hash = Some(hash);
            }
        }
    }
}

fn should_skip_app(app_name: &Option<String>) -> bool {
    if let Some(ref app) = app_name {
        PASSWORD_MANAGERS.iter().any(|pm| app.contains(pm))
    } else {
        false
    }
}

fn normalize_text(content: &str) -> String {
    let trimmed = content.trim();
    WS_RE.replace_all(trimmed, " ").to_string()
}

fn should_skip_programmatic_text(content: &str) -> bool {
    if let Ok(lock) = LAST_PROGRAMMATIC_TEXT.lock() {
        if let Some((last, ts)) = &*lock {
            let now = Utc::now().timestamp();
            let age = now.saturating_sub(*ts);
            // Extended window: 10 seconds to catch programmatic writes more reliably
            return age <= 10 && normalize_text(content) == *last;
        }
    }
    false
}

fn should_skip_programmatic_image(hash: &blake3::Hash) -> bool {
    if let Ok(lock) = LAST_PROGRAMMATIC_IMAGE.lock() {
        if let Some((last_hash, ts)) = &*lock {
            let now = Utc::now().timestamp();
            let age = now.saturating_sub(*ts);
            // Extended window: 10 seconds to catch programmatic writes more reliably
            return age <= 10 && last_hash == hash;
        }
    }
    false
}

fn should_skip_duplicate_text(
    conn: &rusqlite::Connection,
    content: &str,
    app_name: &Option<String>,
    timestamp: i64,
) -> Result<bool> {
    let normalized_new = normalize_text(content);
    // Check against last 5 entries to catch duplicates more reliably
    let mut stmt = conn.prepare(
        "SELECT content, content_type, app_name, timestamp 
         FROM clipboard_history 
         ORDER BY id DESC 
         LIMIT 5",
    )?;

    let previous_entries: Vec<(String, String, Option<String>, i64)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (prev_content, prev_type, prev_app, prev_ts) in previous_entries {
        if prev_type != "text" {
            continue;
        }
        let normalized_prev = normalize_text(&prev_content);
        let _same_app = prev_app == *app_name;
        let close_in_time = timestamp.saturating_sub(prev_ts) <= 120; // 2-minute window
                                                                      // Check both same-app and app-agnostic duplicates
        if normalized_prev == normalized_new && close_in_time {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn save_clipboard_entry(content: &str, app_name: &Option<String>) -> Result<()> {
    let conn = database::get_connection()?;
    let timestamp = Utc::now().timestamp();

    if should_skip_duplicate_text(&conn, content, app_name, timestamp)? {
        return Ok(());
    }

    conn.execute(
        "INSERT INTO clipboard_history (content, content_type, app_name, timestamp, image_width, image_height) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            content,
            "text",
            app_name,
            timestamp,
            Option::<i64>::None,
            Option::<i64>::None
        ],
    )?;
    prune_old(&conn)?;
    Ok(())
}

async fn save_clipboard_image(image: &ImageData<'_>, app_name: &Option<String>) -> Result<()> {
    let conn = database::get_connection()?;
    let timestamp = Utc::now().timestamp();
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let mut encoder =
            png::Encoder::new(&mut png_bytes, image.width as u32, image.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&image.bytes)?;
    }
    let b64 = STANDARD.encode(&png_bytes);
    conn.execute(
        "INSERT INTO clipboard_history (content, content_type, app_name, timestamp, image_width, image_height) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            b64,
            "image",
            app_name,
            timestamp,
            Some(image.width as i64),
            Some(image.height as i64)
        ],
    )?;
    prune_old(&conn)?;
    Ok(())
}

fn prune_old(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM clipboard_history WHERE id NOT IN (SELECT id FROM clipboard_history ORDER BY timestamp DESC LIMIT 1000)",
        [],
    )?;
    Ok(())
}

pub async fn get_history(limit: usize) -> Result<Vec<ClipboardEntry>> {
    let conn = database::get_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, content, content_type, app_name, timestamp, custom_name, is_favorite, is_pinned, image_width, image_height 
         FROM clipboard_history 
         ORDER BY is_pinned DESC, timestamp DESC 
         LIMIT ?1"
    )?;

    let entries = stmt
        .query_map([limit], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content: row.get(1)?,
                content_type: row.get(2)?,
                app_name: row.get(3)?,
                timestamp: row.get(4)?,
                custom_name: row.get(5)?,
                is_favorite: row.get::<_, i64>(6)? == 1,
                is_pinned: row.get::<_, i64>(7)? == 1,
                image_width: row.get::<_, Option<i64>>(8)?,
                image_height: row.get::<_, Option<i64>>(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(entries)
}

pub async fn search_history(query: &str) -> Result<Vec<ClipboardEntry>> {
    let query = query.to_string();
    task::spawn_blocking(move || search_history_blocking(&query))
        .await
        .map_err(|e| anyhow!("clipboard search task failed: {e}"))?
}

fn search_history_blocking(query: &str) -> Result<Vec<ClipboardEntry>> {
    let conn = database::get_connection()?;
    let search_pattern = format!("%{}%", query);

    let mut stmt = conn.prepare(
        "SELECT id, content, content_type, app_name, timestamp, custom_name, is_favorite, is_pinned, image_width, image_height 
         FROM clipboard_history 
         WHERE content LIKE ?1 OR custom_name LIKE ?1 OR app_name LIKE ?1
         ORDER BY is_pinned DESC, timestamp DESC 
         LIMIT 100"
    )?;

    let entries = stmt
        .query_map([&search_pattern], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content: row.get(1)?,
                content_type: row.get(2)?,
                app_name: row.get(3)?,
                timestamp: row.get(4)?,
                custom_name: row.get(5)?,
                is_favorite: row.get::<_, i64>(6)? == 1,
                is_pinned: row.get::<_, i64>(7)? == 1,
                image_width: row.get::<_, Option<i64>>(8)?,
                image_height: row.get::<_, Option<i64>>(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(entries)
}

pub async fn paste_to_active_app(content: &str) -> Result<()> {
    let _guard = ClipboardMonitorPauseGuard::new();
    // Store the frontmost app BEFORE modifying clipboard
    let previous_app = app_launcher::get_frontmost_app_name();

    // Copy to clipboard
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(content)?;
    if let Ok(mut guard) = LAST_PROGRAMMATIC_TEXT.lock() {
        *guard = Some((normalize_text(content), Utc::now().timestamp()));
    }

    // Delay to ensure clipboard is updated
    sleep(Duration::from_millis(100)).await;

    // Explicitly activate the previous app to ensure focus is correct before paste
    if let Some(app_name) = previous_app {
        activate_app(&app_name)?;
    }

    // Longer pause to ensure focus is returned and app is ready
    sleep(Duration::from_millis(200)).await;
    send_paste_keystroke()?;
    Ok(())
}

pub async fn paste_history_entry(
    content: &str,
    content_type: &str,
    _image_width: Option<i64>,
    _image_height: Option<i64>,
) -> Result<()> {
    // Pause monitor so we don't re-ingest the same entry after paste.
    let _guard = ClipboardMonitorPauseGuard::new();
    if content_type == "image" {
        paste_image_to_active_app(content).await?;
    } else {
        paste_to_active_app(content).await?;
    }
    // Hold pause longer to ensure monitor doesn't capture the paste result
    sleep(Duration::from_millis(300)).await;
    Ok(())
}

async fn paste_image_to_active_app(content: &str) -> Result<()> {
    let _guard = ClipboardMonitorPauseGuard::new();
    // Store the frontmost app BEFORE modifying clipboard
    let previous_app = app_launcher::get_frontmost_app_name();

    let image = decode_history_image(content)?;
    let mut clipboard = Clipboard::new()?;
    let hash = blake3::hash(&image.bytes);
    clipboard.set_image(image)?;
    if let Ok(mut guard) = LAST_PROGRAMMATIC_IMAGE.lock() {
        guard.replace((hash, Utc::now().timestamp()));
    }

    // Delay to ensure clipboard is updated
    sleep(Duration::from_millis(100)).await;

    // Explicitly activate the previous app
    if let Some(app_name) = previous_app {
        activate_app(&app_name)?;
    }

    // Longer pause to ensure focus is returned
    sleep(Duration::from_millis(200)).await;
    send_paste_keystroke()?;
    Ok(())
}

fn decode_history_image(content: &str) -> Result<ImageData<'static>> {
    let png_bytes = STANDARD
        .decode(content)
        .context("failed to decode base64 clipboard image")?;
    let cursor = Cursor::new(&png_bytes);
    let decoder = png::Decoder::new(cursor);
    let mut reader = decoder
        .read_info()
        .context("failed to read clipboard image header")?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .context("failed to decode clipboard image data")?;
    buf.truncate(info.buffer_size());
    Ok(ImageData {
        width: info.width as usize,
        height: info.height as usize,
        bytes: Cow::Owned(buf),
    })
}

fn activate_app(app_name: &str) -> Result<()> {
    let script = format!(
        r#"tell application "{}" to activate"#,
        app_name.replace('\"', "\\\"")
    );
    let status = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .context("failed to activate app")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("failed to activate app: {}", app_name))
    }
}

fn send_paste_keystroke() -> Result<()> {
    let status = std::process::Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to keystroke "v" using command down"#)
        .status()
        .context("failed to trigger paste keystroke")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("paste keystroke command failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test normalize_text function
    #[test]
    fn test_normalize_text_trims_whitespace() {
        assert_eq!(normalize_text("  hello  "), "hello");
        assert_eq!(normalize_text("\nhello\n"), "hello");
        assert_eq!(normalize_text("\t\nhello\t\n"), "hello");
    }

    #[test]
    fn test_normalize_text_collapses_multiple_spaces() {
        assert_eq!(normalize_text("hello   world"), "hello world");
        assert_eq!(normalize_text("hello\t\tworld"), "hello world");
        assert_eq!(normalize_text("hello\n\nworld"), "hello world");
        assert_eq!(normalize_text("hello  \t  \n  world"), "hello world");
    }

    #[test]
    fn test_normalize_text_handles_empty_string() {
        assert_eq!(normalize_text(""), "");
        assert_eq!(normalize_text("   "), "");
    }

    #[test]
    fn test_normalize_text_preserves_single_spaces() {
        assert_eq!(normalize_text("hello world"), "hello world");
        assert_eq!(normalize_text("one two three"), "one two three");
    }

    // Test should_skip_app function
    #[test]
    fn test_should_skip_app_detects_password_managers() {
        // Test known password managers
        assert!(should_skip_app(&Some("Keychain Access".to_string())));
        assert!(should_skip_app(&Some("1Password".to_string())));
        assert!(should_skip_app(&Some("Bitwarden".to_string())));
        assert!(should_skip_app(&Some("LastPass".to_string())));
        assert!(should_skip_app(&Some("Dashlane".to_string())));
        assert!(should_skip_app(&Some("KeePassXC".to_string())));
        assert!(should_skip_app(&Some("Enpass".to_string())));
    }

    #[test]
    fn test_should_skip_app_allows_regular_apps() {
        assert!(!should_skip_app(&Some("Safari".to_string())));
        assert!(!should_skip_app(&Some("Chrome".to_string())));
        assert!(!should_skip_app(&Some("VSCode".to_string())));
        assert!(!should_skip_app(&Some("Terminal".to_string())));
        assert!(!should_skip_app(&Some("Finder".to_string())));
    }

    #[test]
    fn test_should_skip_app_handles_none() {
        assert!(!should_skip_app(&None));
    }

    #[test]
    fn test_should_skip_app_partial_matches() {
        // Password manager names as part of longer strings should still match
        assert!(should_skip_app(&Some("1Password 7".to_string())));
        assert!(should_skip_app(&Some("Bitwarden Desktop".to_string())));
        assert!(should_skip_app(&Some("KeePassXC Browser".to_string())));
    }

    // Test ClipboardEntry struct
    #[test]
    fn test_clipboard_entry_default_values() {
        let entry = ClipboardEntry {
            id: 1,
            content: "test".to_string(),
            content_type: "text".to_string(),
            app_name: None,
            timestamp: 1234567890,
            custom_name: None,
            is_favorite: false,
            is_pinned: false,
            image_width: None,
            image_height: None,
        };
        assert_eq!(entry.id, 1);
        assert_eq!(entry.content, "test");
        assert_eq!(entry.content_type, "text");
        assert!(entry.app_name.is_none());
        assert!(!entry.is_favorite);
        assert!(!entry.is_pinned);
    }

    #[test]
    fn test_clipboard_entry_image_type() {
        let entry = ClipboardEntry {
            id: 2,
            content: "base64data".to_string(),
            content_type: "image".to_string(),
            app_name: Some("Preview".to_string()),
            timestamp: 1234567890,
            custom_name: Some("My Screenshot".to_string()),
            is_favorite: true,
            is_pinned: true,
            image_width: Some(1920),
            image_height: Some(1080),
        };
        assert_eq!(entry.content_type, "image");
        assert_eq!(entry.app_name.as_deref(), Some("Preview"));
        assert_eq!(entry.custom_name.as_deref(), Some("My Screenshot"));
        assert!(entry.is_favorite);
        assert!(entry.is_pinned);
        assert_eq!(entry.image_width, Some(1920));
        assert_eq!(entry.image_height, Some(1080));
    }

    #[test]
    fn test_clipboard_entry_serialization() {
        let entry = ClipboardEntry {
            id: 1,
            content: "hello".to_string(),
            content_type: "text".to_string(),
            app_name: Some("Safari".to_string()),
            timestamp: 1234567890,
            custom_name: None,
            is_favorite: false,
            is_pinned: false,
            image_width: None,
            image_height: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: ClipboardEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry.id, deserialized.id);
        assert_eq!(entry.content, deserialized.content);
        assert_eq!(entry.content_type, deserialized.content_type);
        assert_eq!(entry.app_name, deserialized.app_name);
    }

    // Test PASSWORD_MANAGERS constant
    #[test]
    fn test_password_managers_list_contains_expected_apps() {
        assert!(PASSWORD_MANAGERS.contains(&"Keychain Access"));
        assert!(PASSWORD_MANAGERS.contains(&"1Password"));
        assert!(PASSWORD_MANAGERS.contains(&"Bitwarden"));
        assert!(PASSWORD_MANAGERS.contains(&"LastPass"));
        assert!(PASSWORD_MANAGERS.contains(&"Dashlane"));
        assert!(PASSWORD_MANAGERS.contains(&"KeePassXC"));
        assert!(PASSWORD_MANAGERS.contains(&"Enpass"));
    }
}
