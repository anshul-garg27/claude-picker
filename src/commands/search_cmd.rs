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
use std::time::Duration;

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};

use crate::app::{restore_terminal, setup_terminal};
use crate::data::project::discover_projects;
use crate::data::session::{load_session_from_jsonl, noise_prefixes};
use crate::data::Session;
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::search as search_ui;

use search_ui::{extract_snippet, render, SearchMatch, SearchState};

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

            terminal.draw(|f| render(f, f.area(), &state, &theme))?;

            if let Some(ev) = events::next()? {
                let outcome = handle_event(&mut state, ev, &index, &mut matcher, &mut pattern);
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
) -> EventOutcome {
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
            move_cursor(state, -1);
            EventOutcome::Continue
        }
        Event::Down | Event::Key('j') => {
            move_cursor(state, 1);
            EventOutcome::Continue
        }
        Event::PageUp => {
            move_cursor(state, -10);
            EventOutcome::Continue
        }
        Event::PageDown => {
            move_cursor(state, 10);
            EventOutcome::Continue
        }
        Event::Home => {
            state.cursor = 0;
            EventOutcome::Continue
        }
        Event::End => {
            state.cursor = state.filtered_indices.len().saturating_sub(1);
            EventOutcome::Continue
        }
        Event::Backspace => {
            state.query.pop();
            recompute_matches(state, index, matcher, pattern);
            EventOutcome::Continue
        }
        Event::Key('p') if state.query.is_empty() => {
            // `p` toggles preview only when the filter is empty — otherwise
            // it's a regular filter char.
            state.preview_visible = !state.preview_visible;
            EventOutcome::Continue
        }
        Event::Key(c) if is_query_char(c) => {
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
        _ => EventOutcome::Continue,
    }
}

fn is_query_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            ' ' | '-' | '_' | '.' | '/' | '@' | '\'' | '"' | '!' | '^' | '$'
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
        return;
    }

    if state.query.is_empty() {
        // Empty query: just mirror the index in recency order. No snippet.
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
        // Default to showing nothing in the list when the query is empty —
        // the "type to search" empty state reads cleaner than a ranked list
        // of every session.
        *pattern_slot = None;
        state.cursor = 0;
        return;
    }

    // Build a Pattern per keystroke — `Pattern::parse` handles multi-word
    // queries, smart case, and normalization (the brief's preferred API).
    let pattern = Pattern::parse(&state.query, CaseMatching::Smart, Normalization::Smart);

    let mut scored: Vec<(u32, usize)> = Vec::new();
    for (i, ix) in index.iter().enumerate() {
        if let Some(score) = pattern.score(ix.haystack.slice(..), matcher) {
            scored.push((score, i));
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

    let needle = search_ui::dominant_word(&state.query);
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

/// Scan every project + JSONL under `~/.claude/projects/`, build a searchable
/// body for each, and return the indexed set.
fn build_corpus() -> anyhow::Result<Vec<Indexed>> {
    let projects = discover_projects()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects_root = home.join(".claude").join("projects");

    let mut out: Vec<Indexed> = Vec::new();
    for project in projects {
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

            out.push(Indexed {
                session_id: session.id.clone(),
                project_name: project.name.clone(),
                project_cwd: project.path.clone(),
                session_name: display_name(&session),
                body,
                last_ts: session.last_timestamp,
                haystack,
            });
        }
    }
    // Newest first — ties in score prefer recent sessions.
    out.sort_by_key(|ix| std::cmp::Reverse(ix.last_ts));
    Ok(out)
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
}
