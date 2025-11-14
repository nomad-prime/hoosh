use crate::console;
use crate::storage::{ConversationMetadata, ConversationStorage};
use crate::tools::error::ToolError;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

    pub fn is_permission_denied(&self) -> bool {
        if let Err(e) = &self.result {
            e.is_permission_denied()
        } else {
            false
        }
    }

    pub fn to_message(&self) -> ConversationMessage {
        let content = match &self.result {
            Ok(output) => output.clone(),
            Err(error) => error.llm_message(),
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

pub struct Conversation {
    pub metadata: ConversationMetadata,
    pub messages: Vec<ConversationMessage>,
    storage: Option<Arc<ConversationStorage>>,
}

impl Conversation {
    /// Create a new in-memory conversation without persistence
    /// Useful for testing or temporary conversations
    pub fn new() -> Self {
        let id = format!(
            "temp_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        Self {
            metadata: ConversationMetadata::new(id),
            messages: Vec::new(),
            storage: None,
        }
    }

    /// Create a new conversation with persistent storage
    /// This will automatically persist all messages as they're added
    pub fn with_storage(id: String, storage: Arc<ConversationStorage>) -> Result<Self> {
        let metadata = storage.create_conversation(&id)?;
        Ok(Self {
            metadata,
            messages: Vec::new(),
            storage: Some(storage),
        })
    }

    /// Load an existing conversation from storage
    pub fn load(id: &str, storage: Arc<ConversationStorage>) -> Result<Self> {
        let metadata = storage.load_metadata(id)?;
        let messages = storage.load_messages(id)?;
        Ok(Self {
            metadata,
            messages,
            storage: Some(storage),
        })
    }

    /// Create an in-memory conversation with a specific ID
    /// Useful for testing with predictable IDs
    pub fn new_with_id(id: String) -> Self {
        Self {
            metadata: ConversationMetadata::new(id),
            messages: Vec::new(),
            storage: None,
        }
    }

    pub fn add_system_message(&mut self, content: String) {
        let message = ConversationMessage {
            role: "system".to_string(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        self.messages.push(message.clone());
        self.persist_message(&message);
    }

    pub fn add_user_message(&mut self, content: String) {
        let message = ConversationMessage {
            role: "user".to_string(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        self.messages.push(message.clone());
        self.persist_message(&message);
    }

    pub fn add_assistant_message(
        &mut self,
        content: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
    ) {
        let message = ConversationMessage {
            role: "assistant".to_string(),
            content,
            tool_calls,
            tool_call_id: None,
            name: None,
        };
        self.messages.push(message.clone());
        self.persist_message(&message);
    }

    pub fn add_tool_result(&mut self, tool_result: ToolCallResponse) {
        let message = tool_result.to_message();
        self.messages.push(message.clone());
        self.persist_message(&message);
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn get_messages_for_api(&self) -> &Vec<ConversationMessage> {
        &self.messages
    }

    pub fn set_title(&mut self, title: String) {
        self.metadata.title = title.clone();
        self.metadata.update();

        if let Some(storage) = &self.storage && let Err(e) = storage.save_metadata(&self.metadata) {
                console().error(&format!("Warning: Failed to persist title update: {}", e))
            }
    }

    pub fn id(&self) -> &str {
        &self.metadata.id
    }

    pub fn title(&self) -> &str {
        &self.metadata.title
    }

    fn persist_message(&mut self, message: &ConversationMessage) {
        if let Some(storage) = &self.storage {
            if let Err(e) = storage.append_message(&self.metadata.id, message) {
                eprintln!("Warning: Failed to persist message: {}", e);
            } else {
                self.metadata.message_count = self.messages.len();
                self.metadata.update();
            }
        }
    }

    pub fn has_pending_tool_calls(&self) -> bool {
        // Check last message first (assistant with tool_calls)
        if let Some(last_message) = self.messages.last()
            && last_message.role == "assistant"
            && last_message.tool_calls.is_some()
        {
            return true;
        }

        // Check second-to-last message (assistant with tool_calls, followed by user message)
        if self.messages.len() >= 2 {
            let last_is_user = self
                .messages
                .last()
                .map(|m| m.role == "user")
                .unwrap_or(false);

            if last_is_user {
                let second_to_last = &self.messages[self.messages.len() - 2];
                if second_to_last.role == "assistant" && second_to_last.tool_calls.is_some() {
                    return true;
                }
            }
        }

        false
    }

    pub fn get_pending_tool_calls(&self) -> Option<&Vec<ToolCall>> {
        // Check last message first (assistant with tool_calls)
        if let Some(last_message) = self.messages.last()
            && last_message.role == "assistant"
            && let Some(ref tool_calls) = last_message.tool_calls
        {
            return Some(tool_calls);
        }

        // Check second-to-last message (assistant with tool_calls, followed by user message)
        if self.messages.len() >= 2 {
            let last_is_user = self
                .messages
                .last()
                .map(|m| m.role == "user")
                .unwrap_or(false);

            if last_is_user {
                let second_to_last = &self.messages[self.messages.len() - 2];
                if second_to_last.role == "assistant" {
                    return second_to_last.tool_calls.as_ref();
                }
            }
        }

        None
    }

    /// Repairs the conversation by adding synthetic tool_results for pending tool_calls.
    /// This handles the case where the system crashed after persisting an assistant message
    /// with tool_calls but before persisting the tool_results.
    /// Returns true if any repair was performed.
    pub fn repair_incomplete_tool_calls(&mut self) -> bool {
        if !self.has_pending_tool_calls() {
            return false;
        }

        let tool_calls = self.get_pending_tool_calls().unwrap().clone();

        // Check if last message is a user message (interruption + continue scenario)
        let last_is_user = self
            .messages
            .last()
            .map(|m| m.role == "user")
            .unwrap_or(false);

        // If last is user, we need to insert tool results before it
        let user_message = if last_is_user {
            self.messages.pop()
        } else {
            None
        };

        // Add synthetic tool_results for each incomplete tool_call
        for tool_call in tool_calls {
            let synthetic_result = ConversationMessage {
                role: "tool".to_string(),
                content: Some(
                    "Error: Tool execution was interrupted. The previous session ended before this tool could complete. Please try again.".to_string()
                ),
                tool_calls: None,
                tool_call_id: Some(tool_call.id),
                name: None,
            };
            self.messages.push(synthetic_result.clone());
            self.persist_message(&synthetic_result);
        }

        // Put user message back if we removed it
        if let Some(user_msg) = user_message {
            self.messages.push(user_msg);
        }

        true
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

impl Clone for Conversation {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            messages: self.messages.clone(),
            storage: self.storage.clone(),
        }
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

    #[test]
    fn test_has_pending_tool_calls_when_last_message_is_assistant() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("Read test.txt".to_string());

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\": \"test.txt\"}".to_string(),
            },
        };

        conversation.add_assistant_message(None, Some(vec![tool_call]));

        // Last message is assistant with tool_calls
        assert!(conversation.has_pending_tool_calls());
        assert!(conversation.get_pending_tool_calls().is_some());
        assert_eq!(conversation.get_pending_tool_calls().unwrap().len(), 1);
    }

    #[test]
    fn test_has_pending_tool_calls_when_user_message_follows() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("Read test.txt".to_string());

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\": \"test.txt\"}".to_string(),
            },
        };

        conversation.add_assistant_message(None, Some(vec![tool_call]));
        // Simulate user saying "continue" after interruption
        conversation.add_user_message("continue".to_string());

        // Second-to-last message is assistant with tool_calls, last is user
        assert!(conversation.has_pending_tool_calls());
        assert!(conversation.get_pending_tool_calls().is_some());
        assert_eq!(conversation.get_pending_tool_calls().unwrap().len(), 1);
        assert_eq!(
            conversation.get_pending_tool_calls().unwrap()[0].id,
            "call_123"
        );
    }

    #[test]
    fn test_no_pending_tool_calls_when_results_provided() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("Read test.txt".to_string());

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\": \"test.txt\"}".to_string(),
            },
        };

        conversation.add_assistant_message(None, Some(vec![tool_call]));

        let tool_result = ToolCallResponse::success(
            "call_123".to_string(),
            "read_file".to_string(),
            "Read(test.txt)".to_string(),
            "File contents".to_string(),
        );
        conversation.add_tool_result(tool_result);

        // Tool results provided, no pending calls
        assert!(!conversation.has_pending_tool_calls());
        assert!(conversation.get_pending_tool_calls().is_none());
    }

    #[test]
    fn test_repair_incomplete_tool_calls_without_user_message() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("Read test.txt".to_string());

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\": \"test.txt\"}".to_string(),
            },
        };

        conversation
            .add_assistant_message(Some("Let me read that".to_string()), Some(vec![tool_call]));

        // Last message is assistant with tool_calls (interruption scenario)
        assert_eq!(conversation.messages.len(), 2);
        assert!(conversation.has_pending_tool_calls());

        let repaired = conversation.repair_incomplete_tool_calls();
        assert!(repaired);

        // Should have added synthetic tool result
        assert_eq!(conversation.messages.len(), 3);
        assert_eq!(conversation.messages[2].role, "tool");
        assert_eq!(
            conversation.messages[2].tool_call_id,
            Some("call_123".to_string())
        );
        assert!(
            conversation.messages[2]
                .content
                .as_ref()
                .unwrap()
                .contains("interrupted")
        );
    }

    #[test]
    fn test_repair_incomplete_tool_calls_with_user_message() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("Read test.txt".to_string());

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\": \"test.txt\"}".to_string(),
            },
        };

        conversation
            .add_assistant_message(Some("Let me read that".to_string()), Some(vec![tool_call]));
        // User says "continue" after interruption
        conversation.add_user_message("continue".to_string());

        assert_eq!(conversation.messages.len(), 3);
        assert!(conversation.has_pending_tool_calls());

        let repaired = conversation.repair_incomplete_tool_calls();
        assert!(repaired);

        // Should have inserted synthetic tool result BEFORE user message
        assert_eq!(conversation.messages.len(), 4);
        assert_eq!(conversation.messages[2].role, "tool");
        assert_eq!(
            conversation.messages[2].tool_call_id,
            Some("call_123".to_string())
        );
        assert!(
            conversation.messages[2]
                .content
                .as_ref()
                .unwrap()
                .contains("interrupted")
        );
        // User message should still be last
        assert_eq!(conversation.messages[3].role, "user");
        assert_eq!(
            conversation.messages[3].content,
            Some("continue".to_string())
        );
    }

    #[test]
    fn test_repair_multiple_incomplete_tool_calls() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("Read two files".to_string());

        let tool_call1 = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\": \"test1.txt\"}".to_string(),
            },
        };

        let tool_call2 = ToolCall {
            id: "call_456".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\": \"test2.txt\"}".to_string(),
            },
        };

        conversation.add_assistant_message(None, Some(vec![tool_call1, tool_call2]));
        conversation.add_user_message("continue".to_string());

        assert!(conversation.has_pending_tool_calls());
        assert_eq!(conversation.get_pending_tool_calls().unwrap().len(), 2);

        let repaired = conversation.repair_incomplete_tool_calls();
        assert!(repaired);

        // Should have added 2 synthetic tool results before user message
        assert_eq!(conversation.messages.len(), 5);
        assert_eq!(conversation.messages[2].role, "tool");
        assert_eq!(
            conversation.messages[2].tool_call_id,
            Some("call_123".to_string())
        );
        assert_eq!(conversation.messages[3].role, "tool");
        assert_eq!(
            conversation.messages[3].tool_call_id,
            Some("call_456".to_string())
        );
        assert_eq!(conversation.messages[4].role, "user");
    }

    #[test]
    fn test_repair_does_nothing_when_no_pending_calls() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());
        conversation.add_assistant_message(Some("Hi!".to_string()), None);

        assert!(!conversation.has_pending_tool_calls());

        let repaired = conversation.repair_incomplete_tool_calls();
        assert!(!repaired);

        // No changes should be made
        assert_eq!(conversation.messages.len(), 2);
    }

    #[test]
    fn test_repair_idempotent() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("Read test.txt".to_string());

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{\"path\": \"test.txt\"}".to_string(),
            },
        };

        conversation.add_assistant_message(None, Some(vec![tool_call]));
        conversation.add_user_message("continue".to_string());

        // First repair
        let repaired1 = conversation.repair_incomplete_tool_calls();
        assert!(repaired1);
        let len_after_first = conversation.messages.len();

        // Second repair should do nothing (already repaired)
        let repaired2 = conversation.repair_incomplete_tool_calls();
        assert!(!repaired2);
        assert_eq!(conversation.messages.len(), len_after_first);
    }
}
