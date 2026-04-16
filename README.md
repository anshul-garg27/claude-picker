<p align="center">
  <h1 align="center">claude-picker</h1>
  <p align="center">
    <strong>Find, preview, and resume your Claude Code sessions.</strong>
  </p>
  <p align="center">
    A terminal-native session manager for <a href="https://claude.ai/code">Claude Code</a>.<br>
    Browse projects, preview conversations, track costs, and jump back in — from any terminal.
  </p>
  <p align="center">
    <a href="#install">Install</a> &bull;
    <a href="#features">Features</a> &bull;
    <a href="#commands">Commands</a> &bull;
    <a href="#how-it-works">How it works</a>
  </p>
</p>

<!-- TODO: Replace with actual GIF after recording -->
<!-- ![claude-picker demo](assets/demo.gif) -->

---

## The Problem

Claude Code saves every conversation, but finding them again is painful:

```
? Pick a conversation to resume
  4a2e8f1c-9b3d-4e7a... (2 hours ago)
  b7c9d2e0-1f4a-8b6c... (3 hours ago)
  e5f8a3b1-7c2d-9e0f... (yesterday)
```

No project filtering. No preview. No names. Just UUIDs.

**claude-picker** fixes this with a two-step fzf picker, conversation preview, cost tracking, and 20+ features that no other tool has together.

---

## Install

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

**Requirements:**
- [Claude Code](https://claude.ai/code)
- [fzf](https://github.com/junegunn/fzf) (`brew install fzf`)
- Python 3

**What the installer does:**
- Symlinks `claude-picker` to `~/.local/bin/`
- Adds `Ctrl+P` shell keybinding to `.zshrc`
- Auto-detects [Warp](https://warp.dev) and installs tab config

---

## Features

### Browse & Resume

| Feature | Description |
|---------|------------|
| **Project picker** | All directories with Claude sessions, sorted by activity |
| **Session picker** | Named sessions on top, unnamed auto-labeled from first message |
| **Conversation preview** | Last few messages shown in a side panel before you open |
| **Fuzzy search** | Type to filter — powered by fzf |
| **Bookmarks** | `Ctrl+B` to pin important sessions to the top |

### Search & Analyze

| Feature | Description |
|---------|------------|
| **Full-text search** | `--search` greps across ALL sessions in ALL projects |
| **Stats dashboard** | `--stats` shows token usage, cost estimates, activity timeline |
| **Session diff** | `--diff` compares two sessions side-by-side with topic analysis |
| **Session tree** | `--tree` shows all sessions grouped by project, with fork relationships |

### Smart Display

| Feature | Description |
|---------|------------|
| **Token/cost estimates** | Approximate tokens and cost per session, color-coded |
| **Auto-naming** | Unnamed sessions show the first user message as their label |
| **Git branch** | Current branch shown next to each project |
| **Disk usage** | Total `~/.claude/` size and session count in header |
| **Age warnings** | Timestamps turn peach (>7 days) or red with warning icon (>30 days) |
| **Activity bars** | Visual `████` indicators showing session count per project |
| **Relative time** | "5m ago", "2h ago" instead of absolute dates |

### Integrations

| Feature | Description |
|---------|------------|
| **Export to markdown** | `Ctrl+E` saves any session to `~/Desktop/claude-exports/` |
| **Pipe mode** | `--pipe` outputs session ID for scripting |
| **Shell keybinding** | `Ctrl+P` launches claude-picker from anywhere |
| **Warp terminal** | One-click from Warp's `+` menu |
| **Claude Code skill** | Available as a `/claude-picker` skill |
| **Smart filtering** | Only shows Claude CLI sessions (filters out SDK tools) |

---

## Commands

```bash
claude-picker                  # browse projects → sessions → resume
claude-picker --search         # full-text search across all conversations
claude-picker --stats          # terminal dashboard with analytics
claude-picker --tree           # session tree grouped by project
claude-picker --diff           # compare two sessions side-by-side
claude-picker --pipe           # output session ID (for scripting)
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Open / resume session |
| `Ctrl+B` | Toggle bookmark (pinned to top) |
| `Ctrl+E` | Export session to markdown |
| `Ctrl+D` | Delete session |
| `Ctrl+P` | Launch from anywhere (shell keybinding) |
| `Ctrl+C` | Go back / quit |
| Type | Fuzzy search / filter |

---

## Usage Tips

### Name your sessions

```bash
claude --name "auth-refactor"
claude --name "fix-bug-123"
```

Named sessions show at the top with a `●` indicator. Takes 2 seconds, saves you minutes of searching.

### Bookmark important sessions

Press `Ctrl+B` in the picker — bookmarked sessions get a blue `■` pin and appear at the very top, above named sessions.

### Search by content

```bash
claude-picker --search
```

Searches across every message in every session. Type "kubernetes" and find that conversation from last week.

### Check your costs

The picker shows token estimates per session. Sessions over 10k tokens also show a cost estimate (e.g., `~$0.30`). Use `--stats` for a full breakdown.

### Export conversations

Press `Ctrl+E` on any session to save it as clean markdown in `~/Desktop/claude-exports/`.

### Pipe to other tools

```bash
# Resume specific session from a script
claude --resume $(claude-picker --pipe)

# Export a session without opening the picker
python3 ~/.claude-picker/lib/session-export.py <session-id>
```

---

## Configuration

### Claude flags

By default, claude-picker launches Claude with `--dangerously-skip-permissions`. Override:

```bash
export CLAUDE_PICKER_FLAGS=""                    # no flags
export CLAUDE_PICKER_FLAGS="--model sonnet"      # custom model
```

### Warp terminal

The installer auto-detects Warp and adds a tab config. Access via `+` menu → **Claude Picker**.

Manual install:

```bash
cp ~/.claude-picker/warp/claude_picker.toml ~/.warp/tab_configs/
```

---

## How It Works

Claude Code stores sessions in `~/.claude/projects/` as JSONL files. Each project directory is encoded (e.g., `/Users/you/my_project` → `-Users-you-my-project`). Metadata lives in `~/.claude/sessions/`.

claude-picker reads these files to:

1. **Discover projects** — scans all encoded directories, resolves real paths via 3 fallback strategies (metadata lookup, JSONL `cwd` field, encode-and-compare)
2. **Extract session info** — names from `custom-title` entries, message counts, token estimates from content length
3. **Filter noise** — skips SDK-based tools using the `entrypoint` field, strips system messages from previews
4. **Render UI** — ANSI 256-color output (Catppuccin Mocha palette) piped through fzf

No data leaves your machine. Everything is local, read-only (except delete and bookmark).

---

## Project Structure

```
claude-picker/
├── claude-picker            # Main entry point (bash)
├── lib/
│   ├── session-list.py      # Builds fzf session list
│   ├── session-list.sh      # Shell wrapper for list builder
│   ├── session-preview.py   # Conversation preview renderer
│   ├── session-search.py    # Full-text search engine
│   ├── session-export.py    # Markdown exporter
│   ├── session-stats.py     # Analytics dashboard
│   ├── session-tree.py      # Tree visualization
│   ├── session-diff.py      # Session comparison
│   └── session-bookmarks.py # Bookmark manager
├── skill/
│   └── claude-picker.md     # Claude Code skill definition
├── warp/
│   └── claude_picker.toml   # Warp tab config
├── install.sh               # Installer + keybinding setup
├── uninstall.sh             # Clean uninstaller
├── LICENSE                  # MIT
└── README.md
```

---

## Uninstall

```bash
bash ~/.claude-picker/uninstall.sh
```

---

## Contributing

Contributions welcome. Open an issue or PR.

If claude-picker saves you time, [star the repo](https://github.com/anshul-garg27/claude-picker) — it helps others find it.

---

## License

MIT

---

<p align="center">
  Built by <a href="https://github.com/anshul-garg27">Anshul Garg</a>
</p>
