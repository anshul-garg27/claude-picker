//! Integration tests for the `--diff` command.
//!
//! Focused on topic extraction — the one piece of real logic that doesn't
//! live inside a render closure. The UI layer is exercised via the unit tests
//! inside `src/ui/diff.rs` and `src/commands/diff_cmd.rs`.

use claude_picker::commands::diff_cmd::extract_topics;

/// A tiny "session-A": redis + rate-limiter flavoured.
fn corpus_a() -> Vec<String> {
    vec![
        "redis rate limiter for session tokens".into(),
        "session tokens flow through redis properly".into(),
        "rate limiter tuning around session tokens".into(),
        "httponly cookies for session tokens not localStorage".into(),
    ]
}

/// A tiny "session-B": oauth2 flavoured, shares "session tokens" with A.
fn corpus_b() -> Vec<String> {
    vec![
        "oauth2 provider with session tokens".into(),
        "session tokens via oauth2 callback".into(),
        "oauth2 callback validates session tokens".into(),
        "device fingerprinting before issuing session tokens".into(),
    ]
}

#[test]
fn common_topics_contain_shared_tokens() {
    let a = extract_topics(&corpus_a(), 15);
    let b = extract_topics(&corpus_b(), 15);

    // Both corpora mention "session tokens" repeatedly, so either that
    // specific bigram or the unigram "tokens" should appear in both.
    let shared: Vec<&String> = a.iter().filter(|t| b.contains(t)).collect();
    assert!(
        !shared.is_empty(),
        "expected at least one common topic, a={a:?} b={b:?}"
    );
    // The "tokens"/"session" signal must be represented.
    let has_session_signal = shared
        .iter()
        .any(|t| t.contains("session") || t.contains("tokens"));
    assert!(
        has_session_signal,
        "expected session/tokens in common topics, got {shared:?}"
    );
}

#[test]
fn unique_to_a_contains_redis_rate_limiter_flavour() {
    let a = extract_topics(&corpus_a(), 15);
    let b = extract_topics(&corpus_b(), 15);
    let unique_a: Vec<&String> = a.iter().filter(|t| !b.contains(t)).collect();

    // "redis" or "rate limiter" should be uniquely in A.
    let has_a_signal = unique_a
        .iter()
        .any(|t| t.contains("redis") || t.contains("limiter"));
    assert!(
        has_a_signal,
        "expected redis/limiter unique to A, got {unique_a:?}"
    );
}

#[test]
fn unique_to_b_contains_oauth2_flavour() {
    let a = extract_topics(&corpus_a(), 15);
    let b = extract_topics(&corpus_b(), 15);
    let unique_b: Vec<&String> = b.iter().filter(|t| !a.contains(t)).collect();
    let has_b_signal = unique_b
        .iter()
        .any(|t| t.contains("oauth") || t.contains("callback") || t.contains("fingerprinting"));
    assert!(
        has_b_signal,
        "expected oauth/callback unique to B, got {unique_b:?}"
    );
}

#[test]
fn top_n_respected() {
    let mut big: Vec<String> = Vec::new();
    for i in 0..30 {
        big.push(format!(
            "word{i} word{i} redis limiter auth middleware session tokens"
        ));
    }
    let topics = extract_topics(&big, 10);
    assert!(topics.len() <= 10, "top_n=10 exceeded: {}", topics.len());
}

#[test]
fn stopwords_are_filtered_from_topics() {
    let texts = vec![
        "the quick brown fox jumps over the lazy dog".into(),
        "the fox and the dog are just animals".into(),
        "but the dog stayed while the fox ran".into(),
    ];
    let topics = extract_topics(&texts, 10);
    // Core stopwords must not appear as topics.
    for stop in &["the", "and", "but", "just", "are"] {
        assert!(
            !topics.iter().any(|t| t == *stop),
            "stopword '{stop}' leaked into topics: {topics:?}"
        );
    }
}

#[test]
fn empty_corpus_returns_no_topics() {
    assert!(extract_topics(&[], 10).is_empty());
    assert!(extract_topics(&["".to_string()], 10).is_empty());
}
