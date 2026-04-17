//! `?` context-sensitive help overlay.
//!
//! A centered modal popup that lists every keybinding for the current screen.
//! Triggered by `?` and dismissed by `?`, `q`, or `Esc`. Each screen gets its
//! own [`HelpContent`]; the renderer is screen-agnostic and just lays out the
//! group/key/desc triples.
//!
//! Layout:
//! - Modal roughly 70×22, capped to the available frame, centered
//! - Rounded mauve border with title " Keyboard shortcuts "
//! - Two-column grid: pill-styled key on the left, description on the right
//! - Group headers in mauve-bold to chunk the list
//!
//! The content vectors live here so a new screen can opt-in by providing
//! its own [`Screen`] variant and a match arm in [`help_for`].

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::text::display_width;

/// Identifier for which screen the overlay is being rendered on top of.
///
/// Added as a new variant is introduced to a screen — each variant determines
/// the list of keybindings presented by [`help_for`]. A separate enum rather
/// than reusing `app::Mode` keeps the command-level screens (tree / search /
/// stats / diff) addressable without coupling them to the picker state
/// machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// The default picker session-list view.
    SessionList,
    /// The default picker project-list view.
    ProjectList,
    /// `claude-picker tree` fork-tree view.
    Tree,
    /// `claude-picker search` full-text search view.
    Search,
    /// `claude-picker stats` dashboard.
    Stats,
    /// `claude-picker diff` diff view.
    Diff,
    /// Full-screen conversation viewer — the `v` keybinding overlay.
    Viewer,
    /// `claude-picker --files` file-centric pivot view.
    Files,
}

/// One key-to-description pair.
#[derive(Debug, Clone, Copy)]
pub struct KeyEntry {
    pub key: &'static str,
    pub desc: &'static str,
}

/// One named group of keybindings.
#[derive(Debug, Clone, Copy)]
pub struct KeyGroup {
    pub title: &'static str,
    pub entries: &'static [KeyEntry],
}

/// Everything the overlay needs to render for a given screen.
#[derive(Debug, Clone, Copy)]
pub struct HelpContent {
    pub screen: Screen,
    pub groups: &'static [KeyGroup],
}

// ── Content for each screen ────────────────────────────────────────────────

const NAV_SESSION: &[KeyEntry] = &[
    KeyEntry {
        key: "↑ ↓ / j k",
        desc: "move up / down one row",
    },
    KeyEntry {
        key: "gg",
        desc: "jump to top",
    },
    KeyEntry {
        key: "G",
        desc: "jump to bottom",
    },
    KeyEntry {
        key: "PgUp / PgDn",
        desc: "jump 10 rows",
    },
    KeyEntry {
        key: "/",
        desc: "focus filter (or just start typing)",
    },
    KeyEntry {
        key: "Esc",
        desc: "clear filter / pop screen",
    },
    KeyEntry {
        key: "q",
        desc: "quit",
    },
];

const SELECTION_SESSION: &[KeyEntry] = &[
    KeyEntry {
        key: "Enter",
        desc: "resume selected session",
    },
    KeyEntry {
        key: "v",
        desc: "open conversation viewer (fullscreen)",
    },
    KeyEntry {
        key: "Tab",
        desc: "toggle multi-select on row",
    },
    KeyEntry {
        key: "Esc",
        desc: "clear multi-selection (when active)",
    },
];

const ACTIONS_SESSION: &[KeyEntry] = &[
    KeyEntry {
        key: "Ctrl+B",
        desc: "toggle bookmark (pin to top)",
    },
    KeyEntry {
        key: "Ctrl+E",
        desc: "export session (or all selected)",
    },
    KeyEntry {
        key: "Ctrl+D",
        desc: "delete session (or all selected)",
    },
    KeyEntry {
        key: "y",
        desc: "copy id (or all selected ids)",
    },
    KeyEntry {
        key: "Y",
        desc: "copy project path(s) to clipboard",
    },
    KeyEntry {
        key: "r",
        desc: "rename session (currently selected)",
    },
    KeyEntry {
        key: "o",
        desc: "open project in $EDITOR",
    },
];

const HELP_GROUP: &[KeyEntry] = &[KeyEntry {
    key: "?",
    desc: "this overlay",
}];

/// yazi-style background-task drawer bindings. Shown on every picker
/// screen that owns an event loop on `App`, since the task queue itself
/// is app-scoped and visible from anywhere.
const ASYNC_GROUP: &[KeyEntry] = &[
    KeyEntry {
        key: "w",
        desc: "toggle background task drawer",
    },
    KeyEntry {
        key: "j / k",
        desc: "move focus up / down (drawer mode)",
    },
    KeyEntry {
        key: "x",
        desc: "cancel focused task (drawer mode)",
    },
];

/// Vim-style navigation and discoverability primitives shared by the
/// picker screens. Added as part of the keyboard-UX pass — these land in
/// every screen that owns an event loop on `App`.
const NAV_ADVANCED: &[KeyEntry] = &[
    KeyEntry {
        key: "z / Z",
        desc: "undo / redo last destructive action",
    },
    KeyEntry {
        key: "Ctrl-o / Ctrl-i",
        desc: "jump back / forward in selection history",
    },
    KeyEntry {
        key: "3j / 12G",
        desc: "repeat count prefix (rows / goto)",
    },
    KeyEntry {
        key: "Space (hold)",
        desc: "which-key popup",
    },
];

const SESSION_LIST_GROUPS: &[KeyGroup] = &[
    KeyGroup {
        title: "NAVIGATION",
        entries: NAV_SESSION,
    },
    KeyGroup {
        title: "NAVIGATION (advanced)",
        entries: NAV_ADVANCED,
    },
    KeyGroup {
        title: "SELECTION",
        entries: SELECTION_SESSION,
    },
    KeyGroup {
        title: "ACTIONS",
        entries: ACTIONS_SESSION,
    },
    KeyGroup {
        title: "ASYNC",
        entries: ASYNC_GROUP,
    },
    KeyGroup {
        title: "HELP",
        entries: HELP_GROUP,
    },
];

const NAV_PROJECT: &[KeyEntry] = &[
    KeyEntry {
        key: "↑ ↓ / j k",
        desc: "move up / down one row",
    },
    KeyEntry {
        key: "gg",
        desc: "jump to top",
    },
    KeyEntry {
        key: "G",
        desc: "jump to bottom",
    },
    KeyEntry {
        key: "PgUp / PgDn",
        desc: "jump 10 rows",
    },
    KeyEntry {
        key: "/",
        desc: "focus filter (or just start typing)",
    },
    KeyEntry {
        key: "Esc",
        desc: "clear filter / quit",
    },
    KeyEntry {
        key: "q",
        desc: "quit",
    },
];

const SELECTION_PROJECT: &[KeyEntry] = &[KeyEntry {
    key: "Enter",
    desc: "open project → session list",
}];

const ACTIONS_PROJECT: &[KeyEntry] = &[
    KeyEntry {
        key: "Y",
        desc: "copy project path to clipboard",
    },
    KeyEntry {
        key: "o",
        desc: "open project in $EDITOR",
    },
];

const PROJECT_LIST_GROUPS: &[KeyGroup] = &[
    KeyGroup {
        title: "NAVIGATION",
        entries: NAV_PROJECT,
    },
    KeyGroup {
        title: "NAVIGATION (advanced)",
        entries: NAV_ADVANCED,
    },
    KeyGroup {
        title: "SELECTION",
        entries: SELECTION_PROJECT,
    },
    KeyGroup {
        title: "ACTIONS",
        entries: ACTIONS_PROJECT,
    },
    KeyGroup {
        title: "ASYNC",
        entries: ASYNC_GROUP,
    },
    KeyGroup {
        title: "HELP",
        entries: HELP_GROUP,
    },
];

const NAV_TREE: &[KeyEntry] = &[
    KeyEntry {
        key: "↑ ↓ / j k",
        desc: "move up / down one row",
    },
    KeyEntry {
        key: "→ / l",
        desc: "expand fork subtree",
    },
    KeyEntry {
        key: "← / h",
        desc: "collapse, or jump to parent",
    },
    KeyEntry {
        key: "Space",
        desc: "toggle expand / open palette",
    },
    KeyEntry {
        key: "gg",
        desc: "jump to top",
    },
    KeyEntry {
        key: "G",
        desc: "jump to bottom",
    },
    KeyEntry {
        key: "PgUp / PgDn",
        desc: "jump 10 rows",
    },
    KeyEntry {
        key: "Esc / q",
        desc: "quit",
    },
];

const SELECTION_TREE: &[KeyEntry] = &[KeyEntry {
    key: "Enter",
    desc: "resume selected session",
}];

const ACTIONS_TREE: &[KeyEntry] = &[
    KeyEntry {
        key: "y",
        desc: "copy session id to clipboard",
    },
    KeyEntry {
        key: "Y",
        desc: "copy project path to clipboard",
    },
    KeyEntry {
        key: "o",
        desc: "open project in $EDITOR",
    },
];

const TREE_GROUPS: &[KeyGroup] = &[
    KeyGroup {
        title: "NAVIGATION",
        entries: NAV_TREE,
    },
    KeyGroup {
        title: "SELECTION",
        entries: SELECTION_TREE,
    },
    KeyGroup {
        title: "ACTIONS",
        entries: ACTIONS_TREE,
    },
    KeyGroup {
        title: "HELP",
        entries: HELP_GROUP,
    },
];

const NAV_SEARCH: &[KeyEntry] = &[
    KeyEntry {
        key: "↑ ↓ / j k",
        desc: "move up / down one result",
    },
    KeyEntry {
        key: "gg",
        desc: "jump to top",
    },
    KeyEntry {
        key: "G",
        desc: "jump to bottom",
    },
    KeyEntry {
        key: "PgUp / PgDn",
        desc: "jump 10 rows",
    },
    KeyEntry {
        key: "Esc / q",
        desc: "quit",
    },
];

const SELECTION_SEARCH: &[KeyEntry] = &[KeyEntry {
    key: "Enter",
    desc: "resume selected session",
}];

const ACTIONS_SEARCH: &[KeyEntry] = &[
    KeyEntry {
        key: "p / Ctrl+P",
        desc: "toggle preview pane",
    },
    KeyEntry {
        key: "y",
        desc: "copy session id to clipboard",
    },
    KeyEntry {
        key: "Y",
        desc: "copy project path to clipboard",
    },
    KeyEntry {
        key: "o",
        desc: "open project in $EDITOR",
    },
];

const NAV_FILES: &[KeyEntry] = &[
    KeyEntry {
        key: "↑ ↓ / j k",
        desc: "move up / down one row",
    },
    KeyEntry {
        key: "PgUp / PgDn",
        desc: "jump 10 rows",
    },
    KeyEntry {
        key: "Tab",
        desc: "switch focus between file list and session list",
    },
    KeyEntry {
        key: "/",
        desc: "focus filter (fuzzy match on file path)",
    },
    KeyEntry {
        key: "Esc",
        desc: "clear filter / back to file list",
    },
    KeyEntry {
        key: "q",
        desc: "quit",
    },
];

const ACTIONS_FILES: &[KeyEntry] = &[
    KeyEntry {
        key: "s",
        desc: "cycle sort (edits → recency → sessions → path)",
    },
    KeyEntry {
        key: "o",
        desc: "open focused file in $EDITOR",
    },
    KeyEntry {
        key: "Enter",
        desc: "resume focused session (when session pane is focused)",
    },
    KeyEntry {
        key: "v",
        desc: "open conversation viewer for focused session",
    },
];

const FILES_GROUPS: &[KeyGroup] = &[
    KeyGroup {
        title: "NAVIGATION",
        entries: NAV_FILES,
    },
    KeyGroup {
        title: "ACTIONS",
        entries: ACTIONS_FILES,
    },
    KeyGroup {
        title: "ASYNC",
        entries: ASYNC_GROUP,
    },
    KeyGroup {
        title: "HELP",
        entries: HELP_GROUP,
    },
];

const SEARCH_GROUPS: &[KeyGroup] = &[
    KeyGroup {
        title: "NAVIGATION",
        entries: NAV_SEARCH,
    },
    KeyGroup {
        title: "SELECTION",
        entries: SELECTION_SEARCH,
    },
    KeyGroup {
        title: "ACTIONS",
        entries: ACTIONS_SEARCH,
    },
    KeyGroup {
        title: "HELP",
        entries: HELP_GROUP,
    },
];

const STATS_NAV: &[KeyEntry] = &[
    KeyEntry {
        key: "Esc / q",
        desc: "quit",
    },
    KeyEntry {
        key: "r",
        desc: "refresh (full rescan)",
    },
    KeyEntry {
        key: "e",
        desc: "export CSV to ~/Desktop",
    },
];

const STATS_TIMELINE: &[KeyEntry] = &[KeyEntry {
    key: "t",
    desc: "cycle: days → weeks → hours → month",
}];

const STATS_BUDGET: &[KeyEntry] = &[
    KeyEntry {
        key: "b",
        desc: "open budget modal (set monthly limit)",
    },
    KeyEntry {
        key: "f",
        desc: "toggle forecast band",
    },
];

const STATS_GROUPS: &[KeyGroup] = &[
    KeyGroup {
        title: "NAVIGATION",
        entries: STATS_NAV,
    },
    KeyGroup {
        title: "TIMELINE",
        entries: STATS_TIMELINE,
    },
    KeyGroup {
        title: "BUDGET",
        entries: STATS_BUDGET,
    },
    KeyGroup {
        title: "HELP",
        entries: HELP_GROUP,
    },
];

const DIFF_GROUPS: &[KeyGroup] = &[
    KeyGroup {
        title: "NAVIGATION",
        entries: &[
            KeyEntry {
                key: "↑ ↓ / j k",
                desc: "scroll one line",
            },
            KeyEntry {
                key: "PgUp / PgDn",
                desc: "scroll 10 lines",
            },
            KeyEntry {
                key: "Esc / q",
                desc: "quit",
            },
        ],
    },
    KeyGroup {
        title: "HELP",
        entries: HELP_GROUP,
    },
];

const NAV_VIEWER: &[KeyEntry] = &[
    KeyEntry {
        key: "↑ ↓ / j k",
        desc: "scroll one line",
    },
    KeyEntry {
        key: "Space / PgDn",
        desc: "page down",
    },
    KeyEntry {
        key: "b / PgUp",
        desc: "page up",
    },
    KeyEntry {
        key: "Ctrl+D / Ctrl+U",
        desc: "half page down / up",
    },
    KeyEntry {
        key: "gg",
        desc: "jump to top",
    },
    KeyEntry {
        key: "G",
        desc: "jump to bottom",
    },
    KeyEntry {
        key: "Esc / q",
        desc: "close viewer",
    },
];

const ACTIONS_VIEWER: &[KeyEntry] = &[
    KeyEntry {
        key: "/",
        desc: "find in transcript",
    },
    KeyEntry {
        key: "n / N",
        desc: "next / previous match",
    },
    KeyEntry {
        key: "] / [",
        desc: "next / previous tool call",
    },
    KeyEntry {
        key: "y",
        desc: "copy centered message",
    },
];

const VIEWER_GROUPS: &[KeyGroup] = &[
    KeyGroup {
        title: "NAVIGATION",
        entries: NAV_VIEWER,
    },
    KeyGroup {
        title: "ACTIONS",
        entries: ACTIONS_VIEWER,
    },
    KeyGroup {
        title: "HELP",
        entries: HELP_GROUP,
    },
];

/// Return the help content for a given screen.
pub fn help_for(screen: Screen) -> HelpContent {
    let groups = match screen {
        Screen::SessionList => SESSION_LIST_GROUPS,
        Screen::ProjectList => PROJECT_LIST_GROUPS,
        Screen::Tree => TREE_GROUPS,
        Screen::Search => SEARCH_GROUPS,
        Screen::Stats => STATS_GROUPS,
        Screen::Diff => DIFF_GROUPS,
        Screen::Viewer => VIEWER_GROUPS,
        Screen::Files => FILES_GROUPS,
    };
    HelpContent { screen, groups }
}

/// True when the given key press should dismiss the overlay.
pub fn is_dismiss_key(c: char) -> bool {
    matches!(c, '?' | 'q')
}

/// Render the help overlay centered inside `area`, over whatever is below.
///
/// We render a `Clear` widget first so the underlying screen doesn't bleed
/// through — important because Ratatui double-buffers and partial glyph
/// overlap would otherwise happen on the edges.
pub fn render(frame: &mut Frame<'_>, area: Rect, content: HelpContent, theme: &Theme) {
    // Compute content-driven height so the modal auto-fits.
    let total_entries: usize = content.groups.iter().map(|g| g.entries.len()).sum();
    // 2 header+footer lines + 1 line per group heading + 1 blank line between
    // groups + 1 line per entry. Cap at the frame height minus 2 margin rows.
    let desired_h = (2 + content.groups.len() * 2 + total_entries).saturating_add(2) as u16;
    let h = desired_h.min(area.height.saturating_sub(2)).max(8);
    let w = 74u16.min(area.width.saturating_sub(4));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "Keyboard shortcuts",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_bottom(
            Line::from(vec![
                Span::raw(" "),
                Span::styled("?", theme.key_hint()),
                Span::raw(" "),
                Span::styled("close", theme.key_desc()),
                Span::raw(" "),
            ])
            .right_aligned(),
        );

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    // Build the body. For each group: header, entries (pill-styled key on the
    // left, description on the right), then a blank spacer.
    let key_col_width = 14usize; // right-padded key zone, leaves room for wide chords
    let mut lines: Vec<Line<'static>> =
        Vec::with_capacity(total_entries + content.groups.len() * 2 + 1);
    lines.push(Line::raw(""));
    for (gi, group) in content.groups.iter().enumerate() {
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                group.title.to_string(),
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        for entry in group.entries {
            lines.push(render_entry_line(entry, key_col_width, theme));
        }
        if gi + 1 < content.groups.len() {
            lines.push(Line::raw(""));
        }
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);
    frame.render_widget(p, inner);
}

/// Format one key-description row: a pill-styled key on a mantle background,
/// then some space, then the description in muted text.
fn render_entry_line(entry: &KeyEntry, key_col_width: usize, theme: &Theme) -> Line<'static> {
    // Pad the key to a fixed column width so descriptions align. We pad
    // *outside* the pill span so the colored background stays tight to the
    // key text itself. `display_width` so glyphs like "↑↓" (2 cols: arrow
    // + arrow, each a single column) are counted in terminal cells rather
    // than codepoints — otherwise a CJK-labelled hotkey could skew the grid.
    let key_chars = display_width(entry.key);
    let pad = key_col_width.saturating_sub(key_chars).max(1);
    let pill_style = Style::default()
        .fg(theme.yellow)
        .bg(theme.mantle)
        .add_modifier(Modifier::BOLD);
    Line::from(vec![
        Span::raw("   "),
        Span::styled(format!(" {} ", entry.key), pill_style),
        Span::raw(" ".repeat(pad)),
        Span::styled(entry.desc.to_string(), theme.key_desc()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_screens_produce_non_empty_groups() {
        for screen in [
            Screen::SessionList,
            Screen::ProjectList,
            Screen::Tree,
            Screen::Search,
            Screen::Stats,
            Screen::Diff,
        ] {
            let c = help_for(screen);
            assert!(!c.groups.is_empty(), "screen {screen:?} has no groups");
            for g in c.groups {
                assert!(!g.entries.is_empty(), "empty group in {screen:?}");
            }
        }
    }

    #[test]
    fn dismiss_keys_recognised() {
        assert!(is_dismiss_key('?'));
        assert!(is_dismiss_key('q'));
        assert!(!is_dismiss_key('a'));
    }

    #[test]
    fn session_list_has_core_shortcuts() {
        let c = help_for(Screen::SessionList);
        let mut found_y = false;
        let mut found_r = false;
        let mut found_o = false;
        for g in c.groups {
            for e in g.entries {
                if e.key == "y" {
                    found_y = true;
                }
                if e.key == "r" {
                    found_r = true;
                }
                if e.key == "o" {
                    found_o = true;
                }
            }
        }
        assert!(found_y, "session-list help missing y");
        assert!(found_r, "session-list help missing r");
        assert!(found_o, "session-list help missing o");
    }
}
