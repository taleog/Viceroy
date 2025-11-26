# Viceroy - Quick Reference

## 🚀 Quick Start

```bash
# Development
./dev.sh

# Build Release
./build.sh

# Run
./target/release/viceroy
```

## ⌨️ Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Cmd+K` | Open/close launcher |
| `↑` / `↓` | Navigate results |
| `Enter` | Execute selected |
| `Esc` | Close launcher |

## 🔍 Search Examples

| Type | Example | Result |
|------|---------|--------|
| **App** | `safari` | Launch Safari |
| **File** | `document.pdf` | Open file |
| **Calculator** | `2+2*3` | Shows: 8, 0x8, 0b1000, 800% |
| **Command** | `lock` | Lock screen |
| **Clipboard** | `search term` | Find in history |

## 🎨 Configuration Files

| File | Purpose |
|------|---------|
| `~/.config/viceroy/settings.json` | App settings |
| `~/.config/viceroy/clipboard.db` | Clipboard history |

## 🛠️ System Commands

- `lock` - Lock screen
- `sleep` - Sleep computer
- `volume up/down` - Adjust volume
- `mute` - Toggle mute
- `screenshot` - Take screenshot
- `color picker` - Open color picker
- `empty trash` - Empty trash

## 📊 Project Stats

- **Lines of Rust**: ~750
- **Lines of HTML/JS**: ~400
- **Binary Size**: ~8-12MB
- **Dependencies**: 15 direct
- **Search Speed**: <200ms
- **Memory Usage**: ~50MB idle

## 🔧 Development Commands

```bash
# Check compilation
cargo check

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy

# Run with debug logs
RUST_LOG=debug cargo run

# Build release
cargo build --release
```

## 📦 File Structure

```
Viceroy/
├── src/           # Rust backend (9 files)
├── ui/            # HTML frontend (1 file)
├── icons/         # App icons
├── Cargo.toml     # Dependencies
├── tauri.conf.json # Configuration
└── *.sh           # Scripts
```

## 🔐 Required Permissions

1. **Accessibility** - For global hotkeys
2. **Automation** - For app launching
3. **Full Disk Access** - For file search (optional)

## 🐛 Troubleshooting

| Issue | Solution |
|-------|----------|
| Hotkey not working | Check Accessibility permissions |
| Apps won't launch | Check Automation permissions |
| No file results | Rebuild Spotlight: `sudo mdutil -E /` |
| Build fails | Run `cargo clean && cargo build` |

## 📝 Customization

### Change Hotkey
Edit `~/.config/viceroy/settings.json`:
```json
{"hotkey": "CommandOrControl+K"}
```

### Custom Theme
```json
{
  "theme": {
    "background_color": "#1e1e1e",
    "text_color": "#d4d4d4",
    "accent_color": "#007acc"
  }
}
```

## 🎯 Feature Checklist

- [x] Universal search
- [x] App launching
- [x] File search
- [x] Clipboard history
- [x] System commands
- [x] Calculator
- [x] Theming
- [x] Global hotkey
- [ ] Emoji picker
- [ ] Reminders
- [ ] Plugins

## 💡 Tips

- Clipboard history limited to 1000 entries
- App cache refreshes every 5 minutes
- Settings auto-reload on change
- Use calculator for quick conversions
- Search is case-insensitive
- Results sorted by fuzzy match score

## 📚 Documentation

- `README.md` - Overview
- `SETUP.md` - Detailed setup guide
- `CHANGELOG.md` - Version history
- `PROJECT_SUMMARY.md` - Technical details

## 🤝 Contributing

1. Fork the repo
2. Create feature branch
3. Make changes
4. Run `cargo test` and `cargo fmt`
5. Submit PR

## 📄 License

MIT License - Free to use and modify
