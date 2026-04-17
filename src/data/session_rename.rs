//! Rename a Claude Code session by appending a `custom-title` record to its
//! JSONL.
//!
//! Claude Code stores session names as JSONL entries of the form:
//!
//! ```json
//! {"type":"custom-title","customTitle":"new name","sessionId":"<id>"}
//! ```
//!
//! The picker's loader already picks up the last-seen `customTitle` value, so
//! appending a fresh line is enough for both the on-disk format and any other
//! tools that consume the JSONL.
//!
//! We locate the session file by scanning `~/.claude/projects/<encoded>` —
//! the picker doesn't always remember `encoded_dir` for a loaded session, and
//! scanning is fast enough (a few dozen directories in normal use).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use serde_json::json;

/// Append a custom-title JSONL entry so future loads surface `new_name`.
///
/// Returns the path that was modified so the caller can double-check the
/// session file actually exists before reporting success.
pub fn rename_session(session_id: &str, new_name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("name cannot be empty");
    }
    let path = find_session_jsonl(session_id)?;

    let entry = json!({
        "type": "custom-title",
        "customTitle": trimmed,
        "sessionId": session_id,
    });
    let mut line = serde_json::to_string(&entry)?;
    line.push('\n');

    let mut f = OpenOptions::new().append(true).create(false).open(&path)?;
    f.write_all(line.as_bytes())?;
    f.flush()?;
    Ok(path)
}

/// Find the on-disk JSONL for `session_id` by scanning every project
/// directory under `~/.claude/projects`.
fn find_session_jsonl(session_id: &str) -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects = home.join(".claude").join("projects");
    if !projects.is_dir() {
        anyhow::bail!("~/.claude/projects not found");
    }
    for entry in std::fs::read_dir(&projects)? {
        let Ok(entry) = entry else { continue };
        let candidate = entry.path().join(format!("{session_id}.jsonl"));
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("session file not found for {session_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rename_rejects_empty() {
        let err = rename_session("any-id", "   ").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn rename_appends_custom_title_line() {
        // Build a fake home by setting HOME to a tempdir. The rename helper
        // uses `dirs::home_dir()` which picks up HOME on unix.
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path();
        let projects_root = home.join(".claude").join("projects").join("proj-a");
        fs::create_dir_all(&projects_root).expect("mkdir");
        let id = "test-session-42";
        let path = projects_root.join(format!("{id}.jsonl"));
        fs::write(
            &path,
            b"{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"hi\"}}\n",
        )
        .expect("seed");

        // `dirs::home_dir` reads HOME on Unix, USERPROFILE on Windows.
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", home);
        let result = rename_session(id, "shiny new title");
        if let Some(h) = prev_home {
            std::env::set_var("HOME", h);
        }
        result.expect("rename ok");

        let body = fs::read_to_string(&path).expect("read");
        let last = body.lines().last().expect("has line");
        assert!(last.contains("\"type\":\"custom-title\""));
        assert!(last.contains("\"customTitle\":\"shiny new title\""));
        assert!(last.contains(&format!("\"sessionId\":\"{id}\"")));
    }
}
