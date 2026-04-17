//! Cost-optimisation audit over the user's real session corpus.
//!
//! Reads every `~/.claude/projects/<enc>/<id>.jsonl` (re-using the same loaders
//! the picker does), computes three heuristics per session, and returns a
//! flat list of [`AuditFinding`] rows sorted by estimated savings descending.
//! The UI layer consumes this directly; there is no interactive mutation.
//!
//! Heuristics (from the spec):
//!
//! 1. **Tool-call ratio**: share of tokens that came from `tool_use` /
//!    `tool_result` blocks rather than conversational assistant text. Anything
//!    ≥ 70 % flags "could have used Haiku for the read-only parts".
//! 2. **Cache efficiency**: `cache_read / (cache_create + cache_read)` < 20 %
//!    suggests the session was chopped into small pieces and never built up a
//!    warm cache — flag "low cache hit rate".
//! 3. **Model mismatch**: Opus 4.x session with fewer than 5 k total tokens —
//!    Sonnet or Haiku would have done the same job at a fraction of the cost.
//!
//! Savings estimates are deliberately conservative; they assume the flagged
//! portion of cost would have been paid at Haiku rates.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use serde::Deserialize;

use crate::data::pricing::{cost_for, family, Family, TokenCounts};
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

/// A single heuristic hit.
#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: Severity,
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

/// Tool-call ratio threshold. Matches the spec: ≥ 70 % of tokens being
/// tool-traffic means the session spent most of its money on Opus-priced
/// tool output that Haiku could have produced.
pub const TOOL_RATIO_THRESHOLD: f64 = 0.70;

/// Cache efficiency threshold — sessions under this hit rate read as "low
/// cache" and get flagged.
pub const CACHE_EFFICIENCY_THRESHOLD: f64 = 0.20;

/// Below this token count, an Opus session is a "model mismatch" — Sonnet or
/// Haiku would have done the job at a fraction of the price.
pub const SMALL_SESSION_THRESHOLD_TOKENS: u64 = 5_000;

/// Assumed cost ratio between Haiku 4.5 and Opus 4.7. Used for rough savings
/// estimates — Haiku is ~$1/$5 vs Opus's $5/$25 so the blended rate is 5×
/// cheaper. The UI shows "~$X" to signal approximation.
const HAIKU_RATIO_OF_OPUS: f64 = 0.20;

/// Assumed cost ratio between Sonnet 4.x and Opus 4.7.
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
    let mut findings: Vec<Finding> = Vec::new();
    let mut total_savings = 0.0;

    // ── 1. Tool-call ratio ────────────────────────────────────────────────
    if stats.total_tokens >= 1_000 {
        let tool_ratio = if stats.total_tokens == 0 {
            0.0
        } else {
            stats.tool_tokens as f64 / stats.total_tokens as f64
        };
        if tool_ratio >= TOOL_RATIO_THRESHOLD && is_opus_family(&session.model_summary) {
            // Savings = the tool share of the cost × (1 - Haiku-ratio).
            let tool_cost = session.total_cost_usd * tool_ratio;
            let savings = tool_cost * (1.0 - HAIKU_RATIO_OF_OPUS);
            let pct = (tool_ratio * 100.0).round() as i64;
            let msg = format!(
                "{pct}% tool_use tokens \u{2014} Haiku could save ~${:.2}",
                savings
            );
            findings.push(Finding {
                severity: Severity::Warn,
                message: msg,
                savings_usd: savings,
            });
            total_savings += savings;
        }
    }

    // ── 2. Cache efficiency ───────────────────────────────────────────────
    let cache_create = session.tokens.cache_write_5m + session.tokens.cache_write_1h;
    let denom = cache_create + session.tokens.cache_read;
    if denom >= 1_000 {
        let ratio = session.tokens.cache_read as f64 / denom as f64;
        if ratio < CACHE_EFFICIENCY_THRESHOLD {
            // Savings is fuzzier here — a warm cache at Opus rates saves ~90 %
            // on the cached input side. Use a hand-tuned 20 % of total cost
            // as the cap so we don't overpromise.
            let savings = session.total_cost_usd * 0.20;
            let pct = (ratio * 100.0).round() as i64;
            let msg = format!("cache hit rate {pct}% \u{2014} consider session continuation");
            findings.push(Finding {
                severity: Severity::Info,
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
            message: msg,
            savings_usd: savings,
        });
        total_savings += savings;
    }

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

/// True for anything whose dominant model belongs to the Opus family. The
/// cheapest-to-audit sessions are the ones already on Haiku — we'd never
/// suggest "use Haiku" to someone already there.
fn is_opus_family(model: &str) -> bool {
    matches!(family(model), Family::Opus)
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

/// Exposed for testing — build a synthetic stats rollup and run the
/// heuristic logic against it without touching the disk.
#[doc(hidden)]
pub fn audit_session_with_stats(
    session: &Session,
    stats_total_tokens: u64,
    stats_tool_tokens: u64,
    project_name: String,
) -> Option<AuditFinding> {
    let mut findings: Vec<Finding> = Vec::new();
    let mut total_savings = 0.0;

    if stats_total_tokens >= 1_000 {
        let tool_ratio = stats_tool_tokens as f64 / stats_total_tokens as f64;
        if tool_ratio >= TOOL_RATIO_THRESHOLD && is_opus_family(&session.model_summary) {
            let tool_cost = session.total_cost_usd * tool_ratio;
            let savings = tool_cost * (1.0 - HAIKU_RATIO_OF_OPUS);
            let pct = (tool_ratio * 100.0).round() as i64;
            let msg = format!(
                "{pct}% tool_use tokens \u{2014} Haiku could save ~${:.2}",
                savings
            );
            findings.push(Finding {
                severity: Severity::Warn,
                message: msg,
                savings_usd: savings,
            });
            total_savings += savings;
        }
    }

    let cache_create = session.tokens.cache_write_5m + session.tokens.cache_write_1h;
    let denom = cache_create + session.tokens.cache_read;
    if denom >= 1_000 {
        let ratio = session.tokens.cache_read as f64 / denom as f64;
        if ratio < CACHE_EFFICIENCY_THRESHOLD {
            let savings = session.total_cost_usd * 0.20;
            let pct = (ratio * 100.0).round() as i64;
            let msg = format!("cache hit rate {pct}% \u{2014} consider session continuation");
            findings.push(Finding {
                severity: Severity::Info,
                message: msg,
                savings_usd: savings,
            });
            total_savings += savings;
        }
    }

    let total_tokens = session.tokens.total();
    if is_opus_family(&session.model_summary)
        && total_tokens < SMALL_SESSION_THRESHOLD_TOKENS
        && session.total_cost_usd > 0.05
    {
        let savings = session.total_cost_usd * (1.0 - SONNET_RATIO_OF_OPUS);
        let msg = format!(
            "model: opus \u{00B7} {}k tokens \u{2014} Sonnet would suffice (save ~${:.2})",
            total_tokens / 1000,
            savings,
        );
        findings.push(Finding {
            severity: Severity::Info,
            message: msg,
            savings_usd: savings,
        });
        total_savings += savings;
    }

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
        let tokens = TokenCounts {
            input: 10_000,
            output: 10_000,
            ..Default::default()
        };
        let session = mk_session("a", "claude-opus-4-7", 10.0, tokens);
        let finding =
            audit_session_with_stats(&session, 20_000, 16_000, "proj".into()).expect("should flag");
        assert!(finding
            .findings
            .iter()
            .any(|f| f.message.contains("tool_use tokens")));
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
        let finding = audit_session_with_stats(&session, 20_000, 16_000, "proj".into());
        // Haiku sessions might trip *other* rules but never the tool-ratio one,
        // since we'd be recommending the model they already use.
        if let Some(f) = finding {
            for item in &f.findings {
                assert!(
                    !item.message.contains("tool_use tokens"),
                    "Haiku sessions should never get the tool-ratio flag"
                );
            }
        }
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
        let finding =
            audit_session_with_stats(&session, 2_000, 0, "proj".into()).expect("should flag");
        assert!(finding
            .findings
            .iter()
            .any(|f| f.message.contains("cache hit rate")));
    }

    #[test]
    fn small_opus_session_flagged_as_model_mismatch() {
        let tokens = TokenCounts {
            input: 500,
            output: 500,
            ..Default::default()
        };
        let session = mk_session("a", "claude-opus-4-7", 0.37, tokens);
        let finding =
            audit_session_with_stats(&session, 1_000, 0, "proj".into()).expect("should flag");
        assert!(finding
            .findings
            .iter()
            .any(|f| f.message.contains("Sonnet would suffice")));
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
}
