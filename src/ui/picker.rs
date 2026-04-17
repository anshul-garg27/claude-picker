//! Top-level picker screen render.
//!
//! Dispatches on `App::mode` — either the session-list two-pane layout (the
//! main event) or the project-list one-pane layout (shown when no project is
//! selected yet). Delegates the heavy lifting to the per-pane modules.
//!
//! A terminal-too-small short-circuit lives here as well so widgets never
//! receive a `Rect` they can't draw into.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, Mode, Toast};
use crate::theme::{Theme, ThemeName};
use crate::ui::filter_ribbon::FilterRibbon;
use crate::ui::task_drawer::{self, DRAWER_HEIGHT};
use crate::ui::text::{display_width, truncate_to_width};
use crate::ui::{
    command_palette, footer, help_overlay, layout, preview, project_list, rename_modal,
    session_list,
};

pub fn render(f: &mut Frame<'_>, app: &mut App) {
    let full_area = f.area();

    if layout::too_small(full_area) {
        render_too_small(f, full_area, &app.theme);
        return;
    }

    // Reserve the bottom slice for the background-task drawer when visible.
    // The main content (viewer / picker) and all overlays (toast, modals)
    // render into `area`; the drawer owns `drawer_area` and is painted last.
    let (area, drawer_area) = if app.task_drawer.visible
        && full_area.height > DRAWER_HEIGHT
    {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(DRAWER_HEIGHT)])
            .split(full_area);
        (chunks[0], Some(chunks[1]))
    } else {
        (full_area, None)
    };

    // Conversation viewer takes over the whole frame when open — render it
    // first so toasts still layer on top, but skip the underlying picker
    // draws to avoid flicker through the Clear widget.
    if app.viewer.is_some() {
        let theme = app.theme;
        if let Some(viewer) = app.viewer.as_mut() {
            crate::ui::conversation_viewer::render(f, area, viewer, &theme);
        }
        if let Some(toast) = &app.toast {
            render_toast(f, area, toast, &app.theme);
        }
        if let Some(drawer_area) = drawer_area {
            render_task_drawer(f, drawer_area, app);
        }
        return;
    }

    match app.mode {
        Mode::SessionList => render_session_screen(f, area, app),
        Mode::ProjectList => render_project_screen(f, area, app),
    }

    // Which-key overlay pops up between the main content and the modal stack.
    // `should_show_which_key` already enforces the post-pause debounce, so we
    // can render unconditionally here once the getter says it's time.
    if app.should_show_which_key() {
        if let Some(leader) = app.which_key_leader() {
            crate::ui::which_key::render(f, area, leader, &app.theme);
        }
    }

    // Toast / modal overlays render on top of everything. Z-order matters:
    // help/rename/delete come above toasts so they're never obscured.
    if let Some(toast) = &app.toast {
        render_toast(f, area, toast, &app.theme);
    }
    if app.show_delete_confirm {
        render_delete_confirm(f, area, &app.theme);
    }
    if let Some(rename) = &app.rename {
        rename_modal::render(f, area, rename, &app.theme);
    }
    if let Some(palette) = &app.palette {
        command_palette::render(f, area, palette, &app.theme);
    }
    if app.show_help {
        let content = help_overlay::help_for(app.help_screen());
        help_overlay::render(f, area, content, &app.theme);
    }

    // Task drawer sits pinned to the bottom, outside the overlay stack so
    // modals above never shift underneath it and so the user can monitor
    // background work while a toast or palette is up.
    if let Some(drawer_area) = drawer_area {
        render_task_drawer(f, drawer_area, app);
    }
}

/// Paint the background-task drawer into `area`. Splits the single mutable
/// borrow of `App` into the specific fields the widget needs (`task_drawer`
/// mut, `task_queue` locked, `theme` shared) so the compiler is happy.
fn render_task_drawer(f: &mut Frame<'_>, area: Rect, app: &mut App) {
    // Cloning the theme is cheap (Copy-ish — it's just palette indices and
    // a ThemeName tag) and sidesteps the "can't borrow app.theme while
    // app.task_drawer is mutably borrowed" conflict.
    let theme = app.theme;
    let Ok(queue) = app.task_queue.lock() else {
        // Poisoned mutex — another thread panicked while holding it. Skip
        // the drawer this frame rather than crashing the UI.
        return;
    };
    // `&*queue` drops us from `MutexGuard<TaskQueue>` to `&TaskQueue`, which
    // is the reference shape `task_drawer::render` asks for.
    task_drawer::render(f, area, &mut app.task_drawer, &*queue, &theme);
}

fn render_session_screen(f: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = layout::main_picker(area);
    render_title_bar(f, chunks.title_bar, app);

    // Carve out a single row above the session list for the filter ribbon.
    // The ribbon hides itself when the terminal is too narrow (< 80 cols);
    // we unconditionally reserve the 1-row slot to keep layout stable.
    let list_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(chunks.list_pane);
    render_filter_ribbon(f, list_rows[0], app);
    session_list::render(f, list_rows[1], app);

    preview::render(f, chunks.preview_pane, app);
    footer::render_session_list_with_multi(
        f,
        chunks.footer,
        &app.theme,
        app.multi_selected_count(),
        app.multi_mode,
        app.pending_count_value(),
        app.jump_ring_position(),
    );
}

/// Render the filter ribbon into `area`. Uses the live ribbon on `App` when
/// the integrator has wired `App::filter_ribbon()`; otherwise falls back to
/// a transient default so the UI never panics while the wiring is in flight.
fn render_filter_ribbon(f: &mut Frame<'_>, area: Rect, app: &App) {
    // The ribbon widget carries zero mutable state during render, so we can
    // pull it off `App` by reference or synthesise a default. Integrator
    // wires `app.filter_ribbon()` to return &FilterRibbon.
    let ribbon = try_get_ribbon(app);
    let fallback;
    let ribbon_ref: &FilterRibbon = match ribbon {
        Some(r) => r,
        None => {
            fallback = FilterRibbon::default();
            &fallback
        }
    };
    let buf = f.buffer_mut();
    ribbon_ref.render(area, buf, &app.theme);
}

/// Integration shim — same pattern as `project_list::try_get_pinned`. Now
/// that `App::filter_ribbon()` exists we return the live reference; the
/// shim layer stays to keep the rendering call sites uncoupled from the
/// exact accessor name.
fn try_get_ribbon(app: &App) -> Option<&FilterRibbon> {
    Some(app.filter_ribbon())
}

fn render_project_screen(f: &mut Frame<'_>, area: Rect, app: &App) {
    let (title, body, footer_area) = layout::project_picker(area);
    render_title_bar(f, title, app);
    project_list::render(f, body, app);
    footer::render_project_list(f, footer_area, &app.theme);
}

fn render_title_bar(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    // Middle-dot (·) separates peer-level metadata inside the sticky
    // header. We previously used › for hierarchy too, but the contextual
    // header flattens into peer segments at every drill-in level.
    const MID: &str = " \u{00B7} ";

    // Build the contextual segments first as plain strings; we style them
    // once we've confirmed the line fits. This also keeps truncation
    // centralised: we measure `display_width` of the assembled plain text
    // and back off segments from the right until it fits.
    let mut segments: Vec<(String, Style)> = Vec::new();

    // Header identity — differs per drill-in level.
    match app.mode {
        Mode::SessionList => {
            let project_name = app
                .active_project()
                .map(|p| p.name.as_str())
                .unwrap_or("local");

            if let Some(viewer) = app.viewer.as_ref() {
                // Deepest drill-in: `<project>/<session-id> · turn N/total ·
                // <model> · $<cost>`. No `claude-picker` prefix — the user
                // knows the tool they're in; they want session context.
                let (turn, total, model, cost) = viewer_context(viewer, app);
                let id = app
                    .selected_session_ref()
                    .map(|s| s.id.as_str())
                    .unwrap_or("-");
                let id_short = truncate_to_width(id, 8);
                segments.push((
                    format!(" {project_name}/{id_short}"),
                    Style::default()
                        .fg(theme.mauve)
                        .add_modifier(Modifier::BOLD),
                ));
                segments.push((MID.to_string(), theme.dim()));
                segments.push((format!("turn {turn}/{total}"), theme.muted()));
                if !model.is_empty() {
                    segments.push((MID.to_string(), theme.dim()));
                    segments.push((model, theme.muted()));
                }
                segments.push((MID.to_string(), theme.dim()));
                segments.push((format!("${cost:.2}"), theme.muted()));
            } else {
                // Session-list drill-in: `<project> · N sessions · scope:X`.
                segments.push((
                    format!(" {project_name} "),
                    Style::default()
                        .fg(theme.mauve)
                        .add_modifier(Modifier::BOLD),
                ));
                segments.push((MID.to_string(), theme.dim()));
                let count = app.sessions.len();
                let count_label = if count == 1 {
                    "1 session".to_string()
                } else {
                    format!("{count} sessions")
                };
                segments.push((count_label, theme.muted()));
                if let Some(scope) = active_scope_label(app) {
                    segments.push((MID.to_string(), theme.dim()));
                    segments.push((format!("scope:{scope}"), theme.muted()));
                }
            }
        }
        Mode::ProjectList => {
            // Top level: `claude-picker · N projects · scope:X`.
            segments.push((
                " claude-picker ".to_string(),
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ));
            segments.push((MID.to_string(), theme.dim()));
            let count = app.projects.len();
            let count_label = if count == 1 {
                "1 project".to_string()
            } else {
                format!("{count} projects")
            };
            segments.push((count_label, theme.muted()));
            if let Some(scope) = active_scope_label(app) {
                segments.push((MID.to_string(), theme.dim()));
                segments.push((format!("scope:{scope}"), theme.muted()));
            }
        }
    }

    // Theme name — tail position, drops first when we're tight on width.
    if theme.name != ThemeName::default() {
        segments.push((MID.to_string(), theme.dim()));
        segments.push((theme.name.label().to_string(), theme.muted()));
    }

    // Truncate segment-wise to fit. The available budget is `area.width` —
    // we reserve zero trailing columns because a 1-row Paragraph simply
    // overflows the frame edge on ratatui; column-exact is the safe move.
    let budget = area.width as usize;
    let spans = fit_to_width(segments, budget, theme);
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Shrink `segments` left-to-right until the combined `display_width` fits
/// within `budget`. We first drop tail segments (theme name, then scope,
/// then count), and finally fall back to hard-truncating the last segment
/// with `…`. Return the styled spans ready to render.
fn fit_to_width<'a>(
    mut segments: Vec<(String, Style)>,
    budget: usize,
    _theme: &Theme,
) -> Vec<Span<'a>> {
    // Greedy shrink from the right.
    while total_width(&segments) > budget && segments.len() > 1 {
        segments.pop();
    }
    if segments.is_empty() {
        return Vec::new();
    }
    // If the single remaining segment is still too wide, hard-truncate it.
    if total_width(&segments) > budget {
        let mut last = segments.pop().unwrap();
        last.0 = truncate_to_width(&last.0, budget);
        segments.push(last);
    }
    segments
        .into_iter()
        .map(|(text, style)| Span::styled(text, style))
        .collect()
}

fn total_width(segments: &[(String, Style)]) -> usize {
    segments.iter().map(|(s, _)| display_width(s)).sum()
}

/// Resolve the current filter-scope label (`ALL`, `REPO`, …) lower-cased for
/// the sticky header. Returns `None` until the integrator wires the ribbon
/// accessor.
fn active_scope_label(app: &App) -> Option<String> {
    let ribbon = try_get_ribbon(app)?;
    Some(ribbon.scope().label().to_ascii_lowercase())
}

/// Pull (turn_index, total_turns, model, cost) from the live viewer + its
/// session. Values fall through to sensible placeholders when the data
/// layer doesn't expose a particular field — we never render a jagged
/// header just because one number is missing.
fn viewer_context(
    _viewer: &crate::ui::conversation_viewer::ViewerState,
    app: &App,
) -> (usize, usize, String, f64) {
    let (total, model, cost) = app
        .selected_session_ref()
        .map(|s| {
            (
                s.message_count as usize,
                s.model_summary.clone(),
                s.total_cost_usd,
            )
        })
        .unwrap_or((0, String::new(), 0.0));
    // The viewer tracks its own cursor — we don't have a stable accessor
    // here, so surface the total count in both positions until the field
    // is wired. Integrator can swap in `viewer.turn_index()` once the
    // getter exists on ViewerState.
    (total, total, model, cost)
}

fn render_too_small(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let p = Paragraph::new(vec![
        Line::raw(""),
        Line::styled(
            "Terminal too small.",
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            format!(
                "Resize to at least {}×{} (current {}×{}).",
                layout::MIN_WIDTH,
                layout::MIN_HEIGHT,
                area.width,
                area.height
            ),
            theme.muted(),
        ),
    ])
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(p, area);
}

fn render_toast(f: &mut Frame<'_>, area: Rect, toast: &Toast, theme: &Theme) {
    // Max width for a fully-settled toast. The slide-in animation scales the
    // actual width from ~40% → 100% of this over the first 200 ms; the
    // fade-out simultaneously mixes both the border and foreground toward
    // the terminal's base colour over the final 300 ms.
    let full_w = 52_u16.min(area.width.saturating_sub(4));
    let h = 3_u16;

    // Interpolation factor:
    //   scale: 0.0 → 1.0 over the first 200 ms of toast life (unless
    //          animations are disabled).
    //   fade:  0.0 while visible, 0.0 → 1.0 over the final 300 ms.
    let anim_disabled = crate::theme::animations_disabled();
    let scale = if anim_disabled {
        1.0
    } else {
        toast.slide_in_progress()
    };
    let fade = if anim_disabled {
        0.0
    } else {
        toast.fade_out_progress()
    };

    // Width clamps to 40 % so the toast is still readable at the earliest
    // frame of the slide-in.
    let w = {
        let min = (full_w as f32 * 0.40).round() as u16;
        let target = min + ((full_w - min) as f32 * scale).round() as u16;
        target.max(min).min(full_w)
    };
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(h))
        .saturating_sub(4);
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    // Clear the underlying cells so the toast is opaque. The fade effect
    // biases the fg colours — the clear stays fully opaque so the body
    // doesn't show through.
    f.render_widget(Clear, rect);

    let (accent, label) = match toast.kind {
        crate::app::ToastKind::Info => (theme.mauve, "info"),
        crate::app::ToastKind::Success => (theme.green, "done"),
        crate::app::ToastKind::Error => (theme.red, "error"),
    };
    // Mix foreground colours toward `theme.base` as the fade factor climbs
    // — at t=1.0 the toast should look like it dissolved into the panel.
    let border_fg = crate::theme::lerp_color(accent, theme.base, fade);
    let title_fg = crate::theme::lerp_color(accent, theme.base, fade);
    let body_fg = crate::theme::lerp_color(theme.text, theme.base, fade);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_fg))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                label,
                Style::default().fg(title_fg).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));
    let p = Paragraph::new(Line::from(Span::styled(
        format!(" {} ", toast.message),
        Style::default().fg(body_fg),
    )))
    .block(block)
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(p, rect);
}

fn render_delete_confirm(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let w = 60_u16.min(area.width.saturating_sub(4));
    let h = 7_u16;
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.red))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "delete session",
                Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));

    let p = Paragraph::new(vec![
        Line::raw(""),
        Line::styled(
            "This will permanently delete the .jsonl file.",
            theme.muted(),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled("y", theme.key_hint()),
            Span::styled(" confirm    ", theme.key_desc()),
            Span::styled("Esc", theme.key_hint()),
            Span::styled(" cancel", theme.key_desc()),
        ]),
    ])
    .block(block)
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(p, rect);
}
