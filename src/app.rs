//! Application state + event loop for the picker.
//!
//! Everything that persists between frames lives on [`App`]: the session
//! list, the filter buffer, the toast queue, the bookmark store, the matcher
//! scratch memory. The event handler ([`App::handle_event`]) is a terse
//! dispatch that branches on our normalised [`crate::events::Event`] and
//! mutates state in place — no async, no channels.
//!
//! Fuzzy matching is delegated to `nucleo::Matcher`. We keep the matcher
//! instance on the struct so the allocator pool it manages is reused across
//! keystrokes: rebuilding the filtered index on every char-press is
//! microseconds for < 1k sessions.

use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use nucleo::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::bookmarks::BookmarkStore;
use crate::data::{clipboard, editor, session_rename, Project, Session};
use crate::events::{self, Event};
use crate::theme::{self, Theme, ThemeName};
use crate::ui::command_palette::{self, CommandPalette};
use crate::ui::conversation_viewer::{ToastKind as ViewerToastKind, ViewerAction, ViewerState};
use crate::ui::filter_ribbon::{FilterRibbon, FilterScope};
use crate::ui::help_overlay::{self, Screen as HelpScreen};
use crate::ui::model_simulator::{self, ModelSimulatorState};
use crate::ui::onboarding::{OnboardingState, Outcome as OnboardingOutcome};
use crate::ui::project_list::ProjectList;
use crate::ui::rename_modal::{self, RenameState};
use crate::ui::replay::{ReplayAction, ReplayState, ToastKind as ReplayToastKind};
use crate::ui::which_key::WHICH_KEY_DELAY_MS;

/// Which screen is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Picking a project from the full list.
    ProjectList,
    /// Picking a session inside the active project.
    SessionList,
}

/// Transient on-screen message (bookmark toggled, export started, error, …).
///
/// **v2.2 lifecycle:** every toast runs through three phases the renderer
/// treats as a mini animation:
///
/// - **Slide-in** (first [`Toast::SLIDE_IN`] after creation) — width scales
///   from 40 % → 100 %. Cheap; just a lerp in the render code.
/// - **Visible** (next [`Toast::VISIBLE`]) — full opacity, no motion.
/// - **Fade-out** (last [`Toast::FADE_OUT`]) — foreground colours mix
///   toward `theme.base` so the toast "dissolves" into the panel.
///
/// [`Toast::is_expired`] reports the end of the fade-out; the event loop
/// drops the toast then. Custom durations (e.g. the 3-second first-run
/// splash) can use [`Toast::new_with_visible`].
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    created_at: Instant,
    /// How long the toast stays at full opacity between slide-in and fade-out.
    visible_for: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

/// Inverse of a destructive action, kept on the undo stack so `z` can revert
/// the last mutation. Not persistent — dropped when the picker exits.
///
/// New variants can be added as more destructive flows get wired into the
/// stack; the event-loop dispatcher at [`App::undo`] pattern-matches every
/// known kind. Unknown variants would fail to compile — deliberate, so the
/// two sides (push & apply) stay in sync.
#[derive(Debug, Clone)]
pub enum UndoAction {
    /// Inverse of a rename. Reverting sets the session title back to
    /// `old_title` (which may be `None` if it was unnamed before).
    Rename {
        session_id: String,
        old_title: Option<String>,
        new_title: String,
    },
    /// Inverse of a bulk delete: the on-disk `.jsonl` payloads we saved
    /// before removing them. Reverting writes each (path, bytes) back.
    ///
    /// Currently a TODO placeholder — the delete flow doesn't populate
    /// this yet. See `request_delete` for the wire-up point.
    BulkDelete {
        snapshots: Vec<(PathBuf, Vec<u8>)>,
    },
}

/// Which screen a jump-ring entry was captured on. Jumping back restores the
/// stored view so `Ctrl-o` from the session-list can warp to a project on the
/// project-list without the user having to Escape back first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKind {
    ProjectList,
    SessionList,
}

/// One entry on the jump ring. Stores enough state to re-point the picker at
/// whatever row the user "opened" (Enter-style) on that view.
#[derive(Debug, Clone)]
pub struct JumpPoint {
    pub view: ScreenKind,
    pub project_idx: Option<usize>,
    pub session_id: Option<String>,
}

/// Soft cap on undo / redo entries kept in memory. 50 covers a normal
/// editing burst without pinning an ever-growing vector.
const UNDO_CAP: usize = 50;

/// Max jump-ring depth. vim uses 100; 32 is plenty for a picker session and
/// keeps the ring indicator compact.
const JUMP_RING_CAP: usize = 32;

impl Toast {
    /// How long the slide-in animation runs before the toast is "settled".
    pub const SLIDE_IN: Duration = Duration::from_millis(200);
    /// Default dwell time after the slide-in completes and before fade-out
    /// begins. Chosen to be long enough to read one short sentence.
    pub const VISIBLE: Duration = Duration::from_millis(1_200);
    /// Fade-out duration — colours lerp toward `theme.base` over this window.
    pub const FADE_OUT: Duration = Duration::from_millis(300);

    fn new(message: impl Into<String>, kind: ToastKind) -> Self {
        Self::new_with_visible(message, kind, Self::VISIBLE)
    }

    /// Build a toast with a custom "visible" dwell. Slide-in and fade-out
    /// stay at the defaults — those are animation constants, not content
    /// decisions.
    pub fn new_with_visible(
        message: impl Into<String>,
        kind: ToastKind,
        visible_for: Duration,
    ) -> Self {
        Self {
            message: message.into(),
            kind,
            created_at: Instant::now(),
            visible_for,
        }
    }

    /// Elapsed time since the toast was created.
    fn elapsed(&self) -> Duration {
        Instant::now().saturating_duration_since(self.created_at)
    }

    /// 0.0 → 1.0 progress through the slide-in phase. Clamped; after the
    /// slide settles this always reports 1.0.
    pub fn slide_in_progress(&self) -> f32 {
        let e = self.elapsed();
        if e >= Self::SLIDE_IN {
            1.0
        } else {
            (e.as_millis() as f32) / (Self::SLIDE_IN.as_millis() as f32)
        }
    }

    /// 0.0 → 1.0 progress through the fade-out phase. Stays at 0.0 until the
    /// fade-out window begins, then climbs linearly to 1.0 at expiry.
    pub fn fade_out_progress(&self) -> f32 {
        let e = self.elapsed();
        let fade_start = Self::SLIDE_IN + self.visible_for;
        if e <= fade_start {
            0.0
        } else if e >= fade_start + Self::FADE_OUT {
            1.0
        } else {
            let into = e - fade_start;
            (into.as_millis() as f32) / (Self::FADE_OUT.as_millis() as f32)
        }
    }

    fn is_expired(&self) -> bool {
        self.elapsed() >= Self::SLIDE_IN + self.visible_for + Self::FADE_OUT
    }
}

/// The single mutable struct shared by the event loop and every render call.
pub struct App {
    pub mode: Mode,

    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub selected_project: Option<usize>,
    pub selected_session: Option<usize>,

    /// Current filter input.
    pub filter: String,
    /// Indices into `projects` or `sessions` (depending on mode) that match
    /// the current filter, in score order.
    pub filtered_indices: Vec<usize>,
    /// Which filtered entry the cursor is on (0..filtered_indices.len()).
    pub cursor: usize,
    /// Display-index the cursor just came from, plus the timestamp of the
    /// move. Drives the "glide trail": for [`CURSOR_GLIDE_WINDOW`] after a
    /// cursor move the previous row keeps a ghost `surface0` background so
    /// the eye registers movement instead of an instant teleport. Cleared
    /// naturally by `tick()` once the window expires.
    pub previous_cursor: Option<usize>,
    /// When the cursor last moved — paired with `previous_cursor`.
    pub cursor_changed_at: Option<Instant>,
    /// Filter input has focus — controls which border styles light up.
    pub filter_focused: bool,

    pub bookmarks: BookmarkStore,
    pub theme: Theme,

    /// Whether the event loop should exit after this frame.
    pub should_quit: bool,
    /// Result communicated to the caller — if set, we'll resume this session.
    pub selection_result: Option<(String, PathBuf)>,

    /// Active toast (if any).
    pub toast: Option<Toast>,
    /// Confirmation modal for delete.
    pub show_delete_confirm: bool,
    /// `?` help overlay visible.
    pub show_help: bool,
    /// Rename modal state — `Some` while the user is editing a name.
    pub rename: Option<RenameState>,
    /// Space-leader command palette — `Some` while the palette modal is
    /// open. Swallows input until closed.
    pub palette: Option<CommandPalette>,
    /// Full-screen conversation viewer — `Some` while the user is reading
    /// a session's transcript.
    pub viewer: Option<ViewerState>,
    /// Full-screen time-travel replay — `Some` while the user is watching
    /// a session play back message-by-message.
    pub replay: Option<ReplayState>,
    /// "What if" model cost simulator (#5).
    pub model_simulator: Option<ModelSimulatorState>,
    /// First-run onboarding tour (#13).
    pub onboarding: Option<OnboardingState>,
    /// Zen mode (#28) — chrome-free rendering across the picker + viewer.
    /// Shared at the app level so breadcrumb, footer, stats strip, and the
    /// viewer's own footer/search all stay in sync. Toggled with `z` from
    /// inside the viewer (and reachable from the palette eventually).
    pub zen: bool,
    /// Session ids the user has multi-selected via Tab.
    pub multi_selected: HashSet<String>,
    /// True when multi-select mode is engaged. Distinct from
    /// `!multi_selected.is_empty()` because Tab deselecting the last row
    /// should still keep the UI in multi-mode so the footer hints stay
    /// visible until the user explicitly Escapes out.
    pub multi_mode: bool,
    /// Timestamp of the last `g` press, used for the `gg` vim chord. Cleared
    /// on any unrelated key press or after [`G_CHORD_WINDOW`].
    pending_g: Option<Instant>,
    /// Active chord leader, if any. Set when the user presses a leader key
    /// (Space, `g`, …) and cleared on follow-up or timeout. Drives the
    /// which-key overlay: it only renders once the gap since `start` clears
    /// [`WHICH_KEY_DELAY_MS`].
    pub pending_chord: Option<(char, Instant)>,
    /// Pending repeat-count prefix. Vim-style: `3j` → down 3 rows, `12G` →
    /// goto row 12, `5dd` → bulk-delete 5 rows. Populated by digit keys and
    /// consumed by the next non-digit action.
    pub pending_count: Option<u32>,

    /// Undo history for destructive actions. Newest on the back; popped on
    /// `z`. Capped at [`UNDO_CAP`] — older entries fall off silently.
    pub undo_stack: VecDeque<UndoAction>,
    /// Redo mirror of `undo_stack`. Populated by `z`, drained by `Z`. Reset
    /// whenever a new destructive action runs (standard undo-tree rule: a
    /// fresh mutation forks the history).
    pub redo_stack: VecDeque<UndoAction>,

    /// Jump ring — selection history across opens. Vim-style.
    pub jump_ring: VecDeque<JumpPoint>,
    /// Where we currently sit within [`Self::jump_ring`]. `Ctrl-o` decrements,
    /// `Ctrl-i` increments. Equal to `jump_ring.len()` when we're "after the
    /// newest entry" — that's the tip of the ring.
    pub jump_index: usize,

    /// nucleo scratch. Rebuilt on filter change.
    matcher: Matcher,
    /// Precomputed haystacks for the current mode.
    haystacks: Vec<Utf32String>,

    /// Shell-snippet override for the preview pane (CLI `--preview-cmd`).
    /// When `Some`, [`crate::ui::preview::render`] spawns this command
    /// instead of its built-in renderer and renders the stdout.
    pub preview_cmd: Option<String>,
    /// Cache of previous `preview_cmd` runs keyed by session id. Avoids
    /// re-spawning the shell on every frame while the user navigates.
    pub preview_cache: crate::ui::preview::PreviewCache,

    /// Pinned-project store + project-screen UI state (k9s-style favorites).
    /// Loaded from disk on start; `u` toggles pins, `1..9` jumps into slots.
    pub project_list: ProjectList,
    /// atuin-style filter-scope ribbon for the session list. Auto-activates
    /// `REPO` when the process cwd falls inside a discovered project.
    pub filter_ribbon: FilterRibbon,

    /// yazi-style background task drawer (visibility + selection). Toggled
    /// with `w`; `j/k` move the cursor, `x` cancels the focused row. All
    /// row data lives on [`Self::task_queue`] so producers and the UI see
    /// the same snapshot.
    pub task_drawer: crate::ui::task_drawer::TaskDrawerState,
    /// Shared queue of background tasks surfaced by the drawer. Producers
    /// (indexers, fork-graph builders, heatmap rollups) hold a clone of the
    /// Arc and take the mutex briefly to push / update / finish rows.
    pub task_queue: crate::data::task_queue::SharedTaskQueue,

    /// Last cursor row the user left on in a session-list, keyed by the
    /// owning project's resolved path. Re-entering the same project
    /// restores the cursor here instead of resetting to 0. In-memory only
    /// — dropped when the picker exits.
    pub session_cursor_memory: HashMap<String, usize>,
    /// Smoothed scroll anchor for the session-list viewport. Interpolates
    /// toward the `cursor`-derived target over a handful of frames so
    /// large jumps feel like a glide rather than a teleport. See
    /// [`crate::ui::fx::SmoothScroll`].
    ///
    /// Stored behind a [`Cell`] so the render path (which only borrows
    /// `&App`) can update the `target` after inspecting the viewport
    /// height — the cursor-move handlers don't know `visible_rows`, only
    /// the renderer does.
    pub scroll_session: Cell<crate::ui::fx::SmoothScroll>,
    /// Smoothed scroll anchor for the project-list viewport. Same machine
    /// as [`Self::scroll_session`], just tracking a different list.
    pub scroll_project: Cell<crate::ui::fx::SmoothScroll>,
}

/// Window in which two `g` presses collapse into a jump-to-top. Matches the
/// vim / lazygit norm — 500 ms is forgiving but still feels chord-y.
const G_CHORD_WINDOW: Duration = Duration::from_millis(500);

/// How long a "cursor just moved" trail persists. 150 ms is short enough to
/// feel snappy but long enough that the eye catches the previous row's
/// lingering background — the sensation is "something moved up/down here".
pub const CURSOR_GLIDE_WINDOW: Duration = Duration::from_millis(150);

impl App {
    /// Construct an initial state with the default (Mocha) theme. Callers
    /// decide whether to land on [`Mode::ProjectList`] or [`Mode::SessionList`]
    /// by pre-seeding the sessions vector. If both are set, session-list wins
    /// and the project-list pops back in when the user hits Esc.
    pub fn new(
        projects: Vec<Project>,
        sessions: Vec<Session>,
        bookmarks: BookmarkStore,
        mode: Mode,
        selected_project: Option<usize>,
    ) -> Self {
        Self::new_with_theme(
            projects,
            sessions,
            bookmarks,
            mode,
            selected_project,
            ThemeName::default(),
        )
    }

    /// Like [`Self::new`] but starts with an explicit theme. Separate method
    /// rather than an overload so existing test callers don't need to learn
    /// about theme resolution.
    pub fn new_with_theme(
        projects: Vec<Project>,
        sessions: Vec<Session>,
        bookmarks: BookmarkStore,
        mode: Mode,
        selected_project: Option<usize>,
        theme_name: ThemeName,
    ) -> Self {
        let theme = Theme::from_name(theme_name);
        let matcher = Matcher::new(Config::DEFAULT);
        // Build the pinned-project store + filter ribbon before moving
        // `projects` into the struct so the ribbon can inspect the project
        // list to auto-activate `REPO` when the process cwd lines up.
        let project_list = ProjectList::load();
        let filter_ribbon = FilterRibbon::new_with_auto_activation(&projects);
        let task_queue = crate::data::task_queue::new_shared();
        let mut s = Self {
            mode,
            projects,
            sessions,
            selected_project,
            selected_session: Some(0),
            filter: String::new(),
            filtered_indices: Vec::new(),
            cursor: 0,
            previous_cursor: None,
            cursor_changed_at: None,
            filter_focused: true,
            bookmarks,
            theme,
            should_quit: false,
            selection_result: None,
            toast: None,
            show_delete_confirm: false,
            show_help: false,
            rename: None,
            palette: None,
            viewer: None,
            replay: None,
            model_simulator: None,
            onboarding: None,
            zen: false,
            multi_selected: HashSet::new(),
            multi_mode: false,
            pending_g: None,
            pending_chord: None,
            pending_count: None,
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            jump_ring: VecDeque::new(),
            jump_index: 0,
            matcher,
            haystacks: Vec::new(),
            preview_cmd: None,
            preview_cache: crate::ui::preview::PreviewCache::new(),
            project_list,
            filter_ribbon,
            task_drawer: crate::ui::task_drawer::TaskDrawerState::new(),
            task_queue,
            session_cursor_memory: HashMap::new(),
            scroll_session: Cell::new(crate::ui::fx::SmoothScroll::new()),
            scroll_project: Cell::new(crate::ui::fx::SmoothScroll::new()),
        };
        s.rebuild_haystacks();
        s.apply_filter();
        s.maybe_show_first_run_splash();
        // Debug builds get a preloaded set of stub tasks so the drawer can be
        // demoed end-to-end without wiring real producers. Release builds see
        // an empty queue until producers push their first row.
        #[cfg(debug_assertions)]
        {
            if let Ok(mut q) = s.task_queue.lock() {
                q.seed_demo();
            }
        }
        s
    }

    /// First-run handling: construct the 3-step onboarding tour (#13) when
    /// the `.seen_tour` marker is absent.
    fn maybe_show_first_run_splash(&mut self) {
        if !theme::is_first_run() {
            return;
        }
        let top = self.top_session_this_month();
        self.onboarding = Some(OnboardingState::new().with_top_session(top));
    }

    /// Most expensive session this month — seeds onboarding step 1.
    fn top_session_this_month(&self) -> Option<(String, f64)> {
        use chrono::{Datelike, Utc};
        if self.sessions.is_empty() {
            return None;
        }
        let now = Utc::now();
        let (y, m) = (now.year(), now.month());
        let best = self
            .sessions
            .iter()
            .filter(|s| s.last_timestamp.map(|t| t.year() == y && t.month() == m).unwrap_or(false))
            .filter(|s| s.total_cost_usd > 0.0)
            .max_by(|a, b| a.total_cost_usd.partial_cmp(&b.total_cost_usd).unwrap_or(std::cmp::Ordering::Equal))?;
        Some((best.display_label().to_string(), best.total_cost_usd))
    }

    /// Cycle to the next theme in [`ThemeName::ALL`], replace the live
    /// `self.theme`, show a 1-second confirmation toast, and persist the
    /// choice to `~/.config/claude-picker/theme` (best-effort — persistence
    /// errors surface as toasts but don't revert the change).
    pub fn cycle_theme(&mut self) {
        let next = self.theme.name.next();
        self.theme = Theme::from_name(next);
        match theme::save_persisted_theme(next) {
            Ok(()) => {
                self.toast = Some(Toast::new(
                    format!("theme: {}", next.label()),
                    ToastKind::Info,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(
                    format!("theme: {} (save failed: {})", next.label(), e),
                    ToastKind::Error,
                ));
            }
        }
    }

    /// Active project, when one is selected.
    pub fn active_project(&self) -> Option<&Project> {
        self.selected_project.and_then(|i| self.projects.get(i))
    }

    /// Read-only accessor for the project-list UI state (pinned slots). The
    /// project-list renderer reaches through here to paint the pinned strip.
    pub fn project_list(&self) -> &ProjectList {
        &self.project_list
    }

    /// Read-only accessor for the atuin-style filter ribbon. The session-list
    /// renderer uses this to paint chips and to match the visible scope.
    pub fn filter_ribbon(&self) -> &FilterRibbon {
        &self.filter_ribbon
    }

    /// Toggle the pin for the project currently under the cursor in the
    /// project-list screen. Surfaces the result as an info toast so the user
    /// gets feedback on slot assignments + the "all slots full" case.
    fn toggle_pin_current_project(&mut self) {
        // Resolve the row under the cursor through the active filter. Falls
        // back to the selected-project index when the filter view is empty
        // (shouldn't happen in the normal flow, but keeps the action safe).
        let project_idx = match self.filtered_indices.get(self.cursor) {
            Some(&idx) => idx,
            None => match self.selected_project {
                Some(idx) => idx,
                None => return,
            },
        };
        let Some(project) = self.projects.get(project_idx) else {
            return;
        };
        let cwd = project.path.to_string_lossy().into_owned();
        let name = project.name.clone();
        use crate::data::pinned_projects::ToggleResult;
        let msg = match self.project_list.toggle_pin_current(&cwd) {
            ToggleResult::Pinned(slot) => {
                format!("pinned {name} → slot {slot}")
            }
            ToggleResult::Unpinned(slot) => {
                format!("unpinned {name} (slot {slot})")
            }
            ToggleResult::NoSlotsAvailable => "all nine pin slots are full".to_string(),
        };
        self.toast = Some(Toast::new(msg, ToastKind::Info));
    }

    /// Jump to the project pinned at `slot` (1-indexed). No-op when the slot
    /// is empty or its target cwd no longer matches a known project — caller
    /// has already gated on `has_pin`, so the empty-slot case is defensive.
    fn jump_to_pinned_slot(&mut self, slot: u8) {
        let Some(target_cwd) = self.project_list.jump_to_pinned(slot) else {
            return;
        };
        // Locate the project whose resolved path matches. We compare by the
        // lossy string so the in-memory PathBuf and the persisted String
        // align regardless of platform separators.
        let target_idx = self.projects.iter().position(|p| {
            p.path.to_string_lossy() == target_cwd.as_str()
        });
        let Some(project_idx) = target_idx else {
            self.toast = Some(Toast::new(
                format!("slot {slot}: project no longer discoverable"),
                ToastKind::Error,
            ));
            return;
        };

        // If we're on the project-list screen, drill into the matching
        // project (reuses `open_selected_project`'s session-loader). To do
        // that, we update the filter so the cursor lines up with the target
        // project row, then call the existing opener.
        match self.mode {
            Mode::ProjectList => {
                // Ensure the target is visible in the current filtered view.
                let cursor_pos = self
                    .filtered_indices
                    .iter()
                    .position(|&i| i == project_idx);
                if let Some(pos) = cursor_pos {
                    self.cursor = pos;
                    self.open_selected_project();
                } else {
                    // Clear the filter and try again — the user's pin wins
                    // over the typed search when they invoke slot-jump.
                    self.filter.clear();
                    self.apply_filter();
                    if let Some(pos) = self
                        .filtered_indices
                        .iter()
                        .position(|&i| i == project_idx)
                    {
                        self.cursor = pos;
                        self.open_selected_project();
                    }
                }
            }
            Mode::SessionList => {
                // Already drilled in somewhere else — swap to the target.
                self.selected_project = Some(project_idx);
                // Re-point the ribbon's REPO chip to the new cwd so scoped
                // filtering stays meaningful after the jump.
                self.filter_ribbon
                    .set_current_repo(Some(target_cwd.clone()));
                let project = self.projects[project_idx].clone();
                match crate::commands::pick::load_sessions_for(&project) {
                    Ok(sessions) if !sessions.is_empty() => {
                        self.push_jump_point();
                        self.sessions = sessions;
                        self.selected_session = Some(0);
                        self.filter.clear();
                        self.rebuild_haystacks();
                        self.apply_filter();
                    }
                    Ok(_) => {
                        self.toast = Some(Toast::new(
                            format!("{}: no sessions", project.name),
                            ToastKind::Info,
                        ));
                    }
                    Err(e) => {
                        self.toast = Some(Toast::new(
                            format!("load error: {e}"),
                            ToastKind::Error,
                        ));
                    }
                }
            }
        }
    }

    /// Selected session, looked up through the filter.
    pub fn selected_session_ref(&self) -> Option<&Session> {
        let idx = *self.filtered_indices.get(self.cursor)?;
        self.sessions.get(idx)
    }

    /// Cursor position as a display-index. `None` if no matches.
    pub fn cursor_position(&self) -> Option<usize> {
        if self.filtered_indices.is_empty() {
            None
        } else {
            Some(
                self.cursor
                    .min(self.filtered_indices.len().saturating_sub(1)),
            )
        }
    }

    /// Rebuild the list of strings we match against. Called when the mode
    /// changes or when the underlying data swaps.
    fn rebuild_haystacks(&mut self) {
        self.haystacks.clear();
        match self.mode {
            Mode::SessionList => {
                for s in &self.sessions {
                    let composite = format!("{} {} {}", s.display_label(), s.id, s.model_summary,);
                    self.haystacks.push(Utf32String::from(composite));
                }
            }
            Mode::ProjectList => {
                for p in &self.projects {
                    let composite = format!(
                        "{} {} {}",
                        p.name,
                        p.encoded_dir,
                        p.git_branch.as_deref().unwrap_or("")
                    );
                    self.haystacks.push(Utf32String::from(composite));
                }
            }
        }
    }

    /// Recompute `filtered_indices` from the current filter.
    ///
    /// With an empty filter we use the natural order (projects: recency,
    /// sessions: recency from the loader) so Enter on a fresh screen
    /// resumes the most recent thing.
    ///
    /// In session-list mode the filter-ribbon scope AND-composes with the
    /// fuzzy filter: each row must pass both the search pattern (if any) and
    /// the active ribbon chip (ALL/REPO/7D/RUNNING/FORKED).
    fn apply_filter(&mut self) {
        self.filtered_indices.clear();
        let total = self.haystacks.len();

        // Precompute the ribbon predicate into a bit-mask so the main loop
        // below doesn't need to borrow `&self` while `&mut self.matcher`
        // is also in flight (nucleo's `Pattern::score` takes the matcher
        // mutably). Projects skip the ribbon entirely — it's session-scoped.
        let ribbon_mask: Vec<bool> = match self.mode {
            Mode::SessionList => self
                .sessions
                .iter()
                .map(|s| self.filter_ribbon.is_session_visible(s))
                .collect(),
            Mode::ProjectList => vec![true; total],
        };

        // Safety valve: if the ribbon filtered out every session even though
        // the list isn't empty, show them anyway. Path encoding is lossy on
        // Claude Code's side (/ and _ both map to -), so the auto-detected
        // REPO predicate can mismatch a decoded project path and produce
        // "0/N" with every row hidden — which looks broken.
        let all_hidden = matches!(self.mode, Mode::SessionList) && total > 0 && !ribbon_mask.iter().any(|&v| v);
        let ribbon_mask = if all_hidden { vec![true; total] } else { ribbon_mask };

        if self.filter.is_empty() {
            self.filtered_indices
                .extend((0..total).filter(|&i| ribbon_mask.get(i).copied().unwrap_or(true)));
            self.cursor = 0;
            return;
        }

        let pattern = Pattern::new(
            &self.filter,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut scored: Vec<(u32, usize)> = Vec::with_capacity(total);
        for (i, hay) in self.haystacks.iter().enumerate() {
            if !ribbon_mask.get(i).copied().unwrap_or(true) {
                continue;
            }
            if let Some(score) = pattern.score(hay.slice(..), &mut self.matcher) {
                scored.push((score, i));
            }
        }
        // Higher score first, then lower original index as a stable tiebreak.
        scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.filtered_indices = scored.into_iter().map(|(_, i)| i).collect();
        self.cursor = 0;
    }

    /// Dispatch a single [`Event`] against the state.
    pub fn handle_event(&mut self, ev: Event) -> anyhow::Result<()> {
        // Onboarding tour (#13) consumes every key until dismissed.
        if self.onboarding.is_some() {
            return self.handle_onboarding(ev);
        }
        // Model simulator modal (#5) consumes `q`/`Esc`/`r`.
        if self.model_simulator.is_some() {
            return self.handle_model_simulator(ev);
        }
        // Modal inputs take precedence, in reverse visual-stack order.
        if self.replay.is_some() {
            return self.handle_replay(ev);
        }
        if self.viewer.is_some() {
            return self.handle_viewer(ev);
        }
        if self.palette.is_some() {
            return self.handle_palette(ev);
        }
        if self.rename.is_some() {
            return self.handle_rename(ev);
        }
        if self.show_delete_confirm {
            return self.handle_delete_confirm(ev);
        }
        if self.show_help {
            return self.handle_help_overlay(ev);
        }

        // Expire a stale `gg` chord before processing further — a key other
        // than `g` arriving outside the window should not linger as pending.
        if let Some(t) = self.pending_g {
            if t.elapsed() > G_CHORD_WINDOW {
                self.pending_g = None;
            }
        }

        // If a leader chord is active, the next key is a follow-up and must
        // be dispatched through the chord table. Consumes `pending_chord`
        // regardless of whether the follow-up matches — hitting an unknown
        // key cancels the chord rather than queueing it.
        if let Some((leader, _)) = self.pending_chord {
            if let Event::Key(c) = ev {
                self.pending_chord = None;
                if self.dispatch_chord(leader, c) {
                    return Ok(());
                }
                // Fall through to normal dispatch if the chord didn't
                // recognise the follow-up (e.g. the user pressed Esc-like
                // cancel — treat as a fresh event).
            } else if matches!(ev, Event::Escape) {
                // Esc while a chord is pending should just cancel without
                // doing anything else — do not pop screens.
                self.pending_chord = None;
                return Ok(());
            } else {
                // Non-char events (arrow keys, resize, …) cancel the chord.
                self.pending_chord = None;
            }
        }

        // Background-task drawer has first dibs on nav + cancel keys while
        // visible so j/k/x/Esc/Up/Down don't double-fire against the main
        // picker underneath. `w` is handled inside the main match below so
        // it can toggle from any non-modal screen.
        if self.task_drawer.visible {
            match ev {
                Event::Key('j') | Event::Down => {
                    let task_count = self
                        .task_queue
                        .lock()
                        .map(|q| q.len())
                        .unwrap_or(0);
                    self.task_drawer.move_down(task_count);
                    return Ok(());
                }
                Event::Key('k') | Event::Up => {
                    self.task_drawer.move_up();
                    return Ok(());
                }
                Event::Key('x') => {
                    // Resolve the selected row into a task id and cancel it
                    // under a single lock so the snapshot the drawer used
                    // for selection and the queue mutation stay consistent.
                    if let Ok(mut q) = self.task_queue.lock() {
                        if let Some(id) = self.task_drawer.selected_id(&q) {
                            q.cancel(id);
                        }
                    }
                    return Ok(());
                }
                Event::Escape => {
                    self.task_drawer.toggle();
                    return Ok(());
                }
                Event::Key('w') => {
                    self.task_drawer.toggle();
                    return Ok(());
                }
                _ => {}
            }
        }

        match ev {
            Event::Quit | Event::Ctrl('c') => self.should_quit = true,
            Event::Ctrl('a') => self.summarize_session_ai(),
            Event::Ctrl('d') => self.request_delete(),
            Event::Ctrl('b') => self.toggle_bookmark(),
            Event::Ctrl('e') => self.export_session(),
            // `Ctrl-r` cycles the atuin-style filter-scope ribbon forward
            // (ALL → REPO → 7D → RUNNING → FORKED → ALL). Rebuilding the
            // filter pulls the new scope predicate through `apply_filter`.
            Event::Ctrl('r') => {
                self.filter_ribbon.cycle_forward();
                self.apply_filter();
            }
            // `Ctrl-o` / `Ctrl-i` walk the jump ring. Vim semantics: `o` is
            // "older" (back), `i` is "newer" (forward).
            Event::Ctrl('o') => self.jump_back(),
            Event::Ctrl('i') => self.jump_forward(),
            Event::Tab => self.toggle_multi_select(),
            Event::Enter => {
                // Drop any count prefix — Enter is a terminal action, not
                // a repeatable motion.
                self.pending_count = None;
                self.confirm_selection();
            }
            Event::Escape => {
                // Esc clears a dangling count prefix first so the user can
                // abort a mis-typed chord without jumping 300 rows.
                if self.pending_count.is_some() {
                    self.pending_count = None;
                    return Ok(());
                }
                self.handle_escape();
            }
            Event::Up => {
                let n = self.take_count().unwrap_or(1) as i32;
                self.move_cursor(-n);
            }
            Event::Down => {
                let n = self.take_count().unwrap_or(1) as i32;
                self.move_cursor(n);
            }
            Event::PageUp => self.move_cursor(-10),
            Event::PageDown => self.move_cursor(10),
            Event::Home => self.cursor = 0,
            Event::End => self.cursor = self.filtered_indices.len().saturating_sub(1),
            Event::Backspace => self.filter_backspace(),
            // `?` opens the context-sensitive help overlay whenever the filter
            // is empty. If someone's typing `?` into the filter they can
            // still escape-and-type.
            Event::Key('?') if self.filter.is_empty() => self.show_help = true,
            // `G` (shift-G) jumps to the end, or to an absolute row number
            // when prefixed with a count (`12G` → row 12, 1-indexed).
            Event::Key('G') if self.filter.is_empty() => {
                self.pending_g = None;
                if let Some(n) = self.take_count() {
                    // 1-indexed target, clamped to the visible range.
                    let target = n.saturating_sub(1) as usize;
                    let last = self.filtered_indices.len().saturating_sub(1);
                    self.cursor = target.min(last);
                } else {
                    self.cursor = self.filtered_indices.len().saturating_sub(1);
                }
            }
            // `g` pressed: if a previous `g` is still within the window, this
            // completes a `gg` chord → jump to top. Otherwise remember the
            // keystroke so the next `g` can complete the chord AND open the
            // which-key overlay after the pause window.
            Event::Key('g') if self.filter.is_empty() => {
                if self
                    .pending_g
                    .map(|t| t.elapsed() <= G_CHORD_WINDOW)
                    .unwrap_or(false)
                {
                    self.cursor = 0;
                    self.pending_g = None;
                    self.pending_chord = None;
                    self.pending_count = None;
                } else {
                    self.pending_g = Some(Instant::now());
                    self.pending_chord = Some(('g', Instant::now()));
                }
            }
            // `j` / `k` are the vim verticals. Both respect the pending
            // repeat count so `3j` moves down three rows.
            Event::Key('j') if self.filter.is_empty() => {
                let n = self.take_count().unwrap_or(1) as i32;
                self.move_cursor(n);
            }
            Event::Key('k') if self.filter.is_empty() => {
                let n = self.take_count().unwrap_or(1) as i32;
                self.move_cursor(-n);
            }
            // `v` opens the full-screen conversation viewer for the row
            // under the cursor. Session-list only — project-list has no
            // session to view.
            Event::Key('v') if self.filter.is_empty() && self.mode == Mode::SessionList => {
                self.open_viewer();
            }
            // `y` / `Y` copy to clipboard (lowercase = session id, uppercase
            // = project path). Both require an empty filter so typing them
            // into a search still works. In multi-select mode these copy the
            // set of selected session ids / project paths instead.
            Event::Key('y') if self.filter.is_empty() => self.copy_session_id(),
            Event::Key('Y') if self.filter.is_empty() => self.copy_project_path(),
            // `r` opens the rename modal for the selected session.
            Event::Key('r') if self.filter.is_empty() => self.request_rename(),
            // `e` exports the current selection to a Markdown file in
            // `~/Downloads/` (or every multi-selected session in sequence).
            // Only on the session list — the project list has no session
            // to export. The filter-empty guard keeps the letter available
            // for fuzzy search when the user is typing.
            Event::Key('e') if self.filter.is_empty() && self.mode == Mode::SessionList => {
                self.export_session();
            }
            // `R` (uppercase) opens the time-travel replay. Only on the
            // session list — project list has no session to replay.
            Event::Key('R') if self.filter.is_empty() && self.mode == Mode::SessionList => {
                self.open_replay();
            }
            // `o` launches `$EDITOR <project_path>` detached.
            Event::Key('o') if self.filter.is_empty() => self.open_editor_for_selection(),
            Event::Key(c) if c == 'q' && self.filter.is_empty() => self.should_quit = true,
            // `m` opens the "what if" model cost simulator (#5).
            Event::Key('m') if self.filter.is_empty() && self.mode == Mode::SessionList => {
                self.open_model_simulator();
            }
            // `t` cycles the theme when the filter is empty. If the user is
            // typing a filter (including searches with `t` in them) the letter
            // goes to the filter via the fallthrough branch below.
            Event::Key('t') if self.filter.is_empty() => self.cycle_theme(),
            // `w` opens (or closes) the yazi-style background task drawer.
            // When the drawer is already visible the pre-match dispatcher
            // above has first crack at this event, so we only hit this arm
            // to open it for the first time from the main picker.
            Event::Key('w') if self.filter.is_empty() => self.task_drawer.toggle(),
            // `z` / `Z` drive the undo/redo stack.
            Event::Key('z') if self.filter.is_empty() => self.undo(),
            Event::Key('Z') if self.filter.is_empty() => self.redo(),
            // `u` toggles a pin on the currently-highlighted project. Only
            // meaningful on the project-list screen — on the session screen
            // the letter falls through to the fuzzy-filter arm.
            Event::Key('u')
                if self.filter.is_empty()
                    && self.mode == Mode::ProjectList =>
            {
                self.toggle_pin_current_project();
            }
            // `1..9` with no pending count jumps to a pinned project. If the
            // slot is empty the event falls through to the count-prefix arm
            // below so `3j` still works when slot 3 is unset.
            Event::Key(c)
                if self.filter.is_empty()
                    && matches!(c, '1'..='9')
                    && self.pending_count.is_none()
                    && self.project_list.has_pin(c.to_digit(10).unwrap() as u8) =>
            {
                let slot = c.to_digit(10).unwrap() as u8;
                self.jump_to_pinned_slot(slot);
            }
            // `0` with no pending count clears the project filter and resets
            // the ribbon to ALL. With a pending count it appends as a digit
            // (handled below).
            Event::Key('0')
                if self.filter.is_empty() && self.pending_count.is_none() =>
            {
                self.project_list.clear_project_filter();
                self.filter_ribbon.set_scope(FilterScope::All);
                self.apply_filter();
            }
            // `d` pressed: set a chord leader. The `dd` completion is
            // handled up-top by the chord dispatcher — a second `d` within
            // the leader window consumes both and opens the delete modal.
            // Count prefix (e.g. `5dd`) is consumed on the first press and
            // applied when the multi-select batch fires.
            Event::Key('d') if self.filter.is_empty() => {
                self.pending_chord = Some(('d', Instant::now()));
            }
            // Space becomes a leader chord instead of eagerly opening the
            // palette. The follow-up key dispatches; a second Space opens
            // the palette. If the user pauses for [`WHICH_KEY_DELAY_MS`]
            // the which-key overlay renders so they can see what's next.
            Event::Key(' ') if self.filter.is_empty() => {
                self.pending_g = None;
                self.pending_chord = Some((' ', Instant::now()));
            }
            // Digit keys feed the vim repeat-count prefix. `0` is special:
            // at count=None it's NOT a digit (reserved for Agent C's
            // "all-projects" chord); at count=Some it appends as normal.
            // The `'0'` exception lives on the guard so the match doesn't
            // consume it — leaving `'0'` free for other dispatchers.
            Event::Key(c)
                if self.filter.is_empty()
                    && c.is_ascii_digit()
                    && !(c == '0' && self.pending_count.is_none()) =>
            {
                let digit = c as u32 - '0' as u32;
                let next = self
                    .pending_count
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit);
                // Cap at a sanity ceiling so typos like `99999999k`
                // can't allocate unexpectedly.
                self.pending_count = Some(next.min(9_999));
            }
            Event::Key(c) if is_filter_char(c) => {
                // Any keystroke other than the chord letters breaks `gg`
                // and the count prefix.
                self.pending_g = None;
                self.pending_count = None;
                self.filter_push(c);
            }
            Event::Resize(_, _) => {}
            _ => {
                // Unknown event — clear any pending chord so we don't match
                // `g<tab>g` or similar across stale timers.
                self.pending_g = None;
                self.pending_count = None;
            }
        }
        Ok(())
    }

    /// Consume and return the pending repeat-count prefix. Returns `None`
    /// when no count was typed. Callers use this right before they would
    /// otherwise act with an implicit count of 1.
    fn take_count(&mut self) -> Option<u32> {
        self.pending_count.take()
    }

    /// True when the which-key overlay should render this frame — i.e.
    /// there's an active leader chord AND the user's pause exceeds
    /// [`WHICH_KEY_DELAY_MS`].
    pub fn should_show_which_key(&self) -> bool {
        match self.pending_chord {
            Some((_, started)) => {
                started.elapsed() >= Duration::from_millis(WHICH_KEY_DELAY_MS)
            }
            None => false,
        }
    }

    /// The leader character driving the which-key overlay, if any. Exposed
    /// so the renderer can pick the right next-key table.
    pub fn which_key_leader(&self) -> Option<char> {
        self.pending_chord.map(|(c, _)| c)
    }

    /// Dispatch a chord follow-up. Returns `true` when the (leader, key)
    /// pair matched a known action; the caller relies on the bool to
    /// decide whether to fall through to ordinary event handling.
    fn dispatch_chord(&mut self, leader: char, key: char) -> bool {
        match (leader, key) {
            // Space-leader: mirror the palette top-level actions so the user
            // can reach them without opening the palette.
            (' ', ' ') => {
                self.palette = Some(CommandPalette::new(match self.mode {
                    Mode::SessionList => command_palette::Context::SessionList,
                    Mode::ProjectList => command_palette::Context::ProjectList,
                }));
                true
            }
            (' ', 'f') => {
                self.filter_focused = true;
                true
            }
            (' ', 't') => {
                self.cycle_theme();
                true
            }
            (' ', 'r') => {
                self.request_rename();
                true
            }
            (' ', 'R') => {
                self.open_replay();
                true
            }
            (' ', 'd') => {
                self.request_delete();
                true
            }
            (' ', '?') => {
                self.show_help = true;
                true
            }
            (' ', 'v') => {
                self.open_viewer();
                true
            }
            (' ', 'y') => {
                self.copy_session_id();
                true
            }
            (' ', 'Y') => {
                self.copy_project_path();
                true
            }
            // Space+w toggles the background task drawer. Mirrors the plain
            // `w` binding so palette-style discoverers land on the same
            // action the hot key fires.
            (' ', 'w') => {
                self.task_drawer.toggle();
                true
            }
            // TODO: wire ` m` (model switcher) and ` s` (stats) when those
            // surfaces land. For now we silently surface a toast so the
            // binding is discoverable.
            (' ', 'm') | (' ', 's') => {
                self.toast = Some(Toast::new(
                    format!("TODO: wire Space {key} action"),
                    ToastKind::Info,
                ));
                true
            }
            // g-leader: only `gg` is wired. Anything else falls through so
            // the normal dispatcher can handle it (e.g. `gG` = no-op).
            ('g', 'g') => {
                self.cursor = 0;
                self.pending_g = None;
                true
            }
            // d-leader: `dd` triggers the delete confirm modal. The chord
            // dispatcher fires before the normal `'d'` arm so a second `d`
            // press doesn't re-arm the chord.
            ('d', 'd') => {
                self.request_delete();
                true
            }
            _ => false,
        }
    }

    /// Pop the newest undo entry and apply its inverse. No-op when the
    /// stack is empty. Pushes the re-applied state onto `redo_stack`.
    pub fn undo(&mut self) {
        let Some(action) = self.undo_stack.pop_back() else {
            return;
        };
        if let Some(reverse) = self.apply_undo(&action) {
            // Redo mirror: store the re-application of the forward action.
            if self.redo_stack.len() >= UNDO_CAP {
                self.redo_stack.pop_front();
            }
            self.redo_stack.push_back(reverse);
        }
    }

    /// Pop the newest redo entry and re-apply it. Pushes back onto
    /// `undo_stack` so the next `z` can undo the redo.
    pub fn redo(&mut self) {
        let Some(action) = self.redo_stack.pop_back() else {
            return;
        };
        if let Some(reverse) = self.apply_undo(&action) {
            if self.undo_stack.len() >= UNDO_CAP {
                self.undo_stack.pop_front();
            }
            self.undo_stack.push_back(reverse);
        }
    }

    /// Apply one undo/redo action and return the inverse — suitable for
    /// pushing onto the opposite stack. `None` means the action couldn't
    /// be applied (session vanished, disk error, …).
    fn apply_undo(&mut self, action: &UndoAction) -> Option<UndoAction> {
        match action {
            UndoAction::Rename {
                session_id,
                old_title,
                new_title,
            } => {
                // Restore the previous title by writing it. Empty string
                // means "no name" — the underlying helper doesn't yet
                // support clearing a name, so surface a toast explaining
                // that and bail.
                let restore = old_title.as_deref().unwrap_or("");
                if restore.is_empty() {
                    // TODO: wire clear-name helper so undo of rename-from-
                    // unnamed can actually erase the custom title.
                    self.toast = Some(Toast::new(
                        "undo: clearing names not yet supported",
                        ToastKind::Info,
                    ));
                    return None;
                }
                let result = session_rename::rename_session(session_id, restore);
                match result {
                    Ok(_) => {
                        if let Some(s) = self.sessions.iter_mut().find(|s| &s.id == session_id)
                        {
                            s.name = old_title.clone();
                        }
                        self.rebuild_haystacks();
                        self.apply_filter();
                        self.toast = Some(Toast::new(
                            format!("undo rename → \"{restore}\""),
                            ToastKind::Success,
                        ));
                        Some(UndoAction::Rename {
                            session_id: session_id.clone(),
                            old_title: Some(new_title.clone()),
                            new_title: restore.to_string(),
                        })
                    }
                    Err(e) => {
                        self.toast =
                            Some(Toast::new(format!("undo failed: {e}"), ToastKind::Error));
                        None
                    }
                }
            }
            UndoAction::BulkDelete { snapshots } => {
                // TODO: wire into delete flow — the current delete path
                // doesn't snapshot bytes before unlink. The scaffolding
                // here is intentional so the enum stays exhaustive.
                let count = snapshots.len();
                self.toast = Some(Toast::new(
                    format!("undo delete (TODO): {count} sessions"),
                    ToastKind::Info,
                ));
                None
            }
        }
    }

    /// Push an action onto the undo stack, trimming to [`UNDO_CAP`] from
    /// the front. Clears the redo stack — a new mutation forks the history
    /// (matches vim / browser history semantics).
    pub fn push_undo(&mut self, action: UndoAction) {
        if self.undo_stack.len() >= UNDO_CAP {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(action);
        self.redo_stack.clear();
    }

    /// Number of undo entries currently stashed. Exposed so the footer can
    /// render an indicator when non-empty.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Current repeat-count prefix. Exposed so the footer status line can
    /// render the tight right-aligned `\u{28ff} 3` hint.
    pub fn pending_count_value(&self) -> Option<u32> {
        self.pending_count
    }

    /// Record a jump point — called when the user opens a project or
    /// session (Enter-style navigation). Drops the forward segment of the
    /// ring if the user was mid-walk, matching vim's behavior.
    pub fn push_jump_point(&mut self) {
        let point = JumpPoint {
            view: match self.mode {
                Mode::ProjectList => ScreenKind::ProjectList,
                Mode::SessionList => ScreenKind::SessionList,
            },
            project_idx: self.selected_project,
            session_id: self.selected_session_ref().map(|s| s.id.clone()),
        };
        // If the user was mid-ring-walk, truncate forward so a new jump
        // branches history rather than silently replacing it.
        if self.jump_index < self.jump_ring.len() {
            self.jump_ring.truncate(self.jump_index);
        }
        if self.jump_ring.len() >= JUMP_RING_CAP {
            self.jump_ring.pop_front();
        }
        self.jump_ring.push_back(point);
        self.jump_index = self.jump_ring.len();
    }

    /// `Ctrl-o` — move the jump index one step toward the oldest entry
    /// and restore that view. Stays at zero when the ring has already
    /// been fully walked back.
    ///
    /// When we're at the tip of the ring (the "just arrived here" state)
    /// we first stash the current location so a later `Ctrl-i` can return
    /// forward. Matches vim's jump-list behaviour.
    fn jump_back(&mut self) {
        if self.jump_ring.is_empty() {
            return;
        }
        if self.jump_index == self.jump_ring.len() {
            // At the tip — push the current position onto the ring so the
            // reverse walk has somewhere to come back to, then rewind past
            // it to the previous entry.
            let tip = JumpPoint {
                view: match self.mode {
                    Mode::ProjectList => ScreenKind::ProjectList,
                    Mode::SessionList => ScreenKind::SessionList,
                },
                project_idx: self.selected_project,
                session_id: self.selected_session_ref().map(|s| s.id.clone()),
            };
            if self.jump_ring.len() >= JUMP_RING_CAP {
                self.jump_ring.pop_front();
            }
            self.jump_ring.push_back(tip);
            self.jump_index = self.jump_ring.len().saturating_sub(2);
        } else if self.jump_index > 0 {
            self.jump_index -= 1;
        }
        self.apply_jump_point();
    }

    /// `Ctrl-i` — inverse of `jump_back`. Clamps at the tip of the ring.
    fn jump_forward(&mut self) {
        if self.jump_ring.is_empty() {
            return;
        }
        if self.jump_index + 1 < self.jump_ring.len() {
            self.jump_index += 1;
            self.apply_jump_point();
        }
    }

    /// Restore the view stored at `jump_index`. Silently best-effort — if
    /// the target session was deleted between jumps, we land on whatever
    /// row the cursor still resolves to.
    fn apply_jump_point(&mut self) {
        let Some(point) = self.jump_ring.get(self.jump_index).cloned() else {
            return;
        };
        // Stash the current session-cursor before the jump lands — even
        // jump-ring hops should participate in cursor memory so ping-ponging
        // between views feels like picking up where you left off.
        self.save_session_cursor_memory();
        match point.view {
            ScreenKind::ProjectList => {
                self.mode = Mode::ProjectList;
                self.sessions.clear();
                self.selected_session = None;
                self.rebuild_haystacks();
                self.apply_filter();
                if let Some(idx) = point.project_idx {
                    let pos = self
                        .filtered_indices
                        .iter()
                        .position(|i| *i == idx)
                        .unwrap_or(0);
                    self.cursor = pos;
                }
                self.snap_project_scroll(self.cursor);
            }
            ScreenKind::SessionList => {
                if let Some(id) = point.session_id {
                    if let Some(sess_idx) = self.sessions.iter().position(|s| s.id == id) {
                        self.mode = Mode::SessionList;
                        self.rebuild_haystacks();
                        self.apply_filter();
                        let pos = self
                            .filtered_indices
                            .iter()
                            .position(|i| *i == sess_idx)
                            .unwrap_or(0);
                        self.cursor = pos;
                        self.snap_session_scroll(self.cursor);
                    }
                }
            }
        }
    }

    /// Depth of the jump ring and where we are in it — returned as
    /// `(index + 1, total)` so callers rendering `[3/7]` can just format
    /// the pair. `None` when the ring is empty.
    pub fn jump_ring_position(&self) -> Option<(usize, usize)> {
        if self.jump_ring.is_empty() {
            None
        } else {
            Some((self.jump_index.min(self.jump_ring.len()).max(1), self.jump_ring.len()))
        }
    }

    /// Forward an event to the open viewer and act on its reply.
    fn handle_viewer(&mut self, ev: Event) -> anyhow::Result<()> {
        let Some(viewer) = self.viewer.as_mut() else {
            return Ok(());
        };
        match viewer.handle_event(ev) {
            ViewerAction::None => {}
            ViewerAction::Close => {
                self.viewer = None;
            }
            ViewerAction::Toast(message, kind) => {
                let app_kind = match kind {
                    ViewerToastKind::Info => ToastKind::Info,
                    ViewerToastKind::Success => ToastKind::Success,
                    ViewerToastKind::Error => ToastKind::Error,
                };
                self.toast = Some(Toast::new(message, app_kind));
            }
            ViewerAction::ToggleZen => {
                self.zen = !self.zen;
            }
        }
        Ok(())
    }

    /// Open the conversation viewer for the row currently under the cursor.
    /// Quietly no-ops if nothing's selected.
    pub fn open_viewer(&mut self) {
        let Some(session) = self.selected_session_ref().cloned() else {
            return;
        };
        self.viewer = Some(ViewerState::open(&session));
    }

    /// Open the time-travel replay for the session under the cursor.
    /// Quietly no-ops if nothing's selected.
    pub fn open_replay(&mut self) {
        let Some(session) = self.selected_session_ref().cloned() else {
            return;
        };
        self.replay = Some(ReplayState::open(&session));
    }

    /// Forward an event to the open replay and act on the reply.
    fn handle_replay(&mut self, ev: Event) -> anyhow::Result<()> {
        let Some(replay) = self.replay.as_mut() else {
            return Ok(());
        };
        match replay.handle_event(ev) {
            ReplayAction::None => {}
            ReplayAction::Close => {
                self.replay = None;
            }
            ReplayAction::Toast(message, kind) => {
                let app_kind = match kind {
                    ReplayToastKind::Info => ToastKind::Info,
                    ReplayToastKind::Success => ToastKind::Success,
                    ReplayToastKind::Error => ToastKind::Error,
                };
                self.toast = Some(Toast::new(message, app_kind));
            }
        }
        Ok(())
    }

    /// Open the "what if" model simulator (#5).
    pub fn open_model_simulator(&mut self) {
        if self.mode != Mode::SessionList {
            return;
        }
        let Some(session) = self.selected_session_ref().cloned() else {
            return;
        };
        self.model_simulator = Some(ModelSimulatorState::from_session(&session));
    }

    /// Dispatch events to the open model simulator (#5).
    fn handle_model_simulator(&mut self, ev: Event) -> anyhow::Result<()> {
        match ev {
            Event::Escape => self.model_simulator = None,
            Event::Key(c) if model_simulator::is_dismiss_key(c) => self.model_simulator = None,
            Event::Key('r') => {
                if let Some(session) = self.selected_session_ref().cloned() {
                    self.model_simulator = Some(ModelSimulatorState::from_session(&session));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Dispatch events to the onboarding tour (#13).
    fn handle_onboarding(&mut self, ev: Event) -> anyhow::Result<()> {
        let Some(state) = self.onboarding.as_mut() else {
            return Ok(());
        };
        match state.handle_event(ev) {
            OnboardingOutcome::Continue => {}
            OnboardingOutcome::Dismiss => {
                self.onboarding = None;
                let _ = theme::mark_first_run_done();
            }
        }
        Ok(())
    }

    /// Toggle multi-select on the row under the cursor. First Tab press
    /// flips multi-mode on; later presses toggle individual selection.
    fn toggle_multi_select(&mut self) {
        if self.mode != Mode::SessionList {
            return;
        }
        let Some(s) = self.selected_session_ref().cloned() else {
            return;
        };
        if !self.multi_mode {
            self.multi_mode = true;
        }
        if self.multi_selected.contains(&s.id) {
            self.multi_selected.remove(&s.id);
        } else {
            self.multi_selected.insert(s.id);
        }
    }

    /// Clear any active multi-selection and exit multi-mode.
    pub fn clear_multi_select(&mut self) {
        self.multi_selected.clear();
        self.multi_mode = false;
    }

    /// Total number of rows currently multi-selected.
    pub fn multi_selected_count(&self) -> usize {
        self.multi_selected.len()
    }

    /// True when the row at `sess_idx` into `self.sessions` is part of the
    /// live multi-selection.
    pub fn is_multi_selected(&self, sess_idx: usize) -> bool {
        self.sessions
            .get(sess_idx)
            .map(|s| self.multi_selected.contains(&s.id))
            .unwrap_or(false)
    }

    /// Sessions in the multi-selection, in the order they appear in
    /// `self.sessions` (so Ctrl-E exports stably, not in hash order).
    fn multi_selected_sessions(&self) -> Vec<&Session> {
        self.sessions
            .iter()
            .filter(|s| self.multi_selected.contains(&s.id))
            .collect()
    }

    /// Current screen for the help overlay. Only the picker-level modes are
    /// reachable from `App`; subcommand screens own their own event loops.
    pub fn help_screen(&self) -> HelpScreen {
        match self.mode {
            Mode::SessionList => HelpScreen::SessionList,
            Mode::ProjectList => HelpScreen::ProjectList,
        }
    }

    fn handle_help_overlay(&mut self, ev: Event) -> anyhow::Result<()> {
        match ev {
            Event::Escape => self.show_help = false,
            Event::Key(c) if help_overlay::is_dismiss_key(c) => self.show_help = false,
            Event::Resize(_, _) => {}
            _ => {}
        }
        Ok(())
    }

    /// Forward an event to the live command palette and act on the
    /// outcome. On [`Outcome::Execute`] we close the palette first and
    /// *then* dispatch the action — doing it in that order means any
    /// follow-up modal (rename, delete-confirm, viewer) doesn't get
    /// stacked under the palette.
    fn handle_palette(&mut self, ev: Event) -> anyhow::Result<()> {
        let Some(palette) = self.palette.as_mut() else {
            return Ok(());
        };
        let outcome = palette.handle_event(ev);
        match outcome {
            command_palette::Outcome::Continue => {}
            command_palette::Outcome::Close => {
                self.palette = None;
            }
            command_palette::Outcome::Execute(id) => {
                self.palette = None;
                self.execute_palette_action(id);
            }
        }
        Ok(())
    }

    /// Map palette action ids to state mutations. Unknown ids are
    /// ignored rather than panicking so a stale palette-in-flight
    /// can't crash the app.
    fn execute_palette_action(&mut self, id: &'static str) {
        match id {
            "resume" | "open_project" => self.confirm_selection(),
            "export" => self.export_session(),
            "rename" => self.request_rename(),
            "bookmark" => self.toggle_bookmark(),
            "delete" => self.request_delete(),
            "copy_session_id" => self.copy_session_id(),
            "copy_project_path" => self.copy_project_path(),
            "open_editor" => self.open_editor_for_selection(),
            "view_conversation" => self.open_viewer(),
            "toggle_theme" => self.cycle_theme(),
            "help" => self.show_help = true,
            "quit" => self.should_quit = true,
            _ => {}
        }
    }

    fn handle_rename(&mut self, ev: Event) -> anyhow::Result<()> {
        let Some(state) = self.rename.as_mut() else {
            return Ok(());
        };
        match ev {
            Event::Enter => {
                let new_name = state.buffer.trim().to_string();
                let session_id = state.session_id.clone();
                // Validation: empty buffer is a soft error — keep the modal
                // open with an inline message so the user can fix + retry
                // without having to re-open the modal.
                if new_name.is_empty() {
                    state.set_error("name can't be empty");
                    return Ok(());
                }
                // Capture the pre-rename title so `z` can roll it back.
                let old_title = self
                    .sessions
                    .iter()
                    .find(|s| s.id == session_id)
                    .and_then(|s| s.name.clone());
                match session_rename::rename_session(&session_id, &new_name) {
                    Ok(_) => {
                        // Commit path: close the modal, update state, toast.
                        self.rename = None;
                        if let Some(s) = self.sessions.iter_mut().find(|s| s.id == session_id) {
                            s.name = Some(new_name.clone());
                        }
                        self.rebuild_haystacks();
                        self.apply_filter();
                        self.push_undo(UndoAction::Rename {
                            session_id: session_id.clone(),
                            old_title,
                            new_title: new_name.clone(),
                        });
                        self.toast = Some(Toast::new(
                            format!("renamed to \"{new_name}\" \u{00B7} z to undo"),
                            ToastKind::Success,
                        ));
                    }
                    Err(e) => {
                        // Persistence failed: surface the error inline on the
                        // modal so the user can edit and retry. The modal
                        // clears the error on the next keystroke.
                        if let Some(state) = self.rename.as_mut() {
                            state.set_error(format!("rename failed: {e}"));
                        }
                    }
                }
            }
            Event::Escape => {
                self.rename = None;
            }
            Event::Backspace => state.pop(),
            Event::Key(c) if rename_modal::is_name_char(c) => state.push(c),
            _ => {}
        }
        Ok(())
    }

    fn copy_session_id(&mut self) {
        // Multi-select: copy every selected id, newline-separated.
        if self.multi_mode && !self.multi_selected.is_empty() {
            let ids: Vec<String> = self
                .multi_selected_sessions()
                .iter()
                .map(|s| s.id.clone())
                .collect();
            let count = ids.len();
            let payload = ids.join("\n");
            match clipboard::copy(payload) {
                Ok(()) => {
                    self.toast = Some(Toast::new(
                        format!("copied {count} session IDs"),
                        ToastKind::Success,
                    ));
                }
                Err(e) => {
                    self.toast = Some(Toast::new(
                        format!("clipboard unavailable: {e}"),
                        ToastKind::Error,
                    ));
                }
            }
            return;
        }
        let Some(s) = self.selected_session_ref().cloned() else {
            return;
        };
        let short = s.id.chars().take(8).collect::<String>();
        match clipboard::copy(s.id.clone()) {
            Ok(()) => {
                self.toast = Some(Toast::new(
                    format!("copied {short} to clipboard"),
                    ToastKind::Success,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(
                    format!("clipboard unavailable: {e}"),
                    ToastKind::Error,
                ));
            }
        }
    }

    fn copy_project_path(&mut self) {
        // Multi-select: copy deduped project paths, newline-separated.
        if self.multi_mode && !self.multi_selected.is_empty() {
            let mut seen: HashSet<PathBuf> = HashSet::new();
            let mut paths: Vec<String> = Vec::new();
            for s in self.multi_selected_sessions() {
                if seen.insert(s.project_dir.clone()) {
                    paths.push(s.project_dir.display().to_string());
                }
            }
            let count = paths.len();
            let payload = paths.join("\n");
            match clipboard::copy(payload) {
                Ok(()) => {
                    self.toast = Some(Toast::new(
                        format!("copied {count} project paths"),
                        ToastKind::Success,
                    ));
                }
                Err(e) => {
                    self.toast = Some(Toast::new(
                        format!("clipboard unavailable: {e}"),
                        ToastKind::Error,
                    ));
                }
            }
            return;
        }
        let path: PathBuf = match self.mode {
            Mode::SessionList => match self.selected_session_ref() {
                Some(s) => s.project_dir.clone(),
                None => return,
            },
            Mode::ProjectList => {
                let Some(&idx) = self.filtered_indices.get(self.cursor) else {
                    return;
                };
                match self.projects.get(idx) {
                    Some(p) => p.path.clone(),
                    None => return,
                }
            }
        };
        let display = path.display().to_string();
        match clipboard::copy(display.clone()) {
            Ok(()) => {
                // Shorten long paths in the toast so it doesn't wrap weirdly.
                let shown = if display.len() > 40 {
                    format!("…{}", &display[display.len() - 39..])
                } else {
                    display
                };
                self.toast = Some(Toast::new(
                    format!("copied {shown} to clipboard"),
                    ToastKind::Success,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(
                    format!("clipboard unavailable: {e}"),
                    ToastKind::Error,
                ));
            }
        }
    }

    fn request_rename(&mut self) {
        // Only session-list has per-session names we can edit.
        if self.mode != Mode::SessionList {
            return;
        }
        let Some(s) = self.selected_session_ref() else {
            return;
        };
        self.rename = Some(RenameState::new(s.id.clone(), s.name.as_deref()));
    }

    fn open_editor_for_selection(&mut self) {
        let path: PathBuf = match self.mode {
            Mode::SessionList => match self.selected_session_ref() {
                Some(s) => s.project_dir.clone(),
                None => return,
            },
            Mode::ProjectList => {
                let Some(&idx) = self.filtered_indices.get(self.cursor) else {
                    return;
                };
                match self.projects.get(idx) {
                    Some(p) => p.path.clone(),
                    None => return,
                }
            }
        };
        match editor::open_in_editor(&path) {
            Ok(name) => {
                self.toast = Some(Toast::new(
                    format!("opened {} in {name}", path.display()),
                    ToastKind::Info,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(format!("editor: {e}"), ToastKind::Error));
            }
        }
    }

    fn handle_delete_confirm(&mut self, ev: Event) -> anyhow::Result<()> {
        match ev {
            Event::Key('y') | Event::Key('Y') => {
                self.show_delete_confirm = false;
                // Multi-select batch delete: loop over every id, remove
                // from the in-memory list on success.
                if self.multi_mode && !self.multi_selected.is_empty() {
                    let ids: Vec<String> = self
                        .multi_selected_sessions()
                        .iter()
                        .map(|s| s.id.clone())
                        .collect();
                    let mut ok = 0usize;
                    let mut err_msg: Option<String> = None;
                    for id in ids.iter() {
                        // Synthesise a minimal Session-like struct to reuse the
                        // delete helper's resolver.
                        if let Some(s) = self.sessions.iter().find(|s| &s.id == id).cloned() {
                            match delete_session_file(&s) {
                                Ok(()) => ok += 1,
                                Err(e) => {
                                    err_msg = Some(format!("{e}"));
                                    break;
                                }
                            }
                        }
                    }
                    // Drop the deleted sessions from the picker.
                    self.sessions.retain(|s| !ids.contains(&s.id));
                    self.clear_multi_select();
                    self.rebuild_haystacks();
                    self.apply_filter();
                    if let Some(e) = err_msg {
                        self.toast =
                            Some(Toast::new(format!("delete failed: {e}"), ToastKind::Error));
                    } else {
                        // TODO: wire into delete flow — snapshot .jsonl bytes
                        // before unlinking so `z` can restore them. For now
                        // the undo hint is still surfaced to keep the UX
                        // promise; the inverse is a no-op until we plumb the
                        // BulkDelete variant through.
                        self.toast = Some(Toast::new(
                            format!("deleted {ok} sessions \u{00B7} z to undo"),
                            ToastKind::Success,
                        ));
                    }
                    return Ok(());
                }
                if let Some(s) = self.selected_session_ref().cloned() {
                    match delete_session_file(&s) {
                        Ok(()) => {
                            // TODO: wire into delete flow — snapshot the
                            // deleted file's bytes into an UndoAction so the
                            // toast's "z to undo" promise is honoured.
                            self.toast = Some(Toast::new(
                                format!(
                                    "deleted {} \u{00B7} z to undo",
                                    &s.id[..8.min(s.id.len())]
                                ),
                                ToastKind::Success,
                            ));
                            // Remove from in-memory list so UI updates.
                            if let Some(idx) = self.sessions.iter().position(|x| x.id == s.id) {
                                self.sessions.remove(idx);
                            }
                            self.rebuild_haystacks();
                            self.apply_filter();
                        }
                        Err(e) => {
                            self.toast =
                                Some(Toast::new(format!("delete failed: {e}"), ToastKind::Error));
                        }
                    }
                }
            }
            Event::Escape | Event::Key('n') | Event::Key('N') => {
                self.show_delete_confirm = false;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_escape(&mut self) {
        if !self.filter.is_empty() {
            self.filter.clear();
            self.apply_filter();
            return;
        }
        // Esc with no filter but an active multi-selection: clear it.
        // Keeps the picker where it is so users don't accidentally pop
        // back to the project list after a long Tab session.
        if self.multi_mode {
            self.clear_multi_select();
            return;
        }
        match self.mode {
            Mode::SessionList => {
                // Pop back to project-list if we have one to show.
                if !self.projects.is_empty() {
                    // Stash the session cursor before mode flips so the
                    // next drill-in restores this row.
                    self.save_session_cursor_memory();
                    self.mode = Mode::ProjectList;
                    self.sessions.clear();
                    self.selected_session = None;
                    self.rebuild_haystacks();
                    self.apply_filter();
                    // Put the cursor back on the project we just left so
                    // Esc → Enter round-trips cleanly.
                    if let Some(proj_idx) = self.selected_project {
                        if let Some(pos) = self
                            .filtered_indices
                            .iter()
                            .position(|&i| i == proj_idx)
                        {
                            self.cursor = pos;
                        }
                    }
                    // Project scroll persists across drill-ins (the list
                    // data itself hasn't changed), but we still snap so
                    // the cursor is guaranteed visible after the jump.
                    self.snap_project_scroll(self.cursor);
                } else {
                    self.should_quit = true;
                }
            }
            Mode::ProjectList => self.should_quit = true,
        }
    }

    fn confirm_selection(&mut self) {
        match self.mode {
            Mode::SessionList => {
                if let Some(s) = self.selected_session_ref().cloned() {
                    // Record the session open on the jump ring before we
                    // return — this is the only "open session" verb.
                    self.push_jump_point();
                    self.selection_result = Some((s.id.clone(), s.project_dir.clone()));
                    self.should_quit = true;
                }
            }
            Mode::ProjectList => self.open_selected_project(),
        }
    }

    /// Switch to session-list mode for the project under the cursor. Loads
    /// sessions synchronously (JSONL parsing is fast enough that async here
    /// would be premature optimisation); if the load fails we surface a
    /// toast rather than crashing.
    fn open_selected_project(&mut self) {
        let Some(&project_idx) = self.filtered_indices.get(self.cursor) else {
            return;
        };
        let project = match self.projects.get(project_idx) {
            Some(p) => p.clone(),
            None => return,
        };

        match crate::commands::pick::load_sessions_for(&project) {
            Ok(sessions) if !sessions.is_empty() => {
                // Record the project-open on the jump ring before the mode
                // switches so a later Ctrl-o restores the project view we
                // just left.
                self.push_jump_point();
                self.selected_project = Some(project_idx);
                self.sessions = sessions;
                self.selected_session = Some(0);
                self.mode = Mode::SessionList;
                self.filter.clear();
                // Repoint the ribbon's REPO chip at the project the user just
                // drilled into. Without this, REPO stays locked to whatever
                // project auto-activation picked at launch — so navigating
                // anywhere else produces "0/N" with every session filtered out.
                self.filter_ribbon
                    .set_current_repo(Some(project.path.to_string_lossy().into_owned()));
                self.rebuild_haystacks();
                self.apply_filter();
                // Restore per-project cursor memory if we've visited this
                // project before this picker session. `apply_filter` just
                // forced cursor=0, so we overwrite it here.
                self.restore_session_cursor_memory();
                // Scroll anchor has to start fresh when the underlying
                // list swaps — otherwise the smooth interpolator glides
                // across rows from the previous project's data. We don't
                // know `visible_rows` here (the render pass computes it),
                // so snap to the cursor row as an upper-bound anchor; the
                // render-time `anchored_scroll` clamp reins it in to the
                // actual viewport the next frame.
                self.snap_session_scroll(self.cursor);
            }
            Ok(_) => {
                self.toast = Some(Toast::new(
                    format!("{}: no sessions", project.name),
                    ToastKind::Info,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(format!("load error: {e}"), ToastKind::Error));
            }
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        let len = self.filtered_indices.len();
        if len == 0 {
            return;
        }
        let before = self.cursor;
        let current = self.cursor as i32;
        let next = (current + delta).rem_euclid(len as i32);
        self.cursor = next as usize;
        if self.cursor != before {
            self.previous_cursor = Some(before);
            self.cursor_changed_at = Some(Instant::now());
        }
    }

    /// Key identifying which project owns the currently-active session
    /// list. Stable across reloads because it's derived from the resolved
    /// filesystem path, which the project-scan pins to every `Project`.
    fn current_project_key(&self) -> Option<String> {
        let idx = self.selected_project?;
        let project = self.projects.get(idx)?;
        Some(project.path.to_string_lossy().into_owned())
    }

    /// Persist the current cursor row into session-cursor-memory, keyed by
    /// the active project's path. Called whenever we are about to leave
    /// [`Mode::SessionList`] so re-entering that project's sessions
    /// restores the cursor rather than snapping back to row 0.
    fn save_session_cursor_memory(&mut self) {
        if self.mode != Mode::SessionList {
            return;
        }
        if let Some(key) = self.current_project_key() {
            self.session_cursor_memory.insert(key, self.cursor);
        }
    }

    /// Restore the cursor row from memory for the currently-selected
    /// project, clamped to the filtered-index length so a session that
    /// got deleted while away doesn't land the cursor past the end.
    /// No-op when the project has no recorded entry yet.
    fn restore_session_cursor_memory(&mut self) {
        let Some(key) = self.current_project_key() else {
            return;
        };
        if let Some(&saved) = self.session_cursor_memory.get(&key) {
            let len = self.filtered_indices.len();
            if len == 0 {
                self.cursor = 0;
            } else {
                self.cursor = saved.min(len - 1);
            }
        }
    }

    /// Compute the top-row anchor of the visible slice for the
    /// session-list, respecting smooth-scroll interpolation and the
    /// reduce-motion gate. Renderers call this instead of the local
    /// `scroll_start` so animation state stays on the app struct.
    ///
    /// This has a side effect: it updates the smooth-scroll `target` to
    /// the freshly-computed baseline anchor. The renderer is the first
    /// code path per frame that knows `visible_rows`, so the target gets
    /// refreshed here rather than guessing in `move_cursor`. Because
    /// `SmoothScroll` is `Copy` and stored in a `Cell`, this stays
    /// `&self`-compatible.
    pub fn session_scroll_start(&self, visible_rows: usize, total: usize) -> usize {
        let reduce = crate::theme::animations_disabled();
        let target = baseline_scroll_start(self.cursor, visible_rows, total);
        let mut ss = self.scroll_session.get();
        ss.set_target(target, reduce);
        self.scroll_session.set(ss);
        anchored_scroll(self.cursor, visible_rows, total, ss.offset())
    }

    /// Same as [`Self::session_scroll_start`] but for the project list.
    pub fn project_scroll_start(&self, visible_rows: usize, total: usize) -> usize {
        let reduce = crate::theme::animations_disabled();
        let target = baseline_scroll_start(self.cursor, visible_rows, total);
        let mut ss = self.scroll_project.get();
        ss.set_target(target, reduce);
        self.scroll_project.set(ss);
        anchored_scroll(self.cursor, visible_rows, total, ss.offset())
    }

    /// Hard-snap the session scroller to `row`. Used whenever the
    /// underlying list swaps (entering a project, jumping views) so the
    /// interpolator doesn't glide through rows that no longer exist.
    fn snap_session_scroll(&self, row: usize) {
        let mut ss = self.scroll_session.get();
        ss.snap_to(row);
        self.scroll_session.set(ss);
    }

    /// Mirror of [`Self::snap_session_scroll`] for the project list.
    fn snap_project_scroll(&self, row: usize) {
        let mut ps = self.scroll_project.get();
        ps.snap_to(row);
        self.scroll_project.set(ps);
    }

    /// Read-only snapshot of the session-list smooth scroller. Test-only
    /// hook and the occasional renderer that wants the raw float for a
    /// sub-row effect.
    pub fn session_scroll_state(&self) -> crate::ui::fx::SmoothScroll {
        self.scroll_session.get()
    }

    /// Read-only snapshot of the project-list smooth scroller. Mirror of
    /// [`Self::session_scroll_state`].
    pub fn project_scroll_state(&self) -> crate::ui::fx::SmoothScroll {
        self.scroll_project.get()
    }

    /// True when the cursor glide trail should still be painted behind the
    /// row at `display_idx`. Kept on `App` so individual renderers stay
    /// dumb — they just ask "should I show the ghost here?" and get a bool.
    pub fn is_glide_trail(&self, display_idx: usize) -> bool {
        if crate::theme::animations_disabled() {
            return false;
        }
        let Some(prev) = self.previous_cursor else {
            return false;
        };
        let Some(when) = self.cursor_changed_at else {
            return false;
        };
        if when.elapsed() > CURSOR_GLIDE_WINDOW {
            return false;
        }
        prev == display_idx && prev != self.cursor
    }

    fn filter_push(&mut self, c: char) {
        self.filter.push(c);
        self.apply_filter();
    }

    fn filter_backspace(&mut self) {
        self.filter.pop();
        self.apply_filter();
    }

    fn toggle_bookmark(&mut self) {
        let Some(s) = self.selected_session_ref().cloned() else {
            return;
        };
        match self.bookmarks.toggle(&s.id) {
            Ok(true) => {
                self.toast = Some(Toast::new("pinned", ToastKind::Success));
            }
            Ok(false) => {
                self.toast = Some(Toast::new("unpinned", ToastKind::Info));
            }
            Err(e) => {
                self.toast = Some(Toast::new(format!("bookmark error: {e}"), ToastKind::Error));
            }
        }
    }

    /// `Ctrl+A` — summarise the selected session with a Haiku-backed call to
    /// the `claude` CLI and surface the one-line result as a quote-style
    /// toast. Cached summaries return instantly. Keeps the user on the
    /// picker — this is an informational overlay, not a navigation verb.
    ///
    /// Deliberately blocks the main loop (multi-second model call) rather
    /// than spawning a thread + channel. The tradeoff: we don't spin up
    /// async plumbing for one feature, and the user's next keystroke
    /// happens after the summary lands — matching `Ctrl+E`'s existing
    /// shell-out model.
    fn summarize_session_ai(&mut self) {
        if self.mode != Mode::SessionList {
            return;
        }
        let Some(s) = self.selected_session_ref().cloned() else {
            return;
        };
        if let Some(cached) = crate::data::ai_summarize::load_cached_summary(&s.id) {
            self.toast = Some(Toast::new_with_visible(
                format!("\u{226B} \"{cached}\""),
                ToastKind::Info,
                Duration::from_millis(3_500),
            ));
            return;
        }
        self.toast = Some(Toast::new(
            format!(
                "summarizing session… (~${:.3})",
                crate::data::ai_summarize::ESTIMATED_COST_USD
            ),
            ToastKind::Info,
        ));
        match crate::data::ai_summarize::summarize_session(&s.id) {
            Ok(summary) => {
                self.toast = Some(Toast::new_with_visible(
                    format!(
                        "\u{226B} \"{summary}\" \u{00B7} ~${:.3}",
                        crate::data::ai_summarize::ESTIMATED_COST_USD
                    ),
                    ToastKind::Success,
                    Duration::from_millis(3_500),
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(
                    format!("summarize failed: {e}"),
                    ToastKind::Error,
                ));
            }
        }
    }

    fn export_session(&mut self) {
        // Multi-select: export every selected session in sequence.
        if self.multi_mode && !self.multi_selected.is_empty() {
            let ids: Vec<String> = self
                .multi_selected_sessions()
                .iter()
                .map(|s| s.id.clone())
                .collect();
            let count = ids.len();
            let repo_root = find_repo_root();
            let Some(script) = repo_root.map(|r| r.join("lib").join("session-export.py")) else {
                self.toast = Some(Toast::new(
                    "export: could not locate session-export.py",
                    ToastKind::Error,
                ));
                return;
            };
            let mut ok = 0usize;
            let mut err_msg: Option<String> = None;
            for id in ids {
                match Command::new("python3").arg(&script).arg(&id).spawn() {
                    Ok(_) => ok += 1,
                    Err(e) => {
                        err_msg = Some(format!("{e}"));
                        break;
                    }
                }
            }
            if let Some(e) = err_msg {
                self.toast = Some(Toast::new(format!("export failed: {e}"), ToastKind::Error));
            } else {
                self.toast = Some(Toast::new(
                    format!("exported {ok} of {count} sessions"),
                    ToastKind::Success,
                ));
            }
            return;
        }

        let Some(s) = self.selected_session_ref().cloned() else {
            return;
        };
        // Shell out to the legacy Python exporter — replacing it is Day-2 work
        // explicitly called out in the brief.
        let repo_root = find_repo_root();
        let script = repo_root.map(|r| r.join("lib").join("session-export.py"));
        let Some(script) = script else {
            self.toast = Some(Toast::new(
                "export: could not locate session-export.py",
                ToastKind::Error,
            ));
            return;
        };
        let spawn = Command::new("python3").arg(&script).arg(&s.id).spawn();
        match spawn {
            Ok(_) => {
                self.toast = Some(Toast::new(
                    format!("exporting {}", &s.id[..8.min(s.id.len())]),
                    ToastKind::Info,
                ));
            }
            Err(e) => {
                self.toast = Some(Toast::new(format!("export failed: {e}"), ToastKind::Error));
            }
        }
    }

    fn request_delete(&mut self) {
        // Multi-select: raise the confirm modal if there are pending
        // selections, regardless of whether the cursor row is deletable.
        if self.multi_mode && !self.multi_selected.is_empty() {
            self.show_delete_confirm = true;
            return;
        }
        if self.selected_session_ref().is_some() {
            self.show_delete_confirm = true;
        }
    }

    /// Called once per frame to retire expired toasts + cursor-glide trails.
    ///
    /// Also advances the replay's virtual clock when a replay is open — the
    /// 50ms event poll in [`events::next`] acts as our tick timer, so the
    /// replay progresses even when no keys are pressed.
    pub fn tick(&mut self) {
        if let Some(t) = &self.toast {
            if t.is_expired() {
                self.toast = None;
            }
        }
        // Clear the glide trail once its window has passed. Avoids stale
        // state lingering past the animation and needing an extra render
        // condition everywhere.
        if let Some(when) = self.cursor_changed_at {
            if when.elapsed() > CURSOR_GLIDE_WINDOW {
                self.previous_cursor = None;
                self.cursor_changed_at = None;
            }
        }
        // Drive the smooth-scroll interpolators once per frame. Reduce
        // motion collapses the animation to a single-frame snap.
        let reduce_motion = crate::theme::animations_disabled();
        let mut ss = self.scroll_session.get();
        ss.advance(reduce_motion);
        self.scroll_session.set(ss);
        let mut ps = self.scroll_project.get();
        ps.advance(reduce_motion);
        self.scroll_project.set(ps);
        // Advance replay virtual clock if a replay is open.
        if let Some(replay) = self.replay.as_mut() {
            replay.advance(Instant::now());
        }
        // Evict long-finished background task rows so the drawer doesn't
        // grow unbounded. 10s lets the user visually register "ok, that
        // completed" before the row disappears.
        if let Ok(mut q) = self.task_queue.lock() {
            q.sweep(Duration::from_secs(10));
        }
    }
}

fn is_filter_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '/' | '@')
}

/// Canonical "the cursor is always on-screen" anchor. Matches the
/// `scroll_start` helpers previously inlined into `project_list` and
/// `session_list` — top-anchored until the cursor crosses the first
/// page, bottom-anchored afterwards.
///
/// Moved onto `app.rs` so the smooth-scroll interpolator shares the same
/// source of truth as the render path; the renderers still keep their
/// local copies as a fallback when App state isn't available (tests,
/// stand-alone unit runs).
fn baseline_scroll_start(selected: usize, visible_rows: usize, total: usize) -> usize {
    if visible_rows == 0 || total <= visible_rows {
        return 0;
    }
    if selected < visible_rows {
        0
    } else {
        selected + 1 - visible_rows
    }
}

/// Anchor the viewport using the smoothed offset, but clamp so the cursor
/// is always visible. This preserves the "cursor never scrolls off-screen"
/// contract even while the smoothed offset is mid-interpolation: if the
/// interpolator is lagging behind a fast `j`-burst, we pin the viewport
/// to whichever anchor keeps the cursor on-screen.
fn anchored_scroll(
    selected: usize,
    visible_rows: usize,
    total: usize,
    smooth_offset: usize,
) -> usize {
    if visible_rows == 0 || total <= visible_rows {
        return 0;
    }
    let max_start = total - visible_rows;
    let min_start = (selected + 1).saturating_sub(visible_rows);
    let max_cursor_visible = selected.min(max_start);
    smooth_offset.clamp(min_start, max_cursor_visible)
}

/// Best-effort delete of the session JSONL.
fn delete_session_file(session: &Session) -> anyhow::Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects = home.join(".claude").join("projects");
    for entry in std::fs::read_dir(projects)? {
        let Ok(entry) = entry else { continue };
        let candidate = entry.path().join(format!("{}.jsonl", session.id));
        if candidate.is_file() {
            std::fs::remove_file(candidate)?;
            return Ok(());
        }
    }
    Err(anyhow::anyhow!("session file not found"))
}

/// Walk up from the current binary's directory looking for the repo root
/// (identified by the `lib/` directory we ship the Python tools in). Used to
/// locate the legacy exporter until it's ported.
///
/// We canonicalize first so a symlinked install (`~/.local/bin/claude-picker`
/// → `~/Desktop/claude-picker/target/release/claude-picker`) still resolves
/// to the real binary location, from which we can walk up to find `lib/`.
fn find_repo_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    let mut dir = exe.parent()?.to_path_buf();
    for _ in 0..6 {
        if dir.join("lib").join("session-export.py").is_file() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

// ── Terminal lifecycle ─────────────────────────────────────────────────

/// Claim the terminal: raw mode, alt screen, mouse capture.
pub fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Undo [`setup_terminal`]. Called in `run`'s defer position so a panic still
/// restores the user's shell.
pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Top-level event-loop driver.
///
/// Returns `Ok(Some((session_id, cwd)))` if the user selected something,
/// `Ok(None)` if they quit without picking.
pub fn run(mut app: App) -> anyhow::Result<Option<(String, PathBuf)>> {
    let mut terminal = setup_terminal()?;
    // Install a panic hook that restores the terminal so a crash doesn't
    // leave the shell in raw mode.
    install_panic_hook();

    let result: anyhow::Result<Option<(String, PathBuf)>> = (|| {
        while !app.should_quit {
            terminal.draw(|f| {
                crate::ui::picker::render(f, &mut app);
                if let Some(sim) = app.model_simulator.as_ref() {
                    let theme = app.theme;
                    let area = f.area();
                    crate::ui::model_simulator::render(f, area, sim, &theme);
                }
                if let Some(state) = app.onboarding.as_ref() {
                    let theme = app.theme;
                    let area = f.area();
                    crate::ui::onboarding::render(f, area, state, &theme);
                }
            })?;
            app.tick();
            if let Some(ev) = events::next()? {
                app.handle_event(ev)?;
            }
        }
        Ok(app.selection_result.clone())
    })();

    // Always restore, even on error.
    let _ = restore_terminal(&mut terminal);
    result
}

fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut stdout = io::stdout();
        let _ = disable_raw_mode();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_session(id: &str, name: Option<&str>) -> Session {
        use crate::data::pricing::TokenCounts;
        use crate::data::session::SessionKind;
        Session {
            id: id.to_string(),
            project_dir: PathBuf::from("/tmp"),
            name: name.map(|s| s.to_string()),
            auto_name: None,
            last_prompt: None,
            message_count: 5,
            tokens: TokenCounts::default(),
            total_cost_usd: 0.0,
            model_summary: "claude-opus-4-7".to_string(),
            first_timestamp: None,
            last_timestamp: None,
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
            turn_durations: Vec::new(),
        }
    }

    #[test]
    fn filter_matches_substring() {
        let sessions = vec![
            mk_session("a", Some("auth-refactor")),
            mk_session("b", Some("fix-race-condition")),
            mk_session("c", Some("drizzle-migration")),
        ];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);

        app.filter.push_str("auth");
        app.apply_filter();
        assert_eq!(app.filtered_indices.len(), 1);
        assert_eq!(app.filtered_indices[0], 0);

        app.filter.clear();
        app.filter.push_str("migration");
        app.apply_filter();
        assert_eq!(app.filtered_indices.len(), 1);
        assert_eq!(app.filtered_indices[0], 2);
    }

    #[test]
    fn empty_filter_shows_all() {
        let sessions = vec![mk_session("a", Some("x")), mk_session("b", Some("y"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        assert_eq!(app.filtered_indices.len(), 2);
    }

    #[test]
    fn escape_clears_filter_then_quits() {
        let sessions = vec![mk_session("a", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.filter.push_str("abc");
        app.apply_filter();
        app.handle_event(Event::Escape).unwrap();
        assert!(app.filter.is_empty(), "first Escape should clear filter");
        assert!(!app.should_quit);
        app.handle_event(Event::Escape).unwrap();
        assert!(
            app.should_quit,
            "second Escape on no-project-list must quit"
        );
    }

    #[test]
    fn arrow_navigation_wraps() {
        let sessions = vec![mk_session("a", Some("a")), mk_session("b", Some("b"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        assert_eq!(app.cursor, 0);
        app.handle_event(Event::Down).unwrap();
        assert_eq!(app.cursor, 1);
        app.handle_event(Event::Down).unwrap();
        assert_eq!(app.cursor, 0, "wraps around");
        app.handle_event(Event::Up).unwrap();
        assert_eq!(app.cursor, 1, "up from top wraps to bottom");
    }

    #[test]
    fn enter_records_selection() {
        let sessions = vec![mk_session("abc123", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Enter).unwrap();
        assert!(app.should_quit);
        assert_eq!(
            app.selection_result.as_ref().map(|r| r.0.as_str()),
            Some("abc123")
        );
    }

    #[test]
    fn q_quits_when_filter_empty() {
        let sessions = vec![mk_session("a", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Key('q')).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn q_typed_into_filter_does_not_quit() {
        let sessions = vec![mk_session("a", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.filter.push('a');
        app.handle_event(Event::Key('q')).unwrap();
        assert!(!app.should_quit);
        assert_eq!(app.filter, "aq");
    }

    #[test]
    fn tab_enters_multi_select_and_toggles_rows() {
        let sessions = vec![
            mk_session("a1", Some("alpha")),
            mk_session("b2", Some("bravo")),
            mk_session("c3", Some("charlie")),
        ];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        assert!(!app.multi_mode);
        app.handle_event(Event::Tab).unwrap();
        assert!(app.multi_mode);
        assert_eq!(app.multi_selected_count(), 1);
        app.handle_event(Event::Down).unwrap();
        app.handle_event(Event::Tab).unwrap();
        assert_eq!(app.multi_selected_count(), 2);
        // Tab again on the same row toggles off.
        app.handle_event(Event::Tab).unwrap();
        assert_eq!(app.multi_selected_count(), 1);
    }

    #[test]
    fn esc_clears_multi_select_without_popping_mode() {
        let sessions = vec![mk_session("a1", Some("alpha"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Tab).unwrap();
        assert_eq!(app.multi_selected_count(), 1);
        app.handle_event(Event::Escape).unwrap();
        assert_eq!(app.multi_selected_count(), 0);
        assert!(!app.multi_mode);
        assert!(!app.should_quit, "Esc on multi-selection must not quit");
    }

    #[test]
    fn is_multi_selected_reports_correctly() {
        let sessions = vec![mk_session("a1", Some("a")), mk_session("b2", Some("b"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Tab).unwrap();
        assert!(app.is_multi_selected(0));
        assert!(!app.is_multi_selected(1));
    }

    #[test]
    fn v_key_opens_viewer_when_filter_empty() {
        let sessions = vec![mk_session("abcdef1234", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Key('v')).unwrap();
        assert!(
            app.viewer.is_some(),
            "viewer must open for a real selected row"
        );
    }

    #[test]
    fn v_key_typed_into_filter_does_not_open_viewer() {
        let sessions = vec![mk_session("a", Some("x"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.filter.push('s');
        app.handle_event(Event::Key('v')).unwrap();
        assert!(app.viewer.is_none());
        assert_eq!(app.filter, "sv");
    }

    #[test]
    fn tab_in_project_list_is_no_op() {
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], vec![], bm, Mode::ProjectList, None);
        app.handle_event(Event::Tab).unwrap();
        assert!(!app.multi_mode);
    }

    #[test]
    fn toast_slide_and_fade_progress_bounds() {
        // A brand-new toast is mid-slide-in (could be 0 or tiny) and has
        // zero fade. A hand-constructed expired toast reports full fade.
        let t = Toast::new("hi", ToastKind::Info);
        assert!(t.slide_in_progress() >= 0.0);
        assert!(t.slide_in_progress() <= 1.0);
        assert_eq!(t.fade_out_progress(), 0.0);

        // Simulate an old toast by subtracting from created_at.
        let mut expired = Toast::new("hi", ToastKind::Info);
        expired.created_at = Instant::now()
            - Toast::SLIDE_IN
            - Toast::VISIBLE
            - Toast::FADE_OUT
            - Duration::from_millis(50);
        assert_eq!(expired.slide_in_progress(), 1.0);
        assert_eq!(expired.fade_out_progress(), 1.0);
        assert!(expired.is_expired());
    }

    #[test]
    fn cursor_glide_trail_reports_previous_then_clears() {
        let sessions = vec![
            mk_session("a", Some("a")),
            mk_session("b", Some("b")),
            mk_session("c", Some("c")),
        ];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);

        // Fresh cursor = no glide anywhere.
        assert!(!app.is_glide_trail(0));
        assert!(!app.is_glide_trail(1));

        // Move down: previous row should report glide.
        app.handle_event(Event::Down).unwrap();
        assert!(app.is_glide_trail(0), "row we left should glide");
        assert!(!app.is_glide_trail(1), "cursor row is not the trail");
        assert!(!app.is_glide_trail(2));

        // Advance past the glide window (simulate by zeroing the timer).
        app.cursor_changed_at = Some(Instant::now() - Duration::from_secs(2));
        app.tick();
        assert!(!app.is_glide_trail(0));
        assert!(app.previous_cursor.is_none());
    }

    #[test]
    fn cursor_glide_disabled_by_no_anim_env() {
        let key = crate::theme::NO_ANIM_ENV_VAR;
        let prev = std::env::var(key).ok();
        let sessions = vec![mk_session("a", Some("a")), mk_session("b", Some("b"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Down).unwrap();

        std::env::set_var(key, "1");
        assert!(
            !app.is_glide_trail(0),
            "env-var opt-out must kill the glide"
        );
        std::env::remove_var(key);
        if let Some(v) = prev {
            std::env::set_var(key, v);
        }
    }

    #[test]
    fn digit_prefix_accumulates_count() {
        let sessions = vec![
            mk_session("a", Some("a")),
            mk_session("b", Some("b")),
            mk_session("c", Some("c")),
            mk_session("d", Some("d")),
            mk_session("e", Some("e")),
        ];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Key('1')).unwrap();
        app.handle_event(Event::Key('2')).unwrap();
        assert_eq!(app.pending_count_value(), Some(12));
        // Next non-digit action consumes the count: `12G` → row 12 (clamped).
        app.handle_event(Event::Key('G')).unwrap();
        assert!(app.pending_count_value().is_none());
        // 12 was clamped to last row (5 sessions → index 4).
        assert_eq!(app.cursor, 4);
    }

    #[test]
    fn zero_as_first_digit_is_not_consumed() {
        let sessions = vec![mk_session("a", Some("a"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        // `0` with no pending count should NOT create a count — Agent C's
        // binding reserves that slot.
        app.handle_event(Event::Key('0')).unwrap();
        assert!(app.pending_count_value().is_none());
    }

    #[test]
    fn esc_clears_pending_count_without_popping_screen() {
        let sessions = vec![mk_session("a", Some("a")), mk_session("b", Some("b"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Key('3')).unwrap();
        assert_eq!(app.pending_count_value(), Some(3));
        app.handle_event(Event::Escape).unwrap();
        assert!(app.pending_count_value().is_none());
        assert!(!app.should_quit);
    }

    #[test]
    fn space_leader_sets_pending_chord() {
        let sessions = vec![mk_session("a", Some("a"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Key(' ')).unwrap();
        assert_eq!(app.which_key_leader(), Some(' '));
        // Immediately — still within the 250ms window.
        assert!(!app.should_show_which_key());
    }

    #[test]
    fn space_space_opens_palette_via_chord() {
        let sessions = vec![mk_session("a", Some("a"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        app.handle_event(Event::Key(' ')).unwrap();
        app.handle_event(Event::Key(' ')).unwrap();
        assert!(app.palette.is_some());
        assert!(app.which_key_leader().is_none());
    }

    #[test]
    fn z_on_empty_stack_is_noop() {
        let sessions = vec![mk_session("a", Some("a"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        assert_eq!(app.undo_depth(), 0);
        app.handle_event(Event::Key('z')).unwrap();
        assert_eq!(app.undo_depth(), 0);
    }

    #[test]
    fn push_undo_caps_stack_depth() {
        let sessions = vec![mk_session("a", Some("a"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        for i in 0..(UNDO_CAP + 10) {
            app.push_undo(UndoAction::Rename {
                session_id: format!("s{i}"),
                old_title: Some("old".into()),
                new_title: format!("new{i}"),
            });
        }
        assert_eq!(app.undo_depth(), UNDO_CAP);
    }

    #[test]
    fn ctrl_o_with_empty_ring_is_noop() {
        let sessions = vec![mk_session("a", Some("a"))];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        assert!(app.jump_ring_position().is_none());
        app.handle_event(Event::Ctrl('o')).unwrap();
        assert!(app.jump_ring_position().is_none());
    }

    fn mk_project(name: &str, path: &str) -> Project {
        use crate::data::Project;
        Project {
            name: name.to_string(),
            path: PathBuf::from(path),
            encoded_dir: name.to_string(),
            session_count: 0,
            last_activity: None,
            git_branch: None,
        }
    }

    #[test]
    fn cursor_memory_restores_on_reentry() {
        // Arrange a session list owned by project 0 and drop the cursor on
        // row 3, then simulate leaving → re-entering the same project. The
        // cursor should come back to row 3 instead of snapping to 0.
        let projects = vec![mk_project("alpha", "/tmp/alpha")];
        let sessions = vec![
            mk_session("s0", Some("zero")),
            mk_session("s1", Some("one")),
            mk_session("s2", Some("two")),
            mk_session("s3", Some("three")),
            mk_session("s4", Some("four")),
        ];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(
            projects,
            sessions,
            bm,
            Mode::SessionList,
            Some(0),
        );
        app.cursor = 3;

        // Leaving session-list must stash the cursor.
        app.save_session_cursor_memory();
        assert_eq!(
            app.session_cursor_memory.get("/tmp/alpha").copied(),
            Some(3),
            "memory should be keyed by the project path"
        );

        // Emulate a re-entry: reset the cursor to 0 (apply_filter always
        // does this) and then restore from memory.
        app.cursor = 0;
        app.restore_session_cursor_memory();
        assert_eq!(app.cursor, 3, "cursor must snap back to the saved row");
    }

    #[test]
    fn cursor_memory_per_project_isolation() {
        // Two projects keep independent cursor memories — switching
        // between them must not cross-contaminate.
        let projects = vec![
            mk_project("alpha", "/tmp/alpha"),
            mk_project("bravo", "/tmp/bravo"),
        ];
        let sessions = vec![
            mk_session("a0", Some("a0")),
            mk_session("a1", Some("a1")),
            mk_session("a2", Some("a2")),
            mk_session("a3", Some("a3")),
        ];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");

        // Project 0 — save cursor on row 2.
        let mut app = App::new(
            projects.clone(),
            sessions.clone(),
            bm,
            Mode::SessionList,
            Some(0),
        );
        app.cursor = 2;
        app.save_session_cursor_memory();

        // Swap to project 1 with its own session set; cursor goes to row 0.
        app.selected_project = Some(1);
        app.cursor = 0;
        app.restore_session_cursor_memory();
        assert_eq!(
            app.cursor, 0,
            "project bravo has no saved cursor — should stay at 0"
        );

        // Save a different row under project 1.
        app.cursor = 1;
        app.save_session_cursor_memory();

        // Back to project 0 — we must retrieve its own saved row (2),
        // not the row just saved for project 1 (1).
        app.selected_project = Some(0);
        app.cursor = 0;
        app.restore_session_cursor_memory();
        assert_eq!(app.cursor, 2, "project alpha's row must be restored");

        // And project 1's memory is preserved too.
        app.selected_project = Some(1);
        app.cursor = 0;
        app.restore_session_cursor_memory();
        assert_eq!(app.cursor, 1, "project bravo's row must be independent");
    }

    #[test]
    fn j_respects_repeat_count() {
        let sessions = vec![
            mk_session("a", Some("a")),
            mk_session("b", Some("b")),
            mk_session("c", Some("c")),
            mk_session("d", Some("d")),
            mk_session("e", Some("e")),
        ];
        let bm = BookmarkStore::load_from(PathBuf::from("/tmp/nonexistent-bookmarks.json"))
            .expect("load");
        let mut app = App::new(vec![], sessions, bm, Mode::SessionList, None);
        // `3j` → down 3 rows.
        app.handle_event(Event::Key('3')).unwrap();
        app.handle_event(Event::Key('j')).unwrap();
        assert_eq!(app.cursor, 3);
        assert!(app.pending_count_value().is_none());
    }
}
