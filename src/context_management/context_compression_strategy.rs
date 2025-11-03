use crate::context_management::{
    ContextManagementStrategy, ContextManagerConfig, MessageSummarizer, TokenAccountant,
};
use crate::{Conversation, ConversationMessage};
use anyhow::Context;
use async_trait::async_trait;
use std::sync::Arc;

pub struct ContextCompressionStrategy {
    config: ContextManagerConfig,
    summarizer: Arc<MessageSummarizer>,
    token_accountant: Arc<TokenAccountant>,
}

impl ContextCompressionStrategy {
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

    fn should_compress(&self) -> bool {
        let current = self.token_accountant.current_context_tokens();
        let threshold =
            (self.config.max_tokens as f32 * self.config.compression_threshold) as usize;
        current > threshold
    }

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

    async fn compress_messages(
        &self,
        messages: &[ConversationMessage],
    ) -> anyhow::Result<Vec<ConversationMessage>> {
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
}

#[async_trait]
impl ContextManagementStrategy for ContextCompressionStrategy {
    async fn apply(&self, conversation: &mut Conversation) -> anyhow::Result<()> {
        if self.should_compress() {
            let compressed_messages = self.compress_messages(&conversation.messages).await?;
            conversation.messages = compressed_messages;
        }
        Ok(())
    }
}
