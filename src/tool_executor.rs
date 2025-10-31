use anyhow::{Context, Result};
use serde_json::{self, Value};
use tokio::sync::mpsc;

use crate::conversations::{AgentEvent, ToolCall, ToolExecutionError, ToolResult};
use crate::permissions::PermissionManager;
use crate::tools::{BuiltinToolProvider, ToolRegistry};

/// Validate arguments against a JSON schema
/// Returns an error if validation fails
fn validate_against_schema(args: &Value, schema: &Value, tool_name: &str) -> Result<()> {
    let compiled_schema = jsonschema::JSONSchema::compile(schema)
        .map_err(|e| anyhow::anyhow!("Failed to compile schema for tool '{}': {}", tool_name, e))?;

    compiled_schema.validate(args).map_err(|e| {
        let errors: Vec<String> = e.map(|err| err.to_string()).collect();
        let provided_args = args.to_string();
        anyhow::Error::new(ToolExecutionError::invalid_arguments(
            tool_name,
            provided_args,
            errors.join("; "),
        ))
    })?;

    Ok(())
}

/// Handles execution of tool calls
pub struct ToolExecutor {
    tool_registry: ToolRegistry,
    permission_manager: PermissionManager,
    event_sender: Option<mpsc::UnboundedSender<AgentEvent>>,
    autopilot_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    approval_sender: Option<mpsc::UnboundedSender<AgentEvent>>,
    approval_receiver: Option<
        std::sync::Arc<
            tokio::sync::Mutex<mpsc::UnboundedReceiver<crate::conversations::ApprovalResponse>>,
        >,
    >,
}

impl ToolExecutor {
    pub fn new(tool_registry: ToolRegistry, permission_manager: PermissionManager) -> Self {
        Self {
            tool_registry,
            permission_manager,
            event_sender: None,
            autopilot_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            approval_sender: None,
            approval_receiver: None,
        }
    }

    pub fn with_event_sender(mut self, sender: mpsc::UnboundedSender<AgentEvent>) -> Self {
        self.event_sender = Some(sender.clone());
        self.approval_sender = Some(sender);
        self
    }

    pub fn with_autopilot_state(
        mut self,
        autopilot_state: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        self.autopilot_enabled = autopilot_state;
        self
    }

    pub fn with_approval_receiver(
        mut self,
        receiver: mpsc::UnboundedReceiver<crate::conversations::ApprovalResponse>,
    ) -> Self {
        self.approval_receiver = Some(std::sync::Arc::new(tokio::sync::Mutex::new(receiver)));
        self
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
                    tool_name.clone(),
                    anyhow::Error::new(ToolExecutionError::unknown_tool(tool_name)),
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
                    tool_name.clone(),
                    anyhow::anyhow!("Invalid tool arguments: {}", e),
                );
            }
        };

        // Get the display name from the tool (before validation, so we have it even if validation fails)
        let display_name = tool.format_call_display(&args);

        // Validate arguments against the tool's schema
        let schema = tool.parameter_schema();
        if let Err(e) = validate_against_schema(&args, &schema, tool_name) {
            return ToolResult::error(tool_call_id, tool_name.clone(), display_name, e);
        }

        if let Err(e) = self.check_tool_permissions(tool, &args).await {
            return ToolResult::error(tool_call_id, tool_name.clone(), display_name, e);
        }

        // Generate and emit preview if available
        if let Some(preview) = tool.generate_preview(&args).await {
            // Always show the preview in the message stream first
            if let Some(sender) = &self.event_sender {
                let _ = sender.send(AgentEvent::ToolPreview {
                    tool_name: tool_name.clone(),
                    preview: preview.clone(),
                });
            }

            // Check autopilot state atomically
            let is_autopilot = self
                .autopilot_enabled
                .load(std::sync::atomic::Ordering::Relaxed);

            // If not in autopilot mode, request approval before continuing
            if !is_autopilot && let Err(e) = self.request_approval(&tool_call_id, tool_name).await {
                return ToolResult::error(tool_call_id, tool_name.clone(), display_name, e);
            }
        }

        // Execute the tool
        match tool.execute(&args).await {
            Ok(output) => {
                ToolResult::success(tool_call_id, tool_name.clone(), display_name, output)
            }
            Err(e) => ToolResult::error(tool_call_id, tool_name.clone(), display_name, e),
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

    async fn request_approval(&self, tool_call_id: &str, tool_name: &str) -> Result<()> {
        // Send approval request event
        if let Some(sender) = &self.approval_sender {
            let event = AgentEvent::ApprovalRequest {
                tool_call_id: tool_call_id.to_string(),
                tool_name: tool_name.to_string(),
            };
            sender
                .send(event)
                .context("Failed to send approval request event")?;
        } else {
            // No approval system configured, auto-approve
            return Ok(());
        }

        // Wait for response
        if let Some(receiver) = &self.approval_receiver {
            let mut rx = receiver.lock().await;

            let response = rx
                .recv()
                .await
                .ok_or_else(|| anyhow::anyhow!("Approval channel closed"))?;

            // Verify tool_call_id matches
            if response.tool_call_id != tool_call_id {
                anyhow::bail!(
                    "Approval response ID mismatch: expected {}, got {}",
                    tool_call_id,
                    response.tool_call_id
                );
            }

            if !response.approved {
                let reason = response
                    .rejection_reason
                    .unwrap_or_else(|| "User rejected".to_string());
                return Err(anyhow::Error::new(ToolExecutionError::user_rejected(
                    reason,
                )));
            }
        }

        Ok(())
    }

    async fn check_tool_permissions(
        &self,
        tool: &dyn crate::tools::Tool,
        args: &Value,
    ) -> Result<()> {
        if !self.permission_manager.is_enforcing() {
            return Ok(());
        }

        // Extract target from args - use common patterns for file ops and bash
        let target = if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            Some(path)
        } else if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
            Some(command)
        } else {
            None
        };

        // Let the tool describe its own permission requirements
        let descriptor = tool.describe_permission(target);

        let allowed = self
            .permission_manager
            .check_tool_permission(&descriptor)
            .await?;

        if !allowed {
            return Err(anyhow::Error::new(ToolExecutionError::permission_denied(
                tool.tool_name(),
            )));
        }

        Ok(())
    }

    /// Create tools with the correct working directory
    pub fn create_tool_registry_with_working_dir(working_dir: std::path::PathBuf) -> ToolRegistry {
        ToolRegistry::new()
            .with_provider(std::sync::Arc::new(BuiltinToolProvider::new(working_dir)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversations::{ToolCall, ToolFunction};
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_execute_read_file_tool() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, "Hello, World!").await.unwrap();

        let tool_registry =
            ToolExecutor::create_tool_registry_with_working_dir(temp_dir.path().to_path_buf());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            PermissionManager::new(event_tx, response_rx).with_skip_permissions(true);
        let executor = ToolExecutor::new(tool_registry, permission_manager);

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
        let tool_registry =
            ToolExecutor::create_tool_registry_with_working_dir(temp_dir.path().to_path_buf());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager = PermissionManager::new(event_tx, response_rx);
        let executor = ToolExecutor::new(tool_registry, permission_manager);

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
        assert!(
            result
                .result
                .unwrap_err()
                .to_string()
                .contains("Unknown tool")
        );
    }

    #[tokio::test]
    async fn test_execute_read_file_tool_with_invalid_schema() {
        let temp_dir = tempdir().unwrap();
        let tool_registry =
            ToolExecutor::create_tool_registry_with_working_dir(temp_dir.path().to_path_buf());
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let permission_manager =
            PermissionManager::new(event_tx, response_rx).with_skip_permissions(true);
        let executor = ToolExecutor::new(tool_registry, permission_manager);

        let tool_call = ToolCall {
            id: "call_456".to_string(),
            r#type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                arguments: json!({"start_line": "not_a_number"}).to_string(),
            },
        };

        let result = executor.execute_tool_call(&tool_call).await;
        assert!(result.result.is_err());
        let error_msg = result.result.unwrap_err().to_string();
        assert!(
            error_msg.contains("do not match schema") || error_msg.contains("required"),
            "Expected schema validation error, got: {}",
            error_msg
        );
    }
}
