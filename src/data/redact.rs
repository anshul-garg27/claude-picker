//! Secret-shape redaction for preview surfaces (feature #53).
//!
//! Scans arbitrary rendered text for well-known credential formats and
//! replaces the sensitive tail with `****<last4>` so snapshots, demos, and
//! screenshots never accidentally leak a working token. The helper is
//! intentionally "defence in depth": we run it on every piece of message
//! text the preview pane and full-screen conversation viewer render, gated
//! by the `[ui] redact_preview` config toggle (default `true`).
//!
//! ## Shapes matched
//!
//! | Shape           | Example                    | Notes                                        |
//! |-----------------|----------------------------|----------------------------------------------|
//! | Anthropic key   | `sk-ant-abc…`              | `sk-ant-` + ≥ 32 `[A-Za-z0-9_-]` chars       |
//! | OpenAI project  | `sk-proj-abc…`             | same shape, different prefix                 |
//! | AWS access key  | `AKIA0123456789ABCDEF`     | `AKIA` / `ASIA` + exactly 16 `[A-Z0-9]`      |
//! | GitHub PAT      | `ghp_…`, `gho_…`, `ghu_…`, `ghs_…` | `gh[pous]_` + 36 `[A-Za-z0-9]`       |
//! | JWT             | `eyJ….….…`                 | three dot-separated base64url segments       |
//! | Bearer header   | `Bearer abc…`              | case-insensitive, any non-empty token body   |
//!
//! ## Idempotency
//!
//! The replacement format (`<prefix>****<last4>`) no longer matches any of
//! the source patterns because `****` is not a legal character in any of
//! the matched alphabets. Running [`redact_inline`] twice therefore returns
//! byte-identical output on the second pass — enforced by the
//! `redact_is_idempotent` test.
//!
//! ## Defence in depth, not a replacement for log hygiene
//!
//! The regex list is deliberately conservative. Shapes we don't recognise
//! (private keys, Slack tokens, custom bearer schemes) will slip through;
//! the goal is to catch the most common "committed-to-chat" leaks, not to
//! be a universal DLP engine. New shapes land here as we see them in the
//! wild.

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;

/// Compile the redaction regexes once and reuse them for every call. The
/// set is fixed at startup — user config only toggles the whole feature
/// on or off, so a lazy `OnceLock` is the right shape.
///
/// Each entry is `(pattern, replacer)`. The replacer receives a captured
/// match and returns the masked form to splice back into the text. Keeping
/// the replacer as a function pointer keeps the compile-once cache small
/// (no per-call closures on the heap).
fn rules() -> &'static [(Regex, fn(&regex::Captures<'_>) -> String)] {
    static CELL: OnceLock<Vec<(Regex, fn(&regex::Captures<'_>) -> String)>> = OnceLock::new();
    CELL.get_or_init(build_rules)
}

fn build_rules() -> Vec<(Regex, fn(&regex::Captures<'_>) -> String)> {
    // IMPORTANT — order matters: JWTs must run before the generic Bearer
    // rule so that `Bearer eyJ….….…` gets redacted as a JWT (preserving
    // the three-segment shape) rather than as a bearer blob.
    //
    // Why plain function pointers and not closures: the stored value is
    // `fn(_) -> String`, so we can avoid boxing and keep each replacer
    // self-contained. Every rule still has read-only access to the raw
    // capture, which is all any of them need.
    vec![
        // Anthropic API keys — `sk-ant-` followed by 32+ URL-safe chars.
        // Claude Code CLI uses this exact shape, so user-pasted logs or
        // an accidental `echo $ANTHROPIC_API_KEY` inside a session is the
        // classic leak vector.
        (
            Regex::new(r"sk-ant-[A-Za-z0-9_\-]{32,}")
                .expect("sk-ant regex must compile"),
            |caps| mask_prefix_tail(&caps[0], "sk-ant-"),
        ),
        // OpenAI project keys (`sk-proj-…`) — same shape, different
        // prefix. Include them because users who hop between providers
        // often paste keys side-by-side.
        (
            Regex::new(r"sk-proj-[A-Za-z0-9_\-]{32,}")
                .expect("sk-proj regex must compile"),
            |caps| mask_prefix_tail(&caps[0], "sk-proj-"),
        ),
        // AWS access key ids — `AKIA` (long-lived) / `ASIA` (temporary
        // STS) plus exactly 16 `[A-Z0-9]` chars. The uppercase-only body
        // keeps the pattern sharp even inside free-form prose.
        (
            Regex::new(r"(AKIA|ASIA)[0-9A-Z]{16}")
                .expect("aws regex must compile"),
            |caps| {
                // Keep the 4-char scheme marker (AKIA/ASIA) so readers can
                // tell long-lived from temporary creds at a glance.
                let m = &caps[0];
                let scheme = &m[..4];
                let last4 = tail_chars(m, 4);
                format!("{scheme}****{last4}")
            },
        ),
        // GitHub Personal Access Tokens — `ghp_`, `gho_` (OAuth), `ghu_`
        // (user-to-server), `ghs_` (server-to-server) + exactly 36
        // `[A-Za-z0-9]` chars (the classic v2 format).
        (
            Regex::new(r"gh[pous]_[A-Za-z0-9]{36}")
                .expect("gh token regex must compile"),
            |caps| {
                // Preserve the four-char prefix so a leaked token still
                // reads as "was a GitHub PAT" without giving away the body.
                let m = &caps[0];
                let prefix = &m[..4]; // e.g. "ghp_"
                let last4 = tail_chars(m, 4);
                format!("{prefix}****{last4}")
            },
        ),
        // JSON Web Tokens — three base64url segments separated by `.`.
        // The header-marker `eyJ` (base64url of `{"`) keeps the match
        // anchored so we don't redact arbitrary dotted identifiers.
        (
            Regex::new(r"eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+")
                .expect("jwt regex must compile"),
            |caps| {
                let m = &caps[0];
                let last4 = tail_chars(m, 4);
                format!("eyJ****{last4}")
            },
        ),
        // Bearer authorization headers — `Bearer ` + any contiguous run of
        // token-looking characters. Case-insensitive prefix so `bearer`,
        // `BEARER`, and the canonical `Bearer` all collapse. The body
        // alphabet (`[A-Za-z0-9._-]`) matches RFC 6750 for opaque tokens.
        (
            Regex::new(r"(?i)bearer\s+[A-Za-z0-9._\-]+")
                .expect("bearer regex must compile"),
            |caps| {
                let m = &caps[0];
                // Split on the first whitespace run so we preserve the
                // original prefix casing (`Bearer` vs `bearer` vs `BEARER`)
                // — a log snippet that read `BEARER xyz` should mask as
                // `BEARER ****xyz`, not `Bearer ****xyz`.
                let (prefix, rest) = split_prefix_ws(m);
                let last4 = tail_chars(rest, 4);
                format!("{prefix} ****{last4}")
            },
        ),
    ]
}

/// Redact every secret-shaped match in `text`.
///
/// Returns `Cow::Borrowed(text)` when nothing matched so the common case
/// (normal prose) never allocates. When at least one rule fires we
/// re-allocate once per rule — sessions with many keys would iterate
/// multiple times, but the common case is "a couple of matches at most"
/// and the simplicity of a per-rule `Regex::replace_all` outweighs the
/// cost of a unified regex set.
///
/// ## Idempotency
///
/// Calling `redact_inline(redact_inline(x))` yields the same bytes as
/// `redact_inline(x)` — the replacement glyph (`****`) is not in any of
/// the source alphabets, so a second pass never re-matches.
pub fn redact_inline(text: &str) -> Cow<'_, str> {
    let mut out: Cow<'_, str> = Cow::Borrowed(text);
    for (re, replacer) in rules() {
        // Only pay the replacement cost when the rule actually fires —
        // `is_match` is cheaper than `replace_all` on no-match input.
        if !re.is_match(out.as_ref()) {
            continue;
        }
        let replaced = re
            .replace_all(out.as_ref(), |caps: &regex::Captures<'_>| replacer(caps))
            .into_owned();
        out = Cow::Owned(replaced);
    }
    out
}

/// Mask a simple "fixed-prefix + random body" shape as `<prefix>****<last4>`.
///
/// Shared by the `sk-ant-` / `sk-proj-` rules — both have a literal prefix
/// plus a body where every char can move, so the redacted form just needs
/// to keep the prefix plus the last four body chars.
fn mask_prefix_tail(m: &str, prefix: &str) -> String {
    let body = m.strip_prefix(prefix).unwrap_or(m);
    let last4 = tail_chars(body, 4);
    format!("{prefix}****{last4}")
}

/// Return up to the last `n` chars of `s`, char-safe (not byte-safe).
/// Works correctly for ASCII-only inputs (which all matched shapes are)
/// and stays well-behaved on the pathological edge case of a body shorter
/// than `n`.
fn tail_chars(s: &str, n: usize) -> String {
    let total = s.chars().count();
    if total <= n {
        return s.to_string();
    }
    s.chars().skip(total - n).collect()
}

/// Split a string like `Bearer xyz` into (`Bearer`, `xyz`). Tolerant of
/// any amount of whitespace (tabs, multiple spaces) between the prefix and
/// token body — RFC 6750 mandates one space but real-world headers drift.
fn split_prefix_ws(m: &str) -> (&str, &str) {
    // `find` locates the first whitespace byte; the subsequent
    // `trim_start` strips any additional whitespace from the remainder.
    match m.find(char::is_whitespace) {
        Some(idx) => {
            let prefix = &m[..idx];
            let rest = m[idx..].trim_start();
            (prefix, rest)
        }
        None => (m, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Per-shape round-trips ────────────────────────────────────────────

    #[test]
    fn redacts_anthropic_key() {
        let input = "the key is sk-ant-abcdefghijklmnopqrstuvwxyz0123456789 and more";
        let out = redact_inline(input);
        assert!(
            out.contains("sk-ant-****"),
            "anthropic key must mask to sk-ant-****<last4>, got: {out}",
        );
        assert!(out.contains("****6789"), "last 4 chars must be preserved: {out}");
        assert!(
            !out.contains("abcdefghijklmnopqrstuvwxyz0123"),
            "raw body must not survive: {out}",
        );
    }

    #[test]
    fn redacts_openai_proj_key() {
        let input = "sk-proj-ABCDEF0123456789ABCDEF0123456789XYZW trailing";
        let out = redact_inline(input);
        assert!(out.contains("sk-proj-****"), "got: {out}");
        assert!(out.contains("****XYZW"), "got: {out}");
    }

    #[test]
    fn redacts_aws_access_keys() {
        // AKIA (long-lived) and ASIA (temporary STS) share the shape.
        let input = "long AKIA0123456789ABCDEF and short ASIAZZZZZZZZZZZZZZZZ";
        let out = redact_inline(input);
        assert!(out.contains("AKIA****CDEF"), "AKIA last4 preserved: {out}");
        assert!(out.contains("ASIA****ZZZZ"), "ASIA last4 preserved: {out}");
        assert!(!out.contains("0123456789AB"), "raw body leaked: {out}");
    }

    #[test]
    fn redacts_github_pats() {
        // All four v2 scheme markers must land in the masked prefix.
        let input =
            "ghp_000000000000000000000000000000000ABC gho_111111111111111111111111111111111DEF ghu_222222222222222222222222222222222GHI ghs_333333333333333333333333333333333JKL";
        let out = redact_inline(input);
        assert!(out.contains("ghp_****0ABC"), "ghp_ scheme preserved: {out}");
        assert!(out.contains("gho_****1DEF"), "gho_ scheme preserved: {out}");
        assert!(out.contains("ghu_****2GHI"), "ghu_ scheme preserved: {out}");
        assert!(out.contains("ghs_****3JKL"), "ghs_ scheme preserved: {out}");
    }

    #[test]
    fn redacts_jwts() {
        // Canonical three-segment JWT with `eyJ` header marker.
        let input =
            "token: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkphbmUgRG9lIn0.f4KpV8s0_wXY8OFPQ9dY9pj8cZqUCXOh9sS4ZtRvWxY tail";
        let out = redact_inline(input);
        assert!(out.contains("eyJ****"), "jwt must mask: {out}");
        assert!(!out.contains("eyJhbGciOiJIUzI1NiI"), "raw header survived: {out}");
    }

    #[test]
    fn redacts_bearer_headers() {
        // Case-insensitive prefix, preserves caller's casing.
        let lower = "authorization: bearer abcdef1234567890";
        let upper = "Header: BEARER some.opaque-value";
        let mixed = "curl -H 'Authorization: Bearer sk-xxx-yyy-zzz-aabb'";
        let lower_out = redact_inline(lower);
        let upper_out = redact_inline(upper);
        let mixed_out = redact_inline(mixed);
        assert!(
            lower_out.contains("bearer ****7890"),
            "lowercase bearer should keep case + mask: {lower_out}",
        );
        assert!(
            upper_out.contains("BEARER ****alue"),
            "BEARER should keep case: {upper_out}",
        );
        assert!(
            mixed_out.contains("Bearer ****aabb"),
            "Bearer (canonical) should keep case: {mixed_out}",
        );
    }

    // ── Passthrough cases ────────────────────────────────────────────────

    #[test]
    fn leaves_plain_prose_alone() {
        let text = "the quick brown fox jumps over the lazy dog 12345";
        let out = redact_inline(text);
        assert_eq!(out, text, "prose without secrets must be unchanged");
        assert!(
            matches!(out, Cow::Borrowed(_)),
            "no allocation for the no-match common case",
        );
    }

    // ── Idempotency ──────────────────────────────────────────────────────

    #[test]
    fn redact_is_idempotent() {
        // Every shape, one after another — covers the multi-rule
        // interaction with a single assertion.
        let input = "sk-ant-abcdefghijklmnopqrstuvwxyz0123456789 AKIA0123456789ABCDEF ghp_000000000000000000000000000000000ABC eyJheHgiOiJhIn0.eyJzdWIiOiJiIn0.signaturepart Bearer foo.bar-baz";
        let first = redact_inline(input).into_owned();
        let second = redact_inline(&first).into_owned();
        assert_eq!(
            first, second,
            "running redact_inline twice must be a no-op after the first pass",
        );
        // And every rule must have actually fired so the test doesn't
        // pass trivially when one pattern silently rots.
        assert!(first.contains("sk-ant-****"));
        assert!(first.contains("AKIA****"));
        assert!(first.contains("ghp_****"));
        assert!(first.contains("eyJ****"));
        assert!(first.contains("Bearer ****"));
    }
}
