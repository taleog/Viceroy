# Sync Server

Viceroy includes a self-hosted sync server binary so clipboard sync can stay open source and user-operated.

The recommended model is:

- Run the sync server on an always-on machine such as a home server
- Run Viceroy on your Mac, Windows, or Linux devices as clients
- Keep the server behind Tailscale or HTTPS

## Run The Server

From the repository root:

```powershell
cargo run --bin viceroy-sync-server
```

Default behavior:

- Bind address: `0.0.0.0:8787`
- Database: `./viceroy-sync-server.db`
- Auth: disabled unless you set a bearer token

## Environment Variables

```text
VICEROY_SYNC_SERVER_BIND
VICEROY_SYNC_SERVER_DATABASE
VICEROY_SYNC_SERVER_AUTH_TOKEN
```

Example:

```powershell
$env:VICEROY_SYNC_SERVER_BIND="0.0.0.0:8787"
$env:VICEROY_SYNC_SERVER_DATABASE="C:\srv\viceroy\viceroy-sync-server.db"
$env:VICEROY_SYNC_SERVER_AUTH_TOKEN="replace-with-a-long-random-token"
cargo run --bin viceroy-sync-server
```

Health check:

```powershell
Invoke-WebRequest http://127.0.0.1:8787/health
```

Expected response:

```json
{"status":"ok"}
```

## Client Configuration

Viceroy stores client settings in `settings.json` under the `sync` section.

Example:

```json
{
  "sync": {
    "enabled": true,
    "device_id": "generated-per-device",
    "device_name": "Office Laptop",
    "server_url": "https://sync.example.com",
    "auth_token": "replace-with-the-same-token",
    "poll_interval_seconds": 15
  }
}
```

Notes:

- `device_id` is generated locally if blank
- `device_name` should be human-readable so you can identify devices later
- `server_url` should be the base host, not the full API path
- `auth_token` should match `VICEROY_SYNC_SERVER_AUTH_TOKEN` if auth is enabled

## Protocol Shape

The current client/server contract is:

- `POST /api/v1/sync/clipboard/events`
- `GET /api/v1/sync/clipboard/changes`
- `GET /api/v1/sync/clipboard/ws`

Client behavior:

- local clipboard changes upload immediately
- startup/reconnect performs one catch-up request
- while open, the app listens for remote changes over WebSocket

## Recommended Personal Deployment

For personal use:

1. Run the server on your home server
2. Put it behind Tailscale first
3. Set a bearer token
4. Point each Viceroy client at that server URL

For broader public/open-source usage later:

1. Put the server behind HTTPS with Caddy or Nginx
2. Replace the single shared token with user/device auth
3. Move from SQLite to Postgres if you outgrow single-node storage
