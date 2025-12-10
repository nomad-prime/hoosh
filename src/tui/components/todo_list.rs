use crate::tools::todo_write::TodoStatus;
use crate::tui::app_state::AppState;
use crate::tui::colors::palette;
use crate::tui::component::Component;
use crate::tui::events::AgentState;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct TodoListComponent;

impl Component for TodoListComponent {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        if state.todos.is_empty() {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        // Count completed and total for title
        let completed = state
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::Completed)
            .count();
        let total = state.todos.len();

        // Check if status bar has content (not idle and no dialogs)
        let has_status = state.current_retry_status.is_some()
            || state.is_showing_tool_permission_dialog()
            || state.is_showing_approval_dialog()
            || !matches!(state.agent_state, AgentState::Idle);

        for (idx, todo) in state.todos.iter().enumerate() {
            let (status_icon, status_color) = match todo.status {
                TodoStatus::Pending => ("○", palette::SUBDUED_TEXT),
                TodoStatus::InProgress => ("◐", palette::TOOL_STATUS_RUNNING),
                TodoStatus::Completed => ("●", palette::SUCCESS),
            };

            let content_color = match todo.status {
                TodoStatus::Completed => palette::SECONDARY_TEXT,
                TodoStatus::InProgress => palette::PRIMARY_TEXT,
                TodoStatus::Pending => palette::SECONDARY_TEXT,
            };

            // Show active form for in_progress, content otherwise
            let display_text = if todo.status == TodoStatus::InProgress {
                &todo.active_form
            } else {
                &todo.content
            };

            // Use ⎿ for the first item only if there's status content, otherwise no prefix
            let prefix = if idx == 0 && has_status { "⎿ " } else { "  " };

            let line = Line::from(vec![
                Span::styled(prefix, Style::default().fg(palette::SUBDUED_TEXT)),
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(display_text.clone(), Style::default().fg(content_color)),
                Span::styled(
                    if idx == 0 {
                        format!(" ({}/{})", completed, total)
                    } else {
                        String::new()
                    },
                    Style::default().fg(palette::SUBDUED_TEXT),
                ),
            ]);

            lines.push(line);
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }
}
