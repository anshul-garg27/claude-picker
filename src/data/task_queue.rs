//! Task queue for async work surfaced in the background drawer (`w`).
//!
//! Tasks are lightweight — a label, a progress fraction, a lifecycle state.
//! We don't run the actual work here; we just expose a shared handle that
//! background threads can update. The drawer widget reads [`TaskQueue`] each
//! frame and renders a row per task.
//!
//! Threading model:
//! - [`SharedTaskQueue`] is `Arc<Mutex<TaskQueue>>`. Producers (a fork-graph
//!   builder thread, a heatmap rollup, an indexer) hold a clone of the Arc
//!   and take the mutex for short windows: `push` on start, `update` on every
//!   progress event, `finish` on completion.
//! - The UI takes the mutex once per render to copy out what it needs. It
//!   never blocks on producers — the mutex is only held across trivial
//!   reads/writes so contention stays in microseconds even under load.
//!
//! Cancellation: [`cancel`](TaskQueue::cancel) flips the state to
//! [`TaskState::Canceled`]. It is the **producer's** job to observe that
//! state and stop doing work — the queue itself never kills threads. A
//! typical producer polls `queue.lock().is_cancelled(id)` between work units.
//!
//! Sweeping: finished/failed/cancelled rows auto-evict after a configurable
//! age so the drawer doesn't grow unbounded. Call [`TaskQueue::sweep`] on a
//! periodic tick (the app's redraw loop is already the natural home).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Lifecycle of a single task row.
///
/// Progression is `Running -> (Done | Failed | Canceled)`; there is no
/// `Pending` because we only ever push a task when it has actually started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskState {
    /// Work in progress. Rendered with a live progress bar.
    Running,
    /// Finished successfully. Rendered with `done` and eligible for sweep.
    Done,
    /// Failed with the given error message. Eligible for sweep.
    Failed(String),
    /// User-requested cancellation. Eligible for sweep. Producers should
    /// observe this and unwind; the queue itself does nothing to the
    /// underlying work.
    Canceled,
}

impl TaskState {
    /// True if the state is terminal — used by the sweeper to decide whether
    /// a row is a candidate for eviction.
    pub fn is_terminal(&self) -> bool {
        !matches!(self, TaskState::Running)
    }
}

/// One task row. Cheap to clone — no heap beyond the label/error strings.
#[derive(Clone, Debug)]
pub struct TaskHandle {
    /// Monotonically increasing id — unique for the lifetime of the queue.
    /// Callers use this to reference the task in later `update`/`finish`/
    /// `cancel` calls; they should **not** rely on ordering semantics
    /// beyond "bigger = started later".
    pub id: u64,
    /// Human label shown in the drawer. Producers should keep it short —
    /// the drawer truncates to the row width but we don't do wrapping.
    pub label: String,
    /// Progress fraction in `0.0..=1.0`. `None` means indeterminate — the
    /// drawer then renders a pulsing bar instead of a filled one.
    pub progress: Option<f64>,
    /// Current lifecycle state.
    pub state: TaskState,
    /// When the task was first `push`-ed. Used by the sweeper together with
    /// the finish time; also handy for future "oldest first" sort orders.
    pub started_at: Instant,
    /// When the task transitioned to a terminal state. `None` while running.
    /// The sweeper considers `max_age` against this timestamp so Done rows
    /// linger briefly before vanishing (lets the user read the outcome).
    pub finished_at: Option<Instant>,
}

/// Collection of task rows plus a monotonic id counter.
///
/// Keep this type `Sync` via external locking ([`SharedTaskQueue`]) rather
/// than interior mutability so the lock discipline is obvious at every call
/// site — callers write `queue.lock().unwrap().push(...)`, which is loud and
/// audited.
#[derive(Default, Debug)]
pub struct TaskQueue {
    tasks: Vec<TaskHandle>,
    next_id: u64,
}

impl TaskQueue {
    /// New empty queue. No capacity hint — the drawer shows at most ~8
    /// rows and the sweeper evicts terminal rows, so growth is bounded
    /// by steady-state concurrency.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new running task. Returns the id to use for subsequent
    /// `update`/`finish`/`cancel` calls.
    pub fn push(&mut self, label: String) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.tasks.push(TaskHandle {
            id,
            label,
            progress: None,
            state: TaskState::Running,
            started_at: Instant::now(),
            finished_at: None,
        });
        id
    }

    /// Update progress for an existing task. A no-op if the id is unknown
    /// (the task may have been swept) or if the task is no longer running
    /// (producers racing with cancellation is normal and shouldn't panic).
    pub fn update(&mut self, id: u64, progress: Option<f64>) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            if task.state == TaskState::Running {
                task.progress = progress.map(|p| p.clamp(0.0, 1.0));
            }
        }
    }

    /// Mark a task terminal. `Ok(())` -> [`TaskState::Done`]; `Err(msg)` ->
    /// [`TaskState::Failed`]. Sets `finished_at` so the sweeper can age it
    /// out. Double-finish is a no-op (last-write-wins would be surprising).
    pub fn finish(&mut self, id: u64, result: Result<(), String>) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            if task.state == TaskState::Running {
                task.state = match result {
                    Ok(()) => TaskState::Done,
                    Err(msg) => TaskState::Failed(msg),
                };
                task.finished_at = Some(Instant::now());
                // Snap progress to 1.0 on clean finish so the bar looks
                // complete in the half-second before sweep removes it.
                if task.state == TaskState::Done {
                    task.progress = Some(1.0);
                }
            }
        }
    }

    /// Flip a running task to [`TaskState::Canceled`]. Producers observe
    /// this via [`Self::is_cancelled`] and unwind. A no-op on unknown or
    /// already-terminal ids.
    pub fn cancel(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            if task.state == TaskState::Running {
                task.state = TaskState::Canceled;
                task.finished_at = Some(Instant::now());
            }
        }
    }

    /// Probe for cancellation from the producer side. Returns `true` if the
    /// task exists and is in [`TaskState::Canceled`], so a polling producer
    /// can bail out cleanly.
    pub fn is_cancelled(&self, id: u64) -> bool {
        self.tasks
            .iter()
            .any(|t| t.id == id && t.state == TaskState::Canceled)
    }

    /// Iterate rows in push order — the drawer renders them top-to-bottom,
    /// so "oldest first" keeps the visual layout stable as new tasks land
    /// at the bottom.
    pub fn iter(&self) -> impl Iterator<Item = &TaskHandle> {
        self.tasks.iter()
    }

    /// Number of rows currently shown. Drawer uses this to clamp selection
    /// and to decide whether to render the empty state.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// True if no rows exist. Matches the `Vec::is_empty` convention so
    /// clippy stops complaining about `len() == 0`.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Count of currently-running rows. Useful for the status bar ("3 bg
    /// tasks") and for deciding whether to render a footer badge.
    pub fn active_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|t| t.state == TaskState::Running)
            .count()
    }

    /// Look up a task by id. Returns `None` if swept or never existed.
    pub fn get(&self, id: u64) -> Option<&TaskHandle> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Look up by row index (matches `iter` order). The drawer's selection
    /// cursor is an index, so this is how we translate it back to an id
    /// for `cancel`.
    pub fn get_by_index(&self, idx: usize) -> Option<&TaskHandle> {
        self.tasks.get(idx)
    }

    /// Drop Done / Failed / Canceled tasks whose `finished_at` is older
    /// than `max_age`. Running tasks are never swept regardless of age —
    /// a long-running indexer should still show a live progress bar.
    ///
    /// `max_age` in the low seconds feels right; the stub wiring uses
    /// 10s so the user has time to register "ok, that finished" before
    /// the row vanishes.
    pub fn sweep(&mut self, max_age: Duration) {
        let now = Instant::now();
        self.tasks.retain(|t| match (&t.state, t.finished_at) {
            (TaskState::Running, _) => true,
            (_, Some(finished)) => now.duration_since(finished) < max_age,
            // Terminal state without a finish timestamp shouldn't happen,
            // but if it does we drop on sight so it can't linger.
            (_, None) => false,
        });
    }

    /// Dev/test helper — seed 3 stub tasks at varying progress so we can
    /// demo the drawer without wiring real producers yet. Gated on
    /// `debug_assertions` so release builds don't ship a demo hook.
    #[cfg(any(test, debug_assertions))]
    pub fn seed_demo(&mut self) {
        let id1 = self.push("indexing s-38f2".into());
        self.update(id1, Some(0.62));

        let id2 = self.push("fork graph build".into());
        self.update(id2, Some(1.0));
        self.finish(id2, Ok(()));

        let id3 = self.push("heatmap rollup 24x7".into());
        self.update(id3, Some(0.44));
    }
}

/// Handle producers and the UI share. Lock briefly — never across IO, never
/// across long CPU work. Typical use:
///
/// ```ignore
/// let q = queue.clone();
/// std::thread::spawn(move || {
///     let id = q.lock().unwrap().push("indexing".into());
///     for step in 0..100 {
///         if q.lock().unwrap().is_cancelled(id) { return; }
///         q.lock().unwrap().update(id, Some(step as f64 / 100.0));
///         // ... do a chunk of work ...
///     }
///     q.lock().unwrap().finish(id, Ok(()));
/// });
/// ```
pub type SharedTaskQueue = Arc<Mutex<TaskQueue>>;

/// Construct a fresh shared queue. Exists so the app doesn't have to know
/// the concrete `Arc<Mutex<...>>` shape at call sites.
pub fn new_shared() -> SharedTaskQueue {
    Arc::new(Mutex::new(TaskQueue::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_assigns_monotonic_ids() {
        let mut q = TaskQueue::new();
        let a = q.push("a".into());
        let b = q.push("b".into());
        let c = q.push("c".into());
        assert!(b > a && c > b, "ids should increase: {a}, {b}, {c}");
    }

    #[test]
    fn update_clamps_progress_to_unit_interval() {
        let mut q = TaskQueue::new();
        let id = q.push("x".into());
        q.update(id, Some(1.7));
        assert_eq!(q.get(id).unwrap().progress, Some(1.0));
        q.update(id, Some(-0.2));
        assert_eq!(q.get(id).unwrap().progress, Some(0.0));
        q.update(id, None);
        assert_eq!(q.get(id).unwrap().progress, None);
    }

    #[test]
    fn finish_success_marks_done_and_fills_bar() {
        let mut q = TaskQueue::new();
        let id = q.push("x".into());
        q.finish(id, Ok(()));
        let t = q.get(id).unwrap();
        assert_eq!(t.state, TaskState::Done);
        assert_eq!(t.progress, Some(1.0));
        assert!(t.finished_at.is_some());
    }

    #[test]
    fn finish_failure_preserves_message() {
        let mut q = TaskQueue::new();
        let id = q.push("x".into());
        q.finish(id, Err("disk full".into()));
        match &q.get(id).unwrap().state {
            TaskState::Failed(msg) => assert_eq!(msg, "disk full"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn cancel_flips_running_tasks_only() {
        let mut q = TaskQueue::new();
        let running = q.push("r".into());
        let done = q.push("d".into());
        q.finish(done, Ok(()));

        q.cancel(running);
        q.cancel(done); // no-op; should not revert the Done state

        assert_eq!(q.get(running).unwrap().state, TaskState::Canceled);
        assert_eq!(q.get(done).unwrap().state, TaskState::Done);
        assert!(q.is_cancelled(running));
        assert!(!q.is_cancelled(done));
    }

    #[test]
    fn active_count_tracks_running_only() {
        let mut q = TaskQueue::new();
        let a = q.push("a".into());
        let _b = q.push("b".into());
        let c = q.push("c".into());
        assert_eq!(q.active_count(), 3);
        q.finish(a, Ok(()));
        q.cancel(c);
        assert_eq!(q.active_count(), 1);
    }

    #[test]
    fn sweep_retains_running_and_recent_terminal() {
        let mut q = TaskQueue::new();
        let running = q.push("r".into());
        let recent = q.push("recent".into());
        q.finish(recent, Ok(()));

        // 60s is well above the recent finish-time, so nothing evicts.
        q.sweep(Duration::from_secs(60));
        assert!(q.get(running).is_some());
        assert!(q.get(recent).is_some());

        // Zero-age sweep should evict the terminal row but not the runner.
        q.sweep(Duration::from_secs(0));
        assert!(q.get(running).is_some());
        assert!(q.get(recent).is_none());
    }

    #[test]
    fn update_no_op_after_terminal() {
        let mut q = TaskQueue::new();
        let id = q.push("x".into());
        q.finish(id, Ok(()));
        q.update(id, Some(0.1));
        // Still 1.0 — we snapped on finish and update is a no-op.
        assert_eq!(q.get(id).unwrap().progress, Some(1.0));
    }

    #[test]
    fn seed_demo_produces_three_rows() {
        let mut q = TaskQueue::new();
        q.seed_demo();
        assert_eq!(q.len(), 3);
        assert_eq!(q.active_count(), 2);
    }

    #[test]
    fn is_terminal_covers_every_variant() {
        assert!(!TaskState::Running.is_terminal());
        assert!(TaskState::Done.is_terminal());
        assert!(TaskState::Failed("e".into()).is_terminal());
        assert!(TaskState::Canceled.is_terminal());
    }
}
