<p align="center">
  <h1 align="center">claude-picker</h1>
  <p align="center">
    <strong>Terminal session manager for Claude Code.</strong><br>
    Browse projects, preview conversations, track token cost per session, and resume with one keystroke — a fzf-powered picker for your Claude sessions.
  </p>
  <p align="center">
    A fast, terminal-native <a href="https://claude.ai/code">Claude Code</a> session manager. Find any Claude conversation by content, see what each session cost, fork-aware tree view, side-by-side diff, full-text search — no build step, no dependency you don't already have.
  </p>
  <p align="center">
    <a href="#install">Install</a> &bull;
    <a href="#features">Features</a> &bull;
    <a href="#commands">Commands</a> &bull;
    <a href="#how-it-works">How it works</a>
  </p>
</p>

![claude-picker demo](assets/gifs/hero.gif)

<details>
<summary>More feature demos</summary>

| Feature | Demo |
|---------|------|
| `--search` full-text across projects | ![](assets/gifs/search.gif) |
| `--stats` dashboard | ![](assets/gifs/stats.gif) |
| `--tree` with fork detection | ![](assets/gifs/tree.gif) |
| `--diff` two sessions side-by-side | ![](assets/gifs/diff.gif) |
| `Ctrl+B` bookmark | ![](assets/gifs/bookmarks.gif) |
| `Ctrl+E` export to markdown | ![](assets/gifs/export.gif) |

</details>

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

**claude-picker** gives you a two-step fzf picker with labeled borders, conversation preview, cost tracking, bookmarks, full-text search, and 20+ features no other tool has together.

---

## Install

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

**Requirements:**
- [Claude Code](https://claude.ai/code)
- [fzf](https://github.com/junegunn/fzf) 0.58+ (`brew install fzf`)
- Python 3 with [Rich](https://github.com/Textualize/rich) (auto-installed by installer)

**What the installer does:**
- Symlinks `claude-picker` to `~/.local/bin/`
- Installs Python `rich` for styled preview panels
- Adds `Ctrl+P` shell keybinding to `.zshrc`
- Auto-detects [Warp](https://warp.dev) and installs tab config

---

## Features

### Browse and Resume

| Feature | Description |
|---------|------------|
| **Project picker** | All directories with Claude sessions, git branch, session count |
| **Session picker** | Named sessions on top, unnamed auto-labeled from first message |
| **Conversation preview** | Rich-formatted preview panel with styled metadata and messages |
| **Fuzzy search** | Type to filter — powered by fzf with labeled borders |
| **Bookmarks** | `Ctrl+B` to pin important sessions to the top with blue icon |

### Search and Analyze

| Feature | Description |
|---------|------------|
| **Full-text search** | `--search` finds messages across ALL sessions in ALL projects |
| **Stats dashboard** | `--stats` shows tokens, costs, per-project breakdown, activity timeline, top sessions |
| **Session diff** | `--diff` picks two sessions and compares topics side-by-side |
| **Session tree** | `--tree` shows sessions grouped by project, detects fork relationships |

### Smart Display

| Feature | Description |
|---------|------------|
| **Token and cost estimates** | Approximate tokens per session, cost shown for sessions over 10k tokens |
| **Auto-naming** | Unnamed sessions show the first user message instead of "session" |
| **Git branch** | Current branch shown next to each project in the project picker |
| **Age warnings** | Timestamps turn peach after 7 days, red with warning icon after 30 days |
| **Catppuccin Mocha UI** | Full 24-bit color theme using fzf 0.58+ labeled borders and styled panels |

### Integrations

| Feature | Description |
|---------|------------|
| **Export to markdown** | `Ctrl+E` saves any session to `~/Desktop/claude-exports/` |
| **Pipe mode** | `--pipe` outputs session ID for scripting |
| **Shell keybinding** | `Ctrl+P` launches claude-picker from anywhere |
| **Warp terminal** | One-click from Warp's `+` menu |
| **Claude Code skill** | Available as `/claude-picker` inside Claude Code |
| **Smart filtering** | Only shows Claude CLI sessions, filters out SDK-based tools |

---

## Commands

```bash
claude-picker                  # browse projects, pick a session, resume
claude-picker --search         # full-text search across all conversations
claude-picker --stats          # terminal dashboard with token and cost analytics
claude-picker --tree           # session tree grouped by project with fork detection
claude-picker --diff           # compare two sessions side-by-side
claude-picker --pipe           # output session ID to stdout for scripting
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Open and resume session |
| `Ctrl+B` | Toggle bookmark (pinned to top with blue icon) |
| `Ctrl+E` | Export session to markdown |
| `Ctrl+D` | Delete session |
| `Ctrl+P` | Launch from anywhere (shell keybinding) |
| `Ctrl+C` | Go back or quit |
| Type anything | Fuzzy search and filter |

---

## Usage

### Name your sessions

```bash
claude --name "auth-refactor"
claude --name "fix-bug-123"
```

Named sessions appear at the top with a yellow `●` indicator. Takes 2 seconds, saves you minutes of searching later.

### Bookmark important sessions

Press `Ctrl+B` in the picker. Bookmarked sessions get a blue `■` pin and appear at the very top, above everything else.

### Search by content

```bash
claude-picker --search
```

Searches across every message in every interactive session. Type "kubernetes" to find that conversation from last week. Automatically navigates to the correct project directory when you select a result.

### View your stats

```bash
claude-picker --stats
```

Shows a terminal dashboard with total sessions, token estimates, per-project breakdown with bar charts, activity timeline (today/this week/older), and top 5 sessions by token usage.

### Compare sessions

```bash
claude-picker --diff
```

Pick two sessions and see a side-by-side comparison with common topics, unique topics per session, and conversation previews from both.

### View session tree

```bash
claude-picker --tree
```

Shows all sessions grouped by project. Detects fork relationships created with `/branch` or `--fork-session` and displays the parent-child tree.

### Check costs

The session picker shows token estimates next to each session. Sessions over 10k tokens also show a cost estimate like `~$0.30`. Use `--stats` for the full breakdown across all projects.

### Export conversations

Press `Ctrl+E` on any session to save it as clean markdown in `~/Desktop/claude-exports/`.

### Pipe to other tools

```bash
# Resume a specific session from a script
claude --resume $(claude-picker --pipe)

# Export a session by ID
python3 ~/.claude-picker/lib/session-export.py <session-id>
```

---

## Configuration

### Claude flags

By default, claude-picker launches Claude with `--dangerously-skip-permissions`. Override this:

```bash
export CLAUDE_PICKER_FLAGS=""                    # no flags
export CLAUDE_PICKER_FLAGS="--model sonnet"      # custom model
```

### Warp terminal

The installer auto-detects Warp and adds a tab config. Access via `+` menu and select **Claude Picker**.

Manual install:

```bash
cp ~/.claude-picker/warp/claude_picker.toml ~/.warp/tab_configs/
```

---

## How It Works

Claude Code stores sessions in `~/.claude/projects/` as JSONL files. Each project directory is encoded (`/Users/you/my_project` becomes `-Users-you-my-project`). Metadata lives in `~/.claude/sessions/`.

claude-picker reads these files to:

1. **Discover projects** — scans encoded directories, resolves real paths via metadata lookup, JSONL `cwd` field, and encode-and-compare fallback
2. **Extract session info** — names from `custom-title` entries, message counts, token estimates from content length
3. **Detect forks** — reads `forkedFrom` fields to build parent-child session trees
4. **Filter noise** — skips SDK-based tools using the `entrypoint` field, strips system messages from previews
5. **Render UI** — fzf 0.58+ with labeled borders, 24-bit Catppuccin Mocha colors, and Rich-formatted preview panels

No data leaves your machine. Everything is local and read-only (except delete and bookmark).

---

## Project Structure

```
claude-picker/
├── claude-picker            # Main entry point (bash)
├── lib/
│   ├── session-list.py      # Builds the fzf session list with sections
│   ├── session-list.sh      # Shell wrapper
│   ├── session-preview.py   # Rich-formatted conversation preview
│   ├── session-search.py    # Full-text search across all projects
│   ├── session-export.py    # Export sessions to clean markdown
│   ├── session-stats.py     # Terminal analytics dashboard
│   ├── session-tree.py      # Tree visualization with fork detection
│   ├── session-diff.py      # Side-by-side session comparison
│   └── session-bookmarks.py # Bookmark manager
├── skill/
│   └── claude-picker.md     # Claude Code skill definition
├── warp/
│   └── claude_picker.toml   # Warp tab config
├── install.sh               # Installer with keybinding and Rich setup
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
