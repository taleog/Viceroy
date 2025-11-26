# GitHub Copilot / AI Agent Instructions for Viceroy

These instructions make AI agents immediately productive in this codebase. Focus on existing patterns; do not introduce speculative architectures.

This project is a clone of the existing macOS app Monarch (https://monarchapp.com/) named Viceroy. It is written in Rust using the Tauri framework for the desktop application and a static HTML/JS frontend. Please refer to the actual Monarch app for feature implementation details and user experience.

## Architecture Overview
- **Runtime**: Rust + Tauri (single window launcher) with a static HTML/JS UI in `ui/index.html`.
- **Entry Point**: `src/main.rs` configures Tauri, registers `#[tauri::command]` functions, applies macOS vibrancy, registers global shortcut `Alt+Space`, and spawns the asynchronous clipboard monitor thread.
- **Search Orchestrator**: `src/search_engine.rs` unifies queries across modes (apps, files, clipboard, calculator, emoji, dictionary, web search, system commands). Ranking uses `fuzzy-matcher` + layered boosts (`get_smart_score`). Modify ranking ONLY there.
- **Subsystem Modules** (single-purpose, stateless except where noted):
  - `app_launcher.rs`: Discovers & launches apps; also reports frontmost app for clipboard attribution.
  - `file_search.rs`: Spotlight (`mdfind`) wrapper for fast file queries.
  - `clipboard.rs`: Poll-based monitor using `arboard`; persistence via SQLite; supports pin, rename, delete (guarded), pause, paste simulation.
  - `calculator.rs`: Expression parsing + multi-format outputs (decimal, hex, binary, percentage).
  - `system_commands.rs`: Executes macOS shell / AppleScript commands mapped to user queries.
  - `emoji.rs`, `dictionary.rs`, `web_search.rs`: Specialized command matchers.
  - `database.rs`: Initializes and returns SQLite connections (clipboard only for now).
  - `settings.rs`: JSON config under `~/.config/viceroy/settings.json`.

## Data & Persistence
- **Clipboard DB**: SQLite file path resolved via `dirs::config_dir()`. Table: `clipboard_history` with columns (`id`, `content`, `content_type`, `app_name`, `timestamp`, `custom_name`, `is_favorite`, `is_pinned`). Ordering always: pinned first then newest.
- **Migrations**: Opportunistic: adding columns uses `ALTER TABLE ...` with `.ok()` ignore. If changing schema, mimic this minimal pattern; do not add external migration tooling.

## Frontend Integration
- UI is a single file: `ui/index.html` containing styling, mode management, search invocation, clipboard split view, and result rendering.
- Communication: Use `invoke('command_name', { ... })` (see functions in `main.rs`). Maintain existing naming to avoid breaking UI.
- Clipboard specialized view logic lives near top (functions: `updateModeUI`, `classifyClipboardEntry`, `renderClipboardList`, `updateClipboardPreview`). Ensure helper definitions precede usage to avoid runtime `undefined` errors.

## Conventions & Patterns
- **Async Boundary**: Commands exposed with `#[tauri::command]` may be `async`; internal modules mostly sync except clipboard operations interacting with DB.
- **Search Mode Expansion**: Add enum variant in `SearchMode`, integrate into dispatcher order, then extend `SearchResult` if needed. Keep sort logic centralized in `get_smart_score`.
- **Ranking Adjustments**: All boosting logic consolidated in `get_smart_score`; never scatter heuristics elsewhere.
- **Clipboard Monitor**: Poll interval fixed (200ms). Pause state guarded by `lazy_static` `Arc<Mutex<bool>>`. Preserve small footprint; avoid spawning extra runtimes.
- **Safe Deletes**: Pinned clipboard entries must not delete; replicate pattern from `delete_entry` if adding other protected states.
- **Hotkey**: Registered in `setup` with `global_shortcut_manager`. For new hotkeys, reuse pattern, wrap logic in closure toggling window visibility.
- **Window Effects**: macOS vibrancy via `apply_vibrancy(... HudWindow ...)`. Keep effect consistent (don't add platform-specific branching unless required).
- **Size Target (<20MB)**: Avoid large dependencies. Before adding crates, check existing functionality; prefer extending current modules.

## Developer Workflows
- **Dev Mode**: Run `./dev.sh` which starts Python static server for UI then `cargo run`. If UI fails to load, confirm port 8080 is free.
- **Build**: `cargo build --release` (size-optimized via `Cargo.toml` settings). Binary: `target/release/viceroy`.
- **Lint / Format**: `cargo clippy`, `cargo fmt`.
- **Check**: `cargo check` for fast validation.
- **No Test Suite**: Add focused module tests inside `src/*` if needed; keep them small and fast.

## Adding Features (Examples)
- New search mode (e.g. Notes):
  1. Add variant to `SearchMode`.
  2. Add corresponding `SearchResult` variant if needed.
  3. Insert logic block in `search_with_mode` (before generic sections if high priority).
  4. Update UI mode buttons and `updateModeUI` to handle specialized rendering if required.
- Extend clipboard metadata:
  1. Add column in `database::init` with `ALTER TABLE ... .ok()` fallback.
  2. Update `ClipboardEntry` struct + queries in `get_history` / `search_history`.
  3. Adjust UI mapping (`classifyClipboardEntry`).

## Common Pitfalls
- Defining helper functions after usage in `index.html` causes runtime errors—keep ordering consistent (helpers first).
- Escaped backticks inside template literals (`\``) break rendering—write raw template strings.
- Large search queries: Avoid excessive DB reads; honor existing `LIMIT` patterns.
- Global shortcut conflicts: If registration fails, surface error via `setup` return.

## When Modifying
- Keep changes surgical; avoid refactoring cross-module boundaries unless required for a feature.
- Preserve current JSON shape of settings and clipboard entries; UI expects these fields.
- For new Tauri commands: add function + `#[tauri::command]` + entry in `invoke_handler`. Maintain consistent error mapping (`map_err(|e| e.to_string())`).

## Do Not
- Introduce heavy ORM or migration frameworks.
- Add multiple windows; architecture assumes a single hidden/shown launcher.
- Store sensitive clipboard data without respecting existing password manager exclusions.

## Quick Reference
- Main window toggle: in `setup` closure inside `main.rs`.
- Ranking logic: `get_smart_score` in `search_engine.rs`.
- Clipboard DB path: `database::get_db_path()` → `~/.config/viceroy/clipboard.db`.
- UI entry: `ui/index.html` (search input, mode selector, clipboard split view).

Provide diffs with minimal changes. Confirm build with `cargo check` before larger edits.
