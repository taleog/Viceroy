#!/bin/bash
# Quick rebuild and run script for Viceroy

pkill -9 viceroy 2>/dev/null
cd "$(dirname "$0")"
cargo build --release && \
cp target/release/viceroy Viceroy.app/Contents/MacOS/ && \
open Viceroy.app && \
echo "✓ Viceroy launched"
