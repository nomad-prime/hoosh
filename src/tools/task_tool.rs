use crate::backends::LlmBackend;
use crate::permissions::{PermissionManager, ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::task_management::{AgentType, TaskDefinition, TaskManager};
use crate::tools::{Tool, ToolError, ToolRegistry, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct TaskTool {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
}

impl TaskTool {
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

    async fn execute_impl(&self, args: &Value) -> ToolResult<String> {
        let args: TaskArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "task".to_string(),
                message: e.to_string(),
            })?;

        let agent_type =
            AgentType::from_name(&args.subagent_type).map_err(|e| ToolError::InvalidArguments {
                tool: "task".to_string(),
                message: e.to_string(),
            })?;

        let mut task_def = TaskDefinition::new(agent_type, args.prompt, args.description);

        if let Some(model) = args.model {
            task_def = task_def.with_model(model);
        }

        let task_manager = TaskManager::new(
            self.backend.clone(),
            self.tool_registry.clone(),
            self.permission_manager.clone(),
        );

        let result = task_manager
            .execute_task(task_def)
            .await
            .map_err(|e| ToolError::execution_failed(e.to_string()))?;

        if result.success {
            Ok(result.output)
        } else {
            Err(ToolError::execution_failed(result.output))
        }
    }
}

#[derive(Deserialize)]
struct TaskArgs {
    subagent_type: String,
    prompt: String,
    description: String,
    #[serde(default)]
    model: Option<String>,
}

#[async_trait]
impl Tool for TaskTool {
    async fn execute(&self, args: &Value) -> ToolResult<String> {
        self.execute_impl(args).await
    }

    fn name(&self) -> &'static str {
        "task"
    }

    fn display_name(&self) -> &'static str {
        "Task"
    }

    fn description(&self) -> &'static str {
        "Launch a specialized sub-agent to handle complex tasks autonomously. \
        Available agent types:\n\
        - plan: Analyzes codebases and creates implementation plans (max 50 steps)\n\
        - explore: Quickly searches and understands codebases (max 30 steps)"
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subagent_type": {
                    "type": "string",
                    "enum": ["plan", "explore"],
                    "description": "The type of specialized agent to use"
                },
                "prompt": {
                    "type": "string",
                    "description": "The task for the agent to perform autonomously"
                },
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "model": {
                    "type": "string",
                    "description": "Optional model to use (inherits from parent if not specified)"
                }
            },
            "required": ["subagent_type", "prompt", "description"]
        })
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(parsed_args) = serde_json::from_value::<TaskArgs>(args.clone()) {
            format!(
                "Task[{}]({})",
                parsed_args.subagent_type, parsed_args.description
            )
        } else {
            "Task(?)".to_string()
        }
    }

    fn result_summary(&self, result: &str) -> String {
        let preview_len = result.len().min(100);
        if result.len() > 100 {
            format!("{}...", &result[..preview_len])
        } else {
            result.to_string()
        }
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        ToolPermissionBuilder::new(self, target.unwrap_or("*"))
            .into_destructive()
            .with_display_name("Task")
            .build()
            .expect("Failed to build TaskTool permission descriptor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    impl LlmBackend for MockBackend {
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
        let permission_manager =
            Arc::new(PermissionManager::new(event_tx, response_rx).with_skip_permissions(true));

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
    async fn test_task_tool_invalid_agent_type() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let mock_backend: Arc<dyn crate::backends::LlmBackend> = Arc::new(MockBackend::new(vec![]));

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
        assert!(matches!(
            result.unwrap_err(),
            ToolError::InvalidArguments { .. }
        ));
    }

    #[tokio::test]
    async fn test_task_tool_missing_required_args() {
        crate::console::init_console(crate::console::VerbosityLevel::Quiet);

        let mock_backend: Arc<dyn crate::backends::LlmBackend> = Arc::new(MockBackend::new(vec![]));

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
        assert!(matches!(
            result.unwrap_err(),
            ToolError::InvalidArguments { .. }
        ));
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
        let mock_backend: Arc<dyn crate::backends::LlmBackend> = Arc::new(MockBackend::new(vec![]));
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
        let mock_backend: Arc<dyn crate::backends::LlmBackend> = Arc::new(MockBackend::new(vec![]));
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
    }

    #[test]
    fn test_task_tool_parameter_schema() {
        let mock_backend: Arc<dyn crate::backends::LlmBackend> = Arc::new(MockBackend::new(vec![]));
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
        let mock_backend: Arc<dyn crate::backends::LlmBackend> = Arc::new(MockBackend::new(vec![]));
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
        let mock_backend: Arc<dyn crate::backends::LlmBackend> = Arc::new(MockBackend::new(vec![]));
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
}
