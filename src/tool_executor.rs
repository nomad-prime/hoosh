use anyhow::Result;
use serde_json;

use crate::conversation::{ToolCall, ToolResult};
use crate::permissions::PermissionManager;
use crate::tools::{BashTool, ListDirectoryTool, ReadFileTool, ToolRegistry, WriteFileTool};

/// Handles execution of tool calls
pub struct ToolExecutor {
    tool_registry: ToolRegistry,
    permission_manager: PermissionManager,
}

impl ToolExecutor {
    pub fn new(
        tool_registry: ToolRegistry,
        permission_manager: PermissionManager,
    ) -> Self {
        Self {
            tool_registry,
            permission_manager,
        }
    }

    /// Execute a single tool call
    pub async fn execute_tool_call(&self, tool_call: &ToolCall) -> ToolResult {
        let tool_name = &tool_call.function.name;
        let tool_call_id = tool_call.id.clone();

        // Get the tool from registry
        let tool = match self.tool_registry.get_tool(tool_name) {
            Some(tool) => tool,
            None => {
                return ToolResult::error(
                    tool_call_id,
                    tool_name.clone(),
                    anyhow::anyhow!("Unknown tool: {}", tool_name),
                );
            }
        };

        // Parse arguments
        let args = match serde_json::from_str(&tool_call.function.arguments) {
            Ok(args) => args,
            Err(e) => {
                return ToolResult::error(
                    tool_call_id,
                    tool_name.clone(),
                    anyhow::anyhow!("Invalid tool arguments: {}", e),
                );
            }
        };

        // Check permissions
        if let Err(e) = self.check_tool_permissions(tool_name, &args).await {
            return ToolResult::error(tool_call_id, tool_name.clone(), e);
        }

        // Execute the tool
        match tool.execute(&args).await {
            Ok(output) => ToolResult::success(tool_call_id, tool_name.clone(), output),
            Err(e) => ToolResult::error(tool_call_id, tool_name.clone(), e),
        }
    }

    /// Execute multiple tool calls and return results
    pub async fn execute_tool_calls(&self, tool_calls: &[ToolCall]) -> Vec<ToolResult> {
        let mut results = Vec::new();

        for tool_call in tool_calls {
            let result = self.execute_tool_call(tool_call).await;
            results.push(result);
        }

        results
    }

    /// Check if a tool execution is permitted
    async fn check_tool_permissions(&self, tool_name: &str, args: &serde_json::Value) -> Result<()> {
        // Skip permission checks if enforcement is disabled
        if !self.permission_manager.is_enforcing() {
            return Ok(());
        }

        let allowed = match tool_name {
            "read_file" => {
                if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                    self.permission_manager.request_file_read(path).await?
                } else {
                    false
                }
            }
            "write_file" => {
                if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                    self.permission_manager.request_file_write(path).await?
                } else {
                    false
                }
            }
            "list_directory" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                self.permission_manager.request_directory_list(path).await?
            }
            "bash" => {
                if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
                    self.permission_manager.request_bash_execution(command).await?
                } else {
                    false
                }
            }
            _ => {
                // Unknown tool - let it through but log a warning
                eprintln!("⚠️  Unknown tool for permission check: {}", tool_name);
                true
            }
        };

        if !allowed {
            anyhow::bail!("Permission denied for {} operation", tool_name);
        }

        Ok(())
    }

    /// Create tools with the correct working directory
    pub fn create_tool_registry_with_working_dir(working_dir: std::path::PathBuf) -> ToolRegistry {
        ToolRegistry::new()
            .with_tool(std::sync::Arc::new(
                ReadFileTool::with_working_directory(working_dir.clone()),
            ))
            .with_tool(std::sync::Arc::new(
                WriteFileTool::with_working_directory(working_dir.clone()),
            ))
            .with_tool(std::sync::Arc::new(
                ListDirectoryTool::with_working_directory(working_dir.clone()),
            ))
            .with_tool(std::sync::Arc::new(
                BashTool::with_working_directory(working_dir),
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::{ToolCall, ToolFunction};
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_execute_read_file_tool() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, "Hello, World!").await.unwrap();

        let tool_registry = ToolExecutor::create_tool_registry_with_working_dir(
            temp_dir.path().to_path_buf()
        );
        let permission_manager = PermissionManager::new().with_skip_permissions(true);
        let executor = ToolExecutor::new(
            tool_registry,
            permission_manager,
        );

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: json!({"path": "test.txt"}).to_string(),
            },
        };

        let result = executor.execute_tool_call(&tool_call).await;
        assert!(result.result.is_ok());
        assert!(result.result.unwrap().contains("Hello, World!"));
    }

    #[tokio::test]
    async fn test_execute_unknown_tool() {
        let temp_dir = tempdir().unwrap();
        let tool_registry = ToolExecutor::create_tool_registry_with_working_dir(
            temp_dir.path().to_path_buf()
        );
        let permission_manager = PermissionManager::new();
        let executor = ToolExecutor::new(
            tool_registry,
            permission_manager,
        );

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "unknown_tool".to_string(),
                arguments: "{}".to_string(),
            },
        };

        let result = executor.execute_tool_call(&tool_call).await;
        assert!(result.result.is_err());
        assert!(result.result.unwrap_err().to_string().contains("Unknown tool"));
    }
}