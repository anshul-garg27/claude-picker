//! Session list widget — the left pane of the picker.
//!
//! Draws the filter input at the top (inside the outer panel border) and a
//! stateful list below. Each row is hand-rendered as a [`Line`] with three
//! visual zones:
//!
//! - **Name** — left-aligned, truncated with `…` if it overflows the column.
//! - **Cost** — right-aligned, tinted by magnitude (dim / yellow / red).
//! - **Age** — relative timestamp from the session's last activity.
//!
//! The selected row is prefixed with `▸` and tinted via
//! [`crate::theme::Theme::selected_row`], matching the mockup.
//!
//! **Density polish (E6/E7):** each row carries two 1-cell heat gauges —
//! a cost-burn bar just before the cost column (green/amber/rose by spend
//! bucket) and a context-usage gutter anchored at the right edge of the pane
//! (green/amber/rose by `tokens.total()` against a 200k budget). Both
//! degrade gracefully on narrow panes: gutter hides under 60 cols, cost-burn
//! bar under 80 cols.

use std::borrow::Cow;
use std::cell::RefCell;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, TimeZone, Utc};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget,
};
use ratatui::Frame;

use tachyonfx::{Effect, Shader};

use crate::app::App;
use crate::data::chains::{self, Chain};
use crate::data::Session;
use crate::theme::{self, Theme};
use crate::ui::fx as ui_fx;
use crate::ui::model_pill;
use crate::ui::text::{display_width, truncate_to_width};

// ── Strict column budget ─────────────────────────────────────────────────
//
// The row layout is a hard-bounded `Layout::horizontal` split. Every column
// has a budget it cannot exceed. The earlier implementation built one loose
// `Line<Span>` and trusted content widths to line up — but CJK names, cost
// chips with 5-digit dollar amounts, and long teaser strings all crushed
// into each other. Now we `Layout::horizontal` the row into 10 fixed rects
// and render each column into its own rect so overflow clips cleanly at the
// column boundary.
//
//  Col 1  prefix `▸`     2 cols    always
//  Col 2  name           24 cols   bold primary label, truncates with …
//  Col 3  auto subtitle  16 cols   optional, collapses when empty
//  Col 4  model pill     10 cols   `▌opus▐` / `▌sonnet▐` / `▌haiku▐`
//  Col 5  perm badge     8 cols    `▌plan▐` etc — empty when default
//  Col 6  subagent count 3 cols    `②` or blank when 0
//  Col 7  teaser         flex      `"…"` italic, clips at column edge
//  Col 8  cost chip      9 cols    `▌$12.40▐` right-aligned
//  Col 9  timestamp      7 cols    `3m ago`
//  Col 10 context gutter 1 col     gradient tint
//
// Breakpoints (see `ColumnPlan::for_width`):
//   width < 100   drop auto subtitle + teaser
//   width < 80    also drop permission badge
//   width < 60    also drop model pill + subagent counter
//   width < 40    name + cost + age only (drop gutter too)

/// Display width of the name column. Kept public-crate-only so tests and
/// helpers can key off the same constant.
const NAME_COL_WIDTH: usize = 24;
/// Width of the auto-name subtitle column.
const SUBTITLE_COL_WIDTH: usize = 16;
/// Width of the model pill column.
const MODEL_COL_WIDTH: usize = 10;
/// Width of the permission badge column.
const PERM_COL_WIDTH: usize = 8;
/// Width of the subagent counter column (` N ` with a space margin).
const SUBAGENT_COL_WIDTH: usize = 3;
/// Width of the cost chip column — `▌$1234▐` fits in 7 display cells so 9
/// leaves a two-cell breathing margin on either side.
const COST_COL_WIDTH: usize = 9;
/// Width of the age column — widest literal is `Apr 10` at 6 cells.
const AGE_COL_WIDTH: usize = 7;
/// Width of the context gutter sliver — a single cell at the right edge.
const GUTTER_COL_WIDTH: usize = 1;
/// Width of the leading pointer / prefix column (`▸` plus margin).
const PREFIX_COL_WIDTH: usize = 2;
/// Minimum flex budget given to the teaser column — below this we drop the
/// teaser entirely so the cost/age columns can keep their allocation.
const TEASER_MIN_FLEX: u16 = 8;

/// Render the entire left pane into `area`.
///
/// The pane is wrapped in a rounded-border block; the filter input lives
/// inside that block (as the top 3 rows), and the list fills the rest.
pub fn render(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    // Outer block — title top-left, per-project totals top-right. We show
    // `total $X.XX · YY.YM tok` on the right so the user sees the project's
    // running spend without leaving the picker. Only renders when we have
    // sessions (empty-state keeps the bar clean).
    let title = outer_title_spans(app);
    let counter = project_totals_line(app);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style(app, theme))
        .title(title)
        .title_top(counter);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Inner vertical split: filter (3 rows) + list (flex).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    render_filter(f, chunks[0], app);
    render_list(f, chunks[1], app);

    // F3 — bottom-right 3-line pulse HUD. Draws *after* the list so it
    // overlays the bottom rows. Skipped on very short panes where we'd
    // cover meaningful list rows. See `PulseHudState` for the animation
    // pipeline.
    render_pulse_hud(f, chunks[1], app);
}

/// Build the spans that go in the outer-block title.
///
/// Format: ` ~ claude-picker › <N> sessions ` — the breadcrumb arrow (U+203A)
/// keeps hierarchy readable without stealing the eye from the brand mark.
fn outer_title_spans(app: &App) -> Line<'_> {
    let theme = &app.theme;
    let session_count = app.sessions.len();
    let count_label = if session_count == 1 {
        "1 session".to_string()
    } else {
        format!("{session_count} sessions")
    };
    Line::from(vec![
        Span::raw(" "),
        Span::styled("~", theme.muted()),
        Span::raw(" "),
        Span::styled(
            "claude-picker",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" \u{203A} ", theme.dim()),
        Span::styled(count_label, theme.muted()),
        Span::raw(" "),
    ])
}

/// Build the right-aligned project-totals line: ` total $X.XX · YY.YM tok `.
///
/// Sums every loaded session's cost + token total. Kept light — we already
/// have the Session vector in memory; this is an O(N) pass on every frame
/// but N is the in-project count and rendering is the bottleneck anyway.
fn project_totals_line(app: &App) -> Line<'_> {
    let theme = &app.theme;
    if app.sessions.is_empty() {
        // Nothing to total — show the original "filtered/total" counter so
        // the user still gets feedback while typing.
        return Line::from(vec![Span::styled(
            format!("{}/{}", app.filtered_indices.len(), app.sessions.len()),
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        )])
        .right_aligned();
    }
    let mut total_cost = 0.0f64;
    let mut total_tokens: u64 = 0;
    for s in &app.sessions {
        total_cost += s.total_cost_usd;
        total_tokens = total_tokens.saturating_add(s.tokens.total());
    }
    let cost_label = if total_cost < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${total_cost:.2}")
    };
    let tok_label = if total_tokens >= 1_000_000 {
        format!("{:.1}M tok", total_tokens as f64 / 1_000_000.0)
    } else if total_tokens >= 1_000 {
        format!("{:.1}k tok", total_tokens as f64 / 1_000.0)
    } else {
        format!("{total_tokens} tok")
    };
    // Filtered/total counter rides in front so users still see how their
    // filter has narrowed the list.
    Line::from(vec![
        Span::styled(
            format!("{}/{}", app.filtered_indices.len(), app.sessions.len()),
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  \u{2219}  ", theme.dim()),
        Span::styled("total", theme.muted()),
        Span::raw(" "),
        Span::styled(
            cost_label,
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  \u{2219}  ", theme.dim()),
        Span::styled(tok_label, theme.muted()),
        Span::raw(" "),
    ])
    .right_aligned()
}

/// Border style depends on focus — the active pane uses mauve, inactive uses
/// the dim `surface1`.
fn border_style(app: &App, theme: &Theme) -> Style {
    if app.filter_focused {
        theme.panel_border_active()
    } else {
        theme.panel_border()
    }
}

/// Filter input at the top of the pane — rendered as a bordered paragraph.
///
/// When the filter has content the border pops to mauve so users can tell at
/// a glance that typing is landing in the filter. Empty filter keeps the
/// dim `surface1` border so the pane's active outline isn't duplicated.
fn render_filter(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    let text: Line<'_> = if app.filter.is_empty() {
        Line::from(vec![
            Span::styled("> ", theme.muted()),
            Span::styled("type to filter…", theme.filter_placeholder()),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", theme.muted()),
            Span::styled(app.filter.clone(), theme.filter_text()),
            // Block cursor at end — rendered as a reverse-video space.
            Span::styled(" ", Style::default().bg(theme.mauve).fg(theme.crust)),
        ])
    };

    let (border_color, border_type) = if !app.filter.is_empty() {
        // Active filter: bright mauve thick border so it's unmistakable
        // the keystrokes are landing here. Thick ties back to Linear's
        // "active input" language of a heavier weight stroke.
        (Style::default().fg(theme.mauve), BorderType::Thick)
    } else if app.filter_focused {
        (theme.panel_border_active(), BorderType::Rounded)
    } else {
        (Style::default().fg(theme.surface1), BorderType::Rounded)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(border_color);

    let p = Paragraph::new(text).block(block);
    f.render_widget(p, area);
}

/// Render the list of sessions.
///
/// Replaces the previous `List<ListItem>` pipeline with a direct per-row
/// render pass: each visible row gets its own horizontal `Layout` split into
/// the ten strict columns described above. Rendering into per-column rects
/// (rather than a single `Line<Span>`) makes overflow clip at the column
/// edge instead of bleeding into the next column — the exact bug the v0.5.0
/// visual overhaul introduced when it stacked rich pills beside a loose
/// teaser.
///
/// Scrolling uses the same viewport logic ratatui's `List` applies (anchor
/// the top when the cursor sits inside the first page, anchor the bottom
/// otherwise). The external `Scrollbar` widget keys off `app.cursor` so the
/// thumb stays in sync with the manual scroll.
fn render_list(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    // Empty states — different copy depending on cause. Each variant carries
    // a large glyph, a primary subtext1-bold message, and a dim italic
    // secondary hint (see the `empty_copy_*` helpers below).
    if app.sessions.is_empty() {
        empty_state(f, area, theme, empty_copy_no_sessions(theme));
        return;
    }
    if app.filtered_indices.is_empty() {
        empty_state(f, area, theme, empty_copy_no_matches(&app.filter, theme));
        return;
    }

    if area.height == 0 || area.width == 0 {
        return;
    }

    let plan = ColumnPlan::for_width(area.width);
    let visible_rows = area.height as usize;
    let total = app.filtered_indices.len();
    let cursor = app.cursor.min(total.saturating_sub(1));
    let start = scroll_start(cursor, visible_rows, total);

    // Chain detection: group sessions that look like the same feature
    // continued across runs. Computed once per frame (O(N log N) sort + O(N)
    // walk in `detect_chains`), then looked up per visible row. Members get a
    // `⛓` badge prepended to their title in mauve.
    let chains_list: Vec<Chain> = chains::detect_chains(&app.sessions);

    // Render visible rows one-by-one into vertically sliced rects so each
    // row can run its own column-layout pass. `f.buffer_mut()` is held only
    // for the painting call — no overlapping mutable borrows.
    let buf = f.buffer_mut();
    for (offset, display_idx) in (start..total.min(start + visible_rows)).enumerate() {
        let sess_idx = app.filtered_indices[display_idx];
        let s = &app.sessions[sess_idx];
        let is_selected = Some(display_idx) == app.cursor_position();
        let is_bookmarked = app.bookmarks.contains(&s.id);
        let is_multi = app.is_multi_selected(sess_idx);
        let is_glide = app.is_glide_trail(display_idx);
        let is_chained = chains::chain_for_session(&s.id, &chains_list).is_some();
        let row_area = Rect {
            x: area.x,
            y: area.y + offset as u16,
            width: area.width,
            height: 1,
        };
        render_row_into(
            buf,
            row_area,
            s,
            theme,
            is_selected,
            is_bookmarked,
            is_multi,
            is_glide,
            is_chained,
            &plan,
        );
    }

    // Scrollbar on the right edge. Skip entirely when everything fits — a
    // thumb that covers the whole track is noisy.
    if total > area.height as usize {
        render_scrollbar(f, area, total, app.cursor, theme);
    }
}

/// Viewport anchor for the list — matches ratatui's built-in `List` scroll
/// behaviour. Top-anchored while the selection sits on the first page,
/// bottom-anchored afterwards so the cursor never scrolls off-screen.
fn scroll_start(selected: usize, visible_rows: usize, total: usize) -> usize {
    if visible_rows == 0 || total <= visible_rows {
        return 0;
    }
    if selected < visible_rows {
        0
    } else {
        selected + 1 - visible_rows
    }
}

/// Draw a minimalist Catppuccin-coloured scrollbar on the right edge of
/// `area`. The scrollbar is a separate stateful widget — Ratatui renders the
/// track + thumb in a 1-column column at the right of whatever rect we pass.
fn render_scrollbar(f: &mut Frame<'_>, area: Rect, total: usize, position: usize, theme: &Theme) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_style(Style::default().fg(theme.surface1))
        .thumb_style(Style::default().fg(theme.mauve));
    let mut scrollbar_state = ScrollbarState::new(total)
        .position(position)
        .viewport_content_length(area.height as usize);
    f.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 0,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

/// Breakpoint plan for one row: which optional columns are in play, and the
/// exact `Constraint` list the `Layout::horizontal` split uses.
///
/// Computed once per frame and reused for every visible row so every row in
/// the list lines up pixel-perfect regardless of content. When the pane
/// shrinks below a threshold the matching column collapses to zero so later
/// columns slide left — `Paragraph::render` on a zero-width rect is a safe
/// no-op.
#[derive(Debug, Clone, Copy)]
struct ColumnPlan {
    show_subtitle: bool,
    show_model: bool,
    show_perm: bool,
    show_subagent: bool,
    show_teaser: bool,
    show_cost: bool,
    show_age: bool,
    show_gutter: bool,
}

impl ColumnPlan {
    /// Pick which columns are rendered for a pane of `width` cells. The
    /// thresholds mirror the spec: wider panes surface the auto-name +
    /// teaser; 80–100 keeps the badges; 60–80 drops permission / teaser;
    /// 40–60 becomes a bare-bones name/cost/age strip; below 40 we drop
    /// the gutter too, leaving just prefix + name + cost + age.
    fn for_width(width: u16) -> Self {
        let w = width as usize;
        Self {
            show_subtitle: w >= 100,
            show_model: w >= 60,
            show_perm: w >= 80,
            show_subagent: w >= 60,
            show_teaser: w >= 100,
            // `prefix (2) + name (24) + cost (9) + age (7) = 42` — cost/age
            // only fit together with the name column once the pane is ≥ 42
            // cells. Below that we drop them all so the name column has
            // room to read on tiny panes (picker-in-a-corner use case).
            // The gutter needs an extra cell (43) before it turns on.
            show_cost: w >= 42,
            show_age: w >= 42,
            show_gutter: w >= 43,
        }
    }

    /// Build the ratatui constraint list for this breakpoint plan. The
    /// indices line up with the `RowColumns` match on the `areas` tuple —
    /// every entry is always present, collapsed branches use `Length(0)`.
    fn constraints(&self) -> [Constraint; 10] {
        [
            Constraint::Length(PREFIX_COL_WIDTH as u16),
            Constraint::Length(NAME_COL_WIDTH as u16),
            Constraint::Length(if self.show_subtitle { SUBTITLE_COL_WIDTH as u16 } else { 0 }),
            Constraint::Length(if self.show_model { MODEL_COL_WIDTH as u16 } else { 0 }),
            Constraint::Length(if self.show_perm { PERM_COL_WIDTH as u16 } else { 0 }),
            Constraint::Length(if self.show_subagent { SUBAGENT_COL_WIDTH as u16 } else { 0 }),
            Constraint::Min(if self.show_teaser { TEASER_MIN_FLEX } else { 0 }),
            Constraint::Length(if self.show_cost { COST_COL_WIDTH as u16 } else { 0 }),
            Constraint::Length(if self.show_age { AGE_COL_WIDTH as u16 } else { 0 }),
            Constraint::Length(if self.show_gutter { GUTTER_COL_WIDTH as u16 } else { 0 }),
        ]
    }
}

/// Render a single row into `row_area` using the strict-column budget.
///
/// Every column paints into its own `Rect`, which means the ratatui buffer
/// clips content at the column boundary — no `▌$12.40▐` can run over into
/// the age column, and no long teaser can overrun the cost chip. The row
/// background stripe (selected / glide-trail) paints first as a full-width
/// `surface0` wash so the gutter and padding also carry the highlight.
///
/// **v2.2 polish layers still in play:**
/// - Cost column uses `cost_severity_fg` (teal → green → yellow → peach).
/// - Unselected rows fade toward `overlay0` based on the session's last
///   activity — older rows visibly dim so recency reads without dates.
/// - Multi-selected / cursor rows keep full intensity for contrast.
///
/// **Density layers (E6/E7):**
/// - Permission badge / subagent counter / model pill each live in their
///   own column so they can never step on the teaser or the cost chip.
/// - The context-usage gutter is the final 1-cell column. Its colour maps
///   the session's token total against the 200 k window (green/amber/rose
///   by 40 %/80 % thresholds) so readers can spot "this one is close to
///   the wall" without reading numbers.
#[allow(clippy::too_many_arguments)]
fn render_row_into(
    buf: &mut Buffer,
    row_area: Rect,
    s: &Session,
    theme: &Theme,
    selected: bool,
    bookmarked: bool,
    multi: bool,
    glide_trail: bool,
    chained: bool,
    plan: &ColumnPlan,
) {
    // Age in seconds since the last activity timestamp — drives the row-fade.
    // Missing timestamps fade fully (treat as "very old").
    let age = session_age(s);

    // Whether this row should run through the age-fade filter at all. The
    // brief says: fade ONLY unselected rows; selection stays full brightness
    // for contrast. Multi-select rows also stay full-bright.
    let apply_fade = !selected && !multi;

    // Full-row background wash. Painted first so every column rect (including
    // the gutter sliver and any padding inside `Paragraph::render`) carries
    // the highlight. Non-selected / non-glide rows leave the wash empty so
    // downstream styles keep their original `None` background.
    if selected || glide_trail {
        let wash_style = Style::default().bg(theme.surface0);
        for x in row_area.x..row_area.x.saturating_add(row_area.width) {
            for y in row_area.y..row_area.y.saturating_add(row_area.height) {
                buf[(x, y)].set_style(wash_style);
            }
        }
    }

    // Split the row into per-column rects up front. `Layout::horizontal`
    // clamps to the available width so even pathological narrow panes never
    // panic; collapsed columns (Length(0)) simply yield a zero-width rect
    // which `Paragraph::render` treats as a no-op.
    let constraints = plan.constraints();
    let rects = Layout::horizontal(constraints).split(row_area);
    let prefix_rect = rects[0];
    let name_rect = rects[1];
    let subtitle_rect = rects[2];
    let model_rect = rects[3];
    let perm_rect = rects[4];
    let subagent_rect = rects[5];
    let teaser_rect = rects[6];
    let cost_rect = rects[7];
    let age_rect = rects[8];
    let gutter_rect = rects[9];

    let bg = if selected || glide_trail {
        Some(theme.surface0)
    } else {
        None
    };
    let stamp_bg = |mut style: Style| -> Style {
        if let Some(c) = bg {
            style = style.bg(c);
        }
        style
    };

    // ── Col 1: prefix ────────────────────────────────────────────────────
    // `✓` takes the pointer slot when the row is multi-selected (whether or
    // not the cursor is on it). The cursor row without multi-selection keeps
    // the `▸` pointer so the active row is still clear at a glance.
    let pointer_style_base = if multi {
        // Tick mark styled peach so it reads as "you picked me".
        Style::default()
            .fg(theme.peach)
            .add_modifier(Modifier::BOLD)
    } else if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface2)
    };
    let pointer_style = stamp_bg(maybe_fade(pointer_style_base, theme, age, apply_fade));
    let pointer = if multi {
        "✓"
    } else if selected {
        "\u{25B8}"
    } else {
        " "
    };
    let prefix_line = Line::from(vec![
        Span::styled(format!(" {pointer}"), pointer_style),
    ]);
    Paragraph::new(prefix_line)
        .style(stamp_bg(Style::default()))
        .render(prefix_rect, buf);

    // ── Col 2: name ──────────────────────────────────────────────────────
    // Multi-select rows recolor the name in peach-bold regardless of cursor
    // state so the visual distinction reads at a glance. Selection still wins
    // for the cursor row's background stripe (applied below).
    let name_style_base = if multi {
        Style::default()
            .fg(theme.peach)
            .add_modifier(Modifier::BOLD)
    } else if selected {
        theme.selected_row()
    } else if s.name.is_some() {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.subtext0)
            .add_modifier(Modifier::ITALIC)
    };
    let name_style = stamp_bg(maybe_fade(name_style_base, theme, age, apply_fade));
    // Bookmark / fork glyph sits in the first 2 cells of the name rect.
    let lead_span = if bookmarked {
        Span::styled(
            "\u{25A0} ",
            stamp_bg(maybe_fade(
                Style::default().fg(theme.blue),
                theme,
                age,
                apply_fade,
            )),
        )
    } else if s.is_fork {
        Span::styled(
            "\u{21B3} ",
            stamp_bg(maybe_fade(
                Style::default().fg(theme.peach),
                theme,
                age,
                apply_fade,
            )),
        )
    } else {
        Span::raw("  ")
    };
    // Chain badge: `⛓ ` prepended in mauve when this session is part of a
    // detected chain. The glyph + trailing space eats 2 display cells so we
    // shrink the name budget by the same amount to keep the column lined up
    // with its neighbours.
    let chain_span = if chained {
        Some(Span::styled(
            "\u{26D3} ",
            stamp_bg(maybe_fade(
                Style::default().fg(theme.mauve),
                theme,
                age,
                apply_fade,
            )),
        ))
    } else {
        None
    };
    let chain_cost = if chained { 2 } else { 0 };
    let name_budget = NAME_COL_WIDTH.saturating_sub(2 + chain_cost); // lead + optional chain badge
    let primary_raw = s.display_label();
    let primary_text = if display_width(primary_raw) > name_budget {
        truncate_to_width(primary_raw, name_budget)
    } else {
        primary_raw.to_string()
    };
    let mut name_spans: Vec<Span<'_>> = Vec::with_capacity(3);
    name_spans.push(lead_span);
    if let Some(chain) = chain_span {
        name_spans.push(chain);
    }
    name_spans.push(Span::styled(primary_text, name_style));
    let name_line = Line::from(name_spans);
    Paragraph::new(name_line)
        .style(stamp_bg(Style::default()))
        .render(name_rect, buf);

    // ── Col 3: auto-name subtitle ────────────────────────────────────────
    // Only drawn when the user explicitly set `name` AND we have a distinct
    // `auto_name`. The `· ` separator is eaten by the column budget so the
    // reader still sees the hierarchy.
    if plan.show_subtitle {
        if let (Some(name), Some(auto)) = (s.name.as_deref(), s.auto_name.as_deref()) {
            if !name.is_empty() && !auto.is_empty() && name != auto {
                let budget = SUBTITLE_COL_WIDTH.saturating_sub(2);
                let suffix_text = truncate_to_width(auto, budget);
                let suffix_style = stamp_bg(maybe_fade(
                    Style::default().fg(theme.subtext1),
                    theme,
                    age,
                    apply_fade,
                ));
                let subtitle_line = Line::from(vec![
                    Span::styled("\u{00B7} ", stamp_bg(theme.dim())),
                    Span::styled(suffix_text, suffix_style),
                ]);
                Paragraph::new(subtitle_line)
                    .style(stamp_bg(Style::default()))
                    .render(subtitle_rect, buf);
            } else {
                Paragraph::new(Line::from(""))
                    .style(stamp_bg(Style::default()))
                    .render(subtitle_rect, buf);
            }
        } else {
            Paragraph::new(Line::from(""))
                .style(stamp_bg(Style::default()))
                .render(subtitle_rect, buf);
        }
    }

    // ── Col 4: model pill ────────────────────────────────────────────────
    if plan.show_model {
        let mut pill = model_pill::pill(crate::data::pricing::family(&s.model_summary), theme);
        if apply_fade {
            if let Some(fg) = pill.style.fg {
                pill.style.fg = Some(theme::age_fade(theme, fg, age));
            }
        }
        if let Some(bgc) = bg {
            // When the row is selected, the chip's `surface0` bed would be
            // invisible against the row wash — bump it to `surface1` so the
            // chip still reads as a floating slug over the stripe.
            if pill.style.bg == Some(theme.surface0) {
                pill.style.bg = Some(theme.surface1);
            } else {
                pill.style.bg = Some(bgc);
            }
        }
        let line = Line::from(vec![pill]).alignment(Alignment::Left);
        Paragraph::new(line)
            .style(stamp_bg(Style::default()))
            .render(model_rect, buf);
    }

    // ── Col 5: permission badge ──────────────────────────────────────────
    if plan.show_perm {
        if let Some(mut badge) = s
            .permission_mode
            .and_then(|m| permission_badge(m, theme, apply_fade, age))
        {
            // Keep the badge bg — it's the point of the "danger slug" effect
            // — but preserve row wash outside the badge's own cells via the
            // surrounding `stamp_bg`.
            // Truncate the interior label when the budget is tight. Pill
            // glyphs are the two half-blocks (U+258C, U+2590); cap the full
            // content to PERM_COL_WIDTH.
            if display_width(badge.content.as_ref()) > PERM_COL_WIDTH {
                let trimmed = truncate_to_width(badge.content.as_ref(), PERM_COL_WIDTH);
                badge.content = Cow::Owned(trimmed);
            }
            let line = Line::from(vec![Span::raw(" "), badge]);
            Paragraph::new(line)
                .style(stamp_bg(Style::default()))
                .render(perm_rect, buf);
        } else {
            Paragraph::new(Line::from(""))
                .style(stamp_bg(Style::default()))
                .render(perm_rect, buf);
        }
    }

    // ── Col 6: subagent counter ──────────────────────────────────────────
    if plan.show_subagent {
        if s.subagent_count > 0 {
            let base = if selected {
                theme.selected_row()
            } else {
                Style::default().fg(theme.teal).add_modifier(Modifier::BOLD)
            };
            let digit_style = stamp_bg(maybe_fade(base, theme, age, apply_fade));
            let line = Line::from(vec![Span::styled(
                format!(" {} ", circled_digit(s.subagent_count)),
                digit_style,
            )]);
            Paragraph::new(line)
                .style(stamp_bg(Style::default()))
                .render(subagent_rect, buf);
        } else {
            Paragraph::new(Line::from(""))
                .style(stamp_bg(Style::default()))
                .render(subagent_rect, buf);
        }
    }

    // ── Col 7: teaser ────────────────────────────────────────────────────
    // The teaser consumes whatever flex is left. The `Paragraph` clips at
    // the column edge so long prompts never leak into the cost chip.
    if plan.show_teaser && teaser_rect.width >= TEASER_MIN_FLEX {
        if let Some(t) = build_teaser_span(s, theme, selected, apply_fade, age, teaser_rect.width) {
            let line = Line::from(vec![Span::raw(" "), t]);
            Paragraph::new(line)
                .style(stamp_bg(Style::default()))
                .render(teaser_rect, buf);
        }
    }

    // ── Col 8: cost chip ─────────────────────────────────────────────────
    if plan.show_cost {
        let mut chip = cost_chip_span(s.total_cost_usd, theme, selected, apply_fade, age);
        // Fit the chip inside its column. `▌$1234.56▐` is 10 cells — drop
        // to `▌$1234▐` so we never exceed the 9-col budget.
        if display_width(chip.content.as_ref()) > COST_COL_WIDTH {
            chip.content = Cow::Owned(truncate_cost_chip(chip.content.as_ref(), COST_COL_WIDTH));
        }
        // Chip bg over a highlighted row — swap `surface0` for `surface1` so
        // the pill still floats visibly.
        if let Some(bgc) = bg {
            if chip.style.bg == Some(theme.surface0) {
                chip.style.bg = Some(theme.surface1);
            } else {
                chip.style.bg = Some(bgc);
            }
        }
        let line = Line::from(vec![chip]).alignment(Alignment::Right);
        Paragraph::new(line)
            .style(stamp_bg(Style::default()))
            .render(cost_rect, buf);
    }

    // ── Col 9: age ───────────────────────────────────────────────────────
    if plan.show_age {
        let age_label = relative_time(s.last_timestamp);
        let age_style_inner = if selected {
            theme.selected_row()
        } else {
            maybe_fade(age_style(s.last_timestamp, theme), theme, age, apply_fade)
        };
        let line = Line::from(vec![Span::styled(
            format!(" {age_label}"),
            stamp_bg(age_style_inner),
        )]);
        Paragraph::new(line)
            .style(stamp_bg(Style::default()))
            .render(age_rect, buf);
    }

    // ── Col 10: context gutter ───────────────────────────────────────────
    if plan.show_gutter && gutter_rect.width > 0 {
        let ctx_fg = ctx_gutter_color(s, theme);
        let mut gutter_style = Style::default().fg(ctx_fg).add_modifier(Modifier::BOLD);
        if let Some(bgc) = bg {
            gutter_style = gutter_style.bg(bgc);
        }
        // Right-eighth block (U+2595) hugs the right edge without filling
        // the full cell, so selection backgrounds still read cleanly.
        let line = Line::from(vec![Span::styled("\u{2595}", gutter_style)]);
        Paragraph::new(line)
            .style(stamp_bg(Style::default()))
            .render(gutter_rect, buf);
    }
}

/// Shrink a cost chip's label so the total display width fits `budget`.
///
/// The chip shape is `▌<label>▐` (U+258C … U+2590) — the two half-block
/// glyphs flanking a dollar string. When the full chip exceeds the column
/// budget we drop the fractional part (`$1234.56` → `$1234`) before
/// ellipsising, because users prefer reading "$1,234" to "$1.2…".
fn truncate_cost_chip(chip: &str, budget: usize) -> String {
    let total_w = display_width(chip);
    if total_w <= budget || budget < 3 {
        return chip.to_string();
    }
    // Pull the interior between the rails, if any.
    let interior = chip
        .strip_prefix('\u{258C}')
        .and_then(|rest| rest.strip_suffix('\u{2590}'))
        .unwrap_or(chip);
    // Try dropping cents first — "$1234.56" → "$1234".
    let without_cents = match interior.rsplit_once('.') {
        Some((head, tail)) if tail.chars().all(|c| c.is_ascii_digit()) => head.to_string(),
        _ => interior.to_string(),
    };
    let rails = 2; // one cell per half-block
    let interior_budget = budget.saturating_sub(rails);
    let fitted = if display_width(&without_cents) <= interior_budget {
        without_cents
    } else {
        truncate_to_width(&without_cents, interior_budget)
    };
    format!("\u{258C}{fitted}\u{2590}")
}

/// Build the "name zone" for a row: primary label in `primary_style`, an
/// optional dim `· auto-name` suffix in subtext1. Returns the spans plus the
/// total display width used so the caller can pad to [`NAME_COL_WIDTH`] for
/// column-true pill alignment.
///
/// Retained from the pre-column-layout implementation for any external
/// callers that still want a composite title+subtitle zone (the live row
/// renderer now paints the title into its own rect and the subtitle into a
/// second rect for hard column clipping).
#[allow(dead_code)]
fn build_name_zone<'a>(
    s: &'a Session,
    primary_style: Style,
    apply_fade: bool,
    age: Duration,
    theme: &Theme,
) -> (Vec<Span<'a>>, usize) {
    let col_width = NAME_COL_WIDTH;
    let primary = s.display_label();
    let (primary_text, primary_w) = if display_width(primary) > col_width {
        let truncated = truncate_to_width(primary, col_width);
        let w = display_width(&truncated);
        (Cow::Owned(truncated), w)
    } else {
        (Cow::Borrowed(primary), display_width(primary))
    };

    let mut spans: Vec<Span<'a>> = Vec::with_capacity(3);
    match primary_text {
        Cow::Borrowed(b) => spans.push(Span::styled(b, primary_style)),
        Cow::Owned(o) => spans.push(Span::styled(o, primary_style)),
    }

    let mut used = primary_w;
    // Only append the auto-name suffix when the user explicitly set a
    // `name` AND we have a distinct auto-name. The separator eats 3 cols so
    // we need at least that much room plus a few chars of the suffix to
    // be worth printing.
    if let (Some(name), Some(auto)) = (s.name.as_deref(), s.auto_name.as_deref()) {
        if !name.is_empty() && !auto.is_empty() && name != auto && used + 4 < col_width {
            let budget = col_width.saturating_sub(used + 3);
            if budget >= 3 {
                let suffix_text = truncate_to_width(auto, budget);
                let suffix_w = display_width(&suffix_text);
                let suffix_style_base = Style::default().fg(theme.subtext1);
                let suffix_style = maybe_fade(suffix_style_base, theme, age, apply_fade);
                spans.push(Span::styled(" · ".to_string(), theme.dim()));
                spans.push(Span::styled(suffix_text, suffix_style));
                used += 3 + suffix_w;
            }
        }
    }

    (spans, used)
}

/// Permission-mode badge — reverse-video `▌LABEL▐` in the mode's accent bg
/// with pill-text fg. Returns `None` for `PermissionMode::Default`.
///
/// The rails share the same bg as the interior so the badge reads as a solid
/// slug, matching the brief's "reverse-video danger badge" language. Using
/// [`theme::pill_text_color`] keeps the label legible on every palette
/// (dark themes use `crust`; light themes keep the same darkest shade).
fn permission_badge<'a>(
    mode: crate::data::PermissionMode,
    theme: &Theme,
    apply_fade: bool,
    age: Duration,
) -> Option<Span<'a>> {
    use crate::data::PermissionMode;
    let label = mode.pill_label()?;
    let bg = match mode {
        PermissionMode::Plan => theme.sky,
        PermissionMode::BypassPermissions => theme.red,
        PermissionMode::AcceptEdits => theme.yellow,
        PermissionMode::DontAsk => theme.pink,
        PermissionMode::Auto => theme.lavender,
        PermissionMode::Other(_) => theme.mauve,
        PermissionMode::Default => return None,
    };
    let fg = theme::pill_text_color(theme);
    let mut style = Style::default()
        .bg(bg)
        .fg(fg)
        .add_modifier(Modifier::BOLD);
    if apply_fade {
        if let Some(c) = style.fg {
            style.fg = Some(theme::age_fade(theme, c, age));
        }
        if let Some(c) = style.bg {
            style.bg = Some(theme::age_fade(theme, c, age));
        }
    }
    Some(Span::styled(format!("\u{258C}{label}\u{2590}"), style))
}

/// Render the subagent count as a circled glyph (U+2460..U+2468 = 1..9).
/// Counts of 10+ collapse to `⑨+` so the glyph stays a single pre-attentive
/// tag. Chosen over the old `◈ N` pair because the circled digit reads as
/// one self-contained signal.
fn circled_digit(n: u32) -> String {
    if n == 0 {
        return String::new();
    }
    if n >= 10 {
        return "\u{2468}+".to_string();
    }
    let codepoint = 0x2460u32 + (n - 1);
    char::from_u32(codepoint).unwrap_or('?').to_string()
}

/// Cost-severity foreground colour for a running session total.
///
/// Uses the theme's dedicated `cost_*` tokens so every screen agrees about
/// "cheap / medium / hot" spend. Thresholds mirror the stats widget so the
/// session list and the stats page never contradict each other. Zero-cost
/// rows fall back to `subtext0` so an unpriced row still occupies the column
/// visually but reads as "not applicable" rather than "cheap".
fn cost_severity_fg(cost_usd: f64, theme: &Theme) -> ratatui::style::Color {
    if cost_usd <= 0.0 {
        theme.subtext0
    } else if cost_usd < 1.0 {
        theme.cost_green
    } else if cost_usd < 10.0 {
        theme.cost_yellow
    } else if cost_usd < 50.0 {
        theme.cost_amber
    } else if cost_usd < 100.0 {
        theme.cost_red
    } else {
        theme.cost_critical
    }
}

/// Cost chip: `▌$12.40▐` capsule in cost-severity colour over `surface0`.
///
/// Zero-cost rows render the full chip with an em-dash placeholder so the
/// column stays visually anchored — the eye tracks the cost column down the
/// list and never sees a hole where the chip would be.
fn cost_chip_span<'a>(
    cost_usd: f64,
    theme: &Theme,
    selected: bool,
    apply_fade: bool,
    age: Duration,
) -> Span<'a> {
    let label = if cost_usd <= 0.0 {
        " — ".to_string()
    } else if cost_usd < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${cost_usd:.2}")
    };
    let fg = cost_severity_fg(cost_usd, theme);
    let mut style = Style::default()
        .fg(fg)
        .bg(theme.surface0)
        .add_modifier(Modifier::BOLD);
    let _ = selected; // selected row stripe is applied later by the outer pass
    if apply_fade {
        if let Some(c) = style.fg {
            style.fg = Some(theme::age_fade(theme, c, age));
        }
    }
    Span::styled(format!("\u{258C}{label}\u{2590}"), style)
}

/// Italic last-prompt teaser wedged between the pill pack and the cost chip
/// on wide panes. Returns `None` on narrow panes, empty prompts, or when the
/// teaser would simply echo the primary label.
///
/// `col_width` is the budget of the teaser column (from
/// `Layout::horizontal`), not the whole pane — the caller has already
/// reserved cells for the cost / age / gutter. The quotation marks and a
/// leading space eat 3 cols of that budget.
fn build_teaser_span<'a>(
    s: &'a Session,
    theme: &Theme,
    selected: bool,
    apply_fade: bool,
    age: Duration,
    col_width: u16,
) -> Option<Span<'a>> {
    let budget = col_width as usize;
    if budget < 10 {
        return None;
    }
    let prompt = s.last_prompt.as_deref()?;
    // Skip when the teaser would echo the primary label (no user-set name
    // and last-prompt is already the display label).
    if s.name.is_none() && prompt == s.display_label() {
        return None;
    }
    // Leave room for leading space + two quote glyphs.
    let reserved_glyphs: usize = 3;
    let available = budget.saturating_sub(reserved_glyphs);
    if available < 4 {
        return None;
    }
    let flat: String = prompt
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let trimmed = flat.trim();
    if trimmed.is_empty() {
        return None;
    }
    let body = truncate_to_width(trimmed, available);
    let quoted = format!("\u{201C}{body}\u{201D}");
    let mut style = Style::default()
        .fg(theme.subtext1)
        .add_modifier(Modifier::ITALIC);
    if selected {
        style = style.bg(theme.surface0);
    } else if apply_fade {
        if let Some(c) = style.fg {
            style.fg = Some(theme::age_fade(theme, c, age));
        }
    }
    Some(Span::styled(quoted, style))
}

/// Colour for the cost-burn 1-cell bar (E6 fallback variant).
///
/// Buckets per the brief: ≤$1 → green (cool), $1–$10 → amber/yellow, $10+
/// → rose/red. A separate ramp from [`theme::cost_color`] on purpose — this
/// bar is a binary "pay attention" signal, not a fine-grained heat map, so
/// we collapse to three tiers for instant legibility. Zero-cost rows render
/// against the muted overlay so an empty session doesn't light up green.
///
/// Retained after the column-layout refactor (v0.5.1) even though the live
/// row renderer no longer paints a dedicated burn-bar cell — the cost chip
/// fg already carries severity. Downstream widgets / tests still reach for
/// this helper via `cost_burn_color`, so we keep it.
#[allow(dead_code)]
fn cost_burn_color(cost_usd: f64, theme: &Theme) -> ratatui::style::Color {
    if cost_usd <= 0.0 {
        theme.overlay0
    } else if cost_usd < 1.0 {
        theme.green
    } else if cost_usd < 10.0 {
        theme.yellow
    } else {
        theme.red
    }
}

/// Colour for the context-window gutter (E7).
///
/// `ctx_pct = session_tokens.total() / 200_000`. We use `TokenCounts::total()`
/// because the per-message `input_tokens` counter isn't preserved on the
/// aggregated `Session` — `total()` is the closest proxy and it's what the
/// project-totals bar already displays, so the thresholds read consistently
/// across the UI.
fn ctx_gutter_color(s: &Session, theme: &Theme) -> ratatui::style::Color {
    let pct = (s.tokens.total() as f64) / 200_000.0;
    if pct < 0.40 {
        theme.green
    } else if pct < 0.80 {
        theme.yellow
    } else {
        theme.red
    }
}

/// Age of the session since `last_timestamp`. Missing timestamps return a
/// very large duration so the fade pins to the oldest bucket.
fn session_age(s: &Session) -> Duration {
    match s.last_timestamp {
        Some(ts) => Utc::now()
            .signed_duration_since(ts)
            .to_std()
            .unwrap_or_default(),
        None => Duration::from_secs(60 * 24 * 3_600),
    }
}

/// Fade `style` through [`theme::age_fade_style`] when the row is eligible.
/// Callers stamp out the guarded path without cluttering the main block.
fn maybe_fade(style: Style, theme: &Theme, age: Duration, apply: bool) -> Style {
    if !apply {
        return style;
    }
    theme::age_fade_style(theme, style, age)
}

/// Truncate `s` to at most `max_cols` *display columns* (not chars, not
/// bytes), appending `…` if cut.
///
/// Retained for callers outside this module (e.g. `project_list`). New code
/// should prefer [`crate::ui::text::truncate_to_width`] directly; this wrapper
/// keeps the `Cow` signature so the existing borrow semantics stay.
pub fn truncate_with_ellipsis(s: &str, max_cols: usize) -> Cow<'_, str> {
    if display_width(s) <= max_cols {
        return Cow::Borrowed(s);
    }
    Cow::Owned(truncate_to_width(s, max_cols))
}

/// Format a USD cost the way the Python picker does:
/// <$0.01 → dim, <$1 → two-decimal, ≥$1 → two-decimal with prefix.
///
/// Retained for the existing unit tests; the live row renderer now formats
/// cost inline inside [`cost_chip_span`] so this helper is no longer on the
/// hot path.
#[allow(dead_code)]
fn format_cost(cost: f64) -> String {
    if cost <= 0.0 {
        return String::new();
    }
    if cost < 0.01 {
        return "<$0.01".to_string();
    }
    format!("${cost:.2}")
}

/// Heat-mapped coloring for the cost column.
///
/// Superseded by [`cost_chip_span`] which renders the cost as a capsule
/// rather than a flat number. Retained at `#[allow(dead_code)]` in case
/// external modules want the old flat-number treatment.
#[allow(dead_code)]
fn cost_style(cost: f64, theme: &Theme, selected: bool) -> Style {
    let fg = if cost <= 0.0 {
        theme.subtext0
    } else {
        theme::cost_color(theme, cost)
    };
    let mut s = Style::default().fg(fg);
    if selected {
        s = s.bg(theme.surface0);
    }
    s
}

// ── F3: Pulse HUD ────────────────────────────────────────────────────────
//
// Bottom-right 3-line HUD overlay. Computes three at-a-glance figures from
// the sessions already loaded on `App`:
//
//   today  $4.21 ▂▁▃▅▇█▅▁ 9h
//   rate   $0.48/h      ●
//   proj   $8.40 by 6pm
//
// - today  — sum of `total_cost_usd` for every session whose
//   `last_timestamp` falls inside today (local TZ).
// - sparkline — today's cost bucketed into 8 columns, rendered as unicode
//   sparkline glyphs (U+2581..U+2588).
// - rate / proj — `today / hours_elapsed` and `rate * 24`, respectively;
//   no history store required.
//
// The `●` indicator on row 2 pulses via tachyonfx on a 2 s loop. When the
// user has `config.ui.reduce_motion = true`, the glyph is drawn as a solid
// `●` without animation.
//
// When `today_cost > 95 %` of the 10 USD daily soft-budget heuristic, the
// HUD's border flashes once via `parallel(fade_from, translate_in)` for
// 600 ms. The budget threshold is a static heuristic rather than a user
// preference because the brief's copy ("95 % of daily budget") doesn't
// specify a source — integrators with a budget store can swap in their own
// ceiling via `budget_ceiling_usd`.

/// The heuristic daily budget used to gate the flash-border warning.
/// Can be replaced by integrators by routing their own value through
/// `PulseHudState::new_with_budget`.
const DEFAULT_DAILY_BUDGET_USD: f64 = 10.0;

/// Loop length of the live-dot pulse. 2 000 ms matches the brief.
const PULSE_LOOP_MS: u32 = 2_000;
/// How long the over-budget border flash runs. 600 ms per the brief.
const FLASH_MS: u32 = 600;

/// Transient animation state for the pulse HUD. Wrapped in a `thread_local`
/// `RefCell` — the render path is called with `&App`, and adding per-frame
/// mutable animation state to `App` is outside the file-ownership of this
/// patch (see the integration spec at the bottom of the module). The
/// thread-local is safe because ratatui renders single-threaded and the
/// state is strictly per-process.
pub struct PulseHudState {
    /// Long-running pulse of the live-dot glyph. Set at first render,
    /// never replaced.
    pulse: Option<Effect>,
    /// One-shot border-flash effect; re-seeded whenever `today_cost`
    /// crosses the 95%-of-budget threshold.
    flash: Option<Effect>,
    /// Last tick instant — drives the elapsed-time delta tachyonfx wants.
    last_tick: Instant,
    /// `Some(true)` once we've flashed for the current over-budget
    /// episode. Cleared when today's cost drops back under the threshold
    /// so the next breach re-fires the flash.
    flashed_at: Option<f64>,
    /// Cached `reduce_motion` flag — set once per process so we don't
    /// allocate an effect that tachyonfx would run but never display.
    reduce_motion: bool,
}

impl PulseHudState {
    fn new(reduce_motion: bool, low: ratatui::style::Color, high: ratatui::style::Color,
           bg: ratatui::style::Color) -> Self {
        let pulse = if reduce_motion {
            None
        } else {
            Some(ui_fx::pulse(low, high, bg, PULSE_LOOP_MS))
        };
        Self {
            pulse,
            flash: None,
            last_tick: Instant::now(),
            flashed_at: None,
            reduce_motion,
        }
    }

    /// Trigger the border-flash for this frame if today's cost just crossed
    /// 95 % of `budget_ceiling`. Idempotent — a flash already armed for the
    /// same crossing is not re-armed.
    fn maybe_arm_flash(
        &mut self,
        today_cost: f64,
        budget_ceiling: f64,
        accent: ratatui::style::Color,
        bg: ratatui::style::Color,
    ) {
        if self.reduce_motion || budget_ceiling <= 0.0 {
            return;
        }
        let ratio = today_cost / budget_ceiling;
        if ratio >= 0.95 {
            // Only arm when we weren't already flagged for this episode.
            if self.flashed_at.is_none() {
                self.flash = Some(ui_fx::flash_border(accent, bg, FLASH_MS));
                self.flashed_at = Some(ratio);
            }
        } else {
            // Dropped back below threshold — clear the latch so a
            // subsequent breach fires again.
            self.flashed_at = None;
            self.flash = None;
        }
    }
}

thread_local! {
    /// Per-thread holder for the live pulse HUD state. Populated lazily on
    /// the first render so the theme/motion flag are captured from `App`.
    /// The cell is `RefCell` because render call-sites only have `&App`.
    static PULSE_HUD: RefCell<Option<PulseHudState>> = const { RefCell::new(None) };
}

/// Daily-cost snapshot computed from the loaded session list. Kept out of
/// the render function so the tests can exercise the bucket logic without
/// a frame buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudStats {
    /// Sum of costs for sessions whose `last_timestamp` falls in today.
    pub today_cost_usd: f64,
    /// `today_cost / hours_elapsed_today` — undefined while hours ≤ 0.
    pub rate_usd_per_hour: f64,
    /// Projected end-of-day cost at the current rate.
    pub projected_cost_usd: f64,
    /// Hours elapsed since local-midnight.
    pub hours_elapsed: f64,
    /// 8-bucket sparkline of today's per-hour spend (00:00..now). Fixed
    /// length 8 so the HUD row layout is stable.
    pub spark_buckets: [f64; 8],
    /// Number of sessions that landed in "today" — shown as the trailing
    /// `9h` hint on row 1 (hours contributed).
    pub today_session_count: usize,
}

impl HudStats {
    /// Compute the HUD stats from the currently-loaded sessions. `now` is
    /// accepted as a parameter so tests can pin a deterministic wall
    /// clock.
    pub fn compute<'a, I: IntoIterator<Item = &'a Session>>(
        sessions: I,
        now: DateTime<Utc>,
    ) -> Self {
        // Resolve today's local-midnight in UTC. The chain is:
        //   1. Convert `now` (UTC) to the user's local time zone.
        //   2. Grab that local date and rebuild a `NaiveDateTime` at 00:00.
        //   3. Bind that naive value back to `Local` and resolve to UTC for
        //      comparison against the UTC `last_timestamp` field.
        //
        // On DST-transition days `from_local_datetime` can return `None` or
        // `Ambiguous`; we fall back to `now` as the lower bound in those
        // pathological cases so the HUD never panics — the worst-case cost
        // is a temporarily-empty "today" bucket for half an hour.
        let now_local = now.with_timezone(&Local);
        let naive_midnight = now_local
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("midnight is a valid wall-clock time");
        let today_start_local = Local
            .from_local_datetime(&naive_midnight)
            .single()
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);

        let mut today_cost = 0.0f64;
        let mut spark = [0.0f64; 8];
        let mut today_count = 0usize;

        for s in sessions {
            let Some(ts) = s.last_timestamp else { continue };
            if ts < today_start_local {
                continue;
            }
            today_cost += s.total_cost_usd;
            today_count += 1;
            // Bucket across 8 equal slices of the day so the sparkline is
            // always the same width regardless of the user's local hour.
            let seconds_since_midnight =
                (ts - today_start_local).num_seconds().max(0) as f64;
            let bucket = ((seconds_since_midnight / 86_400.0) * 8.0)
                .clamp(0.0, 7.9999) as usize;
            spark[bucket] += s.total_cost_usd;
        }

        let hours_elapsed = ((now - today_start_local).num_seconds().max(0) as f64) / 3_600.0;
        let rate = if hours_elapsed > 0.1 {
            today_cost / hours_elapsed
        } else {
            0.0
        };
        let projected = rate * 24.0;

        Self {
            today_cost_usd: today_cost,
            rate_usd_per_hour: rate,
            projected_cost_usd: projected,
            hours_elapsed,
            spark_buckets: spark,
            today_session_count: today_count,
        }
    }

    /// Render the sparkline string from the buckets. Picks a glyph from
    /// `▁▂▃▄▅▆▇█` proportional to the bucket's share of the max.
    pub fn sparkline(&self) -> String {
        const GLYPHS: [char; 8] =
            ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];
        let max = self
            .spark_buckets
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max);
        if max <= 0.0 {
            // Use the lowest glyph for every column so the HUD still shows
            // a sparkline placeholder rather than blank cells.
            return std::iter::repeat_n(GLYPHS[0], 8).collect();
        }
        self.spark_buckets
            .iter()
            .map(|v| {
                let idx = ((v / max) * 7.0).round().clamp(0.0, 7.0) as usize;
                GLYPHS[idx]
            })
            .collect()
    }
}

fn format_budget_cost(cost: f64) -> String {
    if cost <= 0.0 {
        "$0.00".to_string()
    } else if cost < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${cost:.2}")
    }
}

fn format_projection_target() -> String {
    // End-of-day label — "by 11:59pm" is wordy, so the brief uses the
    // informal "6pm" / "11pm" feel. We compute the current hour-of-day
    // as a marker; the projection itself is always end-of-day, so this
    // is purely a subtitle hint.
    let now = Local::now();
    let hour_12 = now.format("%-I%p").to_string().to_lowercase();
    format!("by {hour_12}")
}

/// Reduce-motion resolution without reaching into `App` beyond its public
/// surface. We key off the existing env-driven `theme::animations_disabled`
/// until `App` exposes the config's `reduce_motion` — the file-ownership of
/// this patch forbids editing `src/app.rs`, so this reads as "env or
/// theme-level opt-out counts as reduce-motion for the HUD as well". See
/// the integration spec.
fn hud_reduce_motion() -> bool {
    theme::animations_disabled()
}

fn render_pulse_hud(f: &mut Frame<'_>, area: Rect, app: &App) {
    // Need at least 3 rows + a bit of slack so the overlay doesn't
    // swallow the list entirely on dense panes.
    if area.height < 8 {
        return;
    }
    // HUD width — 26 cols fits the three rows without feeling chipmunk'd.
    let hud_w: u16 = 26;
    if area.width < hud_w + 4 {
        return;
    }

    let stats = HudStats::compute(app.sessions.iter(), Utc::now());
    let theme = &app.theme;

    let rect = Rect {
        x: area.x + area.width.saturating_sub(hud_w + 1),
        y: area.y + area.height.saturating_sub(3),
        width: hud_w,
        height: 3,
    };

    // Lazily build the thread-local pulse state on first render so the
    // theme colours are captured correctly (they can change if the user
    // hits `t` to cycle themes).
    PULSE_HUD.with(|cell| {
        let mut guard = cell.borrow_mut();
        if guard.is_none() {
            *guard = Some(PulseHudState::new(
                hud_reduce_motion(),
                theme.surface2,
                theme.green,
                theme.base,
            ));
        }
        let state = guard.as_mut().unwrap();
        // Over-budget warning gate — arms the flash exactly once per
        // crossing episode.
        state.maybe_arm_flash(
            stats.today_cost_usd,
            DEFAULT_DAILY_BUDGET_USD,
            theme.red,
            theme.base,
        );

        // Paint the HUD statically first.
        paint_hud(f, rect, &stats, theme, state.reduce_motion);

        // Then overlay the live effects via tachyonfx.
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_tick);
        state.last_tick = now;
        let delta = ui_fx::delta_from(elapsed);

        // Per-frame pulse on the live-dot cell. The effect covers just
        // the single cell where the `●` is drawn so other paint doesn't
        // get dragged through the alpha lerp. Each `process` call takes
        // its own `buffer_mut` so the &mut Buffer lifetime is scoped to
        // the one call — avoids repeated-reborrow ambiguity.
        if let Some(pulse) = state.pulse.as_mut() {
            let dot_rect = Rect {
                x: rect.x + hud_w.saturating_sub(3),
                y: rect.y + 1,
                width: 1,
                height: 1,
            };
            let buf: &mut Buffer = f.buffer_mut();
            pulse.process(delta, buf, dot_rect);
        }

        // Border flash — one-shot, dropped when complete.
        if let Some(flash) = state.flash.as_mut() {
            let buf: &mut Buffer = f.buffer_mut();
            flash.process(delta, buf, rect);
            if flash.done() {
                state.flash = None;
            }
        }
    });
}

/// Paint the static portion of the HUD. The pulse + flash effects run
/// over the top of this layer in `render_pulse_hud`.
fn paint_hud(
    f: &mut Frame<'_>,
    rect: Rect,
    stats: &HudStats,
    theme: &Theme,
    reduce_motion: bool,
) {
    // Clear the cells so the list row underneath doesn't bleed through.
    f.render_widget(ratatui::widgets::Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.surface2));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    // Row 1: `today  $X.XX  sparkline  Nh`
    let spark = stats.sparkline();
    let hours_label = if stats.today_session_count == 0 {
        "0h".to_string()
    } else {
        format!("{}h", stats.hours_elapsed.round() as i64)
    };
    let row_today = Line::from(vec![
        Span::styled("today  ", theme.muted()),
        Span::styled(
            format_budget_cost(stats.today_cost_usd),
            Style::default()
                .fg(if stats.today_cost_usd > DEFAULT_DAILY_BUDGET_USD * 0.95 {
                    theme.red
                } else if stats.today_cost_usd > DEFAULT_DAILY_BUDGET_USD * 0.5 {
                    theme.yellow
                } else {
                    theme.green
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(spark, Style::default().fg(theme.teal)),
        Span::raw(" "),
        Span::styled(hours_label, theme.muted()),
    ]);

    // Row 2: `rate   $X.XX/h      ●`
    // The live-dot is drawn here as solid; tachyonfx then modulates its
    // alpha unless reduce-motion is set — in that case the solid glyph
    // is the final look.
    // Solid green glyph in both modes — tachyonfx modulates alpha at render
    // time when reduce_motion is false. The if/else is collapsed because the
    // effect path doesn't change the base style, only the per-frame alpha.
    let _ = reduce_motion;
    let dot_style = Style::default()
        .fg(theme.green)
        .add_modifier(Modifier::BOLD);
    let rate_label = if stats.rate_usd_per_hour > 0.0 {
        format!("${:.2}/h", stats.rate_usd_per_hour)
    } else {
        "$0.00/h".to_string()
    };
    // Pad the middle so the `●` hits col `hud_w - 3`.
    let inner_w = inner.width as usize;
    let left = format!("rate   {rate_label}");
    let left_w = display_width(&left);
    let pad_w = inner_w.saturating_sub(left_w + 1);
    let row_rate = Line::from(vec![
        Span::styled(left, theme.muted()),
        Span::raw(" ".repeat(pad_w)),
        Span::styled("\u{25CF}", dot_style),
    ]);

    // Row 3: `proj   $X.XX by 6pm`
    let row_proj = Line::from(vec![
        Span::styled("proj   ", theme.muted()),
        Span::styled(
            format_budget_cost(stats.projected_cost_usd),
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(format_projection_target(), theme.muted()),
    ]);

    let p = Paragraph::new(vec![row_today, row_rate, row_proj]);
    f.render_widget(p, inner);
}

/// Relative-time like "2h", "yd" (yesterday), "3d", or "Apr 10".
fn relative_time(ts: Option<DateTime<Utc>>) -> String {
    let Some(ts) = ts else {
        return "—".to_string();
    };
    let now = Utc::now();
    let diff = now.signed_duration_since(ts);
    if diff.num_seconds() < 60 {
        "now".to_string()
    } else if diff.num_minutes() < 60 {
        format!("{}m", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h", diff.num_hours())
    } else if diff.num_days() == 1 {
        "yd".to_string()
    } else if diff.num_days() < 7 {
        format!("{}d", diff.num_days())
    } else if diff.num_days() < 30 {
        format!("{}w", diff.num_days() / 7)
    } else {
        ts.format("%b %d").to_string()
    }
}

/// Tint the age column by recency — old sessions slide toward warning colors.
fn age_style(ts: Option<DateTime<Utc>>, theme: &Theme) -> Style {
    let Some(ts) = ts else {
        return Style::default().fg(theme.overlay0);
    };
    let days = Utc::now().signed_duration_since(ts).num_days();
    let fg = if days > 30 {
        theme.red
    } else if days > 7 {
        theme.peach
    } else {
        theme.overlay0
    };
    Style::default().fg(fg)
}

/// Render an empty-state paragraph centered inside `area`.
fn empty_state(f: &mut Frame<'_>, area: Rect, theme: &Theme, lines: Vec<Line<'_>>) {
    let text_height = lines.len() as u16;
    // Center vertically by injecting blank lines above.
    let padding = area.height.saturating_sub(text_height) / 2;
    let mut padded = Vec::with_capacity(lines.len() + padding as usize);
    for _ in 0..padding {
        padded.push(Line::raw(""));
    }
    padded.extend(lines);

    let p = Paragraph::new(padded)
        .style(theme.muted())
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(p, area);
}

fn empty_copy_no_sessions<'a>(theme: &Theme) -> Vec<Line<'a>> {
    let glyph_style = Style::default()
        .fg(theme.surface2)
        .add_modifier(Modifier::BOLD);
    let primary = Style::default()
        .fg(theme.subtext1)
        .add_modifier(Modifier::BOLD);
    let secondary = Style::default()
        .fg(theme.overlay0)
        .add_modifier(Modifier::ITALIC);
    vec![
        Line::raw(""),
        Line::from(Span::styled("\u{25EF}", glyph_style)),
        Line::raw(""),
        Line::from(Span::styled("no sessions yet", primary)),
        Line::raw(""),
        Line::from(Span::styled(
            "start a claude session to see it here",
            secondary,
        )),
    ]
}

fn empty_copy_no_matches<'a>(filter: &str, theme: &Theme) -> Vec<Line<'a>> {
    let glyph_style = Style::default()
        .fg(theme.surface2)
        .add_modifier(Modifier::BOLD);
    let primary = Style::default()
        .fg(theme.subtext1)
        .add_modifier(Modifier::BOLD);
    let secondary = Style::default()
        .fg(theme.overlay0)
        .add_modifier(Modifier::ITALIC);
    vec![
        Line::raw(""),
        Line::from(Span::styled("\u{25EF}", glyph_style)),
        Line::raw(""),
        Line::from(Span::styled(
            format!("no matches for \u{201C}{filter}\u{201D}"),
            primary,
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "press Esc to clear the filter",
            secondary,
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_strings_unchanged() {
        assert_eq!(truncate_with_ellipsis("short", 10), "short");
    }

    #[test]
    fn truncate_adds_ellipsis() {
        let out = truncate_with_ellipsis("abcdefghij", 5);
        assert_eq!(out, "abcd…");
    }

    #[test]
    fn truncate_is_display_width_aware() {
        // 10 cols of CJK → truncate to 5 cols → 2 chars + ellipsis = 5 cols.
        let out = truncate_with_ellipsis("こんにちは", 5);
        assert!(
            display_width(&out) <= 5,
            "truncated width {}: {}",
            display_width(&out),
            out
        );
    }

    #[test]
    fn cost_formatting_matches_spec() {
        assert_eq!(format_cost(0.0), "");
        assert_eq!(format_cost(0.003), "<$0.01");
        assert_eq!(format_cost(0.41), "$0.41");
        assert_eq!(format_cost(2.07), "$2.07");
    }

    #[test]
    fn relative_time_none_yields_dash() {
        assert_eq!(relative_time(None), "—");
    }

    #[test]
    fn relative_time_buckets() {
        let now = Utc::now();
        let two_hours = now - chrono::Duration::hours(2);
        assert_eq!(relative_time(Some(two_hours)), "2h");
        let yesterday = now - chrono::Duration::hours(26);
        assert_eq!(relative_time(Some(yesterday)), "yd");
    }

    fn mk_session_at(id: &str, cost: f64, ts: DateTime<Utc>) -> Session {
        use crate::data::pricing::TokenCounts;
        use crate::data::SessionKind;
        use std::path::PathBuf;
        Session {
            id: id.into(),
            project_dir: PathBuf::from("/tmp"),
            name: None,
            auto_name: None,
            last_prompt: None,
            message_count: 1,
            tokens: TokenCounts::default(),
            total_cost_usd: cost,
            model_summary: String::new(),
            first_timestamp: Some(ts),
            last_timestamp: Some(ts),
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
            turn_durations: Vec::new(),
        }
    }

    #[test]
    fn hud_stats_empty_sessions_yields_zeroes() {
        let stats = HudStats::compute(std::iter::empty::<&Session>(), Utc::now());
        assert_eq!(stats.today_cost_usd, 0.0);
        assert_eq!(stats.today_session_count, 0);
    }

    #[test]
    fn hud_stats_excludes_yesterday_sessions() {
        let now = Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();
        let yesterday = now - chrono::Duration::hours(30);
        let today = now - chrono::Duration::hours(2);
        let sessions = [
            mk_session_at("old", 5.0, yesterday),
            mk_session_at("new", 1.5, today),
        ];
        let stats = HudStats::compute(sessions.iter(), now);
        assert!((stats.today_cost_usd - 1.5).abs() < 1e-9,
            "yesterday's $5 must not count, only today's $1.50");
        assert_eq!(stats.today_session_count, 1);
    }

    #[test]
    fn hud_stats_sparkline_has_8_glyphs() {
        let now = Utc.with_ymd_and_hms(2026, 4, 17, 20, 0, 0).unwrap();
        let sessions = [mk_session_at("a", 1.0, now - chrono::Duration::hours(1))];
        let stats = HudStats::compute(sessions.iter(), now);
        let spark = stats.sparkline();
        assert_eq!(spark.chars().count(), 8, "sparkline must always be 8 glyphs");
    }

    #[test]
    fn hud_stats_projection_equals_rate_times_24() {
        // Test the invariant (projection == rate * 24) regardless of the
        // runner's timezone — `compute()` uses `Local` to bucket sessions
        // into "today," so absolute rate values depend on the host TZ.
        let now = Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();
        let sessions = [mk_session_at("a", 6.0, now - chrono::Duration::hours(1))];
        let stats = HudStats::compute(sessions.iter(), now);
        if stats.rate_usd_per_hour > 0.0 {
            let ratio = stats.projected_cost_usd / stats.rate_usd_per_hour;
            assert!(
                (ratio - 24.0).abs() < 1e-6,
                "projection must equal rate * 24, got ratio {ratio}"
            );
        }
    }

    #[test]
    fn hud_stats_empty_sparkline_still_renders_placeholder() {
        let now = Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();
        let stats = HudStats::compute(std::iter::empty::<&Session>(), now);
        let spark = stats.sparkline();
        assert_eq!(spark.chars().count(), 8);
        // With no data, every bucket should be the lowest glyph.
        let lowest = '\u{2581}';
        for g in spark.chars() {
            assert_eq!(g, lowest);
        }
    }

    #[test]
    fn cost_burn_buckets_ramp_green_to_red() {
        let t = Theme::mocha();
        assert_eq!(cost_burn_color(0.0, &t), t.overlay0);
        assert_eq!(cost_burn_color(0.01, &t), t.green);
        assert_eq!(cost_burn_color(0.99, &t), t.green);
        assert_eq!(cost_burn_color(1.0, &t), t.yellow);
        assert_eq!(cost_burn_color(9.99, &t), t.yellow);
        assert_eq!(cost_burn_color(10.0, &t), t.red);
        assert_eq!(cost_burn_color(99.0, &t), t.red);
    }

    #[test]
    fn circled_digit_maps_1_through_9() {
        assert_eq!(circled_digit(1), "\u{2460}"); // ①
        assert_eq!(circled_digit(5), "\u{2464}"); // ⑤
        assert_eq!(circled_digit(9), "\u{2468}"); // ⑨
    }

    #[test]
    fn circled_digit_clips_double_digits() {
        assert_eq!(circled_digit(10), "\u{2468}+");
        assert_eq!(circled_digit(99), "\u{2468}+");
    }

    #[test]
    fn circled_digit_zero_is_empty() {
        assert_eq!(circled_digit(0), "");
    }

    #[test]
    fn cost_severity_uses_cost_tokens_not_generic_accents() {
        // The severity helper MUST read from the theme's dedicated cost_*
        // tokens so theme overrides (CB-safe, cold palette) land correctly.
        let t = Theme::mocha();
        assert_eq!(cost_severity_fg(0.0, &t), t.subtext0);
        assert_eq!(cost_severity_fg(0.5, &t), t.cost_green);
        assert_eq!(cost_severity_fg(5.0, &t), t.cost_yellow);
        assert_eq!(cost_severity_fg(20.0, &t), t.cost_amber);
        assert_eq!(cost_severity_fg(75.0, &t), t.cost_red);
        assert_eq!(cost_severity_fg(150.0, &t), t.cost_critical);
    }

    #[test]
    fn cost_chip_always_wraps_in_half_blocks() {
        // The cost chip MUST keep the `▌…▐` rail shape even for zero-cost
        // rows so the column stays visually anchored across the list.
        let t = Theme::mocha();
        let z = cost_chip_span(0.0, &t, false, false, Duration::ZERO);
        assert!(z.content.starts_with('\u{258C}'));
        assert!(z.content.ends_with('\u{2590}'));
        let hot = cost_chip_span(123.45, &t, false, false, Duration::ZERO);
        assert!(hot.content.contains("$123.45"));
        assert_eq!(hot.style.fg, Some(t.cost_critical));
    }

    #[test]
    fn ctx_gutter_thresholds_match_200k_budget() {
        // Build a bare Session stub — we only need `.tokens.total()` to vary.
        use crate::data::pricing::TokenCounts;
        use crate::data::SessionKind;
        use std::path::PathBuf;

        let mk = |total: u64| Session {
            id: "x".into(),
            project_dir: PathBuf::from("/tmp"),
            name: None,
            auto_name: None,
            last_prompt: None,
            message_count: 0,
            tokens: TokenCounts {
                input: total,
                ..TokenCounts::default()
            },
            total_cost_usd: 0.0,
            model_summary: String::new(),
            first_timestamp: None,
            last_timestamp: None,
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
            turn_durations: Vec::new(),
        };
        let t = Theme::mocha();

        // < 40% → green.
        assert_eq!(ctx_gutter_color(&mk(0), &t), t.green);
        assert_eq!(ctx_gutter_color(&mk(79_999), &t), t.green);
        // 40–80% → yellow.
        assert_eq!(ctx_gutter_color(&mk(80_000), &t), t.yellow);
        assert_eq!(ctx_gutter_color(&mk(159_999), &t), t.yellow);
        // ≥ 80% → red.
        assert_eq!(ctx_gutter_color(&mk(160_000), &t), t.red);
        assert_eq!(ctx_gutter_color(&mk(500_000), &t), t.red);
    }

    #[test]
    fn column_plan_wide_pane_shows_everything() {
        let plan = ColumnPlan::for_width(120);
        assert!(plan.show_subtitle, "120 cols keeps the auto-name subtitle");
        assert!(plan.show_teaser, "120 cols renders the italic teaser");
        assert!(plan.show_perm);
        assert!(plan.show_model);
        assert!(plan.show_subagent);
        assert!(plan.show_cost);
        assert!(plan.show_age);
        assert!(plan.show_gutter);
    }

    #[test]
    fn column_plan_drops_subtitle_and_teaser_under_100() {
        let plan = ColumnPlan::for_width(99);
        assert!(!plan.show_subtitle, "99 cols drops the subtitle");
        assert!(!plan.show_teaser, "99 cols drops the teaser");
        assert!(plan.show_perm, "permission badge still visible at 99");
        assert!(plan.show_model);
    }

    #[test]
    fn column_plan_drops_perm_under_80() {
        let plan = ColumnPlan::for_width(79);
        assert!(!plan.show_perm, "79 cols drops the permission badge");
        assert!(plan.show_model, "model pill still visible at 79");
        assert!(plan.show_subagent);
    }

    #[test]
    fn column_plan_drops_model_and_subagent_under_60() {
        let plan = ColumnPlan::for_width(59);
        assert!(!plan.show_model, "59 cols drops the model pill");
        assert!(!plan.show_subagent, "59 cols drops the subagent counter");
        assert!(plan.show_cost, "cost chip still visible at 59");
        assert!(plan.show_age);
    }

    #[test]
    fn column_plan_keeps_name_cost_age_at_42() {
        let plan = ColumnPlan::for_width(42);
        assert!(plan.show_cost);
        assert!(plan.show_age);
        assert!(!plan.show_gutter, "gutter drops below the 43-col guard");
        assert!(!plan.show_model);
        assert!(!plan.show_subagent);
        assert!(!plan.show_perm);
    }

    #[test]
    fn column_plan_drops_cost_age_below_42() {
        // Below the 42-col guard the fixed-column sum would exceed the
        // pane width, so we drop cost + age together.
        let plan = ColumnPlan::for_width(41);
        assert!(!plan.show_cost);
        assert!(!plan.show_age);
        assert!(!plan.show_gutter);
    }

    #[test]
    fn column_plan_constraints_sum_never_exceeds_width_for_fixed_cols() {
        // At every breakpoint, the sum of fixed-length constraints must be
        // ≤ the pane width — otherwise `Layout::horizontal` clips our
        // explicit columns and the flex teaser column collapses.
        let fixed_total = |p: &ColumnPlan| -> u16 {
            let c = p.constraints();
            c.iter()
                .filter_map(|cn| match cn {
                    Constraint::Length(n) => Some(*n),
                    _ => None,
                })
                .sum()
        };
        // Pane widths below 26 cells fall under the always-on `prefix (2)
        // + name (24)` floor. Ratatui clips fixed columns gracefully when
        // the rect is smaller than the constraints' sum, but the
        // breakpoints above that floor are our contract.
        for w in [41u16, 42, 43, 59, 60, 79, 80, 99, 100, 120, 160] {
            let plan = ColumnPlan::for_width(w);
            let total = fixed_total(&plan);
            assert!(
                total <= w,
                "width={w} fixed-column total={total} exceeds pane width",
            );
        }
    }

    #[test]
    fn cost_chip_truncation_drops_cents_first() {
        // `▌$1234.56▐` is 10 cells; trim to the 9-cell budget by dropping
        // the fractional part so the reader still sees the full dollars.
        let trimmed = truncate_cost_chip("\u{258C}$1234.56\u{2590}", 9);
        assert_eq!(trimmed, "\u{258C}$1234\u{2590}");
        assert!(display_width(&trimmed) <= 9);
    }

    #[test]
    fn cost_chip_truncation_leaves_fits_untouched() {
        let src = "\u{258C}$12.40\u{2590}";
        assert_eq!(truncate_cost_chip(src, 9), src);
    }

    #[test]
    fn scroll_start_anchors_to_top_when_inside_first_page() {
        assert_eq!(scroll_start(0, 10, 50), 0);
        assert_eq!(scroll_start(5, 10, 50), 0);
        assert_eq!(scroll_start(9, 10, 50), 0);
        // Past the first page, anchor the bottom so the cursor stays in view.
        assert_eq!(scroll_start(10, 10, 50), 1);
        assert_eq!(scroll_start(49, 10, 50), 40);
        // Small lists — never scroll at all.
        assert_eq!(scroll_start(3, 10, 4), 0);
    }
}

// ─── F3 integration spec ─────────────────────────────────────────────────
//
// The pulse-HUD animation currently reads its reduce-motion flag via
// `theme::animations_disabled()` (the env-driven legacy toggle), because
// this patch's file-ownership forbids editing `src/app.rs`. To honour
// `config.ui.reduce_motion` at the config-file level, swap
// `hud_reduce_motion()` for `app.config.ui.reduce_motion`. The two
// required wiring changes are:
//
//   1. Thread the loaded `Config` onto `App`. Add
//      `pub config: crate::config::Config,` next to the existing `theme`
//      field on the `App` struct, and accept it as a parameter of
//      `App::new` / `App::new_with_theme`. Today the config is loaded in
//      `main.rs` and only its theme string makes it into `App`.
//
//   2. Replace the body of `hud_reduce_motion` (above) with
//      `pub fn hud_reduce_motion(app: &App) -> bool {
//         app.config.ui.reduce_motion || theme::animations_disabled()
//      }`
//      and update `render_pulse_hud` to pass `app` in. The OR keeps the
//      legacy env escape hatch working.
//
// Until the wiring lands, power users can still opt out of the F3
// animation by setting `CLAUDE_PICKER_NO_ANIM=1`. The HUD stats
// themselves render either way — only the pulse + border flash are
// motion-gated.

