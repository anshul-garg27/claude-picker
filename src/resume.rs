//! Resume a Claude Code session by exec-replacing the current process with
//! `claude --resume <id>` inside the session's project cwd.
//!
//! We use `exec` (not `spawn`) so the user sees Claude take over the terminal
//! directly — no orphaned parent process, no "press any key to continue"
//! weirdness, no lingering claude-picker frame.

use std::path::Path;
use std::process::Command;

/// Exec `claude --resume <id>` in `cwd`. This function does not return on
/// success — the current process is replaced. On failure (claude binary not
/// on PATH, or cwd doesn't exist), prints an error to stderr and exits the
/// process with code 127.
pub fn resume_session(id: &str, cwd: &Path) -> ! {
    use std::os::unix::process::CommandExt;

    // Light progress hint before claude takes over. Keeps the user oriented
    // if Claude's own startup has any delay.
    eprintln!("Resuming session {id}");

    let err = Command::new("claude")
        .arg("--resume")
        .arg(id)
        .current_dir(cwd)
        .exec();

    // If exec returned, something went wrong (otherwise we'd be Claude now).
    eprintln!(
        "failed to launch `claude --resume {id}` in {}: {err}",
        cwd.display()
    );
    eprintln!("is the `claude` CLI installed and on your PATH?");
    std::process::exit(127);
}
