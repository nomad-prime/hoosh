pub mod pattern_matcher;
pub mod storage;
mod tool_permission;

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use tokio::sync::mpsc;

pub use crate::permissions::pattern_matcher::{
    BashPatternMatcher, FilePatternMatcher, PatternMatcher,
};
pub use crate::permissions::tool_permission::{ToolPermissionBuilder, ToolPermissionDescriptor};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionScope {
    Specific(String),
    ProjectWide(PathBuf),
}

#[derive(Debug, Clone)]
pub struct PermissionsInfo {
    pub allow_count: usize,
    pub deny_count: usize,
}

#[derive(Clone)]
pub struct PermissionManager {
    skip_permissions: bool,
    event_sender: mpsc::UnboundedSender<crate::agent::AgentEvent>,
    response_receiver: Arc<Mutex<mpsc::UnboundedReceiver<crate::agent::PermissionResponse>>>,
    request_counter: Arc<AtomicU64>,
    project_root: Arc<Mutex<Option<PathBuf>>>,
    permissions_file: Arc<Mutex<storage::PermissionsFile>>,
}

impl PermissionManager {
    pub fn new(
        event_sender: mpsc::UnboundedSender<crate::agent::AgentEvent>,
        response_receiver: mpsc::UnboundedReceiver<crate::agent::PermissionResponse>,
    ) -> Self {
        Self {
            skip_permissions: false,
            event_sender,
            response_receiver: Arc::new(Mutex::new(response_receiver)),
            request_counter: Arc::new(AtomicU64::new(0)),
            project_root: Arc::new(Mutex::new(None)),
            permissions_file: Arc::new(Mutex::new(storage::PermissionsFile::default())),
        }
    }

    pub fn with_project_root(
        self,
        project_root: PathBuf,
    ) -> Result<Self, storage::PermissionLoadError> {
        let permissions = storage::PermissionsFile::load_permissions_safe(&project_root)?;

        if let Ok(mut root) = self.project_root.try_lock() {
            *root = Some(project_root);
        }
        if let Ok(mut perms) = self.permissions_file.try_lock() {
            *perms = permissions;
        }

        Ok(self)
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

    pub fn add_tool_permission_rule(
        &self,
        descriptor: &ToolPermissionDescriptor,
        scope: &PermissionScope,
        allowed: bool,
    ) -> Result<()> {
        let mut permissions_file = self
            .permissions_file
            .try_lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock permissions file: {}", e))?;

        match scope {
            PermissionScope::ProjectWide(_) => {
                // Use suggested pattern if available (for bash commands), otherwise use "*"
                let pattern = descriptor.suggested_pattern().unwrap_or("*").to_string();
                permissions_file.add_permission(
                    storage::PermissionRule::ops_rule(descriptor.kind(), pattern),
                    allowed,
                );
            }
            PermissionScope::Specific(target) => {
                permissions_file.add_permission(
                    storage::PermissionRule::ops_rule(descriptor.kind(), target.clone()),
                    allowed,
                );
            }
        }

        drop(permissions_file);
        self.save_permissions()
    }

    pub fn with_skip_permissions(mut self, skip: bool) -> Self {
        self.skip_permissions = skip;
        self
    }

    pub fn skip_permissions(&self) -> bool {
        self.skip_permissions
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

    pub async fn check_tool_permission(
        &self,
        descriptor: &ToolPermissionDescriptor,
    ) -> Result<bool> {
        if self.skip_permissions {
            return Ok(true);
        }

        if let Some(persistent_decision) = self.check_persistent_tool_permission(descriptor) {
            return Ok(persistent_decision);
        }

        let (allowed, scope) = self.ask_user_tool_permission(descriptor).await?;

        if let Some(ref scope) = scope {
            let _ = self.add_tool_permission_rule(descriptor, scope, allowed);
        }

        Ok(allowed)
    }

    fn check_persistent_tool_permission(
        &self,
        descriptor: &ToolPermissionDescriptor,
    ) -> Option<bool> {
        let permissions_file = self.permissions_file.try_lock().ok()?;
        permissions_file.check_tool_permission(descriptor)
    }

    async fn ask_user_tool_permission(
        &self,
        descriptor: &ToolPermissionDescriptor,
    ) -> Result<(bool, Option<PermissionScope>)> {
        let request_id = self
            .request_counter
            .fetch_add(1, Ordering::SeqCst)
            .to_string();

        let event = crate::agent::AgentEvent::ToolPermissionRequest {
            descriptor: descriptor.clone(),
            request_id: request_id.clone(),
        };
        self.event_sender
            .send(event)
            .context("Failed to send tool permission request event")?;

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
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        Self::new(event_tx, response_rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ReadFileTool;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Helper function to create a PermissionManager for testing
    fn create_test_manager() -> PermissionManager {
        let (event_tx, _) = mpsc::unbounded_channel();
        let (_, response_rx) = mpsc::unbounded_channel();
        PermissionManager::new(event_tx, response_rx)
    }

    fn create_test_descriptor() -> ToolPermissionDescriptor {
        let tool = ReadFileTool::new();
        ToolPermissionBuilder::new(&tool, "test.txt")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap()
    }

    #[test]
    fn test_permission_manager_with_skip_permissions() {
        let manager = create_test_manager().with_skip_permissions(true);
        assert!(manager.skip_permissions());
        assert!(!manager.is_enforcing());
    }

    #[tokio::test]
    async fn test_check_tool_permission_with_skip() {
        let manager = create_test_manager().with_skip_permissions(true);
        let descriptor = create_test_descriptor();

        let result = manager.check_tool_permission(&descriptor).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_get_permissions_info_empty() {
        let manager = create_test_manager();
        let info = manager.get_permissions_info();
        assert_eq!(info.allow_count, 0);
        assert_eq!(info.deny_count, 0);
    }

    #[test]
    fn test_clear_all_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        let manager = create_test_manager()
            .with_project_root(project_root.clone())
            .unwrap();

        let descriptor = create_test_descriptor();

        // Add a permission
        let _ = manager.add_tool_permission_rule(
            &descriptor,
            &PermissionScope::Specific("test.txt".to_string()),
            true,
        );

        // Verify it was added
        let info = manager.get_permissions_info();
        assert_eq!(info.allow_count, 1);

        // Clear all permissions
        let result = manager.clear_all_permissions();
        assert!(result.is_ok());

        // Verify they were cleared
        let info = manager.get_permissions_info();
        assert_eq!(info.allow_count, 0);
        assert_eq!(info.deny_count, 0);
    }

    #[test]
    fn test_add_tool_permission_rule_specific() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        let manager = create_test_manager()
            .with_project_root(project_root.clone())
            .unwrap();

        let descriptor = create_test_descriptor();

        // Add allow rule
        let result = manager.add_tool_permission_rule(
            &descriptor,
            &PermissionScope::Specific("test.txt".to_string()),
            true,
        );
        assert!(result.is_ok());

        let info = manager.get_permissions_info();
        assert_eq!(info.allow_count, 1);
        assert_eq!(info.deny_count, 0);

        // Add deny rule
        let result = manager.add_tool_permission_rule(
            &descriptor,
            &PermissionScope::Specific("other.txt".to_string()),
            false,
        );
        assert!(result.is_ok());

        let info = manager.get_permissions_info();
        assert_eq!(info.allow_count, 1);
        assert_eq!(info.deny_count, 1);
    }

    #[test]
    fn test_add_tool_permission_rule_project_wide() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        let manager = create_test_manager()
            .with_project_root(project_root.clone())
            .unwrap();

        let descriptor = create_test_descriptor();

        // Add project-wide rule
        let result = manager.add_tool_permission_rule(
            &descriptor,
            &PermissionScope::ProjectWide(project_root.clone()),
            true,
        );
        assert!(result.is_ok());

        let info = manager.get_permissions_info();
        assert_eq!(info.allow_count, 1);
    }

    #[test]
    fn test_check_persistent_tool_permission() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        let manager = create_test_manager()
            .with_project_root(project_root.clone())
            .unwrap();

        let descriptor = create_test_descriptor();

        // Initially no persistent permission
        let result = manager.check_persistent_tool_permission(&descriptor);
        assert!(result.is_none());

        // Add a permission
        let _ = manager.add_tool_permission_rule(
            &descriptor,
            &PermissionScope::Specific("test.txt".to_string()),
            true,
        );

        // Now should have persistent permission
        let result = manager.check_persistent_tool_permission(&descriptor);
        assert!(result.is_some());
        assert!(result.unwrap());
    }

    #[test]
    fn test_permission_scope_variants() {
        let scope1 = PermissionScope::Specific("test".to_string());
        let scope2 = PermissionScope::Specific("test".to_string());
        let scope3 = PermissionScope::Specific("other".to_string());
        let scope4 = PermissionScope::ProjectWide(PathBuf::from("/project"));

        assert_eq!(scope1, scope2);
        assert_ne!(scope1, scope3);
        assert_ne!(scope1, scope4);
    }

    #[test]
    fn test_permissions_info_equality() {
        let info1 = PermissionsInfo {
            allow_count: 5,
            deny_count: 3,
        };
        let info2 = PermissionsInfo {
            allow_count: 5,
            deny_count: 3,
        };
        let info3 = PermissionsInfo {
            allow_count: 4,
            deny_count: 3,
        };

        assert_eq!(info1.allow_count, info2.allow_count);
        assert_eq!(info1.deny_count, info2.deny_count);
        assert_ne!(info1.allow_count, info3.allow_count);
    }

    #[test]
    fn test_save_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        let manager = create_test_manager()
            .with_project_root(project_root.clone())
            .unwrap();

        let descriptor = create_test_descriptor();

        // Add a permission
        let _ = manager.add_tool_permission_rule(
            &descriptor,
            &PermissionScope::Specific("test.txt".to_string()),
            true,
        );

        // Save should succeed
        let result = manager.save_permissions();
        assert!(result.is_ok());

        // Verify file was created
        let perms_path = storage::PermissionsFile::get_permissions_path(&project_root);
        assert!(perms_path.exists());
    }

    #[test]
    fn test_with_project_root() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        let manager = create_test_manager();
        let result = manager.with_project_root(project_root.clone());
        assert!(result.is_ok());

        let manager = result.unwrap();
        let info = manager.get_permissions_info();
        // Should start with empty permissions
        assert_eq!(info.allow_count, 0);
        assert_eq!(info.deny_count, 0);
    }

    #[test]
    fn test_multiple_permissions_same_tool() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        let manager = create_test_manager()
            .with_project_root(project_root.clone())
            .unwrap();

        let tool = ReadFileTool::new();
        let desc1 = ToolPermissionBuilder::new(&tool, "file1.txt")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap();
        let desc2 = ToolPermissionBuilder::new(&tool, "file2.txt")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap();

        // Add allow for file1
        let _ = manager.add_tool_permission_rule(
            &desc1,
            &PermissionScope::Specific("file1.txt".to_string()),
            true,
        );

        // Add deny for file2
        let _ = manager.add_tool_permission_rule(
            &desc2,
            &PermissionScope::Specific("file2.txt".to_string()),
            false,
        );

        let info = manager.get_permissions_info();
        assert_eq!(info.allow_count, 1);
        assert_eq!(info.deny_count, 1);
    }

    #[test]
    fn test_permission_persistence_across_managers() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        // First manager - add permission
        {
            let manager = create_test_manager()
                .with_project_root(project_root.clone())
                .unwrap();

            let descriptor = create_test_descriptor();
            let _ = manager.add_tool_permission_rule(
                &descriptor,
                &PermissionScope::Specific("test.txt".to_string()),
                true,
            );
        }

        // Second manager - should load existing permissions
        {
            let manager = create_test_manager()
                .with_project_root(project_root.clone())
                .unwrap();

            let info = manager.get_permissions_info();
            assert_eq!(info.allow_count, 1);
        }
    }
}
