use crate::tui::app::AppState;
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

#[async_trait]
impl InputHandler for QuitHandler {
    fn should_handle(&self, event: &Event, _app: &AppState) -> bool {
        if let Event::Key(key) = event {
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
        } else {
            false
        }
    }

    async fn handle_event(
        &mut self,
        _event: &Event,
        app: &mut AppState,
        agent_task_active: bool,
    ) -> anyhow::Result<KeyHandlerResult> {
        if agent_task_active {
            app.should_cancel_task = true;
            Ok(KeyHandlerResult::ShouldCancelTask)
        } else {
            let input_text = app.get_input_text();
            if !input_text.is_empty() {
                app.clear_input();
                Ok(KeyHandlerResult::Handled)
            } else {
                app.should_quit = true;
                Ok(KeyHandlerResult::ShouldQuit)
            }
        }
    }
}
