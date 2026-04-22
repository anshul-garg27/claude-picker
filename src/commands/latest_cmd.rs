//! `claude-picker latest` — print the most-recent session id(s).
//!
//! Non-interactive, scriptable. The intended use is shell substitution:
//!
//! ```bash
//! claude --resume "$(claude-picker latest --project my-api)"
//! ```
//!
//! Flags:
//!
//! - `--project NAME` filter to a single project by basename
//! - `--count N` print up to N latest IDs (default 1)
//! - `--since 7d` only include sessions whose `last_timestamp` is within
//!   the last N days/hours/minutes
//! - `--format id|json` `id` prints one id per line (default); `json`
//!   emits a structured array

use std::io::Write;

use chrono::{DateTime, Duration as ChronoDuration, Utc};

use crate::commands::pick;
use crate::data::{project, Session};

/// Output shape. Kept as a small enum rather than a bool so adding a third
/// format later is a one-line change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// One session id per line — the default, pipe-friendly.
    #[default]
    Id,
    /// Structured JSON array — one object per session.
    Json,
    /// RFC 4180 CSV — one row per session with the same columns the JSON
    /// objects expose. Designed to drop straight into a spreadsheet.
    Csv,
}

impl Format {
    /// Parse the `--format` argument value. Unknown values are an error at
    /// the CLI layer, so here we just default to `Id` on a miss.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "id" => Some(Self::Id),
            "json" => Some(Self::Json),
            "csv" => Some(Self::Csv),
            _ => None,
        }
    }
}

/// Options passed down from the CLI layer. Public so `main.rs` can build
/// it.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub project: Option<String>,
    pub count: usize,
    pub since: Option<ChronoDuration>,
    pub format: Format,
}

/// Parse `"7d"`, `"12h"`, `"30m"` into a `chrono::Duration`. Returns
/// `None` for malformed input; we lift that into a CLI error upstream.
///
/// Only `d`, `h`, and `m` suffixes are supported; the numeric prefix must
/// be a non-negative integer.
pub fn parse_since(raw: &str) -> Option<ChronoDuration> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let (num, unit) = s.split_at(s.len().saturating_sub(1));
    let amount: i64 = num.parse().ok()?;
    if amount < 0 {
        return None;
    }
    match unit {
        "d" => Some(ChronoDuration::days(amount)),
        "h" => Some(ChronoDuration::hours(amount)),
        "m" => Some(ChronoDuration::minutes(amount)),
        _ => None,
    }
}

/// Entry point. Writes to stdout; nothing else.
pub fn run(opts: Options) -> anyhow::Result<()> {
    let sessions = collect_sessions(&opts)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_output(&mut out, &sessions, opts.format)?;
    Ok(())
}

fn collect_sessions(opts: &Options) -> anyhow::Result<Vec<Session>> {
    let projects = project::discover_projects()?;

    let cutoff: Option<DateTime<Utc>> = opts.since.map(|d| Utc::now() - d);

    let mut all: Vec<Session> = Vec::new();
    for p in &projects {
        if let Some(filter) = &opts.project {
            if &p.name != filter {
                continue;
            }
        }
        let sessions = pick::load_sessions_for(p)?;
        for s in sessions {
            if let Some(cutoff) = cutoff {
                match s.last_timestamp {
                    Some(ts) if ts >= cutoff => {}
                    _ => continue,
                }
            }
            all.push(s);
        }
    }

    all.sort_by_key(|s| std::cmp::Reverse(s.last_timestamp));
    let limit = opts.count.max(1);
    all.truncate(limit);
    Ok(all)
}

/// Render `sessions` in the requested format into `out`. Split from
/// [`run`] so tests can assert on the output buffer directly.
fn write_output<W: Write>(out: &mut W, sessions: &[Session], format: Format) -> std::io::Result<()> {
    match format {
        Format::Id => {
            for s in sessions {
                writeln!(out, "{}", s.id)?;
            }
        }
        Format::Json => {
            writeln!(out, "[")?;
            for (i, s) in sessions.iter().enumerate() {
                let project_name = s
                    .project_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                let title = json_escape(s.display_label());
                let project_esc = json_escape(project_name);
                let last_ts = s
                    .last_timestamp
                    .map(|ts| format!("\"{}\"", ts.to_rfc3339()))
                    .unwrap_or_else(|| "null".to_string());
                let comma = if i + 1 == sessions.len() { "" } else { "," };
                writeln!(
                    out,
                    "  {{\"session_id\":\"{}\",\"project\":\"{}\",\"title\":\"{}\",\"last_timestamp\":{},\"cost_usd\":{:.4},\"messages\":{}}}{}",
                    s.id, project_esc, title, last_ts, s.total_cost_usd, s.message_count, comma,
                )?;
            }
            writeln!(out, "]")?;
        }
        Format::Csv => {
            // Header row — columns mirror the JSON object keys so a consumer
            // can cheaply swap formats without rewriting their parser.
            writeln!(
                out,
                "session_id,project,title,last_timestamp,cost_usd,messages",
            )?;
            for s in sessions {
                let project_name = s
                    .project_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                let last_ts = s
                    .last_timestamp
                    .map(|ts| ts.to_rfc3339())
                    .unwrap_or_default();
                writeln!(
                    out,
                    "{},{},{},{},{:.4},{}",
                    csv_escape(&s.id),
                    csv_escape(project_name),
                    csv_escape(s.display_label()),
                    csv_escape(&last_ts),
                    s.total_cost_usd,
                    s.message_count,
                )?;
            }
        }
    }
    Ok(())
}

/// RFC 4180-style escaping — quote any field that contains `"`, `,`, or a
/// newline. Numeric columns never need escaping so we skip them at the call
/// site.
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

/// Minimal JSON string escaper for titles/project names. Covers the
/// characters that can break a naive `"{s}"` interpolation; we are not
/// trying to be a general-purpose encoder.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::pricing::TokenCounts;
    use crate::data::session::SessionKind;
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn mk_session(id: &str, ts: DateTime<Utc>) -> Session {
        Session {
            id: id.to_string(),
            project_dir: PathBuf::from("/tmp/myproj"),
            name: None,
            auto_name: Some("auto".to_string()),
            last_prompt: None,
            message_count: 4,
            tokens: TokenCounts::default(),
            total_cost_usd: 0.5,
            model_summary: "claude-opus-4-7".to_string(),
            first_timestamp: Some(ts),
            last_timestamp: Some(ts),
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
            turn_durations: Vec::new(),
        }
    }

    #[test]
    fn parse_since_handles_common_suffixes() {
        assert_eq!(parse_since("7d"), Some(ChronoDuration::days(7)));
        assert_eq!(parse_since("12h"), Some(ChronoDuration::hours(12)));
        assert_eq!(parse_since("30m"), Some(ChronoDuration::minutes(30)));
    }

    #[test]
    fn parse_since_rejects_nonsense() {
        assert!(parse_since("").is_none());
        assert!(parse_since("abc").is_none());
        assert!(parse_since("7x").is_none());
        assert!(parse_since("-5d").is_none());
    }

    #[test]
    fn format_parse_roundtrip() {
        assert_eq!(Format::parse("id"), Some(Format::Id));
        assert_eq!(Format::parse("json"), Some(Format::Json));
        assert_eq!(Format::parse("csv"), Some(Format::Csv));
        assert!(Format::parse("yaml").is_none());
    }

    #[test]
    fn write_output_csv_has_header_and_rows() {
        let t = Utc.with_ymd_and_hms(2026, 4, 20, 12, 0, 0).unwrap();
        let sessions = vec![mk_session("aaa", t)];
        let mut buf: Vec<u8> = Vec::new();
        write_output(&mut buf, &sessions, Format::Csv).expect("ok");
        let s = String::from_utf8(buf).expect("utf8");
        // First line is always the header.
        assert!(s.starts_with(
            "session_id,project,title,last_timestamp,cost_usd,messages",
        ));
        // Row must carry the session id and its ISO-8601 timestamp.
        assert!(s.contains(",aaa,") || s.contains("\naaa,"));
        assert!(s.contains("2026-04-20T12:00:00+00:00"));
    }

    #[test]
    fn csv_escape_handles_commas_and_quotes() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_escape("a\nb"), "\"a\nb\"");
    }

    #[test]
    fn write_output_id_one_per_line() {
        let t = Utc.with_ymd_and_hms(2026, 4, 20, 12, 0, 0).unwrap();
        let sessions = vec![
            mk_session("aaa", t),
            mk_session("bbb", t - ChronoDuration::hours(1)),
        ];
        let mut buf: Vec<u8> = Vec::new();
        write_output(&mut buf, &sessions, Format::Id).expect("ok");
        let s = String::from_utf8(buf).expect("utf8");
        assert_eq!(s, "aaa\nbbb\n");
    }

    #[test]
    fn write_output_json_is_structured() {
        let t = Utc.with_ymd_and_hms(2026, 4, 20, 12, 0, 0).unwrap();
        let sessions = vec![mk_session("aaa", t)];
        let mut buf: Vec<u8> = Vec::new();
        write_output(&mut buf, &sessions, Format::Json).expect("ok");
        let s = String::from_utf8(buf).expect("utf8");
        assert!(s.contains("\"session_id\":\"aaa\""));
        assert!(s.contains("\"last_timestamp\":\"2026-04-20T12:00:00+00:00\""));
        assert!(s.trim_start().starts_with('['));
        assert!(s.trim_end().ends_with(']'));
    }

    #[test]
    fn json_escape_handles_quotes_and_newlines() {
        assert_eq!(json_escape("a\"b"), "a\\\"b");
        assert_eq!(json_escape("a\nb"), "a\\nb");
        assert_eq!(json_escape("a\\b"), "a\\\\b");
    }
}
