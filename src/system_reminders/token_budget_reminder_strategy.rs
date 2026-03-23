use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::agent::Conversation;
use crate::system_reminders::{ReminderContext, ReminderStrategy, SideEffectResult};
use anyhow::Result;

pub struct TokenBudgetReminderStrategy {
    token_count: Arc<AtomicUsize>,
    budget: usize,
    warned_80: AtomicBool,
    warned_95: AtomicBool,
}

impl TokenBudgetReminderStrategy {
    pub fn new(token_count: Arc<AtomicUsize>, budget: usize) -> Self {
        Self {
            token_count,
            budget,
            warned_80: AtomicBool::new(false),
            warned_95: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl ReminderStrategy for TokenBudgetReminderStrategy {
    async fn apply(
        &self,
        conversation: &mut Conversation,
        _context: &ReminderContext,
    ) -> Result<SideEffectResult> {
        let used = self.token_count.load(Ordering::Relaxed);
        let remaining = self.budget.saturating_sub(used);
        let pct = used * 100 / self.budget.max(1);

        if pct >= 95 && !self.warned_95.swap(true, Ordering::Relaxed) {
            conversation.add_system_message(format!(
                "URGENT: Only ~{remaining} tokens remaining ({pct}% used). \
                Stop all new work immediately. Commit whatever you have, push the branch, \
                and create a draft PR now.",
            ));
        } else if pct >= 80 && !self.warned_80.swap(true, Ordering::Relaxed) {
            conversation.add_system_message(format!(
                "TOKEN BUDGET WARNING: {pct}% of token budget used (~{remaining} remaining). \
                Finish your current file, then move straight to: commit → push → create PR. \
                Do not start any new features or exploration.",
            ));
        }

        Ok(SideEffectResult::Continue)
    }

    fn name(&self) -> &'static str {
        "token_budget_reminder"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_strategy(used: usize, budget: usize) -> TokenBudgetReminderStrategy {
        let token_count = Arc::new(AtomicUsize::new(used));
        TokenBudgetReminderStrategy::new(token_count, budget)
    }

    #[tokio::test]
    async fn no_warning_under_80_percent() {
        let strategy = make_strategy(70_000, 100_000);
        let mut conv = Conversation::new();
        let result = strategy
            .apply(&mut conv, &ReminderContext { agent_step: 0 })
            .await
            .unwrap();
        assert!(matches!(result, SideEffectResult::Continue));
        assert!(conv.messages.is_empty());
    }

    #[tokio::test]
    async fn warning_at_80_percent() {
        let strategy = make_strategy(80_000, 100_000);
        let mut conv = Conversation::new();
        strategy
            .apply(&mut conv, &ReminderContext { agent_step: 0 })
            .await
            .unwrap();
        assert_eq!(conv.messages.len(), 1);
        assert!(conv.messages[0].content.as_ref().unwrap().contains("80%"));
    }

    #[tokio::test]
    async fn urgent_warning_at_95_percent() {
        let strategy = make_strategy(95_000, 100_000);
        let mut conv = Conversation::new();
        strategy
            .apply(&mut conv, &ReminderContext { agent_step: 0 })
            .await
            .unwrap();
        assert_eq!(conv.messages.len(), 1);
        assert!(
            conv.messages[0]
                .content
                .as_ref()
                .unwrap()
                .contains("URGENT")
        );
    }

    #[tokio::test]
    async fn warning_fires_only_once() {
        let strategy = make_strategy(80_000, 100_000);
        let mut conv = Conversation::new();
        strategy
            .apply(&mut conv, &ReminderContext { agent_step: 0 })
            .await
            .unwrap();
        strategy
            .apply(&mut conv, &ReminderContext { agent_step: 1 })
            .await
            .unwrap();
        assert_eq!(conv.messages.len(), 1);
    }
}
