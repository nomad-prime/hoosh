use anyhow::Result;
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};

pub struct UntrustCommand;

#[async_trait]
impl Command for UntrustCommand {
    fn name(&self) -> &str {
        "untrust"
    }

    fn description(&self) -> &str {
        "Revoke project-wide trust for this session"
    }

    fn usage(&self) -> &str {
        "/untrust - Revoke project-wide trust and re-enable permission dialogs"
    }

    async fn execute(
        &self,
        _args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        // Check if we have a permission manager
        let permission_manager = context
            .permission_manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Permission manager not available"))?;

        // Clear all permissions (including global trust)
        let perms_info = permission_manager.get_permissions_info();

        if perms_info.allow_count > 0 || perms_info.deny_count > 0 {
            permission_manager.clear_all_permissions()?;
            Ok(CommandResult::Success(
                "🔒 All permissions cleared. Permission dialogs will be shown again.".to_string(),
            ))
        } else {
            Ok(CommandResult::Success(
                "ℹ️ No permissions are currently saved.".to_string(),
            ))
        }
    }
}
