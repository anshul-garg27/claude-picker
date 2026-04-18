//! Preview pane — the right half of the picker.
//!
//! Re-parses the currently-selected session's JSONL to pull out the last few
//! user/assistant exchanges, then renders them with role-tinted labels. We
//! deliberately reimplement a focused slice of the loader (no pricing maths,
//! no fork tracking) so the preview call is fast: tens of milliseconds even
//! on multi-megabyte sessions.
//!
//! Shape of the pane (top to bottom):
//! 1. header row — "PREVIEW" left, "<ID8>" right
//! 2. session name (green-bold) + meta line (muted)
//! 3. horizontal rule
//! 4. last ≤6 exchanges as role-tinted lines
//! 5. footer rule + stats row (`msgs · tokens · model · cost`)

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use chrono::Local;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;
use serde::Deserialize;

use crate::app::App;
use crate::data::session::noise_prefixes;
use crate::data::Session;
use crate::theme::{self, Theme};
use crate::ui::model_pill;
use crate::ui::text::{display_width, truncate_to_width};

/// How many exchanges to show. Matches the brief (4-6) — 6 is usually what
/// fits on a 40-row terminal after header / footer / meta.
const MAX_MESSAGES: usize = 6;

/// Maximum display columns per message body before we truncate with ` …`.
/// Keeps each entry readable as a one-glance summary rather than a wall of
/// text. Measured in terminal cells, not codepoints, so a JP-localised
/// transcript doesn't get double the visual real estate.
const MAX_BODY_COLS: usize = 240;

/// Sync, memoising runner for user-supplied `--preview-cmd` snippets.
///
/// # Execution model
///
/// Preview commands run **synchronously** on the first frame that shows a
/// particular session, then the output is cached for the lifetime of the
/// picker process in a [`HashMap`] keyed by session id. Subsequent frames
/// hit the cache and incur no spawn cost, so scrolling stays snappy even
/// when the snippet takes a hundred milliseconds.
///
/// The synchronous design is deliberate. The alternative (thread pool +
/// polling) doubles the control-flow surface for a feature that's already
/// an escape hatch. Users who want a bounded render delay can wrap their
/// snippet in `timeout`:
///
/// ```text
/// --preview-cmd='timeout 0.5 git -C {cwd} log --oneline -10'
/// ```
///
/// [`RefCell`] gives us interior mutability so the UI render path — which
/// holds only `&App` — can mutate the cache. The UI is strictly single-
/// threaded, so the runtime borrow-check never panics.
#[derive(Debug, Default)]
pub struct PreviewCache {
    inner: RefCell<HashMap<String, PreviewOutput>>,
}

impl PreviewCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of cached entries. Exposed for tests.
    pub fn len(&self) -> usize {
        self.inner.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.borrow().is_empty()
    }

    /// Look up or insert-and-return the cached output for `session`. The
    /// closure runs only on cache miss.
    fn get_or_insert<F>(&self, session: &Session, f: F) -> PreviewOutput
    where
        F: FnOnce(&Session) -> PreviewOutput,
    {
        let mut guard = self.inner.borrow_mut();
        if let Some(cached) = guard.get(&session.id) {
            return cached.clone();
        }
        let fresh = f(session);
        guard.insert(session.id.clone(), fresh.clone());
        fresh
    }
}

/// Cached result of a `--preview-cmd` run.
#[derive(Debug, Clone)]
pub struct PreviewOutput {
    pub body: String,
    pub kind: PreviewKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    /// Command exited 0 with non-empty stdout.
    Ok,
    /// Command exited 0 but produced no stdout — renderer falls back to the
    /// built-in view.
    Empty,
    /// Command exited non-zero or failed to spawn. `body` holds stderr or
    /// the OS error message.
    Failed,
}

/// Substitute `{sid}` and `{cwd}` inside a user-supplied command template.
///
/// No other placeholders are recognised — the brief explicitly scopes the
/// allowed substitutions to session id + project path.
pub fn substitute_placeholders(template: &str, sid: &str, cwd: &Path) -> String {
    let cwd_str = cwd.display().to_string();
    template.replace("{sid}", sid).replace("{cwd}", &cwd_str)
}

/// Run the preview command for `session` through `sh -c`. Returns a
/// [`PreviewOutput`] capturing stdout + a kind classification. Callers
/// should memoise via [`PreviewCache`] to keep latency within frame budget.
///
/// stderr is captured but only surfaced on non-zero exit, so a well-behaved
/// command that writes progress noise to stderr still renders cleanly.
pub fn execute_preview_command(template: &str, session: &Session) -> PreviewOutput {
    let cmd = substitute_placeholders(template, &session.id, &session.project_dir);

    let result = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match result {
        Ok(out) if out.status.success() => {
            let body = String::from_utf8_lossy(&out.stdout).to_string();
            let trimmed = body.trim();
            if trimmed.is_empty() {
                PreviewOutput {
                    body: String::new(),
                    kind: PreviewKind::Empty,
                }
            } else {
                PreviewOutput {
                    body,
                    kind: PreviewKind::Ok,
                }
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let body = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!(
                    "command exited with status {}",
                    out.status
                        .code()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "?".to_string())
                )
            };
            PreviewOutput {
                body,
                kind: PreviewKind::Failed,
            }
        }
        Err(e) => PreviewOutput {
            body: format!("spawn failed: {e}"),
            kind: PreviewKind::Failed,
        },
    }
}

/// Render the preview pane into `area`.
///
/// If no session is currently selected — e.g. the filter matched zero rows —
/// falls back to a "nothing to preview" placeholder.
///
/// When `app.preview_cmd` is `Some`, switches to the escape-hatch renderer:
/// spawns the user's snippet (with `{sid}` / `{cwd}` substituted), captures
/// stdout, and renders the output in place of the built-in view. See
/// [`PreviewCache`] for the sync+cache model.
pub fn render(f: &mut Frame<'_>, area: Rect, app: &App) {
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(session) = app.selected_session_ref() else {
        placeholder(f, inner, theme);
        return;
    };

    // Escape-hatch renderer. Still keep the header band so the user has
    // context (label + id) next to the custom output.
    if let Some(cmd) = app.preview_cmd.as_deref() {
        render_custom_cmd(f, inner, session, cmd, &app.preview_cache, theme);
        return;
    }

    // Three zones: header, body (flex), footer stats.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // header + title + meta + rule
            Constraint::Min(1),    // message bodies
            Constraint::Length(2), // separator + stats
        ])
        .split(inner);

    render_header(f, chunks[0], session, theme);
    render_body(f, chunks[1], session, theme);
    render_footer(f, chunks[2], session, theme);
}

fn render_custom_cmd(
    f: &mut Frame<'_>,
    area: Rect,
    session: &Session,
    cmd_template: &str,
    cache: &PreviewCache,
    theme: &Theme,
) {
    let template = cmd_template.to_string();
    let output = cache.get_or_insert(session, |s| execute_preview_command(&template, s));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // header
            Constraint::Min(1),    // custom body
            Constraint::Length(2), // status bar
        ])
        .split(area);

    render_header(f, chunks[0], session, theme);

    if matches!(output.kind, PreviewKind::Empty) {
        render_body(f, chunks[1], session, theme);
    } else {
        render_custom_body(f, chunks[1], &output, theme);
    }

    render_custom_status(f, chunks[2], cmd_template, &output, theme);
}

fn render_custom_body(f: &mut Frame<'_>, area: Rect, output: &PreviewOutput, theme: &Theme) {
    let accent = match output.kind {
        PreviewKind::Ok => theme.text,
        PreviewKind::Failed => theme.red,
        PreviewKind::Empty => theme.muted().fg.unwrap_or(theme.overlay0),
    };

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(output.body.lines().count() + 2);
    lines.push(Line::raw(""));
    for raw_line in output.body.lines() {
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(raw_line.to_string(), Style::default().fg(accent)),
        ]));
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_custom_status(
    f: &mut Frame<'_>,
    area: Rect,
    cmd_template: &str,
    output: &PreviewOutput,
    theme: &Theme,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    f.render_widget(
        Paragraph::new(section_divider_line("preview-cmd", area.width, theme)),
        chunks[0],
    );

    let (label, label_style) = match output.kind {
        PreviewKind::Ok => (
            "ok",
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
        PreviewKind::Empty => (
            "empty \u{2014} showing default",
            Style::default().fg(theme.yellow),
        ),
        PreviewKind::Failed => (
            "failed",
            Style::default().fg(theme.red).add_modifier(Modifier::BOLD),
        ),
    };

    let cmd_trim = truncate_to_width(cmd_template, 60);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(label, label_style),
            Span::styled("  ·  ", theme.dim()),
            Span::styled(format!("`{cmd_trim}`"), theme.muted()),
        ])),
        chunks[1],
    );
}

fn placeholder(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let glyph_style = Style::default()
        .fg(theme.surface2)
        .add_modifier(Modifier::BOLD);
    let primary = Style::default()
        .fg(theme.subtext1)
        .add_modifier(Modifier::BOLD);
    let secondary = Style::default()
        .fg(theme.overlay0)
        .add_modifier(Modifier::ITALIC);
    let pad = area.height.saturating_sub(6) / 2;
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(pad as usize + 6);
    for _ in 0..pad {
        lines.push(Line::raw(""));
    }
    lines.push(Line::from(Span::styled("\u{25EF}", glyph_style)));
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("no session selected", primary)));
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "type to filter, or clear with Esc",
        secondary,
    )));
    let p = Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(p, area);
}

fn render_header(f: &mut Frame<'_>, area: Rect, session: &Session, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header row: project + short id
            Constraint::Length(1), // title
            Constraint::Length(1), // meta key-value row
            Constraint::Length(1), // meta section divider
        ])
        .split(area);

    // Header: project basename (peach bold) + mauve-bold short id right.
    // Using the project basename rather than the flat "PREVIEW" caption
    // grounds the preview in *which thing* the user is inspecting.
    let row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(12)])
        .split(chunks[0]);

    let project_name: String = session
        .project_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("preview")
        .to_string();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                project_name,
                Style::default()
                    .fg(theme.peach)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        row[0],
    );

    // Session ids are ASCII UUIDs so `.chars().take(8)` happens to be safe
    // here; keep it, but route the length through a byte slice with a guard
    // in case a test fixture ever injects a non-ASCII id.
    let short_id: String = session
        .id
        .chars()
        .take(8)
        .collect::<String>()
        .to_uppercase();
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            short_id,
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(ratatui::layout::Alignment::Right),
        row[1],
    );

    // Title line — primary label in the appropriate weight.
    let title_style = if session.name.is_some() {
        Style::default()
            .fg(theme.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.subtext0)
            .add_modifier(Modifier::ITALIC)
    };
    let title_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(session.display_label().to_string(), title_style),
    ]);
    f.render_widget(Paragraph::new(title_line), chunks[1]);

    // Meta row: key-value pairs with keys in subtext1 and values in
    // theme.text bold. Reads as a "spec sheet" for the session.
    let meta = meta_line(session, theme);
    f.render_widget(Paragraph::new(meta), chunks[2]);

    // Section divider: `─── meta ───` — tags what the header block was.
    f.render_widget(
        Paragraph::new(section_divider_line(
            "last message",
            area.width,
            theme,
        )),
        chunks[3],
    );
}

/// Build a `─── label ───` divider line — dim rules flanking a subtle label
/// so the preview pane reads as a stack of titled sections rather than a
/// flat stream of text.
fn section_divider_line<'a>(label: &str, pane_width: u16, theme: &Theme) -> Line<'a> {
    // Total width budget: pane width minus the 1-col leading space.
    let budget = pane_width.saturating_sub(2) as usize;
    let label_w = display_width(label) + 2; // padding around label
    let total_rule = budget.saturating_sub(label_w);
    let left_rule = total_rule / 2;
    let right_rule = total_rule - left_rule;
    let left = "\u{2500}".repeat(left_rule.max(3));
    let right = "\u{2500}".repeat(right_rule.max(3));
    Line::from(vec![
        Span::raw(" "),
        Span::styled(left, theme.dim()),
        Span::raw(" "),
        Span::styled(
            label.to_string(),
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(right, theme.dim()),
    ])
}

/// Build the meta row as a compact key-value sheet:
/// `created MMM DD HH:MM · msgs 128 · first Nm ago`. Keys in subtext1, values
/// in theme.text bold, separators in theme.dim.
fn meta_line<'a>(session: &'a Session, theme: &Theme) -> Line<'a> {
    let created = session
        .first_timestamp
        .map(|ts| ts.with_timezone(&Local).format("%b %d %H:%M").to_string())
        .unwrap_or_else(|| "\u{2014}".to_string());
    let key_style = Style::default().fg(theme.subtext1);
    let val_style = Style::default().fg(theme.text).add_modifier(Modifier::BOLD);
    let sep = Span::styled("  \u{00B7}  ", theme.dim());
    Line::from(vec![
        Span::raw(" "),
        Span::styled("created ", key_style),
        Span::styled(created, val_style),
        sep.clone(),
        Span::styled("msgs ", key_style),
        Span::styled(session.message_count.to_string(), val_style),
    ])
}

fn render_body(f: &mut Frame<'_>, area: Rect, session: &Session, theme: &Theme) {
    let exchanges = load_preview(session);
    if exchanges.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("  (no readable messages)", theme.muted()),
        ]);
        f.render_widget(p, area);
        return;
    }

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(exchanges.len() * 3);
    for ex in exchanges {
        let (label, label_style) = match ex.role {
            Role::User => (
                "user",
                Style::default().fg(theme.blue).add_modifier(Modifier::BOLD),
            ),
            Role::Assistant => (
                "claude",
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        };
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(label, label_style),
            Span::raw("  "),
            Span::styled(ex.body, theme.body()),
        ]));
        // Blank spacer between exchanges — matches the Rich-based preview.
        lines.push(Line::raw(""));
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn render_footer(f: &mut Frame<'_>, area: Rect, session: &Session, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Labeled section divider: `─── stats ───` so the footer row reads as a
    // distinct spec-sheet block rather than a tail caption.
    f.render_widget(
        Paragraph::new(section_divider_line("stats", area.width, theme)),
        chunks[0],
    );

    // Stats row: msgs · tokens · model · cost
    let token_total = session.tokens.total();
    let tokens = if token_total >= 1_000 {
        format!("{:.1}k", token_total as f64 / 1000.0)
    } else {
        format!("{token_total}")
    };
    let cost = if session.total_cost_usd < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${:.2}", session.total_cost_usd)
    };

    let fam = crate::data::pricing::family(&session.model_summary);

    let mut spans = vec![
        Span::raw(" "),
        Span::styled(format!("msgs {}", session.message_count), theme.muted()),
        Span::styled("  ·  ", theme.dim()),
        Span::styled(format!("tokens {tokens}"), theme.muted()),
        Span::styled("  ·  ", theme.dim()),
    ];
    spans.push(model_pill::pill(fam, theme));
    // Permission-mode pill right after the model pill, when interesting.
    if let Some(mode) = session.permission_mode {
        if let Some(pill) = model_pill::permission_pill(mode, theme) {
            spans.push(Span::raw(" "));
            spans.push(pill);
        }
    }
    // Cost colour: heat-mapped on the value so the footer stat echoes the
    // session-list column's visual language. Bold so it reads as a number
    // regardless of magnitude.
    let cost_fg = if session.total_cost_usd <= 0.0 {
        theme.subtext1
    } else {
        theme::cost_color(theme, session.total_cost_usd)
    };
    spans.extend([
        Span::styled("  ·  ", theme.dim()),
        Span::styled(
            format!("cost {cost}"),
            Style::default().fg(cost_fg).add_modifier(Modifier::BOLD),
        ),
    ]);
    // Subagent count appended at the tail so the cost stays the last
    // high-signal piece. Keeps the ◈ N marker from overshadowing.
    if session.subagent_count > 0 {
        spans.push(Span::styled("  ·  ", theme.dim()));
        spans.push(Span::styled(
            format!("◈ {} subagents", session.subagent_count),
            Style::default().fg(theme.teal).add_modifier(Modifier::BOLD),
        ));
    }

    // v3.0: turn-duration summary (feature #14). Inline on wide terminals,
    // word-wraps onto a second row on narrow ones — Paragraph::wrap handles it.
    if !session.turn_durations.is_empty() {
        let (avg, max_t, total) = turn_duration_summary(&session.turn_durations);
        spans.extend([
            Span::styled("  ·  ", theme.dim()),
            Span::styled("turns", theme.muted()),
            Span::styled(" avg ", theme.dim()),
            Span::styled(
                crate::ui::stats::format_duration_short(avg),
                Style::default()
                    .fg(theme.lavender)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · max ", theme.dim()),
            Span::styled(
                crate::ui::stats::format_duration_short(max_t),
                Style::default()
                    .fg(theme.peach)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · total ", theme.dim()),
            Span::styled(
                crate::ui::stats::format_duration_short(total),
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true }),
        chunks[1],
    );
}

/// Summarise turn durations as `(avg, max, total)`.
fn turn_duration_summary(
    durations: &[std::time::Duration],
) -> (
    std::time::Duration,
    std::time::Duration,
    std::time::Duration,
) {
    let total: std::time::Duration = durations
        .iter()
        .copied()
        .fold(std::time::Duration::ZERO, |a, b| a.saturating_add(b));
    let avg = if durations.is_empty() {
        std::time::Duration::ZERO
    } else {
        total / durations.len() as u32
    };
    let max_t = durations
        .iter()
        .copied()
        .max()
        .unwrap_or(std::time::Duration::ZERO);
    (avg, max_t, total)
}

/// Shape of a message we want to render in the preview.
struct Exchange {
    role: Role,
    body: String,
}

enum Role {
    User,
    Assistant,
}

/// Stream the session JSONL and pull up to [`MAX_MESSAGES`] qualifying
/// exchanges from the *tail*. We walk the whole file (sessions are generally
/// small; and random-access tail-reading on JSONL is ugly), but keep only the
/// last N entries in a ring.
fn load_preview(session: &Session) -> Vec<Exchange> {
    let path = jsonl_path_for(session);
    let Some(path) = path else {
        return Vec::new();
    };
    let Ok(file) = File::open(&path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);

    let mut ring: Vec<Exchange> = Vec::with_capacity(MAX_MESSAGES);

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let raw: Result<RawLine, _> = serde_json::from_str(trimmed);
        let Ok(raw) = raw else { continue };
        let Some(kind) = raw.kind.as_deref() else {
            continue;
        };
        let role = match (kind, raw.message.as_ref().and_then(|m| m.role.as_deref())) {
            ("user", Some("user")) => Role::User,
            ("assistant", Some("assistant")) => Role::Assistant,
            _ => continue,
        };
        let Some(msg) = raw.message.as_ref() else {
            continue;
        };
        let body = first_text(msg);
        if body.is_empty() || is_noise(&body) {
            continue;
        }
        let body = clean_body(&body);

        if ring.len() == MAX_MESSAGES {
            // Pop the oldest. Vec::remove(0) is fine for N=6.
            ring.remove(0);
        }
        ring.push(Exchange { role, body });
    }
    ring
}

/// Return the on-disk path for `session`'s JSONL.
fn jsonl_path_for(session: &Session) -> Option<std::path::PathBuf> {
    // Primary: the encoded-dir next to `~/.claude/projects/`.
    let home = dirs::home_dir()?;
    let projects = home.join(".claude").join("projects");
    // We don't always know the encoded_dir directly; derive it from project_dir
    // if the session was loaded from there, otherwise scan by id.
    if projects.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&projects) {
            for entry in entries.flatten() {
                let candidate = entry.path().join(format!("{}.jsonl", session.id));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Raw JSONL record shape — just enough to extract role + text.
#[derive(Deserialize)]
struct RawLine {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<RawMsg>,
}

#[derive(Deserialize)]
struct RawMsg {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<RawContent>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawContent {
    Text(String),
    Blocks(Vec<RawBlock>),
}

#[derive(Deserialize)]
struct RawBlock {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

fn first_text(msg: &RawMsg) -> String {
    let Some(c) = &msg.content else {
        return String::new();
    };
    match c {
        RawContent::Text(s) => s.trim().to_string(),
        RawContent::Blocks(blocks) => {
            for b in blocks {
                if b.kind.as_deref() == Some("text") {
                    if let Some(t) = b.text.as_deref() {
                        let t = t.trim();
                        if !t.is_empty() {
                            return t.to_string();
                        }
                    }
                }
            }
            String::new()
        }
    }
}

fn is_noise(s: &str) -> bool {
    // Tiny-message filter: measure in columns so a 2-char CJK sentinel like
    // "はい" isn't mislabeled as noise. 3 columns = about one CJK glyph or
    // three ASCII chars.
    if display_width(s) <= 3 {
        return true;
    }
    for prefix in noise_prefixes() {
        if s.contains(prefix) {
            return true;
        }
    }
    false
}

fn clean_body(s: &str) -> String {
    // Flatten newlines so wrapping rules fire at the Paragraph level and we
    // don't accidentally break at a bad spot ourselves.
    let flat: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let trimmed = flat.trim();
    // Column-aware truncation — MAX_BODY_COLS is measured in terminal cells,
    // not chars. Grapheme-safe via the shared `truncate_to_width` helper.
    truncate_to_width(trimmed, MAX_BODY_COLS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn turn_duration_summary_averages_and_sums() {
        // 10 + 30 + 60 = 100s total. Avg = 100s / 3 = 33.333…s — `Duration`
        // division preserves sub-second precision, not truncated.
        let d = vec![
            Duration::from_secs(10),
            Duration::from_secs(30),
            Duration::from_secs(60),
        ];
        let (avg, max_t, total) = turn_duration_summary(&d);
        assert_eq!(avg, Duration::from_secs(100) / 3);
        assert_eq!(max_t, Duration::from_secs(60));
        assert_eq!(total, Duration::from_secs(100));
    }

    #[test]
    fn turn_duration_summary_empty_is_zero() {
        let (avg, max_t, total) = turn_duration_summary(&[]);
        assert_eq!(avg, Duration::ZERO);
        assert_eq!(max_t, Duration::ZERO);
        assert_eq!(total, Duration::ZERO);
    }

    #[test]
    fn noise_detection() {
        assert!(is_noise("<bash-in>ls"));
        assert!(is_noise("<system-reminder>foo"));
        assert!(is_noise("hi"));
        assert!(!is_noise("please refactor this file"));
    }

    #[test]
    fn clean_body_truncates_and_flattens() {
        let raw = "line1\nline2\n";
        assert_eq!(clean_body(raw), "line1 line2");
        let long = "x".repeat(500);
        let cleaned = clean_body(&long);
        assert!(cleaned.ends_with('…'));
        // Column-accurate check: 239 x's + ellipsis = 240 cols exactly.
        assert_eq!(display_width(&cleaned), MAX_BODY_COLS);
    }

    #[test]
    fn clean_body_handles_cjk_within_column_budget() {
        // 200 CJK chars = 400 cols; should truncate to fit under MAX_BODY_COLS.
        let long: String = "あ".repeat(200);
        let cleaned = clean_body(&long);
        assert!(
            display_width(&cleaned) <= MAX_BODY_COLS,
            "cleaned width {}: {}",
            display_width(&cleaned),
            cleaned,
        );
    }

    // ── Preview-cmd tests ───────────────────────────────────────────────

    use std::path::PathBuf;

    fn mk_test_session(id: &str) -> Session {
        use crate::data::pricing::TokenCounts;
        use crate::data::session::SessionKind;
        Session {
            id: id.to_string(),
            project_dir: PathBuf::from("/tmp/demo"),
            name: None,
            auto_name: None,
            last_prompt: None,
            message_count: 4,
            tokens: TokenCounts::default(),
            total_cost_usd: 0.0,
            model_summary: "claude-opus-4-7".to_string(),
            first_timestamp: None,
            last_timestamp: None,
            is_fork: false,
            forked_from: None,
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
            turn_durations: Vec::new(),
        }
    }

    #[test]
    fn substitute_placeholders_replaces_sid_and_cwd() {
        let t = substitute_placeholders(
            "cat {cwd}/logs/{sid}.jsonl | head",
            "abc123",
            Path::new("/tmp/demo"),
        );
        assert_eq!(t, "cat /tmp/demo/logs/abc123.jsonl | head");
    }

    #[test]
    fn substitute_placeholders_leaves_unknown_braces_alone() {
        let t = substitute_placeholders("echo {sid} {other}", "sid-1", Path::new("/cwd"));
        assert_eq!(t, "echo sid-1 {other}");
    }

    #[test]
    fn execute_preview_command_captures_stdout_on_success() {
        let sess = mk_test_session("my-session-1");
        let out = execute_preview_command("echo hello from {sid}", &sess);
        assert_eq!(out.kind, PreviewKind::Ok);
        assert!(out.body.contains("hello from my-session-1"));
    }

    #[test]
    fn execute_preview_command_flags_empty_as_empty() {
        let sess = mk_test_session("s");
        let out = execute_preview_command("true", &sess);
        assert_eq!(out.kind, PreviewKind::Empty);
    }

    #[test]
    fn execute_preview_command_flags_nonzero_as_failed() {
        let sess = mk_test_session("s");
        let out = execute_preview_command("false", &sess);
        assert_eq!(out.kind, PreviewKind::Failed);
    }

    #[test]
    fn execute_preview_command_captures_stderr_on_failure() {
        let sess = mk_test_session("s");
        let out = execute_preview_command("echo 'bad things' >&2; exit 2", &sess);
        assert_eq!(out.kind, PreviewKind::Failed);
        assert!(out.body.contains("bad things"));
    }

    #[test]
    fn preview_cache_memoises_across_calls() {
        let cache = PreviewCache::new();
        assert!(cache.is_empty());
        let sess = mk_test_session("cached-session");

        let mut n_runs = 0u32;
        for _ in 0..3 {
            cache.get_or_insert(&sess, |_| {
                n_runs += 1;
                PreviewOutput {
                    body: "cached".into(),
                    kind: PreviewKind::Ok,
                }
            });
        }

        // First call ran the closure; next two hit the cache.
        assert_eq!(n_runs, 1);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn preview_cache_keys_by_session_id() {
        let cache = PreviewCache::new();
        let a = mk_test_session("a");
        let b = mk_test_session("b");

        let mut run_count = 0u32;
        cache.get_or_insert(&a, |_| {
            run_count += 1;
            PreviewOutput {
                body: "a".into(),
                kind: PreviewKind::Ok,
            }
        });
        cache.get_or_insert(&b, |_| {
            run_count += 1;
            PreviewOutput {
                body: "b".into(),
                kind: PreviewKind::Ok,
            }
        });
        cache.get_or_insert(&a, |_| {
            run_count += 1;
            PreviewOutput {
                body: "a again".into(),
                kind: PreviewKind::Ok,
            }
        });

        // a + b + a-cached → 2 runs, 2 entries.
        assert_eq!(run_count, 2);
        assert_eq!(cache.len(), 2);
    }
}
