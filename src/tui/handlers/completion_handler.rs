use crate::tui::app::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

pub struct CompletionHandler;

impl CompletionHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CompletionHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputHandler for CompletionHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        _agent_task_active: bool,
    ) -> KeyHandlerResult {
        if !app.is_completing() {
            return KeyHandlerResult::NotHandled;
        }

        let Event::Key(key_event) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let key = key_event.code;
        // should_handle already checked if completion is active
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

                if let Some(selected) = app.apply_completion()
                    && let Some(idx) = completer_idx
                    && let Some(completer) = app.completers.get(idx)
                    && let Some(trigger_pos) = completer.find_trigger_position(&input_text)
                {
                    let new_text = completer.apply_completion(&input_text, trigger_pos, &selected);

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
                KeyHandlerResult::Handled
            }
            KeyCode::Esc => {
                app.cancel_completion();
                KeyHandlerResult::Handled
            }
            KeyCode::Backspace => {
                app.input
                    .input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
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
                        if let Some(completer) = app.completers.get(idx)
                            && let Ok(candidates) = completer.get_completions(&query).await
                        {
                            app.set_completion_candidates(candidates);
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
                app.input
                    .input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
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
                        if let Some(completer) = app.completers.get(idx)
                            && let Ok(candidates) = completer.get_completions(&query).await
                        {
                            app.set_completion_candidates(candidates);
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
}
