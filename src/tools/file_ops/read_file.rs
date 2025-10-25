use crate::permissions::{OperationType, PermissionManager};
use crate::security::PathValidator;
use crate::tools::{Tool, ToolError, ToolResult};
use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::fs;

pub struct ReadFileTool {
    path_validator: PathValidator,
}

impl ReadFileTool {
    pub fn new() -> Self {
        let working_directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            path_validator: PathValidator::new(working_directory),
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            path_validator: PathValidator::new(working_dir),
        }
    }

    async fn execute_impl(&self, args: &Value) -> ToolResult<String> {
        let args: ReadFileArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "read_file".to_string(),
                message: e.to_string(),
            })?;

        let file_path = self
            .path_validator
            .validate_and_resolve(&args.path)
            .map_err(|e| ToolError::SecurityViolation {
                message: e.to_string(),
            })?;

        let content = fs::read_to_string(&file_path)
            .await
            .map_err(|_| ToolError::ReadFailed {
                path: file_path.clone(),
            })?;

        // Handle line-based reading if specified
        if let (Some(start), Some(end)) = (args.start_line, args.end_line) {
            let lines: Vec<&str> = content.lines().collect();
            if start == 0 || start > lines.len() {
                return Err(ToolError::InvalidArguments {
                    tool: "read_file".to_string(),
                    message: format!(
                        "Invalid start_line: {} (file has {} lines)",
                        start,
                        lines.len()
                    ),
                });
            }
            if end > lines.len() {
                return Err(ToolError::InvalidArguments {
                    tool: "read_file".to_string(),
                    message: format!("Invalid end_line: {} (file has {} lines)", end, lines.len()),
                });
            }
            if start > end {
                return Err(ToolError::InvalidArguments {
                    tool: "read_file".to_string(),
                    message: format!(
                        "start_line ({}) cannot be greater than end_line ({})",
                        start, end
                    ),
                });
            }

            let selected_lines = &lines[start - 1..end];
            Ok(selected_lines.join("\n"))
        } else if let Some(start) = args.start_line {
            let lines: Vec<&str> = content.lines().collect();
            if start == 0 || start > lines.len() {
                return Err(ToolError::InvalidArguments {
                    tool: "read_file".to_string(),
                    message: format!(
                        "Invalid start_line: {} (file has {} lines)",
                        start,
                        lines.len()
                    ),
                });
            }
            let selected_lines = &lines[start - 1..];
            Ok(selected_lines.join("\n"))
        } else {
            Ok(content)
        }
    }
}

#[derive(Deserialize)]
struct ReadFileArgs {
    path: String,
    #[serde(default)]
    start_line: Option<usize>,
    #[serde(default)]
    end_line: Option<usize>,
}

#[async_trait]
impl Tool for ReadFileTool {
    async fn execute(&self, args: &Value) -> Result<String> {
        self.execute_impl(args)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    fn tool_name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file. Supports optional line range selection."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read (relative to working directory)"
                },
                "start_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional: starting line number (1-indexed)"
                },
                "end_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional: ending line number (1-indexed)"
                }
            },
            "required": ["path"]
        })
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(parsed_args) = serde_json::from_value::<ReadFileArgs>(args.clone()) {
            format!("Read({})", parsed_args.path)
        } else {
            "Read(?)".to_string()
        }
    }

    fn result_summary(&self, result: &str) -> String {
        let lines = result.lines().count();
        format!("Read {} lines", lines)
    }

    async fn check_permission(
        &self,
        args: &Value,
        permission_manager: &PermissionManager,
    ) -> Result<bool> {
        let args: ReadFileArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow::anyhow!("Invalid arguments for read_file tool: {}", e))?;

        let file_path = self.path_validator.validate_and_resolve(&args.path)?;
        let normalized_path = file_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path"))?
            .to_string();

        let operation = OperationType::ReadFile(normalized_path);
        permission_manager.check_permission(&operation).await
    }
}

impl Default for ReadFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_read_file_tool() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let content = "Hello, World!\nLine 2\nLine 3";

        fs::write(&test_file, content).await.unwrap();

        let tool = ReadFileTool::with_working_directory(temp_dir.path().to_path_buf());
        let args = serde_json::json!({
            "path": "test.txt"
        });

        let result = tool.execute(&args).await.unwrap();
        assert_eq!(result, content);
    }
}
