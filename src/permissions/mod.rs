use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

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

/// Scope of a permission decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionScope {
    /// Permission applies to a specific file/command
    Specific(String),
    /// Permission applies to all operations in a directory
    Directory(String),
    /// Permission applies to all operations of this type
    Global,
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

    /// Get the base operation type (without the specific path/command)
    pub fn operation_kind(&self) -> &'static str {
        match self {
            OperationType::ReadFile(_) => "read",
            OperationType::WriteFile(_) => "write",
            OperationType::CreateFile(_) => "create",
            OperationType::DeleteFile(_) => "delete",
            OperationType::ExecuteBash(_) => "bash",
            OperationType::ListDirectory(_) => "list",
        }
    }

    /// Get the target (path or command) of this operation
    pub fn target(&self) -> &str {
        match self {
            OperationType::ReadFile(path) => path,
            OperationType::WriteFile(path) => path,
            OperationType::CreateFile(path) => path,
            OperationType::DeleteFile(path) => path,
            OperationType::ExecuteBash(cmd) => cmd,
            OperationType::ListDirectory(path) => path,
        }
    }

    /// Get the directory containing this operation's target (for file operations)
    pub fn parent_directory(&self) -> Option<String> {
        match self {
            OperationType::ReadFile(path)
            | OperationType::WriteFile(path)
            | OperationType::CreateFile(path)
            | OperationType::DeleteFile(path)
            | OperationType::ListDirectory(path) => {
                std::path::Path::new(path)
                    .parent()
                    .and_then(|p| p.to_str())
                    .map(|s| s.to_string())
            }
            OperationType::ExecuteBash(_) => None,
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
#[derive(Clone)]
pub struct PermissionManager {
    skip_permissions: bool,
    default_permission: PermissionLevel,
    /// Cache of permission decisions for this session
    /// Key is a string representation of the operation
    session_cache: Arc<Mutex<HashMap<String, bool>>>,
    /// Event sender for sending permission requests to UI
    event_sender: Option<mpsc::UnboundedSender<crate::conversations::AgentEvent>>,
    /// Response receiver for receiving permission responses from UI
    response_receiver: Option<Arc<Mutex<mpsc::UnboundedReceiver<crate::conversations::PermissionResponse>>>>,
    /// Request ID counter for generating unique permission request IDs
    request_counter: Arc<AtomicU64>,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            skip_permissions: false,
            default_permission: PermissionLevel::Ask,
            session_cache: Arc::new(Mutex::new(HashMap::new())),
            event_sender: None,
            response_receiver: None,
            request_counter: Arc::new(AtomicU64::new(0)),
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

    pub fn with_event_sender(
        mut self,
        sender: mpsc::UnboundedSender<crate::conversations::AgentEvent>,
    ) -> Self {
        self.event_sender = Some(sender);
        self
    }

    pub fn with_response_receiver(
        mut self,
        receiver: mpsc::UnboundedReceiver<crate::conversations::PermissionResponse>,
    ) -> Self {
        self.response_receiver = Some(Arc::new(Mutex::new(receiver)));
        self
    }

    /// Clear the session cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.session_cache.lock() {
            cache.clear();
        }
    }

    /// Get a cache key for a specific operation and scope
    fn get_cache_key_for_scope(&self, operation: &OperationType, scope: &PermissionScope) -> String {
        let kind = operation.operation_kind();
        match scope {
            PermissionScope::Specific(target) => format!("{}:specific:{}", kind, target),
            PermissionScope::Directory(dir) => format!("{}:dir:{}", kind, dir),
            PermissionScope::Global => format!("{}:*", kind),
        }
    }

    /// Check if operation was previously allowed in this session
    /// Checks in hierarchical order: specific file -> directory -> global
    fn check_cache(&self, operation: &OperationType) -> Option<bool> {
        let cache = self.session_cache.lock().ok()?;

        let kind = operation.operation_kind();
        let target = operation.target();

        // 1. Check specific file/command permission
        let specific_key = format!("{}:specific:{}", kind, target);
        if let Some(&decision) = cache.get(&specific_key) {
            return Some(decision);
        }

        // 2. Check directory permission (for file operations)
        if let Some(dir) = operation.parent_directory() {
            let dir_key = format!("{}:dir:{}", kind, dir);
            if let Some(&decision) = cache.get(&dir_key) {
                return Some(decision);
            }
        }

        // 3. Check global permission for this operation type
        let global_key = format!("{}:*", kind);
        if let Some(&decision) = cache.get(&global_key) {
            return Some(decision);
        }

        None
    }

    /// Store permission decision in cache with the specified scope
    fn cache_decision(&self, operation: &OperationType, scope: PermissionScope, allowed: bool) {
        let key = self.get_cache_key_for_scope(operation, &scope);
        if let Ok(mut cache) = self.session_cache.lock() {
            cache.insert(key, allowed);
        }
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

        // Check cache first (hierarchical: specific -> directory -> global)
        if let Some(cached_decision) = self.check_cache(operation) {
            return Ok(cached_decision);
        }

        // Handle based on default permission level
        let (allowed, scope) = match self.default_permission {
            PermissionLevel::Allow => (true, None),
            PermissionLevel::Deny => (false, None),
            PermissionLevel::Ask => self.ask_user_permission(operation).await?,
        };

        // Cache the decision if user chose to remember it
        if let Some(scope) = scope {
            self.cache_decision(operation, scope, allowed);
        }

        Ok(allowed)
    }

    /// Ask user for permission interactively
    /// Returns (allowed, optional_scope)
    async fn ask_user_permission(&self, operation: &OperationType) -> Result<(bool, Option<PermissionScope>)> {
        // If we have an event sender, use the TUI system
        if let (Some(sender), Some(receiver)) = (&self.event_sender, &self.response_receiver) {
            return self.ask_user_permission_via_tui(operation, sender, receiver).await;
        }

        // Otherwise, fall back to CLI/println approach
        self.ask_user_permission_via_cli(operation).await
    }

    /// Ask user for permission via TUI event system
    async fn ask_user_permission_via_tui(
        &self,
        operation: &OperationType,
        sender: &mpsc::UnboundedSender<crate::conversations::AgentEvent>,
        receiver: &Arc<Mutex<mpsc::UnboundedReceiver<crate::conversations::PermissionResponse>>>,
    ) -> Result<(bool, Option<PermissionScope>)> {
        // Generate unique request ID
        let request_id = self.request_counter.fetch_add(1, Ordering::SeqCst).to_string();

        // Send permission request event
        let event = crate::conversations::AgentEvent::PermissionRequest {
            operation: operation.clone(),
            request_id: request_id.clone(),
        };
        sender.send(event).context("Failed to send permission request event")?;

        // Wait for response
        // Need to avoid holding lock across await by using a loop with try_recv
        let receiver_clone = Arc::clone(receiver);
        let response = loop {
            // Try to receive in a block that drops the lock immediately
            let maybe_response = {
                let mut rx = receiver_clone.lock().unwrap();
                rx.try_recv().ok()
            };

            if let Some(response) = maybe_response {
                break response;
            }

            // Small sleep to avoid busy-waiting
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        };

        // Verify request ID matches
        if response.request_id != request_id {
            anyhow::bail!("Permission response ID mismatch");
        }

        Ok((response.allowed, response.scope))
    }

    /// Ask user for permission via CLI (fallback for non-TUI mode)
    async fn ask_user_permission_via_cli(&self, operation: &OperationType) -> Result<(bool, Option<PermissionScope>)> {
        println!(); // Add newline for spacing before permission prompt

        let warning_emoji = if operation.is_destructive() {
            "⚠️"
        } else {
            "🔒"
        };

        println!("{} Permission required to {}",
            warning_emoji,
            operation.description()
        );

        if operation.is_destructive() {
            println!("⚠️  WARNING: This operation may be destructive!");
        }

        println!("Allow this operation?");
        println!("  [y] Yes, once");
        println!("  [n] No");

        // Contextual message for 'a' option based on operation type
        match operation {
            OperationType::ExecuteBash(_) => {
                println!("  [a] Always for this command");
            }
            _ => {
                println!("  [a] Always for this file");
            }
        }

        // Show directory option for file operations
        if let Some(dir) = operation.parent_directory() {
            println!("  [d] Always for this directory ({})", dir);
        }

        println!("  [A] Always for all {} operations", operation.operation_kind());

        print!("Choice [y/N/a/d/A]: ");
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read user input")?;

        let response = input.trim();
        match response {
            "y" | "Y" | "yes" | "Yes" => {
                Ok((true, None)) // Allow once, don't cache
            }
            "a" => {
                let target = operation.target().to_string();
                println!("ℹ️  Permission for '{}' will be remembered for this session.", target);
                Ok((true, Some(PermissionScope::Specific(target))))
            }
            "d" | "D" => {
                if let Some(dir) = operation.parent_directory() {
                    println!("ℹ️  All {} operations in '{}' will be allowed for this session.",
                             operation.operation_kind(), dir);
                    Ok((true, Some(PermissionScope::Directory(dir))))
                } else {
                    println!("⚠️  Directory-based permission not available for this operation.");
                    println!("ℹ️  Using file-specific permission instead.");
                    let target = operation.target().to_string();
                    Ok((true, Some(PermissionScope::Specific(target))))
                }
            }
            "A" => {
                println!("ℹ️  All {} operations will be allowed for this session.",
                         operation.operation_kind());
                Ok((true, Some(PermissionScope::Global)))
            }
            _ => {
                Ok((false, None)) // Deny, no need to cache denials
            }
        }
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

    #[test]
    fn test_permission_cache_stores_decisions() {
        let manager = PermissionManager::new().with_skip_permissions(false);

        // Manually test cache storage and retrieval
        let operation = OperationType::WriteFile("/path/to/test.txt".to_string());

        // Initially should not be cached
        assert!(manager.check_cache(&operation).is_none());

        // Store a specific file decision
        manager.cache_decision(
            &operation,
            PermissionScope::Specific("/path/to/test.txt".to_string()),
            true,
        );

        // Should now be cached
        assert_eq!(manager.check_cache(&operation), Some(true));

        // Same operation should return same cached value
        assert_eq!(manager.check_cache(&operation), Some(true));

        // Different operation should not be cached
        let other_operation = OperationType::WriteFile("/path/to/other.txt".to_string());
        assert!(manager.check_cache(&other_operation).is_none());
    }

    #[test]
    fn test_permission_cache_directory_scope() {
        let manager = PermissionManager::new();

        // Cache a directory-level permission
        let operation = OperationType::WriteFile("/path/to/dir/file1.txt".to_string());
        manager.cache_decision(
            &operation,
            PermissionScope::Directory("/path/to/dir".to_string()),
            true,
        );

        // Should match for files in the same directory
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile("/path/to/dir/file1.txt".to_string())),
            Some(true)
        );
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile("/path/to/dir/file2.txt".to_string())),
            Some(true)
        );

        // Should NOT match for files in different directory
        assert!(
            manager.check_cache(&OperationType::WriteFile("/other/dir/file.txt".to_string())).is_none()
        );
    }

    #[test]
    fn test_permission_cache_global_scope() {
        let manager = PermissionManager::new();

        // Cache a global permission for write operations
        let operation = OperationType::WriteFile("/path/to/file.txt".to_string());
        manager.cache_decision(&operation, PermissionScope::Global, true);

        // Should match for any write operation
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile("/any/path/file.txt".to_string())),
            Some(true)
        );
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile("/other/file.txt".to_string())),
            Some(true)
        );

        // Should NOT match for different operation types
        assert!(
            manager.check_cache(&OperationType::ReadFile("/any/path/file.txt".to_string())).is_none()
        );
    }

    #[test]
    fn test_permission_cache_hierarchy() {
        let manager = PermissionManager::new();

        // Set up hierarchy: global < directory < specific
        manager.cache_decision(
            &OperationType::WriteFile("/dir/file.txt".to_string()),
            PermissionScope::Global,
            true, // Global: allow all writes
        );
        manager.cache_decision(
            &OperationType::WriteFile("/dir/file.txt".to_string()),
            PermissionScope::Directory("/dir".to_string()),
            false, // Directory: deny writes in /dir
        );

        // Directory permission should override global
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile("/dir/file.txt".to_string())),
            Some(false)
        );

        // Now add specific permission
        manager.cache_decision(
            &OperationType::WriteFile("/dir/file.txt".to_string()),
            PermissionScope::Specific("/dir/file.txt".to_string()),
            true, // Specific: allow this one file
        );

        // Specific permission should override directory
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile("/dir/file.txt".to_string())),
            Some(true)
        );
    }

    #[test]
    fn test_permission_cache_cleared() {
        let manager = PermissionManager::new();

        // Cache some decisions at different scopes
        manager.cache_decision(
            &OperationType::WriteFile("/path/file.txt".to_string()),
            PermissionScope::Specific("/path/file.txt".to_string()),
            true,
        );
        manager.cache_decision(
            &OperationType::WriteFile("/path/file.txt".to_string()),
            PermissionScope::Global,
            true,
        );

        assert!(manager.check_cache(&OperationType::WriteFile("/path/file.txt".to_string())).is_some());

        // Clear cache
        manager.clear_cache();

        // Should no longer be cached
        assert!(manager.check_cache(&OperationType::WriteFile("/path/file.txt".to_string())).is_none());
    }
}
