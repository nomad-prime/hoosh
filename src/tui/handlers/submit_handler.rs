use crate::tui::app_state::AppState;
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
            let expanded_input = app.expand_attachments(&input_text);

            app.add_user_input(&expanded_input);

            app.prompt_history.add(expanded_input.clone());
            app.clear_input();
            app.clear_attachments();
            app.quit_armed = false;

            if expanded_input.trim().starts_with('/') {
                // Slash commands are synchronous; no agent turn to restore.
                app.last_submitted_input = None;
                KeyHandlerResult::StartCommand(expanded_input)
            } else {
                app.last_submitted_input = Some(expanded_input.clone());
                KeyHandlerResult::StartConversation(expanded_input)
            }
        } else {
            KeyHandlerResult::Handled
        }
    }
}
