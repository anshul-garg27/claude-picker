//! Walk `~/.claude/file-history/` — checkpoint / `/rewind` browser data.
//!
//! Claude Code's `/rewind` feature persists file snapshots under
//! `~/.claude/file-history/<session-id>/<backupFileName>`. The JSONL record
//! that points at one of these is a `{"type":"file-history-snapshot",...}`
//! line with this shape (real sample from the user's machine):
//!
//! ```json
//! {
//!   "type": "file-history-snapshot",
//!   "messageId": "6e8e12e0-…",
//!   "snapshot": {
//!     "messageId": "6e8e12e0-…",
//!     "timestamp": "2026-04-16T15:47:20.360Z",
//!     "trackedFileBackups": {
//!       "/path/to/file.ts": {
//!         "backupFileName": "884094e9e6f84240@v2",
//!         "version": 2,
//!         "backupTime": "2026-04-16T13:12:05.153Z"
//!       }
//!     }
//!   },
//!   "isSnapshotUpdate": false
//! }
//! ```
//!
//! Strategy:
//! 1. Walk the on-disk `file-history/<sid>/` tree so we know which sessions
//!    have *any* checkpoints at all. The directory contents are opaque
//!    `<hash>@v<n>` files — the names give us version counts but not which
//!    logical files they represent.
//! 2. For each session that has checkpoints, stream its JSONL to recover the
//!    real filenames, snapshot timestamps, and which backupFileName maps to
//!    which tracked path.
//!
//! We deliberately do not load the backup file bytes here — that's a job for
//! the diff viewer, which is owned by another agent.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// One checkpoint snapshot — a single `file-history-snapshot` record, with
/// enough metadata for the browser to show "3 files, 12h ago".
#[derive(Debug, Clone)]
pub struct Checkpoint {
    /// `messageId` from the JSONL record — the hash the user would pass to
    /// `/rewind <hash>`. Rendered truncated in the UI.
    pub message_id: String,
    /// Session id (file stem of the JSONL + name of the file-history dir).
    pub session_id: String,
    /// When Claude Code wrote the snapshot.
    pub timestamp: Option<DateTime<Utc>>,
    /// One entry per file the snapshot tracked.
    pub files: Vec<TrackedFile>,
}

impl Checkpoint {
    /// Short-form hash used in the row label. First 8 chars is what the
    /// transcript logs use internally.
    pub fn short_hash(&self) -> String {
        self.message_id.chars().take(8).collect()
    }
}

/// One tracked-file entry inside a checkpoint.
#[derive(Debug, Clone)]
pub struct TrackedFile {
    pub real_path: PathBuf,
    pub backup_file: String,
    pub version: u32,
    pub backup_time: Option<DateTime<Utc>>,
}

/// One row in the checkpoint browser — aggregates a session's checkpoints.
#[derive(Debug, Clone)]
pub struct CheckpointSession {
    pub session_id: String,
    /// Decoded cwd if we could recover it. `None` falls back to "(unknown
    /// project)" in the UI.
    pub project_dir: Option<PathBuf>,
    /// Every checkpoint in chronological order (oldest first).
    pub checkpoints: Vec<Checkpoint>,
    /// Number of `@v<n>` backup files on disk. >= total tracked-file entries
    /// across every checkpoint in the happy path; exposed separately so the
    /// UI can surface the "on-disk vs logged" mismatch if it ever happens.
    pub on_disk_backups: u32,
}

impl CheckpointSession {
    pub fn total_checkpoints(&self) -> u32 {
        self.checkpoints.len() as u32
    }

    /// Most recent checkpoint timestamp — used to sort the session list.
    pub fn most_recent(&self) -> Option<DateTime<Utc>> {
        self.checkpoints.iter().rev().find_map(|c| c.timestamp)
    }

    /// Project name rendered in the list — decoded cwd basename or `"?"`.
    pub fn project_label(&self) -> String {
        self.project_dir
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "(unknown)".to_string())
    }
}

/// Aggregate result — what the UI consumes.
#[derive(Debug, Clone, Default)]
pub struct CheckpointData {
    pub sessions: Vec<CheckpointSession>,
}

impl CheckpointData {
    pub fn total_checkpoints(&self) -> u32 {
        self.sessions
            .iter()
            .map(|s| s.checkpoints.len() as u32)
            .sum()
    }
}

/// Scan the default `~/.claude/` layout. Missing directories short-circuit
/// to an empty [`CheckpointData`] — never an error.
pub fn scan() -> anyhow::Result<CheckpointData> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let fh = home.join(".claude").join("file-history");
    let projects = home.join(".claude").join("projects");
    Ok(scan_in(&fh, &projects))
}

/// Test-friendly variant: explicit roots.
///
/// For each `<fh>/<sid>/` directory, count on-disk `@v<n>` files and then
/// find the matching JSONL by scanning every `<projects>/*/` for
/// `<sid>.jsonl`. Sessions with no on-disk snapshots are skipped.
pub fn scan_in(file_history: &Path, projects_dir: &Path) -> CheckpointData {
    // 1. Enumerate session dirs on disk with backup counts.
    let mut on_disk: HashMap<String, u32> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(file_history) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let Some(sid) = p.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let count = std::fs::read_dir(&p)
                .map(|it| {
                    it.flatten()
                        .filter(|de| de.path().is_file())
                        .filter(|de| de.file_name().to_str().is_some_and(|n| n.contains("@v")))
                        .count() as u32
                })
                .unwrap_or(0);
            if count > 0 {
                on_disk.insert(sid.to_string(), count);
            }
        }
    }

    if on_disk.is_empty() {
        return CheckpointData::default();
    }

    // 2. Build an index from session-id → (project-dir, jsonl-path).
    let mut sid_index: HashMap<String, (PathBuf, PathBuf)> = HashMap::new();
    if let Ok(projects) = std::fs::read_dir(projects_dir) {
        for pe in projects.flatten() {
            let pdir = pe.path();
            if !pdir.is_dir() {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&pdir) else {
                continue;
            };
            for fe in files.flatten() {
                let path = fe.path();
                if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }
                if let Some(sid) = path.file_stem().and_then(|s| s.to_str()) {
                    sid_index.insert(sid.to_string(), (pdir.clone(), path));
                }
            }
        }
    }

    // 3. For each on-disk session, pull checkpoints from JSONL.
    let mut sessions: Vec<CheckpointSession> = Vec::new();
    for (sid, backup_count) in on_disk.into_iter() {
        let (project_dir, jsonl) = match sid_index.get(&sid) {
            Some(x) => (Some(x.0.clone()), Some(x.1.clone())),
            None => (None, None),
        };
        let mut checkpoints = Vec::new();
        if let Some(jsonl) = jsonl {
            checkpoints = parse_checkpoints(&jsonl, &sid);
        }
        // Decode the project dir (encoded name) into a real cwd on a
        // best-effort basis. The UI only needs a nice label.
        let decoded = project_dir
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(decode_project_dir);
        sessions.push(CheckpointSession {
            session_id: sid,
            project_dir: decoded,
            checkpoints,
            on_disk_backups: backup_count,
        });
    }

    // 4. Sort by most-recent checkpoint desc, fall back to session id for
    //    stability.
    sessions.sort_by(|a, b| match (b.most_recent(), a.most_recent()) {
        (Some(bx), Some(ax)) => bx.cmp(&ax),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.session_id.cmp(&b.session_id),
    });

    CheckpointData { sessions }
}

// ── JSONL line shape ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RawLine {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default, rename = "messageId")]
    message_id: Option<String>,
    #[serde(default)]
    snapshot: Option<RawSnapshot>,
}

#[derive(Debug, Deserialize)]
struct RawSnapshot {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default, rename = "trackedFileBackups")]
    tracked_file_backups: BTreeMap<String, RawTracked>,
}

#[derive(Debug, Deserialize)]
struct RawTracked {
    #[serde(default, rename = "backupFileName")]
    backup_file_name: Option<String>,
    #[serde(default)]
    version: Option<u32>,
    #[serde(default, rename = "backupTime")]
    backup_time: Option<String>,
}

fn parse_checkpoints(path: &Path, session_id: &str) -> Vec<Checkpoint> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        if !line.contains("\"file-history-snapshot\"") {
            continue;
        }
        let Ok(raw) = serde_json::from_str::<RawLine>(&line) else {
            continue;
        };
        if raw.kind.as_deref() != Some("file-history-snapshot") {
            continue;
        }
        let Some(snap) = raw.snapshot else { continue };
        let Some(msg_id) = raw.message_id else {
            continue;
        };

        let ts = snap.timestamp.as_deref().and_then(parse_ts);
        let files = snap
            .tracked_file_backups
            .into_iter()
            .map(|(p, t)| TrackedFile {
                real_path: PathBuf::from(p),
                backup_file: t.backup_file_name.unwrap_or_default(),
                version: t.version.unwrap_or(0),
                backup_time: t.backup_time.as_deref().and_then(parse_ts),
            })
            .collect();
        out.push(Checkpoint {
            message_id: msg_id,
            session_id: session_id.to_string(),
            timestamp: ts,
            files,
        });
    }
    out
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Same naive decoder as `data::settings::decode_project_dir` — we can't
/// share the private fn across module boundaries without exposing it.
fn decode_project_dir(encoded: &str) -> PathBuf {
    if encoded.is_empty() {
        return PathBuf::new();
    }
    let trimmed = encoded.trim_start_matches('-');
    let mut out = String::with_capacity(trimmed.len() + 1);
    out.push('/');
    out.push_str(&trimmed.replace('-', "/"));
    PathBuf::from(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn short_hash_truncates() {
        let c = Checkpoint {
            message_id: "abcdef012345".into(),
            session_id: "sid".into(),
            timestamp: None,
            files: vec![],
        };
        assert_eq!(c.short_hash(), "abcdef01");
    }

    #[test]
    fn scan_happy_path() {
        let tmp = tempfile::tempdir().unwrap();
        let fh = tmp.path().join("file-history");
        let projects = tmp.path().join("projects");
        let sid = "11111111-2222-3333-4444-555555555555";

        // On-disk backup dir with two version files.
        fs::create_dir_all(fh.join(sid)).unwrap();
        fs::write(fh.join(sid).join("deadbeef00000000@v1"), b"old").unwrap();
        fs::write(fh.join(sid).join("deadbeef00000000@v2"), b"new").unwrap();

        // Matching session transcript under an encoded project dir.
        let pdir = projects.join("-Users-me-proj");
        fs::create_dir_all(&pdir).unwrap();
        let jsonl =
            "{\"type\":\"file-history-snapshot\",\"messageId\":\"8a3f2c1d-0000\",\"snapshot\":{\
                \"timestamp\":\"2026-04-16T10:00:00Z\",\
                \"trackedFileBackups\":{\
                    \"/Users/me/proj/src/x.rs\":{\"backupFileName\":\"deadbeef00000000@v2\",\
                                                \"version\":2,\
                                                \"backupTime\":\"2026-04-16T10:00:00Z\"}\
                }\
            }}\n"
                .to_string();
        fs::write(pdir.join(format!("{sid}.jsonl")), jsonl).unwrap();

        let data = scan_in(&fh, &projects);
        assert_eq!(data.sessions.len(), 1);
        let s = &data.sessions[0];
        assert_eq!(s.session_id, sid);
        assert_eq!(s.on_disk_backups, 2);
        assert_eq!(s.checkpoints.len(), 1);
        let cp = &s.checkpoints[0];
        assert_eq!(cp.message_id, "8a3f2c1d-0000");
        assert_eq!(cp.short_hash(), "8a3f2c1d");
        assert_eq!(cp.files.len(), 1);
        assert_eq!(
            cp.files[0].real_path,
            PathBuf::from("/Users/me/proj/src/x.rs")
        );
        assert_eq!(cp.files[0].version, 2);
        assert_eq!(s.project_label(), "proj");
        assert_eq!(data.total_checkpoints(), 1);
    }

    #[test]
    fn scan_returns_empty_for_missing_dirs() {
        let data = scan_in(Path::new("/nope"), Path::new("/nope2"));
        assert!(data.sessions.is_empty());
    }

    #[test]
    fn session_with_backups_but_no_jsonl_still_appears() {
        let tmp = tempfile::tempdir().unwrap();
        let fh = tmp.path().join("file-history");
        let projects = tmp.path().join("projects");
        let sid = "orphan-sid";
        fs::create_dir_all(fh.join(sid)).unwrap();
        fs::write(fh.join(sid).join("aa@v1"), b"").unwrap();
        fs::create_dir_all(&projects).unwrap();

        let data = scan_in(&fh, &projects);
        assert_eq!(data.sessions.len(), 1);
        assert_eq!(data.sessions[0].session_id, sid);
        assert_eq!(data.sessions[0].checkpoints.len(), 0);
        assert_eq!(data.sessions[0].on_disk_backups, 1);
        assert_eq!(data.sessions[0].project_label(), "(unknown)");
    }

    #[test]
    fn empty_backup_dir_is_filtered() {
        let tmp = tempfile::tempdir().unwrap();
        let fh = tmp.path().join("file-history");
        let projects = tmp.path().join("projects");
        fs::create_dir_all(fh.join("empty-sid")).unwrap();
        fs::create_dir_all(&projects).unwrap();

        let data = scan_in(&fh, &projects);
        assert!(data.sessions.is_empty(), "zero-backup dir must be skipped");
    }
}
