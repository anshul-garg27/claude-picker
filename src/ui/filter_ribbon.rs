//! atuin-style filter-scope ribbon.
//!
//! A horizontal strip of chips shown above the session list:
//!
//! ```text
//! [ALL] [REPO*] [7D] [RUNNING] [FORKED]
//! ```
//!
//! The active chip is suffixed with `*` and gets the theme's accent color;
//! the rest are dimmed. `Ctrl-r` cycles forward through the scopes. On app
//! startup, if the process cwd matches a discovered project we auto-activate
//! `REPO` — the "atuin workspace" behaviour.
//!
//! The ribbon never owns session data; it exposes the filter as a predicate
//! ([`Self::is_session_visible`]) that the session list consumes. Keeping the
//! predicate on the widget (rather than e.g. on `App`) means the scope state
//! and the rendered label always agree — a single source of truth.
//!
//! **Graceful degradation:**
//! - terminal width ≥ 80: full ribbon, all chips visible
//! - terminal width < 80: only the active chip is rendered (keeps the label
//!   readable without wrapping)

use std::env;
use std::path::PathBuf;

use chrono::{Duration, Utc};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::data::{Project, Session};
use crate::theme::Theme;

/// Declarative filter scope. Each variant maps to a single predicate in
/// [`FilterRibbon::is_session_visible`]; widening the ribbon is a matter of
/// appending to [`Self::ALL`] and adding an arm to each `match`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterScope {
    /// No filtering — every session passes.
    All,
    /// Only sessions whose project cwd matches the current repo (set at
    /// construction time).
    Repo,
    /// Sessions whose last activity is within the last 7 days.
    SevenDays,
    /// Sessions that are currently "running" — see
    /// [`session_is_running`] for the current stub.
    Running,
    /// Sessions that are forks of another session.
    Forked,
}

impl FilterScope {
    /// Cycle order. `Ctrl-r` moves the ribbon's active scope one step to the
    /// right, wrapping at the end.
    pub const ALL: &'static [FilterScope] = &[
        FilterScope::All,
        FilterScope::Repo,
        FilterScope::SevenDays,
        FilterScope::Running,
        FilterScope::Forked,
    ];

    /// Short label shown on the chip.
    pub fn label(self) -> &'static str {
        match self {
            FilterScope::All => "ALL",
            FilterScope::Repo => "REPO",
            FilterScope::SevenDays => "7D",
            FilterScope::Running => "RUNNING",
            FilterScope::Forked => "FORKED",
        }
    }
}

/// The ribbon widget. One instance lives on whatever struct owns the picker
/// (today that's `App` — wiring happens in `app.rs`, outside this module's
/// ownership).
pub struct FilterRibbon {
    scope: FilterScope,
    /// Absolute cwd string used when `scope == Repo`. Set by
    /// [`Self::new_with_auto_activation`] when the process cwd lines up with
    /// a discovered project; otherwise `None` and the `REPO` chip is a no-op
    /// until the user drills into a project.
    current_repo: Option<String>,
}

impl FilterRibbon {
    /// Build a ribbon, auto-activating `REPO` when the process cwd (from
    /// [`std::env::current_dir`]) is a prefix of one of the discovered
    /// project paths. This matches atuin's "I opened the terminal in my
    /// workspace, scope to it" affordance.
    pub fn new_with_auto_activation(projects: &[Project]) -> Self {
        let cwd = env::current_dir().ok();
        let matched = cwd.as_ref().and_then(|c| project_for_cwd(projects, c));

        match matched {
            Some(path) => Self {
                scope: FilterScope::Repo,
                current_repo: Some(path.to_string_lossy().into_owned()),
            },
            None => Self {
                scope: FilterScope::All,
                current_repo: None,
            },
        }
    }

    /// Explicit no-auto-detection constructor for tests and callers that
    /// want to control the initial state.
    pub fn new(scope: FilterScope, current_repo: Option<String>) -> Self {
        Self {
            scope,
            current_repo,
        }
    }

    /// Currently active scope.
    pub fn scope(&self) -> FilterScope {
        self.scope
    }

    /// Swap the repo cwd used by the `REPO` chip. Called when the user drills
    /// into a project — we pin the ribbon's notion of "this repo" to the one
    /// they just selected, so `REPO` stays meaningful after navigation.
    pub fn set_current_repo(&mut self, cwd: Option<String>) {
        self.current_repo = cwd;
    }

    /// Replace the active scope. Used by the wiring layer to pair e.g.
    /// `ProjectList::clear_project_filter` with `ribbon.set_scope(All)`.
    pub fn set_scope(&mut self, scope: FilterScope) {
        self.scope = scope;
    }

    /// Cycle forward through [`FilterScope::ALL`], wrapping at the end.
    /// Called by the `Ctrl-r` keybind.
    pub fn cycle_forward(&mut self) {
        let idx = FilterScope::ALL
            .iter()
            .position(|&s| s == self.scope)
            .unwrap_or(0);
        self.scope = FilterScope::ALL[(idx + 1) % FilterScope::ALL.len()];
    }

    /// Predicate the session list calls for every session. Runs once per row
    /// per frame; keep it cheap.
    pub fn is_session_visible(&self, session: &Session) -> bool {
        match self.scope {
            FilterScope::All => true,
            FilterScope::Repo => match &self.current_repo {
                Some(repo) => {
                    let s = session.project_dir.to_string_lossy();
                    s == repo.as_str() || s.starts_with(&format!("{repo}/"))
                }
                // No repo context available — show everything rather than
                // hiding the list and confusing the user.
                None => true,
            },
            FilterScope::SevenDays => match session.last_timestamp {
                Some(ts) => Utc::now().signed_duration_since(ts) <= Duration::days(7),
                None => false,
            },
            FilterScope::Running => session_is_running(session),
            FilterScope::Forked => session_has_forks(session),
        }
    }

    /// Render into `area`. Uses `Widget::render` shape so callers that have a
    /// `Buffer` (instead of a `Frame`) — e.g. in composite layouts — can use
    /// the ribbon directly.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let paragraph = if area.width < 80 {
            // Narrow mode: render only the active chip so the label stays
            // readable on mobile-sized terminals.
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("[{}*]", self.scope.label()),
                    Style::default()
                        .fg(theme.mauve)
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
        } else {
            let mut spans: Vec<Span<'_>> = Vec::with_capacity(FilterScope::ALL.len() * 3 + 1);
            spans.push(Span::raw(" "));
            for (i, &s) in FilterScope::ALL.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw(" "));
                }
                let is_active = s == self.scope;
                let label = if is_active {
                    format!("[{}*]", s.label())
                } else {
                    format!("[{}]", s.label())
                };
                let style = if is_active {
                    Style::default()
                        .fg(theme.mauve)
                        .add_modifier(Modifier::BOLD)
                } else {
                    // Dim non-active chips — we want the active one to read
                    // at-a-glance.
                    Style::default().fg(theme.overlay0)
                };
                spans.push(Span::styled(label, style));
            }
            Paragraph::new(Line::from(spans))
        };
        paragraph.render(area, buf);
    }
}

impl Default for FilterRibbon {
    fn default() -> Self {
        Self::new(FilterScope::All, None)
    }
}

/// Find a project whose resolved path is an ancestor of `cwd` (or equal to it).
/// Returns the longest-matching project path so a nested workspace inside a
/// parent repo picks the inner project.
fn project_for_cwd<'a>(projects: &'a [Project], cwd: &std::path::Path) -> Option<&'a PathBuf> {
    let mut best: Option<&PathBuf> = None;
    for p in projects {
        if cwd == p.path || cwd.starts_with(&p.path) {
            match best {
                Some(prev) if prev.as_os_str().len() >= p.path.as_os_str().len() => {}
                _ => best = Some(&p.path),
            }
        }
    }
    best
}

/// Placeholder "is this session currently running" check.
///
// TODO: wire to ~/.claude/sessions/<pid>.json presence check once Horizon 1
// data-layer ships — today we have no source of truth for liveness.
fn session_is_running(_session: &Session) -> bool {
    false
}

/// Placeholder "does this session have forks" check.
///
// TODO: replace with a proper `Session::has_forks()` once the data layer
// aggregates child forks. For now we reuse `is_fork` (self is a fork of
// something else) which is the closest semantic hit today.
fn session_has_forks(session: &Session) -> bool {
    session.is_fork
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn mk_project(path: &str) -> Project {
        Project {
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            path: PathBuf::from(path),
            encoded_dir: path.replace('/', "-"),
            session_count: 1,
            last_activity: None,
            git_branch: None,
        }
    }

    fn mk_session(project_dir: &str) -> Session {
        Session {
            id: "abc".to_string(),
            project_dir: PathBuf::from(project_dir),
            name: None,
            auto_name: None,
            last_prompt: None,
            message_count: 0,
            tokens: crate::data::pricing::TokenCounts::default(),
            total_cost_usd: 0.0,
            model_summary: String::new(),
            first_timestamp: None,
            last_timestamp: Some(Utc::now()),
            is_fork: false,
            forked_from: None,
            entrypoint: crate::data::SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
            turn_durations: Vec::new(),
        }
    }

    #[test]
    fn cycle_forward_wraps() {
        let mut r = FilterRibbon::new(FilterScope::All, None);
        r.cycle_forward();
        assert_eq!(r.scope(), FilterScope::Repo);
        for _ in 0..(FilterScope::ALL.len() - 1) {
            r.cycle_forward();
        }
        assert_eq!(r.scope(), FilterScope::All);
    }

    #[test]
    fn all_scope_passes_everything() {
        let r = FilterRibbon::new(FilterScope::All, None);
        assert!(r.is_session_visible(&mk_session("/anywhere")));
    }

    #[test]
    fn repo_scope_matches_cwd_prefix() {
        let r = FilterRibbon::new(FilterScope::Repo, Some("/w/foo".to_string()));
        assert!(r.is_session_visible(&mk_session("/w/foo")));
        assert!(r.is_session_visible(&mk_session("/w/foo/bar")));
        assert!(!r.is_session_visible(&mk_session("/w/other")));
    }

    #[test]
    fn repo_scope_without_context_passes_everything() {
        let r = FilterRibbon::new(FilterScope::Repo, None);
        assert!(r.is_session_visible(&mk_session("/anywhere")));
    }

    #[test]
    fn seven_days_excludes_older_sessions() {
        let mut s = mk_session("/p");
        s.last_timestamp = Some(Utc::now() - Duration::days(30));
        let r = FilterRibbon::new(FilterScope::SevenDays, None);
        assert!(!r.is_session_visible(&s));
    }

    #[test]
    fn auto_activation_picks_longest_matching_project() {
        // In the tests process we can't depend on current_dir matching a
        // synthetic path — so we exercise the helper directly.
        let projects = vec![mk_project("/w"), mk_project("/w/foo")];
        let cwd = PathBuf::from("/w/foo/bar");
        let matched = project_for_cwd(&projects, &cwd);
        assert_eq!(matched, Some(&PathBuf::from("/w/foo")));
    }
}
