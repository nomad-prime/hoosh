use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::tools::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum OutputMode {
    FilesWithMatches,
    Content,
    Count,
}

#[derive(Debug, Deserialize)]
struct GrepArgs {
    pattern: String,
    path: Option<String>,
    #[serde(default)]
    output_mode: Option<String>,
    glob: Option<String>,
    #[serde(rename = "type")]
    file_type: Option<String>,
    #[serde(rename = "-i")]
    case_insensitive: Option<bool>,
    #[serde(rename = "-n")]
    line_numbers: Option<bool>,
    #[serde(rename = "-A")]
    after_context: Option<u32>,
    #[serde(rename = "-B")]
    before_context: Option<u32>,
    #[serde(rename = "-C")]
    context: Option<u32>,
    multiline: Option<bool>,
    head_limit: Option<u32>,
    offset: Option<u32>,
}

impl GrepArgs {
    fn get_output_mode(&self) -> OutputMode {
        match self.output_mode.as_deref() {
            Some("content") => OutputMode::Content,
            Some("count") => OutputMode::Count,
            _ => OutputMode::FilesWithMatches,
        }
    }

    fn should_show_line_numbers(&self) -> bool {
        self.line_numbers.unwrap_or(true)
    }

    fn is_case_insensitive(&self) -> bool {
        self.case_insensitive.unwrap_or(false)
    }

    fn multiline_enabled(&self) -> bool {
        self.multiline.unwrap_or(false)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Match {
    path: String,
    line_number: Option<u32>,
    content: Option<String>,
    count: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GrepResult {
    matches: Vec<Match>,
    total_count: usize,
    truncated: bool,
}

pub struct GrepTool {
    working_directory: PathBuf,
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GrepTool {
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

    fn build_command(&self, args: &GrepArgs) -> ToolResult<Command> {
        if which::which("rg").is_err() {
            return Err(ToolError::ExecutionFailed {
                message: "ripgrep (rg) not found in PATH. Install with:\n  \
                 macOS:        brew install ripgrep\n  \
                 Ubuntu/Debian: apt install ripgrep\n  \
                 Arch:         pacman -S ripgrep\n  \
                 Windows:      choco install ripgrep\n  \
                 Cargo:        cargo install ripgrep"
                    .to_string(),
            });
        }

        let mut cmd = Command::new("rg");
        cmd.arg("--json");

        let output_mode = args.get_output_mode();
        match output_mode {
            OutputMode::FilesWithMatches => {
                cmd.arg("--files-with-matches");
            }
            OutputMode::Content => {
                if args.should_show_line_numbers() {
                    cmd.arg("--line-number");
                } else {
                    cmd.arg("--no-line-number");
                }
            }
            OutputMode::Count => {
                cmd.arg("--count");
            }
        }

        if args.is_case_insensitive() {
            cmd.arg("--ignore-case");
        }

        if let Some(ctx) = args.context {
            cmd.arg(format!("--context={}", ctx));
        } else {
            if let Some(after) = args.after_context {
                cmd.arg(format!("--after-context={}", after));
            }
            if let Some(before) = args.before_context {
                cmd.arg(format!("--before-context={}", before));
            }
        }

        if args.multiline_enabled() {
            cmd.arg("--multiline");
        }

        if let Some(glob) = &args.glob {
            cmd.arg("--glob").arg(glob);
        }
        if let Some(file_type) = &args.file_type {
            cmd.arg("--type").arg(file_type);
        }

        if let Some(limit) = args.head_limit {
            cmd.arg("--max-count").arg(limit.to_string());
        }

        cmd.arg(&args.pattern);

        let path = args.path.as_deref().unwrap_or(".");
        cmd.arg(path);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.current_dir(&self.working_directory);

        Ok(cmd)
    }

    async fn parse_output(&self, args: &GrepArgs, output: String) -> ToolResult<GrepResult> {
        let mut matches = Vec::new();

        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };

            if msg.get("type").and_then(|t| t.as_str()) != Some("match") {
                continue;
            }

            let Some(path_obj) = msg.get("data").and_then(|d| d.get("path")) else {
                continue;
            };
            let Some(line_num) = msg.get("data").and_then(|d| d.get("line_number")) else {
                continue;
            };
            let Some(lines_obj) = msg.get("data").and_then(|d| d.get("lines")) else {
                continue;
            };

            let Some(path_str) = path_obj.get("text").and_then(|t| t.as_str()) else {
                continue;
            };
            let Some(line_u64) = line_num.as_u64() else {
                continue;
            };
            let Some(content) = lines_obj.get("text").and_then(|t| t.as_str()) else {
                continue;
            };

            matches.push(Match {
                path: path_str.to_string(),
                line_number: Some(line_u64 as u32),
                content: Some(content.to_string()),
                count: None,
            });
        }

        let total_count = matches.len();
        let offset = args.offset.unwrap_or(0) as usize;
        let limit = args.head_limit.map(|l| l as usize);

        let matches: Vec<_> = matches.into_iter().skip(offset).collect();
        let (matches, truncated) = if let Some(limit) = limit {
            let truncated = matches.len() > limit;
            (matches.into_iter().take(limit).collect(), truncated)
        } else {
            (matches, false)
        };

        Ok(GrepResult {
            matches,
            total_count,
            truncated,
        })
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn display_name(&self) -> &'static str {
        "Grep"
    }

    fn description(&self) -> &'static str {
        "Search code using regex patterns. Built on ripgrep for fast, accurate searches across large codebases."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for (ripgrep syntax)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (defaults to current directory)"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["files_with_matches", "content", "count"],
                    "description": "Output format",
                    "default": "files_with_matches"
                },
                "glob": {
                    "type": "string",
                    "description": "File pattern: *.rs, **/*.{ts,tsx}"
                },
                "type": {
                    "type": "string",
                    "description": "File type filter: rust, python, javascript"
                },
                "-i": {
                    "type": "boolean",
                    "description": "Case insensitive search"
                },
                "-n": {
                    "type": "boolean",
                    "description": "Show line numbers (default: true)",
                    "default": true
                },
                "-A": {
                    "type": "integer",
                    "description": "Lines of context after match"
                },
                "-B": {
                    "type": "integer",
                    "description": "Lines of context before match"
                },
                "-C": {
                    "type": "integer",
                    "description": "Lines of context before and after match"
                },
                "multiline": {
                    "type": "boolean",
                    "description": "Enable multi-line pattern matching"
                },
                "head_limit": {
                    "type": "integer",
                    "description": "Limit to first N results"
                },
                "offset": {
                    "type": "integer",
                    "description": "Skip first N results"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: &Value) -> ToolResult<String> {
        let args: GrepArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "grep".to_string(),
                message: format!("Invalid grep arguments: {}", e),
            })?;

        let mut cmd = self.build_command(&args)?;

        let output = cmd.output().await.map_err(|e| ToolError::ExecutionFailed {
            message: format!("Failed to execute ripgrep: {}", e),
        })?;

        if !output.status.success() {
            // Exit code 1 from ripgrep means no matches found, which is not an error
            // Exit code 2+ indicates actual errors
            if output.status.code() != Some(1) {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(ToolError::ExecutionFailed {
                    message: format!("ripgrep failed: {}", stderr),
                });
            }
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let result = self.parse_output(&args, stdout.to_string()).await?;

        serde_json::to_string_pretty(&result).map_err(|e| ToolError::ExecutionFailed {
            message: format!("Failed to serialize result: {}", e),
        })
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        ToolPermissionBuilder::new(self, target.unwrap_or("*"))
            .into_read_only()
            .build()
            .expect("Failed to build grep permission descriptor")
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(grep_args) = serde_json::from_value::<GrepArgs>(args.clone()) {
            let path_str = grep_args.path.as_deref().unwrap_or(".");
            format!("Grep({}, {})", grep_args.pattern, path_str)
        } else {
            "Grep(...)".to_string()
        }
    }

    fn result_summary(&self, result: &str) -> String {
        if let Ok(grep_result) = serde_json::from_str::<GrepResult>(result) {
            let count = grep_result.matches.len();
            let truncated = if grep_result.truncated {
                " (truncated)"
            } else {
                ""
            };
            format!(
                "Found {} match{}{}",
                count,
                if count == 1 { "" } else { "es" },
                truncated
            )
        } else {
            "Search completed".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grep_tool_name() {
        let tool = GrepTool::new();
        assert_eq!(tool.name(), "grep");
    }

    #[test]
    fn test_grep_tool_display_name() {
        let tool = GrepTool::new();
        assert_eq!(tool.display_name(), "Grep");
    }

    #[test]
    fn test_grep_tool_description() {
        let tool = GrepTool::new();
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_grep_args_output_mode() {
        let args = GrepArgs {
            pattern: "test".to_string(),
            path: None,
            output_mode: Some("content".to_string()),
            glob: None,
            file_type: None,
            case_insensitive: None,
            line_numbers: None,
            after_context: None,
            before_context: None,
            context: None,
            multiline: None,
            head_limit: None,
            offset: None,
        };

        match args.get_output_mode() {
            OutputMode::Content => (),
            _ => panic!("Expected Content output mode"),
        }
    }

    #[test]
    fn test_grep_args_default_output_mode() {
        let args = GrepArgs {
            pattern: "test".to_string(),
            path: None,
            output_mode: None,
            glob: None,
            file_type: None,
            case_insensitive: None,
            line_numbers: None,
            after_context: None,
            before_context: None,
            context: None,
            multiline: None,
            head_limit: None,
            offset: None,
        };

        match args.get_output_mode() {
            OutputMode::FilesWithMatches => (),
            _ => panic!("Expected FilesWithMatches output mode"),
        }
    }

    #[test]
    fn test_format_call_display() {
        let tool = GrepTool::new();
        let args = json!({
            "pattern": "test_pattern",
            "path": "src/main.rs"
        });

        let display = tool.format_call_display(&args);
        assert!(display.contains("test_pattern"));
        assert!(display.contains("src/main.rs"));
    }

    #[test]
    fn test_format_call_display_default_path() {
        let tool = GrepTool::new();
        let args = json!({
            "pattern": "test_pattern"
        });

        let display = tool.format_call_display(&args);
        assert!(display.contains("test_pattern"));
        assert!(display.contains("."));
    }

    #[test]
    fn test_result_summary() {
        let tool = GrepTool::new();
        let result = r#"{"matches":[{"path":"test.rs","line_number":1,"content":"test","count":null}],"total_count":1,"truncated":false}"#;

        let summary = tool.result_summary(result);
        assert!(summary.contains("1"));
        assert!(summary.contains("match"));
    }

    #[test]
    fn test_result_summary_truncated() {
        let tool = GrepTool::new();
        let result = r#"{"matches":[],"total_count":100,"truncated":true}"#;

        let summary = tool.result_summary(result);
        assert!(summary.contains("truncated"));
    }

    #[test]
    fn test_parameter_schema() {
        let tool = GrepTool::new();
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

    #[tokio::test]
    async fn test_grep_execution_simple() {
        if which::which("rg").is_err() {
            eprintln!("ripgrep not found, skipping integration test");
            return;
        }

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "fn ",
            "path": "src/tools/grep.rs",
            "output_mode": "content"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_ok(), "Execution should succeed");

        let result_str = result.unwrap();
        let grep_result: GrepResult =
            serde_json::from_str(&result_str).expect("Should deserialize result");

        // With content mode, we should find matches when searching for "fn "
        assert!(
            !grep_result.matches.is_empty(),
            "Should find matches for 'fn ' in grep.rs"
        );
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        if which::which("rg").is_err() {
            eprintln!("ripgrep not found, skipping integration test");
            return;
        }

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "xyzabc_nonexistent_pattern_12345",
            "path": "src/tools/grep.rs"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_grep_with_test_file() {
        use std::fs;

        if which::which("rg").is_err() {
            eprintln!("ripgrep not found, skipping integration test");
            return;
        }

        // Create a test file
        let test_content = "hello world\nfoo bar\nhello again\n";
        fs::write("test_grep_temp.txt", test_content).expect("Failed to create test file");

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "hello",
            "path": "test_grep_temp.txt",
            "output_mode": "content"
        });

        let result = tool.execute(&args).await;

        // Clean up
        let _ = fs::remove_file("test_grep_temp.txt");

        assert!(result.is_ok(), "Execution should succeed");

        let result_str = result.unwrap();
        println!("Grep result: {}", result_str);

        let grep_result: GrepResult =
            serde_json::from_str(&result_str).expect("Should deserialize result");

        // We should find 2 matches for "hello"
        assert_eq!(
            grep_result.matches.len(),
            2,
            "Should find 2 matches for 'hello', but found {}: {:?}",
            grep_result.matches.len(),
            grep_result.matches
        );
    }

    #[tokio::test]
    async fn test_grep_with_absolute_path() {
        use std::fs;

        if which::which("rg").is_err() {
            eprintln!("ripgrep not found, skipping integration test");
            return;
        }

        // Create a test file in the current directory
        let test_content = "hello world\nfoo bar\nhello again\n";
        let filename = "test_grep_absolute.txt";
        fs::write(filename, test_content).expect("Failed to create test file");

        let tool = GrepTool::new();

        // Get absolute path
        let absolute_path = std::env::current_dir()
            .ok()
            .and_then(|pwd| pwd.join(filename).to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| filename.to_string());

        println!("Testing with absolute path: {}", absolute_path);

        let args = json!({
            "pattern": "hello",
            "path": absolute_path,
            "output_mode": "content"
        });

        let result = tool.execute(&args).await;

        // Clean up
        let _ = fs::remove_file(filename);

        assert!(result.is_ok(), "Execution should succeed");

        let result_str = result.unwrap();
        println!("Grep result: {}", result_str);

        let grep_result: GrepResult =
            serde_json::from_str(&result_str).expect("Should deserialize result");

        // We should find 2 matches for "hello"
        assert_eq!(
            grep_result.matches.len(),
            2,
            "Should find 2 matches for 'hello', but found {}: {:?}",
            grep_result.matches.len(),
            grep_result.matches
        );
    }

    #[tokio::test]
    async fn test_grep_current_directory_explicitly() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        if which::which("rg").is_err() {
            eprintln!("ripgrep not found, skipping integration test");
            return;
        }

        // Use unique filename to avoid test conflicts
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let filename = format!("test_grep_cwd_{}.txt", timestamp);

        // Create a test file
        let test_content = "hello world\nfoo bar\nhello again\n";
        fs::write(&filename, test_content).expect("Failed to create test file");

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "hello",
            "path": &filename,
            "output_mode": "content"
        });

        let result = tool.execute(&args).await;

        // Clean up
        let _ = fs::remove_file(&filename);

        assert!(
            result.is_ok(),
            "Execution should succeed: {:?}",
            result.err()
        );

        let result_str = result.unwrap();
        println!("Grep result for '{}': {}", filename, result_str);
    }

    #[test]
    fn test_grep_permission_descriptor() {
        let tool = GrepTool::new();
        let perm = tool.describe_permission(Some("test.rs"));

        assert_eq!(perm.kind(), "grep");
        assert!(perm.is_read_only());
        assert!(!perm.is_destructive());
    }

    #[test]
    fn test_grep_case_insensitive_flag() {
        let args = GrepArgs {
            pattern: "test".to_string(),
            path: None,
            output_mode: None,
            glob: None,
            file_type: None,
            case_insensitive: Some(true),
            line_numbers: None,
            after_context: None,
            before_context: None,
            context: None,
            multiline: None,
            head_limit: None,
            offset: None,
        };

        assert!(args.is_case_insensitive());
    }

    #[test]
    fn test_grep_multiline_enabled() {
        let args = GrepArgs {
            pattern: "test".to_string(),
            path: None,
            output_mode: None,
            glob: None,
            file_type: None,
            case_insensitive: None,
            line_numbers: None,
            after_context: None,
            before_context: None,
            context: None,
            multiline: Some(true),
            head_limit: None,
            offset: None,
        };

        assert!(args.multiline_enabled());
    }

    #[test]
    fn test_grep_line_numbers_default() {
        let args = GrepArgs {
            pattern: "test".to_string(),
            path: None,
            output_mode: None,
            glob: None,
            file_type: None,
            case_insensitive: None,
            line_numbers: None,
            after_context: None,
            before_context: None,
            context: None,
            multiline: None,
            head_limit: None,
            offset: None,
        };

        assert!(args.should_show_line_numbers());
    }

    #[test]
    fn test_grep_line_numbers_explicit_false() {
        let args = GrepArgs {
            pattern: "test".to_string(),
            path: None,
            output_mode: None,
            glob: None,
            file_type: None,
            case_insensitive: None,
            line_numbers: Some(false),
            after_context: None,
            before_context: None,
            context: None,
            multiline: None,
            head_limit: None,
            offset: None,
        };

        assert!(!args.should_show_line_numbers());
    }
}
