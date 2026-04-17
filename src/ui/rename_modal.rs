//! Inline rename modal for the currently selected session.
//!
//! When the user presses `r`, the app opens a small centered modal that shows
//! the current session name (or empty), a text cursor, and key hints. `Enter`
//! saves, `Esc` cancels. Saving is delegated to
//! [`crate::data::session_rename`] which appends a `custom-title` record to the
//! session's JSONL so subsequent runs of the picker (and anything else that
//! reads Claude Code session files) see the new name.
//!
//! State is kept outside this module; we just render. That makes it trivial to
//! unit-test and keeps the modal reusable from other screens later.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

/// Persistent rename-modal state. Owned by [`crate::app::App`].
#[derive(Debug, Clone)]
pub struct RenameState {
    /// Session id being renamed. Used by the save path so the cursor moving
    /// away before save still targets the original row.
    pub session_id: String,
    /// Current buffer the user is typing into.
    pub buffer: String,
}

impl RenameState {
    /// Construct a new modal seeded with the current name (may be empty).
    pub fn new(session_id: impl Into<String>, current_name: Option<&str>) -> Self {
        Self {
            session_id: session_id.into(),
            buffer: current_name.unwrap_or("").to_string(),
        }
    }

    /// Push a character into the buffer. Same filter as the main filter input
    /// — alphanumerics and common word-safe punctuation.
    pub fn push(&mut self, c: char) {
        if is_name_char(c) {
            self.buffer.push(c);
        }
    }

    /// Backspace.
    pub fn pop(&mut self) {
        self.buffer.pop();
    }
}

/// True when a character is allowed in a session name. Keep this generous —
/// users type emoji in custom titles all the time.
pub fn is_name_char(c: char) -> bool {
    !c.is_control()
}

/// Render the modal centered inside `area`.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &RenameState, theme: &Theme) {
    let w = 60u16.min(area.width.saturating_sub(4));
    let h = 7u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "rename session",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));

    let input_line = if state.buffer.is_empty() {
        Line::from(vec![
            Span::raw("  "),
            Span::styled("> ", theme.muted()),
            Span::styled("type a new name…", theme.filter_placeholder()),
            // Block cursor (reverse-video space) — match the main filter.
            Span::styled(" ", Style::default().bg(theme.mauve).fg(theme.crust)),
        ])
    } else {
        Line::from(vec![
            Span::raw("  "),
            Span::styled("> ", theme.muted()),
            Span::styled(state.buffer.clone(), theme.filter_text()),
            Span::styled(" ", Style::default().bg(theme.mauve).fg(theme.crust)),
        ])
    };

    let hints = Line::from(vec![
        Span::raw("  "),
        Span::styled("Enter", theme.key_hint()),
        Span::styled(" save  ", theme.key_desc()),
        Span::styled("Esc", theme.key_hint()),
        Span::styled(" cancel", theme.key_desc()),
    ]);

    let paragraph =
        Paragraph::new(vec![Line::raw(""), input_line, Line::raw(""), hints]).block(block);
    frame.render_widget(paragraph, rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_filters_control_chars() {
        let mut s = RenameState::new("abc", Some("x"));
        s.push('y');
        s.push('\n');
        s.push('\t');
        assert_eq!(s.buffer, "xy");
    }

    #[test]
    fn pop_removes_last_char() {
        let mut s = RenameState::new("abc", Some("hello"));
        s.pop();
        assert_eq!(s.buffer, "hell");
    }
}
