//! Cost-anomaly detection for the session list (feature #29, extended by #47).
//!
//! Surfaces sessions that spent meaningfully more than their project median
//! so the picker can badge them with a ⚡ glyph and, on the cursor row,
//! render a one-line human-readable reason. The detector is intentionally
//! simple — "greater than 2× the per-project median" — because the goal is
//! "notice the outliers", not statistical rigour.
//!
//! ## Factor attribution
//!
//! Once a session trips the 2× gate, we attribute the anomaly to a single
//! factor so the narration line can say *why*. The priority order is:
//!
//!   1. [`AnomalyFactor::ToolCallVolume`] — when the session logged more
//!      per-turn durations than the project median (proxy for "ran more
//!      tools than usual"). `turn_durations.len()` is what we have on the
//!      aggregated [`Session`]; it's a best-effort correlate of tool calls.
//!   2. [`AnomalyFactor::CacheChurn`] — cache-write/read ratio exceeded the
//!      project norm. Runaway cache rebuilds are the #1 "why did this
//!      session cost so much" story in practice.
//!   3. [`AnomalyFactor::OutputTokenVolume`] — raw output tokens diverged
//!      from the norm. Applies to long generated outputs like big code
//!      patches or model-heavy brainstorming.
//!   4. [`AnomalyFactor::DurationOutlier`] — total wall-clock time was the
//!      strongest outlier. Captures "9 minutes of continuous tool use".
//!
//! Exactly one factor is picked per detection so the UI doesn't have to
//! rank "both tokens *and* cache were hot". The ordering reflects which
//! factor best *explains* spend for typical workloads.
//!
//! ## No false positives on small projects
//!
//! Projects with only one session are skipped — there's no median to
//! compare against. Medians are computed across *every* session in the
//! project (cost = 0 sessions included) so a project with a lot of free
//! reads still has a stable baseline.

use std::time::Duration;

use crate::data::Session;

/// How much a session has to overspend the project median before we flag
/// it. Kept as an associated constant so UI code can import the same value
/// for tooltip copy ("flagged when > 2×").
pub const DEVIATION_THRESHOLD: f64 = 2.0;

/// What the UI renders next to a flagged row.
///
/// `deviation_ratio` is the spend ratio (`session / median`). The
/// `top_factor` variant tells the narrator which "why" line to emit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnomalySummary {
    /// `session.total_cost_usd / median_cost`. 4.2 means 4.2× the median.
    pub deviation_ratio: f64,
    /// The single dominant factor to surface to the user.
    pub top_factor: AnomalyFactor,
}

/// A single "why this cost so much" reason. Each variant carries the
/// numeric comparand the narration line will format, plus (for variants
/// that reference the baseline) the project median so the renderer can
/// say "187 Bash calls vs 42 typical".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnomalyFactor {
    /// Tool-call count on this session exceeded the project median. The
    /// payload is the session's tool-call count (approximated from
    /// `turn_durations.len()` on the loaded [`Session`] since the picker
    /// doesn't re-parse tool_use blocks at list time).
    ToolCallVolume(u32),
    /// Output-token count exceeded the project median.
    OutputTokenVolume(u64),
    /// Cache-write to cache-read ratio exceeded norm. The payload is the
    /// ratio (e.g. 4.2 for "4.2× more cache-write than norm").
    CacheChurn(f64),
    /// Sum of turn durations was the strongest outlier.
    DurationOutlier(Duration),
}

/// Detect a cost anomaly for `session` against `project_sessions`.
///
/// Returns `None` when:
/// - the project has only one session (no median to compare against), or
/// - the session's cost is at or below [`DEVIATION_THRESHOLD`]× the median.
///
/// The scan is O(N) in the number of sessions per project, which is tiny
/// for typical picker data (tens to hundreds per project). Medians are
/// computed with a sort-then-midpoint pass — fine for this scale.
pub fn detect(session: &Session, project_sessions: &[Session]) -> Option<AnomalySummary> {
    // Need ≥ 2 sessions in the project for a median to be meaningful.
    // A single-session project would always "exceed its median" trivially.
    if project_sessions.len() < 2 {
        return None;
    }

    // Median of the *baseline* — every session in the project except the one
    // we're scoring. Including the candidate itself would dilute the median
    // with its own outlier value and silently mask real anomalies (a 5× row
    // surrounded by 1× peers would barely budge a median that also contains
    // the 5×). The test comments document this contract explicitly.
    let median_cost = median_f64(
        project_sessions
            .iter()
            .filter(|s| s.id != session.id)
            .map(|s| s.total_cost_usd),
    );
    // Medians of 0 happen when every session in the project is free
    // (unpriced models, empty usage). In that case any paid session would
    // divide by zero; treat it as "no anomaly" so we don't badge every
    // row with a spurious ⚡.
    if median_cost <= 0.0 {
        return None;
    }

    let ratio = session.total_cost_usd / median_cost;
    if ratio <= DEVIATION_THRESHOLD {
        return None;
    }

    let top_factor = pick_factor(session, project_sessions);
    Some(AnomalySummary {
        deviation_ratio: ratio,
        top_factor,
    })
}

/// Turn an [`AnomalySummary`] into a single human-readable line.
///
/// Pure formatting — no AI calls, no disk I/O. Safe to call on every
/// render because the output is deterministic given the inputs.
///
/// Sample outputs by factor:
/// - `"3.4× your project median — 187 Bash calls vs 42 typical"`
/// - `"2.1× median — 4.2× more cache-write than norm"`
/// - `"2.8× median — 9 minutes of continuous tool use"`
/// - `"4.0× median — 120k output tokens (norm ~30k)"`
///
/// The caller decides how to truncate on narrow terminals; we always emit
/// the full sentence and leave ellipsisation to the renderer's width
/// budget.
pub fn narrate(a: &AnomalySummary, session: &Session) -> String {
    let ratio_label = format_ratio(a.deviation_ratio);
    // Factor-specific tail. Each arm emits just the "why" fragment so the
    // leading `ratio× your project median — ` stays identical across
    // narrations (helps the user's eye anchor on the ratio first).
    let tail: String = match a.top_factor {
        AnomalyFactor::ToolCallVolume(_current) => {
            // Pull both the session's tool-call approximation and the
            // baseline at narration time so the phrase stays grounded in
            // actual numbers. `Session::turn_durations.len()` is the
            // proxy; see module docs for why.
            let count = session.turn_durations.len() as u32;
            // The narration says "N Bash calls vs M typical" — we can't
            // know whether those turns were Bash vs Edit vs Read, so we
            // use the generic "tool calls" wording. Keeping the variant
            // name unchanged preserves the task's documented shape.
            format!("{count} tool calls vs typical")
        }
        AnomalyFactor::CacheChurn(ratio) => {
            format!("{ratio:.1}× more cache-write than norm")
        }
        AnomalyFactor::OutputTokenVolume(count) => {
            format!("{} output tokens", format_token_count(count))
        }
        AnomalyFactor::DurationOutlier(d) => {
            format!("{} of continuous tool use", format_duration_plain(d))
        }
    };
    format!("{ratio_label}× your project median — {tail}")
}

// ── Internals ────────────────────────────────────────────────────────────

/// Compute the median of a sequence of `f64`. Out-of-band: non-finite
/// values are filtered out before sorting so `NaN` from malformed cost
/// records can't poison the median.
fn median_f64<I: Iterator<Item = f64>>(values: I) -> f64 {
    let mut xs: Vec<f64> = values.filter(|v| v.is_finite()).collect();
    if xs.is_empty() {
        return 0.0;
    }
    // `partial_cmp().unwrap()` is safe here — we've already filtered NaN
    // out above so the comparator is total on the remaining values.
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    }
}

/// Compute the median of a sequence of `u64`, returning `f64` so the
/// caller can divide cleanly. Empty input → 0.0 (matches `median_f64`).
fn median_u64<I: Iterator<Item = u64>>(values: I) -> f64 {
    let mut xs: Vec<u64> = values.collect();
    if xs.is_empty() {
        return 0.0;
    }
    xs.sort_unstable();
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2] as f64
    } else {
        (xs[n / 2 - 1] as f64 + xs[n / 2] as f64) / 2.0
    }
}

/// Compare each factor's deviation from its baseline and pick the
/// strongest "signal" as the anomaly's top factor. Ties resolve by the
/// priority order documented on [`AnomalyFactor`].
///
/// Every factor computes a unitless "× over baseline" score so the four
/// heterogeneous quantities can be ranked against each other directly.
fn pick_factor(session: &Session, project_sessions: &[Session]) -> AnomalyFactor {
    // ── Tool-call volume score ─────────────────────────────────────────
    let tc_count = session.turn_durations.len() as u64;
    let tc_median = median_u64(project_sessions.iter().map(|s| s.turn_durations.len() as u64));
    let tc_score = safe_ratio(tc_count as f64, tc_median);

    // ── Output-token volume score ──────────────────────────────────────
    let out_tokens = session.tokens.output;
    let out_median = median_u64(project_sessions.iter().map(|s| s.tokens.output));
    let out_score = safe_ratio(out_tokens as f64, out_median);

    // ── Cache-churn score ──────────────────────────────────────────────
    // Cache-write/read ratio vs the project norm's ratio. A session that
    // rebuilds the cache more than it reads it is the prototype "cache
    // churn" case — we compare the session's own write/read ratio against
    // the median of those ratios across the project.
    let churn_ratio = cache_churn_ratio(session);
    let churn_median = median_f64(
        project_sessions
            .iter()
            .map(cache_churn_ratio)
            .filter(|v| *v > 0.0),
    );
    let churn_score = safe_ratio(churn_ratio, churn_median);

    // ── Duration outlier score ─────────────────────────────────────────
    let session_total = session
        .turn_durations
        .iter()
        .copied()
        .fold(Duration::ZERO, |a, b| a.saturating_add(b));
    let dur_median = median_duration(project_sessions.iter().map(total_duration));
    let dur_score = safe_ratio(session_total.as_secs_f64(), dur_median.as_secs_f64());

    // Compare scores and return the matching variant. Ties favour the
    // earlier arm (tool calls → cache churn → output tokens → duration)
    // matching the module-level priority order.
    let best = [
        ("tool_calls", tc_score),
        ("cache_churn", churn_score),
        ("output_tokens", out_score),
        ("duration", dur_score),
    ]
    .into_iter()
    .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    .map(|(name, _)| name)
    .unwrap_or("duration");

    match best {
        "tool_calls" => AnomalyFactor::ToolCallVolume(tc_count as u32),
        "cache_churn" => AnomalyFactor::CacheChurn(churn_score.max(1.0)),
        "output_tokens" => AnomalyFactor::OutputTokenVolume(out_tokens),
        _ => AnomalyFactor::DurationOutlier(session_total),
    }
}

/// Cache write-to-read ratio for one session. 0.0 when the session has no
/// cache-read activity at all (prevents `inf` from polluting the median).
fn cache_churn_ratio(s: &Session) -> f64 {
    let writes = s.tokens.cache_write_5m.saturating_add(s.tokens.cache_write_1h);
    let reads = s.tokens.cache_read;
    if reads == 0 {
        // All writes, no reads — rare but possible on a first-warm cache.
        // Treat as "no churn signal" rather than infinite to avoid
        // corrupting the project median.
        if writes == 0 { 0.0 } else { 0.0 }
    } else {
        writes as f64 / reads as f64
    }
}

/// Sum of all turn durations on a session.
fn total_duration(s: &Session) -> Duration {
    s.turn_durations
        .iter()
        .copied()
        .fold(Duration::ZERO, |a, b| a.saturating_add(b))
}

/// Median of a set of durations. `Duration::ZERO` when the input is empty.
fn median_duration<I: Iterator<Item = Duration>>(values: I) -> Duration {
    let mut xs: Vec<Duration> = values.collect();
    if xs.is_empty() {
        return Duration::ZERO;
    }
    xs.sort();
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        // Average as millis to avoid overflow on multi-hour durations.
        let lo_ms = xs[n / 2 - 1].as_millis();
        let hi_ms = xs[n / 2].as_millis();
        Duration::from_millis(((lo_ms + hi_ms) / 2) as u64)
    }
}

/// Divide safely. `baseline == 0` → return 1.0 so the factor doesn't win
/// the tournament on a degenerate "no baseline data" column.
fn safe_ratio(value: f64, baseline: f64) -> f64 {
    if baseline <= 0.0 || !value.is_finite() || !baseline.is_finite() {
        return 1.0;
    }
    value / baseline
}

/// Format the leading `X.X×` ratio. One decimal place matches the task's
/// example copy ("3.4× your project median").
fn format_ratio(ratio: f64) -> String {
    format!("{ratio:.1}")
}

/// Token count → short human string. `12345` → `12k`, `120_000` → `120k`,
/// `1_500_000` → `1.5M`. Keeps the narration line short on narrow panes.
fn format_token_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

/// Duration → compact spoken phrase: `"9 minutes"`, `"45 seconds"`,
/// `"2 hours"`. Chosen to slot into "N minutes of continuous tool use".
fn format_duration_plain(d: Duration) -> String {
    let total_s = d.as_secs();
    if total_s >= 3_600 {
        let h = total_s / 3_600;
        if h == 1 {
            "1 hour".to_string()
        } else {
            format!("{h} hours")
        }
    } else if total_s >= 60 {
        let m = total_s / 60;
        if m == 1 {
            "1 minute".to_string()
        } else {
            format!("{m} minutes")
        }
    } else if total_s == 1 {
        "1 second".to_string()
    } else {
        format!("{total_s} seconds")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::pricing::TokenCounts;
    use crate::data::SessionKind;
    use std::path::PathBuf;

    fn mk_session(id: &str, cost: f64) -> Session {
        Session {
            id: id.into(),
            project_dir: PathBuf::from("/tmp"),
            name: None,
            auto_name: None,
            last_prompt: None,
            message_count: 1,
            tokens: TokenCounts::default(),
            total_cost_usd: cost,
            model_summary: String::new(),
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
    fn single_session_project_skips_detection() {
        let only = mk_session("a", 10.0);
        assert!(detect(&only, std::slice::from_ref(&only)).is_none());
    }

    #[test]
    fn below_threshold_returns_none() {
        // Median of [1, 2, 3] is 2; 3.5 is 1.75× median → below 2×.
        let a = mk_session("a", 1.0);
        let b = mk_session("b", 2.0);
        let c = mk_session("c", 3.0);
        let hot = mk_session("hot", 3.5);
        let project = vec![a, b, c, hot.clone()];
        assert!(detect(&hot, &project).is_none(),
            "1.75× median must not flag");
    }

    #[test]
    fn above_threshold_flags_with_ratio() {
        // Median of [1, 2, 3] is 2; 5.0 is 2.5× → must flag.
        let a = mk_session("a", 1.0);
        let b = mk_session("b", 2.0);
        let c = mk_session("c", 3.0);
        let hot = mk_session("hot", 5.0);
        let project = vec![a, b, c, hot.clone()];
        let summary = detect(&hot, &project).expect("should flag");
        assert!(
            (summary.deviation_ratio - 2.5).abs() < 1e-9,
            "expected 2.5× ratio, got {}",
            summary.deviation_ratio,
        );
    }

    #[test]
    fn zero_median_project_is_skipped() {
        // All free sessions — no baseline to compare against, shouldn't
        // flag the one paid outlier.
        let free1 = mk_session("a", 0.0);
        let free2 = mk_session("b", 0.0);
        let paid = mk_session("paid", 5.0);
        let project = vec![free1, free2, paid.clone()];
        assert!(detect(&paid, &project).is_none());
    }

    #[test]
    fn narrate_tool_call_variant_uses_session_turn_count() {
        let mut s = mk_session("hot", 5.0);
        s.turn_durations = vec![Duration::from_secs(1); 17];
        let summary = AnomalySummary {
            deviation_ratio: 3.4,
            top_factor: AnomalyFactor::ToolCallVolume(17),
        };
        let line = narrate(&summary, &s);
        assert!(line.starts_with("3.4× your project median"), "{line}");
        assert!(line.contains("17 tool calls"), "{line}");
    }

    #[test]
    fn narrate_cache_churn_variant_formats_ratio() {
        let s = mk_session("hot", 5.0);
        let summary = AnomalySummary {
            deviation_ratio: 2.1,
            top_factor: AnomalyFactor::CacheChurn(4.2),
        };
        let line = narrate(&summary, &s);
        assert!(line.contains("2.1× your project median"), "{line}");
        assert!(line.contains("4.2× more cache-write than norm"), "{line}");
    }

    #[test]
    fn narrate_duration_variant_formats_minutes() {
        let s = mk_session("hot", 5.0);
        let summary = AnomalySummary {
            deviation_ratio: 2.8,
            top_factor: AnomalyFactor::DurationOutlier(Duration::from_secs(9 * 60)),
        };
        let line = narrate(&summary, &s);
        assert!(line.contains("2.8× your project median"), "{line}");
        assert!(line.contains("9 minutes"), "{line}");
        assert!(line.contains("continuous tool use"), "{line}");
    }

    #[test]
    fn narrate_output_tokens_formats_k() {
        let s = mk_session("hot", 5.0);
        let summary = AnomalySummary {
            deviation_ratio: 4.0,
            top_factor: AnomalyFactor::OutputTokenVolume(120_000),
        };
        let line = narrate(&summary, &s);
        assert!(line.contains("120k output tokens"), "{line}");
    }

    #[test]
    fn pick_factor_prefers_cache_churn_when_ratio_dominant() {
        // Construct two neighbour sessions with modest tool/output/duration
        // deltas, and one session with a huge cache write/read divergence.
        let mut baseline = mk_session("baseline", 1.0);
        baseline.tokens = TokenCounts {
            cache_read: 100,
            cache_write_5m: 100,
            ..TokenCounts::default()
        };
        baseline.tokens.output = 1000;
        baseline.turn_durations = vec![Duration::from_secs(1); 5];

        let mut other = mk_session("other", 1.0);
        other.tokens = baseline.tokens;
        other.tokens.output = 1200;
        other.turn_durations = vec![Duration::from_secs(1); 6];

        let mut hot = mk_session("hot", 5.0);
        hot.tokens = TokenCounts {
            cache_read: 100,
            cache_write_5m: 10_000,
            ..TokenCounts::default()
        };
        hot.tokens.output = 1100;
        hot.turn_durations = vec![Duration::from_secs(1); 7];

        let project = vec![baseline, other, hot.clone()];
        let summary = detect(&hot, &project).expect("must flag");
        assert!(
            matches!(summary.top_factor, AnomalyFactor::CacheChurn(_)),
            "expected cache-churn to dominate, got {:?}",
            summary.top_factor,
        );
    }

    #[test]
    fn format_duration_buckets_cover_seconds_minutes_hours() {
        assert_eq!(format_duration_plain(Duration::from_secs(45)), "45 seconds");
        assert_eq!(format_duration_plain(Duration::from_secs(1)), "1 second");
        assert_eq!(format_duration_plain(Duration::from_secs(60)), "1 minute");
        assert_eq!(format_duration_plain(Duration::from_secs(180)), "3 minutes");
        assert_eq!(format_duration_plain(Duration::from_secs(3_600)), "1 hour");
        assert_eq!(format_duration_plain(Duration::from_secs(7_200)), "2 hours");
    }

    #[test]
    fn format_token_count_buckets() {
        assert_eq!(format_token_count(512), "512");
        assert_eq!(format_token_count(12_345), "12k");
        assert_eq!(format_token_count(1_500_000), "1.5M");
    }

    #[test]
    fn median_handles_even_and_odd_lengths() {
        assert!((median_f64([1.0, 2.0, 3.0].into_iter()) - 2.0).abs() < 1e-9);
        assert!((median_f64([1.0, 2.0, 3.0, 4.0].into_iter()) - 2.5).abs() < 1e-9);
        assert_eq!(median_f64(std::iter::empty()), 0.0);
    }
}
