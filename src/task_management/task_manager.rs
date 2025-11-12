use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agent::{Agent, AgentEvent, Conversation};
use crate::backends::LlmBackend;
use crate::permissions::PermissionManager;
use crate::task_management::{TaskDefinition, TaskEvent, TaskResult};
use crate::tool_executor::ToolExecutor;
use crate::tools::ToolRegistry;

pub struct TaskManager {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
    event_tx: Option<mpsc::UnboundedSender<AgentEvent>>,
    tool_call_id: Option<String>,
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

        let parent_event_tx = self.event_tx.clone();
        let tool_call_id = self.tool_call_id.clone();

        let event_collector = tokio::spawn(async move {
            let mut collected_events = Vec::new();
            let mut step_count = 0;

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

                if let (Some(tx), Some(tcid)) = (&parent_event_tx, &tool_call_id)
                    && should_emit_to_parent(&event)
                {
                    step_count += 1;
                    if let Ok(progress_event) =
                        transform_to_subagent_event(&event, tcid, step_count)
                    {
                        let _ = tx.send(progress_event);
                    }
                }
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

        if let (Some(tx), Some(tcid)) = (&self.event_tx, &self.tool_call_id) {
            let _ = tx.send(AgentEvent::SubagentTaskComplete {
                tool_call_id: tcid.clone(),
                total_steps: events.len(),
            });
        }

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

    Ok(AgentEvent::SubagentStepProgress {
        tool_call_id: tool_call_id.to_string(),
        step_number,
        action_type: action_type.to_string(),
        description,
        timestamp: std::time::SystemTime::now(),
    })
}

#[cfg(test)]
mod tests {
    use crate::agent::Conversation;
    use crate::backends::LlmResponse;
    use crate::backends::MockBackend;
    use crate::task_management::{TaskDefinition, TaskManager};
    use crate::{LlmBackend, PermissionManager, ToolRegistry};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    // Mock backend that delays to test timeout
    struct DelayedMockBackend;

    #[async_trait]
    impl LlmBackend for DelayedMockBackend {
        async fn send_message(&self, message: &str) -> Result<String> {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            Ok(format!("Delayed response to: {}", message))
        }

        async fn send_message_with_tools(
            &self,
            _conversation: &Conversation,
            _tools: &ToolRegistry,
        ) -> Result<LlmResponse> {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            Ok(LlmResponse::content_only("Delayed response".to_string()))
        }

        fn backend_name(&self) -> &str {
            "delayed-mock"
        }

        fn model_name(&self) -> &str {
            "delayed-mock-model"
        }
    }

    // Mock backend that returns errors
    struct ErrorMockBackend;

    #[async_trait]
    impl LlmBackend for ErrorMockBackend {
        async fn send_message(&self, _message: &str) -> Result<String> {
            anyhow::bail!("Simulated backend error")
        }

        async fn send_message_with_tools(
            &self,
            _conversation: &Conversation,
            _tools: &ToolRegistry,
        ) -> Result<LlmResponse> {
            anyhow::bail!("Simulated backend error")
        }

        fn backend_name(&self) -> &str {
            "error-mock"
        }

        fn model_name(&self) -> &str {
            "error-mock-model"
        }
    }

    #[tokio::test]
    async fn test_task_manager_execute_simple_task() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let mock_backend: Arc<dyn LlmBackend> = Arc::new(MockBackend::new());

        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));

        let task_manager = TaskManager::new(mock_backend, tool_registry, permission_manager);

        let task_def = TaskDefinition::new(
            crate::task_management::AgentType::Plan,
            "analyze the code".to_string(),
            "code analysis".to_string(),
        );

        let result = task_manager.execute_task(task_def).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        // MockBackend echoes the system message, so check it contains the task prompt
        assert!(result.output.contains("analyze the code"));
    }

    #[tokio::test]
    async fn test_task_manager_execute_explore_task() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let mock_backend: Arc<dyn LlmBackend> = Arc::new(MockBackend::new());

        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));

        let task_manager = TaskManager::new(mock_backend, tool_registry, permission_manager);

        let task_def = TaskDefinition::new(
            crate::task_management::AgentType::Explore,
            "find all rust files".to_string(),
            "find rust files".to_string(),
        );

        let result = task_manager.execute_task(task_def).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
        // MockBackend echoes the system message, so check it contains the task prompt
        assert!(result.output.contains("find all rust files"));
    }

    #[tokio::test]
    async fn test_task_manager_timeout() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        // Use DelayedMockBackend that takes 10 seconds, with timeout of 1 second
        let mock_backend: Arc<dyn LlmBackend> = Arc::new(DelayedMockBackend);

        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));

        let task_manager = TaskManager::new(mock_backend, tool_registry, permission_manager);

        let task_def = TaskDefinition::new(
            crate::task_management::AgentType::Plan,
            "long running task".to_string(),
            "long task".to_string(),
        )
        .with_timeout(1);

        let result = task_manager.execute_task(task_def).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Task timed out"));
    }

    #[tokio::test]
    async fn test_task_manager_backend_error() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        // Use ErrorMockBackend that always returns errors
        let mock_backend: Arc<dyn LlmBackend> = Arc::new(ErrorMockBackend);

        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));

        let task_manager = TaskManager::new(mock_backend, tool_registry, permission_manager);

        let task_def = TaskDefinition::new(
            crate::task_management::AgentType::Plan,
            "task that will fail".to_string(),
            "failing task".to_string(),
        );

        let result = task_manager.execute_task(task_def).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Task failed"));
    }

    #[tokio::test]
    async fn test_task_manager_with_custom_model() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let mock_backend: Arc<dyn LlmBackend> = Arc::new(MockBackend::new());

        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));

        let task_manager = TaskManager::new(mock_backend, tool_registry, permission_manager);

        let task_def = TaskDefinition::new(
            crate::task_management::AgentType::Plan,
            "task with custom model".to_string(),
            "custom model task".to_string(),
        )
        .with_model("gpt-4".to_string());

        let result = task_manager.execute_task(task_def).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_task_manager_filters_task_tool() {
        use crate::tools::TaskToolProvider;
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let mock_backend: Arc<dyn LlmBackend> = Arc::new(MockBackend::new());

        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));

        let tool_registry = Arc::new(ToolRegistry::new());
        let task_provider = Arc::new(TaskToolProvider::new(
            mock_backend.clone(),
            tool_registry.clone(),
            permission_manager.clone(),
        ));
        let mut registry_with_task = (*tool_registry).clone();
        registry_with_task.add_provider(task_provider);
        let registry_with_task = Arc::new(registry_with_task);

        assert!(registry_with_task.get_tool("task").is_some());

        let task_manager =
            TaskManager::new(mock_backend, registry_with_task.clone(), permission_manager);

        let task_def = TaskDefinition::new(
            crate::task_management::AgentType::Plan,
            "test task".to_string(),
            "test".to_string(),
        );

        let result = task_manager.execute_task(task_def).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_task_manager_bridges_subagent_events() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let mock_backend: Arc<dyn LlmBackend> = Arc::new(MockBackend::new());
        let tool_registry = Arc::new(ToolRegistry::new());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));

        let (parent_tx, mut parent_rx) = mpsc::unbounded_channel();

        let task_manager = TaskManager::new(mock_backend, tool_registry, permission_manager)
            .with_event_sender(parent_tx)
            .with_tool_call_id("test-task-123".to_string());

        let task_def = TaskDefinition::new(
            crate::task_management::AgentType::Plan,
            "test task for event bridging".to_string(),
            "test".to_string(),
        );

        tokio::spawn(async move {
            let _ = task_manager.execute_task(task_def).await;
        });

        let mut found_progress = false;
        let mut found_complete = false;

        while let Some(event) = parent_rx.recv().await {
            match event {
                crate::agent::AgentEvent::SubagentStepProgress {
                    tool_call_id,
                    step_number,
                    action_type,
                    description,
                    ..
                } => {
                    assert_eq!(tool_call_id, "test-task-123");
                    assert!(step_number > 0);
                    assert!(!action_type.is_empty());
                    assert!(!description.is_empty());
                    found_progress = true;
                }
                crate::agent::AgentEvent::SubagentTaskComplete {
                    tool_call_id,
                    total_steps,
                } => {
                    assert_eq!(tool_call_id, "test-task-123");
                    assert!(total_steps > 0);
                    found_complete = true;
                    break;
                }
                _ => {}
            }
        }

        assert!(
            found_progress || found_complete,
            "Should receive subagent events"
        );
    }
}
