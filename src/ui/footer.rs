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
    render_session_list_with_multi(f, area, theme, 0, false);
}

/// Session-list footer that knows about multi-select mode. `count` is the
/// number of sessions currently multi-selected; when `multi_mode` is true
/// and count > 0 we render the batch-action hints.
pub fn render_session_list_with_multi(
    f: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    count: usize,
    multi_mode: bool,
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
        ("type", "filter"),
        ("?", "help"),
        ("q", "quit"),
    ];
    render_hints(f, area, theme, &hints);
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
        ("type", "filter"),
        ("?", "help"),
        ("q", "quit"),
    ];
    render_hints(f, area, theme, &hints);
}

fn render_hints(f: &mut Frame<'_>, area: Rect, theme: &Theme, hints: &[(&str, &str)]) {
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
    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}
