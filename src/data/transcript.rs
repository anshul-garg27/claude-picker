//! Full-content session transcript parser.
//!
//! Whereas [`crate::data::session`] aggregates a session's JSONL into rollup
//! metadata (token counts, cost, name), this module parses the entire
//! conversation into a sequence of rendered blocks the conversation viewer
//! can scroll through. Each line in the JSONL becomes at most one
//! [`TranscriptMessage`] which in turn carries one-or-more [`ContentItem`]s:
//! plain text, a tool_use block, a tool_result, or extended-thinking content.
//!
//! The parser is deliberately forgiving in the same style as
//! `load_session_from_jsonl` — malformed lines are skipped, unknown block
//! kinds fall through to `Other`, and we never panic on a surprising shape.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

/// One role-tagged message from the transcript.
#[derive(Debug, Clone)]
pub struct TranscriptMessage {
    pub role: Role,
    pub timestamp: Option<DateTime<Utc>>,
    pub items: Vec<ContentItem>,
}

/// The two conversational roles the viewer cares about. Tool-use /
/// tool-result blocks are surfaced *within* a message's [`ContentItem`] list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// A single content block within a message — matches the shapes Claude Code
/// writes into its JSONL.
#[derive(Debug, Clone)]
pub enum ContentItem {
    /// Plain text from the user or assistant.
    Text(String),
    /// Assistant requested a tool invocation. `input` is kept as JSON so the
    /// renderer can pretty-print or summarise whichever fields are
    /// interesting per-tool.
    ToolUse { name: String, input: Value },
    /// Tool output flowing back into the conversation. Tool calls may return
    /// nested content arrays (for tools that stream multimodal output), so
    /// this recursively carries sub-items.
    ToolResult { content: String, is_error: bool },
    /// Extended-thinking block. Claude writes these on models that support
    /// `thinking` as part of the assistant message; the viewer renders them
    /// in a muted, italic collapsible box.
    Thinking { text: String },
    /// Anything we don't recognise. Carries the raw `type` string so the
    /// renderer can draw a "[unknown: …]" placeholder without losing data.
    Other(String),
}

impl TranscriptMessage {
    /// Flatten all plain-text content into a single string — useful for the
    /// "copy message to clipboard" flow. Tool calls / results are rendered
    /// in a terse format so the copy remains useful even on assistant turns
    /// that are mostly tool invocations.
    pub fn as_plain_text(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            match item {
                ContentItem::Text(s) => {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(s);
                }
                ContentItem::ToolUse { name, input } => {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    let summary = summarize_tool_input(input);
                    if summary.is_empty() {
                        out.push_str(&format!("[tool_use: {name}]"));
                    } else {
                        out.push_str(&format!("[tool_use: {name}] {summary}"));
                    }
                }
                ContentItem::ToolResult { content, is_error } => {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    let prefix = if *is_error {
                        "[tool_result: error]"
                    } else {
                        "[tool_result]"
                    };
                    out.push_str(&format!("{prefix} {content}"));
                }
                ContentItem::Thinking { text } => {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(&format!("[thinking] {text}"));
                }
                ContentItem::Other(k) => {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&format!("[{k}]"));
                }
            }
        }
        out
    }
}

/// Pull out a one-line summary for a tool's input JSON — just the most
/// useful field for the common tools. Not exhaustive; the renderer shows
/// the full JSON below the summary.
fn summarize_tool_input(input: &Value) -> String {
    let Some(obj) = input.as_object() else {
        return String::new();
    };
    // Heuristic per common tool shape.
    for key in ["file_path", "path", "command", "pattern", "query", "url"] {
        if let Some(v) = obj.get(key).and_then(|v| v.as_str()) {
            return format!("{key}: {v}");
        }
    }
    // Fall back to the first string-valued field we see so the summary
    // isn't empty for novel tool shapes.
    for (k, v) in obj {
        if let Some(s) = v.as_str() {
            let truncated = if s.len() > 80 {
                format!("{}…", &s[..80])
            } else {
                s.to_string()
            };
            return format!("{k}: {truncated}");
        }
    }
    String::new()
}

/// Raw JSONL line shape — just enough to drive the transcript parser.
#[derive(Debug, Deserialize, Default)]
struct RawLine {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<RawMessage>,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawMessage {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<Value>,
}

/// Parse a `<id>.jsonl` file into an ordered list of transcript messages.
///
/// Malformed or non-message lines are silently skipped. Returns an empty
/// vector rather than an error for missing files so the viewer can render a
/// "(no messages)" placeholder cleanly.
pub fn load_transcript(path: &Path) -> anyhow::Result<Vec<TranscriptMessage>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut out: Vec<TranscriptMessage> = Vec::new();
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let raw: RawLine = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let kind = raw.kind.as_deref().unwrap_or("");
        if kind != "user" && kind != "assistant" {
            continue;
        }
        let Some(msg) = raw.message else { continue };
        let role = match msg.role.as_deref() {
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            _ => continue,
        };
        let Some(content) = msg.content else {
            continue;
        };
        let items = parse_content(content);
        if items.is_empty() {
            continue;
        }
        let timestamp = raw.timestamp.as_deref().and_then(parse_ts);
        out.push(TranscriptMessage {
            role,
            timestamp,
            items,
        });
    }
    Ok(out)
}

/// Parse a message's `content` field — either a plain string or an array of
/// typed blocks. Returns an empty vec for content we can't parse.
fn parse_content(content: Value) -> Vec<ContentItem> {
    match content {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Vec::new()
            } else {
                vec![ContentItem::Text(trimmed.to_string())]
            }
        }
        Value::Array(blocks) => {
            let mut items = Vec::with_capacity(blocks.len());
            for b in blocks {
                if let Some(item) = parse_block(b) {
                    items.push(item);
                }
            }
            items
        }
        _ => Vec::new(),
    }
}

/// Parse a single typed content block.
fn parse_block(block: Value) -> Option<ContentItem> {
    let obj = block.as_object()?;
    let kind = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match kind {
        "text" => {
            let text = obj
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if text.is_empty() {
                return None;
            }
            Some(ContentItem::Text(text.to_string()))
        }
        "tool_use" => {
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let input = obj.get("input").cloned().unwrap_or(Value::Null);
            Some(ContentItem::ToolUse { name, input })
        }
        "tool_result" => {
            // tool_result.content is either a string or an array of
            // {"type":"text","text":…} blocks. Flatten either to one string.
            let is_error = obj
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let raw_content = obj.get("content").cloned().unwrap_or(Value::Null);
            let content = flatten_tool_result_content(&raw_content);
            Some(ContentItem::ToolResult { content, is_error })
        }
        "thinking" => {
            let text = obj
                .get("thinking")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if text.is_empty() {
                return None;
            }
            Some(ContentItem::Thinking {
                text: text.to_string(),
            })
        }
        other => Some(ContentItem::Other(other.to_string())),
    }
}

/// Flatten a `tool_result.content` value into a single displayable string.
fn flatten_tool_result_content(value: &Value) -> String {
    match value {
        Value::String(s) => s.to_string(),
        Value::Array(blocks) => {
            let mut out = String::new();
            for b in blocks {
                if let Some(o) = b.as_object() {
                    if o.get("type").and_then(|v| v.as_str()) == Some("text") {
                        if let Some(t) = o.get("text").and_then(|v| v.as_str()) {
                            if !out.is_empty() {
                                out.push('\n');
                            }
                            out.push_str(t);
                        }
                    }
                }
            }
            out
        }
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn parse_ts(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Resolve the on-disk path to `<session_id>.jsonl` under the Claude Code
/// projects root. Returns `None` if no such file exists in any project.
pub fn jsonl_path_for_session(session_id: &str) -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let projects = home.join(".claude").join("projects");
    if !projects.is_dir() {
        return None;
    }
    let entries = std::fs::read_dir(&projects).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path().join(format!("{session_id}.jsonl"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_jsonl(dir: &Path, lines: &[&str]) -> std::path::PathBuf {
        let path = dir.join("t.jsonl");
        fs::write(&path, lines.join("\n") + "\n").expect("write");
        path
    }

    #[test]
    fn parses_plain_text_user_and_assistant() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = write_jsonl(
            tmp.path(),
            &[
                r#"{"type":"user","message":{"role":"user","content":"hello there"}}"#,
                r#"{"type":"assistant","message":{"role":"assistant","content":"hi!"}}"#,
            ],
        );
        let t = load_transcript(&path).expect("ok");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].role, Role::User);
        assert_eq!(t[1].role, Role::Assistant);
        match &t[0].items[0] {
            ContentItem::Text(s) => assert_eq!(s, "hello there"),
            other => panic!("wrong item {other:?}"),
        }
    }

    #[test]
    fn parses_tool_use_blocks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = write_jsonl(
            tmp.path(),
            &[
                r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"ok"},{"type":"tool_use","name":"Edit","id":"x","input":{"file_path":"/a/b.rs"}}]}}"#,
            ],
        );
        let t = load_transcript(&path).expect("ok");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].items.len(), 2);
        match &t[0].items[1] {
            ContentItem::ToolUse { name, input } => {
                assert_eq!(name, "Edit");
                assert_eq!(
                    input.get("file_path").and_then(|v| v.as_str()),
                    Some("/a/b.rs")
                );
            }
            other => panic!("wrong item {other:?}"),
        }
    }

    #[test]
    fn parses_tool_result_string_and_array_forms() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = write_jsonl(
            tmp.path(),
            &[
                r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"x","content":"output"}]}}"#,
                r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"y","content":[{"type":"text","text":"part1"},{"type":"text","text":"part2"}]}]}}"#,
            ],
        );
        let t = load_transcript(&path).expect("ok");
        assert_eq!(t.len(), 2);
        match &t[0].items[0] {
            ContentItem::ToolResult { content, .. } => assert_eq!(content, "output"),
            other => panic!("wrong item {other:?}"),
        }
        match &t[1].items[0] {
            ContentItem::ToolResult { content, .. } => {
                assert!(content.contains("part1") && content.contains("part2"))
            }
            other => panic!("wrong item {other:?}"),
        }
    }

    #[test]
    fn parses_thinking_blocks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = write_jsonl(
            tmp.path(),
            &[
                r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"let me consider this"}]}}"#,
            ],
        );
        let t = load_transcript(&path).expect("ok");
        match &t[0].items[0] {
            ContentItem::Thinking { text } => assert_eq!(text, "let me consider this"),
            other => panic!("wrong item {other:?}"),
        }
    }

    #[test]
    fn skips_non_message_lines() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = write_jsonl(
            tmp.path(),
            &[
                r#"{"type":"custom-title","customTitle":"foo"}"#,
                r#"{"type":"permission-mode","permissionMode":"plan"}"#,
                r#"{"type":"user","message":{"role":"user","content":"real message"}}"#,
            ],
        );
        let t = load_transcript(&path).expect("ok");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn ignores_malformed_json() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = write_jsonl(
            tmp.path(),
            &[
                r#"{not valid"#,
                r#"{"type":"user","message":{"role":"user","content":"still works"}}"#,
            ],
        );
        let t = load_transcript(&path).expect("ok");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn as_plain_text_joins_blocks() {
        let msg = TranscriptMessage {
            role: Role::Assistant,
            timestamp: None,
            items: vec![
                ContentItem::Text("first".into()),
                ContentItem::ToolUse {
                    name: "Edit".into(),
                    input: serde_json::json!({"file_path": "/x"}),
                },
            ],
        };
        let plain = msg.as_plain_text();
        assert!(plain.contains("first"));
        assert!(plain.contains("tool_use: Edit"));
        assert!(plain.contains("/x"));
    }
}
