//! Full-screen conversation viewer — the `v` keybinding.
//!
//! Renders a scrollable transcript of an entire session: every user /
//! assistant message, every tool_use / tool_result block, every
//! extended-thinking block. Takes over the whole frame (not a preview pane)
//! so the reader has full width to breathe.
//!
//! State machine lives on [`ViewerState`] and is driven by the parent
//! screen's event loop (main picker, tree, or search). Scrolling, in-viewer
//! search, tool-block cycling, and message copy are all local — the parent
//! just forwards key events until the user presses `q` / `Esc`.
//!
//! Rendering strategy: we flatten the transcript into a `Vec<Line>` once
//! per (width, search-query, transcript) change and cache the result.
//! Scrolling is then an O(1) offset slice. Triple-backtick fenced code
//! blocks get a `surface1` background; everything else is left plain so
//! the terminal's default monospace handles layout.

use std::time::Instant;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::data::transcript::{
    jsonl_path_for_session, load_transcript, ContentItem, Role, TranscriptMessage,
};
use crate::data::Session;
use crate::events::Event;
use crate::theme::Theme;

/// What the viewer wants the parent to do after handling an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerAction {
    /// Stay in the viewer — the event was handled locally.
    None,
    /// Exit the viewer and return to the parent screen.
    Close,
    /// Show a transient status message ("copied message", etc.).
    Toast(String, ToastKind),
}

/// Toast flavour the parent should use. Mirrors [`crate::app::ToastKind`]
/// but is local so the viewer doesn't depend on the picker state machine —
/// subcommand screens translate as they forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

/// Which part of the viewer currently receives keystrokes. When the search
/// bar is open we route typing into the query buffer; otherwise plain keys
/// drive scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    SearchTyping,
}

/// All the transient state for an open viewer instance.
pub struct ViewerState {
    /// Display label for the title bar (e.g. "auth-refactor").
    pub title: String,
    /// Session id — needed for the "copy id" flow if the parent wants it.
    #[allow(dead_code)]
    pub session_id: String,
    /// Message count rolled up from the session (cheaper than len(messages)
    /// post-filter and matches the picker's displayed number).
    pub msg_count_label: String,
    /// Token total, formatted — "18.2k" / "342" etc.
    pub tokens_label: String,
    /// Cost rolled up from the session, formatted.
    pub cost_label: String,

    /// Parsed transcript — one entry per user/assistant message. `None` if
    /// the load failed (file missing, parse error) so render can show a
    /// placeholder.
    pub messages: Vec<TranscriptMessage>,
    /// Error surfaced when loading failed, displayed in the viewer body so
    /// users see *why* it's empty.
    pub load_error: Option<String>,

    /// Scroll offset in rendered lines.
    pub scroll: usize,
    /// Width used to compute `rendered_lines`. Re-rendered when the frame
    /// resizes.
    pub cached_width: u16,
    /// Flattened lines for the current width + search. Rebuilt lazily.
    pub rendered_lines: Vec<Line<'static>>,
    /// Indices into `rendered_lines` that are message boundaries (for
    /// copy-message flow: the line at offset N belongs to whichever
    /// message's range contains N).
    pub message_line_ranges: Vec<(usize, usize)>,
    /// Indices into `rendered_lines` that begin a tool_use block. Used by
    /// `[`/`]` to jump between tool calls.
    pub tool_use_line_indices: Vec<usize>,

    /// In-viewer search state.
    pub search_query: String,
    /// True while `/` search bar is open.
    pub search_open: bool,
    pub input_mode: InputMode,
    /// Line indices that contain matches for the current query (sorted
    /// ascending).
    pub match_line_indices: Vec<usize>,
    /// Which match is currently highlighted (index into
    /// `match_line_indices`). 0 when no matches.
    pub active_match: usize,

    /// Window where `gg` collapses to top.
    pending_g: Option<Instant>,
}

const G_CHORD_WINDOW: std::time::Duration = std::time::Duration::from_millis(500);

/// Minimum width we render at before switching to a "too narrow" hint. The
/// viewer renders cleanly down to 60 cols; below that labels start to
/// collide with tool-block borders.
const MIN_VIEWER_WIDTH: u16 = 60;

impl ViewerState {
    /// Construct a new viewer for a session. Performs the JSONL parse
    /// synchronously — a 1000-message session is typically < 20 ms on an M1
    /// and blocking the frame is preferable to showing a loading spinner
    /// that flashes and disappears.
    pub fn open(session: &Session) -> Self {
        let tokens_total = session.tokens.total();
        let tokens_label = if tokens_total >= 1_000 {
            format!("{:.1}k tok", tokens_total as f64 / 1000.0)
        } else {
            format!("{tokens_total} tok")
        };
        let cost_label = if session.total_cost_usd < 0.01 {
            "<$0.01".to_string()
        } else {
            format!("${:.2}", session.total_cost_usd)
        };
        Self::open_with(
            &session.id,
            session.display_label(),
            format!("{} msgs", session.message_count),
            tokens_label,
            cost_label,
        )
    }

    /// Same as [`Self::open`] but takes a bare session id + labels. Useful
    /// from screens (search) that don't carry a full [`Session`] for the
    /// row they want to view.
    pub fn open_with(
        session_id: &str,
        title: impl Into<String>,
        msg_count_label: impl Into<String>,
        tokens_label: impl Into<String>,
        cost_label: impl Into<String>,
    ) -> Self {
        let mut state = Self {
            title: title.into(),
            session_id: session_id.to_string(),
            msg_count_label: msg_count_label.into(),
            tokens_label: tokens_label.into(),
            cost_label: cost_label.into(),
            messages: Vec::new(),
            load_error: None,
            scroll: 0,
            cached_width: 0,
            rendered_lines: Vec::new(),
            message_line_ranges: Vec::new(),
            tool_use_line_indices: Vec::new(),
            search_query: String::new(),
            search_open: false,
            input_mode: InputMode::Normal,
            match_line_indices: Vec::new(),
            active_match: 0,
            pending_g: None,
        };

        match jsonl_path_for_session(session_id) {
            Some(path) => match load_transcript(&path) {
                Ok(messages) => state.messages = messages,
                Err(e) => state.load_error = Some(format!("parse error: {e}")),
            },
            None => state.load_error = Some("session file not found".to_string()),
        }
        state
    }

    /// Process a single event, returning what the parent should do.
    pub fn handle_event(&mut self, ev: Event) -> ViewerAction {
        // Expire `gg` chord before anything else.
        if let Some(t) = self.pending_g {
            if t.elapsed() > G_CHORD_WINDOW {
                self.pending_g = None;
            }
        }

        // Search-typing captures most text keys so slash + alpha land in
        // the query rather than firing viewer shortcuts.
        if self.input_mode == InputMode::SearchTyping {
            return self.handle_search_typing(ev);
        }

        match ev {
            Event::Key('q') | Event::Escape | Event::Ctrl('c') | Event::Quit => {
                if self.search_open {
                    self.close_search();
                    ViewerAction::None
                } else {
                    ViewerAction::Close
                }
            }
            Event::Up | Event::Key('k') => {
                self.pending_g = None;
                self.scroll_by(-1);
                ViewerAction::None
            }
            Event::Down | Event::Key('j') => {
                self.pending_g = None;
                self.scroll_by(1);
                ViewerAction::None
            }
            Event::Key(' ') | Event::PageDown => {
                self.pending_g = None;
                self.scroll_by(self.visible_height() as i32);
                ViewerAction::None
            }
            Event::Key('b') | Event::PageUp => {
                self.pending_g = None;
                self.scroll_by(-(self.visible_height() as i32));
                ViewerAction::None
            }
            Event::Ctrl('d') => {
                self.scroll_by(self.visible_height() as i32 / 2);
                ViewerAction::None
            }
            Event::Ctrl('u') => {
                self.scroll_by(-(self.visible_height() as i32 / 2));
                ViewerAction::None
            }
            Event::Key('G') => {
                self.pending_g = None;
                self.scroll = self.max_scroll();
                ViewerAction::None
            }
            Event::Key('g') => {
                if self
                    .pending_g
                    .map(|t| t.elapsed() <= G_CHORD_WINDOW)
                    .unwrap_or(false)
                {
                    self.scroll = 0;
                    self.pending_g = None;
                } else {
                    self.pending_g = Some(Instant::now());
                }
                ViewerAction::None
            }
            Event::Home => {
                self.scroll = 0;
                ViewerAction::None
            }
            Event::End => {
                self.scroll = self.max_scroll();
                ViewerAction::None
            }
            Event::Key('/') => {
                self.open_search();
                ViewerAction::None
            }
            Event::Key('n') => {
                self.next_match(1);
                ViewerAction::None
            }
            Event::Key('N') => {
                self.next_match(-1);
                ViewerAction::None
            }
            Event::Key(']') => {
                self.jump_tool(1);
                ViewerAction::None
            }
            Event::Key('[') => {
                self.jump_tool(-1);
                ViewerAction::None
            }
            Event::Key('y') => self.copy_centered_message(),
            Event::Resize(_, _) => ViewerAction::None,
            _ => {
                self.pending_g = None;
                ViewerAction::None
            }
        }
    }

    fn handle_search_typing(&mut self, ev: Event) -> ViewerAction {
        match ev {
            Event::Escape => {
                self.close_search();
                ViewerAction::None
            }
            Event::Enter => {
                // Commit query: switch back to Normal mode but keep the
                // search bar visible so `n`/`N` can cycle.
                self.input_mode = InputMode::Normal;
                if !self.match_line_indices.is_empty() {
                    let idx = self.match_line_indices[self.active_match];
                    self.scroll_to(idx);
                }
                ViewerAction::None
            }
            Event::Backspace => {
                self.search_query.pop();
                self.invalidate_cache();
                ViewerAction::None
            }
            Event::Key(c) if is_search_char(c) => {
                self.search_query.push(c);
                self.invalidate_cache();
                ViewerAction::None
            }
            _ => ViewerAction::None,
        }
    }

    fn open_search(&mut self) {
        self.search_open = true;
        self.input_mode = InputMode::SearchTyping;
        self.search_query.clear();
        self.active_match = 0;
        self.invalidate_cache();
    }

    fn close_search(&mut self) {
        self.search_open = false;
        self.input_mode = InputMode::Normal;
        self.search_query.clear();
        self.match_line_indices.clear();
        self.active_match = 0;
        self.invalidate_cache();
    }

    fn invalidate_cache(&mut self) {
        // Setting cached_width to 0 forces the next render to rebuild.
        self.cached_width = 0;
    }

    fn scroll_by(&mut self, delta: i32) {
        let max = self.max_scroll() as i32;
        let next = (self.scroll as i32).saturating_add(delta).clamp(0, max);
        self.scroll = next as usize;
    }

    fn scroll_to(&mut self, line_idx: usize) {
        // Center the target line in the viewport when possible.
        let h = self.visible_height() as usize;
        let offset = line_idx.saturating_sub(h / 3);
        self.scroll = offset.min(self.max_scroll());
    }

    fn max_scroll(&self) -> usize {
        let total = self.rendered_lines.len();
        let h = self.visible_height() as usize;
        total.saturating_sub(h)
    }

    fn visible_height(&self) -> u16 {
        // Conservative default before the first frame; the real height is
        // recorded on render. 20 is "something reasonable to scroll by if
        // we get a keystroke before the first draw".
        20
    }

    fn next_match(&mut self, delta: i32) {
        if self.match_line_indices.is_empty() {
            return;
        }
        let len = self.match_line_indices.len() as i32;
        let next = ((self.active_match as i32) + delta).rem_euclid(len);
        self.active_match = next as usize;
        let idx = self.match_line_indices[self.active_match];
        self.scroll_to(idx);
    }

    fn jump_tool(&mut self, dir: i32) {
        if self.tool_use_line_indices.is_empty() {
            return;
        }
        let current_scroll = self.scroll;
        if dir > 0 {
            for &idx in &self.tool_use_line_indices {
                if idx > current_scroll {
                    self.scroll_to(idx);
                    return;
                }
            }
            // Wrap to the first tool block.
            self.scroll_to(self.tool_use_line_indices[0]);
        } else {
            for &idx in self.tool_use_line_indices.iter().rev() {
                if idx < current_scroll {
                    self.scroll_to(idx);
                    return;
                }
            }
            // Wrap to the last tool block.
            self.scroll_to(*self.tool_use_line_indices.last().unwrap());
        }
    }

    fn copy_centered_message(&mut self) -> ViewerAction {
        if self.messages.is_empty() {
            return ViewerAction::Toast("no message to copy".to_string(), ToastKind::Info);
        }
        // The "centered" message is whichever one contains the middle
        // visible line.
        let target_line = self.scroll + (self.visible_height() as usize) / 2;
        let mut target_msg = 0;
        for (i, (start, end)) in self.message_line_ranges.iter().enumerate() {
            if target_line >= *start && target_line < *end {
                target_msg = i;
                break;
            }
            if *start > target_line {
                break;
            }
            target_msg = i;
        }
        let Some(msg) = self.messages.get(target_msg) else {
            return ViewerAction::Toast("no message to copy".to_string(), ToastKind::Info);
        };
        let plain = msg.as_plain_text();
        match crate::data::clipboard::copy(plain) {
            Ok(()) => ViewerAction::Toast(
                "copied message to clipboard".to_string(),
                ToastKind::Success,
            ),
            Err(e) => ViewerAction::Toast(format!("clipboard unavailable: {e}"), ToastKind::Error),
        }
    }
}

fn is_search_char(c: char) -> bool {
    !c.is_control() && c != '/'
}

/// Render the viewer across the entire `area`. Recomputes the flattened
/// line cache if the width has changed.
pub fn render(f: &mut Frame<'_>, area: Rect, state: &mut ViewerState, theme: &Theme) {
    // Clear so we truly take over the whole screen — the viewer is
    // modal-on-top-of-picker and the underlying pane's border would
    // otherwise bleed through at the edges.
    f.render_widget(Clear, area);

    if area.width < MIN_VIEWER_WIDTH {
        render_too_narrow(f, area, theme);
        return;
    }

    // Outer bordered block + title.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(title_line(state, theme));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split inner vertically: body + footer (+ search bar if open).
    let constraints: Vec<Constraint> = if state.search_open {
        vec![
            Constraint::Min(3),    // body
            Constraint::Length(1), // search bar
            Constraint::Length(1), // footer
        ]
    } else {
        vec![
            Constraint::Min(3),    // body
            Constraint::Length(1), // footer
        ]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let body_area = chunks[0];
    render_body(f, body_area, state, theme);

    if state.search_open {
        render_search_bar(f, chunks[1], state, theme);
        render_footer_hint(f, chunks[2], state, theme);
    } else {
        render_footer_hint(f, chunks[1], state, theme);
    }
}

fn title_line<'a>(state: &'a ViewerState, theme: &'a Theme) -> Line<'a> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            state.title.clone(),
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(state.msg_count_label.clone(), theme.muted()),
        Span::styled(" · ", theme.dim()),
        Span::styled(state.tokens_label.clone(), theme.muted()),
        Span::styled(" · ", theme.dim()),
        Span::styled(
            state.cost_label.clone(),
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
}

fn render_body(f: &mut Frame<'_>, area: Rect, state: &mut ViewerState, theme: &Theme) {
    // Show load error as a centered placeholder.
    if let Some(err) = &state.load_error {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(err.clone(), theme.muted()),
        ])
        .alignment(Alignment::Center);
        f.render_widget(p, area);
        return;
    }

    if state.messages.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("no messages to display", theme.muted()),
        ])
        .alignment(Alignment::Center);
        f.render_widget(p, area);
        return;
    }

    // Rebuild cache if width changed or search invalidated it.
    if state.cached_width != area.width {
        rebuild_rendered_lines(state, theme, area.width);
        state.cached_width = area.width;
    }

    // Clamp scroll to the valid range now that we know the full line count.
    let h = area.height as usize;
    let max_scroll = state.rendered_lines.len().saturating_sub(h);
    if state.scroll > max_scroll {
        state.scroll = max_scroll;
    }

    let visible: Vec<Line<'static>> = state
        .rendered_lines
        .iter()
        .skip(state.scroll)
        .take(h)
        .cloned()
        .collect();
    let p = Paragraph::new(visible);
    f.render_widget(p, area);
}

fn render_search_bar(f: &mut Frame<'_>, area: Rect, state: &ViewerState, theme: &Theme) {
    let match_count = state.match_line_indices.len();
    let summary = if match_count == 0 {
        " no matches ".to_string()
    } else {
        format!(" {}/{} ", state.active_match + 1, match_count)
    };

    let active_cursor = state.input_mode == InputMode::SearchTyping;
    let mut spans = vec![
        Span::styled(" / ", theme.key_hint()),
        Span::styled(state.search_query.clone(), theme.body()),
    ];
    if active_cursor {
        spans.push(Span::styled(
            " ",
            Style::default().bg(theme.mauve).fg(theme.crust),
        ));
    }
    spans.push(Span::styled("   ", theme.dim()));
    spans.push(Span::styled(summary, theme.muted()));

    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}

fn render_footer_hint(f: &mut Frame<'_>, area: Rect, state: &ViewerState, theme: &Theme) {
    let dim = theme.dim();
    let mut spans = vec![
        Span::raw("  "),
        Span::styled("↑↓/jk", theme.key_hint()),
        Span::raw(" "),
        Span::styled("scroll", theme.key_desc()),
        Span::styled("  ·  ", dim),
        Span::styled("Space", theme.key_hint()),
        Span::raw(" "),
        Span::styled("page", theme.key_desc()),
        Span::styled("  ·  ", dim),
        Span::styled("gg/G", theme.key_hint()),
        Span::raw(" "),
        Span::styled("top/end", theme.key_desc()),
        Span::styled("  ·  ", dim),
        Span::styled("/", theme.key_hint()),
        Span::raw(" "),
        Span::styled("find", theme.key_desc()),
        Span::styled("  ·  ", dim),
        Span::styled("[ ]", theme.key_hint()),
        Span::raw(" "),
        Span::styled("tool", theme.key_desc()),
        Span::styled("  ·  ", dim),
        Span::styled("y", theme.key_hint()),
        Span::raw(" "),
        Span::styled("copy", theme.key_desc()),
        Span::styled("  ·  ", dim),
        Span::styled("q", theme.key_hint()),
        Span::raw(" "),
        Span::styled("back", theme.key_desc()),
    ];
    // Show scroll position as a percentage on the right when the
    // transcript is longer than the viewport.
    let total = state.rendered_lines.len();
    if total > area.height as usize {
        let pct = if state.max_scroll() == 0 {
            100
        } else {
            (state.scroll * 100 / state.max_scroll().max(1)).min(100)
        };
        spans.push(Span::styled("   ", dim));
        spans.push(Span::styled(format!("{pct}%"), theme.muted()));
    }
    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}

fn render_too_narrow(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let p = Paragraph::new(vec![
        Line::raw(""),
        Line::styled(
            "Terminal too narrow for conversation view.",
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            format!("Resize to at least {MIN_VIEWER_WIDTH} cols and retry (q to exit)."),
            theme.muted(),
        ),
    ])
    .alignment(Alignment::Center);
    f.render_widget(p, area);
}

// ── Line-flattening: transcript → Vec<Line> ────────────────────────────────

/// Rebuild [`ViewerState::rendered_lines`] by flattening every message into
/// its display rows at the current width. Also recomputes the
/// message-boundary map and the tool-block jump index, plus — if a search
/// query is live — the list of lines that contain a match.
fn rebuild_rendered_lines(state: &mut ViewerState, theme: &Theme, width: u16) {
    let content_width = width.saturating_sub(2) as usize; // panel padding
    let wrap_width = content_width.saturating_sub(4).max(40);

    state.rendered_lines.clear();
    state.message_line_ranges.clear();
    state.tool_use_line_indices.clear();
    state.match_line_indices.clear();

    let query_lower = state.search_query.to_lowercase();
    let query_active = !query_lower.is_empty();

    for msg in &state.messages {
        let start = state.rendered_lines.len();
        render_message(
            msg,
            theme,
            wrap_width,
            content_width,
            &mut state.rendered_lines,
            &mut state.tool_use_line_indices,
        );
        let end = state.rendered_lines.len();
        // Blank spacer between messages.
        state.rendered_lines.push(Line::raw(""));
        state.message_line_ranges.push((start, end));
    }

    // If we have a search query, highlight matches in yellow and build the
    // match-line index.
    if query_active {
        let active_target = state.match_line_indices.len(); // placeholder
        let _ = active_target;
        for (line_idx, line) in state.rendered_lines.iter_mut().enumerate() {
            let plain = line_plain_text(line);
            if plain.to_lowercase().contains(&query_lower) {
                state.match_line_indices.push(line_idx);
                // Dim-yellow highlight on every match; the "active" one
                // gets a brighter style applied after this pass.
                highlight_line(line, &query_lower, theme.yellow, false);
            }
        }
        // Cap active_match to a valid index.
        if state.active_match >= state.match_line_indices.len() {
            state.active_match = 0;
        }
        if let Some(&active_line) = state.match_line_indices.get(state.active_match) {
            if let Some(line) = state.rendered_lines.get_mut(active_line) {
                highlight_line(line, &query_lower, theme.yellow, true);
            }
        }
    }
}

fn line_plain_text(line: &Line<'_>) -> String {
    let mut s = String::new();
    for span in &line.spans {
        s.push_str(&span.content);
    }
    s
}

/// Tint every span that contains the needle with a yellow background. The
/// `bright` flag controls whether we use a full reverse-video style (the
/// active match) or a muted yellow dimmer (secondary matches).
fn highlight_line(line: &mut Line<'static>, needle_lower: &str, yellow: Color, bright: bool) {
    if needle_lower.is_empty() {
        return;
    }
    let match_style = if bright {
        Style::default()
            .bg(yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(yellow).add_modifier(Modifier::BOLD)
    };
    for span in &mut line.spans {
        if span.content.to_lowercase().contains(needle_lower) {
            span.style = match_style;
        }
    }
}

/// Flatten a single message into `out`. Pushes all of its display rows,
/// records tool-use block entry lines into `tool_use_indices`.
fn render_message(
    msg: &TranscriptMessage,
    theme: &Theme,
    wrap_width: usize,
    content_width: usize,
    out: &mut Vec<Line<'static>>,
    tool_use_indices: &mut Vec<usize>,
) {
    // Role label line.
    let (label, label_style) = match msg.role {
        Role::User => (
            "you",
            Style::default().fg(theme.blue).add_modifier(Modifier::BOLD),
        ),
        Role::Assistant => (
            "claude",
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        ),
    };

    // Concatenate text items on the same "header" row.
    let mut first_text = true;
    for item in &msg.items {
        match item {
            ContentItem::Text(text) => {
                if first_text {
                    // Pull the first line of the text onto the role-label line.
                    let mut lines_iter = split_text_into_blocks(text, wrap_width);
                    if let Some(first) = lines_iter.next() {
                        let mut spans = vec![
                            Span::raw(" "),
                            Span::styled(label.to_string(), label_style),
                            Span::raw("  "),
                        ];
                        spans.extend(first.spans);
                        out.push(Line::from(spans));
                    } else {
                        out.push(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(label.to_string(), label_style),
                        ]));
                    }
                    // Following wrapped lines indent to align with the body.
                    for line in lines_iter {
                        let mut spans = vec![Span::raw("        ")];
                        spans.extend(line.spans);
                        out.push(Line::from(spans));
                    }
                    first_text = false;
                } else {
                    out.push(Line::raw(""));
                    for line in split_text_into_blocks(text, wrap_width) {
                        let mut spans = vec![Span::raw("        ")];
                        spans.extend(line.spans);
                        out.push(Line::from(spans));
                    }
                }
            }
            ContentItem::ToolUse { name, input } => {
                // Ensure role label appears even if no text came first.
                if first_text {
                    out.push(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(label.to_string(), label_style),
                    ]));
                    first_text = false;
                }
                out.push(Line::raw(""));
                tool_use_indices.push(out.len());
                push_tool_use_box(name, input, theme, content_width, out);
            }
            ContentItem::ToolResult { content, is_error } => {
                if first_text {
                    out.push(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(label.to_string(), label_style),
                    ]));
                    first_text = false;
                }
                out.push(Line::raw(""));
                push_tool_result_box(content, *is_error, theme, content_width, out);
            }
            ContentItem::Thinking { text } => {
                if first_text {
                    out.push(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(label.to_string(), label_style),
                    ]));
                    first_text = false;
                }
                out.push(Line::raw(""));
                push_thinking_box(text, theme, content_width, out);
            }
            ContentItem::Other(kind) => {
                if first_text {
                    out.push(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(label.to_string(), label_style),
                    ]));
                    first_text = false;
                }
                out.push(Line::from(vec![
                    Span::raw("        "),
                    Span::styled(format!("[unknown block: {kind}]"), theme.muted()),
                ]));
            }
        }
    }

    // If the message was entirely empty (rare — we skip truly empty
    // messages at parse time), at least render the role line so the user
    // sees it appeared.
    if first_text {
        out.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(label.to_string(), label_style),
        ]));
    }
}

/// Word-wrap `text` and render each resulting line. Also detects
/// triple-backtick code blocks and renders those lines with the code style.
fn split_text_into_blocks(text: &str, wrap_width: usize) -> std::vec::IntoIter<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut in_code = false;
    let width = wrap_width.max(20);

    for raw_line in text.split('\n') {
        let trimmed = raw_line.trim_end();
        if trimmed.starts_with("```") {
            // Toggle code-block state. Don't emit the fence line itself —
            // it clutters the viewer.
            in_code = !in_code;
            continue;
        }
        if in_code {
            // Preserve formatting, skip wrapping — code usually benefits
            // from horizontal overflow hidden rather than broken lines.
            let content = if trimmed.chars().count() > width {
                let mut s: String = trimmed.chars().take(width.saturating_sub(1)).collect();
                s.push('…');
                s
            } else {
                trimmed.to_string()
            };
            out.push(Line::from(vec![Span::styled(
                content,
                Style::default().bg(code_bg()).fg(Color::White),
            )]));
        } else {
            for wrapped in wrap_plain(trimmed, width) {
                out.push(Line::from(vec![Span::styled(
                    wrapped,
                    Style::default().fg(Color::Reset),
                )]));
            }
        }
    }

    out.into_iter()
}

fn code_bg() -> Color {
    // Use a dim gray as the shared code-background. Could be a theme
    // token, but keeping a constant keeps the viewer's "fenced code" style
    // consistent across themes without another palette knob.
    Color::Rgb(0x30, 0x34, 0x46)
}

/// Greedy word-wrap that respects leading whitespace. Returns every line
/// of a paragraph already broken to fit in `width` characters.
fn wrap_plain(line: &str, width: usize) -> Vec<String> {
    if line.chars().count() <= width {
        return vec![line.to_string()];
    }
    let leading: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let rest = &line[leading.len()..];

    let mut out: Vec<String> = Vec::new();
    let mut current = leading.clone();
    let effective_width = width.saturating_sub(leading.chars().count()).max(10);
    let mut word = String::new();
    let mut current_word_chars = 0usize;

    let push_word = |current: &mut String,
                     word: &mut String,
                     word_chars: &mut usize,
                     out: &mut Vec<String>,
                     effective_width: usize| {
        if word.is_empty() {
            return;
        }
        let current_width = current.chars().count();
        let leading_width = current.chars().take_while(|c| c.is_whitespace()).count();
        let content_width = current_width - leading_width;
        if content_width + 1 + *word_chars <= effective_width || content_width == 0 {
            if content_width > 0 {
                current.push(' ');
            }
            current.push_str(word);
        } else {
            out.push(current.clone());
            current.clear();
            // leading preserved on first line only — wrapped lines get no
            // extra indent so they don't march off-screen. Callers that
            // want indent can pre-indent the source.
            current.push_str(word);
        }
        word.clear();
        *word_chars = 0;
    };

    for ch in rest.chars() {
        if ch.is_whitespace() {
            push_word(
                &mut current,
                &mut word,
                &mut current_word_chars,
                &mut out,
                effective_width,
            );
        } else {
            word.push(ch);
            current_word_chars += 1;
        }
    }
    push_word(
        &mut current,
        &mut word,
        &mut current_word_chars,
        &mut out,
        effective_width,
    );
    if !current.trim().is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(line.to_string());
    }
    out
}

fn push_tool_use_box(
    name: &str,
    input: &serde_json::Value,
    theme: &Theme,
    content_width: usize,
    out: &mut Vec<Line<'static>>,
) {
    let border_style = Style::default().fg(theme.overlay0);
    let box_width = content_width.saturating_sub(8).max(20);
    let title = format!(" tool_use: {name} ");
    let top = {
        let body = "─".repeat(box_width.saturating_sub(title.chars().count() + 2));
        format!("╭─{title}{body}╮")
    };
    out.push(Line::from(vec![
        Span::raw("        "),
        Span::styled(top, border_style),
    ]));

    // Input rows — summarise + raw JSON if compact.
    let summary = summarize_tool_input(input);
    if !summary.is_empty() {
        let wrapped = wrap_plain(&summary, box_width.saturating_sub(4));
        for line in wrapped {
            let padded = pad_to_width(&line, box_width.saturating_sub(2));
            out.push(Line::from(vec![
                Span::raw("        "),
                Span::styled("│ ", border_style),
                Span::styled(padded, theme.muted()),
                Span::styled(" │", border_style),
            ]));
        }
    } else {
        let padded = pad_to_width("(no arguments)", box_width.saturating_sub(2));
        out.push(Line::from(vec![
            Span::raw("        "),
            Span::styled("│ ", border_style),
            Span::styled(padded, theme.dim()),
            Span::styled(" │", border_style),
        ]));
    }

    let bottom = format!("╰{}╯", "─".repeat(box_width));
    out.push(Line::from(vec![
        Span::raw("        "),
        Span::styled(bottom, border_style),
    ]));
}

fn push_tool_result_box(
    content: &str,
    is_error: bool,
    theme: &Theme,
    content_width: usize,
    out: &mut Vec<Line<'static>>,
) {
    let border_style = if is_error {
        Style::default().fg(theme.red)
    } else {
        Style::default().fg(theme.overlay0)
    };
    let box_width = content_width.saturating_sub(8).max(20);
    let title = if is_error {
        " tool_result: error ".to_string()
    } else {
        " tool_result ".to_string()
    };
    let top = {
        let body = "─".repeat(box_width.saturating_sub(title.chars().count() + 2));
        format!("╭─{title}{body}╮")
    };
    out.push(Line::from(vec![
        Span::raw("        "),
        Span::styled(top, border_style),
    ]));

    // Cap the tool result to a reasonable preview — huge outputs blow up
    // the line count and the user can always `y` to copy and inspect
    // externally.
    let preview = truncate_lines(content, 8, box_width.saturating_sub(4));
    for line in preview {
        let padded = pad_to_width(&line, box_width.saturating_sub(2));
        out.push(Line::from(vec![
            Span::raw("        "),
            Span::styled("│ ", border_style),
            Span::styled(padded, theme.muted()),
            Span::styled(" │", border_style),
        ]));
    }

    let bottom = format!("╰{}╯", "─".repeat(box_width));
    out.push(Line::from(vec![
        Span::raw("        "),
        Span::styled(bottom, border_style),
    ]));
}

fn push_thinking_box(
    text: &str,
    theme: &Theme,
    content_width: usize,
    out: &mut Vec<Line<'static>>,
) {
    let border_style = Style::default().fg(theme.overlay0);
    let box_width = content_width.saturating_sub(8).max(20);
    let token_count = text.split_whitespace().count();
    let title = format!(" Thinking ({token_count} words) ");
    let top = {
        let body = "─".repeat(box_width.saturating_sub(title.chars().count() + 2));
        format!("╭─{title}{body}╮")
    };
    out.push(Line::from(vec![
        Span::raw("        "),
        Span::styled(top, border_style),
    ]));

    let wrapped = wrap_plain(text, box_width.saturating_sub(4));
    for line in wrapped {
        let padded = pad_to_width(&line, box_width.saturating_sub(2));
        out.push(Line::from(vec![
            Span::raw("        "),
            Span::styled("│ ", border_style),
            Span::styled(
                padded,
                Style::default()
                    .fg(theme.overlay1)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(" │", border_style),
        ]));
    }

    let bottom = format!("╰{}╯", "─".repeat(box_width));
    out.push(Line::from(vec![
        Span::raw("        "),
        Span::styled(bottom, border_style),
    ]));
}

fn truncate_lines(text: &str, max_rows: usize, col_width: usize) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for raw_line in text.split('\n') {
        for wrapped in wrap_plain(raw_line, col_width) {
            if rows.len() >= max_rows {
                rows.push("…".to_string());
                return rows;
            }
            rows.push(wrapped);
        }
    }
    rows
}

fn pad_to_width(s: &str, width: usize) -> String {
    let n = s.chars().count();
    if n >= width {
        let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        return out;
    }
    let mut out = String::with_capacity(s.len() + (width - n));
    out.push_str(s);
    for _ in 0..(width - n) {
        out.push(' ');
    }
    out
}

fn summarize_tool_input(input: &serde_json::Value) -> String {
    let Some(obj) = input.as_object() else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    for (k, v) in obj {
        let rendered = match v {
            serde_json::Value::String(s) => {
                if s.len() > 120 {
                    format!("{}…", &s[..120])
                } else {
                    s.clone()
                }
            }
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Null => "null".to_string(),
            other => {
                let j = other.to_string();
                if j.len() > 120 {
                    format!("{}…", &j[..120])
                } else {
                    j
                }
            }
        };
        parts.push(format!("{k}: {rendered}"));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::pricing::TokenCounts;
    use crate::data::session::SessionKind;
    use std::path::PathBuf;

    fn mk_session() -> Session {
        Session {
            id: "test-session".to_string(),
            project_dir: PathBuf::from("/tmp"),
            name: Some("test".into()),
            auto_name: None,
            last_prompt: None,
            message_count: 5,
            tokens: TokenCounts::default(),
            total_cost_usd: 0.5,
            model_summary: "claude-opus-4-7".into(),
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
    fn wrap_plain_splits_long_line() {
        let s = "a b c d e f g h";
        let out = wrap_plain(s, 5);
        assert!(out.len() > 1);
    }

    #[test]
    fn wrap_plain_short_line_unchanged() {
        let out = wrap_plain("hello", 20);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn open_with_missing_session_records_error() {
        let s = mk_session();
        let state = ViewerState::open(&s);
        // test-session won't exist on disk; state should carry an error.
        assert!(state.load_error.is_some());
        assert!(state.messages.is_empty());
    }

    #[test]
    fn scroll_by_stays_within_bounds() {
        let s = mk_session();
        let mut state = ViewerState::open(&s);
        state.rendered_lines = (0..100).map(|_| Line::raw("x")).collect();
        state.scroll_by(200);
        assert!(state.scroll <= state.rendered_lines.len());
        state.scroll_by(-10_000);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn search_cycles_matches() {
        let s = mk_session();
        let mut state = ViewerState::open(&s);
        state.match_line_indices = vec![10, 20, 30];
        state.active_match = 0;
        state.next_match(1);
        assert_eq!(state.active_match, 1);
        state.next_match(1);
        assert_eq!(state.active_match, 2);
        state.next_match(1);
        assert_eq!(state.active_match, 0, "wraps");
        state.next_match(-1);
        assert_eq!(state.active_match, 2);
    }

    #[test]
    fn pad_to_width_truncates_too_long() {
        let s = pad_to_width("abcdefghij", 5);
        assert_eq!(s.chars().count(), 5);
        assert!(s.ends_with('…'));
    }

    #[test]
    fn pad_to_width_pads_short() {
        let s = pad_to_width("hi", 5);
        assert_eq!(s, "hi   ");
    }
}
