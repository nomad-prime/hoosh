use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::context_management::{TokenAccountant, TokenAccountantStats, TokenUsageRecord};
use crate::conversations::Conversation;

#[async_trait]
pub trait ContextManagementStrategy: Send + Sync {
    async fn apply(&self, conversation: &mut Conversation) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutputTruncationConfig {
    pub max_length: usize,
    pub show_truncation_notice: bool,
    pub smart_truncate: bool,
    pub head_length: usize,
    pub tail_length: usize,
}

impl Default for ToolOutputTruncationConfig {
    fn default() -> Self {
        Self {
            max_length: 4000,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagerConfig {
    pub max_tokens: usize,
    pub compression_threshold: f32,
    pub preserve_recent_percentage: f32,
    pub warning_threshold: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output_truncation: Option<ToolOutputTruncationConfig>,
}

impl Default for ContextManagerConfig {
    fn default() -> Self {
        Self {
            max_tokens: 128_000,
            compression_threshold: 0.80,
            preserve_recent_percentage: 0.50,
            warning_threshold: 0.70,
            tool_output_truncation: Some(ToolOutputTruncationConfig::default()),
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

    pub fn get_token_pressure(&self) -> f32 {
        let current = self.token_accountant.current_context_tokens();
        (current as f32 / self.config.max_tokens as f32).min(1.0)
    }

    pub fn should_warn_about_pressure(&self) -> bool {
        self.get_token_pressure() > self.config.warning_threshold
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
            strategy.apply(conversation).await?;
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

        let pressure = manager.get_token_pressure();
        assert_eq!(pressure, 0.0);
    }

    #[test]
    fn test_token_pressure_with_data() {
        let accountant = TokenAccountant::new();

        accountant.record_usage(TokenUsageRecord::from_backend(5_000, 2_000));

        let accountant = Arc::new(accountant);
        let config = ContextManagerConfig::default();
        let manager = ContextManager::new(config, accountant);

        let pressure = manager.get_token_pressure();
        assert!(pressure > 0.0);
        assert!(pressure <= 1.0);
    }

    #[test]
    fn test_should_warn_about_pressure() {
        let accountant = TokenAccountant::new();

        accountant.record_usage(TokenUsageRecord::from_backend(126_000, 2_000));

        let accountant = Arc::new(accountant);
        let config = ContextManagerConfig::default().with_warning_threshold(0.5);
        let manager = ContextManager::new(config, accountant);

        assert!(manager.should_warn_about_pressure());
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
}
