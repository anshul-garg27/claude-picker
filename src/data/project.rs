//! Project discovery — one [`Project`] per non-empty Claude-CLI project
//! directory under `~/.claude/projects/`.
//!
//! The name is the basename of the resolved cwd when we can recover it, or
//! the best-effort naive decode otherwise. `git_branch` is looked up lazily
//! via `git -C <path> branch --show-current` and left `None` if we can't
//! ask.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use chrono::{DateTime, Utc};

use crate::data::path_resolver::{load_session_metadata, resolve};

/// A single project folder under `~/.claude/projects/`.
#[derive(Debug, Clone)]
pub struct Project {
    /// Basename of the resolved cwd.
    pub name: String,
    /// Resolved real filesystem path (may not exist on disk any more).
    pub path: PathBuf,
    /// The encoded directory name under `~/.claude/projects/`.
    pub encoded_dir: String,
    pub session_count: u32,
    pub last_activity: Option<DateTime<Utc>>,
    pub git_branch: Option<String>,
}

/// Scan `~/.claude/projects/` and return one [`Project`] per subdirectory
/// that holds at least one `.jsonl`.
///
/// This does not load sessions — callers that want aggregated session data
/// should combine this with [`crate::data::session::load_session_from_jsonl`].
pub fn discover_projects() -> anyhow::Result<Vec<Project>> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects_dir = home.join(".claude").join("projects");
    let sessions_meta_dir = home.join(".claude").join("sessions");
    discover_projects_in(&projects_dir, &sessions_meta_dir)
}

/// Injection-friendly variant of [`discover_projects`] for tests and any
/// future callers that want to override the root.
pub fn discover_projects_in(
    projects_dir: &Path,
    sessions_meta_dir: &Path,
) -> anyhow::Result<Vec<Project>> {
    let mut out = Vec::new();
    if !projects_dir.is_dir() {
        return Ok(out);
    }

    let meta = load_session_metadata(sessions_meta_dir);

    for entry in fs::read_dir(projects_dir)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let encoded_dir = entry.file_name().to_string_lossy().into_owned();

        let mut session_count = 0u32;
        let mut latest_mtime: Option<SystemTime> = None;
        for sess_entry in fs::read_dir(&path)?.flatten() {
            let p = sess_entry.path();
            if p.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            session_count = session_count.saturating_add(1);
            if let Ok(m) = sess_entry.metadata() {
                if let Ok(t) = m.modified() {
                    latest_mtime = Some(latest_mtime.map_or(t, |cur| cur.max(t)));
                }
            }
        }
        if session_count == 0 {
            continue;
        }

        let resolved = resolve(&encoded_dir, &meta, projects_dir)
            .unwrap_or_else(|| PathBuf::from(&encoded_dir));
        let name = resolved
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Fallback to the Python-style "last hyphen segment".
                encoded_dir
                    .rsplit('-')
                    .next()
                    .unwrap_or(&encoded_dir)
                    .to_string()
            });

        let last_activity = latest_mtime.map(system_time_to_utc);
        let git_branch = git_branch_for(&resolved);

        out.push(Project {
            name,
            path: resolved,
            encoded_dir,
            session_count,
            last_activity,
            git_branch,
        });
    }

    // Newest first so callers get a sensible default ordering.
    out.sort_by_key(|p| std::cmp::Reverse(p.last_activity));
    Ok(out)
}

fn system_time_to_utc(t: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(t)
}

/// Ask git for the current branch at `path`. `None` if the directory is
/// missing, not a git repo, detached HEAD, or if `git` isn't on PATH.
fn git_branch_for(path: &Path) -> Option<String> {
    if !path.is_dir() {
        return None;
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("branch")
        .arg("--show-current")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_empty_returns_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects = tmp.path().join("projects");
        let sessions = tmp.path().join("sessions");
        fs::create_dir_all(&projects).expect("mkdir");
        let out = discover_projects_in(&projects, &sessions).expect("ok");
        assert!(out.is_empty());
    }

    #[test]
    fn discover_returns_one_project_per_nonempty_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects = tmp.path().join("projects");
        let sessions = tmp.path().join("sessions");
        fs::create_dir_all(projects.join("-Users-me-foo")).expect("mkdir");
        fs::write(
            projects.join("-Users-me-foo").join("abc.jsonl"),
            b"{\"type\":\"user\",\"cwd\":\"/tmp/foo\"}\n",
        )
        .expect("write");
        // Empty dir — should be skipped.
        fs::create_dir_all(projects.join("-Users-me-empty")).expect("mkdir");

        let out = discover_projects_in(&projects, &sessions).expect("ok");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_count, 1);
        assert_eq!(out[0].encoded_dir, "-Users-me-foo");
    }
}
