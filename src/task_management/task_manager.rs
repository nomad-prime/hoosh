use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::{Agent, Conversation};
use crate::backends::LlmBackend;
use crate::permissions::PermissionManager;
use crate::task_management::{TaskDefinition, TaskEvent, TaskResult};
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

pub struct TaskManager {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
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
        }
    }

    pub async fn execute_task(&self, task_def: TaskDefinition) -> Result<TaskResult> {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let sub_agent_registry = Arc::new(self.tool_registry.without("task"));

        let tool_executor = Arc::new(
            ToolExecutor::new(
                (*sub_agent_registry).clone(),
                (*self.permission_manager).clone(),
            )
            .with_event_sender(event_tx.clone()),
        );

        let agent = Agent::new(self.backend.clone(), sub_agent_registry, tool_executor)
            .with_max_steps(task_def.agent_type.max_steps())
            .with_event_sender(event_tx);

        let mut conversation = Conversation::new();
        let system_message = task_def.agent_type.system_message(&task_def.prompt);
        conversation.add_user_message(system_message);

        let event_collector = tokio::spawn(async move {
            let mut collected_events = Vec::new();
            while let Some(event) = event_rx.recv().await {
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
            }
            collected_events
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

        let events = event_collector.await.unwrap_or_else(|_| Vec::new());

        match execute_result {
            Ok(Ok(())) => {
                let final_response = conversation
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "assistant" && m.content.is_some())
                    .and_then(|m| m.content.clone())
                    .unwrap_or_else(|| "Task completed without final message".to_string());

                Ok(TaskResult::success(final_response).with_events(events))
            }
            Ok(Err(e)) => {
                Ok(TaskResult::failure(format!("Task failed: {}", e)).with_events(events))
            }
            Err(_) => Ok(TaskResult::failure("Task timed out".to_string()).with_events(events)),
        }
    }
}

#[cfg(test)]
mod tests {
    include!("task_manager_tests.rs");
}
