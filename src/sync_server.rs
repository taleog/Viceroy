use crate::sync::{CatchUpResponse, PushEventsRequest, SyncEnvelope};
use anyhow::{anyhow, Context, Result};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Path;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use blake3;
use futures_util::{SinkExt, StreamExt};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8787";

#[derive(Debug, Clone)]
pub struct SyncServerConfig {
    pub bind_addr: String,
    pub database_path: PathBuf,
    pub auth_token: Option<String>,
    pub admin_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SyncServerState {
    pub config: SyncServerConfig,
    pub broadcaster: broadcast::Sender<SyncEnvelope>,
    pub claimed_devices_by_token: Arc<Mutex<HashMap<String, String>>>,
}

#[derive(Debug, Deserialize)]
struct CatchUpQuery {
    since: Option<i64>,
    device_id: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct WsQuery {
    device_id: Option<String>,
    device_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateDeviceRequest {
    disabled: bool,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct PushEventsResponse {
    accepted: usize,
    cursor: Option<i64>,
}

#[derive(Debug, Serialize)]
struct DeviceListResponse {
    devices: Vec<DeviceRecord>,
}

#[derive(Debug, Clone, Serialize)]
struct DeviceRecord {
    device_id: String,
    device_name: String,
    platform: String,
    first_seen_at: i64,
    last_seen_at: i64,
    is_disabled: bool,
    disabled_at: Option<i64>,
    disabled_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct DeviceIdentity {
    device_id: String,
    device_name: String,
    platform: String,
}

#[derive(Debug, Clone)]
struct SyncAuthContext {
    token_fingerprint: Option<String>,
}

pub async fn run() -> Result<()> {
    let config = SyncServerConfig::from_env()?;
    let state = build_state(config)?;
    let router = router(state.clone());
    let bind_addr: SocketAddr = state
        .config
        .bind_addr
        .parse()
        .context("invalid VICEROY_SYNC_SERVER_BIND address")?;

    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind sync server on {}", bind_addr))?;

    log::info!("Viceroy sync server listening on {bind_addr}");
    axum::serve(listener, router)
        .await
        .context("sync server stopped unexpectedly")
}

pub fn router(state: Arc<SyncServerState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/sync/clipboard/events", post(post_events))
        .route("/api/v1/sync/clipboard/changes", get(get_changes))
        .route("/api/v1/sync/clipboard/ws", get(ws_handler))
        .route("/api/v1/sync/devices", get(list_devices))
        .route("/api/v1/sync/devices/{device_id}", patch(update_device))
        .with_state(state)
}

pub fn build_state(config: SyncServerConfig) -> Result<Arc<SyncServerState>> {
    init_db(&config.database_path)?;
    let (tx, _) = broadcast::channel(512);
    Ok(Arc::new(SyncServerState {
        config,
        broadcaster: tx,
        claimed_devices_by_token: Arc::new(Mutex::new(HashMap::new())),
    }))
}

impl SyncServerConfig {
    pub fn from_env() -> Result<Self> {
        let bind_addr =
            std::env::var("VICEROY_SYNC_SERVER_BIND").unwrap_or_else(|_| DEFAULT_BIND_ADDR.into());
        let database_path = std::env::var("VICEROY_SYNC_SERVER_DATABASE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("viceroy-sync-server.db"));
        let auth_token = std::env::var("VICEROY_SYNC_SERVER_AUTH_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty());
        let admin_token = std::env::var("VICEROY_SYNC_SERVER_ADMIN_TOKEN")
            .ok()
            .filter(|token| !token.trim().is_empty());

        Ok(Self {
            bind_addr,
            database_path,
            auth_token,
            admin_token,
        })
    }
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse { status: "ok" })
}

async fn list_devices(
    State(state): State<Arc<SyncServerState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, SyncServerError> {
    authorize_admin(&headers, &state)?;
    let conn = open_connection(&state)?;
    let devices = load_devices(&conn)?;
    Ok(Json(DeviceListResponse { devices }))
}

async fn update_device(
    State(state): State<Arc<SyncServerState>>,
    headers: HeaderMap,
    Path(device_id): Path<String>,
    Json(request): Json<UpdateDeviceRequest>,
) -> Result<impl IntoResponse, SyncServerError> {
    authorize_admin(&headers, &state)?;
    let conn = open_connection(&state)?;
    let now = now_ts();
    let updated = set_device_disabled_state(
        &conn,
        &device_id,
        request.disabled,
        request.reason.as_deref(),
        now,
    )?
    .ok_or_else(|| SyncServerError::not_found("device not found"))?;

    Ok(Json(updated))
}

async fn post_events(
    State(state): State<Arc<SyncServerState>>,
    headers: HeaderMap,
    Json(request): Json<PushEventsRequest>,
) -> Result<impl IntoResponse, SyncServerError> {
    let auth = authorize_sync(&headers, &state)?;
    let conn = open_connection(&state)?;
    let device = register_or_touch_device(
        &conn,
        device_identity_from_headers(&headers)?,
        auth.token_fingerprint.as_deref(),
    )?;
    ensure_device_enabled(&device)?;
    remember_token_claim(&state, auth.token_fingerprint.as_deref(), &device.device_id);

    let mut accepted = 0usize;
    let mut cursor = None;

    for event in request.events {
        if insert_event(&conn, &event)? {
            accepted += 1;
            cursor = Some(
                cursor
                    .unwrap_or(event.record.updated_at)
                    .max(event.record.updated_at),
            );
            let _ = state.broadcaster.send(event);
        }
    }

    Ok(Json(PushEventsResponse { accepted, cursor }))
}

async fn get_changes(
    State(state): State<Arc<SyncServerState>>,
    headers: HeaderMap,
    Query(query): Query<CatchUpQuery>,
) -> Result<impl IntoResponse, SyncServerError> {
    let auth = authorize_sync(&headers, &state)?;
    let conn = open_connection(&state)?;
    let device = register_or_touch_device(
        &conn,
        device_identity_from_headers(&headers)?,
        auth.token_fingerprint.as_deref(),
    )?;
    ensure_device_enabled(&device)?;
    remember_token_claim(&state, auth.token_fingerprint.as_deref(), &device.device_id);
    let since = query.since.unwrap_or(0);
    let limit = query.limit.unwrap_or(200).min(1_000);
    let events = load_events_since(&conn, since, query.device_id.as_deref(), limit)?;
    let cursor = events
        .iter()
        .map(|event| event.record.updated_at)
        .max()
        .or(Some(since));

    Ok(Json(CatchUpResponse { cursor, events }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<SyncServerState>>,
    headers: HeaderMap,
    Query(query): Query<WsQuery>,
) -> Result<impl IntoResponse, SyncServerError> {
    let auth = authorize_sync(&headers, &state)?;
    let device =
        resolve_websocket_device(&state, &headers, &query, auth.token_fingerprint.as_deref())?;
    remember_token_claim(&state, auth.token_fingerprint.as_deref(), &device.device_id);

    Ok(ws.on_upgrade(move |socket| {
        websocket_session(socket, state, device.device_id, auth.token_fingerprint)
    }))
}

async fn websocket_session(
    socket: WebSocket,
    state: Arc<SyncServerState>,
    device_id: String,
    token_fingerprint: Option<String>,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut broadcast_rx = state.broadcaster.subscribe();
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(60));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                match touch_device_by_id(&state, &device_id, token_fingerprint.as_deref()) {
                    Ok(device) if device.is_disabled => break,
                    Ok(_) => {}
                    Err(err) => {
                        log::warn!("sync device heartbeat failed for {}: {err:?}", device_id);
                        break;
                    }
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        if sender.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            broadcast = broadcast_rx.recv() => {
                match broadcast {
                    Ok(event) => {
                        if device_id == event.record.source_device_id {
                            continue;
                        }
                        match serde_json::to_string(&event) {
                            Ok(payload) => {
                                if sender.send(Message::Text(payload.into())).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
        }
    }
}

fn authorize_sync(
    headers: &HeaderMap,
    state: &SyncServerState,
) -> Result<SyncAuthContext, SyncServerError> {
    let Some(expected) = &state.config.auth_token else {
        return Ok(SyncAuthContext {
            token_fingerprint: None,
        });
    };

    let Some(actual) = bearer_token(headers) else {
        return Err(SyncServerError::unauthorized(
            "missing or invalid bearer token",
        ));
    };

    if actual != expected.as_str() {
        return Err(SyncServerError::unauthorized(
            "missing or invalid bearer token",
        ));
    }

    Ok(SyncAuthContext {
        token_fingerprint: Some(token_fingerprint(&actual)),
    })
}

fn authorize_admin(headers: &HeaderMap, state: &SyncServerState) -> Result<(), SyncServerError> {
    let expected = state
        .config
        .admin_token
        .as_deref()
        .or(state.config.auth_token.as_deref());

    let Some(expected) = expected else {
        return Ok(());
    };

    let Some(actual) = bearer_token(headers) else {
        return Err(SyncServerError::unauthorized(
            "missing or invalid bearer token",
        ));
    };

    if actual == expected {
        Ok(())
    } else {
        Err(SyncServerError::unauthorized(
            "missing or invalid bearer token",
        ))
    }
}

fn open_connection(state: &SyncServerState) -> Result<Connection, SyncServerError> {
    Connection::open(&state.config.database_path)
        .with_context(|| {
            format!(
                "failed to open sync server database at {}",
                state.config.database_path.display()
            )
        })
        .map_err(SyncServerError::from)
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let header_value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .trim();
    let mut parts = header_value.split_whitespace();
    let scheme = parts.next()?;
    let token = parts.next()?;
    if parts.next().is_none() && scheme.eq_ignore_ascii_case("Bearer") {
        return Some(token.to_string());
    }
    None
}

fn token_fingerprint(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn init_db(path: &PathBuf) -> Result<()> {
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open sync server database at {}", path.display()))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            sync_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            payload TEXT NOT NULL,
            source_device_id TEXT NOT NULL,
            updated_at INTEGER NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_events_unique
         ON sync_events(sync_id, operation, source_device_id, updated_at)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sync_events_updated_at
         ON sync_events(updated_at)",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_devices (
            device_id TEXT PRIMARY KEY,
            device_name TEXT NOT NULL,
            platform TEXT NOT NULL,
            auth_token_fingerprint TEXT,
            first_seen_at INTEGER NOT NULL,
            last_seen_at INTEGER NOT NULL,
            is_disabled INTEGER NOT NULL DEFAULT 0,
            disabled_at INTEGER,
            disabled_reason TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sync_devices_last_seen_at
         ON sync_devices(last_seen_at)",
        [],
    )?;
    Ok(())
}

fn device_identity_from_headers(headers: &HeaderMap) -> Result<DeviceIdentity, SyncServerError> {
    let device_id = header_text(headers, "X-Viceroy-Device-Id")
        .ok_or_else(|| SyncServerError::bad_request("missing X-Viceroy-Device-Id header"))?;
    let device_name = header_text(headers, "X-Viceroy-Device-Name")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Viceroy Device".to_string());
    let platform = header_text(headers, "X-Viceroy-Platform")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(DeviceIdentity {
        device_id,
        device_name,
        platform,
    })
}

fn header_text(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .map(str::to_string)
}

fn resolve_websocket_device(
    state: &SyncServerState,
    headers: &HeaderMap,
    query: &WsQuery,
    token_fingerprint: Option<&str>,
) -> Result<DeviceRecord, SyncServerError> {
    let conn = open_connection(state)?;
    let device_id = query
        .device_id
        .clone()
        .or_else(|| {
            token_fingerprint.and_then(|fingerprint| {
                state
                    .claimed_devices_by_token
                    .lock()
                    .ok()
                    .and_then(|map| map.get(fingerprint).cloned())
            })
        })
        .ok_or_else(|| {
            SyncServerError::bad_request(
                "websocket connections must identify a device through a prior sync request",
            )
        })?;

    let known_device = load_device_by_id(&conn, &device_id)?;
    let identity = if let Some(device) = known_device {
        DeviceIdentity {
            device_id: device.device_id,
            device_name: query
                .device_name
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(device.device_name),
            platform: device.platform,
        }
    } else {
        DeviceIdentity {
            device_id,
            device_name: query
                .device_name
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Viceroy Device".to_string()),
            platform: header_text(headers, "X-Viceroy-Platform")
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "unknown".to_string()),
        }
    };

    let device = register_or_touch_device(&conn, identity, token_fingerprint)?;
    ensure_device_enabled(&device)?;
    remember_token_claim(state, token_fingerprint, &device.device_id);
    Ok(device)
}

fn register_or_touch_device(
    conn: &Connection,
    identity: DeviceIdentity,
    token_fingerprint: Option<&str>,
) -> Result<DeviceRecord, SyncServerError> {
    let now = now_ts();
    conn.execute(
        "INSERT INTO sync_devices (
            device_id,
            device_name,
            platform,
            auth_token_fingerprint,
            first_seen_at,
            last_seen_at,
            is_disabled,
            disabled_at,
            disabled_reason
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, NULL)
        ON CONFLICT(device_id) DO UPDATE SET
            device_name = excluded.device_name,
            platform = excluded.platform,
            auth_token_fingerprint = COALESCE(excluded.auth_token_fingerprint, sync_devices.auth_token_fingerprint),
            last_seen_at = excluded.last_seen_at",
        params![
            identity.device_id,
            identity.device_name,
            identity.platform,
            token_fingerprint,
            now,
            now
        ],
    )?;
    load_device_by_id(conn, &identity.device_id)?
        .ok_or_else(|| SyncServerError::from(anyhow!("failed to reload registered device")))
}

fn load_device_by_id(
    conn: &Connection,
    device_id: &str,
) -> Result<Option<DeviceRecord>, SyncServerError> {
    let mut stmt = conn.prepare(
        "SELECT device_id, device_name, platform, first_seen_at, last_seen_at,
                is_disabled, disabled_at, disabled_reason
         FROM sync_devices
         WHERE device_id = ?1",
    )?;
    let device = stmt
        .query_row(params![device_id], |row| {
            Ok(DeviceRecord {
                device_id: row.get(0)?,
                device_name: row.get(1)?,
                platform: row.get(2)?,
                first_seen_at: row.get(3)?,
                last_seen_at: row.get(4)?,
                is_disabled: row.get::<_, i64>(5)? != 0,
                disabled_at: row.get(6)?,
                disabled_reason: row.get(7)?,
            })
        })
        .optional()?;
    Ok(device)
}

fn load_devices(conn: &Connection) -> Result<Vec<DeviceRecord>, SyncServerError> {
    let mut stmt = conn.prepare(
        "SELECT device_id, device_name, platform, first_seen_at, last_seen_at,
                is_disabled, disabled_at, disabled_reason
         FROM sync_devices
         ORDER BY last_seen_at DESC, device_id ASC",
    )?;
    let devices = stmt
        .query_map([], |row| {
            Ok(DeviceRecord {
                device_id: row.get(0)?,
                device_name: row.get(1)?,
                platform: row.get(2)?,
                first_seen_at: row.get(3)?,
                last_seen_at: row.get(4)?,
                is_disabled: row.get::<_, i64>(5)? != 0,
                disabled_at: row.get(6)?,
                disabled_reason: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(devices)
}

fn set_device_disabled_state(
    conn: &Connection,
    device_id: &str,
    disabled: bool,
    reason: Option<&str>,
    now: i64,
) -> Result<Option<DeviceRecord>, SyncServerError> {
    let rows = if disabled {
        conn.execute(
            "UPDATE sync_devices
             SET is_disabled = 1,
                 disabled_at = ?2,
                 disabled_reason = ?3
             WHERE device_id = ?1",
            params![device_id, now, reason],
        )?
    } else {
        conn.execute(
            "UPDATE sync_devices
             SET is_disabled = 0,
                 disabled_at = NULL,
                 disabled_reason = NULL
             WHERE device_id = ?1",
            params![device_id],
        )?
    };

    if rows == 0 {
        return Ok(None);
    }

    load_device_by_id(conn, device_id)
}

fn touch_device_by_id(
    state: &SyncServerState,
    device_id: &str,
    token_fingerprint: Option<&str>,
) -> Result<DeviceRecord, SyncServerError> {
    let conn = open_connection(state)?;
    let device = load_device_by_id(&conn, device_id)?
        .ok_or_else(|| SyncServerError::bad_request("unknown sync device"))?;
    let identity = DeviceIdentity {
        device_id: device.device_id,
        device_name: device.device_name,
        platform: device.platform,
    };
    register_or_touch_device(&conn, identity, token_fingerprint)
}

fn ensure_device_enabled(device: &DeviceRecord) -> Result<(), SyncServerError> {
    if device.is_disabled {
        let reason = device
            .disabled_reason
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!(": {value}"))
            .unwrap_or_default();
        Err(SyncServerError::forbidden(&format!(
            "device {} is disabled{reason}",
            device.device_id
        )))
    } else {
        Ok(())
    }
}

fn remember_token_claim(state: &SyncServerState, token_fingerprint: Option<&str>, device_id: &str) {
    let Some(token_fingerprint) = token_fingerprint else {
        return;
    };

    if let Ok(mut map) = state.claimed_devices_by_token.lock() {
        map.insert(token_fingerprint.to_string(), device_id.to_string());
    }
}

fn insert_event(conn: &Connection, event: &SyncEnvelope) -> Result<bool, SyncServerError> {
    let payload = serde_json::to_string(event).context("failed to serialize sync event")?;
    let rows = conn.execute(
        "INSERT OR IGNORE INTO sync_events (
            sync_id,
            operation,
            payload,
            source_device_id,
            updated_at,
            created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            event.record.sync_id,
            serde_json::to_string(&event.operation)?,
            payload,
            event.record.source_device_id,
            event.record.updated_at,
            chrono::Utc::now().timestamp()
        ],
    )?;
    Ok(rows > 0)
}

fn load_events_since(
    conn: &Connection,
    since: i64,
    device_id: Option<&str>,
    limit: usize,
) -> Result<Vec<SyncEnvelope>, SyncServerError> {
    let sql = if device_id.is_some() {
        "SELECT payload FROM sync_events
         WHERE updated_at > ?1 AND source_device_id != ?2
         ORDER BY updated_at ASC, id ASC
         LIMIT ?3"
    } else {
        "SELECT payload FROM sync_events
         WHERE updated_at > ?1
         ORDER BY updated_at ASC, id ASC
         LIMIT ?2"
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = if let Some(device_id) = device_id {
        stmt.query_map(params![since, device_id, limit as i64], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map(params![since, limit as i64], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    rows.into_iter()
        .map(|payload| {
            serde_json::from_str::<SyncEnvelope>(&payload).map_err(SyncServerError::from)
        })
        .collect()
}

#[derive(Debug)]
pub struct SyncServerError {
    status: StatusCode,
    message: String,
}

impl SyncServerError {
    fn bad_request(message: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.to_string(),
        }
    }

    fn forbidden(message: &str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.to_string(),
        }
    }

    fn not_found(message: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.to_string(),
        }
    }

    fn unauthorized(message: &str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.to_string(),
        }
    }
}

impl IntoResponse for SyncServerError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}

impl From<anyhow::Error> for SyncServerError {
    fn from(error: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{error:#}"),
        }
    }
}

impl From<rusqlite::Error> for SyncServerError {
    fn from(error: rusqlite::Error) -> Self {
        Self::from(anyhow!(error))
    }
}

impl From<serde_json::Error> for SyncServerError {
    fn from(error: serde_json::Error) -> Self {
        Self::from(anyhow!(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_config_uses_self_hostable_defaults() {
        let config = SyncServerConfig::from_env().unwrap();
        assert!(!config.bind_addr.trim().is_empty());
        assert!(!config.database_path.as_os_str().is_empty());
    }

    #[test]
    fn admin_token_defaults_to_client_token() {
        let config = SyncServerConfig {
            bind_addr: "127.0.0.1:8787".to_string(),
            database_path: PathBuf::from("test.db"),
            auth_token: Some("client-token".to_string()),
            admin_token: None,
        };
        assert_eq!(
            config
                .admin_token
                .as_deref()
                .or(config.auth_token.as_deref()),
            Some("client-token")
        );
    }

    #[test]
    fn device_registration_persists_last_seen_and_metadata() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("sync.db");
        init_db(&db_path).unwrap();
        let conn = Connection::open(&db_path).unwrap();

        let device = register_or_touch_device(
            &conn,
            DeviceIdentity {
                device_id: "device-a".to_string(),
                device_name: "Laptop".to_string(),
                platform: "windows".to_string(),
            },
            Some("fingerprint-a"),
        )
        .unwrap();

        assert_eq!(device.device_id, "device-a");
        assert_eq!(device.device_name, "Laptop");
        assert_eq!(device.platform, "windows");
        assert!(!device.is_disabled);

        let loaded = load_device_by_id(&conn, "device-a").unwrap().unwrap();
        assert_eq!(loaded.device_name, "Laptop");
        assert_eq!(loaded.platform, "windows");
        assert!(loaded.last_seen_at >= loaded.first_seen_at);
    }

    #[test]
    fn disabled_device_is_rejected() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("sync.db");
        init_db(&db_path).unwrap();
        let conn = Connection::open(&db_path).unwrap();
        let device = register_or_touch_device(
            &conn,
            DeviceIdentity {
                device_id: "device-b".to_string(),
                device_name: "Office".to_string(),
                platform: "macos".to_string(),
            },
            Some("fingerprint-b"),
        )
        .unwrap();
        let disabled =
            set_device_disabled_state(&conn, &device.device_id, true, Some("revoked"), now_ts())
                .unwrap()
                .unwrap();

        assert!(disabled.is_disabled);
        assert!(ensure_device_enabled(&disabled).is_err());
    }
}
