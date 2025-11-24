mod periodic_core_reminder_strategy;

use anyhow::Result;

use crate::agent::Agent;
use crate::agent::Conversation;

#[derive(Debug)]
pub enum SideEffectResult {
    Continue,
    ExitTurn,
}

pub struct ReminderContext {
    pub step: usize,
    pub max_steps: usize,
    pub total_tokens: usize,
}

#[async_trait::async_trait]
pub trait ReminderStrategy: Send + Sync {
    async fn apply(
        &self,
        ctx: &ReminderContext,
        conversation: &mut Conversation,
        agent: &Agent,
    ) -> Result<SideEffectResult>;

    fn name(&self) -> &'static str;
}

pub struct SystemReminder {
    strategies: Vec<Box<dyn ReminderStrategy>>,
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
        ctx: &ReminderContext,
        conversation: &mut Conversation,
        agent: &Agent,
    ) -> Result<SideEffectResult> {
        for strategy in &self.strategies {
            let result = strategy.apply(ctx, conversation, agent).await?;

            if matches!(result, SideEffectResult::ExitTurn) {
                return Ok(SideEffectResult::ExitTurn);
            }
        }

        Ok(SideEffectResult::Continue)
    }
}
