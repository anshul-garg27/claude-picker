//! Project selection screen — shown when the user runs `claude-picker` from
//! a directory with no Claude sessions, or when the session screen wants to
//! pop back to project-choice.
//!
//! Single pane, rounded border, one row per project with session count and
//! git branch badge.

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
use crate::data::Project;
use crate::theme::Theme;
use crate::ui::text::pad_to_width;

pub fn render(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border_active())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "claude-picker — projects",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_top(
            Line::from(Span::styled(
                format!(" {}/{} ", app.filtered_indices.len(), app.projects.len()),
                Style::default().fg(theme.subtext1),
            ))
            .right_aligned(),
        );

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    render_filter(f, chunks[0], app);
    render_list(f, chunks[1], app);
}

fn render_filter(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let text: Line<'_> = if app.filter.is_empty() {
        Line::from(vec![
            Span::styled("> ", theme.muted()),
            Span::styled("type to filter projects…", theme.filter_placeholder()),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", theme.muted()),
            Span::styled(app.filter.clone(), theme.filter_text()),
            Span::styled(" ", Style::default().bg(theme.mauve).fg(theme.crust)),
        ])
    };

    // When the filter has content, promote the border to mauve so the user
    // sees at-a-glance that typing is landing in the filter. The active
    // filter also bumps up to Thick so weight matches the session-list
    // variant's visual language.
    let (border_color, border_type) = if !app.filter.is_empty() {
        (Style::default().fg(theme.mauve), BorderType::Thick)
    } else {
        (Style::default().fg(theme.surface1), BorderType::Rounded)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(border_color);

    f.render_widget(Paragraph::new(text).block(block), area);
}

fn render_list(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    if app.projects.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::raw("No Claude Code projects yet."),
            Line::raw(""),
            Line::raw("Run `claude` in any directory to get started."),
        ])
        .style(theme.muted())
        .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem<'_>> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &idx)| {
            let p = &app.projects[idx];
            let is_sel = Some(display_idx) == app.cursor_position();
            ListItem::new(render_row(p, theme, is_sel))
        })
        .collect();

    let mut state = ListState::default();
    state.select(app.cursor_position());
    let list = List::new(items).highlight_symbol("");
    f.render_stateful_widget(list, area, &mut state);

    // Scrollbar on the right edge, only when the list overflows.
    let total = app.filtered_indices.len();
    if total > area.height as usize {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(Style::default().fg(theme.surface1))
            .thumb_style(Style::default().fg(theme.mauve));
        let mut sb_state = ScrollbarState::new(total)
            .position(app.cursor)
            .viewport_content_length(area.height as usize);
        f.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 0,
                horizontal: 0,
            }),
            &mut sb_state,
        );
    }
}

fn render_row<'a>(p: &'a Project, theme: &Theme, selected: bool) -> Line<'a> {
    let name_style = if selected {
        theme.selected_row()
    } else {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    };

    let pointer = if selected { "▸" } else { " " };
    let pointer_style = if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface2)
    };

    // Pad to exactly 30 display columns. The old `format!("{name:<30}")`
    // path measured the formatter's padding in bytes-not-columns; a CJK or
    // emoji project name would shift the branch/sessions columns out of
    // alignment. `pad_to_width` is the column-correct replacement.
    let name = pad_to_width(&p.name, 30);

    let branch = p
        .git_branch
        .as_deref()
        .map(|b| format!(" ⌥ {b}"))
        .unwrap_or_default();

    let sessions = format!("{} sessions", p.session_count);
    let age = project_age(p.last_activity);

    let mut spans = vec![
        Span::styled(format!(" {pointer} "), pointer_style),
        Span::styled(name, name_style),
        Span::styled(branch, Style::default().fg(theme.green)),
        Span::raw(" "),
        Span::styled(sessions, Style::default().fg(theme.overlay1)),
        Span::raw("  "),
        Span::styled(age, theme.muted()),
    ];

    if selected {
        for span in &mut spans {
            span.style.bg = Some(theme.surface0);
        }
    }

    Line::from(spans)
}

fn project_age(ts: Option<DateTime<Utc>>) -> String {
    let Some(ts) = ts else {
        return "—".to_string();
    };
    let now = Utc::now();
    let diff = now.signed_duration_since(ts);
    if diff.num_minutes() < 60 {
        format!("{}m", diff.num_minutes().max(1))
    } else if diff.num_hours() < 24 {
        format!("{}h", diff.num_hours())
    } else if diff.num_days() < 7 {
        format!("{}d", diff.num_days())
    } else {
        ts.format("%b %d").to_string()
    }
}
