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

use clap::{Parser, Subcommand};

use claude_picker::theme::{self, ThemeName};
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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.list_themes {
        print_theme_list();
        return Ok(());
    }

    if cli.classic {
        eprintln!(
            "--classic mode: falling back to Python/fzf — run: bash <repo>/claude-picker --classic"
        );
        return Ok(());
    }

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
    let theme_name = theme::resolve_theme_name(cli.theme.as_deref());

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

    match cli.command {
        None => {
            // Default: the picker. If the user made a selection, hand off to
            // claude itself rather than just printing the id.
            if let Some((id, cwd)) = commands::pick::run_with_theme(theme_name)? {
                resume::resume_session(&id, &cwd); // diverges
            }
            Ok(())
        }
        Some(Command::Pipe) => commands::pipe_cmd::run(),
        Some(Command::Stats) => commands::stats_cmd::run(),
        Some(Command::Tree) => commands::tree_cmd::run(),
        Some(Command::Diff) => commands::diff_cmd::run(),
        Some(Command::Search) => commands::search_cmd::run(),
    }
}

/// `--list-themes` handler. Newline-separated so it's pipe-friendly.
fn print_theme_list() {
    for t in ThemeName::ALL {
        println!("{}", t.label());
    }
}
