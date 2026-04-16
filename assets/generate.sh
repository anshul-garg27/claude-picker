#!/bin/bash
# Generate static PNG + SVG images of print-only claude-picker outputs.
# Requires: freeze (brew install charmbracelet/tap/freeze)
# Run from project root: bash assets/generate.sh

set -e
cd "$(dirname "$0")/.."
mkdir -p assets

common=(
  --theme "catppuccin-mocha"
  --window
  --border.radius 12
  --padding 40
  --shadow.blur 20
  --shadow.x 0
  --shadow.y 8
)

echo "==> stats"
freeze --execute "bash demo-mode-stats.sh" \
  --output assets/stats.png --font.size 14 --width 1100 "${common[@]}"
freeze --execute "bash demo-mode-stats.sh" \
  --output assets/stats.svg --font.size 14 --width 1100 "${common[@]}"

echo "==> tree"
freeze --execute "bash demo-mode-tree.sh" \
  --output assets/tree.png --font.size 14 --width 1100 "${common[@]}"
freeze --execute "bash demo-mode-tree.sh" \
  --output assets/tree.svg --font.size 14 --width 1100 "${common[@]}"

echo "==> diff"
freeze --execute "bash demo-mode-diff.sh" \
  --output assets/diff.png --font.size 13 --width 1300 "${common[@]}"
freeze --execute "bash demo-mode-diff.sh" \
  --output assets/diff.svg --font.size 13 --width 1300 "${common[@]}"

echo ""
echo "Done. Images written to assets/"
ls -lh assets/*.png assets/*.svg
