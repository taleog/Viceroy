# Viceroy Setup Guide

## Quick Start

### Development Mode

```bash
# Start the application in development mode
./dev.sh
```

This will:
1. Start the UI server on http://localhost:8080
2. Launch the Tauri application with live reload
3. Enable debug logging

### Build for Release

```bash
# Build optimized release binary
./build.sh
```

The binary will be at `target/release/viceroy` (estimated 8-12MB).

### Manual Build

```bash
# Development build
source $HOME/.cargo/env
cargo build

# Release build with optimizations
source $HOME/.cargo/env
cargo build --release

# Run
cargo run
# or
./target/release/viceroy
```

## First Run

### macOS Permissions

On first launch, macOS will request permissions:

1. **Accessibility Access** - Required for global hotkeys
   - System Settings > Privacy & Security > Accessibility
   - Enable Viceroy

2. **Automation** - Required for app launching and system commands
   - System Settings > Privacy & Security > Automation
   - Enable Viceroy for relevant apps

3. **Full Disk Access** (Optional) - For comprehensive file search
   - System Settings > Privacy & Security > Full Disk Access
   - Enable Viceroy

### Default Hotkey

Press **Cmd+K** to open the launcher (configurable in settings).

**Note**: Changed from Cmd+Space to avoid conflicts with macOS Spotlight.

## Configuration

### Settings Location

- Settings: `~/.config/viceroy/settings.json`
- Clipboard DB: `~/.config/viceroy/clipboard.db`

### Example Settings

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

## Features

### Search Modes

Viceroy automatically detects what you're searching for:

- **Apps**: Type app names (e.g., "safari", "notes")
- **Files**: Type file names (uses Spotlight)
- **Clipboard**: Search clipboard history
- **Calculator**: Enter math expressions (e.g., "2+2", "sqrt(16)")
- **System Commands**: Type command keywords (e.g., "lock", "sleep")

### Keyboard Shortcuts

- `Cmd+Space` - Open/close launcher
- `↑` / `↓` - Navigate results
- `Enter` - Execute selected item
- `Esc` - Close launcher

### Calculator Examples

- `2 + 2` - Basic arithmetic
- `sqrt(16)` - Square root
- `2^8` - Exponentiation
- `(5 + 3) * 2` - Complex expressions

Results show: decimal, hex, binary, and percentage formats.

### System Commands

Available commands:
- `lock` - Lock screen
- `sleep` - Put computer to sleep
- `volume up/down` - Adjust volume
- `mute` - Toggle mute
- `screenshot` - Take screenshot
- `color picker` - Open color picker
- `empty trash` - Empty trash

## Clipboard History

- Automatic monitoring (every 200ms)
- Stores last 1000 entries
- Metadata: app name, timestamp, content type
- Search through history
- Rename entries for better organization

## File Search

Uses macOS Spotlight (`mdfind`) for fast file search:

- Searches entire system
- Supports external drives
- Real-time indexing (via Spotlight)
- Respects privacy settings

## Customization

### Changing the Hotkey

Edit `~/.config/viceroy/settings.json`:

```json
{
  "hotkey": "CommandOrControl+K"
}
```

Supported modifiers: `CommandOrControl`, `Alt`, `Shift`

### Custom Themes

Create custom color schemes in settings:

```json
{
  "theme": {
    "background_color": "#2d2d30",
    "text_color": "#cccccc",
    "accent_color": "#00ff00",
    "selection_color": "#005500"
  }
}
```

### File Hiding Patterns

Exclude files from search results using regex patterns:

```json
{
  "file_hiding_patterns": [
    "\\.git",
    "node_modules",
    "\\.DS_Store",
    "__pycache__",
    "\\.cache"
  ]
}
```

## Troubleshooting

### Hotkey Not Working

1. Check Accessibility permissions
2. Verify no conflicts with system shortcuts
3. Try a different hotkey combination

### Clipboard History Not Saving

1. Check database file exists: `~/.config/viceroy/clipboard.db`
2. Verify write permissions
3. Check logs: `RUST_LOG=debug cargo run`

### File Search Returns No Results

1. Rebuild Spotlight index: `sudo mdutil -E /`
2. Check Full Disk Access permissions
3. Wait for Spotlight to complete indexing

### App Won't Launch

1. Check Automation permissions
2. Verify app path in error logs
3. Try launching from Terminal: `open -a "App Name"`

## Development

### Project Structure

```
Viceroy/
├── src/                    # Rust backend
│   ├── main.rs            # Entry point & Tauri setup
│   ├── search_engine.rs   # Search coordinator
│   ├── app_launcher.rs    # App launching (NSWorkspace)
│   ├── file_search.rs     # File search (mdfind)
│   ├── clipboard.rs       # Clipboard monitoring
│   ├── system_commands.rs # System commands
│   ├── calculator.rs      # Expression evaluator
│   ├── settings.rs        # Settings management
│   └── database.rs        # SQLite database
├── ui/                    # Frontend
│   └── index.html         # Main UI
├── icons/                 # App icons
├── Cargo.toml             # Dependencies
├── tauri.conf.json        # Tauri configuration
└── build.rs               # Build script
```

### Running Tests

```bash
cargo test
```

### Code Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

### Enabling Debug Logs

```bash
RUST_LOG=debug cargo run
```

## Performance

### Binary Size

Target: <20MB
Expected: 8-12MB

Optimization flags in `Cargo.toml`:
- `opt-level = "z"` - Optimize for size
- `lto = true` - Link-time optimization
- `codegen-units = 1` - Single compilation unit
- `strip = true` - Strip symbols

### Memory Usage

- Idle: ~30-50MB
- Active search: ~100-150MB
- Clipboard monitoring: ~10MB overhead

### Search Performance

- App search: <10ms (cached)
- File search: 50-200ms (Spotlight)
- Clipboard search: <50ms (SQLite)
- Calculator: <1ms

## Contributing

### Adding New System Commands

1. Edit `src/system_commands.rs`
2. Add to `get_all_commands()` list
3. Implement in `execute()` function

### Adding New Search Sources

1. Create new module (e.g., `src/bookmarks.rs`)
2. Implement search function
3. Add to `src/search_engine.rs`
4. Register Tauri command in `src/main.rs`

### Custom Themes

Submit theme PRs with:
- Theme name
- Color values
- Screenshot

## FAQ

**Q: Can I change the default search behavior?**
A: Yes, edit search result ordering in `src/search_engine.rs`

**Q: How much disk space does clipboard history use?**
A: ~1-5MB for 1000 entries (auto-limited)

**Q: Can I disable clipboard monitoring?**
A: Not currently, but you can clear history: `rm ~/.config/viceroy/clipboard.db`

**Q: Does it work on Apple Silicon?**
A: Yes, builds for both x86_64 and aarch64

**Q: Can I run it on macOS Catalina?**
A: Minimum requirement is macOS 10.15 (Catalina)

## License

MIT License - See LICENSE file

## Credits

- Inspired by Viceroy launcher
- Built with Tauri, Rust, and web technologies
