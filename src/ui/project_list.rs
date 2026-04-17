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
use crate::ui::text::{pad_to_width, truncate_to_width};

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
}

impl ProjectList {
    /// Load the pinned store from disk. Malformed files silently fall back
    /// to an empty store — pinned slots are nice-to-have, never load-bearing.
    pub fn load() -> Self {
        Self {
            pinned: PinnedProjects::load(),
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

    // The pinned strip eats one row at the top. When the strip is empty (no
    // pins yet) we still draw a "no pins — press u to pin" hint so the
    // feature is discoverable.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // pinned strip (1 row)
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
/// - When ≥ `PINNED_STRIP_FULL_WIDTH` cols: show the full `[0:all] [1:name]
///   [2:(empty)] …` row.
/// - Below that: collapse to just the active slot's label to avoid wrapping.
fn render_pinned_strip(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    // Safe access — whether `App::pinned_projects()` is wired yet or not we
    // render something. If the App doesn't expose a pinned store the strip
    // prints a placeholder inviting the user to press `u`.
    let maybe_pinned = try_get_pinned(app);

    let active_cwd = app
        .active_project()
        .map(|p| p.path.to_string_lossy().into_owned());

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
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::raw("No Claude Code projects yet."),
            Line::raw(""),
            Line::raw("Run `claude` in any directory to get started."),
        ])
        .style(theme.muted())
        .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem<'_>> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(display_idx, &idx)| {
            let p = &app.projects[idx];
            let is_sel = Some(display_idx) == app.cursor_position();
            ListItem::new(render_row(p, theme, is_sel))
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

fn render_row<'a>(p: &'a Project, theme: &Theme, selected: bool) -> Line<'a> {
    let name_style = if selected {
        theme.selected_row()
    } else {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    };

    let pointer = if selected { "▸" } else { " " };
    let pointer_style = if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface2)
    };

    // Pad to exactly 30 display columns. The old `format!("{name:<30}")`
    // path measured the formatter's padding in bytes-not-columns; a CJK or
    // emoji project name would shift the branch/sessions columns out of
    // alignment. `pad_to_width` is the column-correct replacement.
    let name = pad_to_width(&p.name, 30);

    let branch = p
        .git_branch
        .as_deref()
        .map(|b| format!(" ⌥ {b}"))
        .unwrap_or_default();

    let sessions = format!("{} sessions", p.session_count);
    let age = project_age(p.last_activity);

    let mut spans = vec![
        Span::styled(format!(" {pointer} "), pointer_style),
        Span::styled(name, name_style),
        Span::styled(branch, Style::default().fg(theme.green)),
        Span::raw(" "),
        Span::styled(sessions, Style::default().fg(theme.overlay1)),
        Span::raw("  "),
        Span::styled(age, theme.muted()),
    ];

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
