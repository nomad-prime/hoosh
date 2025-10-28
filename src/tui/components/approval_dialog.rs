use crate::tui::app::AppState;
use crate::tui::component::Component;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

pub struct ApprovalDialog;

impl Component for ApprovalDialog {
    type State = AppState;

    fn render(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        if let Some(dialog_state) = &state.approval_dialog_state {
            // Build the dialog content
            let mut lines = vec![];

            // Tool name header
            lines.push(Line::from(vec![
                Span::styled("Tool: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&dialog_state.tool_name),
            ]));

            lines.push(Line::from(""));

            // Options
            let options = [("y", "Approve"), ("n", "Reject")];

            for (idx, (key, label)) in options.iter().enumerate() {
                let is_selected = idx == dialog_state.selected_index;
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
                "↑/↓ navigate, Enter/y approve, n/Esc reject",
                Style::default().fg(Color::Cyan),
            )));

            Clear.render(area, buf);

            let block = Block::default()
                .title(" Approval Required ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false });

            paragraph.render(area, buf);
        }
    }
}
