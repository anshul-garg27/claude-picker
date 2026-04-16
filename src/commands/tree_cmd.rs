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

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use crate::commands::pick::load_sessions_for;
use crate::data::{project, Project, Session};
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::tree::{build_tree, render as render_tree, NodeKind, TreeNode};

/// Entry point for `claude-picker tree`.
pub fn run() -> anyhow::Result<()> {
    let (projects, sessions_by_project) = load_data()?;
    let nodes = build_tree(&projects, &sessions_by_project);

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
    let mut state = TreeState::new(nodes);

    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let result: anyhow::Result<Option<Selection>> = (|| {
        while !state.should_quit {
            terminal.draw(|f| render_screen(f, &state, &theme))?;
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
    nodes: Vec<TreeNode>,
    /// Index into `nodes`. Constrained to a selectable row.
    cursor: usize,
    should_quit: bool,
    selection: Option<Selection>,
}

impl TreeState {
    fn new(nodes: Vec<TreeNode>) -> Self {
        let mut s = Self {
            nodes,
            cursor: 0,
            should_quit: false,
            selection: None,
        };
        s.cursor = s.first_selectable().unwrap_or(0);
        s
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
        match ev {
            Event::Quit | Event::Escape | Event::Ctrl('c') => self.should_quit = true,
            Event::Key('q') => self.should_quit = true,
            Event::Up | Event::Key('k') => self.step(-1),
            Event::Down | Event::Key('j') => self.step(1),
            Event::PageUp => self.step(-10),
            Event::PageDown => self.step(10),
            Event::Home => self.jump_home(),
            Event::End => self.jump_end(),
            Event::Enter => self.confirm(),
            _ => {}
        }
    }
}

/// Draw one frame: tree body + footer hint line.
fn render_screen(f: &mut Frame<'_>, state: &TreeState, theme: &Theme) {
    let area = f.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);
    render_tree(f, rows[0], &state.nodes, state.cursor, theme);
    render_footer(f, rows[1], theme);
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

    fn mk_session(id: &str) -> Session {
        Session {
            id: id.to_string(),
            project_dir: PathBuf::from("/tmp"),
            name: Some(id.to_string()),
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

    #[test]
    fn cursor_lands_on_first_session_not_header() {
        let nodes = build_tree(&[mk_project("p")], &[vec![mk_session("s1")]]);
        let state = TreeState::new(nodes);
        // nodes[0] = header, nodes[1] = session row.
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn arrow_keys_skip_headers_and_wrap() {
        let nodes = build_tree(
            &[mk_project("p")],
            &[vec![mk_session("s1"), mk_session("s2"), mk_session("s3")]],
        );
        let mut state = TreeState::new(nodes);

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
        let nodes = build_tree(&[mk_project("p")], &[vec![mk_session("abc123")]]);
        let mut state = TreeState::new(nodes);
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
            let nodes = build_tree(&[mk_project("p")], &[vec![mk_session("s")]]);
            let mut state = TreeState::new(nodes);
            state.handle_event(ev);
            assert!(state.should_quit, "{ev:?} should quit");
            assert!(state.selection.is_none(), "{ev:?} should not select");
        }
    }

    #[test]
    fn j_and_k_navigate() {
        let nodes = build_tree(
            &[mk_project("p")],
            &[vec![mk_session("s1"), mk_session("s2")]],
        );
        let mut state = TreeState::new(nodes);
        let start = state.cursor;
        state.handle_event(Event::Key('j'));
        assert_ne!(state.cursor, start);
        state.handle_event(Event::Key('k'));
        assert_eq!(state.cursor, start);
    }
}
