# Sync Server

Viceroy ships with a self-hosted clipboard sync server so your clipboard history can move between devices without relying on a hosted service.

The current model is intentionally simple:
- your Mac and Windows devices act as clients
- the server stores clipboard events and forwards them to other clients
- auth is an optional shared bearer token

If you need the conflict and deduplication rules, see [`sync-model.md`](./sync-model.md).

## What The Server Does

- accepts clipboard change uploads from clients
- stores accepted events in SQLite
- serves catch-up requests for reconnecting clients
- forwards live updates over WebSocket

The server is transport and coordination. It is not a separate clipboard UI.

## Quick Start

### Run from source

```bash
cargo run --bin viceroy-sync-server
```

### Run from a release asset

```bash
tar -xzf viceroy-sync-server-linux-x64-<tag>.tar.gz
./viceroy-sync-server
```

Default behavior:
- bind address: `0.0.0.0:8787`
- database: `./viceroy-sync-server.db`
- auth: disabled unless you set a token

## Configuration

Environment variables:

```text
VICEROY_SYNC_SERVER_BIND
VICEROY_SYNC_SERVER_DATABASE
VICEROY_SYNC_SERVER_AUTH_TOKEN
```

Linux example:

```bash
export VICEROY_SYNC_SERVER_BIND=0.0.0.0:8787
export VICEROY_SYNC_SERVER_DATABASE=/srv/viceroy/viceroy-sync-server.db
export VICEROY_SYNC_SERVER_AUTH_TOKEN=replace-with-a-long-random-token
./viceroy-sync-server
```

Windows PowerShell example:

```powershell
$env:VICEROY_SYNC_SERVER_BIND="0.0.0.0:8787"
$env:VICEROY_SYNC_SERVER_DATABASE="C:\srv\viceroy\viceroy-sync-server.db"
$env:VICEROY_SYNC_SERVER_AUTH_TOKEN="replace-with-a-long-random-token"
cargo run --bin viceroy-sync-server
```

## Client Configuration

Client settings live in `settings.json` under the `sync` section.

Common locations:
- macOS: `~/Library/Application Support/viceroy/settings.json`
- Windows: `%APPDATA%\viceroy\settings.json`

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

Important notes:
- `device_id` is generated locally if blank
- `device_name` should be readable and unique enough to identify the machine
- `server_url` should be the base host only
- do not use `/api/v1/sync/...` as the client URL
- `localhost` only works when the server is running on that same machine
- older flat sync keys are migrated automatically into the nested `sync` section

## Recommended Deployment Pattern

For personal use, the simplest good setup is:

1. run the server on an always-on machine
2. protect it with Tailscale or HTTPS
3. set a bearer token
4. point every client at the same reachable URL

Examples:

```text
http://100.116.102.40:8787
https://sync.example.com
```

## Reverse Proxy Example

If you want HTTPS, put the server behind a reverse proxy such as Caddy or Nginx.

Example Caddyfile:

```caddy
sync.example.com {
    reverse_proxy 127.0.0.1:8787
}
```

Make sure your proxy:
- supports WebSocket upgrades
- forwards `Authorization` headers unchanged

## Health Checks

Check the server:

```bash
curl http://127.0.0.1:8787/health
```

Expected response:

```json
{"status":"ok"}
```

Check catch-up:

```bash
curl "http://127.0.0.1:8787/api/v1/sync/clipboard/changes?since=0"
```

If auth is enabled:

```bash
curl http://127.0.0.1:8787/health \
  -H "Authorization: Bearer replace-with-a-long-random-token"
```

## Auth Model

Current auth is intentionally simple:
- one optional shared bearer token
- every allowed client uses the same token
- there is no user account system yet
- there is no per-device revoke API yet

Operationally:
- if you need to revoke access, rotate the token
- if one device identity gets stuck, clear its `device_id` and restart the client

## Common Gotchas

- `127.0.0.1` on your Mac does not point to your Windows server
- `0.0.0.0` is valid as a bind address, not as a client URL
- if auth is enabled, blank or mismatched client tokens will fail with `401 Unauthorized`
- if WebSocket reconnects forever, check the reverse proxy and confirm the client URL points at the host root

## Related Docs

- [`sync-model.md`](./sync-model.md)
- [`installing.md`](./installing.md)
- [`troubleshooting.md`](./troubleshooting.md)
