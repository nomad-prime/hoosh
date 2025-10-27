use anyhow::{Context, Result};
use serde_json::{self, Value};
use tokio::sync::mpsc;

use crate::conversations::{AgentEvent, ToolCall, ToolResult};
use crate::permissions::PermissionManager;
use crate::tools::{BuiltinToolProvider, ToolRegistry};

/// Validate arguments against a JSON schema
/// Returns an error if validation fails
fn validate_against_schema(args: &Value, schema: &Value, tool_name: &str) -> Result<()> {
    let compiled_schema = jsonschema::JSONSchema::compile(schema)
        .map_err(|e| anyhow::anyhow!("Failed to compile schema for tool '{}': {}", tool_name, e))?;

    compiled_schema.validate(args).map_err(|e| {
        let errors: Vec<String> = e.map(|err| err.to_string()).collect();
        anyhow::anyhow!(
            "Tool '{}' arguments do not match schema: {}",
            tool_name,
            errors.join("; ")
        )
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
            std::sync::Mutex<mpsc::UnboundedReceiver<crate::conversations::ApprovalResponse>>,
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
        self.approval_receiver = Some(std::sync::Arc::new(std::sync::Mutex::new(receiver)));
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
                    tool_name.clone(),
                    anyhow::anyhow!("Invalid tool arguments: {}", e),
                );
            }
        };

        // Validate arguments against the tool's schema
        let schema = tool.parameter_schema();
        if let Err(e) = validate_against_schema(&args, &schema, tool_name) {
            return ToolResult::error(tool_call_id, tool_name.clone(), tool_name.clone(), e);
        }

        // Get the display name from the tool
        let display_name = tool.format_call_display(&args);

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

    /// Request user approval for a tool execution
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
            let receiver_clone = std::sync::Arc::clone(receiver);
            let response = loop {
                // Try to receive in a block that drops the lock immediately
                let maybe_response = {
                    let mut rx = receiver_clone
                        .lock()
                        .map_err(|e| anyhow::anyhow!("Failed to lock receiver: {}", e))?;
                    rx.try_recv().ok()
                };

                if let Some(response) = maybe_response {
                    // Verify tool_call_id matches
                    if response.tool_call_id == tool_call_id {
                        break response;
                    }
                }

                // Small sleep to avoid busy-waiting
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            };

            if !response.approved {
                // Send UserRejection event to signal that user rejected the operation
                if let Some(sender) = &self.event_sender {
                    let _ = sender.send(AgentEvent::UserRejection);
                }
                let reason = response
                    .rejection_reason
                    .unwrap_or_else(|| "User rejected".to_string());
                anyhow::bail!("Operation rejected: {}", reason);
            }
        }

        Ok(())
    }

    /// Check if a tool execution is permitted
    /// Delegates to the tool's own permission check implementation
    async fn check_tool_permissions(
        &self,
        tool: &dyn crate::tools::Tool,
        args: &serde_json::Value,
    ) -> Result<()> {
        // Skip permission checks if enforcement is disabled
        if !self.permission_manager.is_enforcing() {
            return Ok(());
        }

        // Ask the tool to check its own permissions
        let allowed = tool
            .check_permission(args, &self.permission_manager)
            .await?;

        if !allowed {
            anyhow::bail!("Permission denied for {} operation", tool.tool_name());
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
