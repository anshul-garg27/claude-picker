//! Theme tokens — 6 built-in themes + runtime switching.
//!
//! Centralises every color used by the UI so swapping palettes is a runtime
//! concern instead of a recompile. Users pick between 6 baked-in themes
//! (Catppuccin Mocha/Latte, Dracula, TokyoNight, GruvboxDark, Nord) via the
//! `--theme` CLI flag, `CLAUDE_PICKER_THEME` env var, or the `t` keybinding
//! on the main picker screen (which cycles through [`ThemeName::ALL`]).
//!
//! Design:
//! - [`ThemeName`] is a cheap `Copy` enum with `label()`/`from_str()`/`next()`.
//! - [`Theme`] carries the resolved ratatui colors for the active palette.
//!   Every screen borrows `&theme` through `App`; changing theme is a single
//!   field reassignment and the next frame re-renders in new colors.
//! - Composite style helpers (`panel_border()`, `selected_row()`, …) operate
//!   on `&self` so callers never reach for raw tokens.
//!
//! Every theme populates **every field** — even the less-used ones like
//! `sky`, `lavender`, `pink`, `overlay1`, `overlay2` — so widgets that already
//! reference them keep working without guards.

use catppuccin::{Color as CatColor, PALETTE};
use ratatui::style::{Color, Modifier, Style};

/// One of the 6 built-in themes.
///
/// `Copy` so it's cheap to pass around and compare. The order of
/// [`Self::ALL`] is the cycle order used by the runtime `t` keybinding —
/// starts on Mocha (default) and wraps after Nord.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    #[default]
    CatppuccinMocha,
    CatppuccinLatte,
    Dracula,
    TokyoNight,
    GruvboxDark,
    Nord,
}

impl ThemeName {
    /// Every theme in cycle order. First entry is the default.
    pub const ALL: &'static [ThemeName] = &[
        ThemeName::CatppuccinMocha,
        ThemeName::CatppuccinLatte,
        ThemeName::Dracula,
        ThemeName::TokyoNight,
        ThemeName::GruvboxDark,
        ThemeName::Nord,
    ];

    /// Stable kebab-case label. Used for CLI parsing, env var values, the
    /// persistence file on disk, and the toast shown on theme switch.
    pub fn label(self) -> &'static str {
        match self {
            Self::CatppuccinMocha => "catppuccin-mocha",
            Self::CatppuccinLatte => "catppuccin-latte",
            Self::Dracula => "dracula",
            Self::TokyoNight => "tokyo-night",
            Self::GruvboxDark => "gruvbox-dark",
            Self::Nord => "nord",
        }
    }

    /// Parse a label back into a [`ThemeName`]. Case-insensitive so
    /// `Dracula`, `DRACULA`, and `dracula` all resolve.
    //
    // We deliberately don't implement `std::str::FromStr` — that trait
    // requires choosing an error type and returning a `Result`, but our
    // callers genuinely want the "unknown → None, fall through to next
    // source" semantics of an `Option`. Keeping `from_str` as an inherent
    // method is the shortest path to that shape.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|t| t.label().eq_ignore_ascii_case(s))
    }

    /// Next theme in [`Self::ALL`] order, wrapping at the end. Used by the
    /// `t` keybinding on the main picker.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&t| t == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// Resolved theme colors. All fields are `Copy` so passing `Theme` around the
/// render tree is free; in practice `App` owns one instance and the UI
/// borrows `&Theme`.
///
/// Every field is populated by every theme so widgets never need `Option`
/// checks — pick a near neighbour in the palette when the source doesn't
/// have a direct equivalent.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: ThemeName,

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

    // Accent colors. Semantic names are unchanged per theme; the actual hue
    // differs so e.g. "mauve" on Dracula is actually purple.
    pub mauve: Color,    // accent primary
    pub green: Color,    // success / dollar amounts
    pub yellow: Color,   // emphasis / sessions count
    pub blue: Color,     // links / user messages
    pub peach: Color,    // secondary / opus pill
    pub teal: Color,     // sonnet pill
    pub red: Color,      // errors / bypass mode
    pub pink: Color,     // optional accent (stats haiku)
    pub sky: Color,      // optional accent (stats haiku)
    pub lavender: Color, // optional accent (stats sonnet)
}

impl Theme {
    /// Build a theme from its [`ThemeName`]. Cheap — a handful of `Color`
    /// conversions; safe to call on every theme switch.
    pub fn from_name(name: ThemeName) -> Self {
        match name {
            ThemeName::CatppuccinMocha => catppuccin_mocha(),
            ThemeName::CatppuccinLatte => catppuccin_latte(),
            ThemeName::Dracula => dracula(),
            ThemeName::TokyoNight => tokyo_night(),
            ThemeName::GruvboxDark => gruvbox_dark(),
            ThemeName::Nord => nord(),
        }
    }

    /// Convenience — the historical default. Kept so callers that haven't
    /// been plumbed with a theme name (stats_cmd, diff_cmd, etc.) still
    /// compile. Equivalent to `Theme::from_name(ThemeName::CatppuccinMocha)`.
    pub fn mocha() -> Self {
        Self::from_name(ThemeName::CatppuccinMocha)
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

impl Default for Theme {
    fn default() -> Self {
        Self::mocha()
    }
}

/// Convert a Catppuccin palette entry to a ratatui TrueColor. The catppuccin
/// crate ships an optional `ratatui` feature but it targets `ratatui-core`,
/// so we round-trip through RGB components.
#[inline]
fn rgb(c: &CatColor) -> Color {
    Color::Rgb(c.rgb.r, c.rgb.g, c.rgb.b)
}

/// Inline hex-literal helper for the non-catppuccin palettes. Avoids the
/// temptation to sprinkle `Color::Rgb(…)` triples everywhere.
#[inline]
const fn hex(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

// ─── Palette bodies ────────────────────────────────────────────────────────
//
// Each builder returns a fully-populated `Theme`. Keep them pure (no I/O,
// no globals) so the theme-switching path can call them on every keypress.

fn catppuccin_mocha() -> Theme {
    let c = &PALETTE.mocha.colors;
    Theme {
        name: ThemeName::CatppuccinMocha,
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

/// Catppuccin Latte — the light variant. `crust`/`mantle`/`base` are whites;
/// `text` is the near-black `#4c4f69`. Selected-row background becomes a
/// visible tinted stripe instead of a darker shade. Good contrast check
/// against the accent colors, which are deliberately brighter on Latte to
/// stay legible on a white surface.
fn catppuccin_latte() -> Theme {
    let c = &PALETTE.latte.colors;
    Theme {
        name: ThemeName::CatppuccinLatte,
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

/// Dracula — sourced from <https://draculatheme.com/contribute> (the
/// "official spec" table). "mauve" gets pink (`#ff79c6`) which is the
/// theme's primary accent; purple becomes the optional `pink`/`lavender`
/// slot so the secondary tone still reads.
fn dracula() -> Theme {
    let bg = hex(0x28, 0x2a, 0x36); // base
    let current_line = hex(0x44, 0x47, 0x5a); // surface0
    let selection = hex(0x44, 0x47, 0x5a); // surface1 (same hue)
    let foreground = hex(0xf8, 0xf8, 0xf2);
    let comment = hex(0x62, 0x72, 0xa4);
    let cyan = hex(0x8b, 0xe9, 0xfd);
    let green = hex(0x50, 0xfa, 0x7b);
    let orange = hex(0xff, 0xb8, 0x6c);
    let pink = hex(0xff, 0x79, 0xc6);
    let purple = hex(0xbd, 0x93, 0xf9);
    let red = hex(0xff, 0x55, 0x55);
    let yellow = hex(0xf1, 0xfa, 0x8c);

    Theme {
        name: ThemeName::Dracula,
        crust: hex(0x1e, 0x1f, 0x29),
        mantle: hex(0x21, 0x22, 0x2c),
        base: bg,
        surface0: current_line,
        surface1: selection,
        surface2: hex(0x55, 0x58, 0x6a),
        text: foreground,
        subtext1: hex(0xe8, 0xe8, 0xe0),
        subtext0: hex(0xc8, 0xc8, 0xc0),
        overlay2: hex(0x9a, 0xa0, 0xb8),
        overlay1: hex(0x7e, 0x85, 0xa0),
        overlay0: comment,
        mauve: pink, // Dracula's "pink" IS the signature accent
        green,
        yellow,
        blue: cyan,
        peach: orange,
        teal: cyan,
        red,
        pink,
        sky: cyan,
        lavender: purple, // Dracula purple for the "sonnet" pill
    }
}

/// Tokyo Night — the popular Neovim port. Palette from
/// <https://github.com/folke/tokyonight.nvim>.
fn tokyo_night() -> Theme {
    let bg = hex(0x1a, 0x1b, 0x26);
    let bg_dark = hex(0x16, 0x16, 0x1e);
    let fg = hex(0xc0, 0xca, 0xf5);
    let comment = hex(0x56, 0x5f, 0x89);
    let blue = hex(0x7a, 0xa2, 0xf7);
    let cyan = hex(0x7d, 0xcf, 0xff);
    let green = hex(0x9e, 0xce, 0x6a);
    let yellow = hex(0xe0, 0xaf, 0x68);
    let red = hex(0xf7, 0x76, 0x8e);
    let purple = hex(0xbb, 0x9a, 0xf7);
    let magenta = hex(0x9d, 0x7c, 0xd8);
    let orange = hex(0xff, 0x9e, 0x64);

    Theme {
        name: ThemeName::TokyoNight,
        crust: hex(0x0f, 0x10, 0x18),
        mantle: bg_dark,
        base: bg,
        surface0: hex(0x24, 0x28, 0x3b),
        surface1: hex(0x2f, 0x33, 0x4d),
        surface2: hex(0x3b, 0x42, 0x61),
        text: fg,
        subtext1: hex(0xa9, 0xb1, 0xd6),
        subtext0: hex(0x91, 0x9a, 0xca),
        overlay2: hex(0x78, 0x82, 0xb0),
        overlay1: hex(0x69, 0x73, 0xa3),
        overlay0: comment,
        mauve: purple,
        green,
        yellow,
        blue,
        peach: orange,
        teal: cyan,
        red,
        pink: magenta,
        sky: cyan,
        lavender: magenta,
    }
}

/// Gruvbox Dark — "medium" contrast variant; soft palette. From
/// <https://github.com/morhetz/gruvbox>.
fn gruvbox_dark() -> Theme {
    let bg = hex(0x28, 0x28, 0x28);
    let bg0_h = hex(0x1d, 0x20, 0x21);
    let bg1 = hex(0x3c, 0x38, 0x36);
    let bg2 = hex(0x50, 0x49, 0x45);
    let bg3 = hex(0x66, 0x5c, 0x54);
    let fg = hex(0xeb, 0xdb, 0xb2);
    let fg2 = hex(0xd5, 0xc4, 0xa1);
    let fg3 = hex(0xbd, 0xae, 0x93);
    let dim = hex(0x92, 0x83, 0x74);

    let red = hex(0xfb, 0x49, 0x34);
    let green = hex(0xb8, 0xbb, 0x26);
    let yellow = hex(0xfa, 0xbd, 0x2f);
    let blue = hex(0x83, 0xa5, 0x98);
    let purple = hex(0xd3, 0x86, 0x9b);
    let aqua = hex(0x8e, 0xc0, 0x7c);
    let orange = hex(0xfe, 0x80, 0x19);

    Theme {
        name: ThemeName::GruvboxDark,
        crust: hex(0x10, 0x10, 0x10),
        mantle: bg0_h,
        base: bg,
        surface0: bg1,
        surface1: bg2,
        surface2: bg3,
        text: fg,
        subtext1: fg2,
        subtext0: fg3,
        overlay2: hex(0xa8, 0x99, 0x84),
        overlay1: hex(0x95, 0x87, 0x71),
        overlay0: dim,
        mauve: purple,
        green,
        yellow,
        blue,
        peach: orange,
        teal: aqua,
        red,
        pink: purple,
        sky: aqua,
        lavender: purple,
    }
}

/// Nord — arctic, north-bluish palette from <https://www.nordtheme.com/>.
fn nord() -> Theme {
    let nord0 = hex(0x2e, 0x34, 0x40); // polar-night 0 (base)
    let nord1 = hex(0x3b, 0x42, 0x52); // polar-night 1
    let nord2 = hex(0x43, 0x4c, 0x5e);
    let nord3 = hex(0x4c, 0x56, 0x6a);
    let nord4 = hex(0xd8, 0xde, 0xe9);
    let nord5 = hex(0xe5, 0xe9, 0xf0);
    let nord6 = hex(0xec, 0xef, 0xf4);
    let nord7 = hex(0x8f, 0xbc, 0xbb); // frost 1 (teal)
    let nord8 = hex(0x88, 0xc0, 0xd0); // frost 2 (blue)
    let nord9 = hex(0x81, 0xa1, 0xc1);
    let nord10 = hex(0x5e, 0x81, 0xac);
    let nord11 = hex(0xbf, 0x61, 0x6a); // aurora red
    let nord12 = hex(0xd0, 0x87, 0x70); // aurora orange
    let nord13 = hex(0xeb, 0xcb, 0x8b); // aurora yellow
    let nord14 = hex(0xa3, 0xbe, 0x8c); // aurora green
    let nord15 = hex(0xb4, 0x8e, 0xad); // aurora purple
    let comment = hex(0x61, 0x6e, 0x88);

    Theme {
        name: ThemeName::Nord,
        crust: hex(0x24, 0x29, 0x33),
        mantle: nord1,
        base: nord0,
        surface0: nord2,
        surface1: nord3,
        surface2: nord9,
        text: nord6,
        subtext1: nord5,
        subtext0: nord4,
        overlay2: hex(0x9a, 0xa3, 0xb8),
        overlay1: hex(0x7b, 0x87, 0xa0),
        overlay0: comment,
        mauve: nord15,
        green: nord14,
        yellow: nord13,
        blue: nord8,
        peach: nord12,
        teal: nord7,
        red: nord11,
        pink: nord15,
        sky: nord10,
        lavender: nord15,
    }
}

/// Foreground color for the model pill text. Always dark on colored bg so the
/// text remains legible regardless of which family color we picked. For light
/// themes (Latte) this is still the darkest shade in the palette.
pub fn pill_text_color(theme: &Theme) -> Color {
    theme.crust
}

// ─── Persistence ───────────────────────────────────────────────────────────
//
// The user's last-chosen theme is stored at `~/.config/claude-picker/theme`
// as a one-line text file containing the theme label (e.g. `dracula`). The
// `t` keybinding writes this on every cycle so next launch remembers.

/// Env var consulted by [`resolve_theme_name`]. Exported so tests and docs
/// can reference a single source of truth.
pub const THEME_ENV_VAR: &str = "CLAUDE_PICKER_THEME";

/// Path to the one-line persistence file. `None` only if the home dir can't
/// be located (headless CI containers, etc.) — callers should treat that as
/// "no persistence available".
pub fn theme_config_path() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".config").join("claude-picker").join("theme"))
}

/// Load the persisted theme name, if any. Silently returns `None` when the
/// file is missing or malformed — persistence is a convenience, never a
/// hard failure.
pub fn load_persisted_theme() -> Option<ThemeName> {
    let path = theme_config_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    ThemeName::from_str(raw.trim())
}

/// Persist the user's theme choice. Creates `~/.config/claude-picker/` if
/// missing. Errors are surfaced so the caller can toast — the TUI should
/// never crash on a disk-write failure but it's useful to see the reason.
pub fn save_persisted_theme(name: ThemeName) -> std::io::Result<()> {
    let path = theme_config_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "home dir not locatable")
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{}\n", name.label()))
}

/// Resolve which theme to use at startup, honoring CLI → env → persisted →
/// default precedence. `cli` is whatever the user passed via `--theme`; pass
/// `None` when the flag was absent.
///
/// Unknown names are treated as "not set" and fall through to the next
/// source; they don't crash. A CLI-level error path can still be wired in by
/// pre-validating the flag — this function is the fallback chain.
pub fn resolve_theme_name(cli: Option<&str>) -> ThemeName {
    if let Some(raw) = cli {
        if let Some(t) = ThemeName::from_str(raw) {
            return t;
        }
    }
    if let Ok(raw) = std::env::var(THEME_ENV_VAR) {
        if let Some(t) = ThemeName::from_str(&raw) {
            return t;
        }
    }
    if let Some(t) = load_persisted_theme() {
        return t;
    }
    ThemeName::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mocha_is_constructible() {
        // Smoke test — catppuccin conversions round-trip cleanly.
        let t = Theme::mocha();
        // surface0 must differ from base — otherwise selected-row won't pop.
        assert_ne!(t.surface0, t.base);
        assert_eq!(t.name, ThemeName::CatppuccinMocha);
    }

    #[test]
    fn selected_row_has_bold_modifier() {
        let t = Theme::mocha();
        let s = t.selected_row();
        assert!(s.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn theme_name_from_str_is_case_insensitive() {
        assert_eq!(ThemeName::from_str("dracula"), Some(ThemeName::Dracula));
        assert_eq!(ThemeName::from_str("DRACULA"), Some(ThemeName::Dracula));
        assert_eq!(ThemeName::from_str("Dracula"), Some(ThemeName::Dracula));
        assert_eq!(
            ThemeName::from_str("TOKYO-night"),
            Some(ThemeName::TokyoNight)
        );
    }

    #[test]
    fn theme_name_from_str_rejects_unknown() {
        assert!(ThemeName::from_str("solarized").is_none());
        assert!(ThemeName::from_str("").is_none());
        assert!(ThemeName::from_str("moch").is_none());
    }

    #[test]
    fn theme_name_next_cycles_and_wraps() {
        // Cycle through ALL and confirm we wrap back.
        let mut seen = Vec::with_capacity(ThemeName::ALL.len());
        let start = ThemeName::CatppuccinMocha;
        let mut cur = start;
        for _ in 0..ThemeName::ALL.len() {
            seen.push(cur);
            cur = cur.next();
        }
        // After N .next() calls we should be back at the start.
        assert_eq!(cur, start);
        // And we should have visited each variant exactly once.
        assert_eq!(seen.len(), ThemeName::ALL.len());
        for want in ThemeName::ALL {
            assert!(seen.contains(want), "missing {:?}", want);
        }
    }

    #[test]
    fn all_themes_parse_from_label() {
        for want in ThemeName::ALL {
            let label = want.label();
            assert_eq!(
                ThemeName::from_str(label),
                Some(*want),
                "label {label} didn't round-trip"
            );
        }
    }

    #[test]
    fn every_theme_constructs_and_sets_name() {
        for &want in ThemeName::ALL {
            let theme = Theme::from_name(want);
            assert_eq!(theme.name, want);
            // Spot-check that surface0 differs from base — the selected-row
            // contrast invariant needs to hold for every palette.
            assert_ne!(
                theme.surface0, theme.base,
                "{:?}: surface0 == base would flatten selected-row",
                want
            );
        }
    }

    #[test]
    fn latte_is_a_light_theme() {
        // Sanity: Latte's base should be near-white and text near-black, so
        // user-written screens don't invert inadvertently.
        let t = Theme::from_name(ThemeName::CatppuccinLatte);
        let Color::Rgb(br, bg, bb) = t.base else {
            panic!("expected rgb")
        };
        let Color::Rgb(tr, tg, tb) = t.text else {
            panic!("expected rgb")
        };
        let bg_lum = (br as u32) + (bg as u32) + (bb as u32);
        let text_lum = (tr as u32) + (tg as u32) + (tb as u32);
        assert!(bg_lum > text_lum, "Latte base must be brighter than text");
    }

    #[test]
    fn mocha_is_a_dark_theme() {
        let t = Theme::mocha();
        let Color::Rgb(br, bg, bb) = t.base else {
            panic!("expected rgb")
        };
        let Color::Rgb(tr, tg, tb) = t.text else {
            panic!("expected rgb")
        };
        let bg_lum = (br as u32) + (bg as u32) + (bb as u32);
        let text_lum = (tr as u32) + (tg as u32) + (tb as u32);
        assert!(bg_lum < text_lum, "Mocha text must be brighter than base");
    }
}
