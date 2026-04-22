//! First-run onboarding tour (#13).
//!
//! On the very first launch — detected by the absence of
//! `~/.config/claude-picker/.seen_tour` via [`crate::theme::is_first_run`] —
//! the picker drops a lightweight 3-step overlay in front of the user.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::events::Event;
use crate::theme::Theme;

pub const STEP_COUNT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Dismiss,
}

#[derive(Debug, Clone)]
pub struct OnboardingState {
    pub step: usize,
    pub top_session: Option<(String, f64)>,
    pub skipped: bool,
}

impl OnboardingState {
    pub fn new() -> Self {
        Self { step: 0, top_session: None, skipped: false }
    }

    pub fn with_top_session(mut self, top: Option<(String, f64)>) -> Self {
        self.top_session = top;
        self
    }

    pub fn is_last_step(&self) -> bool {
        self.step + 1 >= STEP_COUNT
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Outcome {
        if self.is_last_step() {
            Outcome::Dismiss
        } else {
            self.step += 1;
            Outcome::Continue
        }
    }

    pub fn back(&mut self) {
        if self.step > 0 {
            self.step -= 1;
        }
    }

    pub fn skip(&mut self) -> Outcome {
        self.skipped = true;
        Outcome::Dismiss
    }

    pub fn handle_event(&mut self, ev: Event) -> Outcome {
        match ev {
            Event::Enter | Event::Key('n') => self.next(),
            Event::Key('b') => {
                self.back();
                Outcome::Continue
            }
            Event::Key('s') | Event::Escape => self.skip(),
            _ => Outcome::Continue,
        }
    }
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn step_headline(state: &OnboardingState) -> String {
    match state.step {
        0 => match &state.top_session {
            Some((title, cost)) => format!(
                "Your most expensive session this month: {title} \u{00B7} ${cost:.2} \u{2014} press Enter to open it."
            ),
            None => "Press Enter on any session to resume it \u{2014} claude-picker remembers the working directory.".to_string(),
        },
        1 => "Search with a DSL: model:opus cost:>1 \u{2014} press / anywhere.".to_string(),
        _ => "Press ? to see all keybindings. Press any key to dismiss.".to_string(),
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &OnboardingState, theme: &Theme) {
    let w = 70u16.min(area.width.saturating_sub(4));
    let h = 7u16.min(area.height.saturating_sub(2));
    if w < 30 || h < 5 {
        return;
    }
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + 2;
    let rect = Rect { x, y, width: w, height: h };
    frame.render_widget(Clear, rect);
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("welcome", Style::default().fg(theme.mauve).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" \u{00B7} step {}/{} ", state.step + 1, STEP_COUNT), theme.muted()),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.mauve))
        .title(title);
    let headline = step_headline(state);
    let next_label = if state.is_last_step() { "finish" } else { "next" };
    let hints = Line::from(vec![
        Span::raw(" "),
        Span::styled("Enter", theme.key_hint()),
        Span::styled(format!(" {next_label}   "), theme.key_desc()),
        Span::styled("b", theme.key_hint()),
        Span::styled(" back   ", theme.key_desc()),
        Span::styled("s", theme.key_hint()),
        Span::styled(" skip", theme.key_desc()),
    ]);
    let lines = vec![
        Line::raw(""),
        Line::from(vec![Span::raw(" "), Span::styled(headline, Style::default().fg(theme.text))]),
        Line::raw(""),
        hints,
    ];
    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true }).alignment(Alignment::Left);
    frame.render_widget(paragraph, rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_creates_onboarding_state() {
        let state = OnboardingState::new();
        assert_eq!(state.step, 0);
        assert!(!state.skipped);
        assert!(!state.is_last_step());
        let seeded = OnboardingState::new().with_top_session(Some(("Optimize COPY".to_string(), 42.5)));
        assert_eq!(seeded.step, 0);
        assert!(step_headline(&seeded).contains("Optimize COPY"));
    }

    #[test]
    fn third_step_dismisses_and_marks_seen() {
        let mut state = OnboardingState::new();
        assert_eq!(state.handle_event(Event::Enter), Outcome::Continue);
        assert_eq!(state.step, 1);
        assert_eq!(state.handle_event(Event::Enter), Outcome::Continue);
        assert_eq!(state.step, 2);
        assert!(state.is_last_step());
        assert_eq!(state.handle_event(Event::Enter), Outcome::Dismiss);
        assert!(!state.skipped);
    }

    #[test]
    fn skip_jumps_to_marker_write() {
        let mut state = OnboardingState::new();
        assert_eq!(state.handle_event(Event::Key('s')), Outcome::Dismiss);
        assert!(state.skipped);

        let mut state = OnboardingState::new();
        state.handle_event(Event::Enter);
        assert_eq!(state.handle_event(Event::Escape), Outcome::Dismiss);
        assert!(state.skipped);
    }

    #[test]
    fn back_does_not_underflow_at_step_zero() {
        let mut state = OnboardingState::new();
        state.back();
        assert_eq!(state.step, 0);
        state.handle_event(Event::Enter);
        assert_eq!(state.step, 1);
        state.handle_event(Event::Key('b'));
        assert_eq!(state.step, 0);
    }

    #[test]
    fn n_key_advances_like_enter() {
        let mut state = OnboardingState::new();
        state.handle_event(Event::Key('n'));
        assert_eq!(state.step, 1);
    }

    #[test]
    fn unknown_event_is_harmless() {
        let mut state = OnboardingState::new();
        assert_eq!(state.handle_event(Event::Key('z')), Outcome::Continue);
        assert_eq!(state.step, 0);
    }

    #[test]
    fn step_headline_falls_back_without_top_session() {
        let state = OnboardingState::new();
        assert!(step_headline(&state).contains("resume"));
    }
}
