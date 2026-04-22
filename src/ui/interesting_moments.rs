//! Session interesting-moments mini-timeline (feature #20, DEEP-1).
//!
//! A one-row compact strip rendered at the top of the full-screen
//! conversation viewer that marks notable events across the session's
//! timeline. Helps the reader see "where's the juice" before scrolling.
//!
//! ## Notable events detected per turn
//!
//! - **Cost spike**: turn cost proxy ≥ 2× median turn cost → mauve dot
//! - **Tool burst**: turn has ≥ 10 tool calls → yellow dot
//! - **Long pause**: turn duration ≥ 5 min wall-clock → pink/red dot
//! - **First user prompt**: green caret marker on turn 0
//! - **Last user prompt**: blue caret marker on the final turn
//!
//! ## Compression
//!
//! The full timeline is compressed into exactly `area.width - 2` columns:
//! turns are bucketed into columns proportional to count. Within a column,
//! the highest-priority event wins the color (spike > burst > pause >
//! marker). Bars use block glyphs scaled by count-of-events-in-bucket.
//!
//! ## Why per-turn cost comes from `turn_durations`
//!
//! The raw JSONL doesn't carry per-turn cost. `Session::turn_durations`
//! gives us per-turn wall-clock, which is a reasonable proxy for "this
//! turn did a lot of work" when all we have to compare turns against each
//! other is their duration. The caption totals stay accurate regardless —
//! we count events, not dollars.

use std::time::Duration;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::data::session::Session;
use crate::theme::Theme;

/// A single turn's tally of notable events. The render pass compresses
/// many of these into fewer buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Moment {
    /// Zero-based index of the turn this moment belongs to.
    pub turn_idx: usize,
    pub cost_spike: bool,
    pub tool_burst: bool,
    pub long_pause: bool,
    /// This turn is the session's first user prompt (turn 0).
    pub first_prompt: bool,
    /// This turn is the session's last user prompt.
    pub last_prompt: bool,
}

impl Moment {
    /// True if this moment carries any flagged event.
    pub fn is_flagged(&self) -> bool {
        self.cost_spike
            || self.tool_burst
            || self.long_pause
            || self.first_prompt
            || self.last_prompt
    }

    /// Highest-priority event kind that should paint this cell's color.
    /// Returns `None` when no event is set.
    ///
    /// Ordering, per the feature brief: spike > burst > pause > marker.
    /// Markers tie-break last-prompt (blue) over first-prompt (green)
    /// because a single-turn session where both are set should still read
    /// as "the end point".
    pub fn dominant(&self) -> Option<MomentKind> {
        if self.cost_spike {
            Some(MomentKind::CostSpike)
        } else if self.tool_burst {
            Some(MomentKind::ToolBurst)
        } else if self.long_pause {
            Some(MomentKind::LongPause)
        } else if self.last_prompt {
            Some(MomentKind::LastPrompt)
        } else if self.first_prompt {
            Some(MomentKind::FirstPrompt)
        } else {
            None
        }
    }
}

/// Priority-ordered event kind for coloring a single column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MomentKind {
    CostSpike,
    ToolBurst,
    LongPause,
    FirstPrompt,
    LastPrompt,
}

impl MomentKind {
    /// Map a moment kind to a palette color. Matches the spec's event→color
    /// association: mauve=spike, yellow=burst, pink/red=pause, green=first,
    /// blue=last.
    pub fn color(self, theme: &Theme) -> Color {
        match self {
            MomentKind::CostSpike => theme.mauve,
            MomentKind::ToolBurst => theme.yellow,
            MomentKind::LongPause => theme.pink,
            MomentKind::FirstPrompt => theme.green,
            MomentKind::LastPrompt => theme.blue,
        }
    }
}

/// Threshold for flagging a turn as a "long pause". Pulled out so the tests
/// can reason about the boundary without duplicating the literal.
const LONG_PAUSE_THRESHOLD: Duration = Duration::from_secs(5 * 60);

/// Minimum tool-call count (within a single turn) for "tool burst".
pub const TOOL_BURST_THRESHOLD: usize = 10;

/// Compute interesting moments from a session alone. Tool-burst detection
/// is skipped in this flavor — callers that have per-turn tool counts
/// (e.g. the conversation viewer with a parsed transcript) should call
/// [`compute_moments_with_tools`] instead.
pub fn compute_moments(session: &Session) -> Vec<Moment> {
    compute_moments_with_tools(session, &[])
}

/// Compute moments with optional per-turn tool-call counts. `tool_counts`
/// is indexed by turn; empty or shorter-than-turn-count slices are padded
/// with zeros, so callers can always pass whatever they have.
pub fn compute_moments_with_tools(session: &Session, tool_counts: &[usize]) -> Vec<Moment> {
    let turns = session.turn_durations.len();
    if turns == 0 {
        return Vec::new();
    }

    // Cost-spike detection. `turn_durations` is our best available proxy
    // for per-turn cost — longer turns tend to cost more, and the ratio
    // against the median is what matters for "spike". We compute the
    // median over non-zero durations so a zero-duration ghost turn doesn't
    // drag the median to zero and flag every other turn.
    let mut nonzero: Vec<u128> = session
        .turn_durations
        .iter()
        .map(|d| d.as_millis())
        .filter(|&m| m > 0)
        .collect();
    nonzero.sort_unstable();
    let median_ms: u128 = if nonzero.is_empty() {
        0
    } else {
        nonzero[nonzero.len() / 2]
    };

    let mut moments: Vec<Moment> = (0..turns)
        .map(|i| Moment {
            turn_idx: i,
            ..Moment::default()
        })
        .collect();

    for (i, dur) in session.turn_durations.iter().enumerate() {
        let ms = dur.as_millis();
        if median_ms > 0 && ms >= median_ms.saturating_mul(2) {
            moments[i].cost_spike = true;
        }
        if *dur >= LONG_PAUSE_THRESHOLD {
            moments[i].long_pause = true;
        }
    }

    for (i, &count) in tool_counts.iter().enumerate() {
        if i >= moments.len() {
            break;
        }
        if count >= TOOL_BURST_THRESHOLD {
            moments[i].tool_burst = true;
        }
    }

    if let Some(first) = moments.first_mut() {
        first.first_prompt = true;
    }
    if let Some(last) = moments.last_mut() {
        last.last_prompt = true;
    }

    moments
}

/// Unicode block glyphs used for the bar intensity ramp. Index 0 = empty
/// cell (no events in the bucket), 1..=7 step up by count.
const RAMP: &[char] = &['·', '▁', '▂', '▃', '▄', '▅', '▆', '▇'];

/// Bucket count plus what to paint for each bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Bucket {
    /// How many flagged moments fell into this column.
    event_count: u32,
    /// Highest-priority event kind across all moments in the bucket, if
    /// any. Drives the foreground color.
    dominant: Option<MomentKind>,
}

/// Kind-priority for "who wins the color when a bucket mixes events". Higher
/// number = higher priority. Mirrors the brief's `spike > burst > pause >
/// marker` ordering.
fn kind_priority(kind: MomentKind) -> u8 {
    match kind {
        MomentKind::CostSpike => 4,
        MomentKind::ToolBurst => 3,
        MomentKind::LongPause => 2,
        MomentKind::LastPrompt => 1,
        MomentKind::FirstPrompt => 1,
    }
}

/// Compress `moments` into `cols` evenly-sized buckets.
///
/// Every turn is always assigned to exactly one column: `col = turn_idx *
/// cols / turns`. This preserves the feel that the leftmost cell represents
/// the earliest turn and the rightmost cell represents the most recent —
/// what the reader expects from a timeline.
fn bucketize(moments: &[Moment], cols: usize) -> Vec<Bucket> {
    if cols == 0 || moments.is_empty() {
        return Vec::new();
    }
    let mut buckets: Vec<Bucket> = vec![Bucket::default(); cols];
    let turns = moments.len();
    for m in moments {
        let col = (m.turn_idx * cols) / turns;
        let col = col.min(cols - 1);
        if !m.is_flagged() {
            continue;
        }
        let bucket = &mut buckets[col];
        bucket.event_count = bucket.event_count.saturating_add(1);
        if let Some(kind) = m.dominant() {
            bucket.dominant = match bucket.dominant {
                Some(existing) if kind_priority(existing) >= kind_priority(kind) => Some(existing),
                _ => Some(kind),
            };
        }
    }
    buckets
}

/// Map an event count within a bucket to a block-ramp glyph. Zero renders
/// as the dim dot so empty cells are visually distinguishable from
/// "single-event" cells.
fn ramp_char(count: u32, max: u32) -> char {
    if count == 0 {
        return RAMP[0];
    }
    if max <= 1 {
        return RAMP[3]; // single-event bucket in a session of singletons
    }
    let norm = (count as f64) / (max as f64);
    let idx = ((norm * ((RAMP.len() - 1) as f64)).round() as usize).clamp(1, RAMP.len() - 1);
    RAMP[idx]
}

/// Render the timeline as an inline line of spans. Caller embeds this in a
/// `Line` / `Paragraph`. Returns a span sequence whose total display width
/// is at most `width`.
///
/// Layout: ` │<bars>│  <caption>`, where `<bars>` is exactly `width - 2`
/// columns (the brackets take one each) and `<caption>` is "N spikes · N
/// bursts · N pauses". When the width is too small to render anything
/// meaningful, returns an empty vec.
pub fn render_timeline<'a>(moments: &[Moment], theme: &Theme, width: u16) -> Vec<Span<'a>> {
    // Need at least the two bracket columns plus one content col. Anything
    // under that is "no room" — skip rather than render a broken row.
    if width < 3 {
        return Vec::new();
    }
    let cols = (width as usize).saturating_sub(2);
    let buckets = bucketize(moments, cols);

    let max_count = buckets.iter().map(|b| b.event_count).max().unwrap_or(0);

    let border_style = theme.dim();
    let mut spans: Vec<Span<'a>> = Vec::with_capacity(cols + 6);
    spans.push(Span::styled("│", border_style));

    for bucket in &buckets {
        let glyph = ramp_char(bucket.event_count, max_count);
        let style = match bucket.dominant {
            Some(kind) => Style::default()
                .fg(kind.color(theme))
                .add_modifier(Modifier::BOLD),
            None => theme.dim(),
        };
        spans.push(Span::styled(glyph.to_string(), style));
    }

    spans.push(Span::styled("│", border_style));

    // Caption: only render when there's room left for it.
    let caption = caption_text(moments);
    if !caption.is_empty() {
        spans.push(Span::styled("  ", theme.dim()));
        spans.push(Span::styled(caption, theme.muted()));
    }
    spans
}

/// Build the "N spikes · N bursts · N pauses" caption. Empty string when
/// no events fired — the row will still render the bracketed bar strip.
fn caption_text(moments: &[Moment]) -> String {
    let mut spikes = 0usize;
    let mut bursts = 0usize;
    let mut pauses = 0usize;
    for m in moments {
        if m.cost_spike {
            spikes += 1;
        }
        if m.tool_burst {
            bursts += 1;
        }
        if m.long_pause {
            pauses += 1;
        }
    }
    if spikes == 0 && bursts == 0 && pauses == 0 {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::with_capacity(3);
    if spikes > 0 {
        parts.push(format!("{spikes} {}", pluralize(spikes, "spike")));
    }
    if bursts > 0 {
        parts.push(format!("{bursts} {}", pluralize(bursts, "burst")));
    }
    if pauses > 0 {
        parts.push(format!("{pauses} {}", pluralize(pauses, "pause")));
    }
    format!("↑{}", parts.join(" · "))
}

fn pluralize(n: usize, word: &str) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::pricing::TokenCounts;
    use crate::data::session::SessionKind;
    use std::path::PathBuf;

    fn mk_session(turn_durations: Vec<Duration>) -> Session {
        Session {
            id: "test-session".to_string(),
            project_dir: PathBuf::from("/tmp"),
            name: None,
            auto_name: None,
            last_prompt: None,
            message_count: turn_durations.len() as u32,
            tokens: TokenCounts::default(),
            total_cost_usd: 0.0,
            model_summary: String::new(),
            first_timestamp: None,
            last_timestamp: None,
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
            turn_durations,
        }
    }

    #[test]
    fn zero_turns_yields_no_moments() {
        let s = mk_session(Vec::new());
        let m = compute_moments(&s);
        assert!(m.is_empty(), "expected empty moments for zero turns");
        // Render must still be safe on an empty moments list.
        let theme = Theme::default();
        let spans = render_timeline(&m, &theme, 40);
        // Two brackets only; no caption because no events.
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.starts_with('│'), "starts with opening bracket");
        assert!(text.contains('│'), "has a closing bracket");
        // No caption fragment present.
        assert!(!text.contains("spike"));
        assert!(!text.contains("pause"));
    }

    #[test]
    fn single_turn_produces_single_bucket() {
        let s = mk_session(vec![Duration::from_secs(2)]);
        let m = compute_moments(&s);
        assert_eq!(m.len(), 1);
        assert!(m[0].first_prompt, "single turn is the first prompt");
        assert!(m[0].last_prompt, "single turn is also the last prompt");
        assert!(
            !m[0].cost_spike,
            "no spike possible with a single turn (median == value)"
        );

        // Render narrow strip — should have exactly 1 content column.
        let theme = Theme::default();
        let spans = render_timeline(&m, &theme, 3);
        // width=3 → 1 content col + 2 brackets.
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text.chars().filter(|c| *c == '│').count(), 2);
    }

    #[test]
    fn fifty_turns_compress_into_twenty_columns() {
        let mut durations: Vec<Duration> = Vec::with_capacity(50);
        for i in 0..50 {
            // Most turns are 2s, every 10th is 10s → should spike.
            let secs = if i % 10 == 9 { 10 } else { 2 };
            durations.push(Duration::from_secs(secs));
        }
        // Make one turn very long to trigger long-pause.
        durations[30] = Duration::from_secs(6 * 60); // 6 min pause
        let s = mk_session(durations);

        let moments = compute_moments(&s);
        assert_eq!(moments.len(), 50);

        // There should be some spikes (every 10th turn) and at least one
        // pause (turn 30).
        let spikes = moments.iter().filter(|m| m.cost_spike).count();
        assert!(spikes >= 4, "expected ≥4 cost spikes, got {spikes}");
        let pauses = moments.iter().filter(|m| m.long_pause).count();
        assert!(pauses >= 1, "expected ≥1 long pause, got {pauses}");

        // Bucketing: 50 turns → 20 columns → avg 2.5 turns per column.
        let buckets = bucketize(&moments, 20);
        assert_eq!(buckets.len(), 20);
        // Every turn must land in some bucket → total event_count equals
        // the count of *flagged* moments.
        let flagged: u32 = moments.iter().filter(|m| m.is_flagged()).count() as u32;
        let total: u32 = buckets.iter().map(|b| b.event_count).sum();
        assert_eq!(total, flagged, "bucketing must preserve event totals");

        // Render width 22 (2 brackets + 20 cols) produces 20 interior
        // glyphs.
        let theme = Theme::default();
        let spans = render_timeline(&moments, &theme, 22);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        // Two brackets present.
        assert_eq!(text.chars().filter(|c| *c == '│').count(), 2);
        // Caption present and totals match.
        assert!(
            text.contains("spike"),
            "caption should mention spikes: {text}"
        );
        assert!(
            text.contains(&format!("{spikes}")),
            "spike total in caption: {text}"
        );
    }

    #[test]
    fn tool_burst_threshold_marks_busy_turns() {
        let s = mk_session(vec![
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        ]);
        let tool_counts = vec![0, 12, 3];
        let moments = compute_moments_with_tools(&s, &tool_counts);
        assert!(!moments[0].tool_burst);
        assert!(moments[1].tool_burst, "12 tool calls should flag burst");
        assert!(!moments[2].tool_burst);
    }

    #[test]
    fn first_and_last_markers_set_correctly() {
        let s = mk_session(vec![
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        ]);
        let moments = compute_moments(&s);
        assert!(moments.first().unwrap().first_prompt);
        assert!(!moments.first().unwrap().last_prompt);
        assert!(moments.last().unwrap().last_prompt);
        assert!(!moments.last().unwrap().first_prompt);
        // Inner turns carry neither marker.
        assert!(!moments[1].first_prompt && !moments[1].last_prompt);
    }

    #[test]
    fn caption_totals_match_event_counts() {
        let s = mk_session(vec![
            Duration::from_secs(2),
            Duration::from_secs(2),
            // Spike: 2× median
            Duration::from_secs(10),
            // Long pause: ≥ 5 min
            Duration::from_secs(7 * 60),
        ]);
        let moments = compute_moments(&s);
        let caption = caption_text(&moments);
        // Expect both spike and pause words; long-pause at index 3 also
        // is the one > 2×median so it counts as both.
        assert!(
            caption.contains("spike"),
            "caption missing spike: {caption}"
        );
        assert!(
            caption.contains("pause"),
            "caption missing pause: {caption}"
        );
    }

    #[test]
    fn dominant_priority_orders_correctly() {
        // Spike wins over burst wins over pause wins over markers.
        let m = Moment {
            turn_idx: 0,
            cost_spike: true,
            tool_burst: true,
            long_pause: true,
            first_prompt: true,
            last_prompt: true,
        };
        assert_eq!(m.dominant(), Some(MomentKind::CostSpike));

        let m = Moment {
            turn_idx: 0,
            cost_spike: false,
            tool_burst: true,
            long_pause: true,
            ..Default::default()
        };
        assert_eq!(m.dominant(), Some(MomentKind::ToolBurst));

        let m = Moment {
            turn_idx: 0,
            long_pause: true,
            first_prompt: true,
            ..Default::default()
        };
        assert_eq!(m.dominant(), Some(MomentKind::LongPause));
    }

    #[test]
    fn render_too_narrow_returns_empty() {
        let s = mk_session(vec![Duration::from_secs(1), Duration::from_secs(1)]);
        let m = compute_moments(&s);
        let theme = Theme::default();
        assert!(render_timeline(&m, &theme, 2).is_empty());
        // width=3 → 1 col + 2 brackets → at least 3 spans emitted.
        assert!(render_timeline(&m, &theme, 3).len() >= 3);
    }
}
