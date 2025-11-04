use anyhow::Result;
use async_trait::async_trait;

use crate::context_management::{ContextManagementStrategy, SlidingWindowConfig};
use crate::conversations::{Conversation, ConversationMessage};

pub struct SlidingWindowStrategy {
    config: SlidingWindowConfig,
}

impl SlidingWindowStrategy {
    pub fn new(config: SlidingWindowConfig) -> Self {
        Self { config }
    }

    fn is_system_message(&self, message: &ConversationMessage) -> bool {
        message.role == "system"
    }

    fn is_user_message(&self, message: &ConversationMessage) -> bool {
        message.role == "user"
    }

    fn should_preserve(&self, message: &ConversationMessage, index: usize) -> bool {
        if self.config.preserve_system && self.is_system_message(message) {
            return true;
        }

        if self.config.preserve_initial_task && self.is_user_message(message) && index <= 1 {
            return true;
        }

        false
    }
}

#[async_trait]
impl ContextManagementStrategy for SlidingWindowStrategy {
    async fn apply(&self, conversation: &mut Conversation) -> Result<()> {
        let message_count = conversation.messages.len();

        if message_count <= self.config.min_messages_before_windowing {
            return Ok(());
        }

        let total_to_keep = self.config.window_size;

        if message_count <= total_to_keep {
            return Ok(());
        }

        // Mark which messages to preserve (maintaining their index)
        let mut keep_flags: Vec<bool> = conversation
            .messages
            .iter()
            .enumerate()
            .map(|(index, message)| self.should_preserve(message, index))
            .collect();

        let preserved_count = keep_flags.iter().filter(|&&k| k).count();

        if preserved_count >= total_to_keep {
            // Keep only preserved messages (maintaining order)
            let mut i = 0;
            conversation.messages.retain(|_| {
                let should_keep = keep_flags[i];
                i += 1;
                should_keep
            });

            return Ok(());
        }

        // We need to keep `total_to_keep - preserved_count` recent non-preserved messages
        let regular_to_keep = total_to_keep - preserved_count;

        // Mark the most recent non-preserved messages to keep
        let mut regular_kept = 0;
        for i in (0..keep_flags.len()).rev() {
            if !keep_flags[i] && regular_kept < regular_to_keep {
                keep_flags[i] = true;
                regular_kept += 1;
            }
        }

        // Filter messages while maintaining original order
        let mut i = 0;
        conversation.messages.retain(|_| {
            let should_keep = keep_flags[i];
            i += 1;
            should_keep
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_windowing_below_threshold() {
        let config = SlidingWindowConfig {
            window_size: 10,
            min_messages_before_windowing: 50,
            ..Default::default()
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        for i in 0..30 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(conversation.messages.len(), 30);
    }

    #[tokio::test]
    async fn test_basic_sliding_window() {
        let config = SlidingWindowConfig {
            window_size: 10,
            min_messages_before_windowing: 5,
            preserve_system: false,
            preserve_initial_task: false,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        for i in 0..20 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(conversation.messages.len(), 10);

        assert!(
            conversation.messages[0]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-10")
        );
        assert!(
            conversation.messages[9]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-19")
        );
    }

    #[tokio::test]
    async fn test_preserves_system_messages() {
        let config = SlidingWindowConfig {
            window_size: 10,
            min_messages_before_windowing: 5,
            preserve_system: true,
            preserve_initial_task: false,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        conversation.add_system_message("You are a helpful assistant".to_string());

        for i in 1..21 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(conversation.messages.len(), 10);

        assert_eq!(conversation.messages[0].role, "system");
        assert!(
            conversation.messages[0]
                .content
                .as_ref()
                .unwrap()
                .contains("helpful assistant")
        );

        assert!(
            conversation.messages[1]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-12")
        );
        assert!(
            conversation.messages[9]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-20")
        );
    }

    #[tokio::test]
    async fn test_preserves_initial_task() {
        let config = SlidingWindowConfig {
            window_size: 10,
            min_messages_before_windowing: 5,
            preserve_system: true,
            preserve_initial_task: true,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        conversation.add_system_message("system".to_string());

        conversation.add_user_message("Build a web server".to_string());

        for i in 2..22 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(conversation.messages.len(), 10);

        assert_eq!(conversation.messages[0].role, "system");

        assert!(
            conversation.messages[1]
                .content
                .as_ref()
                .unwrap()
                .contains("Build a web server")
        );

        assert!(
            conversation.messages[2]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-14")
        );
        assert!(
            conversation.messages[9]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-21")
        );
    }

    #[tokio::test]
    async fn test_window_size_includes_preserved() {
        let config = SlidingWindowConfig {
            window_size: 5,
            min_messages_before_windowing: 3,
            preserve_system: true,
            preserve_initial_task: true,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        conversation.add_system_message("system".to_string());
        conversation.add_user_message("initial".to_string());

        for i in 2..12 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(conversation.messages.len(), 5);

        assert!(
            conversation.messages[0]
                .content
                .as_ref()
                .unwrap()
                .contains("system")
        );
        assert!(
            conversation.messages[1]
                .content
                .as_ref()
                .unwrap()
                .contains("initial")
        );
        assert!(
            conversation.messages[2]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-9")
        );
        assert!(
            conversation.messages[4]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-11")
        );
    }

    #[tokio::test]
    async fn test_sliding_window_integration() {
        use crate::context_management::{ContextManager, ContextManagerConfig, TokenAccountant};
        use std::sync::Arc;

        let config = ContextManagerConfig {
            sliding_window: Some(SlidingWindowConfig {
                window_size: 10,
                min_messages_before_windowing: 5,
                preserve_system: true,
                preserve_initial_task: true,
            }),
            tool_output_truncation: None,
            ..Default::default()
        };

        let token_accountant = Arc::new(TokenAccountant::new());
        let mut context_manager_builder = ContextManager::new(config.clone(), token_accountant);

        if let Some(sliding_window_config) = config.sliding_window {
            let sliding_window_strategy = SlidingWindowStrategy::new(sliding_window_config);
            context_manager_builder =
                context_manager_builder.add_strategy(Box::new(sliding_window_strategy));
        }

        let context_manager = context_manager_builder;

        let mut conversation = Conversation::new();

        conversation.add_system_message("You are helpful".to_string());
        conversation.add_user_message("Build app".to_string());

        for i in 2..30 {
            conversation.add_user_message(format!("msg-{}", i));
            conversation.add_assistant_message(Some(format!("response-{}", i)), None);
        }

        context_manager
            .apply_strategies(&mut conversation)
            .await
            .unwrap();

        assert_eq!(conversation.messages.len(), 10);

        assert_eq!(conversation.messages[0].role, "system");
        assert!(
            conversation.messages[1]
                .content
                .as_ref()
                .unwrap()
                .contains("Build app")
        );
    }
}
