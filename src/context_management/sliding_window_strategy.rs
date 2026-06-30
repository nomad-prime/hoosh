use anyhow::Result;
use async_trait::async_trait;

use crate::agent::{Conversation, ConversationMessage, Role};
use crate::context_management::{ContextManagementStrategy, SlidingWindowConfig, StrategyResult};

pub struct SlidingWindowStrategy {
    config: SlidingWindowConfig,
}

impl SlidingWindowStrategy {
    pub fn new(config: SlidingWindowConfig) -> Self {
        Self { config }
    }

    fn is_system_message(&self, message: &ConversationMessage) -> bool {
        message.role == Role::System
    }

    fn is_user_message(&self, message: &ConversationMessage) -> bool {
        message.role == Role::User
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
            if messages[i].role == Role::Assistant && messages[i].tool_calls.is_some() {
                self.mark_tool_results(i, messages, keep_flags);
            }
        }

        for i in 0..messages.len() {
            if !keep_flags[i] {
                continue;
            }

            if messages[i].role == Role::Tool
                && let Some(tool_call_id) = &messages[i].tool_call_id
            {
                for j in (0..i).rev() {
                    if messages[j].role == Role::Assistant
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
                    if messages[k].role == Role::Tool
                        && messages[k].tool_call_id.as_ref() == Some(&tool_call.id)
                    {
                        keep_flags[k] = true;
                    }
                }
            }
        }
    }

    fn apply_token_budget(
        &self,
        conversation: &mut Conversation,
        token_budget: usize,
    ) -> Result<StrategyResult> {
        let messages = &conversation.messages;

        let first_user_message_index = messages.iter().position(|msg| self.is_user_message(msg));

        let mut keep_flags: Vec<bool> = messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let is_first_user_message = first_user_message_index == Some(index);
                self.should_preserve(message, is_first_user_message)
            })
            .collect();

        let mut used_tokens: usize = keep_flags
            .iter()
            .enumerate()
            .filter(|(_, keep)| **keep)
            .map(|(i, _)| Conversation::estimate_message_tokens(&messages[i]))
            .sum();

        for i in (0..messages.len()).rev() {
            if keep_flags[i] {
                continue;
            }

            let cost = Conversation::estimate_message_tokens(&messages[i]);
            if used_tokens + cost > token_budget {
                break;
            }

            keep_flags[i] = true;
            used_tokens += cost;
        }

        self.ensure_tool_call_pairs(messages, &mut keep_flags);

        if keep_flags.iter().all(|&keep| keep) {
            return Ok(StrategyResult::NoChange);
        }

        conversation.messages = conversation
            .messages
            .drain(..)
            .enumerate()
            .filter_map(|(i, msg)| if keep_flags[i] { Some(msg) } else { None })
            .collect();

        Ok(StrategyResult::Applied)
    }
}

#[async_trait]
impl ContextManagementStrategy for SlidingWindowStrategy {
    async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult> {
        self.apply_token_budget(conversation, self.config.max_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(max_tokens: usize) -> SlidingWindowConfig {
        SlidingWindowConfig {
            preserve_system: false,
            preserve_initial_task: false,
            max_tokens,
        }
    }

    fn push_tool_round(conversation: &mut Conversation, round: usize) {
        conversation.add_assistant_message(
            None,
            Some(vec![crate::agent::ToolCall {
                id: format!("call_{}", round),
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
                role: Role::Tool,
                content: Some(format!("result-{}", round)),
                tool_calls: None,
                tool_call_id: Some(format!("call_{}", round)),
                name: Some("tool".to_string()),
                attachments: Vec::new(),
            });
    }

    fn verify_tool_balance(messages: &[ConversationMessage]) {
        let mut calls = std::collections::HashSet::new();
        let mut results = std::collections::HashSet::new();
        for msg in messages {
            if msg.role == Role::Assistant {
                if let Some(tool_calls) = &msg.tool_calls {
                    for tc in tool_calls {
                        calls.insert(tc.id.clone());
                    }
                }
            } else if msg.role == Role::Tool
                && let Some(id) = &msg.tool_call_id
            {
                results.insert(id.clone());
            }
        }
        assert_eq!(calls, results, "tool calls and results must be balanced");
    }

    #[tokio::test]
    async fn test_no_change_when_under_budget() {
        let strategy = SlidingWindowStrategy::new(config(100_000));
        let mut conversation = Conversation::new();
        for i in 0..10 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        let result = strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(result, StrategyResult::NoChange);
        assert_eq!(conversation.messages.len(), 10);
    }

    #[tokio::test]
    async fn test_trims_to_token_budget() {
        let strategy = SlidingWindowStrategy::new(config(40));
        let mut conversation = Conversation::new();
        for i in 0..30 {
            conversation.add_user_message(format!("message number {}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        assert!(conversation.messages.len() < 30);
        assert!(conversation.estimate_token() <= 40);
    }

    #[tokio::test]
    async fn test_keeps_most_recent_messages() {
        let strategy = SlidingWindowStrategy::new(config(40));
        let mut conversation = Conversation::new();
        for i in 0..30 {
            conversation.add_user_message(format!("msg-{}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        let last = conversation.messages.last().unwrap();
        assert_eq!(last.content.as_deref(), Some("msg-29"));
    }

    #[tokio::test]
    async fn test_preserves_system_and_initial_task() {
        let mut cfg = config(20);
        cfg.preserve_system = true;
        cfg.preserve_initial_task = true;
        let strategy = SlidingWindowStrategy::new(cfg);

        let mut conversation = Conversation::new();
        conversation.add_system_message("system prompt".to_string());
        conversation.add_user_message("the original task".to_string());
        for i in 0..30 {
            conversation.add_user_message(format!("filler message {}", i));
        }

        strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(conversation.messages[0].role, Role::System);
        assert_eq!(
            conversation.messages[1].content.as_deref(),
            Some("the original task")
        );
    }

    #[tokio::test]
    async fn test_maintains_tool_balance() {
        let mut cfg = config(60);
        cfg.preserve_system = true;
        cfg.preserve_initial_task = true;
        let strategy = SlidingWindowStrategy::new(cfg);

        let mut conversation = Conversation::new();
        conversation.add_system_message("system".to_string());
        conversation.add_user_message("initial task".to_string());
        for round in 0..6 {
            push_tool_round(&mut conversation, round);
        }

        strategy.apply(&mut conversation).await.unwrap();

        verify_tool_balance(&conversation.messages);
    }
}
