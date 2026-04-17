//! `claude-picker audit` — interactive cost-optimization report.
//!
//! Walks every session on disk, runs the heuristics in
//! [`crate::data::cost_audit`], and presents the results as a scrollable
//! Ratatui list. Enter on any row resumes that session so the user can act
//! on the suggestion immediately.
//!
//! No mutation occurs here — we never rewrite user data. The audit is
//! strictly observational; the remediation belongs to the user.

use std::io::{self, Stdout};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::cost_audit;
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::audit::{self, AuditState};
use crate::ui::help_overlay::{self, Screen as HelpScreen};

/// Entry point for `claude-picker audit`.
pub fn run() -> anyhow::Result<()> {
    let findings = cost_audit::run_audit()?;

    if findings.is_empty() {
        eprintln!();
        eprintln!("  No cost-savings suggestions — every session looks efficient.");
        eprintln!("  Run more Claude Code sessions and try again later.");
        eprintln!();
        return Ok(());
    }

    let mut state = AuditState::new(findings);
    let theme = Theme::mocha();

    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let result: anyhow::Result<Option<audit::AuditSelection>> = (|| {
        while !state.should_quit {
            terminal.draw(|f| render_screen(f, &mut state, &theme))?;
            if let Some(ev) = events::next()? {
                handle_event(&mut state, ev);
            }
        }
        Ok(state.selection.take())
    })();

    let _ = restore_terminal(&mut terminal);

    if let Some(sel) = result? {
        crate::resume::resume_session(&sel.session_id, &sel.project_cwd); // diverges
    }
    Ok(())
}

/// Draw one frame: audit body + help overlay if toggled.
fn render_screen(f: &mut ratatui::Frame<'_>, state: &mut AuditState, theme: &Theme) {
    let area = f.area();
    audit::render(f, area, state, theme);
    if state.show_help {
        // Re-use the Stats help content since both screens share the same
        // minimal keybinding set — we only care about q/esc and help.
        let content = help_overlay::help_for(HelpScreen::Stats);
        help_overlay::render(f, area, content, theme);
    }
}

fn handle_event(state: &mut AuditState, ev: Event) {
    if state.show_help {
        match ev {
            Event::Escape => state.show_help = false,
            Event::Key(c) if help_overlay::is_dismiss_key(c) => state.show_help = false,
            _ => {}
        }
        return;
    }
    match ev {
        Event::Quit | Event::Escape | Event::Ctrl('c') => state.should_quit = true,
        Event::Key('q') => state.should_quit = true,
        Event::Key('?') => state.show_help = true,
        Event::Up | Event::Key('k') => state.move_cursor(-1),
        Event::Down | Event::Key('j') => state.move_cursor(1),
        Event::PageUp => state.move_cursor(-10),
        Event::PageDown => state.move_cursor(10),
        Event::Home => state.cursor = 0,
        Event::End => state.cursor = state.findings.len().saturating_sub(1),
        Event::Enter => state.confirm(),
        _ => {}
    }
}

// ── Terminal lifecycle ───────────────────────────────────────────────────

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
    use super::*;
    use crate::data::cost_audit::{AuditFinding, Finding, Severity};
    use std::path::PathBuf;

    fn mk(id: &str, savings: f64) -> AuditFinding {
        AuditFinding {
            session_id: id.into(),
            project_name: "p".into(),
            project_cwd: PathBuf::from("/tmp"),
            session_label: id.into(),
            total_cost_usd: 1.0,
            model_summary: "claude-opus-4-7".into(),
            findings: vec![Finding {
                severity: Severity::Warn,
                message: "msg".into(),
                savings_usd: savings,
            }],
            estimated_savings_usd: savings,
        }
    }

    #[test]
    fn q_quits_handler() {
        let mut state = AuditState::new(vec![mk("a", 1.0)]);
        handle_event(&mut state, Event::Key('q'));
        assert!(state.should_quit);
    }

    #[test]
    fn enter_sets_selection() {
        let mut state = AuditState::new(vec![mk("abc", 1.0), mk("xyz", 2.0)]);
        state.cursor = 1;
        handle_event(&mut state, Event::Enter);
        let sel = state.selection.expect("selection");
        assert_eq!(sel.session_id, "xyz");
    }

    #[test]
    fn arrow_keys_navigate() {
        let mut state = AuditState::new(vec![mk("a", 1.0), mk("b", 1.0)]);
        handle_event(&mut state, Event::Down);
        assert_eq!(state.cursor, 1);
        handle_event(&mut state, Event::Up);
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn help_toggle_and_dismiss() {
        let mut state = AuditState::new(vec![mk("a", 1.0)]);
        handle_event(&mut state, Event::Key('?'));
        assert!(state.show_help);
        handle_event(&mut state, Event::Escape);
        assert!(!state.show_help);
    }
}
