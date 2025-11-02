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

        let (allowed, scope) = match self.default_permission {
            PermissionLevel::Allow => (true, None),
            PermissionLevel::Deny => (false, None),
            PermissionLevel::Ask => self.ask_user_tool_permission(descriptor).await?,
        };

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

        let event = crate::conversations::AgentEvent::ToolPermissionRequest {
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
}
