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

pub struct BashResultsComponent;

impl Component for BashResultsComponent {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        // Only render if there are active bash streaming tasks
        let bash_tasks: Vec<_> = state
            .active_tool_calls
            .iter()
            .filter(|tc| tc.is_bash_streaming)
            .collect();

        if bash_tasks.is_empty() {
            return;
        }

        let mut lines = Vec::new();
        const MAX_LINES: usize = 5;

        for tool_call in bash_tasks {
            if tool_call.bash_output_lines.is_empty() {
                continue;
            }

            let total_lines = tool_call.bash_output_lines.len();
            let has_more = total_lines > MAX_LINES;

            // Show ellipsis at the top if there are more lines (like terminal scrolling down)
            if has_more {
                lines.push(Line::from(vec![
                    Span::styled("  ⎿ ", Style::default().fg(palette::SECONDARY_TEXT)),
                    Span::styled("...", Style::default().fg(palette::SECONDARY_TEXT)),
                ]));
            }

            // Get the last N lines
            let start = total_lines.saturating_sub(MAX_LINES);
            let recent_lines = &tool_call.bash_output_lines[start..];

            // Show each recent line
            for (i, line) in recent_lines.iter().enumerate() {
                let (prefix_text, style) = if line.stream_type == "stderr" {
                    if i == 0 && !has_more {
                        ("  ⎿  ", Style::default().fg(palette::DESTRUCTIVE))
                    } else {
                        ("     ", Style::default().fg(palette::DESTRUCTIVE))
                    }
                } else if i == 0 && !has_more {
                    ("  ⎿ ", Style::default().fg(palette::SECONDARY_TEXT))
                } else {
                    ("    ", Style::default().fg(palette::SECONDARY_TEXT))
                };

                // Truncate long lines
                let content = if line.content.len() > 80 {
                    format!("{}...", &line.content[..77])
                } else {
                    line.content.clone()
                };

                lines.push(Line::from(vec![
                    Span::styled(prefix_text, style),
                    Span::styled(content, style),
                ]));
            }
        }

        if !lines.is_empty() {
            let paragraph = Paragraph::new(lines);
            paragraph.render(area, buf);
        }
    }
}
