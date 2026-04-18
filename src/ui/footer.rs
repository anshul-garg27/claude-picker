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
/// visually collide with the `▸` cursor glyph on the list. Upgraded to a
/// thick-dot bullet (U+25CF) in theme.dim so separators visually anchor
/// without pulling focus from the pills themselves.
const SEP: &str = "  \u{25CF}  ";

/// Render a key-binding hint as a reverse-video pill: `▌key▐ description`.
///
/// The pill rails + interior use mauve as bg with crust as fg so keys read as
/// a single high-contrast slug regardless of terminal theme. Returns the spans
/// for the pill + a trailing space + the description. `highlight` tints the
/// description in peach bold when the hint is context-sensitive (e.g. a row
/// is selected so Enter/v become live actions) per the brief.
fn key_pill_spans<'a>(key: &str, desc: &str, theme: &Theme, highlight: bool) -> Vec<Span<'a>> {
    let pill_style = Style::default()
        .bg(theme.mauve)
        .fg(theme.crust)
        .add_modifier(Modifier::BOLD);
    let desc_style = if highlight {
        Style::default()
            .fg(theme.peach)
            .add_modifier(Modifier::BOLD)
    } else {
        theme.key_desc()
    };
    vec![
        Span::styled(format!("\u{258C}{key}\u{2590}"), pill_style),
        Span::raw(" "),
        Span::styled(desc.to_string(), desc_style),
    ]
}

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
/// the help overlay. The multi-select banner + action pills paint in peach
/// so the user sees at a glance they're in batch-action mode.
fn render_multi_hints(f: &mut Frame<'_>, area: Rect, theme: &Theme, count: usize) {
    let dim = theme.dim();
    let mut spans = vec![
        Span::raw("  "),
        // Peach-bold banner badge: `▌N selected▐` so the state is visually
        // distinct from the normal key-hint footer.
        Span::styled(
            format!("\u{258C} {count} selected \u{2590}"),
            Style::default()
                .bg(theme.peach)
                .fg(theme.crust)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(SEP, dim),
    ];
    // Batch actions — destructive ones (delete) tint their description in
    // red so the risk reads at a glance.
    let batch: &[(&str, &str, bool)] = &[
        ("Tab", "toggle", false),
        ("Esc", "clear", false),
        ("Ctrl+E", "export all", false),
        ("Ctrl+D", "delete all", true),
        ("y", "copy ids", false),
    ];
    for (i, (key, desc, danger)) in batch.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(SEP, dim));
        }
        let pill_style = Style::default()
            .bg(theme.peach)
            .fg(theme.crust)
            .add_modifier(Modifier::BOLD);
        let desc_style = if *danger {
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD)
        } else {
            theme.key_desc()
        };
        spans.push(Span::styled(format!("\u{258C}{key}\u{2590}"), pill_style));
        spans.push(Span::raw(" "));
        spans.push(Span::styled((*desc).to_string(), desc_style));
    }
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
    // The first two hints (navigate + primary action) are *context-
    // sensitive* — when anything is selected they resolve to live actions,
    // so we tint their description in peach bold so the eye gravitates to
    // the next likely keystroke. Keys past index 1 stay in the normal
    // key_desc tone.
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(SEP, theme.dim()));
        }
        let highlight = i < 2;
        spans.extend(key_pill_spans(key, desc, theme, highlight));
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
