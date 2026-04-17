//! `claude-picker hooks` / `--hooks` handler.
//!
//! Reads `~/.claude/settings.json` (+ per-project overrides), walks session
//! JSONLs for hook-execution events, and drives the [`ui::hooks`] renderer.
//!
//! Key bindings:
//! - `q` / `Esc` / `Ctrl+C` — quit.
//! - `↑` / `↓` — move selection over the flat hook list.
//! - `Enter` — currently prints the filter text that would be used when the
//!   session picker grows a preset filter. Safe no-op that leaves a toast
//!   trace in stderr so tests and scripting callers can observe the effect.
//! - `e` — open `~/.claude/settings.json` in `$EDITOR`.
//! - `r` — re-scan disk.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Stdout};
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::editor::open_in_editor;
use crate::data::settings::{HookRow, Settings};
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::hooks::{self, HookExecutionStats, HooksView};

pub fn run() -> anyhow::Result<()> {
    let (rows, execs) = collect();
    let fired_today = count_fired_today(&execs);

    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let result: anyhow::Result<()> = (|| {
        let theme = Theme::mocha();
        let mut selected: usize = 0;
        let mut should_quit = false;
        let rows = rows;
        let execs = execs;

        while !should_quit {
            terminal.draw(|f| {
                let view = HooksView {
                    rows: &rows,
                    executions: &execs,
                    selected,
                    fired_today,
                };
                hooks::render(f, f.area(), &view, &theme);
            })?;

            let Some(ev) = events::next()? else { continue };
            match ev {
                Event::Quit | Event::Escape | Event::Ctrl('c') => should_quit = true,
                Event::Key('q') => should_quit = true,
                Event::Up if !rows.is_empty() => {
                    selected = selected.saturating_sub(1);
                }
                Event::Down if !rows.is_empty() && selected + 1 < rows.len() => {
                    selected += 1;
                }
                Event::Key('e') => {
                    let path = dirs::home_dir()
                        .map(|h| h.join(".claude").join("settings.json"))
                        .unwrap_or_else(|| PathBuf::from("settings.json"));
                    let _ = open_in_editor(&path);
                }
                Event::Enter => {
                    // Print a note to stderr so pipeline consumers can see what
                    // session filter would be applied — we don't hijack the
                    // picker from here.
                    if let Some(row) = rows.get(selected) {
                        eprintln!(
                            "(hook → session filter: event={} matcher={})",
                            row.event,
                            row.matcher.as_deref().unwrap_or("*"),
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })();

    let _ = restore_terminal(&mut terminal);
    let _ = Instant::now(); // silence unused_import for Instant in release-opt
    result
}

// ── Data collection ───────────────────────────────────────────────────────

/// Read both settings.json and all session JSONLs and return the UI-ready
/// rows + exec stats. Pure over filesystem — no terminal setup here.
fn collect() -> (Vec<HookRow>, Vec<HookExecutionStats>) {
    let settings = Settings::load_all();
    let execs = scan_executions(dirs::home_dir().map(|h| h.join(".claude").join("projects")));
    (settings.hooks, execs)
}

/// Aggregate hook-execution stats from every JSONL under `projects_dir`.
///
/// Looks for `{"type":"attachment","attachment":{"hookEventName":…,…}}`
/// lines and tallies calls, mean duration, and the most recent non-zero exit
/// code per event.
pub fn scan_executions(projects_dir: Option<PathBuf>) -> Vec<HookExecutionStats> {
    let Some(root) = projects_dir else {
        return Vec::new();
    };
    scan_executions_in(&root)
}

/// Test-friendly entry point.
pub fn scan_executions_in(projects_dir: &Path) -> Vec<HookExecutionStats> {
    #[derive(Default)]
    struct Acc {
        calls: u64,
        total_ms: u64,
        samples: u64,
        last_failure: Option<i64>,
        last_ts: Option<DateTime<Utc>>,
    }
    let mut acc: HashMap<String, Acc> = HashMap::new();

    let Ok(projects) = std::fs::read_dir(projects_dir) else {
        return Vec::new();
    };
    for pe in projects.flatten() {
        let pdir = pe.path();
        if !pdir.is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&pdir) else {
            continue;
        };
        for fe in files.flatten() {
            let path = fe.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(file) = File::open(&path) else {
                continue;
            };
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                if !line.contains("hookEventName") {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                let att = value.get("attachment").or(Some(&value));
                let Some(att) = att else { continue };
                let Some(event) = att.get("hookEventName").and_then(|v| v.as_str()) else {
                    continue;
                };
                let entry = acc.entry(event.to_string()).or_default();
                entry.calls = entry.calls.saturating_add(1);
                if let Some(dur) = att.get("durationMs").and_then(|v| v.as_u64()) {
                    entry.total_ms = entry.total_ms.saturating_add(dur);
                    entry.samples = entry.samples.saturating_add(1);
                }
                if let Some(code) = att.get("exitCode").and_then(|v| v.as_i64()) {
                    if code != 0 {
                        entry.last_failure = Some(code);
                    }
                }
                let ts = value.get("timestamp").and_then(|v| v.as_str());
                if let Some(ts) = ts.and_then(parse_ts) {
                    entry.last_ts = Some(match entry.last_ts {
                        Some(e) => e.max(ts),
                        None => ts,
                    });
                }
            }
        }
    }

    let now = Utc::now();
    let mut out: Vec<_> = acc
        .into_iter()
        .map(|(event, a)| HookExecutionStats {
            event,
            calls: a.calls,
            mean_ms: a.total_ms.checked_div(a.samples).unwrap_or(0),
            last_failure: a.last_failure,
            last_relative: a
                .last_ts
                .map(|ts| relative_from(now, ts))
                .unwrap_or_default(),
        })
        .collect();
    // Biggest offenders first.
    out.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.event.cmp(&b.event)));
    out
}

fn count_fired_today(execs: &[HookExecutionStats]) -> u32 {
    // Our scan resets per-day granularity by only recording total calls, so
    // "fired today" is an approximation: sum of calls whose `last_relative`
    // starts with "just now", "<Nm ago>", or "<Nh ago>" (where N < 24).
    // That's rough but good enough for a glanceable caption.
    execs
        .iter()
        .filter(|e| is_within_day(&e.last_relative))
        .map(|e| e.calls as u32)
        .sum()
}

fn is_within_day(rel: &str) -> bool {
    if rel.is_empty() {
        return false;
    }
    if rel == "just now" {
        return true;
    }
    let trimmed = rel.trim_end_matches(" ago");
    let (num, unit) = trimmed.split_at(
        trimmed
            .find(|c: char| c.is_ascii_alphabetic())
            .unwrap_or(trimmed.len()),
    );
    let n: i64 = num.trim().parse().unwrap_or(0);
    matches!(unit, "s" | "m") || (unit == "h" && n < 24)
}

/// Format a UTC timestamp as "N{s,m,h,d} ago" relative to `now`.
pub fn relative_from(now: DateTime<Utc>, ts: DateTime<Utc>) -> String {
    let dur: ChronoDuration = now.signed_duration_since(ts);
    let secs = dur.num_seconds();
    if secs < 30 {
        return "just now".to_string();
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    let mins = dur.num_minutes();
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = dur.num_hours();
    if hours < 24 {
        return format!("{hours}h ago");
    }
    format!("{}d ago", dur.num_days())
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

// ── Terminal lifecycle (mirrors stats_cmd) ────────────────────────────────

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
    use std::fs;

    #[test]
    fn relative_labels() {
        let now = Utc::now();
        assert_eq!(relative_from(now, now), "just now");
        assert_eq!(
            relative_from(now, now - ChronoDuration::seconds(45)),
            "45s ago"
        );
        assert_eq!(
            relative_from(now, now - ChronoDuration::minutes(5)),
            "5m ago"
        );
        assert_eq!(relative_from(now, now - ChronoDuration::hours(3)), "3h ago");
        assert_eq!(relative_from(now, now - ChronoDuration::days(2)), "2d ago");
    }

    #[test]
    fn scan_aggregates_hook_events() {
        let tmp = tempfile::tempdir().unwrap();
        let pdir = tmp.path().join("-proj");
        fs::create_dir_all(&pdir).unwrap();
        let lines = [
            r#"{"type":"attachment","timestamp":"2026-04-16T10:00:00Z","attachment":{"hookEventName":"PreToolUse","durationMs":10,"exitCode":0}}"#,
            r#"{"type":"attachment","timestamp":"2026-04-16T10:01:00Z","attachment":{"hookEventName":"PreToolUse","durationMs":20,"exitCode":0}}"#,
            r#"{"type":"attachment","timestamp":"2026-04-16T11:00:00Z","attachment":{"hookEventName":"UserPromptSubmit","durationMs":5,"exitCode":1}}"#,
        ]
        .join("\n");
        fs::write(pdir.join("s.jsonl"), lines).unwrap();

        let out = scan_executions_in(tmp.path());
        assert_eq!(out.len(), 2);
        // Sorted by calls desc.
        assert_eq!(out[0].event, "PreToolUse");
        assert_eq!(out[0].calls, 2);
        assert_eq!(out[0].mean_ms, 15);
        assert!(out[0].last_failure.is_none());
        let ups = out.iter().find(|e| e.event == "UserPromptSubmit").unwrap();
        assert_eq!(ups.last_failure, Some(1));
    }

    #[test]
    fn missing_projects_dir_is_empty() {
        let out = scan_executions_in(Path::new("/nope"));
        assert!(out.is_empty());
    }

    #[test]
    fn is_within_day_logic() {
        assert!(is_within_day("just now"));
        assert!(is_within_day("42s ago"));
        assert!(is_within_day("5m ago"));
        assert!(is_within_day("18h ago"));
        assert!(!is_within_day("30h ago"));
        assert!(!is_within_day("2d ago"));
        assert!(!is_within_day(""));
    }
}
