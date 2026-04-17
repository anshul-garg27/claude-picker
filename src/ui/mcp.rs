//! `claude-picker --mcp` — MCP server awareness panel.
//!
//! Three stacked sections inside a single rounded panel:
//!
//! 1. **Installed servers** — the union of what's declared in
//!    `~/.claude/settings.json` (or `~/.claude.json` → `mcpServers`) and
//!    what Claude Code actually invoked (`mcp__<server>__*` tool calls).
//! 2. **Top tools used** — the top-N tool names by call count.
//! 3. **Sessions drill-down hint** — a placeholder strip pointing the user
//!    at the `Enter` keybinding. When a server is selected, the command
//!    layer pushes into a session sub-picker, so this is just a hint inside
//!    the panel itself.
//!
//! Render is pure — no filesystem access. Input is [`McpView`] populated by
//! the command layer.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::data::mcp_calls::{ServerStats, ToolCallStats};
use crate::theme::Theme;
use crate::ui::text::{pad_to_width, truncate_to_width};

/// Payload the event loop passes to [`render`].
#[derive(Debug)]
pub struct McpView<'a> {
    pub servers: &'a [ServerStats],
    pub top_tools: &'a [ToolCallStats],
    /// Which server row is focused. When `servers` is empty this is
    /// ignored.
    pub selected: usize,
    /// Total calls in the scan window — shown in the title caption.
    pub total_calls: u64,
    /// Relative label for each server's `last_used`. Index-aligned with
    /// `servers`. Empty string for "never".
    pub last_used_labels: &'a [String],
}

const MIN_W: u16 = 70;
const MIN_H: u16 = 18;
const MAX_W: u16 = 110;

pub fn render(frame: &mut Frame<'_>, area: Rect, view: &McpView<'_>, theme: &Theme) {
    if area.width < MIN_W || area.height < MIN_H {
        render_too_small(frame, area, theme);
        return;
    }
    let inner = center_capped(area, MAX_W);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border_active())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "claude-picker · mcp servers",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_top(
            Line::from(Span::styled(
                format!(
                    " {} installed, {} calls 30d ",
                    view.servers.len(),
                    view.total_calls
                ),
                theme.subtle(),
            ))
            .right_aligned(),
        );
    let body = block.inner(inner);
    frame.render_widget(block, inner);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top blank
            Constraint::Min(6),    // installed
            Constraint::Length(1), // blank
            Constraint::Length(6), // top tools (fixed 5 rows + header)
            Constraint::Length(1), // blank
            Constraint::Length(3), // drill-down hint
            Constraint::Length(1), // footer
        ])
        .split(body);

    render_servers(frame, chunks[1], view, theme);
    render_top_tools(frame, chunks[3], view, theme);
    render_sessions_hint(frame, chunks[5], view, theme);
    render_footer(frame, chunks[6], theme);
}

fn render_servers(frame: &mut Frame<'_>, area: Rect, view: &McpView<'_>, theme: &Theme) {
    let mut lines = Vec::with_capacity(view.servers.len() + 2);
    lines.push(section_header("── installed servers ──", theme));

    if view.servers.is_empty() {
        lines.push(Line::raw(""));
        lines.push(
            Line::from(Span::styled("No MCP servers configured.", theme.muted()))
                .alignment(Alignment::Center),
        );
        lines.push(
            Line::from(Span::styled(
                "Run `claude mcp add <name> <command>` to install one.",
                theme.subtle(),
            ))
            .alignment(Alignment::Center),
        );
        frame.render_widget(Paragraph::new(lines), area);
        return;
    }

    for (i, s) in view.servers.iter().enumerate() {
        let selected = i == view.selected;
        let last_label = view
            .last_used_labels
            .get(i)
            .map(String::as_str)
            .unwrap_or("");
        lines.push(render_server_row(
            s, last_label, selected, area.width, theme,
        ));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// One server row:
/// ```text
/// ▸ context7            72 calls   last: 2h ago
/// ```
fn render_server_row<'a>(
    s: &'a ServerStats,
    last_label: &'a str,
    selected: bool,
    width: u16,
    theme: &Theme,
) -> Line<'a> {
    let caret = if selected { "▸" } else { " " };
    let name_col: usize = 20;
    let calls_col: usize = 10;
    let caret_style = Style::default().fg(if selected {
        theme.mauve
    } else {
        theme.overlay0
    });
    let name_style = if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text)
    };
    let calls_style = if s.calls == 0 {
        theme.muted()
    } else {
        Style::default().fg(theme.green)
    };
    let last_style = theme.muted();

    let _ = width; // reserved for future right-alignment
    Line::from(vec![
        Span::styled(format!("  {caret} "), caret_style),
        Span::styled(pad_to_width(&s.name, name_col), name_style),
        Span::raw(" "),
        Span::styled(
            pad_to_width(&format!("{} calls", s.calls), calls_col),
            calls_style,
        ),
        Span::raw("   "),
        Span::styled(
            if last_label.is_empty() {
                "last: never".to_string()
            } else {
                format!("last: {last_label}")
            },
            last_style,
        ),
    ])
}

fn render_top_tools(frame: &mut Frame<'_>, area: Rect, view: &McpView<'_>, theme: &Theme) {
    let mut lines = Vec::with_capacity(view.top_tools.len() + 2);
    lines.push(section_header("── top tools used ──", theme));
    if view.top_tools.is_empty() {
        lines.push(Line::raw(""));
        lines.push(
            Line::from(Span::styled("(no tool calls observed)", theme.muted()))
                .alignment(Alignment::Center),
        );
    } else {
        // Cap to 5 rows inside the fixed 6-line allotment.
        for t in view.top_tools.iter().take(5) {
            lines.push(render_tool_row(t, area.width, theme));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_tool_row<'a>(t: &'a ToolCallStats, width: u16, theme: &Theme) -> Line<'a> {
    let name_budget = (width as usize).saturating_sub(20);
    Line::from(vec![
        Span::raw("    "),
        Span::styled(
            truncate_to_width(&t.name, name_budget),
            Style::default().fg(theme.text),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} calls", t.calls),
            Style::default().fg(theme.green),
        ),
    ])
}

fn render_sessions_hint(frame: &mut Frame<'_>, area: Rect, view: &McpView<'_>, theme: &Theme) {
    let mut lines = Vec::with_capacity(3);
    lines.push(section_header(
        "── sessions that used this server ──",
        theme,
    ));
    let msg = if view.servers.is_empty() {
        "(nothing to drill into)".to_string()
    } else if let Some(s) = view.servers.get(view.selected) {
        format!(
            "Enter to see the {} session(s) that invoked {}",
            s.sessions.len(),
            s.name
        )
    } else {
        "(select a server)".to_string()
    };
    lines.push(Line::from(Span::styled(
        format!("    {msg}"),
        theme.muted(),
    )));
    frame.render_widget(Paragraph::new(lines), area);
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
            "c",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" config · ", theme.muted()),
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
    use std::collections::BTreeSet;

    fn srv(name: &str, calls: u64) -> ServerStats {
        ServerStats {
            name: name.to_string(),
            calls,
            last_used: None,
            sessions: BTreeSet::new(),
        }
    }
    fn tool(name: &str, server: &str, calls: u64) -> ToolCallStats {
        ToolCallStats {
            name: name.to_string(),
            server: server.to_string(),
            calls,
            last_used: None,
        }
    }

    #[test]
    fn render_draws_header_and_server_rows() {
        let servers = vec![srv("context7", 72), srv("firecrawl", 45)];
        let tools = vec![tool("mcp__context7__query-docs", "context7", 48)];
        let labels = vec!["2h ago".to_string(), "4h ago".to_string()];
        let theme = Theme::mocha();
        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        terminal
            .draw(|f| {
                let view = McpView {
                    servers: &servers,
                    top_tools: &tools,
                    selected: 0,
                    total_calls: 117,
                    last_used_labels: &labels,
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
        assert!(content.contains("mcp servers"));
        assert!(content.contains("context7"));
        assert!(content.contains("firecrawl"));
        assert!(content.contains("2h ago"));
        assert!(content.contains("top tools used"));
        assert!(content.contains("query-docs"));
    }

    #[test]
    fn empty_state_for_no_servers() {
        let theme = Theme::mocha();
        let labels: Vec<String> = Vec::new();
        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        terminal
            .draw(|f| {
                let view = McpView {
                    servers: &[],
                    top_tools: &[],
                    selected: 0,
                    total_calls: 0,
                    last_used_labels: &labels,
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
        assert!(content.contains("No MCP servers configured"));
    }
}
