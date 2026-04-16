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
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::Theme;

/// Separator between hints. A middle dot reads lighter than "|" and doesn't
/// visually collide with the `▸` cursor glyph on the list.
const SEP: &str = "  ·  ";

/// Render the footer for the session-list screen.
pub fn render_session_list(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let hints = [
        ("↑↓", "navigate"),
        ("Enter", "resume"),
        ("a-z", "filter"),
        ("Ctrl+B", "pin"),
        ("Ctrl+E", "export"),
        ("Esc", "reset"),
        ("q", "quit"),
    ];
    render_hints(f, area, theme, &hints);
}

/// Render the footer for the project-list screen.
pub fn render_project_list(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let hints = [
        ("↑↓", "navigate"),
        ("Enter", "open"),
        ("a-z", "filter"),
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
