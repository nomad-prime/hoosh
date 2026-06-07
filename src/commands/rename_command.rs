use anyhow::{Result, anyhow};
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};

pub struct RenameCommand;

#[async_trait]
impl Command for RenameCommand {
    fn name(&self) -> &str {
        "rename"
    }

    fn description(&self) -> &str {
        "Set or clear the human-readable name of the current conversation"
    }

    fn usage(&self) -> &str {
        "/rename <name>\n/rename --clear\n\nSets a human-readable name on the current conversation \
         so it can be resumed later with `--resume <name>`. Pass `--clear` to remove the name."
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let conversation = context
            .conversation
            .as_ref()
            .ok_or_else(|| anyhow!("No active conversation"))?;

        let mut conv = conversation.lock().await;

        if !conv.has_storage() {
            return Ok(CommandResult::Success(
                "Conversation storage is disabled; names are not persisted.".to_string(),
            ));
        }

        if args.is_empty() {
            let current = conv.name().unwrap_or("<unnamed>").to_string();
            return Ok(CommandResult::Success(format!(
                "Current conversation name: {}",
                current
            )));
        }

        if args[0] == "--clear" || args[0] == "-c" {
            conv.set_name(None);
            return Ok(CommandResult::Success(
                "Conversation name cleared.".to_string(),
            ));
        }

        let new_name = args.join(" ");
        conv.set_name(Some(new_name.clone()));
        Ok(CommandResult::Success(format!(
            "Conversation renamed to '{}'.",
            new_name
        )))
    }
}
