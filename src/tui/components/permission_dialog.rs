use crate::tui::app::{AppState, PermissionOption};
use crate::tui::component::Component;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

pub struct PermissionDialog;

impl Component for PermissionDialog {
    type State = AppState;

    fn render(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        if let Some(dialog_state) = &state.permission_dialog_state {
            let operation = &dialog_state.operation;

            let mut lines = vec![];

            lines.push(Line::from(vec![Span::styled(
                operation.approval_prompt(),
                Style::default().add_modifier(Modifier::BOLD),
            )]));

            if operation.is_destructive() {
                lines.push(Line::from(vec![Span::styled(
                    "⚠️  WARNING: Destructive!",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]));
            }

            lines.push(Line::from(""));

            for (idx, option) in dialog_state.options.iter().enumerate() {
                let is_selected = idx == dialog_state.selected_index;
                let (key, label) = match option {
                    PermissionOption::YesOnce => ("y", "Yes, once".to_string()),
                    PermissionOption::No => ("n", "No".to_string()),
                    PermissionOption::TrustProject(_) => {
                        ("t", format!("yes, and {}", operation.persistent_approval()))
                    }
                };

                let prefix = if is_selected { "> " } else { "  " };
                let text = format!("{}[{}] {}", prefix, key, label);

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                lines.push(Line::from(Span::styled(text, style)));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ navigate, Enter/key to choose, Esc cancel",
                Style::default().fg(Color::Cyan),
            )));

            let border_style = if operation.is_destructive() {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Cyan)
            };

            let block = Block::default()
                .title(operation.approval_title().clone())
                .borders(Borders::ALL)
                .border_style(border_style)
                .style(Style::default().bg(Color::Black));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false });

            paragraph.render(area, buf);
        }
    }
}
