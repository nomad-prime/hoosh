use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::agent::Conversation;
use crate::context_management::{
    StrategyResult, TokenAccountant, TokenAccountantStats, TokenUsageRecord,
};

#[async_trait]
pub trait ContextManagementStrategy: Send + Sync {
    async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolOutputTruncationConfig {
    pub max_length: usize,
    pub show_truncation_notice: bool,
    pub smart_truncate: bool,
    pub head_length: usize,
    pub tail_length: usize,
    #[serde(default = "default_preserve_last_tool_result")]
    pub preserve_last_tool_result: bool,
}

fn default_preserve_last_tool_result() -> bool {
    true
}

impl Default for ToolOutputTruncationConfig {
    fn default() -> Self {
        Self {
            max_length: 4000,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
            preserve_last_tool_result: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlidingWindowConfig {
    pub window_size: usize,
    pub preserve_system: bool,
    pub min_messages_before_windowing: usize,
    pub preserve_initial_task: bool,
    #[serde(default = "default_strict_window_size")]
    pub strict_window_size: bool,
}

fn default_strict_window_size() -> bool {
    false
}

impl Default for SlidingWindowConfig {
    fn default() -> Self {
        Self {
            window_size: 40,
            preserve_system: true,
            min_messages_before_windowing: 50,
            preserve_initial_task: true,
            strict_window_size: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextManagerConfig {
    pub max_tokens: usize,
    pub compression_threshold: f32,
    pub preserve_recent_percentage: f32,
    pub warning_threshold: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output_truncation: Option<ToolOutputTruncationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sliding_window: Option<SlidingWindowConfig>,
}

impl Default for ContextManagerConfig {
    fn default() -> Self {
        Self {
            max_tokens: 128_000,
            compression_threshold: 0.80,
            preserve_recent_percentage: 0.50,
            warning_threshold: 0.70,
            tool_output_truncation: Some(ToolOutputTruncationConfig::default()),
            sliding_window: Some(SlidingWindowConfig::default()),
        }
    }
}

impl ContextManagerConfig {
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.compression_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn with_preserve_percentage(mut self, percentage: f32) -> Self {
        self.preserve_recent_percentage = percentage.clamp(0.0, 1.0);
        self
    }

    pub fn with_warning_threshold(mut self, threshold: f32) -> Self {
        self.warning_threshold = threshold.clamp(0.0, 1.0);
        self
    }
}

pub struct ContextManager {
    pub config: ContextManagerConfig,
    pub token_accountant: Arc<TokenAccountant>,
    strategies: Vec<Box<dyn ContextManagementStrategy>>,
}

impl ContextManager {
    pub fn new(config: ContextManagerConfig, token_accountant: Arc<TokenAccountant>) -> Self {
        Self {
            config,
            token_accountant,
            strategies: Vec::new(),
        }
    }

    pub fn with_default_config(token_accountant: Arc<TokenAccountant>) -> Self {
        Self::new(ContextManagerConfig::default(), token_accountant)
    }

    pub fn add_strategy(mut self, strategy: Box<dyn ContextManagementStrategy>) -> Self {
        self.strategies.push(strategy);
        self
    }

    pub fn get_token_pressure(&self, conversation: &Conversation) -> f32 {
        let current = TokenAccountant::estimate_conversation_tokens(conversation);
        (current as f32 / self.config.max_tokens as f32).min(1.0)
    }

    pub fn should_warn_about_pressure(&self, conversation: &Conversation) -> bool {
        self.get_token_pressure(conversation) > self.config.warning_threshold
    }

    pub fn should_warn_about_pressure_value(&self, pressure: f32) -> bool {
        pressure >= self.config.warning_threshold
    }

    pub fn get_token_stats(&self) -> TokenAccountantStats {
        self.token_accountant.statistics()
    }

    pub fn record_token_usage(&self, input_tokens: usize, output_tokens: usize) {
        self.token_accountant
            .record_usage(TokenUsageRecord::from_backend(input_tokens, output_tokens));
    }

    pub async fn apply_strategies(&self, conversation: &mut Conversation) -> Result<()> {
        for strategy in &self.strategies {
            let result = strategy.apply(conversation).await?;

            // If strategy reports target reached, stop processing further strategies
            if result == StrategyResult::TargetReached {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_manager_v2_config() {
        let config = ContextManagerConfig::default()
            .with_max_tokens(100_000)
            .with_threshold(0.75)
            .with_warning_threshold(0.65);

        assert_eq!(config.max_tokens, 100_000);
        assert_eq!(config.compression_threshold, 0.75);
        assert_eq!(config.warning_threshold, 0.65);
    }

    #[test]
    fn test_token_pressure_without_data() {
        let accountant = Arc::new(TokenAccountant::new());
        let config = ContextManagerConfig::default();
        let manager = ContextManager::new(config, accountant);
        let conversation = Conversation::new();

        let pressure = manager.get_token_pressure(&conversation);
        assert_eq!(pressure, 0.0);
    }

    #[test]
    fn test_token_pressure_with_data() {
        use crate::agent::ConversationMessage;

        let accountant = Arc::new(TokenAccountant::new());
        let config = ContextManagerConfig::default();
        let manager = ContextManager::new(config, accountant);

        let mut conversation = Conversation::new();
        conversation.messages.push(ConversationMessage {
            role: "user".to_string(),
            content: Some("x".repeat(20_000)), // ~5000 tokens
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        let pressure = manager.get_token_pressure(&conversation);
        assert!(pressure > 0.0);
        assert!(pressure <= 1.0);
    }

    #[test]
    fn test_should_warn_about_pressure() {
        use crate::agent::ConversationMessage;

        let accountant = Arc::new(TokenAccountant::new());
        let config = ContextManagerConfig::default().with_warning_threshold(0.5);
        let manager = ContextManager::new(config, accountant);

        // Create a conversation with >50% tokens (64K+ for 128K limit)
        let mut conversation = Conversation::new();
        conversation.messages.push(ConversationMessage {
            role: "user".to_string(),
            content: Some("x".repeat(260_000)), // ~65K tokens (>50% of 128K)
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        assert!(manager.should_warn_about_pressure(&conversation));
    }

    #[test]
    fn test_should_warn_about_pressure_value() {
        let accountant = Arc::new(TokenAccountant::new());
        let config = ContextManagerConfig::default().with_warning_threshold(0.70);
        let manager = ContextManager::new(config, accountant);

        // Test direct pressure value checking
        assert!(manager.should_warn_about_pressure_value(0.75));
        assert!(manager.should_warn_about_pressure_value(0.70)); // Equal to threshold
        assert!(!manager.should_warn_about_pressure_value(0.65));
    }

    #[test]
    fn test_get_token_stats() {
        let accountant = TokenAccountant::new();

        accountant.record_usage(TokenUsageRecord::from_backend(100, 50));
        accountant.record_usage(TokenUsageRecord::from_backend(150, 40));

        let accountant = Arc::new(accountant);
        let config = ContextManagerConfig::default();
        let manager = ContextManager::new(config, accountant);

        let stats = manager.get_token_stats();
        assert_eq!(stats.current_context_size, 190);
        assert_eq!(stats.total_consumed, 340);
        assert_eq!(stats.record_count, 2);
    }

    #[tokio::test]
    async fn test_strategy_coordination_stops_at_target_reached() {
        use crate::agent::ConversationMessage;

        // Create a mock strategy that returns TargetReached
        struct TargetReachedStrategy;

        #[async_trait]
        impl ContextManagementStrategy for TargetReachedStrategy {
            async fn apply(&self, _conversation: &mut Conversation) -> Result<StrategyResult> {
                Ok(StrategyResult::TargetReached)
            }
        }

        // Create another mock strategy that tracks if it was called
        use std::sync::Arc as StdArc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let was_called = StdArc::new(AtomicBool::new(false));
        let was_called_clone = was_called.clone();

        struct TrackingStrategy {
            was_called: StdArc<AtomicBool>,
        }

        #[async_trait]
        impl ContextManagementStrategy for TrackingStrategy {
            async fn apply(&self, _conversation: &mut Conversation) -> Result<StrategyResult> {
                self.was_called.store(true, Ordering::Relaxed);
                Ok(StrategyResult::Applied)
            }
        }

        let accountant = Arc::new(TokenAccountant::new());
        let config = ContextManagerConfig::default();
        let mut manager = ContextManager::new(config, accountant);

        // Add TargetReached strategy first, then tracking strategy
        manager = manager.add_strategy(Box::new(TargetReachedStrategy));
        manager = manager.add_strategy(Box::new(TrackingStrategy {
            was_called: was_called_clone,
        }));

        let mut conversation = Conversation::new();
        conversation.messages.push(ConversationMessage {
            role: "user".to_string(),
            content: Some("test".to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // Apply strategies
        manager.apply_strategies(&mut conversation).await.unwrap();

        // Second strategy should NOT have been called because first one returned TargetReached
        assert!(!was_called.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_all_strategies_run_without_target_reached() {
        use crate::agent::ConversationMessage;
        use std::sync::Arc as StdArc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = StdArc::new(AtomicUsize::new(0));

        struct CountingStrategy {
            call_count: StdArc<AtomicUsize>,
        }

        #[async_trait]
        impl ContextManagementStrategy for CountingStrategy {
            async fn apply(&self, _conversation: &mut Conversation) -> Result<StrategyResult> {
                self.call_count.fetch_add(1, Ordering::Relaxed);
                Ok(StrategyResult::Applied)
            }
        }

        let accountant = Arc::new(TokenAccountant::new());
        let config = ContextManagerConfig::default();
        let mut manager = ContextManager::new(config, accountant);

        // Add three strategies - all should run
        for _ in 0..3 {
            manager = manager.add_strategy(Box::new(CountingStrategy {
                call_count: call_count.clone(),
            }));
        }

        let mut conversation = Conversation::new();
        conversation.messages.push(ConversationMessage {
            role: "user".to_string(),
            content: Some("test".to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // Apply strategies
        manager.apply_strategies(&mut conversation).await.unwrap();

        // All three strategies should have been called
        assert_eq!(call_count.load(Ordering::Relaxed), 3);
    }
}
