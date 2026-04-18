//! F2/E17 project thumbnails — per-project "identicon" tiles for the pinned
//! strip at the top of the project-list screen.
//!
//! Each project basename hashes to a deterministic 4×4 symmetric pattern
//! (top-left quadrant randomised, the rest mirrors) and is painted into a
//! 64×64 pixel buffer using a palette derived from the active theme's
//! accent colors. We render the buffer as Unicode halfblocks so every
//! terminal — kitty, iTerm2, tmux, ssh, CI, VHS recorders — shows the
//! same tile. No graphics-protocol probe, no C library dependencies.
//!
//! # Layout of a rendered tile
//!
//! ```text
//! [ 1: ░▓░▓ architex ]     ← halfblocks tile + slot number + basename
//! ```
//!
//! The caller (currently `ui::project_list`) decides spacing, active-slot
//! outlining, and narrow-terminal degradation. This module only owns:
//!
//! - [`ThumbnailRenderer`] — wraps the LRU cache. Constructed once at
//!   startup; cheap to clone-ref.
//! - [`identicon_image`] — deterministic `DynamicImage` for a basename under
//!   a given [`IdenticonPalette`]. Kept even though the halfblock path
//!   reads from the grid directly — other call sites (tests, future
//!   exporters) still want the RGB buffer.
//! - [`halfblock_lines`] — lines-ready-for-ratatui halfblock rendering.
//! - [`render_pinned_strip_with_thumbnails`] — the public entry point the
//!   project-list screen calls.
//!
//! # Deliberate simplifications
//!
//! - **Hash**: `std::collections::hash_map::DefaultHasher` over the
//!   basename bytes. Stable within a single Rust stdlib version (stable
//!   enough for spatial-memory purposes across a single session), and zero
//!   added dependencies. The alternative — pulling in `sha2` or `blake3`
//!   just for a 64-bit decorative hash — is overkill.
//! - **Reduce-motion**: thumbnails are static by construction; there's
//!   nothing to disable.
//! - **No emojis**: the basename label passes through as-is; the renderer
//!   never inserts glyphs.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

use image::{DynamicImage, Rgb, RgbImage};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::text::truncate_to_width;
use crate::ui::thumbnail_cache::{ThumbKey, ThumbnailCache};

/// Side length of the identicon grid in logical cells. 4×4 gives 16 cells,
/// with 4-way symmetry that makes each identicon trivially recognisable.
const GRID: u32 = 4;

/// Pixel scale factor per cell for the image-protocol path. A 16× scale
/// yields a 64×64 pixel buffer — big enough that terminal image backends
/// anti-alias it cleanly into the 4-column-wide tile slot.
const CELL_PX: u32 = 16;

/// Minimum terminal width for rendering the full thumbnail strip. Below
/// this we hide thumbnails entirely (caller decides whether to fall back
/// to a text-only strip). Matches the existing `PINNED_STRIP_FULL_WIDTH`
/// threshold in `project_list` on purpose.
pub const MIN_STRIP_WIDTH: u16 = 80;

/// Width in terminal columns one thumbnail occupies (image or halfblocks).
/// Halfblocks render one cell per column × two cells per row, so a 4×4
/// grid lands in 4 columns × 2 rows — which also matches the spec's
/// `4×2` footprint.
pub const TILE_COLS: u16 = 4;

/// Height in terminal rows one tile occupies.
pub const TILE_ROWS: u16 = 2;

/// Label budget per pinned slot: "N: " prefix + truncated basename.
/// Shared between the strip-layout math and the per-slot renderer so
/// the budgets stay in sync if we ever widen them.
const LABEL_MAX: usize = 14;

/// A 4×4 boolean grid with 4-way symmetry. Index as `cells[y * 4 + x]`.
///
/// Pulling the grid out lets us unit-test the mirror logic without
/// round-tripping through a `DynamicImage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdenticonGrid {
    pub cells: [bool; (GRID * GRID) as usize],
    /// Secondary palette selector — picked from hash byte 5. Caller maps
    /// this to a theme token (peach / green / yellow).
    pub secondary: u8,
}

/// Palette used when painting an [`IdenticonGrid`] to a [`DynamicImage`].
/// Built from the active theme's accent tokens; see
/// [`IdenticonPalette::from_theme`].
#[derive(Debug, Clone, Copy)]
pub struct IdenticonPalette {
    pub bg: Rgb<u8>,
    pub primary: Rgb<u8>,
    pub secondary: [Rgb<u8>; 3],
}

impl IdenticonPalette {
    /// Build a palette from a theme. Primary uses mauve; secondary rotates
    /// through peach / green / yellow so adjacent pinned projects are
    /// visually distinct even when their grids happen to alias.
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            bg: rgb_from_color(theme.base, Rgb([24, 24, 37])),
            primary: rgb_from_color(theme.mauve, Rgb([203, 166, 247])),
            secondary: [
                rgb_from_color(theme.peach, Rgb([250, 179, 135])),
                rgb_from_color(theme.green, Rgb([166, 227, 161])),
                rgb_from_color(theme.yellow, Rgb([249, 226, 175])),
            ],
        }
    }

    /// A stable hash of the palette bytes. The [`ThumbnailCache`] folds
    /// this into its key so a theme switch invalidates cached entries
    /// without us having to track the previous theme.
    pub fn hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        (self.bg.0, self.primary.0).hash(&mut h);
        for s in self.secondary {
            s.0.hash(&mut h);
        }
        h.finish()
    }
}

/// Convert a ratatui [`Color`] into an RGB triple, falling back to a
/// Catppuccin-Mocha-ish default when the color is a 16-color index (which
/// carries no RGB info). We prefer the explicit fallback to an unwrap —
/// themes that lean on ANSI indices still produce a legible identicon.
fn rgb_from_color(c: Color, fallback: Rgb<u8>) -> Rgb<u8> {
    match c {
        Color::Rgb(r, g, b) => Rgb([r, g, b]),
        _ => fallback,
    }
}

/// Compute the identicon grid for `basename`.
///
/// Algorithm:
/// 1. Hash the basename bytes with `DefaultHasher`. 64 bits is plenty for
///    a 4-bit-per-cell pattern with 4-way symmetry (≈ 12 pattern bits +
///    1 palette byte).
/// 2. Top 8 bits → 2×2 top-left quadrant (bits 0..4). Remaining bits pick
///    the palette secondary color.
/// 3. Mirror horizontally and vertically so the 4×4 grid has 4-way
///    symmetry. Users build spatial memory via the overall shape, not
///    individual cells.
pub fn identicon_grid(basename: &str) -> IdenticonGrid {
    let mut h = DefaultHasher::new();
    basename.as_bytes().hash(&mut h);
    let hash = h.finish();

    // First nibble selects the 4 cells of the top-left 2×2 quadrant.
    // We take bits 0..4 for the quadrant and byte 5 for the palette.
    let quad_bits = (hash & 0b1111) as u8 | (((hash >> 4) & 0b1111) as u8) << 4;
    let secondary = ((hash >> 40) & 0xff) as u8;

    let mut cells = [false; 16];
    // Fill the top-left 2×2 (positions (0,0), (1,0), (0,1), (1,1)).
    for i in 0..4 {
        let on = (quad_bits >> i) & 1 == 1;
        let x = i % 2;
        let y = i / 2;
        cells[(y * GRID as usize) + x] = on;
    }
    // Mirror horizontally: (x,y) → (3-x, y).
    for y in 0..2 {
        for x in 0..2 {
            let on = cells[(y * GRID as usize) + x];
            cells[(y * GRID as usize) + (3 - x)] = on;
        }
    }
    // Mirror vertically: (x,y) → (x, 3-y).
    for y in 0..2 {
        for x in 0..GRID as usize {
            let on = cells[(y * GRID as usize) + x];
            cells[((3 - y) * GRID as usize) + x] = on;
        }
    }

    IdenticonGrid { cells, secondary }
}

/// Paint `grid` into a 64×64 RGB image using `palette`. The buffer is
/// kept around for the identicon cache so theme-consumer APIs (tests,
/// future exporters) can read the pixel data directly.
pub fn identicon_image(grid: &IdenticonGrid, palette: &IdenticonPalette) -> DynamicImage {
    let size = GRID * CELL_PX;
    let mut buf = RgbImage::from_pixel(size, size, palette.bg);
    // Alternate primary/secondary across the grid so the tile has two
    // accent tones instead of just one.
    let sec_idx = (grid.secondary as usize) % palette.secondary.len();
    let sec = palette.secondary[sec_idx];

    for cy in 0..GRID as usize {
        for cx in 0..GRID as usize {
            if !grid.cells[cy * GRID as usize + cx] {
                continue;
            }
            // Chequerboard-pick between primary and secondary so pixels
            // that happen to form a solid block still read as two tones.
            let color = if (cx + cy) % 2 == 0 {
                palette.primary
            } else {
                sec
            };
            for py in 0..CELL_PX {
                for px in 0..CELL_PX {
                    let x = cx as u32 * CELL_PX + px;
                    let y = cy as u32 * CELL_PX + py;
                    buf.put_pixel(x, y, color);
                }
            }
        }
    }
    DynamicImage::ImageRgb8(buf)
}

/// Render the halfblock fallback for one identicon. Returns 2 lines that
/// the caller can inline into a ratatui `Line` sequence.
///
/// A 4×4 grid collapses cleanly into 2 halfblock lines: each glyph paints
/// one cell above (foreground) and one below (background) using
/// U+2580 UPPER HALF BLOCK.
pub fn halfblock_lines<'a>(grid: &IdenticonGrid, palette: &IdenticonPalette) -> [Line<'a>; 2] {
    let sec_idx = (grid.secondary as usize) % palette.secondary.len();
    let sec = palette.secondary[sec_idx];
    let line_for = |rows: (usize, usize)| -> Line<'a> {
        let mut spans = Vec::with_capacity(GRID as usize);
        for cx in 0..GRID as usize {
            let top_on = grid.cells[rows.0 * GRID as usize + cx];
            let bot_on = grid.cells[rows.1 * GRID as usize + cx];
            let fg = if top_on {
                rgb_to_color(if (cx + rows.0) % 2 == 0 {
                    palette.primary
                } else {
                    sec
                })
            } else {
                rgb_to_color(palette.bg)
            };
            let bg = if bot_on {
                rgb_to_color(if (cx + rows.1) % 2 == 0 {
                    palette.primary
                } else {
                    sec
                })
            } else {
                rgb_to_color(palette.bg)
            };
            spans.push(Span::styled(
                "\u{2580}",
                Style::default().fg(fg).bg(bg),
            ));
        }
        Line::from(spans)
    };
    [line_for((0, 1)), line_for((2, 3))]
}

fn rgb_to_color(c: Rgb<u8>) -> Color {
    Color::Rgb(c.0[0], c.0[1], c.0[2])
}

/// Top-level renderer: owns the LRU of rendered images. One instance
/// lives in `App` and is passed into
/// [`render_pinned_strip_with_thumbnails`] on every project-list frame.
///
/// Cheap to hold: the cache caps itself at [`CACHE_CAP`] entries and
/// each entry is a 64×64 RGB buffer (~12 KB).
pub struct ThumbnailRenderer {
    cache: ThumbnailCache,
}

impl ThumbnailRenderer {
    /// Construct a fresh renderer. Previously this probed stdio for a
    /// graphics protocol (kitty / iTerm2 / sixel); we render every tile
    /// as Unicode halfblocks now so no probe is needed. Halfblocks look
    /// the same across CI, tmux, ssh, kitty, and iTerm2 — one rendering
    /// path, no C library dependencies, no pkg-config at build time.
    pub fn new() -> Self {
        Self {
            cache: ThumbnailCache::new(),
        }
    }

    /// Drop the cached images. Called on theme switch; cheap but
    /// guarantees the next frame repaints with fresh palette tones.
    pub fn invalidate(&mut self) {
        self.cache.clear();
    }

    /// Look up — or create — the identicon [`DynamicImage`] for
    /// `basename` under `palette`. Always succeeds; the cache just
    /// amortises the 64×64 fill across frames.
    pub fn image_for(&mut self, basename: &str, palette: &IdenticonPalette) -> Arc<DynamicImage> {
        let key = ThumbKey::new(basename, palette.hash());
        if let Some(hit) = self.cache.get(&key) {
            return hit;
        }
        let grid = identicon_grid(basename);
        let img = Arc::new(identicon_image(&grid, palette));
        self.cache.insert(key, img.clone());
        img
    }
}

impl Default for ThumbnailRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Basename of a path for identicon hashing. We deliberately strip trailing
/// slashes and any empty segments so `/work/architex/` and `/work/architex`
/// hash identically.
pub fn basename_for_path(p: &Path) -> String {
    p.components()
        .rev()
        .find_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// Same thing but for the `/path/string/` form that `PinnedProjects`
/// stores on disk.
pub fn basename_for_cwd(cwd: &str) -> String {
    cwd.rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(cwd)
        .to_string()
}

/// One slot in the pinned strip. The `project_list` module flattens its
/// `(slot, cwd)` iterator into this shape before handing us the list so
/// the renderer stays agnostic of the storage type.
pub struct PinnedSlot<'a> {
    pub slot: u8,
    pub basename: &'a str,
    pub is_active: bool,
}

/// Public entry point for the project-list screen. Renders the pinned
/// strip with thumbnails, falling through to halfblocks or a text-only
/// strip as terminal width and protocol support allow.
///
/// The caller (`ui::project_list`) supplies:
/// - the frame and bounding `Rect` (one row tall is enough for the
///   halfblock path, two rows for the image path);
/// - the list of pinned slots in display order;
/// - the [`ThumbnailRenderer`] held by `App`.
///
/// Terminal-width degradation:
/// - `area.width < MIN_STRIP_WIDTH`: returns early without drawing — the
///   caller should have already rendered the name-only fallback.
/// - `area.width >= MIN_STRIP_WIDTH`: draws tiles in sequence.
pub fn render_pinned_strip_with_thumbnails(
    f: &mut Frame<'_>,
    area: Rect,
    slots: &[PinnedSlot<'_>],
    theme: &Theme,
    renderer: &mut ThumbnailRenderer,
) {
    if area.width < MIN_STRIP_WIDTH || area.height == 0 {
        return;
    }

    let palette = IdenticonPalette::from_theme(theme);

    // Layout: each slot is rendered into its own sub-Rect. Tile + label
    // inside each sub-Rect. Between slots we leave a one-column gap so
    // adjacent outlines don't touch.
    const GAP: u16 = 1;
    let slot_cols = TILE_COLS + 2 /* border */ + 1 /* space */ + LABEL_MAX as u16 + 2 /* brackets */;

    let mut cursor_x = area.x;
    for slot in slots {
        if cursor_x + slot_cols > area.x + area.width {
            break;
        }
        let sub = Rect {
            x: cursor_x,
            y: area.y,
            width: slot_cols,
            height: area.height,
        };
        render_one_slot(f, sub, slot, theme, &palette, renderer);
        cursor_x += slot_cols + GAP;
    }
}

/// Render a single `[N: tile name]` tile. Active slots get a mauve
/// rounded outline; inactive slots get a subtle surface-1 outline so
/// tiles still read as discrete units.
fn render_one_slot(
    f: &mut Frame<'_>,
    area: Rect,
    slot: &PinnedSlot<'_>,
    theme: &Theme,
    palette: &IdenticonPalette,
    renderer: &mut ThumbnailRenderer,
) {
    let border_style = if slot.is_active {
        Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface1)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split inner horizontally: [tile (4 cols)] [space] [label].
    if inner.width < TILE_COLS + 2 {
        // Not enough room even inside the border — degrade to label-only.
        let label = truncate_to_width(slot.basename, inner.width as usize);
        let line = Line::from(vec![
            Span::styled(
                format!("{}: ", slot.slot),
                Style::default().fg(theme.subtext1),
            ),
            Span::styled(label, Style::default().fg(theme.text)),
        ]);
        f.render_widget(Paragraph::new(line), inner);
        return;
    }

    let tile_area = Rect {
        x: inner.x,
        y: inner.y,
        width: TILE_COLS,
        height: inner.height.min(TILE_ROWS),
    };
    let label_area = Rect {
        x: inner.x + TILE_COLS + 1,
        y: inner.y,
        width: inner.width.saturating_sub(TILE_COLS + 1),
        height: inner.height,
    };

    // Tile. Always halfblocks — every terminal renders the same tile.
    // The RGB-image cache on `renderer` is still available for
    // non-render consumers (tests, future exporters); the render path
    // reads from the grid directly so we don't pay the 64×64 fill on
    // frames we wouldn't use it.
    let _ = renderer;
    let grid = identicon_grid(slot.basename);
    let [l1, l2] = halfblock_lines(&grid, palette);
    // Two lines fit in the default 2-row tile height; in the 1-row
    // degraded case we just draw the first.
    let para = if tile_area.height >= 2 {
        Paragraph::new(vec![l1, l2])
    } else {
        Paragraph::new(vec![l1])
    };
    f.render_widget(para, tile_area);

    // Label.
    let label = truncate_to_width(slot.basename, LABEL_MAX);
    let label_style = if slot.is_active {
        Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text)
    };
    let line = Line::from(vec![
        Span::styled(
            format!("{}: ", slot.slot),
            Style::default().fg(theme.subtext1),
        ),
        Span::styled(label, label_style),
    ]);
    f.render_widget(Paragraph::new(line), label_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_is_deterministic() {
        let a = identicon_grid("architex");
        let b = identicon_grid("architex");
        assert_eq!(a, b, "same basename must hash to the same grid");
    }

    #[test]
    fn grid_is_four_way_symmetric() {
        let g = identicon_grid("claude-picker");
        for y in 0..GRID as usize {
            for x in 0..GRID as usize {
                let v = g.cells[y * GRID as usize + x];
                let mx = g.cells[y * GRID as usize + (3 - x)];
                let my = g.cells[(3 - y) * GRID as usize + x];
                let mxy = g.cells[(3 - y) * GRID as usize + (3 - x)];
                assert_eq!(v, mx, "h-symmetry fails at ({x},{y})");
                assert_eq!(v, my, "v-symmetry fails at ({x},{y})");
                assert_eq!(v, mxy, "diag-symmetry fails at ({x},{y})");
            }
        }
    }

    #[test]
    fn different_basenames_usually_differ() {
        let a = identicon_grid("architex");
        let b = identicon_grid("infra");
        let c = identicon_grid("claude-picker");
        // At least two of the three must differ — a global collision is
        // astronomically unlikely with a 64-bit hash, but we keep the
        // assertion soft so a future hash swap doesn't wake us at 3am.
        let same_ab = a.cells == b.cells;
        let same_ac = a.cells == c.cells;
        let same_bc = b.cells == c.cells;
        assert!(!(same_ab && same_ac && same_bc));
    }

    #[test]
    fn basename_extraction_is_trailing_slash_agnostic() {
        assert_eq!(basename_for_cwd("/Users/alice/work/architex"), "architex");
        assert_eq!(basename_for_cwd("/Users/alice/work/architex/"), "architex");
        assert_eq!(basename_for_cwd("architex"), "architex");
    }

    #[test]
    fn image_matches_grid_on_cells() {
        let g = identicon_grid("architex");
        let pal = IdenticonPalette {
            bg: Rgb([0, 0, 0]),
            primary: Rgb([200, 100, 200]),
            secondary: [Rgb([255, 150, 100]), Rgb([100, 200, 100]), Rgb([250, 220, 150])],
        };
        let img = identicon_image(&g, &pal).to_rgb8();
        // Sample the centre of each cell; must be bg iff the grid cell is off.
        for cy in 0..GRID {
            for cx in 0..GRID {
                let px = img.get_pixel(cx * CELL_PX + CELL_PX / 2, cy * CELL_PX + CELL_PX / 2);
                let on = g.cells[(cy * GRID + cx) as usize];
                if on {
                    assert_ne!(px, &pal.bg, "cell ({cx},{cy}) should be coloured");
                } else {
                    assert_eq!(px, &pal.bg, "cell ({cx},{cy}) should be bg");
                }
            }
        }
    }
}
