# Troubleshooting

This page collects the problems users and contributors are most likely to hit during the alpha stage.

## macOS: "Viceroy.app is damaged and can't be opened"

This usually means you are using an older downloaded build.

Try this first:
1. Delete the old DMG
2. Delete the copied `Viceroy.app` from `Applications`
3. Download the latest macOS release asset again
4. Reinstall from the new DMG

If you are on `0.1.0-alpha.2`, make sure you are using the republished DMG from the fixed release workflow, not an earlier copy you already had on disk.

## macOS: Gatekeeper says the app cannot be opened

Current builds are unsigned, so a first-launch warning is expected.

Use:
1. Open `Applications`
2. Right-click `Viceroy.app`
3. Choose `Open`
4. Confirm once

## Global hotkey does not work on macOS

Check Accessibility permissions:
1. Open System Settings
2. Go to Privacy & Security
3. Open Accessibility
4. Enable Viceroy

If you installed a new copy, macOS may treat it as a new app entry and require permission again.

## Clipboard item pastes the wrong thing

Make sure you are on a current build. Recent fixes improved:
- frontmost-app reactivation
- clipboard write timing
- paste handoff reliability on macOS

If you still see stale first-paste behavior, capture:
- the source app
- whether you copied text or image content
- whether the second paste is always correct

## Sync server works on one machine but not another

Common causes:
- the second machine cannot reach the server host
- the client is using `localhost` or `127.0.0.1` incorrectly
- the auth token does not match

Quick checks:

```bash
curl http://SERVER:8787/health
curl "http://SERVER:8787/api/v1/sync/clipboard/changes?since=0"
```

Remember:
- `localhost` only points to the current machine
- clients should use a reachable LAN IP, hostname, Tailscale IP, or HTTPS endpoint

## File search seems missing

That may be intentional.

Fallback file search is disabled by default to avoid intrusive permission prompts during app startup.

For development testing:

```bash
export VICEROY_FALLBACK_FS=1
```

## Windows installer opens with a warning

That is expected for an unsigned alpha build.

Packaging is now cleaner, but Windows trust prompts still require code signing to fully smooth out.

## Debug Logging

Useful environment variables:

```bash
RUST_LOG=debug
VICEROY_FALLBACK_FS=1
VICEROY_NO_UPDATE_CHECK=1
```

Example:

```bash
RUST_LOG=debug /Applications/Viceroy.app/Contents/MacOS/Viceroy
```

## Reset Local State

macOS example:

```bash
rm "$HOME/Library/Application Support/viceroy/clipboard.db"
pkill Viceroy
open /Applications/Viceroy.app
```

Use this only when you want to clear local clipboard history and force a clean local restart.
