#!/bin/bash
# Generate app icon from the refined SVG source
# Requirements: cairosvg (pip install cairosvg) on Linux, or on macOS just uses sips/iconutil
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SVG_SOURCE="$SCRIPT_DIR/viceroy-app-icon-transparent.svg"
PNG_OUTPUT="$SCRIPT_DIR/icon.png"

if [ ! -f "$SVG_SOURCE" ]; then
    echo "Error: source SVG not found at $SVG_SOURCE"
    exit 1
fi

echo "🎨 Rendering icon from SVG..."
if command -v python3 &>/dev/null && python3 -c "import cairosvg" 2>/dev/null; then
    # Linux: use cairosvg for proper RGBA output
    python3 -c "
import cairosvg
cairosvg.svg2png(url='$SVG_SOURCE', write_to='$PNG_OUTPUT', output_width=1024, output_height=1024)
print('icon.png generated (1024x1024 RGBA)')
"
elif command -v sips &>/dev/null; then
    # macOS: convert SVG via intermediate PNG (sips can't read SVG directly)
    echo "sips-based conversion not available — use cairosvg or Inkscape"
    exit 1
else
    echo "No SVG rasterizer available. Install cairosvg: pip install cairosvg"
    exit 1
fi

# If on macOS, also generate icon.icns
if command -v iconutil &>/dev/null; then
    echo "🍎 Generating icon.icns..."
    mkdir -p icon.iconset
    for size in 16 32 128 256 512; do
        sips -z $size $size "$PNG_OUTPUT" --out "icon.iconset/icon_${size}x${size}.png" 2>/dev/null || true
        sips -z $((size*2)) $((size*2)) "$PNG_OUTPUT" --out "icon.iconset/icon_${size}x${size}@2x.png" 2>/dev/null || true
    done
    iconutil -c icns icon.iconset
    rm -rf icon.iconset
    echo "icon.icns generated!"
fi

echo "✅ Done: $PNG_OUTPUT"
