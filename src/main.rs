//! claude-picker — terminal session manager for Claude Code.
//!
//! Binary entry. Parses the CLI with `clap`, dispatches to a subcommand, and
//! lets the command implementation own its own output and exit code. Keeping
//! `main` thin makes testing easy: every subcommand is a plain
//! `pub fn run() -> anyhow::Result<()>`.

use clap::{Parser, Subcommand};

use claude_picker::commands;

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

    if cli.classic {
        eprintln!(
            "--classic mode: falling back to Python/fzf — run: bash <repo>/claude-picker --classic"
        );
        return Ok(());
    }

    match cli.command {
        None => {
            // Default: the picker.
            let _ = commands::pick::run()?;
            Ok(())
        }
        Some(Command::Pipe) => commands::pipe_cmd::run(),
        Some(Command::Stats) => commands::stats_cmd::run(),
        Some(Command::Tree) => commands::tree_cmd::run(),
        Some(Command::Diff) => commands::diff_cmd::run(),
        Some(Command::Search) => commands::search_cmd::run(),
    }
}
