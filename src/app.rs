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

use std::collections::HashSet;
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
use crate::data::{clipboard, editor, session_rename, Project, Session};
use crate::events::{self, Event};
use crate::theme::{self, Theme, ThemeName};
use crate::ui::command_palette::{self, CommandPalette};
use crate::ui::conversation_viewer::{ToastKind as ViewerToastKind, ViewerAction, ViewerState};
use crate::ui::help_overlay::{self, Screen as HelpScreen};
use crate::ui::rename_modal::{self, RenameState};

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
    /// `?` help overlay visible.
    pub show_help: bool,
    /// Rename modal state — `Some` while the user is editing a name.
    pub rename: Option<RenameState>,
    /// Space-leader command palette — `Some` while the palette modal is
    /// open. Swallows input until closed.
    pub palette: Option<CommandPalette>,
    /// Full-screen conversation viewer — `Some` while the user is reading
    /// a session's transcript.
    pub viewer: Option<ViewerState>,
    /// Session ids the user has multi-selected via Tab.
    pub multi_selected: HashSet<String>,
    /// True when multi-select mode is engaged. Distinct from
    /// `!multi_selected.is_empty()` because Tab deselecting the last row
    /// should still keep the UI in multi-mode so the footer hints stay
    /// visible until the user explicitly Escapes out.
    pub multi_mode: bool,
    /// Timestamp of the last `g` press, used for the `gg` vim chord. Cleared
    /// on any unrelated key press or after [`G_CHORD_WINDOW`].
    pending_g: Option<Instant>,

    /// nucleo scratch. Rebuilt on filter change.
    matcher: Matcher,
    /// Precomputed haystacks for the current mode.
    haystacks: Vec<Utf32String>,
}

/// Window in which two `g` presses collapse into a jump-to-top. Matches the
/// vim / lazygit norm — 500 ms is forgiving but still feels chord-y.
const G_CHORD_WINDOW: Duration = Duration::from_millis(500);

impl App {
    /// Construct an initial state with the default (Mocha) theme. Callers
    /// decide whether to land on [`Mode::ProjectList`] or [`Mode::SessionList`]
    /// by pre-seeding the sessions vector. If both are set, session-list wins
    /// and the project-list pops back in when the user hits Esc.
    pub fn new(
        projects: Vec<Project>,
        sessions: Vec<Session>,
        bookmarks: BookmarkStore,
        mode: Mode,
        selected_project: Option<usize>,
    ) -> Self {
        Self::new_with_theme(
            projects,
            sessions,
            bookmarks,
            mode,
            selected_project,
            ThemeName::default(),
        )
    }

    /// Like [`Self::new`] but starts with an explicit theme. Separate method
    /// rather than an overload so existing test callers don't need to learn
    /// about theme resolution.
    pub fn new_with_theme(
        projects: Vec<Project>,
        sessions: Vec<Session>,
        bookmarks: BookmarkStore,
        mode: Mode,
        selected_project: Option<usize>,
        theme_name: ThemeName,
    ) -> Self {
        let theme = Theme::from_name(theme_name);
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
            show_help: false,
            rename: None,
            palette: None,
            viewer: None,
            multi_selected: HashSet::new(),
            multi_mode: false,
            pending_g: None,
            matcher,
            haystacks: Vec::new(),
        };
        s.rebuild_haystacks();
        s.apply_filter();
        s
    }

    /// Cycle to the next theme in [`ThemeName::ALL`], replace the live
    /// `self.theme`, show a 1-second confirmation toast, and persist the
    /// choice to `~/.config/claude-picker/theme` (best-effort — persistence
    /// errors surface as toasts but don't revert the change).
    pub fn cycle_theme(&mut self) {
        let next = self.theme.name.next();
        self.theme = Theme::from_name(next);
        match theme::save_persisted_theme(next) {
            Ok(()) => {
                self.toast = Some(Toast::new(
                    format!("theme: {}", next.label()),
                    ToastKind::Info,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(
                    format!("theme: {} (save failed: {})", next.label(), e),
                    ToastKind::Error,
                ));
            }
        }
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
        // Modal inputs take precedence, in reverse visual-stack order.
        if self.viewer.is_some() {
            return self.handle_viewer(ev);
        }
        if self.palette.is_some() {
            return self.handle_palette(ev);
        }
        if self.rename.is_some() {
            return self.handle_rename(ev);
        }
        if self.show_delete_confirm {
            return self.handle_delete_confirm(ev);
        }
        if self.show_help {
            return self.handle_help_overlay(ev);
        }

        // Expire a stale `gg` chord before processing further — a key other
        // than `g` arriving outside the window should not linger as pending.
        if let Some(t) = self.pending_g {
            if t.elapsed() > G_CHORD_WINDOW {
                self.pending_g = None;
            }
        }

        match ev {
            Event::Quit | Event::Ctrl('c') => self.should_quit = true,
            Event::Ctrl('d') => self.request_delete(),
            Event::Ctrl('b') => self.toggle_bookmark(),
            Event::Ctrl('e') => self.export_session(),
            Event::Tab => self.toggle_multi_select(),
            Event::Enter => self.confirm_selection(),
            Event::Escape => self.handle_escape(),
            Event::Up => self.move_cursor(-1),
            Event::Down => self.move_cursor(1),
            Event::PageUp => self.move_cursor(-10),
            Event::PageDown => self.move_cursor(10),
            Event::Home => self.cursor = 0,
            Event::End => self.cursor = self.filtered_indices.len().saturating_sub(1),
            Event::Backspace => self.filter_backspace(),
            // `?` opens the context-sensitive help overlay whenever the filter
            // is empty. If someone's typing `?` into the filter they can
            // still escape-and-type.
            Event::Key('?') if self.filter.is_empty() => self.show_help = true,
            // `G` (shift-G) jumps to the end — no chord.
            Event::Key('G') if self.filter.is_empty() => {
                self.cursor = self.filtered_indices.len().saturating_sub(1);
                self.pending_g = None;
            }
            // `g` pressed: if a previous `g` is still within the window, this
            // completes a `gg` chord → jump to top. Otherwise remember the
            // keystroke so the next `g` can complete the chord.
            Event::Key('g') if self.filter.is_empty() => {
                if self
                    .pending_g
                    .map(|t| t.elapsed() <= G_CHORD_WINDOW)
                    .unwrap_or(false)
                {
                    self.cursor = 0;
                    self.pending_g = None;
                } else {
                    self.pending_g = Some(Instant::now());
                }
            }
            // `v` opens the full-screen conversation viewer for the row
            // under the cursor. Session-list only — project-list has no
            // session to view.
            Event::Key('v') if self.filter.is_empty() && self.mode == Mode::SessionList => {
                self.open_viewer();
            }
            // `y` / `Y` copy to clipboard (lowercase = session id, uppercase
            // = project path). Both require an empty filter so typing them
            // into a search still works. In multi-select mode these copy the
            // set of selected session ids / project paths instead.
            Event::Key('y') if self.filter.is_empty() => self.copy_session_id(),
            Event::Key('Y') if self.filter.is_empty() => self.copy_project_path(),
            // `r` opens the rename modal for the selected session.
            Event::Key('r') if self.filter.is_empty() => self.request_rename(),
            // `o` launches `$EDITOR <project_path>` detached.
            Event::Key('o') if self.filter.is_empty() => self.open_editor_for_selection(),
            Event::Key(c) if c == 'q' && self.filter.is_empty() => self.should_quit = true,
            // `t` cycles the theme when the filter is empty. If the user is
            // typing a filter (including searches with `t` in them) the letter
            // goes to the filter via the fallthrough branch below.
            Event::Key('t') if self.filter.is_empty() => self.cycle_theme(),
            // Space opens the Helix-style command palette when the filter is
            // empty. Inside an active filter the space goes to the filter so
            // the user can type multi-word queries (nucleo supports them).
            Event::Key(' ') if self.filter.is_empty() => {
                self.pending_g = None;
                self.palette = Some(CommandPalette::new(match self.mode {
                    Mode::SessionList => command_palette::Context::SessionList,
                    Mode::ProjectList => command_palette::Context::ProjectList,
                }));
            }
            Event::Key(c) if is_filter_char(c) => {
                // Any keystroke other than the chord letters breaks `gg`.
                self.pending_g = None;
                self.filter_push(c);
            }
            Event::Resize(_, _) => {}
            _ => {
                // Unknown event — clear any pending chord so we don't match
                // `g<tab>g` or similar across stale timers.
                self.pending_g = None;
            }
        }
        Ok(())
    }

    /// Forward an event to the open viewer and act on its reply.
    fn handle_viewer(&mut self, ev: Event) -> anyhow::Result<()> {
        let Some(viewer) = self.viewer.as_mut() else {
            return Ok(());
        };
        match viewer.handle_event(ev) {
            ViewerAction::None => {}
            ViewerAction::Close => {
                self.viewer = None;
            }
            ViewerAction::Toast(message, kind) => {
                let app_kind = match kind {
                    ViewerToastKind::Info => ToastKind::Info,
                    ViewerToastKind::Success => ToastKind::Success,
                    ViewerToastKind::Error => ToastKind::Error,
                };
                self.toast = Some(Toast::new(message, app_kind));
            }
        }
        Ok(())
    }

    /// Open the conversation viewer for the row currently under the cursor.
    /// Quietly no-ops if nothing's selected.
    pub fn open_viewer(&mut self) {
        let Some(session) = self.selected_session_ref().cloned() else {
            return;
        };
        self.viewer = Some(ViewerState::open(&session));
    }

    /// Toggle multi-select on the row under the cursor. First Tab press
    /// flips multi-mode on; later presses toggle individual selection.
    fn toggle_multi_select(&mut self) {
        if self.mode != Mode::SessionList {
            return;
        }
        let Some(s) = self.selected_session_ref().cloned() else {
            return;
        };
        if !self.multi_mode {
            self.multi_mode = true;
        }
        if self.multi_selected.contains(&s.id) {
            self.multi_selected.remove(&s.id);
        } else {
            self.multi_selected.insert(s.id);
        }
    }

    /// Clear any active multi-selection and exit multi-mode.
    pub fn clear_multi_select(&mut self) {
        self.multi_selected.clear();
        self.multi_mode = false;
    }

    /// Total number of rows currently multi-selected.
    pub fn multi_selected_count(&self) -> usize {
        self.multi_selected.len()
    }

    /// True when the row at `sess_idx` into `self.sessions` is part of the
    /// live multi-selection.
    pub fn is_multi_selected(&self, sess_idx: usize) -> bool {
        self.sessions
            .get(sess_idx)
            .map(|s| self.multi_selected.contains(&s.id))
            .unwrap_or(false)
    }

    /// Sessions in the multi-selection, in the order they appear in
    /// `self.sessions` (so Ctrl-E exports stably, not in hash order).
    fn multi_selected_sessions(&self) -> Vec<&Session> {
        self.sessions
            .iter()
            .filter(|s| self.multi_selected.contains(&s.id))
            .collect()
    }

    /// Current screen for the help overlay. Only the picker-level modes are
    /// reachable from `App`; subcommand screens own their own event loops.
    pub fn help_screen(&self) -> HelpScreen {
        match self.mode {
            Mode::SessionList => HelpScreen::SessionList,
            Mode::ProjectList => HelpScreen::ProjectList,
        }
    }

    fn handle_help_overlay(&mut self, ev: Event) -> anyhow::Result<()> {
        match ev {
            Event::Escape => self.show_help = false,
            Event::Key(c) if help_overlay::is_dismiss_key(c) => self.show_help = false,
            Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    /// Forward an event to the live command palette and act on the
    /// outcome. On [`Outcome::Execute`] we close the palette first and
    /// *then* dispatch the action — doing it in that order means any
    /// follow-up modal (rename, delete-confirm, viewer) doesn't get
    /// stacked under the palette.
    fn handle_palette(&mut self, ev: Event) -> anyhow::Result<()> {
        let Some(palette) = self.palette.as_mut() else {
            return Ok(());
        };
        let outcome = palette.handle_event(ev);
        match outcome {
            command_palette::Outcome::Continue => {}
            command_palette::Outcome::Close => {
                self.palette = None;
            }
            command_palette::Outcome::Execute(id) => {
                self.palette = None;
                self.execute_palette_action(id);
            }
        }
        Ok(())
    }

    /// Map palette action ids to state mutations. Unknown ids are
    /// ignored rather than panicking so a stale palette-in-flight
    /// can't crash the app.
    fn execute_palette_action(&mut self, id: &'static str) {
        match id {
            "resume" | "open_project" => self.confirm_selection(),
            "export" => self.export_session(),
            "rename" => self.request_rename(),
            "bookmark" => self.toggle_bookmark(),
            "delete" => self.request_delete(),
            "copy_session_id" => self.copy_session_id(),
            "copy_project_path" => self.copy_project_path(),
            "open_editor" => self.open_editor_for_selection(),
            "view_conversation" => self.open_viewer(),
            "toggle_theme" => self.cycle_theme(),
            "help" => self.show_help = true,
            "quit" => self.should_quit = true,
            _ => {}
        }
    }

    fn handle_rename(&mut self, ev: Event) -> anyhow::Result<()> {
        let Some(state) = self.rename.as_mut() else {
            return Ok(());
        };
        match ev {
            Event::Enter => {
                let new_name = state.buffer.trim().to_string();
                let session_id = state.session_id.clone();
                self.rename = None;
                if new_name.is_empty() {
                    self.toast = Some(Toast::new("rename: name can't be empty", ToastKind::Error));
                    return Ok(());
                }
                match session_rename::rename_session(&session_id, &new_name) {
                    Ok(_) => {
                        // Update in-memory so the list reflects immediately.
                        if let Some(s) = self.sessions.iter_mut().find(|s| s.id == session_id) {
                            s.name = Some(new_name.clone());
                        }
                        self.rebuild_haystacks();
                        self.apply_filter();
                        self.toast = Some(Toast::new(
                            format!("renamed to \"{new_name}\""),
                            ToastKind::Success,
                        ));
                    }
                    Err(e) => {
                        self.toast =
                            Some(Toast::new(format!("rename failed: {e}"), ToastKind::Error));
                    }
                }
            }
            Event::Escape => {
                self.rename = None;
            }
            Event::Backspace => state.pop(),
            Event::Key(c) if rename_modal::is_name_char(c) => state.push(c),
            _ => {}
        }
        Ok(())
    }

    fn copy_session_id(&mut self) {
        // Multi-select: copy every selected id, newline-separated.
        if self.multi_mode && !self.multi_selected.is_empty() {
            let ids: Vec<String> = self
                .multi_selected_sessions()
                .iter()
                .map(|s| s.id.clone())
                .collect();
            let count = ids.len();
            let payload = ids.join("\n");
            match clipboard::copy(payload) {
                Ok(()) => {
                    self.toast = Some(Toast::new(
                        format!("copied {count} session IDs"),
                        ToastKind::Success,
                    ));
                }
                Err(e) => {
                    self.toast = Some(Toast::new(
                        format!("clipboard unavailable: {e}"),
                        ToastKind::Error,
                    ));
                }
            }
            return;
        }
        let Some(s) = self.selected_session_ref().cloned() else {
            return;
        };
        let short = s.id.chars().take(8).collect::<String>();
        match clipboard::copy(s.id.clone()) {
            Ok(()) => {
                self.toast = Some(Toast::new(
                    format!("copied {short} to clipboard"),
                    ToastKind::Success,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(
                    format!("clipboard unavailable: {e}"),
                    ToastKind::Error,
                ));
            }
        }
    }

    fn copy_project_path(&mut self) {
        // Multi-select: copy deduped project paths, newline-separated.
        if self.multi_mode && !self.multi_selected.is_empty() {
            let mut seen: HashSet<PathBuf> = HashSet::new();
            let mut paths: Vec<String> = Vec::new();
            for s in self.multi_selected_sessions() {
                if seen.insert(s.project_dir.clone()) {
                    paths.push(s.project_dir.display().to_string());
                }
            }
            let count = paths.len();
            let payload = paths.join("\n");
            match clipboard::copy(payload) {
                Ok(()) => {
                    self.toast = Some(Toast::new(
                        format!("copied {count} project paths"),
                        ToastKind::Success,
                    ));
                }
                Err(e) => {
                    self.toast = Some(Toast::new(
                        format!("clipboard unavailable: {e}"),
                        ToastKind::Error,
                    ));
                }
            }
            return;
        }
        let path: PathBuf = match self.mode {
            Mode::SessionList => match self.selected_session_ref() {
                Some(s) => s.project_dir.clone(),
                None => return,
            },
            Mode::ProjectList => {
                let Some(&idx) = self.filtered_indices.get(self.cursor) else {
                    return;
                };
                match self.projects.get(idx) {
                    Some(p) => p.path.clone(),
                    None => return,
                }
            }
        };
        let display = path.display().to_string();
        match clipboard::copy(display.clone()) {
            Ok(()) => {
                // Shorten long paths in the toast so it doesn't wrap weirdly.
                let shown = if display.len() > 40 {
                    format!("…{}", &display[display.len() - 39..])
                } else {
                    display
                };
                self.toast = Some(Toast::new(
                    format!("copied {shown} to clipboard"),
                    ToastKind::Success,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(
                    format!("clipboard unavailable: {e}"),
                    ToastKind::Error,
                ));
            }
        }
    }

    fn request_rename(&mut self) {
        // Only session-list has per-session names we can edit.
        if self.mode != Mode::SessionList {
            return;
        }
        let Some(s) = self.selected_session_ref() else {
            return;
        };
        self.rename = Some(RenameState::new(s.id.clone(), s.name.as_deref()));
    }

    fn open_editor_for_selection(&mut self) {
        let path: PathBuf = match self.mode {
            Mode::SessionList => match self.selected_session_ref() {
                Some(s) => s.project_dir.clone(),
                None => return,
            },
            Mode::ProjectList => {
                let Some(&idx) = self.filtered_indices.get(self.cursor) else {
                    return;
                };
                match self.projects.get(idx) {
                    Some(p) => p.path.clone(),
                    None => return,
                }
            }
        };
        match editor::open_in_editor(&path) {
            Ok(name) => {
                self.toast = Some(Toast::new(
                    format!("opened {} in {name}", path.display()),
                    ToastKind::Info,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(format!("editor: {e}"), ToastKind::Error));
            }
        }
    }

    fn handle_delete_confirm(&mut self, ev: Event) -> anyhow::Result<()> {
        match ev {
            Event::Key('y') | Event::Key('Y') => {
                self.show_delete_confirm = false;
                // Multi-select batch delete: loop over every id, remove
                // from the in-memory list on success.
                if self.multi_mode && !self.multi_selected.is_empty() {
                    let ids: Vec<String> = self
                        .multi_selected_sessions()
                        .iter()
                        .map(|s| s.id.clone())
                        .collect();
                    let mut ok = 0usize;
                    let mut err_msg: Option<String> = None;
                    for id in ids.iter() {
                        // Synthesise a minimal Session-like struct to reuse the
                        // delete helper's resolver.
                        if let Some(s) = self.sessions.iter().find(|s| &s.id == id).cloned() {
                            match delete_session_file(&s) {
                                Ok(()) => ok += 1,
                                Err(e) => {
                                    err_msg = Some(format!("{e}"));
                                    break;
                                }
                            }
                        }
                    }
                    // Drop the deleted sessions from the picker.
                    self.sessions.retain(|s| !ids.contains(&s.id));
                    self.clear_multi_select();
                    self.rebuild_haystacks();
                    self.apply_filter();
                    if let Some(e) = err_msg {
                        self.toast =
                            Some(Toast::new(format!("delete failed: {e}"), ToastKind::Error));
                    } else {
                        self.toast = Some(Toast::new(
                            format!("deleted {ok} sessions"),
                            ToastKind::Success,
                        ));
                    }
                    return Ok(());
                }
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
        // Esc with no filter but an active multi-selection: clear it.
        // Keeps the picker where it is so users don't accidentally pop
        // back to the project list after a long Tab session.
        if self.multi_mode {
            self.clear_multi_select();
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
        // Multi-select: export every selected session in sequence.
        if self.multi_mode && !self.multi_selected.is_empty() {
            let ids: Vec<String> = self
                .multi_selected_sessions()
                .iter()
                .map(|s| s.id.clone())
                .collect();
            let count = ids.len();
            let repo_root = find_repo_root();
            let Some(script) = repo_root.map(|r| r.join("lib").join("session-export.py")) else {
                self.toast = Some(Toast::new(
                    "export: could not locate session-export.py",
                    ToastKind::Error,
                ));
                return;
            };
            let mut ok = 0usize;
            let mut err_msg: Option<String> = None;
            for id in ids {
                match Command::new("python3").arg(&script).arg(&id).spawn() {
                    Ok(_) => ok += 1,
                    Err(e) => {
                        err_msg = Some(format!("{e}"));
                        break;
                    }
                }
            }
            if let Some(e) = err_msg {
                self.toast = Some(Toast::new(format!("export failed: {e}"), ToastKind::Error));
            } else {
                self.toast = Some(Toast::new(
                    format!("exported {ok} of {count} sessions"),
                    ToastKind::Success,
                ));
            }
            return;
        }

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
        // Multi-select: raise the confirm modal if there are pending
        // selections, regardless of whether the cursor row is deletable.
        if self.multi_mode && !self.multi_selected.is_empty() {
            self.show_delete_confirm = true;
            return;
        }
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
///
/// We canonicalize first so a symlinked install (`~/.local/bin/claude-picker`
/// → `~/Desktop/claude-picker/target/release/claude-picker`) still resolves
/// to the real binary location, from which we can walk up to find `lib/`.
fn find_repo_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
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
            terminal.draw(|f| crate::ui::picker::render(f, &mut app))?;
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
            last_prompt: None,
            message_count: 5,
            tokens: TokenCounts::default(),
            total_cost_usd: 0.0,
            model_summary: "claude-opus-4-7".to_string(),
            first_timestamp: None,
            last_timestamp: None,
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
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

    #[test]
    fn tab_enters_multi_select_and_toggles_rows() {
        let sessions = vec![
            mk_session("a1", Some("alpha")),
            mk_session("b2", Some("bravo")),
            mk_session("c3", Some("charlie")),
        ];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        assert!(!app.multi_mode);
        app.handle_event(Event::Tab).unwrap();
        assert!(app.multi_mode);
        assert_eq!(app.multi_selected_count(), 1);
        app.handle_event(Event::Down).unwrap();
        app.handle_event(Event::Tab).unwrap();
        assert_eq!(app.multi_selected_count(), 2);
        // Tab again on the same row toggles off.
        app.handle_event(Event::Tab).unwrap();
        assert_eq!(app.multi_selected_count(), 1);
    }

    #[test]
    fn esc_clears_multi_select_without_popping_mode() {
        let sessions = vec![mk_session("a1", Some("alpha"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Tab).unwrap();
        assert_eq!(app.multi_selected_count(), 1);
        app.handle_event(Event::Escape).unwrap();
        assert_eq!(app.multi_selected_count(), 0);
        assert!(!app.multi_mode);
        assert!(!app.should_quit, "Esc on multi-selection must not quit");
    }

    #[test]
    fn is_multi_selected_reports_correctly() {
        let sessions = vec![mk_session("a1", Some("a")), mk_session("b2", Some("b"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Tab).unwrap();
        assert!(app.is_multi_selected(0));
        assert!(!app.is_multi_selected(1));
    }

    #[test]
    fn v_key_opens_viewer_when_filter_empty() {
        let sessions = vec![mk_session("abcdef1234", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Key('v')).unwrap();
        assert!(
            app.viewer.is_some(),
            "viewer must open for a real selected row"
        );
    }

    #[test]
    fn v_key_typed_into_filter_does_not_open_viewer() {
        let sessions = vec![mk_session("a", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.filter.push('s');
        app.handle_event(Event::Key('v')).unwrap();
        assert!(app.viewer.is_none());
        assert_eq!(app.filter, "sv");
    }

    #[test]
    fn tab_in_project_list_is_no_op() {
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], vec![], bm, Mode::ProjectList, None);
        app.handle_event(Event::Tab).unwrap();
        assert!(!app.multi_mode);
    }
}
