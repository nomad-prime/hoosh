pub mod storage;

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionLevel {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionScope {
    Specific(String),
    ProjectWide(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationDisplay {
    pub name: String,
    pub approval_title: String,
    pub approval_prompt: String,
    pub persistent_approval: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationType {
    operation: String,
    target: String,
    is_safe: bool,
    is_destructive: bool,
    parent_dir: Option<String>,
    display: OperationDisplay,
}

impl OperationType {
    pub fn new(
        operation: impl Into<String>,
        target: impl Into<String>,
        is_safe: bool,
        is_destructive: bool,
        parent_dir: Option<String>,
        display: OperationDisplay,
    ) -> Self {
        let operation = operation.into();
        Self {
            operation,
            target: target.into(),
            is_safe,
            is_destructive,
            parent_dir,
            display,
        }
    }

    pub fn description(&self) -> String {
        format!("{} '{}'", self.operation, self.target)
    }

    pub fn kind(&self) -> &str {
        &self.operation
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn parent_directory(&self) -> Option<String> {
        self.parent_dir.clone()
    }

    pub fn is_safe_operation(&self) -> bool {
        self.is_safe
    }

    pub fn is_destructive(&self) -> bool {
        self.is_destructive
    }

    pub fn display(&self) -> &OperationDisplay {
        &self.display
    }
}

#[derive(Debug, Clone)]
pub struct PermissionsInfo {
    pub allow_count: usize,
    pub deny_count: usize,
}

#[derive(Clone)]
pub struct PermissionManager {
    skip_permissions: bool,
    default_permission: PermissionLevel,
    event_sender: mpsc::UnboundedSender<crate::conversations::AgentEvent>,
    response_receiver:
        Arc<Mutex<mpsc::UnboundedReceiver<crate::conversations::PermissionResponse>>>,
    request_counter: Arc<AtomicU64>,
    project_root: Arc<Mutex<Option<PathBuf>>>,
    permissions_file: Arc<Mutex<storage::PermissionsFile>>,
}

impl PermissionManager {
    pub fn new(
        event_sender: mpsc::UnboundedSender<crate::conversations::AgentEvent>,
        response_receiver: mpsc::UnboundedReceiver<crate::conversations::PermissionResponse>,
    ) -> Self {
        Self {
            skip_permissions: false,
            default_permission: PermissionLevel::Ask,
            event_sender,
            response_receiver: Arc::new(Mutex::new(response_receiver)),
            request_counter: Arc::new(AtomicU64::new(0)),
            project_root: Arc::new(Mutex::new(None)),
            permissions_file: Arc::new(Mutex::new(storage::PermissionsFile::default())),
        }
    }

    pub fn with_project_root(self, project_root: PathBuf) -> Self {
        let permissions = storage::PermissionsFile::load_permissions_safe(&project_root);

        if let Ok(mut root) = self.project_root.try_lock() {
            *root = Some(project_root);
        }
        if let Ok(mut perms) = self.permissions_file.try_lock() {
            *perms = permissions;
        }

        self
    }

    pub fn save_permissions(&self) -> Result<()> {
        let project_root = self
            .project_root
            .try_lock()
            .ok()
            .and_then(|r| r.clone())
            .context("No project root set")?;

        let permissions_file = self
            .permissions_file
            .try_lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock permissions file: {}", e))?;

        permissions_file.save_permissions(&project_root)
    }

    pub fn add_permission_rule(
        &self,
        operation: &OperationType,
        scope: &PermissionScope,
        allowed: bool,
    ) -> Result<()> {
        let mut permissions_file = self
            .permissions_file
            .try_lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock permissions file: {}", e))?;

        if let PermissionScope::ProjectWide(_) = scope {
            match operation.kind() {
                "bash" => {
                    permissions_file
                        .add_permission(storage::PermissionRule::ops_rule("bash", "*"), allowed);
                }
                "write" | "edit" => {
                    permissions_file
                        .add_permission(storage::PermissionRule::ops_rule("write", "*"), allowed);
                    permissions_file
                        .add_permission(storage::PermissionRule::ops_rule("edit", "*"), allowed);
                }
                _ => {
                    let rule = self.create_permission_rule(operation, scope);
                    permissions_file.add_permission(rule, allowed);
                }
            }
        } else {
            let rule = self.create_permission_rule(operation, scope);
            permissions_file.add_permission(rule, allowed);
        }

        drop(permissions_file);
        self.save_permissions()
    }

    fn create_permission_rule(
        &self,
        operation: &OperationType,
        scope: &PermissionScope,
    ) -> storage::PermissionRule {
        let operation_str = operation.kind();

        match scope {
            PermissionScope::Specific(target) => {
                storage::PermissionRule::ops_rule(operation_str, target.clone())
            }
            PermissionScope::ProjectWide(_) => {
                storage::PermissionRule::ops_rule(operation_str, "*")
            }
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

    pub fn skip_permissions(&self) -> bool {
        self.skip_permissions
    }

    pub fn default_permission(&self) -> PermissionLevel {
        self.default_permission
    }

    pub fn get_permissions_info(&self) -> PermissionsInfo {
        let permissions_file = self.permissions_file.try_lock().ok();
        match permissions_file {
            Some(perms) => PermissionsInfo {
                allow_count: perms.allow.len(),
                deny_count: perms.deny.len(),
            },
            None => PermissionsInfo {
                allow_count: 0,
                deny_count: 0,
            },
        }
    }

    pub fn clear_all_permissions(&self) -> Result<()> {
        let mut permissions_file = self
            .permissions_file
            .try_lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock permissions file: {}", e))?;

        permissions_file.allow.clear();
        permissions_file.deny.clear();

        drop(permissions_file);
        self.save_permissions()
    }

    pub async fn check_permission(&self, operation: &OperationType) -> Result<bool> {
        if self.skip_permissions {
            return Ok(true);
        }

        if operation.is_safe_operation() {
            return Ok(true);
        }

        if let Some(persistent_decision) = self.check_persistent_permissions(operation) {
            return Ok(persistent_decision);
        }

        let (allowed, scope) = match self.default_permission {
            PermissionLevel::Allow => (true, None),
            PermissionLevel::Deny => (false, None),
            PermissionLevel::Ask => self.ask_user_permission(operation).await?,
        };

        if let Some(ref scope) = scope {
            let _ = self.add_permission_rule(operation, scope, allowed);
        }

        Ok(allowed)
    }

    fn check_persistent_permissions(&self, operation: &OperationType) -> Option<bool> {
        let permissions_file = self.permissions_file.try_lock().ok()?;
        permissions_file.check_permission(operation)
    }

    async fn ask_user_permission(
        &self,
        operation: &OperationType,
    ) -> Result<(bool, Option<PermissionScope>)> {
        let request_id = self
            .request_counter
            .fetch_add(1, Ordering::SeqCst)
            .to_string();

        let event = crate::conversations::AgentEvent::PermissionRequest {
            operation: operation.clone(),
            request_id: request_id.clone(),
        };
        self.event_sender
            .send(event)
            .context("Failed to send permission request event")?;

        let mut receiver = self.response_receiver.lock().await;

        let response = receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Permission response channel closed"))?;

        if response.request_id != request_id {
            anyhow::bail!("Permission response ID mismatch");
        }

        Ok((response.allowed, response.scope))
    }

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
        let read_op = OperationType::new(
            "read_file",
            "test.txt",
            true,
            false,
            Some("./".to_string()),
            OperationDisplay {
                name: "Read".to_string(),
                approval_title: "Read File".to_string(),
                approval_prompt: "Read File Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );
        let write_op = OperationType::new(
            "write_file",
            "test.txt",
            false,
            true,
            Some("./".to_string()),
            OperationDisplay {
                name: "Write".to_string(),
                approval_title: "Write File".to_string(),
                approval_prompt: "Write File Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );
        let edit_op = OperationType::new(
            "edit_file",
            "test.txt",
            false,
            true,
            Some("./".to_string()),
            OperationDisplay {
                name: "Edit".to_string(),
                approval_title: "Edit File".to_string(),
                approval_prompt: "Edit File Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );
        let list_op = OperationType::new(
            "list_directory",
            "./",
            true,
            false,
            None,
            OperationDisplay {
                name: "List".to_string(),
                approval_title: "List Directory".to_string(),
                approval_prompt: "List Directory Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );
        let bash_safe = OperationType::new(
            "bash",
            "echo hello",
            false,
            false,
            None,
            OperationDisplay {
                name: "Bash".to_string(),
                approval_title: "Bash Command".to_string(),
                approval_prompt: "Bash Command Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );
        let bash_dangerous = OperationType::new(
            "bash",
            "rm -rf /",
            false,
            true,
            None,
            OperationDisplay {
                name: "Bash".to_string(),
                approval_title: "Bash Command".to_string(),
                approval_prompt: "Bash Command Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );

        assert!(read_op.is_safe_operation());
        assert!(list_op.is_safe_operation());
        assert!(!write_op.is_safe_operation());
        assert!(!edit_op.is_safe_operation());

        assert!(!read_op.is_destructive());
        assert!(write_op.is_destructive());
        assert!(!bash_safe.is_destructive());
        assert!(bash_dangerous.is_destructive());
    }

    #[tokio::test]
    async fn test_permission_manager_skip_permissions() {
        let manager = create_test_manager().with_skip_permissions(true);
        let operation = OperationType::new(
            "write_file",
            "test.txt",
            false,
            true,
            Some("./".to_string()),
            OperationDisplay {
                name: "Write".to_string(),
                approval_title: "Write File".to_string(),
                approval_prompt: "Write File Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );

        let result = manager.check_permission(&operation).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_permission_manager_safe_operations() {
        let manager = create_test_manager();
        let read_op = OperationType::new(
            "read_file",
            "test.txt",
            true,
            false,
            Some("./".to_string()),
            OperationDisplay {
                name: "Read".to_string(),
                approval_title: "Read File".to_string(),
                approval_prompt: "Read File Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );
        let list_op = OperationType::new(
            "list_directory",
            "./",
            true,
            false,
            None,
            OperationDisplay {
                name: "List".to_string(),
                approval_title: "List Directory".to_string(),
                approval_prompt: "List Directory Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );

        assert!(manager.check_permission(&read_op).await.unwrap());
        assert!(manager.check_permission(&list_op).await.unwrap());
    }

    #[test]
    fn test_operation_description() {
        let ops = vec![
            OperationType::new(
                "read_file",
                "file.txt",
                true,
                false,
                Some(".".to_string()),
                OperationDisplay {
                    name: "Read".to_string(),
                    approval_title: "Read File".to_string(),
                    approval_prompt: "Read File Operation".to_string(),
                    persistent_approval: "Don't ask again".to_string(),
                },
            ),
            OperationType::new(
                "write_file",
                "file.txt",
                false,
                true,
                Some(".".to_string()),
                OperationDisplay {
                    name: "Write".to_string(),
                    approval_title: "Write File".to_string(),
                    approval_prompt: "Write File Operation".to_string(),
                    persistent_approval: "Don't ask again".to_string(),
                },
            ),
            OperationType::new(
                "edit_file",
                "new.txt",
                false,
                true,
                Some(".".to_string()),
                OperationDisplay {
                    name: "Edit".to_string(),
                    approval_title: "Edit File".to_string(),
                    approval_prompt: "Edit File Operation".to_string(),
                    persistent_approval: "Don't ask again".to_string(),
                },
            ),
            OperationType::new(
                "list_directory",
                "/home",
                true,
                false,
                Some("/".to_string()),
                OperationDisplay {
                    name: "List".to_string(),
                    approval_title: "List Directory".to_string(),
                    approval_prompt: "List Directory Operation".to_string(),
                    persistent_approval: "Don't ask again".to_string(),
                },
            ),
            OperationType::new(
                "bash",
                "ls -la",
                false,
                false,
                None,
                OperationDisplay {
                    name: "Bash".to_string(),
                    approval_title: "Bash Command".to_string(),
                    approval_prompt: "Bash Command Operation".to_string(),
                    persistent_approval: "Don't ask again".to_string(),
                },
            ),
        ];

        for op in ops {
            let desc = op.description();
            assert!(!desc.is_empty());
            assert!(desc.len() > 5);
        }
    }

    #[tokio::test]
    async fn test_permission_persistence() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();

        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        let manager =
            PermissionManager::new(event_tx, response_rx).with_project_root(project_path.clone());

        let test_file = project_path.join("test.txt");
        std::fs::write(&test_file, "test").unwrap();

        let operation = OperationType::new(
            "write_file",
            test_file.to_string_lossy().to_string(),
            false,
            true,
            test_file
                .parent()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string()),
            OperationDisplay {
                name: "Write".to_string(),
                approval_title: "Write File".to_string(),
                approval_prompt: "Write File Operation".to_string(),
                persistent_approval: "Don't ask again".to_string(),
            },
        );
        let scope = PermissionScope::Specific(test_file.to_string_lossy().to_string());

        let result = manager.add_permission_rule(&operation, &scope, true);
        assert!(result.is_ok(), "Should save permission successfully");

        let permissions_file_path = storage::PermissionsFile::get_permissions_path(&project_path);
        assert!(
            permissions_file_path.exists(),
            "Permissions file should be created"
        );

        let loaded = storage::PermissionsFile::load_permissions(&project_path).unwrap();
        assert!(
            !loaded.allow.is_empty(),
            "Should have at least one allow rule"
        );
    }
}
