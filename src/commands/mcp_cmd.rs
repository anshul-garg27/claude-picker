//! `claude-picker mcp` / `--mcp` handler.
//!
//! Builds the server + tool lists by combining:
//! - `~/.claude/settings.json` → `mcpServers` map (declared installs)
//! - `~/.claude.json` → top-level `mcpServers` (legacy alt location)
//! - every JSONL under `~/.claude/projects/` (observed invocations)
//!
//! and drives the [`ui::mcp`] renderer.
//!
//! Key bindings:
//! - `q` / `Esc` / `Ctrl+C` — quit.
//! - `↑` / `↓` — move selection.
//! - `Enter` — emits a stderr hint for the session filter the picker would
//!   apply. Full drill-down is deferred; the panel itself surfaces the
//!   relevant session ids so shell callers can pipe them.
//! - `c` — open `~/.claude/settings.json` in `$EDITOR` for config edits.

use std::collections::BTreeMap;
use std::io::{self, Stdout};
use std::path::PathBuf;

use chrono::Utc;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::editor::open_in_editor;
use crate::data::mcp_calls::{self, McpCallData, ServerStats};
use crate::data::settings::Settings;
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::mcp::{self, McpView};

use super::hooks_cmd::relative_from;

pub fn run() -> anyhow::Result<()> {
    let (servers, top_tools, total_calls) = collect()?;
    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let result: anyhow::Result<()> = (|| {
        let theme = Theme::mocha();
        let mut selected: usize = 0;
        let mut should_quit = false;

        let now = Utc::now();
        let last_labels: Vec<String> = servers
            .iter()
            .map(|s| {
                s.last_used
                    .map(|t| relative_from(now, t))
                    .unwrap_or_default()
            })
            .collect();

        while !should_quit {
            terminal.draw(|f| {
                let view = McpView {
                    servers: &servers,
                    top_tools: &top_tools,
                    selected,
                    total_calls,
                    last_used_labels: &last_labels,
                };
                mcp::render(f, f.area(), &view, &theme);
            })?;

            let Some(ev) = events::next()? else { continue };
            match ev {
                Event::Quit | Event::Escape | Event::Ctrl('c') => should_quit = true,
                Event::Key('q') => should_quit = true,
                Event::Up => {
                    selected = selected.saturating_sub(1);
                }
                Event::Down if !servers.is_empty() && selected + 1 < servers.len() => {
                    selected += 1;
                }
                Event::Enter => {
                    if let Some(s) = servers.get(selected) {
                        eprintln!(
                            "(mcp → sessions using {}: {} match)",
                            s.name,
                            s.sessions.len()
                        );
                        for sid in s.sessions.iter().take(10) {
                            eprintln!("  {sid}");
                        }
                    }
                }
                Event::Key('c') => {
                    let path = dirs::home_dir()
                        .map(|h| h.join(".claude").join("settings.json"))
                        .unwrap_or_else(|| PathBuf::from("settings.json"));
                    let _ = open_in_editor(&path);
                }
                _ => {}
            }
        }
        Ok(())
    })();

    let _ = restore_terminal(&mut terminal);
    result
}

/// Pull declared servers from `~/.claude/settings.json` *and*
/// `~/.claude.json` (Claude Code historically wrote the map in both places).
/// Merge with observed calls from JSONL and return a UI-ordered list.
fn collect() -> anyhow::Result<(
    Vec<ServerStats>,
    Vec<crate::data::mcp_calls::ToolCallStats>,
    u64,
)> {
    let settings = Settings::load_all();
    let mut declared: Vec<String> = settings
        .mcp_servers
        .iter()
        .map(|s| s.name.clone())
        .collect();

    if let Some(home) = dirs::home_dir() {
        let path = home.join(".claude.json");
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(obj) = v.get("mcpServers").and_then(|x| x.as_object()) {
                    for k in obj.keys() {
                        if !declared.contains(k) {
                            declared.push(k.clone());
                        }
                    }
                }
            }
        }
    }

    let scan: McpCallData = mcp_calls::scan_mcp_calls().unwrap_or_default();
    // Take the counts before the partial moves below.
    let total_calls = scan.total_calls();
    let merged: BTreeMap<String, ServerStats> = mcp_calls::merge_declared(&declared, &scan.servers);

    // Sort: calls desc, then name asc.
    let mut servers: Vec<ServerStats> = merged.into_values().collect();
    servers.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.name.cmp(&b.name)));

    Ok((servers, scan.tools, total_calls))
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
    // `collect()` touches the real $HOME, so we don't exercise it directly
    // here — the individual building blocks (data::settings, data::mcp_calls)
    // have their own tests. This module's only bespoke logic is sorting +
    // the key map; both are covered by the renderer snapshot tests in
    // `ui::mcp`.
    #[test]
    fn module_compiles() {
        // Presence test; avoids an empty `mod tests`.
    }
}
