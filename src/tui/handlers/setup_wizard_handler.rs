use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::setup_wizard_app::{SetupWizardApp, SetupWizardResult, SetupWizardStep};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

pub struct SetupWizardHandler {
    pub response_tx: mpsc::UnboundedSender<Option<SetupWizardResult>>,
}

impl SetupWizardHandler {
    pub fn new(response_tx: mpsc::UnboundedSender<Option<SetupWizardResult>>) -> Self {
        Self { response_tx }
    }

    fn handle_text_input(&self, app: &mut SetupWizardApp, event: &KeyEvent) {
        match &app.current_step {
            SetupWizardStep::ApiKeyInput => {
                if !app.use_env_var {
                    app.api_key_input.input(*event);
                }
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
        app: &mut SetupWizardApp,
    ) -> KeyHandlerResult {
        let Event::Key(key_event) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let key = key_event.code;
        let modifiers = key_event.modifiers;

        if let KeyCode::Char('c') = key
            && modifiers.contains(KeyModifiers::CONTROL)
        {
            app.cancel_setup();
            let _ = self.response_tx.send(None);
            return KeyHandlerResult::ShouldQuit;
        }

        match &app.current_step {
            SetupWizardStep::Welcome => match key {
                KeyCode::Enter => {
                    app.advance_step();
                }
                KeyCode::Esc => {
                    app.cancel_setup();
                    let _ = self.response_tx.send(None);
                    return KeyHandlerResult::ShouldQuit;
                }
                _ => {}
            },
            SetupWizardStep::BackendSelection => match key {
                KeyCode::Up => app.select_prev_backend(),
                KeyCode::Down => app.select_next_backend(),
                KeyCode::Enter => app.advance_step(),
                KeyCode::Esc => app.go_back(),
                _ => {}
            },
            SetupWizardStep::ApiKeyInput => match key {
                KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                    app.toggle_env_var();
                }
                KeyCode::Enter => app.advance_step(),
                KeyCode::Esc => app.go_back(),
                _ => {
                    self.handle_text_input(app, key_event);
                }
            },
            SetupWizardStep::ModelSelection => match key {
                KeyCode::Enter => app.advance_step(),
                KeyCode::Esc => app.go_back(),
                _ => {
                    self.handle_text_input(app, key_event);
                }
            },
            SetupWizardStep::Confirmation => match key {
                KeyCode::Up => app.select_prev_confirmation_option(),
                KeyCode::Down => app.select_next_confirmation_option(),
                KeyCode::Enter => {
                    if app.selected_confirmation_index == 0 {
                        app.confirm_setup();
                        let _ = self.response_tx.send(app.result.clone());
                    } else {
                        app.cancel_setup();
                        let _ = self.response_tx.send(None);
                    }
                    return KeyHandlerResult::ShouldQuit;
                }
                KeyCode::Esc => app.go_back(),
                _ => {}
            },
        }

        KeyHandlerResult::Handled
    }
}
