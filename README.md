<p align="center">
  <h1 align="center">claude-picker</h1>
  <p align="center">
    <strong>Terminal session manager for Claude Code — written in Rust.</strong><br>
    Twelve screens, one binary. Pivot sessions by file, scrub them like a timeline, one-key AI summaries, and a cost-optimization audit you won't find in any other Claude TUI.
  </p>
  <p align="center">
    <a href="https://crates.io/crates/claude-picker"><img src="https://img.shields.io/crates/v/claude-picker.svg" alt="Crates.io"></a>
    <a href="https://github.com/anshul-garg27/claude-picker/releases"><img src="https://img.shields.io/github/v/release/anshul-garg27/claude-picker.svg" alt="Release"></a>
    <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  </p>
  <p align="center">
    <a href="#install">Install</a> &bull;
    <a href="#features">Features</a> &bull;
    <a href="#commands">Commands</a> &bull;
    <a href="#keyboard">Keyboard</a> &bull;
    <a href="#themes">Themes</a> &bull;
    <a href="#configuration">Config</a> &bull;
    <a href="#how-it-works">How it works</a> &bull;
    <a href="#classic-mode">Classic mode</a>
  </p>
</p>

![claude-picker demo](assets/gifs/hero.gif)

> **Written in Rust. Single static binary (~2.1 MB). 12 screens. 6 themes. Per-model cost tracking, fork-aware tree view, word-level diff, 11-operator filter language, file-centric pivot, time-travel replay, one-key AI summaries, cost-optimization audit. No Python, no fzf, no dependencies.**

<details>
<summary>More feature demos</summary>

| Feature | Demo |
|---------|------|
| `--search` full-text with 11-operator filter language | ![](assets/gifs/search.gif) |
| `--stats` dashboard with heatmaps and histograms | ![](assets/gifs/stats.gif) |
| `--tree` fork tree with drill-down expansion | ![](assets/gifs/tree.gif) |
| `--diff` two sessions with word-level diff | ![](assets/gifs/diff.gif) |
| `--files` file-centric pivot view | ![](assets/gifs/files.gif) |
| `R` time-travel replay of any session | ![](assets/gifs/replay.gif) |
| `Ctrl+A` one-key AI summary via Haiku 4.5 | ![](assets/gifs/summarize.gif) |
| `--audit` cost-optimization report | ![](assets/gifs/audit.gif) |
| `*` bookmark and `Ctrl+E` export to markdown | ![](assets/gifs/bookmarks.gif) |

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

**claude-picker** gives you a Ratatui-powered session manager with labeled borders, rich preview, per-model cost tracking, bookmarks, AI summaries, full-text search with an operator language, file-centric pivot, time-travel replay, side-by-side diff, and a cost-optimization audit — all from one ~2.1 MB binary with zero runtime dependencies.

---

## Install

Pick whichever one you already trust the most. The binary is identical.

```bash
# Homebrew (macOS and Linux)
brew install anshul-garg27/tap/claude-picker

# Shell installer (downloads a prebuilt binary from GitHub Releases)
curl -LsSf https://github.com/anshul-garg27/claude-picker/releases/latest/download/claude-picker-installer.sh | sh

# Cargo (builds from source, needs a Rust toolchain)
cargo install claude-picker

# From source (for contributors)
git clone https://github.com/anshul-garg27/claude-picker.git
cd claude-picker && cargo install --path .
```

Direct downloads for every platform live on the
[Releases page](https://github.com/anshul-garg27/claude-picker/releases).

**Requirements:**
- [Claude Code](https://claude.ai/code) CLI on your PATH
- macOS, Linux, or Windows — no runtime deps
- Rust 1.86+ if you build from source (MSRV)

**What the installer does:**
- Drops the Rust binary into `~/.local/bin/claude-picker`
- Adds a `Ctrl+P` shell keybinding to `.zshrc` / `.bashrc`
- Auto-detects [Warp](https://warp.dev) and installs a tab config

---

## Features

### Three pivots no other Claude TUI has

| Pivot | What it does |
|-------|--------------|
| **`--files` file-centric view** | Every file Claude has ever touched, with a reverse pivot from file → sessions that touched it. Answers "which chats edited `src/auth/middleware.ts`?" in one keystroke. Index cached at `~/.config/claude-picker/file-index.json`. |
| **`R` time-travel replay** | Scrub any session forward and backward as a timeline player with gap capping so long idle stretches compress to a beat. Perfect for reconstructing what you did last Tuesday. |
| **`Ctrl+A` AI summarize** | One keystroke. Claude Haiku 4.5 produces a focused TL;DR of the highlighted session. Cost-gated, and every summary is cached to disk at `~/.config/claude-picker/summaries.json`. |

### Browse and resume

| Feature | Description |
|---------|------------|
| **Project picker** | All directories with Claude sessions, git branch, session count |
| **Session picker** | Two-pane project → session layout with live preview; named sessions float to the top, unnamed ones auto-label from the first user message |
| **Conversation viewer (`v`)** | Full-screen transcript with styled metadata and messages |
| **Fuzzy search** | Instant nucleo 0.5 fuzzy matcher — roughly 6× faster than skim |
| **Multi-select (`Tab`)** | Flag multiple sessions at once for bulk operations |
| **Bookmarks (`*` toggle, `b` filter)** | Pin important sessions; persists in `bookmarks.json` |
| **Rename (`r`)** | Edit the `custom-title` in-place; changes are written back into the JSONL |
| **Open in `$EDITOR` (`o`)** | Drops you into the raw JSONL for when you need to grep offline |
| **Copy (`y` / `Y`)** | `y` copies the session ID, `Y` copies the full content via arboard |
| **Age warnings** | Timestamps turn peach after 7 days, red with a warning icon after 30 days |
| **Permission-mode badges** | Dangerous / acceptEdits / default / plan flagged in the preview pane |
| **Subagent counter** | Parses and surfaces subagent invocations per session |
| **Last-prompt line** | Shows the first 80 chars of the last user message so you recognize sessions at a glance |

### Analyze and audit

| Feature | Description |
|---------|------------|
| **`stats` dashboard** | KPI cards (total cost, tokens, sessions), 24×7 hourly heatmap, GitHub-style monthly activity heatmap, turn-duration histogram, per-project and per-model breakdowns, activity timeline |
| **`tree` view** | Sessions grouped by project with fork relationships (`forkedFrom`) and drill-down expansion (`fork_descendants`, `is_expanded`) |
| **`diff` view** | Side-by-side session compare with a word-level diff toggle powered by `similar 2.6` LCS |
| **`search` view** | Full-text plus an 11-operator filter language: `project:`, `model:`, `cost:>X`, `tokens:>Y`, `has:tools`, `before:`, `after:`, `sub:`, `mode:`, `bookmarked`, and free text. Parser lives in `src/data/search_filters.rs` |
| **`hooks` view** | Every configured Claude Code hook plus execution history |
| **`mcp` view** | Installed MCP servers and tool-call usage across sessions |
| **`checkpoints` view** | File-history checkpoints per session |
| **`files` view** | The pivot. Optional `--project NAME` to scope it |
| **`audit` view** | Cost-optimization report with three heuristics: tool-ratio, cache-efficiency, and model-mismatch |
| **Budget forecast** | Monthly budget stored in `~/.config/claude-picker/budget.toml`, plus a burn-rate forecast |

### Accurate cost tracking

Pricing is verified against the latest Anthropic rates:

| Model | Input ($/MTok) | Output ($/MTok) |
|-------|----------------|-----------------|
| Opus 4.x | $5 | $25 |
| Sonnet 4.x | $3 | $15 |
| Haiku 4.5 | $1 | $5 |
| Opus 3 (legacy) | $15 | $75 |

Cache multipliers: `write_5m = 1.25×` input, `write_1h = 2×` input, `read = 0.1×` input. Tokens are parsed out of every `message.usage` block including `cache_creation.ephemeral_5m_input_tokens`, `cache_creation.ephemeral_1h_input_tokens`, and `cache_read_input_tokens`.

### Integrations and scripting

| Feature | Description |
|---------|------------|
| **`pipe` mode** | `claude-picker pipe` (or `-p` / `--pipe`) writes the selected session ID to stdout — wire it into any script |
| **`ai-titles` batch job** | `claude-picker ai-titles` (or `--ai-titles`) auto-names every unnamed session using Haiku 4.5, with a cost-gated confirmation prompt |
| **`Ctrl+E` export** | Hands the session off to `session-export.py` for clean markdown under `~/Desktop/claude-exports/` |
| **`Enter` resume** | Exec's `claude` via `CommandExt::exec` so it fully replaces the picker process; flags are read from `CLAUDE_PICKER_FLAGS` (defaults to `--dangerously-skip-permissions`) |
| **`--preview-cmd`** | Swap the preview for your own command with `{sid}` and `{cwd}` substitution |
| **Shell keybinding** | `Ctrl+P` launches claude-picker from anywhere |
| **Warp terminal** | One-click from Warp's `+` menu |
| **Claude Code skill** | Available as `/claude-picker` inside Claude Code |

---

## Commands

```bash
claude-picker                       # default picker: project → session with preview
claude-picker stats                 # KPI cards, heatmaps, histograms, breakdowns
claude-picker tree                  # fork-aware session tree with drill-down
claude-picker diff                  # side-by-side compare with word-level diff
claude-picker search                # full-text + 11-operator filter language
claude-picker hooks                 # Claude Code hooks and execution history
claude-picker mcp                   # MCP servers and tool-call usage
claude-picker checkpoints           # file-history checkpoints per session
claude-picker files                 # the file-centric pivot
claude-picker files --project NAME  # scope the pivot to one project
claude-picker audit                 # cost-optimization report
claude-picker pipe                  # print selected session ID to stdout
claude-picker ai-titles             # batch-name unnamed sessions via Haiku 4.5
```

Every subcommand also has a `--flag` alias (`--stats`, `--tree`, `--diff`, `--search` / `-s`, `--hooks`, `--mcp`, `--checkpoints`, `--files`, `--audit`, `--pipe` / `-p`, `--ai-titles`) so old muscle memory keeps working.

### Global flags

| Flag | Purpose |
|------|---------|
| `--theme NAME` | Pick a theme for this run (precedence: flag > env var > config file > default) |
| `--list-themes` | Print every installed theme and exit |
| `--generate-config` | Write a default `config.toml` to `~/.config/claude-picker/` |
| `--config-file PATH` | Load config from an explicit path |
| `--force` | Skip cost-gated confirmation prompts (AI batch jobs) |
| `--preview-cmd CMD` | Override the preview pane; supports `{sid}` and `{cwd}` |
| `--project NAME` | Scope `files`, `search`, and a few others to one project |
| `--classic` | Falls back to the legacy Python + fzf implementation |

---

## Keyboard

Every screen uses the same map. `?` brings up a context-aware help overlay.

| Key | Action |
|-----|--------|
| `j` / `k` or `↓` / `↑` | Move selection |
| `gg` / `G` | Jump to top / bottom (the `gg` chord is tracked via `pending_g` state) |
| `Tab` | Toggle multi-select (a `HashSet` in app state) |
| `Enter` | Exec `claude` on the current session; flags come from `CLAUDE_PICKER_FLAGS` |
| `v` | Open the full-screen conversation viewer |
| `R` | Open the time-travel replay player |
| `r` | Rename the session (writes a new `custom-title` into the JSONL) |
| `o` | Open the session JSONL in `$EDITOR` |
| `y` | Copy session ID (arboard) |
| `Y` | Copy the full session content |
| `Ctrl+A` | AI summarize via Haiku 4.5 (cost-gated, cached on disk) |
| `Ctrl+E` | Export session via `session-export.py` |
| `Space` | Command palette (leader) |
| `?` | Context-aware help overlay |
| `t` | Cycle to the next theme live |
| `/` | Search within the current view |
| `*` | Toggle bookmark |
| `b` | Toggle bookmarks-only filter |
| `q` / `Esc` | Back out or quit |

---

## Themes

Six themes ship in the binary, powered by the `catppuccin 2.7` crate:

- `catppuccin-mocha` (default)
- `catppuccin-macchiato`
- `catppuccin-frappe`
- `catppuccin-latte`
- `tokyonight`
- `gruvbox`

Cycle them live with `t`. List them with `claude-picker --list-themes`. Pick one with any of:

```bash
claude-picker --theme tokyonight          # highest priority
export CLAUDE_PICKER_THEME=gruvbox        # next
# then config.toml ui.theme = "catppuccin-latte"
# then the default (catppuccin-mocha)
```

Precedence is strictly `--theme` flag > `CLAUDE_PICKER_THEME` env var > `config.toml` `[ui].theme` > default.

---

## Usage

### Name your sessions

```bash
claude --name "auth-refactor"
claude --name "fix-bug-123"
```

Named sessions appear at the top with a yellow indicator. Or run `claude-picker ai-titles` once and let Haiku 4.5 name the backlog for you.

### Bookmark important sessions

Press `*` in the picker. Bookmarked sessions pin to the top; toggle bookmarks-only mode with `b`. State lives in `bookmarks.json`.

### Find a session by content

```bash
claude-picker search
```

Free text searches every interactive session. For anything more specific, use the operator language:

```text
kubernetes project:web-app cost:>1.00 model:opus mode:acceptEdits bookmarked
sub:1 has:tools after:2026-04-01 before:2026-04-15 tokens:>50000
```

Operators available: `project:`, `model:`, `cost:>X`, `tokens:>Y`, `has:tools`, `before:`, `after:`, `sub:`, `mode:`, `bookmarked`, plus free text. Parser source: `src/data/search_filters.rs`.

### Pivot from a file to the sessions that touched it

```bash
claude-picker files
claude-picker files --project my-api
```

Lists every file Claude has touched. Drill in and it flips to "sessions that touched this file". The index is cached at `~/.config/claude-picker/file-index.json`.

### Scrub a session like a timeline

Highlight a session and press `R`. Long idle stretches are gap-capped so the playback stays watchable. Step through tool calls, responses, and file edits in order.

### Summarize with one keystroke

Press `Ctrl+A`. Haiku 4.5 produces a focused TL;DR; the cost is shown up front so you can cancel, and the result is cached at `~/.config/claude-picker/summaries.json`. Subsequent `Ctrl+A` on the same session hits the cache for free.

### Audit what's costing you money

```bash
claude-picker audit
```

Runs three heuristics and flags sessions where you could have saved:

- **Tool-ratio** — sessions where tool calls dominate and a cheaper model would've done fine
- **Cache-efficiency** — sessions with weak cache reads that could be prompt-cached
- **Model-mismatch** — Opus on throwaway work, Haiku on reasoning-heavy work

### Compare two sessions

```bash
claude-picker diff
```

Pick two sessions. Toggle word-level diff for the fine-grained view; the LCS is powered by `similar 2.6`.

### View the tree

```bash
claude-picker tree
```

Projects contain sessions; sessions contain forks. Drill-down expansion keeps the view tight until you open a branch.

### Check costs

Every session shows token and cost estimates. `stats` rolls them up by project and model with heatmaps and a turn-duration histogram. Set a monthly budget in `~/.config/claude-picker/budget.toml` and the dashboard forecasts burn-rate against it.

### Export conversations

Press `Ctrl+E` on any session to save it as clean markdown in `~/Desktop/claude-exports/`.

### Pipe to other tools

```bash
# Resume a specific session from a script
claude --resume $(claude-picker pipe)

# Export a session by ID
python3 ~/.claude-picker/lib/session-export.py <session-id>
```

---

## Configuration

All state lives under `~/.config/claude-picker/`:

| File | Purpose |
|------|---------|
| `config.toml` | `[ui] theme = "..."` and other preferences. Generate with `claude-picker --generate-config` |
| `bookmarks.json` | Pinned session IDs |
| `summaries.json` | Cached AI summaries keyed by session ID |
| `file-index.json` | Cached file → sessions index for `--files` |
| `budget.toml` | Monthly budget for the forecast in `stats` |

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

1. **Discover projects** — scans encoded directories, resolves real paths via metadata lookup, the JSONL `cwd` field, and an encode-and-compare fallback
2. **Extract session info** — names from `custom-title` entries, message counts, permission modes, subagent counts, and the last user prompt
3. **Compute cost** — parses `message.usage` including cache-creation and cache-read fields against the pricing table above
4. **Detect forks** — reads `forkedFrom` fields to build parent-child session trees with drill-down expansion
5. **Index files** — walks tool-use events to build the reverse `file → sessions` cache
6. **Filter noise** — skips SDK-based tools via the `entrypoint` field and strips system messages from previews
7. **Render UI** — ratatui 0.28 + crossterm 0.28 with 24-bit colors from the `catppuccin 2.7` crate, unicode-aware widths via `unicode-width 0.2` + `unicode-segmentation 1.12`, fuzzy via `nucleo 0.5`, diff via `similar 2.6`, clipboard via `arboard 3.0`

No data leaves your machine except when you explicitly invoke an AI feature (`Ctrl+A` summarize, `ai-titles`). Every other read is local.

---

## Project Stats

- **28** Rust files
- **~13,000** LOC
- **425** tests
- **~2.1 MB** release binary
- **Rust 1.86+** MSRV
- **12** direct dependencies

### Tech stack

- `ratatui 0.28` + `crossterm 0.28` — TUI engine
- `nucleo 0.5` — fuzzy matcher, ~6× faster than skim
- `catppuccin 2.7` — theme palettes
- `clap 4.5` (derive) — CLI parsing
- `serde` + `serde_json` — JSONL parsing
- `unicode-width 0.2` + `unicode-segmentation 1.12` — correct rendering of CJK, emoji, and ZWJ sequences
- `arboard 3.0` — cross-platform clipboard
- `similar 2.6` — word- and line-level diff
- `toml 0.8` — config and budget
- `chrono` — timestamps
- `anyhow` — error handling

---

## Classic mode

Prefer the original Python + fzf flow? Run `claude-picker --classic`.
Requires Python 3.10+, fzf 0.58+, and `rich`. Still supported, still works,
will be maintained indefinitely for users who don't want the Rust binary.

```bash
# Install the classic scripts side-by-side with the Rust binary:
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
~/.claude-picker/claude-picker --classic
```

The classic wrapper preserves every legacy flag: `--pipe`, `--search`,
`--stats`, `--tree`, `--diff`, and the `Ctrl+B` / `Ctrl+E` / `Ctrl+D`
keybindings inside fzf.

---

## Uninstall

```bash
bash ~/.claude-picker/uninstall.sh
```

---

## Contributing

Contributions welcome. Open an issue or PR.

See [CHANGELOG.md](CHANGELOG.md) for the release history.

If claude-picker saves you time, [star the repo](https://github.com/anshul-garg27/claude-picker) — it helps others find it.

---

## License

MIT

---

<p align="center">
  Built by <a href="https://github.com/anshul-garg27">Anshul Garg</a>
</p>
