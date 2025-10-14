use super::app::AppState;
use crate::permissions::OperationType;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn render(frame: &mut Frame, app: &mut AppState) {
    // Inline viewport: only render status and input area
    // Messages are inserted above using terminal.insert_before()
    let vertical = Layout::vertical([
        Constraint::Length(1), // Status line
        Constraint::Length(3), // Input area (fixed height)
    ]);
    let [status_area, input_area] = vertical.areas(frame.area());

    render_status(frame, status_area, app);
    render_input(frame, input_area, app);

    if app.is_completing() {
        render_completion_popup(frame, input_area, app);
    }

    if app.is_showing_permission_dialog() {
        render_permission_dialog(frame, app);
    }
}

fn render_status(frame: &mut Frame, area: Rect, app: &AppState) {
    use super::events::AgentState;

    let status_text = match app.agent_state {
        AgentState::Idle => String::new(),
        AgentState::Thinking => {
            let spinner =
                &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][app.animation_frame % 10];
            format!(" {} Processing", spinner)
        }
        AgentState::ExecutingTools => {
            let spinner =
                &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][app.animation_frame % 10];
            format!(" {} Executing tools", spinner)
        }
    };

    if !status_text.is_empty() {
        let status_line = Line::from(vec![Span::styled(
            status_text,
            Style::default().fg(Color::Rgb(142, 240, 204)),
        )]);

        let paragraph = Paragraph::new(status_line);
        frame.render_widget(paragraph, area);
    }
}

fn render_input(frame: &mut Frame, area: Rect, app: &mut AppState) {
    let input_widget = app.input.widget();
    let input_block = Block::default().borders(Borders::BOTTOM | Borders::TOP);

    let inner_area = input_block.inner(area);
    frame.render_widget(input_block, area);

    // Split the inner area to add prompt
    let horizontal = Layout::horizontal([
        Constraint::Length(2),  // Prompt area ("> ")
        Constraint::Min(1),     // Text input area
    ]);
    let [prompt_area, text_area] = horizontal.areas(inner_area);

    // Render the prompt
    let prompt = Paragraph::new("> ");
    frame.render_widget(prompt, prompt_area);

    // Render the input
    frame.render_widget(input_widget, text_area);
}

fn render_completion_popup(frame: &mut Frame, input_area: Rect, app: &AppState) {
    if let Some(completion_state) = &app.completion_state {
        if completion_state.candidates.is_empty() {
            return;
        }

        let max_items = 10;
        let visible_candidates =
            &completion_state.candidates[..completion_state.candidates.len().min(max_items)];

        let items: Vec<ListItem> = visible_candidates
            .iter()
            .enumerate()
            .map(|(idx, candidate)| {
                let is_selected = idx == completion_state.selected_index;
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let prefix = if is_selected { "> " } else { "  " };
                ListItem::new(format!("{}{}", prefix, candidate)).style(style)
            })
            .collect();

        let popup_height = (visible_candidates.len() as u16 + 2).min(12);
        let popup_width = visible_candidates
            .iter()
            .map(|c| c.len())
            .max()
            .unwrap_or(20)
            .min(60) as u16
            + 4;

        let popup_x = input_area.x;
        let popup_y = if input_area.y > popup_height {
            input_area.y - popup_height
        } else {
            input_area.y + input_area.height
        };

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };

        let block = Block::default()
            .title(" File Completion ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let list = List::new(items).block(block);

        frame.render_widget(list, popup_area);
    }
}

fn render_permission_dialog(frame: &mut Frame, app: &AppState) {
    use super::app::PermissionOption;

    if let Some(dialog_state) = &app.permission_dialog_state {
        let operation = &dialog_state.operation;

        // Build the dialog content
        let mut lines = vec![];

        // Title
        let warning_emoji = if operation.is_destructive() {
            "⚠️ "
        } else {
            "🔒 "
        };
        let title_style = if operation.is_destructive() {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        };

        lines.push(Line::from(vec![Span::styled(
            format!("{}Permission Required", warning_emoji),
            title_style,
        )]));
        lines.push(Line::from(""));

        // Operation description
        lines.push(Line::from(vec![
            Span::styled("Operation: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(operation.description()),
        ]));
        lines.push(Line::from(""));

        // Destructive warning
        if operation.is_destructive() {
            lines.push(Line::from(vec![Span::styled(
                "⚠️  WARNING: This operation may be destructive!",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]));
            lines.push(Line::from(""));
        }

        // Options
        lines.push(Line::from(Span::styled(
            "Options:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
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
                    ("d", format!("Always for this directory ({})", dir))
                }
                PermissionOption::AlwaysForType => (
                    "A",
                    format!("Always for all {} operations", operation.operation_kind()),
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
            "Use ↑/↓ to navigate, Enter or key letter to choose, Esc to cancel",
            Style::default().fg(Color::Cyan),
        )));

        // Calculate dialog dimensions
        let max_width = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.len()).sum::<usize>())
            .max()
            .unwrap_or(50) as u16;

        let dialog_width = (max_width + 4).min(frame.area().width - 4);
        let dialog_height = (lines.len() as u16 + 2).min(frame.area().height - 4);

        // Center the dialog
        let dialog_x = (frame.area().width.saturating_sub(dialog_width)) / 2;
        let dialog_y = (frame.area().height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x: dialog_x,
            y: dialog_y,
            width: dialog_width,
            height: dialog_height,
        };

        let border_style = if operation.is_destructive() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Yellow)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(Color::Black));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, dialog_area);
    }
}
