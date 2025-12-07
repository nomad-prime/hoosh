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
    ) -> Result<LlmResponse, crate::backends::LlmError> {
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
    ) -> Result<LlmResponse, crate::backends::LlmError> {
        Err(crate::backends::LlmError::Other {
            message: "Simulated backend error".to_string(),
        })
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
    .with_timeout(1); // 1 second timeout, but backend takes 10 seconds

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
async fn test_task_manager_uses_subagent_registry() {
    use crate::tools::SubAgentToolProvider;
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn LlmBackend> = Arc::new(MockBackend::new());

    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager =
        Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));

    // Create a subagent registry (without task tool to prevent recursion)
    let subagent_registry = Arc::new(ToolRegistry::new().with_provider(Arc::new(
        SubAgentToolProvider::new(std::path::PathBuf::from(".")),
    )));

    // Verify task tool is NOT in subagent registry
    assert!(subagent_registry.get_tool("task").is_none());
    // But other tools should be present
    assert!(subagent_registry.get_tool("read_file").is_some());

    let task_manager = TaskManager::new(mock_backend, subagent_registry, permission_manager);

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
                total_tool_uses,
                total_input_tokens,
                total_output_tokens,
            } => {
                assert_eq!(tool_call_id, "test-task-123");
                assert!(total_steps > 0);
                // Token and tool use counts may be 0 in tests
                let _ = (total_tool_uses, total_input_tokens, total_output_tokens);
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
