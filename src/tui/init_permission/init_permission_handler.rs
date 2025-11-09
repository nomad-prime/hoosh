use crate::tui::handler_result::KeyHandlerResult;
use super::init_permission_state::{InitialPermissionChoice, InitialPermissionState};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc;

pub struct InitialPermissionHandler {
    pub response_tx: mpsc::UnboundedSender<Option<InitialPermissionChoice>>,
}

impl InitialPermissionHandler {
    pub fn new(response_tx: mpsc::UnboundedSender<Option<InitialPermissionChoice>>) -> Self {
        Self { response_tx }
    }

    pub async fn handle_event(
        &mut self,
        event: &Event,
        state: &mut InitialPermissionState,
    ) -> KeyHandlerResult {
        let Event::Key(key_event) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let key = key_event.code;
        let modifiers = key_event.modifiers;

        if let KeyCode::Char('c') = key
            && modifiers.contains(KeyModifiers::CONTROL)
        {
            state.should_quit = true;
            let _ = self.response_tx.send(None);
            return KeyHandlerResult::ShouldQuit;
        }

        let choice = match key {
            KeyCode::Up => {
                state.select_prev();
                None
            }
            KeyCode::Down => {
                state.select_next();
                None
            }
            KeyCode::Enter => Some(state.get_selected_choice()),
            KeyCode::Char('1') => Some(InitialPermissionChoice::ReadOnly),
            KeyCode::Char('2') => Some(InitialPermissionChoice::EnableWriteEdit),
            KeyCode::Char('3') | KeyCode::Esc => Some(InitialPermissionChoice::Deny),
            _ => None,
        };

        if let Some(choice) = choice {
            state.should_quit = true;
            let _ = self.response_tx.send(Some(choice));
            return KeyHandlerResult::ShouldQuit;
        }

        KeyHandlerResult::Handled
    }
}
