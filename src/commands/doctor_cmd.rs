//! `claude-picker doctor` — diagnostic scan of `~/.claude/projects/`.
//!
//! Non-interactive, plain-text output designed to be copy-pasted into
//! issues, blog posts, or pull-request bodies. No Ratatui.
//!
//! Reports:
//!
//! 1. Total sessions + total bytes of JSONL on disk.
//! 2. Top 5 most-expensive sessions by `total_cost_usd`.
//! 3. Top 5 largest JSONL files by raw byte size.
//! 4. Orphan session metadata — `~/.claude/sessions/<sid>.json` entries
//!    whose `sessionId` does not match any `.jsonl` file under
//!    `~/.claude/projects/`.
//! 5. Empty / stub sessions — `.jsonl` files that
//!    [`crate::data::session::load_session_from_jsonl`] returns `None`
//!    for (fewer than 2 user+assistant messages, or SDK-only).
//!
//! The `--cleanup` flag (opt-in, additionally gated by `--yes`) deletes
//! the orphan metadata files and the empty stub JSONLs after listing
//! them. The default behavior is strictly read-only.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::data::project;
use crate::data::session::load_session_from_jsonl;

/// Output format for `claude-picker doctor`.
///
/// `Plain` is the historical report — a human-readable text rollup. `Json`
/// and `Csv` emit the same underlying report as structured data so CI jobs
/// can parse `doctor` output without scraping English. Structured formats
/// skip the cleanup phase entirely to keep their output deterministic.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    #[default]
    Plain,
    Json,
    Csv,
}

impl Format {
    /// Parse `--format` argument. Unknown values are surfaced as `None` so
    /// the CLI layer can emit a user-facing error.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "plain" | "text" => Some(Self::Plain),
            "json" => Some(Self::Json),
            "csv" => Some(Self::Csv),
            _ => None,
        }
    }
}

/// Options controlling the doctor run. Kept in a struct so the CLI layer can
/// pass flags in without growing the public signature every time we add a
/// toggle.
#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    /// When `true`, the doctor deletes orphan metadata + empty stub JSONLs
    /// after listing them. Off by default.
    pub cleanup: bool,
    /// Extra confirmation gate for `cleanup`. Required — without it the
    /// cleanup phase is a dry-run that just prints what it *would* delete.
    pub yes: bool,
    /// Output format — plain text (default), JSON, or CSV. Structured
    /// formats skip cleanup to keep their payload deterministic.
    pub format: Format,
}

/// Public entry point — `claude-picker doctor`.
pub fn run(opts: Options) -> anyhow::Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects_dir = home.join(".claude").join("projects");
    let sessions_meta_dir = home.join(".claude").join("sessions");

    let report = build_report(&projects_dir, &sessions_meta_dir)?;

    let stdout = io::stdout();
    let mut out = stdout.lock();
    match opts.format {
        Format::Plain => {
            print_report(&report);
            if opts.cleanup {
                run_cleanup(&report, opts.yes)?;
            }
        }
        Format::Json => {
            // Cleanup deliberately skipped under structured output — the
            // JSON shape is the report, not a side-effect log.
            write_report_json(&mut out, &report)?;
        }
        Format::Csv => {
            write_report_csv(&mut out, &report)?;
        }
    }

    Ok(())
}

/// Emit the report as pretty-printed JSON. Consumers pipe into `jq` or
/// their preferred tooling.
fn write_report_json<W: Write>(out: &mut W, r: &Report) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(r)?;
    writeln!(out, "{json}")?;
    Ok(())
}

/// Emit the report as a single CSV stream with a `section` discriminator
/// column so rows from different sections coexist without needing a
/// multi-table format. The header is constant; callers can filter by the
/// first column to recover the logical section.
fn write_report_csv<W: Write>(out: &mut W, r: &Report) -> anyhow::Result<()> {
    writeln!(
        out,
        "section,key,project,session_id,cost_usd,tokens,messages,bytes,path",
    )?;
    writeln!(
        out,
        "overview,total_sessions,,,,,{},,",
        r.total_sessions,
    )?;
    writeln!(
        out,
        "overview,total_bytes,,,,,,{},",
        r.total_bytes,
    )?;
    for row in &r.top_cost {
        writeln!(
            out,
            "top_cost,{},{},{},{:.4},{},{},,",
            csv_escape(&row.title),
            csv_escape(&row.project),
            csv_escape(&row.session_id),
            row.cost_usd,
            row.tokens,
            row.messages,
        )?;
    }
    for row in &r.top_size {
        writeln!(
            out,
            "top_size,,{},{},,,,{},{}",
            csv_escape(&row.project),
            csv_escape(&row.session_id),
            row.bytes,
            csv_escape(&row.path.display().to_string()),
        )?;
    }
    for path in &r.orphans {
        writeln!(
            out,
            "orphan,,,,,,,,{}",
            csv_escape(&path.display().to_string()),
        )?;
    }
    for path in &r.empty_stubs {
        writeln!(
            out,
            "empty_stub,,,,,,,,{}",
            csv_escape(&path.display().to_string()),
        )?;
    }
    Ok(())
}

/// RFC 4180-style escape. Kept local to this module so each subcommand can
/// pick its own column shape without sharing a helper.
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

/// A single "top session" row.
#[derive(Debug, Clone, Serialize)]
struct SessionRow {
    project: String,
    title: String,
    session_id: String,
    cost_usd: f64,
    tokens: u64,
    messages: u32,
}

/// A single "largest file" row.
#[derive(Debug, Clone, Serialize)]
struct FileSizeRow {
    project: String,
    session_id: String,
    bytes: u64,
    #[serde(serialize_with = "serialize_path")]
    path: PathBuf,
}

fn serialize_path<S: serde::Serializer>(p: &Path, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&p.display().to_string())
}

/// Aggregated doctor output. Held as data so tests can inspect and so the
/// cleanup phase can re-use the discovered paths without re-scanning.
#[derive(Debug, Default, Serialize)]
struct Report {
    total_sessions: u64,
    total_bytes: u64,
    top_cost: Vec<SessionRow>,
    top_size: Vec<FileSizeRow>,
    #[serde(serialize_with = "serialize_path_vec")]
    orphans: Vec<PathBuf>,
    #[serde(serialize_with = "serialize_path_vec")]
    empty_stubs: Vec<PathBuf>,
}

fn serialize_path_vec<S: serde::Serializer>(
    v: &[PathBuf],
    s: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = s.serialize_seq(Some(v.len()))?;
    for p in v {
        seq.serialize_element(&p.display().to_string())?;
    }
    seq.end()
}

fn build_report(projects_dir: &Path, sessions_meta_dir: &Path) -> anyhow::Result<Report> {
    let mut total_sessions: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut cost_rows: Vec<SessionRow> = Vec::new();
    let mut size_rows: Vec<FileSizeRow> = Vec::new();
    let mut empty_stubs: Vec<PathBuf> = Vec::new();

    // Session IDs we actually saw on disk — used to detect orphans below.
    let mut seen_session_ids: HashSet<String> = HashSet::new();

    if projects_dir.is_dir() {
        let projects = project::discover_projects_in(projects_dir, sessions_meta_dir)?;
        for p in &projects {
            let dir = projects_dir.join(&p.encoded_dir);
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }

                total_sessions = total_sessions.saturating_add(1);
                let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                total_bytes = total_bytes.saturating_add(bytes);

                let sid = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string();
                if !sid.is_empty() {
                    seen_session_ids.insert(sid.clone());
                }

                size_rows.push(FileSizeRow {
                    project: p.name.clone(),
                    session_id: sid.clone(),
                    bytes,
                    path: path.clone(),
                });

                match load_session_from_jsonl(&path, p.path.clone()) {
                    Ok(Some(s)) => {
                        cost_rows.push(SessionRow {
                            project: p.name.clone(),
                            title: s.display_label().to_string(),
                            session_id: s.id,
                            cost_usd: s.total_cost_usd,
                            tokens: s.tokens.total(),
                            messages: s.message_count,
                        });
                    }
                    Ok(None) => {
                        // Sub-2-message stub or SDK-only — cleanup candidate.
                        empty_stubs.push(path.clone());
                    }
                    Err(_) => {
                        // Unreadable file — leave it alone; the loader
                        // already logged a stderr warning.
                    }
                }
            }
        }
    }

    // Top 5 by cost (desc).
    cost_rows.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    cost_rows.truncate(5);

    // Top 5 by size (desc).
    size_rows.sort_by_key(|r| std::cmp::Reverse(r.bytes));
    size_rows.truncate(5);

    // Orphans: metadata whose `sessionId` doesn't point to any JSONL we
    // found above.
    let orphans = find_orphans(sessions_meta_dir, &seen_session_ids);

    Ok(Report {
        total_sessions,
        total_bytes,
        top_cost: cost_rows,
        top_size: size_rows,
        orphans,
        empty_stubs,
    })
}

/// Minimal shape — we only need the `sessionId` field to decide "is there a
/// matching JSONL?".
#[derive(Deserialize)]
struct SessionMetaLite {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

fn find_orphans(sessions_meta_dir: &Path, seen_ids: &HashSet<String>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(sessions_meta_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<SessionMetaLite>(&raw) else {
            continue;
        };
        // Fall back to the filename stem if the JSON lacks a sessionId.
        let sid = meta.session_id.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string()
        });
        if sid.is_empty() {
            continue;
        }
        if !seen_ids.contains(&sid) {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Render the report to stdout in plain text. One emoji per section max.
fn print_report(r: &Report) {
    println!("claude-picker doctor");
    println!();

    println!("# Overview");
    println!("  sessions on disk : {}", r.total_sessions);
    println!(
        "  jsonl bytes      : {} ({})",
        r.total_bytes,
        human_bytes(r.total_bytes)
    );
    println!();

    println!("# Top 5 most-expensive sessions");
    if r.top_cost.is_empty() {
        println!("  (none)");
    } else {
        for row in &r.top_cost {
            println!(
                "  {}/{}  ${:.2}  {} tok  {} msgs  [{}]",
                row.project,
                truncate(&row.title, 48),
                row.cost_usd,
                row.tokens,
                row.messages,
                row.session_id,
            );
        }
    }
    println!();

    println!("# Top 5 largest JSONL files");
    if r.top_size.is_empty() {
        println!("  (none)");
    } else {
        for row in &r.top_size {
            println!(
                "  {}  {}  [{}]  {}",
                human_bytes(row.bytes),
                row.project,
                row.session_id,
                row.path.display(),
            );
        }
    }
    println!();

    println!("# Orphan session metadata ({} found)", r.orphans.len());
    if r.orphans.is_empty() {
        println!("  (none)");
    } else {
        for path in &r.orphans {
            println!("  {}", path.display());
        }
    }
    println!();

    println!("# Empty / stub sessions ({} found)", r.empty_stubs.len());
    if r.empty_stubs.is_empty() {
        println!("  (none)");
    } else {
        for path in &r.empty_stubs {
            println!("  {}", path.display());
        }
    }
    println!();
}

/// Delete orphan metadata + empty stubs, or print what would be deleted.
fn run_cleanup(r: &Report, confirmed: bool) -> anyhow::Result<()> {
    println!("# Cleanup");
    if !confirmed {
        println!("  dry-run: pass --yes to actually delete the files below.");
        for path in r.orphans.iter().chain(r.empty_stubs.iter()) {
            println!("  would delete  {}", path.display());
        }
        println!();
        return Ok(());
    }

    let mut deleted: u64 = 0;
    let mut failed: u64 = 0;
    for path in r.orphans.iter().chain(r.empty_stubs.iter()) {
        match fs::remove_file(path) {
            Ok(()) => {
                deleted += 1;
                println!("  deleted  {}", path.display());
            }
            Err(e) => {
                failed += 1;
                eprintln!("  error    {} ({e})", path.display());
            }
        }
    }
    println!();
    println!("  deleted {deleted}, failed {failed}");
    Ok(())
}

/// Byte pretty-printer — KiB/MiB/GiB, up to two decimals.
fn human_bytes(n: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    let f = n as f64;
    if f >= GIB {
        format!("{:.2} GiB", f / GIB)
    } else if f >= MIB {
        format!("{:.2} MiB", f / MIB)
    } else if f >= KIB {
        format!("{:.2} KiB", f / KIB)
    } else {
        format!("{n} B")
    }
}

/// Char-safe truncate with ellipsis — keeps the display column well-bounded.
fn truncate(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let mut out = String::with_capacity(max_chars.saturating_add(1));
    for (i, ch) in s.chars().enumerate() {
        if i + 1 >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_bytes_renders_sensible_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.00 KiB");
        assert!(human_bytes(5 * 1024 * 1024).contains("MiB"));
    }

    #[test]
    fn format_parse_roundtrip() {
        assert_eq!(Format::parse("plain"), Some(Format::Plain));
        assert_eq!(Format::parse("text"), Some(Format::Plain));
        assert_eq!(Format::parse("json"), Some(Format::Json));
        assert_eq!(Format::parse("csv"), Some(Format::Csv));
        assert!(Format::parse("yaml").is_none());
    }

    #[test]
    fn json_writer_emits_parseable_payload() {
        let r = Report {
            total_sessions: 2,
            total_bytes: 1024,
            top_cost: vec![SessionRow {
                project: "api".into(),
                title: "t".into(),
                session_id: "abc".into(),
                cost_usd: 1.5,
                tokens: 100,
                messages: 4,
            }],
            top_size: Vec::new(),
            orphans: Vec::new(),
            empty_stubs: Vec::new(),
        };
        let mut buf = Vec::new();
        write_report_json(&mut buf, &r).expect("ok");
        let s = String::from_utf8(buf).expect("utf8");
        // Round-trip through serde_json — if this parses, the shape is good.
        let v: serde_json::Value = serde_json::from_str(&s).expect("parse");
        assert_eq!(v["total_sessions"], 2);
        assert_eq!(v["top_cost"][0]["session_id"], "abc");
    }

    #[test]
    fn csv_writer_has_stable_header_and_section_column() {
        let r = Report {
            total_sessions: 1,
            total_bytes: 42,
            top_cost: Vec::new(),
            top_size: Vec::new(),
            orphans: vec![PathBuf::from("/tmp/ghost.json")],
            empty_stubs: Vec::new(),
        };
        let mut buf = Vec::new();
        write_report_csv(&mut buf, &r).expect("ok");
        let s = String::from_utf8(buf).expect("utf8");
        let header = s.lines().next().expect("header");
        assert!(header.starts_with(
            "section,key,project,session_id,cost_usd,tokens,messages,bytes,path",
        ));
        assert!(s.contains("overview,total_sessions"));
        assert!(s.contains("orphan,,,,,,,,/tmp/ghost.json"));
    }

    #[test]
    fn csv_escape_handles_delimiters_and_newlines() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_escape("multi\nline"), "\"multi\nline\"");
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("hello", 10), "hello");
        let out = truncate("hello world and a bit", 10);
        assert!(out.chars().count() <= 10);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn build_report_counts_sessions_and_sums_bytes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects = tmp.path().join("projects");
        let sessions = tmp.path().join("sessions");
        let encoded = projects.join("-Users-me-foo");
        fs::create_dir_all(&encoded).expect("mkdir");
        fs::write(
            encoded.join("aaa.jsonl"),
            concat!(
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"hi please help with stuff\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":10,\"output_tokens\":20}}}\n",
            ),
        )
        .expect("write");
        // Stub — single user, no assistant.
        fs::write(
            encoded.join("stub.jsonl"),
            "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"hi\"}}\n",
        )
        .expect("write");

        let r = build_report(&projects, &sessions).expect("ok");
        assert_eq!(r.total_sessions, 2);
        assert!(r.total_bytes > 0);
        assert_eq!(r.top_cost.len(), 1);
        assert_eq!(r.empty_stubs.len(), 1);
    }

    #[test]
    fn orphans_detects_metadata_without_jsonl() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects = tmp.path().join("projects");
        let sessions = tmp.path().join("sessions");
        fs::create_dir_all(&projects).expect("mkdir");
        fs::create_dir_all(&sessions).expect("mkdir");
        // Write a metadata file with a sessionId that has no matching jsonl.
        fs::write(
            sessions.join("ghost.json"),
            r#"{"sessionId":"ghost","cwd":"/tmp/gone"}"#,
        )
        .expect("write");
        let r = build_report(&projects, &sessions).expect("ok");
        assert_eq!(r.orphans.len(), 1);
        assert!(r.orphans[0].ends_with("ghost.json"));
    }

    #[test]
    fn orphans_ignores_metadata_backed_by_a_jsonl() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects = tmp.path().join("projects");
        let sessions = tmp.path().join("sessions");
        let encoded = projects.join("-Users-me-bar");
        fs::create_dir_all(&encoded).expect("mkdir");
        fs::create_dir_all(&sessions).expect("mkdir");
        fs::write(
            encoded.join("known.jsonl"),
            concat!(
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"hi please help\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":10,\"output_tokens\":20}}}\n",
            ),
        )
        .expect("write jsonl");
        fs::write(
            sessions.join("known.json"),
            r#"{"sessionId":"known","cwd":"/tmp/bar"}"#,
        )
        .expect("write meta");
        let r = build_report(&projects, &sessions).expect("ok");
        assert_eq!(r.orphans.len(), 0);
    }
}
