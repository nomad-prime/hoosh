use crate::tui::app_state::{AppState, PermissionOption};
use crate::tui::component::Component;
use crate::tui::palette;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

pub struct PermissionDialog;

impl Component for PermissionDialog {
    type State = AppState;

    fn render(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        if let Some(dialog_state) = &state.tool_permission_dialog_state {
            let descriptor = &dialog_state.descriptor;

            let mut lines = vec![];

            lines.push(Line::from(vec![Span::styled(
                descriptor.approval_prompt(),
                Style::default().add_modifier(Modifier::BOLD),
            )]));

            lines.push(Line::from(""));

            if let Some(preview) = descriptor.command_preview() {
                const MAX_PREVIEW_LINES: usize = 15;
                let preview_lines: Vec<&str> = preview.lines().collect();
                let total_lines = preview_lines.len();

                for preview_line in preview_lines.iter().take(MAX_PREVIEW_LINES) {
                    lines.push(Line::from(vec![
                        Span::styled(" │ ", Style::default().fg(palette::DIMMED_TEXT)),
                        Span::styled(*preview_line, Style::default().fg(palette::SECONDARY_TEXT)),
                    ]));
                }

                // Show indicator if there are more lines
                if total_lines > MAX_PREVIEW_LINES {
                    lines.push(Line::from(vec![
                        Span::styled(" │ ", Style::default().fg(palette::DIMMED_TEXT)),
                        Span::styled(
                            format!("... ({} more lines)", total_lines - MAX_PREVIEW_LINES),
                            Style::default()
                                .fg(palette::DIMMED_TEXT)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]));
                }

                lines.push(Line::from(""));
            }

            for (idx, option) in dialog_state.options.iter().enumerate() {
                let is_selected = idx == dialog_state.selected_index;
                let (key, label) = match option {
                    PermissionOption::YesOnce => ("y", "Yes, once ".to_string()),
                    PermissionOption::No => ("n", "No ".to_string()),
                    PermissionOption::TrustProject(_) => (
                        "t",
                        format!("yes, and {} ", descriptor.persistent_approval()),
                    ),
                };

                let prefix = if is_selected { "> " } else { "  " };
                let text = format!("{}[{}] {}", prefix, key, label);

                let style = if is_selected {
                    Style::default()
                        .fg(palette::SELECTED_FG)
                        .bg(palette::SELECTED_BG)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                lines.push(Line::from(Span::styled(text, style)));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ navigate, Enter/key to choose, Esc cancel",
                Style::default().fg(palette::PRIMARY_BORDER),
            )));

            let border_style = if descriptor.is_destructive() {
                Style::default().fg(palette::DESTRUCTIVE)
            } else {
                Style::default().fg(palette::PRIMARY_BORDER)
            };

            let block = Block::default()
                .title(descriptor.approval_title())
                .borders(Borders::ALL)
                .border_style(border_style)
                .style(Style::default().bg(palette::DIALOG_BG));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false });

            paragraph.render(area, buf);
        }
    }
}
