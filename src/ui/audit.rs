//! Cost audit screen — rendering + state for `claude-picker audit`.
//!
//! The data shape ([`AuditFinding`]) comes from [`crate::data::cost_audit`].
//! This module just knows how to draw the scrollable list and navigate it.
//!
//! Layout (rough sketch):
//!
//! ```text
//! ╭─ claude-picker · cost audit ────────── 23 suggestions ─────╮
//! │                                                             │
//! │  ▸ architex / testing1                        $402.56       │
//! │    ⚠ 81% tool_use tokens — Haiku could save ~$240          │
//! │                                                             │
//! │    ecommerce-api / rate-limiter               $0.37         │
//! │    ℹ model: opus · 1.2k tokens — Sonnet would suffice      │
//! ╰─────────────────────────────────────────────────────────────╯
//!   ↑↓ navigate · Enter open · q quit   total savings: ~$287
//! ```

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::data::cost_audit::{AuditFinding, Severity};
use crate::theme::Theme;

/// View state: the list of findings plus cursor/scroll position.
pub struct AuditState {
    pub findings: Vec<AuditFinding>,
    pub cursor: usize,
    /// Top row currently rendered (scroll offset).
    pub scroll: usize,
    pub should_quit: bool,
    /// If the user hit Enter, the selected session to resume. Consumed by the
    /// command loop after the TUI restores the terminal.
    pub selection: Option<AuditSelection>,
    /// `?` help overlay visible.
    pub show_help: bool,
}

/// What the user chose to open when exiting.
pub struct AuditSelection {
    pub session_id: String,
    pub project_cwd: std::path::PathBuf,
}

impl AuditState {
    pub fn new(findings: Vec<AuditFinding>) -> Self {
        Self {
            findings,
            cursor: 0,
            scroll: 0,
            should_quit: false,
            selection: None,
            show_help: false,
        }
    }

    pub fn move_cursor(&mut self, delta: i32) {
        let len = self.findings.len();
        if len == 0 {
            return;
        }
        let current = self.cursor as i32;
        let next = (current + delta).rem_euclid(len as i32);
        self.cursor = next as usize;
    }

    pub fn confirm(&mut self) {
        if let Some(f) = self.findings.get(self.cursor) {
            self.selection = Some(AuditSelection {
                session_id: f.session_id.clone(),
                project_cwd: f.project_cwd.clone(),
            });
            self.should_quit = true;
        }
    }
}

/// Total potential savings across every finding. Rendered in the footer.
pub fn total_savings(state: &AuditState) -> f64 {
    state.findings.iter().map(|f| f.estimated_savings_usd).sum()
}

/// Render the audit screen into `area`.
pub fn render(f: &mut Frame<'_>, area: Rect, state: &mut AuditState, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);
    render_body(f, chunks[0], state, theme);
    render_footer(f, chunks[1], state, theme);
}

/// Draw the scrollable list of findings inside a rounded border.
fn render_body(f: &mut Frame<'_>, area: Rect, state: &mut AuditState, theme: &Theme) {
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "claude-picker · cost audit",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            format!("{} suggestions", state.findings.len()),
            theme.muted(),
        ),
        Span::raw(" "),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border())
        .title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.findings.is_empty() {
        let msg = Paragraph::new(vec![
            Line::raw(""),
            Line::raw(""),
            Line::styled(
                "  no cost-savings suggestions — every session looks efficient",
                theme.muted(),
            ),
            Line::raw(""),
            Line::styled("  run more sessions and try again later", theme.dim()),
        ]);
        f.render_widget(msg, inner);
        return;
    }

    // Each finding block = 1 header + N findings + 1 blank = variable height.
    // We flatten into lines, then scroll by line so the cursor-to-finding
    // mapping stays straightforward.
    let width = inner.width as usize;
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(state.findings.len() * 4);
    let mut first_line_of: Vec<usize> = Vec::with_capacity(state.findings.len());
    for (i, f) in state.findings.iter().enumerate() {
        first_line_of.push(lines.len());
        let selected = i == state.cursor;
        render_finding_into(&mut lines, f, selected, width, theme);
        // Blank line between blocks — but not trailing.
        if i + 1 < state.findings.len() {
            lines.push(Line::raw(""));
        }
    }

    // Auto-scroll so the selected finding stays on screen.
    let sel_line = first_line_of.get(state.cursor).copied().unwrap_or(0);
    let visible = inner.height as usize;
    if sel_line < state.scroll {
        state.scroll = sel_line;
    } else if sel_line + 2 > state.scroll + visible {
        state.scroll = sel_line + 2 + 1 - visible.max(2);
    }
    let skip = state.scroll.min(lines.len().saturating_sub(1));
    let body: Vec<Line<'_>> = lines.into_iter().skip(skip).take(visible).collect();
    f.render_widget(Paragraph::new(body), inner);
}

/// Append one finding's worth of lines to `lines`. Layout:
///
/// ```text
/// ▸ project / session-label                        $cost
///   ⚠ first finding message
///   ℹ second finding message
/// ```
fn render_finding_into<'a>(
    lines: &mut Vec<Line<'a>>,
    finding: &'a AuditFinding,
    selected: bool,
    width: usize,
    theme: &Theme,
) {
    // Header row.
    let cursor = if selected { "▸" } else { " " };
    let header_left = format!(
        "{cursor} {} / {}",
        finding.project_name, finding.session_label
    );
    let cost_str = if finding.total_cost_usd < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${:.2}", finding.total_cost_usd)
    };
    let used_w = header_left.chars().count() + cost_str.chars().count() + 4;
    let pad = width.saturating_sub(used_w).max(1);

    let header_style = if selected {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtext1)
    };
    let cost_style = if finding.total_cost_usd >= 5.0 {
        Style::default()
            .fg(theme.peach)
            .add_modifier(Modifier::BOLD)
    } else if finding.total_cost_usd >= 1.0 {
        Style::default().fg(theme.yellow)
    } else {
        theme.muted()
    };
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(header_left, header_style),
        Span::raw(" ".repeat(pad)),
        Span::styled(cost_str, cost_style),
        Span::raw("  "),
    ]));

    // Finding detail rows.
    for finding_row in &finding.findings {
        let (glyph, glyph_style) = match finding_row.severity {
            Severity::Warn => (
                "⚠",
                Style::default()
                    .fg(theme.peach)
                    .add_modifier(Modifier::BOLD),
            ),
            Severity::Info => (
                "ℹ",
                Style::default().fg(theme.blue).add_modifier(Modifier::BOLD),
            ),
        };
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(glyph, glyph_style),
            Span::raw(" "),
            Span::styled(finding_row.message.clone(), theme.body()),
        ]));
    }
}

/// Footer with key hints + total savings on the right.
fn render_footer(f: &mut Frame<'_>, area: Rect, state: &AuditState, theme: &Theme) {
    let sep_style = theme.dim();
    let keys = Line::from(vec![
        Span::raw("  "),
        Span::styled("↑↓", theme.key_hint()),
        Span::raw(" "),
        Span::styled("navigate", theme.key_desc()),
        Span::styled("  ·  ", sep_style),
        Span::styled("Enter", theme.key_hint()),
        Span::raw(" "),
        Span::styled("open session", theme.key_desc()),
        Span::styled("  ·  ", sep_style),
        Span::styled("q", theme.key_hint()),
        Span::raw(" "),
        Span::styled("quit", theme.key_desc()),
    ]);
    let savings = total_savings(state);
    let savings_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("total potential savings: ", theme.muted()),
        Span::styled(
            format!("~${:.2}", savings),
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    // Render both lines; ratatui draws one per row if we stack them in a
    // 2-high area.
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(vec![keys, savings_line]), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::cost_audit::{Finding, FindingKind};

    fn mk_finding(id: &str, cost: f64, savings: f64) -> AuditFinding {
        AuditFinding {
            session_id: id.into(),
            project_name: "proj".into(),
            project_cwd: std::path::PathBuf::from("/tmp/proj"),
            session_label: id.into(),
            total_cost_usd: cost,
            model_summary: "claude-opus-4-7".into(),
            findings: vec![Finding {
                severity: Severity::Warn,
                kind: FindingKind::ToolRatio,
                message: "big savings available".into(),
                savings_usd: savings,
            }],
            estimated_savings_usd: savings,
        }
    }

    #[test]
    fn cursor_wraps_with_rem_euclid() {
        let mut s = AuditState::new(vec![mk_finding("a", 1.0, 0.5), mk_finding("b", 2.0, 1.0)]);
        s.move_cursor(-1);
        assert_eq!(s.cursor, 1, "up from 0 wraps to last");
        s.move_cursor(1);
        assert_eq!(s.cursor, 0, "down from last wraps to 0");
    }

    #[test]
    fn confirm_records_selection() {
        let mut s = AuditState::new(vec![mk_finding("abc", 0.5, 0.25)]);
        s.confirm();
        let sel = s.selection.expect("selection set");
        assert_eq!(sel.session_id, "abc");
        assert!(s.should_quit);
    }

    #[test]
    fn total_savings_sums_across_rows() {
        let s = AuditState::new(vec![
            mk_finding("a", 1.0, 0.5),
            mk_finding("b", 2.0, 1.0),
            mk_finding("c", 3.0, 1.5),
        ]);
        assert!((total_savings(&s) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn empty_findings_list_is_allowed() {
        let s = AuditState::new(vec![]);
        assert_eq!(total_savings(&s), 0.0);
        assert_eq!(s.cursor, 0);
    }
}
