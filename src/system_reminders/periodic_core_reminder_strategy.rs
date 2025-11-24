use crate::Conversation;
use crate::system_reminders::{ReminderContext, ReminderStrategy, SideEffectResult};
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Periodic strategy that re-injects core instructions when conversation grows beyond a token threshold.
/// This helps maintain focus on the agent's core mission across long conversations.
///
/// The strategy tracks the token count at the last reminder injection and re-injects when
/// the conversation has grown by approximately `token_interval` tokens.
pub struct PeriodicCoreReminderStrategy {
    token_interval: usize,
    core_instructions: String,
    last_reminder_token_count: Arc<AtomicUsize>,
}

impl PeriodicCoreReminderStrategy {
    pub fn new(token_interval: usize, core_instructions: String) -> Self {
        Self {
            token_interval,
            core_instructions,
            last_reminder_token_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl ReminderStrategy for PeriodicCoreReminderStrategy {
    async fn apply(
        &self,
        conversation: &mut Conversation,
        _context: &ReminderContext,
    ) -> Result<SideEffectResult> {
        let current_tokens = conversation.estimate_token();
        let last_tokens = self.last_reminder_token_count.load(Ordering::SeqCst);

        if current_tokens.saturating_sub(last_tokens) > self.token_interval {
            conversation.add_system_message(self.core_instructions.clone());
            self.last_reminder_token_count
                .store(current_tokens, Ordering::SeqCst);
        }

        Ok(SideEffectResult::Continue)
    }

    fn name(&self) -> &'static str {
        "periodic_core"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_context() -> ReminderContext {
        ReminderContext { agent_step: 0 }
    }

    #[test]
    fn test_strategy_name() {
        let strategy = PeriodicCoreReminderStrategy::new(5000, "Test instructions".to_string());
        assert_eq!(strategy.name(), "periodic_core");
    }

    #[test]
    fn test_constructor() {
        let instructions = "Test".to_string();
        let strategy = PeriodicCoreReminderStrategy::new(10000, instructions.clone());
        assert_eq!(strategy.name(), "periodic_core");
    }

    #[tokio::test]
    async fn test_no_reminder_below_interval() {
        let strategy = PeriodicCoreReminderStrategy::new(10000, "Test message".to_string());
        let mut conversation = Conversation::new();
        conversation.add_user_message("x".repeat(100));
        let context = create_context();

        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        // Only initial message, no reminder yet since growth < interval
        assert_eq!(conversation.get_messages_for_api().len(), 1);
    }

    #[tokio::test]
    async fn test_reminder_after_token_growth() {
        let instructions = "Critical reminder".to_string();
        let strategy = PeriodicCoreReminderStrategy::new(100, instructions.clone());
        let mut conversation = Conversation::new();
        conversation.add_user_message("x".repeat(500));

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        let messages = conversation.get_messages_for_api();
        // Should have user message + reminder
        assert!(messages.len() >= 2);
        assert_eq!(messages.last().unwrap().role, "system");
        assert_eq!(messages.last().unwrap().content, Some(instructions.clone()));
    }

    #[tokio::test]
    async fn test_no_duplicate_reminders() {
        let instructions = "Critical reminder".to_string();
        let strategy = PeriodicCoreReminderStrategy::new(100, instructions.clone());
        let mut conversation = Conversation::new();
        conversation.add_user_message("x".repeat(500));

        let context = create_context();

        // First apply - should add reminder
        let _ = strategy.apply(&mut conversation, &context).await;
        let count_after_first = conversation.get_messages_for_api().len();

        // Second apply - should NOT add another reminder (growth since last is 0)
        let _ = strategy.apply(&mut conversation, &context).await;
        let count_after_second = conversation.get_messages_for_api().len();

        assert_eq!(count_after_first, count_after_second);
    }

    #[tokio::test]
    async fn test_reminder_after_more_growth() {
        let instructions = "Critical reminder".to_string();
        let strategy = PeriodicCoreReminderStrategy::new(100, instructions.clone());
        let mut conversation = Conversation::new();
        conversation.add_user_message("x".repeat(500));

        let context = create_context();

        // First apply - should add reminder
        let _ = strategy.apply(&mut conversation, &context).await;
        let count_after_first = conversation.get_messages_for_api().len();

        // Add more messages to trigger another reminder
        conversation.add_user_message("y".repeat(500));

        // Second apply - should add another reminder (growth > interval)
        let _ = strategy.apply(&mut conversation, &context).await;
        let count_after_second = conversation.get_messages_for_api().len();

        assert!(count_after_second > count_after_first);
        assert_eq!(
            conversation.get_messages_for_api().last().unwrap().role,
            "system"
        );
    }

    #[tokio::test]
    async fn test_always_returns_continue() {
        let strategy = PeriodicCoreReminderStrategy::new(5000, "Test".to_string());
        let mut conversation = Conversation::new();
        let context = create_context();

        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SideEffectResult::Continue));
    }

    #[test]
    fn test_token_estimation() {
        let mut conversation = Conversation::new();
        assert_eq!(conversation.estimate_token(), 0);

        conversation.add_user_message("hello".to_string());
        let tokens = conversation.estimate_token();
        assert!(tokens > 0, "Expected positive token count, got {}", tokens);
    }
}
