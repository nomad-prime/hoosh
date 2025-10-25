use crate::tui::app::AppState;
use crate::tui::events::AgentState;
use crate::tui::layout_builder::WidgetRenderer;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

/// Status widget that displays the current agent state with animated spinners
pub struct StatusWidget<'a> {
    app_state: &'a AppState,
}

impl<'a> StatusWidget<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { app_state }
    }
}

impl<'a> Widget for StatusWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
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

        let (status_text, status_color) =
            if let Some(retry_status) = &self.app_state.current_retry_status {
                let retry_spinner =
                    retry_spinners[self.app_state.animation_frame % retry_spinners.len()];
                (format!(" {} {}", retry_spinner, retry_status), Color::Red)
            } else if self.app_state.is_showing_permission_dialog()
                || self.app_state.is_showing_approval_dialog()
            {
                let waiting_spinner =
                    waiting_spinners[self.app_state.animation_frame % waiting_spinners.len()];
                (format!(" {} Your turn", waiting_spinner), Color::Yellow)
            } else {
                match self.app_state.agent_state {
                    AgentState::Summarizing => {
                        let spinner = thinking_spinners[self.app_state.current_thinking_spinner]
                            [self.app_state.animation_frame
                                % thinking_spinners[self.app_state.current_thinking_spinner].len()];
                        (
                            format!(" {} Summarizing", spinner),
                            Color::Rgb(142, 240, 204),
                        )
                    }
                    AgentState::Idle => (String::new(), Color::Rgb(142, 240, 204)),
                    AgentState::Thinking => {
                        let spinner = thinking_spinners[self.app_state.current_thinking_spinner]
                            [self.app_state.animation_frame
                                % thinking_spinners[self.app_state.current_thinking_spinner].len()];
                        (
                            format!(" {} Processing", spinner),
                            Color::Rgb(142, 240, 204),
                        )
                    }
                    AgentState::ExecutingTools => {
                        let spinner = executing_spinners[self.app_state.current_executing_spinner]
                            [self.app_state.animation_frame
                                % executing_spinners[self.app_state.current_executing_spinner]
                                    .len()];
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
        }
    }
}

pub struct StatusRenderer;

impl WidgetRenderer for StatusRenderer {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        let status_widget = StatusWidget::new(state);
        status_widget.render(area, buf);
    }
}
