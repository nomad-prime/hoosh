use crate::permissions::{OperationType, PermissionManager};
use crate::tools::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;

use super::common::resolve_path;

pub struct WriteFileTool {
    working_directory: PathBuf,
}

impl WriteFileTool {
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
struct WriteFileArgs {
    path: String,
    content: String,
    #[serde(default)]
    create_dirs: bool,
}

#[async_trait]
impl Tool for WriteFileTool {
    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let args: WriteFileArgs = serde_json::from_value(args.clone())
            .context("Invalid arguments for write_file tool")?;

        let file_path = resolve_path(&args.path, &self.working_directory);

        // Security check: ensure we're not writing outside the working directory
        // For write operations, we need to check the parent directory since the file might not exist yet
        let canonical_working = self.working_directory
            .canonicalize()
            .with_context(|| format!("Failed to resolve working directory: {}", self.working_directory.display()))?;

        // Create parent directories if requested
        if args.create_dirs {
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
            }
        }

        // Check if file exists to determine which path to canonicalize
        let path_to_check = if file_path.exists() {
            file_path.canonicalize()
                .with_context(|| format!("Failed to resolve path: {}", file_path.display()))?
        } else if let Some(parent) = file_path.parent() {
            // Check parent directory if file doesn't exist
            let canonical_parent = parent
                .canonicalize()
                .with_context(|| format!("Failed to resolve parent directory: {}", parent.display()))?;
            canonical_parent.join(file_path.file_name().unwrap())
        } else {
            anyhow::bail!("Invalid file path: {}", file_path.display());
        };

        if !path_to_check.starts_with(&canonical_working) {
            anyhow::bail!("Access denied: cannot write files outside working directory");
        }

        fs::write(&file_path, &args.content)
            .await
            .with_context(|| format!("Failed to write file: {}", file_path.display()))?;

        Ok(format!(
            "Successfully wrote {} bytes to {}",
            args.content.len(),
            file_path.display()
        ))
    }

    fn tool_name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write content to a file. Can create parent directories if needed."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write (relative to working directory)"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                },
                "create_dirs": {
                    "type": "boolean",
                    "default": false,
                    "description": "Whether to create parent directories if they don't exist"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(parsed_args) = serde_json::from_value::<WriteFileArgs>(args.clone()) {
            format!("Write({})", parsed_args.path)
        } else {
            "Write(?)".to_string()
        }
    }

    fn result_summary(&self, result: &str) -> String {
        // Extract byte count from result like "Successfully wrote 123 bytes to ..."
        if let Some(bytes_str) = result.split("wrote ").nth(1) {
            if let Some(bytes) = bytes_str.split(" bytes").next() {
                return format!("Wrote {} bytes", bytes);
            }
        }
        "File written successfully".to_string()
    }

    async fn check_permission(
        &self,
        args: &serde_json::Value,
        permission_manager: &PermissionManager,
    ) -> Result<bool> {
        let args: WriteFileArgs = serde_json::from_value(args.clone())
            .context("Invalid arguments for write_file tool")?;

        // Normalize the path for consistent caching
        let file_path = resolve_path(&args.path, &self.working_directory);
        let normalized_path = file_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path"))?
            .to_string();

        // Always use WriteFile for caching consistency
        // Whether creating or overwriting, the permission is the same: writing to a file
        let operation = OperationType::WriteFile(normalized_path);

        permission_manager.check_permission(&operation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_write_file_tool() {
        let temp_dir = tempdir().unwrap();
        let tool = WriteFileTool::with_working_directory(temp_dir.path().to_path_buf());

        let content = "Test content";
        let args = serde_json::json!({
            "path": "new_file.txt",
            "content": content
        });

        let result = tool.execute(&args).await.unwrap();
        assert!(result.contains("Successfully wrote"));

        let written_content = fs::read_to_string(temp_dir.path().join("new_file.txt"))
            .await
            .unwrap();
        assert_eq!(written_content, content);
    }
}
