use crate::tui::app::AppState;
use crate::tui::component::Component;
use crate::tui::events::AgentState;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct Status;

impl Component for Status {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        let thinking_spinners = [
            &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"][..], // Classic rotation
            &["⠐", "⠒", "⠓", "⠋", "⠙", "⠹", "⠸", "⠼"][..], // Wave pattern
            &["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"][..], // Binary progression
            &["⡏", "⡟", "⡻", "⣻", "⣿", "⣯", "⣧", "⡧"][..], // Fill pattern
            &["⣷", "⣯", "⣟", "⡿", "⢿", "⠿", "⠷", "⣶"][..], // Circle pattern
            &["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠴", "⠲"][..], // Twirl pattern
            &["⢹", "⢺", "⢼", "⣸", "⣇", "⡧", "⡏", "⡃"][..], // Rotation pattern
        ];

        let executing_spinners = [
            &["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠤", "⠐"][..], // Zigzag pattern
            &["⠁", "⠉", "⠋", "⠛", "⠟", "⠿", "⠿", "⠟"][..], // Growing/shrinking
            &["⠈", "⠐", "⠠", "⠄", "⠂", "⠆", "⡆", "⡇"][..], // Pulse pattern
            &["⡀", "⡁", "⡃", "⡇", "⡧", "⡷", "⣶", "⣦"][..], // Fill/unfill
            &["⠐", "⠒", "⠖", "⠶", "⠷", "⠿", "⠻", "⠛"][..], // Ascending/descending
            &["⢀", "⢄", "⢤", "⢦", "⢧", "⢧", "⢧", "⢧"][..], // Progressive dots
            &["⣀", "⣄", "⣤", "⣦", "⣶", "⣾", "⣽", "⣻"][..], // Circle fill
        ];

        let waiting_spinners = ["⠄", "⠂", "⠁", "⠂"];
        let retry_spinners = ["⠈", "⠐", "⠠", "⠄", "⠂", "⠆", "⡆", "⡇"];

        let (status_text, status_color) = if let Some(retry_status) = &state.current_retry_status {
            let retry_spinner = retry_spinners[state.animation_frame % retry_spinners.len()];
            (format!(" {} {}", retry_spinner, retry_status), Color::Red)
        } else if state.is_showing_permission_dialog() || state.is_showing_approval_dialog() {
            let waiting_spinner = waiting_spinners[state.animation_frame % waiting_spinners.len()];
            (format!(" {} Your turn", waiting_spinner), Color::Yellow)
        } else {
            match state.agent_state {
                AgentState::Summarizing => {
                    let spinner = thinking_spinners[state.current_thinking_spinner][state
                        .animation_frame
                        % thinking_spinners[state.current_thinking_spinner].len()];
                    (
                        format!(" {} Summarizing", spinner),
                        Color::Rgb(142, 240, 204),
                    )
                }
                AgentState::Idle => (String::new(), Color::Rgb(142, 240, 204)),
                AgentState::Thinking => {
                    let spinner = thinking_spinners[state.current_thinking_spinner][state
                        .animation_frame
                        % thinking_spinners[state.current_thinking_spinner].len()];
                    (
                        format!(" {} Processing", spinner),
                        Color::Rgb(142, 240, 204),
                    )
                }
                AgentState::ExecutingTools => {
                    let spinner = executing_spinners[state.current_executing_spinner][state
                        .animation_frame
                        % executing_spinners[state.current_executing_spinner].len()];
                    (
                        format!(" {} Executing tools", spinner),
                        Color::Rgb(142, 240, 204),
                    )
                }
            }
        };

        if !status_text.is_empty() {
            let status_line =
                Line::from(Span::styled(status_text, Style::default().fg(status_color)));
            let paragraph = Paragraph::new(status_line);
            paragraph.render(area, buf);
        } else {
            let paragraph = Paragraph::new(Line::from(" "));
            paragraph.render(area, buf);
        }
    }
}
