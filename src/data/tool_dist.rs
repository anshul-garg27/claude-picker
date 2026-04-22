//! Per-tool distribution — powers the cost-audit drill-in detail view (#16).
//!
//! When the user hits Enter on a `ToolRatio` finding we want to show *which*
//! tools dominated that session's output. The tool-ratio audit heuristic
//! already knows how to recognise tool-heavy traffic; this module takes it
//! one step further and splits the count by `tool_use.name`.
//!
//! Kept as a sibling to `cost_audit` rather than an extension of it because:
//! 1. The heuristic pipeline does not need this data; only the drill-in
//!    does. A separate module keeps the hot audit-run path from paying for
//!    the extra bookkeeping.
//! 2. The data layout (`HashMap<String, ToolUsage>`) is very different from
//!    the scalar `SessionStats` the heuristics consume, so fusing the two
//!    would muddle both APIs.
//!
//! ## Accounting
//!
//! For each assistant message that contains one or more `tool_use` blocks:
//! - `call_count` ← +1 per block.
//! - `output_tokens` ← the message's `usage.output_tokens`, split evenly
//!   across the blocks (first block carries the remainder so the sum is
//!   exact).
//!
//! For the *next* assistant message after a `tool_result`:
//! - `input_tokens_after` ← that message's `usage.input_tokens`, split
//!   evenly across the tool names that fired last. This is the cost of
//!   reading the tool result back into context.
//!
//! The split accounting isn't perfect — a `Bash` + `Read` combined message
//! will see half the input bytes attributed to each — but it's good enough
//! to rank tools relative to each other and to back the "72% of your output
//! was Bash" narrative the UI wants to show.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Per-tool accounting consumed by the drill-in detail view (#16).
///
/// `call_count` is the number of `tool_use` blocks with this `name`.
/// `output_tokens` is the *assistant* output tokens attributed to the
/// tool_use-carrying message (same accounting as the tool-ratio heuristic).
/// `input_tokens_after` is the input tokens on the assistant message
/// immediately after the tool result — i.e. the cost of reading the tool's
/// output back into context.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ToolUsage {
    pub call_count: u32,
    pub output_tokens: u64,
    pub input_tokens_after: u64,
}

/// Row in the sorted `collect_tool_distribution` output. Keeping the tool
/// name alongside [`ToolUsage`] makes the public API a flat `Vec<_>` ordered
/// by output tokens desc, which is what the UI renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDistEntry {
    pub name: String,
    pub usage: ToolUsage,
}

/// Public drill-in helper: return per-tool usage for one session's JSONL,
/// sorted by `output_tokens` descending. Empty / unreadable path returns an
/// empty vector so the UI can render a "no tool_use blocks found"
/// placeholder without special-casing errors.
pub fn collect_tool_distribution(jsonl_path: &Path) -> Vec<ToolDistEntry> {
    let map = walk_jsonl(jsonl_path);
    let mut entries: Vec<ToolDistEntry> = map
        .into_iter()
        .map(|(name, usage)| ToolDistEntry { name, usage })
        .collect();
    // Descending by output tokens, tie-break alphabetically for determinism.
    entries.sort_by(|a, b| {
        b.usage
            .output_tokens
            .cmp(&a.usage.output_tokens)
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}

/// Resolve the JSONL path for a session by walking every project directory
/// under `~/.claude/projects/` and looking for `<session_id>.jsonl`.
/// Returns the first match — session IDs are UUIDs so ambiguity is
/// effectively impossible. Used by the audit drill-in to compute
/// `collect_tool_distribution` on demand without bloating `AuditFinding`
/// with a stored path.
pub fn find_session_jsonl(session_id: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let projects_root = home.join(".claude").join("projects");
    let entries = std::fs::read_dir(&projects_root).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path().join(format!("{session_id}.jsonl"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Internal: read every assistant message, bucket tool traffic by name.
fn walk_jsonl(path: &Path) -> HashMap<String, ToolUsage> {
    let mut map: HashMap<String, ToolUsage> = HashMap::new();
    let Ok(file) = File::open(path) else {
        return map;
    };
    let reader = BufReader::new(file);

    // Tool names seen on the *previous* assistant message. The next
    // assistant message's input tokens are read as "tool_result traffic"
    // and split evenly across those names.
    let mut last_tool_names: Vec<String> = Vec::new();

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
        if raw.kind.as_deref() != Some("assistant") {
            continue;
        }
        let Some(msg) = raw.message else { continue };
        let Some(usage) = msg.usage else { continue };

        let tool_names: Vec<String> = msg
            .content
            .as_ref()
            .map(collect_tool_use_names)
            .unwrap_or_default();

        if !tool_names.is_empty() {
            let n = tool_names.len() as u64;
            let per_call_output = usage.output_tokens / n;
            let remainder_output = usage.output_tokens % n;
            for (idx, name) in tool_names.iter().enumerate() {
                let slot = map.entry(name.clone()).or_default();
                slot.call_count = slot.call_count.saturating_add(1);
                // First tool carries the remainder so sum-across-tools
                // equals usage.output_tokens exactly.
                let share = per_call_output + if idx == 0 { remainder_output } else { 0 };
                slot.output_tokens = slot.output_tokens.saturating_add(share);
            }
            last_tool_names = tool_names;
        } else if !last_tool_names.is_empty() {
            let n = last_tool_names.len() as u64;
            let per_call_input = usage.input_tokens / n;
            let remainder_input = usage.input_tokens % n;
            for (idx, name) in last_tool_names.iter().enumerate() {
                let slot = map.entry(name.clone()).or_default();
                let share = per_call_input + if idx == 0 { remainder_input } else { 0 };
                slot.input_tokens_after = slot.input_tokens_after.saturating_add(share);
            }
            last_tool_names.clear();
        }
    }
    map
}

/// Extract the `name` field from every `tool_use` block in `content`.
/// Anonymous blocks (missing or non-string `name`) are recorded as
/// `"unknown"` so the "any tool use present?" check still fires on them.
fn collect_tool_use_names(content: &serde_json::Value) -> Vec<String> {
    let Some(blocks) = content.as_array() else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
        .map(|b| {
            b.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string()
        })
        .collect()
}

// ── Minimal JSONL schema just for this module. Kept separate from the
// `RawLine`/`RawMsg` types in `cost_audit` so the two modules stay
// independently updatable. ─────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct RawLine {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<RawMsg>,
}

#[derive(Debug, Deserialize, Default)]
struct RawMsg {
    #[serde(default)]
    content: Option<serde_json::Value>,
    #[serde(default)]
    usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a synthetic JSONL with two tool turns + one text turn.
    fn write_synthetic_jsonl(path: &Path) {
        use std::io::Write;
        let mut f = File::create(path).unwrap();
        // Turn 1: assistant tool_use (Bash) → user tool_result → assistant
        // text (the follow-up carries input_tokens_after for Bash).
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Bash","input":{{}}}}],"usage":{{"input_tokens":0,"output_tokens":1000,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"ok"}}]}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"done"}}],"usage":{{"input_tokens":500,"output_tokens":50,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}}}}"#
        )
        .unwrap();
        // Turn 2: tool_use (Read) → tool_result → text.
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Read","input":{{}}}}],"usage":{{"input_tokens":0,"output_tokens":200,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"ok"}}]}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"done"}}],"usage":{{"input_tokens":100,"output_tokens":30,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}}}}"#
        )
        .unwrap();
    }

    #[test]
    fn collect_tool_distribution_sorts_by_output_desc() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        write_synthetic_jsonl(&path);

        let entries = collect_tool_distribution(&path);
        assert_eq!(entries.len(), 2, "expected Bash + Read");
        // Bash has 1000 output vs Read's 200 → Bash first.
        assert_eq!(entries[0].name, "Bash");
        assert_eq!(entries[0].usage.output_tokens, 1000);
        assert_eq!(entries[0].usage.call_count, 1);
        assert_eq!(entries[0].usage.input_tokens_after, 500);
        assert_eq!(entries[1].name, "Read");
        assert_eq!(entries[1].usage.output_tokens, 200);
        assert_eq!(entries[1].usage.input_tokens_after, 100);
    }

    #[test]
    fn collect_tool_distribution_empty_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.jsonl");
        assert!(collect_tool_distribution(&path).is_empty());
    }

    #[test]
    fn collect_tool_use_names_skips_non_tool_blocks() {
        let content = serde_json::json!([
            {"type": "text", "text": "hello"},
            {"type": "tool_use", "name": "Grep", "input": {}},
            {"type": "tool_use", "name": "Edit", "input": {}},
        ]);
        let names = collect_tool_use_names(&content);
        assert_eq!(names, vec!["Grep".to_string(), "Edit".to_string()]);
    }

    #[test]
    fn collect_tool_use_names_records_unknown_for_anonymous_block() {
        let content = serde_json::json!([
            {"type": "tool_use", "input": {}},
        ]);
        let names = collect_tool_use_names(&content);
        assert_eq!(names, vec!["unknown".to_string()]);
    }

    #[test]
    fn multiple_tool_uses_in_one_message_split_evenly() {
        // Two tools on one message, 1000 output tokens → 500 each (no
        // remainder). Call counts increment independently.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.jsonl");
        use std::io::Write;
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Grep","input":{{}}}},{{"type":"tool_use","name":"Read","input":{{}}}}],"usage":{{"input_tokens":0,"output_tokens":1000,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}}}}"#
        )
        .unwrap();
        drop(f);
        let entries = collect_tool_distribution(&path);
        assert_eq!(entries.len(), 2);
        // Order is alphabetical when tied on output_tokens; but Grep got
        // the remainder (none here) so both share 500 evenly.
        let total: u64 = entries.iter().map(|e| e.usage.output_tokens).sum();
        assert_eq!(total, 1000, "output split must sum to the message total");
        for e in &entries {
            assert_eq!(e.usage.call_count, 1);
            assert_eq!(e.usage.output_tokens, 500);
        }
    }
}
