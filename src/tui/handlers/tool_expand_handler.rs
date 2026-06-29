use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
use crate::tui::state::AppState;
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyModifiers};

pub struct ToolExpandHandler;

impl ToolExpandHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ToolExpandHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InputHandler for ToolExpandHandler {
    async fn handle_event(
        &mut self,
        event: &Event,
        app: &mut AppState,
        _agent_task_active: bool,
    ) -> KeyHandlerResult {
        let Event::Key(key) = event else {
            return KeyHandlerResult::NotHandled;
        };

        let is_ctrl_o =
            key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL);

        // ctrl+o toggles either a collapsed 2+ batch or the live bash output of
        // a single streaming call.
        let has_expandable_bash = app
            .tools
            .active
            .iter()
            .any(|tc| tc.bash.as_ref().is_some_and(|b| !b.lines.is_empty()));

        if is_ctrl_o && (app.tools.active.len() >= 2 || has_expandable_bash) {
            app.tools.expanded = !app.tools.expanded;
            return KeyHandlerResult::Handled;
        }

        KeyHandlerResult::NotHandled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolRender, phrasing};
    use crate::tui::state::{ActiveToolCall, ToolCallStatus};
    use crossterm::event::KeyEvent;

    fn ctrl_o() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))
    }

    fn active_call() -> ActiveToolCall {
        let mut call = ActiveToolCall::new(
            "id".into(),
            "Read(a.rs)".into(),
            ToolRender::Standard,
            phrasing::READ,
        );
        call.status = ToolCallStatus::Executing;
        call
    }

    #[tokio::test]
    async fn ctrl_o_toggles_expansion_with_a_batch() {
        let mut handler = ToolExpandHandler::new();
        let mut app = AppState::new();
        app.tools.active = vec![active_call(), active_call()];

        let result = handler.handle_event(&ctrl_o(), &mut app, true).await;
        assert!(matches!(result, KeyHandlerResult::Handled));
        assert!(app.tools.expanded);

        handler.handle_event(&ctrl_o(), &mut app, true).await;
        assert!(!app.tools.expanded);
    }

    #[tokio::test]
    async fn ctrl_o_ignored_with_single_non_bash_call() {
        let mut handler = ToolExpandHandler::new();
        let mut app = AppState::new();
        app.tools.active = vec![active_call()];

        let result = handler.handle_event(&ctrl_o(), &mut app, true).await;
        assert!(matches!(result, KeyHandlerResult::NotHandled));
        assert!(!app.tools.expanded);
    }

    #[tokio::test]
    async fn ctrl_o_toggles_bash_output_with_single_call() {
        use crate::tui::state::{BashDetail, BashOutputLine};

        let mut handler = ToolExpandHandler::new();
        let mut app = AppState::new();
        let mut bash = active_call();
        bash.bash = Some(BashDetail {
            lines: vec![BashOutputLine {
                line_number: 1,
                content: "building...".into(),
                stream_type: "stdout".into(),
            }],
        });
        app.tools.active = vec![bash];

        let result = handler.handle_event(&ctrl_o(), &mut app, true).await;
        assert!(matches!(result, KeyHandlerResult::Handled));
        assert!(app.tools.expanded);

        handler.handle_event(&ctrl_o(), &mut app, true).await;
        assert!(!app.tools.expanded);
    }

    #[tokio::test]
    async fn ctrl_o_ignored_for_single_bash_call_without_output() {
        let mut handler = ToolExpandHandler::new();
        let mut app = AppState::new();
        let mut bash = active_call();
        bash.bash = Some(crate::tui::state::BashDetail::default());
        app.tools.active = vec![bash];

        let result = handler.handle_event(&ctrl_o(), &mut app, true).await;
        assert!(matches!(result, KeyHandlerResult::NotHandled));
        assert!(!app.tools.expanded);
    }
}
