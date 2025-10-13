use super::app::AppState;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, app: &mut AppState) {
    let vertical = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]);
    let [messages_area, input_area] = vertical.areas(frame.area());

    render_messages(frame, messages_area, app);
    render_input(frame, input_area, app);
}

fn render_messages(frame: &mut Frame, area: Rect, app: &mut AppState) {
    // Update viewport height for scroll calculations
    app.viewport_height = area.height;

    // On first render, scroll to bottom after viewport height is set
    if !app.initial_scroll_done {
        app.scroll_to_bottom();
        app.initial_scroll_done = true;
    }

    // Create lines from all messages
    let lines: Vec<Line> = app
        .messages
        .iter()
        .flat_map(|message| {
            message
                .lines()
                .map(|line| Line::from(Span::raw(line.to_string())))
                .collect::<Vec<_>>()
        })
        .collect();

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::new())
        .scroll((app.scroll_offset, 0));

    frame.render_widget(paragraph, area);
}

fn render_input(frame: &mut Frame, area: Rect, app: &mut AppState) {
    let input_widget = app.input.widget();
    let input_block = Block::default().borders(Borders::BOTTOM | Borders::TOP);

    let inner_area = input_block.inner(area);
    frame.render_widget(input_block, area);
    frame.render_widget(input_widget, inner_area);
}
