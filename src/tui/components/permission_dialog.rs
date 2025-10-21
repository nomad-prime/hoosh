use crate::permissions::OperationType;
use crate::tui::app::{AppState, PermissionOption};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

/// Permission dialog widget that shows permission requests
pub struct PermissionDialogWidget<'a> {
    app_state: &'a AppState,
    anchor_area: Rect,
}

impl<'a> PermissionDialogWidget<'a> {
    pub fn new(app_state: &'a AppState, anchor_area: Rect) -> Self {
        Self {
            app_state,
            anchor_area,
        }
    }
}

impl<'a> Widget for PermissionDialogWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if let Some(dialog_state) = &self.app_state.permission_dialog_state {
            let operation = &dialog_state.operation;

            // Build the dialog content
            let mut lines = vec![];

            // Operation description
            lines.push(Line::from(vec![
                Span::styled("Operation: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(operation.description()),
            ]));

            // Destructive warning
            if operation.is_destructive() {
                lines.push(Line::from(vec![Span::styled(
                    "⚠️  WARNING: Destructive!",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]));
            }

            lines.push(Line::from(""));

            // Render each option with selection highlight
            for (idx, option) in dialog_state.options.iter().enumerate() {
                let is_selected = idx == dialog_state.selected_index;
                let (key, label) = match option {
                    PermissionOption::YesOnce => ("y", "Yes, once".to_string()),
                    PermissionOption::No => ("n", "No".to_string()),
                    PermissionOption::AlwaysForFile => {
                        let label = match operation {
                            OperationType::ExecuteBash(_) => "Always for this command",
                            _ => "Always for this file",
                        };
                        ("a", label.to_string())
                    }
                    PermissionOption::AlwaysForDirectory(dir) => {
                        ("d", format!("Always for dir ({})", dir))
                    }
                    PermissionOption::AlwaysForType => (
                        "A",
                        format!("Always for all {}", operation.operation_kind()),
                    ),
                    PermissionOption::TrustProject(project_path) => (
                        "T",
                        format!("Trust entire project ({})", project_path.display()),
                    ),
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

            // Calculate dropdown dimensions - positioned below anchor area
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

            let border_style = if operation.is_destructive() {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Cyan)
            };

            // Clear the area first to prevent text bleed-through
            Clear.render(dialog_area, buf);

            let block = Block::default()
                .title(" Permission Required ")
                .borders(Borders::ALL)
                .border_style(border_style)
                .style(Style::default().bg(Color::Black));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false });

            paragraph.render(dialog_area, buf);
        }
    }
}
