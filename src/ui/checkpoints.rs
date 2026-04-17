//! `claude-picker --checkpoints` — `/rewind` browser.
//!
//! Two stacked sections:
//!
//! 1. **Recent checkpoints** — one row per `(project, checkpoint)`, showing
//!    the short hash, tracked-file count, and a relative timestamp.
//! 2. **Files in selected checkpoint** — per-file list for the currently
//!    focused row. The UI doesn't render the diff itself (that's owned by
//!    the diff module / another agent); we only show the file list + a line
//!    tally placeholder.
//!
//! Render is pure — the command layer supplies [`CheckpointsView`].

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::data::checkpoints::{Checkpoint, CheckpointSession};
use crate::theme::Theme;
use crate::ui::text::{pad_to_width, truncate_to_width};

/// Flat row index into the panel: (session_idx, checkpoint_idx).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointCursor {
    pub session_index: usize,
    pub checkpoint_index: usize,
}

/// The payload the command layer passes to [`render`].
#[derive(Debug)]
pub struct CheckpointsView<'a> {
    pub sessions: &'a [CheckpointSession],
    pub selected: CheckpointCursor,
    /// Total checkpoints across every session — shown in the header caption.
    pub total: u32,
    /// Index-aligned with the list of flattened (session, checkpoint) rows
    /// in [`flatten_rows`] — human-readable relative timestamps.
    pub relative_labels: &'a [String],
}

const MIN_W: u16 = 72;
const MIN_H: u16 = 20;
const MAX_W: u16 = 110;

pub fn render(frame: &mut Frame<'_>, area: Rect, view: &CheckpointsView<'_>, theme: &Theme) {
    if area.width < MIN_W || area.height < MIN_H {
        render_too_small(frame, area, theme);
        return;
    }
    let inner = center_capped(area, MAX_W);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border_active())
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "claude-picker · checkpoints",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_top(
            Line::from(Span::styled(
                format!(" {} total ", view.total),
                theme.subtle(),
            ))
            .right_aligned(),
        );
    let body = block.inner(inner);
    frame.render_widget(block, inner);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top blank
            Constraint::Min(8),    // recent checkpoints
            Constraint::Length(1), // blank
            Constraint::Length(7), // files in selected
            Constraint::Length(1), // footer
        ])
        .split(body);

    render_recent(frame, chunks[1], view, theme);
    render_files(frame, chunks[3], view, theme);
    render_footer(frame, chunks[4], theme);
}

fn render_recent(frame: &mut Frame<'_>, area: Rect, view: &CheckpointsView<'_>, theme: &Theme) {
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(view.total as usize * 2 + 2);
    lines.push(section_header("── recent checkpoints ──", theme));

    if view.sessions.is_empty() {
        lines.push(Line::raw(""));
        lines.push(
            Line::from(Span::styled("No checkpoints yet.", theme.muted()))
                .alignment(Alignment::Center),
        );
        lines.push(
            Line::from(Span::styled(
                "Claude Code writes these automatically when it edits files.",
                theme.subtle(),
            ))
            .alignment(Alignment::Center),
        );
        frame.render_widget(Paragraph::new(lines), area);
        return;
    }

    let rows = flatten_rows(view.sessions);

    // Render up to what fits in the area minus the header row.
    let available = (area.height as usize).saturating_sub(2);
    let start = start_index(rows.len(), available, view.selected, view.sessions);

    for (i, (sid_idx, cp_idx)) in rows.iter().enumerate().skip(start).take(available) {
        let selected =
            view.selected.session_index == *sid_idx && view.selected.checkpoint_index == *cp_idx;
        let session = &view.sessions[*sid_idx];
        let checkpoint = &session.checkpoints[*cp_idx];
        let rel: &str = view
            .relative_labels
            .get(i)
            .map(String::as_str)
            .unwrap_or("");
        lines.push(render_project_line(session, selected, theme));
        lines.push(render_checkpoint_line(checkpoint, rel, selected, theme));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_project_line<'a>(
    session: &'a CheckpointSession,
    selected: bool,
    theme: &Theme,
) -> Line<'a> {
    let caret = if selected { "▸" } else { " " };
    let proj_style = if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text)
    };
    Line::from(vec![
        Span::styled(
            format!("  {caret} "),
            Style::default().fg(if selected {
                theme.mauve
            } else {
                theme.overlay0
            }),
        ),
        Span::styled(session.project_label(), proj_style),
        Span::styled(" / ", theme.muted()),
        Span::styled(
            short_session(&session.session_id),
            Style::default().fg(theme.teal),
        ),
    ])
}

fn render_checkpoint_line<'a>(
    cp: &'a Checkpoint,
    relative: &'a str,
    selected: bool,
    theme: &Theme,
) -> Line<'a> {
    let hash_style = if selected {
        Style::default()
            .fg(theme.peach)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.peach)
    };
    Line::from(vec![
        Span::raw("      "),
        Span::styled(format!("snapshot {}", cp.short_hash()), hash_style),
        Span::styled("  ·  ", theme.muted()),
        Span::styled(
            format!(
                "{} file{}",
                cp.files.len(),
                if cp.files.len() == 1 { "" } else { "s" }
            ),
            theme.subtle(),
        ),
        Span::styled("  ·  ", theme.muted()),
        Span::styled(
            if relative.is_empty() {
                "unknown time".to_string()
            } else {
                relative.to_string()
            },
            theme.muted(),
        ),
    ])
}

fn render_files(frame: &mut Frame<'_>, area: Rect, view: &CheckpointsView<'_>, theme: &Theme) {
    let mut lines = Vec::with_capacity(8);
    lines.push(section_header("── files in selected checkpoint ──", theme));

    let Some(cp) = selected_checkpoint(view) else {
        lines.push(Line::raw(""));
        lines.push(
            Line::from(Span::styled("(select a checkpoint)", theme.muted()))
                .alignment(Alignment::Center),
        );
        frame.render_widget(Paragraph::new(lines), area);
        return;
    };

    if cp.files.is_empty() {
        lines.push(Line::raw(""));
        lines.push(
            Line::from(Span::styled(
                "(this snapshot tracked zero files)",
                theme.muted(),
            ))
            .alignment(Alignment::Center),
        );
    } else {
        for f in cp.files.iter().take(5) {
            let path_budget = (area.width as usize).saturating_sub(30);
            let glyph = if f.version <= 1 { "A" } else { "M" };
            let glyph_style = if glyph == "A" {
                Style::default().fg(theme.green)
            } else {
                Style::default().fg(theme.yellow)
            };
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(glyph, glyph_style),
                Span::raw("  "),
                Span::styled(
                    truncate_to_width(&f.real_path.display().to_string(), path_budget),
                    Style::default().fg(theme.text),
                ),
                Span::raw("  "),
                Span::styled(pad_to_width(&format!("v{}", f.version), 6), theme.muted()),
            ]));
        }
        if cp.files.len() > 5 {
            lines.push(Line::from(Span::styled(
                format!("    … and {} more", cp.files.len() - 5),
                theme.muted(),
            )));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let line = Line::from(vec![
        Span::styled(
            "  ↑↓",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" navigate · ", theme.muted()),
        Span::styled(
            "Enter",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" view diff · ", theme.muted()),
        Span::styled(
            "r",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" rewind to this · ", theme.muted()),
        Span::styled(
            "q",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" quit", theme.muted()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

// ── Flattening helpers ───────────────────────────────────────────────────

/// Flatten `[(session_idx, checkpoint_idx), …]` for row-by-row rendering.
/// Declared `pub` so the command layer can use it to clamp cursor movement.
pub fn flatten_rows(sessions: &[CheckpointSession]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (i, s) in sessions.iter().enumerate() {
        // Most recent first per session, matching the panel's freshness
        // ordering. Rev so if the data layer sorts oldest-first we still get
        // newest-first in the UI.
        for (j, _) in s.checkpoints.iter().enumerate().rev() {
            out.push((i, j));
        }
    }
    out
}

/// Return the currently selected checkpoint, if the cursor points at one.
pub fn selected_checkpoint<'a>(view: &CheckpointsView<'a>) -> Option<&'a Checkpoint> {
    view.sessions
        .get(view.selected.session_index)
        .and_then(|s| s.checkpoints.get(view.selected.checkpoint_index))
}

/// Compute a viewport start index such that the selected row is visible.
///
/// The section shows 2 lines per checkpoint (project + snapshot), so we work
/// in "rows" of 2. `available` is the number of text lines the panel has.
fn start_index(
    total_rows: usize,
    available: usize,
    selected: CheckpointCursor,
    sessions: &[CheckpointSession],
) -> usize {
    let sel_flat = flat_index(selected, sessions).unwrap_or(0);
    let max_lines = (available / 2).max(1);
    (sel_flat + 1)
        .saturating_sub(max_lines)
        .min(total_rows.saturating_sub(max_lines))
}

fn flat_index(cursor: CheckpointCursor, sessions: &[CheckpointSession]) -> Option<usize> {
    let mut idx = 0usize;
    for (i, s) in sessions.iter().enumerate() {
        for (j, _) in s.checkpoints.iter().enumerate().rev() {
            if cursor.session_index == i && cursor.checkpoint_index == j {
                return Some(idx);
            }
            idx += 1;
        }
    }
    None
}

fn short_session(sid: &str) -> String {
    sid.chars().take(8).collect::<String>() + "…"
}

fn section_header<'a>(label: &'a str, theme: &Theme) -> Line<'a> {
    Line::from(Span::styled(
        format!("  {label}"),
        Style::default()
            .fg(theme.overlay1)
            .add_modifier(Modifier::DIM),
    ))
}

fn center_capped(area: Rect, max_width: u16) -> Rect {
    let w = area.width.min(max_width);
    let x_offset = area.width.saturating_sub(w) / 2;
    Rect {
        x: area.x + x_offset,
        y: area.y,
        width: w,
        height: area.height,
    }
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let p = Paragraph::new(vec![
        Line::raw(""),
        Line::from(Span::styled(
            format!("resize terminal — need at least {MIN_W}×{MIN_H}"),
            theme.muted(),
        ))
        .alignment(Alignment::Center),
    ]);
    frame.render_widget(p, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::checkpoints::TrackedFile;
    use chrono::TimeZone;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn mk_session(sid: &str, project: &str, cps: Vec<Checkpoint>) -> CheckpointSession {
        CheckpointSession {
            session_id: sid.to_string(),
            project_dir: Some(PathBuf::from(format!("/tmp/{project}"))),
            checkpoints: cps,
            on_disk_backups: 2,
        }
    }
    fn mk_cp(hash: &str, files: Vec<TrackedFile>) -> Checkpoint {
        Checkpoint {
            message_id: hash.to_string(),
            session_id: "sid".into(),
            timestamp: Some(chrono::Utc.timestamp_opt(1_700_000_000, 0).unwrap()),
            files,
        }
    }
    fn mk_file(path: &str, v: u32) -> TrackedFile {
        TrackedFile {
            real_path: PathBuf::from(path),
            backup_file: "x@v1".into(),
            version: v,
            backup_time: None,
        }
    }

    #[test]
    fn render_draws_sessions_and_files() {
        let sess = vec![mk_session(
            "abc12345",
            "architex",
            vec![mk_cp(
                "deadbeef0000",
                vec![mk_file("src/auth/middleware.ts", 3)],
            )],
        )];
        let theme = Theme::mocha();
        let labels = vec!["12h ago".to_string()];
        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        terminal
            .draw(|f| {
                let view = CheckpointsView {
                    sessions: &sess,
                    selected: CheckpointCursor {
                        session_index: 0,
                        checkpoint_index: 0,
                    },
                    total: 1,
                    relative_labels: &labels,
                };
                render(f, f.area(), &view, &theme);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let content: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("checkpoints"));
        assert!(content.contains("architex"));
        assert!(content.contains("snapshot deadbeef"));
        assert!(content.contains("12h ago"));
        assert!(content.contains("middleware"));
    }

    #[test]
    fn empty_state() {
        let theme = Theme::mocha();
        let labels: Vec<String> = Vec::new();
        let mut terminal = Terminal::new(TestBackend::new(100, 28)).unwrap();
        terminal
            .draw(|f| {
                let view = CheckpointsView {
                    sessions: &[],
                    selected: CheckpointCursor {
                        session_index: 0,
                        checkpoint_index: 0,
                    },
                    total: 0,
                    relative_labels: &labels,
                };
                render(f, f.area(), &view, &theme);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let content: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("No checkpoints yet"));
    }

    #[test]
    fn flatten_rows_is_newest_first_per_session() {
        let sess = vec![mk_session(
            "sid",
            "x",
            vec![
                mk_cp("one", vec![]),
                mk_cp("two", vec![]),
                mk_cp("three", vec![]),
            ],
        )];
        let rows = flatten_rows(&sess);
        // newest-first = index 2, 1, 0.
        assert_eq!(rows, vec![(0, 2), (0, 1), (0, 0)]);
    }
}
