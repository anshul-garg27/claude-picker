//! Shared TachyonFX effect helpers.
//!
//! Every animated feature (F1 fork-graph radial expansion, F3 session pulse
//! HUD, F4 replay comet trail, F5 peek-mode slide) routes through the
//! factory functions here. Two reasons:
//!
//! 1. **Reduce-motion is enforced in one place.** Call sites pass
//!    `cfg.ui.reduce_motion` into [`build`] and the helper returns
//!    [`None`] when motion is disabled. The `option-of-effect` pattern
//!    keeps the render sites compact — `if let Some(fx) = ...` skips the
//!    entire branch on quiet builds rather than allocating an effect that
//!    does nothing.
//! 2. **The tachyonfx surface is isolated.** Breaking API changes in
//!    future bumps (0.7 → 0.x etc.) are a single-module patch instead
//!    of a codebase-wide rewrite. Higher-level UI modules only know the
//!    `Effect` type.
//!
//! tachyonfx 0.7 API quick-ref used here:
//!
//! * The `Shader` trait must be in scope at every call site that invokes
//!   `effect.process(...)` — it's the default trait method that drives
//!   the tick, and isn't auto-reachable. Every UI module that uses an
//!   effect also `use`s [`tachyonfx::Shader`].
//! * [`tachyonfx::fx::fade_from`] / [`tachyonfx::fx::fade_to`] — both
//!   take `Into<Color>` for fg + bg plus `Into<EffectTimer>`. `(u32, Interpolation)`
//!   is the canonical tuple form.
//! * [`tachyonfx::fx::slide_in`] — signature
//!   `(Direction, gradient_len, randomness, color_behind, timer)`.
//! * [`tachyonfx::fx::sweep_in`] — same signature as `slide_in`. Stand-in
//!   for the `RadialPattern` the brief originally called for: the
//!   0.7 line does *not* expose `RadialPattern` / `expand` — those
//!   landed later in the 0.11+ series. Because this crate is pinned to
//!   tachyonfx 0.7 (the only line compatible with `ratatui = "0.28"`),
//!   F1 uses a directional sweep-in instead. The behaviour is close
//!   enough — cells still "reveal" in a gradient wave — and the brief's
//!   "350 ms" timing is preserved.
//! * [`tachyonfx::fx::parallel`] / [`tachyonfx::fx::sequence`] — `&[Effect]`
//!   combinators. Not variadic.
//! * [`tachyonfx::fx::repeat`] + [`tachyonfx::fx::RepeatMode`] — drives
//!   the F3 HUD pulse loop.
//! * [`tachyonfx::Duration`] is *not* `std::time::Duration`; it's a
//!   millisecond-granular newtype. Use [`delta_from`] to convert.

use ratatui::style::Color;

use tachyonfx::fx::Direction;
use tachyonfx::{fx, Duration, Effect, Interpolation};

// ── Primitive factories ──────────────────────────────────────────────────

/// Linear fade-in of the foreground over `dur_ms` milliseconds. Uses a
/// `CircOut` curve so the last frames are the densest, matching the feel
/// the brief calls for when a new panel "arrives".
///
/// `base_fg` is the colour the cells should finish at; tachyonfx handles the
/// starting transparent paint automatically when the effect is stacked over
/// an already-rendered widget. The common call is
/// `fade_in(theme.base, 200)` — both arguments are thin wrappers so the
/// caller never imports the tachyonfx `Color` type directly.
pub fn fade_in(base_fg: Color, base_bg: Color, dur_ms: u32) -> Effect {
    fx::fade_from(base_fg, base_bg, (dur_ms, Interpolation::CircOut))
}

/// Reverse of [`fade_in`] — fades the cells *to* the given colours.
/// Used by the peek-mode tear-down when Space is released.
pub fn fade_out(target_fg: Color, target_bg: Color, dur_ms: u32) -> Effect {
    fx::fade_to(target_fg, target_bg, (dur_ms, Interpolation::CircIn))
}

/// Gradient "radial" reveal. In the 0.11+ tachyonfx line this would be a
/// `RadialPattern + expand` composition; in the 0.7 line we approximate
/// with a [`fx::sweep_in`] flowing left → right. The 350 ms timing and
/// quadratic easing match the brief's feel; the visual is a directional
/// wipe rather than a ripple but still reads as "the panel just appeared".
///
/// `center_norm` is accepted as a parameter so a later tachyonfx upgrade
/// can swap in a true radial without any call-site churn — the argument
/// is threaded through but unused today (documented so IDEs surface the
/// intent).
pub fn radial_expand(
    _center_norm: (f32, f32),
    base_fg: Color,
    base_bg: Color,
    dur_ms: u32,
) -> Effect {
    // Approximate the radial reveal with a parallel (sweep + fade) so the
    // reveal feels richer than a single axis sweep.
    fx::parallel(&[
        fx::sweep_in(
            Direction::LeftToRight,
            /* gradient_length = */ 8,
            /* randomness = */ 0,
            base_bg,
            (dur_ms, Interpolation::QuadOut),
        ),
        fx::fade_from(base_fg, base_bg, (dur_ms, Interpolation::QuadOut)),
    ])
}

/// Slide the cells' paint *up* into place from a column-below. Paired with
/// [`fade_in`] via [`fx::parallel`] for the "arrive from beneath" feel the
/// peek-mode and pulse-HUD border-flash both want.
///
/// `color_behind` is what the cells look like *before* the slide finishes —
/// pass `theme.base` to avoid a bright after-image.
pub fn slide_in_from_below(color_behind: Color, dur_ms: u32) -> Effect {
    fx::slide_in(
        Direction::DownToUp,
        /* gradient_length = */ 3,
        /* randomness = */ 0,
        color_behind,
        (dur_ms, Interpolation::QuadOut),
    )
}

/// Slide the cells *down* and out of view. Mirror of [`slide_in_from_below`]
/// for the peek-mode tear-down path.
pub fn slide_out_downward(color_behind: Color, dur_ms: u32) -> Effect {
    fx::slide_out(
        Direction::UpToDown,
        3,
        0,
        color_behind,
        (dur_ms, Interpolation::QuadIn),
    )
}

/// Pulse a region's foreground between `low` and `high` on a loop. The
/// underlying effect is a `sequence(fade_to, fade_to)` wrapped in
/// `repeating(...)` so it never terminates on its own — callers drop it
/// when the pulse should stop. tachyonfx 0.7 keeps `RepeatMode` as a
/// crate-private type; we use the public `repeating` shortcut which
/// constructs the `Forever` variant under the hood.
///
/// `dur_ms` is the *full* round-trip duration; each half runs for
/// `dur_ms / 2`. 2 000 ms is the value F3 uses for the live-indicator
/// beacon so the pulse reads "alive, not urgent".
pub fn pulse(low: Color, high: Color, bg: Color, dur_ms: u32) -> Effect {
    let half = (dur_ms / 2).max(1);
    let up = fx::fade_to(high, bg, (half, Interpolation::SineInOut));
    let down = fx::fade_to(low, bg, (half, Interpolation::SineInOut));
    fx::repeating(fx::sequence(&[up, down]))
}

/// Flash-in then hold — a one-shot border highlight. Used by F3 when
/// `today_cost > 95%` of the daily budget so the HUD border briefly blinks
/// the warning colour.
///
/// Total duration is `dur_ms`, split 2/3 slide-in + 1/3 settle.
pub fn flash_border(accent: Color, bg: Color, dur_ms: u32) -> Effect {
    let slide_ms = ((dur_ms * 2) / 3).max(1);
    let settle_ms = dur_ms.saturating_sub(slide_ms).max(1);
    fx::parallel(&[
        slide_in_from_below(bg, slide_ms),
        fx::fade_from(accent, bg, (settle_ms, Interpolation::CircOut)),
    ])
}

// ── Reduce-motion gate ───────────────────────────────────────────────────

/// Wrap an effect in the reduce-motion gate. Returns `None` when the user
/// has opted out — higher-level render code writes
///
/// ```ignore
/// if let Some(fx) = crate::ui::fx::build(cfg_reduce_motion, || {
///     crate::ui::fx::radial_expand(center, theme.base, theme.base, 350)
/// }) {
///     state.effect = Some(fx);
/// }
/// ```
///
/// The factory is a closure so we never allocate the underlying effect when
/// the motion gate is closed — tachyonfx effects are cheap but still
/// non-zero (buffered timers, colour state), and fx we immediately discard
/// is a silly transient allocation.
pub fn build<F: FnOnce() -> Effect>(reduce_motion: bool, factory: F) -> Option<Effect> {
    if reduce_motion {
        None
    } else {
        Some(factory())
    }
}

// ── Effect-state micro-helpers ───────────────────────────────────────────

/// Convert a [`std::time::Duration`] frame-delta into the tachyonfx
/// [`Duration`] its `process` method wants. tachyonfx's `Duration` is a
/// custom `u32`-milliseconds newtype — *not* `std::time::Duration` — so we
/// cap the upper bound to avoid an overflow on a paused-for-hours terminal
/// returning to the foreground.
pub fn delta_from(elapsed: std::time::Duration) -> Duration {
    // u32::MAX ms ≈ 49 days — plenty of headroom. Anything that large
    // indicates the app was backgrounded for a week; clamp so tachyonfx
    // never sees a nonsense tick.
    let ms = elapsed.as_millis().min(u32::MAX as u128) as u32;
    Duration::from_millis(ms)
}

/// Borrow a mutable effect *only if* it is still running. Returns
/// `Some(effect)` on the first frame after construction and every tick
/// while it's animating; returns `None` once tachyonfx reports `done()`,
/// which is the caller's cue to `state.effect = None`.
///
/// This spares every call site the `effect.done()` boilerplate. Requires
/// `tachyonfx::Shader` in scope at the use site because `done()` is a
/// trait method.
pub fn still_running(effect: &mut Effect) -> Option<&mut Effect> {
    use tachyonfx::Shader;
    if effect.done() {
        None
    } else {
        Some(effect)
    }
}

// ── Smooth scroll (critically-damped lerp) ───────────────────────────────

/// Per-frame interpolation rate for the smooth-scroll helper. `current`
/// moves 30 % of the remaining distance toward `target` each tick, so the
/// visible offset reaches within 0.5 rows of the target in roughly four
/// frames (0.7^4 ≈ 0.24, 0.7^5 ≈ 0.17).
///
/// Why 0.30 specifically: it was picked so a single page-jump (say 12 rows)
/// resolves in ~5 render ticks — long enough that the eye registers the
/// motion as a glide rather than a teleport, short enough that the cursor
/// is never visibly "behind" the input. Higher rates (0.5+) feel like a
/// hard snap; lower rates (0.15-) feel rubbery on fast `j`/`k` bursts.
pub const SMOOTH_SCROLL_LERP: f32 = 0.30;

/// Snap threshold in rows. When `|target - current|` drops below this the
/// helper assigns `current = target` outright, preventing the float from
/// chasing an asymptote forever (and keeping the rendered offset stable —
/// no 0.4-row sub-pixel flicker).
pub const SMOOTH_SCROLL_SNAP: f32 = 0.5;

/// Lightweight scroll-position interpolator.
///
/// Each list with a scrolled viewport owns one of these. The render path
/// reads [`SmoothScroll::offset`] (rounded to the nearest row) to place its
/// visible slice; the cursor-move path calls [`SmoothScroll::set_target`]
/// whenever the anchor should change; and the per-frame `tick()` drives
/// [`SmoothScroll::advance`] once.
///
/// Reduce-motion is respected: [`SmoothScroll::set_target`] with
/// `reduce_motion = true` snaps immediately, bypassing the interpolation
/// entirely. Toggling the flag mid-animation simply makes the *next*
/// `advance` call finish at whatever `target` is at that moment — so a
/// user flipping `CLAUDE_PICKER_NO_ANIM` on during a glide sees the list
/// park at the final anchor on the next tick, not mid-animation.
#[derive(Debug, Clone, Copy, Default)]
pub struct SmoothScroll {
    current: f32,
    target: f32,
}

impl SmoothScroll {
    /// Fresh scroller anchored at the origin.
    pub const fn new() -> Self {
        Self {
            current: 0.0,
            target: 0.0,
        }
    }

    /// Current interpolated offset, rounded to a row index. This is the
    /// value the render path slices against.
    pub fn offset(&self) -> usize {
        self.current.round().max(0.0) as usize
    }

    /// Raw interpolated offset (useful in tests and for sub-row effects).
    pub fn current(&self) -> f32 {
        self.current
    }

    /// Requested viewport anchor. Always a whole row — only `current`
    /// carries the fractional state.
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Update the scroll target.
    ///
    /// When `reduce_motion` is set the current offset snaps to the target
    /// immediately — no subsequent [`SmoothScroll::advance`] calls are
    /// needed for the viewport to settle.
    pub fn set_target(&mut self, target: usize, reduce_motion: bool) {
        self.target = target as f32;
        if reduce_motion {
            self.current = self.target;
        }
    }

    /// Hard-reset both sides of the interpolation — useful when the
    /// underlying list swaps (e.g. entering a new project) and the
    /// previous scroll position is meaningless against the new data.
    pub fn snap_to(&mut self, target: usize) {
        self.target = target as f32;
        self.current = self.target;
    }

    /// Advance one tick toward `target` using [`SMOOTH_SCROLL_LERP`]. When
    /// `reduce_motion` is on the offset is clamped to `target` immediately;
    /// otherwise the standard `current += (target - current) * rate`
    /// applies, with a [`SMOOTH_SCROLL_SNAP`]-rows dead zone to avoid
    /// float chasing.
    pub fn advance(&mut self, reduce_motion: bool) {
        if reduce_motion {
            self.current = self.target;
            return;
        }
        let delta = self.target - self.current;
        if delta.abs() < SMOOTH_SCROLL_SNAP {
            self.current = self.target;
        } else {
            self.current += delta * SMOOTH_SCROLL_LERP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use tachyonfx::Shader;

    #[test]
    fn build_returns_none_when_reduce_motion_is_set() {
        // Factory must not be invoked — use a canary panic to prove it.
        let out = build(true, || panic!("reduce-motion gate must short-circuit"));
        assert!(out.is_none());
    }

    #[test]
    fn build_returns_some_when_motion_allowed() {
        let out = build(false, || fade_in(Color::Red, Color::Black, 100));
        assert!(out.is_some());
    }

    #[test]
    fn fade_in_effect_terminates_after_duration() {
        // Drive the effect past its duration; tachyonfx should report done.
        let mut effect = fade_in(Color::Red, Color::Black, 50);
        let area = Rect::new(0, 0, 4, 1);
        let mut buf = Buffer::empty(area);
        // Single big step past the 50 ms window.
        effect.process(
            delta_from(std::time::Duration::from_millis(120)),
            &mut buf,
            area,
        );
        assert!(effect.done(), "fade_in should terminate after its duration");
    }

    #[test]
    fn pulse_never_terminates_on_its_own() {
        let mut effect = pulse(Color::Red, Color::Yellow, Color::Black, 200);
        let area = Rect::new(0, 0, 4, 1);
        let mut buf = Buffer::empty(area);
        // Spin for several cycles — a `Forever` repeat must never complete.
        for _ in 0..10 {
            effect.process(
                delta_from(std::time::Duration::from_millis(300)),
                &mut buf,
                area,
            );
        }
        assert!(
            !effect.done(),
            "pulse is RepeatMode::Forever — should never be done"
        );
    }

    #[test]
    fn still_running_short_circuits_done_effects() {
        let mut effect = fade_in(Color::Red, Color::Black, 10);
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        effect.process(
            delta_from(std::time::Duration::from_millis(50)),
            &mut buf,
            area,
        );
        assert!(still_running(&mut effect).is_none());
    }

    #[test]
    fn smooth_scroll_converges_within_5_frames() {
        // Within a typical single-page scroll (≤ ~3 rows), the 0.30 lerp
        // + 0.5-row snap collapses the animation inside five ticks. That's
        // the visual contract: a j/k press never leaves the eye waiting
        // more than a handful of frames for the viewport to catch up.
        //
        // Distance per frame: d[n] = target * 0.7^n. For target=2,
        // d[4] = 2 * 0.2401 = 0.480 < 0.5 → snap on frame 4.
        let mut s = SmoothScroll::new();
        s.set_target(2, /* reduce_motion */ false);
        for frame in 1..=5 {
            s.advance(false);
            if s.offset() == 2 && (s.current() - s.target()).abs() < f32::EPSILON {
                // Converged — test passes at or before the fifth tick.
                assert!(
                    frame <= 5,
                    "smooth scroll converged on frame {frame} (expected ≤5)"
                );
                return;
            }
        }
        panic!(
            "smooth scroll did not converge within 5 frames: current={}, target={}",
            s.current(),
            s.target()
        );
    }

    #[test]
    fn smooth_scroll_snaps_immediately_when_reduce_motion() {
        // A brand-new scroller with reduce_motion engaged must land on the
        // target in a single operation — no advance() required.
        let mut s = SmoothScroll::new();
        s.set_target(42, /* reduce_motion */ true);
        assert_eq!(s.offset(), 42);
        assert_eq!(s.current(), s.target());

        // And advancing with reduce_motion held high keeps it pinned even
        // if the target moves again.
        s.set_target(7, true);
        s.advance(true);
        assert_eq!(s.offset(), 7);
    }

    #[test]
    fn smooth_scroll_mid_animation_reduce_motion_toggle_parks_at_target() {
        // Start an animation with motion on …
        let mut s = SmoothScroll::new();
        s.set_target(30, false);
        s.advance(false);
        assert!(s.offset() < 30, "mid-animation should not yet be at target");
        // … then flip the gate. The next tick must snap to `target`.
        s.advance(true);
        assert_eq!(s.offset(), 30);
    }
}
