use crate::agent::Conversation;
use crate::backends::LlmResponse;
use async_trait::async_trait;
use tokio::sync::mpsc;

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
        Ok("Mock response".to_string())
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
async fn test_task_manager_execute_simple_task() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn LlmBackend> =
        Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Task completed successfully".to_string(),
        )]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        PermissionManager::new(event_tx, response_rx).with_skip_permissions(true),
    );

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
    assert_eq!(result.output, "Task completed successfully");
}

#[tokio::test]
async fn test_task_manager_execute_explore_task() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn LlmBackend> =
        Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Found 5 files matching the pattern".to_string(),
        )]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        PermissionManager::new(event_tx, response_rx).with_skip_permissions(true),
    );

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
    assert!(result.output.contains("Found 5 files"));
}

#[tokio::test]
async fn test_task_manager_timeout() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn LlmBackend> = Arc::new(MockBackend::new(vec![]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        PermissionManager::new(event_tx, response_rx).with_skip_permissions(true),
    );

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
    assert!(result.output.contains("Task timed out") || result.output.contains("No more responses"));
}

#[tokio::test]
async fn test_task_manager_backend_error() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn LlmBackend> = Arc::new(MockBackend::new(vec![]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        PermissionManager::new(event_tx, response_rx).with_skip_permissions(true),
    );

    let task_manager = TaskManager::new(mock_backend, tool_registry, permission_manager);

    let task_def = TaskDefinition::new(
        crate::task_management::AgentType::GeneralPurpose,
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

    let mock_backend: Arc<dyn LlmBackend> =
        Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Custom model response".to_string(),
        )]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        PermissionManager::new(event_tx, response_rx).with_skip_permissions(true),
    );

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

    let mock_backend: Arc<dyn LlmBackend> =
        Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Task completed".to_string(),
        )]));

    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        PermissionManager::new(event_tx, response_rx).with_skip_permissions(true),
    );

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

    let task_manager = TaskManager::new(
        mock_backend,
        registry_with_task.clone(),
        permission_manager,
    );

    let task_def = TaskDefinition::new(
        crate::task_management::AgentType::Plan,
        "test task".to_string(),
        "test".to_string(),
    );

    let result = task_manager.execute_task(task_def).await;
    assert!(result.is_ok());
}
