//! "What if" model simulator modal (#5).
//!
//! Triggered from the session-list by pressing `m` (when the filter is empty),
//! this modal compares the selected session's cost under a handful of Claude
//! models using the session's actual token mix.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::data::pricing::{cost_for, TokenCounts};
use crate::data::Session;
use crate::theme::Theme;

pub const PROJECTION_MODELS: &[&str] = &[
    "claude-opus-4-7",
    "claude-opus-4-6",
    "claude-sonnet-4-5",
    "claude-haiku-4-5",
];

#[derive(Debug, Clone)]
pub struct ProjectionRow {
    pub model: String,
    pub cost: f64,
    pub delta_ratio: f64,
    pub savings: f64,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct ModelSimulatorState {
    pub title: String,
    pub current_model: String,
    pub current_cost: f64,
    pub tokens: TokenCounts,
    pub rows: Vec<ProjectionRow>,
}

impl ModelSimulatorState {
    pub fn from_session(session: &Session) -> Self {
        let current_model = session.model_summary.clone();
        let current_cost = cost_for(&current_model, session.tokens);
        let rows = Self::build_rows(&current_model, current_cost, session.tokens);
        Self {
            title: session.display_label().to_string(),
            current_model,
            current_cost,
            tokens: session.tokens,
            rows,
        }
    }

    pub fn build_rows(current_model: &str, current_cost: f64, tokens: TokenCounts) -> Vec<ProjectionRow> {
        let mut rows: Vec<ProjectionRow> = PROJECTION_MODELS
            .iter()
            .map(|m| {
                let cost = cost_for(m, tokens);
                let delta_ratio = if current_cost > 0.0 { (cost - current_cost) / current_cost } else { 0.0 };
                let savings = current_cost - cost;
                let is_current = model_matches_current(m, current_model);
                ProjectionRow { model: (*m).to_string(), cost, delta_ratio, savings, is_current }
            })
            .collect();
        rows.sort_by(|a, b| b.cost.partial_cmp(&a.cost).unwrap_or(std::cmp::Ordering::Equal));
        rows
    }
}

fn model_matches_current(row_model: &str, current_model: &str) -> bool {
    let n = row_model.len().min(current_model.len());
    row_model.get(..n) == current_model.get(..n) && n > 0
}

pub fn pretty_model_label(model: &str) -> String {
    let rest = match model.strip_prefix("claude-") {
        Some(r) => r,
        None => return model.to_string(),
    };
    let mut parts = rest.splitn(4, '-');
    let family = match parts.next() {
        Some(f) if !f.is_empty() => f,
        _ => return model.to_string(),
    };
    let major = parts.next().unwrap_or("");
    let minor = parts.next().unwrap_or("");
    let family_cap = capitalize(family);
    match (major.is_empty(), minor.is_empty()) {
        (false, false) => format!("{family_cap} {major}.{minor}"),
        (false, true) => format!("{family_cap} {major}"),
        _ => family_cap,
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &ModelSimulatorState, theme: &Theme) {
    let w = 66u16.min(area.width.saturating_sub(4));
    let projection_count = state.rows.iter().filter(|r| !r.is_current).count() as u16;
    let h = (projection_count + 10).min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    frame.render_widget(Clear, rect);
    let title_label = truncate_title(&state.title, w as usize);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled("What if ", Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD)),
            Span::styled("\u{00B7} ", theme.dim()),
            Span::styled(title_label, Style::default().fg(theme.peach).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
        ]));
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(h as usize);
    lines.push(Line::raw(""));
    let current_label = pretty_model_label(&state.current_model);
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("current: ", theme.muted()),
        Span::styled(format!("{current_label:<22}"), Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(format!("${:.2}", state.current_cost), Style::default().fg(theme.green).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::raw(""));
    for row in state.rows.iter().filter(|r| !r.is_current) {
        lines.push(build_projection_line(row, theme));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("(based on same token mix \u{2014} tools, cache, in/out)", theme.muted()),
    ]));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("press ", theme.muted()),
        Span::styled("q", theme.key_hint()),
        Span::styled(" to close, ", theme.key_desc()),
        Span::styled("r", theme.key_hint()),
        Span::styled(" to reset", theme.key_desc()),
    ]));
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, rect);
}

fn build_projection_line<'a>(row: &ProjectionRow, theme: &Theme) -> Line<'a> {
    let model_label = pretty_model_label(&row.model);
    let label_col = format!("at {model_label:<18}");
    let cost_col = format!("${:.2}", row.cost);
    let delta_segment = build_delta_segment(row, theme);
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(6);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(label_col, theme.subtle()));
    spans.push(Span::styled(format!("{cost_col:<10}"), Style::default().fg(theme.text).add_modifier(Modifier::BOLD)));
    spans.push(Span::raw(" "));
    spans.extend(delta_segment);
    Line::from(spans)
}

fn build_delta_segment<'a>(row: &ProjectionRow, theme: &Theme) -> Vec<Span<'a>> {
    if row.delta_ratio.abs() < 0.005 {
        return vec![Span::styled("(same rates, no change)".to_string(), theme.dim())];
    }
    let pct = (row.delta_ratio * 100.0).round() as i64;
    let pct_text = if pct < 0 { format!("\u{2212}{}%", pct.unsigned_abs()) } else { format!("+{pct}%") };
    let save_abs = row.savings.abs();
    let save_prefix = if row.savings > 0.0 { "save " } else { "add " };
    let save_text = format!("{save_prefix}${save_abs:.2}");
    let delta_color = if row.savings > 0.0 { theme.green } else { theme.red };
    vec![
        Span::styled("(".to_string(), theme.muted()),
        Span::styled(pct_text, Style::default().fg(delta_color).add_modifier(Modifier::BOLD)),
        Span::styled(", ".to_string(), theme.muted()),
        Span::styled(save_text, Style::default().fg(delta_color).add_modifier(Modifier::BOLD)),
        Span::styled(")".to_string(), theme.muted()),
    ]
}

fn truncate_title(title: &str, modal_w: usize) -> String {
    let budget = modal_w.saturating_sub(14);
    if title.chars().count() <= budget {
        return title.to_string();
    }
    let mut s: String = title.chars().take(budget.saturating_sub(1)).collect();
    s.push('\u{2026}');
    s
}

pub fn is_dismiss_key(c: char) -> bool {
    matches!(c, 'q' | 'Q')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_tokens() -> TokenCounts {
        TokenCounts { input: 500_000, output: 2_000_000, cache_read: 10_000_000, cache_write_5m: 1_000_000, cache_write_1h: 0 }
    }

    #[test]
    fn formats_all_model_rows_with_deltas() {
        let tokens = fixture_tokens();
        let current_cost = cost_for("claude-opus-4-7", tokens);
        assert!(current_cost > 0.0);
        let rows = ModelSimulatorState::build_rows("claude-opus-4-7", current_cost, tokens);
        assert_eq!(rows.len(), PROJECTION_MODELS.len());
        let current_rows: Vec<_> = rows.iter().filter(|r| r.is_current).collect();
        assert_eq!(current_rows.len(), 1);
        let sonnet = rows.iter().find(|r| r.model.contains("sonnet")).unwrap();
        assert!(sonnet.delta_ratio < 0.0);
        assert!(sonnet.savings > 0.0);
        let haiku = rows.iter().find(|r| r.model.contains("haiku")).unwrap();
        assert!(haiku.cost < sonnet.cost);
        assert!(haiku.savings > sonnet.savings);
        let opus_46 = rows.iter().find(|r| r.model == "claude-opus-4-6").unwrap();
        assert!(opus_46.delta_ratio.abs() < 1e-9);
        assert!(opus_46.savings.abs() < 1e-9);
    }

    #[test]
    fn shows_save_amount_in_cash_color() {
        let tokens = fixture_tokens();
        let current_cost = cost_for("claude-opus-4-7", tokens);
        let rows = ModelSimulatorState::build_rows("claude-opus-4-7", current_cost, tokens);
        let sonnet = rows.iter().find(|r| r.model.contains("sonnet")).unwrap();
        assert!(sonnet.savings > 0.0);
        let theme = Theme::mocha();
        let spans = build_delta_segment(sonnet, &theme);
        assert_eq!(spans.len(), 5);
        assert_eq!(spans[1].style.fg, Some(theme.green));
        assert_eq!(spans[3].style.fg, Some(theme.green));
        assert!(spans[1].content.as_ref().starts_with('\u{2212}'));
        assert!(spans[3].content.as_ref().starts_with("save "));
    }

    #[test]
    fn same_rates_row_reads_no_change() {
        let tokens = fixture_tokens();
        let current_cost = cost_for("claude-opus-4-7", tokens);
        let rows = ModelSimulatorState::build_rows("claude-opus-4-7", current_cost, tokens);
        let opus_46 = rows.iter().find(|r| r.model == "claude-opus-4-6").unwrap();
        let theme = Theme::mocha();
        let spans = build_delta_segment(opus_46, &theme);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].content.as_ref().contains("same rates"));
    }

    #[test]
    fn pretty_model_label_formats_known_families() {
        assert_eq!(pretty_model_label("claude-opus-4-7"), "Opus 4.7");
        assert_eq!(pretty_model_label("claude-sonnet-4-5"), "Sonnet 4.5");
        assert_eq!(pretty_model_label("claude-haiku-4-5"), "Haiku 4.5");
        assert_eq!(pretty_model_label("claude-opus-4-7-20260416"), "Opus 4.7");
        assert_eq!(pretty_model_label("weird-thing"), "weird-thing");
    }

    #[test]
    fn dismiss_key_matches_q_only() {
        assert!(is_dismiss_key('q'));
        assert!(is_dismiss_key('Q'));
        assert!(!is_dismiss_key('r'));
        assert!(!is_dismiss_key('m'));
    }
}
