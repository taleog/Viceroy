#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
APP_NAME="${VICEROY_APP_NAME:-Viceroy}"
BUNDLE_ID="com.viceroy.app"
ICON_PNG_SOURCE="$PROJECT_ROOT/icons/icon.png"
ICON_ICNS_SOURCE="$PROJECT_ROOT/icons/icon.icns"
OUTPUT_ROOT="${VICEROY_APP_OUT_DIR:-$PROJECT_ROOT}"
OUTPUT_APP_DIR="$OUTPUT_ROOT/$APP_NAME.app"
BUILD_PROFILE="${VICEROY_BUILD_PROFILE:-release}"
STAGING_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/viceroy-app.XXXXXX")"
STAGING_APP_DIR="$STAGING_ROOT/$APP_NAME.app"
CONTENTS_DIR="$STAGING_APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"
export COPYFILE_DISABLE=1

if [[ "$BUILD_PROFILE" == "release" ]]; then
    CARGO_BUILD_ARGS=(--release)
    BINARY_PATH="$PROJECT_ROOT/target/release/viceroy"
else
    BINARY_PATH="$PROJECT_ROOT/target/debug/viceroy"
fi

cleanup() {
    rm -rf "$STAGING_ROOT"
}
trap cleanup EXIT

# Extract version from Cargo.toml
VERSION=$(grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')

if [[ "$BUILD_PROFILE" == "release" ]]; then
    echo "🔨 Building release binary..."
else
    echo "🔨 Building $BUILD_PROFILE binary..."
fi
if [[ "$BUILD_PROFILE" == "release" ]]; then
    cargo build --release
else
    cargo build
fi

echo "📦 Creating app bundle structure..."
mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

echo "📋 Copying binary..."
/bin/cp -X "$BINARY_PATH" "$MACOS_DIR/$APP_NAME"
chmod +x "$MACOS_DIR/$APP_NAME"

echo "📝 Creating Info.plist..."
cat > "$CONTENTS_DIR/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
EOF

# Copy a tracked icon asset into the bundle so the generated app does not rely
# on any files inside the ignored Viceroy.app directory.
#
# Prefer the refined PNG source: macOS can convert it to AppIcon.icns during
# bundle creation, while the legacy .icns file remains as a fallback.
if [ -f "$ICON_PNG_SOURCE" ]; then
    echo "🎨 Converting PNG icon..."
    mkdir -p "$RESOURCES_DIR/AppIcon.iconset"

    for size in 16 32 128 256 512; do
        sips -z $size $size "$ICON_PNG_SOURCE" --out "$RESOURCES_DIR/AppIcon.iconset/icon_${size}x${size}.png" 2>/dev/null || true
        [ $size -le 512 ] && sips -z $((size*2)) $((size*2)) "$ICON_PNG_SOURCE" --out "$RESOURCES_DIR/AppIcon.iconset/icon_${size}x${size}@2x.png" 2>/dev/null || true
    done

    iconutil -c icns "$RESOURCES_DIR/AppIcon.iconset" -o "$RESOURCES_DIR/AppIcon.icns" 2>/dev/null || true
    rm -rf "$RESOURCES_DIR/AppIcon.iconset"

    /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string 'AppIcon'" "$CONTENTS_DIR/Info.plist" 2>/dev/null || \
    /usr/libexec/PlistBuddy -c "Set :CFBundleIconFile 'AppIcon'" "$CONTENTS_DIR/Info.plist"
elif [ -f "$ICON_ICNS_SOURCE" ]; then
    echo "🎨 Copying legacy ICNS icon..."
    /bin/cp -X "$ICON_ICNS_SOURCE" "$RESOURCES_DIR/AppIcon.icns"
    /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string 'AppIcon'" "$CONTENTS_DIR/Info.plist" 2>/dev/null || \
    /usr/libexec/PlistBuddy -c "Set :CFBundleIconFile 'AppIcon'" "$CONTENTS_DIR/Info.plist"
fi

echo "🧹 Clearing bundle attributes..."
/usr/bin/xattr -cr "$STAGING_APP_DIR" || true
/usr/bin/xattr -d com.apple.FinderInfo "$STAGING_APP_DIR" 2>/dev/null || true
/usr/bin/xattr -d com.apple.fileprovider.fpfs#P "$STAGING_APP_DIR" 2>/dev/null || true
/usr/bin/xattr -d com.apple.provenance "$STAGING_APP_DIR" 2>/dev/null || true
/usr/bin/find "$STAGING_APP_DIR" -exec /usr/bin/xattr -c {} + 2>/dev/null || true

if [[ "${VICEROY_SKIP_CODESIGN:-0}" != "1" ]]; then
    echo "🔏 Applying ad-hoc app bundle signature..."
    /usr/bin/codesign --force --deep --sign - "$STAGING_APP_DIR"
    /usr/bin/codesign --verify --deep --strict "$STAGING_APP_DIR"
fi

echo "📤 Exporting app bundle..."
mkdir -p "$OUTPUT_ROOT"
rm -rf "$OUTPUT_APP_DIR"
/usr/bin/ditto "$STAGING_APP_DIR" "$OUTPUT_APP_DIR"

echo "✅ App bundle created: $OUTPUT_APP_DIR"

echo ""
echo "📌 Next steps:"
echo "   1. Test: open $OUTPUT_APP_DIR"
echo "   2. Install: cp -r $OUTPUT_APP_DIR /Applications/"
echo "   3. Launch: Press Cmd+Shift+Space"
echo ""
echo "🔐 Note: First launch may require granting Accessibility permissions"
echo "   System Preferences → Security & Privacy → Privacy → Accessibility"
