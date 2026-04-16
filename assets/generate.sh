#!/bin/bash
# Regenerate all freeze-based static mockup images.
# Requires: freeze (brew install charmbracelet/tap/freeze)
# Run from project root: bash assets/generate.sh

set -e
cd "$(dirname "$0")/.."
mkdir -p assets/mockups

common=(
  --theme "catppuccin-mocha"
  --window
  --border.radius 12
  --padding 40
  --shadow.blur 20
  --shadow.x 0
  --shadow.y 8
)

render() {
  local script="$1" name="$2" fontsize="$3" width="$4"
  echo "==> $name"
  freeze --execute "bash $script" \
    --output "assets/mockups/$name.png" --font.size "$fontsize" --width "$width" "${common[@]}"
  freeze --execute "bash $script" \
    --output "assets/mockups/$name.svg" --font.size "$fontsize" --width "$width" "${common[@]}"
}

render scripts/demo-mode-stats.sh    stats    14 1100
render scripts/demo-mode-tree.sh     tree     14 1100
render scripts/demo-mode-diff.sh     diff     13 1300
render scripts/demo-mockup-before.sh before   14 1200
render scripts/demo-mockup-projects.sh projects 14 1200
render scripts/demo-mockup-sessions.sh sessions 14 1200

echo ""
echo "Done. Images written to assets/mockups/"
ls -lh assets/mockups/
