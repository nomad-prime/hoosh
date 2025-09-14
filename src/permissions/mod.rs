use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::Path;

/// Permission level for different operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionLevel {
    /// Allow operation without asking
    Allow,
    /// Ask user for confirmation
    Ask,
    /// Deny operation
    Deny,
}

/// Types of operations that require permission
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationType {
    ReadFile(String),
    WriteFile(String),
    CreateFile(String),
    DeleteFile(String),
    ExecuteBash(String),
    ListDirectory(String),
}

impl OperationType {
    pub fn description(&self) -> String {
        match self {
            OperationType::ReadFile(path) => format!("read file '{}'", path),
            OperationType::WriteFile(path) => format!("write to file '{}'", path),
            OperationType::CreateFile(path) => format!("create file '{}'", path),
            OperationType::DeleteFile(path) => format!("delete file '{}'", path),
            OperationType::ExecuteBash(cmd) => format!("execute bash command: '{}'", cmd),
            OperationType::ListDirectory(path) => format!("list directory '{}'", path),
        }
    }

    pub fn is_safe_operation(&self) -> bool {
        matches!(
            self,
            OperationType::ReadFile(_) | OperationType::ListDirectory(_)
        )
    }

    pub fn is_destructive(&self) -> bool {
        matches!(self, OperationType::DeleteFile(_))
            || match self {
                OperationType::ExecuteBash(cmd) => Self::is_destructive_command(cmd),
                _ => false,
            }
    }

    fn is_destructive_command(command: &str) -> bool {
        let dangerous_patterns = [
            "rm", "rmdir", "del", "delete", "unlink", "truncate", "dd", "mkfs", "format",
            "shutdown", "reboot", "halt", "poweroff",
        ];

        let command_lower = command.to_lowercase();
        dangerous_patterns
            .iter()
            .any(|&pattern| command_lower.contains(pattern))
    }
}

/// Permission manager for handling operation permissions
pub struct PermissionManager {
    skip_permissions: bool,
    default_permission: PermissionLevel,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            skip_permissions: false,
            default_permission: PermissionLevel::Ask,
        }
    }

    pub fn with_skip_permissions(mut self, skip: bool) -> Self {
        self.skip_permissions = skip;
        self
    }

    pub fn with_default_permission(mut self, level: PermissionLevel) -> Self {
        self.default_permission = level;
        self
    }

    /// Check if an operation is allowed
    pub async fn check_permission(&self, operation: &OperationType) -> Result<bool> {
        // If permissions are skipped, allow everything
        if self.skip_permissions {
            return Ok(true);
        }

        // Safe operations are always allowed
        if operation.is_safe_operation() {
            return Ok(true);
        }

        // Handle based on default permission level
        match self.default_permission {
            PermissionLevel::Allow => Ok(true),
            PermissionLevel::Deny => Ok(false),
            PermissionLevel::Ask => self.ask_user_permission(operation).await,
        }
    }

    /// Ask user for permission interactively
    async fn ask_user_permission(&self, operation: &OperationType) -> Result<bool> {
        let warning_emoji = if operation.is_destructive() {
            "⚠️"
        } else {
            "🔒"
        };

        println!(
            "{} Permission required to {}",
            warning_emoji,
            operation.description()
        );

        if operation.is_destructive() {
            println!("⚠️  WARNING: This operation may be destructive!");
        }

        print!("Allow this operation? [y/N]: ");
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read user input")?;

        let response = input.trim().to_lowercase();
        Ok(matches!(response.as_str(), "y" | "yes"))
    }

    /// Request permission for file read operation
    pub async fn request_file_read(&self, file_path: &str) -> Result<bool> {
        let operation = OperationType::ReadFile(file_path.to_string());
        self.check_permission(&operation).await
    }

    /// Request permission for file write operation
    pub async fn request_file_write(&self, file_path: &str) -> Result<bool> {
        let path = Path::new(file_path);

        let operation = if path.exists() {
            OperationType::WriteFile(file_path.to_string())
        } else {
            OperationType::CreateFile(file_path.to_string())
        };

        self.check_permission(&operation).await
    }

    /// Request permission for file deletion
    pub async fn request_file_delete(&self, file_path: &str) -> Result<bool> {
        let operation = OperationType::DeleteFile(file_path.to_string());
        self.check_permission(&operation).await
    }

    /// Request permission for bash command execution
    pub async fn request_bash_execution(&self, command: &str) -> Result<bool> {
        let operation = OperationType::ExecuteBash(command.to_string());
        self.check_permission(&operation).await
    }

    /// Request permission for directory listing
    pub async fn request_directory_list(&self, dir_path: &str) -> Result<bool> {
        let operation = OperationType::ListDirectory(dir_path.to_string());
        self.check_permission(&operation).await
    }

    /// Check if permissions are being enforced
    pub fn is_enforcing(&self) -> bool {
        !self.skip_permissions
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Custom error types for permission operations
#[derive(Debug, thiserror::Error)]
pub enum PermissionError {
    #[error("Permission denied for operation: {0}")]
    PermissionDenied(String),

    #[error("Permission check failed: {0}")]
    CheckFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_safety_classification() {
        let read_op = OperationType::ReadFile("test.txt".to_string());
        let write_op = OperationType::WriteFile("test.txt".to_string());
        let delete_op = OperationType::DeleteFile("test.txt".to_string());
        let bash_safe = OperationType::ExecuteBash("echo hello".to_string());
        let bash_dangerous = OperationType::ExecuteBash("rm -rf /".to_string());

        assert!(read_op.is_safe_operation());
        assert!(!write_op.is_safe_operation());
        assert!(!delete_op.is_safe_operation());

        assert!(!read_op.is_destructive());
        assert!(delete_op.is_destructive());
        assert!(!bash_safe.is_destructive());
        assert!(bash_dangerous.is_destructive());
    }

    #[tokio::test]
    async fn test_permission_manager_skip_permissions() {
        let manager = PermissionManager::new().with_skip_permissions(true);
        let operation = OperationType::WriteFile("test.txt".to_string());

        let result = manager.check_permission(&operation).await.unwrap();
        assert!(result); // Should allow when permissions are skipped
    }

    #[tokio::test]
    async fn test_permission_manager_safe_operations() {
        let manager = PermissionManager::new(); // Default: ask for permission
        let read_op = OperationType::ReadFile("test.txt".to_string());
        let list_op = OperationType::ListDirectory("./".to_string());

        // Safe operations should be allowed without asking
        assert!(manager.check_permission(&read_op).await.unwrap());
        assert!(manager.check_permission(&list_op).await.unwrap());
    }

    #[test]
    fn test_operation_description() {
        let ops = vec![
            OperationType::ReadFile("file.txt".to_string()),
            OperationType::WriteFile("file.txt".to_string()),
            OperationType::CreateFile("new.txt".to_string()),
            OperationType::DeleteFile("old.txt".to_string()),
            OperationType::ExecuteBash("ls -la".to_string()),
            OperationType::ListDirectory("/home".to_string()),
        ];

        for op in ops {
            let desc = op.description();
            assert!(!desc.is_empty());
            assert!(desc.len() > 5); // Should have meaningful descriptions
        }
    }
}
