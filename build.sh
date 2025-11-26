#!/bin/bash
# Build script for Viceroy

echo "Building Viceroy for release..."
echo ""

# Source cargo environment
source $HOME/.cargo/env

# Build release binary
cargo build --release

# Check if build succeeded
if [ $? -eq 0 ]; then
    echo ""
    echo "✓ Build successful!"
    echo ""
    echo "Binary location: target/release/viceroy"
    
    # Show binary size
    SIZE=$(du -h target/release/viceroy | cut -f1)
    echo "Binary size: $SIZE"
    
    # Strip binary for even smaller size (already done by cargo settings)
    echo ""
    echo "To run the application:"
    echo "  ./target/release/viceroy"
else
    echo ""
    echo "✗ Build failed!"
    exit 1
fi
