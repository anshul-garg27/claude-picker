//! Cross-verification of `src/data/pricing.rs` against Claude Code's own
//! per-session `costUSD` values.
//!
//! Claude Code writes `~/.claude.json` with a `projects[<cwd>].lastModelUsage`
//! dict per project. Each entry has:
//!
//! ```jsonc
//! "claude-sonnet-4-5-20250929": {
//!   "inputTokens": 264,
//!   "outputTokens": 8593,
//!   "cacheReadInputTokens": 730434,
//!   "cacheCreationInputTokens": 70393,
//!   "webSearchRequests": 0,
//!   "costUSD": 0.6127909500000001
//! }
//! ```
//!
//! That's Claude's authoritative per-model cost for the last session run
//! under that project. If our pricing table is correct, `pricing::cost_for`
//! should return a number within 5 % of `costUSD` for every model.
//!
//! We run the test only when `~/.claude.json` exists AND has at least one
//! project with non-zero `lastCost`; otherwise we `return` early so CI
//! boxes without a real Claude install still pass.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use claude_picker::data::pricing::{cost_for, TokenCounts};

#[derive(Debug, Deserialize)]
struct ClaudeJson {
    #[serde(default)]
    projects: HashMap<String, Project>,
}

#[derive(Debug, Deserialize)]
struct Project {
    #[serde(default, rename = "lastCost")]
    last_cost: f64,
    #[serde(default, rename = "lastSessionId")]
    last_session_id: Option<String>,
    #[serde(default, rename = "lastModelUsage")]
    last_model_usage: Option<HashMap<String, ModelUsage>>,
}

#[derive(Debug, Deserialize)]
struct ModelUsage {
    #[serde(default, rename = "inputTokens")]
    input_tokens: u64,
    #[serde(default, rename = "outputTokens")]
    output_tokens: u64,
    #[serde(default, rename = "cacheReadInputTokens")]
    cache_read_input_tokens: u64,
    #[serde(default, rename = "cacheCreationInputTokens")]
    cache_creation_input_tokens: u64,
    #[serde(default, rename = "costUSD")]
    cost_usd: f64,
}

fn load_claude_json() -> Option<ClaudeJson> {
    let home = dirs::home_dir()?;
    let path = home.join(".claude.json");
    if !path.exists() {
        return None;
    }
    read_claude_json(&path).ok()
}

fn read_claude_json(path: &Path) -> anyhow::Result<ClaudeJson> {
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

/// Test the per-model per-project per-session cost delta.
///
/// Iterates every `projects[<cwd>].lastModelUsage[<model>]` entry and
/// asserts `|cost_for() - costUSD| / costUSD < 5%` (with a $0.01
/// absolute floor so rounding doesn't trip us up on tiny sessions).
///
/// Collects every delta first and asserts AT THE END so one bad model
/// doesn't hide the rest. Silently skips hosts where Claude isn't
/// installed.
#[test]
fn pricing_matches_claude_reported_costs_within_5_percent() {
    let Some(data) = load_claude_json() else {
        eprintln!("skip: ~/.claude.json not readable; this is expected on CI");
        return;
    };

    // `tolerance_frac` = 5 %. `tolerance_abs` = 1 cent.
    let tolerance_frac = 0.05;
    let tolerance_abs = 0.01;

    let mut checked = 0usize;
    let mut mismatches: Vec<String> = Vec::new();

    for (cwd, proj) in &data.projects {
        if proj.last_cost <= 0.0 {
            continue;
        }
        let Some(usage) = proj.last_model_usage.as_ref() else {
            continue;
        };
        for (model, u) in usage {
            if u.cost_usd <= 0.0 {
                continue;
            }
            // `.claude.json` reports cache-creation as a single bucket
            // (`cacheCreationInputTokens`), so feed it into `cache_write_5m`
            // which is the same as the legacy field our pricing already
            // handles.
            let tokens = TokenCounts {
                input: u.input_tokens,
                output: u.output_tokens,
                cache_read: u.cache_read_input_tokens,
                cache_write_5m: u.cache_creation_input_tokens,
                cache_write_1h: 0,
            };
            let ours = cost_for(model, tokens);
            let theirs = u.cost_usd;
            let delta = (ours - theirs).abs();
            let rel = if theirs.abs() > 0.0 {
                delta / theirs.abs()
            } else {
                delta
            };
            checked += 1;
            if rel > tolerance_frac && delta > tolerance_abs {
                mismatches.push(format!(
                    "  {cwd} / {model}: ours={:.6}, theirs={:.6}, delta={:.6} ({:.2}%)",
                    ours,
                    theirs,
                    delta,
                    rel * 100.0,
                ));
            }
        }
        // Report the session id so a developer can grep the JSONL.
        let _ = proj.last_session_id.as_ref();
    }

    if checked == 0 {
        eprintln!("skip: no project with per-model cost in ~/.claude.json");
        return;
    }

    eprintln!("pricing cross-verification: checked {checked} model/project cells");
    if !mismatches.is_empty() {
        panic!(
            "pricing table disagrees with Claude Code's reported costs:\n{}",
            mismatches.join("\n"),
        );
    }
}

#[test]
fn pricing_accuracy_smoke_test_known_values() {
    // Sanity anchor — these are the exact values I cross-verified by hand
    // against a real ~/.claude.json on 2026-04-16. Not dependent on the
    // filesystem so they run in CI too.

    // Case 1: Haiku 4.5 @ 18871 in + 574 out = $0.021741
    let t = TokenCounts {
        input: 18871,
        output: 574,
        cache_read: 0,
        cache_write_5m: 0,
        cache_write_1h: 0,
    };
    let ours = cost_for("claude-haiku-4-5-20251001", t);
    let theirs = 0.021_741;
    assert!(
        (ours - theirs).abs() < 1e-6,
        "haiku-4-5 cost mismatch: ours={ours}, theirs={theirs}",
    );

    // Case 2: Sonnet 4.5 with the full cache split.
    //   264 input + 8593 output + 730434 cache_read + 70393 cache_create
    //   → $0.6127909500000001
    let t = TokenCounts {
        input: 264,
        output: 8593,
        cache_read: 730_434,
        cache_write_5m: 70_393,
        cache_write_1h: 0,
    };
    let ours = cost_for("claude-sonnet-4-5-20250929", t);
    let theirs = 0.612_790_95;
    assert!(
        (ours - theirs).abs() < 1e-6,
        "sonnet-4-5 cost mismatch: ours={ours}, theirs={theirs}",
    );

    // Case 3: Opus 4.6[1m] big session.
    let t = TokenCounts {
        input: 133_772,
        output: 497_835,
        cache_read: 96_238_279,
        cache_write_5m: 6_645_487,
        cache_write_1h: 0,
    };
    let ours = cost_for("claude-opus-4-6[1m]", t);
    let theirs = 102.768_168_25;
    assert!(
        (ours - theirs).abs() / theirs < 0.001,
        "opus-4-6 cost mismatch: ours={ours}, theirs={theirs}",
    );
}
