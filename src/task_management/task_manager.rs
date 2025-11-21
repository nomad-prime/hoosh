use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::{Agent, AgentEvent, Conversation};
use crate::backends::LlmBackend;
use crate::permissions::PermissionManager;
use crate::storage::ConversationStorage;
use crate::task_management::{ExecutionBudget, TaskDefinition, TaskEvent, TaskResult};
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

pub struct TaskManager {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
    event_tx: Option<mpsc::UnboundedSender<AgentEvent>>,
    tool_call_id: Option<String>,
    parent_conversation_id: Option<String>,
}

impl TaskManager {
    pub fn new(
        backend: Arc<dyn LlmBackend>,
        tool_registry: Arc<ToolRegistry>,
        permission_manager: Arc<PermissionManager>,
    ) -> Self {
        Self {
            backend,
            tool_registry,
            permission_manager,
            event_tx: None,
            tool_call_id: None,
            parent_conversation_id: None,
        }
    }

    pub fn with_event_sender(mut self, tx: mpsc::UnboundedSender<AgentEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    pub fn with_tool_call_id(mut self, id: String) -> Self {
        self.tool_call_id = Some(id);
        self
    }

    pub fn with_parent_conversation_id(mut self, id: String) -> Self {
        self.parent_conversation_id = Some(id);
        self
    }

    pub async fn execute_task(&self, task_def: TaskDefinition) -> Result<TaskResult> {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let task_def = task_def.initialize_budget();
        let budget_arc = task_def
            .budget
            .as_ref()
            .map(|b| Arc::new(b.clone()))
            .expect("Budget should be defined");

        // The tool_registry passed to TaskManager is already the subagent registry
        // (without task tool) to prevent infinite recursion
        let tool_executor = Arc::new(
            ToolExecutor::new(
                Arc::clone(&self.tool_registry),
                Arc::clone(&self.permission_manager),
            )
            .with_event_sender(event_tx.clone()),
        );

        let mut agent = Agent::new(
            self.backend.clone(),
            self.tool_registry.clone(),
            tool_executor,
        )
        .with_max_steps(task_def.agent_type.max_steps())
        .with_event_sender(event_tx);

        agent = agent.with_execution_budget(budget_arc.clone());

        let conversation_storage = Arc::new(ConversationStorage::with_default_path()?);

        let mut conversation = if let (Some(parent_id), Some(tool_call_id)) =
            (&self.parent_conversation_id, &self.tool_call_id)
        {
            Conversation::with_subagent_storage(parent_id, tool_call_id, conversation_storage)?
        } else {
            Conversation::new()
        };
        let system_message = task_def
            .agent_type
            .system_message(&task_def.prompt, task_def.budget.as_ref());
        conversation.add_user_message(system_message);

        let parent_event_tx = self.event_tx.clone();
        let tool_call_id = self.tool_call_id.clone();

        let event_collector = tokio::spawn(async move {
            let mut collected_events = Vec::new();
            let mut current_step = 0;

            while let Some(event) = event_rx.recv().await {
                // Track the actual step number from StepStarted events
                if let AgentEvent::StepStarted { step } = event {
                    current_step = step;
                }

                let event_string = format!("{:?}", event);
                collected_events.push(TaskEvent {
                    event_type: event_string
                        .split('(')
                        .next()
                        .unwrap_or("Unknown")
                        .to_string(),
                    message: event_string,
                    timestamp: std::time::SystemTime::now(),
                });

                if let (Some(tx), Some(tcid)) = (&parent_event_tx, &tool_call_id)
                    && should_emit_to_parent(&event)
                {
                    if let Ok(progress_event) =
                        transform_to_subagent_event(&event, tcid, current_step, budget_arc.clone())
                    {
                        let _ = tx.send(progress_event);
                    }
                }
            }
            (collected_events, current_step)
        });

        let execute_result = if let Some(timeout_secs) = task_def.timeout_seconds {
            tokio::time::timeout(
                tokio::time::Duration::from_secs(timeout_secs),
                agent.handle_turn(&mut conversation),
            )
            .await
        } else {
            Ok(agent.handle_turn(&mut conversation).await)
        };

        drop(agent);

        let (events, final_step) = event_collector.await.unwrap_or_else(|_| (Vec::new(), 0));

        let total_steps = final_step + 1;

        if let (Some(tx), Some(tcid)) = (&self.event_tx, &self.tool_call_id) {
            let _ = tx.send(AgentEvent::SubagentTaskComplete {
                tool_call_id: tcid.clone(),
                total_steps,
            });
        }

        let budget_info = task_def
            .budget
            .as_ref()
            .map(|b| crate::task_management::BudgetInfo {
                elapsed_seconds: b.elapsed_seconds(),
                remaining_seconds: b.remaining_seconds(),
                total_steps,
                max_steps: task_def.agent_type.max_steps(),
            });

        match execute_result {
            Ok(Ok(())) => {
                let final_response = conversation
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "assistant" && m.content.is_some())
                    .and_then(|m| m.content.clone())
                    .unwrap_or_else(|| "Task completed without final message".to_string());

                let mut result = TaskResult::success(final_response).with_events(events);
                if let Some(info) = budget_info {
                    result = result.with_budget_info(info);
                }
                Ok(result)
            }
            Ok(Err(e)) => {
                let mut result =
                    TaskResult::failure(format!("Task failed: {}", e)).with_events(events);
                if let Some(info) = budget_info {
                    result = result.with_budget_info(info);
                }
                Ok(result)
            }
            Err(_) => {
                let mut result =
                    TaskResult::failure("Task timed out".to_string()).with_events(events);
                if let Some(info) = budget_info {
                    result = result.with_budget_info(info);
                }
                Ok(result)
            }
        }
    }
}

fn should_emit_to_parent(event: &AgentEvent) -> bool {
    matches!(
        event,
        AgentEvent::AssistantThought(_)
            | AgentEvent::ToolExecutionStarted { .. }
            | AgentEvent::ToolExecutionCompleted { .. }
            | AgentEvent::ToolResult { .. }
    )
}

fn transform_to_subagent_event(
    event: &AgentEvent,
    tool_call_id: &str,
    step_number: usize,
    budget: Arc<ExecutionBudget>,
) -> Result<AgentEvent, String> {
    let (action_type, description) = match event {
        AgentEvent::AssistantThought(content) => {
            let preview = if content.len() > 50 {
                format!("{}...", &content[..50])
            } else {
                content.clone()
            };
            ("thinking", preview)
        }
        AgentEvent::ToolExecutionStarted { tool_name, .. } => {
            ("tool_starting", format!("Executing {}", tool_name))
        }
        AgentEvent::ToolExecutionCompleted { tool_name, .. } => {
            ("tool_completed", format!("Completed {}", tool_name))
        }
        AgentEvent::ToolResult { summary, .. } => {
            let preview = if summary.len() > 50 {
                format!("{}...", &summary[..50])
            } else {
                summary.clone()
            };
            ("tool_result", preview)
        }
        _ => return Err("Event not bridged".to_string()),
    };

    let budget_pct = budget.percentage_used(step_number);

    Ok(AgentEvent::SubagentStepProgress {
        tool_call_id: tool_call_id.to_string(),
        step_number,
        action_type: action_type.to_string(),
        description,
        timestamp: std::time::SystemTime::now(),
        budget_pct,
    })
}

#[cfg(test)]
#[path = "task_manager_tests.rs"]
mod tests;
