# Clipboard Sync Model

This document describes the current clipboard sync behavior that Viceroy contributors should preserve.

The important design goal is simple: local devices stay responsible for capturing clipboard changes, while the sync server acts as the transport and deduplication layer between devices.

## Identity And Transport

- Each client has a local `device_id` and a human-readable `device_name`.
- Local clipboard changes are written into the client database and queued for upload.
- The server stores a copy of each accepted event and fans it out to other clients.
- Catch-up requests use a `since` cursor.
- Live delivery uses WebSocket.

## Dedup Rules

Viceroy uses two layers of deduplication:

- Local clipboard deduplication happens before sync. If the local app decides a copy is a duplicate, it does not become a new synced event.
- Sync transport deduplication happens on the server. The server stores clipboard events in SQLite and ignores exact duplicates using the event identity fields.

The current event identity is based on:

- `sync_id`
- `operation`
- `source_device_id`
- `updated_at`

This means:

- Replaying the same event is safe.
- Uploading the same queued operation again does not create another row.
- A client can reconnect and retry without duplicating remote state.

## Conflict Rules

The current resolution model is last-writer-wins by `updated_at`.

- If a remote clipboard item does not exist locally yet, it is inserted.
- If the same `sync_id` already exists locally, the newer `updated_at` wins.
- Older remote updates are ignored when they would overwrite a newer local row.
- Deletes are soft deletes using `deleted_at`, so a delete can be propagated later as a tombstone.

Operationally, that means:

- The newest event timestamp wins.
- Delete events are treated as updates to the same logical clipboard item.
- A remote event from the same source device is ignored by the receiving client so it does not echo back into the originating device view.

## What Counts As The Same Item

The sync system does not deduplicate by clipboard content alone.

- Two devices copying the same text at different times are still separate local events.
- Two updates to the same local row are treated as one logical item with newer state.
- Content-level dedup is a separate local clipboard concern, not a sync concern.

This is intentional. It keeps sync behavior predictable and makes the server independent of content heuristics that may differ between platforms.

## Auth And Revocation

The current server auth model is a shared bearer token.

- If the token is set on the server, clients must send the same token.
- If you need to revoke access, rotate the token and update the remaining devices.
- There is not yet a per-device revoke API or server-side account model.

## Contributor Notes

When changing sync behavior, keep these invariants intact:

- Upload retries must be idempotent.
- Catch-up must be resumable from the last known cursor.
- Remote events from the source device must not loop forever.
- Deletes must survive reconnects as tombstones.
- Client UI should reflect the current connection state, not just the last successful save.

If a future change needs to break one of these rules, document the new behavior here before shipping it.
