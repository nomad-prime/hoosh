use crate::tui::app_state::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::Event;

pub struct PasteHandler;

impl PasteHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PasteHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputHandler for PasteHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        _agent_task_active: bool,
    ) -> KeyHandlerResult {
        let Event::Paste(text) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            app.input.insert_str(line);
            if i < lines.len() - 1 {
                app.input.insert_newline();
            }
        }

        KeyHandlerResult::Handled
    }
}
