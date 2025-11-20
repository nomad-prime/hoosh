use crate::tui::app_state::AppState;
use crate::tui::component::Component;
use crate::tui::palette;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct SubagentResultsComponent;

impl Component for SubagentResultsComponent {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        // Only render if there are active subagent tasks
        let subagent_tasks: Vec<_> = state
            .active_tool_calls
            .iter()
            .filter(|tc| tc.is_subagent_task)
            .collect();

        if subagent_tasks.is_empty() {
            return;
        }

        let mut lines = Vec::new();
        const MAX_STEPS: usize = 5;

        for tool_call in subagent_tasks {
            if tool_call.subagent_steps.is_empty() {
                continue;
            }

            let total_steps = tool_call.subagent_steps.len();
            let has_more = total_steps > MAX_STEPS;

            // Get the last step to show current budget
            let last_step = &tool_call.subagent_steps[total_steps - 1];
            let budget_pct = last_step.budget_pct;
            let budget_color = if budget_pct >= 80.0 {
                palette::DESTRUCTIVE
            } else if budget_pct >= 70.0 {
                palette::WARNING
            } else {
                palette::DIMMED_TEXT
            };

            // Add budget line at the top
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    format!("{:.0}%", budget_pct),
                    Style::default().fg(budget_color),
                ),
            ]));

            // Get the last N steps
            let start = total_steps.saturating_sub(MAX_STEPS);
            let recent_steps = &tool_call.subagent_steps[start..];

            // Show each recent step
            for (i, step) in recent_steps.iter().enumerate() {
                let prefix = if i == 0 {
                    Span::styled("  ⎿ ", Style::default().fg(palette::DIMMED_TEXT))
                } else {
                    Span::styled("    ", Style::default())
                };

                let step_spans = vec![
                    prefix,
                    Span::styled(
                        step.description.to_string(),
                        Style::default().fg(palette::DIMMED_TEXT),
                    ),
                ];

                lines.push(Line::from(step_spans));
            }

            // Show ellipsis at the bottom if there are more steps
            if has_more {
                lines.push(Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::styled("...", Style::default().fg(palette::DIMMED_TEXT)),
                ]));
            }
        }

        if !lines.is_empty() {
            let paragraph = Paragraph::new(lines);
            paragraph.render(area, buf);
        }
    }
}
