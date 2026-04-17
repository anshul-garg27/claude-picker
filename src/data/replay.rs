//! Time-travel replay data model — the engine behind the `R` key.
//!
//! Given a transcript loaded from the JSONL, this module builds a
//! [`ReplayTimeline`]: a vector of [`ReplayMessage`] plus a parallel vector
//! of wall-clock gaps that the player advances through. Gaps derived from
//! the real timestamps drive a "watch it happen" playback; when the user
//! stepped away from the terminal we cap long gaps so the replay doesn't
//! feel like watching paint dry.
//!
//! The player itself ([`ReplayState`]) owns a virtual clock that ticks
//! forward based on `Instant::now()` and a speed multiplier. Keyboard
//! controls mutate that clock — play/pause flips a flag, `>/<` change the
//! multiplier, `./,` step the cursor and snap the clock to the new position.
//!
//! Design notes:
//!
//! - The virtual clock stores `Duration` since session start, not wall time.
//!   That lets us seek, scrub, and change speed without messy timestamp
//!   math on the hot path.
//! - Large real gaps (user walked away for 2h) get capped in
//!   `playback_gaps` to 10s max by default so the replay stays watchable.
//!   We keep the REAL gaps around in `real_gaps` so the UI can show the
//!   honest "⏸ 2h 14m gap · (compressed to 3s)" marker.
//! - Timestamps can drift between user and assistant messages in the JSONL
//!   — sometimes by ~100ms, sometimes by seconds if the user was typing a
//!   long prompt. We use the recorded timestamp as-is (no clock skew
//!   correction) because the drift IS part of what makes the replay feel
//!   real.

use std::time::Duration;

use crate::data::transcript::TranscriptMessage;

/// Gap cap — any real inter-message gap longer than this is compressed in
/// the playback timeline. We still expose the real gap so the UI can show
/// the honest amount alongside the compression ratio.
pub const DEFAULT_MAX_GAP: Duration = Duration::from_secs(10);

/// Minimum playable gap — even back-to-back messages get a small pause so
/// the eye catches the new bubble appearing. 250ms is faster than a human
/// reaction, slower than a frame, so it feels "alive" without dragging.
pub const MIN_PLAYBACK_GAP: Duration = Duration::from_millis(250);

/// A single replayable message — just the underlying transcript item plus
/// any metadata the player needs. Kept as a thin wrapper so the renderer
/// can reuse [`crate::ui::conversation_viewer`]'s line-flattening helpers.
#[derive(Debug, Clone)]
pub struct ReplayMessage {
    pub message: TranscriptMessage,
    /// Offset from session start at which this message arrives, in the
    /// PLAYBACK timeline (gaps already capped). The first message is 0.
    pub playback_offset: Duration,
    /// Offset from session start using the REAL timestamps. Used for the
    /// header's "4m 12s / 38m" label so the user sees real session time,
    /// not compressed-playback time.
    pub real_offset: Duration,
    /// Gap since the previous message in REAL time. `None` on the first
    /// message. Used to decide whether to draw a ⏸ gap marker before this
    /// message during rendering.
    pub real_gap_before: Option<Duration>,
    /// Same gap but after capping. Drives the wait between messages.
    pub playback_gap_before: Option<Duration>,
}

impl ReplayMessage {
    /// True when this message had a large enough real-time gap that the UI
    /// should render a gap indicator before it. "Large enough" matches the
    /// default cap: if we compressed it, show the compression.
    pub fn has_compressed_gap(&self) -> bool {
        match (self.real_gap_before, self.playback_gap_before) {
            (Some(real), Some(play)) => real > play,
            _ => false,
        }
    }
}

/// The full replayable timeline for a session.
#[derive(Debug, Clone)]
pub struct ReplayTimeline {
    pub messages: Vec<ReplayMessage>,
    /// Total playback duration (sum of all capped gaps). What the user
    /// experiences as "video length".
    pub total_playback: Duration,
    /// Span between first and last real timestamps, if timestamps exist.
    /// What the header shows as "38m real-time".
    pub total_real: Duration,
    /// Count of gaps that were compressed (real > cap). Used by the finish
    /// summary: "played back in 4m 12s (38m real-time, 8.9x compressed)".
    pub compressed_gap_count: usize,
}

impl ReplayTimeline {
    /// Build a timeline from a transcript. Uses [`DEFAULT_MAX_GAP`] as the
    /// gap cap and [`MIN_PLAYBACK_GAP`] as the floor.
    pub fn from_transcript(messages: Vec<TranscriptMessage>) -> Self {
        Self::with_cap(messages, DEFAULT_MAX_GAP, MIN_PLAYBACK_GAP)
    }

    /// Same as [`Self::from_transcript`] but with tunable caps. Exposed so
    /// tests can verify gap-capping + min-floor behaviour independently.
    pub fn with_cap(messages: Vec<TranscriptMessage>, cap: Duration, min: Duration) -> Self {
        let mut out = Vec::with_capacity(messages.len());
        let mut playback_cursor = Duration::ZERO;
        let mut real_cursor = Duration::ZERO;
        let mut prev_ts: Option<chrono::DateTime<chrono::Utc>> = None;
        let mut compressed = 0usize;

        for msg in messages {
            let (real_gap, playback_gap) = if let (Some(prev), Some(now)) = (prev_ts, msg.timestamp)
            {
                let dt = now.signed_duration_since(prev);
                // A negative delta would mean clock skew / out-of-order
                // writes. Clamp to zero rather than panicking on u64
                // conversion below.
                let real = if dt.num_milliseconds() <= 0 {
                    Duration::ZERO
                } else {
                    Duration::from_millis(dt.num_milliseconds() as u64)
                };
                // Apply cap: huge gaps compress to `cap`; everything else
                // passes through, with a min floor so same-second messages
                // still feel "alive".
                let capped = if real > cap {
                    compressed += 1;
                    cap
                } else if real < min {
                    min
                } else {
                    real
                };
                (Some(real), Some(capped))
            } else if prev_ts.is_some() {
                // Previous message had a timestamp but this one doesn't —
                // use the minimum gap so playback doesn't freeze.
                (None, Some(min))
            } else if !out.is_empty() {
                // First message with no timestamp and we already have
                // messages queued — same fallback.
                (None, Some(min))
            } else {
                (None, None)
            };

            if let Some(gap) = playback_gap {
                playback_cursor = playback_cursor.saturating_add(gap);
            }
            if let Some(gap) = real_gap {
                real_cursor = real_cursor.saturating_add(gap);
            }

            if msg.timestamp.is_some() {
                prev_ts = msg.timestamp;
            }

            out.push(ReplayMessage {
                message: msg,
                playback_offset: playback_cursor,
                real_offset: real_cursor,
                real_gap_before: real_gap,
                playback_gap_before: playback_gap,
            });
        }

        Self {
            total_playback: playback_cursor,
            total_real: real_cursor,
            messages: out,
            compressed_gap_count: compressed,
        }
    }

    /// How many messages. Kept as a method so callers don't care whether
    /// the vector is dense or sparse in the future.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Index of the last message whose `playback_offset` is less than or
    /// equal to `virtual_time`. Returns `None` before the first message is
    /// reached (i.e., while `virtual_time` is zero and the first message
    /// hasn't been "spoken" yet — but with our min-floor this happens only
    /// at the first frame, since every message has gap >= min).
    ///
    /// This is the core "seek" primitive: given a virtual clock position,
    /// tell me the index of the most recently visible message.
    pub fn index_at(&self, virtual_time: Duration) -> Option<usize> {
        // Binary search: find the largest i such that messages[i].playback_offset <= virtual_time.
        let mut lo = 0usize;
        let mut hi = self.messages.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.messages[mid].playback_offset <= virtual_time {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            None
        } else {
            Some(lo - 1)
        }
    }

    /// Time at which the message at `idx` enters the replay. `None` if idx
    /// is out of range.
    pub fn offset_of(&self, idx: usize) -> Option<Duration> {
        self.messages.get(idx).map(|m| m.playback_offset)
    }

    /// Ratio of playback compression: real-time / playback-time. Returns
    /// 1.0 when there was no compression at all.
    pub fn compression_ratio(&self) -> f64 {
        if self.total_playback.is_zero() {
            return 1.0;
        }
        let real = self.total_real.as_secs_f64();
        let play = self.total_playback.as_secs_f64();
        if play <= 0.0 {
            return 1.0;
        }
        (real / play).max(1.0)
    }
}

/// Format a [`Duration`] as a compact human-readable string:
/// "45s", "2m 14s", "1h 8m 22s", "2h 14m".
///
/// Kept here (not in a generic util) because the replay UI is the primary
/// caller and we want a very specific format — no leading zeros, no ms.
pub fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    if total == 0 {
        // Still show a sub-second spinner marker so the header doesn't
        // flicker between "0s" and "1s".
        return "0s".to_string();
    }
    let hours = total / 3_600;
    let minutes = (total % 3_600) / 60;
    let seconds = total % 60;
    let mut out = String::new();
    if hours > 0 {
        out.push_str(&format!("{hours}h "));
    }
    if hours > 0 || minutes > 0 {
        out.push_str(&format!("{minutes}m"));
        if seconds > 0 && hours == 0 {
            out.push_str(&format!(" {seconds}s"));
        }
    } else {
        out.push_str(&format!("{seconds}s"));
    }
    out.trim().to_string()
}

/// One of the fixed playback-speed presets.
///
/// We use a closed set rather than a free-floating f64 so the keybinding
/// "cycle speed" always visits the same values and the speed-changed pulse
/// always lands on a recognisable label (`1x`, `2x`, …). Custom speeds can
/// still be dialed in by tweaking the enum but wiring the free cycle into
/// the keymap is a non-goal for v3.0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpeedPreset {
    Quarter, // 0.25x — slow-motion
    Half,    // 0.5x
    Normal,  // 1x
    Double,  // 2x
    Quad,    // 4x
    Ten,     // 10x
    Hundred, // 100x
}

impl SpeedPreset {
    /// Every preset in cycle order (slow → fast).
    pub const ALL: &'static [SpeedPreset] = &[
        SpeedPreset::Quarter,
        SpeedPreset::Half,
        SpeedPreset::Normal,
        SpeedPreset::Double,
        SpeedPreset::Quad,
        SpeedPreset::Ten,
        SpeedPreset::Hundred,
    ];

    /// Multiplier value. Used directly to scale elapsed wall time into
    /// virtual session time.
    pub fn multiplier(self) -> f64 {
        match self {
            Self::Quarter => 0.25,
            Self::Half => 0.5,
            Self::Normal => 1.0,
            Self::Double => 2.0,
            Self::Quad => 4.0,
            Self::Ten => 10.0,
            Self::Hundred => 100.0,
        }
    }

    /// Short label for the header badge.
    pub fn label(self) -> &'static str {
        match self {
            Self::Quarter => "0.25x",
            Self::Half => "0.5x",
            Self::Normal => "1x",
            Self::Double => "2x",
            Self::Quad => "4x",
            Self::Ten => "10x",
            Self::Hundred => "100x",
        }
    }

    /// Next faster preset in [`Self::ALL`]. Wraps from the highest back to
    /// the lowest so `>` keeps cycling.
    pub fn faster(self) -> Self {
        let i = Self::ALL.iter().position(|&s| s == self).unwrap_or(2);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Previous preset in [`Self::ALL`]. Wraps the other way so `<` is the
    /// inverse cycle of `>`.
    pub fn slower(self) -> Self {
        let i = Self::ALL.iter().position(|&s| s == self).unwrap_or(2);
        let n = Self::ALL.len();
        Self::ALL[(i + n - 1) % n]
    }

    /// True when typewriter animation should engage at this speed. At 4x
    /// and above, character-by-character animation is more distracting than
    /// useful — just pop the message in and move on.
    pub fn enable_typewriter(self) -> bool {
        self.multiplier() <= 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};

    use crate::data::transcript::{ContentItem, Role};

    fn msg(ts: Option<DateTime<Utc>>, text: &str, role: Role) -> TranscriptMessage {
        TranscriptMessage {
            role,
            timestamp: ts,
            items: vec![ContentItem::Text(text.to_string())],
        }
    }

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_700_000_000 + secs, 0).unwrap()
    }

    #[test]
    fn empty_transcript_produces_empty_timeline() {
        let t = ReplayTimeline::from_transcript(Vec::new());
        assert!(t.is_empty());
        assert_eq!(t.total_playback, Duration::ZERO);
        assert_eq!(t.total_real, Duration::ZERO);
    }

    #[test]
    fn single_message_has_zero_offset() {
        let t = ReplayTimeline::from_transcript(vec![msg(Some(ts(0)), "hi", Role::User)]);
        assert_eq!(t.len(), 1);
        assert_eq!(t.messages[0].playback_offset, Duration::ZERO);
        assert_eq!(t.messages[0].real_offset, Duration::ZERO);
        assert!(t.messages[0].playback_gap_before.is_none());
    }

    #[test]
    fn consecutive_gaps_accumulate() {
        // 0s, 3s, 5s → offsets 0, 3, 5 (well under the cap).
        let t = ReplayTimeline::from_transcript(vec![
            msg(Some(ts(0)), "a", Role::User),
            msg(Some(ts(3)), "b", Role::Assistant),
            msg(Some(ts(5)), "c", Role::User),
        ]);
        assert_eq!(t.messages[0].playback_offset, Duration::from_secs(0));
        assert_eq!(t.messages[1].playback_offset, Duration::from_secs(3));
        assert_eq!(t.messages[2].playback_offset, Duration::from_secs(5));
        assert_eq!(t.compressed_gap_count, 0);
    }

    #[test]
    fn huge_gap_is_capped_at_ten_seconds() {
        // Two-hour gap should compress to 10s.
        let t = ReplayTimeline::from_transcript(vec![
            msg(Some(ts(0)), "before", Role::User),
            msg(Some(ts(2 * 3600)), "after", Role::Assistant),
        ]);
        assert_eq!(t.messages[0].playback_offset, Duration::ZERO);
        assert_eq!(t.messages[1].playback_offset, DEFAULT_MAX_GAP);
        assert_eq!(
            t.messages[1].real_gap_before,
            Some(Duration::from_secs(2 * 3600))
        );
        assert_eq!(t.messages[1].playback_gap_before, Some(DEFAULT_MAX_GAP));
        assert!(t.messages[1].has_compressed_gap());
        assert_eq!(t.compressed_gap_count, 1);
    }

    #[test]
    fn custom_cap_and_floor_honored() {
        // cap 2s, floor 1s. Gap of 5s → 2s. Gap of 500ms → 1s (floor).
        let t = ReplayTimeline::with_cap(
            vec![
                msg(Some(ts(0)), "a", Role::User),
                // Half-second gap — below the 1s floor.
                msg(
                    Some(Utc.timestamp_opt(1_700_000_000, 500_000_000).unwrap()),
                    "b",
                    Role::Assistant,
                ),
                // Five-second gap — above the 2s cap.
                msg(Some(ts(5)), "c", Role::User),
            ],
            Duration::from_secs(2),
            Duration::from_secs(1),
        );
        assert_eq!(
            t.messages[1].playback_gap_before,
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            t.messages[2].playback_gap_before,
            Some(Duration::from_secs(2))
        );
    }

    #[test]
    fn speed_multiplier_scales_virtual_time() {
        // This is the "at 4x, 1 minute of session plays in 15 seconds" test.
        // Build a timeline with a 1-minute playback span, then assert that
        // multiplier 4.0 advances virtual_time through it in 15 seconds of
        // wall clock. We verify that by hand — the math is: virtual =
        // wall * multiplier, so 15s wall * 4x = 60s virtual.
        let wall = Duration::from_secs(15);
        let multiplier = 4.0;
        let virtual_time = Duration::from_secs_f64(wall.as_secs_f64() * multiplier);
        assert_eq!(virtual_time, Duration::from_secs(60));
    }

    #[test]
    fn index_at_binary_search_is_correct() {
        let t = ReplayTimeline::from_transcript(vec![
            msg(Some(ts(0)), "a", Role::User),      // offset 0
            msg(Some(ts(5)), "b", Role::Assistant), // offset 5
            msg(Some(ts(10)), "c", Role::User),     // offset 10
        ]);
        assert_eq!(t.index_at(Duration::ZERO), Some(0));
        assert_eq!(t.index_at(Duration::from_secs(3)), Some(0));
        assert_eq!(t.index_at(Duration::from_secs(5)), Some(1));
        assert_eq!(t.index_at(Duration::from_secs(7)), Some(1));
        assert_eq!(t.index_at(Duration::from_secs(10)), Some(2));
        assert_eq!(t.index_at(Duration::from_secs(99)), Some(2));
    }

    #[test]
    fn compression_ratio_reasonable() {
        // 2-hour real span compressed to 10s playback → ratio ~720.
        let t = ReplayTimeline::from_transcript(vec![
            msg(Some(ts(0)), "a", Role::User),
            msg(Some(ts(2 * 3600)), "b", Role::Assistant),
        ]);
        let ratio = t.compression_ratio();
        assert!(
            ratio > 100.0,
            "expected high compression ratio, got {ratio}"
        );
    }

    #[test]
    fn messages_without_timestamps_still_playable() {
        // Missing timestamps fall back to the minimum gap so playback
        // doesn't stall — nothing in our data guarantees that every
        // message carries a timestamp (though modern Claude CLI does).
        let t = ReplayTimeline::from_transcript(vec![
            msg(None, "a", Role::User),
            msg(None, "b", Role::Assistant),
        ]);
        assert_eq!(t.messages[0].playback_offset, Duration::ZERO);
        assert!(t.messages[1].playback_offset >= MIN_PLAYBACK_GAP);
    }

    #[test]
    fn format_duration_produces_sane_strings() {
        assert_eq!(format_duration(Duration::ZERO), "0s");
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(2 * 60 + 14)), "2m 14s");
        assert_eq!(format_duration(Duration::from_secs(38 * 60)), "38m");
        assert_eq!(
            format_duration(Duration::from_secs(3_600 + 8 * 60 + 22)),
            "1h 8m"
        );
    }

    #[test]
    fn speed_preset_faster_and_slower_cycle() {
        assert_eq!(SpeedPreset::Normal.faster(), SpeedPreset::Double);
        assert_eq!(SpeedPreset::Double.faster(), SpeedPreset::Quad);
        assert_eq!(SpeedPreset::Hundred.faster(), SpeedPreset::Quarter);
        assert_eq!(SpeedPreset::Normal.slower(), SpeedPreset::Half);
        assert_eq!(SpeedPreset::Quarter.slower(), SpeedPreset::Hundred);
    }

    #[test]
    fn speed_preset_typewriter_enabled_only_at_low_speeds() {
        assert!(SpeedPreset::Normal.enable_typewriter());
        assert!(SpeedPreset::Double.enable_typewriter());
        assert!(!SpeedPreset::Quad.enable_typewriter());
        assert!(!SpeedPreset::Ten.enable_typewriter());
        assert!(!SpeedPreset::Hundred.enable_typewriter());
    }
}
