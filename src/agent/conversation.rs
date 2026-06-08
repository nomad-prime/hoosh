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
                .unwrap_or_default()
                .as_secs()
        );
        Self {
            metadata: ConversationMetadata::new(id),
            messages: Vec::new(),
            storage: None,
        }
    }

    pub fn with_storage(id: String, storage: Arc<ConversationStorage>) -> Result<Self> {
        let metadata = storage.create_conversation(&id)?;
        Ok(Self {
            metadata,
            messages: Vec::new(),
            storage: Some(storage),
        })
    }

    pub fn load(id: &str, storage: Arc<ConversationStorage>) -> Result<Self> {
        let metadata = storage.load_metadata(id)?;
        let messages = storage.load_messages(id)?;
        Ok(Self {
            metadata,
            messages,
            storage: Some(storage),
        })
    }

    pub fn with_subagent_storage(
        parent_conversation_id: &str,
        tool_call_id: &str,
        storage: Arc<ConversationStorage>,
    ) -> Result<Self> {
        let subagent_id = format!("{}/subagent-{}", parent_conversation_id, tool_call_id);
        let metadata = storage.create_conversation(&subagent_id)?;
        Ok(Self {
            metadata,
            messages: Vec::new(),
            storage: Some(storage),
        })
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

    pub fn clear_turn_history(&mut self) {
        if self.messages.len() > 2 {
            self.messages.truncate(2);
        }
    }

    pub fn get_messages_for_api(&self) -> &Vec<ConversationMessage> {
        &self.messages
    }

    pub fn set_title(&mut self, title: String) {
        self.metadata.title = title.clone();
        self.metadata.update();

        if let Some(storage) = &self.storage
            && let Err(e) = storage.save_metadata(&self.metadata)
        {
            console().error(&format!("Warning: Failed to persist title update: {}", e))
        }
    }

    pub fn id(&self) -> &str {
        &self.metadata.id
    }

    pub fn title(&self) -> &str {
        &self.metadata.title
    }

    pub fn name(&self) -> Option<&str> {
        self.metadata.name.as_deref()
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.metadata.name = name.filter(|s| !s.is_empty());
        self.metadata.update();

        if let Some(storage) = &self.storage
            && let Err(e) = storage.save_metadata(&self.metadata)
        {
            console().error(&format!("Warning: Failed to persist name update: {}", e))
        }
    }

    pub fn has_storage(&self) -> bool {
        self.storage.is_some()
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

    /// Sanitize orphan tool_calls left by a crash or interruption.
    ///
    /// The previous behaviour injected synthetic "Error: Tool execution was
    /// interrupted" tool messages so each assistant tool_call had a matching
    /// result. That kept the conversation structurally valid but lied to the
    /// model — it would see a "result" it never produced and either try to
    /// recover or get confused.
    ///
    /// New behaviour: drop orphan calls. For each assistant message with
    /// tool_calls whose results aren't fully present in the next run of
    /// `tool` messages, remove the partial tool messages and either:
    ///   - strip the tool_calls field from the assistant message if it has
    ///     non-empty content (so the assistant's thought is preserved), OR
    ///   - remove the assistant message entirely if it had no content.
    ///
    /// Any trailing user/system messages are preserved.
    ///
    /// If the conversation has on-disk storage, the file is rewritten so the
    /// drop survives a reload.
    ///
    /// Returns true if any sanitization happened.
    pub fn sanitize_orphan_tool_calls(&mut self) -> bool {
        use std::collections::HashSet;

        let assistant_with_calls: Vec<usize> = self
            .messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.role == "assistant" && m.tool_calls.is_some())
            .map(|(i, _)| i)
            .collect();

        let mut changed = false;

        // Walk backwards so removals don't invalidate earlier indices.
        for &asst_idx in assistant_with_calls.iter().rev() {
            let expected_ids: Vec<String> = self.messages[asst_idx]
                .tool_calls
                .as_ref()
                .unwrap()
                .iter()
                .map(|tc| tc.id.clone())
                .collect();

            // Collect the run of `tool` messages immediately after.
            let mut tool_idxs = Vec::new();
            let mut found: HashSet<String> = HashSet::new();
            let mut j = asst_idx + 1;
            while j < self.messages.len() && self.messages[j].role == "tool" {
                if let Some(ref id) = self.messages[j].tool_call_id {
                    found.insert(id.clone());
                }
                tool_idxs.push(j);
                j += 1;
            }

            let any_missing = expected_ids.iter().any(|id| !found.contains(id));
            if !any_missing {
                continue;
            }
            changed = true;

            // Remove partial tool messages in reverse.
            for &idx in tool_idxs.iter().rev() {
                self.messages.remove(idx);
            }

            let has_content = self.messages[asst_idx]
                .content
                .as_ref()
                .map(|c| !c.trim().is_empty())
                .unwrap_or(false);
            if has_content {
                self.messages[asst_idx].tool_calls = None;
            } else {
                self.messages.remove(asst_idx);
            }
        }

        if changed {
            // Rewrite the persisted log so the sanitization survives reloads.
            if let Some(storage) = &self.storage
                && let Err(e) = storage.rewrite_messages(&self.metadata.id, &self.messages)
            {
                eprintln!("Warning: failed to rewrite sanitized conversation log: {e}");
            }
        }

        changed
    }

    /// Deprecated name kept for one release to avoid breaking external callers.
    #[deprecated(
        note = "use sanitize_orphan_tool_calls — drops orphans instead of synthesising them"
    )]
    pub fn repair_incomplete_tool_calls(&mut self) -> bool {
        self.sanitize_orphan_tool_calls()
    }

    /// Estimate the number of tokens in this conversation.
    /// Uses ~4 bytes per token (industry standard approximation).
    /// Accounts for message content, tool calls, roles, and names.
    pub fn estimate_token(&self) -> usize {
        const APPROX_BYTES_PER_TOKEN: usize = 4;

        let total_bytes: usize = self.messages.iter().map(Self::estimate_message_bytes).sum();

        // Round up: (bytes + 3) / 4
        total_bytes.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1))
            / APPROX_BYTES_PER_TOKEN
    }

    /// Estimate the byte size of a single conversation message.
    /// This accounts for all message fields including large tool outputs.
    fn estimate_message_bytes(msg: &ConversationMessage) -> usize {
        let mut total = 0;

        // Content field (can be very large for tool outputs)
        if let Some(content) = &msg.content {
            total += content.len();
        }

        // Tool calls (including arguments JSON which can be large)
        if let Some(tool_calls) = &msg.tool_calls {
            for call in tool_calls {
                total += call.function.name.len();
                total += call.function.arguments.len();
            }
        }

        // Role and other fields (small but count them)
        total += msg.role.len();

        if let Some(name) = &msg.name {
            total += name.len();
        }

        total
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

    /// Orphan with no content: drop the assistant message entirely.
    #[test]
    fn sanitize_drops_orphan_assistant_with_no_content() {
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

        assert_eq!(conversation.messages.len(), 2);
        assert!(conversation.sanitize_orphan_tool_calls());
        // Assistant message removed → only user left.
        assert_eq!(conversation.messages.len(), 1);
        assert_eq!(conversation.messages[0].role, "user");
    }

    /// Orphan with content: keep the assistant message but strip tool_calls.
    #[test]
    fn sanitize_strips_tool_calls_but_keeps_assistant_content() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("read it".to_string());

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{}".to_string(),
            },
        };
        conversation.add_assistant_message(
            Some("I'll read that file now.".to_string()),
            Some(vec![tool_call]),
        );

        assert!(conversation.sanitize_orphan_tool_calls());
        assert_eq!(conversation.messages.len(), 2);
        assert_eq!(conversation.messages[1].role, "assistant");
        assert_eq!(
            conversation.messages[1].content,
            Some("I'll read that file now.".to_string())
        );
        assert!(conversation.messages[1].tool_calls.is_none());
    }

    /// Trailing user "continue" should be preserved on the timeline after a drop.
    #[test]
    fn sanitize_preserves_trailing_user_message() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("read test.txt".to_string());

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: "{}".to_string(),
            },
        };
        conversation.add_assistant_message(None, Some(vec![tool_call]));
        conversation.add_user_message("continue".to_string());

        assert!(conversation.sanitize_orphan_tool_calls());
        // Orphan assistant removed; both user messages remain.
        assert_eq!(conversation.messages.len(), 2);
        assert_eq!(conversation.messages[0].role, "user");
        assert_eq!(conversation.messages[1].role, "user");
        assert_eq!(
            conversation.messages[1].content.as_deref(),
            Some("continue")
        );
    }

    /// Partial completion: some results present, others missing — drop the
    /// partial results too, so the model never sees half a tool batch.
    #[test]
    fn sanitize_drops_partial_results_for_incomplete_batch() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("read both".to_string());

        let tc1 = ToolCall {
            id: "call_1".into(),
            r#type: "function".into(),
            function: ToolFunction {
                name: "read_file".into(),
                arguments: "{}".into(),
            },
        };
        let tc2 = ToolCall {
            id: "call_2".into(),
            r#type: "function".into(),
            function: ToolFunction {
                name: "read_file".into(),
                arguments: "{}".into(),
            },
        };
        conversation.add_assistant_message(None, Some(vec![tc1, tc2]));

        // Only call_1 was persisted before the crash.
        conversation.add_tool_result(ToolCallResponse::success(
            "call_1".into(),
            "read_file".into(),
            "Read(a)".into(),
            "result-a".into(),
        ));

        assert!(conversation.sanitize_orphan_tool_calls());
        // Both the assistant (no content) and the partial result should be gone.
        assert_eq!(conversation.messages.len(), 1);
        assert_eq!(conversation.messages[0].role, "user");
    }

    /// Sanitize is a no-op when every tool_call has its result.
    #[test]
    fn sanitize_noop_when_complete() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("read".to_string());

        let tc = ToolCall {
            id: "call_1".into(),
            r#type: "function".into(),
            function: ToolFunction {
                name: "read_file".into(),
                arguments: "{}".into(),
            },
        };
        conversation.add_assistant_message(None, Some(vec![tc]));
        conversation.add_tool_result(ToolCallResponse::success(
            "call_1".into(),
            "read_file".into(),
            "Read(a)".into(),
            "ok".into(),
        ));

        assert!(!conversation.sanitize_orphan_tool_calls());
        assert_eq!(conversation.messages.len(), 3);
    }

    /// Sanitize is idempotent.
    #[test]
    fn sanitize_is_idempotent() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("read".to_string());
        let tc = ToolCall {
            id: "call_1".into(),
            r#type: "function".into(),
            function: ToolFunction {
                name: "read_file".into(),
                arguments: "{}".into(),
            },
        };
        conversation.add_assistant_message(None, Some(vec![tc]));
        conversation.add_user_message("continue".into());

        assert!(conversation.sanitize_orphan_tool_calls());
        let len_after = conversation.messages.len();
        assert!(!conversation.sanitize_orphan_tool_calls());
        assert_eq!(conversation.messages.len(), len_after);
    }

    /// Two separate orphan batches in one conversation: both get cleaned up.
    #[test]
    fn sanitize_handles_multiple_orphan_batches() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("first".into());
        let tc = ToolCall {
            id: "a".into(),
            r#type: "function".into(),
            function: ToolFunction {
                name: "read_file".into(),
                arguments: "{}".into(),
            },
        };
        conversation.add_assistant_message(Some("thinking…".into()), Some(vec![tc]));
        conversation.add_user_message("second".into());
        let tc2 = ToolCall {
            id: "b".into(),
            r#type: "function".into(),
            function: ToolFunction {
                name: "read_file".into(),
                arguments: "{}".into(),
            },
        };
        conversation.add_assistant_message(None, Some(vec![tc2]));

        assert!(conversation.sanitize_orphan_tool_calls());
        // First assistant kept (had content, tool_calls stripped); second dropped (no content).
        let roles: Vec<&str> = conversation
            .messages
            .iter()
            .map(|m| m.role.as_str())
            .collect();
        assert_eq!(roles, vec!["user", "assistant", "user"]);
        assert!(conversation.messages[1].tool_calls.is_none());
    }

    #[test]
    fn test_estimate_token_empty_conversation() {
        let conversation = Conversation::new();
        assert_eq!(conversation.estimate_token(), 0);
    }

    #[test]
    fn test_estimate_token_simple_message() {
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());
        // "user" (4) + "Hello" (5) = 9 bytes / 4 = 2 tokens (rounded up)
        let tokens = conversation.estimate_token();
        (2..=3).contains(&tokens);
    }

    #[test]
    fn test_estimate_token_with_all_fields() {
        let mut conversation = Conversation::new();

        let msg = ConversationMessage {
            role: "assistant".to_string(),         // 9 bytes
            content: Some("Response".to_string()), // 8 bytes
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: ToolFunction {
                    name: "test".to_string(),    // 4 bytes
                    arguments: "{}".to_string(), // 2 bytes
                },
            }]),
            tool_call_id: None,
            name: Some("assistant_name".to_string()), // 14 bytes
        };

        conversation.messages.push(msg);
        // 9 + 8 + 4 + 2 + 14 = 37 bytes / 4 = 9 tokens (rounded up)
        let tokens = conversation.estimate_token();
        (9..=10).contains(&tokens);
    }

    #[test]
    fn test_estimate_token_with_large_content() {
        let mut conversation = Conversation::new();

        let large_output = "x".repeat(10000);
        conversation.add_assistant_message(Some(large_output.clone()), None);

        // 9 ("assistant") + 10000 (content) = 10009 bytes
        // 10009 / 4 = 2502 tokens (rounded up)
        let tokens = conversation.estimate_token();
        (2500..=2510).contains(&tokens);
    }

    #[test]
    fn test_clear_turn_history_preserves_first_two_system_messages() {
        let mut conversation = Conversation::new();
        conversation.add_system_message("agent definition".to_string());
        conversation.add_system_message("env context".to_string());
        conversation.add_user_message("hello".to_string());
        conversation.add_assistant_message(Some("hi".to_string()), None);

        conversation.clear_turn_history();

        assert_eq!(conversation.messages.len(), 2);
        assert_eq!(conversation.messages[0].role, "system");
        assert_eq!(conversation.messages[1].role, "system");
    }

    #[test]
    fn test_clear_turn_history_removes_user_and_assistant_messages() {
        let mut conversation = Conversation::new();
        conversation.add_system_message("sys1".to_string());
        conversation.add_system_message("sys2".to_string());
        conversation.add_user_message("user msg".to_string());
        conversation.add_assistant_message(Some("response".to_string()), None);
        conversation.add_user_message("follow up".to_string());

        conversation.clear_turn_history();

        assert_eq!(conversation.messages.len(), 2);
        assert!(conversation.messages.iter().all(|m| m.role == "system"));
    }

    #[test]
    fn test_clear_turn_history_is_safe_when_fewer_than_two_messages() {
        let mut conversation = Conversation::new();
        conversation.clear_turn_history();
        assert_eq!(conversation.messages.len(), 0);

        conversation.add_system_message("only one".to_string());
        conversation.clear_turn_history();
        assert_eq!(conversation.messages.len(), 1);
    }
}
