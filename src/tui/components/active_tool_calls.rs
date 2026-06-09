use crate::tui::app_state::{AppState, ToolCallStatus};
use crate::tui::component::Component;
use crate::tui::{glyphs, palette};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
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
            // Static glyphs are padded to three cells so the tool-name column
            // stays aligned with the animated executing rows.
            let status_indicator = match &tool_call.status {
                ToolCallStatus::Starting => Span::styled(
                    glyphs::TOOL_STARTING,
                    Style::default().fg(palette::TOOL_STATUS_STARTING),
                ),
                ToolCallStatus::AwaitingApproval => Span::styled(
                    glyphs::TOOL_AWAITING,
                    Style::default().fg(palette::TOOL_STATUS_RUNNING),
                ),
                ToolCallStatus::Executing => {
                    let sweep = glyphs::TOOL_EXECUTING_SWEEP;
                    let frame = sweep[state.animation_frame % sweep.len()];
                    Span::styled(frame, Style::default().fg(palette::TOOL_STATUS_EXECUTING))
                }
                ToolCallStatus::Completed => Span::styled(
                    glyphs::TOOL_COMPLETED,
                    Style::default().fg(palette::TOOL_STATUS_COMPLETED),
                ),
                ToolCallStatus::Error(_) => Span::styled(
                    glyphs::TOOL_ERROR,
                    Style::default().fg(palette::TOOL_STATUS_ERROR),
                ),
            };

            let timer = tool_call.elapsed_time();

            let meta_info = match tool_call.budget_pct {
                Some(pct) => format!(" {} • {:.0}% done", timer, pct),
                None => format!(" {}", timer),
            };

            let mut spans = vec![
                status_indicator,
                Span::raw(" "),
                Span::raw(&tool_call.display_name),
                Span::styled(meta_info, Style::default().fg(palette::SUBDUED_TEXT)),
            ];

            match &tool_call.status {
                ToolCallStatus::AwaitingApproval => {
                    spans.push(Span::styled(
                        " [Awaiting Approval]",
                        Style::default()
                            .fg(palette::WARNING)
                            .add_modifier(Modifier::ITALIC),
                    ));
                }
                ToolCallStatus::Error(err) => {
                    spans.push(Span::styled(
                        format!(" [Error: {}]", err),
                        Style::default().fg(palette::DESTRUCTIVE),
                    ));
                }
                _ => {}
            }

            lines.push(Line::from(spans));

            if let Some(summary) = &tool_call.result_summary {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("⎿ ", Style::default().fg(palette::SUBDUED_TEXT)),
                    Span::styled(summary, Style::default().fg(palette::SECONDARY_TEXT)),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }
}
