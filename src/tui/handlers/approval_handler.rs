use crate::tui::app_state::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc;

pub struct ApprovalHandler {
    pub approval_response_tx: mpsc::UnboundedSender<crate::agent::ApprovalResponse>,
}

impl ApprovalHandler {
    pub fn new(
        approval_response_tx: mpsc::UnboundedSender<crate::agent::ApprovalResponse>,
    ) -> Self {
        Self {
            approval_response_tx,
        }
    }
}

#[async_trait]
impl InputHandler for ApprovalHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        _agent_task_active: bool,
    ) -> KeyHandlerResult {
        if !app.is_showing_approval_dialog() {
            return KeyHandlerResult::NotHandled;
        }

        let Event::Key(key_event) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let key = key_event.code;
        let modifiers = key_event.modifiers;
        // should_handle already checked if approval dialog is showing
        if let Some(dialog_state) = &app.approval_dialog_state {
            let tool_call_id = dialog_state.tool_call_id.clone();
            let selected_index = dialog_state.selected_index;

            // Handle Ctrl+C separately - it should cancel the entire task
            if let KeyCode::Char('c') = key
                && modifiers.contains(KeyModifiers::CONTROL)
            {
                app.hide_approval_dialog();
                app.should_cancel_task = true;
                return KeyHandlerResult::ShouldCancelTask;
            }

            let response = match key {
                KeyCode::Up | KeyCode::Down => {
                    if key == KeyCode::Up {
                        app.select_prev_approval_option();
                    } else {
                        app.select_next_approval_option();
                    }
                    None
                }
                KeyCode::Enter => {
                    // 0 = Approve, 1 = Reject
                    Some((selected_index == 0, None))
                }
                KeyCode::Char('y')
                | KeyCode::Char('Y')
                | KeyCode::Char('a')
                | KeyCode::Char('A') => Some((true, None)),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    Some((false, Some("User rejected".to_string())))
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    Some((false, Some("User requested different approach".to_string())))
                }
                _ => None,
            };

            if let Some((approved, rejection_reason)) = response {
                let approval_response = crate::agent::ApprovalResponse {
                    tool_call_id,
                    approved,
                    rejection_reason,
                };
                let _ = self.approval_response_tx.send(approval_response);
                app.hide_approval_dialog();
            }
        }

        KeyHandlerResult::Handled
    }
}
