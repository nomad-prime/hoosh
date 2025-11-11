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
        let args: TaskArgs = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments {
                tool: "task".to_string(),
                message: e.to_string(),
            }
        })?;

        let agent_type = AgentType::from_str(&args.subagent_type).map_err(|e| {
            ToolError::InvalidArguments {
                tool: "task".to_string(),
                message: e.to_string(),
            }
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
        - explore: Quickly searches and understands codebases (max 30 steps)\n\
        - general-purpose: Handles complex multi-step tasks with full tool access (max 100 steps)"
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subagent_type": {
                    "type": "string",
                    "enum": ["plan", "explore", "general-purpose"],
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
            format!("Task[{}]({})", parsed_args.subagent_type, parsed_args.description)
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

    include!("task_tool_tests.rs");
}
