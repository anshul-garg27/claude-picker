#!/usr/bin/env bash
# demo-mode-tui.sh — launches the Textual prototype with hard-coded demo
# sessions. Ideal for recording GIFs / asciinema / VHS tapes without
# leaking real conversation data.
#
# Usage:
#     scripts/demo-mode-tui.sh
#
# Requires: Textual installed (see prototype-tui/requirements.txt).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
APP="$REPO_ROOT/lib/session-picker-tui.py"

# Prefer the newest python3 on PATH that's >= 3.10. Textual refuses to run
# on 3.9 (which is the system default on macOS 14 / stock Python).
pick_python() {
    for cand in python3.13 python3.12 python3.11 python3.10 python3; do
        if command -v "$cand" >/dev/null 2>&1; then
            v=$("$cand" -c 'import sys; print("%d.%d" % sys.version_info[:2])' 2>/dev/null || echo "0.0")
            major=${v%.*}
            minor=${v#*.}
            if [ "$major" -ge 3 ] && [ "$minor" -ge 10 ]; then
                echo "$cand"
                return 0
            fi
        fi
    done
    return 1
}

PY=$(pick_python) || {
    echo "Need Python 3.10+ for Textual. Install with:" >&2
    echo "    brew install python@3.12" >&2
    exit 1
}

# Verify Textual is importable in that interpreter
if ! "$PY" -c "import textual" >/dev/null 2>&1; then
    echo "Textual not installed for $PY. Install with:" >&2
    echo "    $PY -m pip install --user -r $REPO_ROOT/prototype-tui/requirements.txt" >&2
    exit 1
fi

exec "$PY" "$APP" --demo
