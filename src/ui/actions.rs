//! Palette / help action registry.
//!
//! A single source of truth for the set of named user actions the app
//! exposes per screen. Both the `?` help overlay and the Space-leader
//! command palette render from this table, so keybindings listed to the
//! user in one place match what you can run from the other.
//!
//! Each [`PaletteAction`] is `'static` — the registry is a compile-time
//! table of pointers to descriptors, not a dynamic trait object graph.
//! That keeps the palette's filter pass allocation-free and lets us
//! hand a plain `Vec<&'static PaletteAction>` around.
//!
//! When adding a new action:
//! 1. Add a `PaletteAction` entry below with a unique `id`.
//! 2. Add the id to [`actions_for_context`] for the screens it applies to.
//! 3. Wire the id into the matching `execute_palette_action` branch on
//!    whichever state type owns that screen.

/// The screen a palette is opening from. Determines which actions are
/// offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    SessionList,
    ProjectList,
    Tree,
    Search,
}

/// Grouping tag — used to sort the palette visually, and to render the
/// help overlay's section headings. Matches the help overlay's existing
/// four buckets so the two lists stay visually aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionGroup {
    Navigation,
    Selection,
    Actions,
    Help,
}

impl ActionGroup {
    pub fn title(self) -> &'static str {
        match self {
            Self::Navigation => "NAVIGATION",
            Self::Selection => "SELECTION",
            Self::Actions => "ACTIONS",
            Self::Help => "HELP",
        }
    }
}

/// One entry in the palette registry. The fields are static because the
/// registry is a compile-time table; nothing about the set of actions
/// varies at runtime.
#[derive(Debug, Clone, Copy)]
pub struct PaletteAction {
    /// Stable identifier consumed by state's `execute_palette_action`.
    pub id: &'static str,
    /// Display label (first line in the palette row).
    pub label: &'static str,
    /// Keybinding hint rendered right-aligned next to the label. Empty
    /// string for actions that are only reachable via the palette.
    pub keybinding: &'static str,
    /// Optional glyph shown at the left edge of the row — keeps the
    /// palette visually scannable.
    pub icon: &'static str,
    pub group: ActionGroup,
}

// ── The canonical table. ─────────────────────────────────────────────

/// Resume the session currently under the cursor. Every list context
/// has this — Enter defaults to it.
const RESUME: PaletteAction = PaletteAction {
    id: "resume",
    label: "Resume session",
    keybinding: "Enter",
    icon: "▸",
    group: ActionGroup::Selection,
};

const OPEN_PROJECT: PaletteAction = PaletteAction {
    id: "open_project",
    label: "Open project",
    keybinding: "Enter",
    icon: "▸",
    group: ActionGroup::Selection,
};

const EXPORT_MD: PaletteAction = PaletteAction {
    id: "export",
    label: "Export to markdown",
    keybinding: "Ctrl+E",
    icon: "",
    group: ActionGroup::Actions,
};

const RENAME: PaletteAction = PaletteAction {
    id: "rename",
    label: "Rename session",
    keybinding: "r",
    icon: "",
    group: ActionGroup::Actions,
};

const BOOKMARK: PaletteAction = PaletteAction {
    id: "bookmark",
    label: "Bookmark / pin",
    keybinding: "Ctrl+B",
    icon: "",
    group: ActionGroup::Actions,
};

const DELETE: PaletteAction = PaletteAction {
    id: "delete",
    label: "Delete session",
    keybinding: "Ctrl+D",
    icon: "",
    group: ActionGroup::Actions,
};

const COPY_SESSION_ID: PaletteAction = PaletteAction {
    id: "copy_session_id",
    label: "Copy session ID",
    keybinding: "y",
    icon: "",
    group: ActionGroup::Actions,
};

const COPY_PROJECT_PATH: PaletteAction = PaletteAction {
    id: "copy_project_path",
    label: "Copy project path",
    keybinding: "Y",
    icon: "",
    group: ActionGroup::Actions,
};

const OPEN_EDITOR: PaletteAction = PaletteAction {
    id: "open_editor",
    label: "Open project in editor",
    keybinding: "o",
    icon: "",
    group: ActionGroup::Actions,
};

const VIEW_CONVERSATION: PaletteAction = PaletteAction {
    id: "view_conversation",
    label: "View full conversation",
    keybinding: "v",
    icon: "",
    group: ActionGroup::Actions,
};

const TOGGLE_THEME: PaletteAction = PaletteAction {
    id: "toggle_theme",
    label: "Toggle theme",
    keybinding: "t",
    icon: "",
    group: ActionGroup::Actions,
};

const TOGGLE_PREVIEW: PaletteAction = PaletteAction {
    id: "toggle_preview",
    label: "Toggle preview pane",
    keybinding: "Ctrl+P",
    icon: "",
    group: ActionGroup::Actions,
};

const TOGGLE_EXPAND: PaletteAction = PaletteAction {
    id: "toggle_expand",
    label: "Toggle expand / collapse",
    keybinding: "Space",
    icon: "",
    group: ActionGroup::Navigation,
};

const EXPAND_ALL: PaletteAction = PaletteAction {
    id: "expand_all",
    label: "Expand all forks",
    keybinding: "",
    icon: "",
    group: ActionGroup::Navigation,
};

const COLLAPSE_ALL: PaletteAction = PaletteAction {
    id: "collapse_all",
    label: "Collapse all forks",
    keybinding: "",
    icon: "",
    group: ActionGroup::Navigation,
};

const SHOW_HELP: PaletteAction = PaletteAction {
    id: "help",
    label: "Show help",
    keybinding: "?",
    icon: "",
    group: ActionGroup::Help,
};

const QUIT: PaletteAction = PaletteAction {
    id: "quit",
    label: "Quit",
    keybinding: "q",
    icon: "",
    group: ActionGroup::Help,
};

// ── Per-context bundles ──────────────────────────────────────────────

/// Session-list palette contents. Enter, export, rename, bookmark,
/// delete, clipboard, editor, conversation view, theme, help, quit.
const SESSION_LIST_ACTIONS: &[PaletteAction] = &[
    RESUME,
    EXPORT_MD,
    RENAME,
    BOOKMARK,
    DELETE,
    COPY_SESSION_ID,
    COPY_PROJECT_PATH,
    OPEN_EDITOR,
    VIEW_CONVERSATION,
    TOGGLE_THEME,
    SHOW_HELP,
    QUIT,
];

/// Project-list palette — subset of session-list.
const PROJECT_LIST_ACTIONS: &[PaletteAction] = &[
    OPEN_PROJECT,
    COPY_PROJECT_PATH,
    OPEN_EDITOR,
    TOGGLE_THEME,
    SHOW_HELP,
    QUIT,
];

/// Tree palette — adds drill-down actions.
const TREE_ACTIONS: &[PaletteAction] = &[
    RESUME,
    TOGGLE_EXPAND,
    EXPAND_ALL,
    COLLAPSE_ALL,
    COPY_SESSION_ID,
    COPY_PROJECT_PATH,
    OPEN_EDITOR,
    SHOW_HELP,
    QUIT,
];

/// Search palette — adds preview toggle.
const SEARCH_ACTIONS: &[PaletteAction] = &[
    RESUME,
    TOGGLE_PREVIEW,
    COPY_SESSION_ID,
    COPY_PROJECT_PATH,
    OPEN_EDITOR,
    SHOW_HELP,
    QUIT,
];

/// All registered actions for `context`, in palette display order.
pub fn actions_for_context(context: Context) -> &'static [PaletteAction] {
    match context {
        Context::SessionList => SESSION_LIST_ACTIONS,
        Context::ProjectList => PROJECT_LIST_ACTIONS,
        Context::Tree => TREE_ACTIONS,
        Context::Search => SEARCH_ACTIONS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_context_has_actions() {
        for ctx in [
            Context::SessionList,
            Context::ProjectList,
            Context::Tree,
            Context::Search,
        ] {
            let actions = actions_for_context(ctx);
            assert!(!actions.is_empty(), "no actions for {ctx:?}");
        }
    }

    #[test]
    fn action_ids_are_unique_per_context() {
        for ctx in [
            Context::SessionList,
            Context::ProjectList,
            Context::Tree,
            Context::Search,
        ] {
            let actions = actions_for_context(ctx);
            let mut ids: Vec<_> = actions.iter().map(|a| a.id).collect();
            ids.sort();
            let before = ids.len();
            ids.dedup();
            assert_eq!(ids.len(), before, "duplicate action id in {ctx:?}");
        }
    }

    #[test]
    fn session_list_has_core_actions() {
        let acts = actions_for_context(Context::SessionList);
        let ids: Vec<_> = acts.iter().map(|a| a.id).collect();
        for needed in [
            "resume",
            "export",
            "rename",
            "bookmark",
            "delete",
            "copy_session_id",
            "copy_project_path",
            "open_editor",
            "toggle_theme",
            "help",
            "quit",
        ] {
            assert!(ids.contains(&needed), "session-list missing {needed}");
        }
    }

    #[test]
    fn tree_has_expand_actions() {
        let acts = actions_for_context(Context::Tree);
        let ids: Vec<_> = acts.iter().map(|a| a.id).collect();
        for needed in ["toggle_expand", "expand_all", "collapse_all"] {
            assert!(ids.contains(&needed), "tree missing {needed}");
        }
    }

    #[test]
    fn search_has_preview_toggle() {
        let acts = actions_for_context(Context::Search);
        let ids: Vec<_> = acts.iter().map(|a| a.id).collect();
        assert!(ids.contains(&"toggle_preview"));
    }
}
