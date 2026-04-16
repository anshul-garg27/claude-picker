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

echo "==> before (the problem — claude --resume output)"
freeze --execute "bash demo-mockup-before.sh" \
  --output assets/before.png --font.size 14 --width 1200 "${common[@]}"
freeze --execute "bash demo-mockup-before.sh" \
  --output assets/before.svg --font.size 14 --width 1200 "${common[@]}"

echo "==> projects (static project picker)"
freeze --execute "bash demo-mockup-projects.sh" \
  --output assets/projects.png --font.size 14 --width 1200 "${common[@]}"
freeze --execute "bash demo-mockup-projects.sh" \
  --output assets/projects.svg --font.size 14 --width 1200 "${common[@]}"

echo "==> sessions (static session picker with preview)"
freeze --execute "bash demo-mockup-sessions.sh" \
  --output assets/sessions.png --font.size 14 --width 1200 "${common[@]}"
freeze --execute "bash demo-mockup-sessions.sh" \
  --output assets/sessions.svg --font.size 14 --width 1200 "${common[@]}"

echo ""
echo "Done. Images written to assets/"
ls -lh assets/*.png assets/*.svg
