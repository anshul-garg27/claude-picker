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
pub const MIN_HEIGHT: u16 = 23;

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
    /// True when the word-level diff renderer is active. Toggled by the `d`
    /// key in the diff screen event loop. The two-column layout collapses
    /// into a single merged stream of message pairs, each showing
    /// deletions/insertions inline (delta-style).
    pub word_mode: bool,
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

    /// Flip word-diff mode on/off.
    pub fn toggle_word_mode(&mut self) {
        self.word_mode = !self.word_mode;
        // Reset scroll so the new layout starts at the top; word-mode tends
        // to render a very different number of lines than the side-by-side
        // view.
        self.scroll_offset = 0;
    }

    /// Chunk-jump to the next / previous "hunk" — each exchange pair in
    /// the preview counts as one hunk. Mirrors delta's `n`/`N` so a user
    /// who knows that muscle memory can scan fork-diff output the same
    /// way. `dir > 0` jumps forward, `dir < 0` jumps back.
    ///
    /// The preview renders each pair as 2 rows in side-by-side mode and
    /// 3 rows in word-diff mode (header + body + spacer). Keeps those
    /// strides here so callers don't have to duplicate the math.
    pub fn jump_hunk(&mut self, dir: i32, max: usize) {
        let stride: usize = if self.word_mode { 3 } else { 2 };
        let pair_count = self.preview_a.len().max(self.preview_b.len());
        if pair_count == 0 || stride == 0 {
            return;
        }
        // Which pair index is currently nearest the top of the viewport?
        let current_pair = self.scroll_offset / stride;
        let next_pair = if dir > 0 {
            current_pair.saturating_add(1).min(pair_count.saturating_sub(1))
        } else {
            current_pair.saturating_sub(1)
        };
        let target = next_pair.saturating_mul(stride);
        self.scroll_offset = target.min(max);
    }
}

/// Return `(inserted_words, deleted_words)` when comparing `a` → `b`.
/// Separate from the renderer so it's easy to benchmark and unit-test.
///
/// Delegates to the `similar` crate (LCS / Myers-style diff) to keep the
/// word-by-word pairing correct regardless of length or edit distance.
pub fn word_diff_counts(a: &str, b: &str) -> (usize, usize) {
    let a_words: Vec<&str> = a.split_whitespace().collect();
    let b_words: Vec<&str> = b.split_whitespace().collect();
    let diff = similar::TextDiff::from_slices(&a_words, &b_words);
    let mut ins = 0;
    let mut del = 0;
    for op in diff.iter_all_changes() {
        match op.tag() {
            similar::ChangeTag::Insert => ins += 1,
            similar::ChangeTag::Delete => del += 1,
            similar::ChangeTag::Equal => {}
        }
    }
    (ins, del)
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

    // Vertical layout: cost delta strip (1), header (3 rows), common topics (2),
    // unique topics (4), rule (1), conversation preview (flex), footer (1).
    // The cost strip is the FEAT-2 addition — a one-line headline that pins
    // the money delta to the top so the user can see how much a refactor
    // (or a shorter prompt, or a model swap) saved at a glance.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // cost delta strip
            Constraint::Length(3), // title band
            Constraint::Length(3), // common topics
            Constraint::Length(5), // unique topics (two cols)
            Constraint::Length(1), // horizontal rule
            Constraint::Min(6),    // conversations
            Constraint::Length(1), // footer hint
        ])
        .split(inner);

    render_cost_delta_strip(frame, chunks[0], data, theme);
    render_title_band(frame, chunks[1], data, theme);
    render_common_topics(frame, chunks[2], data, theme);
    render_unique_topics(frame, chunks[3], data, theme);
    render_hrule(frame, chunks[4], theme);
    render_previews(frame, chunks[5], data, theme);
    render_footer(frame, chunks[6], data, theme);
}

/// Top-strip cost headline: `$76.85 vs $32.12   −57%, saved $44.73`.
///
/// The delta is the signed percentage change from A → B. When B is cheaper,
/// we tint the `−N%` and the trailing `saved $X` clause with `theme.cost_green`
/// — the refactor/prompt-compression story reads as a win at first glance.
/// When B is more expensive, the same slot shows `+N%, extra $X` in
/// `theme.cost_red`. Zero-cost diffs collapse to a neutral `$0 vs $0` line
/// so the strip never lies about equality.
fn render_cost_delta_strip(frame: &mut Frame<'_>, area: Rect, data: &DiffData, theme: &Theme) {
    let cost_a = data.session_a.total_cost_usd;
    let cost_b = data.session_b.total_cost_usd;
    let delta_usd = cost_b - cost_a;
    // Percentage change from A to B. Guard against the A = 0 edge: we can't
    // compute a percent change off a zero baseline, so fall through to
    // a neutral "new spend" label.
    let pct_spans: Vec<Span<'_>> = if cost_a > 0.0 {
        let pct = (delta_usd / cost_a) * 100.0;
        let rounded = pct.round() as i64;
        if rounded < 0 {
            // B is cheaper than A — saving.
            let abs_saved = (-delta_usd).abs();
            let style = Style::default()
                .fg(theme.cost_green)
                .add_modifier(Modifier::BOLD);
            vec![
                Span::styled(format!("\u{2212}{}%", rounded.unsigned_abs()), style),
                Span::styled(", ", theme.dim()),
                Span::styled(format!("saved ${abs_saved:.2}"), style),
            ]
        } else if rounded > 0 {
            // B is more expensive than A — overshoot.
            let style = Style::default()
                .fg(theme.cost_red)
                .add_modifier(Modifier::BOLD);
            vec![
                Span::styled(format!("+{rounded}%"), style),
                Span::styled(", ", theme.dim()),
                Span::styled(format!("extra ${delta_usd:.2}"), style),
            ]
        } else {
            // Rounded to 0 % but still non-zero deltas are possible — show
            // the signed dollar figure without a percentage.
            vec![Span::styled(
                "no change".to_string(),
                theme.muted(),
            )]
        }
    } else if cost_b > 0.0 {
        // A had no cost, B has some — percent is undefined.
        vec![Span::styled(
            format!("+${cost_b:.2} (no baseline)"),
            Style::default()
                .fg(theme.cost_red)
                .add_modifier(Modifier::BOLD),
        )]
    } else {
        vec![Span::styled("no change".to_string(), theme.muted())]
    };

    let mut spans = vec![
        Span::raw("  "),
        Span::styled(
            format!("${cost_a:.2}"),
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  vs  ", theme.dim()),
        Span::styled(
            format!("${cost_b:.2}"),
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", theme.dim()),
    ];
    spans.extend(pct_spans);
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
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
    if data.word_mode {
        render_word_diff(frame, area, data, theme);
        return;
    }

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

/// Render the word-level diff stream. For each corresponding message pair
/// between A and B (same position in the preview ring), emit a labelled
/// line showing B's body with insertions in green-bold and deletions from A
/// in red with a strikethrough-feel modifier. Delta-style.
fn render_word_diff(frame: &mut Frame<'_>, area: Rect, data: &DiffData, theme: &Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let name_a = data.session_a.display_label();
    let name_b = data.session_b.display_label();
    let header = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            name_a.to_string(),
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  →  ", theme.dim()),
        Span::styled(
            name_b.to_string(),
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   —   ", theme.dim()),
        Span::styled(
            "word diff",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(vec![header, Line::raw("")]), rows[0]);

    let pair_count = data.preview_a.len().max(data.preview_b.len());
    if pair_count == 0 {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("(no exchanges to diff)", theme.muted()),
            ])),
            rows[1],
        );
        return;
    }

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(pair_count * 4);
    for i in 0..pair_count {
        let a = data.preview_a.get(i);
        let b = data.preview_b.get(i);
        match (a, b) {
            (Some((role_a, body_a)), Some((_role_b, body_b))) => {
                let (label, label_style) = word_diff_role_label(role_a, theme);
                let (ins, del) = word_diff_counts(body_a, body_b);
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(label, label_style),
                    Span::raw("  "),
                    Span::styled(format!("+{ins} / -{del} words"), theme.muted()),
                ]));
                let spans = render_word_diff_spans(body_a, body_b, theme);
                lines.push(Line::from(
                    std::iter::once(Span::raw("    "))
                        .chain(spans)
                        .collect::<Vec<_>>(),
                ));
                lines.push(Line::raw(""));
            }
            (Some((role, body)), None) => {
                let (label, label_style) = word_diff_role_label(role, theme);
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(label, label_style),
                    Span::raw("  "),
                    Span::styled("only in A — deleted", Style::default().fg(theme.red)),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        body.clone(),
                        Style::default()
                            .fg(theme.red)
                            .add_modifier(Modifier::CROSSED_OUT),
                    ),
                ]));
                lines.push(Line::raw(""));
            }
            (None, Some((role, body))) => {
                let (label, label_style) = word_diff_role_label(role, theme);
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(label, label_style),
                    Span::raw("  "),
                    Span::styled("only in B — added", Style::default().fg(theme.green)),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        body.clone(),
                        Style::default()
                            .fg(theme.green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                lines.push(Line::raw(""));
            }
            (None, None) => {}
        }
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((data.scroll_offset as u16, 0));
    frame.render_widget(p, rows[1]);
}

fn word_diff_role_label(role: &Role, theme: &Theme) -> (&'static str, Style) {
    match role {
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
    }
}

/// Walk a word-level LCS diff between two strings and render the result as
/// a flat stream of styled spans. Insertions are bold green, deletions are
/// red with the crossed-out modifier, unchanged words are muted. Matches
/// the delta visual style (without copying delta's code).
fn render_word_diff_spans<'a>(a: &'a str, b: &'a str, theme: &Theme) -> Vec<Span<'a>> {
    let a_words: Vec<&str> = a.split_whitespace().collect();
    let b_words: Vec<&str> = b.split_whitespace().collect();
    let diff = similar::TextDiff::from_slices(&a_words, &b_words);

    let mut out: Vec<Span<'_>> = Vec::with_capacity(a_words.len() + b_words.len());
    let mut first = true;
    for change in diff.iter_all_changes() {
        let word = change.value();
        if word.is_empty() {
            continue;
        }
        if !first {
            out.push(Span::raw(" "));
        }
        first = false;

        let style = match change.tag() {
            similar::ChangeTag::Equal => theme.muted(),
            similar::ChangeTag::Insert => Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
            similar::ChangeTag::Delete => Style::default()
                .fg(theme.red)
                .add_modifier(Modifier::CROSSED_OUT),
        };
        out.push(Span::styled(word.to_string(), style));
    }
    out
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

fn render_footer(frame: &mut Frame<'_>, area: Rect, data: &DiffData, theme: &Theme) {
    let d_hint = if data.word_mode {
        ("d", "side-by-side")
    } else {
        ("d", "word diff")
    };
    let hints = [
        ("↑↓", "scroll"),
        ("Tab", "switch side"),
        ("s", "swap A↔B"),
        d_hint,
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
            word_mode: false,
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

    #[test]
    fn toggle_word_mode_flips_flag_and_resets_scroll() {
        let mut d = mk_data();
        d.scroll_offset = 7;
        assert!(!d.word_mode);
        d.toggle_word_mode();
        assert!(d.word_mode);
        assert_eq!(d.scroll_offset, 0);
        d.toggle_word_mode();
        assert!(!d.word_mode);
    }

    #[test]
    fn word_diff_counts_pure_insert() {
        let (ins, del) = word_diff_counts("hello world", "hello brave new world");
        assert_eq!(del, 0);
        assert_eq!(ins, 2);
    }

    #[test]
    fn word_diff_counts_pure_delete() {
        let (ins, del) = word_diff_counts("one two three four", "one four");
        assert_eq!(ins, 0);
        assert_eq!(del, 2);
    }

    #[test]
    fn word_diff_counts_mixed_insert_and_delete() {
        let (ins, del) = word_diff_counts("the cat sat on the mat", "the dog sat under the mat");
        assert_eq!(ins, 2);
        assert_eq!(del, 2);
    }

    #[test]
    fn word_diff_counts_identical_strings_yield_zeros() {
        let (ins, del) = word_diff_counts(
            "the auth middleware is storing tokens",
            "the auth middleware is storing tokens",
        );
        assert_eq!(ins, 0);
        assert_eq!(del, 0);
    }

    #[test]
    fn word_diff_counts_empty_strings() {
        let (ins, del) = word_diff_counts("", "");
        assert_eq!((ins, del), (0, 0));
    }

    #[test]
    fn word_diff_counts_single_word_swap() {
        let (ins, del) = word_diff_counts("fix bug", "fix crash");
        assert_eq!(ins, 1);
        assert_eq!(del, 1);
    }

    #[test]
    fn jump_hunk_advances_by_pair_stride() {
        let mut d = mk_data();
        d.preview_a = vec![
            (Role::User, "q1".into()),
            (Role::Claude, "a1".into()),
            (Role::User, "q2".into()),
        ];
        d.preview_b = vec![
            (Role::User, "q1".into()),
            (Role::Claude, "a1".into()),
            (Role::User, "q2".into()),
        ];
        d.scroll_offset = 0;
        d.jump_hunk(1, 100);
        assert_eq!(d.scroll_offset, 2, "stride = 2 in side-by-side mode");
        d.jump_hunk(1, 100);
        assert_eq!(d.scroll_offset, 4);
        d.jump_hunk(-1, 100);
        assert_eq!(d.scroll_offset, 2);
    }

    #[test]
    fn jump_hunk_word_mode_stride_is_three() {
        let mut d = mk_data();
        d.word_mode = true;
        d.preview_a = vec![
            (Role::User, "q1".into()),
            (Role::Claude, "a1".into()),
        ];
        d.preview_b = vec![
            (Role::User, "q1".into()),
            (Role::Claude, "a1".into()),
        ];
        d.scroll_offset = 0;
        d.jump_hunk(1, 100);
        assert_eq!(d.scroll_offset, 3, "stride = 3 in word-diff mode");
    }

    #[test]
    fn word_diff_counts_handles_long_bodies() {
        // Perf sanity check — a 1000-word body should diff quickly and
        // return exact counts (each 10th word changes).
        let a: String = (0..1000)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let b: String = (0..1000)
            .map(|i| {
                if i % 10 == 0 {
                    format!("changed{i}")
                } else {
                    format!("word{i}")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let (ins, del) = word_diff_counts(&a, &b);
        assert_eq!(ins, 100);
        assert_eq!(del, 100);
    }
}
