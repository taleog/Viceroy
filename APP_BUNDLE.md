# App Bundle Build

The `build_app.sh` script packages Viceroy into a proper macOS `.app` bundle.
The generated `Viceroy.app` bundle is intentionally not tracked in git.

## Quick Start

```bash
make app
```

This creates `Viceroy.app` in the project root.

## Installation

```bash
# Test locally
open Viceroy.app

# Or install to Applications
cp -r Viceroy.app /Applications/

# Launch from Finder or use hotkey (Cmd+Shift+Space)
```

## Icon (Optional)

The app bundle is built from tracked assets in `icons/`:

- `icons/icon.icns` is copied directly when present
- `icons/icon.png` is used as a fallback and converted into `AppIcon.icns`

To customize the icon:

1. Replace `icons/icon.icns` or `icons/icon.png`
2. Re-run `make app`

The PNG fallback path requires `sips` and `iconutil`, which ship with macOS.

## Distribution

For public release:

1. **Code signing**: `codesign --deep --force --verify --verbose --sign "Developer ID Application: Your Name" Viceroy.app`
2. **Notarization**: Submit to Apple via `xcrun notarytool`
3. **DMG creation**: Use `hdiutil` or a tool like `create-dmg`

## Permissions

On first launch, macOS will prompt for:
- **Accessibility**: Required for global hotkey (Cmd+Shift+Space)
- Go to: System Preferences → Security & Privacy → Privacy → Accessibility → Add Viceroy

## Uninstall

```bash
rm -rf /Applications/Viceroy.app
pkill -9 Viceroy
```
