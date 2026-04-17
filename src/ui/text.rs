//! Unicode-correct display-width helpers for the UI layer.
//!
//! Why this module exists: terminal cells are *columns*, not bytes and not
//! codepoints. A string's display width is the sum of the East-Asian-Width
//! of its grapheme clusters (with zero-width joiners / combining marks
//! collapsing into the preceding cluster). `.len()` returns utf-8 byte
//! length; `.chars().count()` returns codepoint count. Both diverge from
//! column count as soon as a string contains:
//!
//! - CJK characters — each takes **2 columns**, not 1
//! - Emoji — **2 columns**, usually one codepoint (but can be many!)
//! - Combining marks — **0 columns**, attach to the previous cluster
//! - Accented latin (via NFD) — 1 column visually but 2 codepoints in memory
//! - ZWJ sequences like 👨‍👩‍👧‍👦 — dozens of bytes, multiple codepoints, but
//!   still render as a single ~2-column glyph on modern terminals
//!
//! The helpers here wrap [`unicode_width`] + [`unicode_segmentation`] so every
//! call site can round-trip through functions that agree on column math. The
//! slice form of `truncate` is grapheme-boundary-safe: it walks clusters, not
//! codepoints, so you never land mid-emoji.
//!
//! Convention: the ellipsis glyph `…` is counted as 1 column. It is a valid
//! East-Asian "Neutral" character and terminals we care about render it in a
//! single cell.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Display width of `s` in terminal columns.
///
/// Prefer this over `.len()` and `.chars().count()` anywhere the result is
/// used to compute padding, truncation, or right-alignment. The cost is a
/// single pass over the bytes — comparable to `.chars().count()` in practice.
#[inline]
pub fn display_width(s: &str) -> usize {
    // UnicodeWidthStr counts East-Asian-Width; zero-width joiners etc. fold
    // into the preceding cluster naturally. Control chars contribute 0.
    UnicodeWidthStr::width(s)
}

/// Truncate `s` to at most `max_cols` display columns. If anything was cut,
/// the returned string ends with `…` (1 column) so the caller can tell.
///
/// Grapheme-safe: walks by extended grapheme cluster, so multi-codepoint
/// glyphs (ZWJ emoji, combining diacritics, regional-indicator flags) never
/// split. When appending the ellipsis would itself exceed `max_cols` we drop
/// trailing clusters until it fits.
///
/// The returned string's `display_width()` is **guaranteed ≤ `max_cols`**.
pub fn truncate_to_width(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    let width = display_width(s);
    if width <= max_cols {
        return s.to_string();
    }

    // Walk clusters, tracking total width. Stop the moment the next cluster
    // would push us past `max_cols - 1` (reserving 1 column for "…").
    let budget = max_cols.saturating_sub(1); // leave room for ellipsis
    let mut out = String::with_capacity(s.len());
    let mut used = 0usize;
    for g in s.graphemes(true) {
        let w = display_width(g);
        if used + w > budget {
            break;
        }
        out.push_str(g);
        used += w;
    }
    out.push('…');
    out
}

/// Right-pad `s` with spaces to exactly `cols` display columns. When `s` is
/// already wider than `cols`, we first truncate to `cols` columns (with `…`)
/// so the return width is always exactly `cols`.
///
/// The returned string's `display_width()` is **guaranteed == `cols`**
/// (except the degenerate `cols == 0` case, which returns empty).
pub fn pad_to_width(s: &str, cols: usize) -> String {
    if cols == 0 {
        return String::new();
    }
    let w = display_width(s);
    if w == cols {
        return s.to_string();
    }
    if w > cols {
        // Truncate, then ensure the result fits exactly.
        let trunc = truncate_to_width(s, cols);
        let tw = display_width(&trunc);
        if tw == cols {
            return trunc;
        }
        // `truncate_to_width` can produce a string < `cols` when the next
        // cluster after the cut was wide (e.g. a 2-col glyph when 1 col left);
        // pad the difference.
        let mut out = trunc;
        for _ in 0..(cols.saturating_sub(tw)) {
            out.push(' ');
        }
        return out;
    }
    // w < cols: pad right with spaces.
    let mut out = String::with_capacity(s.len() + (cols - w));
    out.push_str(s);
    for _ in 0..(cols - w) {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── display_width ────────────────────────────────────────────────────

    #[test]
    fn ascii_width_matches_len() {
        assert_eq!(display_width(""), 0);
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("abcde fghij"), 11);
    }

    #[test]
    fn japanese_is_double_width() {
        // 5 CJK chars × 2 cols = 10 cols
        let s = "こんにちは";
        assert_eq!(s.chars().count(), 5, "codepoints");
        assert_eq!(display_width(s), 10, "columns");
        // .len() would be 15 bytes — prove it's different from width.
        assert_ne!(s.len(), display_width(s));
    }

    #[test]
    fn emoji_is_double_width() {
        let s = "🎉";
        assert_eq!(display_width(s), 2);
    }

    #[test]
    fn accented_latin_is_single_width_per_char() {
        // "résumé" is 6 chars, 6 columns, but 8 utf-8 bytes (accents are 2
        // bytes each in NFC). If a caller used .len() they'd get 8 and over-
        // allocate / under-truncate.
        let s = "résumé";
        assert_eq!(display_width(s), 6);
        assert_ne!(s.len(), display_width(s));
    }

    #[test]
    fn mixed_ascii_emoji_cjk() {
        // "test" (4) + " " (1) + "🎉" (2) + " " (1) + "café" (4) = 12
        let s = "test 🎉 café";
        assert_eq!(display_width(s), 12);
    }

    #[test]
    fn zwj_family_emoji_is_two_columns() {
        // 👨‍👩‍👧‍👦 — father+mother+girl+boy joined by U+200D. Terminals render
        // this as a single emoji cluster, 2 cols wide. It's 4 codepoints +
        // 3 ZWJs = 7 codepoints but still ~2 cols on compliant terminals.
        let s = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        assert_eq!(display_width(s), 2, "family emoji should be 2 cols");
    }

    #[test]
    fn combining_marks_are_zero_width() {
        // U+0301 (combining acute accent) attaches to the previous cluster
        // and contributes 0 cols. "e" + U+0301 is an NFD "é" — 1 col, 2
        // codepoints. Prove the helper agrees with that.
        let s = "e\u{0301}";
        assert_eq!(s.chars().count(), 2, "two codepoints");
        assert_eq!(display_width(s), 1, "one display column");
    }

    #[test]
    fn zero_width_joiner_itself_is_zero_width() {
        // U+200D is zero-width by design.
        assert_eq!(display_width("\u{200D}"), 0);
    }

    // ── truncate_to_width ────────────────────────────────────────────────

    #[test]
    fn truncate_short_is_unchanged() {
        assert_eq!(truncate_to_width("abc", 5), "abc");
    }

    #[test]
    fn truncate_exact_fit_is_unchanged() {
        assert_eq!(truncate_to_width("abcde", 5), "abcde");
    }

    #[test]
    fn truncate_cuts_and_ellipsises_ascii() {
        let out = truncate_to_width("abcdefghij", 5);
        assert_eq!(out, "abcd…");
        assert_eq!(display_width(&out), 5);
    }

    #[test]
    fn truncate_respects_column_count_not_char_count() {
        // "こんにちは" = 10 cols (5 chars × 2). Truncating to 5 cols should
        // fit 2 chars (4 cols) + "…" (1 col) = 5 cols exact.
        let out = truncate_to_width("こんにちは", 5);
        assert!(
            display_width(&out) <= 5,
            "got {}: {}",
            display_width(&out),
            out
        );
        assert!(out.ends_with('…'));
    }

    #[test]
    fn truncate_never_exceeds_max_cols() {
        for max in [1usize, 2, 3, 5, 7, 10] {
            for s in [
                "abcdefghij",
                "こんにちは",
                "test 🎉 café",
                "résumé is long",
                "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466} family",
            ] {
                let out = truncate_to_width(s, max);
                assert!(
                    display_width(&out) <= max,
                    "truncate_to_width({s:?}, {max}) = {out:?} has width {}",
                    display_width(&out)
                );
            }
        }
    }

    #[test]
    fn truncate_grapheme_safe_on_zwj_emoji() {
        // Family emoji is 2 cols. With a max of 5 cols we should get the whole
        // family + " fam" (4 cols remain, but "…" eats 1, so "fa…" fits).
        let s = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466} family";
        let out = truncate_to_width(s, 5);
        assert!(display_width(&out) <= 5);
        // Must never split mid-ZWJ (ends with whole cluster or ellipsis).
        assert!(
            !out.ends_with('\u{200D}'),
            "truncation left a dangling ZWJ: {out:?}",
        );
    }

    #[test]
    fn truncate_zero_max_is_empty() {
        assert_eq!(truncate_to_width("hello", 0), "");
    }

    // ── pad_to_width ─────────────────────────────────────────────────────

    #[test]
    fn pad_exactly_right() {
        for (s, cols) in [
            ("", 5),
            ("a", 5),
            ("hello", 5),
            ("こんにちは", 10),
            ("résumé", 10),
            ("test 🎉 café", 15),
        ] {
            let padded = pad_to_width(s, cols);
            assert_eq!(
                display_width(&padded),
                cols,
                "pad_to_width({s:?}, {cols}) = {padded:?}",
            );
        }
    }

    #[test]
    fn pad_truncates_oversized_strings() {
        let padded = pad_to_width("abcdefghij", 5);
        assert_eq!(display_width(&padded), 5);
        assert!(padded.ends_with('…'));
    }

    #[test]
    fn pad_truncates_oversized_cjk() {
        // 5-col budget; source is 10 cols.
        let padded = pad_to_width("こんにちは", 5);
        assert_eq!(display_width(&padded), 5, "got {padded:?}");
    }

    #[test]
    fn pad_zero_cols_is_empty() {
        assert_eq!(pad_to_width("hello", 0), "");
    }

    // ── Real-world regression: the bug the audit caught ──────────────────

    #[test]
    fn cjk_session_name_does_not_overflow_column() {
        // A session named "パスワード更新" is 7 CJK chars = 14 cols.
        // The old `pad_right` in session_list.rs counted .chars() and
        // returned 14 cols of content instead of padding it to the 28-col
        // name budget — but then the comparison `chars >= width` incorrectly
        // decided it was too big at 7 chars for a 28-col column, so it
        // received NO padding, and the cost/age trailed the name at column
        // 14 instead of column 28. The audit replaces that with
        // `pad_to_width(name, 28)` which correctly pads from 14 cols up to
        // 28 cols with 14 trailing spaces.
        let s = "パスワード更新";
        let padded = pad_to_width(s, 28);
        assert_eq!(
            display_width(&padded),
            28,
            "padded width must equal col budget"
        );
    }
}
