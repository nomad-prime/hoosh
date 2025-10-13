use crate::permissions::{OperationType, PermissionManager};
use crate::tools::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;

use super::common::resolve_path;

pub struct ReadFileTool {
    working_directory: PathBuf,
}

impl ReadFileTool {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            working_directory: working_dir,
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
    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let args: ReadFileArgs =
            serde_json::from_value(args.clone()).context("Invalid arguments for read_file tool")?;

        let file_path = resolve_path(&args.path, &self.working_directory);

        // Security check: ensure we're not reading outside the working directory
        // Use canonicalize to resolve symlinks and prevent path traversal attacks
        let canonical_file = file_path
            .canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", file_path.display()))?;
        let canonical_working = self.working_directory
            .canonicalize()
            .with_context(|| format!("Failed to resolve working directory: {}", self.working_directory.display()))?;

        if !canonical_file.starts_with(&canonical_working) {
            anyhow::bail!("Access denied: cannot read files outside working directory");
        }

        let content = fs::read_to_string(&canonical_file)
            .await
            .with_context(|| format!("Failed to read file: {}", canonical_file.display()))?;

        // Handle line-based reading if specified
        if let (Some(start), Some(end)) = (args.start_line, args.end_line) {
            let lines: Vec<&str> = content.lines().collect();
            if start == 0 || start > lines.len() {
                anyhow::bail!(
                    "Invalid start_line: {} (file has {} lines)",
                    start,
                    lines.len()
                );
            }
            if end > lines.len() {
                anyhow::bail!("Invalid end_line: {} (file has {} lines)", end, lines.len());
            }
            if start > end {
                anyhow::bail!(
                    "start_line ({}) cannot be greater than end_line ({})",
                    start,
                    end
                );
            }

            let selected_lines = &lines[start - 1..end];
            Ok(selected_lines.join("\n"))
        } else if let Some(start) = args.start_line {
            let lines: Vec<&str> = content.lines().collect();
            if start == 0 || start > lines.len() {
                anyhow::bail!(
                    "Invalid start_line: {} (file has {} lines)",
                    start,
                    lines.len()
                );
            }
            let selected_lines = &lines[start - 1..];
            Ok(selected_lines.join("\n"))
        } else {
            Ok(content)
        }
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
        args: &serde_json::Value,
        permission_manager: &PermissionManager,
    ) -> Result<bool> {
        let args: ReadFileArgs = serde_json::from_value(args.clone())
            .context("Invalid arguments for read_file tool")?;

        // Normalize the path for consistent caching
        // Use the same resolved path that will be used in execute()
        let file_path = resolve_path(&args.path, &self.working_directory);
        let normalized_path = file_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path"))?
            .to_string();

        let operation = OperationType::ReadFile(normalized_path);
        permission_manager.check_permission(&operation).await
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
