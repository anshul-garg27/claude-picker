//! yazi-style background task drawer toggled with `w`.
//!
//! Renders as a pinned panel along the bottom of the screen (~1/3 height).
//! Each row is one task: a focus caret, a label, a Unicode progress bar, a
//! percentage, and a tiny `x` hint that signals "press x to cancel this
//! row". Selection is drawn with the theme's `selected_row` style so it
//! matches the rest of the UI.
//!
//! Inspiration: yazi's task manager (`w` key) + superfile's processes
//! panel. We deliberately keep this read-only from the widget's side —
//! cancellation is routed through [`crate::data::task_queue::TaskQueue`]
//! so the widget never has to know about producer threads.
//!
//! Responsive layout:
//! - Width >= 80 cols: full row with a 16-char progress bar and trailing `x` hint.
//! - Width <  80 cols: compact single-line with a 6-char bar, no hint glyph.
//!
//! The drawer is a pure render function + a small mutable selection state;
//! the backing data lives in [`crate::data::task_queue::TaskQueue`]. This
//! keeps `w` toggle / `j`/`k` / `x` as three independent concerns the event
//! loop can wire in isolation.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget};
use ratatui::Frame;

use crate::data::task_queue::{TaskHandle, TaskQueue, TaskState};
use crate::theme::Theme;
use crate::ui::text::display_width;

/// Width threshold at which we switch from compact one-line rows to the
/// full-width layout. Matches other responsive widgets in this crate so
/// the whole UI breathes or compresses together.
const COMPACT_WIDTH_COLS: u16 = 80;

/// Progress bar glyphs — heavy horizontal (filled) + light horizontal
/// (empty). These are BMP characters so every monospace font renders them
/// consistently, unlike the "block" block-element pair which can show as
/// different heights on some terminals.
const BAR_FILLED: char = '\u{2501}'; // ━
const BAR_EMPTY: char = '\u{2500}'; // ─

/// Focus-caret glyph. Black right-pointing triangle in BMP — no emoji
/// variation selector needed.
const CARET: char = '\u{25B8}'; // ▸

/// How many rows the drawer reserves at the bottom of the frame. Includes
/// top+bottom border (2) plus up to 6 task rows. Matches the integration
/// spec: "Main render should reserve bottom ~8 rows for the drawer when
/// task_drawer.visible."
pub const DRAWER_HEIGHT: u16 = 8;

/// Mutable UI state for the drawer.
///
/// Kept tiny on purpose: visibility toggle + a selection cursor. Everything
/// else (the task rows, progress values, labels) lives in the shared
/// [`TaskQueue`] so producers and the UI see the same data.
#[derive(Debug, Default, Clone)]
pub struct TaskDrawerState {
    /// True when the drawer is shown and consumes bottom screen rows.
    /// Toggled by [`Self::toggle`] in response to the `w` key.
    pub visible: bool,
    /// Row index (into [`TaskQueue::iter`] order) that the cancel hotkey
    /// `x` targets. Clamped to `max(0, len-1)` on render — the event
    /// handler uses [`Self::clamp`] after any mutation that might shrink
    /// the list (sweep, cancel-then-remove, etc.).
    pub selected: usize,
}

impl TaskDrawerState {
    /// Fresh hidden drawer with no selection. This is what `Default`
    /// returns — keeping an explicit constructor makes the call site in
    /// `App::new` self-documenting.
    pub fn new() -> Self {
        Self::default()
    }

    /// Flip the drawer open/closed. Does not touch `selected` so the last
    /// focused row is remembered across open/close cycles.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Move selection up one row. Saturates at 0 — no wrap-around since
    /// yazi and superfile both stop at the edges, which matches what
    /// users expect from a fixed-height panel.
    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Move selection down one row. `max` is the current task count; we
    /// clamp to `max.saturating_sub(1)` so an empty queue leaves the
    /// cursor at 0 rather than underflowing.
    pub fn move_down(&mut self, max: usize) {
        if max > 0 {
            self.selected = (self.selected + 1).min(max - 1);
        }
    }

    /// Clamp selection after the task list shrinks (e.g. after
    /// [`TaskQueue::sweep`]). Idempotent; cheap to call on every render.
    pub fn clamp(&mut self, max: usize) {
        if max == 0 {
            self.selected = 0;
        } else if self.selected >= max {
            self.selected = max - 1;
        }
    }

    /// Resolve the selection index into a task id by indexing into the
    /// current queue snapshot. Returns `None` if the drawer is hidden or
    /// the queue is empty — the event loop should treat that as "no-op for
    /// `x`" without raising an error.
    pub fn selected_id(&self, queue: &TaskQueue) -> Option<u64> {
        if !self.visible {
            return None;
        }
        queue.get_by_index(self.selected).map(|t| t.id)
    }
}

/// Entry point from the main render pipeline.
///
/// `area` should be the bottom slice reserved by the caller (see
/// [`DRAWER_HEIGHT`]). We render a `Clear` widget first so underlying
/// content — the session list, the preview — doesn't bleed through even
/// on terminals with transparent backgrounds.
pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &mut TaskDrawerState,
    queue: &TaskQueue,
    theme: &Theme,
) {
    if !state.visible || area.height == 0 || area.width == 0 {
        return;
    }
    // Keep the cursor inside the list even if the task count shrank since
    // last frame (sweep is the usual culprit).
    state.clamp(queue.len());

    frame.render_widget(Clear, area);
    frame.render_widget(DrawerWidget { state, queue, theme }, area);
}

/// Internal `Widget` impl — kept private so callers only see [`render`].
/// Using a `Widget` (vs. raw frame calls) makes the function compose with
/// `Block::inner` cleanly: the inner area is the content region after the
/// border subtracts its one-cell frame.
struct DrawerWidget<'a> {
    state: &'a TaskDrawerState,
    queue: &'a TaskQueue,
    theme: &'a Theme,
}

impl<'a> Widget for DrawerWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.theme.panel_border_active())
            .title(title_line(self.queue.active_count(), self.theme))
            .title_bottom(
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled("w", self.theme.key_hint()),
                    Span::raw(" "),
                    Span::styled("close", self.theme.key_desc()),
                    Span::raw("  "),
                    Span::styled("x", self.theme.key_hint()),
                    Span::raw(" "),
                    Span::styled("cancel focused", self.theme.key_desc()),
                    Span::raw(" "),
                ])
                .right_aligned(),
            );

        let inner = block.inner(area);
        block.render(area, buf);

        if self.queue.is_empty() {
            render_empty_state(inner, buf, self.theme);
            return;
        }

        let compact = area.width < COMPACT_WIDTH_COLS;
        let visible_rows = inner.height as usize;
        let total = self.queue.len();

        // Simple viewport: always show the focused row. Scroll the window
        // so selection sits inside [start, start+visible_rows). No smooth
        // scrolling because the drawer is short — one-row jumps are fine.
        let start = scroll_start(self.state.selected, visible_rows, total);

        let rows: Vec<Line<'static>> = self
            .queue
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_rows)
            .map(|(idx, task)| {
                render_row(
                    task,
                    idx == self.state.selected,
                    inner.width as usize,
                    compact,
                    self.theme,
                )
            })
            .collect();

        Paragraph::new(rows).render(inner, buf);
    }
}

/// Scroll the viewport so `selected` is visible in a window of
/// `visible_rows` over a list of `total` rows. Anchors the top when
/// possible (selection near the start) and the bottom otherwise, which
/// matches the behaviour of ratatui's built-in `List` widget.
fn scroll_start(selected: usize, visible_rows: usize, total: usize) -> usize {
    if visible_rows == 0 || total <= visible_rows {
        return 0;
    }
    if selected < visible_rows {
        0
    } else {
        selected + 1 - visible_rows
    }
}

/// Title line with a live task count.
///
/// Format matches the mock: `── tasks ── N active ──`. The count updates
/// on every frame so cancelling / finishing tasks is visibly reflected.
fn title_line(active: usize, theme: &Theme) -> Line<'static> {
    // "active" is invariant — "1 active" reads better than "1 actives" and
    // the count makes the plural unambiguous. Keeping a single label avoids
    // yet another tiny English-grammar branch.
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "tasks",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(format!("{active} active"), theme.key_desc()),
        Span::raw(" "),
    ])
}

/// Render the "no background work in flight" empty state. Kept short so
/// users can dismiss the drawer and move on; this is a status panel, not
/// a tutorial.
fn render_empty_state(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled(
            "  no background tasks running",
            theme.muted(),
        )),
    ];
    Paragraph::new(lines).render(area, buf);
}

/// Render one task row.
///
/// Layout (width >= 80):
/// ```text
///  ▸ indexing s-38f2        ━━━━━━━━━━━━░░░░  62%  x
/// ```
/// The caret, label, bar, percentage, and hint are all separate spans so
/// each carries its own style. The bar itself is two spans (filled + empty)
/// so the progress color doesn't bleed into the un-filled remainder.
fn render_row(
    task: &TaskHandle,
    focused: bool,
    row_width: usize,
    compact: bool,
    theme: &Theme,
) -> Line<'static> {
    let caret_span = caret_span(focused, theme);

    // Label — truncate so it doesn't wrap into the bar region. Width
    // budget: row width minus caret (2) minus bar region minus status/hint
    // minus a few separating spaces. We give the label as much room as we
    // can and let it take the slack.
    let bar_cols = if compact { 6 } else { 16 };
    let status_cols = if compact { 6 } else { 10 };
    let hint_cols = if compact { 0 } else { 3 };
    // caret + 1 space + label + 2 spaces + bar + 2 spaces + status + hint
    let fixed = 2 + 1 + 2 + bar_cols + 2 + status_cols + hint_cols;
    let label_budget = row_width.saturating_sub(fixed).max(6);
    let label = truncate_to_cols(&task.label, label_budget);
    let label_pad = label_budget.saturating_sub(display_width(&label));

    let label_span = Span::styled(label, row_label_style(task, focused, theme));

    let (bar_filled, bar_empty) = render_bar_spans(task, bar_cols, theme);
    let status_span = render_status_span(task, status_cols, theme);
    let hint_span = if compact {
        Span::raw(String::new())
    } else {
        render_hint_span(task, focused, theme)
    };

    Line::from(vec![
        caret_span,
        Span::raw(" "),
        label_span,
        Span::raw(" ".repeat(label_pad)),
        Span::raw("  "),
        bar_filled,
        bar_empty,
        Span::raw("  "),
        status_span,
        Span::raw(" "),
        hint_span,
    ])
}

/// Focus caret — visible-but-dim on non-focused rows so the eye locks
/// onto the focused one. Using two characters of raw space on unfocused
/// rows keeps the columns aligned without a per-row conditional in the
/// label renderer.
fn caret_span(focused: bool, theme: &Theme) -> Span<'static> {
    if focused {
        Span::styled(
            format!("{CARET} "),
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!("{CARET} "), theme.dim())
    }
}

/// Pick the style for the task label based on state + focus. Keeps the
/// styling decisions next to the label so the row assembler stays short.
fn row_label_style(task: &TaskHandle, focused: bool, theme: &Theme) -> Style {
    let base = match task.state {
        TaskState::Done => Style::default().fg(theme.green),
        TaskState::Failed(_) => Style::default().fg(theme.red),
        TaskState::Canceled => theme.dim(),
        TaskState::Running => Style::default().fg(theme.text),
    };
    if focused {
        base.add_modifier(Modifier::BOLD)
    } else {
        base
    }
}

/// Produce (filled, empty) spans for a `bar_cols`-wide progress bar.
///
/// - `Running` with known progress: colored fill + dim remainder.
/// - `Running` with `None` progress (indeterminate): all-filled dim bar —
///   the drawer doesn't animate, so we render a neutral band rather than
///   a fake "sweep" that lies about the actual state.
/// - `Done`: fully filled green.
/// - `Failed`: fully filled red.
/// - `Canceled`: fully empty — it never completed, so drawing a fill
///   would overstate progress.
fn render_bar_spans(task: &TaskHandle, bar_cols: usize, theme: &Theme) -> (Span<'static>, Span<'static>) {
    let fill_style = match task.state {
        TaskState::Done => Style::default().fg(theme.green),
        TaskState::Failed(_) => Style::default().fg(theme.red),
        TaskState::Canceled => theme.dim(),
        TaskState::Running => Style::default().fg(theme.mauve),
    };
    let empty_style = theme.dim();

    let fraction = match task.state {
        TaskState::Done => 1.0,
        TaskState::Failed(_) => 1.0,
        TaskState::Canceled => 0.0,
        TaskState::Running => task.progress.unwrap_or(0.0),
    };
    let filled_cells = ((fraction.clamp(0.0, 1.0)) * bar_cols as f64).round() as usize;
    let filled_cells = filled_cells.min(bar_cols);
    let empty_cells = bar_cols - filled_cells;

    (
        Span::styled(BAR_FILLED.to_string().repeat(filled_cells), fill_style),
        Span::styled(BAR_EMPTY.to_string().repeat(empty_cells), empty_style),
    )
}

/// Right-aligned status column — `62%`, `done`, `failed`, `cancel`.
/// Pads on the left so percentages line up regardless of digit count.
fn render_status_span(task: &TaskHandle, width: usize, theme: &Theme) -> Span<'static> {
    let (text, style) = match &task.state {
        TaskState::Running => match task.progress {
            Some(p) => {
                let pct = (p * 100.0).round() as u32;
                (format!("{pct}%"), theme.subtle())
            }
            None => ("...".into(), theme.muted()),
        },
        TaskState::Done => ("done".into(), Style::default().fg(theme.green)),
        TaskState::Failed(_) => ("failed".into(), Style::default().fg(theme.red)),
        TaskState::Canceled => ("cancel".into(), theme.dim()),
    };
    let pad = width.saturating_sub(display_width(&text));
    Span::styled(format!("{}{}", " ".repeat(pad), text), style)
}

/// Tiny trailing `x` glyph that reminds the user of the cancel hotkey.
/// Only shown on Running rows since cancelling a Done row is meaningless.
fn render_hint_span(task: &TaskHandle, focused: bool, theme: &Theme) -> Span<'static> {
    match task.state {
        TaskState::Running => {
            let style = if focused {
                theme.key_hint()
            } else {
                theme.dim()
            };
            Span::styled(" x ".to_string(), style)
        }
        _ => Span::raw("   "),
    }
}

/// Grapheme-boundary-safe truncate. Uses the same `display_width` the rest
/// of the UI uses so CJK / emoji labels don't misalign the bars.
fn truncate_to_cols(s: &str, cols: usize) -> String {
    if display_width(s) <= cols {
        return s.to_string();
    }
    if cols == 0 {
        return String::new();
    }
    // Walk bytes one char at a time — cheap since labels are short and
    // we bail as soon as adding the next char would overflow.
    let mut out = String::new();
    let budget = cols.saturating_sub(1); // leave 1 col for the ellipsis
    let mut used = 0usize;
    for ch in s.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w > budget {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('\u{2026}'); // …
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::task_queue::TaskQueue;

    fn theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut s = TaskDrawerState::new();
        assert!(!s.visible);
        s.toggle();
        assert!(s.visible);
        s.toggle();
        assert!(!s.visible);
    }

    #[test]
    fn move_up_saturates_at_zero() {
        let mut s = TaskDrawerState::new();
        s.move_up();
        s.move_up();
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn move_down_respects_max() {
        let mut s = TaskDrawerState::new();
        s.move_down(3);
        s.move_down(3);
        s.move_down(3);
        s.move_down(3); // beyond end — clamps
        assert_eq!(s.selected, 2);
    }

    #[test]
    fn move_down_no_op_when_empty() {
        let mut s = TaskDrawerState::new();
        s.move_down(0);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn clamp_pulls_selection_into_range() {
        let mut s = TaskDrawerState {
            visible: true,
            selected: 9,
        };
        s.clamp(3);
        assert_eq!(s.selected, 2);
        s.clamp(0);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn selected_id_returns_none_when_hidden() {
        let mut q = TaskQueue::new();
        q.seed_demo();
        let s = TaskDrawerState {
            visible: false,
            selected: 0,
        };
        assert_eq!(s.selected_id(&q), None);
    }

    #[test]
    fn selected_id_maps_index_to_task() {
        let mut q = TaskQueue::new();
        q.seed_demo();
        let s = TaskDrawerState {
            visible: true,
            selected: 1,
        };
        let id = s.selected_id(&q).unwrap();
        assert_eq!(id, q.get_by_index(1).unwrap().id);
    }

    #[test]
    fn scroll_start_anchors_when_selection_near_top() {
        assert_eq!(scroll_start(0, 4, 10), 0);
        assert_eq!(scroll_start(2, 4, 10), 0);
        assert_eq!(scroll_start(3, 4, 10), 0);
        assert_eq!(scroll_start(4, 4, 10), 1);
        assert_eq!(scroll_start(9, 4, 10), 6);
    }

    #[test]
    fn scroll_start_noop_when_list_fits() {
        assert_eq!(scroll_start(5, 10, 3), 0);
        assert_eq!(scroll_start(0, 0, 10), 0);
    }

    #[test]
    fn truncate_to_cols_appends_ellipsis() {
        assert_eq!(truncate_to_cols("short", 10), "short");
        let out = truncate_to_cols("abcdefghijklmnop", 6);
        assert!(out.ends_with('\u{2026}'));
        assert!(display_width(&out) <= 6);
    }

    #[test]
    fn render_bar_spans_fill_matches_progress() {
        let t = theme();
        let mut q = TaskQueue::new();
        let id = q.push("x".into());
        q.update(id, Some(0.5));
        let task = q.get(id).unwrap();
        let (filled, empty) = render_bar_spans(task, 10, &t);
        assert_eq!(filled.content.chars().count(), 5);
        assert_eq!(empty.content.chars().count(), 5);
    }

    #[test]
    fn render_bar_spans_done_fully_filled() {
        let t = theme();
        let mut q = TaskQueue::new();
        let id = q.push("x".into());
        q.finish(id, Ok(()));
        let task = q.get(id).unwrap();
        let (filled, empty) = render_bar_spans(task, 8, &t);
        assert_eq!(filled.content.chars().count(), 8);
        assert_eq!(empty.content.chars().count(), 0);
    }

    #[test]
    fn render_bar_spans_canceled_fully_empty() {
        let t = theme();
        let mut q = TaskQueue::new();
        let id = q.push("x".into());
        q.cancel(id);
        let task = q.get(id).unwrap();
        let (filled, empty) = render_bar_spans(task, 8, &t);
        assert_eq!(filled.content.chars().count(), 0);
        assert_eq!(empty.content.chars().count(), 8);
    }

    #[test]
    fn render_row_produces_line_with_caret() {
        let t = theme();
        let mut q = TaskQueue::new();
        let id = q.push("indexing".into());
        q.update(id, Some(0.5));
        let task = q.get(id).unwrap();
        let line = render_row(task, true, 80, false, &t);
        let joined: String = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
        assert!(joined.contains("indexing"));
        assert!(joined.contains('\u{25B8}'));
    }
}
