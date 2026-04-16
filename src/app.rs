//! Application state + event loop for the picker.
//!
//! Everything that persists between frames lives on [`App`]: the session
//! list, the filter buffer, the toast queue, the bookmark store, the matcher
//! scratch memory. The event handler ([`App::handle_event`]) is a terse
//! dispatch that branches on our normalised [`crate::events::Event`] and
//! mutates state in place — no async, no channels.
//!
//! Fuzzy matching is delegated to `nucleo::Matcher`. We keep the matcher
//! instance on the struct so the allocator pool it manages is reused across
//! keystrokes: rebuilding the filtered index on every char-press is
//! microseconds for < 1k sessions.

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use nucleo::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::bookmarks::BookmarkStore;
use crate::data::{Project, Session};
use crate::events::{self, Event};
use crate::theme::Theme;

/// Which screen is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Picking a project from the full list.
    ProjectList,
    /// Picking a session inside the active project.
    SessionList,
}

/// Transient on-screen message (bookmark toggled, export started, error, …).
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    expires_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

impl Toast {
    fn new(message: impl Into<String>, kind: ToastKind) -> Self {
        Self {
            message: message.into(),
            kind,
            expires_at: Instant::now() + Duration::from_millis(1500),
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// The single mutable struct shared by the event loop and every render call.
pub struct App {
    pub mode: Mode,

    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub selected_project: Option<usize>,
    pub selected_session: Option<usize>,

    /// Current filter input.
    pub filter: String,
    /// Indices into `projects` or `sessions` (depending on mode) that match
    /// the current filter, in score order.
    pub filtered_indices: Vec<usize>,
    /// Which filtered entry the cursor is on (0..filtered_indices.len()).
    pub cursor: usize,
    /// Filter input has focus — controls which border styles light up.
    pub filter_focused: bool,

    pub bookmarks: BookmarkStore,
    pub theme: Theme,

    /// Whether the event loop should exit after this frame.
    pub should_quit: bool,
    /// Result communicated to the caller — if set, we'll resume this session.
    pub selection_result: Option<(String, PathBuf)>,

    /// Active toast (if any).
    pub toast: Option<Toast>,
    /// Confirmation modal for delete.
    pub show_delete_confirm: bool,

    /// nucleo scratch. Rebuilt on filter change.
    matcher: Matcher,
    /// Precomputed haystacks for the current mode.
    haystacks: Vec<Utf32String>,
}

impl App {
    /// Construct an initial state. Callers decide whether to land on
    /// [`Mode::ProjectList`] or [`Mode::SessionList`] by pre-seeding the
    /// sessions vector. If both are set, session-list wins and the
    /// project-list pops back in when the user hits Esc.
    pub fn new(
        projects: Vec<Project>,
        sessions: Vec<Session>,
        bookmarks: BookmarkStore,
        mode: Mode,
        selected_project: Option<usize>,
    ) -> Self {
        let theme = Theme::mocha();
        let matcher = Matcher::new(Config::DEFAULT);
        let mut s = Self {
            mode,
            projects,
            sessions,
            selected_project,
            selected_session: Some(0),
            filter: String::new(),
            filtered_indices: Vec::new(),
            cursor: 0,
            filter_focused: true,
            bookmarks,
            theme,
            should_quit: false,
            selection_result: None,
            toast: None,
            show_delete_confirm: false,
            matcher,
            haystacks: Vec::new(),
        };
        s.rebuild_haystacks();
        s.apply_filter();
        s
    }

    /// Active project, when one is selected.
    pub fn active_project(&self) -> Option<&Project> {
        self.selected_project.and_then(|i| self.projects.get(i))
    }

    /// Selected session, looked up through the filter.
    pub fn selected_session_ref(&self) -> Option<&Session> {
        let idx = *self.filtered_indices.get(self.cursor)?;
        self.sessions.get(idx)
    }

    /// Cursor position as a display-index. `None` if no matches.
    pub fn cursor_position(&self) -> Option<usize> {
        if self.filtered_indices.is_empty() {
            None
        } else {
            Some(
                self.cursor
                    .min(self.filtered_indices.len().saturating_sub(1)),
            )
        }
    }

    /// Rebuild the list of strings we match against. Called when the mode
    /// changes or when the underlying data swaps.
    fn rebuild_haystacks(&mut self) {
        self.haystacks.clear();
        match self.mode {
            Mode::SessionList => {
                for s in &self.sessions {
                    let composite = format!("{} {} {}", s.display_label(), s.id, s.model_summary,);
                    self.haystacks.push(Utf32String::from(composite));
                }
            }
            Mode::ProjectList => {
                for p in &self.projects {
                    let composite = format!(
                        "{} {} {}",
                        p.name,
                        p.encoded_dir,
                        p.git_branch.as_deref().unwrap_or("")
                    );
                    self.haystacks.push(Utf32String::from(composite));
                }
            }
        }
    }

    /// Recompute `filtered_indices` from the current filter.
    ///
    /// With an empty filter we use the natural order (projects: recency,
    /// sessions: recency from the loader) so Enter on a fresh screen
    /// resumes the most recent thing.
    fn apply_filter(&mut self) {
        self.filtered_indices.clear();
        let total = self.haystacks.len();
        if self.filter.is_empty() {
            self.filtered_indices.extend(0..total);
            self.cursor = 0;
            return;
        }

        let pattern = Pattern::new(
            &self.filter,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut scored: Vec<(u32, usize)> = Vec::with_capacity(total);
        for (i, hay) in self.haystacks.iter().enumerate() {
            if let Some(score) = pattern.score(hay.slice(..), &mut self.matcher) {
                scored.push((score, i));
            }
        }
        // Higher score first, then lower original index as a stable tiebreak.
        scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.filtered_indices = scored.into_iter().map(|(_, i)| i).collect();
        self.cursor = 0;
    }

    /// Dispatch a single [`Event`] against the state.
    pub fn handle_event(&mut self, ev: Event) -> anyhow::Result<()> {
        // Delete-confirmation modal steals input.
        if self.show_delete_confirm {
            return self.handle_delete_confirm(ev);
        }

        match ev {
            Event::Quit | Event::Ctrl('c') => self.should_quit = true,
            Event::Ctrl('d') => self.request_delete(),
            Event::Ctrl('b') => self.toggle_bookmark(),
            Event::Ctrl('e') => self.export_session(),
            Event::Enter => self.confirm_selection(),
            Event::Escape => self.handle_escape(),
            Event::Up => self.move_cursor(-1),
            Event::Down => self.move_cursor(1),
            Event::PageUp => self.move_cursor(-10),
            Event::PageDown => self.move_cursor(10),
            Event::Home => self.cursor = 0,
            Event::End => self.cursor = self.filtered_indices.len().saturating_sub(1),
            Event::Backspace => self.filter_backspace(),
            Event::Key(c) if c == 'q' && self.filter.is_empty() => self.should_quit = true,
            Event::Key(c) if is_filter_char(c) => self.filter_push(c),
            Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    fn handle_delete_confirm(&mut self, ev: Event) -> anyhow::Result<()> {
        match ev {
            Event::Key('y') | Event::Key('Y') => {
                self.show_delete_confirm = false;
                if let Some(s) = self.selected_session_ref().cloned() {
                    match delete_session_file(&s) {
                        Ok(()) => {
                            self.toast = Some(Toast::new(
                                format!("deleted {}", &s.id[..8.min(s.id.len())]),
                                ToastKind::Success,
                            ));
                            // Remove from in-memory list so UI updates.
                            if let Some(idx) = self.sessions.iter().position(|x| x.id == s.id) {
                                self.sessions.remove(idx);
                            }
                            self.rebuild_haystacks();
                            self.apply_filter();
                        }
                        Err(e) => {
                            self.toast =
                                Some(Toast::new(format!("delete failed: {e}"), ToastKind::Error));
                        }
                    }
                }
            }
            Event::Escape | Event::Key('n') | Event::Key('N') => {
                self.show_delete_confirm = false;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_escape(&mut self) {
        if !self.filter.is_empty() {
            self.filter.clear();
            self.apply_filter();
            return;
        }
        match self.mode {
            Mode::SessionList => {
                // Pop back to project-list if we have one to show.
                if !self.projects.is_empty() {
                    self.mode = Mode::ProjectList;
                    self.sessions.clear();
                    self.selected_session = None;
                    self.rebuild_haystacks();
                    self.apply_filter();
                } else {
                    self.should_quit = true;
                }
            }
            Mode::ProjectList => self.should_quit = true,
        }
    }

    fn confirm_selection(&mut self) {
        match self.mode {
            Mode::SessionList => {
                if let Some(s) = self.selected_session_ref().cloned() {
                    self.selection_result = Some((s.id.clone(), s.project_dir.clone()));
                    self.should_quit = true;
                }
            }
            Mode::ProjectList => self.open_selected_project(),
        }
    }

    /// Switch to session-list mode for the project under the cursor. Loads
    /// sessions synchronously (JSONL parsing is fast enough that async here
    /// would be premature optimisation); if the load fails we surface a
    /// toast rather than crashing.
    fn open_selected_project(&mut self) {
        let Some(&project_idx) = self.filtered_indices.get(self.cursor) else {
            return;
        };
        let project = match self.projects.get(project_idx) {
            Some(p) => p.clone(),
            None => return,
        };

        match crate::commands::pick::load_sessions_for(&project) {
            Ok(sessions) if !sessions.is_empty() => {
                self.selected_project = Some(project_idx);
                self.sessions = sessions;
                self.selected_session = Some(0);
                self.mode = Mode::SessionList;
                self.filter.clear();
                self.rebuild_haystacks();
                self.apply_filter();
            }
            Ok(_) => {
                self.toast = Some(Toast::new(
                    format!("{}: no sessions", project.name),
                    ToastKind::Info,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(format!("load error: {e}"), ToastKind::Error));
            }
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        let len = self.filtered_indices.len();
        if len == 0 {
            return;
        }
        let current = self.cursor as i32;
        let next = (current + delta).rem_euclid(len as i32);
        self.cursor = next as usize;
    }

    fn filter_push(&mut self, c: char) {
        self.filter.push(c);
        self.apply_filter();
    }

    fn filter_backspace(&mut self) {
        self.filter.pop();
        self.apply_filter();
    }

    fn toggle_bookmark(&mut self) {
        let Some(s) = self.selected_session_ref().cloned() else {
            return;
        };
        match self.bookmarks.toggle(&s.id) {
            Ok(true) => {
                self.toast = Some(Toast::new("pinned", ToastKind::Success));
            }
            Ok(false) => {
                self.toast = Some(Toast::new("unpinned", ToastKind::Info));
            }
            Err(e) => {
                self.toast = Some(Toast::new(format!("bookmark error: {e}"), ToastKind::Error));
            }
        }
    }

    fn export_session(&mut self) {
        let Some(s) = self.selected_session_ref().cloned() else {
            return;
        };
        // Shell out to the legacy Python exporter — replacing it is Day-2 work
        // explicitly called out in the brief.
        let repo_root = find_repo_root();
        let script = repo_root.map(|r| r.join("lib").join("session-export.py"));
        let Some(script) = script else {
            self.toast = Some(Toast::new(
                "export: could not locate session-export.py",
                ToastKind::Error,
            ));
            return;
        };
        let spawn = Command::new("python3").arg(&script).arg(&s.id).spawn();
        match spawn {
            Ok(_) => {
                self.toast = Some(Toast::new(
                    format!("exporting {}", &s.id[..8.min(s.id.len())]),
                    ToastKind::Info,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(format!("export failed: {e}"), ToastKind::Error));
            }
        }
    }

    fn request_delete(&mut self) {
        if self.selected_session_ref().is_some() {
            self.show_delete_confirm = true;
        }
    }

    /// Called once per frame to retire expired toasts.
    pub fn tick(&mut self) {
        if let Some(t) = &self.toast {
            if t.is_expired() {
                self.toast = None;
            }
        }
    }
}

fn is_filter_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '/' | '@')
}

/// Best-effort delete of the session JSONL.
fn delete_session_file(session: &Session) -> anyhow::Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects = home.join(".claude").join("projects");
    for entry in std::fs::read_dir(projects)? {
        let Ok(entry) = entry else { continue };
        let candidate = entry.path().join(format!("{}.jsonl", session.id));
        if candidate.is_file() {
            std::fs::remove_file(candidate)?;
            return Ok(());
        }
    }
    Err(anyhow::anyhow!("session file not found"))
}

/// Walk up from the current binary's directory looking for the repo root
/// (identified by the `lib/` directory we ship the Python tools in). Used to
/// locate the legacy exporter until it's ported.
fn find_repo_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?.to_path_buf();
    for _ in 0..6 {
        if dir.join("lib").join("session-export.py").is_file() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

// ── Terminal lifecycle ─────────────────────────────────────────────────

/// Claim the terminal: raw mode, alt screen, mouse capture.
pub fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Undo [`setup_terminal`]. Called in `run`'s defer position so a panic still
/// restores the user's shell.
pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Top-level event-loop driver.
///
/// Returns `Ok(Some((session_id, cwd)))` if the user selected something,
/// `Ok(None)` if they quit without picking.
pub fn run(mut app: App) -> anyhow::Result<Option<(String, PathBuf)>> {
    let mut terminal = setup_terminal()?;
    // Install a panic hook that restores the terminal so a crash doesn't
    // leave the shell in raw mode.
    install_panic_hook();

    let result: anyhow::Result<Option<(String, PathBuf)>> = (|| {
        while !app.should_quit {
            terminal.draw(|f| crate::ui::picker::render(f, &app))?;
            app.tick();
            if let Some(ev) = events::next()? {
                app.handle_event(ev)?;
            }
        }
        Ok(app.selection_result.clone())
    })();

    // Always restore, even on error.
    let _ = restore_terminal(&mut terminal);
    result
}

fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut stdout = io::stdout();
        let _ = disable_raw_mode();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_session(id: &str, name: Option<&str>) -> Session {
        use crate::data::pricing::TokenCounts;
        use crate::data::session::SessionKind;
        Session {
            id: id.to_string(),
            project_dir: PathBuf::from("/tmp"),
            name: name.map(|s| s.to_string()),
            auto_name: None,
            message_count: 5,
            tokens: TokenCounts::default(),
            total_cost_usd: 0.0,
            model_summary: "claude-opus-4-7".to_string(),
            first_timestamp: None,
            last_timestamp: None,
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
        }
    }

    #[test]
    fn filter_matches_substring() {
        let sessions = vec![
            mk_session("a", Some("auth-refactor")),
            mk_session("b", Some("fix-race-condition")),
            mk_session("c", Some("drizzle-migration")),
        ];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);

        app.filter.push_str("auth");
        app.apply_filter();
        assert_eq!(app.filtered_indices.len(), 1);
        assert_eq!(app.filtered_indices[0], 0);

        app.filter.clear();
        app.filter.push_str("migration");
        app.apply_filter();
        assert_eq!(app.filtered_indices.len(), 1);
        assert_eq!(app.filtered_indices[0], 2);
    }

    #[test]
    fn empty_filter_shows_all() {
        let sessions = vec![mk_session("a", Some("x")), mk_session("b", Some("y"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        assert_eq!(app.filtered_indices.len(), 2);
    }

    #[test]
    fn escape_clears_filter_then_quits() {
        let sessions = vec![mk_session("a", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.filter.push_str("abc");
        app.apply_filter();
        app.handle_event(Event::Escape).unwrap();
        assert!(app.filter.is_empty(), "first Escape should clear filter");
        assert!(!app.should_quit);
        app.handle_event(Event::Escape).unwrap();
        assert!(
            app.should_quit,
            "second Escape on no-project-list must quit"
        );
    }

    #[test]
    fn arrow_navigation_wraps() {
        let sessions = vec![mk_session("a", Some("a")), mk_session("b", Some("b"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        assert_eq!(app.cursor, 0);
        app.handle_event(Event::Down).unwrap();
        assert_eq!(app.cursor, 1);
        app.handle_event(Event::Down).unwrap();
        assert_eq!(app.cursor, 0, "wraps around");
        app.handle_event(Event::Up).unwrap();
        assert_eq!(app.cursor, 1, "up from top wraps to bottom");
    }

    #[test]
    fn enter_records_selection() {
        let sessions = vec![mk_session("abc123", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Enter).unwrap();
        assert!(app.should_quit);
        assert_eq!(
            app.selection_result.as_ref().map(|r| r.0.as_str()),
            Some("abc123")
        );
    }

    #[test]
    fn q_quits_when_filter_empty() {
        let sessions = vec![mk_session("a", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Key('q')).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn q_typed_into_filter_does_not_quit() {
        let sessions = vec![mk_session("a", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.filter.push('a');
        app.handle_event(Event::Key('q')).unwrap();
        assert!(!app.should_quit);
        assert_eq!(app.filter, "aq");
    }
}
