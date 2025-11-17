use crate::permissions::BashPatternMatcher;
use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::tools::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Tool for executing bash commands safely
pub struct BashTool {
    working_directory: PathBuf,
    timeout_seconds: u64,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            timeout_seconds: 30, // Default 30 second timeout
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            working_directory: working_dir,
            timeout_seconds: 30,
        }
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    /// Sanitize command to prevent some basic injection attempts
    /// Note: This is NOT sufficient for security - dangerous commands should be blocked entirely
    fn sanitize_command(&self, command: &str) -> String {
        // Remove null bytes and other control characters that could be problematic
        // Keep newlines and tabs as they might be intentional
        command
            .chars()
            .filter(|c| !c.is_control() || matches!(c, '\n' | '\t' | ' ') || c.is_whitespace())
            .collect()
    }

    async fn execute_impl(&self, args: &Value) -> ToolResult<String> {
        let args: BashArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "bash".to_string(),
                message: e.to_string(),
            })?;

        let command = self.sanitize_command(&args.command);

        let timeout_duration =
            Duration::from_secs(args.timeout_override.unwrap_or(self.timeout_seconds));

        // Execute the command
        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(&command)
            .current_dir(&self.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let command_future = async {
            let output = cmd.output().await.map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to execute command '{}': {}", command, e),
            })?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let mut result = String::new();

            if !stdout.is_empty() {
                result.push_str("STDOUT:\n");
                result.push_str(&stdout);
                if !stdout.ends_with('\n') {
                    result.push('\n');
                }
            }

            if !stderr.is_empty() {
                result.push_str("STDERR:\n");
                result.push_str(&stderr);
                if !stderr.ends_with('\n') {
                    result.push('\n');
                }
            }

            if result.is_empty() {
                result = "(command executed successfully with no output)\n".to_string();
            }

            // Add exit code information
            result.push_str(&format!(
                "Exit code: {}\n",
                output.status.code().unwrap_or(-1)
            ));

            if !output.status.success() {
                result.push_str("⚠️  Command failed with non-zero exit code\n");
            }

            Ok::<String, ToolError>(result)
        };

        // Apply timeout
        match timeout(timeout_duration, command_future).await {
            Ok(result) => result,
            Err(_) => Err(ToolError::Timeout {
                tool: "bash".to_string(),
                seconds: timeout_duration.as_secs(),
            }),
        }
    }
}

#[derive(Deserialize)]
struct BashArgs {
    command: String,
    #[serde(default)]
    timeout_override: Option<u64>,
}

#[async_trait]
impl Tool for BashTool {
    async fn execute(&self, args: &Value) -> ToolResult<String> {
        self.execute_impl(args).await
    }

    fn name(&self) -> &'static str {
        "bash"
    }

    fn display_name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        "Execute bash commands safely with timeout and security restrictions. \
        You are already in the project directory - do not cd into it.
        Only use cd if you need to access files in a different directory.
        Only use Bash if the other tools do not give you the functionalities you need. Bash should be last resort.
        "
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout_override": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 300,
                    "description": "Optional: timeout in seconds (max 300 seconds)"
                }
            },
            "required": ["command"]
        })
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(parsed_args) = serde_json::from_value::<BashArgs>(args.clone()) {
            // Show a preview of the command (truncate if too long)
            let cmd = &parsed_args.command;
            if cmd.chars().count() > 75 {
                let truncated: String = cmd.chars().take(75).collect();
                format!("Bash({}...)", truncated)
            } else {
                format!("Bash({})", cmd)
            }
        } else {
            "Bash(?)".to_string()
        }
    }

    fn result_summary(&self, result: &str) -> String {
        // Check if command completed successfully
        if result.contains("Exit code: 0") {
            // Try to extract meaningful output
            if let Some(stdout) = result.split("STDOUT:\n").nth(1) {
                let output_line = stdout.lines().next().unwrap_or("");
                if !output_line.is_empty() && !output_line.starts_with("Exit code:") {
                    if output_line.chars().count() > 50 {
                        let truncated: String = output_line.chars().take(50).collect();
                        return format!("{}...", truncated);
                    }
                    return output_line.to_string();
                }
            }
            "Command completed successfully".to_string()
        } else if result.contains("Command failed") {
            "Command failed with non-zero exit code".to_string()
        } else {
            let lines = result.lines().count();
            format!("Command completed ({} lines output)", lines)
        }
    }

    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
        use super::{BashCommandClassifier, BashCommandParser, CommandRisk};

        let target_str = target.unwrap_or("*");

        if BashCommandClassifier::classify(target_str) == CommandRisk::Safe {
            return ToolPermissionBuilder::new(self, target_str)
                .into_read_only()
                .with_approval_title(" Bash Command ")
                .with_approval_prompt("Can I run this bash command?".to_string())
                .with_command_preview(target_str.to_string())
                .with_persistent_approval("don't ask me again for bash in this project".to_string())
                .with_suggested_pattern("*".to_string())
                .with_pattern_matcher(Arc::new(BashPatternMatcher))
                .build()
                .expect("Failed to build BashTool permission descriptor");
        }

        let base_commands = BashCommandParser::extract_base_commands(target_str);
        let suggested_pattern = BashCommandParser::suggest_pattern(&base_commands);

        let persistent_message = if suggested_pattern.contains('|') {
            let commands: Vec<&str> = suggested_pattern
                .split('|')
                .map(|s| s.trim_end_matches(":*"))
                .collect();
            let display = commands.join(", ");
            format!(
                "don't ask me again for pipe combination of \"{}\" commands in this project",
                display
            )
        } else {
            let pattern_display = suggested_pattern
                .trim_end_matches(":*")
                .trim_end_matches('*');
            if pattern_display.is_empty() {
                "don't ask me again for bash in this project".to_string()
            } else {
                format!(
                    "don't ask me again for \"{}\" commands in this project",
                    pattern_display
                )
            }
        };

        ToolPermissionBuilder::new(self, target_str)
            .with_approval_title(" Bash Command ")
            .with_approval_prompt("Can I run this bash command?".to_string())
            .with_command_preview(target_str.to_string())
            .with_persistent_approval(persistent_message)
            .with_suggested_pattern(suggested_pattern)
            .with_pattern_matcher(Arc::new(BashPatternMatcher))
            .build()
            .expect("Failed to build BashTool permission descriptor")
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bash_tool_simple_command() {
        let tool = BashTool::new();
        let args = serde_json::json!({
            "command": "echo 'Hello, World!'"
        });

        let result = tool.execute(&args).await.unwrap();
        assert!(result.contains("Hello, World!"));
        assert!(result.contains("Exit code: 0"));
    }

    #[tokio::test]
    async fn test_bash_tool_failed_command() {
        let tool = BashTool::new();
        let args = serde_json::json!({
            "command": "ls /nonexistent/directory"
        });

        let result = tool.execute(&args).await.unwrap();
        assert!(result.contains("STDERR:"));
        assert!(result.contains("Exit code:"));
        assert!(result.contains("Command failed"));
    }

    #[tokio::test]
    async fn test_bash_tool_timeout() {
        let tool = BashTool::new().with_timeout(1); // 1 second timeout
        let args = json!({
            "command": "sleep 5" // This should timeout
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Timeout"));
    }
}
