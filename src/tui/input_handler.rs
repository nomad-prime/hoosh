use crate::tui::app::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crossterm::event::Event;

#[async_trait::async_trait]
pub trait InputHandler {
    /// Determines if this handler should process the given event based on the current app state.
    /// Returns true if this handler wants to handle the event.
    fn should_handle(&self, event: &Event, app: &AppState) -> bool;

    /// Handles the event and returns the result.
    /// This will only be called if should_handle returned true.
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        agent_task_active: bool,
    ) -> anyhow::Result<KeyHandlerResult>;
}
