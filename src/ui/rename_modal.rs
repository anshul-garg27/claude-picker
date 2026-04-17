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
//!
//! **Error surface (E10):** validation failures used to close the modal and
//! surface a red toast; the user had to re-open the modal to fix the name.
//! We now keep the modal open, flip the border to `red`, and render the
//! failure message under the input line. The error clears the instant the
//! user types/deletes a character, so recovery is a single keystroke away.

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
    /// Current validation error, if any. Set by the save handler when
    /// validation fails (e.g. empty name, rename_session returned Err); the
    /// modal renders with a red border and a 1-line error string under the
    /// input until the user types or deletes a character.
    pub error_message: Option<String>,
}

impl RenameState {
    /// Construct a new modal seeded with the current name (may be empty).
    pub fn new(session_id: impl Into<String>, current_name: Option<&str>) -> Self {
        Self {
            session_id: session_id.into(),
            buffer: current_name.unwrap_or("").to_string(),
            error_message: None,
        }
    }

    /// Push a character into the buffer. Same filter as the main filter input
    /// — alphanumerics and common word-safe punctuation.
    ///
    /// Any stored error is cleared on buffer mutation — the next keystroke is
    /// the user's "I acknowledge, let me fix it" signal, and leaving the
    /// error visible past that point is redundant noise.
    pub fn push(&mut self, c: char) {
        if is_name_char(c) {
            self.buffer.push(c);
            self.error_message = None;
        }
    }

    /// Backspace. Also clears any stored error, same rationale as [`push`].
    pub fn pop(&mut self) {
        self.buffer.pop();
        self.error_message = None;
    }

    /// Set the inline error string. Replaces any prior error; pass an empty
    /// string to clear. The save handler calls this instead of closing the
    /// modal so the user can fix the name without reopening.
    pub fn set_error(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if msg.is_empty() {
            self.error_message = None;
        } else {
            self.error_message = Some(msg);
        }
    }
}

/// True when a character is allowed in a session name. Keep this generous —
/// users type emoji in custom titles all the time.
pub fn is_name_char(c: char) -> bool {
    !c.is_control()
}

/// Render the modal centered inside `area`.
pub fn render(frame: &mut Frame<'_>, area: Rect, state: &RenameState, theme: &Theme) {
    // Grow the modal by one row when an error is present so the inline error
    // string has dedicated real estate and doesn't fight the hints line.
    let w = 60u16.min(area.width.saturating_sub(4));
    let h = if state.error_message.is_some() { 8 } else { 7 };
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, rect);

    // Flip the border to the theme's `red` when an error is showing so the
    // failure state is pre-attentive. Title copy picks up the same tone so
    // users don't have to locate the string to know something's wrong.
    let has_error = state.error_message.is_some();
    let border_color = if has_error { theme.red } else { theme.mauve };
    let title_color = border_color;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "rename session",
                Style::default()
                    .fg(title_color)
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

    // Build the body rows. When an error is present, we thread it in between
    // the input and the hints so the user's eye moves input → error → hints,
    // matching the brief's mockup.
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(5);
    lines.push(Line::raw(""));
    lines.push(input_line);
    if let Some(err) = state.error_message.as_deref() {
        // `└─` arm + red body copy — visually tethers the error to the input.
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                "\u{2514}\u{2500} ",
                Style::default().fg(theme.red),
            ),
            Span::styled(
                err.to_string(),
                Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(hints);

    let paragraph = Paragraph::new(lines).block(block);
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

    #[test]
    fn new_state_has_no_error() {
        let s = RenameState::new("abc", Some("hi"));
        assert!(s.error_message.is_none());
    }

    #[test]
    fn set_error_stores_message() {
        let mut s = RenameState::new("abc", Some("hi"));
        s.set_error("name can't be empty");
        assert_eq!(s.error_message.as_deref(), Some("name can't be empty"));
    }

    #[test]
    fn set_error_with_empty_clears() {
        let mut s = RenameState::new("abc", Some("hi"));
        s.set_error("bad");
        s.set_error("");
        assert!(s.error_message.is_none());
    }

    #[test]
    fn push_clears_error() {
        // The core interaction contract — typing ack's the error and resumes
        // normal entry without the user having to dismiss a toast.
        let mut s = RenameState::new("abc", Some("hi"));
        s.set_error("oops");
        s.push('z');
        assert!(s.error_message.is_none());
        assert_eq!(s.buffer, "hiz");
    }

    #[test]
    fn pop_clears_error() {
        let mut s = RenameState::new("abc", Some("hi"));
        s.set_error("oops");
        s.pop();
        assert!(s.error_message.is_none());
        assert_eq!(s.buffer, "h");
    }

    #[test]
    fn push_rejects_control_char_without_clearing_error() {
        // Control chars never land in the buffer, so they shouldn't count as
        // "the user is fixing the error" either — keep the error visible.
        let mut s = RenameState::new("abc", Some("hi"));
        s.set_error("oops");
        s.push('\n');
        assert_eq!(s.buffer, "hi");
        assert!(s.error_message.is_some());
    }
}
