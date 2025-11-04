use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::security::PathValidator;
use crate::tools::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use colored::Colorize;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::fs;

pub struct WriteFileTool {
    path_validator: PathValidator,
}

impl WriteFileTool {
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
        let args: WriteFileArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "write_file".to_string(),
                message: e.to_string(),
            })?;

        let file_path = self
            .path_validator
            .validate_and_resolve(&args.path)
            .map_err(|e| ToolError::SecurityViolation {
                message: e.to_string(),
            })?;

        // Create parent directories if requested
        if args.create_dirs
            && let Some(parent) = file_path.parent()
        {
            fs::create_dir_all(parent)
                .await
                .map_err(|_| ToolError::WriteFailed {
                    path: file_path.clone(),
                })?;
        }

        let content = args.content.as_deref().unwrap_or("");

        fs::write(&file_path, content)
            .await
            .map_err(|_| ToolError::WriteFailed {
                path: file_path.clone(),
            })?;

        Ok(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            file_path.display()
        ))
    }
}

#[derive(Deserialize)]
struct WriteFileArgs {
    path: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    create_dirs: bool,
}

#[async_trait]
impl Tool for WriteFileTool {
    async fn execute(&self, args: &Value) -> ToolResult<String> {
        self.execute_impl(args).await
    }

    fn name(&self) -> &'static str {
        "write_file"
    }

    fn display_name(&self) -> &'static str {
        "write"
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
                    "description": "The content to write to the file. If not provided, an empty file will be created."
                },
                "create_dirs": {
                    "type": "boolean",
                    "default": false,
                    "description": "Whether to create parent directories if they don't exist"
                }
            },
            "required": ["path"]
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
        if let Some(bytes_str) = result.split("wrote ").nth(1)
            && let Some(bytes) = bytes_str.split(" bytes").next()
        {
            if bytes == "0" {
                return "Created empty file".to_string();
            }
            return format!("Wrote {} bytes", bytes);
        }
        "File written successfully".to_string()
    }

    async fn generate_preview(&self, args: &Value) -> Option<String> {
        let args: WriteFileArgs = serde_json::from_value(args.clone()).ok()?;
        let file_path = self.path_validator.validate_and_resolve(&args.path).ok()?;
        let content = args.content.as_deref().unwrap_or("");

        // Check if file exists
        if file_path.exists() {
            // Show diff for overwriting existing file
            let old_content = fs::read_to_string(&file_path).await.ok()?;
            Some(self.generate_diff(&old_content, content, &args.path))
        } else {
            // Show preview of new file content
            Some(self.generate_new_file_preview(content, &args.path))
        }
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        use crate::permissions::FilePatternMatcher;
        use std::sync::Arc;

        ToolPermissionBuilder::new(self, target.unwrap_or("*"))
            .into_destructive()
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .with_display_name("Write")
            .build()
            .expect("Failed to build WriteFileTool permission descriptor")
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
