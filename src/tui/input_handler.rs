use crate::tui::app::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crossterm::event::Event;

#[async_trait::async_trait]
pub trait InputHandler {
    /// Handles the event and returns the result.
    /// Should return KeyHandlerResult::NotHandled if this handler doesn't want to process the event.
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        agent_task_active: bool,
    ) -> KeyHandlerResult;
}
