# Sync Server

Viceroy includes a self-hosted sync server so clipboard sync can stay open source and user-operated.

The recommended model is:

- Run the sync server on an always-on machine such as a home server
- Run Viceroy on your Mac and Windows devices as clients
- Keep the server behind Tailscale or HTTPS
- Treat the server as transport and coordination, not as a clipboard editor

For the sync conflict and dedup rules, see [docs/sync-model.md](./sync-model.md).

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

## Client Configuration

Viceroy stores client settings in `settings.json` under the `sync` section.

The file lives at:

- Windows: `%APPDATA%\viceroy\settings.json`
- macOS: `~/Library/Application Support/viceroy/settings.json`

Example client config:

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
- `server_url` must be reachable from the client you are configuring
- `127.0.0.1` and `localhost` only work when the sync server runs on that same machine
- `auth_token` should match `VICEROY_SYNC_SERVER_AUTH_TOKEN` if auth is enabled
- Older flat sync keys such as `sync_enabled` and `sync_server_url` are migrated into this nested shape automatically

The current sync client is event-driven:

- local clipboard changes upload immediately
- startup and reconnect perform one catch-up request
- while the app is open, inbound changes arrive over WebSocket

The `poll_interval_seconds` field is still preserved for config compatibility, but current delivery does not depend on periodic polling.

## Deployment Patterns

### Personal Home Server

This is the recommended setup for private use:

1. Run `viceroy-sync-server` on an always-on home server
2. Put the server behind Tailscale
3. Set a bearer token
4. Point each desktop client at the server's Tailscale IP or MagicDNS name

Example client URL:

```text
http://100.116.102.40:8787
```

### Reverse Proxy

If you want a conventional HTTPS endpoint, put the server behind Caddy or Nginx.

Example Caddyfile:

```caddy
sync.example.com {
    reverse_proxy 127.0.0.1:8787
}
```

Example client URL:

```text
https://sync.example.com
```

When using a reverse proxy, make sure it supports WebSocket upgrades and forwards `Authorization` headers unchanged.

## Health Checks

Server health:

```powershell
Invoke-WebRequest http://127.0.0.1:8787/health
```

Expected response:

```json
{"status":"ok"}
```

Catch-up check:

```powershell
Invoke-WebRequest "http://127.0.0.1:8787/api/v1/sync/clipboard/changes?since=0"
```

If auth is enabled, add the bearer token header:

```powershell
Invoke-WebRequest "http://127.0.0.1:8787/health" -Headers @{ Authorization = "Bearer replace-with-a-long-random-token" }
```

## Auth And Device Management

Current server auth is intentionally simple:

- One bearer token protects the server
- Every client uses the same token
- There is no per-user account system yet
- There is no server-side revoke API yet

Current device management is mostly client-side:

- Each device has its own local `device_id`
- Each device should use a readable `device_name`
- The server stores `source_device_id` and `source_device_name` on accepted events
- The receiving client uses that metadata to avoid self-echo and to label remote changes

Operational guidance:

- If you need to remove a device, rotate the bearer token and update the remaining devices
- If a device identity gets stale, clear `device_id` in that client's `settings.json` and restart Viceroy
- Keep `device_name` unique enough to be useful in logs and status views

## Common Gotchas

- If the Windows machine hosts the sync server, the Mac must use the Windows machine's LAN IP, hostname, or Tailscale IP. `127.0.0.1` on the Mac points back to the Mac, not the Windows machine.
- If auth is enabled and one client has a blank token, uploads and catch-up requests will fail with `401 Unauthorized`.
- `server_url` should look like `http://host:8787` or `https://sync.example.com`, not `http://host:8787/api/v1/sync`.
- `0.0.0.0` is valid for the server bind address but not for client configuration.
- The macOS and Windows apps store local clipboard history in their own SQLite databases; the sync server only coordinates changes between them.

## Troubleshooting

- `401 Unauthorized`: the token does not match the server token, or the client has a blank token while auth is enabled.
- `invalid sync server_url`: the URL is malformed or points at an unreachable host.
- `sync server URL must use a reachable host`: do not use `0.0.0.0` in client settings.
- WebSocket reconnects forever: check reverse proxy upgrade support and confirm the server URL points at the proxy root, not the `/api/v1/sync` path.
- Sync works on one device but not another: confirm the second device can reach the same server URL from its own network.

## Recommended Personal Deployment

For personal use:

1. Run the server on your home server
2. Put it behind Tailscale first
3. Set a bearer token
4. Point each Viceroy client at that server URL

For broader public/open-source usage later:

1. Put the server behind HTTPS with Caddy or Nginx
2. Add per-device auth and revoke controls
3. Move from SQLite to Postgres if you outgrow single-node storage
