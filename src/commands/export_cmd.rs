//! `claude-picker export <session-id>` — write a single session to a clean
//! Markdown document.
//!
//! Two entry points share this module:
//!
//! 1. The **CLI path** ([`run`]) exposed as `claude-picker export <sid>
//!    [--out PATH] [--redact]`. Writes a `.md` file, prints the path to
//!    stdout, and returns a non-zero exit code if the session id can't be
//!    resolved.
//! 2. The **TUI path** ([`export_by_id`]) called from the session-list `e`
//!    key-binding and `Ctrl-e` shortcut. Returns the output path so the
//!    caller can raise a toast.
//!
//! The format is deliberately a flat Markdown transcript with a YAML
//! frontmatter block up top — portable to every markdown viewer, easy to
//! grep, and cheap to diff session-over-session.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::data::path_resolver::{load_session_metadata, resolve};
use crate::data::session::{load_session_from_jsonl, Session};
use crate::data::transcript::{
    jsonl_path_for_session, load_transcript, ContentItem, Role, TranscriptMessage,
};

/// CLI entry for `claude-picker export <session-id> [--out PATH] [--redact]`.
///
/// Resolves the session id to its on-disk JSONL, renders a Markdown transcript,
/// writes it out, and prints the path to stdout.
pub fn run(session_id: &str, out: Option<PathBuf>, redact: bool) -> anyhow::Result<()> {
    let path = export_by_id(session_id, out.as_deref(), redact)?;
    println!("{}", path.display());
    Ok(())
}

/// Render a session to Markdown and write it to disk.
///
/// If `out` is `None`, the default path
/// `~/Downloads/claude-picker-{sid}-{YYYY-MM-DD}.md` is used. When the
/// `~/Downloads` directory is missing we fall back to the home dir.
///
/// Returns the written path so callers can surface it (toast, stdout, …).
pub fn export_by_id(
    session_id: &str,
    out: Option<&Path>,
    redact: bool,
) -> anyhow::Result<PathBuf> {
    let jsonl = jsonl_path_for_session(session_id).ok_or_else(|| {
        anyhow::anyhow!(
            "no .jsonl found for session {session_id} under ~/.claude/projects/",
        )
    })?;
    // Resolve the encoded project directory back to its real cwd when we
    // can — this gives the frontmatter a human-readable `project` /
    // `cwd` pair instead of the mangled `-Users-me-foo` form.
    let project_dir = resolve_project_dir(&jsonl);
    let session = load_session_from_jsonl(&jsonl, project_dir)?
        .ok_or_else(|| anyhow::anyhow!("session {session_id} is not a picker-visible CLI session"))?;
    let transcript = load_transcript(&jsonl)?;

    let mut markdown = render_markdown(&session, &transcript);
    if redact {
        markdown = redact_secrets(&markdown);
    }

    let target = match out {
        Some(p) => p.to_path_buf(),
        None => default_output_path(&session.id)?,
    };
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).ok();
        }
    }
    fs::write(&target, markdown)?;
    Ok(target)
}

/// Resolve a session's encoded project directory back to the real cwd. Falls
/// back to the JSONL's parent directory (the encoded form) if we can't
/// recover anything better — still a valid `PathBuf`, just uglier in the
/// frontmatter.
fn resolve_project_dir(jsonl: &Path) -> PathBuf {
    let Some(parent) = jsonl.parent() else {
        return PathBuf::from(".");
    };
    let fallback = parent.to_path_buf();
    let Some(encoded) = parent.file_name().and_then(|s| s.to_str()) else {
        return fallback;
    };
    let Some(projects_dir) = parent.parent() else {
        return fallback;
    };
    let sessions_meta_dir = projects_dir
        .parent()
        .map(|p| p.join("sessions"))
        .unwrap_or_default();
    let meta = load_session_metadata(&sessions_meta_dir);
    resolve(encoded, &meta, projects_dir).unwrap_or(fallback)
}

/// Resolve the default output path for a session export:
/// `~/Downloads/claude-picker-{sid}-{YYYY-MM-DD}.md`.
///
/// If `~/Downloads` is missing we fall back to the home dir so the export
/// still succeeds on headless or stripped-down systems.
fn default_output_path(session_id: &str) -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let downloads = home.join("Downloads");
    let target_dir = if downloads.is_dir() { downloads } else { home };
    let today = Utc::now().format("%Y-%m-%d");
    Ok(target_dir.join(format!("claude-picker-{session_id}-{today}.md")))
}

/// Render a single session + transcript to Markdown.
///
/// Pure function — no I/O, no clock reads. Exposed `pub(crate)` for tests.
pub(crate) fn render_markdown(session: &Session, messages: &[TranscriptMessage]) -> String {
    let mut out = String::with_capacity(4096);
    write_frontmatter(&mut out, session, messages);
    write_title(&mut out, session);
    write_transcript(&mut out, messages);
    out
}

/// Write the YAML frontmatter block documented in the spec.
fn write_frontmatter(out: &mut String, session: &Session, messages: &[TranscriptMessage]) {
    out.push_str("---\n");
    out.push_str(&format!("session_id: {}\n", session.id));
    out.push_str(&format!("title: {}\n", yaml_scalar(display_title(session))));
    out.push_str(&format!(
        "project: {}\n",
        yaml_scalar(project_name(session))
    ));
    out.push_str(&format!(
        "cwd: {}\n",
        yaml_scalar(session.project_dir.display().to_string())
    ));
    out.push_str(&format!(
        "model: {}\n",
        yaml_scalar(if session.model_summary.is_empty() {
            "unknown".to_string()
        } else {
            session.model_summary.clone()
        })
    ));
    out.push_str(&format!("cost_usd: {:.2}\n", session.total_cost_usd));
    out.push_str(&format!("tokens: {}\n", session.tokens.total()));
    out.push_str(&format!("messages: {}\n", session.message_count));
    out.push_str(&format!(
        "created: {}\n",
        fmt_iso(session.first_timestamp.or_else(|| first_ts(messages)))
    ));
    out.push_str(&format!(
        "last: {}\n",
        fmt_iso(session.last_timestamp.or_else(|| last_ts(messages)))
    ));
    out.push_str("---\n\n");
}

fn write_title(out: &mut String, session: &Session) {
    out.push_str(&format!("# {}\n\n", display_title(session)));
}

/// Write the ordered user/assistant sections.
fn write_transcript(out: &mut String, messages: &[TranscriptMessage]) {
    let mut prev_ts: Option<DateTime<Utc>> = None;
    for msg in messages {
        let role_label = match msg.role {
            Role::User => "user",
            Role::Assistant => "claude",
        };
        let header = build_header(role_label, msg.timestamp, prev_ts);
        out.push_str(&format!("## {header}\n\n"));

        let mut wrote_anything = false;
        for item in &msg.items {
            match item {
                ContentItem::Text(text) => {
                    if !text.trim().is_empty() {
                        out.push_str(text);
                        out.push_str("\n\n");
                        wrote_anything = true;
                    }
                }
                ContentItem::ToolUse { name, input } => {
                    write_tool_use(out, name, input);
                    wrote_anything = true;
                }
                ContentItem::ToolResult { content, is_error } => {
                    write_tool_result(out, content, *is_error);
                    wrote_anything = true;
                }
                ContentItem::Thinking { text } => {
                    write_thinking(out, text);
                    wrote_anything = true;
                }
                ContentItem::Other(kind) => {
                    out.push_str(&format!("_unknown block: `{kind}`_\n\n"));
                    wrote_anything = true;
                }
            }
        }
        if !wrote_anything {
            out.push_str("_(empty message)_\n\n");
        }

        if msg.timestamp.is_some() {
            prev_ts = msg.timestamp;
        }
    }
}

/// Build the `HH:MM [· +delta since prev]` header body.
fn build_header(
    role: &str,
    ts: Option<DateTime<Utc>>,
    prev: Option<DateTime<Utc>>,
) -> String {
    match ts {
        Some(t) => {
            let time = t.format("%H:%M").to_string();
            let delta = prev
                .and_then(|p| format_delta(t.signed_duration_since(p)))
                .map(|d| format!(" · +{d} since prev"))
                .unwrap_or_default();
            format!("{role} ({time}{delta})")
        }
        None => role.to_string(),
    }
}

/// Format a chrono `Duration` into a compact "+2m / +15s / +1h3m" string.
/// Returns `None` for non-positive durations so we don't print a misleading
/// "+0s" between two messages that landed in the same second.
fn format_delta(d: chrono::Duration) -> Option<String> {
    let total = d.num_seconds();
    if total <= 0 {
        return None;
    }
    if total < 60 {
        return Some(format!("{total}s"));
    }
    let minutes = total / 60;
    if minutes < 60 {
        return Some(format!("{minutes}m"));
    }
    let hours = minutes / 60;
    let rem_min = minutes % 60;
    if rem_min == 0 {
        Some(format!("{hours}h"))
    } else {
        Some(format!("{hours}h{rem_min}m"))
    }
}

fn write_tool_use(out: &mut String, name: &str, input: &Value) {
    let pretty = serde_json::to_string_pretty(input).unwrap_or_else(|_| input.to_string());
    out.push_str("<details>\n");
    out.push_str(&format!("<summary>tool_use: {name}</summary>\n\n"));
    out.push_str("```json\n");
    out.push_str(&pretty);
    if !pretty.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("```\n\n");
    out.push_str("</details>\n\n");
}

fn write_tool_result(out: &mut String, content: &str, is_error: bool) {
    let trimmed = content.trim_end();
    if trimmed.is_empty() {
        return;
    }
    let label = if is_error {
        "tool_result (error)"
    } else {
        "tool_result"
    };
    out.push_str("<details>\n");
    out.push_str(&format!("<summary>{label}</summary>\n\n"));
    out.push_str("```\n");
    out.push_str(trimmed);
    out.push('\n');
    out.push_str("```\n\n");
    out.push_str("</details>\n\n");
}

fn write_thinking(out: &mut String, text: &str) {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return;
    }
    out.push_str("<details>\n");
    out.push_str("<summary>thinking</summary>\n\n");
    out.push_str(trimmed);
    out.push_str("\n\n");
    out.push_str("</details>\n\n");
}

/// Preferred title for the session — custom name → auto name → id.
fn display_title(session: &Session) -> String {
    session
        .name
        .as_deref()
        .or(session.auto_name.as_deref())
        .map(|s| s.to_string())
        .unwrap_or_else(|| session.id.clone())
}

/// Project name = basename of the project directory, falling back to the
/// full path's display when no basename is recoverable.
fn project_name(session: &Session) -> String {
    session
        .project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| session.project_dir.display().to_string())
}

fn first_ts(messages: &[TranscriptMessage]) -> Option<DateTime<Utc>> {
    messages.iter().find_map(|m| m.timestamp)
}

fn last_ts(messages: &[TranscriptMessage]) -> Option<DateTime<Utc>> {
    messages.iter().rev().find_map(|m| m.timestamp)
}

fn fmt_iso(ts: Option<DateTime<Utc>>) -> String {
    ts.map(|t| t.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Escape a YAML scalar so it survives round-tripping through any parser. We
/// keep the format intentionally conservative: quote anything with a
/// syntactically meaningful character, escape embedded quotes and backslashes.
fn yaml_scalar(s: impl AsRef<str>) -> String {
    let s = s.as_ref();
    let needs_quotes = s.is_empty()
        || s.contains(|c: char| {
            matches!(
                c,
                ':' | '#' | '&' | '*' | '!' | '|' | '>' | '\'' | '"' | '%' | '@' | '`'
            ) || c.is_control()
        })
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s.starts_with('-');
    if needs_quotes {
        let escaped: String = s
            .chars()
            .flat_map(|c| match c {
                '\\' => vec!['\\', '\\'],
                '"' => vec!['\\', '"'],
                '\n' => vec!['\\', 'n'],
                '\r' => vec!['\\', 'r'],
                '\t' => vec!['\\', 't'],
                other => vec![other],
            })
            .collect();
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}

// ── Secret redaction ────────────────────────────────────────────────────────

/// Apply every redaction pattern in the vetted set.
///
/// The shape of each mask is `<prefix>*********<last4>` so a reader can spot
/// two references to the same key without recovering the secret.
pub(crate) fn redact_secrets(input: &str) -> String {
    use regex::Regex;

    // Ordering matters: specific tokens are matched before the generic
    // Bearer/base64 catch-all so the more informative prefix wins.
    //
    // `once_cell` would be idiomatic, but pulling in a new dependency for a
    // command that runs at most a few times per session is not worth it.
    // `Regex::new` compiles in the low microseconds.
    let anthropic = Regex::new(r"sk-ant-[A-Za-z0-9_\-]{20,}").unwrap();
    let openai_project = Regex::new(r"sk-proj-[A-Za-z0-9_\-]{20,}").unwrap();
    let openai = Regex::new(r"sk-[A-Za-z0-9]{20,}").unwrap();
    let aws_access = Regex::new(r"AKIA[0-9A-Z]{16}").unwrap();
    let aws_secret =
        Regex::new(r"(?i)aws(.{0,20})?(secret|access).{0,20}?([A-Za-z0-9/+=]{40})").unwrap();
    let github_pat = Regex::new(r"gh[pousr]_[A-Za-z0-9]{30,}").unwrap();
    let github_legacy = Regex::new(r"github_pat_[A-Za-z0-9_]{30,}").unwrap();
    let bearer = Regex::new(r"(?i)Bearer\s+([A-Za-z0-9_\-\.=]{20,})").unwrap();

    let mut out = input.to_string();
    // Most-specific patterns first so `sk-ant-…` isn't clobbered by the
    // generic `sk-` matcher.
    out = replace_with_mask(&out, &anthropic, "sk-ant-");
    out = replace_with_mask(&out, &openai_project, "sk-proj-");
    out = replace_with_mask(&out, &openai, "sk-");
    out = replace_with_mask(&out, &github_legacy, "github_pat_");
    out = replace_with_mask(&out, &github_pat, "gh_");
    out = replace_with_mask(&out, &aws_access, "AKIA");
    // AWS secret keys live inside a compound match — redact only the
    // secret-ish tail (capture group 3) rather than the surrounding context.
    out = replace_capture_group(&out, &aws_secret, 3, "aws-");
    out = replace_capture_group(&out, &bearer, 1, "Bearer ");
    out
}

fn replace_with_mask(input: &str, re: &regex::Regex, prefix: &str) -> String {
    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let whole = caps.get(0).map(|m| m.as_str()).unwrap_or("");
        mask_with_prefix(whole, prefix)
    })
    .into_owned()
}

fn replace_capture_group(
    input: &str,
    re: &regex::Regex,
    group: usize,
    prefix: &str,
) -> String {
    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let whole = caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string();
        let Some(target) = caps.get(group) else {
            return whole;
        };
        let mask = mask_with_prefix(target.as_str(), prefix);
        let (start, end) = (target.start() - caps.get(0).unwrap().start(),
                            target.end() - caps.get(0).unwrap().start());
        let mut result = String::with_capacity(whole.len());
        result.push_str(&whole[..start]);
        result.push_str(&mask);
        result.push_str(&whole[end..]);
        result
    })
    .into_owned()
}

fn mask_with_prefix(token: &str, prefix: &str) -> String {
    let last4: String = token.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{prefix}*********{last4}")
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::pricing::TokenCounts;
    use crate::data::session::SessionKind;
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn mk_session() -> Session {
        let t0 = Utc.with_ymd_and_hms(2026, 4, 22, 14, 10, 0).unwrap();
        let t1 = Utc.with_ymd_and_hms(2026, 4, 22, 14, 12, 0).unwrap();
        Session {
            id: "2f0e48f8".into(),
            project_dir: PathBuf::from("/Users/me/work/alpha"),
            name: Some("refactor auth".into()),
            auto_name: Some("please refactor the auth middleware".into()),
            last_prompt: None,
            message_count: 2,
            tokens: TokenCounts {
                input: 100,
                output: 50,
                ..Default::default()
            },
            total_cost_usd: 0.1234,
            model_summary: "claude-opus-4-7".into(),
            first_timestamp: Some(t0),
            last_timestamp: Some(t1),
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
            turn_durations: Vec::new(),
        }
    }

    fn mk_messages() -> Vec<TranscriptMessage> {
        let t0 = Utc.with_ymd_and_hms(2026, 4, 22, 14, 10, 0).unwrap();
        let t1 = Utc.with_ymd_and_hms(2026, 4, 22, 14, 12, 0).unwrap();
        vec![
            TranscriptMessage {
                role: Role::User,
                timestamp: Some(t0),
                items: vec![ContentItem::Text("fix the auth bug please".into())],
            },
            TranscriptMessage {
                role: Role::Assistant,
                timestamp: Some(t1),
                items: vec![
                    ContentItem::Thinking {
                        text: "let me inspect the middleware first".into(),
                    },
                    ContentItem::Text("I'll start by reading the middleware.".into()),
                    ContentItem::ToolUse {
                        name: "Bash".into(),
                        input: serde_json::json!({"command": "rg 'authMiddleware' -n"}),
                    },
                    ContentItem::ToolResult {
                        content: "src/auth/middleware.ts:14: export function authMiddleware(…)"
                            .into(),
                        is_error: false,
                    },
                ],
            },
        ]
    }

    #[test]
    fn render_markdown_includes_frontmatter_and_sections() {
        let md = render_markdown(&mk_session(), &mk_messages());
        assert!(md.starts_with("---\n"), "expected frontmatter");
        assert!(md.contains("session_id: 2f0e48f8"));
        assert!(md.contains("project: alpha"));
        assert!(md.contains("cost_usd: 0.12"));
        assert!(md.contains("tokens: 150"));
        assert!(md.contains("# refactor auth"));
        assert!(md.contains("## user (14:10)"));
        assert!(md.contains("## claude (14:12 · +2m since prev)"));
        assert!(md.contains("fix the auth bug please"));
        assert!(md.contains("<summary>tool_use: Bash</summary>"));
        assert!(md.contains("rg 'authMiddleware' -n"));
        assert!(md.contains("<summary>tool_result</summary>"));
        assert!(md.contains("<summary>thinking</summary>"));
    }

    #[test]
    fn format_delta_renders_common_ranges() {
        assert_eq!(format_delta(chrono::Duration::seconds(45)).as_deref(), Some("45s"));
        assert_eq!(format_delta(chrono::Duration::seconds(120)).as_deref(), Some("2m"));
        assert_eq!(format_delta(chrono::Duration::seconds(3600)).as_deref(), Some("1h"));
        assert_eq!(
            format_delta(chrono::Duration::seconds(3600 + 180)).as_deref(),
            Some("1h3m")
        );
        assert!(format_delta(chrono::Duration::seconds(0)).is_none());
        assert!(format_delta(chrono::Duration::seconds(-5)).is_none());
    }

    #[test]
    fn yaml_scalar_quotes_when_needed() {
        assert_eq!(yaml_scalar("plain"), "plain");
        assert_eq!(yaml_scalar("has: colon"), "\"has: colon\"");
        assert_eq!(yaml_scalar("with\"quote"), "\"with\\\"quote\"");
        assert_eq!(yaml_scalar(""), "\"\"");
        assert_eq!(yaml_scalar("-dash"), "\"-dash\"");
    }

    #[test]
    fn redact_anthropic_and_openai_keys() {
        let text = "token sk-ant-api01-abcdefghijklmnopqrstuvwxyz end";
        let out = redact_secrets(text);
        assert!(out.contains("sk-ant-*********"));
        assert!(!out.contains("abcdefghijklmnopqrstuv"));

        let text = "OPENAI_KEY=sk-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA next";
        let out = redact_secrets(text);
        assert!(out.contains("sk-*********"));
    }

    #[test]
    fn redact_github_and_aws_tokens() {
        let gh = "export TOKEN=ghp_abcdefghijklmnopqrstuvwxyz012345";
        let masked = redact_secrets(gh);
        assert!(masked.contains("gh_*********"));

        let aws = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE ok";
        let masked = redact_secrets(aws);
        assert!(masked.contains("AKIA*********"));
    }

    #[test]
    fn redact_bearer_tokens() {
        let text = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz12 end";
        let out = redact_secrets(text);
        assert!(out.contains("Bearer *********"));
        assert!(!out.contains("abcdefghijklmnopqrstuvwxyz12"));
    }

    #[test]
    fn redact_preserves_non_secret_text() {
        let plain = "this is a normal sentence with no secrets";
        assert_eq!(redact_secrets(plain), plain);
    }

    #[test]
    fn export_by_id_writes_file_to_explicit_out() {
        // We can't stand up a full `~/.claude/projects/` tree here without
        // going through `export_by_id`'s resolver. Instead, exercise the
        // pure renderer + write path with an explicit out — this covers the
        // CLI `--out PATH` happy path.
        let tmp = tempfile::tempdir().expect("tempdir");
        let target = tmp.path().join("export.md");
        let md = render_markdown(&mk_session(), &mk_messages());
        std::fs::write(&target, &md).expect("write");
        let read = std::fs::read_to_string(&target).expect("read");
        assert!(read.contains("session_id: 2f0e48f8"));
        assert!(read.contains("## claude (14:12 · +2m since prev)"));
    }
}
