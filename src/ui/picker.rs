//! Top-level picker screen render.
//!
//! Dispatches on `App::mode` — either the session-list two-pane layout (the
//! main event) or the project-list one-pane layout (shown when no project is
//! selected yet). Delegates the heavy lifting to the per-pane modules.
//!
//! A terminal-too-small short-circuit lives here as well so widgets never
//! receive a `Rect` they can't draw into.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, Mode, Toast};
use crate::theme::{Theme, ThemeName};
use crate::ui::{
    command_palette, footer, help_overlay, layout, preview, project_list, rename_modal,
    session_list,
};

pub fn render(f: &mut Frame<'_>, app: &mut App) {
    let area = f.area();

    if layout::too_small(area) {
        render_too_small(f, area, &app.theme);
        return;
    }

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
        return;
    }

    match app.mode {
        Mode::SessionList => render_session_screen(f, area, app),
        Mode::ProjectList => render_project_screen(f, area, app),
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
}

fn render_session_screen(f: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = layout::main_picker(area);
    render_title_bar(f, chunks.title_bar, app);
    session_list::render(f, chunks.list_pane, app);
    preview::render(f, chunks.preview_pane, app);
    footer::render_session_list_with_multi(
        f,
        chunks.footer,
        &app.theme,
        app.multi_selected_count(),
        app.multi_mode,
    );
}

fn render_project_screen(f: &mut Frame<'_>, area: Rect, app: &App) {
    let (title, body, footer_area) = layout::project_picker(area);
    render_title_bar(f, title, app);
    project_list::render(f, body, app);
    footer::render_project_list(f, footer_area, &app.theme);
}

fn render_title_bar(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let mut spans = vec![
        Span::styled(
            " claude-picker ",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("·", theme.dim()),
        Span::raw(" "),
    ];
    match app.mode {
        Mode::SessionList => {
            let project_name = app
                .active_project()
                .map(|p| p.name.as_str())
                .unwrap_or("local");
            spans.push(Span::styled(project_name.to_string(), theme.subtle()));
        }
        Mode::ProjectList => {
            spans.push(Span::styled("all projects", theme.subtle()));
        }
    }
    // Subtly append the theme name when it's not the default. Muted so it
    // doesn't compete with the main title, but legible enough that a user
    // who pressed `t` by accident can confirm what they're looking at.
    if theme.name != ThemeName::default() {
        spans.push(Span::styled(" · ", theme.dim()));
        spans.push(Span::styled(theme.name.label().to_string(), theme.muted()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
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
    // Center a ~40-wide, 3-tall box two rows above the bottom.
    let w = 52_u16.min(area.width.saturating_sub(4));
    let h = 3_u16;
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

    // Clear the underlying cells so the toast is opaque.
    f.render_widget(Clear, rect);

    let (accent, label) = match toast.kind {
        crate::app::ToastKind::Info => (theme.mauve, "info"),
        crate::app::ToastKind::Success => (theme.green, "done"),
        crate::app::ToastKind::Error => (theme.red, "error"),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                label,
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));
    let p = Paragraph::new(Line::from(Span::styled(
        format!(" {} ", toast.message),
        theme.body(),
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
