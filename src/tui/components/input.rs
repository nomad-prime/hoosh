use crate::tui::app::AppState;
use crate::tui::component::Component;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct Input;

impl Component for Input {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        let input_widget = state.input.widget();
        let input_block = Block::default().borders(Borders::BOTTOM | Borders::TOP);

        let inner_area = input_block.inner(area);
        input_block.render(area, buf);

        // Split the inner area to add prompt
        let horizontal = Layout::horizontal([
            Constraint::Length(2), // Prompt area ("> ")
            Constraint::Min(1),    // Text input area
        ]);
        let [prompt_area, text_area] = horizontal.areas(inner_area);

        // Render the prompt
        let prompt = Paragraph::new("> ");
        prompt.render(prompt_area, buf);

        // Render the input
        input_widget.render(text_area, buf);
    }
}
