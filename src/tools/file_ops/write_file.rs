use crate::permissions::{OperationType, PermissionManager};
use crate::tools::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use colored::Colorize;
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
        let canonical_working = self.working_directory.canonicalize().with_context(|| {
            format!(
                "Failed to resolve working directory: {}",
                self.working_directory.display()
            )
        })?;

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
            file_path
                .canonicalize()
                .with_context(|| format!("Failed to resolve path: {}", file_path.display()))?
        } else if let Some(parent) = file_path.parent() {
            // Check parent directory if file doesn't exist
            let canonical_parent = parent.canonicalize().with_context(|| {
                format!("Failed to resolve parent directory: {}", parent.display())
            })?;
            canonical_parent.join(
                file_path
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Invalid file path: no file name"))?,
            )
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

    async fn generate_preview(&self, args: &serde_json::Value) -> Option<String> {
        let args: WriteFileArgs = serde_json::from_value(args.clone()).ok()?;
        let file_path = resolve_path(&args.path, &self.working_directory);

        // Check if file exists
        if file_path.exists() {
            // Show diff for overwriting existing file
            let old_content = fs::read_to_string(&file_path).await.ok()?;
            Some(self.generate_diff(&old_content, &args.content, &args.path))
        } else {
            // Show preview of new file content
            Some(self.generate_new_file_preview(&args.content, &args.path))
        }
    }
}

impl WriteFileTool {
    /// Generate a diff showing the old file vs new file content
    fn generate_diff(&self, old_content: &str, new_content: &str, path: &str) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "{}\n\n",
            format!("Overwriting file: {}", path).bold().cyan()
        ));

        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        let total_old = old_lines.len();
        let total_new = new_lines.len();

        // Show size comparison
        output.push_str(&format!(
            "{}\n",
            format!("Old: {} lines, {} bytes", total_old, old_content.len()).yellow()
        ));
        output.push_str(&format!(
            "{}\n\n",
            format!("New: {} lines, {} bytes", total_new, new_content.len()).yellow()
        ));

        let max_preview_lines = 20;

        // Show first few lines of old content
        if total_old > 0 {
            output.push_str(&format!("{}:\n", "Old content (first lines)".bold()));
            for (i, line) in old_lines.iter().take(max_preview_lines).enumerate() {
                output.push_str(&format!(
                    "{}\n",
                    format!("  {:4} - {}", i + 1, line).bright_red()
                ));
                if i == max_preview_lines - 1 && total_old > max_preview_lines {
                    output.push_str(&format!(
                        "{}\n",
                        format!("       ... ({} more lines)", total_old - max_preview_lines)
                            .dimmed()
                    ));
                }
            }
            output.push('\n');
        }

        // Show first few lines of new content
        output.push_str(&format!("{}:\n", "New content (first lines)".bold()));
        for (i, line) in new_lines.iter().take(max_preview_lines).enumerate() {
            output.push_str(&format!(
                "{}\n",
                format!("  {:4} + {}", i + 1, line).green()
            ));
            if i == max_preview_lines - 1 && total_new > max_preview_lines {
                output.push_str(&format!(
                    "{}\n",
                    format!("       ... ({} more lines)", total_new - max_preview_lines).dimmed()
                ));
            }
        }

        output
    }

    /// Generate a preview for creating a new file
    fn generate_new_file_preview(&self, content: &str, path: &str) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "{}\n\n",
            format!("Creating new file: {}", path).bold().bright_cyan()
        ));

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let max_preview_lines = 20;

        output.push_str(&format!(
            "{}\n\n",
            format!("Size: {} lines, {} bytes", total_lines, content.len()).yellow()
        ));
        output.push_str(&format!("{}:\n", "Content".bold()));

        for (i, line) in lines.iter().take(max_preview_lines).enumerate() {
            output.push_str(&format!(
                "{}\n",
                format!("  {:4} + {}", i + 1, line).green()
            ));
            if i == max_preview_lines - 1 && total_lines > max_preview_lines {
                output.push_str(&format!(
                    "{}\n",
                    format!(
                        "       ... ({} more lines)",
                        total_lines - max_preview_lines
                    )
                    .dimmed()
                ));
            }
        }

        output
    }
}

impl Default for WriteFileTool {
    fn default() -> Self {
        Self::new()
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
