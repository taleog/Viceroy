use crate::sync::{CatchUpResponse, PushEventsRequest, SyncEnvelope};
use anyhow::{anyhow, Context, Result};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8787";

#[derive(Debug, Clone)]
pub struct SyncServerConfig {
    pub bind_addr: String,
    pub database_path: PathBuf,
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SyncServerState {
    pub config: SyncServerConfig,
    pub broadcaster: broadcast::Sender<SyncEnvelope>,
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

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct PushEventsResponse {
    accepted: usize,
    cursor: Option<i64>,
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
        .with_state(state)
}

pub fn build_state(config: SyncServerConfig) -> Result<Arc<SyncServerState>> {
    init_db(&config.database_path)?;
    let (tx, _) = broadcast::channel(512);
    Ok(Arc::new(SyncServerState {
        config,
        broadcaster: tx,
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

        Ok(Self {
            bind_addr,
            database_path,
            auth_token,
        })
    }
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse { status: "ok" })
}

async fn post_events(
    State(state): State<Arc<SyncServerState>>,
    headers: HeaderMap,
    Json(request): Json<PushEventsRequest>,
) -> Result<impl IntoResponse, SyncServerError> {
    authorize(&headers, &state)?;
    let conn = open_connection(&state)?;

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
    authorize(&headers, &state)?;
    let conn = open_connection(&state)?;
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
    authorize(&headers, &state)?;
    let device_id = query.device_id.clone();
    let _ = query.device_name;

    Ok(ws.on_upgrade(move |socket| websocket_session(socket, state, device_id)))
}

async fn websocket_session(
    socket: WebSocket,
    state: Arc<SyncServerState>,
    device_id: Option<String>,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut broadcast_rx = state.broadcaster.subscribe();

    loop {
        tokio::select! {
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
                        if device_id.as_deref() == Some(event.record.source_device_id.as_str()) {
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

fn authorize(headers: &HeaderMap, state: &SyncServerState) -> Result<(), SyncServerError> {
    let Some(expected) = &state.config.auth_token else {
        return Ok(());
    };

    let actual = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim);

    match actual {
        Some(token) if token == expected => Ok(()),
        _ => Err(SyncServerError::unauthorized(
            "missing or invalid bearer token",
        )),
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
    Ok(())
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

    #[test]
    fn default_config_uses_self_hostable_defaults() {
        let config = SyncServerConfig::from_env().unwrap();
        assert!(!config.bind_addr.trim().is_empty());
        assert!(!config.database_path.as_os_str().is_empty());
    }
}
