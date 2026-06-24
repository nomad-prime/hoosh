use crate::tui::app_state::AppState;
use crate::tui::component::Component;
use crate::tui::events::AgentState;
use crate::tui::palette;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct StatusBar;

/// Build a subtle "radio wave" pattern: a row of short vertical bars whose
/// heights ripple sideways like the equalizer on a radio. Heights stay small
/// so the animation reads as a quiet pulse rather than a loud bar graph.
fn radio_wave(frame: usize) -> String {
    // Braille dot glyphs that rise in height, so the wave looks subtle and flat.
    const LEVELS: [char; 4] = ['⣀', '⣤', '⣶', '⣿'];
    const WIDTH: usize = 9;

    let mut out = String::with_capacity(WIDTH * 4);
    for cell in 0..WIDTH {
        // A travelling sine-like ripple: each cell is phase-shifted so the
        // crest drifts smoothly across the row.
        let phase = frame + cell * 2;
        // Triangle wave over 8 steps gives a gentle rise/fall: 0..=4..=0.
        let tri = phase % 8;
        let level = if tri <= 4 { tri } else { 8 - tri };
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
            let retry_spinner = retry_spinners[state.animation_frame % retry_spinners.len()];
            (
                format!("{} {}", retry_spinner, retry_status),
                palette::DESTRUCTIVE,
            )
        } else if state.is_showing_tool_permission_dialog() || state.is_showing_approval_dialog() {
            (radio_wave(state.animation_frame), palette::STATUS_WAITING)
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
                AgentState::Thinking | AgentState::ExecutingTools => (
                    radio_wave(state.animation_frame),
                    palette::STATUS_PROCESSING,
                ),
            }
        };

        let token_text = if state.input_tokens > 0 || state.output_tokens > 0 {
            format!(
                "Tokens: {} ↑ | {} ↓ (${:.4}) ",
                state.input_tokens, state.output_tokens, state.total_cost
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
