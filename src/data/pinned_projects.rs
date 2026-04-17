//! Persistent store for user-pinned project slots (k9s-style favorites).
//!
//! Nine numbered slots ("1" through "9") each hold at most one project cwd
//! string. Users press `u` to toggle a pin on the current project, then `1..9`
//! jumps straight to that project and `0` clears any project filter.
//!
//! On-disk shape is a tiny TOML file at
//! `~/.config/claude-picker/pinned.toml`:
//!
//! ```toml
//! [pinned]
//! "1" = "/Users/alice/work/architex"
//! "3" = "/Users/alice/work/claude-picker"
//! ```
//!
//! We deliberately key by stringified slot number (not a fixed-length array)
//! so sparse pins round-trip cleanly through `toml::to_string`. The in-memory
//! form is an array for fast lookup.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Number of pinnable slots. Slot numbers the user sees are 1..=9, which we
/// store at array indices 0..=8.
pub const NUM_SLOTS: usize = 9;

/// Result of [`PinnedProjects::toggle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleResult {
    /// The project was not previously pinned — now occupies this slot
    /// (1-indexed — what the user sees).
    Pinned(u8),
    /// The project was already pinned; the pin was removed. Returns the
    /// slot number (1-indexed) the project was evicted from.
    Unpinned(u8),
    /// Every slot is occupied and the project isn't already pinned.
    NoSlotsAvailable,
}

/// In-memory pinned-slot store. Slots are 1-indexed in the public API and
/// stored at index `slot - 1` internally. Every mutating method automatically
/// persists; callers don't have to remember to call [`save`](Self::save).
#[derive(Debug)]
pub struct PinnedProjects {
    path: Option<PathBuf>,
    /// Slot `slot - 1` holds the project cwd pinned at slot number `slot`.
    slots: [Option<String>; NUM_SLOTS],
}

/// Serde shape of the TOML file. Keys are stringified slot numbers so the
/// output looks like `"1" = "/path"` rather than a verbose array of entries.
#[derive(Debug, Serialize, Deserialize, Default)]
struct PinnedFile {
    #[serde(default)]
    pinned: BTreeMap<String, String>,
}

impl PinnedProjects {
    /// Default location: `~/.config/claude-picker/pinned.toml`. Returns an
    /// empty store when the home directory can't be resolved (headless CI
    /// hosts) — in that case [`save`](Self::save) is a no-op.
    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".config").join("claude-picker").join("pinned.toml"))
    }

    /// Load from the default path. Missing or malformed files return an
    /// empty store — pinned-slot state is "nice to have", not load-bearing.
    pub fn load() -> Self {
        match Self::default_path() {
            Some(path) => Self::load_from(&path).unwrap_or_else(|_| Self::empty_at(Some(path))),
            None => Self::empty_at(None),
        }
    }

    /// Injection-friendly variant of [`load`](Self::load).
    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty_at(Some(path.to_path_buf())));
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading pinned-projects at {}", path.display()))?;
        let parsed: PinnedFile = toml::from_str(&raw)
            .with_context(|| format!("parsing pinned-projects TOML at {}", path.display()))?;

        let mut slots: [Option<String>; NUM_SLOTS] = Default::default();
        for (key, cwd) in parsed.pinned {
            // Keys are "1".."9". Anything outside that range is silently
            // dropped so a hand-edit with typos doesn't nuke load.
            if let Ok(n @ 1..=9) = key.parse::<u8>() {
                slots[(n - 1) as usize] = Some(cwd);
            }
        }
        Ok(Self {
            path: Some(path.to_path_buf()),
            slots,
        })
    }

    /// Empty store with an optional persistence target. Used for headless
    /// hosts and as a fallback when on-disk parsing fails.
    fn empty_at(path: Option<PathBuf>) -> Self {
        Self {
            path,
            slots: Default::default(),
        }
    }

    /// Persist the current state to [`default_path`](Self::default_path).
    /// No-op when a path isn't available (e.g. no `$HOME`).
    pub fn save(&self) -> Result<()> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating {} parent dir", path.display()))?;
        }

        let mut pinned = BTreeMap::new();
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some(cwd) = slot {
                pinned.insert(((i as u8) + 1).to_string(), cwd.clone());
            }
        }
        let body = PinnedFile { pinned };
        let serialized =
            toml::to_string_pretty(&body).context("serializing pinned-projects TOML")?;
        fs::write(path, serialized).with_context(|| format!("writing to {}", path.display()))?;
        Ok(())
    }

    /// Toggle the pin for `project_cwd`.
    ///
    /// - If the cwd is already pinned: remove it and persist; return
    ///   [`ToggleResult::Unpinned`] with the slot it occupied.
    /// - Otherwise, find the lowest empty slot and pin the cwd there; return
    ///   [`ToggleResult::Pinned`].
    /// - If there are no empty slots and the cwd isn't already pinned:
    ///   [`ToggleResult::NoSlotsAvailable`]. State is not mutated.
    ///
    /// Persistence errors after a successful toggle are swallowed — the
    /// in-memory state is authoritative for the session. Callers that care
    /// can still call [`save`](Self::save) explicitly and handle the Result.
    pub fn toggle(&mut self, project_cwd: &str) -> ToggleResult {
        // Already pinned? Clear the slot.
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.as_deref() == Some(project_cwd) {
                *slot = None;
                let _ = self.save();
                return ToggleResult::Unpinned((i as u8) + 1);
            }
        }
        // Not pinned — find the first empty slot.
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(project_cwd.to_string());
                let _ = self.save();
                return ToggleResult::Pinned((i as u8) + 1);
            }
        }
        ToggleResult::NoSlotsAvailable
    }

    /// Project cwd pinned at slot `slot` (1-indexed). Out-of-range inputs
    /// return `None`.
    pub fn at_slot(&self, slot: u8) -> Option<&str> {
        if !(1..=9).contains(&slot) {
            return None;
        }
        self.slots[(slot - 1) as usize].as_deref()
    }

    /// Iterate over `(slot_number, project_cwd)` pairs in slot order (1..9),
    /// including only occupied slots.
    pub fn iter(&self) -> impl Iterator<Item = (u8, &str)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| opt.as_deref().map(|cwd| ((i as u8) + 1, cwd)))
    }

    /// True when no slot is occupied.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn toggle_pins_into_first_empty_slot() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("pinned.toml");
        let mut store = PinnedProjects::load_from(&path).expect("load");

        assert_eq!(store.toggle("/a"), ToggleResult::Pinned(1));
        assert_eq!(store.toggle("/b"), ToggleResult::Pinned(2));
        assert_eq!(store.at_slot(1), Some("/a"));
        assert_eq!(store.at_slot(2), Some("/b"));
    }

    #[test]
    fn toggle_unpins_if_present() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("pinned.toml");
        let mut store = PinnedProjects::load_from(&path).expect("load");
        assert_eq!(store.toggle("/a"), ToggleResult::Pinned(1));
        assert_eq!(store.toggle("/a"), ToggleResult::Unpinned(1));
        assert!(store.at_slot(1).is_none());
    }

    #[test]
    fn roundtrip_persists_sparse_slots() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("pinned.toml");
        {
            let mut s = PinnedProjects::load_from(&path).expect("load");
            s.toggle("/a"); // slot 1
            s.toggle("/b"); // slot 2
            s.toggle("/a"); // unpins slot 1
            s.toggle("/c"); // fills slot 1 (lowest empty)
        }
        let reloaded = PinnedProjects::load_from(&path).expect("reload");
        assert_eq!(reloaded.at_slot(1), Some("/c"));
        assert_eq!(reloaded.at_slot(2), Some("/b"));
        assert!(reloaded.at_slot(3).is_none());
    }

    #[test]
    fn no_slots_available_when_full() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("pinned.toml");
        let mut store = PinnedProjects::load_from(&path).expect("load");
        for i in 0..9 {
            assert!(matches!(
                store.toggle(&format!("/p{i}")),
                ToggleResult::Pinned(_)
            ));
        }
        assert_eq!(store.toggle("/p10"), ToggleResult::NoSlotsAvailable);
    }

    #[test]
    fn missing_file_returns_empty_store() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("nope.toml");
        let store = PinnedProjects::load_from(&path).expect("missing is ok");
        assert!(store.is_empty());
    }

    #[test]
    fn malformed_file_errors() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("pinned.toml");
        fs::write(&path, "not = valid = toml").expect("write");
        let err = PinnedProjects::load_from(&path).expect_err("must fail");
        assert!(format!("{err:#}").contains("parsing pinned-projects TOML"));
    }

    #[test]
    fn iter_yields_occupied_slots_in_order() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("pinned.toml");
        let mut store = PinnedProjects::load_from(&path).expect("load");
        store.toggle("/a");
        store.toggle("/b");
        let got: Vec<_> = store.iter().map(|(s, c)| (s, c.to_string())).collect();
        assert_eq!(got, vec![(1u8, "/a".to_string()), (2u8, "/b".to_string())]);
    }
}
