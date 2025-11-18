use crate::tui::clipboard::ClipboardManager;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::setup::setup_wizard_state::{SetupWizardResult, SetupWizardState, SetupWizardStep};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

pub struct SetupWizardHandler {
    pub response_tx: mpsc::UnboundedSender<Option<SetupWizardResult>>,
    clipboard: ClipboardManager,
}

impl SetupWizardHandler {
    pub fn new(response_tx: mpsc::UnboundedSender<Option<SetupWizardResult>>) -> Self {
        Self {
            response_tx,
            clipboard: ClipboardManager::new(),
        }
    }

    fn handle_text_input(&mut self, app: &mut SetupWizardState, event: &KeyEvent) {
        // Handle paste operations (Ctrl+V)
        if let KeyCode::Char('v') = event.code {
            let is_paste = event.modifiers.contains(KeyModifiers::CONTROL);

            if is_paste {
                if let Ok(text) = self.clipboard.get_text() {
                    match &app.current_step {
                        SetupWizardStep::ApiKeyInput => {
                            let lines: Vec<&str> = text.lines().collect();
                            for (i, line) in lines.iter().enumerate() {
                                app.api_key_input.insert_str(line);
                                if i < lines.len() - 1 {
                                    app.api_key_input.insert_newline();
                                }
                            }
                        }
                        SetupWizardStep::BaseUrlInput => {
                            let lines: Vec<&str> = text.lines().collect();
                            for (i, line) in lines.iter().enumerate() {
                                app.base_url_input.insert_str(line);
                                if i < lines.len() - 1 {
                                    app.base_url_input.insert_newline();
                                }
                            }
                        }
                        SetupWizardStep::ModelSelection => {
                            let lines: Vec<&str> = text.lines().collect();
                            for (i, line) in lines.iter().enumerate() {
                                app.model_input.insert_str(line);
                                if i < lines.len() - 1 {
                                    app.model_input.insert_newline();
                                }
                            }
                        }
                        _ => {}
                    }
                }
                return;
            }
        }

        // Default text input handling
        match &app.current_step {
            SetupWizardStep::ApiKeyInput => {
                app.api_key_input.input(*event);
            }
            SetupWizardStep::BaseUrlInput => {
                app.base_url_input.input(*event);
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
        // Handle paste events (Cmd+V on macOS sends Event::Paste)
        if let Event::Paste(text) = event {
            match &state.current_step {
                SetupWizardStep::ApiKeyInput => {
                    let lines: Vec<&str> = text.lines().collect();
                    for (i, line) in lines.iter().enumerate() {
                        state.api_key_input.insert_str(line);
                        if i < lines.len() - 1 {
                            state.api_key_input.insert_newline();
                        }
                    }
                    return KeyHandlerResult::Handled;
                }
                SetupWizardStep::BaseUrlInput => {
                    let lines: Vec<&str> = text.lines().collect();
                    for (i, line) in lines.iter().enumerate() {
                        state.base_url_input.insert_str(line);
                        if i < lines.len() - 1 {
                            state.base_url_input.insert_newline();
                        }
                    }
                    return KeyHandlerResult::Handled;
                }
                SetupWizardStep::ModelSelection => {
                    let lines: Vec<&str> = text.lines().collect();
                    for (i, line) in lines.iter().enumerate() {
                        state.model_input.insert_str(line);
                        if i < lines.len() - 1 {
                            state.model_input.insert_newline();
                        }
                    }
                    return KeyHandlerResult::Handled;
                }
                _ => return KeyHandlerResult::NotHandled,
            }
        }

        let Event::Key(key_event) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let key = key_event.code;
        let modifiers = key_event.modifiers;

        // Only intercept Ctrl+C when NOT in text input steps to allow copy operations
        let is_text_input_step = matches!(
            &state.current_step,
            SetupWizardStep::ApiKeyInput
                | SetupWizardStep::BaseUrlInput
                | SetupWizardStep::ModelSelection
        );

        if let KeyCode::Char('c') = key
            && modifiers.contains(KeyModifiers::CONTROL)
            && !is_text_input_step
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
            SetupWizardStep::BaseUrlInput => match key {
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
