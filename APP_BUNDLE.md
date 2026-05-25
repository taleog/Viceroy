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

# Launch from Finder or use hotkey (Alt+Space)
```

## Icon (Optional)

The app bundle is built from tracked assets in `icons/`:

- `icons/icon.png` is the preferred source and is converted into `AppIcon.icns`
- `icons/icon.icns` is kept only as a legacy fallback
- the refined SVG source files live alongside the generated assets for reuse in the website

To customize the icon:

1. Replace `icons/icon.png` if you want the bundle icon to change immediately
2. Re-run `make app`

The PNG-to-icns path requires `sips` and `iconutil`, which ship with macOS.

## Distribution

For public release:

1. **Code signing**: `codesign --deep --force --verify --verbose --sign "Developer ID Application: Your Name" Viceroy.app`
2. **Notarization**: Submit to Apple via `xcrun notarytool`
3. **DMG creation**: Use `hdiutil` or a tool like `create-dmg`

## Permissions

On first launch, macOS will prompt for:
- **Accessibility**: Required for the global hotkey (Alt+Space)
- Go to: System Preferences → Security & Privacy → Privacy → Accessibility → Add Viceroy

## Uninstall

```bash
rm -rf /Applications/Viceroy.app
pkill -9 Viceroy
```
