//! Session chain detection — group sessions that look like the same feature
//! continued across multiple runs.
//!
//! Feature #39 / DEEP-24. A "chain" is a run of sessions, all rooted in the
//! same project directory, where each consecutive pair sits within 24h of
//! each other **and** looks topically similar (title overlap, or matching
//! model family as a weak tie-breaker for very short titles).
//!
//! Singletons are intentionally filtered out — a chain means ≥2 members. The
//! [`chain_for_session`] helper lets the UI answer "is this row part of a
//! chain?" in O(N) across the chain list, which is fine for the typical
//! scale of a picker session list (hundreds, not millions).
//!
//! The matching heuristic is deliberately simple: we tokenize each title,
//! strip a tiny stopword list, and compute a shared-word ratio over the
//! longer side so "fix auth bug" and "auth fix crash" both feel
//! "mostly the same feature" without needing to name it the same way.

use chrono::{DateTime, Duration, Utc};

use crate::data::Session;

/// Stopwords dropped from the shared-word similarity tokenizer. Kept tiny on
/// purpose — the point is to discard glue words that make titles look more
/// alike than they are, not to build a proper NLP stopword list.
const STOPWORDS: &[&str] = &[
    "the", "a", "and", "or", "to", "for", "fix", "add", "update", "refactor", "in", "on", "of",
];

/// Minimum shared-word ratio for two titles to count as "about the same
/// feature". Tuned against the spec examples (see tests).
const TITLE_SIMILARITY_THRESHOLD: f32 = 0.6;

/// Maximum gap between one session's `last_timestamp` and the next session's
/// `first_timestamp` for them to still belong to the same chain.
const CHAIN_GAP: Duration = Duration::hours(24);

/// One detected chain — an ordered run of session ids belonging to the same
/// project cwd, each temporally adjacent to the previous member.
#[derive(Debug, Clone)]
pub struct Chain {
    /// Session ids in chronological order (earliest `first_timestamp` first).
    pub members: Vec<String>,
    /// Sum of `total_cost_usd` across every member.
    pub total_cost_usd: f64,
    /// Earliest `first_timestamp` across members. Falls back to
    /// `last_timestamp` if `first_timestamp` was missing.
    pub first_ts: DateTime<Utc>,
    /// Latest `last_timestamp` across members. Falls back to
    /// `first_timestamp` if `last_timestamp` was missing.
    pub last_ts: DateTime<Utc>,
}

/// Group `sessions` into chains. Returns only chains with ≥2 members.
///
/// Sessions without any timestamp are skipped outright — chain detection is a
/// temporal grouping and there's nothing sensible we can do with a session
/// that never recorded a time.
pub fn detect_chains(sessions: &[Session]) -> Vec<Chain> {
    if sessions.is_empty() {
        return Vec::new();
    }

    // Snapshot each session into a small stack-friendly record. We need to
    // re-sort by project then timestamp, which is easier on a flat vec of
    // copies than on a `&[Session]` plus parallel index vec.
    let mut candidates: Vec<Candidate> = sessions
        .iter()
        .filter_map(Candidate::from_session)
        .collect();

    // Sort by (project_key, first_ts) so adjacent candidates with the same
    // project form a single run we can walk in one pass.
    candidates.sort_by(|a, b| {
        a.project_key
            .cmp(&b.project_key)
            .then(a.first_ts.cmp(&b.first_ts))
    });

    let mut chains: Vec<Chain> = Vec::new();
    let mut current: Vec<Candidate> = Vec::new();

    for cand in candidates.into_iter() {
        let extend_current = current
            .last()
            .map(|prev| is_chainable(prev, &cand))
            .unwrap_or(false);

        if extend_current {
            current.push(cand);
        } else {
            flush_chain(&mut current, &mut chains);
            current.push(cand);
        }
    }
    flush_chain(&mut current, &mut chains);

    chains
}

/// Membership lookup: which chain (if any) contains this session id?
///
/// Linear scan over `chains` and their members — fine at picker scale
/// (dozens of chains, a handful of members each).
pub fn chain_for_session<'a>(id: &str, chains: &'a [Chain]) -> Option<&'a Chain> {
    chains
        .iter()
        .find(|chain| chain.members.iter().any(|m| m == id))
}

// ────────────────────────────────────────────────────────────────────────────
// Internals
// ────────────────────────────────────────────────────────────────────────────

/// A lightweight per-session record with just the bits the chaining pass
/// cares about. Using owned strings keeps the detection pass independent of
/// the `Session`'s lifetime, which simplifies the sort-and-group logic.
struct Candidate {
    id: String,
    project_key: String,
    title_tokens: Vec<String>,
    model_family: Option<String>,
    first_ts: DateTime<Utc>,
    last_ts: DateTime<Utc>,
    total_cost_usd: f64,
}

impl Candidate {
    fn from_session(s: &Session) -> Option<Self> {
        // We need at least one timestamp to be able to place the session on
        // the temporal axis; fall back between first/last if only one is set.
        let first_ts = s.first_timestamp.or(s.last_timestamp)?;
        let last_ts = s.last_timestamp.or(s.first_timestamp)?;
        Some(Self {
            id: s.id.clone(),
            project_key: s.project_dir.to_string_lossy().into_owned(),
            title_tokens: normalize_title(s.display_label()),
            model_family: model_family(&s.model_summary),
            first_ts,
            last_ts,
            total_cost_usd: s.total_cost_usd,
        })
    }
}

fn flush_chain(current: &mut Vec<Candidate>, chains: &mut Vec<Chain>) {
    if current.len() < 2 {
        current.clear();
        return;
    }
    // Order members by their first_ts (already sorted on the way in, but the
    // caller relies on the guarantee, so assert via an explicit sort).
    current.sort_by_key(|c| c.first_ts);
    let first_ts = current
        .iter()
        .map(|c| c.first_ts)
        .min()
        .expect("non-empty");
    let last_ts = current.iter().map(|c| c.last_ts).max().expect("non-empty");
    let total_cost_usd = current.iter().map(|c| c.total_cost_usd).sum();
    let members = current.iter().map(|c| c.id.clone()).collect();
    chains.push(Chain {
        members,
        total_cost_usd,
        first_ts,
        last_ts,
    });
    current.clear();
}

/// Two candidates belong to the same chain if they share a project, land
/// within 24h of each other, **and** at least one topical signal agrees
/// (shared title tokens, or matching model family as a weak tie-breaker).
fn is_chainable(prev: &Candidate, next: &Candidate) -> bool {
    if prev.project_key != next.project_key {
        return false;
    }
    let gap = next.first_ts.signed_duration_since(prev.last_ts);
    if gap > CHAIN_GAP || gap < -CHAIN_GAP {
        return false;
    }
    if title_similarity(&prev.title_tokens, &next.title_tokens) > TITLE_SIMILARITY_THRESHOLD {
        return true;
    }
    matches!(
        (&prev.model_family, &next.model_family),
        (Some(a), Some(b)) if a == b
    )
}

/// Shared-word ratio over the longer side: `shared / max(len_a, len_b)`.
///
/// Returns 0.0 when either side has no non-stopword tokens — we can't make a
/// meaningful judgement on the title in that case and the model-family
/// fallback can still rescue the pairing.
fn title_similarity(a: &[String], b: &[String]) -> f32 {
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 0.0;
    }
    let shared = a.iter().filter(|t| b.contains(t)).count();
    shared as f32 / max_len as f32
}

/// Normalize a title into lowercase alphanumeric tokens, minus stopwords.
///
/// Exposed at module scope so callers (and tests) can share the same
/// tokenizer the heuristic itself uses.
fn normalize_title(t: &str) -> Vec<String> {
    t.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOPWORDS.contains(w))
        .map(|s| s.to_string())
        .collect()
}

/// Collapse a model id like `"claude-opus-4-7"` to its family (`"opus"`) for
/// the weak tie-breaker signal. Returns `None` when no known family string is
/// present — an unknown model means we can't use the fallback, which is the
/// conservative choice.
fn model_family(model: &str) -> Option<String> {
    let lower = model.to_lowercase();
    for fam in ["opus", "sonnet", "haiku"] {
        if lower.contains(fam) {
            return Some(fam.to_string());
        }
    }
    None
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration as StdDuration;

    use chrono::TimeZone;

    use crate::data::pricing::TokenCounts;
    use crate::data::session::{PermissionMode, SessionKind};

    fn make_session(
        id: &str,
        project: &str,
        name: Option<&str>,
        model: &str,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
    ) -> Session {
        Session {
            id: id.to_string(),
            project_dir: PathBuf::from(project),
            name: name.map(|s| s.to_string()),
            auto_name: None,
            last_prompt: None,
            message_count: 0,
            tokens: TokenCounts::default(),
            total_cost_usd: 0.0,
            model_summary: model.to_string(),
            first_timestamp: Some(first),
            last_timestamp: Some(last),
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: Some(PermissionMode::Default),
            subagent_count: 0,
            turn_durations: Vec::<StdDuration>::new(),
        }
    }

    fn ts(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0).unwrap()
    }

    #[test]
    fn detect_chains_no_sessions_returns_empty() {
        let chains = detect_chains(&[]);
        assert!(chains.is_empty());
    }

    #[test]
    fn detect_chains_pairs_same_project_same_title_within_window() {
        let a = make_session(
            "a",
            "/proj/alpha",
            Some("fix auth bug"),
            "claude-opus-4-7",
            ts(2026, 4, 20, 10),
            ts(2026, 4, 20, 11),
        );
        let b = make_session(
            "b",
            "/proj/alpha",
            Some("fix auth bug"),
            "claude-opus-4-7",
            ts(2026, 4, 20, 13),
            ts(2026, 4, 20, 14),
        );
        let chains = detect_chains(&[a, b]);
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].members, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn detect_chains_rejects_wide_temporal_gap() {
        let a = make_session(
            "a",
            "/proj/alpha",
            Some("fix auth bug"),
            "claude-opus-4-7",
            ts(2026, 4, 18, 10),
            ts(2026, 4, 18, 11),
        );
        let b = make_session(
            "b",
            "/proj/alpha",
            Some("fix auth bug"),
            "claude-opus-4-7",
            // 48h later — outside the 24h window.
            ts(2026, 4, 20, 11),
            ts(2026, 4, 20, 12),
        );
        let chains = detect_chains(&[a, b]);
        assert!(chains.is_empty(), "48h gap must not chain");
    }

    #[test]
    fn detect_chains_requires_same_project() {
        let a = make_session(
            "a",
            "/proj/alpha",
            Some("fix auth bug"),
            "claude-opus-4-7",
            ts(2026, 4, 20, 10),
            ts(2026, 4, 20, 11),
        );
        let b = make_session(
            "b",
            "/proj/beta",
            Some("fix auth bug"),
            "claude-opus-4-7",
            ts(2026, 4, 20, 13),
            ts(2026, 4, 20, 14),
        );
        let chains = detect_chains(&[a, b]);
        assert!(chains.is_empty(), "different projects must not chain");
    }

    #[test]
    fn detect_chains_matches_reordered_tokens() {
        // "fix auth bug" vs "auth fix crash":
        // tokens after stopwording: ["auth","bug"] vs ["auth","crash"]
        // shared: ["auth"] — 1 / max(2,2) = 0.5 which is below the 0.6 threshold,
        // but model family matches so the pair should still chain on the
        // tie-breaker.
        let a = make_session(
            "a",
            "/proj/alpha",
            Some("fix auth bug"),
            "claude-opus-4-7",
            ts(2026, 4, 20, 10),
            ts(2026, 4, 20, 11),
        );
        let b = make_session(
            "b",
            "/proj/alpha",
            Some("auth fix crash"),
            "claude-opus-4-7",
            ts(2026, 4, 20, 13),
            ts(2026, 4, 20, 14),
        );
        let chains = detect_chains(&[a, b]);
        assert_eq!(chains.len(), 1);
        let chain = &chains[0];
        assert_eq!(chain.members, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn chain_for_session_finds_membership() {
        let a = make_session(
            "a",
            "/proj/alpha",
            Some("refactor session cache"),
            "claude-opus-4-7",
            ts(2026, 4, 20, 10),
            ts(2026, 4, 20, 11),
        );
        let b = make_session(
            "b",
            "/proj/alpha",
            Some("refactor session cache"),
            "claude-opus-4-7",
            ts(2026, 4, 20, 13),
            ts(2026, 4, 20, 14),
        );
        let chains = detect_chains(&[a, b]);
        assert!(chain_for_session("a", &chains).is_some());
        assert!(chain_for_session("b", &chains).is_some());
        assert!(chain_for_session("c", &chains).is_none());
    }

    #[test]
    fn singleton_does_not_produce_a_chain() {
        let a = make_session(
            "solo",
            "/proj/alpha",
            Some("one-shot tweak"),
            "claude-opus-4-7",
            ts(2026, 4, 20, 10),
            ts(2026, 4, 20, 11),
        );
        let chains = detect_chains(&[a]);
        assert!(chains.is_empty());
    }

    #[test]
    fn normalize_title_drops_stopwords_and_punct() {
        let tokens = normalize_title("Fix the auth-bug, add a test!");
        assert_eq!(
            tokens,
            vec![
                "auth".to_string(),
                "bug".to_string(),
                "test".to_string(),
            ]
        );
    }
}
