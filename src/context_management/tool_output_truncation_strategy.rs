use anyhow::Result;
use async_trait::async_trait;

use crate::context_management::{ContextManagementStrategy, ToolOutputTruncationConfig};
use crate::conversations::{Conversation, ConversationMessage};

pub struct ToolOutputTruncationStrategy {
    config: ToolOutputTruncationConfig,
}

impl ToolOutputTruncationStrategy {
    pub fn new(config: ToolOutputTruncationConfig) -> Self {
        Self { config }
    }

    fn truncate_content(&self, content: &str) -> String {
        if content.len() <= self.config.max_length {
            return content.to_string();
        }

        if self.config.smart_truncate {
            self.smart_truncate(content)
        } else {
            self.simple_truncate(content)
        }
    }

    fn simple_truncate(&self, content: &str) -> String {
        let truncated = &content[..self.config.max_length.min(content.len())];

        if self.config.show_truncation_notice {
            format!(
                "{}\n\n[... truncated {} characters ...]",
                truncated,
                content.len() - truncated.len()
            )
        } else {
            truncated.to_string()
        }
    }

    fn smart_truncate(&self, content: &str) -> String {
        let total_keep = self.config.head_length + self.config.tail_length;

        if total_keep >= content.len() {
            return content.to_string();
        }

        let head = &content[..self.config.head_length.min(content.len())];
        let tail_start = content.len().saturating_sub(self.config.tail_length);
        let tail = &content[tail_start..];

        if self.config.show_truncation_notice {
            format!(
                "{}\n\n[... truncated {} characters ...]\n\n{}",
                head,
                content.len() - total_keep,
                tail
            )
        } else {
            format!("{}{}", head, tail)
        }
    }

    fn is_tool_result(&self, message: &ConversationMessage) -> bool {
        message.role == "tool" && message.tool_call_id.is_some()
    }

    fn is_assistant_with_tools(&self, message: &ConversationMessage) -> bool {
        message.role == "assistant" && message.tool_calls.is_some()
    }
}

#[async_trait]
impl ContextManagementStrategy for ToolOutputTruncationStrategy {
    async fn apply(&self, conversation: &mut Conversation) -> Result<()> {
        let message_count = conversation.messages.len();

        if message_count < 2 {
            return Ok(());
        }

        let last_tool_result_index = conversation
            .messages
            .iter()
            .enumerate()
            .rev()
            .find(|(_, msg)| self.is_tool_result(msg))
            .map(|(i, _)| i);

        for i in 0..message_count {
            if Some(i) == last_tool_result_index {
                continue;
            }

            let message = &mut conversation.messages[i];

            if self.is_tool_result(message) {
                if let Some(content) = &message.content {
                    let original_len = content.len();

                    if original_len > self.config.max_length {
                        message.content = Some(self.truncate_content(content));
                    }
                }
            }

            if self.is_assistant_with_tools(message) {
                if let Some(ref mut tool_calls) = message.tool_calls {
                    for tool_call in tool_calls.iter_mut() {
                        let args_str = &tool_call.function.arguments;

                        if args_str.len() > self.config.max_length {
                            if let Ok(mut args_json) = serde_json::from_str::<serde_json::Value>(args_str) {
                                let mut modified = false;

                                if let Some(obj) = args_json.as_object_mut() {
                                    if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
                                        if content.len() > self.config.max_length {
                                            let truncated = self.truncate_content(content);
                                            obj.insert("content".to_string(), serde_json::Value::String(truncated));
                                            modified = true;
                                        }
                                    }

                                    if let Some(command) = obj.get("command").and_then(|v| v.as_str()) {
                                        if command.len() > self.config.max_length {
                                            let truncated = self.truncate_content(command);
                                            obj.insert("command".to_string(), serde_json::Value::String(truncated));
                                            modified = true;
                                        }
                                    }
                                }

                                if modified {
                                    tool_call.function.arguments = serde_json::to_string(&args_json).unwrap_or_else(|_| args_str.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversations::{ToolCall, ToolCallResponse, ToolFunction};

    #[tokio::test]
    async fn test_keeps_last_tool_result_full() {
        let config = ToolOutputTruncationConfig {
            max_length: 20,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        let old_result = ToolCallResponse::success(
            "tool_1".to_string(),
            "read_file".to_string(),
            "Read(file.txt)".to_string(),
            "A".repeat(100),
        );
        conversation.add_tool_result(old_result);

        let recent_result = ToolCallResponse::success(
            "tool_2".to_string(),
            "read_file".to_string(),
            "Read(file2.txt)".to_string(),
            "B".repeat(100),
        );
        conversation.add_tool_result(recent_result);

        conversation.add_user_message("next".to_string());

        strategy.apply(&mut conversation).await.unwrap();

        assert!(conversation.messages[0]
            .content
            .as_ref()
            .unwrap()
            .contains("truncated"));
        assert!(conversation.messages[0].content.as_ref().unwrap().len() < 100);

        assert_eq!(
            conversation.messages[1].content.as_ref().unwrap(),
            &"B".repeat(100)
        );
        assert!(!conversation.messages[1]
            .content
            .as_ref()
            .unwrap()
            .contains("truncated"));
    }

    #[tokio::test]
    async fn test_truncates_tool_call_arguments() {
        let config = ToolOutputTruncationConfig {
            max_length: 50,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        let large_content = "x".repeat(200);
        let args = serde_json::json!({
            "path": "test.txt",
            "content": large_content,
        });

        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "write_file".to_string(),
                arguments: serde_json::to_string(&args).unwrap(),
            },
        }];

        conversation.add_assistant_message(Some("Writing file".to_string()), Some(tool_calls));

        conversation.add_user_message("done".to_string());

        conversation.add_assistant_message(Some("ok".to_string()), None);

        strategy.apply(&mut conversation).await.unwrap();

        let tool_calls = conversation.messages[0].tool_calls.as_ref().unwrap();
        let args_str = &tool_calls[0].function.arguments;
        let args_json: serde_json::Value = serde_json::from_str(args_str).unwrap();
        let content = args_json["content"].as_str().unwrap();

        assert!(content.contains("truncated"));
        assert!(content.len() < large_content.len());
    }

    #[tokio::test]
    async fn test_ignores_non_tool_messages() {
        let config = ToolOutputTruncationConfig {
            max_length: 20,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();
        let long_content = "A".repeat(100);

        conversation.add_user_message(long_content.clone());
        conversation.add_assistant_message(Some("ok".to_string()), None);

        strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(
            conversation.messages[0].content.as_ref().unwrap(),
            &long_content
        );
    }

    #[tokio::test]
    async fn test_smart_truncate_mode() {
        let config = ToolOutputTruncationConfig {
            max_length: 100,
            show_truncation_notice: true,
            smart_truncate: true,
            head_length: 30,
            tail_length: 20,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        let content = "A".repeat(30) + &"B".repeat(100) + &"C".repeat(20);
        let result = ToolCallResponse::success(
            "tool_1".to_string(),
            "read_file".to_string(),
            "Read(file.txt)".to_string(),
            content.clone(),
        );
        conversation.add_tool_result(result);

        let short_result = ToolCallResponse::success(
            "tool_2".to_string(),
            "read_file".to_string(),
            "Read(file2.txt)".to_string(),
            "Short content".to_string(),
        );
        conversation.add_tool_result(short_result);

        strategy.apply(&mut conversation).await.unwrap();

        let truncated = conversation.messages[0].content.as_ref().unwrap();

        assert!(truncated.starts_with(&"A".repeat(30)));
        assert!(truncated.ends_with(&"C".repeat(20)));
        assert!(truncated.contains("truncated"));
        assert!(truncated.len() < content.len());
    }

    #[tokio::test]
    async fn test_no_truncation_for_short_content() {
        let config = ToolOutputTruncationConfig::default();
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        let short_content = "Short output";
        let result = ToolCallResponse::success(
            "tool_1".to_string(),
            "read_file".to_string(),
            "Read(file.txt)".to_string(),
            short_content.to_string(),
        );
        conversation.add_tool_result(result);

        conversation.add_user_message("next".to_string());

        strategy.apply(&mut conversation).await.unwrap();

        assert_eq!(
            conversation.messages[0].content.as_ref().unwrap(),
            short_content
        );
    }
}
