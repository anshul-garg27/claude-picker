//! Session type and JSONL loader.
//!
//! A Claude Code session lives in
//! `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl`. Each line is one
//! JSON record: user message, assistant message, custom title, system
//! notice, fork pointer, etc. We stream the file line-by-line (sessions can
//! be tens of megabytes) and aggregate into one [`Session`].
//!
//! The loader is deliberately forgiving: malformed JSON lines log a warning
//! and we keep going. Only the structural invariants we rely on — there is a
//! `type`, there is a `message`, etc. — can make a line useful; anything
//! else is skipped.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::data::pricing::{cost_for, TokenCounts};

/// How the session was launched. Only `Cli`/`SdkCli` are interactive Claude
/// Code sessions the picker cares about; anything else is a non-picker
/// entrypoint (sdk-py, sdk-ts, test harness, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionKind {
    Cli,
    SdkCli,
    Other(String),
}

impl SessionKind {
    fn from_str(s: &str) -> Self {
        match s {
            "cli" => Self::Cli,
            "sdk-cli" => Self::SdkCli,
            other => Self::Other(other.to_string()),
        }
    }

    /// True when this session belongs in the picker.
    pub fn is_picker_visible(&self) -> bool {
        matches!(self, Self::Cli | Self::SdkCli)
    }
}

/// How a session handles tool-use approval. Claude Code logs this in two
/// places: standalone `{"type":"permission-mode", ...}` records that fire
/// whenever the mode switches mid-session, and a `permissionMode` field on
/// each user message. We tally both and pick whichever value is most common.
///
/// `Default` is the built-in "ask each time" mode — not interesting enough
/// to render in the row, so we treat it as "no pill worth drawing" in the
/// UI layer. Every other variant is badged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionMode {
    /// Ask on every tool use. The out-of-the-box default.
    Default,
    /// Auto-approve file edits, still ask for everything else.
    AcceptEdits,
    /// Plan-mode: Claude drafts but does not execute.
    Plan,
    /// `--dangerously-skip-permissions`. Approves everything.
    BypassPermissions,
    /// Never ask. Similar to bypass but kept separate because Claude Code
    /// logs it distinctly in the JSONL.
    DontAsk,
    /// Managed-auto mode.
    Auto,
    /// Any mode we don't recognise. Payload carries the raw string so the
    /// UI can still surface something useful.
    Other(&'static str),
}

impl PermissionMode {
    /// Parse the stringly-typed value Claude Code writes. Unknown values
    /// fall into `Other(&'static str)` by leaking the string via
    /// [`Box::leak`] — the set of possible values is tiny (the CLI ships a
    /// fixed enum), so this never grows unbounded in practice.
    ///
    /// Deliberately not named `from_str` — we don't want `FromStr`'s
    /// fallible signature, and clippy complains if we shadow the trait.
    pub fn parse(s: &str) -> Self {
        match s {
            "default" => Self::Default,
            "acceptEdits" => Self::AcceptEdits,
            "plan" => Self::Plan,
            "bypassPermissions" => Self::BypassPermissions,
            "dontAsk" => Self::DontAsk,
            "auto" => Self::Auto,
            other => Self::Other(Box::leak(other.to_string().into_boxed_str())),
        }
    }

    /// The short label rendered on the pill. `None` means "don't draw a
    /// pill" — we only badge non-default modes.
    pub fn pill_label(self) -> Option<&'static str> {
        match self {
            Self::Default => None,
            Self::AcceptEdits => Some("ACCEPT"),
            Self::Plan => Some("PLAN"),
            Self::BypassPermissions => Some("BYPASS"),
            Self::DontAsk => Some("DONTASK"),
            Self::Auto => Some("AUTO"),
            Self::Other(_) => Some("MODE"),
        }
    }
}

/// A fully-aggregated session. Built once from a `.jsonl` file; cheap to
/// clone and pass around.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    /// Resolved real cwd if we could recover it; otherwise whatever the
    /// caller passed in (usually the encoded directory).
    pub project_dir: PathBuf,
    /// User-provided title from `claude --name "foo"`.
    pub name: Option<String>,
    /// Fallback title derived from the first non-noise user message.
    pub auto_name: Option<String>,
    /// Most recent user prompt the session recorded (parsed from
    /// `{"type":"last-prompt","lastPrompt": ...}` records). Cheaper to
    /// display than `auto_name` because it reflects where the session
    /// *ended up*, not where it started.
    pub last_prompt: Option<String>,
    pub message_count: u32,
    pub tokens: TokenCounts,
    pub total_cost_usd: f64,
    /// Dominant model id across all assistant messages.
    pub model_summary: String,
    pub first_timestamp: Option<DateTime<Utc>>,
    pub last_timestamp: Option<DateTime<Utc>>,
    pub is_fork: bool,
    pub forked_from: Option<String>,
    pub entrypoint: SessionKind,
    /// Dominant permission mode across every `permission-mode` record and
    /// every `permissionMode` field on user messages. `None` if no mode was
    /// ever recorded (older sessions, or CLI versions that predate the
    /// field).
    pub permission_mode: Option<PermissionMode>,
    /// Number of sub-agent transcripts written alongside this session.
    /// Claude Code stores them at
    /// `~/.claude/projects/<encoded>/<sid>/subagents/agent-*.jsonl`; we
    /// stat the directory and count matching files.
    pub subagent_count: u32,
}

impl Session {
    /// Display label used in the picker list. Falls back:
    /// `name` → `last_prompt` → `auto_name` → "unnamed".
    ///
    /// `last_prompt` sits above `auto_name` because it's a better one-line
    /// summary of what the session is about right now — the first user
    /// message is often just "help me with…" where the last one is
    /// concrete.
    pub fn display_label(&self) -> &str {
        self.name
            .as_deref()
            .or(self.last_prompt.as_deref())
            .or(self.auto_name.as_deref())
            .unwrap_or("unnamed")
    }
}

/// Well-known "content is noise" markers used when deriving an auto-name.
///
/// Matches the prefix list in `lib/session-stats.py` plus the preview
/// script's extras so we stay consistent across the Rust rewrite.
pub fn noise_prefixes() -> &'static [&'static str] {
    &[
        "<local-command",
        "<command-name>",
        "<bash-",
        "<system-reminder>",
        "[Request inter",
        "<command-message>",
        "<user-prompt",
    ]
}

/// Raw line shape. Nearly every field is optional — different record types
/// populate different ones.
#[derive(Debug, Deserialize, Default)]
struct RawLine {
    #[serde(default)]
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<RawMessage>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default, rename = "customTitle")]
    custom_title: Option<String>,
    #[serde(default)]
    entrypoint: Option<String>,
    #[serde(default, rename = "forkedFrom")]
    forked_from: Option<ForkInfo>,
    #[serde(default, rename = "parentSessionId")]
    parent_session_id: Option<String>,
    /// On `last-prompt` records — the most recent user prompt, truncated
    /// to ~200 chars by Claude Code itself.
    #[serde(default, rename = "lastPrompt")]
    last_prompt: Option<String>,
    /// Fallback key some versions write instead of `lastPrompt`.
    #[serde(default)]
    text: Option<String>,
    /// On `permission-mode` records — the mode string. Some versions use
    /// `mode` instead of `permissionMode`, so we accept both.
    #[serde(default, rename = "permissionMode")]
    permission_mode: Option<String>,
    #[serde(default)]
    mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ForkInfo {
    Object {
        #[serde(rename = "sessionId")]
        session_id: Option<String>,
    },
    Id(String),
}

#[derive(Debug, Deserialize, Default)]
struct RawMessage {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    content: Option<Content>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

/// Public `Usage` mirror. The per-message usage block is documented in the
/// session format; we re-export it so downstream crates can work with it
/// directly if they ever want to.
#[derive(Debug, Deserialize, Default, Clone, Copy)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    /// Legacy single-bucket cache-creation tokens. Folded into 5m if no
    /// split value is present.
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_creation: CacheCreation,
}

/// Split cache-creation buckets (5-minute and 1-hour ephemeral).
#[derive(Debug, Deserialize, Default, Clone, Copy)]
pub struct CacheCreation {
    #[serde(default)]
    pub ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    pub ephemeral_1h_input_tokens: u64,
}

impl Usage {
    fn into_token_counts(self) -> TokenCounts {
        let cw5 = if self.cache_creation.ephemeral_5m_input_tokens == 0
            && self.cache_creation.ephemeral_1h_input_tokens == 0
        {
            self.cache_creation_input_tokens
        } else {
            self.cache_creation.ephemeral_5m_input_tokens
        };
        let cw1 = self.cache_creation.ephemeral_1h_input_tokens;
        TokenCounts {
            input: self.input_tokens,
            output: self.output_tokens,
            cache_read: self.cache_read_input_tokens,
            cache_write_5m: cw5,
            cache_write_1h: cw1,
        }
    }
}

/// Extract a leading text block from whatever shape the message content takes.
fn first_text(content: &Content) -> String {
    match content {
        Content::Text(s) => s.trim().to_string(),
        Content::Blocks(blocks) => {
            for block in blocks {
                if block.kind.as_deref() == Some("text") {
                    if let Some(t) = &block.text {
                        let trimmed = t.trim();
                        if !trimmed.is_empty() {
                            return trimmed.to_string();
                        }
                    }
                }
            }
            String::new()
        }
    }
}

/// Unicode-safe truncate: take at most `max_chars` *characters*, not bytes.
///
/// Uses `char_indices` so multi-byte emoji or Devanagari glyphs aren't
/// split mid-codepoint.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let mut end = s.len();
    for (i, (idx, _)) in s.char_indices().enumerate() {
        if i == max_chars {
            end = idx;
            break;
        }
    }
    s[..end].to_string()
}

/// Determine whether a candidate auto-name string qualifies.
fn looks_like_real_prompt(text: &str) -> bool {
    if text.chars().count() <= 3 {
        return false;
    }
    for prefix in noise_prefixes() {
        if text.contains(prefix) {
            return false;
        }
    }
    true
}

/// Clean a raw first-user-message into the auto-name form: single line,
/// trimmed, capped at 50 characters.
fn clean_auto_name(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let trimmed = flat.trim();
    truncate_chars(trimmed, 50)
}

/// Parse an RFC 3339 / ISO 8601 timestamp into UTC.
fn parse_ts(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Load a single session JSONL file into a [`Session`].
///
/// Returns `Ok(None)` if the file is not a Claude CLI/SDK-CLI session or is
/// too short to be worth showing (< 2 user+assistant messages, same
/// threshold as the Python implementation). Returns `Err` only for
/// truly unrecoverable I/O failures; malformed JSON lines are logged to
/// stderr and skipped.
pub fn load_session_from_jsonl(
    path: &Path,
    project_dir: PathBuf,
) -> anyhow::Result<Option<Session>> {
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut entrypoint: Option<SessionKind> = None;
    let mut name: Option<String> = None;
    let mut auto_name: Option<String> = None;
    let mut last_prompt: Option<String> = None;
    let mut message_count: u32 = 0;
    let mut tokens = TokenCounts::default();
    let mut total_cost = 0.0_f64;
    let mut model_counts: HashMap<String, u32> = HashMap::new();
    // Each `permission-mode` record or user-level `permissionMode` field
    // bumps the corresponding counter here; the most common wins.
    let mut permission_counts: HashMap<PermissionMode, u32> = HashMap::new();
    let mut first_ts: Option<DateTime<Utc>> = None;
    let mut last_ts: Option<DateTime<Utc>> = None;
    let mut forked_from: Option<String> = None;

    for (line_no, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("{}:{}: read error: {e}", path.display(), line_no + 1);
                continue;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let raw: RawLine = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "{}:{}: skip malformed line: {e}",
                    path.display(),
                    line_no + 1
                );
                continue;
            }
        };

        // Entrypoint detection — first sighting wins. If the session is
        // not a Claude picker one, bail out early to save work.
        if entrypoint.is_none() {
            if let Some(ep) = raw.entrypoint.as_deref() {
                let kind = SessionKind::from_str(ep);
                if !kind.is_picker_visible() {
                    return Ok(None);
                }
                entrypoint = Some(kind);
            }
        }

        // Timestamps — min/max across all lines.
        if let Some(ts_str) = raw.timestamp.as_deref() {
            if let Some(ts) = parse_ts(ts_str) {
                first_ts = Some(first_ts.map_or(ts, |cur| cur.min(ts)));
                last_ts = Some(last_ts.map_or(ts, |cur| cur.max(ts)));
            }
        }

        // Fork pointer.
        if forked_from.is_none() {
            if let Some(info) = raw.forked_from.as_ref() {
                forked_from = match info {
                    ForkInfo::Object { session_id } => session_id.clone(),
                    ForkInfo::Id(s) => Some(s.clone()),
                };
            } else if let Some(p) = raw.parent_session_id.as_deref() {
                forked_from = Some(p.to_string());
            }
        }

        // Custom title wins over whatever auto-name we've found.
        if raw.kind.as_deref() == Some("custom-title") {
            if let Some(t) = raw.custom_title.as_deref() {
                if !t.is_empty() {
                    name = Some(truncate_chars(t, 35));
                }
            }
            continue;
        }

        // `last-prompt` records — later ones overwrite earlier because the
        // file is chronological.
        if raw.kind.as_deref() == Some("last-prompt") {
            let prompt = raw
                .last_prompt
                .as_deref()
                .or(raw.text.as_deref())
                .unwrap_or("")
                .trim();
            if !prompt.is_empty() {
                last_prompt = Some(clean_auto_name(prompt));
            }
            continue;
        }

        // Standalone `permission-mode` records — bump the dominant tally.
        if raw.kind.as_deref() == Some("permission-mode") {
            let mode_raw = raw
                .permission_mode
                .as_deref()
                .or(raw.mode.as_deref())
                .unwrap_or("");
            if !mode_raw.is_empty() {
                let mode = PermissionMode::parse(mode_raw);
                *permission_counts.entry(mode).or_default() += 1;
            }
            continue;
        }

        // User + assistant message bookkeeping.
        let kind = raw.kind.as_deref().unwrap_or("");

        // Embedded `permissionMode` on user records — also votes in the
        // dominant tally. We handle this *before* the role filter below so
        // even non-standard user records contribute.
        if kind == "user" {
            if let Some(mode_raw) = raw.permission_mode.as_deref() {
                if !mode_raw.is_empty() {
                    let mode = PermissionMode::parse(mode_raw);
                    *permission_counts.entry(mode).or_default() += 1;
                }
            }
        }
        if kind == "user" || kind == "assistant" {
            let Some(msg) = raw.message.as_ref() else {
                continue;
            };
            let role = msg.role.as_deref().unwrap_or("");
            if role != "user" && role != "assistant" {
                continue;
            }
            message_count = message_count.saturating_add(1);

            // Per-assistant pricing + model tally.
            if role == "assistant" {
                let model = msg.model.as_deref().unwrap_or("");
                if !model.is_empty() && model != "<synthetic>" {
                    *model_counts.entry(model.to_string()).or_default() += 1;
                }
                if let Some(usage) = msg.usage {
                    let tc = usage.into_token_counts();
                    tokens.add(tc);
                    total_cost += cost_for(model, tc);
                }
            }

            // Auto-name from the first qualifying user message.
            if auto_name.is_none() && kind == "user" {
                if let Some(content) = msg.content.as_ref() {
                    let text = first_text(content);
                    if looks_like_real_prompt(&text) {
                        auto_name = Some(clean_auto_name(&text));
                    }
                }
            }
        }
    }

    // Match Python: drop stubs shorter than 2 messages.
    if message_count < 2 {
        return Ok(None);
    }

    // Default entrypoint to Cli if none was ever recorded — some older
    // sessions lack the field; Python treated them as Claude sessions.
    let entrypoint = entrypoint.unwrap_or(SessionKind::Cli);

    // Dominant model: highest occurrence wins; ties break alphabetically
    // for determinism.
    let model_summary = model_counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(m, _)| m)
        .unwrap_or_default();

    // Dominant permission mode: same "most common wins" rule as the model.
    // Ties broken by a fixed enum ordering so the result is deterministic.
    let permission_mode = permission_counts
        .into_iter()
        .max_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| permission_tie_break_key(b.0).cmp(&permission_tie_break_key(a.0)))
        })
        .map(|(m, _)| m);

    // Subagent count — stat the sibling `<sid>/subagents/` directory.
    let subagent_count = count_subagents(path, &id);

    Ok(Some(Session {
        id,
        project_dir,
        name,
        auto_name,
        last_prompt,
        message_count,
        tokens,
        total_cost_usd: total_cost,
        model_summary,
        first_timestamp: first_ts,
        last_timestamp: last_ts,
        is_fork: forked_from.is_some(),
        forked_from,
        entrypoint,
        permission_mode,
        subagent_count,
    }))
}

/// Stable tie-break ordering for [`PermissionMode`]. Used when two modes
/// have the same occurrence count — higher number wins, which puts the
/// "interesting" modes ahead of `Default`.
fn permission_tie_break_key(mode: PermissionMode) -> u8 {
    match mode {
        PermissionMode::BypassPermissions => 6,
        PermissionMode::Plan => 5,
        PermissionMode::AcceptEdits => 4,
        PermissionMode::DontAsk => 3,
        PermissionMode::Auto => 2,
        PermissionMode::Other(_) => 1,
        PermissionMode::Default => 0,
    }
}

/// Count the `agent-*.jsonl` files under the session's sibling subagent
/// directory, if one exists.
///
/// Claude Code lays these out at
/// `<projects-root>/<encoded>/<sid>/subagents/agent-<hash>.jsonl` for every
/// sub-agent invocation; reading the directory is O(N) in the number of
/// sub-agents (usually ≤ a handful).
pub fn count_subagents(jsonl_path: &Path, session_id: &str) -> u32 {
    let Some(parent) = jsonl_path.parent() else {
        return 0;
    };
    let subagents_dir = parent.join(session_id).join("subagents");
    let Ok(entries) = std::fs::read_dir(&subagents_dir) else {
        return 0;
    };
    let mut count: u32 = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.starts_with("agent-") {
            count = count.saturating_add(1);
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn truncate_respects_char_boundaries() {
        // Four code points, the middle two are multi-byte.
        let s = "a\u{1F600}b\u{0928}c"; // a 😀 b न c
        assert_eq!(truncate_chars(s, 3), "a\u{1F600}b");
    }

    #[test]
    fn clean_auto_name_flattens_newlines_and_caps_chars() {
        let s = "hello\nworld this is a very long prompt that will be truncated eventually";
        let out = clean_auto_name(s);
        assert!(out.chars().count() <= 50);
        assert!(!out.contains('\n'));
    }

    #[test]
    fn noise_prefixes_filter_out_local_commands() {
        assert!(!looks_like_real_prompt("<local-command-stdout>hi"));
        assert!(!looks_like_real_prompt("abc"));
        assert!(looks_like_real_prompt("please help me refactor"));
    }

    #[test]
    fn skips_non_cli_sessions() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"user\",\"entrypoint\":\"sdk-ts\",\"message\":{\"role\":\"user\",\"content\":\"hi\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n",
            ),
        )
        .expect("write");
        let s = load_session_from_jsonl(&jsonl, tmp.path().to_path_buf()).expect("ok");
        assert!(s.is_none(), "sdk-ts session must be filtered out");
    }

    #[test]
    fn drops_stubs_with_too_few_messages() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        fs::write(
            &jsonl,
            "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"hi\"}}\n",
        )
        .expect("write");
        assert!(load_session_from_jsonl(&jsonl, tmp.path().to_path_buf())
            .expect("ok")
            .is_none());
    }

    #[test]
    fn custom_title_beats_auto_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"custom-title\",\"customTitle\":\"my-title\",\"sessionId\":\"x\"}\n",
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"first user prompt\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":10,\"output_tokens\":20}}}\n",
            ),
        )
        .expect("write");
        let s = load_session_from_jsonl(&jsonl, tmp.path().to_path_buf())
            .expect("ok")
            .expect("session");
        assert_eq!(s.name.as_deref(), Some("my-title"));
        assert_eq!(s.auto_name.as_deref(), Some("first user prompt"));
        assert_eq!(s.display_label(), "my-title");
    }

    #[test]
    fn fork_pointer_parsed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"forkedFrom\":{\"sessionId\":\"parent-xyz\"},\"message\":{\"role\":\"user\",\"content\":\"hi\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n",
            ),
        )
        .expect("write");
        let s = load_session_from_jsonl(&jsonl, tmp.path().to_path_buf())
            .expect("ok")
            .expect("session");
        assert!(s.is_fork);
        assert_eq!(s.forked_from.as_deref(), Some("parent-xyz"));
    }

    #[test]
    fn last_prompt_parsed_and_used_for_label() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"first\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n",
                "{\"type\":\"last-prompt\",\"lastPrompt\":\"fix the auth redirect loop\",\"sessionId\":\"x\"}\n",
            ),
        )
        .expect("write");
        let s = load_session_from_jsonl(&jsonl, tmp.path().to_path_buf())
            .expect("ok")
            .expect("session");
        assert_eq!(s.last_prompt.as_deref(), Some("fix the auth redirect loop"));
        // With no explicit name, the fallback chain picks last_prompt over auto_name.
        assert_eq!(s.display_label(), "fix the auth redirect loop");
    }

    #[test]
    fn custom_title_still_beats_last_prompt() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"custom-title\",\"customTitle\":\"named-session\",\"sessionId\":\"x\"}\n",
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"first user prompt\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n",
                "{\"type\":\"last-prompt\",\"lastPrompt\":\"fix bug\",\"sessionId\":\"x\"}\n",
            ),
        )
        .expect("write");
        let s = load_session_from_jsonl(&jsonl, tmp.path().to_path_buf())
            .expect("ok")
            .expect("session");
        assert_eq!(s.display_label(), "named-session");
    }

    #[test]
    fn permission_mode_parsed_from_standalone_and_user_fields() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        // Three bypass votes (two standalone + one user-field) and one plan
        // vote — bypass wins.
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"permission-mode\",\"permissionMode\":\"bypassPermissions\",\"sessionId\":\"x\"}\n",
                "{\"type\":\"permission-mode\",\"permissionMode\":\"bypassPermissions\",\"sessionId\":\"x\"}\n",
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"permissionMode\":\"bypassPermissions\",\"message\":{\"role\":\"user\",\"content\":\"hello\"}}\n",
                "{\"type\":\"permission-mode\",\"permissionMode\":\"plan\",\"sessionId\":\"x\"}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n",
            ),
        )
        .expect("write");
        let s = load_session_from_jsonl(&jsonl, tmp.path().to_path_buf())
            .expect("ok")
            .expect("session");
        assert_eq!(s.permission_mode, Some(PermissionMode::BypassPermissions));
    }

    #[test]
    fn permission_mode_is_none_when_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"hello\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n",
            ),
        )
        .expect("write");
        let s = load_session_from_jsonl(&jsonl, tmp.path().to_path_buf())
            .expect("ok")
            .expect("session");
        assert!(s.permission_mode.is_none());
    }

    #[test]
    fn subagent_count_counts_agent_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"hello\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n",
            ),
        )
        .expect("write");
        // Sibling directory: `<parent>/x/subagents/agent-*.jsonl`.
        let subagents = tmp.path().join("x").join("subagents");
        fs::create_dir_all(&subagents).expect("mkdir subagents");
        fs::write(subagents.join("agent-a.jsonl"), b"").expect("write agent-a");
        fs::write(subagents.join("agent-b.jsonl"), b"").expect("write agent-b");
        // A non-agent file should be ignored.
        fs::write(subagents.join("agent-a.meta.json"), b"{}").expect("write meta");
        // Non-jsonl file should be ignored.
        fs::write(subagents.join("not-an-agent.txt"), b"").expect("write junk");
        let s = load_session_from_jsonl(&jsonl, tmp.path().to_path_buf())
            .expect("ok")
            .expect("session");
        assert_eq!(s.subagent_count, 2);
    }

    #[test]
    fn subagent_count_is_zero_when_dir_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let jsonl = tmp.path().join("x.jsonl");
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{\"role\":\"user\",\"content\":\"hello\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n",
            ),
        )
        .expect("write");
        let s = load_session_from_jsonl(&jsonl, tmp.path().to_path_buf())
            .expect("ok")
            .expect("session");
        assert_eq!(s.subagent_count, 0);
    }

    #[test]
    fn permission_mode_from_str_round_trip() {
        assert_eq!(PermissionMode::parse("default"), PermissionMode::Default);
        assert_eq!(
            PermissionMode::parse("bypassPermissions"),
            PermissionMode::BypassPermissions
        );
        assert_eq!(PermissionMode::parse("plan"), PermissionMode::Plan);
        assert_eq!(
            PermissionMode::parse("acceptEdits"),
            PermissionMode::AcceptEdits
        );
        assert_eq!(PermissionMode::Default.pill_label(), None);
        assert_eq!(
            PermissionMode::BypassPermissions.pill_label(),
            Some("BYPASS")
        );
        assert_eq!(PermissionMode::Plan.pill_label(), Some("PLAN"));
        assert_eq!(PermissionMode::AcceptEdits.pill_label(), Some("ACCEPT"));
    }
}
