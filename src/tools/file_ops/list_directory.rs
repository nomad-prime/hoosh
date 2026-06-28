use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::tools::{CategoryPhrasing, Tool, ToolError, ToolExecutionContext, ToolResult, phrasing};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct ListDirectoryTool {
    working_directory: PathBuf,
}

impl ListDirectoryTool {
    pub const NAME: &'static str = "list_directory";

    pub fn new() -> Self {
        let working_directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self { working_directory }
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
        let p = Path::new(dir_path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.working_directory.join(p)
        }
    }

    async fn execute_impl(&self, args: &serde_json::Value) -> ToolResult<String> {
        let args: ListDirectoryArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "list_directory".to_string(),
                message: e.to_string(),
            })?;

        let dir_path = self.resolve_path(&args.path);

        let mut entries =
            fs::read_dir(&dir_path)
                .await
                .map_err(|_| ToolError::ExecutionFailed {
                    message: format!("Failed to read directory: {}", dir_path.display()),
                })?;

        let mut directory_entries = Vec::new();

        while let Some(entry) =
            entries
                .next_entry()
                .await
                .map_err(|_| ToolError::ExecutionFailed {
                    message: "Failed to read directory entry".to_string(),
                })?
        {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files unless explicitly requested
            if !args.show_hidden && file_name.starts_with('.') {
                continue;
            }

            let metadata = entry
                .metadata()
                .await
                .map_err(|_| ToolError::ExecutionFailed {
                    message: "Failed to read file metadata".to_string(),
                })?;
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
                    result.push_str(&format!("  {}/\n", dir));
                }
            }

            if !files.is_empty() {
                result.push_str("\nFiles:\n");
                for file in files {
                    result.push_str(&format!("  {}\n", file));
                }
            }
        }

        Ok(result)
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
    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolExecutionContext,
    ) -> ToolResult<String> {
        self.execute_impl(args).await
    }

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn phrasing(&self) -> CategoryPhrasing {
        phrasing::LIST
    }

    fn display_name(&self) -> &'static str {
        "list"
    }

    fn description(&self) -> &'static str {
        "List the contents of a directory, showing files and subdirectories.\n\n\
        Usage:\n\
        - Returns directories first (each with a trailing /), then files\n\
        - File sizes are shown in bytes\n\
        - Hidden files (starting with .) are excluded by default\n\
        - Use this instead of bash ls command\n\n\
        When to use:\n\
        - Understanding project structure\n\
        - Finding files in a specific directory\n\
        - Exploring unfamiliar codebases\n\n\
        When NOT to use:\n\
        - Finding files across multiple directories - use glob instead\n\
        - Reading file contents - use read_file instead\n\
        - Searching for patterns in files - use grep instead"
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "default": "",
                    "description": "The directory path to list. Use \"\" or \".\" for current directory. Examples: \"src\", \"src/components\", \"tests\". Relative to working directory."
                },
                "show_hidden": {
                    "type": "boolean",
                    "default": false,
                    "description": "If true, include hidden files/directories (those starting with '.'). Examples: .gitignore, .env, .github/"
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
        if result.contains("(empty directory)") {
            return "Empty directory".to_string();
        }

        let mut dir_count = 0;
        let mut file_count = 0;
        for line in result.lines() {
            if !line.starts_with("  ") {
                continue;
            }
            if line.trim_end().ends_with('/') {
                dir_count += 1;
            } else {
                file_count += 1;
            }
        }

        if file_count > 0 || dir_count > 0 {
            format!("Found {} files, {} directories", file_count, dir_count)
        } else {
            "Listed directory contents".to_string()
        }
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        use crate::permissions::FilePatternMatcher;
        use std::sync::Arc;

        ToolPermissionBuilder::new(self, target.unwrap_or("*"))
            .into_read_only()
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .with_display_name("List")
            .build()
            .expect("Failed to build ListDirectoryTool permission descriptor")
    }
}

impl Default for ListDirectoryTool {
    fn default() -> Self {
        Self::new()
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

        let context = ToolExecutionContext {
            tool_call_id: "test".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };

        let result = tool.execute(&args, &context).await.unwrap();
        assert!(result.contains("file1.txt"));
        assert!(result.contains("subdir/"));
        assert!(!result.contains('📁'));
        assert!(!result.contains('📄'));
    }

    #[tokio::test]
    async fn result_summary_counts_without_emoji() {
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("a.txt"), "x").await.unwrap();
        fs::write(temp_dir.path().join("b.txt"), "y").await.unwrap();
        fs::create_dir(temp_dir.path().join("sub")).await.unwrap();

        let tool = ListDirectoryTool::with_working_directory(temp_dir.path().to_path_buf());
        let context = ToolExecutionContext {
            tool_call_id: "test".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };
        let result = tool
            .execute(&serde_json::json!({ "path": "" }), &context)
            .await
            .unwrap();

        assert_eq!(tool.result_summary(&result), "Found 2 files, 1 directories");
    }

    #[test]
    fn result_summary_reports_empty_directory() {
        let tool = ListDirectoryTool::new();
        assert_eq!(
            tool.result_summary("Contents of .:\n  (empty directory)\n"),
            "Empty directory"
        );
    }
}
