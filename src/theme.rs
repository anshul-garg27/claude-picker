//! Catppuccin Mocha theme tokens.
//!
//! Centralises every color used by the UI so tweaking the palette (or swapping
//! to a different Catppuccin flavour later) is a one-line change. Colors come
//! straight out of the `catppuccin` crate's palette; we map them to
//! `ratatui::style::Color` at construction time and cache the result on a
//! [`Theme`] struct so hot-path widgets never touch the palette directly.
//!
//! A handful of composite [`Style`] helpers (`selected_row`, `panel_border`,
//! …) live alongside the raw tokens — each one expresses an intent (e.g.
//! "active pane border") rather than a specific color, which keeps callers
//! decoupled from minor restyling.

use catppuccin::{Color as CatColor, PALETTE};
use ratatui::style::{Color, Modifier, Style};

/// Convert a Catppuccin palette entry to a ratatui TrueColor. The catppuccin
/// crate ships an optional `ratatui` feature, but it targets `ratatui-core`
/// rather than the top-level `ratatui` crate, so we round-trip through RGB
/// components — which is exactly what the feature would do internally.
#[inline]
fn rgb(c: &CatColor) -> Color {
    let r = c.rgb.r;
    let g = c.rgb.g;
    let b = c.rgb.b;
    Color::Rgb(r, g, b)
}

/// Catppuccin Mocha palette broken out into ratatui colors.
///
/// Only the subset of the palette we actually use is exposed. All fields are
/// `Copy` so passing `Theme` around the render tree is cheap; in practice we
/// hand out `&Theme` because `App` owns a single instance.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    // Backgrounds, darkest → lightest.
    pub crust: Color,
    pub mantle: Color,
    pub base: Color,
    pub surface0: Color,
    pub surface1: Color,
    pub surface2: Color,

    // Foregrounds, brightest → dimmest.
    pub text: Color,
    pub subtext1: Color,
    pub subtext0: Color,
    pub overlay2: Color,
    pub overlay1: Color,
    pub overlay0: Color,

    // Accent colors used by UI signals.
    pub mauve: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub peach: Color,
    pub teal: Color,
    pub red: Color,
    pub pink: Color,
    pub sky: Color,
    pub lavender: Color,
}

impl Theme {
    /// Build the Mocha theme. Cheap: a handful of `Color` conversions.
    pub fn mocha() -> Self {
        let c = &PALETTE.mocha.colors;
        Self {
            crust: rgb(&c.crust),
            mantle: rgb(&c.mantle),
            base: rgb(&c.base),
            surface0: rgb(&c.surface0),
            surface1: rgb(&c.surface1),
            surface2: rgb(&c.surface2),

            text: rgb(&c.text),
            subtext1: rgb(&c.subtext1),
            subtext0: rgb(&c.subtext0),
            overlay2: rgb(&c.overlay2),
            overlay1: rgb(&c.overlay1),
            overlay0: rgb(&c.overlay0),

            mauve: rgb(&c.mauve),
            green: rgb(&c.green),
            yellow: rgb(&c.yellow),
            blue: rgb(&c.blue),
            peach: rgb(&c.peach),
            teal: rgb(&c.teal),
            red: rgb(&c.red),
            pink: rgb(&c.pink),
            sky: rgb(&c.sky),
            lavender: rgb(&c.lavender),
        }
    }

    // ── Composite styles ────────────────────────────────────────────────

    /// Border color for an inactive panel. Deliberately low-contrast so the
    /// active panel (rendered with [`panel_border_active`](Self::panel_border_active))
    /// visibly "lights up".
    pub fn panel_border(&self) -> Style {
        Style::default().fg(self.surface1)
    }

    /// Border color for the currently focused panel. Mauve reads as "active"
    /// without being loud enough to pull focus from the row cursor.
    pub fn panel_border_active(&self) -> Style {
        Style::default().fg(self.mauve)
    }

    /// Style applied to the row under the selection cursor. Uses `surface0`
    /// as a background stripe + green-bold name to match the website mockup.
    pub fn selected_row(&self) -> Style {
        Style::default()
            .bg(self.surface0)
            .fg(self.green)
            .add_modifier(Modifier::BOLD)
    }

    /// Meta text — timestamps, counts, anything that shouldn't grab the eye.
    pub fn muted(&self) -> Style {
        Style::default().fg(self.overlay0)
    }

    /// Dimmer than `muted`. Used for borders-as-text ("─── recent ───").
    pub fn dim(&self) -> Style {
        Style::default().fg(self.surface2)
    }

    /// Body text — messages, names, anything meant to be read.
    pub fn body(&self) -> Style {
        Style::default().fg(self.text)
    }

    /// Subtitle / helper text. One notch brighter than `muted`.
    pub fn subtle(&self) -> Style {
        Style::default().fg(self.subtext0)
    }

    /// Filter input style. Mauve cursor + body text color matches the TCSS.
    pub fn filter_text(&self) -> Style {
        Style::default().fg(self.text)
    }

    /// Placeholder text inside the filter when empty.
    pub fn filter_placeholder(&self) -> Style {
        Style::default()
            .fg(self.surface2)
            .add_modifier(Modifier::ITALIC)
    }

    /// Key hint — the letters users press. Bright enough to stand out.
    pub fn key_hint(&self) -> Style {
        Style::default()
            .fg(self.yellow)
            .add_modifier(Modifier::BOLD)
    }

    /// Description after a key hint ("navigate", "resume").
    pub fn key_desc(&self) -> Style {
        Style::default().fg(self.overlay1)
    }
}

/// Foreground color for the model pill text. Always dark on colored bg so the
/// text remains legible regardless of which family color we picked.
pub fn pill_text_color(theme: &Theme) -> Color {
    theme.crust
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mocha_is_constructible() {
        // Smoke test. All the conversions through catppuccin should succeed.
        let t = Theme::mocha();
        // surface0 must differ from base — otherwise selected-row won't pop.
        assert_ne!(t.surface0, t.base);
    }

    #[test]
    fn selected_row_has_bold_modifier() {
        let t = Theme::mocha();
        let s = t.selected_row();
        assert!(s.add_modifier.contains(Modifier::BOLD));
    }
}
