use anyhow::Result;
use async_trait::async_trait;

use crate::agent::{Conversation, ConversationMessage};
use crate::context_management::{ContextManagementStrategy, ToolOutputTruncationConfig};

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
        // Find a valid UTF-8 boundary
        let mut truncate_at = self.config.max_length.min(content.len());
        while truncate_at > 0 && !content.is_char_boundary(truncate_at) {
            truncate_at -= 1;
        }

        let truncated = &content[..truncate_at];

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

        // Find valid UTF-8 boundary for head
        let mut head_len = self.config.head_length.min(content.len());
        while head_len > 0 && !content.is_char_boundary(head_len) {
            head_len -= 1;
        }

        // Find valid UTF-8 boundary for tail
        let mut tail_start = content.len().saturating_sub(self.config.tail_length);
        while tail_start < content.len() && !content.is_char_boundary(tail_start) {
            tail_start += 1;
        }

        let head = &content[..head_len];
        let tail = &content[tail_start..];

        if self.config.show_truncation_notice {
            format!(
                "{}\n\n[... truncated {} characters ...]\n\n{}",
                head,
                content.len() - head_len - (content.len() - tail_start),
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

    /// Recursively truncates all string values in a JSON value that exceed max_length.
    /// Returns true if any modifications were made.
    fn truncate_json_strings(&self, value: &mut serde_json::Value) -> bool {
        match value {
            serde_json::Value::String(s) => {
                if s.len() > self.config.max_length {
                    *s = self.truncate_content(s);
                    true
                } else {
                    false
                }
            }
            serde_json::Value::Object(map) => {
                let mut modified = false;
                for (_key, val) in map.iter_mut() {
                    if self.truncate_json_strings(val) {
                        modified = true;
                    }
                }
                modified
            }
            serde_json::Value::Array(arr) => {
                let mut modified = false;
                for item in arr.iter_mut() {
                    if self.truncate_json_strings(item) {
                        modified = true;
                    }
                }
                modified
            }
            _ => false,
        }
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

            if self.is_tool_result(message)
                && let Some(content) = &message.content
            {
                let original_len = content.len();

                if original_len > self.config.max_length {
                    message.content = Some(self.truncate_content(content));
                }
            }

            if self.is_assistant_with_tools(message)
                && let Some(ref mut tool_calls) = message.tool_calls
            {
                for tool_call in tool_calls.iter_mut() {
                    let args_str = &tool_call.function.arguments;

                    if args_str.len() > self.config.max_length
                        && let Ok(mut args_json) =
                            serde_json::from_str::<serde_json::Value>(args_str)
                    {
                        let modified = self.truncate_json_strings(&mut args_json);

                        if modified {
                            tool_call.function.arguments = serde_json::to_string(&args_json)
                                .unwrap_or_else(|_| args_str.clone());
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
    use crate::agent::{ToolCall, ToolCallResponse, ToolFunction};

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

        assert!(
            conversation.messages[0]
                .content
                .as_ref()
                .unwrap()
                .contains("truncated")
        );
        assert!(conversation.messages[0].content.as_ref().unwrap().len() < 100);

        assert_eq!(
            conversation.messages[1].content.as_ref().unwrap(),
            &"B".repeat(100)
        );
        assert!(
            !conversation.messages[1]
                .content
                .as_ref()
                .unwrap()
                .contains("truncated")
        );
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

    #[tokio::test]
    async fn test_unicode_safe_simple_truncate() {
        let config = ToolOutputTruncationConfig {
            max_length: 7,
            show_truncation_notice: false,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let max_length = config.max_length;
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        // "Hello 世界!" - "世" starts at byte 6 and is 3 bytes
        // Truncating at byte 7 would be in the middle of "世"
        // Should truncate at byte 6 instead
        let content = "Hello 世界!";
        let result = ToolCallResponse::success(
            "tool_1".to_string(),
            "read_file".to_string(),
            "Read(file.txt)".to_string(),
            content.to_string(),
        );
        conversation.add_tool_result(result);

        let short_result = ToolCallResponse::success(
            "tool_2".to_string(),
            "read_file".to_string(),
            "Read(file2.txt)".to_string(),
            "Short".to_string(),
        );
        conversation.add_tool_result(short_result);

        strategy.apply(&mut conversation).await.unwrap();

        let truncated = conversation.messages[0].content.as_ref().unwrap();
        // Should not panic and should be valid UTF-8
        assert!(truncated.len() <= max_length);
        assert!(truncated.is_char_boundary(truncated.len()));
        assert_eq!(truncated, "Hello ");
    }

    #[tokio::test]
    async fn test_unicode_safe_smart_truncate() {
        let config = ToolOutputTruncationConfig {
            max_length: 100,
            show_truncation_notice: true,
            smart_truncate: true,
            head_length: 8,
            tail_length: 5,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        // Create content long enough to trigger smart truncation
        // head_length + tail_length = 13, so we need content > 13 bytes
        // And longer than max_length to trigger truncation
        let content = "Hello 世界 this is some middle content that is very long and should be truncated 🌍🎉!".repeat(3);
        let result = ToolCallResponse::success(
            "tool_1".to_string(),
            "read_file".to_string(),
            "Read(file.txt)".to_string(),
            content.to_string(),
        );
        conversation.add_tool_result(result);

        let short_result = ToolCallResponse::success(
            "tool_2".to_string(),
            "read_file".to_string(),
            "Read(file2.txt)".to_string(),
            "Short".to_string(),
        );
        conversation.add_tool_result(short_result);

        strategy.apply(&mut conversation).await.unwrap();

        let truncated = conversation.messages[0].content.as_ref().unwrap();
        // Should not panic and result should be valid UTF-8
        assert!(truncated.contains("[... truncated"));
        // Verify all parts are valid UTF-8 by checking char boundaries
        for (i, _) in truncated.char_indices() {
            assert!(truncated.is_char_boundary(i));
        }
    }

    #[tokio::test]
    async fn test_unicode_safe_with_emojis() {
        let config = ToolOutputTruncationConfig {
            max_length: 15,
            show_truncation_notice: false,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let max_length = config.max_length;
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        // "Test 🌍🎉 emoji" - emojis are 4 bytes each
        let content = "Test 🌍🎉 emoji";
        let result = ToolCallResponse::success(
            "tool_1".to_string(),
            "read_file".to_string(),
            "Read(file.txt)".to_string(),
            content.to_string(),
        );
        conversation.add_tool_result(result);

        let short_result = ToolCallResponse::success(
            "tool_2".to_string(),
            "read_file".to_string(),
            "Read(file2.txt)".to_string(),
            "Short".to_string(),
        );
        conversation.add_tool_result(short_result);

        strategy.apply(&mut conversation).await.unwrap();

        let truncated = conversation.messages[0].content.as_ref().unwrap();
        // Should not panic and should be valid UTF-8
        assert!(truncated.len() <= max_length);
        assert!(truncated.is_char_boundary(truncated.len()));
        // Verify the entire string is valid UTF-8
        for (i, _) in truncated.char_indices() {
            assert!(truncated.is_char_boundary(i));
        }
    }

    #[tokio::test]
    async fn test_unicode_safe_tool_call_arguments() {
        let config = ToolOutputTruncationConfig {
            max_length: 25,
            show_truncation_notice: false,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        let large_content = "Hello 世界! ".repeat(10);
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

        // Should not panic - verify the tool call arguments are still valid JSON
        if let Some(tool_calls) = &conversation.messages[0].tool_calls {
            let args_str = &tool_calls[0].function.arguments;
            // Should be valid JSON
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(args_str);
            assert!(parsed.is_ok());
        }
    }

    #[tokio::test]
    async fn test_truncates_all_string_fields() {
        let config = ToolOutputTruncationConfig {
            max_length: 50,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        let large_string = "x".repeat(200);
        let args = serde_json::json!({
            "path": large_string.clone(),
            "content": large_string.clone(),
            "command": large_string.clone(),
            "custom_field": large_string.clone(),
            "number": 42,
            "boolean": true,
        });

        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "some_tool".to_string(),
                arguments: serde_json::to_string(&args).unwrap(),
            },
        }];

        conversation.add_assistant_message(Some("Executing".to_string()), Some(tool_calls));
        conversation.add_user_message("done".to_string());

        strategy.apply(&mut conversation).await.unwrap();

        let tool_calls = conversation.messages[0].tool_calls.as_ref().unwrap();
        let args_str = &tool_calls[0].function.arguments;
        let args_json: serde_json::Value = serde_json::from_str(args_str).unwrap();

        // All string fields should be truncated
        let path = args_json["path"].as_str().unwrap();
        assert!(path.contains("truncated"));
        assert!(path.len() < large_string.len());

        let content = args_json["content"].as_str().unwrap();
        assert!(content.contains("truncated"));
        assert!(content.len() < large_string.len());

        let command = args_json["command"].as_str().unwrap();
        assert!(command.contains("truncated"));
        assert!(command.len() < large_string.len());

        let custom_field = args_json["custom_field"].as_str().unwrap();
        assert!(custom_field.contains("truncated"));
        assert!(custom_field.len() < large_string.len());

        // Non-string fields should remain unchanged
        assert_eq!(args_json["number"].as_i64().unwrap(), 42);
        assert!(args_json["boolean"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_truncates_nested_json_fields() {
        let config = ToolOutputTruncationConfig {
            max_length: 50,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        let large_string = "y".repeat(200);
        let args = serde_json::json!({
            "outer": {
                "inner": {
                    "deep_field": large_string.clone(),
                    "number": 123,
                },
                "another_field": large_string.clone(),
            },
            "top_level": large_string.clone(),
        });

        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "nested_tool".to_string(),
                arguments: serde_json::to_string(&args).unwrap(),
            },
        }];

        conversation.add_assistant_message(Some("Executing".to_string()), Some(tool_calls));
        conversation.add_user_message("done".to_string());

        strategy.apply(&mut conversation).await.unwrap();

        let tool_calls = conversation.messages[0].tool_calls.as_ref().unwrap();
        let args_str = &tool_calls[0].function.arguments;
        let args_json: serde_json::Value = serde_json::from_str(args_str).unwrap();

        // Nested string fields should be truncated
        let deep_field = args_json["outer"]["inner"]["deep_field"].as_str().unwrap();
        assert!(deep_field.contains("truncated"));
        assert!(deep_field.len() < large_string.len());

        let another_field = args_json["outer"]["another_field"].as_str().unwrap();
        assert!(another_field.contains("truncated"));
        assert!(another_field.len() < large_string.len());

        let top_level = args_json["top_level"].as_str().unwrap();
        assert!(top_level.contains("truncated"));
        assert!(top_level.len() < large_string.len());

        // Non-string fields should remain unchanged
        assert_eq!(args_json["outer"]["inner"]["number"].as_i64().unwrap(), 123);
    }

    #[tokio::test]
    async fn test_truncates_array_fields() {
        let config = ToolOutputTruncationConfig {
            max_length: 50,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        let large_string = "z".repeat(200);
        let args = serde_json::json!({
            "items": [
                large_string.clone(),
                "short",
                large_string.clone(),
            ],
            "nested_array": [
                {
                    "field": large_string.clone(),
                },
                {
                    "field": "short",
                }
            ],
        });

        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "array_tool".to_string(),
                arguments: serde_json::to_string(&args).unwrap(),
            },
        }];

        conversation.add_assistant_message(Some("Executing".to_string()), Some(tool_calls));
        conversation.add_user_message("done".to_string());

        strategy.apply(&mut conversation).await.unwrap();

        let tool_calls = conversation.messages[0].tool_calls.as_ref().unwrap();
        let args_str = &tool_calls[0].function.arguments;
        let args_json: serde_json::Value = serde_json::from_str(args_str).unwrap();

        // Array string elements should be truncated
        let item0 = args_json["items"][0].as_str().unwrap();
        assert!(item0.contains("truncated"));
        assert!(item0.len() < large_string.len());

        let item1 = args_json["items"][1].as_str().unwrap();
        assert_eq!(item1, "short");

        let item2 = args_json["items"][2].as_str().unwrap();
        assert!(item2.contains("truncated"));
        assert!(item2.len() < large_string.len());

        // Nested array objects should have their strings truncated
        let nested0 = args_json["nested_array"][0]["field"].as_str().unwrap();
        assert!(nested0.contains("truncated"));
        assert!(nested0.len() < large_string.len());

        let nested1 = args_json["nested_array"][1]["field"].as_str().unwrap();
        assert_eq!(nested1, "short");
    }

    #[tokio::test]
    async fn test_does_not_truncate_short_arguments() {
        let config = ToolOutputTruncationConfig {
            max_length: 1000,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
        };
        let strategy = ToolOutputTruncationStrategy::new(config);

        let mut conversation = Conversation::new();

        let args = serde_json::json!({
            "path": "test.txt",
            "content": "Short content",
            "command": "ls -la",
        });

        let original_args_str = serde_json::to_string(&args).unwrap();

        let tool_calls = vec![ToolCall {
            id: "call_1".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "some_tool".to_string(),
                arguments: original_args_str.clone(),
            },
        }];

        conversation.add_assistant_message(Some("Executing".to_string()), Some(tool_calls));
        conversation.add_user_message("done".to_string());

        strategy.apply(&mut conversation).await.unwrap();

        let tool_calls = conversation.messages[0].tool_calls.as_ref().unwrap();
        let args_str = &tool_calls[0].function.arguments;

        // Arguments should remain unchanged since they're short
        assert_eq!(args_str, &original_args_str);
    }
}
