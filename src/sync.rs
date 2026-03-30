use crate::{database, settings};
use anyhow::{anyhow, Context, Result};
use chrono::{Local, LocalResult, TimeZone, Utc};
use futures_util::StreamExt;
use lazy_static::lazy_static;
use reqwest::Url;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration as StdDuration;
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
const STATE_CONNECTION_KEY: &str = "connection_state";
const STATE_LAST_SUCCESS_KEY: &str = "last_successful_sync_at";
const STATE_LAST_ERROR_KEY: &str = "last_error";
const PERMANENT_ERROR_PREFIX: &str = "permanent:";
const MAX_RECORDED_ERROR_CHARS: usize = 320;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalDevice {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownSyncDevice {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub is_current: bool,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncConnectionTestIssue {
    None,
    InvalidConfiguration,
    AuthenticationFailed,
    ServerUnreachable,
    UnexpectedResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConnectionTestResult {
    pub ok: bool,
    pub issue: SyncConnectionTestIssue,
    pub message: String,
    pub normalized_server_url: Option<String>,
    pub checked_at: i64,
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
    pub server_url: Option<String>,
    pub connection_state: SyncConnectionState,
    pub last_successful_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub pending_operations: usize,
    pub known_devices: Vec<KnownSyncDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncConnectionState {
    Disabled,
    Disconnected,
    Reconnecting,
    Connected,
}

impl SyncConnectionState {
    pub fn storage_value(&self) -> &'static str {
        match self {
            SyncConnectionState::Disabled => "disabled",
            SyncConnectionState::Disconnected => "disconnected",
            SyncConnectionState::Reconnecting => "reconnecting",
            SyncConnectionState::Connected => "connected",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            SyncConnectionState::Disabled => "Disabled",
            SyncConnectionState::Disconnected => "Disconnected",
            SyncConnectionState::Reconnecting => "Reconnecting",
            SyncConnectionState::Connected => "Connected",
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SyncRuntimeConfig {
    device: LocalDevice,
    server_url: String,
    auth_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncSessionExit {
    Reconfigure,
}

#[derive(Debug, Deserialize)]
struct DeviceListResponse {
    devices: Vec<RemoteSyncDevice>,
}

#[derive(Debug, Deserialize)]
struct RemoteSyncDevice {
    device_id: String,
    device_name: String,
    platform: String,
    first_seen_at: i64,
    last_seen_at: i64,
}

pub fn init() -> Result<SyncStatus> {
    let conn = database::get_connection()?;
    let device = ensure_local_device(&conn)?;
    status_with_connection(&conn, device)
}

pub fn normalize_server_url(input: &str) -> Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("sync server URL cannot be empty"));
    }

    let candidate = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };

    let url = Url::parse(&candidate).context("invalid sync server_url")?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(anyhow!("unsupported sync server scheme: {other}")),
    }

    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("sync server URL must include a host"))?;
    if matches!(host, "0.0.0.0" | "::" | "[::]") {
        return Err(anyhow!(
            "sync server URL must use a reachable host, not {host}; use 127.0.0.1, localhost, a LAN IP, or a hostname"
        ));
    }

    Ok(url.to_string().trim_end_matches('/').to_string())
}

pub fn validate_server_url_for_local_device(server_url: &str) -> Result<()> {
    let url = Url::parse(server_url).context("invalid sync server_url")?;
    if !is_loopback_host(url.host_str()) {
        return Ok(());
    }

    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("sync server URL must include a port or use http/https"))?;

    let Some(host) = url.host_str() else {
        return Ok(());
    };

    let mut resolved = (host, port)
        .to_socket_addrs()
        .with_context(|| format!("failed to resolve sync server host {host}"))?;

    let timeout = StdDuration::from_millis(750);
    let reachable = resolved.any(|addr| TcpStream::connect_timeout(&addr, timeout).is_ok());

    if reachable {
        return Ok(());
    }

    Err(anyhow!(
        "sync server URL points at this device ({host}:{port}), but no server is reachable there; if your sync server runs on another machine, use that machine's LAN IP or hostname instead"
    ))
}

pub fn start_background_worker() -> Result<()> {
    if SYNC_WORKER_STARTED.swap(true, Ordering::SeqCst) {
        notify_worker();
        return Ok(());
    }

    if runtime_config()?.is_none() {
        SYNC_WORKER_STARTED.store(false, Ordering::SeqCst);
        return Ok(());
    }

    thread::spawn(move || {
        let runtime = Runtime::new().expect("failed to create sync runtime");
        runtime.block_on(async move {
            if let Err(err) = run_background_loop().await {
                log::error!("Sync worker exited: {err:#}");
            }
        });
        SYNC_WORKER_STARTED.store(false, Ordering::SeqCst);
    });

    Ok(())
}

pub fn status() -> Result<SyncStatus> {
    let conn = database::get_connection()?;
    let device = ensure_local_device(&conn)?;
    status_with_connection(&conn, device)
}

pub async fn refresh_remote_status() -> Result<SyncStatus> {
    if let Some(config) = runtime_config()? {
        let client = sync_http_client(Some(Duration::from_secs(5)))?;
        refresh_known_devices(&client, &config).await?;
    }
    status()
}

pub async fn test_connection(
    server_url_input: &str,
    auth_token_input: Option<&str>,
) -> SyncConnectionTestResult {
    let checked_at = now_ts();
    let normalized_server_url = match normalize_server_url(server_url_input) {
        Ok(url) => url,
        Err(err) => {
            return SyncConnectionTestResult {
                ok: false,
                issue: SyncConnectionTestIssue::InvalidConfiguration,
                message: format!("Invalid sync server URL: {err:#}"),
                normalized_server_url: None,
                checked_at,
            };
        }
    };

    if let Err(err) = validate_server_url_for_local_device(&normalized_server_url) {
        return SyncConnectionTestResult {
            ok: false,
            issue: SyncConnectionTestIssue::InvalidConfiguration,
            message: format!("Invalid sync server URL: {err:#}"),
            normalized_server_url: Some(normalized_server_url),
            checked_at,
        };
    }

    let device = match database::get_connection() {
        Ok(conn) => match ensure_local_device(&conn) {
            Ok(device) => device,
            Err(err) => {
                return SyncConnectionTestResult {
                    ok: false,
                    issue: SyncConnectionTestIssue::UnexpectedResponse,
                    message: format!("Unable to load local sync identity: {err:#}"),
                    normalized_server_url: Some(normalized_server_url),
                    checked_at,
                };
            }
        },
        Err(err) => {
            return SyncConnectionTestResult {
                ok: false,
                issue: SyncConnectionTestIssue::UnexpectedResponse,
                message: format!("Unable to open the local database: {err:#}"),
                normalized_server_url: Some(normalized_server_url),
                checked_at,
            };
        }
    };

    let client = match sync_http_client(Some(Duration::from_secs(4))) {
        Ok(client) => client,
        Err(err) => {
            return SyncConnectionTestResult {
                ok: false,
                issue: SyncConnectionTestIssue::UnexpectedResponse,
                message: format!("Unable to create sync HTTP client: {err:#}"),
                normalized_server_url: Some(normalized_server_url),
                checked_at,
            };
        }
    };

    let config = SyncRuntimeConfig {
        device,
        server_url: normalized_server_url.clone(),
        auth_token: auth_token_input.and_then(|token| {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }),
    };

    let url = match api_base_url(&config.server_url).and_then(|base| {
        base.join("devices")
            .context("failed to build sync device list URL")
    }) {
        Ok(url) => url,
        Err(err) => {
            return SyncConnectionTestResult {
                ok: false,
                issue: SyncConnectionTestIssue::InvalidConfiguration,
                message: format!("Invalid sync server URL: {err:#}"),
                normalized_server_url: Some(normalized_server_url),
                checked_at,
            };
        }
    };

    let response = match authorized_request(client.get(url), &config).send().await {
        Ok(response) => response,
        Err(err) => {
            return SyncConnectionTestResult {
                ok: false,
                issue: classify_connection_issue(&err),
                message: connection_send_error_message(&err),
                normalized_server_url: Some(normalized_server_url),
                checked_at,
            };
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return SyncConnectionTestResult {
            ok: false,
            issue: classify_status_issue(status),
            message: connection_status_message(status, &body),
            normalized_server_url: Some(normalized_server_url),
            checked_at,
        };
    }

    let body: DeviceListResponse = match response.json().await {
        Ok(body) => body,
        Err(err) => {
            return SyncConnectionTestResult {
                ok: false,
                issue: SyncConnectionTestIssue::UnexpectedResponse,
                message: format!(
                    "The sync server responded, but the device list could not be parsed: {err:#}"
                ),
                normalized_server_url: Some(normalized_server_url),
                checked_at,
            };
        }
    };

    let devices = body
        .devices
        .into_iter()
        .map(|device| remote_device_into_known(device, &config.device.device_id))
        .collect::<Vec<_>>();

    if let Ok(conn) = database::get_connection() {
        let _ = store_known_devices(&conn, &config.device.device_id, &devices);
    }

    let device_count = devices.len();
    let noun = if device_count == 1 {
        "device"
    } else {
        "devices"
    };
    SyncConnectionTestResult {
        ok: true,
        issue: SyncConnectionTestIssue::None,
        message: format!(
            "Connection succeeded. {device_count} {noun} are visible on the sync server."
        ),
        normalized_server_url: Some(normalized_server_url),
        checked_at,
    }
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
           AND (last_error IS NULL OR last_error NOT LIKE 'permanent:%')
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
    let error = compact_error_message(error);
    conn.execute(
        "UPDATE sync_outbox
         SET attempts = attempts + 1,
             last_error = ?1
         WHERE id = ?2",
        params![error, operation_id],
    )?;
    Ok(())
}

pub fn mark_operation_permanent_failure(operation_id: i64, error: &str) -> Result<()> {
    let permanent_error = if error.starts_with(PERMANENT_ERROR_PREFIX) {
        error.to_string()
    } else {
        format!("{PERMANENT_ERROR_PREFIX} {error}")
    };
    mark_operation_failed(operation_id, &permanent_error)
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
        "SELECT COUNT(*) FROM sync_outbox
         WHERE sent_at IS NULL
           AND (last_error IS NULL OR last_error NOT LIKE 'permanent:%')",
        [],
        |row| row.get(0),
    )?;
    let app_settings = settings::load()?;
    let server_url = app_settings
        .sync
        .server_url
        .clone()
        .filter(|value| !value.trim().is_empty());
    let connection_state = if !app_settings.sync.enabled {
        SyncConnectionState::Disabled
    } else if server_url.is_none() {
        SyncConnectionState::Disconnected
    } else {
        read_connection_state(conn)?.unwrap_or(SyncConnectionState::Disconnected)
    };

    Ok(SyncStatus {
        device,
        server_url,
        connection_state,
        last_successful_sync_at: read_state_i64(conn, STATE_LAST_SUCCESS_KEY)?,
        last_error: read_state_string(conn, STATE_LAST_ERROR_KEY)?,
        pending_operations: pending_operations as usize,
        known_devices: load_known_devices(conn)?,
    })
}

async fn run_background_loop() -> Result<()> {
    let client = sync_http_client(None)?;

    loop {
        let Some(config) = runtime_config()? else {
            let _ = set_connection_state(desired_idle_connection_state()?);
            SYNC_NOTIFY.notified().await;
            continue;
        };

        let _ = set_connection_state(SyncConnectionState::Reconnecting);
        match run_sync_session(&client, &config).await {
            Ok(SyncSessionExit::Reconfigure) => continue,
            Err(err) => {
                let error_message = format!("{err:#}");
                let _ = record_sync_error(&error_message);
                log::error!("Sync session failed: {err:#}");
            }
        }
        sleep(Duration::from_secs(RECONNECT_DELAY_SECONDS)).await;
    }
}

async fn run_sync_session(
    client: &reqwest::Client,
    config: &SyncRuntimeConfig,
) -> Result<SyncSessionExit> {
    refresh_known_devices(client, config).await?;
    mark_sync_success()?;
    catch_up_once(client, config).await?;
    mark_sync_success()?;
    process_outbox(client, config).await?;
    mark_sync_success()?;

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
    mark_sync_success()?;
    let (_write, mut read) = ws_stream.split();

    loop {
        tokio::select! {
            _ = SYNC_NOTIFY.notified() => {
                if should_reconfigure_session(config)? {
                    return Ok(SyncSessionExit::Reconfigure);
                }
                process_outbox(client, config).await?;
                mark_sync_success()?;
            }
            next_message = read.next() => {
                match next_message {
                    Some(Ok(message)) => {
                        if let Some(cursor) = handle_ws_message(message).await? {
                            update_cursor(cursor)?;
                            mark_sync_success()?;
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

    process_outbox_batch(client, config, &pending).await
}

async fn process_outbox_batch(
    client: &reqwest::Client,
    config: &SyncRuntimeConfig,
    pending: &[PendingSyncOperation],
) -> Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    let url = api_base_url(&config.server_url)?.join("clipboard/events")?;
    let mut batches = vec![pending.to_vec()];

    while let Some(batch) = batches.pop() {
        let request = PushEventsRequest {
            device: config.device.clone(),
            events: batch
                .iter()
                .map(|operation| SyncEnvelope {
                    operation: operation.operation.clone(),
                    record: operation.payload.clone(),
                })
                .collect(),
        };

        let response = authorized_request(client.post(url.clone()), config)
            .json(&request)
            .send()
            .await
            .context("failed to upload pending sync events")?;
        let status = response.status();

        if status.is_success() {
            for operation in &batch {
                mark_operation_sent(operation.id)?;
            }
            continue;
        }

        if status == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
            if batch.len() > 1 {
                let midpoint = batch.len() / 2;
                batches.push(batch[midpoint..].to_vec());
                batches.push(batch[..midpoint].to_vec());
                continue;
            }

            let operation = &batch[0];
            let event = SyncEnvelope {
                operation: operation.operation.clone(),
                record: operation.payload.clone(),
            };
            let payload_size = serde_json::to_vec(&event)
                .map(|bytes| bytes.len())
                .unwrap_or_default();
            let message = format!(
                "sync payload too large for the server limit; sync_id={} payload_bytes={payload_size}. Raise VICEROY_SYNC_SERVER_MAX_EVENT_BYTES on the server or remove the oversized clipboard item.",
                operation.entry_sync_id
            );
            mark_operation_permanent_failure(operation.id, &message)?;
            return Err(anyhow!(message).context("sync event upload failed"));
        }

        let error = response
            .error_for_status()
            .expect_err("non-success response should produce an error");
        let message = compact_error_message(&format!("{error:#}"));
        for operation in &batch {
            let _ = mark_operation_failed(operation.id, &message);
        }
        return Err(error).context("failed to upload pending sync events");
    }

    Ok(())
}

async fn refresh_known_devices(client: &reqwest::Client, config: &SyncRuntimeConfig) -> Result<()> {
    let devices = request_known_devices(client, config).await?;
    let conn = database::get_connection()?;
    store_known_devices(&conn, &config.device.device_id, &devices)?;
    Ok(())
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

async fn request_known_devices(
    client: &reqwest::Client,
    config: &SyncRuntimeConfig,
) -> Result<Vec<KnownSyncDevice>> {
    let url = api_base_url(&config.server_url)?.join("devices")?;
    let response = authorized_request(client.get(url), config)
        .send()
        .await
        .context("failed to request sync device list")?
        .error_for_status()
        .context("sync device list request failed")?;

    let body: DeviceListResponse = response
        .json()
        .await
        .context("failed to parse sync device list response")?;
    Ok(body
        .devices
        .into_iter()
        .map(|device| remote_device_into_known(device, &config.device.device_id))
        .collect())
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

    let server_url = normalize_server_url(&server_url)?;
    validate_server_url_for_local_device(&server_url)?;

    Ok(Some(SyncRuntimeConfig {
        device,
        server_url,
        auth_token: app_settings.sync.auth_token.clone(),
    }))
}

fn desired_idle_connection_state() -> Result<SyncConnectionState> {
    let app_settings = settings::load()?;
    if !app_settings.sync.enabled {
        Ok(SyncConnectionState::Disabled)
    } else if app_settings
        .sync
        .server_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        Ok(SyncConnectionState::Disconnected)
    } else {
        Ok(SyncConnectionState::Reconnecting)
    }
}

fn should_reconfigure_session(active_config: &SyncRuntimeConfig) -> Result<bool> {
    Ok(runtime_config()?.as_ref() != Some(active_config))
}

fn sync_http_client(timeout: Option<Duration>) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder();
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    builder.build().context("failed to create sync HTTP client")
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

fn load_known_devices(conn: &Connection) -> Result<Vec<KnownSyncDevice>> {
    let mut stmt = conn.prepare(
        "SELECT device_id, device_name, platform, is_current, first_seen_at, last_seen_at
         FROM sync_devices
         ORDER BY last_seen_at DESC, device_id ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(KnownSyncDevice {
            device_id: row.get(0)?,
            device_name: row.get(1)?,
            platform: row.get(2)?,
            is_current: row.get::<_, i64>(3)? == 1,
            first_seen_at: row.get(4)?,
            last_seen_at: row.get(5)?,
        })
    })?;

    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn store_known_devices(
    conn: &Connection,
    current_device_id: &str,
    devices: &[KnownSyncDevice],
) -> Result<()> {
    conn.execute(
        "UPDATE sync_devices
         SET is_current = CASE WHEN device_id = ?1 THEN 1 ELSE 0 END",
        params![current_device_id],
    )?;

    for device in devices {
        let is_current = device.device_id == current_device_id;
        conn.execute(
            "INSERT INTO sync_devices (
                device_id,
                device_name,
                platform,
                is_current,
                first_seen_at,
                last_seen_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(device_id) DO UPDATE SET
                device_name = excluded.device_name,
                platform = excluded.platform,
                is_current = excluded.is_current,
                first_seen_at = MIN(sync_devices.first_seen_at, excluded.first_seen_at),
                last_seen_at = MAX(sync_devices.last_seen_at, excluded.last_seen_at)",
            params![
                device.device_id,
                device.device_name,
                device.platform,
                is_current as i64,
                device.first_seen_at,
                device.last_seen_at
            ],
        )?;
    }

    Ok(())
}

fn remote_device_into_known(device: RemoteSyncDevice, current_device_id: &str) -> KnownSyncDevice {
    KnownSyncDevice {
        is_current: device.device_id == current_device_id,
        device_id: device.device_id,
        device_name: device.device_name,
        platform: device.platform,
        first_seen_at: device.first_seen_at,
        last_seen_at: device.last_seen_at,
    }
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

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("127.0.0.1" | "localhost" | "::1"))
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

fn read_state_string(conn: &Connection, key: &str) -> Result<Option<String>> {
    let value = conn
        .query_row(
            "SELECT value FROM sync_state WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(value.filter(|text| !text.trim().is_empty()))
}

fn read_state_i64(conn: &Connection, key: &str) -> Result<Option<i64>> {
    Ok(read_state_string(conn, key)?.and_then(|value| value.parse::<i64>().ok()))
}

fn write_state_value(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn delete_state_value(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM sync_state WHERE key = ?1", params![key])?;
    Ok(())
}

fn set_connection_state(state: SyncConnectionState) -> Result<()> {
    let conn = database::get_connection()?;
    write_state_value(&conn, STATE_CONNECTION_KEY, state.storage_value())
}

fn read_connection_state(conn: &Connection) -> Result<Option<SyncConnectionState>> {
    Ok(
        read_state_string(conn, STATE_CONNECTION_KEY)?.and_then(|value| match value.as_str() {
            "disabled" => Some(SyncConnectionState::Disabled),
            "disconnected" => Some(SyncConnectionState::Disconnected),
            "reconnecting" => Some(SyncConnectionState::Reconnecting),
            "connected" => Some(SyncConnectionState::Connected),
            _ => None,
        }),
    )
}

fn mark_sync_success() -> Result<()> {
    let conn = database::get_connection()?;
    let now = now_ts();
    write_state_value(
        &conn,
        STATE_CONNECTION_KEY,
        SyncConnectionState::Connected.storage_value(),
    )?;
    write_state_value(&conn, STATE_LAST_SUCCESS_KEY, &now.to_string())?;
    delete_state_value(&conn, STATE_LAST_ERROR_KEY)?;
    Ok(())
}

fn record_sync_error(error: &str) -> Result<()> {
    let conn = database::get_connection()?;
    let error = compact_error_message(error);
    write_state_value(
        &conn,
        STATE_CONNECTION_KEY,
        SyncConnectionState::Disconnected.storage_value(),
    )?;
    write_state_value(&conn, STATE_LAST_ERROR_KEY, &error)?;
    Ok(())
}

fn compact_error_message(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.chars().count() <= MAX_RECORDED_ERROR_CHARS {
        return trimmed.to_string();
    }

    let compact = trimmed
        .chars()
        .take(MAX_RECORDED_ERROR_CHARS.saturating_sub(1))
        .collect::<String>();
    format!("{compact}…")
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

fn classify_connection_issue(error: &reqwest::Error) -> SyncConnectionTestIssue {
    if error.is_connect() || error.is_timeout() {
        SyncConnectionTestIssue::ServerUnreachable
    } else {
        SyncConnectionTestIssue::UnexpectedResponse
    }
}

fn classify_status_issue(status: reqwest::StatusCode) -> SyncConnectionTestIssue {
    match status {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            SyncConnectionTestIssue::AuthenticationFailed
        }
        reqwest::StatusCode::NOT_FOUND => SyncConnectionTestIssue::InvalidConfiguration,
        _ => SyncConnectionTestIssue::UnexpectedResponse,
    }
}

fn connection_send_error_message(error: &reqwest::Error) -> String {
    if error.is_connect() {
        "Could not reach the sync server. Check that the host, port, and firewall settings are correct."
            .to_string()
    } else if error.is_timeout() {
        "The sync server did not respond in time. Check the server URL and whether the server is reachable from this device."
            .to_string()
    } else {
        format!("The sync server request failed before a response was returned: {error:#}")
    }
}

fn connection_status_message(status: reqwest::StatusCode, body: &str) -> String {
    let detail = body
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(160)
        .collect::<String>();

    match status {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            "The sync server rejected the credentials. Check that the auth token matches the server configuration."
                .to_string()
        }
        reqwest::StatusCode::NOT_FOUND => {
            "The server responded, but `/api/v1/sync/devices` was not found. Point the server URL at the Viceroy sync server root."
                .to_string()
        }
        _ if detail.is_empty() => format!(
            "The sync server returned {}. Check that the URL points at your Viceroy sync server and that the server is healthy.",
            status
        ),
        _ => format!("The sync server returned {}. {}", status, detail.trim()),
    }
}

fn now_ts() -> i64 {
    Utc::now().timestamp()
}

pub fn format_timestamp(timestamp: Option<i64>) -> String {
    let Some(timestamp) = timestamp else {
        return "Never".to_string();
    };

    let dt = match Local.timestamp_opt(timestamp, 0) {
        LocalResult::Single(dt) => dt,
        _ => return "Never".to_string(),
    };
    dt.format("%Y-%m-%d %H:%M:%S %Z").to_string()
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

    #[test]
    fn normalize_server_url_adds_http_scheme() {
        let normalized = normalize_server_url("127.0.0.1:8787").unwrap();
        assert_eq!(normalized, "http://127.0.0.1:8787");
    }

    #[test]
    fn normalize_server_url_rejects_unspecified_bind_host() {
        let err = normalize_server_url("0.0.0.0:8787").unwrap_err();
        assert!(format!("{err:#}").contains("reachable host"));
    }

    #[test]
    fn connection_state_labels_round_trip_for_storage() {
        assert_eq!(SyncConnectionState::Disabled.storage_value(), "disabled");
        assert_eq!(SyncConnectionState::Connected.display_label(), "Connected");
    }

    #[test]
    fn timestamp_formatter_handles_missing_values() {
        assert_eq!(format_timestamp(None), "Never");
    }

    #[test]
    fn loopback_host_detection_matches_local_hosts() {
        assert!(is_loopback_host(Some("127.0.0.1")));
        assert!(is_loopback_host(Some("localhost")));
        assert!(is_loopback_host(Some("::1")));
        assert!(!is_loopback_host(Some("192.168.1.50")));
        assert!(!is_loopback_host(Some("sync.example.com")));
        assert!(!is_loopback_host(None));
    }
}
