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

use std::fs::File;
use std::io::{BufRead, BufReader};

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
use crate::theme::Theme;
use crate::ui::model_pill;

/// How many exchanges to show. Matches the brief (4-6) — 6 is usually what
/// fits on a 40-row terminal after header / footer / meta.
const MAX_MESSAGES: usize = 6;

/// Maximum characters per message body before we truncate with ` …`. Keeps
/// each entry readable as a one-glance summary rather than a wall of text.
const MAX_BODY_CHARS: usize = 240;

/// Render the preview pane into `area`.
///
/// If no session is currently selected — e.g. the filter matched zero rows —
/// falls back to a "nothing to preview" placeholder.
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

fn placeholder(f: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let p = Paragraph::new(vec![
        Line::raw(""),
        Line::raw(""),
        Line::styled("no session selected", theme.muted()),
        Line::raw(""),
        Line::styled("type to filter, or clear with Esc", theme.dim()),
    ])
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(p, area);
}

fn render_header(f: &mut Frame<'_>, area: Rect, session: &Session, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header row
            Constraint::Length(1), // title
            Constraint::Length(1), // meta
            Constraint::Length(1), // rule
        ])
        .split(area);

    // Header: "PREVIEW" left, short id right. We split the row so the left
    // and right paragraphs can align independently — trying to right-align
    // a tail span inside a single Line isn't supported cleanly.
    let row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(10)])
        .split(chunks[0]);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "PREVIEW",
                Style::default()
                    .fg(theme.overlay0)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
        row[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            session
                .id
                .chars()
                .take(8)
                .collect::<String>()
                .to_uppercase(),
            theme.muted(),
        )))
        .alignment(ratatui::layout::Alignment::Right),
        row[1],
    );

    // Title.
    let title_style = if session.name.is_some() {
        Style::default()
            .fg(theme.green)
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

    // Meta.
    let meta = meta_line(session, theme);
    f.render_widget(Paragraph::new(meta), chunks[2]);

    // Rule.
    let rule_width = area.width.saturating_sub(2) as usize;
    let rule = "─".repeat(rule_width);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(rule, theme.dim()),
        ])),
        chunks[3],
    );
}

fn meta_line<'a>(session: &'a Session, theme: &Theme) -> Line<'a> {
    let created = session
        .first_timestamp
        .map(|ts| ts.with_timezone(&Local).format("%b %d %H:%M").to_string())
        .unwrap_or_else(|| "—".to_string());
    Line::from(vec![
        Span::raw(" "),
        Span::styled(format!("created {created}"), theme.muted()),
        Span::styled("  ·  ", theme.dim()),
        Span::styled(format!("{} msgs", session.message_count), theme.muted()),
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

    let rule_width = area.width.saturating_sub(2) as usize;
    let rule = "─".repeat(rule_width);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(rule, theme.dim()),
        ])),
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
    spans.extend([
        Span::styled("  ·  ", theme.dim()),
        Span::styled(
            format!("cost {cost}"),
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
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
    f.render_widget(
        Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true }),
        chunks[1],
    );
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
    if s.chars().count() <= 3 {
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
    if trimmed.chars().count() <= MAX_BODY_CHARS {
        return trimmed.to_string();
    }
    let mut out = String::with_capacity(MAX_BODY_CHARS * 4);
    for (i, ch) in trimmed.chars().enumerate() {
        if i == MAX_BODY_CHARS {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(cleaned.chars().count(), MAX_BODY_CHARS + 1);
    }
}
