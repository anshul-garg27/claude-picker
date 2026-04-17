//! Activity-heatmap widgets — hourly and monthly variants.
//!
//! Shared plumbing for the two heatmap modes the `--stats` dashboard cycles
//! through via the `t` key:
//!
//! - [`render_hourly`] — 24-cell ribbon, one cell per hour of day, summed
//!   across the last N days. Shows "when do you actually Claude?"
//! - [`render_monthly`] — GitHub-contribution-graph calendar: rows are
//!   ISO weeks, columns are Sun..Sat, cells are session count on that day.
//!
//! Both variants share:
//!
//! - The same [`ramp_char`] mapping session count → unicode block glyph.
//! - The same [`ramp_color`] function for count → muted/surface → mauve
//!   saturation, so the two modes feel visually unified.
//!
//! Color ramp — we do NOT use ratatui's `Sparkline` because we want explicit
//! per-cell foreground control. Instead, we compute a simple linear blend
//! from `theme.surface1` (dim, low activity) through `theme.overlay1`
//! (mid) to `theme.mauve` (peak). The block glyph itself ramps from `▁` to
//! `█` and the combination reads more clearly than either signal alone —
//! the shape of the glyph + the saturation of the color reinforce each
//! other.

use chrono::{Datelike, Duration as ChronoDuration, NaiveDate, Weekday};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::text::display_width;

/// Unicode block ramp used by both heatmap variants. Index 0 = empty cell
/// (rendered as `·` by [`ramp_char`] to avoid drawing a visible bar when
/// there's no activity); 1..=7 step up in height.
const RAMP: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Map a raw count to the block-ramp glyph. Returns a dot for the zero case
/// so empty cells are visibly different from "tiny but non-zero" cells.
///
/// Linear-interp against `max` with a floor lift: we don't want the smallest
/// non-zero count to render as the indistinguishable `▁` — it gets promoted
/// to at least index 2 (`▃`) so a lone entry is actually readable.
pub fn ramp_char(count: u32, max: u32) -> char {
    if count == 0 {
        return '·';
    }
    if max <= 1 {
        return '▄';
    }
    let norm = count as f64 / max as f64;
    let idx = ((norm * (RAMP.len() - 1) as f64).round() as usize).clamp(2, RAMP.len() - 1);
    RAMP[idx]
}

/// Map a raw count to the cell foreground color.
///
/// Ramp: surface1 (low) → overlay1 (mid) → mauve (peak). The thresholds
/// are normalized against `max`; when every bucket is empty, everything
/// renders as `theme.surface1` which matches the dot-only state.
pub fn ramp_color(count: u32, max: u32, theme: &Theme) -> Color {
    if count == 0 || max == 0 {
        return theme.surface1;
    }
    let norm = count as f64 / max as f64;
    if norm < 0.25 {
        theme.overlay0
    } else if norm < 0.50 {
        theme.overlay1
    } else if norm < 0.75 {
        theme.overlay2
    } else {
        theme.mauve
    }
}

// ── Hourly heatmap ───────────────────────────────────────────────────────

/// Render a 24-cell "by hour" heatmap.
///
/// `buckets[i]` is the session count for hour `i` (0..=23). We draw two
/// rows of 12 cells each so the whole band fits under ~45 columns — the
/// alternative of a single 24-column row is narrower per cell but more
/// fragile at smaller terminals.
///
/// Returns the number of lines consumed so the caller can offset the next
/// section. The caller is responsible for placing the section heading
/// before us.
pub fn render_hourly(frame: &mut Frame<'_>, area: Rect, buckets: &[u32; 24], theme: &Theme) -> u16 {
    let max = buckets.iter().copied().max().unwrap_or(1).max(1);

    // Build two 12-cell rows so we stay narrow. Each cell is "HH <glyph> ",
    // so one row of 12 cells is ~60 cols wide.
    let row1 = build_hourly_row(&buckets[..12], 0, max, theme);
    let row2 = build_hourly_row(&buckets[12..], 12, max, theme);

    frame.render_widget(
        Paragraph::new(vec![row1, row2]),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 2,
        },
    );
    2
}

fn build_hourly_row<'a>(slice: &[u32], hour_offset: usize, max: u32, theme: &Theme) -> Line<'a> {
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(slice.len() * 4 + 2);
    spans.push(Span::raw("  "));
    for (i, count) in slice.iter().enumerate() {
        let hour = hour_offset + i;
        let ch = ramp_char(*count, max);
        let cell_color = ramp_color(*count, max, theme);
        spans.push(Span::styled(
            format!("{hour:02} "),
            Style::default().fg(theme.overlay0),
        ));
        spans.push(Span::styled(
            ch.to_string(),
            Style::default().fg(cell_color).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
    }
    Line::from(spans)
}

/// Most-active and quietest hour summary — used as the trailing annotation
/// under the hourly heatmap. Returns a `(most, quiet)` pair of `Option<u8>`.
///
/// When every bucket is zero, both returns are `None`. When ties exist for
/// the peak we deterministically pick the *earliest* hour (stable ordering
/// matters when the UI refreshes on each frame).
pub fn hourly_extrema(buckets: &[u32; 24]) -> (Option<u8>, Option<u8>) {
    let mut most: Option<(u8, u32)> = None;
    let mut quiet: Option<(u8, u32)> = None;
    for (i, &c) in buckets.iter().enumerate() {
        let h = i as u8;
        if c > 0 {
            match most {
                Some((_, cur)) if cur >= c => {}
                _ => most = Some((h, c)),
            }
        }
        // Quiet hour — smallest count across any bucket, including 0.
        match quiet {
            Some((_, cur)) if cur <= c => {}
            _ => quiet = Some((h, c)),
        }
    }
    (most.map(|(h, _)| h), quiet.map(|(h, _)| h))
}

// ── Monthly heatmap ──────────────────────────────────────────────────────

/// Render a GitHub-style monthly calendar.
///
/// The layout is always rows-of-7, Sunday through Saturday, starting on the
/// first Sunday at-or-before the 1st of the month so week rows align with
/// real calendar weeks. Days outside the month render as empty spaces;
/// empty cells inside the month render as `·`.
///
/// Returns the number of lines consumed (varies 2..=7 depending on how many
/// weeks the month spans plus the header).
pub fn render_monthly(
    frame: &mut Frame<'_>,
    area: Rect,
    month: &MonthlyActivity,
    theme: &Theme,
) -> u16 {
    // Header row: " S  M  T  W  T  F  S"
    let header = Line::from(vec![
        Span::raw("  "),
        Span::styled("S  M  T  W  T  F  S", theme.dim()),
    ]);

    let weeks = month.weeks_of_cells();
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(weeks.len() + 1);
    lines.push(header);

    let max = month.day_counts.iter().copied().max().unwrap_or(1).max(1);

    for week in &weeks {
        let mut spans: Vec<Span<'_>> = Vec::with_capacity(7 * 2 + 2);
        spans.push(Span::raw("  "));
        for cell in week {
            match cell {
                Cell::Outside => spans.push(Span::raw("   ")),
                Cell::InMonth { count, .. } => {
                    let ch = ramp_char(*count, max);
                    let color = ramp_color(*count, max, theme);
                    spans.push(Span::styled(
                        format!("{ch}  "),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ));
                }
                Cell::Future => {
                    // Still within the month but after `today` — render as
                    // dim dot so the reader can see the month's shape.
                    spans.push(Span::styled("·  ", theme.dim()));
                }
            }
        }
        lines.push(Line::from(spans));
    }

    let h = lines.len().min(area.height as usize) as u16;
    let rect = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: h,
    };
    frame.render_widget(Paragraph::new(lines), rect);
    h
}

/// One calendar cell in the monthly heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Outside the current month (leading/trailing padding to align weeks).
    Outside,
    /// Inside the month, at-or-before `today`. Carries the session count
    /// for the heatmap color.
    InMonth { day: u8, count: u32 },
    /// Inside the month but in the future — render as a dot ring instead of
    /// a block so the month's remaining days are visibly "not yet".
    Future,
}

/// Materialised per-day activity for a single calendar month.
///
/// `day_counts[d - 1]` is the session count on day `d` of the month. Length
/// equals the number of days in the month (28..=31). Days after `today`
/// are kept at whatever value the caller passes in — the renderer won't
/// show them as blocks.
#[derive(Debug, Clone)]
pub struct MonthlyActivity {
    pub year: i32,
    pub month: u32,
    pub today: NaiveDate,
    pub day_counts: Vec<u32>,
}

impl MonthlyActivity {
    /// Build the 2D grid of Sun..Sat weeks the renderer draws. Leading
    /// blanks fill the first row up to day 1; trailing blanks pad the
    /// final row to 7 columns.
    pub fn weeks_of_cells(&self) -> Vec<[Cell; 7]> {
        let first = NaiveDate::from_ymd_opt(self.year, self.month, 1).expect("valid month");
        // Columns are Sun..Sat; Sunday = 0.
        let first_col = sunday_column(first.weekday());
        let days_in_month = self.day_counts.len() as u32;

        let mut weeks = Vec::new();
        let mut row = [Cell::Outside; 7];
        for day_num in 1..=days_in_month {
            let col = (first_col + (day_num - 1)) % 7;
            if day_num > 1 && col == 0 {
                weeks.push(row);
                row = [Cell::Outside; 7];
            }
            let date = NaiveDate::from_ymd_opt(self.year, self.month, day_num).unwrap();
            let count = self.day_counts[(day_num - 1) as usize];
            row[col as usize] = if date > self.today {
                Cell::Future
            } else {
                Cell::InMonth {
                    day: day_num as u8,
                    count,
                }
            };
        }
        weeks.push(row);
        weeks
    }

    /// Find the weekday with the highest average token count across the
    /// month. Callers pass in a parallel "tokens per day" array. Returns
    /// `None` when every day is zero. Rendered under the calendar as the
    /// "most active: Tuesday (3.2k tokens avg)" line.
    pub fn most_active_weekday(&self, tokens_per_day: &[u64]) -> Option<Weekday> {
        assert_eq!(
            tokens_per_day.len(),
            self.day_counts.len(),
            "tokens_per_day must be day-aligned",
        );
        let mut sums = [0u64; 7];
        let mut counts = [0u32; 7];
        for (idx, &tok) in tokens_per_day.iter().enumerate() {
            let day_num = (idx + 1) as u32;
            let date = NaiveDate::from_ymd_opt(self.year, self.month, day_num)?;
            // Only count days we've actually lived through.
            if date > self.today {
                continue;
            }
            let col = sunday_column(date.weekday()) as usize;
            sums[col] = sums[col].saturating_add(tok);
            counts[col] = counts[col].saturating_add(1);
        }
        let mut best: Option<(usize, f64)> = None;
        for i in 0..7 {
            if counts[i] == 0 {
                continue;
            }
            let avg = sums[i] as f64 / counts[i] as f64;
            match best {
                Some((_, cur)) if cur >= avg => {}
                _ => best = Some((i, avg)),
            }
        }
        best.map(|(col, _)| weekday_from_sunday_column(col))
    }

    /// Find the weekday with the *lowest* average session count. Rendered
    /// under the calendar as "quiet: Saturday".
    pub fn quietest_weekday(&self) -> Option<Weekday> {
        let mut sums = [0u32; 7];
        let mut counts = [0u32; 7];
        for (idx, &c) in self.day_counts.iter().enumerate() {
            let day_num = (idx + 1) as u32;
            let date = NaiveDate::from_ymd_opt(self.year, self.month, day_num)?;
            if date > self.today {
                continue;
            }
            let col = sunday_column(date.weekday()) as usize;
            sums[col] = sums[col].saturating_add(c);
            counts[col] = counts[col].saturating_add(1);
        }
        let mut best: Option<(usize, f64)> = None;
        for i in 0..7 {
            if counts[i] == 0 {
                continue;
            }
            let avg = sums[i] as f64 / counts[i] as f64;
            match best {
                Some((_, cur)) if cur <= avg => {}
                _ => best = Some((i, avg)),
            }
        }
        best.map(|(col, _)| weekday_from_sunday_column(col))
    }

    /// Count the days in month `year`/`month`. Handles leap Februarys via
    /// chrono's calendar math rather than a lookup table.
    pub fn days_in_month(year: i32, month: u32) -> u32 {
        let (y, m) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        let next_first = NaiveDate::from_ymd_opt(y, m, 1).expect("valid month");
        next_first.pred_opt().unwrap().day()
    }
}

/// Map chrono `Weekday` to a Sunday-first column index (0..=6).
#[inline]
fn sunday_column(w: Weekday) -> u32 {
    match w {
        Weekday::Sun => 0,
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
    }
}

#[inline]
fn weekday_from_sunday_column(col: usize) -> Weekday {
    match col {
        0 => Weekday::Sun,
        1 => Weekday::Mon,
        2 => Weekday::Tue,
        3 => Weekday::Wed,
        4 => Weekday::Thu,
        5 => Weekday::Fri,
        _ => Weekday::Sat,
    }
}

/// Human label for a weekday — used in the monthly annotation.
pub fn weekday_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Sun => "Sunday",
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
    }
}

// ── Shared helpers ───────────────────────────────────────────────────────

/// Left-align text after a 2-col leading padding; used by both heatmaps.
#[allow(dead_code)]
pub(crate) fn padded_line<'a>(s: &'a str, theme: &Theme) -> Line<'a> {
    let w = display_width(s);
    let _ = w; // kept for future truncation logic
    Line::from(vec![Span::raw("  "), Span::styled(s, theme.muted())])
}

/// Collapse a sparse (date → count) aggregation into a 24-cell hourly array
/// for the last `days` days. `f` is called per-session to pick out both
/// the hour (0..=23) and whether to include the session; returns the final
/// bucket array.
///
/// Kept generic so both sessions and turn-duration events can flow through
/// the same aggregation pipe.
pub fn aggregate_hourly<I, F>(iter: I, extract: F) -> [u32; 24]
where
    I: IntoIterator,
    F: Fn(&I::Item) -> Option<u32>,
{
    let mut out = [0u32; 24];
    for item in iter {
        if let Some(h) = extract(&item) {
            if h < 24 {
                out[h as usize] = out[h as usize].saturating_add(1);
            }
        }
    }
    out
}

/// Shift `today` back by `days - 1` and collect any date within that window
/// into a histogram keyed on `NaiveDate`. Callers use this to bucket the
/// 7-day window the hourly heatmap displays.
pub fn window_start(today: NaiveDate, days: u32) -> NaiveDate {
    today - ChronoDuration::days(days.saturating_sub(1) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_char_zero_is_dot() {
        assert_eq!(ramp_char(0, 10), '·');
    }

    #[test]
    fn ramp_char_floor_is_lifted_above_the_smallest_glyph() {
        // 1 of 10 maps to index 0.9 round → 1, then clamped up to at least 2.
        let ch = ramp_char(1, 10);
        assert_ne!(ch, '▁', "floor should be lifted to at least ▃");
        assert_ne!(ch, '·', "non-zero must not be a dot");
    }

    #[test]
    fn ramp_char_max_is_full_block() {
        assert_eq!(ramp_char(10, 10), '█');
    }

    #[test]
    fn ramp_char_handles_max_of_one() {
        // Any non-zero count when max==1 should render as the mid glyph.
        assert_eq!(ramp_char(1, 1), '▄');
    }

    #[test]
    fn hourly_extrema_finds_peak_and_quiet() {
        let mut buckets = [0u32; 24];
        buckets[9] = 3;
        buckets[10] = 12;
        buckets[14] = 7;
        let (most, quiet) = hourly_extrema(&buckets);
        assert_eq!(most, Some(10));
        // Any 0 bucket counts as "quiet"; 0 is the first, so it's hour 0.
        assert_eq!(quiet, Some(0));
    }

    #[test]
    fn hourly_extrema_empty_returns_none_for_peak() {
        let buckets = [0u32; 24];
        let (most, quiet) = hourly_extrema(&buckets);
        assert_eq!(most, None);
        assert_eq!(quiet, Some(0));
    }

    #[test]
    fn monthly_weeks_wrap_at_sunday() {
        // April 2026: April 1 is a Wednesday. So the first week is:
        // [Sun=outside, Mon=outside, Tue=outside, Wed=1, Thu=2, Fri=3, Sat=4]
        let today = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();
        let m = MonthlyActivity {
            year: 2026,
            month: 4,
            today,
            day_counts: vec![0; 30],
        };
        let weeks = m.weeks_of_cells();
        assert!(weeks.len() >= 5, "april 2026 spans at least 5 weeks");
        // First row: first 3 columns outside-of-month.
        assert_eq!(weeks[0][0], Cell::Outside);
        assert_eq!(weeks[0][1], Cell::Outside);
        assert_eq!(weeks[0][2], Cell::Outside);
        match weeks[0][3] {
            Cell::InMonth { day, .. } => assert_eq!(day, 1),
            other => panic!("wed cell should be day 1, got {other:?}"),
        }
    }

    #[test]
    fn monthly_future_days_render_as_future_cells() {
        // April 2026, today = 16, so days 17..=30 should be Future.
        let today = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();
        let m = MonthlyActivity {
            year: 2026,
            month: 4,
            today,
            day_counts: vec![1; 30],
        };
        let weeks = m.weeks_of_cells();
        let mut seen_future = false;
        let mut seen_in_month = false;
        for week in weeks {
            for cell in week {
                match cell {
                    Cell::Future => seen_future = true,
                    Cell::InMonth { day, .. } if day <= 16 => {
                        seen_in_month = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(seen_future, "april 17..30 should render Future");
        assert!(seen_in_month, "april 1..16 should render InMonth");
    }

    #[test]
    fn most_active_weekday_picks_highest_average() {
        // April 2026: day 1 = Wed, day 7 = Tue, day 14 = Tue, day 21 = Tue.
        // If Tuesday tokens are very high and other days zero, the
        // result must be Tuesday.
        let mut tokens = vec![0u64; 30];
        tokens[6] = 1000; // day 7 = Tue
        tokens[13] = 2000; // day 14 = Tue
        tokens[20] = 3000; // day 21 = Tue
        let today = NaiveDate::from_ymd_opt(2026, 4, 30).unwrap();
        let m = MonthlyActivity {
            year: 2026,
            month: 4,
            today,
            day_counts: vec![0; 30],
        };
        assert_eq!(m.most_active_weekday(&tokens), Some(Weekday::Tue));
    }

    #[test]
    fn days_in_month_handles_leap_year() {
        assert_eq!(MonthlyActivity::days_in_month(2024, 2), 29);
        assert_eq!(MonthlyActivity::days_in_month(2025, 2), 28);
        assert_eq!(MonthlyActivity::days_in_month(2026, 4), 30);
        assert_eq!(MonthlyActivity::days_in_month(2026, 12), 31);
    }

    #[test]
    fn aggregate_hourly_puts_items_in_right_buckets() {
        let items = vec![2u32, 2, 5, 23];
        let buckets = aggregate_hourly(items, |h| Some(*h));
        assert_eq!(buckets[2], 2);
        assert_eq!(buckets[5], 1);
        assert_eq!(buckets[23], 1);
        assert_eq!(buckets[0], 0);
    }

    #[test]
    fn aggregate_hourly_ignores_out_of_range() {
        let items = vec![99u32, 100];
        let buckets = aggregate_hourly(items, |h| Some(*h));
        assert_eq!(buckets, [0u32; 24]);
    }

    #[test]
    fn weekday_name_matches_chrono() {
        assert_eq!(weekday_name(Weekday::Tue), "Tuesday");
        assert_eq!(weekday_name(Weekday::Sat), "Saturday");
    }

    #[test]
    fn window_start_is_today_minus_days_minus_one() {
        let today = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();
        let start = window_start(today, 7);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 4, 10).unwrap());
    }
}
