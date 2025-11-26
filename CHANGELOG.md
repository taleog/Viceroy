# Changelog

All notable changes to Viceroy will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-11-24

### Added
- Initial release of Viceroy
- Universal search across apps, files, clipboard, and system commands
- Fuzzy matching with SkimMatcherV2
- Clipboard history monitoring and storage
- App launcher with NSWorkspace integration
- File search using macOS Spotlight (mdfind)
- System commands (lock, sleep, volume, etc.)
- Built-in calculator with multiple format display
- Customizable themes and color schemes
- Global hotkey support (default: Cmd+Space)
- System tray integration
- Settings persistence to ~/.config/viceroy/
- SQLite database for clipboard history
- Aggressive size optimization (<20MB target)
- macOS 10.15+ support
- Borderless overlay window with fade animations
- Keyboard navigation for results
- Auto-detection of search intent (calculator, apps, files, etc.)

### Technical Details
- Built with Tauri 1.5 + Rust
- Binary size: ~8-12MB (optimized)
- Search performance: <200ms average
- Clipboard polling: 200ms interval
- Result limit: 50 items (configurable)
- History limit: 1000 clipboard entries

### Known Limitations
- Requires macOS Accessibility permissions for global hotkeys
- Clipboard monitoring is polling-based (not event-driven)
- No cloud sync for clipboard history
- Single window instance only
- Emoji picker not yet implemented
- No plugin/extension system yet

## [Unreleased]

### Planned Features
- [ ] Emoji picker with Unicode 15.0 support
- [ ] Quick reminders integration
- [ ] Superlinks (custom URL scheme handlers)
- [ ] External drive indexing optimization
- [ ] Multi-monitor positioning
- [ ] Keyboard shortcut customization UI
- [ ] Import/export settings
- [ ] Cloud sync for clipboard history
- [ ] Plugin/extension API
- [ ] Smart sorting based on usage frequency
- [ ] Bookmark search integration
- [ ] Contact search
- [ ] Dictionary/translation lookup
- [ ] Unit conversion
- [ ] Currency conversion
- [ ] Time zone conversion
- [ ] Weather lookup
- [ ] Custom actions/workflows
