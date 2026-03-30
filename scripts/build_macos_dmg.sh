#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_NAME="Viceroy"
APP_BUNDLE="$PROJECT_ROOT/$APP_NAME.app"
TAG="${1:?usage: build_macos_dmg.sh <tag>}"
DIST_DIR="$PROJECT_ROOT/dist"
DMG_NAME="$APP_NAME-macOS-$TAG.dmg"
DMG_PATH="$DIST_DIR/$DMG_NAME"
TMP_DIR="$(mktemp -d)"
STAGING_DIR="$TMP_DIR/dmg-root"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if [ ! -d "$APP_BUNDLE" ]; then
  echo "Expected app bundle at $APP_BUNDLE" >&2
  exit 1
fi

mkdir -p "$DIST_DIR" "$STAGING_DIR"
ditto "$APP_BUNDLE" "$STAGING_DIR/$APP_NAME.app"
xattr -cr "$STAGING_DIR/$APP_NAME.app"
ln -s /Applications "$STAGING_DIR/Applications"
rm -f "$DMG_PATH"

hdiutil create \
  -volname "$APP_NAME" \
  -srcfolder "$STAGING_DIR" \
  -ov \
  -format UDZO \
  "$DMG_PATH"

echo "Created $DMG_PATH"
