use serde::{Deserialize, Serialize};

use crate::tools::error::ToolError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String, // Always "function"
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

#[derive(Debug)]
pub struct ToolCallResponse {
    pub tool_call_id: String,
    pub tool_name: String,
    pub display_name: String,
    pub result: Result<String, ToolError>,
}

impl ToolCallResponse {
    pub fn success(
        tool_call_id: String,
        tool_name: String,
        display_name: String,
        output: String,
    ) -> Self {
        Self {
            tool_call_id,
            tool_name,
            display_name,
            result: Ok(output),
        }
    }

    pub fn error(
        tool_call_id: String,
        tool_name: String,
        display_name: String,
        error: ToolError,
    ) -> Self {
        Self {
            tool_call_id,
            tool_name,
            display_name,
            result: Err(error),
        }
    }

    pub fn is_rejected(&self) -> bool {
        if let Err(e) = &self.result {
            e.is_user_rejection()
        } else {
            false
        }
    }

    pub fn to_message(&self) -> ConversationMessage {
        let content = match &self.result {
            Ok(output) => output.clone(),
            Err(error) => format!("Error: {}", error),
        };

        ConversationMessage {
            role: "tool".to_string(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: Some(self.tool_call_id.clone()),
            name: Some(self.tool_name.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Conversation {
    pub messages: Vec<ConversationMessage>,
}

impl Conversation {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn add_system_message(&mut self, content: String) {
        self.messages.push(ConversationMessage {
            role: "system".to_string(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
    }

    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(ConversationMessage {
            role: "user".to_string(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
    }

    pub fn add_assistant_message(
        &mut self,
        content: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
    ) {
        self.messages.push(ConversationMessage {
            role: "assistant".to_string(),
            content,
            tool_calls,
            tool_call_id: None,
            name: None,
        });
    }

    pub fn add_tool_result(&mut self, tool_result: ToolCallResponse) {
        self.messages.push(tool_result.to_message());
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn get_messages_for_api(&self) -> &Vec<ConversationMessage> {
        &self.messages
    }

    pub fn has_pending_tool_calls(&self) -> bool {
        if let Some(last_message) = self.messages.last()
            && last_message.role == "assistant"
        {
            return last_message.tool_calls.is_some();
        }
        false
    }

    pub fn get_pending_tool_calls(&self) -> Option<&Vec<ToolCall>> {
        if let Some(last_message) = self.messages.last()
            && last_message.role == "assistant"
        {
            return last_message.tool_calls.as_ref();
        }
        None
    }

    /// Compact the conversation by replacing old messages with a summary
    /// while preserving recent messages and system context
    pub fn compact_with_summary(&mut self, summary: String, keep_recent: usize) {
        let total = self.messages.len();

        if total <= keep_recent {
            return; // Nothing to compact
        }

        // Keep system message if present
        let system_msg = self.messages.iter().find(|m| m.role == "system").cloned();

        // Get recent messages
        let recent: Vec<_> = self
            .messages
            .iter()
            .skip(total - keep_recent)
            .cloned()
            .collect();

        // Create summary message
        let summary_msg = ConversationMessage {
            role: "user".to_string(),
            content: Some(format!(
                "[CONVERSATION HISTORY SUMMARY - {} messages]\n\n{}\n\n[END SUMMARY - Recent conversation continues below]",
                total - keep_recent,
                summary
            )),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        // Rebuild messages: system + summary + recent
        self.messages.clear();
        if let Some(sys) = system_msg {
            self.messages.push(sys);
        }
        self.messages.push(summary_msg);
        self.messages.extend(recent);
    }

    /// Check if the conversation has been compacted
    pub fn is_compacted(&self) -> bool {
        self.messages.iter().any(|m| {
            if let Some(content) = &m.content {
                content.starts_with("[CONVERSATION HISTORY SUMMARY")
            } else {
                false
            }
        })
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ToolExecutionContext {
    pub conversation: Conversation,
    pub working_directory: std::path::PathBuf,
    pub allow_dangerous_commands: bool,
}

impl ToolExecutionContext {
    pub fn new(working_directory: std::path::PathBuf) -> Self {
        Self {
            conversation: Conversation::new(),
            working_directory,
            allow_dangerous_commands: false,
        }
    }

    pub fn with_dangerous_commands(mut self, allow: bool) -> Self {
        self.allow_dangerous_commands = allow;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_basic_flow() {
        let mut conversation = Conversation::new();

        // Add user message
        conversation.add_user_message("Hello".to_string());
        assert_eq!(conversation.messages.len(), 1);
        assert_eq!(conversation.messages[0].role, "user");
        assert_eq!(conversation.messages[0].content, Some("Hello".to_string()));

        // Add assistant response
        conversation.add_assistant_message(Some("Hi there!".to_string()), None);
        assert_eq!(conversation.messages.len(), 2);
        assert_eq!(conversation.messages[1].role, "assistant");
        assert_eq!(
            conversation.messages[1].content,
            Some("Hi there!".to_string())
        );
    }

    #[test]
    fn test_tool_call_flow() {
        let mut conversation = Conversation::new();

        // Add user message
        conversation.add_user_message("Read the file test.txt".to_string());

        // Add assistant response with tool call
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\": \"test.txt\"}".to_string(),
            },
        };

        conversation.add_assistant_message(None, Some(vec![tool_call]));

        // Check that we have pending tool calls
        assert!(conversation.has_pending_tool_calls());

        let pending_calls = conversation
            .get_pending_tool_calls()
            .expect("Should have pending tool calls");
        assert_eq!(pending_calls.len(), 1);
        assert_eq!(pending_calls[0].function.name, "read_file");

        // Add tool result
        let tool_result = ToolCallResponse::success(
            "call_123".to_string(),
            "read_file".to_string(),
            "Read(test.txt)".to_string(),
            "File contents here".to_string(),
        );
        conversation.add_tool_result(tool_result);

        assert_eq!(conversation.messages.len(), 3);
        assert_eq!(conversation.messages[2].role, "tool");
        assert_eq!(
            conversation.messages[2].content,
            Some("File contents here".to_string())
        );
        assert_eq!(
            conversation.messages[2].tool_call_id,
            Some("call_123".to_string())
        );
    }

    #[test]
    fn test_tool_result_error() {
        let error = ToolError::execution_failed("File not found");
        let tool_result = ToolCallResponse::error(
            "call_123".to_string(),
            "read_file".to_string(),
            "Read(test.txt)".to_string(),
            error,
        );

        let message = tool_result.to_message();
        assert_eq!(message.role, "tool");
        assert!(message.content.unwrap().starts_with("Error: "));
        assert_eq!(message.tool_call_id, Some("call_123".to_string()));
        assert_eq!(message.name, Some("read_file".to_string()));
    }
}
