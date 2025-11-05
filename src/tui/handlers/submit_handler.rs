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
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        agent_task_active: bool,
    ) -> KeyHandlerResult {
        let Event::Key(key) = event else {
            return KeyHandlerResult::NotHandled;
        };

        if key.code != KeyCode::Enter {
            return KeyHandlerResult::NotHandled;
        }

        let input_text = app.get_input_text();
        if !input_text.trim().is_empty() && !agent_task_active {
            app.add_user_input(&input_text);

            app.prompt_history.add(input_text.clone());
            app.clear_input();

            if input_text.trim().starts_with('/') {
                KeyHandlerResult::StartCommand(input_text)
            } else {
                KeyHandlerResult::StartConversation(input_text)
            }
        } else {
            KeyHandlerResult::Handled
        }
    }
}
