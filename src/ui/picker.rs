//! Top-level picker screen render.
//!
//! Dispatches on `App::mode` — either the session-list two-pane layout (the
//! main event) or the project-list one-pane layout (shown when no project is
//! selected yet). Delegates the heavy lifting to the per-pane modules.
//!
//! A terminal-too-small short-circuit lives here as well so widgets never
//! receive a `Rect` they can't draw into.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use tachyonfx::{fx, Effect, Shader};

use crate::app::{App, Mode, Toast};
use crate::theme::{self, Theme, ThemeName};
use crate::ui::filter_ribbon::FilterRibbon;
use crate::ui::fx as ui_fx;
use crate::ui::task_drawer::{self, DRAWER_HEIGHT};
use crate::ui::text::{display_width, truncate_to_width};
use crate::ui::{
    command_palette, footer, help_overlay, layout, preview, project_list, rename_modal,
    session_list,
};

// ── F5: Peek mode ────────────────────────────────────────────────────────
//
// Press-and-hold Space over a session row → floating 60×20 preview of the
// last 20 turns fades in; release → fades out. Terminals do NOT distinguish
// keydown vs keyup for Space — you just get repeated `KeyDown` events as
// long as the key is pressed — so we approximate "release" via a dead-man
// timer: after [`PEEK_RELEASE_IDLE_MS`] with no Space event, consider the
// key released.
//
// The Space-Space (double-tap within 200 ms) chord already exists as the
// palette leader on `app.pending_chord`; peek must NOT trigger in that
// window. `handle_space_event` enforces this gate explicitly.

/// Time Space must be held continuously before the peek preview slides in.
/// 200 ms matches the brief and is long enough to not collide with the
/// Space-Space palette leader chord.
pub const PEEK_HOLD_THRESHOLD_MS: u64 = 200;

/// Idle window after the last Space event before we consider the key
/// released. Terminals repeat Space at ~30–50 ms intervals when held, so
/// 300 ms is well beyond the repeat floor.
pub const PEEK_RELEASE_IDLE_MS: u64 = 300;

/// Floating preview size. 60 × 20 matches the brief.
pub const PEEK_OVERLAY_WIDTH: u16 = 60;
pub const PEEK_OVERLAY_HEIGHT: u16 = 20;

/// F5 animation + bookkeeping state. One instance lives in a thread-local
/// because, per the file-ownership rules for this patch, the renderer only
/// has `&App` / `&mut App` and I'm forbidden from editing `src/app.rs` to
/// add the `space_held_since` / `peek_visible` fields. See the integration
/// spec at the bottom of the module — once those fields exist on `App`,
/// swap `PEEK_STATE.with(...)` for `app.space_held_since` / `app.peek_visible`.
pub struct PeekState {
    /// `Some(instant)` while Space has been seen at least once without a
    /// [`PEEK_RELEASE_IDLE_MS`] idle gap. Cleared on release. The brief
    /// names this `space_held_since` — kept here so the naming matches
    /// when the field moves to `App`.
    pub space_held_since: Option<Instant>,
    /// Timestamp of the most recent Space event — drives the idle-timer
    /// release detection.
    pub last_space_at: Option<Instant>,
    /// True after the hold threshold fires and before release.
    pub peek_visible: bool,
    /// In-flight fade-in / slide-in stack for the peek overlay. Cleared
    /// once [`Effect::done`] fires.
    pub enter_effect: Option<Effect>,
    /// In-flight fade-out / slide-out stack. Set on release, cleared on
    /// completion.
    pub exit_effect: Option<Effect>,
    /// Session id the peek is showing — captured when we transitioned to
    /// visible so a mid-hold cursor change doesn't race the preview.
    pub pinned_session_id: Option<String>,
}

impl PeekState {
    fn new() -> Self {
        Self {
            space_held_since: None,
            last_space_at: None,
            peek_visible: false,
            enter_effect: None,
            exit_effect: None,
            pinned_session_id: None,
        }
    }
}

thread_local! {
    static PEEK_STATE: RefCell<PeekState> = RefCell::new(PeekState::new());
}

/// Public entry point for the event loop. Call once for every Space-key
/// event the app sees BEFORE the existing Space-Space palette handler
/// runs: the palette chord is detected by `app.pending_chord`, which
/// already fires on the *second* Space within 200 ms. The peek timer
/// starts on the *first* Space; once the second lands within the Space-
/// Space window, we abort the peek (the palette wins).
///
/// Returns `true` when the peek machine handled the event (so the caller
/// should skip Space's default toggle-play / palette-leader dispatch
/// *only* when the peek is visible — otherwise both paths coexist).
///
/// NOTE: this function is a stub that records state and returns. It does
/// NOT actually fire until the integrator wires it into
/// `App::handle_event` on the session-list mode. See the integration spec.
pub fn handle_space_event(app: &App, is_keydown: bool) -> bool {
    // The palette double-tap chord wins — we bail out immediately when
    // the user is mid-Space-Space. `pending_chord` is already populated
    // by `App::handle_event` for the first Space in the window.
    if let Some((_, started)) = app.pending_chord {
        if started.elapsed() < Duration::from_millis(PEEK_HOLD_THRESHOLD_MS) {
            return false;
        }
    }

    PEEK_STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        let now = Instant::now();
        if is_keydown {
            if st.space_held_since.is_none() {
                st.space_held_since = Some(now);
            }
            st.last_space_at = Some(now);
        }
    });
    false
}

/// Drive the peek timer every tick — promotes `space_held_since` into
/// `peek_visible` after the hold threshold, and releases when the idle
/// window expires. Integrators wire this next to the existing per-tick
/// handlers on the session-list screen.
pub fn tick_peek(app: &App) {
    PEEK_STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        let now = Instant::now();

        // Release on idle: Space events stopped flowing long enough to
        // treat the key as released.
        let idle_expired = st
            .last_space_at
            .map(|t| now.saturating_duration_since(t) >= Duration::from_millis(PEEK_RELEASE_IDLE_MS))
            .unwrap_or(true);
        if st.space_held_since.is_some() && idle_expired {
            st.space_held_since = None;
            st.last_space_at = None;
            if st.peek_visible {
                // Trigger the exit animation.
                st.peek_visible = false;
                st.pinned_session_id = None;
                let reduce_motion = theme::animations_disabled();
                st.exit_effect = ui_fx::build(reduce_motion, || {
                    fx::parallel(&[
                        ui_fx::fade_out(app.theme.subtext0, app.theme.base, 120),
                        ui_fx::slide_out_downward(app.theme.base, 120),
                    ])
                });
                st.enter_effect = None;
            }
        }

        // Hold threshold reached — promote to visible.
        if let Some(since) = st.space_held_since {
            if !st.peek_visible
                && now.saturating_duration_since(since)
                    >= Duration::from_millis(PEEK_HOLD_THRESHOLD_MS)
            {
                st.peek_visible = true;
                st.pinned_session_id = app
                    .selected_session_ref()
                    .map(|s| s.id.clone());
                let reduce_motion = theme::animations_disabled();
                st.enter_effect = ui_fx::build(reduce_motion, || {
                    fx::parallel(&[
                        ui_fx::fade_in(app.theme.text, app.theme.base, 140),
                        ui_fx::slide_in_from_below(app.theme.base, 140),
                    ])
                });
                st.exit_effect = None;
            }
        }
    });
}

/// Render the peek overlay — call after the main picker paint so the
/// overlay sits on top. No-op when the peek isn't visible and no exit
/// animation is mid-flight.
fn render_peek_overlay(f: &mut Frame<'_>, area: Rect, app: &App) {
    PEEK_STATE.with(|cell| {
        let mut st = cell.borrow_mut();
        let is_visible_or_exiting = st.peek_visible || st.exit_effect.is_some();
        if !is_visible_or_exiting {
            return;
        }

        // Size the overlay — degrade gracefully on panes narrower than
        // the spec'd 60×20.
        let w = PEEK_OVERLAY_WIDTH.min(area.width.saturating_sub(4));
        let h = PEEK_OVERLAY_HEIGHT.min(area.height.saturating_sub(4));
        if w < 20 || h < 5 {
            return;
        }
        let rect = Rect {
            x: area.x + area.width.saturating_sub(w) / 2,
            y: area.y + area.height.saturating_sub(h) / 2,
            width: w,
            height: h,
        };

        f.render_widget(Clear, rect);
        let theme = &app.theme;
        let session_label = st
            .pinned_session_id
            .as_ref()
            .and_then(|id| app.sessions.iter().find(|s| &s.id == id))
            .map(|s| s.display_label().to_string())
            .unwrap_or_else(|| "(no session)".to_string());

        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "peek ",
                Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD),
            ),
            Span::styled(session_label.clone(), theme.muted()),
            Span::raw(" "),
        ]);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.mauve))
            .title(title);
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Body: placeholder text. The brief specs "last 20 turns" — a real
        // transcript render lives in ui::conversation_viewer and pulling
        // it here without plumbing through the transcript loader would
        // exceed the file-ownership of this patch. Leave a visibly
        // intentional placeholder so the overlay is obviously live
        // during manual testing; the integrator swaps in the 20-turn
        // renderer when wiring `app.peek_visible` onto `App`.
        let body = vec![
            Line::raw(""),
            Line::styled(
                "  last 20 turns",
                Style::default()
                    .fg(theme.subtext1)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            Line::styled(
                format!("  session: {session_label}"),
                theme.muted(),
            ),
            Line::raw(""),
            Line::styled(
                "  (hold Space to keep open, release to dismiss)",
                theme.muted(),
            ),
        ];
        f.render_widget(Paragraph::new(body), inner);

        // Drive the enter effect — runs once from invisible → visible.
        // We grab `buffer_mut()` fresh for each `process` call so the
        // reborrow lifetimes stay tidy; each `process` call starts and
        // ends a distinct `&mut Buffer` borrow.
        let now = Instant::now();
        let enter_delta =
            ui_fx::delta_from(now.saturating_duration_since(st.last_space_at.unwrap_or(now)));
        if let Some(effect) = st.enter_effect.as_mut() {
            let buf = f.buffer_mut();
            effect.process(enter_delta, buf, rect);
            if effect.done() {
                st.enter_effect = None;
            }
        }
        if let Some(effect) = st.exit_effect.as_mut() {
            let buf = f.buffer_mut();
            effect.process(ui_fx::delta_from(Duration::from_millis(16)), buf, rect);
            if effect.done() {
                st.exit_effect = None;
            }
        }
    });
}

/// Test-only helper so unit tests can peek inside the thread-local state
/// without exposing its internals.
#[cfg(test)]
pub fn peek_state_visible_for_test() -> bool {
    PEEK_STATE.with(|c| c.borrow().peek_visible)
}

/// Reset the F5 thread-local between tests. Only exposed in `cfg(test)`.
#[cfg(test)]
pub fn reset_peek_state_for_test() {
    PEEK_STATE.with(|c| *c.borrow_mut() = PeekState::new());
}

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

    // F5 peek-mode overlay sits between the modal stack and the task
    // drawer — modals should still clobber the preview when opened, but
    // the task drawer stays underneath. `render_peek_overlay` is a no-op
    // when the peek isn't visible and no exit animation is in flight.
    render_peek_overlay(f, area, app);

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
    task_drawer::render(f, area, &mut app.task_drawer, &queue, &theme);
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

#[cfg(test)]
mod peek_tests {
    use super::*;

    #[test]
    fn peek_visible_is_false_by_default() {
        reset_peek_state_for_test();
        assert!(!peek_state_visible_for_test());
    }

    #[test]
    fn peek_thresholds_match_brief() {
        // The brief specifies 200 ms hold before the preview appears and
        // 300 ms idle before the preview hides. Pin those values — they
        // are part of the UX contract and must not drift silently.
        assert_eq!(PEEK_HOLD_THRESHOLD_MS, 200);
        assert_eq!(PEEK_RELEASE_IDLE_MS, 300);
        // The release timer must be longer than terminal Space repeat
        // intervals (~30–50 ms) and longer than the hold threshold so a
        // brief tap does not immediately release during the same key-
        // repeat cadence.
        assert!(PEEK_RELEASE_IDLE_MS > PEEK_HOLD_THRESHOLD_MS);
    }

    #[test]
    fn peek_overlay_dimensions_match_brief() {
        assert_eq!(PEEK_OVERLAY_WIDTH, 60);
        assert_eq!(PEEK_OVERLAY_HEIGHT, 20);
    }
}

// ─── F5 integration spec ─────────────────────────────────────────────────
//
// The peek-mode state machine currently lives in a thread-local because
// this patch's file-ownership forbids editing `src/app.rs`. The brief
// explicitly flags this as an integration point, so the spec below is
// the blocking work before F5 is "live":
//
//   1. Add two fields to `App` in `src/app.rs`, next to the existing
//      `pending_chord`:
//
//          pub space_held_since: Option<Instant>,
//          pub peek_visible: bool,
//
//      Initialise both to `None` / `false` in `App::new_with_theme`.
//
//   2. In `App::handle_event`, before the existing Space dispatch,
//      forward the key to the peek machine:
//
//          Event::Key(' ') => {
//              crate::ui::picker::handle_space_event(self, true);
//              // existing Space handling continues…
//          }
//
//      Optionally gate the existing Space-Space palette leader detection
//      so a held Space doesn't try to chord with itself — `pending_chord`
//      already self-expires at 200 ms, which happens to be the same as
//      `PEEK_HOLD_THRESHOLD_MS`, so the defaults line up.
//
//   3. In the app's per-tick path (the `App::tick` that already advances
//      toasts / replay), call `crate::ui::picker::tick_peek(self);`.
//      That's what promotes the hold to `peek_visible = true` and
//      detects the idle release.
//
//   4. Once the fields exist on `App`, replace `PEEK_STATE.with(...)`
//      calls in this module with reads/writes against `app.space_held_since`
//      and `app.peek_visible`. The current thread-local is a drop-in
//      stand-in that shares the same field names for minimal diff.
//
//   5. Reduce-motion: the module currently gates via
//      `theme::animations_disabled()`. When `app.config.ui.reduce_motion`
//      is plumbed (see F3 integration notes), OR the two flags.
//
// The integrator owns fields on `App`; this module owns the timer logic,
// the animations, and the overlay rendering.
