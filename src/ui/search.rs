//! `claude-picker search` — full-text search screen.
//!
//! Renders a single-pane (or split, with preview) Ratatui view: a filter input
//! at the top, a scoreboard-ordered list of matches below, and an optional
//! preview pane on the right. The match list shows two lines per result:
//!
//! 1. `▸ project / session-name` (pointer only on cursor row).
//! 2. An 80-char snippet window around the first occurrence of the query, with
//!    the hit highlighted in yellow.
//!
//! All scoring + haystack prep is done by the command handler; this module
//! only knows how to draw state. The split keeps the rendering logic testable
//! without needing to stand up a temp home dir full of JSONL files.
//!
//! See [`super::search`](../index.html) wiring and
//! [`crate::commands::search_cmd`] for the event loop.

use std::path::PathBuf;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::text::{display_width, truncate_to_width};

/// Width threshold above which the preview pane can eat a larger slice.
const WIDE_THRESHOLD: u16 = 130;

/// Maximum rows of matches we render. Arbitrary but big enough to feel like
/// "all the hits": at 2 rows per match that's 20 visible hits on a 40-row
/// terminal after the header + footer.
const MAX_VISIBLE: usize = 50;

/// A single scored hit. Assembled by the command handler; the UI module is
/// pure data-in.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub session_id: String,
    pub project_name: String,
    pub project_cwd: PathBuf,
    /// What the user sees — usually `Session::display_label()`.
    pub session_name: String,
    /// ~80-char context window around the first hit inside the session body.
    pub snippet: String,
    /// Nucleo score; higher is better.
    pub score: u32,
}

/// Mutable view state driven by the event loop.
pub struct SearchState {
    pub query: String,
    /// Full universe of possible hits — one per session. Filtering picks a
    /// subset and ranks it.
    pub all_matches: Vec<SearchMatch>,
    /// Indices into `all_matches`, in render order.
    pub filtered_indices: Vec<usize>,
    pub cursor: usize,
    pub preview_visible: bool,
    /// True while the background loader is still populating `all_matches`.
    /// Drives the "Loading sessions…" placeholder.
    pub loading: bool,
    /// `?` help overlay visible.
    pub show_help: bool,
    /// One-shot status message shown in a toast — set by clipboard / editor
    /// shortcuts. Cleared by the event loop's tick.
    pub toast: Option<(String, ToastKind, std::time::Instant)>,
    /// Space-leader command palette. `Some` while open; takes input
    /// priority over the underlying search list.
    pub palette: Option<crate::ui::command_palette::CommandPalette>,
    /// Full-screen conversation viewer — `Some` while reading a transcript.
    pub viewer: Option<crate::ui::conversation_viewer::ViewerState>,
    /// Chip labels for filters parsed from the current query. Empty when
    /// the query has no filter operators. Rendered as pill-like chips on
    /// the `active:` row above the match list.
    pub active_filters: Vec<String>,
}

/// Local toast kind — kept lightweight so the UI module stays self-contained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            all_matches: Vec::new(),
            filtered_indices: Vec::new(),
            cursor: 0,
            preview_visible: false,
            loading: true,
            show_help: false,
            toast: None,
            palette: None,
            viewer: None,
            active_filters: Vec::new(),
        }
    }

    /// Set a toast that lives for 1.5s. Called from the command handler.
    pub fn set_toast(&mut self, message: impl Into<String>, kind: ToastKind) {
        self.toast = Some((
            message.into(),
            kind,
            std::time::Instant::now() + std::time::Duration::from_millis(1500),
        ));
    }

    /// Drop any toast that has lived past its expiry. Call once per frame.
    pub fn tick(&mut self) {
        if let Some((_, _, expires)) = &self.toast {
            if std::time::Instant::now() >= *expires {
                self.toast = None;
            }
        }
    }

    /// Current cursor, clamped so we never look past the filtered list.
    pub fn cursor_clamped(&self) -> usize {
        if self.filtered_indices.is_empty() {
            0
        } else {
            self.cursor
                .min(self.filtered_indices.len().saturating_sub(1))
        }
    }

    /// The match under the cursor (if any).
    pub fn selected_match(&self) -> Option<&SearchMatch> {
        let idx = *self.filtered_indices.get(self.cursor_clamped())?;
        self.all_matches.get(idx)
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

/// Top-level render entry. Lays out header / counter / list / optional
/// preview, and dispatches to the per-section helpers. Takes `&mut state`
/// because the conversation viewer caches flattened lines on its state
/// during render, and that cache rebuilds on width / search changes.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &mut SearchState, theme: &Theme) {
    // Viewer takes over the whole frame when open.
    if let Some(viewer) = state.viewer.as_mut() {
        crate::ui::conversation_viewer::render(frame, area, viewer, theme);
        if let Some((msg, kind, _)) = &state.toast {
            render_toast(frame, area, msg, *kind, theme);
        }
        return;
    }

    // Split horizontally if the preview pane is open; otherwise use the full
    // area for the list column.
    let (list_area, preview_area) = if state.preview_visible && area.width >= 90 {
        // 60/40 on normal terminals, 65/35 on very wide so the list stays
        // readable. A cap at 140 cols matches the brief.
        let (left, right) = if area.width >= WIDE_THRESHOLD {
            (65u16, 35u16)
        } else {
            (60u16, 40u16)
        };
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(left), Constraint::Percentage(right)])
            .split(area);
        (cols[0], Some(cols[1]))
    } else {
        (area, None)
    };

    render_list_column(frame, list_area, state, theme);
    if let Some(pv) = preview_area {
        render_preview_column(frame, pv, state, theme);
    }

    if let Some((msg, kind, _)) = &state.toast {
        render_toast(frame, area, msg, *kind, theme);
    }
    if let Some(palette) = &state.palette {
        crate::ui::command_palette::render(frame, area, palette, theme);
    }
    if state.show_help {
        let content = crate::ui::help_overlay::help_for(crate::ui::help_overlay::Screen::Search);
        crate::ui::help_overlay::render(frame, area, content, theme);
    }
}

/// Centred toast, sibling to the one in `ui::picker`. Duplicated rather than
/// moved because toasting is self-contained here — the search state owns its
/// own local toast type.
fn render_toast(frame: &mut Frame<'_>, area: Rect, msg: &str, kind: ToastKind, theme: &Theme) {
    use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
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

fn render_list_column(frame: &mut Frame<'_>, area: Rect, state: &SearchState, theme: &Theme) {
    // Outer rounded border with a titled header ("claude-picker · search")
    // and a right-aligned "N matches" counter.
    let count_label = if state.loading {
        "loading…".to_string()
    } else if state.query.is_empty() {
        format!("{} sessions", state.all_matches.len())
    } else {
        format!("{} matches", state.filtered_indices.len())
    };

    let title_left = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "claude-picker",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", theme.dim()),
        Span::styled("search", theme.subtle()),
        Span::raw(" "),
    ]);
    let title_right = Line::from(vec![
        Span::raw(" "),
        Span::styled(count_label, theme.muted()),
        Span::raw(" "),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border_active())
        .title(title_left)
        .title(title_right.alignment(Alignment::Right));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical layout inside the outer border:
    //   1 line: blank breathing room
    //   1 line: query input (" > foo_ ")
    //   1 line: blank breathing room
    //   flex:   list / empty state
    //   1 line: keybind footer
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // pad
            Constraint::Length(1), // query input
            Constraint::Length(1), // pad
            Constraint::Min(3),    // list body
            Constraint::Length(1), // footer
        ])
        .split(inner);

    render_query_input(frame, rows[1], state, theme);
    // The "breathing room" line (rows[2]) doubles as the active-filter
    // chip bar when any filters are parsed out of the current query.
    if !state.active_filters.is_empty() {
        render_filter_chips(frame, rows[2], &state.active_filters, theme);
    }
    render_body(frame, rows[3], state, theme);
    render_footer(frame, rows[4], theme);
}

/// Render the chip-bar underneath the query input:
///
///   `  active: [bookmarked] [@opus] [#week] [$>1]`
///
/// Chips get a type-colored accent so the filter kind reads at a glance.
fn render_filter_chips(frame: &mut Frame<'_>, area: Rect, chips: &[String], theme: &Theme) {
    let mut spans: Vec<Span<'_>> = Vec::with_capacity(chips.len() * 3 + 2);
    spans.push(Span::raw("  "));
    spans.push(Span::styled("active:", theme.muted()));
    for chip in chips {
        spans.push(Span::raw(" "));
        // Accent colour by chip prefix — `@`=blue (model/mode),
        // `#`=green (time), `$`/quantities=yellow, bang=mauve.
        let accent = if chip.starts_with('@') {
            theme.blue
        } else if chip.starts_with('#') {
            theme.green
        } else if chip.starts_with('$')
            || chip.starts_with("tokens")
            || chip.starts_with("msgs")
            || chip.starts_with('<')
        {
            theme.yellow
        } else {
            theme.mauve
        };
        spans.push(Span::styled(
            format!("[{chip}]"),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_query_input(frame: &mut Frame<'_>, area: Rect, state: &SearchState, theme: &Theme) {
    let mut spans = vec![
        Span::raw("  "),
        Span::styled(
            ">",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];
    if state.query.is_empty() {
        spans.push(Span::styled(
            "search across every session…",
            theme.filter_placeholder(),
        ));
    } else {
        spans.push(Span::styled(state.query.clone(), theme.filter_text()));
    }
    // Block cursor as a dim mauve underscore so the user can tell the input
    // has focus even when the buffer is empty.
    spans.push(Span::styled(
        "_",
        Style::default().fg(theme.mauve).add_modifier(Modifier::DIM),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_body(frame: &mut Frame<'_>, area: Rect, state: &SearchState, theme: &Theme) {
    // Loading / empty-state pathways first.
    if state.loading {
        render_centered(
            frame,
            area,
            &[
                ("Loading sessions…", theme.muted()),
                (
                    "(scanning every JSONL under ~/.claude/projects)",
                    theme.dim(),
                ),
            ],
        );
        return;
    }
    if state.query.is_empty() {
        render_centered(
            frame,
            area,
            &[
                (
                    "Type to search across every Claude Code conversation.",
                    theme.muted(),
                ),
                (
                    "Matches bodies of user and assistant messages.",
                    theme.dim(),
                ),
            ],
        );
        return;
    }
    if state.filtered_indices.is_empty() {
        let msg = format!("No matches for \"{}\".", state.query);
        render_centered(
            frame,
            area,
            &[
                (msg.as_str(), theme.muted()),
                ("Try a different query.", theme.dim()),
            ],
        );
        return;
    }

    // Real list render.
    let rows_needed = area.height as usize / 3; // title+snippet+spacer = 3 lines per hit
    let visible = state
        .filtered_indices
        .len()
        .min(MAX_VISIBLE)
        .min(rows_needed.max(1));

    // Simple viewport scroll — keep the cursor in view. Since MAX_VISIBLE is
    // already small and each row is 3 lines, we do a minimal top-of-window
    // computation instead of full virtualization.
    let cursor = state.cursor_clamped();
    let window_start = cursor.saturating_sub(visible.saturating_sub(1));
    let window_end = (window_start + visible).min(state.filtered_indices.len());

    let mut lines: Vec<Line<'_>> = Vec::with_capacity((window_end - window_start) * 3);
    for (display_i, global_i) in (window_start..window_end).enumerate() {
        let idx = state.filtered_indices[global_i];
        let Some(m) = state.all_matches.get(idx) else {
            continue;
        };
        let is_cursor = global_i == cursor;
        lines.push(build_title_line(m, is_cursor, theme));
        lines.push(build_snippet_line(m, &state.query, theme));
        // Blank spacer between entries — gives the 2-line-per-hit feel in the
        // mockup without drawing separators.
        if display_i + 1 < window_end - window_start {
            lines.push(Line::raw(""));
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn build_title_line<'a>(m: &'a SearchMatch, is_cursor: bool, theme: &Theme) -> Line<'a> {
    let pointer = if is_cursor {
        Span::styled(
            "  ▸ ",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("    ")
    };

    let name_style = if is_cursor {
        Style::default()
            .fg(theme.green)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default()
            .fg(theme.green)
            .add_modifier(Modifier::BOLD)
    };

    Line::from(vec![
        pointer,
        Span::styled(m.project_name.clone(), Style::default().fg(theme.blue)),
        Span::styled(" / ", theme.dim()),
        Span::styled(m.session_name.clone(), name_style),
    ])
}

fn build_snippet_line<'a>(m: &'a SearchMatch, query: &str, theme: &Theme) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(6);
    spans.push(Span::raw("      ")); // 6 spaces of indent (4 past the pointer column)

    // Highlight every case-insensitive occurrence of `query` (or its longest
    // word if query is multi-word).
    let needle = dominant_word(query);
    if needle.is_empty() {
        spans.push(Span::styled(m.snippet.clone(), theme.body()));
        return Line::from(spans);
    }

    let snippet = &m.snippet;
    let hay_lower = snippet.to_lowercase();
    let needle_lower = needle.to_lowercase();

    let mut cursor = 0usize;
    while cursor < snippet.len() {
        // Find the next match of `needle` starting at `cursor`.
        let remaining = &hay_lower[cursor..];
        match remaining.find(&needle_lower) {
            Some(rel) => {
                let start = cursor + rel;
                let end = start + needle.len();
                if start > cursor {
                    spans.push(Span::styled(
                        snippet[cursor..start].to_string(),
                        theme.body(),
                    ));
                }
                // Guard against mid-codepoint cuts: if indices don't fall on
                // char boundaries, bail out and emit the rest as plain text.
                if !snippet.is_char_boundary(start) || !snippet.is_char_boundary(end) {
                    spans.push(Span::styled(snippet[cursor..].to_string(), theme.body()));
                    return Line::from(spans);
                }
                spans.push(Span::styled(
                    snippet[start..end].to_string(),
                    Style::default()
                        .fg(theme.yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                cursor = end;
            }
            None => {
                spans.push(Span::styled(snippet[cursor..].to_string(), theme.body()));
                break;
            }
        }
    }

    Line::from(spans)
}

fn render_centered(frame: &mut Frame<'_>, area: Rect, items: &[(&str, Style)]) {
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(items.len() + 2);
    lines.push(Line::raw(""));
    for (text, style) in items {
        lines.push(Line::styled((*text).to_string(), *style));
    }
    let p = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(p, area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let spans = vec![
        Span::raw(" "),
        Span::styled("↑↓", theme.key_hint()),
        Span::raw(" "),
        Span::styled("navigate", theme.key_desc()),
        Span::styled("  ·  ", theme.dim()),
        Span::styled("Enter", theme.key_hint()),
        Span::raw(" "),
        Span::styled("resume", theme.key_desc()),
        Span::styled("  ·  ", theme.dim()),
        Span::styled("p", theme.key_hint()),
        Span::raw(" "),
        Span::styled("preview", theme.key_desc()),
        Span::styled("  ·  ", theme.dim()),
        Span::styled("?", theme.key_hint()),
        Span::raw(" "),
        Span::styled("help", theme.key_desc()),
        Span::styled("  ·  ", theme.dim()),
        Span::styled("q", theme.key_hint()),
        Span::raw(" "),
        Span::styled("quit", theme.key_desc()),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_preview_column(frame: &mut Frame<'_>, area: Rect, state: &SearchState, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "preview",
                Style::default()
                    .fg(theme.overlay0)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(m) = state.selected_match() else {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("  (select a match to preview)", theme.muted()),
        ]);
        frame.render_widget(p, inner);
        return;
    };

    // Reuse the exchange loader from ui::preview — same file layout, same
    // rules for noise filtering. We don't pass an `App`, so pull the subset
    // we need directly via the helper exposed below.
    let lines = render_preview_lines(m, theme);
    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(p, inner);
}

/// Build a list of `Line`s summarising the cursor session. Self-contained so
/// the preview pane can render without an `App` to hand off to
/// `ui::preview::render`.
fn render_preview_lines<'a>(m: &'a SearchMatch, theme: &Theme) -> Vec<Line<'a>> {
    use serde::Deserialize;
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    #[derive(Deserialize)]
    struct Raw {
        #[serde(default, rename = "type")]
        kind: Option<String>,
        #[serde(default)]
        message: Option<RawMsg>,
    }
    #[derive(Deserialize)]
    struct RawMsg {
        #[serde(default)]
        role: Option<String>,
        #[serde(default)]
        content: Option<RawContent>,
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RawContent {
        Text(String),
        Blocks(Vec<RawBlock>),
    }
    #[derive(Deserialize)]
    struct RawBlock {
        #[serde(default, rename = "type")]
        kind: Option<String>,
        #[serde(default)]
        text: Option<String>,
    }

    let Some(path) = jsonl_path_for_session(&m.session_id) else {
        return vec![
            Line::raw(""),
            Line::styled("  (preview unavailable)", theme.muted()),
        ];
    };
    let Ok(file) = File::open(&path) else {
        return vec![
            Line::raw(""),
            Line::styled("  (preview unavailable)", theme.muted()),
        ];
    };

    let mut header = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(m.project_name.clone(), Style::default().fg(theme.blue)),
            Span::styled(" / ", theme.dim()),
            Span::styled(
                m.session_name.clone(),
                Style::default()
                    .fg(theme.green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("id {}", &m.session_id[..8.min(m.session_id.len())]),
                theme.muted(),
            ),
        ]),
        Line::raw(""),
    ];

    let mut ring: Vec<(String, String)> = Vec::with_capacity(6);
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(raw) = serde_json::from_str::<Raw>(trimmed) else {
            continue;
        };
        let Some(kind) = raw.kind.as_deref() else {
            continue;
        };
        let Some(msg) = raw.message else { continue };
        let role = match (kind, msg.role.as_deref()) {
            ("user", Some("user")) => "user",
            ("assistant", Some("assistant")) => "claude",
            _ => continue,
        };
        let body = match msg.content {
            Some(RawContent::Text(s)) => s,
            Some(RawContent::Blocks(blocks)) => blocks
                .into_iter()
                .find_map(|b| {
                    if b.kind.as_deref() == Some("text") {
                        b.text
                    } else {
                        None
                    }
                })
                .unwrap_or_default(),
            None => String::new(),
        };
        let body = body.trim().to_string();
        // Column-aware short-body filter — `はい` (2 cols) still reads as
        // noise; 4 cols is about "one CJK word" or "four ASCII chars".
        if body.is_empty() || display_width(&body) < 4 {
            continue;
        }
        if crate::data::session::noise_prefixes()
            .iter()
            .any(|n| body.contains(n))
        {
            continue;
        }
        let flat: String = body
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect();
        // Truncate by column count — mirrors the preview module's MAX_BODY_COLS
        // convention so a bilingual transcript has uniform visual length.
        let truncated = truncate_to_width(&flat, 200);
        if ring.len() == 6 {
            ring.remove(0);
        }
        ring.push((role.to_string(), truncated));
    }

    if ring.is_empty() {
        header.push(Line::styled("  (no readable messages)", theme.muted()));
        return header;
    }
    for (role, body) in ring {
        let (label, lstyle) = if role == "user" {
            (
                "user",
                Style::default().fg(theme.blue).add_modifier(Modifier::BOLD),
            )
        } else {
            (
                "claude",
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            )
        };
        header.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(label, lstyle),
            Span::raw("  "),
            Span::styled(body, theme.body()),
        ]));
        header.push(Line::raw(""));
    }
    header
}

fn jsonl_path_for_session(id: &str) -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let projects = home.join(".claude").join("projects");
    if !projects.is_dir() {
        return None;
    }
    for entry in std::fs::read_dir(&projects).ok()?.flatten() {
        let candidate = entry.path().join(format!("{id}.jsonl"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

// ── Snippet extraction (public helper; the command handler calls it too) ──

/// Return the longest word in `query`, or the whole query if it's one word.
///
/// Used both for snippet extraction (find the first occurrence of *this*
/// substring) and for the in-snippet highlight pass. Multi-word queries
/// highlight the word that's most likely to be the "load-bearing" one.
pub fn dominant_word(query: &str) -> String {
    let mut best = "";
    for w in query.split_whitespace() {
        if w.chars().count() > best.chars().count() {
            best = w;
        }
    }
    if best.is_empty() {
        query.trim().to_string()
    } else {
        best.to_string()
    }
}

/// Carve an ~80-char window around the first case-insensitive occurrence of
/// `needle` inside `body`. Falls back to the head of `body` if the needle is
/// absent. Newlines collapse to spaces; ellipses mark truncation on either
/// side.
pub fn extract_snippet(body: &str, needle: &str) -> String {
    const AROUND: usize = 40;
    const MAX_LEN: usize = 80;

    // Flatten newlines first so the window doesn't contain hard breaks.
    let flat: String = body
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let flat = flat.trim();
    if flat.is_empty() {
        return String::new();
    }

    let needle_trim = needle.trim();
    if needle_trim.is_empty() {
        return window_from_head(flat, MAX_LEN);
    }

    let flat_lower = flat.to_lowercase();
    let needle_lower = needle_trim.to_lowercase();

    // Find the first occurrence, measured in chars (not bytes) so the window
    // arithmetic is Unicode-safe.
    let Some(byte_pos) = flat_lower.find(&needle_lower) else {
        return window_from_head(flat, MAX_LEN);
    };
    // Convert byte index → char index by counting chars up to `byte_pos`.
    let char_pos = flat[..byte_pos].chars().count();
    let total_chars = flat.chars().count();

    let start_char = char_pos.saturating_sub(AROUND);
    let needle_chars = needle_trim.chars().count();
    let end_char = (char_pos + needle_chars + AROUND).min(total_chars);

    // Slice by char index, not byte index.
    let mut slice = String::with_capacity(MAX_LEN * 4);
    for (i, ch) in flat.chars().enumerate() {
        if i < start_char {
            continue;
        }
        if i >= end_char {
            break;
        }
        slice.push(ch);
    }

    // Add ellipses. Always add both sides when the slice was cut.
    let mut out = String::with_capacity(slice.len() + 6);
    if start_char > 0 {
        out.push('…');
    }
    // Trim whitespace at the edges so ellipses don't read as " … ".
    out.push_str(slice.trim());
    if end_char < total_chars {
        out.push('…');
    }

    // If the result is still longer than MAX_LEN + ellipses slack, hard-cap
    // it. Keeps a pathological no-space haystack from blowing up the row.
    enforce_max_chars(&out, MAX_LEN + 2)
}

fn window_from_head(flat: &str, max_cols: usize) -> String {
    // Measured in display columns so CJK / emoji snippets don't blow past
    // the width budget.
    truncate_to_width(flat, max_cols)
}

fn enforce_max_chars(s: &str, max_cols: usize) -> String {
    // Measured in display columns — matches `window_from_head` so the final
    // row stays under the column cap on any terminal width.
    truncate_to_width(s, max_cols)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_word_picks_longest() {
        assert_eq!(dominant_word("the quick brown"), "quick");
        assert_eq!(dominant_word("single"), "single");
        assert_eq!(dominant_word(""), "");
        assert_eq!(dominant_word("   padded  "), "padded");
    }

    #[test]
    fn extract_snippet_centers_on_first_hit_with_ellipses() {
        let body = "a".repeat(100) + " race condition " + &"b".repeat(100);
        let snippet = extract_snippet(&body, "race");
        assert!(snippet.contains("race"));
        assert!(
            snippet.starts_with('…'),
            "expected leading ellipsis: {snippet}"
        );
        assert!(
            snippet.ends_with('…'),
            "expected trailing ellipsis: {snippet}"
        );
        // The window is ~80 chars + ellipses; let a little slack for trim+pad.
        assert!(
            snippet.chars().count() <= 90,
            "snippet too long: {} chars: {}",
            snippet.chars().count(),
            snippet
        );
    }

    #[test]
    fn extract_snippet_is_case_insensitive() {
        let body = "before RACE CONDITION after";
        let s = extract_snippet(body, "race");
        assert!(s.contains("RACE"), "got: {s}");
    }

    #[test]
    fn extract_snippet_flattens_newlines() {
        let body = "first line\nsecond line with needle here\nthird line";
        let s = extract_snippet(body, "needle");
        assert!(!s.contains('\n'));
        assert!(s.contains("needle"));
    }

    #[test]
    fn extract_snippet_fallback_when_needle_missing() {
        let body = "hello world this does not contain the search term";
        let s = extract_snippet(body, "missing");
        assert!(!s.is_empty());
        // Falls back to head-of-body.
        assert!(s.starts_with("hello"));
    }

    #[test]
    fn extract_snippet_short_body_returns_whole_thing() {
        let body = "tiny race";
        let s = extract_snippet(body, "race");
        assert_eq!(s, "tiny race");
    }
}
