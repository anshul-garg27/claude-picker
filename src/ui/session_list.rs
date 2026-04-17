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

use std::borrow::Cow;

use chrono::{DateTime, Utc};
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;

use crate::app::App;
use crate::data::Session;
use crate::theme::Theme;
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

    // Outer block — title top-left, counter top-right.
    let title = outer_title_spans(app);
    let counter = Line::from(vec![Span::styled(
        format!("{}/{}", app.filtered_indices.len(), app.sessions.len()),
        Style::default()
            .fg(theme.subtext1)
            .add_modifier(Modifier::BOLD),
    )])
    .right_aligned();

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
}

/// Build the spans that go in the outer-block title.
fn outer_title_spans(app: &App) -> Line<'_> {
    let theme = &app.theme;
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
        Span::raw(" "),
    ])
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

    let border_color = if !app.filter.is_empty() {
        // Active filter: bright mauve so it's unmistakable the keystrokes
        // are landing here.
        Style::default().fg(theme.mauve)
    } else if app.filter_focused {
        theme.panel_border_active()
    } else {
        Style::default().fg(theme.surface1)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
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

    let items: Vec<ListItem<'_>> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &sess_idx)| {
            let s = &app.sessions[sess_idx];
            let is_selected = Some(display_idx) == app.cursor_position();
            let is_bookmarked = app.bookmarks.contains(&s.id);
            let is_multi = app.is_multi_selected(sess_idx);
            ListItem::new(render_row(s, theme, is_selected, is_bookmarked, is_multi))
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
/// `▸ session-name…………… [opus] $1.24 2h`
fn render_row<'a>(
    s: &'a Session,
    theme: &Theme,
    selected: bool,
    bookmarked: bool,
    multi: bool,
) -> Line<'a> {
    // Multi-select rows recolor the name in peach-bold regardless of cursor
    // state so the visual distinction reads at a glance. Selection still wins
    // for the cursor row's background stripe (applied below).
    let name_style = if multi {
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

    let pointer_style = if multi {
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
        Span::styled("■ ", Style::default().fg(theme.blue))
    } else if s.is_fork {
        Span::styled("↳ ", Style::default().fg(theme.peach))
    } else {
        Span::raw("  ")
    };

    // display_width-aware: CJK / emoji session names used to overflow the
    // column because .chars().count() undercounted them. Use the unicode
    // helpers so the name always occupies exactly NAME_COL_WIDTH terminal
    // cells, pad or truncate.
    let name = pad_to_width(s.display_label(), NAME_COL_WIDTH);
    let name_span = Span::styled(name, name_style);

    let pill = model_pill::pill(crate::data::pricing::family(&s.model_summary), theme);

    // Optional permission-mode pill — only drawn for non-default modes.
    let perm_pill = s
        .permission_mode
        .and_then(|m| model_pill::permission_pill(m, theme));

    // Subagent marker — tiny "◈ N" glyph when the session spawned
    // sub-agents, otherwise nothing. Using ASCII to stay brand-aligned
    // (no emojis anywhere in the UI).
    let subagent_marker = if s.subagent_count > 0 {
        Some(Span::styled(
            format!(" ◈{} ", s.subagent_count),
            if selected {
                theme.selected_row()
            } else {
                Style::default().fg(theme.teal).add_modifier(Modifier::BOLD)
            },
        ))
    } else {
        None
    };

    let cost = format_cost(s.total_cost_usd);
    let cost_style = cost_style(s.total_cost_usd, theme, selected);
    let cost_span = Span::styled(format!("{cost:>7}"), cost_style);

    let age = relative_time(s.last_timestamp);
    let age_span = Span::styled(
        format!(" {age:>4}"),
        if selected {
            theme.selected_row()
        } else {
            age_style(s.last_timestamp, theme)
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
    spans.push(cost_span);
    spans.push(age_span);

    // If selected, stripe the row background by injecting a surface0 span
    // of leading whitespace. We already styled pieces, so just ensure the
    // name/cost/age segments carry the bg.
    if selected {
        for span in &mut spans {
            span.style.bg = Some(theme.surface0);
        }
    }

    Line::from(spans)
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

/// Bucketed coloring for the cost column — tiny/dim, medium/yellow, big/red.
fn cost_style(cost: f64, theme: &Theme, selected: bool) -> Style {
    let fg = if cost < 0.10 {
        theme.subtext0
    } else if cost < 1.00 {
        theme.yellow
    } else {
        theme.peach
    };
    let mut s = Style::default().fg(fg);
    if selected {
        s = s.bg(theme.surface0);
    }
    s
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
}
