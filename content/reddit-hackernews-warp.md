# Community Posts

---

## Reddit: r/ClaudeAI

**Title:** I built a session manager for Claude Code — browse, preview, and resume conversations from any project

**Body:**

Hey everyone,

I've been using Claude Code daily for the past few months. One thing that kept bugging me: session management is basically non-existent.

`claude --resume` shows a flat list of UUIDs across all projects. No preview, no filtering, no way to tell sessions apart unless you named them.

So I built **claude-picker** — a terminal tool that:

- Shows all your projects with Claude sessions (sorted by recent activity)
- Lists sessions per project with names, timestamps, and message counts
- Shows a **conversation preview** panel so you can see what a session was about before opening it
- Lets you fuzzy-search and delete sessions
- Filters out non-Claude sessions (SDK tools, etc.)

**How it works:** I reverse-engineered Claude Code's session storage format (`~/.claude/projects/` has JSONL files, `~/.claude/sessions/` has metadata). The tool reads these files and presents them in a polished fzf interface.

**432 lines total.** No dependencies beyond fzf and python3. Works in any terminal.

Install:

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

Would love feedback. If you use Claude Code regularly, give it a try and let me know what you think.

---

## Reddit: r/commandline

**Title:** claude-picker: a fzf-based session manager for Claude Code (browse, preview, resume conversations)

**Body:**

Built a session manager for Claude Code using bash, python3, and fzf.

**The problem:** Claude Code stores sessions as JSONL files in `~/.claude/projects/` but the built-in `--resume` flag just shows a flat list of UUIDs. No project filtering, no preview, no names.

**What it does:**

- Two-step fzf picker: project → session
- Conversation preview in fzf's preview window (extracts last few messages from JSONL)
- Named sessions (via `claude --name "..."`) appear on top
- Ctrl+D to delete, fuzzy search to filter
- 256-color ANSI output with visual hierarchy
- Auto-detects Warp terminal for tab config integration

**How it works:**

Claude encodes directory paths by replacing `/` and `_` with `-`. Sessions are JSONL files with `custom-title` entries for names and `entrypoint` fields to distinguish CLI sessions from SDK tools. The tool uses three fallback strategies to resolve encoded paths back to real directories.

432 lines. bash + python3 + fzf. MIT licensed.

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

---

## Reddit: r/terminal

**Title:** Built a polished fzf picker for browsing Claude Code sessions — with conversation previews and ANSI colors

**Body:**

Sharing a tool I built to manage Claude Code sessions from the terminal.

The interesting part (from a terminal UI perspective):

- Full 256-color palette using `\033[38;5;XXXm` codes
- fzf with `--preview` running a Python script for conversation rendering
- `--bind` with `execute-silent` + `reload` for delete-and-refresh
- Visual activity bars (`████████`) showing session counts per project
- Relative timestamps, section headers, icons

It's 432 lines across 3 files (bash orchestrator + bash list builder + python preview renderer). Works in any terminal that supports 256 colors.

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

---

## Hacker News: Show HN

**Title:** Show HN: Claude-Picker – Terminal session manager for Claude Code (fzf + bash)

**URL:** https://github.com/anshul-garg27/claude-picker

**Text (optional):**

Claude Code stores conversations as JSONL files in ~/.claude/projects/ but the built-in --resume flag shows a flat list of UUIDs with no project filtering or preview.

claude-picker is a 432-line terminal tool (bash + python3 + fzf) that adds:

- Project-level session browsing
- Conversation preview (extracts messages from JSONL)
- Named session support
- Fuzzy search and delete
- Smart filtering (CLI sessions only, excludes SDK tools)

I reverse-engineered the session storage format to build this. The interesting technical bits: lossy path encoding (/ and _ both become -), three-layer path resolution, and JSONL noise filtering for clean previews.

Works in any terminal. Optional Warp tab config integration.

---

## Warp Community Post

**Title:** Claude Picker — Browse and resume Claude Code sessions from the + menu

**Body:**

Hey Warp community!

I built a session manager for Claude Code that integrates with Warp's tab configs.

**What it does:**
Click `+` → "Claude Picker" and you get a two-step fzf flow:
1. Pick a project directory (shows all dirs with Claude sessions)
2. Browse sessions with names, message counts, and a **live conversation preview**

**Features:**
- Named sessions appear on top (use `claude --name "feature-x"` to name them)
- Fuzzy search to find sessions instantly
- Ctrl+D to delete old sessions
- Preview panel shows the last few messages so you know what a session was about
- Only shows Claude Code sessions (filters out SDK-based tools)
- Activity bars showing how many sessions each project has

**Install:**

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

The installer auto-detects Warp and adds the tab config to `~/.warp/tab_configs/`.

It also works in any other terminal — the Warp integration is a bonus, not a requirement.

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

Would love to hear what you think!
