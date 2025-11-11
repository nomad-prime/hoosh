use crate::agent::Conversation;
use crate::backends::LlmResponse;
use async_trait::async_trait;
use serde_json::json;
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
impl crate::backends::LlmBackend for MockBackend {
    async fn send_message(&self, _message: &str) -> anyhow::Result<String> {
        Ok("Mock response".to_string())
    }

    async fn send_message_with_tools(
        &self,
        _conversation: &Conversation,
        _tools: &ToolRegistry,
    ) -> anyhow::Result<LlmResponse> {
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
async fn test_task_tool_execute_plan() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Plan created successfully".to_string(),
        )]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(
        mock_backend,
        tool_registry.clone(),
        permission_manager.clone(),
    );

    let args = json!({
        "subagent_type": "plan",
        "prompt": "Create a plan to implement feature X",
        "description": "Feature X planning"
    });

    let result = task_tool.execute(&args).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Plan created successfully");
}

#[tokio::test]
async fn test_task_tool_execute_explore() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Found 10 matching files".to_string(),
        )]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);

    let args = json!({
        "subagent_type": "explore",
        "prompt": "Find all Rust files in the project",
        "description": "Find Rust files"
    });

    let result = task_tool.execute(&args).await;
    assert!(result.is_ok());
    assert!(result.unwrap().contains("matching files"));
}

#[tokio::test]
async fn test_task_tool_execute_general_purpose() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Complex task completed".to_string(),
        )]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);

    let args = json!({
        "subagent_type": "general-purpose",
        "prompt": "Perform complex analysis and refactoring",
        "description": "Complex refactoring"
    });

    let result = task_tool.execute(&args).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_task_tool_invalid_agent_type() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);

    let args = json!({
        "subagent_type": "invalid_type",
        "prompt": "Some task",
        "description": "Test task"
    });

    let result = task_tool.execute(&args).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ToolError::InvalidArguments { .. }));
}

#[tokio::test]
async fn test_task_tool_missing_required_args() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);

    let args = json!({
        "subagent_type": "plan"
    });

    let result = task_tool.execute(&args).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ToolError::InvalidArguments { .. }));
}

#[tokio::test]
async fn test_task_tool_with_custom_model() {
    crate::console::init_console(crate::console::VerbosityLevel::Quiet);

    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![LlmResponse::content_only(
            "Response from custom model".to_string(),
        )]));

    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);

    let args = json!({
        "subagent_type": "plan",
        "prompt": "Create a plan",
        "description": "Planning task",
        "model": "gpt-4"
    });

    let result = task_tool.execute(&args).await;
    assert!(result.is_ok());
}

#[test]
fn test_task_tool_name() {
    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![]));
    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);
    assert_eq!(task_tool.name(), "task");
    assert_eq!(task_tool.display_name(), "Task");
}

#[test]
fn test_task_tool_description() {
    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![]));
    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);
    let description = task_tool.description();
    assert!(description.contains("specialized sub-agent"));
    assert!(description.contains("plan"));
    assert!(description.contains("explore"));
    assert!(description.contains("general-purpose"));
}

#[test]
fn test_task_tool_parameter_schema() {
    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![]));
    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);
    let schema = task_tool.parameter_schema();

    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["subagent_type"].is_object());
    assert!(schema["properties"]["prompt"].is_object());
    assert!(schema["properties"]["description"].is_object());
    assert!(schema["properties"]["model"].is_object());

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("subagent_type")));
    assert!(required.contains(&json!("prompt")));
    assert!(required.contains(&json!("description")));
}

#[test]
fn test_task_tool_format_call_display() {
    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![]));
    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);

    let args = json!({
        "subagent_type": "plan",
        "prompt": "Create a plan",
        "description": "Planning task"
    });

    let display = task_tool.format_call_display(&args);
    assert_eq!(display, "Task[plan](Planning task)");
}

#[test]
fn test_task_tool_result_summary() {
    let mock_backend: Arc<dyn crate::backends::LlmBackend> =
        Arc::new(MockBackend::new(vec![]));
    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let (_, response_rx) = mpsc::unbounded_channel();
    let permission_manager = Arc::new(
        crate::permissions::PermissionManager::new(event_tx, response_rx)
            .with_skip_permissions(true),
    );

    let task_tool = TaskTool::new(mock_backend, tool_registry, permission_manager);

    let short_result = "Short output";
    assert_eq!(task_tool.result_summary(short_result), "Short output");

    let long_result = "A".repeat(150);
    let summary = task_tool.result_summary(&long_result);
    assert!(summary.len() <= 103);
    assert!(summary.ends_with("..."));
}
