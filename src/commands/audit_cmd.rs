//! `claude-picker audit` — cost-optimization report.
//!
//! Walks every session on disk, runs the heuristics in
//! [`crate::data::cost_audit`], and either presents the results as a
//! scrollable Ratatui list (the default `tui` format) or emits them as
//! structured JSON / CSV for downstream scripting. Under the TUI, `Enter`
//! on any row resumes that session so the user can act on the suggestion
//! immediately.
//!
//! No mutation occurs here — we never rewrite user data. The audit is
//! strictly observational; the remediation belongs to the user.

use std::io::{self, Stdout, Write};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::data::cost_audit::{self, AuditFinding, FindingKind, Severity};
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::audit::{self, AuditState};
use crate::ui::help_overlay::{self, Screen as HelpScreen};

/// Output format for `claude-picker audit`.
///
/// `Tui` is the historical interactive dashboard (default). `Json` and
/// `Csv` skip the TUI entirely and dump the raw findings to stdout, keeping
/// the command scriptable from CI.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    #[default]
    Tui,
    Json,
    Csv,
}

impl Format {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "tui" => Some(Self::Tui),
            "json" => Some(Self::Json),
            "csv" => Some(Self::Csv),
            _ => None,
        }
    }
}

/// Options surfaced from the CLI layer.
#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    pub format: Format,
}

/// Backwards-compatible entry — keeps `audit_cmd::run()` callable without
/// knowing about the new format flag.
pub fn run() -> anyhow::Result<()> {
    run_with(Options::default())
}

/// Entry point for `claude-picker audit` with explicit options.
pub fn run_with(opts: Options) -> anyhow::Result<()> {
    let findings = cost_audit::run_audit()?;

    match opts.format {
        Format::Json => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            write_findings_json(&mut out, &findings)?;
            return Ok(());
        }
        Format::Csv => {
            let stdout = io::stdout();
            let mut out = stdout.lock();
            write_findings_csv(&mut out, &findings)?;
            return Ok(());
        }
        Format::Tui => {}
    }

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

// ── Structured-output writers ────────────────────────────────────────────

/// Per-finding row exposed to JSON consumers. Flattened from the nested
/// `AuditFinding → Vec<Finding>` shape so every line corresponds to one
/// actionable suggestion — which is how callers usually want to consume it.
#[derive(Debug, serde::Serialize)]
struct JsonFindingRow<'a> {
    session_id: &'a str,
    project: &'a str,
    session_label: &'a str,
    total_cost_usd: f64,
    model_summary: &'a str,
    kind: &'static str,
    severity: &'static str,
    message: &'a str,
    savings_usd: f64,
}

#[derive(Debug, serde::Serialize)]
struct JsonReport<'a> {
    total_savings_usd: f64,
    annual_run_rate_usd: f64,
    findings: Vec<JsonFindingRow<'a>>,
}

fn write_findings_json<W: Write>(
    out: &mut W,
    findings: &[AuditFinding],
) -> anyhow::Result<()> {
    let total: f64 = findings.iter().map(|af| af.estimated_savings_usd).sum();
    // The TUI already shows an "annual run-rate" projection computed as
    // `total × 12` (rough but matches the existing headline). Mirror that
    // here so the JSON and the screen agree.
    let annual = total * 12.0;
    let rows: Vec<JsonFindingRow<'_>> = findings
        .iter()
        .flat_map(|af| {
            af.findings.iter().map(move |f| JsonFindingRow {
                session_id: &af.session_id,
                project: &af.project_name,
                session_label: &af.session_label,
                total_cost_usd: af.total_cost_usd,
                model_summary: &af.model_summary,
                kind: kind_label(f.kind),
                severity: severity_label(f.severity),
                message: &f.message,
                savings_usd: f.savings_usd,
            })
        })
        .collect();
    let report = JsonReport {
        total_savings_usd: total,
        annual_run_rate_usd: annual,
        findings: rows,
    };
    let json = serde_json::to_string_pretty(&report)?;
    writeln!(out, "{json}")?;
    Ok(())
}

fn write_findings_csv<W: Write>(out: &mut W, findings: &[AuditFinding]) -> anyhow::Result<()> {
    writeln!(
        out,
        "session_id,project,kind,severity,savings_usd,total_cost_usd,model,message",
    )?;
    for af in findings {
        for f in &af.findings {
            writeln!(
                out,
                "{},{},{},{},{:.4},{:.4},{},{}",
                csv_escape(&af.session_id),
                csv_escape(&af.project_name),
                kind_label(f.kind),
                severity_label(f.severity),
                f.savings_usd,
                af.total_cost_usd,
                csv_escape(&af.model_summary),
                csv_escape(&f.message),
            )?;
        }
    }
    Ok(())
}

fn kind_label(k: FindingKind) -> &'static str {
    match k {
        FindingKind::ToolRatio => "ToolRatio",
        FindingKind::CacheEfficiency => "CacheEfficiency",
        FindingKind::ModelMismatch => "ModelMismatch",
    }
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
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
    use crate::data::cost_audit::{AuditFinding, Finding, FindingKind, Severity};
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
                kind: FindingKind::ToolRatio,
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

    #[test]
    fn format_parse_handles_tui_json_csv() {
        assert_eq!(Format::parse("tui"), Some(Format::Tui));
        assert_eq!(Format::parse("json"), Some(Format::Json));
        assert_eq!(Format::parse("csv"), Some(Format::Csv));
        assert!(Format::parse("yaml").is_none());
    }

    #[test]
    fn json_writer_carries_summary_and_rows() {
        let findings = vec![mk("abc", 36.40), mk("xyz", 27.60)];
        let mut buf = Vec::new();
        write_findings_json(&mut buf, &findings).expect("ok");
        let s = String::from_utf8(buf).expect("utf8");
        let v: serde_json::Value = serde_json::from_str(&s).expect("parse");
        let total = v["total_savings_usd"].as_f64().expect("total");
        assert!((total - 64.0).abs() < 1e-6);
        // annual_run_rate_usd is the documented 12× projection.
        let annual = v["annual_run_rate_usd"].as_f64().expect("annual");
        assert!((annual - 64.0 * 12.0).abs() < 1e-6);
        assert_eq!(v["findings"].as_array().expect("arr").len(), 2);
        assert_eq!(v["findings"][0]["kind"], "ToolRatio");
        assert_eq!(v["findings"][0]["session_id"], "abc");
    }

    #[test]
    fn csv_writer_has_one_row_per_finding() {
        let findings = vec![mk("abc", 1.0)];
        let mut buf = Vec::new();
        write_findings_csv(&mut buf, &findings).expect("ok");
        let s = String::from_utf8(buf).expect("utf8");
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 2, "header + one finding");
        assert!(lines[0].starts_with(
            "session_id,project,kind,severity,savings_usd,total_cost_usd,model,message",
        ));
        assert!(lines[1].starts_with("abc,p,ToolRatio,warn,"));
    }
}
