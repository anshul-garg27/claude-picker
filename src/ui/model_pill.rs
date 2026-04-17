//! Coloured model pill widget — the "[opus]", "[sonnet]", "[haiku]" tag,
//! plus the secondary "permission-mode" pill (`PLAN`, `BYPASS`, `ACCEPT`).
//!
//! A tiny renderer that converts a model family into a single [`Span`] coloured
//! to match the website mockup: peach for Opus, teal for Sonnet, blue for
//! Haiku, mauve for anything we don't recognise. Text is always `crust` (dark)
//! so foreground/background contrast stays readable.
//!
//! The pill is rendered as `[name]` rather than with block glyphs so it
//! reflows cleanly in any row layout and stays legible at 4-6 chars wide
//! (the brief called that out explicitly).
//!
//! **v2.2 polish:** the default pill is now a *chip* — a block-character
//! frame `▌opus▐` where the left/right half-blocks read as a tinted border
//! and the middle is the same family colour on a darker `surface0` bed so
//! the chip visually floats above the row. Falls back to the flat pill via
//! [`flat_pill`] when the caller is on a background that already uses
//! `surface0` (toasts, modals) and the chip effect would vanish.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::data::pricing::Family;
use crate::data::PermissionMode;
use crate::theme::Theme;

/// Colour + label for the given family, exposed so other widgets can match
/// the pill colour without hard-coding.
pub fn family_color(family: Family, theme: &Theme) -> ratatui::style::Color {
    match family {
        Family::Opus => theme.peach,
        Family::Sonnet => theme.teal,
        Family::Haiku => theme.blue,
        Family::Unknown => theme.mauve,
    }
}

/// Default model pill — the "chip" style.
///
/// Renders `▌label▐` where the half-blocks read as a tinted 1-column border
/// and the interior uses the family colour as foreground over `surface0`.
/// Works on any TrueColor terminal; degrades to a readable two-tone slab on
/// 256-colour emulators (the half-block glyphs are still in Unicode BMP).
///
/// This replaces the flat-pill lookups from v2.1. Callers that need the flat
/// solid-bg pill — modal bodies where the surface0 bed would disappear —
/// should call [`flat_pill`] explicitly.
pub fn pill<'a>(family: Family, theme: &Theme) -> Span<'a> {
    chip_pill(family, theme)
}

/// Chip rendering: `▌label▐` with family colour over `surface0`.
///
/// The left / right half-blocks (U+258C / U+2590) act as a subtle 1-col frame
/// in the family colour; the interior text is the same family colour over a
/// slightly lighter bed so the chip looks like it's floating one level above
/// the row. Bold for weight — the effect is "floating chip" rather than "flat
/// tag", matching Raycast / Linear's pill treatment.
pub fn chip_pill<'a>(family: Family, theme: &Theme) -> Span<'a> {
    let accent = family_color(family, theme);
    let label = match family {
        Family::Opus => "opus",
        Family::Sonnet => "sonnet",
        Family::Haiku => "haiku",
        Family::Unknown => "?",
    };
    let style = Style::default()
        .fg(accent)
        .bg(theme.surface0)
        .add_modifier(Modifier::BOLD);
    // `▌` and `▐` are Left/Right Half Block. On TrueColor terminals they
    // render as a tinted 1-col rail flanking the label; on 256-colour
    // terminals they still look like block glyphs in the accent colour.
    Span::styled(format!("\u{258C}{label}\u{2590}"), style)
}

/// Legacy flat-bg pill. Still useful on modal bodies where we don't want the
/// `surface0` bed to blend into the modal's own `surface0` title stripe.
pub fn flat_pill<'a>(family: Family, theme: &Theme) -> Span<'a> {
    let accent = family_color(family, theme);
    let label = match family {
        Family::Opus => "opus",
        Family::Sonnet => "sonnet",
        Family::Haiku => "haiku",
        Family::Unknown => "?",
    };
    let style = Style::default()
        .bg(accent)
        .fg(theme.crust)
        .add_modifier(Modifier::BOLD);
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

/// Pill for a non-default [`PermissionMode`]. Returns `None` when the mode
/// is `Default` — we deliberately don't render a badge for the common case
/// to avoid visual noise.
///
/// Colors:
/// - `PLAN`     → cyan (`sky`) — neutral, explanatory
/// - `BYPASS`   → red — "this is risky, pay attention"
/// - `ACCEPT`   → yellow — "semi-auto, still careful"
/// - `DONTASK`  → pink — a step past `acceptEdits`
/// - `AUTO`     → lavender — managed-auto mode
/// - other      → mauve (fallback)
pub fn permission_pill<'a>(mode: PermissionMode, theme: &Theme) -> Option<Span<'a>> {
    let label = mode.pill_label()?;
    let bg = match mode {
        PermissionMode::Plan => theme.sky,
        PermissionMode::BypassPermissions => theme.red,
        PermissionMode::AcceptEdits => theme.yellow,
        PermissionMode::DontAsk => theme.pink,
        PermissionMode::Auto => theme.lavender,
        PermissionMode::Other(_) => theme.mauve,
        PermissionMode::Default => return None,
    };
    let style = Style::default()
        .bg(bg)
        .fg(theme.crust)
        .add_modifier(Modifier::BOLD);
    Some(Span::styled(format!(" {label} "), style))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_chip_uses_peach_fg_over_surface0() {
        // Chip-style: family colour in the fg (text + half-block rails),
        // surface0 as the floating-chip bed.
        let t = Theme::mocha();
        let span = pill(Family::Opus, &t);
        assert!(span.content.contains("opus"));
        assert!(span.content.starts_with('\u{258C}'));
        assert!(span.content.ends_with('\u{2590}'));
        assert_eq!(span.style.fg, Some(t.peach));
        assert_eq!(span.style.bg, Some(t.surface0));
    }

    #[test]
    fn flat_pill_keeps_solid_bg() {
        // Legacy flat variant still available for callers on a surface0 bg.
        let t = Theme::mocha();
        let span = flat_pill(Family::Opus, &t);
        assert_eq!(span.style.bg, Some(t.peach));
        assert_eq!(span.style.fg, Some(t.crust));
    }

    #[test]
    fn unknown_pill_is_mauve() {
        let t = Theme::mocha();
        let span = pill(Family::Unknown, &t);
        assert_eq!(span.style.fg, Some(t.mauve));
    }

    #[test]
    fn family_color_is_stable_across_themes() {
        // Every family maps to a concrete colour on every palette — the
        // chip's fg never collapses to None.
        for &tn in crate::theme::ThemeName::ALL {
            let t = Theme::from_name(tn);
            for f in [Family::Opus, Family::Sonnet, Family::Haiku, Family::Unknown] {
                let _ = family_color(f, &t);
            }
        }
    }

    #[test]
    fn default_mode_has_no_pill() {
        let t = Theme::mocha();
        assert!(permission_pill(PermissionMode::Default, &t).is_none());
    }

    #[test]
    fn bypass_is_red_plan_is_sky() {
        let t = Theme::mocha();
        let bypass = permission_pill(PermissionMode::BypassPermissions, &t).expect("pill");
        assert!(bypass.content.contains("BYPASS"));
        assert_eq!(bypass.style.bg, Some(t.red));

        let plan = permission_pill(PermissionMode::Plan, &t).expect("pill");
        assert!(plan.content.contains("PLAN"));
        assert_eq!(plan.style.bg, Some(t.sky));

        let accept = permission_pill(PermissionMode::AcceptEdits, &t).expect("pill");
        assert!(accept.content.contains("ACCEPT"));
        assert_eq!(accept.style.bg, Some(t.yellow));
    }
}
