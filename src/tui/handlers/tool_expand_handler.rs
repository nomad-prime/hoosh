use crate::tui::app_state::AppState;
use crate::tui::handler_result::KeyHandlerResult;
use crate::tui::input_handler::InputHandler;
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

        if is_ctrl_o && app.tools.active.len() >= 2 {
            app.tools.expanded = !app.tools.expanded;
            return KeyHandlerResult::Handled;
        }

        KeyHandlerResult::NotHandled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolCategory, ToolRender};
    use crate::tui::app_state::{ActiveToolCall, ToolCallStatus};
    use crossterm::event::KeyEvent;
    use std::time::Instant;

    fn ctrl_o() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))
    }

    fn active_call() -> ActiveToolCall {
        ActiveToolCall {
            tool_call_id: "id".into(),
            display_name: "Read(a.rs)".into(),
            render: ToolRender::Standard,
            category: ToolCategory::Read,
            status: ToolCallStatus::Executing,
            preview: None,
            result_summary: None,
            subagent_steps: Vec::new(),
            is_subagent_task: false,
            bash_output_lines: Vec::new(),
            is_bash_streaming: false,
            start_time: Instant::now(),
            budget_pct: None,
            total_tool_uses: None,
            total_tokens: None,
        }
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
    async fn ctrl_o_ignored_with_single_call() {
        let mut handler = ToolExpandHandler::new();
        let mut app = AppState::new();
        app.tools.active = vec![active_call()];

        let result = handler.handle_event(&ctrl_o(), &mut app, true).await;
        assert!(matches!(result, KeyHandlerResult::NotHandled));
        assert!(!app.tools.expanded);
    }
}
