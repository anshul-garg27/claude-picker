//! `claude-picker stats` — full stats dashboard.
//!
//! Day-2 work. The v1 cut of this subcommand prints a friendly "coming soon"
//! message and exits cleanly so CI pipelines and install smoke-tests don't
//! fail on the presence of the subcommand.

pub fn run() -> anyhow::Result<()> {
    eprintln!("claude-picker stats — coming in Day 2.");
    eprintln!("For now, use: claude-picker (default picker) or `lib/session-stats.py`.");
    Ok(())
}
