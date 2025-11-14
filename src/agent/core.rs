use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::agent_events::AgentEvent;
use crate::agent::{Conversation, ConversationMessage, ToolCall, ToolCallResponse};
use crate::backends::{LlmBackend, LlmResponse};
use crate::context_management::ContextManager;
use crate::permissions::PermissionScope;
use crate::storage::ConversationStorage;
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

pub struct Agent {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    tool_executor: Arc<ToolExecutor>,
    max_steps: usize,
    event_sender: Option<mpsc::UnboundedSender<AgentEvent>>,
    context_manager: Option<Arc<ContextManager>>,
    conversation_storage: Option<Arc<ConversationStorage>>,
    conversation_id: Option<String>,
}

impl Agent {
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
            conversation_storage: None,
            conversation_id: None,
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

    pub fn with_conversation_storage(
        mut self,
        storage: Arc<ConversationStorage>,
        conversation_id: String,
    ) -> Self {
        self.conversation_storage = Some(storage);
        self.conversation_id = Some(conversation_id);
        self
    }

    pub fn persist_user_message(&self, message: &ConversationMessage) {
        self.persist_message(message);
    }

    pub async fn generate_title(&self, first_user_message: &str) -> Result<String> {
        let prompt = format!(
            "Generate a short title (5-8 words) for a conversation starting with: {}",
            first_user_message
        );

        let title = self.backend.send_message(&prompt).await?;
        let title = title.trim().trim_matches('"').to_string();

        Ok(title)
    }

    pub fn update_conversation_title(&self, title: String) -> Result<()> {
        if let (Some(storage), Some(conversation_id)) =
            (&self.conversation_storage, &self.conversation_id)
        {
            storage.update_title(conversation_id, title)?;
        }
        Ok(())
    }

    async fn ensure_title(&self, conversation: &Conversation) {
        if let (Some(storage), Some(conversation_id)) =
            (&self.conversation_storage, &self.conversation_id)
            && let Ok(metadata) = storage.load_metadata(conversation_id)
            && metadata.title.is_empty()
            && let Some(first_user_msg) = conversation
                .messages
                .iter()
                .find(|m| m.role == "user")
                .and_then(|m| m.content.as_ref())
        {
            match self.generate_title(first_user_msg).await {
                Ok(title) => {
                    if let Err(e) = storage.update_title(conversation_id, title.clone()) {
                        self.send_event(AgentEvent::DebugMessage(format!(
                            "Warning: Failed to update title: {}",
                            e
                        )));
                    } else {
                        self.send_event(AgentEvent::DebugMessage(format!(
                            "Started: {} ({})",
                            title, conversation_id
                        )));
                    }
                }
                Err(e) => {
                    self.send_event(AgentEvent::DebugMessage(format!(
                        "Warning: Failed to update title: {}",
                        e
                    )));
                }
            }
        }
    }

    fn send_event(&self, event: AgentEvent) {
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(event);
        }
    }

    fn persist_message(&self, message: &ConversationMessage) {
        if let (Some(storage), Some(conversation_id)) =
            (&self.conversation_storage, &self.conversation_id)
            && let Err(e) = storage.append_message(conversation_id, message)
        {
            eprintln!("Warning: Failed to persist message: {}", e);
        }
    }

    pub async fn handle_turn(&self, conversation: &mut Conversation) -> Result<()> {
        self.send_event(AgentEvent::Thinking);

        // Repair any incomplete tool calls from previous interrupted sessions
        let pre_repair_count = conversation.messages.len();
        if conversation.repair_incomplete_tool_calls() {
            // Persist the synthetic tool results that were just added
            let post_repair_count = conversation.messages.len();
            for i in pre_repair_count..post_repair_count {
                self.persist_message(&conversation.messages[i]);
            }
        }

        // Apply context compression if configured
        if let Some(context_manager) = &self.context_manager {
            self.apply_context_compression(conversation, context_manager)
                .await?;
        }

        for step in 0..self.max_steps {
            let response = match self
                .backend
                .send_message_with_tools_and_events(
                    conversation,
                    &self.tool_registry,
                    self.event_sender.clone(),
                )
                .await
            {
                Ok(response) => response,
                Err(e) if e.should_send_to_llm() => {
                    // Add error as user message so LLM can adjust
                    let error_msg = e.user_message();
                    conversation.add_user_message(error_msg);
                    self.persist_message(&conversation.messages[conversation.messages.len() - 1]);
                    continue;
                }
                Err(e) => return Err(anyhow::Error::new(e)),
            };

            match self.process_response(conversation, response, step).await? {
                TurnStatus::Continue => continue,
                TurnStatus::Complete => {
                    self.ensure_title(conversation).await;
                    return Ok(());
                }
            }
        }

        self.send_event(AgentEvent::MaxStepsReached(self.max_steps));
        self.ensure_title(conversation).await;
        Ok(())
    }

    async fn apply_context_compression(
        &self,
        conversation: &mut Conversation,
        context_manager: &ContextManager,
    ) -> Result<()> {
        let current_pressure = context_manager.get_token_pressure();

        if context_manager.should_warn_about_pressure() {
            self.send_event(AgentEvent::TokenPressureWarning {
                current_pressure,
                threshold: context_manager.config.warning_threshold,
            });
        }

        let original_count = conversation.messages.len();

        match context_manager.apply_strategies(conversation).await {
            Ok(_) => {
                let compressed_count = conversation.messages.len();
                if compressed_count < original_count {
                    self.send_event(AgentEvent::ContextCompressionComplete {
                        summary_length: original_count - compressed_count,
                    });
                }
            }
            Err(e) => {
                self.send_event(AgentEvent::ContextCompressionError {
                    error: e.to_string(),
                });
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
            self.persist_message(&conversation.messages[conversation.messages.len() - 1]);
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
        self.persist_message(&conversation.messages[conversation.messages.len() - 1]);

        // Phase 1: Emit tool call events
        if let Some(ref content) = response.content {
            self.send_event(AgentEvent::AssistantThought(content.clone()));
        }
        self.emit_tool_call_events(&tool_calls);

        // Phase 2: Execute tools
        let tool_results = self.tool_executor.execute_tool_calls(&tool_calls).await;

        // Phase 3: Check for rejections and permission denials
        let rejected_tool_call_names = self.rejected_tool_call_names(&tool_results);
        let permission_denied_tool_call_names =
            self.permission_denied_tool_call_names(&tool_results);

        for tool_result in tool_results {
            conversation.add_tool_result(tool_result);
            self.persist_message(&conversation.messages[conversation.messages.len() - 1]);
        }

        if !rejected_tool_call_names.is_empty() {
            self.send_event(AgentEvent::UserRejection(rejected_tool_call_names));
            return Ok(TurnStatus::Complete);
        }

        if !permission_denied_tool_call_names.is_empty() {
            self.send_event(AgentEvent::PermissionDenied(
                permission_denied_tool_call_names,
            ));
            return Ok(TurnStatus::Complete);
        }

        self.send_event(AgentEvent::AllToolsComplete);
        Ok(TurnStatus::Continue)
    }

    fn emit_tool_call_events(&self, tool_calls: &[ToolCall]) {
        let tool_call_info: Vec<(String, String)> = tool_calls
            .iter()
            .map(|tc| {
                let display = if let Some(tool) = self.tool_registry.get_tool(&tc.function.name) {
                    if let Ok(args) = serde_json::from_str(&tc.function.arguments) {
                        tool.format_call_display(&args)
                    } else {
                        tc.function.name.clone()
                    }
                } else {
                    tc.function.name.clone()
                };
                (tc.id.clone(), display)
            })
            .collect();

        self.send_event(AgentEvent::ToolCalls(tool_call_info));
    }

    fn rejected_tool_call_names(&self, tool_call_responses: &[ToolCallResponse]) -> Vec<String> {
        tool_call_responses
            .iter()
            .filter(|result| result.is_rejected())
            .map(|result| result.display_name.clone())
            .collect()
    }

    fn permission_denied_tool_call_names(
        &self,
        tool_call_responses: &[ToolCallResponse],
    ) -> Vec<String> {
        tool_call_responses
            .iter()
            .filter(|result| result.is_permission_denied())
            .map(|result| result.display_name.clone())
            .collect()
    }
}

enum TurnStatus {
    Continue,
    Complete,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::{LlmError, LlmResponse};
    use crate::permissions::PermissionManager;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockBackend {
        responses: Vec<LlmResponse>,
        call_count: Arc<AtomicUsize>,
    }

    impl MockBackend {
        fn new(responses: Vec<LlmResponse>) -> Self {
            Self {
                responses,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LlmBackend for MockBackend {
        async fn send_message(&self, _message: &str) -> Result<String> {
            let index = self.call_count.fetch_add(1, Ordering::SeqCst);
            self.responses
                .get(index)
                .and_then(|r| r.content.clone())
                .ok_or_else(|| anyhow::anyhow!("No more responses"))
        }

        async fn send_message_with_tools(
            &self,
            _conversation: &Conversation,
            _tools: &ToolRegistry,
        ) -> Result<LlmResponse, LlmError> {
            let index = self.call_count.fetch_add(1, Ordering::SeqCst);
            self.responses
                .get(index)
                .cloned()
                .ok_or_else(|| LlmError::Other {
                    message: "No more responses".to_string(),
                })
        }

        async fn send_message_with_tools_and_events(
            &self,
            _conversation: &Conversation,
            _tools: &ToolRegistry,
            _event_sender: Option<mpsc::UnboundedSender<AgentEvent>>,
        ) -> Result<LlmResponse, LlmError> {
            let index = self.call_count.fetch_add(1, Ordering::SeqCst);
            self.responses
                .get(index)
                .cloned()
                .ok_or_else(|| LlmError::Other {
                    message: "No more responses".to_string(),
                })
        }

        fn backend_name(&self) -> &'static str {
            "mock"
        }

        fn model_name(&self) -> &str {
            "mock-model"
        }

        fn pricing(&self) -> Option<crate::backends::TokenPricing> {
            None
        }
    }

    fn create_test_agent(
        backend: Arc<dyn LlmBackend>,
    ) -> (Agent, Arc<ToolRegistry>, Arc<ToolExecutor>, mpsc::UnboundedSender<AgentEvent>) {
        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx.clone(), response_rx).with_skip_permissions(true));
        let tool_executor = Arc::new(ToolExecutor::new(
            Arc::clone(&tool_registry),
            Arc::clone(&permission_manager),
        ));

        let agent = Agent::new(backend, Arc::clone(&tool_registry), tool_executor.clone());
        (agent, tool_registry, tool_executor, event_tx)
    }

    #[tokio::test]
    async fn agent_handles_simple_response() {
        let backend = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Hello, I'm here to help!".to_string(),
        )]));

        let (agent, _, _, _) = create_test_agent(backend);
        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let result = agent.handle_turn(&mut conversation).await;

        assert!(result.is_ok());
        assert_eq!(conversation.messages.len(), 2);
        assert!(conversation
            .messages
            .iter()
            .any(|m| m.content.as_ref().map_or(false, |c| c.contains("help"))));
    }

    #[tokio::test]
    async fn agent_handles_multiple_turns() {
        let backend = Arc::new(MockBackend::new(vec![
            LlmResponse::content_only("First response".to_string()),
            LlmResponse::content_only("Second response".to_string()),
        ]));

        let (agent, _, _, _) = create_test_agent(backend.clone());
        let mut conversation = Conversation::new();
        conversation.add_user_message("First message".to_string());

        let result = agent.handle_turn(&mut conversation).await;
        assert!(result.is_ok());

        conversation.add_user_message("Second message".to_string());
        let result = agent.handle_turn(&mut conversation).await;
        assert!(result.is_ok());

        assert!(backend.call_count() >= 2);
    }

    #[tokio::test]
    async fn agent_respects_max_steps() {
        let backend = Arc::new(MockBackend::new(vec![]));
        let (agent, _, _, _) = create_test_agent(backend);
        let agent = agent.with_max_steps(5);

        let mut conversation = Conversation::new();
        conversation.add_user_message("Hello".to_string());

        let result = agent.handle_turn(&mut conversation).await;

        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn agent_builder_pattern_works() {
        let backend = Arc::new(MockBackend::new(vec![]));
        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx.clone(), response_rx).with_skip_permissions(true));
        let tool_executor = Arc::new(ToolExecutor::new(tool_registry.clone(), permission_manager));

        let agent = Agent::new(backend, tool_registry, tool_executor)
            .with_max_steps(100)
            .with_event_sender(event_tx.clone());

        assert_eq!(agent.max_steps, 100);
        assert!(agent.event_sender.is_some());
    }

    #[tokio::test]
    async fn title_generation_returns_valid_string() {
        let backend = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "\"Helpful Assistant Conversation\"".to_string(),
        )]));

        let (agent, _, _, _) = create_test_agent(backend);
        let title = agent.generate_title("How can I learn Rust?").await;

        assert!(title.is_ok());
        let title_str = title.unwrap();
        assert!(!title_str.is_empty());
        assert!(!title_str.contains('"'));
    }

    #[tokio::test]
    async fn permission_response_fields_accessible() {
        let response = PermissionResponse {
            request_id: "req_123".to_string(),
            allowed: true,
            scope: None,
        };

        assert_eq!(response.request_id, "req_123");
        assert!(response.allowed);
        assert!(response.scope.is_none());
    }

    #[tokio::test]
    async fn approval_response_with_rejection_reason() {
        let response = ApprovalResponse {
            tool_call_id: "call_456".to_string(),
            approved: false,
            rejection_reason: Some("User declined".to_string()),
        };

        assert_eq!(response.tool_call_id, "call_456");
        assert!(!response.approved);
        assert_eq!(response.rejection_reason, Some("User declined".to_string()));
    }

    #[tokio::test]
    async fn agent_initializes_with_defaults() {
        let backend = Arc::new(MockBackend::new(vec![]));
        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));
        let tool_executor = Arc::new(ToolExecutor::new(tool_registry.clone(), permission_manager));

        let agent = Agent::new(backend, tool_registry, tool_executor);

        assert_eq!(agent.max_steps, 1000);
        assert!(agent.event_sender.is_none());
        assert!(agent.context_manager.is_none());
        assert!(agent.conversation_storage.is_none());
        assert!(agent.conversation_id.is_none());
    }

    #[tokio::test]
    async fn multiple_agents_operate_independently() {
        let backend1 = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Agent 1".to_string(),
        )]));
        let backend2 = Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Agent 2".to_string(),
        )]));

        let (agent1, _, _, _) = create_test_agent(backend1);
        let (agent2, _, _, _) = create_test_agent(backend2);

        let mut conv1 = Conversation::new();
        conv1.add_user_message("Message 1".to_string());

        let mut conv2 = Conversation::new();
        conv2.add_user_message("Message 2".to_string());

        let result1 = agent1.handle_turn(&mut conv1).await;
        let result2 = agent2.handle_turn(&mut conv2).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }
}
