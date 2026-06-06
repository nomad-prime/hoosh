use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::tools::{Tool, ToolError, ToolExecutionContext, ToolResult};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::fs;

pub struct UpdateSessionFileTool;

#[async_trait]
impl Tool for UpdateSessionFileTool {
    fn name(&self) -> &'static str {
        "update_session_file"
    }

    fn display_name(&self) -> &'static str {
        "UpdateSessionFile"
    }

    fn description(&self) -> &'static str {
        "Write a concise summary of this turn to the session memory file. \
        Call this as your LAST tool call each turn — after all work is done, \
        before your final response. The summary is injected at the start of \
        the next turn so you can continue without re-reading the full history."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "Concise summary of this turn: goal, actions taken, outcomes, current state, and next steps.",
                    "minLength": 1
                }
            },
            "required": ["summary"]
        })
    }

    async fn execute(&self, args: &Value, context: &ToolExecutionContext) -> ToolResult<String> {
        let conv_id = context.parent_conversation_id.as_deref().ok_or_else(|| {
            ToolError::ExecutionFailed {
                message: "update_session_file: no conversation ID available in execution context"
                    .to_string(),
            }
        })?;

        let summary = args["summary"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArguments {
                tool: "update_session_file".to_string(),
                message: "missing required field: summary".to_string(),
            })?;

        let memory_dir = std::env::current_dir()
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("update_session_file: {}", e),
            })?
            .join(".hoosh")
            .join("memory")
            .join(conv_id);

        fs::create_dir_all(&memory_dir).map_err(|e| ToolError::ExecutionFailed {
            message: format!("update_session_file: failed to create directory: {}", e),
        })?;

        let summary_path = memory_dir.join("summary.txt");
        fs::write(&summary_path, summary).map_err(|e| ToolError::ExecutionFailed {
            message: format!("update_session_file: failed to write summary: {}", e),
        })?;

        Ok("Session summary written to memory.".to_string())
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        let target_str = target.unwrap_or(".hoosh/memory/<conv_id>/summary.txt");
        ToolPermissionBuilder::new(self, target_str)
            .into_write_safe()
            .build()
            .expect("Failed to build update_session_file permission descriptor")
    }

    fn format_call_display(&self, _args: &Value) -> String {
        "UpdateSessionFile(summary.txt)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolExecutionContext;

    fn make_context(conv_id: Option<&str>) -> ToolExecutionContext {
        ToolExecutionContext {
            tool_call_id: "test-call-id".to_string(),
            event_tx: None,
            parent_conversation_id: conv_id.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_tool_name_is_update_session_file() {
        let tool = UpdateSessionFileTool;
        assert_eq!(tool.name(), "update_session_file");
    }

    #[tokio::test]
    async fn test_tool_returns_error_without_conversation_id() {
        let tool = UpdateSessionFileTool;
        let args = json!({ "summary": "some content" });
        let context = make_context(None);

        let result = tool.execute(&args, &context).await;
        assert!(result.is_err());
    }
}
