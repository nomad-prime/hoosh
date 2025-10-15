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
    let viewport_area = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(0),
    ]);
    let [status_area, input_area, _spacer] = vertical.areas(viewport_area);

    render_status(frame, status_area, app);
    render_input(frame, input_area, app);

    if app.is_completing() {
        render_completion_popup(frame, input_area, app);
    }

    if app.is_showing_permission_dialog() {
        render_permission_dialog(frame, input_area, app);
    }
}

fn render_status(frame: &mut Frame, area: Rect, app: &AppState) {
    use super::events::AgentState;

    // 7 different braille spinner sequences for Thinking state
    let thinking_spinners = [
        &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"][..], // Classic rotation
        &["⠐", "⠒", "⠓", "⠋", "⠙", "⠹", "⠸", "⠼"][..], // Wave pattern
        &["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"][..], // Binary progression
        &["⡏", "⡟", "⡻", "⣻", "⣿", "⣯", "⣧", "⡧"][..], // Fill pattern
        &["⣷", "⣯", "⣟", "⡿", "⢿", "⠿", "⠷", "⣶"][..], // Circle pattern
        &["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠴", "⠲"][..], // Twirl pattern
        &["⢹", "⢺", "⢼", "⣸", "⣇", "⡧", "⡏", "⡃"][..], // Rotation pattern
    ];

    // 7 different braille spinner sequences for ExecutingTools state
    let executing_spinners = [
        &["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠤", "⠐"][..], // Zigzag pattern
        &["⠁", "⠉", "⠋", "⠛", "⠟", "⠿", "⠿", "⠟"][..], // Growing/shrinking
        &["⠈", "⠐", "⠠", "⠄", "⠂", "⠆", "⡆", "⡇"][..], // Pulse pattern
        &["⡀", "⡁", "⡃", "⡇", "⡧", "⡷", "⣶", "⣦"][..], // Fill/unfill
        &["⠐", "⠒", "⠖", "⠶", "⠷", "⠿", "⠻", "⠛"][..], // Ascending/descending
        &["⢀", "⢄", "⢤", "⢦", "⢧", "⢧", "⢧", "⢧"][..], // Progressive dots
        &["⣀", "⣄", "⣤", "⣦", "⣶", "⣾", "⣽", "⣻"][..], // Circle fill
    ];

    let status_text = match app.agent_state {
        AgentState::Idle => String::new(),
        AgentState::Thinking => {
            // Use the fixed spinner sequence for this thinking session
            let spinner = thinking_spinners[app.current_thinking_spinner]
                [app.animation_frame % thinking_spinners[app.current_thinking_spinner].len()];
            format!(" {} Processing", spinner)
        }
        AgentState::ExecutingTools => {
            // Use the fixed spinner sequence for this execution session
            let spinner = executing_spinners[app.current_executing_spinner]
                [app.animation_frame % executing_spinners[app.current_executing_spinner].len()];
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
        Constraint::Length(2), // Prompt area ("> ")
        Constraint::Min(1),    // Text input area
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

        let viewport_area = frame.area();
        let popup_start_y = input_area.y + input_area.height;
        let viewport_bottom = viewport_area.y + viewport_area.height;
        let available_height = viewport_bottom.saturating_sub(popup_start_y);
        let desired_height = visible_candidates.len() as u16 + 2;
        let popup_height = desired_height.min(available_height).max(3);

        let popup_width = visible_candidates
            .iter()
            .map(|c| c.len())
            .max()
            .unwrap_or(20)
            .min(60) as u16
            + 4;

        let popup_area = Rect {
            x: input_area.x,
            y: popup_start_y,
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

fn render_permission_dialog(frame: &mut Frame, input_area: Rect, app: &AppState) {
    use super::app::PermissionOption;

    if let Some(dialog_state) = &app.permission_dialog_state {
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

        // Calculate dropdown dimensions - positioned below input area
        let viewport_area = frame.area();
        let popup_start_y = input_area.y + input_area.height;
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
            x: input_area.x,
            y: popup_start_y,
            width: dialog_width,
            height: dialog_height,
        };

        let border_style = if operation.is_destructive() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Cyan)
        };

        let block = Block::default()
            .title(" Permission Required ")
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(Color::Black));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, dialog_area);
    }
}
