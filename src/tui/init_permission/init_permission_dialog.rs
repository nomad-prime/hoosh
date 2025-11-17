use super::init_permission_state::InitialPermissionState;
use crate::tui::component::Component;
use crate::tui::palette;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

pub struct InitialPermissionDialog;

impl Component for InitialPermissionDialog {
    type State = InitialPermissionState;

    fn render(&self, state: &InitialPermissionState, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Project: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(state.project_root.display().to_string()),
            ]),
            Line::from(""),
            Line::from("Choose initial permission level:"),
            Line::from(""),
        ];

        let options = [
            (
                "1",
                "Read Only",
                "For exploring code base (no write/edit or bash tools)",
            ),
            (
                "2",
                "Enable Write/Edit",
                "All write/edit will be allowed for this project",
            ),
            ("3", "Deny", "Exit without granting permissions"),
        ];

        for (idx, (key, label, desc)) in options.iter().enumerate() {
            let is_selected = idx == state.selected_index;
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

            if is_selected {
                lines.push(Line::from(Span::styled(
                    format!("    {}", desc),
                    Style::default()
                        .fg(palette::WARNING)
                        .add_modifier(Modifier::ITALIC),
                )));
            }
        }

        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            "↑/↓ navigate, Enter/key to choose, Esc cancel",
            Style::default().fg(palette::PRIMARY_BORDER),
        )));

        let border_style = if state.selected_index == 1 {
            Style::default().fg(palette::WARNING)
        } else {
            Style::default().fg(palette::PRIMARY_BORDER)
        };

        let block = Block::default()
            .title(" First time opening this project ")
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(palette::DIALOG_BG));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }
}
