use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::conversations::{ConversationMessage, MessageSummarizer, TokenAccountant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagerConfig {
    pub max_tokens: usize,
    pub compression_threshold: f32,
    pub preserve_recent_percentage: f32,
    pub warning_threshold: f32,
}

impl Default for ContextManagerConfig {
    fn default() -> Self {
        Self {
            max_tokens: 10000,
            compression_threshold: 0.80,
            preserve_recent_percentage: 0.50,
            warning_threshold: 0.70,
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
    summarizer: Arc<MessageSummarizer>,
    token_accountant: Arc<TokenAccountant>,
}

impl ContextManager {
    pub fn new(
        config: ContextManagerConfig,
        summarizer: Arc<MessageSummarizer>,
        token_accountant: Arc<TokenAccountant>,
    ) -> Self {
        Self {
            config,
            summarizer,
            token_accountant,
        }
    }

    pub fn with_default_config(
        summarizer: Arc<MessageSummarizer>,
        token_accountant: Arc<TokenAccountant>,
    ) -> Self {
        Self::new(
            ContextManagerConfig::default(),
            summarizer,
            token_accountant,
        )
    }

    /// Get current token pressure based on actual usage
    pub fn get_token_pressure(&self) -> f32 {
        let total = self.token_accountant.total_tokens();
        (total as f32 / self.config.max_tokens as f32).min(1.0)
    }

    /// Check if token pressure warrants a warning
    pub fn should_warn_about_pressure(&self) -> bool {
        self.get_token_pressure() > self.config.warning_threshold
    }

    /// Check if we should trigger compression based on current token usage
    pub fn should_compress(&self, current_context_tokens: usize) -> bool {
        let threshold =
            (self.config.max_tokens as f32 * self.config.compression_threshold) as usize;
        current_context_tokens > threshold
    }

    /// Get calibration statistics
    pub fn get_token_stats(&self) -> crate::conversations::TokenAccountantStats {
        self.token_accountant.statistics()
    }

    /// Split messages into old and recent sections
    fn split_messages(
        &self,
        messages: &[ConversationMessage],
    ) -> (Vec<ConversationMessage>, Vec<ConversationMessage>) {
        let total = messages.len();
        let split_point =
            ((total as f32) * (1.0 - self.config.preserve_recent_percentage)) as usize;
        let split_point = split_point.max(1).min(total - 1);

        let (old, recent) = messages.split_at(split_point);
        (old.to_vec(), recent.to_vec())
    }

    /// Compress message history by summarizing old messages
    pub async fn compress_messages(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<Vec<ConversationMessage>> {
        let (old_messages, recent_messages) = self.split_messages(messages);

        let summary = self
            .summarizer
            .summarize(&old_messages, None)
            .await
            .context("Failed to summarize old messages during context compression")?;

        let summary_message = ConversationMessage {
            role: "system".to_string(),
            content: Some(format!(
                "[CONTEXT COMPRESSION: Previous {} messages summarized]\n\n{}\n\n[End of summary - recent context continues below]",
                old_messages.len(),
                summary
            )),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        let mut compressed = vec![summary_message];
        compressed.extend(recent_messages);

        Ok(compressed)
    }

    /// Apply context compression to messages if needed
    pub async fn apply_context_compression(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<Vec<ConversationMessage>> {
        // Use actual token count from accountant
        let current_tokens = self.token_accountant.total_tokens();

        if self.should_compress(current_tokens) {
            self.compress_messages(messages).await
        } else {
            Ok(messages.to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::mock::MockBackend;

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
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = Arc::new(crate::conversations::MessageSummarizer::new(mock_backend));
        let accountant = Arc::new(TokenAccountant::new());

        let config = ContextManagerConfig::default();
        let manager = ContextManager::new(config, summarizer, accountant);

        // Without data, pressure should be 0
        let pressure = manager.get_token_pressure();
        assert_eq!(pressure, 0.0);
    }

    #[test]
    fn test_token_pressure_with_data() {
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = Arc::new(crate::conversations::MessageSummarizer::new(mock_backend));
        let mut accountant = TokenAccountant::new();

        // Add some actual token usage data
        for _ in 0..5 {
            accountant.record_usage(crate::conversations::TokenUsageRecord::from_backend(
                10_000, 5_000,
            ));
        }

        let accountant = Arc::new(accountant);
        let config = ContextManagerConfig::default();
        let manager = ContextManager::new(config, summarizer, accountant);

        let pressure = manager.get_token_pressure();
        assert!(pressure > 0.0);
        assert!(pressure <= 1.0);
    }

    #[test]
    fn test_should_warn_about_pressure() {
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = Arc::new(crate::conversations::MessageSummarizer::new(mock_backend));
        let mut accountant = TokenAccountant::new();

        // Add usage that will create high pressure
        for _ in 0..5 {
            accountant.record_usage(crate::conversations::TokenUsageRecord::from_backend(
                50_000, 25_000,
            ));
        }

        let accountant = Arc::new(accountant);
        let config = ContextManagerConfig::default().with_warning_threshold(0.5);
        let manager = ContextManager::new(config, summarizer, accountant);

        assert!(manager.should_warn_about_pressure());
    }

    #[test]
    fn test_get_token_stats() {
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = Arc::new(crate::conversations::MessageSummarizer::new(mock_backend));
        let mut accountant = TokenAccountant::new();

        accountant.record_usage(crate::conversations::TokenUsageRecord::from_backend(80, 20));
        accountant.record_usage(crate::conversations::TokenUsageRecord::from_backend(90, 10));

        let accountant = Arc::new(accountant);
        let config = ContextManagerConfig::default();
        let manager = ContextManager::new(config, summarizer, accountant);

        let stats = manager.get_token_stats();
        assert_eq!(stats.total_tokens, 200);
        assert_eq!(stats.record_count, 2);
    }
}
