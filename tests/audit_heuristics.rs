//! Integration tests for the three audit heuristics.
//!
//! These tests live in `tests/` (not inside `cost_audit.rs`'s `#[cfg(test)]`
//! block) so they exercise the crate exactly as a downstream caller would —
//! every type they touch has to be `pub`. That forces the public shape of
//! [`AuditFinding`], [`Finding`], and [`FindingKind`] to stay stable.
//!
//! Helper sessions are constructed by hand to match the real [`Session`]
//! struct; see `docs/superpowers/notes/2026-04-22-audit-findings.md` §6 for
//! the rationale on why we don't assume `turn_count()` / `cache_write_tokens`
//! (neither exists).

use std::path::PathBuf;

use claude_picker::data::cost_audit::{
    audit_session_with_stats, AuditFinding, FindingKind,
};
use claude_picker::data::pricing::TokenCounts;
use claude_picker::data::{Session, SessionKind};

/// Build a test session with the full `Session` field-set populated to
/// neutral defaults. Tests override only the fields they care about.
fn make_session(model: &str, cost: f64, tokens: TokenCounts, message_count: u32) -> Session {
    Session {
        id: "test-id".into(),
        project_dir: PathBuf::from("/tmp"),
        name: None,
        auto_name: Some("test-session".into()),
        last_prompt: None,
        message_count,
        tokens,
        total_cost_usd: cost,
        model_summary: model.into(),
        first_timestamp: None,
        last_timestamp: None,
        is_fork: false,
        forked_from: None,
        entrypoint: SessionKind::Cli,
        permission_mode: None,
        subagent_count: 0,
        turn_durations: Vec::new(),
    }
}

/// True if the [`AuditFinding`] contains at least one finding of `kind`.
fn has_kind(af: &AuditFinding, kind: FindingKind) -> bool {
    af.findings.iter().any(|f| f.kind == kind)
}

/// Task 1.1: the anchor test for the NEW tool-ratio formula.
///
/// Denominator is now `session.tokens.output`, not `stats.total_tokens`. We
/// build a session where the tool token count exceeds 70 % of output tokens
/// and assert the heuristic fires with `kind == FindingKind::ToolRatio`.
#[test]
fn test_tool_ratio_finding_fires_on_high_output_tool_ratio() {
    // Output = 10_000, tool tokens = 8_000 → ratio = 0.80 ≥ 0.70.
    let tokens = TokenCounts {
        input: 5_000,
        output: 10_000,
        ..Default::default()
    };
    let session = make_session("claude-opus-4-7", 5.00, tokens, 20);
    let af = audit_session_with_stats(&session, 50_000, 8_000, "proj".into())
        .expect("tool-heavy opus session should flag");
    assert!(
        has_kind(&af, FindingKind::ToolRatio),
        "expected a ToolRatio finding; got: {:?}",
        af.findings.iter().map(|f| f.kind).collect::<Vec<_>>()
    );
    let tool_finding = af
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::ToolRatio)
        .unwrap();
    assert!(tool_finding.savings_usd > 0.0, "savings should be positive");
}

/// Task 1.3: cache-efficiency still fires post-reformulation and carries the
/// correct `FindingKind`.
#[test]
fn test_cache_efficiency_finding_fires_on_zero_cache_hits() {
    // 0 cache_read / 10_000 cache_write → 0% hit rate (< 20% threshold).
    let tokens = TokenCounts {
        input: 0,
        output: 0,
        cache_read: 0,
        cache_write_5m: 10_000,
        cache_write_1h: 0,
    };
    let session = make_session("claude-opus-4-7", 0.50, tokens, 20);
    let af = audit_session_with_stats(&session, 10_000, 0, "proj".into())
        .expect("zero-cache-hit session should flag");
    assert!(
        has_kind(&af, FindingKind::CacheEfficiency),
        "expected a CacheEfficiency finding; got: {:?}",
        af.findings.iter().map(|f| f.kind).collect::<Vec<_>>()
    );
    let cache = af
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::CacheEfficiency)
        .unwrap();
    assert!(
        cache
            .message
            .to_lowercase()
            .contains("cache hit rate")
            || cache.message.to_lowercase().contains("continuation"),
        "message should mention cache or continuation: {}",
        cache.message
    );
}

/// Task 1.4: short Opus session trips model-mismatch with `FindingKind`
/// correctly set.
#[test]
fn test_model_mismatch_finding_fires_on_small_opus_session() {
    // Total tokens < 5_000, cost > $0.05, Opus family, >= 5 messages.
    let tokens = TokenCounts {
        input: 1_000,
        output: 500,
        ..Default::default()
    };
    let session = make_session("claude-opus-4-7", 0.30, tokens, 8);
    let af = audit_session_with_stats(&session, 1_500, 0, "proj".into())
        .expect("small opus session should flag");
    assert!(
        has_kind(&af, FindingKind::ModelMismatch),
        "expected a ModelMismatch finding; got: {:?}",
        af.findings.iter().map(|f| f.kind).collect::<Vec<_>>()
    );
    let mm = af
        .findings
        .iter()
        .find(|f| f.kind == FindingKind::ModelMismatch)
        .unwrap();
    assert!(mm.savings_usd > 0.0, "positive savings");
    assert!(
        mm.savings_usd < session.total_cost_usd,
        "savings should never exceed the session cost"
    );
    assert!(
        mm.message.to_lowercase().contains("sonnet")
            || mm.message.to_lowercase().contains("haiku"),
        "message should suggest a cheaper family: {}",
        mm.message
    );
}

/// Task 1.4 (follow-on): empty `model_summary` must not panic the audit,
/// even when stats_total_tokens would otherwise trip tool-ratio.
#[test]
fn test_audit_session_empty_model_summary_does_not_panic() {
    let tokens = TokenCounts {
        input: 10_000,
        output: 10_000,
        ..Default::default()
    };
    let session = make_session("", 5.00, tokens, 20);
    // Should not panic; may return None (all heuristics that require a
    // family gate are silent when model_summary is empty).
    let result = audit_session_with_stats(&session, 20_000, 15_000, "proj".into());
    // The one heuristic that doesn't need a family gate is cache-efficiency
    // — but this session has no cache tokens, so nothing fires.
    assert!(result.is_none(), "empty model + no cache should return None");
}

