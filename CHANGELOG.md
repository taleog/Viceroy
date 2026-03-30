# Changelog

All notable changes to Viceroy will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.2] - 2026-03-29

### Added
- Development workflow automation with GitHub Actions
- Pre-commit hooks for documentation updates
- Issue and PR templates for consistent contributions
- CONTRIBUTING.md with development guidelines
- Windows desktop app on the shared Rust backend
- Self-hosted `viceroy-sync-server` binary for clipboard sync
- Sync settings UI and persistence on macOS and Windows
- Sync settings migration from legacy flat keys to the nested `sync` section

### Changed
- Version scheme updated to alpha release (0.1.0-alpha.1)
- Repository cleanup for public development: generated `Viceroy.app` is no longer tracked, bundle assets now build from `icons/`, and setup docs now point contributors to source and release-based install paths
- README and docs now describe the branch's Windows app, self-hosted sync server, and cross-device sync setup more accurately

## [0.1.0-alpha.1] - 2024-12-04

### Added
- Initial alpha release
- Universal search across apps, files, clipboard history, system commands, emoji, web search, and calculator
- Clipboard history with text and image support
- App launcher with usage tracking
- File search via Spotlight (`mdfind`)
- System commands (lock, sleep, volume, etc.)
- Calculator with multi-format output (decimal, hex, binary, percentage)
- Emoji picker with keyword search
- Dictionary shortcuts
- Global hotkey support
- Native macOS UI using Rust + Cocoa
- Auto-updater system
- Tab navigation between search and clipboard views

### Fixed
- Clipboard paste determinism with extended write detection window
- Duplicate clipboard entries detection
- Permission prompts on startup (file search disabled by default)

### Known Issues
- Tab responsiveness needs optimization
- File search disabled by default due to permission prompts
- Animation helpers defined but not integrated

[Unreleased]: https://github.com/taleog/Viceroy/compare/v0.1.0-alpha.2...HEAD
[0.1.0-alpha.2]: https://github.com/taleog/Viceroy/releases/tag/v0.1.0-alpha.2
[0.1.0-alpha.1]: https://github.com/taleog/Viceroy/releases/tag/v0.1.0-alpha.1
