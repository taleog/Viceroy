# GitHub Copilot / AI Agent Instructions for Viceroy

These instructions make AI agents immediately productive in this codebase. Focus on existing patterns; do not introduce speculative architectures.

This project is a clone of the existing macOS app Monarch (https://monarchapp.com/) named Viceroy. It is written in Rust using native macOS Cocoa bindings for maximum performance and lightweight footprint (<20MB).

## Architecture Overview
- **Runtime**: Pure Rust + Cocoa (via `cacao`, `cocoa`, `objc` crates). No webviews, no Tauri, no HTML/JS.
- **Entry Point**: `src/main.rs` initializes `NSApplication`, sets up the `AppDelegate`, and constructs the programmatic UI.
- **UI Framework**: Direct usage of AppKit (`NSWindow`, `NSTextField`, `NSTableView`) via `objc::msg_send!` and `cocoa` bindings.
- **Search Orchestrator**: `src/search_engine.rs` unifies queries. It uses `tokio::join!` to execute search futures in parallel.
- **Concurrency Model**:
  - **Main Thread**: Handles all UI events and drawing (AppKit requirement).
  - **Background**: `SEARCH_RT` (global `tokio::runtime::Runtime`) executes async search logic.
  - **Bridge**: `dispatch::Queue::main().exec_async(...)` marshals results back to the UI thread.
- **State Management**: Global `lazy_static` `Mutex` protected state (`TABLE_DATA`, `TABLE_RESULTS`, `WINDOW_SHOWING`) in `src/main.rs`.

## Subsystem Modules
- `app_launcher.rs`: Discovers & launches apps; reports frontmost app.
- `file_search.rs`: Spotlight (`mdfind`) wrapper. Uses `spawn_blocking`.
- `clipboard.rs`: Poll-based monitor using `arboard`; persistence via SQLite.
- `calculator.rs`: Expression parsing + multi-format outputs.
- `system_commands.rs`: Executes macOS shell / AppleScript commands.
- `database.rs`: SQLite connection (`~/.config/viceroy/clipboard.db`).
- `settings.rs`: JSON config (`~/.config/viceroy/settings.json`).

## Native UI Implementation
- **Window**: Custom `NSWindow` subclass (`MKKeyWindow`) for borderless, translucent, floating behavior.
- **Styling**: `NSVisualEffectView` for vibrancy/blur. `CALayer` for corner radius and borders.
- **Search Field**: Custom `NSTextField` subclass (`MKEscapeTextField`) handling key events (Escape, Arrows) and custom drawing (`MKTextFieldCell`).
- **Results**: `NSTableView` with `MKTableDelegate`. Rows are drawn programmatically using `NSTextField` and `NSImageView` subviews.
- **Menu Bar**: `NSStatusBar` item created programmatically with a native `NSMenu`.

## Conventions & Patterns
- **Objective-C Interop**: Use `msg_send!` for AppKit calls. Always wrap in `unsafe`.
- **Class Registration**: Custom subclasses (delegates, views) are registered at runtime using `objc::declare::ClassDecl`. Check for existence before registering (`Class::get("Name").is_some()`).
- **Memory Management**: Be mindful of `retain`/`release` when working with raw `id` pointers, though most UI objects are managed by their parent views.
- **Thread Safety**: NEVER call AppKit methods from background threads. Always use `dispatch::Queue::main()`.
- **Ranking**: Modify ranking logic ONLY in `src/search_engine.rs` (`get_smart_score`).

## Developer Workflows
- **Build**: `cargo build --release`.
- **Run**: `cargo run`.
- **Logs**: `env_logger` is configured. Use `eprintln!` for debug output.
- **Debugging**: If the UI freezes, check for deadlocks in `Mutex` usage or main thread blocking.
- **Deployment & Testing**: After implementing changes, always perform this sequence to let the user test:
  1. Close running instance: `pkill Viceroy || true`
  2. Build bundle: `./build_app.sh`
  3. Install: `cp -r Viceroy.app /Applications/`
  4. Launch: `open /Applications/Viceroy.app`

## Adding Features (Examples)
- **New Search Mode**:
  1. Add variant to `SearchMode` in `src/search_engine.rs`.
  2. Add logic to `search_with_mode`.
  3. Update `MKTableDelegate` in `src/main.rs` to handle the new result type's icon and text.
- **UI Changes**:
  1. Modify `create_results_table` or `view_for_row` in `src/main.rs`.
  2. Use `msg_send!` to configure NSView properties.
  3. No CSS/HTML - all styling is code.

## Common Pitfalls
- **Main Thread**: Panics if `msg_send!` to UI objects happens off the main thread.
- **Selectors**: Ensure selector names in `sel!(...)` match Objective-C exactly (including colons).
- **Nil Checks**: `msg_send!` to `nil` returns 0/nil/false silently. Check pointers if things aren't appearing.
- **Coordinate System**: Cocoa uses bottom-left origin (y=0 is bottom), but some views might be flipped.

## When Modifying
- Keep `src/main.rs` focused on UI and event handling. Move logic to modules.
- Preserve the "Viceroy" aesthetic: dark mode, translucency, rounded corners.
- Do not introduce web-based UI crates.
