use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::setup::setup_wizard_state::{SetupWizardResult, SetupWizardState, SetupWizardStep};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

pub struct SetupWizardHandler {
    pub response_tx: mpsc::UnboundedSender<Option<SetupWizardResult>>,
}

impl SetupWizardHandler {
    pub fn new(response_tx: mpsc::UnboundedSender<Option<SetupWizardResult>>) -> Self {
        Self { response_tx }
    }

    fn handle_text_input(&self, app: &mut SetupWizardState, event: &KeyEvent) {
        match &app.current_step {
            SetupWizardStep::ApiKeyInput => {
                app.api_key_input.input(*event);
            }
            SetupWizardStep::ModelSelection => {
                app.model_input.input(*event);
            }
            _ => {}
        }
    }

    pub async fn handle_event(
        &mut self,
        event: &Event,
        state: &mut SetupWizardState,
    ) -> KeyHandlerResult {
        let Event::Key(key_event) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let key = key_event.code;
        let modifiers = key_event.modifiers;

        if let KeyCode::Char('c') = key
            && modifiers.contains(KeyModifiers::CONTROL)
        {
            state.cancel_setup();
            let _ = self.response_tx.send(None);
            return KeyHandlerResult::ShouldQuit;
        }

        match &state.current_step {
            SetupWizardStep::Welcome => match key {
                KeyCode::Enter => {
                    state.advance_step();
                }
                KeyCode::Esc => {
                    state.cancel_setup();
                    let _ = self.response_tx.send(None);
                    return KeyHandlerResult::ShouldQuit;
                }
                _ => {}
            },
            SetupWizardStep::BackendSelection => match key {
                KeyCode::Up => state.select_prev_backend(),
                KeyCode::Down => state.select_next_backend(),
                KeyCode::Enter => state.advance_step(),
                KeyCode::Esc => state.go_back(),
                _ => {}
            },
            SetupWizardStep::ApiKeyInput => match key {
                KeyCode::Enter => state.advance_step(),
                KeyCode::Esc => state.go_back(),
                _ => {
                    self.handle_text_input(state, key_event);
                }
            },
            SetupWizardStep::ModelSelection => match key {
                KeyCode::Enter => state.advance_step(),
                KeyCode::Esc => state.go_back(),
                _ => {
                    self.handle_text_input(state, key_event);
                }
            },
            SetupWizardStep::Confirmation => match key {
                KeyCode::Up => state.select_prev_confirmation_option(),
                KeyCode::Down => state.select_next_confirmation_option(),
                KeyCode::Enter => {
                    if state.selected_confirmation_index == 0 {
                        state.confirm_setup();
                        let _ = self.response_tx.send(state.result.clone());
                    } else {
                        state.cancel_setup();
                        let _ = self.response_tx.send(None);
                    }
                    return KeyHandlerResult::ShouldQuit;
                }
                KeyCode::Esc => state.go_back(),
                _ => {}
            },
        }

        KeyHandlerResult::Handled
    }
}
