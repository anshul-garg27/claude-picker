//! Sub-agent tree visualisation for `Task` tool-use results.
//!
//! When the assistant dispatches a sub-agent via the `Task` tool, the
//! `tool_result` that comes back is a serialised mini-transcript — the
//! sub-conversation the sub-agent had before returning. The default flat
//! renderer in [`crate::ui::conversation_viewer`] dumps that text as-is,
//! which makes the sub-agent turns indistinguishable from the parent
//! conversation.
//!
//! This module provides a pure helper that turns the sub-agent result into
//! indented, connector-prefixed [`Line`]s so the sub-conversation visually
//! nests under the parent `Task` call, using the same box-drawing dialect
//! most TUI trees speak (`├─`, `└─`, `│ `).
//!
//! Parsing strategy: the sub-agent transcript comes in a few shapes —
//! double-blank-line-separated plain text is by far the most common, with
//! the `**Assistant**` / `**User**` markdown separator a close second, and
//! `{"messages":[…]}` JSON a rare third. To stay robust we do a cheap
//! structural detection (JSON first, then markdown separators, then blank
//! lines) and fall back to dumping the raw result with a leading `│ `
//! connector on every line. That fallback is still far more useful than the
//! current flat output because the whole block is visually indented and
//! ruled off as a single sub-conversation.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme::Theme;

/// Horizontal indent applied to every sub-agent line so the tree sits under
/// the parent `Task` call. Eight spaces matches the body-indent used by
/// [`crate::ui::conversation_viewer::render_message`] for assistant text.
const INDENT: &str = "        ";

/// Connector used for every turn except the last.
const MID_CONNECTOR: &str = "├─ ";
/// Connector used for the final turn — closes the vertical rule.
const LAST_CONNECTOR: &str = "└─ ";
/// Connector used for wrapped / body lines belonging to a turn that is not
/// the last. Keeps the vertical rule alive to the left of the body.
const MID_BODY_PREFIX: &str = "│  ";
/// Connector used for wrapped / body lines that belong to the final turn.
/// The vertical rule has already terminated, so we just indent.
const LAST_BODY_PREFIX: &str = "   ";

/// One parsed turn of the sub-agent transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SubTurn {
    /// Short label rendered after the connector — e.g. `Subagent`, `User`,
    /// `Tool: Read`.
    head: String,
    /// Optional body lines. The first is rendered on the same line as the
    /// head (after an em-dash); subsequent lines wrap below.
    body: Vec<String>,
}

/// Render a sub-agent `tool_result` string as a nested, connector-prefixed
/// block. `width` is the outer content width — we reserve room for the
/// indent + connector before wrapping body text so nothing overflows.
///
/// Never panics: on parse failure we fall back to the "dump every line with
/// a `│ ` connector" degrade path described in the module docs.
pub fn render_subagent_block(result: &str, theme: &Theme, width: usize) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    if result.trim().is_empty() {
        // Empty result — still draw the anchor so the parent Task call
        // doesn't look orphaned.
        out.push(styled_tree_line(
            LAST_CONNECTOR,
            "(subagent returned no output)",
            theme,
            /*italic=*/ true,
        ));
        return out;
    }

    let turns = parse_turns(result);
    if turns.is_empty() {
        // Graceful-degrade path: no structured turns parsed, dump the raw
        // result indented with a `│ ` rule. The reader still gets the
        // nesting cue even if we couldn't break it into turns.
        let available = body_width(width, MID_BODY_PREFIX);
        let lines: Vec<String> = result
            .lines()
            .flat_map(|l| wrap_to_width(l, available))
            .collect();
        for (i, line) in lines.iter().enumerate() {
            let is_last = i + 1 == lines.len();
            let prefix = if is_last {
                LAST_BODY_PREFIX
            } else {
                MID_BODY_PREFIX
            };
            out.push(styled_tree_line(prefix, line, theme, false));
        }
        return out;
    }

    let total = turns.len();
    for (idx, turn) in turns.iter().enumerate() {
        let is_last = idx + 1 == total;
        let head_connector = if is_last {
            LAST_CONNECTOR
        } else {
            MID_CONNECTOR
        };
        let body_prefix = if is_last {
            LAST_BODY_PREFIX
        } else {
            MID_BODY_PREFIX
        };

        // Head line — connector + short label.
        let head_available = body_width(width, head_connector);
        let head_text = truncate_or_pass(&turn.head, head_available);
        out.push(styled_tree_line(head_connector, &head_text, theme, false));

        // Body lines — indented under the connector, wrapped to the column.
        let body_available = body_width(width, body_prefix);
        for raw_body_line in &turn.body {
            for wrapped in wrap_to_width(raw_body_line, body_available) {
                out.push(styled_tree_line(body_prefix, &wrapped, theme, false));
            }
        }
    }

    out
}

/// Build a `Line` with the indent + connector styled as overlay0 and the
/// body text styled as subtext1. `italic` flips italic on for placeholder
/// lines (e.g. the "no output" case).
fn styled_tree_line(connector: &str, body: &str, theme: &Theme, italic: bool) -> Line<'static> {
    let connector_style = Style::default().fg(theme.overlay0);
    let mut body_style = Style::default().fg(theme.subtext1);
    if italic {
        body_style = body_style.add_modifier(Modifier::ITALIC);
    }
    Line::from(vec![
        Span::raw(INDENT),
        Span::styled(connector.to_string(), connector_style),
        Span::styled(body.to_string(), body_style),
    ])
}

/// Compute the available character width for body text after subtracting the
/// indent + connector prefix. Bottoms out at 20 so narrow terminals don't
/// wrap every character onto its own line.
fn body_width(outer: usize, prefix: &str) -> usize {
    let consumed = INDENT.chars().count() + prefix.chars().count();
    outer.saturating_sub(consumed).max(20)
}

/// Parse the sub-agent result string into a list of turns. Tries three
/// shapes in descending likelihood; returns an empty vec if none match so
/// the caller can fall back to the flat dump.
fn parse_turns(result: &str) -> Vec<SubTurn> {
    let trimmed = result.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // 1. JSON — `{"messages":[{"role":..., "content":...}, …]}`
    if let Some(turns) = try_parse_json(trimmed) {
        if !turns.is_empty() {
            return turns;
        }
    }

    // 2. Markdown separators — `**Assistant**` / `**User**` / `**Tool**`.
    if let Some(turns) = try_parse_markdown(trimmed) {
        if turns.len() >= 2 {
            return turns;
        }
    }

    // 3. Double-blank-line-separated plain text — the most common shape.
    let blank_split = try_parse_blank_separated(trimmed);
    if blank_split.len() >= 2 {
        return blank_split;
    }

    // Single chunk of text — still worth rendering as one turn so the
    // tree shape shows up.
    if !blank_split.is_empty() {
        return blank_split;
    }

    Vec::new()
}

/// `{"messages":[…]}` JSON shape. Returns `None` if the input isn't JSON or
/// doesn't contain a `messages` array we can read.
fn try_parse_json(input: &str) -> Option<Vec<SubTurn>> {
    if !input.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    let arr = value.get("messages").and_then(|v| v.as_array())?;
    let mut out = Vec::with_capacity(arr.len());
    for entry in arr {
        let obj = entry.as_object()?;
        let role = obj
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("message")
            .to_string();
        let content = obj
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let (head, body) = split_head_body(&role, &content);
        out.push(SubTurn { head, body });
    }
    Some(out)
}

/// `**Assistant**: hi\n\n**User**: ...` markdown shape.
fn try_parse_markdown(input: &str) -> Option<Vec<SubTurn>> {
    // Quick-reject: if there's no `**` pair at all this shape doesn't apply.
    if !input.contains("**") {
        return None;
    }
    let mut turns: Vec<SubTurn> = Vec::new();
    let mut current_role: Option<String> = None;
    let mut current_body: Vec<String> = Vec::new();

    for raw_line in input.lines() {
        let line = raw_line.trim_end();
        if let Some(role) = parse_markdown_role(line) {
            if let Some(prev_role) = current_role.take() {
                turns.push(mk_turn(&prev_role, &current_body));
                current_body.clear();
            }
            current_role = Some(role);
        } else if current_role.is_some() {
            current_body.push(line.to_string());
        }
    }
    if let Some(role) = current_role {
        turns.push(mk_turn(&role, &current_body));
    }
    if turns.is_empty() {
        return None;
    }
    Some(turns)
}

/// Detect `**Role**` at the start of a line (optionally followed by `:`).
fn parse_markdown_role(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("**")?;
    let end = rest.find("**")?;
    let role = rest[..end].trim().to_string();
    if role.is_empty() || role.len() > 40 {
        return None;
    }
    Some(role)
}

fn mk_turn(role: &str, body: &[String]) -> SubTurn {
    // Drop leading/trailing blank body lines so the turn reads cleanly.
    let mut body: Vec<String> = body
        .iter()
        .skip_while(|l| l.trim().is_empty())
        .cloned()
        .collect();
    while body.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        body.pop();
    }
    // Promote the first body line into the head if it's short — that's the
    // zellij-style "label — summary" look the viewer wants.
    let (head, body) = split_head_body(role, &body.join("\n"));
    SubTurn { head, body }
}

/// Double-blank-line split — each chunk becomes a turn with the first
/// non-blank line as the head.
fn try_parse_blank_separated(input: &str) -> Vec<SubTurn> {
    let chunks: Vec<&str> = input
        .split("\n\n")
        .map(|c| c.trim_matches('\n'))
        .filter(|c| !c.trim().is_empty())
        .collect();
    if chunks.is_empty() {
        return Vec::new();
    }
    let mut turns = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut lines = chunk.lines();
        let head = lines.next().unwrap_or("").trim().to_string();
        let body: Vec<String> = lines.map(|l| l.to_string()).collect();
        turns.push(SubTurn {
            head: if head.is_empty() {
                "(turn)".to_string()
            } else {
                head
            },
            body,
        });
    }
    turns
}

/// Given a role + raw body, split off the first short line to ride alongside
/// the head after an em-dash separator. Keeps the rest as body lines.
fn split_head_body(role: &str, content: &str) -> (String, Vec<String>) {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return (format_head(role, None), Vec::new());
    }
    let mut iter = trimmed.lines();
    let first = iter.next().unwrap_or("").trim();
    let rest: Vec<String> = iter.map(|l| l.to_string()).collect();
    if !first.is_empty() && first.chars().count() <= 80 {
        (format_head(role, Some(first)), rest)
    } else {
        (
            format_head(role, None),
            trimmed.lines().map(|l| l.to_string()).collect(),
        )
    }
}

fn format_head(role: &str, lead: Option<&str>) -> String {
    let role = role.trim();
    match lead {
        Some(l) if !l.is_empty() => format!("{role} — {l}"),
        _ => role.to_string(),
    }
}

/// Truncate with an ellipsis if it won't fit.
fn truncate_or_pass(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        return s.to_string();
    }
    let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Greedy whitespace wrap — mirrors the viewer's style. Returns at least one
/// element so callers can always `for line in …`.
fn wrap_to_width(line: &str, width: usize) -> Vec<String> {
    let width = width.max(10);
    if line.chars().count() <= width {
        return vec![line.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        let word_len = word.chars().count();
        let current_len = current.chars().count();
        if current_len == 0 {
            // Word might itself overflow — in that case chop it.
            if word_len > width {
                out.push(
                    word.chars()
                        .take(width.saturating_sub(1))
                        .collect::<String>()
                        + "…",
                );
                continue;
            }
            current.push_str(word);
        } else if current_len + 1 + word_len <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            out.push(current.clone());
            current.clear();
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(line.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Theme, ThemeName};

    fn test_theme() -> Theme {
        Theme::from_name(ThemeName::CatppuccinMocha)
    }

    fn line_plain_text(line: &Line<'_>) -> String {
        let mut s = String::new();
        for span in &line.spans {
            s.push_str(&span.content);
        }
        s
    }

    #[test]
    fn empty_result_renders_placeholder_with_last_connector() {
        let theme = test_theme();
        let lines = render_subagent_block("", &theme, 80);
        assert_eq!(
            lines.len(),
            1,
            "empty result renders a single placeholder line"
        );
        let text = line_plain_text(&lines[0]);
        assert!(
            text.contains("└─"),
            "placeholder uses the last connector, got: {text:?}"
        );
        assert!(
            text.to_lowercase().contains("no output"),
            "placeholder mentions no output, got: {text:?}"
        );
    }

    #[test]
    fn single_turn_plain_text_has_last_connector_only() {
        let theme = test_theme();
        let result = "Subagent: started reading src/auth/middleware.rs";
        let lines = render_subagent_block(result, &theme, 80);
        assert!(!lines.is_empty());
        // Single turn ⇒ the head must carry the └─ connector, no ├─.
        let joined: String = lines
            .iter()
            .map(line_plain_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("└─") || joined.contains("│  "),
            "single turn should render with last or body connector, got: {joined:?}"
        );
        assert!(
            !joined.contains("├─"),
            "single turn must not emit the mid connector, got: {joined:?}"
        );
        assert!(
            joined.contains("Subagent"),
            "body text preserved, got: {joined:?}"
        );
    }

    #[test]
    fn multi_turn_result_wraps_long_body_lines() {
        let theme = test_theme();
        // Two turns, each separated by a blank line, with a long body line
        // in the second turn that must wrap within the available width.
        let long_body = "the quick brown fox jumps over the lazy dog ".repeat(8);
        let result = format!(
            "Subagent: started\nreading src/auth/middleware.rs\n\nTool: Read src/auth/token.rs\n{long_body}"
        );
        let lines = render_subagent_block(&result, &theme, 60);
        assert!(
            lines.len() >= 3,
            "expected multiple rendered lines, got {}",
            lines.len()
        );

        let joined: String = lines
            .iter()
            .map(line_plain_text)
            .collect::<Vec<_>>()
            .join("\n");
        // Multi-turn ⇒ at least one mid connector AND the last connector.
        assert!(
            joined.contains("├─"),
            "multi-turn missing mid connector: {joined}"
        );
        assert!(
            joined.contains("└─"),
            "multi-turn missing last connector: {joined}"
        );
        // No rendered line should exceed the viewer width (accounting for
        // the fixed 8-space indent which is part of every line).
        for line in &lines {
            let w = line_plain_text(line).chars().count();
            assert!(
                w <= 60 + INDENT.chars().count(),
                "line wider than allocated width: {w} for {line:?}"
            );
        }
    }

    #[test]
    fn markdown_separator_result_is_parsed_as_multiple_turns() {
        let theme = test_theme();
        let result = "**Assistant**: I'll investigate.\n\n**User**: thanks";
        let lines = render_subagent_block(result, &theme, 80);
        let joined: String = lines
            .iter()
            .map(line_plain_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("Assistant"),
            "assistant turn present: {joined}"
        );
        assert!(joined.contains("User"), "user turn present: {joined}");
        assert!(
            joined.contains("├─"),
            "multi-turn uses mid connector: {joined}"
        );
        assert!(
            joined.contains("└─"),
            "multi-turn uses last connector: {joined}"
        );
    }

    #[test]
    fn every_line_carries_the_indent_prefix() {
        let theme = test_theme();
        let result = "first turn body\n\nsecond turn body";
        let lines = render_subagent_block(result, &theme, 80);
        for line in &lines {
            let text = line_plain_text(line);
            assert!(
                text.starts_with(INDENT),
                "every rendered line starts with the fixed indent: {text:?}"
            );
        }
    }
}
