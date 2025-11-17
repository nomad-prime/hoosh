use crate::tui::component::Component;
use crate::tui::setup::setup_wizard_state::{BackendType, SetupWizardState, SetupWizardStep};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

pub struct SetupWizardDialog;

impl SetupWizardDialog {
    fn render_welcome(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "Welcome to Hoosh Setup Wizard!",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from("  • Select your preferred LLM backend"),
            Line::from("  • Configure API credentials"),
            Line::from("  • Choose your default model"),
            Line::from(""),
            Line::from("You can rerun this wizard using 'hoosh setup'"),
            Line::from("Or reconfigure these settings using 'hoosh config'"),
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled(
                "Press Enter to continue, Esc to skip setup",
                Style::default().fg(Color::Yellow),
            )),
        ];

        let block = Block::default()
            .title(" Setup Wizard ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }

    fn render_backend_selection(&self, state: &SetupWizardState, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "Select LLM Backend",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ];

        let backends = BackendType::all_backends();
        for (idx, backend) in backends.iter().enumerate() {
            let is_selected = idx == state.selected_backend_index;
            let prefix = if is_selected { "> " } else { "  " };
            let text = format!("{}{}", prefix, backend.as_str());

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
                    format!("    {}", backend.description()),
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::ITALIC),
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "↑/↓ navigate, Enter to select, Esc to cancel",
            Style::default().fg(Color::Cyan),
        )));

        let block = Block::default()
            .title(" Backend Selection ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }

    fn render_api_key_input(&self, state: &SetupWizardState, area: Rect, buf: &mut Buffer) {
        let backend_name = state
            .selected_backend
            .as_ref()
            .map(|b| b.as_str())
            .unwrap_or("unknown");

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                format!("Configure {} API Key", backend_name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(Span::styled(
                "API Key:",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        let widget = state.api_key_input.widget();
        widget.render(
            Rect {
                x: area.x + 2,
                y: area.y + lines.len() as u16 + 1,
                width: area.width.saturating_sub(4),
                height: 1,
            },
            buf,
        );
        lines.push(Line::from(""));
        lines.push(Line::from(""));

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Enter to continue, Esc to go back",
            Style::default().fg(Color::Cyan),
        )));

        let block = Block::default()
            .title(" API Key Configuration ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }

    fn render_model_selection(&self, state: &SetupWizardState, area: Rect, buf: &mut Buffer) {
        let backend_name = state
            .selected_backend
            .as_ref()
            .map(|b| b.as_str())
            .unwrap_or("unknown");

        let default_model = state
            .selected_backend
            .as_ref()
            .map(|b| b.default_model())
            .unwrap_or("");

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                format!("Select Model for {}", backend_name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(format!("Default: {}", default_model)),
            Line::from(""),
            Line::from(Span::styled(
                "Model:",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        let widget = state.model_input.widget();
        widget.render(
            Rect {
                x: area.x + 2,
                y: area.y + lines.len() as u16 + 1,
                width: area.width.saturating_sub(4),
                height: 1,
            },
            buf,
        );
        lines.push(Line::from(""));
        lines.push(Line::from(""));

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Enter to continue, Esc to go back",
            Style::default().fg(Color::Cyan),
        )));

        let block = Block::default()
            .title(" Model Selection ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }

    fn render_confirmation(&self, state: &SetupWizardState, area: Rect, buf: &mut Buffer) {
        let backend_name = state
            .selected_backend
            .as_ref()
            .map(|b| b.as_str())
            .unwrap_or("unknown");

        let model = state
            .model_input
            .lines()
            .first()
            .map(|s| s.as_str())
            .unwrap_or("");

        let api_key_status = if state.api_key_input.lines()[0].is_empty() {
            "Not set".to_string()
        } else {
            "Set".to_string()
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "Confirm Configuration",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Backend: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(backend_name),
            ]),
            Line::from(vec![
                Span::styled("Model: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(model),
            ]),
            Line::from(vec![
                Span::styled("API Key: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(api_key_status),
            ]),
            Line::from(""),
            Line::from(""),
        ];

        let options = [("Save", 0), ("Cancel", 1)];

        for (label, idx) in options {
            let is_selected = idx == state.selected_confirmation_index;
            let prefix = if is_selected { "> " } else { "  " };
            let text = format!("{}{}", prefix, label);

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
            "↑/↓ navigate, Enter to confirm, Esc to go back",
            Style::default().fg(Color::Cyan),
        )));

        let block = Block::default()
            .title(" Confirmation ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }
}

impl Component for SetupWizardDialog {
    type State = SetupWizardState;

    fn render(&self, state: &SetupWizardState, area: Rect, buf: &mut Buffer) {
        match state.current_step {
            SetupWizardStep::Welcome => self.render_welcome(area, buf),
            SetupWizardStep::BackendSelection => self.render_backend_selection(state, area, buf),
            SetupWizardStep::ApiKeyInput => self.render_api_key_input(state, area, buf),
            SetupWizardStep::ModelSelection => self.render_model_selection(state, area, buf),
            SetupWizardStep::Confirmation => self.render_confirmation(state, area, buf),
        }
    }
}
