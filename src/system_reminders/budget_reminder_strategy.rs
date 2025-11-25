use std::sync::Arc;

use crate::Conversation;
use crate::system_reminders::{ReminderContext, ReminderStrategy, SideEffectResult};
use crate::task_management::ExecutionBudget;
use anyhow::Result;

pub struct BudgetReminderStrategy {
    budget: Arc<ExecutionBudget>,
    max_steps: usize,
}

impl BudgetReminderStrategy {
    pub fn new(budget: Arc<ExecutionBudget>, max_steps: usize) -> Self {
        Self { budget, max_steps }
    }
}

#[async_trait::async_trait]
impl ReminderStrategy for BudgetReminderStrategy {
    async fn apply(
        &self,
        conversation: &mut Conversation,
        context: &ReminderContext,
    ) -> Result<SideEffectResult> {
        let remaining = self.budget.remaining_seconds();
        let step = context.agent_step;

        if remaining == 0 {
            return Ok(SideEffectResult::ExitTurn {
                inject_user_message: Some(
                    "Time budget has been exhausted. Please provide a brief summary of what you've accomplished so far.".to_string()
                ),
                error_message: Some("Time budget exhausted".to_string()),
            });
        }

        if self.budget.should_wrap_up(step) {
            let wrap_up_message = format!(
                "BUDGET ALERT: You have approximately {} seconds and {} steps remaining. \
                Please prioritize wrapping up your work and providing a final answer.",
                remaining,
                self.max_steps.saturating_sub(step)
            );
            conversation.add_system_message(wrap_up_message);
        }

        Ok(SideEffectResult::Continue)
    }

    fn name(&self) -> &'static str {
        "budget_reminder"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_context(step: usize) -> ReminderContext {
        ReminderContext { agent_step: step }
    }

    #[test]
    fn test_strategy_name() {
        let budget = Arc::new(ExecutionBudget::new(Duration::from_secs(60), 10));
        let strategy = BudgetReminderStrategy::new(budget, 10);
        assert_eq!(strategy.name(), "budget_reminder");
    }

    #[tokio::test]
    async fn test_no_action_with_plenty_of_budget() {
        let budget = Arc::new(ExecutionBudget::new(Duration::from_secs(600), 100));
        let strategy = BudgetReminderStrategy::new(budget, 100);

        let mut conversation = Conversation::new();
        let context = create_context(5);
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SideEffectResult::Continue));
        assert!(conversation.messages.is_empty());
    }

    #[tokio::test]
    async fn test_wrap_up_warning_at_high_step_usage() {
        let budget = Arc::new(ExecutionBudget::new(Duration::from_secs(600), 10));
        let strategy = BudgetReminderStrategy::new(budget, 10);

        let mut conversation = Conversation::new();
        let context = create_context(8);
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SideEffectResult::Continue));
        assert_eq!(conversation.messages.len(), 1);
        assert!(
            conversation.messages[0]
                .content
                .as_ref()
                .unwrap()
                .contains("BUDGET ALERT")
        );
    }

    #[tokio::test]
    async fn test_exit_turn_when_time_exhausted() {
        let budget = Arc::new(ExecutionBudget::new(Duration::from_millis(1), 100));
        std::thread::sleep(Duration::from_millis(10));

        let strategy = BudgetReminderStrategy::new(budget, 100);
        let mut conversation = Conversation::new();
        let context = create_context(0);
        let result = strategy.apply(&mut conversation, &context).await;

        assert!(result.is_ok());
        match result.unwrap() {
            SideEffectResult::ExitTurn {
                inject_user_message,
                error_message,
            } => {
                assert!(inject_user_message.is_some());
                assert!(inject_user_message.unwrap().contains("exhausted"));
                assert!(error_message.is_some());
            }
            _ => panic!("Expected ExitTurn"),
        }
    }
}
