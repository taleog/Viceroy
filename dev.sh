#!/bin/bash
# Development script for Viceroy

echo "Starting Viceroy in development mode..."
echo ""

# Cleanup function
cleanup() {
    echo ""
    echo "Stopping services..."
    if [ ! -z "$UI_PID" ]; then
        kill $UI_PID 2>/dev/null
    fi
    exit 0
}

# Set up trap to cleanup on exit
trap cleanup EXIT INT TERM

# Start UI server in background
echo "Starting UI server on http://localhost:8080..."
python3 serve_ui.py &
UI_PID=$!

# Wait for server to start
sleep 2

# Check if server started successfully
if ! lsof -i:8080 > /dev/null 2>&1; then
    echo "Error: UI server failed to start on port 8080"
    exit 1
fi

# Run Tauri in dev mode
echo "Starting Tauri application..."
source $HOME/.cargo/env
cargo run
