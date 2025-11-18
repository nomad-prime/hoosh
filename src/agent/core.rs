use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::agent_events::AgentEvent;
use crate::agent::{Conversation, ToolCall, ToolCallResponse};
use crate::backends::{LlmBackend, LlmResponse};
use crate::context_management::ContextManager;
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

pub struct Agent {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    tool_executor: Arc<ToolExecutor>,
    max_steps: usize,
    event_sender: Option<mpsc::UnboundedSender<AgentEvent>>,
    context_manager: Option<Arc<ContextManager>>,
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

    pub async fn generate_title(&self, first_user_message: &str) -> Result<String> {
        let prompt = format!(
            "Generate a short title (5-8 words) for a conversation starting with: {}",
            first_user_message
        );

        let title = self.backend.send_message(&prompt).await?;
        let title = title.trim().trim_matches('"').to_string();

        Ok(title)
    }

    async fn ensure_title(&self, conversation: &mut Conversation) {
        if conversation.title().is_empty()
            && let Some(first_user_msg) = conversation
                .messages
                .iter()
                .find(|m| m.role == "user")
                .and_then(|m| m.content.as_ref())
        {
            match self.generate_title(first_user_msg).await {
                Ok(title) => {
                    conversation.set_title(title.clone());
                    self.send_event(AgentEvent::DebugMessage(format!(
                        "Started: {} ({})",
                        title,
                        conversation.id()
                    )));
                }
                Err(e) => {
                    self.send_event(AgentEvent::DebugMessage(format!(
                        "Warning: Failed to generate title: {}",
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

    pub async fn handle_turn(&self, conversation: &mut Conversation) -> Result<()> {
        self.send_event(AgentEvent::Thinking);

        // Repair any incomplete tool calls from previous interrupted sessions
        conversation.repair_incomplete_tool_calls();

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
                    continue;
                }
                Err(e) => {
                    self.send_event(AgentEvent::Error(e.user_message()));
                    return Err(anyhow::Error::new(e));
                }
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

        context_manager
            .apply_strategies(conversation)
            .await
            .expect("error applying context management");

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

        // Phase 3: Check for rejections and permission denials
        let rejected_tool_call_names = self.rejected_tool_call_names(&tool_results);
        let permission_denied_tool_call_names =
            self.permission_denied_tool_call_names(&tool_results);

        for tool_result in tool_results {
            conversation.add_tool_result(tool_result);
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
#[path = "core_tests.rs"]
mod tests;
