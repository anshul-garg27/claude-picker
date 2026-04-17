//! Session fork-tree widget.
//!
//! Flattens `(projects, sessions)` into a renderable list of [`TreeNode`]s:
//! a project header followed by its sessions, with fork children nested
//! depth-first underneath their parent. The flatten step produces the
//! ASCII-tree metadata each row needs (depth, is-last-child, the bitmap of
//! ancestor "is-last-child" flags) so the renderer can emit the right
//! sequence of `│`, `├─`, `└─`, and space characters without re-walking the
//! tree.
//!
//! The renderer itself is a pure function over the flattened slice: given
//! the list, the cursor index, and the theme, it paints the panel. The
//! event loop is deliberately somewhere else — this module exists to be
//! unit-testable without a terminal.
//!
//! Columns on a session row:
//!
//! ```text
//! ▸ ├─ ● auth-refactor            2h ago   45 msgs    $0.41
//! ```
//!
//! - `▸` — selection cursor (1 col + space).
//! - `├─` / `└─` — tree connector for the session's depth.
//! - `●` / `◆` / `○` — status glyph (named / forked / unnamed).
//! - right-aligned age, msg-count, and cost columns.
//!
//! Wide-terminal centring is handled by [`render`]: the content block caps
//! at 120 columns and is horizontally centred when the frame is wider.

use std::time::Duration;

use chrono::{DateTime, Utc};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::data::{Project, Session};
use crate::theme::{self, Theme};
use crate::ui::text::{display_width, pad_to_width, truncate_to_width};

/// Hard cap on how wide the tree panel renders. Anything wider is centred
/// so sessions on 160+ column monitors don't stretch to an unreadable
/// width.
pub const MAX_WIDTH: u16 = 120;

/// One line in the flattened tree. Headers and session rows share a struct
/// so the selection cursor can step over the whole list with a single
/// index; rendering decides what to do with each kind.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub kind: NodeKind,
    /// 0 for project headers and root sessions, 1+ for fork descendants.
    pub depth: usize,
    /// True when this node is the last child at its depth level — drives
    /// `└─` vs `├─` at the node's own connector column.
    pub is_last_child: bool,
    /// For each ancestor depth (excluding our own), was *that* ancestor
    /// the last child? If so the vertical `│` bar at that column is
    /// suppressed (drawn as blank space); otherwise the bar continues.
    ///
    /// `ancestor_bars.len() == depth.saturating_sub(1)` for session rows.
    pub ancestor_bars: Vec<bool>,
    /// For session rows that have fork children in this project: the
    /// total count of *descendants* underneath this node (transitive).
    /// Zero for leaves and non-session rows. Used by the renderer to
    /// show a `(+N forks)` hint when the node is collapsed, and by the
    /// event loop to decide whether `→`/`←` do anything meaningful.
    pub fork_descendants: usize,
    /// True when this session row has at least one direct or transitive
    /// fork child AND is currently expanded. Drives the `▾`/`▸` glyph
    /// shown in the gutter.
    pub is_expanded: bool,
    /// When this node is a session with children, its parent's id lives
    /// here so `←` on a collapsed root can jump to the parent (if any).
    /// `None` for project headers, spacers, and leaf sessions.
    pub parent_session_id: Option<String>,
}

/// Kind of row.
#[derive(Debug, Clone)]
pub enum NodeKind {
    /// Project section header ("architex/   14 sessions").
    ProjectHeader {
        project: Project,
        session_count: usize,
    },
    /// Blank spacer between project sections. Lets the cursor skip past
    /// it because it is not selectable.
    Spacer,
    /// One session row.
    SessionRow { session: Session },
}

impl TreeNode {
    /// True when Enter on this row should emit a selection. Project
    /// headers and spacers are decorative — the cursor may hop over them
    /// but pressing Enter on one is a no-op.
    pub fn is_selectable(&self) -> bool {
        matches!(self.kind, NodeKind::SessionRow { .. })
    }

    /// True when this session has fork children that can be shown/hidden.
    /// Leaves, headers and spacers all return false.
    pub fn is_collapsible(&self) -> bool {
        matches!(self.kind, NodeKind::SessionRow { .. }) && self.fork_descendants > 0
    }

    /// Session id under this node, if it is a session row.
    pub fn session_id(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::SessionRow { session } => Some(session.id.as_str()),
            _ => None,
        }
    }
}

/// Build the flattened list for one or more projects.
///
/// Projects are emitted in the order they were provided. Inside each
/// project, sessions form a forest:
///   1. Roots (sessions with `forked_from = None`, or whose parent is not
///      present in this project) first, sorted by `last_timestamp` desc.
///   2. Each root's fork subtree depth-first, each level also sorted by
///      `last_timestamp` desc.
///
/// Orphaned forks (their claimed parent id is missing) are promoted to
/// roots so they still render — they keep the `◆` glyph for clarity.
///
/// Every session row that has fork children renders in either an
/// expanded or collapsed state. This free function has historically been
/// "everything is expanded" — it still is, for back-compat. Callers that
/// want drill-down use [`build_tree_with_collapsed`].
pub fn build_tree(projects: &[Project], sessions_by_project: &[Vec<Session>]) -> Vec<TreeNode> {
    build_tree_with_collapsed(projects, sessions_by_project, &Default::default())
}

/// Same as [`build_tree`] but with an explicit "these session ids are
/// collapsed" set. Collapsed nodes still emit their own row (so the
/// cursor can land on them and the user knows they exist) — their fork
/// descendants are simply omitted from the output.
///
/// This is the form the live `--tree` screen uses. The default on entry
/// is *every fork root collapsed*; the `→` / `Space` / `l` keys remove
/// the node's id from the set, and `←` / `h` put it back.
pub fn build_tree_with_collapsed(
    projects: &[Project],
    sessions_by_project: &[Vec<Session>],
    collapsed: &std::collections::HashSet<String>,
) -> Vec<TreeNode> {
    assert_eq!(
        projects.len(),
        sessions_by_project.len(),
        "projects and sessions_by_project length mismatch",
    );

    let mut out: Vec<TreeNode> = Vec::new();

    let mut first = true;
    for (project, sessions) in projects.iter().zip(sessions_by_project.iter()) {
        if sessions.is_empty() {
            continue;
        }
        if !first {
            out.push(TreeNode {
                kind: NodeKind::Spacer,
                depth: 0,
                is_last_child: true,
                ancestor_bars: Vec::new(),
                fork_descendants: 0,
                is_expanded: false,
                parent_session_id: None,
            });
        }
        first = false;

        out.push(TreeNode {
            kind: NodeKind::ProjectHeader {
                project: project.clone(),
                session_count: sessions.len(),
            },
            depth: 0,
            is_last_child: true,
            ancestor_bars: Vec::new(),
            fork_descendants: 0,
            is_expanded: false,
            parent_session_id: None,
        });

        append_project_sessions(&mut out, sessions, collapsed);
    }

    out
}

/// Session ids (inside a single project) that are fork roots — i.e.
/// session rows at depth 1 that have at least one fork child.
///
/// Returned as a fresh `HashSet` so the caller can merge with existing
/// collapsed state (e.g., the live `--tree` seeds its collapsed set with
/// everything this returns).
pub fn collapsible_fork_root_ids(
    sessions_by_project: &[Vec<Session>],
) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for sessions in sessions_by_project.iter() {
        let mut id_to_index: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for (i, s) in sessions.iter().enumerate() {
            id_to_index.insert(s.id.as_str(), i);
        }
        let mut has_child = vec![false; sessions.len()];
        for s in sessions.iter() {
            if let Some(&parent_idx) = s.forked_from.as_deref().and_then(|p| id_to_index.get(p)) {
                has_child[parent_idx] = true;
            }
        }
        for (i, s) in sessions.iter().enumerate() {
            if has_child[i] {
                out.insert(s.id.clone());
            }
        }
    }
    out
}

/// Append all session rows for one project into `out`.
fn append_project_sessions(
    out: &mut Vec<TreeNode>,
    sessions: &[Session],
    collapsed: &std::collections::HashSet<String>,
) {
    // Index sessions by id for parent lookups.
    let mut id_to_index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, s) in sessions.iter().enumerate() {
        id_to_index.insert(s.id.as_str(), i);
    }

    // children_of[parent_index] = [child_indices].
    let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); sessions.len()];
    let mut roots: Vec<usize> = Vec::new();
    for (i, s) in sessions.iter().enumerate() {
        match s.forked_from.as_deref().and_then(|p| id_to_index.get(p)) {
            Some(&parent_idx) if parent_idx != i => children_of[parent_idx].push(i),
            _ => roots.push(i),
        }
    }

    // Newest-first sort everywhere.
    let sort_by_recency = |a: &usize, b: &usize, sessions: &[Session]| {
        sessions[*b]
            .last_timestamp
            .cmp(&sessions[*a].last_timestamp)
    };
    roots.sort_by(|a, b| sort_by_recency(a, b, sessions));
    for kids in children_of.iter_mut() {
        kids.sort_by(|a, b| sort_by_recency(a, b, sessions));
    }

    // Pre-compute transitive descendant counts so every session row can
    // report "(+N forks)" when collapsed.
    let mut descendant_count = vec![0usize; sessions.len()];
    fn count_desc(idx: usize, children_of: &[Vec<usize>], out: &mut [usize]) -> usize {
        let mut total = 0;
        for &c in &children_of[idx] {
            total += 1 + count_desc(c, children_of, out);
        }
        out[idx] = total;
        total
    }
    for &r in &roots {
        count_desc(r, &children_of, &mut descendant_count);
    }

    for (i, &root_idx) in roots.iter().enumerate() {
        let is_last = i + 1 == roots.len();
        walk(
            out,
            sessions,
            &children_of,
            &descendant_count,
            collapsed,
            root_idx,
            None, // roots have no parent in-project.
            1,
            is_last,
            &[], // roots have no ancestors above them in the in-project tree
        );
    }
}

/// Depth-first recursion emitting one [`TreeNode`] per visit. Skips the
/// subtree under any session whose id is in `collapsed`.
#[allow(clippy::too_many_arguments)]
fn walk(
    out: &mut Vec<TreeNode>,
    sessions: &[Session],
    children_of: &[Vec<usize>],
    descendant_count: &[usize],
    collapsed: &std::collections::HashSet<String>,
    idx: usize,
    parent_id: Option<&str>,
    depth: usize,
    is_last_child: bool,
    ancestor_bars: &[bool],
) {
    let this_id = sessions[idx].id.clone();
    let descendants = descendant_count[idx];
    let is_collapsed = descendants > 0 && collapsed.contains(&this_id);
    let is_expanded = descendants > 0 && !is_collapsed;

    out.push(TreeNode {
        kind: NodeKind::SessionRow {
            session: sessions[idx].clone(),
        },
        depth,
        is_last_child,
        ancestor_bars: ancestor_bars.to_vec(),
        fork_descendants: descendants,
        is_expanded,
        parent_session_id: parent_id.map(|s| s.to_string()),
    });

    let kids = &children_of[idx];
    if kids.is_empty() || is_collapsed {
        return;
    }

    // For my children, I contribute one more bar column: `!is_last_child`
    // means "my vertical bar still needs to be drawn at my column".
    let mut child_bars = ancestor_bars.to_vec();
    child_bars.push(is_last_child);

    for (i, &child) in kids.iter().enumerate() {
        let child_last = i + 1 == kids.len();
        walk(
            out,
            sessions,
            children_of,
            descendant_count,
            collapsed,
            child,
            Some(&this_id),
            depth + 1,
            child_last,
            &child_bars,
        );
    }
}

/// Build the connector prefix string for a session row. Includes trailing
/// space so the glyph can be appended directly.
///
/// Format at depth 1: `├─ ` or `└─ `.
/// At depth ≥ 2: `│  ` / `   ` per ancestor, then the leaf connector.
pub fn connector_prefix(node: &TreeNode) -> String {
    if node.depth == 0 {
        return String::new();
    }
    let mut s = String::new();
    for &bar_was_last in &node.ancestor_bars {
        // If that ancestor was the last child, its vertical bar is gone —
        // leave blank space; otherwise continue the bar.
        if bar_was_last {
            s.push_str("   ");
        } else {
            s.push_str("│  ");
        }
    }
    if node.is_last_child {
        s.push_str("└─ ");
    } else {
        s.push_str("├─ ");
    }
    s
}

/// Status glyph + its color style.
fn glyph_for(session: &Session, theme: &Theme) -> (&'static str, Style) {
    if session.is_fork {
        ("◆", Style::default().fg(theme.peach))
    } else if session.name.is_some() || session.auto_name.is_some() {
        ("●", Style::default().fg(theme.green))
    } else {
        ("○", Style::default().fg(theme.overlay0))
    }
}

/// Relative age like "2h ago", "yesterday", "Apr 14".
fn relative_age(ts: Option<DateTime<Utc>>) -> String {
    let Some(ts) = ts else {
        return "—".to_string();
    };
    let now = Utc::now();
    let diff = now.signed_duration_since(ts);
    if diff.num_seconds() < 60 {
        "just now".to_string()
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_days() == 1 {
        "yesterday".to_string()
    } else if diff.num_days() < 7 {
        format!("{}d ago", diff.num_days())
    } else {
        ts.format("%b %d").to_string()
    }
}

/// Format a USD cost the same way the picker does.
fn format_cost(cost: f64) -> String {
    if cost <= 0.0 {
        String::new()
    } else if cost < 0.01 {
        "<$0.01".to_string()
    } else {
        format!("${cost:.2}")
    }
}

/// Cap `s` to `max_cols` **display columns**, appending `…` if truncated.
///
/// Delegates to the unicode-aware helper so CJK / emoji session names never
/// exceed the column budget allotted by the row layout.
#[inline]
fn truncate(s: &str, max_cols: usize) -> String {
    truncate_to_width(s, max_cols)
}

/// Find the centred sub-rect inside `area` capped at [`MAX_WIDTH`] cols.
fn centred_block(area: Rect) -> Rect {
    let w = area.width.min(MAX_WIDTH);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    Rect {
        x,
        y: area.y,
        width: w,
        height: area.height,
    }
}

/// Render the tree screen. Caller provides the flattened `nodes` slice and
/// the current selection index.
pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    nodes: &[TreeNode],
    selected_index: usize,
    theme: &Theme,
) {
    let block_area = centred_block(area);

    // Outer block: rounded border, title + session count.
    let session_count: usize = nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::SessionRow { .. }))
        .count();

    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "claude-picker",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", theme.dim()),
        Span::styled("tree", theme.subtle()),
        Span::raw(" "),
    ]);
    let counter = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!(
                "{} session{}",
                session_count,
                if session_count == 1 { "" } else { "s" }
            ),
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
    .right_aligned();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border_active())
        .title(title)
        .title_top(counter);

    let inner = block.inner(block_area);
    frame.render_widget(block, block_area);

    if session_count == 0 {
        render_empty_state(frame, inner, theme);
        return;
    }

    let width = inner.width as usize;
    let items: Vec<ListItem<'_>> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let selected = i == selected_index;
            ListItem::new(render_row(n, theme, selected, width))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default())
        .highlight_symbol("");
    let mut state = ListState::default();
    state.select(Some(selected_index.min(nodes.len().saturating_sub(1))));
    frame.render_stateful_widget(list, inner, &mut state);
}

/// Centred "nothing to show" message.
fn render_empty_state(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let lines = vec![
        Line::raw(""),
        Line::styled(
            "No Claude Code sessions found.",
            Style::default()
                .fg(theme.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(
            "Run `claude` somewhere to create one.",
            Style::default().fg(theme.overlay1),
        ),
    ];
    let vertical_pad = area.height.saturating_sub(lines.len() as u16) / 2;
    let mut padded: Vec<Line<'_>> = Vec::with_capacity(lines.len() + vertical_pad as usize);
    for _ in 0..vertical_pad {
        padded.push(Line::raw(""));
    }
    padded.extend(lines);
    let p = Paragraph::new(padded).alignment(Alignment::Center);
    frame.render_widget(p, area);
}

/// Render a single row as a styled [`Line`].
fn render_row<'a>(node: &'a TreeNode, theme: &Theme, selected: bool, width: usize) -> Line<'a> {
    match &node.kind {
        NodeKind::Spacer => Line::raw(""),
        NodeKind::ProjectHeader {
            project,
            session_count,
        } => render_project_header(project, *session_count, theme, width),
        NodeKind::SessionRow { session } => {
            render_session_row(node, session, theme, selected, width)
        }
    }
}

fn render_project_header(
    project: &Project,
    session_count: usize,
    theme: &Theme,
    width: usize,
) -> Line<'static> {
    let label = format!("{}/", project.name);
    let meta = format!(
        "{} session{}",
        session_count,
        if session_count == 1 { "" } else { "s" }
    );

    // Space padding between name (left) and count (right-aligned).
    // `2` accounts for the leading gutter + trailing space. Use column width
    // so wide glyphs don't overflow.
    let used = 2 + display_width(&label) + display_width(&meta);
    let pad = width.saturating_sub(used).max(1);

    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            label,
            Style::default().fg(theme.blue).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(pad)),
        Span::styled(
            meta,
            Style::default()
                .fg(theme.overlay1)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn render_session_row<'a>(
    node: &TreeNode,
    session: &'a Session,
    theme: &Theme,
    selected: bool,
    width: usize,
) -> Line<'a> {
    // Session age drives the row-fade. Missing timestamp = very old.
    let age = match session.last_timestamp {
        Some(ts) => Utc::now()
            .signed_duration_since(ts)
            .to_std()
            .unwrap_or_default(),
        None => Duration::from_secs(60 * 24 * 3_600),
    };
    // Only fade unselected rows — the cursor row stays at full intensity.
    let apply_fade = !selected;

    // ── Gutter + cursor ────────────────────────────────────────────────
    let cursor_style_base = if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface2)
    };
    let cursor_style = maybe_fade(cursor_style_base, theme, age, apply_fade);
    let cursor = if selected { "▸" } else { " " };

    // ── Expand marker ────────────────────────────────────────────────
    // Only nodes with fork children get a marker; leaves get a space so
    // columns line up. `▾` when expanded, `▸` when collapsed. We also
    // show the marker on depth-0 fork roots only — inner descendants
    // (depth >= 2) are always visible by virtue of being rendered at
    // all, so they don't need a twisty.
    let expand_marker = if node.fork_descendants > 0 {
        if node.is_expanded {
            "▾"
        } else {
            "▸"
        }
    } else {
        " "
    };
    let expand_style_base = if node.fork_descendants > 0 {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface2)
    };
    let expand_style = maybe_fade(expand_style_base, theme, age, apply_fade);

    // ── Tree connector ────────────────────────────────────────────────
    let connector = connector_prefix(node);
    let connector_style = maybe_fade(Style::default().fg(theme.surface2), theme, age, apply_fade);

    // ── Glyph + name ──────────────────────────────────────────────────
    let (glyph, glyph_style_base) = glyph_for(session, theme);
    let glyph_style = maybe_fade(glyph_style_base, theme, age, apply_fade);

    let is_named = session.name.is_some();
    let label_text: String = session
        .name
        .clone()
        .or_else(|| session.auto_name.clone())
        .unwrap_or_else(|| "unnamed".to_string());
    let name_base = if session.is_fork {
        Style::default().fg(theme.peach)
    } else if is_named {
        Style::default()
            .fg(theme.green)
            .add_modifier(Modifier::BOLD)
    } else if session.auto_name.is_some() {
        Style::default().fg(theme.subtext0)
    } else {
        Style::default()
            .fg(theme.overlay0)
            .add_modifier(Modifier::ITALIC)
    };
    let name_style = maybe_fade(name_base, theme, age, apply_fade);

    // ── Right-aligned meta columns ────────────────────────────────────
    let age_label = relative_age(session.last_timestamp);
    let msgs = format!("{} msgs", session.message_count);
    let cost = format_cost(session.total_cost_usd);
    let age_col = 10;
    let msgs_col = 9;
    let cost_col = 9;
    // "(+N forks)" hint when collapsed — shown between name and meta,
    // right-aligned against the meta block. Lets the user see at a
    // glance "this root has things under it" without peeking.
    let fork_hint = if !node.is_expanded && node.fork_descendants > 0 {
        Some(format!(
            "(+{} fork{})",
            node.fork_descendants,
            if node.fork_descendants == 1 { "" } else { "s" }
        ))
    } else {
        None
    };
    let fork_hint_width = fork_hint.as_deref().map(display_width).unwrap_or(0);
    let fork_hint_pad = if fork_hint.is_some() { 2 } else { 0 };
    let meta_width = age_col + msgs_col + cost_col + 3 + fork_hint_width + fork_hint_pad;

    // Truncate the name so meta always fits. The left-side prefix consumes:
    //   cursor (2) + expand (2) + connector (variable cols) + glyph+space (2) + leading gutter (2)
    //
    // `display_width` so connector box-drawing glyphs and any future
    // ornamental unicode are counted in actual terminal cells.
    let prefix_chars =
        2 /* cursor + space */ + 2 /* expand + space */ + display_width(&connector) + 2 /* glyph + space */ + 2 /* gutter */;
    let name_budget = width
        .saturating_sub(prefix_chars)
        .saturating_sub(meta_width)
        .saturating_sub(1)
        .max(4);
    let label_trunc = truncate(&label_text, name_budget);
    let label_padded = pad_right(&label_trunc, name_budget);

    // Colors for meta.
    let meta_muted = maybe_fade(Style::default().fg(theme.overlay0), theme, age, apply_fade);
    let fork_hint_style = maybe_fade(Style::default().fg(theme.overlay0), theme, age, apply_fade);
    // Heat-mapped cost — identical ramp to the session-list column.
    let cost_style_base = if session.total_cost_usd <= 0.0 {
        Style::default().fg(theme.subtext0)
    } else {
        Style::default().fg(theme::cost_color(theme, session.total_cost_usd))
    };
    let cost_style = maybe_fade(cost_style_base, theme, age, apply_fade);

    let mut spans: Vec<Span<'_>> = vec![
        Span::styled(format!(" {cursor}"), cursor_style),
        Span::raw(" "),
        Span::styled(expand_marker.to_string(), expand_style),
        Span::raw(" "),
        Span::styled(connector, connector_style),
        Span::styled(glyph.to_string(), glyph_style),
        Span::raw(" "),
        Span::styled(label_padded, name_style),
    ];
    if let Some(hint) = fork_hint {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(hint, fork_hint_style));
        spans.push(Span::raw(" "));
    } else {
        spans.push(Span::raw(" "));
    }
    spans.extend([
        Span::styled(
            format!("{:>width$}", age_label, width = age_col),
            meta_muted,
        ),
        Span::raw(" "),
        Span::styled(format!("{:>width$}", msgs, width = msgs_col), meta_muted),
        Span::raw(" "),
        Span::styled(format!("{:>width$}", cost, width = cost_col), cost_style),
    ]);

    if selected {
        // Paint the row background so the cursor row pops.
        for span in &mut spans {
            span.style.bg = Some(theme.surface0);
        }
    }

    Line::from(spans)
}

/// Fade `style` through [`theme::age_fade_style`] when the row is eligible.
/// Mirrors the helper on `session_list` so both renderers stay in lockstep.
fn maybe_fade(style: Style, theme: &Theme, age: Duration, apply: bool) -> Style {
    if !apply {
        return style;
    }
    theme::age_fade_style(theme, style, age)
}

/// Pad `s` to exactly `width` **display columns**. Delegates to the
/// unicode-aware helper.
#[inline]
fn pad_right(s: &str, width: usize) -> String {
    pad_to_width(s, width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::pricing::TokenCounts;
    use crate::data::session::SessionKind;
    use std::path::PathBuf;

    fn mk_session(id: &str, forked_from: Option<&str>, name: Option<&str>) -> Session {
        Session {
            id: id.to_string(),
            project_dir: PathBuf::from("/tmp"),
            name: name.map(|s| s.to_string()),
            auto_name: None,
            last_prompt: None,
            message_count: 5,
            tokens: TokenCounts::default(),
            total_cost_usd: 0.0,
            model_summary: "claude-opus-4-7".to_string(),
            first_timestamp: None,
            last_timestamp: None,
            is_fork: forked_from.is_some(),
            forked_from: forked_from.map(|s| s.to_string()),
            entrypoint: SessionKind::Cli,
            permission_mode: None,
            subagent_count: 0,
            turn_durations: Vec::new(),
        }
    }

    fn mk_project(name: &str) -> Project {
        Project {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}")),
            encoded_dir: format!("-tmp-{name}"),
            session_count: 0,
            last_activity: None,
            git_branch: None,
        }
    }

    #[test]
    fn empty_projects_produce_empty_tree() {
        let nodes = build_tree(&[], &[]);
        assert!(nodes.is_empty());
    }

    #[test]
    fn root_with_fork_and_unrelated_flatten_order() {
        // Tree shape:
        //   project/
        //   ├─ root        (root1)
        //   │  └─ fork1    (forked_from = root1)
        //   └─ other       (unrelated, no fork)
        let project = mk_project("proj");
        let sessions = vec![
            mk_session("root1", None, Some("root-session")),
            mk_session("fork1", Some("root1"), Some("fork-of-root")),
            mk_session("other", None, Some("unrelated")),
        ];

        let nodes = build_tree(&[project], &[sessions]);

        // 1 header + 3 sessions = 4 nodes.
        assert_eq!(nodes.len(), 4);

        // Header first.
        assert!(matches!(nodes[0].kind, NodeKind::ProjectHeader { .. }));

        // Next: a root session at depth 1.
        match &nodes[1].kind {
            NodeKind::SessionRow { session } => {
                assert!(matches!(session.id.as_str(), "root1" | "other"));
            }
            _ => panic!("expected session row"),
        }
        assert_eq!(nodes[1].depth, 1);

        // The fork must appear immediately after its parent, at depth 2.
        // Find where root1 is, then check the next row is fork1.
        let root1_pos = nodes
            .iter()
            .position(
                |n| matches!(&n.kind, NodeKind::SessionRow { session } if session.id == "root1"),
            )
            .expect("root1 present");
        let next = &nodes[root1_pos + 1];
        match &next.kind {
            NodeKind::SessionRow { session } => assert_eq!(session.id, "fork1"),
            _ => panic!("fork1 should follow root1 directly"),
        }
        assert_eq!(next.depth, 2);

        // "other" should appear after the fork — so we see it at depth 1
        // somewhere in the tail.
        let other_pos = nodes
            .iter()
            .position(
                |n| matches!(&n.kind, NodeKind::SessionRow { session } if session.id == "other"),
            )
            .expect("other present");
        assert!(other_pos > root1_pos + 1);
        assert_eq!(nodes[other_pos].depth, 1);
    }

    #[test]
    fn connector_for_root_depth_one() {
        let node = TreeNode {
            kind: NodeKind::SessionRow {
                session: mk_session("a", None, None),
            },
            depth: 1,
            is_last_child: false,
            ancestor_bars: vec![],
            fork_descendants: 0,
            is_expanded: false,
            parent_session_id: None,
        };
        assert_eq!(connector_prefix(&node), "├─ ");

        let last = TreeNode {
            kind: NodeKind::SessionRow {
                session: mk_session("a", None, None),
            },
            depth: 1,
            is_last_child: true,
            ancestor_bars: vec![],
            fork_descendants: 0,
            is_expanded: false,
            parent_session_id: None,
        };
        assert_eq!(connector_prefix(&last), "└─ ");
    }

    #[test]
    fn connector_for_deep_fork() {
        // depth 3, my parent was NOT last-child, my grandparent was.
        let node = TreeNode {
            kind: NodeKind::SessionRow {
                session: mk_session("a", None, None),
            },
            depth: 3,
            is_last_child: false,
            // Ordered root → down: the first bar is the root-level
            // ancestor ("was last child?"), then the parent.
            ancestor_bars: vec![false, true],
            fork_descendants: 0,
            is_expanded: false,
            parent_session_id: None,
        };
        // Root ancestor not last → `│  `, parent was last → `   `, then `├─ `.
        assert_eq!(connector_prefix(&node), "│     ├─ ");
    }

    #[test]
    fn orphaned_fork_appears_as_root() {
        // forked_from points to an id that doesn't exist in this project.
        let project = mk_project("p");
        let sessions = vec![mk_session("orphan", Some("missing-parent"), Some("x"))];
        let nodes = build_tree(&[project], &[sessions]);
        assert_eq!(nodes.len(), 2);
        // Header + one root-depth session row.
        match &nodes[1].kind {
            NodeKind::SessionRow { session } => assert_eq!(session.id, "orphan"),
            _ => panic!("expected session row"),
        }
        assert_eq!(nodes[1].depth, 1, "orphaned fork must render as root");
    }

    #[test]
    fn deep_chain_flattens_depth_first() {
        // a -> b -> c -> d, single chain.
        let project = mk_project("p");
        let sessions = vec![
            mk_session("a", None, Some("a")),
            mk_session("b", Some("a"), Some("b")),
            mk_session("c", Some("b"), Some("c")),
            mk_session("d", Some("c"), Some("d")),
        ];
        let nodes = build_tree(&[project], &[sessions]);
        // header + 4 sessions
        assert_eq!(nodes.len(), 5);
        let depths: Vec<usize> = nodes[1..].iter().map(|n| n.depth).collect();
        assert_eq!(depths, vec![1, 2, 3, 4]);

        // Connectors: root └─, each descendant is the only child so also └─,
        // and each level's ancestor_bars contains only last-child=true entries
        // (meaning bars collapse to blanks).
        let conns: Vec<String> = nodes[1..].iter().map(connector_prefix).collect();
        assert_eq!(conns[0], "└─ ");
        assert_eq!(conns[1], "   └─ ");
        assert_eq!(conns[2], "      └─ ");
        assert_eq!(conns[3], "         └─ ");
    }

    #[test]
    fn sibling_forks_use_branch_connector() {
        // root has two children: fork1, fork2. fork1 gets ├─, fork2 gets └─.
        let project = mk_project("p");
        let sessions = vec![
            mk_session("root", None, Some("root")),
            mk_session("fork1", Some("root"), Some("f1")),
            mk_session("fork2", Some("root"), Some("f2")),
        ];
        let nodes = build_tree(&[project], &[sessions]);
        // header + 3 sessions
        assert_eq!(nodes.len(), 4);

        // Children order is newest-first; both have no timestamp so the
        // order is deterministic (cmp of Options — both None are equal so
        // the original order is preserved). Their connectors share the
        // leading blank/bar column contributed by the (only) root
        // "last-child" ancestor, so we check `ends_with` rather than
        // exact-match.
        let fork_nodes: Vec<&TreeNode> = nodes[2..=3].iter().collect();
        let mut saw_branch = false;
        let mut saw_last = false;
        for n in fork_nodes {
            let c = connector_prefix(n);
            if c.ends_with("├─ ") {
                saw_branch = true;
            } else if c.ends_with("└─ ") {
                saw_last = true;
            }
        }
        assert!(
            saw_branch && saw_last,
            "need both ├─ and └─ for two siblings"
        );
    }

    #[test]
    fn multi_project_inserts_spacer() {
        let p1 = mk_project("p1");
        let p2 = mk_project("p2");
        let s1 = vec![mk_session("a", None, Some("a"))];
        let s2 = vec![mk_session("b", None, Some("b"))];
        let nodes = build_tree(&[p1, p2], &[s1, s2]);
        // header, row, spacer, header, row = 5
        assert_eq!(nodes.len(), 5);
        assert!(matches!(nodes[0].kind, NodeKind::ProjectHeader { .. }));
        assert!(matches!(nodes[1].kind, NodeKind::SessionRow { .. }));
        assert!(matches!(nodes[2].kind, NodeKind::Spacer));
        assert!(matches!(nodes[3].kind, NodeKind::ProjectHeader { .. }));
        assert!(matches!(nodes[4].kind, NodeKind::SessionRow { .. }));
    }

    #[test]
    fn truncate_unicode_safe() {
        // 😀 = 2 cols, so in a 3-col budget we fit "a" (1) + "…" (1) = 2 cols
        // and still stay under the cap. Check columns, not codepoints.
        let s = "a\u{1F600}b\u{0928}c";
        let out = truncate(s, 3);
        assert!(
            display_width(&out) <= 3,
            "got width {}: {out}",
            display_width(&out)
        );
        assert!(out.ends_with('…'));
    }

    #[test]
    fn format_cost_cases() {
        assert_eq!(format_cost(0.0), "");
        assert_eq!(format_cost(0.005), "<$0.01");
        assert_eq!(format_cost(0.41), "$0.41");
        assert_eq!(format_cost(1.23), "$1.23");
    }

    #[test]
    fn relative_age_handles_missing() {
        assert_eq!(relative_age(None), "—");
    }

    #[test]
    fn selection_skips_non_selectable() {
        // Sanity-check on is_selectable — only session rows are selectable.
        let header = TreeNode {
            kind: NodeKind::ProjectHeader {
                project: mk_project("p"),
                session_count: 0,
            },
            depth: 0,
            is_last_child: true,
            ancestor_bars: vec![],
            fork_descendants: 0,
            is_expanded: false,
            parent_session_id: None,
        };
        assert!(!header.is_selectable());
        let spacer = TreeNode {
            kind: NodeKind::Spacer,
            depth: 0,
            is_last_child: true,
            ancestor_bars: vec![],
            fork_descendants: 0,
            is_expanded: false,
            parent_session_id: None,
        };
        assert!(!spacer.is_selectable());
        let row = TreeNode {
            kind: NodeKind::SessionRow {
                session: mk_session("a", None, None),
            },
            depth: 1,
            is_last_child: true,
            ancestor_bars: vec![],
            fork_descendants: 0,
            is_expanded: false,
            parent_session_id: None,
        };
        assert!(row.is_selectable());
    }

    #[test]
    fn collapsed_root_hides_descendants_but_counts_them() {
        let project = mk_project("p");
        let sessions = vec![
            mk_session("root", None, Some("root")),
            mk_session("fork1", Some("root"), Some("f1")),
            mk_session("fork2", Some("root"), Some("f2")),
        ];
        let mut collapsed = std::collections::HashSet::new();
        collapsed.insert("root".to_string());
        let nodes = build_tree_with_collapsed(&[project], &[sessions], &collapsed);
        // header + 1 root row (forks hidden)
        assert_eq!(nodes.len(), 2);
        let row = &nodes[1];
        match &row.kind {
            NodeKind::SessionRow { session } => assert_eq!(session.id, "root"),
            _ => panic!("expected session row"),
        }
        assert_eq!(row.fork_descendants, 2, "descendant count preserved");
        assert!(!row.is_expanded);
    }

    #[test]
    fn expanded_root_shows_descendants() {
        let project = mk_project("p");
        let sessions = vec![
            mk_session("root", None, Some("root")),
            mk_session("fork1", Some("root"), Some("f1")),
        ];
        let empty = std::collections::HashSet::new();
        let nodes = build_tree_with_collapsed(&[project], &[sessions], &empty);
        // header + root + fork1
        assert_eq!(nodes.len(), 3);
        let root_row = &nodes[1];
        assert!(root_row.is_expanded);
        assert_eq!(root_row.fork_descendants, 1);
    }

    #[test]
    fn collapsible_ids_returns_only_fork_parents() {
        let sessions = vec![
            mk_session("root_with_child", None, Some("a")),
            mk_session("leaf_fork", Some("root_with_child"), Some("b")),
            mk_session("lonely_root", None, Some("c")),
        ];
        let ids = collapsible_fork_root_ids(&[sessions]);
        assert!(ids.contains("root_with_child"));
        assert!(!ids.contains("leaf_fork"));
        assert!(!ids.contains("lonely_root"));
    }

    #[test]
    fn parent_session_id_set_for_forks_not_roots() {
        let project = mk_project("p");
        let sessions = vec![
            mk_session("root", None, Some("r")),
            mk_session("fork", Some("root"), Some("f")),
        ];
        let empty = std::collections::HashSet::new();
        let nodes = build_tree_with_collapsed(&[project], &[sessions], &empty);
        // nodes[1] = root (parent = None), nodes[2] = fork (parent = "root")
        let root = &nodes[1];
        let fork = &nodes[2];
        assert!(root.parent_session_id.is_none());
        assert_eq!(fork.parent_session_id.as_deref(), Some("root"));
    }
}
