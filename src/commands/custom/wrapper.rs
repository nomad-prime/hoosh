use crate::commands::custom::parser::ParsedCommand;
use crate::commands::{Command, CommandContext, CommandResult};
use anyhow::Result;
use async_trait::async_trait;

pub struct CustomCommandWrapper {
    command: ParsedCommand,
}

impl CustomCommandWrapper {
    pub fn new(command: ParsedCommand) -> Self {
        Self { command }
    }
}

#[async_trait]
impl Command for CustomCommandWrapper {
    fn name(&self) -> &str {
        &self.command.name
    }

    fn description(&self) -> &str {
        &self.command.metadata.description
    }

    fn aliases(&self) -> Vec<&str> {
        Vec::new()
    }

    fn usage(&self) -> &str {
        "Custom command - see description for details"
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let args_str = args.join(" ");
        let processed_body = self.command.body.replace("$ARGUMENTS", &args_str);

        if let Some(conversation) = &context.conversation {
            let mut conv = conversation.lock().await;
            conv.add_user_message(processed_body);
        }

        // Signal that agent should run on the added message
        Ok(CommandResult::RunAgent)
    }
}
