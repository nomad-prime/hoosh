use crate::tools::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Tool for reading file contents
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

    fn resolve_path(&self, file_path: &str) -> PathBuf {
        let path = Path::new(file_path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
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

        let file_path = self.resolve_path(&args.path);

        // Security check: ensure we're not reading outside the working directory
        if !file_path.starts_with(&self.working_directory) {
            anyhow::bail!("Access denied: cannot read files outside working directory");
        }

        let content = fs::read_to_string(&file_path)
            .await
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

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
}

/// Tool for writing/creating files
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

    fn resolve_path(&self, file_path: &str) -> PathBuf {
        let path = Path::new(file_path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
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

        let file_path = self.resolve_path(&args.path);

        // Security check: ensure we're not writing outside the working directory
        if !file_path.starts_with(&self.working_directory) {
            anyhow::bail!("Access denied: cannot write files outside working directory");
        }

        // Create parent directories if requested
        if args.create_dirs {
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
            }
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
}

/// Tool for listing directory contents
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
        if !dir_path.starts_with(&self.working_directory) {
            anyhow::bail!("Access denied: cannot access directories outside working directory");
        }

        let mut entries = fs::read_dir(&dir_path)
            .await
            .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

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
        let mut result = format!("Contents of {}:\n", dir_path.display());

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
