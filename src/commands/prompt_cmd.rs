//! `claude-picker prompt` — single-line PS1-friendly summary of spend.
//!
//! Example output:
//!
//! ```text
//! claude: $5.20 today · $128.00 month · 67% budget
//! ```
//!
//! The command is designed for shell prompt integration, so it must be
//! fast. To avoid re-scanning every JSONL on every shell redraw we write a
//! one-line cache to `~/.claude-picker/prompt-cache.txt`. A cache hit
//! within 60 seconds is printed verbatim; older caches trigger a fresh
//! aggregation.
//!
//! Output formats:
//!
//! - `PS1`   (default) the human string above
//! - `JSON`  `{"today": 5.20, "month": 128.00, "budget_pct": 67}`
//!
//! A `--no-color` flag is accepted for terminals that can't render ANSI;
//! the default PS1 output is plain text so the flag is a no-op today but
//! kept so shells can wire it in without breaking later when we add
//! color.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{Datelike, Utc};

use crate::commands::pick::load_sessions_for;
use crate::data::budget::Budget;
use crate::data::project;

/// Cache TTL — how long a prompt rollup is considered fresh.
const CACHE_TTL: Duration = Duration::from_secs(60);

/// Output format for `claude-picker prompt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// Human one-liner embedded in PS1.
    #[default]
    Ps1,
    /// Structured JSON for consumers that want to compose themselves.
    Json,
    /// Three-column CSV: `today,month,budget_pct`. Budget column renders as
    /// an empty field when no monthly cap is configured so downstream
    /// parsers see a stable shape.
    Csv,
}

impl Format {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "PS1" | "ps1" => Some(Self::Ps1),
            "JSON" | "json" => Some(Self::Json),
            "CSV" | "csv" => Some(Self::Csv),
            _ => None,
        }
    }
}

/// Options surfaced from the CLI.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub format: Format,
    /// Reserved — today both formats emit plain text. Kept so shells that
    /// wire it in now won't break when we start using color.
    pub no_color: bool,
}

/// The per-shell rollup. Serialised to the cache file and from there into
/// the chosen output format.
#[derive(Debug, Clone, Copy, Default)]
struct Rollup {
    today_cost: f64,
    month_cost: f64,
    /// `Some(pct)` when a monthly cap is configured; `None` means the
    /// budget segment should be omitted from PS1 output.
    budget_pct: Option<u32>,
}

/// Public entry point.
pub fn run(opts: Options) -> anyhow::Result<()> {
    // Fast path — a fresh cache short-circuits everything.
    if let Some(cached) = read_cache_if_fresh(opts.format) {
        print_line(&cached);
        return Ok(());
    }

    let rollup = compute_rollup()?;
    let line = format_line(&rollup, opts.format);

    // Caching is a best-effort optimisation: if the write fails (read-only
    // home, full disk, …) we still return the computed value.
    let _ = write_cache(&line, opts.format);

    print_line(&line);
    Ok(())
}

fn print_line(line: &str) {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = writeln!(out, "{line}");
}

/// `~/.claude-picker/prompt-cache.txt`. Returns `None` on headless hosts.
fn cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude-picker").join("prompt-cache.txt"))
}

/// A per-format cache sidecar so switching `--format` between prompts
/// doesn't trample the other format's cached line.
fn cache_path_for(format: Format) -> Option<PathBuf> {
    let base = cache_path()?;
    let parent = base.parent()?;
    let name = match format {
        Format::Ps1 => "prompt-cache.ps1.txt",
        Format::Json => "prompt-cache.json.txt",
        Format::Csv => "prompt-cache.csv.txt",
    };
    Some(parent.join(name))
}

fn read_cache_if_fresh(format: Format) -> Option<String> {
    let path = cache_path_for(format)?;
    let meta = fs::metadata(&path).ok()?;
    let mtime = meta.modified().ok()?;
    let age = SystemTime::now().duration_since(mtime).ok()?;
    if age > CACHE_TTL {
        return None;
    }
    let raw = fs::read_to_string(&path).ok()?;
    // The cache file holds exactly one line — strip the trailing newline
    // so the caller doesn't print a blank line after it.
    Some(raw.trim_end_matches('\n').to_string())
}

fn write_cache(line: &str, format: Format) -> std::io::Result<()> {
    let Some(path) = cache_path_for(format) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, format!("{line}\n"))
}

/// Compute today-cost, month-to-date-cost, and the optional budget
/// percentage.
fn compute_rollup() -> anyhow::Result<Rollup> {
    let projects = project::discover_projects()?;
    let today = Utc::now().date_naive();
    let (year, month) = (today.year(), today.month());

    let mut today_cost = 0.0_f64;
    let mut month_cost = 0.0_f64;

    for p in &projects {
        let sessions = match load_sessions_for(p) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for s in sessions {
            let Some(ts) = s.last_timestamp else { continue };
            let d = ts.date_naive();
            if d == today {
                today_cost += s.total_cost_usd;
            }
            if d.year() == year && d.month() == month {
                month_cost += s.total_cost_usd;
            }
        }
    }

    let budget = Budget::load();
    let budget_pct = if budget.monthly_limit_usd > 0.0 {
        let raw = (month_cost / budget.monthly_limit_usd) * 100.0;
        Some(raw.round().clamp(0.0, 99_999.0) as u32)
    } else {
        None
    };

    Ok(Rollup {
        today_cost,
        month_cost,
        budget_pct,
    })
}

/// Turn a [`Rollup`] into the exact line we'll print (and cache).
fn format_line(r: &Rollup, format: Format) -> String {
    match format {
        Format::Ps1 => match r.budget_pct {
            Some(pct) => format!(
                "claude: ${:.2} today · ${:.2} month · {}% budget",
                r.today_cost, r.month_cost, pct,
            ),
            None => format!(
                "claude: ${:.2} today · ${:.2} month",
                r.today_cost, r.month_cost,
            ),
        },
        Format::Json => match r.budget_pct {
            Some(pct) => format!(
                "{{\"today\": {:.2}, \"month\": {:.2}, \"budget_pct\": {}}}",
                r.today_cost, r.month_cost, pct,
            ),
            None => format!(
                "{{\"today\": {:.2}, \"month\": {:.2}}}",
                r.today_cost, r.month_cost,
            ),
        },
        // Three-column CSV: `today,month,budget_pct`. Budget column renders
        // as an empty field when no cap is set so consumers can parse with a
        // fixed three-column schema. No header row — the shape is part of
        // the documented API.
        Format::Csv => match r.budget_pct {
            Some(pct) => format!("{:.2},{:.2},{}", r.today_cost, r.month_cost, pct),
            None => format!("{:.2},{:.2},", r.today_cost, r.month_cost),
        },
    }
}

/// Read-through of the cache using an explicit path — used in tests so we
/// don't touch `$HOME`.
#[cfg(test)]
fn read_cache_if_fresh_at(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let age = SystemTime::now().duration_since(mtime).ok()?;
    if age > CACHE_TTL {
        return None;
    }
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim_end_matches('\n').to_string())
}

// Silence dead_code lint when the test-only helper above is compiled but
// not used from any non-test module.
#[allow(dead_code)]
fn _reference_path_is_used(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_parse_accepts_both_cases() {
        assert_eq!(Format::parse("PS1"), Some(Format::Ps1));
        assert_eq!(Format::parse("ps1"), Some(Format::Ps1));
        assert_eq!(Format::parse("JSON"), Some(Format::Json));
        assert_eq!(Format::parse("json"), Some(Format::Json));
        assert_eq!(Format::parse("CSV"), Some(Format::Csv));
        assert_eq!(Format::parse("csv"), Some(Format::Csv));
        assert!(Format::parse("xml").is_none());
    }

    #[test]
    fn csv_line_has_three_columns_with_budget() {
        let r = Rollup {
            today_cost: 5.2,
            month_cost: 128.0,
            budget_pct: Some(67),
        };
        let line = format_line(&r, Format::Csv);
        assert_eq!(line, "5.20,128.00,67");
    }

    #[test]
    fn csv_line_leaves_budget_column_empty_when_unset() {
        let r = Rollup {
            today_cost: 5.2,
            month_cost: 128.0,
            budget_pct: None,
        };
        let line = format_line(&r, Format::Csv);
        assert_eq!(line, "5.20,128.00,");
        // Shape assertion — always three comma-separated fields.
        assert_eq!(line.split(',').count(), 3);
    }

    #[test]
    fn ps1_line_omits_budget_segment_when_none() {
        let r = Rollup {
            today_cost: 5.2,
            month_cost: 128.0,
            budget_pct: None,
        };
        let line = format_line(&r, Format::Ps1);
        assert!(line.contains("$5.20 today"));
        assert!(line.contains("$128.00 month"));
        assert!(!line.contains("budget"));
    }

    #[test]
    fn ps1_line_includes_budget_pct_when_set() {
        let r = Rollup {
            today_cost: 5.2,
            month_cost: 128.0,
            budget_pct: Some(67),
        };
        let line = format_line(&r, Format::Ps1);
        assert!(line.contains("67% budget"));
    }

    #[test]
    fn json_line_shape_without_budget() {
        let r = Rollup {
            today_cost: 5.2,
            month_cost: 128.0,
            budget_pct: None,
        };
        let line = format_line(&r, Format::Json);
        // Order matters for reproducibility — humans grepping this will
        // look for the `today` key first.
        assert!(line.starts_with("{\"today\": 5.20"));
        assert!(line.contains("\"month\": 128.00"));
        assert!(!line.contains("budget_pct"));
    }

    #[test]
    fn json_line_includes_budget_pct_when_set() {
        let r = Rollup {
            today_cost: 0.0,
            month_cost: 50.0,
            budget_pct: Some(25),
        };
        let line = format_line(&r, Format::Json);
        assert!(line.contains("\"budget_pct\": 25"));
    }

    #[test]
    fn cache_is_fresh_for_60_seconds() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("prompt-cache.txt");
        fs::write(&path, "claude: $0 today · $0 month\n").expect("write");
        let got = read_cache_if_fresh_at(&path).expect("fresh");
        assert!(got.starts_with("claude:"));
        assert!(!got.ends_with('\n'));
    }
}
