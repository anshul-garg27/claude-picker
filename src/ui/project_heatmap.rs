//! Project-cost 30-day heatmap panel (feature #8, FEAT-6).
//!
//! Renders a horizontal strip of 30 cells per project, where each cell
//! is one day in the last 30 days and the color intensity ramps from
//! `surface0` (no activity) → `mauve` → `peach` → `red` (heaviest day
//! *for that project*). Each row normalises against its own per-project
//! peak so a low-volume project is still legible alongside a whale.
//!
//! ```text
//!   ■ apex-auth            ████▒▒▓▒ ▒▒▒▒▓▓▓▒▒▓   $128.44
//!   ■ architex-cpm         ▒▒    ▒▓▓▒▒   ▒▒▒ ▓   $ 64.10
//!   ■ …                    …                      $  …
//! ```
//!
//! The module is pure rendering — the per-project × per-day grid is
//! built by `commands::stats_cmd::build_stats_data` and handed in via
//! `StatsData::project_day_cost`.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::data::pricing::Family;
use crate::theme::Theme;
use crate::ui::stats::{format_cost, ProjectStats};
use crate::ui::text::{display_width, pad_to_width, truncate_to_width};

/// Width of the day strip in cells. Exposed so the layout math upstream
/// can size the panel without guessing.
pub const STRIP_CELLS: usize = 30;

/// Maximum number of project rows the heatmap will render.
pub const MAX_ROWS: usize = 8;

/// One cell in the 30-day strip — just the index into the shade ramp.
///
/// Index 0 = "no activity" (surface0); higher indices walk the
/// mauve → peach → red ramp. Keeping the mapping in a tiny enum-ish
/// usize means the color-picker can stay a pure lookup that tests can
/// exercise without a live terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intensity(pub u8);

impl Intensity {
    pub const EMPTY: Intensity = Intensity(0);
    pub const PEAK: Intensity = Intensity(4);
}

/// Bucket a day's cost into a 0..=4 intensity index relative to the
/// project's own peak. The floor-lift ensures any non-zero cost reads
/// as at least index 1 — otherwise a $0.01 day would render identically
/// to a zero day and hide actual activity.
pub fn intensity_for(cost: f64, peak: f64) -> Intensity {
    if !(cost > 0.0) || !(peak > 0.0) {
        return Intensity::EMPTY;
    }
    let norm = (cost / peak).clamp(0.0, 1.0);
    let idx = if norm >= 0.99 {
        4
    } else if norm >= 0.60 {
        3
    } else if norm >= 0.30 {
        2
    } else {
        1
    };
    Intensity(idx)
}

/// Map an intensity index to the foreground color for the cell glyph.
/// Ramp: `surface0` (empty) → `overlay1` → `mauve` → `peach` → `red`.
pub fn intensity_color(intensity: Intensity, theme: &Theme) -> Color {
    match intensity.0 {
        0 => theme.surface0,
        1 => theme.overlay1,
        2 => theme.mauve,
        3 => theme.peach,
        _ => theme.red,
    }
}

/// Glyph for a cell. Empty days get a mid-weight shade so they read as
/// "nothing happened" rather than accidentally vanishing; non-empty
/// days walk a standard block ramp so the eye can weight the row at a
/// glance.
pub fn intensity_glyph(intensity: Intensity) -> char {
    match intensity.0 {
        0 => '░',
        1 => '▒',
        2 => '▓',
        3 => '▓',
        _ => '█',
    }
}

/// Build the cells for a single row. `days` is the per-day cost array
/// ordered oldest → newest (index 0 = 29 days ago, index 29 = today).
///
/// Extracted so tests can assert the bucketing without constructing a
/// `Frame`.
pub fn cells_for(days: &[f64; STRIP_CELLS]) -> [Intensity; STRIP_CELLS] {
    let peak = days.iter().copied().fold(0.0f64, f64::max);
    let mut out = [Intensity::EMPTY; STRIP_CELLS];
    for (i, &cost) in days.iter().enumerate() {
        out[i] = intensity_for(cost, peak);
    }
    out
}

/// Family-tinted "■" left-marker. Mirrors the by-model pill palette so
/// a project's dominant-model color is consistent across the dashboard.
fn marker_color(family: Family, theme: &Theme) -> Color {
    match family {
        Family::Opus => theme.peach,
        Family::Sonnet => theme.teal,
        Family::Haiku => theme.blue,
        Family::Unknown => theme.subtext0,
    }
}

/// Rendered height (in text rows) for a panel with `project_rows`
/// project entries. Accounts for 1 rule + 1 blank + N project rows + 1
/// legend + 1 trailing blank. Returns 0 when there's nothing to draw.
pub fn panel_height(project_rows: usize) -> u16 {
    if project_rows == 0 {
        return 0;
    }
    let rows = project_rows.min(MAX_ROWS);
    // rule + blank + N rows + blank + legend
    (rows as u16).saturating_add(4)
}

/// Pair a project's metadata with its 30-day cost strip.
///
/// `by_project` is sorted by lifetime cost desc upstream; the heatmap
/// takes the top N and looks each one up in `project_day_cost`.
pub struct PanelInput<'a> {
    pub by_project: &'a [ProjectStats],
    pub project_day_cost: &'a [(String, [f64; STRIP_CELLS])],
}

/// Render the panel. Returns early when there is nothing to draw so
/// callers can safely reserve zero-height slots.
pub fn render(frame: &mut Frame<'_>, area: Rect, input: &PanelInput<'_>, theme: &Theme) {
    if input.by_project.is_empty() || input.project_day_cost.is_empty() || area.height == 0 {
        return;
    }

    // Section rule — thick + bold to match the other hero sections.
    let title = "project heat (30d) ";
    let rule_width = area.width.saturating_sub(5 + display_width(title) as u16) as usize;
    let rule = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "━━ ",
            Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            title,
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("━".repeat(rule_width), theme.dim()),
    ]);

    let mut lines: Vec<Line> = Vec::with_capacity(input.by_project.len() + 4);
    lines.push(rule);
    lines.push(Line::raw(""));

    // Column widths: "  " + marker(2) + " " + name(18) + "  " + strip(30) + "  " + cost(~10)
    let name_w: usize = 18;
    let cost_w: usize = 10;

    // Fast lookup: project name → day strip.
    let day_lookup: std::collections::HashMap<&str, &[f64; STRIP_CELLS]> = input
        .project_day_cost
        .iter()
        .map(|(n, d)| (n.as_str(), d))
        .collect();

    for project in input.by_project.iter().take(MAX_ROWS) {
        let marker_fg = marker_color(project.color_family, theme);
        let name_disp = pad_to_width(&truncate_to_width(&project.name, name_w), name_w);

        let strip_spans = match day_lookup.get(project.name.as_str()) {
            Some(days) => build_strip_spans(days, theme),
            None => {
                // No per-day data (e.g. cache fast-path). Render a
                // faded placeholder so the row still reads as a row.
                let empty: String = "░".repeat(STRIP_CELLS);
                vec![Span::styled(empty, Style::default().fg(theme.surface0))]
            }
        };

        let cost_str = pad_to_width(&format_cost(project.cost_usd), cost_w);

        let mut spans: Vec<Span> = Vec::with_capacity(strip_spans.len() + 6);
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            "■ ",
            Style::default().fg(marker_fg).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            name_disp,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
        spans.extend(strip_spans);
        spans.push(Span::raw("  "));
        spans.push(Span::styled(cost_str, theme.muted()));
        lines.push(Line::from(spans));
    }

    // Legend: show the intensity ramp so readers can decode the row.
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled("less", theme.dim()),
        Span::raw(" "),
        Span::styled("░", Style::default().fg(theme.surface0)),
        Span::raw(" "),
        Span::styled("▒", Style::default().fg(theme.overlay1)),
        Span::raw(" "),
        Span::styled("▓", Style::default().fg(theme.mauve)),
        Span::raw(" "),
        Span::styled(
            "▓",
            Style::default().fg(theme.peach).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            "█",
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled("more", theme.dim()),
        Span::raw("   "),
        Span::styled(
            "30 cells = last 30 days, per-project normalised",
            theme.dim(),
        ),
    ]));

    frame.render_widget(Paragraph::new(lines), area);
}

fn build_strip_spans<'a>(days: &[f64; STRIP_CELLS], theme: &Theme) -> Vec<Span<'a>> {
    let cells = cells_for(days);
    let mut out: Vec<Span<'a>> = Vec::with_capacity(STRIP_CELLS);
    for cell in cells.iter() {
        let color = intensity_color(*cell, theme);
        let glyph = intensity_glyph(*cell);
        let style = if cell.0 >= 3 {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };
        out.push(Span::styled(glyph.to_string(), style));
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intensity_for_returns_empty_on_zero_and_nonfinite() {
        // Zero cost → empty regardless of peak.
        assert_eq!(intensity_for(0.0, 10.0), Intensity::EMPTY);
        // Zero peak → empty regardless of cost (guards div-by-zero).
        assert_eq!(intensity_for(1.0, 0.0), Intensity::EMPTY);
        // Negative costs treated as empty (defensive — shouldn't happen
        // in the wild, but we don't want NaN colors).
        assert_eq!(intensity_for(-1.0, 10.0), Intensity::EMPTY);
    }

    #[test]
    fn intensity_for_walks_the_ramp_relative_to_peak() {
        let peak = 100.0;
        // 0 < cost < 30% → index 1 (low).
        assert_eq!(intensity_for(5.0, peak), Intensity(1));
        assert_eq!(intensity_for(29.9, peak), Intensity(1));
        // 30-60% → index 2.
        assert_eq!(intensity_for(30.0, peak), Intensity(2));
        assert_eq!(intensity_for(59.9, peak), Intensity(2));
        // 60-99% → index 3.
        assert_eq!(intensity_for(60.0, peak), Intensity(3));
        assert_eq!(intensity_for(98.9, peak), Intensity(3));
        // ≥99% → peak.
        assert_eq!(intensity_for(100.0, peak), Intensity::PEAK);
        // Cost above peak (shouldn't happen, but stay clamped).
        assert_eq!(intensity_for(500.0, peak), Intensity::PEAK);
    }

    #[test]
    fn cells_for_handles_empty_row() {
        // 30 zeros → all cells empty, no panic on zero-peak division.
        let days = [0.0f64; STRIP_CELLS];
        let cells = cells_for(&days);
        assert_eq!(cells.len(), STRIP_CELLS);
        assert!(
            cells.iter().all(|c| *c == Intensity::EMPTY),
            "an empty row must render as entirely empty"
        );
    }

    #[test]
    fn cells_for_single_spike_row_marks_only_peak_day() {
        // One project with a single heavy day — that day hits PEAK and
        // the rest stay empty. This is the "one project" test case.
        let mut days = [0.0f64; STRIP_CELLS];
        days[15] = 42.0;
        let cells = cells_for(&days);
        assert_eq!(cells[15], Intensity::PEAK);
        for (i, c) in cells.iter().enumerate() {
            if i != 15 {
                assert_eq!(*c, Intensity::EMPTY, "day {i} should be empty");
            }
        }
    }

    #[test]
    fn cells_for_multi_intensity_row_walks_each_bucket() {
        // Multi-project / varying-intensity case. Peak = 100 so we can
        // land a cell in every bucket.
        let mut days = [0.0f64; STRIP_CELLS];
        days[0] = 0.0; // empty
        days[1] = 10.0; // 10% → 1
        days[2] = 45.0; // 45% → 2
        days[3] = 75.0; // 75% → 3
        days[4] = 100.0; // 100% → 4
        let cells = cells_for(&days);
        assert_eq!(cells[0], Intensity::EMPTY);
        assert_eq!(cells[1], Intensity(1));
        assert_eq!(cells[2], Intensity(2));
        assert_eq!(cells[3], Intensity(3));
        assert_eq!(cells[4], Intensity::PEAK);
        // Untouched days stay empty.
        assert_eq!(cells[5], Intensity::EMPTY);
    }

    #[test]
    fn intensity_color_ramp_is_distinct_across_buckets() {
        // Regression guard: each bucket must render a distinct color so
        // the strip is readable. If a future palette change collapses
        // two slots this test catches it before shipping.
        let theme = Theme::mocha();
        let colors: Vec<Color> = (0..=4)
            .map(|i| intensity_color(Intensity(i), &theme))
            .collect();
        for (i, a) in colors.iter().enumerate() {
            for (j, b) in colors.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        a, b,
                        "intensities {i} and {j} must have distinct colors"
                    );
                }
            }
        }
    }

    #[test]
    fn panel_height_collapses_for_empty_input_and_caps_at_max_rows() {
        assert_eq!(panel_height(0), 0);
        assert_eq!(panel_height(1), 5);
        assert_eq!(panel_height(MAX_ROWS), (MAX_ROWS as u16) + 4);
        // Over-cap input still returns the capped height.
        assert_eq!(panel_height(MAX_ROWS + 10), (MAX_ROWS as u16) + 4);
    }
}
