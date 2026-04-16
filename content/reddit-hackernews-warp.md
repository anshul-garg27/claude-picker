# Community Posts

---

## Reddit: r/ClaudeAI

**Title:** I kept losing track of my Claude Code sessions, so I built a terminal browser with search, stats, fork detection, and diff — open source

**Body:**

I've been using Claude Code daily for the past few months across 4-5 projects. My biggest frustration wasn't the model or the tooling — it was finding old sessions.

`claude --resume` shows a flat list of UUIDs with timestamps. That's it. No project filtering, no preview, no names. Every time I needed to get back to a specific conversation, I'd click through 3-4 wrong sessions before finding the right one. Sometimes I'd give up and start a new one, losing all that context.

So I got nerd-sniped and spent an afternoon reverse-engineering how Claude Code stores sessions. That was a couple of months ago. The tool has grown since. Here's what it does now.

**The basics (run `claude-picker`):**

- Shows all your project directories with Claude sessions, with git branch, session counts, and activity bars
- Lets you pick a project, then pick a session with a conversation preview panel
- Named sessions (created with `claude --name "foo"`) pin to the top under a "saved" header
- Everything else goes under "recent"
- Fuzzy search, relative timestamps, token count per session, cost estimate for sessions over 10k tokens

**The flags that actually changed my workflow:**

- `claude-picker --search` — full-text search across every message in every session across every project. Type "race condition" or "drizzle migration" and get back a ranked list of sessions with the matching lines highlighted. Opens in the correct project dir.
- `claude-picker --stats` — terminal dashboard: total sessions, total tokens, estimated cost, per-project breakdown with bar charts, activity timeline (today/this week/older), top 5 sessions by token count. Not accounting-grade but useful for "am I using this too much".
- `claude-picker --tree` — sessions grouped by project, forks included. When you use `/branch` or `--fork-session` in Claude Code, the child session gets a `forkedFrom` field. The tree view walks that graph so you can see how conversations split. I had 3 forks off a single refactor I'd completely forgotten about.
- `claude-picker --diff` — pick two sessions, side-by-side comparison (common topics, unique topics, conversation previews). Good for figuring out which fork actually got anywhere.
- `claude-picker --pipe` — dumps the selected session ID to stdout for scripting.

**Keyboard shortcuts inside the picker:**

- `Ctrl+B` — bookmark (pins with a blue ■ above the saved list)
- `Ctrl+E` — export session to clean markdown in `~/Desktop/claude-exports/`
- `Ctrl+D` — delete session, auto-refresh
- `Ctrl+P` — global shortcut installed by the installer, opens the picker from any prompt

**Plus:** filters out non-Claude sessions (I also use Wibey; those were cluttering things up — filters via the `entrypoint` field on each message), auto-names unnamed sessions from the first user message, age warnings after 7/30 days, Catppuccin Mocha theme.

Around 800 lines of bash + python3 + fzf. No dependencies beyond fzf 0.58+ and python3 with the `rich` package (installer handles it).

Install:

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
bash ~/.claude-picker/install.sh
```

There's also a `/claude-picker` Claude Code skill — inside any Claude session you can type `/claude-picker` and swap to another conversation.

The thing that changed my workflow more than the tool itself: I started naming every session. `claude --name "auth-refactor"` takes two seconds and makes sessions instantly findable.

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

Would love feedback. Especially if you've hit pain points I haven't thought about.

---

## Reddit: r/commandline

**Title:** Built a session browser for Claude Code with fzf 0.58+ — labeled borders, two-step picker, nine Python scripts composed into one bash orchestrator

**Body:**

Quick share for anyone on this sub who uses Claude Code from the terminal.

Claude Code saves sessions as JSONL files in `~/.claude/projects/` but the built-in `--resume` only shows UUIDs. No filtering, no preview. So I built a proper picker.

**What's in the box:**

1. Project picker — discovers all directories with Claude sessions, resolves their real paths (Claude's encoding is lossy: `/` and `_` both become `-`, so the resolver tries metadata lookup → JSONL `cwd` field scan → encode-and-compare against known paths)
2. Session picker — relative timestamps, session counts, named sessions pinned to top, conversation preview
3. Full-text search across every message in every session (`--search`)
4. Stats dashboard (`--stats`): token/cost estimates, per-project bar charts, activity timeline
5. Tree view (`--tree`): sessions grouped by project, forks linked via the `forkedFrom` field in each JSONL
6. Diff view (`--diff`): two sessions side by side, common/unique topics
7. `--pipe` for scripting

**The fzf 0.58+ labeled borders trick:**

The new `--border-label` on preview windows lets you put section titles directly on the border. Saves vertical space and looks clean. I use it for "conversation preview", "match context", "stats", and the diff columns. Each subcommand reuses the same fzf wrapper function with different preview commands and border labels.

**Composition:**

The bash entry point is ~150 lines. Everything else is nine Python scripts under `lib/`:

- `session-list.py` — reads JSONL, extracts names (the `custom-title` entry), message counts, timestamps, entrypoint filtering
- `session-list.sh` — wraps session-list.py with ANSI coloring and fzf input formatting
- `session-preview.py` — Rich-formatted conversation preview, strips system noise (hook outputs, command metadata, system reminders)
- `session-search.py` — full-text search with per-session match grouping
- `session-export.py` — session → clean markdown
- `session-stats.py` — the dashboard
- `session-tree.py` — builds the fork graph
- `session-diff.py` — two-session comparison
- `session-bookmarks.py` — bookmark storage

Each script is small and does one thing. The bash orchestrator dispatches to the right one based on flags. Shared theme (Catppuccin Mocha, 24-bit color) so nothing looks out of place.

**Other terminal tricks worth stealing:**

- `execute-silent(rm ...)+reload(list)` for delete-and-refresh in one keystroke
- `--delimiter` + `--with-nth` to hide internal IDs from the display while keeping them accessible to the preview command
- `--color=pointer,prompt,border,gutter` overrides for a cohesive dark theme
- Activity bars (`████`) scaled to the most active project for quick visual ranking
- Section header rows ("── saved ──", "── recent ──") marked as disabled so fzf skips them on Enter
- Installer auto-detects zsh vs bash and drops a `Ctrl+P` binding into the right rc file

Around 800 lines total. MIT licensed.

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
bash ~/.claude-picker/install.sh
```

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

Happy to answer questions about the fzf configuration or the Python composition — took a fair amount of trial and error to get the labeled borders + ANSI colors + preview commands to play nice.

---

## Reddit: r/terminal

**Title:** Terminal UI for browsing Claude Code sessions — Catppuccin Mocha theme, fzf 0.58+ labeled borders, Rich-formatted preview panel

**Body:**

Sharing a tool I built for managing Claude Code sessions. Posting here because the terminal UI aspect might be interesting regardless of whether you use Claude Code.

**The design uses:**

- fzf 0.58+ with `--border-label` for titled preview panels (conversation preview, match context, stats, diff columns all use this)
- Catppuccin Mocha palette (24-bit color, consistent across all subcommands)
- Python Rich for the preview renderer — cyan "you:" labels, yellow "ai:" labels, dimmed system lines
- `--bind` with `execute-silent` + `reload` for delete-and-refresh in one keystroke
- `--delimiter` and `--with-nth` to hide internal IDs while keeping them accessible for preview commands
- Activity bars (`████████`) rendered inline showing session counts per project, scaled to the most active
- Section headers ("── saved ──" / "── recent ──") as disabled rows that fzf skips
- Age warnings — timestamps turn peach after 7 days, red with a warning icon after 30
- Bookmarked sessions render with a blue `■` glyph and pin above the saved list
- Git branch shown next to each project name

**Subcommands (each is its own TUI):**

- `--search` — results list on the left, matching lines with query highlighted on the right
- `--stats` — full-screen dashboard: bar charts, activity timeline, top-5 list
- `--tree` — Unicode box-drawing tree with forks linked via Claude's `forkedFrom` field
- `--diff` — two columns, common and unique topics at the top, conversation previews below

The whole thing is bash + python3 + fzf. Around 800 lines across nine Python scripts plus the bash orchestrator. The theme and preview styling are shared modules so the look stays consistent.

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
bash ~/.claude-picker/install.sh
```

Needs fzf 0.58+ and python3 + rich. MIT licensed.

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

Happy to answer questions about the fzf configuration, the Rich formatting, or how the subcommands compose.

---

## Hacker News: Show HN

**Title:** Show HN: Claude-Picker – Browse, search, and diff Claude Code sessions from your terminal

**URL:** https://github.com/anshul-garg27/claude-picker

**Text:**

Hi HN. I've been using Claude Code daily and kept running into the same problem: the built-in --resume shows a flat list of session UUIDs with no project filtering, no preview, no search.

I spent an afternoon reverse-engineering how Claude Code stores sessions. They're JSONL files in ~/.claude/projects/, keyed by an encoded directory path. Session names are custom-title entries inside the JSONL. Each message has an entrypoint field distinguishing CLI sessions from SDK-based tools. Forked sessions (from /branch or --fork-session) have a forkedFrom field pointing at the parent.

The tool is bash + python3 + fzf, around 800 lines across nine Python scripts and a bash orchestrator.

Technically interesting parts:

- Path resolution is ugly. Claude's encoding replaces both / and _ with -, which is lossy: /my_project and /my/project encode to the same string. Three fallback strategies: metadata lookup, scanning JSONL for cwd fields, and encode-then-compare against known paths. Not pretty but covers every session I've seen.

- Fork detection walks the forkedFrom graph. If a parent is missing (user deleted it), the child becomes a root with an orphan marker. --tree renders the graph with Unicode box-drawing.

- Stats are rough but useful. Token estimate = content_length / 4 (standard cheap approximation). Cost estimate = tokens * 0.000015, a conservative blended rate between input/output pricing. Sessions under 10k tokens don't get a cost line because sub-cent numbers aren't useful. The errors wash out across 60+ sessions, so "am I using this too much" is a meaningful question to ask the data.

- Full-text search (--search) groups hits by session and highlights matches. The one bug I had to fix: resolving the project dir from the JSONL's cwd field before launching Claude, otherwise resume runs in the wrong directory.

- --diff compares two sessions using keyword frequency (stopword-filtered content tokens). Not semantic, but good enough to tell which fork made progress.

- fzf 0.58+ only. Labeled borders on preview panels are worth the hard dependency.

- Entrypoint filtering: "cli" for Claude Code, "sdk-cli" for print mode, "sdk-ts" for SDK tools. One filter to keep the picker focused on Claude Code sessions.

Also included:

- Ctrl+B bookmarks (pin above saved sessions, blue ■ glyph)
- Ctrl+E exports a session to clean markdown
- Ctrl+D delete + reload in one keystroke
- Ctrl+P global shell keybinding (installer sets this up for zsh/bash)
- A /claude-picker Claude Code skill for swapping sessions from inside Claude itself
- Warp + menu tab config

No dependencies beyond fzf 0.58+ and python3 with the rich package (installer handles it). MIT licensed.

I'd appreciate feedback, especially from anyone who's worked with Claude Code's session format. Curious whether there's a cleaner way to resolve the encoded paths, and whether the cost estimate is close enough to real billing to be trustworthy.

---

## Warp Community / Discord

**Title:** Claude Picker — Browse, search, and diff Claude Code sessions from the + menu

**Body:**

Hey Warp fam,

Built a session manager for Claude Code that plugs right into Warp's tab configs.

**The problem:** Claude Code's `--resume` shows UUIDs across all projects. When you have 15+ sessions across multiple repos, finding the right one is painful.

**What it does (click `+` → "Claude Picker"):**

1. Project picker — all directories where you've used Claude Code, with git branch, session counts, and activity bars
2. Session browser — named sessions on top, conversation preview panel, fuzzy search, relative timestamps
3. Token and cost estimates per session (cost annotated on sessions over 10k tokens)

**The flags are where it gets interesting:**

- `claude-picker --search` — full-text search across every message in every session across every project, opens in the right project dir
- `claude-picker --stats` — terminal dashboard with per-project bar charts, activity timeline, top-5 sessions by tokens
- `claude-picker --tree` — sessions grouped by project with fork relationships rendered as a tree (uses Claude Code's `forkedFrom` field from `/branch` or `--fork-session`)
- `claude-picker --diff` — two-session comparison with common/unique topics

**Inside the picker:**

- `Ctrl+B` bookmark, `Ctrl+E` export to markdown, `Ctrl+D` delete, `Ctrl+P` to open from anywhere in your shell

**Install:**

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
bash ~/.claude-picker/install.sh
```

The installer auto-detects Warp and drops a tab config into `~/.warp/tab_configs/`. You'll see "Claude Picker" in the `+` menu immediately. It also wires up `Ctrl+P` in your shell rc (zsh or bash) so you can invoke it from any prompt.

Around 800 lines of bash + python. Catppuccin Mocha theme. MIT licensed. Works in any terminal — the Warp integration is a bonus for us.

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

---

## Posting Strategy

**Order:**
1. Reddit r/ClaudeAI first (Tuesday morning) — biggest relevant audience
2. Twitter thread same day, 2-3 hours later
3. Hacker News Show HN next morning (Wednesday 8-11 AM ET)
4. Reddit r/commandline and r/terminal same day as HN
5. Warp community anytime after

**Critical:**
- Attach a demo GIF to every Reddit post and tweet 1. Ideally show --stats and --tree in a second GIF for follow-up tweets.
- On Reddit, engage with EVERY comment in the first 2 hours
- On HN, answer technical questions with depth — especially about the path encoding, fork detection, and cost estimation methodology
- Don't cross-post the exact same text — each post is written for its platform (HN is drier and more technical, r/ClaudeAI is user-facing, r/commandline digs into the fzf composition, r/terminal is about the UI craft)
- Don't post to all subreddits on the same day (looks spammy)
- Have --search and --tree GIFs ready to drop in replies when people ask "show me what that looks like"
