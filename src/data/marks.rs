//! Vim-style jumplist marks.
//!
//! Two-character chords — `m<a-z>` to set a mark, `'<a-z>` to jump — let
//! users park at a session (or project-list row) and teleport back later,
//! even across picker invocations. Persisted as JSON at
//! `~/.claude-picker/marks.json` so a restart doesn't forget everything.
//!
//! ```json
//! {
//!   "a": { "session_id": "abc123…", "view": "session_list" },
//!   "b": { "project_idx": 3, "view": "project_list" }
//! }
//! ```
//!
//! Missing files, malformed JSON, and unwritable `$HOME` all degrade to an
//! empty in-memory store — marks are a convenience, not critical data, so
//! we never surface a hard error just because the disk state is wonky.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Which screen a mark was captured on. Mirrors the values used by the
/// jump-ring so the two systems can swap payloads later if we want to union
/// the code paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkView {
    ProjectList,
    SessionList,
}

/// A single stored mark. Holds enough state to re-point the picker at the
/// row the user stamped. `session_id` is the primary hook for session-list
/// marks; `project_idx` carries a best-effort pointer for project-list
/// marks. Neither field alone is load-bearing — the jump handler falls
/// back gracefully when the session/project has been removed since.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mark {
    /// Which screen this mark was captured on.
    pub view: MarkView,
    /// Session id if one was selected at capture time. Always present for
    /// session-list marks; optional for project-list ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Index into the project list at capture time. Best-effort — the
    /// jumper prefers `session_id` when it exists because indices shift
    /// when projects appear or disappear between sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_idx: Option<usize>,
}

impl Mark {
    /// Construct a session-list mark for a given session id.
    pub fn session(session_id: impl Into<String>) -> Self {
        Self {
            view: MarkView::SessionList,
            session_id: Some(session_id.into()),
            project_idx: None,
        }
    }

    /// Construct a project-list mark for a given project index.
    pub fn project(idx: usize) -> Self {
        Self {
            view: MarkView::ProjectList,
            session_id: None,
            project_idx: Some(idx),
        }
    }
}

/// In-memory mark table keyed by single-letter `a..=z`. Round-trips through
/// JSON at [`Marks::default_path`].
#[derive(Debug, Clone, Default)]
pub struct Marks {
    path: Option<PathBuf>,
    entries: BTreeMap<char, Mark>,
}

impl Marks {
    /// Default location: `~/.claude-picker/marks.json`. Returns `None` when
    /// the home dir is unresolvable (headless CI) — the in-memory store
    /// still works in that case, [`save`](Self::save) just becomes a no-op.
    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".claude-picker").join("marks.json"))
    }

    /// Load from the default path. Missing or malformed files return an
    /// empty store — marks are not worth crashing the picker over.
    pub fn load() -> Self {
        match Self::default_path() {
            Some(path) => Self::load_from(path.clone()).unwrap_or_else(|_| Self::empty_at(Some(path))),
            None => Self::empty_at(None),
        }
    }

    /// Injection-friendly variant of [`load`](Self::load). Used by tests so
    /// they can point at a tempdir path.
    pub fn load_from(path: PathBuf) -> io::Result<Self> {
        if !path.exists() {
            return Ok(Self::empty_at(Some(path)));
        }
        let raw = fs::read_to_string(&path)?;
        // Tolerate empty / malformed JSON by falling back to an empty map.
        // We only return Err for actual I/O errors above.
        let entries: BTreeMap<char, Mark> = serde_json::from_str(&raw).unwrap_or_default();
        Ok(Self {
            path: Some(path),
            entries,
        })
    }

    fn empty_at(path: Option<PathBuf>) -> Self {
        Self {
            path,
            entries: BTreeMap::new(),
        }
    }

    /// Persist the current table to the default path. No-op when no path
    /// is available. Creates the parent directory on demand.
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(&self.entries).map_err(io::Error::other)?;
        fs::write(path, body)?;
        Ok(())
    }

    /// Store `mark` under `key`. Only accepts ascii lowercase `a..=z` —
    /// other keys are rejected silently so mistyped chords can't corrupt
    /// the table.
    pub fn set(&mut self, key: char, mark: Mark) {
        if !key.is_ascii_lowercase() {
            return;
        }
        self.entries.insert(key, mark);
    }

    /// Fetch the mark stored under `key`, if any.
    pub fn get(&self, key: char) -> Option<&Mark> {
        self.entries.get(&key)
    }

    /// Iterate all stored `(key, mark)` pairs. Intended for debugging /
    /// future overlays — the picker itself only reads via `get`.
    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = (&char, &Mark)> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_roundtrip() {
        let mut m = Marks::default();
        m.set('a', Mark::session("abc123"));
        let got = m.get('a').expect("mark a");
        assert_eq!(got.session_id.as_deref(), Some("abc123"));
        assert_eq!(got.view, MarkView::SessionList);
    }

    #[test]
    fn set_rejects_non_lowercase() {
        let mut m = Marks::default();
        m.set('A', Mark::session("x"));
        m.set('1', Mark::session("x"));
        assert!(m.get('A').is_none());
        assert!(m.get('1').is_none());
    }

    #[test]
    fn load_from_missing_file_is_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("marks.json");
        let m = Marks::load_from(path).expect("load");
        assert!(m.get('a').is_none());
    }

    #[test]
    fn load_from_malformed_file_is_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("marks.json");
        fs::write(&path, b"not json").expect("write");
        let m = Marks::load_from(path).expect("load");
        assert!(m.get('a').is_none());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("marks.json");
        let mut m = Marks::load_from(path.clone()).expect("load");
        m.set('a', Mark::session("abc123"));
        m.set('b', Mark::project(3));
        m.save().expect("save");

        let reloaded = Marks::load_from(path).expect("reload");
        assert_eq!(
            reloaded.get('a').map(|v| v.session_id.as_deref()),
            Some(Some("abc123"))
        );
        assert_eq!(
            reloaded.get('b').map(|v| v.project_idx),
            Some(Some(3))
        );
    }

    #[test]
    fn set_overwrites_previous_mark() {
        let mut m = Marks::default();
        m.set('a', Mark::session("first"));
        m.set('a', Mark::session("second"));
        assert_eq!(m.get('a').unwrap().session_id.as_deref(), Some("second"));
    }
}
