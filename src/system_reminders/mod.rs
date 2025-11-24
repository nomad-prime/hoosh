pub mod periodic_core_reminder_strategy;
pub use periodic_core_reminder_strategy::PeriodicCoreReminderStrategy;

use anyhow::Result;

use crate::agent::Agent;
use crate::agent::Conversation;

#[derive(Debug)]
pub enum SideEffectResult {
    Continue,
    ExitTurn,
}

pub struct ReminderContext {
    pub total_tokens: usize,
}

#[async_trait::async_trait]
pub trait ReminderStrategy: Send + Sync {
    async fn apply(
        &self,
        conversation: &mut Conversation,
        agent: &Agent,
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
        agent: &Agent,
    ) -> Result<SideEffectResult> {
        for strategy in &self.strategies {
            let result = strategy.apply(conversation, agent).await?;

            if matches!(result, SideEffectResult::ExitTurn) {
                return Ok(SideEffectResult::ExitTurn);
            }
        }

        Ok(SideEffectResult::Continue)
    }
}
