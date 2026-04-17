//! `claude-picker --hooks` — hook awareness panel.
//!
//! Two vertically-stacked sections, all inside one rounded border:
//!
//! 1. **Configured hooks** — one row per `(event, matcher, command, source)`
//!    tuple, split into a "global" group and a "per-project" group.
//! 2. **Recent executions** — one row per hook-event-name, with call count,
//!    mean duration, and last-exit-code failure state. Sourced from the
//!    `{"type":"attachment","attachment":{"hookEventName":…,"exitCode":…}}`
//!    records in every session JSONL.
//!
//! The render function is pure: it never talks to the filesystem. All data
//! comes in through [`HooksView`]. That keeps us easy to unit-test and
//! mirrors how the stats dashboard is structured.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::data::settings::{HookRow, HookSource};
use crate::theme::Theme;
use crate::ui::text::{pad_to_width, truncate_to_width};

/// What the event loop passes to [`render`].
#[derive(Debug)]
pub struct HooksView<'a> {
    pub rows: &'a [HookRow],
    pub executions: &'a [HookExecutionStats],
    pub selected: usize,
    /// Header captions: total count + "fired today" tally.
    pub fired_today: u32,
}

/// One row in the "recent executions" block.
#[derive(Debug, Clone)]
pub struct HookExecutionStats {
    /// Hook event name (matches [`HookRow::event`]).
    pub event: String,
    pub calls: u64,
    /// Mean duration — milliseconds. Rendered as `12ms`. Zero when the
    /// JSONL never recorded a `durationMs`.
    pub mean_ms: u64,
    /// Last-seen non-zero exit code. `None` means every recorded run was a
    /// success; renders as `✓`. `Some(code)` renders as `✗ FAILED`.
    pub last_failure: Option<i64>,
    /// Relative label for the last call — "3h ago", "just now". Computed in
    /// the command layer, not here — the UI is pure.
    pub last_relative: String,
}

/// Minimum viable size — below this we show a "resize me" hint.
const MIN_W: u16 = 70;
const MIN_H: u16 = 18;
const MAX_W: u16 = 110;

/// Top-level render. `area` is the full frame; we cap width and center.
pub fn render(frame: &mut Frame<'_>, area: Rect, view: &HooksView<'_>, theme: &Theme) {
    if area.width < MIN_W || area.height < MIN_H {
        render_too_small(frame, area, theme);
        return;
    }
    let inner = center_capped(area, MAX_W);

    // Title + count banner + configured block + executions block + footer.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border_active())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "claude-picker · hooks",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_top(
            Line::from(Span::styled(
                format!(
                    " {} configured, {} fired today ",
                    view.rows.len(),
                    view.fired_today
                ),
                theme.subtle(),
            ))
            .right_aligned(),
        );

    let body = block.inner(inner);
    frame.render_widget(block, inner);

    // Vertical split — roughly 60% configured / 40% executions, separated by
    // a one-line "section header" divider.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top blank
            Constraint::Min(6),    // configured
            Constraint::Length(1), // blank
            Constraint::Min(5),    // executions
            Constraint::Length(1), // footer
        ])
        .split(body);

    render_configured(frame, chunks[1], view, theme);
    render_executions(frame, chunks[3], view, theme);
    render_footer(frame, chunks[4], theme);
}

fn render_configured(frame: &mut Frame<'_>, area: Rect, view: &HooksView<'_>, theme: &Theme) {
    if view.rows.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::from(Span::styled("No hooks configured.", theme.muted()))
                .alignment(Alignment::Center),
            Line::raw(""),
            Line::from(Span::styled(
                "Edit ~/.claude/settings.json to add one.",
                theme.subtle(),
            ))
            .alignment(Alignment::Center),
        ]);
        frame.render_widget(p, area);
        return;
    }

    // Partition rows into global + per-project, preserving order. The UI
    // contract is that each source gets its own labeled sub-heading.
    let global: Vec<&HookRow> = view
        .rows
        .iter()
        .filter(|r| matches!(r.source, HookSource::Global))
        .collect();
    let project: Vec<&HookRow> = view
        .rows
        .iter()
        .filter(|r| matches!(r.source, HookSource::Project(_)))
        .collect();

    // Row numbers run global-first, matching the flat rows slice.
    let mut lines: Vec<ListItem<'_>> = Vec::with_capacity(view.rows.len() + 4);
    lines.push(ListItem::new(section_header(
        "── global hooks (~/.claude/settings.json) ──",
        theme,
    )));
    let mut row_idx = 0;
    for r in &global {
        lines.push(ListItem::new(render_hook_row(
            r,
            row_idx == view.selected,
            area.width,
            theme,
        )));
        row_idx += 1;
    }

    if !project.is_empty() {
        lines.push(ListItem::new(Line::raw("")));
        lines.push(ListItem::new(section_header(
            "── per-project overrides ──",
            theme,
        )));
        for r in &project {
            lines.push(ListItem::new(render_hook_row(
                r,
                row_idx == view.selected,
                area.width,
                theme,
            )));
            row_idx += 1;
        }
    }

    // The `List` widget auto-handles highlight — but we're baking selection
    // into the row ourselves (to control column alignment), so ListState
    // selection is unused here.
    let state = ListState::default();
    let list = List::new(lines);
    frame.render_stateful_widget(list, area, &mut state.clone());
}

/// Build one hook row:
/// ```text
/// ▸ pre-tool-use    Bash    /path/to/script.sh
/// ```
fn render_hook_row<'a>(row: &'a HookRow, selected: bool, width: u16, theme: &Theme) -> Line<'a> {
    let caret = if selected { "▸" } else { " " };
    // Column widths chosen so a 100-col terminal still fits the command.
    let event_col: usize = 22;
    let matcher_col: usize = 10;
    let prefix_width = 2 + event_col + 1 + matcher_col + 1; // caret + space pads
    let command_budget = (width as usize).saturating_sub(prefix_width + 2);
    let matcher_text = row.matcher.as_deref().unwrap_or("·");
    let event_style = if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text)
    };
    let matcher_style = if row.matcher.is_some() {
        Style::default().fg(theme.teal)
    } else {
        theme.muted()
    };

    Line::from(vec![
        Span::styled(
            format!("{caret} "),
            Style::default().fg(if selected {
                theme.mauve
            } else {
                theme.overlay0
            }),
        ),
        Span::styled(pad_to_width(&row.event, event_col), event_style),
        Span::raw(" "),
        Span::styled(pad_to_width(matcher_text, matcher_col), matcher_style),
        Span::raw(" "),
        Span::styled(
            truncate_to_width(&row.command, command_budget),
            theme.subtle(),
        ),
    ])
}

fn render_executions(frame: &mut Frame<'_>, area: Rect, view: &HooksView<'_>, theme: &Theme) {
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(view.executions.len() + 2);
    lines.push(section_header("── recent executions (last 24h) ──", theme));

    if view.executions.is_empty() {
        lines.push(Line::raw(""));
        lines.push(
            Line::from(Span::styled("No hook events recorded yet.", theme.muted()))
                .alignment(Alignment::Center),
        );
    } else {
        for ex in view.executions {
            lines.push(render_execution_row(ex, theme));
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

/// One execution row:
/// ```text
///   ✓ pre-tool-use   5ms    42 times
///   ✗ user-prompt   FAILED  1 time  last: 3h ago
/// ```
fn render_execution_row<'a>(ex: &'a HookExecutionStats, theme: &Theme) -> Line<'a> {
    let (glyph, glyph_style) = match ex.last_failure {
        None => (
            "✓",
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
        Some(_) => (
            "✗",
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
        ),
    };
    let duration_col = if ex.last_failure.is_some() {
        "FAILED".to_string()
    } else if ex.mean_ms == 0 {
        "—".to_string()
    } else {
        format!("{}ms", ex.mean_ms)
    };
    let times_suffix = if ex.calls == 1 { "time" } else { "times" };
    Line::from(vec![
        Span::raw("  "),
        Span::styled(glyph, glyph_style),
        Span::raw(" "),
        Span::styled(pad_to_width(&ex.event, 22), theme.body()),
        Span::raw(" "),
        Span::styled(pad_to_width(&duration_col, 7), theme.subtle()),
        Span::raw(" "),
        Span::styled(format!("{} {times_suffix}", ex.calls), theme.muted()),
        Span::raw("  "),
        Span::styled(
            if ex.last_relative.is_empty() {
                String::new()
            } else {
                format!("last: {}", ex.last_relative)
            },
            theme.muted(),
        ),
    ])
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let line = Line::from(vec![
        Span::styled(
            "  ↑↓",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" navigate · ", theme.muted()),
        Span::styled(
            "Enter",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" view sessions · ", theme.muted()),
        Span::styled(
            "e",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" edit hook · ", theme.muted()),
        Span::styled(
            "q",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" quit", theme.muted()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn section_header<'a>(label: &'a str, theme: &Theme) -> Line<'a> {
    Line::from(Span::styled(
        format!("  {label}"),
        Style::default()
            .fg(theme.overlay1)
            .add_modifier(Modifier::DIM),
    ))
}

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

fn render_too_small(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let p = Paragraph::new(vec![
        Line::raw(""),
        Line::from(Span::styled(
            format!("resize terminal — need at least {MIN_W}×{MIN_H}"),
            theme.muted(),
        ))
        .alignment(Alignment::Center),
    ]);
    frame.render_widget(p, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn mk_row(event: &str, matcher: Option<&str>, cmd: &str, source: HookSource) -> HookRow {
        HookRow {
            event: event.to_string(),
            matcher: matcher.map(String::from),
            command: cmd.to_string(),
            source,
        }
    }

    #[test]
    fn render_draws_global_and_project_sections() {
        let rows = vec![
            mk_row("PreToolUse", Some("Bash"), "/bin/x.sh", HookSource::Global),
            mk_row(
                "PostToolUse",
                None,
                "/bin/y.sh",
                HookSource::Project(std::path::PathBuf::from("/tmp/proj")),
            ),
        ];
        let execs = vec![HookExecutionStats {
            event: "PreToolUse".into(),
            calls: 5,
            mean_ms: 12,
            last_failure: None,
            last_relative: "2m ago".into(),
        }];
        let theme = Theme::mocha();
        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        terminal
            .draw(|f| {
                let view = HooksView {
                    rows: &rows,
                    executions: &execs,
                    selected: 0,
                    fired_today: 5,
                };
                render(f, f.area(), &view, &theme);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let content: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        // Every 100 chars is one row.
        assert!(content.contains("claude-picker"));
        assert!(content.contains("global hooks"));
        assert!(content.contains("per-project overrides"));
        assert!(content.contains("PreToolUse"));
        assert!(content.contains("PostToolUse"));
        assert!(content.contains("recent executions"));
    }

    #[test]
    fn render_shows_empty_state_when_no_hooks() {
        let rows: Vec<HookRow> = Vec::new();
        let execs: Vec<HookExecutionStats> = Vec::new();
        let theme = Theme::mocha();
        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        terminal
            .draw(|f| {
                let view = HooksView {
                    rows: &rows,
                    executions: &execs,
                    selected: 0,
                    fired_today: 0,
                };
                render(f, f.area(), &view, &theme);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let content: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("No hooks configured"));
    }

    #[test]
    fn small_terminal_shows_resize_hint() {
        let theme = Theme::mocha();
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal
            .draw(|f| {
                let view = HooksView {
                    rows: &[],
                    executions: &[],
                    selected: 0,
                    fired_today: 0,
                };
                render(f, f.area(), &view, &theme);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let content: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("resize"));
    }
}
