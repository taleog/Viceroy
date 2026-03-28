use crate::{database, settings};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use reqwest::Url;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::Notify;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, AUTHORIZATION};
use tokio_tungstenite::tungstenite::Message;

lazy_static! {
    static ref SYNC_NOTIFY: Arc<Notify> = Arc::new(Notify::new());
}

static SYNC_WORKER_STARTED: AtomicBool = AtomicBool::new(false);

const OUTBOX_BATCH_SIZE: usize = 100;
const RECONNECT_DELAY_SECONDS: u64 = 5;
const CURSOR_KEY: &str = "clipboard_cursor";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDevice {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncOperationKind {
    UpsertClipboardEntry,
    DeleteClipboardEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardSyncRecord {
    pub sync_id: String,
    pub content: String,
    pub content_type: String,
    pub app_name: Option<String>,
    pub timestamp: i64,
    pub custom_name: Option<String>,
    pub is_favorite: bool,
    pub is_pinned: bool,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub source_device_id: String,
    pub source_device_name: String,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSyncOperation {
    pub id: i64,
    pub entry_sync_id: String,
    pub operation: SyncOperationKind,
    pub payload: ClipboardSyncRecord,
    pub created_at: i64,
    pub attempts: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub device: LocalDevice,
    pub pending_operations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEnvelope {
    pub operation: SyncOperationKind,
    pub record: ClipboardSyncRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushEventsRequest {
    pub device: LocalDevice,
    pub events: Vec<SyncEnvelope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatchUpResponse {
    pub cursor: Option<i64>,
    pub events: Vec<SyncEnvelope>,
}

#[derive(Debug, Clone)]
struct SyncRuntimeConfig {
    device: LocalDevice,
    server_url: String,
    auth_token: Option<String>,
}

pub fn init() -> Result<SyncStatus> {
    let conn = database::get_connection()?;
    let device = ensure_local_device(&conn)?;
    status_with_connection(&conn, device)
}

pub fn start_background_worker() -> Result<()> {
    if SYNC_WORKER_STARTED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let Some(config) = runtime_config()? else {
        return Ok(());
    };

    thread::spawn(move || {
        let runtime = Runtime::new().expect("failed to create sync runtime");
        runtime.block_on(async move {
            if let Err(err) = run_background_loop(config).await {
                log::error!("Sync worker exited: {err:#}");
            }
        });
    });

    Ok(())
}

pub fn status() -> Result<SyncStatus> {
    let conn = database::get_connection()?;
    let device = ensure_local_device(&conn)?;
    status_with_connection(&conn, device)
}

pub fn queue_local_clipboard_upsert(entry_id: i64) -> Result<()> {
    let conn = database::get_connection()?;
    let device = ensure_local_device(&conn)?;
    let now = now_ts();
    let sync_id = ensure_entry_identity(&conn, entry_id, &device, now)?;
    conn.execute(
        "UPDATE clipboard_history
         SET source_device_id = ?1,
             source_device_name = ?2,
             updated_at = ?3,
             deleted_at = NULL
         WHERE id = ?4",
        params![device.device_id, device.device_name, now, entry_id],
    )?;
    let payload = load_clipboard_payload(&conn, entry_id)
        .with_context(|| format!("failed to load clipboard payload for {sync_id}"))?;
    enqueue_operation(
        &conn,
        &sync_id,
        SyncOperationKind::UpsertClipboardEntry,
        &payload,
        now,
    )?;
    notify_worker();
    Ok(())
}

pub fn queue_local_clipboard_delete(entry_id: i64) -> Result<()> {
    let conn = database::get_connection()?;
    let device = ensure_local_device(&conn)?;
    let now = now_ts();
    let sync_id = ensure_entry_identity(&conn, entry_id, &device, now)?;
    conn.execute(
        "UPDATE clipboard_history
         SET source_device_id = ?1,
             source_device_name = ?2,
             updated_at = ?3,
             deleted_at = COALESCE(deleted_at, ?3)
         WHERE id = ?4",
        params![device.device_id, device.device_name, now, entry_id],
    )?;
    let payload = load_clipboard_payload(&conn, entry_id)
        .with_context(|| format!("failed to load clipboard delete payload for {sync_id}"))?;
    enqueue_operation(
        &conn,
        &sync_id,
        SyncOperationKind::DeleteClipboardEntry,
        &payload,
        now,
    )?;
    notify_worker();
    Ok(())
}

pub fn pending_operations(limit: usize) -> Result<Vec<PendingSyncOperation>> {
    let conn = database::get_connection()?;
    let mut stmt = conn.prepare(
        "SELECT id, entry_sync_id, operation, payload, created_at, attempts, last_error
         FROM sync_outbox
         WHERE sent_at IS NULL
         ORDER BY created_at ASC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map([limit as i64], |row| {
        let operation = parse_operation(&row.get::<_, String>(2)?);
        let payload: ClipboardSyncRecord = serde_json::from_str(&row.get::<_, String>(3)?)
            .map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
        Ok(PendingSyncOperation {
            id: row.get(0)?,
            entry_sync_id: row.get(1)?,
            operation,
            payload,
            created_at: row.get(4)?,
            attempts: row.get::<_, i64>(5)? as u32,
            last_error: row.get(6)?,
        })
    })?;

    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn mark_operation_sent(operation_id: i64) -> Result<()> {
    let conn = database::get_connection()?;
    conn.execute(
        "UPDATE sync_outbox SET sent_at = ?1, last_error = NULL WHERE id = ?2",
        params![now_ts(), operation_id],
    )?;
    Ok(())
}

pub fn mark_operation_failed(operation_id: i64, error: &str) -> Result<()> {
    let conn = database::get_connection()?;
    conn.execute(
        "UPDATE sync_outbox
         SET attempts = attempts + 1,
             last_error = ?1
         WHERE id = ?2",
        params![error, operation_id],
    )?;
    Ok(())
}

pub fn apply_remote_clipboard_record(record: &ClipboardSyncRecord) -> Result<i64> {
    let conn = database::get_connection()?;
    let device = ensure_local_device(&conn)?;
    if record.source_device_id == device.device_id {
        return Ok(0);
    }

    let existing = conn
        .query_row(
            "SELECT id, updated_at FROM clipboard_history WHERE sync_id = ?1",
            params![record.sync_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .optional()?;

    if let Some((id, Some(existing_updated_at))) = existing {
        if existing_updated_at > record.updated_at {
            return Ok(id);
        }

        conn.execute(
            "UPDATE clipboard_history
             SET content = ?1,
                 content_type = ?2,
                 app_name = ?3,
                 timestamp = ?4,
                 custom_name = ?5,
                 is_favorite = ?6,
                 is_pinned = ?7,
                 image_width = ?8,
                 image_height = ?9,
                 source_device_id = ?10,
                 source_device_name = ?11,
                 updated_at = ?12,
                 deleted_at = ?13
             WHERE id = ?14",
            params![
                record.content,
                record.content_type,
                record.app_name,
                record.timestamp,
                record.custom_name,
                record.is_favorite as i64,
                record.is_pinned as i64,
                record.image_width,
                record.image_height,
                record.source_device_id,
                record.source_device_name,
                record.updated_at,
                record.deleted_at,
                id
            ],
        )?;
        return Ok(id);
    }

    conn.execute(
        "INSERT INTO clipboard_history (
            content,
            content_type,
            app_name,
            timestamp,
            custom_name,
            image_width,
            image_height,
            is_favorite,
            is_pinned,
            sync_id,
            source_device_id,
            source_device_name,
            updated_at,
            deleted_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            record.content,
            record.content_type,
            record.app_name,
            record.timestamp,
            record.custom_name,
            record.image_width,
            record.image_height,
            record.is_favorite as i64,
            record.is_pinned as i64,
            record.sync_id,
            record.source_device_id,
            record.source_device_name,
            record.updated_at,
            record.deleted_at
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

fn status_with_connection(conn: &Connection, device: LocalDevice) -> Result<SyncStatus> {
    let pending_operations: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sync_outbox WHERE sent_at IS NULL",
        [],
        |row| row.get(0),
    )?;

    Ok(SyncStatus {
        device,
        pending_operations: pending_operations as usize,
    })
}

async fn run_background_loop(config: SyncRuntimeConfig) -> Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("failed to create sync HTTP client")?;

    loop {
        if let Err(err) = run_sync_session(&client, &config).await {
            log::error!("Sync session failed: {err:#}");
        }
        sleep(Duration::from_secs(RECONNECT_DELAY_SECONDS)).await;
    }
}

async fn run_sync_session(client: &reqwest::Client, config: &SyncRuntimeConfig) -> Result<()> {
    catch_up_once(client, config).await?;
    process_outbox(client, config).await?;

    let ws_url = websocket_url(&config.server_url, &config.device)?;
    let mut ws_request = ws_url
        .as_str()
        .into_client_request()
        .context("failed to build websocket request")?;
    if let Some(token) = &config.auth_token {
        ws_request.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .context("invalid sync auth token header")?,
        );
    }

    let (ws_stream, _) = connect_async(ws_request)
        .await
        .context("failed to connect websocket")?;
    let (_write, mut read) = ws_stream.split();

    loop {
        tokio::select! {
            _ = SYNC_NOTIFY.notified() => {
                process_outbox(client, config).await?;
            }
            next_message = read.next() => {
                match next_message {
                    Some(Ok(message)) => {
                        if let Some(cursor) = handle_ws_message(message).await? {
                            update_cursor(cursor)?;
                        }
                    }
                    Some(Err(err)) => return Err(err).context("websocket receive failed"),
                    None => return Err(anyhow!("websocket closed")),
                }
            }
        }
    }
}

async fn process_outbox(client: &reqwest::Client, config: &SyncRuntimeConfig) -> Result<()> {
    let pending = pending_operations(OUTBOX_BATCH_SIZE)?;
    if pending.is_empty() {
        return Ok(());
    }

    let request = PushEventsRequest {
        device: config.device.clone(),
        events: pending
            .iter()
            .map(|operation| SyncEnvelope {
                operation: operation.operation.clone(),
                record: operation.payload.clone(),
            })
            .collect(),
    };

    let url = api_base_url(&config.server_url)?.join("clipboard/events")?;
    let response = authorized_request(client.post(url), config)
        .json(&request)
        .send()
        .await;

    match response {
        Ok(response) => {
            response
                .error_for_status()
                .context("sync event upload failed")?;
            for operation in pending {
                mark_operation_sent(operation.id)?;
            }
            Ok(())
        }
        Err(err) => {
            let message = format!("{err:#}");
            for operation in pending {
                let _ = mark_operation_failed(operation.id, &message);
            }
            Err(err).context("failed to upload pending sync events")
        }
    }
}

async fn catch_up_once(client: &reqwest::Client, config: &SyncRuntimeConfig) -> Result<()> {
    let cursor = current_cursor()?.unwrap_or(0);
    let mut url = api_base_url(&config.server_url)?.join("clipboard/changes")?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("since", &cursor.to_string());
        pairs.append_pair("device_id", &config.device.device_id);
        pairs.append_pair("limit", &OUTBOX_BATCH_SIZE.to_string());
    }

    let response = authorized_request(client.get(url), config)
        .send()
        .await
        .context("failed to request catch-up sync")?
        .error_for_status()
        .context("catch-up sync request failed")?;

    let body: CatchUpResponse = response
        .json()
        .await
        .context("failed to parse catch-up sync response")?;

    let mut newest_cursor = body.cursor.unwrap_or(cursor);
    for event in body.events {
        apply_remote_clipboard_record(&event.record)?;
        newest_cursor = newest_cursor.max(event.record.updated_at);
    }
    update_cursor(newest_cursor)?;
    Ok(())
}

async fn handle_ws_message(message: Message) -> Result<Option<i64>> {
    match message {
        Message::Text(text) => {
            let event: SyncEnvelope =
                serde_json::from_str(&text).context("failed to decode websocket sync payload")?;
            apply_remote_clipboard_record(&event.record)?;
            Ok(Some(event.record.updated_at))
        }
        Message::Binary(bytes) => {
            let event: SyncEnvelope = serde_json::from_slice(&bytes)
                .context("failed to decode binary websocket sync payload")?;
            apply_remote_clipboard_record(&event.record)?;
            Ok(Some(event.record.updated_at))
        }
        Message::Close(_) => Err(anyhow!("websocket closed by remote peer")),
        _ => Ok(None),
    }
}

fn runtime_config() -> Result<Option<SyncRuntimeConfig>> {
    let conn = database::get_connection()?;
    let device = ensure_local_device(&conn)?;
    let app_settings = settings::load()?;
    let Some(server_url) = app_settings
        .sync
        .server_url
        .clone()
        .filter(|url| !url.trim().is_empty())
    else {
        return Ok(None);
    };

    if !app_settings.sync.enabled {
        return Ok(None);
    }

    Ok(Some(SyncRuntimeConfig {
        device,
        server_url,
        auth_token: app_settings.sync.auth_token.clone(),
    }))
}

fn ensure_local_device(conn: &Connection) -> Result<LocalDevice> {
    let mut app_settings = settings::load()?;
    let mut changed = false;

    if app_settings.sync.device_name.trim().is_empty() {
        app_settings.sync.device_name = default_device_name();
        changed = true;
    }
    if app_settings.sync.device_id.trim().is_empty() {
        app_settings.sync.device_id = generate_device_id(&app_settings.sync.device_name);
        changed = true;
    }
    if changed {
        settings::save(&app_settings)?;
    }

    let device = LocalDevice {
        device_id: app_settings.sync.device_id.clone(),
        device_name: app_settings.sync.device_name.clone(),
        platform: default_platform().to_string(),
    };

    let now = now_ts();
    conn.execute(
        "UPDATE sync_devices SET is_current = 0 WHERE is_current = 1",
        [],
    )?;
    conn.execute(
        "INSERT INTO sync_devices (
            device_id,
            device_name,
            platform,
            is_current,
            first_seen_at,
            last_seen_at
        ) VALUES (?1, ?2, ?3, 1, ?4, ?4)
        ON CONFLICT(device_id) DO UPDATE SET
            device_name = excluded.device_name,
            platform = excluded.platform,
            is_current = 1,
            last_seen_at = excluded.last_seen_at",
        params![device.device_id, device.device_name, device.platform, now],
    )?;

    Ok(device)
}

fn ensure_entry_identity(
    conn: &Connection,
    entry_id: i64,
    device: &LocalDevice,
    updated_at: i64,
) -> Result<String> {
    let existing = conn
        .query_row(
            "SELECT sync_id FROM clipboard_history WHERE id = ?1",
            params![entry_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?;

    let sync_id = existing
        .flatten()
        .unwrap_or_else(|| generate_entry_sync_id(entry_id, device, updated_at));

    conn.execute(
        "UPDATE clipboard_history
         SET sync_id = ?1,
             source_device_id = COALESCE(source_device_id, ?2),
             source_device_name = COALESCE(source_device_name, ?3),
             updated_at = COALESCE(updated_at, ?4)
         WHERE id = ?5",
        params![
            sync_id,
            device.device_id,
            device.device_name,
            updated_at,
            entry_id
        ],
    )?;

    Ok(sync_id)
}

fn enqueue_operation(
    conn: &Connection,
    sync_id: &str,
    operation: SyncOperationKind,
    payload: &ClipboardSyncRecord,
    created_at: i64,
) -> Result<()> {
    conn.execute(
        "DELETE FROM sync_outbox WHERE entry_sync_id = ?1 AND sent_at IS NULL",
        params![sync_id],
    )?;

    conn.execute(
        "INSERT INTO sync_outbox (entry_sync_id, operation, payload, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            sync_id,
            operation_name(&operation),
            serde_json::to_string(payload)?,
            created_at
        ],
    )?;
    Ok(())
}

fn load_clipboard_payload(conn: &Connection, entry_id: i64) -> Result<ClipboardSyncRecord> {
    conn.query_row(
        "SELECT
            sync_id,
            content,
            content_type,
            app_name,
            timestamp,
            custom_name,
            is_favorite,
            is_pinned,
            image_width,
            image_height,
            source_device_id,
            source_device_name,
            updated_at,
            deleted_at
         FROM clipboard_history
         WHERE id = ?1",
        params![entry_id],
        |row| {
            Ok(ClipboardSyncRecord {
                sync_id: row.get(0)?,
                content: row.get(1)?,
                content_type: row.get(2)?,
                app_name: row.get(3)?,
                timestamp: row.get(4)?,
                custom_name: row.get(5)?,
                is_favorite: row.get::<_, i64>(6)? == 1,
                is_pinned: row.get::<_, i64>(7)? == 1,
                image_width: row.get(8)?,
                image_height: row.get(9)?,
                source_device_id: row.get(10)?,
                source_device_name: row.get(11)?,
                updated_at: row.get::<_, Option<i64>>(12)?.unwrap_or_default(),
                deleted_at: row.get(13)?,
            })
        },
    )
    .context("failed to load clipboard sync payload")
}

fn authorized_request(
    builder: reqwest::RequestBuilder,
    config: &SyncRuntimeConfig,
) -> reqwest::RequestBuilder {
    let builder = builder
        .header("X-Viceroy-Device-Id", &config.device.device_id)
        .header("X-Viceroy-Device-Name", &config.device.device_name)
        .header("X-Viceroy-Platform", &config.device.platform);

    if let Some(token) = &config.auth_token {
        builder.bearer_auth(token)
    } else {
        builder
    }
}

fn api_base_url(server_url: &str) -> Result<Url> {
    let mut url = Url::parse(server_url).context("invalid sync server_url")?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    url.join("api/v1/sync/")
        .context("failed to build sync API base URL")
}

fn websocket_url(server_url: &str, device: &LocalDevice) -> Result<Url> {
    let mut url = api_base_url(server_url)?
        .join("clipboard/ws")
        .context("failed to build websocket URL")?;
    match url.scheme() {
        "https" => url
            .set_scheme("wss")
            .map_err(|_| anyhow!("failed to convert https URL to wss"))?,
        "http" => url
            .set_scheme("ws")
            .map_err(|_| anyhow!("failed to convert http URL to ws"))?,
        "wss" | "ws" => {}
        other => return Err(anyhow!("unsupported sync server scheme: {other}")),
    }
    url.query_pairs_mut()
        .append_pair("device_id", &device.device_id)
        .append_pair("device_name", &device.device_name);
    Ok(url)
}

fn current_cursor() -> Result<Option<i64>> {
    let conn = database::get_connection()?;
    let value = conn
        .query_row(
            "SELECT value FROM sync_state WHERE key = ?1",
            params![CURSOR_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(value.and_then(|value| value.parse::<i64>().ok()))
}

fn update_cursor(cursor: i64) -> Result<()> {
    let conn = database::get_connection()?;
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![CURSOR_KEY, cursor.to_string()],
    )?;
    Ok(())
}

fn operation_name(operation: &SyncOperationKind) -> &'static str {
    match operation {
        SyncOperationKind::UpsertClipboardEntry => "upsert_clipboard_entry",
        SyncOperationKind::DeleteClipboardEntry => "delete_clipboard_entry",
    }
}

fn parse_operation(value: &str) -> SyncOperationKind {
    match value {
        "delete_clipboard_entry" => SyncOperationKind::DeleteClipboardEntry,
        _ => SyncOperationKind::UpsertClipboardEntry,
    }
}

fn notify_worker() {
    SYNC_NOTIFY.notify_one();
}

fn generate_device_id(device_name: &str) -> String {
    blake3::hash(
        format!(
            "device:{}:{}:{}:{}",
            device_name,
            default_platform(),
            now_ts(),
            std::process::id()
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string()
}

fn generate_entry_sync_id(entry_id: i64, device: &LocalDevice, updated_at: i64) -> String {
    blake3::hash(
        format!(
            "entry:{}:{}:{}:{}",
            device.device_id, device.device_name, entry_id, updated_at
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string()
}

fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Viceroy Device".to_string())
}

fn default_platform() -> &'static str {
    std::env::consts::OS
}

fn now_ts() -> i64 {
    Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_names_round_trip() {
        let upsert = parse_operation(operation_name(&SyncOperationKind::UpsertClipboardEntry));
        let delete = parse_operation(operation_name(&SyncOperationKind::DeleteClipboardEntry));
        assert_eq!(upsert, SyncOperationKind::UpsertClipboardEntry);
        assert_eq!(delete, SyncOperationKind::DeleteClipboardEntry);
    }

    #[test]
    fn generated_ids_are_non_empty() {
        let device = LocalDevice {
            device_id: "device-a".to_string(),
            device_name: "Laptop".to_string(),
            platform: "windows".to_string(),
        };
        assert!(!generate_device_id("Laptop").is_empty());
        assert!(!generate_entry_sync_id(42, &device, 100).is_empty());
    }

    #[test]
    fn websocket_url_uses_ws_scheme_and_sync_path() {
        let device = LocalDevice {
            device_id: "device-a".to_string(),
            device_name: "Laptop".to_string(),
            platform: "windows".to_string(),
        };
        let url = websocket_url("https://sync.example.com", &device).unwrap();
        assert_eq!(url.scheme(), "wss");
        assert_eq!(url.path(), "/api/v1/sync/clipboard/ws");
    }
}
