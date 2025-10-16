use anyhow::Result;
use async_trait::async_trait;

use super::registry::{Command, CommandContext, CommandResult};

pub struct ExitCommand;

#[async_trait]
impl Command for ExitCommand {
    fn name(&self) -> &str {
        "exit"
    }

    fn description(&self) -> &str {
        "Exit the application"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["quit", "q"]
    }

    fn usage(&self) -> &str {
        "/exit\n\nExits the application."
    }

    async fn execute(
        &self,
        _args: Vec<String>,
        _context: &mut CommandContext,
    ) -> Result<CommandResult> {
        Ok(CommandResult::Exit)
    }
}
