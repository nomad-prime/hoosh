use crate::agent::AgentEvent;
use crate::permissions::BashPatternMatcher;
use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::tools::bash::BashCommandPatternRegistry;
use crate::tools::{Tool, ToolError, ToolExecutionContext, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
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
            timeout_seconds: 240, // Default 30 second timeout
        }
    }

    pub fn with_working_directory(mut self, working_dir: PathBuf) -> Self {
        self.working_directory = working_dir;
        self
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

    async fn execute_impl(
        &self,
        args: &Value,
        context: Option<ToolExecutionContext>,
    ) -> ToolResult<String> {
        let args: BashArgs =
            serde_json::from_value(args.clone()).map_err(|e| ToolError::InvalidArguments {
                tool: "bash".to_string(),
                message: e.to_string(),
            })?;

        let command = self.sanitize_command(&args.command);

        let timeout_duration =
            Duration::from_secs(args.timeout_override.unwrap_or(self.timeout_seconds));

        // Check if we should stream output
        let should_stream = context.as_ref().and_then(|c| c.event_tx.as_ref()).is_some();

        if should_stream {
            self.execute_with_streaming(command, timeout_duration, context.unwrap())
                .await
        } else {
            self.execute_without_streaming(command, timeout_duration)
                .await
        }
    }

    async fn execute_without_streaming(
        &self,
        command: String,
        timeout_duration: Duration,
    ) -> ToolResult<String> {
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

    async fn execute_with_streaming(
        &self,
        command: String,
        timeout_duration: Duration,
        context: ToolExecutionContext,
    ) -> ToolResult<String> {
        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(&command)
            .current_dir(&self.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        let command_future = async {
            let mut child = cmd.spawn().map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to spawn command '{}': {}", command, e),
            })?;

            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| ToolError::ExecutionFailed {
                    message: "Failed to capture stdout".to_string(),
                })?;

            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| ToolError::ExecutionFailed {
                    message: "Failed to capture stderr".to_string(),
                })?;

            let tool_call_id = context.tool_call_id.clone();
            let event_tx = context.event_tx.clone();

            // Accumulated output for final result
            let stdout_lines = Arc::new(tokio::sync::Mutex::new(Vec::new()));
            let stderr_lines = Arc::new(tokio::sync::Mutex::new(Vec::new()));

            let stdout_lines_clone = Arc::clone(&stdout_lines);
            let stderr_lines_clone = Arc::clone(&stderr_lines);

            // Spawn tasks to read stdout and stderr
            let stdout_task = {
                let tool_call_id = tool_call_id.clone();
                let event_tx = event_tx.clone();
                let stdout_lines = stdout_lines_clone;
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    let mut line_number = 1;

                    while let Ok(Some(line)) = reader.next_line().await {
                        // Store line for final output
                        stdout_lines.lock().await.push(line.clone());

                        // Send event
                        if let Some(tx) = &event_tx {
                            let _ = tx.send(AgentEvent::BashOutputChunk {
                                tool_call_id: tool_call_id.clone(),
                                output_line: line,
                                stream_type: "stdout".to_string(),
                                line_number,
                                timestamp: std::time::SystemTime::now(),
                            });
                        }
                        line_number += 1;
                    }
                })
            };

            let stderr_task = {
                let tool_call_id = tool_call_id.clone();
                let event_tx = event_tx.clone();
                let stderr_lines = stderr_lines_clone;
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stderr).lines();
                    let mut line_number = 1;

                    while let Ok(Some(line)) = reader.next_line().await {
                        // Store line for final output
                        stderr_lines.lock().await.push(line.clone());

                        // Send event
                        if let Some(tx) = &event_tx {
                            let _ = tx.send(AgentEvent::BashOutputChunk {
                                tool_call_id: tool_call_id.clone(),
                                output_line: line,
                                stream_type: "stderr".to_string(),
                                line_number,
                                timestamp: std::time::SystemTime::now(),
                            });
                        }
                        line_number += 1;
                    }
                })
            };

            // Wait for command to complete
            let status = child.wait().await.map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to wait for command '{}': {}", command, e),
            })?;

            // Wait for stream readers to finish
            let _ = tokio::join!(stdout_task, stderr_task);

            // Build final result
            let stdout_vec = stdout_lines.lock().await;
            let stderr_vec = stderr_lines.lock().await;

            let mut result = String::new();

            if !stdout_vec.is_empty() {
                result.push_str("STDOUT:\n");
                for line in stdout_vec.iter() {
                    result.push_str(line);
                    result.push('\n');
                }
            }

            if !stderr_vec.is_empty() {
                result.push_str("STDERR:\n");
                for line in stderr_vec.iter() {
                    result.push_str(line);
                    result.push('\n');
                }
            }

            if result.is_empty() {
                result = "(command executed successfully with no output)\n".to_string();
            }

            // Add exit code information
            result.push_str(&format!("Exit code: {}\n", status.code().unwrap_or(-1)));

            if !status.success() {
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
    async fn execute(&self, args: &Value, context: &ToolExecutionContext) -> ToolResult<String> {
        self.execute_impl(args, Some(context.clone())).await
    }

    fn name(&self) -> &'static str {
        "bash"
    }

    fn display_name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        r#"Execute bash commands with timeout and security restrictions.\n\n\
        IMPORTANT: This tool is for terminal operations ONLY. Do NOT use it for:\n\
        - Reading files - use read_file instead of cat/head/tail\n\
        - Editing files - use edit_file instead of sed/awk\n\
        - Writing files - use write_file instead of echo > or cat <<EOF\n\
        - Finding files - use glob instead of find\n\
        - Searching content - use grep tool instead of grep/rg commands\n\
        - Listing directories - use list_directory instead of ls\n\n\
        Appropriate uses for bash:\n\
        - Build commands: cargo build, cargo test, npm run, make\n\
        - Git operations: git status, git diff, git commit\n\
        - Package managers: cargo add, npm install, pip install\n\
        - Running tests: cargo test, pytest, npm test\n\
        - Linting: cargo clippy, eslint, rustfmt\n\n\
        - bash("cargo test")                    # NOT: bash("cd /path && cargo test") \n
        - bash("cargo check")                   # NOT: bash("cd /path && cargo check") \n
        - bash(".hoosh/skills/cargo_pipeline.sh")  # NOT: bash("cd /path && .hoosh/skills/cargo_pipeline.sh") \n
        - bash("git status") \n
        - bash("npm install") \n
        Usage notes:\n\
        - You are already in the project directory - do not cd into it\n\
        - Commands timeout after 30 seconds by default (max 300s)\n\
        - Always quote file paths with spaces: cd \"path with spaces\"\n\
        - Avoid interactive commands (-i flags) as they are not supported"#
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute. Examples: \"cargo build --release\", \"git status\", \"cargo test -- --nocapture\", \"npm run build\". Avoid find/grep/cat - use specialized tools instead."
                },
                "timeout_override": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 300,
                    "description": "Optional: timeout in seconds (1-300). Default is 30s. Use higher values for long-running commands like builds or test suites. Example: timeout_override=120 for 2 minutes."
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
        let target_str = target.unwrap_or("*");
        let registry = BashCommandPatternRegistry::new();
        let pattern_result = registry.analyze_command(target_str);

        // Build descriptor with safety info, but let ToolExecutor decide approval
        let mut builder = ToolPermissionBuilder::new(self, target_str)
            .with_approval_title(" Bash Command ")
            .with_approval_prompt("Can I run this bash command?")
            .with_command_preview(target_str.to_string())
            .with_persistent_approval(pattern_result.persistent_message)
            .with_suggested_pattern(pattern_result.pattern)
            .with_pattern_matcher(Arc::new(BashPatternMatcher::new()));

        // Mark as read-only if safe (for ToolExecutor to auto-approve)
        if pattern_result.safe {
            builder = builder.into_read_only();
        }

        builder
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

        let context = ToolExecutionContext {
            tool_call_id: "test".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };

        let result = tool.execute(&args, &context).await.unwrap();
        assert!(result.contains("Hello, World!"));
        assert!(result.contains("Exit code: 0"));
    }

    #[tokio::test]
    async fn test_bash_tool_failed_command() {
        let tool = BashTool::new();
        let args = serde_json::json!({
            "command": "ls /nonexistent/directory"
        });

        let context = ToolExecutionContext {
            tool_call_id: "test".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };

        let result = tool.execute(&args, &context).await.unwrap();
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

        let context = ToolExecutionContext {
            tool_call_id: "test".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };

        let result = tool.execute(&args, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Timeout"));
    }

    #[tokio::test]
    async fn test_bash_tool_streaming_with_context() {
        use tokio::sync::mpsc;

        let tool = BashTool::new();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let context = ToolExecutionContext {
            tool_call_id: "test_call_123".to_string(),
            event_tx: Some(event_tx),
            parent_conversation_id: None,
        };

        let args = json!({
            "command": "echo 'line1'\necho 'line2'\necho 'line3'"
        });

        // Spawn task to collect events
        let events_collector = tokio::spawn(async move {
            let mut collected_events = Vec::new();
            while let Some(event) = event_rx.recv().await {
                if let AgentEvent::BashOutputChunk {
                    tool_call_id,
                    output_line,
                    stream_type,
                    line_number,
                    ..
                } = event
                {
                    collected_events.push((tool_call_id, output_line, stream_type, line_number));
                }
            }
            collected_events
        });

        // Execute command with context
        let result = tool.execute(&args, &context).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.contains("line1"));
        assert!(output.contains("line2"));
        assert!(output.contains("line3"));
        assert!(output.contains("Exit code: 0"));

        // Drop the context to close the event channel
        drop(context);

        // Check that events were emitted
        let events = events_collector.await.unwrap();
        assert_eq!(events.len(), 3, "Should have received 3 output chunks");
        assert_eq!(events[0].0, "test_call_123");
        assert_eq!(events[0].1, "line1");
        assert_eq!(events[0].2, "stdout");
        assert_eq!(events[0].3, 1);
    }

    #[tokio::test]
    async fn test_bash_tool_streaming_stderr() {
        use tokio::sync::mpsc;

        let tool = BashTool::new();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let context = ToolExecutionContext {
            tool_call_id: "test_call_456".to_string(),
            event_tx: Some(event_tx),
            parent_conversation_id: None,
        };

        let args = json!({
            "command": "echo 'stdout line' && echo 'stderr line' >&2"
        });

        // Spawn task to collect events
        let events_collector = tokio::spawn(async move {
            let mut collected_events = Vec::new();
            while let Some(event) = event_rx.recv().await {
                if let AgentEvent::BashOutputChunk {
                    output_line,
                    stream_type,
                    ..
                } = event
                {
                    collected_events.push((output_line, stream_type));
                }
            }
            collected_events
        });

        // Execute command with context
        let result = tool.execute(&args, &context).await;
        assert!(result.is_ok());

        // Drop the context to close the event channel
        drop(context);

        // Check that we got both stdout and stderr events
        let events = events_collector.await.unwrap();
        assert_eq!(events.len(), 2);

        // Find stdout and stderr events
        let stdout_events: Vec<_> = events
            .iter()
            .filter(|(_, stream_type)| stream_type == "stdout")
            .collect();
        let stderr_events: Vec<_> = events
            .iter()
            .filter(|(_, stream_type)| stream_type == "stderr")
            .collect();

        assert_eq!(stdout_events.len(), 1);
        assert_eq!(stderr_events.len(), 1);
        assert_eq!(stdout_events[0].0, "stdout line");
        assert_eq!(stderr_events[0].0, "stderr line");
    }

    #[tokio::test]
    async fn test_bash_tool_no_streaming_without_context() {
        let tool = BashTool::new();

        let args = json!({
            "command": "echo 'test output'"
        });

        let context = ToolExecutionContext {
            tool_call_id: "test".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };

        // Execute without streaming event channel
        let result = tool.execute(&args, &context).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test output"));
    }
}
