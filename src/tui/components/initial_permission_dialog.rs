use crate::tui::app::AppState;
use crate::tui::component::Component;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

pub struct InitialPermissionDialog;

impl Component for InitialPermissionDialog {
    type State = AppState;

    fn render(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        if let Some(dialog_state) = &state.initial_permission_dialog_state {
            let mut lines = vec![];

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Project: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(dialog_state.project_path.display().to_string()),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from("Choose initial permission level:"));
            lines.push(Line::from(""));

            let options = vec![
                ("1", "Read-Only Access", "Grant read permissions only"),
                (
                    "2",
                    "Trust Project",
                    "Grant all permissions (use with caution!)",
                ),
                ("3", "Deny", "Exit without granting permissions"),
            ];

            for (idx, (key, label, desc)) in options.iter().enumerate() {
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

                if is_selected {
                    lines.push(Line::from(Span::styled(
                        format!("    {}", desc),
                        Style::default()
                            .fg(Color::LightYellow)
                            .add_modifier(Modifier::ITALIC),
                    )));
                }
            }

            lines.push(Line::from(""));

            if dialog_state.selected_index == 1 {
                lines.push(Line::from(Span::styled(
                    "⚠️  Warning: Trusting a project grants permission for all file operations,",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(Span::styled(
                    "   bash commands, and network access. Only trust projects you control.",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
            }

            lines.push(Line::from(Span::styled(
                "↑/↓ navigate, Enter/key to choose, Esc cancel",
                Style::default().fg(Color::Cyan),
            )));

            let border_style = if dialog_state.selected_index == 1 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Cyan)
            };

            let block = Block::default()
                .title(" First time opening this project ")
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
