use anyhow::{anyhow, Result};
use axum::serve;
use chrono::Utc;
use futures_util::StreamExt;
use reqwest::{Client, Url};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{HeaderValue, AUTHORIZATION};
use tokio_tungstenite::tungstenite::Message;
use viceroy::database;
use viceroy::settings::{self, Settings};
use viceroy::sync::{
    self, ClipboardSyncRecord, PushEventsRequest, SyncEnvelope, SyncOperationKind,
};
use viceroy::sync_server::{self, SyncServerConfig};

#[derive(Debug, Deserialize)]
struct PushResponse {
    accepted: usize,
    cursor: Option<i64>,
}

#[derive(Debug)]
struct TestServer {
    base_url: String,
    handle: JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sync_round_trip_persists_and_delivers_changes() -> Result<()> {
    let temp = TempDir::new()?;
    configure_test_config_root(temp.path());

    let server = start_sync_server(temp.path().join("server.db"), "sync-test-token").await?;
    let client = authenticated_client()?;
    wait_for_server_health(&client, &server.base_url).await?;

    let mut app_settings = Settings::default();
    app_settings.sync.enabled = true;
    app_settings.sync.device_name = "Test Laptop".to_string();
    app_settings.sync.server_url = Some(server.base_url.clone());
    app_settings.sync.auth_token = Some("sync-test-token".to_string());
    settings::save(&app_settings)?;

    database::init()?;
    let local_status = sync::init()?;
    sync::start_background_worker()?;

    let local_entry_id = insert_local_clipboard_entry("local clip from the source device")?;
    sync::queue_local_clipboard_upsert(local_entry_id)?;
    let pending = sync::pending_operations(10)?;
    assert_eq!(pending.len(), 1);
    let local_event = SyncEnvelope {
        operation: pending[0].operation.clone(),
        record: pending[0].payload.clone(),
    };

    wait_until(Duration::from_secs(20), || {
        Ok(sync::status()?.pending_operations == 0)
    })
    .await?;

    let initial_catch_up = fetch_changes(
        &client,
        &server.base_url,
        0,
        "observer-device",
        "Observer Device",
        "linux",
        "sync-test-token",
    )
    .await?;
    assert_eq!(initial_catch_up.events.len(), 1);
    assert_eq!(
        initial_catch_up.events[0].record.sync_id,
        local_event.record.sync_id
    );
    let remote_base_timestamp = initial_catch_up
        .cursor
        .unwrap_or_else(|| Utc::now().timestamp())
        + 10;

    let duplicate_upload = post_events(
        &client,
        &server.base_url,
        vec![local_event.clone()],
        "manual-source",
        "manual-source-name",
        "linux",
        "sync-test-token",
    )
    .await?;
    assert_eq!(duplicate_upload.accepted, 0);

    let ws_url = websocket_url(&server.base_url, "observer-device", "observer", "linux")?;
    let mut ws_request = ws_url.as_str().into_client_request()?;
    ws_request.headers_mut().insert(
        AUTHORIZATION,
        HeaderValue::from_str("Bearer sync-test-token")?,
    );
    insert_identity_headers(
        &mut ws_request,
        "observer-device",
        "Observer Device",
        "linux",
    )?;
    let (ws_stream, _) = connect_async(ws_request).await?;
    let (_write, mut read) = ws_stream.split();

    let remote_sync_id = "remote-lifecycle-sync-id";
    let remote_create = make_record(
        remote_sync_id,
        "Remote Device",
        "remote-device-1",
        remote_base_timestamp,
        None,
        "remote clip v1",
        Some("Remote clip"),
    );
    let remote_create_event = SyncEnvelope {
        operation: SyncOperationKind::UpsertClipboardEntry,
        record: remote_create.clone(),
    };
    let remote_create_response = post_events(
        &client,
        &server.base_url,
        vec![remote_create_event.clone()],
        "remote-device-1",
        "Remote Device",
        "windows",
        "sync-test-token",
    )
    .await?;
    assert_eq!(remote_create_response.accepted, 1);
    assert_eq!(
        remote_create_response.cursor,
        Some(remote_create.updated_at)
    );

    let received = timeout(Duration::from_secs(20), async {
        loop {
            match read.next().await {
                Some(Ok(Message::Text(text))) => {
                    let event: SyncEnvelope = serde_json::from_str(&text)?;
                    if event.record.sync_id == remote_sync_id {
                        break Ok::<SyncEnvelope, anyhow::Error>(event);
                    }
                }
                Some(Ok(Message::Binary(bytes))) => {
                    let event: SyncEnvelope = serde_json::from_slice(&bytes)?;
                    if event.record.sync_id == remote_sync_id {
                        break Ok::<SyncEnvelope, anyhow::Error>(event);
                    }
                }
                Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                Some(Ok(Message::Close(_))) => {
                    break Err::<SyncEnvelope, anyhow::Error>(anyhow!(
                        "websocket closed unexpectedly"
                    ))
                }
                Some(Ok(_)) => {}
                Some(Err(err)) => break Err::<SyncEnvelope, anyhow::Error>(err.into()),
                None => {
                    break Err::<SyncEnvelope, anyhow::Error>(anyhow!(
                        "websocket closed unexpectedly"
                    ))
                }
            }
        }
    })
    .await??;
    assert_eq!(received.record.content, remote_create.content);

    wait_until(Duration::from_secs(20), || {
        let row = load_local_clipboard_row(remote_sync_id)?;
        Ok(matches!(row, Some(ref row) if row.content == "remote clip v1" && row.deleted_at.is_none()))
    })
    .await?;

    let duplicate_record = make_record(
        "duplicate-local-record",
        "Dup Device",
        "dup-device",
        remote_base_timestamp + 100,
        None,
        "duplicate clip",
        None,
    );
    let first_id = sync::apply_remote_clipboard_record(&duplicate_record)?;
    let second_id = sync::apply_remote_clipboard_record(&duplicate_record)?;
    assert_eq!(first_id, second_id);
    assert_eq!(count_rows_by_sync_id("duplicate-local-record")?, 1);

    let remote_update = make_record(
        remote_sync_id,
        "Remote Device",
        "remote-device-1",
        remote_base_timestamp + 10,
        None,
        "remote clip v2",
        Some("Remote clip updated"),
    );
    post_events(
        &client,
        &server.base_url,
        vec![SyncEnvelope {
            operation: SyncOperationKind::UpsertClipboardEntry,
            record: remote_update.clone(),
        }],
        "remote-device-1",
        "Remote Device",
        "windows",
        "sync-test-token",
    )
    .await?;

    wait_until(Duration::from_secs(20), || {
        let row = load_local_clipboard_row(remote_sync_id)?;
        Ok(matches!(row, Some(ref row) if row.content == "remote clip v2" && row.custom_name.as_deref() == Some("Remote clip updated") && row.updated_at == remote_update.updated_at))
    })
    .await?;

    let remote_delete = make_record(
        remote_sync_id,
        "Remote Device",
        "remote-device-1",
        remote_base_timestamp + 20,
        Some(remote_base_timestamp + 20),
        "remote clip v2",
        Some("Remote clip updated"),
    );
    post_events(
        &client,
        &server.base_url,
        vec![SyncEnvelope {
            operation: SyncOperationKind::DeleteClipboardEntry,
            record: remote_delete.clone(),
        }],
        "remote-device-1",
        "Remote Device",
        "windows",
        "sync-test-token",
    )
    .await?;

    wait_until(Duration::from_secs(20), || {
        let row = load_local_clipboard_row(remote_sync_id)?;
        Ok(matches!(row, Some(ref row) if row.deleted_at.is_some()))
    })
    .await?;

    let final_catch_up = fetch_changes(
        &client,
        &server.base_url,
        initial_catch_up.cursor.unwrap_or(0),
        "observer-device",
        "Observer Device",
        "linux",
        "sync-test-token",
    )
    .await?;
    assert!(final_catch_up.events.len() >= 2);
    assert!(final_catch_up.events.iter().any(|event| event.operation
        == SyncOperationKind::UpsertClipboardEntry
        && event.record.sync_id == remote_sync_id));
    assert!(final_catch_up.events.iter().any(|event| event.operation
        == SyncOperationKind::DeleteClipboardEntry
        && event.record.sync_id == remote_sync_id));

    assert!(!local_status.device.device_id.trim().is_empty());
    Ok(())
}

fn configure_test_config_root(root: &Path) {
    let config_root = root.join("config");
    let home_root = root.join("home");
    #[cfg(target_os = "windows")]
    {
        std::env::set_var("APPDATA", &config_root);
        std::env::set_var("LOCALAPPDATA", root.join("local"));
        std::env::set_var("USERPROFILE", &home_root);
        std::env::set_var("HOME", &home_root);
    }
    #[cfg(target_os = "macos")]
    {
        std::env::set_var("HOME", &home_root);
    }
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("XDG_CONFIG_HOME", &config_root);
        std::env::set_var("HOME", &home_root);
    }
}

async fn start_sync_server(database_path: PathBuf, auth_token: &str) -> Result<TestServer> {
    let config = SyncServerConfig {
        bind_addr: "127.0.0.1:0".to_string(),
        database_path,
        auth_token: Some(auth_token.to_string()),
        admin_token: Some(auth_token.to_string()),
    };
    let state = sync_server::build_state(config)?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let base_url = format!("http://{}", addr);
    let router = sync_server::router(state);
    let handle = tokio::spawn(async move {
        let _ = serve(listener, router).await;
    });

    Ok(TestServer { base_url, handle })
}

async fn wait_for_server_health(client: &Client, base_url: &str) -> Result<()> {
    let health_url = Url::parse(base_url)?.join("health")?;
    timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(response) = client.get(health_url.clone()).send().await {
                if response.status().is_success() {
                    return Ok::<(), anyhow::Error>(());
                }
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await??;
    Ok(())
}

async fn fetch_changes(
    client: &Client,
    base_url: &str,
    since: i64,
    device_id: &str,
    device_name: &str,
    platform: &str,
    token: &str,
) -> Result<viceroy::sync::CatchUpResponse> {
    let url = sync_api_url(base_url, "clipboard/changes")?;
    let response = client
        .get(url)
        .header("X-Viceroy-Device-Id", device_id)
        .header("X-Viceroy-Device-Name", device_name)
        .header("X-Viceroy-Platform", platform)
        .query(&[
            ("since", since.to_string()),
            ("device_id", device_id.to_string()),
            ("limit", "100".to_string()),
        ])
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json().await?)
}

async fn post_events(
    client: &Client,
    base_url: &str,
    events: Vec<SyncEnvelope>,
    device_id: &str,
    device_name: &str,
    platform: &str,
    token: &str,
) -> Result<PushResponse> {
    let url = sync_api_url(base_url, "clipboard/events")?;
    let request = PushEventsRequest {
        device: viceroy::sync::LocalDevice {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            platform: std::env::consts::OS.to_string(),
        },
        events,
    };
    let response = client
        .post(url)
        .header("X-Viceroy-Device-Id", device_id)
        .header("X-Viceroy-Device-Name", device_name)
        .header("X-Viceroy-Platform", platform)
        .bearer_auth(token)
        .json(&request)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json().await?)
}

fn authenticated_client() -> Result<Client> {
    Ok(Client::builder().build()?)
}

fn sync_api_url(base_url: &str, path: &str) -> Result<Url> {
    let mut url = Url::parse(base_url)?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    Ok(url.join(&format!("api/v1/sync/{path}"))?)
}

fn websocket_url(
    base_url: &str,
    device_id: &str,
    device_name: &str,
    platform: &str,
) -> Result<Url> {
    let mut url = sync_api_url(base_url, "clipboard/ws")?;
    match url.scheme() {
        "http" => {
            url.set_scheme("ws")
                .map_err(|_| anyhow!("failed to convert http to ws"))?;
        }
        "https" => {
            url.set_scheme("wss")
                .map_err(|_| anyhow!("failed to convert https to wss"))?;
        }
        other => return Err(anyhow!("unsupported sync server scheme: {other}")),
    }
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("device_id", device_id);
        pairs.append_pair("device_name", device_name);
        pairs.append_pair("platform", platform);
    }
    Ok(url)
}

fn insert_identity_headers(
    request: &mut tokio_tungstenite::tungstenite::http::Request<()>,
    device_id: &str,
    device_name: &str,
    platform: &str,
) -> Result<()> {
    request
        .headers_mut()
        .insert("X-Viceroy-Device-Id", HeaderValue::from_str(device_id)?);
    request
        .headers_mut()
        .insert("X-Viceroy-Device-Name", HeaderValue::from_str(device_name)?);
    request
        .headers_mut()
        .insert("X-Viceroy-Platform", HeaderValue::from_str(platform)?);
    Ok(())
}

fn make_record(
    sync_id: &str,
    source_device_name: &str,
    source_device_id: &str,
    updated_at: i64,
    deleted_at: Option<i64>,
    content: &str,
    custom_name: Option<&str>,
) -> ClipboardSyncRecord {
    ClipboardSyncRecord {
        sync_id: sync_id.to_string(),
        content: content.to_string(),
        content_type: "text".to_string(),
        app_name: Some("Terminal".to_string()),
        timestamp: updated_at,
        custom_name: custom_name.map(ToString::to_string),
        is_favorite: false,
        is_pinned: false,
        image_width: None,
        image_height: None,
        source_device_id: source_device_id.to_string(),
        source_device_name: source_device_name.to_string(),
        updated_at,
        deleted_at,
    }
}

fn insert_local_clipboard_entry(content: &str) -> Result<i64> {
    let conn = database::get_connection()?;
    let timestamp = Utc::now().timestamp();
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
            is_pinned
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            content,
            "text",
            Some("Terminal"),
            timestamp,
            Option::<String>::None,
            Option::<i64>::None,
            Option::<i64>::None,
            0i64,
            0i64,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn load_local_clipboard_row(sync_id: &str) -> Result<Option<LocalClipboardRow>> {
    let conn = database::get_connection()?;
    let row = conn
        .query_row(
            "SELECT content, custom_name, deleted_at, updated_at
             FROM clipboard_history
             WHERE sync_id = ?1",
            params![sync_id],
            |row| {
                Ok(LocalClipboardRow {
                    content: row.get(0)?,
                    custom_name: row.get(1)?,
                    deleted_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

fn count_rows_by_sync_id(sync_id: &str) -> Result<i64> {
    let conn = database::get_connection()?;
    let count = conn.query_row(
        "SELECT COUNT(*) FROM clipboard_history WHERE sync_id = ?1",
        params![sync_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

async fn wait_until(
    timeout_duration: Duration,
    mut check: impl FnMut() -> Result<bool>,
) -> Result<()> {
    timeout(timeout_duration, async {
        loop {
            if check()? {
                return Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await??;
    Ok(())
}

#[derive(Debug)]
struct LocalClipboardRow {
    content: String,
    custom_name: Option<String>,
    deleted_at: Option<i64>,
    updated_at: i64,
}
