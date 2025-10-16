use anyhow::Result;
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};

pub struct HelpCommand;

#[async_trait]
impl Command for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "Show available commands and usage"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["h", "?"]
    }

    fn usage(&self) -> &str {
        "/help [command]\n\nShow help for all commands or a specific command."
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let help_text = if args.is_empty() {
            if let Some(registry) = &context.command_registry {
                registry.get_help(None)
            } else {
                "Command registry not available".to_string()
            }
        } else if let Some(registry) = &context.command_registry {
            registry.get_help(Some(&args[0]))
        } else {
            "Command registry not available".to_string()
        };

        Ok(CommandResult::Success(help_text))
    }
}
