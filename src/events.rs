//! Event normalisation over `crossterm`.
//!
//! The Ratatui event loop reads from `crossterm::event`. Two things make the
//! raw enum awkward:
//!
//! 1. We only care about key events that *press* a key — the `kind == Release`
//!    events that modern terminals emit would double-fire every action.
//! 2. We want a terse `Event::Key(c) | Event::Ctrl('d') | Event::Up` rather
//!    than spelling out `KeyEvent { code: KeyCode::Up, modifiers: …, .. }` in
//!    the handler. That keeps [`App::handle_event`] a readable match.
//!
//! This module is therefore a thin translation layer: poll, convert, hand off.

use std::time::Duration;

use crossterm::event::{self, Event as CtEvent, KeyCode, KeyEventKind, KeyModifiers};

/// Poll interval — a snappy feel without pinning a CPU core. Anything under
/// 250 ms is imperceptible to humans; 50 ms is the traditional REPL cadence.
const POLL: Duration = Duration::from_millis(50);

/// Normalised UI events.
///
/// `Key(char)` is any plain printable character; modifier-only combos become
/// `Ctrl(char)` so the handler doesn't care whether Shift was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A plain character — letter, digit, `-`, `_`, `space`.
    Key(char),
    /// `Ctrl + <char>`. Char is always lowercase.
    Ctrl(char),
    /// Cursor navigation.
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    /// Return / Enter.
    Enter,
    /// Escape.
    Escape,
    /// Backspace.
    Backspace,
    /// Delete (forward-delete). Treated like Backspace in some contexts.
    Delete,
    /// Tab — used to switch focus in future screens (unused today).
    Tab,
    /// Terminal resize — prompt a redraw.
    Resize(u16, u16),
    /// The window system asked us to quit (e.g. SIGHUP, window close).
    Quit,
}

/// Block for one poll cycle and return whichever event shows up.
///
/// `Ok(None)` means "no event in the poll window" — caller re-enters the
/// draw/poll loop. `Ok(Some(..))` is a real event; `Err` is an I/O error from
/// the terminal itself and propagates up to `main`.
pub fn next() -> anyhow::Result<Option<Event>> {
    if !event::poll(POLL)? {
        return Ok(None);
    }
    match event::read()? {
        CtEvent::Key(k) => {
            // Kitty / Windows emit both Press and Release. Only act on Press.
            if k.kind != KeyEventKind::Press {
                return Ok(None);
            }
            Ok(translate_key(k.code, k.modifiers))
        }
        CtEvent::Resize(w, h) => Ok(Some(Event::Resize(w, h))),
        // Mouse / paste / focus events are ignored for now; a future feature
        // (mouse scroll, bracketed paste into filter) can slot in here.
        _ => Ok(None),
    }
}

/// Translate a crossterm key into our normalised enum.
///
/// Returns `None` for key combinations we don't understand (f-keys today,
/// etc.) so the caller can keep running without a panic.
fn translate_key(code: KeyCode, mods: KeyModifiers) -> Option<Event> {
    // Handle Ctrl+<char> first, before the plain-char branch swallows it.
    if mods.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = code {
            // Ctrl+Shift+X and Ctrl+X collapse to the same thing — we care
            // about the letter, not the case.
            return Some(Event::Ctrl(c.to_ascii_lowercase()));
        }
    }

    Some(match code {
        KeyCode::Up => Event::Up,
        KeyCode::Down => Event::Down,
        KeyCode::Left => Event::Left,
        KeyCode::Right => Event::Right,
        KeyCode::Home => Event::Home,
        KeyCode::End => Event::End,
        KeyCode::PageUp => Event::PageUp,
        KeyCode::PageDown => Event::PageDown,
        KeyCode::Enter => Event::Enter,
        KeyCode::Esc => Event::Escape,
        KeyCode::Backspace => Event::Backspace,
        KeyCode::Delete => Event::Delete,
        KeyCode::Tab => Event::Tab,
        KeyCode::Char(c) => Event::Key(c),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_char_becomes_key() {
        assert_eq!(
            translate_key(KeyCode::Char('a'), KeyModifiers::NONE),
            Some(Event::Key('a'))
        );
    }

    #[test]
    fn ctrl_combines() {
        assert_eq!(
            translate_key(KeyCode::Char('b'), KeyModifiers::CONTROL),
            Some(Event::Ctrl('b'))
        );
    }

    #[test]
    fn ctrl_shift_case_folds() {
        // Shift doesn't matter — Ctrl+B and Ctrl+Shift+B both fire 'b'.
        let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert_eq!(
            translate_key(KeyCode::Char('B'), mods),
            Some(Event::Ctrl('b'))
        );
    }

    #[test]
    fn arrows_map_directly() {
        assert_eq!(
            translate_key(KeyCode::Up, KeyModifiers::NONE),
            Some(Event::Up)
        );
        assert_eq!(
            translate_key(KeyCode::Down, KeyModifiers::NONE),
            Some(Event::Down)
        );
    }

    #[test]
    fn enter_escape_backspace() {
        assert_eq!(
            translate_key(KeyCode::Enter, KeyModifiers::NONE),
            Some(Event::Enter)
        );
        assert_eq!(
            translate_key(KeyCode::Esc, KeyModifiers::NONE),
            Some(Event::Escape)
        );
        assert_eq!(
            translate_key(KeyCode::Backspace, KeyModifiers::NONE),
            Some(Event::Backspace)
        );
    }
}
