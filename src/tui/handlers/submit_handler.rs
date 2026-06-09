use crate::tui::app_state::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode};

pub struct SubmitHandler;

impl SubmitHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SubmitHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputHandler for SubmitHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        agent_task_active: bool,
    ) -> KeyHandlerResult {
        let Event::Key(key) = event else {
            return KeyHandlerResult::NotHandled;
        };

        if key.code != KeyCode::Enter {
            return KeyHandlerResult::NotHandled;
        }

        let input_text = app.get_input_text();
        if input_text.trim().is_empty() {
            return KeyHandlerResult::Handled;
        }

        let expanded_input = app.expand_attachments(&input_text);

        if agent_task_active {
            // Queue the prompt for delivery after the current turn finishes.
            // The QueuedPromptsComponent above the input bar surfaces the
            // queue visually — no need to dump status lines into the
            // conversation buffer.
            app.prompt_history.add(expanded_input.clone());
            app.clear_input();
            app.clear_attachments();
            app.queued_prompts.push_back(expanded_input);
            return KeyHandlerResult::Handled;
        }

        app.add_user_input(&expanded_input);
        app.prompt_history.add(expanded_input.clone());

        // Drain any clipboard-pasted image attachments before clear_attachments
        // wipes them. These travel with the prompt to the agent.
        let image_attachments = app.drain_image_attachments();

        app.clear_input();
        app.clear_attachments();
        app.quit_armed = false;

        if expanded_input.trim().starts_with('/') {
            // Slash commands are synchronous; no agent turn to restore.
            app.last_submitted_input = None;
            KeyHandlerResult::StartCommand(expanded_input)
        } else {
            app.last_submitted_input = Some(expanded_input.clone());
            KeyHandlerResult::StartConversation {
                input: expanded_input,
                image_attachments,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn enter_event() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    #[tokio::test]
    async fn submit_while_idle_starts_conversation() {
        let mut app = AppState::new();
        app.set_input_text("hello world");
        let mut h = SubmitHandler::new();
        let r = h.handle_event(&enter_event(), &mut app, false).await;
        assert!(matches!(
            r,
            KeyHandlerResult::StartConversation { ref input, .. } if input == "hello world"
        ));
        assert_eq!(app.get_input_text(), "");
        assert!(app.queued_prompts.is_empty());
        assert_eq!(app.last_submitted_input.as_deref(), Some("hello world"));
    }

    #[tokio::test]
    async fn submit_while_busy_queues_and_clears_input() {
        let mut app = AppState::new();
        app.set_input_text("queued prompt");
        let mut h = SubmitHandler::new();
        let r = h.handle_event(&enter_event(), &mut app, true).await;
        // Queued, not started.
        assert!(matches!(r, KeyHandlerResult::Handled));
        assert_eq!(app.get_input_text(), "");
        assert_eq!(app.queued_prompts.len(), 1);
        assert_eq!(
            app.queued_prompts.front().map(String::as_str),
            Some("queued prompt")
        );
        // last_submitted_input not touched — that belongs to the in-flight turn.
        assert!(app.last_submitted_input.is_none());
    }

    #[tokio::test]
    async fn submit_while_busy_with_empty_input_is_noop() {
        let mut app = AppState::new();
        let mut h = SubmitHandler::new();
        let r = h.handle_event(&enter_event(), &mut app, true).await;
        assert!(matches!(r, KeyHandlerResult::Handled));
        assert!(app.queued_prompts.is_empty());
    }

    #[tokio::test]
    async fn multiple_submits_while_busy_queue_in_order() {
        let mut app = AppState::new();
        let mut h = SubmitHandler::new();
        for prompt in ["one", "two", "three"] {
            app.set_input_text(prompt);
            h.handle_event(&enter_event(), &mut app, true).await;
        }
        assert_eq!(app.queued_prompts.len(), 3);
        let collected: Vec<String> = app.queued_prompts.iter().cloned().collect();
        assert_eq!(collected, vec!["one", "two", "three"]);
    }
}
