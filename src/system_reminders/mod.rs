pub mod budget_reminder_strategy;
pub mod periodic_core_reminder_strategy;
pub mod todo_reminder_strategy;

pub use budget_reminder_strategy::BudgetReminderStrategy;
pub use periodic_core_reminder_strategy::PeriodicCoreReminderStrategy;
pub use todo_reminder_strategy::TodoReminderStrategy;

use anyhow::Result;

use crate::agent::Conversation;

#[derive(Debug)]
pub enum SideEffectResult {
    Continue,
    ExitTurn {
        inject_user_message: Option<String>,
        error_message: Option<String>,
    },
}

pub struct ReminderContext {
    pub agent_step: usize,
}

#[async_trait::async_trait]
pub trait ReminderStrategy: Send + Sync {
    async fn apply(
        &self,
        conversation: &mut Conversation,
        context: &ReminderContext,
    ) -> Result<SideEffectResult>;

    fn name(&self) -> &'static str;
}

pub struct SystemReminder {
    strategies: Vec<Box<dyn ReminderStrategy>>,
}

impl Default for SystemReminder {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemReminder {
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    pub fn add_strategy(mut self, strategy: Box<dyn ReminderStrategy>) -> Self {
        self.strategies.push(strategy);
        self
    }

    pub async fn apply(
        &self,
        conversation: &mut Conversation,
        context: &ReminderContext,
    ) -> Result<SideEffectResult> {
        for strategy in &self.strategies {
            let result = strategy.apply(conversation, context).await?;

            if matches!(result, SideEffectResult::ExitTurn { .. }) {
                return Ok(result);
            }
        }

        Ok(SideEffectResult::Continue)
    }
}
