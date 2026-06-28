use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use crate::tui::state::AppState;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyModifiers};

/// Cancel and quit semantics.
///
/// Rules:
/// - Esc or Ctrl+C while the agent is running: cancel the turn. The handler
///   that processes `ShouldCancelTask` restores the submitted prompt back into
///   the input buffer. The restored prompt then behaves like normal typed
///   input — Ctrl+C clears it (arming quit), and the next Ctrl+C exits.
/// - Ctrl+C while idle:
///   - If quit is armed (already cleared input once), exit.
///   - Else if input has text, clear input and arm quit.
///   - Else exit immediately.
/// - Esc while idle: not handled here — leave it to dialog/completion handlers
///   that close themselves on Esc.
pub struct QuitHandler;

impl QuitHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for QuitHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputHandler for QuitHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        agent_task_active: bool,
    ) -> KeyHandlerResult {
        let Event::Key(key) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let is_ctrl_c =
            key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
        let is_esc = key.code == KeyCode::Esc;

        if !is_ctrl_c && !is_esc {
            return KeyHandlerResult::NotHandled;
        }

        if agent_task_active {
            app.should_cancel_task = true;
            return KeyHandlerResult::ShouldCancelTask;
        }

        // Idle path. Esc is for dialogs/completion — let it through.
        if is_esc {
            return KeyHandlerResult::NotHandled;
        }

        // Ctrl+C idle.
        if app.quit_armed {
            app.should_quit = true;
            return KeyHandlerResult::ShouldQuit;
        }

        let input_text = app.get_input_text();
        if !input_text.is_empty() {
            app.clear_input();
            app.quit_armed = true;
            KeyHandlerResult::Handled
        } else {
            app.should_quit = true;
            KeyHandlerResult::ShouldQuit
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::AppState;
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};

    fn key(code: KeyCode, mods: KeyModifiers) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: mods,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn ctrl_c() -> Event {
        key(KeyCode::Char('c'), KeyModifiers::CONTROL)
    }

    fn esc() -> Event {
        key(KeyCode::Esc, KeyModifiers::NONE)
    }

    #[tokio::test]
    async fn ctrl_c_while_agent_active_cancels_and_arms_quit_via_loop() {
        let mut app = AppState::new();
        let mut h = QuitHandler::new();
        let result = h.handle_event(&ctrl_c(), &mut app, true).await;
        assert!(matches!(result, KeyHandlerResult::ShouldCancelTask));
        assert!(app.should_cancel_task);
    }

    #[tokio::test]
    async fn esc_while_agent_active_cancels() {
        let mut app = AppState::new();
        let mut h = QuitHandler::new();
        let result = h.handle_event(&esc(), &mut app, true).await;
        assert!(matches!(result, KeyHandlerResult::ShouldCancelTask));
    }

    #[tokio::test]
    async fn esc_while_idle_is_passed_through_to_other_handlers() {
        let mut app = AppState::new();
        let mut h = QuitHandler::new();
        let result = h.handle_event(&esc(), &mut app, false).await;
        assert!(matches!(result, KeyHandlerResult::NotHandled));
    }

    #[tokio::test]
    async fn ctrl_c_idle_with_text_clears_input_and_arms_quit() {
        let mut app = AppState::new();
        app.set_input_text("some half-typed thought");
        let mut h = QuitHandler::new();

        let result = h.handle_event(&ctrl_c(), &mut app, false).await;
        assert!(matches!(result, KeyHandlerResult::Handled));
        assert_eq!(app.get_input_text(), "");
        assert!(app.quit_armed);
    }

    #[tokio::test]
    async fn second_ctrl_c_after_arming_exits_even_with_text() {
        let mut app = AppState::new();
        app.quit_armed = true;
        app.set_input_text("restored prompt");
        let mut h = QuitHandler::new();

        let result = h.handle_event(&ctrl_c(), &mut app, false).await;
        assert!(matches!(result, KeyHandlerResult::ShouldQuit));
        assert!(app.should_quit);
    }

    #[tokio::test]
    async fn ctrl_c_idle_empty_input_quits_immediately() {
        let mut app = AppState::new();
        let mut h = QuitHandler::new();
        let result = h.handle_event(&ctrl_c(), &mut app, false).await;
        assert!(matches!(result, KeyHandlerResult::ShouldQuit));
    }

    #[tokio::test]
    async fn non_cancel_key_is_not_handled() {
        let mut app = AppState::new();
        let mut h = QuitHandler::new();
        let result = h
            .handle_event(&key(KeyCode::Char('x'), KeyModifiers::NONE), &mut app, true)
            .await;
        assert!(matches!(result, KeyHandlerResult::NotHandled));
    }
}
