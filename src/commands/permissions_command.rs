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

                if let Some(project) = permission_manager.get_trusted_project() {
                    output.push_str(&format!(
                        "✓ Trusted project: {}\n",
                        project.display()
                    ));
                } else {
                    output.push_str("✗ Project not trusted\n");
                }

                if permission_manager.skip_permissions() {
                    output.push_str("⚠️  Permission checks are disabled (--skip-permissions)\n");
                }

                Ok(CommandResult::Success(output))
            }
            "reset" => {
                permission_manager.clear_cache();
                permission_manager.clear_trusted_project();
                Ok(CommandResult::Success(
                    "Permission cache cleared. All future operations will require approval.".to_string(),
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
