//! Resume a Claude Code session by exec-replacing the current process with
//! `claude <flags> --resume <id>` inside the session's project cwd.
//!
//! We use `exec` (not `spawn`) so the user sees Claude take over the terminal
//! directly — no orphaned parent process, no "press any key to continue"
//! weirdness, no lingering claude-picker frame.
//!
//! The flags passed to `claude` come from the `CLAUDE_PICKER_FLAGS` env var,
//! defaulting to `--dangerously-skip-permissions` to match the v1 bash wrapper's
//! behaviour. Users who want vanilla permissions can override with
//! `CLAUDE_PICKER_FLAGS=""` in their shell rc.

use std::path::Path;
use std::process::Command;

/// Default flags passed to `claude --resume`. Matches the v1 Python/bash
/// wrapper, which was `CLAUDE_PICKER_FLAGS="${CLAUDE_PICKER_FLAGS:---dangerously-skip-permissions}"`.
const DEFAULT_FLAGS: &str = "--dangerously-skip-permissions";

/// Read `CLAUDE_PICKER_FLAGS` from the environment. Empty string = pass no
/// extra flags. Unset = use `DEFAULT_FLAGS`. Splits on whitespace so users
/// can chain multiple flags.
fn claude_flags() -> Vec<String> {
    let raw = std::env::var("CLAUDE_PICKER_FLAGS").unwrap_or_else(|_| DEFAULT_FLAGS.to_string());
    raw.split_whitespace().map(str::to_string).collect()
}

/// Exec `claude <flags> --resume <id>` in `cwd`. This function does not
/// return on success — the current process is replaced. On failure
/// (claude binary not on PATH, or cwd doesn't exist), prints an error to
/// stderr and exits with code 127.
pub fn resume_session(id: &str, cwd: &Path) -> ! {
    use std::os::unix::process::CommandExt;

    let flags = claude_flags();

    // Light progress hint before claude takes over. Keeps the user oriented
    // if Claude's own startup has any delay.
    eprintln!("Resuming session {id}");

    let err = Command::new("claude")
        .args(&flags)
        .arg("--resume")
        .arg(id)
        .current_dir(cwd)
        .exec();

    // If exec returned, something went wrong (otherwise we'd be Claude now).
    eprintln!(
        "failed to launch `claude {} --resume {id}` in {}: {err}",
        flags.join(" "),
        cwd.display()
    );
    eprintln!("is the `claude` CLI installed and on your PATH?");
    std::process::exit(127);
}
