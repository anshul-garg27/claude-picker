//! Bookmark storage at `~/.claude-picker/bookmarks.json`.
//!
//! The file is a flat JSON array of session-id strings, matching the
//! pre-existing bash/Python implementation so users upgrading from v1 keep
//! their pinned sessions.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::PathBuf;

/// Pinned-session store backed by a single JSON file.
pub struct BookmarkStore {
    path: PathBuf,
    ids: HashSet<String>,
}

impl BookmarkStore {
    /// Default location: `~/.claude-picker/bookmarks.json`. An empty store
    /// is returned if the file doesn't exist yet.
    pub fn load() -> anyhow::Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        let path = home.join(".claude-picker").join("bookmarks.json");
        Self::load_from(path)
    }

    /// Load from an explicit path — useful for tests and tool overrides.
    pub fn load_from(path: PathBuf) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self {
                path,
                ids: HashSet::new(),
            });
        }
        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        // Tolerate completely empty files and legacy variants where someone
        // hand-edited the JSON into an object; fall back to empty.
        let ids: HashSet<String> = serde_json::from_reader(reader).unwrap_or_default();
        Ok(Self { path, ids })
    }

    pub fn contains(&self, session_id: &str) -> bool {
        self.ids.contains(session_id)
    }

    /// Toggle the bookmark for `session_id` and persist. Returns the new
    /// state: `true` means bookmarked, `false` means removed.
    pub fn toggle(&mut self, session_id: &str) -> anyhow::Result<bool> {
        let now_bookmarked = if self.ids.contains(session_id) {
            self.ids.remove(session_id);
            false
        } else {
            self.ids.insert(session_id.to_string());
            true
        };
        self.save()?;
        Ok(now_bookmarked)
    }

    /// Read-only view of the bookmarked ids.
    pub fn ids(&self) -> &HashSet<String> {
        &self.ids
    }

    /// Persist the current set to disk. Creates the parent directory if it
    /// doesn't exist.
    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut list: Vec<&String> = self.ids.iter().collect();
        list.sort(); // deterministic file contents
        let body = serde_json::to_string_pretty(&list)?;
        let mut file = File::create(&self.path)?;
        file.write_all(body.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_empty_then_toggle() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("bookmarks.json");
        let mut store = BookmarkStore::load_from(path.clone()).expect("load");

        assert!(!store.contains("sid1"));
        assert!(store.toggle("sid1").expect("toggle"));
        assert!(store.contains("sid1"));
        assert!(!store.toggle("sid1").expect("toggle"));
        assert!(!store.contains("sid1"));

        // Persisted state should match.
        let reloaded = BookmarkStore::load_from(path).expect("reload");
        assert!(!reloaded.contains("sid1"));
    }

    #[test]
    fn multiple_bookmarks_persist_sorted() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("bookmarks.json");
        let mut store = BookmarkStore::load_from(path.clone()).expect("load");
        store.toggle("b").expect("b");
        store.toggle("a").expect("a");
        store.toggle("c").expect("c");

        let raw = fs::read_to_string(&path).expect("read");
        let first = raw.find('"').expect("quote");
        // The first id written must be the lexicographically smallest.
        assert!(raw[first..].starts_with("\"a\""));
    }

    #[test]
    fn handles_missing_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("none").join("bookmarks.json");
        let store = BookmarkStore::load_from(path).expect("load");
        assert!(store.ids().is_empty());
    }

    #[test]
    fn tolerates_malformed_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("bookmarks.json");
        fs::write(&path, b"not json").expect("write");
        let store = BookmarkStore::load_from(path).expect("load");
        assert!(store.ids().is_empty());
    }
}
