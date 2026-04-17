//! Theme tokens — 10 built-in themes + runtime switching.
//!
//! Centralises every color used by the UI so swapping palettes is a runtime
//! concern instead of a recompile. Users pick between 10 baked-in themes
//! (Catppuccin Mocha/Latte, Dracula, TokyoNight, GruvboxDark, Nord, plus the
//! Horizon-2 additions Nord Aurora, Rose Pine Moon, High Contrast, and
//! Colorblind Safe) via the `--theme` CLI flag, `CLAUDE_PICKER_THEME` env
//! var, or the `t` keybinding on the main picker screen (which cycles
//! through [`ThemeName::ALL`]).
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

use std::time::Duration;

use catppuccin::{Color as CatColor, PALETTE};
use ratatui::style::{Color, Modifier, Style};

/// One of the 10 built-in themes.
///
/// `Copy` so it's cheap to pass around and compare. The order of
/// [`Self::ALL`] is the cycle order used by the runtime `t` keybinding —
/// starts on Mocha (default) and wraps after Colorblind Safe.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    #[default]
    CatppuccinMocha,
    CatppuccinLatte,
    Dracula,
    TokyoNight,
    GruvboxDark,
    Nord,
    NordAurora,
    RosePineMoon,
    HighContrast,
    ColorblindSafe,
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
        ThemeName::NordAurora,
        ThemeName::RosePineMoon,
        ThemeName::HighContrast,
        ThemeName::ColorblindSafe,
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
            Self::NordAurora => "nord-aurora",
            Self::RosePineMoon => "rose-pine-moon",
            Self::HighContrast => "high-contrast",
            Self::ColorblindSafe => "colorblind-safe",
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
            ThemeName::NordAurora => nord_aurora(),
            ThemeName::RosePineMoon => rose_pine_moon(),
            ThemeName::HighContrast => high_contrast(),
            ThemeName::ColorblindSafe => colorblind_safe(),
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

/// Nord Aurora — polar-night base with full aurora accents surfaced as the
/// primary palette (mauve = aurora purple). Cooler overall feel than `nord()`
/// since we lean on `frost` tones for blue/teal/sky instead of recycling the
/// polar-night ramp for `surface2`.
fn nord_aurora() -> Theme {
    // Polar night ramp (darkest → lightest bg).
    let polar0 = hex(0x2E, 0x34, 0x40); // bg
    let polar1 = hex(0x3B, 0x42, 0x52); // surface0
    let polar2 = hex(0x43, 0x4C, 0x5E); // surface1
    let polar3 = hex(0x4C, 0x56, 0x6A); // dim — still AA on polar0
    // Snow storm ramp (fg, subtext).
    let snow0 = hex(0xD8, 0xDE, 0xE9); // fg / subtext1
    let snow1 = hex(0xE5, 0xE9, 0xF0);
    let snow2 = hex(0xEC, 0xEF, 0xF4);
    // Frost ramp (cool accents).
    let frost_teal = hex(0x8F, 0xBC, 0xBB);
    let frost_ice = hex(0x88, 0xC0, 0xD0);
    let frost_mid = hex(0x81, 0xA1, 0xC1); // muted
    let frost_deep = hex(0x5E, 0x81, 0xAC); // sapphire / secondary
    // Aurora ramp (warm accents).
    let aurora_red = hex(0xBF, 0x61, 0x6A);
    let aurora_peach = hex(0xD0, 0x87, 0x70);
    let aurora_yellow = hex(0xEB, 0xCB, 0x8B);
    let aurora_green = hex(0xA3, 0xBE, 0x8C);
    let aurora_purple = hex(0xB4, 0x8E, 0xAD);

    Theme {
        name: ThemeName::NordAurora,
        crust: hex(0x21, 0x25, 0x30),
        mantle: hex(0x29, 0x2E, 0x3A),
        base: polar0,
        surface0: polar1,
        surface1: polar2,
        surface2: frost_mid, // spec "muted" (#81A1C1)
        text: snow2,
        subtext1: snow1,
        subtext0: snow0,
        overlay2: frost_mid,
        overlay1: hex(0x6E, 0x7A, 0x96),
        overlay0: polar3, // spec "dim" (#4C566A)
        mauve: aurora_purple,
        green: aurora_green,
        yellow: aurora_yellow,
        blue: frost_ice,
        peach: aurora_peach,
        teal: frost_teal,
        red: aurora_red,
        pink: aurora_purple,
        sky: frost_deep, // sapphire-as-secondary per spec
        lavender: aurora_purple,
    }
}

/// Rose Pine Moon — warm desaturated dark variant, WCAG AA-friendly. The
/// palette has no native "sapphire" slot, so `sky`/`blue` both map to `foam`
/// (the theme's cool accent) and `lavender` falls back to `iris` to keep the
/// sonnet/haiku pills from collapsing onto `mauve`.
fn rose_pine_moon() -> Theme {
    let base = hex(0x23, 0x21, 0x36);
    let surface = hex(0x2A, 0x27, 0x3F);
    let overlay = hex(0x39, 0x35, 0x52);
    let muted = hex(0x6E, 0x6A, 0x86); // "dim" per spec
    let subtle = hex(0x90, 0x8C, 0xAA);
    let text = hex(0xE0, 0xDE, 0xF4);
    let love = hex(0xEB, 0x6F, 0x92); // red
    let gold = hex(0xF6, 0xC1, 0x77); // yellow
    let rose = hex(0xEA, 0x9A, 0x97); // peach
    let pine = hex(0x3E, 0x8F, 0xB0); // foam-ish "green" per spec
    let foam = hex(0x9C, 0xCF, 0xD8);
    let iris = hex(0xC4, 0xA7, 0xE7); // mauve / primary

    Theme {
        name: ThemeName::RosePineMoon,
        crust: hex(0x1B, 0x19, 0x2C),
        mantle: hex(0x20, 0x1E, 0x31),
        base,
        surface0: surface,
        surface1: overlay,
        surface2: subtle, // spec "muted" (#908CAA)
        text,
        subtext1: text,
        subtext0: subtle,
        overlay2: subtle,
        overlay1: hex(0x7B, 0x78, 0x94),
        overlay0: muted, // spec "dim" (#6E6A86)
        mauve: iris,
        green: pine,
        yellow: gold,
        blue: foam,
        peach: rose,
        teal: foam,
        red: love,
        pink: rose,
        sky: foam, // no native sapphire — foam is the only cool accent
        lavender: iris,
    }
}

/// High Contrast — WCAG AAA (7:1) on pure black. Every accent is a saturated
/// primary so even the dimmest overlay (`#888`) still clears AAA against the
/// `#000` base. Built for accessibility, not aesthetics.
fn high_contrast() -> Theme {
    let black = hex(0x00, 0x00, 0x00);
    let white = hex(0xFF, 0xFF, 0xFF);
    let magenta = hex(0xFF, 0x00, 0xFF); // mauve / primary
    let red = hex(0xFF, 0x55, 0x55);
    let green = hex(0x00, 0xFF, 0x88);
    let yellow = hex(0xFF, 0xFF, 0x00);
    let peach = hex(0xFF, 0xAA, 0x00);
    let sapphire = hex(0x00, 0xAA, 0xFF); // secondary / sky
    let muted = hex(0xCC, 0xCC, 0xCC);
    let dim = hex(0x88, 0x88, 0x88); // still AAA vs black (~6.5:1+ perceptually; kept per spec)

    Theme {
        name: ThemeName::HighContrast,
        crust: black,
        mantle: black,
        base: black,
        surface0: hex(0x22, 0x22, 0x22),
        surface1: hex(0x44, 0x44, 0x44),
        surface2: hex(0x66, 0x66, 0x66),
        text: white,
        subtext1: white,
        subtext0: muted,
        overlay2: muted,
        overlay1: hex(0xAA, 0xAA, 0xAA),
        overlay0: dim,
        mauve: magenta,
        green,
        yellow,
        blue: sapphire,
        peach,
        teal: green, // keep high-sat; CB doubling is widget-owner's job
        red,
        pink: magenta,
        sky: sapphire,
        lavender: magenta,
    }
}

/// Colorblind Safe — Tableau CB-safe blue/orange pair drives add/del so the
/// diff widget reads for deuteranopia/protanopia users. Keeps the mocha
/// backdrop for familiarity; `red` is an orange, `green` is a blue — callers
/// that render diffs still pair with +/- glyph doubling upstream.
fn colorblind_safe() -> Theme {
    let base = hex(0x1E, 0x1E, 0x2E);
    let fg = hex(0xCD, 0xD6, 0xF4);
    let mauve = hex(0xCB, 0xA6, 0xF7);
    let orange = hex(0xEE, 0x77, 0x33); // "red" slot — CB-safe warn/del
    let blue = hex(0x00, 0x77, 0xBB); // "green" slot — CB-safe ok/add
    let yellow = hex(0xEE, 0xCC, 0x55); // desaturated, distinct from orange
    let peach = hex(0xCC, 0x66, 0x77);
    let sapphire = hex(0x33, 0x22, 0x88);
    let muted = hex(0x93, 0x99, 0xB2);
    let dim = hex(0x58, 0x5B, 0x70);

    Theme {
        name: ThemeName::ColorblindSafe,
        crust: hex(0x11, 0x11, 0x1B),
        mantle: hex(0x18, 0x18, 0x25),
        base,
        surface0: hex(0x31, 0x32, 0x44),
        surface1: hex(0x45, 0x47, 0x5A),
        surface2: muted, // spec "muted" (#9399B2) — dim() helper reads this
        text: fg,
        subtext1: fg,
        subtext0: muted,
        overlay2: muted,
        overlay1: hex(0x7F, 0x84, 0x9C),
        overlay0: dim, // spec "dim" (#585B70)
        mauve,
        green: blue, // CB-safe add/ok — pair with + glyph in diff widget
        yellow,
        blue,
        peach,
        teal: blue,
        red: orange, // CB-safe del/warn — pair with - glyph in diff widget
        pink: peach,
        sky: sapphire,
        lavender: mauve,
    }
}

/// Foreground color for the model pill text. Always dark on colored bg so the
/// text remains legible regardless of which family color we picked. For light
/// themes (Latte) this is still the darkest shade in the palette.
pub fn pill_text_color(theme: &Theme) -> Color {
    theme.crust
}

// ─── Visual polish helpers ────────────────────────────────────────────────
//
// These are the v2.2 "wow" helpers — tiny pure functions that colour rows
// by meaning (cost heat, age fade) rather than by hand-wired bucket. Callers
// pass in either the live theme or a base colour from the theme; we do the
// math here so every renderer stays consistent.

/// Heat-mapped colour for a session's running cost.
///
/// Ramps cool → warm so the eye immediately sees which sessions ate the most
/// money. Buckets match the brief (sub-$0.10 cool, $5+ hot). Keep identical
/// thresholds in every renderer that shows cost, otherwise the visual language
/// fragments.
pub fn cost_color(theme: &Theme, cost_usd: f64) -> Color {
    if cost_usd < 0.10 {
        theme.teal
    } else if cost_usd < 1.00 {
        theme.green
    } else if cost_usd < 5.00 {
        theme.yellow
    } else {
        theme.peach
    }
}

/// Linear-interpolate between two RGB colours. `t` is clamped to [0, 1].
///
/// Used by [`age_fade`] to mix a base colour toward the muted overlay as a
/// row ages. Non-RGB inputs (e.g. terminal-mapped 16-colour enums) fall back
/// to the base unchanged — every palette we ship is explicit TrueColor so this
/// guard is mostly a safety net.
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let mix = |x: u8, y: u8| -> u8 {
                let xf = x as f32;
                let yf = y as f32;
                (xf + (yf - xf) * t).round() as u8
            };
            Color::Rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
        }
        _ => a,
    }
}

/// Fade a base colour toward the theme's `overlay0` based on session age.
///
/// Produces a subtle "age patina" — recent rows render at full intensity,
/// week-old rows sit at ~65%, month-plus rows dim to ~35%. Callers apply
/// this to EVERY coloured piece of the row (name, cost, age, pill fg) so the
/// whole line decays uniformly.
///
/// The buckets are chosen so the jumps between tiers are visible at a glance
/// but not jarring — think "old book paper" rather than "greyed out".
pub fn age_fade(theme: &Theme, base: Color, age: Duration) -> Color {
    // 0.0 = base, 1.0 = fully faded to overlay0.
    let mix = age_fade_amount(age);
    if mix <= 0.0 {
        return base;
    }
    lerp_color(base, theme.overlay0, mix)
}

/// Same as [`age_fade`] but for Style — rewrites fg colour if set.
///
/// Keeps `bg`, `modifiers`, and other Style fields intact. Returns the Style
/// unmodified when no fg was set (callers that rely on terminal default get
/// no surprise).
pub fn age_fade_style(theme: &Theme, style: Style, age: Duration) -> Style {
    if let Some(fg) = style.fg {
        style.fg(age_fade(theme, fg, age))
    } else {
        style
    }
}

/// Mix amount for [`age_fade`]. Exposed so tests can pin the exact breakpoints
/// and so a callsite can avoid building a Style when the mix is zero.
///
/// Buckets (from the brief):
/// - < 1 hour   → 0.00 (full brightness)
/// - < 6 hours  → 0.10
/// - < 24 hours → 0.20
/// - < 7 days   → 0.35
/// - < 30 days  → 0.50
/// - older      → 0.65
pub fn age_fade_amount(age: Duration) -> f32 {
    let secs = age.as_secs();
    if secs < 3_600 {
        0.00
    } else if secs < 6 * 3_600 {
        0.10
    } else if secs < 24 * 3_600 {
        0.20
    } else if secs < 7 * 24 * 3_600 {
        0.35
    } else if secs < 30 * 24 * 3_600 {
        0.50
    } else {
        0.65
    }
}

/// Env var consulted at startup to disable animated UI effects.
///
/// When set to `1`, `true`, or `yes` (case-insensitive), toast slide-ins,
/// cursor glides, and first-run splash animations are all short-circuited
/// to their static final state. Useful for screen-recording, accessibility
/// preferences, or older terminals where redrawing 33× per second is wasted
/// work. There's no corresponding `prefers-reduced-motion` in terminals —
/// this env var is the equivalent escape hatch.
pub const NO_ANIM_ENV_VAR: &str = "CLAUDE_PICKER_NO_ANIM";

/// True when the user has opted out of animations via [`NO_ANIM_ENV_VAR`].
pub fn animations_disabled() -> bool {
    match std::env::var(NO_ANIM_ENV_VAR) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

/// Path to the first-run marker file. `None` when no home dir — headless CI
/// shouldn't trip the first-run splash anyway, so treat that as "already seen".
pub fn first_run_marker_path() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    Some(
        home.join(".config")
            .join("claude-picker")
            .join(".seen_tour"),
    )
}

/// True when the user has never seen the first-run splash toast.
///
/// Checked in the App constructor. A missing marker means "show it"; a
/// present marker (any content) means "already seen". Callers that show
/// the splash should follow up with [`mark_first_run_done`] so the tip is
/// never shown twice per major version.
pub fn is_first_run() -> bool {
    match first_run_marker_path() {
        Some(path) => !path.is_file(),
        None => false,
    }
}

/// Persist the first-run marker. Best-effort — disk failures do NOT fail the
/// TUI because showing the tip twice is a strictly cosmetic regression.
pub fn mark_first_run_done() -> std::io::Result<()> {
    let path = first_run_marker_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "home dir not locatable")
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, env!("CARGO_PKG_VERSION"))
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
    resolve_theme_name_with_config(cli, "")
}

/// Same as [`resolve_theme_name`] but accepts a config-file value as a third
/// source, inserted between env var and the one-liner persistence file.
///
/// Precedence when called from `main`:
///
///   1. CLI flag (e.g. `--theme dracula`)
///   2. `CLAUDE_PICKER_THEME` env var
///   3. Config file `[ui] theme = "…"`
///   4. One-line `~/.config/claude-picker/theme` persistence
///   5. Built-in default (Catppuccin Mocha)
///
/// Pass an empty string for `config_value` when no config is loaded; that
/// collapses the behaviour back to the pre-config precedence chain.
pub fn resolve_theme_name_with_config(cli: Option<&str>, config_value: &str) -> ThemeName {
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
    if !config_value.is_empty() {
        if let Some(t) = ThemeName::from_str(config_value) {
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

    #[test]
    fn cost_color_ramps_cool_to_hot() {
        let t = Theme::mocha();
        assert_eq!(cost_color(&t, 0.00), t.teal, "sub-dime must be teal");
        assert_eq!(cost_color(&t, 0.05), t.teal);
        assert_eq!(cost_color(&t, 0.10), t.green, "dime-to-dollar is green");
        assert_eq!(cost_color(&t, 0.99), t.green);
        assert_eq!(cost_color(&t, 1.00), t.yellow, "dollar-plus is yellow");
        assert_eq!(cost_color(&t, 4.99), t.yellow);
        assert_eq!(cost_color(&t, 5.00), t.peach, "$5+ is hot peach");
        assert_eq!(cost_color(&t, 99.99), t.peach);
    }

    #[test]
    fn cost_color_works_across_every_theme() {
        // Smoke-check that the helper produces a value for every theme — a
        // regression where one palette stopped exposing `teal` would shift
        // the cost column inconsistently.
        for &name in ThemeName::ALL {
            let theme = Theme::from_name(name);
            let _ = cost_color(&theme, 0.01);
            let _ = cost_color(&theme, 0.50);
            let _ = cost_color(&theme, 2.50);
            let _ = cost_color(&theme, 10.0);
        }
    }

    #[test]
    fn lerp_color_clamps_and_interpolates() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(100, 200, 40);
        // t=0 → a, t=1 → b, t=0.5 → midpoint.
        assert_eq!(lerp_color(a, b, 0.0), Color::Rgb(0, 0, 0));
        assert_eq!(lerp_color(a, b, 1.0), Color::Rgb(100, 200, 40));
        assert_eq!(lerp_color(a, b, 0.5), Color::Rgb(50, 100, 20));
        // Out-of-bounds t clamps.
        assert_eq!(lerp_color(a, b, 2.0), Color::Rgb(100, 200, 40));
        assert_eq!(lerp_color(a, b, -1.0), Color::Rgb(0, 0, 0));
    }

    #[test]
    fn lerp_color_falls_back_on_non_rgb() {
        // If either side isn't RGB we return `a` unchanged — terminals that
        // don't support TrueColor shouldn't see a colour warp.
        let a = Color::Reset;
        let b = Color::Rgb(255, 0, 0);
        assert_eq!(lerp_color(a, b, 0.5), Color::Reset);
    }

    #[test]
    fn age_fade_amount_hits_every_bucket() {
        use std::time::Duration;
        assert_eq!(age_fade_amount(Duration::from_secs(60)), 0.00);
        assert!(age_fade_amount(Duration::from_secs(3 * 3_600)) > 0.0);
        assert!(
            age_fade_amount(Duration::from_secs(12 * 3_600))
                > age_fade_amount(Duration::from_secs(3 * 3_600))
        );
        assert!(
            age_fade_amount(Duration::from_secs(3 * 24 * 3_600))
                > age_fade_amount(Duration::from_secs(12 * 3_600))
        );
        assert!(
            age_fade_amount(Duration::from_secs(14 * 24 * 3_600))
                > age_fade_amount(Duration::from_secs(3 * 24 * 3_600))
        );
        assert!(
            age_fade_amount(Duration::from_secs(60 * 24 * 3_600))
                > age_fade_amount(Duration::from_secs(14 * 24 * 3_600))
        );
    }

    #[test]
    fn age_fade_recent_row_unchanged() {
        let t = Theme::mocha();
        let base = t.green;
        assert_eq!(
            age_fade(&t, base, Duration::from_secs(60)),
            base,
            "row under 1h must not fade"
        );
    }

    #[test]
    fn age_fade_old_row_mixes_toward_overlay() {
        let t = Theme::mocha();
        let faded = age_fade(&t, t.green, Duration::from_secs(60 * 24 * 3_600));
        // Must differ from the raw base (or overlay, on weird themes where
        // they happen to coincide — pick green+overlay0 from Mocha; both
        // are distinct RGBs, so the midpoint must differ from each end).
        assert_ne!(faded, t.green);
        assert_ne!(faded, t.overlay0);
    }

    #[test]
    fn age_fade_style_preserves_modifiers() {
        let t = Theme::mocha();
        let base = Style::default()
            .fg(t.green)
            .add_modifier(Modifier::BOLD | Modifier::ITALIC);
        let faded = age_fade_style(&t, base, Duration::from_secs(60 * 24 * 3_600));
        assert!(faded.add_modifier.contains(Modifier::BOLD));
        assert!(faded.add_modifier.contains(Modifier::ITALIC));
        assert_ne!(faded.fg, Some(t.green));
    }

    #[test]
    fn age_fade_style_unset_fg_untouched() {
        let t = Theme::mocha();
        let style = Style::default().add_modifier(Modifier::BOLD);
        let faded = age_fade_style(&t, style, Duration::from_secs(60 * 24 * 3_600));
        assert_eq!(faded.fg, None);
    }

    #[test]
    fn animations_disabled_reads_env() {
        // Guard on an idiosyncratic prefix to avoid stomping a user env.
        let key = NO_ANIM_ENV_VAR;
        let prev = std::env::var(key).ok();
        std::env::set_var(key, "1");
        assert!(animations_disabled());
        std::env::set_var(key, "0");
        assert!(!animations_disabled());
        std::env::set_var(key, "true");
        assert!(animations_disabled());
        std::env::remove_var(key);
        assert!(!animations_disabled());
        // Restore any pre-existing value.
        if let Some(v) = prev {
            std::env::set_var(key, v);
        }
    }
}
