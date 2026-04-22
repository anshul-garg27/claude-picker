//! Project selection screen — shown when the user runs `claude-picker` from
//! a directory with no Claude sessions, or when the session screen wants to
//! pop back to project-choice.
//!
//! Layout (top-down):
//!
//! 1. **Pinned strip** (k9s-style favorites): a numbered horizontal row of
//!    tiles across the top. Tiles show `N:name`, with the currently-active
//!    project (if any) highlighted in the theme accent. Pressing `1..9`
//!    jumps to that slot; `u` toggles a pin on the current project; `0`
//!    clears any project filter.
//! 2. **Filter input** — unchanged typed-filter across the project list.
//! 3. **Project list** — alphabetical-ish project rows.
//!
//! [`ProjectList`] holds the persistent state (pinned store + transient UI
//! toggles) so render can stay pure-data-in. Keybind handlers call the
//! mutating methods on `ProjectList` from `app.rs`.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget,
};
use ratatui::Frame;

use crate::app::App;
use crate::data::pinned_projects::{PinnedProjects, ToggleResult};
use crate::data::Project;
use crate::theme::{self, Theme};
use crate::ui::text::{display_width, truncate_to_width};
use crate::ui::thumbnail::{
    self, PinnedSlot, ThumbnailRenderer, MIN_STRIP_WIDTH as THUMB_MIN_STRIP_WIDTH,
    TILE_ROWS as THUMB_TILE_ROWS,
};

/// Minimum terminal width at which we render the full pinned strip. Below
/// this, only the active pin (if any) is shown so labels don't wrap.
const PINNED_STRIP_FULL_WIDTH: u16 = 60;

/// Stateful component backing the project-list screen. Today it carries:
///
/// - [`PinnedProjects`] — persistent slot store.
/// - The user-facing cursor into the pinned strip (None = on the list).
///
/// Construction is cheap (reads one small TOML file); defaulting to an empty
/// store is safe on CI and headless hosts where `$HOME` isn't set.
pub struct ProjectList {
    pinned: PinnedProjects,
    /// F2/E17 thumbnail renderer. Wrapped in `RefCell` because `App` only
    /// exposes `&ProjectList` and the renderer needs `&mut self` to bump
    /// its LRU. The borrow is held for one render pass so there's no risk
    /// of overlap.
    ///
    /// Lazily initialised on first access; construction is cheap now that
    /// the renderer no longer probes stdio for a graphics protocol.
    thumbnails: RefCell<Option<ThumbnailRenderer>>,
}

impl ProjectList {
    /// Load the pinned store from disk. Malformed files silently fall back
    /// to an empty store — pinned slots are nice-to-have, never load-bearing.
    pub fn load() -> Self {
        Self {
            pinned: PinnedProjects::load(),
            thumbnails: RefCell::new(None),
        }
    }

    /// Drop cached identicon images. Called by the wiring layer on theme
    /// switch so the next frame rebuilds tiles in the new palette. Safe
    /// to call before the renderer is initialised (no-op).
    pub fn invalidate_thumbnail_cache(&self) {
        if let Some(r) = self.thumbnails.borrow_mut().as_mut() {
            r.invalidate();
        }
    }

    /// Read-only access to the pinned store (for rendering / integration).
    pub fn pinned(&self) -> &PinnedProjects {
        &self.pinned
    }

    /// Toggle the pin for `project_cwd`. Persists immediately.
    ///
    /// The spec calls this `toggle_pin_current` but we take `cwd` explicitly
    /// so the struct stays decoupled from `App`. The wiring layer in
    /// `app.rs` resolves "current project" and forwards the cwd here.
    pub fn toggle_pin_current(&mut self, project_cwd: &str) -> ToggleResult {
        self.pinned.toggle(project_cwd)
    }

    /// Return the project cwd pinned at `slot` (1-indexed), if any. Returns
    /// `None` for an empty or out-of-range slot. The wiring layer uses the
    /// returned cwd to locate the matching project index in `App::projects`
    /// and updates `selected_project` / mode.
    ///
    /// Named per the spec (`jump_to_pinned`) even though the "jump" itself
    /// is performed by the caller — this method is the source of truth for
    /// *where* to jump.
    pub fn jump_to_pinned(&self, slot: u8) -> Option<String> {
        self.pinned.at_slot(slot).map(|s| s.to_string())
    }

    /// Return true when slot `slot` is occupied. Used by the caller to
    /// decide whether a `1..9` keypress is a no-op (empty slot) or should
    /// trigger navigation.
    pub fn has_pin(&self, slot: u8) -> bool {
        self.pinned.at_slot(slot).is_some()
    }

    /// Clear any project-scoped filter. Today this is a no-op because the
    /// filter_ribbon owns the REPO scope and filtering is driven by the
    /// ribbon's predicate — calling this should reset the ribbon's scope to
    /// `ALL` in the wiring layer. We keep the method on `ProjectList` per
    /// the spec so the call-site reads as one cohesive action.
    pub fn clear_project_filter(&mut self) {
        // Nothing to clear locally; the ribbon owns scope state. The
        // wiring layer pairs `clear_project_filter` with
        // `FilterRibbon::set_scope(All)`.
    }
}

impl Default for ProjectList {
    fn default() -> Self {
        Self::load()
    }
}

pub fn render(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border_active())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "claude-picker — projects",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_top(
            Line::from(Span::styled(
                format!(" {}/{} ", app.filtered_indices.len(), app.projects.len()),
                Style::default().fg(theme.subtext1),
            ))
            .right_aligned(),
        );

    let inner = block.inner(area);
    f.render_widget(block, area);

    // The pinned strip is taller when thumbnails are in play: each tile is
    // a 2-row-tall identicon inside a 1-row border on each side, totalling
    // 4 rows. When the strip is text-only (narrow terminal, no pins, or
    // legacy degraded path) we stay at 1 row. We pick the taller layout
    // whenever the terminal is wide enough AND there's at least one pin —
    // a "no pins, press u" hint is still one row.
    let wants_thumbnails = inner.width >= THUMB_MIN_STRIP_WIDTH
        && (1u8..=9).any(|s| app.project_list().pinned().at_slot(s).is_some());
    let strip_height = if wants_thumbnails {
        // 2 rows for the identicon + 1 row each for top/bottom border of
        // the surrounding block = 4. Matches the block drawn inside
        // `thumbnail::render_pinned_strip_with_thumbnails`.
        THUMB_TILE_ROWS + 2
    } else {
        1
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(strip_height),
            Constraint::Length(3), // filter input
            Constraint::Min(1),    // list body
        ])
        .split(inner);

    render_pinned_strip(f, chunks[0], app);
    render_filter(f, chunks[1], app);
    render_list(f, chunks[2], app);
}

/// Render the numbered pinned strip.
///
/// Layering:
/// - `area.width >= THUMB_MIN_STRIP_WIDTH` **and** there's at least one pin:
///   delegate to `thumbnail::render_pinned_strip_with_thumbnails`, which
///   renders each slot as `[N: identicon basename]` using the image
///   protocol or a halfblock fallback.
/// - `PINNED_STRIP_FULL_WIDTH <= area.width < THUMB_MIN_STRIP_WIDTH`: the
///   original text-only strip (`[0:all] [1:name] [2:(empty)] …`).
/// - Below `PINNED_STRIP_FULL_WIDTH`: collapse to just the active slot.
fn render_pinned_strip(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    // Safe access — whether `App::pinned_projects()` is wired yet or not we
    // render something. If the App doesn't expose a pinned store the strip
    // prints a placeholder inviting the user to press `u`.
    let maybe_pinned = try_get_pinned(app);

    let active_cwd = app
        .active_project()
        .map(|p| p.path.to_string_lossy().into_owned());

    // Thumbnail path: wide enough terminal + at least one pin.
    if area.width >= THUMB_MIN_STRIP_WIDTH {
        if let Some(pinned) = maybe_pinned {
            let any_pin = (1u8..=9).any(|s| pinned.at_slot(s).is_some());
            if any_pin {
                // Build the slot vec first so the `RefCell` borrow is scoped
                // tightly around the render call.
                let slots: Vec<(u8, String, bool)> = (1u8..=9)
                    .filter_map(|slot| {
                        let cwd = pinned.at_slot(slot)?;
                        let basename = thumbnail::basename_for_cwd(cwd);
                        let active = active_cwd.as_deref() == Some(cwd);
                        Some((slot, basename, active))
                    })
                    .collect();

                let pl = app.project_list();
                let mut cell = pl.thumbnails.borrow_mut();
                let renderer = cell.get_or_insert_with(ThumbnailRenderer::new);

                let slot_refs: Vec<PinnedSlot<'_>> = slots
                    .iter()
                    .map(|(slot, basename, active)| PinnedSlot {
                        slot: *slot,
                        basename: basename.as_str(),
                        is_active: *active,
                    })
                    .collect();

                thumbnail::render_pinned_strip_with_thumbnails(
                    f, area, &slot_refs, theme, renderer,
                );
                return;
            }
        }
        // Fall through to the text strip when no pins yet — keeps the
        // "press u to pin" hint visible and avoids a blank tall row.
    }

    // Narrow mode: only show the "current" slot if any, else the ALL chip.
    if area.width < PINNED_STRIP_FULL_WIDTH {
        let label = match (&maybe_pinned, &active_cwd) {
            (Some(pins), Some(cwd)) => pins
                .iter()
                .find_map(|(slot, c)| (c == cwd.as_str()).then(|| format!("{slot}:pinned")))
                .unwrap_or_else(|| "0:all".to_string()),
            _ => "0:all".to_string(),
        };
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                format!("[{label}]"),
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    // Full strip: `[0:all] [1:name] [2:(empty)] …`.
    let Some(pinned) = maybe_pinned else {
        // Fallback render when the integrator hasn't wired the pinned store
        // yet. Still shows the `0:all` tile + a hint so the UI isn't broken
        // while the wiring lands.
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "[0:all]",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("press u to pin the current project", theme.muted()),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    };

    let mut spans: Vec<Span<'_>> = Vec::with_capacity(22);
    spans.push(Span::raw(" "));

    // Slot 0 ("all") is always first.
    let zero_active = active_cwd.is_none();
    spans.push(tile_span(
        "0",
        "all",
        zero_active,
        /*pinned=*/ true,
        theme,
    ));

    // Slots 1..9: filled tiles get their basename, empties render as
    // "(empty)".
    for slot in 1..=9u8 {
        spans.push(Span::raw(" "));
        let cwd_opt = pinned.at_slot(slot);
        let (label, is_active, is_filled) = match cwd_opt {
            Some(cwd) => {
                let name = cwd
                    .rsplit('/')
                    .find(|s| !s.is_empty())
                    .unwrap_or("?")
                    .to_string();
                let active = active_cwd.as_deref() == Some(cwd);
                (name, active, true)
            }
            None => ("(empty)".to_string(), false, false),
        };
        // Cap each tile at a sensible width so nine tiles can fit on typical
        // terminals without wrapping.
        let label = truncate_to_width(&label, 14);
        spans.push(tile_span(
            &slot.to_string(),
            &label,
            is_active,
            is_filled,
            theme,
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Build one chip in the pinned strip. `is_active` is the "the user is
/// currently viewing this project" flag; `is_filled` is "this slot has a
/// project assigned at all" — empty slots render dim even when they happen
/// to sit under the cursor (which shouldn't ever happen, but stay safe).
fn tile_span<'a>(slot: &str, label: &str, is_active: bool, is_filled: bool, theme: &Theme) -> Span<'a> {
    let text = format!("[{slot}:{label}]");
    let style = if is_active && is_filled {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else if is_filled {
        Style::default().fg(theme.subtext1)
    } else {
        // Empty slots read dimmer than "present but inactive" tiles.
        Style::default().fg(theme.overlay0)
    };
    Span::styled(text, style)
}

/// Best-effort getter for the pinned store on `App`. Now that
/// `App::project_list()` exists we return the live reference; keeping the
/// shim in place lets the rest of this file stay agnostic of the accessor
/// name — a future rename is a one-line patch.
fn try_get_pinned(app: &App) -> Option<&PinnedProjects> {
    Some(app.project_list().pinned())
}

fn render_filter(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;
    let text: Line<'_> = if app.filter.is_empty() {
        Line::from(vec![
            Span::styled("> ", theme.muted()),
            Span::styled("type to filter projects…", theme.filter_placeholder()),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", theme.muted()),
            Span::styled(app.filter.clone(), theme.filter_text()),
            Span::styled(" ", Style::default().bg(theme.mauve).fg(theme.crust)),
        ])
    };

    // When the filter has content, promote the border to mauve so the user
    // sees at-a-glance that typing is landing in the filter. The active
    // filter also bumps up to Thick so weight matches the session-list
    // variant's visual language.
    let (border_color, border_type) = if !app.filter.is_empty() {
        (Style::default().fg(theme.mauve), BorderType::Thick)
    } else {
        (Style::default().fg(theme.surface1), BorderType::Rounded)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(border_color);

    f.render_widget(Paragraph::new(text).block(block), area);
}

// ── Strict column budget (project row) ────────────────────────────────────
//
// Same treatment as the session list: every column is a fixed rect that
// ratatui clips at the boundary. The old implementation concatenated spans
// in one loose `Line`, so CJK names or four-digit session counts crushed
// into whatever came next (the "weird heatmap-like block" the user saw was
// the session-pill's `▐` half-block butting up against the `▐` of a nearby
// run of cells — no column boundary enforcing breathing room).
//
//  Col 1  pin prefix `1:`   3 cols    peach-bold, blank otherwise
//  Col 2  name              28 cols   bold, truncated with `…`
//  Col 3  git branch        14 cols   `⌥ main`, empty otherwise
//  Col 4  session-count     14 cols   `▌16 sessions▐`
//  Col 5  flex spacer       flex      empty — pushes age to the right
//  Col 6  cost chip         10 cols   `▌$910▐`, blank when unavailable
//  Col 7  last-activity     8 cols    `14m ago`
//
// Breakpoints: width < 90 drops the branch column; width < 70 drops the
// cost chip entirely. Width < 40 keeps only name + activity.

const PIN_COL_WIDTH: usize = 3;
const NAME_COL_WIDTH: usize = 28;
const BRANCH_COL_WIDTH: usize = 14;
const SESSIONS_COL_WIDTH: usize = 14;
const COST_COL_WIDTH: usize = 10;
const ACTIVITY_COL_WIDTH: usize = 8;

/// Breakpoint plan for a single project row. Drives which columns collapse
/// to zero in the `Layout::horizontal` split.
#[derive(Debug, Clone, Copy)]
struct ProjectColumnPlan {
    show_pin: bool,
    show_branch: bool,
    show_sessions: bool,
    show_cost: bool,
    show_activity: bool,
}

impl ProjectColumnPlan {
    fn for_width(width: u16) -> Self {
        let w = width as usize;
        // Fixed-column budgets for the non-flex columns:
        //   pin (3) + name (28) + branch (14) + sessions (14) + cost (10) + activity (8) = 77
        // Each breakpoint keeps the budget ≤ w so the flex spacer never
        // collapses to a negative width (which would panic `Layout::split`).
        Self {
            show_pin: w >= 40,
            show_branch: w >= 90,     // +14 keeps the pin+name+sessions+activity visible
            show_sessions: w >= 54,   // 3+28+14+8 = 53, so ≥ 54 leaves flex room
            show_cost: w >= 70,       // +10 adds the cost chip when space allows
            show_activity: w >= 40,
        }
    }

    fn constraints(&self) -> [Constraint; 7] {
        [
            Constraint::Length(if self.show_pin { PIN_COL_WIDTH as u16 } else { 0 }),
            Constraint::Length(NAME_COL_WIDTH as u16),
            Constraint::Length(if self.show_branch { BRANCH_COL_WIDTH as u16 } else { 0 }),
            Constraint::Length(if self.show_sessions { SESSIONS_COL_WIDTH as u16 } else { 0 }),
            Constraint::Min(0),
            Constraint::Length(if self.show_cost { COST_COL_WIDTH as u16 } else { 0 }),
            Constraint::Length(if self.show_activity { ACTIVITY_COL_WIDTH as u16 } else { 0 }),
        ]
    }
}

fn render_list(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    // Cold-start skeleton: no projects loaded yet AND still within the
    // init window means the project enumerator is probably in flight.
    // Paint pulsing bars so the pane reads as "loading" rather than
    // "empty". See session_list::render_skeleton_rows for the sibling.
    if app.projects.is_empty() && is_within_skeleton_window(app.init_instant) {
        render_skeleton_rows(f, area, theme);
        return;
    }

    if app.projects.is_empty() {
        let glyph_style = Style::default()
            .fg(theme.surface2)
            .add_modifier(Modifier::BOLD);
        let primary = Style::default()
            .fg(theme.subtext1)
            .add_modifier(Modifier::BOLD);
        let secondary = Style::default()
            .fg(theme.overlay0)
            .add_modifier(Modifier::ITALIC);
        // Pad vertically so the glyph lands closer to the optical center.
        let pad = area.height.saturating_sub(6) / 2;
        let mut lines: Vec<Line<'_>> = Vec::with_capacity(pad as usize + 6);
        for _ in 0..pad {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(Span::styled("\u{25EF}", glyph_style)));
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled("no projects yet", primary)));
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "run `claude` in any directory to get started",
            secondary,
        )));
        let p = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
        return;
    }

    if area.height == 0 || area.width == 0 {
        return;
    }

    // Pre-compute pin-slot lookup so rows can prefix a `N:` badge in peach
    // bold when the project is pinned. The pinned store is a small map
    // (max 9 entries) so scanning it inside the row renderer would also be
    // cheap — we lift the HashMap here anyway so the row renderer is pure.
    let pinned = app.project_list().pinned();
    let slot_by_cwd: std::collections::HashMap<String, u8> = (1u8..=9)
        .filter_map(|slot| pinned.at_slot(slot).map(|cwd| (cwd.to_string(), slot)))
        .collect();

    let plan = ProjectColumnPlan::for_width(area.width);
    let visible_rows = area.height as usize;
    let total = app.filtered_indices.len();
    let cursor = app.cursor.min(total.saturating_sub(1));
    let _ = cursor; // retained for clarity — see session_list for rationale
    // Smooth-scroll aware anchor. Matches the semantics of the inline
    // `scroll_start` helper below, just with interpolated intermediate
    // frames when animations are enabled.
    let start = app.project_scroll_start(visible_rows, total);

    let buf = f.buffer_mut();
    for (offset, display_idx) in (start..total.min(start + visible_rows)).enumerate() {
        let idx = app.filtered_indices[display_idx];
        let p = &app.projects[idx];
        let is_sel = Some(display_idx) == app.cursor_position();
        let slot = slot_by_cwd
            .get(&p.path.to_string_lossy().into_owned())
            .copied();
        let row_area = Rect {
            x: area.x,
            y: area.y + offset as u16,
            width: area.width,
            height: 1,
        };
        render_row_into(buf, row_area, p, theme, is_sel, slot, &plan);
    }

    // Scrollbar on the right edge, only when the list overflows.
    if total > area.height as usize {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(Style::default().fg(theme.surface1))
            .thumb_style(Style::default().fg(theme.mauve));
        let mut sb_state = ScrollbarState::new(total)
            .position(app.cursor)
            .viewport_content_length(area.height as usize);
        f.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 0,
                horizontal: 0,
            }),
            &mut sb_state,
        );
    }
}

/// Viewport start index — same formula as `ratatui::widgets::List` so the
/// cursor never scrolls off-screen.
///
/// Retained as a local helper because the test suite exercises the pure
/// semantics directly; the live render path now goes through
/// [`App::project_scroll_start`] which wraps this formula in the smooth-
/// scroll interpolator.
#[allow(dead_code)]
fn scroll_start(selected: usize, visible_rows: usize, total: usize) -> usize {
    if visible_rows == 0 || total <= visible_rows {
        return 0;
    }
    if selected < visible_rows {
        0
    } else {
        selected + 1 - visible_rows
    }
}

/// Paint a single project row into `row_area` using the strict column plan.
///
/// Each column paints into its own rect; the full-row surface0 wash under a
/// selected row is painted first so the highlight covers every column
/// (including the pin-prefix gutter and any flex spacer between the session
/// pill and the cost chip).
fn render_row_into(
    buf: &mut Buffer,
    row_area: Rect,
    p: &Project,
    theme: &Theme,
    selected: bool,
    pin_slot: Option<u8>,
    plan: &ProjectColumnPlan,
) {
    // Full-row selection stripe. Matches the session-list treatment so the
    // highlight reads consistently across both screens.
    if selected {
        let wash_style = Style::default().bg(theme.surface0);
        for x in row_area.x..row_area.x.saturating_add(row_area.width) {
            for y in row_area.y..row_area.y.saturating_add(row_area.height) {
                buf[(x, y)].set_style(wash_style);
            }
        }
    }

    let bg = if selected { Some(theme.surface0) } else { None };
    let stamp_bg = |mut style: Style| -> Style {
        if let Some(c) = bg {
            style = style.bg(c);
        }
        style
    };

    let rects = Layout::horizontal(plan.constraints()).split(row_area);
    let pin_rect = rects[0];
    let name_rect = rects[1];
    let branch_rect = rects[2];
    let sessions_rect = rects[3];
    // rects[4] is the flex spacer — nothing to paint; the row-wash already
    // carries the selection colour there.
    let cost_rect = rects[5];
    let activity_rect = rects[6];

    // ── Col 1: pin prefix / pointer ──────────────────────────────────────
    if plan.show_pin {
        let pointer = if selected { "\u{25B8}" } else { " " };
        let pointer_style = stamp_bg(if selected {
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.surface2)
        });
        let pin_span = match pin_slot {
            Some(n) => Span::styled(
                format!("{n}:"),
                stamp_bg(
                    Style::default()
                        .fg(theme.peach)
                        .add_modifier(Modifier::BOLD),
                ),
            ),
            None => Span::styled("  ", stamp_bg(Style::default())),
        };
        let line = Line::from(vec![
            Span::styled(pointer, pointer_style),
            pin_span,
        ]);
        Paragraph::new(line)
            .style(stamp_bg(Style::default()))
            .render(pin_rect, buf);
    }

    // ── Col 2: name ──────────────────────────────────────────────────────
    let name_style = stamp_bg(if selected {
        theme.selected_row()
    } else {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    });
    let name_text = if display_width(&p.name) > NAME_COL_WIDTH {
        truncate_to_width(&p.name, NAME_COL_WIDTH)
    } else {
        p.name.clone()
    };
    let name_line = Line::from(vec![Span::styled(name_text, name_style)]);
    Paragraph::new(name_line)
        .style(stamp_bg(Style::default()))
        .render(name_rect, buf);

    // ── Col 3: git branch ────────────────────────────────────────────────
    if plan.show_branch {
        if let Some(branch) = p.git_branch.as_deref() {
            // Leave 3 cells for the leading space + decorator glyph.
            let budget = BRANCH_COL_WIDTH.saturating_sub(3);
            let trimmed = truncate_to_width(branch, budget);
            let branch_line = Line::from(vec![Span::styled(
                format!(" \u{2325} {trimmed}"),
                stamp_bg(Style::default().fg(theme.green)),
            )]);
            Paragraph::new(branch_line)
                .style(stamp_bg(Style::default()))
                .render(branch_rect, buf);
        }
    }

    // ── Col 4: session-count pill ────────────────────────────────────────
    if plan.show_sessions {
        // Fit the pill inside its column — `▌16 sessions▐` is 14 cells
        // exactly; 3-digit counts ("▌127 sessions▐") are 15 cells, so we
        // fall back to the short form when the long label overflows.
        let long_label = if p.session_count == 1 {
            " 1 session ".to_string()
        } else {
            format!(" {} sessions ", p.session_count)
        };
        let long_chip = format!("\u{258C}{long_label}\u{2590}");
        let chip_text = if display_width(&long_chip) <= SESSIONS_COL_WIDTH {
            long_chip
        } else {
            // Short form: `▌123 ⌁▐` is cramped but always ≤ 14 cells.
            let short = format!(" {} ", p.session_count);
            let short_chip = format!("\u{258C}{short}\u{2590}");
            if display_width(&short_chip) <= SESSIONS_COL_WIDTH {
                short_chip
            } else {
                truncate_to_width(&short_chip, SESSIONS_COL_WIDTH)
            }
        };
        let mut chip_style = Style::default()
            .fg(theme.subtext1)
            .add_modifier(Modifier::BOLD);
        // Use surface1 for the chip bed on a selected row so it still reads
        // as a floating slug over the highlight stripe.
        chip_style = chip_style.bg(if selected { theme.surface1 } else { theme.surface0 });
        let line = Line::from(vec![Span::raw(" "), Span::styled(chip_text, chip_style)]);
        Paragraph::new(line)
            .style(stamp_bg(Style::default()))
            .render(sessions_rect, buf);
    }

    // ── Col 6: cost chip ─────────────────────────────────────────────────
    // Project aggregates don't currently carry a cost field — see
    // `data::Project`. The column is reserved in the layout so the right-
    // hand side stays stable; when a future patch adds per-project cost
    // totals, plug the chip in here (and flip `show_cost` on zero-cost
    // rows to keep the gutter empty). Today we paint a blank rect so the
    // row-wash still covers the column when a row is selected.
    if plan.show_cost {
        Paragraph::new(Line::from(""))
            .style(stamp_bg(Style::default()))
            .render(cost_rect, buf);
    }

    // ── Col 7: last-activity ─────────────────────────────────────────────
    if plan.show_activity {
        let age = project_age(p.last_activity);
        let line = Line::from(vec![Span::styled(format!(" {age}"), stamp_bg(theme.dim()))])
            .alignment(Alignment::Left);
        Paragraph::new(line)
            .style(stamp_bg(Style::default()))
            .render(activity_rect, buf);
    }
}

fn project_age(ts: Option<DateTime<Utc>>) -> String {
    let Some(ts) = ts else {
        return "—".to_string();
    };
    let now = Utc::now();
    let diff = now.signed_duration_since(ts);
    if diff.num_minutes() < 60 {
        format!("{}m", diff.num_minutes().max(1))
    } else if diff.num_hours() < 24 {
        format!("{}h", diff.num_hours())
    } else if diff.num_days() < 7 {
        format!("{}d", diff.num_days())
    } else {
        ts.format("%b %d").to_string()
    }
}

/// Render the pinned strip into `area` using an explicit [`ProjectList`]
/// handle. Use this when the integrator has plumbed `App::project_list` and
/// wants the strip to reflect live data (the `&App`-only path above falls
/// back to a placeholder because it can't peek at the struct before the
/// field exists).
///
/// Keeping this helper public lets `picker.rs` (which is in this agent's
/// scope) render the strip directly once integration lands, without
/// touching this file again.
pub fn render_pinned_strip_with(
    f: &mut Frame<'_>,
    area: Rect,
    app: &App,
    pl: &ProjectList,
    theme: &Theme,
) {
    let active_cwd = app
        .active_project()
        .map(|p| p.path.to_string_lossy().into_owned());

    if area.width < PINNED_STRIP_FULL_WIDTH {
        let label = match &active_cwd {
            Some(cwd) => pl
                .pinned
                .iter()
                .find_map(|(slot, c)| (c == cwd.as_str()).then(|| format!("{slot}:pinned")))
                .unwrap_or_else(|| "0:all".to_string()),
            None => "0:all".to_string(),
        };
        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                format!("[{label}]"),
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    let mut spans: Vec<Span<'_>> = Vec::with_capacity(22);
    spans.push(Span::raw(" "));
    let zero_active = active_cwd.is_none();
    spans.push(tile_span("0", "all", zero_active, true, theme));

    for slot in 1..=9u8 {
        spans.push(Span::raw(" "));
        let cwd_opt = pl.pinned.at_slot(slot);
        let (label, is_active, is_filled) = match cwd_opt {
            Some(cwd) => {
                let name = cwd
                    .rsplit('/')
                    .find(|s| !s.is_empty())
                    .unwrap_or("?")
                    .to_string();
                let active = active_cwd.as_deref() == Some(cwd);
                (name, active, true)
            }
            None => ("(empty)".to_string(), false, false),
        };
        let label = truncate_to_width(&label, 14);
        spans.push(tile_span(&slot.to_string(), &label, is_active, is_filled, theme));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Render the pinned strip using F2/E17 project thumbnails. This is the
/// spec-named entry point — callers that want tiled identicons reach
/// through here instead of [`render_pinned_strip_with`]. Behaviour:
///
/// - Wide terminal (≥ [`thumbnail::MIN_STRIP_WIDTH`]) with ≥1 pin:
///   image-protocol tiles (kitty / iTerm2 / Sixel) when the probe
///   succeeds; halfblocks otherwise.
/// - Narrower or no-pin: delegates to [`render_pinned_strip_with`], so
///   callers don't need to duplicate degradation logic.
pub fn render_pinned_strip_with_thumbnails(
    f: &mut Frame<'_>,
    area: Rect,
    app: &App,
    pl: &ProjectList,
    theme: &Theme,
) {
    if area.width < THUMB_MIN_STRIP_WIDTH {
        render_pinned_strip_with(f, area, app, pl, theme);
        return;
    }
    let any_pin = (1u8..=9).any(|s| pl.pinned.at_slot(s).is_some());
    if !any_pin {
        render_pinned_strip_with(f, area, app, pl, theme);
        return;
    }

    let active_cwd = app
        .active_project()
        .map(|p| p.path.to_string_lossy().into_owned());

    let slots: Vec<(u8, String, bool)> = (1u8..=9)
        .filter_map(|slot| {
            let cwd = pl.pinned.at_slot(slot)?;
            let basename = thumbnail::basename_for_cwd(cwd);
            let active = active_cwd.as_deref() == Some(cwd);
            Some((slot, basename, active))
        })
        .collect();

    let mut cell = pl.thumbnails.borrow_mut();
    let renderer = cell.get_or_insert_with(ThumbnailRenderer::new);

    let slot_refs: Vec<PinnedSlot<'_>> = slots
        .iter()
        .map(|(slot, basename, active)| PinnedSlot {
            slot: *slot,
            basename: basename.as_str(),
            is_active: *active,
        })
        .collect();

    thumbnail::render_pinned_strip_with_thumbnails(f, area, &slot_refs, theme, renderer);
}

// ── Cold-start loading skeletons ─────────────────────────────────────────
//
// Mirrors `session_list::render_skeleton_rows`. See that module for the
// design rationale. Short version: while the enumeration that populates
// `app.projects` is still in flight, paint eight pulsing bars so the
// pane reads as "loading" rather than the "no projects yet" empty state.

/// How long we show the skeleton rows on a cold start.
pub(crate) const SKELETON_WINDOW: Duration = Duration::from_millis(1200);

/// Number of skeleton rows painted on the project list.
pub(crate) const SKELETON_ROW_COUNT: usize = 8;

/// True when `init_instant` is recent enough that we should paint
/// skeletons instead of the empty-state copy.
pub(crate) fn is_within_skeleton_window(init_instant: Instant) -> bool {
    Instant::now().saturating_duration_since(init_instant) < SKELETON_WINDOW
}

/// Render eight pulsing skeleton bars that approximate the (pin, name,
/// sessions, cost) shape of a real project row. Reduce-motion users
/// (`CLAUDE_PICKER_NO_ANIM=1`) get a static `surface1` wash.
fn render_skeleton_rows(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let reduce_motion = theme::animations_disabled();
    paint_skeleton_rows(f.buffer_mut(), area, theme, reduce_motion, Instant::now());

    let row_count = SKELETON_ROW_COUNT.min(area.height as usize);
    if row_count > 2 {
        let label_y = area.y.saturating_add(2);
        let label_x = area.x.saturating_add(2);
        let label_area = Rect {
            x: label_x,
            y: label_y,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        let para = Paragraph::new(Line::from(Span::styled(
            "loading\u{2026}",
            Style::default()
                .fg(theme.overlay0)
                .add_modifier(Modifier::ITALIC),
        )));
        f.render_widget(para, label_area);
    }
}

/// Buffer-only skeleton paint — the testable core of [`render_skeleton_rows`].
/// Paints exactly [`SKELETON_ROW_COUNT`] rows (clamped to area height).
/// Returns the actual number of rows painted so tests can assert
/// emission without needing a [`Frame`].
pub(crate) fn paint_skeleton_rows(
    buf: &mut Buffer,
    area: Rect,
    theme: &Theme,
    reduce_motion: bool,
    now: Instant,
) -> usize {
    if area.height == 0 || area.width == 0 {
        return 0;
    }

    let bar_color = if reduce_motion {
        theme.surface1
    } else {
        skeleton_pulse_color(theme, now)
    };

    let row_count = SKELETON_ROW_COUNT.min(area.height as usize);
    for row_ix in 0..row_count {
        let row_y = area.y.saturating_add(row_ix as u16);
        if row_y >= area.y.saturating_add(area.height) {
            break;
        }

        let jitter = (row_ix * 7 + 3) % 6;
        let name_w = (22 + jitter).min(area.width.saturating_sub(2) as usize / 2) as u16;
        let cost_w = 6u16;
        let age_w = 4u16;

        paint_bar(buf, area, row_y, 2, name_w, bar_color);
        if area.width > (name_w + cost_w + age_w + 6) {
            let cost_x = area
                .x
                .saturating_add(area.width.saturating_sub(cost_w + age_w + 3));
            paint_bar(buf, area, row_y, cost_x - area.x, cost_w, bar_color);
        }
        if area.width > (age_w + 3) {
            let age_x = area.x.saturating_add(area.width.saturating_sub(age_w + 1));
            paint_bar(buf, area, row_y, age_x - area.x, age_w, bar_color);
        }
    }
    row_count
}

/// Triangular-wave pulse between `theme.surface0` and `theme.surface1`
/// on a 1.8 s loop. Keeps a thread-local anchor so phase is stable
/// across frames.
fn skeleton_pulse_color(theme: &Theme, now: Instant) -> ratatui::style::Color {
    thread_local! {
        static ANCHOR: std::cell::OnceCell<Instant> = const { std::cell::OnceCell::new() };
    }
    let anchor = ANCHOR.with(|c| *c.get_or_init(Instant::now));
    let elapsed_ms = now.saturating_duration_since(anchor).as_millis() as u64;
    let period_ms: u64 = 1800;
    let phase_u = elapsed_ms % period_ms;
    let half = period_ms / 2;
    let t = if phase_u <= half {
        phase_u as f32 / half as f32
    } else {
        1.0 - ((phase_u - half) as f32 / half as f32)
    };
    theme::lerp_color(theme.surface0, theme.surface1, t.clamp(0.0, 1.0))
}

/// Paint a horizontal bar of `width` cells starting at
/// `(area.x + offset_x, row_y)`. Clips at the area boundary.
fn paint_bar(
    buf: &mut Buffer,
    area: Rect,
    row_y: u16,
    offset_x: u16,
    width: u16,
    color: ratatui::style::Color,
) {
    let start_x = area.x.saturating_add(offset_x);
    let end_x = start_x
        .saturating_add(width)
        .min(area.x.saturating_add(area.width));
    if row_y < area.y || row_y >= area.y.saturating_add(area.height) {
        return;
    }
    let style = Style::default().bg(color);
    for x in start_x..end_x {
        buf[(x, row_y)].set_style(style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_plan_wide_pane_shows_everything() {
        let p = ProjectColumnPlan::for_width(120);
        assert!(p.show_pin);
        assert!(p.show_branch);
        assert!(p.show_sessions);
        assert!(p.show_cost);
        assert!(p.show_activity);
    }

    #[test]
    fn project_plan_drops_branch_under_90() {
        let p = ProjectColumnPlan::for_width(89);
        assert!(!p.show_branch, "branch drops at 89 cols");
        assert!(p.show_sessions);
        assert!(p.show_cost);
    }

    #[test]
    fn project_plan_drops_cost_under_70() {
        let p = ProjectColumnPlan::for_width(69);
        assert!(!p.show_cost);
        assert!(p.show_sessions);
    }

    #[test]
    fn project_plan_fixed_columns_never_exceed_width() {
        // Same guardrail as the session list — the sum of explicit Length
        // constraints must fit inside the pane width at every breakpoint
        // so `Layout::horizontal` never has to clip fixed columns.
        let fixed_total = |p: &ProjectColumnPlan| -> u16 {
            p.constraints()
                .iter()
                .filter_map(|c| match c {
                    Constraint::Length(n) => Some(*n),
                    _ => None,
                })
                .sum()
        };
        for w in [40u16, 53, 54, 69, 70, 89, 90, 100, 120, 160] {
            let plan = ProjectColumnPlan::for_width(w);
            let total = fixed_total(&plan);
            assert!(
                total <= w,
                "width={w} fixed-column total={total} exceeds pane width",
            );
        }
    }

    #[test]
    fn project_scroll_start_matches_list_semantics() {
        assert_eq!(scroll_start(0, 10, 50), 0);
        assert_eq!(scroll_start(5, 10, 50), 0);
        assert_eq!(scroll_start(9, 10, 50), 0);
        assert_eq!(scroll_start(10, 10, 50), 1);
        assert_eq!(scroll_start(49, 10, 50), 40);
        // Short list — never scrolls.
        assert_eq!(scroll_start(2, 10, 3), 0);
    }

    // ── Skeleton rendering ───────────────────────────────────────────────

    #[test]
    fn skeleton_rows_emit_8_rows_on_cold_empty_list() {
        let theme = crate::theme::Theme::from_name(crate::theme::ThemeName::default());
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        let row_count = paint_skeleton_rows(
            &mut buf,
            area,
            &theme,
            /* reduce_motion = */ true,
            Instant::now(),
        );
        assert_eq!(row_count, SKELETON_ROW_COUNT);
        assert_eq!(SKELETON_ROW_COUNT, 8);

        let bar_cells = (0..SKELETON_ROW_COUNT as u16)
            .filter(|y| buf[(2_u16, *y)].bg == theme.surface1)
            .count();
        assert_eq!(bar_cells, SKELETON_ROW_COUNT);
    }

    #[test]
    fn skeleton_hides_once_data_arrives() {
        let long_ago = Instant::now()
            .checked_sub(SKELETON_WINDOW + Duration::from_millis(100))
            .expect("Instant subtraction");
        assert!(
            !is_within_skeleton_window(long_ago),
            "skeleton window must close after SKELETON_WINDOW elapses"
        );

        let fresh = Instant::now();
        assert!(
            is_within_skeleton_window(fresh),
            "skeleton window must be open on fresh construction"
        );
    }
}
