//! `claude-picker stats` — Ratatui dashboard.
//!
//! Re-implements the Python `lib/session-stats.py` layout in native
//! ratatui widgets:
//!
//! ```text
//!  claude-picker --stats                    last 30 days · all projects
//!  ╭─ tokens ──────╮  ╭─ cost ───────╮  ╭─ sessions ────╮
//!    109.0M ···█·…    $132.38 ····█     21    ·…█
//!    107.7M in · 1.3M out   avg $4.41/d      16 named · 5 unn
//!  ╰───────────────╯  ╰──────────────╯  ╰───────────────╯
//!
//!  ── per project ──────────────────────────────────────
//!  architex       ████████████  $94.40  ·  89.6M tok  · 12 ses
//!  …
//!
//!  ── activity (30d) ───────────────────────────────────
//!    ·  ·  ·  ·  ·  ·  ·  ▃  ·  ·  ·  █
//!    Mar 18      Mar 24       Mar 31       Apr 16
//!                                          ↑ today
//!
//!  by model: opus-4-7 $109.42 · opus-4-6 $22.96
//!
//!  press q quit · e export · t toggle days/weeks
//! ```
//!
//! The module is pure rendering — the [`StatsData`] struct is built elsewhere
//! (see [`crate::commands::stats_cmd::aggregate`]). Everything here is a
//! function of that struct plus a few UI knobs (timeline mode, toast).

use std::time::Duration;

use chrono::{Datelike, Duration as ChronoDuration, NaiveDate};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::data::pricing::{Family, TokenCounts};
use crate::theme::Theme;
use crate::ui::heatmap::{self, MonthlyActivity};
use crate::ui::text::{display_width, pad_to_width, truncate_to_width};

// ── Data types ────────────────────────────────────────────────────────────

/// Aggregated totals across every session seen.
///
/// Totals are computed across *all* sessions — not just the 30-day window —
/// because they're primarily useful as a running lifetime tally. The 30-day
/// window only drives the sparklines and activity timeline.
#[derive(Debug, Clone, Default)]
pub struct Totals {
    pub total_tokens: TokenCounts,
    pub total_cost_usd: f64,
    pub total_sessions: u32,
    /// Mean of `total_cost_usd` spread over the 30-day window. Matches the
    /// Python "avg $X / day" caption on the cost KPI card.
    pub avg_cost_per_day: f64,
}

/// One row in the per-project bar chart.
#[derive(Debug, Clone)]
pub struct ProjectStats {
    pub name: String,
    pub cost_usd: f64,
    pub total_tokens: u64,
    pub session_count: u32,
    /// Dominant model family — picks the row's bar color.
    pub color_family: Family,
}

/// One bucket in the daily activity series.
#[derive(Debug, Clone)]
pub struct DailyStats {
    pub date: NaiveDate,
    pub sessions: u32,
    pub tokens: u64,
    pub cost_usd: f64,
}

/// Buckets used by the turn-duration histogram (feature #14).
pub const DURATION_BUCKETS: &[(&str, Duration)] = &[
    ("0-10s", Duration::from_secs(10)),
    ("10-30s", Duration::from_secs(30)),
    ("30-60s", Duration::from_secs(60)),
    ("1-3min", Duration::from_secs(180)),
    ("3-10min", Duration::from_secs(600)),
    ("10min+", Duration::MAX),
];

/// Turn-duration stats for the dashboard histogram.
#[derive(Debug, Clone, Default)]
pub struct TurnDurationStats {
    /// Per-bucket counts, indexed into [`DURATION_BUCKETS`].
    pub counts: [u64; 6],
    /// Total number of turns counted.
    pub total_turns: u64,
    /// Sum across every turn.
    pub total_wall_time: Duration,
}

impl TurnDurationStats {
    pub fn bucket_index(d: Duration) -> usize {
        for (i, (_, upper)) in DURATION_BUCKETS.iter().enumerate() {
            if d <= *upper {
                return i;
            }
        }
        DURATION_BUCKETS.len() - 1
    }

    pub fn push(&mut self, d: Duration) {
        let i = Self::bucket_index(d);
        self.counts[i] = self.counts[i].saturating_add(1);
        self.total_turns = self.total_turns.saturating_add(1);
        self.total_wall_time = self.total_wall_time.saturating_add(d);
    }

    pub fn max_count(&self) -> u64 {
        self.counts.iter().copied().max().unwrap_or(0)
    }
}

/// Fully aggregated payload for the stats dashboard.
#[derive(Debug, Clone)]
pub struct StatsData {
    pub totals: Totals,
    pub by_project: Vec<ProjectStats>,
    pub daily: Vec<DailyStats>,
    pub by_model: Vec<(String, f64)>,
    pub named_count: u32,
    pub unnamed_count: u32,
    /// Sum of `total_cost_usd` for sessions whose last timestamp falls in
    /// the current calendar month. Drives the budget forecast.
    pub month_to_date_cost: f64,
    /// Build date; used by the budget math.
    pub today: NaiveDate,
    /// Sessions-per-hour-of-day histogram over the last 7 days (local time).
    pub hourly_buckets: [u32; 24],
    /// Activity for the current calendar month.
    pub monthly: MonthlyActivity,
    /// Parallel to `monthly.day_counts` — tokens summed per day.
    pub monthly_tokens: Vec<u64>,
    /// Turn-duration histogram over the last 30 days.
    pub turn_durations: TurnDurationStats,
}

/// Timeline window mode — cycles through 4 modes on `t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineMode {
    Days30,
    Weeks12,
    Hours24,
    Month,
}

impl TimelineMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Days30 => "last 30 days",
            Self::Weeks12 => "last 12 weeks",
            Self::Hours24 => "by hour (7d)",
            Self::Month => "month at a glance",
        }
    }

    pub fn buckets(self) -> usize {
        match self {
            Self::Days30 => 30,
            Self::Weeks12 => 12,
            Self::Hours24 => 24,
            Self::Month => 31,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Days30 => Self::Weeks12,
            Self::Weeks12 => Self::Hours24,
            Self::Hours24 => Self::Month,
            Self::Month => Self::Days30,
        }
    }

    pub fn is_heatmap(self) -> bool {
        matches!(self, Self::Hours24 | Self::Month)
    }
}

// ── Dashboard state used purely by the renderer ──────────────────────────

/// Transient state the render layer owns. Split out from [`StatsData`] so the
/// data layer doesn't care about UI concerns.
#[derive(Debug, Clone)]
pub struct StatsView<'a> {
    pub data: &'a StatsData,
    pub mode: TimelineMode,
    pub toast: Option<&'a str>,
    pub toast_kind: ToastKind,
    /// Forecast band visible? Toggled by `f`.
    pub show_forecast: bool,
    /// User's monthly USD budget cap. 0.0 = unset.
    pub monthly_limit_usd: f64,
    /// Budget-modal in-progress input, `Some` when open.
    pub budget_modal: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Info,
    Error,
}

// ── Public render entry point ────────────────────────────────────────────

/// Maximum width the dashboard will draw into. Matches the Python MAX_W.
const MAX_W: u16 = 120;
/// Minimum width where the layout still renders sensibly. Below this we
/// show a "resize please" placeholder.
const MIN_W: u16 = 80;
const MIN_H: u16 = 24;

/// Render the full dashboard into `area`.
///
/// The function is idempotent — calling it repeatedly with the same
/// arguments produces the same pixels. That matters because the event loop
/// calls it on every frame.
pub fn render(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    if area.width < MIN_W || area.height < MIN_H {
        render_too_small(frame, area, theme);
        return;
    }

    // Cap width + center.
    let inner = center_capped(area, MAX_W);

    // Optional bands, sized based on state. The richer budget band wants
    // room for the rule + MTD bar + forecast bar + model breakdown (up to 3
    // rows). When no budget is configured AND forecast is hidden we collapse
    // to zero so the rest of the dashboard reclaims the space.
    let budget_model_rows = view
        .data
        .by_model
        .iter()
        .filter(|(_, cost)| {
            // Only show per-model rows when there's an actual monthly spend.
            let mtd = view.data.month_to_date_cost;
            mtd > 0.0 && *cost > 0.0
        })
        .count()
        .min(3) as u16;
    let budget_h: u16 = if view.show_forecast || view.monthly_limit_usd > 0.0 {
        // 1 rule + 1 blank + 1 MTD progress + 1 forecast + blank +
        // budget_model_rows + 1 trailing blank.
        5 + budget_model_rows
    } else {
        0
    };
    let hist_h: u16 = if view.data.turn_durations.total_turns > 0 {
        12
    } else {
        0
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),        // title
            Constraint::Length(1),        // blank
            Constraint::Length(8),        // kpi cards (rich)
            Constraint::Length(1),        // blank
            Constraint::Min(8),           // per-project + activity flex
            Constraint::Length(hist_h),   // turn-duration histogram
            Constraint::Length(budget_h), // budget band
            Constraint::Length(1),        // by-model
            Constraint::Length(1),        // footer
        ])
        .split(inner);

    render_title(frame, rows[0], view, theme);
    render_kpi_row(frame, rows[2], view, theme);
    render_body(frame, rows[4], view, theme);
    if hist_h > 0 {
        render_turn_duration_hist(frame, rows[5], view, theme);
    }
    if budget_h > 0 {
        render_budget_band(frame, rows[6], view, theme);
    }
    render_by_model(frame, rows[7], view, theme);
    render_footer(frame, rows[8], theme);

    if let Some(msg) = view.toast {
        render_toast(frame, area, msg, view.toast_kind, theme);
    }
    if let Some(buf) = view.budget_modal {
        render_budget_modal(frame, area, buf, view, theme);
    }
}

/// Take the sub-rect of `area` capped at `max_width`, centered horizontally.
fn center_capped(area: Rect, max_width: u16) -> Rect {
    let w = area.width.min(max_width);
    let x_offset = area.width.saturating_sub(w) / 2;
    Rect {
        x: area.x + x_offset,
        y: area.y,
        width: w,
        height: area.height,
    }
}

// ── Title bar ────────────────────────────────────────────────────────────

fn render_title(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    let left = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "claude-picker --stats",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let right = Line::from(vec![
        Span::styled(view.mode.label(), theme.subtle()),
        Span::styled(" · ", theme.dim()),
        Span::styled("all projects", theme.subtle()),
        Span::raw(" "),
    ])
    .alignment(Alignment::Right);

    // Two-column split: left fixed at title width, right fills the rest.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    frame.render_widget(Paragraph::new(left), cols[0]);
    frame.render_widget(Paragraph::new(right), cols[1]);
}

// ── KPI row ──────────────────────────────────────────────────────────────

/// Compute a percent-delta (-100..) between the **current-half** and
/// **prior-half** of a daily series. Returns `None` if the prior window is
/// empty (no baseline to compare against) — the callsite then renders a
/// neutral dash instead of a colored triangle.
///
/// For a 30-day `daily` vec this gives us "last 15 days vs previous 15
/// days", which is the window the KPI delta chip advertises.
fn half_over_half_delta(series: &[f64]) -> Option<f64> {
    if series.len() < 4 {
        return None;
    }
    let mid = series.len() / 2;
    let prior: f64 = series[..mid].iter().sum();
    let current: f64 = series[mid..].iter().sum();
    if prior <= f64::EPSILON {
        return None;
    }
    Some(((current - prior) / prior) * 100.0)
}

/// Render a small colored delta chip (▲12% / ▼5% / ─). "Up = good" for
/// tokens/sessions but "up = bad" for cost — the callsite owns the polarity
/// flag via `up_is_good`.
fn delta_chip<'a>(delta: Option<f64>, theme: &Theme, up_is_good: bool) -> Vec<Span<'a>> {
    match delta {
        None => vec![
            Span::styled("─ ", theme.dim()),
            Span::styled("no baseline", theme.dim()),
        ],
        Some(d) => {
            let (arrow, color) = if d.abs() < 0.5 {
                ("─", theme.subtext0)
            } else if d > 0.0 {
                ("▲", if up_is_good { theme.green } else { theme.red })
            } else {
                ("▼", if up_is_good { theme.red } else { theme.green })
            };
            let pct = format!("{:>3.0}%", d.abs());
            vec![
                Span::styled(
                    format!("{arrow} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    pct,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" vs prior", theme.dim()),
            ]
        }
    }
}

/// 6-step braille sparkline glyph picker — maps a normalised value [0, 1]
/// to one of `▁▂▃▄▅▆▇█`. Used inside the KPI cards where Ratatui's built-in
/// `Sparkline` is a little too heavy (it steals a whole row and doesn't
/// compose well inside a one-line value row).
fn mini_sparkline(series: &[u64]) -> String {
    if series.is_empty() {
        return String::new();
    }
    const RAMP: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = series.iter().copied().max().unwrap_or(1).max(1);
    series
        .iter()
        .map(|&v| {
            if v == 0 {
                ' '
            } else {
                let norm = v as f64 / max as f64;
                let idx = ((norm * (RAMP.len() - 1) as f64).round() as usize)
                    .min(RAMP.len() - 1);
                RAMP[idx]
            }
        })
        .collect()
}

fn render_kpi_row(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    // 3 cards, side by side. Ratio-thirds so every card is the same width
    // even when the dashboard is narrower than MAX_W.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    let t = &view.data.totals;
    let tokens_spark: Vec<u64> = view.data.daily.iter().map(|d| d.tokens).collect();
    let cost_spark: Vec<u64> = view
        .data
        .daily
        .iter()
        // Sparkline takes u64, cost is a float — round to cents so the shape
        // of the trend survives the cast.
        .map(|d| (d.cost_usd * 100.0).round() as u64)
        .collect();
    let sessions_spark: Vec<u64> =
        view.data.daily.iter().map(|d| d.sessions as u64).collect();

    // Half-over-half deltas (last 15d vs prior 15d).
    let tokens_series: Vec<f64> = view.data.daily.iter().map(|d| d.tokens as f64).collect();
    let cost_series: Vec<f64> = view.data.daily.iter().map(|d| d.cost_usd).collect();
    let sessions_series: Vec<f64> =
        view.data.daily.iter().map(|d| d.sessions as f64).collect();

    let tokens_delta = half_over_half_delta(&tokens_series);
    let cost_delta = half_over_half_delta(&cost_series);
    let sessions_delta = half_over_half_delta(&sessions_series);

    // Narrow mode: below ~96 cols the sparklines inside the cards eat the
    // value text. Switch to a compact card that drops the sparkline row.
    let compact = area.width < 96;

    // Card 1 — tokens. Teal accent, big number + chip + sparkline.
    render_kpi_card(
        frame,
        cols[0],
        theme,
        KpiCard {
            title: "tokens",
            icon: "◈",
            big_value: &format_tokens(t.total_tokens.total()),
            big_color: theme.peach,
            spark_data: &tokens_spark,
            spark_color: theme.teal,
            delta: tokens_delta,
            up_is_good: true,
            subtitle: &format!(
                "{} in · {} out",
                format_tokens(
                    t.total_tokens
                        .input
                        .saturating_add(t.total_tokens.cache_read)
                        .saturating_add(t.total_tokens.cache_write_5m)
                        .saturating_add(t.total_tokens.cache_write_1h)
                ),
                format_tokens(t.total_tokens.output),
            ),
            is_hero: false,
            compact,
        },
    );

    // Card 2 — cost (the hero). Mauve thick border, big green number.
    render_kpi_card(
        frame,
        cols[1],
        theme,
        KpiCard {
            title: "cost",
            icon: "$",
            big_value: &format_cost(t.total_cost_usd),
            big_color: theme.green,
            spark_data: &cost_spark,
            spark_color: theme.peach,
            delta: cost_delta,
            up_is_good: false, // rising cost = bad
            subtitle: &format!("avg {} / day", format_cost(t.avg_cost_per_day)),
            is_hero: true,
            compact,
        },
    );

    // Card 3 — sessions. Yellow accent.
    render_kpi_card(
        frame,
        cols[2],
        theme,
        KpiCard {
            title: "sessions",
            icon: "◉",
            big_value: &t.total_sessions.to_string(),
            big_color: theme.yellow,
            spark_data: &sessions_spark,
            spark_color: theme.yellow,
            delta: sessions_delta,
            up_is_good: true,
            subtitle: &format!(
                "{} named · {} unnamed",
                view.data.named_count, view.data.unnamed_count
            ),
            is_hero: false,
            compact,
        },
    );
}

/// Props bag for [`render_kpi_card`]. Reduces the argument list from nine
/// positional parameters to a single struct — also lets clippy stop
/// complaining about `too_many_arguments`.
struct KpiCard<'a> {
    title: &'a str,
    icon: &'a str,
    big_value: &'a str,
    big_color: Color,
    spark_data: &'a [u64],
    spark_color: Color,
    delta: Option<f64>,
    up_is_good: bool,
    subtitle: &'a str,
    /// When true, the card gets a thick mauve border (the "hero" card of the
    /// row). Currently reserved for the cost card.
    is_hero: bool,
    /// Narrow-mode rendering: when true, we hide the inline sparkline row
    /// and drop the underline to keep the card usable under ~96 cols.
    compact: bool,
}

/// Render a single KPI card inside `area`.
///
/// Anatomy:
/// ```text
/// ┏━ cost ━━━━━━━━━━━ $ ┓
/// ┃                      ┃
/// ┃  $925.66             ┃
/// ┃  ━━━━━━━━━━━━        ┃
/// ┃  ▲ 8%  ▁▃▅▆▇█        ┃
/// ┃  avg $30.85 / day    ┃
/// ┗━━━━━━━━━━━━━━━━━━━━━━┛
/// ```
fn render_kpi_card(frame: &mut Frame<'_>, area: Rect, theme: &Theme, c: KpiCard) {
    // Give each card a small horizontal margin so the three cards don't kiss.
    let card_area = Rect {
        x: area.x.saturating_add(1),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    // Title + trailing icon. We render the icon via the Block's second title
    // on the right (ratatui supports multiple titles when positioned with
    // alignment).
    let title_span = Span::styled(
        format!(" {} ", c.title),
        Style::default()
            .fg(theme.subtext0)
            .add_modifier(Modifier::BOLD),
    );
    let icon_span = Span::styled(
        format!(" {} ", c.icon),
        Style::default()
            .fg(if c.is_hero { theme.peach } else { theme.overlay1 })
            .add_modifier(Modifier::BOLD),
    );

    let border_style = if c.is_hero {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface2)
    };
    let border_type = if c.is_hero {
        BorderType::Thick
    } else {
        BorderType::Rounded
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(border_style)
        .title(Line::from(vec![title_span]))
        .title(Line::from(vec![icon_span]).alignment(Alignment::Right));
    let inner = block.inner(card_area);
    frame.render_widget(block, card_area);

    // Inner layout:
    //   row 0: padding
    //   row 1: big value
    //   row 2: underline rule
    //   row 3: delta chip + mini-sparkline
    //   row 4: subtitle
    //   rows 5+: padding
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // padding
            Constraint::Length(1), // big value
            Constraint::Length(1), // underline rule
            Constraint::Length(1), // delta + spark
            Constraint::Length(1), // subtitle
            Constraint::Min(0),    // trailing padding
        ])
        .split(inner);

    // Big value — bold, colored, 2 cols of left pad.
    let value_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            c.big_value.to_string(),
            Style::default()
                .fg(c.big_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(value_line), rows[1]);

    // Underline rule — scaled to the width of the value plus a little, so
    // the eye anchors on the number. In hero mode we use the thick
    // horizontal; in non-hero we use a lighter em dash-ish row.
    let rule_w = (display_width(c.big_value) + 4).min(inner.width as usize);
    let rule_char = if c.is_hero { '━' } else { '─' };
    let rule_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            rule_char.to_string().repeat(rule_w.saturating_sub(2)),
            Style::default().fg(if c.is_hero {
                theme.mauve
            } else {
                theme.surface2
            }),
        ),
    ]);
    if !c.compact {
        frame.render_widget(Paragraph::new(rule_line), rows[2]);
    }

    // Delta chip + inline mini-sparkline.
    let chip_spans = delta_chip(c.delta, theme, c.up_is_good);
    let mut row3: Vec<Span<'_>> = Vec::with_capacity(chip_spans.len() + 4);
    row3.push(Span::raw("  "));
    row3.extend(chip_spans);
    if !c.compact {
        // Pack the sparkline into the remaining width after the chip.
        let used: usize = row3.iter().map(|s| display_width(&s.content)).sum();
        let remaining = (inner.width as usize).saturating_sub(used + 3);
        if remaining >= 6 {
            // Take the last `remaining` cells from the series so we show
            // the most recent trend.
            let slice = if c.spark_data.len() > remaining {
                &c.spark_data[c.spark_data.len() - remaining..]
            } else {
                c.spark_data
            };
            let spark = mini_sparkline(slice);
            row3.push(Span::raw("  "));
            row3.push(Span::styled(
                spark,
                Style::default()
                    .fg(c.spark_color)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(row3)), rows[3]);

    // Subtitle — the secondary line ("555.6M in · 4M out" etc).
    let subtitle_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(c.subtitle.to_string(), theme.muted()),
    ]);
    frame.render_widget(Paragraph::new(subtitle_line), rows[4]);
}

// ── Body: per-project + activity timeline ────────────────────────────────

fn render_body(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    // Projects now reserve rows for the rule, a blank, 8 data rows max, a
    // blank + legend row. Add +5 over the raw row count.
    let projects_needed = view.data.by_project.len().min(8) as u16 + 5;
    let activity_height: u16 = match view.mode {
        // GitHub grid wants: 1 rule + 1 blank + 1 header + 7 weekday rows +
        // 1 legend = 11. Clamp to 11 so the rest of the layout still fits.
        TimelineMode::Days30 | TimelineMode::Weeks12 => 11,
        TimelineMode::Hours24 => 6,
        TimelineMode::Month => 9,
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(projects_needed.min(area.height.saturating_sub(activity_height))),
            Constraint::Min(activity_height),
        ])
        .split(area);

    render_projects(frame, rows[0], view, theme);
    match view.mode {
        TimelineMode::Days30 | TimelineMode::Weeks12 => {
            render_activity(frame, rows[1], view, theme)
        }
        TimelineMode::Hours24 => render_activity_hourly(frame, rows[1], view, theme),
        TimelineMode::Month => render_activity_monthly(frame, rows[1], view, theme),
    }
}

fn render_projects(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    // Section rule — thick header for the primary data block.
    let title_text = "per project (sorted by cost) ";
    let rule = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "━━ ",
            Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            title_text,
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "━".repeat(
                area.width
                    .saturating_sub(5 + display_width(title_text) as u16)
                    as usize,
            ),
            theme.dim(),
        ),
    ]);

    let mut lines = Vec::with_capacity(view.data.by_project.len() + 4);
    lines.push(rule);
    lines.push(Line::raw(""));

    let max_cost = view
        .data
        .by_project
        .first()
        .map(|p| p.cost_usd)
        .unwrap_or(1.0)
        .max(f64::EPSILON);

    // Width budgeting for the per-project row:
    //   "  " + rank(2) + " " + name(18) + "  " + bar(flex) + "  " + right(~34) + margin
    let rank_w: usize = 2;
    let name_w: usize = 18;
    let right_w: usize = 34;
    let bar_w = (area.width as usize)
        .saturating_sub(2 + rank_w + 1 + name_w + 2 + 2 + right_w)
        .max(10);

    for (i, project) in view.data.by_project.iter().take(8).enumerate() {
        let family_color = family_color(project.color_family, theme);
        let name = truncate_str(&project.name, name_w);

        // Rank badge for the top 3.
        let rank_span = match i {
            0 => Span::styled(
                "① ",
                Style::default().fg(theme.peach).add_modifier(Modifier::BOLD),
            ),
            1 => Span::styled(
                "② ",
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            2 => Span::styled(
                "③ ",
                Style::default()
                    .fg(theme.green)
                    .add_modifier(Modifier::BOLD),
            ),
            _ => Span::styled("  ", theme.dim()),
        };

        // Multi-segment bar. The raw ratio drives total length; inside the
        // filled portion we taper heavy → medium → light so the bar reads
        // as "dominant family + tail" instead of a flat block.
        let bar_len = if max_cost > 0.0 && project.cost_usd > 0.0 {
            ((project.cost_usd / max_cost) * bar_w as f64).round() as usize
        } else {
            0
        }
        .max(1)
        .min(bar_w);

        let heavy_len = (bar_len as f64 * 0.60).ceil() as usize;
        let medium_len = ((bar_len as f64 * 0.25).round() as usize)
            .min(bar_len.saturating_sub(heavy_len));
        let light_len = bar_len
            .saturating_sub(heavy_len)
            .saturating_sub(medium_len);
        let empty_len = bar_w.saturating_sub(bar_len);

        // Pair a "secondary" color with each family so the tapered
        // segments read as a stack rather than a single hue with alpha.
        let secondary = match project.color_family {
            Family::Opus => theme.mauve, // rose → mauve tail
            Family::Sonnet => theme.blue,
            Family::Haiku => theme.green,
            Family::Unknown => theme.overlay1,
        };

        let heavy: String = "█".repeat(heavy_len);
        let medium: String = "▓".repeat(medium_len);
        let light: String = "▒".repeat(light_len);
        let empty: String = "░".repeat(empty_len);

        let right_text = format!(
            "{:>8} \u{2502} {:>6} tok \u{2502} {:>3} sess",
            format_cost(project.cost_usd),
            format_tokens(project.total_tokens),
            project.session_count,
        );

        // Name gets BOLD + family-tinted; rank 0 gets text color so the
        // leader reads as "the hero" without the family color stealing it.
        let name_color = if i == 0 { theme.text } else { family_color };

        let line = Line::from(vec![
            Span::raw("  "),
            rank_span,
            Span::styled(
                pad_right(&name, name_w),
                Style::default()
                    .fg(name_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(heavy, Style::default().fg(family_color)),
            Span::styled(medium, Style::default().fg(secondary)),
            Span::styled(light, Style::default().fg(secondary)),
            Span::styled(empty, Style::default().fg(theme.surface1)),
            Span::raw("  "),
            Span::styled(right_text, theme.muted()),
        ]);
        lines.push(line);
    }

    if view.data.by_project.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("no sessions yet.", theme.muted()),
        ]));
    } else {
        // Legend: explain the bar segments' meaning.
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                "█",
                Style::default().fg(theme.peach).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Opus    ", theme.muted()),
            Span::styled(
                "▓",
                Style::default().fg(theme.teal).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Sonnet  ", theme.muted()),
            Span::styled(
                "▒",
                Style::default().fg(theme.blue).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Haiku   ", theme.muted()),
            Span::styled(
                "░",
                Style::default().fg(theme.surface1),
            ),
            Span::styled(" remaining budget headroom", theme.dim()),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_activity(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // rule
            Constraint::Length(1), // blank
            Constraint::Min(9),    // grid body (header + 7 weekday rows + legend)
        ])
        .split(area);

    // Section rule — thick + bold to match the other hero sections.
    let title = match view.mode {
        TimelineMode::Days30 => "activity (30d) ",
        TimelineMode::Weeks12 => "activity (12w) ",
        // Heatmap modes route to their own renderers and never reach here.
        TimelineMode::Hours24 | TimelineMode::Month => "activity ",
    };
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
        Span::styled(
            "━".repeat(area.width.saturating_sub(5 + display_width(title) as u16) as usize),
            theme.dim(),
        ),
    ]);
    frame.render_widget(Paragraph::new(rule), rows[0]);

    let buckets = &view.data.daily;
    if buckets.is_empty() {
        let p = Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("no activity yet.", theme.muted()),
        ]));
        frame.render_widget(p, rows[2]);
        return;
    }

    match view.mode {
        TimelineMode::Days30 => {
            // Re-use the shared weekday-grid helper from heatmap.rs.
            let today = view.data.today;
            let cells: Vec<heatmap::WeekdayCell> = buckets
                .iter()
                .map(|d| heatmap::WeekdayCell {
                    date: d.date,
                    count: d.sessions,
                    is_today: d.date == today,
                })
                .collect();
            heatmap::render_weekday_grid(frame, rows[2], &cells, theme);
        }
        TimelineMode::Weeks12 => {
            // Fall back to the legacy bar strip for the 12-week mode — the
            // weekday grid only makes sense for day-resolution data.
            render_weekly_strip(frame, rows[2], view, theme);
        }
        _ => {}
    }
}

/// 12-week bar strip used by `TimelineMode::Weeks12`. Each bucket's `date`
/// is the Monday of that ISO week, `sessions` is the weekly total.
fn render_weekly_strip(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    let buckets = &view.data.daily;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    let max_sessions = buckets
        .iter()
        .map(|d| d.sessions)
        .max()
        .unwrap_or(1)
        .max(1);
    let n = buckets.len();
    let slot = (area.width.saturating_sub(6) as usize / n.max(1)).max(1);
    let bar_area_width = slot * n;
    let left_pad = (area.width as usize).saturating_sub(bar_area_width) / 2;

    let today_idx = buckets.len().saturating_sub(1);
    let mut bar_spans: Vec<Span> = Vec::with_capacity(n * 2 + 1);
    bar_spans.push(Span::raw(" ".repeat(left_pad)));
    for (i, d) in buckets.iter().enumerate() {
        let ch = day_bar_char(d.sessions, max_sessions);
        let style = if i == today_idx && d.sessions > 0 {
            Style::default()
                .fg(theme.peach)
                .add_modifier(Modifier::BOLD)
        } else if d.sessions == 0 {
            Style::default().fg(theme.surface1)
        } else {
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD)
        };
        bar_spans.push(Span::styled(ch.to_string(), style));
        if slot > 1 {
            bar_spans.push(Span::raw(" ".repeat(slot - 1)));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(bar_spans)), rows[1]);

    let label_line = build_label_line(view, slot, left_pad, theme);
    frame.render_widget(Paragraph::new(label_line), rows[2]);

    if today_idx > 0 && buckets.last().map(|d| d.sessions > 0).unwrap_or(false) {
        let arrow = "↑ today";
        let today_col = left_pad + today_idx * slot;
        let marker_start = today_col.saturating_sub(display_width(arrow) - 1);
        let ann = Line::from(vec![
            Span::raw(" ".repeat(marker_start)),
            Span::styled(arrow, Style::default().fg(theme.peach)),
        ]);
        frame.render_widget(Paragraph::new(ann), rows[3]);
    }
}

fn build_label_line<'a>(
    view: &StatsView<'a>,
    slot: usize,
    left_pad: usize,
    theme: &Theme,
) -> Line<'a> {
    let buckets = &view.data.daily;
    let n = buckets.len();
    if n == 0 {
        return Line::raw("");
    }

    // Anchor indices — first, ~1/4, ~1/2, ~3/4, last. Cap at 4 labels; the
    // arrow acts as the fifth anchor.
    let anchors: Vec<usize> = match n {
        0 => vec![],
        1 => vec![0],
        _ => vec![0, n / 4, n / 2, (3 * n) / 4, n - 1],
    };

    // Format each anchor's date.
    let formatter = |d: NaiveDate| -> String {
        match view.mode {
            TimelineMode::Days30 => d.format("%b %d").to_string(),
            TimelineMode::Weeks12 => format!("W{}", d.iso_week().week()),
            TimelineMode::Hours24 | TimelineMode::Month => d.format("%b %d").to_string(),
        }
    };

    // Build a padded string of width = left_pad + n*slot.
    let total_width = left_pad + n * slot;
    let mut cells: Vec<char> = vec![' '; total_width];
    for &idx in &anchors {
        let Some(bucket) = buckets.get(idx) else {
            continue;
        };
        let label = formatter(bucket.date);
        let start_col = left_pad + idx * slot;
        // Right-align the final anchor so it doesn't overflow. Use column
        // width (ASCII-only for these labels today, but consistent with the
        // rest of the audit).
        let start = if idx == n - 1 {
            total_width.saturating_sub(display_width(&label))
        } else {
            start_col
        };
        for (k, c) in label.chars().enumerate() {
            if start + k < total_width {
                cells[start + k] = c;
            }
        }
    }

    let text: String = cells.into_iter().collect();
    Line::from(vec![Span::styled(text, theme.muted())])
}

/// Map session count to a block char from the sparkline ramp.
fn day_bar_char(count: u32, max: u32) -> char {
    const RAMP: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if count == 0 {
        return '·';
    }
    if max <= 1 {
        return '▄';
    }
    // Lift the minimum visible bar off the floor so lone entries read.
    let norm = count as f64 / max as f64;
    let idx = ((norm * (RAMP.len() - 1) as f64).round() as usize).clamp(2, RAMP.len() - 1);
    RAMP[idx]
}

// ── By-model footer line ─────────────────────────────────────────────────

fn render_by_model(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    if view.data.by_model.len() < 2 {
        return;
    }

    let mut spans: Vec<Span> = Vec::with_capacity(view.data.by_model.len() * 4 + 2);
    spans.push(Span::raw("  "));
    spans.push(Span::styled("by model:", theme.muted()));
    spans.push(Span::raw("  "));

    for (i, (model, cost)) in view.data.by_model.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", theme.dim()));
        }
        let short = short_model(model);
        let name_color = if i % 2 == 0 { theme.mauve } else { theme.blue };
        spans.push(Span::styled(short, Style::default().fg(name_color)));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format_cost(*cost),
            Style::default().fg(theme.green),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ── Footer ───────────────────────────────────────────────────────────────

fn render_footer(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let hints = [
        ("q", "quit"),
        ("e", "export"),
        ("t", "cycle timeline"),
        ("b", "budget"),
        ("f", "forecast"),
        ("r", "refresh"),
    ];
    let mut spans: Vec<Span> = Vec::with_capacity(hints.len() * 4 + 2);
    spans.push(Span::raw("  "));
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", theme.dim()));
        }
        spans.push(Span::styled("press ", theme.muted()));
        spans.push(Span::styled(*key, theme.key_hint()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*desc, theme.key_desc()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ── Toast overlay ────────────────────────────────────────────────────────

fn render_toast(frame: &mut Frame<'_>, area: Rect, msg: &str, kind: ToastKind, theme: &Theme) {
    // Column-aware: a CJK toast message would previously size the modal too
    // narrow (chars ≠ columns), clipping the right edge.
    let w = (display_width(msg) as u16 + 10).clamp(20, area.width.saturating_sub(4));
    let h = 3_u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(h))
        .saturating_sub(3);
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, rect);

    let (accent, label) = match kind {
        ToastKind::Success => (theme.green, "done"),
        ToastKind::Info => (theme.mauve, "info"),
        ToastKind::Error => (theme.red, "error"),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                label,
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));
    let p = Paragraph::new(Line::from(Span::styled(format!(" {msg} "), theme.body())))
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(p, rect);
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let p = Paragraph::new(vec![
        Line::raw(""),
        Line::styled(
            "Terminal too small for stats.",
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            format!(
                "Resize to at least {}×{} (current {}×{}).",
                MIN_W, MIN_H, area.width, area.height
            ),
            theme.muted(),
        ),
    ])
    .alignment(Alignment::Center);
    frame.render_widget(p, area);
}

// ── v3.0 heatmap / histogram / budget renderers ──────────────────────────

fn render_activity_hourly(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = "activity by hour (7d) ";
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
        Span::styled(
            "━".repeat(area.width.saturating_sub(5 + display_width(title) as u16) as usize),
            theme.dim(),
        ),
    ]);
    frame.render_widget(Paragraph::new(rule), rows[0]);

    heatmap::render_hourly(frame, rows[2], &view.data.hourly_buckets, theme);

    let (peak, quiet) = heatmap::hourly_extrema(&view.data.hourly_buckets);
    let ann = match (peak, quiet) {
        (Some(p), Some(q)) => format!("peak {p:02}:00 · quiet {q:02}:00"),
        (Some(p), None) => format!("peak {p:02}:00"),
        _ => "no activity yet".to_string(),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(ann, theme.muted()),
        ])),
        rows[3],
    );
}

fn render_activity_monthly(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    let title = "month at a glance ";
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
        Span::styled(
            "━".repeat(area.width.saturating_sub(5 + display_width(title) as u16) as usize),
            theme.dim(),
        ),
    ]);
    frame.render_widget(Paragraph::new(rule), rows[0]);

    heatmap::render_monthly(frame, rows[2], &view.data.monthly, theme);

    let most = view
        .data
        .monthly
        .most_active_weekday(&view.data.monthly_tokens);
    let quiet = view.data.monthly.quietest_weekday();
    let mut ann = String::new();
    if let Some(w) = most {
        ann.push_str(&format!("most active: {}", heatmap::weekday_name(w)));
    }
    if let Some(w) = quiet {
        if !ann.is_empty() {
            ann.push_str(" · ");
        }
        ann.push_str(&format!("quiet: {}", heatmap::weekday_name(w)));
    }
    if ann.is_empty() {
        ann = "no activity this month".to_string();
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(ann, theme.muted()),
        ])),
        rows[3],
    );
}

/// Speed palette used by the turn-duration histogram. Index matches the
/// bucket index from [`DURATION_BUCKETS`]: 0 = fastest, 5 = slowest.
///
/// Ramp reads cool → hot so eye finds "slow" without any label reading:
///   0-10s    green   (fast!)
///   10-30s   yellow  (ok)
///   30-60s   yellow  (still ok)
///   1-3min   peach   (slow)
///   3-10min  red     (painful)
///   10min+   red+bold (glacial)
fn speed_color(bucket_index: usize, theme: &Theme) -> (Color, Modifier) {
    match bucket_index {
        0 => (theme.green, Modifier::BOLD),
        1 => (theme.yellow, Modifier::empty()),
        2 => (theme.yellow, Modifier::BOLD),
        3 => (theme.peach, Modifier::BOLD),
        4 => (theme.red, Modifier::BOLD),
        _ => (theme.red, Modifier::BOLD | Modifier::REVERSED),
    }
}

/// Locate the bucket where the cumulative distribution crosses `percentile`
/// (0.0..=1.0). Used to annotate p50 / p95 / p99 onto the histogram.
fn percentile_bucket(counts: &[u64; 6], total: u64, percentile: f64) -> usize {
    if total == 0 {
        return 0;
    }
    let target = (total as f64 * percentile).ceil() as u64;
    let mut acc: u64 = 0;
    for (i, c) in counts.iter().enumerate() {
        acc = acc.saturating_add(*c);
        if acc >= target {
            return i;
        }
    }
    counts.len().saturating_sub(1)
}

fn render_turn_duration_hist(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &StatsView<'_>,
    theme: &Theme,
) {
    let title = "how long does Claude think? ";
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
        Span::styled(
            "━".repeat(area.width.saturating_sub(5 + display_width(title) as u16) as usize),
            theme.dim(),
        ),
    ]);

    let stats = &view.data.turn_durations;
    let max = stats.max_count().max(1);
    let total = stats.total_turns;

    // Find the bucket holding the median (p50) and the tail markers.
    let p50 = percentile_bucket(&stats.counts, total, 0.50);
    let p95 = percentile_bucket(&stats.counts, total, 0.95);
    let p99 = percentile_bucket(&stats.counts, total, 0.99);

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(10);
    lines.push(rule);
    lines.push(Line::raw(""));

    // Summary line: mean duration, median bucket, total turns.
    let mean_str = if total > 0 {
        let secs = stats.total_wall_time.as_secs_f64() / total as f64;
        format_duration_short(Duration::from_secs_f64(secs))
    } else {
        "—".to_string()
    };
    let median_label = DURATION_BUCKETS.get(p50).map(|(l, _)| *l).unwrap_or("—");
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled("median ", theme.muted()),
        Span::styled(
            median_label,
            Style::default().fg(speed_color(p50, theme).0).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" \u{2219} mean {mean_str} \u{2219} {total} turns over 30d"),
            theme.muted(),
        ),
    ]));
    lines.push(Line::raw(""));

    let label_col = 9usize;
    let count_col = 7usize;
    let tail_col = 10usize;
    let bar_w = (area.width as usize)
        .saturating_sub(4 + label_col + 1 + count_col + 1 + tail_col + 2)
        .max(8);

    for (i, (label, _upper)) in DURATION_BUCKETS.iter().enumerate() {
        let count = stats.counts[i];
        let norm = count as f64 / max as f64;
        let bar_len = ((norm * bar_w as f64).round() as usize).min(bar_w);
        let (bar_color, bar_mod) = speed_color(i, theme);
        let bar = if count == 0 {
            "·".to_string()
        } else if bar_len == 0 {
            "▏".to_string()
        } else {
            "█".repeat(bar_len)
        };
        let pad = if count > 0 {
            " ".repeat(bar_w.saturating_sub(bar_len))
        } else {
            " ".repeat(bar_w.saturating_sub(1))
        };
        let count_str = format_u64_compact(count);

        // Percentile badges — right-aligned, after the bar.
        let mut tail_spans: Vec<Span<'_>> = Vec::with_capacity(4);
        if i == p50 && total > 0 {
            tail_spans.push(Span::styled(
                " p50",
                Style::default()
                    .fg(theme.green)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if i == p95 && i != p50 && total > 0 {
            tail_spans.push(Span::styled(
                " p95",
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if i == p99 && i != p95 && i != p50 && total > 0 {
            tail_spans.push(Span::styled(
                " p99",
                Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
            ));
        }

        let mut row: Vec<Span<'_>> = vec![
            Span::raw("    "),
            Span::styled(
                pad_to_width(label, label_col),
                Style::default()
                    .fg(theme.subtext0)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(bar, Style::default().fg(bar_color).add_modifier(bar_mod)),
            Span::raw(pad),
            Span::raw(" "),
            Span::styled(
                pad_to_width(&count_str, count_col - 1),
                theme.muted(),
            ),
        ];
        row.extend(tail_spans);
        lines.push(Line::from(row));
    }

    // Legend — explain the color coding.
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            "█",
            Style::default().fg(theme.green).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" fast  ", theme.muted()),
        Span::styled(
            "█",
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ok    ", theme.muted()),
        Span::styled(
            "█",
            Style::default().fg(theme.peach).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" slow  ", theme.muted()),
        Span::styled(
            "█",
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" glacial", theme.muted()),
    ]));

    let height = lines.len() as u16;
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: height.min(area.height),
        },
    );
}

fn render_budget_band(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    let month_name_str = month_name(view.data.today.month());
    let title = format!("budget ({} {}) ", month_name_str, view.data.today.year());
    let rule = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "━━ ",
            Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            title.clone(),
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "━".repeat(area.width.saturating_sub(5 + display_width(&title) as u16) as usize),
            theme.dim(),
        ),
    ]);

    let mtd = view.data.month_to_date_cost;
    let day_of_month = view.data.today.day();
    let days_in_month =
        MonthlyActivity::days_in_month(view.data.today.year(), view.data.today.month());
    let forecast = if day_of_month == 0 {
        mtd
    } else {
        mtd * days_in_month as f64 / day_of_month as f64
    };

    let month_pct = (day_of_month as f64 / days_in_month as f64).clamp(0.0, 1.0);

    // Row 1: month-to-date progress bar. Fill color ramps green → yellow →
    // red based on % of budget consumed (or % of month when no cap set).
    let mtd_line = build_mtd_progress_line(mtd, view.monthly_limit_usd, month_pct, theme);

    // Row 2: forecast badge. When a limit exists, compare forecast vs cap.
    let forecast_line = build_forecast_badge_line(forecast, view.monthly_limit_usd, theme);

    // Rows 3..: per-model breakdown pills, up to 3 rows.
    let model_lines = build_budget_model_lines(view, mtd, theme);

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(6 + model_lines.len());
    lines.push(rule);
    lines.push(Line::raw(""));
    lines.push(mtd_line);
    lines.push(forecast_line);
    if !model_lines.is_empty() {
        lines.push(Line::raw(""));
        lines.extend(model_lines);
    }

    frame.render_widget(Paragraph::new(lines), area);
}

/// Build the month-to-date progress bar line.
fn build_mtd_progress_line<'a>(
    mtd: f64,
    limit: f64,
    month_pct: f64,
    theme: &'a Theme,
) -> Line<'a> {
    let bar_w: usize = 30;
    let (filled_cells, base_color, over_cells, cap_label) = if limit > 0.0 {
        let used_pct = (mtd / limit).clamp(0.0, 1.5);
        let filled = ((used_pct.min(1.0)) * bar_w as f64).round() as usize;
        let over = if used_pct > 1.0 {
            (((used_pct - 1.0).min(0.5)) * bar_w as f64).round() as usize
        } else {
            0
        };
        // Traffic light by % of budget consumed.
        let color = if used_pct < 0.60 {
            theme.green
        } else if used_pct < 0.80 {
            theme.yellow
        } else if used_pct < 1.00 {
            theme.peach
        } else {
            theme.red
        };
        (
            filled,
            color,
            over,
            format!("{} / {}", format_cost(mtd), format_cost(limit)),
        )
    } else {
        // No cap — the bar tracks "% of month" as a neutral pacing signal.
        let filled = (month_pct * bar_w as f64).round() as usize;
        (
            filled.min(bar_w),
            theme.mauve,
            0,
            format!("{} (no cap set)", format_cost(mtd)),
        )
    };

    let filled = "█".repeat(filled_cells.min(bar_w));
    let over = "█".repeat(over_cells);
    let empty = "░".repeat(
        bar_w
            .saturating_sub(filled_cells.min(bar_w))
            .saturating_sub(over_cells),
    );

    // Trailing pct-of-month chip.
    let month_chip = if limit > 0.0 {
        let used = (mtd / limit * 100.0).round() as i64;
        let over_budget = mtd > limit;
        let (warn, color) = if over_budget {
            ("⚠ ", theme.red)
        } else if used >= 80 {
            ("⚠ ", theme.peach)
        } else {
            ("", theme.subtext0)
        };
        Span::styled(
            format!("   {warn}{used}% of budget"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            format!("   {:.0}% of month", month_pct * 100.0),
            theme.muted(),
        )
    };

    Line::from(vec![
        Span::raw("  "),
        Span::styled("month-to-date ", theme.muted()),
        Span::styled(
            filled,
            Style::default().fg(base_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            over,
            Style::default()
                .fg(theme.red)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        ),
        Span::styled(empty, Style::default().fg(theme.surface1)),
        Span::raw("  "),
        Span::styled(
            cap_label,
            Style::default()
                .fg(theme.text)
                .add_modifier(Modifier::BOLD),
        ),
        month_chip,
    ])
}

/// Build the "forecast → $N" row with a colored badge.
fn build_forecast_badge_line<'a>(
    forecast: f64,
    limit: f64,
    theme: &'a Theme,
) -> Line<'a> {
    let (badge, badge_color) = if limit > 0.0 {
        if forecast > limit {
            let overshoot = ((forecast - limit) / limit * 100.0).round() as i64;
            (
                format!("⚠ over-track — {overshoot}% over cap"),
                theme.red,
            )
        } else if forecast > limit * 0.90 {
            (
                format!(
                    "~ close call — {:.0}% of cap",
                    forecast / limit * 100.0
                ),
                theme.peach,
            )
        } else {
            (
                format!(
                    "✓ on-track — {:.0}% of cap",
                    forecast / limit * 100.0
                ),
                theme.green,
            )
        }
    } else {
        (
            "(set a budget with 'b' to enable tracking)".to_string(),
            theme.surface2,
        )
    };

    Line::from(vec![
        Span::raw("  "),
        Span::styled("forecast      ", theme.muted()),
        Span::styled(
            "▲ ",
            Style::default()
                .fg(forecast_color(forecast, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format_cost(forecast),
            Style::default()
                .fg(forecast_color(forecast, theme))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" at current burn   ", theme.muted()),
        Span::styled(
            badge,
            Style::default()
                .fg(badge_color)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

/// Per-model spend pills — up to 3 lines, one per model family.
fn build_budget_model_lines<'a>(
    view: &StatsView<'a>,
    mtd: f64,
    theme: &'a Theme,
) -> Vec<Line<'a>> {
    if mtd <= 0.0 {
        return Vec::new();
    }
    let total: f64 = view.data.by_model.iter().map(|(_, c)| *c).sum();
    let denom = if total > 0.0 { total } else { mtd };
    view.data
        .by_model
        .iter()
        .filter(|(_, c)| *c > 0.0)
        .take(3)
        .enumerate()
        .map(|(i, (model, cost))| {
            let short = short_model(model);
            let pct = (cost / denom * 100.0).round() as i64;
            let label = if i == 0 { "by model      " } else { "              " };
            let pill_color = family_color(
                crate::data::pricing::family(model),
                theme,
            );
            Line::from(vec![
                Span::raw("  "),
                Span::styled(label, theme.muted()),
                Span::styled(
                    "● ",
                    Style::default()
                        .fg(pill_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    pad_to_width(&short, 14),
                    Style::default()
                        .fg(pill_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format_cost(*cost),
                    Style::default()
                        .fg(theme.text)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("   ({pct}%)"),
                    theme.muted(),
                ),
            ])
        })
        .collect()
}


/// Color the forecast number by magnitude.
pub fn forecast_color(forecast: f64, theme: &Theme) -> Color {
    if forecast < 50.0 {
        theme.green
    } else if forecast < 200.0 {
        theme.yellow
    } else if forecast < 500.0 {
        theme.peach
    } else {
        theme.red
    }
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "january",
        2 => "february",
        3 => "march",
        4 => "april",
        5 => "may",
        6 => "june",
        7 => "july",
        8 => "august",
        9 => "september",
        10 => "october",
        11 => "november",
        12 => "december",
        _ => "",
    }
}

fn render_budget_modal(
    frame: &mut Frame<'_>,
    area: Rect,
    buf: &str,
    view: &StatsView<'_>,
    theme: &Theme,
) {
    let w: u16 = 52;
    let h: u16 = 9;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w.min(area.width.saturating_sub(2)),
        height: h.min(area.height.saturating_sub(2)),
    };
    frame.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "Set monthly budget",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let current_caption = if view.monthly_limit_usd > 0.0 {
        format!("current limit: ${:.0}", view.monthly_limit_usd)
    } else {
        "no limit currently set".to_string()
    };
    let input_display = if buf.is_empty() {
        "  (empty to clear)".to_string()
    } else {
        format!("$ {buf}")
    };
    let input_style = if buf.is_empty() {
        theme.dim()
    } else {
        Style::default()
            .fg(theme.green)
            .add_modifier(Modifier::BOLD)
    };

    let lines = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Enter a monthly USD cap. Leave empty to clear.",
                theme.muted(),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(input_display, input_style),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(current_caption, theme.dim()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Enter", theme.key_hint()),
            Span::raw(" "),
            Span::styled("save  ", theme.key_desc()),
            Span::styled("Esc", theme.key_hint()),
            Span::raw(" "),
            Span::styled("cancel", theme.key_desc()),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn format_u64_compact(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Human-readable `std::time::Duration` → `3m 12s` / `2h 18m`.
pub fn format_duration_short(d: Duration) -> String {
    let total = d.as_secs();
    if total < 60 {
        return format!("{total}s");
    }
    if total < 3600 {
        let m = total / 60;
        let s = total % 60;
        return format!("{m}m {s:02}s");
    }
    let h = total / 3600;
    let m = (total % 3600) / 60;
    format!("{h}h {m:02}m")
}

// ── Formatters / small helpers ───────────────────────────────────────────

pub fn format_tokens(t: u64) -> String {
    if t >= 1_000_000 {
        let m = t as f64 / 1_000_000.0;
        format!("{m:.1}M")
    } else if t >= 1_000 {
        let k = t as f64 / 1_000.0;
        format!("{k:.0}k")
    } else {
        t.to_string()
    }
}

pub fn format_cost(c: f64) -> String {
    if !c.is_finite() || c <= 0.0 {
        return "$0.00".to_string();
    }
    if c >= 1000.0 {
        format!("${c:.0}")
    } else {
        format!("${c:.2}")
    }
}

/// Shorten a long Claude model id into something a footer can fit.
///
/// `claude-opus-4-7-20260416` → `opus-4-7`, drops any date suffix.
fn short_model(model: &str) -> String {
    let stripped = model.strip_prefix("claude-").unwrap_or(model);
    // Drop a trailing `-\d{8}` date stamp if one is present.
    let parts: Vec<&str> = stripped.split('-').collect();
    let mut end = parts.len();
    if let Some(last) = parts.last() {
        if last.len() == 8 && last.chars().all(|c| c.is_ascii_digit()) {
            end = end.saturating_sub(1);
        }
    }
    parts[..end].join("-")
}

/// Primary color for a model family — the hue used for the heaviest portion
/// of the per-project stacked bar. Kept as a thin wrapper so the mapping
/// lives in one place (other widgets may later consume it too).
///
/// Opus (expensive) renders as peach; Sonnet (mid) as teal; Haiku (cheap) as
/// blue; Unknown falls back to the muted subtext tone so it doesn't compete
/// with the named families.
fn family_color(family: Family, theme: &Theme) -> Color {
    match family {
        Family::Opus => theme.peach,
        Family::Sonnet => theme.teal,
        Family::Haiku => theme.blue,
        Family::Unknown => theme.subtext0,
    }
}

/// Per-row color for the per-project block: blend family + rank so top
/// rows stand out but family stays identifiable.
#[allow(dead_code)]
fn project_color(index: usize, family: Family, theme: &Theme) -> Color {
    // Family is the primary signal.
    let family_color = match family {
        Family::Opus => theme.peach,
        Family::Sonnet => theme.teal,
        Family::Haiku => theme.blue,
        Family::Unknown => theme.subtext0,
    };
    // Top-ranked row for a family gets a mildly brighter variant so the eye
    // draws toward the biggest spender. After rank 3, everyone fades to
    // overlay0 so the chart doesn't become a rainbow.
    match index {
        0 => family_color,
        1 => match family {
            Family::Opus => theme.yellow,
            Family::Sonnet => theme.green,
            Family::Haiku => theme.sky,
            Family::Unknown => theme.overlay2,
        },
        2 => match family {
            Family::Opus => theme.pink,
            Family::Sonnet => theme.lavender,
            Family::Haiku => theme.mauve,
            Family::Unknown => theme.overlay1,
        },
        _ => theme.overlay0,
    }
}

/// Truncate `s` to at most `max_cols` display columns, grapheme-safe, with
/// ellipsis if cut. Delegates to the shared unicode helper so every screen
/// uses the same column math.
#[inline]
fn truncate_str(s: &str, max_cols: usize) -> String {
    truncate_to_width(s, max_cols)
}

/// Pad to exactly `width` display columns. Delegates to the shared helper.
#[inline]
fn pad_right(s: &str, width: usize) -> String {
    pad_to_width(s, width)
}

// ── Aggregation helpers shared with the command handler ──────────────────

/// Build a 30-bucket daily window ending at `today`, filling gaps with zeros.
///
/// Input is any collection of `(date, sessions, tokens, cost)` tuples; the
/// function groups them by day and returns exactly 30 entries. Days outside
/// the window are silently dropped (but callers should still include them in
/// totals).
pub fn build_daily_window(today: NaiveDate, raw: &[DailyStats], days: usize) -> Vec<DailyStats> {
    use std::collections::HashMap;
    let by_date: HashMap<NaiveDate, &DailyStats> = raw.iter().map(|d| (d.date, d)).collect();
    let start = today - ChronoDuration::days(days.saturating_sub(1) as i64);
    (0..days)
        .map(|i| {
            let d = start + ChronoDuration::days(i as i64);
            by_date
                .get(&d)
                .map(|s| DailyStats {
                    date: d,
                    sessions: s.sessions,
                    tokens: s.tokens,
                    cost_usd: s.cost_usd,
                })
                .unwrap_or(DailyStats {
                    date: d,
                    sessions: 0,
                    tokens: 0,
                    cost_usd: 0.0,
                })
        })
        .collect()
}

/// Collapse the 30-day series into a 12-week series. Each week bucket
/// accumulates session count / tokens / cost across its 7 days.
///
/// Returns exactly 12 entries. The final entry's `date` is the start of the
/// week that contains `today` (i.e. the most recent Monday); earlier entries
/// step back 7 days at a time.
pub fn build_weekly_window(today: NaiveDate, raw: &[DailyStats]) -> Vec<DailyStats> {
    use std::collections::HashMap;

    const WEEKS: usize = 12;
    let by_date: HashMap<NaiveDate, &DailyStats> = raw.iter().map(|d| (d.date, d)).collect();

    // Anchor the most recent week on the Monday of today.
    let days_from_monday = today.weekday().num_days_from_monday() as i64;
    let this_monday = today - ChronoDuration::days(days_from_monday);
    let start = this_monday - ChronoDuration::days(7 * (WEEKS as i64 - 1));

    (0..WEEKS)
        .map(|w| {
            let week_start = start + ChronoDuration::days(7 * w as i64);
            let mut sessions = 0u32;
            let mut tokens = 0u64;
            let mut cost = 0.0f64;
            for offset in 0..7 {
                let d = week_start + ChronoDuration::days(offset);
                if let Some(bucket) = by_date.get(&d) {
                    sessions = sessions.saturating_add(bucket.sessions);
                    tokens = tokens.saturating_add(bucket.tokens);
                    cost += bucket.cost_usd;
                }
            }
            DailyStats {
                date: week_start,
                sessions,
                tokens,
                cost_usd: cost,
            }
        })
        .collect()
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_buckets() {
        assert_eq!(format_tokens(42), "42");
        assert_eq!(format_tokens(1_500), "2k");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn format_cost_buckets() {
        assert_eq!(format_cost(0.0), "$0.00");
        assert_eq!(format_cost(12.345), "$12.35");
        assert_eq!(format_cost(1_234.5), "$1234");
    }

    #[test]
    fn short_model_drops_prefix_and_date() {
        assert_eq!(short_model("claude-opus-4-7-20260416"), "opus-4-7");
        assert_eq!(short_model("claude-sonnet-4-5"), "sonnet-4-5");
        assert_eq!(short_model("unknown-model"), "unknown-model");
    }

    #[test]
    fn day_bar_empty_for_zero() {
        assert_eq!(day_bar_char(0, 10), '·');
    }

    #[test]
    fn day_bar_lifts_floor() {
        // 1 of 10 would otherwise be `▁`, we lift to at least `▃`.
        let ch = day_bar_char(1, 10);
        assert!(ch != '▁', "floor lifted, got {ch}");
    }

    #[test]
    fn truncate_str_short_unchanged() {
        assert_eq!(truncate_str("abc", 10), "abc");
    }

    #[test]
    fn truncate_str_adds_ellipsis() {
        assert_eq!(truncate_str("abcdef", 4), "abc…");
    }

    #[test]
    fn build_daily_window_produces_exact_length() {
        let today = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();
        let raw = vec![DailyStats {
            date: today,
            sessions: 2,
            tokens: 100,
            cost_usd: 1.5,
        }];
        let series = build_daily_window(today, &raw, 30);
        assert_eq!(series.len(), 30);
        assert_eq!(series.last().unwrap().date, today);
        assert_eq!(series.last().unwrap().sessions, 2);
        assert_eq!(series[0].sessions, 0);
    }

    #[test]
    fn timeline_mode_cycle_wraps() {
        assert_eq!(TimelineMode::Days30.next(), TimelineMode::Weeks12);
        assert_eq!(TimelineMode::Weeks12.next(), TimelineMode::Hours24);
        assert_eq!(TimelineMode::Hours24.next(), TimelineMode::Month);
        assert_eq!(TimelineMode::Month.next(), TimelineMode::Days30);
    }

    #[test]
    fn timeline_mode_heatmap_predicate() {
        assert!(!TimelineMode::Days30.is_heatmap());
        assert!(!TimelineMode::Weeks12.is_heatmap());
        assert!(TimelineMode::Hours24.is_heatmap());
        assert!(TimelineMode::Month.is_heatmap());
    }

    #[test]
    fn turn_duration_bucket_boundaries() {
        assert_eq!(TurnDurationStats::bucket_index(Duration::from_secs(0)), 0);
        assert_eq!(TurnDurationStats::bucket_index(Duration::from_secs(10)), 0);
        assert_eq!(TurnDurationStats::bucket_index(Duration::from_secs(11)), 1);
        assert_eq!(TurnDurationStats::bucket_index(Duration::from_secs(30)), 1);
        assert_eq!(TurnDurationStats::bucket_index(Duration::from_secs(31)), 2);
        assert_eq!(TurnDurationStats::bucket_index(Duration::from_secs(60)), 2);
        assert_eq!(TurnDurationStats::bucket_index(Duration::from_secs(180)), 3);
        assert_eq!(TurnDurationStats::bucket_index(Duration::from_secs(600)), 4);
        assert_eq!(TurnDurationStats::bucket_index(Duration::from_secs(601)), 5);
    }

    #[test]
    fn turn_duration_push_accumulates() {
        let mut s = TurnDurationStats::default();
        s.push(Duration::from_secs(5));
        s.push(Duration::from_secs(20));
        s.push(Duration::from_secs(400));
        assert_eq!(s.counts[0], 1);
        assert_eq!(s.counts[1], 1);
        assert_eq!(s.counts[4], 1);
        assert_eq!(s.total_turns, 3);
        assert_eq!(s.total_wall_time, Duration::from_secs(425));
    }

    #[test]
    fn format_duration_short_bands() {
        assert_eq!(format_duration_short(Duration::from_secs(5)), "5s");
        assert_eq!(format_duration_short(Duration::from_secs(59)), "59s");
        assert_eq!(format_duration_short(Duration::from_secs(60)), "1m 00s");
        assert_eq!(format_duration_short(Duration::from_secs(192)), "3m 12s");
        assert_eq!(format_duration_short(Duration::from_secs(3_600)), "1h 00m");
        assert_eq!(format_duration_short(Duration::from_secs(8_280)), "2h 18m");
    }

    #[test]
    fn forecast_color_bands() {
        let t = Theme::mocha();
        assert_eq!(forecast_color(49.99, &t), t.green);
        assert_eq!(forecast_color(50.00, &t), t.yellow);
        assert_eq!(forecast_color(199.99, &t), t.yellow);
        assert_eq!(forecast_color(200.00, &t), t.peach);
        assert_eq!(forecast_color(499.99, &t), t.peach);
        assert_eq!(forecast_color(500.00, &t), t.red);
    }

    #[test]
    fn format_u64_compact_rounds() {
        assert_eq!(format_u64_compact(0), "0");
        assert_eq!(format_u64_compact(42), "42");
        assert_eq!(format_u64_compact(1_200), "1.2k");
        assert_eq!(format_u64_compact(12_345), "12.3k");
        assert_eq!(format_u64_compact(2_500_000), "2.5M");
    }

    #[test]
    fn build_weekly_window_buckets_by_seven() {
        let today = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap(); // Thursday
                                                                   // Two sessions on today, one session three days ago — both fall in
                                                                   // the same week bucket.
        let raw = vec![
            DailyStats {
                date: today,
                sessions: 2,
                tokens: 100,
                cost_usd: 1.0,
            },
            DailyStats {
                date: today - ChronoDuration::days(3),
                sessions: 1,
                tokens: 50,
                cost_usd: 0.5,
            },
        ];
        let series = build_weekly_window(today, &raw);
        assert_eq!(series.len(), 12);
        assert_eq!(series.last().unwrap().sessions, 3);
        assert!((series.last().unwrap().cost_usd - 1.5).abs() < 1e-9);
    }

    // ── KPI delta + sparkline ────────────────────────────────────────────

    #[test]
    fn half_over_half_delta_none_for_short_series() {
        assert!(half_over_half_delta(&[1.0, 2.0]).is_none());
        assert!(half_over_half_delta(&[]).is_none());
    }

    #[test]
    fn half_over_half_delta_none_for_zero_baseline() {
        // All-zero prior half → no baseline to compare against.
        let s = vec![0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 5.0, 5.0];
        assert!(half_over_half_delta(&s).is_none());
    }

    #[test]
    fn half_over_half_delta_positive_when_trending_up() {
        let s = vec![10.0, 10.0, 10.0, 10.0, 20.0, 20.0, 20.0, 20.0];
        let d = half_over_half_delta(&s).expect("some");
        assert!((d - 100.0).abs() < 1e-6);
    }

    #[test]
    fn half_over_half_delta_negative_when_trending_down() {
        let s = vec![20.0, 20.0, 20.0, 20.0, 10.0, 10.0, 10.0, 10.0];
        let d = half_over_half_delta(&s).expect("some");
        assert!((d + 50.0).abs() < 1e-6);
    }

    #[test]
    fn mini_sparkline_maps_zero_to_space() {
        let s = mini_sparkline(&[0, 0, 0]);
        assert_eq!(s, "   ");
    }

    #[test]
    fn mini_sparkline_renders_peak_as_full_block() {
        let s = mini_sparkline(&[1, 2, 10]);
        // Last entry = max → full block.
        assert_eq!(s.chars().last(), Some('█'));
    }

    #[test]
    fn mini_sparkline_length_matches_input() {
        let s = mini_sparkline(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(s.chars().count(), 6);
    }

    // ── Histogram percentile / speed ─────────────────────────────────────

    #[test]
    fn percentile_bucket_zero_total_returns_zero() {
        let counts = [0u64; 6];
        assert_eq!(percentile_bucket(&counts, 0, 0.50), 0);
    }

    #[test]
    fn percentile_bucket_median_splits_even_distribution() {
        // 10 turns, all in bucket 2.
        let mut counts = [0u64; 6];
        counts[2] = 10;
        assert_eq!(percentile_bucket(&counts, 10, 0.50), 2);
        assert_eq!(percentile_bucket(&counts, 10, 0.95), 2);
    }

    #[test]
    fn percentile_bucket_finds_tail() {
        // 100 fast + 5 slow turns: median is bucket 0, p95 slides into
        // the tail bucket.
        let mut counts = [0u64; 6];
        counts[0] = 100;
        counts[5] = 5;
        let total = 105;
        assert_eq!(percentile_bucket(&counts, total, 0.50), 0);
        // p95 = ceil(105 * 0.95) = 100; cumulative hits exactly at bucket 0.
        // Any higher percentile must push into the tail.
        assert!(percentile_bucket(&counts, total, 0.99) >= 5);
    }

    #[test]
    fn speed_color_ramps_fast_to_slow() {
        let t = Theme::mocha();
        assert_eq!(speed_color(0, &t).0, t.green);
        assert_eq!(speed_color(1, &t).0, t.yellow);
        assert_eq!(speed_color(2, &t).0, t.yellow);
        assert_eq!(speed_color(3, &t).0, t.peach);
        assert_eq!(speed_color(4, &t).0, t.red);
        assert_eq!(speed_color(5, &t).0, t.red);
    }

    // ── Model / family palette ───────────────────────────────────────────

    #[test]
    fn family_color_maps_each_variant_to_a_different_token() {
        let t = Theme::mocha();
        assert_eq!(family_color(Family::Opus, &t), t.peach);
        assert_eq!(family_color(Family::Sonnet, &t), t.teal);
        assert_eq!(family_color(Family::Haiku, &t), t.blue);
        assert_eq!(family_color(Family::Unknown, &t), t.subtext0);
    }
}
