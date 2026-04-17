//! Launch the user's editor on a project directory.
//!
//! Picks `$EDITOR` first. If that's unset we try `code`, then `cursor`, then
//! `vim`, then `nvim`. The first one on `PATH` wins. Spawn is detached —
//! `Command::spawn` returns immediately so the picker keeps running.
//!
//! Returns a short label on success ("code"), or an error string on failure.

use std::path::Path;
use std::process::{Command, Stdio};

/// Fallback list when `$EDITOR` is unset. Ordered by what a 2026 Claude Code
/// user is most likely to have installed.
const FALLBACKS: &[&str] = &["code", "cursor", "nvim", "vim"];

/// Try to open `path` in the user's preferred editor. Returns the command name
/// that was launched on success.
pub fn open_in_editor(path: &Path) -> Result<String, String> {
    let candidate = std::env::var("EDITOR")
        .ok()
        .filter(|s| !s.trim().is_empty());
    if let Some(editor) = candidate {
        return launch(&editor, path).map(|_| editor);
    }

    let mut last_err = String::from("$EDITOR not set");
    for fallback in FALLBACKS {
        if which(fallback) {
            match launch(fallback, path) {
                Ok(()) => return Ok((*fallback).to_string()),
                Err(e) => last_err = format!("{fallback}: {e}"),
            }
        }
    }
    Err(last_err)
}

fn launch(program: &str, path: &Path) -> Result<(), String> {
    // `$EDITOR` may include args ("code --wait"), so split on whitespace.
    let mut parts = program.split_whitespace();
    let Some(bin) = parts.next() else {
        return Err("empty editor command".to_string());
    };

    let mut cmd = Command::new(bin);
    for arg in parts {
        cmd.arg(arg);
    }
    cmd.arg(path);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn().map(|_| ()).map_err(|e| format!("{e}"))
}

/// Cheap PATH lookup. Walks `$PATH` segments looking for an executable
/// `program`. Platform-agnostic enough for our fallback list.
fn which(program: &str) -> bool {
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_finds_sh() {
        // `sh` is essentially always on PATH on unix. On Windows we'd need a
        // different smoke test, but the picker targets macOS/Linux first.
        #[cfg(unix)]
        assert!(which("sh"));
    }

    #[test]
    fn which_rejects_garbage() {
        assert!(!which("zzz-definitely-not-a-command-42"));
    }
}
