use crate::tools::todo_write::TodoStatus;
use crate::tui::app_state::AppState;
use crate::tui::colors::palette;
use crate::tui::component::Component;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
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

        for todo in state.todos.iter() {
            let (status_icon, status_color) = match todo.status {
                TodoStatus::Pending => ("○", palette::DIMMED_TEXT),
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

            let line = Line::from(vec![
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(display_text.clone(), Style::default().fg(content_color)),
            ]);

            lines.push(line);
        }

        let title = format!(" Tasks ({}/{}) ", completed, total);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::PRIMARY_BORDER));

        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(area, buf);
    }
}
