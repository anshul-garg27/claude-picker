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
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;

use tachyonfx::{Effect, Shader};

use crate::app::App;
use crate::data::Session;
use crate::theme::{self, Theme};
use crate::ui::fx as ui_fx;
use crate::ui::model_pill;
use crate::ui::text::{display_width, pad_to_width, truncate_to_width};

/// Width of the name column within a row. The row renderer appends cost and
/// age after this, so keep the header/list aligned by anchoring off of it.
const NAME_COL_WIDTH: usize = 28;

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

/// Render the list of sessions. Builds `ListItem`s for the filtered slice
/// only; Ratatui's `List` handles scrolling based on the cursor index.
fn render_list(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    // Empty states — different copy depending on cause.
    if app.sessions.is_empty() {
        empty_state(f, area, theme, empty_copy_no_sessions());
        return;
    }
    if app.filtered_indices.is_empty() {
        empty_state(f, area, theme, empty_copy_no_matches(&app.filter));
        return;
    }

    // Gate the density decorations on available width. The gutter lives at
    // the right edge; on very narrow panes it would collide with the scrollbar
    // or force the name column to truncate below readability. Same story with
    // the cost-burn chip just before the cost column — drop it first when
    // width gets tight.
    let cols = area.width as usize;
    let show_gutter = cols >= 60;
    let show_cost_bar = cols >= 80;

    let items: Vec<ListItem<'_>> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &sess_idx)| {
            let s = &app.sessions[sess_idx];
            let is_selected = Some(display_idx) == app.cursor_position();
            let is_bookmarked = app.bookmarks.contains(&s.id);
            let is_multi = app.is_multi_selected(sess_idx);
            // Lingering cursor trail: the row we just left keeps a faint
            // `surface0` wash for the glide window so the eye catches the
            // direction of movement.
            let is_glide = app.is_glide_trail(display_idx);
            ListItem::new(render_row(
                s,
                theme,
                is_selected,
                is_bookmarked,
                is_multi,
                is_glide,
                cols,
                show_gutter,
                show_cost_bar,
            ))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default()) // we paint our own selection
        .highlight_symbol("");

    let mut state = ListState::default();
    state.select(app.cursor_position());
    f.render_stateful_widget(list, area, &mut state);

    // Scrollbar on the right edge. Skip entirely when everything fits — a
    // thumb that covers the whole track is noisy.
    let total = app.filtered_indices.len();
    if total > area.height as usize {
        render_scrollbar(f, area, total, app.cursor, theme);
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

/// Render a single row as a styled [`Line`].
///
/// Layout (at 55%-wide panels that gives us ~50 cols):
/// `▸ session-name…………… [opus] ▍ $1.24 2h           ▕`
///                                       ↑ cost-burn  ↑ ctx gutter
///
/// **v2.2 polish layers:**
/// - Cost column uses `theme::cost_color` (teal → green → yellow → peach).
/// - Unselected rows fade toward `overlay0` based on the session's last
///   activity — older rows visibly dim so recency reads without dates.
/// - Multi-selected / cursor rows keep full intensity for contrast.
///
/// **Density layers (E6/E7):**
/// - 1-cell cost-burn bar (green/amber/rose) sits between the pill group and
///   the cost number, giving a pre-attentive "how hot is this?" cue before
///   the reader parses the dollar figure.
/// - 1-cell context-usage gutter anchored 1 col inset from the right edge
///   shows how full the 200k token window is (green/amber/rose by 40%/80%
///   thresholds). Inset so the scrollbar thumb never paints over it.
#[allow(clippy::too_many_arguments)]
fn render_row<'a>(
    s: &'a Session,
    theme: &Theme,
    selected: bool,
    bookmarked: bool,
    multi: bool,
    glide_trail: bool,
    area_width: usize,
    show_gutter: bool,
    show_cost_bar: bool,
) -> Line<'a> {
    // Age in seconds since the last activity timestamp — drives the row-fade.
    // Missing timestamps fade fully (treat as "very old").
    let age = session_age(s);

    // Whether this row should run through the age-fade filter at all. The
    // brief says: fade ONLY unselected rows; selection stays full brightness
    // for contrast. Multi-select rows also stay full-bright.
    let apply_fade = !selected && !multi;

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
    let name_style = maybe_fade(name_style_base, theme, age, apply_fade);

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
    let pointer_style = maybe_fade(pointer_style_base, theme, age, apply_fade);
    // `✓` takes the pointer slot when the row is multi-selected (whether or
    // not the cursor is on it). The cursor row without multi-selection keeps
    // the `▸` pointer so the active row is still clear at a glance.
    let pointer = if multi {
        "✓"
    } else if selected {
        "▸"
    } else {
        " "
    };

    let pin = if bookmarked {
        Span::styled(
            "■ ",
            maybe_fade(Style::default().fg(theme.blue), theme, age, apply_fade),
        )
    } else if s.is_fork {
        Span::styled(
            "↳ ",
            maybe_fade(Style::default().fg(theme.peach), theme, age, apply_fade),
        )
    } else {
        Span::raw("  ")
    };

    // display_width-aware: CJK / emoji session names used to overflow the
    // column because .chars().count() undercounted them. Use the unicode
    // helpers so the name always occupies exactly NAME_COL_WIDTH terminal
    // cells, pad or truncate.
    let name = pad_to_width(s.display_label(), NAME_COL_WIDTH);
    let name_span = Span::styled(name, name_style);

    // Chip-style model pill. Fade fg only when unselected — we don't want a
    // year-old session's pill to be indistinguishable from the border.
    let mut pill = model_pill::pill(crate::data::pricing::family(&s.model_summary), theme);
    if apply_fade {
        if let Some(fg) = pill.style.fg {
            pill.style.fg = Some(theme::age_fade(theme, fg, age));
        }
    }

    // Optional permission-mode pill — only drawn for non-default modes.
    let perm_pill = s
        .permission_mode
        .and_then(|m| model_pill::permission_pill(m, theme));

    // Subagent marker — tiny "◈ N" glyph when the session spawned
    // sub-agents, otherwise nothing. Using ASCII to stay brand-aligned
    // (no emojis anywhere in the UI).
    let subagent_marker = if s.subagent_count > 0 {
        let base = if selected {
            theme.selected_row()
        } else {
            Style::default().fg(theme.teal).add_modifier(Modifier::BOLD)
        };
        Some(Span::styled(
            format!(" ◈{} ", s.subagent_count),
            maybe_fade(base, theme, age, apply_fade),
        ))
    } else {
        None
    };

    let cost = format_cost(s.total_cost_usd);
    let cost_style_base = cost_style(s.total_cost_usd, theme, selected);
    let cost_style = maybe_fade(cost_style_base, theme, age, apply_fade);
    let cost_span = Span::styled(format!("{cost:>7}"), cost_style);

    let age_label = relative_time(s.last_timestamp);
    let age_span = Span::styled(
        format!(" {age_label:>4}"),
        if selected {
            theme.selected_row()
        } else {
            // The age column is the rare thing we DO want to still look aged
            // — even a "3d"/"Apr 10" string should colour-fade in sync with
            // the rest of the row. Use the static age_style, then fade.
            maybe_fade(age_style(s.last_timestamp, theme), theme, age, apply_fade)
        },
    );

    let mut spans = vec![
        Span::styled(format!(" {pointer} "), pointer_style),
        pin,
        name_span,
        Span::raw(" "),
        pill,
    ];
    if let Some(p) = perm_pill {
        spans.push(Span::raw(" "));
        spans.push(p);
    }
    if let Some(m) = subagent_marker {
        spans.push(m);
    }
    spans.push(Span::raw(" "));
    // Cost-burn heat bar (E6 fallback: per-turn history isn't on `Session`,
    // so tint a 1-cell bar by the session's running total). Drops out on
    // panes narrower than 80 cols so the name column keeps its width.
    if show_cost_bar {
        let bar_fg = cost_burn_color(s.total_cost_usd, theme);
        let mut bar_style = Style::default().fg(bar_fg).add_modifier(Modifier::BOLD);
        if apply_fade {
            bar_style = theme::age_fade_style(theme, bar_style, age);
        }
        // Left three-eighths block — reads as a vertical "heat rail" without
        // using a full block that would visually merge into the pill/cost.
        spans.push(Span::styled("\u{258D}", bar_style));
        spans.push(Span::raw(" "));
    }
    spans.push(cost_span);
    spans.push(age_span);

    // If selected, stripe the row background by injecting a surface0 span
    // of leading whitespace. We already styled pieces, so just ensure the
    // name/cost/age segments carry the bg.
    //
    // The glide trail uses the same surface0 wash but only during the
    // 150 ms ghost window — so a just-moved-from row fades back into the
    // list in the next few frames.
    if selected || glide_trail {
        for span in &mut spans {
            span.style.bg = Some(theme.surface0);
        }
    }

    // Context-usage gutter (E7): 1 cell anchored at `area_width - 2`. Inset
    // by 1 so the scrollbar thumb on the last column never clobbers it. We
    // pad with spaces (carrying the selected-row bg, if any) so the gutter
    // aligns visually regardless of the row content width.
    //
    // Skip entirely if the row already overflows the target column — a
    // visually misaligned gutter reads as a rendering bug, and the content
    // will be clipped by Ratatui at `area_width` anyway.
    if show_gutter && area_width >= 2 {
        let content_w: usize = spans
            .iter()
            .map(|sp| display_width(sp.content.as_ref()))
            .sum();
        // Target column for the gutter cell — reserve 1 cell to the left of
        // the rightmost column so a scrollbar thumb doesn't paint over the
        // gauge.
        let gutter_col = area_width.saturating_sub(2);
        if content_w <= gutter_col {
            let pad_n = gutter_col - content_w;
            if pad_n > 0 {
                let mut pad_style = Style::default();
                if selected || glide_trail {
                    pad_style = pad_style.bg(theme.surface0);
                }
                spans.push(Span::styled(" ".repeat(pad_n), pad_style));
            }
            let ctx_fg = ctx_gutter_color(s, theme);
            let mut gutter_style = Style::default().fg(ctx_fg).add_modifier(Modifier::BOLD);
            if selected || glide_trail {
                gutter_style = gutter_style.bg(theme.surface0);
            }
            // Right-eighth block (U+2595) hugs the right edge without
            // filling the full cell, so tall selection backgrounds still
            // read cleanly.
            spans.push(Span::styled("\u{2595}", gutter_style));
        }
    }

    Line::from(spans)
}

/// Colour for the cost-burn 1-cell bar (E6 fallback variant).
///
/// Buckets per the brief: ≤$1 → green (cool), $1–$10 → amber/yellow, $10+
/// → rose/red. A separate ramp from [`theme::cost_color`] on purpose — this
/// bar is a binary "pay attention" signal, not a fine-grained heat map, so
/// we collapse to three tiers for instant legibility. Zero-cost rows render
/// against the muted overlay so an empty session doesn't light up green.
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
/// Zero-cost rows stay dim (they're still "cheap"); everything else rides the
/// shared `theme::cost_color` ramp so the session-list, tree, preview, and
/// conversation-viewer all agree about "hot" vs "cool" money.
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

fn empty_copy_no_sessions<'a>() -> Vec<Line<'a>> {
    vec![
        Line::raw("No Claude Code sessions found."),
        Line::raw(""),
        Line::raw("Run `claude` somewhere to create one."),
    ]
}

fn empty_copy_no_matches(filter: &str) -> Vec<Line<'_>> {
    vec![
        Line::raw(format!("No matches for \"{filter}\".")),
        Line::raw(""),
        Line::raw("Press Esc to clear the filter."),
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

