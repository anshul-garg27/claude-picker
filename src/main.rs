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
