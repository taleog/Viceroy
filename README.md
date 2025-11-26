# Viceroy

A lightweight Rust macOS productivity launcher inspired by Viceroy, featuring instant search, clipboard history, app launching, file navigation, and system commands.

## Features

- **🔍 Universal Search**: Fuzzy search across apps, files, clipboard history, and system commands
- **📋 Clipboard History**: Automatic clipboard monitoring with searchable history and metadata
- **🚀 App Launcher**: Quick application launching with macOS NSWorkspace integration
- **📁 File Search**: Spotlight-powered file search with external drive support
- **⚡ System Commands**: Volume control, sleep, lock, color picker, and more
- **🔢 Calculator**: Built-in expression evaluator with hex/binary/percentage formats
- **🎨 Themeable**: Custom color themes and appearance settings
- **⌨️ Global Hotkey**: Cmd+K (configurable) for instant access

## Size Optimization

Targeting <20MB binary with aggressive compilation flags:
- LTO (Link-Time Optimization)
- Strip symbols
- Optimize for size (`opt-level = "z"`)
- Single codegen unit

## Installation

### Prerequisites

- macOS 10.15 or later
- Rust toolchain (automatically installed if not present)

### Build from Source

```bash
# Clone the repository
cd /Users/taleo/Nextcloud/Viceroy

# Build release binary
cargo build --release

# Run the application
cargo run --release
```

The binary will be located at `target/release/viceroy`.

## Usage

1. **Launch**: Start the application (it will run in the system tray)
2. **Open Launcher**: Press `Cmd+K` (or your configured hotkey)
3. **Search**: Start typing to search apps, files, clipboard, commands
4. **Navigate**: Use arrow keys to select results
5. **Execute**: Press Enter to launch/open the selected item
6. **Close**: Press Esc or click outside the window

## Keyboard Shortcuts

- `Cmd+K` - Open/hide launcher
- `↑↓` - Navigate results
- `Enter` - Execute selected result
- `Esc` - Close launcher

## Permissions

Viceroy requires the following macOS permissions:

- **Accessibility**: For global hotkey registration
- **Automation**: For system commands and app launching
- **Full Disk Access**: For comprehensive file search (optional)

The app will guide you through the permission setup on first launch.

## Configuration

Settings are stored in `~/.config/viceroy/settings.json`:

```json
{
  "theme": {
    "background_color": "#1e1e1e",
    "text_color": "#d4d4d4",
    "accent_color": "#007acc",
    "selection_color": "#264f78"
  },
  "file_hiding_patterns": ["\\.git", "node_modules", "\\.DS_Store"],
  "retype_delay_enabled": false,
  "max_results": 50,
  "hotkey": "CommandOrControl+Space"
}
```

## Database

Clipboard history is stored in `~/.config/viceroy/clipboard.db` (SQLite).

## Architecture

- **Backend**: Rust with Tauri framework
- **Frontend**: HTML/CSS/JavaScript
- **Search**: fuzzy-matcher with SkimMatcherV2
- **Clipboard**: arboard for cross-platform clipboard access
- **macOS Integration**: Cocoa/ObjectiveC bindings for native features
- **File Search**: mdfind (Spotlight) wrapper
- **Database**: SQLite via rusqlite

## Project Structure

```
Viceroy/
├── src/
│   ├── main.rs              # Application entry point & Tauri setup
│   ├── search_engine.rs     # Unified search coordinator
│   ├── app_launcher.rs      # macOS app discovery & launching
│   ├── file_search.rs       # Spotlight file search wrapper
│   ├── clipboard.rs         # Clipboard monitoring & history
│   ├── system_commands.rs   # System command execution
│   ├── calculator.rs        # Expression evaluator
│   ├── settings.rs          # Configuration management
│   └── database.rs          # SQLite initialization
├── ui/
│   └── index.html           # Main UI interface
├── icons/
│   └── icon.icns            # App icon
├── Cargo.toml               # Dependencies & build config
├── tauri.conf.json          # Tauri window & permission config
└── build.rs                 # Build script

```

## Development

```bash
# Run in development mode
cargo run

# Run tests
cargo test

# Check for errors
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Binary Size

Current estimated size breakdown:
- Tauri runtime: ~3-4MB
- Rust stdlib: ~1-2MB
- Dependencies: ~2-3MB
- Application code: ~1-2MB
- **Total**: ~8-12MB (well under 20MB target)

## Further Enhancements

- [ ] Emoji picker with Unicode 15.0 data
- [ ] Quick reminders integration
- [ ] Superlinks (custom URL scheme handlers)
- [ ] External drive indexing optimization
- [ ] Plugin/extension system
- [ ] Multi-monitor support
- [ ] Keyboard shortcut customization UI
- [ ] Import/export settings
- [ ] Cloud sync for clipboard history

## License

MIT License - See LICENSE file for details

## Credits

Inspired by Viceroy launcher for macOS.

Built with:
- [Tauri](https://tauri.app/) - Lightweight cross-platform framework
- [fuzzy-matcher](https://crates.io/crates/fuzzy-matcher) - Fuzzy string matching
- [rusqlite](https://crates.io/crates/rusqlite) - SQLite bindings
- [arboard](https://crates.io/crates/arboard) - Clipboard access
