use crate::tui::app_state::{AppState, InitialPermissionChoice};
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc;

pub struct InitialPermissionHandler {
    pub response_tx: mpsc::UnboundedSender<InitialPermissionChoice>,
}

impl InitialPermissionHandler {
    pub fn new(response_tx: mpsc::UnboundedSender<InitialPermissionChoice>) -> Self {
        Self { response_tx }
    }
}

#[async_trait]
impl InputHandler for InitialPermissionHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        _agent_task_active: bool,
    ) -> KeyHandlerResult {
        if !app.is_showing_initial_permission_dialog() {
            return KeyHandlerResult::NotHandled;
        }

        let Event::Key(key_event) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let key = key_event.code;
        let modifiers = key_event.modifiers;

        if let KeyCode::Char('c') = key
            && modifiers.contains(KeyModifiers::CONTROL)
        {
            app.hide_initial_permission_dialog();
            app.should_quit = true;
            return KeyHandlerResult::ShouldQuit;
        }

        let choice = match key {
            KeyCode::Up => {
                app.select_prev_initial_permission_option();
                None
            }
            KeyCode::Down => {
                app.select_next_initial_permission_option();
                None
            }
            KeyCode::Enter => app.get_selected_initial_permission_choice(),
            KeyCode::Char('1') => Some(InitialPermissionChoice::ReadOnly),
            KeyCode::Char('2') => Some(InitialPermissionChoice::EnableWriteEdit),
            KeyCode::Char('3') | KeyCode::Esc => Some(InitialPermissionChoice::Deny),
            _ => None,
        };

        if let Some(choice) = choice {
            app.hide_initial_permission_dialog();
            let _ = self.response_tx.send(choice);
        }

        KeyHandlerResult::Handled
    }
}
