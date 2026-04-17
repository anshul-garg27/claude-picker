//! `claude-picker checkpoints` / `--checkpoints` handler.
//!
//! Walks `~/.claude/file-history/` + matching session JSONLs and drives the
//! [`ui::checkpoints`] renderer.
//!
//! Key bindings:
//! - `q` / `Esc` / `Ctrl+C` — quit.
//! - `↑` / `↓` — move selection over the flat checkpoint list.
//! - `Enter` — emit a stderr hint describing the diff command the user
//!   could run (we don't render diffs here — that's the diff module's job).
//! - `r` — emit the `claude --resume <sid>` command the user would type to
//!   rewind. We don't exec it for them to avoid clobbering their terminal.

use std::io::{self, Stdout};

use chrono::Utc;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::checkpoints::{self, CheckpointData};
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::checkpoints::{self as cp_ui, CheckpointCursor, CheckpointsView};

use super::hooks_cmd::relative_from;

pub fn run() -> anyhow::Result<()> {
    let data: CheckpointData = checkpoints::scan().unwrap_or_default();
    let flat = cp_ui::flatten_rows(&data.sessions);

    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let result: anyhow::Result<()> = (|| {
        let theme = Theme::mocha();
        // Pointer into the flat list — rendered + bookkept here.
        let mut flat_idx: usize = 0;
        let mut should_quit = false;

        let now = Utc::now();
        let relative_labels: Vec<String> = flat
            .iter()
            .map(|(i, j)| {
                data.sessions[*i].checkpoints[*j]
                    .timestamp
                    .map(|t| relative_from(now, t))
                    .unwrap_or_default()
            })
            .collect();

        while !should_quit {
            let cursor = flat
                .get(flat_idx)
                .map(|(i, j)| CheckpointCursor {
                    session_index: *i,
                    checkpoint_index: *j,
                })
                .unwrap_or(CheckpointCursor {
                    session_index: 0,
                    checkpoint_index: 0,
                });

            terminal.draw(|f| {
                let view = CheckpointsView {
                    sessions: &data.sessions,
                    selected: cursor,
                    total: data.total_checkpoints(),
                    relative_labels: &relative_labels,
                };
                cp_ui::render(f, f.area(), &view, &theme);
            })?;

            let Some(ev) = events::next()? else { continue };
            match ev {
                Event::Quit | Event::Escape | Event::Ctrl('c') => should_quit = true,
                Event::Key('q') => should_quit = true,
                Event::Up => {
                    flat_idx = flat_idx.saturating_sub(1);
                }
                Event::Down if flat_idx + 1 < flat.len() => {
                    flat_idx += 1;
                }
                Event::Enter => {
                    if let Some((i, j)) = flat.get(flat_idx) {
                        let cp = &data.sessions[*i].checkpoints[*j];
                        eprintln!(
                            "(diff preview) snapshot {} · session {} · {} file(s)",
                            cp.short_hash(),
                            cp.session_id,
                            cp.files.len()
                        );
                        for f in &cp.files {
                            eprintln!("  {} @v{}", f.real_path.display(), f.version);
                        }
                    }
                }
                Event::Key('r') => {
                    if let Some((i, j)) = flat.get(flat_idx) {
                        let cp = &data.sessions[*i].checkpoints[*j];
                        eprintln!(
                            "(rewind) run: claude --resume {} /rewind {}",
                            cp.session_id, cp.message_id
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })();

    let _ = restore_terminal(&mut terminal);
    result
}

// ── Terminal lifecycle ────────────────────────────────────────────────────

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    terminal.show_cursor()?;
    Ok(())
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
    #[test]
    fn module_compiles() {}
}
