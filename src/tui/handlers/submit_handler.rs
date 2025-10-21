use crate::tui::app::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode};

pub struct SubmitHandler;

impl SubmitHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SubmitHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputHandler for SubmitHandler {
    fn should_handle(&self, event: &Event, _app: &AppState) -> bool {
        matches!(event, Event::Key(key) if key.code == KeyCode::Enter)
    }

    async fn handle_event(
        &mut self,
        _event: &Event,
        app: &mut AppState,
        agent_task_active: bool,
    ) -> anyhow::Result<KeyHandlerResult> {
        let input_text = app.get_input_text();
        if !input_text.trim().is_empty() && !agent_task_active {
            app.add_message(format!("\n> {}", input_text));
            app.add_message("\n".to_string());

            app.prompt_history.add(input_text.clone());
            app.clear_input();

            if input_text.trim().starts_with('/') {
                Ok(KeyHandlerResult::StartCommand(input_text))
            } else {
                Ok(KeyHandlerResult::StartConversation(input_text))
            }
        } else {
            Ok(KeyHandlerResult::Handled)
        }
    }
}
