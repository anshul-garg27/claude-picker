//! claude-picker — terminal session manager for Claude Code.
//!
//! Binary entry. Parses the CLI with `clap`, dispatches to a subcommand, and
//! lets the command implementation own its own output and exit code. Keeping
//! `main` thin makes testing easy: every subcommand is a plain
//! `pub fn run() -> anyhow::Result<()>`.
//!
//! Theme handling also lives here: the `--theme` flag, `--list-themes`
//! subcommand, and `CLAUDE_PICKER_THEME` env var fallbacks feed into
//! [`claude_picker::theme::resolve_theme_name`] which returns the active
//! [`ThemeName`] to hand off to the picker.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use claude_picker::config::Config;
use claude_picker::theme::{self, ThemeName};
use claude_picker::ui::masthead;
use claude_picker::{commands, resume};

#[derive(Parser, Debug)]
#[command(
    name = "claude-picker",
    version,
    about = "Terminal session manager for Claude Code"
)]
struct Cli {
    /// Use classic fzf-based UI instead of Ratatui.
    ///
    /// v2 delegates to the bash wrapper which in turn calls the legacy
    /// Python/fzf stack; this binary only prints the redirect hint.
    #[arg(long, global = true)]
    classic: bool,

    /// Override the active color theme for this run.
    ///
    /// Takes precedence over `CLAUDE_PICKER_THEME` and the on-disk
    /// persisted choice. Value is one of the labels printed by
    /// `--list-themes`.
    #[arg(long, global = true, value_name = "NAME")]
    theme: Option<String>,

    /// Print the built-in theme names and exit.
    ///
    /// Bypasses the TUI entirely — useful in shells that want to
    /// auto-complete or document the options.
    #[arg(long, global = true)]
    list_themes: bool,

    // ── Flag-style aliases for subcommands ──────────────────────────
    // v1 bash wrapper used `--stats`, `--tree`, `--diff`, `--search`,
    // `--pipe` as long flags; keep that ergonomics. Users can write
    // either `claude-picker --stats` or `claude-picker stats`.
    /// Open the stats dashboard (alias for the `stats` subcommand).
    #[arg(long, conflicts_with_all = ["tree_flag", "diff_flag", "search_flag", "pipe_flag"])]
    stats: bool,
    /// Open the session tree with fork detection (alias for `tree`).
    #[arg(long = "tree", conflicts_with_all = ["stats", "diff_flag", "search_flag", "pipe_flag"])]
    tree_flag: bool,
    /// Compare two sessions side-by-side (alias for `diff`).
    #[arg(long = "diff", conflicts_with_all = ["stats", "tree_flag", "search_flag", "pipe_flag"])]
    diff_flag: bool,
    /// Full-text search across all sessions (alias for `search`).
    #[arg(long = "search", short = 's', conflicts_with_all = ["stats", "tree_flag", "diff_flag", "pipe_flag"])]
    search_flag: bool,
    /// Print selected session ID to stdout (alias for `pipe`).
    #[arg(long = "pipe", short = 'p', conflicts_with_all = ["stats", "tree_flag", "diff_flag", "search_flag"])]
    pipe_flag: bool,

    /// File-centric pivot view (alias for `files`).
    ///
    /// Lists every file Claude Code ever touched, with the sessions that
    /// touched each one. Answers "which sessions modified
    /// `src/auth/middleware.ts`?" — a pivot no other session manager
    /// offers. Pair with `--project <name>` to scope to one project.
    #[arg(long = "files")]
    files_flag: bool,

    /// Show all configured Claude Code hooks and their execution history.
    #[arg(long = "hooks")]
    hooks_flag: bool,

    /// Show installed MCP servers and tool-call usage across sessions.
    #[arg(long = "mcp")]
    mcp_flag: bool,

    /// Browse file-history checkpoints per session.
    #[arg(long = "checkpoints")]
    checkpoints_flag: bool,

    /// Cost-optimization audit — flag sessions that could be cheaper.
    #[arg(long = "audit")]
    audit_flag: bool,

    /// Batch-title unnamed sessions via Haiku 4.5 (prompts for confirmation).
    #[arg(long = "ai-titles")]
    ai_titles_flag: bool,

    /// Restrict the `--files` view to a single project (by basename).
    /// Example: `claude-picker --files --project architex`.
    #[arg(long, global = true, value_name = "NAME")]
    project: Option<String>,

    /// Write a commented default `config.toml` and exit.
    ///
    /// Target is `~/.config/claude-picker/config.toml` unless `--config-file`
    /// points elsewhere. Refuses to overwrite an existing file; pass
    /// `--force` to replace.
    #[arg(long, global = true)]
    generate_config: bool,

    /// Allow `--generate-config` to overwrite an existing config.
    #[arg(long, global = true)]
    force: bool,

    /// Override the config-file location (defaults to
    /// `~/.config/claude-picker/config.toml`). Useful for tests and
    /// split-personality setups.
    #[arg(long, global = true, value_name = "PATH")]
    config_file: Option<PathBuf>,

    /// Override the built-in preview renderer with a user command.
    ///
    /// When set, the picker spawns the given shell snippet for every
    /// highlighted session. `{sid}` is substituted with the session id
    /// and `{cwd}` with the project path. Stdout becomes the preview
    /// body; a non-zero exit renders stderr with error styling. Output
    /// is cached per session-id so scrolling doesn't re-run the command.
    ///
    /// Only `{sid}` and `{cwd}` are placeholders — the rest of the
    /// string is passed to the user's shell verbatim. Do not embed
    /// untrusted data into the command itself.
    ///
    /// Examples:
    ///
    ///   --preview-cmd='cat ~/.claude/projects/*/{sid}.jsonl | head -100'
    ///
    ///   --preview-cmd='git -C {cwd} log --oneline -10'
    #[arg(long, global = true, value_name = "CMD")]
    preview_cmd: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Stats dashboard (tokens, cost, per-project, timeline).
    Stats,
    /// Session tree with fork detection.
    Tree,
    /// Diff two sessions side-by-side.
    Diff,
    /// Full-text search across all sessions.
    Search,
    /// Print selected session ID to stdout (for scripting).
    Pipe,
    /// File-centric pivot view — list every file, pivot to sessions.
    Files,
    /// Show all configured Claude Code hooks + execution history.
    Hooks,
    /// Show installed MCP servers + tool-call usage.
    Mcp,
    /// Browse file-history checkpoints per session.
    Checkpoints,
    /// Cost-optimization audit across every session.
    Audit,
    /// Batch-title unnamed sessions via a Haiku summarizer.
    #[command(name = "ai-titles")]
    AiTitles,
    /// Export a session transcript to a Markdown file.
    Export {
        /// Session id (matches the `.jsonl` stem under `~/.claude/projects/`).
        session_id: String,
        /// Output path. Defaults to
        /// `~/Downloads/claude-picker-{sid}-{YYYY-MM-DD}.md`.
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,
        /// Mask Anthropic / OpenAI / AWS / GitHub / Bearer tokens before writing.
        #[arg(long)]
        redact: bool,
    },
    /// Diagnostic scan of `~/.claude/projects/` — sizes, top sessions, orphans.
    Doctor {
        /// Delete orphan metadata + empty stub sessions listed in the report.
        #[arg(long)]
        cleanup: bool,
        /// Required confirmation when combined with `--cleanup`. Without it
        /// the cleanup phase runs as a dry-run.
        #[arg(long)]
        yes: bool,
        /// Output format: `plain` (default), `json`, or `csv`.
        #[arg(long, default_value = "plain", value_name = "FORMAT")]
        format: String,
    },
    /// Print the most-recent session id(s) for scripting.
    Latest {
        /// Filter to a single project (by basename).
        #[arg(long, value_name = "NAME")]
        project: Option<String>,
        /// How many ids to print.
        #[arg(long, default_value_t = 1)]
        count: usize,
        /// Only include sessions within the last N (e.g. `7d`, `12h`, `30m`).
        #[arg(long, value_name = "WINDOW")]
        since: Option<String>,
        /// Output format: `id` (one per line) or `json` (structured array).
        #[arg(long, default_value = "id", value_name = "FORMAT")]
        format: String,
    },
    /// Single-line spend summary for embedding in your shell prompt.
    Prompt {
        /// `PS1` (human) or `JSON` (structured).
        #[arg(long, default_value = "PS1", value_name = "FORMAT")]
        format: String,
        /// Suppress ANSI color for prompts that can't render it.
        #[arg(long)]
        no_color: bool,
    },
    /// Emit a shell-completion script (bash / zsh / fish / elvish / powershell).
    Completions {
        /// Shell name. Use `bash`, `zsh`, `fish`, `elvish`, or `powershell`.
        shell: String,
    },
}

fn main() -> anyhow::Result<()> {
    // Branding pass — print the ASCII masthead above clap's help output,
    // but only for the **top-level** `claude-picker --help` / `-h` and only
    // when stdout is a real TTY. Subcommand help stays un-banner'd so focused
    // sub-docs read cleanly.
    if masthead::wants_top_level_help(std::env::args().skip(1)) {
        masthead::print_if_tty();
    }

    let cli = Cli::parse();

    if cli.list_themes {
        print_theme_list();
        return Ok(());
    }

    // `--generate-config` is handled before every other path so a bad config
    // on disk doesn't block a user from regenerating it.
    if cli.generate_config {
        let path = cli
            .config_file
            .clone()
            .or_else(Config::default_path)
            .ok_or_else(|| anyhow::anyhow!("could not determine config path (no home dir?)"))?;
        Config::write_template(&path, cli.force)?;
        eprintln!("wrote {}", path.display());
        return Ok(());
    }

    if cli.classic {
        eprintln!(
            "--classic mode: falling back to Python/fzf — run: bash <repo>/claude-picker --classic"
        );
        return Ok(());
    }

    // Load the on-disk config. Missing file is NOT an error. A malformed
    // file IS, but we degrade gracefully to defaults so a broken TOML
    // can't brick the picker — surface a stderr warning instead.
    let config = match cli.config_file.as_deref() {
        Some(path) => Config::load_from(path).unwrap_or_else(|e| {
            eprintln!("claude-picker: config error ({e:#}), using defaults");
            Config::default()
        }),
        None => Config::load().unwrap_or_else(|e| {
            eprintln!("claude-picker: config error ({e:#}), using defaults");
            Config::default()
        }),
    };

    // Resolve theme up front so every subcommand sees the same one. Invalid
    // CLI values fall through to the next source; surface a warning so the
    // user knows their flag was ignored.
    if let Some(raw) = cli.theme.as_deref() {
        if ThemeName::from_str(raw).is_none() {
            eprintln!(
                "claude-picker: unknown theme {raw:?} — using fallback. \
                 See `claude-picker --list-themes`."
            );
        }
    }
    let theme_name = theme::resolve_theme_name_with_config(cli.theme.as_deref(), &config.ui.theme);

    // Flag aliases take precedence over the subcommand slot — they're
    // mutually exclusive via clap's conflicts_with_all, so at most one is
    // true at a time.
    if cli.stats {
        return commands::stats_cmd::run();
    }
    if cli.tree_flag {
        return commands::tree_cmd::run();
    }
    if cli.diff_flag {
        return commands::diff_cmd::run();
    }
    if cli.search_flag {
        return commands::search_cmd::run();
    }
    if cli.pipe_flag {
        return commands::pipe_cmd::run();
    }
    if cli.files_flag {
        return commands::files_cmd::run(cli.project.clone());
    }
    if cli.hooks_flag {
        return commands::hooks_cmd::run();
    }
    if cli.mcp_flag {
        return commands::mcp_cmd::run();
    }
    if cli.checkpoints_flag {
        return commands::checkpoints_cmd::run();
    }
    if cli.audit_flag {
        return commands::audit_cmd::run();
    }
    if cli.ai_titles_flag {
        return commands::ai_titles_cmd::run();
    }

    match cli.command {
        None => {
            // Default: the picker. If the user made a selection, hand off to
            // claude itself rather than just printing the id.
            if let Some((id, cwd)) =
                commands::pick::run_with_theme_and_preview(theme_name, cli.preview_cmd.clone())?
            {
                resume::resume_session(&id, &cwd); // diverges
            }
            Ok(())
        }
        Some(Command::Pipe) => commands::pipe_cmd::run(),
        Some(Command::Stats) => commands::stats_cmd::run(),
        Some(Command::Tree) => commands::tree_cmd::run(),
        Some(Command::Diff) => commands::diff_cmd::run(),
        Some(Command::Search) => commands::search_cmd::run(),
        Some(Command::Files) => commands::files_cmd::run(cli.project.clone()),
        Some(Command::Hooks) => commands::hooks_cmd::run(),
        Some(Command::Mcp) => commands::mcp_cmd::run(),
        Some(Command::Checkpoints) => commands::checkpoints_cmd::run(),
        Some(Command::Audit) => commands::audit_cmd::run(),
        Some(Command::AiTitles) => commands::ai_titles_cmd::run(),
        Some(Command::Export {
            session_id,
            out,
            redact,
        }) => commands::export_cmd::run(&session_id, out, redact),
        Some(Command::Doctor {
            cleanup,
            yes,
            format,
        }) => {
            let format = commands::doctor_cmd::Format::parse(&format).unwrap_or_default();
            commands::doctor_cmd::run(commands::doctor_cmd::Options {
                cleanup,
                yes,
                format,
            })
        }
        Some(Command::Latest {
            project,
            count,
            since,
            format,
        }) => {
            let since = match since.as_deref() {
                Some(raw) => match commands::latest_cmd::parse_since(raw) {
                    Some(d) => Some(d),
                    None => {
                        eprintln!(
                            "claude-picker: invalid --since value {raw:?} (use e.g. 7d / 12h / 30m)"
                        );
                        std::process::exit(2);
                    }
                },
                None => None,
            };
            let format = commands::latest_cmd::Format::parse(&format).unwrap_or_default();
            commands::latest_cmd::run(commands::latest_cmd::Options {
                project,
                count,
                since,
                format,
            })
        }
        Some(Command::Prompt { format, no_color }) => {
            let format = commands::prompt_cmd::Format::parse(&format).unwrap_or_default();
            commands::prompt_cmd::run(commands::prompt_cmd::Options { format, no_color })
        }
        Some(Command::Completions { shell }) => {
            use claude_picker::completions;
            let parsed = completions::CompletionShell::parse(&shell).unwrap_or_else(|| {
                eprintln!(
                    "claude-picker: unknown shell {shell:?} — \
                     expected bash | zsh | fish | elvish | powershell"
                );
                std::process::exit(2);
            });
            let mut cmd = <Cli as clap::CommandFactory>::command();
            completions::emit_to_stdout(parsed, &mut cmd)?;
            Ok(())
        }
    }
}

/// `--list-themes` handler. Newline-separated so it's pipe-friendly.
fn print_theme_list() {
    for t in ThemeName::ALL {
        println!("{}", t.label());
    }
}
