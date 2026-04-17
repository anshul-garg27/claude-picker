//! File-centric pivot — the inverse of the normal session→files mapping.
//!
//! Normal claude-picker answers *"which files did session X touch?"*. This
//! module answers *"which sessions touched file Y?"* — a pivot nobody else
//! in the Claude Code session-manager space does.
//!
//! ## Load pipeline
//!
//! For every `~/.claude/projects/<encoded>/*.jsonl` we scan each line,
//! looking for assistant-message `tool_use` blocks whose `name` is one of
//! the file-touching tools (`Edit`, `Write`, `MultiEdit`, `NotebookEdit`,
//! `Read`). From each one we pull the file path out of `input.file_path`
//! (or `input.notebook_path` for notebooks) and fold it into a
//! [`FileStats`] keyed by absolute path.
//!
//! We deliberately include `Read` with a very small weight — reads don't
//! modify files but they *are* signal that a file mattered to a session,
//! and the edit count is tracked separately so the UI can show "12
//! sessions read, 3 edited" if that ever becomes useful. Today the "N
//! sessions" header just counts distinct sessions.
//!
//! ## Cache
//!
//! A full scan over ~100 heavy users can hit a million lines. To keep the
//! screen snappy we cache the aggregated index to
//! `~/.config/claude-picker/file-index.json`. The cache is considered
//! fresh when:
//!
//!   1. Its own file is less than [`CACHE_TTL`] old, AND
//!   2. No JSONL under `~/.claude/projects/` has an mtime newer than the
//!      cache's own mtime.
//!
//! (2) is the important one — a fresh cache that predates a session edit
//! is still stale. We walk project dirs and compare mtimes; ~100 stat
//! calls is sub-millisecond.
//!
//! On cold launch with no cache, a full scan ships a progress count back
//! to the UI thread via `mpsc`, same pattern as `commands::search_cmd`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How long a cached file-index is considered fresh even if no JSONL has
/// changed. 10 minutes — long enough to dominate the cold→warm launch
/// path, short enough that a user who stepped away doesn't see wildly
/// stale data when they come back.
pub const CACHE_TTL: Duration = Duration::from_secs(10 * 60);

/// Tools that touch files. We look at the `name` on assistant
/// `tool_use` blocks; these names match Claude Code's own tool registry.
const FILE_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit", "Read"];

/// A single file's aggregated activity across every session that touched
/// it. One of these lives in [`FileIndex::files`] keyed by absolute path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStats {
    /// Absolute path as written by the tool call. We don't canonicalise —
    /// sessions running in different worktrees see different real paths,
    /// and canonicalising can hide that.
    pub path: PathBuf,
    /// Number of distinct sessions that ever touched this file.
    pub session_count: u32,
    /// Total number of file-touching tool calls (Edit + Write + MultiEdit +
    /// NotebookEdit + Read) across every session.
    pub edit_count: u32,
    /// Best-effort "lines added" — `Write` contributes its full content
    /// line count; `Edit`/`MultiEdit` count the `new_string` line count;
    /// `NotebookEdit` counts the `new_source` line count. `Read` is 0.
    pub total_lines_added: u32,
    /// Mirror for `total_lines_added` — `Edit`'s `old_string` line count
    /// and `MultiEdit`'s per-edit `old_string` line count.
    pub total_lines_removed: u32,
    /// The most recent tool call that touched this file.
    pub last_touched: DateTime<Utc>,
    /// Per-session rollups — one row per session that touched this file.
    pub sessions: Vec<SessionRef>,
    /// The project this file most recently belonged to. Inferred from the
    /// session that last touched it. Used to filter the file list by
    /// project name.
    pub project_name: String,
}

/// One session's contribution to a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRef {
    pub session_id: String,
    pub session_name: String,
    pub project_cwd: PathBuf,
    pub edits_in_this_session: u32,
    pub lines_added: u32,
    pub lines_removed: u32,
    pub last_edit_in_session: DateTime<Utc>,
    /// Total session cost (USD). Populated from `load_session_from_jsonl`
    /// when available — useful for the right-pane "$4.20" tag next to
    /// each session row.
    #[serde(default)]
    pub session_cost_usd: f64,
}

/// Top-level index the UI consumes. Immutable once built.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileIndex {
    /// One entry per distinct file path. Flat on the wire so we can
    /// serialize cleanly; the UI sorts / filters at render time.
    pub files: Vec<FileStats>,
    /// Number of sessions scanned. Shown in the header as
    /// "N sessions".
    pub session_total: u32,
    /// When this index was generated.
    pub built_at: DateTime<Utc>,
}

impl FileIndex {
    /// Build an index by scanning every JSONL under
    /// `~/.claude/projects/`. `progress` fires once per session scanned
    /// with the running count; the UI uses this to animate a
    /// "Scanning sessions… N found" placeholder. Call this off the UI
    /// thread — it is CPU-bound and touches the disk.
    ///
    /// `project_filter`, when `Some`, restricts the scan to the named
    /// project (matched by `Project::name`). Cuts work proportionally on
    /// users with many projects.
    pub fn build(
        project_filter: Option<&str>,
        mut progress: impl FnMut(u32),
    ) -> anyhow::Result<Self> {
        use crate::data::path_resolver::{load_session_metadata, resolve};
        use crate::data::session::load_session_from_jsonl;

        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        let projects_root = home.join(".claude").join("projects");
        let sessions_meta_dir = home.join(".claude").join("sessions");
        let meta = load_session_metadata(&sessions_meta_dir);

        let mut files: HashMap<PathBuf, FileStats> = HashMap::new();
        let mut session_total: u32 = 0;

        let Ok(project_entries) = fs::read_dir(&projects_root) else {
            return Ok(Self {
                files: Vec::new(),
                session_total: 0,
                built_at: Utc::now(),
            });
        };

        for entry in project_entries.flatten() {
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }
            let encoded = entry.file_name().to_string_lossy().into_owned();
            let resolved_cwd =
                resolve(&encoded, &meta, &projects_root).unwrap_or_else(|| PathBuf::from(&encoded));
            let project_name = resolved_cwd
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| encoded.rsplit('-').next().unwrap_or(&encoded).to_string());

            if let Some(filter) = project_filter {
                if project_name != filter {
                    continue;
                }
            }

            let Ok(session_entries) = fs::read_dir(&project_dir) else {
                continue;
            };
            for sess_entry in session_entries.flatten() {
                let path = sess_entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }

                // Pull metadata (id, name, cost) from the standard loader.
                // It may filter out the session entirely (non-CLI /
                // stub) — we still honour that filter so the file list
                // agrees with every other screen's notion of "a session".
                let session = match load_session_from_jsonl(&path, resolved_cwd.clone()) {
                    Ok(Some(s)) => s,
                    _ => continue,
                };
                session_total = session_total.saturating_add(1);
                progress(session_total);

                accumulate_file_calls(
                    &path,
                    &session.id,
                    session.display_label(),
                    &resolved_cwd,
                    &project_name,
                    session.total_cost_usd,
                    &mut files,
                );
            }
        }

        let mut flat: Vec<FileStats> = files.into_values().collect();
        // Stable default order: most-edited first. The UI may re-sort.
        flat.sort_by(|a, b| {
            b.edit_count
                .cmp(&a.edit_count)
                .then_with(|| b.last_touched.cmp(&a.last_touched))
        });

        Ok(Self {
            files: flat,
            session_total,
            built_at: Utc::now(),
        })
    }

    /// Apply the standard junk-path filter. Mutates in place so callers
    /// that want the full set can skip this.
    pub fn filter_junk(&mut self) {
        self.files.retain(|f| !is_junk_path(&f.path));
    }

    /// True if the named file has a session that touched it since `since`.
    /// Used by the example query "which sessions touched package.json in
    /// the last week?" in integration tests.
    pub fn sessions_touching_since(
        &self,
        file_basename: &str,
        since: DateTime<Utc>,
    ) -> Vec<&SessionRef> {
        let mut out = Vec::new();
        for f in &self.files {
            if f.path
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s == file_basename)
                .unwrap_or(false)
            {
                for s in &f.sessions {
                    if s.last_edit_in_session >= since {
                        out.push(s);
                    }
                }
            }
        }
        out
    }

    /// Default cache path under `~/.config/claude-picker/`.
    pub fn default_cache_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| {
            h.join(".config")
                .join("claude-picker")
                .join("file-index.json")
        })
    }

    /// Load a cached index from disk. Returns `None` when the cache file
    /// is missing, unreadable, malformed, or would require a rescan
    /// (TTL expired or a JSONL is newer than the cache).
    ///
    /// This is a cheap check — ~100 stats on a heavy user — so the UI
    /// can call it on the hot launch path before dispatching the real
    /// scan.
    pub fn load_cached(path: &Path) -> Option<Self> {
        let meta = fs::metadata(path).ok()?;
        let cache_mtime = meta.modified().ok()?;
        if cache_mtime.elapsed().unwrap_or(Duration::MAX) > CACHE_TTL {
            return None;
        }
        // Any JSONL newer than the cache mtime invalidates. We only
        // need the max; bail on the first hit.
        if projects_have_newer_jsonl_than(cache_mtime) {
            return None;
        }
        let raw = fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Write this index to `path` (creating parent directories as
    /// needed). Errors are returned but non-fatal — cache miss on the
    /// next run is harmless.
    pub fn save_cache(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string(self)?;
        fs::write(path, raw)?;
        Ok(())
    }
}

/// Scan one session's JSONL, fold every file-touching tool call into
/// `files`. Mutates in place so callers can accumulate across many
/// sessions.
fn accumulate_file_calls(
    jsonl_path: &Path,
    session_id: &str,
    session_name: &str,
    project_cwd: &Path,
    project_name: &str,
    session_cost_usd: f64,
    files: &mut HashMap<PathBuf, FileStats>,
) {
    let Ok(file) = fs::File::open(jsonl_path) else {
        return;
    };
    let reader = BufReader::new(file);

    // Per-session rollup: path -> (edits, lines_added, lines_removed, last_edit).
    // We build this first, then fold into the global map at the end so one
    // session shows up once per file.
    type Rollup = (u32, u32, u32, Option<DateTime<Utc>>);
    let mut per_session: HashMap<PathBuf, Rollup> = HashMap::new();

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(raw): Result<Value, _> = serde_json::from_str(trimmed) else {
            continue;
        };
        // We only look at assistant messages — user messages don't call tools.
        if raw.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }
        let ts = raw
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(parse_ts);
        let Some(content) = raw.pointer("/message/content").and_then(|v| v.as_array()) else {
            continue;
        };
        for block in content {
            let Some(obj) = block.as_object() else {
                continue;
            };
            if obj.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                continue;
            }
            let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if !FILE_TOOLS.contains(&name) {
                continue;
            }
            let Some(input) = obj.get("input").and_then(|v| v.as_object()) else {
                continue;
            };

            // Collect path(s). MultiEdit carries a top-level `file_path`;
            // NotebookEdit uses `notebook_path`.
            let maybe_path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .or_else(|| input.get("notebook_path").and_then(|v| v.as_str()));
            let Some(fp_str) = maybe_path else {
                continue;
            };
            let fp = PathBuf::from(fp_str);

            let (added, removed) = line_delta(name, input);
            let entry = per_session.entry(fp).or_insert((0, 0, 0, None));
            entry.0 = entry.0.saturating_add(1);
            entry.1 = entry.1.saturating_add(added);
            entry.2 = entry.2.saturating_add(removed);
            if let Some(t) = ts {
                entry.3 = Some(entry.3.map_or(t, |cur| cur.max(t)));
            }
        }
    }

    // Fold per-session rollup into the global map.
    let fallback_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0))
        .unwrap_or_else(Utc::now);

    for (path, (edits, added, removed, last_ts)) in per_session {
        let last = last_ts.unwrap_or(fallback_ts);

        let entry = files.entry(path.clone()).or_insert_with(|| FileStats {
            path: path.clone(),
            session_count: 0,
            edit_count: 0,
            total_lines_added: 0,
            total_lines_removed: 0,
            last_touched: last,
            sessions: Vec::new(),
            project_name: project_name.to_string(),
        });

        entry.session_count = entry.session_count.saturating_add(1);
        entry.edit_count = entry.edit_count.saturating_add(edits);
        entry.total_lines_added = entry.total_lines_added.saturating_add(added);
        entry.total_lines_removed = entry.total_lines_removed.saturating_add(removed);
        if last > entry.last_touched {
            entry.last_touched = last;
            // The "project_name" shown in the list follows whichever
            // session last touched the file. This is what a user expects:
            // if I moved a file, the list should show its current home.
            entry.project_name = project_name.to_string();
        }
        entry.sessions.push(SessionRef {
            session_id: session_id.to_string(),
            session_name: session_name.to_string(),
            project_cwd: project_cwd.to_path_buf(),
            edits_in_this_session: edits,
            lines_added: added,
            lines_removed: removed,
            last_edit_in_session: last,
            session_cost_usd,
        });
        // Keep the per-file session list in most-recent-first order.
        entry
            .sessions
            .sort_by_key(|s| std::cmp::Reverse(s.last_edit_in_session));
    }
}

/// Count newlines in a string + 1 if non-empty. 0 for an empty string.
/// Matches the intuitive "how many lines does this replace?" metric.
fn count_lines(s: &str) -> u32 {
    if s.is_empty() {
        return 0;
    }
    (s.matches('\n').count() as u32).saturating_add(1)
}

/// Best-effort (lines_added, lines_removed) per tool invocation.
///
/// - `Write`: everything in `content` is new; nothing removed.
/// - `Edit`: `new_string` added, `old_string` removed.
/// - `MultiEdit`: sum across the `edits` array.
/// - `NotebookEdit`: `new_source` added; nothing attributed as removed
///   (notebooks don't carry an old source on insert; on replace we'd
///   need the cell body, which we don't have).
/// - `Read`: 0 / 0. Read doesn't change files.
fn line_delta(tool: &str, input: &serde_json::Map<String, Value>) -> (u32, u32) {
    match tool {
        "Write" => {
            let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
            (count_lines(content), 0)
        }
        "Edit" => {
            let new_s = input
                .get("new_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let old_s = input
                .get("old_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (count_lines(new_s), count_lines(old_s))
        }
        "MultiEdit" => {
            let mut a = 0u32;
            let mut r = 0u32;
            if let Some(edits) = input.get("edits").and_then(|v| v.as_array()) {
                for e in edits {
                    let Some(o) = e.as_object() else { continue };
                    let new_s = o.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
                    let old_s = o.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
                    a = a.saturating_add(count_lines(new_s));
                    r = r.saturating_add(count_lines(old_s));
                }
            }
            (a, r)
        }
        "NotebookEdit" => {
            let src = input
                .get("new_source")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            (count_lines(src), 0)
        }
        _ => (0, 0),
    }
}

fn parse_ts(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// The default junk-path filter. Returns true when the file should be
/// hidden from the list.
///
/// We look for common noise directories as *segments* — that way
/// `/Users/.../my-project/node_modules/...` matches but a repo that
/// happens to be *named* `node_modules` wouldn't.
pub fn is_junk_path(path: &Path) -> bool {
    const JUNK_SEGMENTS: &[&str] = &[
        ".git",
        "node_modules",
        "target",
        "__pycache__",
        ".venv",
        "venv",
        "dist",
        "build",
        ".next",
        ".nuxt",
        ".cache",
    ];
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if file_name == ".DS_Store" {
        return true;
    }
    for seg in path.components() {
        let s = seg.as_os_str().to_string_lossy();
        if JUNK_SEGMENTS.iter().any(|j| *j == s) {
            return true;
        }
    }
    false
}

/// Walk every `~/.claude/projects/<encoded>/*.jsonl` and return true as
/// soon as we find one with an mtime strictly newer than `threshold`.
/// Bails on the first hit so it's cheap on invalidation.
fn projects_have_newer_jsonl_than(threshold: SystemTime) -> bool {
    let Some(home) = dirs::home_dir() else {
        return true;
    };
    let root = home.join(".claude").join("projects");
    let Ok(iter) = fs::read_dir(&root) else {
        return false;
    };
    for proj in iter.flatten() {
        if !proj.path().is_dir() {
            continue;
        }
        let Ok(files) = fs::read_dir(proj.path()) else {
            continue;
        };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(meta) = f.metadata() else { continue };
            let Ok(m) = meta.modified() else { continue };
            if m > threshold {
                return true;
            }
        }
    }
    false
}

/// Testing helper — used by integration tests + the CLI `--files --project`
/// path to de-dupe session counts at a glance.
pub fn distinct_session_count(stats: &FileStats) -> usize {
    let ids: HashSet<&str> = stats
        .sessions
        .iter()
        .map(|s| s.session_id.as_str())
        .collect();
    ids.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_lines_matches_intuition() {
        assert_eq!(count_lines(""), 0);
        assert_eq!(count_lines("one"), 1);
        assert_eq!(count_lines("a\nb"), 2);
        assert_eq!(count_lines("a\nb\nc\n"), 4); // trailing newline counts the final empty line
    }

    #[test]
    fn line_delta_edit() {
        let mut m = serde_json::Map::new();
        m.insert("old_string".into(), Value::String("a\nb".into()));
        m.insert("new_string".into(), Value::String("a\nb\nc".into()));
        assert_eq!(line_delta("Edit", &m), (3, 2));
    }

    #[test]
    fn line_delta_write_and_read() {
        let mut m = serde_json::Map::new();
        m.insert("content".into(), Value::String("a\nb\nc".into()));
        assert_eq!(line_delta("Write", &m), (3, 0));

        let empty = serde_json::Map::new();
        assert_eq!(line_delta("Read", &empty), (0, 0));
    }

    #[test]
    fn line_delta_multi_edit_sums() {
        use serde_json::json;
        let v = json!({
            "file_path": "/tmp/x.rs",
            "edits": [
                {"old_string": "a", "new_string": "aa\nbb"},
                {"old_string": "c\nd", "new_string": "e"},
            ]
        });
        let m = v.as_object().unwrap();
        assert_eq!(line_delta("MultiEdit", m), (2 + 1, 1 + 2));
    }

    #[test]
    fn line_delta_notebook() {
        let mut m = serde_json::Map::new();
        m.insert(
            "new_source".into(),
            Value::String("import os\nprint('hi')".into()),
        );
        assert_eq!(line_delta("NotebookEdit", &m), (2, 0));
    }

    #[test]
    fn is_junk_filters_expected_paths() {
        assert!(is_junk_path(Path::new("/a/b/node_modules/c.js")));
        assert!(is_junk_path(Path::new("/a/b/.git/HEAD")));
        assert!(is_junk_path(Path::new("/a/b/target/debug/app")));
        assert!(is_junk_path(Path::new("/a/b/__pycache__/x.pyc")));
        assert!(is_junk_path(Path::new("/a/b/.DS_Store")));
        assert!(!is_junk_path(Path::new("/a/b/src/main.rs")));
        // Similar-looking but distinct names don't match.
        assert!(!is_junk_path(Path::new("/a/nodemodule/x.js")));
    }

    // Requires exclusive access to `$HOME`, which conflicts with any other
    // test that mutates it. The proper coverage lives in
    // `tests/file_index_tests.rs` where a shared mutex is available; this
    // unit-test version is only safe to run by itself (`cargo test --lib
    // -- --test-threads=1 build_aggregates_across_two_sessions`), so we
    // gate it behind `#[ignore]`.
    #[test]
    #[ignore = "race: mutates HOME; use tests/file_index_tests.rs instead"]
    fn build_aggregates_across_two_sessions() {
        use std::fs as sfs;
        use tempfile::tempdir;

        let tmp = tempdir().expect("tempdir");
        let home = tmp.path();
        // Build the directory shape `build` expects.
        std::env::set_var("HOME", home);
        let projects = home.join(".claude").join("projects");
        sfs::create_dir_all(&projects).expect("mkdir projects");
        let proj_dir = projects.join("-tmp-demo-project");
        sfs::create_dir_all(&proj_dir).expect("mkdir proj");

        let session_a = proj_dir.join("aaaa.jsonl");
        let session_b = proj_dir.join("bbbb.jsonl");
        // Each session has a user msg, an assistant msg with a tool_use
        // Edit on /tmp/file.rs, and enough messages to pass the 2-msg gate.
        let mk = |sid: &str, new_s: &str| {
            format!(
                concat!(
                    "{{\"type\":\"user\",\"entrypoint\":\"cli\",\"message\":{{\"role\":\"user\",\"content\":\"hi\"}},\"sessionId\":\"{sid}\"}}\n",
                    "{{\"type\":\"assistant\",\"timestamp\":\"2026-04-16T10:00:00Z\",\"message\":{{\"role\":\"assistant\",\"model\":\"claude-opus-4-7\",\"content\":[{{\"type\":\"tool_use\",\"name\":\"Edit\",\"id\":\"x\",\"input\":{{\"file_path\":\"/tmp/file.rs\",\"old_string\":\"a\",\"new_string\":\"{new}\"}}}}],\"usage\":{{\"input_tokens\":1,\"output_tokens\":1}}}}}}\n"
                ),
                sid = sid,
                new = new_s
            )
        };
        sfs::write(&session_a, mk("aaaa", "one line")).expect("write a");
        sfs::write(&session_b, mk("bbbb", "one\\ntwo")).expect("write b");

        let idx = FileIndex::build(None, |_| {}).expect("build");
        assert_eq!(idx.session_total, 2);
        // One file, touched by two sessions.
        let f = idx
            .files
            .iter()
            .find(|f| f.path == Path::new("/tmp/file.rs"))
            .expect("file present");
        assert_eq!(f.session_count, 2);
        assert_eq!(f.edit_count, 2);
        assert_eq!(f.sessions.len(), 2);
        assert_eq!(distinct_session_count(f), 2);
    }

    #[test]
    fn cache_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("cache.json");
        let idx = FileIndex {
            files: vec![FileStats {
                path: PathBuf::from("/tmp/x.rs"),
                session_count: 3,
                edit_count: 12,
                total_lines_added: 100,
                total_lines_removed: 40,
                last_touched: Utc::now(),
                sessions: vec![],
                project_name: "demo".into(),
            }],
            session_total: 1,
            built_at: Utc::now(),
        };
        idx.save_cache(&path).expect("save");
        // We don't check freshness here — a synthesised cache doesn't
        // round-trip the mtime check. Load the raw file directly.
        let raw = fs::read_to_string(&path).expect("read");
        let loaded: FileIndex = serde_json::from_str(&raw).expect("parse");
        assert_eq!(loaded.files.len(), 1);
        assert_eq!(loaded.files[0].path, PathBuf::from("/tmp/x.rs"));
        assert_eq!(loaded.files[0].edit_count, 12);
    }

    #[test]
    fn sessions_touching_since_finds_recent_only() {
        let now = Utc::now();
        let week_ago = now - chrono::Duration::days(7);
        let two_weeks = now - chrono::Duration::days(14);
        let idx = FileIndex {
            files: vec![FileStats {
                path: PathBuf::from("/a/b/package.json"),
                session_count: 2,
                edit_count: 10,
                total_lines_added: 0,
                total_lines_removed: 0,
                last_touched: now,
                sessions: vec![
                    SessionRef {
                        session_id: "recent".into(),
                        session_name: "recent work".into(),
                        project_cwd: PathBuf::from("/a/b"),
                        edits_in_this_session: 5,
                        lines_added: 0,
                        lines_removed: 0,
                        last_edit_in_session: now,
                        session_cost_usd: 0.0,
                    },
                    SessionRef {
                        session_id: "stale".into(),
                        session_name: "old work".into(),
                        project_cwd: PathBuf::from("/a/b"),
                        edits_in_this_session: 5,
                        lines_added: 0,
                        lines_removed: 0,
                        last_edit_in_session: two_weeks,
                        session_cost_usd: 0.0,
                    },
                ],
                project_name: "b".into(),
            }],
            session_total: 2,
            built_at: now,
        };
        let hits = idx.sessions_touching_since("package.json", week_ago);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session_id, "recent");
    }
}
