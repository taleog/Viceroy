#!/bin/bash
# Clean and rebuild script

echo "Cleaning old builds..."
cd /Users/taleo/Nextcloud/Viceroy
source $HOME/.cargo/env
cargo clean

echo ""
echo "Building release with fixes..."
cargo build --release 2>&1 | tail -30

if [ $? -eq 0 ]; then
    echo ""
    echo "✓ Build successful!"
    echo ""
    SIZE=$(du -h target/release/viceroy | cut -f1)
    echo "Binary: target/release/viceroy ($SIZE)"
    echo ""
    echo "Test it:"
    echo "  ./target/release/viceroy"
    echo ""
    echo "Hotkey: Cmd+K (changed from Cmd+Space to avoid Spotlight)"
else
    echo ""
    echo "✗ Build failed!"
fi
