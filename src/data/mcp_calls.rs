//! Extract MCP tool-use events from session JSONL files.
//!
//! Claude Code logs every tool invocation as a `message.content[*].type ==
//! "tool_use"` entry. Tools exposed by an MCP server are prefixed
//! `mcp__<server>__<tool>`; the double-underscore delimiter is stable across
//! the Claude Code CLI, the Anthropic Agent SDK, and every MCP server we've
//! audited. (See `RESEARCH-claude-features.md` — "MCP" section.)
//!
//! This module streams every `.jsonl` under `~/.claude/projects/*/` and
//! aggregates calls by **tool name** and derived **server name**. Output is
//! two flat lists plus a per-server → session-ids back-reference; the UI
//! layer then sorts / formats them.
//!
//! Streaming is line-at-a-time because a single session can be 100 MB. The
//! parser is forgiving — one bad line never kills the rest of the file.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// One aggregate entry for a single MCP tool.
#[derive(Debug, Clone)]
pub struct ToolCallStats {
    /// Full tool name, e.g. `"mcp__context7__query-docs"`.
    pub name: String,
    /// Derived server name — the middle segment. `"context7"` above.
    pub server: String,
    pub calls: u64,
    pub last_used: Option<DateTime<Utc>>,
}

/// One aggregate entry for an MCP server — the sum of every tool it exposes
/// that Claude Code actually invoked.
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub name: String,
    pub calls: u64,
    pub last_used: Option<DateTime<Utc>>,
    /// Session ids (file stems) where this server was used. Used by the
    /// `Enter → list sessions` drill-down.
    pub sessions: BTreeSet<String>,
}

/// Bundle returned by [`scan_mcp_calls`].
#[derive(Debug, Clone, Default)]
pub struct McpCallData {
    /// One entry per distinct tool name. Sorted by `calls` descending.
    pub tools: Vec<ToolCallStats>,
    /// One entry per distinct server. Sorted by `calls` descending.
    pub servers: Vec<ServerStats>,
}

impl McpCallData {
    /// Sum of calls across every tool — the dashboard header uses this.
    pub fn total_calls(&self) -> u64 {
        self.tools.iter().map(|t| t.calls).sum()
    }
}

// ── JSONL parsing ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RawLine {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    message: Option<RawMessage>,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    #[serde(default)]
    content: serde_json::Value,
}

/// Scan `~/.claude/projects/**/*.jsonl` and aggregate every MCP tool-use.
///
/// Returns `Ok(McpCallData::default())` when the projects dir is missing or
/// empty. `Err` only when `$HOME` is unresolvable.
pub fn scan_mcp_calls() -> anyhow::Result<McpCallData> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects_dir = home.join(".claude").join("projects");
    Ok(scan_mcp_calls_in(&projects_dir))
}

/// Test-friendly variant: scan a specific root. Walks one level deep
/// (`<root>/<project>/*.jsonl`) to match Claude Code's layout.
pub fn scan_mcp_calls_in(projects_dir: &Path) -> McpCallData {
    let mut by_tool: HashMap<String, ToolCallStats> = HashMap::new();
    let mut by_server: HashMap<String, ServerStats> = HashMap::new();

    let Ok(projects) = std::fs::read_dir(projects_dir) else {
        return McpCallData::default();
    };

    for pe in projects.flatten() {
        let pdir = pe.path();
        if !pdir.is_dir() {
            continue;
        }
        let Ok(sessions) = std::fs::read_dir(&pdir) else {
            continue;
        };
        for se in sessions.flatten() {
            let path = se.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let sid = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            scan_one(&path, &sid, &mut by_tool, &mut by_server);
        }
    }

    finalise(by_tool, by_server)
}

/// Stream one JSONL and feed the aggregators. Malformed lines are silently
/// skipped — the whole point of the shape is that we have no schema contract
/// with Claude Code, so robustness > strictness.
fn scan_one(
    path: &PathBuf,
    session_id: &str,
    by_tool: &mut HashMap<String, ToolCallStats>,
    by_server: &mut HashMap<String, ServerStats>,
) {
    let Ok(file) = File::open(path) else { return };
    let reader = BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        // Cheap guard — every JSONL entry we care about contains `"mcp__"`
        // literally. Skip the JSON parse if it doesn't.
        if !line.contains("\"mcp__") {
            continue;
        }
        let Ok(raw) = serde_json::from_str::<RawLine>(&line) else {
            continue;
        };
        let Some(msg) = raw.message else { continue };
        let ts = raw.timestamp.as_deref().and_then(parse_ts);

        // `content` is an array of items, each has a `type` field. We only
        // care about `tool_use` items whose `name` starts with `mcp__`.
        let Some(arr) = msg.content.as_array() else {
            continue;
        };
        for item in arr {
            let Some(obj) = item.as_object() else {
                continue;
            };
            if obj.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
                continue;
            }
            let Some(name) = obj.get("name").and_then(|n| n.as_str()) else {
                continue;
            };
            if !name.starts_with("mcp__") {
                continue;
            }
            let server = derive_server(name).unwrap_or_else(|| "(unknown)".to_string());

            // Per-tool.
            let t = by_tool.entry(name.to_string()).or_insert(ToolCallStats {
                name: name.to_string(),
                server: server.clone(),
                calls: 0,
                last_used: None,
            });
            t.calls = t.calls.saturating_add(1);
            t.last_used = later(t.last_used, ts);

            // Per-server.
            let s = by_server.entry(server.clone()).or_insert(ServerStats {
                name: server.clone(),
                calls: 0,
                last_used: None,
                sessions: BTreeSet::new(),
            });
            s.calls = s.calls.saturating_add(1);
            s.last_used = later(s.last_used, ts);
            s.sessions.insert(session_id.to_string());
        }
    }
}

fn finalise(
    by_tool: HashMap<String, ToolCallStats>,
    by_server: HashMap<String, ServerStats>,
) -> McpCallData {
    let mut tools: Vec<_> = by_tool.into_values().collect();
    tools.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.name.cmp(&b.name)));
    let mut servers: Vec<_> = by_server.into_values().collect();
    servers.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.name.cmp(&b.name)));
    McpCallData { tools, servers }
}

/// `mcp__<server>__<tool>` → `<server>`. Returns `None` when the prefix is
/// missing the middle segment (should never happen in practice).
pub fn derive_server(tool_name: &str) -> Option<String> {
    let rest = tool_name.strip_prefix("mcp__")?;
    let idx = rest.find("__")?;
    Some(rest[..idx].to_string())
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn later(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(x), Some(y)) => Some(x.max(y)),
    }
}

/// Merge settings-declared servers with scan-observed servers. The UI
/// displays *every* declared server (so a zero-call install still appears in
/// the list) and any server that Claude Code actually called without being
/// in settings.json (which happens when it's configured at the CLI level or
/// shipped as part of a plugin).
pub fn merge_declared(
    declared: &[String],
    observed: &[ServerStats],
) -> BTreeMap<String, ServerStats> {
    let mut out: BTreeMap<String, ServerStats> = BTreeMap::new();
    for name in declared {
        out.insert(
            name.clone(),
            ServerStats {
                name: name.clone(),
                calls: 0,
                last_used: None,
                sessions: BTreeSet::new(),
            },
        );
    }
    for s in observed {
        out.entry(s.name.clone())
            .and_modify(|e| {
                e.calls = s.calls;
                e.last_used = s.last_used;
                e.sessions = s.sessions.clone();
            })
            .or_insert_with(|| s.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn derive_server_happy_path() {
        assert_eq!(
            derive_server("mcp__context7__query-docs"),
            Some("context7".into())
        );
        assert_eq!(
            derive_server("mcp__firecrawl__scrape"),
            Some("firecrawl".into())
        );
    }

    #[test]
    fn derive_server_rejects_bad_prefix() {
        assert!(derive_server("Bash").is_none());
        assert!(derive_server("mcp__onlyone").is_none());
    }

    #[test]
    fn scan_aggregates_calls() {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path();
        let pdir = projects.join("-Users-me-foo");
        fs::create_dir_all(&pdir).unwrap();
        // Each JSONL line is one record; we want `tool_use` items inside
        // `message.content`.
        let line = |ts: &str, name: &str| {
            format!(
                r#"{{"timestamp":"{ts}","message":{{"content":[{{"type":"tool_use","name":"{name}"}}]}}}}"#
            )
        };
        let contents = [
            line("2026-04-16T10:00:00Z", "mcp__context7__query-docs"),
            line("2026-04-16T12:00:00Z", "mcp__context7__query-docs"),
            line("2026-04-16T14:00:00Z", "mcp__firecrawl__scrape"),
            // Non-mcp tool — ignored.
            line("2026-04-16T15:00:00Z", "Bash"),
        ]
        .join("\n");
        fs::write(pdir.join("abc.jsonl"), contents).unwrap();

        let data = scan_mcp_calls_in(projects);
        assert_eq!(data.tools.len(), 2);
        let ctx = data
            .tools
            .iter()
            .find(|t| t.name == "mcp__context7__query-docs")
            .unwrap();
        assert_eq!(ctx.calls, 2);
        assert_eq!(ctx.server, "context7");

        // Servers sorted by calls desc — context7 (2) before firecrawl (1).
        assert_eq!(data.servers.len(), 2);
        assert_eq!(data.servers[0].name, "context7");
        assert_eq!(data.servers[0].calls, 2);
        assert_eq!(data.servers[0].sessions.len(), 1);
        assert!(data.servers[0].sessions.contains("abc"));
        assert_eq!(data.servers[1].name, "firecrawl");
    }

    #[test]
    fn scan_skips_malformed_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let pdir = tmp.path().join("proj");
        fs::create_dir_all(&pdir).unwrap();
        let contents = "\n".to_string()
            + "{{not json\n"
            + r#"{"message":{"content":[{"type":"tool_use","name":"mcp__ctx__q"}]}}"#
            + "\n"
            + r#"{"message":{"content":"not an array"}}"#
            + "\n";
        fs::write(pdir.join("a.jsonl"), contents).unwrap();

        let data = scan_mcp_calls_in(tmp.path());
        assert_eq!(data.servers.len(), 1);
        assert_eq!(data.servers[0].name, "ctx");
    }

    #[test]
    fn merge_declared_keeps_empty_installs() {
        let decl = vec!["context7".to_string(), "unused".to_string()];
        let obs = vec![ServerStats {
            name: "context7".into(),
            calls: 5,
            last_used: None,
            sessions: BTreeSet::new(),
        }];
        let merged = merge_declared(&decl, &obs);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged["context7"].calls, 5);
        assert_eq!(merged["unused"].calls, 0);
    }

    #[test]
    fn missing_projects_dir_returns_empty() {
        let data = scan_mcp_calls_in(Path::new("/does/not/exist"));
        assert!(data.tools.is_empty());
        assert!(data.servers.is_empty());
    }
}
