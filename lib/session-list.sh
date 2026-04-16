#!/bin/bash
# Builds the session list for fzf — delegates to Python

export PROJECT_DIR="${PROJECT_DIR:-$HOME/.claude/projects/$(echo "$PWD" | sed 's|[/_]|-|g')}"
export SESSIONS_META_DIR="${SESSIONS_META_DIR:-$HOME/.claude/sessions}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
python3 "$SCRIPT_DIR/session-list.py"
