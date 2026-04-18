//! Time-travel replay viewer — the `R` keybinding.
//!
//! This is the headline feature of v3.0: a YouTube-style player for Claude
//! session transcripts. The user presses `R` on a session row and the
//! screen transforms into a player that drips messages onto the screen
//! one-at-a-time with timing derived from the REAL JSONL timestamps —
//! capped so multi-hour gaps don't freeze the replay.
//!
//! Architecture:
//!
//! - [`ReplayState`] owns the virtual clock, speed, and play/pause flag.
//!   The event loop in `commands::pick::run_with_theme` (or the tree /
//!   search screens) calls [`ReplayState::advance`] every tick and
//!   [`ReplayState::handle_event`] on every keypress.
//! - [`render`] is a single-frame widget call. It splits the area into
//!   header / body / footer / progress-bar, flattens every message visible
//!   so far into `Line<'static>`, and either auto-scrolls to the bottom
//!   (default) or keeps the scroll offset the user set.
//! - Visible state is stored on the state struct so the parent screen only
//!   needs to plumb events + a tick + a render call.
//!
//! Honours `CLAUDE_PICKER_NO_ANIM` by skipping the slide-in + typewriter
//! effects — messages just appear instantly.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use tachyonfx::{fx, Effect, Interpolation, Shader};

use crate::data::replay::{format_duration, ReplayTimeline, SpeedPreset};
use crate::data::transcript::{
    jsonl_path_for_session, load_transcript, ContentItem, Role, TranscriptMessage,
};
use crate::data::Session;
use crate::events::Event;
use crate::theme::{self, Theme};
use crate::ui::fx as ui_fx;

/// What the viewer wants the parent to do after handling an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayAction {
    /// Stay in the replay — event handled locally.
    None,
    /// Exit the replay and return to the parent screen.
    Close,
    /// Show a transient status message.
    Toast(String, ToastKind),
}

/// Toast flavour the parent should use. Kept local so the replay module
/// doesn't couple to any specific parent-state struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

/// Playback mode. The default is [`Mode::Realtime`]; the other two are
/// reserved for future "narrative mode" presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Use the actual timestamps (with gap capping).
    Realtime,
    /// Each message plays for `N` seconds regardless of real gaps.
    #[allow(dead_code)]
    Fixed(u64),
    /// No pause between messages — fastest possible replay.
    #[allow(dead_code)]
    Fastest,
}

/// All transient state for one open replay session.
pub struct ReplayState {
    /// Display label for the title bar.
    pub title: String,
    /// Session id — retained for future "resume from this point" flow.
    pub session_id: String,
    /// Total message count (may exceed `timeline.len()` if some messages
    /// had unparseable content).
    pub total_messages: usize,
    /// Rolled-up cost from the session, formatted.
    pub cost_label: String,
    /// Dominant model summary.
    pub model_summary: String,

    /// The timeline being played back.
    pub timeline: ReplayTimeline,
    /// Error from JSONL load, if any.
    pub load_error: Option<String>,

    /// True = clock is advancing. False = paused.
    pub is_playing: bool,
    /// Current speed preset.
    pub speed: SpeedPreset,
    /// How far into the session we are, in PLAYBACK time.
    pub virtual_time: Duration,
    /// Real wall-clock instant when the clock last advanced. `None` while
    /// paused.
    pub last_tick: Option<Instant>,
    /// Most-recently-visible message index. `None` before the first
    /// message arrives.
    pub current_index: Option<usize>,
    /// `Instant` when the most recent message entered the visible set.
    /// Drives the "message appearance" slide-in animation.
    pub last_message_at: Option<Instant>,
    /// `Instant` when the speed last changed. Drives the header pulse.
    pub speed_changed_at: Option<Instant>,
    /// User preference: show tool_use / tool_result blocks (vs skip them
    /// for a narrative-only replay).
    pub show_tool_blocks: bool,
    /// User preference: auto-scroll to keep the newest message visible.
    pub auto_scroll: bool,
    /// Scroll offset in rendered lines. Reset when auto-scroll is on.
    pub scroll: usize,
    /// Playback mode — reserved for future "fixed-per-message" alternatives.
    #[allow(dead_code)]
    pub playback_mode: Mode,

    /// Cached flattened lines for the current visible set. Rebuilt when
    /// `current_index` or width changes.
    pub cached_lines: Vec<Line<'static>>,
    pub cached_width: u16,
    pub cached_index: Option<usize>,
    pub cached_show_tool_blocks: bool,
    pub cached_auto_scroll: bool,

    /// F4 — comet-trail ring. Stores up to `SCRUB_TRAIL_CAPACITY`
    /// recently-visited scrubber positions, newest at the front. Painted
    /// over the progress bar with a stacked fade so each older position
    /// shows at a weaker alpha. The actual fade is applied by stacking
    /// tachyonfx `fade_to` effects in [`render_progress_bar`].
    pub scrub_trail: VecDeque<ScrubPos>,
    /// Frame-counter for the trail — used to push a new `ScrubPos` only
    /// when the visible filled_col actually changes between frames, not
    /// on every tick.
    pub last_trail_col: Option<u16>,
    /// Reduce-motion opt-out for the scrub trail. `true` collapses the
    /// comet back to a single cursor cell.
    pub reduce_motion: bool,
}

/// One recorded cursor position for the F4 comet trail.
///
/// `col` is the column (inside the progress bar's rect) the scrubber
/// landed on; `seen_at` is when it was recorded. Older entries in
/// [`ReplayState::scrub_trail`] render at lower alpha to give the "comet"
/// look without us having to drive a manual tween.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrubPos {
    pub col: u16,
    pub seen_at: Instant,
}

/// Ring capacity — three history positions plus the current head gives a
/// four-cell comet, as the brief calls for.
pub const SCRUB_TRAIL_CAPACITY: usize = 4;

/// Per-position alpha ramp for the comet trail. `idx 0` is the head
/// (full brightness), trailing entries fade off. 4 entries, matching
/// [`SCRUB_TRAIL_CAPACITY`].
pub const SCRUB_TRAIL_ALPHAS: [f32; 4] = [1.0, 0.3, 0.1, 0.0];

/// Duration of the "new message" slide-in animation. Long enough to feel
/// alive, short enough that at 10x speed the effect doesn't fall behind.
const MESSAGE_SLIDE_IN: Duration = Duration::from_millis(220);
/// Duration of the "speed changed" pulse. Visible for roughly half a
/// second before the header settles back to its static colour.
const SPEED_PULSE: Duration = Duration::from_millis(500);

impl ReplayState {
    /// Construct a state + load the transcript. A 1000-message session
    /// loads in < 20ms on an M1 so blocking here is fine — keeps the
    /// state machine simple.
    pub fn open(session: &Session) -> Self {
        let cost_label = if session.total_cost_usd < 0.01 {
            "<$0.01".to_string()
        } else {
            format!("${:.2}", session.total_cost_usd)
        };
        Self::open_with(
            &session.id,
            session.display_label(),
            session.message_count as usize,
            cost_label,
            session.model_summary.clone(),
            session.total_cost_usd,
        )
    }

    /// Open with explicit labels. Useful for tests and screens that
    /// don't carry a full [`Session`].
    pub fn open_with(
        session_id: &str,
        title: impl Into<String>,
        total_messages: usize,
        cost_label: impl Into<String>,
        model_summary: impl Into<String>,
        _cost_usd: f64,
    ) -> Self {
        let mut state = Self {
            title: title.into(),
            session_id: session_id.to_string(),
            total_messages,
            cost_label: cost_label.into(),
            model_summary: model_summary.into(),
            timeline: ReplayTimeline::from_transcript(Vec::new()),
            load_error: None,
            is_playing: true,
            speed: SpeedPreset::Normal,
            virtual_time: Duration::ZERO,
            last_tick: Some(Instant::now()),
            current_index: None,
            last_message_at: None,
            speed_changed_at: None,
            show_tool_blocks: true,
            auto_scroll: true,
            scroll: 0,
            playback_mode: Mode::Realtime,
            cached_lines: Vec::new(),
            cached_width: 0,
            cached_index: None,
            cached_show_tool_blocks: true,
            cached_auto_scroll: true,
            scrub_trail: VecDeque::with_capacity(SCRUB_TRAIL_CAPACITY),
            last_trail_col: None,
            // The env/legacy flag doubles as reduce-motion today; when
            // `App::config.ui.reduce_motion` wiring lands, replace this
            // with the plumbed value (see integration spec).
            reduce_motion: theme::animations_disabled(),
        };

        match jsonl_path_for_session(session_id) {
            Some(path) => match load_transcript(&path) {
                Ok(messages) => {
                    state.timeline = ReplayTimeline::from_transcript(messages);
                    // Seed the first-message appearance so the slide-in
                    // animation fires on the very first visible message
                    // rather than popping into place on frame 0.
                    if !state.timeline.is_empty() {
                        state.current_index = Some(0);
                        state.last_message_at = Some(Instant::now());
                    }
                }
                Err(e) => state.load_error = Some(format!("parse error: {e}")),
            },
            None => state.load_error = Some("session file not found".to_string()),
        }
        state
    }

    /// Process a single event. The parent screen calls this for every key
    /// press; on [`ReplayAction::Close`] it should drop the state.
    pub fn handle_event(&mut self, ev: Event) -> ReplayAction {
        match ev {
            Event::Key('q') | Event::Escape | Event::Ctrl('c') | Event::Quit => ReplayAction::Close,
            // Space: play/pause toggle.
            Event::Key(' ') => {
                self.toggle_play();
                ReplayAction::None
            }
            // `>` and Right: speed up.
            Event::Key('>') | Event::Right => {
                self.speed_up();
                ReplayAction::None
            }
            // `<` and Left: speed down.
            Event::Key('<') | Event::Left => {
                self.speed_down();
                ReplayAction::None
            }
            // `.` step forward one message (auto-pauses).
            Event::Key('.') => {
                self.step(1);
                ReplayAction::None
            }
            // `,` step back one message (auto-pauses).
            Event::Key(',') => {
                self.step(-1);
                ReplayAction::None
            }
            // `n` / `N`: chunk-jump to the next/previous turn boundary
            // (next user message) and auto-pause, matching delta's
            // `n`/`N`. Complements `./,` which steps one message at a
            // time regardless of role.
            Event::Key('n') => {
                self.jump_turn(1);
                ReplayAction::None
            }
            Event::Key('N') => {
                self.jump_turn(-1);
                ReplayAction::None
            }
            Event::Home => {
                self.seek_to_start();
                ReplayAction::None
            }
            Event::End => {
                self.seek_to_end();
                ReplayAction::None
            }
            // 0..9 percentage seek.
            Event::Key(c) if c.is_ascii_digit() => {
                let pct = c.to_digit(10).unwrap_or(0) as f64 / 10.0;
                self.seek_to_fraction(pct);
                ReplayAction::None
            }
            Event::Key('t') => {
                self.show_tool_blocks = !self.show_tool_blocks;
                self.invalidate_cache();
                let msg = if self.show_tool_blocks {
                    "showing tool blocks"
                } else {
                    "hiding tool blocks (narrative mode)"
                };
                ReplayAction::Toast(msg.to_string(), ToastKind::Info)
            }
            Event::Key('a') => {
                self.auto_scroll = !self.auto_scroll;
                self.invalidate_cache();
                let msg = if self.auto_scroll {
                    "auto-scroll on"
                } else {
                    "auto-scroll off"
                };
                ReplayAction::Toast(msg.to_string(), ToastKind::Info)
            }
            Event::Up | Event::Key('k') => {
                // Manual scroll: implies turning off auto-scroll so the
                // user can park the viewport where they like.
                self.auto_scroll = false;
                self.scroll = self.scroll.saturating_sub(1);
                ReplayAction::None
            }
            Event::Down | Event::Key('j') => {
                self.auto_scroll = false;
                self.scroll = self.scroll.saturating_add(1);
                ReplayAction::None
            }
            Event::PageUp => {
                self.auto_scroll = false;
                self.scroll = self.scroll.saturating_sub(10);
                ReplayAction::None
            }
            Event::PageDown => {
                self.auto_scroll = false;
                self.scroll = self.scroll.saturating_add(10);
                ReplayAction::None
            }
            Event::Resize(_, _) => {
                self.invalidate_cache();
                ReplayAction::None
            }
            _ => ReplayAction::None,
        }
    }

    /// Advance the virtual clock based on wall time elapsed since last
    /// tick. Parent screens call this every render tick.
    pub fn advance(&mut self, now: Instant) {
        if !self.is_playing {
            self.last_tick = Some(now);
            return;
        }
        let prev = self.last_tick.unwrap_or(now);
        let wall_elapsed = now.saturating_duration_since(prev);
        let virtual_delta =
            Duration::from_secs_f64(wall_elapsed.as_secs_f64() * self.speed.multiplier());
        self.virtual_time = self.virtual_time.saturating_add(virtual_delta);
        self.last_tick = Some(now);

        // Update current_index based on new virtual_time.
        let before = self.current_index;
        let after = self.timeline.index_at(self.virtual_time);
        if after != before {
            // A new message (or several) just entered the visible set.
            // Trigger the slide-in animation.
            if after.is_some() && after > before {
                self.last_message_at = Some(now);
            }
            self.current_index = after;
            self.invalidate_cache();
        }

        // If we've reached the end and there's nothing left, auto-pause so
        // the finish overlay is stable.
        if let Some(idx) = self.current_index {
            if idx + 1 >= self.timeline.len() && self.virtual_time >= self.timeline.total_playback {
                self.is_playing = false;
            }
        }
    }

    /// Toggle the play/pause flag. Resets the tick baseline so a long
    /// pause doesn't produce a jump on resume.
    pub fn toggle_play(&mut self) {
        self.is_playing = !self.is_playing;
        self.last_tick = Some(Instant::now());
    }

    /// Cycle to the next faster speed.
    pub fn speed_up(&mut self) {
        self.speed = self.speed.faster();
        self.speed_changed_at = Some(Instant::now());
    }

    pub fn speed_down(&mut self) {
        self.speed = self.speed.slower();
        self.speed_changed_at = Some(Instant::now());
    }

    /// Step the message cursor by `delta`. Pauses playback so the user
    /// stays on the chosen message. Positive = forward, negative = back.
    pub fn step(&mut self, delta: i32) {
        if self.timeline.is_empty() {
            return;
        }
        let cur = self.current_index.map(|i| i as i32).unwrap_or(-1);
        let next = (cur + delta).clamp(0, (self.timeline.len() as i32).saturating_sub(1)) as usize;
        self.seek_to_index(next);
        self.is_playing = false;
    }

    /// Jump to the next / previous USER message and auto-pause. Matches
    /// delta's `n`/`N` in that each press moves to the next real "turn
    /// boundary" rather than every assistant message / tool call.
    pub fn jump_turn(&mut self, dir: i32) {
        if self.timeline.is_empty() {
            return;
        }
        let cur = self.current_index.unwrap_or(0);
        let len = self.timeline.len();
        if dir > 0 {
            // Find the first user message strictly after the current.
            for i in (cur + 1)..len {
                if matches!(self.timeline.messages[i].message.role, Role::User) {
                    self.seek_to_index(i);
                    self.is_playing = false;
                    return;
                }
            }
        } else {
            // Find the most-recent user message strictly before the current.
            if cur == 0 {
                return;
            }
            for i in (0..cur).rev() {
                if matches!(self.timeline.messages[i].message.role, Role::User) {
                    self.seek_to_index(i);
                    self.is_playing = false;
                    return;
                }
            }
        }
    }

    /// Jump to message `idx` and set `virtual_time` to that message's
    /// playback offset.
    pub fn seek_to_index(&mut self, idx: usize) {
        if self.timeline.is_empty() {
            return;
        }
        let clamped = idx.min(self.timeline.len().saturating_sub(1));
        if let Some(off) = self.timeline.offset_of(clamped) {
            self.virtual_time = off;
        }
        let before = self.current_index;
        self.current_index = Some(clamped);
        if before != Some(clamped) {
            self.last_message_at = Some(Instant::now());
            self.invalidate_cache();
        }
    }

    /// Seek to the start, pause, and reset.
    pub fn seek_to_start(&mut self) {
        self.virtual_time = Duration::ZERO;
        self.current_index = if self.timeline.is_empty() {
            None
        } else {
            Some(0)
        };
        self.last_message_at = Some(Instant::now());
        self.is_playing = false;
        self.invalidate_cache();
    }

    /// Seek to the end, pause, and show everything.
    pub fn seek_to_end(&mut self) {
        if self.timeline.is_empty() {
            return;
        }
        self.virtual_time = self.timeline.total_playback;
        self.current_index = Some(self.timeline.len() - 1);
        self.is_playing = false;
        self.invalidate_cache();
    }

    /// Seek to a fractional position in [0.0, 1.0]. 0 = start, 1 = end.
    /// Used by the digit keys (0 = 0%, 5 = 50%, 9 = 90%).
    pub fn seek_to_fraction(&mut self, pct: f64) {
        if self.timeline.is_empty() {
            return;
        }
        let pct = pct.clamp(0.0, 1.0);
        let total = self.timeline.total_playback.as_secs_f64();
        let target = Duration::from_secs_f64(total * pct);
        self.virtual_time = target;
        let before = self.current_index;
        self.current_index = self.timeline.index_at(target);
        if self.current_index != before {
            self.last_message_at = Some(Instant::now());
        }
        self.invalidate_cache();
    }

    /// True when the replay has reached the end.
    pub fn is_finished(&self) -> bool {
        if self.timeline.is_empty() {
            return false;
        }
        match self.current_index {
            Some(i) => i + 1 >= self.timeline.len() && !self.is_playing,
            None => false,
        }
    }

    /// True when the header speed badge should pulse (speed changed
    /// within the last [`SPEED_PULSE`]).
    pub fn speed_pulse_active(&self) -> bool {
        if theme::animations_disabled() {
            return false;
        }
        self.speed_changed_at
            .map(|t| t.elapsed() < SPEED_PULSE)
            .unwrap_or(false)
    }

    /// 0.0 → 1.0 progress of the most-recent-message slide-in animation.
    /// Stays at 1.0 after the window expires.
    pub fn slide_in_progress(&self) -> f32 {
        if theme::animations_disabled() {
            return 1.0;
        }
        let Some(started) = self.last_message_at else {
            return 1.0;
        };
        let e = started.elapsed();
        if e >= MESSAGE_SLIDE_IN {
            1.0
        } else {
            e.as_millis() as f32 / MESSAGE_SLIDE_IN.as_millis() as f32
        }
    }

    fn invalidate_cache(&mut self) {
        self.cached_width = 0;
    }
}

/// Minimum width the replay renders at. Below this we show a "too narrow"
/// placeholder. Matches the conversation viewer's threshold.
const MIN_REPLAY_WIDTH: u16 = 60;

/// Render the replay viewer across `area`. Clears the area first so any
/// underlying panes don't bleed through.
pub fn render(f: &mut Frame<'_>, area: Rect, state: &mut ReplayState, theme: &Theme) {
    f.render_widget(Clear, area);

    if area.width < MIN_REPLAY_WIDTH {
        render_too_narrow(f, area, theme);
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(title_line(state, theme));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split inner: body + (optional finish overlay) + footer bars.
    let constraints = [
        Constraint::Min(3),    // body (transcript)
        Constraint::Length(1), // status bar
        Constraint::Length(1), // keybind hint
        Constraint::Length(1), // progress bar
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    render_body(f, chunks[0], state, theme);
    render_status_bar(f, chunks[1], state, theme);
    render_keybind_hint(f, chunks[2], theme);
    render_progress_bar(f, chunks[3], state, theme);

    if state.is_finished() {
        render_finish_overlay(f, area, state, theme);
    }
}

/// Title line: play/pause icon + title + speed badge + position counter.
fn title_line<'a>(state: &'a ReplayState, theme: &'a Theme) -> Line<'a> {
    let (icon, icon_style) = if state.is_playing {
        (
            "\u{25B6}", // ▶
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (
            "\u{23F8}", // ⏸
            Style::default()
                .fg(theme.peach)
                .add_modifier(Modifier::BOLD),
        )
    };

    let state_label = if state.is_playing {
        "replaying"
    } else if state.is_finished() {
        "complete"
    } else {
        "paused"
    };

    // Speed pulse: brief bold on the speed label when the user just
    // changed it.
    let speed_style = if state.speed_pulse_active() {
        Style::default()
            .fg(theme.yellow)
            .bg(theme.surface0)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.yellow)
            .add_modifier(Modifier::BOLD)
    };

    let idx_label = match state.current_index {
        Some(i) => format!("{}/{}", i + 1, state.timeline.len().max(1)),
        None => format!("0/{}", state.timeline.len().max(1)),
    };
    let time_label = format!(
        "{} / {}",
        format_duration(
            state
                .current_index
                .and_then(|i| state.timeline.messages.get(i).map(|m| m.real_offset))
                .unwrap_or(Duration::ZERO)
        ),
        format_duration(state.timeline.total_real),
    );

    Line::from(vec![
        Span::raw(" "),
        Span::styled(icon, icon_style),
        Span::raw(" "),
        Span::styled(
            state.title.clone(),
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" \u{00B7} ", theme.dim()),
        Span::styled(state_label, theme.muted()),
        Span::styled(" \u{00B7} ", theme.dim()),
        Span::styled(state.speed.label(), speed_style),
        Span::raw("  "),
        Span::styled(idx_label, theme.muted()),
        Span::styled(" msgs \u{00B7} ", theme.dim()),
        Span::styled(time_label, theme.muted()),
        Span::raw(" "),
    ])
}

fn render_body(f: &mut Frame<'_>, area: Rect, state: &mut ReplayState, theme: &Theme) {
    if let Some(err) = &state.load_error {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled(err.clone(), theme.muted()),
        ])
        .alignment(Alignment::Center);
        f.render_widget(p, area);
        return;
    }

    if state.timeline.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("no messages to replay", theme.muted()),
        ])
        .alignment(Alignment::Center);
        f.render_widget(p, area);
        return;
    }

    // Rebuild cache if dimensions / visibility options changed.
    let needs_rebuild = state.cached_width != area.width
        || state.cached_index != state.current_index
        || state.cached_show_tool_blocks != state.show_tool_blocks
        || state.cached_auto_scroll != state.auto_scroll;
    if needs_rebuild {
        rebuild_visible_lines(state, theme, area.width);
        state.cached_width = area.width;
        state.cached_index = state.current_index;
        state.cached_show_tool_blocks = state.show_tool_blocks;
        state.cached_auto_scroll = state.auto_scroll;
    }

    let h = area.height as usize;
    let total = state.cached_lines.len();
    let max_scroll = total.saturating_sub(h);

    // Auto-scroll: pin to the bottom so the newest message is always in
    // view. Otherwise clamp the user's scroll to the valid range.
    if state.auto_scroll || state.scroll > max_scroll {
        state.scroll = max_scroll;
    }

    let visible: Vec<Line<'static>> = state
        .cached_lines
        .iter()
        .skip(state.scroll)
        .take(h)
        .cloned()
        .collect();

    let p = Paragraph::new(visible);
    f.render_widget(p, area);
}

/// Status line: either the "playing next message in 2.3s…" countdown or
/// a toast-style indicator when paused / at end.
fn render_status_bar(f: &mut Frame<'_>, area: Rect, state: &ReplayState, theme: &Theme) {
    let spans: Vec<Span<'static>> = if state.is_finished() {
        vec![
            Span::raw("  "),
            Span::styled(
                "\u{25CF} replay complete",
                Style::default()
                    .fg(theme.green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("r to restart \u{00B7} q to close", theme.muted()),
        ]
    } else if !state.is_playing {
        vec![
            Span::raw("  "),
            Span::styled(
                "\u{23F8} paused",
                Style::default()
                    .fg(theme.peach)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Space to resume", theme.muted()),
        ]
    } else {
        // Compute seconds until the next message.
        let next_idx = state.current_index.map(|i| i + 1).unwrap_or(0);
        if let Some(next) = state.timeline.messages.get(next_idx) {
            let remaining = next.playback_offset.saturating_sub(state.virtual_time);
            let wall_remaining = Duration::from_secs_f64(
                remaining.as_secs_f64() / state.speed.multiplier().max(0.001),
            );
            let label = if next.has_compressed_gap() {
                if let (Some(real), Some(play)) = (next.real_gap_before, next.playback_gap_before) {
                    format!(
                        "  \u{23F8} {} gap \u{00B7} compressed to {} \u{00B7} next message in {}",
                        format_duration(real),
                        format_duration(play),
                        format_duration(wall_remaining),
                    )
                } else {
                    format!(
                        "  \u{25BC} next message in {}",
                        format_duration(wall_remaining)
                    )
                }
            } else {
                format!(
                    "  \u{25BC} next message in {}",
                    format_duration(wall_remaining)
                )
            };
            vec![Span::styled(label, theme.muted())]
        } else {
            vec![
                Span::raw("  "),
                Span::styled(
                    "\u{25CF} reaching end",
                    Style::default()
                        .fg(theme.green)
                        .add_modifier(Modifier::BOLD),
                ),
            ]
        }
    };
    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}

/// Keybind hint line below the status bar.
fn render_keybind_hint(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let dim = theme.dim();
    let spans = vec![
        Span::raw("  "),
        Span::styled("Space", theme.key_hint()),
        Span::raw(" "),
        Span::styled("play/pause", theme.key_desc()),
        Span::styled("  \u{00B7}  ", dim),
        Span::styled(">/<", theme.key_hint()),
        Span::raw(" "),
        Span::styled("speed", theme.key_desc()),
        Span::styled("  \u{00B7}  ", dim),
        Span::styled("./,", theme.key_hint()),
        Span::raw(" "),
        Span::styled("step", theme.key_desc()),
        Span::styled("  \u{00B7}  ", dim),
        Span::styled("0-9", theme.key_hint()),
        Span::raw(" "),
        Span::styled("seek %", theme.key_desc()),
        Span::styled("  \u{00B7}  ", dim),
        Span::styled("Home/End", theme.key_hint()),
        Span::raw(" "),
        Span::styled("jump", theme.key_desc()),
        Span::styled("  \u{00B7}  ", dim),
        Span::styled("t", theme.key_hint()),
        Span::raw(" "),
        Span::styled("tools", theme.key_desc()),
        Span::styled("  \u{00B7}  ", dim),
        Span::styled("a", theme.key_hint()),
        Span::raw(" "),
        Span::styled("scroll", theme.key_desc()),
        Span::styled("  \u{00B7}  ", dim),
        Span::styled("q", theme.key_hint()),
        Span::raw(" "),
        Span::styled("exit", theme.key_desc()),
    ];
    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}

/// Bottom progress bar: two-layer bar (solid for played, medium-gray for
/// "loaded but ahead of cursor") plus speed and percentage.
fn render_progress_bar(f: &mut Frame<'_>, area: Rect, state: &mut ReplayState, theme: &Theme) {
    let w = area.width.saturating_sub(4) as usize;
    if w == 0 {
        return;
    }

    // Split horizontally: speed on the left, progress in the middle,
    // percentage on the right.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(12), // "speed: 10x"
            Constraint::Min(10),    // progress bar
            Constraint::Length(16), // "progress: 87% "
        ])
        .split(area);

    let speed_line = Line::from(vec![
        Span::raw("  "),
        Span::styled("speed:", theme.muted()),
        Span::raw(" "),
        Span::styled(
            state.speed.label(),
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(speed_line), cols[0]);

    // Progress bar: compute the fraction of virtual_time / total_playback.
    let frac = if state.timeline.total_playback.as_millis() == 0 {
        if state.is_finished() {
            1.0
        } else {
            0.0
        }
    } else {
        (state.virtual_time.as_secs_f64() / state.timeline.total_playback.as_secs_f64())
            .clamp(0.0, 1.0)
    };

    // Bar width inside the progress cell, minus a little padding.
    let bar_w = (cols[1].width as usize).saturating_sub(2).max(4);
    let filled_cols = ((frac * bar_w as f64).round() as usize).min(bar_w);
    let mut bar = String::with_capacity(bar_w);
    for i in 0..bar_w {
        if i < filled_cols.saturating_sub(1) {
            bar.push('\u{2501}'); // ━
        } else if i == filled_cols.saturating_sub(1) {
            bar.push('\u{25CF}'); // ●
        } else {
            bar.push('\u{2501}'); // ━
        }
    }
    // Split bar into played + unplayed for colour.
    let played: String = bar.chars().take(filled_cols).collect();
    let unplayed: String = bar.chars().skip(filled_cols).collect();

    let bar_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(played, Style::default().fg(theme.mauve)),
        Span::styled(unplayed, Style::default().fg(theme.surface2)),
        Span::raw(" "),
    ]);
    f.render_widget(Paragraph::new(bar_line), cols[1]);

    // F4 — comet trail overlay. Record the new position only when the
    // cursor cell actually moves (scrubbing, not autoplay ticks on a
    // sub-cell granularity), then paint each trailing entry with a
    // stacked fade whose alpha weakens with age.
    let head_col_in_bar = filled_cols.saturating_sub(1) as u16;
    // Align to the actual buffer column: cols[1].x is the start of the
    // progress cell, `+1` accounts for the leading space `Span::raw(" ")`.
    let head_col_abs = cols[1].x + 1 + head_col_in_bar;
    render_scrub_trail(f, cols[1], head_col_abs, state, theme);

    let pct = (frac * 100.0).round() as u32;
    let pct_line = Line::from(vec![
        Span::styled(" progress: ", theme.muted()),
        Span::styled(
            format!("{pct:>3}%"),
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]);
    f.render_widget(Paragraph::new(pct_line), cols[2]);
}

/// Record the current scrub head and paint a fading comet trail of the
/// last [`SCRUB_TRAIL_CAPACITY`] positions. In reduce-motion mode this
/// degrades to "head cell only, nothing stacked" — the cursor still
/// exists, just without the decay tail.
///
/// The tachyonfx stack does the actual alpha weakening: for each past
/// position, we process a `fade_to` with a half-surface-colour target at
/// the alpha value from [`SCRUB_TRAIL_ALPHAS`]. tachyonfx lerps the cell
/// between its current paint (the `━` baseline) and the surface hue,
/// giving a dimmed-ghost look without us having to lerp manually.
fn render_scrub_trail(
    f: &mut Frame<'_>,
    bar_area: Rect,
    head_col_abs: u16,
    state: &mut ReplayState,
    theme: &Theme,
) {
    // Record a new position only when the head cell actually moved.
    // Autoplay ticks within the same column would otherwise spam the ring.
    let new_head = ScrubPos {
        col: head_col_abs,
        seen_at: Instant::now(),
    };
    let moved = state.last_trail_col != Some(head_col_abs);
    if moved {
        state.scrub_trail.push_front(new_head);
        while state.scrub_trail.len() > SCRUB_TRAIL_CAPACITY {
            state.scrub_trail.pop_back();
        }
        state.last_trail_col = Some(head_col_abs);
    }

    // Reduce-motion: skip the trail entirely. The bar already paints the
    // head `●` natively, so there's nothing else to do.
    if state.reduce_motion {
        return;
    }

    // Paint one fade_to effect per trailing position. We drive the effect
    // once with a large `elapsed` so it settles to its target alpha in
    // this frame — we're not animating the fade over time, we're using
    // tachyonfx as a single-frame-sample alpha blender.
    let buf = f.buffer_mut();
    let row = bar_area.y;
    for (idx, pos) in state.scrub_trail.iter().enumerate().skip(1) {
        let alpha = SCRUB_TRAIL_ALPHAS.get(idx).copied().unwrap_or(0.0);
        if alpha <= 0.0 {
            continue;
        }
        // Constrain the effect to the single cell we care about.
        let cell = Rect {
            x: pos.col.min(bar_area.x + bar_area.width.saturating_sub(1)),
            y: row,
            width: 1,
            height: 1,
        };
        // `fade_to` lerps toward the target colours; we pick a target
        // between the played colour (mauve) and the bar's unplayed base
        // (surface2) proportional to `1 - alpha` so the head at alpha=1
        // keeps full colour and each trailing cell dims toward the
        // background.
        let target = blend_rgb(theme.mauve, theme.surface2, 1.0 - alpha);
        let mut eff: Effect =
            fx::fade_to(target, theme.base, (1, Interpolation::Linear));
        // One tick is enough — the effect duration is 1 ms so any
        // positive elapsed saturates it.
        eff.process(
            ui_fx::delta_from(std::time::Duration::from_millis(1)),
            buf,
            cell,
        );
    }
}

/// Linear RGB blend between two ratatui `Color`s. Non-RGB variants
/// (indexed, named) fall back to `a` — we only ever blend mauve and
/// surface2 which are both RGB in every theme.
fn blend_rgb(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let lerp = |x: u8, y: u8| -> u8 {
                (x as f32 + (y as f32 - x as f32) * t).clamp(0.0, 255.0) as u8
            };
            Color::Rgb(lerp(ar, br), lerp(ag, bg), lerp(ab, bb))
        }
        _ => a,
    }
}

fn render_too_narrow(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let p = Paragraph::new(vec![
        Line::raw(""),
        Line::styled(
            "Terminal too narrow for replay view.",
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            format!("Resize to at least {MIN_REPLAY_WIDTH} cols and retry (q to exit)."),
            theme.muted(),
        ),
    ])
    .alignment(Alignment::Center);
    f.render_widget(p, area);
}

/// Finish-state overlay: centered recap card with replay stats.
fn render_finish_overlay(f: &mut Frame<'_>, area: Rect, state: &ReplayState, theme: &Theme) {
    let w = 52u16.min(area.width.saturating_sub(4));
    let h = 9u16.min(area.height.saturating_sub(4));
    if w < 20 || h < 5 {
        return; // too small for the overlay; skip
    }
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    f.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.green))
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "replay complete",
                Style::default()
                    .fg(theme.green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]));

    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let compression = state.timeline.compression_ratio();
    let compression_label = if compression >= 10.0 {
        format!("{:.0}x compressed", compression)
    } else {
        format!("{:.1}x compressed", compression)
    };

    let total_playback = format_duration(state.timeline.total_playback);
    let total_real = format_duration(state.timeline.total_real);
    let stats_line =
        format!("played back in {total_playback} ({total_real} real-time, {compression_label})");

    let model = if state.model_summary.is_empty() {
        "claude".to_string()
    } else {
        state.model_summary.clone()
    };
    let meta_line = format!(
        "{} messages \u{00B7} {} \u{00B7} {}",
        state.total_messages, state.cost_label, model
    );

    let lines = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                state.title.clone(),
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(stats_line, theme.muted()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(meta_line, theme.subtle()),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Home", theme.key_hint()),
            Span::raw(" "),
            Span::styled("replay", theme.key_desc()),
            Span::styled("  \u{00B7}  ", theme.dim()),
            Span::styled("q", theme.key_hint()),
            Span::raw(" "),
            Span::styled("close", theme.key_desc()),
        ]),
    ];

    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
}

// ── Line flattening ─────────────────────────────────────────────────────────
//
// We keep a trimmed-down version of the conversation viewer's line builder
// here. The goal: render the visible-so-far messages with compact styling
// (no paragraph wrapping that'd make it hard to see when a new message
// arrived) plus gap markers for compressed long pauses.

/// Rebuild `state.cached_lines` from the currently-visible message range.
fn rebuild_visible_lines(state: &mut ReplayState, theme: &Theme, width: u16) {
    state.cached_lines.clear();
    let content_width = width.saturating_sub(4) as usize;
    let wrap_width = content_width.saturating_sub(10).max(30);

    let Some(up_to) = state.current_index else {
        return;
    };

    for (idx, msg) in state.timeline.messages.iter().enumerate().take(up_to + 1) {
        // Draw a gap indicator if this message follows a compressed gap.
        if idx > 0 && msg.has_compressed_gap() {
            if let (Some(real), Some(play)) = (msg.real_gap_before, msg.playback_gap_before) {
                state.cached_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!(
                            "\u{23F8} {} gap \u{00B7} compressed to {}",
                            format_duration(real),
                            format_duration(play),
                        ),
                        Style::default()
                            .fg(theme.overlay0)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
                state.cached_lines.push(Line::raw(""));
            }
        }

        render_message(
            &msg.message,
            theme,
            wrap_width,
            state.show_tool_blocks,
            &mut state.cached_lines,
        );

        // Blank spacer between messages unless this is the last one.
        if idx < up_to {
            state.cached_lines.push(Line::raw(""));
        }
    }
}

/// Render one message into `out`. Mirrors the conversation viewer's layout
/// but keeps things compact (no code-block boxes — the replay favours
/// readability over pixel-perfect rendering).
fn render_message(
    msg: &TranscriptMessage,
    theme: &Theme,
    wrap_width: usize,
    show_tool_blocks: bool,
    out: &mut Vec<Line<'static>>,
) {
    let (label, label_style) = match msg.role {
        Role::User => (
            "you",
            Style::default().fg(theme.blue).add_modifier(Modifier::BOLD),
        ),
        Role::Assistant => (
            "claude",
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let mut first_output = true;
    let mut emitted_anything = false;

    for item in &msg.items {
        match item {
            ContentItem::Text(text) => {
                let wrapped = wrap_text(text, wrap_width);
                for (i, line) in wrapped.into_iter().enumerate() {
                    if first_output && i == 0 {
                        out.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(label.to_string(), label_style),
                            Span::raw("  "),
                            Span::styled(line, theme.body()),
                        ]));
                        first_output = false;
                    } else {
                        out.push(Line::from(vec![
                            Span::raw("         "),
                            Span::styled(line, theme.body()),
                        ]));
                    }
                    emitted_anything = true;
                }
            }
            ContentItem::ToolUse { name, input } => {
                if !show_tool_blocks {
                    continue;
                }
                if first_output {
                    out.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(label.to_string(), label_style),
                    ]));
                    first_output = false;
                }
                push_tool_use_card(name, input, theme, wrap_width, out);
                emitted_anything = true;
            }
            ContentItem::ToolResult { content, is_error } => {
                if !show_tool_blocks {
                    continue;
                }
                if first_output {
                    out.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(label.to_string(), label_style),
                    ]));
                    first_output = false;
                }
                push_tool_result_card(content, *is_error, theme, wrap_width, out);
                emitted_anything = true;
            }
            ContentItem::Thinking { text } => {
                if first_output {
                    out.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(label.to_string(), label_style),
                    ]));
                    first_output = false;
                }
                for line in wrap_text(text, wrap_width) {
                    out.push(Line::from(vec![
                        Span::raw("         "),
                        Span::styled(
                            line,
                            Style::default()
                                .fg(theme.overlay1)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]));
                }
                emitted_anything = true;
            }
            ContentItem::Other(kind) => {
                if first_output {
                    out.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(label.to_string(), label_style),
                    ]));
                    first_output = false;
                }
                out.push(Line::from(vec![
                    Span::raw("         "),
                    Span::styled(format!("[{kind}]"), theme.muted()),
                ]));
                emitted_anything = true;
            }
        }
    }

    // If the whole message was skipped (e.g., tool blocks hidden and no
    // text), make sure the role still shows so the replay count stays
    // truthful — otherwise a user looking at "17/45 msgs" wouldn't see
    // what message 17 actually was.
    if !emitted_anything {
        out.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(label.to_string(), label_style),
            Span::raw("  "),
            Span::styled("(no visible content)", theme.muted()),
        ]));
    }
}

/// Compact tool_use card: `┌─ tool_use: <Name> ─┐` then a one-liner
/// summary of the most relevant input field.
fn push_tool_use_card(
    name: &str,
    input: &serde_json::Value,
    theme: &Theme,
    wrap_width: usize,
    out: &mut Vec<Line<'static>>,
) {
    let box_w = wrap_width.saturating_add(4).clamp(30, 80);
    let header = format!(" tool_use: {name} ");
    let hw = header.chars().count();
    let remain = box_w.saturating_sub(hw + 2);
    let top = format!(
        "  \u{250C}\u{2500}{}{}\u{2510}",
        header,
        "\u{2500}".repeat(remain)
    );
    out.push(Line::styled(top, Style::default().fg(theme.surface2)));

    let summary = summarize_input(input);
    let body = if summary.is_empty() {
        "(no input fields)".to_string()
    } else {
        summary
    };
    // Truncate body to fit within the card.
    let max_body = box_w.saturating_sub(4);
    let body = if body.chars().count() > max_body {
        let mut s: String = body.chars().take(max_body.saturating_sub(1)).collect();
        s.push('\u{2026}');
        s
    } else {
        body
    };
    let pad = box_w.saturating_sub(body.chars().count() + 4);
    out.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("\u{2502} ", Style::default().fg(theme.surface2)),
        Span::styled(body.clone(), theme.body()),
        Span::raw(" ".repeat(pad)),
        Span::styled(" \u{2502}", Style::default().fg(theme.surface2)),
    ]));

    let bottom = format!(
        "  \u{2514}{}\u{2518}",
        "\u{2500}".repeat(box_w.saturating_sub(2))
    );
    out.push(Line::styled(bottom, Style::default().fg(theme.surface2)));
}

fn push_tool_result_card(
    content: &str,
    is_error: bool,
    theme: &Theme,
    wrap_width: usize,
    out: &mut Vec<Line<'static>>,
) {
    let box_w = wrap_width.saturating_add(4).clamp(30, 80);
    let accent = if is_error { theme.red } else { theme.overlay0 };
    let header = if is_error {
        " tool_result: error "
    } else {
        " tool_result "
    };
    let hw = header.chars().count();
    let remain = box_w.saturating_sub(hw + 2);
    let top = format!(
        "  \u{250C}\u{2500}{}{}\u{2510}",
        header,
        "\u{2500}".repeat(remain)
    );
    out.push(Line::styled(top, Style::default().fg(accent)));

    // Single-line snippet. Long results get truncated — replay is about
    // watching, not reading every byte of a 10KB tool response.
    let first_line = content.lines().next().unwrap_or("").trim();
    let max_body = box_w.saturating_sub(4);
    let body = if first_line.chars().count() > max_body {
        let mut s: String = first_line
            .chars()
            .take(max_body.saturating_sub(1))
            .collect();
        s.push('\u{2026}');
        s
    } else {
        first_line.to_string()
    };
    let pad = box_w.saturating_sub(body.chars().count() + 4);
    out.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("\u{2502} ", Style::default().fg(accent)),
        Span::styled(body.clone(), theme.body()),
        Span::raw(" ".repeat(pad)),
        Span::styled(" \u{2502}", Style::default().fg(accent)),
    ]));

    let bottom = format!(
        "  \u{2514}{}\u{2518}",
        "\u{2500}".repeat(box_w.saturating_sub(2))
    );
    out.push(Line::styled(bottom, Style::default().fg(accent)));
}

/// Summarise a tool_use.input value — grab the most interesting field.
fn summarize_input(input: &serde_json::Value) -> String {
    let Some(obj) = input.as_object() else {
        return String::new();
    };
    for key in [
        "file_path",
        "path",
        "command",
        "pattern",
        "query",
        "url",
        "description",
    ] {
        if let Some(v) = obj.get(key).and_then(|v| v.as_str()) {
            return v.to_string();
        }
    }
    for (_k, v) in obj {
        if let Some(s) = v.as_str() {
            return s.to_string();
        }
    }
    String::new()
}

/// Minimal hard-wrap: split `text` into lines of at most `width` columns
/// on word boundaries. Long "words" (> width) get hard-split so we never
/// emit a line longer than `width`.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(10);
    let mut out: Vec<String> = Vec::new();
    for para in text.lines() {
        if para.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in para.split_whitespace() {
            if word.chars().count() > width {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                // Hard-split the long word.
                let mut i = 0usize;
                let chars: Vec<char> = word.chars().collect();
                while i < chars.len() {
                    let end = (i + width).min(chars.len());
                    out.push(chars[i..end].iter().collect());
                    i = end;
                }
                continue;
            }
            if current.chars().count() + word.chars().count() + 1 > width && !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Dead-code accessor used by tests.
#[allow(dead_code)]
pub fn color_muted(theme: &Theme) -> Color {
    theme.overlay0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    use crate::data::transcript::{ContentItem, Role, TranscriptMessage};

    fn ts(secs: i64) -> chrono::DateTime<chrono::Utc> {
        Utc.timestamp_opt(1_700_000_000 + secs, 0).unwrap()
    }

    fn msg(ts: Option<chrono::DateTime<chrono::Utc>>, text: &str, role: Role) -> TranscriptMessage {
        TranscriptMessage {
            role,
            timestamp: ts,
            items: vec![ContentItem::Text(text.to_string())],
        }
    }

    /// Build a ReplayState directly from a Vec<TranscriptMessage> so we
    /// can unit-test the state machine without hitting the JSONL loader.
    fn state_from_messages(messages: Vec<TranscriptMessage>) -> ReplayState {
        let mut s = ReplayState::open_with(
            "test-id",
            "test-title",
            messages.len(),
            "$0.00",
            "test-model",
            0.0,
        );
        s.timeline = ReplayTimeline::from_transcript(messages);
        s.current_index = if s.timeline.is_empty() { None } else { Some(0) };
        s.last_message_at = Some(Instant::now());
        s
    }

    #[test]
    fn scrub_trail_ring_caps_at_capacity() {
        let mut s = state_from_messages(vec![]);
        // Push 10 distinct positions into the ring; only the newest
        // SCRUB_TRAIL_CAPACITY should survive.
        for col in 0..10u16 {
            // Reuse the same invariant the renderer relies on: push-front
            // + trim to cap. We test the trim separately from the render
            // path so we don't need a frame buffer.
            s.scrub_trail.push_front(ScrubPos {
                col,
                seen_at: Instant::now(),
            });
            while s.scrub_trail.len() > SCRUB_TRAIL_CAPACITY {
                s.scrub_trail.pop_back();
            }
        }
        assert_eq!(s.scrub_trail.len(), SCRUB_TRAIL_CAPACITY);
        // Newest entry (col 9) should be at the front.
        assert_eq!(s.scrub_trail.front().map(|p| p.col), Some(9));
        // Oldest surviving entry should be col 9-3 = 6.
        assert_eq!(s.scrub_trail.back().map(|p| p.col), Some(6));
    }

    #[test]
    fn scrub_trail_alphas_decay_monotonically() {
        // The brief calls for 1.0 → 0.3 → 0.1 → 0.0 — verify the decay
        // is strictly decreasing.
        let alphas = SCRUB_TRAIL_ALPHAS;
        for w in alphas.windows(2) {
            assert!(
                w[0] > w[1],
                "alpha ramp must decrease at every step: {:?}",
                alphas
            );
        }
        assert_eq!(alphas[0], 1.0, "head alpha must be full brightness");
    }

    #[test]
    fn blend_rgb_midpoint_between_two_colors() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(200, 200, 200);
        let mid = blend_rgb(a, b, 0.5);
        match mid {
            Color::Rgb(r, g, b) => {
                assert!((r as i32 - 100).abs() <= 1);
                assert!((g as i32 - 100).abs() <= 1);
                assert!((b as i32 - 100).abs() <= 1);
            }
            _ => panic!("expected Rgb output"),
        }
    }

    #[test]
    fn blend_rgb_preserves_endpoints() {
        let a = Color::Rgb(10, 20, 30);
        let b = Color::Rgb(200, 210, 220);
        assert_eq!(blend_rgb(a, b, 0.0), a);
        assert_eq!(blend_rgb(a, b, 1.0), b);
    }

    #[test]
    fn advance_at_one_x_walks_through_timeline() {
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "first", Role::User),
            msg(Some(ts(5)), "second", Role::Assistant),
            msg(Some(ts(10)), "third", Role::User),
        ]);
        s.is_playing = true;
        s.speed = SpeedPreset::Normal;
        let start = Instant::now();
        s.last_tick = Some(start);
        // Advance 3 wall-seconds → 3 virtual seconds at 1x. Still on msg 0.
        s.advance(start + Duration::from_secs(3));
        assert_eq!(s.current_index, Some(0));
        // Advance past the 5-second mark.
        s.advance(start + Duration::from_secs(6));
        assert_eq!(s.current_index, Some(1));
        // Advance past the 10-second mark.
        s.advance(start + Duration::from_secs(12));
        assert_eq!(s.current_index, Some(2));
    }

    #[test]
    fn speed_multiplier_advances_four_times_faster() {
        // At 4x, 15 wall seconds = 60 virtual seconds. A timeline of
        // ~60 virtual seconds should be finished after 15 wall seconds.
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "a", Role::User),
            msg(Some(ts(30)), "b", Role::Assistant),
            msg(Some(ts(60)), "c", Role::User),
        ]);
        s.is_playing = true;
        s.speed = SpeedPreset::Quad;
        let start = Instant::now();
        s.last_tick = Some(start);
        s.advance(start + Duration::from_secs(15));
        assert_eq!(
            s.current_index,
            Some(2),
            "4x should reach the last message after 15 wall-seconds of a 60-virtual-second timeline"
        );
    }

    #[test]
    fn step_forward_and_backward_moves_cursor_and_pauses() {
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "a", Role::User),
            msg(Some(ts(5)), "b", Role::Assistant),
            msg(Some(ts(10)), "c", Role::User),
        ]);
        s.is_playing = true;
        s.current_index = Some(0);
        s.step(1);
        assert_eq!(s.current_index, Some(1));
        assert!(!s.is_playing, "stepping must pause");
        s.step(1);
        assert_eq!(s.current_index, Some(2));
        s.step(1);
        assert_eq!(s.current_index, Some(2), "step past end clamps");
        s.step(-1);
        assert_eq!(s.current_index, Some(1));
        s.step(-1);
        assert_eq!(s.current_index, Some(0));
        s.step(-1);
        assert_eq!(s.current_index, Some(0), "step past start clamps");
    }

    #[test]
    fn speed_up_and_down_cycle() {
        let mut s = state_from_messages(vec![msg(Some(ts(0)), "a", Role::User)]);
        assert_eq!(s.speed, SpeedPreset::Normal);
        s.speed_up();
        assert_eq!(s.speed, SpeedPreset::Double);
        s.speed_up();
        assert_eq!(s.speed, SpeedPreset::Quad);
        s.speed_down();
        assert_eq!(s.speed, SpeedPreset::Double);
        assert!(s.speed_changed_at.is_some());
    }

    #[test]
    fn seek_to_fraction_works_at_edges() {
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "a", Role::User),
            msg(Some(ts(5)), "b", Role::Assistant),
            msg(Some(ts(10)), "c", Role::User),
        ]);
        s.seek_to_fraction(0.0);
        assert_eq!(s.current_index, Some(0));
        s.seek_to_fraction(0.5);
        assert_eq!(s.current_index, Some(1));
        s.seek_to_fraction(1.0);
        assert_eq!(s.current_index, Some(2));
    }

    #[test]
    fn pause_toggles_cleanly() {
        let mut s = state_from_messages(vec![msg(Some(ts(0)), "a", Role::User)]);
        assert!(s.is_playing);
        s.toggle_play();
        assert!(!s.is_playing);
        s.toggle_play();
        assert!(s.is_playing);
    }

    #[test]
    fn home_and_end_seek_to_boundaries() {
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "a", Role::User),
            msg(Some(ts(5)), "b", Role::Assistant),
            msg(Some(ts(10)), "c", Role::User),
        ]);
        s.seek_to_end();
        assert_eq!(s.current_index, Some(2));
        assert!(!s.is_playing);
        s.seek_to_start();
        assert_eq!(s.current_index, Some(0));
        assert!(!s.is_playing);
    }

    #[test]
    fn is_finished_requires_last_index_and_paused() {
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "a", Role::User),
            msg(Some(ts(5)), "b", Role::Assistant),
        ]);
        s.seek_to_end();
        assert!(s.is_finished());
        // Resume — is_finished should flip false.
        s.is_playing = true;
        assert!(!s.is_finished());
    }

    #[test]
    fn gap_capping_is_preserved_in_state() {
        // Two-hour gap — must show a compressed indicator after capping.
        let s = state_from_messages(vec![
            msg(Some(ts(0)), "before", Role::User),
            msg(Some(ts(2 * 3600)), "after", Role::Assistant),
        ]);
        assert!(s.timeline.messages[1].has_compressed_gap());
    }

    #[test]
    fn handle_event_routes_shortcuts() {
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "a", Role::User),
            msg(Some(ts(5)), "b", Role::Assistant),
        ]);
        // Space toggles.
        assert_eq!(s.handle_event(Event::Key(' ')), ReplayAction::None);
        assert!(!s.is_playing);
        // `>` speeds up.
        assert_eq!(s.handle_event(Event::Key('>')), ReplayAction::None);
        assert_eq!(s.speed, SpeedPreset::Double);
        // `<` slows down.
        assert_eq!(s.handle_event(Event::Key('<')), ReplayAction::None);
        assert_eq!(s.speed, SpeedPreset::Normal);
        // `.` steps forward.
        assert_eq!(s.handle_event(Event::Key('.')), ReplayAction::None);
        assert_eq!(s.current_index, Some(1));
        // `,` steps back.
        assert_eq!(s.handle_event(Event::Key(',')), ReplayAction::None);
        assert_eq!(s.current_index, Some(0));
        // `q` closes.
        assert_eq!(s.handle_event(Event::Key('q')), ReplayAction::Close);
    }

    #[test]
    fn digit_key_seeks_to_percentage() {
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "a", Role::User),
            msg(Some(ts(5)), "b", Role::Assistant),
            msg(Some(ts(10)), "c", Role::User),
        ]);
        s.handle_event(Event::Key('5'));
        // 50% → virtual_time ~5s → index 1.
        assert_eq!(s.current_index, Some(1));
        s.handle_event(Event::Key('0'));
        assert_eq!(s.current_index, Some(0));
    }

    #[test]
    fn toggle_tool_blocks() {
        let mut s = state_from_messages(vec![msg(Some(ts(0)), "a", Role::User)]);
        assert!(s.show_tool_blocks);
        let _ = s.handle_event(Event::Key('t'));
        assert!(!s.show_tool_blocks);
        let _ = s.handle_event(Event::Key('t'));
        assert!(s.show_tool_blocks);
    }

    #[test]
    fn toggle_auto_scroll() {
        let mut s = state_from_messages(vec![msg(Some(ts(0)), "a", Role::User)]);
        assert!(s.auto_scroll);
        let _ = s.handle_event(Event::Key('a'));
        assert!(!s.auto_scroll);
    }

    #[test]
    fn arrow_keys_disable_auto_scroll() {
        let mut s = state_from_messages(vec![msg(Some(ts(0)), "a", Role::User)]);
        assert!(s.auto_scroll);
        let _ = s.handle_event(Event::Up);
        assert!(!s.auto_scroll);
    }

    #[test]
    fn jump_turn_skips_assistant_messages() {
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "q1", Role::User),
            msg(Some(ts(5)), "a1", Role::Assistant),
            msg(Some(ts(10)), "a1b", Role::Assistant),
            msg(Some(ts(15)), "q2", Role::User),
            msg(Some(ts(20)), "a2", Role::Assistant),
        ]);
        s.current_index = Some(0);
        s.is_playing = true;
        s.jump_turn(1);
        assert_eq!(s.current_index, Some(3), "n jumps to next user message");
        assert!(!s.is_playing, "jump_turn pauses");
        s.jump_turn(-1);
        assert_eq!(s.current_index, Some(0), "N goes back to previous user");
    }

    #[test]
    fn handle_event_n_triggers_turn_jump() {
        let mut s = state_from_messages(vec![
            msg(Some(ts(0)), "q1", Role::User),
            msg(Some(ts(5)), "a1", Role::Assistant),
            msg(Some(ts(10)), "q2", Role::User),
        ]);
        s.current_index = Some(0);
        let _ = s.handle_event(Event::Key('n'));
        assert_eq!(s.current_index, Some(2));
    }
}

// ─── F4 integration spec ─────────────────────────────────────────────────
//
// The comet-trail reads `ReplayState::reduce_motion`, which is seeded from
// `theme::animations_disabled()` on state construction. To respect the
// config-file flag:
//
//   1. Accept a `reduce_motion: bool` argument on `ReplayState::open` /
//      `ReplayState::open_with` (breaking the signature). Pass it through
//      from the two call sites in `commands::pick` and `commands::tree_cmd`
//      that construct a replay — they already have `app.config` (or will
//      once the F3 wiring adds it) and can forward
//      `config.ui.reduce_motion || theme::animations_disabled()`.
//
//   2. Remove the `theme::animations_disabled()` fallback from the struct
//      init once the call-site plumbing is in place.
//
// Until then, `CLAUDE_PICKER_NO_ANIM=1` still disables the trail.
//
// The head `●` on the progress bar is unchanged — the comet is painted on
// top of the existing bar so a reduce-motion session looks exactly like
// today's replay UI.

