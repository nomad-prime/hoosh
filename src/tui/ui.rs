use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use super::app::AppState;
use super::events::AgentState;

pub fn render(frame: &mut Frame, app: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // Status bar
            Constraint::Min(0),      // Messages
            Constraint::Length(3),   // Input
        ])
        .split(frame.area());

    render_status_bar(frame, chunks[0], &app.agent_state);
    render_messages(frame, chunks[1], app);
    render_input(frame, chunks[2], app);
}

fn render_status_bar(frame: &mut Frame, area: Rect, agent_state: &AgentState) {
    let status_text = match agent_state {
        AgentState::Idle => "Ready",
        AgentState::Thinking => "Thinking...",
        AgentState::ExecutingTools => "Executing tools...",
    };

    let status_color = match agent_state {
        AgentState::Idle => Color::Green,
        AgentState::Thinking => Color::Yellow,
        AgentState::ExecutingTools => Color::Blue,
    };

    let status = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("🚀 Hoosh", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" | "),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title("Status"));

    frame.render_widget(status, area);
}

fn render_messages(frame: &mut Frame, area: Rect, app: &AppState) {
    let messages: Vec<ListItem> = app
        .messages
        .iter()
        .map(|msg| {
            if msg.is_empty() {
                ListItem::new(Line::from(""))
            } else {
                ListItem::new(Line::from(msg.clone()))
            }
        })
        .collect();

    let messages_list = List::new(messages)
        .block(Block::default().borders(Borders::ALL).title("Conversation"));

    frame.render_widget(messages_list, area);
}

fn render_input(frame: &mut Frame, area: Rect, app: &mut AppState) {
    let input_widget = app.input.widget();
    let input_block = Block::default()
        .borders(Borders::ALL)
        .title("Input (Ctrl+C to quit, Enter to send)");

    let inner_area = input_block.inner(area);
    frame.render_widget(input_block, area);
    frame.render_widget(input_widget, inner_area);
}
