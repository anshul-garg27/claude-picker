<h1 align="center">claude-picker</h1>

<p align="center">
  <strong>A terminal session manager for Claude Code.</strong><br>
  Thirteen screens. Ten themes. One Rust binary. Zero runtime dependencies.
</p>

<p align="center">
  <a href="https://crates.io/crates/claude-picker"><img src="https://img.shields.io/crates/v/claude-picker.svg?style=flat-square" alt="crates.io"></a>
  <a href="https://github.com/anshul-garg27/claude-picker/releases"><img src="https://img.shields.io/github/v/release/anshul-garg27/claude-picker.svg?style=flat-square" alt="releases"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="MIT"></a>
  <img src="https://img.shields.io/badge/rust-1.86%2B-orange.svg?style=flat-square" alt="Rust 1.86+">
  <img src="https://img.shields.io/badge/tests-500+-brightgreen.svg?style=flat-square" alt="500+ tests">
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#the-headline-features">Features</a> ·
  <a href="#screens">Screens</a> ·
  <a href="#keyboard">Keyboard</a> ·
  <a href="#themes">Themes</a> ·
  <a href="#configuration">Config</a> ·
  <a href="#how-it-works">How it works</a>
</p>

---

## The problem

Claude Code writes every conversation to disk, but the built-in `/resume` is a flat list of UUIDs:

```
? Pick a conversation to resume
  4a2e8f1c-9b3d-4e7a…  (2 hours ago)
  b7c9d2e0-1f4a-8b6c…  (3 hours ago)
  e5f8a3b1-7c2d-9e0f…  (yesterday)
```

No projects. No preview. No names. No cost. No search. No way to find that one session from last Tuesday where you fixed the auth bug.

**claude-picker** reads those same JSONL files and turns them into thirteen tightly-wired screens: two-pane project/session browser with live preview, fork-aware tree view, word-level diff, file-centric pivot ("which sessions touched `auth.rs`?"), time-travel replay, 11-operator filter language, one-key AI summaries, cost-optimization audit, and a stats dashboard with per-model spend and a GitHub-style activity heatmap.

---

## Install

Three ways. The binary is identical everywhere.

```bash
# 1. Cargo (always-latest from crates.io — no cache lag)
cargo install claude-picker

# 2. Homebrew (macOS + Linux)
brew install anshul-garg27/tap/claude-picker

# 3. Shell installer (curl a prebuilt binary from GitHub Releases)
curl -LsSf https://github.com/anshul-garg27/claude-picker/releases/latest/download/claude-picker-installer.sh | sh
```

Prebuilt binaries for every platform live on the [Releases page](https://github.com/anshul-garg27/claude-picker/releases).

**Requirements**
- [Claude Code](https://claude.ai/code) on your `PATH`
- macOS, Linux, or Windows (no runtime deps)
- Rust 1.86+ if building from source

---

## The headline features

### Three pivots no other Claude TUI has

| | What it does | How you get there |
|---|---|---|
| **File-centric pivot** | Every file Claude has ever touched, with a reverse pivot from file → sessions. Answers *"which chats edited `src/auth/middleware.ts`?"* in one keystroke. Cached at `~/.config/claude-picker/file-index.json`. | `claude-picker files` / `--files` |
| **Time-travel replay** | Scrub any session as a timeline. Gap-capping compresses long idle stretches so playback stays watchable. Comet-trail scrubber so you can see where you were. | `R` on any session |
| **One-key AI summary** | Claude Haiku 4.5 produces a TL;DR of the highlighted session. Cost-gated, cached to disk at `~/.config/claude-picker/summaries.json`, so re-press is free. | `Ctrl+A` |

### Five more you'll reach for daily

- **Cost audit** (`--audit`) flags sessions that could have been cheaper with three heuristics: tool-ratio, cache-efficiency, and model-mismatch.
- **Pinned project slots** (`u` pins, `1`–`9` jumps, `0` clears) — `k9s`-style favorites for the projects you touch every day.
- **Filter ribbon** (`Ctrl-r`) cycles `[ALL] [REPO] [7D] [RUNNING] [FORKED]`. Auto-activates `REPO` when you launch from inside a project directory, `atuin`-style.
- **11-operator filter language** — `project:web cost:>1 model:opus mode:acceptEdits bookmarked sub:1 has:tools after:2026-04-01 tokens:>50k`. Parser in `src/data/search_filters.rs`.
- **Which-key popup** — press `Space` or `g` and wait 250ms; a helix-style grid pops up showing every follow-up key with a description. Nothing to memorize.

---

## Screens

Thirteen of them. Each has its own screen, keyboard context, and help overlay.

| Screen | Launch | What it shows |
|---|---|---|
| **Picker** (default) | `claude-picker` | Two-pane projects → sessions with live preview, filter ribbon, pinned slots |
| **Stats** | `stats` / `--stats` | KPI hero cards (tokens, cost, sessions) with delta chips + inline sparklines, rank-badged per-project table with model-colored stacked bars, GitHub-style 30-day activity heatmap, speed-colored turn-duration histogram with p50/p95/p99 markers, traffic-light budget with per-model pill breakdown |
| **Tree** | `tree` / `--tree` | Session fork tree with jless-style collapsed-node summaries `{3 branches · 127 turns · $4.21}`; `e` / `E` expand / collapse subtree |
| **Diff** | `diff` / `--diff` | Side-by-side session compare; `d` toggles word-level inline diff; `n` / `N` jump between hunks |
| **Search** | `search` / `--search` / `-s` | Full-text plus the 11-operator filter language |
| **Conversation viewer** | `v` on a session | Full-screen transcript with right-edge heatmap gutter colored by cost / duration / tokens (`c` cycles); `Ctrl-e` pipes the current turn to `$EDITOR` |
| **Time-travel replay** | `R` on a session | Timeline scrubber with gap-capping and a 4-position comet trail |
| **Files** | `files` / `--files` | The file-centric pivot. Add `--project NAME` to scope |
| **Hooks** | `hooks` / `--hooks` | Every configured Claude Code hook + execution history |
| **MCP** | `mcp` / `--mcp` | Installed MCP servers + tool-call usage rolled up across sessions |
| **Checkpoints** | `checkpoints` / `--checkpoints` | File-history checkpoints per session |
| **Audit** | `audit` / `--audit` | Cost-optimization report |
| **Task drawer** | `w` (overlay, any screen) | Background jobs with progress bars; `j` / `k` navigate, `x` cancels the focused task |

Plus two scripting modes: `pipe` / `--pipe` / `-p` writes the selected session ID to stdout, and `ai-titles` / `--ai-titles` batch-names every unnamed session via Haiku 4.5 (cost-gated).

---

## Keyboard

Every screen shares the same core map. Press `?` for a context-aware help overlay. Hold any leader key (`Space`, `g`) for 250ms and a helix-style which-key popup appears with every follow-up.

### Navigation

| Key | Action |
|---|---|
| `j` / `k` or `↓` / `↑` | Move selection |
| `gg` / `G` | Top / bottom |
| `3j` `12G` `5dd` | Vim-style count prefix |
| `Ctrl-o` / `Ctrl-i` | Jump back / forward (selection history ring) |
| `Tab` | Multi-select toggle |
| `q` / `Esc` | Back out or quit |

### Action

| Key | Action |
|---|---|
| `Enter` | Exec `claude` on the selected session (replaces this process) |
| `v` | Full-screen conversation viewer |
| `R` | Time-travel replay |
| `r` | Rename session (writes `custom-title` back into JSONL) |
| `o` | Open raw JSONL in `$EDITOR` |
| `y` / `Y` | Copy session ID / full content to clipboard |
| `Ctrl+A` | AI summarize via Haiku 4.5 (cached) |
| `Ctrl-e` | Send current turn (in viewer) to `$EDITOR` |
| `*` / `b` | Toggle bookmark / filter to bookmarks-only |
| `z` / `Z` | Undo / redo (rename today; delete coming) |

### Project + scope

| Key | Action |
|---|---|
| `u` | Pin current project |
| `1`–`9` | Jump to pinned slot |
| `0` | Clear project filter (all projects) |
| `Ctrl-r` | Cycle filter ribbon (`ALL` → `REPO` → `7D` → `RUNNING` → `FORKED`) |
| `/` | Filter within current view |
| `Space` | Command palette (leader) |

### Special (viewer / tree / stats)

| Key | Action |
|---|---|
| `c` (viewer) | Cycle heatmap dimension (cost / duration / tokens) |
| `n` / `N` (viewer) | Jump to next / previous turn boundary |
| `n` / `N` (diff) | Jump to next / previous hunk |
| `e` / `E` (tree) | Expand / collapse subtree recursively |
| `w` / `x` | Toggle task drawer / cancel focused task |
| `t` | Cycle theme live |
| `?` | Help overlay |

---

## Themes

Ten themes ship in the binary. Cycle live with `t`, list with `--list-themes`.

| Theme | Mood |
|---|---|
| `catppuccin-mocha` *(default)* | Purple-forward dark, punchy accents |
| `nord` | Slightly softer cousin of mocha |
| `dracula` | Mid-contrast dark, desaturated |
| `catppuccin-latte` | Cream-light for daylight desks |
| `tokyo-night` | Neon indigo on near-black |
| `gruvbox-dark` | Warm retro, boosted greens |
| `nord-aurora` | Cool polar-night base with brightened aurora accents |
| `rose-pine-moon` | Warm desaturated, WCAG-readable |
| `high-contrast` | AAA (7:1) ratios everywhere, for low-vision use |
| `colorblind-safe` | Blue / orange diff pair — never red-green |

**Precedence**: `--theme` flag > `CLAUDE_PICKER_THEME` env > `config.toml` `[ui].theme` > default.

```bash
claude-picker --theme tokyo-night          # highest priority
export CLAUDE_PICKER_THEME=nord-aurora    # next
# config.toml: [ui] theme = "rose-pine-moon"
```

Every theme carries the same 12 semantic tokens (`cost_green/yellow/amber/red/critical`, `speed_fast/medium/slow/glacial`, `model_opus/sonnet/haiku`) so stats render with consistent meaning across palettes. `colorblind-safe` deliberately maps `cost_green = blue` and `cost_red = orange` — no red-green pairs anywhere.

---

## Accurate cost tracking

Pricing is verified against the latest Anthropic rates:

| Model | Input ($/MTok) | Output ($/MTok) |
|---|---|---|
| Opus 4.x | $5 | $25 |
| Sonnet 4.x | $3 | $15 |
| Haiku 4.5 | $1 | $5 |
| Opus 3 (legacy) | $15 | $75 |

Cache pricing: `write_5m = 1.25×` input, `write_1h = 2×` input, `read = 0.1×` input. Tokens come from every `message.usage` block, including `cache_creation.ephemeral_5m_input_tokens`, `cache_creation.ephemeral_1h_input_tokens`, and `cache_read_input_tokens`.

Set a monthly budget in `~/.config/claude-picker/budget.toml`; the stats dashboard flashes the budget band at >95% of forecast.

---

## Configuration

Everything lives under `~/.config/claude-picker/`. Generate a starter with `claude-picker --generate-config`.

| File | Purpose |
|---|---|
| `config.toml` | `[ui]` theme, `reduce_motion` toggle, other preferences |
| `bookmarks.json` | Pinned session IDs |
| `summaries.json` | Cached AI summaries keyed by session ID |
| `file-index.json` | `--files` pivot index |
| `budget.toml` | Monthly budget for the stats forecast |
| `pinned.toml` | `u`-pinned project slots (1–9) |

### `reduce_motion`

```toml
[ui]
reduce_motion = true
```

Disables every animation — fork-tree reveal, pulsing HUD dot, replay comet trail, peek-mode fade, cursor glide, toast slide. Respects screen-reader and accessibility preferences.

### Claude flags

By default, claude-picker launches `claude` with `--dangerously-skip-permissions`. Override via env:

```bash
export CLAUDE_PICKER_FLAGS=""                  # vanilla permissions
export CLAUDE_PICKER_FLAGS="--model sonnet"    # force sonnet
```

### Global CLI flags

| Flag | Purpose |
|---|---|
| `--theme NAME` | Theme for this run |
| `--list-themes` | Print all themes and exit |
| `--generate-config` | Write a default `config.toml` |
| `--config-file PATH` | Use a non-default config path |
| `--preview-cmd CMD` | Override the preview pane (supports `{sid}` / `{cwd}`) |
| `--project NAME` | Scope `files` and a few others to one project |
| `--force` | Skip cost-gated confirmations (AI batch jobs) |

---

## Usage

### Find a session by content

```bash
claude-picker search
```

Free text, or the operator language:

```text
kubernetes project:web-app cost:>1 model:opus after:2026-04-01 bookmarked
```

### Pivot from a file

```bash
claude-picker files                        # every file, ever
claude-picker files --project my-api       # scoped
```

Drill in and it flips to "sessions that touched this file". Index at `~/.config/claude-picker/file-index.json`.

### Audit what's costing you money

```bash
claude-picker audit
```

- **Tool-ratio** — sessions dominated by tool calls that a cheaper model could've handled
- **Cache-efficiency** — weak cache reads that could be prompt-cached
- **Model-mismatch** — Opus on throwaway work, Haiku on reasoning-heavy work

### Scrub a long session

Highlight a session, press `R`. Gap-capping compresses long idle stretches. Every tool call, file edit, and assistant turn becomes a timeline frame.

### Pipe into other tools

```bash
# resume a specific session from a script
claude --resume $(claude-picker pipe)

# one-shot: jump to the last session that touched auth.rs
claude --resume $(claude-picker files --project my-api --pipe auth.rs)
```

---

## How it works

Claude Code stores sessions in `~/.claude/projects/` as JSONL files. Each project directory is lossy-encoded (`/Users/you/my_project` → `-Users-you-my-project`). Per-session metadata lives in `~/.claude/sessions/`.

claude-picker reads these directly to:

1. **Discover projects** — scans encoded directories, resolves real paths via a three-layer decoder (session-metadata lookup → JSONL `cwd` scan → naive decode fallback)
2. **Extract session info** — names from `custom-title` entries, message counts, permission modes, subagent counts, last user prompt
3. **Compute cost** — parses `message.usage` including cache-creation and cache-read fields against the pricing table above
4. **Detect forks** — follows `forkedFrom` to build parent/child trees with drill-down expansion
5. **Index files** — walks tool-use events to build the reverse `file → sessions` map
6. **Filter noise** — skips SDK-entrypoint sessions and strips system messages from previews
7. **Render** — `ratatui` + `crossterm` with 24-bit color, unicode-correct rendering, ~6× faster fuzzy than skim, LCS word-diff, clipboard via `arboard`, Kitty/iTerm2/halfblock identicon thumbnails, `tachyonfx` animations (all respecting `reduce_motion`)

Nothing leaves your machine unless you explicitly call an AI feature (`Ctrl+A` summarize, `ai-titles`). Everything else is local file IO.

---

## Project stats

- **74** Rust files · **42 k** LOC
- **500+** tests (unit + integration)
- **~2.5 MB** release binary
- **18** direct dependencies
- **Rust 1.86+** MSRV

### Tech stack

- `ratatui` + `crossterm` — TUI engine
- `nucleo` — fuzzy matcher (~6× faster than `skim`)
- `catppuccin` — palette source for 4 of the 10 themes
- `tachyonfx` — shader-style animations (fork reveal, comet trail, peek fade, pulse HUD) — all gated by `reduce_motion`
- `image` — pure-Rust pixel buffer for identicon thumbnails (halfblock rendering, works in every terminal)
- `clap` (derive) — CLI parsing
- `serde` + `serde_json` — JSONL parsing
- `unicode-width` + `unicode-segmentation` — grapheme-safe rendering for CJK and emoji
- `arboard` — cross-platform clipboard
- `similar` — LCS diff (word + line)
- `toml` — config + budget
- `chrono`, `anyhow`, `thiserror` — utilities

---

## Contributing

Open an issue or PR — contributions welcome.

See [CHANGELOG.md](CHANGELOG.md) for the release history. See [BREW-TAP.md](BREW-TAP.md) for Homebrew tap maintenance.

If claude-picker saves you time, [star the repo](https://github.com/anshul-garg27/claude-picker) — it helps others find it.

---

## License

MIT. See [LICENSE](LICENSE).

<p align="center">
  Built by <a href="https://github.com/anshul-garg27">Anshul Garg</a>.
</p>
