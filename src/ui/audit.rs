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

use crate::data::cost_audit::{
    summary_by_kind, total_potential_savings, AuditFinding, FindingKind, Severity,
};
use crate::data::tool_dist::{collect_tool_distribution, find_session_jsonl, ToolDistEntry};
use crate::theme::Theme;
use crate::ui::audit_detail;

/// Multiplier applied to current-window total savings to produce an annual
/// projection. 365 / 30 ≈ 12.17 — treats the audit window as a "monthly" slice
/// even though the actual window is the user's whole session corpus. The
/// figure is deliberately a rough run-rate, not a forecast.
const ANNUAL_RUN_RATE_MULTIPLIER: f64 = 12.17;

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
/// 1. Summary band (5 rows: top rule + 3 heuristic rows + bottom rule; grows
///    to 6 when a positive savings total lets us tack on the annual run-rate
///    line) — always present so every run visibly proves all three
///    heuristics ran.
/// 2. Findings list — the scrollable per-session findings (height capped at
///    its natural content so the by-project band below always has room).
/// 3. By-project band — horizontal cost bars per project so the lower half
///    of the screen surfaces "where is the money going" instead of going
///    black when there are only a handful of findings.
/// 4. Footer — key hints + total savings (2 rows).
pub fn render(f: &mut Frame<'_>, area: Rect, state: &mut AuditState, theme: &Theme) {
    let summary = summary_by_kind(&state.findings);
    let total_savings_usd = total_potential_savings(&state.findings);
    let show_annual = total_savings_usd > 0.0;
    // Summary is 5 rows by default (top rule + 3 heuristic rows + bottom
    // rule). Add one more row for the "annual run-rate" beat when we have
    // something to project.
    let summary_height: u16 = if show_annual { 6 } else { 5 };

    // Drill-in branch (#16) — when the detail overlay is up we hand the
    // whole body (findings + by-project band) to the overlay renderer so
    // per-tool bars have room to breathe. Summary + footer stay put so the
    // "back via Enter" hint in the footer remains visible.
    if state.detail.is_some() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(summary_height),
                Constraint::Min(4),
                Constraint::Length(2),
            ])
            .split(area);
        render_summary_band(f, chunks[0], summary, total_savings_usd, theme);
        if let Some(detail) = state.detail.as_ref() {
            audit_detail::render(f, chunks[1], detail, theme);
        }
        render_footer(f, chunks[2], state, theme);
        return;
    }

    // Natural height the findings list wants inside its border: 1 header +
    // N detail rows per finding + blank lines between blocks. Used to cap
    // the findings panel so the by-project band always gets room.
    let findings_inner_lines = findings_body_line_count(&state.findings);
    let findings_natural = findings_inner_lines.saturating_add(2).max(4) as u16;
    // Reserve at least ~10 rows for the by-project band when the terminal is
    // tall enough; on tiny terminals we let it shrink gracefully.
    const RESERVED_FOR_PROJECTS: u16 = 10;
    let available_body = area
        .height
        .saturating_sub(summary_height)
        .saturating_sub(2);
    let findings_cap = available_body.saturating_sub(RESERVED_FOR_PROJECTS).max(4);
    let findings_height = findings_natural.min(findings_cap).max(4);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(summary_height),
            Constraint::Length(findings_height),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(area);
    render_summary_band(f, chunks[0], summary, total_savings_usd, theme);
    render_body(f, chunks[1], state, theme);
    render_by_project_band(f, chunks[2], &state.findings, theme);
    render_footer(f, chunks[3], state, theme);
}

/// Sum the number of lines the findings list will emit inside its border —
/// used to size the findings panel so the by-project band always gets room
/// below it. Matches the layout produced by [`render_finding_into`]: 1
/// header + N detail rows per finding, with a blank line between blocks.
fn findings_body_line_count(findings: &[AuditFinding]) -> usize {
    if findings.is_empty() {
        // Empty-state paragraph is ~5 lines; reserve that so the message
        // doesn't get clipped.
        return 5;
    }
    let mut total = 0usize;
    for (i, f) in findings.iter().enumerate() {
        total += 1 + f.findings.len();
        if i + 1 < findings.len() {
            total += 1;
        }
    }
    total
}

/// Top-of-screen three-row band that always shows every heuristic category,
/// even when zero findings — the point is to make the "three heuristics"
/// promise visible on every audit run. When `total_savings_usd > 0` a final
/// "annual run-rate" line is appended as the closing visual beat. Shape:
///
/// ```text
/// ─── summary ───────────────────────────
///   tool-ratio         2 findings   $3.40
///   cache-efficiency   6 findings   $0.28
///   model-mismatch     0 findings   $0.00
///   annual run-rate    ×12.17       ~$44.83 avoidable/year
/// ───────────────────────────────────────
/// ```
fn render_summary_band(
    f: &mut Frame<'_>,
    area: Rect,
    summary: [(FindingKind, usize, f64); 3],
    total_savings_usd: f64,
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

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(6);
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

    // Annual projection — the big "this adds up" reveal. Only rendered when
    // there is something to project; a zero line would just be noise.
    if total_savings_usd > 0.0 {
        let annual = total_savings_usd * ANNUAL_RUN_RATE_MULTIPLIER;
        let label = "annual run-rate";
        let multiplier_str = format!("\u{00D7}{:.2}", ANNUAL_RUN_RATE_MULTIPLIER);
        let annual_str = format!("~${:.2} avoidable/year", annual);
        let label_pad = 18usize.saturating_sub(label.chars().count());
        let mult_pad = 14usize.saturating_sub(multiplier_str.chars().count());
        let annual_style = Style::default()
            .fg(theme.cost_amber)
            .add_modifier(Modifier::BOLD);
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(label, Style::default().fg(theme.subtext1)),
            Span::raw(" ".repeat(label_pad.max(1))),
            Span::styled(multiplier_str, theme.muted()),
            Span::raw(" ".repeat(mult_pad.max(3))),
            Span::styled(annual_str, annual_style),
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
        // Friendlier empty state than the old stderr fallback — keeps the
        // audit screen looking intentional when the user has nothing to fix.
        let msg = Paragraph::new(vec![
            Line::raw(""),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "\u{2713} nothing to audit \u{2014} every session looks efficient",
                    Style::default()
                        .fg(theme.green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::raw(""),
            Line::styled(
                "  all three heuristics ran and found no red flags.",
                theme.body(),
            ),
            Line::styled("  run more sessions and try again later.", theme.dim()),
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

/// One row of the by-project band: project label, a horizontal bar sized
/// proportionally to the project's total cost, and a right-aligned dollar
/// total. When the project has an `avoidable` slice (i.e. any finding fired
/// against it in this audit) we overlay that slice in [`Theme::cost_red`] so
/// the user sees the "money bleeding" portion of each bar at a glance.
#[derive(Debug, Clone)]
struct ProjectBarRow {
    name: String,
    total_cost_usd: f64,
    avoidable_usd: f64,
}

/// Collapse [`AuditFinding`] rows by project for the by-project band.
///
/// We deliberately aggregate only from `state.findings` instead of walking
/// the whole corpus again: the findings list already covers every session
/// with any heuristic hit, and doing a second disk pass here would double
/// the audit startup cost for a view that's meant to feel instant. Projects
/// with zero findings just won't appear; the panel title makes that clear.
fn per_project_rows(findings: &[AuditFinding]) -> Vec<ProjectBarRow> {
    use std::collections::HashMap;
    let mut acc: HashMap<String, (f64, f64)> = HashMap::new();
    for f in findings {
        let slot = acc.entry(f.project_name.clone()).or_insert((0.0, 0.0));
        slot.0 += f.total_cost_usd;
        slot.1 += f.estimated_savings_usd;
    }
    let mut rows: Vec<ProjectBarRow> = acc
        .into_iter()
        .map(|(name, (total, avoidable))| ProjectBarRow {
            name,
            total_cost_usd: total,
            avoidable_usd: avoidable,
        })
        .collect();
    // Biggest-spend first — matches the findings list, which is sorted by
    // savings desc. Tie-break on project name for deterministic ordering.
    rows.sort_by(|a, b| {
        b.total_cost_usd
            .partial_cmp(&a.total_cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

/// Build one row's line: `  name  ████████░░░░░░  $X.XX   avoidable $Y.YY`.
/// `max_total` is the largest total in the set — the longest bar gets the
/// full `bar_width`, the rest are scaled proportionally. `bar_width` is the
/// number of cells available for the bar itself (already excludes the label
/// column and the right-hand numeric columns).
fn render_project_bar_line<'a>(
    row: &'a ProjectBarRow,
    max_total: f64,
    label_col: usize,
    bar_width: usize,
    theme: &Theme,
) -> Line<'a> {
    let fraction = if max_total > 0.0 {
        (row.total_cost_usd / max_total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let fill_cells = ((fraction * bar_width as f64).round() as usize).min(bar_width);
    // Avoidable overlay is a sub-slice of the fill — clamp so rounding
    // never pushes the red past the main bar.
    let avoidable_frac = if row.total_cost_usd > 0.0 {
        (row.avoidable_usd / row.total_cost_usd).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let red_cells = ((avoidable_frac * fill_cells as f64).round() as usize).min(fill_cells);
    let main_cells = fill_cells.saturating_sub(red_cells);
    let empty_cells = bar_width.saturating_sub(fill_cells);

    let name_display: String = if row.name.chars().count() > label_col {
        let mut s: String = row.name.chars().take(label_col.saturating_sub(1)).collect();
        s.push('\u{2026}'); // ellipsis
        s
    } else {
        row.name.clone()
    };
    let name_pad = label_col.saturating_sub(name_display.chars().count());

    let bar_main_style = Style::default().fg(theme.model_opus);
    let bar_red_style = Style::default()
        .fg(theme.cost_red)
        .add_modifier(Modifier::BOLD);
    let bar_empty_style = theme.dim();
    let cost_style = if row.total_cost_usd >= 50.0 {
        Style::default()
            .fg(theme.cost_red)
            .add_modifier(Modifier::BOLD)
    } else if row.total_cost_usd >= 10.0 {
        Style::default().fg(theme.cost_amber)
    } else {
        Style::default().fg(theme.subtext1)
    };
    let cost_str = if row.total_cost_usd < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${:.2}", row.total_cost_usd)
    };

    let mut spans: Vec<Span<'a>> = Vec::with_capacity(9);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        name_display,
        Style::default().fg(theme.subtext1),
    ));
    spans.push(Span::raw(" ".repeat(name_pad.max(1))));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        "\u{2588}".repeat(main_cells),
        bar_main_style,
    ));
    spans.push(Span::styled("\u{2588}".repeat(red_cells), bar_red_style));
    spans.push(Span::styled(
        "\u{2591}".repeat(empty_cells),
        bar_empty_style,
    ));
    spans.push(Span::raw("  "));
    // Right-hand numeric columns. Fixed-width-ish so rows align.
    spans.push(Span::styled(format!("{:>9}", cost_str), cost_style));
    if row.avoidable_usd > 0.0 {
        let avoid_str = format!(" avoidable ${:.2}", row.avoidable_usd);
        spans.push(Span::styled(
            avoid_str,
            Style::default()
                .fg(theme.cost_red)
                .add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

/// Bottom-half band that fills the space under the findings list with a
/// per-project cost bar chart. Title rule matches the summary band so the
/// two feel like siblings. Empty case renders a gentle "nothing to show"
/// line instead of blank space.
fn render_by_project_band(
    f: &mut Frame<'_>,
    area: Rect,
    findings: &[AuditFinding],
    theme: &Theme,
) {
    if area.height < 3 {
        return;
    }
    let width = area.width as usize;
    let rule = "─".repeat(width.saturating_sub(16));
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(area.height as usize);
    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled(format!("─── by project {}", rule), theme.dim()),
    ]));

    let rows = per_project_rows(findings);
    if rows.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "no per-project spend to chart yet",
                theme.muted(),
            ),
        ]));
        f.render_widget(Paragraph::new(lines), area);
        return;
    }

    // Layout: label column ~ 22 chars, numeric right column ~ 28 chars,
    // bar fills the middle. Clamp so tiny terminals still render something.
    let label_col: usize = 22.min(width.saturating_sub(34).max(12));
    let right_col: usize = 28;
    // `label_col` + 2 (gap) + bar + 2 (gap) + right_col ≤ width
    let bar_width: usize = width
        .saturating_sub(label_col + right_col + 6)
        .max(8);
    let max_total = rows
        .iter()
        .map(|r| r.total_cost_usd)
        .fold(0.0f64, f64::max);

    // Drop the rule line + anything that wouldn't fit; area.height - 1 for
    // the rule leaves room for this many data rows.
    let row_budget = (area.height as usize).saturating_sub(1);
    for row in rows.iter().take(row_budget) {
        lines.push(render_project_bar_line(
            row, max_total, label_col, bar_width, theme,
        ));
    }

    f.render_widget(Paragraph::new(lines), area);
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
            render_summary_band(f, area, summary, 0.0, &theme);
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
        // Annual-rate line must NOT appear when total savings are zero —
        // it would just print "$0.00 avoidable/year", pure noise.
        assert!(
            !dump.contains("annual run-rate"),
            "annual run-rate should be hidden when total savings = 0: {dump}"
        );
    }

    #[test]
    fn render_summary_band_shows_positive_savings_when_flagged() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 6);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        let summary = [
            (FindingKind::ToolRatio, 2usize, 3.40f64),
            (FindingKind::CacheEfficiency, 6, 0.28),
            (FindingKind::ModelMismatch, 0, 0.0),
        ];
        term.draw(|f| {
            let area = f.area();
            render_summary_band(f, area, summary, 3.68, &theme);
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

    #[test]
    fn render_summary_band_includes_annual_projection_when_savings_positive() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 6);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        let summary = [
            (FindingKind::ToolRatio, 1usize, 64.00f64),
            (FindingKind::CacheEfficiency, 0, 0.0),
            (FindingKind::ModelMismatch, 0, 0.0),
        ];
        let total = 64.00_f64;
        term.draw(|f| {
            let area = f.area();
            render_summary_band(f, area, summary, total, &theme);
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
        assert!(
            dump.contains("annual run-rate"),
            "annual run-rate label missing: {dump}"
        );
        // 64.00 × 12.17 = 778.88 — formatted to 2dp.
        let expected = format!("~${:.2} avoidable/year", total * ANNUAL_RUN_RATE_MULTIPLIER);
        assert!(
            dump.contains(&expected),
            "expected annual figure {expected} missing: {dump}"
        );
    }

    #[test]
    fn per_project_rows_aggregates_and_sorts_descending() {
        let findings = vec![
            AuditFinding {
                session_id: "a".into(),
                project_name: "small-proj".into(),
                project_cwd: std::path::PathBuf::new(),
                session_label: "s".into(),
                total_cost_usd: 1.50,
                model_summary: "claude-opus-4-7".into(),
                findings: vec![],
                estimated_savings_usd: 0.50,
            },
            AuditFinding {
                session_id: "b".into(),
                project_name: "big-proj".into(),
                project_cwd: std::path::PathBuf::new(),
                session_label: "s".into(),
                total_cost_usd: 40.00,
                model_summary: "claude-opus-4-7".into(),
                findings: vec![],
                estimated_savings_usd: 10.00,
            },
            AuditFinding {
                session_id: "c".into(),
                project_name: "big-proj".into(),
                project_cwd: std::path::PathBuf::new(),
                session_label: "s".into(),
                total_cost_usd: 20.00,
                model_summary: "claude-opus-4-7".into(),
                findings: vec![],
                estimated_savings_usd: 5.00,
            },
        ];
        let rows = per_project_rows(&findings);
        assert_eq!(rows.len(), 2);
        // big-proj aggregates 40 + 20 = 60, comes first.
        assert_eq!(rows[0].name, "big-proj");
        assert!((rows[0].total_cost_usd - 60.00).abs() < 1e-9);
        assert!((rows[0].avoidable_usd - 15.00).abs() < 1e-9);
        assert_eq!(rows[1].name, "small-proj");
        assert!((rows[1].total_cost_usd - 1.50).abs() < 1e-9);
    }

    #[test]
    fn render_by_project_band_draws_rows_and_dollar_amounts() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let findings = vec![
            AuditFinding {
                session_id: "a".into(),
                project_name: "alpha".into(),
                project_cwd: std::path::PathBuf::new(),
                session_label: "s".into(),
                total_cost_usd: 203.17,
                model_summary: "claude-opus-4-7".into(),
                findings: vec![],
                estimated_savings_usd: 36.40,
            },
            AuditFinding {
                session_id: "b".into(),
                project_name: "beta".into(),
                project_cwd: std::path::PathBuf::new(),
                session_label: "s".into(),
                total_cost_usd: 12.50,
                model_summary: "claude-opus-4-7".into(),
                findings: vec![],
                estimated_savings_usd: 0.0,
            },
        ];

        let backend = TestBackend::new(100, 6);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        term.draw(|f| {
            let area = f.area();
            render_by_project_band(f, area, &findings, &theme);
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
        assert!(dump.contains("by project"), "title rule missing: {dump}");
        assert!(dump.contains("alpha"), "alpha project name missing: {dump}");
        assert!(dump.contains("$203.17"), "alpha cost missing: {dump}");
        assert!(
            dump.contains("avoidable $36.40"),
            "avoidable overlay label missing: {dump}"
        );
        assert!(dump.contains("beta"), "beta project name missing: {dump}");
        assert!(dump.contains("$12.50"), "beta cost missing: {dump}");
    }

    #[test]
    fn render_by_project_band_empty_state_does_not_panic() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 4);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        term.draw(|f| {
            let area = f.area();
            render_by_project_band(f, area, &[], &theme);
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
        assert!(
            dump.contains("by project"),
            "title should still render on empty state: {dump}"
        );
    }

    #[test]
    fn findings_body_line_count_matches_rendered_layout() {
        let a = mk_finding("a", 1.0, 0.5);
        let b = mk_finding("b", 2.0, 1.0);
        // Each finding: 1 header + 1 detail = 2 lines. Two findings plus 1
        // blank between = 5 lines total.
        assert_eq!(findings_body_line_count(&[a, b]), 5);
        // Empty → 5 (the reserved empty-state height).
        assert_eq!(findings_body_line_count(&[]), 5);
    }

    #[test]
    fn render_body_empty_state_includes_friendly_message() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(80, 8);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        let mut state = AuditState::new(vec![]);
        term.draw(|f| {
            let area = f.area();
            render_body(f, area, &mut state, &theme);
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
        assert!(
            dump.contains("nothing to audit"),
            "empty-state friendlier copy missing: {dump}"
        );
    }
}
