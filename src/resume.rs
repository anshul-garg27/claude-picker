//! Resume a Claude Code session by handing the terminal over to
//! `claude <flags> --resume <id>` inside the session's project cwd.
//!
//! On Unix we exec-replace the current process so the user sees Claude take
//! over directly — no orphaned parent, no lingering claude-picker frame.
//! Windows has no `execvp`, so we spawn-and-wait and mirror the child's exit
//! status. From the user's perspective the flow is identical.
//!
//! The flags passed to `claude` come from the `CLAUDE_PICKER_FLAGS` env var,
//! defaulting to `--dangerously-skip-permissions` to match the v1 bash wrapper's
//! behaviour. Users who want vanilla permissions can override with
//! `CLAUDE_PICKER_FLAGS=""` in their shell rc.

use std::path::Path;
use std::process::Command;

const DEFAULT_FLAGS: &str = "--dangerously-skip-permissions";

fn claude_flags() -> Vec<String> {
    let raw = std::env::var("CLAUDE_PICKER_FLAGS").unwrap_or_else(|_| DEFAULT_FLAGS.to_string());
    raw.split_whitespace().map(str::to_string).collect()
}

/// Launch `claude <flags> --resume <id>` in `cwd`. Diverges on success (Unix
/// via `execvp`, Windows via spawn + exit-with-status). On failure prints an
/// error and exits 127.
pub fn resume_session(id: &str, cwd: &Path) -> ! {
    let flags = claude_flags();
    eprintln!("Resuming session {id}");

    let mut cmd = Command::new("claude");
    cmd.args(&flags).arg("--resume").arg(id).current_dir(cwd);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        eprintln!(
            "failed to launch `claude {} --resume {id}` in {}: {err}",
            flags.join(" "),
            cwd.display()
        );
        eprintln!("is the `claude` CLI installed and on your PATH?");
        std::process::exit(127);
    }

    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(0)),
            Err(err) => {
                eprintln!(
                    "failed to launch `claude {} --resume {id}` in {}: {err}",
                    flags.join(" "),
                    cwd.display()
                );
                eprintln!("is the `claude` CLI installed and on your PATH?");
                std::process::exit(127);
            }
        }
    }
}
