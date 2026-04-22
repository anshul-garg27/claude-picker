//! Per-model pricing table for Claude Code sessions.
//!
//! Rates verified against Anthropic's 2026-04 pricing. Prefix-match the model
//! id reported in each assistant message so mixed-model sessions
//! (Opus 4.7 today, Opus 4.6 earlier, maybe a pinch of Sonnet) stay accurate.
//!
//! All figures are USD per million tokens in the published table and are
//! divided to per-token rates inside [`Rates::new`] so cost math is a simple
//! multiply-and-sum at call sites.

/// Per-token dollar rates for a single Claude model.
#[derive(Debug, Clone, Copy)]
pub struct Rates {
    /// Input tokens (non-cache).
    pub input: f64,
    /// Output tokens.
    pub output: f64,
    /// Cache-creation tokens with a 5-minute ephemeral window.
    pub cache_write_5m: f64,
    /// Cache-creation tokens with a 1-hour ephemeral window.
    pub cache_write_1h: f64,
    /// Cache reads (the cheap, pre-warmed path).
    pub cache_read: f64,
}

impl Rates {
    /// Build a [`Rates`] from the published per-million-token rates.
    const fn new(i: f64, o: f64, cw5: f64, cw1: f64, cr: f64) -> Self {
        Self {
            input: i / 1_000_000.0,
            output: o / 1_000_000.0,
            cache_write_5m: cw5 / 1_000_000.0,
            cache_write_1h: cw1 / 1_000_000.0,
            cache_read: cr / 1_000_000.0,
        }
    }
}

/// Prefix-match table, most-specific prefix first so
/// `claude-3-5-sonnet` does not collide with `claude-sonnet-4` matching.
const PRICES: &[(&str, Rates)] = &[
    // Opus 4.x — Opus 4.7 launched Apr 16 2026, kept Opus 4.6's pricing.
    ("claude-opus-4", Rates::new(5.00, 25.00, 6.25, 10.00, 0.50)),
    // Opus 3 — legacy; expensive older rates.
    (
        "claude-3-opus",
        Rates::new(15.00, 75.00, 18.75, 30.00, 1.50),
    ),
    // Sonnet 4.x and 3.x sonnets all land at $3 / $15.
    ("claude-sonnet-4", Rates::new(3.00, 15.00, 3.75, 6.00, 0.30)),
    (
        "claude-3-7-sonnet",
        Rates::new(3.00, 15.00, 3.75, 6.00, 0.30),
    ),
    (
        "claude-3-5-sonnet",
        Rates::new(3.00, 15.00, 3.75, 6.00, 0.30),
    ),
    // Haiku 4.5 — $1 / $5.
    ("claude-haiku-4", Rates::new(1.00, 5.00, 1.25, 2.00, 0.10)),
    // Haiku 3.5 — older and slightly cheaper.
    ("claude-3-5-haiku", Rates::new(0.80, 4.00, 1.00, 1.60, 0.08)),
];

/// Fallback rate applied to unrecognised model ids — the conservative Opus 4
/// rate. Keeps cost reporting an overestimate rather than a silent zero.
const FALLBACK_RATES: Rates = PRICES[0].1;

/// Return the rates for a given model id.
///
/// `None` means "do not score this message" — which is how Claude Code flags
/// synthetic internal traffic. Unknown models fall back to Opus 4 rates.
pub fn rates_for(model: &str) -> Option<Rates> {
    if model.is_empty() || model == "<synthetic>" {
        return None;
    }
    for (prefix, rates) in PRICES {
        if model.starts_with(prefix) {
            return Some(*rates);
        }
    }
    Some(FALLBACK_RATES)
}

/// Aggregated token counts for a single message or a full session.
///
/// Derived from the `usage` block on assistant messages. `cache_write_5m`
/// captures the legacy `cache_creation_input_tokens` field when the split
/// `cache_creation.ephemeral_*_input_tokens` variants are absent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenCounts {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
}

impl TokenCounts {
    /// Total billable tokens across all five buckets.
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write_5m)
            .saturating_add(self.cache_write_1h)
    }

    /// Add another count into this one. Used when aggregating per-message
    /// usage into a session total.
    pub fn add(&mut self, other: TokenCounts) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_write_5m = self.cache_write_5m.saturating_add(other.cache_write_5m);
        self.cache_write_1h = self.cache_write_1h.saturating_add(other.cache_write_1h);
    }
}

/// Compute the dollar cost of a token bundle for a given model.
///
/// Synthetic or empty model ids return `0.0`.
pub fn cost_for(model: &str, tokens: TokenCounts) -> f64 {
    let Some(r) = rates_for(model) else {
        return 0.0;
    };
    tokens.input as f64 * r.input
        + tokens.output as f64 * r.output
        + tokens.cache_read as f64 * r.cache_read
        + tokens.cache_write_5m as f64 * r.cache_write_5m
        + tokens.cache_write_1h as f64 * r.cache_write_1h
}

/// Coarse pricing family used for UI pill coloring (peach / teal / blue).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Opus,
    Sonnet,
    Haiku,
    Unknown,
}

/// Classify a model id into its broad family.
pub fn family(model: &str) -> Family {
    if model.contains("opus") {
        Family::Opus
    } else if model.contains("sonnet") {
        Family::Sonnet
    } else if model.contains("haiku") {
        Family::Haiku
    } else {
        Family::Unknown
    }
}

/// Per-output-token rate for the dominant model of a session.
///
/// Used by the audit's tool-ratio savings calc, which needs to know how much
/// the *output tokens alone* cost at this model's rate (the denominator of
/// the new tool-ratio formula is output tokens, not total). Unknown / empty
/// model strings fall back to the conservative Opus rate so we never silently
/// return zero.
pub fn output_rate_for(model: &str) -> f64 {
    if let Some(r) = rates_for(model) {
        r.output
    } else {
        FALLBACK_RATES.output
    }
}

/// Haiku 4.5's output rate as a fraction of `model`'s output rate.
///
/// The savings formula is `output_cost × tool_ratio × (1 − haiku_ratio)` —
/// so for Opus (output $25/MTok) this returns 5/25 = 0.20, for Sonnet
/// (output $15/MTok) it returns 5/15 ≈ 0.333, and for Haiku (already cheap)
/// it returns 1.0 so the "savings" term collapses to zero. Unknown models
/// fall back to the Opus ratio.
pub fn haiku_output_ratio_to(model: &str) -> f64 {
    // Read Haiku 4.5's output rate straight from the PRICES table so the
    // table stays the single source of truth — no duplicated magic number.
    let haiku_rate = output_rate_for("claude-haiku-4-5");
    let model_rate = output_rate_for(model);
    if model_rate <= 0.0 {
        // Synthetic or unreachable — treat as Opus (most conservative claim
        // since Opus is the most expensive model).
        return haiku_rate / FALLBACK_RATES.output;
    }
    (haiku_rate / model_rate).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: exercise a row at exactly 1M tokens per bucket so the published
    /// per-million rates fall straight out of the math. Floating-point is fine
    /// here because every divide-then-multiply is by `1_000_000` in both
    /// directions.
    fn one_million_each(model: &str, expected_input: f64, expected_output: f64) {
        let t = TokenCounts {
            input: 1_000_000,
            output: 1_000_000,
            cache_read: 0,
            cache_write_5m: 0,
            cache_write_1h: 0,
        };
        let c = cost_for(model, t);
        let expected = expected_input + expected_output;
        assert!(
            (c - expected).abs() < 1e-9,
            "cost for {model}: got {c}, expected {expected}"
        );
    }

    #[test]
    fn opus_4_rates() {
        one_million_each("claude-opus-4-7", 5.00, 25.00);
        one_million_each("claude-opus-4-6", 5.00, 25.00);
    }

    #[test]
    fn opus_3_rates() {
        one_million_each("claude-3-opus-20240229", 15.00, 75.00);
    }

    #[test]
    fn sonnet_rates() {
        one_million_each("claude-sonnet-4-5", 3.00, 15.00);
        one_million_each("claude-3-7-sonnet-20250219", 3.00, 15.00);
        one_million_each("claude-3-5-sonnet-20241022", 3.00, 15.00);
    }

    #[test]
    fn haiku_rates() {
        one_million_each("claude-haiku-4-5", 1.00, 5.00);
        one_million_each("claude-3-5-haiku-20241022", 0.80, 4.00);
    }

    #[test]
    fn cache_buckets_priced_correctly() {
        // 1M cache reads + 1M 5m writes + 1M 1h writes on opus-4.
        let t = TokenCounts {
            input: 0,
            output: 0,
            cache_read: 1_000_000,
            cache_write_5m: 1_000_000,
            cache_write_1h: 1_000_000,
        };
        let c = cost_for("claude-opus-4-7", t);
        let expected = 0.50 + 6.25 + 10.00;
        assert!((c - expected).abs() < 1e-9, "opus-4 cache cost: {c}");
    }

    #[test]
    fn synthetic_and_empty_return_none() {
        assert!(rates_for("").is_none());
        assert!(rates_for("<synthetic>").is_none());
        assert_eq!(cost_for("", TokenCounts::default()), 0.0);
    }

    #[test]
    fn unknown_model_falls_back_to_opus() {
        // Some future model — falls back to Opus 4 rates.
        let t = TokenCounts {
            input: 1_000_000,
            output: 0,
            ..TokenCounts::default()
        };
        assert!((cost_for("claude-future-9", t) - 5.00).abs() < 1e-9);
    }

    #[test]
    fn family_classification() {
        assert_eq!(family("claude-opus-4-7"), Family::Opus);
        assert_eq!(family("claude-sonnet-4-5"), Family::Sonnet);
        assert_eq!(family("claude-3-5-haiku-20241022"), Family::Haiku);
        assert_eq!(family("claude-future-9"), Family::Unknown);
    }

    #[test]
    fn output_rate_for_known_models() {
        assert!((output_rate_for("claude-opus-4-7") - 25.0 / 1_000_000.0).abs() < 1e-12);
        assert!((output_rate_for("claude-sonnet-4-5") - 15.0 / 1_000_000.0).abs() < 1e-12);
        assert!((output_rate_for("claude-haiku-4-5") - 5.0 / 1_000_000.0).abs() < 1e-12);
    }

    #[test]
    fn output_rate_for_empty_falls_back_to_opus() {
        // Empty / synthetic → FALLBACK_RATES.output = Opus 4 output rate.
        assert!((output_rate_for("") - 25.0 / 1_000_000.0).abs() < 1e-12);
        assert!((output_rate_for("<synthetic>") - 25.0 / 1_000_000.0).abs() < 1e-12);
    }

    #[test]
    fn haiku_ratio_is_correct_per_family() {
        // Opus: 5/25 = 0.20
        assert!((haiku_output_ratio_to("claude-opus-4-7") - 0.20).abs() < 1e-9);
        // Sonnet: 5/15 ≈ 0.333…
        assert!((haiku_output_ratio_to("claude-sonnet-4-5") - (5.0 / 15.0)).abs() < 1e-9);
        // Haiku: 5/5 = 1.0 → "savings" term (1 - 1.0) = 0.
        assert!((haiku_output_ratio_to("claude-haiku-4-5") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn token_counts_total_and_add() {
        let mut a = TokenCounts {
            input: 1,
            output: 2,
            cache_read: 3,
            cache_write_5m: 4,
            cache_write_1h: 5,
        };
        assert_eq!(a.total(), 15);
        a.add(TokenCounts {
            input: 10,
            output: 20,
            cache_read: 30,
            cache_write_5m: 40,
            cache_write_1h: 50,
        });
        assert_eq!(a.total(), 165);
    }
}
