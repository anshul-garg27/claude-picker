#!/usr/bin/env python3
"""Bookmark management for Claude Code sessions.

Manages a bookmarks file at ~/.claude-picker/bookmarks.json.
When called with a session_id, toggles bookmark on/off.
When called with --list, outputs bookmarked session IDs.
"""

import json, os, sys

BOOKMARKS_DIR = os.path.expanduser('~/.claude-picker')
BOOKMARKS_FILE = os.path.join(BOOKMARKS_DIR, 'bookmarks.json')


def load_bookmarks():
    """Load bookmarks from file."""
    if not os.path.exists(BOOKMARKS_FILE):
        return []
    try:
        with open(BOOKMARKS_FILE) as f:
            data = json.load(f)
            return data if isinstance(data, list) else []
    except:
        return []


def save_bookmarks(bookmarks):
    """Save bookmarks to file."""
    os.makedirs(BOOKMARKS_DIR, exist_ok=True)
    with open(BOOKMARKS_FILE, 'w') as f:
        json.dump(bookmarks, f, indent=2)


def toggle_bookmark(session_id):
    """Toggle bookmark for a session. Returns True if bookmarked, False if removed."""
    bookmarks = load_bookmarks()
    if session_id in bookmarks:
        bookmarks.remove(session_id)
        save_bookmarks(bookmarks)
        return False
    else:
        bookmarks.append(session_id)
        save_bookmarks(bookmarks)
        return True


def list_bookmarks():
    """Print all bookmarked session IDs, one per line."""
    for sid in load_bookmarks():
        print(sid)


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: session-bookmarks.py <session_id> | --list", file=sys.stderr)
        sys.exit(1)

    arg = sys.argv[1].strip()

    if arg == '--list':
        list_bookmarks()
    elif arg.startswith('__'):
        # Skip special entries
        sys.exit(0)
    else:
        added = toggle_bookmark(arg)
        status = 'bookmarked' if added else 'unbookmarked'
        print(status)
