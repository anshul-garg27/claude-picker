//! Lossy path encoding reverser.
//!
//! Claude Code names its per-project session directories by replacing every
//! `/` and `_` with `-`. This is ambiguous: `/my_dir` and `/my/dir` both
//! become `-my-dir`. We recover the true path through a three-layer
//! resolver, ported from `lib/session-stats.py`:
//!
//! 1. **Metadata lookup.** For any session whose `.jsonl` carries an id we
//!    have metadata for in `~/.claude/sessions/*.json`, the saved `cwd` is
//!    authoritative.
//! 2. **JSONL cwd scan.** Otherwise, stream a few lines of any `.jsonl` in
//!    the directory until one carries a `"cwd"` field, which is equally
//!    authoritative.
//! 3. **Naive decode fallback.** Replace leading `-` with `/` and the rest
//!    with `/` best-effort. Lossy but correct for the common case where no
//!    underscores appear in the path.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Minimal shape of `~/.claude/sessions/<sid>.json` — we only need two fields.
#[derive(Deserialize)]
struct SessionMeta {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    cwd: Option<String>,
}

/// Scan `~/.claude/sessions/*.json` and return the cwd-by-session-id map.
///
/// Silently skips files that fail to open or parse — metadata is an
/// optimisation, not a hard requirement.
pub fn load_session_metadata(sessions_dir: &Path) -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();
    let Ok(entries) = fs::read_dir(sessions_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(file) = File::open(&path) else {
            continue;
        };
        let reader = BufReader::new(file);
        let Ok(meta): std::result::Result<SessionMeta, _> = serde_json::from_reader(reader) else {
            continue;
        };
        if let (Some(sid), Some(cwd)) = (meta.session_id, meta.cwd) {
            if !sid.is_empty() && !cwd.is_empty() {
                out.insert(sid, PathBuf::from(cwd));
            }
        }
    }
    out
}

/// Resolve an encoded project-directory name to its real filesystem path.
///
/// Returns `None` only if every strategy fails, which in practice means the
/// directory is empty or corrupt.
pub fn resolve(
    encoded_dir: &str,
    sessions_meta: &HashMap<String, PathBuf>,
    projects_dir: &Path,
) -> Option<PathBuf> {
    let full = projects_dir.join(encoded_dir);
    if !full.is_dir() {
        return None;
    }

    // Strategy 1 & 2: scan session files.
    if let Ok(entries) = fs::read_dir(&full) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let session_id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

            // 1. Metadata lookup by sid.
            if !session_id.is_empty() {
                if let Some(cwd) = sessions_meta.get(session_id) {
                    if cwd.is_dir() {
                        return Some(cwd.clone());
                    }
                }
            }

            // 2. JSONL cwd scan — read just enough to find a cwd.
            if let Some(cwd) = first_cwd_in_jsonl(&path) {
                let candidate = PathBuf::from(&cwd);
                if candidate.is_dir() {
                    return Some(candidate);
                }
            }
        }
    }

    // Strategy 3: naive decode.
    let decoded = naive_decode(encoded_dir);
    if decoded.is_dir() {
        return Some(decoded);
    }
    // Even if the dir doesn't exist (moved project, etc.), return the best
    // guess so the caller can still render something meaningful.
    Some(naive_decode(encoded_dir))
}

/// Read up to 16 KiB of a `.jsonl` file looking for a `"cwd"` field.
///
/// We parse line-by-line instead of slurping the file so enormous sessions
/// don't balloon our memory footprint for what's usually a 50-byte answer.
fn first_cwd_in_jsonl(path: &Path) -> Option<String> {
    #[derive(Deserialize)]
    struct Line {
        cwd: Option<String>,
    }
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    // Hard cap iteration so a degenerate file can't stall us forever.
    for line in reader.lines().take(256) {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(parsed): std::result::Result<Line, _> = serde_json::from_str(trimmed) else {
            continue;
        };
        if let Some(cwd) = parsed.cwd {
            if !cwd.is_empty() {
                return Some(cwd);
            }
        }
    }
    None
}

/// Last-resort decoder.
///
/// Strips the leading `-` (which always encodes root `/`) and replaces every
/// remaining `-` with `/`. This is lossy when the original path contains
/// underscores, but it is correct for the majority of developer paths which
/// do not.
pub fn naive_decode(encoded: &str) -> PathBuf {
    // The encoded form always starts with '-' because every path encoded is
    // absolute. Strip it, then flip hyphens to slashes.
    let body = encoded.strip_prefix('-').unwrap_or(encoded);
    // Rebuild with a leading `/`.
    let mut decoded = String::with_capacity(body.len() + 1);
    decoded.push('/');
    for ch in body.chars() {
        if ch == '-' {
            decoded.push('/');
        } else {
            decoded.push(ch);
        }
    }
    PathBuf::from(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naive_decode_flips_hyphens() {
        assert_eq!(
            naive_decode("-Users-a0g11b6-claude-picker"),
            PathBuf::from("/Users/a0g11b6/claude/picker")
        );
    }

    #[test]
    fn naive_decode_on_no_leading_hyphen_still_rooted() {
        assert_eq!(naive_decode("foo-bar"), PathBuf::from("/foo/bar"));
    }

    #[test]
    fn naive_decode_empty() {
        assert_eq!(naive_decode(""), PathBuf::from("/"));
    }

    #[test]
    fn resolve_prefers_metadata() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects = tmp.path().join("projects");
        let encoded = "-Users-me-foo";
        let proj_dir = projects.join(encoded);
        fs::create_dir_all(&proj_dir).expect("mkdir projects");

        // A session file exists so the resolver visits the directory.
        let jsonl = proj_dir.join("deadbeef.jsonl");
        fs::write(&jsonl, b"").expect("write empty jsonl");

        // Point the metadata at a real directory to prove it is returned.
        let real_dir = tmp.path().join("my_foo");
        fs::create_dir_all(&real_dir).expect("mkdir real");

        let mut meta = HashMap::new();
        meta.insert("deadbeef".to_string(), real_dir.clone());

        let resolved = resolve(encoded, &meta, &projects);
        assert_eq!(resolved, Some(real_dir));
    }

    #[test]
    fn resolve_uses_jsonl_cwd_when_no_metadata() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects = tmp.path().join("projects");
        let encoded = "-Users-me-foo";
        let proj_dir = projects.join(encoded);
        fs::create_dir_all(&proj_dir).expect("mkdir projects");

        let real_dir = tmp.path().join("real");
        fs::create_dir_all(&real_dir).expect("mkdir real");

        let jsonl = proj_dir.join("aaa.jsonl");
        let line = format!(
            "{{\"type\":\"user\",\"sessionId\":\"aaa\",\"cwd\":{:?}}}\n",
            real_dir.to_str().unwrap()
        );
        fs::write(&jsonl, line).expect("write jsonl");

        let resolved = resolve(encoded, &HashMap::new(), &projects);
        assert_eq!(resolved, Some(real_dir));
    }

    #[test]
    fn resolve_falls_back_to_naive_decode() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let projects = tmp.path().join("projects");
        let encoded = "-nonexistent-fallback";
        fs::create_dir_all(projects.join(encoded)).expect("mkdir projects");

        let resolved = resolve(encoded, &HashMap::new(), &projects);
        // Expect the naive decode (unchanged dir may or may not exist).
        assert_eq!(resolved, Some(PathBuf::from("/nonexistent/fallback")));
    }

    #[test]
    fn load_metadata_reads_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sess = tmp.path().join("sessions");
        fs::create_dir_all(&sess).expect("mkdir sessions");
        fs::write(
            sess.join("abc.json"),
            r#"{"sessionId":"abc","cwd":"/tmp/foo","name":"hi"}"#,
        )
        .expect("write meta");
        // Intentionally broken file — must be skipped.
        fs::write(sess.join("bad.json"), b"{not json").expect("write bad");

        let meta = load_session_metadata(&sess);
        assert_eq!(meta.get("abc"), Some(&PathBuf::from("/tmp/foo")));
        assert_eq!(meta.len(), 1);
    }
}
