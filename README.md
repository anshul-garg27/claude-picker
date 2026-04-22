<h1 align="center">claude-picker</h1>

<p align="center">
  <strong>A terminal session manager for Claude Code.</strong><br>
  Fifteen themes. Seventeen commands. One 3.7 MB Rust binary. Zero runtime dependencies.
</p>

<p align="center">
  <a href="https://crates.io/crates/claude-picker"><img src="https://img.shields.io/crates/v/claude-picker.svg?style=flat-square" alt="crates.io"></a>
  <a href="https://github.com/anshul-garg27/claude-picker/releases"><img src="https://img.shields.io/github/v/release/anshul-garg27/claude-picker.svg?style=flat-square" alt="releases"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="MIT"></a>
  <img src="https://img.shields.io/badge/rust-1.86%2B-orange.svg?style=flat-square" alt="Rust 1.86+">
  <img src="https://img.shields.io/badge/edition-2021-blue.svg?style=flat-square" alt="Rust edition 2021">
  <img src="https://img.shields.io/badge/tests-714%20passing-brightgreen.svg?style=flat-square" alt="714 tests">
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#first-run">First run</a> ·
  <a href="#whats-new-in-v06">v0.6</a> ·
  <a href="#screens">Screens</a> ·
  <a href="#keyboard">Keyboard</a> ·
  <a href="#themes">Themes</a> ·
  <a href="#audit--stats-deep-dive">Audit</a> ·
  <a href="#scripting--shell-integration">Scripting</a> ·
  <a href="#configuration">Config</a> ·
  <a href="#how-it-works">How it works</a>
</p>

<p align="center">
  <img src="assets/hero-marketing.png" alt="claude-picker key art — a Kanagawa-palette terminal window on a sumi-ink canvas showing three model-cost rows (opus $1,357.80, sonnet $128.44, haiku $12.18), set against a hand-painted moon and Japanese washi texture. Kanji on the right reads 選択はコストなり (selection is cost)." width="92%">
</p>

<p align="center">
  <img src="assets/hero.gif" alt="claude-picker cold-start: Kanagawa-themed picker with fuzzy filter and model/permission pills; tabbing to the audit dashboard surfaces two tool-ratio findings worth $64 with an annual run-rate projection of ~$779/year; the stats dashboard lights up with 7×24 usage heatmap and per-project 30-day cost bars; finishing with a single-line `claude-picker prompt` suitable for shell-prompt integration" width="88%">
</p>

<p align="center"><sub>
  Full-resolution: <a href="assets/hero.mp4">assets/hero.mp4</a> · 1920×1200 · 35 s · 563 KB.<br>
  Want to regenerate against your own sandbox? See <a href="../scripts/capture/tapes/01-hero-v06.tape"><code>scripts/capture/tapes/01-hero-v06.tape</code></a>.
</sub></p>

---

## The 90-second pitch

Claude Code stores every conversation on disk, but the built-in `/resume` picker is a flat list of UUIDs:

```
? Pick a conversation to resume
  4a2e8f1c-9b3d-4e7a…  (2 hours ago)
  b7c9d2e0-1f4a-8b6c…  (3 hours ago)
  e5f8a3b1-7c2d-9e0f…  (yesterday)
```

No projects. No preview. No names. No cost. No search. No way to find that one session from last Tuesday where you fixed the auth bug.

**claude-picker** reads those same JSONL files and wires them into a tightly-designed two-pane picker with live preview, fork-aware tree, word-level diff, file-centric pivot, time-travel replay, 11-operator filter language, and a cost-optimization audit that — in our own dogfood — identified **$148.17 of spend** across findings like *"71% tool_use tokens, Haiku could save ~$73"*. Everything is local. Nothing is scraped. Nothing is sent anywhere unless you explicitly press the AI summary key.

---

## Install

Four ways. The binary is identical everywhere.

```bash
# 1. Cargo (always-latest from crates.io — no cache lag)
cargo install claude-picker

# 2. Cargo from source (this repo)
cargo install --path .

# 3. Homebrew (macOS + Linux)
brew install anshul-garg27/tap/claude-picker

# 4. Shell installer (curl a prebuilt binary from GitHub Releases)
curl -LsSf https://github.com/anshul-garg27/claude-picker/releases/latest/download/claude-picker-installer.sh | sh
```

Prebuilt binaries for every platform live on the [Releases page](https://github.com/anshul-garg27/claude-picker/releases).

**Requirements**
- [Claude Code](https://claude.ai/code) on your `PATH`
- macOS, Linux, or Windows (no runtime deps)
- Rust 1.86+ if building from source — `rust-toolchain.toml` pins the `stable` channel, so `RUSTUP_TOOLCHAIN=stable cargo install --path .` works even if your default toolchain is nightly

---

## First run

```bash
claude-picker
```

Type to fuzzy-filter. `?` pops a context-aware help overlay. `Enter` resumes the highlighted session by exec'ing `claude --resume …`. That's the whole muscle memory — everything else is discoverable from the help key.

---

## What's new in v0.6

Fifteen themes (up from ten), five new headless subcommands, and a round of UI surfaces aimed at making the picker and conversation viewer feel lived-in rather than spreadsheet-y.

- **[Kanagawa is the new default](#themes)** — warm ink-wash palette replaces Catppuccin Mocha as the out-of-box theme. `finance-terminal`, `parchment-dark`, `paperwhite-warm`, and `terminal-classic` round the theme count up to 15.
- **[Doctor, export, latest, prompt, completions](#scripting--shell-integration)** — five new subcommands for scripting and shell integration. `prompt` emits a PS1-friendly spend line. `export --redact` writes a Markdown transcript with secrets masked. `doctor --format json` surfaces orphans and top-cost sessions.
- **[Audit JSON/CSV output](#audit--stats-deep-dive)** — `audit --format json` pipes straight into `jq`, Datadog, or your spreadsheet of choice. The TUI now always shows a 3-heuristic summary band and an **annual-savings run-rate** (monthly × 12.17).
- **[Chain and anomaly badges](#screens) in the session list** — `⛓` marks sessions in the same project opened within 24h with similar titles. `⚡` marks sessions whose cost is ≥2× the project median.
- **[Zebra rows + loading skeletons + cursor memory + smooth scroll](#ui-polish)** — tabular lists alternate shades on dark themes. Cold start shows pulsing grey placeholders for ~1.2s until enumeration settles. Cursor position restores per-project. Scroll interpolates instead of jumping. Every animation respects `reduce_motion`.
- **[Conversation viewer rework](#keyboard)** — every message gets a `HH:MM · +Nm ·` timestamp. `z` toggles **zen mode** (drop breadcrumb, footer, search bar). Subagent Task tool calls render as a tree with `├─` / `└─` / `│ ` connectors. An **interesting-moments mini-timeline** at the top marks cost spikes, tool bursts, long pauses, and the first+last prompts.
- **[Stats dashboard extras](#audit--stats-deep-dive)** — burn-rate alert vs prior month, 7×24 day-of-week × hour-of-day heatmap (`p` cycles metric), optional quota progress bar gated on `[ui] plan_tier`, per-project 30-day cost heatmap.
- **[Auto-redact in preview](#privacy)** — `sk-ant-…`, `AKIA…`, `ghp_…`, `eyJ….….…` JWTs, and `Bearer …` headers are masked as `****<last4>` before rendering. Opt out via `[ui] redact_preview = false`.

---

## Screens

Fourteen first-class screens plus five headless subcommands. Every TUI screen has its own keyboard context and `?` help overlay.

| Screen | Launch | What it shows |
|---|---|---|
| **Picker** (default) | `claude-picker` | Two-pane projects → sessions with live preview, filter ribbon, pinned slots, zebra rows, chain/anomaly badges |
| **Stats** | `stats` / `--stats` | KPI hero cards (tokens, cost, sessions) + burn-rate alert, rank-badged per-project table with model-colored stacked bars, GitHub-style 30-day activity heatmap, project-cost 30-day heatmap, 7×24 day-of-week × hour-of-day heatmap (`p` cycles metric), speed-colored turn-duration histogram with p50/p95/p99 markers, traffic-light budget, optional quota panel (`plan_tier`) |
| **Tree** | `tree` / `--tree` | Session fork tree with jless-style collapsed-node summaries `{3 branches · 127 turns · $4.21}`; `e` / `E` expand / collapse subtree |
| **Diff** | `diff` / `--diff` | Side-by-side session compare with cost delta header; `d` toggles word-level inline diff; `n` / `N` jump between hunks |
| **Search** | `search` / `--search` / `-s` | Full-text plus the 11-operator filter language, with context excerpts around matches |
| **Conversation viewer** | `v` on a session | Full-screen transcript with per-message `HH:MM · +Nm ·` timestamps, interesting-moments mini-timeline, subagent tree, right-edge heatmap gutter (`c` cycles cost / duration / tokens), `z` zen toggle, `Ctrl-e` pipes the current turn to `$EDITOR` |
| **Time-travel replay** | `R` on a session | Timeline scrubber with gap-capping and a 4-position comet trail |
| **Files** | `files` / `--files` | The file-centric pivot. Add `--project NAME` to scope |
| **Hooks** | `hooks` / `--hooks` | Every configured Claude Code hook + execution history |
| **MCP** | `mcp` / `--mcp` | Installed MCP servers + tool-call usage rolled up across sessions |
| **Checkpoints** | `checkpoints` / `--checkpoints` | File-history checkpoints per session |
| **Audit** | `audit` / `--audit` | Cost-optimization report with always-visible 3-heuristic summary band, annual-savings run-rate, per-project cost bars, drill-in per-finding overlay (`--format tui` \| `json` \| `csv`) |
| **AI titles** | `ai-titles` / `--ai-titles` | Batch-name every unnamed session via Haiku 4.5 (cost-gated, cached to `summaries.json`) |
| **Task drawer** | `w` (overlay, any screen) | Background jobs with progress bars; `j` / `k` navigate, `x` cancels the focused task |

### Headless subcommands

| Command | One-line purpose |
|---|---|
| `pipe` | Print selected session ID to stdout (for `claude --resume $(...)` ) |
| `export <sid> [--out PATH] [--redact]` | Export transcript to Markdown; `--redact` masks `sk-ant-…` / `AKIA…` / `ghp_…` / JWTs / `Bearer …` |
| `doctor [--cleanup --yes --format plain\|json\|csv]` | Diagnostic scan of `~/.claude/projects/` — sizes, top sessions, orphan stubs |
| `latest [--project NAME --count N]` | Print the most-recent session id(s) for scripting |
| `prompt [--format PS1\|JSON --no-color]` | Single-line spend summary for embedding in your shell prompt |
| `completions <bash\|zsh\|fish\|elvish\|powershell>` | Emit a shell-completion script |

---

## Keyboard

Every screen shares the same core map. Press `?` for a context-aware help overlay. Hold any leader key (`Space`, `g`) for 250ms and a helix-style which-key popup appears with every follow-up.

### Navigation (all screens)

| Key | Action |
|---|---|
| `j` / `k` or `↓` / `↑` | Move selection |
| `gg` / `G` | Top / bottom |
| `3j` `12G` `5dd` | Vim-style count prefix |
| `Ctrl-o` / `Ctrl-i` | Jump back / forward (selection history ring) |
| `Tab` | Multi-select toggle |
| `q` / `Esc` | Back out or quit |

### Session list

| Key | Action |
|---|---|
| `Enter` | Exec `claude --resume` on the selected session (replaces this process) |
| `v` | Full-screen conversation viewer |
| `R` | Time-travel replay |
| `r` | Rename session (writes `custom-title` back into JSONL) |
| `e` | Export session transcript to Markdown |
| `o` | Open raw JSONL in `$EDITOR` |
| `y` / `Y` | Copy session ID / project path to clipboard |
| `Ctrl+A` | AI summarize via Haiku 4.5 (cached) |
| `m` | Mark / unmark for bulk action |
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

### Conversation viewer

| Key | Action |
|---|---|
| `c` | Cycle heatmap gutter dimension (cost / duration / tokens) |
| `n` / `N` | Jump to next / previous turn boundary |
| `z` | Toggle **zen mode** — drop breadcrumb, footer, search chrome |
| `Ctrl-e` | Send current turn to `$EDITOR` |

### Diff / tree / stats / task drawer

| Key | Action |
|---|---|
| `n` / `N` (diff) | Jump to next / previous hunk |
| `d` (diff) | Toggle word-level inline diff |
| `e` / `E` (tree) | Expand / collapse subtree recursively |
| `p` (stats) | Cycle heatmap metric (cost / tokens / sessions) |
| `w` / `x` | Toggle task drawer / cancel focused task |
| `t` | Cycle theme live |
| `?` | Help overlay |

---

## Themes

Fifteen themes ship in the binary. Cycle live with `t`, list with `--list-themes`. A side-by-side comparison renders at `docs/design/theme-comparison.html` in the workspace root.

| Theme | Mood |
|---|---|
| `kanagawa` *(default)* | Warm ink-wash on dusk blue — Fujiyama-in-a-terminal |
| `finance-terminal` | Bloomberg-orange on graphite black, amber tickers, monospace discipline |
| `parchment-dark` | Aged-paper cream on chocolate base, editorial serif feel |
| `paperwhite-warm` | Cream paper with warm ochre accents — daylight desk mode |
| `catppuccin-mocha` | Purple-forward dark, punchy accents (former default) |
| `catppuccin-latte` | Cream-light sibling of mocha |
| `dracula` | Mid-contrast dark, desaturated |
| `tokyo-night` | Neon indigo on near-black |
| `gruvbox-dark` | Warm retro, boosted greens |
| `nord` | Slightly softer cousin of mocha |
| `nord-aurora` | Cool polar-night base with brightened aurora accents |
| `rose-pine-moon` | Warm desaturated, WCAG-readable |
| `high-contrast` | AAA (7:1) ratios everywhere, for low-vision use |
| `colorblind-safe` | Blue / orange diff pair — never red-green |
| `terminal-classic` | Retro-CRT phosphor green on black (bonus) |

**Precedence**: `--theme` flag > `CLAUDE_PICKER_THEME` env > `config.toml` `[ui].theme` > default (`kanagawa`).

```bash
claude-picker --theme tokyo-night         # highest priority
export CLAUDE_PICKER_THEME=nord-aurora    # next
# config.toml: [ui] theme = "parchment-dark"
```

Every theme carries the same 12 semantic tokens (`cost_green/yellow/amber/red/critical`, `speed_fast/medium/slow/glacial`, `model_opus/sonnet/haiku`) so stats render with consistent meaning across palettes. `colorblind-safe` deliberately maps `cost_green = blue` and `cost_red = orange` — no red-green pairs anywhere.

---

## Audit + stats deep-dive

`claude-picker audit` scores every session in `~/.claude/projects/` against three heuristics and produces a run-rate-aware savings estimate. The TUI has always shown findings — v0.6 adds an always-visible 3-heuristic summary band and a drill-in per-finding overlay with per-tool distribution.

<p align="center">
  <img src="assets/audit-summary-band.svg" alt="Always-visible summary band on the audit dashboard, split into three labeled rectangles: tool-ratio (warn-yellow, 6 findings, ~$110.40), cache-efficiency (info-blue, 1 finding, ~$0.11), model-mismatch (ok-green, 0 findings, $0.00)" width="72%">
</p>

### The three heuristics

- **Tool-ratio** — sessions where `tool_use` tokens dominate the output budget. The ratio is computed as `tool_use / (output + cache-create)`, and anything over 50% is flagged as *"could have been a Haiku call"*.
- **Cache-efficiency** — weak `cache_read` vs `cache_creation` ratio. Sessions that regenerate the same 5-minute ephemeral context over and over get flagged; typical savings from prompt-caching that input.
- **Model-mismatch** — Opus on throwaway work (all-cheap tool calls), or Haiku on heavy reasoning (long free-form plans). Direction-aware so you don't get warned about *"your Opus session should be on Opus"*.

### JSON output (sample from this repo)

```bash
claude-picker audit --format json | jq '.total_savings_usd, .annual_run_rate_usd, (.findings | length)'
```

```json
{
  "total_savings_usd": 148.18,
  "annual_run_rate_usd": 1778.14,
  "findings": [
    {
      "session_id": "1402ab4e-a256-468d-8c66-858c0ddcccb6",
      "project": "architex",
      "session_label": "testing1",
      "total_cost_usd": 1357.81,
      "model_summary": "claude-opus-4-7",
      "kind": "ToolRatio",
      "severity": "warn",
      "message": "71% tool_use tokens — Haiku could save ~$73.00",
      "savings_usd": 72.99
    }
  ]
}
```

### Stats dashboard — project-cost 30-day heatmap

The stats dashboard renders a per-project 30-day heatmap so you can see at a glance which projects eat budget and when.

<p align="center">
  <img src="assets/heatmap-row.svg" alt="One row of the stats project-cost heatmap: project name on the left, 30 daily cells shaded from empty grey through yellow to red based on that day's cost, a total cost in the right margin" width="78%">
</p>

Cells are quantile-shaded (p25 / p50 / p75 / p90 over non-zero days). Press `p` on the stats screen to cycle between cost, tokens, and sessions.

---

## Scripting + shell integration

Four primitives make claude-picker composable with everything else in a shell pipeline.

### `latest` — jump to the most recent session

```bash
$ claude-picker latest --count 3
dc218c2f-b469-40ad-aacd-72aacb18b203
ca0d1766-8eed-4d08-b324-b00e733727a3
f424a1b2-4aef-4bd7-8d8d-f3daa6313a4a

# one-shot resume of the last session in a specific project
claude --resume $(claude-picker latest --project claude-picker)
```

### `prompt` — spend in your PS1

```bash
$ claude-picker prompt
claude: $2343.93 today · $5927.40 month

$ claude-picker prompt --format json
{"today": 2343.93, "month": 5927.40}
```

Add to your shell prompt:

```bash
# bash / zsh
PS1='$(claude-picker prompt --no-color) \$ '

# fish
function fish_prompt
    echo -n (claude-picker prompt --no-color) ' $ '
end
```

### `export --redact` — share a session safely

```bash
claude-picker export 1402ab4e-a256-468d-8c66-858c0ddcccb6 \
  --out ~/Desktop/session.md \
  --redact
```

Writes Markdown with `sk-ant-…` / `AKIA…` / `ghp_…` / JWTs / `Bearer …` masked as `sk-ant-****<last4>` etc.

### `completions` — shell auto-complete

```bash
# zsh
claude-picker completions zsh > ~/.zsh/_claude-picker

# bash
claude-picker completions bash > /usr/local/etc/bash_completion.d/claude-picker

# fish
claude-picker completions fish > ~/.config/fish/completions/claude-picker.fish
```

### Pipe into `claude --resume`

```bash
# interactive pick, then resume
claude --resume $(claude-picker pipe)

# file-pivot: jump to the last session that touched auth.rs
claude --resume $(claude-picker files --project my-api --pipe auth.rs)
```

### `audit --format json` into your tooling

```bash
claude-picker audit --format json | jq '.findings | group_by(.project) | map({project: .[0].project, savings: map(.savings_usd) | add})'
claude-picker audit --format csv > audit.csv
```

---

## Configuration

Everything lives under `~/.config/claude-picker/`. Generate a starter with `claude-picker --generate-config`.

| File | Purpose |
|---|---|
| `config.toml` | `[ui]` / `[picker]` / `[actions]` / `[keys]` / `[bookmarks]` preferences |
| `bookmarks.json` | Pinned session IDs |
| `summaries.json` | Cached AI summaries keyed by session ID |
| `file-index.json` | `--files` pivot index |
| `budget.toml` | Monthly budget for the stats forecast |
| `pinned.toml` | `u`-pinned project slots (1–9) |

### Full `config.toml` example

```toml
[ui]
# Theme name — one of `--list-themes` output. Default: "kanagawa".
theme = "kanagawa"

# Disable every animation (fork reveal, pulse HUD, replay comet trail,
# peek fade, cursor glide, toast slide). Respects screen-reader and
# accessibility preferences. Default: false.
reduce_motion = false

# Alternating row shades on tabular lists. Default: true on dark themes.
zebra_rows = true

# Subscription tier for the stats quota panel. One of:
#   "none" (panel hidden, default), "pro" ($20), "max" ($100),
#   "max20" ($200), "team" ($30/user), "enterprise" (no cap).
plan_tier = "none"

# Auto-redact secret shapes (sk-ant-…, AKIA…, ghp_…, JWTs, Bearer …) in
# preview + viewer. Flip off if you're debugging a token yourself.
redact_preview = true

# Stats column cap. 0 = use full terminal width.
stats_width = 0

# Custom date format (strftime). Empty = auto.
date_format = ""

[picker]
# One of: "recent", "cost", "msgs", "name", "bookmarked-first".
sort = "bookmarked-first"
include_hidden_projects = true
min_messages = 2          # sessions below this are hidden
model_filter = ""         # "", "opus", "sonnet", "haiku"

[actions]
# Flags forwarded to `claude --resume`. Env CLAUDE_PICKER_FLAGS wins if set.
claude_flags = "--dangerously-skip-permissions"

# Editor override for `o`. Empty = $EDITOR → code → cursor → nvim → vim.
editor = ""

[bookmarks]
# Session IDs that should always float to the top.
ids = []
```

### Accurate cost tracking

Pricing is verified against the latest Anthropic rates:

| Model | Input ($/MTok) | Output ($/MTok) |
|---|---|---|
| Opus 4.x | $5 | $25 |
| Sonnet 4.x | $3 | $15 |
| Haiku 4.5 | $1 | $5 |
| Opus 3 (legacy) | $15 | $75 |

Cache pricing: `write_5m = 1.25×` input, `write_1h = 2×` input, `read = 0.1×` input. Tokens come from every `message.usage` block, including `cache_creation.ephemeral_5m_input_tokens`, `cache_creation.ephemeral_1h_input_tokens`, and `cache_read_input_tokens`.

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

<a id="ui-polish"></a>
## UI polish in v0.6

A handful of small things that together make the picker feel alive instead of spreadsheet-y. Everything below respects `[ui] reduce_motion = true`.

- **Loading skeletons** — cold start shows pulsing grey placeholder rows for ~1.2s while session enumeration settles, instead of snapping from empty to full.
- **Cursor memory** — re-enter a project and the cursor lands where you left it, not at the top.
- **Smooth scroll** — scroll interpolates over a few frames instead of jumping a page.
- **Chain badge (⛓)** — session list surfaces sessions that appear to continue each other: same project, opened within 24h, similar titles.
- **Cost anomaly badge (⚡)** — sessions whose cost is ≥2× the project median get a lightning chip so you spot runaway runs without opening the audit.
- **Zebra rows** — tabular lists alternate `base` and `surface0` on dark themes. Auto-off on light themes where the delta would flip contrast.
- **Interesting-moments timeline** in the conversation viewer — a compact top strip marks cost spikes, tool bursts, long pauses, and the first+last user prompts. One glance tells you where the session's weight sits.
- **Subagent tree** — Task tool calls render as nested children with `├─` / `└─` / `│ ` connectors, so multi-agent runs read as a tree instead of a flat log.

---

## Privacy

- **Nothing leaves your machine** except two explicit opt-in AI features:
  - `Ctrl+A` — AI summarize the highlighted session via Claude Haiku 4.5 (cached to `summaries.json`).
  - `ai-titles` — batch-name unnamed sessions via Haiku 4.5 (prompts for confirmation, runs cost-gated).
- **No telemetry.** No analytics. No background sync. No phone-home on startup.
- **Auto-redact in preview** — known secret shapes are masked before rendering. `sk-ant-…`, `sk-proj-…`, `AKIA…`, `ASIA…`, `ghp_…`, `gho_…`, `ghu_…`, `ghs_…`, `eyJ….….…` JWTs, `Bearer …` headers all get replaced with `****<last4>`. Opt out via `[ui] redact_preview = false`.
- **Export with `--redact`** — transcripts you share go through the same secret-masking pass before they hit disk.

---

## How it works

Claude Code stores sessions in `~/.claude/projects/` as JSONL files. Each project directory is lossy-encoded (`/Users/you/my_project` → `-Users-you-my-project`). Per-session metadata lives in `~/.claude/sessions/`.

claude-picker reads these directly to:

1. **Discover projects** — scans encoded directories, resolves real paths via a three-layer decoder (session-metadata lookup → JSONL `cwd` scan → naive decode fallback)
2. **Extract session info** — names from `custom-title` entries, message counts, permission modes, subagent counts, last user prompt
3. **Compute cost** — parses `message.usage` including cache-creation and cache-read fields against the pricing table above
4. **Detect forks and chains** — follows `forkedFrom` to build parent/child trees; heuristically detects cross-session chains by project + recency + title similarity
5. **Index files** — walks tool-use events to build the reverse `file → sessions` map
6. **Filter noise** — skips SDK-entrypoint sessions and strips system messages from previews
7. **Redact secrets** — runs every preview and export through a shape-based secret masker
8. **Render** — `ratatui` + `crossterm` with 24-bit color, unicode-correct rendering, `nucleo` fuzzy (~6× faster than skim), LCS word-diff, clipboard via `arboard`, Kitty/iTerm2/halfblock identicon thumbnails, `tachyonfx` animations (all respecting `reduce_motion`)

Pure Rust, no daemon, ~3.7 MB release binary.

---

## Project stats

- **93** Rust source files · **53 k** LOC
- **714** unit tests passing (0.07s via `cargo test --release --lib`)
- **~3.7 MB** release binary (stripped)
- **Rust 1.86+** MSRV · edition 2021

### Tech stack

- `ratatui` + `crossterm` — TUI engine
- `nucleo` — fuzzy matcher (~6× faster than `skim`)
- `catppuccin` — palette source for the Catppuccin pair
- `tachyonfx` — shader-style animations (fork reveal, comet trail, peek fade, pulse HUD) — all gated by `reduce_motion`
- `image` — pure-Rust pixel buffer for identicon thumbnails (halfblock rendering, works in every terminal)
- `clap` (derive) + `clap_complete` — CLI parsing + completions emitter
- `serde` + `serde_json` — JSONL parsing
- `unicode-width` + `unicode-segmentation` — grapheme-safe rendering for CJK and emoji
- `arboard` — cross-platform clipboard
- `similar` — LCS diff (word + line)
- `regex` — secret-redaction patterns
- `toml`, `chrono`, `anyhow`, `thiserror` — utilities

---

## Contributing

Open an issue or PR — contributions welcome.

```bash
RUSTUP_TOOLCHAIN=stable cargo test --release --lib
RUSTUP_TOOLCHAIN=stable cargo build --release --bin claude-picker
```

See [CHANGELOG.md](CHANGELOG.md) for the release history. See [BREW-TAP.md](BREW-TAP.md) for Homebrew tap maintenance.

If claude-picker saves you time, [star the repo](https://github.com/anshul-garg27/claude-picker) — it helps others find it.

---

## Gallery

Real TUI captures rendered against the `scripts/capture/seed-demo-home.sh` sandbox
(`HOME=/tmp/claude-picker-demo vhs scripts/capture/tapes/01-hero-v06.tape`).

<details>
<summary><strong>Session picker</strong> — filter, cost chips, model/permission pills, timestamps, live preview</summary>

<p align="center">
  <img src="assets/picker.png" alt="Left pane: session list inside a project with filter input at top, two sessions sorted by date, cost chips ($7.80 / $9.31), bookmark/pin indicators, and a running `today $0.00 · avg $0.57/day` cost counter. Right pane: live conversation preview with HH:MM timestamps and +Nm relative deltas, a `sonnet` model pill, and an `ACCEPT` permission badge. Footer key hints: ↑↓ navigate · Enter resume · v view · Tab multi · Ctrl-r scope · 1-9 pin." width="88%">
</p>
</details>

<details>
<summary><strong>Cost audit</strong> — 3-heuristic summary band, annual run-rate, per-project bar</summary>

<p align="center">
  <img src="assets/audit-tui.png" alt="Audit dashboard with top summary band listing three heuristics (tool-ratio 2 findings ~$64.00 in green; cache-efficiency 0 findings; model-mismatch 0 findings) followed by an `annual run-rate × 12.17 ~$778.88 avoidable/year` line. Below, two findings (data-pipeline / Optimize Redshift COPY command $76.85 and platform-infra / Debug terraform plan diff $56.47) each flagged with a ⚠ and a 'Haiku could save ~$36.40' / '~$27.60' narration. Bottom: a per-project horizontal bar split into purple (avoidable) and pink (other) segments." width="88%">
</p>
</details>

<details>
<summary><strong>Stats dashboard</strong> — KPI cards, 7×24 heatmap, project 30-day cost, budget, by-model</summary>

<p align="center">
  <img src="assets/stats.png" alt="Stats dashboard with three KPI cards (tokens 157.1M ▲203% vs prior · cost $458.38 ▲199% · sessions 19 ▲180%), a 7×24 day-of-week × hour-of-day pattern heatmap highlighting the Saturday-8pm peak, a project-heat 30-day grid for six projects sorted by cost ($203.17 down to $7.63), a month-to-date budget band at 73% of the month with a forecast of $625.07 at current burn, and a by-model breakdown (opus-4-7 76%, sonnet-4-5 23%, haiku-4-5 2%)." width="88%">
</p>
</details>

<details>
<summary><strong>Shell-prompt integration</strong> — single-line spend summary</summary>

<p align="center">
  <img src="assets/prompt.png" alt="Terminal running `claude-picker prompt` and emitting a single-line summary formatted for embedding in a PS1 shell prompt." width="74%">
</p>

Example output (copied verbatim from the live binary):

```bash
$ claude-picker prompt
claude: $458.38 today · $1,257.84 month
```
</details>

<details>
<summary><strong>Alternative branding</strong> — light theme + social card (for folks who want to reuse the art)</summary>

<p align="center">
  <img src="assets/hero-square-light.png" alt="Light-theme editorial composition — a floating ink-black terminal window on warm ivory canvas with five sample sessions and their costs, topped by a small orange-seal accent next to the claude-picker wordmark, tagline 'cost-aware terminal session manager' below." width="52%">
  <img src="assets/social-banner.png" alt="Wide GitHub social-share banner — sumi-ink background, Kanagawa accents, actual audit output with 3 heuristic rows in the right-side terminal pane, serif wordmark claude-picker on the left, vertical kanji 賢い選択 ('wise choice') on the far right edge." width="88%">
</p>

Both banners are generated art (Azure OpenAI `gpt-image-2`, `size=1792×1024 / 1024×1024`,
`quality=high`) — drop-in material for blog posts, conference talks, or GitHub social
preview cards.
</details>

### Still to capture

The following still need a VHS tape or Playwright scene before the gallery is complete:

- [ ] `assets/viewer.png` — conversation viewer with interesting-moments timeline, timestamps, and subagent tree
- [ ] `assets/replay.png` — time-travel replay with 4-position comet trail

The VHS tape at [`scripts/capture/tapes/01-hero-v06.tape`](../scripts/capture/tapes/01-hero-v06.tape)
is the live template for the other six above — duplicate it, swap the scene, and
re-run with `HOME=/tmp/claude-picker-demo vhs …`.

---

## License

MIT. See [LICENSE](LICENSE).

<p align="center">
  Built by <a href="https://github.com/anshul-garg27">Anshul Garg</a>.
</p>
