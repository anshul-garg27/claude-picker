//! Diff screen — side-by-side comparison of two sessions.
//!
//! Rendered as a single bordered panel with four stacked bands:
//!
//! 1. **Header** — "A vs B" titles with a `(forked)` badge when applicable.
//! 2. **Common topics** — a wrapped line of green `●` chips.
//! 3. **Unique topics** — two peach-tinted columns (`◆` chips).
//! 4. **Conversation preview** — two columns of the tail user/claude exchanges,
//!    separated by a vertical rule.
//!
//! The screen is resize-aware: width is capped at 140 cols (wider terminals
//! just leave outer whitespace), and if the terminal is too small we fall back
//! to a "resize please" placeholder. Scrolling (`↑↓`) moves both previews in
//! lock-step; `Tab` merely shifts the visual focus indicator — there is no
//! independent per-pane scroll in v1.
//!
//! Topic extraction lives in [`crate::commands::diff_cmd`]; this module only
//! renders whatever [`DiffData`] it receives.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::data::Session;
use crate::theme::Theme;

/// Maximum screen width. Beyond this we center the panel so long lines don't
/// stretch to an unreadable width.
pub const MAX_WIDTH: u16 = 140;

/// Below this width the diff is unusable; we prompt for a resize.
pub const MIN_WIDTH: u16 = 80;
/// Below this height the diff is unusable; we prompt for a resize.
pub const MIN_HEIGHT: u16 = 22;

/// One exchange in the conversation preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    User,
    Claude,
}

/// Everything the diff screen needs in order to render.
///
/// Built by [`crate::commands::diff_cmd`] and handed to [`render`]. All fields
/// are owned so the event loop can mutate `scroll_offset` / `focus_right`
/// between frames without re-building anything else.
#[derive(Debug, Clone)]
pub struct DiffData {
    pub session_a: Session,
    pub session_b: Session,
    /// Tail user/claude pairs for session A, oldest → newest.
    pub preview_a: Vec<(Role, String)>,
    pub preview_b: Vec<(Role, String)>,
    pub topics_common: Vec<String>,
    pub topics_unique_a: Vec<String>,
    pub topics_unique_b: Vec<String>,
    /// Synchronized scroll offset (in rendered lines) applied to both preview
    /// columns.
    pub scroll_offset: usize,
    /// Which column has focus — tinted borders + footer hint reflect this.
    pub focus_right: bool,
}

impl DiffData {
    /// Flip A and B. Called in response to the `s` / `Shift+Tab` keybinding.
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.session_a, &mut self.session_b);
        std::mem::swap(&mut self.preview_a, &mut self.preview_b);
        std::mem::swap(&mut self.topics_unique_a, &mut self.topics_unique_b);
        // `topics_common` and `scroll_offset` don't change under swap.
    }

    /// Move the scroll cursor by `delta` (negative = up), clamped to `[0, max]`.
    pub fn scroll_by(&mut self, delta: i32, max: usize) {
        let current = self.scroll_offset as i32;
        let next = (current + delta).max(0) as usize;
        self.scroll_offset = next.min(max);
    }
}

/// Render the diff screen into `area`.
///
/// Assumes `area` is already the terminal's full rect; width capping and
/// centering happen inside this function.
pub fn render(frame: &mut Frame<'_>, area: Rect, data: &DiffData, theme: &Theme) {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area, theme);
        return;
    }

    let centered = center_to_max(area, MAX_WIDTH);

    // Outer panel — title, counter.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "claude-picker",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · ", theme.dim()),
            Span::styled("diff", Style::default().fg(theme.subtext1)),
            Span::raw(" "),
        ]));
    let inner = block.inner(centered);
    frame.render_widget(block, centered);

    // Vertical layout: top header (3 rows), common topics (2), unique topics (4),
    // rule (1), conversation preview (flex), footer (1).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title band
            Constraint::Length(3), // common topics
            Constraint::Length(5), // unique topics (two cols)
            Constraint::Length(1), // horizontal rule
            Constraint::Min(6),    // conversations
            Constraint::Length(1), // footer hint
        ])
        .split(inner);

    render_title_band(frame, chunks[0], data, theme);
    render_common_topics(frame, chunks[1], data, theme);
    render_unique_topics(frame, chunks[2], data, theme);
    render_hrule(frame, chunks[3], theme);
    render_previews(frame, chunks[4], data, theme);
    render_footer(frame, chunks[5], data, theme);
}

/// Center `area` horizontally to `max_width`. Returns a rect no wider than
/// `max_width`, centered on `area`'s x axis. Height is unchanged.
fn center_to_max(area: Rect, max_width: u16) -> Rect {
    if area.width <= max_width {
        return area;
    }
    let extra = area.width - max_width;
    let pad = extra / 2;
    Rect {
        x: area.x + pad,
        y: area.y,
        width: max_width,
        height: area.height,
    }
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let p = Paragraph::new(vec![
        Line::raw(""),
        Line::raw(""),
        Line::styled(
            "Terminal too small for diff view.",
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            format!(
                "Resize to at least {}×{} (current {}×{}).",
                MIN_WIDTH, MIN_HEIGHT, area.width, area.height
            ),
            theme.muted(),
        ),
    ])
    .alignment(Alignment::Center);
    frame.render_widget(p, area);
}

fn render_title_band(frame: &mut Frame<'_>, area: Rect, data: &DiffData, theme: &Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // Row 1: "session diff — pick results"
    let heading = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "session diff",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(heading), rows[0]);

    // Row 2: blank.
    // Row 3: "<name A>      vs     <name B>   (forked)"
    let name_a = data.session_a.display_label();
    let name_b = data.session_b.display_label();
    let forked_note = if is_fork_relationship(&data.session_a, &data.session_b) {
        Some("(forked)")
    } else {
        None
    };

    let mut spans = vec![
        Span::raw("  "),
        Span::styled(
            name_a.to_string(),
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   vs   ", theme.dim()),
        Span::styled(
            name_b.to_string(),
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(note) = forked_note {
        spans.push(Span::styled("  ", theme.dim()));
        spans.push(Span::styled(
            note.to_string(),
            Style::default().fg(theme.peach),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), rows[2]);
}

/// True when either session points at the other as its fork parent.
fn is_fork_relationship(a: &Session, b: &Session) -> bool {
    a.forked_from.as_deref() == Some(b.id.as_str())
        || b.forked_from.as_deref() == Some(a.id.as_str())
}

fn render_common_topics(frame: &mut Frame<'_>, area: Rect, data: &DiffData, theme: &Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    let heading = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "common topics",
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  ({})", data.topics_common.len()), theme.muted()),
    ]);
    frame.render_widget(Paragraph::new(heading), rows[0]);

    if data.topics_common.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("(no overlapping topics)", theme.dim()),
            ])),
            rows[1],
        );
        return;
    }

    let mut spans = vec![Span::raw("  ")];
    for (i, topic) in data.topics_common.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled("● ", Style::default().fg(theme.green)));
        spans.push(Span::styled(topic.clone(), theme.body()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false }),
        rows[1],
    );
}

fn render_unique_topics(frame: &mut Frame<'_>, area: Rect, data: &DiffData, theme: &Theme) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_unique_column(
        frame,
        cols[0],
        &data.topics_unique_a,
        data.session_a.display_label(),
        theme,
    );
    render_unique_column(
        frame,
        cols[1],
        &data.topics_unique_b,
        data.session_b.display_label(),
        theme,
    );
}

fn render_unique_column(
    frame: &mut Frame<'_>,
    area: Rect,
    topics: &[String],
    session_label: &str,
    theme: &Theme,
) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    let heading = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("unique to {session_label}"),
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  ({})", topics.len()), theme.muted()),
    ]);
    frame.render_widget(Paragraph::new(heading), rows[0]);

    if topics.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("(none)", theme.dim()),
            ])),
            rows[1],
        );
        return;
    }

    let mut spans = vec![Span::raw("  ")];
    for (i, topic) in topics.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled("◆ ", Style::default().fg(theme.peach)));
        spans.push(Span::styled(topic.clone(), theme.body()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false }),
        rows[1],
    );
}

fn render_hrule(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let width = area.width.saturating_sub(2) as usize;
    let rule = "─".repeat(width);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(rule, theme.dim()),
        ])),
        area,
    );
}

fn render_previews(frame: &mut Frame<'_>, area: Rect, data: &DiffData, theme: &Theme) {
    // Split into [left col] [1-wide vertical rule] [right col].
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Length(1),
            Constraint::Min(10),
        ])
        .split(area);

    render_conversation_column(
        frame,
        cols[0],
        &data.session_a,
        &data.preview_a,
        data.scroll_offset,
        !data.focus_right,
        theme,
    );
    render_vrule(frame, cols[1], theme);
    render_conversation_column(
        frame,
        cols[2],
        &data.session_b,
        &data.preview_b,
        data.scroll_offset,
        data.focus_right,
        theme,
    );
}

fn render_vrule(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let mut spans_lines = Vec::with_capacity(area.height as usize);
    for _ in 0..area.height {
        spans_lines.push(Line::from(Span::styled("│", theme.dim())));
    }
    frame.render_widget(Paragraph::new(spans_lines), area);
}

fn render_conversation_column(
    frame: &mut Frame<'_>,
    area: Rect,
    session: &Session,
    preview: &[(Role, String)],
    scroll_offset: usize,
    focused: bool,
    theme: &Theme,
) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    // Column header: "<name>    <N> msgs"
    let header_style = if focused {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtext1)
    };

    let name = session.display_label();
    let msgs = format!("{} msgs", session.message_count);
    let header = Line::from(vec![
        Span::raw(" "),
        Span::styled(name.to_string(), header_style),
        Span::styled("   ", theme.dim()),
        Span::styled(msgs, theme.muted()),
    ]);
    frame.render_widget(Paragraph::new(vec![header, Line::raw("")]), rows[0]);

    // Body: role-tinted lines.
    if preview.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled("(no readable messages)", theme.muted()),
            ])),
            rows[1],
        );
        return;
    }

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(preview.len() * 3);
    for (role, body) in preview {
        let (label, label_style) = match role {
            Role::User => (
                "you",
                Style::default().fg(theme.blue).add_modifier(Modifier::BOLD),
            ),
            Role::Claude => (
                "claude",
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        };
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(label, label_style),
            Span::raw("  "),
            Span::styled(body.clone(), theme.body()),
        ]));
        lines.push(Line::raw(""));
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .scroll((scroll_offset as u16, 0));
    frame.render_widget(p, rows[1]);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, _data: &DiffData, theme: &Theme) {
    let hints = [
        ("↑↓", "scroll"),
        ("Tab", "switch side"),
        ("s", "swap A↔B"),
        ("q", "quit"),
        ("Esc", "back"),
    ];
    let mut spans: Vec<Span> = Vec::with_capacity(hints.len() * 4);
    spans.push(Span::raw("  "));
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", theme.dim()));
        }
        spans.push(Span::styled((*key).to_string(), theme.key_hint()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled((*desc).to_string(), theme.key_desc()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::pricing::TokenCounts;
    use crate::data::session::SessionKind;
    use std::path::PathBuf;

    fn mk_session(id: &str, name: Option<&str>) -> Session {
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

    fn mk_data() -> DiffData {
        DiffData {
            session_a: mk_session("aaa", Some("auth-refactor")),
            session_b: mk_session("bbb", Some("auth-refactor-v3")),
            preview_a: vec![(Role::User, "hi".into())],
            preview_b: vec![(Role::Claude, "there".into())],
            topics_common: vec!["session".into()],
            topics_unique_a: vec!["redis".into()],
            topics_unique_b: vec!["oauth2".into()],
            scroll_offset: 0,
            focus_right: false,
        }
    }

    #[test]
    fn swap_flips_sessions_and_uniques() {
        let mut d = mk_data();
        d.swap();
        assert_eq!(d.session_a.id, "bbb");
        assert_eq!(d.session_b.id, "aaa");
        assert_eq!(d.topics_unique_a, vec!["oauth2".to_string()]);
        assert_eq!(d.topics_unique_b, vec!["redis".to_string()]);
        assert_eq!(d.topics_common, vec!["session".to_string()]);
    }

    #[test]
    fn scroll_clamps_at_zero_and_max() {
        let mut d = mk_data();
        d.scroll_by(-5, 10);
        assert_eq!(d.scroll_offset, 0);
        d.scroll_by(3, 10);
        assert_eq!(d.scroll_offset, 3);
        d.scroll_by(100, 10);
        assert_eq!(d.scroll_offset, 10);
    }

    #[test]
    fn fork_relationship_bidirectional() {
        let mut a = mk_session("a", None);
        let mut b = mk_session("b", None);
        b.forked_from = Some("a".into());
        assert!(is_fork_relationship(&a, &b));
        assert!(is_fork_relationship(&b, &a));
        a.forked_from = Some("c".into());
        b.forked_from = None;
        assert!(!is_fork_relationship(&a, &b));
    }

    #[test]
    fn center_to_max_centers_wide_area() {
        let area = Rect::new(0, 0, 200, 50);
        let centered = center_to_max(area, 140);
        assert_eq!(centered.width, 140);
        assert_eq!(centered.x, 30); // (200 - 140) / 2
        assert_eq!(centered.height, 50);
    }

    #[test]
    fn center_to_max_passthrough_small_area() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = center_to_max(area, 140);
        assert_eq!(centered.width, 100);
        assert_eq!(centered.x, 0);
    }
}
