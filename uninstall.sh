#!/bin/bash
# claude-picker uninstaller

set -e

INSTALL_DIR="$HOME/.claude-picker"
BIN_DIR="$HOME/.local/bin"

echo ""
echo "  Uninstalling claude-picker..."

rm -f "$BIN_DIR/claude-picker"
rm -rf "$INSTALL_DIR"
rm -f "$HOME/.warp/tab_configs/claude_picker.toml"

echo "  ✓ Removed successfully"
echo ""
