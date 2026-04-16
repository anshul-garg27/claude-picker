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

use chrono::{Datelike, Duration as ChronoDuration, NaiveDate};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Sparkline};
use ratatui::Frame;

use crate::data::pricing::{Family, TokenCounts};
use crate::theme::Theme;

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

/// Fully aggregated payload for the stats dashboard.
#[derive(Debug, Clone)]
pub struct StatsData {
    pub totals: Totals,
    /// Top 8 by cost, sorted descending. The dashboard only has room for
    /// about 8 rows; trimming here keeps the render function dumb.
    pub by_project: Vec<ProjectStats>,
    /// Exactly 30 entries for day-mode (oldest first, ending at today), or
    /// exactly 12 for week-mode.
    pub daily: Vec<DailyStats>,
    /// Per-model cost totals, sorted descending.
    pub by_model: Vec<(String, f64)>,
    pub named_count: u32,
    pub unnamed_count: u32,
}

/// Timeline window — days vs weeks.
///
/// Toggled by the `t` key in the event loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineMode {
    /// 30 buckets, each is one day. Matches the Python default.
    Days30,
    /// 12 buckets, each is one ISO week.
    Weeks12,
}

impl TimelineMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Days30 => "last 30 days",
            Self::Weeks12 => "last 12 weeks",
        }
    }

    pub fn buckets(self) -> usize {
        match self {
            Self::Days30 => 30,
            Self::Weeks12 => 12,
        }
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
const MIN_H: u16 = 22;

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

    // Vertical layout: title, kpi row (6 lines), per-project block, activity
    // block, by-model, footer.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Length(1), // blank
            Constraint::Length(6), // kpi cards
            Constraint::Length(1), // blank
            Constraint::Min(8),    // per-project + activity flex
            Constraint::Length(1), // by-model
            Constraint::Length(1), // footer
        ])
        .split(inner);

    render_title(frame, rows[0], view, theme);
    render_kpi_row(frame, rows[2], view, theme);
    render_body(frame, rows[4], view, theme);
    render_by_model(frame, rows[5], view, theme);
    render_footer(frame, rows[6], theme);

    if let Some(msg) = view.toast {
        render_toast(frame, area, msg, view.toast_kind, theme);
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

fn render_kpi_row(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    // 3 cards, side by side, with a 1-col gutter.
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
        // of the trend survives the cast. Multiply first so $0.01 still
        // shows up as a bar.
        .map(|d| (d.cost_usd * 100.0).round() as u64)
        .collect();
    let sessions_spark: Vec<u64> = view.data.daily.iter().map(|d| d.sessions as u64).collect();

    // Card 1 — tokens
    render_kpi_card(
        frame,
        cols[0],
        theme,
        " tokens ",
        &format_tokens(t.total_tokens.total()),
        theme.text,
        &tokens_spark,
        theme.teal,
        &format!(
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
    );

    // Card 2 — cost
    render_kpi_card(
        frame,
        cols[1],
        theme,
        " cost ",
        &format_cost(t.total_cost_usd),
        theme.green,
        &cost_spark,
        theme.green,
        &format!("avg {} / day", format_cost(t.avg_cost_per_day)),
    );

    // Card 3 — sessions
    render_kpi_card(
        frame,
        cols[2],
        theme,
        " sessions ",
        &t.total_sessions.to_string(),
        theme.yellow,
        &sessions_spark,
        theme.yellow,
        &format!(
            "{} named · {} unnamed",
            view.data.named_count, view.data.unnamed_count
        ),
    );
}

/// Render a single KPI card inside `area`.
///
/// Card anatomy (6 rows):
/// ```text
/// ╭─ label ─────╮
///               │
///   109.0M  ▂█… │
///   107.7M in…  │
///               │
/// ╰─────────────╯
/// ```
#[allow(clippy::too_many_arguments)]
fn render_kpi_card(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: &Theme,
    title: &str,
    big_value: &str,
    big_color: Color,
    spark_data: &[u64],
    spark_color: Color,
    subtitle: &str,
) {
    // Give each card a small horizontal margin so the three cards don't kiss.
    let card_area = Rect {
        x: area.x.saturating_add(1),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.surface1))
        .title(Span::styled(
            title,
            Style::default()
                .fg(theme.subtext0)
                .add_modifier(Modifier::DIM),
        ));
    let inner = block.inner(card_area);
    frame.render_widget(block, card_area);

    // Inner layout: 1 row blank padding, value+spark row, subtitle row,
    // 1 row blank padding.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // padding
            Constraint::Length(1), // big value + spark
            Constraint::Length(1), // subtitle
            Constraint::Min(0),    // trailing padding
        ])
        .split(inner);

    // big value + sparkline row. The sparkline steals whatever width is left
    // after the value plus a small gap.
    let value_width = big_value.chars().count() as u16 + 3; // "  {value} "
    let spark_width = rows[1].width.saturating_sub(value_width).saturating_sub(2);
    let value_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(value_width),
            Constraint::Length(spark_width),
            Constraint::Min(0),
        ])
        .split(rows[1]);

    let value_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            big_value.to_string(),
            Style::default().fg(big_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]);
    frame.render_widget(Paragraph::new(value_line), value_cols[0]);

    if spark_width >= 4 && spark_data.iter().any(|&v| v > 0) {
        let sparkline = Sparkline::default()
            .data(spark_data)
            .style(Style::default().fg(spark_color))
            .max(spark_data.iter().copied().max().unwrap_or(1));
        frame.render_widget(sparkline, value_cols[1]);
    } else {
        // Empty sparkline — draw dots so the card doesn't look broken.
        let dots: String = "·".repeat(spark_width as usize);
        let p = Paragraph::new(Line::from(Span::styled(
            dots,
            Style::default().fg(theme.surface1),
        )));
        frame.render_widget(p, value_cols[1]);
    }

    let subtitle_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(subtitle.to_string(), theme.muted()),
    ]);
    frame.render_widget(Paragraph::new(subtitle_line), rows[2]);
}

// ── Body: per-project + activity timeline ────────────────────────────────

fn render_body(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    let projects_needed = view.data.by_project.len().min(8) as u16 + 3;
    // Reserve bottom for the activity block (4 lines: rule, bar, labels, ann).
    let activity_height = 5;

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(projects_needed.min(area.height.saturating_sub(activity_height))),
            Constraint::Min(activity_height),
        ])
        .split(area);

    render_projects(frame, rows[0], view, theme);
    render_activity(frame, rows[1], view, theme);
}

fn render_projects(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    // Section rule.
    let rule = Line::from(vec![
        Span::raw("  "),
        Span::styled("── ", theme.dim()),
        Span::styled("per project ", theme.subtle()),
        Span::styled(
            "─".repeat(area.width.saturating_sub(17) as usize),
            theme.dim(),
        ),
    ]);

    let mut lines = Vec::with_capacity(view.data.by_project.len() + 2);
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
    //   "  " + name(18) + "  " + bar(flex) + "  " + right(~32) + margin
    let name_w: usize = 18;
    let right_w: usize = 32;
    let bar_w = (area.width as usize)
        .saturating_sub(2 + name_w + 2 + 2 + right_w)
        .max(10);

    for (i, project) in view.data.by_project.iter().take(8).enumerate() {
        let color = project_color(i, project.color_family, theme);
        let name = truncate_str(&project.name, name_w);

        let bar_len = if max_cost > 0.0 && project.cost_usd > 0.0 {
            ((project.cost_usd / max_cost) * bar_w as f64).round() as usize
        } else {
            0
        }
        .max(1)
        .min(bar_w);

        let filled: String = "█".repeat(bar_len);
        let empty: String = "░".repeat(bar_w.saturating_sub(bar_len));

        let right_text = format!(
            "{:>7}  ·  {:>6} tok  ·  {:>3} ses",
            format_cost(project.cost_usd),
            format_tokens(project.total_tokens),
            project.session_count,
        );

        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                pad_right(&name, name_w),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(filled, Style::default().fg(color)),
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
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_activity(frame: &mut Frame<'_>, area: Rect, view: &StatsView<'_>, theme: &Theme) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // rule
            Constraint::Length(1), // blank
            Constraint::Length(1), // bars
            Constraint::Length(1), // labels
            Constraint::Length(1), // today marker
        ])
        .split(area);

    // Rule.
    let title = match view.mode {
        TimelineMode::Days30 => "activity (30d) ",
        TimelineMode::Weeks12 => "activity (12w) ",
    };
    let rule = Line::from(vec![
        Span::raw("  "),
        Span::styled("── ", theme.dim()),
        Span::styled(title, theme.subtle()),
        Span::styled(
            "─".repeat(area.width.saturating_sub(5 + title.len() as u16) as usize),
            theme.dim(),
        ),
    ]);
    frame.render_widget(Paragraph::new(rule), rows[0]);

    // Bars — one per bucket in view.data.daily.
    let buckets = &view.data.daily;
    if buckets.is_empty() {
        return;
    }

    let max_sessions = buckets.iter().map(|d| d.sessions).max().unwrap_or(1).max(1);

    // Lay out exactly N bars across the available width. Reserve a couple of
    // chars of padding on each side so the content doesn't hug the border.
    let n = buckets.len();
    let slot = (area.width.saturating_sub(6) as usize / n).max(1);
    let bar_area_width = slot * n;
    let left_pad = (area.width as usize).saturating_sub(bar_area_width) / 2;

    let today_idx = buckets.len().saturating_sub(1);
    let mut bar_spans: Vec<Span> = Vec::with_capacity(n * 2 + 1);
    bar_spans.push(Span::raw(" ".repeat(left_pad)));
    for (i, d) in buckets.iter().enumerate() {
        let ch = day_bar_char(d.sessions, max_sessions);
        let style = if i == today_idx && d.sessions > 0 {
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD)
        } else if d.sessions == 0 {
            Style::default().fg(theme.surface1)
        } else {
            Style::default().fg(theme.mauve)
        };
        bar_spans.push(Span::styled(ch.to_string(), style));
        // Right-pad each bar to the slot width.
        if slot > 1 {
            bar_spans.push(Span::raw(" ".repeat(slot - 1)));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(bar_spans)), rows[2]);

    // Labels — 5 anchor positions spaced across the window.
    let label_line = build_label_line(view, slot, left_pad, theme);
    frame.render_widget(Paragraph::new(label_line), rows[3]);

    // Today marker.
    if today_idx > 0 && buckets.last().map(|d| d.sessions > 0).unwrap_or(false) {
        let arrow = "↑ today";
        let today_col = left_pad + today_idx * slot;
        let marker_start = today_col.saturating_sub(arrow.len() - 1);
        let ann = Line::from(vec![
            Span::raw(" ".repeat(marker_start)),
            Span::styled(arrow, Style::default().fg(theme.green)),
        ]);
        frame.render_widget(Paragraph::new(ann), rows[4]);
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
        // Right-align the final anchor so it doesn't overflow.
        let start = if idx == n - 1 {
            total_width.saturating_sub(label.len())
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
        ("t", "toggle days/weeks"),
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
    let w = (msg.chars().count() as u16 + 10).clamp(20, area.width.saturating_sub(4));
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

/// Per-row color for the per-project block: blend family + rank so top
/// rows stand out but family stays identifiable.
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

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(max_chars * 4);
    for (i, ch) in s.chars().enumerate() {
        if i >= max_chars - 1 {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

fn pad_right(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count >= width {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + (width - count));
    out.push_str(s);
    for _ in 0..(width - count) {
        out.push(' ');
    }
    out
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
}
