//! `claude-picker tree` — interactive session fork-tree screen.
//!
//! Walks every project under `~/.claude/projects/`, loads their sessions
//! via the data layer, and presents a single scrollable panel grouped by
//! project. Fork relationships (via `forkedFrom`) nest children under
//! their parents using ASCII tree connectors.
//!
//! Keyboard model:
//! - `↑/↓` or `j/k` — move the cursor, wrapping at the ends. Headers and
//!   spacer rows are skipped.
//! - `Enter` — if the cursor is on a session row, print the selection
//!   directive to stdout and exit; a shell wrapper picks that up and
//!   execs `claude --resume`.
//! - `q` / `Esc` / `Ctrl+C` — quit without selecting.
//!
//! The flatten + render logic lives in [`crate::ui::tree`]; this module
//! is the event loop + data-loading glue.

use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::{Frame, Terminal};

use crate::commands::pick::load_sessions_for;
use crate::data::{clipboard, editor, project, Project, Session};
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::command_palette::{self, CommandPalette};
use crate::ui::conversation_viewer::{
    self as viewer_ui, ToastKind as ViewerToastKind, ViewerAction, ViewerState,
};
use crate::ui::help_overlay::{self, Screen as HelpScreen};
use crate::ui::tree::{
    build_tree_with_collapsed, collapsible_fork_root_ids, render as render_tree, NodeKind, TreeNode,
};

/// Window in which two `g` presses become a jump-to-top chord.
const G_CHORD_WINDOW: Duration = Duration::from_millis(500);

/// Lightweight transient toast — the subcommand screens don't own the
/// picker's richer state machine, so this local flavour is enough.
struct Toast {
    message: String,
    kind: ToastKind,
    expires_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastKind {
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

/// Entry point for `claude-picker tree`.
pub fn run() -> anyhow::Result<()> {
    let (projects, sessions_by_project) = load_data()?;

    // Seed the collapsed set with every fork root so the default view is
    // drill-down-ready: roots only, hit `→` to open the subtree.
    let initial_collapsed = collapsible_fork_root_ids(&sessions_by_project);
    let nodes = build_tree_with_collapsed(&projects, &sessions_by_project, &initial_collapsed);

    // Special-case the empty state: no alt-screen dance, just print and
    // exit so scripted callers don't need to toggle a terminal.
    if nodes
        .iter()
        .all(|n| !matches!(n.kind, NodeKind::SessionRow { .. }))
    {
        print_empty_and_exit();
        return Ok(());
    }

    let theme = Theme::mocha();
    let mut state = TreeState::new(nodes, projects, sessions_by_project, initial_collapsed);

    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let result: anyhow::Result<Option<Selection>> = (|| {
        while !state.should_quit {
            terminal.draw(|f| render_screen(f, &mut state, &theme))?;
            state.tick();
            if let Some(ev) = events::next()? {
                state.handle_event(ev);
            }
        }
        Ok(state.selection.take())
    })();

    let _ = restore_terminal(&mut terminal);

    if let Some(sel) = result? {
        crate::resume::resume_session(&sel.session_id, &sel.project_cwd); // diverges
    }
    Ok(())
}

/// Selected session + project cwd. Printed to stdout on Enter.
struct Selection {
    session_id: String,
    project_cwd: PathBuf,
}

/// Collect all projects and their sessions. Empty projects are dropped.
fn load_data() -> anyhow::Result<(Vec<Project>, Vec<Vec<Session>>)> {
    let mut projects = project::discover_projects()?;
    // Stable alphabetical order by project name so the tree doesn't
    // shuffle between invocations — the picker's default list uses
    // recency, but the tree reads better grouped alphabetically.
    projects.sort_by_key(|p| p.name.to_lowercase());

    let mut sessions_by_project: Vec<Vec<Session>> = Vec::with_capacity(projects.len());
    let mut kept_projects: Vec<Project> = Vec::with_capacity(projects.len());
    for p in projects {
        match load_sessions_for(&p) {
            Ok(ss) if !ss.is_empty() => {
                sessions_by_project.push(ss);
                kept_projects.push(p);
            }
            Ok(_) => {} // skip empty projects
            Err(e) => eprintln!("{}: load error: {e}", p.name),
        }
    }
    Ok((kept_projects, sessions_by_project))
}

fn print_empty_and_exit() {
    eprintln!();
    eprintln!("  No Claude Code sessions found.");
    eprintln!("  Run `claude` somewhere to create one.");
    eprintln!();
}

/// Per-screen event-loop state.
struct TreeState {
    /// The *flat* rendered node list. Rebuilt from `projects` +
    /// `sessions_by_project` + `collapsed` whenever the user expands
    /// or collapses something.
    nodes: Vec<TreeNode>,
    /// Raw project list — retained so we can re-flatten after an
    /// expand/collapse without going back to disk.
    projects: Vec<Project>,
    /// Raw per-project session lists, same shape as above.
    sessions_by_project: Vec<Vec<Session>>,
    /// Session ids whose fork subtrees should be hidden. Starts with
    /// *every* collapsible root (so only top-level rows are visible by
    /// default); `→`/`Space`/`l` remove ids from this set, `←`/`h` add
    /// them back.
    collapsed: std::collections::HashSet<String>,
    /// Index into `nodes`. Constrained to a selectable row.
    cursor: usize,
    should_quit: bool,
    selection: Option<Selection>,
    /// `?` help overlay visible.
    show_help: bool,
    /// Space-leader command palette. `Some` while open.
    palette: Option<CommandPalette>,
    /// Full-screen conversation viewer — `Some` while reading a transcript.
    viewer: Option<ViewerState>,
    /// Transient status message (clipboard / editor feedback).
    toast: Option<Toast>,
    /// Timestamp of the last `g` press for the `gg` chord.
    pending_g: Option<Instant>,
}

impl TreeState {
    fn new(
        nodes: Vec<TreeNode>,
        projects: Vec<Project>,
        sessions_by_project: Vec<Vec<Session>>,
        collapsed: std::collections::HashSet<String>,
    ) -> Self {
        let mut s = Self {
            nodes,
            projects,
            sessions_by_project,
            collapsed,
            cursor: 0,
            should_quit: false,
            selection: None,
            show_help: false,
            palette: None,
            viewer: None,
            toast: None,
            pending_g: None,
        };
        s.cursor = s.first_selectable().unwrap_or(0);
        s
    }

    /// Re-flatten the tree after a collapse/expand change. Tries to keep
    /// the cursor on the session that was selected before the rebuild;
    /// falls back to the first selectable row when that session is no
    /// longer visible (e.g., the user collapsed its parent).
    fn rebuild(&mut self) {
        let prev_id = self
            .nodes
            .get(self.cursor)
            .and_then(|n| n.session_id().map(|s| s.to_string()));
        self.nodes =
            build_tree_with_collapsed(&self.projects, &self.sessions_by_project, &self.collapsed);
        if let Some(id) = prev_id {
            if let Some(i) = self
                .nodes
                .iter()
                .position(|n| n.session_id() == Some(id.as_str()))
            {
                self.cursor = i;
                return;
            }
        }
        self.cursor = self.first_selectable().unwrap_or(0);
    }

    /// Toggle expand/collapse on the cursor row. No-op for non-collapsible
    /// rows (headers, spacers, leaf sessions).
    fn toggle_expand(&mut self) {
        let Some(node) = self.nodes.get(self.cursor) else {
            return;
        };
        if !node.is_collapsible() {
            return;
        }
        let id = match node.session_id() {
            Some(s) => s.to_string(),
            None => return,
        };
        if self.collapsed.contains(&id) {
            self.collapsed.remove(&id);
        } else {
            self.collapsed.insert(id);
        }
        self.rebuild();
    }

    /// `→` / `l` — expand the current node if it has children.
    fn expand_current(&mut self) {
        let Some(node) = self.nodes.get(self.cursor) else {
            return;
        };
        if !node.is_collapsible() {
            return;
        }
        let id = match node.session_id() {
            Some(s) => s.to_string(),
            None => return,
        };
        if self.collapsed.contains(&id) {
            self.collapsed.remove(&id);
            self.rebuild();
        }
    }

    /// `←` / `h` — if the current node is an expanded collapsible, close
    /// it. If it's already collapsed (or a leaf), jump to the parent.
    fn collapse_or_parent(&mut self) {
        let Some(node) = self.nodes.get(self.cursor).cloned() else {
            return;
        };
        let is_collapsible = node.is_collapsible();
        let id = node.session_id().map(|s| s.to_string());
        let parent = node.parent_session_id.clone();

        // Expanded collapsible → collapse it in place.
        if is_collapsible {
            if let Some(id) = &id {
                if !self.collapsed.contains(id) {
                    self.collapsed.insert(id.clone());
                    self.rebuild();
                    return;
                }
            }
        }

        // Otherwise jump to the parent row (if one is visible).
        if let Some(pid) = parent {
            if let Some(i) = self
                .nodes
                .iter()
                .position(|n| n.session_id() == Some(pid.as_str()))
            {
                self.cursor = i;
            }
        }
    }

    fn tick(&mut self) {
        if let Some(t) = &self.toast {
            if t.is_expired() {
                self.toast = None;
            }
        }
    }

    fn first_selectable(&self) -> Option<usize> {
        self.nodes.iter().position(|n| n.is_selectable())
    }

    fn last_selectable(&self) -> Option<usize> {
        self.nodes.iter().rposition(|n| n.is_selectable())
    }

    /// Move the cursor by `delta`, wrapping at the ends and snapping to
    /// the nearest selectable row in the direction of travel.
    fn step(&mut self, delta: i32) {
        if self.nodes.is_empty() {
            return;
        }
        let Some(_) = self.first_selectable() else {
            return;
        };
        let len = self.nodes.len() as i32;
        let mut idx = self.cursor as i32;
        // Try up to `len` hops — guarantees we hit every row.
        for _ in 0..len {
            idx = (idx + delta).rem_euclid(len);
            let i = idx as usize;
            if self.nodes[i].is_selectable() {
                self.cursor = i;
                return;
            }
        }
    }

    fn jump_home(&mut self) {
        if let Some(i) = self.first_selectable() {
            self.cursor = i;
        }
    }

    fn jump_end(&mut self) {
        if let Some(i) = self.last_selectable() {
            self.cursor = i;
        }
    }

    fn confirm(&mut self) {
        let Some(node) = self.nodes.get(self.cursor) else {
            return;
        };
        if let NodeKind::SessionRow { session } = &node.kind {
            self.selection = Some(Selection {
                session_id: session.id.clone(),
                project_cwd: session.project_dir.clone(),
            });
            self.should_quit = true;
        }
    }

    fn handle_event(&mut self, ev: Event) {
        // Modal overlays steal input. Viewer → palette → help → normal dispatch.
        if self.viewer.is_some() {
            self.handle_viewer_event(ev);
            return;
        }
        if self.palette.is_some() {
            self.handle_palette_event(ev);
            return;
        }
        if self.show_help {
            match ev {
                Event::Escape => self.show_help = false,
                Event::Key(c) if help_overlay::is_dismiss_key(c) => self.show_help = false,
                _ => {}
            }
            return;
        }

        // Expire a stale `gg` chord before processing further.
        if let Some(t) = self.pending_g {
            if t.elapsed() > G_CHORD_WINDOW {
                self.pending_g = None;
            }
        }

        match ev {
            Event::Quit | Event::Escape | Event::Ctrl('c') => self.should_quit = true,
            Event::Key('q') => self.should_quit = true,
            Event::Up | Event::Key('k') => {
                self.pending_g = None;
                self.step(-1);
            }
            Event::Down | Event::Key('j') => {
                self.pending_g = None;
                self.step(1);
            }
            Event::PageUp => {
                self.pending_g = None;
                self.step(-10);
            }
            Event::PageDown => {
                self.pending_g = None;
                self.step(10);
            }
            Event::Home => {
                self.pending_g = None;
                self.jump_home();
            }
            Event::End => {
                self.pending_g = None;
                self.jump_end();
            }
            // Drill-down: `→` / `l` expand, `←` / `h` collapse-or-parent.
            Event::Right | Event::Key('l') => {
                self.pending_g = None;
                self.expand_current();
            }
            Event::Left | Event::Key('h') => {
                self.pending_g = None;
                self.collapse_or_parent();
            }
            // Space toggles expansion on the current row; if the cursor
            // isn't on a collapsible node, it opens the palette instead
            // so the key still feels useful.
            Event::Key(' ') => {
                self.pending_g = None;
                let is_collapsible = self
                    .nodes
                    .get(self.cursor)
                    .map(|n| n.is_collapsible())
                    .unwrap_or(false);
                if is_collapsible {
                    self.toggle_expand();
                } else {
                    self.palette = Some(CommandPalette::new(command_palette::Context::Tree));
                }
            }
            Event::Enter => self.confirm(),
            Event::Key('?') => self.show_help = true,
            Event::Key('G') => {
                self.pending_g = None;
                self.jump_end();
            }
            Event::Key('g') => {
                if self
                    .pending_g
                    .map(|t| t.elapsed() <= G_CHORD_WINDOW)
                    .unwrap_or(false)
                {
                    self.jump_home();
                    self.pending_g = None;
                } else {
                    self.pending_g = Some(Instant::now());
                }
            }
            Event::Key('y') => self.copy_session_id(),
            Event::Key('Y') => self.copy_project_path(),
            Event::Key('o') => self.open_in_editor(),
            Event::Key('v') => self.open_viewer(),
            _ => {
                self.pending_g = None;
            }
        }
    }

    /// Open the conversation viewer for the session under the cursor.
    fn open_viewer(&mut self) {
        let Some(session) = self.selected_session().cloned() else {
            return;
        };
        self.viewer = Some(ViewerState::open(&session));
    }

    /// Forward an event into the open viewer and react to its reply.
    fn handle_viewer_event(&mut self, ev: Event) {
        let Some(viewer) = self.viewer.as_mut() else {
            return;
        };
        match viewer.handle_event(ev) {
            ViewerAction::None => {}
            ViewerAction::Close => self.viewer = None,
            ViewerAction::Toast(message, kind) => {
                let local_kind = match kind {
                    ViewerToastKind::Info => ToastKind::Info,
                    ViewerToastKind::Success => ToastKind::Success,
                    ViewerToastKind::Error => ToastKind::Error,
                };
                self.toast = Some(Toast::new(message, local_kind));
            }
        }
    }

    /// Route an event into the command palette, then run any action it
    /// yielded back against `self`. The palette closes itself on Enter
    /// or Escape — we just check whether it produced an action after.
    fn handle_palette_event(&mut self, ev: Event) {
        let Some(palette) = self.palette.as_mut() else {
            return;
        };
        let outcome = palette.handle_event(ev);
        match outcome {
            command_palette::Outcome::Continue => {}
            command_palette::Outcome::Close => {
                self.palette = None;
            }
            command_palette::Outcome::Execute(action_id) => {
                self.palette = None;
                self.execute_palette_action(action_id);
            }
        }
    }

    /// Map palette action ids to state mutations. Limited to actions
    /// that make sense on the tree screen.
    fn execute_palette_action(&mut self, id: &'static str) {
        match id {
            "resume" => self.confirm(),
            "copy_session_id" => self.copy_session_id(),
            "copy_project_path" => self.copy_project_path(),
            "open_editor" => self.open_in_editor(),
            "toggle_expand" => self.toggle_expand(),
            "expand_all" => {
                self.collapsed.clear();
                self.rebuild();
            }
            "collapse_all" => {
                self.collapsed = collapsible_fork_root_ids(&self.sessions_by_project);
                self.rebuild();
            }
            "help" => self.show_help = true,
            "quit" => self.should_quit = true,
            _ => {}
        }
    }

    /// Session under cursor, if any.
    fn selected_session(&self) -> Option<&Session> {
        self.nodes.get(self.cursor).and_then(|n| match &n.kind {
            NodeKind::SessionRow { session } => Some(session),
            _ => None,
        })
    }

    fn copy_session_id(&mut self) {
        let Some(s) = self.selected_session() else {
            return;
        };
        let short = s.id.chars().take(8).collect::<String>();
        let id = s.id.clone();
        match clipboard::copy(id) {
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
        let Some(s) = self.selected_session() else {
            return;
        };
        let path = s.project_dir.clone();
        let display = path.display().to_string();
        match clipboard::copy(display.clone()) {
            Ok(()) => {
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

    fn open_in_editor(&mut self) {
        let Some(s) = self.selected_session() else {
            return;
        };
        let path = s.project_dir.clone();
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
}

/// Draw one frame: tree body + footer hint line. Takes `&mut state` because
/// the conversation viewer caches flattened lines on its state as it
/// renders, and that cache is invalidated by width / search changes.
fn render_screen(f: &mut Frame<'_>, state: &mut TreeState, theme: &Theme) {
    let area = f.area();

    // Viewer takes over the whole frame when open — short-circuit to avoid
    // drawing the tree behind it.
    if let Some(viewer) = state.viewer.as_mut() {
        viewer_ui::render(f, area, viewer, theme);
        if let Some(toast) = &state.toast {
            render_toast(f, area, toast, theme);
        }
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);
    render_tree(f, rows[0], &state.nodes, state.cursor, theme);
    render_footer(f, rows[1], theme);

    if let Some(toast) = &state.toast {
        render_toast(f, area, toast, theme);
    }
    if state.show_help {
        let content = help_overlay::help_for(HelpScreen::Tree);
        help_overlay::render(f, area, content, theme);
    }
    if let Some(palette) = &state.palette {
        command_palette::render(f, area, palette, theme);
    }
}

/// Render a centred toast just above the bottom of the screen. Matches the
/// style of the main picker's toast so the UI feels consistent between
/// subcommand screens.
fn render_toast(f: &mut Frame<'_>, area: Rect, toast: &Toast, theme: &Theme) {
    let w = 52u16.min(area.width.saturating_sub(4));
    let h = 3u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(h))
        .saturating_sub(4);
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    f.render_widget(Clear, rect);

    let (accent, label) = match toast.kind {
        ToastKind::Info => (theme.mauve, "info"),
        ToastKind::Success => (theme.green, "done"),
        ToastKind::Error => (theme.red, "error"),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                label,
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));
    let p = Paragraph::new(Line::from(Span::styled(
        format!(" {} ", toast.message),
        Style::default().fg(theme.text),
    )))
    .block(block)
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(p, rect);
}

/// Two-line footer: keys first, legend second.
fn render_footer(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let sep_style = theme.dim();
    let keys = Line::from(vec![
        Span::raw("  "),
        Span::styled("↑↓", theme.key_hint()),
        Span::raw(" "),
        Span::styled("navigate", theme.key_desc()),
        Span::styled("  ·  ", sep_style),
        Span::styled("Enter", theme.key_hint()),
        Span::raw(" "),
        Span::styled("resume", theme.key_desc()),
        Span::styled("  ·  ", sep_style),
        Span::styled("?", theme.key_hint()),
        Span::raw(" "),
        Span::styled("help", theme.key_desc()),
        Span::styled("  ·  ", sep_style),
        Span::styled("q", theme.key_hint()),
        Span::raw(" "),
        Span::styled("quit", theme.key_desc()),
    ]);
    let legend = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "●",
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("named", theme.muted()),
        Span::styled("   ", sep_style),
        Span::styled(
            "◆",
            Style::default()
                .fg(theme.peach)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("forked", theme.muted()),
        Span::styled("   ", sep_style),
        Span::styled(
            "○",
            Style::default()
                .fg(theme.overlay0)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("unnamed", theme.muted()),
    ]);
    let p = Paragraph::new(vec![keys, legend]);
    f.render_widget(p, area);
}

// ── Terminal lifecycle ─────────────────────────────────────────────────

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    terminal.show_cursor()?;
    Ok(())
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
    use crate::data::pricing::TokenCounts;
    use crate::data::session::SessionKind;
    use crate::ui::tree::build_tree_with_collapsed;

    fn mk_session(id: &str) -> Session {
        Session {
            id: id.to_string(),
            project_dir: PathBuf::from("/tmp"),
            name: Some(id.to_string()),
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
            turn_durations: Vec::new(),
        }
    }

    fn mk_fork(id: &str, parent: &str) -> Session {
        let mut s = mk_session(id);
        s.forked_from = Some(parent.to_string());
        s.is_fork = true;
        s
    }

    fn mk_project(name: &str) -> Project {
        Project {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}")),
            encoded_dir: format!("-tmp-{name}"),
            session_count: 0,
            last_activity: None,
            git_branch: None,
        }
    }

    /// Convenience factory for tests: projects, sessions, empty collapsed
    /// set — matches the historical "expand everything" behaviour.
    fn mk_state(projects: Vec<Project>, sessions_by_project: Vec<Vec<Session>>) -> TreeState {
        let collapsed = std::collections::HashSet::new();
        let nodes = build_tree_with_collapsed(&projects, &sessions_by_project, &collapsed);
        TreeState::new(nodes, projects, sessions_by_project, collapsed)
    }

    #[test]
    fn cursor_lands_on_first_session_not_header() {
        let state = mk_state(vec![mk_project("p")], vec![vec![mk_session("s1")]]);
        // nodes[0] = header, nodes[1] = session row.
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn arrow_keys_skip_headers_and_wrap() {
        let mut state = mk_state(
            vec![mk_project("p")],
            vec![vec![mk_session("s1"), mk_session("s2"), mk_session("s3")]],
        );

        state.handle_event(Event::Down);
        // The header is at index 0, sessions at 1, 2, 3.
        assert_eq!(state.cursor, 2);
        state.handle_event(Event::Down);
        assert_eq!(state.cursor, 3);
        // Wrap past the end back to the first selectable row, which is 1.
        state.handle_event(Event::Down);
        assert_eq!(state.cursor, 1);

        // Up from 1 should wrap to the last session (3).
        state.handle_event(Event::Up);
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn enter_records_selection() {
        let mut state = mk_state(vec![mk_project("p")], vec![vec![mk_session("abc123")]]);
        state.handle_event(Event::Enter);
        assert!(state.should_quit);
        let sel = state.selection.expect("selection set");
        assert_eq!(sel.session_id, "abc123");
    }

    #[test]
    fn quit_keys_set_should_quit() {
        for ev in [
            Event::Escape,
            Event::Key('q'),
            Event::Ctrl('c'),
            Event::Quit,
        ] {
            let mut state = mk_state(vec![mk_project("p")], vec![vec![mk_session("s")]]);
            state.handle_event(ev);
            assert!(state.should_quit, "{ev:?} should quit");
            assert!(state.selection.is_none(), "{ev:?} should not select");
        }
    }

    #[test]
    fn j_and_k_navigate() {
        let mut state = mk_state(
            vec![mk_project("p")],
            vec![vec![mk_session("s1"), mk_session("s2")]],
        );
        let start = state.cursor;
        state.handle_event(Event::Key('j'));
        assert_ne!(state.cursor, start);
        state.handle_event(Event::Key('k'));
        assert_eq!(state.cursor, start);
    }

    #[test]
    fn right_arrow_expands_collapsed_fork_root() {
        // Start with the root collapsed — only header + root are visible.
        let projects = vec![mk_project("p")];
        let sessions = vec![vec![
            mk_session("root"),
            mk_fork("fork1", "root"),
            mk_fork("fork2", "root"),
        ]];
        let mut collapsed = std::collections::HashSet::new();
        collapsed.insert("root".to_string());
        let nodes = build_tree_with_collapsed(&projects, &sessions, &collapsed);
        let mut state = TreeState::new(nodes, projects, sessions, collapsed);

        // Cursor lands on the root (nodes[1]).
        assert_eq!(state.cursor, 1);
        // With the root collapsed, we have: header + root = 2 nodes.
        assert_eq!(state.nodes.len(), 2);

        state.handle_event(Event::Right);
        // Now expanded: header + root + fork1 + fork2 = 4 nodes.
        assert_eq!(state.nodes.len(), 4);
        // Cursor still on root.
        assert_eq!(state.nodes[state.cursor].session_id(), Some("root"));
    }

    #[test]
    fn left_arrow_collapses_expanded_fork_root() {
        let projects = vec![mk_project("p")];
        let sessions = vec![vec![mk_session("root"), mk_fork("child", "root")]];
        // Start fully expanded.
        let mut state = mk_state(projects, sessions);
        // Park cursor on the root row.
        let root_idx = state
            .nodes
            .iter()
            .position(|n| n.session_id() == Some("root"))
            .expect("root present");
        state.cursor = root_idx;
        let before_len = state.nodes.len();

        state.handle_event(Event::Left);
        assert!(
            state.nodes.len() < before_len,
            "left arrow on expanded root must collapse it"
        );
        assert!(state.collapsed.contains("root"));
    }

    #[test]
    fn left_arrow_on_child_jumps_to_parent() {
        let projects = vec![mk_project("p")];
        let sessions = vec![vec![mk_session("root"), mk_fork("child", "root")]];
        let mut state = mk_state(projects, sessions);
        let child_idx = state
            .nodes
            .iter()
            .position(|n| n.session_id() == Some("child"))
            .expect("child visible when expanded");
        state.cursor = child_idx;

        state.handle_event(Event::Left);
        // Cursor should have moved to parent row.
        assert_eq!(state.nodes[state.cursor].session_id(), Some("root"));
    }

    #[test]
    fn space_toggles_expand() {
        let projects = vec![mk_project("p")];
        let sessions = vec![vec![mk_session("root"), mk_fork("fork1", "root")]];
        let mut collapsed = std::collections::HashSet::new();
        collapsed.insert("root".to_string());
        let nodes = build_tree_with_collapsed(&projects, &sessions, &collapsed);
        let mut state = TreeState::new(nodes, projects, sessions, collapsed);

        let before = state.nodes.len();
        state.handle_event(Event::Key(' '));
        assert!(state.nodes.len() > before, "space expands collapsed root");
        state.handle_event(Event::Key(' '));
        assert_eq!(state.nodes.len(), before, "space again collapses");
    }

    #[test]
    fn space_on_leaf_opens_palette() {
        // Single root with no forks — space should fall through to
        // opening the command palette rather than a no-op.
        let mut state = mk_state(vec![mk_project("p")], vec![vec![mk_session("lonely")]]);
        assert!(state.palette.is_none());
        state.handle_event(Event::Key(' '));
        assert!(state.palette.is_some());
    }

    #[test]
    fn palette_execute_quit() {
        let mut state = mk_state(vec![mk_project("p")], vec![vec![mk_session("s")]]);
        state.handle_event(Event::Key(' '));
        assert!(state.palette.is_some());
        // Dispatch directly through the palette handler path.
        state.execute_palette_action("quit");
        assert!(state.should_quit);
    }
}
