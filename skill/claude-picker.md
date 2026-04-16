---
name: claude-picker
description: Browse, preview, and resume Claude Code sessions from a single Rust binary. Full-text search, per-model token cost, bookmarks, export, stats dashboard, and fork-aware tree view.
---

# Claude Picker — Session Manager

Use this skill to manage your Claude Code sessions. It runs a Ratatui-powered
session browser written in Rust — one static binary, no runtime deps.

## Installation

If `claude-picker` is not on your PATH, install it with whichever you prefer:

```bash
brew install anshul-garg27/tap/claude-picker                                    # Homebrew
curl --proto '=https' --tlsv1.2 -sSf https://claude-picker.dev/install.sh | sh  # Shell installer
cargo install claude-picker                                                     # From crates.io
```

Requirements: `claude` CLI on PATH. No other runtime deps for the Rust binary.
(Legacy classic mode still needs `fzf` 0.58+ and `python3` with `rich` — see
`claude-picker --classic`.)

## Available Commands

Run these in the terminal:

### Browse Sessions
```bash
claude-picker
```
Two-step flow: pick a project → pick a session → resume it.

### Search Across All Conversations
```bash
claude-picker --search
```
Full-text search across every session in every project. Type to filter.

### View Session Statistics
```bash
claude-picker --stats
```
Terminal dashboard showing: total sessions, token estimates, per-project breakdown, activity timeline, top sessions by cost.

### View Session Tree
```bash
claude-picker --tree
```
All sessions grouped by project. Shows fork relationships when they exist.

### Compare Two Sessions
```bash
claude-picker --diff
```
Pick two sessions and see a side-by-side comparison with common/unique topics.

### Pipe Mode (for scripting)
```bash
claude-picker --pipe
```
Outputs session ID to stdout instead of opening Claude. Use with: `claude --resume $(claude-picker --pipe)`

## Keyboard Shortcuts (inside the picker)

| Key | Action |
|-----|--------|
| `Enter` | Open/resume session |
| `Ctrl+E` | Export session to markdown |
| `Ctrl+B` | Toggle bookmark |
| `Ctrl+D` | Delete session |
| `Ctrl+P` | Launch from anywhere (shell keybinding) |

## Tips

- **Name your sessions:** `claude --name "auth-refactor"` — named sessions appear at the top
- **Bookmark important sessions:** Press `Ctrl+B` to pin them
- **Export conversations:** Press `Ctrl+E` to save as markdown in `~/Desktop/claude-exports/`
- **Check costs:** Token estimates and cost are shown per session
