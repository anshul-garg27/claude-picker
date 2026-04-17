//! AI-powered session summarisation via the `claude` CLI itself.
//!
//! The public surface is tiny and synchronous: [`summarize_session`] takes a
//! session id, returns a one-sentence summary, and transparently hits a
//! persisted cache on `~/.config/claude-picker/summaries.json` so the UI can
//! show prior answers instantly without paying a second API round trip.
//!
//! Under the hood it shells out to:
//!
//! ```text
//! claude -p "<prompt>" --model claude-haiku-4-5-20251001 --dangerously-skip-permissions
//! ```
//!
//! piping a *truncated* transcript on stdin. Truncation keeps the prompt under
//! ~6 k chars — summaries don't need full context and Haiku tokens aren't free.
//!
//! ## Testing pattern
//!
//! The real `claude` CLI is too expensive to hit in unit tests, so every entry
//! point takes its backend function pointer through a `SummarizeBackend`
//! newtype stored in an `AtomicPtr`. In release builds the default backend is
//! [`default_summarize_backend`], which shells out for real. In tests, the
//! helper [`install_mock_backend`] swaps in a stub that returns a canned
//! string without touching the filesystem or the network.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicPtr, Ordering};

use serde::{Deserialize, Serialize};

use crate::data::transcript::{self, jsonl_path_for_session, ContentItem, TranscriptMessage};

/// Model id used for every summarise / auto-title call. Hard-coded to Haiku
/// 4.5 because it's the cheapest model capable of following the short-output
/// instruction. If a future cheaper model lands, change this one place.
pub const SUMMARIZE_MODEL: &str = "claude-haiku-4-5-20251001";

/// Maximum transcript length fed to the model. Haiku input pricing is
/// $1 / 1 M input tokens ≈ $0.000001 / token ≈ 4 chars → 1.5 k tokens for 6 k
/// chars. Round-trip cost lands near $0.002, matching the toast copy in the
/// spec.
const MAX_TRANSCRIPT_CHARS: usize = 6_000;

/// System prompt for the one-sentence summariser. Anchored so the model
/// doesn't slip into "Here is a summary:" lead-ins.
const SUMMARY_PROMPT: &str =
    "Summarize this Claude Code session in ONE sentence (max 80 chars, no lead-in):";

/// System prompt for the auto-title pass. Lower char budget and "3–5 words"
/// directive to get a kebab-case-friendly label.
const TITLE_PROMPT: &str =
    "Give this Claude Code session a 3-5 word title, lowercase, kebab-case, no quotes:";

/// Approximate per-call cost at Haiku 4.5 rates for the observed prompt
/// sizes. Reported in the success toast so the user always knows what they
/// just spent.
pub const ESTIMATED_COST_USD: f64 = 0.002;

/// Backend signature — the boundary between `summarize_session` and the
/// actual `claude` CLI invocation. `transcript` is the already-truncated
/// content to feed to `claude -p`.
pub type SummarizeBackend = fn(prompt: &str, transcript: &str) -> anyhow::Result<String>;

/// Global backend slot. `ptr::null_mut()` means "use the default" — an
/// indirection through [`backend_fn`] handles the null case so tests can
/// install a mock without worrying about initialisation order.
///
/// We store an `fn` pointer inside a `'static`-lifetime leaked Box. The
/// `AtomicPtr<fn(...)>` dance is because raw function pointers aren't `Sync`
/// directly when generic, but `AtomicPtr` over the address is always fine.
static BACKEND: AtomicPtr<SummarizeBackend> = AtomicPtr::new(std::ptr::null_mut());

/// Swap in a test backend that returns canned strings. Leaks the function
/// pointer on purpose — tests run in-process for the lifetime of the suite.
///
/// Call from a single test-setup site; concurrent tests that each install a
/// mock would race. The existing tests serialise via `#[test]` ordering
/// guaranteed by cargo's per-thread default.
#[doc(hidden)]
pub fn install_mock_backend(mock: SummarizeBackend) {
    let boxed: Box<SummarizeBackend> = Box::new(mock);
    let ptr = Box::into_raw(boxed);
    BACKEND.store(ptr, Ordering::SeqCst);
}

/// Return the currently-installed backend, or the default real one.
fn backend_fn() -> SummarizeBackend {
    let raw = BACKEND.load(Ordering::SeqCst);
    if raw.is_null() {
        default_summarize_backend
    } else {
        // SAFETY: the pointer was produced by `Box::into_raw` on an
        // `SummarizeBackend` and the Box was leaked so it lives forever.
        unsafe { *raw }
    }
}

/// Default backend — shells out to the `claude` CLI. Pipes `transcript` on
/// stdin so arbitrarily long content doesn't blow past shell argv limits.
pub fn default_summarize_backend(prompt: &str, transcript: &str) -> anyhow::Result<String> {
    let mut child = Command::new("claude")
        .arg("-p")
        .arg(prompt)
        .arg("--model")
        .arg(SUMMARIZE_MODEL)
        .arg("--dangerously-skip-permissions")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to spawn claude CLI: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(transcript.as_bytes())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| anyhow::anyhow!("claude CLI failed while running: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "claude CLI exited with {}: {}",
            output.status,
            stderr.trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(clean_output(&stdout))
}

/// Strip the model's lead-in fluff / quotes and cap at 80 chars.
fn clean_output(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    // Drop any "summary:" / "title:" / "here is..." lead-ins the model sometimes slips in
    // despite the prompt telling it not to.
    let lowered = trimmed.to_lowercase();
    let cleaned = if lowered.starts_with("summary:") {
        trimmed[8..].trim()
    } else if lowered.starts_with("title:") {
        trimmed[6..].trim()
    } else {
        trimmed
    };
    // Collapse whitespace + trim. 80-char cap is soft — if the model came back
    // with 82 chars we'd rather show them than drop the final word mid-letter.
    let out: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if out.chars().count() > 110 {
        out.chars().take(110).collect::<String>() + "…"
    } else {
        out
    }
}

/// Location on disk for the persisted summary cache.
pub fn cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| {
        h.join(".config")
            .join("claude-picker")
            .join("summaries.json")
    })
}

/// Persisted cache shape — a plain `{ session_id: summary }` map.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SummaryCache {
    #[serde(default)]
    pub summaries: HashMap<String, String>,
}

impl SummaryCache {
    pub fn load_from(path: &Path) -> Self {
        let Ok(raw) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self)?;
        fs::write(path, body)?;
        Ok(())
    }
}

/// Return the cached summary for `session_id` if one exists.
pub fn load_cached_summary(session_id: &str) -> Option<String> {
    let path = cache_path()?;
    let cache = SummaryCache::load_from(&path);
    cache.summaries.get(session_id).cloned()
}

/// Persist a summary for `session_id`, creating the cache file if it doesn't
/// exist yet.
pub fn save_summary(session_id: &str, summary: &str) -> anyhow::Result<()> {
    let path =
        cache_path().ok_or_else(|| anyhow::anyhow!("no home directory for summary cache"))?;
    let mut cache = SummaryCache::load_from(&path);
    cache
        .summaries
        .insert(session_id.to_string(), summary.to_string());
    cache.save_to(&path)
}

/// Summarise a session. Cache hit returns instantly; cache miss hits the
/// installed backend and persists the result before returning it.
pub fn summarize_session(session_id: &str) -> anyhow::Result<String> {
    if let Some(cached) = load_cached_summary(session_id) {
        return Ok(cached);
    }
    let transcript = load_transcript_content(session_id)?;
    let truncated = truncate_transcript(&transcript, MAX_TRANSCRIPT_CHARS);
    let backend = backend_fn();
    let raw = backend(SUMMARY_PROMPT, &truncated)?;
    let cleaned = clean_output(&raw);
    if cleaned.is_empty() {
        anyhow::bail!("claude CLI returned an empty summary");
    }
    // Best-effort persist — failing to save isn't fatal for the UX; we still
    // have the summary in hand for the toast.
    let _ = save_summary(session_id, &cleaned);
    Ok(cleaned)
}

/// Same machinery as [`summarize_session`] but uses the auto-title prompt.
/// Always re-queries (doesn't consult / write the sentence cache) because
/// the output is a different shape and goes into the JSONL via
/// `session_rename`.
pub fn generate_title(session_id: &str) -> anyhow::Result<String> {
    let transcript = load_transcript_content(session_id)?;
    let truncated = truncate_transcript(&transcript, MAX_TRANSCRIPT_CHARS);
    let backend = backend_fn();
    let raw = backend(TITLE_PROMPT, &truncated)?;
    let cleaned = clean_output(&raw);
    if cleaned.is_empty() {
        anyhow::bail!("claude CLI returned an empty title");
    }
    // Titles go through `session_rename::rename_session`'s 35-char cap, but
    // we still trim here so a chatty model doesn't trip it.
    Ok(cleaned.chars().take(35).collect::<String>())
}

/// Read a session's JSONL and flatten it to a plain-text transcript suitable
/// for feeding a model. Keeps role labels so the model knows whose turn is
/// whose.
fn load_transcript_content(session_id: &str) -> anyhow::Result<String> {
    let Some(path) = jsonl_path_for_session(session_id) else {
        anyhow::bail!("no session file found for id {session_id}");
    };
    let transcript = transcript::load_transcript(&path)?;
    Ok(render_transcript(&transcript))
}

/// Flatten a parsed transcript to a text representation: "user: …" / "claude: …"
/// turns separated by blank lines. Tool calls collapse to
/// `[tool: name input]` so the summariser stays focused on prose.
fn render_transcript(messages: &[TranscriptMessage]) -> String {
    let mut out = String::with_capacity(2048);
    for msg in messages {
        let role = match msg.role {
            transcript::Role::User => "user",
            transcript::Role::Assistant => "claude",
        };
        let mut body = String::new();
        for item in &msg.items {
            match item {
                ContentItem::Text(s) => {
                    if !body.is_empty() {
                        body.push('\n');
                    }
                    body.push_str(s);
                }
                ContentItem::ToolUse { name, .. } => {
                    if !body.is_empty() {
                        body.push('\n');
                    }
                    body.push_str(&format!("[tool: {name}]"));
                }
                ContentItem::ToolResult { content, .. } => {
                    if !body.is_empty() {
                        body.push('\n');
                    }
                    // Tool results can be huge — cap at 120 chars so the
                    // transcript doesn't balloon.
                    let snippet: String = content.chars().take(120).collect();
                    body.push_str(&format!("[tool_result: {snippet}]"));
                }
                ContentItem::Thinking { text } => {
                    if !body.is_empty() {
                        body.push('\n');
                    }
                    body.push_str(&format!("[thinking: {text}]"));
                }
                ContentItem::Other(k) => {
                    if !body.is_empty() {
                        body.push('\n');
                    }
                    body.push_str(&format!("[{k}]"));
                }
            }
        }
        if body.trim().is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(role);
        out.push_str(": ");
        out.push_str(body.trim());
    }
    out
}

/// Cap the transcript at `max_chars` characters. If we truncate, we take
/// from the *tail* — the last messages are usually the most informative for
/// a one-line summary, and the first turn is often just "help me with…".
fn truncate_transcript(transcript: &str, max_chars: usize) -> String {
    let chars: Vec<char> = transcript.chars().collect();
    if chars.len() <= max_chars {
        return transcript.to_string();
    }
    let start = chars.len() - max_chars;
    let tail: String = chars[start..].iter().collect();
    // Re-anchor at a newline so we never begin mid-line.
    if let Some(pos) = tail.find('\n') {
        let anchored = &tail[pos + 1..];
        if !anchored.is_empty() {
            return format!("[truncated]\n{anchored}");
        }
    }
    format!("[truncated]\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// Stub that echoes the prompt's first word so tests can verify the
    /// summary/title distinction without hitting the network.
    fn stub_summarize(prompt: &str, _transcript: &str) -> anyhow::Result<String> {
        if prompt.starts_with("Give this") {
            Ok("auth-refactor".to_string())
        } else {
            Ok("refactored auth middleware to use HttpOnly cookies".to_string())
        }
    }

    /// Tests install the mock backend and mutate the XDG home dir; they
    /// can't run in parallel without racing on those globals. Serialise
    /// through a mutex.
    fn serial_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn clean_output_strips_leadins_and_quotes() {
        assert_eq!(
            clean_output("\"refactored auth middleware\""),
            "refactored auth middleware"
        );
        assert_eq!(
            clean_output("Summary: refactored things"),
            "refactored things"
        );
        assert_eq!(clean_output("title: foo-bar-baz"), "foo-bar-baz");
    }

    #[test]
    fn truncate_transcript_keeps_tail_and_prepends_marker() {
        let s: String = "a\n".repeat(100);
        let out = truncate_transcript(&s, 20);
        assert!(
            out.starts_with("[truncated]"),
            "expected marker, got: {out:?}"
        );
        assert!(out.chars().count() <= 20 + "[truncated]\n".len() + 1);
    }

    #[test]
    fn truncate_noop_when_below_cap() {
        let s = "hello\nworld";
        assert_eq!(truncate_transcript(s, 6_000), s);
    }

    #[test]
    fn render_transcript_flattens_roles_and_tool_uses() {
        use crate::data::transcript::Role;
        let msgs = vec![
            TranscriptMessage {
                role: Role::User,
                timestamp: None,
                items: vec![ContentItem::Text("please fix the bug".into())],
            },
            TranscriptMessage {
                role: Role::Assistant,
                timestamp: None,
                items: vec![
                    ContentItem::Text("on it".into()),
                    ContentItem::ToolUse {
                        name: "Edit".into(),
                        input: serde_json::json!({"file_path":"/x"}),
                    },
                ],
            },
        ];
        let rendered = render_transcript(&msgs);
        assert!(rendered.contains("user: please fix the bug"));
        assert!(rendered.contains("claude: on it"));
        assert!(rendered.contains("[tool: Edit]"));
    }

    #[test]
    fn summary_cache_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("summaries.json");
        let mut cache = SummaryCache::default();
        cache
            .summaries
            .insert("abc".to_string(), "one-line".to_string());
        cache.save_to(&path).expect("save");
        let loaded = SummaryCache::load_from(&path);
        assert_eq!(
            loaded.summaries.get("abc").map(String::as_str),
            Some("one-line")
        );
    }

    #[test]
    fn summarize_session_uses_mock_and_persists() {
        let _g = serial_lock().lock().unwrap();
        // Redirect HOME so the cache writes into a tempdir — keeps tests from
        // polluting the user's real ~/.config.
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());

        // Seed a fake session file.
        let session_id = "ai-summarize-test-session";
        let proj_dir = tmp.path().join(".claude").join("projects").join("proj");
        fs::create_dir_all(&proj_dir).expect("mkdir");
        let jsonl = proj_dir.join(format!("{session_id}.jsonl"));
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"please refactor auth\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":\"ok\"}}\n",
            ),
        )
        .expect("write");

        install_mock_backend(stub_summarize);

        let out = summarize_session(session_id).expect("summary");
        assert!(out.contains("refactored auth"), "got {out}");

        // Persisted?
        let path = cache_path().expect("cache path");
        let cache = SummaryCache::load_from(&path);
        assert_eq!(cache.summaries.get(session_id).cloned(), Some(out.clone()));

        // Cache hit returns the stored value without asking the backend — flip
        // the backend to a panic-on-call stub to prove it.
        fn boom(_p: &str, _t: &str) -> anyhow::Result<String> {
            panic!("should not re-query on cache hit");
        }
        install_mock_backend(boom);
        let again = summarize_session(session_id).expect("cache hit");
        assert_eq!(again, out);

        // Restore HOME.
        if let Some(h) = prev_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
        // Leave the mock pointed at the stub; subsequent tests that call
        // into summarize_session re-install before use.
        install_mock_backend(stub_summarize);
    }

    #[test]
    fn generate_title_uses_title_prompt() {
        let _g = serial_lock().lock().unwrap();
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev_home = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());

        let session_id = "ai-title-test-session";
        let proj_dir = tmp.path().join(".claude").join("projects").join("proj");
        fs::create_dir_all(&proj_dir).expect("mkdir");
        let jsonl = proj_dir.join(format!("{session_id}.jsonl"));
        fs::write(
            &jsonl,
            concat!(
                "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"rework auth\"}}\n",
                "{\"type\":\"assistant\",\"message\":{\"role\":\"assistant\",\"content\":\"ok\"}}\n",
            ),
        )
        .expect("write");

        install_mock_backend(stub_summarize);
        let title = generate_title(session_id).expect("title");
        assert_eq!(title, "auth-refactor");

        if let Some(h) = prev_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
    }
}
