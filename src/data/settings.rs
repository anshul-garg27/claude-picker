//! Parser for `~/.claude/settings.json` — hook + MCP-server awareness.
//!
//! Claude Code persists two forms of user configuration in this file:
//!
//! 1. **Hooks**. The `hooks` object maps a hook-event name (`PreToolUse`,
//!    `PostToolUse`, `UserPromptSubmit`, `Stop`, …) to a list of
//!    *hook-groups*. Each group optionally carries a `matcher` (tool name or
//!    `*`) and a flat `hooks` list of command entries.
//!
//!    ```json
//!    {
//!      "hooks": {
//!        "PreToolUse": [{
//!          "matcher": "Bash",
//!          "hooks": [{"type": "command", "command": "..."}]
//!        }],
//!        "PostToolUse": [...],
//!        "UserPromptSubmit": [...]
//!      }
//!    }
//!    ```
//!
//! 2. **MCP servers**. `mcpServers` is a map from server name to a config
//!    object. The shape differs per transport (`stdio` uses `command`,
//!    `http`/`sse` uses `url`) but we only need the keyset + transport type.
//!
//! Per-project overrides live in `~/.claude-code-projects/<encoded>/settings.json`
//! (note: `.claude-code-projects`, separate from `.claude/projects/` which
//! stores transcripts). When that file exists its `hooks` / `mcpServers`
//! objects merge on top of the global entries. We keep both layers distinct
//! in the returned data so the UI can surface "overridden by project X" hints.
//!
//! Forgiving parsing: a missing key, a missing file, or a malformed JSON
//! value returns an empty entry rather than an error. The UI handles empty
//! state explicitly.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// One concrete command entry attached to a hook event.
#[derive(Debug, Clone)]
pub struct HookCommand {
    /// `"command"` in practice — Claude Code only ships the one type today.
    pub kind: String,
    /// Shell command, as the user wrote it. Kept verbatim so the UI can
    /// truncate for display without losing the original.
    pub command: String,
}

/// One hook group under a hook-event.
#[derive(Debug, Clone)]
pub struct HookGroup {
    /// Tool-name pattern that triggers the hook. `None` or `"*"` means "any
    /// tool" — we collapse that to a dot glyph at render time.
    pub matcher: Option<String>,
    pub commands: Vec<HookCommand>,
}

/// One row in the hook panel. Flattens groups/commands so the UI can iterate
/// without nested loops — easier to navigate, select, and filter.
#[derive(Debug, Clone)]
pub struct HookRow {
    /// Hook event name (`PreToolUse`, `PostToolUse`, `UserPromptSubmit`, …).
    pub event: String,
    /// `Some` when the user scoped the hook to a specific tool. `*` and empty
    /// both normalise to `None` so the display pipeline has one path.
    pub matcher: Option<String>,
    /// The shell command text.
    pub command: String,
    /// Which config file it was declared in — global vs a project path.
    pub source: HookSource,
}

/// Where a hook or MCP server was declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookSource {
    /// `~/.claude/settings.json`.
    Global,
    /// `~/.claude-code-projects/<encoded>/settings.json` — the stored path is
    /// the real project cwd (decoded best-effort). Kept around for display
    /// labels and the `Enter → filter to project` action.
    Project(PathBuf),
}

impl HookSource {
    /// Short label for rendering — `"global"` or the project basename.
    pub fn label(&self) -> String {
        match self {
            Self::Global => "global".to_string(),
            Self::Project(p) => p
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| p.display().to_string()),
        }
    }
}

/// One MCP server entry from the `mcpServers` map.
#[derive(Debug, Clone)]
pub struct McpServer {
    pub name: String,
    /// Transport name: `"stdio"`, `"sse"`, `"http"`, …. Empty if the user's
    /// config omits it (Claude Code defaults to stdio).
    pub transport: String,
    /// For stdio: the command. For http/sse: the URL. Everything else ends up
    /// empty; the UI falls back to a dim `—` glyph.
    pub connection: String,
    pub source: HookSource,
}

/// Top-level parsed settings file.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub hooks: Vec<HookRow>,
    pub mcp_servers: Vec<McpServer>,
}

impl Settings {
    /// Load the global settings file plus any per-project overrides.
    ///
    /// Returns an empty `Settings` on any error — the UI handles the empty
    /// case, and a single malformed project file shouldn't suppress the rest.
    pub fn load_all() -> Self {
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return Self::default(),
        };
        Self::load_all_from(&home)
    }

    /// Test-friendly entry point: pretend `$HOME` is some other directory so
    /// we can drive the parser against fixtures without touching the user's
    /// real settings.
    pub fn load_all_from(home: &Path) -> Self {
        let mut out = Self::default();

        // 1. Global settings.
        let global_path = home.join(".claude").join("settings.json");
        if let Ok(raw) = fs::read_to_string(&global_path) {
            parse_into(&raw, HookSource::Global, &mut out);
        }

        // 2. Per-project overrides. The projects dir exists independently of
        //    any one project — absent dir is fine, empty entries are fine.
        let projects_root = home.join(".claude-code-projects");
        if let Ok(entries) = fs::read_dir(&projects_root) {
            for e in entries.flatten() {
                let p = e.path();
                if !p.is_dir() {
                    continue;
                }
                let settings_path = p.join("settings.json");
                let Ok(raw) = fs::read_to_string(&settings_path) else {
                    continue;
                };
                // Best-effort decode: leading `-` + remaining `-` → `/`.
                let encoded = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string();
                let real_path = decode_project_dir(&encoded);
                parse_into(&raw, HookSource::Project(real_path), &mut out);
            }
        }

        out
    }

    /// True if nothing configured — useful for empty-state copy in the UI.
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty() && self.mcp_servers.is_empty()
    }
}

// ── JSON shape ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RawSettings {
    #[serde(default)]
    hooks: BTreeMap<String, Vec<RawHookGroup>>,
    #[serde(default, rename = "mcpServers")]
    mcp_servers: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawHookGroup {
    #[serde(default)]
    matcher: Option<String>,
    #[serde(default)]
    hooks: Vec<RawHookCommand>,
}

#[derive(Debug, Deserialize)]
struct RawHookCommand {
    // `type` is always "command" in the wild, but kept in the shape so
    // serde accepts the field (ignoring unknowns would also work, but
    // being explicit documents the schema). The underscore prefix tells
    // the compiler we're OK with not reading it.
    #[serde(rename = "type", default)]
    _kind: Option<String>,
    #[serde(default)]
    command: Option<String>,
}

fn parse_into(raw: &str, source: HookSource, out: &mut Settings) {
    let Ok(parsed) = serde_json::from_str::<RawSettings>(raw) else {
        return;
    };

    // Hooks: flatten event → group → commands into a Vec<HookRow>.
    for (event, groups) in parsed.hooks {
        for g in groups {
            let matcher = g
                .matcher
                .filter(|s| !s.is_empty() && s != "*")
                .map(|s| s.to_string());
            for cmd in g.hooks {
                let Some(command) = cmd.command else { continue };
                out.hooks.push(HookRow {
                    event: event.clone(),
                    matcher: matcher.clone(),
                    command,
                    source: source.clone(),
                });
            }
        }
    }

    // MCP servers.
    for (name, cfg) in parsed.mcp_servers {
        let (transport, connection) = classify_mcp(&cfg);
        out.mcp_servers.push(McpServer {
            name,
            transport,
            connection,
            source: source.clone(),
        });
    }
}

/// Inspect an MCP-server JSON value and pull out a transport label + a
/// single-line connection string. Best-effort — unknown shapes render as
/// `"stdio"` / empty.
fn classify_mcp(v: &serde_json::Value) -> (String, String) {
    let obj = match v.as_object() {
        Some(o) => o,
        None => return (String::from("stdio"), String::new()),
    };

    if let Some(url) = obj.get("url").and_then(|u| u.as_str()) {
        // `type` overrides the transport if explicit; else guess from url.
        let transport = obj
            .get("type")
            .and_then(|t| t.as_str())
            .map(String::from)
            .unwrap_or_else(|| "http".to_string());
        return (transport, url.to_string());
    }

    if let Some(cmd) = obj.get("command").and_then(|c| c.as_str()) {
        let mut line = cmd.to_string();
        if let Some(args) = obj.get("args").and_then(|a| a.as_array()) {
            for a in args.iter().take(3) {
                if let Some(s) = a.as_str() {
                    line.push(' ');
                    line.push_str(s);
                }
            }
            if args.len() > 3 {
                line.push_str(" …");
            }
        }
        let transport = obj
            .get("type")
            .and_then(|t| t.as_str())
            .map(String::from)
            .unwrap_or_else(|| "stdio".to_string());
        return (transport, line);
    }

    (String::from("stdio"), String::new())
}

/// Reverse the encoding scheme used by `~/.claude-code-projects/<encoded>/`.
/// Identical to the `~/.claude/projects/<encoded>/` encoding used elsewhere —
/// leading `-` → `/`, remaining `-` → `/`.
fn decode_project_dir(encoded: &str) -> PathBuf {
    if encoded.is_empty() {
        return PathBuf::new();
    }
    let trimmed = encoded.trim_start_matches('-');
    // Naive decode — ambiguous when the source had underscores, but good
    // enough for labels. The full 3-layer resolver lives in
    // `data::path_resolver`; we don't need the round-trip here.
    let mut out = String::with_capacity(trimmed.len() + 1);
    out.push('/');
    out.push_str(&trimmed.replace('-', "/"));
    PathBuf::from(out)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_hooks_object_with_multiple_events() {
        let raw = r#"{
          "hooks": {
            "PreToolUse": [{
              "matcher": "Bash",
              "hooks": [{"type":"command","command":"~/scripts/audit.sh"}]
            }],
            "UserPromptSubmit": [{
              "hooks": [{"type":"command","command":"~/scripts/log.sh"}]
            }]
          }
        }"#;
        let mut s = Settings::default();
        parse_into(raw, HookSource::Global, &mut s);
        assert_eq!(s.hooks.len(), 2);
        // BTreeMap key order is alphabetical — PreToolUse sorts before UserPromptSubmit.
        assert_eq!(s.hooks[0].event, "PreToolUse");
        assert_eq!(s.hooks[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(s.hooks[0].command, "~/scripts/audit.sh");
        assert_eq!(s.hooks[1].event, "UserPromptSubmit");
        assert!(s.hooks[1].matcher.is_none(), "empty matcher → None");
    }

    #[test]
    fn star_matcher_normalises_to_none() {
        let raw = r#"{"hooks":{"PostToolUse":[
            {"matcher":"*","hooks":[{"type":"command","command":"x"}]}]}}"#;
        let mut s = Settings::default();
        parse_into(raw, HookSource::Global, &mut s);
        assert_eq!(s.hooks.len(), 1);
        assert!(s.hooks[0].matcher.is_none(), "* must collapse to None");
    }

    #[test]
    fn mcp_servers_stdio_and_http() {
        let raw = r#"{
          "mcpServers": {
            "ctx7": {"command":"npx","args":["-y","@ctx/mcp"],"type":"stdio"},
            "firecrawl": {"url":"https://mcp.firecrawl.dev","type":"http"}
          }
        }"#;
        let mut s = Settings::default();
        parse_into(raw, HookSource::Global, &mut s);
        assert_eq!(s.mcp_servers.len(), 2);
        let ctx = s.mcp_servers.iter().find(|m| m.name == "ctx7").unwrap();
        assert_eq!(ctx.transport, "stdio");
        assert!(ctx.connection.starts_with("npx -y"), "stdio → command line");
        let fc = s
            .mcp_servers
            .iter()
            .find(|m| m.name == "firecrawl")
            .unwrap();
        assert_eq!(fc.transport, "http");
        assert_eq!(fc.connection, "https://mcp.firecrawl.dev");
    }

    #[test]
    fn garbage_json_yields_empty() {
        let mut s = Settings::default();
        parse_into("{{{ not valid json", HookSource::Global, &mut s);
        assert!(s.is_empty(), "malformed input must not produce rows");
    }

    #[test]
    fn missing_file_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        // A $HOME with no .claude/ dir is the zero-config case.
        let s = Settings::load_all_from(tmp.path());
        assert!(s.is_empty());
    }

    #[test]
    fn global_and_project_layers_merge() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::write(
            home.join(".claude/settings.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[
                {"type":"command","command":"/bin/g"}]}]}}"#,
        )
        .unwrap();
        // Per-project override dir.
        fs::create_dir_all(home.join(".claude-code-projects").join("-tmp-proj")).unwrap();
        fs::write(
            home.join(".claude-code-projects/-tmp-proj/settings.json"),
            r#"{"hooks":{"PostToolUse":[{"hooks":[
                {"type":"command","command":"/bin/p"}]}]}}"#,
        )
        .unwrap();

        let s = Settings::load_all_from(home);
        assert_eq!(s.hooks.len(), 2);
        assert!(s
            .hooks
            .iter()
            .any(|h| matches!(&h.source, HookSource::Global) && h.command == "/bin/g"));
        assert!(s.hooks.iter().any(|h| matches!(
            &h.source,
            HookSource::Project(p) if p.to_string_lossy().contains("tmp/proj")
        ) && h.command == "/bin/p"));
    }

    #[test]
    fn decode_project_dir_basic() {
        assert_eq!(
            decode_project_dir("-Users-me-work"),
            PathBuf::from("/Users/me/work")
        );
        assert_eq!(decode_project_dir(""), PathBuf::from(""));
    }

    #[test]
    fn hook_source_label() {
        assert_eq!(HookSource::Global.label(), "global");
        assert_eq!(
            HookSource::Project(PathBuf::from("/Users/me/architex")).label(),
            "architex"
        );
    }
}
