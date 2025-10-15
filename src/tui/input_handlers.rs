use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::conversations::PermissionResponse;
use crate::permissions::PermissionScope;

use super::app::AppState;
use super::app::PermissionOption;

pub enum KeyHandlerResult {
    Handled,
    NotHandled,
    ShouldQuit,
    StartCommand(String),
    StartConversation(String),
}

pub fn handle_permission_keys(
    key: KeyCode,
    app: &mut AppState,
    permission_response_tx: &mpsc::UnboundedSender<PermissionResponse>,
) -> KeyHandlerResult {
    if !app.is_showing_permission_dialog() {
        return KeyHandlerResult::NotHandled;
    }

    if let Some(dialog_state) = &app.permission_dialog_state {
        let operation = dialog_state.operation.clone();
        let request_id = dialog_state.request_id.clone();
        let selected_option = dialog_state
            .options
            .get(dialog_state.selected_index)
            .cloned();

        let response = match key {
            KeyCode::Up => {
                app.select_prev_permission_option();
                None
            }
            KeyCode::Down => {
                app.select_next_permission_option();
                None
            }
            KeyCode::Enter => {
                selected_option.as_ref().and_then(|opt| match opt {
                    PermissionOption::YesOnce => Some((true, None)),
                    PermissionOption::No => Some((false, None)),
                    PermissionOption::AlwaysForFile => {
                        let target = operation.target().to_string();
                        Some((true, Some(PermissionScope::Specific(target))))
                    }
                    PermissionOption::AlwaysForDirectory(dir) => {
                        Some((true, Some(PermissionScope::Directory(dir.clone()))))
                    }
                    PermissionOption::AlwaysForType => Some((true, Some(PermissionScope::Global))),
                })
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => Some((true, None)),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some((false, None)),
            KeyCode::Char('a') => {
                let target = operation.target().to_string();
                Some((true, Some(PermissionScope::Specific(target))))
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(dir) = operation.parent_directory() {
                    Some((true, Some(PermissionScope::Directory(dir))))
                } else {
                    let target = operation.target().to_string();
                    Some((true, Some(PermissionScope::Specific(target))))
                }
            }
            KeyCode::Char('A') => Some((true, Some(PermissionScope::Global))),
            _ => None,
        };

        if let Some((allowed, scope)) = response {
            let perm_response = PermissionResponse {
                request_id,
                allowed,
                scope,
            };
            let _ = permission_response_tx.send(perm_response);
            app.hide_permission_dialog();
        }
    }

    KeyHandlerResult::Handled
}

pub async fn handle_completion_keys(key: KeyCode, app: &mut AppState) -> KeyHandlerResult {
    if !app.is_completing() {
        return KeyHandlerResult::NotHandled;
    }

    match key {
        KeyCode::Up => {
            app.select_prev_completion();
            KeyHandlerResult::Handled
        }
        KeyCode::Down => {
            app.select_next_completion();
            KeyHandlerResult::Handled
        }
        KeyCode::Tab | KeyCode::Enter => {
            // Save the completer index before applying completion
            let completer_idx = app.completion_state.as_ref().map(|s| s.completer_index);
            let input_text = app.get_input_text();

            if let Some(selected) = app.apply_completion() {
                if let Some(idx) = completer_idx {
                    if let Some(completer) = app.completers.get(idx) {
                        if let Some(trigger_pos) = completer.find_trigger_position(&input_text) {
                            let new_text =
                                completer.apply_completion(&input_text, trigger_pos, &selected);

                            // Clear and rebuild the input with the new text
                            app.clear_input();

                            // Insert each line, adding newlines between them
                            let lines: Vec<&str> = new_text.lines().collect();
                            for (i, line) in lines.iter().enumerate() {
                                app.input.insert_str(line);
                                // Add newline after each line except the last
                                if i < lines.len() - 1 {
                                    app.input.insert_newline();
                                }
                            }
                        }
                    }
                }
            }
            KeyHandlerResult::Handled
        }
        KeyCode::Esc => {
            app.cancel_completion();
            KeyHandlerResult::Handled
        }
        KeyCode::Backspace => {
            app.input.input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            let input_text = app.get_input_text();
            let completer_idx = app.completion_state.as_ref().map(|s| s.completer_index);

            if let Some(idx) = completer_idx {
                let trigger_and_query = app.completers.get(idx).and_then(|completer| {
                    completer
                        .find_trigger_position(&input_text)
                        .map(|pos| (pos, input_text[pos + 1..].to_string()))
                });

                if let Some((_, query)) = trigger_and_query {
                    app.update_completion_query(query.clone());
                    if let Some(completer) = app.completers.get(idx) {
                        if let Ok(candidates) = completer.get_completions(&query).await {
                            app.set_completion_candidates(candidates);
                        }
                    }
                } else {
                    app.cancel_completion();
                }
            } else {
                app.cancel_completion();
            }
            KeyHandlerResult::Handled
        }
        KeyCode::Char(c) => {
            app.input.input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
            let input_text = app.get_input_text();
            let completer_idx = app.completion_state.as_ref().map(|s| s.completer_index);

            if let Some(idx) = completer_idx {
                let trigger_and_query = app.completers.get(idx).and_then(|completer| {
                    completer
                        .find_trigger_position(&input_text)
                        .map(|pos| (pos, input_text[pos + 1..].to_string()))
                });

                if let Some((_, query)) = trigger_and_query {
                    app.update_completion_query(query.clone());
                    if let Some(completer) = app.completers.get(idx) {
                        if let Ok(candidates) = completer.get_completions(&query).await {
                            app.set_completion_candidates(candidates);
                        }
                    }
                }
            }
            KeyHandlerResult::Handled
        }
        _ => {
            app.cancel_completion();
            KeyHandlerResult::Handled
        }
    }
}

pub async fn handle_normal_keys(
    key: KeyCode,
    modifiers: KeyModifiers,
    app: &mut AppState,
    agent_task_active: bool,
) -> KeyHandlerResult {
    match key {
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
            KeyHandlerResult::ShouldQuit
        }
        KeyCode::Enter => {
            let input_text = app.get_input_text();
            if !input_text.trim().is_empty() && !agent_task_active {
                app.add_message(format!("> {}", input_text));
                app.add_message("\n".to_string());
                app.clear_input();

                // Check if this is a command
                if input_text.trim().starts_with('/') {
                    KeyHandlerResult::StartCommand(input_text)
                } else {
                    KeyHandlerResult::StartConversation(input_text)
                }
            } else {
                KeyHandlerResult::Handled
            }
        }
        KeyCode::Char(c) => {
            // Check if this char triggers any completer
            if let Some(completer_idx) = app.find_completer_for_key(c) {
                app.input.input(KeyEvent::new(KeyCode::Char(c), modifiers));
                let cursor_pos = app.input.cursor();
                app.start_completion(cursor_pos.0, completer_idx);

                if let Some(completer) = app.completers.get(completer_idx) {
                    if let Ok(candidates) = completer.get_completions("").await {
                        app.set_completion_candidates(candidates);
                    }
                }
            } else {
                app.input.input(KeyEvent::new(KeyCode::Char(c), modifiers));
            }
            KeyHandlerResult::Handled
        }
        _ => {
            app.input.input(KeyEvent::new(key, modifiers));
            KeyHandlerResult::Handled
        }
    }
}
