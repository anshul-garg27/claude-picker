//! Integration tests for the `--search` screen primitives.
//!
//! The command handler itself is an event-loop wrapper — we can't drive a
//! real terminal from `cargo test`. What we *can* test is the pure logic
//! backing it: snippet extraction (from `ui::search`) and score ordering
//! (nucleo's `Pattern` API, exercised the same way the handler uses it).

use claude_picker::ui::search::{dominant_word, extract_snippet};

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};

#[test]
fn snippet_centers_on_hit_with_ellipses() {
    // Long padding on both sides forces both ellipses.
    let pre = "x".repeat(120);
    let post = "y".repeat(120);
    let body = format!("{pre} race condition {post}");
    let snippet = extract_snippet(&body, "race");

    assert!(
        snippet.contains("race"),
        "snippet must include the hit: {snippet}"
    );
    assert!(
        snippet.starts_with('…'),
        "snippet should start with ellipsis when content is trimmed on the left: {snippet}"
    );
    assert!(
        snippet.ends_with('…'),
        "snippet should end with ellipsis when content is trimmed on the right: {snippet}"
    );
    // ≈ 80 chars window; allow a little slack for ellipses + trim.
    assert!(
        snippet.chars().count() <= 90,
        "snippet length out of bounds: {} chars ({})",
        snippet.chars().count(),
        snippet
    );
}

#[test]
fn snippet_multi_word_query_uses_longest_word() {
    // For the query "the race condition", the dominant word is "condition";
    // the snippet should be built around that match.
    let body = "preamble filler words then race condition appears here with more text afterwards";
    let needle = dominant_word("the race condition");
    assert_eq!(needle, "condition");
    let snippet = extract_snippet(body, &needle);
    assert!(snippet.contains("condition"), "got: {snippet}");
}

#[test]
fn exact_substring_outranks_fuzzy_partial() {
    // Same setup the command handler uses: `Pattern::parse` with Smart case
    // + Smart normalization. The contiguous exact match must score higher
    // than a haystack where the letters only appear as gap fuzz.
    let mut matcher = Matcher::new(Config::DEFAULT);
    let p = Pattern::parse("auth refactor", CaseMatching::Smart, Normalization::Smart);

    let exact = Utf32String::from("please help with the auth refactor PR");
    let fuzzy = Utf32String::from("automation that is factory-made and reactors");

    let s_exact = p
        .score(exact.slice(..), &mut matcher)
        .expect("exact must match");
    let s_fuzzy = p.score(fuzzy.slice(..), &mut matcher);

    match s_fuzzy {
        None => { /* exact wins trivially */ }
        Some(s) => assert!(
            s_exact > s,
            "exact substring ({s_exact}) must outrank fuzzy partial ({s})"
        ),
    }
}
