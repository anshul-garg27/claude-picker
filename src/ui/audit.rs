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

use crate::data::cost_audit::{summary_by_kind, AuditFinding, FindingKind, Severity};
use crate::data::tool_dist::{collect_tool_distribution, find_session_jsonl, ToolDistEntry};
use crate::theme::Theme;
use crate::ui::audit_detail;

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
    /// Drill-in overlay (#16): when `Some`, the detail view replaces the
    /// findings list. Opened by Enter on a `ToolRatio` finding, closed by
    /// another Enter press (since the outer event loop consumes Esc / q as
    /// the audit-quit path).
    pub detail: Option<AuditDetailView>,
}

/// What the user chose to open when exiting.
pub struct AuditSelection {
    pub session_id: String,
    pub project_cwd: std::path::PathBuf,
}

/// Snapshot shown by the drill-in overlay.
///
/// Cached at open-time so repeated frames don't re-parse the JSONL. The
/// `finding` is cloned from the selected row so the overlay keeps rendering
/// even if the underlying list shifts (sort order, etc.).
pub struct AuditDetailView {
    pub finding: AuditFinding,
    pub tool_dist: Vec<ToolDistEntry>,
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
            detail: None,
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

    /// Enter-key handler.
    ///
    /// - If the drill-in overlay is open → close it (Enter acts as "back").
    /// - Else if the cursor is on a `ToolRatio` finding → open the drill-in
    ///   detail view with a per-tool breakdown of the session's output.
    /// - Else → record the selection so the command loop can resume the
    ///   session after restoring the terminal (existing behaviour for
    ///   cache-efficiency / model-mismatch findings, and as a fallback when
    ///   the JSONL for a ToolRatio finding cannot be located).
    pub fn confirm(&mut self) {
        // Enter while detail is open = close detail. Cleanest "back" gesture
        // available without touching the outer command loop.
        if self.detail.is_some() {
            self.detail = None;
            return;
        }
        let Some(f) = self.findings.get(self.cursor) else {
            return;
        };
        let is_tool_ratio = f
            .findings
            .iter()
            .any(|row| row.kind == FindingKind::ToolRatio);
        if is_tool_ratio {
            if let Some(path) = find_session_jsonl(&f.session_id) {
                let tool_dist = collect_tool_distribution(&path);
                self.detail = Some(AuditDetailView {
                    finding: f.clone(),
                    tool_dist,
                });
                return;
            }
            // Fall through to resume-semantics when the JSONL disappeared
            // under us (e.g. the user wiped ~/.claude between runs).
        }
        self.selection = Some(AuditSelection {
            session_id: f.session_id.clone(),
            project_cwd: f.project_cwd.clone(),
        });
        self.should_quit = true;
    }
}

/// Total potential savings across every finding. Rendered in the footer.
pub fn total_savings(state: &AuditState) -> f64 {
    state.findings.iter().map(|f| f.estimated_savings_usd).sum()
}

/// Render the audit screen into `area`.
///
/// Layout (top to bottom):
/// 1. Summary band (5 rows: top rule + 3 heuristic rows + bottom rule) —
///    always present so every run visibly proves all three heuristics ran.
/// 2. Body — per-session findings list (the scrollable part) OR the
///    drill-in detail overlay when [`AuditState::detail`] is `Some` (#16).
/// 3. Footer — key hints + total savings (2 rows).
pub fn render(f: &mut Frame<'_>, area: Rect, state: &mut AuditState, theme: &Theme) {
    let summary = summary_by_kind(&state.findings);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);
    render_summary_band(f, chunks[0], summary, theme);
    if let Some(detail) = state.detail.as_ref() {
        audit_detail::render(f, chunks[1], detail, theme);
    } else {
        render_body(f, chunks[1], state, theme);
    }
    render_footer(f, chunks[2], state, theme);
}

/// Top-of-screen three-row band that always shows every heuristic category,
/// even when zero findings — the point is to make the "three heuristics"
/// promise visible on every audit run. Shape:
///
/// ```text
/// ─── summary ───────────────────────────
///   tool-ratio         2 findings   $3.40
///   cache-efficiency   6 findings   $0.28
///   model-mismatch     0 findings   $0.00
/// ───────────────────────────────────────
/// ```
fn render_summary_band(
    f: &mut Frame<'_>,
    area: Rect,
    summary: [(FindingKind, usize, f64); 3],
    theme: &Theme,
) {
    let rule_style = theme.dim();
    let label_style = Style::default().fg(theme.subtext1);
    let zero_label_style = theme.muted();
    let count_style = Style::default().fg(theme.mauve);
    let savings_style = Style::default()
        .fg(theme.green)
        .add_modifier(Modifier::BOLD);
    let zero_savings_style = theme.muted();

    let width = area.width as usize;
    let rule = "─".repeat(width.saturating_sub(2));

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(5);
    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled(format!("─── summary {}", rule), rule_style),
    ]));

    for (kind, count, savings) in summary {
        let label = match kind {
            FindingKind::ToolRatio => "tool-ratio",
            FindingKind::CacheEfficiency => "cache-efficiency",
            FindingKind::ModelMismatch => "model-mismatch",
        };
        let count_str = format!("{} findings", count);
        let savings_str = format!("~${:.2}", savings);
        let label_pad = 18usize.saturating_sub(label.chars().count());
        let count_pad = 14usize.saturating_sub(count_str.chars().count());
        let (lab_s, cnt_s, sav_s) = if count == 0 {
            (zero_label_style, theme.muted(), zero_savings_style)
        } else {
            (label_style, count_style, savings_style)
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(label, lab_s),
            Span::raw(" ".repeat(label_pad.max(1))),
            Span::styled(count_str, cnt_s),
            Span::raw(" ".repeat(count_pad.max(3))),
            Span::styled(savings_str, sav_s),
        ]));
    }

    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled(format!("────────────{}", rule), rule_style),
    ]));

    f.render_widget(Paragraph::new(lines), area);
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

    #[test]
    fn render_summary_band_emits_all_three_rows_even_when_empty() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 5);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        let summary = [
            (FindingKind::ToolRatio, 0usize, 0.0f64),
            (FindingKind::CacheEfficiency, 0, 0.0),
            (FindingKind::ModelMismatch, 0, 0.0),
        ];
        term.draw(|f| {
            let area = f.area();
            render_summary_band(f, area, summary, &theme);
        })
        .unwrap();
        let buf = term.backend().buffer();
        let dump: String = (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(dump.contains("tool-ratio"), "tool-ratio row missing: {dump}");
        assert!(
            dump.contains("cache-efficiency"),
            "cache-efficiency row missing: {dump}"
        );
        assert!(
            dump.contains("model-mismatch"),
            "model-mismatch row missing: {dump}"
        );
        assert!(dump.contains("0 findings"), "zero-findings text missing");
        assert!(dump.contains("$0.00"), "zero-savings text missing");
    }

    #[test]
    fn render_summary_band_shows_positive_savings_when_flagged() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 5);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        let summary = [
            (FindingKind::ToolRatio, 2usize, 3.40f64),
            (FindingKind::CacheEfficiency, 6, 0.28),
            (FindingKind::ModelMismatch, 0, 0.0),
        ];
        term.draw(|f| {
            let area = f.area();
            render_summary_band(f, area, summary, &theme);
        })
        .unwrap();
        let buf = term.backend().buffer();
        let dump: String = (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(dump.contains("2 findings"), "tool-ratio count missing");
        assert!(dump.contains("$3.40"), "tool-ratio savings missing");
        assert!(dump.contains("6 findings"), "cache-efficiency count missing");
        assert!(dump.contains("$0.28"), "cache-efficiency savings missing");
    }
}
