use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::backends::{LlmBackend, LlmResponse};
use crate::conversations::agent_events::AgentEvent;
use crate::conversations::{ContextManager, Conversation, ToolCall, ToolCallResponse};
use crate::permissions::PermissionScope;
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

#[derive(Debug, Clone)]
pub struct PermissionResponse {
    pub request_id: String,
    pub allowed: bool,
    pub scope: Option<PermissionScope>,
}

#[derive(Debug, Clone)]
pub struct ApprovalResponse {
    pub tool_call_id: String,
    pub approved: bool,
    pub rejection_reason: Option<String>,
}

pub struct ConversationHandler {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    tool_executor: Arc<ToolExecutor>,
    max_steps: usize,
    event_sender: Option<mpsc::UnboundedSender<AgentEvent>>,
    context_manager: Option<Arc<ContextManager>>,
}

impl ConversationHandler {
    pub fn new(
        backend: Arc<dyn LlmBackend>,
        tool_registry: Arc<ToolRegistry>,
        tool_executor: Arc<ToolExecutor>,
    ) -> Self {
        Self {
            backend,
            tool_registry,
            tool_executor,
            max_steps: 1000,
            event_sender: None,
            context_manager: None,
        }
    }

    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    pub fn with_event_sender(mut self, sender: mpsc::UnboundedSender<AgentEvent>) -> Self {
        self.event_sender = Some(sender);
        self
    }

    pub fn with_context_manager(mut self, context_manager: Arc<ContextManager>) -> Self {
        self.context_manager = Some(context_manager);
        self
    }

    fn send_event(&self, event: AgentEvent) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event);
        }
    }

    pub async fn handle_turn(&self, conversation: &mut Conversation) -> Result<()> {
        self.send_event(AgentEvent::Thinking);

        // Apply context compression if configured
        if let Some(context_manager) = &self.context_manager {
            self.apply_context_compression(conversation, context_manager)
                .await?;
        }

        for step in 0..self.max_steps {
            let response = self
                .backend
                .send_message_with_tools_and_events(
                    conversation,
                    &self.tool_registry,
                    self.event_sender.clone(),
                )
                .await?;

            match self.process_response(conversation, response, step).await? {
                TurnStatus::Continue => continue,
                TurnStatus::Complete => return Ok(()),
            }
        }

        self.send_event(AgentEvent::MaxStepsReached(self.max_steps));
        Ok(())
    }

    async fn apply_context_compression(
        &self,
        conversation: &mut Conversation,
        context_manager: &ContextManager,
    ) -> Result<()> {
        let current_pressure = context_manager.get_token_pressure();

        // Emit warning if approaching threshold
        if context_manager.should_warn_about_pressure() {
            self.send_event(AgentEvent::TokenPressureWarning {
                current_pressure,
                threshold: context_manager.config.warning_threshold,
            });
        }

        if context_manager.should_compress() {
            let original_count = conversation.messages.len();
            self.send_event(AgentEvent::ContextCompressionTriggered {
                original_message_count: original_count,
                compressed_message_count: 0,
                token_pressure: current_pressure,
            });

            match context_manager
                .apply_context_compression(&conversation.messages)
                .await
            {
                Ok(compressed_messages) => {
                    let compressed_count = compressed_messages.len();
                    conversation.messages = compressed_messages;

                    self.send_event(AgentEvent::ContextCompressionComplete {
                        summary_length: original_count - compressed_count,
                    });
                }
                Err(e) => {
                    self.send_event(AgentEvent::ContextCompressionError {
                        error: e.to_string(),
                    });
                    // Continue without compression on error
                }
            }
        }

        Ok(())
    }

    async fn process_response(
        &self,
        conversation: &mut Conversation,
        response: LlmResponse,
        step: usize,
    ) -> Result<TurnStatus> {
        // Record token usage in context manager if available
        if let (Some(input_tokens), Some(output_tokens)) =
            (response.input_tokens, response.output_tokens)
        {
            if let Some(context_manager) = &self.context_manager {
                context_manager.record_token_usage(input_tokens, output_tokens);
            }
            let cost = self
                .backend
                .pricing()
                .map(|p| p.calculate_cost(input_tokens, output_tokens));
            self.send_event(AgentEvent::TokenUsage {
                input_tokens,
                output_tokens,
                cost,
            });
        }

        if let Some(ref tool_calls) = response.tool_calls
            && !tool_calls.is_empty()
        {
            return self.handle_tool_calls(conversation, response, step).await;
        }

        if let Some(content) = response.content {
            self.send_event(AgentEvent::FinalResponse(content.clone()));
            conversation.add_assistant_message(Some(content), None);
            return Ok(TurnStatus::Complete);
        }

        self.send_event(AgentEvent::Error("No response received".to_string()));
        Ok(TurnStatus::Complete)
    }

    async fn handle_tool_calls(
        &self,
        conversation: &mut Conversation,
        response: LlmResponse,
        _step: usize,
    ) -> Result<TurnStatus> {
        let tool_calls = response
            .tool_calls
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Expected tool calls but none found"))?;

        conversation.add_assistant_message(response.content.clone(), Some(tool_calls.clone()));

        // Phase 1: Emit tool call events
        if let Some(ref content) = response.content {
            self.send_event(AgentEvent::AssistantThought(content.clone()));
        }
        self.emit_tool_call_events(&tool_calls);

        // Phase 2: Execute tools
        let tool_results = self.tool_executor.execute_tool_calls(&tool_calls).await;

        // Phase 3: Check for rejections
        if self.has_user_rejection(&tool_results) {
            self.emit_tool_results(&tool_results);
            for tool_result in tool_results {
                conversation.add_tool_result(tool_result);
            }
            self.send_event(AgentEvent::UserRejection);
            return Ok(TurnStatus::Complete);
        }

        // Phase 4: Emit results and update conversation
        self.emit_tool_results(&tool_results);
        for tool_result in tool_results {
            conversation.add_tool_result(tool_result);
        }
        self.send_event(AgentEvent::ToolExecutionComplete);

        Ok(TurnStatus::Continue)
    }

    fn emit_tool_call_events(&self, tool_calls: &[ToolCall]) {
        let tool_call_displays: Vec<String> = tool_calls
            .iter()
            .map(|tc| {
                if let Some(tool) = self.tool_registry.get_tool(&tc.function.name) {
                    if let Ok(args) = serde_json::from_str(&tc.function.arguments) {
                        tool.format_call_display(&args)
                    } else {
                        tc.function.name.clone()
                    }
                } else {
                    tc.function.name.clone()
                }
            })
            .collect();

        self.send_event(AgentEvent::ToolCalls(tool_call_displays));
    }

    fn emit_tool_results(&self, tool_results: &[ToolCallResponse]) {
        for tool_result in tool_results {
            let summary = self.get_tool_result_summary(tool_result);
            self.send_event(AgentEvent::ToolResult {
                tool_name: tool_result.display_name.clone(),
                summary,
            });
        }
    }

    fn get_tool_result_summary(&self, tool_result: &ToolCallResponse) -> String {
        if let Some(tool) = self.tool_registry.get_tool(&tool_result.tool_name) {
            match &tool_result.result {
                Ok(output) => tool.result_summary(output),
                Err(e) => self.format_error_summary(e),
            }
        } else {
            match &tool_result.result {
                Ok(_) => "Completed".to_string(),
                Err(e) => self.format_error_summary(e),
            }
        }
    }

    fn format_error_summary(&self, error: &crate::tools::error::ToolError) -> String {
        error.user_facing_message()
    }

    fn has_user_rejection(&self, tool_results: &[ToolCallResponse]) -> bool {
        tool_results.iter().any(|result| result.is_rejected())
    }
}

enum TurnStatus {
    Continue,
    Complete,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::LlmResponse;
    use crate::conversations::{ToolCall, ToolFunction};
    use crate::permissions::PermissionManager;
    use async_trait::async_trait;

    struct MockBackend {
        responses: Vec<LlmResponse>,
        current_index: std::sync::Mutex<usize>,
    }

    impl MockBackend {
        fn new(responses: Vec<LlmResponse>) -> Self {
            Self {
                responses,
                current_index: std::sync::Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl LlmBackend for MockBackend {
        async fn send_message(&self, _message: &str) -> Result<String> {
            unimplemented!()
        }

        async fn send_message_with_tools(
            &self,
            _conversation: &Conversation,
            _tools: &ToolRegistry,
        ) -> Result<LlmResponse> {
            let mut index = self
                .current_index
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock current_index: {}", e))?;
            let response = self.responses.get(*index).cloned();
            *index += 1;
            response.ok_or_else(|| anyhow::anyhow!("No more responses"))
        }

        fn backend_name(&self) -> &'static str {
            "mock"
        }

        fn model_name(&self) -> &str {
            "mock-model"
        }
    }

    #[tokio::test]
    async fn test_conversation_handler_simple_response() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let mock_backend: Arc<dyn LlmBackend> =
            Arc::new(MockBackend::new(vec![LlmResponse::content_only(
                "Hello, how can I help?".to_string(),
            )]));

        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            PermissionManager::new(event_tx, response_rx).with_skip_permissions(true);
        let tool_executor = Arc::new(ToolExecutor::new(
            (*tool_registry).clone(),
            permission_manager,
        ));

        let handler = ConversationHandler::new(mock_backend, tool_registry, tool_executor);

        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let result = handler.handle_turn(&mut conversation).await;
        assert!(result.is_ok());
        assert_eq!(conversation.messages.len(), 2);
    }

    #[tokio::test]
    async fn test_conversation_handler_with_tool_call() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: r#"{"path": "test.txt"}"#.to_string(),
            },
        };

        let mock_backend: Arc<dyn LlmBackend> = Arc::new(MockBackend::new(vec![
            LlmResponse::with_tool_calls(Some("Reading file".to_string()), vec![tool_call]),
            LlmResponse::content_only("File read successfully".to_string()),
        ]));

        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();

        let tool_registry = Arc::new(ToolExecutor::create_tool_registry_with_working_dir(
            temp_dir.path().to_path_buf(),
        ));
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            PermissionManager::new(event_tx, response_rx).with_skip_permissions(true);
        let tool_executor = Arc::new(ToolExecutor::new(
            (*tool_registry).clone(),
            permission_manager,
        ));

        let handler = ConversationHandler::new(mock_backend, tool_registry, tool_executor);

        let mut conversation = Conversation::new();
        conversation.add_user_message("Read test.txt".to_string());

        let result = handler.handle_turn(&mut conversation).await;
        assert!(result.is_ok());
    }
}
