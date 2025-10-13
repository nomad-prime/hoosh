use crate::permissions::{OperationType, PermissionManager};
use crate::tools::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct ListDirectoryTool {
    working_directory: PathBuf,
}

impl ListDirectoryTool {
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

    fn resolve_path(&self, dir_path: &str) -> PathBuf {
        if dir_path.is_empty() || dir_path == "." {
            return self.working_directory.clone();
        }

        let path = Path::new(dir_path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
        }
    }
}

#[derive(Deserialize)]
struct ListDirectoryArgs {
    #[serde(default)]
    path: String,
    #[serde(default)]
    show_hidden: bool,
}

#[derive(Serialize)]
struct DirectoryEntry {
    name: String,
    is_file: bool,
    is_dir: bool,
    size: Option<u64>,
}

#[async_trait]
impl Tool for ListDirectoryTool {
    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let args: ListDirectoryArgs = serde_json::from_value(args.clone())
            .context("Invalid arguments for list_directory tool")?;

        let dir_path = self.resolve_path(&args.path);

        // Security check: ensure we're not accessing outside the working directory
        // Use canonicalize to resolve symlinks and prevent path traversal attacks
        let canonical_dir = dir_path
            .canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", dir_path.display()))?;
        let canonical_working = self.working_directory
            .canonicalize()
            .with_context(|| format!("Failed to resolve working directory: {}", self.working_directory.display()))?;

        if !canonical_dir.starts_with(&canonical_working) {
            anyhow::bail!("Access denied: cannot access directories outside working directory");
        }

        let mut entries = fs::read_dir(&canonical_dir)
            .await
            .with_context(|| format!("Failed to read directory: {}", canonical_dir.display()))?;

        let mut directory_entries = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files unless explicitly requested
            if !args.show_hidden && file_name.starts_with('.') {
                continue;
            }

            let metadata = entry.metadata().await?;
            let is_file = metadata.is_file();
            let is_dir = metadata.is_dir();
            let size = if is_file { Some(metadata.len()) } else { None };

            directory_entries.push(DirectoryEntry {
                name: file_name,
                is_file,
                is_dir,
                size,
            });
        }

        // Sort entries: directories first, then files, both alphabetically
        directory_entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        // Format output
        let mut result = format!("Contents of {}:\n", canonical_dir.display());

        if directory_entries.is_empty() {
            result.push_str("  (empty directory)\n");
        } else {
            let mut dirs = Vec::new();
            let mut files = Vec::new();

            for entry in directory_entries {
                if entry.is_dir {
                    dirs.push(entry.name);
                } else {
                    let size_str = entry
                        .size
                        .map(|s| format!(" ({} bytes)", s))
                        .unwrap_or_default();
                    files.push(format!("{}{}", entry.name, size_str));
                }
            }

            if !dirs.is_empty() {
                result.push_str("\nDirectories:\n");
                for dir in dirs {
                    result.push_str(&format!("  📁 {}/\n", dir));
                }
            }

            if !files.is_empty() {
                result.push_str("\nFiles:\n");
                for file in files {
                    result.push_str(&format!("  📄 {}\n", file));
                }
            }
        }

        Ok(result)
    }

    fn tool_name(&self) -> &'static str {
        "list_directory"
    }

    fn description(&self) -> &'static str {
        "List the contents of a directory, showing files and subdirectories."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "default": "",
                    "description": "The path to the directory to list (empty or '.' for current directory)"
                },
                "show_hidden": {
                    "type": "boolean",
                    "default": false,
                    "description": "Whether to show hidden files (those starting with '.')"
                }
            },
            "required": []
        })
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(parsed_args) = serde_json::from_value::<ListDirectoryArgs>(args.clone()) {
            if parsed_args.path.is_empty() || parsed_args.path == "." {
                "List(.)".to_string()
            } else {
                format!("List({})", parsed_args.path)
            }
        } else {
            "List(?)".to_string()
        }
    }

    fn result_summary(&self, result: &str) -> String {
        let file_count = result.matches("📄").count();
        let dir_count = result.matches("📁").count();

        if file_count > 0 || dir_count > 0 {
            format!("Found {} files, {} directories", file_count, dir_count)
        } else if result.contains("empty directory") {
            "Empty directory".to_string()
        } else {
            "Listed directory contents".to_string()
        }
    }

    async fn check_permission(
        &self,
        args: &Value,
        permission_manager: &PermissionManager,
    ) -> Result<bool> {
        let args: ListDirectoryArgs = serde_json::from_value(args.clone())
            .context("Invalid arguments for list_directory tool")?;

        // Normalize the path for consistent caching
        let dir_path = self.resolve_path(&args.path);
        let normalized_path = dir_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path"))?
            .to_string();

        let operation = OperationType::ListDirectory(normalized_path);
        permission_manager.check_permission(&operation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_list_directory_tool() {
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("file1.txt"), "content")
            .await
            .unwrap();
        fs::create_dir(temp_dir.path().join("subdir"))
            .await
            .unwrap();

        let tool = ListDirectoryTool::with_working_directory(temp_dir.path().to_path_buf());
        let args = serde_json::json!({
            "path": ""
        });

        let result = tool.execute(&args).await.unwrap();
        assert!(result.contains("file1.txt"));
        assert!(result.contains("subdir/"));
    }
}
