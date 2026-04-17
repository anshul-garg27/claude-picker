//! Default picker command — the one users hit when they run `claude-picker`
//! with no subcommand.
//!
//! Responsibilities:
//! 1. Discover projects via the data layer.
//! 2. If the user is inside a known project directory, jump straight to
//!    session-list for that project; otherwise land on project-list.
//! 3. Run the Ratatui event loop.
//! 4. On Enter, print `Resuming session <id>` to stderr and forward the id
//!    as the process's selection (future work: actually `exec claude
//!    --resume <id>` once the real dispatch lives in the bash wrapper).

use std::path::{Path, PathBuf};

use crate::app::{self, App, Mode};
use crate::data::bookmarks::BookmarkStore;
use crate::data::{project, session, Project, Session};
use crate::theme::ThemeName;

/// Entry point for the default picker. Uses the default theme — kept for
/// backwards compatibility with test callers that don't care about theming.
pub fn run() -> anyhow::Result<Option<(String, PathBuf)>> {
    run_with_theme(ThemeName::default())
}

/// Same as [`run`] but with an explicit theme. Called from `main` after it
/// has resolved the CLI / env / persisted precedence chain.
pub fn run_with_theme(theme_name: ThemeName) -> anyhow::Result<Option<(String, PathBuf)>> {
    run_with_theme_and_preview(theme_name, None)
}

/// Like [`run_with_theme`] but with an optional `--preview-cmd` override.
/// When `preview_cmd` is `Some`, the picker uses a shell-snippet preview
/// instead of the built-in renderer; see [`crate::ui::preview`] for how the
/// command's output is cached and displayed.
pub fn run_with_theme_and_preview(
    theme_name: ThemeName,
    preview_cmd: Option<String>,
) -> anyhow::Result<Option<(String, PathBuf)>> {
    let projects = project::discover_projects()?;
    let bookmarks = BookmarkStore::load().unwrap_or_else(|_| {
        BookmarkStore::load_from(PathBuf::from("/tmp/.claude-picker-bookmarks.json"))
            .expect("fallback bookmark store")
    });

    // Try to land directly in session-list if we're inside a project the user
    // already has.
    let cwd = std::env::current_dir().ok();
    let active_idx = cwd.as_deref().and_then(|c| match_project(&projects, c));

    let (mode, sessions, selected_project) = match active_idx {
        Some(idx) => {
            let sessions = load_sessions_for(&projects[idx])?;
            if sessions.is_empty() {
                (Mode::ProjectList, vec![], Some(idx))
            } else {
                (Mode::SessionList, sessions, Some(idx))
            }
        }
        None => (Mode::ProjectList, vec![], None),
    };

    let mut app = App::new_with_theme(
        projects,
        sessions,
        bookmarks,
        mode,
        selected_project,
        theme_name,
    );
    app.preview_cmd = preview_cmd;
    let selection = app::run(app)?;

    if let Some((id, _cwd)) = &selection {
        eprintln!("Resuming session {id}");
    }
    Ok(selection)
}

/// True when `cwd` falls inside the project's resolved path.
fn match_project(projects: &[Project], cwd: &Path) -> Option<usize> {
    for (i, p) in projects.iter().enumerate() {
        if cwd.starts_with(&p.path) {
            return Some(i);
        }
    }
    None
}

/// Load all sessions for a project, skipping ones the data loader filters out
/// (SDK-only sessions, stubs with <2 messages).
///
/// Public so the event loop can re-enter this path when the user picks a
/// project from the project-list screen.
pub fn load_sessions_for(project: &Project) -> anyhow::Result<Vec<Session>> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home"))?;
    let dir = home
        .join(".claude")
        .join("projects")
        .join(&project.encoded_dir);
    if !dir.is_dir() {
        return Ok(vec![]);
    }

    let mut out: Vec<Session> = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        match session::load_session_from_jsonl(&path, project.path.clone()) {
            Ok(Some(s)) => out.push(s),
            Ok(None) => {}
            Err(e) => eprintln!("{}: load error: {e}", path.display()),
        }
    }

    // Most recent first so Enter on the default selection resumes the right
    // session.
    out.sort_by_key(|s| std::cmp::Reverse(s.last_timestamp));
    Ok(out)
}
