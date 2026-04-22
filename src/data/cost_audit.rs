//! Cost-optimisation audit over the user's real session corpus.
//!
//! Reads every `~/.claude/projects/<enc>/<id>.jsonl` (re-using the same loaders
//! the picker does), computes three heuristics per session, and returns a
//! flat list of [`AuditFinding`] rows sorted by estimated savings descending.
//! The UI layer consumes this directly; there is no interactive mutation.
//!
//! Heuristics (from the spec):
//!
//! 1. **Tool-call ratio**: share of *output* tokens that were spent on
//!    `tool_use` / `tool_result` traffic rather than conversational assistant
//!    text. Anything ≥ 70 % flags "could have used Haiku for the read-only
//!    parts". The ratio is computed against `session.tokens.output` (not the
//!    total token bucket — including cache-reads in the denominator drowns
//!    the signal on real sessions where 80–90 % of bytes are warm-cache
//!    reads).
//! 2. **Cache efficiency**: `cache_read / (cache_create + cache_read)` < 20 %
//!    suggests the session was chopped into small pieces and never built up a
//!    warm cache — flag "low cache hit rate".
//! 3. **Model mismatch**: Opus 4.x session with fewer than 5 k total tokens —
//!    Sonnet or Haiku would have done the same job at a fraction of the cost.
//!
//! Every heuristic requires `session.message_count >= 5` to suppress the
//! probe-session noise (abort-on-boot runs with 1-2 messages that the user
//! doesn't actually care about cost-optimising).
//!
//! Savings estimates are deliberately conservative. Tool-ratio savings are
//! computed as `output_cost × tool_ratio × (1 − haiku_output_ratio)` — i.e.
//! we assume only the output tokens attributable to tool traffic would be
//! re-priced at Haiku's rate. Cache-efficiency and model-mismatch still use
//! the cruder "fraction of total cost" approximation; the tool-ratio fix is
//! the one where the old formula was demonstrably misleading (see the
//! Phase 0 audit notes).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use serde::Deserialize;

use crate::data::pricing::{
    cost_for, family, haiku_output_ratio_to, output_rate_for, Family, TokenCounts,
};
use crate::data::project::discover_projects;
use crate::data::session::load_session_from_jsonl;
use crate::data::Session;

/// One row in the audit output. Carries enough metadata for the UI to build
/// its line AND for the user to jump straight back into that session.
#[derive(Debug, Clone)]
pub struct AuditFinding {
    pub session_id: String,
    pub project_name: String,
    pub project_cwd: PathBuf,
    pub session_label: String,
    pub total_cost_usd: f64,
    pub model_summary: String,
    pub findings: Vec<Finding>,
    pub estimated_savings_usd: f64,
}

/// Which of the three heuristics produced a given [`Finding`]. Lets the UI
/// group rows into per-heuristic sections and lets callers aggregate savings
/// by category without string-matching the user-facing message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindingKind {
    /// A session spent most of its output tokens on tool traffic — Haiku
    /// could have produced the same output for less.
    ToolRatio,
    /// The warm-cache hit rate fell below [`CACHE_EFFICIENCY_THRESHOLD`].
    CacheEfficiency,
    /// A small Opus session — Sonnet would have sufficed.
    ModelMismatch,
}

/// A single heuristic hit.
#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: Severity,
    pub kind: FindingKind,
    pub message: String,
    pub savings_usd: f64,
}

/// Whether the finding is a hard "you're bleeding money" warning or a softer
/// "consider this" nudge. Chooses the glyph + colour in the UI layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warn,
    Info,
}

/// Tool-call ratio threshold. Matches the spec: ≥ 70 % of *output* tokens
/// being tool-traffic means the session spent most of its output-billable
/// money on Opus-priced tool output that Haiku could have produced.
pub const TOOL_RATIO_THRESHOLD: f64 = 0.70;

/// Cache efficiency threshold — sessions under this hit rate read as "low
/// cache" and get flagged.
pub const CACHE_EFFICIENCY_THRESHOLD: f64 = 0.20;

/// Cache-efficiency savings heuristic: flag session as recoverable for this
/// fraction of its total cost when cache hit rate falls below the threshold.
/// Hand-tuned; conservative ceiling for "if you had continued, you would've
/// saved this much" — a warm cache at Opus rates saves ~90% on the cached
/// input side, but not all tokens are cache-eligible, so 20% is the cap we
/// quote. Numerically coincides with `CACHE_EFFICIENCY_THRESHOLD` but is
/// semantically independent: tuning the threshold does not imply tuning this.
const CACHE_SAVINGS_FRACTION: f64 = 0.20;

/// Below this token count, an Opus session is a "model mismatch" — Sonnet or
/// Haiku would have done the job at a fraction of the price.
pub const SMALL_SESSION_THRESHOLD_TOKENS: u64 = 5_000;

/// Below this message count, a session is considered a probe / abort-on-boot
/// run. Not worth flagging: the user didn't actually do anything and the
/// advice "use Haiku" is noise.
const PROBE_SESSION_MIN_MESSAGES: u32 = 5;

/// Assumed cost ratio between Sonnet 4.x and Opus 4.7. Used by the
/// model-mismatch savings estimate.
const SONNET_RATIO_OF_OPUS: f64 = 0.60;

/// Public entrypoint — discovers every project, walks its sessions, returns
/// findings sorted by estimated savings desc.
pub fn run_audit() -> anyhow::Result<Vec<AuditFinding>> {
    let projects = discover_projects()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let projects_root = home.join(".claude").join("projects");

    let mut out: Vec<AuditFinding> = Vec::new();
    for project in &projects {
        let dir = projects_root.join(&project.encoded_dir);
        if !dir.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let session = match load_session_from_jsonl(&path, project.path.clone()) {
                Ok(Some(s)) => s,
                _ => continue,
            };
            let Some(finding) = audit_session(&session, &path, project.name.clone()) else {
                continue;
            };
            out.push(finding);
        }
    }
    // Biggest-savings first — the UI shows the top N first.
    out.sort_by(|a, b| {
        b.estimated_savings_usd
            .partial_cmp(&a.estimated_savings_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(out)
}

/// Inspect a single session and return an [`AuditFinding`] if any heuristic
/// hits. Returns `None` for sessions that are fully efficient — rendering a
/// "no findings" row would be noise.
pub fn audit_session(
    session: &Session,
    jsonl_path: &std::path::Path,
    project_name: String,
) -> Option<AuditFinding> {
    let stats = collect_session_stats(jsonl_path);
    let (findings, total_savings) =
        evaluate_heuristics(session, stats.total_tokens, stats.tool_tokens);
    if findings.is_empty() {
        return None;
    }
    Some(AuditFinding {
        session_id: session.id.clone(),
        project_name,
        project_cwd: session.project_dir.clone(),
        session_label: session.display_label().to_string(),
        total_cost_usd: session.total_cost_usd,
        model_summary: session.model_summary.clone(),
        findings,
        estimated_savings_usd: total_savings,
    })
}

/// Pure heuristic evaluation — runs all three gates against already-computed
/// stats and returns the findings plus their summed savings. Shared by
/// [`audit_session`] (which reads JSONL) and [`audit_session_with_stats`]
/// (which tests feed synthetic numbers) so there is a single source of truth
/// for what each heuristic does.
fn evaluate_heuristics(
    session: &Session,
    stats_total_tokens: u64,
    stats_tool_tokens: u64,
) -> (Vec<Finding>, f64) {
    let mut findings: Vec<Finding> = Vec::new();
    let mut total_savings = 0.0;

    let probe_ok = session.message_count >= PROBE_SESSION_MIN_MESSAGES;

    // ── 1. Tool-call ratio ────────────────────────────────────────────────
    // Denominator is output tokens only. Including cache-reads (as the old
    // formula did) swamps the numerator — in the launch-corpus check, every
    // real session scored < 6 % tool-ratio under the old formula, even ones
    // that were genuinely tool-heavy.
    if stats_total_tokens >= 1_000 && probe_ok && !session.model_summary.is_empty() {
        let output_tokens = session.tokens.output;
        let tool_ratio = if output_tokens == 0 {
            0.0
        } else {
            // Cap at 1.0 — `stats_tool_tokens` sums output-of-tool-use plus
            // input-of-next-turn, which can overshoot pure output.
            (stats_tool_tokens as f64 / output_tokens as f64).min(1.0)
        };
        if tool_ratio >= TOOL_RATIO_THRESHOLD && is_opus_or_sonnet(&session.model_summary) {
            let output_cost = output_tokens as f64 * output_rate_for(&session.model_summary);
            let haiku_ratio = haiku_output_ratio_to(&session.model_summary);
            let savings = output_cost * tool_ratio * (1.0 - haiku_ratio);
            let pct = (tool_ratio * 100.0).round() as i64;
            let msg = format!(
                "{pct}% tool_use tokens \u{2014} Haiku could save ~${:.2}",
                savings
            );
            findings.push(Finding {
                severity: Severity::Warn,
                kind: FindingKind::ToolRatio,
                message: msg,
                savings_usd: savings,
            });
            total_savings += savings;
        }
    }

    // ── 2. Cache efficiency ───────────────────────────────────────────────
    let cache_create = session.tokens.cache_write_5m + session.tokens.cache_write_1h;
    let denom = cache_create + session.tokens.cache_read;
    if denom >= 1_000 && probe_ok {
        let ratio = session.tokens.cache_read as f64 / denom as f64;
        if ratio < CACHE_EFFICIENCY_THRESHOLD {
            // Savings fraction — see `CACHE_SAVINGS_FRACTION` for the
            // reasoning. Conservative cap, not the threshold reused by value.
            let savings = session.total_cost_usd * CACHE_SAVINGS_FRACTION;
            let pct = (ratio * 100.0).round() as i64;
            let msg = format!("cache hit rate {pct}% \u{2014} consider session continuation");
            findings.push(Finding {
                severity: Severity::Info,
                kind: FindingKind::CacheEfficiency,
                message: msg,
                savings_usd: savings,
            });
            total_savings += savings;
        }
    }

    // ── 3. Model mismatch ─────────────────────────────────────────────────
    let total_tokens = session.tokens.total();
    if is_opus_family(&session.model_summary)
        && total_tokens < SMALL_SESSION_THRESHOLD_TOKENS
        && session.total_cost_usd > 0.05
        && probe_ok
    {
        // Small session on Opus → Sonnet or Haiku would suffice. We quote
        // Sonnet savings (the safer downgrade) rather than Haiku.
        let savings = session.total_cost_usd * (1.0 - SONNET_RATIO_OF_OPUS);
        let msg = format!(
            "model: opus \u{00B7} {}k tokens \u{2014} Sonnet would suffice (save ~${:.2})",
            total_tokens / 1000,
            savings,
        );
        findings.push(Finding {
            severity: Severity::Info,
            kind: FindingKind::ModelMismatch,
            message: msg,
            savings_usd: savings,
        });
        total_savings += savings;
    }

    (findings, total_savings)
}

/// True for anything whose dominant model belongs to the Opus family. The
/// cheapest-to-audit sessions are the ones already on Haiku — we'd never
/// suggest "use Haiku" to someone already there.
fn is_opus_family(model: &str) -> bool {
    matches!(family(model), Family::Opus)
}

/// Opus or Sonnet — the two families where recommending a Haiku downgrade
/// on tool-heavy traffic makes economic sense. Haiku is already the cheapest
/// and recommending it to a Haiku user would be noise.
fn is_opus_or_sonnet(model: &str) -> bool {
    matches!(family(model), Family::Opus | Family::Sonnet)
}

/// Per-session rollup used by the heuristics. We re-parse the JSONL here
/// (rather than extend `Session`) so the audit stays a clearly bounded
/// additional pass — no load-time overhead for users who never run it.
#[derive(Debug, Default)]
struct SessionStats {
    /// Total tokens across all buckets for this session.
    total_tokens: u64,
    /// Tokens attributable to `tool_use` / `tool_result` blocks — measured
    /// as the *output* token count of assistant messages that contained a
    /// tool-use block plus the *input* token count of the next assistant
    /// message (tool results are fed back as input). This is approximate but
    /// it beats trying to tokenise the raw JSON ourselves.
    tool_tokens: u64,
}

/// Second-pass reader that counts which messages were "tool heavy" so the
/// audit heuristic can answer "how much of this cost was tool traffic".
fn collect_session_stats(path: &std::path::Path) -> SessionStats {
    let Ok(file) = File::open(path) else {
        return SessionStats::default();
    };
    let reader = BufReader::new(file);

    let mut stats = SessionStats::default();
    // Track whether the *previous* assistant message contained a tool_use
    // block — if so, the next assistant message's input tokens are
    // "tool_result traffic" for our accounting.
    let mut last_was_tool_use = false;

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let raw: RawLine = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if raw.kind.as_deref() != Some("assistant") {
            continue;
        }
        let Some(msg) = raw.message else { continue };
        let Some(usage) = msg.usage else { continue };
        let per_msg = msg_total(&usage);
        stats.total_tokens = stats.total_tokens.saturating_add(per_msg);

        let has_tool = msg.content.as_ref().is_some_and(has_tool_use);
        if has_tool {
            // Output tokens on a tool-use message ≈ the tool call args; small
            // but real, still tool-attributed traffic.
            stats.tool_tokens = stats.tool_tokens.saturating_add(usage.output_tokens);
            last_was_tool_use = true;
        } else if last_was_tool_use {
            // This assistant message is the follow-up after a tool result —
            // the input tokens include the tool_result payload, which is
            // exactly what we want to attribute to tool traffic.
            stats.tool_tokens = stats.tool_tokens.saturating_add(usage.input_tokens);
            last_was_tool_use = false;
        }
    }
    stats
}

#[derive(Debug, Deserialize, Default)]
struct RawLine {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<RawMsg>,
}

#[derive(Debug, Deserialize, Default)]
struct RawMsg {
    #[serde(default)]
    content: Option<serde_json::Value>,
    #[serde(default)]
    usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize, Default, Clone, Copy)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

fn msg_total(u: &RawUsage) -> u64 {
    u.input_tokens
        .saturating_add(u.output_tokens)
        .saturating_add(u.cache_read_input_tokens)
        .saturating_add(u.cache_creation_input_tokens)
}

fn has_tool_use(content: &serde_json::Value) -> bool {
    let Some(blocks) = content.as_array() else {
        return false;
    };
    blocks
        .iter()
        .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
}

/// Sum of `estimated_savings_usd` across every finding. Used for the
/// "total potential savings" footer in the UI.
pub fn total_potential_savings(findings: &[AuditFinding]) -> f64 {
    findings.iter().map(|f| f.estimated_savings_usd).sum()
}

/// Per-heuristic rollup across every [`AuditFinding`]. Returns
/// `[(kind, count, sum_savings_usd); 3]` in a stable order suitable for the
/// UI summary band. Missing categories appear with `count = 0, savings = 0`.
pub fn summary_by_kind(audit_findings: &[AuditFinding]) -> [(FindingKind, usize, f64); 3] {
    let mut totals: [(FindingKind, usize, f64); 3] = [
        (FindingKind::ToolRatio, 0, 0.0),
        (FindingKind::CacheEfficiency, 0, 0.0),
        (FindingKind::ModelMismatch, 0, 0.0),
    ];
    for af in audit_findings {
        for f in &af.findings {
            let idx = match f.kind {
                FindingKind::ToolRatio => 0,
                FindingKind::CacheEfficiency => 1,
                FindingKind::ModelMismatch => 2,
            };
            totals[idx].1 += 1;
            totals[idx].2 += f.savings_usd;
        }
    }
    totals
}

/// Exposed for testing — build a synthetic stats rollup and run the
/// heuristic logic against it without touching the disk. Delegates to the
/// shared [`evaluate_heuristics`] helper so the disk-reading and test-fixture
/// paths cannot drift apart.
#[doc(hidden)]
pub fn audit_session_with_stats(
    session: &Session,
    stats_total_tokens: u64,
    stats_tool_tokens: u64,
    project_name: String,
) -> Option<AuditFinding> {
    let (findings, total_savings) =
        evaluate_heuristics(session, stats_total_tokens, stats_tool_tokens);
    if findings.is_empty() {
        return None;
    }
    Some(AuditFinding {
        session_id: session.id.clone(),
        project_name,
        project_cwd: session.project_dir.clone(),
        session_label: session.display_label().to_string(),
        total_cost_usd: session.total_cost_usd,
        model_summary: session.model_summary.clone(),
        findings,
        estimated_savings_usd: total_savings,
    })
}

/// Recompute `total_cost_usd` for a session from its tokens — exposed so
/// tests can build sessions without redoing the math.
pub fn recompute_cost(model: &str, tokens: TokenCounts) -> f64 {
    cost_for(model, tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::session::SessionKind;

    fn mk_session(id: &str, model: &str, cost: f64, tokens: TokenCounts) -> Session {
        Session {
            id: id.into(),
            project_dir: PathBuf::from("/tmp"),
            name: None,
            auto_name: Some(id.into()),
            last_prompt: None,
            message_count: 10,
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

    #[test]
    fn tool_heavy_opus_session_flagged_with_haiku_savings() {
        // Output = 10_000, tool tokens = 8_000 → ratio 0.80 ≥ 0.70.
        let tokens = TokenCounts {
            input: 10_000,
            output: 10_000,
            ..Default::default()
        };
        let session = mk_session("a", "claude-opus-4-7", 10.0, tokens);
        let finding = audit_session_with_stats(&session, 20_000, 8_000, "proj".into())
            .expect("should flag");
        assert!(finding
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::ToolRatio));
        assert!(
            finding.estimated_savings_usd > 0.0,
            "positive savings expected"
        );
    }

    #[test]
    fn haiku_session_never_flagged_for_tool_ratio() {
        let tokens = TokenCounts {
            input: 10_000,
            output: 10_000,
            ..Default::default()
        };
        let session = mk_session("a", "claude-haiku-4-5", 1.0, tokens);
        let finding = audit_session_with_stats(&session, 20_000, 8_000, "proj".into());
        // Haiku sessions might trip *other* rules but never the tool-ratio one,
        // since we'd be recommending the model they already use.
        if let Some(f) = finding {
            for item in &f.findings {
                assert!(
                    item.kind != FindingKind::ToolRatio,
                    "Haiku sessions should never get the tool-ratio flag"
                );
            }
        }
    }

    #[test]
    fn sonnet_session_can_flag_for_tool_ratio() {
        // New formula: Sonnet is downgradeable to Haiku too (5/15 output-
        // cost ratio). Haiku ratio for Sonnet = 1/3 so savings = output_cost
        // × 0.80 × (1 − 0.333…) ≈ output_cost × 0.533.
        let tokens = TokenCounts {
            input: 10_000,
            output: 10_000,
            ..Default::default()
        };
        let session = mk_session("s", "claude-sonnet-4-5", 0.50, tokens);
        let finding = audit_session_with_stats(&session, 20_000, 8_000, "proj".into())
            .expect("sonnet tool-heavy session should flag");
        assert!(finding
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::ToolRatio));
    }

    #[test]
    fn low_cache_hit_rate_flagged() {
        let tokens = TokenCounts {
            input: 0,
            output: 0,
            cache_read: 100,
            cache_write_5m: 10_000,
            cache_write_1h: 0,
        };
        let session = mk_session("a", "claude-opus-4-7", 5.0, tokens);
        let finding = audit_session_with_stats(&session, 2_000, 0, "proj".into())
            .expect("should flag");
        assert!(finding
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::CacheEfficiency));
    }

    #[test]
    fn small_opus_session_flagged_as_model_mismatch() {
        let tokens = TokenCounts {
            input: 500,
            output: 500,
            ..Default::default()
        };
        let session = mk_session("a", "claude-opus-4-7", 0.37, tokens);
        let finding = audit_session_with_stats(&session, 1_000, 0, "proj".into())
            .expect("should flag");
        assert!(finding
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::ModelMismatch));
    }

    #[test]
    fn totally_fine_session_returns_none() {
        let tokens = TokenCounts {
            input: 100_000,
            output: 50_000,
            cache_read: 80_000,
            cache_write_5m: 5_000,
            cache_write_1h: 0,
        };
        let session = mk_session("a", "claude-opus-4-7", 5.0, tokens);
        // Not tool-heavy (0 tool tokens), good cache ratio, big session.
        let finding = audit_session_with_stats(&session, 200_000, 0, "proj".into());
        assert!(finding.is_none());
    }

    #[test]
    fn probe_session_never_flagged() {
        // 2 messages and otherwise tool-heavy — the probe floor kills it.
        let tokens = TokenCounts {
            input: 10_000,
            output: 10_000,
            ..Default::default()
        };
        let mut session = mk_session("probe", "claude-opus-4-7", 10.0, tokens);
        session.message_count = 2;
        let finding = audit_session_with_stats(&session, 20_000, 8_000, "proj".into());
        assert!(
            finding.is_none(),
            "abort-on-boot probe sessions should be silent"
        );
    }

    #[test]
    fn tool_ratio_is_capped_at_one() {
        // stats_tool_tokens > output → would raw-compute to > 100 %. The cap
        // prevents the UI rendering "113 % tool_use tokens".
        let tokens = TokenCounts {
            input: 0,
            output: 10_000,
            ..Default::default()
        };
        let session = mk_session("over", "claude-opus-4-7", 5.0, tokens);
        let finding = audit_session_with_stats(&session, 50_000, 50_000, "proj".into())
            .expect("still flags");
        let tool = finding
            .findings
            .iter()
            .find(|f| f.kind == FindingKind::ToolRatio)
            .expect("tool-ratio finding present");
        // Message prefix rounds to 100 % when ratio is capped at 1.0.
        assert!(
            tool.message.starts_with("100%"),
            "expected capped 100% prefix, got: {}",
            tool.message
        );
    }

    #[test]
    fn total_savings_sums_findings() {
        let sessions = vec![AuditFinding {
            session_id: "a".into(),
            project_name: "p".into(),
            project_cwd: PathBuf::new(),
            session_label: "t".into(),
            total_cost_usd: 1.0,
            model_summary: "claude-opus-4-7".into(),
            findings: vec![],
            estimated_savings_usd: 1.25,
        }];
        assert!((total_potential_savings(&sessions) - 1.25).abs() < 1e-9);
    }

    #[test]
    fn recompute_cost_uses_pricing_table() {
        let tokens = TokenCounts {
            input: 1_000_000,
            output: 0,
            ..Default::default()
        };
        // Opus 4 input = $5 / 1M = 5.0 on 1M.
        assert!((recompute_cost("claude-opus-4-7", tokens) - 5.00).abs() < 1e-9);
    }

    #[test]
    fn summary_by_kind_orders_and_sums_correctly() {
        let findings = vec![
            AuditFinding {
                session_id: "a".into(),
                project_name: "p".into(),
                project_cwd: PathBuf::new(),
                session_label: "t".into(),
                total_cost_usd: 1.0,
                model_summary: "claude-opus-4-7".into(),
                findings: vec![
                    Finding {
                        severity: Severity::Warn,
                        kind: FindingKind::ToolRatio,
                        message: "tool".into(),
                        savings_usd: 2.0,
                    },
                    Finding {
                        severity: Severity::Info,
                        kind: FindingKind::CacheEfficiency,
                        message: "cache".into(),
                        savings_usd: 0.5,
                    },
                ],
                estimated_savings_usd: 2.5,
            },
            AuditFinding {
                session_id: "b".into(),
                project_name: "p".into(),
                project_cwd: PathBuf::new(),
                session_label: "t".into(),
                total_cost_usd: 1.0,
                model_summary: "claude-opus-4-7".into(),
                findings: vec![Finding {
                    severity: Severity::Warn,
                    kind: FindingKind::ToolRatio,
                    message: "tool2".into(),
                    savings_usd: 1.0,
                }],
                estimated_savings_usd: 1.0,
            },
        ];
        let s = summary_by_kind(&findings);
        assert_eq!(s[0].0, FindingKind::ToolRatio);
        assert_eq!(s[0].1, 2);
        assert!((s[0].2 - 3.0).abs() < 1e-9);
        assert_eq!(s[1].0, FindingKind::CacheEfficiency);
        assert_eq!(s[1].1, 1);
        assert!((s[1].2 - 0.5).abs() < 1e-9);
        assert_eq!(s[2].0, FindingKind::ModelMismatch);
        assert_eq!(s[2].1, 0);
        assert!(s[2].2.abs() < 1e-9);
    }

    #[test]
    fn empty_model_summary_does_not_panic_and_skips_gated_heuristics() {
        // Mixed-model sessions that never got a scorable assistant message
        // end up with an empty model_summary. Tool-ratio + model-mismatch
        // are gated on family() so they silently skip; cache-efficiency may
        // still fire.
        let tokens = TokenCounts {
            input: 0,
            output: 0,
            cache_read: 100,
            cache_write_5m: 10_000,
            cache_write_1h: 0,
        };
        let session = mk_session("empty", "", 1.0, tokens);
        // Should not panic.
        let finding = audit_session_with_stats(&session, 20_000, 8_000, "proj".into());
        if let Some(af) = finding {
            for f in &af.findings {
                // Tool-ratio must be suppressed when model_summary is empty.
                assert!(f.kind != FindingKind::ToolRatio);
            }
            // Model-mismatch must also be suppressed: family("") is Unknown,
            // so is_opus_family returns false and the gate skips.
            assert!(
                !af.findings
                    .iter()
                    .any(|f| f.kind == FindingKind::ModelMismatch),
                "empty model_summary should not trigger model-mismatch"
            );
        }
    }
}
