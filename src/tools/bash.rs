use crate::tools::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
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
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            timeout_seconds: 30, // Default 30 second timeout
            allow_dangerous_commands: false,
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            working_directory: working_dir,
            timeout_seconds: 30,
            allow_dangerous_commands: false,
        }
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    pub fn allow_dangerous_commands(mut self, allow: bool) -> Self {
        self.allow_dangerous_commands = allow;
        self
    }

    /// Check if a command contains potentially dangerous operations
    fn is_dangerous_command(&self, command: &str) -> bool {
        if self.allow_dangerous_commands {
            return false;
        }

        let dangerous_patterns = [
            "rm -rf",
            "rm -fr",
            "sudo",
            "su ",
            "passwd",
            "chmod 777",
            "chown",
            "dd if=",
            "mkfs",
            "fdisk",
            "format",
            "del /f",
            "rmdir /s",
            "> /dev/",
            "shutdown",
            "reboot",
            "halt",
            "init 0",
            "init 6",
            "poweroff",
            "systemctl",
            "service ",
            "kill -9",
            "killall",
            "pkill",
            "forkbomb",
            ":(){ :|:& };:",
            "curl | sh",
            "wget | sh",
            "curl | bash",
            "wget | bash",
        ];

        let command_lower = command.to_lowercase();
        dangerous_patterns
            .iter()
            .any(|&pattern| command_lower.contains(pattern))
    }

    /// Sanitize command to prevent some basic injection attempts
    fn sanitize_command(&self, command: &str) -> String {
        // Remove null bytes and other control characters that could be problematic
        command
            .chars()
            .filter(|c| !c.is_control() || c.is_whitespace())
            .collect()
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
    async fn execute(&self, args: &serde_json::Value) -> Result<String> {
        let args: BashArgs =
            serde_json::from_value(args.clone()).context("Invalid arguments for bash tool")?;

        let command = self.sanitize_command(&args.command);

        // Security check for dangerous commands
        if self.is_dangerous_command(&command) {
            anyhow::bail!(
                "Command rejected for security reasons: '{}'. \
                 Use --skip-permissions flag to bypass this check.",
                command
            );
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
            let output = cmd
                .output()
                .await
                .with_context(|| format!("Failed to execute command: {}", command))?;

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

            Ok::<String, anyhow::Error>(result)
        };

        // Apply timeout
        match timeout(timeout_duration, command_future).await {
            Ok(result) => result,
            Err(_) => {
                anyhow::bail!(
                    "Command timed out after {} seconds: {}",
                    timeout_duration.as_secs(),
                    command
                );
            }
        }
    }

    fn tool_name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        "Execute bash commands safely with timeout and security restrictions."
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
        let tool = BashTool::new();
        let args = serde_json::json!({
            "command": "rm -rf /"
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("security reasons"));
    }

    #[tokio::test]
    async fn test_bash_tool_allow_dangerous_commands() {
        let tool = BashTool::new().allow_dangerous_commands(true);
        let args = serde_json::json!({
            "command": "echo 'This would be dangerous: rm -rf /'"
        });

        // This should work because we're just echoing, not actually running rm
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
        let args = serde_json::json!({
            "command": "sleep 5" // This should timeout
        });

        let result = tool.execute(&args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }
}
