use anyhow::Result;
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};

pub struct PermissionsCommand;

#[async_trait]
impl Command for PermissionsCommand {
    fn name(&self) -> &str {
        "permissions"
    }

    fn description(&self) -> &str {
        "Manage project permissions"
    }

    fn usage(&self) -> &str {
        "/permissions [list|reset] - View or manage permissions"
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let permission_manager = context
            .permission_manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Permission manager not available"))?;

        let subcommand = args.first().map(|s| s.as_str()).unwrap_or("list");

        match subcommand {
            "list" => {
                let mut output = String::from("Current permissions:\n\n");

                // Check if there's a permissions file loaded
                let perms_info = permission_manager.get_permissions_info();
                if perms_info.allow_count > 0 || perms_info.deny_count > 0 {
                    output.push_str(&format!("✓ Permissions loaded from storage\n"));
                    output.push_str(&format!("  Allow rules: {}\n", perms_info.allow_count));
                    output.push_str(&format!("  Deny rules: {}\n", perms_info.deny_count));
                } else {
                    output.push_str("✗ No persistent permissions saved\n");
                }

                if permission_manager.skip_permissions() {
                    output.push_str("\n⚠️  Permission checks are disabled (--skip-permissions)\n");
                }

                Ok(CommandResult::Success(output))
            }
            "reset" => {
                permission_manager.clear_all_permissions()?;
                Ok(CommandResult::Success(
                    "All permissions cleared. Future operations will require approval.".to_string(),
                ))
            }
            _ => Ok(CommandResult::Success(format!(
                "Unknown subcommand: {}\n{}",
                subcommand,
                self.usage()
            ))),
        }
    }
}
