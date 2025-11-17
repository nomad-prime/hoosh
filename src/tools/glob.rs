use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::tools::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct GlobArgs {
    pattern: String,
    path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileMatch {
    path: String,
    modified: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GlobResult {
    matches: Vec<FileMatch>,
    total_count: usize,
}

pub struct GlobTool;

impl Default for GlobTool {
    fn default() -> Self {
        Self
    }
}

impl GlobTool {
    pub fn new() -> Self {
        Self
    }

    fn match_files(&self, args: &GlobArgs) -> ToolResult<Vec<FileMatch>> {
        let pattern = Pattern::new(&args.pattern).map_err(|e| ToolError::InvalidArguments {
            tool: "glob".to_string(),
            message: format!("Invalid glob pattern: {}", e),
        })?;

        let search_path = args.path.as_deref().unwrap_or(".");
        let mut matches = Vec::new();

        for entry in WalkDir::new(search_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();

            if pattern.matches(&path_str) {
                let modified = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);

                matches.push(FileMatch {
                    path: path_str,
                    modified,
                });
            }
        }

        // Sort by modification time (most recent first)
        matches.sort_by(|a, b| match (b.modified, a.modified) {
            (Some(b_time), Some(a_time)) => b_time.cmp(&a_time),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        Ok(matches)
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &'static str {
        "glob"
    }

    fn display_name(&self) -> &'static str {
        "Glob"
    }

    fn description(&self) -> &'static str {
        "Fast file pattern matching tool for finding files by name patterns (e.g., **/*.js, **/src/**/*.rs)"
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against (e.g., **/*.rs, **/src/**/*.ts)"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in. If not specified, the current working directory will be used. IMPORTANT: Omit this field to use the default directory. DO NOT enter \"undefined\" or \"null\" - simply omit it for the default behavior. Must be a valid directory path if provided."
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: &Value) -> ToolResult<String> {
        let args: GlobArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "glob".to_string(),
                message: format!("Invalid glob arguments: {}", e),
            })?;

        let matches = self.match_files(&args)?;
        let total_count = matches.len();

        let result = GlobResult {
            matches,
            total_count,
        };

        serde_json::to_string_pretty(&result).map_err(|e| ToolError::ExecutionFailed {
            message: format!("Failed to serialize result: {}", e),
        })
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        ToolPermissionBuilder::new(self, target.unwrap_or("*"))
            .into_read_only()
            .build()
            .expect("Failed to build glob permission descriptor")
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(glob_args) = serde_json::from_value::<GlobArgs>(args.clone()) {
            let path_str = glob_args.path.as_deref().unwrap_or(".");
            format!("Glob({}, {})", glob_args.pattern, path_str)
        } else {
            "Glob(...)".to_string()
        }
    }

    fn result_summary(&self, result: &str) -> String {
        if let Ok(glob_result) = serde_json::from_str::<GlobResult>(result) {
            let count = glob_result.total_count;
            format!("Found {} file{}", count, if count == 1 { "" } else { "s" })
        } else {
            "Search completed".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_tool_name() {
        let tool = GlobTool::new();
        assert_eq!(tool.name(), "glob");
    }

    #[test]
    fn test_glob_tool_display_name() {
        let tool = GlobTool::new();
        assert_eq!(tool.display_name(), "Glob");
    }

    #[test]
    fn test_glob_tool_description() {
        let tool = GlobTool::new();
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_parameter_schema() {
        let tool = GlobTool::new();
        let schema = tool.parameter_schema();

        assert_eq!(schema.get("type").and_then(|v| v.as_str()), Some("object"));
        let props = schema.get("properties");
        assert!(props.is_some());
        let required = schema.get("required");
        assert!(required.is_some());
        assert!(
            required
                .unwrap()
                .as_array()
                .unwrap()
                .contains(&"pattern".into())
        );
    }

    #[test]
    fn test_format_call_display() {
        let tool = GlobTool::new();
        let args = json!({
            "pattern": "**/*.rs",
            "path": "src"
        });

        let display = tool.format_call_display(&args);
        assert!(display.contains("**/*.rs"));
        assert!(display.contains("src"));
    }

    #[test]
    fn test_format_call_display_default_path() {
        let tool = GlobTool::new();
        let args = json!({
            "pattern": "**/*.rs"
        });

        let display = tool.format_call_display(&args);
        assert!(display.contains("**/*.rs"));
        assert!(display.contains("."));
    }

    #[test]
    fn test_result_summary() {
        let tool = GlobTool::new();
        let result = r#"{"matches":[{"path":"test.rs","modified":null}],"total_count":1}"#;

        let summary = tool.result_summary(result);
        assert!(summary.contains("1"));
        assert!(summary.contains("file"));
        assert!(!summary.contains("files"));
    }

    #[test]
    fn test_result_summary_multiple() {
        let tool = GlobTool::new();
        let result = r#"{"matches":[{"path":"test1.rs","modified":null},{"path":"test2.rs","modified":null}],"total_count":2}"#;

        let summary = tool.result_summary(result);
        assert!(summary.contains("2"));
        assert!(summary.contains("files"));
    }

    #[test]
    fn test_glob_permission_descriptor() {
        let tool = GlobTool::new();
        let perm = tool.describe_permission(Some("*.rs"));

        assert_eq!(perm.kind(), "glob");
        assert!(perm.is_read_only());
        assert!(!perm.is_destructive());
    }

    #[tokio::test]
    async fn test_glob_execution_simple() {
        let tool = GlobTool::new();
        let args = json!({
            "pattern": "*.toml",
            "path": "."
        });

        let result = tool.execute(&args).await;
        assert!(result.is_ok(), "Execution should succeed");
    }

    #[tokio::test]
    async fn test_glob_invalid_pattern() {
        let tool = GlobTool::new();
        let args = json!({
            "pattern": "[invalid"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
    }
}
