use anyhow::Result;
use async_trait::async_trait;

use crate::agent::{Conversation, ConversationMessage};
use crate::context_management::{ContextManagementStrategy, SlidingWindowConfig};

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

    fn should_preserve(&self, message: &ConversationMessage, is_first_user_message: bool) -> bool {
        if self.config.preserve_system && self.is_system_message(message) {
            return true;
        }

        if self.config.preserve_initial_task && is_first_user_message {
            return true;
        }

        false
    }

    fn ensure_tool_call_pairs(&self, messages: &[ConversationMessage], keep_flags: &mut [bool]) {
        for i in 0..messages.len() {
            if !keep_flags[i] {
                continue;
            }

            if messages[i].role == "assistant"
                && let Some(tool_calls) = &messages[i].tool_calls
            {
                for tool_call in tool_calls {
                    for j in (i + 1)..messages.len() {
                        if messages[j].role == "tool"
                            && messages[j].tool_call_id.as_ref() == Some(&tool_call.id)
                        {
                            keep_flags[j] = true;
                        }
                    }
                }
            }
        }

        for i in 0..messages.len() {
            if !keep_flags[i] {
                continue;
            }

            if messages[i].role == "tool"
                && let Some(tool_call_id) = &messages[i].tool_call_id
            {
                for j in (0..i).rev() {
                    if messages[j].role == "assistant"
                        && let Some(tool_calls) = &messages[j].tool_calls
                        && tool_calls.iter().any(|tc| &tc.id == tool_call_id)
                    {
                        keep_flags[j] = true;
                        for tool_call in tool_calls {
                            for k in (j + 1)..messages.len() {
                                if messages[k].role == "tool"
                                    && messages[k].tool_call_id.as_ref() == Some(&tool_call.id)
                                {
                                    keep_flags[k] = true;
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
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

        // Find the index of the first user message
        let first_user_message_index = conversation
            .messages
            .iter()
            .position(|msg| self.is_user_message(msg));

        // Mark which messages to preserve (maintaining their index)
        let mut keep_flags: Vec<bool> = conversation
            .messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let is_first_user_message = first_user_message_index == Some(index);
                self.should_preserve(message, is_first_user_message)
            })
            .collect();

        let preserved_count = keep_flags.iter().filter(|&&k| k).count();

        if preserved_count >= total_to_keep {
            self.ensure_tool_call_pairs(&conversation.messages, &mut keep_flags);

            conversation.messages = conversation
                .messages
                .drain(..)
                .enumerate()
                .filter_map(|(i, msg)| if keep_flags[i] { Some(msg) } else { None })
                .collect();

            return Ok(());
        }

        let regular_to_keep = total_to_keep - preserved_count;

        let mut regular_kept = 0;
        for i in (0..keep_flags.len()).rev() {
            if !keep_flags[i] && regular_kept < regular_to_keep {
                keep_flags[i] = true;
                regular_kept += 1;
            }
        }

        self.ensure_tool_call_pairs(&conversation.messages, &mut keep_flags);

        conversation.messages = conversation
            .messages
            .drain(..)
            .enumerate()
            .filter_map(|(i, msg)| if keep_flags[i] { Some(msg) } else { None })
            .collect();

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

    #[tokio::test]
    async fn test_preserves_initial_task_with_multiple_system_messages() {
        let config = SlidingWindowConfig {
            window_size: 10,
            min_messages_before_windowing: 5,
            preserve_system: true,
            preserve_initial_task: true,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        // Multiple system messages at the start
        conversation.add_system_message("System prompt 1".to_string());
        conversation.add_system_message("System prompt 2".to_string());
        conversation.add_system_message("System prompt 3".to_string());

        // First user message (initial task) - should be preserved
        conversation.add_user_message("Build a web server".to_string());

        // Add more messages
        for i in 4..24 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(conversation.messages.len(), 10);

        // All system messages should be preserved
        assert_eq!(conversation.messages[0].role, "system");
        assert!(
            conversation.messages[0]
                .content
                .as_ref()
                .unwrap()
                .contains("System prompt 1")
        );
        assert_eq!(conversation.messages[1].role, "system");
        assert!(
            conversation.messages[1]
                .content
                .as_ref()
                .unwrap()
                .contains("System prompt 2")
        );
        assert_eq!(conversation.messages[2].role, "system");
        assert!(
            conversation.messages[2]
                .content
                .as_ref()
                .unwrap()
                .contains("System prompt 3")
        );

        // First user message should be preserved despite being at index 3
        assert_eq!(conversation.messages[3].role, "user");
        assert!(
            conversation.messages[3]
                .content
                .as_ref()
                .unwrap()
                .contains("Build a web server")
        );

        // We have 4 preserved messages (3 system + 1 initial user)
        // Window size is 10, so we keep 6 more recent messages
        // Total messages before windowing: 24 (3 system + 1 initial + 20 regular)
        // We keep the last 6 of the 20 regular messages: msg-18 through msg-23
        assert!(
            conversation.messages[4]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-18")
        );
        assert!(
            conversation.messages[9]
                .content
                .as_ref()
                .unwrap()
                .contains("msg-23")
        );
    }

    #[tokio::test]
    async fn test_no_user_messages_only_system() {
        let config = SlidingWindowConfig {
            window_size: 5,
            min_messages_before_windowing: 3,
            preserve_system: true,
            preserve_initial_task: true,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        // Only system messages, no user messages
        for i in 0..10 {
            conversation.add_system_message(format!("system-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        // When preserve_system is true, all system messages are kept even if they exceed window_size
        // This is the current behavior - preserved messages take priority
        assert_eq!(conversation.messages.len(), 10);

        // All should be system messages
        for msg in &conversation.messages {
            assert_eq!(msg.role, "system");
        }
    }

    #[tokio::test]
    async fn test_preserves_tool_call_pairs_basic() {
        let config = SlidingWindowConfig {
            window_size: 5,
            min_messages_before_windowing: 3,
            preserve_system: false,
            preserve_initial_task: false,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        for i in 0..10 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        conversation.add_user_message("read file".to_string());
        conversation.add_assistant_message(
            None,
            Some(vec![crate::agent::ToolCall {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: crate::agent::ToolFunction {
                    name: "read_file".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
        );
        conversation
            .messages
            .push(crate::agent::ConversationMessage {
                role: "tool".to_string(),
                content: Some("file contents".to_string()),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
                name: Some("read_file".to_string()),
            });
        conversation.add_assistant_message(Some("done".to_string()), None);

        strategy.apply(&mut conversation).await.unwrap();

        let has_tool_call = conversation
            .messages
            .iter()
            .any(|m| m.role == "assistant" && m.tool_calls.is_some());
        let has_tool_result = conversation
            .messages
            .iter()
            .any(|m| m.role == "tool" && m.tool_call_id == Some("call_1".to_string()));

        assert_eq!(has_tool_call, has_tool_result);
    }

    #[tokio::test]
    async fn test_preserves_tool_call_pairs_multiple_calls() {
        let config = SlidingWindowConfig {
            window_size: 8,
            min_messages_before_windowing: 3,
            preserve_system: false,
            preserve_initial_task: false,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        for i in 0..10 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        conversation.add_user_message("read files".to_string());
        conversation.add_assistant_message(
            None,
            Some(vec![
                crate::agent::ToolCall {
                    id: "call_1".to_string(),
                    r#type: "function".to_string(),
                    function: crate::agent::ToolFunction {
                        name: "read_file".to_string(),
                        arguments: r#"{"path":"a.txt"}"#.to_string(),
                    },
                },
                crate::agent::ToolCall {
                    id: "call_2".to_string(),
                    r#type: "function".to_string(),
                    function: crate::agent::ToolFunction {
                        name: "read_file".to_string(),
                        arguments: r#"{"path":"b.txt"}"#.to_string(),
                    },
                },
            ]),
        );
        conversation
            .messages
            .push(crate::agent::ConversationMessage {
                role: "tool".to_string(),
                content: Some("contents a".to_string()),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
                name: Some("read_file".to_string()),
            });
        conversation
            .messages
            .push(crate::agent::ConversationMessage {
                role: "tool".to_string(),
                content: Some("contents b".to_string()),
                tool_calls: None,
                tool_call_id: Some("call_2".to_string()),
                name: Some("read_file".to_string()),
            });
        conversation.add_assistant_message(Some("done".to_string()), None);

        strategy.apply(&mut conversation).await.unwrap();

        let assistant_with_calls = conversation
            .messages
            .iter()
            .find(|m| m.role == "assistant" && m.tool_calls.is_some());

        if let Some(msg) = assistant_with_calls {
            let tool_calls = msg.tool_calls.as_ref().unwrap();
            for tool_call in tool_calls {
                let has_result = conversation
                    .messages
                    .iter()
                    .any(|m| m.role == "tool" && m.tool_call_id.as_ref() == Some(&tool_call.id));
                assert!(
                    has_result,
                    "Tool call {} must have corresponding result",
                    tool_call.id
                );
            }
        }
    }

    #[tokio::test]
    async fn test_removes_complete_tool_pairs_together() {
        let config = SlidingWindowConfig {
            window_size: 5,
            min_messages_before_windowing: 3,
            preserve_system: false,
            preserve_initial_task: false,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        conversation.add_user_message("old request".to_string());
        conversation.add_assistant_message(
            None,
            Some(vec![crate::agent::ToolCall {
                id: "old_call".to_string(),
                r#type: "function".to_string(),
                function: crate::agent::ToolFunction {
                    name: "old_tool".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
        );
        conversation
            .messages
            .push(crate::agent::ConversationMessage {
                role: "tool".to_string(),
                content: Some("old result".to_string()),
                tool_calls: None,
                tool_call_id: Some("old_call".to_string()),
                name: Some("old_tool".to_string()),
            });
        conversation.add_assistant_message(Some("old done".to_string()), None);

        for i in 0..10 {
            conversation.add_user_message(format!("new-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        let has_old_call = conversation
            .messages
            .iter()
            .any(|m| m.tool_call_id.as_ref() == Some(&"old_call".to_string()));

        assert!(!has_old_call);
    }

    #[tokio::test]
    async fn test_no_user_messages_system_not_preserved() {
        let config = SlidingWindowConfig {
            window_size: 5,
            min_messages_before_windowing: 3,
            preserve_system: false,
            preserve_initial_task: true,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        // Only system messages, no user messages
        for i in 0..10 {
            conversation.add_system_message(format!("system-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        // When preserve_system is false, should keep only the most recent 5
        assert_eq!(conversation.messages.len(), 5);

        // All should be system messages
        for msg in &conversation.messages {
            assert_eq!(msg.role, "system");
        }

        // Should have the last 5 system messages
        assert!(
            conversation.messages[0]
                .content
                .as_ref()
                .unwrap()
                .contains("system-5")
        );
        assert!(
            conversation.messages[4]
                .content
                .as_ref()
                .unwrap()
                .contains("system-9")
        );
    }
}
