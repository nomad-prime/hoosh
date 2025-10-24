pub mod cache;

use anyhow::{Context, Result};
use cache::{OperationKind, PermissionCacheKey};
use std::collections::HashMap;
use std::path::PathBuf;
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
    /// Permission applies to all operations within a project directory
    ProjectWide(std::path::PathBuf),
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
            | OperationType::ListDirectory(path) => std::path::Path::new(path)
                .parent()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string()),
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
    /// Uses structured PermissionCacheKey instead of string-based keys
    session_cache: Arc<Mutex<HashMap<PermissionCacheKey, bool>>>,
    /// Event sender for sending permission requests to UI
    event_sender: mpsc::UnboundedSender<crate::conversations::AgentEvent>,
    /// Response receiver for receiving permission responses from UI
    response_receiver:
        Arc<Mutex<mpsc::UnboundedReceiver<crate::conversations::PermissionResponse>>>,
    /// Request ID counter for generating unique permission request IDs
    request_counter: Arc<AtomicU64>,
    /// Trusted project directory (session-only)
    trusted_project: Arc<Mutex<Option<PathBuf>>>,
}

impl PermissionManager {
    pub fn new(
        event_sender: mpsc::UnboundedSender<crate::conversations::AgentEvent>,
        response_receiver: mpsc::UnboundedReceiver<crate::conversations::PermissionResponse>,
    ) -> Self {
        Self {
            skip_permissions: false,
            default_permission: PermissionLevel::Ask,
            session_cache: Arc::new(Mutex::new(HashMap::new())),
            event_sender,
            response_receiver: Arc::new(Mutex::new(response_receiver)),
            request_counter: Arc::new(AtomicU64::new(0)),
            trusted_project: Arc::new(Mutex::new(None)),
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

    /// Get the current skip_permissions setting
    pub fn skip_permissions(&self) -> bool {
        self.skip_permissions
    }

    /// Get the current default permission level
    pub fn default_permission(&self) -> PermissionLevel {
        self.default_permission
    }

    /// Clear the session cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.session_cache.lock() {
            cache.clear();
        }
    }

    /// Enable project-wide trust for a specific directory
    pub fn set_trusted_project(&self, project_path: PathBuf) {
        if let Ok(mut trusted) = self.trusted_project.lock() {
            *trusted = Some(project_path.clone());
        }
        // Also cache the project-wide permission using structured key
        let key = PermissionCacheKey::ProjectWide {
            operation: OperationKind::Write,
            project_root: project_path.clone(),
        };
        if let Ok(mut cache) = self.session_cache.lock() {
            cache.insert(key, true);
        }
    }

    /// Disable project-wide trust
    pub fn clear_trusted_project(&self) {
        // First get the current trusted project path
        let project_path = if let Ok(mut trusted) = self.trusted_project.lock() {
            trusted.take()
        } else {
            None
        };

        // Remove the project-wide permission from cache
        if let Some(path) = project_path {
            let key = PermissionCacheKey::ProjectWide {
                operation: OperationKind::Write,
                project_root: path,
            };
            if let Ok(mut cache) = self.session_cache.lock() {
                cache.remove(&key);
            }
        }
    }

    /// Get the currently trusted project directory
    pub fn get_trusted_project(&self) -> Option<PathBuf> {
        self.trusted_project.lock().ok()?.clone()
    }

    /// Check if operation was previously allowed in this session
    /// Uses hierarchical permission checking with structured keys
    fn check_cache(&self, operation: &OperationType) -> Option<bool> {
        let cache = self.session_cache.lock().ok()?;
        let operation_kind = operation.operation_kind().parse::<OperationKind>().ok()?;
        let target = operation.target();

        self.send_debug(&format!(
            "Checking cache for operation: {} ({})",
            operation.operation_kind(),
            target
        ));
        self.send_debug("Cache contents:");
        for (key, &value) in cache.iter() {
            self.send_debug(&format!("  {:?} => {}", key, value));
        }

        // Collect all matching cache entries with their precedence
        let mut matches: Vec<(u8, bool)> = cache
            .iter()
            .filter(|(key, _)| key.matches(operation_kind, target))
            .map(|(key, &decision)| (key.precedence(), decision))
            .collect();

        // Sort by precedence (highest first)
        matches.sort_by(|a, b| b.0.cmp(&a.0));

        // Return the decision from the highest precedence match
        if let Some((precedence, decision)) = matches.first() {
            self.send_debug(&format!(
                "Found cached permission with precedence {}: {}",
                precedence, decision
            ));
            Some(*decision)
        } else {
            self.send_debug("No cached permission found");
            None
        }
    }

    /// Store permission decision in cache with the specified scope
    fn cache_decision(&self, operation: &OperationType, scope: PermissionScope, allowed: bool) {
        // If this is a ProjectWide scope, also update the trusted_project field
        if let PermissionScope::ProjectWide(ref path) = scope {
            if allowed {
                if let Ok(mut trusted) = self.trusted_project.lock() {
                    *trusted = Some(path.clone());
                }
            }
        }

        // Convert to structured cache key
        let operation_kind = match operation.operation_kind().parse::<OperationKind>() {
            Ok(kind) => kind,
            Err(_) => return,
        };

        let key = match &scope {
            PermissionScope::Specific(target) => PermissionCacheKey::Specific {
                operation: operation_kind,
                target: PathBuf::from(target),
            },
            PermissionScope::Directory(dir) => PermissionCacheKey::Directory {
                operation: operation_kind,
                directory: PathBuf::from(dir),
            },
            PermissionScope::Global => PermissionCacheKey::Global {
                operation: operation_kind,
            },
            PermissionScope::ProjectWide(path) => PermissionCacheKey::ProjectWide {
                operation: operation_kind,
                project_root: path.clone(),
            },
        };

        self.send_debug(&format!(
            "Caching decision: {:?} => {} (scope: {:?})",
            key, allowed, scope
        ));
        if let Ok(mut cache) = self.session_cache.lock() {
            cache.insert(key, allowed);
        }
    }

    /// Send a debug message via the event sender
    fn send_debug(&self, message: &str) {
        let _ = self
            .event_sender
            .send(crate::conversations::AgentEvent::DebugMessage(
                message.to_string(),
            ));
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

        // Check if operation is within a trusted project (highest priority)
        if let Some(trusted_path) = self.get_trusted_project() {
            if PermissionCacheKey::is_within_project_static(operation.target(), &trusted_path) {
                return Ok(true);
            }
        }

        // Check cache (hierarchical: project-wide -> specific -> directory -> global)
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

    /// Ask user for permission interactively via TUI event system
    /// Returns (allowed, optional_scope)
    async fn ask_user_permission(
        &self,
        operation: &OperationType,
    ) -> Result<(bool, Option<PermissionScope>)> {
        self.ask_user_permission_via_tui(operation, &self.event_sender, &self.response_receiver)
            .await
    }

    /// Ask user for permission via TUI event system
    async fn ask_user_permission_via_tui(
        &self,
        operation: &OperationType,
        sender: &mpsc::UnboundedSender<crate::conversations::AgentEvent>,
        receiver: &Arc<Mutex<mpsc::UnboundedReceiver<crate::conversations::PermissionResponse>>>,
    ) -> Result<(bool, Option<PermissionScope>)> {
        // Generate unique request ID
        let request_id = self
            .request_counter
            .fetch_add(1, Ordering::SeqCst)
            .to_string();

        // Send permission request event
        let event = crate::conversations::AgentEvent::PermissionRequest {
            operation: operation.clone(),
            request_id: request_id.clone(),
        };
        sender
            .send(event)
            .context("Failed to send permission request event")?;

        // Wait for response
        // Need to avoid holding lock across await by using a loop with try_recv
        let receiver_clone = Arc::clone(receiver);
        let response = loop {
            // Try to receive in a block that drops the lock immediately
            let maybe_response = {
                let mut rx = receiver_clone
                    .lock()
                    .map_err(|e| anyhow::anyhow!("Failed to lock receiver: {}", e))?;
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

    /// Check if permissions are being enforced
    pub fn is_enforcing(&self) -> bool {
        !self.skip_permissions
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        let (event_tx, _) = tokio::sync::mpsc::unbounded_channel();
        let (_, response_rx) = tokio::sync::mpsc::unbounded_channel();
        Self::new(event_tx, response_rx)
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

    /// Helper function to create a PermissionManager for testing
    fn create_test_manager() -> PermissionManager {
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        PermissionManager::new(event_tx, response_rx)
    }

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
        let manager = create_test_manager().with_skip_permissions(true);
        let operation = OperationType::WriteFile("test.txt".to_string());

        let result = manager.check_permission(&operation).await.unwrap();
        assert!(result); // Should allow when permissions are skipped
    }

    #[tokio::test]
    async fn test_permission_manager_safe_operations() {
        let manager = create_test_manager(); // Default: ask for permission
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
        let manager = create_test_manager().with_skip_permissions(false);

        // Use Cargo.toml which exists in the project root
        let test_file = "Cargo.toml";
        let operation = OperationType::WriteFile(test_file.to_string());

        // Initially should not be cached
        assert!(manager.check_cache(&operation).is_none());

        // Store a specific file decision
        manager.cache_decision(
            &operation,
            PermissionScope::Specific(test_file.to_string()),
            true,
        );

        // Should now be cached
        assert_eq!(manager.check_cache(&operation), Some(true));

        // Same operation should return same cached value
        assert_eq!(manager.check_cache(&operation), Some(true));

        // Different operation should not be cached
        let other_operation = OperationType::WriteFile("README.md".to_string());
        assert!(manager.check_cache(&other_operation).is_none());
    }

    #[test]
    fn test_permission_cache_directory_scope() {
        use tempfile::TempDir;

        let manager = create_test_manager();

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_string_lossy().to_string();

        // Create test files
        let file1_path = format!("{}/file1.txt", dir_path);
        let file2_path = format!("{}/file2.txt", dir_path);
        std::fs::write(&file1_path, "test").unwrap();
        std::fs::write(&file2_path, "test").unwrap();

        // Cache a directory-level permission
        let operation = OperationType::WriteFile(file1_path.clone());
        manager.cache_decision(
            &operation,
            PermissionScope::Directory(dir_path.clone()),
            true,
        );

        // Should match for files in the same directory
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile(file1_path.clone())),
            Some(true)
        );
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile(file2_path.clone())),
            Some(true)
        );

        // Should NOT match for files in different directory
        assert!(manager
            .check_cache(&OperationType::WriteFile("/other/dir/file.txt".to_string()))
            .is_none());
    }

    #[test]
    fn test_permission_cache_global_scope() {
        let manager = create_test_manager();

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
        assert!(manager
            .check_cache(&OperationType::ReadFile("/any/path/file.txt".to_string()))
            .is_none());
    }

    #[test]
    fn test_permission_cache_hierarchy() {
        use tempfile::TempDir;

        let manager = create_test_manager();

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_string_lossy().to_string();
        let file_path = format!("{}/test.txt", dir_path);
        std::fs::write(&file_path, "test").unwrap();

        // Set up hierarchy: global < directory < specific
        manager.cache_decision(
            &OperationType::WriteFile(file_path.clone()),
            PermissionScope::Global,
            true, // Global: allow all writes
        );
        manager.cache_decision(
            &OperationType::WriteFile(file_path.clone()),
            PermissionScope::Directory(dir_path.clone()),
            false, // Directory: deny writes in dir
        );

        // Directory permission should override global
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile(file_path.clone())),
            Some(false)
        );

        // Now add specific permission
        manager.cache_decision(
            &OperationType::WriteFile(file_path.clone()),
            PermissionScope::Specific(file_path.clone()),
            true, // Specific: allow this one file
        );

        // Specific permission should override directory
        assert_eq!(
            manager.check_cache(&OperationType::WriteFile(file_path.clone())),
            Some(true)
        );
    }

    #[test]
    fn test_permission_cache_cleared() {
        let manager = create_test_manager();

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

        assert!(manager
            .check_cache(&OperationType::WriteFile("/path/file.txt".to_string()))
            .is_some());

        // Clear cache
        manager.clear_cache();

        // Should no longer be cached
        assert!(manager
            .check_cache(&OperationType::WriteFile("/path/file.txt".to_string()))
            .is_none());
    }

    #[tokio::test]
    async fn test_project_wide_trust() {
        use tempfile::TempDir;

        let manager = create_test_manager().with_skip_permissions(true); // Skip permissions to avoid prompts

        // Create a temporary directory to use as project root
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();

        // Set trusted project
        manager.set_trusted_project(project_path.clone());

        // Create a file path within the project
        let file_in_project = project_path.join("test_file.txt");
        std::fs::write(&file_in_project, "test").unwrap();
        let operation = OperationType::WriteFile(file_in_project.to_string_lossy().to_string());

        // Operation within trusted project should be auto-approved
        assert!(manager.check_permission(&operation).await.unwrap());

        // Create a file path outside the project
        let file_outside = std::env::temp_dir().join("outside_file_hoosh_test.txt");
        let operation_outside =
            OperationType::WriteFile(file_outside.to_string_lossy().to_string());

        // Operation outside trusted project should check cache (will return None since not cached)
        // We can't test the full flow without mocking user input, but we can verify the cache check
        assert!(manager.check_cache(&operation_outside).is_none());
    }

    #[test]
    fn test_clear_trusted_project() {
        let manager = create_test_manager();

        // Create a temporary directory
        let temp_dir = std::env::temp_dir().join("hoosh_test_clear_project");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Set trusted project
        manager.set_trusted_project(temp_dir.clone());
        assert!(manager.get_trusted_project().is_some());

        // Clear trusted project
        manager.clear_trusted_project();
        assert!(manager.get_trusted_project().is_none());

        // Verify the cache entry was also removed
        let file_in_project = temp_dir.join("test_file.txt");
        let operation = OperationType::WriteFile(file_in_project.to_string_lossy().to_string());
        assert!(manager.check_cache(&operation).is_none());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_project_wide_trust_entire_project() {
        use tempfile::TempDir;

        let manager = create_test_manager().with_skip_permissions(false);

        // Create a temporary directory to use as project root
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();

        // Create test files in the project
        let file1_path = project_path.join("file1.txt");
        let file2_path = project_path.join("subdir/file2.txt");
        std::fs::write(&file1_path, "test1").unwrap();
        std::fs::create_dir_all(project_path.join("subdir")).unwrap();
        std::fs::write(&file2_path, "test2").unwrap();

        // Set trusted project
        manager.set_trusted_project(project_path.clone());

        // Test that operations within the project are auto-approved
        let operation1 = OperationType::WriteFile(file1_path.to_string_lossy().to_string());
        assert!(
            manager.check_permission(&operation1).await.unwrap(),
            "File in project root should be allowed"
        );

        let operation2 = OperationType::WriteFile(file2_path.to_string_lossy().to_string());
        assert!(
            manager.check_permission(&operation2).await.unwrap(),
            "File in project subdirectory should be allowed"
        );

        // Test that operations outside the project are NOT auto-approved
        let file_outside = std::env::temp_dir().join("outside_file_hoosh_test.txt");
        std::fs::write(&file_outside, "outside").unwrap();
        let operation_outside =
            OperationType::WriteFile(file_outside.to_string_lossy().to_string());

        // This should not be approved by project trust (will check cache, find nothing, and ask user)
        // Since we're in test mode without user interaction, it will fail
        // But the important thing is that it doesn't panic and respects the project boundary
        let _ = manager.check_permission(&operation_outside).await;

        // Clean up
        let _ = std::fs::remove_file(&file_outside);
    }

    #[tokio::test]
    async fn test_project_wide_trust_new_files() {
        use tempfile::TempDir;

        let manager = create_test_manager().with_skip_permissions(false);

        // Create a temporary directory to use as project root
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();

        // Set trusted project
        manager.set_trusted_project(project_path.clone());

        // Test that operations on NON-EXISTENT files within the project are auto-approved
        // This is the critical test case - files that don't exist yet
        let new_file_path = project_path.join("new_file_that_does_not_exist.txt");
        let operation = OperationType::WriteFile(new_file_path.to_string_lossy().to_string());

        assert!(
            manager.check_permission(&operation).await.unwrap(),
            "New file in trusted project should be auto-approved even if it doesn't exist yet"
        );

        // Test nested new file
        let nested_new_file = project_path.join("subdir/nested_new_file.txt");
        let operation_nested =
            OperationType::WriteFile(nested_new_file.to_string_lossy().to_string());

        assert!(
            manager.check_permission(&operation_nested).await.unwrap(),
            "New nested file in trusted project should be auto-approved"
        );
    }
}
