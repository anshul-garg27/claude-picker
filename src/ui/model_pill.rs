//! Coloured model pill widget — the "[opus]", "[sonnet]", "[haiku]" tag.
//!
//! A tiny renderer that converts a model family into a single [`Span`] coloured
//! to match the website mockup: peach for Opus, teal for Sonnet, blue for
//! Haiku, mauve for anything we don't recognise. Text is always `crust` (dark)
//! so foreground/background contrast stays readable.
//!
//! The pill is rendered as `[name]` rather than with block glyphs so it
//! reflows cleanly in any row layout and stays legible at 4-6 chars wide
//! (the brief called that out explicitly).

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::data::pricing::Family;
use crate::theme::Theme;

/// Build the Span for a single model family. Callers compose this into a row
/// Line alongside other text; the widget itself is just a styled string.
pub fn pill<'a>(family: Family, theme: &Theme) -> Span<'a> {
    let (bg, label) = match family {
        Family::Opus => (theme.peach, "opus"),
        Family::Sonnet => (theme.teal, "sonnet"),
        Family::Haiku => (theme.blue, "haiku"),
        Family::Unknown => (theme.mauve, "?"),
    };

    let style = Style::default()
        .bg(bg)
        .fg(theme.crust)
        .add_modifier(Modifier::BOLD);

    // The extra spaces act as pill "caps" — at TrueColor terminals they
    // render as rounded-looking tinted padding. Works at all widths.
    Span::styled(format!(" {label} "), style)
}

/// Mixed-model session: render multiple pills separated by a thin space.
/// Used when [`crate::data::Session`] flags multi-family usage.
pub fn pills<'a>(families: &[Family], theme: &Theme) -> Vec<Span<'a>> {
    let mut out = Vec::with_capacity(families.len() * 2);
    for (i, f) in families.iter().enumerate() {
        if i > 0 {
            out.push(Span::raw(" "));
        }
        out.push(pill(*f, theme));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_gets_peach_bg() {
        let t = Theme::mocha();
        let span = pill(Family::Opus, &t);
        // content should contain "opus"
        assert!(span.content.contains("opus"));
        assert_eq!(span.style.bg, Some(t.peach));
        assert_eq!(span.style.fg, Some(t.crust));
    }

    #[test]
    fn unknown_pill_is_mauve() {
        let t = Theme::mocha();
        let span = pill(Family::Unknown, &t);
        assert_eq!(span.style.bg, Some(t.mauve));
    }
}
