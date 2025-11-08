use crate::tui::app_state::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyModifiers};

pub struct QuitHandler;

impl QuitHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for QuitHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputHandler for QuitHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        agent_task_active: bool,
    ) -> KeyHandlerResult {
        let Event::Key(key) = event else {
            return KeyHandlerResult::NotHandled;
        };

        if !(key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)) {
            return KeyHandlerResult::NotHandled;
        }

        if agent_task_active {
            app.should_cancel_task = true;
            KeyHandlerResult::ShouldCancelTask
        } else {
            let input_text = app.get_input_text();
            if !input_text.is_empty() {
                app.clear_input();
                KeyHandlerResult::Handled
            } else {
                app.should_quit = true;
                KeyHandlerResult::ShouldQuit
            }
        }
    }
}
