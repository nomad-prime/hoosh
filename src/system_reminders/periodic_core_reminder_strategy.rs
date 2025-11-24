use crate::system_reminders::{ReminderContext, ReminderStrategy, SideEffectResult};
use crate::{Agent, Conversation};
use anyhow::Result;

pub struct PeriodicCoreReminderStrategy {
    interval: usize,
    core_instructions: String,
}

impl PeriodicCoreReminderStrategy {
    pub fn new(interval: usize, core_instructions: String) -> Self {
        Self {
            interval,
            core_instructions,
        }
    }
}

#[async_trait::async_trait]
impl ReminderStrategy for PeriodicCoreReminderStrategy {
    async fn apply(
        &self,
        ctx: &ReminderContext,
        conversation: &mut Conversation,
        _agent: &Agent,
    ) -> Result<SideEffectResult> {
        if ctx.step > 0 && ctx.step.is_multiple_of(self.interval) {
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

    fn create_test_context(step: usize, max_steps: usize) -> ReminderContext {
        ReminderContext {
            step,
            max_steps,
            total_tokens: 0,
        }
    }

    #[test]
    fn test_strategy_name() {
        let strategy = PeriodicCoreReminderStrategy::new(5, "Test instructions".to_string());
        assert_eq!(strategy.name(), "periodic_core");
    }

    #[test]
    fn test_constructor() {
        let instructions = "Test".to_string();
        let strategy = PeriodicCoreReminderStrategy::new(10, instructions.clone());
        assert_eq!(strategy.name(), "periodic_core");
    }

    #[tokio::test]
    async fn test_no_reminder_at_step_zero() {
        let strategy = PeriodicCoreReminderStrategy::new(5, "Test message".to_string());
        let mut conversation = Conversation::new();
        let ctx = create_test_context(0, 100);
        // Mock agent - we only need it to be passed, not used
        let mock_agent = crate::Agent::new(
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
        );

        let result = strategy.apply(&ctx, &mut conversation, &mock_agent).await;

        assert!(result.is_ok());
        assert_eq!(conversation.get_messages_for_api().len(), 0);
    }

    #[tokio::test]
    async fn test_reminder_at_first_interval() {
        let instructions = "First reminder at step 5".to_string();
        let strategy = PeriodicCoreReminderStrategy::new(5, instructions.clone());
        let mut conversation = Conversation::new();
        let ctx = create_test_context(5, 100);
        let mock_agent = crate::Agent::new(
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
        );

        let result = strategy.apply(&ctx, &mut conversation, &mock_agent).await;

        assert!(result.is_ok());
        let messages = conversation.get_messages_for_api();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, Some(instructions));
    }

    #[tokio::test]
    async fn test_always_returns_continue() {
        let strategy = PeriodicCoreReminderStrategy::new(5, "Test".to_string());
        let mut conversation = Conversation::new();
        let mock_agent = crate::Agent::new(
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
        );

        let ctx = create_test_context(5, 100);
        let result = strategy.apply(&ctx, &mut conversation, &mock_agent).await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SideEffectResult::Continue));
    }

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
}
