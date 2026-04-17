//! `claude-picker ai-titles` — generate AI-authored titles for every unnamed
//! session.
//!
//! The flow is deliberately blocking + confirmation-gated because this is the
//! one place where the picker spends real API budget:
//!
//! 1. Enumerate every session that lacks a `custom-title`.
//! 2. Print the session count + estimated cost (Haiku rates × N sessions).
//! 3. Wait for the user to type `y` / `Y` before proceeding.
//! 4. For each candidate, call [`ai_summarize::generate_title`] and write the
//!    result back to the session's JSONL via `session_rename::rename_session`.
//!
//! No alt-screen UI — this is a simple stdout-progress command so the user
//! can Ctrl-C out safely and CI can log it. Errors on individual sessions are
//! reported inline; we keep going instead of aborting the whole batch.

use std::io::Write;

use crate::data::ai_summarize::{self, ESTIMATED_COST_USD};
use crate::data::project::discover_projects;
use crate::data::session::load_session_from_jsonl;
use crate::data::session_rename;

/// Entry point for `claude-picker ai-titles`.
pub fn run() -> anyhow::Result<()> {
    let candidates = collect_unnamed_sessions()?;
    if candidates.is_empty() {
        println!("No unnamed sessions — every session already has a custom title.");
        return Ok(());
    }

    let est = ESTIMATED_COST_USD * candidates.len() as f64;
    println!(
        "About to generate AI titles for {} unnamed sessions.",
        candidates.len()
    );
    println!("Estimated cost: ${est:.3} (using Haiku 4.5)");
    print!("Proceed? [y/N] ");
    std::io::stdout().flush().ok();

    if !prompt_confirm()? {
        println!("Aborted.");
        return Ok(());
    }

    let total = candidates.len();
    let mut ok = 0usize;
    let mut failed = 0usize;
    for (i, session_id) in candidates.iter().enumerate() {
        print!("  [{n}/{total}] {session_id} … ", n = i + 1);
        std::io::stdout().flush().ok();
        match ai_summarize::generate_title(session_id) {
            Ok(title) => match session_rename::rename_session(session_id, &title) {
                Ok(_) => {
                    println!("\"{title}\"");
                    ok += 1;
                }
                Err(e) => {
                    println!("rename failed: {e}");
                    failed += 1;
                }
            },
            Err(e) => {
                println!("title failed: {e}");
                failed += 1;
            }
        }
    }

    println!();
    println!(
        "Titled {ok} sessions ({failed} failed). Estimated total cost: ~${:.3}",
        ESTIMATED_COST_USD * ok as f64
    );
    Ok(())
}

/// Read a single line from stdin and return `true` if it starts with `y`/`Y`.
fn prompt_confirm() -> anyhow::Result<bool> {
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf)?;
    let trimmed = buf.trim();
    Ok(matches!(trimmed, "y" | "Y" | "yes" | "YES"))
}

/// Walk every project and return the ids of sessions with no `name` (no
/// `custom-title` recorded). Sorts by most-recent activity so the first few
/// titles cover the user's freshest sessions.
fn collect_unnamed_sessions() -> anyhow::Result<Vec<String>> {
    let projects = discover_projects()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects_root = home.join(".claude").join("projects");

    let mut out: Vec<(chrono::DateTime<chrono::Utc>, String)> = Vec::new();
    for project in &projects {
        let dir = projects_root.join(&project.encoded_dir);
        if !dir.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let session = match load_session_from_jsonl(&path, project.path.clone()) {
                Ok(Some(s)) => s,
                _ => continue,
            };
            if session.name.is_some() {
                continue;
            }
            let ts = session
                .last_timestamp
                .unwrap_or_else(|| chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap());
            out.push((ts, session.id));
        }
    }
    out.sort_by_key(|(ts, _)| std::cmp::Reverse(*ts));
    Ok(out.into_iter().map(|(_, id)| id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimated_cost_scales_linearly_per_session() {
        let n = 12;
        let est = ESTIMATED_COST_USD * n as f64;
        assert!(
            (est - 0.024).abs() < 1e-9,
            "spec example: 12 sessions = $0.024, got {est}"
        );
    }
}
