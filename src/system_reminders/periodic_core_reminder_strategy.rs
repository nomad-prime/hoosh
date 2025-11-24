use crate::system_reminders::{ReminderStrategy, SideEffectResult};
use crate::{Agent, Conversation};
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
        _agent: &Agent,
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
    use crate::backends::{LlmBackend, LlmError, LlmResponse};
    use async_trait::async_trait;

    struct MockBackend;

    #[async_trait]
    impl LlmBackend for MockBackend {
        async fn send_message(&self, _message: &str) -> Result<String> {
            Ok("mock response".to_string())
        }

        async fn send_message_with_tools(
            &self,
            _conversation: &Conversation,
            _tools: &crate::tools::ToolRegistry,
        ) -> Result<LlmResponse, LlmError> {
            Ok(LlmResponse::content_only("mock".to_string()))
        }

        async fn send_message_with_tools_and_events(
            &self,
            _conversation: &Conversation,
            _tools: &crate::tools::ToolRegistry,
            _event_sender: Option<tokio::sync::mpsc::UnboundedSender<crate::AgentEvent>>,
        ) -> Result<LlmResponse, LlmError> {
            Ok(LlmResponse::content_only("mock".to_string()))
        }

        fn backend_name(&self) -> &str {
            "mock"
        }

        fn model_name(&self) -> &str {
            "mock-model"
        }

        fn pricing(&self) -> Option<crate::backends::TokenPricing> {
            None
        }
    }

    fn create_mock_agent() -> Agent {
        Agent::new(
            std::sync::Arc::new(MockBackend),
            std::sync::Arc::new(crate::tools::ToolRegistry::new()),
            std::sync::Arc::new(crate::tool_executor::ToolExecutor::new(
                std::sync::Arc::new(crate::tools::ToolRegistry::new()),
                std::sync::Arc::new(crate::permissions::PermissionManager::new(
                    {
                        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
                        tx
                    },
                    {
                        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
                        rx
                    },
                )),
            )),
        )
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
        let mock_agent = create_mock_agent();

        let result = strategy.apply(&mut conversation, &mock_agent).await;

        assert!(result.is_ok());
        assert_eq!(conversation.get_messages_for_api().len(), 0);
    }

    #[tokio::test]
    async fn test_reminder_above_threshold() {
        let instructions = "Critical reminder".to_string();
        let strategy = PeriodicCoreReminderStrategy::new(100, instructions.clone());
        let mut conversation = Conversation::new();

        // Add content to exceed threshold: 500 chars ≈ 125 tokens
        // (4 bytes for "user" + 500 = 504 bytes / 4 = 126 tokens)
        conversation.add_user_message("x".repeat(500));

        let mock_agent = create_mock_agent();
        let result = strategy.apply(&mut conversation, &mock_agent).await;

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
        let mock_agent = create_mock_agent();

        let result = strategy.apply(&mut conversation, &mock_agent).await;

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
