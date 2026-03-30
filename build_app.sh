#!/bin/bash
set -e

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
APP_NAME="Viceroy"
BUNDLE_ID="com.viceroy.app"
ICON_ICNS_SOURCE="$PROJECT_ROOT/icons/icon.icns"
ICON_PNG_SOURCE="$PROJECT_ROOT/icons/icon.png"
# Extract version from Cargo.toml
VERSION=$(grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')

echo "🔨 Building release binary..."
cargo build --release

echo "📦 Creating app bundle structure..."
APP_DIR="$PROJECT_ROOT/$APP_NAME.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

# Clean and create directories
rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

echo "📋 Copying binary..."
cp "$PROJECT_ROOT/target/release/viceroy" "$MACOS_DIR/$APP_NAME"
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
if [ -f "$ICON_ICNS_SOURCE" ]; then
    echo "🎨 Copying app icon..."
    cp "$ICON_ICNS_SOURCE" "$RESOURCES_DIR/AppIcon.icns"
    /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string 'AppIcon'" "$CONTENTS_DIR/Info.plist" 2>/dev/null || \
    /usr/libexec/PlistBuddy -c "Set :CFBundleIconFile 'AppIcon'" "$CONTENTS_DIR/Info.plist"
elif [ -f "$ICON_PNG_SOURCE" ]; then
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
fi

echo "✅ App bundle created: $APP_DIR"

echo "🧹 Clearing bundle attributes..."
xattr -cr "$APP_DIR"

echo "🔏 Applying ad-hoc app bundle signature..."
codesign --force --deep --sign - "$APP_DIR"
xattr -cr "$APP_DIR"
codesign --verify --deep --strict "$APP_DIR"

echo ""
echo "📌 Next steps:"
echo "   1. Test: open $APP_DIR"
echo "   2. Install: cp -r $APP_DIR /Applications/"
echo "   3. Launch: Press Cmd+Shift+Space"
echo ""
echo "🔐 Note: First launch may require granting Accessibility permissions"
echo "   System Preferences → Security & Privacy → Privacy → Accessibility"
