//! Always-visible breadcrumb at row 0.
//!
//! Every screen the picker owns (project list, session list, full-screen
//! conversation viewer) paints this one-line header before anything else so
//! the user always knows where they are in the hierarchy. Subcommand
//! screens (stats, audit, …) live outside this module's reach, but the
//! picker proper always shows it.
//!
//! The breadcrumb's content adapts to the active drill-in level:
//!
//! - **Project list:** `claude-picker · N projects · [SCOPE] · filter:"…"`
//! - **Session list:** `claude-picker › <project> (N) · [SCOPE] · filter:"…"`
//! - **Viewer:**       `claude-picker › viewer › <session title>`
//!
//! The viewer variant is deliberately compact — the viewer takes over the
//! whole frame and the surrounding screen already shows turn / cost / model
//! metadata on its own title bar inside the border. All we owe the user
//! here is "you are in the viewer looking at <session>".

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Mode};
use crate::theme::{Theme, ThemeName};
use crate::ui::text::{display_width, truncate_to_width};

/// Peer-level separator — middle dot (·) for metadata siblings.
const MID: &str = "  \u{00B7}  ";
/// Hierarchy-step separator — tail arrow (›) between breadcrumb levels.
const BREAD: &str = "  \u{203A}  ";

/// Render the breadcrumb into `area` (typically 1-row tall, sitting at
/// row 0 of the frame). Dispatches on whether a viewer is open — not on
/// picker mode alone — so the deepest drill-in wins.
pub fn render(f: &mut Frame<'_>, area: Rect, app: &App) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    if app.viewer.is_some() {
        render_viewer(f, area, app);
    } else {
        render_picker(f, area, app);
    }
}

fn render_viewer(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let title = app
        .selected_session_ref()
        .map(|s| s.display_label().to_string())
        .unwrap_or_else(|| "-".to_string());

    let mut segments: Vec<(String, Style)> = vec![
        (
            " claude-picker".to_string(),
            Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD),
        ),
        (BREAD.to_string(), theme.dim()),
        (
            "viewer".to_string(),
            Style::default().fg(theme.subtext1).add_modifier(Modifier::BOLD),
        ),
        (BREAD.to_string(), theme.dim()),
        (
            title,
            Style::default().fg(theme.peach).add_modifier(Modifier::BOLD),
        ),
    ];

    let budget = area.width as usize;
    let spans = fit_to_width(&mut segments, budget);
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_picker(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let mut segments: Vec<(String, Style)> = Vec::new();

    match app.mode {
        Mode::SessionList => {
            let project_name = app
                .active_project()
                .map(|p| p.name.as_str())
                .unwrap_or("local");

            segments.push((
                " claude-picker".to_string(),
                Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD),
            ));
            segments.push((BREAD.to_string(), theme.dim()));
            segments.push((
                project_name.to_string(),
                Style::default().fg(theme.peach).add_modifier(Modifier::BOLD),
            ));
            let count = app.sessions.len();
            segments.push((
                format!(" ({count})"),
                Style::default().fg(theme.subtext1).add_modifier(Modifier::BOLD),
            ));
            if let Some(chip) = scope_chip(app, theme) {
                segments.push((MID.to_string(), theme.dim()));
                segments.push(chip);
            }
            if let Some(filter_segs) = filter_expr_segments(app, theme) {
                segments.push((MID.to_string(), theme.dim()));
                segments.extend(filter_segs);
            }
        }
        Mode::ProjectList => {
            segments.push((
                " claude-picker".to_string(),
                Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD),
            ));
            segments.push((MID.to_string(), theme.dim()));
            let count = app.projects.len();
            let count_label = if count == 1 {
                "1 project".to_string()
            } else {
                format!("{count} projects")
            };
            segments.push((
                count_label,
                Style::default().fg(theme.subtext1).add_modifier(Modifier::BOLD),
            ));
            if let Some(chip) = scope_chip(app, theme) {
                segments.push((MID.to_string(), theme.dim()));
                segments.push(chip);
            }
            if let Some(filter_segs) = filter_expr_segments(app, theme) {
                segments.push((MID.to_string(), theme.dim()));
                segments.extend(filter_segs);
            }
        }
    }

    if theme.name != ThemeName::default() {
        segments.push((MID.to_string(), theme.dim()));
        segments.push((theme.name.label().to_string(), theme.muted()));
    }

    let budget = area.width as usize;
    let spans = fit_to_width(&mut segments, budget);
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn scope_chip(app: &App, theme: &Theme) -> Option<(String, Style)> {
    let label = app.filter_ribbon().scope().label().to_ascii_lowercase();
    let upper = label.to_ascii_uppercase();
    let style = Style::default()
        .bg(theme.mauve)
        .fg(theme.crust)
        .add_modifier(Modifier::BOLD);
    Some((format!("\u{258C}{upper}\u{2590}"), style))
}

fn filter_expr_segments(app: &App, theme: &Theme) -> Option<Vec<(String, Style)>> {
    if app.filter.is_empty() {
        return None;
    }
    let body = truncate_to_width(&app.filter, 32);
    Some(vec![
        ("filter:".to_string(), theme.muted()),
        (
            format!("\u{201C}{body}\u{201D}"),
            Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn fit_to_width<'a>(
    segments: &mut Vec<(String, Style)>,
    budget: usize,
) -> Vec<Span<'a>> {
    while total_width(segments) > budget && segments.len() > 1 {
        segments.pop();
    }
    if segments.is_empty() {
        return Vec::new();
    }
    if total_width(segments) > budget {
        let mut last = segments.pop().unwrap();
        last.0 = truncate_to_width(&last.0, budget);
        segments.push(last);
    }
    segments
        .iter()
        .map(|(text, style)| Span::styled(text.clone(), *style))
        .collect()
}

fn total_width(segments: &[(String, Style)]) -> usize {
    segments.iter().map(|(s, _)| display_width(s)).sum()
}
