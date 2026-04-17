//! Structured filter operators for the `claude-picker search` query language.
//!
//! Tokens in the query that start with a sigil get pulled out of the fuzzy
//! text and turned into hard filters applied *before* nucleo ever runs:
//!
//! - `!bookmarked`, `!named`                 → boolean filters
//! - `@opus` / `@sonnet` / `@haiku`          → model filter (repeatable)
//! - `@plan` / `@bypass` / `@accept` / …     → permission-mode filter
//! - `#today` / `#yesterday` / `#week` / …   → age filter (Duration range)
//! - `#2024-04-15` / `#apr-15`               → specific-date filter
//! - `$>1`, `$<0.5`, `$>=5`                  → cost comparison ($USD)
//! - `tokens>50k`, `msgs>100`                → quantitative filters
//!
//! Unknown tokens fall through to the fuzzy-match pass unchanged, so a user
//! typo like `$abc` reads as literal text rather than an error.
//!
//! This module is data-layer; the UI just consumes the parsed [`Filters`]
//! plus the remaining plain-text needle to feed nucleo.

use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate, TimeZone};

use crate::data::session::PermissionMode;

/// All filters extractable from a query. Every field is `None`/empty by
/// default, meaning "no restriction for this dimension". Construction is
/// additive: merging two `Filters` ANDs them together.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Filters {
    pub bookmarked: Option<bool>,
    pub named: Option<bool>,
    /// Lowercased model family substrings (`"opus"`, `"sonnet"`, `"haiku"`).
    /// A session matches if any of its model id contains any of these.
    pub models: Vec<String>,
    pub permission_modes: Vec<PermissionMode>,
    /// Minimum *age* — a session's newest-timestamp must be at least this old.
    /// Pairs with `max_age`. Both measured as a [`Duration`] relative to "now".
    pub min_age: Option<Duration>,
    /// Maximum *age* — a session's newest-timestamp must be no older than this.
    pub max_age: Option<Duration>,
    /// Exact calendar date in the local timezone. Wins over min/max age when
    /// present.
    pub specific_date: Option<NaiveDate>,
    pub min_cost: Option<f64>,
    pub max_cost: Option<f64>,
    pub min_tokens: Option<u64>,
    pub max_tokens: Option<u64>,
    pub min_msgs: Option<u32>,
    pub max_msgs: Option<u32>,
}

impl Filters {
    /// True when no filters have been set. Useful for the UI so it knows
    /// whether to draw the "active:" chip row.
    pub fn is_empty(&self) -> bool {
        self.bookmarked.is_none()
            && self.named.is_none()
            && self.models.is_empty()
            && self.permission_modes.is_empty()
            && self.min_age.is_none()
            && self.max_age.is_none()
            && self.specific_date.is_none()
            && self.min_cost.is_none()
            && self.max_cost.is_none()
            && self.min_tokens.is_none()
            && self.max_tokens.is_none()
            && self.min_msgs.is_none()
            && self.max_msgs.is_none()
    }

    /// Human-readable chip labels — drives the "active:" row in the search UI.
    /// Order is stable so the UI doesn't flicker between frames.
    pub fn chip_labels(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(v) = self.bookmarked {
            out.push(if v {
                "bookmarked".to_string()
            } else {
                "!bookmarked".to_string()
            });
        }
        if let Some(v) = self.named {
            out.push(if v {
                "named".to_string()
            } else {
                "!named".to_string()
            });
        }
        for m in &self.models {
            out.push(format!("@{m}"));
        }
        for pm in &self.permission_modes {
            out.push(match pm {
                PermissionMode::Plan => "@plan".to_string(),
                PermissionMode::BypassPermissions => "@bypass".to_string(),
                PermissionMode::AcceptEdits => "@accept".to_string(),
                PermissionMode::DontAsk => "@dontask".to_string(),
                PermissionMode::Auto => "@auto".to_string(),
                PermissionMode::Default => "@default".to_string(),
                PermissionMode::Other(s) => format!("@{s}"),
            });
        }
        if let Some(d) = self.specific_date {
            out.push(format!("#{}", d.format("%Y-%m-%d")));
        } else {
            match (self.min_age, self.max_age) {
                (None, Some(max)) if max <= Duration::from_secs(60 * 60 * 24) => {
                    out.push("#today".to_string())
                }
                (Some(min), Some(max))
                    if min == Duration::from_secs(60 * 60 * 24)
                        && max == Duration::from_secs(60 * 60 * 48) =>
                {
                    out.push("#yesterday".to_string())
                }
                (None, Some(max)) if max == Duration::from_secs(60 * 60 * 24 * 7) => {
                    out.push("#week".to_string())
                }
                (None, Some(max)) if max == Duration::from_secs(60 * 60 * 24 * 30) => {
                    out.push("#month".to_string())
                }
                _ => {}
            }
        }
        if let Some(min) = self.min_cost {
            out.push(format!("${}+", trim_float(min)));
        }
        if let Some(max) = self.max_cost {
            out.push(format!("<${}", trim_float(max)));
        }
        if let Some(min) = self.min_tokens {
            out.push(format!("tokens>{}", format_tokens(min)));
        }
        if let Some(max) = self.max_tokens {
            out.push(format!("tokens<{}", format_tokens(max)));
        }
        if let Some(min) = self.min_msgs {
            out.push(format!("msgs>{min}"));
        }
        if let Some(max) = self.max_msgs {
            out.push(format!("msgs<{max}"));
        }
        out
    }
}

/// Parse `query` into `(Filters, fuzzy_text)`. The fuzzy text has every
/// recognised filter token stripped and remaining whitespace collapsed to a
/// single space so nucleo sees a clean needle.
///
/// Unknown tokens stay put — a user typing `@noop` falls through to fuzzy
/// where `@` is an acceptable character. This makes the parser forgiving
/// without swallowing typos silently.
pub fn parse(query: &str) -> (Filters, String) {
    let mut f = Filters::default();
    let mut fuzzy_parts: Vec<&str> = Vec::new();

    for tok in query.split_whitespace() {
        if apply_token(&mut f, tok) {
            continue;
        }
        fuzzy_parts.push(tok);
    }

    (f, fuzzy_parts.join(" "))
}

/// Attempt to interpret one token as a filter. Returns true on success so
/// the caller knows to drop it from the fuzzy text.
fn apply_token(f: &mut Filters, raw: &str) -> bool {
    if raw.is_empty() {
        return false;
    }
    let lower = raw.to_lowercase();

    // ! bang filters — boolean attributes. `!!bookmarked` toggles the
    // negation so `!!` still reads as an override if a user wants it.
    if let Some(rest) = lower.strip_prefix("!!") {
        return match rest {
            "bookmarked" => {
                f.bookmarked = Some(false);
                true
            }
            "named" => {
                f.named = Some(false);
                true
            }
            _ => false,
        };
    }
    if let Some(rest) = lower.strip_prefix('!') {
        return match rest {
            "bookmarked" => {
                f.bookmarked = Some(true);
                true
            }
            "named" => {
                f.named = Some(true);
                true
            }
            _ => false,
        };
    }

    // @ model + permission-mode filters.
    if let Some(rest) = lower.strip_prefix('@') {
        return apply_at_token(f, rest);
    }

    // # age / date filters.
    if let Some(rest) = lower.strip_prefix('#') {
        return apply_hash_token(f, rest);
    }

    // $ cost filters.
    if let Some(rest) = raw.strip_prefix('$') {
        return apply_cost_token(f, rest);
    }

    // tokens>N / msgs>N and their `<`, `>=`, `<=` siblings. Not prefixed
    // with a sigil; detect by the keyword + comparator.
    if let Some((kind, comp)) = split_keyword_comparison(&lower) {
        return apply_keyword_token(f, kind, comp);
    }

    false
}

fn apply_at_token(f: &mut Filters, rest: &str) -> bool {
    match rest {
        "opus" | "sonnet" | "haiku" => {
            f.models.push(rest.to_string());
            true
        }
        "plan" => {
            f.permission_modes.push(PermissionMode::Plan);
            true
        }
        "bypass" | "bypasspermissions" => {
            f.permission_modes.push(PermissionMode::BypassPermissions);
            true
        }
        "accept" | "acceptedits" => {
            f.permission_modes.push(PermissionMode::AcceptEdits);
            true
        }
        "auto" => {
            f.permission_modes.push(PermissionMode::Auto);
            true
        }
        "dontask" => {
            f.permission_modes.push(PermissionMode::DontAsk);
            true
        }
        "default" => {
            f.permission_modes.push(PermissionMode::Default);
            true
        }
        _ => false,
    }
}

fn apply_hash_token(f: &mut Filters, rest: &str) -> bool {
    let day = Duration::from_secs(60 * 60 * 24);
    match rest {
        "today" => {
            f.max_age = Some(day);
            true
        }
        "yesterday" => {
            f.min_age = Some(day);
            f.max_age = Some(day * 2);
            true
        }
        "week" => {
            f.max_age = Some(day * 7);
            true
        }
        "month" => {
            f.max_age = Some(day * 30);
            true
        }
        _ => {
            if let Some(d) = parse_date(rest) {
                f.specific_date = Some(d);
                true
            } else {
                false
            }
        }
    }
}

/// Parse `<rest>` of a `$...` token. Accepts: `>N`, `<N`, `>=N`, `<=N`, `N`
/// (exact, currently treated as `>=N`). `N` is a float with optional
/// leading `$` already stripped by the caller.
fn apply_cost_token(f: &mut Filters, rest: &str) -> bool {
    let (op, num) = split_comparator(rest);
    let Ok(val) = num.parse::<f64>() else {
        return false;
    };
    match op {
        ">" => f.min_cost = Some(nudge_up(val)),
        ">=" => f.min_cost = Some(val),
        "<" => f.max_cost = Some(nudge_down(val)),
        "<=" => f.max_cost = Some(val),
        "=" | "" => f.min_cost = Some(val),
        _ => return false,
    }
    true
}

/// Classify a keyword token like `tokens>50k` or `msgs>=100` into
/// `(kind, comparison)` so [`apply_keyword_token`] can branch cleanly.
fn split_keyword_comparison(lower: &str) -> Option<(&'static str, &str)> {
    for prefix in ["tokens", "msgs"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            // Must be followed by a comparator to qualify — `tokensrunner`
            // for example is a regular identifier and must fall through.
            let first = rest.chars().next()?;
            if matches!(first, '>' | '<' | '=') {
                let kind = if prefix == "tokens" { "tokens" } else { "msgs" };
                return Some((kind, rest));
            }
        }
    }
    None
}

fn apply_keyword_token(f: &mut Filters, kind: &str, rest: &str) -> bool {
    let (op, num) = split_comparator(rest);
    match kind {
        "tokens" => {
            let Some(val) = parse_token_number(num) else {
                return false;
            };
            match op {
                ">" => f.min_tokens = Some(val.saturating_add(1)),
                ">=" => f.min_tokens = Some(val),
                "<" => f.max_tokens = Some(val.saturating_sub(1)),
                "<=" => f.max_tokens = Some(val),
                "=" | "" => f.min_tokens = Some(val),
                _ => return false,
            }
            true
        }
        "msgs" => {
            let Ok(val) = num.parse::<u32>() else {
                return false;
            };
            match op {
                ">" => f.min_msgs = Some(val.saturating_add(1)),
                ">=" => f.min_msgs = Some(val),
                "<" => f.max_msgs = Some(val.saturating_sub(1)),
                "<=" => f.max_msgs = Some(val),
                "=" | "" => f.min_msgs = Some(val),
                _ => return false,
            }
            true
        }
        _ => false,
    }
}

/// Pull the leading `>`, `<`, `>=`, `<=`, or `=` off a value string.
/// Returns `(comparator, remaining-number-text)`. The comparator is the
/// empty string when the value is bare.
fn split_comparator(rest: &str) -> (&str, &str) {
    for op in [">=", "<=", ">", "<", "="] {
        if let Some(n) = rest.strip_prefix(op) {
            return (op, n);
        }
    }
    ("", rest)
}

/// Parse a token number that may end with the SI suffixes `k`, `m`, `b`.
/// `"50k"` → 50_000. Case-insensitive. Returns `None` on any parse error.
fn parse_token_number(s: &str) -> Option<u64> {
    let (num_part, mult): (&str, u64) = match s.chars().last() {
        Some('k') | Some('K') => (&s[..s.len() - 1], 1_000),
        Some('m') | Some('M') => (&s[..s.len() - 1], 1_000_000),
        Some('b') | Some('B') => (&s[..s.len() - 1], 1_000_000_000),
        _ => (s, 1),
    };
    let n: f64 = num_part.parse().ok()?;
    Some((n * mult as f64) as u64)
}

/// Parse a date token. Accepts ISO `YYYY-MM-DD` as well as short-month
/// forms `apr-15` (defaults to current year).
fn parse_date(s: &str) -> Option<NaiveDate> {
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d);
    }
    // Short month: "apr-15".
    let (month_str, day_str) = s.split_once('-')?;
    let month = month_from_short(month_str)?;
    let day: u32 = day_str.parse().ok()?;
    let year = Local::now().year();
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Map `jan` .. `dec` to 1..12. Case-insensitive.
fn month_from_short(s: &str) -> Option<u32> {
    let s = s.to_lowercase();
    match s.as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

/// Used to turn `$>1` into a minimum of $1.0000…001 — a float epsilon bigger
/// than the provided value so the comparison is strictly-greater.
fn nudge_up(v: f64) -> f64 {
    v.next_up()
}
fn nudge_down(v: f64) -> f64 {
    v.next_down()
}

/// Trim trailing zeros off a `f64` for chip display: `1.0` → `"1"`,
/// `1.5` → `"1.5"`. Keeps the chip bar from reading as `$1.00+`.
fn trim_float(v: f64) -> String {
    let formatted = format!("{v:.4}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Format a token count for chip display: `50000` → `"50k"`.
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}b", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}m", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        format!("{n}")
    }
}

/// Convert a chrono UTC timestamp → local [`NaiveDate`].
pub fn timestamp_to_local_date(ts: chrono::DateTime<chrono::Utc>) -> NaiveDate {
    ts.with_timezone(&Local).date_naive()
}

/// Convert a local [`NaiveDate`] to the UTC timestamp at its start. Handy for
/// comparing sessions against `#2024-04-15` style filters.
pub fn local_date_to_start(d: NaiveDate) -> Option<chrono::DateTime<chrono::Utc>> {
    Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
        .single()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_yields_empty_filters_and_text() {
        let (f, text) = parse("");
        assert!(f.is_empty());
        assert_eq!(text, "");
    }

    #[test]
    fn plain_text_passes_through_untouched() {
        let (f, text) = parse("auth refactor redis");
        assert!(f.is_empty());
        assert_eq!(text, "auth refactor redis");
    }

    #[test]
    fn bang_bookmarked_sets_filter_and_strips_token() {
        let (f, text) = parse("auth !bookmarked");
        assert_eq!(f.bookmarked, Some(true));
        assert_eq!(text, "auth");
    }

    #[test]
    fn double_bang_bookmarked_sets_negation() {
        let (f, _) = parse("!!bookmarked");
        assert_eq!(f.bookmarked, Some(false));
    }

    #[test]
    fn bang_named_sets_named_filter() {
        let (f, _) = parse("!named");
        assert_eq!(f.named, Some(true));
    }

    #[test]
    fn at_opus_sets_model_filter() {
        let (f, text) = parse("@opus auth");
        assert_eq!(f.models, vec!["opus".to_string()]);
        assert_eq!(text, "auth");
    }

    #[test]
    fn at_sonnet_and_haiku_both_recognised() {
        let (f, _) = parse("@sonnet");
        assert_eq!(f.models, vec!["sonnet".to_string()]);
        let (f, _) = parse("@haiku");
        assert_eq!(f.models, vec!["haiku".to_string()]);
    }

    #[test]
    fn at_plan_sets_permission_mode() {
        let (f, _) = parse("@plan");
        assert_eq!(f.permission_modes, vec![PermissionMode::Plan]);
    }

    #[test]
    fn at_bypass_and_accept_recognised() {
        let (f, _) = parse("@bypass");
        assert_eq!(f.permission_modes, vec![PermissionMode::BypassPermissions]);
        let (f, _) = parse("@accept");
        assert_eq!(f.permission_modes, vec![PermissionMode::AcceptEdits]);
    }

    #[test]
    fn hash_today_sets_max_age_one_day() {
        let (f, _) = parse("#today");
        assert_eq!(f.max_age, Some(Duration::from_secs(60 * 60 * 24)));
    }

    #[test]
    fn hash_yesterday_sets_range_one_to_two_days() {
        let (f, _) = parse("#yesterday");
        assert_eq!(f.min_age, Some(Duration::from_secs(60 * 60 * 24)));
        assert_eq!(f.max_age, Some(Duration::from_secs(60 * 60 * 48)));
    }

    #[test]
    fn hash_week_sets_seven_days() {
        let (f, _) = parse("#week");
        assert_eq!(f.max_age, Some(Duration::from_secs(60 * 60 * 24 * 7)));
    }

    #[test]
    fn hash_month_sets_thirty_days() {
        let (f, _) = parse("#month");
        assert_eq!(f.max_age, Some(Duration::from_secs(60 * 60 * 24 * 30)));
    }

    #[test]
    fn hash_iso_date_parses() {
        let (f, _) = parse("#2024-04-15");
        assert_eq!(
            f.specific_date,
            Some(NaiveDate::from_ymd_opt(2024, 4, 15).unwrap())
        );
    }

    #[test]
    fn hash_short_date_parses_with_current_year() {
        let (f, _) = parse("#apr-15");
        assert!(f.specific_date.is_some());
        let d = f.specific_date.unwrap();
        assert_eq!(d.month(), 4);
        assert_eq!(d.day(), 15);
        // Year is current, which we won't hardcode in a test.
    }

    #[test]
    fn dollar_greater_sets_min_cost() {
        let (f, _) = parse("$>1");
        assert!(f.min_cost.is_some());
        assert!(f.min_cost.unwrap() > 1.0);
    }

    #[test]
    fn dollar_less_sets_max_cost() {
        let (f, _) = parse("$<0.5");
        assert!(f.max_cost.is_some());
        assert!(f.max_cost.unwrap() < 0.5);
    }

    #[test]
    fn dollar_ge_sets_inclusive_min_cost() {
        let (f, _) = parse("$>=5");
        assert_eq!(f.min_cost, Some(5.0));
    }

    #[test]
    fn tokens_comparisons_accept_k_suffix() {
        let (f, _) = parse("tokens>50k");
        assert_eq!(f.min_tokens, Some(50_001));
        let (f, _) = parse("tokens<=1m");
        assert_eq!(f.max_tokens, Some(1_000_000));
    }

    #[test]
    fn msgs_comparisons_parse_integers() {
        let (f, _) = parse("msgs>100");
        assert_eq!(f.min_msgs, Some(101));
        let (f, _) = parse("msgs<=10");
        assert_eq!(f.max_msgs, Some(10));
    }

    #[test]
    fn multi_filter_query_combines_all() {
        let (f, text) = parse("auth !bookmarked @opus #week $>1");
        assert_eq!(f.bookmarked, Some(true));
        assert_eq!(f.models, vec!["opus".to_string()]);
        assert_eq!(f.max_age, Some(Duration::from_secs(60 * 60 * 24 * 7)));
        assert!(f.min_cost.unwrap() > 1.0);
        assert_eq!(text, "auth");
    }

    #[test]
    fn unknown_tokens_fall_through_to_fuzzy_text() {
        let (f, text) = parse("@xerxes auth $$$ ?garbage");
        // None of these parsed → all remain as fuzzy text.
        assert!(f.is_empty());
        assert_eq!(text, "@xerxes auth $$$ ?garbage");
    }

    #[test]
    fn chip_labels_include_bookmarked_and_model() {
        let (f, _) = parse("!bookmarked @opus #week $>1");
        let chips = f.chip_labels();
        assert!(chips.iter().any(|c| c == "bookmarked"));
        assert!(chips.iter().any(|c| c == "@opus"));
        assert!(chips.iter().any(|c| c == "#week"));
        assert!(chips.iter().any(|c| c.starts_with('$')));
    }

    #[test]
    fn is_empty_flags_default_filters() {
        let f = Filters::default();
        assert!(f.is_empty());
        let g = Filters {
            bookmarked: Some(true),
            ..Filters::default()
        };
        assert!(!g.is_empty());
    }

    #[test]
    fn token_number_suffix_parsing() {
        assert_eq!(parse_token_number("50k"), Some(50_000));
        assert_eq!(parse_token_number("2M"), Some(2_000_000));
        assert_eq!(parse_token_number("3b"), Some(3_000_000_000));
        assert_eq!(parse_token_number("100"), Some(100));
        assert_eq!(parse_token_number("bad"), None);
    }

    #[test]
    fn split_comparator_peels_leading_op() {
        assert_eq!(split_comparator(">=5"), (">=", "5"));
        assert_eq!(split_comparator(">5"), (">", "5"));
        assert_eq!(split_comparator("<=5"), ("<=", "5"));
        assert_eq!(split_comparator("<5"), ("<", "5"));
        assert_eq!(split_comparator("=5"), ("=", "5"));
        assert_eq!(split_comparator("5"), ("", "5"));
    }

    #[test]
    fn combined_with_plain_text_preserves_order_of_fuzzy_tokens() {
        let (f, text) = parse("redis !bookmarked auth @opus middleware");
        assert_eq!(f.bookmarked, Some(true));
        assert_eq!(f.models, vec!["opus".to_string()]);
        // Fuzzy tokens emerge in original order.
        assert_eq!(text, "redis auth middleware");
    }

    #[test]
    fn bang_with_non_matching_keyword_falls_through() {
        // `!unknown` is an unknown filter, not a recognised negation. Fall
        // through to fuzzy text.
        let (f, text) = parse("!unknown");
        assert!(f.is_empty());
        assert_eq!(text, "!unknown");
    }
}
