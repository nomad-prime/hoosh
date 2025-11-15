use crate::tui::app_state::AppState;
use crate::tui::component::Component;
use crate::tui::events::AgentState;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct StatusBar;

impl Component for StatusBar {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        let thinking_spinners = [
            &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"][..],
            &["⠐", "⠒", "⠓", "⠋", "⠙", "⠹", "⠸", "⠼"][..],
            &["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"][..],
            &["⡏", "⡟", "⡻", "⣻", "⣿", "⣯", "⣧", "⡧"][..],
            &["⣷", "⣯", "⣟", "⡿", "⢿", "⠿", "⠷", "⣶"][..],
            &["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠴", "⠲"][..],
            &["⢹", "⢺", "⢼", "⣸", "⣇", "⡧", "⡏", "⡃"][..],
        ];

        let executing_spinners = [
            &["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠤", "⠐"][..],
            &["⠁", "⠉", "⠋", "⠛", "⠟", "⠿", "⠿", "⠟"][..],
            &["⠈", "⠐", "⠠", "⠄", "⠂", "⠆", "⡆", "⡇"][..],
            &["⡀", "⡁", "⡃", "⡇", "⡧", "⡷", "⣶", "⣦"][..],
            &["⠐", "⠒", "⠖", "⠶", "⠷", "⠿", "⠻", "⠛"][..],
            &["⢀", "⢄", "⢤", "⢦", "⢧", "⢧", "⢧", "⢧"][..],
            &["⣀", "⣄", "⣤", "⣦", "⣶", "⣾", "⣽", "⣻"][..],
        ];

        let waiting_spinners = ["⠄", "⠂", "⠁", "⠂"];
        let retry_spinners = ["⠈", "⠐", "⠠", "⠄", "⠂", "⠆", "⡆", "⡇"];

        let (status_text, status_color) = if let Some(retry_status) = &state.current_retry_status {
            let retry_spinner = retry_spinners[state.animation_frame % retry_spinners.len()];
            (format!("{} {}", retry_spinner, retry_status), Color::LightRed)
        } else if state.is_showing_tool_permission_dialog() || state.is_showing_approval_dialog() {
            let waiting_spinner = waiting_spinners[state.animation_frame % waiting_spinners.len()];
            (format!("{} Your turn", waiting_spinner), Color::Yellow)
        } else {
            match state.agent_state {
                AgentState::Summarizing => {
                    let spinner = thinking_spinners[state.current_thinking_spinner][state
                        .animation_frame
                        % thinking_spinners[state.current_thinking_spinner].len()];
                    (
                        format!("{} Summarizing", spinner),
                        Color::Rgb(142, 240, 204),
                    )
                }
                AgentState::Idle => (String::new(), Color::Rgb(142, 240, 204)),
                AgentState::Thinking => {
                    let spinner = thinking_spinners[state.current_thinking_spinner][state
                        .animation_frame
                        % thinking_spinners[state.current_thinking_spinner].len()];
                    (format!("{} Processing", spinner), Color::Rgb(142, 240, 204))
                }
                AgentState::ExecutingTools => {
                    let spinner = executing_spinners[state.current_executing_spinner][state
                        .animation_frame
                        % executing_spinners[state.current_executing_spinner].len()];
                    (
                        format!("{} Executing tools", spinner),
                        Color::Rgb(142, 240, 204),
                    )
                }
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

        let token_color = Color::LightCyan;

        let areas = Layout::horizontal([Constraint::Fill(1), Constraint::Length(36)]).split(area);

        if !status_text.is_empty() {
            let status_line =
                Line::from(Span::styled(status_text, Style::default().fg(status_color)));
            let paragraph = Paragraph::new(status_line);
            paragraph.render(areas[0], buf);
        }

        let token_line = Line::from(Span::styled(token_text, Style::default().fg(token_color)));
        let paragraph = Paragraph::new(token_line).right_aligned();
        paragraph.render(areas[1], buf);
    }
}
