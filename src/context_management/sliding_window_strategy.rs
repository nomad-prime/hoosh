use anyhow::Result;
use async_trait::async_trait;

use crate::agent::{Conversation, ConversationMessage};
use crate::context_management::{ContextManagementStrategy, SlidingWindowConfig, StrategyResult};

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
            // If it's an assistant message with tool calls, ensure its results are kept.
            if messages[i].role == "assistant" && messages[i].tool_calls.is_some() {
                self.mark_tool_results(i, messages, keep_flags);
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
                        // Assistant is found and should be kept.
                        if !keep_flags[j] {
                            keep_flags[j] = true;
                            // Crucial: Re-run the forward check for this newly-kept assistant.
                            self.mark_tool_results(j, messages, keep_flags);
                        }
                        break; // Stop searching backward once the parent is found
                    }
                }
            }
        }
    }

    fn mark_tool_results(
        &self,
        assistant_index: usize,
        messages: &[ConversationMessage],
        keep_flags: &mut [bool],
    ) {
        if let Some(tool_calls) = &messages[assistant_index].tool_calls {
            for tool_call in tool_calls {
                for k in (assistant_index + 1)..messages.len() {
                    if messages[k].role == "tool"
                        && messages[k].tool_call_id.as_ref() == Some(&tool_call.id)
                    {
                        keep_flags[k] = true;
                    }
                }
            }
        }
    }
}

#[async_trait]
impl ContextManagementStrategy for SlidingWindowStrategy {
    async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult> {
        let initial_count = conversation.messages.len();

        if initial_count <= self.config.min_messages_before_windowing {
            return Ok(StrategyResult::NoChange);
        }

        let total_to_keep = self.config.window_size;

        if initial_count <= total_to_keep {
            return Ok(StrategyResult::NoChange);
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
            if self.config.strict_window_size {
                // Strict mode: enforce window_size as hard limit
                // Keep the MOST RECENT total_to_keep preserved messages
                let preserved_indices: Vec<usize> = keep_flags
                    .iter()
                    .enumerate()
                    .filter(|(_, k)| **k)
                    .map(|(i, _)| i)
                    .collect();

                // Keep only the last total_to_keep indices
                let indices_to_keep: Vec<usize> = if preserved_indices.len() > total_to_keep {
                    preserved_indices
                        .iter()
                        .rev()
                        .take(total_to_keep)
                        .copied()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect()
                } else {
                    preserved_indices
                };

                // Reset keep_flags and set only the indices we want
                keep_flags.fill(false);
                for idx in indices_to_keep {
                    keep_flags[idx] = true;
                }
            }

            self.ensure_tool_call_pairs(&conversation.messages, &mut keep_flags);

            conversation.messages = conversation
                .messages
                .drain(..)
                .enumerate()
                .filter_map(|(i, msg)| if keep_flags[i] { Some(msg) } else { None })
                .collect();

            return Ok(StrategyResult::Applied);
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

        Ok(StrategyResult::Applied)
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
            strict_window_size: false,
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
            strict_window_size: false,
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
            strict_window_size: false,
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
    async fn test_sliding_window_integration() {
        use crate::context_management::{ContextManager, ContextManagerConfig, TokenAccountant};
        use std::sync::Arc;

        let config = ContextManagerConfig {
            sliding_window: Some(SlidingWindowConfig {
                window_size: 10,
                min_messages_before_windowing: 5,
                preserve_system: true,
                preserve_initial_task: true,
                strict_window_size: false,
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
            strict_window_size: false,
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
            strict_window_size: false,
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
            strict_window_size: false,
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
            strict_window_size: false,
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
            strict_window_size: false,
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
            strict_window_size: false,
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

    #[tokio::test]
    async fn test_non_strict_window_size_mode() {
        let config = SlidingWindowConfig {
            window_size: 10,
            min_messages_before_windowing: 5,
            preserve_system: true,
            preserve_initial_task: true,
            strict_window_size: false,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        // Add many system messages (simulating config with lots of system info)
        for i in 0..15 {
            conversation.add_system_message(format!("system-{}", i));
        }

        // Add user message
        conversation.add_user_message("user message".to_string());

        strategy.apply(&mut conversation).await.unwrap();

        // In legacy mode, with preserve_system=true, all 15 system messages should be kept
        // plus the user message = 16 total (can exceed window_size)
        assert_eq!(conversation.messages.len(), 16);
    }

    #[tokio::test]
    async fn test_strict_window_size_mode_enforced() {
        let config = SlidingWindowConfig {
            window_size: 10,
            min_messages_before_windowing: 5,
            preserve_system: true,
            preserve_initial_task: true,
            strict_window_size: true, // Strict mode
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        // Add many system messages (simulating config with lots of system info)
        for i in 0..15 {
            conversation.add_system_message(format!("system-{}", i));
        }

        // Add user message
        conversation.add_user_message("user message".to_string());

        strategy.apply(&mut conversation).await.unwrap();

        // In strict mode, window_size must be enforced as hard limit
        // Should keep ONLY the most recent 10 messages total
        assert_eq!(conversation.messages.len(), 10);

        // The most recent messages should be kept (system-5 through system-9 and user)
        // Then the most recent 10
        let last_system = conversation
            .messages
            .iter()
            .rfind(|msg| msg.role == "system")
            .unwrap()
            .content
            .as_ref()
            .unwrap();

        // Should have one of the higher-numbered system messages
        assert!(last_system.contains("system-") && !last_system.contains("system-0"));
    }

    #[tokio::test]
    async fn test_strict_window_size_with_mixed_messages() {
        let config = SlidingWindowConfig {
            window_size: 5,
            min_messages_before_windowing: 3,
            preserve_system: true,
            preserve_initial_task: true,
            strict_window_size: true,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        // Add system messages
        for i in 0..5 {
            conversation.add_system_message(format!("system-{}", i));
        }

        // Add first user message
        conversation.add_user_message("first user".to_string());

        // Add assistant messages
        for i in 0..5 {
            conversation.add_assistant_message(Some(format!("response-{}", i)), None);
        }

        // Add more user messages
        for i in 0..5 {
            conversation.add_user_message(format!("user-{}", i));
        }

        let initial_count = conversation.messages.len();
        assert!(initial_count > 5);

        strategy.apply(&mut conversation).await.unwrap();

        // With strict_window_size, should keep exactly 5 messages
        assert_eq!(conversation.messages.len(), 5);
    }

    /// Helper function to verify tool call/result balance
    fn verify_tool_balance(messages: &[ConversationMessage]) {
        // Collect all tool call IDs from assistant messages
        let mut tool_calls_seen = std::collections::HashSet::new();
        let mut tool_results_seen = std::collections::HashSet::new();

        for msg in messages {
            if msg.role == "assistant" {
                if let Some(tool_calls) = &msg.tool_calls {
                    for tc in tool_calls {
                        tool_calls_seen.insert(tc.id.clone());
                    }
                }
            } else if msg.role == "tool"
                && let Some(tool_call_id) = &msg.tool_call_id
            {
                tool_results_seen.insert(tool_call_id.clone());
            }
        }

        // Every tool call should have a result
        for call_id in &tool_calls_seen {
            assert!(
                tool_results_seen.contains(call_id),
                "Tool call {} has no corresponding result",
                call_id
            );
        }

        // Every tool result should have a call
        for result_id in &tool_results_seen {
            assert!(
                tool_calls_seen.contains(result_id),
                "Tool result {} has no corresponding call",
                result_id
            );
        }
    }

    #[tokio::test]
    async fn test_sliding_cuts_off_old_tool_calls_with_results() {
        let config = SlidingWindowConfig {
            window_size: 5,
            min_messages_before_windowing: 3,
            preserve_system: false,
            preserve_initial_task: false,
            strict_window_size: false,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        // Create old tool call that should be cut off
        conversation.add_user_message("old request".to_string());
        conversation.add_assistant_message(
            None,
            Some(vec![crate::agent::ToolCall {
                id: "old_call_1".to_string(),
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
                tool_call_id: Some("old_call_1".to_string()),
                name: Some("old_tool".to_string()),
            });

        // Add many recent messages
        for i in 0..15 {
            conversation.add_user_message(format!("recent-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        // Verify tool balance - old call should be completely removed
        verify_tool_balance(&conversation.messages);

        // Verify the old tool call was removed (no orphaned results)
        let has_old_call = conversation
            .messages
            .iter()
            .any(|m| m.tool_call_id.as_ref() == Some(&"old_call_1".to_string()));
        assert!(!has_old_call);
    }

    #[tokio::test]
    async fn test_sliding_preserves_complete_tool_pairs_when_keeping_call() {
        let config = SlidingWindowConfig {
            window_size: 8,
            min_messages_before_windowing: 3,
            preserve_system: false,
            preserve_initial_task: false,
            strict_window_size: false,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        // Add many old messages
        for i in 0..10 {
            conversation.add_user_message(format!("old-{}", i));
        }

        // Add tool call that should be kept (recent enough)
        conversation.add_user_message("request for tool".to_string());
        conversation.add_assistant_message(
            None,
            Some(vec![crate::agent::ToolCall {
                id: "kept_call".to_string(),
                r#type: "function".to_string(),
                function: crate::agent::ToolFunction {
                    name: "my_tool".to_string(),
                    arguments: r#"{"arg":"value"}"#.to_string(),
                },
            }]),
        );
        conversation
            .messages
            .push(crate::agent::ConversationMessage {
                role: "tool".to_string(),
                content: Some("tool result".to_string()),
                tool_calls: None,
                tool_call_id: Some("kept_call".to_string()),
                name: Some("my_tool".to_string()),
            });
        conversation.add_assistant_message(Some("processed".to_string()), None);

        strategy.apply(&mut conversation).await.unwrap();

        // Verify tool balance
        verify_tool_balance(&conversation.messages);

        // Verify the kept tool call has its result
        let has_call = conversation
            .messages
            .iter()
            .any(|m| m.role == "assistant" && m.tool_calls.is_some());
        let has_result = conversation
            .messages
            .iter()
            .any(|m| m.role == "tool" && m.tool_call_id == Some("kept_call".to_string()));

        if has_call {
            assert!(has_result, "If tool call is kept, result must be kept too");
        }
    }

    #[tokio::test]
    async fn test_sliding_multiple_tool_calls_sliding_cuts_tool_call() {
        let config = SlidingWindowConfig {
            window_size: 9,
            min_messages_before_windowing: 3,
            preserve_system: false,
            preserve_initial_task: false,
            strict_window_size: false,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        // Scenario: 3 rounds of tool calls, window cuts off middle one
        for round in 0..3 {
            conversation.add_user_message(format!("request-{}", round));
            conversation.add_assistant_message(
                None,
                Some(vec![crate::agent::ToolCall {
                    id: format!("call_{}", round),
                    r#type: "function".to_string(),
                    function: crate::agent::ToolFunction {
                        name: "operation".to_string(),
                        arguments: format!(r#"{{"index":{}}}"#, round),
                    },
                }]),
            );
            conversation
                .messages
                .push(crate::agent::ConversationMessage {
                    role: "tool".to_string(),
                    content: Some(format!("result-{}", round)),
                    tool_calls: None,
                    tool_call_id: Some(format!("call_{}", round)),
                    name: Some("operation".to_string()),
                });
            conversation.add_assistant_message(Some(format!("response-{}", round)), None);
        }

        // Add padding to trigger windowing
        for i in 0..7 {
            conversation.add_user_message(format!("padding-{}", i));
        }

        let initial_count = conversation.messages.len();
        strategy.apply(&mut conversation).await.unwrap();

        // Verify we actually did windowing
        assert!(conversation.messages.len() < initial_count);

        // Verify tool balance - no orphaned calls or results
        verify_tool_balance(&conversation.messages);

        // balancing should force the system for a larger windows
        assert_eq!(conversation.messages.len(), 10);
    }

    #[tokio::test]
    async fn test_sliding_strict_mode_maintains_tool_balance() {
        let config = SlidingWindowConfig {
            window_size: 8,
            min_messages_before_windowing: 3,
            preserve_system: true,
            preserve_initial_task: true,
            strict_window_size: true,
        };
        let strategy = SlidingWindowStrategy::new(config);

        let mut conversation = Conversation::new();

        conversation.add_system_message("system".to_string());
        conversation.add_user_message("initial task".to_string());

        // Add tool call rounds
        for round in 0..5 {
            conversation.add_user_message(format!("round-{}", round));
            conversation.add_assistant_message(
                None,
                Some(vec![crate::agent::ToolCall {
                    id: format!("strict_call_{}", round),
                    r#type: "function".to_string(),
                    function: crate::agent::ToolFunction {
                        name: "tool".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
            );
            conversation
                .messages
                .push(crate::agent::ConversationMessage {
                    role: "tool".to_string(),
                    content: Some(format!("result-{}", round)),
                    tool_calls: None,
                    tool_call_id: Some(format!("strict_call_{}", round)),
                    name: Some("tool".to_string()),
                });
        }

        strategy.apply(&mut conversation).await.unwrap();

        // Verify tool balance in strict mode
        verify_tool_balance(&conversation.messages);
    }
}
