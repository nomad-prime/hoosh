use super::app::AppState;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, app: &mut AppState) {
    let vertical = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(3),
        Constraint::Length(3),
    ]);
    let [messages_area, input_area, _bottom_padding] = vertical.areas(frame.area());

    render_messages(frame, messages_area, app);
    render_input(frame, input_area, app);

    if app.is_completing() {
        render_completion_popup(frame, input_area, app);
    }
}

fn render_messages(frame: &mut Frame, area: Rect, app: &mut AppState) {
    use super::app::MessageLine;

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
        .flat_map(|message| match message {
            MessageLine::Plain(text) => text
                .lines()
                .map(|line| Line::from(Span::raw(line.to_string())))
                .collect::<Vec<_>>(),
            MessageLine::Styled(line) => vec![line.clone()],
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
