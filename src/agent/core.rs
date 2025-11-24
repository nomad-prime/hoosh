use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::agent_events::AgentEvent;
use crate::agent::{Conversation, ToolCall, ToolCallResponse};
use crate::backends::{LlmBackend, LlmResponse};
use crate::context_management::ContextManager;
use crate::permissions::PermissionScope;
use crate::system_reminders::{ReminderContext, SideEffectResult, SystemReminder};
use crate::task_management::ExecutionBudget;
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
    execution_budget: Option<Arc<ExecutionBudget>>,
    system_reminder: Option<Arc<SystemReminder>>,
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
            execution_budget: None,
            system_reminder: None,
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

    pub fn with_execution_budget(mut self, budget: Arc<ExecutionBudget>) -> Self {
        self.execution_budget = Some(budget);
        self
    }

    pub fn with_system_reminder(mut self, reminder: Arc<SystemReminder>) -> Self {
        self.system_reminder = Some(reminder);
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
            self.apply_context_strategies(conversation, context_manager)
                .await?;
        }

        for step in 0..self.max_steps {
            self.send_event(AgentEvent::StepStarted { step });
            let should_exit = self.handle_budget(conversation, step).await?;
            if should_exit {
                return Ok(());
            }

            // Apply system reminders
            let total_tokens = conversation
                .messages
                .iter()
                .map(|m| m.content.as_ref().map(|c| c.len() / 4).unwrap_or(0))
                .sum::<usize>();
            
            let reminder_result = self
                .apply_system_reminders(conversation, step, total_tokens)
                .await?;
            
            if matches!(reminder_result, SideEffectResult::ExitTurn) {
                self.ensure_title(conversation).await;
                return Ok(());
            }

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

            match self.process_response(conversation, response).await? {
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

    async fn handle_budget(&self, conversation: &mut Conversation, step: usize) -> Result<bool> {
        if let Some(budget) = &self.execution_budget {
            let remaining = budget.remaining_seconds();

            if budget.should_wrap_up(step) {
                let wrap_up_message = format!(
                    "BUDGET ALERT: You have approximately {} seconds and {} steps remaining. \
                Please prioritize wrapping up your work and providing a final answer.",
                    remaining,
                    self.max_steps.saturating_sub(step)
                );
                conversation.add_system_message(wrap_up_message);
            }

            if remaining == 0 {
                self.send_event(AgentEvent::Error("Time budget exhausted".to_string()));
                let conclusion_message = "Time budget has been exhausted. Please provide a brief summary of what you've accomplished so far.";
                conversation.add_user_message(conclusion_message.to_string());

                let response = self
                    .backend
                    .send_message_with_tools_and_events(
                        conversation,
                        &self.tool_registry,
                        self.event_sender.clone(),
                    )
                    .await?;

                if let Some(content) = response.content {
                    self.send_event(AgentEvent::FinalResponse(content.clone()));
                    conversation.add_assistant_message(Some(content), None);
                }

                self.ensure_title(conversation).await;
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn apply_context_strategies(
        &self,
        conversation: &mut Conversation,
        context_manager: &ContextManager,
    ) -> Result<()> {
        context_manager
            .apply_strategies(conversation)
            .await
            .expect("error applying context management");

        let pressure_after = context_manager.get_token_pressure(conversation);

        if context_manager.should_warn_about_pressure_value(pressure_after) {
            self.send_event(AgentEvent::TokenPressureWarning {
                current_pressure: pressure_after,
                threshold: context_manager.config.warning_threshold,
            });
        }

        Ok(())
    }

    async fn apply_system_reminders(
        &self,
        conversation: &mut Conversation,
        step: usize,
        total_tokens: usize,
    ) -> Result<SideEffectResult> {
        if let Some(reminder) = &self.system_reminder {
            let ctx = ReminderContext {
                step,
                max_steps: self.max_steps,
                total_tokens,
            };
            reminder.apply(&ctx, conversation, self).await
        } else {
            Ok(SideEffectResult::Continue)
        }
    }

    async fn process_response(
        &self,
        conversation: &mut Conversation,
        response: LlmResponse,
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
            return self.handle_tool_calls(conversation, response).await;
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
        let conversation_id = Some(conversation.id());
        let tool_results = self
            .tool_executor
            .execute_tool_calls(&tool_calls, conversation_id)
            .await;

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
            .filter_map(|tc| {
                let tool = self.tool_registry.get_tool(&tc.function.name);

                // Skip hidden tools - they shouldn't appear in the UI
                if tool.map(|t| t.is_hidden()).unwrap_or(false) {
                    return None;
                }

                let display = if let Some(tool) = tool {
                    if let Ok(args) = serde_json::from_str(&tc.function.arguments) {
                        tool.format_call_display(&args)
                    } else {
                        tc.function.name.clone()
                    }
                } else {
                    tc.function.name.clone()
                };
                Some((tc.id.clone(), display))
            })
            .collect();

        if !tool_call_info.is_empty() {
            self.send_event(AgentEvent::ToolCalls(tool_call_info));
        }
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
