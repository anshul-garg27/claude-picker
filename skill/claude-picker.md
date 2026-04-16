---
name: claude-picker
description: Browse, preview, and resume Claude Code sessions with fzf. Full-text search, token/cost estimates, bookmarks, export, and stats dashboard.
---

# Claude Picker — Session Manager

Use this skill to manage your Claude Code sessions. It provides a terminal-native session browser powered by fzf.

## Installation

If claude-picker is not installed, run:

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

Requirements: `fzf` and `python3`.

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
