# Installing Viceroy

This guide covers the release assets and the most common first-run questions.

## Release Assets

Current release downloads are split by use case:

- macOS client: `Viceroy-macOS-<tag>.dmg`
- Windows client: `Viceroy-Windows-Setup-<tag>.exe`
- Linux sync server: `viceroy-sync-server-linux-x64-<tag>.tar.gz`
- checksum manifest: `checksums-<tag>.txt`

The desktop builds are currently unsigned.

That means:
- macOS may show a Gatekeeper warning on first launch
- Windows may show a SmartScreen warning on first launch

## macOS Client

### Install from a release

1. Download `Viceroy-macOS-<tag>.dmg`
2. Open the DMG
3. Drag `Viceroy.app` into `Applications`
4. Launch `Viceroy.app` from `Applications`

### First launch behavior

If macOS warns on first launch:
1. Find `Viceroy.app` in `Applications`
2. Right-click it
3. Choose `Open`
4. Confirm once

If you downloaded an older broken DMG before the packaging fix, delete the old DMG and old copied app before trying again.

### Permissions

Viceroy may need Accessibility access for:
- the global hotkey
- paste automation back into the frontmost app

To grant it:
1. Open System Settings
2. Go to Privacy & Security
3. Open Accessibility
4. Enable Viceroy

## Windows Client

### Install from a release

1. Download `Viceroy-Windows-Setup-<tag>.exe`
2. Run the installer
3. Launch Viceroy from the Start menu

If SmartScreen warns, use the normal Windows "More info" flow and allow the app once if you trust the build.

## Linux Sync Server

### Install from a release

1. Download `viceroy-sync-server-linux-x64-<tag>.tar.gz`
2. Extract it
3. Run `./viceroy-sync-server`
4. Optional: verify the download against `checksums-<tag>.txt` from the release page before you run it

Important environment variables:

```text
VICEROY_SYNC_SERVER_BIND
VICEROY_SYNC_SERVER_DATABASE
VICEROY_SYNC_SERVER_AUTH_TOKEN
```

Example:

```bash
export VICEROY_SYNC_SERVER_BIND=0.0.0.0:8787
export VICEROY_SYNC_SERVER_DATABASE=./viceroy-sync-server.db
export VICEROY_SYNC_SERVER_AUTH_TOKEN=replace-me
./viceroy-sync-server
```

For fuller setup and deployment guidance, see [`sync-server.md`](./sync-server.md).

## Build Locally

From source:

```bash
git clone https://github.com/taleog/Viceroy.git
cd Viceroy
make app
```

This creates `Viceroy.app` in the repo root.

Useful local commands:

```bash
make install-app
cargo run
cargo run --bin viceroy-sync-server
```

## Recommended Future Polish

To make release installs feel more conventional for non-technical users, the next big step is signing:

- macOS: Developer ID signing + notarization
- Windows: code signing certificate

That is separate from packaging. Packaging controls the file type; signing controls how much the OS trusts the build on first launch.
