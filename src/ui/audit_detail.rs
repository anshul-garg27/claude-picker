//! Drill-in detail overlay for the cost audit (#16).
//!
//! Rendered when the user hits `Enter` on a `ToolRatio` finding in the main
//! audit list. Shows which specific tools dominated that session's output so
//! "72% of your output was Bash" goes from an abstract ratio to a concrete
//! line item the user can act on.
//!
//! Layout (rough sketch):
//!
//! ```text
//!   ← back    ▌ data-pipeline / Optimize Redshift COPY command    $76.85
//!
//!   tool                         calls    out tokens    cost-attribution
//!   ─────────────────────────────────────────────────────────────────────
//!   Bash                            8      1.36M        $34.00  (72%)
//!   Read                            3      240.0k       $6.00   (13%)
//!   …
//!
//!   haiku projection:  $7.80  (save $28.60)
//! ```
//!
//! Kept in its own module so `audit.rs` stays focused on the list view —
//! this screen has its own empty-state, its own column maths, and will
//! likely grow more columns as the heuristics mature.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::data::pricing::{haiku_output_ratio_to, output_rate_for};
use crate::data::tool_dist::ToolDistEntry;
use crate::theme::Theme;
use crate::ui::audit::AuditDetailView;

/// Render the drill-in overlay into `area`. The caller (`audit::render`)
/// places this below the summary band so the audit chrome stays visible.
pub fn render(f: &mut Frame<'_>, area: Rect, view: &AuditDetailView, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border())
        .title(title_line(view, theme));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 3 || inner.width < 40 {
        let msg = Paragraph::new(Line::from(Span::styled(
            "  terminal too narrow to show tool distribution",
            theme.muted(),
        )));
        f.render_widget(msg, inner);
        return;
    }

    let width = inner.width as usize;
    let mut lines: Vec<Line<'_>> = Vec::with_capacity((view.tool_dist.len() + 6).max(8));

    lines.push(Line::raw(""));
    lines.push(header_row(width, theme));
    lines.push(rule_row(width, theme));

    if view.tool_dist.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("no tool_use blocks found in this session", theme.muted()),
        ]));
    } else {
        let total_output: u64 = view.tool_dist.iter().map(|e| e.usage.output_tokens).sum();
        let output_rate = output_rate_for(&view.finding.model_summary);
        for entry in &view.tool_dist {
            lines.push(tool_row(entry, total_output, output_rate, width, theme));
        }
    }

    lines.push(Line::raw(""));
    lines.push(projection_row(view, theme));
    lines.push(Line::raw(""));
    lines.push(footer_hint(theme));

    let body: Vec<Line<'_>> = lines.into_iter().take(inner.height as usize).collect();
    f.render_widget(Paragraph::new(body), inner);
}

fn title_line<'a>(view: &'a AuditDetailView, theme: &Theme) -> Line<'a> {
    let cost = view.finding.total_cost_usd;
    let cost_str = if cost < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${:.2}", cost)
    };
    Line::from(vec![
        Span::raw(" "),
        Span::styled("\u{2190} back", theme.key_hint()),
        Span::raw("   "),
        Span::styled(
            "\u{258C} ",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "{} / {}",
                view.finding.project_name, view.finding.session_label
            ),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            cost_str,
            Style::default()
                .fg(theme.peach)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
}

fn header_row<'a>(width: usize, theme: &Theme) -> Line<'a> {
    let (tool_w, calls_w, tokens_w, cost_w) = column_widths(width);
    Line::from(vec![
        Span::raw("  "),
        Span::styled(format!("{:<width$}", "tool", width = tool_w), theme.muted()),
        Span::styled(
            format!("{:>width$}", "calls", width = calls_w),
            theme.muted(),
        ),
        Span::styled(
            format!("{:>width$}", "out tokens", width = tokens_w),
            theme.muted(),
        ),
        Span::styled(
            format!("{:>width$}", "cost-attribution", width = cost_w),
            theme.muted(),
        ),
    ])
}

fn rule_row<'a>(width: usize, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled("\u{2500}".repeat(width.saturating_sub(4)), theme.dim()),
    ])
}

fn tool_row<'a>(
    entry: &'a ToolDistEntry,
    total_output: u64,
    output_rate: f64,
    width: usize,
    theme: &Theme,
) -> Line<'a> {
    let (tool_w, calls_w, tokens_w, cost_w) = column_widths(width);
    let pct = if total_output > 0 {
        (entry.usage.output_tokens as f64 / total_output as f64) * 100.0
    } else {
        0.0
    };
    let attributed_usd = entry.usage.output_tokens as f64 * output_rate;
    let cost_frag = if attributed_usd < 0.01 {
        format!("<$0.01  ({:.0}%)", pct)
    } else {
        format!("${:.2}  ({:.0}%)", attributed_usd, pct)
    };

    // Highlight the dominant tool: when it accounts for at least half of
    // the session's tool output, it's the one the user should actually
    // consider downgrading.
    let is_top = total_output > 0 && entry.usage.output_tokens * 2 >= total_output;
    let name_style = if is_top {
        Style::default()
            .fg(theme.peach)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtext1)
    };
    let cost_style = if is_top {
        Style::default()
            .fg(theme.cost_amber)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtext1)
    };

    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{:<width$}", truncate_to(&entry.name, tool_w), width = tool_w),
            name_style,
        ),
        Span::styled(
            format!("{:>width$}", entry.usage.call_count, width = calls_w),
            Style::default().fg(theme.mauve),
        ),
        Span::styled(
            format!(
                "{:>width$}",
                format_token_count(entry.usage.output_tokens),
                width = tokens_w
            ),
            Style::default().fg(theme.subtext1),
        ),
        Span::styled(format!("{:>width$}", cost_frag, width = cost_w), cost_style),
    ])
}

fn projection_row<'a>(view: &'a AuditDetailView, theme: &Theme) -> Line<'a> {
    let total_output: u64 = view.tool_dist.iter().map(|e| e.usage.output_tokens).sum();
    let output_rate = output_rate_for(&view.finding.model_summary);
    let current_cost = total_output as f64 * output_rate;
    let haiku_ratio = haiku_output_ratio_to(&view.finding.model_summary);
    let haiku_cost = current_cost * haiku_ratio;
    let savings = (current_cost - haiku_cost).max(0.0);
    Line::from(vec![
        Span::raw("  "),
        Span::styled("haiku projection:", theme.muted()),
        Span::raw("  "),
        Span::styled(
            format!("${:.2}", haiku_cost),
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("(save ${:.2})", savings),
            Style::default()
                .fg(theme.cost_amber)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn footer_hint<'a>(theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled("Enter", theme.key_hint()),
        Span::raw(" "),
        Span::styled("back to findings", theme.key_desc()),
        Span::styled("   \u{00B7}   ", theme.dim()),
        Span::styled("q / Esc", theme.key_hint()),
        Span::raw(" "),
        Span::styled("exit audit", theme.key_desc()),
    ])
}

fn column_widths(width: usize) -> (usize, usize, usize, usize) {
    let calls_w = 7usize;
    let tokens_w = 14usize;
    let cost_w = 22usize;
    let used = calls_w + tokens_w + cost_w;
    let tool_w = width.saturating_sub(used + 4).max(10);
    (tool_w, calls_w, tokens_w, cost_w)
}

fn truncate_to(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "\u{2026}".to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

fn format_token_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else if n >= 1_000 {
        format!("{:.2}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::cost_audit::{AuditFinding, Finding, FindingKind, Severity};
    use crate::data::tool_dist::ToolUsage;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn mk_view(tool_dist: Vec<ToolDistEntry>) -> AuditDetailView {
        AuditDetailView {
            finding: AuditFinding {
                session_id: "sess-1".into(),
                project_name: "data-pipeline".into(),
                project_cwd: std::path::PathBuf::from("/tmp/data-pipeline"),
                session_label: "Optimize Redshift COPY command".into(),
                total_cost_usd: 76.85,
                model_summary: "claude-opus-4-7".into(),
                findings: vec![Finding {
                    severity: Severity::Warn,
                    kind: FindingKind::ToolRatio,
                    message: "72% tool_use tokens".into(),
                    savings_usd: 28.60,
                }],
                estimated_savings_usd: 28.60,
            },
            tool_dist,
        }
    }

    fn render_to_string(view: &AuditDetailView) -> String {
        let backend = TestBackend::new(100, 18);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        term.draw(|f| {
            let area = f.area();
            render(f, area, view, &theme);
        })
        .unwrap();
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_rows_sorted_by_output_desc() {
        let view = mk_view(vec![
            ToolDistEntry {
                name: "Bash".into(),
                usage: ToolUsage {
                    call_count: 8,
                    output_tokens: 1_360_000,
                    input_tokens_after: 0,
                },
            },
            ToolDistEntry {
                name: "Read".into(),
                usage: ToolUsage {
                    call_count: 3,
                    output_tokens: 240_000,
                    input_tokens_after: 0,
                },
            },
            ToolDistEntry {
                name: "Grep".into(),
                usage: ToolUsage {
                    call_count: 2,
                    output_tokens: 120_000,
                    input_tokens_after: 0,
                },
            },
        ]);
        let dump = render_to_string(&view);
        let bash_idx = dump.find("Bash").expect("Bash present");
        let read_idx = dump.find("Read").expect("Read present");
        let grep_idx = dump.find("Grep").expect("Grep present");
        assert!(
            bash_idx < read_idx && read_idx < grep_idx,
            "expected Bash before Read before Grep in render:\n{dump}"
        );
        assert!(dump.contains("1.36M"), "Bash token count missing");
        assert!(
            dump.contains("Optimize Redshift COPY command"),
            "session label missing from title"
        );
    }

    #[test]
    fn renders_haiku_projection_savings() {
        // Opus 4.7 → Haiku 4.5: output ratio 5/25 = 0.20. With 1M output
        // tokens at $25/M the current cost is $25.00; Haiku ≈ $5.00 so the
        // savings column should read ~$20.00.
        let view = mk_view(vec![ToolDistEntry {
            name: "Bash".into(),
            usage: ToolUsage {
                call_count: 1,
                output_tokens: 1_000_000,
                input_tokens_after: 0,
            },
        }]);
        let dump = render_to_string(&view);
        assert!(
            dump.contains("haiku projection:"),
            "haiku projection label missing: {dump}"
        );
        assert!(dump.contains("$5.00"), "haiku cost $5.00 missing: {dump}");
        assert!(dump.contains("save $20.00"), "savings $20.00 missing: {dump}");
    }

    #[test]
    fn empty_tool_dist_shows_placeholder() {
        let view = mk_view(vec![]);
        let dump = render_to_string(&view);
        assert!(
            dump.contains("no tool_use blocks found"),
            "placeholder copy missing when tool_dist is empty: {dump}"
        );
    }

    #[test]
    fn title_displays_finding_cost() {
        let view = mk_view(vec![]);
        let dump = render_to_string(&view);
        assert!(dump.contains("$76.85"), "finding cost missing from title");
        assert!(dump.contains("data-pipeline"), "project name missing");
    }
}
