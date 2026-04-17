//! `claude-picker --files` — the file-centric pivot screen.
//!
//! Two-pane layout. The left pane is a filtered, sorted list of every
//! file Claude Code ever touched (across every session). The right pane
//! is split vertically into a session list for the focused file and a
//! preview of the file's most-recent changes.
//!
//! This module is pure view state + rendering; `commands::files_cmd`
//! owns the event loop and the data load.

use chrono::{DateTime, Utc};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::data::file_index::{FileIndex, FileStats, SessionRef};
use crate::theme::Theme;
use crate::ui::text::truncate_to_width;

/// Which pane has focus. Determines where cursor keys move and what
/// Enter means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    FileList,
    SessionList,
}

/// Default + cycle-order for the sort menu. `s` cycles through these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    EditsDesc,
    RecencyDesc,
    SessionCountDesc,
    PathAlpha,
}

impl Sort {
    pub fn label(self) -> &'static str {
        match self {
            Self::EditsDesc => "edits",
            Self::RecencyDesc => "recency",
            Self::SessionCountDesc => "sessions",
            Self::PathAlpha => "path",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::EditsDesc => Self::RecencyDesc,
            Self::RecencyDesc => Self::SessionCountDesc,
            Self::SessionCountDesc => Self::PathAlpha,
            Self::PathAlpha => Self::EditsDesc,
        }
    }
}

/// Mutable screen state driven by the event loop.
pub struct FilesState {
    pub index: FileIndex,
    /// Fuzzy filter text.
    pub filter: String,
    /// Indices into `index.files` in current render order.
    pub visible: Vec<usize>,
    /// Cursor position inside `visible` for the file list.
    pub file_cursor: usize,
    /// Cursor position inside the focused file's sessions list.
    pub session_cursor: usize,
    pub focus: Focus,
    pub sort: Sort,
    pub loading: bool,
    /// Live count shown while the background loader is still running.
    pub loader_progress: u32,
    /// Toast — one-shot status message with a 1.5s TTL.
    pub toast: Option<(String, ToastKind, std::time::Instant)>,
    /// `?` help overlay.
    pub show_help: bool,
    /// Optional scoped project name (passed from `--files --project foo`).
    pub project_scope: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

impl FilesState {
    pub fn new(project_scope: Option<String>) -> Self {
        Self {
            index: FileIndex::default(),
            filter: String::new(),
            visible: Vec::new(),
            file_cursor: 0,
            session_cursor: 0,
            focus: Focus::FileList,
            sort: Sort::EditsDesc,
            loading: true,
            loader_progress: 0,
            toast: None,
            show_help: false,
            project_scope,
        }
    }

    /// Expire toasts older than 1.5s. Called once per frame.
    pub fn tick(&mut self) {
        if let Some((_, _, expires)) = &self.toast {
            if std::time::Instant::now() >= *expires {
                self.toast = None;
            }
        }
    }

    pub fn set_toast(&mut self, msg: impl Into<String>, kind: ToastKind) {
        self.toast = Some((
            msg.into(),
            kind,
            std::time::Instant::now() + std::time::Duration::from_millis(1500),
        ));
    }

    /// Currently-focused file, if any.
    pub fn focused_file(&self) -> Option<&FileStats> {
        let idx = *self.visible.get(self.file_cursor_clamped())?;
        self.index.files.get(idx)
    }

    /// Currently-focused session under the focused file.
    pub fn focused_session(&self) -> Option<&SessionRef> {
        self.focused_file()?
            .sessions
            .get(self.session_cursor_clamped())
    }

    pub fn file_cursor_clamped(&self) -> usize {
        if self.visible.is_empty() {
            0
        } else {
            self.file_cursor.min(self.visible.len() - 1)
        }
    }

    pub fn session_cursor_clamped(&self) -> usize {
        let Some(f) = self.focused_file() else {
            return 0;
        };
        if f.sessions.is_empty() {
            0
        } else {
            self.session_cursor.min(f.sessions.len() - 1)
        }
    }

    /// Re-sort + re-filter the visible list. Call this whenever the
    /// filter text, sort mode, or index itself changes.
    pub fn recompute(&mut self) {
        use nucleo::pattern::{CaseMatching, Normalization, Pattern};
        use nucleo::{Config, Matcher, Utf32String};

        // 1) Build ordered indices.
        let mut scored: Vec<usize> = (0..self.index.files.len()).collect();

        // 2) Apply filter.
        if !self.filter.is_empty() {
            let mut matcher = Matcher::new(Config::DEFAULT);
            let pat = Pattern::parse(&self.filter, CaseMatching::Smart, Normalization::Smart);
            let mut with_score: Vec<(u32, usize)> = Vec::new();
            for i in scored {
                let hay = self
                    .index
                    .files
                    .get(i)
                    .map(|f| f.path.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let hs = Utf32String::from(hay);
                if let Some(score) = pat.score(hs.slice(..), &mut matcher) {
                    with_score.push((score, i));
                }
            }
            with_score.sort_unstable_by_key(|(score, _)| std::cmp::Reverse(*score));
            scored = with_score.into_iter().map(|(_, i)| i).collect();
        } else {
            // No filter → sort by the chosen mode.
            let files = &self.index.files;
            scored.sort_by(|&a, &b| {
                let fa = &files[a];
                let fb = &files[b];
                match self.sort {
                    Sort::EditsDesc => fb
                        .edit_count
                        .cmp(&fa.edit_count)
                        .then_with(|| fb.last_touched.cmp(&fa.last_touched)),
                    Sort::RecencyDesc => fb.last_touched.cmp(&fa.last_touched),
                    Sort::SessionCountDesc => fb
                        .session_count
                        .cmp(&fa.session_count)
                        .then_with(|| fb.last_touched.cmp(&fa.last_touched)),
                    Sort::PathAlpha => fa.path.cmp(&fb.path),
                }
            });
        }

        self.visible = scored;
        // Clamp cursors.
        if self.file_cursor >= self.visible.len() {
            self.file_cursor = self.visible.len().saturating_sub(1);
        }
        self.session_cursor = 0;
    }
}

impl Default for FilesState {
    fn default() -> Self {
        Self::new(None)
    }
}

// ── Rendering ────────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &mut FilesState, theme: &Theme) {
    // Outer block + header.
    let total_files = state.index.files.len();
    let visible = state.visible.len();
    let session_total = state.index.session_total;

    let title_left = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "claude-picker",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", theme.dim()),
        Span::styled("files", theme.subtle()),
        Span::raw(" "),
    ]);

    let header_right = if state.loading {
        format!("scanning sessions… {} found ", state.loader_progress)
    } else if state.filter.is_empty() {
        if let Some(p) = &state.project_scope {
            format!(
                "{} files · {} sessions · project {} ",
                total_files, session_total, p
            )
        } else {
            format!(
                "{} files touched across {} sessions ",
                total_files, session_total
            )
        }
    } else {
        format!("{} / {} matches ", visible, total_files)
    };

    let title_right = Line::from(vec![
        Span::raw(" "),
        Span::styled(header_right, theme.muted()),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border_active())
        .title(title_left)
        .title(title_right.alignment(Alignment::Right));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical split:
    //   1 pad
    //   1 filter input
    //   1 pad
    //   60% file list
    //   40% bottom pane (session list + preview horizontal)
    //   1 footer
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Percentage(55),
            Constraint::Min(7),
            Constraint::Length(1),
        ])
        .split(inner);

    render_filter(frame, rows[1], state, theme);
    render_file_list(frame, rows[3], state, theme);
    render_bottom(frame, rows[4], state, theme);
    render_footer(frame, rows[5], state, theme);

    if let Some((msg, kind, _)) = &state.toast {
        render_toast(frame, area, msg, *kind, theme);
    }
    if state.show_help {
        let content = crate::ui::help_overlay::help_for(crate::ui::help_overlay::Screen::Files);
        crate::ui::help_overlay::render(frame, area, content, theme);
    }
}

fn render_filter(frame: &mut Frame<'_>, area: Rect, state: &FilesState, theme: &Theme) {
    let mut spans = vec![
        Span::raw("  "),
        Span::styled(
            "Filter:",
            Style::default()
                .fg(theme.subtext0)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];
    if state.filter.is_empty() {
        spans.push(Span::styled(
            "type to fuzzy-filter file paths…",
            theme.filter_placeholder(),
        ));
    } else {
        spans.push(Span::styled(state.filter.clone(), theme.filter_text()));
    }
    spans.push(Span::styled(
        "_",
        Style::default().fg(theme.mauve).add_modifier(Modifier::DIM),
    ));
    spans.push(Span::raw("   "));
    spans.push(Span::styled(
        format!("sort: {}", state.sort.label()),
        theme.muted(),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_file_list(frame: &mut Frame<'_>, area: Rect, state: &FilesState, theme: &Theme) {
    if state.loading && state.index.files.is_empty() {
        let msg = format!("Scanning sessions… {} found so far", state.loader_progress);
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(msg, theme.muted()),
            Line::styled(
                "(reading every JSONL under ~/.claude/projects)",
                theme.dim(),
            ),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(p, area);
        return;
    }
    if state.visible.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                if state.filter.is_empty() {
                    "No files found.".to_string()
                } else {
                    format!("No files match \"{}\".", state.filter)
                },
                theme.muted(),
            ),
        ])
        .alignment(Alignment::Center);
        frame.render_widget(p, area);
        return;
    }

    let rows_available = area.height as usize;
    // Simple window scroll — keep the cursor in view.
    let cursor = state.file_cursor_clamped();
    let window_start = cursor.saturating_sub(rows_available.saturating_sub(2));
    let window_end = (window_start + rows_available).min(state.visible.len());

    let now = Utc::now();
    let path_col_width = (area.width as usize).saturating_sub(44).max(20);

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(window_end - window_start);
    for i in window_start..window_end {
        let idx = state.visible[i];
        let Some(f) = state.index.files.get(idx) else {
            continue;
        };
        let is_cursor = i == cursor && state.focus == Focus::FileList;
        let pointer = if is_cursor { "▸ " } else { "  " };

        let pointer_span = Span::styled(
            pointer,
            if is_cursor {
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.overlay0)
            },
        );

        let short = shorten_path(&f.path, path_col_width);
        let name_style = if is_cursor {
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD)
        } else {
            theme.body()
        };

        let session_label = if f.session_count == 1 {
            "1 session".to_string()
        } else {
            format!("{} sessions", f.session_count)
        };
        let bar = activity_bar(now, f.last_touched);
        let recency = relative_time(now, f.last_touched);

        // Pre-compute padding so the meta columns line up, then build
        // the row in one shot.
        let used = crate::ui::text::display_width(&short) + 2; // pointer
        let pad = path_col_width.saturating_sub(used) + 2;
        let mut row = vec![
            pointer_span,
            Span::styled(short, name_style),
            Span::raw(" ".repeat(pad)),
        ];
        row.push(Span::styled(
            format!("{:>12}", session_label),
            Style::default().fg(theme.yellow),
        ));
        row.push(Span::raw("  "));
        row.push(Span::styled(bar, Style::default().fg(theme.mauve)));
        row.push(Span::raw(" "));
        row.push(Span::styled(format!("{:>5}", recency), theme.muted()));

        lines.push(Line::from(row));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_bottom(frame: &mut Frame<'_>, area: Rect, state: &FilesState, theme: &Theme) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    render_session_list(frame, cols[0], state, theme);
    render_preview(frame, cols[1], state, theme);
}

fn render_session_list(frame: &mut Frame<'_>, area: Rect, state: &FilesState, theme: &Theme) {
    let is_focused = state.focus == Focus::SessionList;
    let border = if is_focused {
        theme.panel_border_active()
    } else {
        theme.panel_border()
    };
    let title = match state.focused_file() {
        Some(f) => format!(
            " sessions that touched {} ",
            f.path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(f.path.to_string_lossy().as_ref())
        ),
        None => " sessions ".to_string(),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Line::from(Span::styled(
            title,
            Style::default()
                .fg(theme.subtext0)
                .add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(file) = state.focused_file() else {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("  (no file selected)", theme.muted()),
        ]);
        frame.render_widget(p, inner);
        return;
    };

    if file.sessions.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("  (no sessions attributed)", theme.muted()),
        ]);
        frame.render_widget(p, inner);
        return;
    }

    let cursor = state.session_cursor_clamped();
    let rows_available = inner.height as usize;
    // Reserve 2 lines for the totals footer.
    let list_rows = rows_available.saturating_sub(2);
    let window_start = cursor.saturating_sub(list_rows.saturating_sub(1));
    let window_end = (window_start + list_rows).min(file.sessions.len());

    let now = Utc::now();

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(window_end - window_start + 3);
    lines.push(Line::raw(""));
    for i in window_start..window_end {
        let Some(s) = file.sessions.get(i) else {
            continue;
        };
        let is_cursor = i == cursor && is_focused;
        let glyph = session_glyph(now, s.last_edit_in_session);
        let glyph_span = Span::styled(
            format!("  {} ", glyph),
            Style::default().fg(if is_cursor { theme.mauve } else { theme.teal }),
        );
        let name_style = if is_cursor {
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD)
        } else {
            theme.body()
        };
        let name = truncate_to_width(&s.session_name, 26);
        let rel = relative_time(now, s.last_edit_in_session);
        let cost = if s.session_cost_usd > 0.0 {
            format!("${:.2}", s.session_cost_usd)
        } else {
            "—".to_string()
        };

        lines.push(Line::from(vec![
            glyph_span,
            Span::styled(name, name_style),
            Span::raw("  "),
            Span::styled(format!("{:>6}", rel), theme.muted()),
            Span::raw("  "),
            Span::styled(format!("{:>7}", cost), Style::default().fg(theme.green)),
        ]));
    }
    // Totals line.
    let total_edits: u32 = file.sessions.iter().map(|s| s.edits_in_this_session).sum();
    let total_cost: f64 = file.sessions.iter().map(|s| s.session_cost_usd).sum();
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!(
                "Totals: {} sessions · {} edits · ${:.2}",
                file.sessions.len(),
                total_edits,
                total_cost
            ),
            theme.muted(),
        ),
    ]));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_preview(frame: &mut Frame<'_>, area: Rect, state: &FilesState, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border())
        .title(Line::from(Span::styled(
            " most-recent edits ",
            Style::default()
                .fg(theme.subtext0)
                .add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(file) = state.focused_file() else {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("  (select a file)", theme.muted()),
        ]);
        frame.render_widget(p, inner);
        return;
    };

    let mut lines: Vec<Line<'_>> = Vec::new();
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(file.path.to_string_lossy().into_owned(), theme.body()),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!(
                "{} edits · +{} lines · -{} lines",
                file.edit_count, file.total_lines_added, file.total_lines_removed
            ),
            theme.muted(),
        ),
    ]));
    lines.push(Line::raw(""));

    // Show up to 3 of the most-recent sessions' summary. This is the
    // "preview of this file's most-recent changes" pane from the mock —
    // cheap and non-disk-hitting.
    let now = Utc::now();
    for s in file.sessions.iter().take(3) {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("── {} ──", truncate_to_width(&s.session_name, 24)),
                Style::default().fg(theme.mauve).add_modifier(Modifier::DIM),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                format!("[{}]", s.last_edit_in_session.format("%H:%M")),
                theme.muted(),
            ),
            Span::raw(" "),
            Span::styled(
                format!(
                    "{} edit{}, +{} / -{} lines · {} ago",
                    s.edits_in_this_session,
                    if s.edits_in_this_session == 1 {
                        ""
                    } else {
                        "s"
                    },
                    s.lines_added,
                    s.lines_removed,
                    relative_time(now, s.last_edit_in_session)
                ),
                theme.body(),
            ),
        ]));
        lines.push(Line::raw(""));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: &FilesState, theme: &Theme) {
    let hints: &[(&str, &str)] = if state.focus == Focus::FileList {
        &[
            ("↑↓/jk", "nav"),
            ("/", "filter"),
            ("s", "sort"),
            ("Tab", "sessions"),
            ("o", "open file"),
            ("?", "help"),
            ("q", "quit"),
        ]
    } else {
        &[
            ("↑↓/jk", "nav"),
            ("Enter", "resume"),
            ("v", "viewer"),
            ("Tab", "files"),
            ("Esc", "back"),
            ("?", "help"),
            ("q", "quit"),
        ]
    };
    let mut spans: Vec<Span<'_>> = Vec::with_capacity(hints.len() * 4);
    spans.push(Span::raw(" "));
    for (i, (k, d)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", theme.dim()));
        }
        spans.push(Span::styled(*k, theme.key_hint()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*d, theme.key_desc()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_toast(frame: &mut Frame<'_>, area: Rect, msg: &str, kind: ToastKind, theme: &Theme) {
    use ratatui::widgets::Clear;
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
    frame.render_widget(Clear, rect);
    let (accent, label) = match kind {
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
    let p = Paragraph::new(Line::from(Span::styled(format!(" {msg} "), theme.body())))
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(p, rect);
}

// ── Small presentation helpers ──────────────────────────────────────────

/// The activity bar next to each file. `█████` / `████░` / `██░░░` /
/// `░░░░░` depending on how recently the file was last touched.
pub fn activity_bar(now: DateTime<Utc>, last: DateTime<Utc>) -> String {
    let diff = now.signed_duration_since(last);
    let hours = diff.num_hours();
    if hours < 1 {
        "█████".to_string()
    } else if hours < 24 {
        "████░".to_string()
    } else if hours < 24 * 7 {
        "██░░░".to_string()
    } else {
        "░░░░░".to_string()
    }
}

/// `● / ◆ / ○` marker for each session row, sized by recency. Mirrors the
/// mockup's "fresh / intermediate / stale" glyphs.
pub fn session_glyph(now: DateTime<Utc>, last: DateTime<Utc>) -> char {
    let hours = now.signed_duration_since(last).num_hours();
    if hours < 6 {
        '●'
    } else if hours < 24 * 2 {
        '◆'
    } else {
        '○'
    }
}

/// Smart short-form relative time. "5m" / "2h" / "3d" / "4w".
pub fn relative_time(now: DateTime<Utc>, then: DateTime<Utc>) -> String {
    let diff = now.signed_duration_since(then);
    let seconds = diff.num_seconds();
    if seconds < 60 {
        return "now".to_string();
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{}m", minutes);
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{}h", hours);
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{}d", days);
    }
    let weeks = days / 7;
    if weeks < 52 {
        return format!("{}w", weeks);
    }
    let years = days / 365;
    format!("{}y", years)
}

/// Shorten a long path to fit a column budget. Prefers showing the tail
/// ("…/src/main.rs") because users recognise file names; a basename
/// always survives.
pub fn shorten_path(path: &std::path::Path, max_chars: usize) -> String {
    let s = path.to_string_lossy().into_owned();
    if s.chars().count() <= max_chars {
        return s;
    }
    // Fall back to tail with a leading ellipsis.
    let take_from_end = max_chars.saturating_sub(1);
    let tail: String = s
        .chars()
        .rev()
        .take(take_from_end)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{}", tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use std::path::PathBuf;

    #[test]
    fn activity_bar_buckets() {
        let now = Utc::now();
        assert_eq!(activity_bar(now, now - Duration::minutes(5)), "█████");
        assert_eq!(activity_bar(now, now - Duration::hours(5)), "████░");
        assert_eq!(activity_bar(now, now - Duration::days(3)), "██░░░");
        assert_eq!(activity_bar(now, now - Duration::days(30)), "░░░░░");
    }

    #[test]
    fn session_glyph_buckets() {
        let now = Utc::now();
        assert_eq!(session_glyph(now, now - Duration::minutes(10)), '●');
        assert_eq!(session_glyph(now, now - Duration::hours(10)), '◆');
        assert_eq!(session_glyph(now, now - Duration::days(5)), '○');
    }

    #[test]
    fn relative_time_matches_expected_shape() {
        let now = Utc::now();
        assert_eq!(relative_time(now, now - Duration::seconds(15)), "now");
        assert_eq!(relative_time(now, now - Duration::minutes(5)), "5m");
        assert_eq!(relative_time(now, now - Duration::hours(3)), "3h");
        assert_eq!(relative_time(now, now - Duration::days(2)), "2d");
        // Weeks label only kicks in above the 30-day "d" threshold, so 3w (21 days)
        // still renders as "21d". Both sides of the boundary:
        assert_eq!(relative_time(now, now - Duration::days(25)), "25d");
        assert_eq!(relative_time(now, now - Duration::days(60)), "8w");
    }

    #[test]
    fn sort_cycle_order() {
        assert_eq!(Sort::EditsDesc.next(), Sort::RecencyDesc);
        assert_eq!(Sort::RecencyDesc.next(), Sort::SessionCountDesc);
        assert_eq!(Sort::SessionCountDesc.next(), Sort::PathAlpha);
        assert_eq!(Sort::PathAlpha.next(), Sort::EditsDesc);
    }

    #[test]
    fn shorten_path_keeps_tail_on_long_paths() {
        let p = PathBuf::from("/Users/someone/very/deep/path/to/project/src/auth/middleware.ts");
        let short = shorten_path(&p, 30);
        assert!(short.chars().count() <= 30);
        assert!(short.ends_with("middleware.ts"));
    }

    #[test]
    fn shorten_path_passthrough_when_short() {
        let p = PathBuf::from("src/x.rs");
        assert_eq!(shorten_path(&p, 30), "src/x.rs");
    }

    #[test]
    fn recompute_respects_sort_modes() {
        use crate::data::file_index::FileStats;
        let older = Utc::now() - Duration::days(10);
        let newer = Utc::now();
        let mut state = FilesState::new(None);
        state.index = FileIndex {
            files: vec![
                FileStats {
                    path: PathBuf::from("/a/old.rs"),
                    session_count: 10,
                    edit_count: 2,
                    total_lines_added: 0,
                    total_lines_removed: 0,
                    last_touched: older,
                    sessions: vec![],
                    project_name: "a".into(),
                },
                FileStats {
                    path: PathBuf::from("/a/new.rs"),
                    session_count: 1,
                    edit_count: 100,
                    total_lines_added: 0,
                    total_lines_removed: 0,
                    last_touched: newer,
                    sessions: vec![],
                    project_name: "a".into(),
                },
            ],
            session_total: 2,
            built_at: newer,
        };

        state.sort = Sort::EditsDesc;
        state.recompute();
        assert_eq!(state.visible[0], 1); // new.rs has more edits

        state.sort = Sort::RecencyDesc;
        state.recompute();
        assert_eq!(state.visible[0], 1); // new.rs touched more recently

        state.sort = Sort::SessionCountDesc;
        state.recompute();
        assert_eq!(state.visible[0], 0); // old.rs has more sessions

        state.sort = Sort::PathAlpha;
        state.recompute();
        // "/a/new.rs" < "/a/old.rs" alphabetically.
        assert_eq!(state.visible[0], 1);
    }

    #[test]
    fn filter_narrows_visible_list() {
        use crate::data::file_index::FileStats;
        let now = Utc::now();
        let mut state = FilesState::new(None);
        state.index = FileIndex {
            files: vec![
                FileStats {
                    path: PathBuf::from("/a/auth/middleware.ts"),
                    session_count: 1,
                    edit_count: 1,
                    total_lines_added: 0,
                    total_lines_removed: 0,
                    last_touched: now,
                    sessions: vec![],
                    project_name: "a".into(),
                },
                FileStats {
                    path: PathBuf::from("/a/db/schema.sql"),
                    session_count: 1,
                    edit_count: 1,
                    total_lines_added: 0,
                    total_lines_removed: 0,
                    last_touched: now,
                    sessions: vec![],
                    project_name: "a".into(),
                },
            ],
            session_total: 2,
            built_at: now,
        };
        state.filter = "midd".to_string();
        state.recompute();
        assert_eq!(state.visible.len(), 1);
        // Only middleware.ts should match.
        assert_eq!(
            state.index.files[state.visible[0]].path,
            PathBuf::from("/a/auth/middleware.ts")
        );
    }
}
