#!/bin/bash
# Quick test build script

cd /Users/taleo/Nextcloud/Viceroy
source $HOME/.cargo/env

echo "Building Viceroy..."
cargo build 2>&1 | tail -20
