use serde::{Deserialize, Serialize};

use crate::cascades::{ExecutionTier, TaskComplexity};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeContext {
    pub initial_complexity: Option<TaskComplexity>,
    pub current_tier: ExecutionTier,
    pub escalation_count: u32,
    pub max_escalations: u32,
    pub escalation_history: Vec<ExecutionTier>,
}

impl CascadeContext {
    pub fn new(initial_tier: ExecutionTier, max_escalations: u32) -> Self {
        Self {
            initial_complexity: None,
            current_tier: initial_tier,
            escalation_count: 0,
            max_escalations,
            escalation_history: vec![initial_tier],
        }
    }

    pub fn with_complexity(
        complexity: TaskComplexity,
        max_escalations: u32,
    ) -> Self {
        let tier = complexity.tier;

        Self {
            initial_complexity: Some(complexity),
            current_tier: tier,
            escalation_count: 0,
            max_escalations,
            escalation_history: vec![tier],
        }
    }

    pub fn escalate(&mut self) -> Result<ExecutionTier, String> {
        if self.escalation_count >= self.max_escalations {
            return Err("Maximum escalations reached".to_string());
        }

        let next_tier = match self.current_tier {
            ExecutionTier::Light => ExecutionTier::Medium,
            ExecutionTier::Medium => ExecutionTier::Heavy,
            ExecutionTier::Heavy => return Err("Already at maximum tier".to_string()),
        };

        self.current_tier = next_tier;
        self.escalation_count += 1;
        self.escalation_history.push(next_tier);

        Ok(next_tier)
    }

    pub fn can_escalate(&self) -> bool {
        self.escalation_count < self.max_escalations && self.current_tier != ExecutionTier::Heavy
    }

    pub fn next_tier(&self) -> Option<ExecutionTier> {
        if !self.can_escalate() {
            return None;
        }

        Some(match self.current_tier {
            ExecutionTier::Light => ExecutionTier::Medium,
            ExecutionTier::Medium => ExecutionTier::Heavy,
            ExecutionTier::Heavy => return None,
        })
    }

    pub fn escalation_summary(&self) -> String {
        format!(
            "Started at {}, now at {}, {} escalation(s)",
            self.escalation_history[0],
            self.current_tier,
            self.escalation_count
        )
    }
}

impl Default for CascadeContext {
    fn default() -> Self {
        Self::new(ExecutionTier::Medium, 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_new_context() {
        let ctx = CascadeContext::new(ExecutionTier::Light, 2);
        assert_eq!(ctx.current_tier, ExecutionTier::Light);
        assert_eq!(ctx.escalation_count, 0);
        assert!(ctx.can_escalate());
    }

    #[test]
    fn test_escalate_from_light_to_medium() {
        let mut ctx = CascadeContext::new(ExecutionTier::Light, 2);
        let result = ctx.escalate();
        assert!(result.is_ok());
        assert_eq!(ctx.current_tier, ExecutionTier::Medium);
        assert_eq!(ctx.escalation_count, 1);
    }

    #[test]
    fn test_escalate_from_medium_to_heavy() {
        let mut ctx = CascadeContext::new(ExecutionTier::Medium, 2);
        let result = ctx.escalate();
        assert!(result.is_ok());
        assert_eq!(ctx.current_tier, ExecutionTier::Heavy);
        assert_eq!(ctx.escalation_count, 1);
    }

    #[test]
    fn test_cannot_escalate_from_heavy() {
        let mut ctx = CascadeContext::new(ExecutionTier::Heavy, 2);
        let result = ctx.escalate();
        assert!(result.is_err());
        assert_eq!(ctx.current_tier, ExecutionTier::Heavy);
    }

    #[test]
    fn test_max_escalations_enforced() {
        let mut ctx = CascadeContext::new(ExecutionTier::Light, 1);
        assert!(ctx.escalate().is_ok()); // Light -> Medium
        assert!(ctx.escalate().is_err()); // Medium -> Heavy fails (max reached)
        assert_eq!(ctx.current_tier, ExecutionTier::Medium);
    }

    #[test]
    fn test_escalation_history() {
        let mut ctx = CascadeContext::new(ExecutionTier::Light, 2);
        ctx.escalate().ok();
        ctx.escalate().ok();
        assert_eq!(ctx.escalation_history.len(), 3);
        assert_eq!(ctx.escalation_history[0], ExecutionTier::Light);
        assert_eq!(ctx.escalation_history[1], ExecutionTier::Medium);
        assert_eq!(ctx.escalation_history[2], ExecutionTier::Heavy);
    }

    #[test]
    fn test_next_tier() {
        let ctx = CascadeContext::new(ExecutionTier::Light, 2);
        assert_eq!(ctx.next_tier(), Some(ExecutionTier::Medium));

        let ctx = CascadeContext::new(ExecutionTier::Medium, 2);
        assert_eq!(ctx.next_tier(), Some(ExecutionTier::Heavy));

        let ctx = CascadeContext::new(ExecutionTier::Heavy, 2);
        assert_eq!(ctx.next_tier(), None);
    }
}
