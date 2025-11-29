# Viceroy

Viceroy is a lightweight **native macOS launcher** written in Rust.  
It gives you a fast, Spotlight-style command palette for **apps, files, clipboard history, system commands, emoji, web search, and a calculator** — all in one place.

> ⚠️ Status: early/experimental. Expect rough edges and breaking changes.

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

---

## Requirements

- macOS (Intel or Apple Silicon)
- Rust toolchain (Rust 2021 edition compatible)
- `cargo` for building and running

> Viceroy uses native macOS APIs (Cocoa/AppKit) directly, so it only targets macOS.

---

## Getting Started

### 1. Clone and build

```bash
# Clone the repo
git clone <your-repo-url> viceroy
cd viceroy

# Run in debug mode
cargo run
```

This will start Viceroy and show the floating search window.  
On first run, macOS may ask for **Accessibility** permission so the global hotkey can work.

### 2. Build a `.app` bundle

There is a helper script to create a proper macOS app bundle:

```bash
./build_app.sh
```

This produces `Viceroy.app` in the project root.

You can then:

```bash
# Install locally (optional)
cp -r Viceroy.app /Applications/
open /Applications/Viceroy.app
```

For more details (DMG, signing, etc.), see [`APP_BUNDLE.md`](./APP_BUNDLE.md).

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

Viceroy stores its config and data under your OS config directory, e.g.:

- **Settings**: `~/.config/viceroy/settings.json`
- **Clipboard DB**: `~/.config/viceroy/clipboard.db`
- **Usage data** (for app ranking): `~/.config/viceroy/usage.json`

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
  "dismiss_on_click_away": true
}
```

> Note: The hotkey syntax is parsed by the `global-hotkey` crate. Not all combinations may be valid on all macOS versions.

---

## Architecture (for developers)

High-level overview:

- **Runtime / UI**
  - Pure Rust + Cocoa/AppKit:
    - `cacao`, `cocoa`, `objc`, `objc-foundation`, `core-foundation`, `core-graphics`
  - UI is built programmatically:
    - `NSApplication`, `NSWindow`, `NSTextField`, `NSTableView`, etc.
  - A custom floating window mimics Spotlight/Raycast behaviour.

- **Concurrency Model**
  - All UI work runs on the **main thread** (AppKit requirement).
  - Background work uses a global `tokio::runtime::Runtime` (`SEARCH_RT`).
  - A small helper in `ui/helpers.rs` (`run_on_main`) marshals results back to the main thread via `dispatch::Queue::main()`.

- **Key Modules**
  - `app_launcher.rs` — app discovery, frontmost app detection, and launching.
  - `file_search.rs` — wraps `mdfind` for Spotlight-backed file search.
  - `clipboard.rs` — async clipboard monitor using `arboard`, writes to SQLite via `database.rs`.
  - `calculator.rs` — expression evaluation and formatting (decimal/hex/binary/percentage).
  - `system_commands.rs` — shell/AppleScript wrappers for system actions.
  - `web_search.rs` — builds search URLs and opens them with `open`.
  - `emoji.rs` — small in-memory emoji database + keyword search.
  - `dictionary.rs` — opens macOS Dictionary with `dict://` URLs.
  - `database.rs` — SQLite schema & connections (`clipboard_history` table + index).
  - `settings.rs` — JSON settings load/save (`settings.json`).
  - `usage.rs` — simple usage tracking of app launches to influence ranking.
  - `ui/*` — helper functions and state for the main window, table view, and clipboard list.

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
