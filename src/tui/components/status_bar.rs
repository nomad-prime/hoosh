use crate::tui::component::Component;
use crate::tui::events::AgentState;
use crate::tui::palette;
use crate::tui::state::AppState;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct StatusBar;

/// Build a subtle "radio wave" pattern: a row of short vertical bars whose
/// heights ripple sideways like the equalizer on a radio. Two ripples of
/// different periods are layered so crests land at uneven intervals, giving
/// the row an organic, irregular pulse rather than a uniform sweep.
fn radio_wave(frame: usize) -> String {
    const LEVELS: [char; 4] = ['⣀', '⣤', '⣶', '⣿'];
    const WIDTH: usize = 3;

    let triangle = |phase: usize, period: usize| {
        let half = period / 2;
        let t = phase % period;
        if t <= half { t } else { period - t }
    };

    let mut out = String::with_capacity(WIDTH * 4);
    for cell in 0..WIDTH {
        let fast = triangle(frame + cell * 3, 6);
        let slow = triangle(frame + cell * 5, 14);
        let level = (fast + slow) / 4;
        let glyph = LEVELS
            .get(level.min(LEVELS.len() - 1))
            .copied()
            .unwrap_or('⣀');
        out.push(glyph);
    }
    out
}

impl Component for StatusBar {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        let retry_spinners = ["⠈", "⠐", "⠠", "⠄", "⠂", "⠆", "⡆", "⡇"];

        let (status_text, status_color) = if let Some(retry_status) = &state.current_retry_status {
            let retry_spinner = retry_spinners[state.animation.frame % retry_spinners.len()];
            (
                format!("{} {}", retry_spinner, retry_status),
                palette::DESTRUCTIVE,
            )
        } else if state.is_showing_tool_permission_dialog() || state.is_showing_approval_dialog() {
            (radio_wave(state.animation.frame), palette::STATUS_WAITING)
        } else {
            match state.agent_state {
                AgentState::Idle => {
                    // Show "Todos" when idle and there are todos
                    if !state.todos.is_empty() {
                        ("Todos".to_string(), palette::STATUS_TODOS)
                    } else {
                        (String::new(), palette::STATUS_IDLE)
                    }
                }
                AgentState::Thinking if state.visible_streaming_text().is_some() => {
                    // The streamed text is the feedback; drop the spinner.
                    (String::new(), palette::STATUS_PROCESSING)
                }
                AgentState::Thinking | AgentState::ExecutingTools => (
                    radio_wave(state.animation.frame),
                    palette::STATUS_PROCESSING,
                ),
            }
        };

        let token_text = if state.metrics.has_usage() {
            format!(
                "Tokens: {} ↑ | {} ↓ (${:.4}) ",
                state.metrics.input_tokens, state.metrics.output_tokens, state.metrics.total_cost
            )
        } else {
            "Tokens: 0 ↑ | 0 ↓ ".to_string()
        };

        let token_color = palette::INFO;

        let mode_text = if state.display_compact {
            "mode: compact"
        } else {
            "mode: full"
        };

        let areas = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(36),
        ])
        .split(area);

        if !status_text.is_empty() {
            let status_line =
                Line::from(Span::styled(status_text, Style::default().fg(status_color)));
            let paragraph = Paragraph::new(status_line);
            paragraph.render(areas[0], buf);
        }

        let mode_line = Line::from(Span::styled(
            mode_text,
            Style::default().fg(palette::SUBDUED_TEXT),
        ));
        let paragraph = Paragraph::new(mode_line).right_aligned();
        paragraph.render(areas[1], buf);

        let token_line = Line::from(Span::styled(token_text, Style::default().fg(token_color)));
        let paragraph = Paragraph::new(token_line).right_aligned();
        paragraph.render(areas[2], buf);
    }
}
