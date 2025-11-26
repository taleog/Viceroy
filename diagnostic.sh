#!/bin/bash
# Quick diagnostic script

echo "=== Viceroy Diagnostic ==="
echo ""

echo "1. Checking if UI server is running..."
if lsof -i:8080 > /dev/null 2>&1; then
    echo "✓ UI server is running on port 8080"
else
    echo "✗ UI server is NOT running"
fi

echo ""
echo "2. Checking if Viceroy process is running..."
if pgrep -x "viceroy" > /dev/null; then
    echo "✓ Viceroy is running"
    echo "  PID: $(pgrep -x viceroy)"
else
    echo "✗ Viceroy is NOT running"
fi

echo ""
echo "3. Checking database..."
if [ -f ~/.config/viceroy/clipboard.db ]; then
    echo "✓ Database exists"
    ls -lh ~/.config/viceroy/clipboard.db
else
    echo "✗ Database not found"
fi

echo ""
echo "4. Checking settings..."
if [ -f ~/.config/viceroy/settings.json ]; then
    echo "✓ Settings exist"
    cat ~/.config/viceroy/settings.json
else
    echo "✗ Settings not found (will be created on first run)"
fi

echo ""
echo "5. Testing UI server..."
curl -s http://localhost:8080 | head -5

echo ""
echo "6. Checking permissions..."
echo "You may need to grant:"
echo "  - Accessibility (for global hotkeys)"
echo "  - Automation (for app launching)"
echo "  - Full Disk Access (for file search)"
echo ""
echo "Check: System Settings > Privacy & Security"
