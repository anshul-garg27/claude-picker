//! Cross-platform clipboard helper.
//!
//! Wraps `arboard::Clipboard` behind a tiny API so callers don't have to know
//! about the upstream error type — a `Result<(), String>` is enough context
//! for the picker's toast messaging. `arboard` is pure Rust on macOS (no X11
//! deps), Windows, and modern Linux; we only initialise on demand so the
//! binary still launches cleanly in headless terminals that can't open a
//! display.

/// Copy `text` into the system clipboard. Returns a short human-readable error
/// on failure (typically when the environment has no display to attach to).
pub fn copy(text: impl Into<String>) -> Result<(), String> {
    let text = text.into();
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("{e}"))?;
    cb.set_text(text).map_err(|e| format!("{e}"))
}
