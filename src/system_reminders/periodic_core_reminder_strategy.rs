use crate::Conversation;
use crate::system_reminders::{ReminderContext, ReminderStrategy, SideEffectResult};
use anyhow::Result;

/// Periodic strategy that re-injects core instructions when conversation grows beyond a token threshold.
/// This helps maintain focus on the agent's core mission across long conversations.
pub struct PeriodicCoreReminderStrategy {
    token_threshold: usize,
    core_instructions: String,
}

impl PeriodicCoreReminderStrategy {
    pub fn new(token_threshold: usize, core_instructions: String) -> Self {
        Self {
            token_threshold,
            core_instructions,
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
        if conversation.estimate_token() > self.token_threshold {
            conversation.add_system_message(self.core_instructions.clone());
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
    async fn test_no_reminder_below_threshold() {
        let strategy = PeriodicCoreReminderStrategy::new(10000, "Test message".to_string());
        let mut conversation = Conversation::new();
        let context = create_context();

        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        assert_eq!(conversation.get_messages_for_api().len(), 0);
    }

    #[tokio::test]
    async fn test_reminder_above_threshold() {
        let instructions = "Critical reminder".to_string();
        let strategy = PeriodicCoreReminderStrategy::new(100, instructions.clone());
        let mut conversation = Conversation::new();
        conversation.add_user_message("x".repeat(500));

        let context = create_context();
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        let messages = conversation.get_messages_for_api();
        assert!(messages.len() >= 2);
        assert_eq!(messages.last().unwrap().role, "system");
        assert_eq!(messages.last().unwrap().content, Some(instructions));
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
