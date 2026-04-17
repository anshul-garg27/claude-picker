# Changelog

All notable changes to `claude-picker` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-04-16

The "v3.0 mega-sprint" release. claude-picker grew from a picker-plus-stats tool into a full 12-screen session manager, with three pivots no other Claude TUI offers: file-centric navigation, time-travel replay, and one-key AI summaries.

### Added

#### Novel pivots
- **`files` screen (`claude-picker files` / `--files`)** — new file-centric pivot. Lists every file Claude has ever touched across all projects and lets you drill in to see every session that edited it. Index cached to `~/.config/claude-picker/file-index.json`. Scope with `--project NAME`.
- **`R` time-travel replay** — scrub any session forward and backward like a video. Gap capping compresses long idle stretches so playback stays watchable.
- **`Ctrl+A` AI summarize** — one keystroke produces a focused TL;DR via Claude Haiku 4.5. Cost-gated confirmation before the call. Every summary is cached on disk at `~/.config/claude-picker/summaries.json`, so repeat presses are free.
- **`audit` screen (`claude-picker audit` / `--audit`)** — cost-optimization report with three heuristics: tool-ratio (sessions dominated by tool calls), cache-efficiency (sessions with weak cache reads), and model-mismatch (Opus on throwaway work, Haiku on reasoning-heavy work).

#### New screens
- **`hooks` screen (`--hooks`)** — shows every configured Claude Code hook and its execution history.
- **`mcp` screen (`--mcp`)** — installed MCP servers plus tool-call usage rolled up across sessions.
- **`checkpoints` screen (`--checkpoints`)** — browse file-history checkpoints attached to each session.
- **Conversation viewer (`v` key)** — full-screen transcript view with styled metadata and messages.

#### Stats dashboard overhaul
- KPI cards for total cost, total tokens, and total sessions.
- 24×7 hourly activity heatmap.
- GitHub-style monthly activity heatmap.
- Turn-duration histogram.
- Per-project and per-model breakdowns.
- Monthly budget forecast: set a budget in `~/.config/claude-picker/budget.toml` and the dashboard shows burn rate against it.

#### Search language
- New 11-operator filter language parsed in `src/data/search_filters.rs`: `project:`, `model:`, `cost:>X`, `tokens:>Y`, `has:tools`, `before:`, `after:`, `sub:`, `mode:`, `bookmarked`, plus free text.
- Search is available as its own screen (`--search` / `-s`) and from `/` within any view.

#### Diff
- Word-level diff toggle on top of the existing line-level diff, powered by `similar 2.6` LCS.

#### Tree
- Drill-down expansion for fork chains (`fork_descendants`, `is_expanded`) so long branch trees stay readable.

#### Themes
- Six themes ship with the binary: `catppuccin-mocha` (default), `catppuccin-macchiato`, `catppuccin-frappe`, `catppuccin-latte`, `tokyonight`, and `gruvbox`, via the `catppuccin 2.7` crate.
- `--theme NAME` flag, `CLAUDE_PICKER_THEME` env var, and `config.toml` `[ui].theme` setting, with precedence flag > env > config > default.
- `--list-themes` prints every installed theme.
- `t` key cycles themes live without restarting.

#### Config
- `--generate-config` writes a default `config.toml` to `~/.config/claude-picker/`.
- `--config-file PATH` loads from an explicit path.
- Four on-disk stores: `config.toml`, `bookmarks.json`, `summaries.json`, `file-index.json`, `budget.toml`.

#### CLI and scripting
- `pipe` subcommand (`--pipe` / `-p`) prints the selected session ID for shell pipelines.
- `ai-titles` subcommand (`--ai-titles`) batch-names every unnamed session via Haiku 4.5, with a cost-gated confirmation prompt. Override with `--force`.
- `--preview-cmd CMD` swaps the preview pane for your own command, with `{sid}` and `{cwd}` substitution.
- `--project NAME` scopes `files`, `search`, and friends to one project.
- Every subcommand also has a `--flag` alias so existing muscle memory still works.

#### Keyboard
- `Tab` multi-select — stage multiple sessions at once via a `HashSet` in app state.
- `gg` / `G` — Vim-style top/bottom jumps, with the `gg` chord tracked by a `pending_g` flag.
- `r` renames the session by rewriting the `custom-title` entry in the JSONL.
- `o` opens the raw JSONL in `$EDITOR`.
- `y` copies the session ID, `Y` copies the full content, both via `arboard 3.0`.
- `*` toggles bookmarks, `b` toggles bookmarks-only filter.
- `Space` opens a command palette leader.
- `?` is a context-aware help overlay that reflects the current screen.

#### Data surfacing
- Last-prompt line in the session row (first 80 chars of the last user message).
- Permission-mode badges in the preview (`dangerous`, `acceptEdits`, `default`, `plan`).
- Subagent counter parsed from the JSONL.
- Auto-name fallback uses the first user prompt when no `custom-title` is set.

#### Distribution
- Homebrew tap: `brew install anshul-garg27/tap/claude-picker`.
- Shell installer via cargo-dist: `curl -LsSf https://github.com/anshul-garg27/claude-picker/releases/latest/download/claude-picker-installer.sh | sh`.
- `cargo install claude-picker`.
- GitHub Actions `release.yml` builds every platform automatically.

### Changed
- Default picker is now a two-pane project → session layout with a live preview, replacing the single-list view.
- Resume (`Enter`) now exec's `claude` via `CommandExt::exec`, so the picker process is fully replaced rather than forked. Flags are read from `CLAUDE_PICKER_FLAGS` and default to `--dangerously-skip-permissions`.
- Fuzzy matcher swapped from skim-style to `nucleo 0.5` — roughly 6× faster on large session lists.
- Pricing table verified against current Anthropic rates: Opus 4.x $5/$25 per MTok, Sonnet 4.x $3/$15, Haiku 4.5 $1/$5, legacy Opus 3 $15/$75. Cache multipliers are write-5m 1.25×, write-1h 2×, read 0.1× input.
- Token parsing now reads the full `message.usage` block including `cache_creation.ephemeral_5m_input_tokens`, `cache_creation.ephemeral_1h_input_tokens`, and `cache_read_input_tokens`, giving accurate cost attribution on cache-heavy sessions.
- Age warnings updated to flag peach at 7 days and red with a warning icon at 30 days.

### Fixed
- Sessions with unicode in titles (CJK, emoji, ZWJ sequences) now measure correctly via `unicode-width 0.2` + `unicode-segmentation 1.12`, so rows no longer wrap or truncate in the wrong place.
- Project discovery now handles encoded directory names reliably via metadata lookup, the JSONL `cwd` field, and an encode-and-compare fallback — previously some projects were missed when the encoded path didn't round-trip.
- SDK-based tool sessions (those with a non-CLI `entrypoint`) are filtered out of the picker instead of polluting the list.
- System messages are stripped from previews so the preview pane reads like a conversation, not a transcript dump.
- Bookmarked sessions survive rename operations — the bookmark is keyed by session ID, not title.

## [0.1.0] - 2026-03-18

Initial Rust rewrite. Replaces the original Python + fzf implementation with a single static binary (~2.1 MB) and zero runtime dependencies. The Python version is still available behind `--classic`.

### Added
- Rust binary on crates.io as `claude-picker`, with `cargo install claude-picker` support.
- Two-pane picker: projects on the left, sessions on the right, live preview.
- Fuzzy search over projects and sessions.
- Named sessions float to the top of the list; unnamed sessions auto-label from the first user message.
- Basic token and cost estimates in each row; cost shown for sessions over 10k tokens.
- Fork detection via the JSONL `forkedFrom` field.
- `--search` full-text search across every interactive session in every project.
- `--stats` terminal dashboard: total sessions, token estimates, per-project bars, activity timeline (today / this week / older), top 5 sessions by token usage.
- `--tree` sessions grouped by project with fork relationships.
- `--diff` side-by-side session comparison.
- `--pipe` prints the selected session ID to stdout for scripting.
- `Ctrl+B` bookmark, `Ctrl+E` export to markdown, `Ctrl+D` delete.
- `Ctrl+P` shell keybinding installed into `.zshrc` / `.bashrc` by the installer.
- Warp terminal tab config auto-detected and installed.
- Catppuccin Mocha 24-bit color scheme with labeled panel borders.
- `--classic` flag drops into the legacy Python + fzf flow for anyone who prefers it.
- MIT license, `assets/gifs/` demo set, and README covering install, features, and usage.

### Changed
- Runtime requirement dropped from "Python 3.10+, fzf 0.58+, rich" to "nothing but the binary".
- Startup time dropped by roughly an order of magnitude versus the Python version on large session directories, because everything now runs in-process instead of shelling out to fzf and Python per render.
