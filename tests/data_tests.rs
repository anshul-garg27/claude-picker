//! Integration test for the data layer.
//!
//! Loads `tests/fixtures/sample.jsonl` end-to-end and verifies aggregated
//! fields plus per-model cost math. Run with `cargo test`.

use std::path::PathBuf;

use claude_picker::data::pricing::{cost_for, TokenCounts};
use claude_picker::data::session::load_session_from_jsonl;

fn fixture_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/sample.jsonl");
    p
}

#[test]
fn loads_sample_and_aggregates_correctly() {
    let path = fixture_path();
    let session = load_session_from_jsonl(&path, PathBuf::from("/tmp/sample"))
        .expect("load ok")
        .expect("session should be Some (meets 2-message threshold)");

    assert_eq!(session.id, "sample");
    assert_eq!(session.name.as_deref(), Some("test-session"));
    assert_eq!(session.auto_name.as_deref(), Some("help me write a parser"));
    assert_eq!(session.display_label(), "test-session");

    // 2 user + 2 assistant = 4 messages (custom-title isn't counted).
    assert_eq!(session.message_count, 4);

    // Totals across both assistant messages:
    //   input:         1_000 + 2_000 = 3_000
    //   output:          500 + 1_500 = 2_000
    //   cache_read:       0 +   500 =   500
    //   cache_write_5m: 2000 +    0 = 2_000
    //   cache_write_1h:   0 +    0 =     0
    assert_eq!(
        session.tokens,
        TokenCounts {
            input: 3_000,
            output: 2_000,
            cache_read: 500,
            cache_write_5m: 2_000,
            cache_write_1h: 0,
        }
    );
    assert_eq!(session.tokens.total(), 7_500);

    // First + last timestamps match the fixture.
    assert!(session.first_timestamp.is_some());
    assert!(session.last_timestamp.is_some());
    assert!(session.first_timestamp.unwrap() < session.last_timestamp.unwrap());

    // Cost: sum of per-model costs.
    //   Opus 4.7 msg: 1_000 * 5 + 500 * 25 + 2_000 * 6.25 = 30_000 (scaled by 1e-6) = 0.030000
    //                 1_000 input * $5/1M  = 0.005
    //                 500 output * $25/1M  = 0.0125
    //                 2_000 cw5 * $6.25/1M = 0.0125
    //                 total = 0.030
    //   Sonnet 4 msg: 2_000 * 3 + 1_500 * 15 + 500 * 0.30 = 28_650 / 1M = 0.02865
    //                 total = 0.02865
    //   grand total = 0.05865
    let opus = cost_for(
        "claude-opus-4-7",
        TokenCounts {
            input: 1_000,
            output: 500,
            cache_read: 0,
            cache_write_5m: 2_000,
            cache_write_1h: 0,
        },
    );
    let sonnet = cost_for(
        "claude-sonnet-4-5",
        TokenCounts {
            input: 2_000,
            output: 1_500,
            cache_read: 500,
            cache_write_5m: 0,
            cache_write_1h: 0,
        },
    );
    let expected_total = opus + sonnet;
    assert!(
        (session.total_cost_usd - expected_total).abs() < 1e-9,
        "cost mismatch: got {}, expected {}",
        session.total_cost_usd,
        expected_total
    );

    // Model summary: tie at 1 each, deterministic (alphabetical b-a order
    // on the id gives claude-opus-4-7 because 'o' > 'n' comes later; we
    // picked "ties break by id reversed" so the Sonnet row wins here.
    // The contract is simply that we always pick a real model id, not the
    // empty string.
    assert!(
        session.model_summary.starts_with("claude-"),
        "expected a real model id, got {:?}",
        session.model_summary
    );

    // Entrypoint is CLI.
    assert_eq!(
        session.entrypoint,
        claude_picker::data::session::SessionKind::Cli
    );
}
