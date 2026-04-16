# claude-picker

**Find, preview, and resume your Claude Code sessions.**

A terminal-native session manager for [Claude Code](https://claude.ai/code). Browse all your projects, preview conversations, and jump back into any session — right from your terminal.

<!-- 
TODO: Replace with actual screenshots/GIFs after recording
![claude-picker demo](assets/demo.gif) 
-->

## Why

Claude Code saves every conversation, but finding and resuming them is painful:

- `claude --resume` shows **all sessions globally** — no way to filter by project
- No preview of what a session was about
- No way to delete old sessions
- Session IDs are UUIDs — meaningless

**claude-picker** fixes all of this.

## Features

- **Project picker** — see all directories where you've used Claude Code, with git branch and disk usage
- **Session picker** — browse sessions with named sessions on top, unnamed auto-labeled from first message
- **Conversation preview** — see the last few messages before opening a session
- **Full-text search** — `claude-picker --search` to search across ALL sessions in ALL projects
- **Token/cost estimates** — see approximate token usage per session, color-coded by cost
- **Auto-naming** — unnamed sessions show the first user message instead of "session"
- **Named session support** — sessions created with `claude --name "feature-x"` show their name prominently
- **Export to markdown** — press `Ctrl+E` to export any session to `~/Desktop/claude-exports/`
- **Delete sessions** — press `Ctrl+D` to remove sessions you don't need
- **Smart filtering** — only shows Claude Code sessions (filters out SDK/third-party tool sessions)
- **Fuzzy search** — powered by fzf, type to filter instantly
- **Git branch display** — see current branch for each project in the project picker
- **Disk usage** — see total `~/.claude/` size and session count
- **Shell keybinding** — `Ctrl+P` to launch from anywhere (installed automatically)
- **Activity bars** — visual indicators showing session count per project
- **Relative timestamps** — "5m ago", "2h ago", "3d ago" instead of dates
- **Warp integration** — optional one-click access from Warp's `+` menu

## Install

**One-line install:**

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

**Requirements:**
- [Claude Code](https://claude.ai/code) (you need sessions to browse)
- [fzf](https://github.com/junegunn/fzf) (`brew install fzf` on macOS)
- Python 3 (comes with macOS/most Linux)

## Usage

```bash
claude-picker              # browse sessions
claude-picker --search     # search across ALL conversations
Ctrl+P                     # keybinding (after install)
```

Two-step flow:

1. **Pick a project** — shows all directories with Claude sessions, current git branch, disk usage
2. **Pick a session** — browse, preview, and resume

### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Open selected session |
| `Ctrl+E` | Export session to markdown (`~/Desktop/claude-exports/`) |
| `Ctrl+D` | Delete selected session |
| `Ctrl+C` | Go back / quit |
| Type anything | Fuzzy search / filter |

### Named sessions

Give your sessions names for easy identification:

```bash
claude --name "auth-refactor"
claude --name "fix-bug-123"
```

Named sessions appear at the top with a `●` indicator. Unnamed sessions appear below under "recent".

## Configuration

### Claude flags

By default, claude-picker launches Claude with `--dangerously-skip-permissions`. Change this by setting:

```bash
export CLAUDE_PICKER_FLAGS=""                    # no flags
export CLAUDE_PICKER_FLAGS="--model sonnet"      # custom flags
```

### Warp terminal

If you use [Warp](https://warp.dev), the installer automatically adds a tab config. Access claude-picker from the `+` menu → "Claude Picker".

To install the Warp integration manually:

```bash
cp ~/.claude-picker/warp/claude_picker.toml ~/.warp/tab_configs/
```

## How it works

Claude Code stores session data in `~/.claude/projects/` as JSONL files. Each project directory is encoded (e.g., `/Users/you/my-project` becomes `-Users-you-my-project`). Session metadata lives in `~/.claude/sessions/`.

claude-picker reads these files to:
1. Discover all projects with Claude sessions
2. Extract session names from `custom-title` entries in JSONL
3. Count messages and compute relative timestamps
4. Render a preview by extracting the last few user/AI messages
5. Filter out non-Claude sessions (e.g., SDK-based tools) using the `entrypoint` field

No data leaves your machine. Everything is local, read-only (except delete).

## Project structure

```
claude-picker/
├── claude-picker           # Main entry point
├── lib/
│   ├── session-list.sh     # Builds the fzf session list
│   ├── session-preview.py  # Generates conversation preview
│   ├── session-search.py   # Full-text search across all sessions
│   └── session-export.py   # Export sessions to markdown
├── warp/
│   └── claude_picker.toml  # Warp tab config (optional)
├── install.sh              # Installer (includes keybinding setup)
├── uninstall.sh            # Uninstaller
├── LICENSE                 # MIT
└── README.md
```

## Uninstall

```bash
bash ~/.claude-picker/uninstall.sh
```

## Contributing

Contributions welcome! Feel free to open issues or PRs.

## License

MIT

---

Built by [Anshul Garg](https://github.com/anshul-garg27)
