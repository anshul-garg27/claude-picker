//! `claude-picker --files` / `claude-picker files` — file-centric pivot
//! dispatcher.
//!
//! Owns the event loop and the background index load. Pure glue — the
//! index math lives in [`crate::data::file_index`] and the rendering lives
//! in [`crate::ui::files`].
//!
//! Load strategy:
//!
//!   1. Try to read a cached index from
//!      `~/.config/claude-picker/file-index.json`. Fresh cache ⇒ use it.
//!   2. Otherwise spawn a background worker that builds the index from
//!      scratch and streams progress (session count) back over an mpsc
//!      channel. The UI shows "Scanning sessions… N found" until the
//!      full payload lands.
//!   3. On successful build we save the new cache.
//!
//! The user sees a responsive list within one frame even on a cold
//! launch with hundreds of sessions.

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use crate::app::{restore_terminal, setup_terminal};
use crate::data::editor;
use crate::data::file_index::FileIndex;
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::files::{self as files_ui, Focus, Sort, ToastKind};
use crate::ui::help_overlay;

/// One message shuttled back from the loader thread.
enum Progress {
    Scanned(u32),
    Done(FileIndex),
}

/// Public entry point. `project_filter`, when `Some`, restricts the scan
/// to a single project (by name). Matches the `--files --project foo`
/// CLI path.
pub fn run(project_filter: Option<String>) -> anyhow::Result<()> {
    // ── Step 0: try cache ─────────────────────────────────────────────────
    let cache_path = FileIndex::default_cache_path();
    let cached = cache_path.as_deref().and_then(FileIndex::load_cached);
    let (initial_index, use_cache) = match cached {
        Some(mut idx) => {
            if let Some(filter) = &project_filter {
                idx.files.retain(|f| f.project_name == *filter);
            }
            idx.filter_junk();
            (Some(idx), true)
        }
        None => (None, false),
    };

    // ── Step 1: kick off the loader ───────────────────────────────────────
    let (tx, rx) = mpsc::channel::<Progress>();
    if !use_cache {
        let filter_for_worker = project_filter.clone();
        let cache_path_for_worker = cache_path.clone();
        thread::spawn(move || {
            let tx_progress = tx.clone();
            let idx = FileIndex::build(filter_for_worker.as_deref(), move |count| {
                let _ = tx_progress.send(Progress::Scanned(count));
            })
            .unwrap_or_default();
            if let Some(path) = cache_path_for_worker.as_deref() {
                let _ = idx.save_cache(path);
            }
            let _ = tx.send(Progress::Done(idx));
        });
    } else {
        // Drop the dangling sender so `rx.try_recv()` returns Disconnected
        // immediately on the fast path. The state machine already tolerates
        // this: `loading` is false and we never wait for messages.
        drop(tx);
    }

    // ── Step 2: terminal lifecycle + event loop ───────────────────────────
    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let theme = Theme::mocha();
    let mut state = files_ui::FilesState::new(project_filter.clone());
    if let Some(idx) = initial_index {
        state.index = idx;
        state.loading = false;
        state.recompute();
    }

    let result: anyhow::Result<Option<(String, PathBuf)>> = (|| {
        loop {
            // Drain loader messages.
            if state.loading {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        Progress::Scanned(n) => state.loader_progress = n,
                        Progress::Done(mut idx) => {
                            idx.filter_junk();
                            state.index = idx;
                            state.loading = false;
                            state.recompute();
                        }
                    }
                }
            }

            state.tick();
            terminal.draw(|f| files_ui::render(f, f.area(), &mut state, &theme))?;

            let Some(ev) = events::next()? else {
                // Poll cycle elapsed with no event — let the loader drain
                // again on the next iteration. Don't sleep: the poll
                // inside `events::next` already budgets 50 ms.
                continue;
            };
            match handle_event(&mut state, ev) {
                EventOutcome::Continue => {}
                EventOutcome::Quit => return Ok(None),
                EventOutcome::Resume(id, cwd) => return Ok(Some((id, cwd))),
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

/// Event-handler outcome. Mirrors `commands::search_cmd`.
enum EventOutcome {
    Continue,
    Quit,
    Resume(String, PathBuf),
}

fn handle_event(state: &mut files_ui::FilesState, ev: Event) -> EventOutcome {
    // Help overlay steals input while visible.
    if state.show_help {
        match ev {
            Event::Escape => state.show_help = false,
            Event::Key(c) if help_overlay::is_dismiss_key(c) => state.show_help = false,
            _ => {}
        }
        return EventOutcome::Continue;
    }

    match ev {
        Event::Quit | Event::Ctrl('c') => EventOutcome::Quit,
        Event::Escape => {
            // Esc semantics: clear filter, pop focus to files, else quit.
            if !state.filter.is_empty() {
                state.filter.clear();
                state.recompute();
                EventOutcome::Continue
            } else if state.focus == Focus::SessionList {
                state.focus = Focus::FileList;
                EventOutcome::Continue
            } else {
                EventOutcome::Quit
            }
        }
        Event::Key('q') if state.filter.is_empty() => EventOutcome::Quit,
        Event::Tab => {
            state.focus = match state.focus {
                Focus::FileList => Focus::SessionList,
                Focus::SessionList => Focus::FileList,
            };
            EventOutcome::Continue
        }
        Event::Up | Event::Key('k') if state.filter.is_empty() => {
            move_cursor(state, -1);
            EventOutcome::Continue
        }
        Event::Down | Event::Key('j') if state.filter.is_empty() => {
            move_cursor(state, 1);
            EventOutcome::Continue
        }
        // Arrows always navigate, even mid-filter — typing "j" into a
        // filter is legitimate, but a user pressing the Up arrow always
        // means "move cursor".
        Event::Up => {
            move_cursor(state, -1);
            EventOutcome::Continue
        }
        Event::Down => {
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
            match state.focus {
                Focus::FileList => state.file_cursor = 0,
                Focus::SessionList => state.session_cursor = 0,
            }
            EventOutcome::Continue
        }
        Event::End => {
            match state.focus {
                Focus::FileList => {
                    state.file_cursor = state.visible.len().saturating_sub(1);
                }
                Focus::SessionList => {
                    if let Some(f) = state.focused_file() {
                        state.session_cursor = f.sessions.len().saturating_sub(1);
                    }
                }
            }
            EventOutcome::Continue
        }
        Event::Enter => {
            if state.focus == Focus::SessionList {
                if let Some(s) = state.focused_session() {
                    return EventOutcome::Resume(s.session_id.clone(), s.project_cwd.clone());
                }
            }
            EventOutcome::Continue
        }
        Event::Key('?') if state.filter.is_empty() => {
            state.show_help = true;
            EventOutcome::Continue
        }
        Event::Key('s') if state.filter.is_empty() && state.focus == Focus::FileList => {
            state.sort = state.sort.next();
            state.recompute();
            state.set_toast(
                format!("sort: {}", sort_label_verbose(state.sort)),
                ToastKind::Info,
            );
            EventOutcome::Continue
        }
        Event::Key('/') if state.filter.is_empty() => {
            // Placeholder — `/` is meaningful only when it's the first char
            // of a new filter. Suppressed to avoid polluting the filter
            // with a literal '/'.
            EventOutcome::Continue
        }
        Event::Key('o') if state.filter.is_empty() && state.focus == Focus::FileList => {
            if let Some(f) = state.focused_file().cloned() {
                if f.path.exists() {
                    match editor::open_in_editor(&f.path) {
                        Ok(name) => state.set_toast(
                            format!("opened {} in {name}", f.path.display()),
                            ToastKind::Info,
                        ),
                        Err(e) => state.set_toast(format!("editor: {e}"), ToastKind::Error),
                    }
                } else {
                    state.set_toast(
                        format!("file not on disk: {}", f.path.display()),
                        ToastKind::Error,
                    );
                }
            }
            EventOutcome::Continue
        }
        Event::Backspace => {
            if !state.filter.is_empty() {
                state.filter.pop();
                state.recompute();
            }
            EventOutcome::Continue
        }
        Event::Key(c) if is_filter_char(c) => {
            state.filter.push(c);
            state.recompute();
            EventOutcome::Continue
        }
        _ => EventOutcome::Continue,
    }
}

/// What the `s` toast says after a cycle — a human-readable rendering of
/// the chosen mode.
fn sort_label_verbose(s: Sort) -> &'static str {
    match s {
        Sort::EditsDesc => "most edited first",
        Sort::RecencyDesc => "most recent first",
        Sort::SessionCountDesc => "most-session files first",
        Sort::PathAlpha => "alphabetical by path",
    }
}

fn move_cursor(state: &mut files_ui::FilesState, delta: i32) {
    match state.focus {
        Focus::FileList => {
            let len = state.visible.len();
            if len == 0 {
                return;
            }
            let cur = state.file_cursor_clamped() as i32;
            state.file_cursor = (cur + delta).rem_euclid(len as i32) as usize;
            state.session_cursor = 0;
        }
        Focus::SessionList => {
            let Some(f) = state.focused_file() else {
                return;
            };
            let len = f.sessions.len();
            if len == 0 {
                return;
            }
            let cur = state.session_cursor_clamped() as i32;
            state.session_cursor = (cur + delta).rem_euclid(len as i32) as usize;
        }
    }
}

fn is_filter_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '/' | '@' | '#' | '+' | '=' | ':')
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
    fn is_filter_char_accepts_path_glyphs() {
        assert!(is_filter_char('a'));
        assert!(is_filter_char('/'));
        assert!(is_filter_char('.'));
        assert!(is_filter_char('-'));
        assert!(is_filter_char('_'));
        assert!(!is_filter_char('\n'));
        assert!(!is_filter_char('\t'));
    }

    #[test]
    fn sort_label_verbose_is_useful() {
        assert!(sort_label_verbose(Sort::EditsDesc).contains("edit"));
        assert!(sort_label_verbose(Sort::RecencyDesc).contains("recent"));
        assert!(sort_label_verbose(Sort::SessionCountDesc).contains("session"));
        assert!(sort_label_verbose(Sort::PathAlpha).contains("alphab"));
    }
}
