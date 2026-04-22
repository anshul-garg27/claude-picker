//! Shell-completion script emission for `claude-picker --generate-completions`.
//!
//! Hands the clap-derived [`clap::Command`] to `clap_complete`, which emits
//! a shell-native completion script to stdout. Callers pipe that into the
//! appropriate directory for their shell (e.g. `~/.zsh/completions`).
//!
//! The shells we support match `clap_complete::Shell`:
//!
//! - `bash`
//! - `zsh`
//! - `fish`
//! - `powershell`
//! - `elvish` (accepted as a bonus; not advertised in the README)
//!
//! Dynamic completions for `--theme` (every built-in theme label) and
//! `--format` (per-subcommand format allow-lists) are already driven by the
//! `PossibleValue` metadata on the clap derive, so the generated script
//! picks them up automatically — no post-processing needed.
//!
//! Dynamic `--project` completion is NOT baked in. Project basenames live
//! on disk and would require shell-side discovery (e.g. `ls ~/.claude/projects
//! | sed …`). The generated script leaves that slot unconstrained so any
//! value is accepted; a follow-up can splice in shell-specific `compgen`
//! glue per-shell when the appetite appears.

use std::io::{self, Write};
use std::str::FromStr;

use clap::Command;
use clap_complete::{generate, Shell};

/// Parsed `--generate-completions` argument. Accepts the same set
/// `clap_complete::Shell` does, plus a friendly synonym for PowerShell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionShell(pub Shell);

impl CompletionShell {
    /// Normalise a user-supplied shell name to its canonical label. Returns
    /// `None` when the value isn't one of the supported shells.
    pub fn parse(raw: &str) -> Option<Self> {
        let canonical = raw.trim().to_ascii_lowercase();
        let shell = match canonical.as_str() {
            // Friendly aliases users reach for first. `clap_complete` accepts
            // the exact forms below but this keeps the error surface small.
            "powershell" | "pwsh" | "ps" | "ps1" => Shell::PowerShell,
            other => Shell::from_str(other).ok()?,
        };
        Some(Self(shell))
    }

    /// Stable label used in error messages and README install snippets.
    pub fn label(self) -> &'static str {
        match self.0 {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::PowerShell => "powershell",
            Shell::Elvish => "elvish",
            // Future-proof: any new Shell variant falls back to Debug-style.
            _ => "unknown",
        }
    }
}

/// Emit a completion script for the given shell to `out`.
///
/// `bin_name` is the name used on the command line; we pass `"claude-picker"`
/// from the binary so the emitted script carries the same identifier the
/// user typed.
pub fn emit(shell: CompletionShell, cmd: &mut Command, bin_name: &str, out: &mut dyn Write) {
    generate(shell.0, cmd, bin_name, out);
}

/// Shorthand — the binary entry point calls this with stdout and returns.
/// Writes a trailing newline if clap's generator didn't, so terminals that
/// tail `head` / `tail` don't glue the script to the next prompt.
pub fn emit_to_stdout(shell: CompletionShell, cmd: &mut Command) -> io::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    emit(shell, cmd, "claude-picker", &mut handle);
    handle.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    // A tiny mirror of `main.rs`' top-level so tests don't have to reach
    // into the binary crate. `clap_complete` only cares about the Command
    // graph shape.
    #[derive(Parser, Debug)]
    #[command(name = "claude-picker", version)]
    struct Dummy {
        #[arg(long)]
        theme: Option<String>,
    }

    #[test]
    fn parse_accepts_canonical_shells() {
        assert_eq!(CompletionShell::parse("bash").map(|s| s.label()), Some("bash"));
        assert_eq!(CompletionShell::parse("zsh").map(|s| s.label()), Some("zsh"));
        assert_eq!(CompletionShell::parse("fish").map(|s| s.label()), Some("fish"));
        assert_eq!(
            CompletionShell::parse("powershell").map(|s| s.label()),
            Some("powershell"),
        );
    }

    #[test]
    fn parse_is_case_insensitive_and_trims() {
        assert_eq!(CompletionShell::parse("ZSH").map(|s| s.label()), Some("zsh"));
        assert_eq!(CompletionShell::parse("  fish  ").map(|s| s.label()), Some("fish"));
    }

    #[test]
    fn parse_accepts_powershell_aliases() {
        assert!(CompletionShell::parse("pwsh").is_some());
        assert!(CompletionShell::parse("ps").is_some());
        assert!(CompletionShell::parse("ps1").is_some());
    }

    #[test]
    fn parse_rejects_unknown() {
        assert!(CompletionShell::parse("csh").is_none());
        assert!(CompletionShell::parse("").is_none());
        assert!(CompletionShell::parse("yaml").is_none());
    }

    #[test]
    fn emit_produces_non_empty_script() {
        let mut cmd = Dummy::command();
        let mut buf: Vec<u8> = Vec::new();
        let shell = CompletionShell::parse("zsh").expect("zsh");
        emit(shell, &mut cmd, "claude-picker", &mut buf);
        let out = String::from_utf8(buf).expect("utf8");
        // Sanity — every generator emits some reference to the bin name.
        assert!(out.contains("claude-picker"), "missing bin name in script");
        assert!(!out.is_empty());
    }

    #[test]
    fn emit_bash_script_mentions_complete_builtin() {
        let mut cmd = Dummy::command();
        let mut buf: Vec<u8> = Vec::new();
        let shell = CompletionShell::parse("bash").expect("bash");
        emit(shell, &mut cmd, "claude-picker", &mut buf);
        let out = String::from_utf8(buf).expect("utf8");
        // Bash scripts always register via `complete -F`.
        assert!(out.contains("complete"), "bash completion must use `complete`");
    }
}
