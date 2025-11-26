# App Bundle Build

The `build_app.sh` script packages Viceroy into a proper macOS `.app` bundle.

## Quick Start

```bash
./build_app.sh
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

To add a custom icon:

1. Place a 1024x1024 PNG at `Viceroy.app/Contents/Resources/icon.png`
2. Re-run `./build_app.sh`

The script will auto-generate the `.icns` file if `sips` and `iconutil` are available (standard on macOS).

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
