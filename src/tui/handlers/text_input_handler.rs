use crate::tui::app_state::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

pub struct TextInputHandler;

impl TextInputHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TextInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputHandler for TextInputHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        _agent_task_active: bool,
    ) -> KeyHandlerResult {
        let Event::Key(key_event) = event else {
            return KeyHandlerResult::NotHandled;
        };

        match key_event.code {
            KeyCode::BackTab => {
                // Shift+Tab toggles autopilot mode
                app.toggle_autopilot();
            }
            KeyCode::Char('v') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Ok(text) = app.clipboard.get_text() {
                    let lines: Vec<&str> = text.lines().collect();
                    for (i, line) in lines.iter().enumerate() {
                        app.input.insert_str(line);
                        if i < lines.len() - 1 {
                            app.input.insert_newline();
                        }
                    }
                }
            }
            KeyCode::Up => {
                // Navigate to previous prompt in history
                let current_input = app.get_input_text();
                if let Some(prev_prompt) = app.prompt_history.prev(&current_input) {
                    app.clear_input();
                    let lines: Vec<&str> = prev_prompt.lines().collect();
                    for (i, line) in lines.iter().enumerate() {
                        app.input.insert_str(line);
                        if i < lines.len() - 1 {
                            app.input.insert_newline();
                        }
                    }
                }
            }
            KeyCode::Down => {
                // Navigate to next prompt in history
                if let Some(next_prompt) = app.prompt_history.next_entry() {
                    app.clear_input();
                    let lines: Vec<&str> = next_prompt.lines().collect();
                    for (i, line) in lines.iter().enumerate() {
                        app.input.insert_str(line);
                        if i < lines.len() - 1 {
                            app.input.insert_newline();
                        }
                    }
                }
            }
            KeyCode::Char(c) => {
                // Reset history navigation when user starts typing
                if app.prompt_history.is_navigating() {
                    app.prompt_history.reset();
                }

                // Check if this char triggers any completer
                if let Some(completer_idx) = app.find_completer_for_key(c) {
                    app.input
                        .input(KeyEvent::new(KeyCode::Char(c), key_event.modifiers));
                    let cursor_pos = app.input.cursor();
                    app.start_completion(cursor_pos.0, completer_idx);

                    if let Some(completer) = app.completers.get(completer_idx)
                        && let Ok(candidates) = completer.get_completions("").await
                    {
                        app.set_completion_candidates(candidates);
                    }
                } else {
                    app.input
                        .input(KeyEvent::new(KeyCode::Char(c), key_event.modifiers));
                }
            }
            _ => {
                // Reset history navigation on other key presses
                if app.prompt_history.is_navigating() {
                    app.prompt_history.reset();
                }
                app.input
                    .input(KeyEvent::new(key_event.code, key_event.modifiers));
            }
        }

        KeyHandlerResult::Handled
    }
}
