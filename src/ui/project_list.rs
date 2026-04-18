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

use chrono::{DateTime, Utc};
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;

use crate::app::App;
use crate::data::pinned_projects::{PinnedProjects, ToggleResult};
use crate::data::Project;
use crate::theme::Theme;
use crate::ui::text::{display_width, pad_to_width, truncate_to_width};
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

fn render_list(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

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

    // Pre-compute pin-slot lookup so rows can prefix a `N:` badge in peach
    // bold when the project is pinned. The pinned store is a small map
    // (max 9 entries) so scanning it inside the row renderer would also be
    // cheap — we lift the HashMap here anyway so the row renderer is pure.
    let pinned = app.project_list().pinned();
    let slot_by_cwd: std::collections::HashMap<String, u8> = (1u8..=9)
        .filter_map(|slot| pinned.at_slot(slot).map(|cwd| (cwd.to_string(), slot)))
        .collect();

    let items: Vec<ListItem<'_>> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &idx)| {
            let p = &app.projects[idx];
            let is_sel = Some(display_idx) == app.cursor_position();
            let slot = slot_by_cwd
                .get(&p.path.to_string_lossy().into_owned())
                .copied();
            ListItem::new(render_row(p, theme, is_sel, slot))
        })
        .collect();

    let mut state = ListState::default();
    state.select(app.cursor_position());
    let list = List::new(items).highlight_symbol("");
    f.render_stateful_widget(list, area, &mut state);

    // Scrollbar on the right edge, only when the list overflows.
    let total = app.filtered_indices.len();
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

fn render_row<'a>(
    p: &'a Project,
    theme: &Theme,
    selected: bool,
    pin_slot: Option<u8>,
) -> Line<'a> {
    // Name style — theme.text bold for the primary label, selection stripe
    // wins when the row is under the cursor.
    let name_style = if selected {
        theme.selected_row()
    } else {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    };

    let pointer = if selected { "\u{25B8}" } else { " " };
    let pointer_style = if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface2)
    };

    // Pin-slot prefix: `N:` in peach bold when the project is pinned into
    // one of the k9s-style slots. Non-pinned rows get a 2-space gutter so
    // names stay column-aligned across pinned / unpinned neighbours.
    let pin_prefix = match pin_slot {
        Some(n) => Span::styled(
            format!("{n}:"),
            Style::default()
                .fg(theme.peach)
                .add_modifier(Modifier::BOLD),
        ),
        None => Span::raw("  "),
    };

    // Pad the name to 28 display cols so the following pills line up.
    let name_col_width: usize = 28;
    let name = if display_width(&p.name) > name_col_width {
        truncate_to_width(&p.name, name_col_width)
    } else {
        pad_to_width(&p.name, name_col_width)
    };

    // Optional git-branch inline — dimmer than the name, kept short. Uses
    // `⌥` as a decorator so the branch reads as a subtitle, not a pill.
    let branch_span = p
        .git_branch
        .as_deref()
        .map(|b| {
            let trimmed = truncate_to_width(b, 16);
            Span::styled(
                format!(" \u{2325} {trimmed}"),
                Style::default().fg(theme.green),
            )
        })
        .unwrap_or_else(|| Span::raw(""));

    // Sessions counter as a subtle pill: `▌12 sessions▐` in subtext1 over
    // surface0 (floating chip look). The pluralisation stays textual so the
    // pill reads the same on tiny and huge projects.
    let session_label = if p.session_count == 1 {
        " 1 session ".to_string()
    } else {
        format!(" {} sessions ", p.session_count)
    };
    let session_pill = Span::styled(
        format!("\u{258C}{session_label}\u{2590}"),
        Style::default()
            .fg(theme.subtext1)
            .bg(theme.surface0)
            .add_modifier(Modifier::BOLD),
    );

    // Last activity — right-side, theme.dim, small. The helper collapses to
    // `—` when we have no timestamp so the column stays anchored.
    let age = project_age(p.last_activity);
    let age_span = Span::styled(format!(" {age:>6}"), theme.dim());

    let mut spans: Vec<Span<'a>> = Vec::with_capacity(10);
    spans.push(Span::styled(format!(" {pointer} "), pointer_style));
    spans.push(pin_prefix);
    spans.push(Span::styled(name, name_style));
    spans.push(branch_span);
    spans.push(Span::raw(" "));
    spans.push(session_pill);
    spans.push(Span::raw("  "));
    spans.push(age_span);

    if selected {
        for span in &mut spans {
            span.style.bg = Some(theme.surface0);
        }
    }

    Line::from(spans)
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
