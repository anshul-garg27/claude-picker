#!/bin/bash
# Extract key static frames from the rendered GIFs.
# Requires: ffmpeg. Run from project root: bash assets/extract-frames.sh

set -e
cd "$(dirname "$0")/.."
mkdir -p assets/frames

extract() {
  local gif="$1" ts="$2" out="$3"
  ffmpeg -y -ss "$ts" -i "assets/gifs/$gif" -vframes 1 -q:v 2 "assets/frames/$out" 2>/dev/null && echo "  $out"
}

echo "==> hero.gif"
extract hero.gif 3.2   hero-01-projects.png
extract hero.gif 5.0   hero-02-sessions.png
extract hero.gif 7.5   hero-03-preview.png
extract hero.gif 10.0  hero-04-selected.png

echo "==> search.gif"
extract search.gif 2.5  search-01-query.png
extract search.gif 4.5  search-02-browsing.png
extract search.gif 8.0  search-03-selected.png

echo "==> stats.gif"
extract stats.gif 3.0   stats-dashboard.png

echo "==> tree.gif"
extract tree.gif 4.0    tree-full.png

echo "==> diff.gif"
extract diff.gif 4.0    diff-full.png

echo "==> bookmarks.gif"
extract bookmarks.gif 3.5  bookmarks-01-browsing.png
extract bookmarks.gif 6.0  bookmarks-02-pinned.png
extract bookmarks.gif 9.0  bookmarks-03-top.png

echo "==> export.gif"
extract export.gif 3.5   export-01-selected.png
extract export.gif 6.5   export-02-exported.png

echo ""
echo "Done."
ls -lh assets/frames/ | tail -n +2 | awk '{printf "  %s  %s\n", $5, $NF}'
