//! Compact inline timestamp prefix for transcript message rendering (#9).
//!
//! Both the split-pane preview and the full-screen conversation viewer render
//! a stream of role-tagged messages. A plain `user {text}` / `claude {text}`
//! list gives the reader *what* was said but none of the *when*. This helper
//! emits a tiny timestamp prefix that adds session-timeline awareness without
//! overwhelming the message bodies.
//!
//! Format rules:
//! - First message shown → absolute local time, e.g. `14:32 · `
//! - Subsequent messages → relative delta since previous turn, e.g.
//!   `14:35 · +3m · `
//! - Deltas under 30s collapse to `14:35 ·  just now · `
//! - Deltas over 24h collapse to `14:40 ·  next day · `
//! - Missing timestamp → empty span list (renderer drops the prefix cleanly)
//!
//! All spans are styled via `theme.muted()` so they read as secondary detail
//! beside the message content.

use chrono::{DateTime, Local, Utc};
use ratatui::style::Style;
use ratatui::text::Span;

use crate::theme::Theme;

/// Build the compact timestamp prefix for one message.
///
/// `prev_ts` is the previous shown message's timestamp, if any. Passing
/// `None` means "this is the first message we're rendering" and emits the
/// absolute-time variant. Passing `Some` switches to the delta variant.
///
/// Returns a list of [`Span`]s ready to prepend onto the role-label line.
/// When `this_ts` is `None` the returned list is empty — callers should
/// simply skip prepending it rather than rendering a stray separator.
pub fn format_message_timestamp(
    prev_ts: Option<DateTime<Utc>>,
    this_ts: Option<DateTime<Utc>>,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let Some(ts) = this_ts else {
        return Vec::new();
    };

    let style: Style = theme.muted();
    let abs = ts.with_timezone(&Local).format("%H:%M").to_string();

    match prev_ts {
        None => {
            // First message: absolute time only, followed by our separator.
            vec![
                Span::styled(abs, style),
                Span::styled(" \u{00B7} ", style),
            ]
        }
        Some(prev) => {
            let delta = ts.signed_duration_since(prev);
            let delta_label = format_delta(delta);
            vec![
                Span::styled(abs, style),
                Span::styled(" \u{00B7} ", style),
                Span::styled(delta_label, style),
                Span::styled(" \u{00B7} ", style),
            ]
        }
    }
}

/// Collapse a `chrono::Duration` into the compact delta label the brief
/// calls for: `just now` when under 30s, `next day` over 24h, otherwise
/// `+Nm` / `+Ns` / `+Nh`. Negative deltas (clock skew, out-of-order JSONL)
/// are clamped to `just now` rather than rendered as `-3m`.
fn format_delta(delta: chrono::Duration) -> String {
    // Treat anything at-or-before prev as "just now" — we only want forward
    // deltas in the UI.
    let secs = delta.num_seconds();
    if secs < 30 {
        return " just now".to_string();
    }
    if delta.num_hours() >= 24 {
        return " next day".to_string();
    }
    let mins = delta.num_minutes();
    if mins < 1 {
        // 30..60s → show as Ns so the delta still reads non-trivial.
        return format!("+{secs}s");
    }
    if mins < 60 {
        return format!("+{mins}m");
    }
    let hours = delta.num_hours();
    format!("+{hours}h")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn theme() -> Theme {
        Theme::default()
    }

    fn ts(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    /// Flatten the span list back to plain text so tests can assert on the
    /// rendered label without coupling to `ratatui::Span` equality.
    fn plain(spans: &[Span<'_>]) -> String {
        spans.iter().map(|s| s.content.to_string()).collect()
    }

    #[test]
    fn formats_first_message_as_absolute_time() {
        // prev_ts = None → absolute-time variant, no delta.
        let now = ts(2026, 4, 22, 14, 32, 10);
        let spans = format_message_timestamp(None, Some(now), &theme());
        let rendered = plain(&spans);
        // We can't hardcode "14:32" because the test host's TZ offset is
        // unknown — assert on the structural shape instead: exactly two
        // spans (time + separator) and a `HH:MM · ` tail.
        assert_eq!(spans.len(), 2);
        assert!(rendered.ends_with(" \u{00B7} "));
        // Time part should be 5 chars of HH:MM form.
        let time_part = &rendered[..5];
        assert!(
            time_part.chars().nth(2) == Some(':'),
            "expected HH:MM time, got {time_part:?}"
        );
    }

    #[test]
    fn formats_followup_as_relative_delta() {
        // +3 minute delta → absolute time + "+3m" label + two separators.
        let prev = ts(2026, 4, 22, 14, 32, 0);
        let now = ts(2026, 4, 22, 14, 35, 0);
        let spans = format_message_timestamp(Some(prev), Some(now), &theme());
        let rendered = plain(&spans);
        assert_eq!(spans.len(), 4, "abs + sep + delta + sep");
        assert!(
            rendered.contains("+3m"),
            "expected +3m delta, got {rendered:?}"
        );
    }

    #[test]
    fn renders_just_now_under_30s() {
        let prev = ts(2026, 4, 22, 14, 32, 0);
        let now = ts(2026, 4, 22, 14, 32, 15); // +15s
        let spans = format_message_timestamp(Some(prev), Some(now), &theme());
        let rendered = plain(&spans);
        assert!(
            rendered.contains("just now"),
            "expected `just now`, got {rendered:?}"
        );
        assert!(
            !rendered.contains("+15s"),
            "should not show seconds delta under 30s, got {rendered:?}"
        );
    }

    #[test]
    fn renders_next_day_across_midnight() {
        let prev = ts(2026, 4, 22, 23, 55, 0);
        let now = ts(2026, 4, 24, 0, 10, 0); // >24h gap
        let spans = format_message_timestamp(Some(prev), Some(now), &theme());
        let rendered = plain(&spans);
        assert!(
            rendered.contains("next day"),
            "expected `next day`, got {rendered:?}"
        );
    }

    #[test]
    fn handles_missing_timestamp_gracefully() {
        // this_ts = None → empty span list; callers drop the prefix.
        let spans = format_message_timestamp(None, None, &theme());
        assert!(spans.is_empty(), "no timestamp → no spans");

        let prev = ts(2026, 4, 22, 10, 0, 0);
        let spans = format_message_timestamp(Some(prev), None, &theme());
        assert!(spans.is_empty(), "missing this_ts → no spans even with prev");
    }

    #[test]
    fn shows_seconds_delta_between_30s_and_1m() {
        // Sanity: 45s should land in the `+45s` branch, not `just now` or `+0m`.
        let prev = ts(2026, 4, 22, 10, 0, 0);
        let now = ts(2026, 4, 22, 10, 0, 45);
        let spans = format_message_timestamp(Some(prev), Some(now), &theme());
        let rendered = plain(&spans);
        assert!(rendered.contains("+45s"), "got {rendered:?}");
    }

    #[test]
    fn shows_hours_delta_over_one_hour() {
        // 2h 15m delta → label is `+2h` (hours granularity once we cross 1h).
        let prev = ts(2026, 4, 22, 10, 0, 0);
        let now = ts(2026, 4, 22, 12, 15, 0);
        let spans = format_message_timestamp(Some(prev), Some(now), &theme());
        let rendered = plain(&spans);
        assert!(rendered.contains("+2h"), "got {rendered:?}");
    }
}
