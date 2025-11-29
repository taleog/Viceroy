#!/usr/bin/env zsh
set -euo pipefail

# install_and_open_viceroy.sh
# Stops any running Viceroy, builds the app using build_app.sh,
# copies the produced Viceroy.app to /Applications, and opens it.

REPO_ROOT="$(cd "$(dirname "${0}")" >/dev/null 2>&1 && pwd -P)"
APP_NAME="Viceroy.app"
APP_SRC_PATH="$REPO_ROOT/$APP_NAME"
APP_DST="/Applications/$APP_NAME"

echo "Stopping running Viceroy (if any)..."
if pgrep -x "Viceroy" >/dev/null 2>&1; then
  pkill -x "Viceroy" || true
  sleep 0.5
fi

echo "Building app using ./build_app.sh..."
if [[ -x "$REPO_ROOT/build_app.sh" ]]; then
  (cd "$REPO_ROOT" && ./build_app.sh)
else
  (cd "$REPO_ROOT" && bash build_app.sh)
fi

if [[ ! -d "$APP_SRC_PATH" ]]; then
  echo "Build did not produce $APP_NAME at $APP_SRC_PATH"
  if [[ -d "$REPO_ROOT/target/release/$APP_NAME" ]]; then
    APP_SRC_PATH="$REPO_ROOT/target/release/$APP_NAME"
    echo "Found app at $APP_SRC_PATH"
  else
    echo "Error: cannot find built app. Exiting." >&2
    exit 1
  fi
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
