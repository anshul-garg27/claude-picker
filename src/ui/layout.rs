//! Top-level layout helpers.
//!
//! Isolates the chunk math for the outer frame. A single [`LayoutChunks`]
//! struct is returned so the caller can address fields by name
//! (`chunks.list_pane`) rather than indexing into a `Vec<Rect>` and relying on
//! ordering, which is easy to break during a resize.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Minimum terminal size we guarantee a usable layout for. Below this we
/// render a single-line "resize please" message instead.
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 20;

/// Width threshold below which we switch the two-pane split from 55/45
/// to 50/50 (so both panes stay legible on narrow terminals).
const NARROW_THRESHOLD: u16 = 110;

/// Layout of the main two-pane picker screen.
pub struct LayoutChunks {
    pub title_bar: Rect,
    pub list_pane: Rect,
    pub preview_pane: Rect,
    pub footer: Rect,
}

/// Compute the full-screen layout, returning zones for each part.
pub fn main_picker(area: Rect) -> LayoutChunks {
    // 1-line title, the main pane (flex), 1-line footer — plus a blank line
    // of breathing room between body and footer.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Min(5),    // body
            Constraint::Length(1), // footer
        ])
        .split(area);

    // Horizontal split inside the body — 55/45 by default, 50/50 when narrow.
    let (left_pct, right_pct) = if area.width < NARROW_THRESHOLD {
        (50u16, 50u16)
    } else {
        (55u16, 45u16)
    };

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_pct),
            Constraint::Percentage(right_pct),
        ])
        .split(rows[1]);

    LayoutChunks {
        title_bar: rows[0],
        list_pane: body[0],
        preview_pane: body[1],
        footer: rows[2],
    }
}

/// Layout for the project-picker screen — one pane + footer.
pub fn project_picker(area: Rect) -> (Rect, Rect, Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);
    (rows[0], rows[1], rows[2])
}

/// True when the terminal is too small to render a usable picker. The caller
/// should render a "resize please" placeholder instead.
pub fn too_small(area: Rect) -> bool {
    area.width < MIN_WIDTH || area.height < MIN_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_terminal_uses_55_45() {
        // 120 is above the narrow threshold, so left should be ~66.
        let chunks = main_picker(Rect::new(0, 0, 120, 40));
        assert!(chunks.list_pane.width >= 60);
        assert!(chunks.preview_pane.width >= 40);
    }

    #[test]
    fn narrow_terminal_uses_50_50() {
        let chunks = main_picker(Rect::new(0, 0, 90, 40));
        // +/- 1 rounding is fine.
        let diff = (chunks.list_pane.width as i32 - chunks.preview_pane.width as i32).abs();
        assert!(
            diff <= 1,
            "expected ~even split, got {} vs {}",
            chunks.list_pane.width,
            chunks.preview_pane.width
        );
    }

    #[test]
    fn too_small_detection() {
        assert!(too_small(Rect::new(0, 0, 60, 20)));
        assert!(too_small(Rect::new(0, 0, 80, 15)));
        assert!(!too_small(Rect::new(0, 0, 80, 20)));
    }
}
