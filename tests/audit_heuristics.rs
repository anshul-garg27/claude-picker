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
