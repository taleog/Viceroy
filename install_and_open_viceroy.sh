#!/usr/bin/env zsh
set -euo pipefail

# install_and_open_viceroy.sh
# Stops any running Viceroy, builds the app using build_app.sh,
# copies the produced Viceroy.app to /Applications, and opens it.

REPO_ROOT="$(cd "$(dirname "${0}")" >/dev/null 2>&1 && pwd -P)"
APP_NAME="Viceroy.app"
BUILD_OUT="$(mktemp -d "${TMPDIR:-/tmp}/viceroy-install.XXXXXX")"
trap 'rm -rf "$BUILD_OUT"' EXIT
APP_SRC_PATH="$BUILD_OUT/$APP_NAME"
APP_DST="/Applications/$APP_NAME"

echo "Stopping running Viceroy (if any)..."
if pgrep -x "Viceroy" >/dev/null 2>&1; then
  pkill -x "Viceroy" || true
  sleep 0.5
fi

echo "Building app using ./build_app.sh..."
if [[ -x "$REPO_ROOT/build_app.sh" ]]; then
  (cd "$REPO_ROOT" && VICEROY_APP_OUT_DIR="$BUILD_OUT" ./build_app.sh)
else
  (cd "$REPO_ROOT" && VICEROY_APP_OUT_DIR="$BUILD_OUT" bash build_app.sh)
fi

if [[ ! -d "$APP_SRC_PATH" ]]; then
  echo "Build did not produce $APP_NAME at $APP_SRC_PATH"
  echo "Error: cannot find built app. Exiting." >&2
  exit 1
fi

echo "Copying $APP_SRC_PATH to $APP_DST (using ditto)..."
if ditto "$APP_SRC_PATH" "$APP_DST" 2>/dev/null; then
  echo "Copied to $APP_DST"
else
  echo "Permission denied or failed. Retrying with sudo..."
  sudo ditto "$APP_SRC_PATH" "$APP_DST"
fi

echo "Opening $APP_DST..."
open -a "$APP_DST"

echo "Done."
