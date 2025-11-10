use crate::tui::app_state::{AppState, ToolCallStatus};
use crate::tui::component::Component;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct ActiveToolCallsComponent;

impl Component for ActiveToolCallsComponent {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        if state.active_tool_calls.is_empty() {
            return;
        }

        let mut lines = Vec::new();

        for tool_call in &state.active_tool_calls {
            let status_indicator = match &tool_call.status {
                ToolCallStatus::Starting => Span::styled("○", Style::default().fg(Color::Gray)),
                ToolCallStatus::AwaitingApproval => {
                    Span::styled("◎", Style::default().fg(Color::Yellow))
                }
                ToolCallStatus::Executing => Span::styled("●", Style::default().fg(Color::Cyan)),
                ToolCallStatus::Completed => Span::styled("✓", Style::default().fg(Color::Green)),
                ToolCallStatus::Error(_) => Span::styled("✗", Style::default().fg(Color::Red)),
            };

            let mut spans = vec![
                status_indicator,
                Span::raw(" "),
                Span::raw(&tool_call.display_name),
            ];

            match &tool_call.status {
                ToolCallStatus::AwaitingApproval => {
                    spans.push(Span::styled(
                        " [Awaiting Approval]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ));
                }
                ToolCallStatus::Error(err) => {
                    spans.push(Span::styled(
                        format!(" [Error: {}]", err),
                        Style::default().fg(Color::Red),
                    ));
                }
                _ => {}
            }

            lines.push(Line::from(spans));

            if let Some(summary) = &tool_call.result_summary {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("⎿ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(summary, Style::default().fg(Color::Gray)),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }
}
