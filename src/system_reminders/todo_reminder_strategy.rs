use crate::Conversation;
use crate::system_reminders::{ReminderContext, ReminderStrategy, SideEffectResult};
use crate::tools::TodoState;
use anyhow::Result;

pub struct TodoReminderStrategy {
    todo_state: TodoState,
}

impl TodoReminderStrategy {
    pub fn new(todo_state: TodoState) -> Self {
        Self { todo_state }
    }
}

#[async_trait::async_trait]
impl ReminderStrategy for TodoReminderStrategy {
    async fn apply(
        &self,
        conversation: &mut Conversation,
        _context: &ReminderContext,
    ) -> Result<SideEffectResult> {
        if let Some(reminder) = self.todo_state.format_for_llm().await
            && let Some(last_msg) = conversation.messages.last_mut()
            && last_msg.role == "user"
            && let Some(content) = &mut last_msg.content
        {
            content.push_str("\n\n");
            content.push_str(&reminder);
        }

        Ok(SideEffectResult::Continue)
    }

    fn name(&self) -> &'static str {
        "todo_reminder"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::todo_write::{TodoItem, TodoStatus};

    fn create_context() -> ReminderContext {
        ReminderContext { agent_step: 0 }
    }

    #[test]
    fn test_strategy_name() {
        let state = TodoState::new();
        let strategy = TodoReminderStrategy::new(state);
        assert_eq!(strategy.name(), "todo_reminder");
    }

    #[tokio::test]
    async fn test_injects_reminder_into_user_message() {
        let state = TodoState::new();
        let todos = vec![TodoItem::new(
            "Test task".to_string(),
            TodoStatus::Pending,
            "Testing task".to_string(),
        )];
        state.update(todos).await;

        let strategy = TodoReminderStrategy::new(state);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        let last_msg = conversation.messages.last().unwrap();
        assert!(last_msg.content.as_ref().unwrap().contains("Test task"));
        assert!(
            last_msg
                .content
                .as_ref()
                .unwrap()
                .contains("system-reminder")
        );
    }

    #[tokio::test]
    async fn test_empty_todo_still_injects_reminder() {
        let state = TodoState::new();
        let strategy = TodoReminderStrategy::new(state);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        let last_msg = conversation.messages.last().unwrap();
        assert!(
            last_msg
                .content
                .as_ref()
                .unwrap()
                .contains("todo list is currently empty")
        );
    }

    #[tokio::test]
    async fn test_no_injection_without_user_message() {
        let state = TodoState::new();
        let strategy = TodoReminderStrategy::new(state);
        let mut conversation = Conversation::new();

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        assert!(conversation.messages.is_empty());
    }

    #[tokio::test]
    async fn test_always_returns_continue() {
        let state = TodoState::new();
        let strategy = TodoReminderStrategy::new(state);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SideEffectResult::Continue));
    }
}
