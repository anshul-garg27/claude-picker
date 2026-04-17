//! Bottom key-hint bar.
//!
//! Rendered as a single line with key/description pairs separated by a
//! mid-dot. The hints shown depend on the current screen — we expose two
//! render paths (`session_list_hints` and `project_list_hints`) to keep the
//! set appropriate rather than showing irrelevant keys.
//!
//! The bar never draws a border; it lives inside the outer frame. That keeps
//! the vertical budget free for the list itself.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::Theme;

/// Separator between hints. A middle dot reads lighter than "|" and doesn't
/// visually collide with the `▸` cursor glyph on the list.
const SEP: &str = "  ·  ";

/// Render the footer for the session-list screen. When in multi-select
/// mode this renders the context-aware batch-action hints instead.
pub fn render_session_list(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    render_session_list_with_multi(f, area, theme, 0, false, None, None);
}

/// Session-list footer that knows about multi-select mode. `count` is the
/// number of sessions currently multi-selected; when `multi_mode` is true
/// and count > 0 we render the batch-action hints.
///
/// `pending_count` — Agent A's vim-style repeat prefix; when `Some(n)` we
/// append a right-aligned `⣿ n` indicator so the user sees their count
/// accumulating.
///
/// `jump_ring` — `(current_index_1_based, total)` if the jump ring has any
/// entries; we print `⎈ jumps: [i/n]` so the user can see where Ctrl-o/Ctrl-i
/// will land them next.
pub fn render_session_list_with_multi(
    f: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    count: usize,
    multi_mode: bool,
    pending_count: Option<u32>,
    jump_ring: Option<(usize, usize)>,
) {
    if multi_mode && count > 0 {
        render_multi_hints(f, area, theme, count);
        return;
    }
    let hints = [
        ("↑↓", "navigate"),
        ("Enter", "resume"),
        ("v", "view"),
        ("Tab", "multi"),
        ("Ctrl-r", "scope"),
        ("1-9", "pin"),
        ("?", "help"),
        ("q", "quit"),
    ];
    render_hints_with_status(f, area, theme, &hints, pending_count, jump_ring);
}

/// Special-case footer shown while the user has a live multi-selection.
/// Calls out the batch actions so they're discoverable without opening
/// the help overlay.
fn render_multi_hints(f: &mut Frame<'_>, area: Rect, theme: &Theme, count: usize) {
    let dim = theme.dim();
    let spans = vec![
        Span::raw("  "),
        Span::styled(
            format!("{count} selected"),
            Style::default()
                .fg(theme.peach)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(SEP, dim),
        Span::styled("Tab", theme.key_hint()),
        Span::raw(" "),
        Span::styled("toggle", theme.key_desc()),
        Span::styled(SEP, dim),
        Span::styled("Esc", theme.key_hint()),
        Span::raw(" "),
        Span::styled("clear", theme.key_desc()),
        Span::styled(SEP, dim),
        Span::styled("Ctrl+E", theme.key_hint()),
        Span::raw(" "),
        Span::styled("export all", theme.key_desc()),
        Span::styled(SEP, dim),
        Span::styled("Ctrl+D", theme.key_hint()),
        Span::raw(" "),
        Span::styled("delete all", theme.key_desc()),
        Span::styled(SEP, dim),
        Span::styled("y", theme.key_hint()),
        Span::raw(" "),
        Span::styled("copy ids", theme.key_desc()),
    ];
    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}

/// Render the footer for the project-list screen.
pub fn render_project_list(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let hints = [
        ("↑↓", "navigate"),
        ("Enter", "open"),
        ("u", "pin"),
        ("1-9", "jump"),
        ("0", "all"),
        ("?", "help"),
        ("q", "quit"),
    ];
    render_hints(f, area, theme, &hints);
}

fn render_hints(f: &mut Frame<'_>, area: Rect, theme: &Theme, hints: &[(&str, &str)]) {
    render_hints_with_status(f, area, theme, hints, None, None);
}

/// Variant of [`render_hints`] that appends right-aligned status indicators
/// for the vim-style count prefix and the jump ring. Both indicators are
/// `Option` so callers that don't need them (project-list footer) can pass
/// `None` without padding concerns. Indicators fall off the right edge first
/// when the terminal is too narrow for everything.
fn render_hints_with_status(
    f: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    hints: &[(&str, &str)],
    pending_count: Option<u32>,
    jump_ring: Option<(usize, usize)>,
) {
    let mut spans: Vec<Span> = Vec::with_capacity(hints.len() * 4);
    spans.push(Span::raw("  "));
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(SEP, theme.dim()));
        }
        spans.push(Span::styled((*key).to_string(), theme.key_hint()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled((*desc).to_string(), theme.key_desc()));
    }

    // Right-aligned indicators. Use a filler span so the status drifts to
    // the right edge of the pane. Both glyphs are from the Braille / nav
    // Unicode ranges so they read without needing a Nerd Font.
    let mut status: Vec<Span> = Vec::new();
    if let Some(n) = pending_count {
        status.push(Span::styled(SEP, theme.dim()));
        status.push(Span::styled(
            format!("\u{28FF} {n}"),
            Style::default()
                .fg(theme.peach)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let Some((pos, total)) = jump_ring {
        status.push(Span::styled(SEP, theme.dim()));
        status.push(Span::styled(
            format!("\u{2388} jumps: [{pos}/{total}]"),
            theme.key_desc(),
        ));
    }

    // Only reserve space for the status on the right side when there IS
    // status. A single Paragraph line with spans lays out left-to-right, so
    // we approximate right-alignment by padding between hints and status.
    if status.is_empty() {
        let p = Paragraph::new(Line::from(spans));
        f.render_widget(p, area);
        return;
    }

    // Measure the already-placed hint text plus the status suffix to
    // compute how many spaces to insert between them. Underestimates with
    // double-width glyphs are fine — the status just hugs the right side
    // tightly instead of drifting to the absolute edge.
    let hint_len: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let status_len: usize = status.iter().map(|s| s.content.chars().count()).sum();
    let total_w = area.width as usize;
    let pad = total_w.saturating_sub(hint_len + status_len + 2);
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    spans.extend(status);

    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}
