//! In-memory LRU cache for rendered identicon thumbnails.
//!
//! Keyed on `(basename, theme_hash)` so a theme swap invalidates every entry
//! (the palette derives from the active theme's accent tokens). Capped at
//! [`CACHE_CAP`] entries — with nine pinned slots plus a handful of recent
//! hovers we rarely see more than ~20, so the cap is mostly belt-and-braces.
//!
//! The value is an [`Arc<DynamicImage>`] so the renderer can clone cheaply
//! without copying the 64×64 RGB buffer. The halfblock renderer actually
//! reads from the grid directly — the pixel buffer is kept for tests and
//! future non-render consumers.
//!
//! Not thread-safe: the UI thread is the only writer/reader. If we ever push
//! identicon generation off the render thread (it's ~1ms for a 64×64 tile so
//! unlikely) wrap it in a `Mutex` then.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use image::DynamicImage;

/// Maximum number of cached images. Nine pinned slots + a few hover tiles
/// fits comfortably under this; overflow evicts least-recently-used.
pub const CACHE_CAP: usize = 50;

/// Cache key: basename plus a theme hash. The theme hash is opaque to this
/// module — the caller folds in whatever palette bytes vary with the active
/// theme so a theme swap produces a fresh key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThumbKey {
    pub basename: String,
    pub theme_hash: u64,
}

impl ThumbKey {
    /// Ergonomic constructor so call-sites don't have to name the fields.
    pub fn new(basename: impl Into<String>, theme_hash: u64) -> Self {
        Self {
            basename: basename.into(),
            theme_hash,
        }
    }
}

/// Tiny fixed-cap LRU. A `VecDeque<ThumbKey>` tracks recency; the map holds
/// the actual images. Eviction pops the front of the deque and removes the
/// matching entry from the map.
///
/// We deliberately don't pull in `lru` / `schnellru` — the logic is ~30
/// lines and this keeps the dependency graph lean.
pub struct ThumbnailCache {
    entries: HashMap<ThumbKey, Arc<DynamicImage>>,
    order: VecDeque<ThumbKey>,
    cap: usize,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self::with_capacity(CACHE_CAP)
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(cap),
            order: VecDeque::with_capacity(cap),
            cap: cap.max(1),
        }
    }

    /// Look up a key. On hit, bumps the key to the most-recently-used slot.
    pub fn get(&mut self, key: &ThumbKey) -> Option<Arc<DynamicImage>> {
        let img = self.entries.get(key)?.clone();
        self.touch(key);
        Some(img)
    }

    /// Insert or replace a key. May evict the least-recently-used entry if
    /// the cache is at capacity. Inserting an existing key refreshes its
    /// recency without changing capacity pressure.
    pub fn insert(&mut self, key: ThumbKey, img: Arc<DynamicImage>) {
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), img);
            self.touch(&key);
            return;
        }
        if self.entries.len() >= self.cap {
            if let Some(evict) = self.order.pop_front() {
                self.entries.remove(&evict);
            }
        }
        self.entries.insert(key.clone(), img);
        self.order.push_back(key);
    }

    /// Drop everything. Called on theme switch as a belt-and-braces cleanup
    /// — the theme_hash key bump already invalidates entries, but clearing
    /// reclaims the memory right away instead of waiting for LRU eviction.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    /// Number of entries currently cached (mostly for tests / debug).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Move `key` to the back of the recency queue. Safe to call with a key
    /// not currently tracked (no-op).
    fn touch(&mut self, key: &ThumbKey) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            if let Some(k) = self.order.remove(pos) {
                self.order.push_back(k);
            }
        }
    }
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbImage};

    fn img(r: u8) -> Arc<DynamicImage> {
        let buf = RgbImage::from_pixel(4, 4, image::Rgb([r, 0, 0]));
        Arc::new(DynamicImage::ImageRgb8(buf))
    }

    #[test]
    fn hit_bumps_recency() {
        let mut c = ThumbnailCache::with_capacity(2);
        c.insert(ThumbKey::new("a", 1), img(1));
        c.insert(ThumbKey::new("b", 1), img(2));
        // touch `a` so `b` becomes the LRU and is evicted next.
        let _ = c.get(&ThumbKey::new("a", 1));
        c.insert(ThumbKey::new("c", 1), img(3));
        assert!(c.get(&ThumbKey::new("a", 1)).is_some());
        assert!(c.get(&ThumbKey::new("b", 1)).is_none());
        assert!(c.get(&ThumbKey::new("c", 1)).is_some());
    }

    #[test]
    fn clear_drops_everything() {
        let mut c = ThumbnailCache::with_capacity(4);
        c.insert(ThumbKey::new("a", 1), img(1));
        c.insert(ThumbKey::new("b", 1), img(2));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn reinsert_does_not_evict() {
        let mut c = ThumbnailCache::with_capacity(2);
        c.insert(ThumbKey::new("a", 1), img(1));
        c.insert(ThumbKey::new("b", 1), img(2));
        // Re-inserting an existing key must not push capacity over — `a`
        // stays present and nothing gets evicted.
        c.insert(ThumbKey::new("a", 1), img(9));
        assert_eq!(c.len(), 2);
        assert!(c.get(&ThumbKey::new("a", 1)).is_some());
        assert!(c.get(&ThumbKey::new("b", 1)).is_some());
    }
}
