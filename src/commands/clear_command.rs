use anyhow::Result;
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};

pub struct ClearCommand;

#[async_trait]
impl Command for ClearCommand {
    fn name(&self) -> &str {
        "clear"
    }

    fn description(&self) -> &str {
        "Clear conversation history"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["c"]
    }

    fn usage(&self) -> &str {
        "/clear\n\nClears the current conversation history, starting a fresh session."
    }

    async fn execute(
        &self,
        _args: Vec<String>,
        _context: &mut CommandContext,
    ) -> Result<CommandResult> {
        Ok(CommandResult::ClearConversation)
    }
}
