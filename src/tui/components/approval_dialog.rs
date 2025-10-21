use crate::tui::app::AppState;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

/// Approval dialog widget that shows tool approval requests
pub struct ApprovalDialogWidget<'a> {
    app_state: &'a AppState,
    anchor_area: Rect,
}

impl<'a> ApprovalDialogWidget<'a> {
    pub fn new(app_state: &'a AppState, anchor_area: Rect) -> Self {
        Self {
            app_state,
            anchor_area,
        }
    }
}

impl<'a> Widget for ApprovalDialogWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if let Some(dialog_state) = &self.app_state.approval_dialog_state {
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

            // Calculate dialog dimensions - positioned below anchor area
            let viewport_area = area;
            let popup_start_y = self.anchor_area.y + self.anchor_area.height;
            let viewport_bottom = viewport_area.y + viewport_area.height;
            let available_height = viewport_bottom.saturating_sub(popup_start_y);

            let max_width = lines
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.len()).sum::<usize>())
                .max()
                .unwrap_or(50) as u16;

            let dialog_width = (max_width + 4).min(viewport_area.width.saturating_sub(4));
            let desired_height = lines.len() as u16 + 2;
            let dialog_height = desired_height.min(available_height).max(5);

            let dialog_area = Rect {
                x: self.anchor_area.x,
                y: popup_start_y,
                width: dialog_width,
                height: dialog_height,
            };

            // Clear the area first to prevent text bleed-through
            Clear.render(dialog_area, buf);

            let block = Block::default()
                .title(" Approval Required ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false });

            paragraph.render(dialog_area, buf);
        }
    }
}
