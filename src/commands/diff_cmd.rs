//! `claude-picker diff` — interactive two-session comparison.
//!
//! Flow:
//!
//! 1. Aggregate every session under `~/.claude/projects/`.
//! 2. Two sequential pick steps (mini picker — single pane, filter + list).
//! 3. Topic extraction (tokenize → stopword prune → word & bigram frequency).
//! 4. Render the diff screen (see [`crate::ui::diff`]) and loop for keys.
//!
//! We intentionally do NOT reuse [`crate::app::App`] for the picker steps: the
//! main App is tightly coupled to project-list + session-list modes, and
//! retrofitting a "pick one session and return" flow would add conditionals
//! throughout. A purpose-built 150-line mini-picker is cleaner.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Stdout};
use std::path::{Path, PathBuf};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use ratatui::{Frame, Terminal};
use serde::Deserialize;

use crate::data::session::{load_session_from_jsonl, noise_prefixes};
use crate::data::{project, Session};
use crate::events::{self, Event};
use crate::theme::Theme;
use crate::ui::diff::{self, DiffData, Role};

/// Entry point. Drives pick-A → pick-B → render-diff → cleanup.
pub fn run() -> anyhow::Result<()> {
    let sessions = discover_all_sessions()?;
    if sessions.len() < 2 {
        eprintln!(
            "claude-picker diff: need at least 2 sessions, found {}",
            sessions.len()
        );
        return Ok(());
    }

    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let outcome: anyhow::Result<()> = (|| {
        // Step 1: pick A.
        let Some(idx_a) = run_picker(
            &mut terminal,
            &sessions,
            "session diff — pick FIRST session to compare",
            None,
        )?
        else {
            return Ok(());
        };

        // Step 2: pick B (can match A; we just show a toast-like note but still allow it).
        let Some(idx_b) = run_picker(
            &mut terminal,
            &sessions,
            "session diff — pick SECOND session",
            Some(idx_a),
        )?
        else {
            return Ok(());
        };

        let mut data = build_diff_data(&sessions[idx_a], &sessions[idx_b]);
        run_diff_screen(&mut terminal, &mut data)?;
        Ok(())
    })();

    let _ = restore_terminal(&mut terminal);
    outcome
}

// ── Session discovery ────────────────────────────────────────────────────

/// Load every picker-visible session across every project. Sorted by recency,
/// newest first.
fn discover_all_sessions() -> anyhow::Result<Vec<Session>> {
    let projects = project::discover_projects()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let base = home.join(".claude").join("projects");

    let mut out: Vec<Session> = Vec::new();
    for p in &projects {
        let dir = base.join(&p.encoded_dir);
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            match load_session_from_jsonl(&path, p.path.clone()) {
                Ok(Some(s)) => out.push(s),
                Ok(None) => {}
                Err(e) => eprintln!("{}: load error: {e}", path.display()),
            }
        }
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.last_timestamp));
    Ok(out)
}

// ── Mini picker ──────────────────────────────────────────────────────────

/// Lightweight single-pane picker. Keeps its own filter buffer + cursor; does
/// substring match case-insensitively.
///
/// Returns `Ok(Some(index))` on Enter, `Ok(None)` on Esc / q / Ctrl-C.
fn run_picker(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    sessions: &[Session],
    heading: &str,
    highlight_idx: Option<usize>,
) -> anyhow::Result<Option<usize>> {
    let theme = Theme::mocha();
    let mut filter = String::new();
    let mut cursor: usize = 0;
    let mut filtered: Vec<usize> = (0..sessions.len()).collect();

    loop {
        terminal.draw(|f| {
            render_picker(
                f,
                sessions,
                &filtered,
                &filter,
                cursor,
                heading,
                highlight_idx,
                &theme,
            );
        })?;

        let Some(ev) = events::next()? else {
            continue;
        };

        match ev {
            Event::Quit | Event::Ctrl('c') => return Ok(None),
            Event::Escape => {
                if !filter.is_empty() {
                    filter.clear();
                    filtered = apply_picker_filter(sessions, &filter);
                    cursor = 0;
                } else {
                    return Ok(None);
                }
            }
            Event::Enter => {
                if let Some(&idx) = filtered.get(cursor) {
                    return Ok(Some(idx));
                }
            }
            Event::Up if !filtered.is_empty() => {
                cursor = if cursor == 0 {
                    filtered.len() - 1
                } else {
                    cursor - 1
                };
            }
            Event::Down if !filtered.is_empty() => {
                cursor = (cursor + 1) % filtered.len();
            }
            Event::PageUp => {
                cursor = cursor.saturating_sub(10);
            }
            Event::PageDown if !filtered.is_empty() => {
                cursor = (cursor + 10).min(filtered.len() - 1);
            }
            Event::Home => cursor = 0,
            Event::End => cursor = filtered.len().saturating_sub(1),
            Event::Backspace => {
                filter.pop();
                filtered = apply_picker_filter(sessions, &filter);
                cursor = 0;
            }
            Event::Key(c) if c == 'q' && filter.is_empty() => return Ok(None),
            Event::Key(c) if is_filter_char(c) => {
                filter.push(c);
                filtered = apply_picker_filter(sessions, &filter);
                cursor = 0;
            }
            _ => {}
        }
    }
}

fn apply_picker_filter(sessions: &[Session], filter: &str) -> Vec<usize> {
    if filter.is_empty() {
        return (0..sessions.len()).collect();
    }
    let needle = filter.to_lowercase();
    sessions
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let label = s.display_label().to_lowercase();
            let id = s.id.to_lowercase();
            if label.contains(&needle) || id.contains(&needle) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

fn is_filter_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '/' | '@')
}

#[allow(clippy::too_many_arguments)]
fn render_picker(
    f: &mut Frame<'_>,
    sessions: &[Session],
    filtered: &[usize],
    filter: &str,
    cursor: usize,
    heading: &str,
    highlight_idx: Option<usize>,
    theme: &Theme,
) {
    let area = f.area();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    // Title bar.
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            heading.to_string(),
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(title), rows[0]);

    // Main panel.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border_active())
        .title_top(
            Line::from(Span::styled(
                format!(" {}/{} ", filtered.len(), sessions.len()),
                Style::default().fg(theme.subtext1),
            ))
            .right_aligned(),
        );
    let inner = block.inner(rows[1]);
    f.render_widget(block, rows[1]);

    let inner_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    render_filter(f, inner_rows[0], filter, theme);
    render_list(
        f,
        inner_rows[1],
        sessions,
        filtered,
        cursor,
        highlight_idx,
        theme,
    );

    // Footer.
    let hints = [
        ("↑↓", "navigate"),
        ("Enter", "pick"),
        ("a-z", "filter"),
        ("Esc", "back"),
        ("q", "quit"),
    ];
    let mut spans: Vec<Span> = Vec::with_capacity(hints.len() * 4);
    spans.push(Span::raw("  "));
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", theme.dim()));
        }
        spans.push(Span::styled((*key).to_string(), theme.key_hint()));
        spans.push(Span::raw(" "));
        spans.push(Span::styled((*desc).to_string(), theme.key_desc()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), rows[2]);
}

fn render_filter(f: &mut Frame<'_>, area: Rect, filter: &str, theme: &Theme) {
    let text: Line<'_> = if filter.is_empty() {
        Line::from(vec![
            Span::styled("> ", theme.muted()),
            Span::styled("type to filter sessions…", theme.filter_placeholder()),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", theme.muted()),
            Span::styled(filter.to_string(), theme.filter_text()),
            Span::styled(" ", Style::default().bg(theme.mauve).fg(theme.crust)),
        ])
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.surface1));
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn render_list(
    f: &mut Frame<'_>,
    area: Rect,
    sessions: &[Session],
    filtered: &[usize],
    cursor: usize,
    highlight_idx: Option<usize>,
    theme: &Theme,
) {
    if sessions.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("No sessions found.", theme.muted()),
        ])
        .alignment(Alignment::Center);
        f.render_widget(p, area);
        return;
    }
    if filtered.is_empty() {
        let p = Paragraph::new(vec![
            Line::raw(""),
            Line::styled("No matches.", theme.muted()),
        ])
        .alignment(Alignment::Center);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem<'_>> = filtered
        .iter()
        .enumerate()
        .map(|(display_idx, &sess_idx)| {
            let s = &sessions[sess_idx];
            let is_selected = display_idx == cursor;
            let is_already_picked = highlight_idx == Some(sess_idx);
            ListItem::new(render_row(s, theme, is_selected, is_already_picked))
        })
        .collect();

    let list = List::new(items).highlight_symbol("");
    let mut state = ListState::default();
    state.select(Some(cursor.min(filtered.len().saturating_sub(1))));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_row<'a>(s: &'a Session, theme: &Theme, selected: bool, already_picked: bool) -> Line<'a> {
    let pointer = if selected { "▸" } else { " " };
    let pointer_style = if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface2)
    };

    let name_style = if selected {
        theme.selected_row()
    } else if already_picked {
        Style::default()
            .fg(theme.green)
            .add_modifier(Modifier::BOLD)
    } else if s.name.is_some() {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.subtext0)
            .add_modifier(Modifier::ITALIC)
    };

    let name = truncate_chars(s.display_label(), 34);
    let id_short: String = s.id.chars().take(8).collect();

    let pick_tag = if already_picked {
        Span::styled(" (A) ", Style::default().fg(theme.green))
    } else {
        Span::raw("     ")
    };

    let mut spans = vec![
        Span::styled(format!(" {pointer} "), pointer_style),
        Span::styled(pad_right(&name, 34), name_style),
        Span::raw(" "),
        Span::styled(id_short, theme.muted()),
        pick_tag,
        Span::styled(format!("{} msgs", s.message_count), theme.muted()),
    ];

    if selected {
        for span in &mut spans {
            span.style.bg = Some(theme.surface0);
        }
    }
    Line::from(spans)
}

fn pad_right(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count >= width {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + (width - count));
    out.push_str(s);
    for _ in 0..(width - count) {
        out.push(' ');
    }
    out
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    let mut out = String::with_capacity(max_chars * 4);
    for (i, ch) in s.chars().enumerate() {
        if i == max_chars - 1 {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

// ── Diff screen event loop ───────────────────────────────────────────────

/// Max vertical lines the preview columns can scroll before we clamp. Since
/// we render at most a handful of exchanges with ~3 lines each, 60 is ample.
const MAX_SCROLL: usize = 60;

fn run_diff_screen(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    data: &mut DiffData,
) -> anyhow::Result<()> {
    let theme = Theme::mocha();
    let mut show_help = false;
    loop {
        terminal.draw(|f| {
            diff::render(f, f.area(), data, &theme);
            if show_help {
                let content =
                    crate::ui::help_overlay::help_for(crate::ui::help_overlay::Screen::Diff);
                crate::ui::help_overlay::render(f, f.area(), content, &theme);
            }
        })?;

        let Some(ev) = events::next()? else { continue };

        if show_help {
            match ev {
                Event::Escape => show_help = false,
                Event::Key(c) if crate::ui::help_overlay::is_dismiss_key(c) => {
                    show_help = false;
                }
                _ => {}
            }
            continue;
        }

        match ev {
            Event::Quit | Event::Ctrl('c') | Event::Escape => return Ok(()),
            Event::Key('q') => return Ok(()),
            Event::Key('?') => show_help = true,
            Event::Up | Event::Key('k') => data.scroll_by(-1, MAX_SCROLL),
            Event::Down | Event::Key('j') => data.scroll_by(1, MAX_SCROLL),
            Event::PageUp => data.scroll_by(-10, MAX_SCROLL),
            Event::PageDown => data.scroll_by(10, MAX_SCROLL),
            Event::Home => data.scroll_offset = 0,
            Event::End => data.scroll_offset = MAX_SCROLL,
            Event::Tab => data.focus_right = !data.focus_right,
            Event::Key('s') | Event::Key('S') => data.swap(),
            // `d` toggles the word-level diff renderer. Delta-style inline
            // highlights replace the two-column body when on.
            Event::Key('d') | Event::Key('D') => data.toggle_word_mode(),
            // `n` / `N`: chunk-jump between hunks (delta-style). Each
            // exchange pair is one hunk; stride is kept inside
            // `DiffData::jump_hunk` so both side-by-side and word-diff
            // modes agree on what "next pair" means.
            Event::Key('n') => data.jump_hunk(1, MAX_SCROLL),
            Event::Key('N') => data.jump_hunk(-1, MAX_SCROLL),
            _ => {}
        }
    }
}

// ── Diff data construction ───────────────────────────────────────────────

/// Build everything the diff renderer needs from two sessions: topics
/// (common / unique-A / unique-B) and tail conversation excerpts for each.
pub fn build_diff_data(a: &Session, b: &Session) -> DiffData {
    let extract_a = extract_session_text(a);
    let extract_b = extract_session_text(b);

    let topics_a = extract_topics(&extract_a.texts, 15);
    let topics_b = extract_topics(&extract_b.texts, 15);
    let set_a: HashSet<&String> = topics_a.iter().collect();
    let set_b: HashSet<&String> = topics_b.iter().collect();

    // Preserve insertion order — topics_* are already ranked by frequency.
    let topics_common: Vec<String> = topics_a
        .iter()
        .filter(|t| set_b.contains(*t))
        .cloned()
        .collect();
    let topics_unique_a: Vec<String> = topics_a
        .iter()
        .filter(|t| !set_b.contains(*t))
        .cloned()
        .collect();
    let topics_unique_b: Vec<String> = topics_b
        .iter()
        .filter(|t| !set_a.contains(*t))
        .cloned()
        .collect();

    DiffData {
        session_a: a.clone(),
        session_b: b.clone(),
        preview_a: extract_a.preview,
        preview_b: extract_b.preview,
        topics_common,
        topics_unique_a,
        topics_unique_b,
        scroll_offset: 0,
        focus_right: false,
        word_mode: false,
    }
}

// ── Topic extraction ─────────────────────────────────────────────────────

/// Stopword list ported from `lib/session-diff.py`. Lowercase, 3+ chars only
/// (the tokenizer already drops <3-char words).
const STOP_WORDS: &[&str] = &[
    "the", "was", "were", "been", "being", "have", "has", "had", "does", "did", "will", "would",
    "could", "should", "may", "might", "shall", "can", "need", "dare", "ought", "used", "for",
    "with", "into", "through", "during", "before", "after", "above", "below", "between", "out",
    "off", "over", "under", "again", "further", "then", "once", "here", "there", "when", "where",
    "why", "how", "all", "both", "each", "few", "more", "most", "other", "some", "such", "nor",
    "not", "only", "own", "same", "than", "too", "very", "just", "because", "but", "and", "while",
    "about", "what", "which", "who", "whom", "this", "that", "these", "those", "its", "your",
    "his", "her", "our", "their", "him", "them", "you", "she", "they", "also", "like", "make",
    "get", "got", "let", "want", "know", "think", "see", "look", "use", "file", "code", "please",
    "sure", "okay", "yes", "right", "well", "don", "doesn", "didn", "won", "wouldn", "couldn",
    "shouldn", "hasn", "haven", "wasn", "weren", "isn", "aren", "now", "going", "using", "new",
    "one", "two", "first", "way", "work", "are", "add", "run", "set", "way", "put", "yep", "nope",
    "kind", "yeah", "still", "back",
];

/// Produce a list of up to `top_n` topic tokens for a bag of lowercase text
/// snippets. Mixes single words and bigrams: bigrams that appear ≥2 times
/// outrank single words by a 2× weight, matching the Python implementation.
pub fn extract_topics(texts: &[String], top_n: usize) -> Vec<String> {
    if texts.is_empty() {
        return Vec::new();
    }
    let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();

    let mut word_freq: HashMap<String, u32> = HashMap::new();
    let mut bigram_freq: HashMap<String, u32> = HashMap::new();

    for text in texts {
        let tokens: Vec<&str> = tokens_of(text, &stop);
        for t in &tokens {
            *word_freq.entry(t.to_string()).or_default() += 1;
        }
        for pair in tokens.windows(2) {
            let bg = format!("{} {}", pair[0], pair[1]);
            *bigram_freq.entry(bg).or_default() += 1;
        }
    }

    let mut topics: Vec<(String, u32)> = Vec::new();

    // Top bigrams with count ≥ 2, weighted 2×.
    let mut bigrams: Vec<(String, u32)> = bigram_freq.into_iter().collect();
    bigrams.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut seen_words: HashSet<String> = HashSet::new();
    for (bg, count) in bigrams.into_iter().take(top_n * 2) {
        if count < 2 {
            break;
        }
        for w in bg.split_whitespace() {
            seen_words.insert(w.to_string());
        }
        topics.push((bg, count * 2));
    }

    // Then unigrams (count ≥ 2) that aren't already inside a chosen bigram.
    let mut unigrams: Vec<(String, u32)> = word_freq.into_iter().collect();
    unigrams.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (word, count) in unigrams.into_iter().take(top_n * 3) {
        if count < 2 || seen_words.contains(&word) {
            continue;
        }
        topics.push((word, count));
    }

    // Rank by weighted count desc, ties broken alphabetically for determinism.
    topics.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    topics.into_iter().take(top_n).map(|(t, _)| t).collect()
}

/// Tokenize: lowercase, keep `[a-z][a-z0-9_]{2,}` runs, drop stopwords.
fn tokens_of<'a>(text: &'a str, stop: &HashSet<&str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Find start: must be ASCII lowercase a-z.
        if !bytes[i].is_ascii_lowercase() {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i + 1;
        while j < bytes.len() {
            let c = bytes[j];
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' {
                j += 1;
            } else {
                break;
            }
        }
        // Length check: `[a-z][a-z0-9_]{2,}` = 3+ total chars.
        if j - start >= 3 {
            // Safe: we've only walked ASCII bytes, so the slice is valid utf-8.
            let tok = std::str::from_utf8(&bytes[start..j]).unwrap_or("");
            if !tok.is_empty() && !stop.contains(tok) {
                out.push(tok);
            }
        }
        i = j.max(i + 1);
    }
    out
}

// ── Session text extraction ──────────────────────────────────────────────

/// How many tail exchanges to surface in the preview band.
const PREVIEW_COUNT: usize = 5;
/// Cap each preview body so one message doesn't monopolise the column.
const PREVIEW_CHAR_CAP: usize = 280;

struct SessionText {
    /// Full lowercase text corpus (user + assistant) for topic mining.
    texts: Vec<String>,
    /// Tail conversation excerpt, oldest → newest.
    preview: Vec<(Role, String)>,
}

/// Stream a session's JSONL once, collecting both the topic corpus and the
/// tail preview. Noise-filtered identically to the Python reference.
fn extract_session_text(session: &Session) -> SessionText {
    let Some(path) = jsonl_path_for(session) else {
        return SessionText {
            texts: Vec::new(),
            preview: Vec::new(),
        };
    };
    let Ok(file) = File::open(&path) else {
        return SessionText {
            texts: Vec::new(),
            preview: Vec::new(),
        };
    };
    let reader = BufReader::new(file);

    let mut texts: Vec<String> = Vec::new();
    let mut ring: Vec<(Role, String)> = Vec::with_capacity(PREVIEW_COUNT);

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let raw: RawLine = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let Some(kind) = raw.kind.as_deref() else {
            continue;
        };
        let role = match (kind, raw.message.as_ref().and_then(|m| m.role.as_deref())) {
            ("user", Some("user")) => Role::User,
            ("assistant", Some("assistant")) => Role::Claude,
            _ => continue,
        };
        let Some(msg) = raw.message.as_ref() else {
            continue;
        };
        let body = first_text(msg);
        if body.is_empty() || is_noise(&body) {
            continue;
        }
        texts.push(body.to_lowercase());

        if ring.len() == PREVIEW_COUNT {
            ring.remove(0);
        }
        ring.push((role, clean_body(&body)));
    }

    SessionText {
        texts,
        preview: ring,
    }
}

/// Locate `session.id`'s JSONL anywhere under `~/.claude/projects/`.
fn jsonl_path_for(session: &Session) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let projects = home.join(".claude").join("projects");
    if !projects.is_dir() {
        return None;
    }
    let Ok(entries) = std::fs::read_dir(&projects) else {
        return None;
    };
    for entry in entries.flatten() {
        let candidate = entry.path().join(format!("{}.jsonl", session.id));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

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
    let flat: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let trimmed = flat.trim();
    if trimmed.chars().count() <= PREVIEW_CHAR_CAP {
        return trimmed.to_string();
    }
    let mut out = String::with_capacity(PREVIEW_CHAR_CAP * 4);
    for (i, ch) in trimmed.chars().enumerate() {
        if i == PREVIEW_CHAR_CAP {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

// ── Terminal plumbing ────────────────────────────────────────────────────

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut stdout = io::stdout();
        let _ = disable_raw_mode();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
        default(info);
    }));
}

// Keep an unused-import guard so `Path` is always referenced — helpful when
// callers tweak jsonl_path_for and silent dead-code suppresses our tests.
#[allow(dead_code)]
fn _path_guard(_p: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_of_extracts_words_and_drops_stopwords() {
        let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();
        let toks = tokens_of("the auth middleware stores session tokens", &stop);
        assert!(!toks.contains(&"the"));
        assert!(toks.contains(&"auth"));
        assert!(toks.contains(&"middleware"));
        assert!(toks.contains(&"session"));
        assert!(toks.contains(&"tokens"));
    }

    #[test]
    fn tokens_of_drops_short_words() {
        let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();
        let toks = tokens_of("a b to go", &stop);
        // "a", "b", "to", "go" — all < 3 chars → dropped.
        assert!(toks.is_empty());
    }

    #[test]
    fn extract_topics_returns_frequent_items() {
        let texts = vec![
            "redis rate limiter for auth middleware".into(),
            "the auth middleware uses redis rate limiter".into(),
            "auth tokens flow through middleware".into(),
        ];
        let topics = extract_topics(&texts, 10);
        assert!(topics.iter().any(|t| t.contains("auth")));
        assert!(topics.iter().any(|t| t.contains("middleware")));
    }

    #[test]
    fn extract_topics_empty_input_returns_empty() {
        assert!(extract_topics(&[], 10).is_empty());
    }

    #[test]
    fn diff_data_common_and_unique_split_correctly() {
        let texts_a = vec![
            "redis rate limiter for session tokens".into(),
            "session tokens flow through redis".into(),
            "rate limiter tuning for tokens".into(),
        ];
        let texts_b = vec![
            "oauth2 provider with session tokens".into(),
            "session tokens via oauth2 callback".into(),
            "oauth2 callback validates tokens".into(),
        ];
        let a = extract_topics(&texts_a, 10);
        let b = extract_topics(&texts_b, 10);
        let set_a: HashSet<&String> = a.iter().collect();
        let set_b: HashSet<&String> = b.iter().collect();

        let common: Vec<&String> = a.iter().filter(|t| set_b.contains(*t)).collect();
        let unique_a: Vec<&String> = a.iter().filter(|t| !set_b.contains(*t)).collect();
        let unique_b: Vec<&String> = b.iter().filter(|t| !set_a.contains(*t)).collect();

        // "tokens" appears in both corpora, so it should be common.
        assert!(common.iter().any(|t| t.contains("tokens")));
        // Redis is unique to A.
        assert!(unique_a.iter().any(|t| t.contains("redis")));
        // oauth2 is unique to B.
        assert!(unique_b.iter().any(|t| t.contains("oauth2")));
    }

    #[test]
    fn bigrams_rank_above_unigrams_when_repeated() {
        let texts = vec![
            "rate limiter and rate limiter again".into(),
            "rate limiter handles bursts".into(),
        ];
        let topics = extract_topics(&texts, 5);
        // "rate limiter" should beat any single-word topic thanks to the 2× weight.
        assert!(
            topics.iter().any(|t| t == "rate limiter"),
            "expected 'rate limiter' bigram in {topics:?}"
        );
    }

    #[test]
    fn is_noise_trips_on_short_and_prefixed() {
        assert!(is_noise("hi"));
        assert!(is_noise("<bash-in>ls -la"));
        assert!(!is_noise("please refactor the auth middleware"));
    }

    #[test]
    fn clean_body_flattens_newlines_and_caps() {
        let raw = "line1\nline2\r\nline3";
        let c = clean_body(raw);
        assert_eq!(c, "line1 line2  line3");
        let long = "x".repeat(PREVIEW_CHAR_CAP + 50);
        let cc = clean_body(&long);
        assert_eq!(cc.chars().count(), PREVIEW_CHAR_CAP + 1);
        assert!(cc.ends_with('…'));
    }

    #[test]
    fn apply_picker_filter_matches_label_and_id_case_insensitive() {
        use crate::data::pricing::TokenCounts;
        use crate::data::session::SessionKind;
        fn mk(id: &str, name: &str) -> Session {
            Session {
                id: id.into(),
                project_dir: PathBuf::from("/tmp"),
                name: Some(name.into()),
                auto_name: None,
                last_prompt: None,
                message_count: 1,
                tokens: TokenCounts::default(),
                total_cost_usd: 0.0,
                model_summary: "claude-opus-4-7".into(),
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
        let sessions = vec![mk("ABC123", "Auth-Refactor"), mk("def456", "db-migration")];
        assert_eq!(apply_picker_filter(&sessions, ""), vec![0, 1]);
        assert_eq!(apply_picker_filter(&sessions, "auth"), vec![0]);
        assert_eq!(apply_picker_filter(&sessions, "DEF"), vec![1]);
        assert_eq!(apply_picker_filter(&sessions, "xyz"), Vec::<usize>::new());
    }
}
