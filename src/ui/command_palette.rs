//! Space-leader command palette.
//!
//! A centered modal opened by pressing `Space` on any list screen.
//! Shows a filter input at the top and the current screen's actions
//! below, filterable via nucleo fuzzy matching. `↑/↓` navigate, `Enter`
//! executes, `Esc` closes without doing anything.
//!
//! The palette is a pure state + render pair — it does not mutate app
//! state directly. The event loop that owns it feeds keys in via
//! [`CommandPalette::handle_event`] and reacts to the resulting
//! [`Outcome`]. Actions are keyed by a stable `'static` id so each
//! calling screen can pattern-match in its own dispatch.

use nucleo::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32String};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::events::Event;
use crate::theme::Theme;
use crate::ui::actions::{actions_for_context, ActionGroup, PaletteAction};

// Re-export `Context` through the palette module so `command_palette::Context`
// is the idiomatic path. Keeps callers from having to also `use ui::actions`
// when they only touch the palette.
pub use crate::ui::actions::Context;

/// Result of feeding an event into the palette.
pub enum Outcome {
    /// Palette consumed the event and wants to stay open.
    Continue,
    /// Palette was dismissed (Esc). Caller should drop it.
    Close,
    /// User selected an action. Caller should drop the palette *and*
    /// run the branch of its own dispatch matching this id.
    Execute(&'static str),
}

/// Live state of an open palette. Construct via [`CommandPalette::new`]
/// each time the user presses Space.
pub struct CommandPalette {
    /// The active context — picks which actions to show.
    context: Context,
    /// The full set of actions for `context`. Matched against `query`.
    all: Vec<PaletteAction>,
    /// Pre-encoded Utf32String haystacks (label + keybinding + id) —
    /// built once at construction so filter keystrokes are allocation-
    /// free except for the scratch matcher.
    haystacks: Vec<Utf32String>,
    /// Indices of `all` that match the current query, ordered high-
    /// score-first.
    filtered: Vec<usize>,
    /// Filter buffer.
    query: String,
    /// Cursor in `filtered` (0..filtered.len()).
    cursor: usize,
    /// Nucleo matcher — kept so scratch memory is reused across keys.
    matcher: Matcher,
}

impl CommandPalette {
    pub fn new(context: Context) -> Self {
        let actions: Vec<PaletteAction> = actions_for_context(context).to_vec();
        let haystacks: Vec<Utf32String> = actions
            .iter()
            .map(|a| {
                let composite = format!(
                    "{} {} {} {}",
                    a.label,
                    a.keybinding,
                    a.id,
                    group_key(a.group)
                );
                Utf32String::from(composite)
            })
            .collect();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        Self {
            context,
            all: actions,
            haystacks,
            filtered,
            query: String::new(),
            cursor: 0,
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    /// Current context — exposed so the renderer can show a pretty
    /// label in the palette title.
    pub fn context(&self) -> Context {
        self.context
    }

    /// Feed a single event. See [`Outcome`].
    pub fn handle_event(&mut self, ev: Event) -> Outcome {
        match ev {
            Event::Escape => Outcome::Close,
            Event::Up => {
                self.step(-1);
                Outcome::Continue
            }
            Event::Down => {
                self.step(1);
                Outcome::Continue
            }
            Event::PageUp => {
                self.step(-5);
                Outcome::Continue
            }
            Event::PageDown => {
                self.step(5);
                Outcome::Continue
            }
            Event::Home => {
                self.cursor = 0;
                Outcome::Continue
            }
            Event::End => {
                self.cursor = self.filtered.len().saturating_sub(1);
                Outcome::Continue
            }
            Event::Enter => match self.selected_id() {
                Some(id) => Outcome::Execute(id),
                None => Outcome::Continue,
            },
            Event::Backspace => {
                self.query.pop();
                self.refilter();
                Outcome::Continue
            }
            Event::Key(c) if is_query_char(c) => {
                self.query.push(c);
                self.refilter();
                Outcome::Continue
            }
            // Swallow everything else so random keys don't leak to the
            // underlying screen while the palette is open.
            _ => Outcome::Continue,
        }
    }

    fn step(&mut self, delta: i32) {
        let len = self.filtered.len() as i32;
        if len == 0 {
            return;
        }
        let next = (self.cursor as i32 + delta).rem_euclid(len);
        self.cursor = next as usize;
    }

    fn selected_id(&self) -> Option<&'static str> {
        let idx = *self.filtered.get(self.cursor)?;
        self.all.get(idx).map(|a| a.id)
    }

    fn refilter(&mut self) {
        self.filtered.clear();
        self.cursor = 0;
        if self.query.is_empty() {
            self.filtered.extend(0..self.all.len());
            return;
        }
        let pattern = Pattern::new(
            &self.query,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut scored: Vec<(u32, usize)> = Vec::with_capacity(self.all.len());
        for (i, hay) in self.haystacks.iter().enumerate() {
            if let Some(score) = pattern.score(hay.slice(..), &mut self.matcher) {
                scored.push((score, i));
            }
        }
        scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.filtered = scored.into_iter().map(|(_, i)| i).collect();
    }
}

/// Accept most keystrokes into the filter. Exclude control + newline +
/// tab so those stay hot-keys.
fn is_query_char(c: char) -> bool {
    !c.is_control() && c != '\n' && c != '\r' && c != '\t'
}

/// A stable `group_key` that sorts the same way visually as the
/// ActionGroup variants — just a string that nucleo can match against
/// so queries like "help" surface items in the Help group.
fn group_key(g: ActionGroup) -> &'static str {
    g.title()
}

/// Public render entry. Caller owns the frame area (the full terminal
/// frame) and passes the live state in.
pub fn render(frame: &mut Frame<'_>, area: Rect, palette: &CommandPalette, theme: &Theme) {
    // Centred modal: ~60 wide × ~18 tall, capped to the frame minus a
    // couple of rows of margin.
    let w = 64u16.min(area.width.saturating_sub(4));
    let h = 20u16.min(area.height.saturating_sub(2)).max(8);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(Clear, rect);

    let context_label = match palette.context {
        Context::SessionList => "sessions",
        Context::ProjectList => "projects",
        Context::Tree => "tree",
        Context::Search => "search",
    };
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "⌘ Command palette",
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  ", theme.dim()),
        Span::styled(context_label, theme.subtle()),
        Span::raw(" "),
    ]);
    let close_hint = Line::from(vec![
        Span::raw(" "),
        Span::styled("Esc", theme.key_hint()),
        Span::raw(" "),
        Span::styled("close", theme.key_desc()),
        Span::raw("   "),
        Span::styled("Enter", theme.key_hint()),
        Span::raw(" "),
        Span::styled("run", theme.key_desc()),
        Span::raw(" "),
    ])
    .right_aligned();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(title)
        .title_bottom(close_hint);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    // Split inner: first 3 rows = input + separator, rest = list.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // padding
            Constraint::Length(1), // input
            Constraint::Length(1), // separator
            Constraint::Min(1),    // list
        ])
        .split(inner);

    render_input(frame, rows[1], palette, theme);
    render_separator(frame, rows[2], theme);
    render_list(frame, rows[3], palette, theme);
}

fn render_input(frame: &mut Frame<'_>, area: Rect, palette: &CommandPalette, theme: &Theme) {
    let prompt_style = Style::default()
        .fg(theme.mauve)
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(theme.text);
    let placeholder_style = Style::default()
        .fg(theme.overlay0)
        .add_modifier(Modifier::ITALIC);

    let spans: Vec<Span<'_>> = if palette.query.is_empty() {
        vec![
            Span::raw(" "),
            Span::styled(">", prompt_style),
            Span::raw(" "),
            Span::styled("run action…", placeholder_style),
        ]
    } else {
        vec![
            Span::raw(" "),
            Span::styled(">", prompt_style),
            Span::raw(" "),
            Span::styled(palette.query.clone(), text_style),
            Span::styled(
                "_",
                Style::default()
                    .fg(theme.mauve)
                    .add_modifier(Modifier::RAPID_BLINK),
            ),
        ]
    };
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_separator(frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
    let line = "─".repeat(area.width as usize);
    let p = Paragraph::new(Line::styled(line, Style::default().fg(theme.surface2)));
    frame.render_widget(p, area);
}

fn render_list(frame: &mut Frame<'_>, area: Rect, palette: &CommandPalette, theme: &Theme) {
    if palette.filtered.is_empty() {
        let p = Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("no matches", Style::default().fg(theme.overlay0)),
        ]));
        frame.render_widget(p, area);
        return;
    }

    let width = area.width as usize;
    let items: Vec<ListItem<'_>> = palette
        .filtered
        .iter()
        .enumerate()
        .map(|(ri, &idx)| {
            let action = &palette.all[idx];
            let selected = ri == palette.cursor;
            ListItem::new(render_row(action, selected, width, theme))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(Style::default())
        .highlight_symbol("");
    let mut state = ListState::default();
    state.select(Some(
        palette.cursor.min(palette.filtered.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_row<'a>(
    action: &'a PaletteAction,
    selected: bool,
    width: usize,
    theme: &Theme,
) -> Line<'a> {
    let cursor = if selected { "▸" } else { " " };
    let cursor_style = if selected {
        Style::default()
            .fg(theme.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.surface2)
    };
    let label_style = if selected {
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.subtext1)
    };
    let keybind_style = Style::default().fg(theme.overlay0);
    let icon = if action.icon.is_empty() {
        "  "
    } else {
        action.icon
    };

    // Reserve the right edge for the keybinding.
    let kb_w = action.keybinding.chars().count();
    let left = format!(" {cursor} {icon} {}", action.label);
    let left_w = left.chars().count();
    let kb_reserved = if kb_w > 0 { kb_w + 2 } else { 0 };
    let pad = width
        .saturating_sub(left_w)
        .saturating_sub(kb_reserved)
        .max(1);

    let mut spans: Vec<Span<'_>> = vec![
        Span::styled(format!(" {cursor}"), cursor_style),
        Span::raw(" "),
        Span::styled(icon.to_string(), Style::default().fg(theme.mauve)),
        Span::raw(" "),
        Span::styled(action.label.to_string(), label_style),
        Span::raw(" ".repeat(pad)),
    ];
    if !action.keybinding.is_empty() {
        spans.push(Span::styled(action.keybinding.to_string(), keybind_style));
        spans.push(Span::raw(" "));
    }
    if selected {
        for span in &mut spans {
            span.style.bg = Some(theme.surface0);
        }
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_palette_shows_all_actions_for_context() {
        let p = CommandPalette::new(Context::SessionList);
        assert_eq!(p.all.len(), p.filtered.len());
        assert!(!p.all.is_empty());
        assert_eq!(p.cursor, 0);
    }

    #[test]
    fn typing_filters_actions() {
        let mut p = CommandPalette::new(Context::SessionList);
        let before = p.filtered.len();
        // Type "copy" — should narrow to copy_session_id + copy_project_path.
        for c in "copy".chars() {
            let _ = p.handle_event(Event::Key(c));
        }
        assert!(!p.filtered.is_empty(), "copy must match at least one");
        assert!(p.filtered.len() < before, "filter must narrow results");
    }

    #[test]
    fn enter_returns_execute_outcome() {
        let mut p = CommandPalette::new(Context::SessionList);
        match p.handle_event(Event::Enter) {
            Outcome::Execute(id) => {
                assert!(!id.is_empty());
            }
            other => panic!("expected Execute, got {}", outcome_label(&other)),
        }
    }

    #[test]
    fn escape_returns_close() {
        let mut p = CommandPalette::new(Context::Tree);
        match p.handle_event(Event::Escape) {
            Outcome::Close => {}
            other => panic!("expected Close, got {}", outcome_label(&other)),
        }
    }

    #[test]
    fn arrow_moves_cursor_with_wrap() {
        let mut p = CommandPalette::new(Context::Tree);
        let n = p.filtered.len();
        assert!(n >= 2, "tree palette should have at least 2 actions");
        // Down: cursor moves to 1.
        let _ = p.handle_event(Event::Down);
        assert_eq!(p.cursor, 1);
        // Up from 1 → 0.
        let _ = p.handle_event(Event::Up);
        assert_eq!(p.cursor, 0);
        // Up from 0 wraps to last.
        let _ = p.handle_event(Event::Up);
        assert_eq!(p.cursor, n - 1);
    }

    #[test]
    fn backspace_restores_previous_filter() {
        let mut p = CommandPalette::new(Context::SessionList);
        for c in "copy".chars() {
            let _ = p.handle_event(Event::Key(c));
        }
        let narrow = p.filtered.len();
        let _ = p.handle_event(Event::Backspace);
        assert!(p.filtered.len() >= narrow, "backspace should broaden");
    }

    fn outcome_label(o: &Outcome) -> &'static str {
        match o {
            Outcome::Continue => "Continue",
            Outcome::Close => "Close",
            Outcome::Execute(_) => "Execute",
        }
    }
}
