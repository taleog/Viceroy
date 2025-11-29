use crate::app_launcher;
use crate::database;
use anyhow::{anyhow, Context, Result};
use arboard::{Clipboard, ImageData};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::Utc;
use lazy_static::lazy_static;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

lazy_static! {
    static ref MONITOR_PAUSED: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
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

async fn save_clipboard_entry(content: &str, app_name: &Option<String>) -> Result<()> {
    let conn = database::get_connection()?;
    let timestamp = Utc::now().timestamp();
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
    // First, copy to clipboard
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(content)?;
    // Small delay to ensure clipboard is updated
    sleep(Duration::from_millis(50)).await;
    send_paste_keystroke()?;
    Ok(())
}

pub async fn paste_history_entry(
    content: &str,
    content_type: &str,
    _image_width: Option<i64>,
    _image_height: Option<i64>,
) -> Result<()> {
    if content_type == "image" {
        paste_image_to_active_app(content).await
    } else {
        paste_to_active_app(content).await
    }
}

async fn paste_image_to_active_app(content: &str) -> Result<()> {
    let image = decode_history_image(content)?;
    let mut clipboard = Clipboard::new()?;
    clipboard.set_image(image)?;
    sleep(Duration::from_millis(60)).await;
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
