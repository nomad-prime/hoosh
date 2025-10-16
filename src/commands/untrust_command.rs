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

        // Check if a project is currently trusted
        if permission_manager.get_trusted_project().is_some() {
            // Clear the trust
            permission_manager.clear_trusted_project();
            Ok(CommandResult::Success(
                "🔒 Project trust revoked. Permission dialogs will be shown again.".to_string(),
            ))
        } else {
            Ok(CommandResult::Success(
                "ℹ️ No project is currently trusted.".to_string(),
            ))
        }
    }
}
