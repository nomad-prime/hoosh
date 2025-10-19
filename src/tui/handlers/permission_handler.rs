use crate::tui::app::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::mpsc;

pub struct PermissionHandler {
    pub permission_response_tx: mpsc::UnboundedSender<crate::conversations::PermissionResponse>,
}

impl PermissionHandler {
    pub fn new(
        permission_response_tx: mpsc::UnboundedSender<crate::conversations::PermissionResponse>,
    ) -> Self {
        Self {
            permission_response_tx,
        }
    }
}

#[async_trait]
impl InputHandler for PermissionHandler {
    fn should_handle(&self, event: &Event, app: &AppState) -> bool {
        matches!(event, Event::Key(_)) && app.is_showing_permission_dialog()
    }

    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        _agent_task_active: bool,
    ) -> anyhow::Result<KeyHandlerResult> {
        let Event::Key(key_event) = event else {
            return Ok(KeyHandlerResult::Handled);
        };

        let key = key_event.code;
        let modifiers = key_event.modifiers;
        // should_handle already checked if permission dialog is showing
        if let Some(dialog_state) = &app.permission_dialog_state {
            let operation = dialog_state.operation.clone();
            let request_id = dialog_state.request_id.clone();
            let selected_option = dialog_state
                .options
                .get(dialog_state.selected_index)
                .cloned();

            // Handle Ctrl+C separately - it should cancel the entire task
            if let KeyCode::Char('c') = key {
                if modifiers.contains(KeyModifiers::CONTROL) {
                    app.hide_permission_dialog();
                    app.should_cancel_task = true;
                    return Ok(KeyHandlerResult::ShouldCancelTask);
                }
            }

            let response = match key {
                KeyCode::Up => {
                    app.select_prev_permission_option();
                    None
                }
                KeyCode::Down => {
                    app.select_next_permission_option();
                    None
                }
                KeyCode::Enter => selected_option.as_ref().map(|opt| match opt {
                    crate::tui::app::PermissionOption::YesOnce => (true, None),
                    crate::tui::app::PermissionOption::No => (false, None),
                    crate::tui::app::PermissionOption::AlwaysForFile => {
                        let target = operation.target().to_string();
                        (
                            true,
                            Some(crate::permissions::PermissionScope::Specific(target)),
                        )
                    }
                    crate::tui::app::PermissionOption::AlwaysForDirectory(dir) => (
                        true,
                        Some(crate::permissions::PermissionScope::Directory(dir.clone())),
                    ),
                    crate::tui::app::PermissionOption::AlwaysForType => {
                        (true, Some(crate::permissions::PermissionScope::Global))
                    }
                    crate::tui::app::PermissionOption::TrustProject(project_path) => (
                        true,
                        Some(crate::permissions::PermissionScope::ProjectWide(
                            project_path.clone(),
                        )),
                    ),
                }),
                KeyCode::Char('y') | KeyCode::Char('Y') => Some((true, None)),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some((false, None)),
                KeyCode::Char('a') => {
                    let target = operation.target().to_string();
                    Some((
                        true,
                        Some(crate::permissions::PermissionScope::Specific(target)),
                    ))
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    if let Some(dir) = operation.parent_directory() {
                        Some((
                            true,
                            Some(crate::permissions::PermissionScope::Directory(dir)),
                        ))
                    } else {
                        let target = operation.target().to_string();
                        Some((
                            true,
                            Some(crate::permissions::PermissionScope::Specific(target)),
                        ))
                    }
                }
                KeyCode::Char('A') => {
                    Some((true, Some(crate::permissions::PermissionScope::Global)))
                }
                KeyCode::Char('T') => {
                    if let Ok(current_dir) = std::env::current_dir() {
                        Some((
                            true,
                            Some(crate::permissions::PermissionScope::ProjectWide(
                                current_dir,
                            )),
                        ))
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some((allowed, scope)) = response {
                // Update app state if ProjectWide scope was selected
                if let Some(crate::permissions::PermissionScope::ProjectWide(ref path)) = scope {
                    if allowed {
                        app.set_trusted_project(path.clone());
                        app.add_message("Project trusted for this session. \n".to_string());
                    }
                }

                let perm_response = crate::conversations::PermissionResponse {
                    request_id,
                    allowed,
                    scope,
                };
                let _ = self.permission_response_tx.send(perm_response);
                app.hide_permission_dialog();
            }
        }

        Ok(KeyHandlerResult::Handled)
    }
}
