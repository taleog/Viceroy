# Viceroy

[![CI](https://github.com/taleog/Viceroy/actions/workflows/ci.yml/badge.svg)](https://github.com/taleog/Viceroy/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.1.0--alpha.2-blue)](https://github.com/taleog/Viceroy/releases)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

Viceroy is a lightweight launcher written in Rust with a native macOS experience, a Windows desktop app on this branch, and a self-hosted clipboard sync server.  
It gives you a fast command palette for **apps, files, clipboard history, system commands, emoji, web search, and a calculator** — all in one place.

> ⚠️ **Status: Early alpha (0.1.0-alpha.x)**. Expect rough edges and breaking changes while the project matures in public.

---

## Features

- **🔍 Universal Search**
  - Fuzzy search across:
    - Installed apps
    - Files (via Spotlight / `mdfind`)
    - Clipboard history (text + images)
    - System commands (sleep, lock, volume, etc.)
    - Calculator expressions
    - Emoji and dictionary shortcuts
    - Web search helpers (Google, DuckDuckGo, etc.)

- **📋 Clipboard History**
  - Background clipboard monitor (text + images)
  - Stores:
    - Content
    - Source app name
    - Timestamp
    - Optional custom names + pinned items
  - Searchable history with relative times (“2 min ago”, “Yesterday”)
  - Optional self-hosted sync across devices via the built-in sync server

- **🚀 App Launcher**
  - Scans common app locations (`/Applications`, `/System/Applications`, `~/Applications`, etc.)
  - Tracks basic usage data (last used, launch count) to boost frequently used apps

- **📁 File Search**
  - Uses `mdfind` under the hood instead of re-implementing indexing
  - Respects hide patterns (e.g. `.git`, `node_modules`, `.DS_Store`)

- **⚡ System Commands**
  - Built-in commands like:
    - Lock / sleep / restart / shutdown
    - Volume up/down/mute
    - Empty trash
    - Toggle hidden files in Finder
    - Screenshot, color picker, and more

- **🔢 Calculator**
  - Type math expressions directly:
    - `2 + 2`, `10 * (5 + 3)`, etc.
  - Shows:
    - Decimal
    - Hex
    - Binary
    - Percentage

- **😊 Emoji & Dictionary Helpers**
  - Emoji picker with common emoji and keyword search (e.g. `:smile`, `:heart`)
  - Dictionary shortcuts:
    - `define word`
    - `def word`
    - `d word`
  - Opens macOS Dictionary via `dict://` links


- **⌨️ Global Hotkey**
  - Configurable hotkey to toggle the launcher from anywhere
  - Uses native macOS accessibility APIs (requires Accessibility permission on first run)

- **🔄 Self-Hosted Clipboard Sync**
  - Built-in sync client on macOS and Windows in this branch
  - Included `viceroy-sync-server` binary for self-hosted deployments
  - Supports HTTP(S) and Tailscale-friendly deployments
  - Uses a simple SQLite-backed event log and WebSocket fan-out

---

## Requirements

- macOS (Intel or Apple Silicon) for the native Spotlight-style launcher
- Windows for the desktop Windows app on this branch
- Rust toolchain (Rust 2021 edition compatible)
- `cargo` for building and running

macOS remains the most polished experience. This branch also includes a Windows app and a minimal CLI fallback for other platforms.

---

## Getting Started

### Option 1: Download a release

```bash
# Download the latest GitHub release
open https://github.com/taleog/Viceroy/releases
```

Release assets are built in CI and currently include:

- `Viceroy-macOS-<tag>.dmg` for the macOS client
- `Viceroy-Windows-Setup-<tag>.exe` for the Windows client
- `viceroy-sync-server-linux-x64-<tag>.tar.gz` for the Linux sync server

The repository does not track generated app bundles or installers.

### Option 2: Build from source

```bash
git clone https://github.com/taleog/Viceroy.git
cd Viceroy

# Run the app for your current platform
cargo run
```

On macOS this starts the floating launcher window. On Windows it starts the native Windows desktop app.  
On first macOS run, the system may ask for **Accessibility** permission so the global hotkey can work.

### Run the self-hosted sync server

```bash
cargo run --bin viceroy-sync-server
```

See [`docs/sync-server.md`](./docs/sync-server.md) for server setup, client settings, and protocol details.

### Build a `.app` bundle locally

There is a helper command to create a proper macOS app bundle from the tracked source and icon assets:

```bash
make app
```

This produces `Viceroy.app` in the project root.

You can then:

```bash
# Install locally (optional)
cp -r Viceroy.app /Applications/
open /Applications/Viceroy.app
```

Or use the one-step helper:

```bash
make install-app
```

For more details (DMG, signing, etc.), see [`APP_BUNDLE.md`](./APP_BUNDLE.md).

---

## Updates

- Viceroy performs a **non-blocking update check** shortly after launch (the UI stays responsive while the helper task runs) and only logs failures so it never interrupts your workflow.
- Disable the updater with `--no-update-check` or `VICEROY_NO_UPDATE_CHECK=1`.
- Run a silent check (no prompt; the update is downloaded automatically) with `--silent-update-check` or `VICEROY_SILENT_UPDATE_CHECK=1`.
- Override the metadata source with `VICEROY_UPDATE_METADATA_URL` when you need to point to a staging/mock server (see the ignored integration test for an example).

### How the updater works

1. Viceroy resolves the metadata URL (default is `https://example.com/viceroy/latest.json` but `VICEROY_UPDATE_METADATA_URL` can override it).
2. It downloads the metadata JSON (version + download URL + sha256 checksum) and compares the declared version with `env!("CARGO_PKG_VERSION")` using `semver`.
3. If a newer version is found and neither `--silent-update-check` nor `VICEROY_SILENT_UPDATE_CHECK` was passed, the user is prompted on the console (`Y/n`).
4. The release binary is streamed to a temporary `<current-exe>.download` on disk while a SHA-256 digest is computed.
5. Once the checksum matches, the helper copies the executable bit from the running binary, renames the temp file to replace the current executable (macOS requires restarting Viceroy to pick up the new code), and logs the successful install.
6. Errors are logged via `env_logger` (see `src/main.rs`) and the update gracefully gives up instead of panicking.

### Metadata contract

The metadata endpoint must return this document:

```json
{
  "version": "0.1.1",
  "download_url": "https://example.com/releases/viceroy-0.1.1",
  "sha256": "abc123..."
}
```

`download_url` needs to point to a raw executable built for the same architecture that is currently running. The SHA checksum should be computed over that executable so Viceroy can verify the download before renaming it into place.

### Testing

- `tests/updater_integration.rs` is ignored by default because it expects a mock server on `http://127.0.0.1:8999`. Set `VICEROY_UPDATE_METADATA_URL` to your local metadata endpoint before running it:

  ```bash
  export VICEROY_UPDATE_METADATA_URL="http://127.0.0.1:8999/latest.json"
  cargo test updater_integration -- --ignored
  ```

Start the local helper server (after building the release binary) with:

```bash
python3 scripts/mock_update_server.py --binary target/release/viceroy --version 0.1.1 --port 8999
```

It prints the metadata document and a matching download URL (`/download`). Point `VICEROY_UPDATE_METADATA_URL` at the printed metadata endpoint if you use a non-default port.

The mocked server should serve the metadata above and a valid binary blob with the matching SHA so the end-to-end flow can finish.

---

## Usage

> The exact keybindings and behaviour may change as the project evolves. The high-level usage pattern will stay the same.

### Global hotkey

- Press the configured global hotkey (configurable in `settings.json`) to:
  - Show the Viceroy window
  - Focus the search field
- Hit `Esc` or click away to dismiss (also configurable).

### Searching

Type into the search box:

- **Plain text** → searches:
  - Apps
  - Files
  - Clipboard entries
  - System commands
- **Math** → calculator mode
  - `2 + 2`
  - `100 / 4`
- **Emoji**:
  - Prefix with `:` or switch to emoji mode:
    - `:smile`
    - `:heart`
- **Dictionary**:
  - `define concurrency`
  - `def architecture`
  - `d polymorphism`
- **Web search shortcuts**:
  - `search how to boil pasta`
  - `google rust ffi tutorial`
  - `ddg launchctl tutorial`
  - `duckduckgo spotlight alternatives`

Use arrow keys / mouse to select a row, then:

- `Enter` to launch/open/execute
- For clipboard entries: pressing enter copies them back to the system clipboard and (optionally) pastes.

---

## Configuration

Viceroy stores its config and data under your OS config directory. Common locations are:

- **macOS Settings**: `~/Library/Application Support/viceroy/settings.json`
- **macOS Clipboard DB**: `~/Library/Application Support/viceroy/clipboard.db`
- **Linux Settings**: `~/.config/viceroy/settings.json`
- **Linux Clipboard DB**: `~/.config/viceroy/clipboard.db`
- **Windows Settings**: `%AppData%\viceroy\settings.json`
- **Windows Clipboard DB**: `%AppData%\viceroy\clipboard.db`
- **Usage data** (for app ranking): stored alongside the app config directory as `usage.json`

### Example `settings.json`

On first run, Viceroy will create a default config. You can edit it by hand, e.g.:

```json
{
  "theme": {
    "background_color": "#1e1e1e",
    "text_color": "#d4d4d4",
    "accent_color": "#007acc",
    "selection_color": "#264f78"
  },
  "file_hiding_patterns": [
    "\\.git",
    "node_modules",
    "\\.DS_Store"
  ],
  "retype_delay_enabled": false,
  "max_results": 50,
  "hotkey": "Alt+Space",
  "dismiss_on_escape": true,
  "dismiss_on_click_away": true,
  "sync": {
    "enabled": false,
    "device_id": "",
    "device_name": "Office Laptop",
    "server_url": "http://100.116.102.40:8787",
    "auth_token": null,
    "poll_interval_seconds": 15
  }
}
```

> Note: The hotkey syntax is parsed by the `global-hotkey` crate. Not all combinations may be valid on all macOS versions.
> Note: Older flat sync keys such as `sync_enabled` and `sync_server_url` are migrated automatically into the nested `sync` section on load.

### Sync setup notes

- Use the base server URL only, for example `http://100.116.102.40:8787`, not `/api/v1/sync/...`
- `127.0.0.1` or `localhost` only work if the sync server is running on the same device as the client
- If you secure the server with `VICEROY_SYNC_SERVER_AUTH_TOKEN`, set the same bearer token in each client
- Tailscale works well for personal setups because each device can point at the server's Tailscale IP or DNS name

For a fuller walkthrough, see [`docs/sync-server.md`](./docs/sync-server.md).

---

## Development Workflow

### Quick Start

```bash
# Clone and set up development environment
git clone https://github.com/taleog/Viceroy.git
cd Viceroy
make setup  # Installs git hooks and checks toolchain
```

The repo ships with a `Makefile` so repetitive commands stay discoverable:

```bash
make help
```

### Common Commands

**Development:**
- `make setup` — Set up development environment (git hooks, toolchain check)
- `make run RUN_ARGS='--silent-update-check'` — Run Viceroy with optional CLI arguments
- `make fmt` / `make lint` / `make test` — Formatting, clippy (fails on warnings), and tests
- `make check` — Run all checks (fmt + lint + test)

**Build & Release:**
- `make release` — Build release binary at `target/release/viceroy`
- `make app` — Build `Viceroy.app` in the repo root
- `make install-app` — Build, install to `/Applications`, and open the app
- `make version` — Show current version

**Update System Testing:**
- `make mock-server` — Serve release binary + metadata locally
- `make mock-e2e` — Full end-to-end test with mock server

### Git Hooks

After running `make setup`, git hooks are installed:

- **pre-commit**: Runs `cargo fmt` and `cargo clippy`, reminds you to update CHANGELOG.md
- **commit-msg**: Validates commit messages follow [Conventional Commits](https://www.conventionalcommits.org/)

### Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed contribution guidelines.

---

## Architecture (for developers)

High-level overview:

- **Runtime / UI**
  - macOS UI uses Rust + Cocoa/AppKit:
    - `cacao`, `cocoa`, `objc`, `objc-foundation`, `core-foundation`, `core-graphics`
  - Windows uses a native desktop UI built with `eframe`/`egui`
  - Non-macOS/non-Windows platforms fall back to a minimal CLI entrypoint
  - The macOS launcher window is built programmatically with AppKit widgets

- **Concurrency Model**
  - All UI work runs on the **main thread** (AppKit requirement).
  - Background work uses a global `tokio::runtime::Runtime` (`SEARCH_RT`).
  - A small helper in `ui/helpers.rs` (`run_on_main`) marshals results back to the main thread via `dispatch::Queue::main()`.

- **Key Modules**
  - `app_launcher.rs` — app discovery, frontmost app detection, and launching.
  - `file_search.rs` — wraps `mdfind` for Spotlight-backed file search.
  - `clipboard.rs` — async clipboard monitor using `arboard`, writes to SQLite via `database.rs`.
  - `sync.rs` — clipboard sync client, outbox, catch-up flow, and WebSocket listener.
  - `sync_server.rs` — self-hosted sync server router and SQLite-backed event store.
  - `calculator.rs` — expression evaluation and formatting (decimal/hex/binary/percentage).
  - `system_commands.rs` — shell/AppleScript wrappers for system actions.
  - `web_search.rs` — builds search URLs and opens them with `open`.
  - `emoji.rs` — small in-memory emoji database + keyword search.
  - `dictionary.rs` — opens macOS Dictionary with `dict://` URLs.
  - `database.rs` — SQLite schema & connections (`clipboard_history`, `sync_outbox`, `sync_state`, and related indices).
  - `settings.rs` — JSON settings load/save (`settings.json`).
  - `usage.rs` — simple usage tracking of app launches to influence ranking.
  - `ui/*` — helper functions and state for the main window, table view, and clipboard list.
  - `windows_app.rs` — Windows desktop application shell and settings UI.

- **Search Orchestration**
  - Centralized in `search_engine.rs`:
    - Accepts a query + mode (All, Apps, Files, Clipboard, Calculator, Emoji, etc.).
    - Uses `fuzzy-matcher` to score results.
    - Runs multiple search tasks in parallel with `tokio::join!`.

---

## Roadmap / Ideas

This is a hobby/learning project, so the roadmap is flexible. Some ideas:

- More search modes (notes, colors, audio, etc.)
- Richer emoji database with categories
- Better keyboard shortcuts & discoverability
- Plugin system / external command hooks
- Smarter ranking for results based on usage and context
- Better sync observability (connection testing, richer status, conflict tooling)

Issues and PRs are welcome, especially for:

- Bug fixes
- Performance improvements
- Better macOS integration
- Documentation and examples

---

## License

MIT License — see [`LICENSE`](./LICENSE) for details.

---

## Credits

- **Libraries**
  - https://crates.io/crates/cacao
  - https://crates.io/crates/cocoa
  - https://crates.io/crates/objc
  - https://crates.io/crates/tokio
  - https://crates.io/crates/fuzzy-matcher
  - https://crates.io/crates/rusqlite
  - https://crates.io/crates/arboard
  - https://crates.io/crates/global-hotkey

Built as a learning project and a daily driver launcher.
