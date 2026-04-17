//! `claude-picker search` — full-text search across every session.
//!
//! Day-2 feature. Scans every `~/.claude/projects/<encoded>/<id>.jsonl`,
//! concatenates the user + assistant message bodies into a searchable corpus,
//! and runs a fuzzy/substring matcher on user input. Each hit carries:
//!
//!   - the raw `session_id` (so the bash wrapper can `claude --resume <id>`),
//!   - the project cwd (so the wrapper can `cd` first),
//!   - a ~80-char snippet window around the first match (for the UI), and
//!   - the nucleo score (for ranking).
//!
//! The UI is rendered by [`crate::ui::search`] — this module owns the corpus,
//! the matcher, and the event loop glue.
//!
//! Output contract when the user hits Enter:
//!   `__SELECTION__ <session_id>\t<project_cwd>\n`
//! printed to stdout, then exit 0.

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};

use crate::app::{restore_terminal, setup_terminal};
use crate::data::bookmarks::BookmarkStore;
use crate::data::project::discover_projects;
use crate::data::search_filters::{self, Filters};
use crate::data::session::{load_session_from_jsonl, noise_prefixes, PermissionMode};
use crate::data::{clipboard, editor, Session};
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::command_palette::{self, CommandPalette};
use crate::ui::conversation_viewer::{ToastKind as ViewerToastKind, ViewerAction, ViewerState};
use crate::ui::help_overlay;
use crate::ui::search as search_ui;

use search_ui::{extract_snippet, render, SearchMatch, SearchState, ToastKind};

/// Window for the `gg` vim chord.
const G_CHORD_WINDOW: Duration = Duration::from_millis(500);

/// An indexed session — the raw body text we search against, plus the
/// metadata we need to build a [`SearchMatch`] once we have a score.
struct Indexed {
    session_id: String,
    project_name: String,
    project_cwd: PathBuf,
    session_name: String,
    /// Concatenated user+assistant text, noise-filtered, newlines preserved so
    /// snippet extraction can replace them with spaces after the window is
    /// picked.
    body: String,
    last_ts: Option<chrono::DateTime<chrono::Utc>>,
    /// Cached Utf32String for the nucleo matcher. Same allocation gets reused
    /// across keystrokes.
    haystack: Utf32String,
    // ── Filter-attribute fields — populated from the loaded Session so
    //    the pre-nucleo filter pass can decide without re-reading JSONLs.
    //    history-only rows default these to "no, no, no" since we don't
    //    have the per-session metadata there.
    is_bookmarked: bool,
    has_custom_name: bool,
    model_summary: String,
    permission_mode: Option<PermissionMode>,
    total_cost_usd: f64,
    total_tokens: u64,
    message_count: u32,
}

/// Cap on scored results we carry around. With ~1000 sessions and a loose
/// query we don't want to render or score more than the top 50.
const MAX_MATCHES: usize = 50;

/// Public entry point. Runs the screen and, on Enter, prints the selection
/// to stdout in the bash-wrapper contract format.
pub fn run() -> anyhow::Result<()> {
    // ── Stage 1: kick off the background loader ──────────────────────────
    let (tx, rx) = mpsc::channel::<Vec<Indexed>>();
    thread::spawn(move || {
        // Errors during load aren't fatal — we just render an empty corpus.
        let index = build_corpus().unwrap_or_default();
        let _ = tx.send(index);
    });

    // ── Stage 2: terminal lifecycle + event loop ─────────────────────────
    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let theme = Theme::mocha();
    let mut state = SearchState::new();

    // A matcher keeps its scratch memory across keystrokes.
    let mut matcher = Matcher::new(Config::DEFAULT);
    // The current parsed pattern — rebuilt whenever the query changes.
    let mut pattern: Option<Pattern> = None;
    // The universe of indexed sessions; filled in once the loader reports.
    let mut index: Vec<Indexed> = Vec::new();
    // Pending `g` for the gg chord. Local so it doesn't live on SearchState
    // (which is shared across UI modules that don't care about it).
    let mut pending_g: Option<Instant> = None;

    let result = (|| -> anyhow::Result<Option<(String, PathBuf)>> {
        loop {
            // Non-blocking check for the loader's one-shot payload.
            if state.loading {
                if let Ok(payload) = rx.try_recv() {
                    index = payload;
                    // Populate all_matches with an empty-query view so the
                    // user can see session count while scanning an empty buffer.
                    state.all_matches = index
                        .iter()
                        .map(|ix| SearchMatch {
                            session_id: ix.session_id.clone(),
                            project_name: ix.project_name.clone(),
                            project_cwd: ix.project_cwd.clone(),
                            session_name: ix.session_name.clone(),
                            snippet: String::new(),
                            score: 0,
                        })
                        .collect();
                    state.loading = false;
                    recompute_matches(&mut state, &index, &mut matcher, &mut pattern);
                }
            }

            state.tick();
            terminal.draw(|f| render(f, f.area(), &mut state, &theme))?;

            if let Some(ev) = events::next()? {
                let outcome = handle_event(
                    &mut state,
                    ev,
                    &index,
                    &mut matcher,
                    &mut pattern,
                    &mut pending_g,
                );
                match outcome {
                    EventOutcome::Continue => {}
                    EventOutcome::Quit => return Ok(None),
                    EventOutcome::Resume(id, cwd) => return Ok(Some((id, cwd))),
                }
            } else {
                // Idle tick — check for loader payload if the user hasn't
                // typed yet; avoids leaving the loading indicator up for an
                // extra frame after the corpus arrives.
                std::thread::sleep(Duration::from_millis(0));
            }
        }
    })();

    let _ = restore_terminal(&mut terminal);

    match result? {
        Some((id, cwd)) => {
            crate::resume::resume_session(&id, &cwd); // diverges
        }
        None => Ok(()),
    }
}

/// What the event handler decided.
enum EventOutcome {
    Continue,
    Quit,
    Resume(String, PathBuf),
}

fn handle_event(
    state: &mut SearchState,
    ev: Event,
    index: &[Indexed],
    matcher: &mut Matcher,
    pattern: &mut Option<Pattern>,
    pending_g: &mut Option<Instant>,
) -> EventOutcome {
    // Viewer first — full-screen takes priority over everything else.
    if state.viewer.is_some() {
        return handle_viewer_event(state, ev);
    }
    // Palette next: while it's open it owns input. On an Execute we
    // close it and dispatch the named action below.
    if state.palette.is_some() {
        return handle_palette_event(state, ev);
    }
    // Help overlay steals input while visible.
    if state.show_help {
        match ev {
            Event::Escape => state.show_help = false,
            Event::Key(c) if help_overlay::is_dismiss_key(c) => state.show_help = false,
            _ => {}
        }
        return EventOutcome::Continue;
    }

    // Expire stale `gg` chord.
    if let Some(t) = *pending_g {
        if t.elapsed() > G_CHORD_WINDOW {
            *pending_g = None;
        }
    }

    match ev {
        Event::Quit | Event::Ctrl('c') | Event::Escape => EventOutcome::Quit,
        Event::Key('q') if state.query.is_empty() => EventOutcome::Quit,
        Event::Enter => {
            if let Some(m) = state.selected_match() {
                EventOutcome::Resume(m.session_id.clone(), m.project_cwd.clone())
            } else {
                EventOutcome::Continue
            }
        }
        Event::Up | Event::Key('k') => {
            *pending_g = None;
            move_cursor(state, -1);
            EventOutcome::Continue
        }
        Event::Down | Event::Key('j') => {
            *pending_g = None;
            move_cursor(state, 1);
            EventOutcome::Continue
        }
        Event::PageUp => {
            *pending_g = None;
            move_cursor(state, -10);
            EventOutcome::Continue
        }
        Event::PageDown => {
            *pending_g = None;
            move_cursor(state, 10);
            EventOutcome::Continue
        }
        Event::Home => {
            *pending_g = None;
            state.cursor = 0;
            EventOutcome::Continue
        }
        Event::End => {
            *pending_g = None;
            state.cursor = state.filtered_indices.len().saturating_sub(1);
            EventOutcome::Continue
        }
        Event::Backspace => {
            state.query.pop();
            recompute_matches(state, index, matcher, pattern);
            EventOutcome::Continue
        }
        // Shortcuts only fire when the query is empty so they don't collide
        // with search input.
        Event::Key('?') if state.query.is_empty() => {
            state.show_help = true;
            EventOutcome::Continue
        }
        Event::Key('G') if state.query.is_empty() => {
            *pending_g = None;
            state.cursor = state.filtered_indices.len().saturating_sub(1);
            EventOutcome::Continue
        }
        Event::Key('g') if state.query.is_empty() => {
            if pending_g
                .map(|t| t.elapsed() <= G_CHORD_WINDOW)
                .unwrap_or(false)
            {
                state.cursor = 0;
                *pending_g = None;
            } else {
                *pending_g = Some(Instant::now());
            }
            EventOutcome::Continue
        }
        Event::Key('y') if state.query.is_empty() => {
            copy_session_id(state);
            EventOutcome::Continue
        }
        Event::Key('Y') if state.query.is_empty() => {
            copy_project_path(state);
            EventOutcome::Continue
        }
        Event::Key('o') if state.query.is_empty() => {
            open_editor(state);
            EventOutcome::Continue
        }
        Event::Key('v') if state.query.is_empty() => {
            open_viewer(state);
            EventOutcome::Continue
        }
        Event::Key('p') if state.query.is_empty() => {
            // `p` toggles preview only when the filter is empty — otherwise
            // it's a regular filter char.
            state.preview_visible = !state.preview_visible;
            EventOutcome::Continue
        }
        // Space opens the command palette when the query is empty.
        // Inside a query the space is part of the query (nucleo tolerates
        // multi-word fuzzy input).
        Event::Key(' ') if state.query.is_empty() => {
            *pending_g = None;
            state.palette = Some(CommandPalette::new(command_palette::Context::Search));
            EventOutcome::Continue
        }
        Event::Key(c) if is_query_char(c) => {
            *pending_g = None;
            state.query.push(c);
            recompute_matches(state, index, matcher, pattern);
            EventOutcome::Continue
        }
        // `Ctrl+p` toggles preview regardless of filter state, matching the
        // spec's "p toggle preview" key hint without the empty-buffer caveat
        // surfacing to the user as a surprise.
        Event::Ctrl('p') => {
            state.preview_visible = !state.preview_visible;
            EventOutcome::Continue
        }
        _ => {
            *pending_g = None;
            EventOutcome::Continue
        }
    }
}

/// Drive the open palette. On Enter it returns `Execute(id)` which we
/// translate into a search-screen action; on Esc it closes. Non-Enter
/// events stay inside the palette so the underlying list doesn't move.
fn handle_palette_event(state: &mut SearchState, ev: Event) -> EventOutcome {
    let Some(palette) = state.palette.as_mut() else {
        return EventOutcome::Continue;
    };
    match palette.handle_event(ev) {
        command_palette::Outcome::Continue => EventOutcome::Continue,
        command_palette::Outcome::Close => {
            state.palette = None;
            EventOutcome::Continue
        }
        command_palette::Outcome::Execute(id) => {
            state.palette = None;
            execute_palette_action(state, id)
        }
    }
}

/// Map a palette action id to the corresponding search-screen effect.
/// `resume`, being the primary action, yields a Resume outcome so the
/// caller returns from the event loop; everything else only mutates
/// `state` and continues.
fn execute_palette_action(state: &mut SearchState, id: &'static str) -> EventOutcome {
    match id {
        "resume" => {
            if let Some(m) = state.selected_match() {
                EventOutcome::Resume(m.session_id.clone(), m.project_cwd.clone())
            } else {
                EventOutcome::Continue
            }
        }
        "toggle_preview" => {
            state.preview_visible = !state.preview_visible;
            EventOutcome::Continue
        }
        "copy_session_id" => {
            copy_session_id(state);
            EventOutcome::Continue
        }
        "copy_project_path" => {
            copy_project_path(state);
            EventOutcome::Continue
        }
        "open_editor" => {
            open_editor(state);
            EventOutcome::Continue
        }
        "help" => {
            state.show_help = true;
            EventOutcome::Continue
        }
        "quit" => EventOutcome::Quit,
        _ => EventOutcome::Continue,
    }
}

fn copy_session_id(state: &mut SearchState) {
    let Some(m) = state.selected_match() else {
        return;
    };
    let id = m.session_id.clone();
    let short = id.chars().take(8).collect::<String>();
    match clipboard::copy(id) {
        Ok(()) => state.set_toast(format!("copied {short} to clipboard"), ToastKind::Success),
        Err(e) => state.set_toast(format!("clipboard unavailable: {e}"), ToastKind::Error),
    }
}

fn copy_project_path(state: &mut SearchState) {
    let Some(m) = state.selected_match() else {
        return;
    };
    let path = m.project_cwd.clone();
    let display = path.display().to_string();
    match clipboard::copy(display.clone()) {
        Ok(()) => {
            let shown = if display.len() > 40 {
                format!("…{}", &display[display.len() - 39..])
            } else {
                display
            };
            state.set_toast(format!("copied {shown} to clipboard"), ToastKind::Success);
        }
        Err(e) => state.set_toast(format!("clipboard unavailable: {e}"), ToastKind::Error),
    }
}

fn open_viewer(state: &mut SearchState) {
    let Some(m) = state.selected_match() else {
        return;
    };
    // Search screen only carries the snippet-scale metadata, so just open
    // with the id + session name and let the viewer parse its own JSONL
    // for the transcript.
    state.viewer = Some(ViewerState::open_with(
        &m.session_id,
        m.session_name.clone(),
        String::new(),
        String::new(),
        String::new(),
        0.0,
    ));
}

/// Dispatch an event into the open viewer. Mirrors `App::handle_viewer`.
fn handle_viewer_event(state: &mut SearchState, ev: Event) -> EventOutcome {
    let Some(viewer) = state.viewer.as_mut() else {
        return EventOutcome::Continue;
    };
    match viewer.handle_event(ev) {
        ViewerAction::None => EventOutcome::Continue,
        ViewerAction::Close => {
            state.viewer = None;
            EventOutcome::Continue
        }
        ViewerAction::Toast(message, kind) => {
            let local_kind = match kind {
                ViewerToastKind::Info => ToastKind::Info,
                ViewerToastKind::Success => ToastKind::Success,
                ViewerToastKind::Error => ToastKind::Error,
            };
            state.set_toast(message, local_kind);
            EventOutcome::Continue
        }
    }
}

fn open_editor(state: &mut SearchState) {
    let Some(m) = state.selected_match() else {
        return;
    };
    let path = m.project_cwd.clone();
    match editor::open_in_editor(&path) {
        Ok(name) => state.set_toast(
            format!("opened {} in {name}", path.display()),
            ToastKind::Info,
        ),
        Err(e) => state.set_toast(format!("editor: {e}"), ToastKind::Error),
    }
}

fn is_query_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            ' ' | '-'
                | '_'
                | '.'
                | '/'
                | '@'
                | '\''
                | '"'
                | '!'
                | '^'
                | '$'
                | '#'
                | '>'
                | '<'
                | '='
        )
}

fn move_cursor(state: &mut SearchState, delta: i32) {
    let len = state.filtered_indices.len();
    if len == 0 {
        return;
    }
    let current = state.cursor_clamped() as i32;
    let next = (current + delta).rem_euclid(len as i32);
    state.cursor = next as usize;
}

/// Re-score the corpus against the current query, pick the top K, and fill
/// `state.filtered_indices` + `state.all_matches` with fresh
/// [`SearchMatch`]es (snippet included).
fn recompute_matches(
    state: &mut SearchState,
    index: &[Indexed],
    matcher: &mut Matcher,
    pattern_slot: &mut Option<Pattern>,
) {
    state.filtered_indices.clear();

    if index.is_empty() {
        state.all_matches.clear();
        state.cursor = 0;
        state.active_filters = Vec::new();
        return;
    }

    // Parse the query into (filters, fuzzy_text). Filters apply *before*
    // nucleo so we only fuzzy-score rows that passed the hard filter —
    // cheaper and the ranking stays honest.
    let (filters, fuzzy_text) = search_filters::parse(&state.query);
    state.active_filters = filters.chip_labels();

    // Pre-filter the corpus. Collect (row_idx, &Indexed) so we can still
    // retrieve the original for snippet extraction after fuzzy scoring.
    let now = chrono::Utc::now();
    let prefiltered: Vec<(usize, &Indexed)> = index
        .iter()
        .enumerate()
        .filter(|(_, ix)| session_matches_filters(ix, &filters, now))
        .collect();

    if fuzzy_text.trim().is_empty() {
        // No fuzzy text — surviving rows in recency order.
        state.all_matches = prefiltered
            .iter()
            .map(|(_, ix)| SearchMatch {
                session_id: ix.session_id.clone(),
                project_name: ix.project_name.clone(),
                project_cwd: ix.project_cwd.clone(),
                session_name: ix.session_name.clone(),
                snippet: String::new(),
                score: 0,
            })
            .collect();
        *pattern_slot = None;
        // A query that's pure filters (`!bookmarked`) shows its matches
        // immediately. A truly-empty query still shows the "type to
        // search" empty state — consistent with the pre-filters UI.
        if filters.is_empty() {
            state.filtered_indices.clear();
        } else {
            state.filtered_indices = (0..state.all_matches.len()).collect();
        }
        state.cursor = 0;
        return;
    }

    // Build a Pattern per keystroke — `Pattern::parse` handles multi-word
    // queries, smart case, and normalization (the brief's preferred API).
    let pattern = Pattern::parse(&fuzzy_text, CaseMatching::Smart, Normalization::Smart);

    let mut scored: Vec<(u32, usize)> = Vec::new();
    for (i, ix) in prefiltered.iter() {
        if let Some(score) = pattern.score(ix.haystack.slice(..), matcher) {
            scored.push((score, *i));
        }
    }
    // Higher score first; tiebreak on recency (newer wins).
    scored.sort_unstable_by(|a, b| {
        b.0.cmp(&a.0).then_with(|| {
            let ta = index[a.1].last_ts;
            let tb = index[b.1].last_ts;
            tb.cmp(&ta)
        })
    });
    scored.truncate(MAX_MATCHES);
    *pattern_slot = Some(pattern);

    let needle = search_ui::dominant_word(&fuzzy_text);
    state.all_matches = scored
        .iter()
        .map(|(score, i)| {
            let ix = &index[*i];
            SearchMatch {
                session_id: ix.session_id.clone(),
                project_name: ix.project_name.clone(),
                project_cwd: ix.project_cwd.clone(),
                session_name: ix.session_name.clone(),
                snippet: extract_snippet(&ix.body, &needle),
                score: *score,
            }
        })
        .collect();
    state.filtered_indices = (0..state.all_matches.len()).collect();
    state.cursor = 0;
}

/// Evaluate a session's metadata against a parsed [`Filters`] set.
///
/// Each dimension is ANDed: a row must pass every active filter to survive.
/// `None` fields on the filter mean "no restriction" — they always pass.
fn session_matches_filters(ix: &Indexed, f: &Filters, now: chrono::DateTime<chrono::Utc>) -> bool {
    if let Some(want) = f.bookmarked {
        if ix.is_bookmarked != want {
            return false;
        }
    }
    if let Some(want) = f.named {
        if ix.has_custom_name != want {
            return false;
        }
    }
    if !f.models.is_empty() {
        let lower = ix.model_summary.to_lowercase();
        if !f.models.iter().any(|m| lower.contains(m)) {
            return false;
        }
    }
    if !f.permission_modes.is_empty() {
        let Some(actual) = ix.permission_mode else {
            return false;
        };
        if !f.permission_modes.contains(&actual) {
            return false;
        }
    }

    // Age / date filters — skipped when the session has no timestamp.
    if f.min_age.is_some() || f.max_age.is_some() || f.specific_date.is_some() {
        let Some(ts) = ix.last_ts else {
            return false;
        };
        if let Some(date) = f.specific_date {
            if search_filters::timestamp_to_local_date(ts) != date {
                return false;
            }
        } else {
            let age = now.signed_duration_since(ts);
            let age = age.to_std().unwrap_or_default();
            if let Some(min) = f.min_age {
                if age < min {
                    return false;
                }
            }
            if let Some(max) = f.max_age {
                if age > max {
                    return false;
                }
            }
        }
    }

    if let Some(min) = f.min_cost {
        if ix.total_cost_usd < min {
            return false;
        }
    }
    if let Some(max) = f.max_cost {
        if ix.total_cost_usd > max {
            return false;
        }
    }
    if let Some(min) = f.min_tokens {
        if ix.total_tokens < min {
            return false;
        }
    }
    if let Some(max) = f.max_tokens {
        if ix.total_tokens > max {
            return false;
        }
    }
    if let Some(min) = f.min_msgs {
        if ix.message_count < min {
            return false;
        }
    }
    if let Some(max) = f.max_msgs {
        if ix.message_count > max {
            return false;
        }
    }

    true
}

/// Scan every project + JSONL under `~/.claude/projects/`, build a searchable
/// body for each, and return the indexed set.
fn build_corpus() -> anyhow::Result<Vec<Indexed>> {
    let projects = discover_projects()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects_root = home.join(".claude").join("projects");

    // Bookmarks drive the `!bookmarked` filter. Failure (missing home dir,
    // unreadable file) is non-fatal — we just fall back to "no sessions
    // are bookmarked" so the filter still behaves in a consistent way.
    let bookmarks = BookmarkStore::load().ok();

    let mut out: Vec<Indexed> = Vec::new();
    // Keep project cwd → (project_name, project_cwd) so we can attribute
    // history.jsonl entries back to a project without re-discovering.
    let mut project_by_cwd: std::collections::HashMap<PathBuf, String> =
        std::collections::HashMap::with_capacity(projects.len());
    for project in &projects {
        project_by_cwd.insert(project.path.clone(), project.name.clone());
    }

    for project in &projects {
        let dir = projects_root.join(&project.encoded_dir);
        if !dir.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let session = match load_session_from_jsonl(&path, project.path.clone()) {
                Ok(Some(s)) => s,
                _ => continue,
            };
            let body = read_body_text(&path);
            if body.trim().is_empty() {
                continue;
            }
            // Pre-build the Utf32String the matcher needs. Includes the
            // session name + project name so filenames / titles are
            // searchable too (matches the Python reference UX).
            let composite = format!("{} {} {}", project.name, display_name(&session), body);
            let haystack = Utf32String::from(composite);

            let is_bookmarked = bookmarks.as_ref().is_some_and(|b| b.contains(&session.id));
            let has_custom_name = session.name.is_some();
            let model_summary = session.model_summary.clone();
            let permission_mode = session.permission_mode;
            let total_cost_usd = session.total_cost_usd;
            let total_tokens = session.tokens.total();
            let message_count = session.message_count;

            out.push(Indexed {
                session_id: session.id.clone(),
                project_name: project.name.clone(),
                project_cwd: project.path.clone(),
                session_name: display_name(&session),
                body,
                last_ts: session.last_timestamp,
                haystack,
                is_bookmarked,
                has_custom_name,
                model_summary,
                permission_mode,
                total_cost_usd,
                total_tokens,
                message_count,
            });
        }
    }

    // Supplementary corpus: `~/.claude/history.jsonl` is a cross-session
    // rollup of every prompt the user typed. Entries we don't already
    // have in the per-project corpus become standalone hits so a search
    // hits them too. Entries *already* covered (same sessionId as a
    // project-level row) just append text to that row's body so the
    // ranking benefits without double-counting.
    let history_path = home.join(".claude").join("history.jsonl");
    if let Some(history) = load_history_entries(&history_path) {
        merge_history_into_corpus(&mut out, history, &project_by_cwd);
    }

    // Newest first — ties in score prefer recent sessions.
    out.sort_by_key(|ix| std::cmp::Reverse(ix.last_ts));
    Ok(out)
}

/// One line of `~/.claude/history.jsonl` — a cross-session user-prompt
/// rollup Claude Code writes independent of the per-project JSONL.
#[derive(serde::Deserialize)]
struct HistoryEntry {
    #[serde(default)]
    display: Option<String>,
    #[serde(default, rename = "sessionId")]
    session_id: Option<String>,
    #[serde(default)]
    project: Option<PathBuf>,
    /// Epoch millis.
    #[serde(default)]
    timestamp: Option<i64>,
}

fn load_history_entries(path: &std::path::Path) -> Option<Vec<HistoryEntry>> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry): Result<HistoryEntry, _> = serde_json::from_str(trimmed) else {
            continue;
        };
        // Drop empty or slash-command-only rows — they're noise, not intent.
        let text = entry.display.as_deref().unwrap_or("").trim();
        if text.is_empty() || (text.starts_with('/') && !text.contains(' ')) {
            continue;
        }
        out.push(entry);
    }
    Some(out)
}

/// Fold history.jsonl entries into the existing corpus.
///
/// Two outcomes per entry:
///
/// 1. Session already in corpus → append the prompt text to that row's
///    body + haystack so score/snippet improve. This is the common case
///    because the session's own JSONL has the same text in richer form.
/// 2. Session missing → create a new entry. This picks up sessions the
///    per-project loader drops (too few messages, non-CLI entrypoint,
///    …) but that still had a human-readable prompt worth searching.
fn merge_history_into_corpus(
    corpus: &mut Vec<Indexed>,
    history: Vec<HistoryEntry>,
    project_by_cwd: &std::collections::HashMap<PathBuf, String>,
) {
    let mut id_to_idx: std::collections::HashMap<String, usize> =
        std::collections::HashMap::with_capacity(corpus.len());
    for (i, ix) in corpus.iter().enumerate() {
        id_to_idx.insert(ix.session_id.clone(), i);
    }

    for entry in history {
        let Some(sid) = entry.session_id.as_deref() else {
            continue;
        };
        let text = entry.display.as_deref().unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        if let Some(&i) = id_to_idx.get(sid) {
            let ix = &mut corpus[i];
            ix.body.push('\n');
            ix.body.push_str(text);
            ix.haystack = Utf32String::from(format!(
                "{} {} {}",
                ix.project_name, ix.session_name, ix.body
            ));
            continue;
        }
        // New entry. Attribute it to a project when we can resolve the
        // cwd; otherwise synthesise a minimal row so the prompt is still
        // searchable.
        let cwd = entry.project.unwrap_or_default();
        let project_name = project_by_cwd.get(&cwd).cloned().unwrap_or_else(|| {
            cwd.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".to_string())
        });
        let last_ts = entry
            .timestamp
            .and_then(chrono::DateTime::from_timestamp_millis);
        let composite = format!("{} {} {}", project_name, sid, text);
        let ix = Indexed {
            session_id: sid.to_string(),
            project_name,
            project_cwd: cwd,
            session_name: text.chars().take(50).collect(),
            body: text.to_string(),
            last_ts,
            haystack: Utf32String::from(composite),
            // History-only rows lack the per-session metadata, so they're
            // treated as "not-bookmarked, no custom name, no model, no
            // cost". Filters that check those dimensions will exclude
            // them — which is the honest answer because the session's
            // per-project file is the source of truth for cost/tokens.
            is_bookmarked: false,
            has_custom_name: false,
            model_summary: String::new(),
            permission_mode: None,
            total_cost_usd: 0.0,
            total_tokens: 0,
            message_count: 0,
        };
        id_to_idx.insert(sid.to_string(), corpus.len());
        corpus.push(ix);
    }
}

fn display_name(session: &Session) -> String {
    session.display_label().to_string()
}

/// Second pass over a JSONL that pulls user + assistant message bodies as
/// one big string. This is deliberately separate from
/// `load_session_from_jsonl` — that aggregator cares about tokens, cost, and
/// per-model stats; here we only want searchable text.
fn read_body_text(path: &std::path::Path) -> String {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let Ok(file) = File::open(path) else {
        return String::new();
    };
    let reader = BufReader::new(file);

    let noise = noise_prefixes();
    let mut out = String::with_capacity(4 * 1024);

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "user" && kind != "assistant" {
            continue;
        }
        let role = value
            .pointer("/message/role")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if role != "user" && role != "assistant" {
            continue;
        }
        let content = value.pointer("/message/content");
        let text = match content {
            Some(serde_json::Value::String(s)) => s.trim().to_string(),
            Some(serde_json::Value::Array(blocks)) => {
                let mut collected = String::new();
                for b in blocks {
                    if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                        if let Some(t) = b.get("text").and_then(|v| v.as_str()) {
                            if !collected.is_empty() {
                                collected.push(' ');
                            }
                            collected.push_str(t.trim());
                        }
                    }
                }
                collected
            }
            _ => String::new(),
        };
        if text.chars().count() < 5 {
            continue;
        }
        if noise.iter().any(|n| text.contains(n)) {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&text);
    }
    out
}

fn install_panic_hook() {
    use crossterm::event::DisableMouseCapture;
    use crossterm::execute;
    use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut stdout = std::io::stdout();
        let _ = disable_raw_mode();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_outranks_partial() {
        // Build two haystacks: one contains "race condition" verbatim, the
        // other spreads the letters across unrelated words. Nucleo's
        // fuzzy-parse Pattern must score the contiguous hit higher.
        let mut matcher = Matcher::new(Config::DEFAULT);
        let p = Pattern::parse("race condition", CaseMatching::Smart, Normalization::Smart);

        let a = Utf32String::from("fix race condition in order processing");
        let b = Utf32String::from("render the cache cone of despair distribution");
        let sa = p.score(a.slice(..), &mut matcher);
        let sb = p.score(b.slice(..), &mut matcher);

        assert!(sa.is_some(), "contiguous match must score");
        // The "cache cone of despair distribution" string doesn't contain the
        // contiguous letters of "condition" — it may not match at all, which
        // is still an "a > b" outcome. If it does, it must be lower.
        match sb {
            None => {} // fine — a matched, b didn't.
            Some(sb) => assert!(
                sa.unwrap() > sb,
                "exact should outrank partial: {} vs {}",
                sa.unwrap(),
                sb
            ),
        }
    }

    #[test]
    fn is_query_char_accepts_typical_search_chars() {
        assert!(is_query_char('a'));
        assert!(is_query_char(' '));
        assert!(is_query_char('-'));
        assert!(is_query_char('\''));
        assert!(!is_query_char('\n'));
        assert!(!is_query_char('\t'));
    }

    #[test]
    fn merge_history_appends_to_existing_rows_and_creates_new_ones() {
        let mut corpus = vec![Indexed {
            session_id: "existing".into(),
            project_name: "alpha".into(),
            project_cwd: PathBuf::from("/tmp/alpha"),
            session_name: "alpha session".into(),
            body: "one line of body".into(),
            last_ts: None,
            haystack: Utf32String::from("alpha alpha session one line of body"),
            is_bookmarked: false,
            has_custom_name: false,
            model_summary: String::new(),
            permission_mode: None,
            total_cost_usd: 0.0,
            total_tokens: 0,
            message_count: 0,
        }];
        let mut project_by_cwd: std::collections::HashMap<PathBuf, String> =
            std::collections::HashMap::new();
        project_by_cwd.insert(PathBuf::from("/tmp/beta"), "beta".into());

        let history = vec![
            HistoryEntry {
                display: Some("extra prompt for existing".into()),
                session_id: Some("existing".into()),
                project: Some(PathBuf::from("/tmp/alpha")),
                timestamp: Some(1_700_000_000_000),
            },
            HistoryEntry {
                display: Some("a brand-new prompt".into()),
                session_id: Some("missing".into()),
                project: Some(PathBuf::from("/tmp/beta")),
                timestamp: Some(1_700_000_100_000),
            },
            // Slash-only row should be filtered out before reaching merge.
            HistoryEntry {
                display: Some("/help".into()),
                session_id: Some("ignored".into()),
                project: Some(PathBuf::from("/tmp/alpha")),
                timestamp: None,
            },
        ];

        merge_history_into_corpus(&mut corpus, history, &project_by_cwd);

        // Existing row grew.
        assert!(corpus[0].body.contains("extra prompt for existing"));
        // New row appended.
        assert!(corpus.iter().any(|ix| ix.session_id == "missing"));
        assert_eq!(
            corpus
                .iter()
                .find(|ix| ix.session_id == "missing")
                .unwrap()
                .project_name,
            "beta"
        );
        // Slash-only rows were pre-filtered by `load_history_entries`; if
        // one slips through merge still accepts non-empty display values.
        assert!(corpus.iter().any(|ix| ix.session_id == "ignored"));
    }

    #[test]
    fn load_history_entries_drops_empty_and_slash_only_rows() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("history.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"display\":\"fix the auth redirect loop\",\"sessionId\":\"sid-1\"}\n",
                "{\"display\":\"/cost \",\"sessionId\":\"sid-2\"}\n",
                "{\"display\":\"\",\"sessionId\":\"sid-3\"}\n",
                "{\"display\":\"/rename to banana\",\"sessionId\":\"sid-4\"}\n",
            ),
        )
        .expect("write");
        let entries = load_history_entries(&path).expect("some");
        let ids: Vec<_> = entries
            .iter()
            .map(|e| e.session_id.as_deref().unwrap_or(""))
            .collect();
        assert!(ids.contains(&"sid-1"), "kept real prompt");
        assert!(!ids.contains(&"sid-2"), "dropped bare /cost");
        assert!(!ids.contains(&"sid-3"), "dropped empty");
        assert!(
            ids.contains(&"sid-4"),
            "kept /rename ... (has space so not bare-command)"
        );
    }
}
