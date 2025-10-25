use crate::tui::app::AppState;
use crate::tui::layout_builder::WidgetRenderer;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders, Paragraph, Widget},
};
use tui_textarea::TextArea;

/// Input widget that displays the text input area with a prompt
pub struct InputWidget<'a> {
    input: &'a TextArea<'static>,
}

impl<'a> InputWidget<'a> {
    pub fn new(input: &'a TextArea<'static>) -> Self {
        Self { input }
    }
}

impl<'a> Widget for InputWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let input_widget = self.input.widget();
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

pub struct InputRenderer;

impl WidgetRenderer for InputRenderer {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        let input_widget = InputWidget::new(&state.input);
        input_widget.render(area, buf);
    }
}
