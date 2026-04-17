//! Which-key popup for leader chords.
//!
//! Helix-style discovery aid: when the user presses a chord-starting key
//! (Space, `g`, …) and does NOT type a follow-up within ~250 ms, we pop a
//! small floating panel listing the next-key bindings. The event loop does
//! NOT block — if a follow-up arrives while the overlay is visible the
//! action fires and the overlay vanishes on the next tick.
//!
//! The renderer is pure state: pass in the leader char + an optional theme
//! and a centered panel pops up. `App` decides when to call it via
//! [`crate::app::App::pending_which_key`].

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

/// How long a chord leader must sit with no follow-up before the overlay
/// appears. 250 ms matches the Helix default — long enough that a fast
/// typist never sees the overlay, short enough that a pausing user gets
/// the hint almost immediately.
pub const WHICH_KEY_DELAY_MS: u64 = 250;

/// One next-key entry in a leader's which-key grid.
#[derive(Debug, Clone, Copy)]
pub struct ChordEntry {
    /// Literal key label ("f", "Space", "?").
    pub key: &'static str,
    /// Short description shown next to the key.
    pub desc: &'static str,
}

/// Return the entries shown for a given leader char. `None` means we don't
/// have a which-key table for this leader — the overlay simply shouldn't
/// render.
pub fn entries_for(leader: char) -> Option<&'static [ChordEntry]> {
    match leader {
        ' ' => Some(SPACE_LEADER),
        'g' => Some(G_LEADER),
        _ => None,
    }
}

/// Pretty label for the leader, used in the overlay title.
pub fn title_for(leader: char) -> &'static str {
    match leader {
        ' ' => "Space \u{00B7} leader",
        'g' => "g \u{00B7} goto",
        _ => "leader",
    }
}

/// Space-leader next keys. Mirrors the palette menu but shown inline so
/// the user doesn't have to open the palette to remember what's there.
const SPACE_LEADER: &[ChordEntry] = &[
    ChordEntry {
        key: "f",
        desc: "find session",
    },
    ChordEntry {
        key: "m",
        desc: "model switcher",
    },
    ChordEntry {
        key: "t",
        desc: "theme cycle",
    },
    ChordEntry {
        key: "r",
        desc: "rename",
    },
    ChordEntry {
        key: "d",
        desc: "diff viewer",
    },
    ChordEntry {
        key: "R",
        desc: "replay player",
    },
    ChordEntry {
        key: "s",
        desc: "stats",
    },
    ChordEntry {
        key: "?",
        desc: "help",
    },
    ChordEntry {
        key: "w",
        desc: "tasks drawer",
    },
    ChordEntry {
        key: "Space",
        desc: "palette",
    },
];

/// `g`-leader next keys. Only `gg` is wired today but the overlay still
/// teaches the chord.
const G_LEADER: &[ChordEntry] = &[ChordEntry {
    key: "g",
    desc: "jump to top",
}];

/// Render the which-key overlay centered inside `area`. Callers decide
/// whether it should be visible — this function just draws the panel.
pub fn render(frame: &mut Frame<'_>, area: Rect, leader: char, theme: &Theme) {
    let Some(entries) = entries_for(leader) else {
        return;
    };

    // Fixed 40x12 panel as the spec requests; clamp to area to avoid
    // overflow on tiny terminals.
    let w = 48u16.min(area.width.saturating_sub(4));
    let h = 12u16.min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title_for(leader),
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    // Two-column grid: the spec's mock lays out entries left-to-right in
    // row-major order so the longest descriptions break the visual rhythm
    // less. Column width computed from the inner width.
    let inner_w = inner.width.max(2) as usize;
    let col_w = inner_w / 2;

    let mut lines: Vec<Line<'static>> = Vec::with_capacity((entries.len() + 1) / 2 + 1);
    lines.push(Line::raw(""));
    let mut i = 0usize;
    while i < entries.len() {
        let left = format_entry(&entries[i], col_w.saturating_sub(1), theme);
        let mut spans = vec![Span::raw(" ")];
        spans.extend(left);
        if i + 1 < entries.len() {
            spans.push(Span::raw(" "));
            let right = format_entry(&entries[i + 1], col_w.saturating_sub(1), theme);
            spans.extend(right);
        }
        lines.push(Line::from(spans));
        i += 2;
    }

    let p = Paragraph::new(lines).alignment(Alignment::Left);
    frame.render_widget(p, inner);
}

/// Render one "key  desc" pair as a sequence of styled spans. Key is
/// rendered in the accent colour + bold; description in muted text.
fn format_entry(entry: &ChordEntry, target_width: usize, theme: &Theme) -> Vec<Span<'static>> {
    // Pad keys to a consistent column so descriptions line up vertically
    // inside each half of the grid.
    let key_col = 6usize;
    let key_len = entry.key.chars().count();
    let key_pad = key_col.saturating_sub(key_len);
    // Truncate desc if it can't fit in the remaining width to avoid wrap.
    let max_desc = target_width.saturating_sub(key_col + 2).max(1);
    let desc = if entry.desc.chars().count() > max_desc {
        let taken: String = entry.desc.chars().take(max_desc.saturating_sub(1)).collect();
        format!("{taken}\u{2026}")
    } else {
        entry.desc.to_string()
    };
    vec![
        Span::styled(
            entry.key.to_string(),
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(key_pad + 1)),
        Span::styled(desc, theme.key_desc()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_leader_has_entries() {
        let e = entries_for(' ').expect("space leader present");
        assert!(!e.is_empty());
    }

    #[test]
    fn g_leader_has_gg() {
        let e = entries_for('g').expect("g leader present");
        assert!(e.iter().any(|c| c.key == "g"));
    }

    #[test]
    fn unknown_leader_has_no_entries() {
        assert!(entries_for('x').is_none());
    }

    #[test]
    fn delay_is_tight_enough_for_fast_typists() {
        // Sanity: we don't want to accidentally push this past a human's
        // minimum comfortable chord interval.
        assert!(WHICH_KEY_DELAY_MS <= 300);
        assert!(WHICH_KEY_DELAY_MS >= 100);
    }
}
