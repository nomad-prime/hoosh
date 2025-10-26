use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::conversations::{ConversationMessage, MessageSummarizer};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagerConfig {
    /// Maximum tokens allowed per model
    pub max_tokens: usize,
    /// Trigger compression when reaching this fraction of max_tokens (0.0-1.0)
    pub compression_threshold: f32,
    /// Percentage of recent messages to preserve during compression (0.0-1.0)
    pub preserve_recent_percentage: f32,
}

impl Default for ContextManagerConfig {
    fn default() -> Self {
        Self {
            max_tokens: 64_000, // Conservative default
            compression_threshold: 0.80,
            preserve_recent_percentage: 0.50,
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
}

/// Estimates token counts for messages
pub struct TokenEstimator;

impl TokenEstimator {
    /// Rough estimate of tokens for a message
    /// Uses ~4 characters per token as a conservative estimate
    pub fn estimate_tokens(message: &ConversationMessage) -> usize {
        let mut tokens = 0;

        // Account for message structure overhead
        tokens += 4;

        // Content tokens
        if let Some(content) = &message.content {
            tokens += Self::estimate_text_tokens(content);
        }

        // Tool call tokens (if present)
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                tokens += Self::estimate_text_tokens(&tool_call.function.name);
                tokens += Self::estimate_text_tokens(&tool_call.function.arguments);
                tokens += 10; // Overhead for tool call structure
            }
        }

        tokens
    }

    /// Estimate tokens for a list of messages
    pub fn estimate_messages_tokens(messages: &[ConversationMessage]) -> usize {
        messages.iter().map(Self::estimate_tokens).sum::<usize>() + (messages.len() * 2) // Add overhead for message boundaries
    }

    fn estimate_text_tokens(text: &str) -> usize {
        // Conservative estimate: ~4 characters per token
        (text.len() as f32 / 4.0).ceil() as usize
    }
}

/// Manages context compression and token pressure detection
pub struct ContextManager {
    pub config: ContextManagerConfig,
    summarizer: Arc<MessageSummarizer>,
}

impl ContextManager {
    pub fn new(config: ContextManagerConfig, summarizer: Arc<MessageSummarizer>) -> Self {
        Self { config, summarizer }
    }

    pub fn with_default_config(summarizer: Arc<MessageSummarizer>) -> Self {
        Self::new(ContextManagerConfig::default(), summarizer)
    }

    /// Check if messages are approaching token limit
    pub fn should_compress(&self, messages: &[ConversationMessage]) -> bool {
        let current_tokens = TokenEstimator::estimate_messages_tokens(messages);
        let threshold_tokens =
            (self.config.max_tokens as f32 * self.config.compression_threshold) as usize;
        current_tokens > threshold_tokens
    }

    /// Get current token pressure as a fraction (0.0-1.0)
    pub fn get_token_pressure(&self, messages: &[ConversationMessage]) -> f32 {
        let current_tokens = TokenEstimator::estimate_messages_tokens(messages);
        (current_tokens as f32 / self.config.max_tokens as f32).min(1.0)
    }

    /// Split messages into old and recent sections
    fn split_messages(
        &self,
        messages: &[ConversationMessage],
    ) -> (Vec<ConversationMessage>, Vec<ConversationMessage>) {
        let total = messages.len();
        let split_point =
            ((total as f32) * (1.0 - self.config.preserve_recent_percentage)) as usize;
        let split_point = split_point.max(1).min(total - 1); // Ensure we don't split at edges

        let (old, recent) = messages.split_at(split_point);
        (old.to_vec(), recent.to_vec())
    }

    /// Compress message history by summarizing old messages
    pub async fn compress_messages(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<Vec<ConversationMessage>> {
        if !self.should_compress(messages) {
            return Ok(messages.to_vec());
        }

        let (old_messages, recent_messages) = self.split_messages(messages);

        // Summarize old messages
        let summary = self
            .summarizer
            .summarize(&old_messages, None)
            .await
            .context("Failed to summarize old messages during context compression")?;

        // Create system message with summary
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

        // Build new compressed context
        let mut compressed = vec![summary_message];
        compressed.extend(recent_messages);

        Ok(compressed)
    }

    /// Apply context compression to messages if needed
    pub async fn apply_context_compression(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<Vec<ConversationMessage>> {
        if self.should_compress(messages) {
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
    fn test_token_estimator_basic() {
        let msg = ConversationMessage {
            role: "user".to_string(),
            content: Some("Hello world".to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        let tokens = TokenEstimator::estimate_tokens(&msg);
        assert!(tokens > 0);
        assert!(tokens < 50); // "Hello world" should be ~3-4 tokens
    }

    #[test]
    fn test_token_estimator_multiple_messages() {
        let messages = vec![
            ConversationMessage {
                role: "user".to_string(),
                content: Some("Hello".to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            ConversationMessage {
                role: "assistant".to_string(),
                content: Some("Hi there!".to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ];

        let total_tokens = TokenEstimator::estimate_messages_tokens(&messages);
        assert!(total_tokens > 0);
    }

    #[test]
    fn test_context_manager_config_defaults() {
        let config = ContextManagerConfig::default();
        assert_eq!(config.max_tokens, 128_000);
        assert_eq!(config.compression_threshold, 0.80);
        assert_eq!(config.preserve_recent_percentage, 0.50);
    }

    #[test]
    fn test_context_manager_config_builder() {
        let config = ContextManagerConfig::default()
            .with_max_tokens(100_000)
            .with_threshold(0.75)
            .with_preserve_percentage(0.60);

        assert_eq!(config.max_tokens, 100_000);
        assert_eq!(config.compression_threshold, 0.75);
        assert_eq!(config.preserve_recent_percentage, 0.60);
    }

    #[test]
    fn test_context_manager_should_compress() {
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = Arc::new(MessageSummarizer::new(mock_backend));
        let config = ContextManagerConfig {
            max_tokens: 100,
            compression_threshold: 0.5,
            preserve_recent_percentage: 0.5,
        };

        let manager = ContextManager::new(config, summarizer);

        // Create messages that will exceed threshold
        let mut messages = Vec::new();
        for i in 0..20 {
            messages.push(ConversationMessage {
                role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: Some(format!(
                    "Message {}: This is a test message with some content",
                    i
                )),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        let should_compress = manager.should_compress(&messages);
        assert!(
            should_compress,
            "Should trigger compression with many messages"
        );
    }

    #[test]
    fn test_context_manager_token_pressure() {
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = Arc::new(MessageSummarizer::new(mock_backend));
        let config = ContextManagerConfig::default();
        let manager = ContextManager::new(config, summarizer);

        let messages = vec![ConversationMessage {
            role: "user".to_string(),
            content: Some("Hello".to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }];

        let pressure = manager.get_token_pressure(&messages);
        assert!((0.0..=1.0).contains(&pressure));
        assert!(pressure < 0.01); // Single message should be minimal pressure
    }

    #[test]
    fn test_split_messages() {
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = Arc::new(MessageSummarizer::new(mock_backend));
        let config = ContextManagerConfig::default();
        let manager = ContextManager::new(config, summarizer);

        let messages: Vec<_> = (0..10)
            .map(|i| ConversationMessage {
                role: "user".to_string(),
                content: Some(format!("Message {}", i)),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            })
            .collect();

        let (old, recent) = manager.split_messages(&messages);
        assert!(!old.is_empty());
        assert!(!recent.is_empty());
        assert_eq!(old.len() + recent.len(), messages.len());
    }

    #[test]
    fn test_config_builder_chain() {
        let config = ContextManagerConfig::default()
            .with_max_tokens(50_000)
            .with_threshold(0.60)
            .with_preserve_percentage(0.40);

        assert_eq!(config.max_tokens, 50_000);
        assert_eq!(config.compression_threshold, 0.60);
        assert_eq!(config.preserve_recent_percentage, 0.40);
    }

    #[test]
    fn test_config_threshold_clamping() {
        // Thresholds should be clamped to [0.0, 1.0]
        let config = ContextManagerConfig::default()
            .with_threshold(2.0) // Should be clamped to 1.0
            .with_preserve_percentage(-0.5); // Should be clamped to 0.0

        assert_eq!(config.compression_threshold, 1.0);
        assert_eq!(config.preserve_recent_percentage, 0.0);
    }

    #[test]
    fn test_token_pressure_progression() {
        let mock_backend = Arc::new(MockBackend::new());
        let summarizer = Arc::new(MessageSummarizer::new(mock_backend));

        let config = ContextManagerConfig {
            max_tokens: 1000,
            compression_threshold: 0.80,
            preserve_recent_percentage: 0.50,
        };
        let manager = ContextManager::new(config, summarizer);

        let mut conversation = crate::conversations::Conversation::new();

        // Start with no pressure
        let mut pressure = manager.get_token_pressure(&conversation.messages);
        assert_eq!(pressure, 0.0);

        // Add messages and track pressure increase
        for i in 0..20 {
            conversation.add_user_message(format!(
                "Message {}: Adding more content to increase token count in the conversation",
                i
            ));

            let new_pressure = manager.get_token_pressure(&conversation.messages);
            assert!(
                new_pressure >= pressure,
                "Pressure should increase monotonically"
            );
            pressure = new_pressure;
        }

        // Final pressure should be reasonable
        assert!(pressure > 0.0 && pressure <= 1.0);
        assert!(pressure < 1.0, "Should not exceed 1.0");
    }
}
