//! Fast-path reader for `~/.claude.json`.
//!
//! Claude Code stamps a compact per-project summary into `~/.claude.json`
//! at the end of every session. Each `projects[<cwd>]` entry carries the
//! running cost, lines added/removed, token split, and per-model usage for
//! the *last* session run under that cwd — everything the `/stats`
//! dashboard needs, cached for free.
//!
//! We read this file on startup and use it as a fast fallback so the
//! dashboard can launch in under 100ms on datasets that would otherwise
//! need re-parsing every JSONL. When the cached `lastSessionId` matches
//! the most recent session we find on disk for that project, we trust
//! the cache entirely; when it doesn't, we fall back to a JSONL scan.
//!
//! The file shape we care about:
//!
//! ```json
//! {
//!   "projects": {
//!     "/path/to/cwd": {
//!       "lastCost": 1.23,
//!       "lastLinesAdded": 45,
//!       "lastLinesRemoved": 12,
//!       "lastTotalInputTokens": 1000,
//!       "lastTotalOutputTokens": 400,
//!       "lastTotalCacheCreationInputTokens": 500,
//!       "lastTotalCacheReadInputTokens": 200,
//!       "lastSessionId": "uuid",
//!       "lastModelUsage": { "claude-opus-4-7": { ... } }
//!     }
//!   }
//! }
//! ```

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// One project's cached snapshot. Every numeric field defaults to zero so a
/// partially-populated entry still deserialises — older CLI versions
/// sometimes lacked individual `last*` keys.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectCache {
    /// Dollar cost of the last session run for this project.
    #[serde(default, rename = "lastCost")]
    pub last_cost: f64,
    #[serde(default, rename = "lastLinesAdded")]
    pub last_lines_added: u64,
    #[serde(default, rename = "lastLinesRemoved")]
    pub last_lines_removed: u64,
    #[serde(default, rename = "lastTotalInputTokens")]
    pub last_total_input_tokens: u64,
    #[serde(default, rename = "lastTotalOutputTokens")]
    pub last_total_output_tokens: u64,
    #[serde(default, rename = "lastTotalCacheCreationInputTokens")]
    pub last_total_cache_creation_input_tokens: u64,
    #[serde(default, rename = "lastTotalCacheReadInputTokens")]
    pub last_total_cache_read_input_tokens: u64,
    /// The session id `lastCost` belongs to. We use this to decide whether
    /// the cache is still "fresh" against what we see on disk.
    #[serde(default, rename = "lastSessionId")]
    pub last_session_id: Option<String>,
    /// Per-model cost/token breakdown. Opaque shape — different CLI
    /// versions emit different keys — so we keep it as raw JSON.
    #[serde(default, rename = "lastModelUsage")]
    pub last_model_usage: serde_json::Value,
}

impl ProjectCache {
    /// Sum of every `last*Tokens` field.
    pub fn total_tokens(&self) -> u64 {
        self.last_total_input_tokens
            .saturating_add(self.last_total_output_tokens)
            .saturating_add(self.last_total_cache_creation_input_tokens)
            .saturating_add(self.last_total_cache_read_input_tokens)
    }
}

/// Top-level `~/.claude.json` view — we only deserialise the `projects`
/// dict. Everything else (`numStartups`, Statsig caches, tips history, …)
/// is ignored.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClaudeJsonCache {
    #[serde(default)]
    pub projects: HashMap<PathBuf, ProjectCache>,
}

impl ClaudeJsonCache {
    /// Load the cache from `~/.claude.json`, returning an empty cache if
    /// the file is missing or malformed. Never errors — the caller should
    /// always get *some* cache back so the cache-trust logic stays simple.
    pub fn load() -> Self {
        let Some(home) = dirs::home_dir() else {
            return Self::default();
        };
        Self::load_from(&home.join(".claude.json"))
    }

    /// Injection-friendly variant of [`load`](Self::load).
    pub fn load_from(path: &Path) -> Self {
        let Ok(file) = File::open(path) else {
            return Self::default();
        };
        let reader = BufReader::new(file);
        serde_json::from_reader::<_, Self>(reader).unwrap_or_default()
    }

    /// Look up a project's cached entry by its resolved cwd.
    pub fn for_project(&self, cwd: &Path) -> Option<&ProjectCache> {
        self.projects.get(cwd)
    }

    /// Is the cached entry for `cwd` still trustworthy? Trust requires the
    /// entry to exist AND for `lastSessionId` to match `on_disk_latest`
    /// (the most recent session id we see in the JSONL directory). A
    /// mismatch means the user ran a session we don't have cached stats
    /// for, so the caller should full-scan for this project.
    pub fn is_fresh_for(&self, cwd: &Path, on_disk_latest: Option<&str>) -> bool {
        let Some(entry) = self.for_project(cwd) else {
            return false;
        };
        let Some(cached) = entry.last_session_id.as_deref() else {
            return false;
        };
        on_disk_latest == Some(cached)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_returns_empty_cache() {
        let cache = ClaudeJsonCache::load_from(Path::new("/nonexistent/claude.json"));
        assert!(cache.projects.is_empty());
    }

    #[test]
    fn malformed_file_returns_empty_cache() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("claude.json");
        std::fs::write(&path, b"{not valid json").expect("write");
        let cache = ClaudeJsonCache::load_from(&path);
        assert!(cache.projects.is_empty());
    }

    #[test]
    fn parses_per_project_fields() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("claude.json");
        std::fs::write(
            &path,
            r#"{
              "numStartups": 7,
              "projects": {
                "/Users/me/foo": {
                  "lastCost": 1.23,
                  "lastLinesAdded": 45,
                  "lastLinesRemoved": 12,
                  "lastTotalInputTokens": 1000,
                  "lastTotalOutputTokens": 400,
                  "lastTotalCacheCreationInputTokens": 500,
                  "lastTotalCacheReadInputTokens": 200,
                  "lastSessionId": "sid-abc",
                  "lastModelUsage": {"claude-opus-4-7": {"tokens": 100}}
                }
              }
            }"#,
        )
        .expect("write");

        let cache = ClaudeJsonCache::load_from(&path);
        let entry = cache
            .for_project(Path::new("/Users/me/foo"))
            .expect("entry");
        assert!((entry.last_cost - 1.23).abs() < 1e-9);
        assert_eq!(entry.last_lines_added, 45);
        assert_eq!(entry.last_lines_removed, 12);
        assert_eq!(entry.total_tokens(), 1000 + 400 + 500 + 200);
        assert_eq!(entry.last_session_id.as_deref(), Some("sid-abc"));
        assert_eq!(
            entry
                .last_model_usage
                .get("claude-opus-4-7")
                .and_then(|v| v.get("tokens"))
                .and_then(|v| v.as_u64()),
            Some(100)
        );
    }

    #[test]
    fn freshness_matches_on_disk_session_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("claude.json");
        std::fs::write(
            &path,
            r#"{"projects":{"/x":{"lastSessionId":"sid-1","lastCost":0.5}}}"#,
        )
        .expect("write");
        let cache = ClaudeJsonCache::load_from(&path);

        // Match → trusted.
        assert!(cache.is_fresh_for(Path::new("/x"), Some("sid-1")));
        // Mismatch → stale.
        assert!(!cache.is_fresh_for(Path::new("/x"), Some("sid-2")));
        // Unknown project → not trusted.
        assert!(!cache.is_fresh_for(Path::new("/y"), Some("sid-1")));
        // No on-disk latest → not trusted.
        assert!(!cache.is_fresh_for(Path::new("/x"), None));
    }

    #[test]
    fn defaults_to_zero_when_fields_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("claude.json");
        std::fs::write(&path, r#"{"projects":{"/empty":{}}}"#).expect("write");
        let cache = ClaudeJsonCache::load_from(&path);
        let entry = cache.for_project(Path::new("/empty")).expect("entry");
        assert!(entry.last_cost.abs() < 1e-9);
        assert_eq!(entry.total_tokens(), 0);
        assert!(entry.last_session_id.is_none());
    }
}
