use crate::app_launcher;
use crate::database;
use crate::sync;
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
    static ref MONITOR_PAUSE_DEPTH: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    static ref WS_RE: Regex = Regex::new(r"\s+").expect("failed to compile whitespace regex");
    static ref LAST_PROGRAMMATIC_TEXT: Arc<Mutex<Option<(String, i64)>>> =
        Arc::new(Mutex::new(None));
    static ref LAST_PROGRAMMATIC_IMAGE: Arc<Mutex<Option<(blake3::Hash, i64)>>> =
        Arc::new(Mutex::new(None));
    static ref LAST_OBSERVED_TEXT: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    static ref LAST_OBSERVED_IMAGE: Arc<Mutex<Option<blake3::Hash>>> = Arc::new(Mutex::new(None));
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
        if let Ok(mut depth) = MONITOR_PAUSE_DEPTH.lock() {
            *depth += 1;
        }
        ClipboardMonitorPauseGuard
    }
}

impl Drop for ClipboardMonitorPauseGuard {
    fn drop(&mut self) {
        if let Ok(mut depth) = MONITOR_PAUSE_DEPTH.lock() {
            *depth = depth.saturating_sub(1);
        }
    }
}

pub async fn start_monitor() -> Result<()> {
    loop {
        sleep(Duration::from_millis(200)).await;
        if monitor_is_paused() {
            continue;
        }

        let mut clipboard = match Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(err) => {
                eprintln!("Failed to access clipboard: {err}");
                continue;
            }
        };

        if monitor_is_paused() {
            continue;
        }
        let app_name = app_launcher::get_frontmost_app_name();

        // Text capture
        if let Ok(content) = clipboard.get_text() {
            let changed = last_observed_text()
                .map(|last| last != content)
                .unwrap_or(true);
            if changed && !content.trim().is_empty() {
                if should_skip_programmatic_text(&content) {
                    set_last_observed_text(Some(content));
                    continue;
                }
                if should_skip_app(&app_name) {
                    set_last_observed_text(Some(content));
                    continue;
                }
                if let Err(e) = save_clipboard_entry(&content, &app_name).await {
                    eprintln!("Failed to save clipboard entry: {}", e);
                }
                set_last_observed_text(Some(content));
            }
        }
        // Image capture
        if let Ok(image) = clipboard.get_image() {
            let hash = blake3::hash(&image.bytes);
            let changed = last_observed_image_hash()
                .map(|last| last != hash)
                .unwrap_or(true);
            if changed {
                if should_skip_programmatic_image(&hash) {
                    set_last_observed_image_hash(Some(hash));
                    continue;
                }
                if should_skip_app(&app_name) {
                    set_last_observed_image_hash(Some(hash));
                    continue;
                }
                if let Err(e) = save_clipboard_image(&image, &app_name).await {
                    eprintln!("Failed to save clipboard image: {}", e);
                }
                set_last_observed_image_hash(Some(hash));
            }
        }
    }
}

fn monitor_is_paused() -> bool {
    MONITOR_PAUSE_DEPTH
        .lock()
        .map(|depth| *depth > 0)
        .unwrap_or(false)
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

fn last_observed_text() -> Option<String> {
    LAST_OBSERVED_TEXT
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

fn set_last_observed_text(value: Option<String>) {
    if let Ok(mut guard) = LAST_OBSERVED_TEXT.lock() {
        *guard = value;
    }
}

fn last_observed_image_hash() -> Option<blake3::Hash> {
    LAST_OBSERVED_IMAGE.lock().ok().and_then(|guard| *guard)
}

fn set_last_observed_image_hash(value: Option<blake3::Hash>) {
    if let Ok(mut guard) = LAST_OBSERVED_IMAGE.lock() {
        *guard = value;
    }
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
         WHERE deleted_at IS NULL
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
    let entry_id = conn.last_insert_rowid();
    if let Err(err) = sync::queue_local_clipboard_upsert(entry_id) {
        eprintln!("Failed to queue clipboard sync for text entry {entry_id}: {err:#}");
    }
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
    let entry_id = conn.last_insert_rowid();
    if let Err(err) = sync::queue_local_clipboard_upsert(entry_id) {
        eprintln!("Failed to queue clipboard sync for image entry {entry_id}: {err:#}");
    }
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
         WHERE deleted_at IS NULL
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

pub async fn delete_entry(id: i64) -> Result<()> {
    task::spawn_blocking(move || delete_entry_blocking(id))
        .await
        .map_err(|e| anyhow!("clipboard delete task failed: {e}"))?
}

pub async fn update_entry(id: i64, content: String, custom_name: Option<String>) -> Result<()> {
    task::spawn_blocking(move || update_entry_blocking(id, content, custom_name))
        .await
        .map_err(|e| anyhow!("clipboard update task failed: {e}"))?
}

pub async fn update_custom_name(id: i64, name: Option<String>) -> Result<()> {
    task::spawn_blocking(move || update_custom_name_blocking(id, name))
        .await
        .map_err(|e| anyhow!("clipboard update task failed: {e}"))?
}

fn search_history_blocking(query: &str) -> Result<Vec<ClipboardEntry>> {
    let conn = database::get_connection()?;
    let search_pattern = format!("%{}%", query);

    let mut stmt = conn.prepare(
        "SELECT id, content, content_type, app_name, timestamp, custom_name, is_favorite, is_pinned, image_width, image_height 
         FROM clipboard_history 
         WHERE deleted_at IS NULL
           AND (content LIKE ?1 OR custom_name LIKE ?1 OR app_name LIKE ?1)
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

fn delete_entry_blocking(id: i64) -> Result<()> {
    let conn = database::get_connection()?;
    let deleted_at = Utc::now().timestamp();
    let rows_updated = conn.execute(
        "UPDATE clipboard_history
         SET deleted_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        params![deleted_at, id],
    )?;
    if rows_updated > 0 {
        if let Err(err) = sync::queue_local_clipboard_delete(id) {
            eprintln!("Failed to queue clipboard delete sync for entry {id}: {err:#}");
        }
    }
    Ok(())
}

fn update_entry_blocking(id: i64, content: String, custom_name: Option<String>) -> Result<()> {
    let conn = database::get_connection()?;
    let rows_updated = conn.execute(
        "UPDATE clipboard_history SET content = ?1, custom_name = ?2 WHERE id = ?3",
        params![content, custom_name, id],
    )?;
    if rows_updated > 0 {
        if let Err(err) = sync::queue_local_clipboard_upsert(id) {
            eprintln!("Failed to queue clipboard sync for updated entry {id}: {err:#}");
        }
    }
    Ok(())
}

fn update_custom_name_blocking(id: i64, name: Option<String>) -> Result<()> {
    let conn = database::get_connection()?;
    let rows_updated = conn.execute(
        "UPDATE clipboard_history SET custom_name = ?1 WHERE id = ?2",
        params![name, id],
    )?;
    if rows_updated > 0 {
        if let Err(err) = sync::queue_local_clipboard_upsert(id) {
            eprintln!("Failed to queue clipboard sync for renamed entry {id}: {err:#}");
        }
    }
    Ok(())
}

pub async fn paste_to_active_app(
    content: &str,
    target_app: Option<app_launcher::FrontmostApp>,
) -> Result<()> {
    set_text_clipboard(content).await?;

    if let Some(app) = target_app.as_ref() {
        activate_app(app)?;
    }

    sleep(Duration::from_millis(150)).await;
    send_paste_keystroke()?;
    sleep(Duration::from_millis(200)).await;
    Ok(())
}

pub async fn restore_history_entry_to_clipboard(
    content: &str,
    content_type: &str,
    _image_width: Option<i64>,
    _image_height: Option<i64>,
) -> Result<()> {
    if content_type == "image" {
        set_image_clipboard(content).await?;
    } else {
        set_text_clipboard(content).await?;
    }
    Ok(())
}

pub async fn paste_history_entry(
    content: &str,
    content_type: &str,
    image_width: Option<i64>,
    image_height: Option<i64>,
    target_app: Option<app_launcher::FrontmostApp>,
) -> Result<()> {
    if content_type == "image" {
        let _ = (image_width, image_height);
        paste_image_to_active_app(content, target_app.as_ref()).await?;
    } else {
        restore_history_entry_to_clipboard(content, content_type, image_width, image_height)
            .await?;
        if let Some(app) = target_app.as_ref() {
            activate_app(app)?;
        }
        sleep(Duration::from_millis(150)).await;
        send_paste_keystroke()?;
    }
    sleep(Duration::from_millis(200)).await;
    Ok(())
}

async fn paste_image_to_active_app(
    content: &str,
    target_app: Option<&app_launcher::FrontmostApp>,
) -> Result<()> {
    set_image_clipboard(content).await?;

    if let Some(app) = target_app {
        activate_app(app)?;
    }

    sleep(Duration::from_millis(150)).await;
    send_paste_keystroke()?;
    sleep(Duration::from_millis(200)).await;
    Ok(())
}

async fn set_text_clipboard(content: &str) -> Result<()> {
    let _guard = ClipboardMonitorPauseGuard::new();
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(content)?;
    drop(clipboard);
    if let Ok(mut guard) = LAST_PROGRAMMATIC_TEXT.lock() {
        *guard = Some((normalize_text(content), Utc::now().timestamp()));
    }
    set_last_observed_text(Some(content.to_string()));
    wait_for_text_clipboard(content).await?;
    Ok(())
}

async fn set_image_clipboard(content: &str) -> Result<()> {
    let _guard = ClipboardMonitorPauseGuard::new();
    let image = decode_history_image(content)?;
    let hash = blake3::hash(&image.bytes);
    let mut clipboard = Clipboard::new()?;
    clipboard.set_image(image)?;
    drop(clipboard);
    if let Ok(mut guard) = LAST_PROGRAMMATIC_IMAGE.lock() {
        guard.replace((hash, Utc::now().timestamp()));
    }
    set_last_observed_image_hash(Some(hash));
    wait_for_image_clipboard(hash).await?;
    Ok(())
}

async fn wait_for_text_clipboard(expected: &str) -> Result<()> {
    let expected = normalize_text(expected);
    for _ in 0..20 {
        if let Ok(mut clipboard) = Clipboard::new() {
            if let Ok(content) = clipboard.get_text() {
                if normalize_text(&content) == expected {
                    return Ok(());
                }
            }
        }
        sleep(Duration::from_millis(25)).await;
    }

    Err(anyhow!("clipboard text did not update in time"))
}

async fn wait_for_image_clipboard(expected_hash: blake3::Hash) -> Result<()> {
    for _ in 0..20 {
        if let Ok(mut clipboard) = Clipboard::new() {
            if let Ok(image) = clipboard.get_image() {
                if blake3::hash(&image.bytes) == expected_hash {
                    return Ok(());
                }
            }
        }
        sleep(Duration::from_millis(25)).await;
    }

    Err(anyhow!("clipboard image did not update in time"))
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

fn activate_app(app: &app_launcher::FrontmostApp) -> Result<()> {
    app_launcher::activate_frontmost_app(app).context("failed to activate paste target")
}

fn send_paste_keystroke() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("osascript")
            .arg("-e")
            .arg(r#"tell application "System Events" to keystroke "v" using command down"#)
            .status()
            .context("failed to trigger paste keystroke")?;
        if status.success() {
            return Ok(());
        }
        Err(anyhow!("paste keystroke command failed"))
    }

    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('^v')",
            ])
            .status()
            .context("failed to trigger paste keystroke")?;
        if status.success() {
            return Ok(());
        }
        Err(anyhow!("paste keystroke command failed"))
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let status = std::process::Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+v"])
            .status()
            .context("failed to trigger paste keystroke")?;
        if status.success() {
            return Ok(());
        }
        Err(anyhow!("paste keystroke command failed"))
    }
}
